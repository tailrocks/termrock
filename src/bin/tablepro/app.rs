use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

use junie_tui::clock::{Clock, WallClock};
use junie_tui::core::event::{Input, Key, Outcome};
use junie_tui::core::focus::FocusChange;

use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{
    MouseTarget, PointerEvent, PointerHost, PointerParts, PointerRouter,
};
use junie_tui::core::response::Response;
use junie_tui::shell::{self, PageKey, Shell};
use junie_tui::theme::{StatusKind, Strings, Theme, Tone};
use junie_tui::ui::context::ContextConfig;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::{
    DismissReason, Dismissed, LayerCx, LayerEntry, LayerPointer, LayerTag, ShellLayers, Taken,
};
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::leave::{Leave, LeaveGuard, Resolved, Stakes};

use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::{Dialog, DialogBody, DialogResult};
use junie_tui::widgets::grid::CellValue;

use junie_tui::widgets::input::TextInput;

use junie_tui::widgets::menu::MenuAction;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::{Picker, PickerAction};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::too_small::TooSmall;

use crate::connections::ConnectionsScreen;
use crate::context::{self, CtxMenu};
use crate::db::{Catalog, Environment, SafeMode};
use crate::filter_editor::{FilterDone, FilterEditor};
use crate::model::{History, SwitchTarget, SwitcherIndex};
use crate::pages::Tablepro;
use crate::sql::{self, Decision};
use crate::tabs::{Filter, FilterOp};
use crate::workbench::{WorkTab, Workbench};
use junie_tui::shell::Page;

const STRIP_SAFE: WidgetId = WidgetId::of("strip.safe");
const STRIP_SCOPE: WidgetId = WidgetId::of("strip.scope");
const STRIP_CONN: WidgetId = WidgetId::of("strip.conn");
const STRIP_HELP: WidgetId = WidgetId::of("strip.help");

pub const STRIP_APP: WidgetId = WidgetId::of("strip.app");
const HELP_DIALOG: WidgetId = WidgetId::of("dialog.help");
const SAFETY_DIALOG: WidgetId = WidgetId::of("dialog.safety");
const DISCARD_DIALOG: WidgetId = WidgetId::of("dialog.discard");
const CLOSE_DIALOG: WidgetId = WidgetId::of("dialog.close");
const COMMIT_DIALOG: WidgetId = WidgetId::of("dialog.commit");
const PREVIEW_DIALOG: WidgetId = WidgetId::of("dialog.preview");
const VIEWER_DIALOG: WidgetId = WidgetId::of("dialog.viewer");

const LEAVE_DIALOG: WidgetId = WidgetId::of("dialog.quit");

pub enum Act {
    EditFilter(Option<usize>),
    FilterOnCell(usize, CellValue),
    ConfirmDiscard,
    CommitPending,
    PreviewSql,
    OpenQuery(String, bool),
    OpenTableFiltered(String, String, String),
    OpenViewer(String, CellValue),
    ConfirmCloseTab(usize),

    Connected(usize),
}

pub type Request = junie_tui::shell::Request<ModalResult, (), Act>;

pub type Cx<'a> = junie_tui::shell::Cx<'a, ModalResult, (), Act>;

pub trait OpenDialog {
    fn open_dialog(&mut self, d: Dialog);
}

impl OpenDialog for Cx<'_> {
    fn open_dialog(&mut self, mut d: Dialog) {
        CHROME.fit(&mut d);
        self.open(d, LayerTag::new("dialog"));
    }
}

pub const CHROME: shell::Chrome = shell::Chrome::new().inset(1);

pub use junie_tui::motion::Motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Connections,
    Workbench,
}

const SWITCHER: &str = "switcher";

const TAB_LIST: &str = "tablist";

const SAFE_MODE: &str = "safemode";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalResult {
    Lib(LayerResult),
    Filter(FilterDone),
}

impl From<LayerResult> for ModalResult {
    fn from(r: LayerResult) -> Self {
        Self::Lib(r)
    }
}

pub type Layers = ShellLayers<ModalResult>;

pub struct App {
    pub theme: Theme,

    pub router: shell::Router<Route>,

    pub leave: LeaveGuard<shell::Nav<Route>>,
    pub connections: ConnectionsScreen,
    pub workbench: Option<Workbench>,
    pub history: History,

    pub shell: Shell<Option<StatusKind>>,

    pub context: ContextConfig,

    pub(crate) clock: WallClock,

    pub layers: Layers,
    pub tick: u64,

    pub keys: shell::ShellKeys<TpCmd>,

    committing: Option<u32>,
    scope: usize,
    switcher_targets: Vec<SwitchTarget>,

    switcher_index: Option<SwitcherIndex>,
}

impl App {
    pub fn new(theme: Theme) -> Self {
        let connections = ConnectionsScreen::new(crate::db::connections());
        let mut shell = Shell::new();
        shell.focus.focus(WidgetId::of("connections").sub("tree"));
        Self {
            theme,
            router: shell::Router::new(Route::Connections),
            leave: LeaveGuard::new(LEAVE_DIALOG).save(false),
            connections,
            workbench: None,
            history: History::seeded(),
            shell,
            context: ContextConfig::DEFAULT,
            clock: WallClock::for_run(),
            layers: Layers::new(),
            tick: 0,
            keys: shell_keys(),
            committing: None,
            scope: 0,
            switcher_targets: vec![],
            switcher_index: None,
        }
    }

    pub fn connect(&mut self, index: usize) {
        let Some(c) = self.connections.connections.get(index).cloned() else {
            return;
        };
        let mut wb = Workbench::new(c, Catalog::acme_prod());
        wb.new_query("");

        self.shell.focus.focus(crate::workbench::EXPLORER);
        self.workbench = Some(wb);

        let nav = match self.route() {
            Route::Connections => shell::Nav::Push(Route::Workbench),
            Route::Workbench => shell::Nav::Replace(Route::Workbench),
        };
        self.router.apply(nav, |r| r);
        self.set_status_as(
            StatusKind::Ok,
            format!(
                "Connected to {}",
                self.workbench.as_ref().unwrap().connection.name
            ),
        );
    }

    pub fn route(&self) -> Route {
        *self.router.current()
    }

    pub fn set_motion(&mut self, motion: Motion, frame: u64) {
        self.shell.motion = Motion::Full;
        if motion == Motion::Paused {
            for _ in 0..frame {
                self.advance();
            }
            self.tick = frame;
        }
        self.shell.motion = motion;
    }

    pub fn animating(&self) -> bool {
        if self.shell.motion == Motion::Paused {
            return false;
        }
        self.connections.animating()
            || self.workbench.as_ref().is_some_and(|w| w.animating())
            || self.shell.pointer.has_flash()
            || self.committing.is_some()
    }

    pub fn tick_interval(&self) -> Duration {
        if self.shell.pointer.autoscrolling() {
            return Duration::from_millis(self.shell.pointer.config.autoscroll_ms);
        }

        if self.shell.pointer.track_repeating() {
            return Duration::from_millis(self.shell.pointer.config.track_repeat_ms);
        }
        if self.animating() {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(400)
        }
    }

    fn set_status(&mut self, s: String) {
        self.put_status(s, None);
    }

    fn set_status_as(&mut self, kind: StatusKind, s: String) {
        self.put_status(s, Some(kind));
    }

