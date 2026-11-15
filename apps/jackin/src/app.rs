use std::sync::Arc;

use junie_tui::clock::Clock;
use junie_tui::core::event::{External, Input, Key, MouseKind, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{
    MouseTarget, PointerEvent, PointerHost, PointerParts, PointerRouter,
};
use junie_tui::core::response::Response;
use junie_tui::motion::Motion;
use junie_tui::runtime::ExternalReceiver;
use junie_tui::shell::{self, Chrome, Menus, NativeClipboard, PageKey, Router, Shell};
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::layer::{
    DismissReason, Dismissed, LayerCx, LayerEntry, LayerPointer, ShellLayers, Taken,
};
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::hintbar::HintBar;
use junie_tui::widgets::keyhint::hint;
use junie_tui::widgets::picker::{Picker, PickerAction};
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::too_small::TooSmall;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use crate::arbiter::{EntryDecision, ExitDecision};
use crate::backend::{Effect, Ticket};
use crate::domain::instance::{InstanceEntry, InstanceId, InstanceStatus};
use crate::domain::launch::EmptyLanding;
use crate::domain::vocabulary::fill_in;
use crate::launch::LaunchTarget;
use crate::pages::{Page, Pages, TAB_ACCOUNTS, TAB_MANAGER, TAB_ROLES, TAB_USAGE};
use crate::panes::{PaneId, PanePolicy};
use crate::rain::{self, HANDOFF_LEN, IntroState, OutroState, handoff_stage};
use crate::screens::capsule::CapsuleScreen;
use crate::screens::cockpit::CockpitScreen;
use crate::screens::editor::EditorScreen;
use crate::screens::modals::{FormAction, FormDialog, HelpOverlay, InfoResult, OpFlow};
use crate::screens::prelude::PreludeScreen;
use crate::screens::role_editor::RoleEditorScreen;
use crate::screens::settings::SettingsScreen;
use crate::screens::{AppLayer, Cx, Go, LayerTag, ModalResult, Request, Screen};
use crate::world::{Absorbed, Health, Msg, Snapshot, World, job};
use junie_tui::shell::Page as _;
use junie_tui::ui::leave::{LeaveGuard, Stakes};
use junie_tui::widgets::brand::Lockup;
use junie_tui::widgets::dialog::DialogResult;
use junie_tui::widgets::menu::{MenuBar, MenuBarAction, MenuItem};

pub const BRAND_MARK: &str = "jackin❯";
const STRIP_HELP: WidgetId = WidgetId::of("strip.help");
const HOST_MENU: WidgetId = WidgetId::of("host.menu");
const STRIP_USAGE: WidgetId = WidgetId::of("strip.usage");
const STRIP_ACCOUNTS: WidgetId = WidgetId::of("strip.accounts");
const STRIP_SETTINGS: WidgetId = WidgetId::of("strip.settings");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Intro,
    Manager,
    Prelude,
    Editor,
    Settings,
    Accounts,
    Usage,
    Roles,
    RoleEditor,
    Doctor,
    Cockpit,
    Handoff,
    Capsule,
    Outro,
}

impl Route {
    fn tick_ms(self, animating: bool) -> u64 {
        match self {
            Route::Intro | Route::Outro | Route::Handoff | Route::Cockpit => rain::TICK_MS,
            Route::Capsule => 80,
            _ => {
                if animating {
                    80
                } else {
                    200
                }
            }
        }
    }
}

type Layers = ShellLayers<ModalResult, World, Route>;
type ClosedLayer = LayerEntry<ModalResult, World, Route>;

#[derive(Debug, Clone)]
pub enum Remote {
    Msg(Msg),
    Reloaded(Box<Snapshot>),
}

