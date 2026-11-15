use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::BlurAction;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, KeyMap, NavKeys, hint_key, label_for};
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::{HScroll, ScrollbarVisibility, TrackClick};
use junie_tui::theme::Tone;
use junie_tui::ui::columns::HeaderLabels;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::chips::{Chip, ChipBar};
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::grid::{
    CellEditResult, CellKind, CellValue, ColumnSpec, DataGrid, DeletedRow, FinishEdit, GridAction,
    GridChangeSet, GridCmd, GridCommitFailure, GridCommitToken, GridEditError, GridRows,
    GridUndoKind, InsertedRow, RowState, RowTotal, UpdatedRow,
};
use junie_tui::widgets::option_editor::{
    Affordance, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("grid");
const PREVIEW: WidgetId = ID.sub("preview");
const REVIEW: WidgetId = ID.sub("review");
const PAGE: usize = 40;
const ALL: usize = 96;

const TABLE: &str = "customers";
const SEATS_COLUMN: usize = 3;

const NAMES: &[&str] = &[
    "Northwind Traders",
    "Blue Yonder Airlines",
    "Contoso Pharmaceuticals",
    "Fabrikam Robotics",
    "Litware Analytics",
    "Tailspin Toys",
    "Wide World Importers",
    "Adventure Works",
    "Proseware Studio",
    "Woodgrove Bank",
    "Alpine Ski House",
    "Coho Winery",
    "Lucerne Publishing",
    "Margie's Travel",
    "Trey Research",
    "Humongous Insurance",
];
const PLANS: &[&str] = &["free", "pro", "team", "enterprise"];
const OWNERS: &[&str] = &["mira", "jonas", "ana", "kai"];
const TIERS: &[&str] = &["standard", "priority", "dedicated"];

fn row(i: usize) -> Vec<CellValue> {
    let plan = PLANS[(i * 7 + 3) % PLANS.len()];
    let seats = [1, 3, 5, 12, 25, 40, 80, 150][(i * 5 + 1) % 8];
    let mrr = match plan {
        "free" => 0.0,
        "pro" => 29.0 * seats as f64,
        "team" => 24.0 * seats as f64,
        _ => 19.0 * seats as f64,
    };
    let suffix = if i >= NAMES.len() {
        format!(" {}", i / NAMES.len() + 1)
    } else {
        String::new()
    };
    let renewed = if plan == "free" {
        CellValue::Null
    } else {
        CellValue::Text(format!("2026-{:02}-{:02}", 1 + i % 12, 1 + (i * 3) % 28))
    };
    let notes = if i.is_multiple_of(4) {
        CellValue::Json(format!(
            "{{\"owner\":\"{}\",\"seats\":{seats}}}",
            OWNERS[i % OWNERS.len()]
        ))
    } else {
        CellValue::Null
    };
    vec![
        CellValue::Int(1001 + i as i64),
        CellValue::Text(format!("{}{suffix}", NAMES[i % NAMES.len()])),
        CellValue::Text(plan.to_owned()),
        CellValue::Int(seats),
        CellValue::Num(mrr),
        CellValue::Bool(i % 5 != 3),
        renewed,
        CellValue::Int(201 + (i % 4) as i64),
        notes,
        CellValue::Text(TIERS[i % TIERS.len()].to_owned()),
        match i % 6 {
            2 => CellValue::Null,
            0 | 3 => CellValue::Bool(true),
            _ => CellValue::Bool(false),
        },
        CellValue::Num(1.0045 + 0.25 * (i % 4) as f64),
    ]
}

fn page(from: usize, total: RowTotal, cols: usize) -> GridRows {
    let to = (from + PAGE).min(ALL);
    GridRows {
        rows: (from..to)
            .map(|i| {
                let mut r = row(i);
                r.truncate(cols);
                r
            })
            .collect(),
        total: if to >= ALL {
            RowTotal::Exact(ALL)
        } else {
            total
        },
        more: to < ALL,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NonFinite {
    row: usize,
    column: String,
}

impl NonFinite {
    fn message(&self) -> String {
        format!(
            "Cannot build the SQL: row {} · {} is not a finite number",
            self.row + 1,
            self.column
        )
    }
}

/// SQL projection of a pending cell. A finite number prints the shortest text
/// that parses back to the stored `f64`, never the two-decimal display text.
fn literal(row: usize, column: &str, v: &CellValue) -> Result<String, NonFinite> {
    Ok(match v {
        CellValue::Null => "NULL".into(),
        CellValue::Default => "DEFAULT".into(),
        CellValue::Text(s) | CellValue::Json(s) => format!("'{}'", s.replace('\'', "''")),
        CellValue::Num(n) if !n.is_finite() => {
            return Err(NonFinite {
                row,
                column: column.to_owned(),
            });
        }
        CellValue::Num(n) => {
            let mut text = n.to_string();
            if !text.contains(['.', 'e', 'E']) {
                text.push_str(".0");
            }
            text
        }
        other => other.text(),
    })
}

fn delete_statement(row: &DeletedRow) -> String {
    format!("DELETE FROM {TABLE} WHERE id = {};", row.before()[0].text())
}

fn undo_status(kind: GridUndoKind) -> &'static str {
    match kind {
        GridUndoKind::Cell => "Undid a cell edit",
        GridUndoKind::DeleteMark => "Undid a delete mark · the staged edits are back",
        GridUndoKind::Insert => "Undid an insert · the row is gone",
        GridUndoKind::RemovedInsert => "Undid a removed insert · the row and its edits are back",
    }
}

fn tiers() -> Options {
    Options::new(
        std::iter::once(OptionItem::new("").label("(none)"))
            .chain(TIERS.iter().map(|t| OptionItem::new(t))),
    )
}

fn columns(strict: bool) -> Vec<ColumnSpec> {
    let mut cols = vec![
        ColumnSpec::new("id", CellKind::Id)
            .primary()
            .read_only()
            .nullable(false)
            .type_label("integer"),
        ColumnSpec::new("customer", CellKind::Text)
            .nullable(false)
            .type_label("text"),
        ColumnSpec::new("plan", CellKind::Enum)
            .enum_values(PLANS)
            .editor(ColumnEditor::for_options(Options::values(PLANS)))
            .nullable(false)
            .type_label("plan_tier"),
        ColumnSpec::new("seats", CellKind::Number)
            .nullable(false)
            .type_label("integer"),
        ColumnSpec::new("mrr", CellKind::Number)
            .read_only()
            .type_label("numeric(10,2)"),
        ColumnSpec::new("active", CellKind::Bool)
            .nullable(false)
            .type_label("boolean"),
        ColumnSpec::new("renewed_at", CellKind::Timestamp).type_label("date"),
        ColumnSpec::new("account_id", CellKind::Number)
            .references("accounts")
            .type_label("integer"),
        ColumnSpec::new("notes", CellKind::Json).type_label("jsonb"),
    ];
    if strict {
        cols.extend([
            ColumnSpec::new("tier", CellKind::Text)
                .editor(ColumnEditor::Select(tiers()))
                .type_label("text"),
            ColumnSpec::new("paused", CellKind::Bool).type_label("boolean"),
            ColumnSpec {
                min_width: 8,
                ..ColumnSpec::new("rate", CellKind::Number).type_label("double precision")
            },
        ]);
    }
    cols
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Knobs {
    access: usize,
    rows: usize,
    total: usize,
    row_numbers: usize,
    overflow_badges: usize,
    labels: usize,
    type_ahead: usize,
    affordance: usize,
    keymap: usize,
    duplicate_keys: usize,
    nav: usize,
    width_sample_rows: usize,
    seams: usize,
    track_click: usize,
    horizontal: usize,
    validator: usize,
    blur_action: usize,
    malformed: usize,
}

const KEYS: [&str; 18] = [
    "datagrid.access",
    "datagrid.rows",
    "datagrid.total",
    "datagrid.row_numbers",
    "datagrid.overflow_badges",
    "datagrid.labels",
    "datagrid.type_ahead",
    "datagrid.affordance",
    "datagrid.keymap",
    "datagrid.duplicate_keys",
    "datagrid.nav",
    "datagrid.width_sample_rows",
    "datagrid.seams",
    "datagrid.track_click",
    "datagrid.horizontal",
    "datagrid.validator",
    "datagrid.blur_action",
    "datagrid.malformed",
];

impl Knobs {
    fn defaults() -> Self {
        let mut k = Self::default();
        for key in KEYS {
            let opt = crate::options::find(key).expect("a datagrid option");
            *k.slot(key) = opt.default;
        }
        k
    }

    fn slot(&mut self, key: &str) -> &mut usize {
        match key {
            "datagrid.access" => &mut self.access,
            "datagrid.rows" => &mut self.rows,
            "datagrid.total" => &mut self.total,
            "datagrid.row_numbers" => &mut self.row_numbers,
            "datagrid.overflow_badges" => &mut self.overflow_badges,
            "datagrid.labels" => &mut self.labels,
            "datagrid.type_ahead" => &mut self.type_ahead,
            "datagrid.affordance" => &mut self.affordance,
            "datagrid.keymap" => &mut self.keymap,
            "datagrid.duplicate_keys" => &mut self.duplicate_keys,
            "datagrid.nav" => &mut self.nav,
            "datagrid.width_sample_rows" => &mut self.width_sample_rows,
            "datagrid.seams" => &mut self.seams,
            "datagrid.track_click" => &mut self.track_click,
            "datagrid.horizontal" => &mut self.horizontal,
            "datagrid.validator" => &mut self.validator,
            "datagrid.blur_action" => &mut self.blur_action,
            _ => &mut self.malformed,
        }
    }

    fn total(&self) -> RowTotal {
        [
            RowTotal::Estimated(4_812),
            RowTotal::Exact(4_812),
            RowTotal::Unknown,
        ][self.total]
    }
}

const READ_ONLY: [Option<&str>; 4] = [
    None,
    Some("customer_summary is a view: it cannot be edited"),
    Some("customers has no primary key: rows cannot be told apart"),
    Some("Query results are read-only: open the table to edit"),
];

fn custom_keymap() -> KeyMap<GridCmd> {
    let mut map: KeyMap<GridCmd> = GridCmd::default_keymap()
        .into_iter()
        .filter(|(_, c)| {
            !matches!(
                c,
                GridCmd::ClearCell | GridCmd::InsertRow | GridCmd::ToggleDelete | GridCmd::Refresh
            )
        })
        .collect();
    map.extend([
        (KeyBinding::char('x'), GridCmd::ClearCell),
        (KeyBinding::char('o'), GridCmd::InsertRow),
        (KeyBinding::char('d'), GridCmd::ToggleDelete),
        (KeyBinding::ctrl('r'), GridCmd::Refresh),
    ]);
    map
}

fn strict(spec: &ColumnSpec, text: &str) -> Result<CellValue, String> {
    let parsed = junie_tui::widgets::grid::default_validator(spec, text)?;
    match (&*spec.name, &parsed) {
        ("seats", CellValue::Int(n)) if *n > 500 => Err("Seats above the plan limit (500)".into()),
        ("customer", CellValue::Text(s)) if s.trim().len() < 3 => {
            Err("A customer name has at least 3 letters".into())
        }
        ("active", CellValue::Bool(false)) => Err("Accounts are deactivated by support".into()),
        ("tier", CellValue::Text(s)) if s.is_empty() => {
            Err("A support tier is required: pick one".into())
        }
        _ => Ok(parsed),
    }
}

fn build(k: &Knobs) -> DataGrid {
    let cols = columns(k.validator == 2);
    let width = cols.len();
    let mut grid = DataGrid::new(ID.sub("grid"), cols)
        .editable(k.access == 0)
        .overflow_badges(k.overflow_badges == 1)
        .type_ahead(k.type_ahead > 0)
        .type_ahead_column((k.type_ahead == 2).then_some(1))
        .editor_config(EditorConfig {
            affordance: [Affordance::Focused, Affordance::Always, Affordance::Never][k.affordance],
            ..EditorConfig::default()
        })
        .nav([NavKeys::DEFAULT, NavKeys::VIM][k.nav])
        .width_sample_rows([200, 8][k.width_sample_rows])
        .resizable_columns(k.seams != 1)
        .seam_hit_w(if k.seams == 2 { 3 } else { 2 })
        .empty(match k.rows {
            2 => EmptyState::error("Could not load customers")
                .hint("connection reset by peer · r retries"),
            _ => EmptyState::new("No customers match").hint("Clear the filters to see every row"),
        });
    if k.keymap == 1 {
        grid = grid.keymap(custom_keymap());
    }
    if k.duplicate_keys == 1 {
        grid = grid.duplicate_keys(&[Key {
            code: KeyCode::Char('D'),
            mods: KeyModifiers::NONE,
        }]);
    }
    if k.labels == 1 {
        grid = grid.bar_labels(["Review SQL", "Revert", "Apply"]);
        grid.header_labels = HeaderLabels {
            sort_asc: "Sort A to Z",
            sort_desc: "Sort Z to A",
            clear_sort: "Unsort",
            autofit: "Fit Width",
            reset_widths: "Default Widths",
        };
    }
    if k.track_click > 0 {
        grid =
            grid.track_click([TrackClick::Page, TrackClick::Page, TrackClick::Jump][k.track_click]);
    }
    if k.horizontal == 1 {
        grid = grid.horizontal(HScroll {
            bar: ScrollbarVisibility::Never,
            ..HScroll::default()
        });
    }
    if k.validator >= 1 {
        grid = grid.validator(strict);
    }
    grid.read_only_reason = READ_ONLY[k.access].map(str::to_owned);
    grid.row_numbers = k.row_numbers == 1;
    grid.blur_action = [BlurAction::Commit, BlurAction::Cancel][k.blur_action];
    grid.local_sort = true;
    let mut loaded = if k.rows == 0 {
        page(0, k.total(), width)
    } else {
        GridRows {
            rows: Vec::new(),
            total: RowTotal::Exact(0),
            more: false,
        }
    };
    if k.rows == 0 && k.malformed > 0 {
        loaded.rows.insert(0, malformed_row(k.malformed, width));
    }
    match grid.try_set_rows(loaded) {
        Ok(()) => {}
        Err(e) => {
            grid.empty = EmptyState::error("Rows rejected before display").hint(&e.to_string());
        }
    }
    grid
}

fn malformed_row(kind: usize, width: usize) -> Vec<CellValue> {
    match kind {
        1 => vec![CellValue::Int(1000), CellValue::Text("Short row".into())],
        _ => {
            let mut cells = row(0);
            cells.truncate(width);
            cells.push(CellValue::Text("one cell too many".into()));
            cells
        }
    }
}

const COMMANDS: &[(GridCmd, &str)] = &[
    (GridCmd::ClearCell, "Set NULL · CellChanged"),
    (GridCmd::ToggleDelete, "Delete row · RowDeleted"),
    (GridCmd::InsertRow, "Insert row · RowInserted"),
    (GridCmd::DuplicateRow, "Duplicate row · RowInserted"),
    (GridCmd::Copy, "Copy · Copy"),
    (GridCmd::CopyWithHeader, "Copy with header · Copy"),
    (GridCmd::Commit, "Save review · CommitRequested"),
    (GridCmd::Sort, "Sort · SortRequested"),
    (GridCmd::ClearSort, "Clear sort · SortRequested"),
    (GridCmd::FilterOnCell, "Filter on cell · FilterOnCell"),
    (GridCmd::OpenFilters, "Filters · OpenFilters"),
    (GridCmd::ClearFilters, "Clear filters · ClearFilters"),
    (GridCmd::Refresh, "Reload · Refresh"),
    (
        GridCmd::Undo,
        "Undo the last draft change · Undone names its key",
    ),
    (GridCmd::Discard, "Discard · DiscardRequested"),
    (GridCmd::PreviewSql, "Preview SQL · PreviewSql"),
    (
        GridCmd::FollowReference,
        "Follow account_id · FollowReference",
    ),
    (GridCmd::SelectAll, "Mark every row"),
];

const FIXED: &[(&str, &str)] = &[
    (
        "Enter / F2",
        "Edit · OpenViewer on JSON · ReadOnly says why not",
    ),
    ("Enter (read-only)", "Activated"),
    ("Space", "Mark the row"),
    (
        "Tab at the last cell",
        "LeaveForward · Shift+Tab LeaveBackward",
    ),
    ("past the last row", "FetchMore"),
];

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let cols = || {
        vec![
            ColumnSpec::new("id", CellKind::Id).primary().read_only(),
            ColumnSpec::new("customer", CellKind::Text),
            ColumnSpec::new("seats", CellKind::Number),
        ]
    };
    let rows = || GridRows {
        rows: (0..2)
            .map(|i| {
                let r = row(i);
                vec![r[0].clone(), r[1].clone(), r[3].clone()]
            })
            .collect(),
        total: RowTotal::Exact(3),
        more: false,
    };
    let mut clean = DataGrid::new(id.child(0), cols());
    clean
        .try_set_rows(GridRows {
            rows: (0..3)
                .map(|i| {
                    let r = row(i);
                    vec![r[0].clone(), r[1].clone(), r[3].clone()]
                })
                .collect(),
            total: RowTotal::Exact(3),
            more: false,
        })
        .expect("rectangular fixture");
    let mut changed = DataGrid::new(id.child(1), cols());
    changed.try_set_rows(rows()).expect("rectangular fixture");
    changed
        .record_cell(0, 2, CellValue::Int(40))
        .expect("a fresh grid is never being saved");
    changed
        .record_cell(1, 2, CellValue::Int(7))
        .expect("a fresh grid is never being saved");
    changed
        .toggle_delete(1)
        .expect("a fresh grid is never being saved");
    changed
        .insert_row()
        .expect("a fresh grid is never being saved");
    let mut read_only = DataGrid::new(id.child(2), cols()).editable(false);
    read_only.read_only_reason = READ_ONLY[1].map(str::to_owned);
    read_only.try_set_rows(rows()).expect("rectangular fixture");
    StateGrid::new(id)
        .rows(&[
            Rest,
            Hover,
            Pressed,
            Focused,
            FocusedHover,
            Selected,
            Editing,
            Error,
            Empty,
            Loading,
            Overflow,
            Narrow,
        ])
        .column_width(56)
        .narrow_width(30)
        .column("Editable", clean)
        .column("Pending changes", changed)
        .column("Read-only (view)", read_only)
}

/// A save the page captured when it was reviewed. The delayed completion and
/// its limit check read this snapshot, never the grid's live draft.
struct DemoCommit {
    snapshot: GridChangeSet,
    token: GridCommitToken,
    stage: SaveStage,
}

enum SaveStage {
    Reviewing,
    Submitted { ticks_left: u8 },
}

pub struct GridPage {
    template: Template,
    knobs: Knobs,
    pub grid: DataGrid,

    filters: ChipBar,

    filter_values: Vec<(usize, CellValue)>,
    sections: crate::pages::layout::Sections,
    commit: Option<DemoCommit>,
    saved: u32,
}

impl GridPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let Some(r) = self.grid.route(ev) else {
            return Outcome::Ignored;
        };
        let (o, actions) = r.into_parts();
        if ev.is(Phase::Click) {
            if self.grid.bar_ids().contains(&ev.id) {
                cx.focus.focus(ev.id);
            } else if ev.bar.is_none() {
                cx.focus.focus(self.grid.id);
            }
        }
        for gev in actions {
            self.on_event(Some(gev), cx);
        }
        o
    }

    pub fn new() -> Self {
        let knobs = Knobs::defaults();
        let mut filters = ChipBar::new(ID.sub("filters"));
        filters.lead = Some("Filters".into());
        Self {
            template: Template::new(ID, states_grid()),
            grid: build(&knobs),
            knobs,
            filters,
            filter_values: Vec::new(),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
            commit: None,
            saved: 0,
        }
    }

    /// SQL projection of one captured change set. The snapshot owns the rows:
    /// an insert lists every cell that is not `DEFAULT`, an update matches the
    /// base row, and nothing here reads the draft.
    fn statements(&self, changes: &GridChangeSet) -> Result<Vec<String>, NonFinite> {
        let mut out = Vec::new();
        for row in changes.updated() {
            self.update_statements(row, &mut out)?;
        }
        for row in changes.inserted() {
            out.push(self.insert_statement(row)?);
        }
        for row in changes.deleted() {
            out.push(delete_statement(row));
        }
        Ok(out)
    }

    fn row_of(&self, key: ItemKey) -> usize {
        self.grid.source_index(key).unwrap_or_default()
    }

    fn update_statements(&self, row: &UpdatedRow, out: &mut Vec<String>) -> Result<(), NonFinite> {
        for change in row.cells() {
            let name = &self.grid.columns()[change.column()].name;
            out.push(format!(
                "UPDATE {TABLE} SET {name} = {} WHERE id = {};",
                literal(self.row_of(row.key()), name, change.after())?,
                row.before()[0].text()
            ));
        }
        Ok(())
    }

    fn insert_statement(&self, row: &InsertedRow) -> Result<String, NonFinite> {
        let mut names = Vec::new();
        let mut values = Vec::new();
        for (col, value) in self.grid.columns().iter().zip(row.values()) {
            if matches!(value, CellValue::Default) {
                continue;
            }
            names.push(col.name.as_str());
            values.push(literal(self.row_of(row.key()), &col.name, value)?);
        }
        Ok(if names.is_empty() {
            format!("INSERT INTO {TABLE} DEFAULT VALUES;")
        } else {
            format!(
                "INSERT INTO {TABLE} ({}) VALUES ({});",
                names.join(", "),
                values.join(", ")
            )
        })
    }

    fn changes_dialog(&self, changes: &GridChangeSet, review: bool) -> Result<Dialog, NonFinite> {
        let code = self.statements(changes)?;
        let (changed, inserted, deleted) = changes.counts();
        let facts = vec![
            Prop::new("Statements", code.len().to_string()),
            Prop::new(
                "Rows",
                format!("{changed} changed · {inserted} inserted · {deleted} deleted"),
            ),
            Prop::new("Target", TABLE),
        ];
        Ok(if review {
            Dialog::facts(
                REVIEW,
                "Save changes",
                facts,
                code,
                Some(TABLE),
                Button::primary(REVIEW.sub("ok"), "Save"),
            )
        } else {
            Dialog::facts(
                PREVIEW,
                "Pending changes",
                facts,
                code,
                None,
                Button::primary(PREVIEW.sub("ok"), "Copy SQL"),
            )
        })
    }

    /// Captures the draft once and renders the preview from that capture.
    fn open_preview(&self, cx: &mut Cx) {
        match self.changes_dialog(&self.grid.snapshot_changes(), false) {
            Ok(dialog) => cx.open_dialog(dialog),
            Err(e) => cx.status_in(e.message(), Tone::Error),
        }
    }

    /// Captures the draft once, locks the grid on that capture and renders the
    /// review from it. The save that follows commits exactly what was shown.
    fn begin_review(&mut self, cx: &mut Cx) {
        if self.commit.is_some() {
            cx.status("A save is already in progress");
            return;
        }
        let snapshot = self.grid.snapshot_changes();
        let dialog = match self.changes_dialog(&snapshot, true) {
            Ok(dialog) => dialog,
            Err(e) => return cx.status_in(e.message(), Tone::Error),
        };
        match self.grid.begin_submission(snapshot.clone()) {
            Ok(token) => {
                self.commit = Some(DemoCommit {
                    snapshot,
                    token,
                    stage: SaveStage::Reviewing,
                });
                cx.open_dialog(dialog);
            }
            Err(e) => cx.status(e.to_string()),
        }
    }

    /// Releases the lock and forgets the save, leaving the draft as it was.
    fn cancel_commit(&mut self) {
        if let Some(commit) = self.commit.take() {
            let _ = self.grid.cancel_submission(commit.token);
            self.grid.set_loading(false);
        }
    }

    fn set_filters(&mut self) {
        self.grid.filtered_cols = self.filter_values.iter().map(|(c, _)| *c).collect();
        self.filters.chips = self
            .filter_values
            .iter()
            .map(|(c, v)| Chip::new(&format!("{} = {}", self.grid.columns()[*c].name, v.text())))
            .collect();
    }

    /// Resolves a live cell editor before the changes dialog reads the queue.
    /// A rejected value keeps the editor open and stops the save.
    fn settle_edit(&mut self, cx: &mut Cx) -> bool {
        match self.grid.finish_edit_session() {
            Ok(FinishEdit::NoSession) => true,
            Ok(FinishEdit::Accepted { result, .. }) => {
                if result == CellEditResult::Unchanged {
                    cx.status("The open cell kept its value");
                }
                true
            }
            Err(e) => {
                cx.status(match e {
                    GridEditError::Validation(_) | GridEditError::NoChoice => {
                        format!("Fix the open cell first: {e}")
                    }
                    GridEditError::Busy | GridEditError::InvalidTarget => e.to_string(),
                });
                false
            }
        }
    }

    /// A rejected toggle or clear leaves a `!` on its cell; the status line
    /// says why while the cursor is on it.
    fn explain_cell_error(&self, cx: &mut Cx) {
        let (display, col) = self.grid.cursor;
        let src = self.grid.source_row(display);
        if let Some(message) = self.grid.cell_errors.get(&(src, col)) {
            cx.status(message.clone());
        }
    }

    /// Pasting onto a cell that is not being edited sets its value through the
    /// same parser the editors use.
    fn paste_into_cell(&mut self, text: &str, cx: &mut Cx) -> Outcome {
        let (display, col) = self.grid.cursor;
        let src = self.grid.source_row(display);
        match self.grid.try_edit_text(src, col, text) {
            Ok(CellEditResult::Changed) => {
                cx.status(format!("{} pending", self.grid.pending().total()));
            }
            Ok(CellEditResult::Unchanged) => cx.status("The cell already has that value"),
            Err(GridEditError::Validation(e)) => {
                let message = e.message().to_owned();
                cx.status(format!("Paste rejected: {message}"));
                self.grid.cell_errors.insert((src, col), message);
            }
            Err(e) => cx.status(e.to_string()),
        }
        Outcome::Changed
    }

    fn on_event(&mut self, ev: Option<GridAction>, cx: &mut Cx) {
        match ev {
            Some(GridAction::CellChanged { .. }) => {
                cx.status(format!("{} pending", self.grid.pending().total()))
            }
            Some(GridAction::RowInserted(_)) => cx.status("Row inserted · fill it in, then Save"),
            Some(GridAction::RowDeleted(_)) => cx.status("Row queued for deletion · u undoes"),
            Some(GridAction::Undone(undone)) => cx.status(undo_status(undone.kind)),
            Some(GridAction::FetchMore) => {
                let from = self.grid.len();
                match self.grid.try_append_rows(page(
                    from,
                    self.knobs.total(),
                    self.grid.columns().len(),
                )) {
                    Ok(()) => cx.status(format!("Fetched rows {}–{}", from + 1, self.grid.len())),
                    Err(e) => cx.status(format!("Fetch rejected · {e}")),
                }
            }
            Some(GridAction::Refresh) => {
                match self
                    .grid
                    .try_set_rows(page(0, self.knobs.total(), self.grid.columns().len()))
                {
                    Ok(()) => cx.status("Reloaded from the source"),
                    Err(e) => cx.status(format!("Reload rejected · {e}")),
                }
            }
            Some(GridAction::CommitRequested) => {
                if !self.settle_edit(cx) {
                    return;
                }
                if self.grid.pending().is_empty() {
                    cx.status("Nothing to save");
                } else {
                    self.begin_review(cx);
                }
            }
            Some(GridAction::DiscardRequested) => match self.grid.discard() {
                Ok(_) => cx.status("Changes discarded"),
                Err(e) => cx.status(e.to_string()),
            },
            Some(GridAction::PreviewSql) => {
                if self.settle_edit(cx) {
                    self.open_preview(cx);
                }
            }
            Some(GridAction::Copy(s)) => cx.status(format!("Copied {} chars", s.len())),
            Some(GridAction::FollowReference { row, col }) => cx.status(format!(
                "Follow {} = {} into {}",
                self.grid.columns()[col].name,
                self.grid.value(row, col).text(),
                self.grid.columns()[col]
                    .references
                    .as_deref()
                    .unwrap_or("?")
            )),
            Some(GridAction::OpenViewer { row, col }) => cx.status(format!(
                "Would open the viewer for {} on row {}",
                self.grid.columns()[col].name,
                row + 1
            )),
            Some(GridAction::FilterOnCell { col, value }) => {
                self.filter_values.retain(|(c, _)| *c != col);
                self.filter_values.push((col, value));
                self.set_filters();
                cx.status("Filter added · F clears the filters");
            }
            Some(GridAction::OpenFilters) => cx.status("The filter editor belongs to the app"),
            Some(GridAction::ClearFilters) => {
                if self.filter_values.is_empty() {
                    cx.status("No filters to clear");
                } else {
                    self.filter_values.clear();
                    self.set_filters();
                    cx.status("Filters cleared");
                }
            }
            Some(GridAction::Activated(r)) => cx.status(format!("Row {} activated", r + 1)),
            Some(GridAction::LeaveForward) => cx.focus_next(),
            Some(GridAction::LeaveBackward) => cx.focus_prev(),
            Some(GridAction::ReadOnly { reason }) => cx.status(reason),
            Some(GridAction::SortRequested(_)) | None => {}
        }
    }

    /// Completes the submitted save from its captured snapshot. The plan limit
    /// is checked on the snapshot's rows, so cells edited since the review (the
    /// grid refuses that) or a rebuilt grid can never change the outcome.
    fn finish_commit(&mut self, commit: DemoCommit, cx: &mut Cx) {
        self.grid.set_loading(false);
        let DemoCommit {
            snapshot, token, ..
        } = commit;
        let over_limit = |value: &CellValue| matches!(value, CellValue::Int(n) if *n > 500);
        let bad = snapshot
            .updated()
            .iter()
            .find(|row| {
                row.cells()
                    .iter()
                    .any(|c| c.column() == SEATS_COLUMN && over_limit(c.after()))
            })
            .map(|row| row.key())
            .or_else(|| {
                snapshot
                    .inserted()
                    .iter()
                    .find(|row| row.values().get(SEATS_COLUMN).is_some_and(over_limit))
                    .map(|row| row.key())
            });
        let (n, outcome) = match bad {
            Some(key) => (
                0,
                Err(GridCommitFailure {
                    key: Some(key),
                    message: "seats above the plan limit (500)".into(),
                }),
            ),
            None => {
                let (u, i, d) = snapshot.counts();
                (u + i + d, Ok(snapshot.identity_receipt()))
            }
        };
        let failed = outcome.is_err();
        match self.grid.complete_submission(token, outcome) {
            Ok(()) if failed => cx.status("Save failed · the row is marked"),
            Ok(()) => {
                self.saved += n as u32;
                cx.status(format!("Saved {n} changes"));
            }
            Err(_) => cx.status("The save was cancelled · changes kept pending"),
        }
    }

    fn schema(&self) -> Vec<Prop> {
        self.grid
            .columns()
            .iter()
            .map(|c| {
                let mut v = vec![format!("{:?}", c.kind), c.type_label.clone()];
                if c.primary {
                    v.push("primary key".into());
                }
                if !c.nullable {
                    v.push("not null".into());
                }
                if c.read_only {
                    v.push("read-only".into());
                }
                if let Some(r) = &c.references {
                    v.push(format!("references {r}"));
                }
                if !c.enum_values.is_empty() {
                    v.push(format!("one of {}", c.enum_values.join(", ")));
                }
                if !matches!(c.editor, ColumnEditor::Text) {
                    v.push("▾ picks from a fixed set".into());
                }
                Prop::new(&c.name, v.join(" · "))
            })
            .collect()
    }

    fn commands(&self) -> Vec<Prop> {
        let mut out: Vec<Prop> = COMMANDS
            .iter()
            .map(|(cmd, what)| {
                let keys = label_for(&self.grid.keymap, *cmd).unwrap_or_else(|| "—".into());
                Prop::new(&keys, *what)
            })
            .collect();
        out.extend(FIXED.iter().map(|(k, w)| Prop::new(k, *w)));
        out
    }

    fn heights(&self, width: u16) -> [u16; 4] {
        let wrap = |v: Vec<Prop>| -> Vec<Prop> { v.into_iter().map(Prop::wrap).collect() };
        [
            GRID_H,
            3,
            crate::pages::layout::sheet_height(
                &Panel::card(Some("Schema")),
                &wrap(self.schema()),
                width,
            ),
            crate::pages::layout::sheet_height(
                &Panel::card(Some("Commands")),
                &self.commands(),
                width,
            ),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focus = if ctx.interaction.has_focus(self.grid.id)
            || self
                .grid
                .bar_ids()
                .iter()
                .any(|b| ctx.interaction.has_focus(*b))
        {
            Some(0)
        } else {
            None
        };
        let schema = self.schema();
        let commands = self.commands();
        let filters_h = u16::from(!self.filter_values.is_empty());
        let heights = self.heights(area.width);
        let l = self
            .sections
            .layout(area, &heights.map(StackChild::fixed), focus, buf, ctx);
        match l.slot(0) {
            Some(card) => {
                let focused = ctx.interaction.focused(self.grid.id)
                    || self
                        .grid
                        .bar_ids()
                        .iter()
                        .any(|b| ctx.interaction.focused(*b));
                let panel = Panel::card(Some(TABLE)).focused(focused);
                let bg = panel.bg(t);
                let inner = panel.render(card, buf, t);
                if filters_h > 0 {
                    let r = Rect::new(inner.x, inner.y, inner.width, 1);
                    self.filters.render(r, buf, ctx, bg);
                }
                let body = Rect::new(
                    inner.x,
                    inner.y + filters_h,
                    inner.width,
                    inner.height.saturating_sub(filters_h),
                );
                self.grid.render(body, buf, ctx, bg);
                let meta = match (self.grid.edit_error(), &self.grid.read_only_reason) {
                    (Some(e), _) => e.to_owned(),
                    (None, Some(_)) if !self.grid.editable => {
                        format!("read-only · {}", self.grid.position_label())
                    }
                    _ => self.grid.position_label(),
                };
                panel.draw_meta(card, buf, t, &meta);
            }
            None => ctx.focus_stop(self.grid.id),
        }
        if let Some(r) = l.slot(1) {
            let marks = [
                (RowState::Modified, "modified"),
                (RowState::Inserted, "inserted"),
                (RowState::Deleted, "deleted"),
                (RowState::Error, "error"),
            ];
            let legend = marks
                .iter()
                .map(|(s, name)| format!("{} {name}", t.change_glyph(*s).0))
                .collect::<Vec<_>>()
                .join("   ");
            let pending = self
                .grid
                .pending_label()
                .unwrap_or_else(|| "no pending changes".into());
            let props = vec![
                Prop::new("Change marks", legend),
                Prop::new("Pending", pending),
            ];
            props::render(
                Rect::new(r.x + 2, r.y, r.width.saturating_sub(4), 2),
                buf,
                t,
                &props,
                t.canvas,
            );
        }
        let wrap = |v: Vec<Prop>| -> Vec<Prop> { v.into_iter().map(Prop::wrap).collect() };
        for (slot, title, props) in [(2, "Schema", wrap(schema)), (3, "Commands", commands)] {
            if let Some(r) = l.slot(slot) {
                let panel = Panel::card(Some(title));
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                props::render(inner, buf, t, &props, bg);
            }
        }
    }
}

const GRID_H: u16 = 26;

impl Page for GridPage {
    fn title(&self) -> &'static str {
        "Data grid"
    }
    fn blurb(&self) -> &'static str {
        "Typed cells, a pending-change queue, paging and local sort"
    }
    fn leave_stakes(&self) -> junie_tui::ui::leave::Stakes {
        if !self.grid.pending().is_empty() {
            junie_tui::ui::leave::Stakes::Unsaved { changes: 0 }
        } else {
            junie_tui::ui::leave::Stakes::Clean
        }
    }

    fn editing(&self) -> bool {
        self.grid.is_editing()
    }
    fn animating(&self) -> bool {
        matches!(
            self.commit,
            Some(DemoCommit {
                stage: SaveStage::Submitted { .. },
                ..
            })
        )
    }

    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        use junie_tui::ui::measure::Measure;
        let wrap = |v: Vec<Prop>| -> Vec<Prop> { v.into_iter().map(Prop::wrap).collect() };

        let meta = format!("read-only · {}", self.grid.position_label());
        let card = Panel::card(Some(TABLE))
            .meta(&meta)
            .outer(ratatui::layout::Size::new(0, 0))
            .width;
        let w = [
            card,
            self.grid.min_width(ratatui::layout::Size::new(0, 0), ctx) + 4,
            crate::pages::layout::sheet_width(&Panel::card(Some("Schema")), &wrap(self.schema())),
            crate::pages::layout::sheet_width(&Panel::card(Some("Commands")), &self.commands()),
        ]
        .into_iter()
        .fold(avail.width, u16::max);
        let h = self.heights(w).iter().sum::<u16>() + 3;
        ratatui::layout::Size::new(w, h)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        KEYS.contains(&key).then(|| *self.knobs.clone().slot(key))
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if !KEYS.contains(&key) {
            return;
        }
        let n = crate::options::find(key).map_or(1, |o| o.values.len());
        *self.knobs.slot(key) = value.min(n - 1);
        self.cancel_commit();
        self.grid = build(&self.knobs);
        self.filter_values.clear();
        self.set_filters();
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if let PageEvent::Blur(ch) = ev {
            if ch.from != self.grid.id {
                return Outcome::Ignored;
            }
            let (out, actions) = self.grid.on_blur().into_parts();
            for gev in actions {
                self.on_event(Some(gev), cx);
            }
            return out;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Tick => {
                let Some(DemoCommit {
                    stage: SaveStage::Submitted { ticks_left },
                    ..
                }) = self.commit.as_mut()
                else {
                    return Outcome::Ignored;
                };
                *ticks_left = ticks_left.saturating_sub(1);
                if *ticks_left == 0
                    && let Some(commit) = self.commit.take()
                {
                    self.finish_commit(commit, cx);
                }
                Outcome::Changed
            }
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.grid.id {
                    let (o, actions) = self.grid.on_key(key).into_parts();
                    for ev in actions {
                        self.on_event(Some(ev), cx);
                    }
                    self.explain_cell_error(cx);
                    return o;
                }
                if self.grid.bar_ids().contains(&f) {
                    let (o, ev) = self.grid.on_bar_key(f, key);
                    self.on_event(ev, cx);
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Paste(text) => match self.grid.on_paste(text) {
                Outcome::Ignored if cx.focus.is(self.grid.id) => self.paste_into_cell(text, cx),
                o => o,
            },
            PageEvent::Pointer(p) => self.pointer(p, cx),
            PageEvent::DialogClosed { id, result, .. }
                if *id == PREVIEW && *result == DialogResult::Action(1) =>
            {
                let copied = self
                    .statements(&self.grid.snapshot_changes())
                    .map_or(0, |s| s.len());
                cx.status(format!("Copied {copied} statements"));
                Outcome::Changed
            }
            PageEvent::DialogClosed { id, result, .. } if *id == REVIEW => {
                match self.commit.as_mut() {
                    Some(commit)
                        if *result == DialogResult::Action(1)
                            && matches!(commit.stage, SaveStage::Reviewing) =>
                    {
                        commit.stage = SaveStage::Submitted { ticks_left: 4 };
                        self.grid.set_loading(true);
                        cx.status("Saving…");
                    }
                    _ => {
                        self.cancel_commit();
                        cx.status("Changes kept pending");
                    }
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.grid.submission_pending() {
            return vec![("↑↓←→", "Cell"), ("Saving…", "edits are locked")];
        }
        if self.grid.is_editing() {
            return vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next cell")];
        }
        if focus.is_some_and(|f| self.grid.bar_ids().contains(&f)) {
            return vec![("Enter", "Activate"), ("Tab", "Next")];
        }
        let mut h = vec![("↑↓←→", "Cell")];

        if self.grid.cell_editable() {
            h.push(junie_tui::widgets::widget_hints::EDIT_PAIR);
        }

        let key = |cmd| hint_key(&self.grid.keymap, cmd);
        if let Some(k) = key(GridCmd::Sort) {
            h.push((k, "Sort"));
        }
        h.push(("Space", "Select row"));
        if self.grid.editable {
            for (cmd, what) in [
                (GridCmd::InsertRow, "Insert"),
                (GridCmd::ToggleDelete, "Delete"),
                (GridCmd::Undo, "Undo"),
            ] {
                if let Some(k) = key(cmd) {
                    h.push((k, what));
                }
            }
        }
        h
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::page_text;

    fn with_malformed(value: usize) -> String {
        let mut page = GridPage::new();
        page.set_option("datagrid.malformed", value);
        page_text(&mut page, 120, 40)
    }

    #[test]
    fn issue42_malformed_input_option_is_off_by_default() {
        let page = GridPage::new();
        assert_eq!(page.option_value("datagrid.malformed"), Some(0));
        let text = page_text(&mut GridPage::new(), 120, 40);
        assert!(text.contains("Northwind"), "default rows load");
        assert!(
            !text.contains("rejected"),
            "no rejection without the option"
        );
    }

    #[test]
    fn issue42_malformed_input_option_is_listed_on_the_options_tab() {
        let listed = crate::options::page_options(crate::app::PageId::Grid);
        let opt = listed
            .iter()
            .find(|o| o.key == "datagrid.malformed")
            .expect("the Options tab lists it");
        assert_eq!(opt.index_of("long"), Some(2));
    }

    #[test]
    fn issue42_long_row_shows_the_data_error() {
        let text = with_malformed(2);
        assert!(text.contains("Rows rejected before display"), "{text}");
        assert!(text.contains("row 0 has 10 cells, expected 9"), "{text}");
        assert!(!text.contains("Northwind"), "the whole batch is refused");
    }

    #[test]
    fn issue42_short_row_is_padded_with_null() {
        let text = with_malformed(1);
        assert!(text.contains("Short row"), "{text}");
        assert!(text.contains("NULL"), "padded cells render as NULL: {text}");
        assert!(!text.contains("rejected"), "{text}");
    }

    const MRR: usize = 4;
    const SEATS: usize = 3;

    struct Rig {
        page: GridPage,
        focus: junie_tui::core::focus::Focus,
        ring: junie_tui::core::focus::FocusRing,
        statuses: Vec<String>,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                page: GridPage::new(),
                focus: junie_tui::core::focus::Focus::default(),
                ring: junie_tui::core::focus::FocusRing::default(),
                statuses: Vec::new(),
            }
        }

        fn with_cx(&mut self, f: impl FnOnce(&mut GridPage, &mut Cx)) {
            let mut cx = Cx::new(&mut self.focus, &self.ring);
            f(&mut self.page, &mut cx);
            for request in cx.take() {
                if let junie_tui::shell::Request::Status(text, _) = request {
                    self.statuses.push(text);
                }
            }
        }

        fn event(&mut self, ev: PageEvent) {
            self.with_cx(|page, cx| {
                page.handle(&ev, cx);
            });
        }

        fn save(&mut self) {
            self.with_cx(|page, cx| page.on_event(Some(GridAction::CommitRequested), cx));
        }

        fn answer_review(&mut self, result: DialogResult) {
            self.event(PageEvent::DialogClosed {
                id: REVIEW,
                result,
                value: None,
            });
        }

        fn ticks(&mut self, n: usize) {
            for _ in 0..n {
                self.event(PageEvent::Tick);
            }
        }

        fn last_status(&self) -> &str {
            self.statuses.last().map_or("", String::as_str)
        }
    }

    #[test]
    fn issue11_showcase_timer_uses_reviewed_snapshot() {
        let mut rig = Rig::new();
        let _ = rig.page.grid.record_cell(0, SEATS, CellValue::Int(40));
        rig.save();
        rig.answer_review(DialogResult::Action(1));
        assert_eq!(rig.last_status(), "Saving…");

        let _ = rig.page.grid.record_cell(1, SEATS, CellValue::Int(900));
        rig.ticks(4);

        assert_eq!(rig.last_status(), "Saved 1 changes", "{:?}", rig.statuses);
        assert!(rig.page.grid.pending().is_empty());
        assert_ne!(
            rig.page.grid.value(1, SEATS),
            &CellValue::Int(900),
            "an edit between confirm and completion never reached the save"
        );
        assert_eq!(rig.page.grid.value(0, SEATS), &CellValue::Int(40));
        assert!(rig.page.commit.is_none());
    }

    #[test]
    fn issue11_showcase_option_replacement_cancels_request() {
        let mut rig = Rig::new();
        let _ = rig.page.grid.record_cell(0, SEATS, CellValue::Int(40));
        rig.save();
        rig.answer_review(DialogResult::Action(1));
        let replaced = rig.page.grid.instance_id();

        rig.page.set_option(KEYS[0], 0);
        assert_ne!(rig.page.grid.instance_id(), replaced, "a fresh grid");
        assert!(rig.page.commit.is_none(), "the request was cancelled");
        rig.ticks(6);

        assert!(
            !rig.statuses.iter().any(|s| s.starts_with("Saved")),
            "a rebuilt grid never reports the old save: {:?}",
            rig.statuses
        );
        assert_eq!(rig.page.saved, 0);
        assert!(!rig.page.grid.submission_pending());
    }

    #[test]
    fn issue11_showcase_review_locks_and_cancel_releases() {
        let mut rig = Rig::new();
        let _ = rig.page.grid.record_cell(0, SEATS, CellValue::Int(40));
        rig.save();
        assert!(
            rig.page.grid.submission_pending(),
            "the review locks the draft"
        );
        assert_eq!(
            rig.page.grid.record_cell(1, SEATS, CellValue::Int(7)),
            Err(GridEditError::Busy)
        );
        rig.save();
        assert_eq!(rig.last_status(), "A save is already in progress");

        rig.answer_review(DialogResult::Cancelled);
        assert!(!rig.page.grid.submission_pending());
        assert_eq!(rig.page.grid.pending().total(), 1, "the draft survived");
        rig.ticks(6);
        assert_eq!(rig.page.saved, 0);
    }

    #[test]
    fn issue11_showcase_limit_check_marks_the_reviewed_row() {
        let mut rig = Rig::new();
        let _ = rig.page.grid.record_cell(0, SEATS, CellValue::Int(900));
        rig.save();
        rig.answer_review(DialogResult::Action(1));
        rig.ticks(4);

        assert_eq!(rig.last_status(), "Save failed · the row is marked");
        assert!(!rig.page.grid.submission_pending(), "a failure unlocks");
        assert_eq!(rig.page.grid.pending().total(), 1, "the draft is retained");
        assert!(rig.page.grid.row_errors.contains_key(&0));
    }

    fn sql(page: &GridPage) -> Result<Vec<String>, NonFinite> {
        page.statements(&page.grid.snapshot_changes())
    }

    #[test]
    fn issue49_showcase_preview_preserves_numeric_precision() {
        let mut page = GridPage::new();
        page.grid
            .record_cell(0, MRR, CellValue::Num(0.001))
            .unwrap();
        page.grid
            .record_cell(1, MRR, CellValue::Num(1.234_567))
            .unwrap();
        page.grid
            .record_cell(2, MRR, CellValue::Num(100.0))
            .unwrap();
        page.grid.insert_row().unwrap();
        let insert = page.grid.len() - 1;
        page.grid
            .record_cell(insert, SEATS, CellValue::Int(5))
            .unwrap();
        page.grid
            .record_cell(insert, MRR, CellValue::Num(-0.000_25))
            .unwrap();

        assert_eq!(
            sql(&page).unwrap(),
            vec![
                "UPDATE customers SET mrr = 0.001 WHERE id = 1001;".to_owned(),
                "UPDATE customers SET mrr = 1.234567 WHERE id = 1002;".to_owned(),
                "UPDATE customers SET mrr = 100.0 WHERE id = 1003;".to_owned(),
                "INSERT INTO customers (customer, plan, seats, mrr, active, renewed_at, account_id, notes) VALUES (NULL, NULL, 5, -0.00025, NULL, NULL, NULL, NULL);".to_owned(),
            ]
        );
        assert!(
            page.changes_dialog(&page.grid.snapshot_changes(), false)
                .is_ok()
        );
        assert!(
            page.changes_dialog(&page.grid.snapshot_changes(), true)
                .is_ok()
        );
    }

    #[test]
    fn issue49_showcase_preview_rejects_nonfinite_numbers_entirely() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut page = GridPage::new();
            page.grid.record_cell(0, MRR, CellValue::Num(2.5)).unwrap();
            page.grid.record_cell(1, MRR, CellValue::Num(bad)).unwrap();
            let want = NonFinite {
                row: 1,
                column: "mrr".into(),
            };
            assert_eq!(sql(&page), Err(want.clone()));
            assert_eq!(
                page.changes_dialog(&page.grid.snapshot_changes(), false)
                    .err(),
                Some(want.clone())
            );
            assert_eq!(
                page.changes_dialog(&page.grid.snapshot_changes(), true)
                    .err(),
                Some(want)
            );
            assert_eq!(page.grid.pending().cells().count(), 2, "the draft is kept");
        }
    }
    const CUSTOMER: usize = 1;
    const PLAN: usize = 2;

    fn insert_statement(page: &GridPage) -> String {
        let statements = sql(page).expect("the draft serializes");
        let inserts: Vec<&String> = statements
            .iter()
            .filter(|s| s.starts_with("INSERT"))
            .collect();
        assert_eq!(inserts.len(), 1, "{statements:?}");
        inserts[0].clone()
    }

    #[test]
    fn issue50_showcase_duplicate_review_contains_copied_values() {
        let mut page = GridPage::new();
        let source = page.grid.rows()[0].clone();
        page.grid.duplicate_row().expect("row duplicates");

        let untouched = insert_statement(&page);
        assert!(!untouched.contains("DEFAULT VALUES"), "{untouched}");
        assert!(
            untouched.starts_with("INSERT INTO customers (customer, plan, seats, "),
            "id and mrr reset to DEFAULT and are omitted: {untouched}"
        );
        assert!(
            untouched.contains(&literal(0, "customer", &source[CUSTOMER]).unwrap()),
            "{untouched}"
        );
        assert!(
            untouched.contains(&literal(0, "plan", &source[PLAN]).unwrap()),
            "{untouched}"
        );

        let copy = page.grid.len() - 1;
        page.grid
            .record_cell(copy, SEATS, CellValue::Int(99))
            .unwrap();
        let edited = insert_statement(&page);
        assert!(edited.contains(", 99,"), "{edited}");
        assert!(
            edited.contains(&literal(0, "customer", &source[CUSTOMER]).unwrap()),
            "the other copies survive one edit: {edited}"
        );
    }

    #[test]
    fn issue47_showcase_columns_reject_impossible_named_values() {
        const NOTES: usize = 8;
        const RENEWED: usize = 6;
        let mut page = GridPage::new();
        for (col, bad) in [
            (RENEWED, "2026-02-30"),
            (RENEWED, "2026-99-99"),
            (NOTES, "{not json}"),
        ] {
            let got = page.grid.try_edit_text(1, col, bad);
            assert!(
                matches!(got, Err(GridEditError::Validation(_))),
                "{bad:?}: {got:?}"
            );
        }
        assert!(page.grid.pending().is_empty());
        for (col, good) in [(RENEWED, "2026-02-28"), (NOTES, "{\"owner\": \"kai\"}")] {
            assert_eq!(
                page.grid.try_edit_text(1, col, good),
                Ok(CellEditResult::Changed)
            );
        }
    }
}
