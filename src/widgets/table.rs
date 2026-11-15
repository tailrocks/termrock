use ratatui::buffer::Buffer;
use std::collections::BTreeSet;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier};

use crate::core::data::{DataError, ShapeError, validate_rectangular};
use crate::core::event::{ClientPlatform, Key, Outcome};
use crate::core::focus::BlurAction;
use crate::core::hit::Axis;
use crate::core::hit::PartMap;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keyed::{KeyedSourceError, KeyedState, KeyedVec};
use crate::core::keymap::unassigned_chord;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup, select_all_bindings};
use crate::core::list::{Cursor, KeyStep, KeyedRowSource, Moved, Navigable, RowSource};
use crate::core::response::Response;
use crate::core::scroll::{HScroll, ScrollPane, Scrollable};
use crate::core::selection::{ClickPolicy, RowSelection, SelectGesture, SelectionMode, TriState};
use crate::ui::columns::{ColumnSizing, HeaderAction, HeaderLabels};
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::row::{MarkKind, RowLook, RowUi};
use crate::widgets::edit::{EditExit, EditField, Validator};
use crate::widgets::option_editor::{
    Affordance, ChoiceEditor, ChoiceResult, ColumnEditor, EditorConfig,
};
use crate::widgets::scrollbar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub title: String,
    pub width: Constraint,
    pub align: Align,
    pub editable: bool,
    pub sortable: bool,

    pub editor: ColumnEditor,
}

impl Column {
    pub fn new(title: &str, width: Constraint) -> Self {
        Self {
            title: title.to_owned(),
            width,
            align: Align::Left,
            editable: false,
            sortable: true,
            editor: ColumnEditor::Text,
        }
    }

    pub fn editor(mut self, editor: ColumnEditor) -> Self {
        self.editable = true;
        self.editor = editor;
        self
    }
    pub fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }
    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }
    pub fn min_width(&self) -> u16 {
        match self.width {
            Constraint::Length(n) | Constraint::Min(n) => n,
            _ => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub error: Option<String>,
    pub tone: Tone,
}

pub use crate::theme::Tone;

impl Cell {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: None,
            tone: Tone::Normal,
        }
    }
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl Cell {
    /// The borrowed facet the shared table engine reads.
    pub fn borrowed(&self) -> CellRef<'_> {
        CellRef {
            text: &self.text,
            error: self.error.as_deref(),
            tone: self.tone,
        }
    }
}

/// One table cell borrowed from its provider.
#[derive(Debug, Clone, Copy)]
pub struct CellRef<'a> {
    pub text: &'a str,
    pub error: Option<&'a str>,
    pub tone: Tone,
}

/// A keyed provider of table rows. `enabled(i)` of the inherited
/// [`RowSource`] says whether visible row `i` can be activated or selected.
/// Row order is the provider's: a borrowed table does not sort.
pub trait TableSource: KeyedRowSource {
    fn columns(&self) -> &[Column];
    fn cell(&self, visible_row: usize, column: usize) -> CellRef<'_>;
}

/// A [`TableSource`] that accepts committed edits by row key. A successful
/// `set_cell` must advance the provider's `data` revision.
pub trait TableEditSource: TableSource {
    fn set_cell(&mut self, row: ItemKey, column: usize, text: String) -> Result<(), String>;
}

const EMPTY_CELL: CellRef<'static> = CellRef {
    text: "",
    error: None,
    tone: Tone::Normal,
};

/// The rows the shared table engine works over.
trait TableRows {
    fn len(&self) -> usize;
    fn cell(&self, src: usize, col: usize) -> Option<CellRef<'_>>;
    /// Whether the provider, not the table's own set, disables this row.
    fn row_disabled(&self, _src: usize) -> bool {
        false
    }
    /// Whether committed edits have somewhere to go.
    fn editable(&self) -> bool {
        true
    }
    /// Whether the table may reorder rows by column.
    fn sortable(&self) -> bool {
        true
    }
    /// Identity of the row's hit regions at display position `display`.
    fn hit_key(&self, display: usize, _src: usize) -> ItemKey {
        ItemKey::index(display)
    }
    /// Stores validated text. `Err` keeps the editor open with that message.
    fn commit(&mut self, src: usize, col: usize, text: String) -> Result<(), String>;
    /// Marks a cell with a validation error where the provider keeps one.
    fn flag(&mut self, _src: usize, _col: usize, _error: Option<String>) {}
    /// The provider's current data revision, for rows that accept commits.
    fn data_revision(&self) -> Option<u64> {
        None
    }
}

struct OwnedRows(Vec<Vec<Cell>>);

impl TableRows for OwnedRows {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, src: usize, col: usize) -> Option<CellRef<'_>> {
        self.0.get(src)?.get(col).map(Cell::borrowed)
    }

    fn commit(&mut self, src: usize, col: usize, text: String) -> Result<(), String> {
        let cell = self
            .0
            .get_mut(src)
            .and_then(|row| row.get_mut(col))
            .ok_or_else(String::new)?;
        cell.text = text;
        cell.error = None;
        Ok(())
    }

    fn flag(&mut self, src: usize, col: usize, error: Option<String>) {
        if let Some(cell) = self.0.get_mut(src).and_then(|row| row.get_mut(col)) {
            cell.error = error;
        }
    }
}

struct ReadRows<'a, S: ?Sized>(&'a S);

impl<S: TableSource + ?Sized> TableRows for ReadRows<'_, S> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, src: usize, col: usize) -> Option<CellRef<'_>> {
        (src < self.0.len() && col < self.0.columns().len()).then(|| self.0.cell(src, col))
    }

    fn row_disabled(&self, src: usize) -> bool {
        !self.0.enabled(src)
    }

    fn editable(&self) -> bool {
        false
    }

    fn sortable(&self) -> bool {
        false
    }

    fn hit_key(&self, _display: usize, src: usize) -> ItemKey {
        self.0.key(src)
    }

    fn commit(&mut self, _src: usize, _col: usize, _text: String) -> Result<(), String> {
        Err("read-only table".to_owned())
    }
}

struct EditRows<'a, S: ?Sized>(&'a mut S);

impl<S: TableEditSource + ?Sized> TableRows for EditRows<'_, S> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, src: usize, col: usize) -> Option<CellRef<'_>> {
        (src < self.0.len() && col < self.0.columns().len()).then(|| self.0.cell(src, col))
    }

    fn row_disabled(&self, src: usize) -> bool {
        !self.0.enabled(src)
    }

    fn sortable(&self) -> bool {
        false
    }

    fn hit_key(&self, _display: usize, src: usize) -> ItemKey {
        self.0.key(src)
    }

    fn commit(&mut self, src: usize, col: usize, text: String) -> Result<(), String> {
        let key = self.0.key(src);
        let before = self.0.stamp();
        self.0.set_cell(key, col, text)?;
        let after = self.0.stamp();
        if after.instance != before.instance || after.data == before.data {
            return Err("the table source did not advance its data revision".to_owned());
        }
        Ok(())
    }

    fn data_revision(&self) -> Option<u64> {
        Some(self.0.stamp().data)
    }
}

enum Refused {
    Ineligible,
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct ChoiceEdit {
    pub row: usize,
    pub col: usize,
    pub editor: ChoiceEditor,
    generation: u64,
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub row: usize,
    pub col: usize,

    pub field: EditField,
    generation: u64,
}

impl crate::ui::measure::Measure for DataTable {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let n = self.columns.len();
        let check_w: u16 = if self.multi() && self.selection_column {
            4
        } else {
            0
        };

        let col = |i: usize| -> u16 {
            if let Some(w) = self.sizing.user(i) {
                return w;
            }
            let c = &self.columns[i];
            match c.width {
                Constraint::Length(n) => n,
                _ => {
                    let cells = self
                        .rows
                        .iter()
                        .filter_map(|r| r.get(i))
                        .map(|cell| crate::ui::text::width(&cell.text) as u16)
                        .max()
                        .unwrap_or(0);
                    c.min_width()
                        .max(crate::ui::text::width(&c.title) as u16 + 2)
                        .max(cells)
                }
            }
        };
        let cols: u16 = (0..n).map(col).sum::<u16>() + 2 * n.saturating_sub(1) as u16;
        ratatui::layout::Size::new(cols + 5 + check_w + 1, 1 + self.rows.len().max(1) as u16)
    }

    fn min_width(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> u16 {
        let check_w: u16 = if self.multi() && self.selection_column {
            4
        } else {
            0
        };
        let first = self.columns.first().map_or(6, Column::min_width);
        first + 5 + check_w + 1
    }
}

#[derive(Debug, Clone)]
pub struct DataTable {
    parts: PartMap,
    pub id: WidgetId,
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,

    order: Vec<usize>,
    pub sort: Option<(usize, SortDir)>,
    pub cursor_row: usize,
    pub cursor_col: usize,

    pub cell_nav: bool,
    pub selected: Option<usize>,

    pub scroll: ScrollPane,
    pub edit: Option<EditState>,

    pub validator: Option<Validator<(), usize>>,

    pub blur_action: BlurAction,
    pub empty_text: String,

    strings: crate::theme::Strings,

    pub empty: Option<crate::widgets::empty::EmptyState>,
    pub area: Rect,
    body: Rect,
    col_rects: Vec<Rect>,

    pub sizing: ColumnSizing,

    pub header_labels: HeaderLabels,

    pub numeric: Vec<bool>,

    pub reveal: Option<crate::ui::reveal::Reveal>,

    pub horizontal: Option<HScroll>,

    pub overflow_marks: bool,

    pub selection_mode: SelectionMode,

    pub selection: RowSelection,

    pub selection_column: bool,

    pub esc_clears: bool,

    pub toggle_mods: KeyModifiers,

    pub click: ClickPolicy,

    pub disabled: BTreeSet<usize>,

    pub nav: NavKeys,

    pub keymap: KeyMap<TableCmd>,

    pub type_ahead: bool,

    pub type_ahead_column: Option<usize>,

    pub type_ahead_precedence: Option<crate::core::list::TypeAheadPrecedence>,
    typeahead: crate::core::typeahead::TypeAhead,

    pub choice: Option<ChoiceEdit>,
    structural_generation: u64,

    pub editor_config: EditorConfig,

