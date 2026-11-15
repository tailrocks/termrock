use std::borrow::Cow;
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
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
use junie_tui::core::response::Response;
use junie_tui::theme::{EnvKind, Glyphs, SyntaxTone, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Maximized, Split, SplitDir, StackChild};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::code::{CodeEditor, Diagnostic, EditorAction, Severity};
use junie_tui::widgets::document_workspace::{
    CloseDecision, CloseToken, DocumentWorkspace, OpenKind, OperationToken, WorkspaceAction,
    WorkspaceDocument,
};
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

/// The work in flight, captured when the run started. The SQL and mode are
/// owned copies, so later edits, tab moves or closes cannot change what runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunRequest {
    token: OperationToken,
    sql: String,
    explain: bool,
    ticks_left: u8,
}

/// One open query document. The editor (text, caret, selection, undo,
/// scroll) and the result view live with the document, so activation only
/// changes which document is routed and painted; it never copies text.
struct Doc {
    key: ItemKey,
    title: String,
    editor: CodeEditor,
    /// The text the document opened with; the document is dirty when the
    /// editor no longer holds it.
    opened_text: String,
    result: Outcome2,
    results: DataTable,
    /// The exact text `result` was produced from, and the run that made it.
    /// A demonstration or initial result has no run.
    result_sql: Option<String>,
    result_token: Option<OperationToken>,
}

/// Painting and view-only access to a [`Doc`]. Its field is private, so a
/// caller of [`DocumentWorkspace::view_mut`] cannot reach the editor's text.
pub struct DocView<'a> {
    document: &'a mut Doc,
}

