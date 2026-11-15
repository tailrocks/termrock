use ratatui::buffer::Buffer;
use std::collections::BTreeSet;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier};

use crate::core::event::{ClientPlatform, Key, Outcome};
use crate::core::focus::BlurAction;
use crate::core::hit::Axis;
use crate::core::hit::PartMap;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::unassigned_chord;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup, select_all_bindings};
use crate::core::list::{Cursor, KeyStep, Moved, Navigable, RowSource};
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

#[derive(Debug, Clone)]
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

enum Refused {
    Ineligible,
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct ChoiceEdit {
    pub row: usize,
    pub col: usize,
    pub editor: ChoiceEditor,
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub row: usize,
    pub col: usize,

    pub field: EditField,
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
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,

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

    pub editor_config: EditorConfig,
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
pub enum TableAction {
    Committed { row: usize, col: usize },
    Cancelled,
    Activated(usize),

    SelectionChanged,

    LeaveForward,
    LeaveBackward,
}

impl DataTable {
    pub fn new(id: WidgetId, columns: Vec<Column>, rows: Vec<Vec<Cell>>) -> Self {
        let n = rows.len();
        let numeric = vec![false; columns.len()];
        Self {
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
            editor_config: EditorConfig::default(),
        }
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
        let (Some(row), Some(c)) = (self.rows.get(src), self.columns.get(col)) else {
            return false;
        };
        c.editor.out_of_set(&row[col].text, &self.editor_config)
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

    pub fn select_all(&mut self) -> bool {
        if !self.multi() {
            return false;
        }
        let disabled = &self.disabled;
        self.selection
            .select_all(self.order.iter().copied(), |r| !disabled.contains(&r))
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
        let disabled = &self.disabled;
        src < self.rows.len() && self.selection.toggle(src, |r| !disabled.contains(&r))
    }

    pub fn set_row(&mut self, src: usize, cells: Vec<Cell>) {
        if src >= self.rows.len() {
            return;
        }
        let cursor_src = self.order.get(self.cursor_row).copied();
        self.rows[src] = cells;
        if let Some((c, d)) = self.sort {
            self.apply_sort(c, d);
            if let Some(pos) = cursor_src.and_then(|s| self.order.iter().position(|&r| r == s)) {
                self.cursor_row = pos;
                self.scroll.ensure_visible(pos);
            }
        }
    }

    pub fn remove_rows(&mut self, srcs: &[usize]) {
        let gone: BTreeSet<usize> = srcs.iter().copied().collect();
        let map = |r: usize| (!gone.contains(&r)).then(|| r - gone.range(..r).count());
        let cursor_src = self.order.get(self.cursor_row).copied();
        let len = self.rows.len();
        self.edit = None;
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
        if display >= self.rows.len() || !self.multi() {
            return (Outcome::Consumed, None);
        }
        self.point_cursor(display, self.cursor_col);
        let src = self.order[display];
        if self.toggle_row(src) {
            (Outcome::Changed, Some(TableAction::SelectionChanged))
        } else {
            (Outcome::Changed, None)
        }
    }

    pub fn on_click_check_all(&mut self) -> (Outcome, Option<TableAction>) {
        if !self.multi() {
            return (Outcome::Consumed, None);
        }
        let disabled = &self.disabled;
        if self
            .selection
            .toggle_all(self.order.iter().copied(), |r| !disabled.contains(&r))
        {
            (Outcome::Changed, Some(TableAction::SelectionChanged))
        } else {
            (Outcome::Consumed, None)
        }
    }

    fn on_key_multi(&mut self, key: &Key) -> Option<(Outcome, Option<TableAction>)> {
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
                Some(changed(self.toggle_row(src)))
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
                            .apply(nav, self, self.scroll.viewport_len)
                            .index()
                    })
                    .unwrap_or(from);
                self.set_cursor(to, self.cursor_col);
                let disabled = &self.disabled;
                let c = self.selection.apply(
                    SelectGesture::Extend { from },
                    self.cursor_row,
                    &self.order,
                    |r| !disabled.contains(&r),
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
    fn cell_part(display: usize) -> PartRef {
        PartRef::item(Part::CELL, ItemKey::index(display))
    }

    pub fn set_rows(&mut self, rows: Vec<Vec<Cell>>) {
        self.edit = None;
        self.choice = None;
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
        let numeric = self.numeric.get(col).copied().unwrap_or(false);
        let rows = &self.rows;
        self.order.sort_by(|&a, &b| {
            let ca = &rows[a][col].text;
            let cb = &rows[b][col].text;
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
        if col >= self.columns.len() || !self.columns[col].sortable {
            return Outcome::Consumed;
        }

        let src = self.order.get(self.cursor_row).copied();
        self.sort = match self.sort {
            Some((c, SortDir::Asc)) if c == col => Some((col, SortDir::Desc)),
            Some((c, SortDir::Desc)) if c == col => None,
            _ => Some((col, SortDir::Asc)),
        };
        match self.sort {
            Some((c, d)) => self.apply_sort(c, d),
            None => self.order = (0..self.rows.len()).collect(),
        }
        if let Some(src) = src
            && let Some(pos) = self.order.iter().position(|&r| r == src)
        {
            self.cursor_row = pos;
            self.scroll.ensure_visible(pos);
        }
        Outcome::Changed
    }

    fn set_cursor(&mut self, row: usize, col: usize) {
        self.cursor_row = row.min(self.rows.len().saturating_sub(1));
        self.cursor_col = col.min(self.columns.len().saturating_sub(1));
        self.scroll.ensure_visible(self.cursor_row);
        self.ensure_col_visible();
    }

    fn point_cursor(&mut self, row: usize, col: usize) {
        self.scroll.pointer_move();
        self.set_cursor(row, col);
    }

    fn ensure_col_visible(&mut self) {
        self.scroll.reveal_column(self.cursor_col);
    }

    pub fn begin_edit(&mut self) -> Outcome {
        self.start_edit().0
    }

    pub fn start_edit(&mut self) -> (Outcome, Option<TableAction>) {
        if self.rows.is_empty() {
            return (Outcome::Consumed, None);
        }
        let col = self.cursor_col;
        let Some(&src) = self.order.get(self.cursor_row) else {
            return (Outcome::Consumed, None);
        };
        if !self.editable_target(src, col) {
            return (Outcome::Consumed, None);
        }

        self.scroll.ensure_visible(self.cursor_row);
        self.ensure_col_visible();
        let text = self.rows[src][col].text.clone();
        let editor = &self.columns[col].editor;
        if let Some(next) = editor.toggled(&text) {
            return (Outcome::Changed, self.apply_value(src, col, next).ok());
        }
        if editor.is_constrained() {
            let opened = ChoiceEditor::open(self.choice_id(), editor, &text, &self.editor_config);
            self.choice = opened.map(|editor| ChoiceEdit {
                row: src,
                col,
                editor,
            });
            return (Outcome::Changed, None);
        }
        let mut field = EditField::single(text);
        field.validator = self.validator.as_ref().map(|v| v.bind(col));
        self.edit = Some(EditState {
            row: src,
            col,
            field,
        });
        (Outcome::Changed, None)
    }

    fn editable_target(&self, src: usize, col: usize) -> bool {
        !self.is_disabled(src)
            && self.columns.get(col).is_some_and(|c| c.editable)
            && self.rows.get(src).and_then(|row| row.get(col)).is_some()
    }

    fn apply_value(
        &mut self,
        src: usize,
        col: usize,
        text: String,
    ) -> Result<TableAction, Refused> {
        if !self.editable_target(src, col) {
            return Err(Refused::Ineligible);
        }
        if let Some(Err(err)) = self.validator.as_ref().map(|v| v.validate_in(&col, &text)) {
            return Err(Refused::Invalid(err.into_message()));
        }
        self.rows[src][col].text = text;
        self.rows[src][col].error = None;
        if let Some((c, d)) = self.sort
            && c == col
        {
            self.apply_sort(c, d);
            if let Some(pos) = self.order.iter().position(|&r| r == src) {
                self.cursor_row = pos;
                self.scroll.ensure_visible(pos);
            }
        }
        Ok(TableAction::Committed { row: src, col })
    }

    fn on_choice(&mut self, r: Option<ChoiceResult>) -> Option<TableAction> {
        let (value, tab) = match r? {
            ChoiceResult::Cancel => {
                self.choice = None;
                return Some(TableAction::Cancelled);
            }
            ChoiceResult::Commit(v) => (v, None),
            ChoiceResult::CommitAndTab { value, backward } => (value, Some(backward)),
        };
        let (src, col) = self.choice.as_ref().map(|c| (c.row, c.col))?;
        match self.apply_value(src, col, value) {
            Err(Refused::Ineligible) => {
                self.choice = None;
                Some(TableAction::Cancelled)
            }
            Ok(ev) => {
                self.choice = None;
                match tab {
                    Some(backward) => match self.tab_to_editable(backward) {
                        Some(leave) => Some(leave),
                        None => {
                            self.start_edit();
                            Some(ev)
                        }
                    },
                    None => Some(ev),
                }
            }
            Err(Refused::Invalid(msg)) => {
                if let Some(c) = self.choice.as_mut() {
                    c.editor.error = Some(msg);
                }
                None
            }
        }
    }

    fn tab_to_editable(&mut self, backward: bool) -> Option<TableAction> {
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
        self.set_cursor(self.cursor_row, col);
        None
    }

    pub fn on_click_editor(&mut self, id: WidgetId) -> Option<(Outcome, Option<TableAction>)> {
        if let Some(c) = self.choice.as_mut()
            && c.editor.contains_id(id)
        {
            let (out, r) = c.editor.on_click(id);
            return Some((out, self.on_choice(r)));
        }
        let (di, ci) = self.parts.cell(id, Part::ACTIONS)?;
        self.point_cursor(di, ci);
        Some(self.start_edit())
    }

    pub fn commit_edit(&mut self) -> Option<TableAction> {
        let mut e = self.edit.take()?;
        if !self.editable_target(e.row, e.col) {
            return Some(TableAction::Cancelled);
        }
        if !e.field.validate() {
            self.edit = Some(e);
            return None;
        }
        let text = e.field.text().to_owned();
        match self.apply_value(e.row, e.col, text) {
            Ok(action) => Some(action),
            Err(Refused::Ineligible) => Some(TableAction::Cancelled),
            Err(Refused::Invalid(_)) => {
                self.edit = Some(e);
                None
            }
        }
    }

    pub fn cancel_edit(&mut self) -> Option<TableAction> {
        if self.choice.take().is_some() {
            return Some(TableAction::Cancelled);
        }
        self.edit.take().map(|_| TableAction::Cancelled)
    }

    pub fn on_blur(&mut self) -> Option<TableAction> {
        if let Some(c) = &self.choice {
            return match self.editor_config.blur {
                BlurAction::Commit => match c.editor.blur_value(&self.editor_config) {
                    Some(v) => self.on_choice(Some(ChoiceResult::Commit(v))),
                    None => self.on_choice(Some(ChoiceResult::Cancel)),
                },
                BlurAction::Cancel => self.on_choice(Some(ChoiceResult::Cancel)),
                BlurAction::Keep => None,
            };
        }
        self.edit.as_ref()?;
        match self.blur_action {
            BlurAction::Commit => self.commit_edit(),
            BlurAction::Cancel => self.cancel_edit(),
            BlurAction::Keep => None,
        }
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<TableAction>) {
        if let Some(c) = self.choice.as_mut() {
            let (out, r) = c.editor.on_key(key, &self.editor_config);
            return (out, self.on_choice(r));
        }
        if let Some(e) = self.edit.as_mut() {
            let r = e.field.on_key(key);
            return match r.exit {
                Some(EditExit::Commit) => match self.commit_edit() {
                    Some(ev) => (Outcome::Changed, Some(ev)),
                    None => (Outcome::Changed, None),
                },
                Some(EditExit::Cancel) => (Outcome::Changed, self.cancel_edit()),
                Some(EditExit::Tab { backward }) => {
                    let ev = self.commit_edit();
                    if ev.is_none() || ev == Some(TableAction::Cancelled) {
                        return (Outcome::Changed, ev);
                    }

                    if let Some(leave) = self.tab_to_editable(backward) {
                        return (Outcome::Changed, Some(leave));
                    }
                    self.start_edit();
                    (Outcome::Changed, ev)
                }
                None => (r.outcome, None),
            };
        }
        if self.rows.is_empty() {
            return (Outcome::Ignored, None);
        }
        if let Some(KeyStep::Type(c)) = KeyStep::ordered(
            key,
            &self.nav,
            self.type_ahead,
            self.type_ahead_precedence
                .unwrap_or_else(crate::core::list::TypeAheadPrecedence::current),
        ) {
            let mut ta = std::mem::take(&mut self.typeahead);
            let moved = self.cursor().type_ahead(c, &mut ta, self);
            self.typeahead = ta;
            if let Moved::To(d) = moved {
                self.set_cursor(d, self.cursor_col);
            }
            return (moved.outcome(), None);
        }
        if self.multi()
            && let Some(r) = self.on_key_multi(key)
        {
            return r;
        }
        if unassigned_chord(key, &self.keymap) {
            return (Outcome::Ignored, None);
        }
        let (r, c) = (self.cursor_row, self.cursor_col);
        if let Some(action) = lookup(&self.keymap, key) {
            return match action {
                TableCmd::SortByColumn => (self.sort_by(c), None),
                TableCmd::SelectAll if self.multi() => {
                    if self.select_all() {
                        (Outcome::Changed, Some(TableAction::SelectionChanged))
                    } else {
                        (Outcome::Consumed, None)
                    }
                }

                TableCmd::SelectAll => (Outcome::Ignored, None),
            };
        }

        if key.code == KeyCode::Down
            && key.alt()
            && self.cell_nav
            && self.columns[c].editable
            && self.columns[c].editor.has_popup()
        {
            return self.start_edit();
        }
        if let Some(nav) = self.nav.action(key) {
            return self.on_nav(nav);
        }
        match key.code {
            KeyCode::Enter | KeyCode::F(2) if self.cell_nav && self.columns[c].editable => {
                self.start_edit()
            }

            KeyCode::Char(' ')
                if self.cell_nav
                    && self.columns[c].editable
                    && self.columns[c].editor.is_constrained() =>
            {
                self.start_edit()
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let src = self.order[r];
                if self.is_disabled(src) {
                    return (Outcome::Consumed, None);
                }
                if self.selection_mode == SelectionMode::Single {
                    self.selected = Some(src);
                }
                (Outcome::Changed, Some(TableAction::Activated(src)))
            }
            _ => (Outcome::Ignored, None),
        }
    }

    fn on_nav(&mut self, nav: Nav) -> (Outcome, Option<TableAction>) {
        let (r, c) = (self.cursor_row, self.cursor_col);
        match nav {
            Nav::Up | Nav::Down | Nav::PageUp | Nav::PageDown | Nav::Home | Nav::End => {
                let to = self
                    .cursor()
                    .apply(nav, self, self.scroll.viewport_len)
                    .index()
                    .unwrap_or(r);
                self.set_cursor(to, c);
                (Outcome::Changed, None)
            }
            Nav::Left if self.cell_nav => {
                self.set_cursor(r, c.saturating_sub(1));
                (Outcome::Changed, None)
            }
            Nav::Right if self.cell_nav => {
                self.set_cursor(r, c + 1);
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

    pub fn on_click_header(&mut self, col: usize) -> Outcome {
        if self.edit.is_some() {
            self.commit_edit();
        }
        self.sort_by(col)
    }

    pub fn on_click_cell(
        &mut self,
        display: usize,
        col: usize,
        pos: Position,
    ) -> (Outcome, Option<TableAction>) {
        self.on_click_cell_count(display, col, pos, KeyModifiers::NONE, 1)
    }

    pub fn on_click_cell_with(
        &mut self,
        display: usize,
        col: usize,
        pos: Position,
        mods: KeyModifiers,
    ) -> (Outcome, Option<TableAction>) {
        self.on_click_cell_count(display, col, pos, mods, 1)
    }

    pub fn on_click_cell_count(
        &mut self,
        display: usize,
        col: usize,
        pos: Position,
        mods: KeyModifiers,
        count: u8,
    ) -> (Outcome, Option<TableAction>) {
        let count = count.max(1);
        if display >= self.rows.len() {
            return (Outcome::Consumed, None);
        }

        self.choice = None;
        let col = col.min(self.columns.len().saturating_sub(1));
        if let Some(e) = &self.edit {
            if e.row == self.order[display] && e.col == col {
                if let Some(at) = self.edit_offset(pos)
                    && let Some(e) = self.edit.as_mut()
                {
                    e.field.click(at, count, false);
                }
                return (Outcome::Changed, None);
            }
            self.commit_edit();
            if self.edit.is_some() {
                return (Outcome::Changed, None);
            }
        }
        let same = self.cursor_row == display && (!self.cell_nav || self.cursor_col == col);
        let from = self.cursor_row;
        self.point_cursor(display, if self.cell_nav { col } else { self.cursor_col });
        if self.multi() && !self.cell_nav {
            let src = self.order[display];
            let disabled = &self.disabled;
            let enabled = |r: usize| !disabled.contains(&r);
            let gesture = SelectGesture::from_mods(mods, self.toggle_mods, from);
            if gesture == SelectGesture::Only && !enabled(src) {
                return (Outcome::Changed, None);
            }
            let changed = self.selection.apply(gesture, display, &self.order, enabled);
            if gesture == SelectGesture::Only && self.click.activates(count, same) {
                return (Outcome::Changed, Some(TableAction::Activated(src)));
            }
            return (
                Outcome::Changed,
                changed.then_some(TableAction::SelectionChanged),
            );
        }
        if self.cell_nav && self.columns[col].editable && self.click.edits(count, same) {
            return self.start_edit();
        }
        if !self.cell_nav {
            let src = self.order[display];
            if self.is_disabled(src) {
                return (Outcome::Changed, None);
            }
            if self.selection_mode == SelectionMode::Single {
                self.selected = Some(src);
            }
            if self.click.activates(count, same) {
                return (Outcome::Changed, Some(TableAction::Activated(src)));
            }
        }
        (Outcome::Changed, None)
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

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
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
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        let editing = self.edit.is_some() && focused;

        let mut choice_anchor: Option<Rect> = None;
        let mut open_marks: Vec<(usize, usize, Rect)> = Vec::new();

        ctx.control(self.id, area, false);
        ctx.scrollable(self.id, area);

        let h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        self.scroll.h_cfg = h_cfg;

        let rows_len = self.rows.len();
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

        let has_hb = !self.rows.is_empty() && body.height > 1 && h_cfg.shows_bar(&self.scroll.h);
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
            let state = self.selection_state();
            let lifted = ctx.interaction.hovered(cid) || ctx.interaction.pressed(cid);
            let st = crate::widgets::grid::header_style(ctx, cid, header_bg, true, false);
            let st = match state {
                _ if lifted => st,
                TriState::None => st,
                _ => st.fg(t.legible(ctx.chosen(focused), st.bg.unwrap_or(header_bg))),
            };
            let r = Rect::new(area.x + 3, header_y, 3, 1);
            buf.set_string(r.x, r.y, state.glyph_in(&t.glyphs), st);
            if !self.rows.is_empty() {
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

        if self.rows.is_empty() {
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
            let y = body.y + i as u16;
            let src = self.order[di];
            let row_rect = Rect::new(
                area.x,
                y,
                area.width.saturating_sub(if has_sb { 1 } else { 0 }),
                1,
            );
            let rid = self.row_id(di);
            let mut s = ctx.child_state(self.id, rid, di == self.cursor_row);

            let hovered_cell = (0..self.columns.len()).find(|&c| {
                ctx.interaction.hovered(self.cell_id(di, c))
                    || ctx.interaction.hovered(self.open_id(di, c))
            });
            s.hovered |= hovered_cell.is_some();
            s.pressed |= ctx
                .interaction
                .pressed_any((0..self.columns.len()).map(|c| self.cell_id(di, c)));
            s.focused = focused && di == self.cursor_row;
            s.selected = match self.selection_mode {
                SelectionMode::Single => self.selected == Some(src),
                SelectionMode::Multi => self.selection.contains(src),
                SelectionMode::None => false,
            };
            let row_disabled = self.is_disabled(src);
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
                let cell = &self.rows[src][ci];
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

                let shown: std::borrow::Cow<str> = if let Some(on) = col.editor.checked(&cell.text)
                {
                    if on {
                        t.glyphs.check_on
                    } else {
                        t.glyphs.check_off
                    }
                    .into()
                } else if constrained {
                    col.editor.display(&cell.text, cfg).into()
                } else {
                    cell.text.as_str().into()
                };
                let flagged = cell.error.is_some()
                    || (cfg.flag_out_of_set && col.editor.out_of_set(&cell.text, cfg));
                let tone = match (cell.tone, col.editor.tone(&cell.text)) {
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
                        self.cell_id(di, ci),
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
                        open_marks.push((di, ci, mark));
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
                let cid = self.cell_id(di, ci);
                let cell = (Self::cell_part(di), ci);
                ctx.register_part_as(cid, self.id, cell.0, cell_rect);
            }
            ctx.register_part(self.id, Self::row_part(di), row_rect);

            for ci in 0..self.columns.len() {
                let r = self.col_rects[ci];
                if !r.is_empty() {
                    let cid = self.cell_id(di, ci);
                    let cell = (Self::cell_part(di), ci);
                    let rect = Rect::new(r.x, y, r.width, 1);
                    ctx.register_part_as(cid, self.id, cell.0, rect);
                }
            }
            if check_w > 0 && area.width > 3 + check_w && !row_disabled {
                let rect = Rect::new(area.x + 3, y, 3, 1);
                ctx.register_part(self.id, Self::check_part(di), rect);
            }
        }

        for (di, ci, rect) in open_marks {
            let oid = self.open_id(di, ci);
            let open = (PartRef::item(Part::ACTIONS, ItemKey::index(di)), ci);

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

    fn on_edit_cell(&self, ev: &crate::core::pointer::PointerEvent) -> bool {
        let Some(e) = &self.edit else {
            return false;
        };
        ev.on(self.id)
            && ev
                .index_of(Part::CELL)
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
        self.on_blur();
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
        use crate::core::intent::Phase;
        use crate::core::response::Response;

        if let Some(c) = self.choice.as_mut()
            && ev.on(c.editor.id)
            && ev.wheel().is_none()
        {
            let (out, r) = c.editor.pointer(ev).into_parts();
            return (out, self.on_choice(r)).into();
        }

        if (ev.on(self.id) || ev.wheel().is_some())
            && let Some(r) = crate::core::pointer::scroll_input(self, ev)
        {
            return r.lift();
        }
        let (id, pos) = (ev.id, ev.pos());

        match ev.phase() {
            Some(Phase::Press) if self.on_edit_cell(ev) => {
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
                if let Some(r) = self.on_click_editor(id) {
                    return r.into();
                }
                if id == self.check_all_id() {
                    return self.on_click_check_all().into();
                }
                if !ev.on(self.id) {
                    return Response::ignored();
                }
                if let Some(d) = ev.index_of(Part::MARKER) {
                    return self.on_click_check(d).into();
                }
                if let Some(c) = ev.index_of(Part::HEADER) {
                    return self.on_click_header(c).into();
                }

                let col = |t: &Self| t.column_at(pos.x).unwrap_or(t.cursor_col);
                let at = match (ev.index_of(Part::ROW), ev.index_of(Part::CELL)) {
                    (Some(row), _) => Some((row, self.cursor_col)),
                    (None, Some(row)) => Some((row, col(self))),
                    _ => None,
                };
                match at {
                    Some((row, col)) => self
                        .on_click_cell_count(row, col, pos, ev.mods(), ev.count())
                        .into(),
                    None => Response::ignored(),
                }
            }
            _ => Response::ignored(),
        }
    }
}
