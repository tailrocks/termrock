use std::collections::{BTreeSet, HashMap};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

use crate::core::event::{Key, Outcome};
use crate::core::focus::BlurAction;
use crate::core::hit::Axis;
use crate::core::hit::PartMap;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use crate::core::list::{Cursor, KeyStep, Moved, Navigable, RowFn, RowSource};
use crate::core::response::Response;
use crate::core::scroll::{HScroll, ScrollPane, Scrollable};
use crate::core::selection::{ClickPolicy, RowSelection, SelectGesture};
use crate::theme::{SyntaxTone, Theme};
use crate::ui::columns::{ColumnSizing, HeaderAction, HeaderLabels};
use crate::ui::ctx::{RenderCtx, VisualState, fill};
use crate::ui::row::{MarkKind, RowLook, RowUi};
use crate::ui::text::{fit, fit_right, truncate, truncate_middle, wider_than, width, width_capped};
use crate::widgets::button::{Button, row_layout_right};
use crate::widgets::edit::{EditExit, EditField, FieldError, Validator};
use crate::widgets::option_editor::{
    Affordance, ChoiceEditor, ChoiceResult, ColumnEditor, EditorConfig,
};
use crate::widgets::scrollbar;
use crate::widgets::table::SortDir;

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Null,

    Default,
    Text(String),
    Int(i64),
    Num(f64),
    Bool(bool),
    Json(String),
}

impl CellValue {
    pub fn text(&self) -> String {
        self.text_ref().into_owned()
    }

    pub fn text_ref(&self) -> std::borrow::Cow<'_, str> {
        match self {
            CellValue::Null => "NULL".into(),
            CellValue::Default => "DEFAULT".into(),
            CellValue::Text(s) | CellValue::Json(s) => s.as_str().into(),
            CellValue::Int(i) => i.to_string().into(),
            CellValue::Num(n) => format!("{n:.2}").into(),
            CellValue::Bool(b) => b.to_string().into(),
        }
    }

    pub fn edit_text(&self) -> String {
        match self {
            CellValue::Null | CellValue::Default => String::new(),
            v => v.text(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Text,
    Id,
    Number,
    Bool,
    Timestamp,
    Json,
    Enum,
}

impl CellKind {
    fn default_width(self) -> (u16, u16) {
        match self {
            CellKind::Id => (9, 36),
            CellKind::Text => (6, 40),
            CellKind::Number => (4, 22),
            CellKind::Bool => (5, 5),
            CellKind::Timestamp => (10, 29),
            CellKind::Json => (8, 40),
            CellKind::Enum => (6, 16),
        }
    }
    fn right_aligned(self) -> bool {
        self == CellKind::Number
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: String,
    pub kind: CellKind,
    pub primary: bool,
    pub nullable: bool,
    pub read_only: bool,
    pub references: Option<String>,
    pub enum_values: Vec<String>,
    pub sortable: bool,
    pub min_width: u16,
    pub max_width: u16,

    pub type_label: String,

    pub editor: ColumnEditor,
}

impl ColumnSpec {
    pub fn new(name: &str, kind: CellKind) -> Self {
        let (min_width, max_width) = kind.default_width();
        Self {
            name: name.to_owned(),
            kind,
            primary: false,
            nullable: true,
            read_only: false,
            references: None,
            enum_values: vec![],
            sortable: kind != CellKind::Json,
            min_width,
            max_width,
            type_label: String::new(),
            editor: ColumnEditor::Text,
        }
    }

    pub fn editor(mut self, editor: ColumnEditor) -> Self {
        self.editor = editor;
        self
    }
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }
    pub fn nullable(mut self, n: bool) -> Self {
        self.nullable = n;
        self
    }
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
    pub fn references(mut self, t: &str) -> Self {
        self.references = Some(t.to_owned());
        self
    }
    pub fn type_label(mut self, l: &str) -> Self {
        self.type_label = l.to_owned();
        self
    }
    pub fn enum_values(mut self, v: &[&str]) -> Self {
        self.enum_values = v.iter().map(|s| (*s).to_owned()).collect();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowTotal {
    Exact(usize),
    Estimated(usize),
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridRows {
    pub rows: Vec<Vec<CellValue>>,
    pub total: RowTotal,

    pub more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridRevision {
    pub source: u64,
    pub draft: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridKeyError {
    KeyCount { expected: usize, actual: usize },
    DuplicateKey(ItemKey),
}

impl std::fmt::Display for GridKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyCount { expected, actual } => {
                write!(f, "expected {expected} row keys, got {actual}")
            }
            Self::DuplicateKey(key) => write!(f, "row key {key:?} appears more than once"),
        }
    }
}

impl std::error::Error for GridKeyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridUndoKind {
    Cell,
    DeleteMark,
    Insert,
    RemovedInsert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridUndo {
    pub key: ItemKey,
    pub kind: GridUndoKind,
}

#[derive(Debug, Clone)]
enum UndoAction {
    Cell {
        key: ItemKey,
        col: usize,
        before: Option<CellValue>,
    },
    Delete {
        key: ItemKey,
        was_deleted: bool,
    },
    Insert {
        key: ItemKey,
    },
    RemoveInserted {
        row: Box<RemovedDraftRow>,
    },
}

#[derive(Debug, Clone)]
struct RemovedDraftRow {
    key: ItemKey,
    position: usize,
    base: Vec<CellValue>,
    cells: Vec<(usize, CellValue)>,
    cell_errors: Vec<(usize, String)>,
    row_error: Option<String>,
    view: StructuralViewSnapshot,
}

#[derive(Debug, Clone)]
struct StructuralViewSnapshot {
    source_keys: Vec<ItemKey>,
    selection: RowSelection,
    cursor: Option<(ItemKey, usize)>,
    anchor: Option<(ItemKey, usize)>,
    scroll_offset: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingChanges {
    cells: HashMap<(ItemKey, usize), CellValue>,
    inserted: BTreeSet<ItemKey>,
    deleted: BTreeSet<ItemKey>,
}

impl PendingChanges {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.inserted.is_empty() && self.deleted.is_empty()
    }
    pub fn cells(&self) -> impl Iterator<Item = (ItemKey, usize, &CellValue)> + '_ {
        self.cells.iter().map(|(&(key, col), v)| (key, col, v))
    }
    pub fn inserted(&self) -> impl Iterator<Item = ItemKey> + '_ {
        self.inserted.iter().copied()
    }
    pub fn deleted(&self) -> impl Iterator<Item = ItemKey> + '_ {
        self.deleted.iter().copied()
    }
    pub fn is_inserted(&self, key: ItemKey) -> bool {
        self.inserted.contains(&key)
    }
    pub fn is_deleted(&self, key: ItemKey) -> bool {
        self.deleted.contains(&key)
    }
    pub fn dirty_rows(&self) -> BTreeSet<ItemKey> {
        self.cells
            .keys()
            .map(|(key, _)| *key)
            .filter(|key| !self.inserted.contains(key) && !self.deleted.contains(key))
            .collect()
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.dirty_rows().len(),
            self.inserted.len(),
            self.deleted.len(),
        )
    }
    pub fn total(&self) -> usize {
        let (u, i, d) = self.counts();
        u + i + d
    }
    pub fn is_dirty(&self, key: ItemKey, col: usize) -> bool {
        self.cells.contains_key(&(key, col))
    }
    pub fn value(&self, key: ItemKey, col: usize) -> Option<&CellValue> {
        self.cells.get(&(key, col))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Clean,
    Modified,
    Inserted,
    Deleted,
    Error,
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub row: usize,
    pub col: usize,

    pub field: EditField,
    generation: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GridAction {
    CellChanged { row: usize, col: usize },
    Undone(GridUndo),
    RowInserted(usize),
    RowDeleted(usize),
    SortRequested(Option<(usize, SortDir)>),
    FetchMore,
    Refresh,
    CommitRequested,
    DiscardRequested,
    PreviewSql,
    Copy(String),
    FollowReference { row: usize, col: usize },
    OpenViewer { row: usize, col: usize },
    FilterOnCell { col: usize, value: CellValue },
    OpenFilters,
    ClearFilters,
    Activated(usize),
    LeaveForward,
    LeaveBackward,

    ReadOnly { reason: String },
}

pub type CellParser = Validator<CellValue, ColumnSpec>;

pub fn cell_validator(
    f: impl Fn(&ColumnSpec, &str) -> Result<CellValue, String> + 'static,
) -> CellParser {
    Validator::with_context(move |spec: &ColumnSpec, text: &str| {
        f(spec, text).map_err(FieldError::from)
    })
}

fn validate_row_keys(rows: usize, keys: &[ItemKey]) -> Result<(), GridKeyError> {
    if keys.len() != rows {
        return Err(GridKeyError::KeyCount {
            expected: rows,
            actual: keys.len(),
        });
    }
    let mut seen = std::collections::HashSet::with_capacity(keys.len());
    match keys.iter().find(|&&key| !seen.insert(key)) {
        Some(&key) => Err(GridKeyError::DuplicateKey(key)),
        None => Ok(()),
    }
}

fn parse_cell(parser: &CellParser, spec: &ColumnSpec, text: &str) -> Result<CellValue, FieldError> {
    if text.is_empty() {
        return match spec.kind {
            CellKind::Text | CellKind::Json | CellKind::Enum => Ok(CellValue::Text(String::new())),
            _ if spec.nullable => Ok(CellValue::Null),
            _ => Err(FieldError::from("Empty: use Delete for NULL")),
        };
    }
    parser.validate_in(spec, text)
}

pub fn default_validator(col: &ColumnSpec, text: &str) -> Result<CellValue, String> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("null") {
        return if col.nullable {
            Ok(CellValue::Null)
        } else {
            Err(format!("{} is NOT NULL", col.name))
        };
    }
    match col.kind {
        CellKind::Number => {
            if let Ok(i) = t.parse::<i64>() {
                Ok(CellValue::Int(i))
            } else {
                t.parse::<f64>()
                    .map(CellValue::Num)
                    .map_err(|_| "Must be a number".to_owned())
            }
        }
        CellKind::Bool => match t.to_ascii_lowercase().as_str() {
            "true" | "t" | "1" | "yes" => Ok(CellValue::Bool(true)),
            "false" | "f" | "0" | "no" => Ok(CellValue::Bool(false)),
            _ => Err("Must be true or false".into()),
        },
        CellKind::Json => {
            if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']'))
            {
                Ok(CellValue::Json(t.into()))
            } else {
                Err("Must be a JSON object or array".into())
            }
        }
        CellKind::Enum if !col.enum_values.is_empty() => {
            if col.enum_values.iter().any(|v| v == t) {
                Ok(CellValue::Text(t.into()))
            } else {
                Err(format!("Must be one of: {}", col.enum_values.join(", ")))
            }
        }
        CellKind::Id => {
            if t.len() == 36 && t.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                Ok(CellValue::Text(t.into()))
            } else {
                Err("Must be a UUID".into())
            }
        }
        CellKind::Timestamp => {
            if t.len() >= 10 && t.as_bytes()[4] == b'-' && t.as_bytes()[7] == b'-' {
                Ok(CellValue::Text(t.into()))
            } else {
                Err("Use YYYY-MM-DD".into())
            }
        }
        _ => Ok(CellValue::Text(t.into())),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridDrag {
    Cells(WidgetId),

    Rows {
        id: WidgetId,
        from: usize,
        mark: bool,
    },
}

impl crate::ui::measure::Measure for DataGrid {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        _ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let n = self.columns.len();
        let cols: u16 = (0..n).map(|c| self.col_w(c)).sum::<u16>() + n.saturating_sub(1) as u16;
        ratatui::layout::Size::new(cols + GRID_CHROME_W, self.len() as u16 + 2)
    }

    fn min_width(&self, _avail: ratatui::layout::Size, _ctx: &crate::ui::ctx::RenderCtx) -> u16 {
        GRID_MIN_W
    }
}

const FETCH_MORE_KEY: ItemKey = ItemKey::text("datagrid.fetch-more");

const GRID_CHROME_W: u16 = 6;

const GRID_MIN_W: u16 = 24;

#[derive(Debug, Clone)]
pub struct DataGrid {
    parts: PartMap,
    pub id: WidgetId,
    pub columns: Vec<ColumnSpec>,
    rows: Vec<Vec<CellValue>>,
    row_keys: Vec<ItemKey>,
    key_to_source: HashMap<ItemKey, usize>,
    next_local_key: u64,
    revision: GridRevision,
    order: Vec<usize>,
    pub total: RowTotal,
    pub more: bool,
    pub sort: Option<(usize, SortDir)>,

    pub local_sort: bool,
    pub filtered_cols: BTreeSet<usize>,
    pub cursor: (usize, usize),

    anchor: Option<(usize, usize)>,

    pub selection: RowSelection,
    pending: PendingChanges,
    undo: Vec<UndoAction>,

    pub scroll: ScrollPane,
    pub edit: Option<EditState>,
    pub editable: bool,

    pub click: ClickPolicy,

