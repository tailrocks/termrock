use std::collections::BTreeSet;

use junie_tui::clock::Clock;
use junie_tui::core::event::{Input, Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::ItemKey;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{
    MouseTarget, PointerEvent, PointerHost, PointerParts, PointerRouter,
};
use junie_tui::core::response::Response;
use junie_tui::shell::{self, PageKey, Shell};
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::{
    DismissReason, Dismissed, Layer, LayerCx, LayerPointer, LayerTag, Taken,
};
use junie_tui::ui::leave::{Leave, LeaveGuard, Resolved, Stakes};
use junie_tui::ui::text::{crumb_fit, plural, truncate, truncate_middle, width};
use junie_tui::widgets::brand::Lockup;
use junie_tui::widgets::breadcrumb::Breadcrumb;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::help::{HelpOverlay, HelpSection};
use junie_tui::widgets::hintbar::HintBar;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::hint;
use junie_tui::widgets::menu::{ContextMenu, MenuBar, MenuBarAction, MenuItem, Placement};
use junie_tui::widgets::option_list;
use junie_tui::widgets::picker::{Picker, PickerAction};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::statusbar::{StatusBar, StatusItem};
use junie_tui::widgets::tabs::{TabAction, TabItem, Tabs};
use junie_tui::widgets::too_small::TooSmall;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use crate::domain::action::{Confirmation, Item, Launch, Risk};
use crate::domain::activity::{ActivityKind, ActivityState};
use crate::domain::context::{HostRole, Scope};
use crate::domain::plan::{PlanPhase, StepState};
use crate::scenario::{Motion, Scenario};
use crate::screens::activity::ActivityTab;
use crate::screens::cleanup::CleanupPage;
use crate::screens::disk::DiskPage;
use crate::screens::files::FilesPage;
use crate::screens::finder::FinderPage;
use crate::screens::modals::{TextLine, TextModal};
use crate::screens::plan::{PlanReviewPage, PlanTab};
use crate::screens::review::{ArgsPage, GatePage, TrustPage};
use crate::screens::snapshot::SnapshotPage;
use crate::screens::{
    Cx, GateTarget, Go, Holla, LayerSlot, Layers, ModalResult, Page, Request, Screen,
};
use crate::sim::world::{Msg, QuitWork, STOPPING_REFUSAL, World};

pub const BRAND_MARK: &str = "holla❯";

pub const MENU: WidgetId = WidgetId::of("shell.menu");
pub const TABS: WidgetId = WidgetId::of("shell.tabs");

pub const CRUMBS: WidgetId = WidgetId::of("shell.crumbs");

const CHROME: shell::Chrome = shell::Chrome::new().header(4).gap(0).footer(3).inset(1);

const LEAVE_TAG: &str = "leave";
const STATUS_PATH: WidgetId = WidgetId::of("shell.status.path");
const STATUS_RUNNING: WidgetId = WidgetId::of("shell.status.running");
const STATUS_GIT: WidgetId = WidgetId::of("shell.status.git");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabKind {
    Here,
    Activity(String),
    Plan(String),
}

pub struct TabEntry {
    pub kind: TabKind,

    pub pending: Vec<Request>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AltCmd {
    Run,
    Copy,
    Insert,
    Args,
    Alternative(String),
    Pin,
    Alias,
    Hide,
    Reset,
    Why,
}

pub struct App {
    pub theme: Theme,
    pub scenario: Scenario,
    pub world: World,

    pub router: shell::Router<Box<dyn Screen>>,
    pub tabs: Vec<TabEntry>,

    leave: LeaveGuard<shell::Nav<Page>>,
    pub(crate) modals: Layers,

    pub shell: Shell<Vec<(String, Tone)>>,

    menus: shell::Menus<HollaCmd>,
    menu: MenuBar,
    strip: Tabs,

    pub crumbs: Breadcrumb,
    menu_item: Option<String>,
    menu_actions: Vec<AltCmd>,
    pending_gate: Option<GateTarget>,
    pending_confirm: Option<(String, Vec<(String, String)>)>,
    /// Activity IDs whose "Stop and close" was confirmed while the process
    /// was still live. The tab closes on the activity's terminal event.
    pending_activity_closes: BTreeSet<String>,
    pub clipboard_gen: u32,

    pub quitting: bool,

    pub last_query: Option<String>,

    pub keys: shell::ShellKeys<HollaCmd>,
}

fn plan_key(id: &str) -> ItemKey {
    ItemKey::text(&format!("plan:{id}"))
}

impl App {
    pub fn launch(
        scenario: Scenario,
        motion: Motion,
        frame: u64,
        theme: Theme,
        history: bool,
    ) -> Self {
        let mut world = crate::domain::fixtures::world_for(scenario, motion);
        if !history {
            world.memory.usage = crate::domain::usage::UsageStore::disabled();
        }
        let mut shell = Shell::new();
        shell.motion = motion;
        let mut app = Self {
            theme,
            scenario,
            world,
            router: shell::Router::new(Box::new(FinderPage::new(None))),
            tabs: vec![TabEntry {
                kind: TabKind::Here,
                pending: vec![],
            }],
            leave: LeaveGuard::new(WidgetId::of("holla.leave")).save(false),
            modals: Layers::new(),
            shell,
            menus: Self::menus(),
            menu: Self::menus().bar(MENU).brand(Lockup::new(BRAND_MARK)),
            strip: Tabs::with_items(TABS, vec![]),
            crumbs: Breadcrumb::new(CRUMBS, vec![]),
            menu_item: None,
            menu_actions: vec![],
            pending_gate: None,
            pending_confirm: None,
            pending_activity_closes: BTreeSet::new(),
            clipboard_gen: 0,
            quitting: false,
            last_query: None,

            keys: shell_keys(),
        };
        app.start();

        let running = app.world.clock.running;
        app.world.clock.running = true;
        for _ in 0..frame {
            app.on_tick();
        }
        app.world.clock.running = running && !app.shell.test_requests;
        app
    }

    fn start(&mut self) {
        self.enter_top();
        match self.scenario {
            Scenario::DockerCleanup => {
                if let Some(f) = self.finder_mut() {
                    f.set_query("docker clean");
                }
                self.enter_top();
            }
            Scenario::DiskCleanup => {
                let cwd = self.world.location.cwd.clone();
                self.push_page(Page::Disk { path: cwd });
            }
            Scenario::UpgradePlan => {
                self.push_page(Page::PlanReview {
                    plan: "upgrade".into(),
                });
            }
            Scenario::ActivitiesMulti => {
                let ids: Vec<String> = self.world.activities.iter().map(|a| a.id.clone()).collect();
                for id in ids {
                    self.open_activity_tab(&id, false);
                }
                self.router.select(0);
                self.enter_top();
            }
            Scenario::LaunchFailure => {
                let ids: Vec<String> = self.world.activities.iter().map(|a| a.id.clone()).collect();
                for id in ids {
                    self.open_activity_tab(&id, true);
                }
            }
            _ => {}
        }
    }

    fn finder_mut(&mut self) -> Option<&mut FinderPage> {
        self.router.pages_mut().next()?.as_finder()
    }

    pub fn active(&self) -> usize {
        self.router.active()
    }

    pub fn set_status(&mut self, s: &str, tone: Tone) {
        self.shell.status.set_at(s, tone, self.world.clock.now());
    }

    pub fn set_status_parts(&mut self, parts: Vec<(String, Tone)>, tone: Tone) {
        let text: String = parts.iter().map(|(p, _)| p.as_str()).collect();
        self.shell
            .status
            .set_with_at(text, tone, parts, self.world.clock.now());
    }

    fn top(&self) -> Option<&dyn Screen> {
        Some(self.router.current().as_ref())
    }

    fn animating(&self) -> bool {
        self.shell.pointer.has_flash()
            || self.world.discovering()
            || self.world.live_activities() > 0
            || self
                .world
                .plans
                .iter()
                .any(|p| p.phase == PlanPhase::Running)
            || self.top().is_some_and(|s| s.animating(&self.world))
    }

    pub fn tick_interval(&self) -> std::time::Duration {
        if self.shell.motion == Motion::Paused {
            return std::time::Duration::from_millis(500);
        }
        std::time::Duration::from_millis(if self.animating() { 80 } else { 200 })
    }

    fn now_ms(&self) -> u64 {
        self.world.now_ms().max(0) as u64
    }

    fn with_top<F: FnOnce(&mut Box<dyn Screen>, &mut World, &mut Cx) -> Outcome>(
        &mut self,
        f: F,
    ) -> Outcome {
        let active = self.active();
        let (o, reqs) = self.deliver_tab(active, true, Outcome::Ignored, f);
        o.or(self.apply_requests(reqs, active))
    }

    fn deliver_tab(
        &mut self,
        ti: usize,
        shown: bool,
        absent: Outcome,
        f: impl FnOnce(&mut Box<dyn Screen>, &mut World, &mut Cx) -> Outcome,
    ) -> (Outcome, Vec<Request>) {
        self.shell
            .deliver(shown, |cx| match self.router.top_of_mut(ti) {
                Some(s) => f(s, &mut self.world, cx),
                None => absent,
            })
    }

    fn enter_top(&mut self) {
        self.with_top(|s, w, cx| {
            s.enter(w, cx);
            Outcome::Changed
        });
        let pf = self.top().and_then(|s| s.primary_focus());
        self.shell.focus.set(pf);
    }

    fn blur_all(&mut self, ch: FocusChange) -> Outcome {
        let out = self.modals.on_blur(ch);
        out.or(self.deliver_all(|s, w, cx| s.on_blur(ch, w, cx)))
    }

    fn deliver_all(
        &mut self,
        f: impl FnMut(&mut Box<dyn Screen>, &mut World, &mut Cx) -> Outcome,
    ) -> Outcome {
        self.deliver_tabs(|_| true, f)
    }

    /// The tick fanout. A hidden tab whose top page only refreshes a
    /// projection skips its tick and catches up when it is entered.
    fn deliver_ticks(&mut self) -> Outcome {
        self.deliver_tabs(|s| s.background_tick(), |s, w, cx| s.on_tick(w, cx))
    }

    fn deliver_tabs(
        &mut self,
        runs_hidden: impl Fn(&dyn Screen) -> bool,
        mut f: impl FnMut(&mut Box<dyn Screen>, &mut World, &mut Cx) -> Outcome,
    ) -> Outcome {
        let mut out = Outcome::Ignored;
        for ti in 0..self.router.tab_count() {
            let shown = ti == self.active();
            if !shown
                && self
                    .router
                    .top_of(ti)
                    .is_some_and(|s| !runs_hidden(s.as_ref()))
            {
                continue;
            }
            let (o, reqs) = self.deliver_tab(ti, shown, Outcome::Ignored, &mut f);
            if shown {
                out = out.or(o).or(self.apply_requests(reqs, ti));
            } else {
                self.tabs[ti].pending.extend(reqs);
            }
        }
        out
    }

    fn apply_pending(&mut self, out: Outcome) -> Outcome {
        let active = self.active();
        match self
            .tabs
            .get_mut(active)
            .map(|t| std::mem::take(&mut t.pending))
        {
            Some(pending) if !pending.is_empty() => out.or(self.apply_requests(pending, active)),
            _ => out,
        }
    }

    fn handle_input(&mut self, input: Input) -> Outcome {
        match input {
            Input::Resize(..) => Outcome::Changed,
            Input::Tick => self.on_tick(),
            Input::Mouse(m) => PointerRouter::route(self, m),
            other => shell::KeyRouter::input(self, other),
        }
    }

    fn on_tick(&mut self) -> Outcome {
        let msgs = self.world.tick();
        let mut out = if self.animating() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        };

        if let Some(idx) = crate::screens::cleanup::step(&mut self.world) {
            let r = &self.world.reports[idx];
            let (summary, tone) = (
                r.summary(),
                if r.incomplete() {
                    Tone::Error
                } else {
                    Tone::Secondary
                },
            );
            self.set_status(&summary, tone);
            out = Outcome::Changed;
        }
        if self.world.cleanup_job.is_some() {
            out = Outcome::Changed;
        }
        let now = self.now_ms();
        if self.shell.pointer.expire_flash(now) {
            out = Outcome::Changed;
        }

        out = out.or(PointerRouter::tick(self));
        if self.shell.status.expire(self.world.clock.now()) {
            out = Outcome::Changed;
        }
        if self
            .modals
            .top()
            .is_some_and(|m| m.tag.kind == "activities")
        {
            let items = self.activity_picker_items();
            if let Some(p) = self.modals.top_as_mut::<Picker>() {
                p.replace_items(items)
                    .unwrap_or_else(|error| p.report_item_error(error));
            }
        }
        for entry in self.modals.iter_mut() {
            out = out.or(entry.layer.on_tick(&()));
        }
        out = out.or(self.deliver_ticks());
        for m in msgs {
            out = out.or(self.dispatch_msg(m));
        }
        if self.quitting {
            let left = self.world.quit_work();
            if left.is_empty() {
                self.shell.quit = true;
            } else {
                let text = self.quit_wait_text(left);
                self.set_status(&text, Tone::Secondary);
            }
            out = Outcome::Changed;
        }
        out
    }

    fn dispatch_msg(&mut self, m: Msg) -> Outcome {
        match &m {
            Msg::ActivityEnded { id, ok } => {
                if let Some(a) = self.world.activity(id) {
                    let name = a.name.clone();
                    let exit = a.exit.unwrap_or(0);
                    if *ok {
                        self.set_status(&format!("{name} finished · exit 0"), Tone::Secondary);
                    } else if a.state == ActivityState::Stopped {
                        self.set_status(&format!("{name} stopped · exit {exit}"), Tone::Secondary);
                    } else {
                        self.set_status(&format!("{name} failed · exit {exit}"), Tone::Error);
                    }
                }
                if self.pending_activity_closes.contains(id) {
                    self.close_activity_tab_if_finished(id);
                }
            }
            Msg::BatchDone { id, ok } => {
                let parts = self
                    .world
                    .batches
                    .iter()
                    .find(|b| &b.id == id)
                    .map(|b| {
                        let (okn, failed, cancelled, _) = b.tally(&self.world);
                        let sep = || (" · ".to_owned(), Tone::Secondary);
                        vec![
                            (b.label.clone(), Tone::Secondary),
                            sep(),
                            (
                                format!("{okn} ok"),
                                if okn > 0 { Tone::Success } else { Tone::Muted },
                            ),
                            sep(),
                            (
                                format!("{failed} failed"),
                                if failed > 0 { Tone::Error } else { Tone::Muted },
                            ),
                            sep(),
                            (format!("{cancelled} cancelled"), Tone::Muted),
                        ]
                    })
                    .unwrap_or_default();
                self.set_status_parts(parts, if *ok { Tone::Success } else { Tone::Error });
            }
            Msg::PlanDone { plan } => {
                if let Some(p) = self.world.plan(plan) {
                    let cancelled = p
                        .steps
                        .iter()
                        .filter(|s| s.state == StepState::Cancelled)
                        .count();
                    let (_, failed, ..) = p.outcome();
                    let mut text = if p.stopped {
                        format!("{} stopped · {cancelled} cancelled", p.title)
                    } else if cancelled > 0 {
                        format!("{} finished · {cancelled} cancelled", p.title)
                    } else if failed > 0 {
                        format!("{} · {failed} failed", p.title)
                    } else {
                        format!("{} finished", p.title)
                    };
                    if cancelled > 0 && failed > 0 {
                        text.push_str(&format!(" · {failed} failed"));
                    }
                    let tone = if failed > 0 {
                        Tone::Error
                    } else if cancelled > 0 {
                        Tone::Warning
                    } else {
                        Tone::Secondary
                    };
                    self.set_status(&text, tone);
                }
            }
            Msg::DiscoveryDone { .. } | Msg::PlanStepEnded { .. } | Msg::ScanDone { .. } => {}
        }

        Outcome::Changed.or(self.deliver_all(|s, w, cx| s.on_msg(&m, w, cx)))
    }

    fn attached(&self) -> bool {
        self.top().is_some_and(|s| s.is_editing())
            && matches!(
                self.tabs.get(self.active()).map(|t| &t.kind),
                Some(TabKind::Activity(_))
            )
    }

    fn open_menu_key(&mut self, key: &Key) -> Outcome {
        let (o, ev) = self.menu.on_key(key);
        match ev {
            Some(MenuBarAction::Brand) => {
                self.open_about();
                Outcome::Changed
            }
            Some(act) => match self.menus.command(act) {
                Some(cmd) => self.run_cmd(cmd),
                None => o.or(Outcome::Changed),
            },
            _ => o.or(Outcome::Changed),
        }
    }

    fn chrome_focus_key(&mut self, key: &Key) -> Outcome {
        if self.shell.context.keys.matches(key) {
            let o = self.with_top(|s, w, cx| s.on_context_key(w, cx));
            if o != Outcome::Ignored {
                return o;
            }
        }
        if self.shell.focus.is(MENU) {
            let (o, ev) = self.menu.on_key(key);
            if let Some(MenuBarAction::Opened(_)) = ev {
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
            if key.code == KeyCode::Esc {
                let pf = self.top().and_then(|s| s.primary_focus());
                self.shell.focus.set(pf);
                return Outcome::Changed;
            }
        }
        if self.shell.focus.is(TABS) {
            let (o, ev) = self.strip.on_key(key);
            match ev {
                Some(TabAction::Activated(i)) => {
                    self.activate_tab(i);
                    self.shell.focus.focus(TABS);
                    return Outcome::Changed;
                }
                Some(TabAction::Close(i)) => {
                    self.router.select(i);
                    self.go(Go::CloseTab);
                    return Outcome::Changed;
                }
                _ => {}
            }
            if o.consumed() {
                return o;
            }
            if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                let pf = self.top().and_then(|s| s.primary_focus());
                self.shell.focus.set(pf);
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }

    fn tab_key(&mut self, n: usize) -> Outcome {
        if n < self.router.tab_count() {
            self.activate_tab(n);
        } else {
            self.set_status(&format!("No tab {n}"), Tone::Secondary);
        }
        Outcome::Changed
    }

    fn menus() -> shell::Menus<HollaCmd> {
        use HollaCmd as C;
        let file = vec![
            (MenuItem::new("Run").shortcut("Enter"), C::Run),
            (
                MenuItem::new("Alternatives…").shortcut("Alt+Enter"),
                C::Alternatives,
            ),
            (MenuItem::new("Copy command").separator(), C::CopyCommand),
            (
                MenuItem::new("Close tab").shortcut("Ctrl+W").separator(),
                C::CloseTab,
            ),
            (MenuItem::new("Quit").shortcut("Ctrl+Q"), C::Quit),
        ];
        let mut go = vec![
            (MenuItem::new("Here").shortcut("Alt+0"), C::Tab(0)),
            (
                MenuItem::new("Activities…").shortcut("Ctrl+G").separator(),
                C::Activities,
            ),
            (
                MenuItem::new("Parent scope").shortcut("Ctrl+↑"),
                C::Scope(Scope::Parent),
            ),
            (
                MenuItem::new("Children scope").shortcut("Ctrl+↓"),
                C::Scope(Scope::Children),
            ),
            (
                MenuItem::new("System scope").separator(),
                C::Scope(Scope::System),
            ),
        ];
        for g in ["Tasks", "Git", "Files", "Disk", "Services", "System"] {
            go.push((MenuItem::new(g), C::Group(g)));
        }
        let help = vec![
            (MenuItem::new("Key reference").shortcut("F1"), C::Help),
            (MenuItem::new("Why is this here?"), C::Why),
            (MenuItem::new("About holla"), C::About),
        ];
        shell::Menus::new()
            .menu("File", file)
            .menu("Go", go)
            .menu("Help", help)
    }

    fn run_cmd(&mut self, cmd: HollaCmd) -> Outcome {
        let key = |code: KeyCode, mods: KeyModifiers| Key { code, mods };
        match cmd {
            HollaCmd::Run => {
                return shell::KeyRouter::replay(self, &key(KeyCode::Enter, KeyModifiers::NONE));
            }
            HollaCmd::Alternatives => {
                return shell::KeyRouter::replay(self, &key(KeyCode::Enter, KeyModifiers::ALT));
            }
            HollaCmd::CopyCommand => {
                let cmd = self
                    .current_item()
                    .map(|i| i.commands.join("\n"))
                    .unwrap_or_default();
                if cmd.is_empty() {
                    self.set_status("Nothing to copy here", Tone::Secondary);
                } else {
                    self.copy(cmd);
                }
            }
            HollaCmd::CloseTab => self.go(Go::CloseTab),
            HollaCmd::Quit => self.request_quit(),
            HollaCmd::Tab(n) => return self.tab_key(n as usize),
            HollaCmd::Activities => self.open_activities(),
            HollaCmd::Scope(s) => self.go(Go::Scope(s)),
            HollaCmd::Group(g) => {
                self.activate_tab(0);
                self.push_page(Page::Finder { group: Some(g) });
            }
            HollaCmd::Help => self.open_help(),
            HollaCmd::Why => self.open_why(),
            HollaCmd::About => self.open_about(),
        }
        Outcome::Changed
    }

    fn current_item(&mut self) -> Option<Item> {
        if self.active() != 0 {
            return None;
        }
        let id = self.finder_page_current_id()?;
        self.world.items().into_iter().find(|i| i.id == id)
    }

    fn finder_page_current_id(&mut self) -> Option<String> {
        self.router.top_of_mut(0)?.as_finder()?.current_id()
    }

    fn open_about(&mut self) {
        let lines = vec![
            TextLine::kv("Product", "holla · this folder, this host, right now"),
            TextLine::kv("Scenario", self.world.scenario.name()),
            TextLine::kv(
                "Simulation",
                "mise, git, gh, docker, btm, pg_activity, ssh and the filesystem are in-memory fixtures · no stack command is ever executed",
            ),
            TextLine::kv(
                "Design system",
                "Junie TUI (junie_tui) · one accent, state as geometry, one hint bar",
            ),
            TextLine::kv(
                "Model",
                "a tab is something that runs · a page is something you are deciding",
            ),
        ];
        let m = TextModal::new(WidgetId::of("about"), "About holla❯", lines).width(70);
        self.open_layer(Box::new(m), LayerTag::new("about"));
    }

    fn open_help(&mut self) {
        let kind = self
            .tabs
            .get(self.active())
            .map(|t| t.kind.clone())
            .unwrap_or(TabKind::Here);

        let mut sections: Vec<HelpSection> = vec![];
        let mut section = |title: &str, rows: &[(&str, &str)]| {
            sections.push(
                rows.iter()
                    .fold(HelpSection::new(title), |s, (k, v)| s.row(*k, *v)),
            );
        };
        match kind {
            TabKind::Here => {
                section(
                    "Finder",
                    &[
                        (
                            "Type",
                            "search actions and resources together · @parent @children @system @all narrow the scope",
                        ),
                        ("↑↓", "move · PgUp PgDn Home End"),
                        (
                            "Enter",
                            "primary action · destructive rows open a review instead",
                        ),
                        (
                            "Alt+Enter",
                            "alternatives · run, copy, insert, arguments, pin, alias, hide, why",
                        ),
                        ("Tab", "preview › tab strip › finder"),
                        (
                            "Ctrl+↑ / Ctrl+↓",
                            "scope outward (parent, system) / inward (children)",
                        ),
                        ("Esc", "clear query › scope back to here › back a page"),
                        (
                            "Aliases",
                            "gp pull · du disk usage · test the test task here · d then u · dc docker cleanup",
                        ),
                    ],
                );
                section(
                    "Pages",
                    &[
                        (
                            "Disk",
                            "↑↓ move · Space select · a all · Enter drill · c cleanup plan · p details",
                        ),
                        (
                            "Plan review",
                            "Space include or exclude · u undo · Enter step facts · c confirm",
                        ),
                        (
                            "Gates",
                            "Gate 1 reviews facts and the full sequence · Gate 2 types the target-bound phrase",
                        ),
                    ],
                );
            }
            TabKind::Activity(_) => {
                section(
                    "Activity",
                    &[
                        ("↑↓ PgUp PgDn", "scroll · f follow tail · y copy selection"),
                        ("s", "stop · r restart · x close tab"),
                        (
                            "Enter / i",
                            "attach to an interactive program · Ctrl+] detaches",
                        ),
                        ("1–9", "show or hide a merged log stream"),
                        ("Esc", "back to Here · the activity keeps running"),
                    ],
                );
            }
            TabKind::Plan(_) => {
                section(
                    "Plan",
                    &[
                        ("↑↓", "select a step · Enter maximise its output · f follow"),
                        ("r", "retry the failed step · k skip an optional failure"),
                        ("Ctrl+C", "cancel the remaining work"),
                        ("Esc", "back to Here · the plan keeps running"),
                    ],
                );
            }
        }
        section(
            "Mouse",
            &[
                ("Click", "select"),
                ("Double-click", "run"),
                ("Right-click", "alternatives"),
                ("Wheel", "scroll under the pointer"),
            ],
        );
        sections.push(self.keys.help_section("Everywhere", |c| {
            Some(match c {
                HollaCmd::Activities => "Activities",
                HollaCmd::CloseTab => "Close tab",
                HollaCmd::Tab(0) => "Here",

                _ => return None,
            })
        }));
        if let Some(s) = sections.last_mut() {
            s.rows.push(("Alt+1–9".into(), "Show that tab".into()));
        }
        let h = HelpOverlay::from_keymaps(WidgetId::of("help"), &self.crumb(), sections)
            .title("Key reference");
        self.open_layer(Box::new(h), LayerTag::new("help"));
    }

    fn open_why(&mut self) {
        let Some(it) = self.current_item() else {
            self.set_status("Select a row first", Tone::Secondary);
            return;
        };
        let cwd = self.world.location.cwd_short();
        let rows = crate::domain::ranking::explain(&it, &cwd);
        let mut lines: Vec<TextLine> = rows.iter().map(|(k, v)| TextLine::kv(k, v)).collect();
        lines.push(TextLine::blank());
        lines.push(TextLine::text("Ranking order: alias › pin › live state › context › query › used here › used anywhere › default. Risk changes the gate, never the rank.", Tone::Muted));
        let m = TextModal::new(
            WidgetId::of("why"),
            &format!("Why is “{}” here?", it.label),
            lines,
        )
        .width(74);
        self.open_layer(Box::new(m), LayerTag::new("why"));
    }

    pub fn quit_stakes(&self) -> Stakes {
        let work = self.world.quit_work();
        if work.is_empty() {
            return Stakes::Clean;
        }
        let mut parts = vec![];
        if work.activities > 0 {
            parts.push(plural(work.activities, "activity", "activities"));
        }
        if work.plans > 0 {
            parts.push(plural(work.plans, "plan", "plans"));
        }
        if work.cleanup {
            parts.push("a cleanup".to_owned());
        }
        Stakes::Running {
            what: parts.join(" and "),
        }
    }

    fn request_quit(&mut self) {
        if self.quitting {
            self.set_status(
                "Already stopping · the shell leaves when every process is gone",
                Tone::Secondary,
            );
            return;
        }
        let stakes = shell::App::stakes(self);
        if let Err(d) = self
            .shell
            .quit_through(&mut self.leave, &stakes, &self.theme.strings)
        {
            if self.world.quit_work().is_empty() {
                self.open_layer(d, LayerTag::new("quit"));
            } else {
                self.open_quit_confirm();
            }
        }
    }

    /// Confirmed quit: stop everything running, deliver the completions it
    /// produced, then leave at once or wait for what cannot stop synchronously.
    /// True (with a status) while a confirmed quit is stopping work; launch,
    /// restart and plan-arm entry points return early on it.
    fn refuse_new_work(&mut self) -> bool {
        let refused = self.world.stopping();
        if refused {
            self.set_status(STOPPING_REFUSAL, Tone::Secondary);
        }
        refused
    }

    fn begin_quit(&mut self) {
        let stopping = self.world.quit_work().activities;
        let msgs = self.world.stop_work_for_quit();
        for m in msgs {
            self.dispatch_msg(m);
        }
        let left = self.world.quit_work();
        if left.is_empty() {
            self.shell.quit = true;
            return;
        }
        self.quitting = true;
        let text = if left.activities == 0 {
            "leaving when the cleanup settles · its report and log land first".to_string()
        } else {
            format!(
                "stopping {} · quitting when every process is gone",
                plural(stopping, "activity", "activities")
            )
        };
        self.set_status(&text, Tone::Secondary);
    }

    fn quit_wait_text(&self, left: QuitWork) -> String {
        if let Some(job) = &self.world.cleanup_job {
            format!(
                "waiting for the cleanup · {} of {} items · quitting when it settles",
                job.exec.completed(),
                job.exec.total()
            )
        } else {
            format!(
                "stopping {} · {} still cancelling · quitting when every process is gone",
                plural(left.activities, "activity", "activities"),
                self.world.cancelling()
            )
        }
    }

    fn open_quit_confirm(&mut self) {
        let work = self.world.quit_work();
        let cleanup = self
            .world
            .cleanup_job
            .as_ref()
            .map(|j| (j.exec.completed(), j.exec.total()));
        let running = work.activities + work.plans;
        let running_text = {
            let mut parts = vec![];
            if work.activities > 0 {
                parts.push(plural(work.activities, "activity", "activities"));
            }
            if work.plans > 0 {
                parts.push(plural(work.plans, "plan", "plans"));
            }
            format!(
                "{} {} running. Quitting stops {}.",
                parts.join(" and "),
                if running == 1 { "is" } else { "are" },
                if running == 1 { "it" } else { "them" },
            )
        };
        let body = match (running, cleanup) {
            (0, None) => "Nothing is running.".to_owned(),
            (_, None) => running_text,
            (0, Some((k, t))) => format!(
                "A cleanup is running ({k} of {t} items). Committed deletions cannot be stopped; leaving waits for the rest, then quits."
            ),
            (_, Some((k, t))) => format!(
                "{running_text} A cleanup is at {k} of {t} items; quitting waits for its report and log."
            ),
        };

        let title = if self.world.host.remote {
            format!("Quit holla❯ on {}?", self.world.host.name)
        } else {
            "Quit holla❯?".to_owned()
        };
        let body = if self.world.host.remote {
            format!(
                "◆ {} · {} · over SSH. {body}",
                self.world.host.name,
                self.world.host.role.label()
            )
        } else {
            body
        };
        let d = if running > 0 {
            Dialog::destructive(WidgetId::of("quit"), &title, &body, "Stop and quit")
                .cancel_label("Stay")
        } else if cleanup.is_some() {
            Dialog::destructive(WidgetId::of("quit"), &title, &body, "Leave when it settles")
                .cancel_label("Stay")
        } else {
            Dialog::confirm(WidgetId::of("quit"), &title, &body, "Quit")
        };
        self.open_layer(Box::new(d), LayerTag::new("quit"));
    }

    fn activity_picker_items(&self) -> Vec<option_list::Item> {
        let mut v = vec![];
        for a in &self.world.activities {
            let glyph = match a.state {
                ActivityState::Running | ActivityState::Queued => "⠋",
                ActivityState::Cancelling => "◐",
                ActivityState::Succeeded => "✓",
                ActivityState::Failed => "!",
                ActivityState::Stopped | ActivityState::Detached => "○",
            };
            let mut detail = format!(
                "{} · {} · {}",
                a.state.label(),
                a.scope.word,
                crate::screens::ticks_label(a.duration_ticks(self.world.tick))
            );
            if a.waiting.is_some() {
                detail = format!("{detail} · waiting for input");
            }
            v.push(option_list::Item {
                tag: (a
                    .waiting
                    .as_ref()
                    .map(|p| if p.secret { "password" } else { "input" }))
                .map(Into::into),
                ..option_list::Item::new(a.name.clone())
                    .detail(detail)
                    .glyph(glyph)
                    .group("activities")
                    .key(ItemKey::text(&a.id))
            });
        }
        for p in &self.world.plans {
            if p.phase != PlanPhase::Review {
                let (ok, failed, ..) = p.outcome();
                v.push(
                    option_list::Item::new(p.title.clone())
                        .detail(format!(
                            "plan · {} · {ok} done · {failed} failed",
                            if p.phase == PlanPhase::Done {
                                "finished"
                            } else {
                                "running"
                            }
                        ))
                        .glyph(if p.phase == PlanPhase::Done {
                            if failed > 0 { "!" } else { "✓" }
                        } else {
                            "⠋"
                        })
                        .group("plans")
                        .key(plan_key(&p.id)),
                );
            }
        }
        v
    }

    fn open_activities(&mut self) {
        let items = self.activity_picker_items();
        let mut p = Picker::new(WidgetId::of("activities"), "Activities").choose_hint("Switch");
        p.set_searchable(false)
            .expect("disabling picker search is valid");
        p.set_width(70);
        p.empty_text = "Nothing has been started yet".into();
        p.scope = Some(format!("{} live", self.world.live_activities()));
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        self.open_layer(Box::new(p), LayerTag::new("activities"));
    }

    fn open_layer(&mut self, layer: Box<dyn Layer<ModalResult>>, tag: LayerTag) {
        self.modals
            .open(layer, tag, self.active(), &mut self.shell.focus);

        if let Some(d) = self.modals.top_as_mut::<Dialog>() {
            d.chrome = (1, 2);
        }
        self.shell.pointer.hover = None;
        self.shell.pointer.pressed = None;
    }

    fn layer_cx(&mut self) -> (Option<&mut LayerSlot>, LayerCx<'_>) {
        (
            self.modals.top_mut(),
            LayerCx {
                focus: &mut self.shell.focus,
                ring: &self.shell.ring,
                app: &(),
            },
        )
    }

    fn settle_layer(&mut self, out: Outcome) -> Outcome {
        Layers::settle(
            self,
            |app| (&mut app.modals, &mut app.shell.focus),
            out,
            |app, taken| match taken {
                Taken::Event(r) => app.on_layer_event(r),
                Taken::Closed(entry, r) => app.on_layer_result(entry, r),
            },
        )
    }

    fn dismiss_layer(&mut self, reason: DismissReason) -> Outcome {
        let dismiss = self.top_dismiss();
        let d = self.modals.dismiss(reason, &mut self.shell.focus);
        self.dismissed(reason, dismiss, d)
    }

    fn top_dismiss(&self) -> junie_tui::ui::popup::Dismiss {
        self.modals
            .top()
            .map(|e| e.layer.spec().dismiss)
            .unwrap_or_default()
    }

    fn dismissed(
        &mut self,
        reason: DismissReason,
        dismiss: junie_tui::ui::popup::Dismiss,
        d: Dismissed<ModalResult, (), usize>,
    ) -> Outcome {
        match d {
            Dismissed::None => Outcome::Ignored,
            Dismissed::Refused => {
                let now = self.now_ms();
                if reason == DismissReason::OutsideClick
                    && junie_tui::ui::modal::refuse_outside_click(
                        &mut self.shell.pointer,
                        dismiss,
                        now,
                    )
                {
                    return Outcome::Changed;
                }
                Outcome::Consumed
            }
            Dismissed::Closed(entry, r) => {
                self.on_layer_result(entry, r.unwrap_or(ModalResult::Cancelled))
            }
        }
    }

    fn on_layer_event(&mut self, result: ModalResult) -> Outcome {
        let Some((tag, owner)) = self.modals.top().map(|e| (e.tag.clone(), e.owner)) else {
            return Outcome::Ignored;
        };
        if tag.kind == "files-jump" {
            return match result {
                ModalResult::PickerEvent(ev) => self.files_jump(ev, None),
                _ => Outcome::Consumed,
            };
        }
        self.report_to_owner(owner, &tag, result)
    }

    fn on_layer_result(&mut self, entry: LayerSlot, result: ModalResult) -> Outcome {
        let tag = entry.tag.clone();
        match tag.kind {
            "quit" => {
                if let Resolved::Discard(Leave::Quit) = self.leave.resolve(answer(&result)) {
                    self.begin_quit();
                }
                Outcome::Changed
            }
            LEAVE_TAG => {
                if let Resolved::Discard(Leave::To(nav)) = self.leave.resolve(answer(&result)) {
                    self.apply_nav(nav);
                    self.enter_top();
                }
                Outcome::Changed
            }
            "confirm-one" => {
                if let ModalResult::Dialog {
                    action: Some(1), ..
                } = result
                    && let Some((item, args)) = self.pending_confirm.take()
                {
                    self.execute_item(&item, args);
                } else {
                    self.pending_confirm = None;
                    self.set_status("Cancelled · nothing was executed", Tone::Secondary);
                }
                Outcome::Changed
            }
            "gate2" => {
                if let ModalResult::Dialog {
                    action: Some(1), ..
                } = result
                {
                    self.gate2_confirmed();
                } else {
                    self.set_status("Cancelled · nothing was executed", Tone::Secondary);
                }
                Outcome::Changed
            }
            "alias" => {
                if let ModalResult::Dialog {
                    action: Some(1),
                    text: Some(t),
                } = result
                {
                    let alias = t.trim().to_owned();
                    if alias.is_empty() {
                        self.set_status("No alias set: the field was empty", Tone::Secondary);
                    } else {
                        self.world
                            .edit_memory(|w| w.memory.set_alias(&alias, &tag.key));
                        self.set_status(
                            &format!("Alias {alias} set · it beats learned ranking from now on"),
                            Tone::Secondary,
                        );
                    }
                }
                Outcome::Changed
            }
            "activities" => {
                if let ModalResult::Picked(i) = result {
                    let row = entry
                        .get::<Picker>()
                        .and_then(|p| p.visible_item(i).map(|v| v.item))
                        .and_then(|it| it.key.map(|key| (key, it.group == "plans")));
                    match row {
                        Some((k, true)) => {
                            let plans = self.world.plans.iter();
                            match plans.map(|p| p.id.clone()).find(|id| plan_key(id) == k) {
                                Some(pid) => self.open_plan_tab(&pid),
                                None => self.set_status("That plan is gone", Tone::Secondary),
                            }
                        }
                        Some((k, false)) => {
                            let all = self.world.activities.iter();
                            match all.map(|a| a.id.clone()).find(|id| ItemKey::text(id) == k) {
                                Some(id) => self.open_activity_tab(&id, true),
                                None => self.set_status("That activity is gone", Tone::Secondary),
                            }
                        }
                        None => self.set_status("That activity is gone", Tone::Secondary),
                    }
                }
                Outcome::Changed
            }
            "alternatives" => {
                if let ModalResult::MenuChosen(i) = result {
                    let action = self.menu_actions.get(i).cloned();
                    if let Some(a) = action {
                        self.menu_action(a, &tag.key);
                    }
                }
                Outcome::Changed
            }
            "close-tab" => {
                if let ModalResult::Dialog {
                    action: Some(1), ..
                } = result
                {
                    self.stop_and_close_activity(&tag.key);
                }
                Outcome::Changed
            }
            "close-plan-tab" => {
                if let ModalResult::Dialog {
                    action: Some(1), ..
                } = result
                {
                    self.stop_and_close_plan(&tag.key);
                }
                Outcome::Changed
            }
            "files-jump" => match result {
                ModalResult::Picked(i) => self.files_jump(PickerAction::Chosen(i), Some(entry)),
                _ => Outcome::Changed,
            },
            _ => self.report_to_owner(entry.owner, &tag, result),
        }
    }

    fn report_to_owner(&mut self, owner: usize, tag: &LayerTag, result: ModalResult) -> Outcome {
        let (o, reqs) = self.deliver_tab(owner, true, Outcome::Changed, |s, w, cx| {
            s.on_layer_result(tag, result, w, cx)
        });
        o.or(self.apply_requests(reqs, owner)).or(Outcome::Changed)
    }

    fn layer_key(&mut self, key: Key) -> Outcome {
        let (Some(top), mut cx) = self.layer_cx() else {
            return Outcome::Ignored;
        };
        let out = top.layer.on_key(&key, &mut cx);
        self.settle_layer(out)
    }

    fn files_jump_rows(&mut self, error: Option<String>) {
        let active = self.active();
        let Some(query) = self.modals.top_as::<Picker>().map(|p| p.query().to_owned()) else {
            return;
        };
        let rows = self
            .router
            .top_of_mut(active)
            .and_then(|s| s.as_files())
            .map(|f| {
                f.set_jump_error(error);
                f.jump_rows(&query, &self.world)
            });
        if let Some((items, status)) = rows
            && let Some(p) = self.modals.top_as_mut::<Picker>()
        {
            p.replace_items(items)
                .unwrap_or_else(|error| p.report_item_error(error));
            p.status = status;
        }
    }

    fn files_jump(&mut self, ev: PickerAction, chosen: Option<LayerSlot>) -> Outcome {
        let active = self.active();
        match ev {
            PickerAction::QueryChanged | PickerAction::Back => {
                self.files_jump_rows(None);
                Outcome::Changed
            }
            PickerAction::Chosen(_) | PickerAction::ChosenAlt(_) | PickerAction::Submit => {
                let picker = match &chosen {
                    Some(entry) => entry.get::<Picker>(),
                    None => self.modals.top_as::<Picker>(),
                };
                let Some(query) = picker.map(|p| p.query().to_owned()) else {
                    return Outcome::Consumed;
                };

                let selected = match ev {
                    PickerAction::Submit => None,
                    _ => picker
                        .and_then(|p| p.current())
                        .map(|it| it.insertion().to_owned()),
                };
                let mut cx = self.shell.cx();
                let outcome = self
                    .router
                    .top_of_mut(active)
                    .and_then(|s| s.as_files())
                    .map(
                        |f| match f.jump_target(&query, selected.as_deref(), &self.world) {
                            None => Err("Type a path or choose a suggestion".to_owned()),
                            Some(target) => f
                                .perform_jump(&target, &self.world, &mut cx)
                                .map(|_| target),
                        },
                    );
                let requests = cx.take();
                match outcome {
                    Some(Ok(target)) => {
                        if chosen.is_none() {
                            let _ = self
                                .modals
                                .dismiss(DismissReason::Programmatic, &mut self.shell.focus);
                        }
                        let short = self.world.location.short(&target);
                        self.set_status(&format!("Jumped to {short}"), Tone::Secondary);
                        self.apply_requests(requests, active);
                    }
                    Some(Err(e)) => {
                        if let Some(entry) = chosen {
                            self.modals.open(
                                entry.layer,
                                entry.tag,
                                entry.owner,
                                &mut self.shell.focus,
                            );
                        }
                        self.files_jump_rows(Some(e));
                    }
                    None => {
                        if chosen.is_none() {
                            let _ = self
                                .modals
                                .dismiss(DismissReason::Programmatic, &mut self.shell.focus);
                        }
                    }
                }
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    fn deliver_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        if ev.on(HintBar::ID) {
            return match self.shell.footer_keys.clicked(ev) {
                Some(key) => shell::KeyRouter::replay(self, &key).or(Outcome::Changed),
                None => Outcome::Consumed,
            };
        }
        if !self.modals.is_empty() {
            return self.layer_pointer(ev);
        }
        if let Some(o) = self.chrome_pointer(ev) {
            return o;
        }
        let ev = *ev;
        self.with_top(|s, w, cx| s.pointer(&ev, w, cx))
    }

    fn layer_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        if ev.is(Phase::Click) && ev.id == junie_tui::ui::modal::CLOSE {
            return self.dismiss_layer(DismissReason::CloseButton);
        }
        let dismiss = self.top_dismiss();
        match self
            .modals
            .pointer(ev, &mut self.shell.focus, &self.shell.ring, &())
        {
            LayerPointer::Handled(out) => self.settle_layer(out),
            LayerPointer::Dismissed(d) => self.dismissed(DismissReason::OutsideClick, dismiss, d),
        }
    }

    fn chrome_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        if let Some(r) = self.crumbs.route(ev) {
            let (mut out, levels) = r.into_parts();
            for _ in levels {
                if let Some(nav) = self.router.crumb_click(&self.crumbs, 0, ev.id) {
                    if self.navigate(nav) {
                        self.enter_top();
                    }
                    out = Outcome::Changed;
                }
            }
            return Some(out);
        }
        if let Some(r) = self.menu.route(ev) {
            let (mut out, actions) = r.into_parts();
            for act in actions {
                out = match act {
                    act @ MenuBarAction::Chosen(..) => match self.menus.command(act) {
                        Some(cmd) => self.run_cmd(cmd),
                        None => out,
                    },
                    MenuBarAction::Brand => {
                        self.open_about();
                        Outcome::Changed
                    }
                    _ => out,
                };
            }
            return Some(out);
        }

        if let Some(r) = self.strip.route(ev) {
            let (mut out, actions) = r.into_parts();
            for act in actions {
                out = match act {
                    TabAction::Activated(i) => {
                        self.activate_tab(i);
                        Outcome::Changed
                    }
                    TabAction::Close(i) => {
                        self.router.select(i);
                        self.go(Go::CloseTab);
                        Outcome::Changed
                    }
                    _ => out,
                };
            }
            return Some(out);
        }
        if !ev.is(Phase::Click) {
            return None;
        }

        if ev.id == STATUS_RUNNING {
            self.open_activities();
            return Some(Outcome::Changed);
        }
        if ev.id == STATUS_PATH {
            let p = self.world.location.cwd.clone();
            self.copy(p);
            return Some(Outcome::Changed);
        }
        if ev.id == STATUS_GIT {
            self.go(Go::Run {
                item: "git.status".into(),
                args: vec![],
            });
            return Some(Outcome::Changed);
        }
        None
    }

    fn apply_requests(&mut self, requests: Vec<Request>, _owner: usize) -> Outcome {
        let mut out = Outcome::Ignored;
        for r in requests {
            out = Outcome::Changed;
            match r {
                Request::Status(s, tone) => self.set_status(&s, tone),
                Request::StatusAs(s, kind) => self.set_status(&s, kind.tone()),
                Request::Sticky(s) => {
                    let now = self.world.clock.now();
                    self.shell.status.sticky_at(s, now);
                }
                Request::Progress(s, job) => {
                    let now = self.world.clock.now();
                    self.shell.status.progress_at(job, s, now);
                }
                Request::Open(layer, tag) => self.open_layer(layer, tag),
                Request::Go(g) => self.go(g),
                Request::Back => self.go(Go::Pop),
                Request::Copy(s, _) => self.copy(s),

                Request::Close
                | Request::Top(_)
                | Request::Help
                | Request::RefusedOutsideClick(_) => {}
            }
        }
        out
    }

    fn copy(&mut self, s: String) {
        let shown = truncate(&s.replace('\n', " · "), 48);
        self.world.clipboard = Some(s);
        self.clipboard_gen += 1;
        self.set_status(&format!("Copied {shown}"), Tone::Secondary);
    }

    pub fn go(&mut self, g: Go) {
        match g {
            Go::Run { item, args } => {
                self.last_query = self
                    .finder_mut()
                    .map(|f| f.query().to_owned())
                    .filter(|q| !q.trim().is_empty());
                self.run_item(&item, args)
            }
            Go::Alternatives { item, anchor } => self.open_alternatives(&item, anchor),
            Go::Push(page) => {
                self.activate_tab(0);
                self.push_page(page);
            }
            Go::Pop => self.pop_page(),
            Go::Here => self.activate_tab(0),
            Go::Scope(s) => {
                self.activate_tab(0);

                if !self.navigate(shell::Nav::PopTo(0)) {
                    return;
                }
                if let Some(f) = self.finder_mut() {
                    f.set_scope(s);
                }
                self.enter_top();
                let loc = &self.world.location;
                let word = match s {
                    Scope::Here => format!("here · {}", loc.cwd_short()),
                    Scope::Parent => match &loc.workspace {
                        Some(p) => format!("parent · {}", loc.short(&p.root)),
                        None => "parent · no root above".to_owned(),
                    },
                    Scope::Children => format!(
                        "children · {}",
                        junie_tui::ui::text::plural(loc.children.len(), "project", "projects")
                    ),
                    Scope::System => format!("system · {}", self.world.host.name),
                };
                self.set_status(&format!("Scope {word}"), Tone::Secondary);
            }
            Go::ConfirmPlan(id) => self.confirm_plan(&id),
            Go::Gate1Accepted(target) => self.open_gate2(target),
            Go::Trusted { config, then } => {
                let custom = self
                    .world
                    .custom_project
                    .as_ref()
                    .filter(|c| c.path == config)
                    .cloned();
                if let Some(cfg) = custom {
                    let now_text = match self.world.fs.get(&cfg.path).map(|n| &n.content) {
                        Some(crate::sim::fs::Content::Text(t)) => t.clone(),
                        _ => String::new(),
                    };
                    let now_digest = crate::domain::digest::sha256_hex(now_text.as_bytes());
                    if now_digest != cfg.digest {
                        self.pop_page();
                        self.reload_project_config();
                        self.set_status(
                            "The configuration changed during review · nothing was trusted or run · review again",
                            Tone::Error,
                        );
                        return;
                    }
                    let cwd = self
                        .find_item(&then)
                        .map(|i| i.scope.runs_in.clone())
                        .unwrap_or(self.world.location.cwd.clone());
                    match self
                        .world
                        .edit_catalog(|w| w.trust.approve(&cfg.digest, &cfg.path, &cwd))
                    {
                        Ok(text) => {
                            self.world.persisted.trust = Some(text);
                            self.pop_page();
                            self.set_status(
                                &format!(
                                    "Trusted {} · this exact content at this path",
                                    self.world.location.short(&config)
                                ),
                                Tone::Secondary,
                            );
                            self.run_item(&then, vec![]);
                        }
                        Err(e) => {
                            self.pop_page();
                            self.set_status(&format!("{e} · nothing was run"), Tone::Error);
                        }
                    }
                    return;
                }
                if !self.world.trusted_now.contains(&config) {
                    self.world
                        .edit_catalog(|w| w.trusted_now.push(config.clone()));
                }
                self.pop_page();
                self.set_status(
                    &format!(
                        "Trusted {} · re-resolving the task",
                        self.world.location.short(&config)
                    ),
                    Tone::Secondary,
                );
                self.run_item(&then, vec![]);
            }
            Go::CleanupPlan(selected) => {
                let plan = crate::domain::fixtures::plan_cleanup_work(&self.world, &selected);
                self.world.plans.retain(|p| p.id != "cleanup-work");
                self.world.plans.push(plan);
                self.push_page(Page::PlanReview {
                    plan: "cleanup-work".into(),
                });
            }
            Go::Restart(id) => self.run_derived(&format!("activity.{id}.restart"), ""),
            Go::CloseTab => self.request_close_tab(),
            Go::Reload => {
                self.reload_project_config();
                self.enter_top();
            }
            Go::Exec { label, argv } => {
                if self.refuse_new_work() {
                    return;
                }
                let scope = crate::domain::context::ScopeTag::here(&self.world.location.cwd);
                let script = self.world.script_for(None, &argv);
                let aid = self.world.start_activity(
                    argv,
                    script,
                    &label,
                    &label,
                    ActivityKind::Task,
                    scope,
                );
                self.open_activity_tab(&aid, true);
            }
            Go::Osc52(value) => self.copy_osc52(value),
            Go::Analyze(path) => {
                self.activate_tab(0);
                if !self.world.fs.exists(&path) {
                    self.set_status(&format!("{path}: no such file or directory"), Tone::Error);
                } else {
                    self.world.start_scan(&path);
                    self.push_page(Page::Disk { path });
                }
            }
        }
    }

    fn page_screen(world: &mut World, page: Page) -> Box<dyn Screen> {
        match page {
            Page::Finder { group } => Box::new(FinderPage::new(group)),
            Page::Disk { path } => Box::new(DiskPage::new(&path)),
            Page::PlanReview { plan } => {
                ensure_plan(world, &plan);
                Box::new(PlanReviewPage::new(&plan))
            }
            Page::Review(target) => Box::new(GatePage::new(target, world)),
            Page::Trust { config, then } => Box::new(TrustPage::new(&config, &then)),
            Page::Args { item } => Box::new(ArgsPage::new(&item, world)),
            Page::Snapshot { kind } => Box::new(SnapshotPage::new(&kind, world)),
            Page::Files { path, query } => Box::new(FilesPage::new(&path, query.as_deref(), world)),
            Page::Find => Box::new(FilesPage::find(world)),
            Page::DiskOverview => Box::new(DiskPage::overview()),
            Page::TopFiles => Box::new(DiskPage::top_files()),
            Page::Cleanup { category } => Box::new(CleanupPage::new(category.as_deref())),
            Page::CleanupGate { plan } => Box::new(GatePage::new(GateTarget::Cleanup(plan), world)),
            Page::Report { index } => {
                Box::new(SnapshotPage::new(&format!("report:{index}"), world))
            }
            Page::Config { path } => Box::new(SnapshotPage::new(&format!("config:{path}"), world)),
        }
    }

    fn reload_project_config(&mut self) {
        let Some(cfg) = self.world.custom_project.clone() else {
            return;
        };
        let text = match self.world.fs.get(&cfg.path).map(|n| &n.content) {
            Some(crate::sim::fs::Content::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let builtin = crate::domain::catalog::builtin_ids(&self.world);
        let builtin_refs: Vec<&str> = builtin.iter().map(String::as_str).collect();
        let reserved: Vec<String> = self
            .world
            .custom_global
            .as_ref()
            .map(|g| g.actions.iter().map(|a| a.id.clone()).collect())
            .unwrap_or_default();
        let reloaded = crate::domain::custom::parse_config(
            &cfg.path,
            cfg.origin,
            &text,
            &builtin_refs,
            &reserved,
        );
        self.world
            .edit_catalog(|w| w.custom_project = Some(reloaded));
    }

    pub(crate) fn push_page(&mut self, page: Page) {
        self.router.select(0);
        self.navigate(shell::Nav::Push(page));
        self.enter_top();
    }

    fn navigate(&mut self, nav: shell::Nav<Page>) -> bool {
        let n = self.router.dropped_by(&nav).count();
        let mut stakes = Stakes::Clean;
        for p in self.router.stack_mut().rev().take(n) {
            stakes = stakes.max(shell::Page::<Holla>::stakes(p.as_mut(), &self.world));
        }
        match self
            .router
            .request(&mut self.leave, nav, || stakes, &self.theme.strings)
        {
            Ok(nav) => {
                self.apply_nav(nav);
                true
            }
            Err(d) => {
                self.open_layer(d, LayerTag::new(LEAVE_TAG));
                false
            }
        }
    }

    fn apply_nav(&mut self, nav: shell::Nav<Page>) {
        let world = &mut self.world;
        self.router
            .apply(nav, |page| Self::page_screen(world, page));
    }

    fn leave_review_pages(&mut self) -> bool {
        self.router.select(0);
        self.navigate(shell::Nav::PopTo(0))
    }

    fn pop_page(&mut self) {
        if self.active() == 0 && self.router.depth() > 1 {
            if self.navigate(shell::Nav::Back) {
                self.enter_top();
            }
        } else if self.active() != 0 {
            self.activate_tab(0);
        }
    }

    fn activate_tab(&mut self, i: usize) {
        if i >= self.router.tab_count() {
            return;
        }
        self.router.select(i);
        self.strip.set_active(i);
        self.enter_top();
    }

    pub(crate) fn add_tab(&mut self, kind: TabKind, root: Box<dyn Screen>) -> usize {
        let shown = self.active();
        let i = self.router.open_tab(root);
        self.router.select(shown);
        self.tabs.push(TabEntry {
            kind,
            pending: vec![],
        });
        i
    }

    fn open_activity_tab(&mut self, id: &str, activate: bool) {
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| t.kind == TabKind::Activity(id.to_owned()))
        {
            if activate {
                self.activate_tab(i);
            }
            return;
        }
        let i = self.add_tab(
            TabKind::Activity(id.to_owned()),
            Box::new(ActivityTab::new(id)),
        );
        if activate {
            self.activate_tab(i);
        }
    }

    fn open_plan_tab(&mut self, id: &str) {
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| t.kind == TabKind::Plan(id.to_owned()))
        {
            self.activate_tab(i);
            return;
        }
        let i = self.add_tab(TabKind::Plan(id.to_owned()), Box::new(PlanTab::new(id)));
        self.activate_tab(i);
    }

    fn request_close_tab(&mut self) {
        let active = self.active();
        if active == 0 {
            self.set_status("Here cannot be closed · Ctrl+Q quits", Tone::Secondary);
            return;
        }
        let live = match &self.tabs[active].kind {
            TabKind::Activity(id) => self.world.activity(id).is_some_and(|a| a.state.live()),
            TabKind::Plan(id) => self
                .world
                .plan(id)
                .is_some_and(|p| p.phase == PlanPhase::Running),
            TabKind::Here => false,
        };
        if live {
            let name = self.tab_label(active);
            let d = Dialog::destructive(
                WidgetId::of("close-tab"),
                &format!("Stop {name}?"),
                "Closing the tab stops the work it holds. Detach instead to keep a monitor running.",
                "Stop and close",
            )
            .cancel_label("Keep running");
            let tag = match &self.tabs[active].kind {
                TabKind::Activity(id) => LayerTag::new("close-tab").key(id.clone()),
                TabKind::Plan(id) => LayerTag::new("close-plan-tab").key(id.clone()),
                TabKind::Here => LayerTag::new("close-tab"),
            };
            self.open_layer(Box::new(d), tag);
        } else {
            self.close_tab(active, false);
        }
    }

    /// Stops a plan through the world and delivers its completion messages
    /// before any presentation changes.
    fn stop_plan(&mut self, id: &str) {
        for m in self.world.stop_plan(id) {
            self.dispatch_msg(m);
        }
    }

    /// Stop-and-close for a plan identified when the dialog opened, so a tab
    /// selection change cannot redirect it to another plan.
    fn stop_and_close_plan(&mut self, id: &str) {
        let tab = self
            .tabs
            .iter()
            .position(|t| matches!(&t.kind, TabKind::Plan(p) if p == id));
        match tab {
            Some(i) => self.close_tab(i, true),
            None => self.stop_plan(id),
        }
    }

    /// Stop-and-close for an activity identified when the dialog opened. A
    /// live process keeps its tab and a recorded close request until its
    /// terminal event arrives, so switching tabs cannot redirect the close.
    fn stop_and_close_activity(&mut self, id: &str) {
        let tick = self.world.tick;
        if self.world.activity(id).is_some_and(|a| a.state.live()) {
            self.pending_activity_closes.insert(id.to_owned());
            self.world.edit_activity(id, |a| a.stop(tick));
            if !self.close_activity_tab_if_finished(id) {
                self.set_status(
                    "Stopping · the tab closes once the process is gone",
                    Tone::Secondary,
                );
            }
        } else {
            self.close_activity_tab_if_finished(id);
        }
    }

    /// Fulfils a recorded close request for `id` once the activity is no
    /// longer live. Returns whether a tab was removed. The request is retired
    /// whenever it can no longer be fulfilled, so it never outlives its tab.
    fn close_activity_tab_if_finished(&mut self, id: &str) -> bool {
        let tab = self
            .tabs
            .iter()
            .position(|t| t.kind == TabKind::Activity(id.to_owned()));
        let Some(tab) = tab else {
            self.pending_activity_closes.remove(id);
            return false;
        };
        match self.world.activity(id) {
            Some(a) if a.state.live() => return false,
            Some(_) => self.close_tab(tab, false),
            None => {
                self.remove_presentation_tab(tab);
                self.set_status("That activity is gone", Tone::Secondary);
            }
        }
        self.pending_activity_closes.remove(id);
        true
    }

    fn close_tab(&mut self, i: usize, stop: bool) {
        if i == 0 || i >= self.router.tab_count() {
            return;
        }
        match self.tabs[i].kind.clone() {
            TabKind::Activity(id) => {
                if self.world.activity(&id).is_some_and(|a| a.state.live()) {
                    return;
                }
                // The record outlives its tab: batch tallies count members by
                // ID, so only the tab is removed below.
            }
            TabKind::Plan(id) => {
                if stop {
                    self.stop_plan(&id);
                }
            }
            TabKind::Here => {}
        }
        self.remove_presentation_tab(i);
    }

    /// Removes tab `i` from the strip, router and modal owners together. The
    /// shown tab stays shown when a background tab closes, and a closed shown
    /// tab hands over to its predecessor.
    fn remove_presentation_tab(&mut self, i: usize) {
        let was_shown = i == self.active();
        self.tabs.remove(i);
        self.router.close_tab(i);
        for entry in self.modals.iter_mut() {
            if entry.owner > i || (entry.owner == i && i > 0) {
                entry.owner -= 1;
            }
        }
        if was_shown {
            self.activate_tab(i.saturating_sub(1).min(self.tabs.len() - 1));
        } else {
            self.strip.set_active(self.active());
        }
    }

    fn find_item(&self, id: &str) -> Option<Item> {
        self.world.items().into_iter().find(|i| i.id == id)
    }

    fn run_item(&mut self, id: &str, args: Vec<(String, String)>) {
        let Some(it) = self.find_item(id) else {
            self.set_status(&format!("{id} is no longer available here"), Tone::Error);
            return;
        };
        if let crate::domain::action::Freshness::Unavailable(why) = &it.freshness
            && !matches!(
                it.launch,
                Launch::Snapshot { .. }
                    | Launch::Files { .. }
                    | Launch::Insert
                    | Launch::Config { .. }
            )
        {
            self.set_status(&format!("{} is unavailable · {why}", it.label), Tone::Error);
            return;
        }

        if let Confirmation::Trust { .. } = &it.confirmation {
            let config = self.config_path_for(&it);
            let trusted = match &it.trust_key {
                Some((path, digest)) => {
                    let cwd = it.scope.runs_in.clone();
                    self.world.trust.status(digest, path, &cwd)
                        == crate::domain::custom::TrustStatus::Trusted
                }
                None => self.world.trusted_now.contains(&config),
            };
            if !trusted {
                self.activate_tab(0);
                self.push_page(Page::Trust {
                    config,
                    then: it.id.clone(),
                });
                return;
            }
        }

        if !it.args.is_empty() && args.is_empty() {
            self.activate_tab(0);
            self.push_page(Page::Args {
                item: it.id.clone(),
            });
            return;
        }

        if let Launch::Plan { plan } = &it.launch {
            self.remember_use(&it.id);
            self.activate_tab(0);
            self.push_page(Page::PlanReview { plan: plan.clone() });
            return;
        }
        match &it.confirmation {
            Confirmation::None | Confirmation::Trust { .. } => self.execute_item(&it.id, args),

            Confirmation::One
                if matches!(
                    it.launch,
                    Launch::Cleanup { .. } | Launch::Disk { .. } | Launch::TopFiles | Launch::Find
                ) =>
            {
                self.execute_item(&it.id, args)
            }
            Confirmation::One => self.open_confirm_one(&it, args),
            Confirmation::TwoGate { .. } => {
                self.activate_tab(0);
                self.push_page(Page::Review(GateTarget::Item {
                    item: it.id.clone(),
                    args,
                }));
            }
        }
    }

    fn remember_use(&mut self, id: &str) {
        self.world.record_use(id);
        if let Some(q) = self.last_query.take()
            && !q.trim().is_empty()
        {
            let host = self.world.host.name.clone();
            let now = self.world.now_secs();
            self.world
                .edit_memory(|w| w.memory.usage.learn_query(&q, id, &host, now));
        }
        self.persist_usage();
    }

    fn persist_usage(&mut self) {
        let now = self.world.now_secs();
        let on_disk = self.world.persisted.frecency.clone();
        let saved = self
            .world
            .edit_memory(|w| w.memory.usage.save(on_disk.as_deref(), now));
        match saved {
            Ok(text) => self.world.persisted.frecency = Some(text),
            Err(e) if self.world.memory.usage.enabled => {
                self.set_status(&format!("history not saved · {e}"), Tone::Error);
            }
            Err(_) => {}
        }
    }

    fn config_path_for(&self, it: &Item) -> String {
        if let Some((path, _)) = &it.trust_key {
            return path.clone();
        }
        self.world
            .mise
            .configs
            .iter()
            .find(|c| {
                c.tasks
                    .iter()
                    .any(|t| format!("mise.task.{}", t.namespaced) == it.id)
            })
            .map(|c| c.path.clone())
            .unwrap_or_else(|| format!("{}/mise.toml", it.scope.defined_at))
    }

    fn open_confirm_one(&mut self, it: &Item, args: Vec<(String, String)>) {
        let mut facts = crate::screens::finder::item_facts(it, &self.world, &self.theme);
        if !args.is_empty() {
            let a: Vec<String> = args.iter().map(|(k, v)| format!("{k} = {v}")).collect();
            facts.push(Prop::new("Arguments", a.join(" · ")));
        }
        facts.push(
            Prop::new("Confirmation", "one explicit confirmation · not remembered")
                .tone(Tone::Muted),
        );
        let code = it.commands.clone();
        let dangerous = matches!(it.risk, Risk::Destructive | Risk::Privileged);
        let confirm = if dangerous {
            Button::danger(WidgetId::of("confirm-one").sub("ok"), "Run")
        } else {
            Button::primary(WidgetId::of("confirm-one").sub("ok"), "Run")
        };
        let mut d = Dialog::facts(
            WidgetId::of("confirm-one"),
            &it.label,
            facts,
            code,
            None,
            confirm,
        );
        d.initial_focus = if dangerous {
            d.actions[0].id
        } else {
            d.actions[1].id
        };
        d.width = 74;
        self.pending_confirm = Some((it.id.clone(), args));
        self.open_layer(Box::new(d), LayerTag::new("confirm-one"));
    }

    fn confirm_plan(&mut self, id: &str) {
        let Some(p) = self.world.plan(id).cloned() else {
            return;
        };
        if p.included_count() == 0 {
            self.set_status("Nothing to run: every step is excluded", Tone::Secondary);
            return;
        }
        if p.phrase.is_some() {
            self.push_page(Page::Review(GateTarget::Plan(id.to_owned())));
        } else {
            let mut facts: Vec<Prop> = vec![
                Prop::new("Plan", format!("{} · {}", p.title, p.intent)).wrap(),
                Prop::new("Host", p.host.clone()).tone(
                    if p.host == self.world.host.name && self.world.host.role.sensitive() {
                        Tone::Warning
                    } else {
                        Tone::Normal
                    },
                ),
                Prop::new(
                    "Steps",
                    format!(
                        "{} included · {} excluded · ~{}",
                        p.included_count(),
                        p.steps.len() - p.included_count(),
                        crate::screens::ticks_label(p.estimate_ticks())
                    ),
                ),
            ];
            for (k, v) in p.facts.iter().filter(|(k, _)| k != "Host") {
                facts.push(Prop::new(k, v.clone()).wrap());
            }
            let code: Vec<String> = p
                .steps
                .iter()
                .filter(|s| s.included)
                .flat_map(|s| s.commands.clone())
                .collect();

            let confirm = Button::primary(WidgetId::of("start-plan").sub("ok"), "Start plan");
            let mut d = Dialog::facts(
                WidgetId::of("start-plan"),
                &format!("Start {}?", p.title.to_lowercase()),
                facts,
                code,
                None,
                confirm,
            );
            d.initial_focus = d.actions[0].id;
            d.width = 76;
            self.pending_gate = Some(GateTarget::Plan(id.to_owned()));
            self.open_layer(Box::new(d), LayerTag::new("gate2"));
        }
    }

    fn open_gate2(&mut self, target: GateTarget) {
        let (title, facts, code, phrase, confirm_label) = match &target {
            GateTarget::Plan(id) => {
                let Some(p) = self.world.plan(id).cloned() else {
                    return;
                };
                let mut facts = vec![
                    Prop::new("Action", p.intent.clone()).wrap(),
                    Prop::new("Host", p.host.clone()).tone(Tone::Normal),
                    Prop::new("Steps", format!("{} included", p.included_count())),
                ];
                for (k, v) in p.facts.iter().take(3) {
                    facts.push(Prop::new(k, v.clone()).wrap());
                }
                let code: Vec<String> = p
                    .steps
                    .iter()
                    .filter(|s| s.included)
                    .flat_map(|s| s.commands.clone())
                    .collect();
                (
                    p.title.clone(),
                    facts,
                    code,
                    p.gate_phrase().unwrap_or_default(),
                    "Execute".to_owned(),
                )
            }
            GateTarget::Item { item, .. } => {
                let Some(it) = self.find_item(item) else {
                    return;
                };
                let facts = crate::screens::finder::item_facts(&it, &self.world, &self.theme);
                (
                    it.label.clone(),
                    facts,
                    it.commands.clone(),
                    it.confirmation.phrase().unwrap_or_default(),
                    "Execute".to_owned(),
                )
            }
            GateTarget::Cleanup(plan) => {
                let facts = crate::screens::cleanup::plan_facts(plan, &self.world);
                let code: Vec<String> = plan.items.iter().map(|i| i.path.clone()).collect();
                (
                    format!(
                        "{} {} items",
                        if plan.dry_run {
                            "Dry run"
                        } else if plan.mode == crate::domain::cleanup::Mode::Permanent {
                            "Permanently delete"
                        } else {
                            "Trash"
                        },
                        plan.items.len()
                    ),
                    facts,
                    code,
                    plan.phrase(),
                    if plan.dry_run {
                        "Dry run".to_owned()
                    } else {
                        "Execute".to_owned()
                    },
                )
            }
        };
        let confirm = Button::danger(WidgetId::of("gate2").sub("ok"), &confirm_label);
        let mut d = Dialog::facts(
            WidgetId::of("gate2"),
            &format!("{title} · gate 2 of 2"),
            facts,
            code,
            Some(&phrase),
            confirm,
        );
        d.width = 78;
        self.pending_gate = Some(target);
        self.open_layer(Box::new(d), LayerTag::new("gate2"));
    }

    fn gate2_confirmed(&mut self) {
        let Some(target) = self.pending_gate.take() else {
            return;
        };
        if matches!(target, GateTarget::Plan(_) | GateTarget::Cleanup(_)) && self.refuse_new_work()
        {
            return;
        }
        match target {
            GateTarget::Plan(id) => {
                let drift = self.world.plan(&id).and_then(|p| {
                    if p.drift.is_some() && !p.drift_consumed {
                        p.drift.clone()
                    } else {
                        None
                    }
                });
                if let Some(d) = drift {
                    if let Some(p) = self.world.plan_mut(&id) {
                        p.drift_consumed = true;
                    }
                    self.set_status(
                        &format!("Plan changed · {d} · confirmation invalidated, review again"),
                        Tone::Error,
                    );

                    self.enter_top();
                    return;
                }

                if !self.leave_review_pages() {
                    return;
                }
                let tick = self.world.tick;
                let armed = self.world.plan_mut(&id).is_some_and(|p| p.arm(tick));
                let title = self
                    .world
                    .plan(&id)
                    .map(|p| p.title.clone())
                    .unwrap_or_default();
                self.open_plan_tab(&id);
                let note = if armed {
                    "started"
                } else {
                    "is already running"
                };
                self.set_status(&format!("{title} {note}"), Tone::Secondary);
            }
            GateTarget::Item { item, args } => {
                if !self.leave_review_pages() {
                    return;
                }
                self.enter_top();
                self.execute_item(&item, args);
            }
            GateTarget::Cleanup(plan) => {
                let outcome = crate::screens::cleanup::commit(&plan, &mut self.world);
                match outcome {
                    Ok(index) => {
                        self.leave_review_pages();
                        self.push_page(Page::Report { index });
                        let n = plan.items.len();
                        self.set_status(
                            &format!(
                                "cleanup started · {} · quitting waits for it",
                                junie_tui::ui::text::plural(n, "item", "items")
                            ),
                            Tone::Secondary,
                        );
                    }
                    Err(e) => {
                        self.set_status(&e, Tone::Error);
                        self.enter_top();
                    }
                }
            }
        }
    }

    fn activity_name(it: &Item) -> String {
        if let Some(ns) = it.id.strip_prefix("mise.task.") {
            let ns = ns.trim_start_matches("//");
            return ns
                .replace(':', " ")
                .replace("apps/", "")
                .replace("services/", "")
                .replace("tools/", "");
        }
        let l = it.label.to_lowercase();
        for v in ["run ", "start ", "follow ", "open ", "connect to ", "show "] {
            if let Some(rest) = l.strip_prefix(v) {
                return rest.to_owned();
            }
        }
        l
    }

    fn execute_item(&mut self, id: &str, args: Vec<(String, String)>) {
        let Some(it) = self.find_item(id) else {
            return;
        };
        if matches!(
            it.launch,
            Launch::Activity { .. } | Launch::Batch | Launch::Handoff { .. }
        ) && self.refuse_new_work()
        {
            return;
        }
        self.remember_use(id);
        match it.launch.clone() {
            Launch::Activity { script } => {
                let name = Self::activity_name(&it);
                let kind = if it.id == "docker.compose_logs" {
                    ActivityKind::Logs {
                        services: self
                            .world
                            .docker
                            .compose
                            .as_ref()
                            .map(|c| {
                                c.services
                                    .iter()
                                    .filter(|s| !matches!(s.as_str(), "db" | "redis"))
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }
                } else {
                    ActivityKind::Task
                };

                let argv: Vec<crate::domain::exec::Command> = it
                    .exec
                    .iter()
                    .map(|c| {
                        let mut c = c.clone();
                        for a in &mut c.args {
                            for (k, v) in &args {
                                let ph = format!("<{k}>");
                                if a.contains(&ph) {
                                    *a = a.replace(&ph, v);
                                }
                            }
                        }
                        if c.program == "kill"
                            && let Some(sig) = args.iter().find(|(k, _)| k == "signal")
                        {
                            c.args[0] = format!("-{}", sig.1);
                        }
                        c
                    })
                    .collect();
                if argv.is_empty() && script.is_none() {
                    self.set_status(
                        &format!(
                            "{} has no executable specification in this preview",
                            it.label
                        ),
                        Tone::Error,
                    );
                    return;
                }
                let script = self.world.script_for(script.as_deref(), &argv);
                let aid = self.world.start_activity(
                    argv,
                    script,
                    &name,
                    &it.label,
                    kind,
                    it.scope.clone(),
                );
                self.world.edit_activity(&aid, |a| {
                    if !args.is_empty() {
                        a.insights = args
                            .iter()
                            .filter(|(k, _)| !it.args.iter().any(|s| &s.name == k && s.secret))
                            .cloned()
                            .collect();
                    }
                    a.effect = it.effect.clone();
                    if it.id == "system.kill"
                        && let Some(pid) = args
                            .iter()
                            .find(|(k, _)| k == "pid")
                            .and_then(|(_, v)| v.trim().parse::<u32>().ok())
                    {
                        a.effect = Some(crate::domain::effect::Effect::KillProcess(pid));
                    }
                });
                self.open_activity_tab(&aid, true);
                self.set_status(
                    &format!(
                        "Started {name} in {}",
                        self.world.location.short(&it.scope.runs_in)
                    ),
                    Tone::Secondary,
                );
            }
            Launch::Batch => {
                let members: Vec<_> = it
                    .batch
                    .iter()
                    .map(|(name, argv, effect, scope)| {
                        let script = self.world.script_for(None, argv);
                        (
                            name.clone(),
                            argv.clone(),
                            script,
                            effect.clone(),
                            scope.clone(),
                        )
                    })
                    .collect();
                let ids =
                    self.world
                        .start_batch(&it.id, &it.label, it.batch_mode, members, &it.label);
                if let Some(first) = ids.first() {
                    let first = first.clone();
                    self.open_activity_tab(&first, true);
                }
                self.set_status(
                    &format!(
                        "{} · {} started {}",
                        it.label,
                        junie_tui::ui::text::plural(ids.len(), "activity", "activities"),
                        match it.batch_mode {
                            crate::sim::world::BatchMode::Parallel =>
                                "in parallel · Ctrl+G lists them",
                            crate::sim::world::BatchMode::Sequential =>
                                "one after another · Ctrl+G lists them",
                        }
                    ),
                    Tone::Secondary,
                );
            }
            Launch::Handoff { tool } => {
                let script = match tool.as_str() {
                    "btm" => "monitor:btm".to_owned(),
                    "pg_activity" => "monitor:pg_activity".to_owned(),
                    t if t.starts_with("ssh ") => format!("ssh:{}", t.trim_start_matches("ssh ")),
                    t => format!("handoff:{t}"),
                };
                let kind = if tool.starts_with("ssh ") {
                    ActivityKind::Ssh {
                        alias: tool.trim_start_matches("ssh ").to_owned(),
                    }
                } else {
                    ActivityKind::Monitor { tool: tool.clone() }
                };
                let argv = it.exec.clone();
                let script = self.world.script_for(Some(&script), &argv);
                let aid = self.world.start_activity(
                    argv,
                    script,
                    &tool,
                    &it.label,
                    kind,
                    it.scope.clone(),
                );
                self.open_activity_tab(&aid, true);
                self.set_status(
                    &format!("{tool} opened as an activity · Enter attaches, Ctrl+] detaches"),
                    Tone::Secondary,
                );
            }
            Launch::Disk { path } => {
                let path = if path == "<path>" {
                    args.iter()
                        .find(|(k, _)| k == "path")
                        .map(|(_, v)| v.trim().to_owned())
                        .unwrap_or_default()
                } else {
                    path
                };
                self.activate_tab(0);
                if path.is_empty() {
                    self.push_page(Page::DiskOverview);
                    return;
                }
                if !path.starts_with('/') {
                    self.set_status(&format!("{path}: the path must be absolute"), Tone::Error);
                    return;
                }
                if !self.world.fs.exists(&path) {
                    self.set_status(&format!("{path}: no such file or directory"), Tone::Error);
                    return;
                }
                self.world.start_scan(&path);
                self.push_page(Page::Disk { path });
            }
            Launch::Files { path } => {
                self.activate_tab(0);
                self.push_page(Page::Files { path, query: None });
            }
            Launch::Find => {
                self.activate_tab(0);
                self.push_page(Page::Find);
            }
            Launch::Cleanup { category } => {
                self.activate_tab(0);
                self.push_page(Page::Cleanup { category });
            }
            Launch::TopFiles => {
                self.activate_tab(0);
                self.push_page(Page::TopFiles);
            }
            Launch::Config { path } => {
                self.activate_tab(0);
                self.push_page(Page::Config { path });
            }
            Launch::Copy { value } => self.copy_osc52(value),
            Launch::Group(g) => {
                self.activate_tab(0);
                self.push_page(Page::Finder { group: Some(g) });
            }
            Launch::Scope(s) => self.go(Go::Scope(s)),
            Launch::Snapshot { snapshot } => {
                self.activate_tab(0);
                let kind = if snapshot == "port" {
                    let port = args
                        .iter()
                        .find(|(k, _)| k == "port")
                        .map(|(_, v)| v.clone())
                        .unwrap_or("5173".into());
                    format!("port:{port}")
                } else {
                    snapshot
                };
                self.push_page(Page::Snapshot { kind });
            }
            Launch::OpenActivity { id } => self.open_activity_tab(&id, true),
            Launch::Plan { plan } => {
                self.activate_tab(0);
                self.push_page(Page::PlanReview { plan });
            }
            Launch::Insert => {
                let cmd = it.commands.first().cloned().unwrap_or_default();
                self.set_status(
                    &format!(
                        "Inserted into the shell: {} · holla returns to the prompt",
                        truncate(&cmd, 40)
                    ),
                    Tone::Secondary,
                );
            }
        }
    }

    pub fn copy_osc52(&mut self, value: String) {
        let p = &self.world.platform;

        let clipboard = shell::Clipboard::new()
            .osc52(p.osc52)
            .limit(p.osc52_limit)
            .write(false);
        let copied = clipboard.copy(&value);
        match copied {
            shell::Copied::Copied { .. } | shell::Copied::Requested { .. } => {
                let shown = truncate(&value.replace('\n', " · "), 40);
                self.world.clipboard = Some(value);
                self.clipboard_gen += 1;
                self.set_status(
                    &format!("Copied to the terminal clipboard (OSC 52, flushed) · {shown}"),
                    Tone::Secondary,
                );
            }
            shell::Copied::Unsupported => self.set_status(
                "Not copied · this terminal does not accept OSC 52 · no system clipboard is wired",
                Tone::Error,
            ),
            shell::Copied::TooLarge { .. } => {
                let (text, tone) = shell::Clipboard::report(&copied, &value, None);
                self.set_status(&text, tone);
            }
        }
    }

    pub(crate) fn open_alternatives(&mut self, id: &str, anchor: Rect) {
        let Some(it) = self.find_item(id) else {
            return;
        };
        let mut items = vec![];
        let mut actions = vec![];
        let primary = match (&it.launch, &it.confirmation) {
            (_, Confirmation::TwoGate { .. }) => "Review…",
            (Launch::Plan { .. }, _) => "Review the plan",
            (
                Launch::Disk { .. } | Launch::Group(_) | Launch::Snapshot { .. } | Launch::Scope(_),
                _,
            ) => "Open",
            (_, Confirmation::One) => "Run after confirmation…",
            _ => "Run now",
        };
        let mut m = MenuItem::new(primary).shortcut("Enter");
        if matches!(it.risk, Risk::Destructive | Risk::Privileged) {
            m = m.danger();
        }
        items.push(m);
        actions.push(AltCmd::Run);
        if !it.commands.is_empty() {
            items.push(MenuItem::new("Copy command").shortcut("y"));
            actions.push(AltCmd::Copy);
            items.push(MenuItem::new("Insert into shell"));
            actions.push(AltCmd::Insert);
        }
        if !it.args.is_empty() {
            items.push(MenuItem::new("Arguments…"));
            actions.push(AltCmd::Args);
        }
        if let Some(last) = items.last_mut() {
            last.separator_after = true;
        }
        for a in &it.alternatives {
            let mut item = MenuItem::new(a.label.clone());

            if !self.alternative_supported(&a.item) {
                item = item.reason("Not in this preview");
            }
            items.push(item);
            actions.push(AltCmd::Alternative(a.item.clone()));
        }
        if !it.alternatives.is_empty()
            && let Some(last) = items.last_mut()
        {
            last.separator_after = true;
        }
        items.push(MenuItem::new(if it.pinned {
            "Unpin here"
        } else {
            "Pin here"
        }));
        actions.push(AltCmd::Pin);
        items.push(MenuItem::new(match self.world.memory.alias_for(&it.id) {
            Some(a) => format!("Change alias ({a})…"),
            None => "Set alias…".into(),
        }));
        actions.push(AltCmd::Alias);
        items.push(MenuItem::new(if it.hidden {
            "Restore here"
        } else {
            "Hide here"
        }));
        actions.push(AltCmd::Hide);
        items.push(MenuItem::new("Reset ranking").separator());
        actions.push(AltCmd::Reset);
        items.push(MenuItem::new("Why is this here?"));
        actions.push(AltCmd::Why);
        let menu = ContextMenu::new(WidgetId::of("alternatives"), items)
            .anchor(anchor, Placement::Below)
            .title(&it.label);
        self.menu_actions = actions;
        self.menu_item = Some(it.id.clone());
        self.open_layer(
            Box::new(menu),
            LayerTag::new("alternatives").key(it.id.clone()),
        );
    }

    pub(crate) fn alternative_supported(&self, alt: &str) -> bool {
        self.find_item(alt).is_some() || derived_supported(alt)
    }

    fn run_derived(&mut self, alt: &str, origin: &str) {
        let tick = self.world.tick;
        let scope = self
            .find_item(origin)
            .map(|i| i.scope.clone())
            .unwrap_or_else(|| crate::domain::context::ScopeTag::here(&self.world.location.cwd));
        if let Some(ns) = alt
            .strip_prefix("mise.task.")
            .and_then(|s| s.strip_suffix(".dry"))
        {
            if self.refuse_new_work() {
                return;
            }
            let argv = vec![crate::domain::exec::Command::argv(
                "mise",
                &["run", "--dry-run", ns],
                &scope.runs_in,
                &self.world.host.name,
            )];
            let script = self.world.script_for(None, &argv);
            let aid = self.world.start_activity(
                argv,
                script,
                &format!("{} · dry run", ns.trim_start_matches("//")),
                "Dry run",
                ActivityKind::Task,
                scope,
            );
            self.open_activity_tab(&aid, true);
            self.set_status("Dry run started · nothing executes", Tone::Secondary);
            return;
        }
        if let Some(rest) = alt.strip_prefix("ssh.")
            && let Some(alias) = rest.strip_suffix(".resolve")
        {
            self.activate_tab(0);
            self.push_page(Page::Snapshot {
                kind: format!("ssh:{alias}"),
            });
            return;
        }
        if let Some(rest) = alt.strip_prefix("activity.") {
            if let Some(aid) = rest.strip_suffix(".stop") {
                self.world.edit_activity(aid, |a| a.stop(tick));
                self.set_status(
                    "Stop requested · output kept · the state settles once the process is gone",
                    Tone::Secondary,
                );
                return;
            }
            if let Some(aid) = rest.strip_suffix(".restart") {
                if self.refuse_new_work() {
                    return;
                }
                if self.world.activity(aid).is_some_and(|a| a.state.live()) {
                    self.set_status("Still running · stop it first", Tone::Secondary);
                    return;
                }
                self.world.edit_activity(aid, |a| {
                    a.restart(tick);
                    a.advance(tick);
                });
                let aid = aid.to_owned();
                self.open_activity_tab(&aid, true);
                self.set_status("Restarted", Tone::Secondary);
                return;
            }
        }
        if alt.ends_with(".alias") {
            self.menu_action(AltCmd::Alias, origin);
        }
    }

    fn menu_action(&mut self, a: AltCmd, id: &str) {
        let cwd = self.world.location.cwd.clone();
        match a {
            AltCmd::Run => self.run_item(id, vec![]),
            AltCmd::Copy => {
                let cmd = self
                    .find_item(id)
                    .map(|i| i.commands.join("\n"))
                    .unwrap_or_default();
                self.copy(cmd);
            }
            AltCmd::Insert => {
                let cmd = self
                    .find_item(id)
                    .and_then(|i| i.commands.first().cloned())
                    .unwrap_or_default();
                self.set_status(
                    &format!(
                        "Inserted into the shell: {} · holla returns to the prompt",
                        truncate(&cmd, 40)
                    ),
                    Tone::Secondary,
                );
            }
            AltCmd::Args => {
                self.activate_tab(0);
                self.push_page(Page::Args {
                    item: id.to_owned(),
                });
            }
            AltCmd::Alternative(alt) => {
                if self.find_item(&alt).is_some() {
                    self.run_item(&alt, vec![]);
                } else {
                    self.run_derived(&alt, id);
                }
            }
            AltCmd::Pin => {
                let on = self.world.edit_memory(|w| w.memory.toggle_pin(&cwd, id));
                self.set_status(
                    if on {
                        "Pinned here · it stays near the top in this folder"
                    } else {
                        "Unpinned"
                    },
                    Tone::Secondary,
                );
                self.enter_top();
            }
            AltCmd::Alias => {
                let existing = self.world.memory.alias_for(id).unwrap_or("").to_owned();
                let input = TextInput::new(WidgetId::of("alias").sub("field"), "Alias")
                    .value(&existing)
                    .placeholder("gp · du · dc")
                    .help("exact match beats every learned signal · never drifts");
                let d = Dialog::prompt(WidgetId::of("alias"), "Set an alias", input, "Save");
                self.open_layer(Box::new(d), LayerTag::new("alias").key(id.to_owned()));
            }
            AltCmd::Hide => {
                let hidden = self
                    .world
                    .memory
                    .hidden
                    .iter()
                    .any(|(p, i)| p == &cwd && i == id);
                if hidden {
                    self.world.edit_memory(|w| w.memory.restore(&cwd, id));
                    self.set_status("Restored here", Tone::Secondary);
                } else {
                    self.world.edit_memory(|w| w.memory.hide(&cwd, id));
                    self.set_status(
                        "Hidden here · still reachable by search · Restore from the same menu",
                        Tone::Secondary,
                    );
                }
                self.enter_top();
            }
            AltCmd::Reset => {
                self.world.edit_memory(|w| w.memory.reset(id));
                self.set_status(
                    "Ranking reset for this row · pins and aliases kept",
                    Tone::Secondary,
                );
                self.enter_top();
            }
            AltCmd::Why => {
                if let Some(it) = self.find_item(id) {
                    let cwd_short = self.world.location.cwd_short();
                    let rows = crate::domain::ranking::explain(&it, &cwd_short);
                    let mut lines: Vec<TextLine> =
                        rows.iter().map(|(k, v)| TextLine::kv(k, v)).collect();
                    lines.push(TextLine::blank());
                    lines.push(TextLine::text("Ranking order: alias › pin › live state › context › query › used here › used anywhere › default. Risk changes the gate, never the rank.", Tone::Muted));
                    let m = TextModal::new(
                        WidgetId::of("why"),
                        &format!("Why is “{}” here?", it.label),
                        lines,
                    )
                    .width(74);
                    self.open_layer(Box::new(m), LayerTag::new("why"));
                }
            }
        }
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.draw_frame(area, buf, ctx);

        let mut modals = std::mem::take(&mut self.modals);
        for entry in modals.iter_mut() {
            let spec = entry.layer.spec();
            if spec.inert_below {
                ctx.begin_modal(buf);
            }
            entry.layer.render(area, buf, ctx, &());
            if let Some(id) = spec.initial_focus
                && !ctx.ring.contains(id)
            {
                ctx.ring.register(id);
            }
        }
        let open = !modals.is_empty();
        self.modals = modals;
        if open {
            let r = CHROME.regions(area).footer;
            let footer = Rect::new(r.x, r.bottom() - 1, r.width, 1);
            self.footer(true).render(footer, buf, ctx, &mut self.shell);
        }
    }

    fn crumb(&self) -> String {
        self.crumb_parts().join(" › ")
    }

    fn crumb_parts(&self) -> Vec<String> {
        trail(self.router.tab(0).map(|s| s.crumb(&self.world)).collect())
    }

    fn tab_label(&self, i: usize) -> String {
        match &self.tabs[i].kind {
            TabKind::Here => self.crumb(),
            TabKind::Activity(id) => self
                .world
                .activity(id)
                .map(|a| a.name.clone())
                .unwrap_or("activity".into()),
            TabKind::Plan(id) => self
                .world
                .plan(id)
                .map(|p| p.title.clone())
                .unwrap_or("plan".into()),
        }
    }

    fn strip_items(&self) -> Vec<TabItem> {
        let mut v = vec![];
        for (i, tab) in self.tabs.iter().enumerate() {
            let label = self.tab_label(i);
            let label = if i == 0 {
                crumb_fit(&label, 48)
            } else {
                truncate(&label, 24)
            };
            let mut item = TabItem::new(&label);
            match &tab.kind {
                TabKind::Here => {}
                TabKind::Activity(id) => {
                    item = item.closable();
                    if let Some(a) = self.world.activity(id) {
                        match a.state {
                            ActivityState::Running | ActivityState::Cancelling => item.busy = true,
                            ActivityState::Queued => item = item.suffix("…"),
                            ActivityState::Failed => item.error = true,
                            ActivityState::Succeeded => item = item.done(true),
                            ActivityState::Stopped | ActivityState::Detached => {
                                item = item.suffix("○")
                            }
                        }
                    }
                }
                TabKind::Plan(id) => {
                    item = item.closable();
                    if let Some(p) = self.world.plan(id) {
                        match p.phase {
                            PlanPhase::Running => item.busy = true,
                            PlanPhase::Done => {
                                if p.outcome().1 > 0 {
                                    item.error = true;
                                } else {
                                    item = item.done(true);
                                }
                            }
                            PlanPhase::Review => {}
                        }
                    }
                }
            }
            v.push(item);
        }
        v
    }

    fn draw_frame(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        let r = CHROME.regions(area);

        let menu = Rect {
            height: 1,
            ..r.header
        };
        let strip = Rect::new(
            r.header.x + 1,
            r.header.y + 2,
            r.header.width.saturating_sub(2),
            2,
        );

        let status = Rect::new(r.footer.x, r.footer.bottom() - 2, r.footer.width, 1);
        let footer = Rect::new(r.footer.x, r.footer.bottom() - 1, r.footer.width, 1);
        self.draw_menu(menu, buf, ctx);

        let items = self.strip_items();
        self.strip.set_items(items);
        self.strip.set_active(self.active());
        self.strip.render(strip, buf, ctx, t.canvas);

        if self.active() == 0 && self.router.depth() > 1 {
            let mut crumbs = self.router.breadcrumb(CRUMBS, |s| s.crumb(&self.world));
            crumbs.segments = trail(crumbs.segments);
            let row = Rect::new(
                strip.x + 1,
                r.header.y + 1,
                strip.width.saturating_sub(2),
                1,
            );
            crumbs.render(row, buf, ctx, t.canvas);
            self.crumbs = crumbs;
        } else {
            self.crumbs = Breadcrumb::new(CRUMBS, vec![]);
        }

        self.router
            .current_mut()
            .render(r.body, buf, ctx, &self.world);
        self.draw_status(status, buf, ctx);
        self.footer(false).render(footer, buf, ctx, &mut self.shell);
    }

    fn draw_menu(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        self.menu.render(area, buf, ctx, t.canvas);
        let used = self
            .menu
            .areas
            .iter()
            .map(|r| r.right())
            .max()
            .unwrap_or(area.x)
            .max(self.menu.brand_area.right());
        let rest = Rect::new(used + 2, area.y, area.right().saturating_sub(used + 2), 1);
        let w = &self.world;
        let mut right = vec![];
        let project = match (&w.location.workspace, &w.location.project) {
            (Some(ws), Some(p)) => format!("{} › {} · {}", ws.name, p.name, p.kind.label()),
            (None, Some(p)) => format!("{} · {}", p.name, p.kind.label()),
            _ => "no project".into(),
        };
        let host = match w.host.role {
            HostRole::Production => format!("◆ {} · production", w.host.name),
            r => format!("{} · {}", w.host.name, r.label()),
        };

        let budget = (rest.width as usize).saturating_sub(width(&host) + 6);
        if budget >= 12 {
            right
                .push(Segment::new(truncate_middle(&project, budget), Tone::Secondary).priority(6));
        }

        right.push(Segment::new(host, Tone::Normal).bold().priority(9));
        segments::render(rest, buf, ctx, &[], &right, t.canvas);
    }

    fn draw_status(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let w = &self.world;
        let mut bar = StatusBar::new();
        bar.left.push(
            StatusItem::new(truncate_middle(&w.location.cwd_short(), 44), Tone::Normal)
                .strong()
                .clickable(STATUS_PATH)
                .priority(10),
        );
        if let Some(g) = w.git_here() {
            let state = g.state_parts();
            bar.left.push(
                StatusItem::new(truncate_middle(&g.head_label(), 24), Tone::Muted)
                    .clickable(STATUS_GIT)
                    .priority(if state.is_empty() { 5 } else { 4 }),
            );
            for (text, tone) in state {
                bar.left.push(
                    StatusItem::new(text, tone)
                        .clickable(STATUS_GIT)
                        .priority(5),
                );
            }
        }
        let bits = self.top().map(|s| s.status(w)).unwrap_or_default();
        if let Some(c) = bits.center {
            bar.center.push(c);
        }
        for r in bits.right {
            bar.right.push(r);
        }
        if self.attached() {
            bar.right.push(
                StatusItem::new("attached", Tone::Secondary)
                    .chip()
                    .priority(8),
            );
        }
        let live = w.live_activities();
        if live > 0 {
            bar.right.push(
                StatusItem::new(
                    junie_tui::ui::text::plural(live, "running", "running"),
                    Tone::Secondary,
                )
                .busy()
                .chip()
                .clickable(STATUS_RUNNING)
                .priority(6),
            );
        }
        if let Some(fs) = w.disk.root_fs()
            && fs.pct() >= 85
        {
            bar.right.push(
                StatusItem::new(fs.mount.clone(), Tone::Muted)
                    .meter(Some(fs.pct()), crate::screens::disk::FS_METER)
                    .priority(4),
            );
        }
        bar.right.push(
            StatusItem::new(
                format!(
                    "{} · {}×{}",
                    self.theme.level.label(),
                    self.shell.size.0,
                    self.shell.size.1
                ),
                Tone::Faint,
            )
            .priority(1),
        );
        bar.render(area, buf, ctx);
    }

    fn footer(&self, modal: bool) -> shell::Footer {
        use junie_tui::core::hints::{HasHints, HintCx};
        let focus = self.shell.focus.current();
        let editing = self.modals.top().is_some_and(|t| t.layer.is_editing())
            || (!modal && self.top().is_some_and(|s| s.is_editing()) && self.active() == 0);
        let page = if focus == Some(TABS) {
            vec![
                hint("←→", "Switch tab"),
                hint("1–9", "Jump"),
                hint("x", "Close"),
                hint("Enter", "Into the tab"),
                hint("Esc", "Back"),
            ]
        } else {
            self.top()
                .map(|s| s.hints(focus, &self.world))
                .unwrap_or_default()
        };
        let footer = if modal {
            shell::Footer::new().layer(self.modals.top().map(|t| t.layer.hints(focus)))
        } else {
            shell::Footer::new()
                .layer(Some(HasHints::hints(&self.menu, &HintCx::new(focus))))
                .layer(Some(page))
                .fallback(vec![hint("F1", "Help"), hint("Esc", "Back")])
        };
        footer.editing(editing).status_parts(
            self.shell.status.get(),
            self.shell.status.payload().cloned().unwrap_or_default(),
        )
    }
}

const BRAND_PADDED: &str = " holla❯ ";

impl shell::App for App {
    type Status = Vec<(String, Tone)>;
    fn shell(&self) -> &Shell<Vec<(String, Tone)>> {
        &self.shell
    }
    fn shell_mut(&mut self) -> &mut Shell<Vec<(String, Tone)>> {
        &mut self.shell
    }
    fn theme(&self) -> Theme {
        self.theme
    }
    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new(BRAND_PADDED).brand_style(Lockup::style(&self.theme))
    }
    fn now_ms(&self) -> u64 {
        App::now_ms(self)
    }
    fn tick(&self) -> u64 {
        (self.world.now_ms() / 80) as u64
    }
    fn tick_interval(&self) -> std::time::Duration {
        App::tick_interval(self)
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        App::draw(self, area, buf, ctx)
    }
    fn on_input(&mut self, input: Input) -> Outcome {
        self.handle_input(input)
    }
    fn advance_clock(&mut self, ms: u64) -> Option<Outcome> {
        Some(junie_tui::clock::run_step(
            self,
            |a| &mut a.world.clock,
            ms,
            Self::on_tick,
        ))
    }
    fn on_blur(&mut self, ch: FocusChange) -> Outcome {
        self.blur_all(ch)
    }
    fn after_input(&mut self, _input: &Input, out: Outcome) -> Outcome {
        self.apply_pending(out)
    }
    fn layer_open(&self) -> bool {
        !self.modals.is_empty()
    }

    fn stakes(&mut self) -> Stakes {
        let mut stakes = self.quit_stakes();
        for p in self.router.pages_mut() {
            stakes = stakes.max(shell::Page::<Holla>::stakes(p.as_mut(), &self.world));
        }
        stakes
    }
    fn page_under_layer(&self) -> bool {
        self.menu.hides_page_focus() || self.modals.hides_page_focus()
    }
    fn page_key(&self) -> PageKey {
        self.router.key()
    }
    fn primary_focus(&self) -> Option<WidgetId> {
        self.top().and_then(|s| s.primary_focus())
    }
}

impl PointerHost for App {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        self.shell.pointer_parts()
    }

    fn layer_open(&self) -> bool {
        !self.modals.is_empty()
    }

    fn now_ms(&self) -> u64 {
        App::now_ms(self)
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        self.deliver_pointer(ev).into()
    }

    fn outside(&mut self, _pos: Position) -> Response<()> {
        if !self.modals.is_empty() {
            return self
                .dismiss_layer(DismissReason::OutsideClick)
                .or(Outcome::Changed)
                .into();
        }
        if self.menu.is_open() {
            self.menu.close();
        }
        Response::changed()
    }
}

fn trail(crumbs: Vec<String>) -> Vec<String> {
    let mut parts: Vec<String> = vec![];
    for crumb in crumbs {
        let (family, rest) = match crumb.split_once(" › ") {
            Some((f, r)) => (f.to_owned(), Some(r.to_owned())),
            None => (crumb.clone(), None),
        };
        let same_family = parts
            .last()
            .is_some_and(|prev| prev.split(" › ").next() == Some(family.as_str()));
        match (same_family, rest) {
            (true, Some(r)) => parts.push(r),
            _ => parts.push(crumb),
        }
    }
    parts
}

fn ensure_plan(world: &mut World, id: &str) {
    if world.plan(id).is_some() {
        return;
    }
    let plan = match id {
        "docker-cleanup" => crate::domain::fixtures::plan_docker_cleanup(world),
        "upgrade" => crate::domain::fixtures::plan_upgrade(world),
        "git-pull-all" => crate::domain::fixtures::plan_git_pull_all(world),
        "git-switch-primary" => crate::domain::fixtures::plan_git_switch_primary(world),
        "upgrade-all" => crate::domain::fixtures::plan_upgrade_all(world),
        _ => {
            let sel: Vec<String> = world
                .disk
                .candidates
                .iter()
                .filter(|c| c.default_selected() && c.path.starts_with(&world.location.cwd))
                .map(|c| c.path.clone())
                .collect();
            crate::domain::fixtures::plan_cleanup_work(world, &sel)
        }
    };
    world.plans.push(plan);
}

fn answer(result: &ModalResult) -> DialogResult {
    match result {
        ModalResult::Dialog {
            action: Some(i), ..
        } => DialogResult::Action(*i),
        _ => DialogResult::Cancelled,
    }
}

fn derived_supported(alt: &str) -> bool {
    let mise_dry = alt
        .strip_prefix("mise.task.")
        .is_some_and(|s| s.ends_with(".dry"));
    let ssh = alt
        .strip_prefix("ssh.")
        .is_some_and(|s| s.ends_with(".resolve"));
    let activity = alt
        .strip_prefix("activity.")
        .is_some_and(|s| s.ends_with(".stop") || s.ends_with(".restart"));
    mise_dry || ssh || activity || alt.ends_with(".alias")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HollaCmd {
    Activities,

    CloseTab,

    Tab(u8),

    Run,

    Alternatives,

    CopyCommand,

    Quit,

    Scope(Scope),

    Group(&'static str),

    Help,

    Why,

    About,
}

pub fn shell_keys() -> shell::ShellKeys<HollaCmd> {
    use junie_tui::core::keymap::KeyBinding;
    use shell::ShellAction::App as Cmd;
    let mut keys = shell::ShellKeys::new()
        .quit(junie_tui::ui::leave::QuitKeys::DEFAULT.letter(None))
        .bind(KeyBinding::ctrl('g'), Cmd(HollaCmd::Activities))
        .bind(KeyBinding::ctrl('w'), Cmd(HollaCmd::CloseTab));
    for d in 0..=9u8 {
        keys = keys.bind(KeyBinding::alt(char::from(b'0' + d)), Cmd(HollaCmd::Tab(d)));
    }
    keys
}

impl shell::KeyHost for App {
    type Cmd = HollaCmd;

    fn shell_keys(&self) -> &shell::ShellKeys<HollaCmd> {
        &self.keys
    }

    fn layer_key(&mut self, key: &Key) -> Option<Outcome> {
        if self.modals.is_empty() {
            return None;
        }
        Some(App::layer_key(self, *key))
    }

    fn menu_key(&mut self, key: &Key) -> Option<Outcome> {
        self.menu.is_open().then(|| self.open_menu_key(key))
    }

    fn capturing(&self) -> bool {
        self.attached()
    }

    fn text_focus(&self) -> bool {
        self.attached() || self.top().is_some_and(|s| s.typing_hot())
    }

    fn interrupt(&mut self) -> Outcome {
        self.with_top(|s, w, cx| s.on_interrupt(w, cx))
    }

    fn shell_action(&mut self, action: shell::ShellAction<HollaCmd>, key: &Key) -> Outcome {
        use shell::ShellAction;
        match action {
            ShellAction::MenuBar if self.shell.context.keys.matches(key) => Outcome::Ignored,
            ShellAction::MenuBar => {
                self.menu.open_menu(0);
                Outcome::Changed
            }
            ShellAction::Help => {
                self.open_help();
                Outcome::Changed
            }
            ShellAction::NextPage => {
                let n = self.router.tab_count();
                self.tab_key((self.active() + 1) % n)
            }
            ShellAction::PrevPage => {
                let n = self.router.tab_count();
                self.tab_key((self.active() + n - 1) % n)
            }
            ShellAction::App(cmd) => self.run_cmd(cmd),
            _ => Outcome::Ignored,
        }
    }

    fn chrome_key(&mut self, key: &Key) -> Outcome {
        self.chrome_focus_key(key)
    }

    fn page_key(&mut self, key: &Key) -> Outcome {
        self.with_top(|s, w, cx| s.on_key(key, w, cx))
    }

    fn layer_paste(&mut self, text: &str) -> Option<Outcome> {
        let top = self.modals.top_mut()?;
        let out = top.layer.on_paste(text);
        Some(self.settle_layer(out.or(Outcome::Consumed)))
    }

    fn page_paste(&mut self, text: &str) -> Outcome {
        self.with_top(|s, w, cx| s.on_paste(text, w, cx))
    }

    fn esc_top(&mut self) -> Outcome {
        if self.active() != 0 {
            self.activate_tab(0);
            return Outcome::Changed;
        }
        self.with_top(|s, w, cx| s.on_esc_top(w, cx))
    }

    fn request_quit(&mut self) -> Outcome {
        App::request_quit(self);
        Outcome::Changed
    }

    fn note(&mut self, text: &str) {
        self.set_status(text, Tone::Secondary);
    }
}

#[cfg(test)]
mod tests;
