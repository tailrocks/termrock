//! The Finder page: a demo of how an app composes the shared `SearchDetail`.
//!
//! Ownership. The library owns everything generic: the query text and its
//! edit commands, the request/token lifecycle, keyed selection and cell
//! navigation, the keymap, click and context routing, preview scroll
//! retention, layout, hit regions and both painters (the standard one and the
//! `SearchPaint` hook of the Hosted painter in `finder/hosted.rs`). This page
//! owns only domain and demo wiring: the `ACTIONS` catalog and its `rank`,
//! the `Results` projection built once per `QueryRequested`, the scope and
//! the demo backend (ready, loading, failing), the preview lines, the
//! alternatives menu and why dialog, and the option tables. It holds no
//! selection, query or preview-scroll state of its own.

mod hosted;

use std::collections::VecDeque;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};

use self::hosted::Hosted;
use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::row_menu::{MENU_HINTS, RowMenu};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::edit::TextCmd;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, KeyMap};
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::response::Response;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::theme::{ButtonKind, Tone};
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::layout::Split;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::fuzzy;
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list_detail::{
    Action as PreviewAction, Line, ListDetailSource, ListRowRef, SourceFrame,
};
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::search_detail::{
    MissingSelection, QueryPasteMode, QueryRequest, ResultRowKind, SearchAction, SearchCmd,
    SearchConfig, SearchDetail, SearchFocusMode, SearchPhase, SearchSource,
};
use junie_tui::widgets::table::{Cell, Column, DataTable};

const ID: WidgetId = WidgetId::of("finder");
const QUERY: WidgetId = ID.sub("query");
const RESULTS: WidgetId = ID.sub("results");
const ALTS: WidgetId = ID.sub("alternatives-menu");
const WHY: WidgetId = ID.sub("why-dialog");
const SCOPES: &[&str] = &["Here", "Project", "Everywhere"];

const SCOPE_LINK: WidgetId = ID.sub("scope");

/// The preview sits beside the results from this width up.
const SPLIT_FROM: u16 = 90;

const INSTANCE: ItemKey = ItemKey::text("showcase.finder");
const CATALOG_REVISION: u64 = 1;
const PREVIEW_ALTERNATIVES: &str = "alternatives";
const PREVIEW_WHY: &str = "why";

struct Action {
    name: &'static str,
    group: &'static str,

    scope: usize,
    note: &'static str,
    runs: &'static str,
    risk: &'static str,
    alternatives: &'static [&'static str],

    recency: u32,
}

const ACTIONS: &[Action] = &[
    Action {
        name: "Pull",
        group: "git",
        scope: 0,
        note: "up to date",
        runs: "git pull --ff-only",
        risk: "mutating",
        alternatives: &["Fetch only", "Pull with merge", "Pull with rebase"],
        recency: 9,
    },
    Action {
        name: "Pull with rebase",
        group: "git",
        scope: 0,
        note: "rewrites local commits",
        runs: "git pull --rebase",
        risk: "mutating",
        alternatives: &["Pull", "Fetch only"],
        recency: 3,
    },
    Action {
        name: "Push",
        group: "git",
        scope: 0,
        note: "2 commits ahead",
        runs: "git push",
        risk: "remote",
        alternatives: &["Push with lease", "Push tags"],
        recency: 7,
    },
    Action {
        name: "Stash changes",
        group: "git",
        scope: 0,
        note: "3 files changed",
        runs: "git stash push",
        risk: "mutating",
        alternatives: &["Stash with message", "Stash untracked too"],
        recency: 4,
    },
    Action {
        name: "Build",
        group: "cargo",
        scope: 0,
        note: "debug · 14 crates",
        runs: "cargo build",
        risk: "safe",
        alternatives: &["Build release", "Check only"],
        recency: 8,
    },
    Action {
        name: "Test",
        group: "cargo",
        scope: 0,
        note: "nextest · 431 tests",
        runs: "cargo nextest run",
        risk: "safe",
        alternatives: &["Test this file", "Test with logs"],
        recency: 6,
    },
    Action {
        name: "Update dependencies",
        group: "cargo",
        scope: 1,
        note: "4 outdated",
        runs: "cargo update",
        risk: "mutating",
        alternatives: &["Dry run", "Update one crate"],
        recency: 2,
    },
    Action {
        name: "Compose up",
        group: "docker",
        scope: 1,
        note: "3 services",
        runs: "docker compose up -d",
        risk: "mutating",
        alternatives: &["Compose up (attached)", "Rebuild and up"],
        recency: 5,
    },
    Action {
        name: "Prune images",
        group: "docker",
        scope: 2,
        note: "4.2 GB reclaimable",
        runs: "docker image prune",
        risk: "destructive",
        alternatives: &["Prune all unused", "Show sizes"],
        recency: 1,
    },
    Action {
        name: "Open in IDEA",
        group: "files",
        scope: 1,
        note: "holla",
        runs: "idea .",
        risk: "safe",
        alternatives: &["Open in Zed", "Reveal in Finder"],
        recency: 4,
    },
    Action {
        name: "Clean target",
        group: "files",
        scope: 1,
        note: "9.8 GB",
        runs: "cargo clean",
        risk: "destructive",
        alternatives: &["Clean release only", "Show sizes"],
        recency: 1,
    },
    Action {
        name: "Upgrade Homebrew",
        group: "system",
        scope: 2,
        note: "12 formulae",
        runs: "brew upgrade",
        risk: "mutating",
        alternatives: &["List outdated", "Upgrade one"],
        recency: 2,
    },
];