    fn put_status(&mut self, s: String, kind: Option<StatusKind>) {
        let tone = if kind == Some(StatusKind::Fail) {
            Tone::Error
        } else {
            Tone::Secondary
        };
        self.shell
            .status
            .set_with_at(s, tone, kind, self.clock.now());
    }

    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn flash_clock(&self) -> u64 {
        if self.shell.motion == Motion::Paused {
            0
        } else {
            self.now_ms()
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

    fn disconnect(&mut self) -> Outcome {
        if self.route() != Route::Workbench {
            return Outcome::Ignored;
        }
        self.navigate(shell::Nav::Back)
    }

    fn navigate(&mut self, nav: shell::Nav<Route>) -> Outcome {
        let stakes = || {
            self.workbench
                .as_mut()
                .map(|w| Page::stakes(w, &self.history))
                .unwrap_or_default()
        };
        match self
            .router
            .request(&mut self.leave, nav, stakes, &Strings::DEFAULT)
        {
            Ok(nav) => self.go(nav),
            Err(d) => {
                let name = self
                    .workbench
                    .as_ref()
                    .map(|w| w.connection.name.clone())
                    .unwrap_or_default();
                let d = self.leave_dialog(*d, &format!("Disconnect from {name}?"), "Disconnect");
                self.open_dialog(d);
                Outcome::Changed
            }
        }
    }

    fn go(&mut self, nav: shell::Nav<Route>) -> Outcome {
        let Some(dropped) = self.router.apply(nav, |r| r) else {
            return Outcome::Ignored;
        };
        if dropped.contains(&Route::Workbench)
            && let Some(w) = self.workbench.take()
        {
            self.shell.focus.focus(crate::connections::TREE);
            self.set_status(format!("Disconnected from {}", w.connection.name));
        }
        Outcome::Changed
    }

    fn leave_dialog(&self, mut d: Dialog, title: &str, verb: &str) -> Dialog {
        d.title = title.to_owned();
        if let Some(lost) = self.workbench.as_ref().and_then(|w| w.losses()) {
            d.body = DialogBody::Text(format!("{lost} will be lost."));
        }
        if let Some(b) = d.actions.get_mut(1) {
            b.label = verb.to_owned();
        }
        d
    }

    fn new_connection(&mut self) -> Outcome {
        let (o, reqs) = self.shell.deliver(true, |cx| {
            self.connections
                .leave_form(crate::connections::FormLeave::New, cx)
        });
        o.or(self.apply_requests(reqs))
    }

    fn request_quit(&mut self) -> Outcome {
        let stakes = self.connections.form_stakes();
        if let Err(d) = self
            .connections
            .leave
            .request_quit(&stakes, &Strings::DEFAULT)
        {
            self.open_dialog(*d);
            return Outcome::Changed;
        }

        let stakes = shell::App::stakes(self);
        match self
            .shell
            .quit_through(&mut self.leave, &stakes, &Strings::DEFAULT)
        {
            Ok(()) => Outcome::Consumed,
            Err(d) => {
                let d = self.leave_dialog(*d, "Quit TablePro?", "Quit");
                self.open_dialog(d);
                Outcome::Changed
            }
        }
    }

    fn autoscroll(&mut self) -> Outcome {
        PointerRouter::tick(self)
    }

    fn on_tick(&mut self) -> Outcome {
        let auto = self.autoscroll();
        if self.shell.motion == Motion::Paused {
            return auto;
        }
        self.advance().or(auto)
    }

    fn advance(&mut self) -> Outcome {
        let mut out = Outcome::Ignored;
        if self.animating() {
            if self.shell.motion == Motion::Full {
                self.tick = self.tick.wrapping_add(1);
            }
            out = Outcome::Changed;
        }
        let now = self.now_ms();
        if self.shell.pointer.expire_flash(now) {
            out = Outcome::Changed;
        }
        if self.shell.status.expire(self.clock.now()) {
            out = Outcome::Changed;
        }

        let (o, reqs) = self.shell.deliver(true, |cx| {
            Page::on_tick(&mut self.connections, &mut self.history, cx)
        });
        out = out.or(o).or(self.apply_requests(reqs));
        if let Some(left) = self.committing.as_mut() {
            *left = left.saturating_sub(1);
            if *left == 0 {
                self.committing = None;
                self.finish_commit();
            }
            out = Outcome::Changed;
        }
        if let Some(w) = self.workbench.as_mut() {
            if w.tick_explorer() {
                out = Outcome::Changed;
            }
            let conn = w.connection.name.clone();
            let db = w.catalog.database.clone();
            let cat = &w.catalog;
            let mut entries = vec![];
            for t in w.tabs.iter_mut() {
                if let WorkTab::Query(q) = t {
                    let was = q.is_running();
                    entries.extend(q.tick(cat, &conn, &db));
                    if was && !q.is_running() {
                        out = Outcome::Changed;
                    }
                }
            }
            if !entries.is_empty() {
                for e in entries {
                    self.history.push(e);
                }

                let hist = &self.history;
                let conn2 = conn.clone();
                for t in w.tabs.iter_mut() {
                    if let WorkTab::History(h) = t {
                        h.refresh(hist, &conn2);
                    }
                }
                out = Outcome::Changed;
            }
            if w.running().is_some() {
                out = Outcome::Changed;
            }
        }
        out
    }

    fn shown(&self) -> Option<&dyn Page<Tablepro>> {
        match self.route() {
            Route::Connections => Some(&self.connections),
            Route::Workbench => self.workbench.as_ref().map(|w| w as &dyn Page<Tablepro>),
        }
    }

    fn shown_mut(&mut self) -> (Option<&mut dyn Page<Tablepro>>, &mut History) {
        let page: Option<&mut dyn Page<Tablepro>> = match self.route() {
            Route::Connections => Some(&mut self.connections),
            Route::Workbench => self
                .workbench
                .as_mut()
                .map(|w| w as &mut dyn Page<Tablepro>),
        };
        (page, &mut self.history)
    }

    fn with_page(
        &mut self,
        f: impl FnOnce(&mut dyn Page<Tablepro>, &mut History, &mut Cx) -> Outcome,
    ) -> Outcome {
        let route = self.route();
        let (o, reqs) = self.shell.deliver(true, |cx| {
            let page: Option<&mut dyn Page<Tablepro>> = match route {
                Route::Connections => Some(&mut self.connections),
                Route::Workbench => self
                    .workbench
                    .as_mut()
                    .map(|w| w as &mut dyn Page<Tablepro>),
            };
            match page {
                Some(p) => f(p, &mut self.history, cx),
                None => Outcome::Ignored,
            }
        });
        o.or(self.apply_requests(reqs))
    }

    fn apply_requests(&mut self, requests: Vec<Request>) -> Outcome {
        let mut out = Outcome::Ignored;
        for r in requests {
            out = Outcome::Changed;
            match r {
                Request::Status(s, tone) => {
                    let now = self.clock.now();
                    self.shell.status.set_with_at(s, tone, None, now);
                }
                Request::StatusAs(s, kind) => self.set_status_as(kind, s),
                Request::Sticky(s) => {
                    let now = self.clock.now();
                    self.shell.status.sticky_at(s, now);
                }
                Request::Progress(s, job) => {
                    let now = self.clock.now();
                    self.shell.status.progress_at(job, s, now);
                }
                Request::Open(layer, tag) => self.open_layer(layer, tag),
                Request::Go(a) => self.act(a),

                Request::Copy(text, what) => {
                    let now = self.clock.now();
                    self.shell.copy_at(&text, what.as_deref(), now);
                }

                Request::Close
                | Request::Top(_)
                | Request::Back
                | Request::Help
                | Request::RefusedOutsideClick(_) => {}
            }
        }
        out
    }

    fn act(&mut self, a: Act) {
        match a {
            Act::Connected(i) => self.connect(i),
            Act::EditFilter(i) => self.open_filter_editor(i, None),
            Act::FilterOnCell(col, v) => {
                let val = match &v {
                    CellValue::Null => (FilterOp::IsNull, String::new()),
                    other => (FilterOp::Eq, other.text()),
                };
                self.open_filter_editor(None, Some((col, val.0, val.1)));
            }
            Act::ConfirmDiscard => {
                let n = self
                    .workbench
                    .as_ref()
                    .map(|w| w.pending_total())
                    .unwrap_or(0);
                let d = Dialog::destructive(
                    DISCARD_DIALOG,
                    "Discard unsaved changes?",
                    &format!(
                        "{n} pending change{} will be dropped. The rows are reloaded from the server.",
                        if n == 1 { "" } else { "s" }
                    ),
                    "Discard",
                );
                self.open_dialog(d);
            }
            Act::CommitPending => self.begin_commit(),
            Act::PreviewSql => self.open_preview(),
            Act::OpenQuery(sql, run) => {
                if let Some(w) = self.workbench.as_mut() {
                    let i = w.new_query(&sql);
                    if let Some(pf) = w.primary_focus() {
                        self.shell.focus.focus(pf);
                    }
                    if run {
                        let _ = i;
                        let _ = self.run_active(false, None);
                    }
                }
            }
            Act::OpenTableFiltered(table, col, value) => {
                if let Some(w) = self.workbench.as_mut() {
                    let schema = w
                        .catalog
                        .find(None, &table)
                        .map(|t| t.schema.clone())
                        .unwrap_or("public".into());
                    if let Some(i) = w.open_table(&schema, &table, false) {
                        if let Some(WorkTab::Table(t)) = w.tabs.get_mut(i) {
                            t.filters = vec![Filter {
                                column: col,
                                op: FilterOp::Eq,
                                value,
                                value2: String::new(),
                                enabled: true,
                            }];
                            let cat = w.catalog.clone();
                            t.load(&cat);
                        }
                        if let Some(pf) = w.primary_focus() {
                            self.shell.focus.focus(pf);
                        }
                    }
                }
            }
            Act::OpenViewer(name, v) => {
                let text = match &v {
                    CellValue::Json(j) => pretty_json(j),
                    other => other.text(),
                };
                let lines: Vec<String> = text.lines().map(str::to_owned).collect();
                let mut d = Dialog::facts(
                    VIEWER_DIALOG,
                    &name,
                    vec![],
                    lines,
                    None,
                    Button::secondary(VIEWER_DIALOG.sub("close"), "Close"),
                );
                d.actions.remove(0);
                d.cancel_index = Some(0);
                d.initial_focus = d.actions[0].id;
                d.width = 80;
                self.open_dialog(d);
            }
            Act::ConfirmCloseTab(i) => {
                let d = Dialog::destructive(
                    CLOSE_DIALOG.child(i),
                    "Close tab with unsaved work?",
                    "Pending row edits and unsaved query text in this tab will be lost.",
                    "Close anyway",
                );
                self.open_dialog(d);
            }
        }
    }

    fn run_cmd(&mut self, cmd: TpCmd) -> Outcome {
        if cmd == TpCmd::NewConnection {
            if self.route() != Route::Connections {
                return Outcome::Ignored;
            }
            return self.new_connection();
        }
        if self.route() != Route::Workbench || self.workbench.is_none() {
            return Outcome::Ignored;
        }
        match cmd {
            TpCmd::NewConnection => Outcome::Ignored,
            TpCmd::CancelQuery => {
                let Some(w) = self.workbench.as_mut() else {
                    return Outcome::Ignored;
                };
                if w.running().is_none() {
                    return Outcome::Ignored;
                }
                for t in w.tabs.iter_mut() {
                    if let WorkTab::Query(q) = t
                        && q.cancel()
                    {
                        self.set_status_as(StatusKind::Info, "Query cancelled".into());
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            TpCmd::Run => self.run_active(false, None),

            TpCmd::RunOrRefresh if self.active_is_query() => self.run_active(false, None),
            TpCmd::RunOrRefresh => Outcome::Ignored,
            TpCmd::RunAll => self.run_active(true, None),
            TpCmd::Explain => self.run_active(false, Some(false)),
            TpCmd::ExplainAnalyze => self.run_active(false, Some(true)),
            TpCmd::NewQuery => {
                if let Some(w) = self.workbench.as_mut() {
                    w.new_query("");
                    if let Some(pf) = w.primary_focus() {
                        self.shell.focus.focus(pf);
                    }
                }
                Outcome::Changed
            }
            TpCmd::CloseTab => {
                if let Some(w) = self.workbench.as_ref() {
                    self.request_close_tab(w.active);
                }
                Outcome::Changed
            }
            TpCmd::Switcher => {
                self.open_switcher();
                Outcome::Changed
            }
            TpCmd::TabList => {
                self.open_tab_list();
                Outcome::Changed
            }
            TpCmd::History => {
                if let Some(w) = self.workbench.as_mut() {
                    w.open_history(&self.history);
                    if let Some(pf) = w.primary_focus() {
                        self.shell.focus.focus(pf);
                    }
                }
                Outcome::Changed
            }
            TpCmd::ToggleExplorer => {
                let shown = self.workbench.as_ref().is_some_and(|w| w.explorer_visible);
                self.set_explorer(!shown)
            }
            TpCmd::SafeMode => {
                self.open_safe_mode_picker();
                Outcome::Changed
            }
            TpCmd::ToggleStructure => {
                if let Some(w) = self.workbench.as_mut()
                    && let Some(WorkTab::Table(t)) = w.active_tab_mut()
                {
                    t.mode_tabs
                        .set_active(if t.mode_tabs.active == 0 { 1 } else { 0 });
                    let cat = w.catalog.clone();
                    if let Some(WorkTab::Table(t)) = w.active_tab_mut() {
                        t.structure_refresh(&cat);
                        let pf = if t.mode_tabs.active == 1 {
                            t.structure.id
                        } else {
                            t.grid.id
                        };
                        self.shell.focus.focus(pf);
                    }
                }
                Outcome::Changed
            }
            TpCmd::Commit => {
                self.begin_commit();
                Outcome::Changed
            }
            TpCmd::Find => {
                if self.active_is_query() {
                    if let Some(w) = self.workbench.as_mut()
                        && let Some(WorkTab::Query(q)) = w.active_tab_mut()
                    {
                        q.editor.open_find();
                        self.shell.focus.focus(q.editor.id);
                    }
                } else {
                    self.open_filter_editor(None, None);
                }
                Outcome::Changed
            }
            TpCmd::Maximize => {
                if let Some(w) = self.workbench.as_mut() {
                    w.maximized = !w.maximized;
                    let maximized = w.maximized;
                    let f = self.shell.focus.current();
                    if let Some(WorkTab::Query(q)) = w.active_tab_mut() {
                        let target = if f == Some(q.editor.id) {
                            junie_tui::ui::layout::Maximized::First
                        } else {
                            junie_tui::ui::layout::Maximized::Second
                        };
                        q.split.maximized = if maximized {
                            target
                        } else {
                            junie_tui::ui::layout::Maximized::None
                        };
                    }
                }
                Outcome::Changed
            }
            TpCmd::GrowResults | TpCmd::GrowEditor => {
                if let Some(w) = self.workbench.as_mut()
                    && let Some(WorkTab::Query(q)) = w.active_tab_mut()
                {
                    q.split.grow(if cmd == TpCmd::GrowEditor { 8 } else { -8 });
                }
                Outcome::Changed
            }
            TpCmd::Explorer => {
                if let Some(w) = self.workbench.as_mut() {
                    w.explorer_visible = true;
                    w.maximized = false;
                }
                self.shell.focus.focus(crate::workbench::EXPLORER);
                Outcome::Changed
            }
        }
    }

    fn step_tab(&mut self, step: isize) -> Outcome {
        if self.route() != Route::Workbench {
            return Outcome::Ignored;
        }
        if let Some(w) = self.workbench.as_mut()
            && !w.tabs.is_empty()
        {
            let n = w.tabs.len() as isize;
            let i = (w.active as isize + step).rem_euclid(n) as usize;
            w.set_active(i);
            if let Some(pf) = w.primary_focus()
                && !self.shell.focus.is(crate::workbench::EXPLORER)
                && !self.shell.focus.is(crate::workbench::TABSTRIP)
            {
                self.shell.focus.focus(pf);
            }
        }
        Outcome::Changed
    }

    fn active_is_query(&self) -> bool {
        matches!(
            self.workbench.as_ref().and_then(|w| w.active_tab()),
            Some(WorkTab::Query(_))
        )
    }

    fn set_explorer(&mut self, show: bool) -> Outcome {
        if let Some(w) = self.workbench.as_mut() {
            w.explorer_visible = show;
            if show {
                self.shell.focus.focus(crate::workbench::EXPLORER);
            } else if (self.shell.focus.is(crate::workbench::EXPLORER)
                || self.shell.focus.is(w.explorer_filter.id))
                && let Some(pf) = w.primary_focus()
            {
                self.shell.focus.focus(pf);
            }
        }
        Outcome::Changed
    }

    fn cancel_running(&mut self) -> Outcome {
        if let Some(w) = self.workbench.as_mut() {
            for t in w.tabs.iter_mut() {
                if let WorkTab::Query(q) = t
                    && q.cancel()
                {
                    self.set_status_as(StatusKind::Info, "Query cancelled".into());
                    return Outcome::Changed;
                }
            }
        }
        Outcome::Consumed
    }

    pub fn run_active(&mut self, all: bool, explain: Option<bool>) -> Outcome {
        let Some(w) = self.workbench.as_mut() else {
            return Outcome::Ignored;
        };
        let level = w.connection.safe_mode;
        let env = w.connection.environment;
        let conn = w.connection.name.clone();
        let db = w.catalog.database.clone();
        let tab_index = w.active;
        let Some(WorkTab::Query(q)) = w.active_tab_mut() else {
            self.set_status("Open a query tab to run SQL (Ctrl+T)".into());
            return Outcome::Changed;
        };
        if q.is_running() {
            self.set_status("Already running · Esc cancels".into());
            return Outcome::Changed;
        }
        let statements = q.statements_to_run(all);
        if statements.is_empty() {
            self.set_status("Nothing to run".into());
            return Outcome::Changed;
        }

        let mut worst: Option<(Decision, sql::Statement, String)> = None;
        let mut parse_errors = false;
        for (text, _) in &statements {
            let Ok(mut stmt) = sql::parse(text) else {
                parse_errors = true;
                continue;
            };
            if let Some(analyze) = explain {
                stmt = sql::Statement::Explain {
                    analyze,
                    inner: Box::new(stmt),
                };
            }
            let d = sql::gate(level, &stmt);
            let rank = |d: &Decision| match d {
                Decision::Run => 0,
                Decision::Confirm { deliberate: false } => 1,
                Decision::Confirm { deliberate: true } => 2,
                Decision::Deny => 3,
            };
            if worst.as_ref().is_none_or(|(wd, ws, _)| {
                rank(&d) > rank(wd) || (rank(&d) == rank(wd) && sql::tier(&stmt) > sql::tier(ws))
            }) {
                worst = Some((d, stmt, text.clone()));
            }
        }
        let _ = parse_errors;
        let Some((decision, stmt, text)) = worst else {
            q.start(statements, all, explain);
            return Outcome::Changed;
        };
        match decision {
            Decision::Run => {
                q.start(statements, all, explain);
                Outcome::Changed
            }
            Decision::Deny => {
                self.set_status_as(StatusKind::Warn, "Cannot execute write queries: TablePro's Safe Mode is set to read-only for this connection · Ctrl+L changes Safe Mode".into());
                Outcome::Changed
            }
            Decision::Confirm { deliberate } => {
                let table = w.catalog.find(None, stmt.target().unwrap_or(""));
                let risk = sql::assess(&stmt, table);
                w.pending_run = Some((tab_index, statements.clone(), all, explain));
                let tier = sql::tier(&stmt);
                let dangerous = sql::is_dangerous(&stmt);
                let title = if dangerous {
                    "This query may permanently modify or delete data"
                } else if tier == sql::Tier::Safe {
                    "Execute query?"
                } else {
                    "Execute write query?"
                };
                let production = env == Environment::Production;
                let env_label = format!("{} · {}", conn, env.label());
                let mut facts = vec![
                    Prop::new("Action", risk.action.clone()).tone(if dangerous {
                        Tone::Error
                    } else {
                        Tone::Normal
                    }),
                    Prop::new(
                        "Target",
                        format!(
                            "{env_label} · {db}{}",
                            stmt.target().map(|t| format!(" · {t}")).unwrap_or_default()
                        ),
                    )
                    .tone(if production {
                        env.kind().tone()
                    } else {
                        Tone::Secondary
                    }),
                ];
                if !risk.scope.is_empty() {
                    facts.push(
                        Prop::new("Scope", risk.scope.clone())
                            .tone(Tone::Secondary)
                            .wrap(),
                    );
                }
                if !risk.risk.is_empty() {
                    facts.push(
                        Prop::new("Risk", risk.risk.clone())
                            .tone(if dangerous {
                                Tone::Warning
                            } else {
                                Tone::Secondary
                            })
                            .wrap(),
                    );
                }
                if !risk.reversible.is_empty() {
                    facts.push(
                        Prop::new("Reversible", risk.reversible.to_owned())
                            .tone(Tone::Secondary)
                            .wrap(),
                    );
                }
                facts.push(
                    Prop::new(
                        "Safe Mode",
                        format!(
                            "{} · {}",
                            level.label(),
                            if deliberate {
                                "type to confirm"
                            } else {
                                "confirm before running"
                            }
                        ),
                    )
                    .tone(Tone::Muted),
                );
                let code: Vec<String> = statements
                    .iter()
                    .flat_map(|(s, _)| s.lines().map(|l| l.to_owned()).collect::<Vec<_>>())
                    .collect();
                let token = if deliberate || (dangerous && env == Environment::Production) {
                    Some(stmt.target().unwrap_or("yes").to_owned())
                } else {
                    None
                };

                let confirm = if dangerous || tier == sql::Tier::Destructive || production {
                    Button::danger(SAFETY_DIALOG.sub("ok"), "Execute")
                } else {
                    Button::primary(SAFETY_DIALOG.sub("ok"), "Execute")
                };
                let mut d =
                    Dialog::facts(SAFETY_DIALOG, title, facts, code, token.as_deref(), confirm);
                if production {
                    d = d.title_status(StatusKind::Warn);
                }
                if token.is_none() {
                    d.initial_focus = if dangerous {
                        d.actions[0].id
                    } else {
                        d.actions[1].id
                    };
                }
                d.width = 74;
                let _ = text;
                self.open_dialog(d);
                Outcome::Changed
            }
        }
    }

    fn begin_commit(&mut self) {
        let Some(w) = self.workbench.as_mut() else {
            return;
        };
        let level = w.connection.safe_mode;
        let env = w.connection.environment;
        let conn = w.connection.name.clone();
        let db = w.catalog.database.clone();
        let (schema, name) = match w.active_tab() {
            Some(WorkTab::Table(t)) => (t.schema.clone(), t.name.clone()),
            _ => {
                self.set_status("Nothing to save".into());
                return;
            }
        };
        let table = w.catalog.find(Some(&schema), &name).cloned();
        let Some(table) = table else { return };
        let Some(WorkTab::Table(t)) = w.active_tab_mut() else {
            return;
        };
        if t.grid.pending.is_empty() {
            self.set_status("No pending changes".into());
            return;
        }
        if level == SafeMode::ReadOnly {
            self.set_status_as(
                StatusKind::Warn,
                "Cannot save changes: TablePro's Safe Mode is set to read-only for this connection"
                    .into(),
            );
            return;
        }
        let sqls = crate::model::preview_sql(&table, &t.columns, &t.grid);
        let (u, i, d) = t.grid.pending.counts();
        let deletes = d;
        let deliberate = level.requires_authentication();
        let mut facts = vec![
            Prop::new("Action", "Save changes".to_owned()),
            Prop::new(
                "Target",
                format!("{} · {} · {} · {}", conn, env.label(), db, t.qualified()),
            )
            .tone(if env == Environment::Production {
                Tone::Normal
            } else {
                Tone::Secondary
            }),
            Prop::new(
                "Scope",
                format!(
                    "{u} update{} · {i} insert{} · {d} delete{}",
                    if u == 1 { "" } else { "s" },
                    if i == 1 { "" } else { "s" },
                    if d == 1 { "" } else { "s" }
                ),
            )
            .tone(Tone::Secondary),
        ];
        if deletes > 0 {
            facts.push(
                Prop::new("Risk", "Deleted rows cannot be restored without a backup.")
                    .tone(Tone::Warning)
                    .wrap(),
            );
        }
        facts.push(
            Prop::new(
                "Transaction",
                "All statements run in one transaction; a failure rolls everything back.",
            )
            .tone(Tone::Muted)
            .wrap(),
        );
        facts.push(
            Prop::new(
                "Safe Mode",
                format!(
                    "{} · {}",
                    level.label(),
                    if deliberate {
                        "type to confirm"
                    } else if level.requires_confirmation() || deletes > 0 {
                        "confirm before running"
                    } else {
                        "runs after this review"
                    }
                ),
            )
            .tone(Tone::Muted),
        );
        let token = if deliberate {
            Some(t.name.clone())
        } else {
            None
        };
        let confirm = if deletes > 0 {
            Button::danger(COMMIT_DIALOG.sub("ok"), "Save")
        } else {
            Button::primary(COMMIT_DIALOG.sub("ok"), "Save")
        };
        let title = if deletes > 0 {
            format!(
                "Delete {deletes} row{} and save?",
                if deletes == 1 { "" } else { "s" }
            )
        } else {
            "Save changes?".to_owned()
        };
        let mut dlg = Dialog::facts(
            COMMIT_DIALOG,
            &title,
            facts,
            sqls,
            token.as_deref(),
            confirm,
        );
        if token.is_none() {
            dlg.initial_focus = if deletes > 0 {
                dlg.actions[0].id
            } else {
                dlg.actions[1].id
            };
        }
        dlg.width = 78;
        self.open_dialog(dlg);
    }

    fn finish_commit(&mut self) {
        let Some(w) = self.workbench.as_mut() else {
            return;
        };
        let conn = w.connection.name.clone();
        let db = w.catalog.database.clone();
        let (schema, name) = match w.active_tab() {
            Some(WorkTab::Table(t)) => (t.schema.clone(), t.name.clone()),
            _ => return,
        };
        let table = w.catalog.find(Some(&schema), &name).cloned();
        let Some(WorkTab::Table(t)) = w.active_tab_mut() else {
            return;
        };
        let sqls = table
            .as_ref()
            .map(|tb| crate::model::preview_sql(tb, &t.columns, &t.grid))
            .unwrap_or_default();
        let n = t.grid.pending.total();
        let qualified = t.qualified();
        let schema = t.schema.clone();
        t.grid.apply_commit_result(Ok(()));
        for s in sqls {
            self.history.push(crate::model::HistoryEntry {
                id: 0,
                sql: s,
                connection: conn.clone(),
                database: db.clone(),
                schema: schema.clone(),
                minutes_ago: 0,
                duration_ms: Some(3),
                rows: Some(1),
                error: None,
                source: crate::model::HistorySource::RowEdits,
            });
        }
        self.set_status_as(
            StatusKind::Ok,
            format!(
                "Saved {n} change{} to {qualified}",
                if n == 1 { "" } else { "s" }
            ),
        );
    }

    fn open_preview(&mut self) {
        let Some(w) = self.workbench.as_ref() else {
            return;
        };
        let Some(WorkTab::Table(t)) = w.active_tab() else {
            return;
        };
        let Some(table) = w.catalog.find(Some(&t.schema), &t.name) else {
            return;
        };
        let sqls = crate::model::preview_sql(table, &t.columns, &t.grid);
        let n = sqls.len();
        let mut d = Dialog::facts(
            PREVIEW_DIALOG,
            &format!(
                "SQL preview · {n} statement{}",
                if n == 1 { "" } else { "s" }
            ),
            vec![Prop::new("Applied on", "Save · one transaction").tone(Tone::Muted)],
            sqls,
            None,
            Button::secondary(PREVIEW_DIALOG.sub("close"), "Close"),
        );
        d.actions.remove(0);
        d.cancel_index = Some(0);
        d.initial_focus = d.actions[0].id;
        d.width = 90;
        self.open_dialog(d);
    }

    fn open_layer(
        &mut self,
        layer: Box<dyn junie_tui::ui::layer::Layer<ModalResult>>,
        tag: LayerTag,
    ) {
        self.layers.open(layer, tag, (), &mut self.shell.focus);
        self.shell.pointer.hover = None;
        self.shell.pointer.pressed = None;
    }

    fn open_dialog(&mut self, mut d: Dialog) {
        CHROME.fit(&mut d);
        self.open_layer(Box::new(d), LayerTag::new("dialog"));
    }

    fn open_help(&mut self) {
        use junie_tui::widgets::help::{HelpOverlay, HelpSection};
        let shell = self
            .keys
            .help_section("Shell and workbench", |c| Some(c.label()));
        let page = HelpSection::new("Grid and editor")
            .row("Enter F2", "Edit a cell · Esc cancels")
            .row("Ctrl+F", "Filter (grid) or find (editor)")
            .row("p", "Preview the SQL of pending changes")
            .row("z", "Zoom the focused pane")
            .row("Shift+F10", "Menu of the focused row, cell or tab")
            .row("Right-click", "The same menu at the pointer");
        let h = HelpOverlay::from_keymaps(HELP_DIALOG, "TablePro", vec![shell, page]);
        self.open_layer(Box::new(h), LayerTag::new("help"));
    }

    fn open_switcher(&mut self) {
        let Some(w) = self.workbench.as_ref() else {
            return;
        };
        let open: Vec<(usize, String)> = w
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.label()))
            .collect();
        let index = SwitcherIndex::build(&w.catalog, &w.connection.name, &open, &self.history);
        let mut p = Picker::new(WidgetId::of("switcher"), "Open Quickly");
        p.placeholder = "Search tables, views, schemas, tabs, queries…".into();
        p.set_width(88);
        p.hints =
            "↑↓ Move · Enter Open · Alt+Enter New tab · Tab Scope · Esc Clear / Close".to_owned();
        self.scope = 0;
        p.scope = Some("All · Tab scope".into());
        self.switcher_index = Some(index);
        self.layers.open(
            Box::new(p),
            LayerTag::new(SWITCHER),
            (),
            &mut self.shell.focus,
        );
        self.refresh_switcher();
    }

    fn refresh_switcher(&mut self) {
        if self.top_kind() != Some(SWITCHER) {
            return;
        }
        let (Some(p), Some(index)) = (
            self.layers.top_as_mut::<Picker>(),
            self.switcher_index.as_ref(),
        ) else {
            return;
        };
        let scopes = ["All", "Tables", "Schemas", "Queries"];
        p.scope = Some(format!("{} · Tab scope", scopes[self.scope]));
        let mut results = index.query(p.query());
        results.retain(|r| match self.scope {
            1 => r.group == "Tables" || r.group == "Views",
            2 => r.group == "Schemas" || r.group == "Databases",
            3 => r.group == "Recent queries",
            _ => true,
        });
        if p.query().is_empty() && self.scope == 0 {
            results.truncate(24);
        }
        let items: Vec<Item> = results
            .iter()
            .map(|r| {
                let it = Item::new(r.label.clone())
                    .detail(r.path.clone())
                    .glyph(match r.group {
                        "Tables" => "T",
                        "Views" => "V",
                        "Schemas" => "S",
                        "Databases" => "D",
                        "Open tabs" => "≡",
                        _ => "Q",
                    })
                    .group(r.group)
                    .matched(r.matched.clone());
                if r.open && r.group != "Open tabs" {
                    it.tag("open")
                } else {
                    it
                }
            })
            .collect();
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));

        self.switcher_targets = results.into_iter().map(|r| r.target).collect();
    }

    fn open_tab_list(&mut self) {
        let Some(w) = self.workbench.as_ref() else {
            return;
        };
        let mut p = Picker::new(WidgetId::of("tablist"), "Open tabs");
        p.placeholder = "Filter tabs…".into();
        p.hints = "↑↓ Move · Enter Switch · Delete Close tab · Esc Close".to_owned();
        p.set_items(tab_list_items(w, ""))
            .unwrap_or_else(|error| p.report_item_error(error));
        p.set_cursor(w.active);
        self.layers.open(
            Box::new(p),
            LayerTag::new(TAB_LIST),
            (),
            &mut self.shell.focus,
        );
    }

    fn open_safe_mode_picker(&mut self) {
        let Some(w) = self.workbench.as_ref() else {
            return;
        };
        let current = w.connection.safe_mode;
        let mut p = Picker::new(WidgetId::of("safemode"), "Safe Mode · this connection");
        p.set_searchable(false)
            .expect("disabling picker search is valid");
        p.set_width(112);
        p.hints =
            "↑↓ Move · Enter Set level · Esc Keep · levels are saved to the connection".to_owned();
        let items: Vec<Item> = SafeMode::ALL
            .iter()
            .map(|s| {
                let it = Item::new(s.label())
                    .detail(s.description())
                    .glyph(if *s == current { "›" } else { " " });
                if *s == current { it.tag("current") } else { it }
            })
            .collect();
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        p.set_cursor(
            SafeMode::ALL
                .iter()
                .position(|s| *s == current)
                .unwrap_or(0),
        );
        p.empty_text = String::new();
        self.layers.open(
            Box::new(p),
            LayerTag::new(SAFE_MODE),
            (),
            &mut self.shell.focus,
        );
    }

    fn open_filter_editor(
        &mut self,
        index: Option<usize>,
        prefill: Option<(usize, FilterOp, String)>,
    ) {
        let Some(w) = self.workbench.as_ref() else {
            return;
        };
        let Some(WorkTab::Table(t)) = w.active_tab() else {
            self.set_status("Filters apply to table tabs".into());
            return;
        };
        let columns = t.columns.clone();
        let existing = index.and_then(|i| t.filters.get(i)).cloned();
        let (col_i, op, value, value2) = match (&existing, &prefill) {
            (Some(f), _) => (
                columns.iter().position(|c| c.0 == f.column).unwrap_or(0),
                f.op,
                f.value.clone(),
                f.value2.clone(),
            ),
            (None, Some((c, op, v))) => (*c, *op, v.clone(), String::new()),
            _ => (
                t.grid.cursor.1.min(columns.len().saturating_sub(1)),
                FilterOp::Eq,
                String::new(),
                String::new(),
            ),
        };
        let update = existing.is_some();
        let editor = FilterEditor::new(index, update, columns, col_i, op, &value, &value2);
        self.layers.open(
            Box::new(editor),
            LayerTag::new("filter"),
            (),
            &mut self.shell.focus,
        );
    }

    fn request_close_tab(&mut self, index: usize) {
        let Some(w) = self.workbench.as_mut() else {
            return;
        };
        if w.tabs.get(index).is_some_and(|t| t.dirty()) {
            self.act(Act::ConfirmCloseTab(index));
        } else {
            w.close_tab(index);
            let focus = w.primary_focus().unwrap_or(crate::workbench::EXPLORER);
            self.shell.focus.focus(focus);
        }
    }

    fn top_kind(&self) -> Option<&'static str> {
        self.layers.top().map(|e| e.tag.kind)
    }

