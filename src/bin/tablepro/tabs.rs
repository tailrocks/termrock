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

    pub fn to_sql(&self) -> String {
        let c = &self.column;
        let v = &self.value;
        match self.op {
            FilterOp::Eq => format!("{c} = '{v}'"),
            FilterOp::Ne => format!("{c} != '{v}'"),
            FilterOp::Contains => format!("{c} LIKE '%{v}%'"),
            FilterOp::NotContains => format!("{c} NOT LIKE '%{v}%'"),
            FilterOp::StartsWith => format!("{c} LIKE '{v}%'"),
            FilterOp::EndsWith => format!("{c} LIKE '%{v}'"),
            FilterOp::Gt => format!("{c} > '{v}'"),
            FilterOp::Ge => format!("{c} >= '{v}'"),
            FilterOp::Lt => format!("{c} < '{v}'"),
            FilterOp::Le => format!("{c} <= '{v}'"),
            FilterOp::IsNull => format!("{c} IS NULL"),
            FilterOp::IsNotNull => format!("{c} IS NOT NULL"),
            FilterOp::IsEmpty => format!("{c} = ''"),
            FilterOp::IsNotEmpty => format!("{c} != ''"),
            FilterOp::In => format!(
                "{c} IN ({})",
                v.split(',')
                    .map(|s| format!("'{}'", s.trim()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            FilterOp::NotIn => format!(
                "{c} NOT IN ({})",
                v.split(',')
                    .map(|s| format!("'{}'", s.trim()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            FilterOp::Between => format!("{c} >= '{v}' AND {c} <= '{}'", self.value2),
            FilterOp::Regex => format!("{c} ~ '{v}'"),
        }
    }

    pub fn predicates(&self) -> Vec<sql::Predicate> {
        use sql::Cmp;
        let p = |cmp: Cmp, value: &str| sql::Predicate {
            column: self.column.clone(),
            cmp,
            value: value.to_owned(),
        };
        match self.op {
            FilterOp::Eq => vec![p(Cmp::Eq, &self.value)],
            FilterOp::Ne => vec![p(Cmp::Ne, &self.value)],
            FilterOp::Contains => vec![p(Cmp::Like, &format!("%{}%", self.value))],
            FilterOp::NotContains => vec![],
            FilterOp::StartsWith => vec![p(Cmp::Like, &format!("{}%", self.value))],
            FilterOp::EndsWith => vec![p(Cmp::Like, &format!("%{}", self.value))],
            FilterOp::Gt => vec![p(Cmp::Gt, &self.value)],
            FilterOp::Ge => vec![p(Cmp::Ge, &self.value)],
            FilterOp::Lt => vec![p(Cmp::Lt, &self.value)],
            FilterOp::Le => vec![p(Cmp::Le, &self.value)],
            FilterOp::IsNull => vec![p(Cmp::IsNull, "")],
            FilterOp::IsNotNull => vec![p(Cmp::IsNotNull, "")],
            FilterOp::IsEmpty => vec![p(Cmp::Eq, "")],
            FilterOp::IsNotEmpty => vec![p(Cmp::Ne, "")],
            FilterOp::In => vec![p(
                Cmp::In(self.value.split(',').map(|s| s.trim().to_owned()).collect()),
                "",
            )],
            FilterOp::NotIn => vec![],
            FilterOp::Between => vec![p(Cmp::Ge, &self.value), p(Cmp::Le, &self.value2)],
            FilterOp::Regex => vec![p(Cmp::Like, &format!("%{}%", self.value))],
        }
    }
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
    pub filters: Vec<Filter>,
    pub match_all: bool,
    pub sort: Option<(usize, SortDir)>,
    pub preview: bool,
    pub structure_tabs: Tabs,
    pub structure: DataTable,
    pub ddl: ScrollPanel,
    pub loading: u32,
    pub columns: Vec<(String, ColType)>,
    pub total: usize,
}

impl TableTab {
    pub fn new(key: ItemKey, id: WidgetId, cat: &Catalog, table: &Table, preview: bool) -> Self {
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
        chips.lead = Some("match all ▾".into());

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
            filters: vec![],
            match_all: true,
            sort: None,
            preview,
            structure_tabs,
            structure: DataTable::try_new(id.sub("structure"), vec![], vec![])
                .expect("rectangular fixture"),
            ddl: ScrollPanel::new(id.sub("ddl"), vec![]),
            loading: 3,
            columns,
            total: table.row_count,
        };
        t.rebuild_structure(table);
        t.load(cat).expect("a new table tab is never being saved");
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

    fn select(&self) -> sql::Select {
        let mut predicates = Vec::new();
        for f in self.filters.iter().filter(|f| f.enabled) {
            predicates.extend(f.predicates());
        }
        sql::Select {
            columns: vec!["*".into()],
            schema: Some(self.schema.clone()),
            table: self.name.clone(),
            predicates,
            order: self
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

    /// Reloads the rows from the current query. A save in flight refuses it
    /// before anything changes, so callers that edit the view first must check
    /// [`TableTab::saving`] before they do.
    pub fn load(&mut self, cat: &Catalog) -> Result<(), GridDataError> {
        if self.saving() {
            return Err(GridDataError::Busy);
        }
        let sel = self.select();
        match sql::run_select(cat, &sel) {
            Ok(rs) => {
                let rows: Vec<Vec<CellValue>> = rs
                    .rows
                    .iter()
                    .map(|r| r.iter().map(to_cell).collect())
                    .collect();
                let total = if self.filters.iter().any(|f| f.enabled) {
                    RowTotal::Estimated(rs.total)
                } else {
                    RowTotal::Exact(rs.total)
                };
                self.total = rs.total;
                let loaded = self.grid.try_set_rows(GridRows {
                    rows,
                    total,
                    more: rs.total > sql::ROW_CAP,
                });
                match loaded {
                    Ok(()) => {
                        self.grid.sort = self.sort;
                        self.grid.filtered_cols = self
                            .filters
                            .iter()
                            .filter(|f| f.enabled)
                            .filter_map(|f| self.columns.iter().position(|c| c.0 == f.column))
                            .collect();
                    }
                    Err(e @ GridDataError::Busy) => return Err(e),
                    Err(e) => self.show_load_failure(&e.to_string()),
                }
            }
            Err(e) => self.show_load_failure(&e.message),
        }
        self.rebuild_chips();
        Ok(())
    }

    fn show_load_failure(&mut self, message: &str) {
        self.grid
            .try_set_rows(GridRows {
                rows: vec![],
                total: RowTotal::Exact(0),
                more: false,
            })
            .expect("an empty row set fits any schema");
        self.grid.empty = EmptyState::new("Query failed").hint(message);
    }

    fn rebuild_chips(&mut self) {
        self.chips.chips = self
            .filters
            .iter()
            .map(|f| {
                let mut c = Chip::new(&f.chip_label());
                c.enabled = f.enabled;
                c
            })
            .collect();
        self.chips.lead = Some(if self.match_all {
            "match all ▾".into()
        } else {
            "match any ▾".into()
        });
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

        let mut ddl = vec![format!("CREATE TABLE {} (", table.qualified())];
        for (i, c) in table.columns.iter().enumerate() {
            let mut line = format!("    {} {}", c.name, c.ty.sql());
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
            let mut line = format!("    CONSTRAINT {} {} {}", k.name, k.kind, k.definition);
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
            ddl.push(format!(
                "CREATE INDEX {} ON {} USING {} ({});",
                ix.name,
                table.qualified(),
                ix.method,
                ix.columns.join(", ")
            ));
        }
        if let Some(c) = &table.comment {
            ddl.push(format!(
                "COMMENT ON TABLE {} IS '{}';",
                table.qualified(),
                c
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
                h.extend(hint_key(km, ChipCmd::Add).map(|k| hint(k, "Add")));
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
                if !self.filters.is_empty() || ctx.interaction.focused(self.chips.id) {
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
                if let Some((c, d)) = self.sort {
                    parts.push((
                        format!(
                            "sort {} {}",
                            self.columns[c].0,
                            if d == SortDir::Asc { "▴" } else { "▾" }
                        ),
                        4,
                    ));
                }
                let active = self.filters.iter().filter(|f| f.enabled).count();
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
                    let what = self.structure_tabs.items[self.structure_tabs.active]
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
    pub label: String,
    pub pinned: bool,
    pub anchor: Range<usize>,
    pub duration_ms: u32,
    pub body: ResultBody,
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

        let pinned: Vec<ResultSet> = self.results.drain(..).filter(|r| r.pinned).collect();
        self.results = pinned;
        self.editor.set_running(Some(statements[0].1.clone()));
        self.editor.diagnostics.clear();
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
        if let Some(r) = self.running.take() {
            self.editor.set_running(None);
            let (_, range) = r.statements[r.current].clone();
            self.results.push(ResultSet {
                label: "Cancelled".into(),
                pinned: false,
                anchor: range,
                duration_ms: r.started_ticks * 80,
                body: ResultBody::Cancelled,
            });
            self.sync_result_tabs();
            self.last_status = Some(("cancelled".into(), false));
            return true;
        }
        false
    }

    pub fn tick(&mut self, cat: &Catalog, connection: &str, database: &str) -> Vec<HistoryEntry> {
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
        let (result, entry) = execute(
            cat,
            &sql_text,
            range.clone(),
            explain,
            connection,
            database,
            self.result_counter + 1,
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
            self.editor.diagnostics.push(Diagnostic {
                range: start..end,
                severity: Severity::Error,
                message: message.clone(),
            });
        }
        r.results.push(result);
        out.push(entry);
        if failed || r.current + 1 >= r.statements.len() {
            let done = self.running.take().unwrap();
            self.editor.set_running(None);
            let n = done.results.len();
            let total_ms: u32 = done.results.iter().map(|r| r.duration_ms).sum();
            let last_failed = failed;
            self.results.extend(done.results);
            self.sync_result_tabs();
            self.active_result = self.results.len().saturating_sub(1);
            self.result_tabs.set_active(self.active_result);
            self.last_duration = Some(total_ms);
            if last_failed {
                let msg = if done.all && n > 1 {
                    format!("Statement {n}/{} failed", done.statements.len())
                } else {
                    "error".into()
                };
                self.last_status = Some((msg, true));
            } else {
                self.last_status = Some((
                    format!(
                        "{} · {}",
                        if n > 1 {
                            format!("{n} statements")
                        } else {
                            "ok".into()
                        },
                        duration(
                            std::time::Duration::from_millis(u64::from(total_ms)),
                            DurationStyle::Compact
                        )
                    ),
                    false,
                ));
            }
            self.unseen = true;
        } else {
            r.current += 1;
            r.ticks_left = 4;
            let next = r.statements[r.current].1.clone();
            self.editor.set_running(Some(next));
        }
        out
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
            self.sync_result_tabs();
            self.active_result = self.active_result.min(self.results.len().saturating_sub(1));
            self.result_tabs.set_active(self.active_result);
        }
    }

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

    pub fn trigger_completion(&mut self, cat: &Catalog, manual: bool) {
        let cur = self.editor.cursor_offset();
        let text = self.editor.text();
        if !manual && !model::auto_trigger(text, cur) {
            self.completion.close();
            return;
        }
        let (items, replace) = model::complete(cat, text, cur);
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
        self.completion.apply(i, &mut self.editor.field.buffer);
        self.completion.close();
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

    pub fn on_key(&mut self, key: &Key, cx: &mut Cx, cat: &Catalog) -> Outcome {
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
                self.trigger_completion(cat, true);
                return Outcome::Changed;
            }
            let (o, ev) = self.editor.on_key(key);
            match ev {
                Some(EditorAction::Changed) => {
                    self.editor.diagnostics.clear();
                    self.trigger_completion(cat, false);
                }
                Some(EditorAction::CursorMoved) => {
                    if self.completion.is_open() {
                        self.trigger_completion(cat, false);
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
    cat: &Catalog,
    text: &str,
    range: Range<usize>,
    explain: Option<bool>,
    connection: &str,
    database: &str,
    n: usize,
) -> (ResultSet, HistoryEntry) {
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
        Statement::Select(sel) => match sql::run_select(cat, &sel) {
            Ok(rs) => {
                let table = cat.find(sel.schema.as_deref(), &sel.table);
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
                if let Err(e) = grid.try_set_rows(GridRows { rows, total, more }) {
                    let message = e.to_string();
                    entry.error = Some(message.clone());
                    return (
                        ResultSet {
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
                entry.duration_ms = Some(rs.duration_ms);
                entry.rows = Some(rs.rows.len());
                (
                    ResultSet {
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
            Statement::Select(sel) => match sql::explain(cat, &sel, analyze) {
                Ok(plan) => {
                    let mut nodes = vec![];
                    let root = plan_to_tree(&plan, &mut nodes, max_total_cost(&plan));
                    let mut tree = TreeView::new(WidgetId::of("plan").child(n), vec![root]);
                    tree.expand_all();
                    let mut raw_lines = vec![];
                    sql::plan_text(&plan, 0, &mut raw_lines);
                    let planning = 0.21 + sel.predicates.len() as f64 * 0.09;
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
            let table = target.as_deref().and_then(|t| cat.find(None, t));
            let (rows, ms) = match &other {
                Statement::Update { has_where, .. } | Statement::Delete { has_where, .. } => {
                    let n = table.map(|t| t.row_count).unwrap_or(0);
                    if *has_where {
                        ((n / 150).max(1), 12 + (n / 40_000) as u32)
                    } else {
                        (n, 40 + (n / 5_000) as u32)
                    }
                }
                Statement::Insert { .. } => (1, 4),
                Statement::Truncate { .. } => (table.map(|t| t.row_count).unwrap_or(0), 30),
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

pub struct HistoryTab {
    pub key: ItemKey,
    pub id: WidgetId,
    pub list: ListBox,
    pub detail: CodeEditor,
    pub search: TextInput,
    pub scope_all: bool,
    pub failed_only: bool,

    pub slow_ms: u32,
    pub filtered: Vec<usize>,
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
        self.filtered = matches.iter().map(|e| e.id).collect();
        let items: Vec<ListItem> = matches
            .iter()
            .map(|e| {
                let mut label = e.first_line();
                if !e.ok() {
                    label = label.to_string();
                }
                let meta = format!("{} · {}", e.when(), e.duration());
                let mut it = ListItem::new(&junie_tui::ui::text::truncate(&label, 90)).meta(&meta);
                if e.duration_ms.is_some_and(|ms| ms > self.slow_ms) {
                    it = it.meta_tone(Tone::Warning);
                }
                it.disabled = false;
                it
            })
            .collect();
        let cursor = self.list.cursor;
        self.list = ListBox::new(self.id.sub("list"), items, SelectMode::Single).empty_text(
            if q.is_empty() {
                "No query history"
            } else {
                "No matching queries"
            },
        );
        self.list.cursor = cursor.min(self.filtered.len().saturating_sub(1));
        self.list.chosen = if self.filtered.is_empty() {
            None
        } else {
            Some(self.list.cursor)
        };
        self.sync_detail(history);
    }

    pub fn current_entry<'a>(&self, history: &'a History) -> Option<&'a HistoryEntry> {
        let id = *self.filtered.get(self.list.cursor)?;
        history.entries.iter().find(|e| e.id == id)
    }

    fn sync_detail(&mut self, history: &History) {
        let text = self
            .current_entry(history)
            .map(|e| e.sql.clone())
            .unwrap_or_default();
        if self.detail.text() != text {
            self.detail.set_text(&text);
        }
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
            .filter_map(|id| history.entries.iter().find(|e| e.id == *id))
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
                Rect::new(inner.x, ay, inner.width, 1),
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
