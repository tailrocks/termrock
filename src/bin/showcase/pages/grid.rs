use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::BlurAction;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, KeyMap, NavKeys, hint_key, label_for};
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::{HScroll, ScrollbarVisibility, TrackClick};
use junie_tui::ui::columns::HeaderLabels;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::chips::{Chip, ChipBar};
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridCmd, GridRows, RowState, RowTotal,
};
use junie_tui::widgets::option_editor::{Affordance, ColumnEditor, EditorConfig, Options};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("grid");
const PREVIEW: WidgetId = ID.sub("preview");
const REVIEW: WidgetId = ID.sub("review");
const PAGE: usize = 40;
const ALL: usize = 96;

const TABLE: &str = "customers";

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
    ]
}

fn page(from: usize, total: RowTotal) -> GridRows {
    let to = (from + PAGE).min(ALL);
    GridRows {
        rows: (from..to).map(row).collect(),
        total: if to >= ALL {
            RowTotal::Exact(ALL)
        } else {
            total
        },
        more: to < ALL,
    }
}

fn literal(v: &CellValue) -> String {
    match v {
        CellValue::Null => "NULL".into(),
        CellValue::Default => "DEFAULT".into(),
        CellValue::Text(s) | CellValue::Json(s) => format!("'{}'", s.replace('\'', "''")),
        other => other.text(),
    }
}

fn columns() -> Vec<ColumnSpec> {
    vec![
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
    ]
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
}

const KEYS: [&str; 17] = [
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
            _ => &mut self.blur_action,
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
        _ => Ok(parsed),
    }
}

