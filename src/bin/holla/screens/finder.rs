use crate::screens::Holla;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::scroll::ScrollState;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::shell::Page as ShellPage;
use junie_tui::theme::{CursorStyle, Theme, Tone};
use junie_tui::ui::columns::{self, Col};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::master_detail::MasterDetail;
use junie_tui::ui::text::{fit, truncate, truncate_middle, wider_than, width, wrap};
use junie_tui::widgets::edit::EditField;
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::field_common::EditAction;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::page_header::Heading;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use crate::domain::action::{Confirmation, Item, Kind, Launch, Risk};
use crate::domain::context::Scope;
use crate::domain::projection::{CatalogStamp, Deadline, Seen};
use crate::domain::ranking::{Ranked, search};
use crate::screens::{Cx, Go, Page, Screen, StatusBits, risk_tone, scroll_drag, scroll_press};
use crate::sim::world::World;
use junie_tui::ui::text::{plural, truncate_sep};

pub const FINDER: WidgetId = WidgetId::of("here.finder");
pub const PREVIEW: WidgetId = WidgetId::of("here.preview");
pub const SCOPE: WidgetId = WidgetId::of("here.scope");
const EXPLORE_CELL: WidgetId = WidgetId::of("here.explore");

const PREVIEW_COMMANDS: usize = 6;

type PreviewLine = (String, String, Tone, bool);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Heading(String),
    Item(usize, Ranked),

    Explore(Vec<usize>),
    Blank,
}

/// What the ranked rows were built from besides the catalog and memory: the
/// query, scope and group. Compared by value, so an edit through any public
/// field is seen without a flag to forget.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ViewKey {
    query: String,
    scope: Scope,
    group: Option<&'static str>,
}

/// The visible text that changes with time but not with ranking: the data
/// freshness of the selected item, activity line counts and durations, and the
/// age of recent rows. A change repaints; it never rebuilds the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayStamp {
    data: Option<String>,
    activities: Vec<(usize, usize, String)>,
    ago: Vec<String>,
}

fn selectable(row: &Row) -> bool {
    match row {
        Row::Item(..) => true,
        Row::Explore(cells) => !cells.is_empty(),
        Row::Heading(_) | Row::Blank => false,
    }
}

pub struct FinderPage {
    pub group: Option<&'static str>,
    pub query: EditField,
    pub scope: Scope,
    source: Vec<Item>,
    source_seen: Seen<CatalogStamp>,
    source_deadline: Deadline,
    items: Vec<Item>,
    memory_seen: Option<u64>,
    decor_deadline: Deadline,
    view_seen: Option<ViewKey>,
    display_seen: Option<DisplayStamp>,

    pub warnings: Vec<String>,
    rows: Vec<Row>,
    pub cursor: usize,
    scroll: ScrollState,
    preview_scroll: ScrollState,
    explore_cursor: usize,

    pub explore_overflow_marker: bool,

    md: MasterDetail,
    list_area: Rect,
    preview_area: Rect,
    scope_area: Rect,
    explore_areas: Vec<(WidgetId, Rect, usize)>,

    pub rebuilds: u32,

    query_changed: bool,

    last_tick: u64,
}

fn query_field() -> EditField {
    use junie_tui::core::edit::{CMD_TEXT_KEYS, TextCmd};
    use junie_tui::core::keymap::KeyBinding;
    let mut keys = CMD_TEXT_KEYS.to_vec();
    keys.extend([
        (KeyBinding::ctrl('a'), TextCmd::SelectAll),
        (KeyBinding::ctrl('z').shift(), TextCmd::Redo),
        (KeyBinding::ctrl('Z'), TextCmd::Redo),
        (KeyBinding::ctrl('y'), TextCmd::Redo),
        (KeyBinding::ctrl('z'), TextCmd::Undo),
    ]);
    let mut f = EditField::single("");
    f.text_keymap = Some(keys);

    f.undo_runs = false;
    f
}