    pub toggle_mods: KeyModifiers,

    pub blur_action: BlurAction,
    pub read_only_reason: Option<String>,
    pub loading: bool,
    pub cell_errors: HashMap<(usize, usize), String>,
    pub row_errors: HashMap<usize, String>,
    pub empty: crate::widgets::empty::EmptyState,
    pub validator: CellParser,
    pub row_numbers: bool,
    pub area: Rect,
    body: Rect,

    drag: Option<GridDrag>,
    widths: Vec<u16>,

    pub sizing: ColumnSizing,

    pub header_labels: HeaderLabels,
    col_rects: Vec<Rect>,
    show_bar: bool,
    bar: [Button; 3],

    strings: crate::theme::Strings,

    pub reveal: Option<crate::ui::reveal::Reveal>,

    pub horizontal: Option<HScroll>,

    pub overflow_badges: bool,

    pub nav: NavKeys,

    pub keymap: KeyMap<GridCmd>,

    pub type_ahead: bool,

    pub type_ahead_column: Option<usize>,

    pub type_ahead_precedence: Option<crate::core::list::TypeAheadPrecedence>,
    typeahead: crate::core::typeahead::TypeAhead,

    pub width_sample_rows: usize,

    pub choice: Option<GridChoice>,
    structural_generation: u64,

    pub editor_config: EditorConfig,
}

#[derive(Debug, Clone)]
pub struct GridChoice {
    pub row: usize,
    pub col: usize,
    pub editor: ChoiceEditor,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridCmd {
    ClearCell,

    ToggleDelete,

    InsertRow,

    DuplicateRow,

    Copy,

    CopyWithHeader,

    Commit,

    Sort,

    ClearSort,

    FilterOnCell,

    OpenFilters,

    ClearFilters,

    Refresh,

    Undo,

    Discard,

    PreviewSql,

    FollowReference,

    SelectAll,
}

impl Actions for GridCmd {
    fn label(self) -> &'static str {
        match self {
            GridCmd::ClearCell => "Clear cell",
            GridCmd::ToggleDelete => "Mark for deletion",
            GridCmd::InsertRow => "Insert row",
            GridCmd::DuplicateRow => "Duplicate row",
            GridCmd::Copy => "Copy",
            GridCmd::CopyWithHeader => "Copy with header",
            GridCmd::Commit => "Save",
            GridCmd::Sort => "Sort",
            GridCmd::ClearSort => "Clear sort",
            GridCmd::FilterOnCell => "Filter on cell",
            GridCmd::OpenFilters => "Filters",
            GridCmd::ClearFilters => "Clear filters",
            GridCmd::Refresh => "Refresh",
            GridCmd::Undo => "Undo",
            GridCmd::Discard => "Discard",
            GridCmd::PreviewSql => "Preview SQL",
            GridCmd::FollowReference => "Follow reference",
            GridCmd::SelectAll => "Select all",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::key(KeyCode::Delete), GridCmd::ClearCell),
            (KeyBinding::key(KeyCode::Backspace), GridCmd::ClearCell),
            (KeyBinding::char('-'), GridCmd::ToggleDelete),
            (KeyBinding::char('+'), GridCmd::InsertRow),
            (KeyBinding::ctrl('d'), GridCmd::DuplicateRow),
            (KeyBinding::alt('d'), GridCmd::DuplicateRow),
            (KeyBinding::cmd('c'), GridCmd::Copy),
            (KeyBinding::char('y'), GridCmd::Copy),
            (KeyBinding::char('Y'), GridCmd::CopyWithHeader),
            (KeyBinding::ctrl('s'), GridCmd::Commit),
            (KeyBinding::char('s'), GridCmd::Sort),
            (KeyBinding::char('S'), GridCmd::ClearSort),
            (KeyBinding::char('f'), GridCmd::FilterOnCell),
            (KeyBinding::char('/'), GridCmd::OpenFilters),
            (KeyBinding::char('F'), GridCmd::ClearFilters),
            (KeyBinding::char('r'), GridCmd::Refresh),
            (KeyBinding::key(KeyCode::F(5)), GridCmd::Refresh),
            (KeyBinding::char('u'), GridCmd::Undo),
            (KeyBinding::char('U'), GridCmd::Discard),
            (KeyBinding::char('p'), GridCmd::PreviewSql),
            (KeyBinding::ctrl(']'), GridCmd::FollowReference),
            (crate::core::keymap::SELECT_ALL[0], GridCmd::SelectAll),
            (crate::core::keymap::SELECT_ALL[1], GridCmd::SelectAll),
        ]
    }
}

pub const DEFAULT_WIDTH_SAMPLE_ROWS: usize = 200;