fn risk_tone(risk: &str) -> Tone {
    match risk {
        "safe" => Tone::Success,
        "remote" => Tone::Info,
        "destructive" => Tone::Danger,
        _ => Tone::Warning,
    }
}

fn rank(query: &str, scope: usize) -> Vec<usize> {
    let mut hits: Vec<(u32, u32, usize)> = ACTIONS
        .iter()
        .enumerate()
        .filter(|(_, a)| a.scope <= scope)
        .filter_map(|(i, a)| fuzzy(a.name, query.trim()).map(|(p, _)| (p, a.recency, i)))
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

    let mut groups: Vec<&str> = Vec::new();
    for (_, _, i) in &hits {
        if !groups.contains(&ACTIONS[*i].group) {
            groups.push(ACTIONS[*i].group);
        }
    }
    groups
        .iter()
        .flat_map(|g| {
            hits.iter()
                .filter(|h| ACTIONS[h.2].group == *g)
                .map(|h| h.2)
        })
        .collect()
}

const GROUPS: &[&str] = &["git", "cargo", "docker", "files", "system"];

fn group_of(action: usize) -> usize {
    GROUPS
        .iter()
        .position(|g| *g == ACTIONS[action].group)
        .expect("every action names a listed group")
}

fn action_key(action: usize) -> ItemKey {
    ItemKey::pair(1, action as u64)
}

fn group_key(group: usize) -> ItemKey {
    ItemKey::pair(2, group as u64)
}

fn spacer_key(group: usize) -> ItemKey {
    ItemKey::pair(3, group as u64)
}

fn action_of(key: ItemKey) -> Option<usize> {
    (0..ACTIONS.len()).find(|&i| action_key(i) == key)
}

/// How the ranked actions become result rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// One selectable item per action.
    Rows,
    /// A heading and a spacer around each group of items.
    Grouped,
    /// One row per group whose actions are separately selectable cells.
    Cells,
}

struct Row {
    kind: ResultRowKind,
    key: ItemKey,
    label: &'static str,
    detail: String,
    meta: &'static str,
    meta_tone: Tone,
    actions: Vec<usize>,
}

impl Row {
    fn heading(group: usize) -> Self {
        Self::bare(ResultRowKind::Heading, group_key(group), GROUPS[group])
    }

    fn spacer(group: usize) -> Self {
        Self::bare(ResultRowKind::Spacer, spacer_key(group), "")
    }

    fn bare(kind: ResultRowKind, key: ItemKey, label: &'static str) -> Self {
        Self {
            kind,
            key,
            label,
            detail: String::new(),
            meta: "",
            meta_tone: Tone::Muted,
            actions: Vec::new(),
        }
    }

    fn item(action: usize, risk: bool) -> Self {
        let a = &ACTIONS[action];
        Self {
            detail: a.note.to_owned(),
            meta: if risk { a.risk } else { "" },
            meta_tone: risk_tone(a.risk),
            actions: vec![action],
            ..Self::bare(ResultRowKind::Item, action_key(action), a.name)
        }
    }

