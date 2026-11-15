use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect, Size};
use ratatui::style::Modifier;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::keymap::hint_key;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::theme::{StatusKind, SyntaxTone, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::scroll_view::{Axes, ScrollView};
use junie_tui::ui::text::{DurationStyle, duration};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::chips::{Chip, ChipBar, ChipCmd};
use junie_tui::widgets::code::{CodeEditor, Diagnostic, EditorAction, Severity};
use junie_tui::widgets::completion::{Completion, CompletionAction};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridCmd, GridDataError, GridRows,
    RowTotal,
};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::{Panel, ScrollPanel};
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::table::{Cell, Column, DataTable, SortDir};
use junie_tui::widgets::tabs::{TabAction, TabCmd, TabItem, Tabs};
use junie_tui::widgets::tree::{TreeNode, TreeView};

use crate::app::Cx;
use crate::db::{Catalog, ColType, Table, Value};
use crate::model::{self, History, HistoryEntry, HistorySource};
use crate::sql::{self, PlanNode, Statement};
use crate::store::{Source, TableId};
use crate::workbench::WorkTab;

const ALT_D: junie_tui::core::event::Key = junie_tui::core::event::Key {
    code: KeyCode::Char('d'),
    mods: ratatui::crossterm::event::KeyModifiers::ALT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Ne,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Gt,
    Ge,
    Lt,
    Le,
    IsNull,
    IsNotNull,
    IsEmpty,
    IsNotEmpty,
    In,
    NotIn,
    Between,
    Regex,
}