    fn layer_key(&mut self, key: Key) -> Outcome {
        let Some(top) = self.layers.top_mut() else {
            return Outcome::Ignored;
        };
        let mut cx = LayerCx {
            focus: &mut self.shell.focus,
            ring: &self.shell.ring,
            app: &(),
        };
        let out = top.layer.on_key(&key, &mut cx);
        if out == Outcome::Ignored && key.is(KeyCode::Esc) {
            return self.dismiss_layer(DismissReason::Esc);
        }
        self.take_layer().or(out)
    }

    fn take_layer(&mut self) -> Outcome {
        match self.layers.take(&mut self.shell.focus) {
            None => Outcome::Ignored,
            Some(Taken::Event(r)) => self.layer_event(r),
            Some(Taken::Closed(entry, r)) => self.layer_closed(entry, r),
        }
    }

    fn dismiss_layer(&mut self, reason: DismissReason) -> Outcome {
        let dismissed = self.layers.dismiss(reason, &mut self.shell.focus);
        self.dismissed(reason, dismissed)
    }

    fn dismissed(&mut self, reason: DismissReason, d: Dismissed<ModalResult>) -> Outcome {
        match d {
            Dismissed::None => Outcome::Ignored,
            Dismissed::Refused => {
                if reason == DismissReason::OutsideClick
                    && let Some(top) = self.layers.top()
                {
                    let dismiss = top.layer.spec().dismiss;
                    let now = self.now_ms();
                    junie_tui::ui::modal::refuse_outside_click(
                        &mut self.shell.pointer,
                        dismiss,
                        now,
                    );
                    return Outcome::Changed;
                }
                Outcome::Consumed
            }
            Dismissed::Closed(entry, Some(r)) => self.layer_closed(entry, r).or(Outcome::Changed),
            Dismissed::Closed(_, None) => Outcome::Changed,
        }
    }

