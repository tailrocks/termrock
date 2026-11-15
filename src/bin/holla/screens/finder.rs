use crate::screens::Holla;
use junie_tui::core::edit::TextCmd;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::keymap::KeyBinding;
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::response::Response;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::shell::Page as ShellPage;
use junie_tui::theme::{CursorStyle, Theme, Tone};
use junie_tui::ui::columns::{self, Col};
use junie_tui::ui::context::{ContextAnchor, ContextSource};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::layout::Split;
use junie_tui::ui::master_detail::SPLIT_MIN;
use junie_tui::ui::text::{fit, truncate, truncate_middle, wider_than, width, wrap};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::list_detail::DetailPane;
use junie_tui::widgets::page_header::Heading;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::search_detail::{
    MissingSelection, QueryPasteMode, QueryRequest, QueryView, ResultRowKind, RowPaint,
    SearchAction, SearchCmd, SearchConfig, SearchDetail, SearchFocusMode, SearchPaint,
    SearchSource,
};
use junie_tui::widgets::statusbar::StatusItem;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::action::{Confirmation, Item, Kind, Launch, Risk};
use crate::domain::context::Scope;
use crate::domain::projection::{CatalogStamp, Deadline, Seen};
use crate::domain::ranking::{Ranked, search};
use crate::screens::{Cx, Go, Page, Screen, StatusBits, risk_tone};
use crate::sim::world::World;
use junie_tui::ui::text::{plural, truncate_sep};

pub const FINDER: WidgetId = WidgetId::of("here.finder");
pub const SCOPE: WidgetId = WidgetId::of("here.scope");
const QUERY: WidgetId = WidgetId::of("here.query");

const PREVIEW_COMMANDS: usize = 6;

/// How many heading and spacer keys the authoritative key universe reserves:
/// every row that is not an item needs a key of its own.
const HEADING_KEYS: usize = 16;
const BLANK_KEYS: usize = 8;
const EXPLORE_ROW: ItemKey = ItemKey::pair(3, 0);

type PreviewLine = (String, String, Tone, bool);

/// What one request is ranked into, before the rows get their keys.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Plan {
    Heading(String),
    Item(usize, Ranked),

    Explore(Vec<usize>),
    Blank,
}

/// One visible result row, keyed for the shared result source.
#[derive(Debug, Clone)]
enum Row {
    Heading {
        key: ItemKey,
        text: String,
    },
    Item {
        key: ItemKey,
        index: usize,
        ranked: Ranked,
    },
    Explore {
        cells: Vec<usize>,
    },
    Blank {
        key: ItemKey,
    },
}

/// The stable identity of everything the finder can show, built once per
/// catalog. An item is its key wherever it shows, as a row or as an Explore
/// chip, so a selection follows it from one to the other.
#[derive(Debug, Default)]
struct Keys {
    items: Vec<ItemKey>,
    labels: Vec<String>,
    universe: Vec<ItemKey>,
    index: HashMap<ItemKey, usize>,
}

impl Keys {
    fn new(items: &[Item]) -> Self {
        let mut keys = Keys::default();
        for (at, it) in items.iter().enumerate() {
            let key = ItemKey::text(&it.id);
            keys.items.push(key);
            keys.labels.push(it.label.clone());
            keys.universe.push(key);
            keys.index.insert(key, at);
        }
        keys.universe
            .extend((0..HEADING_KEYS).map(|n| ItemKey::pair(1, n as u64)));
        keys.universe
            .extend((0..BLANK_KEYS).map(|n| ItemKey::pair(2, n as u64)));
        keys.universe.push(EXPLORE_ROW);
        keys
    }
}

fn selectable(plan: &Plan) -> bool {
    match plan {
        Plan::Item(..) => true,
        Plan::Explore(cells) => !cells.is_empty(),
        Plan::Heading(_) | Plan::Blank => false,
    }
}

/// Gives each planned row its key. An item shows once: a second row for the
/// same item is dropped, because a selectable key is unique in a projection. A
/// projection with nothing to select shows no rows at all, so the empty state
/// replaces the headings.
fn keyed_rows(plan: Vec<Plan>, keys: &Keys) -> Vec<Row> {
    if !plan.iter().any(selectable) {
        return vec![];
    }
    let (mut headings, mut blanks) = (0, 0);
    let mut shown = std::collections::HashSet::new();
    let plan = plan
        .into_iter()
        .filter(|p| !matches!(p, Plan::Item(index, _) if !shown.insert(*index)))
        .collect::<Vec<_>>();
    plan.into_iter()
        .map(|p| match p {
            Plan::Heading(text) => {
                headings += 1;
                Row::Heading {
                    key: ItemKey::pair(1, (headings - 1).min(HEADING_KEYS - 1) as u64),
                    text,
                }
            }
            Plan::Blank => {
                blanks += 1;
                Row::Blank {
                    key: ItemKey::pair(2, (blanks - 1).min(BLANK_KEYS - 1) as u64),
                }
            }
            Plan::Item(index, ranked) => Row::Item {
                key: keys.items[index],
                index,
                ranked,
            },
            Plan::Explore(cells) => Row::Explore { cells },
        })
        .collect()
}

/// The ranked rows one query request is answered with. The authoritative keys
/// are the whole catalog, so a scope or a query that hides items changes the
/// rows and not the key universe.
struct Results {
    stamp: SourceStamp,
    keys: Rc<Keys>,
    rows: Vec<Row>,
}

impl Results {
    fn empty() -> Self {
        Self {
            stamp: SourceStamp {
                instance: ItemKey::num(0),
                data: 0,
                view: 0,
            },
            keys: Rc::new(Keys::default()),
            rows: vec![],
        }
    }

    fn has_items(&self) -> bool {
        self.rows.iter().any(|r| matches!(r, Row::Item { .. }))
    }
}

impl RowSource for Results {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn enabled(&self, i: usize) -> bool {
        matches!(self.rows[i], Row::Item { .. } | Row::Explore { .. })
    }

    fn label(&self, i: usize) -> &str {
        match &self.rows[i] {
            Row::Heading { text, .. } => text,
            Row::Item { index, .. } => &self.keys.labels[*index],
            Row::Explore { .. } | Row::Blank { .. } => "",
        }
    }

    fn key(&self, i: usize) -> ItemKey {
        match &self.rows[i] {
            Row::Heading { key, .. } | Row::Item { key, .. } | Row::Blank { key } => *key,
            Row::Explore { .. } => EXPLORE_ROW,
        }
    }
}

impl KeyedRowSource for Results {
    fn stamp(&self) -> SourceStamp {
        self.stamp
    }

    fn source_len(&self) -> usize {
        self.keys.universe.len()
    }

    fn source_key(&self, i: usize) -> ItemKey {
        self.keys.universe[i]
    }
}

impl SearchSource for Results {
    fn row_kind(&self, row: usize) -> ResultRowKind {
        match self.rows[row] {
            Row::Heading { .. } => ResultRowKind::Heading,
            Row::Item { .. } => ResultRowKind::Item,
            Row::Explore { .. } => ResultRowKind::Cells,
            Row::Blank { .. } => ResultRowKind::Spacer,
        }
    }

    fn cell_count(&self, row: usize) -> usize {
        match &self.rows[row] {
            Row::Item { .. } => 1,
            Row::Explore { cells } => cells.len(),
            Row::Heading { .. } | Row::Blank { .. } => 0,
        }
    }

    fn cell_key(&self, row: usize, cell: usize) -> ItemKey {
        match &self.rows[row] {
            Row::Explore { cells } => self.keys.items[cells[cell]],
            _ => self.key(row),
        }
    }