    keyed: KeyedState,
    layout_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCmd {
    SortByColumn,

    SelectAll,
}

impl Actions for TableCmd {
    fn label(self) -> &'static str {
        match self {
            TableCmd::SortByColumn => "Sort",
            TableCmd::SelectAll => "Select all",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        let mut map = vec![(KeyBinding::char('s'), TableCmd::SortByColumn)];
        map.extend(select_all_bindings(TableCmd::SelectAll));
        map
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAction<K = usize> {
    Committed { row: K, col: usize },
    Cancelled,
    Activated(K),

    SelectionChanged,

    LeaveForward,
    LeaveBackward,
}

impl TableAction<usize> {
    fn keyed(self, key: impl Fn(usize) -> ItemKey) -> TableAction<ItemKey> {
        match self {
            TableAction::Committed { row, col } => TableAction::Committed { row: key(row), col },
            TableAction::Cancelled => TableAction::Cancelled,
            TableAction::Activated(row) => TableAction::Activated(key(row)),
            TableAction::SelectionChanged => TableAction::SelectionChanged,
            TableAction::LeaveForward => TableAction::LeaveForward,
            TableAction::LeaveBackward => TableAction::LeaveBackward,
        }
    }
}

impl DataTable {
    pub fn try_new(
        id: WidgetId,
        columns: Vec<Column>,
        rows: Vec<Vec<Cell>>,
    ) -> Result<Self, DataError> {
        validate_rectangular(columns.len(), &rows)?;
        let n = rows.len();
        let numeric = vec![false; columns.len()];
        Ok(Self {
            id,
            columns,
            rows,
            order: (0..n).collect(),
            sort: None,
            cursor_row: 0,
            cursor_col: 0,
            cell_nav: false,
            selected: None,
            scroll: ScrollPane::new(n).columns(true),
            edit: None,
            validator: None,
            blur_action: BlurAction::Commit,
            empty_text: crate::theme::Strings::DEFAULT.no_rows.to_owned(),
            strings: crate::theme::Strings::DEFAULT,
            empty: None,
            area: Rect::ZERO,
            parts: PartMap::default(),
            body: Rect::ZERO,
            col_rects: vec![],
            sizing: ColumnSizing::default(),
            header_labels: HeaderLabels::DEFAULT,
            numeric,
            reveal: None,
            horizontal: None,
            overflow_marks: true,
            selection_mode: SelectionMode::Single,
            selection: RowSelection::new(),
            selection_column: false,
            esc_clears: true,
            toggle_mods: Self::toggle_mods_for(ClientPlatform::current()),
            click: ClickPolicy::DATA,
            disabled: BTreeSet::new(),
            nav: NavKeys::default(),
            keymap: TableCmd::default_keymap(),
            type_ahead: false,
            type_ahead_column: None,
            type_ahead_precedence: None,
            typeahead: Default::default(),
            choice: None,
            structural_generation: 0,
            editor_config: EditorConfig::default(),
            keyed: KeyedState::new(),
            layout_generation: 0,
        })
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn rows(&self) -> &[Vec<Cell>] {
        &self.rows
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.typeahead.timeout = timeout;
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.type_ahead = on;
        self
    }

    pub fn type_ahead_precedence(
        mut self,
        precedence: crate::core::list::TypeAheadPrecedence,
    ) -> Self {
        self.type_ahead_precedence = Some(precedence);
        self
    }

    pub fn type_ahead_column(mut self, col: Option<usize>) -> Self {
        self.type_ahead_column = col;
        self
    }

    pub fn editor_config(mut self, cfg: EditorConfig) -> Self {
        self.editor_config = cfg;
        self
    }

    pub fn choice_id(&self) -> WidgetId {
        self.id.sub("choice")
    }

    pub fn open_id(&self, display: usize, col: usize) -> WidgetId {
        self.cell_id(display, col).part(PartRef::of(Part::ACTIONS))
    }

    pub fn out_of_set(&self, src: usize, col: usize) -> bool {
        let (Some(cell), Some(c)) = (self.cell(src, col), self.columns.get(col)) else {
            return false;
        };
        c.editor.out_of_set(&cell.text, &self.editor_config)
    }

    pub fn out_of_set_cells(&self) -> Vec<(usize, usize)> {
        (0..self.rows.len())
            .flat_map(|r| (0..self.columns.len()).map(move |c| (r, c)))
            .filter(|&(r, c)| self.out_of_set(r, c))
            .collect()
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<TableCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn selection_column(mut self, on: bool) -> Self {
        self.selection_column = on;
        self
    }

    pub fn esc_clears(mut self, on: bool) -> Self {
        self.esc_clears = on;
        self
    }

    pub const TOGGLE_CTRL_CMD: KeyModifiers = SelectGesture::TOGGLE_CTRL_CMD;

    pub const TOGGLE_OPTION_CMD: KeyModifiers = SelectGesture::TOGGLE_OPTION_CMD;

    pub const TOGGLE_WITH_ALT: KeyModifiers = SelectGesture::TOGGLE_WITH_ALT;

    pub const fn toggle_mods_for(platform: ClientPlatform) -> KeyModifiers {
        SelectGesture::toggle_mods_for(platform)
    }

    pub fn toggle_mods(mut self, mods: KeyModifiers) -> Self {
        self.toggle_mods = mods;
        self
    }

    pub fn click(mut self, policy: ClickPolicy) -> Self {
        self.click = policy;
        self
    }

    pub fn disabled_rows(mut self, rows: &[usize]) -> Self {
        self.disabled.extend(rows.iter().copied());
        self
    }

    pub fn set_row_disabled(&mut self, src: usize, disabled: bool) {
        if disabled {
            self.disabled.insert(src);
            if self.edit.as_ref().is_some_and(|e| e.row == src) {
                self.edit = None;
            }
            if self.choice.as_ref().is_some_and(|c| c.row == src) {
                self.choice = None;
            }
            self.selection.retain(|r| r != src);
            if self.selected == Some(src) {
                self.selected = None;
            }
        } else {
            self.disabled.remove(&src);
        }
    }

    pub fn is_disabled(&self, src: usize) -> bool {
        self.disabled.contains(&src)
    }

    fn is_disabled_in<R: TableRows + ?Sized>(&self, rows: &R, src: usize) -> bool {
        self.disabled.contains(&src) || rows.row_disabled(src)
    }

    /// Runs `f` over the owned rows without copying them.
    fn with_rows<T>(&mut self, f: impl FnOnce(&mut Self, &mut OwnedRows) -> T) -> T {
        let mut rows = OwnedRows(std::mem::take(&mut self.rows));
        let out = f(self, &mut rows);
        self.rows = rows.0;
        out
    }

    fn multi(&self) -> bool {
        self.selection_mode == SelectionMode::Multi
    }

    pub fn selected_rows(&self) -> Vec<usize> {
        match self.selection_mode {
            SelectionMode::Multi => self.selection.to_vec(),
            SelectionMode::Single => self.selected.into_iter().collect(),
            SelectionMode::None => vec![],
        }
    }

    pub fn selection_label(&self) -> String {
        match self.selected_rows().len() {
            0 => String::new(),
            n => format!("{n} selected"),
        }
    }

    pub fn selection_state(&self) -> TriState {
        let disabled = &self.disabled;
        self.selection
            .tri_state(0..self.rows.len(), |r| !disabled.contains(&r))
    }

    fn selection_state_in<R: TableRows + ?Sized>(&self, rows: &R) -> TriState {
        let disabled = &self.disabled;
        self.selection.tri_state(0..rows.len(), |r| {
            !disabled.contains(&r) && !rows.row_disabled(r)
        })
    }

    pub fn select_all(&mut self) -> bool {
        self.with_rows(|me, rows| me.select_all_in(rows))
    }

    fn select_all_in<R: TableRows + ?Sized>(&mut self, rows: &R) -> bool {
        if !self.multi() {
            return false;
        }
        let disabled = &self.disabled;
        self.selection.select_all(self.order.iter().copied(), |r| {
            !disabled.contains(&r) && !rows.row_disabled(r)
        })
    }

    pub fn clear_selection(&mut self) -> bool {
        let single = self.selected.take().is_some();
        self.selection.clear() || single
    }

    pub fn context_selection(&mut self, display: usize) -> bool {
        if !self.multi() || self.cell_nav {
            return false;
        }
        let disabled = &self.disabled;
        self.selection
            .context_select(display, &self.order, |r| !disabled.contains(&r))
    }

    pub fn toggle_row(&mut self, src: usize) -> bool {
        self.with_rows(|me, rows| me.toggle_row_in(rows, src))
    }

    fn toggle_row_in<R: TableRows + ?Sized>(&mut self, rows: &R, src: usize) -> bool {
        let disabled = &self.disabled;
        src < rows.len()
            && self
                .selection
                .toggle(src, |r| !disabled.contains(&r) && !rows.row_disabled(r))
    }

    pub fn try_set_row(&mut self, src: usize, cells: Vec<Cell>) -> Result<(), DataError> {
        if src >= self.rows.len() {
            return Err(DataError::MissingRow(src));
        }
        if cells.len() != self.columns.len() {
            return Err(DataError::Shape(ShapeError {
                row: src,
                expected: self.columns.len(),
                actual: cells.len(),
            }));
        }
        let cursor_src = self.order.get(self.cursor_row).copied();
        self.structural_change();
        self.rows[src] = cells;
        if let Some((c, d)) = self.sort {
            self.apply_sort(c, d);
            if let Some(pos) = cursor_src.and_then(|s| self.order.iter().position(|&r| r == s)) {
                self.cursor_row = pos;
                self.scroll.ensure_visible(pos);
            }
        }
        Ok(())
    }

    pub fn remove_rows(&mut self, srcs: &[usize]) {
        let gone: BTreeSet<usize> = srcs
            .iter()
            .copied()
            .filter(|&r| r < self.rows.len())
            .collect();
        if gone.is_empty() {
            return;
        }
        let map = |r: usize| (!gone.contains(&r)).then(|| r - gone.range(..r).count());
        let cursor_src = self.order.get(self.cursor_row).copied();
        let len = self.rows.len();
        self.structural_change();
        self.selection.remap(map);
        self.disabled = self.disabled.iter().filter_map(|&r| map(r)).collect();
        self.selected = self.selected.and_then(map);
        let mut i = 0;
        self.rows.retain(|_| {
            let keep = !gone.contains(&i);
            i += 1;
            keep
        });
        self.order = (0..self.rows.len()).collect();
        self.scroll.set_content(self.rows.len());
        if let Some((c, d)) = self.sort {
            self.apply_sort(c, d);
        }

        let next = cursor_src.and_then(|c| {
            (c..len)
                .find_map(map)
                .or_else(|| (0..c).rev().find_map(map))
        });
        self.cursor_row = next
            .and_then(|s| self.order.iter().position(|&r| r == s))
            .unwrap_or(0)
            .min(self.rows.len().saturating_sub(1));
        self.scroll.ensure_visible(self.cursor_row);
    }

    pub fn check_id(&self, display: usize) -> WidgetId {
        self.id.part(Self::check_part(display))
    }

    fn check_part(display: usize) -> PartRef {
        PartRef::item(Part::MARKER, ItemKey::index(display))
    }

    pub fn check_all_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::MARKER))
    }

    pub fn on_click_check(&mut self, display: usize) -> (Outcome, Option<TableAction>) {
        self.with_rows(|me, rows| me.on_click_check_in(rows, display))
    }

    fn on_click_check_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &R,
        display: usize,
    ) -> (Outcome, Option<TableAction>) {
        if display >= rows.len() || !self.multi() {
            return (Outcome::Consumed, None);
        }
        self.point_cursor(rows.len(), display, self.cursor_col);
        let src = self.order[display];
        if self.toggle_row_in(rows, src) {
            (Outcome::Changed, Some(TableAction::SelectionChanged))
        } else {
            (Outcome::Changed, None)
        }
    }

    pub fn on_click_check_all(&mut self) -> (Outcome, Option<TableAction>) {
        self.with_rows(|me, rows| me.on_click_check_all_in(rows))
    }

    fn on_click_check_all_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &R,
    ) -> (Outcome, Option<TableAction>) {
        if !self.multi() {
            return (Outcome::Consumed, None);
        }
        let disabled = &self.disabled;
        if self.selection.toggle_all(self.order.iter().copied(), |r| {
            !disabled.contains(&r) && !rows.row_disabled(r)
        }) {
            (Outcome::Changed, Some(TableAction::SelectionChanged))
        } else {
            (Outcome::Consumed, None)
        }
    }