impl DocView<'_> {
    /// Ends text editing, as when the document stops being the active one.
    fn leave(&mut self) {
        self.document.editor.editing = false;
    }

    /// Stores a run's outcome. A result is not editor content, so it does not
    /// stale a pending close.
    fn install_for(&mut self, result: Outcome2, sql: String, token: Option<OperationToken>) {
        self.document.install_for(result, sql, token);
    }

    /// Stores the demonstration outcome for the text now in the editor.
    fn install(&mut self, result: Outcome2) {
        self.document.install(result);
    }

    /// A wheel gesture over the editor. It scrolls; any other pointer input
    /// can open the edit menu and goes through [`DocumentWorkspace::update`].
    fn editor_wheel(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        ev.wheel()?;
        self.document.editor.route(ev).map(|r| r.outcome())
    }

    fn results_key(&mut self, key: &Key) -> Response<TableAction> {
        self.document.results.on_key(key)
    }

    fn results_route(&mut self, ev: &PointerEvent) -> Option<Response<TableAction>> {
        self.document.results.route(ev)
    }

    fn render_editor(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let doc = &mut *self.document;
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

    fn render_results(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, running: bool) {
        let t = ctx.theme;
        let doc = &mut *self.document;
        let earlier = doc.result_is_for_earlier_text();
        let result = doc.result.clone();
        let meta = if running {
            "running".to_owned()
        } else if earlier {
            "Result for earlier query text".to_owned()
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
}

impl WorkspaceDocument for Doc {
    type View<'a> = DocView<'a>;

    fn title(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.title)
    }

    fn dirty(&self) -> bool {
        self.editor.text() != self.opened_text
    }

    fn focus_target(&self) -> Option<WidgetId> {
        Some(self.editor.id)
    }

    fn view(&mut self) -> DocView<'_> {
        DocView { document: self }
    }
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
            opened_text: sql.to_owned(),
            result: Outcome2::None,
            results: result_table(results_id(key), "", 0),
            result_sql: None,
            result_token: None,
        };
        doc.install(result);
        doc
    }

    /// Stores a configured outcome for the text currently in the editor.
    fn install(&mut self, result: Outcome2) {
        let sql = self.editor.text().to_owned();
        self.install_for(result, sql, None);
    }

    /// Stores an outcome produced from `sql` and rebuilds this document's
    /// result view. Diagnostics are placed only when the editor still holds
    /// exactly `sql`, because their ranges are byte offsets into it.
    fn install_for(&mut self, result: Outcome2, sql: String, token: Option<OperationToken>) {
        self.editor.set_diagnostics(Vec::new());
        match &result {
            Outcome2::Rows { table, count, .. } => {
                self.results = result_table(results_id(self.key), table, *count);
            }
            Outcome2::Plan { .. } => self.results = plan_table(results_id(self.key)),
            Outcome2::Error { .. } => {
                if self.editor.text() == sql
                    && let Some((_, range)) = from_table(&sql)
                {
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
        self.result_sql = Some(sql);
        self.result_token = token;
    }

    /// A run produced the result and the editor no longer holds its text.
    fn result_is_for_earlier_text(&self) -> bool {
        self.result_token.is_some() && self.result_sql.as_deref() != Some(self.editor.text())
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
    DataTable::try_new(id, cols, rows)
        .expect("rectangular fixture")
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
    DataTable::try_new(
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
    .expect("rectangular fixture")
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
    /// A projection of the workspace: its items and active index are rebuilt
    /// from the ordered keys, and a strip event is translated back to a key
    /// before anything changes.
    pub tabs: Tabs,
    workspace: DocumentWorkspace<Doc>,
    /// The close request waiting for a second gesture when "Close with edits"
    /// is set to ask.
    pending_close: Option<CloseToken>,

    opened: usize,
    pub toolbar: Toolbar,

    running: Option<RunRequest>,
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
        let mut workspace = DocumentWorkspace::new(ID);
        let (first, _) = workspace.open_with(
            |key| {
                Doc::new(
                    key,
                    "Query 1".into(),
                    FIRST_QUERY,
                    Outcome2::Rows {
                        table: "orders".into(),
                        count: 25,
                        ms: 15,
                    },
                    read_only,
                )
            },
            OpenKind::Pinned,
        );
        let _ = workspace.open_with(
            |key| {
                Doc::new(
                    key,
                    "customers".into(),
                    "SELECT * FROM customers LIMIT 100;\n",
                    Outcome2::None,
                    read_only,
                )
            },
            OpenKind::Pinned,
        );
        let _ = workspace.activate(first);
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts,
            filter: TextInput::new(ID.sub("filter"), "")
                .placeholder("Filter objects")
                .plain_label(),
            explorer,
            loading: Vec::new(),
            tabs: Tabs::with_items(ID.sub("tabs"), vec![]),
            workspace,
            pending_close: None,
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

    fn active_doc(&self) -> Option<&Doc> {
        self.workspace.get(self.workspace.active_key()?)
    }

    fn open_mode(&self) -> OpenKind {
        match self.opts.value("queryworkbench.explorer_open") {
            "preview" => OpenKind::Preview,
            _ => OpenKind::Pinned,
        }
    }

    fn asks_before_discarding(&self) -> bool {
        self.opts.value("queryworkbench.dirty_close") == "ask"
    }

    /// Rebuilds layout and strip options only. Document editors and result
    /// views are configured in place and keep their text, caret, undo and
    /// scroll state.
    fn rebuild(&mut self) {
        let o = &self.opts;
        self.tabs = Tabs::with_items(ID.sub("tabs"), vec![])
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
        for key in self.workspace.ordered_keys().to_vec() {
            let _ = self
                .workspace
                .update(key, |doc| doc.editor.read_only = read_only);
        }
        self.sync_strip();
    }

    /// Installs the demonstration result chosen by the Result option on the
    /// active document, once, when that option changes.
    fn apply_result_option(&mut self) {
        if let (Some(result), Some(key)) = (self.result_option(), self.workspace.active_key())
            && let Ok(mut view) = self.workspace.view_mut(key)
        {
            view.install(result);
        }
    }

    /// Rebuilds the strip from the workspace: one item per document in order,
    /// the active tab following the active key. Showcase documents are never
    /// saved, so the strip shows no dirty mark.
    fn sync_strip(&mut self) {
        self.workspace.sync_tabs_with(&mut self.tabs, |doc, item| {
            item.prefix(if doc.title.starts_with("Query") {
                "≡"
            } else {
                "▦"
            })
            .dirty(false)
            .error(matches!(doc.result, Outcome2::Error { .. }))
        });
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

    /// A document is busy only while the workspace still holds the generation
    /// the page's request was started under.
    fn is_running(&self, key: ItemKey) -> bool {
        self.running.as_ref().is_some_and(|request| {
            request.token.document == key && self.workspace.running(key) == Some(request.token)
        })
    }

    fn running_doc(&self) -> Option<&Doc> {
        let key = self.running.as_ref()?.token.document;
        self.is_running(key).then(|| self.workspace.get(key))?
    }

    fn sync_toolbar(&mut self) {
        self.toolbar
            .set_items(toolbar_items(self.running.is_some()));
    }

    /// Drops the request when it belongs to `key` and cancels that document's
    /// operation, so the pending completion can no longer match.
    fn cancel_running_for(&mut self, key: ItemKey) -> bool {
        if self
            .running
            .as_ref()
            .is_none_or(|request| request.token.document != key)
        {
            return false;
        }
        self.running = None;
        let _ = self.workspace.cancel_operation(key);
        self.sync_toolbar();
        true
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

    /// Handles a workspace response in order. `previous` is the active key
    /// before the transition that produced it.
    fn apply(
        &mut self,
        previous: Option<ItemKey>,
        resp: Response<WorkspaceAction<Doc>>,
        cx: &mut Cx,
    ) {
        let mut closed = Vec::new();
        for action in resp.into_actions() {
            match action {
                WorkspaceAction::Activated(key) => {
                    if let Some(prev) = previous.filter(|prev| *prev != key) {
                        if let Ok(mut view) = self.workspace.view_mut(prev) {
                            view.leave();
                        }
                        Self::retarget_focus(cx.focus, prev, key);
                    }
                }
                WorkspaceAction::PreviewPromoted(_) => {}
                WorkspaceAction::CancelOperation(token) => {
                    if self.running.as_ref().is_some_and(|r| r.token == token) {
                        self.running = None;
                    }
                }
                WorkspaceAction::Closed { key, document } => {
                    if self.pending_close.is_some_and(|t| t.document == key) {
                        self.pending_close = None;
                    }
                    cx.status(format!("Closed {}", document.title));
                    closed.push(key);
                }
                WorkspaceAction::CloseRequested(token) => self.close_requested(token, cx),
            }
        }
        if self.workspace.active_key().is_none() {
            for key in closed {
                if cx.focus.is(editor_id(key)) || cx.focus.is(results_id(key)) {
                    cx.focus.focus(self.tabs.id);
                }
            }
        }
        self.sync_strip();
        self.sync_toolbar();
    }

    /// A document with edits or a running query asked to close. By default the
    /// page discards at once, as closing a query tab always did; with "Close
    /// with edits" set to ask, the same close gesture a second time confirms,
    /// and any edit in between makes the request stale.
    fn close_requested(&mut self, token: CloseToken, cx: &mut Cx) {
        let previous = self.workspace.active_key();
        if self.asks_before_discarding() && self.pending_close != Some(token) {
            self.pending_close = Some(token);
            let title = self
                .workspace
                .get(token.document)
                .map_or_else(String::new, |doc| doc.title.clone());
            cx.status(format!(
                "{title} has unsaved changes · close it again to discard, Esc keeps it"
            ));
            return;
        }
        self.pending_close = None;
        match self.workspace.resolve_close(token, CloseDecision::Discard) {
            Ok(resp) => self.apply(previous, resp, cx),
            Err(error) => cx.status(error.to_string()),
        }
    }

    /// Keeps the document the pending close names.
    fn cancel_pending_close(&mut self, cx: &mut Cx) -> bool {
        let Some(token) = self.pending_close.take() else {
            return false;
        };
        let previous = self.workspace.active_key();
        if let Ok(resp) = self.workspace.resolve_close(token, CloseDecision::Cancel) {
            self.apply(previous, resp, cx);
        }
        cx.status("Kept the tab open");
        true
    }

    /// The one activation transition: the workspace compares the target with
    /// its active key, never with the strip, and never touches document text.
    /// Returns whether the active document changed.
    fn activate_document(&mut self, key: ItemKey, cx: &mut Cx) -> bool {
        let previous = self.workspace.active_key();
        let resp = self.workspace.activate(key);
        let changed = resp.has_actions();
        self.apply(previous, resp, cx);
        changed
    }

    /// Checked adapter from a strip position to a document key.
    fn switch_to(&mut self, index: usize, cx: &mut Cx) -> bool {
        let Some(key) = self.workspace.key_at(index) else {
            return false;
        };
        self.activate_document(key, cx)
    }

    fn new_tab(&mut self, title: String, sql: &str, kind: OpenKind, cx: &mut Cx) -> bool {
        let read_only = self.opts.on("queryworkbench.read_only");
        let previous = self.workspace.active_key();
        let (_, resp) = self.workspace.open_with(
            |key| Doc::new(key, title, sql, Outcome2::None, read_only),
            kind,
        );
        self.apply(previous, resp, cx);
        true
    }

    fn close_tab(&mut self, index: usize, cx: &mut Cx) {
        let Some(key) = self.workspace.key_at(index) else {
            return;
        };
        let previous = self.workspace.active_key();
        let resp = self.workspace.request_close(key);
        self.apply(previous, resp, cx);
    }

    fn run(&mut self, explain: bool, cx: &mut Cx) {
        if let Some(pending) = self.running_doc() {
            cx.status(format!("A query is already running in {}", pending.title));
            return;
        }
        let Some(key) = self.workspace.active_key() else {
            return;
        };
        let Some(sql) = self
            .workspace
            .get(key)
            .map(|doc| doc.editor.text().to_owned())
        else {
            return;
        };
        if sql.trim().is_empty() {
            cx.status("Nothing to run");
            return;
        }
        let (token, resp) = match self.workspace.begin_operation(key) {
            Ok(started) => started,
            Err(error) => {
                cx.status(error.to_string());
                return;
            }
        };
        self.running = Some(RunRequest {
            token,
            sql,
            explain,
            ticks_left: RUN_TICKS,
        });
        let previous = self.workspace.active_key();
        self.apply(previous, resp, cx);
        cx.status(if explain {
            "Explaining…"
        } else {
            "Running…"
        });
    }

    fn stop(&mut self, cx: &mut Cx) {
        let Some(key) = self.running.as_ref().map(|request| request.token.document) else {
            return;
        };
        let title = self.workspace.get(key).map(|doc| doc.title.clone());
        self.cancel_running_for(key);
        self.sync_strip();
        cx.status(match title {
            Some(title) if self.workspace.active_key() != Some(key) => {
                format!("Query cancelled in {title}")
            }
            _ => "Query cancelled".into(),
        });
    }

    /// Completes `request` for the document and generation that started it.
    /// The captured SQL is evaluated, never the editor's current text. A
    /// request whose document is gone, cancelled or already completed is
    /// refused by the workspace and has no effect at all.
    fn finish(&mut self, request: RunRequest, cx: &mut Cx) {
        let RunRequest {
            token,
            sql,
            explain,
            ..
        } = request;
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
        let Ok(((), resp)) = self
            .workspace
            .end_operation(token, |mut view| view.install_for(result, sql, Some(token)))
        else {
            return;
        };
        let msg = match self.workspace.get(token.document) {
            Some(doc) if self.workspace.active_key() != Some(token.document) => {
                format!("{}: {msg}", doc.title)
            }
            _ => msg,
        };
        let previous = self.workspace.active_key();
        self.apply(previous, resp, cx);
        cx.status(msg);
    }

    fn tick(&mut self, cx: &mut Cx) -> Outcome {
        let mut changed = false;
        if let Some(request) = self.running.as_mut() {
            changed = true;
            request.ticks_left = request.ticks_left.saturating_sub(1);
            if request.ticks_left == 0
                && let Some(request) = self.running.take()
            {
                self.sync_toolbar();
                self.finish(request, cx);
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
                let existing = self
                    .workspace
                    .iter()
                    .find(|(_, doc)| doc.title == name)
                    .map(|(key, _)| key);
                let opened = match existing {
                    Some(key) => {
                        let previous = self.workspace.active_key();
                        let promote = previous == Some(key);
                        self.activate_document(key, cx);
                        if promote {
                            let resp = self.workspace.promote(key);
                            self.apply(previous, resp, cx);
                        }
                        true
                    }
                    None => self.new_tab(
                        name.clone(),
                        &format!("SELECT * FROM {name} LIMIT 100;\n"),
                        self.open_mode(),
                        cx,
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
                self.switch_to(i, cx);
            }
            TabAction::Close(i) => self.close_tab(i, cx),
            TabAction::New => {
                self.opened += 1;
                self.new_tab(format!("Query {}", self.opened), "", OpenKind::Pinned, cx);
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
                    if let Some(key) = self.workspace.active_key() {
                        let _ = self.workspace.update(key, |doc| doc.editor.set_text(""));
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
        let Some(key) = self.workspace.active_key() else {
            return;
        };
        if let Ok(mut view) = self.workspace.view_mut(key) {
            view.render_editor(r, buf, ctx);
        }
    }

    fn render_results(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let Some(key) = self.workspace.active_key() else {
            return;
        };
        let running = self.is_running(key);
        if let Ok(mut view) = self.workspace.view_mut(key) {
            view.render_results(r, buf, ctx, running);
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
        let tabs = junie_tui::ui::text::plural(self.workspace.len(), "tab", "tabs");
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
        let Some(key) = self.workspace.active_key() else {
            return Outcome::Ignored;
        };
        let (editor, results) = (editor_id(key), results_id(key));
        if !ev.is(Phase::Move) {
            let routed = if ev.wheel().is_some() {
                self.workspace
                    .view_mut(key)
                    .ok()
                    .and_then(|mut view| view.editor_wheel(ev))
            } else {
                self.workspace
                    .update(key, |doc| doc.editor.route(ev).map(|r| r.outcome()))
                    .ok()
                    .and_then(|(outcome, _)| outcome)
            };
            if let Some(outcome) = routed {
                if click && ev.bar.is_none() {
                    cx.focus.focus(editor);
                }
                return outcome;
            }
        }
        let Ok(mut view) = self.workspace.view_mut(key) else {
            return Outcome::Ignored;
        };
        if let Some(r) = view.results_route(ev) {
            if click && !scrollbar::is_bar_of(ev.id, results) {
                cx.focus.focus(results);
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
            if key.is(KeyCode::Esc) && self.cancel_pending_close(cx) {
                return Outcome::Changed;
            }
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
        let Some(active) = self.workspace.active_key() else {
            return Outcome::Ignored;
        };
        if f == editor_id(active) {
            let edited = self.workspace.update(active, |doc| {
                let (o, a) = doc.editor.on_key(key);
                match a {
                    Some(EditorAction::Changed) => doc.editor.set_diagnostics(Vec::new()),
                    Some(EditorAction::Leave { backward: false }) => return (o, 1),
                    Some(EditorAction::Leave { backward: true }) => return (o, -1),
                    _ => {}
                }
                (o, 0)
            });
            return match edited {
                Ok(((o, leave), _)) => {
                    match leave {
                        1 => cx.focus_next(),
                        -1 => cx.focus_prev(),
                        _ => {}
                    }
                    o
                }
                Err(error) => {
                    cx.status(error.to_string());
                    Outcome::Consumed
                }
            };
        }
        if f == results_id(active) {
            let Ok(mut view) = self.workspace.view_mut(active) else {
                return Outcome::Ignored;
            };
            let (o, actions) = view.results_key(key).into_parts();
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
                } else if let Some(key) = self.workspace.active_key()
                    && cx.focus.is(editor_id(key))
                {
                    self.workspace
                        .update(key, |doc| doc.editor.on_paste(text))
                        .map_or(Outcome::Consumed, |(outcome, _)| outcome)
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
        statuses: Vec<String>,
    }

    impl Rig {
        fn new() -> Self {
            let mut rig = Self {
                page: QueryWorkbenchPage::new(),
                focus: Focus::default(),
                ring: FocusRing::default(),
                hits: HitRegistry::default(),
                statuses: Vec::new(),
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
            Self::collect(&mut self.statuses, cx);
            self.paint();
            out
        }

        fn with_cx(&mut self, f: impl FnOnce(&mut QueryWorkbenchPage, &mut Cx)) {
            let mut cx = Cx::new(&mut self.focus, &self.ring);
            f(&mut self.page, &mut cx);
            Self::collect(&mut self.statuses, cx);
            self.paint();
        }

        fn collect(statuses: &mut Vec<String>, mut cx: Cx) {
            let statuses_iter = cx.take().into_iter().filter_map(|request| match request {
                junie_tui::shell::Request::Status(text, _) => Some(text),
                _ => None,
            });
            statuses.extend(statuses_iter);
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
            let id = self.page.edit_active(|doc| {
                doc.editor.begin_edit();
                doc.editor.id
            });
            self.focus.focus(id);
            self.send(PageEvent::Paste(text.to_owned()));
        }
    }

    impl QueryWorkbenchPage {
        fn active_key(&self) -> Option<ItemKey> {
            self.workspace.active_key()
        }

        fn doc_index(&self, key: ItemKey) -> Option<usize> {
            self.workspace.index_of(key)
        }

        fn docs(&self) -> Vec<&Doc> {
            self.workspace.iter().map(|(_, doc)| doc).collect()
        }

        fn edit_active<R>(&mut self, f: impl FnOnce(&mut Doc) -> R) -> R {
            let key = self.workspace.active_key().expect("active doc");
            self.workspace.update(key, f).expect("open document").0
        }
    }

    const CUSTOMERS_SQL: &str = "SELECT * FROM customers LIMIT 100;\n";

    fn doc(p: &QueryWorkbenchPage, i: usize) -> &Doc {
        p.docs()[i]
    }

    fn texts(p: &QueryWorkbenchPage) -> Vec<String> {
        p.docs()
            .iter()
            .map(|d| d.editor.text().to_owned())
            .collect()
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
        let key = p.active_key().expect("active key");
        let index = p.doc_index(key).expect("active key names a document");
        assert_eq!(p.tabs.active, index, "strip highlights the active document");
        assert_eq!(p.tabs.items()[index].label, p.docs()[index].title);
        let d = p.docs()[index];
        assert_eq!(d.editor.id, editor_id(key));
        assert_eq!(d.results.id, results_id(key));
        assert!(
            rig.hits.area_of(d.editor.id).is_some(),
            "editor is hit-registered"
        );
        for other in p.docs().into_iter().filter(|o| o.key != key) {
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
            p.switch_to(0, cx);
        });
        rig.focus.focus(rig.page.tabs.id);
        rig.key(KeyCode::Down, KeyModifiers::NONE);
        assert!(rig.page.tabs.menu_open(), "overflow menu is open");
        rig.key(KeyCode::Enter, KeyModifiers::NONE);
        let chosen = rig.page.tabs.active;
        assert!(chosen > 1, "a previously hidden tab was chosen");
        assert_eq!(active(&rig.page).title, rig.page.tabs.items()[chosen].label);
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
        let id = rig.page.edit_active(|doc| {
            doc.editor.begin_edit();
            doc.editor.id
        });
        rig.focus.focus(id);
    }

    fn switch(rig: &mut Rig, index: usize) {
        rig.with_cx(|p, cx| {
            p.switch_to(index, cx);
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
        rig.page.edit_active(|doc| doc.editor.jump_to(10));
        for _ in 0..3 {
            rig.key(KeyCode::Right, KeyModifiers::SHIFT);
        }
        {
            rig.page.edit_active(|doc| {
                doc.editor.scroll.v.scroll_to(5);
                doc.editor.hscroll = 7;
            });
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
        let keys: Vec<ItemKey> = rig.page.docs().iter().map(|d| d.key).collect();
        assert_eq!(keys.len(), 3);
        rig.click_tab(1);
        let b_key = keys[1];
        enter_edit(&mut rig);
        rig.send(PageEvent::Paste("-- b\n".into()));
        let b = editor_state(active(&rig.page));
        let b_results = active(&rig.page).results.cursor_row;

        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(0), cx));
        assert_eq!(rig.page.active_key(), Some(b_key));
        assert_eq!(editor_state(active(&rig.page)), b);
        assert_eq!(active(&rig.page).results.cursor_row, b_results);
        assert_agree(&mut rig);

        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(1), cx));
        assert_eq!(rig.page.active_key(), Some(b_key));
        assert_eq!(editor_state(active(&rig.page)), b);
        assert_agree(&mut rig);

        rig.focus.focus(active(&rig.page).editor.id);
        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(0), cx));
        assert!(rig.page.docs().is_empty());
        assert_eq!(rig.page.active_key(), None);
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
            assert!(!p.switch_to(0, cx));
        });

        rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
        let fresh = rig.page.active_key().expect("new doc is active");
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
            let key = p.active_key().expect("active key");
            assert!(!p.activate_document(key, cx), "equal activation is a no-op");
            assert!(!p.switch_to(0, cx));
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
        let a_key = rig.page.active_key().expect("key");
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
        let b_key = rig.page.active_key().expect("key");
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
            rig.page.docs()[0].editor.text(),
            a_state.text,
            "paste never reaches A"
        );

        // B holds a selection. Drag and Release carrying A's editor id, at A's
        // old geometry, must not reach A or B. Safety comes from the
        // key-scoped editor ids: no hit region or focus role names A now.
        rig.page.edit_active(|doc| doc.editor.jump_to(5));
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
        assert_eq!(rig.page.active_key(), Some(b_key));
        assert_eq!(
            editor_state(active(&rig.page)),
            b_before,
            "B text and selection"
        );
        assert_eq!(rig.page.docs()[0].editor.text(), a_state.text, "A text");
        assert_eq!(
            rig.page.docs()[0].editor.field.buffer.selected_text(),
            None,
            "A has no selection"
        );
        assert_eq!(rig.focus.current(), focus_before, "focus unchanged");

        rig.with_cx(|p, cx| {
            p.tree_action(TreeAction::Activate(vec![0, 0, 0, 1]), cx);
        });
        rig.with_cx(|p, cx| {
            p.switch_to(0, cx);
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
        rig.page.edit_active(|doc| doc.editor.jump_to(10));
        for _ in 0..3 {
            rig.key(KeyCode::Right, KeyModifiers::SHIFT);
        }
        {
            rig.page.edit_active(|doc| {
                doc.editor.scroll.v.scroll_to(5);
                doc.editor.hscroll = 7;
            });
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
        assert_eq!(a_back.selection, a.selection, "A's selection");
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

    const ORDERS_25_SQL: &str = "SELECT * FROM orders LIMIT 25;\n";
    const EARLIER_NOTE: &str = "Result for earlier query text";

    fn customers_rows() -> Outcome2 {
        Outcome2::Rows {
            table: "customers".into(),
            count: 40,
            ms: 15,
        }
    }

    fn orders_rows() -> Outcome2 {
        Outcome2::Rows {
            table: "orders".into(),
            count: 25,
            ms: 15,
        }
    }

    /// Documents A (orders), B (customers) and C (orders LIMIT 25), A active.
    fn three_docs() -> Rig {
        let mut rig = Rig::new();
        rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
        rig.page
            .edit_active(|doc| doc.editor.set_text(ORDERS_25_SQL));
        switch(&mut rig, 0);
        rig
    }

    fn tick_to_completion(rig: &mut Rig) {
        for _ in 0..RUN_TICKS {
            rig.send(PageEvent::Tick);
        }
    }

    fn screen_text(buf: &Buffer) -> String {
        buf.content()
            .chunks(usize::from(buf.area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn close(rig: &mut Rig, index: usize) {
        rig.with_cx(|p, cx| p.tab_action(TabAction::Close(index), cx));
    }

    fn toolbar_idle(rig: &Rig) -> bool {
        let items = rig.page.toolbar.items();
        !items[0].disabled && !items[1].disabled && items[2].disabled
    }

    #[test]
    fn issue53_close_earlier_keeps_run_on_b() {
        let mut rig = three_docs();
        let c_before = doc(&rig.page, 2).result.clone();
        switch(&mut rig, 1);
        let b_key = active(&rig.page).key;
        rig.run();
        assert!(rig.page.running.is_some());
        close(&mut rig, 0);
        assert_eq!(rig.page.doc_index(b_key), Some(0));
        tick_to_completion(&mut rig);

        assert_eq!(
            doc(&rig.page, 0).result,
            customers_rows(),
            "B gets B's result"
        );
        assert_eq!(doc(&rig.page, 1).result, c_before, "C is untouched");
        assert!(rig.page.running.is_none());
        assert!(rig.page.tabs.items().iter().all(|tab| !tab.busy));
        assert!(toolbar_idle(&rig));

        let mut rig = Rig::new();
        switch(&mut rig, 1);
        rig.run();
        close(&mut rig, 0);
        tick_to_completion(&mut rig);
        assert_eq!(rig.page.docs().len(), 1);
        assert_eq!(
            doc(&rig.page, 0).result,
            customers_rows(),
            "the result is not dropped when the saved index vanished"
        );
    }

    #[test]
    fn issue53_query_text_is_captured_at_submission() {
        let mut rig = three_docs();
        switch(&mut rig, 1);
        let b = active(&rig.page);
        let b_key = b.key;
        rig.page
            .edit_active(|doc| doc.editor.set_text(ORDERS_25_SQL));
        rig.run();
        let edited = "SELECT * FROM customers LIMIT 100;\n-- later edit\n";
        rig.page.edit_active(|doc| doc.editor.set_text(edited));
        tick_to_completion(&mut rig);

        let b = rig.page.docs()[rig.page.doc_index(b_key).expect("B")];
        assert_eq!(b.result, orders_rows(), "executed the captured query");
        assert_eq!(b.editor.text(), edited, "the late edit is intact");
        assert!(screen_text(&rig.paint()).contains(EARLIER_NOTE));

        let mut rig = three_docs();
        switch(&mut rig, 1);
        let failing = "SELECT * FROM nope LIMIT 1;\n";
        rig.page.edit_active(|doc| doc.editor.set_text(failing));
        rig.run();
        rig.page
            .edit_active(|doc| doc.editor.set_text(&format!("-- moved\n{failing}")));
        tick_to_completion(&mut rig);
        assert!(matches!(active(&rig.page).result, Outcome2::Error { .. }));
        assert!(
            active(&rig.page).editor.diagnostics().is_empty(),
            "stale byte offsets are not attached to new text"
        );

        let mut rig = three_docs();
        switch(&mut rig, 1);
        rig.page.edit_active(|doc| doc.editor.set_text(failing));
        rig.run();
        tick_to_completion(&mut rig);
        assert_eq!(
            active(&rig.page).editor.diagnostics().len(),
            1,
            "unchanged text keeps its diagnostic"
        );
        assert!(!screen_text(&rig.paint()).contains(EARLIER_NOTE));
    }

    #[test]
    fn issue53_target_close_cancels_only_matching_run() {
        let mut rig = three_docs();
        let old_keys: Vec<ItemKey> = rig.page.docs().iter().map(|d| d.key).collect();
        switch(&mut rig, 1);
        rig.run();
        close(&mut rig, 0);
        assert!(rig.page.running.is_some(), "closing A leaves B's run");
        close(&mut rig, 1);
        assert!(rig.page.running.is_some(), "closing C leaves B's run");
        assert!(!toolbar_idle(&rig));

        close(&mut rig, 0);
        assert!(rig.page.running.is_none(), "closing B cancels its run");
        assert!(toolbar_idle(&rig));
        assert!(rig.page.tabs.items().is_empty());

        rig.with_cx(|p, cx| p.tab_action(TabAction::New, cx));
        let fresh = active(&rig.page).key;
        assert!(
            !old_keys.contains(&fresh),
            "a reopened tab is a new identity"
        );
        rig.statuses.clear();
        tick_to_completion(&mut rig);
        assert_eq!(active(&rig.page).result, Outcome2::None);
        assert!(
            rig.statuses.iter().all(|s| !s.starts_with('✓')),
            "an old run reports nothing: {:?}",
            rig.statuses
        );
    }

    #[test]
    fn issue53_inactive_completion_does_not_steal_focus() {
        let mut rig = three_docs();
        switch(&mut rig, 1);
        let b_key = active(&rig.page).key;
        rig.run();
        switch(&mut rig, 2);
        let c_key = active(&rig.page).key;
        enter_edit(&mut rig);
        rig.statuses.clear();
        tick_to_completion(&mut rig);

        assert_eq!(rig.page.active_key(), Some(c_key));
        assert_eq!(rig.focus.current(), Some(editor_id(c_key)));
        let b = rig.page.docs()[rig.page.doc_index(b_key).expect("B")];
        assert_eq!(b.result, customers_rows());
        assert!(rig.page.tabs.items().iter().all(|tab| !tab.busy));
        assert_eq!(
            rig.statuses.last().map(String::as_str),
            Some("customers: ✓ 40 rows · 15 ms"),
            "the status names the document the result belongs to"
        );

        switch(&mut rig, 1);
        assert!(screen_text(&rig.paint()).contains("40 rows"));
    }

    fn deliver(rig: &mut Rig, request: &RunRequest) {
        let request = RunRequest {
            ticks_left: 0,
            ..request.clone()
        };
        rig.with_cx(|p, cx| p.finish(request, cx));
    }

    #[test]
    fn issue53_stale_and_duplicate_generation_ignored() {
        let mut rig = three_docs();
        switch(&mut rig, 1);
        rig.run();
        let old = rig.page.running.clone().expect("running");
        rig.with_cx(|p, cx| p.stop(cx));
        assert!(rig.page.running.is_none());
        assert!(toolbar_idle(&rig));
        assert_eq!(
            rig.page.workspace.running(old.token.document),
            None,
            "the cancelled generation is retired"
        );

        rig.page
            .edit_active(|doc| doc.editor.set_text(ORDERS_25_SQL));
        rig.run();
        let newest = rig.page.running.clone().expect("running again");
        assert!(newest.token.generation > old.token.generation);
        assert_eq!(newest.token.document, old.token.document);

        let statuses = rig.statuses.len();
        deliver(&mut rig, &old);
        assert_eq!(active(&rig.page).result, Outcome2::None, "stale result");
        assert_eq!(rig.statuses.len(), statuses, "stale status");
        assert!(rig.page.running.is_some(), "the newer run keeps running");
        assert!(
            !toolbar_idle(&rig),
            "stale delivery does not touch busy state"
        );

        tick_to_completion(&mut rig);
        assert_eq!(active(&rig.page).result, orders_rows());
        assert_eq!(active(&rig.page).result_token, Some(newest.token));
        assert_eq!(active(&rig.page).result_sql.as_deref(), Some(ORDERS_25_SQL));
        let statuses = rig.statuses.len();

        deliver(&mut rig, &newest);
        assert_eq!(rig.statuses.len(), statuses, "duplicate status");
        assert_eq!(active(&rig.page).result_token, Some(newest.token));
    }

    #[test]
    fn issue53_cancel_after_ready_before_install_rejects_result() {
        let mut rig = three_docs();
        switch(&mut rig, 1);
        rig.run();
        let ready = rig.page.running.clone().expect("running");
        rig.with_cx(|p, cx| p.stop(cx));
        let statuses = rig.statuses.len();
        deliver(&mut rig, &ready);
        assert_eq!(active(&rig.page).result, Outcome2::None);
        assert_eq!(active(&rig.page).result_token, None);
        assert_eq!(rig.statuses.len(), statuses);

        rig.run();
        let ready = rig.page.running.clone().expect("running");
        close(&mut rig, 1);
        let before: Vec<Outcome2> = rig.page.docs().iter().map(|d| d.result.clone()).collect();
        deliver(&mut rig, &ready);
        let after: Vec<Outcome2> = rig.page.docs().iter().map(|d| d.result.clone()).collect();
        assert_eq!(before, after, "no other document takes the result");
        assert_eq!(rig.statuses.len(), statuses + 2, "only run and close speak");
    }

    #[test]
    fn issue53_second_run_is_refused_naming_pending_document() {
        let mut rig = three_docs();
        switch(&mut rig, 1);
        rig.run();
        let pending = rig.page.running.clone().expect("running");
        switch(&mut rig, 2);
        rig.statuses.clear();
        rig.run();
        assert_eq!(rig.page.running.as_ref(), Some(&pending));
        assert_eq!(rig.statuses, ["A query is already running in customers"]);
        assert!(rig.page.toolbar.items()[0].disabled && rig.page.toolbar.items()[1].disabled);
        assert!(
            !rig.page.toolbar.items()[2].disabled,
            "Stop stays available"
        );

        rig.statuses.clear();
        rig.with_cx(|p, cx| p.stop(cx));
        assert_eq!(rig.statuses, ["Query cancelled in customers"]);
        assert!(toolbar_idle(&rig));
    }

    fn titles(p: &QueryWorkbenchPage) -> Vec<String> {
        p.docs().iter().map(|d| d.title.clone()).collect()
    }

    fn open_table(rig: &mut Rig, row: usize) {
        rig.with_cx(|p, cx| p.tree_action(TreeAction::Activate(vec![0, 0, 0, row]), cx));
    }

    #[test]
    fn query_workbench_preview_open_replaces_a_clean_preview_and_pins_on_edit() {
        let mut rig = Rig::new();
        rig.page.set_option("queryworkbench.explorer_open", 1);
        open_table(&mut rig, 2);
        let products = rig.page.active_key().expect("products opened");
        assert_eq!(rig.page.workspace.preview_key(), Some(products));

        open_table(&mut rig, 3);
        assert_eq!(
            titles(&rig.page),
            ["Query 1", "customers", "orders"],
            "the clean preview was replaced, not kept"
        );
        assert!(rig.page.doc_index(products).is_none());
        let orders = rig.page.active_key().expect("orders opened");
        assert_ne!(orders, products, "a replacement is a new document");

        rig.type_into_editor("-- keep this\n");
        assert_eq!(
            rig.page.workspace.preview_key(),
            None,
            "the first edit pinned the preview"
        );
        open_table(&mut rig, 4);
        assert_eq!(
            titles(&rig.page),
            ["Query 1", "customers", "orders", "order_items"],
            "the edited document survived the next preview"
        );

        open_table(&mut rig, 4);
        assert_eq!(
            rig.page.workspace.preview_key(),
            None,
            "opening the active preview again pins it"
        );
    }

    #[test]
    fn query_workbench_close_discards_edits_without_asking_by_default() {
        let mut rig = Rig::new();
        rig.type_into_editor("-- scratch\n");
        close(&mut rig, 0);
        assert_eq!(titles(&rig.page), ["customers"]);
        assert!(rig.page.pending_close.is_none());
    }

    #[test]
    fn query_workbench_ask_mode_needs_a_second_close_and_an_edit_makes_it_stale() {
        let mut rig = Rig::new();
        rig.page.set_option("queryworkbench.dirty_close", 1);
        rig.type_into_editor("-- scratch\n");

        close(&mut rig, 0);
        assert_eq!(rig.page.docs().len(), 2, "the first close only asks");
        let asked = rig.page.pending_close.expect("a close request is waiting");
        assert!(
            rig.statuses
                .last()
                .is_some_and(|s| s.contains("unsaved changes")),
            "{:?}",
            rig.statuses
        );

        rig.type_into_editor("-- more\n");
        close(&mut rig, 0);
        assert_eq!(rig.page.docs().len(), 2, "an edit made the request stale");
        let again = rig.page.pending_close.expect("asked again");
        assert_ne!(again, asked, "the new request has its own token");

        rig.focus.focus(rig.page.tabs.id);
        rig.key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(rig.page.pending_close.is_none(), "Esc keeps the tab");
        assert_eq!(rig.page.docs().len(), 2);

        close(&mut rig, 0);
        close(&mut rig, 0);
        assert_eq!(titles(&rig.page), ["customers"], "a second close discards");
    }

    #[test]
    fn query_workbench_editor_wheel_keeps_a_pending_close_valid() {
        let mut rig = Rig::new();
        rig.page.set_option("queryworkbench.dirty_close", 1);
        let long: String = (0..80).map(|i| format!("-- line {i}\n")).collect();
        rig.type_into_editor(&long);
        close(&mut rig, 0);
        let asked = rig.page.pending_close.expect("a close request is waiting");
        let key = rig.page.active_key().expect("active");
        let revision = rig.page.workspace.revision(key);

        let editor = active(&rig.page).editor.id;
        let area = rig.hits.area_of(editor).expect("editor is hit-registered");
        let before = active(&rig.page).editor.scroll.v.offset;
        let pos = Position::new(area.x + 2, area.y + 1);
        let ev = PointerEvent::wheel_on(
            &rig.hits,
            editor,
            junie_tui::core::hit::Axis::Vertical,
            3,
            pos,
            KeyModifiers::NONE,
        );
        rig.send(PageEvent::Pointer(ev));

        assert!(
            active(&rig.page).editor.scroll.v.offset > before,
            "the wheel scrolled the editor"
        );
        assert_eq!(rig.page.workspace.revision(key), revision);
        assert_eq!(rig.page.pending_close, Some(asked));
        close(&mut rig, 0);
        assert_eq!(
            titles(&rig.page),
            ["customers"],
            "the second close confirms the first request"
        );
    }

    #[test]
    fn query_workbench_ask_mode_confirmation_targets_the_asked_document() {
        let mut rig = Rig::new();
        rig.page.set_option("queryworkbench.dirty_close", 1);
        rig.type_into_editor("-- scratch\n");
        let asked = rig.page.active_key().expect("active");
        close(&mut rig, 0);

        switch(&mut rig, 1);
        close(&mut rig, 1);
        assert_eq!(titles(&rig.page), ["Query 1"], "a clean tab closes at once");
        assert!(rig.page.doc_index(asked).is_some(), "the asked tab waits");
        close(&mut rig, 0);
        assert!(
            rig.page.docs().is_empty(),
            "the second close confirmed the asked document, the only one left"
        );
    }
}