fn build(k: &Knobs) -> DataGrid {
    let mut grid = DataGrid::new(ID.sub("grid"), columns())
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
    if k.validator == 1 {
        grid = grid.validator(strict);
    }
    grid.read_only_reason = READ_ONLY[k.access].map(str::to_owned);
    grid.row_numbers = k.row_numbers == 1;
    grid.blur_action = [BlurAction::Commit, BlurAction::Cancel][k.blur_action];
    grid.local_sort = true;
    grid.set_rows(if k.rows == 0 {
        page(0, k.total())
    } else {
        GridRows {
            rows: Vec::new(),
            total: RowTotal::Exact(0),
            more: false,
        }
    });
    grid
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
    (GridCmd::Undo, "Undo a cell, delete or insert"),
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
    clean.set_rows(GridRows {
        rows: (0..3)
            .map(|i| {
                let r = row(i);
                vec![r[0].clone(), r[1].clone(), r[3].clone()]
            })
            .collect(),
        total: RowTotal::Exact(3),
        more: false,
    });
    let mut changed = DataGrid::new(id.child(1), cols());
    changed.set_rows(rows());
    changed.record_cell(0, 2, CellValue::Int(40));
    changed.toggle_delete(1);
    changed.insert_row();
    let mut read_only = DataGrid::new(id.child(2), cols()).editable(false);
    read_only.read_only_reason = READ_ONLY[1].map(str::to_owned);
    read_only.set_rows(rows());
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

pub struct GridPage {
    template: Template,
    knobs: Knobs,
    pub grid: DataGrid,

    filters: ChipBar,

    filter_values: Vec<(usize, CellValue)>,
    sections: crate::pages::layout::Sections,
    commit_ticks: u8,
    saved: u32,
}

impl GridPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let Some(r) = self.grid.route(ev) else {
            return Outcome::Ignored;
        };
        let (o, gev) = r.into_parts();
        if ev.is(Phase::Click) {
            if self.grid.bar_ids().contains(&ev.id) {
                cx.focus.focus(ev.id);
            } else if ev.bar.is_none() {
                cx.focus.focus(self.grid.id);
            }
        }
        self.on_event(gev, cx);
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
            commit_ticks: 0,
            saved: 0,
        }
    }

    fn statements(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut cells: Vec<_> = self.grid.pending.cells.iter().collect();
        cells.sort_by_key(|((r, c), _)| (*r, *c));
        for ((r, c), v) in cells {
            if self.grid.pending.inserted.contains(r) {
                continue;
            }
            out.push(format!(
                "UPDATE {TABLE} SET {} = {} WHERE id = {};",
                self.grid.columns[*c].name,
                literal(v),
                self.grid.value(*r, 0).text()
            ));
        }
        for r in &self.grid.pending.inserted {
            let cols: Vec<&str> = self
                .grid
                .columns
                .iter()
                .enumerate()
                .filter(|(c, _)| self.grid.pending.cells.contains_key(&(*r, *c)))
                .map(|(_, col)| col.name.as_str())
                .collect();
            if cols.is_empty() {
                out.push(format!("INSERT INTO {TABLE} DEFAULT VALUES;"));
            } else {
                let vals: Vec<String> = self
                    .grid
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(c, _)| self.grid.pending.cells.get(&(*r, c)))
                    .map(literal)
                    .collect();
                out.push(format!(
                    "INSERT INTO {TABLE} ({}) VALUES ({});",
                    cols.join(", "),
                    vals.join(", ")
                ));
            }
        }
        for r in &self.grid.pending.deleted {
            out.push(format!(
                "DELETE FROM {TABLE} WHERE id = {};",
                self.grid.value(*r, 0).text()
            ));
        }
        out
    }

    fn changes_dialog(&self, review: bool) -> Dialog {
        let code = self.statements();
        let (changed, inserted, deleted) = self.grid.pending.counts();
        let facts = vec![
            Prop::new("Statements", code.len().to_string()),
            Prop::new(
                "Rows",
                format!("{changed} changed · {inserted} inserted · {deleted} deleted"),
            ),
            Prop::new("Target", TABLE),
        ];
        if review {
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
        }
    }

    fn set_filters(&mut self) {
        self.grid.filtered_cols = self.filter_values.iter().map(|(c, _)| *c).collect();
        self.filters.chips = self
            .filter_values
            .iter()
            .map(|(c, v)| Chip::new(&format!("{} = {}", self.grid.columns[*c].name, v.text())))
            .collect();
    }

    fn on_event(&mut self, ev: Option<GridAction>, cx: &mut Cx) {
        match ev {
            Some(GridAction::CellChanged { .. }) => {
                cx.status(format!("{} pending", self.grid.pending.total()))
            }
            Some(GridAction::RowInserted(_)) => cx.status("Row inserted · fill it in, then Save"),
            Some(GridAction::RowDeleted(_)) => cx.status("Row queued for deletion · u undoes"),
            Some(GridAction::FetchMore) => {
                let from = self.grid.len();
                self.grid.append_rows(page(from, self.knobs.total()));
                cx.status(format!("Fetched rows {}–{}", from + 1, self.grid.len()));
            }
            Some(GridAction::Refresh) => {
                self.grid.set_rows(page(0, self.knobs.total()));
                cx.status("Reloaded from the source");
            }
            Some(GridAction::CommitRequested) => {
                if self.grid.pending.is_empty() {
                    cx.status("Nothing to save");
                } else {
                    cx.open_dialog(self.changes_dialog(true));
                }
            }
            Some(GridAction::DiscardRequested) => {
                self.grid.discard();
                cx.status("Changes discarded");
            }
            Some(GridAction::PreviewSql) => cx.open_dialog(self.changes_dialog(false)),
            Some(GridAction::Copy(s)) => cx.status(format!("Copied {} chars", s.len())),
            Some(GridAction::FollowReference { row, col }) => cx.status(format!(
                "Follow {} = {} into {}",
                self.grid.columns[col].name,
                self.grid.value(row, col).text(),
                self.grid.columns[col].references.as_deref().unwrap_or("?")
            )),
            Some(GridAction::OpenViewer { row, col }) => cx.status(format!(
                "Would open the viewer for {} on row {}",
                self.grid.columns[col].name,
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

    fn finish_commit(&mut self, cx: &mut Cx) {
        self.grid.set_loading(false);

        let bad = self
            .grid
            .pending
            .cells
            .iter()
            .find(|((_, c), v)| *c == 3 && matches!(v, CellValue::Int(n) if *n > 500))
            .map(|((r, _), _)| *r);
        match bad {
            Some(r) => {
                self.grid
                    .apply_commit_result(Err((r, "seats above the plan limit (500)".into())));
                cx.status("Save failed · the row is marked");
            }
            None => {
                let n = self.grid.pending.total();
                self.grid.apply_commit_result(Ok(()));
                self.saved += n as u32;
                cx.status(format!("Saved {n} changes"));
            }
        }
    }

    fn schema(&self) -> Vec<Prop> {
        self.grid
            .columns
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
                let meta = match &self.grid.read_only_reason {
                    Some(_) if !self.grid.editable => {
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
        if !self.grid.pending.is_empty() {
            junie_tui::ui::leave::Stakes::Unsaved { changes: 0 }
        } else {
            junie_tui::ui::leave::Stakes::Clean
        }
    }

    fn editing(&self) -> bool {
        self.grid.is_editing()
    }
    fn animating(&self) -> bool {
        self.commit_ticks > 0
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
            let (out, gev) = self.grid.on_blur();
            self.on_event(gev, cx);
            return out;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Tick => {
                if self.commit_ticks == 0 {
                    return Outcome::Ignored;
                }
                self.commit_ticks -= 1;
                if self.commit_ticks == 0 {
                    self.finish_commit(cx);
                }
                Outcome::Changed
            }
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.grid.id {
                    let (o, ev) = self.grid.on_key(key);
                    self.on_event(ev, cx);
                    return o;
                }
                if self.grid.bar_ids().contains(&f) {
                    let (o, ev) = self.grid.on_bar_key(f, key);
                    self.on_event(ev, cx);
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Paste(text) => self.grid.on_paste(text),
            PageEvent::Pointer(p) => self.pointer(p, cx),
            PageEvent::DialogClosed { id, result, .. }
                if *id == PREVIEW && *result == DialogResult::Action(1) =>
            {
                cx.status(format!("Copied {} statements", self.statements().len()));
                Outcome::Changed
            }
            PageEvent::DialogClosed { id, result, .. }
                if *id == REVIEW && *result == DialogResult::Action(1) =>
            {
                self.grid.set_loading(true);
                self.commit_ticks = 4;
                cx.status("Saving…");
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
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