pub type LaunchEntry = LaunchTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryPage {
    Manager,
    Accounts,
    Roles {
        select: Option<String>,
    },
    RoleEditor {
        role: String,
    },
    Launch(LaunchEntry),
    Attach {
        instance: InstanceId,
        pane: Option<PaneId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub page: EntryPage,
    pub outro_when_framed: bool,
}

#[derive(Debug, Clone)]
pub enum ClipboardMode {
    Preview,
    Host(Arc<dyn NativeClipboard>),
}

#[derive(Debug, Clone)]
pub struct AppOptions {
    pub motion: Motion,
    pub frame: u64,
    pub theme: Theme,
    pub clipboard: ClipboardMode,
    pub panes: PanePolicy,
}

pub fn assemble(world: World, entry: Entry, options: AppOptions) -> App {
    App::new(world, entry, options)
}

pub struct App {
    pub theme: Theme,
    pub world: World,
    pub intro: Option<IntroState>,
    pub outro: Option<OutroState>,

    pub handoff: Option<u64>,

    pub router: Router<Page>,
    layers: Layers,

    pub shell: Shell,

    pub keys: shell::ShellKeys<JackinCmd>,

    quit_guard: LeaveGuard<()>,

    pub host_menu: MenuBar,

    host_menus: Menus<HostCmd>,
    host_menu_route: Route,
    intro_guard: u8,
    pub clipboard_gen: u32,
    exit_after_status: Option<i64>,
    external: Option<ExternalReceiver>,
    held: Vec<Msg>,
    surface_kinds: SurfaceKinds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SurfaceKinds {
    roles: bool,
    doctor: bool,
}

impl SurfaceKinds {
    fn of(world: &World) -> Self {
        Self {
            roles: world.surfaces.roles.is_some(),
            doctor: world.surfaces.doctor.is_some(),
        }
    }
}

impl App {
    pub fn new(mut world: World, entry: Entry, options: AppOptions) -> Self {
        let AppOptions {
            motion,
            frame,
            theme,
            clipboard,
            panes,
        } = options;
        world.pane_policy = Arc::new(panes);
        for daemon in world.daemons.values_mut() {
            daemon.set_policy(&world.pane_policy);
        }
        let mut shell = Shell::new();
        shell.motion = motion;
        // Under the test clock only `advance_clock` moves the simulated clock.
        world.clock.running = motion != Motion::Paused && !shell.test_requests;

        shell.set_clipboard(match clipboard {
            ClipboardMode::Preview => shell::Clipboard::new().write(false),
            ClipboardMode::Host(sink) => shell::Clipboard::new().native(sink),
        });
        let router = crate::pages::router(&world);
        let surface_kinds = SurfaceKinds::of(&world);
        let keys = shell_keys(&world);
        let host_menus = Self::build_host_menu(Route::Manager, &world);
        let mut app = Self {
            theme,
            world,
            intro: None,
            outro: None,
            handoff: None,
            router,
            layers: ShellLayers::new(),
            shell,
            keys,
            quit_guard: LeaveGuard::new(WidgetId::of("quit.guard")),
            host_menu: Self::host_bar(&host_menus),
            host_menus,
            host_menu_route: Route::Manager,
            intro_guard: 3,
            clipboard_gen: 0,
            exit_after_status: None,
            external: None,
            held: Vec::new(),
            surface_kinds,
        };
        app.start(entry, frame);
        app
    }

    /// Feeds [`Remote`] messages from a background host into the app.
    ///
    /// A host whose producer can outrun the UI builds the channel with
    /// `junie_tui::runtime::bounded_external_channel` and retries
    /// `ExternalSender::try_send` on `Full`. The runtime hands the app a bounded
    /// group of queued messages per turn, whichever constructor made the channel.
    pub fn with_external(mut self, receiver: ExternalReceiver) -> Self {
        self.external = Some(receiver);
        self
    }

    pub fn deliver(&mut self, msg: &Msg) -> Outcome {
        self.dispatch_msg(msg)
    }

    fn on_external(&mut self, external: &External) -> Outcome {
        match external.downcast_ref::<Remote>() {
            Some(Remote::Msg(msg)) => self.dispatch_msg(msg),
            Some(Remote::Reloaded(snapshot)) => {
                let shown_validation = self.world.displayed_validation.clone();
                self.world.apply((**snapshot).clone());
                self.end_retired_validation(shown_validation);
                self.after_snapshot();
                Outcome::Changed
            }
            None => Outcome::Ignored,
        }
    }

    fn after_snapshot(&mut self) {
        self.reconcile_surfaces();
        self.drop_orphaned_daemons();
    }

    fn reconcile_surfaces(&mut self) {
        let wanted = SurfaceKinds::of(&self.world);
        if wanted == self.surface_kinds {
            return;
        }
        self.surface_kinds = wanted;
        self.keys = shell_keys(&self.world);
        let active = self.router.active();
        let has_roles_tab = self.router.tab_count() > TAB_ROLES;
        match (has_roles_tab, wanted.roles) {
            (false, true) => {
                self.router.open_tab(Page::Roles(Box::default()));
                self.router.select(active);
            }
            (true, false) => {
                self.drop_layers_of(Route::Roles);
                if active == TAB_ROLES {
                    self.show_tab(TAB_MANAGER);
                    self.enter_route();
                }
                self.router.close_tab(TAB_ROLES);
            }
            _ => {}
        }
        self.host_menus = Self::build_host_menu(self.host_menu_route, &self.world);
        self.host_menu = Self::host_bar(&self.host_menus);
    }

    fn drop_layers_of(&mut self, owner: Route) {
        while self.layers.iter().any(|e| e.owner == owner) {
            self.layers.pop(&mut self.shell.focus);
        }
        self.layer_gone();
    }

    fn drop_orphaned_daemons(&mut self) {
        let mut degraded: Vec<(String, String)> = Vec::new();
        let gone: Vec<(InstanceId, String, String)> = self
            .world
            .daemons
            .keys()
            .filter_map(|id| match self.world.entry(id) {
                None => Some((
                    id.clone(),
                    id.short_label(),
                    "was removed elsewhere".to_owned(),
                )),
                Some(InstanceEntry::Unavailable(unavailable)) if unavailable.phase.ready() => {
                    let display = unavailable.display();
                    degraded.push((
                        display.sandbox_name,
                        format!("metadata unavailable · {}", display.reason),
                    ));
                    None
                }
                Some(InstanceEntry::Unavailable(unavailable)) => {
                    let display = unavailable.display();
                    Some((
                        id.clone(),
                        display.sandbox_name,
                        format!("metadata unavailable · {}", display.reason),
                    ))
                }
                Some(InstanceEntry::Available(i)) if i.status != InstanceStatus::Running => {
                    Some((id.clone(), id.short_label(), "stopped elsewhere".to_owned()))
                }
                Some(InstanceEntry::Available(_)) => None,
            })
            .collect();
        for (name, why) in degraded {
            self.set_status(&format!("Instance {name} {why}"), Tone::Secondary);
        }
        let open = self.open_capsule();
        for (instance, name, why) in gone {
            if open.as_ref() == Some(&instance) {
                self.go(Go::InstanceEnded {
                    instance: instance.clone(),
                    purge: false,
                });
                self.set_status(&format!("Instance {name} {why}"), Tone::Secondary);
            } else {
                self.world.daemons.remove(&instance);
            }
        }
    }

    fn start(&mut self, entry: Entry, frame: u64) {
        if entry.outro_when_framed && frame > 0 {
            self.world.arbiter.set_running(0);
            let decision = self.world.arbiter.request_exit(self.world.now_ms());
            let elapsed = match decision {
                ExitDecision::Outro { elapsed_secs } => elapsed_secs,
                _ => Some(8_040),
            };
            self.outro = Some(OutroState::new(self.shell.motion, elapsed, frame));
            return;
        }
        match self.world.arbiter.request_entry() {
            EntryDecision::PlayIntro => {
                if frame >= rain::INTRO_END && self.shell.motion != Motion::Reduced {
                    self.world.arbiter.complete_entry(self.world.now_ms());
                    self.enter_manager();
                } else {
                    self.intro = Some(IntroState::new(self.shell.motion, frame));
                }
            }
            EntryDecision::JoinActive { .. } => self.enter_manager(),
            EntryDecision::Duplicate => {
                self.enter_manager();
                let text = format!(
                    "Another client is entering {} · joined without replay",
                    self.world.surfaces.vocabulary.construct
                );
                self.set_notice(&text, Tone::Secondary);
            }
            EntryDecision::Unknown(e) => {
                self.enter_manager();
                self.set_notice(
                    &format!(
                        "Could not confirm running instances: {} · list may be incomplete",
                        self.world.surfaces.vocabulary.discovery(e)
                    ),
                    Tone::Warning,
                );
            }
        }

        match entry.page {
            EntryPage::Manager => {}
            EntryPage::Accounts => self.go(Go::Accounts { select: None }),
            EntryPage::Roles { select } => self.go(Go::Roles { select }),
            EntryPage::RoleEditor { role } => self.go(Go::RoleEditor { role }),
            EntryPage::Launch(target) => {
                self.go(Go::LaunchMany {
                    workspace: target.workspace,
                    role: target.role,
                    instances: target.instances,
                    task: target.task,
                });
                if frame > 0 {
                    let (_, reqs) = self.shell.deliver(true, |cx| {
                        if let Some(c) = self.router.cockpit_mut() {
                            c.seek(frame, &mut self.world, cx);
                        }
                        Outcome::Ignored
                    });
                    self.apply_requests(reqs, Route::Cockpit);
                }
            }
            EntryPage::Attach { instance, pane } => self.go(Go::Attach { instance, pane }),
        }
    }

    fn enter_manager(&mut self) {
        self.show_tab(TAB_MANAGER);
        self.on_route(Route::Manager, Outcome::Ignored, |s, w, cx| {
            s.enter(w, cx);
            Outcome::Ignored
        });
    }

    pub fn route(&self) -> Route {
        if self.outro.is_some() {
            Route::Outro
        } else if self.intro.is_some() {
            Route::Intro
        } else if self.handoff.is_some() {
            Route::Handoff
        } else {
            self.router.current().route()
        }
    }

    fn show_tab(&mut self, tab: usize) {
        self.intro = None;
        self.handoff = None;
        self.router.select(TAB_MANAGER);
        self.router.pop_to(0);
        self.router.select(tab);
    }

    fn open_page(&mut self, page: Page) {
        self.show_tab(TAB_MANAGER);
        self.router.push(page);
        self.enter_route();
    }

    pub fn set_status(&mut self, s: &str, tone: Tone) {
        self.post_status(s, tone, false);
    }

    fn set_notice(&mut self, s: &str, tone: Tone) {
        self.post_status(s, tone, true);
    }

    fn post_status(&mut self, s: &str, tone: Tone, notice: bool) {
        if tone != Tone::Error
            && let Some(warning) = self.world.committed_save_warning
        {
            if s.contains(warning.status()) {
                self.world.committed_save_status = Some(s.to_owned());
            }
            self.show_committed_save_warning();
            return;
        }
        let now = self.world.clock.now();
        if notice {
            self.shell.status.notice_at(s, tone, now);
        } else {
            self.shell.status.set_at(s, tone, now);
        }
    }

    fn show_committed_save_warning(&mut self) {
        if let Some(warning) = self.world.committed_save_warning {
            self.shell.status.sticky_at(
                self.world
                    .committed_save_status
                    .as_deref()
                    .unwrap_or_else(|| warning.status()),
                self.world.clock.now(),
            );
        }
    }

    /// Whether something on screen moves by itself, so every tick needs a
    /// frame. Quiet panes do not: a link that has nothing to say is polled, not
    /// drawn.
    fn animating(&self) -> bool {
        matches!(
            self.route(),
            Route::Intro | Route::Outro | Route::Handoff | Route::Cockpit
        ) || self.shell.pointer.has_flash()
            || self
                .router
                .screen(self.route())
                .is_some_and(|s| s.animating(&self.world))
            || self.layer_animates()
            || !self.world.jobs.is_empty()
    }

    /// Whether a live link is waiting to be polled. Polling keeps its cadence
    /// whether or not anything is drawn.
    fn polls_links(&self) -> bool {
        self.world.daemons.values().any(|d| !d.panes.is_empty())
    }

    pub fn tick_interval(&self) -> std::time::Duration {
        if self.shell.motion == Motion::Paused {
            return std::time::Duration::from_millis(500);
        }
        std::time::Duration::from_millis(
            self.route().tick_ms(self.animating() || self.polls_links()),
        )
    }

    /// Whether the page on screen draws something of this instance's panes.
    fn shows_panes_of(&self, id: &InstanceId) -> bool {
        match self.route() {
            Route::Capsule => self.router.capsule().is_some_and(|c| c.instance == *id),
            Route::Manager => matches!(
                &self.router.manager().selected,
                crate::screens::manager::RowKey::Instance(selected) if selected == id
            ),
            _ => false,
        }
    }

    fn now_ms(&self) -> u64 {
        self.world.now_ms().max(0) as u64
    }

    fn blur_all(&mut self, ch: FocusChange) -> Outcome {
        let mut out = self.layers.on_blur(ch);
        {
            out = self.settle_layers(out);

            for route in self.router.routes() {
                out =
                    out.or(self.on_route(route, Outcome::Ignored, |s, w, cx| s.on_blur(ch, w, cx)));
            }
        }
        out
    }

    fn handle_input(&mut self, input: Input) -> Outcome {
        if matches!(&input, Input::Key(_) | Input::Paste(_))
            || matches!(&input, Input::Mouse(mouse) if !matches!(mouse.kind, MouseKind::Move))
        {
            self.world.dismiss_committed_save_warning();
        }
        let out = match input {
            Input::Resize(..) => Outcome::Changed,
            Input::Tick => self.tick_step(),
            Input::Mouse(m) => PointerRouter::route(self, m),
            Input::External(external) => self.on_external(&external),
            other => shell::KeyRouter::input(self, other),
        };
        out.or(self.deliver_due())
    }

    /// Delivers the replies that are due now, and the ones they start, so an
    /// effect that answers at once is finished before the next frame.
    fn deliver_due(&mut self) -> Outcome {
        self.send_op_requests();
        let mut out = Outcome::Ignored;
        while let Some(m) = self.world.take_due() {
            out = out.or(self.dispatch_msg(&m));
        }
        out
    }

    /// Sends the listing each open 1Password picker wants, in the same step
    /// as the key or click that made it want one.
    fn send_op_requests(&mut self) {
        let asked: Vec<_> = self
            .layers
            .iter_mut()
            .filter_map(|e| e.get_mut::<OpFlow>())
            .filter_map(|flow| flow.take_request().map(|scope| (flow, scope)))
            .map(|(flow, scope)| {
                let ticket = self.world.next_ticket();
                flow.sent(ticket);
                Effect::LoadOp { ticket, scope }
            })
            .collect();
        for effect in asked {
            self.world.perform(effect);
        }
    }

    fn on_tick(&mut self) -> Outcome {
        let out = self.tick_step();
        out.or(self.deliver_due())
    }

    fn tick_step(&mut self) -> Outcome {
        let out = self.step_world_and_page();
        if out == Outcome::Changed {
            self.world.projection.work.count_changed_tick();
        }
        out
    }

    fn step_world_and_page(&mut self) -> Outcome {
        let interval = self.route().tick_ms(true) as i64;
        let tick = self.world.tick_with_changes(interval);
        self.held.extend(tick.messages);
        let mut out = if self.animating()
            || !tick.summaries_changed.is_empty()
            || tick.panes_changed.iter().any(|id| self.shows_panes_of(id))
        {
            Outcome::Changed
        } else {
            Outcome::Ignored
        };
        let now = self.now_ms();
        if self.shell.pointer.expire_flash(now) {
            out = Outcome::Changed;
        }

        out = out.or(PointerRouter::tick(self));
        if self.shell.status.expire(self.world.clock.now()) {
            out = Outcome::Changed;
        }
        if self.shell.status.get().is_none() {
            self.show_committed_save_warning();
        }
        if let Some(at) = self.exit_after_status
            && self.world.now_ms() >= at
        {
            self.shell.quit = true;
            return Outcome::Changed;
        }
        if self.intro_guard > 0 {
            self.intro_guard -= 1;
        }
        match self.route() {
            Route::Intro => {
                if let Some(s) = self.intro.as_mut() {
                    s.on_tick();
                    if s.is_done() {
                        self.finish_intro();
                    }
                }
                return Outcome::Changed;
            }
            Route::Outro => {
                if let Some(s) = self.outro.as_mut() {
                    s.on_tick();
                    if s.is_done() {
                        self.shell.quit = true;
                    }
                }
                return Outcome::Changed;
            }
            Route::Handoff => {
                if self.world.clock.running {
                    let h = self.handoff.unwrap_or(0) + 1;
                    self.handoff = Some(h);
                    if h >= HANDOFF_LEN {
                        self.finish_handoff();
                    }
                }
                return Outcome::Changed;
            }
            _ => {}
        }

        if let Some(top) = self.layers.top_mut() {
            let o = top.layer.on_tick(&self.world);
            out = self.settle_layers(out.or(o));
        }

        let route = self.route();
        out = out.or(self.on_route(route, Outcome::Ignored, |s, w, cx| s.on_tick(w, cx)));

        for m in std::mem::take(&mut self.held) {
            out = out.or(self.dispatch_msg(&m));
        }
        out
    }

    fn dispatch_msg(&mut self, m: &Msg) -> Outcome {
        if let Msg::PaneWake { instance, pane } = m {
            let roster = self.world.epochs().roster;
            let changed = self.world.wake_pane(instance, *pane);
            return if changed
                && (self.shows_panes_of(instance) || self.world.epochs().roster != roster)
            {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }
        if let Msg::OpLoaded { ticket, result } = m {
            let taken = self
                .layers
                .iter_mut()
                .filter_map(|e| e.get_mut::<OpFlow>())
                .any(|flow| flow.loaded(*ticket, result));
            return if taken {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }
        let applied = self.world.snapshots_applied;
        let out = self.dispatch_reply(m);
        if self.world.snapshots_applied != applied {
            self.after_snapshot();
        }
        out
    }

    fn dispatch_reply(&mut self, m: &Msg) -> Outcome {
        let shown_validation = self.world.displayed_validation.clone();
        let owns_status = match m {
            Msg::AccountValidated {
                ticket, account, ..
            } => self.world.validation_owns_status(account, *ticket),
            _ => false,
        };
        let absorbed = self.world.absorb(m);
        self.show_committed_save_warning();
        if absorbed == Absorbed::Stale {
            return Outcome::Ignored;
        }
        self.tell_forms_of_reply(m);
        let validation = matches!(m, Msg::AccountValidated { .. });
        let mut ended = !validation
            && m.job()
                .is_some_and(|job| !self.job_continues(m) && self.shell.status.end_job(job));
        let route = self.route();
        let (o, reqs) = self
            .shell
            .deliver(true, |cx| match self.router.screen_mut(route) {
                Some(s) => s.on_msg(m, &mut self.world, cx),
                None => Outcome::Ignored,
            });

        let (o, behind) = self.shell.deliver(false, |cx| {
            let mut o = o;
            if !o.consumed() && route != Route::Manager {
                o = self.router.manager_mut().on_msg(m, &mut self.world, cx);
            }
            if !o.consumed() && route != Route::Accounts {
                o = self.router.accounts_mut().on_msg(m, &mut self.world, cx);
            }
            if !o.consumed()
                && route != Route::Roles
                && let Some(roles) = self.router.roles_mut()
            {
                o = roles.on_msg(m, &mut self.world, cx);
            }
            o
        });
        if owns_status {
            self.world.displayed_validation = None;
        }
        ended |= self.end_retired_validation(shown_validation);
        let o = o.or(self.apply_requests(reqs, route));
        let o = o.or(self.apply_requests(behind, route));
        if ended { o.or(Outcome::Changed) } else { o }
    }

    /// Ends the validation progress when the request it showed is no longer
    /// displayed: its reply was accepted, or an installed snapshot retired it.
    fn end_retired_validation(
        &mut self,
        shown_before: Option<(crate::domain::account::AccountId, Ticket)>,
    ) -> bool {
        shown_before.is_some()
            && self.world.displayed_validation.is_none()
            && self.shell.status.end_job(job::ACCOUNT_VALIDATE)
    }

    fn job_continues(&self, m: &Msg) -> bool {
        self.router.pages().any(|p| p.screen().job_continues(m))
    }

    fn finish_intro(&mut self) {
        self.world.arbiter.complete_entry(self.world.now_ms());
        self.intro = None;
        self.enter_manager();
    }

    fn finish_handoff(&mut self) {
        self.handoff = None;

        if let Some(capsule) = self.router.back() {
            self.router.replace(capsule);
        }
        self.on_route(Route::Capsule, Outcome::Ignored, |s, w, cx| {
            s.enter(w, cx);
            Outcome::Ignored
        });
    }

    fn sequence_key(&mut self, key: &Key) -> Option<Outcome> {
        match self.route() {
            Route::Intro => {
                if self.intro_guard > 0 && self.shell.motion != Motion::Paused {
                    return Some(Outcome::Consumed);
                }
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    if let Some(s) = self.intro.as_mut() {
                        s.skip();
                        if s.is_done() {
                            self.finish_intro();
                        }
                    }
                    return Some(Outcome::Changed);
                }
                if key.ctrl_char('c') {
                    self.world.arbiter.release_entry();
                    self.shell.quit = true;
                }
                Some(Outcome::Consumed)
            }
            Route::Outro => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    if let Some(s) = self.outro.as_mut() {
                        s.skip();
                        if s.is_done() {
                            self.shell.quit = true;
                        }
                    }
                    return Some(Outcome::Changed);
                }
                if key.ctrl_char('c') {
                    self.shell.quit = true;
                }
                Some(Outcome::Consumed)
            }
            Route::Handoff => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    self.finish_handoff();
                }
                Some(Outcome::Changed)
            }
            _ => None,
        }
    }

    fn host_editing(&self) -> bool {
        self.router
            .screen(self.route())
            .is_some_and(|s| s.is_editing() || s.layer_open())
    }

    fn leave_key(&mut self) -> Outcome {
        let route = self.route();
        let dirty = self
            .router
            .screen_mut(route)
            .is_some_and(|s| !s.stakes(&self.world).is_clean());
        if dirty {
            self.set_status("Unsaved changes · save, or Esc to leave", Tone::Secondary);
        } else if route == Route::Manager {
            return self.request_quit();
        } else {
            self.go(Go::Manager);
        }
        Outcome::Changed
    }

    fn request_quit(&mut self) -> Outcome {
        let stakes = shell::App::stakes(self);
        match self
            .shell
            .quit_through(&mut self.quit_guard, &stakes, &self.theme.strings)
        {
            Ok(()) => {
                self.world.arbiter.release_entry();
                Outcome::Consumed
            }

            Err(_) => {
                let route = self.route();
                self.on_route(route, Outcome::Ignored, |s, w, cx| s.request_leave(w, cx))
            }
        }
    }

    fn quit_now(&mut self) {
        self.world.arbiter.release_entry();
        self.shell.quit = true;
    }

    fn is_host(route: Route) -> bool {
        matches!(
            route,
            Route::Manager
                | Route::Accounts
                | Route::Usage
                | Route::Roles
                | Route::RoleEditor
                | Route::Doctor
                | Route::Settings
                | Route::Editor
                | Route::Prelude
        )
    }

    fn roots(route: Route) -> bool {
        matches!(
            route,
            Route::Manager | Route::Accounts | Route::Usage | Route::Roles
        )
    }

    fn build_host_menu(route: Route, world: &World) -> Menus<HostCmd> {
        use HostCmd::{Press, Quit};
        let k = |c: char| Press(plain(KeyCode::Char(c)));
        let file: Vec<(MenuItem, HostCmd)> = match route {
            Route::Manager => vec![
                (MenuItem::new("New workspace…").shortcut("n"), k('n')),
                (MenuItem::new("Edit workspace…").shortcut("e"), k('e')),
                (
                    MenuItem::new("Launch…").shortcut("Enter"),
                    Press(plain(KeyCode::Enter)),
                ),
                (MenuItem::new("Prewarm").shortcut("w"), k('w')),
                (
                    MenuItem::new("Delete workspace…")
                        .shortcut("d")
                        .danger()
                        .separator(),
                    k('d'),
                ),
                (
                    MenuItem::new("Refresh").shortcut("F5").separator(),
                    Press(plain(KeyCode::F(5))),
                ),
                (MenuItem::new("Quit").shortcut("Ctrl+Q"), Quit),
            ],
            Route::Editor | Route::Settings => vec![
                (
                    MenuItem::new("Save…").shortcut("Ctrl+S").separator(),
                    Press(Key {
                        code: KeyCode::Char('s'),
                        mods: KeyModifiers::CONTROL,
                    }),
                ),
                (
                    MenuItem::new("Cancel").shortcut("Esc"),
                    Press(plain(KeyCode::Esc)),
                ),
            ],
            Route::Accounts => vec![
                (MenuItem::new("Add account…").shortcut("a"), k('a')),
                (MenuItem::new("Edit account…").shortcut("e"), k('e')),
                (MenuItem::new("Validate").shortcut("v").separator(), k('v')),
                (MenuItem::new("Refresh selection").shortcut("r"), k('r')),
                (
                    MenuItem::new("Refresh all").shortcut("F5"),
                    Press(plain(KeyCode::F(5))),
                ),
                (MenuItem::new("Filter…").shortcut("/"), k('/')),
            ],
            Route::Usage => vec![
                (MenuItem::new("Refresh").shortcut("r"), k('r')),
                (MenuItem::new("Manage in Accounts").shortcut("m"), k('m')),
            ],
            Route::Roles => vec![
                (MenuItem::new("Add role…").shortcut("a"), k('a')),
                (MenuItem::new("Edit manifest…").shortcut("e"), k('e')),
                (MenuItem::new("Check").shortcut("v").separator(), k('v')),
                (
                    MenuItem::new("Check all").shortcut("F5"),
                    Press(plain(KeyCode::F(5))),
                ),
                (MenuItem::new("Filter…").shortcut("/"), k('/')),
            ],
            Route::RoleEditor => vec![
                (
                    MenuItem::new("Save…").shortcut("Ctrl+S").separator(),
                    Press(Key {
                        code: KeyCode::Char('s'),
                        mods: KeyModifiers::CONTROL,
                    }),
                ),
                (
                    MenuItem::new("Check").shortcut("F5"),
                    Press(plain(KeyCode::F(5))),
                ),
                (
                    MenuItem::new("Cancel").shortcut("Esc"),
                    Press(plain(KeyCode::Esc)),
                ),
            ],
            Route::Doctor => vec![
                (MenuItem::new("Run checks").shortcut("r"), k('r')),
                (MenuItem::new("Copy report").shortcut("y"), k('y')),
            ],
            _ => vec![(
                MenuItem::new("Cancel").shortcut("Esc"),
                Press(plain(KeyCode::Esc)),
            )],
        };
        let mut go = vec![
            (
                MenuItem::new("Workspace manager").shortcut("Esc"),
                HostCmd::Manager,
            ),
            (
                MenuItem::new("Account & Usage Center").shortcut("c"),
                HostCmd::Accounts,
            ),
            (MenuItem::new("Usage").shortcut("u"), HostCmd::Usage),
            (
                MenuItem::new("Global settings").shortcut("s"),
                HostCmd::Settings,
            ),
        ];
        if world.surfaces.roles.is_some() {
            go.push((MenuItem::new("Roles").shortcut("R"), HostCmd::Roles));
        }
        if world.surfaces.doctor.is_some() {
            go.push((MenuItem::new("Doctor").shortcut("D"), HostCmd::Doctor));
        }
        let help = vec![
            (MenuItem::new("Key reference").shortcut("?"), HostCmd::Help),
            (
                MenuItem::new(&world.surfaces.vocabulary.about_label),
                HostCmd::About,
            ),
        ];
        Menus::new()
            .menu("File", file)
            .menu("Go", go)
            .menu("Help", help)
    }

    fn host_bar(menus: &Menus<HostCmd>) -> MenuBar {
        menus.bar(HOST_MENU).brand(Lockup::new(BRAND_MARK))
    }

    fn sync_host_menu(&mut self) {
        if self.host_menu_route != self.route() && Self::is_host(self.route()) {
            self.host_menus = Self::build_host_menu(self.route(), &self.world);
            self.host_menu = Self::host_bar(&self.host_menus);
            self.host_menu_route = self.route();
        }
    }

    fn host_menu_action(&mut self, action: MenuBarAction, o: Outcome) -> Outcome {
        if let MenuBarAction::Brand = action {
            self.open_about();
            return Outcome::Changed;
        }
        match self.host_menus.command(action) {
            Some(cmd) => self.run_host_cmd(cmd),
            None => o,
        }
    }

    fn run_host_cmd(&mut self, cmd: HostCmd) -> Outcome {
        let editing_screen = matches!(
            self.route(),
            Route::Editor | Route::Settings | Route::Prelude | Route::RoleEditor
        );
        match cmd {
            HostCmd::Manager if editing_screen => {
                return shell::KeyRouter::replay(self, &plain(KeyCode::Esc));
            }
            HostCmd::Manager => self.go(Go::Manager),
            HostCmd::Accounts
            | HostCmd::Usage
            | HostCmd::Settings
            | HostCmd::Roles
            | HostCmd::Doctor
                if editing_screen =>
            {
                self.set_status("Save or cancel this screen first", Tone::Secondary);
            }
            HostCmd::Accounts => self.go(Go::Accounts { select: None }),
            HostCmd::Usage => self.go(Go::Usage { select: None }),
            HostCmd::Settings => self.go(Go::Settings),
            HostCmd::Roles => self.go(Go::Roles { select: None }),
            HostCmd::Doctor => self.go(Go::Doctor),
            HostCmd::Help => self.open_help(),
            HostCmd::About => self.open_about(),
            HostCmd::Quit => return self.request_quit(),
            HostCmd::Press(key) => return shell::KeyRouter::replay(self, &key),
        }
        Outcome::Changed
    }

    fn open_about(&mut self) {
        let props = self.world.about_props();
        let d = crate::screens::modals::InfoDialog::new(WidgetId::of("host.about"), "About", props)
            .width(64);
        self.open_layer(Box::new(d), LayerTag::new("about"), self.route());
    }

    fn draw_host_menu(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, route: Route) {
        let t = self.theme;
        self.sync_host_menu();
        self.host_menu.render(area, buf, ctx, t.canvas);
        let used = self
            .host_menu
            .areas
            .iter()
            .map(|r| r.right())
            .max()
            .unwrap_or(area.x)
            .max(self.host_menu.brand_area.right());
        let rest = Rect::new(used + 2, area.y, area.right().saturating_sub(used + 2), 1);
        let (state, tone) = self.construct_state(route);
        let mut right = vec![];
        if let Some(s) = self.router.screen(route) {
            right.push(Segment::new(s.crumb(&self.world), Tone::Secondary).priority(7));
            right.extend(s.strip_right(&self.world));
        }
        right.push(Segment::new(state, tone).priority(6));
        match self.world.arbiter.running() {
            Ok(0) => right.push(Segment::new("no instances", Tone::Muted).priority(5)),
            Ok(n) => right.push(
                Segment::new(format!("{n} running"), Tone::Secondary)
                    .lead(self.theme.glyphs.dot_on, Tone::Active)
                    .priority(5),
            ),
            Err(e) => right.push(
                Segment::new(
                    format!("! {}", self.world.surfaces.vocabulary.discovery(e)),
                    Tone::Error,
                )
                .priority(8),
            ),
        }
        if let Some(s) = self.health_segment() {
            right.push(s);
        }
        if !self.world.jobs.is_empty() {
            right.push(
                Segment::new(
                    format!(
                        "{} working",
                        junie_tui::widgets::progress::spinner_frame(ctx.interaction.tick)
                    ),
                    Tone::Secondary,
                )
                .priority(4),
            );
        }
        segments::render(rest, buf, ctx, &[], &right, t.canvas);
    }

    fn health_segment(&self) -> Option<Segment> {
        let hv = self.world.health_view();
        let (word, tone) = match hv.health {
            Health::Healthy => return None,
            Health::Stale => ("stale", Tone::Warning),
            Health::Failing => ("failing", Tone::Error),
        };
        Some(Segment::new(format!("▲ {} {word}", hv.noun), tone).priority(8))
    }

    fn open_help(&mut self) {
        if !self.layers.is_empty() {
            self.set_status("Close the dialog first", Tone::Secondary);
            return;
        }
        let scope = self
            .router
            .screen(self.route())
            .map(|s| s.crumb(&self.world))
            .unwrap_or_default();
        let capsule = self.world.surfaces.vocabulary.capsule.clone();
        let info_hint = self.world.surfaces.vocabulary.info_label().to_lowercase();
        let inspect_hint = format!("inspect {}", self.world.surfaces.vocabulary.noun_lower());
        let project = self.world.surfaces.vocabulary.project_tab.clone();
        let sections: Vec<(&str, Vec<(&str, &str)>)> = match self.route() {
            Route::Capsule => vec![
                (
                    capsule.as_str(),
                    vec![
                        ("F10", "menu bar"),
                        ("right-click", "tab / pane menu"),
                        ("Ctrl+B", "pane commands…"),
                        ("Ctrl+\\", "command palette"),
                        ("Ctrl+Q", "quit"),
                        ("Alt+Shift+↑↓←→", "resize split"),
                        ("click", "focus pane"),
                        ("wheel", "scrollback"),
                        ("drag", "select text"),
                        ("2×click", "select word / rename tab"),
                    ],
                ),
                (
                    "Prefix commands",
                    vec![
                        ("c", "new tab"),
                        ("n p", "next / previous tab"),
                        ("0–9", "jump to tab"),
                        ("x", "close pane"),
                        ("&", "close tab"),
                        ("\" %", "split below / right"),
                        ("z", "zoom"),
                        ("h j k l", "move focus"),
                        ("H J K L", "resize split"),
                        ("Ctrl+L", "clear pane"),
                        ("d", "detach"),
                        ("u", "Usage"),
                        (",", "change tab title"),
                        ("m", "tab menu"),
                        ("Space :", "palette"),
                        ("r", "redraw"),
                    ],
                ),
                (
                    "Inspect changes",
                    vec![
                        ("View › Inspect changes", "open"),
                        ("Enter", "open a file's diff"),
                        ("m F2", "compact / advanced"),
                        ("v", "unified / side by side"),
                        ("Tab", "tree / diff"),
                    ],
                ),
                (
                    "Scrollback",
                    vec![
                        ("↑↓", "scroll"),
                        ("PgUp PgDn", "page"),
                        ("Home End", "oldest / live"),
                        ("Esc", "back to live"),
                        ("y", "copy selection"),
                    ],
                ),
            ],
            Route::Cockpit => vec![
                (
                    "Launch",
                    vec![
                        ("b", "build log"),
                        ("i", &info_hint),
                        ("x", "cancel launch…"),
                        ("t", "toggle debug"),
                        ("Ctrl+Q", "quit · asks only about unsaved edits"),
                        ("Ctrl+C", "hard abort"),
                    ],
                ),
                (
                    "Build log",
                    vec![
                        ("↑↓ j k", "scroll"),
                        ("PgUp PgDn", "page"),
                        ("End", "follow tail"),
                        ("Esc", "close"),
                    ],
                ),
            ],
            Route::Accounts => vec![
                (
                    "Accounts",
                    vec![
                        ("↑↓ j k", "move"),
                        ("←→ h l", "fold provider / back"),
                        ("Enter", "details"),
                        ("Tab", "inspector → actions"),
                        ("a", "add account…"),
                        ("e", "edit…"),
                        ("Space", "set provider default"),
                        ("t", "enable / disable"),
                        ("v", "validate"),
                        ("d", "remove…"),
                        ("r", "refresh selection"),
                        ("F5", "refresh all"),
                        ("/", "filter"),
                        ("m", "Usage overlay"),
                    ],
                ),
                (
                    "Credential sources",
                    vec![
                        (
                            "1Password",
                            "item / field reference · resolved at launch, never stored",
                        ),
                        ("Local folder", "the agent's own login folder"),
                        (
                            "Plain-text",
                            "typed once · masked · fingerprint + 4-char tail kept",
                        ),
                    ],
                ),
            ],
            Route::Usage => vec![
                (
                    "Usage",
                    vec![
                        ("↑↓ j k", "move"),
                        ("Enter", "detail"),
                        ("r", "refresh every account"),
                        ("m", "manage in Accounts"),
                        ("Esc", "close"),
                    ],
                ),
                (
                    "Reading meters",
                    vec![
                        ("━", "used share of the window"),
                        ("▲", "warning ≥ 75 %"),
                        ("!", "exhausted · error"),
                        ("stale", "last good value kept, dimmed"),
                    ],
                ),
            ],
            Route::Roles => vec![
                (
                    "Roles",
                    vec![
                        ("↑↓ j k", "move"),
                        ("Enter", "details"),
                        ("e", "edit manifest…"),
                        ("a", "add role…"),
                        ("d", "remove…"),
                        ("v", "check"),
                        ("F5", "check all"),
                        ("/", "filter"),
                    ],
                ),
                (
                    "Account bindings",
                    vec![
                        ("Enter", "pick an account…"),
                        ("d", "clear the binding"),
                        ("u", "undo"),
                        ("Tab", "actions"),
                        ("← Esc", "back to the list"),
                    ],
                ),
            ],
            Route::RoleEditor => vec![
                (
                    "Tabs",
                    vec![
                        ("←→ 1–3", "switch tab"),
                        ("[ ]", "switch from anywhere"),
                        ("Enter ↓", "into the tab body"),
                        ("Esc", "back to tabs · leave"),
                    ],
                ),
                (
                    "Manifest · Hooks",
                    vec![
                        ("Enter", "edit field"),
                        ("Choose…", "pick a repository file"),
                        ("Space", "toggle Docker"),
                        ("i", "Docker risks"),
                    ],
                ),
                (
                    "Environment",
                    vec![
                        ("a", "add entry…"),
                        ("e Enter", "edit…"),
                        ("d", "remove"),
                        ("u", "undo"),
                    ],
                ),
                (
                    "Check and save",
                    vec![
                        ("F5", "run the role check"),
                        ("Ctrl+S", "preview then save"),
                        ("Esc", "back · asks when dirty"),
                    ],
                ),
            ],
            Route::Doctor => vec![(
                "Doctor",
                vec![
                    ("↑↓ j k", "move"),
                    ("Enter", "details"),
                    ("r F5", "run the checks"),
                    ("y", "copy the report"),
                    ("Esc", "back"),
                ],
            )],
            Route::Editor => vec![
                (
                    "Tabs",
                    vec![
                        ("←→ h l 1–5", "switch tab"),
                        ("[ ]", "switch from anywhere"),
                        ("Enter ↓", "into the tab body"),
                        ("Esc", "back to tabs · leave"),
                    ],
                ),
                (
                    "General",
                    vec![
                        ("Enter", "edit field"),
                        ("Esc", "cancel the edit"),
                        ("Space", "toggle"),
                        ("Choose…", "working directory picker"),
                    ],
                ),
                (
                    project.as_str(),
                    vec![
                        ("Enter e", "edit…"),
                        ("r", "read-only"),
                        ("i 1 2 3", "isolation"),
                        ("o", "open source"),
                        ("d", "remove (u restores)"),
                        ("a", "add…"),
                    ],
                ),
                (
                    "Roles",
                    vec![
                        ("Space", "allow / disallow"),
                        ("Enter", "set default ★"),
                        ("a", "load role…"),
                    ],
                ),
                (
                    "Environments · Auth",
                    vec![
                        ("Enter e", "edit…"),
                        ("m", "show / mask"),
                        ("p", "re-pick from 1Password"),
                        ("s", "scope…"),
                        ("Space", "cycle auth mode"),
                        ("d", "remove · auth back to sync"),
                        ("u", "undo row"),
                    ],
                ),
                (
                    "Save",
                    vec![
                        ("Ctrl+S", "preview then save"),
                        ("Esc", "back · asks when dirty"),
                    ],
                ),
            ],
            Route::Settings => vec![
                (
                    "Tabs",
                    vec![
                        ("←→ h l 1–5", "switch tab"),
                        ("[ ]", "switch from anywhere"),
                        ("Enter ↓", "into the tab body"),
                        ("Esc", "back to tabs · leave"),
                    ],
                ),
                (
                    "Mounts · Environments · Auth",
                    vec![
                        ("Enter e", "edit…"),
                        ("s", "scope global ⇄ role"),
                        ("r i", "read-only · isolation"),
                        ("m p", "show · 1Password"),
                        ("Space", "cycle auth mode"),
                        ("d", "remove · auth back to sync"),
                        ("u", "undo row"),
                        ("c", "manage accounts (Auth)"),
                    ],
                ),
                (
                    "Trust",
                    vec![
                        ("Space", "trust / untrust a role source"),
                        ("o", "open the source"),
                    ],
                ),
                (
                    "Save",
                    vec![
                        ("Ctrl+S", "preview then save"),
                        ("Esc", "back · asks when dirty"),
                    ],
                ),
            ],
            Route::Prelude => vec![(
                "New workspace",
                vec![
                    ("Enter", "open / choose"),
                    ("Backspace", "up one directory"),
                    ("Space", "choose directory"),
                    ("g", "Git URL…"),
                    ("Tab", "next control"),
                    ("Esc", "back one step · cancel"),
                ],
            )],
            _ => vec![
                (
                    "Workspaces",
                    vec![
                        ("↑↓ j k", "move"),
                        ("←→ h l", "collapse / expand"),
                        ("Space", "fold workspace"),
                        ("Enter", "launch"),
                        ("e", "edit workspace"),
                        ("n", "new workspace"),
                        ("d", "delete…"),
                        ("w", "prewarm"),
                        ("o", "open in GitHub"),
                        ("* -", "expand / collapse all"),
                        ("Tab", "details"),
                    ],
                ),
                (
                    "Instances",
                    vec![
                        ("Enter r", "reconnect / restore"),
                        ("a", "new session"),
                        ("x", "open shell"),
                        ("i", &inspect_hint),
                        ("t", "stop"),
                        ("p", "purge…"),
                    ],
                ),
                ("Everywhere", vec![("F5", "refresh now")]),
                (
                    "Editor and Settings",
                    vec![
                        ("←→ 1–5 [ ]", "switch tab"),
                        ("Ctrl+S", "save (preview first)"),
                        ("Space", "toggle / cycle"),
                        ("a e d", "add / edit / remove"),
                        ("m p s", "mask · 1Password · scope"),
                    ],
                ),
                (
                    "Mouse",
                    vec![
                        ("click", "select · 2× activates"),
                        ("wheel", "scroll under pointer"),
                        ("drag", "seam · scrollbar thumb"),
                    ],
                ),
            ],
        };

        let mut sections: Vec<junie_tui::widgets::help::HelpSection> = sections
            .into_iter()
            .map(|(title, rows)| {
                rows.into_iter().fold(
                    junie_tui::widgets::help::HelpSection::new(title),
                    |s, (k, v)| s.row(k, v),
                )
            })
            .collect();
        sections.push(self.keys.help_section("Shell", |c| {
            Some(match c {
                JackinCmd::Usage => "Usage",
                JackinCmd::Accounts => "Accounts",
                JackinCmd::Settings => "Settings",
                JackinCmd::Roles => "Roles",
                JackinCmd::Doctor => "Doctor",
                JackinCmd::Leave => "Leave this screen",
            })
        }));
        let h = HelpOverlay::from_keymaps(WidgetId::of("help"), &scope, sections);
        self.open_layer(Box::new(h), LayerTag::new("help"), self.route());
    }

    fn open_layer(&mut self, layer: Box<AppLayer>, tag: LayerTag, owner: Route) {
        self.layers.open(layer, tag, owner, &mut self.shell.focus);
        self.shell.pointer.hover = None;
        self.shell.pointer.pressed = None;
        if let Some(c) = self.router.capsule_mut() {
            c.dialog_open = true;
            c.sync_focus(&mut self.world);
        }
    }

    fn layer_gone(&mut self) {
        if self.layers.is_empty()
            && let Some(c) = self.router.capsule_mut()
        {
            c.dialog_open = false;
            c.sync_focus(&mut self.world);
        }
    }

    fn settle_layers(&mut self, out: Outcome) -> Outcome {
        Layers::settle(
            self,
            |app| (&mut app.layers, &mut app.shell.focus),
            out,
            |app, taken| match taken {
                Taken::Event(r) => app.layer_event(r),
                Taken::Closed(entry, r) => {
                    app.layer_gone();
                    app.layer_closed(entry, r)
                }
            },
        )
    }

    fn layer_event(&mut self, r: ModalResult) -> Outcome {
        let Some(top) = self.layers.top() else {
            return Outcome::Ignored;
        };
        let (tag, owner) = (top.tag.clone(), top.owner);
        let r = match r {
            ModalResult::PickerEvent(PickerAction::QueryChanged) => {
                self.refresh_picker();
                return Outcome::Changed;
            }
            ModalResult::FormEvent(FormAction::Changed(_)) => {
                self.form_changed();
                return Outcome::Changed;
            }
            ModalResult::FormEvent(FormAction::Choose(name)) => {
                ModalResult::FormAction(format!("choose:{name}"), self.top_form_values())
            }
            ModalResult::FormEvent(FormAction::Action(name)) => {
                ModalResult::FormAction(name, self.top_form_values())
            }
            ModalResult::Info(InfoResult::Copy(v)) => {
                self.copy(v, None);
                return Outcome::Changed;
            }
            r => r,
        };
        let scope = r == ModalResult::Scope;
        let o = self.tell_owner(owner, &tag, r);
        if scope {
            self.refresh_picker();
        }
        o.or(Outcome::Changed)
    }

    fn layer_closed(&mut self, entry: ClosedLayer, r: ModalResult) -> Outcome {
        let cancel_index = entry.get::<Dialog>().and_then(|d| d.cancel_index);
        let r = match r {
            ModalResult::Help => return Outcome::Changed,

            ModalResult::FormEvent(FormAction::Save) => ModalResult::Form(Some(
                entry
                    .get::<FormDialog>()
                    .map(FormDialog::values)
                    .unwrap_or_default(),
            )),

            ModalResult::Dialog { action: None, text } => ModalResult::Dialog {
                action: cancel_index,
                text,
            },
            r => r,
        };
        self.tell_owner(entry.owner, &entry.tag, r)
    }

    fn tell_owner(&mut self, owner: Route, tag: &LayerTag, r: ModalResult) -> Outcome {
        self.on_route(owner, Outcome::Changed, |s, w, cx| {
            s.on_layer_result(tag, r, w, cx)
        })
        .or(Outcome::Changed)
    }

    fn top_form_values(&self) -> crate::screens::modals::FormValues {
        self.layers
            .top_as::<FormDialog>()
            .map(FormDialog::values)
            .unwrap_or_default()
    }

    fn copy(&mut self, v: String, what: Option<String>) {
        let now = self.world.clock.now();
        self.shell.copy_at(&v, what.as_deref(), now);
        self.world.clipboard = Some(v);
        self.clipboard_gen += 1;
    }

    fn refresh_picker(&mut self) {
        let Some(top) = self.layers.top() else {
            return;
        };
        let Some(p) = top.get::<Picker>() else {
            return;
        };
        let (tag, owner, query) = (top.tag.clone(), top.owner, p.query().to_owned());
        let items = self
            .router
            .screen_mut(owner)
            .and_then(|s| s.picker_items(&tag, &query, &self.world));
        if let Some(items) = items
            && let Some(p) = self.layers.top_as_mut::<Picker>()
        {
            let n = items.len();
            p.set_items(items)
                .unwrap_or_else(|error| p.report_item_error(error));
            if tag.kind == "palette" {
                p.scope = Some(format!("{n} of 20"));
            }
        }
    }

    fn tell_forms_of_reply(&mut self, m: &Msg) {
        for entry in self.layers.iter_mut() {
            let (tag, owner) = (entry.tag.clone(), entry.owner);
            if let Some(form) = entry.get_mut::<FormDialog>()
                && let Some(s) = self.router.screen_mut(owner)
            {
                s.reply_arrives(&tag, form, m);
            }
        }
    }

    fn form_changed(&mut self) {
        let Some(top) = self.layers.top_mut() else {
            return;
        };
        let (tag, owner) = (top.tag.clone(), top.owner);
        let Some(form) = top.get_mut::<FormDialog>() else {
            return;
        };
        if let Some(s) = self.router.screen_mut(owner) {
            s.form_changed(&tag, form, &self.world);
        }
    }

    fn layer_key(&mut self, key: Key) -> Outcome {
        let Some(top) = self.layers.top_mut() else {
            return Outcome::Ignored;
        };
        let mut cx = LayerCx {
            focus: &mut self.shell.focus,
            ring: &self.shell.ring,
            app: &self.world,
        };
        let o = top.layer.on_key(&key, &mut cx);
        self.post_layer_refusal();
        if o == Outcome::Ignored && key.is(KeyCode::Esc) {
            return self.dismiss_layer(DismissReason::Esc);
        }
        self.settle_layers(o.or(Outcome::Consumed))
    }

    fn post_layer_refusal(&mut self) {
        if let Some(reason) = junie_tui::core::refusal::take_refusal() {
            self.set_status(&reason.text, Tone::Secondary);
        }
    }

    fn dismiss_layer(&mut self, reason: DismissReason) -> Outcome {
        let d = self.layers.dismiss(reason, &mut self.shell.focus);
        self.dismissed(reason, d)
    }

    fn dismissed(
        &mut self,
        reason: DismissReason,
        d: Dismissed<ModalResult, World, Route>,
    ) -> Outcome {
        match d {
            Dismissed::None => Outcome::Ignored,
            Dismissed::Refused => {
                let dismiss = self
                    .layers
                    .top()
                    .map(|t| t.layer.spec().dismiss)
                    .unwrap_or_default();
                let now = self.now_ms();
                if reason == DismissReason::OutsideClick
                    && junie_tui::ui::modal::refuse_outside_click(
                        &mut self.shell.pointer,
                        dismiss,
                        now,
                    )
                {
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            Dismissed::Closed(entry, r) => {
                self.layer_gone();
                let o = match r {
                    Some(r) => self.layer_closed(entry, r),
                    None => Outcome::Changed,
                };
                self.settle_layers(o)
            }
        }
    }

    fn layer_animates(&self) -> bool {
        self.layers.top().is_some_and(|t| t.layer.animating())
    }

    fn layer_editing(&self) -> bool {
        self.layers.top().is_some_and(|t| t.layer.is_editing())
    }

    fn deliver_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        if matches!(self.route(), Route::Intro | Route::Outro | Route::Handoff) {
            return if ev.is(Phase::Click) {
                self.skip_animation()
            } else {
                Outcome::Ignored
            };
        }

        if ev.on(HintBar::ID) {
            return match self.shell.footer_keys.clicked(ev) {
                Some(key) => shell::KeyRouter::replay(self, &key).or(Outcome::Changed),
                None => Outcome::Consumed,
            };
        }
        if !self.layers.is_empty() {
            return self.layer_pointer(ev);
        }
        if let Some(o) = self.chrome_pointer(ev) {
            return o;
        }
        self.screen_pointer(ev)
    }

    fn skip_animation(&mut self) -> Outcome {
        if matches!(self.route(), Route::Intro | Route::Outro)
            && self.shell.pointer.config.click_skips_animation
        {
            return shell::KeyRouter::replay(
                self,
                &Key {
                    code: KeyCode::Enter,
                    mods: KeyModifiers::NONE,
                },
            );
        }
        Outcome::Ignored
    }

    fn layer_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        if ev.is(Phase::Click) && ev.id == junie_tui::ui::modal::CLOSE {
            return self.dismiss_layer(DismissReason::CloseButton);
        }
        let handled = self
            .layers
            .pointer(ev, &mut self.shell.focus, &self.shell.ring, &self.world);
        self.post_layer_refusal();
        match handled {
            LayerPointer::Handled(o) => self.settle_layers(o),
            LayerPointer::Dismissed(d) => self.dismissed(DismissReason::OutsideClick, d),
        }
    }

    fn chrome_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        if Self::is_host(self.route()) {
            if let Some(r) = self.host_menu.route(ev) {
                let (mut o, actions) = r.into_parts();
                for a in actions {
                    o = self.host_menu_action(a, o);
                }
                return Some(o);
            }

            if self.host_menu.is_open() && ev.is(Phase::Click) {
                self.host_menu.close();
                return Some(Outcome::Changed);
            }
        }
        if !ev.is(Phase::Click) {
            return None;
        }
        let go = match ev.id {
            STRIP_HELP => {
                self.open_help();
                return Some(Outcome::Changed);
            }
            STRIP_USAGE => Go::Usage { select: None },
            STRIP_ACCOUNTS => Go::Accounts { select: None },
            STRIP_SETTINGS => Go::Settings,
            _ => return None,
        };
        self.go(go);
        Some(Outcome::Changed)
    }

    fn screen_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        let route = self.route();
        self.on_route(route, Outcome::Ignored, |s, w, cx| s.pointer(ev, w, cx))
    }

    fn on_route(
        &mut self,
        route: Route,
        absent: Outcome,
        f: impl FnOnce(&mut dyn Screen, &mut World, &mut Cx) -> Outcome,
    ) -> Outcome {
        let shown = route == self.route();
        let (o, reqs) = self
            .shell
            .deliver(shown, |cx| match self.router.screen_mut(route) {
                Some(s) => {
                    let o = f(s, &mut self.world, cx);
                    crate::screens::post_refusal(cx);
                    o
                }
                None => absent,
            });
        o.or(self.apply_requests(reqs, route))
    }

    fn apply_requests(&mut self, requests: Vec<Request>, owner: Route) -> Outcome {
        let mut out = Outcome::Ignored;
        for r in requests {
            out = Outcome::Changed;
            match r {
                Request::Status(s, tone) => self.set_status(&s, tone),
                Request::StatusAs(s, kind) => self.set_status(&s, kind.tone()),
                Request::Notice(s, tone) => self.set_notice(&s, tone),
                Request::Sticky(s) => {
                    if self.world.committed_save_warning.is_some() {
                        self.show_committed_save_warning();
                    } else {
                        let now = self.world.clock.now();
                        self.shell.status.sticky_at(s, now);
                    }
                }
                Request::Progress(s, job) => {
                    if self.world.committed_save_warning.is_some() {
                        self.show_committed_save_warning();
                    } else {
                        let now = self.world.clock.now();
                        self.shell.status.progress_at(job, s, now);
                    }
                }
                Request::Open(layer, tag) => self.open_layer(layer, tag, owner),

                Request::Close => {
                    self.layers.pop(&mut self.shell.focus);
                    self.layer_gone();
                }
                Request::Go(g) => self.go(g),
                Request::Help => self.open_help(),
                Request::Back => self.go(Go::Manager),
                Request::Copy(s, what) => self.copy(s, what),
                Request::Top(f) => {
                    if let Some(top) = self.layers.top_mut() {
                        f(&mut *top.layer);
                    }
                }
                Request::RefusedOutsideClick(_) => {}
            }
        }
        out
    }

    /// The instance whose Capsule is on screen.
    fn open_capsule(&self) -> Option<InstanceId> {
        (self.route() == Route::Capsule)
            .then(|| self.router.capsule().map(|c| c.instance.clone()))
            .flatten()
    }

    fn attachable(&self, instance: &InstanceId) -> bool {
        self.world
            .instance(instance)
            .is_some_and(|i| i.status == InstanceStatus::Running)
    }

    fn refuse_attach(&mut self, instance: &InstanceId) {
        let unavailable_reason = self.world.entry(instance).and_then(|entry| match entry {
            InstanceEntry::Unavailable(unavailable) => Some(unavailable.display().reason),
            InstanceEntry::Available(_) => None,
        });
        self.set_status(
            &unavailable_reason.map_or_else(
                || format!("Cannot attach: {} is not running", instance.short_suffix()),
                |reason| format!("Cannot attach: agent metadata unavailable · {reason}"),
            ),
            Tone::Error,
        );
    }

    /// Navigates to `g`. When that takes the operator out of the open Capsule,
    /// every pane of it loses the focus before this returns and before the
    /// backend hears `capsule_left`, so a program that is no longer on screen
    /// neither counts as watched nor passes the clipboard's focus check. A
    /// refused or same-instance attach is not a departure.
    pub fn go(&mut self, g: Go) {
        let from = self.open_capsule();
        self.navigate(g);
        if let Some(instance) = from
            && self.open_capsule().as_ref() != Some(&instance)
        {
            let now = self.world.now_ms();
            if let Some(daemon) = self.world.daemons.get_mut(&instance) {
                daemon.blur(now);
            }
            let backend = Arc::clone(&self.world.backend);
            backend.capsule_left(&self.world, &instance);
        }
    }

    fn navigate(&mut self, g: Go) {
        if self.quit_guard.pending().is_some() && matches!(g, Go::Manager) {
            self.quit_guard.resolve(DialogResult::Action(1));
            self.quit_now();
            return;
        }
        let now = self.world.now_ms();
        match g {
            Go::Manager => {
                self.show_tab(TAB_MANAGER);
                self.enter_route();
            }
            Go::Settings => {
                let page = Page::Settings(Box::new(SettingsScreen::new(&self.world)));
                self.open_page(page);
            }
            Go::Accounts { select } => {
                self.show_tab(TAB_ACCOUNTS);
                self.router.accounts_mut().select(select);
                self.enter_route();
            }
            Go::Usage { select } => {
                self.show_tab(TAB_USAGE);
                self.router.usage_mut().select(select);
                self.enter_route();
            }
            Go::Roles { select } => {
                if let Some(roles) = self.router.roles_mut() {
                    roles.select(select);
                    self.show_tab(TAB_ROLES);
                    self.enter_route();
                }
            }
            Go::RoleEditor { role } => {
                let page = Page::RoleEditor(Box::new(RoleEditorScreen::new(&self.world, &role)));
                self.open_page(page);
            }
            Go::Doctor => {
                self.open_page(Page::Doctor(Box::default()));
            }
            Go::Editor { workspace, pending } => {
                let page = Page::Editor(Box::new(EditorScreen::new(
                    &self.world,
                    workspace,
                    pending.map(|b| *b),
                )));
                self.open_page(page);
            }
            Go::Prelude => {
                let page = Page::Prelude(Box::new(PreludeScreen::new(&self.world)));
                self.open_page(page);
            }
            Go::LaunchMany {
                workspace,
                role,
                instances,
                task,
            } => {
                let page = Page::Cockpit(Box::new(CockpitScreen::new(
                    &self.world,
                    LaunchTarget {
                        workspace,
                        role,
                        instances,
                        task,
                    },
                    self.shell.motion,
                )));
                self.open_page(page);
            }
            Go::Attach { instance, pane } => {
                if !self.attachable(&instance) {
                    self.refuse_attach(&instance);
                    return;
                }
                self.world.open_first_session(&instance);
                if let Some(d) = self.world.daemons.get_mut(&instance) {
                    d.attached_by = Some("this terminal".into());
                    d.set_policy(&self.world.pane_policy);
                }
                let page = Page::Capsule(Box::new(CapsuleScreen::new(
                    &instance,
                    &mut self.world,
                    pane,
                )));
                if self.route() == Route::Cockpit {
                    self.router.push(page);
                    self.handoff = Some(0);
                    if self.shell.motion == Motion::Reduced {
                        self.finish_handoff();
                    }
                } else {
                    self.open_page(page);
                    let name = self
                        .world
                        .daemons
                        .get(&instance)
                        .map(|d| d.workspace.clone())
                        .unwrap_or_default();
                    let status = fill_in(
                        &self.world.surfaces.vocabulary.attached_status,
                        &[("name", &name)],
                    );
                    self.set_status(&status, Tone::Secondary);
                }
            }
            Go::NewSession {
                instance,
                agent,
                account,
            } => {
                if !self.attachable(&instance) {
                    self.refuse_attach(&instance);
                    return;
                }
                self.world.open_first_session(&instance);
                let backend = Arc::clone(&self.world.backend);
                let request = crate::panes::PaneRequest::new(&instance, agent, account.clone());
                if let Some(d) = self.world.daemons.get_mut(&instance) {
                    d.new_tab(&request, now, |spawn| backend.spawn_pane(&request, spawn));
                }
                crate::world::refresh_snapshots(&mut self.world);
                self.world.refresh_now();
                self.navigate(Go::Attach {
                    instance,
                    pane: None,
                });
                let label = agent
                    .map(|a| a.label().to_owned())
                    .unwrap_or("Shell".into());
                self.set_status(&format!("Started {label} in a new tab"), Tone::Secondary);
            }
            Go::Detach => {
                let Some(inst) = self.router.capsule().map(|c| c.instance.clone()) else {
                    self.show_tab(TAB_MANAGER);
                    return;
                };
                if let Some(d) = self.world.daemons.get_mut(&inst) {
                    d.attached_by = None;
                }
                self.show_tab(TAB_MANAGER);
                self.router
                    .manager_mut()
                    .select_instance(&inst, &self.world);
                self.enter_route();
                let name = self
                    .world
                    .daemons
                    .get(&inst)
                    .map(|d| d.workspace.clone())
                    .unwrap_or_default();
                let n = self.world.running_count();
                self.set_status(
                    &format!(
                        "Detached · {name} keeps running · {} {}",
                        junie_tui::ui::text::plural(n, "instance", "instances"),
                        self.world.surfaces.vocabulary.instances_where
                    ),
                    Tone::Secondary,
                );
            }
            Go::InstanceEnded { instance, purge } => {
                self.world.end_instance_record(&instance, purge);
                self.world.daemons.remove(&instance);
                crate::world::refresh_snapshots(&mut self.world);
                self.world.sync_arbiter();

                self.show_tab(TAB_MANAGER);
                self.leave_construct(Some(&instance));
            }
            Go::PowerStart { instance } => match self.world.entry(&instance) {
                Some(InstanceEntry::Unavailable(unavailable)) if unavailable.phase.startable() => {
                    let row = crate::screens::manager::RowKey::Instance(instance.clone());
                    if self
                        .router
                        .manager_mut()
                        .busy_rows
                        .iter()
                        .any(|(k, _)| *k == row)
                    {
                        self.set_status(
                            "Power action already pending for this sandbox",
                            Tone::Warning,
                        );
                    } else {
                        self.router.manager_mut().busy_rows.push((row, "starting…"));
                        self.world
                            .perform(crate::backend::Effect::PowerStart { instance });
                        self.set_status("Starting sandbox…", Tone::Secondary);
                    }
                }
                Some(InstanceEntry::Unavailable(unavailable)) => {
                    let display = unavailable.display();
                    let message = if unavailable.phase.ready() {
                        format!(
                            "Sandbox {} is already ready · use t to stop",
                            display.sandbox_name
                        )
                    } else {
                        format!(
                            "Cannot start sandbox {} while {} · only stopped sandboxes can start",
                            display.sandbox_name,
                            unavailable.phase.label()
                        )
                    };
                    self.set_status(&message, Tone::Warning);
                }
                _ => self.set_status(
                    "Cannot start: unavailable sandbox metadata is no longer present",
                    Tone::Error,
                ),
            },
            Go::PowerStop { instance } => {
                if matches!(
                    self.world.entry(&instance),
                    Some(InstanceEntry::Unavailable(unavailable)) if unavailable.phase.ready()
                ) {
                    let row = crate::screens::manager::RowKey::Instance(instance.clone());
                    if self
                        .router
                        .manager_mut()
                        .busy_rows
                        .iter()
                        .any(|(k, _)| *k == row)
                    {
                        self.set_status(
                            "Power action already pending for this sandbox",
                            Tone::Warning,
                        );
                    } else {
                        self.router.manager_mut().busy_rows.push((row, "stopping…"));
                        self.world
                            .perform(crate::backend::Effect::PowerStop { instance });
                        self.set_status("Stopping sandbox…", Tone::Secondary);
                    }
                } else {
                    self.set_status(
                        "Cannot stop: only a ready unavailable sandbox can be stopped",
                        Tone::Warning,
                    );
                }
            }
            Go::LaunchFailedAck { instance } => {
                self.show_tab(TAB_MANAGER);
                if self.world.running_count() > 0 {
                    if let Some(i) = instance {
                        self.router.manager_mut().select_instance(&i, &self.world);
                    }
                    self.enter_route();
                    self.set_status(
                        &format!(
                            "Launch failed · {} still running {}",
                            junie_tui::ui::text::plural(
                                self.world.running_count(),
                                "instance",
                                "instances"
                            ),
                            self.world.surfaces.vocabulary.instances_where
                        ),
                        Tone::Error,
                    );
                } else if self.world.backend.launch_content().when_empty == EmptyLanding::Manager {
                    if let Some(i) = instance {
                        self.router.manager_mut().select_instance(&i, &self.world);
                    }
                    self.enter_route();
                } else {
                    self.leave_construct(instance.as_ref());
                }
            }

            Go::CloseTop { tag, then } => {
                if self.layers.top().is_some_and(|e| e.tag == tag) {
                    self.layers.pop(&mut self.shell.focus);
                    self.layer_gone();
                    if let Some(id) = then {
                        self.shell.focus.focus(id);
                    }
                }
            }
            Go::Quit => self.quit_now(),
        }
    }

    fn leave_construct(&mut self, instance: Option<&InstanceId>) {
        let decision = self.world.arbiter.request_exit(self.world.now_ms());
        match decision {
            ExitDecision::Outro { elapsed_secs } => {
                self.outro = Some(OutroState::new(self.shell.motion, elapsed_secs, 0));
                self.layers.clear(&mut self.shell.focus);
            }
            ExitDecision::StillInside { remaining } => {
                self.show_tab(TAB_MANAGER);
                if let Some(instance) = instance {
                    self.router
                        .manager_mut()
                        .select_instance(instance, &self.world);
                }
                self.enter_route();
                let rows: Vec<String> = self
                    .world
                    .running_entries()
                    .iter()
                    .map(|entry| match entry {
                        InstanceEntry::Available(i) => {
                            let ws = i
                                .workspace
                                .and_then(|x| self.world.workspace(x))
                                .map(|x| x.name.clone())
                                .unwrap_or("current directory".into());
                            format!(
                                "{ws} › {} · {}",
                                i.role_label(),
                                self.world
                                    .mask_path(&format!("{}/src/{ws}", self.world.home))
                            )
                        }
                        InstanceEntry::Unavailable(u) => {
                            let d = u.display();
                            format!("{} › {} · {}", d.workspace_name, d.role, d.reason)
                        }
                    })
                    .collect();
                let text = format!(
                    "{} · {}",
                    self.world.surfaces.vocabulary.still_inside,
                    junie_tui::ui::text::plural(remaining, "instance remains", "instances remain")
                );
                self.router.manager_mut().still_inside =
                    Some((text.clone(), rows, self.world.now_ms() + 6_000));
                self.set_status(&text, Tone::Secondary);
            }
            ExitDecision::AlreadyEnded => {
                self.show_tab(TAB_MANAGER);
                self.enter_route();
                let text = format!(
                    "Another client already ended {} · returning to the host",
                    self.world.surfaces.vocabulary.construct
                );
                self.set_status(&text, Tone::Secondary);
                self.exit_after_status = Some(self.world.now_ms() + 2_500);
            }
            ExitDecision::Unknown(e) => {
                self.show_tab(TAB_MANAGER);
                self.enter_route();
                self.set_status(&format!("Could not confirm remaining instances: {} · returning to host without the outro", self.world.surfaces.vocabulary.discovery(e)), Tone::Warning);
                self.exit_after_status = Some(self.world.now_ms() + 2_500);
            }
        }
    }

    fn enter_route(&mut self) {
        let route = self.route();
        self.on_route(route, Outcome::Ignored, |s, w, cx| {
            s.enter(w, cx);
            Outcome::Ignored
        });
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        match self.route() {
            Route::Intro => {
                let mut state = self
                    .intro
                    .take()
                    .unwrap_or_else(|| IntroState::new(self.shell.motion, 0));
                rain::render_intro(buf, area, &mut state, &t);
                self.intro = Some(state);
            }
            Route::Outro => {
                let mut state = self
                    .outro
                    .take()
                    .unwrap_or_else(|| OutroState::new(self.shell.motion, None, 0));
                rain::render_outro(
                    buf,
                    area,
                    &mut state,
                    &self.world.surfaces.vocabulary.construct,
                    &t,
                );
                self.outro = Some(state);
            }
            Route::Handoff => {
                let h = self.handoff.unwrap_or(0);
                ctx.inert = true;
                match handoff_stage(h) {
                    rain::HandoffStage::CockpitDim(n) => {
                        self.draw_frame(area, buf, ctx, Route::Cockpit);
                        rain::dim_buffer(buf, area, n, &t);
                    }
                    rain::HandoffStage::Canvas => {}
                    rain::HandoffStage::CapsuleDim(n) => {
                        self.draw_frame(area, buf, ctx, Route::Capsule);
                        rain::dim_buffer(buf, area, n, &t);
                    }
                    rain::HandoffStage::Capsule => self.draw_frame(area, buf, ctx, Route::Capsule),
                }
                ctx.inert = false;
            }
            r => {
                self.draw_frame(area, buf, ctx, r);

                for entry in self.layers.iter_mut() {
                    entry.layer.render(area, buf, ctx, &self.world);
                }
                if !self.layers.is_empty() {
                    let footer = Self::chrome(r).regions(area).footer;
                    self.footer(true).render(footer, buf, ctx, &mut self.shell);
                }
            }
        }
    }

    fn chrome(route: Route) -> Chrome {
        match route {
            Route::Capsule => Chrome::new().header(0).gap(0),
            Route::Cockpit => Chrome::new(),
            _ => Chrome::new().inset(1),
        }
    }

    fn draw_frame(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, route: Route) {
        let t = self.theme;
        fill(buf, area, t.base());
        let shell::Regions {
            header,
            body,
            footer,
        } = Self::chrome(route).regions(area);
        if Self::is_host(route) {
            self.draw_host_menu(header, buf, ctx, route);
        } else if !header.is_empty() {
            self.draw_strip(header, buf, ctx, route);
        }
        if let Some(s) = self.router.screen_mut(route) {
            s.render(body, buf, ctx, &self.world);
        }
        self.footer(false).render(footer, buf, ctx, &mut self.shell);
    }

    fn construct_state(&self, route: Route) -> (String, Tone) {
        let construct = &self.world.surfaces.vocabulary.construct;
        let (word, tone) = match route {
            Route::Intro => ("entering", Tone::Secondary),
            Route::Cockpit | Route::Handoff => ("entering", Tone::Normal),
            Route::Capsule => ("inside", Tone::Normal),
            Route::Outro => ("leaving", Tone::Secondary),
            _ => {
                if self.world.arbiter.entered_at_ms.is_some() || self.world.running_count() > 0 {
                    ("inside", Tone::Secondary)
                } else {
                    ("outside", Tone::Secondary)
                }
            }
        };
        (format!("{word} {construct}"), tone)
    }

    fn draw_strip(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, route: Route) {
        let t = self.theme;
        let (state, tone) = self.construct_state(route);
        let lockup = junie_tui::widgets::brand::Lockup::new(BRAND_MARK);
        let pill_w = lockup.render(area.x + 1, area.y, buf, &t);
        let area = Rect::new(
            area.x + 1 + pill_w,
            area.y,
            area.width.saturating_sub(1 + pill_w),
            area.height,
        );
        let mut left = vec![Segment::new(state, tone).priority(9)];
        match self.world.arbiter.running() {
            Ok(0) => left.push(Segment::new("no instances", Tone::Muted).priority(8)),
            Ok(n) => left.push(
                Segment::new(format!("{n} running"), Tone::Secondary)
                    .lead(t.glyphs.dot_on, Tone::Active)
                    .priority(8),
            ),
            Err(e) => left.push(
                Segment::new(
                    format!("! {}", self.world.surfaces.vocabulary.discovery(e)),
                    Tone::Error,
                )
                .priority(8),
            ),
        }
        if let Some(s) = self.health_segment() {
            left.push(s);
        }
        if let Some(s) = self.router.screen(route) {
            left.push(Segment::new(s.crumb(&self.world), Tone::Secondary).priority(7));
        }
        let mut right = vec![];
        if let Some(s) = self.router.screen(route) {
            right.extend(s.strip_right(&self.world));
        }
        if !self.world.jobs.is_empty() && !matches!(route, Route::Cockpit | Route::Capsule) {
            right.push(
                Segment::new(
                    format!(
                        "{} working",
                        junie_tui::widgets::progress::spinner_frame(ctx.interaction.tick)
                    ),
                    Tone::Secondary,
                )
                .priority(6),
            );
        }
        if matches!(
            route,
            Route::Manager
                | Route::Accounts
                | Route::Usage
                | Route::Settings
                | Route::Editor
                | Route::Prelude
        ) {
            right.push(
                Segment::new(
                    format!(
                        "{} · {}×{}",
                        t.level.label(),
                        self.shell.size.0,
                        self.shell.size.1
                    ),
                    Tone::Faint,
                )
                .priority(1),
            );
            if route != Route::Usage {
                right.push(
                    Segment::new("u Usage", Tone::Muted)
                        .clickable(STRIP_USAGE)
                        .priority(2),
                );
            }
            if route != Route::Accounts {
                right.push(
                    Segment::new("c Accounts", Tone::Muted)
                        .clickable(STRIP_ACCOUNTS)
                        .priority(3),
                );
            }
            if route != Route::Settings {
                right.push(
                    Segment::new("s Settings", Tone::Muted)
                        .clickable(STRIP_SETTINGS)
                        .priority(3),
                );
            }
        }
        right.push(
            Segment::new("? help", Tone::Muted)
                .clickable(STRIP_HELP)
                .priority(4),
        );
        segments::render(area, buf, ctx, &left, &right, t.canvas);
    }

    fn footer(&self, modal: bool) -> shell::Footer {
        use junie_tui::core::hints::{HasHints, HintCx};
        let focus = self.shell.focus.current();
        let editing = self
            .router
            .screen(self.route())
            .is_some_and(|s| s.is_editing())
            || self.layer_editing();
        let still = self
            .router
            .manager()
            .still_inside
            .as_ref()
            .map(|(s, _, _)| s.as_str());
        let status = match (still, self.shell.status.get()) {
            (Some(s), None) if self.route() == Route::Manager => Some((s, Tone::Secondary)),
            (_, s) => s,
        };
        let footer = if modal {
            shell::Footer::new().layer(self.layers.top().map(|t| t.layer.hints(focus)))
        } else {
            let menu = Self::is_host(self.route())
                .then(|| HasHints::hints(&self.host_menu, &HintCx::new(focus)));
            let mut screen = self
                .router
                .screen(self.route())
                .map(|s| s.hints(focus, &self.world));
            if Self::roots(self.route())
                && !editing
                && let Some(h) = screen.as_mut()
            {
                if self.world.surfaces.roles.is_some() && self.route() != Route::Roles {
                    h.push(hint("R", "Roles"));
                }
                if self.world.surfaces.doctor.is_some() {
                    h.push(hint("D", "Doctor"));
                }
            }
            shell::Footer::new()
                .layer(menu)
                .layer(screen)
                .fallback(vec![hint("?", "Help"), hint("Esc", "Back")])
        };
        footer.editing(editing).status(status)
    }
}