    fn layer_event(&mut self, r: ModalResult) -> Outcome {
        let ModalResult::Lib(LayerResult::Picker(ev)) = r else {
            return Outcome::Ignored;
        };
        let kind = self.top_kind();
        match ev {
            PickerAction::QueryChanged => {
                if kind == Some(SWITCHER) {
                    self.refresh_switcher();
                } else if kind == Some(TAB_LIST) {
                    self.filter_tab_list();
                }
                Outcome::Changed
            }
            PickerAction::NextScope | PickerAction::PrevScope => {
                if kind == Some(SWITCHER) {
                    let step = if ev == PickerAction::PrevScope { 3 } else { 1 };
                    self.scope = (self.scope + step) % 4;
                    self.refresh_switcher();
                }
                Outcome::Changed
            }
            PickerAction::Secondary(i) => {
                if kind == Some(TAB_LIST) {
                    let idx = self
                        .layers
                        .top_as::<Picker>()
                        .map_or(i, |p| tab_index(p, i));
                    // Pop the picker first so the close guard restores the
                    // workbench's focus when cancelled or confirmed.
                    self.layers.pop(&mut self.shell.focus);
                    self.request_close_tab(idx);
                    if self.layers.is_empty() {
                        self.open_tab_list();
                    }
                }
                Outcome::Changed
            }

            _ => Outcome::Ignored,
        }
    }