    fn cell_label(&self, row: usize, cell: usize) -> &str {
        match &self.rows[row] {
            Row::Explore { cells } => &self.keys.labels[cells[cell]],
            _ => self.label(row),
        }
    }

    fn cell_enabled(&self, _row: usize, _cell: usize) -> bool {
        true
    }

    /// An item follows the data click policy, a chip is a navigation target:
    /// one click opens it.
    fn cell_click_policy(&self, row: usize, _cell: usize) -> Option<ClickPolicy> {
        match self.rows[row] {
            Row::Item { .. } => Some(ClickPolicy::DATA),
            Row::Explore { .. } => Some(ClickPolicy::NAVIGATION),
            Row::Heading { .. } | Row::Blank { .. } => None,
        }
    }
}

/// What the ranked rows were built from besides the catalog and memory: the
/// scope and group. They are the view of the input stamp, so a change to
/// either requests new results.
fn view_of(scope: Scope, group: Option<&str>) -> u64 {
    let scope = match scope {
        Scope::Here => 0,
        Scope::Parent => 1,
        Scope::Children => 2,
        Scope::System => 3,
    };
    let group = match group.map(ItemKey::text) {
        Some(ItemKey::Text(hash)) => hash,
        _ => 0,
    };
    group.rotate_left(3) ^ scope
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// The visible text that changes with time but not with ranking: the data
/// freshness of the selected item, activity line counts and durations, and the
/// age of recent rows. A change repaints; it never rebuilds the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayStamp {
    data: Option<String>,
    activities: Vec<(usize, usize, String)>,
    ago: Vec<String>,
}

/// The query line, the keyboard routes and the preview of the Holla Finder.
/// The shared [`SearchDetail`] owns the query text, the selection, the
/// preview scroll and the pointer routes; this page owns the catalog, the
/// ranking and what every pane looks like.
pub struct FinderPage {
    pub group: Option<&'static str>,
    pub scope: Scope,
    source: Vec<Item>,
    source_seen: Seen<CatalogStamp>,
    source_deadline: Deadline,
    items: Vec<Item>,
    keys: Rc<Keys>,
    revision: u64,
    instance: ItemKey,
    memory_seen: Option<u64>,
    decor_deadline: Deadline,
    display_seen: Option<DisplayStamp>,

    pub warnings: Vec<String>,
    search: SearchDetail,
    results: Results,
    pending_query: Option<QueryRequest>,

    pub explore_overflow_marker: bool,

    list_area: Rect,
    preview_area: Rect,

    pub rebuilds: u32,

    last_tick: u64,
}

fn holla_keymap() -> Vec<(KeyBinding, SearchCmd)> {
    vec![
        (KeyBinding::key(KeyCode::Enter), SearchCmd::Activate),
        (KeyBinding::key(KeyCode::Up), SearchCmd::Up),
        (KeyBinding::key(KeyCode::Down), SearchCmd::Down),
        (KeyBinding::key(KeyCode::Left), SearchCmd::Left),
        (KeyBinding::key(KeyCode::Right), SearchCmd::Right),
        (KeyBinding::key(KeyCode::Home), SearchCmd::Home),
        (KeyBinding::key(KeyCode::End), SearchCmd::End),
        (KeyBinding::key(KeyCode::PageUp), SearchCmd::PageUp),
        (KeyBinding::key(KeyCode::PageDown), SearchCmd::PageDown),
    ]
}

fn new_search() -> SearchDetail {
    use junie_tui::core::edit::CMD_TEXT_KEYS;
    let mut search = SearchDetail::new(FINDER, TextInput::new(QUERY, ""), Split::new(60, 28, 34))
        .config(
            SearchConfig::default()
                .keymap(holla_keymap())
                .focus_mode(SearchFocusMode::Combined)
                .missing_selection(MissingSelection::FirstEnabled)
                .paste_mode(QueryPasteMode::JoinLinesWithSpaceTrim)
                .type_to_query_from_detail(true)
                .reset_preview_on_query(true)
                .click(ClickPolicy::DATA),
        )
        .arrange(|md| {
            md.detail_share(34, 34, 48)
                .summary_rows(1)
                .collapse_below(SPLIT_MIN)
        });
    let _ = search.configure_query(|field| {
        let mut keys = CMD_TEXT_KEYS.to_vec();
        keys.extend([
            (KeyBinding::ctrl('a'), TextCmd::SelectAll),
            (KeyBinding::ctrl('z').shift(), TextCmd::Redo),
            (KeyBinding::ctrl('Z'), TextCmd::Redo),
            (KeyBinding::ctrl('y'), TextCmd::Redo),
            (KeyBinding::ctrl('z'), TextCmd::Undo),
        ]);
        field.text_keymap = Some(keys);
        field.undo_runs = false;
    });
    search
}