impl FinderPage {
    pub fn new(group: Option<&'static str>) -> Self {
        Self {
            group,
            query: query_field(),
            scope: Scope::Here,
            source: vec![],
            source_seen: Seen::default(),
            source_deadline: Deadline::default(),
            items: vec![],
            memory_seen: None,
            decor_deadline: Deadline::default(),
            view_seen: None,
            display_seen: None,
            warnings: vec![],
            rows: vec![],
            cursor: 0,
            scroll: ScrollState::default(),
            preview_scroll: ScrollState::default(),
            explore_cursor: 0,
            explore_overflow_marker: true,
            md: MasterDetail::new(FINDER, PREVIEW)
                .detail_share(34, 34, 48)
                .summary_rows(1),
            list_area: Rect::ZERO,
            preview_area: Rect::ZERO,
            scope_area: Rect::ZERO,
            explore_areas: vec![],
            rebuilds: 0,
            query_changed: false,
            last_tick: u64::MAX,
        }
    }

    pub fn current(&self) -> Option<&Item> {
        match self.rows.get(self.cursor) {
            Some(Row::Item(i, _)) => self.items.get(*i),
            Some(Row::Explore(cells)) => cells
                .get(self.explore_cursor)
                .and_then(|i| self.items.get(*i)),
            _ => None,
        }
    }

    pub fn current_id(&self) -> Option<String> {
        self.current().map(|i| i.id.clone())
    }

    pub fn set_scope(&mut self, s: Scope) {
        self.scope = s;
    }

    fn view_key(&self) -> ViewKey {
        ViewKey {
            query: self.query.text().to_owned(),
            scope: self.scope,
            group: self.group,
        }
    }

    /// Brings the ranked rows up to date with the world and returns whether it
    /// did any work. Each layer rebuilds only for its own dependency. A source
    /// stamp or a source deadline rebuilds the structural catalog. A source
    /// change, a memory epoch or a memory deadline decorates it again. Any of
    /// those or a changed query, scope or group reranks. A stamp is published
    /// only after the whole projection is installed.
    fn ensure_projection(&mut self, w: &World) -> bool {
        let now = w.now_secs();
        let stamp = w.catalog_stamp();
        let memory = w.memory_epoch();
        let view = self.view_key();
        let source_stale = self.source_seen.needs_rebuild(stamp) || self.source_deadline.due(now);
        let decor_stale =
            source_stale || self.memory_seen != Some(memory) || self.decor_deadline.due(now);
        let view_stale = self.view_seen.as_ref() != Some(&view);
        if !(source_stale || decor_stale || view_stale) {
            #[cfg(test)]
            self.assert_matches_reference(w);
            return false;
        }
        let keep = self.current_id();
        let source = source_stale.then(|| crate::domain::catalog::build_sources_with_warnings(w));
        let decorated = decor_stale.then(|| {
            crate::domain::catalog::decorate(
                w,
                source.as_ref().map_or(self.source.as_slice(), |s| &s.0),
            )
        });
        if let Some((items, warnings)) = source {
            self.source = items;
            self.warnings = warnings;
            self.source_deadline =
                Deadline::new(now, crate::domain::catalog::source_deadline_secs(w));
        }
        if let Some(items) = decorated {
            self.items = items;
            self.decor_deadline = Deadline::new(
                now,
                crate::domain::projection::memory_deadline(
                    &w.memory.usage,
                    &w.location.cwd,
                    &w.host.name,
                    now,
                ),
            );
        }
        self.rank_rows(w, keep);
        self.source_seen.published(stamp);
        self.memory_seen = Some(memory);
        self.view_seen = Some(view);
        self.display_seen = Some(self.display_stamp(w));
        true
    }