    fn on_key_multi<R: TableRows + ?Sized>(
        &mut self,
        rows: &R,
        key: &Key,
    ) -> Option<(Outcome, Option<TableAction>)> {
        let changed = |c: bool| {
            if c {
                (Outcome::Changed, Some(TableAction::SelectionChanged))
            } else {
                (Outcome::Consumed, None)
            }
        };
        let command = key.ctrl() || key.cmd();
        match key.code {
            KeyCode::Char(' ') if key.mods.is_empty() => {
                let src = self.order[self.cursor_row];
                Some(changed(self.toggle_row_in(rows, src)))
            }
            KeyCode::Esc
                if key.mods.is_empty() && self.esc_clears && !self.selection.is_empty() =>
            {
                self.selection.clear();
                Some((Outcome::Changed, Some(TableAction::SelectionChanged)))
            }
            _ if key.shift()
                && !command
                && !key.alt()
                && matches!(
                    self.nav.action(key),
                    Some(Nav::Up | Nav::Down | Nav::Home | Nav::End | Nav::PageUp | Nav::PageDown)
                ) =>
            {
                let from = self.cursor_row;
                let to = self
                    .nav
                    .action(key)
                    .and_then(|nav| {
                        self.cursor()
                            .apply(nav, &View { table: self, rows }, self.scroll.viewport_len)
                            .index()
                    })
                    .unwrap_or(from);
                self.set_cursor(rows.len(), to, self.cursor_col);
                let disabled = &self.disabled;
                let c = self.selection.apply(
                    SelectGesture::Extend { from },
                    self.cursor_row,
                    &self.order,
                    |r| !disabled.contains(&r) && !rows.row_disabled(r),
                );
                Some((Outcome::Changed, c.then_some(TableAction::SelectionChanged)))
            }
            _ => None,
        }
    }

    pub fn horizontal(mut self, policy: HScroll) -> Self {
        self.horizontal = Some(policy);
        self
    }

    pub fn overflow_marks(mut self, on: bool) -> Self {
        self.overflow_marks = on;
        self
    }

    pub fn resizable_columns(mut self, on: bool) -> Self {
        self.sizing.resizable = on;
        self
    }

    pub fn seam_hit_w(mut self, cells: u16) -> Self {
        self.sizing.seam_w = cells.max(1);
        self
    }

    pub fn seam_id(&self, c: usize) -> WidgetId {
        self.id.part(crate::ui::columns::seam_part(c))
    }

    fn seam_of(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::SEAM, 0..self.columns.len())
    }

    pub fn header_menu_at(&self, ev: &crate::core::pointer::PointerEvent) -> Option<usize> {
        if !ev.on(self.id) {
            return None;
        }
        ev.index_of(Part::HEADER)
            .or_else(|| ev.index_of(Part::SEAM))
    }