    fn filter_tab_list(&mut self) {
        let Some(w) = self.workbench.as_ref() else {
            return;
        };
        let Some(p) = self.layers.top_as_mut::<Picker>() else {
            return;
        };
        let items = tab_list_items(w, p.query());
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
    }

    fn layer_closed(&mut self, entry: LayerEntry<ModalResult>, r: ModalResult) -> Outcome {
        match r {
            ModalResult::Lib(LayerResult::Dialog { action, .. }) => {
                let result = action.map_or(DialogResult::Cancelled, DialogResult::Action);
                match entry.get::<Dialog>() {
                    Some(d) => self.dialog_closed(d.id, result),
                    None => Outcome::Changed,
                }
            }
            ModalResult::Lib(LayerResult::Picker(
                ev @ (PickerAction::Chosen(i) | PickerAction::ChosenAlt(i)),
            )) => {
                let alt = matches!(ev, PickerAction::ChosenAlt(_));
                self.picker_chosen(&entry, i, alt)
            }
            ModalResult::Lib(LayerResult::Menu(MenuAction::Chosen(i))) => {
                match entry.get::<CtxMenu>() {
                    Some(m) => self.context_chosen(m, i),
                    None => Outcome::Changed,
                }
            }
            ModalResult::Filter(Some((index, filter))) => self.apply_filter(index, filter),
            _ => Outcome::Changed,
        }
    }