impl FilterOp {
    pub const ALL: [FilterOp; 18] = [
        FilterOp::Eq,
        FilterOp::Ne,
        FilterOp::Contains,
        FilterOp::NotContains,
        FilterOp::StartsWith,
        FilterOp::EndsWith,
        FilterOp::Gt,
        FilterOp::Ge,
        FilterOp::Lt,
        FilterOp::Le,
        FilterOp::IsNull,
        FilterOp::IsNotNull,
        FilterOp::IsEmpty,
        FilterOp::IsNotEmpty,
        FilterOp::In,
        FilterOp::NotIn,
        FilterOp::Between,
        FilterOp::Regex,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FilterOp::Eq => "=",
            FilterOp::Ne => "!=",
            FilterOp::Contains => "contains",
            FilterOp::NotContains => "not contains",
            FilterOp::StartsWith => "starts with",
            FilterOp::EndsWith => "ends with",
            FilterOp::Gt => ">",
            FilterOp::Ge => ">=",
            FilterOp::Lt => "<",
            FilterOp::Le => "<=",
            FilterOp::IsNull => "is NULL",
            FilterOp::IsNotNull => "is not NULL",
            FilterOp::IsEmpty => "is empty",
            FilterOp::IsNotEmpty => "is not empty",
            FilterOp::In => "in list",
            FilterOp::NotIn => "not in list",
            FilterOp::Between => "between",
            FilterOp::Regex => "matches regex",
        }
    }
    pub fn needs_value(self) -> bool {
        !matches!(
            self,
            FilterOp::IsNull | FilterOp::IsNotNull | FilterOp::IsEmpty | FilterOp::IsNotEmpty
        )
    }

    pub fn ordered_for(ty: ColType) -> Vec<FilterOp> {
        let first: &[FilterOp] = match ty {
            ColType::Int | ColType::Numeric | ColType::Timestamp | ColType::Date => &[
                FilterOp::Eq,
                FilterOp::Ne,
                FilterOp::Gt,
                FilterOp::Ge,
                FilterOp::Lt,
                FilterOp::Le,
                FilterOp::Between,
                FilterOp::IsNull,
                FilterOp::IsNotNull,
            ],
            ColType::Bool => &[FilterOp::Eq, FilterOp::IsNull, FilterOp::IsNotNull],
            ColType::Enum => &[
                FilterOp::Eq,
                FilterOp::Ne,
                FilterOp::In,
                FilterOp::NotIn,
                FilterOp::IsNull,
                FilterOp::IsNotNull,
            ],
            ColType::Json => &[
                FilterOp::IsNull,
                FilterOp::IsNotNull,
                FilterOp::IsEmpty,
                FilterOp::Contains,
            ],
            _ => &[
                FilterOp::Eq,
                FilterOp::Ne,
                FilterOp::Contains,
                FilterOp::StartsWith,
                FilterOp::EndsWith,
                FilterOp::IsNull,
                FilterOp::IsNotNull,
                FilterOp::IsEmpty,
            ],
        };
        let mut out: Vec<FilterOp> = first.to_vec();
        for op in FilterOp::ALL {
            if !out.contains(&op) {
                out.push(op);
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
    pub value2: String,
    pub enabled: bool,
}

impl Filter {
    pub fn chip_label(&self) -> String {
        match self.op {
            FilterOp::Between => {
                format!("{} between {} and {}", self.column, self.value, self.value2)
            }
            op if !op.needs_value() => format!("{} {}", self.column, op.label()),
            op => format!("{} {} {}", self.column, op.label(), quote(&self.value)),
        }
    }

    /// The one expression that evaluates this filter and renders its SQL.
    pub fn expression(&self) -> sql::Expr {
        use sql::{Expr, PredicateOp as P};
        let leaf = |op: P| {
            Expr::Leaf(sql::Predicate {
                column: self.column.clone(),
                op,
            })
        };
        let v = self.value.clone();
        let list = || self.value.split(',').map(|s| s.trim().to_owned()).collect();
        match self.op {
            FilterOp::Eq => leaf(P::Eq(v)),
            FilterOp::Ne => leaf(P::Ne(v)),
            FilterOp::Contains => leaf(P::Contains(v)),
            FilterOp::NotContains => Expr::Not(Box::new(leaf(P::Contains(v)))),
            FilterOp::StartsWith => leaf(P::StartsWith(v)),
            FilterOp::EndsWith => leaf(P::EndsWith(v)),
            FilterOp::Gt => leaf(P::Gt(v)),
            FilterOp::Ge => leaf(P::Ge(v)),
            FilterOp::Lt => leaf(P::Lt(v)),
            FilterOp::Le => leaf(P::Le(v)),
            FilterOp::IsNull => leaf(P::IsNull),
            FilterOp::IsNotNull => leaf(P::IsNotNull),
            FilterOp::IsEmpty => leaf(P::Eq(String::new())),
            FilterOp::IsNotEmpty => leaf(P::Ne(String::new())),
            FilterOp::In => leaf(P::In(list())),
            FilterOp::NotIn => Expr::Not(Box::new(leaf(P::In(list())))),
            FilterOp::Between => Expr::All(vec![leaf(P::Ge(v)), leaf(P::Le(self.value2.clone()))]),
            FilterOp::Regex => leaf(P::Regex(v)),
        }
    }

    pub fn to_sql(&self) -> String {
        self.expression().to_sql()
    }

    /// Rules the editor enforces before a filter can replace the current view.
    /// Whatever passes here evaluates on every row without a per-row failure.
    pub fn check(&self, ty: ColType) -> Result<(), FilterError> {
        let fail = |field, message: &str| {
            Err(FilterError {
                field,
                message: message.into(),
            })
        };
        if self.op.needs_value() && self.value.is_empty() {
            return fail(FilterField::Value, "A value is required for this operator");
        }
        if self.op == FilterOp::Between && self.value2.is_empty() {
            return fail(FilterField::Value2, "A value is required for this operator");
        }
        let is_list = matches!(self.op, FilterOp::In | FilterOp::NotIn);
        if is_list && self.value.split(',').any(|s| s.trim().is_empty()) {
            return fail(FilterField::Value, "List items cannot be empty");
        }
        if self.op == FilterOp::Regex {
            sql::compile_regex(&self.value).map_err(|message| FilterError {
                field: FilterField::Value,
                message,
            })?;
        }
        let numeric = matches!(ty, ColType::Int | ColType::Numeric);
        let ordinal = matches!(
            self.op,
            FilterOp::Eq
                | FilterOp::Ne
                | FilterOp::Gt
                | FilterOp::Ge
                | FilterOp::Lt
                | FilterOp::Le
                | FilterOp::In
                | FilterOp::NotIn
                | FilterOp::Between
        );
        type Parses = fn(&str) -> bool;
        let typed: Option<(&str, Parses)> = match ty {
            ColType::Int => Some(("Enter a whole number", |t| t.parse::<i64>().is_ok())),
            ColType::Numeric => Some(("Enter a number", |t| {
                t.parse::<f64>().is_ok_and(f64::is_finite)
            })),
            ColType::Bool => Some(("Enter true or false", |t| {
                t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false")
            })),
            _ => None,
        };
        if let (true, Some((msg, parses))) = (ordinal, typed) {
            let operands: Vec<(FilterField, &str)> = match self.op {
                FilterOp::In | FilterOp::NotIn => self
                    .value
                    .split(',')
                    .map(|t| (FilterField::Value, t.trim()))
                    .collect(),
                FilterOp::Between => vec![
                    (FilterField::Value, self.value.as_str()),
                    (FilterField::Value2, self.value2.as_str()),
                ],
                _ => vec![(FilterField::Value, self.value.as_str())],
            };
            for (field, text) in operands {
                if !parses(text) {
                    return fail(field, msg);
                }
            }
        }
        if self.op == FilterOp::Between {
            let ordered = if ty == ColType::Int {
                matches!(
                    (self.value.parse::<i64>(), self.value2.parse::<i64>()),
                    (Ok(lo), Ok(hi)) if lo <= hi
                )
            } else if numeric {
                let lo = self.value.parse::<f64>().unwrap_or(f64::NAN);
                let hi = self.value2.parse::<f64>().unwrap_or(f64::NAN);
                lo <= hi
            } else {
                self.value.to_lowercase() <= self.value2.to_lowercase()
            };
            if !ordered {
                return fail(
                    FilterField::Value2,
                    "The upper bound is below the lower bound",
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterField {
    Value,
    Value2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterError {
    pub field: FilterField,
    pub message: String,
}

fn quote(v: &str) -> String {
    if v.parse::<f64>().is_ok() || v == "true" || v == "false" {
        v.to_owned()
    } else {
        format!("'{v}'")
    }
}

pub fn cell_kind(ty: ColType) -> CellKind {
    match ty {
        ColType::Uuid => CellKind::Id,
        ColType::Text => CellKind::Text,
        ColType::Int | ColType::Numeric => CellKind::Number,
        ColType::Bool => CellKind::Bool,
        ColType::Timestamp | ColType::Date => CellKind::Timestamp,
        ColType::Json => CellKind::Json,
        ColType::Enum => CellKind::Enum,
    }
}

pub fn to_cell(v: &Value) -> CellValue {
    match v {
        Value::Null => CellValue::Null,
        Value::Text(s) => CellValue::Text(s.clone()),
        Value::Int(i) => CellValue::Int(*i),
        Value::Num(n) => CellValue::Num(*n),
        Value::Bool(b) => CellValue::Bool(*b),
        Value::Json(j) => CellValue::Json(j.clone()),
    }
}

pub fn from_cell(v: &CellValue) -> Value {
    match v {
        CellValue::Null | CellValue::Default => Value::Null,
        CellValue::Text(s) => Value::Text(s.clone()),
        CellValue::Int(i) => Value::Int(*i),
        CellValue::Num(n) => Value::Num(*n),
        CellValue::Bool(b) => Value::Bool(*b),
        CellValue::Json(j) => Value::Json(j.clone()),
    }
}

fn column_specs(table: &Table, columns: &[(String, ColType)]) -> Vec<ColumnSpec> {
    columns
        .iter()
        .map(|(name, ty)| {
            let col = table.column(name);
            let mut spec = ColumnSpec::new(name, cell_kind(*ty))
                .nullable(col.is_none_or(|c| c.nullable))
                .type_label(ty.sql());
            if col.is_some_and(|c| c.primary) {
                spec = spec.primary();
            }
            if col.is_some_and(|c| c.generated) {
                spec = spec.read_only();
            }
            if let Some(r) = col.and_then(|c| c.references.as_ref()) {
                spec = spec.references(&r.0);
            }
            if let Some(c) = col
                && !c.enum_values.is_empty()
            {
                spec = spec.enum_values(&c.enum_values);
            }
            spec
        })
        .collect()
}

pub fn highlight_sql(src: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    sql::tokenize(src)
        .into_iter()
        .filter_map(|t| {
            let tone = match t.kind {
                sql::TokKind::Keyword => SyntaxTone::Keyword,
                sql::TokKind::Ident => SyntaxTone::Ident,
                sql::TokKind::Number => SyntaxTone::Number,
                sql::TokKind::Literal => SyntaxTone::Literal,
                sql::TokKind::String => SyntaxTone::Str,
                sql::TokKind::Operator => SyntaxTone::Operator,
                sql::TokKind::Punct => SyntaxTone::Punct,
                sql::TokKind::Comment => SyntaxTone::Comment,
                sql::TokKind::Whitespace => return None,
            };
            Some((t.start..t.end, tone))
        })
        .collect()
}

pub fn segment_sql(src: &str) -> Vec<Range<usize>> {
    sql::split_statements(src)
        .into_iter()
        .map(|(a, b)| a..b)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableMode {
    Data,
    Structure,
}

/// The controls that decide which rows a table shows. They change only
/// together, when a prepared load is installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableViewState {
    pub sort: Option<(usize, SortDir)>,
    pub filters: Vec<Filter>,
    pub match_all: bool,
}

impl Default for TableViewState {
    fn default() -> Self {
        Self {
            sort: None,
            filters: vec![],
            match_all: true,
        }
    }
}

impl TableViewState {
    /// The WHERE clause for the enabled filters, joined by the match mode.
    /// With no enabled filter there is no clause at all.
    pub fn filter_expr(&self) -> Option<sql::Expr> {
        let children: Vec<sql::Expr> = self
            .filters
            .iter()
            .filter(|f| f.enabled)
            .map(Filter::expression)
            .collect();
        if children.is_empty() {
            None
        } else if self.match_all {
            Some(sql::Expr::All(children))
        } else {
            Some(sql::Expr::Any(children))
        }
    }
}

/// Rows read for one view, not yet shown. Preparing one touches nothing.
pub struct LoadedTable {
    rows: GridRows,
    keys: Vec<ItemKey>,
    total: usize,
    table_revision: u64,
}

pub struct TableTab {
    /// Never-reused identity of this tab within its workbench. `id` is a render
    /// identity; operations that outlive one event name the tab by this key.
    pub key: ItemKey,
    pub id: WidgetId,
    pub schema: String,
    pub name: String,
    pub mode: TableMode,
    pub mode_tabs: Tabs,
    pub grid: DataGrid,
    pub chips: ChipBar,
    view: TableViewState,
    view_revision: u64,
    pub structure_tabs: Tabs,
    pub structure: DataTable,
    pub ddl: ScrollPanel,
    pub loading: u32,
    pub columns: Vec<(String, ColType)>,
    pub total: usize,
    /// How many saves the store had accepted for this table when the shown rows
    /// were read. It is the table's revision, not the grid's.
    table_revision: u64,
    /// Another view saved this table after these rows were read.
    source_changed: bool,
}

impl TableTab {
    pub fn new(key: ItemKey, id: WidgetId, src: Source, table: &Table) -> Self {
        let columns: Vec<(String, ColType)> = table
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.ty))
            .collect();
        let mut grid =
            DataGrid::new(id.sub("grid"), column_specs(table, &columns)).duplicate_keys(&[ALT_D]);
        grid.editable =
            table.kind == crate::db::ObjectKind::Table && !table.primary_key().is_empty();
        if !grid.editable {
            grid.read_only_reason = Some(if table.kind == crate::db::ObjectKind::View {
                "These rows come from a view, which cannot be edited.".into()
            } else {
                "This table has no primary key, so rows cannot be identified.".into()
            });
        }
        grid.empty = EmptyState::new("No rows").hint("f changes the filter");
        let mut chips = ChipBar::new(id.sub("chips"));
        chips.set_lead(Some("match all ▾".into()));

        let mut mode_tabs = Tabs::new(id.sub("mode"), &["Data", "Structure"]);
        mode_tabs.active = 0;
        let mut structure_tabs = Tabs::new(
            id.sub("structure-tabs"),
            &[
                "Columns",
                "Indexes",
                "Foreign keys",
                "Constraints",
                "Triggers",
                "DDL",
            ],
        );
        structure_tabs.active = 0;
        let mut t = Self {
            key,
            id,
            schema: table.schema.clone(),
            name: table.name.clone(),
            mode: TableMode::Data,
            mode_tabs,
            grid,
            chips,
            view: TableViewState::default(),
            view_revision: 0,
            structure_tabs,
            structure: DataTable::try_new(id.sub("structure"), vec![], vec![])
                .expect("rectangular fixture"),
            ddl: ScrollPanel::new(id.sub("ddl"), vec![]),
            loading: 3,
            columns,
            total: src.row_count(table),
            table_revision: 0,
            source_changed: false,
        };
        t.rebuild_structure(table);
        t.load_first(src);
        t
    }

    pub fn label(&self) -> String {
        if self.schema == "public" {
            self.name.clone()
        } else {
            format!("{}.{}", self.schema, self.name)
        }
    }

    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    fn select(&self, view: &TableViewState) -> sql::Select {
        sql::Select {
            columns: vec!["*".into()],
            schema: Some(self.schema.clone()),
            table: self.name.clone(),
            filter: view.filter_expr(),
            order: view
                .sort
                .map(|(c, d)| (self.columns[c].0.clone(), d == SortDir::Asc)),
            limit: Some(sql::ROW_CAP),
            count_only: false,
        }
    }

    /// Whether a captured save owns this table's rows, so nothing may reload,
    /// sort or filter them.
    pub fn saving(&self) -> bool {
        self.grid.submission_pending()
    }

    /// The controls the shown rows were read with.
    pub fn view(&self) -> &TableViewState {
        &self.view
    }

    /// Counts the views this tab has installed. A request made against an older
    /// count was made against controls that are no longer shown.
    pub fn view_revision(&self) -> u64 {
        self.view_revision
    }

    /// The store revision of the table when the shown rows were read.
    pub fn table_revision(&self) -> u64 {
        self.table_revision
    }

    /// Whether another view saved this table after these rows were read. The
    /// rows and any draft stay as they are until the table is reloaded.
    pub fn source_changed(&self) -> bool {
        self.source_changed
    }

    pub fn mark_source_changed(&mut self) {
        self.source_changed = true;
    }

    /// Records this tab's own accepted save. The rows are the saved ones, so
    /// the table revision they were read at is the store's now.
    pub fn saved(&mut self, revision: u64, live_rows: usize, inserted: usize, deleted: usize) {
        self.table_revision = revision;
        self.source_changed = false;
        if self.view.filters.iter().any(|f| f.enabled) {
            self.total = self.total.saturating_add(inserted).saturating_sub(deleted);
            self.grid.total = RowTotal::Estimated(self.total);
        } else {
            self.total = live_rows;
            self.grid.total = RowTotal::Exact(live_rows);
        }
    }

    /// Reads the rows `view` selects. Nothing the tab shows changes, so a failed
    /// query leaves the rows, draft and controls exactly as they were.
    pub fn prepare_load(
        &self,
        src: Source,
        view: &TableViewState,
    ) -> Result<LoadedTable, sql::ExecError> {
        let rs = sql::run_select(src, &self.select(view))?;
        let rows: Vec<Vec<CellValue>> = rs
            .rows
            .iter()
            .map(|r| r.iter().map(to_cell).collect())
            .collect();
        let total = if view.filters.iter().any(|f| f.enabled) {
            RowTotal::Estimated(rs.total)
        } else {
            RowTotal::Exact(rs.total)
        };
        Ok(LoadedTable {
            rows: GridRows {
                rows,
                total,
                more: rs.total > sql::ROW_CAP,
            },
            keys: rs
                .row_keys
                .expect("a SELECT of every column returns its row keys"),
            total: rs.total,
            table_revision: rs
                .source_revision
                .expect("a SELECT of a table returns its revision"),
        })
    }

    /// Shows a prepared load and its view together. The grid replaces its rows
    /// once, so a refusal (a save owns the rows) changes nothing, draft and
    /// controls included. The replacement itself drops the draft.
    pub fn install_load(
        &mut self,
        loaded: LoadedTable,
        view: TableViewState,
    ) -> Result<(), GridDataError> {
        let revision = self
            .view_revision
            .checked_add(1)
            .expect("table view revisions exhausted");
        self.grid.try_set_rows_keyed(loaded.rows, loaded.keys)?;
        self.total = loaded.total;
        self.table_revision = loaded.table_revision;
        self.source_changed = false;
        self.grid.sort = view.sort;
        self.grid.filtered_cols = view
            .filters
            .iter()
            .filter(|f| f.enabled)
            .filter_map(|f| self.columns.iter().position(|c| c.0 == f.column))
            .collect();
        self.grid.empty = EmptyState::new("No rows").hint("f changes the filter");
        self.view = view;
        self.view_revision = revision;
        self.rebuild_chips();
        Ok(())
    }

    /// The first load of a new tab. With no earlier rows to keep, a failed query
    /// shows an empty error state.
    fn load_first(&mut self, src: Source) {
        let view = TableViewState::default();
        match self.prepare_load(src, &view) {
            Ok(loaded) => self
                .install_load(loaded, view)
                .expect("a new table tab is never being saved"),
            Err(e) => {
                self.grid
                    .try_set_rows(GridRows {
                        rows: vec![],
                        total: RowTotal::Exact(0),
                        more: false,
                    })
                    .expect("a new table tab is never being saved");
                self.grid.empty = EmptyState::new("Query failed").hint(&e.message);
                self.rebuild_chips();
            }
        }
    }

    /// Puts the sort marker and chips back to the shown view, after a gesture
    /// moved them ahead of a change that was not applied.
    pub fn restore_view_controls(&mut self) {
        self.grid.sort = self.view.sort;
        self.rebuild_chips();
    }

    fn rebuild_chips(&mut self) {
        let chips = self
            .view
            .filters
            .iter()
            .map(|f| {
                let mut c = Chip::new(&f.chip_label());
                c.enabled = f.enabled;
                c
            })
            .collect();
        self.chips.set_chips(chips);
        self.chips.set_lead(Some(if self.view.match_all {
            "match all ▾".into()
        } else {
            "match any ▾".into()
        }));
    }

    fn rebuild_structure(&mut self, table: &Table) {
        let (cols, rows): (Vec<Column>, Vec<Vec<Cell>>) = match self.structure_tabs.active {
            0 => (
                vec![
                    Column::new("Name", Constraint::Min(16)),
                    Column::new("Type", Constraint::Length(14)),
                    Column::new("Nullable", Constraint::Length(8)),
                    Column::new("Default", Constraint::Length(22)),
                    Column::new("Key", Constraint::Length(6)),
                ],
                table
                    .columns
                    .iter()
                    .map(|c| {
                        let key = if c.primary {
                            "PK"
                        } else if c.references.is_some() {
                            "FK"
                        } else {
                            ""
                        };
                        vec![
                            Cell::new(c.name.clone()),
                            Cell::new(c.ty.sql()).tone(Tone::Secondary),
                            Cell::new(if c.nullable { "yes" } else { "no" }).tone(if c.nullable {
                                Tone::Muted
                            } else {
                                Tone::Secondary
                            }),
                            Cell::new(c.default.clone().unwrap_or("—".into())).tone(Tone::Muted),
                            Cell::new(key).tone(Tone::Secondary),
                        ]
                    })
                    .collect(),
            ),
            1 => (
                vec![
                    Column::new("Name", Constraint::Min(20)),
                    Column::new("Columns", Constraint::Min(18)),
                    Column::new("Unique", Constraint::Length(6)),
                    Column::new("Method", Constraint::Length(6)),
                ],
                table
                    .indexes
                    .iter()
                    .map(|i| {
                        vec![
                            Cell::new(i.name.clone()),
                            Cell::new(i.columns.join(", ")).tone(Tone::Secondary),
                            Cell::new(if i.unique { "yes" } else { "" }).tone(Tone::Secondary),
                            Cell::new(i.method).tone(Tone::Muted),
                        ]
                    })
                    .collect(),
            ),
            2 => (
                vec![
                    Column::new("Column", Constraint::Min(16)),
                    Column::new("References", Constraint::Min(22)),
                    Column::new("On delete", Constraint::Length(10)),
                ],
                table
                    .columns
                    .iter()
                    .filter_map(|c| c.references.as_ref().map(|r| (c, r)))
                    .map(|(c, r)| {
                        let cascade = table.constraints.iter().any(|k| {
                            k.definition.contains(&format!("({})", c.name))
                                && k.definition.contains("CASCADE")
                        });
                        vec![
                            Cell::new(c.name.clone()),
                            Cell::new(format!("{}({})", r.0, r.1)).tone(Tone::Secondary),
                            Cell::new(if cascade { "CASCADE" } else { "NO ACTION" })
                                .tone(Tone::Muted),
                        ]
                    })
                    .collect(),
            ),
            3 => (
                vec![
                    Column::new("Name", Constraint::Min(22)),
                    Column::new("Kind", Constraint::Length(12)),
                    Column::new("Definition", Constraint::Min(24)),
                ],
                table
                    .constraints
                    .iter()
                    .map(|k| {
                        vec![
                            Cell::new(k.name.clone()),
                            Cell::new(k.kind).tone(Tone::Secondary),
                            Cell::new(k.definition.clone()).tone(Tone::Muted),
                        ]
                    })
                    .collect(),
            ),
            4 => (
                vec![
                    Column::new("Trigger", Constraint::Min(20)),
                    Column::new("Timing · event", Constraint::Min(24)),
                ],
                table
                    .triggers
                    .iter()
                    .map(|tr| {
                        let (name, rest) = tr.split_once(' ').unwrap_or((tr.as_str(), ""));
                        vec![Cell::new(name), Cell::new(rest).tone(Tone::Secondary)]
                    })
                    .collect(),
            ),
            _ => (vec![], vec![]),
        };
        let empty = match self.structure_tabs.active {
            1 => "No indexes",
            2 => "No foreign keys",
            3 => "No constraints",
            4 => "No triggers",
            _ => "Nothing here",
        };
        self.structure = DataTable::try_new(self.id.sub("structure"), cols, rows)
            .expect("rectangular fixture")
            .empty_text(empty);

        let mut ddl = vec![format!("CREATE TABLE {} (", model::table_ident(table))];
        for (i, c) in table.columns.iter().enumerate() {
            let mut line = format!("    {} {}", model::quote_ident(&c.name), c.ty.sql());
            if !c.nullable {
                line.push_str(" NOT NULL");
            }
            if let Some(d) = &c.default {
                line.push_str(&format!(" DEFAULT {d}"));
            }
            if i + 1 < table.columns.len() || !table.constraints.is_empty() {
                line.push(',');
            }
            ddl.push(line);
        }
        for (i, k) in table.constraints.iter().enumerate() {
            let mut line = format!(
                "    CONSTRAINT {} {} {}",
                model::quote_ident(&k.name),
                k.kind,
                k.definition
            );
            if i + 1 < table.constraints.len() {
                line.push(',');
            }
            ddl.push(line);
        }
        ddl.push(");".into());
        for ix in table
            .indexes
            .iter()
            .filter(|i| !i.name.ends_with("_pkey") && !i.name.ends_with("_key"))
        {
            let columns: Vec<String> = ix.columns.iter().map(|c| model::quote_ident(c)).collect();
            ddl.push(format!(
                "CREATE INDEX {} ON {} USING {} ({});",
                model::quote_ident(&ix.name),
                model::table_ident(table),
                ix.method,
                columns.join(", ")
            ));
        }
        if let Some(c) = &table.comment {
            let literal =
                model::sql_literal(&Value::Text(c.clone())).expect("text literals serialize");
            ddl.push(format!(
                "COMMENT ON TABLE {} IS {literal};",
                model::table_ident(table),
            ));
        }
        self.ddl = ScrollPanel::new(self.id.sub("ddl"), ddl);
    }

    pub fn structure_refresh(&mut self, cat: &Catalog) {
        if let Some(t) = cat.find(Some(&self.schema), &self.name) {
            let t = t.clone();
            self.rebuild_structure(&t);
        }
    }

    pub fn is_editing(&self) -> bool {
        self.grid.is_editing()
    }

    pub fn dirty_count(&self) -> usize {
        self.grid.pending().total()
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.grid.is_editing() {
            return junie_tui::widgets::keyhint::EDIT_CELL.to_vec();
        }
        match focus {
            Some(f) if f == self.mode_tabs.id => {
                vec![hint("←→", "Switch"), hint("Ctrl+D", "Data / Structure")]
            }
            Some(f) if f == self.chips.id => {
                let km = &self.chips.keymap;
                let mut h = vec![hint("Enter", "Edit filter"), hint("Space", "Enable")];
                h.extend(hint_key(km, ChipCmd::Remove).map(|k| hint(k, "Remove")));
                if self.chips.add_label().is_some() {
                    h.extend(hint_key(km, ChipCmd::Add).map(|k| hint(k, "Add")));
                }
                h
            }
            Some(f) if f == self.structure_tabs.id => {
                vec![hint("←→", "Section"), hint("1–6", "Jump")]
            }
            Some(f) if f == self.grid.id => {
                let km = &self.grid.keymap;
                let mut h = vec![hint("↑↓←→", "Cell")];

                if self.grid.cell_editable() {
                    h.push(junie_tui::widgets::widget_hints::EDIT);
                }
                if self.grid.editable
                    && let Some(k) = hint_key(km, GridCmd::DuplicateRow)
                {
                    h.push(hint(k, "Duplicate row"));
                }
                h.extend(hint_key(km, GridCmd::Sort).map(|k| hint(k, "Sort")));
                h.extend(hint_key(km, GridCmd::FilterOnCell).map(|k| hint(k, "Filter")));
                if self.dirty_count() > 0 {
                    h.extend(hint_key(km, GridCmd::Commit).map(|k| hint(k, "Save")));
                } else {
                    h.push(hint("Space", "Select row"));
                }
                h.push(junie_tui::widgets::keyhint::CONTEXT_MENU);
                h
            }
            _ => vec![hint("↑↓", "Move"), hint("Ctrl+D", "Data / Structure")],
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        let (strip, body) = Tabs::layout(area);
        self.mode_tabs.render(strip, buf, ctx, bg);
        self.mode = if self.mode_tabs.active == 0 {
            TableMode::Data
        } else {
            TableMode::Structure
        };
        match self.mode {
            TableMode::Data => {
                let mut y = body.y;
                if !self.view.filters.is_empty() || ctx.interaction.focused(self.chips.id) {
                    self.chips
                        .render(Rect::new(body.x, y, body.width, 1), buf, ctx, bg);
                    y += 2;
                }
                let grid_area =
                    Rect::new(body.x, y, body.width, body.bottom().saturating_sub(y + 1));
                if self.loading > 0 {
                    self.grid.loading = true;
                }
                self.grid.render(grid_area, buf, ctx, bg);

                let sy = body.bottom().saturating_sub(1);

                let mut parts: Vec<(String, u8)> = vec![];
                if let Some((c, d)) = self.view.sort {
                    parts.push((
                        format!(
                            "sort {} {}",
                            self.columns[c].0,
                            if d == SortDir::Asc { "▴" } else { "▾" }
                        ),
                        4,
                    ));
                }
                let active = self.view.filters.iter().filter(|f| f.enabled).count();
                if active > 0 {
                    parts.push((format!("filtered ({active})"), 4));
                }
                parts.push((self.grid.rows_label(), 5));
                if let Some(c) = self.grid.cols_label() {
                    parts.push((c, 2));
                }
                if let Some(r) = &self.grid.read_only_reason {
                    parts.push((format!("read-only: {r}"), 3));
                }
                let avail = body.width.saturating_sub(2) as usize;
                let joined = |parts: &[(String, u8)]| {
                    parts
                        .iter()
                        .map(|p| p.0.as_str())
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                while parts.len() > 1 && junie_tui::ui::text::width(&joined(&parts)) > avail {
                    let (i, _) = parts
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, p)| p.1)
                        .expect("non-empty");
                    parts.remove(i);
                }
                buf.set_string(
                    body.x + 1,
                    sy,
                    junie_tui::ui::reveal::cut(
                        Rect::new(body.x + 1, sy, (avail) as u16, 1),
                        &joined(&parts),
                    ),
                    t.muted().bg(bg),
                );
            }
            TableMode::Structure => {
                let (strip, inner) = Tabs::layout(body);
                self.structure_tabs.render(strip, buf, ctx, bg);
                if self.structure_tabs.active == 5 {
                    let focused = ctx.interaction.focused(self.ddl.id);
                    let pos = scrollbar::position_label(&self.ddl.scroll);
                    let panel = Panel::card(Some("DDL")).focused(focused).meta(&pos);
                    let cbg = panel.bg(t);
                    let pin = panel.render(inner, buf, t);
                    self.ddl.render(pin, buf, ctx, cbg, |t, line| {
                        if line.starts_with("CREATE") || line.starts_with("COMMENT") {
                            t.primary().add_modifier(Modifier::BOLD)
                        } else if line.trim_start().starts_with("CONSTRAINT") {
                            t.secondary()
                        } else {
                            t.primary()
                        }
                    });
                } else {
                    self.structure.render(
                        Rect::new(
                            inner.x,
                            inner.y,
                            inner.width,
                            inner.height.saturating_sub(1),
                        ),
                        buf,
                        ctx,
                        bg,
                    );
                    let sy = inner.bottom().saturating_sub(1);
                    let n = self.structure.len();
                    let what = self.structure_tabs.items()[self.structure_tabs.active]
                        .label
                        .to_lowercase();
                    buf.set_string(
                        inner.x + 1,
                        sy,
                        format!(
                            "{n} {what} · read from the catalog · changes are queued until Save"
                        ),
                        t.muted().bg(bg),
                    );
                }
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum ResultBody {
    Rows(DataGrid),
    Affected {
        rows: usize,
        verb: String,
    },
    Error {
        message: String,
        detail: Option<String>,
        at: Option<usize>,
    },
    Plan {
        tree: TreeView,
        raw: ScrollPanel,
        show_raw: bool,
        planning_ms: f64,
        execution_ms: Option<f64>,
        nodes: Vec<PlanInfo>,
    },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct PlanInfo {
    pub op: String,
    pub relation: Option<String>,
    pub cost: (f64, f64),
    pub rows: usize,
    pub actual_ms: Option<f64>,
    pub loops: usize,
    pub detail: Vec<(String, String)>,
    pub warning: Option<String>,
    pub share: f64,
}

pub struct ResultSet {
    pub key: ItemKey,
    pub label: String,
    pub pinned: bool,
    pub anchor: Range<usize>,
    pub duration_ms: u32,
    pub body: ResultBody,
    /// The table and revision the rows were read from. The rows stay as they
    /// were read; a later save only marks them.
    pub source: Option<(TableId, u64)>,
    pub source_changed: bool,
}

pub struct Running {
    pub statements: Vec<(String, Range<usize>)>,
    pub current: usize,
    pub ticks_left: u32,
    pub started_ticks: u32,
    pub all: bool,
    pub explain: Option<bool>,
    pub results: Vec<ResultSet>,
}

#[derive(Clone, Copy)]
struct ResultSlot {
    n: usize,
    key: ItemKey,
}

enum RunTerminal {
    Complete { failed: bool },
    Cancelled,
}

fn mint_result_key(counter: &mut u64) -> ItemKey {
    *counter += 1;
    ItemKey::num(*counter)
}

/// The active index after `removed` leaves a strip that now holds `new_len` results.
/// Closing an earlier result shifts the active one down, and closing the active one
/// selects its successor, or its predecessor when it was last.
fn active_after_remove(active: usize, removed: usize, new_len: usize) -> usize {
    if new_len == 0 {
        0
    } else if removed < active {
        active - 1
    } else {
        active.min(new_len - 1)
    }
}

pub struct QueryTab {
    pub key: ItemKey,
    pub id: WidgetId,
    pub name: String,
    pub editor: CodeEditor,
    pub results: Vec<ResultSet>,
    pub result_tabs: Tabs,
    pub active_result: usize,
    pub running: Option<Running>,
    pub completion: Completion,
    pub split: Split,

    pub seam: Splitter,

    pub split_area: Rect,
    pub last_status: Option<(String, bool)>,
    pub last_duration: Option<u32>,
    pub result_counter: usize,
    next_result_key: u64,
    pub unseen: bool,
    pub saved_text: String,

    pub result_facts: ScrollView,
}

impl QueryTab {
    pub fn new(key: ItemKey, id: WidgetId, name: &str, text: &str) -> Self {
        let editor = CodeEditor::new(id.sub("editor"), text)
            .highlighter(highlight_sql)
            .segmenter(segment_sql)
            .placeholder("Type SQL. Ctrl+R runs the statement under the cursor.");
        let mut result_tabs = Tabs::new(id.sub("result-tabs"), &[]);
        result_tabs.allow_new = false;
        Self {
            key,
            id,
            name: name.to_owned(),
            editor,
            results: vec![],
            result_tabs,
            active_result: 0,
            running: None,
            completion: Completion::new(id.sub("completion")),
            split: Split::new(38, 4, 6),
            seam: Splitter::new(id.sub("seam"), SplitDir::Vertical),
            result_facts: ScrollView::new(id.sub("result-facts")).axes(Axes::VERTICAL),
            split_area: Rect::ZERO,
            last_status: None,
            last_duration: None,
            result_counter: 0,
            next_result_key: 0,
            unseen: false,
            saved_text: text.to_owned(),
        }
    }

    pub fn dirty(&self) -> bool {
        self.editor.text() != self.saved_text
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub(crate) fn active_grid_mut(&mut self) -> Option<&mut DataGrid> {
        match self
            .results
            .get_mut(self.active_result)
            .map(|r| &mut r.body)
        {
            Some(ResultBody::Rows(g)) => Some(g),
            _ => None,
        }
    }

    pub fn active_grid(&self) -> Option<&DataGrid> {
        match self.results.get(self.active_result).map(|r| &r.body) {
            Some(ResultBody::Rows(g)) => Some(g),
            _ => None,
        }
    }

    pub fn is_editing(&self) -> bool {
        self.editor.wants_text_input() || self.active_grid().is_some_and(|g| g.is_editing())
    }

    pub fn statements_to_run(&self, all: bool) -> Vec<(String, Range<usize>)> {
        let text = self.editor.text();
        if all {
            return sql::split_statements(text)
                .into_iter()
                .map(|(a, b)| (text[a..b].to_owned(), a..b))
                .collect();
        }
        match self.editor.selection_or_block() {
            Some((s, r)) => {
                let inner = sql::split_statements(&s);
                if inner.len() > 1 {
                    inner
                        .into_iter()
                        .map(|(a, b)| (s[a..b].to_owned(), r.start + a..r.start + b))
                        .collect()
                } else {
                    vec![(s.trim().to_owned(), r)]
                }
            }
            None => vec![],
        }
    }

    pub fn start(
        &mut self,
        statements: Vec<(String, Range<usize>)>,
        all: bool,
        explain: Option<bool>,
    ) {
        if statements.is_empty() {
            self.last_status = Some(("nothing to run".into(), false));
            return;
        }

        let active = self.results.get(self.active_result).map(|r| r.key);
        self.results.retain(|r| r.pinned);
        self.active_result = active
            .and_then(|k| self.results.iter().position(|r| r.key == k))
            .unwrap_or(0);
        self.sync_result_tabs();
        self.editor.set_running(Some(statements[0].1.clone()));
        self.editor.set_diagnostics(Vec::new());
        self.running = Some(Running {
            statements,
            current: 0,
            ticks_left: 6,
            started_ticks: 0,
            all,
            explain,
            results: vec![],
        });
        self.completion.close();
    }

    pub fn cancel(&mut self) -> bool {
        let Some(run) = self.running.take() else {
            return false;
        };
        self.finish_running(run, RunTerminal::Cancelled);
        true
    }

    pub fn tick(&mut self, src: Source, connection: &str, database: &str) -> Vec<HistoryEntry> {
        let mut out = vec![];
        let Some(r) = self.running.as_mut() else {
            return out;
        };
        r.started_ticks += 1;
        if r.ticks_left > 0 {
            r.ticks_left -= 1;
            return out;
        }
        let (sql_text, range) = r.statements[r.current].clone();
        let explain = r.explain;
        let key = mint_result_key(&mut self.next_result_key);
        let (result, entry) = execute(
            src,
            &sql_text,
            range.clone(),
            explain,
            connection,
            database,
            ResultSlot {
                n: self.result_counter + 1,
                key,
            },
        );
        self.result_counter += 1;
        let failed = matches!(result.body, ResultBody::Error { .. });
        if let ResultBody::Error { at, message, .. } = &result.body {
            let start = range.start + at.unwrap_or(0);
            let end = if at.is_some() {
                (start + 8).min(range.end)
            } else {
                range.end
            };
            self.editor.push_diagnostic(Diagnostic {
                range: start..end,
                severity: Severity::Error,
                message: message.clone(),
            });
        }
        r.results.push(result);
        out.push(entry);
        if failed || r.current + 1 >= r.statements.len() {
            let done = self.running.take().unwrap();
            self.finish_running(done, RunTerminal::Complete { failed });
        } else {
            r.current += 1;
            r.ticks_left = 4;
            let next = r.statements[r.current].1.clone();
            self.editor.set_running(Some(next));
        }
        out
    }

    /// Marks every result read from `table` before it reached `revision`. The
    /// rows are a snapshot of the query as it ran and are never rerun.
    pub fn table_saved(&mut self, table: &TableId, revision: u64) {
        let mut marked = false;
        for r in &mut self.results {
            if r.source
                .as_ref()
                .is_some_and(|(t, read_at)| t == table && *read_at < revision)
            {
                marked |= !r.source_changed;
                r.source_changed = true;
            }
        }
        if marked {
            self.sync_result_tabs();
        }
    }

    fn finish_running(&mut self, run: Running, terminal: RunTerminal) {
        self.editor.set_running(None);
        let Running {
            statements,
            current,
            started_ticks,
            all,
            results: done,
            ..
        } = run;
        let n = done.len();
        let total_ms: u32 = done.iter().map(|r| r.duration_ms).sum();
        self.results.extend(done);
        match terminal {
            RunTerminal::Cancelled => {
                let key = mint_result_key(&mut self.next_result_key);
                self.results.push(ResultSet {
                    key,
                    source: None,
                    source_changed: false,
                    label: "Cancelled".into(),
                    pinned: false,
                    anchor: statements[current].1.clone(),
                    duration_ms: started_ticks * 80,
                    body: ResultBody::Cancelled,
                });
                self.last_status = Some(("cancelled".into(), false));
            }
            RunTerminal::Complete { failed } => {
                self.last_duration = Some(total_ms);
                self.last_status = Some(if failed {
                    let msg = if all && n > 1 {
                        format!("Statement {n}/{} failed", statements.len())
                    } else {
                        "error".into()
                    };
                    (msg, true)
                } else {
                    let summary = if n > 1 {
                        format!("{n} statements")
                    } else {
                        "ok".into()
                    };
                    let took = duration(
                        std::time::Duration::from_millis(u64::from(total_ms)),
                        DurationStyle::Compact,
                    );
                    (format!("{summary} · {took}"), false)
                });
                self.unseen = true;
            }
        }
        self.active_result = self.results.len().saturating_sub(1);
        self.sync_result_tabs();
    }

    fn sync_result_tabs(&mut self) {
        let items: Vec<TabItem> = self
            .results
            .iter()
            .map(|r| {
                let mut it = TabItem::new(&r.label);
                it.error = matches!(r.body, ResultBody::Error { .. });
                it.closable = !r.pinned;
                if r.pinned {
                    it.prefix = Some("▪".into());
                }
                if r.source_changed {
                    it.suffix = Some("source changed".into());
                }
                it
            })
            .collect();
        let active = self.active_result.min(items.len().saturating_sub(1));

        debug_assert_eq!(self.result_tabs.id, self.id.sub("result-tabs"));
        self.result_tabs.set_items(items);
        self.result_tabs.set_active(active);
    }

    pub fn set_active_result(&mut self, i: usize) {
        if i < self.results.len() {
            self.active_result = i;
            self.result_tabs.set_active(i);
            let anchor = self.results[i].anchor.clone();
            self.editor.jump_to(anchor.start);
        }
    }

    pub fn close_result(&mut self, i: usize) {
        if i < self.results.len() && !self.results[i].pinned {
            self.results.remove(i);
            self.active_result = active_after_remove(self.active_result, i, self.results.len());
            self.sync_result_tabs();
        }
    }

    /// Pinning or unpinning is an explicit action on that result, so it becomes the active one.
    pub fn toggle_pin(&mut self, i: usize) {
        if i < self.results.len() {
            self.results[i].pinned = !self.results[i].pinned;

            let r = self.results.remove(i);
            if r.pinned {
                self.results.insert(0, r);
                self.active_result = 0;
            } else {
                self.results.push(r);
                self.active_result = self.results.len() - 1;
            }
            self.sync_result_tabs();
        }
    }

    pub fn trigger_completion(&mut self, src: Source, manual: bool) {
        let cur = self.editor.cursor_offset();
        let text = self.editor.text();
        if !manual && !model::auto_trigger(text, cur) {
            self.completion.close();
            return;
        }
        let (items, replace) = model::complete(src, text, cur);
        let anchor = self
            .editor
            .cursor_cell()
            .map(|c| Rect::new(c.x.saturating_sub(replace as u16), c.y, 1, 1))
            .unwrap_or(Rect::ZERO);
        let items: Vec<Item> = items
            .into_iter()
            .map(|c| {
                Item::new(c.label.clone())
                    .glyph(c.kind.glyph())
                    .detail(c.detail)
                    .insert(c.insert)
                    .matched(c.matched)
            })
            .collect();
        if items.is_empty() {
            self.completion.close();
        } else {
            self.completion.open(items, anchor, replace);
        }
    }

    fn accept_completion(&mut self, i: usize) {
        if self.completion.apply(i, &mut self.editor.field.buffer) {
            self.completion.close();
        }
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.is_running() {
            return vec![hint("Esc", "Cancel query")];
        }
        if self.completion.is_open() {
            return junie_tui::core::hints::HasHints::hints(
                &self.completion,
                &junie_tui::core::hints::HintCx::new(focus),
            );
        }
        if self.editor.wants_text_input() {
            let mut h = vec![
                hint("Ctrl+R", "Run"),
                hint(crate::app::key_label(&crate::app::RUN_ALL), "Run all"),
                hint("Ctrl+Space", "Complete"),
            ];
            h.extend(self.editor.hints());
            return h;
        }
        if let Some(g) = self.active_grid()
            && focus == Some(g.id)
        {
            if g.is_editing() {
                return vec![hint("Enter", "Apply"), hint("Esc", "Cancel")];
            }

            let km = &g.keymap;
            let mut hints = vec![hint("↑↓←→", "Cell")];
            if g.cell_editable() {
                hints.push(junie_tui::widgets::widget_hints::EDIT);
            }
            if g.editable
                && let Some(k) = hint_key(km, GridCmd::DuplicateRow)
            {
                hints.push(hint(k, "Duplicate row"));
            }
            hints.extend(hint_key(km, GridCmd::Sort).map(|k| hint(k, "Sort")));
            hints.extend(hint_key(km, GridCmd::Copy).map(|k| hint(k, "Copy")));
            hints.push(junie_tui::widgets::keyhint::CONTEXT_MENU);
            return hints;
        }
        match focus {
            Some(f) if f == self.editor.id => vec![
                junie_tui::widgets::widget_hints::EDIT,
                hint("Ctrl+R", "Run"),
                hint(crate::app::key_label(&crate::app::RUN_ALL), "Run all"),
                hint("Ctrl+X", "Explain"),
                hint("/", "Find"),
            ],
            Some(f) if f == self.result_tabs.id => {
                let mut h = vec![hint("←→", "Result"), hint("p", "Pin")];
                h.extend(
                    hint_key(&self.result_tabs.keymap, TabCmd::Close).map(|k| hint(k, "Close")),
                );
                h.push(hint("Enter", "Go to statement"));
                h
            }
            _ => vec![hint("↑↓", "Move")],
        }
    }

    pub fn on_key(&mut self, key: &Key, cx: &mut Cx, src: Source) -> Outcome {
        let Some(f) = cx.focus.current() else {
            return Outcome::Ignored;
        };
        if f == self.editor.id {
            if self.completion.is_open() {
                let (o, ev) = self.completion.on_key(key);
                match ev {
                    Some(CompletionAction::Accept(i)) => {
                        self.accept_completion(i);
                        return Outcome::Changed;
                    }
                    Some(CompletionAction::Dismiss) => return Outcome::Changed,
                    None => {
                        if o.consumed() {
                            return o;
                        }
                    }
                }
            }
            if key.ctrl_char(' ')
                || (key.code == KeyCode::Char(' ') && key.ctrl())
                || key.code == KeyCode::Null
            {
                if !self.editor.editing {
                    self.editor.begin_edit();
                }
                self.trigger_completion(src, true);
                return Outcome::Changed;
            }
            let (o, ev) = self.editor.on_key(key);
            match ev {
                Some(EditorAction::Changed) => {
                    self.editor.set_diagnostics(Vec::new());
                    self.trigger_completion(src, false);
                }
                Some(EditorAction::CursorMoved) => {
                    if self.completion.is_open() {
                        self.trigger_completion(src, false);
                    }
                }
                Some(EditorAction::Committed) => self.completion.close(),
                Some(EditorAction::Leave { backward }) => {
                    if backward {
                        cx.focus_prev()
                    } else {
                        cx.focus_next()
                    }
                }
                None => {}
            }
            return o;
        }
        if f == self.result_tabs.id {
            match key.code {
                KeyCode::Char('p') | KeyCode::Char('.') => {
                    self.toggle_pin(self.result_tabs.cursor);
                    return Outcome::Changed;
                }
                KeyCode::Char('x') => {
                    self.close_result(self.result_tabs.cursor);
                    return Outcome::Changed;
                }
                _ => {}
            }
            let (o, ev) = self.result_tabs.on_key(key);
            match ev {
                Some(TabAction::Activated(i)) => self.set_active_result(i),
                Some(TabAction::Close(i)) => self.close_result(i),
                _ => {}
            }
            return o;
        }

        let Some(r) = self.results.get_mut(self.active_result) else {
            return Outcome::Ignored;
        };
        match &mut r.body {
            ResultBody::Rows(g) if f == g.id => {
                let (o, actions) = g.on_key(key).into_parts();
                for ev in actions {
                    match ev {
                        GridAction::Copy(s) => cx.copy(s),
                        GridAction::SortRequested(_) => {}
                        GridAction::LeaveForward => cx.focus_next(),
                        GridAction::LeaveBackward => cx.focus_prev(),
                        GridAction::ReadOnly { reason } => cx.status(reason),
                        _ => {}
                    }
                }
                o
            }
            ResultBody::Plan {
                tree,
                raw,
                show_raw,
                ..
            } => {
                if key.is_char('r') {
                    *show_raw = !*show_raw;
                    return Outcome::Changed;
                }
                if *show_raw && f == raw.id {
                    return raw.on_key(key);
                }
                if f == tree.id {
                    return tree.on_key(key).outcome();
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    pub fn press(&mut self, ev: &PointerEvent) -> Outcome {
        if let Some(r) = self.editor.route(ev) {
            return r.outcome();
        }
        if let Some(g) = self.active_grid_mut()
            && let Some(r) = g.route(ev)
        {
            return r.outcome();
        }
        self.plan_route(ev)
    }

    fn plan_route(&mut self, ev: &PointerEvent) -> Outcome {
        let Some(r) = self.results.get_mut(self.active_result) else {
            return Outcome::Ignored;
        };
        match &mut r.body {
            ResultBody::Plan { tree, raw, .. } => tree
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| raw.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            _ => Outcome::Ignored,
        }
    }

    pub fn middle(&mut self, ev: &PointerEvent) -> Outcome {
        let Some((o, actions)) = self.result_tabs.route(ev).map(|r| r.into_parts()) else {
            return Outcome::Ignored;
        };
        let mut out = o;
        for action in actions {
            if let TabAction::Close(i) = action {
                self.close_result(i);
                out = Outcome::Changed;
            }
        }
        out
    }

    pub fn click(&mut self, ev: &PointerEvent, cx: &mut Cx, cat: &Catalog) -> Outcome {
        let id = ev.id;
        if id == self.seam.id {
            return self.seam.on_click(&mut self.split, ev.count());
        }
        if self.completion.is_open() {
            match self.completion.route(ev).map(|r| r.into_parts()) {
                Some((_, actions)) => {
                    for action in actions {
                        if let CompletionAction::Accept(i) = action {
                            self.accept_completion(i);
                            return Outcome::Changed;
                        }
                    }
                }
                None => self.completion.close(),
            }
        }
        if let Some(r) = self.editor.route(ev) {
            if id == self.editor.id {
                cx.focus.focus(id);
            }
            let _ = cat;
            return r.outcome();
        }
        if let Some(r) = self.result_tabs.route(ev) {
            cx.focus.focus(self.result_tabs.id);
            let (o, actions) = r.into_parts();
            for act in actions {
                match act {
                    TabAction::Activated(i) => self.set_active_result(i),
                    TabAction::Close(i) => self.close_result(i),
                    _ => {}
                }
            }
            return o;
        }
        let Some(r) = self.results.get_mut(self.active_result) else {
            return Outcome::Ignored;
        };
        match &mut r.body {
            ResultBody::Rows(g) => match g.route(ev) {
                Some(r) => {
                    cx.focus.focus(g.id);
                    let (o, actions) = r.into_parts();
                    for act in actions {
                        if let GridAction::Copy(s) = act {
                            cx.copy(s);
                        }
                    }
                    o
                }
                None => Outcome::Ignored,
            },
            ResultBody::Plan { tree, raw, .. } => {
                if ev.on(tree.id) {
                    if tree.row_at(ev).is_some() {
                        cx.focus.focus(tree.id);
                    }
                    return tree.pointer(ev).outcome();
                }
                raw.route(ev).map_or(Outcome::Ignored, |r| r.outcome())
            }
            _ => Outcome::Ignored,
        }
    }

    pub fn drag(&mut self, ev: &PointerEvent) -> Outcome {
        if ev.id == self.seam.id {
            let area = self.split_area;
            return self.seam.on_drag(&mut self.split, area, 1, ev.pos());
        }
        if let Some(r) = self.editor.route(ev) {
            return r.outcome();
        }
        if let Some(g) = self.active_grid_mut()
            && let Some(r) = g.route(ev)
        {
            return r.outcome();
        }
        self.plan_route(ev)
    }

    pub fn wheel(&mut self, ev: &PointerEvent) -> Outcome {
        let horizontal = matches!(ev.wheel(), Some((Axis::Horizontal, _)));
        if ev.on(self.completion.id) {
            return if horizontal {
                Outcome::Consumed
            } else {
                self.completion.pointer(ev).outcome()
            };
        }
        if let Some(r) = self.editor.route(ev) {
            return r.outcome();
        }
        let Some(r) = self.results.get_mut(self.active_result) else {
            return Outcome::Ignored;
        };
        match &mut r.body {
            ResultBody::Rows(g) => g.route(ev).map_or(Outcome::Ignored, |r| r.outcome()),
            ResultBody::Plan { tree, raw, .. } => {
                if ev.on(tree.id) {
                    if horizontal {
                        Outcome::Consumed
                    } else {
                        tree.pointer(ev).outcome()
                    }
                } else {
                    raw.route(ev).map_or(Outcome::Ignored, |r| r.outcome())
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn render_toolbar(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let running = self.running.is_some();
        let mut buttons = vec![
            Button::primary(TOOLBAR_RUN, "Run")
                .tooltip("Run the statement at the cursor")
                .shortcut("Ctrl+R")
                .disabled(running),
            Button::subtle(TOOLBAR_RUN_ALL, "Run all")
                .tooltip("Run every statement")
                .shortcut(crate::app::key_label(&crate::app::RUN_ALL))
                .disabled(running),
            Button::subtle(TOOLBAR_EXPLAIN, "Explain")
                .tooltip("Show the plan of the statement at the cursor")
                .shortcut("Ctrl+X")
                .disabled(running),
        ];
        if running {
            buttons.push(
                Button::secondary(TOOLBAR_CANCEL, "Cancel")
                    .tooltip("Cancel the running query")
                    .shortcut("Esc"),
            );
        }

        let mut buttons: Vec<Button> = buttons.into_iter().map(|b| b.focusable(false)).collect();
        let widths: Vec<u16> = buttons.iter().map(Button::width).collect();
        let rects = junie_tui::widgets::button::row_layout(area, &widths, 1);
        for (b, r) in buttons.iter_mut().zip(rects) {
            if r.bottom() <= area.bottom() {
                b.render(r, buf, ctx, bg);
            }
        }
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if self.editor.editing {
            return self.editor.on_paste(text);
        }
        if let Some(g) = self.active_grid_mut() {
            return g.on_paste(text);
        }
        Outcome::Ignored
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        let (top, bottom) = self.split.vertical(area, 1);

        let top = if top.height > 3 {
            self.render_toolbar(Rect::new(top.x, top.y, top.width, 1), buf, ctx, bg);
            Rect::new(top.x, top.y + 1, top.width, top.height - 1)
        } else {
            top
        };
        if !top.is_empty() {
            self.editor.render(top, buf, ctx, bg);
        }
        self.split_area = area;
        let handle = self.split.handle(SplitDir::Vertical, area, 1);
        self.seam.render(handle, buf, ctx, bg);
        if bottom.is_empty() {
            return;
        }

        let has_results = !self.results.is_empty();
        let mut y = bottom.y;
        if has_results {
            self.result_tabs
                .render(Rect::new(bottom.x, y, bottom.width, 2), buf, ctx, bg);
            y += 2;
        }

        let status_line = if let Some(r) = &self.running {
            format!(
                "{} running {} · Esc cancels",
                junie_tui::widgets::progress::spinner_frame(ctx.interaction.tick),
                duration(
                    std::time::Duration::from_millis(u64::from(r.started_ticks * 80)),
                    DurationStyle::Compact
                )
            )
        } else if let Some(rs) = self.results.get(self.active_result) {
            match &rs.body {
                ResultBody::Rows(g) => {
                    let total = match g.total {
                        RowTotal::Exact(n) | RowTotal::Estimated(n) => n,
                        RowTotal::Unknown => g.len(),
                    };
                    let rows = if g.more {
                        format!("Showing {} rows", junie_tui::ui::text::thousands(g.len()))
                    } else if total == 1 {
                        "1 row".into()
                    } else if total == 0 {
                        "No rows".into()
                    } else {
                        format!("{} rows", junie_tui::ui::text::thousands(total))
                    };
                    format!(
                        "{rows} · {}",
                        duration(
                            std::time::Duration::from_millis(u64::from(rs.duration_ms)),
                            DurationStyle::Compact
                        )
                    )
                }
                ResultBody::Affected { rows, verb } => format!(
                    "{verb} · {} row{} affected · {}",
                    rows,
                    if *rows == 1 { "" } else { "s" },
                    duration(
                        std::time::Duration::from_millis(u64::from(rs.duration_ms)),
                        DurationStyle::Compact
                    )
                ),
                ResultBody::Error { .. } => {
                    format!(
                        "failed · {}",
                        duration(
                            std::time::Duration::from_millis(u64::from(rs.duration_ms)),
                            DurationStyle::Compact
                        )
                    )
                }
                ResultBody::Plan {
                    planning_ms,
                    execution_ms,
                    ..
                } => match execution_ms {
                    Some(e) => format!("Planning {planning_ms:.3} ms · Execution {e:.3} ms"),
                    None => format!("Planning {planning_ms:.3} ms · r Raw"),
                },
                ResultBody::Cancelled => "Cancelled · nothing was recorded".into(),
            }
        } else {
            String::new()
        };
        if !status_line.is_empty() {
            let done = self
                .results
                .get(self.active_result)
                .filter(|_| self.running.is_none())
                .and_then(|r| match r.body {
                    ResultBody::Error { .. } => Some(StatusKind::Fail),
                    ResultBody::Rows(_) | ResultBody::Affected { .. } => Some(StatusKind::Ok),
                    _ => None,
                });
            let st = match done {
                Some(StatusKind::Fail) => t.status_label(StatusKind::Fail),
                _ if self.running.is_some() => t.secondary(),
                _ => t.muted(),
            };
            let line = match done {
                Some(kind) => format!("{} {status_line}", t.status(kind).0),
                None => status_line,
            };
            let shown =
                junie_tui::ui::text::truncate(&line, bottom.width.saturating_sub(2) as usize);
            buf.set_string(bottom.x + 1, y, &shown, st.bg(bg));
            if let Some(kind) = done
                && !shown.is_empty()
            {
                let (glyph, gs) = t.status(kind);
                buf.set_string(bottom.x + 1, y, glyph, gs.bg(bg));
            }
            if self.running.is_some() {
                buf.set_string(
                    bottom.x + 1,
                    y,
                    junie_tui::widgets::progress::spinner_frame(ctx.interaction.tick),
                    t.accent_fg().bg(bg),
                );
            }
            y += 1;
        }
        let body = Rect::new(bottom.x, y, bottom.width, bottom.bottom().saturating_sub(y));
        if self.results.is_empty() && self.running.is_none() {
            junie_tui::widgets::empty::render(
                body,
                buf,
                t,
                &EmptyState::new("No results yet").hint(&format!(
                    "Ctrl+R runs the statement under the cursor · {} runs all",
                    crate::app::key_label(&crate::app::RUN_ALL)
                )),
                bg,
            );
        } else {
            let tick = ctx.interaction.tick;
            if let Some(rs) = self.results.get_mut(self.active_result) {
                match &mut rs.body {
                    ResultBody::Rows(g) => g.render(body, buf, ctx, bg),
                    ResultBody::Affected { rows, verb } => {
                        let card = Panel::card(Some("Statement executed"));
                        let cbg = card.bg(t);
                        let inner = card.render(
                            Rect::new(body.x, body.y, body.width.min(60), body.height.min(6)),
                            buf,
                            t,
                        );
                        props::render_in(
                            &mut self.result_facts,
                            inner,
                            buf,
                            ctx,
                            &[
                                Prop::new("Statement", verb.clone()),
                                Prop::new("Rows affected", junie_tui::ui::text::thousands(*rows)),
                                Prop::new(
                                    "Duration",
                                    duration(
                                        std::time::Duration::from_millis(u64::from(rs.duration_ms)),
                                        DurationStyle::Compact,
                                    ),
                                )
                                .tone(Tone::Muted),
                            ],
                            cbg,
                        );
                    }
                    ResultBody::Error {
                        message,
                        detail,
                        at,
                    } => {
                        let card = Panel::card(Some("Error"));
                        let cbg = card.bg(t);
                        let inner = card.render(
                            Rect::new(body.x, body.y, body.width.min(90), body.height.min(8)),
                            buf,
                            t,
                        );
                        let pos = match at {
                            Some(_) => {
                                "Enter on the result tab jumps to the statement · the offending token is underlined in the editor"
                            }
                            None => "Enter on the result tab jumps to the statement",
                        };
                        let mut notice = Notice::error(message.as_str()).hint(pos);
                        if let Some(d) = detail {
                            notice = notice.detail(d.as_str());
                        }
                        notice.render(inner, buf, ctx, cbg);
                    }
                    ResultBody::Plan {
                        tree,
                        raw,
                        show_raw,
                        nodes,
                        ..
                    } => {
                        if *show_raw {
                            let focused = ctx.interaction.focused(raw.id);
                            let card = Panel::card(Some("EXPLAIN · raw"))
                                .focused(focused)
                                .meta("r Tree");
                            let cbg = card.bg(t);
                            let inner = card.render(body, buf, t);
                            raw.render(inner, buf, ctx, cbg, |t, _| t.secondary());
                        } else {
                            let detail_w = if body.width >= 110 { 40 } else { 0 };
                            let tree_area = Rect::new(
                                body.x,
                                body.y,
                                body.width
                                    .saturating_sub(detail_w + if detail_w > 0 { 2 } else { 0 }),
                                body.height,
                            );

                            tree.reserve_right = PLAN_COLS_W;
                            tree.show_meta = false;
                            buf.set_string(
                                tree_area.x + 3,
                                tree_area.y,
                                "Operation",
                                t.muted().bg(bg),
                            );
                            tree.render(
                                Rect::new(
                                    tree_area.x,
                                    tree_area.y + 1,
                                    tree_area.width,
                                    tree_area.height.saturating_sub(1),
                                ),
                                buf,
                                ctx,
                                bg,
                            );
                            let strip = tree.trailing();
                            let cols_x = strip.x;
                            if strip.width > 0 {
                                buf.set_string(
                                    cols_x,
                                    tree_area.y,
                                    plan_header(),
                                    t.muted().bg(bg),
                                );
                            }

                            let rows = tree.rows().to_vec();
                            for (i, ri) in tree.scroll.visible_range().enumerate() {
                                let y = tree_area.y + 1 + i as u16;
                                let Some(row) = rows.get(ri) else { continue };
                                let idx = row.meta.as_deref().and_then(|m| m.parse::<usize>().ok());
                                let Some(info) = idx.and_then(|k| nodes.get(k)) else {
                                    continue;
                                };
                                if strip.width == 0 {
                                    continue;
                                }
                                let focused_row =
                                    ctx.interaction.focused(tree.id) && ri == tree.cursor;
                                let base = if focused_row {
                                    t.primary().add_modifier(Modifier::BOLD)
                                } else {
                                    t.secondary()
                                };
                                let share = info.share * 100.0;
                                let share_style = if share > HOT_SHARE * 100.0 {
                                    t.primary().fg(t.warning).add_modifier(Modifier::BOLD)
                                } else if share > 20.0 {
                                    t.primary().add_modifier(Modifier::BOLD)
                                } else if share > 5.0 {
                                    t.secondary()
                                } else {
                                    t.muted()
                                };
                                let actual = info
                                    .actual_ms
                                    .map(|m| format!("{m:.1} ms"))
                                    .unwrap_or("—".into());
                                let text = format!(
                                    "{:>13} {:>8} {:>10}",
                                    format!("{:.0}..{:.0}", info.cost.0, info.cost.1),
                                    sql::fmt_rows(info.rows),
                                    actual
                                );
                                let bgc = buf[(cols_x, y)].bg;
                                buf.set_string(cols_x, y, &text, base.bg(bgc));

                                let sh = format!(
                                    "{:>4} {}",
                                    share.round() as u32,
                                    if share > HOT_SHARE * 100.0 {
                                        t.glyphs.warning
                                    } else {
                                        " "
                                    }
                                );
                                buf.set_string(cols_x + 34, y, &sh, share_style.bg(bgc));
                            }
                            if detail_w > 0 {
                                let d =
                                    Rect::new(tree_area.right() + 2, body.y, detail_w, body.height);
                                let rows = tree.rows();
                                if let Some(row) = rows.get(tree.cursor) {
                                    let idx =
                                        row.meta.as_deref().and_then(|m| m.parse::<usize>().ok());
                                    if let Some(info) = idx.and_then(|k| nodes.get(k)) {
                                        let card = Panel::card(Some(&info.op));
                                        let cbg = card.bg(t);
                                        let inner = card.render(
                                            Rect::new(d.x, d.y, d.width, d.height.min(16)),
                                            buf,
                                            t,
                                        );
                                        let mut facts = vec![];
                                        if let Some(r) = &info.relation {
                                            facts.push(Prop::new("Relation", r.clone()));
                                        }
                                        facts.push(
                                            Prop::new(
                                                "Cost",
                                                format!("{:.2}..{:.2}", info.cost.0, info.cost.1),
                                            )
                                            .tone(Tone::Secondary),
                                        );
                                        facts.push(
                                            Prop::new(
                                                "Est. rows",
                                                junie_tui::ui::text::thousands(info.rows),
                                            )
                                            .tone(Tone::Secondary),
                                        );
                                        if let Some(a) = info.actual_ms {
                                            facts.push(
                                                Prop::new(
                                                    "Actual",
                                                    format!(
                                                        "{a:.3} ms · {} loop{}",
                                                        info.loops,
                                                        if info.loops == 1 { "" } else { "s" }
                                                    ),
                                                )
                                                .tone(Tone::Secondary),
                                            );
                                        }
                                        for (k, v) in &info.detail {
                                            facts.push(
                                                Prop::new(k, v.clone()).tone(Tone::Muted).wrap(),
                                            );
                                        }
                                        if let Some(w) = &info.warning {
                                            facts.push(
                                                Prop::new("Note", w.clone())
                                                    .tone(Tone::Warning)
                                                    .wrap(),
                                            );
                                        }
                                        props::render(inner, buf, t, &facts, cbg);
                                    }
                                }
                            }
                        }
                    }
                    ResultBody::Cancelled => {
                        junie_tui::widgets::empty::render(
                        body,
                        buf,
                        t,
                        &EmptyState::new("Cancelled").hint(
                            "The statement was stopped before it finished; nothing was recorded",
                        ),
                        bg,
                    );
                    }
                }
            }
            let _ = tick;
        }

        if self.completion.is_open() {
            let screen = *buf.area();
            self.completion.render(screen, buf, ctx);
        }
    }
}

const HOT_SHARE: f64 = 0.5;

const PLAN_COLS_W: u16 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Run,
    RunAll,
    Explain,
    Cancel,
}

pub const TOOLBAR_RUN: WidgetId = WidgetId::of("query.toolbar.run");
pub const TOOLBAR_RUN_ALL: WidgetId = WidgetId::of("query.toolbar.run_all");
pub const TOOLBAR_EXPLAIN: WidgetId = WidgetId::of("query.toolbar.explain");
pub const TOOLBAR_CANCEL: WidgetId = WidgetId::of("query.toolbar.cancel");

pub fn toolbar_action(id: WidgetId) -> Option<ToolbarAction> {
    match id {
        i if i == TOOLBAR_RUN => Some(ToolbarAction::Run),
        i if i == TOOLBAR_RUN_ALL => Some(ToolbarAction::RunAll),
        i if i == TOOLBAR_EXPLAIN => Some(ToolbarAction::Explain),
        i if i == TOOLBAR_CANCEL => Some(ToolbarAction::Cancel),
        _ => None,
    }
}

fn plan_header() -> String {
    format!("{:>13} {:>8} {:>10} {:>4}", "cost", "rows", "actual", "%")
}

fn max_total_cost(node: &PlanNode) -> f64 {
    node.children
        .iter()
        .map(max_total_cost)
        .fold(node.cost.1, f64::max)
}

fn plan_to_tree(node: &PlanNode, nodes: &mut Vec<PlanInfo>, root_total: f64) -> TreeNode {
    let children_total: f64 = node.children.iter().map(|c| c.cost.1).sum();
    let exclusive = (node.cost.1 - children_total).max(0.0);
    let share = if root_total > 0.0 {
        (exclusive / root_total).min(1.0)
    } else {
        0.0
    };
    let idx = nodes.len();
    nodes.push(PlanInfo {
        op: node.op.clone(),
        relation: node.relation.clone(),
        cost: node.cost,
        rows: node.rows,
        actual_ms: node.actual_ms,
        loops: node.loops,
        detail: node.detail.clone(),
        warning: node.warning.clone(),
        share,
    });
    let label = match &node.relation {
        Some(r) => format!("{} on {r}", node.op),
        None => node.op.clone(),
    };
    let children: Vec<TreeNode> = node
        .children
        .iter()
        .map(|c| plan_to_tree(c, nodes, root_total))
        .collect();
    let mut n = if children.is_empty() {
        TreeNode::leaf(&label)
    } else {
        TreeNode::dir(&label, children)
    };

    if share > HOT_SHARE {
        n = n.label_tone(Tone::Warning);
    }
    n.meta(&idx.to_string())
}

fn execute(
    src: Source,
    text: &str,
    range: Range<usize>,
    explain: Option<bool>,
    connection: &str,
    database: &str,
    slot: ResultSlot,
) -> (ResultSet, HistoryEntry) {
    let ResultSlot { n, key } = slot;
    let mut entry = HistoryEntry {
        id: 0,
        sql: text.to_owned(),
        connection: connection.to_owned(),
        database: database.to_owned(),
        schema: "public".into(),
        minutes_ago: 0,
        duration_ms: None,
        rows: None,
        error: None,
        source: if explain.is_some() {
            HistorySource::Explain
        } else {
            HistorySource::Editor
        },
    };
    let parsed = match sql::parse(text) {
        Ok(p) => p,
        Err(e) => {
            entry.error = Some(e.message.clone());
            return (
                ResultSet {
                    key,
                    source: None,
                    source_changed: false,
                    label: format!("Error {n}"),
                    pinned: false,
                    anchor: range,
                    duration_ms: 1,
                    body: ResultBody::Error {
                        message: e.message,
                        detail: Some("syntax error".into()),
                        at: Some(e.at),
                    },
                },
                entry,
            );
        }
    };
    let stmt = match (explain, parsed) {
        (Some(analyze), Statement::Select(s)) => Statement::Explain {
            analyze,
            inner: Box::new(Statement::Select(s)),
        },
        (Some(analyze), other) => Statement::Explain {
            analyze,
            inner: Box::new(other),
        },
        (None, p) => p,
    };
    let label_for = |verb: &str, target: Option<&str>| -> String {
        let l = match target {
            Some(t) => format!("{verb} {t}"),
            None => verb.to_owned(),
        };
        junie_tui::ui::text::truncate(&l, 28)
    };
    match stmt {
        Statement::Select(sel) => match sql::run_select(src, &sel) {
            Ok(rs) => {
                let table = src.catalog.find(sel.schema.as_deref(), &sel.table);
                let specs = table
                    .map(|t| column_specs(t, &rs.columns))
                    .unwrap_or_else(|| {
                        rs.columns
                            .iter()
                            .map(|(n, ty)| ColumnSpec::new(n, cell_kind(*ty)))
                            .collect()
                    });
                let mut grid =
                    DataGrid::new(WidgetId::of("result").child(n), specs).duplicate_keys(&[ALT_D]);

                grid.editable = false;
                grid.read_only_reason = Some(if rs.editable {
                    "Query results are read-only · open the table to edit its rows.".into()
                } else {
                    "TablePro cannot tell which table these rows came from.".into()
                });
                grid.local_sort = true;
                grid.empty = EmptyState::new("No rows").hint("The query matched nothing");
                let rows: Vec<Vec<CellValue>> = rs
                    .rows
                    .iter()
                    .map(|r| r.iter().map(to_cell).collect())
                    .collect();
                let more = rs.total > rows.len() && sel.limit.is_none_or(|l| l > rows.len());
                let total = if more {
                    RowTotal::Estimated(rs.total)
                } else {
                    RowTotal::Exact(rows.len())
                };
                let data = GridRows { rows, total, more };
                let seated = match rs.row_keys.clone() {
                    Some(keys) => grid.try_set_rows_keyed(data, keys),
                    None => grid.try_set_rows(data),
                };
                if let Err(e) = seated {
                    let message = e.to_string();
                    entry.error = Some(message.clone());
                    return (
                        ResultSet {
                            key,
                            source: None,
                            source_changed: false,
                            label: format!("Error {n}"),
                            pinned: false,
                            anchor: range,
                            duration_ms: 2,
                            body: ResultBody::Error {
                                message,
                                detail: None,
                                at: None,
                            },
                        },
                        entry,
                    );
                }
                let source = table
                    .zip(rs.source_revision)
                    .map(|(t, rev)| (src.table_id(t), rev));
                entry.duration_ms = Some(rs.duration_ms);
                entry.rows = Some(rs.rows.len());
                (
                    ResultSet {
                        key,
                        source,
                        source_changed: false,
                        label: format!(
                            "{} ({})",
                            label_for("SELECT", Some(&sel.table)),
                            rs.rows.len()
                        ),
                        pinned: false,
                        anchor: range,
                        duration_ms: rs.duration_ms,
                        body: ResultBody::Rows(grid),
                    },
                    entry,
                )
            }
            Err(e) => {
                entry.error = Some(e.message.clone());
                (
                    ResultSet {
                        key,
                        source: None,
                        source_changed: false,
                        label: format!("Error {n}"),
                        pinned: false,
                        anchor: range,
                        duration_ms: 2,
                        body: ResultBody::Error {
                            message: e.message,
                            detail: e.detail,
                            at: e.at,
                        },
                    },
                    entry,
                )
            }
        },
        Statement::Explain { analyze, inner } => match *inner {
            Statement::Select(sel) => match sql::explain(src, &sel, analyze) {
                Ok(plan) => {
                    let mut nodes = vec![];
                    let root = plan_to_tree(&plan, &mut nodes, max_total_cost(&plan));
                    let mut tree = TreeView::new(WidgetId::of("plan").child(n), vec![root]);
                    tree.expand_all();
                    let mut raw_lines = vec![];
                    sql::plan_text(&plan, 0, &mut raw_lines);
                    let planning =
                        0.21 + sel.filter.as_ref().map_or(0, sql::Expr::leaves) as f64 * 0.09;
                    raw_lines.push(format!("Planning Time: {planning:.3} ms"));
                    let exec = analyze.then(|| plan.actual_ms.unwrap_or(0.0) + 0.4);
                    if let Some(e) = exec {
                        raw_lines.push(format!("Execution Time: {e:.3} ms"));
                    }
                    let raw = ScrollPanel::new(WidgetId::of("plan-raw").child(n), raw_lines);
                    entry.duration_ms = Some(exec.map(|e| e as u32).unwrap_or(1) + 1);
                    entry.rows = Some(nodes.len());
                    (
                        ResultSet {
                            key,
                            source: None,
                            source_changed: false,
                            label: if analyze {
                                "EXPLAIN ANALYZE".into()
                            } else {
                                "EXPLAIN".into()
                            },
                            pinned: false,
                            anchor: range,
                            duration_ms: entry.duration_ms.unwrap_or(1),
                            body: ResultBody::Plan {
                                tree,
                                raw,
                                show_raw: false,
                                planning_ms: planning,
                                execution_ms: exec,
                                nodes,
                            },
                        },
                        entry,
                    )
                }
                Err(e) => {
                    entry.error = Some(e.message.clone());
                    (
                        ResultSet {
                            key,
                            source: None,
                            source_changed: false,
                            label: format!("Error {n}"),
                            pinned: false,
                            anchor: range,
                            duration_ms: 1,
                            body: ResultBody::Error {
                                message: e.message,
                                detail: e.detail,
                                at: None,
                            },
                        },
                        entry,
                    )
                }
            },
            other => {
                let msg = format!(
                    "EXPLAIN is only implemented for SELECT in this prototype ({})",
                    other.verb()
                );
                entry.error = Some(msg.clone());
                (
                    ResultSet {
                        key,
                        source: None,
                        source_changed: false,
                        label: format!("Error {n}"),
                        pinned: false,
                        anchor: range,
                        duration_ms: 1,
                        body: ResultBody::Error {
                            message: msg,
                            detail: None,
                            at: None,
                        },
                    },
                    entry,
                )
            }
        },
        other => {
            let target = other.target().map(str::to_owned);
            let table = target.as_deref().and_then(|t| src.catalog.find(None, t));
            let (rows, ms) = match &other {
                Statement::Update { has_where, .. } | Statement::Delete { has_where, .. } => {
                    let n = table.map(|t| src.row_count(t)).unwrap_or(0);
                    if *has_where {
                        ((n / 150).max(1), 12 + (n / 40_000) as u32)
                    } else {
                        (n, 40 + (n / 5_000) as u32)
                    }
                }
                Statement::Insert { .. } => (1, 4),
                Statement::Truncate { .. } => (table.map(|t| src.row_count(t)).unwrap_or(0), 30),
                _ => (0, 25),
            };
            if let Some(t) = target.as_deref()
                && table.is_none()
                && matches!(
                    other,
                    Statement::Update { .. }
                        | Statement::Delete { .. }
                        | Statement::Truncate { .. }
                        | Statement::Drop { .. }
                )
            {
                let msg = format!("relation \"{t}\" does not exist");
                entry.error = Some(msg.clone());
                return (
                    ResultSet {
                        key,
                        source: None,
                        source_changed: false,
                        label: format!("Error {n}"),
                        pinned: false,
                        anchor: range,
                        duration_ms: 1,
                        body: ResultBody::Error {
                            message: msg,
                            detail: None,
                            at: None,
                        },
                    },
                    entry,
                );
            }
            entry.duration_ms = Some(ms);
            entry.rows = Some(rows);
            (
                ResultSet {
                    key,
                    source: None,
                    source_changed: false,
                    label: format!("{} ({})", label_for(other.verb(), target.as_deref()), rows),
                    pinned: false,
                    anchor: range,
                    duration_ms: ms,
                    body: ResultBody::Affected {
                        rows,
                        verb: format!(
                            "{}{}",
                            other.verb(),
                            target.map(|t| format!(" {t}")).unwrap_or_default()
                        ),
                    },
                },
                entry,
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryCommand {
    Open,
    Rerun,
    Copy,
}

/// Row to select after a refresh. A selection that is still listed keeps its
/// entry. A hidden or deleted one moves to the nearest entry after it in the
/// previous order that is still listed, then the nearest before it, then the
/// row at its old position, and an empty list selects nothing.
fn reselect(before: &[ItemKey], at: usize, key: ItemKey, now: &[ItemKey]) -> Option<usize> {
    let rows: std::collections::HashMap<ItemKey, usize> =
        now.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    if let Some(row) = rows.get(&key) {
        return Some(*row);
    }
    let after = before.get(at + 1..).unwrap_or_default().iter();
    let ahead = before.get(..at).unwrap_or_default().iter().rev();
    after
        .chain(ahead)
        .find_map(|k| rows.get(k).copied())
        .or_else(|| now.len().checked_sub(1).map(|last| at.min(last)))
}

pub struct HistoryTab {
    pub key: ItemKey,
    pub id: WidgetId,
    pub list: ListBox,
    pub detail: CodeEditor,
    pub search: TextInput,
    pub scope_all: bool,
    pub failed_only: bool,

    pub slow_ms: u32,
    pub filtered: Vec<ItemKey>,
    detail_loaded: Option<(ItemKey, String)>,
    pub open_btn: Button,
    pub rerun_btn: Button,
    pub copy_btn: Button,
    pub split: Split,

    pub seam: Splitter,
    pub split_area: Rect,

    pub detail_view: ScrollView,
}

impl HistoryTab {
    pub fn new(key: ItemKey, id: WidgetId) -> Self {
        Self {
            key,
            id,
            list: ListBox::new(id.sub("list"), vec![], SelectMode::Single)
                .empty_text("No matching queries"),
            detail: CodeEditor::new(id.sub("detail"), "")
                .highlighter(highlight_sql)
                .segmenter(segment_sql)
                .read_only(true),
            detail_view: ScrollView::new(id.sub("detail-view")).axes(Axes::VERTICAL),
            search: TextInput::new(id.sub("search"), "")
                .placeholder("Search history · terms are ANDed")
                .plain_label(),
            scope_all: false,
            failed_only: false,
            slow_ms: 1000,
            filtered: vec![],
            detail_loaded: None,

            open_btn: Button::primary(id.sub("open"), "Open in new tab")
                .tooltip("Open the selected query in a new tab")
                .shortcut("Enter"),
            rerun_btn: Button::secondary(id.sub("rerun"), "Run in new tab")
                .tooltip("Run the selected query again in a new tab")
                .shortcut("r"),
            copy_btn: Button::subtle(id.sub("copy"), "Copy")
                .tooltip("Copy the selected query")
                .shortcut("y"),
            split: Split::new(50, 30, 30),
            seam: Splitter::new(id.sub("seam"), SplitDir::Horizontal),
            split_area: Rect::ZERO,
        }
    }

    pub fn refresh(&mut self, history: &History, connection: &str) {
        let q = self.search.text().to_owned();
        let matches = history.search(
            &q,
            if self.scope_all {
                None
            } else {
                Some(connection)
            },
            self.failed_only,
        );
        let before = std::mem::replace(
            &mut self.filtered,
            matches.iter().map(|e| e.key()).collect(),
        );
        let (cursor, offset, viewport) = (
            self.list.cursor,
            self.list.scroll.offset,
            self.list.scroll.viewport_len,
        );
        let selected = before.get(cursor).copied();
        let kept = selected.map_or_else(
            || (!self.filtered.is_empty()).then_some(0),
            |key| reselect(&before, cursor, key, &self.filtered),
        );

        let items: Vec<ListItem> = matches
            .iter()
            .map(|e| {
                let label = e.first_line();
                let meta = format!("{} · {}", e.when(), e.duration());
                let mut it = ListItem::new(&junie_tui::ui::text::truncate(&label, 90)).meta(&meta);
                if e.duration_ms.is_some_and(|ms| ms > self.slow_ms) {
                    it = it.meta_tone(Tone::Warning);
                }
                it
            })
            .collect();
        self.list = ListBox::new(self.id.sub("list"), items, SelectMode::Single).empty_text(
            if q.is_empty() {
                "No query history"
            } else {
                "No matching queries"
            },
        );
        self.list.cursor = kept.unwrap_or(0);
        self.list.chosen = kept;
        if let Some(row) = kept
            && viewport > 0
        {
            self.list.scroll.set_viewport(viewport);
            let below_top = cursor.saturating_sub(offset).min(viewport - 1);
            self.list.scroll.scroll_to(row.saturating_sub(below_top));
        }
        self.sync_detail(history);
    }

    pub fn selected_key(&self) -> Option<ItemKey> {
        self.filtered.get(self.list.cursor).copied()
    }

    pub fn current_entry<'a>(&self, history: &'a History) -> Option<&'a HistoryEntry> {
        history.get(self.selected_key()?)
    }

    pub fn row_of(&self, key: ItemKey) -> Option<usize> {
        self.filtered.iter().position(|k| *k == key)
    }

    pub fn run_selected(&self, cmd: HistoryCommand, history: &History, cx: &mut Cx) -> Outcome {
        match self.selected_key() {
            Some(key) => self.run(key, cmd, history, cx),
            None => Outcome::Changed,
        }
    }

    pub fn run(
        &self,
        key: ItemKey,
        cmd: HistoryCommand,
        history: &History,
        cx: &mut Cx,
    ) -> Outcome {
        let Some(e) = history.get(key) else {
            cx.status("That history entry is no longer available");
            return Outcome::Changed;
        };
        match cmd {
            HistoryCommand::Open => cx.go(crate::app::Act::OpenQuery(e.sql.clone(), false)),
            HistoryCommand::Rerun => cx.go(crate::app::Act::OpenQuery(e.sql.clone(), true)),
            HistoryCommand::Copy => cx.copy_as(e.sql.clone(), "query"),
        }
        Outcome::Changed
    }

    pub fn sync_detail(&mut self, history: &History) {
        let shown = self.current_entry(history);
        let loaded = match (&self.detail_loaded, shown) {
            (Some((key, sql)), Some(e)) => *key == e.key() && *sql == e.sql,
            (None, None) => true,
            _ => false,
        };
        if loaded {
            return;
        }
        self.detail_loaded = shown.map(|e| (e.key(), e.sql.clone()));
        let text = shown.map_or("", |e| e.sql.as_str());
        self.detail.set_text(text);
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.search.editing {
            return vec![
                hint("Type", "Search"),
                hint("Enter", "Done"),
                hint("Esc", "Clear"),
            ];
        }
        match focus {
            Some(f) if f == self.list.id => vec![
                hint("Enter", "Open in new tab"),
                hint("r", "Rerun"),
                hint("y", "Copy"),
                hint("/", "Search"),
                hint("c s", "Scope · Status"),
                junie_tui::widgets::keyhint::CONTEXT_MENU,
            ],
            _ => vec![hint("↑↓", "Move"), hint("/", "Search")],
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
        history: &History,
        connection: &str,
    ) {
        let t = ctx.theme;

        let scope = format!(
            "scope: {}  ·  status: {}",
            if self.scope_all {
                "all connections"
            } else {
                connection
            },
            if self.failed_only { "failed" } else { "any" }
        );

        let scope_w = junie_tui::ui::text::width(&scope) as u16;
        let search_w = area
            .width
            .min(60)
            .min(area.width.saturating_sub(scope_w + 2))
            .max(30)
            .min(area.width);
        self.search
            .render(Rect::new(area.x, area.y, search_w, 2), buf, ctx, bg);
        let sx = area.x + search_w + 2;
        if sx + 20 < area.right() {
            buf.set_string(
                sx,
                area.y + 1,
                junie_tui::ui::reveal::cut(
                    Rect::new(sx, area.y + 1, area.right().saturating_sub(sx), 1),
                    &scope,
                ),
                t.muted().bg(bg),
            );
        }
        let body = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
        );
        let (l, r) = self.split.horizontal(body, 2);
        self.split_area = body;
        let handle = self.split.handle(SplitDir::Horizontal, body, 2);
        self.seam.render(
            Rect::new(handle.x + 1, handle.y, 1, handle.height),
            buf,
            ctx,
            bg,
        );

        self.list.render(l, buf, ctx, bg);

        let entries: Vec<&HistoryEntry> = self
            .filtered
            .iter()
            .filter_map(|key| history.get(*key))
            .collect();
        for (k, i) in self.list.scroll.visible_range().enumerate() {
            let y = l.y + k as u16;
            if let Some(e) = entries.get(i) {
                let st = buf[(l.x + 1, y)].style();
                if !e.ok() {
                    buf.set_string(l.x + 1, y, "!", st.fg(t.error).add_modifier(Modifier::BOLD));
                }
            }
        }
        if r.is_empty() {
            return;
        }
        let Some(e) = self.current_entry(history).cloned() else {
            junie_tui::widgets::empty::render(
                r,
                buf,
                t,
                &EmptyState::new("Select a query").hint("↑↓ moves through history"),
                bg,
            );
            return;
        };
        let focused = ctx.interaction.focused(self.detail.id);
        let meta = format!("{} · {}", e.source.label(), e.when());
        let card = Panel::card(Some("Query")).focused(focused).meta(&meta);
        let cbg = card.bg(t);
        let inner = card.render(r, buf, t);

        let wrapped = e
            .sql
            .lines()
            .flat_map(|l| junie_tui::ui::text::wrap(l, inner.width.saturating_sub(8) as usize))
            .collect::<Vec<_>>()
            .join("\n");
        if self.detail.text() != wrapped {
            self.detail.set_text(&wrapped);
        }
        let mut facts = vec![
            Prop::new(
                "Connection",
                format!("{} · {}.{}", e.connection, e.database, e.schema),
            )
            .tone(Tone::Secondary),
            Prop::new("Duration", e.duration()).tone(
                if e.duration_ms.is_some_and(|ms| ms > self.slow_ms) {
                    Tone::Warning
                } else {
                    Tone::Secondary
                },
            ),
            Prop::new(
                "Rows",
                e.rows
                    .map(junie_tui::ui::text::thousands)
                    .unwrap_or("–".into()),
            )
            .tone(Tone::Secondary),
        ];
        if let Some(err) = &e.error {
            facts.push(Prop::new("Error", err.clone()).tone(Tone::Error).wrap());
        }

        let editor_nat = (wrapped.lines().count() as u16 + 1).max(3);
        let action_widths = [
            self.open_btn.width(),
            self.rerun_btn.width(),
            self.copy_btn.width(),
        ];

        let natural = |a: Size| {
            Size::new(
                a.width,
                editor_nat
                    + 1
                    + props::measure(&facts, a.width)
                    + 1
                    + junie_tui::widgets::button::row_lines(a.width, &action_widths, 2),
            )
        };
        let size = self.detail_view.fit(inner.as_size(), natural);
        let mut frame = self.detail_view.begin(inner, size, buf, ctx);
        let inner = frame.area();
        {
            let buf = frame.buffer(buf);
            self.detail.render(
                Rect::new(
                    inner.x.saturating_sub(1),
                    inner.y,
                    inner.width + 1,
                    editor_nat,
                ),
                buf,
                ctx,
                cbg,
            );
            let fy = inner.y + editor_nat + 1;
            let used = props::render(
                Rect::new(inner.x, fy, inner.width, inner.bottom().saturating_sub(fy)),
                buf,
                t,
                &facts,
                cbg,
            );

            let lines = junie_tui::widgets::button::row_lines(inner.width, &action_widths, 2);
            let ay = (fy + used + 1).min(inner.bottom().saturating_sub(lines));
            let rects = junie_tui::widgets::button::row_layout(
                Rect::new(inner.x, ay, inner.width, lines),
                &action_widths,
                2,
            );
            self.open_btn.render(rects[0], buf, ctx, cbg);
            self.rerun_btn.render(rects[1], buf, ctx, cbg);
            self.copy_btn.render(rects[2], buf, ctx, cbg);
        }
        self.detail_view.end(frame, buf, ctx);
    }
}

#[cfg(test)]
mod issue47_tests {
    use super::*;
    use junie_tui::widgets::grid::{CellKind, default_validator};

    #[test]
    fn issue47_fixture_named_values_parse_under_the_explicit_grammar() {
        let catalog = Catalog::acme_prod();
        let mut checked = [0usize; 3];
        for table in catalog.tables.iter().filter(|t| !t.columns.is_empty()) {
            let columns: Vec<(String, ColType)> = table
                .columns
                .iter()
                .map(|c| (c.name.clone(), c.ty))
                .collect();
            let specs = column_specs(table, &columns);
            for row in crate::db::rows(table, 0, 64) {
                for (spec, value) in specs.iter().zip(&row) {
                    let slot = match spec.kind {
                        CellKind::Timestamp => 0,
                        CellKind::Id => 1,
                        CellKind::Json => 2,
                        _ => continue,
                    };
                    if *value == Value::Null {
                        continue;
                    }
                    let text = value.display();
                    let parsed = default_validator(spec, &text);
                    assert_eq!(
                        parsed.map(|v| v == to_cell(value)),
                        Ok(true),
                        "{}.{} fixture {text:?} must round-trip the default parser",
                        table.name,
                        spec.name
                    );
                    checked[slot] += 1;
                }
            }
        }
        assert!(
            checked.iter().all(|&n| n > 0),
            "every named kind is exercised: {checked:?}"
        );
    }
}

#[cfg(test)]
mod issue194_tests {
    use super::*;
    use crate::db::{ColType, Column, Constraint, Index, ObjectKind};

    #[test]
    fn issue194_ddl_quotes_identifiers() {
        let table = Table {
            schema: "public".into(),
            name: "UserOrders".into(),
            kind: ObjectKind::Table,
            columns: vec![
                Column {
                    name: "order".into(),
                    ty: ColType::Int,
                    nullable: false,
                    default: None,
                    primary: true,
                    references: None,
                    enum_values: vec![],
                    generated: false,
                },
                Column {
                    name: "UserId".into(),
                    ty: ColType::Text,
                    nullable: true,
                    default: None,
                    primary: false,
                    references: None,
                    enum_values: vec![],
                    generated: false,
                },
                Column {
                    name: "we\"ird".into(),
                    ty: ColType::Text,
                    nullable: true,
                    default: None,
                    primary: false,
                    references: None,
                    enum_values: vec![],
                    generated: false,
                },
            ],
            indexes: vec![Index {
                name: "Idx\"Order".into(),
                columns: vec!["order".into(), "UserId".into()],
                unique: false,
                method: "btree",
            }],
            constraints: vec![Constraint {
                name: "order_check".into(),
                kind: "CHECK",
                definition: "CHECK (\"order\" > 0)".into(),
            }],
            triggers: vec![],
            row_count: 0,
            comment: Some("it's a note".into()),
        };
        let cat = Catalog {
            database: "db".into(),
            schemas: vec!["public".into()],
            tables: vec![table.clone()],
        };
        let store = crate::store::SessionStore::new(1);
        let tab = TableTab::new(
            ItemKey::num(0),
            WidgetId::of("issue194"),
            crate::store::Source {
                catalog: &cat,
                store: &store,
            },
            &table,
        );
        let ddl = tab.ddl.lines();

        assert_eq!(ddl[0], "CREATE TABLE \"public\".\"UserOrders\" (");
        assert_eq!(ddl[1], "    \"order\" integer NOT NULL,");
        assert_eq!(ddl[2], "    \"UserId\" text,");
        assert_eq!(ddl[3], "    \"we\"\"ird\" text,");
        assert_eq!(
            ddl[4],
            "    CONSTRAINT \"order_check\" CHECK CHECK (\"order\" > 0)"
        );
        assert_eq!(ddl[5], ");");
        assert_eq!(
            ddl[6],
            "CREATE INDEX \"Idx\"\"Order\" ON \"public\".\"UserOrders\" USING btree (\"order\", \"UserId\");"
        );
        assert_eq!(
            ddl[7],
            "COMMENT ON TABLE \"public\".\"UserOrders\" IS 'it''s a note';"
        );
    }
}

/// Painting and view-only access to one workspace tab. Its field is private,
/// so a caller of `DocumentWorkspace::view_mut` cannot reach a tab's rows,
/// draft or text through it. Every operation here moves a cursor, a scroll
/// offset or a divider, or paints; none can edit.
pub struct WorkTabView<'a> {
    document: &'a mut WorkTab,
}

impl<'a> WorkTabView<'a> {
    pub(crate) fn new(document: &'a mut WorkTab) -> Self {
        Self { document }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
        history: &History,
        connection: &str,
    ) {
        match self.document {
            WorkTab::Table(t) => t.render(area, buf, ctx, bg),
            WorkTab::Query(q) => q.render(area, buf, ctx, bg),
            WorkTab::History(h) => h.render(area, buf, ctx, bg, history, connection),
        }
    }

    /// One poll of a running query. It moves the run forward and returns the
    /// history entries its statements made, and whether the run is still
    /// going. Run progress and results are not editor text, so this is not a
    /// content change.
    pub fn tick(
        &mut self,
        src: Source,
        connection: &str,
        database: &str,
    ) -> (Vec<HistoryEntry>, bool) {
        match self.document {
            WorkTab::Query(q) => (q.tick(src, connection, database), q.is_running()),
            _ => (vec![], false),
        }
    }

    /// Clears a query's "result not seen yet" mark.
    pub fn mark_seen(&mut self) {
        if let WorkTab::Query(q) = self.document {
            q.unseen = false;
        }
    }

    /// Moves the focused grid's cell cursor or the focused editor's caret for
    /// a plain navigation key. Any other key, including every one that could
    /// type, delete or commit, is refused.
    pub fn navigate(&mut self, key: &Key, focus: Option<WidgetId>) -> bool {
        use ratatui::crossterm::event::KeyModifiers;
        let plain = key.mods.is_empty() || key.mods == KeyModifiers::SHIFT;
        let moves = matches!(
            key.code,
            KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End
        );
        if !plain || !moves {
            return false;
        }
        match self.document {
            WorkTab::Table(t) if focus == Some(t.grid.id) && !t.grid.is_editing() => {
                let _ = t.grid.on_key(key);
                true
            }
            WorkTab::Query(q) if focus == Some(q.editor.id) => {
                let _ = q.editor.on_key(key);
                true
            }
            _ => false,
        }
    }

    /// A wheel gesture over the tab's scrollable parts.
    pub fn wheel(&mut self, ev: &PointerEvent) -> Outcome {
        let horizontal = matches!(ev.wheel(), Some((Axis::Horizontal, _)));
        match self.document {
            WorkTab::Table(t) => t
                .grid
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| t.structure.route(ev).map(|r| r.outcome()))
                .or_else(|| t.ddl.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            WorkTab::Query(q) => q.wheel(ev),
            WorkTab::History(h) => {
                if ev.on(h.list.id) {
                    return if horizontal {
                        Outcome::Consumed
                    } else {
                        h.list.pointer(ev).outcome()
                    };
                }
                h.detail
                    .route(ev)
                    .map(|r| r.outcome())
                    .unwrap_or(Outcome::Ignored)
            }
        }
    }

    /// Pointer input on the read-only detail pane (history detail or query
    /// result facts): scrolling and selecting.
    pub fn facts_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        let facts = match self.document {
            WorkTab::History(h) => &mut h.detail_view,
            WorkTab::Query(q) => &mut q.result_facts,
            WorkTab::Table(_) => return None,
        };
        facts.pointer(ev).map(|r| r.outcome())
    }

    /// A press on the tab's divider.
    pub fn seam_press(&mut self, id: WidgetId, pos: ratatui::layout::Position) -> bool {
        match self.document {
            WorkTab::Query(q) if id == q.seam.id => {
                q.seam.on_press(&q.split, q.split_area, 1, pos);
                true
            }
            WorkTab::History(h) if id == h.seam.id => {
                h.seam.on_press(&h.split, h.split_area, 2, pos);
                true
            }
            _ => false,
        }
    }

    /// A drag of the tab's divider.
    pub fn seam_drag(&mut self, id: WidgetId, pos: ratatui::layout::Position) -> Option<Outcome> {
        match self.document {
            WorkTab::History(h) if id == h.seam.id => {
                let area = h.split_area;
                Some(h.seam.on_drag(&mut h.split, area, 2, pos))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod issue10_tests {
    use super::*;
    use crate::db::{self, Value};

    fn orders(cat: &Catalog) -> &Table {
        cat.find(Some("public"), "orders").expect("orders fixture")
    }

    fn filter(column: &str, op: FilterOp, value: &str, value2: &str) -> Filter {
        Filter {
            column: column.into(),
            op,
            value: value.into(),
            value2: value2.into(),
            enabled: true,
        }
    }

    fn col(table: &Table, name: &str) -> usize {
        table
            .columns
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("column {name}"))
    }

    fn text<'a>(table: &Table, row: &'a [Value], name: &str) -> Option<&'a str> {
        match &row[col(table, name)] {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    fn id_of(table: &Table, row: &[Value]) -> String {
        row[col(table, "id")].display()
    }

    /// Runs the tab's own select (its filters and match mode) through the simulator.
    fn tab_ids(cat: &Catalog, filters: Vec<Filter>, match_all: bool) -> Vec<String> {
        let table = orders(cat);
        let store = crate::store::SessionStore::new(1);
        let src = Source {
            catalog: cat,
            store: &store,
        };
        let tab = TableTab::new(ItemKey::num(0), WidgetId::of("issue10"), src, table);
        let view = TableViewState {
            sort: None,
            filters,
            match_all,
        };
        let rs = sql::run_select(src, &tab.select(&view)).expect("filtered select runs");
        let id = rs
            .columns
            .iter()
            .position(|c| c.0 == "id")
            .expect("id column");
        rs.rows.iter().map(|r| r[id].display()).collect()
    }

    /// Independent oracle over the same generated rows the simulator scans.
    fn expected(cat: &Catalog, keep: impl Fn(&Table, &[Value]) -> bool) -> Vec<String> {
        let table = orders(cat);
        let scan = table.row_count.min(2_000);
        db::rows(table, 0, scan)
            .iter()
            .filter(|r| keep(table, r))
            .take(sql::ROW_CAP)
            .map(|r| id_of(table, r))
            .collect()
    }

    fn status_is(t: &Table, r: &[Value], want: &str) -> bool {
        text(t, r, "status") == Some(want)
    }

    fn is_gift(t: &Table, r: &[Value]) -> bool {
        r[col(t, "is_gift")] == Value::Bool(true)
    }

    #[test]
    fn issue10_match_any_returns_the_union_of_enabled_filters() {
        let cat = Catalog::acme_prod();
        let filters = vec![
            filter("status", FilterOp::Eq, "paid", ""),
            filter("is_gift", FilterOp::Eq, "true", ""),
        ];
        assert_eq!(
            tab_ids(&cat, filters, false),
            expected(&cat, |t, r| status_is(t, r, "paid") || is_gift(t, r)),
        );
    }

    #[test]
    fn issue10_match_all_still_intersects_enabled_filters() {
        let cat = Catalog::acme_prod();
        let filters = vec![
            filter("status", FilterOp::Eq, "paid", ""),
            filter("is_gift", FilterOp::Eq, "true", ""),
        ];
        assert_eq!(
            tab_ids(&cat, filters, true),
            expected(&cat, |t, r| status_is(t, r, "paid") && is_gift(t, r)),
        );
    }

    #[test]
    fn issue10_no_enabled_filters_is_unrestricted() {
        let cat = Catalog::acme_prod();
        let disabled = Filter {
            enabled: false,
            ..filter("status", FilterOp::Eq, "paid", "")
        };
        assert_eq!(tab_ids(&cat, vec![], true), expected(&cat, |_, _| true));
        assert_eq!(
            tab_ids(&cat, vec![disabled], false),
            expected(&cat, |_, _| true)
        );
    }

    #[test]
    fn issue10_not_contains_excludes_matches_and_nulls() {
        let cat = Catalog::acme_prod();
        let got = tab_ids(
            &cat,
            vec![filter("notes", FilterOp::NotContains, "gift", "")],
            true,
        );
        assert_eq!(
            got,
            expected(&cat, |t, r| text(t, r, "notes")
                .is_some_and(|s| !s.to_lowercase().contains("gift"))),
        );
    }

    #[test]
    fn issue10_not_in_excludes_listed_values_and_nulls() {
        let cat = Catalog::acme_prod();
        let got = tab_ids(
            &cat,
            vec![filter("status", FilterOp::NotIn, "paid, shipped", "")],
            true,
        );
        assert_eq!(
            got,
            expected(&cat, |t, r| text(t, r, "status")
                .is_some_and(|s| s != "paid" && s != "shipped")),
        );
    }

    #[test]
    fn issue10_regex_is_a_pattern_not_a_substring() {
        let cat = Catalog::acme_prod();
        let got = tab_ids(
            &cat,
            vec![filter("status", FilterOp::Regex, "^p.id$", "")],
            true,
        );
        assert_eq!(got, expected(&cat, |t, r| status_is(t, r, "paid")));
    }

    #[test]
    fn issue10_between_stays_one_child_under_match_any() {
        let cat = Catalog::acme_prod();
        let filters = vec![
            filter("order_number", FilterOp::Between, "10000", "10002"),
            filter("status", FilterOp::Eq, "paid", ""),
        ];
        assert_eq!(
            tab_ids(&cat, filters, false),
            expected(&cat, |t, r| {
                let in_range = match r[col(t, "order_number")] {
                    Value::Int(n) => (10_000..=10_002).contains(&n),
                    _ => false,
                };
                in_range || status_is(t, r, "paid")
            }),
        );
    }

    #[test]
    fn issue10_is_null_selects_only_null_rows() {
        let cat = Catalog::acme_prod();
        let got = tab_ids(&cat, vec![filter("notes", FilterOp::IsNull, "", "")], true);
        assert_eq!(
            got,
            expected(&cat, |t, r| r[col(t, "notes")] == Value::Null)
        );
    }

    #[test]
    fn issue10_editor_rejects_values_that_cannot_evaluate() {
        let int_filter = |op, value: &str, value2: &str| Filter {
            column: "order_number".into(),
            op,
            value: value.into(),
            value2: value2.into(),
            enabled: true,
        };
        let text_filter = |op, value: &str| Filter {
            column: "notes".into(),
            op,
            value: value.into(),
            value2: String::new(),
            enabled: true,
        };
        let err = int_filter(FilterOp::Gt, "abc", "")
            .check(ColType::Int)
            .unwrap_err();
        assert_eq!(err.field, FilterField::Value);
        assert!(err.message.contains("whole number"), "{}", err.message);

        let err = int_filter(FilterOp::Between, "5", "1")
            .check(ColType::Int)
            .unwrap_err();
        assert_eq!(err.field, FilterField::Value2);

        let err = text_filter(FilterOp::Regex, "(")
            .check(ColType::Text)
            .unwrap_err();
        assert_eq!(err.field, FilterField::Value);
        assert!(err.message.starts_with("invalid regular expression"));

        let err = text_filter(FilterOp::In, "paid,,shipped")
            .check(ColType::Enum)
            .unwrap_err();
        assert_eq!(err.field, FilterField::Value);

        assert!(
            int_filter(FilterOp::Gt, "3", "")
                .check(ColType::Int)
                .is_ok()
        );
        assert!(
            int_filter(FilterOp::Between, "1", "3")
                .check(ColType::Int)
                .is_ok()
        );
        assert!(
            text_filter(FilterOp::Regex, "^p.id$")
                .check(ColType::Text)
                .is_ok()
        );
    }

    #[test]
    fn issue10_boolean_filter_rejects_non_boolean_operands() {
        let gift = |op, value: &str| filter("is_gift", op, value, "");
        let err = gift(FilterOp::Eq, "yes").check(ColType::Bool).unwrap_err();
        assert_eq!(err.field, FilterField::Value);
        assert_eq!(err.message, "Enter true or false");
        let err = gift(FilterOp::In, "true, maybe")
            .check(ColType::Bool)
            .unwrap_err();
        assert_eq!(err.field, FilterField::Value);
        assert!(gift(FilterOp::Eq, "true").check(ColType::Bool).is_ok());
        assert!(gift(FilterOp::Ne, "FALSE").check(ColType::Bool).is_ok());
        assert!(gift(FilterOp::IsNull, "").check(ColType::Bool).is_ok());
    }

    #[test]
    fn issue10_integer_range_order_is_exact() {
        let err = filter(
            "order_number",
            FilterOp::Between,
            "9007199254740993",
            "9007199254740992",
        )
        .check(ColType::Int)
        .unwrap_err();
        assert_eq!(err.field, FilterField::Value2);
    }
}

#[cfg(test)]
mod issue9_tests {
    use super::*;

    #[test]
    fn issue9_query_editor_unicode_paste() {
        let sql = "SELECT é, 名 FROM 表 WHERE note = '💾;中'; SELECT e\u{301} FROM t";
        let mut tab = QueryTab::new(ItemKey::num(0), WidgetId::of("issue9"), "Query 1", "");
        tab.editor.editing = true;
        assert_eq!(tab.on_paste(sql), Outcome::Changed);
        assert_eq!(tab.editor.text(), sql);

        let runs = tab.statements_to_run(true);
        let texts: Vec<&str> = runs.iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "SELECT é, 名 FROM 表 WHERE note = '💾;中'",
                "SELECT e\u{301} FROM t"
            ]
        );
        for (text, range) in &runs {
            assert_eq!(&sql[range.clone()], text.as_str());
        }
        assert!(!highlight_sql(sql).is_empty());
    }
}

#[cfg(test)]
mod issue84_tests {
    use super::*;
    use crate::db::Catalog;

    #[test]
    fn completion_prefix_editor_home_acceptance() {
        let cat = Catalog::acme_prod();
        let original = " SELECT id FROM users";
        let mut tab = QueryTab::new(ItemKey::num(0), WidgetId::of("issue84"), "Query 1", "");
        tab.editor.set_text(original);
        assert_eq!(tab.editor.cursor_offset(), 0);
        let store = crate::store::SessionStore::new(1);
        tab.trigger_completion(
            Source {
                catalog: &cat,
                store: &store,
            },
            true,
        );
        assert!(tab.completion.is_open());
        tab.accept_completion(0);
        let text = tab.editor.text();
        assert!(text.len() > original.len(), "{text:?}");
        assert!(text.ends_with(original), "{text:?}");
    }
}