    pub fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        let Some(c) = self.seam_of(id) else {
            return Outcome::Ignored;
        };
        if !self.sizing.resizable {
            return Outcome::Consumed;
        }
        if count >= 2 {
            self.sizing.release();
            self.autofit(c);
            return Outcome::Changed;
        }
        let w = self.col_rects.get(c).map_or(0, |r| r.width);
        self.sizing.press(c, w, pos.x);
        Outcome::Changed
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if self.seam_of(pressed).is_some() {
            return match self.sizing.drag_to(pos.x) {
                Some(_) => Outcome::Changed,
                None => Outcome::Consumed,
            };
        }
        if scrollbar::is_bar_of(pressed, self.id) {
            return match self.pane().bar_of(pressed) {
                Some(bar) => self.bar_input(&bar, pos, false).outcome(),
                None => Outcome::Consumed,
            };
        }
        Outcome::Ignored
    }

    pub fn autofit(&mut self, c: usize) {
        let Some(col) = self.columns.get(c) else {
            return;
        };
        let cap = (self.area.width.saturating_sub(4) as usize).max(6);
        let mut need = crate::ui::text::width(&col.title)
            + if self.sort.is_some_and(|(sc, _)| sc == c) {
                2
            } else {
                0
            };
        for d in self.scroll.visible_range() {
            if let Some(cell) = self.order.get(d).and_then(|&r| self.rows[r].get(c)) {
                need = need.max(crate::ui::text::width_capped(&cell.text, cap));
            }
        }
        self.sizing.set(c, need.min(cap) as u16);
    }

    pub fn header_menu(&self, c: usize) -> Vec<(HeaderAction, crate::widgets::menu::MenuItem)> {
        let sortable = self.columns.get(c).is_some_and(|col| col.sortable);
        let sorted = self.sort.and_then(|(sc, d)| (sc == c).then_some(d));
        crate::ui::columns::header_menu(
            &self.header_labels,
            sortable,
            sorted,
            self.sizing.resizable,
            !self.sizing.is_default(),
        )
    }

    pub fn on_header_action(&mut self, c: usize, action: HeaderAction) -> Outcome {
        let want = match action {
            HeaderAction::SortAsc => Some(Some((c, SortDir::Asc))),
            HeaderAction::SortDesc => Some(Some((c, SortDir::Desc))),
            HeaderAction::ClearSort => Some(None),
            HeaderAction::Autofit => {
                self.autofit(c);
                return Outcome::Changed;
            }
            HeaderAction::ResetWidths => {
                self.sizing.reset();
                return Outcome::Changed;
            }
        };
        if let Some(want) = want {
            if c >= self.columns.len() || !self.columns[c].sortable {
                return Outcome::Consumed;
            }

            for _ in 0..3 {
                if self.sort == want {
                    break;
                }
                self.sort_by(c);
            }
        }
        Outcome::Changed
    }

    pub fn reveal(mut self, config: crate::ui::reveal::Reveal) -> Self {
        self.reveal = Some(config);
        self
    }

    pub fn cell_nav(mut self, on: bool) -> Self {
        self.cell_nav = on;
        self
    }
    pub fn numeric(mut self, cols: &[usize]) -> Self {
        for &c in cols {
            if c < self.numeric.len() {
                self.numeric[c] = true;
            }
        }
        self
    }

    pub fn validator(mut self, v: impl Fn(usize, &str) -> Option<String> + 'static) -> Self {
        self.validator = Some(Validator::with_context(move |col: &usize, text: &str| {
            v(*col, text).map_or(Ok(()), |m| Err(m.into()))
        }));
        self
    }

    pub fn empty(mut self, state: crate::widgets::empty::EmptyState) -> Self {
        self.empty = Some(state);
        self
    }

    pub fn empty_text(mut self, s: &str) -> Self {
        self.empty_text = s.to_owned();
        self
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn is_editing(&self) -> bool {
        self.edit.is_some() || self.choice.is_some()
    }

    pub fn source_row(&self, i: usize) -> usize {
        self.order[i]
    }

    pub fn header_id(&self, col: usize) -> WidgetId {
        self.id
            .part(PartRef::item(Part::HEADER, ItemKey::index(col)))
    }
    pub fn row_id(&self, display: usize) -> WidgetId {
        self.id.part(Self::row_part(display))
    }
    fn row_part(display: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(display))
    }

    pub fn cell_id(&self, display: usize, col: usize) -> WidgetId {
        self.row_id(display).child(col)
    }

    pub fn try_set_rows(&mut self, rows: Vec<Vec<Cell>>) -> Result<(), DataError> {
        validate_rectangular(self.columns.len(), &rows)?;
        self.install_rows(rows);
        Ok(())
    }

    pub fn try_set_model(
        &mut self,
        columns: Vec<Column>,
        rows: Vec<Vec<Cell>>,
    ) -> Result<(), DataError> {
        validate_rectangular(columns.len(), &rows)?;
        let same_column = |c: usize| self.columns.get(c) == columns.get(c);
        self.numeric = (0..columns.len())
            .map(|c| same_column(c) && self.numeric.get(c).copied().unwrap_or(false))
            .collect();
        self.sort = self
            .sort
            .filter(|&(c, _)| c < columns.len() && same_column(c));
        self.sizing.reset();
        self.cursor_col = self.cursor_col.min(columns.len().saturating_sub(1));
        self.columns = columns;
        self.install_rows(rows);
        Ok(())
    }

    pub fn update_column(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut Column),
    ) -> Result<bool, usize> {
        let Some(original) = self.columns.get(index) else {
            return Err(index);
        };
        let mut candidate = original.clone();
        edit(&mut candidate);
        if candidate == *original {
            return Ok(false);
        }
        let resized = candidate.width != original.width;
        let reedits =
            candidate.editable != original.editable || candidate.editor != original.editor;
        self.columns[index] = candidate;
        if resized {
            self.sizing.clear(index);
        }
        if !self.columns[index].sortable && self.sort.is_some_and(|(c, _)| c == index) {
            let at = self.order.get(self.cursor_row).copied();
            self.sort = None;
            self.order = (0..self.rows.len()).collect();
            if let Some(src) = at {
                self.cursor_row = src;
                self.scroll.ensure_visible(src);
            }
        }
        if reedits {
            if self.edit.as_ref().is_some_and(|e| e.col == index) {
                self.edit = None;
            }
            if self.choice.as_ref().is_some_and(|c| c.col == index) {
                self.choice = None;
            }
        }
        Ok(true)
    }

    pub fn update_cell(&mut self, src: usize, col: usize, edit: impl FnOnce(&mut Cell)) -> bool {
        let Some(cell) = self.cell_mut(src, col) else {
            return false;
        };
        edit(cell);
        if let Some((c, d)) = self.sort
            && c == col
        {
            let at = self.order.get(self.cursor_row).copied();
            self.apply_sort(c, d);
            if let Some(src) = at
                && let Some(pos) = self.order.iter().position(|&r| r == src)
            {
                self.cursor_row = pos;
                self.scroll.ensure_visible(pos);
            }
        }
        true
    }

    fn cell(&self, src: usize, col: usize) -> Option<&Cell> {
        self.rows.get(src)?.get(col)
    }

    fn cell_mut(&mut self, src: usize, col: usize) -> Option<&mut Cell> {
        self.rows.get_mut(src)?.get_mut(col)
    }

    fn install_rows(&mut self, rows: Vec<Vec<Cell>>) {
        self.structural_change();
        self.selected = None;
        self.selection.clear();
        self.disabled.retain(|&r| r < rows.len());
        self.rows = rows;
        self.order = (0..self.rows.len()).collect();
        self.scroll.set_content(self.rows.len());
        if let Some((c, d)) = self.sort {
            self.apply_sort(c, d);
        }
        self.cursor_row = self.cursor_row.min(self.rows.len().saturating_sub(1));
    }

    fn apply_sort(&mut self, col: usize, dir: SortDir) {
        self.with_rows(|me, rows| me.apply_sort_in(rows, col, dir));
    }

    fn apply_sort_in<R: TableRows + ?Sized>(&mut self, rows: &R, col: usize, dir: SortDir) {
        let numeric = self.numeric.get(col).copied().unwrap_or(false);
        let text = |src: usize| rows.cell(src, col).map_or("", |c| c.text);
        self.order.sort_by(|&a, &b| {
            let ca = text(a);
            let cb = text(b);
            let ord = if numeric {
                parse_num(ca).cmp(&parse_num(cb))
            } else {
                ca.to_lowercase().cmp(&cb.to_lowercase())
            };
            let ord = ord.then_with(|| a.cmp(&b));
            match dir {
                SortDir::Asc => ord,
                SortDir::Desc => ord.reverse(),
            }
        });
    }

    pub fn sort_by(&mut self, col: usize) -> Outcome {
        self.with_rows(|me, rows| me.sort_by_in(rows, col))
    }

    fn sort_by_in<R: TableRows + ?Sized>(&mut self, rows: &R, col: usize) -> Outcome {
        if col >= self.columns.len() || !self.columns[col].sortable || !rows.sortable() {
            return Outcome::Consumed;
        }

        let src = self.order.get(self.cursor_row).copied();
        self.sort = match self.sort {
            Some((c, SortDir::Asc)) if c == col => Some((col, SortDir::Desc)),
            Some((c, SortDir::Desc)) if c == col => None,
            _ => Some((col, SortDir::Asc)),
        };
        match self.sort {
            Some((c, d)) => self.apply_sort_in(rows, c, d),
            None => self.order = (0..rows.len()).collect(),
        }
        if let Some(src) = src
            && let Some(pos) = self.order.iter().position(|&r| r == src)
        {
            self.cursor_row = pos;
            self.scroll.ensure_visible(pos);
        }
        Outcome::Changed
    }

    fn clamp_cursor(&mut self, len: usize) {
        self.cursor_row = self.cursor_row.min(len.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(self.columns.len().saturating_sub(1));
    }

    fn set_cursor(&mut self, len: usize, row: usize, col: usize) {
        self.cursor_row = row.min(len.saturating_sub(1));
        self.cursor_col = col.min(self.columns.len().saturating_sub(1));
        self.scroll.ensure_visible(self.cursor_row);
        self.ensure_col_visible();
    }

    fn point_cursor(&mut self, len: usize, row: usize, col: usize) {
        self.scroll.pointer_move();
        self.set_cursor(len, row, col);
    }

    fn ensure_col_visible(&mut self) {
        self.scroll.reveal_column(self.cursor_col);
    }

    pub fn begin_edit(&mut self) -> Response<TableAction> {
        self.start_edit()
    }

    /// Explicit activation of the cursor cell (Enter, F2, double click):
    /// toggle editors flip their value and report the commit.
    pub fn start_edit(&mut self) -> Response<TableAction> {
        self.with_rows(|me, rows| me.start_edit_in(rows))
    }

    fn start_edit_in<R: TableRows + ?Sized>(&mut self, rows: &mut R) -> Response<TableAction> {
        self.open_editor(rows, false)
    }

    /// Passive arrival at a cell (Tab traversal): text and choice editors
    /// open, but a toggle editor is only revealed, never flipped.
    fn begin_edit_for_traversal<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
    ) -> Response<TableAction> {
        self.open_editor(rows, true)
    }

    fn open_editor<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        traversal: bool,
    ) -> Response<TableAction> {
        self.clamp_cursor(rows.len());
        let col = self.cursor_col;
        let Some(&src) = self.order.get(self.cursor_row) else {
            return Response::consumed();
        };
        if rows.len() == 0 || !self.editable_target(rows, src, col) {
            return Response::consumed();
        }

        self.scroll.ensure_visible(self.cursor_row);
        self.ensure_col_visible();
        let Some(text) = rows.cell(src, col).map(|c| c.text.to_owned()) else {
            return Response::consumed();
        };
        let editor = &self.columns[col].editor;
        if let Some(next) = editor.toggled(&text) {
            if traversal {
                return Response::changed();
            }
            return match self.apply_value(rows, src, col, next) {
                Ok(Some(action)) => Response::emit(action),
                Ok(None) => Response::changed(),
                Err(Refused::Invalid(msg)) => {
                    rows.flag(src, col, Some(msg));
                    Response::changed()
                }
                Err(Refused::Ineligible) => Response::consumed(),
            };
        }
        if editor.is_constrained() {
            let opened = ChoiceEditor::open(self.choice_id(), editor, &text, &self.editor_config);
            self.choice = opened.map(|editor| ChoiceEdit {
                row: src,
                col,
                editor,
                generation: self.structural_generation,
            });
            return Response::changed();
        }
        let mut field = EditField::single(text);
        field.validator = self.validator.as_ref().map(|v| v.bind(col));
        self.edit = Some(EditState {
            row: src,
            col,
            field,
            generation: self.structural_generation,
        });
        Response::changed()
    }

    fn editable_target<R: TableRows + ?Sized>(&self, rows: &R, src: usize, col: usize) -> bool {
        rows.editable()
            && !rows.row_disabled(src)
            && !self.is_disabled(src)
            && self.columns.get(col).is_some_and(|c| c.editable)
            && rows.cell(src, col).is_some()
    }

    /// Validates and stores `text`. `Ok(None)` is a valid no-op: the text equals
    /// the stored value, so the target error clears but no commit is reported.
    fn apply_value<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        src: usize,
        col: usize,
        text: String,
    ) -> Result<Option<TableAction>, Refused> {
        if !self.editable_target(rows, src, col) {
            return Err(Refused::Ineligible);
        }
        if let Some(Err(err)) = self.validator.as_ref().map(|v| v.validate_in(&col, &text)) {
            return Err(Refused::Invalid(err.into_message()));
        }
        let Some(cell) = rows.cell(src, col) else {
            return Err(Refused::Ineligible);
        };
        if cell.text == text {
            rows.flag(src, col, None);
            return Ok(None);
        }
        rows.commit(src, col, text).map_err(Refused::Invalid)?;
        if let Some((c, d)) = self.sort
            && c == col
        {
            self.apply_sort_in(rows, c, d);
            if let Some(pos) = self.order.iter().position(|&r| r == src) {
                self.cursor_row = pos;
                self.scroll.ensure_visible(pos);
            }
        }
        Ok(Some(TableAction::Committed { row: src, col }))
    }

    fn on_choice<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        r: Option<ChoiceResult>,
    ) -> Response<TableAction> {
        let Some(r) = r else {
            return Response::ignored();
        };
        let (value, tab) = match r {
            ChoiceResult::Cancel => {
                self.choice = None;
                return Response::emit(TableAction::Cancelled);
            }
            ChoiceResult::Commit(v) => (v, None),
            ChoiceResult::CommitAndTab { value, backward } => (value, Some(backward)),
        };
        let Some((src, col, generation)) =
            self.choice.as_ref().map(|c| (c.row, c.col, c.generation))
        else {
            return Response::ignored();
        };
        if !self.session_target_live(generation) {
            self.choice = None;
            return Response::emit(TableAction::Cancelled);
        }
        match self.apply_value(rows, src, col, value) {
            Err(Refused::Ineligible) => {
                self.choice = None;
                Response::emit(TableAction::Cancelled)
            }
            Ok(committed) => {
                self.choice = None;
                let mut resp = Response::changed();
                if let Some(action) = committed {
                    resp.push_action(action);
                }
                match tab {
                    Some(backward) => resp.merge(self.tab_after_commit(rows, backward)),
                    None => resp,
                }
            }
            Err(Refused::Invalid(msg)) => {
                if let Some(c) = self.choice.as_mut() {
                    c.editor.error = Some(msg);
                }
                Response::ignored()
            }
        }
    }

    /// Folds an editor-level outcome and every result it carries through
    /// [`Self::on_choice`], in order.
    fn on_choices<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        out: Outcome,
        results: crate::core::response::Actions<ChoiceResult>,
    ) -> Response<TableAction> {
        let mut resp = Response::from(out);
        for result in results {
            resp |= self.on_choice(rows, Some(result));
        }
        resp
    }

    /// After a successful commit: land on the next editable cell, or report
    /// that traversal left the table at its edge.
    fn tab_after_commit<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        backward: bool,
    ) -> Response<TableAction> {
        match self.tab_to_editable(rows.len(), backward) {
            Some(leave) => Response::emit(leave),
            None => self.begin_edit_for_traversal(rows),
        }
    }

    fn tab_to_editable(&mut self, len: usize, backward: bool) -> Option<TableAction> {
        let n = self.columns.len();
        let mut col = self.cursor_col;
        loop {
            if backward {
                if col == 0 {
                    return Some(TableAction::LeaveBackward);
                }
                col -= 1;
            } else {
                col += 1;
                if col >= n {
                    return Some(TableAction::LeaveForward);
                }
            }
            if self.columns[col].editable {
                break;
            }
        }
        self.set_cursor(len, self.cursor_row, col);
        None
    }

    pub fn on_click_editor(&mut self, id: WidgetId) -> Option<Response<TableAction>> {
        self.with_rows(|me, rows| me.on_click_editor_in(rows, id))
    }

    fn on_click_editor_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        id: WidgetId,
    ) -> Option<Response<TableAction>> {
        if let Some(c) = self.choice.as_mut()
            && c.editor.contains_id(id)
        {
            let (out, r) = c.editor.on_click(id);
            return Some(Response::from(out) | self.on_choice(rows, r));
        }
        let (di, ci) = self.parts.cell(id, Part::ACTIONS)?;
        self.point_cursor(rows.len(), di, ci);
        Some(self.start_edit_in(rows))
    }

    /// Commits the open text edit. `None` means either the validator kept the
    /// editor open (`is_editing` stays true) or the text was unchanged; neither
    /// reports a commit.
    pub fn commit_edit(&mut self) -> Option<TableAction> {
        self.with_rows(|me, rows| me.commit_edit_in(rows))
    }

    fn commit_edit_in<R: TableRows + ?Sized>(&mut self, rows: &mut R) -> Option<TableAction> {
        let mut e = self.edit.take()?;
        let src = e.row;
        if !self.session_target_live(e.generation) || !self.editable_target(rows, e.row, e.col) {
            return Some(TableAction::Cancelled);
        }
        if !e.field.validate() {
            self.edit = Some(e);
            return None;
        }
        let text = e.field.text().to_owned();
        let cell = rows.cell(src, e.col)?;
        if cell.text == text {
            rows.flag(src, e.col, None);
            return None;
        }
        if let Err(message) = rows.commit(src, e.col, text) {
            e.field
                .set_error(Some(crate::widgets::edit::FieldError::new(message)));
            self.edit = Some(e);
            return None;
        }
        if let Some((c, d)) = self.sort
            && c == e.col
        {
            self.apply_sort_in(rows, c, d);
            if let Some(pos) = self.order.iter().position(|&r| r == src) {
                self.cursor_row = pos;
                self.scroll.ensure_visible(pos);
            }
        }
        Some(TableAction::Committed {
            row: src,
            col: e.col,
        })
    }

    pub fn cancel_edit(&mut self) -> Option<TableAction> {
        self.cancel_sessions().then_some(TableAction::Cancelled)
    }

    fn cancel_sessions(&mut self) -> bool {
        let text_closed = self.edit.take().is_some();
        let choice_closed = self.choice.take().is_some();
        text_closed || choice_closed
    }

    fn structural_change(&mut self) {
        self.cancel_sessions();
        self.structural_generation = self.structural_generation.wrapping_add(1);
    }

    fn session_target_live(&self, generation: u64) -> bool {
        generation == self.structural_generation
    }

    /// Focus left the table. Owned tables apply the blur policy to the open
    /// editor. A table driven through the `*_source` methods holds no rows, so
    /// it leaves its editor alone and leaves the commit to
    /// [`DataTable::on_blur_edit_source`].
    pub fn on_blur(&mut self) -> Response<TableAction> {
        if self.keyed.stamp().is_some() {
            return Response::ignored();
        }
        self.with_rows(|me, rows| me.on_blur_in(rows))
    }

    fn on_blur_in<R: TableRows + ?Sized>(&mut self, rows: &mut R) -> Response<TableAction> {
        if let Some(c) = &self.choice {
            return match self.editor_config.blur {
                BlurAction::Commit => match c.editor.blur_value(&self.editor_config) {
                    Some(v) => self.on_choice(rows, Some(ChoiceResult::Commit(v))),
                    None => self.on_choice(rows, Some(ChoiceResult::Cancel)),
                },
                BlurAction::Cancel => self.on_choice(rows, Some(ChoiceResult::Cancel)),
                BlurAction::Keep => Response::ignored(),
            };
        }
        if self.edit.is_none() {
            return Response::ignored();
        }
        let action = match self.blur_action {
            BlurAction::Commit => self.commit_edit_in(rows),
            BlurAction::Cancel => self.cancel_edit(),
            BlurAction::Keep => return Response::ignored(),
        };
        let mut resp = Response::changed();
        if let Some(action) = action {
            resp.push_action(action);
        }
        resp
    }

    pub fn on_key(&mut self, key: &Key) -> Response<TableAction> {
        self.with_rows(|me, rows| me.on_key_in(rows, key))
    }

    fn on_key_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        key: &Key,
    ) -> Response<TableAction> {
        if let Some(c) = self.choice.as_mut() {
            let (out, r) = c.editor.on_key(key, &self.editor_config);
            return Response::from(out) | self.on_choice(rows, r);
        }
        if let Some(e) = self.edit.as_mut() {
            let r = e.field.on_key(key);
            return match r.exit {
                Some(EditExit::Commit) => {
                    Response::from((Outcome::Changed, self.commit_edit_in(rows)))
                }
                Some(EditExit::Cancel) => Response::from((Outcome::Changed, self.cancel_edit())),
                Some(EditExit::Tab { backward }) => {
                    let committed = self.commit_edit_in(rows);
                    if self.edit.is_some() {
                        return Response::changed();
                    }
                    let mut resp = Response::changed();
                    if let Some(action) = committed {
                        resp.push_action(action);
                    }
                    if committed == Some(TableAction::Cancelled) {
                        return resp;
                    }
                    resp.merge(self.tab_after_commit(rows, backward))
                }
                None => Response::from(r.outcome),
            };
        }
        if rows.len() == 0 {
            return Response::ignored();
        }
        self.clamp_cursor(rows.len());
        if let Some(KeyStep::Type(c)) = KeyStep::ordered(
            key,
            &self.nav,
            self.type_ahead,
            self.type_ahead_precedence
                .unwrap_or_else(crate::core::list::TypeAheadPrecedence::current),
        ) {
            let mut ta = std::mem::take(&mut self.typeahead);
            let moved = self
                .cursor()
                .type_ahead(c, &mut ta, &View { table: self, rows });
            self.typeahead = ta;
            if let Moved::To(d) = moved {
                self.set_cursor(rows.len(), d, self.cursor_col);
            }
            return Response::from(moved.outcome());
        }
        if self.multi()
            && let Some(r) = self.on_key_multi(rows, key)
        {
            return r.into();
        }
        if unassigned_chord(key, &self.keymap) {
            return Response::ignored();
        }
        let (r, c) = (self.cursor_row, self.cursor_col);
        if let Some(action) = lookup(&self.keymap, key) {
            return match action {
                TableCmd::SortByColumn => self.sort_by_in(rows, c).into(),
                TableCmd::SelectAll if self.multi() => {
                    if self.select_all_in(rows) {
                        Response::emit(TableAction::SelectionChanged)
                    } else {
                        Response::consumed()
                    }
                }

                TableCmd::SelectAll => Response::ignored(),
            };
        }

        if key.code == KeyCode::Down
            && key.alt()
            && self.cell_nav
            && self.columns[c].editable
            && self.columns[c].editor.has_popup()
        {
            return self.start_edit_in(rows);
        }
        if let Some(nav) = self.nav.action(key) {
            return self.on_nav(rows, nav).into();
        }
        match key.code {
            KeyCode::Enter | KeyCode::F(2) if self.cell_nav && self.columns[c].editable => {
                self.start_edit_in(rows)
            }

            KeyCode::Char(' ')
                if self.cell_nav
                    && self.columns[c].editable
                    && self.columns[c].editor.is_constrained() =>
            {
                self.start_edit_in(rows)
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let src = self.order[r];
                if self.is_disabled_in(rows, src) {
                    return Response::consumed();
                }
                if self.selection_mode == SelectionMode::Single {
                    self.selected = Some(src);
                }
                Response::emit(TableAction::Activated(src))
            }
            _ => Response::ignored(),
        }
    }

    fn on_nav<R: TableRows + ?Sized>(
        &mut self,
        rows: &R,
        nav: Nav,
    ) -> (Outcome, Option<TableAction>) {
        let (r, c) = (self.cursor_row, self.cursor_col);
        match nav {
            Nav::Up | Nav::Down | Nav::PageUp | Nav::PageDown | Nav::Home | Nav::End => {
                let to = self
                    .cursor()
                    .apply(nav, &View { table: self, rows }, self.scroll.viewport_len)
                    .index()
                    .unwrap_or(r);
                self.set_cursor(rows.len(), to, c);
                (Outcome::Changed, None)
            }
            Nav::Left if self.cell_nav => {
                self.set_cursor(rows.len(), r, c.saturating_sub(1));
                (Outcome::Changed, None)
            }
            Nav::Right if self.cell_nav => {
                self.set_cursor(rows.len(), r, c + 1);
                (Outcome::Changed, None)
            }
            Nav::Left => {
                self.scroll.h.scroll_by(-1);
                (Outcome::Changed, None)
            }
            Nav::Right => {
                self.scroll.h.scroll_by(1);
                (Outcome::Changed, None)
            }
        }
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if let Some(c) = self.choice.as_mut() {
            return c.editor.on_paste(text);
        }
        match self.edit.as_mut() {
            Some(e) => {
                e.field.paste(text);
                Outcome::Changed
            }
            None => Outcome::Ignored,
        }
    }

    pub fn on_click_header(&mut self, col: usize) -> Response<TableAction> {
        self.with_rows(|me, rows| me.on_click_header_in(rows, col))
    }

    fn on_click_header_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        col: usize,
    ) -> Response<TableAction> {
        let committed = self.commit_edit_in(rows);
        let mut resp = Response::from(self.sort_by_in(rows, col));
        if let Some(action) = committed {
            resp = Response::emit(action).merge(resp);
        }
        resp
    }

    pub fn on_click_cell(
        &mut self,
        display: usize,
        col: usize,
        pos: Position,
    ) -> Response<TableAction> {
        self.on_click_cell_count(display, col, pos, KeyModifiers::NONE, 1)
    }

    pub fn on_click_cell_with(
        &mut self,
        display: usize,
        col: usize,
        pos: Position,
        mods: KeyModifiers,
    ) -> Response<TableAction> {
        self.on_click_cell_count(display, col, pos, mods, 1)
    }

    pub fn on_click_cell_count(
        &mut self,
        display: usize,
        col: usize,
        pos: Position,
        mods: KeyModifiers,
        count: u8,
    ) -> Response<TableAction> {
        self.with_rows(|me, rows| me.on_click_cell_count_in(rows, display, col, pos, mods, count))
    }

    fn on_click_cell_count_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        display: usize,
        col: usize,
        pos: Position,
        mods: KeyModifiers,
        count: u8,
    ) -> Response<TableAction> {
        let count = count.max(1);
        if display >= rows.len() {
            return Response::consumed();
        }

        self.choice = None;
        let col = col.min(self.columns.len().saturating_sub(1));
        let mut committed = Response::ignored();
        if let Some(e) = &self.edit {
            if e.row == self.order[display] && e.col == col {
                if let Some(at) = self.edit_offset(pos)
                    && let Some(e) = self.edit.as_mut()
                {
                    e.field.click(at, count, false);
                }
                return Response::changed();
            }
            if let Some(action) = self.commit_edit_in(rows) {
                committed.push_action(action);
            }
            if self.edit.is_some() {
                return Response::changed();
            }
        }
        committed.merge(self.click_cell_after_commit(rows, display, col, mods, count))
    }

    fn click_cell_after_commit<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        display: usize,
        col: usize,
        mods: KeyModifiers,
        count: u8,
    ) -> Response<TableAction> {
        let same = self.cursor_row == display && (!self.cell_nav || self.cursor_col == col);
        let from = self.cursor_row;
        self.point_cursor(
            rows.len(),
            display,
            if self.cell_nav { col } else { self.cursor_col },
        );
        if self.multi() && !self.cell_nav {
            let src = self.order[display];
            let disabled = &self.disabled;
            let provider = &*rows;
            let enabled = |r: usize| !disabled.contains(&r) && !provider.row_disabled(r);
            let gesture = SelectGesture::from_mods(mods, self.toggle_mods, from);
            if gesture == SelectGesture::Only && !enabled(src) {
                return Response::changed();
            }
            let changed = self.selection.apply(gesture, display, &self.order, enabled);
            if gesture == SelectGesture::Only && self.click.activates(count, same) {
                return Response::emit(TableAction::Activated(src));
            }
            return Response::from((
                Outcome::Changed,
                changed.then_some(TableAction::SelectionChanged),
            ));
        }
        if self.cell_nav && self.columns[col].editable && self.click.edits(count, same) {
            return self.start_edit_in(rows);
        }
        if !self.cell_nav {
            let src = self.order[display];
            if self.is_disabled_in(rows, src) {
                return Response::changed();
            }
            if self.selection_mode == SelectionMode::Single {
                self.selected = Some(src);
            }
            if self.click.activates(count, same) {
                return Response::emit(TableAction::Activated(src));
            }
        }
        Response::changed()
    }

    fn layout_columns(&mut self, area: Rect) {
        let n = self.columns.len();

        let gap = 2u16;
        let widths = (0..n)
            .map(|i| self.sizing.width(i, self.columns[i].min_width()))
            .collect();
        let fit = self.scroll.layout_columns(widths, gap, area.width);
        let fit = fit.max(1).min(n - self.scroll.h.offset.min(n));
        self.scroll.h.content_len = n;
        self.scroll.h.viewport_len = fit;
        self.scroll.h.clamp();
        let visible = self.scroll.h.offset..(self.scroll.h.offset + fit).min(n);
        let constraints: Vec<Constraint> = visible
            .clone()
            .map(|i| match self.sizing.user(i) {
                Some(w) => Constraint::Length(w),
                None => self.columns[i].width,
            })
            .collect();
        let rects = Layout::horizontal(constraints).spacing(gap).split(area);
        self.col_rects = vec![Rect::ZERO; n];
        for (k, i) in visible.enumerate() {
            self.col_rects[i] = rects[k];
        }
    }

    /// The stable-identity state of a table driven through the `*_source`
    /// methods.
    pub fn keyed(&self) -> &KeyedState {
        &self.keyed
    }

    fn bind<S: TableSource + ?Sized>(&mut self, source: &S) -> Result<(), KeyedSourceError> {
        let first = self.keyed.stamp().is_none();
        let out = self.keyed.reconcile(source)?;
        if self.columns != source.columns() {
            self.numeric = vec![false; source.columns().len()];
            self.columns = source.columns().to_vec();
            self.sort = None;
            self.sizing.reset();
            self.cursor_col = self.cursor_col.min(self.columns.len().saturating_sub(1));
            self.edit = None;
            self.choice = None;
            self.keyed.cancel_edit();
        }
        if first || out.data_changed || out.view_changed || out.cursor_changed {
            let n = self.keyed.visible_len();
            self.order = (0..n).collect();
            self.scroll.set_content(n);
            self.cursor_row = self
                .keyed
                .cursor_view()
                .unwrap_or(0)
                .min(n.saturating_sub(1));
            let selected = self.keyed.selected_view();
            match self.selection_mode {
                SelectionMode::Multi => self.selection.replace(selected, self.keyed.anchor_view()),
                SelectionMode::Single => self.selected = selected.first().copied(),
                SelectionMode::None => {}
            }
            if out.edit_cancelled {
                self.edit = None;
                self.choice = None;
            }
            let edited = self
                .keyed
                .edit_key()
                .and_then(|key| self.keyed.visible_index(key));
            if let Some(row) = edited {
                if let Some(e) = self.edit.as_mut() {
                    e.row = row;
                }
                if let Some(c) = self.choice.as_mut() {
                    c.row = row;
                }
            }
            self.scroll.ensure_visible(self.cursor_row);
        }
        Ok(())
    }

    /// Writes the engine's view-position state back to keys and shapes the
    /// actions by key.
    fn adopt(
        &mut self,
        response: Response<TableAction>,
        selected_before: Option<usize>,
        data: Option<u64>,
    ) -> Response<TableAction<ItemKey>> {
        let n = self.keyed.visible_len();
        if n > 0 {
            self.keyed.adopt_cursor_view(self.cursor_row);
        }
        match self.selection_mode {
            SelectionMode::Multi => {
                self.keyed
                    .adopt_selection_view(self.selection.iter(), self.selection.anchor());
            }
            SelectionMode::Single if self.selected != selected_before => {
                let key = self.selected.and_then(|row| self.keyed.visible_key(row));
                self.keyed.replace_selection(key);
            }
            _ => {}
        }
        let committed = response
            .actions()
            .any(|action| matches!(action, TableAction::Committed { .. }));
        if committed {
            self.keyed.finish_edit();
        }
        if self.edit.is_some() || self.choice.is_some() {
            if self.keyed.edit_key().is_none() {
                let row = self
                    .edit
                    .as_ref()
                    .map(|e| e.row)
                    .or_else(|| self.choice.as_ref().map(|c| c.row));
                if let Some(key) = row.and_then(|row| self.keyed.visible_key(row)) {
                    match data.filter(|_| committed) {
                        Some(data) => self.keyed.begin_edit_at(key, data),
                        None => self.keyed.begin_edit(key),
                    };
                }
            }
        } else {
            self.keyed.cancel_edit();
        }
        let keyed = &self.keyed;
        response.map_actions(|action| {
            action.keyed(|row| keyed.visible_key(row).unwrap_or(ItemKey::Index(row)))
        })
    }

    fn note_layout(&mut self, area: Rect, buf: &Buffer) {
        if area.intersection(*buf.area()) != self.area {
            self.layout_generation += 1;
            self.keyed.invalidate_layout(self.layout_generation);
        }
    }

    /// Keyboard input over a borrowed read-only keyed provider: navigation,
    /// selection and activation by key. A read-only source has no editors.
    pub fn on_key_source<S: TableSource + ?Sized>(
        &mut self,
        key: &Key,
        source: &S,
    ) -> Result<Response<TableAction<ItemKey>>, KeyedSourceError> {
        self.bind(source)?;
        let selected = self.selected;
        let response = self.on_key_in(&mut ReadRows(source), key);
        Ok(self.adopt(response, selected, None))
    }

    /// Keyboard input over a borrowed editable provider. A commit is applied
    /// with `set_cell` to the row captured when the edit began.
    pub fn on_key_edit_source<S: TableEditSource + ?Sized>(
        &mut self,
        key: &Key,
        source: &mut S,
    ) -> Result<Response<TableAction<ItemKey>>, KeyedSourceError> {
        self.bind(&*source)?;
        let selected = self.selected;
        let mut rows = EditRows(source);
        let response = self.on_key_in(&mut rows, key);
        Ok(self.adopt(response, selected, rows.data_revision()))
    }

    fn pointer_keyed<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Response<TableAction<ItemKey>> {
        use crate::core::intent::Phase;
        self.keyed.note_pointer(ev.phase());
        let hit = [Part::CELL, Part::ROW, Part::MARKER]
            .into_iter()
            .find_map(|part| ev.key_of(part));
        if let Some(key) = hit.filter(|_| ev.on(self.id)) {
            match ev.phase() {
                Some(Phase::Press) => {
                    self.keyed.capture_press(key, self.layout_generation);
                }
                Some(Phase::Click) if !self.keyed.release_click(key, self.layout_generation) => {
                    return Response::consumed();
                }
                _ => {}
            }
        }
        let selected = self.selected;
        let response = self.pointer_in(rows, ev, true);
        self.adopt(response, selected, rows.data_revision())
    }

    /// Pointer input over a borrowed read-only keyed provider. Hit regions
    /// carry the row key, so a press can only complete on the row it began on.
    pub fn on_pointer_source<S: TableSource + ?Sized>(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        source: &S,
    ) -> Result<Response<TableAction<ItemKey>>, KeyedSourceError> {
        self.bind(source)?;
        Ok(self.pointer_keyed(&mut ReadRows(source), ev))
    }

    /// Pointer input over a borrowed editable keyed provider.
    pub fn on_pointer_edit_source<S: TableEditSource + ?Sized>(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        source: &mut S,
    ) -> Result<Response<TableAction<ItemKey>>, KeyedSourceError> {
        self.bind(&*source)?;
        Ok(self.pointer_keyed(&mut EditRows(source), ev))
    }

    /// Focus left a table over a borrowed editable keyed provider. The blur
    /// policy decides the open editor's fate: `Commit` applies the typed text
    /// with `set_cell` to the row captured when the edit began, like an Enter
    /// commit.
    pub fn on_blur_edit_source<S: TableEditSource + ?Sized>(
        &mut self,
        source: &mut S,
    ) -> Result<Response<TableAction<ItemKey>>, KeyedSourceError> {
        self.bind(&*source)?;
        let selected = self.selected;
        let mut rows = EditRows(source);
        let response = self.on_blur_in(&mut rows);
        Ok(self.adopt(response, selected, rows.data_revision()))
    }

    /// Paints rows borrowed from a read-only keyed provider.
    pub fn render_source<S: TableSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &S,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) -> Result<(), KeyedSourceError> {
        self.bind(source)?;
        self.note_layout(area, buf);
        self.render_in(&ReadRows(source), area, buf, ctx, bg);
        Ok(())
    }

    /// Paints rows borrowed from an editable keyed provider.
    pub fn render_edit_source<S: TableEditSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &mut S,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) -> Result<(), KeyedSourceError> {
        self.bind(&*source)?;
        self.note_layout(area, buf);
        self.render_in(&EditRows(source), area, buf, ctx, bg);
        Ok(())
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.with_rows(|me, rows| me.render_in(rows, area, buf, ctx, bg));
    }

    fn render_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &R,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        self.parts.clear();
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        crate::theme::Strings::follow(&mut self.empty_text, old.no_rows, s.no_rows);
        self.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() || area.height < 2 {
            return;
        }
        self.area = area;
        self.clamp_cursor(rows.len());
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        let editing = self.edit.is_some() && focused;

        let mut choice_anchor: Option<Rect> = None;
        let mut open_marks: Vec<(ItemKey, usize, Rect)> = Vec::new();

        ctx.control(self.id, area, false);
        ctx.scrollable(self.id, area);

        let h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        self.scroll.h_cfg = h_cfg;

        let rows_len = rows.len();
        let mut has_sb = rows_len > area.height as usize - 1;

        let check_w: u16 = if self.multi() && self.selection_column {
            4
        } else {
            0
        };
        let cols_for = |has_sb: bool| {
            Rect::new(
                area.x + 3 + check_w,
                area.y,
                area.width.saturating_sub(5 + check_w + u16::from(has_sb)),
                area.height,
            )
        };
        self.layout_columns(cols_for(has_sb));
        let header_y = area.y;
        let mut body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);

        let has_hb = rows.len() > 0 && body.height > 1 && h_cfg.shows_bar(&self.scroll.h);
        if has_hb {
            body.height -= 1;
            if !has_sb && rows_len > body.height as usize {
                has_sb = true;
                self.layout_columns(cols_for(true));
            }
        }
        let cols_area = cols_for(has_sb);
        self.scroll.set_content(rows_len);
        self.scroll.set_viewport(body.height as usize);
        self.body = body;

        let header_bg = bg;
        fill(
            buf,
            Rect::new(area.x, header_y, area.width, 1),
            Style::new().bg(header_bg),
        );
        for (ci, col) in self.columns.iter().enumerate() {
            let r = self.col_rects[ci];
            if r.is_empty() {
                continue;
            }
            let hid = self.header_id(ci);
            let sorted = self.sort.map(|(c, _)| c == ci).unwrap_or(false);
            let st = crate::widgets::grid::header_style(ctx, hid, header_bg, col.sortable, sorted);
            let ind = match self.sort {
                Some((c, SortDir::Asc)) if c == ci => format!(" {}", t.glyphs.sort_asc),
                Some((c, SortDir::Desc)) if c == ci => format!(" {}", t.glyphs.sort_desc),
                _ => String::new(),
            };
            let ind = ind.as_str();
            let ind_w = crate::ui::text::width(ind);
            let shown =
                crate::ui::text::truncate(&col.title, (r.width as usize).saturating_sub(ind_w));
            let cut = shown != col.title;
            let title = format!("{shown}{ind}");
            let text = match col.align {
                Align::Left => crate::ui::text::fit(&title, r.width as usize),
                Align::Right => crate::ui::text::fit_right(&title, r.width as usize),
            };
            buf.set_string(r.x, header_y, &text, st);
            let head = Rect::new(r.x, header_y, r.width, 1);
            if col.sortable {
                ctx.register_part(
                    self.id,
                    PartRef::item(Part::HEADER, ItemKey::index(ci)),
                    head,
                );
            } else if cut && !ctx.inert {
                ctx.hits.register_hover(hid, head);
            }
            if cut {
                crate::ui::reveal::truncated(ctx, self.reveal, hid, head, &col.title);
            }
            if self.sizing.resizable {
                let hit = self.sizing.seam_rect(r.right(), header_y, 2, area);
                crate::ui::columns::render_seam(
                    buf,
                    ctx,
                    (self.id, ci),
                    hit,
                    r.right(),
                    header_bg,
                    false,
                );
            }
        }
        if check_w > 0 && area.width > 3 + check_w {
            let cid = self.check_all_id();
            let state = self.selection_state_in(rows);
            let lifted = ctx.interaction.hovered(cid) || ctx.interaction.pressed(cid);
            let st = crate::widgets::grid::header_style(ctx, cid, header_bg, true, false);
            let st = match state {
                _ if lifted => st,
                TriState::None => st,
                _ => st.fg(t.legible(ctx.chosen(focused), st.bg.unwrap_or(header_bg))),
            };
            let r = Rect::new(area.x + 3, header_y, 3, 1);
            buf.set_string(r.x, r.y, state.glyph_in(&t.glyphs), st);
            if rows.len() > 0 {
                ctx.register_part(self.id, PartRef::of(Part::MARKER), r);
            }
        }

        if self.overflow_marks && self.scroll.h.overflows() {
            let more_right = self.scroll.h.offset + self.scroll.h.viewport_len < self.columns.len();
            let more_left = self.scroll.h.offset > 0;
            if more_left {
                let st = t.faint().bg(header_bg);
                buf.set_string(area.x + 1, header_y, t.glyphs.ellipsis, st);
            }
            if more_right {
                buf.set_string(
                    cols_area.right() + 1,
                    header_y,
                    t.glyphs.ellipsis,
                    t.faint().bg(header_bg),
                );
            }
        }

        if rows.len() == 0 {
            if let Some(e) = &self.empty {
                crate::widgets::empty::render(body, buf, t, e, bg);
                return;
            }
            let msg = crate::ui::text::truncate(&self.empty_text, area.width as usize);
            let y = body.y + body.height / 2;
            let x = body.x
                + body
                    .width
                    .saturating_sub(crate::ui::text::width(&msg) as u16)
                    / 2;
            buf.set_string(x, y, &msg, t.muted().bg(bg));
            return;
        }

        for (i, di) in self.scroll.visible_range().enumerate() {
            crate::record_work!(VisibleRowsPainted, 1usize);
            let y = body.y + i as u16;
            let src = self.order[di];
            let row_rect = Rect::new(
                area.x,
                y,
                area.width.saturating_sub(if has_sb { 1 } else { 0 }),
                1,
            );
            let key = rows.hit_key(di, src);
            let rid = self.id.part(PartRef::item(Part::ROW, key));
            let cell_id = |c: usize| rid.child(c);
            let open_id = |c: usize| cell_id(c).part(PartRef::of(Part::ACTIONS));
            let mut s = ctx.child_state(self.id, rid, di == self.cursor_row);

            let hovered_cell = (0..self.columns.len()).find(|&c| {
                ctx.interaction.hovered(cell_id(c)) || ctx.interaction.hovered(open_id(c))
            });
            s.hovered |= hovered_cell.is_some();
            s.pressed |= ctx
                .interaction
                .pressed_any((0..self.columns.len()).map(cell_id));
            s.focused = focused && di == self.cursor_row;
            s.selected = match self.selection_mode {
                SelectionMode::Single => self.selected == Some(src),
                SelectionMode::Multi => self.selection.contains(src),
                SelectionMode::None => false,
            };
            let row_disabled = self.is_disabled_in(rows, src);
            if row_disabled {
                s.hovered = false;
                s.pressed = false;
            }
            let rp = RowUi::paint(ctx, buf, row_rect, s, RowLook::Bar, bg);
            let mut row_style = rp.style();
            if row_disabled {
                row_style = row_style.fg(t.legible(t.text_muted, row_style.bg.unwrap_or(bg)));
            }
            match (self.selection_mode, s.selected) {
                (SelectionMode::Single, true) => {
                    rp.mark(ctx, buf, row_rect.x + 1, y, MarkKind::Current, focused)
                }
                (SelectionMode::Multi, true) if check_w == 0 => {
                    rp.mark(ctx, buf, row_rect.x + 1, y, MarkKind::Tick, focused)
                }
                _ => {}
            }
            if check_w > 0 && area.width > 3 + check_w {
                let r = Rect::new(area.x + 3, y, 3, 1);
                let (glyph, gs) = if row_disabled {
                    (
                        t.glyphs.check_off,
                        row_style.fg(t.disabled).add_modifier(Modifier::DIM),
                    )
                } else if s.selected {
                    (t.glyphs.check_on, row_style.fg(ctx.chosen(focused)))
                } else {
                    (t.glyphs.check_off, row_style.fg(t.text_muted))
                };

                buf.set_string(r.x, r.y, glyph, gs);
            }
            for (ci, col) in self.columns.iter().enumerate() {
                let r = self.col_rects[ci];
                if r.is_empty() {
                    continue;
                }
                let cell_rect = Rect::new(r.x, y, r.width, 1);
                let cell = rows.cell(src, ci).unwrap_or(EMPTY_CELL);
                let is_cursor_cell = self.cell_nav && s.focused && ci == self.cursor_col;
                let is_edit = editing
                    && self
                        .edit
                        .as_ref()
                        .map(|e| e.row == src && e.col == ci)
                        .unwrap_or(false);
                let mut st = row_style;
                let cfg = &self.editor_config;
                let constrained = col.editor.is_constrained();

                let shown: std::borrow::Cow<str> = if let Some(on) = col.editor.checked(cell.text) {
                    if on {
                        t.glyphs.check_on
                    } else {
                        t.glyphs.check_off
                    }
                    .into()
                } else if constrained {
                    col.editor.display(cell.text, cfg).into()
                } else {
                    cell.text.into()
                };
                let flagged = cell.error.is_some()
                    || (cfg.flag_out_of_set && col.editor.out_of_set(cell.text, cfg));
                let tone = match (cell.tone, col.editor.tone(cell.text)) {
                    (Tone::Normal, Some(tone)) => tone,
                    (tone, _) => tone,
                };
                let is_choice_cell = self
                    .choice
                    .as_ref()
                    .is_some_and(|c| c.row == src && c.col == ci);
                if is_choice_cell {
                    choice_anchor = Some(cell_rect);
                }
                st = match tone {
                    Tone::Normal => st,
                    _ if row_disabled => st,
                    other => st.fg(t.tone(other)),
                };
                if s.pressed {
                    st = row_style;
                }
                if is_edit {
                    let e = self.edit.as_ref().unwrap();
                    let es = t.field_style(crate::ui::ctx::VisualState {
                        editing: true,
                        ..Default::default()
                    });
                    fill(buf, cell_rect, es);
                    let text = e.field.text();
                    let cw = r.width.saturating_sub(1) as usize;
                    let cur = e.field.buffer.cursor_pos().col;
                    let off = cur.saturating_sub(cw.saturating_sub(1));
                    let shown = crate::ui::text::slice_cells(text, off, cw);
                    let mut ts = es
                        .add_modifier(Modifier::UNDERLINED)
                        .underline_color(t.accent);
                    if e.field.error.is_some() {
                        ts = ts.underline_color(t.error);
                    }
                    buf.set_string(r.x, y, &shown, ts);
                    if cw > 0 {
                        ctx.set_cursor(Position::new(r.x + (cur - off) as u16, y));
                    }
                    if e.field.error.is_some() {
                        buf.set_string(
                            r.right().saturating_sub(1),
                            y,
                            "!",
                            es.fg(t.error).add_modifier(Modifier::BOLD),
                        );
                    }
                } else {
                    if is_cursor_cell || is_choice_cell {
                        st = t.cell_cursor(flagged, s.pressed);
                    } else if flagged {
                        st = st.fg(t.error);
                    } else if col.editable && self.cell_nav && s.hovered && hovered_cell == Some(ci)
                    {
                        st = st
                            .add_modifier(Modifier::UNDERLINED)
                            .underline_color(t.border_strong);
                    }

                    let affordance = col.editable
                        && col.editor.has_popup()
                        && r.width > 3
                        && match cfg.affordance {
                            Affordance::Never => false,
                            Affordance::Always => true,
                            Affordance::Focused => {
                                is_cursor_cell
                                    || is_choice_cell
                                    || (s.hovered && hovered_cell == Some(ci))
                            }
                        };
                    let text_w = if affordance { r.width - 2 } else { r.width } as usize;
                    let text = match col.align {
                        Align::Left => crate::ui::text::fit(&shown, text_w),
                        Align::Right => crate::ui::text::fit_right(&shown, text_w),
                    };
                    if affordance {
                        fill(buf, cell_rect, st);
                    }
                    buf.set_string(r.x, y, &text, st);
                    let cut = crate::ui::reveal::truncated(
                        ctx,
                        self.reveal,
                        cell_id(ci),
                        Rect::new(r.x, y, text_w as u16, 1),
                        &shown,
                    );

                    if cut && s.focused && (!self.cell_nav || ci == self.cursor_col) {
                        let cfg = self.reveal.unwrap_or(ctx.reveal);
                        if cfg.hover || cfg.keyboard.is_some() {
                            crate::ui::reveal::peek(ctx, &shown);
                        }
                    }
                    if affordance {
                        let mark = Rect::new(r.right() - 1, y, 1, 1);
                        let ms = if is_cursor_cell || is_choice_cell {
                            st
                        } else {
                            st.fg(t.text_secondary)
                        };
                        buf.set_string(mark.x, y, t.glyphs.disclosure_open, ms);
                        open_marks.push((key, ci, mark));
                    }
                    if flagged && !is_cursor_cell && !is_choice_cell && !affordance {
                        buf.set_string(
                            r.right().saturating_sub(1),
                            y,
                            "!",
                            st.add_modifier(Modifier::BOLD),
                        );
                    }
                }
                let cid = cell_id(ci);
                let cell = (PartRef::item(Part::CELL, key), ci);
                ctx.register_part_as(cid, self.id, cell.0, cell_rect);
            }
            ctx.register_part(self.id, PartRef::item(Part::ROW, key), row_rect);

            for ci in 0..self.columns.len() {
                let r = self.col_rects[ci];
                if !r.is_empty() {
                    let cid = cell_id(ci);
                    let cell = (PartRef::item(Part::CELL, key), ci);
                    let rect = Rect::new(r.x, y, r.width, 1);
                    ctx.register_part_as(cid, self.id, cell.0, rect);
                }
            }
            if check_w > 0 && area.width > 3 + check_w && !row_disabled {
                let rect = Rect::new(area.x + 3, y, 3, 1);
                ctx.register_part(self.id, PartRef::item(Part::MARKER, key), rect);
            }
        }

        for (key, ci, rect) in open_marks {
            let oid = self
                .id
                .part(PartRef::item(Part::ROW, key))
                .child(ci)
                .part(PartRef::of(Part::ACTIONS));
            let open = (PartRef::item(Part::ACTIONS, key), ci);

            self.parts.record_cell(oid, open.0, open.1);
            ctx.register_part_as(oid, self.id, open.0, rect);
        }
        if let (Some(anchor), Some(c)) = (choice_anchor, self.choice.as_mut()) {
            c.editor.render(anchor, buf, ctx, &self.editor_config);
        }

        ctx.scrollable(self.id, area);
        if has_sb {
            crate::ui::fade::scroll_edges_except(
                buf,
                ctx,
                Rect::new(
                    area.x,
                    body.y,
                    (area.right() - 1).saturating_sub(area.x),
                    body.height,
                ),
                &self.scroll,
                &self
                    .scroll
                    .visible_range()
                    .position(|i| i == self.cursor_row)
                    .map(|k| body.y + k as u16)
                    .into_iter()
                    .collect::<Vec<_>>(),
            );
            let sb = Rect::new(area.right() - 1, body.y, 1, body.height);
            self.scroll.draw_v(sb, buf, ctx, self.id, focused);
        }
        if has_hb {
            let y = body.bottom();
            let right = area.right() - u16::from(has_sb);
            fill(
                buf,
                Rect::new(area.x, y, area.width, 1),
                Style::new().bg(bg),
            );
            let hb = Rect::new(cols_area.x, y, right.saturating_sub(cols_area.x), 1);
            self.scroll.draw_h(hb, buf, ctx, self.id, focused);
        }
    }

    pub(crate) fn column_at(&self, x: u16) -> Option<usize> {
        self.col_rects
            .iter()
            .position(|r| !r.is_empty() && x >= r.x && x < r.right())
    }

    fn edit_offset(&self, pos: Position) -> Option<usize> {
        let e = self.edit.as_ref()?;
        let rect = self.col_rects.get(e.col)?;
        let cw = rect.width.saturating_sub(1) as usize;
        let off = e
            .field
            .buffer
            .cursor_pos()
            .col
            .saturating_sub(cw.saturating_sub(1));
        let c = (off as isize + (pos.x as isize - rect.x as isize)).max(0) as usize;
        Some(e.field.buffer.offset_at(0, c))
    }

    fn on_edit_cell(&self, ev: &crate::core::pointer::PointerEvent, keyed: bool) -> bool {
        let Some(e) = &self.edit else {
            return false;
        };
        ev.on(self.id)
            && self
                .hit_row(ev, Part::CELL, keyed)
                .is_some_and(|d| self.order.get(d) == Some(&e.row))
            && self.column_at(ev.pos().x) == Some(e.col)
    }

    pub fn edit_error(&self) -> Option<&str> {
        if let Some(c) = &self.choice {
            return c.editor.error.as_deref();
        }
        self.edit.as_ref().and_then(|e| e.field.error_message())
    }
}