const BRAND_PADDED: &str = " jackin❯ ";

impl shell::App for App {
    type Status = ();
    fn shell(&self) -> &Shell {
        &self.shell
    }
    fn shell_mut(&mut self) -> &mut Shell {
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
    fn external(&self) -> Option<&ExternalReceiver> {
        self.external.as_ref()
    }
    fn layer_open(&self) -> bool {
        !self.layers.is_empty()
    }
    fn page_under_layer(&self) -> bool {
        self.layers.hides_page_focus()
            || self.host_menu.hides_page_focus()
            || self
                .router
                .screen(self.route())
                .is_some_and(|s| s.layer_open())
    }

    fn page_key(&self) -> PageKey {
        match self.route() {
            r @ (Route::Intro | Route::Outro | Route::Handoff) => PageKey(u64::MAX - r as u64),
            _ => self.router.key(),
        }
    }

    fn stakes(&mut self) -> Stakes {
        let world = &self.world;
        self.router
            .pages_mut()
            .fold(Stakes::Clean, |s, p| s.max(p.screen_mut().stakes(world)))
    }
    fn primary_focus(&self) -> Option<WidgetId> {
        self.router
            .screen(self.route())
            .and_then(|s| s.primary_focus())
    }
}

impl PointerHost for App {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        self.shell.pointer_parts()
    }