impl FinderPage {
    pub fn new(group: Option<&'static str>) -> Self {
        Self {
            group,
            scope: Scope::Here,
            source: vec![],
            source_seen: Seen::default(),
            source_deadline: Deadline::default(),
            items: vec![],
            keys: Rc::new(Keys::default()),
            revision: 0,
            instance: ItemKey::num(NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed)),
            memory_seen: None,
            decor_deadline: Deadline::default(),
            display_seen: None,
            warnings: vec![],
            search: new_search(),
            results: Results::empty(),
            pending_query: None,
            explore_overflow_marker: true,
            list_area: Rect::ZERO,
            preview_area: Rect::ZERO,
            rebuilds: 0,
            last_tick: u64::MAX,
        }
    }

    pub fn query(&self) -> &str {
        self.search.query()
    }

    /// Replaces the query text. The request it makes is answered on the next
    /// [`ensure_projection`](Self::ensure_projection), which every input,
    /// tick, enter and paint runs first.
    pub fn set_query(&mut self, text: &str) -> Outcome {
        let reply = self.search.set_query(text);
        let (outcome, _) = self.queue_requests(reply);
        outcome
    }

    fn selected_index(&self) -> Option<usize> {
        let key = self.search.selected_key()?;
        self.keys.index.get(&key).copied()
    }

    pub fn current(&self) -> Option<&Item> {
        self.items.get(self.selected_index()?)
    }

    #[cfg(test)]
    pub(crate) fn search_on_detail(&self, focus: Option<WidgetId>) -> bool {
        self.search.on_detail(focus)
    }

    pub fn current_id(&self) -> Option<String> {
        self.current().map(|i| i.id.clone())
    }

    pub fn set_scope(&mut self, s: Scope) {
        self.scope = s;
    }

    fn input_stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: self.instance,
            data: self.revision,
            view: view_of(self.scope, self.group),
        }
    }

    /// Takes the query requests out of `reply`. A newer request replaces an
    /// older one that was not answered yet.
    fn queue_requests(&mut self, reply: Response<SearchAction>) -> (Outcome, Vec<SearchAction>) {
        let (outcome, actions) = reply.into_parts();
        let mut rest = vec![];
        for action in actions {
            match action {
                SearchAction::QueryRequested(request) => self.pending_query = Some(request),
                other => rest.push(other),
            }
        }
        (outcome, rest)
    }

    /// Ranks and submits the latest pending request. The request is dropped
    /// when the input changed since it was made; a newer one is already
    /// queued then. Returns the selection actions of the accepted results.
    fn execute_pending_query(&mut self, w: &World) -> Response<SearchAction> {
        let Some(request) = self.pending_query.take() else {
            return Response::ignored();
        };
        if request.token.source != self.input_stamp() {
            return Response::ignored();
        }
        self.rebuilds += 1;
        let results = Results {
            stamp: request.token.result_stamp(),
            keys: Rc::clone(&self.keys),
            rows: keyed_rows(self.plan_rows(w, &request.text), &self.keys),
        };
        match self.search.submit(request.token, &results) {
            Ok(reply) => {
                self.results = results;
                reply
            }
            Err(_) => Response::ignored(),
        }
    }

    /// Answers every pending request, then the requests the answers cause.
    /// Returns whether it ranked anything and the actions the answers made.
    fn answer_requests(&mut self, w: &World) -> (bool, Vec<SearchAction>) {
        let (mut ranked, mut rest) = (false, vec![]);
        while self.pending_query.is_some() {
            ranked = true;
            let reply = self.execute_pending_query(w);
            rest.extend(self.queue_requests(reply).1);
        }
        (ranked, rest)
    }

    /// Brings the catalog and the ranked rows up to date with the world and
    /// returns whether it did any work. A source stamp or a source deadline
    /// rebuilds the structural catalog. A source change, a memory epoch or a
    /// memory deadline decorates it again. Any of those, or a changed scope or
    /// group, requests new results, and so does a query edit. Nothing is ranked
    /// without a request, and a stamp is published only after the whole catalog
    /// is installed.
    fn ensure_projection(&mut self, w: &World) -> bool {
        let catalog = self.sync_catalog(w);
        let reply = self.search.refresh_source(self.input_stamp());
        self.queue_requests(reply);
        let (ranked, _) = self.answer_requests(w);
        if !(catalog || ranked) {
            #[cfg(test)]
            self.assert_matches_reference(w);
            return false;
        }
        self.display_seen = Some(self.display_stamp(w));
        true
    }

    fn sync_catalog(&mut self, w: &World) -> bool {
        let now = w.now_secs();
        let stamp = w.catalog_stamp();
        let memory = w.memory_epoch();
        let source_stale = self.source_seen.needs_rebuild(stamp) || self.source_deadline.due(now);
        let decor_stale =
            source_stale || self.memory_seen != Some(memory) || self.decor_deadline.due(now);
        if !decor_stale {
            return false;
        }
        let source = source_stale.then(|| crate::domain::catalog::build_sources_with_warnings(w));
        let decorated = crate::domain::catalog::decorate(
            w,
            source.as_ref().map_or(self.source.as_slice(), |s| &s.0),
        );
        if let Some((items, warnings)) = source {
            self.source = items;
            self.warnings = warnings;
            self.source_deadline =
                Deadline::new(now, crate::domain::catalog::source_deadline_secs(w));
        }
        self.items = decorated;
        self.decor_deadline = Deadline::new(
            now,
            crate::domain::projection::memory_deadline(
                &w.memory.usage,
                &w.location.cwd,
                &w.host.name,
                now,
            ),
        );
        self.keys = Rc::new(Keys::new(&self.items));
        self.revision += 1;
        self.source_seen.published(stamp);
        self.memory_seen = Some(memory);
        true
    }

    fn plan_rows(&self, w: &World, query: &str) -> Vec<Plan> {
        let learned = if crate::domain::ranking::Query::parse(query).is_empty() {
            None
        } else {
            w.memory
                .usage
                .learned_for(query, &w.host.name)
                .map(str::to_owned)
        };
        let ranked = crate::domain::ranking::search_with(
            &self.items,
            query,
            self.scope,
            self.group,
            learned.as_deref(),
        );
        let has_query = !crate::domain::ranking::Query::parse(query).is_empty();
        let mut rows: Vec<Plan> = vec![];
        if let Some(g) = self.group {
            rows.push(Plan::Heading(format!(
                "{g} · {}",
                plural(ranked.len(), "entry", "entries")
            )));
            for r in ranked {
                rows.push(Plan::Item(r.index, r));
            }
        } else if has_query || self.scope != Scope::Here {
            let title = if has_query {
                format!("Results · {}", ranked.len())
            } else {
                format!(
                    "{} · {}",
                    self.scope_title(w),
                    plural(ranked.len(), "entry", "entries")
                )
            };
            rows.push(Plan::Heading(title));
            for r in ranked
                .into_iter()
                .filter(|r| has_query || self.items[r.index].kind != Kind::Explore)
                .take(60)
            {
                rows.push(Plan::Item(r.index, r));
            }
        } else {
            let mut used = std::collections::BTreeSet::new();

            let suggested: Vec<&Ranked> = ranked
                .iter()
                .filter(|r| {
                    let it = &self.items[r.index];
                    it.is_recommended() && !matches!(it.kind, Kind::Explore | Kind::Activity)
                })
                .take(5)
                .collect();
            if !suggested.is_empty() {
                rows.push(Plan::Heading("Suggested here".into()));
                for r in suggested {
                    used.insert(r.index);
                    rows.push(Plan::Item(r.index, r.clone()));
                }
            }

            let projection = w
                .memory
                .usage
                .recent(&w.location.cwd, &w.host.name, w.now_secs());
            let recent: Vec<&Ranked> = projection
                .iter()
                .filter_map(|(id, _)| ranked.iter().find(|r| self.items[r.index].id == *id))
                .filter(|r| !used.contains(&r.index))
                .collect();
            rows.push(Plan::Blank);
            rows.push(Plan::Heading("Recent here".into()));
            if recent.is_empty() {
                rows.push(Plan::Heading(
                    if projection.is_empty() {
                        "nothing used here yet"
                    } else {
                        "everything used here is suggested above"
                    }
                    .into(),
                ));
            }
            for r in recent {
                used.insert(r.index);
                let mut rr = r.clone();
                if let Some(secs) = self.items[r.index].last_used_secs {
                    rr.reason = w.clock.ago(secs);
                    rr.ago_secs = Some(secs);
                }
                rows.push(Plan::Item(r.index, rr));
            }

            if w.location.workspace.is_some() {
                let parent_ranked = search(&self.items, "", Scope::Parent, None);

                let eligible = |r: &&Ranked| {
                    let it = &self.items[r.index];
                    it.scope.word == "parent"
                        && it.kind != Kind::Explore
                        && !used.contains(&r.index)
                };
                let ecosystem_rank = |r: &&Ranked| {
                    let id = self.items[r.index].id.as_str();
                    match id.rsplit(['.', ':']).next().unwrap_or("") {
                        "dev" => 0,
                        "up" => 1,
                        "setup" => 2,
                        _ => 3,
                    }
                };
                let mut tasks: Vec<&Ranked> = parent_ranked
                    .iter()
                    .filter(eligible)
                    .filter(|r| self.items[r.index].kind == Kind::Mise)
                    .collect();
                tasks.sort_by_key(|r| ecosystem_rank(r));
                let mut parent: Vec<&Ranked> = tasks.into_iter().take(2).collect();
                for r in parent_ranked.iter().filter(eligible) {
                    if parent.len() >= 2 {
                        break;
                    }
                    if !parent.iter().any(|p| p.index == r.index) {
                        parent.push(r);
                    }
                }
                if !parent.is_empty() {
                    let name = w
                        .location
                        .workspace
                        .as_ref()
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    rows.push(Plan::Blank);
                    rows.push(Plan::Heading(format!("From {name} · the parent root")));
                    for r in parent {
                        used.insert(r.index);
                        rows.push(Plan::Item(r.index, r.clone()));
                    }
                }
            }

            let running: Vec<&Ranked> = ranked
                .iter()
                .filter(|r| self.items[r.index].kind == Kind::Activity)
                .take(4)
                .collect();
            if !running.is_empty() {
                rows.push(Plan::Blank);
                rows.push(Plan::Heading("Running".into()));
                for r in running {
                    used.insert(r.index);
                    rows.push(Plan::Item(r.index, r.clone()));
                }
            }

            rows.push(Plan::Blank);
            rows.push(Plan::Heading("Explore".into()));
            let cells: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter(|(i, it)| {
                    it.kind == Kind::Explore && it.id.starts_with("explore.") && !used.contains(i)
                })
                .map(|(i, _)| i)
                .collect();
            if !cells.is_empty() {
                rows.push(Plan::Explore(cells));
            }
            for r in ranked
                .iter()
                .filter(|r| self.items[r.index].id.starts_with("scope."))
            {
                rows.push(Plan::Item(r.index, r.clone()));
            }
        }
        rows
    }

    fn live_item<'a>(&self, it: &'a Item, w: &World) -> std::borrow::Cow<'a, Item> {
        live_item(it, w)
    }

    fn display_stamp(&self, w: &World) -> DisplayStamp {
        let mut activities = vec![];
        let mut ago = vec![];
        for r in &self.results.rows {
            let Row::Item { index, ranked, .. } = r else {
                continue;
            };
            if let Some(secs) = ranked.ago_secs {
                ago.push(w.clock.ago(secs));
            }
            let it = &self.items[*index];
            if it.kind == Kind::Activity
                && let Some(a) = it
                    .id
                    .strip_prefix("activity.")
                    .and_then(|id| w.activity(id))
            {
                activities.push((
                    a.output.len(),
                    a.dropped,
                    crate::screens::ticks_label(a.duration_ticks(w.tick)),
                ));
            }
        }
        DisplayStamp {
            data: self
                .current()
                .map(|it| self.live_item(it, w).freshness.label()),
            activities,
            ago,
        }
    }

    #[cfg(test)]
    pub(crate) fn rebuild_signature(&self, w: &World) -> Vec<String> {
        let mut v = vec![];
        for r in &self.results.rows {
            v.push(match r {
                Row::Heading { text, .. } => format!("heading {text}"),
                Row::Blank { .. } => "blank".into(),
                Row::Explore { cells } => format!(
                    "explore {:?}",
                    cells.iter().map(|i| &self.items[*i].id).collect::<Vec<_>>()
                ),
                Row::Item { index, ranked, .. } => {
                    let it = &self.items[*index];
                    format!(
                        "item {} {} {:?} {}",
                        it.id,
                        ranked.score,
                        ranked.matched,
                        row_reason(it, ranked, w)
                    )
                }
            });
        }
        v.push(format!("warnings {:?}", self.warnings));
        for it in &self.items {
            let mut live = self.live_item(it, w).into_owned();
            live.frecency = crate::domain::ranking::frecency_boost(live.frecency) as f64;
            live.freshness = crate::domain::action::Freshness::Unavailable(live.freshness.label());
            v.push(format!("{live:?}"));
        }
        v
    }

    /// The differential check of the whole cache: a settled page must read
    /// exactly like a page built from nothing at this instant.
    #[cfg(test)]
    fn assert_matches_reference(&self, w: &World) {
        let mut reference = FinderPage::new(self.group);
        reference.scope = self.scope;
        reference.set_query(self.query());
        crate::domain::projection::uncounted(|| reference.ensure_projection(w));
        crate::domain::projection::assert_same_signature(
            &self.rebuild_signature(w),
            &reference.rebuild_signature(w),
            "finder",
        );
    }

    #[cfg(test)]
    pub(crate) fn settle_projection_for_test(&mut self, w: &World) {
        self.ensure_projection(w);
        self.ensure_projection(w);
    }

    fn scope_title(&self, w: &World) -> String {
        scope_title(self.scope, w)
    }

    fn primary_hint(&self) -> Hint {
        match self.current() {
            Some(it) => match (&it.launch, &it.confirmation) {
                (_, Confirmation::TwoGate { .. }) => hint("Enter", "Review…"),
                (_, Confirmation::Trust { .. }) => hint("Enter", "Trust…"),
                (Launch::Plan { .. }, _) => hint("Enter", "Review plan"),
                (
                    Launch::Disk { .. }
                    | Launch::Group(_)
                    | Launch::Scope(_)
                    | Launch::Snapshot { .. },
                    _,
                ) => hint("Enter", "Open"),
                (Launch::OpenActivity { .. }, _) => hint("Enter", "Switch"),
                (_, Confirmation::One) => hint("Enter", "Confirm…"),
                _ => hint("Enter", "Run"),
            },
            None => hint("Enter", "Run"),
        }
    }

    fn activate(&mut self, key: ItemKey, cx: &mut Cx) {
        let Some(it) = self.keys.index.get(&key).and_then(|&i| self.items.get(i)) else {
            return;
        };
        match &it.launch {
            Launch::Group(g) => cx.go(Go::Push(Page::Finder { group: Some(g) })),
            Launch::Scope(s) => cx.go(Go::Scope(*s)),
            _ => cx.go(Go::Run {
                item: it.id.clone(),
                args: vec![],
            }),
        }
    }

    fn activate_current(&mut self, cx: &mut Cx) -> Outcome {
        match self.search.selected_key() {
            Some(key) => {
                self.activate(key, cx);
                Outcome::Changed
            }
            None => Outcome::Consumed,
        }
    }

    fn open_alternatives(&mut self, key: ItemKey, anchor: ContextAnchor, cx: &mut Cx) {
        let Some(it) = self.keys.index.get(&key).and_then(|&i| self.items.get(i)) else {
            return;
        };
        let anchor = match anchor {
            ContextAnchor::Pointer(pos) => Rect::new(pos.x, pos.y, 1, 1),
            ContextAnchor::Control { fallback, .. } => {
                Rect::new(self.list_area.x + 6, fallback.y, 1, 1)
            }
        };
        cx.go(Go::Alternatives {
            item: it.id.clone(),
            anchor,
        });
    }

    fn alternatives(&mut self, cx: &mut Cx) -> Outcome {
        let Some(key) = self.search.selected_key() else {
            return Outcome::Consumed;
        };
        let anchor = self
            .search
            .context_cursor()
            .map_or(ContextAnchor::Pointer(self.list_area.as_position()), |r| {
                r.anchor
            });
        self.open_alternatives(key, anchor, cx);
        Outcome::Changed
    }

    /// Answers the requests in `reply`, then runs the other actions in order.
    fn serve(&mut self, reply: Response<SearchAction>, w: &World, cx: &mut Cx) -> Outcome {
        let (mut outcome, mut actions) = self.queue_requests(reply);
        let (ranked, answered) = self.answer_requests(w);
        actions.extend(answered);
        if ranked {
            self.display_seen = Some(self.display_stamp(w));
            outcome = outcome.or(Outcome::Changed);
        }
        for action in actions {
            match action {
                SearchAction::Activate(key) => self.activate(key, cx),
                SearchAction::ContextRequested(request) => {
                    self.open_alternatives(request.target, request.anchor, cx);
                }
                SearchAction::QueryCommand { cmd, effective } => {
                    let note = match (cmd, effective) {
                        (TextCmd::SelectAll, true) => "Query selected · typing replaces it",
                        (TextCmd::SelectAll, false) => "Nothing to select",
                        (TextCmd::Undo, true) => "Undone",
                        (TextCmd::Undo, false) => "Nothing to undo",
                        (TextCmd::Redo, true) => "Redone",
                        (TextCmd::Redo, false) => "Nothing to redo",
                        _ => continue,
                    };
                    cx.status(note);
                }
                SearchAction::QueryRequested(_)
                | SearchAction::SelectionChanged(_)
                | SearchAction::DetailAction { .. }
                | SearchAction::DetailItem { .. } => {}
            }
        }
        outcome
    }

    /// Whether the shared controller should see `key`. The query field would
    /// take Tab and the other keys the page leaves to focus traversal and the
    /// shell.
    fn is_query_or_result_key(&self, key: &Key) -> bool {
        self.search.session().command_for(key).is_some()
            || match key.code {
                KeyCode::Esc | KeyCode::Backspace => true,
                KeyCode::Char(_) => !key.alt(),
                _ => false,
            }
    }

    fn preview_key(&mut self, key: &Key, w: &World, cx: &mut Cx) -> Option<Outcome> {
        match key.code {
            KeyCode::Char('y') if key.plain() => {
                if let Some(it) = self.current() {
                    let cmd = it.commands.join("\n");
                    if cmd.is_empty() {
                        cx.status("Nothing to copy: this row has no command");
                    } else {
                        cx.copy(cmd);
                    }
                }
                Some(Outcome::Changed)
            }
            KeyCode::Enter => Some(self.activate_current(cx)),
            KeyCode::Up | KeyCode::Down | KeyCode::Esc | KeyCode::Char(_) => {
                let reply = match self.search.on_key(key, &self.results, cx.focus) {
                    Ok(reply) => reply,
                    Err(_) => return Some(Outcome::Ignored),
                };
                Some(self.serve(reply, w, cx))
            }
            _ => Some(Outcome::Ignored),
        }
    }
}