use ratatui::style::Style;

fn parse_num(s: &str) -> Option<ordered::F64> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    cleaned
        .parse::<f64>()
        .ok()
        .filter(|f| !f.is_nan())
        .map(ordered::F64)
}

mod ordered {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct F64(pub f64);
    impl Eq for F64 {}
    impl PartialOrd for F64 {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for F64 {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0.total_cmp(&other.0)
        }
    }
}

impl crate::core::focus::BlurTarget for DataTable {
    fn blur_id(&self) -> WidgetId {
        self.id
    }
    fn blur(&mut self) -> Outcome {
        let editing = self.edit.is_some();
        // `BlurTarget` reports only an outcome; hosts that need the commit
        // action call `DataTable::on_blur` directly, or
        // `DataTable::on_blur_edit_source` for keyed tables.
        let _ = self.on_blur();
        if editing && self.edit.is_none() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }
}

impl Scrollable for DataTable {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        let c = self.choice.as_mut().filter(|_| axis == Axis::Vertical)?;
        Some(c.editor.wheel(axis, delta))
    }
}

impl RowSource for DataTable {
    fn len(&self) -> usize {
        self.order.len()
    }
    fn enabled(&self, d: usize) -> bool {
        !self.is_disabled(self.order[d])
    }
    fn label(&self, d: usize) -> &str {
        let col = self.type_ahead_column.unwrap_or(self.cursor_col);
        self.rows[self.order[d]]
            .get(col)
            .map_or("", |cell| cell.text.as_str())
    }
}