    fn picker_chosen(&mut self, entry: &LayerEntry<ModalResult>, i: usize, alt: bool) -> Outcome {
        match entry.tag.kind {
            SWITCHER => {
                self.switcher_index = None;
                let target = self.switcher_targets.get(i).cloned();
                let Some(target) = target else {
                    return Outcome::Changed;
                };
                let Some(w) = self.workbench.as_mut() else {
                    return Outcome::Changed;
                };
                match target {
                    SwitchTarget::Table { schema, name } | SwitchTarget::View { schema, name } => {
                        w.open_table(&schema, &name, false);
                        let _ = alt;
                        if let Some(pf) = w.primary_focus() {
                            self.shell.focus.focus(pf);
                        }

                        self.set_status(format!("Opened {schema}.{name}"));
                    }
                    SwitchTarget::Schema(s) => {
                        w.schema = s.clone();
                        w.explorer_filter = TextInput::new(w.explorer_filter.id, "")
                            .placeholder("Filter objects")
                            .plain_label();
                        w.explorer.set_filter(None);
                        let _ = w.explorer.rows();

                        let conn = w.connection.clone();
                        let cat = w.catalog.clone();
                        let tabs = std::mem::take(&mut w.tabs);
                        let active = w.active;
                        let qc = w.query_counter;
                        let mut nw = Workbench::new(conn, cat);
                        nw.tabs = tabs;
                        nw.active = active;
                        nw.query_counter = qc;
                        nw.schema = s.clone();
                        nw.rebuild_for_schema();
                        *w = nw;
                        self.shell.focus.focus(crate::workbench::EXPLORER);
                        self.set_status(format!("Schema {s}"));
                    }
                    SwitchTarget::Database(_) => {
                        self.shell.focus.focus(crate::workbench::EXPLORER);
                    }
                    SwitchTarget::OpenTab(i) => {
                        w.set_active(i);
                        if let Some(pf) = w.primary_focus() {
                            self.shell.focus.focus(pf);
                        }
                    }
                    SwitchTarget::RecentQuery(id) => {
                        let sql = self
                            .history
                            .entries
                            .iter()
                            .find(|e| e.id == id)
                            .map(|e| e.sql.clone())
                            .unwrap_or_default();
                        w.new_query(&sql);
                        if let Some(pf) = w.primary_focus() {
                            self.shell.focus.focus(pf);
                        }
                    }
                }
                Outcome::Changed
            }
            TAB_LIST => {
                let idx = entry.get::<Picker>().map_or(i, |p| tab_index(p, i));
                if let Some(w) = self.workbench.as_mut() {
                    w.set_active(idx);
                    if let Some(pf) = w.primary_focus() {
                        self.shell.focus.focus(pf);
                    }
                }
                Outcome::Changed
            }
            SAFE_MODE => {
                let level = SafeMode::ALL[i.min(5)];
                if let Some(w) = self.workbench.as_mut() {
                    w.connection.safe_mode = level;
                    let name = w.connection.name.clone();
                    if let Some(c) = self
                        .connections
                        .connections
                        .iter_mut()
                        .find(|c| c.name == name)
                    {
                        c.safe_mode = level;
                    }
                }
                self.set_status_as(
                    StatusKind::Ok,
                    format!(
                        "Safety level set to {} · saved to the connection",
                        level.label()
                    ),
                );
                Outcome::Changed
            }
            _ => Outcome::Changed,
        }
    }

    fn apply_filter(&mut self, index: Option<usize>, filter: Filter) -> Outcome {
        let mut applied = None;
        if let Some(w) = self.workbench.as_mut() {
            let cat = w.catalog.clone();
            if let Some(WorkTab::Table(t)) = w.active_tab_mut() {
                match index {
                    Some(i) if i < t.filters.len() => t.filters[i] = filter,
                    _ => t.filters.push(filter),
                }
                t.preview = false;
                t.load(&cat);
                let n = t.filters.iter().filter(|f| f.enabled).count();
                applied = Some((n, t.grid.id));
            }
        }
        if let Some((n, gid)) = applied {
            self.set_status(format!(
                "{n} filter{} applied",
                if n == 1 { "" } else { "s" }
            ));
            self.shell.focus.focus(gid);
        }
        Outcome::Changed
    }

    fn dialog_closed(&mut self, id: WidgetId, result: DialogResult) -> Outcome {
        if self.connections.leave.is_dialog(id) {
            let mut quit = false;
            let (_, reqs) = self.shell.deliver(true, |cx| {
                quit = self.connections.resolve_leave(result, cx);
                Outcome::Ignored
            });
            self.apply_requests(reqs);
            if quit {
                return self.request_quit().or(Outcome::Changed);
            }
            return Outcome::Changed;
        }
        if self.connections.on_dialog_closed(id, result) {
            return Outcome::Changed;
        }
        if self.leave.is_dialog(id) {
            return match self.leave.resolve(result) {
                Resolved::Discard(Leave::Quit) => {
                    self.shell.quit = true;
                    Outcome::Changed
                }
                Resolved::Discard(Leave::To(nav)) => self.go(nav).or(Outcome::Changed),

                Resolved::Save(_) | Resolved::Stay => Outcome::Changed,
            };
        }
        if id == SAFETY_DIALOG {
            let Some(w) = self.workbench.as_mut() else {
                return Outcome::Changed;
            };
            let pending = w.pending_run.take();
            if result == DialogResult::Action(1) {
                if let Some((tab, statements, all, explain)) = pending
                    && let Some(WorkTab::Query(q)) = w.tabs.get_mut(tab)
                {
                    q.start(statements, all, explain);
                }
            } else {
                self.set_status_as(StatusKind::Info, "Cancelled · nothing was executed".into());
            }
            return Outcome::Changed;
        }
        if id == COMMIT_DIALOG {
            if result == DialogResult::Action(1) {
                self.committing = Some(5);
                self.set_status("Saving…".into());
            } else {
                self.set_status("Changes kept pending".into());
            }
            return Outcome::Changed;
        }
        if id == DISCARD_DIALOG {
            if result == DialogResult::Action(1) {
                if let Some(w) = self.workbench.as_mut() {
                    let cat = w.catalog.clone();
                    if let Some(WorkTab::Table(t)) = w.active_tab_mut() {
                        t.grid.discard();
                        t.load(&cat);
                    }
                }
                self.set_status("Changes discarded".into());
            }
            return Outcome::Changed;
        }
        if let Some(w) = self.workbench.as_mut() {
            for i in 0..w.tabs.len() + 1 {
                if id == CLOSE_DIALOG.child(i) {
                    if result == DialogResult::Action(1) {
                        w.close_tab(i);
                        let pf = w.primary_focus().unwrap_or(crate::workbench::EXPLORER);
                        self.shell.focus.focus(pf);
                    }
                    return Outcome::Changed;
                }
            }
        }
        Outcome::Changed
    }