    fn cells(group: usize, actions: Vec<usize>) -> Self {
        Self {
            actions,
            ..Self::bare(ResultRowKind::Cells, group_key(group), GROUPS[group])
        }
    }
}

/// The ranked projection one query request is answered with. The authoritative
/// keys are the whole catalog, so a scope that hides actions changes the rows
/// and not the key universe.
struct Results {
    stamp: SourceStamp,
    rows: Vec<Row>,
    hits: usize,
    total: usize,
}

impl Results {
    fn empty() -> Self {
        Self {
            stamp: SourceStamp {
                instance: INSTANCE,
                data: CATALOG_REVISION,
                view: 0,
            },
            rows: Vec::new(),
            hits: 0,
            total: 0,
        }
    }

    fn build(request: &QueryRequest, scope: usize, layout: Layout, risk: bool) -> Self {
        let ranked = rank(&request.text, scope);
        let mut rows = Vec::new();
        let mut at = 0;
        while at < ranked.len() {
            let group = group_of(ranked[at]);
            let end = ranked[at..]
                .iter()
                .position(|&i| group_of(i) != group)
                .map_or(ranked.len(), |n| at + n);
            let members = &ranked[at..end];
            match layout {
                Layout::Rows => {
                    rows.extend(members.iter().map(|&i| Row::item(i, risk)));
                }
                Layout::Grouped => {
                    if !rows.is_empty() {
                        rows.push(Row::spacer(group));
                    }
                    rows.push(Row::heading(group));
                    rows.extend(members.iter().map(|&i| Row::item(i, risk)));
                }
                Layout::Cells => rows.push(Row::cells(group, members.to_vec())),
            }
            at = end;
        }
        Self {
            stamp: request.token.result_stamp(),
            rows,
            hits: ranked.len(),
            total: ACTIONS.iter().filter(|a| a.scope <= scope).count(),
        }
    }
}

impl RowSource for Results {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn enabled(&self, i: usize) -> bool {
        matches!(
            self.rows[i].kind,
            ResultRowKind::Item | ResultRowKind::Cells
        )
    }

    fn label(&self, i: usize) -> &str {
        self.rows[i].label
    }

    fn key(&self, i: usize) -> ItemKey {
        self.rows[i].key
    }
}

impl KeyedRowSource for Results {
    fn stamp(&self) -> SourceStamp {
        self.stamp
    }

    fn source_len(&self) -> usize {
        ACTIONS.len() + 2 * GROUPS.len()
    }

    fn source_key(&self, i: usize) -> ItemKey {
        if i < ACTIONS.len() {
            action_key(i)
        } else if i < ACTIONS.len() + GROUPS.len() {
            group_key(i - ACTIONS.len())
        } else {
            spacer_key(i - ACTIONS.len() - GROUPS.len())
        }
    }
}

impl SearchSource for Results {
    fn row_kind(&self, row: usize) -> ResultRowKind {
        self.rows[row].kind
    }

    fn cell_count(&self, row: usize) -> usize {
        match self.rows[row].kind {
            ResultRowKind::Item => 1,
            ResultRowKind::Cells => self.rows[row].actions.len(),
            _ => 0,
        }
    }

    fn cell_key(&self, row: usize, cell: usize) -> ItemKey {
        match self.rows[row].kind {
            ResultRowKind::Cells => action_key(self.rows[row].actions[cell]),
            _ => self.rows[row].key,
        }
    }

    fn cell_label(&self, row: usize, cell: usize) -> &str {
        match self.rows[row].kind {
            ResultRowKind::Cells => ACTIONS[self.rows[row].actions[cell]].name,
            _ => self.rows[row].label,
        }
    }

    fn cell_enabled(&self, _row: usize, _cell: usize) -> bool {
        true
    }

    /// A chip is a navigation target: one click runs it, as in Holla's Explore
    /// tiles. Items follow the page's click policy.
    fn cell_click_policy(&self, row: usize, _cell: usize) -> Option<ClickPolicy> {
        (self.rows[row].kind == ResultRowKind::Cells).then_some(ClickPolicy::NAVIGATION)
    }
}

impl ListDetailSource for Results {
    fn row(&self, visible: usize) -> ListRowRef<'_> {
        let row = &self.rows[visible];
        ListRowRef {
            mark: None,
            label: row.label,
            detail: &row.detail,
            meta: row.meta,
            meta_tone: row.meta_tone,
            faint: false,
        }
    }
}