impl Navigable for DataTable {
    fn cursor(&self) -> Cursor {
        Cursor::new(self.cursor_row).skip_disabled(false)
    }
}

impl crate::core::pointer::MouseTarget for DataTable {
    type Action = TableAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<TableAction>> {
        let editor = self.choice.as_ref().map(|c| c.editor.id);
        (ev.on(self.id) || editor.is_some_and(|e| ev.on(e))).then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<TableAction> {
        self.with_rows(|me, rows| me.pointer_in(rows, ev, false))
    }
}

impl DataTable {
    /// The display row an event hit on `part`: by position for an owned table,
    /// by the row key carried in the hit for a keyed one.
    fn hit_row(
        &self,
        ev: &crate::core::pointer::PointerEvent,
        part: Part,
        keyed: bool,
    ) -> Option<usize> {
        if keyed {
            ev.key_of(part)
                .and_then(|key| self.keyed.visible_index(key))
        } else {
            ev.index_of(part)
        }
    }

    fn pointer_in<R: TableRows + ?Sized>(
        &mut self,
        rows: &mut R,
        ev: &crate::core::pointer::PointerEvent,
        keyed: bool,
    ) -> Response<TableAction> {
        use crate::core::intent::Phase;

        if let Some(c) = self.choice.as_mut()
            && ev.on(c.editor.id)
            && ev.wheel().is_none()
        {
            let (out, results) =
                crate::core::pointer::MouseTarget::pointer(&mut c.editor, ev).into_parts();
            return self.on_choices(rows, out, results);
        }

        if (ev.on(self.id) || ev.wheel().is_some())
            && let Some(r) = crate::core::pointer::scroll_input(self, ev)
        {
            return r.lift();
        }
        let (id, pos) = (ev.id, ev.pos());

        match ev.phase() {
            Some(Phase::Press) if self.on_edit_cell(ev, keyed) => {
                if let Some(at) = self.edit_offset(pos)
                    && let Some(e) = self.edit.as_mut()
                {
                    e.field.press(at, ev.count(), false);
                }
                return Response::changed();
            }
            Some(Phase::Drag) if self.edit.as_ref().is_some_and(|e| e.field.pressed()) => {
                if let Some(at) = self.edit_offset(pos)
                    && let Some(e) = self.edit.as_mut()
                {
                    e.field.drag(at);
                }
                return Response::changed();
            }
            _ => {}
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_press(id, pos, ev.count()).into(),
            Some(Phase::Drag) => self.on_drag(id, pos).into(),
            Some(Phase::Click) => {
                if let Some(r) = self.on_click_editor_in(rows, id) {
                    return r;
                }
                if id == self.check_all_id() {
                    return self.on_click_check_all_in(rows).into();
                }
                if !ev.on(self.id) {
                    return Response::ignored();
                }
                if let Some(d) = self.hit_row(ev, Part::MARKER, keyed) {
                    return self.on_click_check_in(rows, d).into();
                }
                if let Some(c) = ev.index_of(Part::HEADER) {
                    return self.on_click_header_in(rows, c);
                }

                let col = |t: &Self| t.column_at(pos.x).unwrap_or(t.cursor_col);
                let at = match (
                    self.hit_row(ev, Part::ROW, keyed),
                    self.hit_row(ev, Part::CELL, keyed),
                ) {
                    (Some(row), _) => Some((row, self.cursor_col)),
                    (None, Some(row)) => Some((row, col(self))),
                    _ => None,
                };
                match at {
                    Some((row, col)) => {
                        self.on_click_cell_count_in(rows, row, col, pos, ev.mods(), ev.count())
                    }
                    None => Response::ignored(),
                }
            }
            _ => Response::ignored(),
        }
    }
}

/// Why a [`KeyedTable`] rejected its input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyedTableError {
    Shape(DataError),
    Key(KeyedSourceError),
}