    fn deliver_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        if !self.layers.is_empty() {
            return self.layer_pointer(ev);
        }
        if ev.is(Phase::Secondary) {
            return self.on_secondary(ev);
        }
        if ev.is(Phase::Click)
            && let Some(o) = self.chrome_click(ev)
        {
            return o;
        }
        self.with_page(|p, h, cx| p.pointer(ev, h, cx))
    }

    fn layer_pointer(&mut self, ev: &PointerEvent) -> Outcome {
        if ev.is(Phase::Click) && ev.id == junie_tui::ui::modal::CLOSE {
            return self.dismiss_layer(DismissReason::CloseButton);
        }
        match self
            .layers
            .pointer(ev, &mut self.shell.focus, &self.shell.ring, &())
        {
            LayerPointer::Handled(out) => self.take_layer().or(out),
            LayerPointer::Dismissed(d) => self.dismissed(DismissReason::OutsideClick, d),
        }
    }

    fn chrome_click(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        let id = ev.id;

        if let Some(key) = self.shell.footer_keys.clicked(ev) {
            return Some(shell::KeyRouter::replay(self, &key).or(Outcome::Changed));
        }

        if let Some(r) = self.workbench.as_mut().and_then(|w| w.rail.route(ev)) {
            let mut out = Outcome::Consumed;
            for a in r.into_actions() {
                out = self.set_explorer(a == junie_tui::widgets::rail::RailAction::Expand);
            }
            return Some(out);
        }

        if let Some(action) = crate::tabs::toolbar_action(id) {
            use crate::tabs::ToolbarAction as A;
            return Some(match action {
                A::Run => self.run_active(false, None),
                A::RunAll => self.run_active(true, None),
                A::Explain => self.run_active(false, Some(false)),
                A::Cancel => self.cancel_running(),
            });
        }
        if id == STRIP_HELP {
            self.open_help();
            return Some(Outcome::Changed);
        }
        if id == STRIP_SAFE {
            self.open_safe_mode_picker();
            return Some(Outcome::Changed);
        }
        if id == STRIP_SCOPE {
            self.open_switcher();
            self.scope = 2;
            self.refresh_switcher();
            return Some(Outcome::Changed);
        }

        if id == STRIP_APP || id == STRIP_CONN {
            let name = match (self.route(), self.workbench.as_ref()) {
                (Route::Workbench, Some(w)) => Some(w.connection.name.clone()),
                _ => None,
            };
            let at = self
                .shell
                .hits
                .area_of(id)
                .map(|r| Position::new(r.x, r.y))
                .unwrap_or(ev.pos());
            let menu = context::app_menu(name.as_deref(), at);
            return Some(self.open_context(menu));
        }
        None
    }

    fn on_secondary(&mut self, ev: &PointerEvent) -> Outcome {
        if !self.layers.is_empty() {
            return Outcome::Consumed;
        }
        let cfg = self.context;
        let menu = match (self.route(), self.workbench.as_mut()) {
            (Route::Workbench, Some(w)) => {
                let menu = context::on_secondary(w, &cfg, ev);

                if let Some(WorkTab::History(h)) = w.active_tab_mut() {
                    h.sync_detail_public(&self.history);
                }
                menu
            }
            (Route::Connections, _) => {
                context::on_secondary_connections(&mut self.connections, &cfg, ev)
            }
            _ => None,
        };
        match menu {
            Some(menu) => self.open_context(menu),
            None => Outcome::Ignored,
        }
    }

    fn open_context(&mut self, menu: CtxMenu) -> Outcome {
        self.shell.focus.focus(menu.owner);
        self.layers.open(
            Box::new(menu),
            LayerTag::new("menu"),
            (),
            &mut self.shell.focus,
        );
        self.shell.pointer.pressed = None;
        Outcome::Changed
    }

    fn context_chosen(&mut self, m: &CtxMenu, i: usize) -> Outcome {
        let (key, target, owner) = (m.keys.get(i).copied().flatten(), m.target, m.owner);
        let header = m.header.get(i).copied();
        let app = m.app.clone();

        if let (context::Target::Header(col), Some(action)) = (target, header) {
            self.shell.focus.focus(owner);
            let (o, reqs) = self
                .shell
                .deliver(true, |cx| match self.workbench.as_mut() {
                    Some(w) => w.on_header_action(col, action, cx, &mut self.history),
                    None => Outcome::Ignored,
                });
            return self.apply_requests(reqs).or(o).or(Outcome::Changed);
        }
        if let context::Target::App = target {
            return match app.get(0, i) {
                Some(context::AppCommand::OpenTabs) => {
                    self.open_tab_list();
                    Outcome::Changed
                }
                Some(context::AppCommand::Disconnect) => self.disconnect(),
                Some(context::AppCommand::Quit) => self.request_quit(),
                None => Outcome::Changed,
            };
        }
        let Some(key) = key else {
            return Outcome::Changed;
        };
        match target {
            context::Target::Connection(row) => self.connections.select_row(row),
            _ => {
                if let Some(w) = self.workbench.as_mut() {
                    context::select(w, target);
                }
            }
        }
        self.shell.focus.focus(owner);
        shell::KeyRouter::replay(self, &key).or(Outcome::Changed)
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let shell::Regions {
            header,
            body,
            footer,
        } = CHROME.regions(area);
        self.draw_strip(header, buf, ctx);
        let (page, history) = self.shown_mut();
        if let Some(p) = page {
            p.render(body, buf, ctx, history);
        }
        self.footer().render(footer, buf, ctx, &mut self.shell);

        let mut layers = std::mem::take(&mut self.layers);
        for e in layers.iter_mut() {
            e.layer.render(area, buf, ctx, &());
        }
        self.layers = layers;
    }

    fn draw_strip(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        let mut left = vec![
            Segment::new("▪", Tone::Active).priority(9),
            Segment::new("TablePro", Tone::Normal)
                .bold()
                .clickable(STRIP_APP)
                .priority(9),
        ];
        let mut right = vec![];
        match self.route() {
            Route::Connections => {
                left.push(Segment::new("Connections", Tone::Secondary).priority(8));
                let n = self.connections.connections.len();
                left.push(Segment::new(format!("{n} saved"), Tone::Muted).priority(3));
            }
            Route::Workbench => {
                if let Some(w) = &self.workbench {
                    let c = &w.connection;
                    let env = c.environment;

                    let kind = env.kind();
                    let glyph = t.glyphs.env(kind);
                    let label = if glyph.is_empty() {
                        env.label().to_owned()
                    } else {
                        format!("{glyph} {}", env.label())
                    };
                    let mut env_seg = Segment::new(label, kind.tone());
                    if t.env(kind)
                        .1
                        .add_modifier
                        .contains(ratatui::style::Modifier::BOLD)
                    {
                        env_seg = env_seg.bold();
                    }
                    left.push(
                        Segment::new(
                            junie_tui::ui::text::truncate_middle(&c.name, 18),
                            Tone::Normal,
                        )
                        .bold()
                        .clickable(STRIP_CONN)
                        .priority(9),
                    );
                    left.push(env_seg.priority(8));
                    left.push(
                        Segment::new(
                            format!("{} › {}", w.catalog.database, w.schema),
                            Tone::Secondary,
                        )
                        .clickable(STRIP_SCOPE)
                        .priority(7),
                    );
                    let level = c.safe_mode;

                    let (status, bold) = match level {
                        SafeMode::Silent if env == Environment::Production => {
                            (Some(StatusKind::Warn), true)
                        }
                        SafeMode::Safe | SafeMode::SafeFull => (Some(StatusKind::Ok), false),
                        SafeMode::ReadOnly => (None, true),
                        _ => (None, false),
                    };
                    let tone = match (status, level) {
                        (Some(kind), _) => kind.tone(),
                        (None, SafeMode::Silent) => Tone::Faint,
                        (None, SafeMode::Alert | SafeMode::AlertFull) => Tone::Secondary,
                        (None, _) => Tone::Normal,
                    };
                    let text = match status {
                        Some(kind) => format!("{} {}", t.status(kind).0, level.token()),
                        None => level.token().to_owned(),
                    };
                    let mut s = Segment::new(text, tone).clickable(STRIP_SAFE).priority(8);
                    if bold {
                        s = s.bold();
                    }
                    left.push(s);
                    if w.running().is_some() {
                        right.push(
                            Segment::new(
                                format!(
                                    "{} running",
                                    junie_tui::widgets::progress::spinner_frame(
                                        ctx.interaction.tick
                                    ),
                                ),
                                Tone::Secondary,
                            )
                            .priority(9),
                        );
                    }
                    let pending = w.pending_total();
                    if pending > 0 {
                        right.push(
                            Segment::new(format!("• {pending} pending"), Tone::Warning).priority(8),
                        );
                    }
                }
            }
        }
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
        right.push(
            Segment::new("? help", Tone::Muted)
                .clickable(STRIP_HELP)
                .priority(4),
        );
        segments::render(area, buf, ctx, &left, &right, t.canvas);
    }

    fn footer(&self) -> shell::Footer {
        let focus = self.shell.focus.current();
        let editing = self.shown().is_some_and(|p| p.is_editing());
        let open = !self.layers.is_empty();
        let hints = match self.layers.top() {
            Some(e) => e.layer.hints(focus),
            None => self
                .shown()
                .map(|p| p.hints(focus, &self.history))
                .unwrap_or_default(),
        };
        shell::Footer::new()
            .layer(Some(hints))
            .editing(editing && !open)
            .tab_next(!open && !editing)
            .status_kind(
                self.shell
                    .status
                    .entry()
                    .map(|(text, _, kind)| (text, *kind)),
            )
    }
}

impl shell::App for App {
    type Status = Option<StatusKind>;
    fn shell(&self) -> &Shell<Option<StatusKind>> {
        &self.shell
    }
    fn shell_mut(&mut self) -> &mut Shell<Option<StatusKind>> {
        &mut self.shell
    }
    fn theme(&self) -> Theme {
        self.theme
    }
    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new("TablePro")
    }
    fn now_ms(&self) -> u64 {
        App::now_ms(self)
    }
    fn flash_ms(&self) -> u64 {
        self.flash_clock()
    }
    fn tick(&self) -> u64 {
        self.tick
    }
    fn tick_interval(&self) -> Duration {
        App::tick_interval(self)
    }
    fn advance_clock(&mut self, ms: u64) -> Option<Outcome> {
        Some(junie_tui::clock::step_wall(
            self,
            |a| &mut a.clock,
            ms,
            |a| a.tick_interval().as_millis() as u64,
            Self::on_tick,
        ))
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        App::draw(self, area, buf, ctx)
    }
    fn on_input(&mut self, input: Input) -> Outcome {
        self.handle_input(input)
    }

    fn on_blur(&mut self, ch: FocusChange) -> Outcome {
        let mut out = self.layers.on_blur(ch);

        let connections = self.route() == Route::Connections;
        let (o, front) = self.shell.deliver(connections, |cx| {
            Page::on_blur(&mut self.connections, ch, &mut self.history, cx)
        });
        out = out.or(o);
        let (o, back) = self
            .shell
            .deliver(!connections, |cx| match self.workbench.as_mut() {
                Some(w) => Page::on_blur(w, ch, &mut self.history, cx),
                None => Outcome::Ignored,
            });
        out = out.or(o).or(self.apply_requests(front));
        out.or(self.apply_requests(back))
    }
    fn layer_open(&self) -> bool {
        !self.layers.is_empty()
    }
    fn page_under_layer(&self) -> bool {
        self.layers.hides_page_focus()
    }
    fn page_key(&self) -> PageKey {
        self.router.key()
    }

    fn stakes(&mut self) -> Stakes {
        let form = Page::stakes(&mut self.connections, &self.history);
        let work = self
            .workbench
            .as_mut()
            .map(|w| Page::stakes(w, &self.history))
            .unwrap_or_default();
        form.max(work)
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
        if self.layers.is_empty() {
            return Response::changed();
        }
        self.dismiss_layer(DismissReason::OutsideClick)
            .or(Outcome::Changed)
            .into()
    }
}

fn tab_index(p: &Picker, i: usize) -> usize {
    p.items().get(i).map_or(i, |it| it.value)
}