fn scope_title(scope: Scope, w: &World) -> String {
    match scope {
        Scope::Here => "Here".into(),
        Scope::Parent => match &w.location.workspace {
            Some(p) => format!("Parent · {}", p.name),
            None => match &w.location.project {
                Some(p) => format!("Parent · {}", p.name),
                None => "Parent".into(),
            },
        },
        Scope::Children => format!(
            "Children · {}",
            plural(w.location.children.len(), "project", "projects")
        ),
        Scope::System => format!("System · {}", w.host.name),
    }
}

fn scope_word(scope: Scope) -> &'static str {
    match scope {
        Scope::Here => "here",
        Scope::Parent => "parent",
        Scope::Children => "children",
        Scope::System => "system",
    }
}

/// The item as it reads at the world's current tick: the activity text and
/// the data freshness are recomputed from the live world, everything else
/// stays as cached.
fn live_item<'a>(it: &'a Item, w: &World) -> std::borrow::Cow<'a, Item> {
    let activity = it
        .id
        .strip_prefix("activity.")
        .filter(|_| it.kind == Kind::Activity)
        .and_then(|id| w.activity(id));
    let observed = it.freshness.observed_tick();
    if activity.is_none() && observed.is_none() {
        return std::borrow::Cow::Borrowed(it);
    }
    let mut live = it.clone();
    if let Some(a) = activity {
        let display = crate::domain::catalog::activity_display(a, w.tick);
        live.summary = display.summary;
        if let Some(r) = live.reasons.first_mut() {
            r.text = display.reason;
        }
    }
    if let Some(t) = observed {
        let elapsed = w.tick.saturating_sub(t).min(i64::MAX as u64) as i64;
        live.freshness = it
            .freshness
            .advanced(elapsed.saturating_mul(crate::sim::world::TICK_MS));
    }
    std::borrow::Cow::Owned(live)
}

