use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::forward_blur;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::theme::{EnvKind, Glyphs, SyntaxTone, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Maximized, Split, SplitDir, StackChild};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::code::{CodeEditor, Diagnostic, EditorAction, Severity};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::render_indeterminate;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction};
use junie_tui::widgets::tabs::{CloseReveal, TabAction, TabItem, Tabs};
use junie_tui::widgets::toolbar::{ToolItem, Toolbar, ToolbarAction};
use junie_tui::widgets::tree::{TreeAction, TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("queryworkbench");
const OUTER: WidgetId = WidgetId::of("queryworkbench").sub("outer");
const INNER: WidgetId = WidgetId::of("queryworkbench").sub("inner");

const RUN_TICKS: u8 = 6;

const WORK_MIN: u16 = 15;

const TOOLBAR_MIN: u16 = 20;

const TABLES: &[(&str, &str)] = &[
    ("organizations", "1.2k"),
    ("customers", "48k"),
    ("products", "3.1k"),
    ("orders", "1.2M"),
    ("order_items", "4.8M"),
    ("payments", "1.1M"),
    ("subscriptions", "9.4k"),
];

const FIRST_QUERY: &str = "\
-- Pending orders, newest first
SELECT id, customer, status, total, created_at
FROM orders
WHERE status = 'pending'
ORDER BY created_at DESC
LIMIT 25;
";

const KEYWORDS: &[&str] = &[
    "select", "from", "where", "order", "by", "limit", "and", "or", "not", "in", "is", "null",
    "join", "left", "inner", "on", "group", "having", "as", "desc", "asc", "explain", "analyze",
    "insert", "update", "delete", "set", "values", "into", "count", "distinct",
];

fn highlight(src: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            let end = src[i..].find('\n').map_or(b.len(), |n| i + n);
            out.push((i..end, SyntaxTone::Comment));
            i = end;
        } else if c == b'\'' {
            let end = src[i + 1..].find('\'').map_or(b.len(), |n| i + n + 2);
            out.push((i..end, SyntaxTone::Str));
            i = end;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            out.push((start..i, SyntaxTone::Number));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = src[start..i].to_ascii_lowercase();
            if KEYWORDS.contains(&word.as_str()) {
                out.push((start..i, SyntaxTone::Keyword));
            }
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome2 {
    Rows {
        table: String,
        count: usize,
        ms: u32,
    },

    Plan {
        ms: u32,
    },

    Error {
        message: String,
        detail: String,
    },

    None,
}

struct Doc {
    title: String,
    sql: String,
    result: Outcome2,
}

fn explorer_nodes() -> Vec<TreeNode> {
    let tables = TABLES
        .iter()
        .map(|(t, n)| TreeNode::leaf_meta(t, n).glyph("T"))
        .collect();
    vec![
        TreeNode::dir(
            "acme_prod",
            vec![
                TreeNode::dir(
                    "public",
                    vec![
                        TreeNode::dir("Tables", tables),
                        TreeNode::lazy("Views"),
                        TreeNode::lazy("Functions"),
                    ],
                )
                .glyph("S"),
                TreeNode::lazy("analytics").glyph("S"),
                TreeNode::lazy("audit").glyph("S"),
            ],
        )
        .glyph("D"),
    ]
}

fn lazy_children(label: &str) -> Vec<TreeNode> {
    match label {
        "Views" => vec![
            TreeNode::leaf("active_customers").glyph("V"),
            TreeNode::leaf("monthly_revenue").glyph("V"),
        ],
        "Functions" => vec![TreeNode::leaf("refresh_totals()").glyph("ƒ")],
        "analytics" => vec![TreeNode::leaf_meta("events", "88M").glyph("T")],
        _ => vec![TreeNode::note("No objects")],
    }
}

const CUSTOMERS: &[&str] = &[
    "Northwind Traders",
    "Globex",
    "Initech",
    "Umbrella Health",
    "Stark Logistics",
    "Wayne Foods",
    "Acme Retail",
    "Hooli",
];

fn result_table(table: &str, count: usize) -> DataTable {
    let id = ID.sub("results");
    let (cols, rows): (Vec<Column>, Vec<Vec<Cell>>) = match table {
        "orders" => (
            vec![
                Column::new("id", Constraint::Length(7)),
                Column::new("customer", Constraint::Min(16)),
                Column::new("status", Constraint::Length(10)),
                Column::new("total", Constraint::Length(10)).right(),
                Column::new("created_at", Constraint::Length(17)),
            ],
            (0..count)
                .map(|i| {
                    let status = ["pending", "pending", "paid", "shipped"][i % 4];
                    let tone = match status {
                        "pending" => Tone::Warning,
                        "paid" => Tone::Success,
                        _ => Tone::Secondary,
                    };
                    vec![
                        Cell::new(format!("{}", 10_480 - i * 7)),
                        Cell::new(CUSTOMERS[i % CUSTOMERS.len()]),
                        Cell::new(status).tone(tone),
                        Cell::new(format!("{}.{:02}", 40 + (i * 37) % 900, (i * 13) % 100)),
                        Cell::new(format!(
                            "2026-10-{:02} {:02}:{:02}",
                            17 - i / 6,
                            23 - i % 24,
                            (i * 7) % 60
                        )),
                    ]
                })
                .collect(),
        ),
        _ => (
            vec![
                Column::new("id", Constraint::Length(7)),
                Column::new("name", Constraint::Min(16)),
                Column::new("created_at", Constraint::Length(11)),
            ],
            (0..count)
                .map(|i| {
                    vec![
                        Cell::new(format!("{}", i + 1)),
                        Cell::new(format!(
                            "{} {}",
                            CUSTOMERS[i % CUSTOMERS.len()],
                            i / CUSTOMERS.len() + 1
                        )),
                        Cell::new(format!("2026-09-{:02}", 1 + i % 28)),
                    ]
                })
                .collect(),
        ),
    };
    DataTable::new(id, cols, rows)
        .numeric(&[0])
        .empty(EmptyState::new("0 rows").hint("The query ran; nothing matched"))
}

fn plan_table() -> DataTable {
    let rows = [
        ("Limit", "0.42..2.10", "25"),
        ("  └ Index Scan Backward on orders", "0.42..8 410", "1 204"),
        ("      Index cond: status = 'pending'", "", ""),
        ("      Using orders_created_at_idx", "", ""),
    ];
    DataTable::new(
        ID.sub("results"),
        vec![
            Column::new("Plan node", Constraint::Min(30)),
            Column::new("Cost", Constraint::Length(12)).right(),
            Column::new("Rows", Constraint::Length(7)).right(),
        ],
        rows.iter()
            .map(|(a, b, c)| {
                vec![
                    Cell::new(*a),
                    Cell::new(*b).tone(Tone::Secondary),
                    Cell::new(*c),
                ]
            })
            .collect(),
    )
}

fn from_table(sql: &str) -> Option<(String, Range<usize>)> {
    let lower = sql.to_ascii_lowercase();
    let at = lower.find("from ")? + 5;
    let rest = &sql[at..];
    let start = at + (rest.len() - rest.trim_start().len());
    let end = sql[start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(sql.len(), |n| start + n);
    (end > start).then(|| (sql[start..end].to_owned(), start..end))
}

fn toolbar_items(running: bool) -> Vec<ToolItem> {
    let off = |item: ToolItem, off: bool, why: &str| if off { item.reason(why) } else { item };
    vec![
        off(
            ToolItem::new("Run").shortcut("Ctrl+R").tip("Run the query"),
            running,
            "A query is running",
        ),
        off(
            ToolItem::new("Explain").tip("Show the query plan"),
            running,
            "A query is running",
        ),
        off(
            ToolItem::new("Stop")
                .tip("Stop the running query")
                .separator(),
            !running,
            "No query is running",
        ),
        ToolItem::new("Clear").tip("Empty the editor"),
    ]
}

pub fn page() -> Box<dyn Page> {
    Box::new(QueryWorkbenchPage::new())
}

pub struct QueryWorkbenchPage {
    template: Template,
    opts: PageValues,
    pub filter: TextInput,
    pub explorer: TreeView,

    loading: Vec<(Vec<usize>, u8)>,
    pub tabs: Tabs,
    docs: Vec<Doc>,

    opened: usize,
    pub toolbar: Toolbar,
    pub editor: CodeEditor,
    pub results: DataTable,

    running: Option<(usize, bool, u8)>,
    outer: Split,
    inner: Split,
    outer_seam: Splitter,
    inner_seam: Splitter,
    outer_area: Rect,
    inner_area: Rect,
    sections: crate::pages::layout::Sections,
}

impl QueryWorkbenchPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::QueryWorkbench));
        let mut explorer = TreeView::new(ID.sub("explorer"), explorer_nodes());
        explorer.expanded.insert(vec![0]);
        explorer.expanded.insert(vec![0, 0]);
        explorer.expanded.insert(vec![0, 0, 0]);
        explorer.flatten();
        let docs = vec![
            Doc {
                title: "Query 1".into(),
                sql: FIRST_QUERY.into(),
                result: Outcome2::Rows {
                    table: "orders".into(),
                    count: 25,
                    ms: 15,
                },
            },
            Doc {
                title: "customers".into(),
                sql: "SELECT * FROM customers LIMIT 100;\n".into(),
                result: Outcome2::None,
            },
        ];
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts,
            filter: TextInput::new(ID.sub("filter"), "")
                .placeholder("Filter objects")
                .plain_label(),
            explorer,
            loading: Vec::new(),
            tabs: Tabs::with_items(ID.sub("tabs"), vec![]),
            docs,
            opened: 1,
            toolbar: Toolbar::new(ID.sub("toolbar"), toolbar_items(false)),
            editor: CodeEditor::new(ID.sub("editor"), FIRST_QUERY),
            results: result_table("orders", 25),
            running: None,
            outer: Split::new(32, 22, 40),
            inner: Split::new(40, 5, 6),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
            outer_seam: Splitter::new(OUTER, SplitDir::Horizontal),
            inner_seam: Splitter::new(INNER, SplitDir::Vertical),
            outer_area: Rect::ZERO,
            inner_area: Rect::ZERO,
        };
        page.rebuild();
        page
    }

    fn env(&self) -> EnvKind {
        match self.opts.value("queryworkbench.env") {
            "staging" => EnvKind::Staging,
            "development" => EnvKind::Development,
            "local" => EnvKind::Local,
            _ => EnvKind::Production,
        }
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let active = self.tabs.active;
        self.tabs = Tabs::with_items(ID.sub("tabs"), self.tab_items())
            .allow_new(o.on("queryworkbench.allow_new"))
            .overflow_menu(true)
            .close_reveal(match o.value("queryworkbench.close_reveal") {
                "hover" => CloseReveal::Hover,
                _ => CloseReveal::Always,
            });
        self.tabs
            .set_active(active.min(self.docs.len().saturating_sub(1)));
        let explorer_pct = o.number("queryworkbench.explorer_width").unwrap_or(32);
        let mut outer = Split::new(explorer_pct, 22, 40);
        if !o.on("queryworkbench.explorer") {
            outer.toggle_max(Maximized::Second);
        }
        self.outer = outer;
        self.inner = Split::new(o.number("queryworkbench.editor_share").unwrap_or(40), 5, 6);
        self.load_editor();
        if let Some(r) = self.result_option() {
            if let Some(d) = self.docs.get_mut(self.tabs.active) {
                d.result = r;
            }
            self.sync_results();
        }
    }

    fn result_option(&self) -> Option<Outcome2> {
        Some(match self.opts.value("queryworkbench.result") {
            "empty" => Outcome2::Rows {
                table: "orders".into(),
                count: 0,
                ms: 9,
            },
            "error" => Outcome2::Error {
                message: "relation \"ordrs\" does not exist".into(),
                detail: "SQLSTATE 42P01 · line 3 · Did you mean \"orders\"?".into(),
            },
            "plan" => Outcome2::Plan { ms: 2 },
            _ => return None,
        })
    }

    fn tab_items(&self) -> Vec<TabItem> {
        self.docs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let busy = self.running.is_some_and(|(t, _, _)| t == i);
                TabItem::new(&d.title)
                    .prefix(if d.title.starts_with("Query") {
                        "≡"
                    } else {
                        "▦"
                    })
                    .closable()
                    .busy(busy)
                    .error(matches!(d.result, Outcome2::Error { .. }))
            })
            .collect()
    }

    fn sync_tabs(&mut self) {
        let items = self.tab_items();
        for (t, it) in self.tabs.items.iter_mut().zip(items) {
            t.busy = it.busy;
            t.error = it.error;
        }
    }

    fn load_editor(&mut self) {
        let sql = self
            .docs
            .get(self.tabs.active)
            .map_or(String::new(), |d| d.sql.clone());
        self.editor = CodeEditor::new(ID.sub("editor"), &sql)
            .highlighter(highlight)
            .read_only(self.opts.on("queryworkbench.read_only"))
            .placeholder("Write a query, then Ctrl+R");
        self.sync_results();
    }

    fn store_editor(&mut self) {
        let text = self.editor.text().to_owned();
        if let Some(d) = self.docs.get_mut(self.tabs.active) {
            d.sql = text;
        }
    }

    fn sync_results(&mut self) {
        self.editor.diagnostics.clear();
        match self.docs.get(self.tabs.active).map(|d| d.result.clone()) {
            Some(Outcome2::Rows { table, count, .. }) => {
                self.results = result_table(&table, count);
            }
            Some(Outcome2::Plan { .. }) => self.results = plan_table(),
            Some(Outcome2::Error { .. }) => {
                let sql = self.editor.text().to_owned();
                if let Some((_, range)) = from_table(&sql) {
                    self.editor.diagnostics.push(Diagnostic {
                        range,
                        severity: Severity::Error,
                        message: "unknown relation".into(),
                    });
                }
            }
            _ => {}
        }
    }

    fn switch_to(&mut self, i: usize) {
        if i == self.tabs.active && !self.docs.is_empty() {
            return;
        }
        self.store_editor();
        self.tabs.set_active(i);
        self.load_editor();
    }

    fn new_tab(&mut self, title: String, sql: &str) {
        self.store_editor();
        self.docs.push(Doc {
            title,
            sql: sql.into(),
            result: Outcome2::None,
        });
        self.tabs.set_items(self.tab_items());
        let n = self.docs.len() - 1;
        self.tabs.set_active(n);
        self.tabs.cursor = n;
        self.load_editor();
    }

    fn close_tab(&mut self, i: usize, cx: &mut Cx) {
        self.store_editor();
        let title = self.docs.remove(i).title;
        if self.running.is_some_and(|(t, _, _)| t == i) {
            self.running = None;
        }
        self.tabs.remove(i);
        self.load_editor();
        cx.status(format!("Closed {title}"));
    }

    fn run(&mut self, explain: bool, cx: &mut Cx) {
        if self.running.is_some() || self.docs.is_empty() {
            return;
        }
        self.store_editor();
        if self.editor.text().trim().is_empty() {
            cx.status("Nothing to run");
            return;
        }
        self.running = Some((self.tabs.active, explain, RUN_TICKS));
        self.toolbar.items = toolbar_items(true);
        self.sync_tabs();
        cx.status(if explain {
            "Explaining…"
        } else {
            "Running…"
        });
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.running.take().is_some() {
            self.toolbar.items = toolbar_items(false);
            self.sync_tabs();
            cx.status("Query cancelled");
        }
    }

    fn finish(&mut self, tab: usize, explain: bool, cx: &mut Cx) {
        self.toolbar.items = toolbar_items(false);
        let Some(doc) = self.docs.get_mut(tab) else {
            return;
        };
        let table = from_table(&doc.sql).map(|(t, _)| t);
        doc.result = match table {
            Some(t) if TABLES.iter().any(|(n, _)| *n == t) => {
                if explain {
                    Outcome2::Plan { ms: 2 }
                } else {
                    let count = if doc.sql.contains("LIMIT 25") { 25 } else { 40 };
                    Outcome2::Rows {
                        table: t,
                        count,
                        ms: 15,
                    }
                }
            }
            Some(t) => Outcome2::Error {
                message: format!("relation \"{t}\" does not exist"),
                detail: "SQLSTATE 42P01 · check the name in the explorer".into(),
            },
            None => Outcome2::Error {
                message: "syntax error: no FROM clause".into(),
                detail: "SQLSTATE 42601".into(),
            },
        };
        let msg = match &doc.result {
            Outcome2::Rows { count, ms, .. } => format!("✓ {count} rows · {ms} ms"),
            Outcome2::Plan { .. } => "✓ Plan ready".into(),
            Outcome2::Error { message, .. } => format!("✗ {message}"),
            Outcome2::None => String::new(),
        };
        self.sync_tabs();
        if tab == self.tabs.active {
            self.sync_results();
        }
        cx.status(msg);
    }

    fn tick(&mut self, cx: &mut Cx) -> Outcome {
        let mut changed = false;
        if let Some((tab, explain, left)) = self.running.as_mut() {
            changed = true;
            *left -= 1;
            if *left == 0 {
                let (tab, explain) = (*tab, *explain);
                self.running = None;
                self.finish(tab, explain, cx);
            }
        }
        for (_, left) in &mut self.loading {
            *left = left.saturating_sub(1);
            changed = true;
        }
        let done: Vec<Vec<usize>> = self
            .loading
            .iter()
            .filter(|(_, l)| *l == 0)
            .map(|(p, _)| p.clone())
            .collect();
        for p in done {
            let label = self
                .explorer
                .node(&p)
                .map_or(String::new(), |n| n.label.clone());
            self.explorer.set_children(&p, lazy_children(&label));
            self.loading.retain(|(q, _)| *q != p);
        }
        if changed {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn tree_action(&mut self, a: TreeAction, cx: &mut Cx) {
        match a {
            TreeAction::Expand(p) => {
                self.explorer.set_busy(&p, true);
                self.loading.push((p, RUN_TICKS));
            }
            TreeAction::Activate(p) => {
                let Some(node) = self.explorer.node(&p) else {
                    return;
                };
                let name = node.label.clone();
                if !TABLES.iter().any(|(t, _)| *t == name) {
                    return;
                }
                match self.docs.iter().position(|d| d.title == name) {
                    Some(i) => self.switch_to(i),
                    None => {
                        self.new_tab(name.clone(), &format!("SELECT * FROM {name} LIMIT 100;\n"))
                    }
                }
                cx.status(format!("Opened {name}"));
            }
        }
    }

    fn tab_action(&mut self, a: TabAction, cx: &mut Cx) {
        match a {
            TabAction::Activated(i) => self.switch_to(i),
            TabAction::Close(i) => self.close_tab(i, cx),
            TabAction::New => {
                self.opened += 1;
                self.new_tab(format!("Query {}", self.opened), "");
            }
        }
    }

    fn toolbar_action(&mut self, a: ToolbarAction, cx: &mut Cx) {
        if let ToolbarAction::Activated(i) = a {
            match i {
                0 => self.run(false, cx),
                1 => self.run(true, cx),
                2 => self.stop(cx),
                _ => {
                    self.editor.set_text("");
                    self.store_editor();
                }
            }
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let l = self.sections.layout(
            area,
            &[
                StackChild::flex(WORK_MIN, area.height.saturating_sub(2).max(WORK_MIN)),
                StackChild::fixed(1),
            ],
            Some(0),
            buf,
            ctx,
        );
        if let Some(body) = l.slot(0) {
            self.outer_area = body;
            let (left, right) = self.outer.layout(SplitDir::Horizontal, body, 1);
            let seam = self.outer.handle(SplitDir::Horizontal, body, 1);
            if !left.is_empty() {
                self.render_explorer(left, buf, ctx);
            }
            self.outer_seam.render(seam, buf, ctx, t.canvas);
            self.render_main(right, buf, ctx);
        }
        if let Some(status) = l.slot(1) {
            self.render_status(status, buf, ctx);
        }
    }

    fn render_explorer(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.explorer.id);
        let panel = Panel::card(Some("Explorer"))
            .meta("public")
            .focused(focused);
        let bg = panel.bg(t);
        let Some(inner) = panel.try_render(r, buf, t) else {
            return;
        };

        self.filter.render(
            Rect::new(inner.x.saturating_sub(1), inner.y, inner.width + 1, 2),
            buf,
            ctx,
            bg,
        );
        let tree = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        self.explorer.render(tree, buf, ctx, bg);
    }

    fn render_main(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        if r.height < 4 {
            return;
        }
        let (strip, body) = Tabs::layout(r);
        self.tabs.render(strip, buf, ctx, t.canvas);
        if self.docs.is_empty() {
            let hint = if self.tabs.allow_new {
                "Click + or press n on the tabs to open one"
            } else {
                "Open a table from the explorer"
            };
            let e = EmptyState::new("No query open").hint(hint);
            empty::render_ctx(body, buf, ctx, &e, t.canvas);
            return;
        }
        let top = if r.height >= TOOLBAR_MIN {
            self.toolbar
                .render(Rect::new(r.x, r.y + 2, r.width, 1), buf, ctx, t.canvas);
            4
        } else {
            3
        };
        let split = Rect::new(r.x, r.y + top, r.width, r.height.saturating_sub(top));
        self.inner_area = split;
        let (top, bottom) = self.inner.layout(SplitDir::Vertical, split, 1);
        let seam = self.inner.handle(SplitDir::Vertical, split, 1);
        self.render_editor(top, buf, ctx);
        self.inner_seam.render(seam, buf, ctx, t.canvas);
        if !bottom.is_empty() {
            self.render_results(bottom, buf, ctx);
        }
    }

    fn render_editor(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let title = self.docs[self.tabs.active].title.clone();
        let meta = if self.editor.read_only {
            "read-only".to_owned()
        } else {
            format!("{} lines", self.editor.text().lines().count().max(1))
        };
        let panel = Panel::card(Some(&title))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.editor.id));
        let bg = panel.bg(t);
        if let Some(inner) = panel.try_render(r, buf, t) {
            self.editor.render(inner, buf, ctx, bg);
        }
    }

    fn render_results(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let result = self.docs[self.tabs.active].result.clone();
        let running = self
            .running
            .is_some_and(|(tab, _, _)| tab == self.tabs.active);
        let meta = if running {
            "running".to_owned()
        } else {
            match &result {
                Outcome2::Rows { count, ms, .. } => format!(
                    "{} · {ms} ms",
                    junie_tui::ui::text::plural(*count, "row", "rows")
                ),
                Outcome2::Plan { ms } => format!("plan · {ms} ms"),
                Outcome2::Error { .. } => "failed".into(),
                Outcome2::None => "not run".into(),
            }
        };
        let title = if matches!(result, Outcome2::Plan { .. }) {
            "Plan"
        } else {
            "Results"
        };
        let panel = Panel::card(Some(title))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.results.id));
        let bg = panel.bg(t);
        let Some(inner) = panel.try_render(r, buf, t) else {
            return;
        };
        if running {
            render_indeterminate(
                Rect::new(inner.x, inner.y, inner.width.min(48), 1),
                buf,
                ctx,
                "Running  ",
                bg,
            );
            return;
        }
        match result {
            Outcome2::Rows { .. } | Outcome2::Plan { .. } => {
                self.results.render(inner, buf, ctx, bg);
            }
            Outcome2::Error { message, detail } => {
                let e = EmptyState::error(&message).hint(&detail);
                empty::render_ctx(inner, buf, ctx, &e, bg);
            }
            Outcome2::None => {
                let e = EmptyState::new("Not run yet").hint("Ctrl+R runs the query");
                empty::render_ctx(inner, buf, ctx, &e, bg);
            }
        }
    }

    fn render_status(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let env = self.env();
        let glyph = Glyphs::default().env(env);
        let name = match env {
            EnvKind::Production => "production",
            EnvKind::Staging => "staging",
            EnvKind::Development => "development",
            EnvKind::Local => "local",
        };
        let left = [
            Segment::new(name, env.tone())
                .lead(glyph, env.tone())
                .bold(),
            Segment::new("acme_prod › public", Tone::Secondary),
            if env == EnvKind::Production {
                Segment::new("safe mode", Tone::Success).lead("✓", Tone::Success)
            } else {
                Segment::new("writes allowed", Tone::Muted)
            },
        ];
        let tabs = junie_tui::ui::text::plural(self.docs.len(), "tab", "tabs");
        let right = [Segment::new(tabs, Tone::Muted).priority(1)];
        segments::render(r, buf, ctx, &left, &right, ctx.theme.canvas);
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        let seam = if id == OUTER {
            Some((&mut self.outer_seam, &mut self.outer, self.outer_area))
        } else if id == INNER {
            Some((&mut self.inner_seam, &mut self.inner, self.inner_area))
        } else {
            None
        };
        if let Some((seam, split, area)) = seam {
            return match ev.phase() {
                Some(Phase::Press) => {
                    seam.on_press(split, area, 1, pos);
                    Outcome::Consumed
                }
                Some(Phase::Drag) => seam.on_drag(split, area, 1, pos),
                Some(Phase::Click) => seam.on_click(split, ev.count()),
                _ => Outcome::Ignored,
            };
        }
        let click = ev.is(Phase::Click);
        if let Some(r) = self.filter.route(ev) {
            if click {
                cx.focus.focus(self.filter.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.explorer.route(ev) {
            if click && self.explorer.row_at(ev).is_some() {
                cx.focus.focus(self.explorer.id);
            }
            let (o, actions) = r.into_parts();
            for a in actions {
                self.tree_action(a, cx);
            }
            return o;
        }
        if let Some(r) = self.tabs.route(ev) {
            if click {
                cx.focus.focus(self.tabs.id);
            }
            let (o, actions) = r.into_parts();
            if !actions.is_empty() {
                for a in actions {
                    self.tab_action(a, cx);
                }
                return Outcome::Changed;
            }
            return o;
        }
        if let Some(r) = self.toolbar.route(ev) {
            let (o, actions) = r.into_parts();
            if !actions.is_empty() {
                for a in actions {
                    self.toolbar_action(a, cx);
                }
                return Outcome::Changed;
            }
            return o;
        }
        if let Some(r) = self.editor.route(ev) {
            if click && ev.bar.is_none() {
                cx.focus.focus(self.editor.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.results.route(ev) {
            if click && !scrollbar::is_bar_of(ev.id, self.results.id) {
                cx.focus.focus(self.results.id);
            }
            return r.outcome();
        }
        Outcome::Ignored
    }

    fn key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        if key.ctrl_char('r') {
            self.run(false, cx);
            return Outcome::Changed;
        }
        let Some(f) = cx.focus.current() else {
            return Outcome::Ignored;
        };
        if f == self.filter.id {
            let (o, a) = self.filter.on_key(key);
            match a {
                Some(InputAction::Changed) => {
                    let q = self.filter.text().to_owned();
                    self.explorer
                        .set_filter((!q.is_empty()).then_some(q.as_str()));
                }
                Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                _ => {}
            }
            return o;
        }
        if f == self.explorer.id {
            let (o, actions) = self.explorer.on_key(key).into_parts();
            if !actions.is_empty() {
                for a in actions {
                    self.tree_action(a, cx);
                }
                return Outcome::Changed;
            }
            return o;
        }
        if f == self.tabs.id {
            let (o, a) = self.tabs.on_key(key);
            if let Some(a) = a {
                self.tab_action(a, cx);
                return Outcome::Changed;
            }
            return o;
        }
        if f == self.toolbar.id {
            let (o, a) = self.toolbar.on_key(key);
            if let Some(a) = a {
                self.toolbar_action(a, cx);
                return Outcome::Changed;
            }
            return o;
        }
        if f == self.editor.id {
            let (o, a) = self.editor.on_key(key);
            match a {
                Some(EditorAction::Changed) => self.editor.diagnostics.clear(),
                Some(EditorAction::Leave { backward: false }) => cx.focus_next(),
                Some(EditorAction::Leave { backward: true }) => cx.focus_prev(),
                _ => {}
            }
            return o;
        }
        if f == self.results.id {
            let (o, actions) = self.results.on_key(key).into_parts();
            for a in actions {
                match a {
                    TableAction::LeaveForward => cx.focus_next(),
                    TableAction::LeaveBackward => cx.focus_prev(),
                    _ => {}
                }
            }
            return o;
        }
        Outcome::Ignored
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let strip = Tabs::with_items(
        id.child(0),
        vec![
            TabItem::new("Query 1").prefix("≡").closable(),
            TabItem::new("orders").prefix("▦").closable(),
        ],
    )
    .allow_new(true);
    let mut tree = TreeView::new(
        id.child(1),
        vec![
            TreeNode::dir(
                "public",
                vec![
                    TreeNode::leaf_meta("customers", "48k").glyph("T"),
                    TreeNode::leaf_meta("orders", "1.2M").glyph("T"),
                ],
            )
            .glyph("S"),
        ],
    );
    tree.expanded.insert(vec![0]);
    tree.flatten();
    StateGrid::new(id)
        .rows(&[Rest, Hover, Focused, Selected, Busy, Overflow, Narrow])
        .column_width(38)
        .column("Query tabs", strip)
        .column("Explorer", tree)
}

impl Page for QueryWorkbenchPage {
    fn title(&self) -> &'static str {
        "Query workbench"
    }
    fn blurb(&self) -> &'static str {
        "Explorer, query tabs, editor over results, status line"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev {
            return self.tick(cx);
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Blur(ch) => forward_blur(*ch, &mut [&mut self.filter]),
            PageEvent::Key(key) => self.key(key, cx),
            PageEvent::Paste(text) => {
                if cx.focus.is(self.filter.id) {
                    self.filter.on_paste(text)
                } else if cx.focus.is(self.editor.id) {
                    self.editor.on_paste(text)
                } else {
                    Outcome::Ignored
                }
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.filter.editing || self.editor.editing
    }

    fn animating(&self) -> bool {
        self.running.is_some() || !self.loading.is_empty()
    }

    fn leave_stakes(&self) -> junie_tui::ui::leave::Stakes {
        if self.running.is_some() {
            junie_tui::ui::leave::Stakes::Running {
                what: "The query".into(),
            }
        } else {
            junie_tui::ui::leave::Stakes::Clean
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        match focus {
            Some(f) if f == self.explorer.id => {
                vec![("↑↓", "Move"), ("←→", "Fold"), ("Enter", "Open table")]
            }
            Some(f) if f == self.tabs.id => vec![("←→", "Switch"), ("Enter", "Open")],
            Some(f) if f == self.results.id => vec![("↑↓←→", "Cell"), ("Ctrl+R", "Run")],
            _ => vec![("Ctrl+R", "Run"), ("Tab", "Next")],
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.store_editor();
            self.rebuild();
        }
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