    fn rank_rows(&mut self, w: &World, keep: Option<String>) {
        self.rebuilds += 1;
        junie_tui::record_work!(ProjectionRebuilds, 1usize);
        let learned = if crate::domain::ranking::Query::parse(self.query.text()).is_empty() {
            None
        } else {
            w.memory
                .usage
                .learned_for(self.query.text(), &w.host.name)
                .map(str::to_owned)
        };
        let ranked = crate::domain::ranking::search_with(
            &self.items,
            self.query.text(),
            self.scope,
            self.group,
            learned.as_deref(),
        );
        let has_query = !crate::domain::ranking::Query::parse(self.query.text()).is_empty();
        let mut rows = vec![];
        if let Some(g) = self.group {
            rows.push(Row::Heading(format!(
                "{g} · {}",
                plural(ranked.len(), "entry", "entries")
            )));
            for r in ranked {
                rows.push(Row::Item(r.index, r));
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
            rows.push(Row::Heading(title));
            for r in ranked
                .into_iter()
                .filter(|r| has_query || self.items[r.index].kind != Kind::Explore)
                .take(60)
            {
                rows.push(Row::Item(r.index, r));
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
                rows.push(Row::Heading("Suggested here".into()));
                for r in suggested {
                    used.insert(r.index);
                    rows.push(Row::Item(r.index, r.clone()));
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
            rows.push(Row::Blank);
            rows.push(Row::Heading("Recent here".into()));
            if recent.is_empty() {
                rows.push(Row::Heading(
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
                rows.push(Row::Item(r.index, rr));
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
                    rows.push(Row::Blank);
                    rows.push(Row::Heading(format!("From {name} · the parent root")));
                    for r in parent {
                        used.insert(r.index);
                        rows.push(Row::Item(r.index, r.clone()));
                    }
                }
            }

            let running: Vec<&Ranked> = ranked
                .iter()
                .filter(|r| self.items[r.index].kind == Kind::Activity)
                .take(4)
                .collect();
            if !running.is_empty() {
                rows.push(Row::Blank);
                rows.push(Row::Heading("Running".into()));
                for r in running {
                    used.insert(r.index);
                    rows.push(Row::Item(r.index, r.clone()));
                }
            }

            rows.push(Row::Blank);
            rows.push(Row::Heading("Explore".into()));
            let cells: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, it)| it.kind == Kind::Explore && it.id.starts_with("explore."))
                .map(|(i, _)| i)
                .collect();
            if !cells.is_empty() {
                rows.push(Row::Explore(cells));
            }
            for r in ranked
                .iter()
                .filter(|r| self.items[r.index].id.starts_with("scope."))
            {
                rows.push(Row::Item(r.index, r.clone()));
            }
        }
        self.rows = rows;

        let query_changed = std::mem::take(&mut self.query_changed);
        let target = if query_changed {
            None
        } else {
            keep.as_deref().and_then(|id| self.locate_item(id))
        };
        match target {
            Some((row, column)) => {
                self.cursor = row;
                self.explore_cursor = column;
            }
            None => {
                self.cursor = self.first_selectable();
                self.explore_cursor = 0;
            }
        }
        self.scroll.set_content(self.rows.len());
        self.scroll.ensure_visible(self.cursor);

        if query_changed || keep != self.current_id() {
            self.preview_scroll.jump_start();
        }
    }

    /// The item as it reads at the world's current tick: the activity text and
    /// the data freshness are recomputed from the live world, everything else
    /// stays as cached.
    fn live_item<'a>(&self, it: &'a Item, w: &World) -> std::borrow::Cow<'a, Item> {
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

    fn row_reason(&self, it: &Item, rk: &Ranked, w: &World) -> String {
        let reason = match rk.ago_secs {
            Some(secs) => w.clock.ago(secs),
            None => match self.live_item(it, w) {
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

    fn display_stamp(&self, w: &World) -> DisplayStamp {
        let mut activities = vec![];
        let mut ago = vec![];
        for r in &self.rows {
            let Row::Item(i, rk) = r else { continue };
            if let Some(secs) = rk.ago_secs {
                ago.push(w.clock.ago(secs));
            }
            let it = &self.items[*i];
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
        for r in &self.rows {
            v.push(match r {
                Row::Heading(h) => format!("heading {h}"),
                Row::Blank => "blank".into(),
                Row::Explore(c) => format!(
                    "explore {:?}",
                    c.iter().map(|i| &self.items[*i].id).collect::<Vec<_>>()
                ),
                Row::Item(i, rk) => {
                    let it = &self.items[*i];
                    format!(
                        "item {} {} {:?} {}",
                        it.id,
                        rk.score,
                        rk.matched,
                        self.row_reason(it, rk, w)
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
        reference.query.buffer.set_text(self.query.text());
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

    /// Finds an item by its stable id in the rows just built. Returns the row
    /// and its Explore column (0 for ordinary rows).
    fn locate_item(&self, id: &str) -> Option<(usize, usize)> {
        self.rows
            .iter()
            .enumerate()
            .find_map(|(row, entry)| match entry {
                Row::Item(index, _) => self
                    .items
                    .get(*index)
                    .filter(|item| item.id == id)
                    .map(|_| (row, 0)),
                Row::Explore(cells) => cells
                    .iter()
                    .position(|index| self.items.get(*index).is_some_and(|item| item.id == id))
                    .map(|column| (row, column)),
                Row::Heading(_) | Row::Blank => None,
            })
    }

    fn first_selectable(&self) -> usize {
        self.rows.iter().position(selectable).unwrap_or(0)
    }

    fn scope_title(&self, w: &World) -> String {
        match self.scope {
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

    fn step(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let n = self.rows.len() as isize;
        let mut c = self.cursor as isize;
        loop {
            c += delta;
            if c < 0 || c >= n {
                return;
            }
            if selectable(&self.rows[c as usize]) {
                self.cursor = c as usize;
                self.preview_scroll.jump_start();
                self.scroll.ensure_visible(self.cursor);

                if self.cursor > 0
                    && self.cursor == self.scroll.offset
                    && matches!(self.rows[self.cursor - 1], Row::Heading(_))
                {
                    self.scroll.scroll_by(-1);
                }
                return;
            }
        }
    }

    fn query_edited(&mut self) {
        self.query_changed = true;
        self.scroll.jump_start();
    }

    fn query_selected(&self) -> bool {
        !self.query.text().is_empty()
            && self.query.buffer.selected_text() == Some(self.query.text())
    }

    fn query_apply(&mut self, action: EditAction, key: &Key) -> Outcome {
        if self.query.apply(action, key).edited {
            self.query_edited();
        }
        Outcome::Changed
    }

    fn clear_query(&mut self) {
        if self.query.text().is_empty() {
            return;
        }
        self.query.record(junie_tui::core::edit::EditKind::Other);
        self.query.buffer.set_text("");
        self.query_edited();
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

    fn preview_lines(&self, it: &Item, w: &World) -> Vec<PreviewLine> {
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

        let defined =
            if it.scope.direction == Scope::System || it.scope.defined_at == it.scope.runs_in {
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

    fn render_preview(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(PREVIEW);
        let Some(it) = self.current().cloned() else {
            let hint_text = if self.rows.iter().any(|r| matches!(r, Row::Item(..))) {
                "↑↓ moves the cursor"
            } else if self.query.text().trim().is_empty() {
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
            ctx.control(PREVIEW, area, false);
            return;
        };
        let it = self.live_item(&it, w).into_owned();
        let meta = format!("{} · {}", it.kind.label(), it.scope.word);
        let panel = Panel::card(Some(&it.label)).focused(focused).meta(&meta);
        let bg = panel.bg(t);

        let lines = self.preview_lines(&it, w);
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
        ctx.control(PREVIEW, area, false);
        self.preview_area = inner;
        ctx.scrollable(PREVIEW, inner);
        if inner.is_empty() {
            return;
        }
        self.preview_scroll.set_content(flat.len());
        self.preview_scroll.set_viewport(inner.height as usize);
        let has_sb = self.preview_scroll.overflows();
        for (k, i) in self.preview_scroll.visible_range().enumerate() {
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
                PREVIEW,
                &self.preview_scroll,
                focused,
            );
        }
    }

    fn render_query(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(FINDER);
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

        let readout = format!("scope ‹ {} ›", self.scope_word());
        let rw = width(&readout) as u16 + 2;
        let rx = area.right().saturating_sub(rw);

        let mut rs = ctx.title_style(SCOPE, bg);
        let lifted = ctx.interaction.hovered(SCOPE) || ctx.interaction.pressed(SCOPE);
        if self.scope == Scope::Here && !lifted {
            rs = rs.fg(t.legible(t.text_muted, rs.bg.unwrap_or(bg)));
        }
        buf.set_string(rx, area.y, format!(" {readout} "), rs);
        self.scope_area = Rect::new(rx, area.y, rw, 1);
        ctx.clickable(SCOPE, self.scope_area);
        let text_w = rx.saturating_sub(area.x + 3) as usize;
        let query = self.query.text().to_owned();
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
            let shown = if width(&query) > text_w {
                truncate_middle(&query, text_w)
            } else {
                query.clone()
            };
            let st = if self.query_selected() {
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
            let cx = area.x + 2 + width(&query).min(text_w) as u16;
            ctx.set_cursor(Position::new(cx, area.y));
        }
    }

    fn scope_word(&self) -> String {
        match self.scope {
            Scope::Here => "here".into(),
            Scope::Parent => "parent".into(),
            Scope::Children => "children".into(),
            Scope::System => "system".into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_rows(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(FINDER);
        self.list_area = area;
        ctx.control(FINDER, area, false);
        ctx.scrollable(FINDER, area);
        self.explore_areas.clear();
        if self.rows.iter().all(|r| !selectable(r)) {
            let title = if !self.query.text().trim().is_empty() {
                format!("No matches for “{}”", truncate(self.query.text(), 30))
            } else {
                match self.scope {
                    Scope::Parent => format!("No project root above {}", w.location.cwd_short()),
                    Scope::Children => {
                        format!("No child projects below {}", w.location.cwd_short())
                    }
                    _ => "Nothing here yet".to_owned(),
                }
            };
            let hint_text = match self.scope {
                Scope::Here => "Esc clears the query · Ctrl+↑ widens to the parent scope",
                Scope::Children => "Esc clears the query · Ctrl+↑ returns to here",
                _ => "Esc clears the query · Ctrl+↓ narrows the scope",
            };
            empty::render(area, buf, t, &EmptyState::new(&title).hint(hint_text), bg);
            return;
        }
        self.scroll.set_content(self.rows.len());
        self.scroll.set_viewport(area.height as usize);
        let has_sb = self.scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));

        let item_rows: Vec<(&Item, &Ranked)> = self
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Item(i, rk) => Some((&self.items[*i], rk)),
                _ => None,
            })
            .collect();
        let max_label = item_rows
            .iter()
            .map(|(it, _)| width(&it.label))
            .max()
            .unwrap_or(10) as u16;
        let type_w = item_rows
            .iter()
            .map(|(it, _)| width(it.kind.label()))
            .max()
            .unwrap_or(4) as u16;
        let scope_w = item_rows
            .iter()
            .map(|(it, _)| width(&it.scope.word))
            .max()
            .unwrap_or(4) as u16;
        let content_w = row_w.saturating_sub(4);

        let risk_w = item_rows
            .iter()
            .filter(|(it, _)| matches!(it.risk, Risk::Destructive | Risk::Privileged))
            .map(|(it, _)| width(it.risk.label()) as u16)
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
        let (label_w, type_w, scope_w, reason_w) = (widths[0], widths[1], widths[2], widths[3]);
        let show_reason = reason_w > 0;
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let row = Rect::new(area.x, y, row_w, 1);
            match &self.rows[i] {
                Row::Blank => {}
                Row::Heading(h) => Heading::new(h).render(
                    Rect::new(area.x + 3, y, row_w.saturating_sub(3), 1),
                    buf,
                    t,
                    bg,
                ),
                Row::Explore(cells) => {
                    let rid = FINDER.child(i);
                    let mut s = ctx.state(rid);
                    s.focused = focused && i == self.cursor;
                    let is_cursor = i == self.cursor;

                    let cr = t.cursor_row(s, bg, CursorStyle::Fill);
                    let st = cr.style;
                    fill(buf, row, st);
                    buf.set_string(row.x, y, cr.gutter, cr.gutter_style);
                    let mut x = row.x + 3;

                    ctx.clickable(rid, row);
                    let widths: Vec<u16> = cells
                        .iter()
                        .map(|idx| width(&self.items[*idx].label) as u16 + 2)
                        .collect();
                    let (shown, more) = junie_tui::widgets::chips::overflow_run(
                        &widths,
                        row.right().saturating_sub(x),
                        1,
                    );
                    for (ci, idx) in cells.iter().enumerate().take(shown) {
                        let it = &self.items[*idx];
                        let cid = EXPLORE_CELL.child(ci);
                        let chosen = is_cursor && ci == self.explore_cursor;
                        let cw = widths[ci];

                        let mut cs = ctx.title_style(cid, st.bg.unwrap_or(bg)).fg(t.text_primary);
                        if chosen {
                            cs = cs.add_modifier(Modifier::BOLD);
                            buf.set_string(x, y, "›", st.fg(t.row_ink(&cr, t.text_secondary)));
                        }
                        buf.set_string(x + 1, y, &it.label, cs);
                        ctx.clickable(cid, Rect::new(x, y, cw, 1));
                        self.explore_areas.push((cid, Rect::new(x, y, cw, 1), ci));
                        x += cw + 1;
                    }
                    if let Some(m) = more.filter(|_| self.explore_overflow_marker) {
                        buf.set_string(x, y, &m, st.fg(t.text_muted));
                    }
                }
                Row::Item(idx, rk) => {
                    let it = &self.items[*idx];
                    let rid = FINDER.child(i);
                    let mut s = ctx.state(rid);
                    s.focused = focused && i == self.cursor;
                    s.selected = i == self.cursor;

                    let cr = t.cursor_row(s, bg, CursorStyle::Fill);
                    let st = cr.style;
                    fill(buf, row, st);
                    if s.selected {
                        buf.set_string(row.x + 1, y, "›", st.fg(t.row_ink(&cr, t.text_secondary)));
                    }
                    let mut x = row.x + 3;

                    let label_cell = Rect::new(x, y, label_w, 1);
                    let mut cut = wider_than(&it.label, label_w as usize);
                    let label = truncate(&it.label, label_w as usize);
                    junie_tui::ui::reveal::queue_cut(label_cell, &it.label);
                    let mut lx = x;
                    for (bi, ch) in label.char_indices() {
                        let mut cs = st;
                        if rk.matched.contains(&bi) {
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
                    if show_reason {
                        let reason = self.row_reason(it, rk, w);
                        let rs = plain.fg(t.row_ink(
                            &cr,
                            if rk.tag.is_some() {
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
                        let rx = row.right().saturating_sub(width(r) as u16 + 1);
                        buf.set_string(rx, y, r, plain.fg(t.row_ink(&cr, t.text_secondary)));
                    }
                    ctx.clickable(rid, row);
                }
            }
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(area.x, area.y, area.width, area.height),
                buf,
                ctx,
                FINDER,
                &self.scroll,
                focused,
            );
        }
    }

    fn locate_row(&self, id: WidgetId) -> Option<usize> {
        self.scroll.visible_range().find(|&i| FINDER.child(i) == id)
    }

    fn activate_current(&mut self, cx: &mut Cx) -> Outcome {
        let Some(it) = self.current() else {
            return Outcome::Consumed;
        };
        match &it.launch {
            Launch::Group(g) => cx.go(Go::Push(Page::Finder { group: Some(g) })),
            Launch::Scope(s) => cx.go(Go::Scope(*s)),
            _ => cx.go(Go::Run {
                item: it.id.clone(),
                args: vec![],
            }),
        }
        Outcome::Changed
    }

    fn alternatives(&mut self, cx: &mut Cx) -> Outcome {
        let Some(it) = self.current() else {
            return Outcome::Consumed;
        };
        let anchor = self
            .scroll
            .visible_range()
            .find(|&i| i == self.cursor)
            .map(|i| {
                Rect::new(
                    self.list_area.x + 6,
                    self.list_area.y + (i - self.scroll.offset) as u16,
                    1,
                    1,
                )
            })
            .unwrap_or(self.list_area);
        cx.go(Go::Alternatives {
            item: it.id.clone(),
            anchor,
        });
        Outcome::Changed
    }

    fn preview_key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.preview_scroll.scroll_by(-1);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.preview_scroll.scroll_by(1);
                Outcome::Changed
            }
            KeyCode::Char('y') => {
                if let Some(it) = self.current() {
                    let cmd = it.commands.join("\n");
                    if cmd.is_empty() {
                        cx.status("Nothing to copy: this row has no command");
                    } else {
                        cx.copy(cmd);
                    }
                }
                Outcome::Changed
            }
            KeyCode::Esc => {
                self.md.close_drawer(cx.focus);
                Outcome::Changed
            }
            KeyCode::Enter => self.activate_current(cx),
            _ => Outcome::Ignored,
        }
    }
}

impl FinderPage {
    fn click(&mut self, id: WidgetId, pos: Position, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_projection(w);
        if id == SCOPE {
            let next = match self.scope {
                Scope::System => Scope::Children,
                s => s.outward(),
            };
            cx.go(Go::Scope(next));
            return Outcome::Changed;
        }
        if id == scrollbar::id_for(FINDER) {
            cx.focus.focus(FINDER);
            return scroll_press(self.list_area, pos, &mut self.scroll);
        }
        if id == scrollbar::id_for(PREVIEW) {
            cx.focus.focus(PREVIEW);
            return scroll_press(self.preview_area, pos, &mut self.preview_scroll);
        }
        if id == PREVIEW {
            cx.focus.focus(PREVIEW);
            return Outcome::Changed;
        }
        if let Some((_, _, ci)) = self.explore_areas.iter().find(|(cid, _, _)| *cid == id) {
            if let Some(r) = self.rows.iter().position(|r| matches!(r, Row::Explore(_))) {
                self.cursor = r;
                self.explore_cursor = *ci;
                self.preview_scroll.jump_start();
            }
            cx.focus.focus(FINDER);
            return self.activate_current(cx);
        }
        if let Some(i) = self.locate_row(id) {
            if selectable(&self.rows[i]) {
                self.cursor = i;
                self.preview_scroll.jump_start();
            }
            cx.focus.focus(FINDER);
            return Outcome::Changed;
        }
        if id == FINDER {
            cx.focus.focus(FINDER);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}

impl ShellPage<Holla> for FinderPage {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_projection(w);
        if cx.focus.is(PREVIEW) {
            let typed = matches!(
                key.code,
                KeyCode::Char(c) if !key.chord() && !matches!(c, 'j' | 'k' | 'y')
            );
            if typed {
                self.md.close_drawer(cx.focus);
            } else {
                return self.preview_key(key, cx);
            }
        }
        if !cx.focus.is(FINDER) {
            return Outcome::Ignored;
        }
        if key.alt() && key.code == KeyCode::Enter {
            return self.alternatives(cx);
        }
        if key.ctrl() {
            return match key.code {
                KeyCode::Up => {
                    cx.go(Go::Scope(self.scope.outward()));
                    Outcome::Changed
                }
                KeyCode::Down => {
                    cx.go(Go::Scope(self.scope.inward()));
                    Outcome::Changed
                }

                KeyCode::Char('w') | KeyCode::Char('W') => Outcome::Ignored,

                _ => match self.query.action(key) {
                    EditAction::SelectAll => {
                        let any = !self.query.text().is_empty();
                        self.query_apply(EditAction::SelectAll, key);
                        cx.status(if any {
                            "Query selected · typing replaces it"
                        } else {
                            "Nothing to select"
                        });
                        Outcome::Changed
                    }
                    EditAction::Undo => {
                        let can = self.query.can_undo();
                        self.query_apply(EditAction::Undo, key);
                        cx.status(if can { "Undone" } else { "Nothing to undo" });
                        Outcome::Changed
                    }
                    EditAction::Redo => {
                        let can = self.query.can_redo();
                        self.query_apply(EditAction::Redo, key);
                        cx.status(if can { "Redone" } else { "Nothing to redo" });
                        Outcome::Changed
                    }
                    a @ (EditAction::Apply(_)
                    | EditAction::Paste
                    | EditAction::Copy
                    | EditAction::Cut) => self.query_apply(a, key),
                    _ => Outcome::Ignored,
                },
            };
        }
        if key.alt() && key.code == KeyCode::Backspace {
            let a = self.query.action(key);
            return self.query_apply(a, key);
        }
        match key.code {
            KeyCode::Up => {
                self.step(-1);
                Outcome::Changed
            }
            KeyCode::Down => {
                self.step(1);
                Outcome::Changed
            }
            KeyCode::PageUp => {
                for _ in 0..self.scroll.viewport_len.max(1) {
                    self.step(-1);
                }
                Outcome::Changed
            }
            KeyCode::PageDown => {
                for _ in 0..self.scroll.viewport_len.max(1) {
                    self.step(1);
                }
                Outcome::Changed
            }
            KeyCode::Home => {
                self.cursor = self.first_selectable();
                self.preview_scroll.jump_start();
                self.scroll.jump_start();
                Outcome::Changed
            }
            KeyCode::End => {
                self.cursor = self.rows.iter().rposition(selectable).unwrap_or(0);
                self.preview_scroll.jump_start();
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Left | KeyCode::Right
                if matches!(self.rows.get(self.cursor), Some(Row::Explore(_))) =>
            {
                let n = match &self.rows[self.cursor] {
                    Row::Explore(c) => c.len(),
                    _ => 0,
                };
                let before = self.explore_cursor;
                if key.code == KeyCode::Left {
                    self.explore_cursor = self.explore_cursor.saturating_sub(1);
                } else {
                    self.explore_cursor = (self.explore_cursor + 1).min(n.saturating_sub(1));
                }
                if self.explore_cursor != before {
                    self.preview_scroll.jump_start();
                }
                Outcome::Changed
            }
            KeyCode::Enter => self.activate_current(cx),
            KeyCode::Backspace if self.query.text().is_empty() => Outcome::Consumed,
            KeyCode::Backspace => {
                let a = self.query.action(key);
                self.query_apply(a, key)
            }
            KeyCode::Esc => {
                if self.query_selected() {
                    self.query.buffer.clear_selection();
                    return Outcome::Changed;
                }
                if !self.query.text().is_empty() {
                    self.clear_query();
                    return Outcome::Changed;
                }
                if self.scope != Scope::Here {
                    cx.go(Go::Scope(Scope::Here));
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            KeyCode::Char(c) if !key.chord() => self.query_apply(EditAction::Insert(c), key),
            _ => Outcome::Ignored,
        }
    }

    fn on_paste(&mut self, text: &str, _w: &mut World, _cx: &mut Cx) -> Outcome {
        let flat: String = text.split(['\n', '\r']).collect::<Vec<_>>().join(" ");
        let flat = flat.trim().to_owned();
        if flat.is_empty() || !self.query.paste(&flat) {
            return Outcome::Consumed;
        }
        self.query_edited();
        Outcome::Changed
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            if axis != Axis::Vertical {
                return Outcome::Ignored;
            }
            if id == PREVIEW {
                self.preview_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            if id == FINDER {
                self.scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        match ev.phase() {
            Some(Phase::Press) => {
                if id == scrollbar::id_for(FINDER) {
                    return scroll_press(self.list_area, pos, &mut self.scroll);
                }
                if id == scrollbar::id_for(PREVIEW) {
                    return scroll_press(self.preview_area, pos, &mut self.preview_scroll);
                }
                Outcome::Ignored
            }
            Some(Phase::Drag) => {
                if id == scrollbar::id_for(FINDER) {
                    return scroll_drag(self.list_area, pos, &mut self.scroll);
                }
                if id == scrollbar::id_for(PREVIEW) {
                    return scroll_drag(self.preview_area, pos, &mut self.preview_scroll);
                }
                Outcome::Ignored
            }
            Some(Phase::Click) => {
                if let Some(i) = self.locate_row(id)
                    && matches!(self.rows[i], Row::Item(..))
                    && ClickPolicy::DATA.activates(ev.count(), self.cursor == i)
                {
                    self.cursor = i;
                    self.preview_scroll.jump_start();
                    return self.activate_current(cx);
                }
                self.click(id, pos, w, cx)
            }
            Some(Phase::Secondary) => {
                if let Some(i) = self.locate_row(id)
                    && let Some(Row::Item(ix, _)) = self.rows.get(i)
                    && let Some(item) = self.items.get(*ix).map(|it| it.id.clone())
                {
                    if cx.context.moves_cursor {
                        self.cursor = i;
                        self.preview_scroll.jump_start();
                    }
                    cx.focus.focus(FINDER);

                    cx.go(Go::Alternatives {
                        item,
                        anchor: Rect::new(pos.x, pos.y, 1, 1),
                    });
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
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
        let t = ctx.theme;
        let query = Rect::new(area.x, area.y, area.width, 1);
        self.render_query(query, buf, ctx);
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(2),
        );
        let l = self.md.layout(body, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            self.render_rows(l.master, buf, ctx, w);
        }
        if !l.detail.is_empty() {
            self.render_preview(l.detail, buf, ctx, w);
        }

        if !l.summary.is_empty()
            && let Some(it) = self.current()
        {
            let cmd = it
                .commands
                .first()
                .cloned()
                .unwrap_or_else(|| "opens inside holla".into());

            let tail = format!(" · {} · {}", it.scope.word, it.risk.label());
            let room = (body.width.saturating_sub(3) as usize).saturating_sub(2 + width(&tail));
            let line = format!("→ {}{tail}", truncate_middle(&cmd, room.max(8)));
            buf.set_string(
                body.x + 3,
                l.summary.y,
                junie_tui::ui::reveal::cut(
                    Rect::new(body.x + 3, l.summary.y, body.width.saturating_sub(3), 1),
                    &line,
                ),
                t.muted(),
            );
        }
        self.md.finish(ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if focus == Some(PREVIEW) {
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
        v.push(if !self.query.text().is_empty() {
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