/// How the demo ranking backend answers, so each request state is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Ready,
    /// The first query is answered; later ones stay pending.
    Loading,
    /// The first query is answered; the next one fails, and a retry succeeds.
    Failing,
}

pub fn page() -> Box<dyn Page> {
    Box::new(FinderPage::new())
}

pub struct FinderPage {
    template: Template,
    opts: PageValues,
    search: SearchDetail,
    results: Results,
    scope: usize,
    layout: Layout,
    backend: Backend,
    hosted: bool,
    answered: bool,
    failed: bool,
    menu: RowMenu<ItemKey>,
}

impl FinderPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::Finder));
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            search: new_search(&opts),
            opts,
            results: Results::empty(),
            scope: 0,
            layout: Layout::Rows,
            backend: Backend::Ready,
            hosted: false,
            answered: false,
            failed: false,
            menu: RowMenu::default(),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let q = match self.opts.value("finder.query") {
            "empty" => "",
            "nomatch" => "zzq",
            _ => "pu",
        };
        self.layout = match self.opts.value("finder.rows") {
            "grouped" => Layout::Grouped,
            "cells" => Layout::Cells,
            _ => Layout::Rows,
        };
        self.backend = match self.opts.value("finder.results") {
            "loading" => Backend::Loading,
            "failing" => Backend::Failing,
            _ => Backend::Ready,
        };
        self.hosted = self.opts.value("finder.painter") == "hosted";
        self.search = new_search(&self.opts);
        self.results = Results::empty();
        self.answered = false;
        self.failed = false;
        self.menu = RowMenu::default();
        let scope = match self.opts.value("finder.scope") {
            "project" => 1,
            "everywhere" => 2,
            _ => 0,
        };
        let reply = self.search.set_query(q);
        self.settle(reply);
        self.set_scope(scope);
    }

    pub fn set_scope(&mut self, scope: usize) {
        self.scope = scope.min(SCOPES.len() - 1);
        self.search
            .set_query_readout(&format!("scope ‹ {} ›", SCOPES[self.scope].to_lowercase()));
        let reply = self.search.refresh_source(SourceStamp {
            instance: INSTANCE,
            data: CATALOG_REVISION,
            view: self.scope as u64,
        });
        self.settle(reply);
    }

    /// Answers every query request in `reply` and returns whether there was
    /// one, then the other actions in order. Ranking runs only here, once per
    /// request.
    fn settle(&mut self, reply: Response<SearchAction>) -> (bool, Vec<SearchAction>) {
        let mut queue: VecDeque<SearchAction> = reply.into_actions().into_iter().collect();
        let mut rest = Vec::new();
        let mut answered = false;
        while let Some(action) = queue.pop_front() {
            match action {
                SearchAction::QueryRequested(request) => {
                    answered = true;
                    queue.extend(self.answer(request).into_actions());
                }
                other => rest.push(other),
            }
        }
        (answered, rest)
    }

    fn answer(&mut self, request: QueryRequest) -> Response<SearchAction> {
        let first = !std::mem::replace(&mut self.answered, true);
        match (self.backend, first) {
            (Backend::Loading, false) => Response::changed(),
            (Backend::Failing, false) if !std::mem::replace(&mut self.failed, true) => self
                .search
                .fail(
                    request.token,
                    "the ranking service did not answer".to_owned(),
                )
                .unwrap_or_default(),
            _ => {
                let risk = self.opts.on("finder.risk_column");
                let results = Results::build(&request, self.scope, self.layout, risk);
                match self.search.submit(request.token, &results) {
                    Ok(reply) => {
                        self.results = results;
                        reply
                    }
                    Err(_) => Response::ignored(),
                }
            }
        }
    }

    fn serve(&mut self, reply: Response<SearchAction>, cx: &mut Cx) -> Outcome {
        let outcome = reply.outcome();
        let (answered, rest) = self.settle(reply);
        for action in rest {
            match action {
                SearchAction::Activate(key) => {
                    if let Some(i) = action_of(key) {
                        cx.status(format!("✓ Ran {}", ACTIONS[i].name));
                    }
                }
                SearchAction::ContextRequested(request) => {
                    self.open_alternatives(request.target, request.anchor);
                }
                SearchAction::DetailAction {
                    key,
                    name: PREVIEW_ALTERNATIVES,
                } => self.open_alternatives(key, self.button_anchor()),
                SearchAction::DetailAction {
                    key,
                    name: PREVIEW_WHY,
                } => self.open_why(key, cx),
                // Undo and Redo with an empty history say so; Select all on an
                // empty query stays quiet, as the query is already all there is.
                SearchAction::QueryCommand {
                    cmd: cmd @ (TextCmd::Undo | TextCmd::Redo),
                    effective: false,
                } => cx.status(format!("Nothing to {}", cmd.label().to_lowercase())),
                _ => {}
            }
        }
        if answered { Outcome::Changed } else { outcome }
    }

    fn open_alternatives(&mut self, key: ItemKey, anchor: ContextAnchor) {
        let Some(a) = action_of(key).map(|i| &ACTIONS[i]) else {
            return;
        };
        let items = a.alternatives.iter().map(|s| MenuItem::new(*s)).collect();
        let menu = anchor
            .apply(ContextMenu::new(ALTS, items))
            .title(format!("{} · alternatives", a.name));
        self.menu.open(menu, key);
    }

    fn button_anchor(&self) -> ContextAnchor {
        ContextAnchor::Control {
            id: self.search.detail_id().sub(PREVIEW_ALTERNATIVES),
            fallback: Rect::ZERO,
        }
    }

    fn open_why(&self, key: ItemKey, cx: &mut Cx) {
        let Some(a) = action_of(key).map(|i| &ACTIONS[i]) else {
            return;
        };
        let q = self.search.query().trim().to_owned();
        let how = match fuzzy(a.name, &q) {
            _ if q.is_empty() => "no query: recent first".to_owned(),
            Some((0, _)) => format!("\"{q}\" starts the name"),
            Some((p, _)) if p < 100 => format!("\"{q}\" is in the name"),
            Some(_) => format!("the letters of \"{q}\" appear in order"),
            None => "no match".to_owned(),
        };
        let d = InfoDialog::new(
            WHY,
            &format!("Why {}?", a.name),
            vec![
                Prop::new("Match", how),
                Prop::new("Recency", format!("used {} times this week", a.recency)),
                Prop::new("Scope", SCOPES[a.scope]),
                Prop::new("Risk", a.risk).tone(risk_tone(a.risk)),
            ],
        )
        .intro(vec![(
            "Ranked by match, then by how recently you ran it.".into(),
            Tone::Muted,
        )])
        .width(56);
        cx.open(d, LayerTag::new("why"));
    }

    fn status_line(&self) -> Option<String> {
        match self.search.phase() {
            SearchPhase::Failed => Some(format!(
                "Search failed: {}. Ctrl+R retries.",
                self.search.session().error().unwrap_or("unknown error")
            )),
            SearchPhase::Loading => Some("Searching… showing the previous results".to_owned()),
            SearchPhase::Ready if self.results.hits == 0 => Some(format!(
                "No matches. Nothing among {} actions here matches; try Everywhere",
                self.results.total
            )),
            _ => None,
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let selected = self.search.selected_key().and_then(action_of);
        let (title, meta, lines, actions) = match selected.map(|i| &ACTIONS[i]) {
            Some(a) => (
                a.name.to_owned(),
                format!("{} · {}", a.group, SCOPES[a.scope].to_lowercase()),
                vec![
                    Line::Prop("Runs".into(), a.runs.into(), Tone::Normal),
                    Line::Prop("In".into(), "~/work/holla".into(), Tone::Normal),
                    Line::Prop("Host".into(), "mbp · local".into(), Tone::Normal),
                    Line::Prop("State".into(), a.note.into(), Tone::Normal),
                    Line::Prop("Risk".into(), a.risk.into(), risk_tone(a.risk)),
                    Line::Prop(
                        "Gate".into(),
                        if a.risk == "destructive" {
                            "asks first"
                        } else {
                            "runs directly"
                        }
                        .into(),
                        Tone::Normal,
                    ),
                    Line::Prop("Data".into(), "live · 2 s ago".into(), Tone::Muted),
                ],
                vec![
                    PreviewAction::new(
                        PREVIEW_ALTERNATIVES,
                        "Alternatives…",
                        ButtonKind::Secondary,
                    ),
                    PreviewAction::new(PREVIEW_WHY, "Why?", ButtonKind::Subtle),
                ],
            ),
            None => (
                "Preview".to_owned(),
                String::new(),
                vec![Line::Text("Nothing selected".into(), Tone::Muted)],
                Vec::new(),
            ),
        };
        let under = self.status_line();
        if self.hosted {
            let (body, foot) = match &under {
                Some(_) if area.height > 2 => (
                    Rect::new(area.x, area.y, area.width, area.height - 1),
                    Some(Rect::new(area.x, area.bottom() - 1, area.width, 1)),
                ),
                _ => (area, None),
            };
            let mut host = Hosted::new(self.scope, selected);
            let _ = self
                .search
                .render_hosted(body, &self.results, buf, ctx, &mut host);
            if let (Some(foot), Some(line)) = (foot, &under) {
                buf.set_string(foot.x + 2, foot.y, line, ctx.theme.muted());
            }
        } else {
            let count = self.results.hits.to_string();
            let frame = SourceFrame {
                title: "Results",
                meta: &count,
                head: &[],
                under: under.as_deref(),
                detail_title: &title,
                detail_meta: &meta,
                lines: &lines,
                actions: &actions,
            };
            let _ = self
                .search
                .render_source(area, &self.results, &frame, buf, ctx);
        }
        self.menu.render(area, buf, ctx);
        let pending = self.search.emit_pending_context();
        for action in self.settle(pending).1 {
            if let SearchAction::ContextRequested(request) = action {
                self.open_alternatives(request.target, request.anchor);
            }
        }
    }

    fn pointer(&mut self, ev: &junie_tui::core::pointer::PointerEvent, cx: &mut Cx) -> Outcome {
        if ev.id == SCOPE_LINK {
            if ev.is(Phase::Click) {
                self.set_scope((self.scope + 1) % SCOPES.len());
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        match self.search.pointer(ev, &self.results, cx.focus) {
            Ok(reply) => self.serve(reply, cx),
            Err(_) => Outcome::Ignored,
        }
    }

    fn key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        if (cx.focus.is(QUERY) || (self.hosted && cx.focus.is(RESULTS))) && key.alt() {
            match key.code {
                KeyCode::Up => {
                    self.set_scope(self.scope + 1);
                    return Outcome::Changed;
                }
                KeyCode::Down => {
                    self.set_scope(self.scope.saturating_sub(1));
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if key.ctrl_char('r') && self.search.phase() == SearchPhase::Failed {
            let reply = self.search.retry();
            return self.serve(reply, cx);
        }
        match self.search.on_key(key, &self.results, cx.focus) {
            Ok(reply) => self.serve(reply, cx),
            Err(_) => Outcome::Ignored,
        }
    }
}

fn new_search(opts: &PageValues) -> SearchDetail {
    let click = match opts.value("finder.click") {
        "single" => ClickPolicy::NAVIGATION,
        _ => ClickPolicy::DATA,
    };
    let missing = match opts.value("finder.missing") {
        "first" => MissingSelection::FirstEnabled,
        _ => MissingSelection::NearestPosition,
    };
    let paste = match opts.value("finder.paste") {
        "join" => QueryPasteMode::JoinLinesWithSpaceTrim,
        _ => QueryPasteMode::StripLineBreaks,
    };
    let split_from = if opts.value("finder.preview") == "line" {
        u16::MAX
    } else {
        SPLIT_FROM
    };
    let keymap = match opts.value("finder.keys") {
        "emacs" => emacs_keymap(),
        _ => SearchCmd::default_keymap(),
    };
    let (reset_on_query, keep_on_same_key) = match opts.value("finder.preview_scroll") {
        "reset on query" => (true, true),
        "reset on result" => (false, false),
        _ => (false, true),
    };
    let mut config = SearchConfig::default()
        .keymap(keymap)
        .click(click)
        .missing_selection(missing)
        .paste_mode(paste)
        .select_first_on_query(opts.on("finder.select_first"))
        .navigate_while_loading(opts.on("finder.nav_loading"))
        .reset_preview_on_query(reset_on_query)
        .preserve_preview_on_same_key(keep_on_same_key);
    let mut search = SearchDetail::new(
        RESULTS,
        TextInput::new(QUERY, "")
            .placeholder("Search actions, files and tasks")
            .plain_label()
            .readout_link(SCOPE_LINK),
        Split::new(60, 28, 34),
    );
    if opts.value("finder.painter") == "hosted" {
        // One focus owner for the query and the results, as an app that
        // draws its own query line has; typing from the preview returns to it.
        config = config
            .focus_mode(SearchFocusMode::Combined)
            .type_to_query_from_detail(true);
        search = search.detail_share(34, 34, 48).summary_rows(1);
    }
    search.config(config).collapse_below(split_from)
}

/// The arrow keys give way to Ctrl+N/P and Alt+H/L; focus, preview, activation
/// and paging keep the default keys.
fn emacs_keymap() -> KeyMap<SearchCmd> {
    vec![
        (KeyBinding::ctrl('f'), SearchCmd::FocusQuery),
        (KeyBinding::key(KeyCode::Tab), SearchCmd::FocusResults),
        (KeyBinding::ctrl('o'), SearchCmd::OpenDetail),
        (KeyBinding::key(KeyCode::Enter), SearchCmd::Activate),
        (KeyBinding::ctrl('p'), SearchCmd::Up),
        (KeyBinding::ctrl('n'), SearchCmd::Down),
        (KeyBinding::alt('h'), SearchCmd::Left),
        (KeyBinding::alt('l'), SearchCmd::Right),
        (KeyBinding::key(KeyCode::Home), SearchCmd::Home),
        (KeyBinding::key(KeyCode::End), SearchCmd::End),
        (KeyBinding::key(KeyCode::PageUp), SearchCmd::PageUp),
        (KeyBinding::key(KeyCode::PageDown), SearchCmd::PageDown),
    ]
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let rows = ACTIONS[..3]
        .iter()
        .map(|a| vec![Cell::new(a.name), Cell::new(a.group).tone(Tone::Secondary)])
        .collect();
    let table = DataTable::try_new(
        id.child(1),
        vec![
            Column::new("Action", Constraint::Min(12)),
            Column::new("Group", Constraint::Length(6)),
        ],
        rows,
    )
    .expect("rectangular fixture")
    .empty(EmptyState::new("No matches"));
    StateGrid::new(id)
        .rows(&[Rest, Hover, Focused, Selected, Empty, Overflow, Narrow])
        .column_width(30)
        .column(
            "Scope",
            TextInput::new(id.child(0), "")
                .placeholder("Search actions")
                .plain_label()
                .readout("scope ‹ here ›")
                .readout_link(id.child(2)),
        )
        .column("Results", table)
}

impl Page for FinderPage {
    fn title(&self) -> &'static str {
        "Finder"
    }
    fn blurb(&self) -> &'static str {
        "Typing-hot query, scope, ranked results, preview, alternatives and why"
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
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some((out, chosen)) = self.menu.handle(ev) {
            if let Some((key, i)) = chosen
                && let Some(alt) = action_of(key).and_then(|a| ACTIONS[a].alternatives.get(i))
            {
                cx.status(format!("✓ Ran {alt}"));
            }
            return out;
        }
        match ev {
            PageEvent::Blur(change) => self.search.on_blur(*change).outcome(),
            PageEvent::Key(key) => self.key(key, cx),
            PageEvent::Paste(text) => {
                let reply = self.search.on_paste(text, cx.focus);
                self.serve(reply, cx)
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            _ => Outcome::Ignored,
        }
    }

    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus()
    }

    fn editing(&self) -> bool {
        self.search.editing()
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        let retry = self.search.phase() == SearchPhase::Failed;
        let step = if self.opts.value("finder.keys") == "emacs" {
            "Ctrl+N/P"
        } else {
            "↑↓"
        };
        let mut hints = match focus {
            Some(f) if f == self.search.id() && self.hosted => vec![
                ("Type", "Search"),
                (step, "Move"),
                ("Enter", "Run"),
                ("Alt+↑↓", "Scope"),
                ("Ctrl+O", "Preview"),
                crate::pages::row_menu::OPEN_HINT,
            ],
            Some(f) if f == self.search.id() => vec![
                (step, "Move"),
                ("Enter", "Run"),
                ("Ctrl+O", "Preview"),
                ("Ctrl+F", "Search"),
                crate::pages::row_menu::OPEN_HINT,
            ],
            f if self.search.on_detail(f) && self.hosted => {
                vec![("↑↓", "Scroll"), ("Esc", "Back")]
            }
            f if self.search.on_detail(f) => {
                vec![("↑↓", "Scroll"), ("Tab", "Actions"), ("Esc", "Back")]
            }
            _ => vec![("Type", "Search"), ("Alt+↑↓", "Scope"), ("Tab", "Results")],
        };
        if retry {
            hints.push(("Ctrl+R", "Retry"));
        }
        hints
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
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

#[cfg(all(test, feature = "work-metrics"))]
pub(crate) const ACTION_COUNT: usize = ACTIONS.len();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::options::OptSet;
    use junie_tui::core::event::Input;
    use junie_tui::motion::Motion;
    use junie_tui::runtime::Application;
    use junie_tui::theme::{ColorLevel, Theme};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;

    struct Run {
        app: App,
        term: Terminal<TestBackend>,
    }

    impl Run {
        fn new(cols: u16, rows: u16) -> Self {
            let mut app =
                App::with_motion(Theme::for_level(ColorLevel::TrueColor), Motion::Paused, 0);
            app.open_page(PageId::Finder);
            for (key, value) in [("finder.rows", "cells"), ("finder.query", "empty")] {
                let at = page_options(PageId::Finder)
                    .iter()
                    .find(|o| o.key == key)
                    .and_then(|o| o.values.iter().position(|v| *v == value))
                    .expect("a finder option value");
                app.apply_opt(OptSet { key, value: at });
            }
            let mut run = Self {
                app,
                term: Terminal::new(TestBackend::new(cols, rows)).expect("test terminal"),
            };
            for _ in 0..10 {
                run.send(Input::Tick);
            }
            run
        }

        fn send(&mut self, input: Input) {
            self.app.handle(input);
            self.draw();
        }

        fn draw(&mut self) {
            let app = &mut self.app;
            self.term.draw(|frame| app.render(frame)).expect("draws");
        }

        fn press(&mut self, code: KeyCode) {
            self.send(Input::Key(Key {
                code,
                mods: KeyModifiers::NONE,
            }));
        }

        fn resize(&mut self, cols: u16, rows: u16) {
            self.term.backend_mut().resize(cols, rows);
            self.send(Input::Resize(cols, rows));
        }

        fn screen(&self) -> String {
            let buf = self.term.backend().buffer();
            buf.content()
                .chunks(buf.area.width as usize)
                .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        }

        fn selected(&self) -> Option<usize> {
            let page = self.app.pages[PageId::Finder.index()]
                .as_any()
                .and_then(|any| any.downcast_ref::<FinderPage>())
                .expect("the finder page");
            page.search.selected_key().and_then(action_of)
        }
    }

    fn action_named(name: &str) -> usize {
        ACTIONS
            .iter()
            .position(|a| a.name == name)
            .expect("a catalog action")
    }

    /// The Finder in chip rows with the last git chip selected, drawn wide
    /// enough to show it.
    fn with_the_last_chip_selected() -> Run {
        let mut run = Run::new(140, 44);
        for _ in 0..3 {
            run.press(KeyCode::Tab);
        }
        for _ in 0..3 {
            run.press(KeyCode::Right);
        }
        let last = action_named("Pull with rebase");
        assert_eq!(
            run.selected(),
            Some(last),
            "premise: the last chip is selected"
        );
        let screen = run.screen();
        assert!(
            screen.contains("Pull with rebase   │") && screen.contains(ACTIONS[last].runs),
            "premise: the chip is drawn and the preview describes it\n{screen}"
        );
        run
    }

    #[test]
    fn the_preview_follows_a_selection_moved_off_a_chip_that_no_longer_fits() {
        let mut run = with_the_last_chip_selected();
        let last = action_named("Pull with rebase");
        run.resize(130, 44);
        let now = run.selected().expect("the selection moved to a drawn chip");
        let screen = run.screen();
        assert_ne!(
            now, last,
            "premise: narrowing left the last chip out\n{screen}"
        );
        assert!(
            screen.contains(ACTIONS[now].runs) && !screen.contains(ACTIONS[last].runs),
            "the frame that moved the selection shows {}'s preview, not {}'s\n{screen}",
            ACTIONS[now].name,
            ACTIONS[last].name
        );
    }
}