fn row_reason(it: &Item, rk: &Ranked, w: &World) -> String {
    let reason = match rk.ago_secs {
        Some(secs) => w.clock.ago(secs),
        None => match live_item(it, w) {
            std::borrow::Cow::Owned(live) if it.kind == Kind::Activity => {
                match (it.reasons.first(), live.reasons.first()) {
                    (Some(old), Some(new)) => rk.reason.replacen(&old.text, &new.text, 1),
                    _ => rk.reason.clone(),
                }
            }
            _ => rk.reason.clone(),
        },
    };
    match &rk.tag {
        Some(tag) if !reason.starts_with(tag.as_str()) => format!("{tag} · {reason}"),
        _ => reason,
    }
}

fn preview_lines(it: &Item, w: &World) -> Vec<PreviewLine> {
    let mut v: Vec<PreviewLine> = vec![];
    v.push(("Does".into(), it.summary.clone(), Tone::Normal, true));
    for (i, c) in it.commands.iter().enumerate().take(PREVIEW_COMMANDS) {
        v.push((
            if i == 0 { "Runs".into() } else { String::new() },
            c.clone(),
            Tone::Secondary,
            false,
        ));
    }
    if it.commands.len() > PREVIEW_COMMANDS {
        v.push((
            String::new(),
            format!("… {} more", it.commands.len() - PREVIEW_COMMANDS),
            Tone::Muted,
            false,
        ));
    }
    if it.commands.is_empty() {
        v.push((
            "Runs".into(),
            "no command · opens inside holla".into(),
            Tone::Muted,
            true,
        ));
    }
    let runs_in = if it.scope.direction == Scope::System {
        format!("{} · host-wide", w.host.name)
    } else {
        w.location.short(&it.scope.runs_in)
    };
    v.push(("In".into(), runs_in, Tone::Normal, false));
    v.push((
        "Host".into(),
        format!(
            "{} · {}{}",
            w.host.name,
            w.host.role.label(),
            if w.host.remote { " · over SSH" } else { "" }
        ),
        Tone::Normal,
        true,
    ));

    let defined = if it.scope.direction == Scope::System || it.scope.defined_at == it.scope.runs_in
    {
        String::new()
    } else {
        format!(" · {}", w.location.short(&it.scope.defined_at))
    };
    v.push((
        "Scope".into(),
        format!("{}{}", it.scope.word, defined),
        Tone::Normal,
        false,
    ));
    let mut reasons: Vec<&str> = it.reasons.iter().map(|r| r.text.as_str()).collect();
    reasons.sort();
    let why = {
        let mut sorted: Vec<&crate::domain::action::Reason> = it.reasons.iter().collect();
        sorted.sort_by_key(|r| std::cmp::Reverse(r.signal));
        sorted
            .iter()
            .map(|r| r.text.as_str())
            .take(3)
            .collect::<Vec<_>>()
            .join(" · ")
    };
    v.push((
        "Why".into(),
        if why.is_empty() {
            "available".into()
        } else {
            why
        },
        Tone::Normal,
        true,
    ));
    let changes = if it.effects.is_empty() {
        match it.risk {
            Risk::ReadOnly => "none · read-only".to_owned(),
            _ => "see the command".to_owned(),
        }
    } else {
        it.effects.join(" · ")
    };
    v.push(("Changes".into(), changes, Tone::Normal, true));
    v.push((
        "Risk".into(),
        it.risk.label().into(),
        risk_tone(it.risk),
        true,
    ));
    v.push((
        "Data".into(),
        it.freshness.label(),
        match it.freshness {
            crate::domain::action::Freshness::Unavailable(_) => Tone::Error,
            crate::domain::action::Freshness::Loading
            | crate::domain::action::Freshness::Partial { .. } => Tone::Warning,
            _ => Tone::Muted,
        },
        true,
    ));
    let gate = match (&it.launch, &it.confirmation) {
        (Launch::Cleanup { .. } | Launch::Disk { .. } | Launch::TopFiles | Launch::Find, _) => {
            "opens a review · the deletion is gated there (typed phrase)".to_owned()
        }
        (_, Confirmation::TwoGate { .. }) => format!(
            "two gates · type {}",
            it.confirmation.phrase().unwrap_or_default()
        ),
        (_, c) => c.label(),
    };
    v.push((
        "Gate".into(),
        gate,
        if matches!(it.confirmation, Confirmation::None) {
            Tone::Muted
        } else {
            Tone::Normal
        },
        true,
    ));
    if !it.args.is_empty() {
        let names: Vec<String> = it
            .args
            .iter()
            .map(|a| {
                if a.default.is_empty() {
                    a.name.clone()
                } else {
                    format!("{} = {}", a.name, a.default)
                }
            })
            .collect();
        v.push(("Args".into(), names.join(" · "), Tone::Normal, true));
    }
    if let Some(tool) = &it.preferred_tool {
        v.push(("Prefers".into(), tool.clone(), Tone::Muted, true));
    }
    let mut alts: Vec<String> = it.alternatives.iter().map(|a| a.label.clone()).collect();
    alts.push("Pin".into());
    alts.push("Alias".into());
    alts.push("Why here?".into());

    let key = if junie_tui::core::keyboard::label_deliverable("Alt+Enter") {
        "Alt+Enter"
    } else {
        "Alternatives…"
    };
    v.push((key.into(), alts.join(" · "), Tone::Muted, true));
    let _ = reasons;
    v
}