/// An owned keyed table store: columns plus rows addressed by [`ItemKey`].
///
/// It validates every row against the column count (the #42 rule), advances
/// its own revisions on each mutation and implements [`TableEditSource`], so a
/// [`DataTable`] can show and edit it without copying its cells. A `Clone` is
/// an independent store with a fresh instance.
#[derive(Debug, Clone)]
pub struct KeyedTable {
    columns: Vec<Column>,
    rows: KeyedVec<Vec<Cell>>,
}

impl KeyedTable {
    pub fn try_new(
        columns: Vec<Column>,
        rows: impl IntoIterator<Item = (ItemKey, Vec<Cell>)>,
    ) -> Result<Self, KeyedTableError> {
        let mut table = Self {
            columns,
            rows: KeyedVec::new(),
        };
        table.replace_rows(rows)?;
        Ok(table)
    }

    fn check(&self, row: usize, cells: &[Cell]) -> Result<(), KeyedTableError> {
        if self.columns.is_empty() {
            return Err(KeyedTableError::Shape(DataError::EmptySchemaWithRows));
        }
        if cells.len() == self.columns.len() {
            return Ok(());
        }
        Err(KeyedTableError::Shape(DataError::Shape(ShapeError {
            row,
            expected: self.columns.len(),
            actual: cells.len(),
        })))
    }