    fn layer_open(&self) -> bool {
        !self.layers.is_empty()
    }

    fn now_ms(&self) -> u64 {
        App::now_ms(self)
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        self.deliver_pointer(ev).into()
    }

    fn outside(&mut self, _pos: Position) -> Response<()> {
        if matches!(self.route(), Route::Intro | Route::Outro | Route::Handoff) {
            return self.skip_animation().into();
        }
        if !self.layers.is_empty() {
            return self
                .dismiss_layer(DismissReason::OutsideClick)
                .or(Outcome::Changed)
                .into();
        }
        if Self::is_host(self.route()) && self.host_menu.is_open() {
            self.host_menu.close();
        }
        Response::changed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostCmd {
    Manager,

    Accounts,

    Usage,

    Settings,

    Roles,

    Doctor,

    Help,

    About,

    Quit,

    Press(Key),
}

fn plain(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JackinCmd {
    Usage,

    Accounts,

    Settings,

    Roles,

    Doctor,

    Leave,
}

pub fn shell_keys(world: &World) -> shell::ShellKeys<JackinCmd> {
    use junie_tui::core::keymap::KeyBinding;
    use shell::ShellAction::{self, App as Cmd};
    let mut keys = shell::ShellKeys::new()
        .quit(junie_tui::ui::leave::QuitKeys::DEFAULT.letter(None))
        .bind(KeyBinding::char('u'), Cmd(JackinCmd::Usage))
        .bind(KeyBinding::char('c'), Cmd(JackinCmd::Accounts))
        .bind(KeyBinding::char('s'), Cmd(JackinCmd::Settings))
        .bind_fallback(KeyBinding::char('q'), Cmd(JackinCmd::Leave))
        .idle_only(ShellAction::MenuBar);
    if world.surfaces.roles.is_some() {
        keys = keys.bind(KeyBinding::char('R'), Cmd(JackinCmd::Roles));
    }
    if world.surfaces.doctor.is_some() {
        keys = keys.bind(KeyBinding::char('D'), Cmd(JackinCmd::Doctor));
    }
    keys
}

impl shell::KeyHost for App {
    type Cmd = JackinCmd;

    fn shell_keys(&self) -> &shell::ShellKeys<JackinCmd> {
        &self.keys
    }

    fn layer_key(&mut self, key: &Key) -> Option<Outcome> {
        if let Some(o) = self.sequence_key(key) {
            return Some(o);
        }
        if !self.layers.is_empty() {
            return Some(App::layer_key(self, *key));
        }
        self.quit_guard.resolve(DialogResult::Cancelled);
        None
    }

    fn menu_key(&mut self, key: &Key) -> Option<Outcome> {
        if !(Self::is_host(self.route()) && self.host_menu.is_open()) {
            return None;
        }
        let (o, ev) = self.host_menu.on_key(key);
        Some(match ev {
            Some(a @ (MenuBarAction::Chosen(..) | MenuBarAction::Brand)) => {
                self.host_menu_action(a, o)
            }
            _ => o.or(Outcome::Changed),
        })
    }

    fn capturing(&self) -> bool {
        matches!(self.route(), Route::Cockpit | Route::Capsule) || self.host_editing()
    }

    fn shell_action(&mut self, action: shell::ShellAction<JackinCmd>, _key: &Key) -> Outcome {
        use shell::ShellAction;
        let route = self.route();
        if !Self::is_host(route) {
            return Outcome::Ignored;
        }
        match action {
            ShellAction::MenuBar => {
                self.sync_host_menu();
                self.host_menu.open_menu(0);
                Outcome::Changed
            }
            ShellAction::Help => {
                self.open_help();
                Outcome::Changed
            }
            ShellAction::App(JackinCmd::Usage)
                if matches!(route, Route::Manager | Route::Accounts) =>
            {
                self.go(Go::Usage { select: None });
                Outcome::Changed
            }
            ShellAction::App(JackinCmd::Accounts)
                if matches!(route, Route::Manager | Route::Usage) =>
            {
                self.go(Go::Accounts { select: None });
                Outcome::Changed
            }
            ShellAction::App(JackinCmd::Settings)
                if matches!(route, Route::Manager | Route::Accounts | Route::Usage) =>
            {
                self.go(Go::Settings);
                Outcome::Changed
            }
            ShellAction::App(JackinCmd::Roles) if Self::roots(route) => {
                self.go(Go::Roles { select: None });
                Outcome::Changed
            }
            ShellAction::App(JackinCmd::Doctor) if Self::roots(route) => {
                self.go(Go::Doctor);
                Outcome::Changed
            }
            ShellAction::App(JackinCmd::Leave) => self.leave_key(),
            _ => Outcome::Ignored,
        }
    }

    fn chrome_key(&mut self, key: &Key) -> Outcome {
        if !(Self::is_host(self.route()) && self.shell.focus.is(HOST_MENU)) {
            return Outcome::Ignored;
        }
        self.sync_host_menu();
        let (o, ev) = self.host_menu.on_key(key);
        if let Some(MenuBarAction::Opened(_)) = ev {
            return Outcome::Changed;
        }
        if o.consumed() {
            return o;
        }
        if key.is(KeyCode::Esc) {
            let pf = self
                .router
                .screen(self.route())
                .and_then(|s| s.primary_focus());
            self.shell.focus.set(pf);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn page_key(&mut self, key: &Key) -> Outcome {
        let route = self.route();
        self.on_route(route, Outcome::Ignored, |s, w, cx| s.on_key(key, w, cx))
    }

    fn layer_paste(&mut self, text: &str) -> Option<Outcome> {
        let top = self.layers.top_mut()?;
        let o = top
            .layer
            .on_paste_focused(text, &self.shell.focus)
            .or(Outcome::Consumed);
        Some(self.settle_layers(o))
    }

    fn page_paste(&mut self, text: &str) -> Outcome {
        let route = self.route();
        self.on_route(route, Outcome::Consumed, |s, w, cx| s.on_paste(text, w, cx))
    }

    fn esc_top(&mut self) -> Outcome {
        let route = self.route();
        let o = self.on_route(route, Outcome::Ignored, |s, w, cx| s.on_esc_top(w, cx));
        if Self::is_host(route) {
            o
        } else {
            o.or(Outcome::Consumed)
        }
    }

    fn interrupt(&mut self) -> Outcome {
        let route = self.route();
        self.on_route(route, Outcome::Ignored, |s, w, cx| s.on_interrupt(w, cx))
    }

    fn request_quit(&mut self) -> Outcome {
        App::request_quit(self)
    }

    fn note(&mut self, text: &str) {
        self.set_status(text, Tone::Secondary);
    }
}