/// Paints the Finder's own look into the panes of the shared [`SearchDetail`].
struct Paint<'a> {
    w: &'a World,
    items: &'a [Item],
    results: &'a Results,
    scope: Scope,
    group: Option<&'static str>,
    query: &'a str,
    current: Option<&'a Item>,
    marker: bool,
    list_area: &'a mut Rect,
    preview_area: &'a mut Rect,
    cols: Columns,
}

/// The widths of the list columns, solved once per paint from every item row.
#[derive(Debug, Default, Clone, Copy)]
struct Columns {
    label: u16,
    kind: u16,
    scope: u16,
    reason: u16,
    risk: u16,
}

impl Columns {
    fn solve(results: &Results, items: &[Item], row_w: u16) -> Self {
        let item_rows: Vec<&Item> = results
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Item { index, .. } => Some(&items[*index]),
                _ => None,
            })
            .collect();
        let max_label = item_rows
            .iter()
            .map(|it| width(&it.label))
            .max()
            .unwrap_or(10) as u16;
        let type_w = item_rows
            .iter()
            .map(|it| width(it.kind.label()))
            .max()
            .unwrap_or(4) as u16;
        let scope_w = item_rows
            .iter()
            .map(|it| width(&it.scope.word))
            .max()
            .unwrap_or(4) as u16;
        let content_w = row_w.saturating_sub(4);

        let risk_w = item_rows
            .iter()
            .filter(|it| matches!(it.risk, Risk::Destructive | Risk::Privileged))
            .map(|it| width(it.risk.label()) as u16)
            .max()
            .unwrap_or(0);

        let label_cap = max_label.min((content_w * 36 / 100).max(14));
        let mut cols = vec![
            Col::content(label_cap, label_cap).max(max_label),
            Col::fixed(type_w),
            Col::fixed(scope_w),
            Col::content(12, 12).flex(1, u16::MAX).droppable(1),
        ];
        if risk_w > 0 {
            cols.push(Col::fixed(risk_w));
        }
        let widths = columns::solve(&cols, content_w, 2);
        Self {
            label: widths[0],
            kind: widths[1],
            scope: widths[2],
            reason: widths[3],
            risk: risk_w,
        }
    }
}