fn tab_list_items(w: &Workbench, query: &str) -> Vec<Item> {
    let q = query.to_lowercase();
    w.tabs
        .iter()
        .enumerate()
        .filter(|(_, t)| q.is_empty() || t.label().to_lowercase().contains(&q))
        .map(|(i, t)| {
            let it = Item::new(t.label())
                .detail(match t {
                    WorkTab::Table(tt) => format!(
                        "{} · {}",
                        tt.qualified(),
                        if tt.mode_tabs.active == 0 {
                            "data"
                        } else {
                            "structure"
                        }
                    ),
                    WorkTab::Query(q) => {
                        if q.is_running() {
                            "running".into()
                        } else {
                            q.last_status
                                .as_ref()
                                .map(|s| s.0.clone())
                                .unwrap_or("query".into())
                        }
                    }
                    WorkTab::History(_) => "history".into(),
                })
                .glyph(match t {
                    WorkTab::Table(_) => "T",
                    WorkTab::Query(_) => "≡",
                    WorkTab::History(_) => "H",
                })
                .value(i);
            if i == w.active {
                it.tag("active")
            } else if t.dirty() {
                it.tag("unsaved")
            } else {
                it
            }
        })
        .collect()
}

fn pretty_json(j: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut in_str = false;
    for c in j.chars() {
        match c {
            '"' => {
                in_str = !in_str;
                out.push(c);
            }
            '{' | '[' if !in_str => {
                depth += 1;
                out.push(c);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
            }
            '}' | ']' if !in_str => {
                depth = depth.saturating_sub(1);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
                out.push(c);
            }
            ',' if !in_str => {
                out.push(c);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
            }
            ' ' if !in_str => {}
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpCmd {
    NewConnection,

    CancelQuery,
    Run,
    RunOrRefresh,
    RunAll,
    Explain,
    ExplainAnalyze,
    NewQuery,
    CloseTab,
    Switcher,
    TabList,
    History,
    ToggleExplorer,
    SafeMode,
    ToggleStructure,
    Commit,
    Find,
    Maximize,
    GrowEditor,
    GrowResults,

    Explorer,
}

impl TpCmd {
    pub fn label(self) -> &'static str {
        match self {
            TpCmd::NewConnection => "New connection",
            TpCmd::CancelQuery => "Cancel the running query",
            TpCmd::Run => "Run the statement at the cursor",
            TpCmd::RunOrRefresh => "Run, or refresh the table",
            TpCmd::RunAll => "Run all statements",
            TpCmd::Explain => "EXPLAIN",
            TpCmd::ExplainAnalyze => "EXPLAIN ANALYZE",
            TpCmd::NewQuery => "New query tab",
            TpCmd::CloseTab => "Close tab",
            TpCmd::Switcher => "Open Quickly",
            TpCmd::TabList => "Tab list",
            TpCmd::History => "Query history",
            TpCmd::ToggleExplorer => "Show or hide the explorer",
            TpCmd::SafeMode => "Safe Mode",
            TpCmd::ToggleStructure => "Data / Structure",
            TpCmd::Commit => "Save pending changes",
            TpCmd::Find => "Filter or find",
            TpCmd::Maximize => "Maximize the pane",
            TpCmd::GrowEditor => "Grow the editor",
            TpCmd::GrowResults => "Grow the results",
            TpCmd::Explorer => "Focus the explorer",
        }
    }
}

pub const RUN_ALL: [junie_tui::core::keymap::KeyBinding; 2] = [
    junie_tui::core::keymap::KeyBinding::alt('r'),
    junie_tui::core::keymap::KeyBinding::new(
        KeyCode::F(5),
        ratatui::crossterm::event::KeyModifiers::SHIFT,
    ),
];

pub const EXPLAIN_ANALYZE: [junie_tui::core::keymap::KeyBinding; 2] = [
    junie_tui::core::keymap::KeyBinding::alt('x'),
    junie_tui::core::keymap::KeyBinding::new(
        KeyCode::F(7),
        ratatui::crossterm::event::KeyModifiers::SHIFT,
    ),
];

pub fn key_label(keys: &[junie_tui::core::keymap::KeyBinding]) -> &'static str {
    let map: Vec<_> = keys.iter().map(|&b| (b, ())).collect();
    junie_tui::core::keymap::hint_key(&map, ()).unwrap_or("")
}

pub fn shell_keys() -> shell::ShellKeys<TpCmd> {
    use junie_tui::core::keymap::KeyBinding;
    use ratatui::crossterm::event::KeyModifiers;
    use shell::ShellAction::{self, App as Cmd};
    let mut keys = shell::ShellKeys::new();
    let before = [
        (KeyBinding::key(KeyCode::Esc), TpCmd::CancelQuery),
        (KeyBinding::ctrl('r'), TpCmd::Run),
        (KeyBinding::ctrl('R'), TpCmd::Run),
        (KeyBinding::key(KeyCode::F(5)), TpCmd::RunOrRefresh),
        (RUN_ALL[0], TpCmd::RunAll),
        (RUN_ALL[1], TpCmd::RunAll),
        (KeyBinding::ctrl('x'), TpCmd::Explain),
        (EXPLAIN_ANALYZE[0], TpCmd::ExplainAnalyze),
        (EXPLAIN_ANALYZE[1], TpCmd::ExplainAnalyze),
        (KeyBinding::ctrl('t'), TpCmd::NewQuery),
        (KeyBinding::ctrl('w'), TpCmd::CloseTab),
        (KeyBinding::ctrl('o'), TpCmd::Switcher),
        (KeyBinding::ctrl('p'), TpCmd::Switcher),
        (KeyBinding::ctrl('g'), TpCmd::TabList),
        (KeyBinding::ctrl('y'), TpCmd::History),
        (KeyBinding::ctrl('b'), TpCmd::ToggleExplorer),
        (KeyBinding::ctrl('l'), TpCmd::SafeMode),
        (KeyBinding::ctrl('d'), TpCmd::ToggleStructure),
        (KeyBinding::ctrl('s'), TpCmd::Commit),
        (KeyBinding::ctrl('f'), TpCmd::Find),
        (KeyBinding::char('z'), TpCmd::Maximize),
        (
            KeyBinding::new(KeyCode::Up, KeyModifiers::CONTROL),
            TpCmd::GrowResults,
        ),
        (
            KeyBinding::new(KeyCode::Down, KeyModifiers::CONTROL),
            TpCmd::GrowEditor,
        ),
    ];
    keys.priority = before.into_iter().map(|(b, c)| (b, Cmd(c))).collect();
    keys = keys
        .bind(KeyBinding::ctrl('n'), Cmd(TpCmd::NewConnection))
        .bind_priority(KeyBinding::char('['), ShellAction::PrevPage)
        .bind_priority(KeyBinding::char(']'), ShellAction::NextPage)
        .bind_fallback(KeyBinding::char('0'), Cmd(TpCmd::Explorer))
        .dirty_letter_note(None);
    for c in [
        TpCmd::CloseTab,
        TpCmd::SafeMode,
        TpCmd::ToggleStructure,
        TpCmd::Commit,
        TpCmd::Find,
        TpCmd::RunOrRefresh,
    ] {
        keys = keys.idle_only(Cmd(c));
    }
    keys
}

impl shell::KeyHost for App {
    type Cmd = TpCmd;

    fn shell_keys(&self) -> &shell::ShellKeys<TpCmd> {
        &self.keys
    }

    fn layer_key(&mut self, key: &Key) -> Option<Outcome> {
        if self.layers.is_empty() {
            return None;
        }
        Some(App::layer_key(self, *key))
    }

    fn capturing(&self) -> bool {
        self.shown().is_some_and(|p| p.is_editing())
    }

    fn interrupt(&mut self) -> Outcome {
        self.with_page(|p, h, cx| p.on_interrupt(h, cx))
    }

    fn dirty(&mut self) -> bool {
        self.connections.form_dirty()
    }

    fn shell_action(&mut self, action: shell::ShellAction<TpCmd>, _key: &Key) -> Outcome {
        use shell::ShellAction;
        match action {
            ShellAction::Help => {
                self.open_help();
                Outcome::Changed
            }
            ShellAction::NextPage => self.step_tab(1),
            ShellAction::PrevPage => self.step_tab(-1),
            ShellAction::App(cmd) => self.run_cmd(cmd),
            _ => Outcome::Ignored,
        }
    }

    fn chrome_key(&mut self, key: &Key) -> Outcome {
        let focus = self.shell.focus.current();
        let menu = match self.route() {
            Route::Workbench => self
                .workbench
                .as_ref()
                .and_then(|w| context::on_key(w, &self.context, focus, key)),
            Route::Connections => {
                context::on_key_connections(&self.connections, &self.context, focus, key)
            }
        };
        match menu {
            Some(menu) => self.open_context(menu),
            None => Outcome::Ignored,
        }
    }

    fn page_key(&mut self, key: &Key) -> Outcome {
        self.with_page(|p, h, cx| p.on_key(key, h, cx))
    }

    fn layer_paste(&mut self, text: &str) -> Option<Outcome> {
        let top = self.layers.top_mut()?;
        let out = top.layer.on_paste(text).or(Outcome::Consumed);
        Some(self.take_layer().or(out))
    }

    fn page_paste(&mut self, text: &str) -> Outcome {
        self.with_page(|p, h, cx| p.on_paste(text, h, cx))
    }

    fn esc_top(&mut self) -> Outcome {
        self.with_page(|p, h, cx| p.on_esc_top(h, cx))
    }

    fn request_quit(&mut self) -> Outcome {
        App::request_quit(self)
    }
}
