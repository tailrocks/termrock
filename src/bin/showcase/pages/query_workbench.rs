use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, forward_blur};
use junie_tui::core::id::{ItemKey, Part, PartRef, WidgetId};
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

/// One open query document. The editor (text, caret, selection, undo,
/// scroll) and the result view live with the document, so activation only
/// changes which document is routed and painted; it never copies text.
struct Doc {
    key: ItemKey,
    title: String,
    editor: CodeEditor,
    result: Outcome2,
    results: DataTable,
}

fn editor_id(key: ItemKey) -> WidgetId {
    ID.sub("editor").part(PartRef::item(Part::ROW, key))
}

fn results_id(key: ItemKey) -> WidgetId {
    ID.sub("results").part(PartRef::item(Part::ROW, key))
}

impl Doc {
    fn new(key: ItemKey, title: String, sql: &str, result: Outcome2, read_only: bool) -> Self {
        let mut doc = Self {
            key,
            title,
            editor: CodeEditor::new(editor_id(key), sql)
                .highlighter(highlight)
                .read_only(read_only)
                .placeholder("Write a query, then Ctrl+R"),
            result: Outcome2::None,
            results: result_table(results_id(key), "", 0),
        };
        doc.install(result);
        doc
    }

    /// Stores a new outcome and rebuilds this document's result view and
    /// diagnostics. Called only when a result is produced or configured.
    fn install(&mut self, result: Outcome2) {
        self.editor.set_diagnostics(Vec::new());
        match &result {
            Outcome2::Rows { table, count, .. } => {
                self.results = result_table(results_id(self.key), table, *count);
            }
            Outcome2::Plan { .. } => self.results = plan_table(results_id(self.key)),
            Outcome2::Error { .. } => {
                if let Some((_, range)) = from_table(self.editor.text()) {
                    self.editor.push_diagnostic(Diagnostic {
                        range,
                        severity: Severity::Error,
                        message: "unknown relation".into(),
                    });
                }
            }
            Outcome2::None => {}
        }
        self.result = result;
    }
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

fn result_table(id: WidgetId, table: &str, count: usize) -> DataTable {
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

fn plan_table(id: WidgetId) -> DataTable {
    let rows = [
        ("Limit", "0.42..2.10", "25"),
        ("  └ Index Scan Backward on orders", "0.42..8 410", "1 204"),
        ("      Index cond: status = 'pending'", "", ""),
        ("      Using orders_created_at_idx", "", ""),
    ];
    DataTable::new(
        id,
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
    /// Authoritative active document. The tab strip mirrors it. Tabs moves
    /// its own active index before it emits `Activated`, so the strip can
    /// already show a document the page has not activated yet.
    active_key: Option<ItemKey>,
    next_doc_key: u64,

    opened: usize,
    pub toolbar: Toolbar,

    running: Option<(ItemKey, bool, u8)>,
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
        let read_only = opts.on("queryworkbench.read_only");
        let first = ItemKey::num(0);
        let docs = vec![
            Doc::new(
                first,
                "Query 1".into(),
                FIRST_QUERY,
                Outcome2::Rows {
                    table: "orders".into(),
                    count: 25,
                    ms: 15,
                },
                read_only,
            ),
            Doc::new(
                ItemKey::num(1),
                "customers".into(),
                "SELECT * FROM customers LIMIT 100;\n",
                Outcome2::None,
                read_only,
            ),
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
            active_key: Some(first),
            next_doc_key: 2,
            opened: 1,
            toolbar: Toolbar::new(ID.sub("toolbar"), toolbar_items(false)),
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
        page.apply_result_option();
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

    fn doc_index(&self, key: ItemKey) -> Option<usize> {
        self.docs.iter().position(|doc| doc.key == key)
    }

    fn active_doc(&self) -> Option<&Doc> {
        let key = self.active_key?;
        self.docs.iter().find(|doc| doc.key == key)
    }

    fn active_doc_mut(&mut self) -> Option<&mut Doc> {
        let key = self.active_key?;
        self.docs.iter_mut().find(|doc| doc.key == key)
    }

    /// Keys are allocated monotonically and never reused. Exhaustion refuses
    /// the allocation before anything is inserted.
    fn alloc_key(&mut self) -> Option<ItemKey> {
        let n = self.next_doc_key;
        self.next_doc_key = n.checked_add(1)?;
        Some(ItemKey::num(n))
    }

    /// Rebuilds layout and strip options only. Document editors and result
    /// views are configured in place and keep their text, caret, undo and
    /// scroll state.
    fn rebuild(&mut self) {
        let o = &self.opts;
        self.tabs = Tabs::with_items(ID.sub("tabs"), self.tab_items())
            .allow_new(o.on("queryworkbench.allow_new"))
            .overflow_menu(true)
            .close_reveal(match o.value("queryworkbench.close_reveal") {
                "hover" => CloseReveal::Hover,
                _ => CloseReveal::Always,
            });
        let explorer_pct = o.number("queryworkbench.explorer_width").unwrap_or(32);
        let mut outer = Split::new(explorer_pct, 22, 40);
        if !o.on("queryworkbench.explorer") {
            outer.toggle_max(Maximized::Second);
        }
        self.outer = outer;
        self.inner = Split::new(o.number("queryworkbench.editor_share").unwrap_or(40), 5, 6);
        let read_only = o.on("queryworkbench.read_only");
        for doc in &mut self.docs {
            doc.editor.read_only = read_only;
        }
        self.sync_strip();
    }

    /// Installs the demonstration result chosen by the Result option on the
    /// active document, once, when that option changes.
    fn apply_result_option(&mut self) {
        if let (Some(result), Some(doc)) = (self.result_option(), self.active_doc_mut()) {
            doc.install(result);
        }
    }

    /// Points the strip at the authoritative active document.
    fn sync_strip(&mut self) {
        let index = self
            .active_key
            .and_then(|key| self.doc_index(key))
            .unwrap_or(0);
        self.tabs.set_active(index);
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
            .map(|d| {
                let busy = self.running.is_some_and(|(key, _, _)| key == d.key);
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

    /// Moves keyboard focus from a document's editor or result view to the
    /// same role in another document. Focus on the strip or elsewhere stays.
    fn retarget_focus(focus: &mut Focus, from: ItemKey, to: ItemKey) {
        if focus.is(editor_id(from)) {
            focus.focus(editor_id(to));
        } else if focus.is(results_id(from)) {
            focus.focus(results_id(to));
        }
    }

    /// The one activation transition. It compares the target against
    /// `active_key`, never against the strip, and never touches document
    /// text. Returns whether the active document changed.
    fn activate_document(&mut self, key: ItemKey, focus: &mut Focus) -> bool {
        let Some(index) = self.doc_index(key) else {
            return false;
        };
        let previous = self.active_key;
        self.tabs.set_active(index);
        if previous == Some(key) {
            return false;
        }
        if let Some(departing) = previous.and_then(|k| self.docs.iter_mut().find(|d| d.key == k)) {
            departing.editor.editing = false;
        }
        self.active_key = Some(key);
        if let Some(previous) = previous {
            Self::retarget_focus(focus, previous, key);
        }
        true
    }

    /// Checked adapter from a strip position to a document key.
    fn switch_to(&mut self, index: usize, focus: &mut Focus) -> bool {
        let Some(key) = self.docs.get(index).map(|doc| doc.key) else {
            return false;
        };
        self.activate_document(key, focus)
    }

    fn new_tab(&mut self, title: String, sql: &str, focus: &mut Focus) -> bool {
        let Some(key) = self.alloc_key() else {
            return false;
        };
        let read_only = self.opts.on("queryworkbench.read_only");
        self.docs
            .push(Doc::new(key, title, sql, Outcome2::None, read_only));
        self.tabs.set_items(self.tab_items());
        self.activate_document(key, focus);
        true
    }

    fn close_tab(&mut self, index: usize, cx: &mut Cx) {
        if index >= self.docs.len() {
            return;
        }
        let closed = self.docs.remove(index);
        if self.running.is_some_and(|(key, _, _)| key == closed.key) {
            self.running = None;
            self.toolbar.items = toolbar_items(false);
        }
        if self.active_key == Some(closed.key) {
            let next = self.docs.get(index).or(self.docs.last());
            self.active_key = next.map(|doc| doc.key);
            match self.active_key {
                Some(next) => Self::retarget_focus(cx.focus, closed.key, next),
                None => {
                    if cx.focus.is(editor_id(closed.key)) || cx.focus.is(results_id(closed.key)) {
                        cx.focus.focus(self.tabs.id);
                    }
                }
            }
        }
        self.tabs.remove(index);
        self.sync_strip();
        self.sync_tabs();
        cx.status(format!("Closed {}", closed.title));
    }

    fn run(&mut self, explain: bool, cx: &mut Cx) {
        if self.running.is_some() {
            return;
        }
        let Some(doc) = self.active_doc() else {
            return;
        };
        if doc.editor.text().trim().is_empty() {
            cx.status("Nothing to run");
            return;
        }
        self.running = Some((doc.key, explain, RUN_TICKS));
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

    fn finish(&mut self, key: ItemKey, explain: bool, cx: &mut Cx) {
        self.toolbar.items = toolbar_items(false);
        let Some(doc) = self.docs.iter_mut().find(|d| d.key == key) else {
            return;
        };
        let sql = doc.editor.text().to_owned();
        let table = from_table(&sql).map(|(t, _)| t);
        let result = match table {
            Some(t) if TABLES.iter().any(|(n, _)| *n == t) => {
                if explain {
                    Outcome2::Plan { ms: 2 }
                } else {
                    let count = if sql.contains("LIMIT 25") { 25 } else { 40 };
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
        let msg = match &result {
            Outcome2::Rows { count, ms, .. } => format!("✓ {count} rows · {ms} ms"),
            Outcome2::Plan { .. } => "✓ Plan ready".into(),
            Outcome2::Error { message, .. } => format!("✗ {message}"),
            Outcome2::None => String::new(),
        };
        doc.install(result);
        self.sync_tabs();
        cx.status(msg);
    }

    fn tick(&mut self, cx: &mut Cx) -> Outcome {
        let mut changed = false;
        if let Some((key, explain, left)) = self.running.as_mut() {
            changed = true;
            *left -= 1;
            if *left == 0 {
                let (key, explain) = (*key, *explain);
                self.running = None;
                self.finish(key, explain, cx);
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
                let opened = match self.docs.iter().position(|d| d.title == name) {
                    Some(i) => {
                        self.switch_to(i, cx.focus);
                        true
                    }
                    None => self.new_tab(
                        name.clone(),
                        &format!("SELECT * FROM {name} LIMIT 100;\n"),
                        cx.focus,
                    ),
                };
                if opened {
                    cx.status(format!("Opened {name}"));
                }
            }
        }
    }

    fn tab_action(&mut self, a: TabAction, cx: &mut Cx) {
        match a {
            TabAction::Activated(i) => {
                self.switch_to(i, cx.focus);
            }
            TabAction::Close(i) => self.close_tab(i, cx),
            TabAction::New => {
                self.opened += 1;
                self.new_tab(format!("Query {}", self.opened), "", cx.focus);
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
                    if let Some(doc) = self.active_doc_mut() {
                        doc.editor.set_text("");
                    }
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
        if self.active_doc().is_none() {
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
        let Some(doc) = self.active_doc_mut() else {
            return;
        };
        let meta = if doc.editor.read_only {
            "read-only".to_owned()
        } else {
            format!("{} lines", doc.editor.text().lines().count().max(1))
        };
        let panel = Panel::card(Some(&doc.title))
            .meta(&meta)
            .focused(ctx.interaction.focused(doc.editor.id));
        let bg = panel.bg(t);
        if let Some(inner) = panel.try_render(r, buf, t) {
            doc.editor.render(inner, buf, ctx, bg);
        }
    }

    fn render_results(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let running = self
            .running
            .is_some_and(|(key, _, _)| Some(key) == self.active_key);
        let Some(doc) = self.active_doc_mut() else {
            return;
        };
        let result = doc.result.clone();
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
            .focused(ctx.interaction.focused(doc.results.id));
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
                doc.results.render(inner, buf, ctx, bg);
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
            let (o, a) = r.into_parts();
            if let Some(a) = a {
                self.tree_action(a, cx);
            }
            return o;
        }
        if let Some(r) = self.tabs.route(ev) {
            if click {
                cx.focus.focus(self.tabs.id);
            }
            let (o, a) = r.into_parts();
            if let Some(a) = a {
                self.tab_action(a, cx);
                return Outcome::Changed;
            }
            return o;
        }
        if let Some(r) = self.toolbar.route(ev) {
            let (o, a) = r.into_parts();
            if let Some(a) = a {
                self.toolbar_action(a, cx);
                return Outcome::Changed;
            }
            return o;
        }
        let Some(doc) = self.active_doc_mut() else {
            return Outcome::Ignored;
        };
        if let Some(r) = doc.editor.route(ev) {
            if click && ev.bar.is_none() {
                cx.focus.focus(doc.editor.id);
            }
            return r.outcome();
        }
        if let Some(r) = doc.results.route(ev) {
            if click && !scrollbar::is_bar_of(ev.id, doc.results.id) {
                cx.focus.focus(doc.results.id);
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
            let (o, a) = self.explorer.on_key(key);
            if let Some(a) = a {
                self.tree_action(a, cx);
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
        let Some(doc) = self.active_doc_mut() else {
            return Outcome::Ignored;
        };
        if f == doc.editor.id {
            let (o, a) = doc.editor.on_key(key);
            match a {
                Some(EditorAction::Changed) => doc.editor.set_diagnostics(Vec::new()),
                Some(EditorAction::Leave { backward: false }) => cx.focus_next(),
                Some(EditorAction::Leave { backward: true }) => cx.focus_prev(),
                _ => {}
            }
            return o;
        }
        if f == doc.results.id {
            let (o, a) = doc.results.on_key(key);
            match a {
                Some(TableAction::LeaveForward) => cx.focus_next(),
                Some(TableAction::LeaveBackward) => cx.focus_prev(),
                _ => {}
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
                } else if let Some(doc) = self.active_doc_mut()
                    && cx.focus.is(doc.editor.id)
                {
                    doc.editor.on_paste(text)
                } else {
                    Outcome::Ignored
                }
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.filter.editing || self.active_doc().is_some_and(|d| d.editor.editing)
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
            Some(f) if self.active_doc().is_some_and(|d| f == d.results.id) => {
                vec![("↑↓←→", "Cell"), ("Ctrl+R", "Run")]
            }
            _ => vec![("Ctrl+R", "Run"), ("Tab", "Next")],
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let before = self.opts.get(key);
        if self.opts.set(key, value) {
            self.rebuild();
            if key == "queryworkbench.result" && self.opts.get(key) != before {
                self.apply_result_option();
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::Interaction;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::layout::Position;

    const AREA: Rect = Rect::new(0, 0, 140, 44);

    struct Rig {
        page: QueryWorkbenchPage,
        focus: Focus,
        ring: FocusRing,
        hits: HitRegistry,
    }

    impl Rig {
        fn new() -> Self {
            let mut rig = Self {
                page: QueryWorkbenchPage::new(),
                focus: Focus::default(),
                ring: FocusRing::default(),
                hits: HitRegistry::default(),
            };
            rig.paint();
            rig
        }

        fn paint(&mut self) -> Buffer {
            let theme = Theme::for_level(ColorLevel::TrueColor);
            let mut buf = Buffer::empty(AREA);
            let mut hits = HitRegistry::default();
            let mut ring = FocusRing::default();
            let interaction = Interaction {
                focus: self.focus.current(),
                ..Interaction::default()
            };
            let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
            self.page.render(AREA, &mut buf, &mut ctx);
            ctx.flush_overlays(&mut buf);
            self.hits = hits;
            self.ring = ring;
            buf
        }

        fn send(&mut self, ev: PageEvent) -> Outcome {
            let mut cx = Cx::new(&mut self.focus, &self.ring);
            let out = self.page.handle(&ev, &mut cx);
            self.paint();
            out
        }

        fn with_cx(&mut self, f: impl FnOnce(&mut QueryWorkbenchPage, &mut Cx)) {
            let mut cx = Cx::new(&mut self.focus, &self.ring);
            f(&mut self.page, &mut cx);
            self.paint();
        }

        fn key(&mut self, code: KeyCode, mods: KeyModifiers) {
            self.send(PageEvent::Key(Key { code, mods }));
        }

        fn run(&mut self) {
            self.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
        }

        fn click_tab(&mut self, i: usize) {
            let area = self.page.tabs.areas[i];
            let pos = Position::new(area.x + 1, area.y);
            let id = self.hits.hit(pos).expect("tab hit region");
            let ev = PointerEvent::new(&self.hits, Phase::Click, id, pos, 1, KeyModifiers::NONE);
            self.send(PageEvent::Pointer(ev));
        }

        fn type_into_editor(&mut self, text: &str) {
            let editor = &mut self.page.active_doc_mut().expect("active doc").editor;
            editor.begin_edit();
            let id = editor.id;
            self.focus.focus(id);
            self.send(PageEvent::Paste(text.to_owned()));
        }
    }

    const CUSTOMERS_SQL: &str = "SELECT * FROM customers LIMIT 100;\n";

    fn doc(p: &QueryWorkbenchPage, i: usize) -> &Doc {
        &p.docs[i]
    }

    fn texts(p: &QueryWorkbenchPage) -> Vec<String> {
        p.docs.iter().map(|d| d.editor.text().to_owned()).collect()
    }

    fn active(p: &QueryWorkbenchPage) -> &Doc {
        p.active_doc().expect("active doc")
    }

    #[derive(Debug, PartialEq)]
    struct EditorState {
        text: String,
        cursor: usize,
        selection: Option<String>,
        v_offset: usize,
        h_offset: usize,
    }

    fn editor_state(d: &Doc) -> EditorState {
        EditorState {
            text: d.editor.text().to_owned(),
            cursor: d.editor.cursor_offset(),
            selection: d.editor.field.buffer.selected_text().map(str::to_owned),
            v_offset: d.editor.scroll.v.offset,
            h_offset: d.editor.hscroll,
        }
    }

    /// Strip, active key, painted editor and result owner name one document.
    fn assert_agree(rig: &mut Rig) {
        let buf = rig.paint();
        let p = &rig.page;
        let key = p.active_key.expect("active key");
        let index = p.doc_index(key).expect("active key names a document");
        assert_eq!(p.tabs.active, index, "strip highlights the active document");
        assert_eq!(p.tabs.items[index].label, p.docs[index].title);
        let d = &p.docs[index];
        assert_eq!(d.editor.id, editor_id(key));
        assert_eq!(d.results.id, results_id(key));
        assert!(
            rig.hits.area_of(d.editor.id).is_some(),
            "editor is hit-registered"
        );
        for other in p.docs.iter().filter(|o| o.key != key) {
            assert!(
                rig.hits.area_of(other.editor.id).is_none(),
                "inactive editor {:?} is not routed",
                other.title
            );
        }
        let screen: String = buf.content().iter().map(|c| c.symbol()).collect();
        let top = d.editor.scroll.v.offset;
        let line = d.editor.text().lines().nth(top).unwrap_or("");
        // A horizontal scroll paints an ellipsis in the first column in place
        // of one character, so the visible text starts one column later.
        let skip = if d.editor.hscroll > 0 {
            d.editor.hscroll + 1
        } else {
            0
        };
        let probe: String = line.chars().skip(skip).take(12).collect();
        assert!(screen.contains(&probe), "screen paints {probe:?}");
    }

    #[test]
    fn issue52_click_activation_keeps_document_text() {
        let mut rig = Rig::new();
        rig.type_into_editor("-- scratch\n");
        let query1 = active(&rig.page).editor.text().to_owned();
        assert!(query1.starts_with("-- scratch"));

        rig.click_tab(1);
        assert_eq!(rig.page.tabs.active, 1, "strip highlights customers");
        assert_eq!(active(&rig.page).editor.text(), CUSTOMERS_SQL);
        assert_agree(&mut rig);

        rig.run();
        assert_eq!(texts(&rig.page), [query1.clone(), CUSTOMERS_SQL.into()]);

        rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
        assert_eq!(
            texts(&rig.page),
            [query1.clone(), CUSTOMERS_SQL.into(), String::new()]
        );
        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(2), cx));
        assert_eq!(texts(&rig.page), [query1, CUSTOMERS_SQL.into()]);
        assert_agree(&mut rig);
    }

    #[test]
    fn issue52_keyboard_overflow_explorer_activation() {
        let mut rig = Rig::new();
        rig.type_into_editor("-- one\n");
        rig.focus.focus(rig.page.tabs.id);

        rig.key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(active(&rig.page).title, "customers");
        assert_agree(&mut rig);
        rig.key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(active(&rig.page).title, "Query 1");
        assert!(active(&rig.page).editor.text().starts_with("-- one"));
        assert_agree(&mut rig);

        rig.key(KeyCode::Right, KeyModifiers::NONE);
        rig.run();
        assert!(doc(&rig.page, 0).editor.text().starts_with("-- one"));
        assert_eq!(doc(&rig.page, 1).editor.text(), CUSTOMERS_SQL);

        for n in 3..=14 {
            rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
            rig.type_into_editor(&format!("-- doc {n}\n"));
        }
        rig.with_cx(|p, cx| {
            p.switch_to(0, cx.focus);
        });
        rig.focus.focus(rig.page.tabs.id);
        rig.key(KeyCode::Down, KeyModifiers::NONE);
        assert!(rig.page.tabs.menu_open(), "overflow menu is open");
        rig.key(KeyCode::Enter, KeyModifiers::NONE);
        let chosen = rig.page.tabs.active;
        assert!(chosen > 1, "a previously hidden tab was chosen");
        assert_eq!(active(&rig.page).title, rig.page.tabs.items[chosen].label);
        assert!(active(&rig.page).editor.text().starts_with("-- doc"));
        assert_agree(&mut rig);

        rig.with_cx(|p, cx| {
            p.tree_action(TreeAction::Activate(vec![0, 0, 0, 1]), cx);
        });
        assert_eq!(active(&rig.page).title, "customers");
        assert_eq!(active(&rig.page).editor.text(), CUSTOMERS_SQL);
        assert_agree(&mut rig);
    }

    fn enter_edit(rig: &mut Rig) {
        let editor = &mut rig.page.active_doc_mut().expect("active doc").editor;
        editor.begin_edit();
        let id = editor.id;
        rig.focus.focus(id);
    }

    fn switch(rig: &mut Rig, index: usize) {
        rig.with_cx(|p, cx| {
            p.switch_to(index, cx.focus);
        });
    }

    #[test]
    fn issue52_editor_state_retained_per_document() {
        let mut rig = Rig::new();
        let results_a = active(&rig.page).results.id;
        rig.focus.focus(results_a);
        for _ in 0..4 {
            rig.key(KeyCode::Down, KeyModifiers::NONE);
        }
        let result_a = (
            active(&rig.page).results.cursor_row,
            active(&rig.page).results.scroll.v.offset,
        );
        assert_eq!(result_a.0, 4);

        let long: String = (0..60)
            .map(|i| format!("-- line {i} {}\n", "x".repeat(150)))
            .collect();
        rig.type_into_editor(&long);
        rig.page
            .active_doc_mut()
            .expect("active doc")
            .editor
            .jump_to(10);
        for _ in 0..3 {
            rig.key(KeyCode::Right, KeyModifiers::SHIFT);
        }
        {
            let editor = &mut rig.page.active_doc_mut().expect("active doc").editor;
            editor.scroll.v.scroll_to(5);
            editor.hscroll = 7;
        }
        rig.paint();
        let a = editor_state(active(&rig.page));
        assert_eq!(a.selection.as_deref(), Some("xxx"));
        assert_eq!((a.v_offset, a.h_offset), (5, 7));

        switch(&mut rig, 1);
        enter_edit(&mut rig);
        rig.send(PageEvent::Paste("-- b\n".into()));
        let b = editor_state(active(&rig.page));
        assert!(b.text.starts_with("-- b\nSELECT"));

        for _ in 0..3 {
            switch(&mut rig, 0);
            assert_eq!(editor_state(active(&rig.page)), a);
            assert_eq!(
                (
                    active(&rig.page).results.cursor_row,
                    active(&rig.page).results.scroll.v.offset
                ),
                result_a
            );
            switch(&mut rig, 1);
            assert_eq!(editor_state(active(&rig.page)), b);
        }

        enter_edit(&mut rig);
        rig.key(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(
            active(&rig.page).editor.text(),
            CUSTOMERS_SQL,
            "undo reverts B only"
        );
        assert_eq!(
            doc(&rig.page, 0).editor.text(),
            a.text,
            "A untouched by B's undo"
        );

        switch(&mut rig, 0);
        enter_edit(&mut rig);
        rig.key(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(
            active(&rig.page).editor.text(),
            FIRST_QUERY,
            "A's own undo history survived"
        );
        assert_eq!(doc(&rig.page, 1).editor.text(), CUSTOMERS_SQL);
    }

    #[test]
    fn issue52_close_inactive_preserves_active_key() {
        let mut rig = Rig::new();
        rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
        rig.type_into_editor("-- c\n");
        let keys: Vec<ItemKey> = rig.page.docs.iter().map(|d| d.key).collect();
        assert_eq!(keys.len(), 3);
        rig.click_tab(1);
        let b_key = keys[1];
        enter_edit(&mut rig);
        rig.send(PageEvent::Paste("-- b\n".into()));
        let b = editor_state(active(&rig.page));
        let b_results = active(&rig.page).results.cursor_row;

        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(0), cx));
        assert_eq!(rig.page.active_key, Some(b_key));
        assert_eq!(editor_state(active(&rig.page)), b);
        assert_eq!(active(&rig.page).results.cursor_row, b_results);
        assert_agree(&mut rig);

        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(1), cx));
        assert_eq!(rig.page.active_key, Some(b_key));
        assert_eq!(editor_state(active(&rig.page)), b);
        assert_agree(&mut rig);

        rig.focus.focus(active(&rig.page).editor.id);
        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(0), cx));
        assert!(rig.page.docs.is_empty());
        assert_eq!(rig.page.active_key, None);
        assert_eq!(
            rig.focus.current(),
            Some(rig.page.tabs.id),
            "focus falls back to the strip"
        );
        rig.paint();
        assert!(!rig.page.editing());
        rig.run();
        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(0), cx));
        rig.with_cx(|p, cx| {
            assert!(!p.switch_to(0, cx.focus));
        });

        rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
        let fresh = rig.page.active_key.expect("new doc is active");
        assert!(!keys.contains(&fresh), "keys are never reused");
        assert_agree(&mut rig);
    }

    #[test]
    fn issue52_options_and_noop_activation_do_not_reset() {
        let mut rig = Rig::new();
        rig.type_into_editor("-- keep\n");
        let focus_id = active(&rig.page).results.id;
        rig.focus.focus(focus_id);
        rig.key(KeyCode::Down, KeyModifiers::NONE);
        rig.key(KeyCode::Down, KeyModifiers::NONE);
        let before_a = editor_state(active(&rig.page));
        let rows_a = active(&rig.page).results.cursor_row;
        let customers = Outcome2::None;
        assert_eq!(rows_a, 2);

        rig.with_cx(|p, cx| {
            let key = p.active_key.expect("active key");
            assert!(
                !p.activate_document(key, cx.focus),
                "equal activation is a no-op"
            );
            assert!(!p.switch_to(0, cx.focus));
        });
        assert_eq!(editor_state(active(&rig.page)), before_a);

        for (key, value) in [
            ("queryworkbench.read_only", 1),
            ("queryworkbench.read_only", 0),
            ("queryworkbench.editor_share", 2),
            ("queryworkbench.allow_new", 0),
        ] {
            rig.page.set_option(key, value);
            assert_eq!(editor_state(active(&rig.page)), before_a, "{key}");
            assert_eq!(active(&rig.page).results.cursor_row, rows_a, "{key}");
        }
        assert!(active(&rig.page).editor.text().starts_with("-- keep"));
        rig.paint();
        enter_edit(&mut rig);
        rig.key(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert!(
            !active(&rig.page).editor.text().starts_with("-- keep"),
            "undo history survived rebuilds"
        );

        rig.page.set_option("queryworkbench.result", 3);
        assert!(matches!(active(&rig.page).result, Outcome2::Plan { .. }));
        assert_eq!(
            doc(&rig.page, 1).result,
            customers,
            "other documents keep their result"
        );
        let plan_cursor = active(&rig.page).results.cursor_row;
        rig.page.set_option("queryworkbench.explorer_width", 1);
        assert_eq!(active(&rig.page).results.cursor_row, plan_cursor);
    }

    #[test]
    fn issue52_switch_scopes_focus_to_active_document() {
        let mut rig = Rig::new();
        rig.type_into_editor("-- a\n");
        let a_key = rig.page.active_key.expect("key");
        let a_state = editor_state(active(&rig.page));
        rig.paint();
        let a_area = rig
            .hits
            .area_of(editor_id(a_key))
            .expect("A is hit-registered before the switch");
        let a_pos = Position::new(a_area.x + 1, a_area.y + 1);

        rig.with_cx(|p, cx| {
            p.tree_action(TreeAction::Activate(vec![0, 0, 0, 1]), cx);
        });
        let b_key = rig.page.active_key.expect("key");
        assert_ne!(a_key, b_key);
        assert_eq!(
            rig.focus.current(),
            Some(editor_id(b_key)),
            "focus follows the active document"
        );
        rig.paint();
        assert!(
            rig.hits.area_of(editor_id(a_key)).is_none(),
            "A has no hit region"
        );
        assert!(rig.hits.area_of(editor_id(b_key)).is_some());

        enter_edit(&mut rig);
        rig.send(PageEvent::Paste("-- b\n".into()));
        assert_eq!(
            rig.page.docs[0].editor.text(),
            a_state.text,
            "paste never reaches A"
        );

        // B holds a selection. Drag and Release carrying A's editor id, at A's
        // old geometry, must not reach A or B. Safety comes from the
        // key-scoped editor ids: no hit region or focus role names A now.
        rig.page
            .active_doc_mut()
            .expect("active doc")
            .editor
            .jump_to(5);
        for _ in 0..3 {
            rig.key(KeyCode::Right, KeyModifiers::SHIFT);
        }
        let b_before = editor_state(active(&rig.page));
        assert_eq!(b_before.selection.as_deref(), Some("SEL"));
        let focus_before = rig.focus.current();
        for phase in [Phase::Drag, Phase::Release] {
            let ev = PointerEvent::new(
                &rig.hits,
                phase,
                editor_id(a_key),
                a_pos,
                1,
                KeyModifiers::NONE,
            );
            let out = rig.send(PageEvent::Pointer(ev));
            assert_eq!(out, Outcome::Ignored, "{phase:?} with A's id");
        }
        assert_eq!(rig.page.active_key, Some(b_key));
        assert_eq!(
            editor_state(active(&rig.page)),
            b_before,
            "B text and selection"
        );
        assert_eq!(rig.page.docs[0].editor.text(), a_state.text, "A text");
        assert_eq!(
            rig.page.docs[0].editor.field.buffer.selected_text(),
            None,
            "A has no selection"
        );
        assert_eq!(rig.focus.current(), focus_before, "focus unchanged");

        rig.with_cx(|p, cx| {
            p.tree_action(TreeAction::Activate(vec![0, 0, 0, 1]), cx);
        });
        rig.with_cx(|p, cx| {
            p.switch_to(0, cx.focus);
        });
        assert_eq!(rig.focus.current(), Some(editor_id(a_key)));
        assert_eq!(editor_state(active(&rig.page)), a_state);
    }

    #[test]
    fn issue52_tab_click_round_trip_retains_each_document() {
        let mut rig = Rig::new();
        rig.focus.focus(active(&rig.page).results.id);
        for _ in 0..4 {
            rig.key(KeyCode::Down, KeyModifiers::NONE);
        }
        let result_a = (
            active(&rig.page).results.cursor_row,
            active(&rig.page).results.scroll.v.offset,
        );

        let long: String = (0..60)
            .map(|i| format!("-- line {i} {}\n", "x".repeat(150)))
            .collect();
        rig.type_into_editor(&long);
        rig.page
            .active_doc_mut()
            .expect("active doc")
            .editor
            .jump_to(10);
        for _ in 0..3 {
            rig.key(KeyCode::Right, KeyModifiers::SHIFT);
        }
        {
            let editor = &mut rig.page.active_doc_mut().expect("active doc").editor;
            editor.scroll.v.scroll_to(5);
            editor.hscroll = 7;
        }
        rig.paint();
        let a = editor_state(active(&rig.page));
        assert_eq!(a.selection.as_deref(), Some("xxx"));

        rig.focus.focus(rig.page.tabs.id);
        rig.paint();
        rig.click_tab(1);
        assert_eq!(rig.page.tabs.active, 1);
        assert_agree(&mut rig);

        enter_edit(&mut rig);
        rig.send(PageEvent::Paste("-- b\n".into()));
        let b = editor_state(active(&rig.page));

        rig.focus.focus(rig.page.tabs.id);
        rig.paint();
        rig.click_tab(0);
        assert_eq!(rig.page.tabs.active, 0);
        assert_agree(&mut rig);

        let a_back = editor_state(active(&rig.page));
        assert_eq!(a_back.text, a.text, "A's text");
        assert_eq!(a_back.cursor, a.cursor, "A's caret");
        assert_eq!(
            (a_back.v_offset, a_back.h_offset),
            (a.v_offset, a.h_offset),
            "A's scroll"
        );
        // Known loss, tracked in #105: painting an editor that
        // lost focus commits it (src/widgets/code.rs, `commit` on unfocused
        // paint), which clears the selection. Caret, undo and scroll survive.
        assert_eq!(a_back.selection, None, "A's selection is cleared on blur");
        assert_eq!(
            (
                active(&rig.page).results.cursor_row,
                active(&rig.page).results.scroll.v.offset
            ),
            result_a,
            "A's result view"
        );

        rig.click_tab(1);
        assert_eq!(editor_state(active(&rig.page)), b, "B's state");

        rig.click_tab(0);
        enter_edit(&mut rig);
        rig.key(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(
            active(&rig.page).editor.text(),
            FIRST_QUERY,
            "A's undo history survived the round trip"
        );
        assert_eq!(texts(&rig.page)[1], b.text, "B untouched by A's undo");
    }
}