    pub fn replace_rows(
        &mut self,
        rows: impl IntoIterator<Item = (ItemKey, Vec<Cell>)>,
    ) -> Result<(), KeyedTableError> {
        let rows: Vec<(ItemKey, Vec<Cell>)> = rows.into_iter().collect();
        for (at, (_, cells)) in rows.iter().enumerate() {
            self.check(at, cells)?;
        }
        self.rows.replace(rows).map_err(KeyedTableError::Key)
    }

    pub fn insert(
        &mut self,
        position: usize,
        key: ItemKey,
        cells: Vec<Cell>,
    ) -> Result<(), KeyedTableError> {
        self.check(position, &cells)?;
        self.rows
            .insert(position, key, cells)
            .map_err(KeyedTableError::Key)
    }

    pub fn remove(&mut self, key: ItemKey) -> Option<Vec<Cell>> {
        self.rows.remove(key)
    }

    pub fn move_row(&mut self, key: ItemKey, position: usize) -> bool {
        self.rows.move_to(key, position)
    }

    /// Edits one cell; the store advances its data revision.
    pub fn update_cell(&mut self, key: ItemKey, col: usize, edit: impl FnOnce(&mut Cell)) -> bool {
        if col >= self.columns.len() {
            return false;
        }
        self.rows
            .update(key, |cells| edit(&mut cells[col]))
            .is_some()
    }

    pub fn row(&self, key: ItemKey) -> Option<&[Cell]> {
        self.rows.get(key).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn row_keys(&self) -> impl Iterator<Item = ItemKey> + '_ {
        self.rows.iter().map(|(key, _)| key)
    }
}

impl RowSource for KeyedTable {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn label(&self, i: usize) -> &str {
        self.rows
            .value_at(i)
            .and_then(|cells| cells.first())
            .map_or("", |cell| cell.text.as_str())
    }

    fn key(&self, i: usize) -> ItemKey {
        self.rows.key_at(i).unwrap_or(ItemKey::Index(i))
    }
}

impl KeyedRowSource for KeyedTable {
    fn stamp(&self) -> crate::core::list::SourceStamp {
        self.rows.stamp()
    }

    fn source_len(&self) -> usize {
        self.rows.len()
    }

    fn source_key(&self, source_index: usize) -> ItemKey {
        self.key(source_index)
    }
}

impl TableSource for KeyedTable {
    fn columns(&self) -> &[Column] {
        &self.columns
    }

    fn cell(&self, visible_row: usize, column: usize) -> CellRef<'_> {
        self.rows
            .value_at(visible_row)
            .and_then(|cells| cells.get(column))
            .map_or(EMPTY_CELL, Cell::borrowed)
    }
}

impl TableEditSource for KeyedTable {
    fn set_cell(&mut self, row: ItemKey, column: usize, text: String) -> Result<(), String> {
        let done = self.update_cell(row, column, |cell| {
            cell.text = text;
            cell.error = None;
        });
        if done {
            Ok(())
        } else {
            Err("row no longer exists".to_owned())
        }
    }
}

/// The visible order of a table over its rows, for cursor movement and
/// type-ahead.
struct View<'a, R: ?Sized> {
    table: &'a DataTable,
    rows: &'a R,
}

impl<R: TableRows + ?Sized> RowSource for View<'_, R> {
    fn len(&self) -> usize {
        self.table.order.len()
    }

    fn enabled(&self, d: usize) -> bool {
        !self.table.is_disabled_in(self.rows, self.table.order[d])
    }

    fn label(&self, d: usize) -> &str {
        let col = self
            .table
            .type_ahead_column
            .unwrap_or(self.table.cursor_col);
        self.rows
            .cell(self.table.order[d], col)
            .map_or("", |cell| cell.text)
    }
}

#[cfg(test)]
mod stale_session_tests {
    use super::*;

    /// A text session forged at `row`, bound to the table's current structural
    /// generation, as one that survived a row-set change would be.
    fn forge_session(t: &mut DataTable, row: usize, text: &str) {
        t.edit = Some(EditState {
            row,
            col: 0,
            field: EditField::single(text),
            generation: t.structural_generation,
        });
    }

    #[test]
    fn commit_refuses_a_session_whose_row_left_the_set() {
        let mut t = DataTable::try_new(
            WidgetId::of("t.stale"),
            vec![Column::new("name", Constraint::Length(10)).editable()],
            vec![vec![Cell::new("x")], vec![Cell::new("y")]],
        )
        .expect("rectangular fixture")
        .cell_nav(true);
        t.try_set_rows(vec![vec![Cell::new("x")]])
            .expect("rectangular");
        assert!(t.edit.is_none(), "a row-set change closes real sessions");
        forge_session(&mut t, 1, "phantom");

        assert_eq!(t.commit_edit(), Some(TableAction::Cancelled));
        assert!(!t.is_editing(), "the stale session is consumed");
        assert_eq!(t.rows().len(), 1, "no phantom row appears");
        assert_eq!(t.rows()[0][0].text, "x");
    }
}