impl DataGrid {
    pub fn new(id: WidgetId, columns: Vec<ColumnSpec>) -> Self {
        let widths = columns.iter().map(|c| c.min_width).collect();
        Self {
            id,
            columns,
            rows: vec![],
            row_keys: vec![],
            key_to_source: HashMap::new(),
            next_local_key: 0,
            revision: GridRevision::default(),
            order: vec![],
            total: RowTotal::Unknown,
            more: false,
            sort: None,
            local_sort: false,
            filtered_cols: BTreeSet::new(),
            cursor: (0, 0),
            anchor: None,
            selection: RowSelection::new(),
            pending: PendingChanges::default(),
            undo: vec![],
            scroll: ScrollPane::default().columns(true),
            edit: None,
            editable: true,
            click: ClickPolicy::DATA,
            toggle_mods: SelectGesture::toggle_mods(),
            blur_action: BlurAction::Commit,
            read_only_reason: None,
            loading: false,
            cell_errors: HashMap::new(),
            row_errors: HashMap::new(),
            empty: crate::widgets::empty::EmptyState::new(crate::theme::Strings::DEFAULT.no_rows),
            validator: cell_validator(default_validator),
            row_numbers: true,
            area: Rect::ZERO,
            parts: PartMap::default(),
            body: Rect::ZERO,
            drag: None,
            widths,
            sizing: ColumnSizing::default(),
            header_labels: HeaderLabels::DEFAULT,
            col_rects: vec![],
            show_bar: true,
            bar: [
                Button::subtle(
                    id.sub("preview"),
                    crate::theme::Strings::DEFAULT.preview_sql,
                ),
                Button::subtle(id.sub("discard"), crate::theme::Strings::DEFAULT.discard),
                Button::primary(id.sub("save"), crate::theme::Strings::DEFAULT.save_changes),
            ],
            strings: crate::theme::Strings::DEFAULT,
            reveal: None,
            horizontal: None,
            overflow_badges: true,
            width_sample_rows: DEFAULT_WIDTH_SAMPLE_ROWS,
            nav: NavKeys::default(),
            keymap: GridCmd::default_keymap(),
            type_ahead: false,
            type_ahead_column: None,
            type_ahead_precedence: None,
            typeahead: Default::default(),
            choice: None,
            structural_generation: 0,
            editor_config: EditorConfig::default(),
        }
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

    fn locate_open(&self, id: WidgetId) -> Option<(usize, usize)> {
        self.parts.cell(id, Part::ACTIONS)
    }

    pub fn out_of_set(&self, src: usize, col: usize) -> bool {
        let Some(spec) = self.columns.get(col) else {
            return false;
        };
        match self.value(src, col) {
            CellValue::Text(t) => spec.editor.out_of_set(t, &self.editor_config),
            _ => false,
        }
    }

    fn apply_choice(
        &mut self,
        src: usize,
        col: usize,
        value: String,
    ) -> Result<GridAction, String> {
        let parsed = if value.is_empty() {
            Ok(CellValue::Text(String::new()))
        } else {
            self.validator
                .validate_in(&self.columns[col], &value)
                .map_err(FieldError::into_message)
        }?;
        self.record_cell(src, col, parsed);
        Ok(GridAction::CellChanged { row: src, col })
    }

    fn next_writable(&self, backward: bool) -> Result<usize, GridAction> {
        let n = self.columns.len();
        let mut col = self.cursor.1;
        loop {
            if backward {
                if col == 0 {
                    return Err(GridAction::LeaveBackward);
                }
                col -= 1;
            } else {
                col += 1;
                if col >= n {
                    return Err(GridAction::LeaveForward);
                }
            }
            if !self.columns[col].read_only && !self.columns[col].primary {
                return Ok(col);
            }
        }
    }

    fn on_choice(&mut self, r: Option<ChoiceResult>) -> Option<GridAction> {
        let (value, tab) = match r? {
            ChoiceResult::Cancel => {
                self.choice = None;
                return None;
            }
            ChoiceResult::Commit(v) => (v, None),
            ChoiceResult::CommitAndTab { value, backward } => (value, Some(backward)),
        };
        let (src, col, generation) = self.choice.as_ref().map(|c| (c.row, c.col, c.generation))?;
        if !self.session_target_live(src, col, generation) {
            self.choice = None;
            return None;
        }
        match self.apply_choice(src, col, value) {
            Ok(ev) => {
                self.choice = None;
                if let Some(backward) = tab {
                    match self.next_writable(backward) {
                        Ok(c) => {
                            self.set_cursor(self.cursor.0, c, false);
                            let _ = self.begin_edit();
                        }
                        Err(leave) => return Some(leave),
                    }
                }
                Some(ev)
            }
            Err(msg) => {
                if let Some(c) = self.choice.as_mut() {
                    c.editor.error = Some(msg);
                }
                None
            }
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

    pub fn horizontal(mut self, policy: HScroll) -> Self {
        self.horizontal = Some(policy);
        self
    }

    fn follow_strings(&mut self, s: crate::theme::Strings) {
        use crate::theme::Strings;
        let old = std::mem::replace(&mut self.strings, s);
        if old == s {
            return;
        }
        Strings::follow(&mut self.empty.title, old.no_rows, s.no_rows);
        let pairs = [
            (old.preview_sql, s.preview_sql),
            (old.discard, s.discard),
            (old.save_changes, s.save_changes),
        ];
        for (b, (from, to)) in self.bar.iter_mut().zip(pairs) {
            Strings::follow(&mut b.label, from, to);
        }
    }

    pub fn overflow_badges(mut self, on: bool) -> Self {
        self.overflow_badges = on;
        self
    }

    pub fn bar_labels(mut self, labels: [&str; 3]) -> Self {
        for (b, l) in self.bar.iter_mut().zip(labels) {
            b.label = l.to_owned();
        }
        self
    }

    pub fn empty(mut self, state: crate::widgets::empty::EmptyState) -> Self {
        self.empty = state;
        self
    }

    pub fn validator(
        mut self,
        f: impl Fn(&ColumnSpec, &str) -> Result<CellValue, String> + 'static,
    ) -> Self {
        self.validator = cell_validator(f);
        self
    }

    pub fn editable(mut self, on: bool) -> Self {
        self.editable = on;
        self
    }

    pub fn click(mut self, policy: ClickPolicy) -> Self {
        self.click = policy;
        self
    }

    pub fn toggle_mods(mut self, mods: KeyModifiers) -> Self {
        self.toggle_mods = mods;
        self
    }

    pub fn duplicate_keys(mut self, keys: &[Key]) -> Self {
        self.keymap.retain(|(_, a)| *a != GridCmd::DuplicateRow);
        self.keymap.extend(
            keys.iter()
                .map(|k| (KeyBinding::new(k.code, k.mods), GridCmd::DuplicateRow)),
        );
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<GridCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn width_sample_rows(mut self, rows: usize) -> Self {
        self.width_sample_rows = rows;
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

    fn col_w(&self, c: usize) -> u16 {
        self.sizing
            .width(c, self.widths.get(c).copied().unwrap_or(0))
    }

    pub fn autofit(&mut self, c: usize) {
        let Some(col) = self.columns.get(c) else {
            return;
        };
        let cap = (self.area.width.saturating_sub(4) as usize).max(8);
        let sorted = self.sort.is_some_and(|(sc, _)| sc == c);
        let mut need = width(&col.name)
            + if col.primary { 2 } else { 0 }
            + if self.filtered_cols.contains(&c) {
                2
            } else {
                0
            }
            + if sorted { 2 } else { 0 };
        for d in self.scroll.visible_range() {
            if let Some(&src) = self.order.get(d) {
                need = need.max(width_capped(&self.value(src, c).text_ref(), cap));
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

    pub fn on_header_action(
        &mut self,
        c: usize,
        action: HeaderAction,
    ) -> (Outcome, Option<GridAction>) {
        match action {
            HeaderAction::SortAsc => self.request_sort(Some((c, SortDir::Asc))),
            HeaderAction::SortDesc => self.request_sort(Some((c, SortDir::Desc))),
            HeaderAction::ClearSort => self.request_sort(None),
            HeaderAction::Autofit => {
                self.autofit(c);
                (Outcome::Changed, None)
            }
            HeaderAction::ResetWidths => {
                self.sizing.reset();
                (Outcome::Changed, None)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn rows(&self) -> &[Vec<CellValue>] {
        &self.rows
    }
    pub fn source_row(&self, display: usize) -> usize {
        self.order.get(display).copied().unwrap_or(display)
    }

    pub fn value(&self, src: usize, col: usize) -> &CellValue {
        self.row_keys
            .get(src)
            .and_then(|&key| self.pending.value(key, col))
            .unwrap_or_else(|| {
                self.rows
                    .get(src)
                    .and_then(|r| r.get(col))
                    .unwrap_or(&CellValue::Null)
            })
    }

    pub fn pending(&self) -> &PendingChanges {
        &self.pending
    }

    pub fn revision(&self) -> GridRevision {
        self.revision
    }

    pub fn row_key(&self, src: usize) -> Option<ItemKey> {
        self.row_keys.get(src).copied()
    }

    pub fn source_index(&self, key: ItemKey) -> Option<usize> {
        self.key_to_source.get(&key).copied()
    }

    fn next_revision(&self, source: bool, draft: bool) -> GridRevision {
        let bump = |v: u64, on: bool| {
            if on {
                v.checked_add(1).expect("DataGrid revision exhausted")
            } else {
                v
            }
        };
        GridRevision {
            source: bump(self.revision.source, source),
            draft: bump(self.revision.draft, draft),
        }
    }

    fn alloc_key(&mut self) -> ItemKey {
        loop {
            let key = ItemKey::num(self.next_local_key);
            self.next_local_key = self
                .next_local_key
                .checked_add(1)
                .expect("DataGrid row key space exhausted");
            if !self.key_to_source.contains_key(&key) {
                return key;
            }
        }
    }

    fn alloc_keys(&mut self, n: usize) -> Vec<ItemKey> {
        (0..n).map(|_| self.alloc_key()).collect()
    }

    fn reindex_keys(&mut self) {
        self.key_to_source = self
            .row_keys
            .iter()
            .enumerate()
            .map(|(src, &key)| (key, src))
            .collect();
    }

    fn draft_open(&self) -> bool {
        !self.pending.is_empty() || !self.undo.is_empty()
    }

    pub fn set_rows(&mut self, data: GridRows) {
        let keys = self.alloc_keys(data.rows.len());
        self.install_rows(data, keys);
    }

    pub fn try_set_rows_keyed(
        &mut self,
        data: GridRows,
        keys: Vec<ItemKey>,
    ) -> Result<(), GridKeyError> {
        validate_row_keys(data.rows.len(), &keys)?;
        self.install_rows(data, keys);
        Ok(())
    }

    pub fn try_set_model_keyed(
        &mut self,
        columns: Vec<ColumnSpec>,
        data: GridRows,
        keys: Vec<ItemKey>,
    ) -> Result<(), GridKeyError> {
        validate_row_keys(data.rows.len(), &keys)?;
        self.sizing.reset();
        self.sort = self.sort.filter(|&(c, _)| c < columns.len());
        self.filtered_cols.retain(|&c| c < columns.len());
        self.cursor.1 = self.cursor.1.min(columns.len().saturating_sub(1));
        self.columns = columns;
        self.install_rows(data, keys);
        Ok(())
    }

    fn install_rows(&mut self, data: GridRows, keys: Vec<ItemKey>) {
        let revision = self.next_revision(true, self.draft_open());
        self.rows = data.rows;
        self.row_keys = keys;
        self.reindex_keys();
        self.revision = revision;
        self.total = data.total;
        self.more = data.more;
        self.order = (0..self.rows.len()).collect();
        self.pending = PendingChanges::default();
        self.undo.clear();
        self.cell_errors.clear();
        self.row_errors.clear();
        self.selection.clear();
        self.anchor = None;
        self.structural_change();
        self.cursor.0 = self.cursor.0.min(self.rows.len().saturating_sub(1));
        self.sample_widths();
        if self.local_sort
            && let Some((c, d)) = self.sort
        {
            self.apply_local_sort(c, d);
        }
        self.scroll.set_content(self.content_rows());
        self.scroll.ensure_visible(self.cursor.0);
    }

    pub fn append_rows(&mut self, data: GridRows) {
        let start = self.rows.len();
        let revision = self.next_revision(true, false);
        let keys = self.alloc_keys(data.rows.len());
        self.rows.extend(data.rows);
        for (offset, key) in keys.into_iter().enumerate() {
            self.row_keys.push(key);
            self.key_to_source.insert(key, start + offset);
        }
        self.revision = revision;
        self.order.extend(start..self.rows.len());
        self.total = data.total;
        self.more = data.more;
        self.loading = false;
        self.scroll.set_content(self.content_rows());
    }

    pub fn set_loading(&mut self, on: bool) {
        self.loading = on;
    }

    fn content_rows(&self) -> usize {
        self.rows.len() + usize::from(self.more)
    }

    fn sample_widths(&mut self) {
        self.widths = self
            .columns
            .iter()
            .enumerate()
            .map(|(ci, c)| {
                let header = width(&c.name) as u16 + if c.primary { 2 } else { 0 } + 2;

                let max = c.max_width.max(header.min(24));

                let mut ws: Vec<usize> = self
                    .rows
                    .iter()
                    .take(self.width_sample_rows)
                    .map(|r| {
                        r.get(ci)
                            .map_or(0, |v| width_capped(&v.text_ref(), max as usize))
                    })
                    .collect();
                ws.sort_unstable();
                let p95 = ws.get(ws.len() * 95 / 100).copied().unwrap_or(0) as u16;
                p95.max(header).clamp(c.min_width.min(max), max)
            })
            .collect();
    }

    fn apply_local_sort(&mut self, col: usize, dir: SortDir) {
        let (rows, keys, pending) = (&self.rows, &self.row_keys, &self.pending);
        self.order.sort_by(|&a, &b| {
            let (ia, ib) = (pending.is_inserted(keys[a]), pending.is_inserted(keys[b]));
            if ia || ib {
                return ia.cmp(&ib).then_with(|| a.cmp(&b));
            }
            let va = &rows[a][col];
            let vb = &rows[b][col];
            let o = cmp_cells(va, vb).then_with(|| a.cmp(&b));
            if dir == SortDir::Asc { o } else { o.reverse() }
        });
    }

    fn rebuild_order(&mut self) {
        self.order = (0..self.rows.len()).collect();
        if self.local_sort
            && let Some((c, d)) = self.sort
        {
            self.apply_local_sort(c, d);
        }
    }

    pub fn row_state(&self, src: usize) -> RowState {
        self.row_state_with(src, |key| self.pending.cells.keys().any(|(k, _)| *k == key))
    }

    fn row_state_with(&self, src: usize, edited: impl Fn(ItemKey) -> bool) -> RowState {
        let key = self.row_key(src);
        if self.row_errors.contains_key(&src) {
            RowState::Error
        } else if key.is_some_and(|k| self.pending.is_deleted(k)) {
            RowState::Deleted
        } else if key.is_some_and(|k| self.pending.is_inserted(k)) {
            RowState::Inserted
        } else if key.is_some_and(edited) {
            RowState::Modified
        } else {
            RowState::Clean
        }
    }

    fn src_deleted(&self, src: usize) -> bool {
        self.row_key(src)
            .is_some_and(|key| self.pending.is_deleted(key))
    }

    pub fn is_editing(&self) -> bool {
        if self.choice.is_some() {
            return true;
        }
        self.edit.is_some()
    }

    fn edit_offset(&self, pos: Position) -> Option<usize> {
        let e = self.edit.as_ref()?;
        let rect = self
            .col_rects
            .get(e.col.checked_sub(self.scroll.h.offset)?)?;
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

    pub fn edit_error(&self) -> Option<&str> {
        if let Some(c) = &self.choice {
            return c.editor.error.as_deref();
        }
        self.edit.as_ref().and_then(|e| e.field.error_message())
    }

    fn cursor_src(&self) -> Option<usize> {
        self.order.get(self.cursor.0).copied()
    }

    pub fn read_only_reason_at_cursor(&self) -> Option<String> {
        let src = self.cursor_src()?;
        if !self.editable {
            return self.read_only_reason.clone();
        }
        let spec = self.columns.get(self.cursor.1)?;
        if self.src_deleted(src) {
            Some(self.strings.grid_row_deleted.to_owned())
        } else if spec.read_only {
            Some(crate::theme::Strings::fill(
                self.strings.grid_read_only,
                &[&spec.name],
            ))
        } else {
            None
        }
    }

    pub fn cell_editable(&self) -> bool {
        self.editable
            && !self.on_more_row()
            && self.cursor_src().is_some()
            && self.read_only_reason_at_cursor().is_none()
    }

    pub fn begin_edit(&mut self) -> (Outcome, Option<GridAction>) {
        let Some(src) = self.cursor_src() else {
            return (Outcome::Consumed, None);
        };
        if !self.editable {
            return (Outcome::Consumed, None);
        }
        let col = self.cursor.1;
        if let Some(reason) = self.read_only_reason_at_cursor() {
            return (Outcome::Changed, Some(GridAction::ReadOnly { reason }));
        }

        self.scroll.ensure_visible(self.cursor.0);
        self.ensure_col_visible();
        let spec = &self.columns[col];
        let v = self.value(src, col).clone();

        if spec.editor.is_constrained() {
            let raw = v.edit_text();
            if let Some(next) = spec.editor.toggled(&raw) {
                return match self.apply_choice(src, col, next) {
                    Ok(ev) => (Outcome::Changed, Some(ev)),
                    Err(_) => (Outcome::Consumed, None),
                };
            }
            let opened =
                ChoiceEditor::open(self.choice_id(), &spec.editor, &raw, &self.editor_config);
            self.choice = opened.map(|editor| GridChoice {
                row: src,
                col,
                editor,
                generation: self.structural_generation,
            });
            return (Outcome::Changed, None);
        }
        match spec.kind {
            CellKind::Bool => {
                let next = match v {
                    CellValue::Bool(true) => CellValue::Bool(false),
                    CellValue::Bool(false) if spec.nullable => CellValue::Null,
                    _ => CellValue::Bool(true),
                };
                self.record_cell(src, col, next);
                return (
                    Outcome::Changed,
                    Some(GridAction::CellChanged { row: src, col }),
                );
            }
            CellKind::Json => {
                return (
                    Outcome::Changed,
                    Some(GridAction::OpenViewer { row: src, col }),
                );
            }
            CellKind::Text
                if width(&v.text()) > 2 * self.widths.get(col).copied().unwrap_or(20) as usize =>
            {
                return (
                    Outcome::Changed,
                    Some(GridAction::OpenViewer { row: src, col }),
                );
            }
            _ => {}
        }
        let mut field = EditField::single(v.edit_text());

        let (parser, spec) = (self.validator.clone(), self.columns[col].clone());
        field.validator = Some(Validator::new(move |text| {
            parse_cell(&parser, &spec, text).map(|_| ())
        }));
        self.edit = Some(EditState {
            row: src,
            col,
            field,
            generation: self.structural_generation,
        });
        (Outcome::Changed, None)
    }

    pub fn commit_edit(&mut self) -> Option<GridAction> {
        let mut e = self.edit.take()?;
        if !self.session_target_live(e.row, e.col, e.generation) {
            return None;
        }
        let parsed = parse_cell(&self.validator, &self.columns[e.col], e.field.text());
        match parsed {
            Ok(v) => {
                self.record_cell(e.row, e.col, v);
                Some(GridAction::CellChanged {
                    row: e.row,
                    col: e.col,
                })
            }
            Err(err) => {
                e.field.set_error(Some(err));
                self.edit = Some(e);
                None
            }
        }
    }

    pub fn cancel_edit(&mut self) -> bool {
        self.cancel_sessions()
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

    fn session_target_live(&self, row: usize, col: usize, generation: u64) -> bool {
        generation == self.structural_generation
            && col < self.columns.len()
            && self.rows.get(row).is_some_and(|r| col < r.len())
    }

    pub fn on_blur(&mut self) -> (Outcome, Option<GridAction>) {
        if let Some(c) = &self.choice {
            let r = match self.editor_config.blur {
                BlurAction::Commit => match c.editor.blur_value(&self.editor_config) {
                    Some(v) => ChoiceResult::Commit(v),
                    None => ChoiceResult::Cancel,
                },
                BlurAction::Cancel => ChoiceResult::Cancel,
                BlurAction::Keep => return (Outcome::Ignored, None),
            };
            return (Outcome::Changed, self.on_choice(Some(r)));
        }
        if self.edit.is_none() {
            return (Outcome::Ignored, None);
        }
        match self.blur_action {
            BlurAction::Commit => (Outcome::Changed, self.commit_edit()),
            BlurAction::Cancel => {
                self.cancel_edit();
                (Outcome::Changed, None)
            }
            BlurAction::Keep => (Outcome::Ignored, None),
        }
    }

    pub fn record_cell(&mut self, src: usize, col: usize, value: CellValue) -> bool {
        let Some(&key) = self.row_keys.get(src) else {
            return false;
        };
        if col >= self.columns.len() {
            return false;
        }
        self.cell_errors.remove(&(src, col));
        if value == *self.value(src, col) {
            return false;
        }
        let revision = self.next_revision(false, true);
        let before = self.pending.cells.get(&(key, col)).cloned();
        let stored = self.rows[src].get(col).unwrap_or(&CellValue::Null);
        if value == *stored && !self.pending.is_inserted(key) {
            self.pending.cells.remove(&(key, col));
        } else {
            self.pending.cells.insert((key, col), value);
        }
        self.undo.push(UndoAction::Cell { key, col, before });
        self.revision = revision;
        true
    }

    pub fn toggle_delete(&mut self, src: usize) -> Option<GridAction> {
        if !self.editable {
            return None;
        }
        let key = self.row_key(src)?;
        if self.pending.is_inserted(key) {
            self.remove_inserted_recorded(src);
            return Some(GridAction::RowDeleted(src));
        }
        let revision = self.next_revision(false, true);
        self.cancel_sessions();
        let was_deleted = self.pending.deleted.remove(&key);
        if !was_deleted {
            self.pending.deleted.insert(key);
        }
        self.undo.push(UndoAction::Delete { key, was_deleted });
        self.revision = revision;
        Some(GridAction::RowDeleted(src))
    }

    fn remove_inserted_recorded(&mut self, src: usize) {
        let revision = self.next_revision(false, true);
        let row = self.capture_removed(src);
        self.remove_rows(&[src]);
        self.undo
            .push(UndoAction::RemoveInserted { row: Box::new(row) });
        self.revision = revision;
    }

    fn view_key(&self, display: usize, col: usize) -> Option<(ItemKey, usize)> {
        let &src = self.order.get(display)?;
        Some((self.row_keys[src], col))
    }

    fn capture_removed(&self, src: usize) -> RemovedDraftRow {
        let key = self.row_keys[src];
        let mut cells: Vec<(usize, CellValue)> = self
            .pending
            .cells
            .iter()
            .filter(|((k, _), _)| *k == key)
            .map(|((_, col), v)| (*col, v.clone()))
            .collect();
        cells.sort_by_key(|(col, _)| *col);
        RemovedDraftRow {
            key,
            position: src,
            base: self.rows[src].clone(),
            cells,
            cell_errors: self
                .cell_errors
                .iter()
                .filter(|((r, _), _)| *r == src)
                .map(|((_, col), msg)| (*col, msg.clone()))
                .collect(),
            row_error: self.row_errors.get(&src).cloned(),
            view: StructuralViewSnapshot {
                source_keys: self.row_keys.clone(),
                selection: self.selection.clone(),
                cursor: self.view_key(self.cursor.0, self.cursor.1),
                anchor: self.anchor.and_then(|(d, c)| self.view_key(d, c)),
                scroll_offset: self.scroll.offset,
            },
        }
    }

    fn display_of(&self, key: ItemKey) -> Option<usize> {
        let src = self.source_index(key)?;
        self.order.iter().position(|&s| s == src)
    }

    fn remove_rows(&mut self, removed: &[usize]) {
        if removed.is_empty() {
            return;
        }
        self.structural_change();
        let cursor_key = self.view_key(self.cursor.0, self.cursor.1);
        let anchor_key = self.anchor.and_then(|(d, c)| self.view_key(d, c));
        let map = |i: usize| match removed.binary_search(&i) {
            Ok(_) => None,
            Err(shift) => Some(i - shift),
        };
        let gone: std::collections::HashSet<ItemKey> =
            removed.iter().map(|&src| self.row_keys[src]).collect();
        self.pending.cells.retain(|(key, _), _| !gone.contains(key));
        self.pending.inserted.retain(|key| !gone.contains(key));
        self.pending.deleted.retain(|key| !gone.contains(key));
        let mut i = 0;
        self.rows.retain(|_| {
            i += 1;
            removed.binary_search(&(i - 1)).is_err()
        });
        self.row_keys.retain(|key| !gone.contains(key));
        self.reindex_keys();
        self.cell_errors = std::mem::take(&mut self.cell_errors)
            .into_iter()
            .filter_map(|((r, c), msg)| map(r).map(|r| ((r, c), msg)))
            .collect();
        self.row_errors = std::mem::take(&mut self.row_errors)
            .into_iter()
            .filter_map(|(r, msg)| map(r).map(|r| (r, msg)))
            .collect();
        self.selection.remap(map);
        self.order = self.order.iter().filter_map(|&src| map(src)).collect();
        self.scroll.set_content(self.content_rows());
        self.cursor.0 = cursor_key
            .and_then(|(key, _)| self.display_of(key))
            .unwrap_or_else(|| self.cursor.0.min(self.content_rows().saturating_sub(1)));
        self.anchor = anchor_key.and_then(|(key, c)| Some((self.display_of(key)?, c)));
    }

    fn restore_removed(&mut self, row: RemovedDraftRow) {
        self.structural_change();
        let RemovedDraftRow {
            key,
            position,
            base,
            cells,
            cell_errors,
            row_error,
            view,
        } = row;
        let saved: std::collections::HashSet<ItemKey> = view.source_keys.iter().copied().collect();
        let appended: Vec<ItemKey> = self
            .selection
            .iter()
            .filter_map(|src| self.row_key(src))
            .filter(|key| !saved.contains(key))
            .collect();
        let at = position.min(self.rows.len());
        let shift = |i: usize| if i >= at { i + 1 } else { i };
        self.cell_errors = std::mem::take(&mut self.cell_errors)
            .into_iter()
            .map(|((r, c), msg)| ((shift(r), c), msg))
            .collect();
        self.row_errors = std::mem::take(&mut self.row_errors)
            .into_iter()
            .map(|(r, msg)| (shift(r), msg))
            .collect();
        self.rows.insert(at, base);
        self.row_keys.insert(at, key);
        self.reindex_keys();
        self.pending.inserted.insert(key);
        for (col, value) in cells {
            self.pending.cells.insert((key, col), value);
        }
        for (col, msg) in cell_errors {
            self.cell_errors.insert((at, col), msg);
        }
        if let Some(msg) = row_error {
            self.row_errors.insert(at, msg);
        }
        self.rebuild_order();

        let mut selection = view.selection;
        selection.remap(|old| {
            view.source_keys
                .get(old)
                .and_then(|k| self.source_index(*k))
        });
        for key in appended {
            if let Some(src) = self.source_index(key) {
                selection.set(src, true);
            }
        }
        self.selection = selection;
        self.scroll.set_content(self.content_rows());
        if let Some((key, col)) = view.cursor
            && let Some(display) = self.display_of(key)
        {
            self.cursor = (display, col);
        }
        self.anchor = view
            .anchor
            .and_then(|(key, col)| Some((self.display_of(key)?, col)));
        self.scroll.offset = view.scroll_offset;
        self.scroll.ensure_visible(self.cursor.0);
    }

    pub fn insert_row(&mut self) -> Option<GridAction> {
        if !self.editable {
            return None;
        }
        let row: Vec<CellValue> = self
            .columns
            .iter()
            .map(|c| {
                if c.primary || c.read_only {
                    CellValue::Default
                } else {
                    CellValue::Null
                }
            })
            .collect();
        let src = self.push_inserted(row);
        let disp = self.order.iter().position(|&r| r == src).unwrap_or(0);
        let col = self
            .columns
            .iter()
            .position(|c| !c.primary && !c.read_only)
            .unwrap_or(0);
        self.set_cursor(disp, col, false);
        Some(GridAction::RowInserted(src))
    }

    fn push_inserted(&mut self, row: Vec<CellValue>) -> usize {
        let revision = self.next_revision(false, true);
        let key = self.alloc_key();
        self.cancel_sessions();
        let src = self.rows.len();
        self.rows.push(row);
        self.row_keys.push(key);
        self.key_to_source.insert(key, src);
        self.order.push(src);
        self.pending.inserted.insert(key);
        self.undo.push(UndoAction::Insert { key });
        self.revision = revision;
        self.scroll.set_content(self.content_rows());
        src
    }

    pub fn duplicate_row(&mut self) -> Option<GridAction> {
        let src = self.cursor_src()?;
        if !self.editable {
            return None;
        }
        let mut row: Vec<CellValue> = (0..self.columns.len())
            .map(|c| self.value(src, c).clone())
            .collect();
        for (ci, c) in self.columns.iter().enumerate() {
            if c.primary || c.read_only {
                row[ci] = CellValue::Default;
            }
        }
        let new = self.push_inserted(row);
        let disp = self.order.len() - 1;
        self.set_cursor(disp, self.cursor.1, false);
        Some(GridAction::RowInserted(new))
    }

    pub fn undo(&mut self) -> Option<GridUndo> {
        if self.undo.is_empty() {
            return None;
        }
        let revision = self.next_revision(false, true);
        let action = self.undo.pop()?;
        self.cancel_sessions();
        let undone = match action {
            UndoAction::Cell { key, col, before } => {
                let src = self.source_index(key);
                debug_assert!(src.is_some(), "an undo target key resolves");
                src?;
                match before {
                    Some(v) => {
                        self.pending.cells.insert((key, col), v);
                    }
                    None => {
                        self.pending.cells.remove(&(key, col));
                    }
                }
                GridUndo {
                    key,
                    kind: GridUndoKind::Cell,
                }
            }
            UndoAction::Delete { key, was_deleted } => {
                let src = self.source_index(key);
                debug_assert!(src.is_some(), "an undo target key resolves");
                src?;
                if was_deleted {
                    self.pending.deleted.insert(key);
                } else {
                    self.pending.deleted.remove(&key);
                }
                GridUndo {
                    key,
                    kind: GridUndoKind::DeleteMark,
                }
            }
            UndoAction::Insert { key } => {
                let src = self.source_index(key);
                debug_assert!(src.is_some(), "an undo target key resolves");
                self.remove_rows(&[src?]);
                GridUndo {
                    key,
                    kind: GridUndoKind::Insert,
                }
            }
            UndoAction::RemoveInserted { row } => {
                let key = row.key;
                self.restore_removed(*row);
                GridUndo {
                    key,
                    kind: GridUndoKind::RemovedInsert,
                }
            }
        };
        self.revision = revision;
        Some(undone)
    }

    pub fn apply_commit_result(&mut self, result: Result<(), (usize, String)>) {
        match result {
            Ok(()) => {
                let revision = self.next_revision(true, self.draft_open());
                self.structural_change();
                let cursor_key = self.view_key(self.cursor.0, self.cursor.1);
                let pending = std::mem::take(&mut self.pending);
                for ((key, col), value) in pending.cells {
                    if pending.deleted.contains(&key) {
                        continue;
                    }
                    if let Some(&src) = self.key_to_source.get(&key)
                        && let Some(cell) = self.rows.get_mut(src).and_then(|r| r.get_mut(col))
                    {
                        *cell = value;
                    }
                }
                let mut deleted: Vec<usize> = pending
                    .deleted
                    .iter()
                    .filter_map(|&key| self.source_index(key))
                    .collect();
                deleted.sort_unstable();
                self.remove_rows(&deleted);
                self.undo.clear();
                self.row_errors.clear();
                self.cell_errors.clear();
                self.rebuild_order();
                self.revision = revision;
                self.scroll.set_content(self.content_rows());
                self.cursor.0 = match cursor_key.and_then(|(key, _)| self.display_of(key)) {
                    Some(display) => display,
                    None => self.cursor.0.min(self.rows.len().saturating_sub(1)),
                };
            }
            Err((row, msg)) => {
                if row < self.rows.len() {
                    self.row_errors.insert(row, msg);
                }
            }
        }
    }

    pub fn discard(&mut self) {
        let revision = self.next_revision(false, self.draft_open());
        let mut inserted: Vec<usize> = self
            .pending
            .inserted
            .iter()
            .filter_map(|&key| self.source_index(key))
            .collect();
        inserted.sort_unstable();
        self.structural_change();
        self.remove_rows(&inserted);
        self.pending = PendingChanges::default();
        self.undo.clear();
        self.cell_errors.clear();
        self.row_errors.clear();
        self.revision = revision;
    }

    fn set_cursor(&mut self, row: usize, col: usize, extend: bool) {
        let n = self.content_rows();
        let row = row.min(n.saturating_sub(1));
        let col = col.min(self.columns.len().saturating_sub(1));
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = (row, col);
        self.scroll.ensure_visible(row);
        self.ensure_col_visible();
    }

    fn point_cursor(&mut self, row: usize, col: usize, extend: bool) {
        self.scroll.pointer_move();
        self.set_cursor(row, col, extend);
    }

    fn ensure_col_visible(&mut self) {
        self.scroll.reveal_column(self.cursor.1);
    }

    fn in_range(&self, row: usize, col: usize) -> bool {
        let Some(a) = self.anchor else {
            return false;
        };
        let (r0, r1) = (a.0.min(self.cursor.0), a.0.max(self.cursor.0));
        let (c0, c1) = (a.1.min(self.cursor.1), a.1.max(self.cursor.1));
        row >= r0 && row <= r1 && col >= c0 && col <= c1
    }

    fn on_more_row(&self) -> bool {
        self.more && self.cursor.0 == self.rows.len()
    }

    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() || !self.selection.is_empty()
    }

    fn copy_text(&self, with_header: bool) -> String {
        let mut out = String::new();
        if let Some(a) = self.anchor {
            let (r0, r1) = (a.0.min(self.cursor.0), a.0.max(self.cursor.0));
            let (c0, c1) = (a.1.min(self.cursor.1), a.1.max(self.cursor.1));
            if with_header {
                out.push_str(
                    &self.columns[c0..=c1]
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
                out.push('\n');
            }
            for r in r0..=r1 {
                if let Some(&src) = self.order.get(r) {
                    let line: Vec<String> = (c0..=c1).map(|c| self.value(src, c).text()).collect();
                    out.push_str(&line.join("\t"));
                    out.push('\n');
                }
            }
        } else if !self.selection.is_empty() || with_header {
            let rows: Vec<usize> = if self.selection.is_empty() {
                self.cursor_src().into_iter().collect()
            } else {
                self.selection.to_vec()
            };
            if with_header {
                out.push_str(
                    &self
                        .columns
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
                out.push('\n');
            }
            for src in rows {
                let line: Vec<String> = (0..self.columns.len())
                    .map(|c| self.value(src, c).text())
                    .collect();
                out.push_str(&line.join("\t"));
                out.push('\n');
            }
        } else if let Some(src) = self.cursor_src() {
            out = self.value(src, self.cursor.1).text();
        }
        out
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<GridAction>) {
        if let Some(c) = self.choice.as_mut() {
            let (out, r) = c.editor.on_key(key, &self.editor_config);
            return (out, self.on_choice(r));
        }

        if key.code == KeyCode::Down
            && key.alt()
            && self.editable
            && self
                .columns
                .get(self.cursor.1)
                .is_some_and(|c| c.editor.has_popup())
        {
            return self.begin_edit();
        }
        if let Some(e) = self.edit.as_mut() {
            let r = e.field.on_key(key);
            return match r.exit {
                Some(EditExit::Commit) => match self.commit_edit() {
                    Some(ev) => (Outcome::Changed, Some(ev)),
                    None => (Outcome::Changed, None),
                },
                Some(EditExit::Cancel) => {
                    self.cancel_edit();
                    (Outcome::Changed, None)
                }
                Some(EditExit::Tab { backward }) => {
                    let ev = self.commit_edit();
                    if self.edit.is_some() {
                        return (Outcome::Changed, None);
                    }

                    let n = self.columns.len();
                    let mut col = self.cursor.1;
                    loop {
                        if backward {
                            if col == 0 {
                                return (Outcome::Changed, Some(GridAction::LeaveBackward));
                            }
                            col -= 1;
                        } else {
                            col += 1;
                            if col >= n {
                                return (Outcome::Changed, Some(GridAction::LeaveForward));
                            }
                        }
                        if !self.columns[col].read_only && !self.columns[col].primary {
                            break;
                        }
                    }
                    self.set_cursor(self.cursor.0, col, false);
                    let _ = self.begin_edit();
                    (Outcome::Changed, ev)
                }
                None => (r.outcome, None),
            };
        }

        let (r, c) = self.cursor;
        let shift = key.shift();
        let ctrl = key.ctrl();
        let n_rows = self.content_rows();
        if n_rows == 0 {
            return match lookup(&self.keymap, key) {
                Some(GridCmd::InsertRow) if self.editable => (Outcome::Changed, self.insert_row()),
                Some(GridCmd::Refresh) => (Outcome::Changed, Some(GridAction::Refresh)),
                Some(GridCmd::FilterOnCell) => (Outcome::Changed, Some(GridAction::OpenFilters)),
                Some(GridCmd::ClearFilters) => (Outcome::Changed, Some(GridAction::ClearFilters)),
                _ => (Outcome::Ignored, None),
            };
        }
        if let Some(KeyStep::Type(ch)) = KeyStep::ordered(
            key,
            &self.nav,
            self.type_ahead,
            self.type_ahead_precedence
                .unwrap_or_else(crate::core::list::TypeAheadPrecedence::current),
        ) {
            let col = self.type_ahead_column.unwrap_or(c);
            let texts: Vec<String> = self
                .order
                .iter()
                .map(|&src| {
                    self.rows[src]
                        .get(col)
                        .map(CellValue::text)
                        .unwrap_or_default()
                })
                .collect();
            let moved = Cursor::new(r).type_ahead(ch, &mut self.typeahead, &texts[..]);
            if let Moved::To(d) = moved {
                self.set_cursor(d, c, false);
            }
            return (moved.outcome(), None);
        }

        let copy = (key.ctrl_char('c') && self.has_selection()).then_some(GridCmd::Copy);
        if let Some(action) = lookup(&self.keymap, key).or(copy) {
            return self.on_action(action);
        }
        if let Some(nav) = self.nav.action(key) {
            let letter = matches!(key.code, KeyCode::Char(_));
            let rows = RowFn {
                len: n_rows,
                enabled: &|_| true,
                label: &|_| "",
            };
            let page = self.scroll.viewport_len;
            let row = |nav| Cursor::new(r).apply(nav, &rows, page).index().unwrap_or(r);
            match nav {
                Nav::Up | Nav::Down | Nav::PageUp | Nav::PageDown => {
                    self.set_cursor(row(nav), c, shift)
                }
                Nav::Left if ctrl => {
                    self.scroll
                        .h
                        .scroll_by(-(self.scroll.h.viewport_len.max(1) as isize));
                }
                Nav::Right if ctrl => {
                    self.scroll
                        .h
                        .scroll_by(self.scroll.h.viewport_len.max(1) as isize);
                }
                Nav::Left => self.set_cursor(r, c.saturating_sub(1), shift),
                Nav::Right => self.set_cursor(r, c + 1, shift),
                Nav::Home | Nav::End if letter => self.set_cursor(row(nav), c, false),
                Nav::Home | Nav::End if ctrl => self.set_cursor(row(nav), c, shift),
                Nav::Home => self.set_cursor(r, 0, shift),
                Nav::End => self.set_cursor(r, usize::MAX, shift),
            }
            return (Outcome::Changed, None);
        }
        match key.code {
            KeyCode::Enter | KeyCode::F(2) => {
                if self.on_more_row() {
                    return (Outcome::Changed, Some(GridAction::FetchMore));
                }
                if !self.editable {
                    if let Some(reason) = self.read_only_reason_at_cursor() {
                        return (Outcome::Changed, Some(GridAction::ReadOnly { reason }));
                    }
                    return (
                        Outcome::Changed,
                        self.cursor_src().map(GridAction::Activated),
                    );
                }
                self.begin_edit()
            }
            KeyCode::Char(' ') => {
                if self.on_more_row() {
                    return (Outcome::Changed, Some(GridAction::FetchMore));
                }
                self.select_rows(SelectGesture::Toggle, self.cursor.0);
                (Outcome::Changed, None)
            }
            KeyCode::Esc => {
                if self.anchor.is_some() || !self.selection.is_empty() {
                    self.anchor = None;
                    self.selection.clear();
                    (Outcome::Changed, None)
                } else {
                    (Outcome::Ignored, None)
                }
            }
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn context_items(&self) -> Vec<(GridCmd, crate::widgets::menu::MenuItem)> {
        use crate::ui::context::{ContextCmd, command_items};
        let row = self.cursor_src();
        let spec = self.columns.get(self.cursor.1);
        let read_only = (!self.editable).then_some("Read-only rows");
        let no_row = row.is_none().then_some("No row here");
        let clear = read_only.or(no_row).or(match spec {
            Some(s) if s.read_only => Some("Read-only column"),
            Some(s) if !s.nullable && self.selection.is_empty() => Some("NOT NULL column"),
            _ => None,
        });
        let cmds = [
            ContextCmd::new(GridCmd::Copy).unless(no_row),
            ContextCmd::new(GridCmd::CopyWithHeader)
                .unless(no_row)
                .group_end(),
            ContextCmd::new(GridCmd::InsertRow).unless(read_only),
            ContextCmd::new(GridCmd::DuplicateRow).unless(read_only.or(no_row)),
            ContextCmd::new(GridCmd::ToggleDelete)
                .unless(read_only.or(no_row))
                .danger(),
            ContextCmd::new(GridCmd::ClearCell)
                .unless(clear)
                .group_end(),
            ContextCmd::new(GridCmd::Undo)
                .unless(self.undo.is_empty().then_some("Nothing to undo"))
                .group_end(),
            ContextCmd::new(GridCmd::FilterOnCell).unless(no_row),
            ContextCmd::new(GridCmd::ClearFilters),
        ];
        command_items(&self.keymap, &cmds)
    }

    pub fn run(&mut self, cmd: GridCmd) -> (Outcome, Option<GridAction>) {
        if cmd == GridCmd::InsertRow && !self.editable {
            return (Outcome::Consumed, None);
        }
        self.on_action(cmd)
    }

    fn on_action(&mut self, action: GridCmd) -> (Outcome, Option<GridAction>) {
        let c = self.cursor.1;
        match action {
            GridCmd::ClearCell => {
                if !self.editable {
                    return (Outcome::Consumed, None);
                }
                if !self.selection.is_empty() {
                    let keys: Vec<ItemKey> = self
                        .selection
                        .iter()
                        .filter_map(|src| self.row_key(src))
                        .collect();
                    let mut last = None;
                    for key in keys {
                        if let Some(src) = self.source_index(key) {
                            last = self.toggle_delete(src);
                        }
                    }
                    return (Outcome::Changed, last);
                }
                let Some(src) = self.cursor_src() else {
                    return (Outcome::Consumed, None);
                };
                let spec = &self.columns[c];
                if spec.read_only {
                    return (Outcome::Consumed, None);
                }
                if spec.nullable {
                    self.record_cell(src, c, CellValue::Null);
                    (
                        Outcome::Changed,
                        Some(GridAction::CellChanged { row: src, col: c }),
                    )
                } else {
                    self.cell_errors
                        .insert((src, c), format!("{} is NOT NULL", spec.name));
                    (Outcome::Changed, None)
                }
            }
            GridCmd::ToggleDelete => {
                let Some(src) = self.cursor_src() else {
                    return (Outcome::Consumed, None);
                };
                (Outcome::Changed, self.toggle_delete(src))
            }
            GridCmd::InsertRow => (Outcome::Changed, self.insert_row()),
            GridCmd::DuplicateRow => (Outcome::Changed, self.duplicate_row()),
            GridCmd::Copy => (
                Outcome::Changed,
                Some(GridAction::Copy(self.copy_text(false))),
            ),
            GridCmd::CopyWithHeader => (
                Outcome::Changed,
                Some(GridAction::Copy(self.copy_text(true))),
            ),
            GridCmd::Commit => (Outcome::Changed, Some(GridAction::CommitRequested)),
            GridCmd::Sort => {
                if !self.columns[c].sortable {
                    return (Outcome::Consumed, None);
                }
                let next = match self.sort {
                    Some((sc, SortDir::Asc)) if sc == c => Some((c, SortDir::Desc)),
                    Some((sc, SortDir::Desc)) if sc == c => None,
                    _ => Some((c, SortDir::Asc)),
                };
                self.request_sort(next)
            }
            GridCmd::ClearSort => self.request_sort(None),
            GridCmd::FilterOnCell => {
                let value = self
                    .cursor_src()
                    .map(|src| self.value(src, c).clone())
                    .unwrap_or(CellValue::Null);
                (
                    Outcome::Changed,
                    Some(GridAction::FilterOnCell { col: c, value }),
                )
            }
            GridCmd::OpenFilters => (Outcome::Changed, Some(GridAction::OpenFilters)),
            GridCmd::ClearFilters => (Outcome::Changed, Some(GridAction::ClearFilters)),
            GridCmd::Refresh => (Outcome::Changed, Some(GridAction::Refresh)),
            GridCmd::Undo => match self.undo() {
                Some(undone) => (Outcome::Changed, Some(GridAction::Undone(undone))),
                None => (Outcome::Consumed, None),
            },
            GridCmd::Discard => (Outcome::Changed, Some(GridAction::DiscardRequested)),
            GridCmd::SelectAll => {
                if self.select_rows(SelectGesture::All, 0) {
                    (Outcome::Changed, None)
                } else {
                    (Outcome::Consumed, None)
                }
            }
            GridCmd::PreviewSql => (Outcome::Changed, Some(GridAction::PreviewSql)),
            GridCmd::FollowReference => {
                if self.columns[c].references.is_some()
                    && let Some(src) = self.cursor_src()
                {
                    return (
                        Outcome::Changed,
                        Some(GridAction::FollowReference { row: src, col: c }),
                    );
                }
                (Outcome::Consumed, None)
            }
        }
    }

    fn request_sort(&mut self, next: Option<(usize, SortDir)>) -> (Outcome, Option<GridAction>) {
        self.sort = next;
        if self.local_sort {
            match next {
                Some((c, d)) => self.apply_local_sort(c, d),
                None => self.order = (0..self.rows.len()).collect(),
            }
            (Outcome::Changed, None)
        } else {
            (Outcome::Changed, Some(GridAction::SortRequested(next)))
        }
    }

    pub fn header_id(&self, c: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::HEADER, ItemKey::index(c)))
    }

    pub fn cell_id(&self, display: usize, c: usize) -> WidgetId {
        self.id
            .part(PartRef::item(Part::ROW, ItemKey::index(display)))
            .child(c)
    }
    fn cell_part(display: usize) -> PartRef {
        PartRef::item(Part::CELL, ItemKey::index(display))
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
    pub fn rownum_id(&self, display: usize) -> WidgetId {
        self.id.part(Self::rownum_part(display))
    }
    fn rownum_part(display: usize) -> PartRef {
        PartRef::item(Part::ROW_NUMBER, ItemKey::index(display))
    }

    pub fn more_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::ROW))
    }
    pub fn left_id(&self) -> WidgetId {
        self.id.part(PartRef::OVERFLOW_BEFORE)
    }
    pub fn right_id(&self) -> WidgetId {
        self.id.part(PartRef::OVERFLOW_AFTER)
    }

    fn cell_of(&self, id: WidgetId) -> Option<(usize, usize)> {
        self.parts.cell(id, Part::CELL)
    }

    fn header_of(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::HEADER, 0..self.columns.len())
    }

    fn rownum_of(&self, id: WidgetId) -> Option<usize> {
        self.parts.index(id, Part::ROW_NUMBER)
    }

    pub fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        if let Some((d, c)) = self.cell_of(id)
            && self
                .edit
                .as_ref()
                .is_some_and(|e| self.order.get(d) == Some(&e.row) && e.col == c)
        {
            if let Some(at) = self.edit_offset(pos)
                && let Some(e) = self.edit.as_mut()
            {
                e.field.press(at, count, false);
            }
            return Outcome::Changed;
        }
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
        let w = self.col_w(c);
        self.sizing.press(c, w, pos.x);
        Outcome::Changed
    }

    pub fn on_click(&mut self, id: WidgetId, pos: Position) -> (Outcome, Option<GridAction>) {
        self.on_click_with(id, pos, KeyModifiers::NONE, 1)
    }

    pub fn context_selection(&mut self, display: usize, col: usize) {
        if self.anchor.is_some() && self.in_range(display, col) {
            return;
        }
        self.anchor = None;
        self.cursor = (display, col);
        if !self.selection.is_empty() {
            self.selection
                .context_select(display, &self.order, |_| true);
        }
    }

    pub fn select_rows(&mut self, g: SelectGesture, display: usize) -> bool {
        self.selection.apply(g, display, &self.order, |_| true)
    }

    pub fn on_click_with(
        &mut self,
        id: WidgetId,
        pos: Position,
        mods: KeyModifiers,
        count: u8,
    ) -> (Outcome, Option<GridAction>) {
        let count = count.max(1);
        if self.seam_of(id).is_some() {
            self.sizing.release();
            return (Outcome::Consumed, None);
        }
        if let Some(c) = self.choice.as_mut() {
            if c.editor.contains_id(id) {
                let (out, r) = c.editor.on_click(id);
                return (out, self.on_choice(r));
            }

            self.choice = None;
        }
        if let Some((d, c)) = self.locate_open(id) {
            self.point_cursor(d, c, false);
            return self.begin_edit();
        }
        if id == self.bar[0].id && self.bar[0].on_click() {
            return (Outcome::Changed, Some(GridAction::PreviewSql));
        }
        if id == self.bar[1].id && self.bar[1].on_click() {
            return (Outcome::Changed, Some(GridAction::DiscardRequested));
        }
        if id == self.bar[2].id && self.bar[2].on_click() {
            return (Outcome::Changed, Some(GridAction::CommitRequested));
        }
        if id == self.more_id() {
            self.set_cursor(self.rows.len(), self.cursor.1, false);
            return (Outcome::Changed, Some(GridAction::FetchMore));
        }
        if id == self.left_id() {
            self.scroll.h.scroll_by(-1);
            return (Outcome::Changed, None);
        }
        if id == self.right_id() {
            self.scroll.h.scroll_by(1);
            return (Outcome::Changed, None);
        }
        if scrollbar::is_bar_of(id, self.id) {
            return (self.bar_press(pos).outcome().or(Outcome::Consumed), None);
        }
        if let Some(c) = self.header_of(id) {
            if self.edit.is_some() {
                self.commit_edit();
                if self.edit.is_some() {
                    return (Outcome::Changed, None);
                }
            }
            if !self.columns[c].sortable {
                return (Outcome::Consumed, None);
            }
            let next = match self.sort {
                Some((sc, SortDir::Asc)) if sc == c => Some((c, SortDir::Desc)),
                Some((sc, SortDir::Desc)) if sc == c => None,
                _ => Some((c, SortDir::Asc)),
            };
            return self.request_sort(next);
        }
        if let Some(d) = self.rownum_of(id) {
            if let Some(GridDrag::Rows { id: from_id, .. }) = self.drag.take()
                && from_id == id
            {
                return (Outcome::Changed, None);
            }
            if d < self.order.len() {
                let from = self.cursor.0;
                self.point_cursor(d, self.cursor.1, false);
                let g = SelectGesture::from_mods(mods, self.toggle_mods, from);
                self.select_rows(g, d);
            }
            return (Outcome::Changed, None);
        }
        if let Some((d, c)) = self.cell_of(id) {
            if let Some(e) = &self.edit {
                if self.order.get(d) == Some(&e.row) && e.col == c {
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
            let same = self.cursor == (d, c) && self.anchor.is_none();

            if let Some(rect) = self.col_rects.get(c.saturating_sub(self.scroll.h.offset))
                && self.columns[c].references.is_some()
                && pos.x + 1 == rect.right()
                && let Some(&src) = self.order.get(d)
            {
                return (
                    Outcome::Changed,
                    Some(GridAction::FollowReference { row: src, col: c }),
                );
            }

            let extend = mods.contains(KeyModifiers::SHIFT);
            self.point_cursor(d, c, extend);
            if extend {
                return (Outcome::Changed, None);
            }
            if self.editable && self.click.edits(count, same) {
                return self.begin_edit();
            }
            if !self.editable && self.click.activates(count, same) {
                return (
                    Outcome::Changed,
                    self.cursor_src().map(GridAction::Activated),
                );
            }
            return (Outcome::Changed, None);
        }
        (Outcome::Ignored, None)
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if self.edit.as_ref().is_some_and(|e| e.field.pressed()) {
            if let Some(at) = self.edit_offset(pos)
                && let Some(e) = self.edit.as_mut()
            {
                e.field.drag(at);
            }
            return Outcome::Changed;
        }
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

        let row_origin = match self.drag {
            Some(GridDrag::Rows { id, .. }) if id == pressed => None,
            _ => self.rownum_of(pressed),
        };
        if row_origin.is_some()
            || matches!(self.drag, Some(GridDrag::Rows { id, .. }) if id == pressed)
        {
            return self.drag_rows(pressed, row_origin, pos);
        }

        let from_cell = matches!(self.drag, Some(GridDrag::Cells(id)) if id == pressed)
            || self.cell_of(pressed).is_some();
        if !from_cell || self.body.height == 0 {
            return Outcome::Ignored;
        }
        self.drag = Some(GridDrag::Cells(pressed));
        let (d, c) = (self.drag_row(pos.y), self.drag_col(pos.x));
        if (d, c) != self.cursor {
            self.point_cursor(d, c, true);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn drag_row(&self, y: u16) -> usize {
        let d = self.scroll.offset as isize + (y as isize - self.body.y as isize);
        (d.max(0) as usize).min(self.content_rows().saturating_sub(1))
    }

    pub(crate) fn column_at(&self, x: u16) -> Option<usize> {
        self.col_rects
            .iter()
            .position(|r| x >= r.x && x < r.right())
            .map(|k| self.scroll.h.offset + k)
    }

    fn drag_col(&self, x: u16) -> usize {
        let (Some(first), Some(last)) = (self.col_rects.first(), self.col_rects.last()) else {
            return self.cursor.1;
        };
        let off = self.scroll.h.offset;
        let c = if x < first.x {
            off.saturating_sub(1)
        } else if x >= last.right() + 2 {
            off + self.col_rects.len()
        } else {
            off + self
                .col_rects
                .iter()
                .position(|r| x >= r.x && x < r.right() + 2)
                .unwrap_or(0)
        };
        c.min(self.columns.len().saturating_sub(1))
    }

    fn drag_rows(&mut self, pressed: WidgetId, origin: Option<usize>, pos: Position) -> Outcome {
        let d = self.drag_row(pos.y);
        let (from, mark) = match (self.drag, origin) {
            (Some(GridDrag::Rows { from, mark, .. }), _) => (from, mark),
            (_, Some(from)) => {
                if d == from {
                    return Outcome::Ignored;
                }
                let src = self.order.get(from).copied();
                (from, !src.is_some_and(|s| self.selection.contains(s)))
            }
            _ => return Outcome::Ignored,
        };
        self.drag = Some(GridDrag::Rows {
            id: pressed,
            from,
            mark,
        });
        for r in from.min(d)..=from.max(d) {
            if let Some(&src) = self.order.get(r) {
                if mark {
                    self.selection.set(src, true);
                } else {
                    self.selection.set(src, false);
                }
            }
        }
        self.point_cursor(d, self.cursor.1, false);
        Outcome::Changed
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

    pub fn position_label(&self) -> String {
        match self.cols_label() {
            Some(c) => format!("{} · {c}", self.rows_label()),
            None => self.rows_label(),
        }
    }

    pub fn rows_label(&self) -> String {
        use crate::ui::text::thousands;
        if self.rows.is_empty() {
            return "0 rows".into();
        }
        let r = self.scroll.visible_range();
        let (a, b) = (r.start + 1, r.end.min(self.rows.len()));
        let total = match self.total {
            RowTotal::Exact(n) if self.more => format!(
                "{} loaded · {} total",
                thousands(self.rows.len()),
                thousands(n)
            ),
            RowTotal::Estimated(n) if self.more => format!(
                "{} loaded · ~{} total",
                thousands(self.rows.len()),
                thousands(n)
            ),
            RowTotal::Exact(n) => thousands(n),
            RowTotal::Estimated(n) => format!("~{}", thousands(n)),
            RowTotal::Unknown => thousands(self.rows.len()),
        };
        format!("rows {}–{} of {total}", thousands(a), thousands(b))
    }

    pub fn cols_label(&self) -> Option<String> {
        if self.rows.is_empty() || !self.scroll.h.overflows() {
            return None;
        }
        let c0 = self.scroll.h.offset + 1;
        let c1 = (self.scroll.h.offset + self.scroll.h.viewport_len).min(self.columns.len());
        Some(format!("cols {c0}–{c1} of {}", self.columns.len()))
    }

    pub fn pending_label(&self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        let (u, i, d) = self.pending.counts();
        let mut parts = vec![];
        if u > 0 {
            parts.push(format!("{u} update{}", if u == 1 { "" } else { "s" }));
        }
        if i > 0 {
            parts.push(format!("{i} insert{}", if i == 1 { "" } else { "s" }));
        }
        if d > 0 {
            parts.push(format!("{d} delete{}", if d == 1 { "" } else { "s" }));
        }
        Some(parts.join(" · "))
    }

    fn layout_columns(&mut self, area: Rect) {
        let n = self.columns.len();
        let gap = 2u16;
        let widths = (0..n).map(|i| self.col_w(i)).collect();
        let fit = self.scroll.layout_columns(widths, gap, area.width);
        let fit = fit
            .max(1)
            .min(n.saturating_sub(self.scroll.h.offset).max(1));
        self.scroll.h.content_len = n;
        self.scroll.h.viewport_len = fit;
        self.scroll.h.clamp();
        self.col_rects.clear();
        let mut x = area.x;
        for i in self.scroll.h.offset..(self.scroll.h.offset + fit).min(n) {
            let w = self.col_w(i).min(area.right().saturating_sub(x));
            self.col_rects.push(Rect::new(x, area.y, w, area.height));
            x += w + gap;
        }

        let next = self.scroll.h.offset + fit;
        if next < n && x < area.right() {
            let rest = area.right() - x;
            if rest >= 6 {
                self.col_rects.push(Rect::new(x, area.y, rest, area.height));
            }
        }
    }

    fn fit_header_marks(&mut self) {
        for ci in 0..self.columns.len() {
            let sorted = self.sort.is_some_and(|(c, _)| c == ci);
            let filtered = self.filtered_cols.contains(&ci);
            if !sorted && !filtered {
                continue;
            }
            let col = &self.columns[ci];
            let need = width(&col.name)
                + if col.primary { 2 } else { 0 }
                + if filtered { 2 } else { 0 }
                + if sorted { 2 } else { 0 }
                + 1;
            let need = (need as u16).min(col.max_width.max(col.min_width));
            if let Some(w) = self.widths.get_mut(ci) {
                *w = (*w).max(need);
            }
        }
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.parts.clear();

        if let Some(GridDrag::Cells(id) | GridDrag::Rows { id, .. }) = self.drag
            && ctx.interaction.pressed != Some(id)
        {
            self.drag = None;
        }
        self.follow_strings(ctx.theme.strings);
        self.scroll.begin(ctx.track_click);
        self.fit_header_marks();
        let area = area.intersection(*buf.area());
        if area.is_empty() || area.height < 2 {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);

        ctx.control(self.id, area, false);
        if self.has_selection() {
            ctx.selection(self.id);
        }
        ctx.scrollable(self.id, area);

        let bar_h = if self.show_bar && !self.pending.is_empty() {
            2
        } else {
            0
        };
        let grid_area = Rect::new(
            area.x,
            area.y,
            area.width,
            area.height.saturating_sub(bar_h),
        );
        let mut body = Rect::new(
            grid_area.x,
            grid_area.y + 1,
            grid_area.width,
            grid_area.height.saturating_sub(1),
        );
        let h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        self.scroll.h_cfg = h_cfg;

        let rows_len = self.content_rows();
        let mut has_sb = body.height > 0 && rows_len > body.height as usize;

        let num_w = if self.row_numbers {
            (self.rows.len().max(1).to_string().len() as u16).max(2)
        } else {
            0
        };
        let gutter_w = 3 + num_w + if self.row_numbers { 1 } else { 0 };
        let cols_for = |has_sb: bool| {
            Rect::new(
                grid_area.x + gutter_w,
                grid_area.y,
                grid_area
                    .width
                    .saturating_sub(gutter_w + 4 + u16::from(has_sb)),
                grid_area.height,
            )
        };
        self.layout_columns(cols_for(has_sb));

        let has_rows = !self.rows.is_empty() || self.more;
        let has_hb = has_rows && body.height > 1 && h_cfg.shows_bar(&self.scroll.h);
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
        if has_rows {
            ctx.autoscroll(body);
        }

        let header_y = grid_area.y;
        fill(
            buf,
            Rect::new(grid_area.x, header_y, grid_area.width, 1),
            Style::new().bg(bg),
        );
        if self.overflow_badges && self.scroll.h.offset > 0 {
            let lbl = format!("{}{}", t.glyphs.overflow_left, self.scroll.h.offset);
            let st = ctx.title_style(self.left_id(), bg);
            buf.set_string(grid_area.x + 1, header_y, &lbl, st);
            ctx.register_part(
                self.id,
                PartRef::OVERFLOW_BEFORE,
                Rect::new(
                    grid_area.x + 1,
                    header_y,
                    crate::ui::text::width(&lbl) as u16,
                    1,
                ),
            );
        }
        for k in 0..self.col_rects.len() {
            let rect = &self.col_rects[k];
            let ci = self.scroll.h.offset + k;
            let col = &self.columns[ci];
            let hid = self.header_id(ci);
            let sorted = self.sort.map(|(c, _)| c == ci).unwrap_or(false);
            let filtered = self.filtered_cols.contains(&ci);
            let active = sorted || filtered || ci == self.cursor.1 && focused;
            let st = header_style(ctx, hid, bg, col.sortable, active);
            let mut suffix = String::new();
            if filtered {
                suffix.push(' ');
                suffix.push_str(t.glyphs.filter);
            }
            match self.sort {
                Some((c, SortDir::Asc)) if c == ci => {
                    suffix.push(' ');
                    suffix.push_str(t.glyphs.sort_asc);
                }
                Some((c, SortDir::Desc)) if c == ci => {
                    suffix.push(' ');
                    suffix.push_str(t.glyphs.sort_desc);
                }
                _ => {}
            }

            let prefix = if col.primary { "  " } else { "" };
            let avail = (rect.width as usize).saturating_sub(width(&suffix) + width(prefix));
            let shown = truncate(&col.name, avail.max(1));
            let cut = shown != col.name;
            let title = format!("{prefix}{shown}{suffix}");
            let text = if col.kind.right_aligned() {
                fit_right(&title, rect.width as usize)
            } else {
                fit(&title, rect.width as usize)
            };
            buf.set_string(rect.x, header_y, &text, st);
            if col.primary {
                let px = if col.kind.right_aligned() {
                    rect.right().saturating_sub(width(&title) as u16)
                } else {
                    rect.x
                };
                buf.set_string(px, header_y, t.glyphs.key, st.fg(t.text_faint));
            }
            let head = Rect::new(rect.x, header_y, rect.width, 1);
            if col.sortable {
                ctx.register_part(
                    self.id,
                    PartRef::item(Part::HEADER, ItemKey::index(ci)),
                    head,
                );
            } else if (cut || !col.type_label.is_empty()) && !ctx.inert {
                ctx.hits.register_hover(hid, head);
            }
            if !col.type_label.is_empty() {
                let tip = crate::ui::tooltip::Tip::new(&format!(
                    "{}{}{}",
                    col.name, t.glyphs.separator, col.type_label
                ));
                crate::ui::tooltip::show(ctx, hid, head, &tip, None);
            } else if cut {
                crate::ui::reveal::truncated(ctx, self.reveal, hid, head, &col.name);
            }
            if self.sizing.resizable {
                let hit = self.sizing.seam_rect(rect.right(), header_y, 2, grid_area);
                let held = self.sizing.dragging() == Some(ci);
                crate::ui::columns::render_seam(
                    buf,
                    ctx,
                    (self.id, ci),
                    hit,
                    rect.right(),
                    bg,
                    held,
                );
            }
        }
        let hidden_right = self
            .columns
            .len()
            .saturating_sub(self.scroll.h.offset + self.scroll.h.viewport_len);
        if self.overflow_badges && hidden_right > 0 {
            let lbl = format!("{hidden_right}{}", t.glyphs.overflow_right);
            let x = cols_area.right() + 1;
            let st = ctx.title_style(self.right_id(), bg);
            buf.set_string(x, header_y, &lbl, st);
            ctx.register_part(
                self.id,
                PartRef::OVERFLOW_AFTER,
                Rect::new(x, header_y, crate::ui::text::width(&lbl) as u16, 1),
            );
        }

        if self.rows.is_empty() && !self.more {
            if self.loading {
                crate::widgets::progress::render_spinner(
                    Rect::new(body.x + 3, body.y + body.height / 2, body.width, 1),
                    buf,
                    ctx,
                    t.strings.loading_rows,
                    bg,
                );
            } else {
                crate::widgets::empty::render(body, buf, t, &self.empty, bg);
            }
            return;
        }

        let edited_rows: std::collections::HashSet<ItemKey> =
            self.pending.cells.keys().map(|&(key, _)| key).collect();
        let editing_here = self
            .edit
            .as_ref()
            .filter(|_| focused)
            .map(|e| (e.row, e.col));
        let choice_here = self.choice.as_ref().map(|c| (c.row, c.col));
        let mut choice_anchor: Option<Rect> = None;
        let mut open_marks: Vec<(usize, usize, Rect)> = Vec::new();

        let flagged_msg = String::new();
        for (i, d) in self.scroll.visible_range().enumerate() {
            let y = body.y + i as u16;
            let row_rect = Rect::new(
                grid_area.x,
                y,
                grid_area.width.saturating_sub(u16::from(has_sb)),
                1,
            );
            if d >= self.rows.len() {
                let mid = self.more_id();
                let mut s = ctx.child_state(self.id, mid, d == self.cursor.0);
                s.focused = focused && d == self.cursor.0;
                let st = RowUi::paint(ctx, buf, row_rect, s, RowLook::Bar, bg).style();
                let text = if self.loading {
                    format!(
                        "{} {}",
                        t.glyphs.spinner_frame(ctx.interaction.tick),
                        t.strings.fetching
                    )
                } else {
                    format!(
                        "{} {}",
                        t.glyphs.arrows[1],
                        crate::theme::Strings::fill(
                            t.strings.fetch_more,
                            &[&crate::ui::text::thousands(self.rows.len())]
                        )
                    )
                };
                buf.set_string(row_rect.x + gutter_w, y, &text, st.fg(t.text_muted));
                ctx.register_part(self.id, PartRef::of(Part::ROW), row_rect);
                continue;
            }
            let src = self.order[d];
            let rid = self.rownum_id(d);
            let mut s = ctx.child_state(self.id, rid, d == self.cursor.0);
            let visible_cols =
                self.scroll.h.offset..self.scroll.h.offset + self.scroll.h.viewport_len;
            let hovered_cell = visible_cols
                .clone()
                .find(|&c| ctx.interaction.hovered(self.cell_id(d, c)));

            s.hovered |= hovered_cell.is_some();
            s.pressed |= ctx
                .interaction
                .pressed_any(visible_cols.map(|c| self.cell_id(d, c)));
            s.focused = focused && d == self.cursor.0;
            s.selected = self.selection.contains(src);
            let state = self.row_state_with(src, |key| edited_rows.contains(&key));
            let rp = RowUi::resolve(ctx, row_rect, s, RowLook::Bar, bg);
            let mut drawn = rp;
            if state == RowState::Deleted {
                drawn.look.style = rp
                    .look
                    .style
                    .fg(t.text_faint)
                    .add_modifier(Modifier::CROSSED_OUT);
            }
            drawn.draw(buf);
            let row_style = drawn.style();
            if s.selected {
                rp.mark(ctx, buf, row_rect.x + 1, y, MarkKind::Tick, focused);
            }
            let (glyph, gs) = match state {
                RowState::Clean => (" ", row_style),
                RowState::Modified => (t.glyphs.dirty, row_style.fg(t.warning)),
                RowState::Inserted => ("+", row_style.fg(t.text_secondary)),
                RowState::Deleted => (t.glyphs.removed, row_style.fg(t.text_muted)),
                RowState::Error => ("!", row_style.fg(t.error).add_modifier(Modifier::BOLD)),
            };
            buf.set_string(
                row_rect.x + 2,
                y,
                glyph,
                gs.remove_modifier(Modifier::CROSSED_OUT),
            );
            if self.row_numbers {
                let n = fit_right(&(src + 1).to_string(), num_w as usize);
                let ns = row_style
                    .fg(if s.focused {
                        t.text_secondary
                    } else {
                        t.text_faint
                    })
                    .remove_modifier(Modifier::BOLD | Modifier::CROSSED_OUT);
                buf.set_string(row_rect.x + 3, y, &n, ns);
                let rect = Rect::new(row_rect.x, y, gutter_w, 1);
                let part = Self::rownum_part(d);
                self.parts.record(self.id.part(part), part);
                ctx.register_part(self.id, part, rect);
            }
            for k in 0..self.col_rects.len() {
                let rect = &self.col_rects[k];
                let ci = self.scroll.h.offset + k;
                let col = &self.columns[ci];
                let cell_rect = Rect::new(rect.x, y, rect.width, 1);
                let is_cursor = focused && (d, ci) == self.cursor;
                if editing_here == Some((src, ci)) {
                    let e = self.edit.as_ref().unwrap();
                    let es = t.field_style(VisualState {
                        editing: true,
                        ..Default::default()
                    });
                    fill(buf, cell_rect, es);
                    let cw = rect.width.saturating_sub(1) as usize;
                    let cur = e.field.buffer.cursor_pos().col;
                    let off = cur.saturating_sub(cw.saturating_sub(1));
                    let text = e.field.text();
                    let shown = crate::ui::text::slice_cells(text, off, cw);
                    let mut ts = es
                        .add_modifier(Modifier::UNDERLINED)
                        .underline_color(t.accent);
                    if e.field.error.is_some() {
                        ts = ts.underline_color(t.error);
                    }
                    buf.set_string(rect.x, y, &shown, ts);
                    if cw > 0 {
                        ctx.set_cursor(Position::new(rect.x + (cur - off) as u16, y));
                    }
                    if e.field.error.is_some() {
                        buf.set_string(
                            rect.right().saturating_sub(1),
                            y,
                            "!",
                            es.fg(t.error).add_modifier(Modifier::BOLD),
                        );
                    }
                } else {
                    let v = self.value(src, ci);
                    let dirty = self
                        .row_key(src)
                        .is_some_and(|key| self.pending.is_dirty(key, ci))
                        && state != RowState::Inserted;
                    let cfg = &self.editor_config;
                    let in_choice = choice_here == Some((src, ci));
                    if in_choice {
                        choice_anchor = Some(cell_rect);
                    }
                    let is_cursor = is_cursor || in_choice;

                    let flagged = cfg.flag_out_of_set && self.out_of_set(src, ci);
                    let err = self.cell_errors.get(&(src, ci));
                    let err = err.or(flagged.then_some(&flagged_msg));
                    let mut st = row_style;
                    let affordance = self.editable
                        && !col.read_only
                        && col.editor.has_popup()
                        && state != RowState::Deleted
                        && rect.width > 3
                        && match cfg.affordance {
                            Affordance::Never => false,
                            Affordance::Always => true,
                            Affordance::Focused => is_cursor || hovered_cell == Some(ci),
                        };

                    let ref_mark = col.references.is_some()
                        && !matches!(v, CellValue::Null | CellValue::Default)
                        && rect.width > 6;
                    let reaches_mark = ref_mark
                        && (col.kind.right_aligned()
                            || width(&v.text_ref()) + 1 >= rect.width as usize);
                    let text_w = if affordance || reaches_mark {
                        rect.width - 2
                    } else {
                        rect.width
                    } as usize;
                    let text = match v {
                        CellValue::Text(raw) if col.editor.is_constrained() => {
                            truncate(&col.editor.display(raw, cfg), text_w)
                        }
                        _ => cell_text_with(v, col.kind, text_w, &t.glyphs),
                    };

                    let on = st.bg.unwrap_or(bg);
                    let literal = |st: Style, tone: SyntaxTone| {
                        let syn = t.syntax(tone);
                        st.fg(t.legible(syn.fg.unwrap_or(t.text_primary), on))
                            .add_modifier(syn.add_modifier)
                    };
                    match v {
                        CellValue::Null => {
                            st = literal(st, SyntaxTone::Literal).add_modifier(Modifier::ITALIC)
                        }
                        CellValue::Default => {
                            st = st.fg(t.text_muted).add_modifier(Modifier::ITALIC)
                        }
                        CellValue::Bool(_) => st = literal(st, SyntaxTone::Literal),
                        CellValue::Json(_) => st = literal(st, SyntaxTone::Str),
                        CellValue::Text(s) if s.is_empty() => st = st.fg(t.text_faint),
                        _ => {}
                    }
                    if col.primary && !s.focused {
                        st = st.fg(t.text_secondary);
                    }
                    if self.in_range(d, ci) && !is_cursor {
                        st = st.patch(t.selection());
                    }
                    if dirty {
                        st = st.fg(t.warning).remove_modifier(Modifier::ITALIC);
                    }
                    if err.is_some() {
                        st = st.fg(t.error).remove_modifier(Modifier::ITALIC);
                    }
                    if is_cursor {
                        st = if self.in_range(d, ci) {
                            t.cursor_in_selection(err.is_some(), s.pressed)
                        } else {
                            t.cell_cursor(err.is_some(), s.pressed)
                        };
                        if state == RowState::Deleted {
                            st = st.add_modifier(Modifier::CROSSED_OUT);
                        }
                    } else if hovered_cell == Some(ci)
                        && self.editable
                        && !col.read_only
                        && state != RowState::Deleted
                    {
                        st = st
                            .add_modifier(Modifier::UNDERLINED)
                            .underline_color(t.border_strong);
                    }
                    let shown = if col.kind.right_aligned() {
                        fit_right(&text, text_w)
                    } else {
                        fit(&text, text_w)
                    };
                    if affordance || reaches_mark {
                        fill(buf, cell_rect, st);
                    }
                    buf.set_string(rect.x, y, &shown, st);
                    if affordance {
                        let mark = Rect::new(rect.right() - 1, y, 1, 1);
                        let ms = if is_cursor {
                            st
                        } else {
                            st.fg(t.text_secondary)
                        };
                        buf.set_string(mark.x, y, t.glyphs.disclosure_open, ms);
                        open_marks.push((d, ci, mark));
                    }

                    let full = v.text_ref();
                    if text != *full {
                        crate::ui::reveal::truncated(
                            ctx,
                            self.reveal,
                            self.cell_id(d, ci),
                            Rect::new(rect.x, y, rect.width, 1),
                            &full,
                        );

                        let cfg = self.reveal.unwrap_or(ctx.reveal);
                        if is_cursor && (cfg.hover || cfg.keyboard.is_some()) {
                            crate::ui::reveal::peek(ctx, &full);
                        }
                    }
                    if ref_mark {
                        buf.set_string(
                            rect.right().saturating_sub(1),
                            y,
                            t.glyphs.arrow,
                            st.fg(if is_cursor { t.canvas } else { t.text_muted }),
                        );
                    }
                    if err.is_some() && !is_cursor && !affordance {
                        buf.set_string(
                            rect.right().saturating_sub(1),
                            y,
                            "!",
                            st.add_modifier(Modifier::BOLD),
                        );
                    }
                }
                let cid = self.cell_id(d, ci);
                let cell = (Self::cell_part(d), ci);
                self.parts.record_cell(cid, cell.0, cell.1);
                ctx.register_part_as(cid, self.id, cell.0, cell_rect);
            }
        }

        for (d, ci, rect) in open_marks {
            let oid = self.open_id(d, ci);
            let open = (PartRef::item(Part::ACTIONS, ItemKey::index(d)), ci);
            self.parts.record_cell(oid, open.0, open.1);
            ctx.register_part_as(oid, self.id, open.0, rect);
        }
        if let (Some(anchor), Some(c)) = (choice_anchor, self.choice.as_mut()) {
            c.editor.render(anchor, buf, ctx, &self.editor_config);
        }

        ctx.scrollable(self.id, area);
        if has_sb {
            crate::ui::fade::scroll_edges(
                buf,
                ctx,
                Rect::new(
                    area.x,
                    body.y,
                    (area.right() - 1).saturating_sub(area.x),
                    body.height,
                ),
                &self.scroll,
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

        if bar_h > 0 {
            let by = area.bottom() - 1;
            let bar_area = Rect::new(area.x, by, area.width, 1);
            fill(buf, bar_area, Style::new().bg(bg));
            let (u, i, d) = self.pending.counts();
            let count = u + i + d;
            let text = format!("{} {count} pending", t.glyphs.dirty);
            buf.set_string(area.x + 1, by, &text, t.primary().fg(t.warning).bg(bg));

            let cursor_src = self.order.get(self.cursor.0).copied();
            let dot = t.glyphs.separator.trim_start();
            let (detail, ds) = match cursor_src.and_then(|r| self.row_errors.get(&r)) {
                Some(msg) => (format!("{dot}{msg}"), t.error_fg().bg(bg)),
                None => (
                    self.pending_label()
                        .map(|d| format!("{dot}{d}"))
                        .unwrap_or_default(),
                    t.muted().bg(bg),
                ),
            };
            let bw: u16 = self.bar.iter().map(|b| b.width() + 1).sum();
            let room = area.width.saturating_sub(width(&text) as u16 + 4 + bw) as usize;
            buf.set_string(
                area.x + 2 + width(&text) as u16,
                by,
                truncate(&detail, room),
                ds,
            );
            let widths: Vec<u16> = self.bar.iter().map(|b| b.width()).collect();
            let rects = row_layout_right(
                Rect::new(area.x, by, area.width.saturating_sub(1), 1),
                &widths,
                1,
            );
            for (b, r) in self.bar.iter_mut().zip(rects) {
                b.render(r, buf, ctx, bg);
            }
        }
    }

    pub fn bar_ids(&self) -> [WidgetId; 3] {
        [self.bar[0].id, self.bar[1].id, self.bar[2].id]
    }

    pub fn on_bar_key(&mut self, id: WidgetId, key: &Key) -> (Outcome, Option<GridAction>) {
        for (i, b) in self.bar.iter_mut().enumerate() {
            if b.id == id {
                let (o, act) = b.on_key(key);
                if act {
                    let ev = match i {
                        0 => GridAction::PreviewSql,
                        1 => GridAction::DiscardRequested,
                        _ => GridAction::CommitRequested,
                    };
                    return (o, Some(ev));
                }
                return (o, None);
            }
        }
        (Outcome::Ignored, None)
    }
}

fn cmp_cells(a: &CellValue, b: &CellValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let num = |v: &CellValue| match v {
        CellValue::Int(i) => Some(*i as f64),
        CellValue::Num(n) => Some(*n),
        _ => None,
    };
    match (a, b) {
        (CellValue::Null, CellValue::Null) => Ordering::Equal,
        (CellValue::Null, _) => Ordering::Greater,
        (_, CellValue::Null) => Ordering::Less,
        _ => match (num(a), num(b)) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            _ => a.text().to_lowercase().cmp(&b.text().to_lowercase()),
        },
    }
}

pub fn cell_text(v: &CellValue, kind: CellKind, w: usize) -> String {
    cell_text_with(v, kind, w, &crate::theme::Glyphs::UNICODE)
}

pub fn cell_text_with(v: &CellValue, kind: CellKind, w: usize, g: &crate::theme::Glyphs) -> String {
    match v {
        CellValue::Null => "NULL".into(),
        CellValue::Default => "DEFAULT".into(),
        CellValue::Text(s) if s.is_empty() => "''".into(),
        CellValue::Json(j) => {
            if w < 8 {
                if j.trim_start().starts_with('[') {
                    format!("[{}]", g.ellipsis)
                } else {
                    format!("{{{}}}", g.ellipsis)
                }
            } else {
                truncate(&shown_prefix(w, |limit| one_line_json(j, limit)), w)
            }
        }
        CellValue::Text(s) => {
            if kind == CellKind::Id {
                truncate_middle(&sanitized(s, usize::MAX, g).0, w)
            } else {
                truncate(&shown_prefix(w, |limit| sanitized(s, limit, g)), w)
            }
        }
        other => truncate(&other.text_ref(), w),
    }
}

const TEXT_CELL_CHARS: usize = 10_000;

fn sanitized(s: &str, limit: usize, g: &crate::theme::Glyphs) -> (String, bool) {
    let mut out = String::new();
    let mut chars = s.chars();
    for _ in 0..TEXT_CELL_CHARS {
        if out.len() >= limit {
            return (out, false);
        }
        let Some(c) = chars.next() else {
            return (out, true);
        };
        match c {
            '\n' => out.push_str(g.newline),
            '\t' => out.push_str(g.tab),
            c if c.is_control() => out.push_str(g.control),
            c => out.push(c),
        }
    }
    (out, true)
}

fn one_line_json(j: &str, limit: usize) -> (String, bool) {
    let mut out = String::new();
    for word in j.split_whitespace() {
        if out.len() >= limit {
            return (out, false);
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    (out, true)
}

fn shown_prefix(w: usize, make: impl Fn(usize) -> (String, bool)) -> String {
    let mut limit = w.saturating_mul(4).saturating_add(64);
    loop {
        let (text, complete) = make(limit);
        if complete || wider_than(&text, w.saturating_add(2)) {
            return text;
        }
        limit = limit.saturating_mul(2);
    }
}

impl Theme {
    pub fn change_glyph(&self, state: RowState) -> (&'static str, Color) {
        match state {
            RowState::Clean => (" ", self.text_faint),
            RowState::Modified => (self.glyphs.dirty, self.warning),
            RowState::Inserted => ("+", self.text_secondary),
            RowState::Deleted => (self.glyphs.removed, self.text_muted),
            RowState::Error => ("!", self.error),
        }
    }
}

pub(crate) fn header_style(
    ctx: &RenderCtx,
    id: WidgetId,
    bg: Color,
    sortable: bool,
    active: bool,
) -> Style {
    let t = ctx.theme;
    let base = if sortable {
        ctx.title_style(id, bg)
    } else {
        Style::new().bg(bg)
    };
    let lifted = sortable && (ctx.interaction.hovered(id) || ctx.interaction.pressed(id));
    let ink = if active || lifted {
        t.text_primary
    } else {
        t.text_muted
    };
    let st = base.fg(t.legible(ink, base.bg.unwrap_or(bg)));

    if active {
        st.add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        st
    }
}

impl crate::core::focus::BlurTarget for DataGrid {
    fn blur_id(&self) -> WidgetId {
        self.id
    }
    fn blur(&mut self) -> Outcome {
        self.on_blur().0
    }
}

impl Scrollable for DataGrid {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        let c = self.choice.as_mut().filter(|_| axis == Axis::Vertical)?;
        Some(c.editor.wheel(axis, delta))
    }
}

impl RowSource for DataGrid {
    fn len(&self) -> usize {
        self.content_rows()
    }
    fn label(&self, _: usize) -> &str {
        ""
    }
    fn key(&self, display: usize) -> ItemKey {
        self.order
            .get(display)
            .and_then(|&src| self.row_key(src))
            .unwrap_or(FETCH_MORE_KEY)
    }
}

impl Navigable for DataGrid {
    fn cursor(&self) -> Cursor {
        Cursor::new(self.cursor.0)
    }
}

impl crate::core::pointer::MouseTarget for DataGrid {
    type Action = GridAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<GridAction>> {
        let owner = ev.owner();
        let mine = owner == self.id
            || self.bar_ids().contains(&owner)
            || self.choice.as_ref().is_some_and(|c| c.editor.id == owner);
        mine.then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<GridAction> {
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
            Some(Phase::Press) => self.on_press(id, pos, ev.count()).into(),
            Some(Phase::Drag) => self.on_drag(id, pos).into(),
            Some(Phase::Click) => self.on_click_with(id, pos, ev.mods(), ev.count()).into(),
            _ => Response::ignored(),
        }
    }
}