impl SearchPaint<Results> for Paint<'_> {
    fn query(&mut self, area: Rect, view: QueryView<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focused = view.focused;
        let fs = t.field_style(junie_tui::ui::ctx::VisualState {
            focused,
            editing: focused,
            ..Default::default()
        });
        fill(buf, area, fs);
        let bg = fs.bg.unwrap_or(t.field);
        buf.set_string(
            area.x,
            area.y,
            t.gutter_symbol(junie_tui::ui::ctx::VisualState {
                focused,
                ..Default::default()
            }),
            t.gutter(
                junie_tui::ui::ctx::VisualState {
                    focused,
                    ..Default::default()
                },
                bg,
                false,
            ),
        );

        let readout = format!("scope ‹ {} ›", scope_word(self.scope));
        let rw = width(&readout) as u16 + 2;
        let rx = area.right().saturating_sub(rw);

        let mut rs = ctx.title_style(SCOPE, bg);
        let lifted = ctx.interaction.hovered(SCOPE) || ctx.interaction.pressed(SCOPE);
        if self.scope == Scope::Here && !lifted {
            rs = rs.fg(t.legible(t.text_muted, rs.bg.unwrap_or(bg)));
        }
        buf.set_string(rx, area.y, format!(" {readout} "), rs);
        ctx.clickable(SCOPE, Rect::new(rx, area.y, rw, 1));
        let text_w = rx.saturating_sub(area.x + 3) as usize;
        let query = view.text;
        if query.is_empty() {
            let placeholder = match self.group {
                Some(g) => format!("Search {}…", g.to_lowercase()),
                None => "Search actions and resources…".into(),
            };
            buf.set_string(
                area.x + 2,
                area.y,
                junie_tui::ui::reveal::cut(
                    Rect::new(area.x + 2, area.y, (text_w) as u16, 1),
                    &placeholder,
                ),
                fs.fg(t.text_muted),
            );
        } else {
            let shown = if width(query) > text_w {
                truncate_middle(query, text_w)
            } else {
                query.to_owned()
            };
            let all_selected = view.selection == Some(0..query.len());
            let st = if all_selected {
                fs.fg(t.canvas).bg(t.accent)
            } else if focused {
                fs.add_modifier(Modifier::UNDERLINED)
                    .underline_color(t.text_secondary)
            } else {
                fs
            };
            buf.set_string(area.x + 2, area.y, &shown, st);
        }
        if focused {
            let cx = area.x + 2 + width(query).min(text_w) as u16;
            ctx.set_cursor(Position::new(cx, area.y));
        }
    }

    fn rows_begin(&mut self, area: Rect, row_w: u16) {
        *self.list_area = area;
        self.cols = Columns::solve(self.results, self.items, row_w);
    }

    fn row(&mut self, mut row: RowPaint<'_, Results>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.canvas;
        let rect = row.rect;
        let y = rect.y;
        let Columns {
            label: label_w,
            kind: type_w,
            scope: scope_w,
            reason: reason_w,
            risk: risk_w,
        } = self.cols;
        match &row.source.rows[row.row] {
            Row::Blank { .. } => {}
            Row::Heading { text, .. } => Heading::new(text).render(
                Rect::new(rect.x + 3, y, rect.width.saturating_sub(3), 1),
                buf,
                t,
                bg,
            ),
            Row::Explore { cells } => {
                let mut s = ctx.state(row.hit);
                s.focused = row.focused && row.selected;

                let cr = t.cursor_row(s, bg, CursorStyle::Fill);
                let st = cr.style;
                fill(buf, rect, st);
                buf.set_string(rect.x, y, cr.gutter, cr.gutter_style);
                let mut x = rect.x + 3;

                let widths: Vec<u16> = cells
                    .iter()
                    .map(|idx| width(&self.items[*idx].label) as u16 + 2)
                    .collect();
                let (shown, more) = junie_tui::widgets::chips::overflow_run(
                    &widths,
                    rect.right().saturating_sub(x),
                    1,
                );
                for (ci, idx) in cells.iter().enumerate().take(shown) {
                    let it = &self.items[*idx];
                    let cid = row.cell_hit(ci);
                    let chosen = row.selected_cell == Some(ci);
                    let cw = widths[ci];

                    let mut cs = ctx.title_style(cid, st.bg.unwrap_or(bg)).fg(t.text_primary);
                    if chosen {
                        cs = cs.add_modifier(Modifier::BOLD);
                        buf.set_string(x, y, "›", st.fg(t.row_ink(&cr, t.text_secondary)));
                    }
                    buf.set_string(x + 1, y, &it.label, cs);
                    row.place_cell(ci, Rect::new(x, y, cw, 1));
                    x += cw + 1;
                }
                if let Some(m) = more.filter(|_| self.marker) {
                    buf.set_string(x, y, &m, st.fg(t.text_muted));
                }
            }
            Row::Item { index, ranked, .. } => {
                let it = &self.items[*index];
                let mut s = ctx.state(row.hit);
                s.focused = row.focused && row.selected;
                s.selected = row.selected;

                let cr = t.cursor_row(s, bg, CursorStyle::Fill);
                let st = cr.style;
                fill(buf, rect, st);
                if s.selected {
                    buf.set_string(rect.x + 1, y, "›", st.fg(t.row_ink(&cr, t.text_secondary)));
                }
                let mut x = rect.x + 3;

                let label_cell = Rect::new(x, y, label_w, 1);
                let mut cut = wider_than(&it.label, label_w as usize);
                let label = truncate(&it.label, label_w as usize);
                junie_tui::ui::reveal::queue_cut(label_cell, &it.label);
                let mut lx = x;
                for (bi, ch) in label.char_indices() {
                    let mut cs = st;
                    if ranked.matched.contains(&bi) {
                        cs = cs.add_modifier(Modifier::BOLD);
                    } else if !s.focused {
                        cs = cs.remove_modifier(Modifier::BOLD);
                    }
                    if it.hidden {
                        cs = cs.fg(t.row_ink(&cr, t.text_faint));
                    }
                    let g = ch.to_string();
                    buf.set_string(lx, y, &g, cs);
                    lx += width(&g) as u16;
                }
                x += label_w + 2;
                let plain = st.remove_modifier(Modifier::BOLD);
                buf.set_string(
                    x,
                    y,
                    fit(it.kind.label(), type_w as usize),
                    plain.fg(t.row_ink(&cr, t.text_muted)),
                );
                x += type_w + 2;
                let scope_tone = if it.scope.direction == Scope::Here {
                    t.text_muted
                } else {
                    t.text_secondary
                };
                buf.set_string(
                    x,
                    y,
                    fit(&it.scope.word, scope_w as usize),
                    plain.fg(t.row_ink(&cr, scope_tone)),
                );
                x += scope_w + 2;
                if reason_w > 0 {
                    let reason = row_reason(it, ranked, self.w);
                    let rs = plain.fg(t.row_ink(
                        &cr,
                        if ranked.tag.is_some() {
                            t.text_secondary
                        } else {
                            t.text_muted
                        },
                    ));
                    buf.set_string(x, y, truncate_sep(&reason, reason_w as usize), rs);
                    junie_tui::ui::reveal::queue_cut(Rect::new(x, y, reason_w, 1), &reason);
                    cut |= wider_than(&reason, reason_w as usize);
                    if cut && s.focused {
                        junie_tui::ui::reveal::peek(ctx, &format!("{} — {reason}", it.label));
                    }
                } else if cut && s.focused {
                    junie_tui::ui::reveal::peek(ctx, &it.label);
                }

                if risk_w > 0 && matches!(it.risk, Risk::Destructive | Risk::Privileged) {
                    let r = it.risk.label();
                    let rx = rect.right().saturating_sub(width(r) as u16 + 1);
                    buf.set_string(rx, y, r, plain.fg(t.row_ink(&cr, t.text_secondary)));
                }
            }
        }
    }

    fn empty(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        *self.list_area = area;
        let title = if !self.query.trim().is_empty() {
            format!("No matches for “{}”", truncate(self.query, 30))
        } else {
            match self.scope {
                Scope::Parent => format!("No project root above {}", self.w.location.cwd_short()),
                Scope::Children => {
                    format!("No child projects below {}", self.w.location.cwd_short())
                }
                _ => "Nothing here yet".to_owned(),
            }
        };
        let hint_text = match self.scope {
            Scope::Here => "Esc clears the query · Ctrl+↑ widens to the parent scope",
            Scope::Children => "Esc clears the query · Ctrl+↑ returns to here",
            _ => "Esc clears the query · Ctrl+↓ narrows the scope",
        };
        empty::render(
            area,
            buf,
            t,
            &EmptyState::new(&title).hint(hint_text),
            t.canvas,
        );
    }

    fn detail(&mut self, pane: DetailPane<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (area, id) = (pane.area, pane.id);
        let scroll = pane.scroll;
        let focused = ctx.interaction.focused(id);
        let Some(it) = self.current else {
            let hint_text = if self.results.has_items() {
                "↑↓ moves the cursor"
            } else if self.query.trim().is_empty() {
                "Ctrl+↑ widens the scope"
            } else {
                "Esc clears the query"
            };
            let card_h = 7.min(area.height);
            let area = Rect::new(area.x, area.y, area.width, card_h);
            let panel = Panel::card(Some("Preview")).focused(focused);
            let inner = panel.render(area, buf, t);
            empty::render(
                inner,
                buf,
                t,
                &EmptyState::new("Nothing to preview").hint(hint_text),
                panel.bg(t),
            );
            ctx.control(id, area, false);
            return;
        };
        let it = live_item(it, self.w).into_owned();
        let meta = format!("{} · {}", it.kind.label(), it.scope.word);
        let panel = Panel::card(Some(&it.label)).focused(focused).meta(&meta);
        let bg = panel.bg(t);

        let lines = preview_lines(&it, self.w);
        let label_w = lines.iter().map(|(l, ..)| width(l)).max().unwrap_or(4) as u16 + 2;
        let inner_w = area.width.saturating_sub(4);
        let vw = inner_w.saturating_sub(label_w + 1) as usize;
        let mut flat: Vec<(String, String, Tone)> = vec![];
        for (l, v, tone, wraps) in lines {
            if wraps {
                let parts = wrap(&v, vw.max(8));
                for (i, p) in parts.into_iter().enumerate() {
                    flat.push((if i == 0 { l.clone() } else { String::new() }, p, tone));
                }
            } else {
                flat.push((l, truncate_middle(&v, vw.max(8)), tone));
            }
        }
        let card_h = (flat.len() as u16 + 3).min(area.height);
        let area = Rect::new(area.x, area.y, area.width, card_h);
        let inner = panel.render(area, buf, t);
        ctx.control(id, area, false);
        *self.preview_area = inner;
        ctx.scrollable(id, inner);
        if inner.is_empty() {
            return;
        }
        scroll.set_content(flat.len());
        scroll.set_viewport(inner.height as usize);
        let has_sb = scroll.overflows();
        for (k, i) in scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let (l, v, tone) = &flat[i];
            buf.set_string(inner.x, y, l, t.muted().bg(bg));
            let st = Style::new().fg(t.tone(*tone)).bg(bg);
            buf.set_string(
                inner.x + label_w,
                y,
                junie_tui::ui::reveal::cut(Rect::new(inner.x + label_w, y, (vw) as u16, 1), v),
                st,
            );
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(inner.x, inner.y, inner.width, inner.height),
                buf,
                ctx,
                id,
                scroll,
                focused,
            );
        }
    }

    fn summary(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let Some(it) = self.current else { return };
        let cmd = it
            .commands
            .first()
            .cloned()
            .unwrap_or_else(|| "opens inside holla".into());

        let tail = format!(" · {} · {}", it.scope.word, it.risk.label());
        let room = (area.width.saturating_sub(3) as usize).saturating_sub(2 + width(&tail));
        let line = format!("→ {}{tail}", truncate_middle(&cmd, room.max(8)));
        buf.set_string(
            area.x + 3,
            area.y,
            junie_tui::ui::reveal::cut(
                Rect::new(area.x + 3, area.y, area.width.saturating_sub(3), 1),
                &line,
            ),
            t.muted(),
        );
    }

    fn is_lead(&self, source: &Results, row: usize) -> bool {
        matches!(source.rows[row], Row::Heading { .. })
    }
}

impl ShellPage<Holla> for FinderPage {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_projection(w);
        if self.search.on_detail(cx.focus.current()) {
            return self.preview_key(key, w, cx).unwrap_or(Outcome::Ignored);
        }
        if !cx.focus.is(FINDER) {
            return Outcome::Ignored;
        }
        if key.alt() && key.code == KeyCode::Enter {
            return self.alternatives(cx);
        }
        if key.ctrl() {
            match key.code {
                KeyCode::Up => {
                    cx.go(Go::Scope(self.scope.outward()));
                    return Outcome::Changed;
                }
                KeyCode::Down => {
                    cx.go(Go::Scope(self.scope.inward()));
                    return Outcome::Changed;
                }
                KeyCode::Char('w') | KeyCode::Char('W') => return Outcome::Ignored,
                _ => {}
            }
        }
        if !self.is_query_or_result_key(key) {
            return Outcome::Ignored;
        }
        let reply = match self.search.on_key(key, &self.results, cx.focus) {
            Ok(reply) => reply,
            Err(_) => return Outcome::Ignored,
        };
        let outcome = self.serve(reply, w, cx);
        if outcome == Outcome::Ignored && key.code == KeyCode::Esc && self.scope != Scope::Here {
            cx.go(Go::Scope(Scope::Here));
            return Outcome::Changed;
        }
        outcome
    }

    fn on_paste(&mut self, text: &str, w: &mut World, cx: &mut Cx) -> Outcome {
        let reply = self.search.on_paste(text, cx.focus);
        self.serve(reply, w, cx)
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_projection(w);
        if ev.id == SCOPE {
            if !ev.is(junie_tui::core::intent::Phase::Click) {
                return Outcome::Ignored;
            }
            let next = match self.scope {
                Scope::System => Scope::Children,
                s => s.outward(),
            };
            cx.go(Go::Scope(next));
            return Outcome::Changed;
        }
        self.search.config_mut().context = cx.context;
        match self.search.pointer(ev, &self.results, cx.focus) {
            Ok(reply) => self.serve(reply, w, cx),
            Err(_) => Outcome::Ignored,
        }
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        if w.tick == self.last_tick {
            return Outcome::Ignored;
        }
        self.last_tick = w.tick;
        let rebuilt = self.ensure_projection(w);
        let stamp = self.display_stamp(w);
        let repaint = rebuilt || self.display_seen.as_ref() != Some(&stamp);
        self.display_seen = Some(stamp);
        if repaint {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn enter(&mut self, w: &mut World, _cx: &mut Cx) {
        self.ensure_projection(w);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.ensure_projection(w);
        self.display_seen = Some(self.display_stamp(w));
        let Self {
            search,
            results,
            items,
            keys,
            scope,
            group,
            explore_overflow_marker,
            list_area,
            preview_area,
            ..
        } = self;
        let query = search.query().to_owned();
        let current = search
            .selected_key()
            .and_then(|key| keys.index.get(&key))
            .and_then(|&i| items.get(i));
        let mut paint = Paint {
            w,
            items,
            results,
            scope: *scope,
            group: *group,
            query: &query,
            current,
            marker: *explore_overflow_marker,
            list_area,
            preview_area,
            cols: Columns::default(),
        };
        let _ = search.render_hosted(area, results, buf, ctx, &mut paint);
    }

    fn hints(&self, focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if self.search.on_detail(focus) {
            return vec![
                hint("↑↓", "Scroll"),
                hint("y", "Copy command"),
                hint("Enter", "Run"),
                hint("Esc", "Back to results"),
            ];
        }
        let mut v = vec![
            hint("Type", "Search"),
            hint("↑↓", "Move"),
            self.primary_hint(),
            hint("Alt+Enter", "Alternatives…"),
            hint("Tab", "Preview"),
            hint("Ctrl+↑↓", "Scope"),
        ];
        v.push(if !self.query().is_empty() {
            hint("Esc", "Clear")
        } else if self.scope != Scope::Here {
            hint("Esc", "Back to Here")
        } else if self.group.is_some() {
            hint("Esc", "Back")
        } else {
            hint("Ctrl+G", "Activities")
        });
        v
    }

    fn crumb(&self, _w: &World) -> String {
        match self.group {
            Some(g) => g.to_owned(),
            None => "Here".into(),
        }
    }

    fn is_editing(&self) -> bool {
        false
    }

    fn animating(&self, w: &World) -> bool {
        w.discovering() || w.live_activities() > 0
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(FINDER)
    }

    fn on_esc_top(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if self.group.is_some() {
            cx.go(Go::Pop);
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }
}

impl Screen for FinderPage {
    fn background_tick(&self) -> bool {
        false
    }

    fn on_context_key(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if !cx.focus.is(FINDER) {
            return Outcome::Ignored;
        }
        self.alternatives(cx)
    }

    fn status(&self, w: &World) -> StatusBits {
        let mut bits = StatusBits::default();
        if w.discovering() {
            let pending = w.pending_sources();
            let shown: Vec<&str> = pending.iter().copied().take(3).collect();
            bits.center = Some(
                StatusItem::new(
                    format!(
                        "discovering {}{} · {} of {}",
                        shown.join(", "),
                        if pending.len() > 3 { ", …" } else { "" },
                        w.sources_done(),
                        w.sources.len()
                    ),
                    Tone::Secondary,
                )
                .busy()
                .priority(6),
            );
        } else {
            let failed = w.failed_sources();
            if !failed.is_empty() {
                let names: Vec<&str> = failed.iter().map(|(n, _)| *n).collect();
                bits.center = Some(
                    StatusItem::new(format!("! {} unavailable", names.join(", ")), Tone::Error)
                        .priority(7),
                );
            } else if self.scope != Scope::Here {
                bits.center = Some(
                    StatusItem::new(
                        format!("scope {}", self.scope_title(w).to_lowercase()),
                        Tone::Secondary,
                    )
                    .priority(6),
                );
            } else if self.group.is_some() {
                bits.center = Some(StatusItem::new("discovery complete", Tone::Muted).priority(3));
            }
        }
        if !self.warnings.is_empty() {
            bits.right.push(
                StatusItem::new(
                    format!(
                        "▲ {}",
                        plural(self.warnings.len(), "config warning", "config warnings")
                    ),
                    Tone::Warning,
                )
                .priority(5),
            );
        }
        bits
    }

    fn typing_hot(&self) -> bool {
        true
    }

    fn as_finder(&mut self) -> Option<&mut FinderPage> {
        Some(self)
    }
}

pub fn item_facts(it: &Item, w: &World, theme: &Theme) -> Vec<junie_tui::widgets::props::Prop> {
    use junie_tui::widgets::props::Prop;
    let _ = theme;
    let mut v = vec![
        Prop::new("Action", it.summary.clone()).wrap(),
        Prop::new("Host", format!("{} · {}", w.host.name, w.host.role.label())),
        Prop::new(
            "In",
            if it.scope.direction == Scope::System {
                format!("{} · host-wide", w.host.name)
            } else {
                w.location.short(&it.scope.runs_in)
            },
        ),
        Prop::new(
            "Scope",
            if it.scope.direction == Scope::System || it.scope.defined_at == it.scope.runs_in {
                it.scope.word.clone()
            } else {
                format!(
                    "{} · {}",
                    it.scope.word,
                    w.location.short(&it.scope.defined_at)
                )
            },
        ),
    ];
    if !it.effects.is_empty() {
        v.push(Prop::new("Changes", it.effects.join(" · ")).wrap());
    }
    v.push(Prop::new("Risk", it.risk.label()).tone(risk_tone(it.risk)));
    v.push(Prop::new("Data", it.freshness.label()).tone(Tone::Muted));
    v
}

#[cfg(test)]
mod tests;
