use std::io::{Write, stdout};
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::cursor::Show;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, MouseEventKind,
};
use ratatui::crossterm::style::Colored;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::crossterm::{execute, queue};
use ratatui::style::Color;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::core::event::{External, Input, InputConfig, Key, Mouse, MouseKind, Outcome};
use crate::core::keyboard::{self, KeyboardProtocol};

pub trait Application {
    fn handle(&mut self, input: Input) -> Outcome;
    fn render(&mut self, frame: &mut Frame);
    fn should_quit(&self) -> bool;

    fn tick_interval(&self) -> Duration;

    fn coalesce(&self) -> Coalesce {
        Coalesce::default()
    }

    fn shapes(&self) -> crate::core::shape::Shapes {
        crate::core::shape::Shapes::default()
    }

    fn input_config(&self) -> InputConfig {
        InputConfig::default()
    }

    fn raw_input_policy(&self) -> RawInputPolicy {
        RawInputPolicy::default()
    }

    fn keyboard_protocol(&self) -> KeyboardProtocol {
        KeyboardProtocol::current()
    }

    fn take_output(&mut self) -> Vec<String> {
        Vec::new()
    }

    fn external(&self) -> Option<&ExternalReceiver> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ExternalSender(std::sync::mpsc::Sender<External>);

impl ExternalSender {
    pub fn send<T: std::any::Any + Send + Sync>(&self, message: T) -> bool {
        self.0.send(External::new(message)).is_ok()
    }
}

#[derive(Debug)]
pub struct ExternalReceiver {
    queue: std::sync::mpsc::Receiver<External>,
    latency: Duration,
}

impl ExternalReceiver {
    pub const DEFAULT_LATENCY: Duration = Duration::from_millis(10);

    pub fn latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    pub fn drain(&self) -> Vec<External> {
        self.queue.try_iter().collect()
    }
}

pub fn external_channel() -> (ExternalSender, ExternalReceiver) {
    let (tx, rx) = std::sync::mpsc::channel();
    (
        ExternalSender(tx),
        ExternalReceiver {
            queue: rx,
            latency: ExternalReceiver::DEFAULT_LATENCY,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coalesce {
    pub wheel: bool,

    pub pointer_moves: bool,

    pub key_repeat: bool,

    pub resize: bool,

    pub max_batch: usize,

    pub max_latency: Duration,
}

impl Coalesce {
    pub const OFF: Self = Self {
        wheel: false,
        pointer_moves: false,
        key_repeat: false,
        resize: false,
        max_batch: 1,
        max_latency: Duration::ZERO,
    };

    pub fn folds(&self, last: &Input, next: &Input) -> bool {
        match (last, next) {
            (Input::Mouse(a), Input::Mouse(b)) => {
                (self.wheel && a.wheel().is_some() && b.wheel().is_some() && a.mods == b.mods)
                    || (self.pointer_moves
                        && a.kind == MouseKind::Move
                        && b.kind == MouseKind::Move)
            }
            (Input::Key(a), Input::Key(b)) => self.key_repeat && a == b && is_navigation(a),
            (Input::Resize(..), Input::Resize(..)) => self.resize,
            _ => false,
        }
    }
}

impl Default for Coalesce {
    fn default() -> Self {
        Self {
            wheel: true,
            pointer_moves: true,
            key_repeat: true,
            resize: true,
            max_batch: 64,
            max_latency: Duration::from_millis(16),
        }
    }
}

fn is_navigation(key: &Key) -> bool {
    matches!(
        key.code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
    ) && key
        .mods
        .difference(KeyModifiers::SHIFT | KeyModifiers::CONTROL)
        .is_empty()
}

pub struct TerminalSession {
    terminal: ratatui::DefaultTerminal,
    restoration: Restoration,
}

struct Restoration {
    restore: fn(),
    active: bool,
}

impl Restoration {
    fn leave(&mut self) {
        if self.active {
            self.active = false;
            (self.restore)();
        }
    }

    fn reacquire(&mut self) {
        self.active = true;
    }
}

impl Drop for Restoration {
    fn drop(&mut self) {
        self.leave();
    }
}

fn initialize_with_restoration<T>(
    restore: fn(),
    initialize: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<(T, Restoration)> {
    let restoration = Restoration {
        restore,
        active: true,
    };
    Ok((initialize()?, restoration))
}

const ENABLE_WRAP: &str = "\x1b[?7h";

impl TerminalSession {
    pub fn enter() -> std::io::Result<Self> {
        Self::enter_with(KeyboardProtocol::current())
    }

    pub fn enter_with(protocol: KeyboardProtocol) -> std::io::Result<Self> {
        enable_raw_mode()?;
        let (terminal, restoration) = initialize_with_restoration(restore_terminal, || {
            prime_input()?;
            let mut out = stdout();
            execute!(
                out,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste,
                EnableFocusChange
            )?;

            if let Some(push) = keyboard::session::enter(protocol, || {
                ratatui::crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false)
            }) {
                out.write_all(push.as_bytes())?;
                out.flush()?;
            }
            let backend = ratatui::backend::CrosstermBackend::new(out);
            ratatui::Terminal::new(backend)
        })?;
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            previous(info);
        }));
        Ok(Self {
            terminal,
            restoration,
        })
    }

    pub fn terminal(&mut self) -> &mut ratatui::DefaultTerminal {
        &mut self.terminal
    }

    pub fn leave(&mut self) {
        self.restoration.leave();
    }

    #[cfg(unix)]
    pub fn suspend(&mut self) -> std::io::Result<(u16, u16)> {
        self.restoration.leave();
        job_control::stop_self();
        self.reenter()
    }

    #[cfg(unix)]
    fn reenter(&mut self) -> std::io::Result<(u16, u16)> {
        enable_raw_mode()?;
        let mut out = stdout();
        if let Err(e) = execute!(
            out,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            EnableFocusChange
        ) {
            restore_terminal();
            return Err(e);
        }
        if let Some(push) = keyboard::session::resume()
            && let Err(e) = out.write_all(push.as_bytes()).and_then(|()| out.flush())
        {
            restore_terminal();
            return Err(e);
        }
        self.restoration.reacquire();

        execute!(out, Clear(ClearType::All))?;
        self.terminal.swap_buffers();
        self.terminal.swap_buffers();
        let size = self.terminal.size()?;
        Ok((size.width, size.height))
    }
}

#[cfg(unix)]
pub mod job_control {
    use std::ffi::c_int;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    const SIGTSTP: c_int = 18;
    #[cfg(not(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    const SIGTSTP: c_int = 20;
    const SIG_DFL: usize = 0;

    static REQUESTED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" {
        fn signal(sig: c_int, handler: usize) -> usize;
        fn raise(sig: c_int) -> c_int;
    }

    extern "C" fn on_tstp(_sig: c_int) {
        REQUESTED.store(true, Ordering::SeqCst);
    }

    pub fn install() {
        let handler: extern "C" fn(c_int) = on_tstp;
        unsafe {
            signal(SIGTSTP, handler as *const () as usize);
        }
    }

    pub fn take_request() -> bool {
        REQUESTED.swap(false, Ordering::SeqCst)
    }

    pub fn stop_self() {
        unsafe {
            signal(SIGTSTP, SIG_DFL);
            raise(SIGTSTP);
        }
        install();
    }
}

fn restore_terminal() {
    let mut out = stdout();

    let _ = execute!(out, EndSynchronizedUpdate);
    let _ = out.write_all(crate::core::shape::reset_sequence().as_bytes());
    if let Some(pop) = keyboard::session::leave() {
        let _ = out.write_all(pop.as_bytes());
    }
    let _ = execute!(
        out,
        DisableFocusChange,
        DisableBracketedPaste,
        DisableMouseCapture,
        Show
    );
    let _ = out.write_all(ENABLE_WRAP.as_bytes());
    let _ = execute!(out, LeaveAlternateScreen);
    let _ = out.flush();
    let _ = disable_raw_mode();
}

pub fn run(app: &mut impl Application) -> std::io::Result<()> {
    let mut session = TerminalSession::enter_with(app.keyboard_protocol())?;
    #[cfg(unix)]
    job_control::install();
    let result = event_loop(&mut session, app);
    session.leave();
    result
}

fn prime_input() -> std::io::Result<()> {
    poll_uninterrupted(Duration::ZERO).map(|_| ())
}

fn first_poll_delay() -> Option<Duration> {
    std::env::var("JUNIE_TUI_FIRST_POLL_DELAY_MS")
        .ok()?
        .parse()
        .ok()
        .map(Duration::from_millis)
}

fn poll_uninterrupted(wait: Duration) -> std::io::Result<bool> {
    match poll_input(wait) {
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Ok(false),
        other => other,
    }
}

fn poll_input(wait: Duration) -> std::io::Result<bool> {
    if event::poll(Duration::ZERO)? {
        return Ok(true);
    }
    #[cfg(unix)]
    if tty_input::rearm() && event::poll(Duration::ZERO)? {
        return Ok(true);
    }
    if wait.is_zero() {
        return Ok(false);
    }
    event::poll(wait)
}

#[cfg(unix)]
mod tty_input {
    use std::mem::MaybeUninit;
    use std::os::fd::{AsRawFd, RawFd};
    use std::sync::OnceLock;

    fn fd() -> Option<RawFd> {
        static TTY: OnceLock<Option<(RawFd, Option<std::fs::File>)>> = OnceLock::new();
        TTY.get_or_init(|| {
            if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
                return Some((libc::STDIN_FILENO, None));
            }
            let tty = std::fs::File::open("/dev/tty").ok()?;
            Some((tty.as_raw_fd(), Some(tty)))
        })
        .as_ref()
        .map(|(fd, _)| *fd)
    }

    fn queued(fd: RawFd) -> bool {
        let mut p = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };

        unsafe { libc::poll(&mut p, 1, 0) > 0 && p.revents & libc::POLLIN != 0 }
    }

    pub fn rearm() -> bool {
        let Some(fd) = fd() else {
            return false;
        };
        if !queued(fd) {
            return false;
        }

        unsafe {
            if libc::tcgetpgrp(fd) != libc::getpgrp() {
                return false;
            }
            let mut attrs = MaybeUninit::<libc::termios>::uninit();
            if libc::tcgetattr(fd, attrs.as_mut_ptr()) != 0 {
                return false;
            }
            libc::tcsetattr(fd, libc::TCSANOW, attrs.as_ptr()) == 0
        }
    }
}

fn event_loop(session: &mut TerminalSession, app: &mut impl Application) -> std::io::Result<()> {
    let suppress_colors = Colored::ansi_color_disabled_memoized();
    let mut dirty = true;
    let mut last_tick = Instant::now();
    let mut pump = InputPump::new(app.coalesce());

    let mut shapes =
        crate::core::shape::ShapeSync::new(crate::core::shape::ShapeSupport::current());
    let mut hover_due: Option<(u64, Instant)> = None;
    let mut first_poll_delay = first_poll_delay();
    let mut sizes = SizeWatch::new(terminal_size(&session.terminal));
    let mut raw_pending = false;
    loop {
        #[cfg(unix)]
        if job_control::take_request() {
            let (w, h) = session.suspend()?;

            shapes.forget();

            let resize = Input::Resize(w, h);
            sizes.observe(&resize);
            app.handle(resize);
            dirty = true;
        }
        let terminal = &mut session.terminal;
        if dirty {
            draw_frame(terminal, app, suppress_colors)?;
            dirty = false;
            if let Some(delay) = first_poll_delay.take() {
                std::thread::sleep(delay);
            }
        }

        if shapes.apply(app.shapes(), terminal.backend_mut())? {
            terminal.backend_mut().flush()?;
        }

        let output = app.take_output();
        if !output.is_empty() {
            for seq in output {
                terminal.backend_mut().write_all(seq.as_bytes())?;
            }
            terminal.backend_mut().flush()?;
        }

        if app.should_quit() {
            return Ok(());
        }
        let interval = app.tick_interval();
        let mut wait = interval.saturating_sub(last_tick.elapsed());

        let now = Instant::now();
        hover_due = match (crate::ui::reveal::hover_wait(), hover_due) {
            (None, _) => None,
            (Some((frame, _)), Some((seen, due))) if frame == seen => Some((seen, due)),
            (Some((frame, ms)), _) => Some((frame, now + Duration::from_millis(ms))),
        };
        if let Some((_, due)) = hover_due {
            wait = wait.min(due.saturating_duration_since(now));
        }
        if let Some(external) = app.external() {
            wait = wait.min(external.latency);
        }
        wait = turn_wait(wait, raw_pending);

        if raw_pending || pump.has_pending() || poll_uninterrupted(wait)? {
            let changed = input_turn(
                app,
                &mut pump,
                &mut CrosstermSource,
                &mut sizes,
                &mut raw_pending,
            )?;
            if app.should_quit() {
                return Ok(());
            }
            if changed {
                draw_frame(terminal, app, suppress_colors)?;
                if pump.settle(app) {
                    draw_frame(terminal, app, suppress_colors)?;
                }
            }
        }
        if hover_due.is_some_and(|(_, due)| due <= now + wait) {
            dirty = true;
        }

        let external = app.external().map(ExternalReceiver::drain);
        for message in external.into_iter().flatten() {
            if app.handle(Input::External(message)) == Outcome::Changed {
                dirty = true;
            }
        }
        if app.should_quit() {
            return Ok(());
        }

        if let Some(size) = terminal_size(terminal)
            && let Some(resize) = sizes.check(size)
            && app.handle(resize) == Outcome::Changed
        {
            dirty = true;
        }
        if last_tick.elapsed() >= interval {
            last_tick = Instant::now();
            if app.handle(Input::Tick) == Outcome::Changed {
                dirty = true;
            }
            if app.should_quit() {
                if dirty {
                    draw_frame(terminal, app, suppress_colors)?;
                }
                return Ok(());
            }
        }
    }
}

fn terminal_size(terminal: &ratatui::DefaultTerminal) -> Option<(u16, u16)> {
    terminal.size().ok().map(|s| (s.width, s.height))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SizeWatch {
    known: Option<(u16, u16)>,
}

impl SizeWatch {
    pub fn new(size: Option<(u16, u16)>) -> Self {
        Self { known: size }
    }

    pub fn observe(&mut self, input: &Input) {
        if let Input::Resize(w, h) = input {
            self.known = Some((*w, *h));
        }
    }

    pub fn check(&mut self, actual: (u16, u16)) -> Option<Input> {
        if self.known == Some(actual) {
            return None;
        }
        self.known = Some(actual);
        Some(Input::Resize(actual.0, actual.1))
    }
}

fn effective_input_config(app: InputConfig, protocol_active: bool) -> InputConfig {
    InputConfig {
        legacy_ctrl_punctuation: app.legacy_ctrl_punctuation && !protocol_active,
        ..app
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusClickGuard {
    pub window: Duration,
    gained_at: Option<Instant>,
    swallowing: bool,
}

impl FocusClickGuard {
    pub const DEFAULT_WINDOW: Duration = Duration::from_millis(300);

    pub const fn new(window: Duration) -> Self {
        Self {
            window,
            gained_at: None,
            swallowing: false,
        }
    }

    pub fn drops(&mut self, ev: &Event, now: Instant) -> bool {
        match ev {
            Event::FocusGained => {
                self.gained_at = Some(now);
                true
            }
            Event::FocusLost => {
                self.gained_at = None;
                true
            }
            Event::Mouse(m) => match m.kind {
                MouseEventKind::Down(_) => {
                    let fresh = self
                        .gained_at
                        .take()
                        .is_some_and(|at| now.saturating_duration_since(at) < self.window);
                    self.swallowing = fresh;
                    fresh
                }
                MouseEventKind::Drag(_) => self.swallowing,
                MouseEventKind::Up(_) => std::mem::take(&mut self.swallowing),
                _ => false,
            },
            _ => false,
        }
    }
}

impl Default for FocusClickGuard {
    fn default() -> Self {
        Self::new(Self::DEFAULT_WINDOW)
    }
}

thread_local! {
    static FOCUS_GUARD: std::cell::Cell<FocusClickGuard> =
        std::cell::Cell::new(FocusClickGuard::default());
}

pub fn set_focus_click_guard(window: Duration) {
    FOCUS_GUARD.with(|g| g.set(FocusClickGuard::new(window)));
}

/// Bound on the raw terminal events one runtime turn may read before it must
/// return to the external, resize and tick checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawInputPolicy {
    /// Raw events read per turn, counting those filtered before an `Input`
    /// exists. Zero is treated as one.
    pub max_events: usize,
    /// Time charged from the turn start and checked between reads. A turn
    /// always reads one event, so a zero duration still makes progress.
    pub max_duration: Duration,
}

impl RawInputPolicy {
    pub(crate) fn event_limit(&self) -> usize {
        self.max_events.max(1)
    }

    pub const DEFAULT_MAX_EVENTS: usize = 256;
    pub const DEFAULT_MAX_DURATION: Duration = Duration::from_millis(2);
}

impl Default for RawInputPolicy {
    fn default() -> Self {
        Self {
            max_events: Self::DEFAULT_MAX_EVENTS,
            max_duration: Self::DEFAULT_MAX_DURATION,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RawBudget {
    max_events: usize,
    max_duration: Duration,
    started: Instant,
    read: usize,
    filtered: usize,
}

impl RawBudget {
    fn new(policy: RawInputPolicy, started: Instant) -> Self {
        Self {
            max_events: policy.event_limit(),
            max_duration: policy.max_duration,
            started,
            read: 0,
            filtered: 0,
        }
    }

    fn exhausted(&self, now: Instant) -> bool {
        self.read >= self.max_events
            || (self.read > 0 && now.saturating_duration_since(self.started) >= self.max_duration)
    }

    #[allow(dead_code)]
    fn read(&self) -> usize {
        self.read
    }

    #[allow(dead_code)]
    fn filtered(&self) -> usize {
        self.filtered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RawRead {
    Input(Input),
    Empty,
    Yielded,
}

trait RawSource {
    fn poll(&mut self) -> std::io::Result<bool>;
    fn read(&mut self) -> std::io::Result<Event>;
    fn dropped(&mut self, event: &Event) -> bool;
    fn now(&mut self) -> Instant;
}

struct CrosstermSource;

impl RawSource for CrosstermSource {
    fn poll(&mut self) -> std::io::Result<bool> {
        poll_input(Duration::ZERO)
    }

    fn read(&mut self) -> std::io::Result<Event> {
        event::read()
    }

    fn dropped(&mut self, event: &Event) -> bool {
        FOCUS_GUARD.with(|g| {
            let mut guard = g.get();
            let dropped = guard.drops(event, Instant::now());
            g.set(guard);
            dropped
        })
    }

    fn now(&mut self) -> Instant {
        Instant::now()
    }
}

fn scan_next(
    source: &mut impl RawSource,
    budget: &mut RawBudget,
    config: InputConfig,
) -> std::io::Result<RawRead> {
    loop {
        if !source.poll()? {
            return Ok(RawRead::Empty);
        }
        if budget.exhausted(source.now()) {
            return Ok(RawRead::Yielded);
        }
        let ev = source.read()?;
        budget.read += 1;
        if source.dropped(&ev) {
            budget.filtered += 1;
            continue;
        }
        match Input::from_crossterm_with(ev, config) {
            Some(input) => {
                keyboard::observe(&input);
                return Ok(RawRead::Input(input));
            }
            None => budget.filtered += 1,
        }
    }
}

fn turn_wait(wait: Duration, raw_pending: bool) -> Duration {
    if raw_pending { Duration::ZERO } else { wait }
}

fn input_turn(
    app: &mut impl Application,
    pump: &mut InputPump,
    source: &mut impl RawSource,
    sizes: &mut SizeWatch,
    raw_pending: &mut bool,
) -> std::io::Result<bool> {
    pump.set_policy(app.coalesce());
    let config = effective_input_config(app.input_config(), keyboard::active());
    let mut budget = RawBudget::new(app.raw_input_policy(), source.now());
    pump.drain(app, || match scan_next(source, &mut budget, config)? {
        RawRead::Input(input) => {
            sizes.observe(&input);
            Ok(Some(input))
        }
        RawRead::Empty => {
            *raw_pending = false;
            Ok(None)
        }
        RawRead::Yielded => {
            *raw_pending = true;
            Ok(None)
        }
    })
}

pub const DRAIN_BUDGET: usize = 256;

pub fn drain_ready_inputs(
    app: &mut impl Application,
    next: impl FnMut() -> std::io::Result<Option<Input>>,
) -> std::io::Result<bool> {
    InputPump::new(Coalesce::OFF).drain(app, next)
}

#[derive(Debug, Clone, Default)]
pub struct InputPump {
    policy: Coalesce,

    pending: Option<Input>,

    settle: Option<Mouse>,
}

impl InputPump {
    pub fn new(policy: Coalesce) -> Self {
        Self {
            policy,
            pending: None,
            settle: None,
        }
    }

    pub fn set_policy(&mut self, policy: Coalesce) {
        self.policy = policy;
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn drain(
        &mut self,
        app: &mut impl Application,
        mut next: impl FnMut() -> std::io::Result<Option<Input>>,
    ) -> std::io::Result<bool> {
        let started = Instant::now();
        let mut unchanged = 0;

        let mut burst: Option<(Input, usize)> = None;
        self.settle = None;
        loop {
            if let Some((last, folded)) = &burst
                && (!self.policy.folds(last, last)
                    || *folded >= self.policy.max_batch
                    || started.elapsed() >= self.policy.max_latency)
            {
                break;
            }
            let input = match self.pending.take() {
                Some(input) => input,
                None => match next()? {
                    Some(input) => input,
                    None => break,
                },
            };
            if let Some((last, _)) = &burst
                && !self.policy.folds(last, &input)
            {
                self.pending = Some(input);
                break;
            }

            let changed = crate::ui::reveal::on_pointer_wheel(&input)
                .or_else(|| crate::ui::tooltip::on_input(&input))
                .unwrap_or_else(|| app.handle(input.clone()))
                == Outcome::Changed;
            if app.should_quit() {
                return Ok(changed || burst.is_some());
            }
            match &mut burst {
                Some((last, folded)) => {
                    *last = input;
                    *folded += 1;
                }
                None if changed => burst = Some((input, 1)),
                None => {
                    unchanged += 1;
                    if unchanged >= DRAIN_BUDGET {
                        return Ok(false);
                    }
                }
            }
        }
        if let Some((Input::Mouse(m), folded)) = &burst
            && m.kind == MouseKind::Move
            && *folded > 1
        {
            self.settle = Some(*m);
        }
        Ok(burst.is_some())
    }

    pub fn settle(&mut self, app: &mut impl Application) -> bool {
        match self.settle.take() {
            Some(m) => app.handle(Input::Mouse(m)) == Outcome::Changed,
            None => false,
        }
    }
}

fn draw_frame(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut impl Application,
    suppress_colors: bool,
) -> std::io::Result<()> {
    queue!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
    let drawn = terminal
        .draw(|f| render_frame(app, f, suppress_colors))
        .map(|_| ());
    let ended = execute!(terminal.backend_mut(), EndSynchronizedUpdate);
    drawn?;
    ended
}

fn render_frame(app: &mut impl Application, frame: &mut Frame, suppress_colors: bool) {
    app.render(frame);
    if suppress_colors {
        reset_frame_colors(frame.buffer_mut());
    }
}

fn reset_frame_colors(buffer: &mut Buffer) {
    for cell in &mut buffer.content {
        cell.fg = Color::Reset;
        cell.bg = Color::Reset;
        cell.underline_color = Color::Reset;
    }
}

/// Discards every queued raw terminal event, however many arrive.
///
/// This never gives up while the queue keeps refilling. [`shell::run`] does
/// not call it; use [`drain_pending_input_bounded`] at startup instead.
///
/// [`shell::run`]: crate::shell::run
pub fn drain_pending_input() -> std::io::Result<usize> {
    drain_pending_input_bounded(usize::MAX)
}

/// Discards at most `max_events` queued raw terminal events and returns how
/// many it read, filtered event kinds included. Events beyond the bound stay
/// queued for normal input handling. `0` returns at once without touching
/// the terminal.
///
/// Raw mode is enabled for the scan when it was off and disabled again on
/// every return; a disable failure takes precedence over a scan failure.
pub fn drain_pending_input_bounded(max_events: usize) -> std::io::Result<usize> {
    discard_pending(&mut CrosstermSource, &mut TerminalRawMode, max_events)
}

trait RawModeControl {
    fn enabled(&mut self) -> std::io::Result<bool>;
    fn enable(&mut self) -> std::io::Result<()>;
    fn disable(&mut self) -> std::io::Result<()>;
}

struct TerminalRawMode;

impl RawModeControl for TerminalRawMode {
    fn enabled(&mut self) -> std::io::Result<bool> {
        ratatui::crossterm::terminal::is_raw_mode_enabled()
    }

    fn enable(&mut self) -> std::io::Result<()> {
        enable_raw_mode()
    }

    fn disable(&mut self) -> std::io::Result<()> {
        disable_raw_mode()
    }
}

struct RawModeRestore<'a, M: RawModeControl> {
    mode: &'a mut M,
    armed: bool,
}

impl<M: RawModeControl> RawModeRestore<'_, M> {
    fn finish(mut self) -> std::io::Result<()> {
        self.armed = false;
        self.mode.disable()
    }
}

impl<M: RawModeControl> Drop for RawModeRestore<'_, M> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.mode.disable();
        }
    }
}

fn discard_pending(
    source: &mut impl RawSource,
    mode: &mut impl RawModeControl,
    max_events: usize,
) -> std::io::Result<usize> {
    if max_events == 0 {
        return Ok(0);
    }
    let restore = if mode.enabled()? {
        None
    } else {
        mode.enable()?;
        Some(RawModeRestore { mode, armed: true })
    };
    let scanned = discard_ready(source, max_events);
    if let Some(restore) = restore {
        restore.finish()?;
    }
    scanned
}

fn discard_ready(source: &mut impl RawSource, max_events: usize) -> std::io::Result<usize> {
    let mut discarded = 0;
    while discarded < max_events && source.poll()? {
        source.read()?;
        discarded += 1;
    }
    Ok(discarded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{
        KeyEvent, KeyEventKind, KeyEventState, MouseButton, MouseEvent,
    };
    use std::collections::VecDeque;

    struct Script {
        queue: VecDeque<Event>,
        reads: usize,
        base: Instant,
        step: Duration,
        guard: FocusClickGuard,
        fail_read_at: Option<usize>,
        endless: Option<Event>,
    }

    impl Script {
        fn new(queue: impl IntoIterator<Item = Event>) -> Self {
            Self {
                queue: queue.into_iter().collect(),
                reads: 0,
                base: Instant::now(),
                step: Duration::ZERO,
                guard: FocusClickGuard::default(),
                fail_read_at: None,
                endless: None,
            }
        }

        fn releases_then(n: usize, tail: impl IntoIterator<Item = Event>) -> Self {
            Self::new((0..n).map(|_| release('r')).chain(tail))
        }

        fn endless(event: Event) -> Self {
            let mut script = Self::new([]);
            script.endless = Some(event);
            script
        }
    }

    impl RawSource for Script {
        fn poll(&mut self) -> std::io::Result<bool> {
            Ok(self.endless.is_some() || !self.queue.is_empty())
        }

        fn read(&mut self) -> std::io::Result<Event> {
            if self.fail_read_at == Some(self.reads) {
                return Err(std::io::Error::other("read failed"));
            }
            self.reads += 1;
            match &self.endless {
                Some(event) => Ok(event.clone()),
                None => Ok(self.queue.pop_front().expect("poll said ready")),
            }
        }

        fn dropped(&mut self, event: &Event) -> bool {
            let now = self.now();
            self.guard.drops(event, now)
        }

        fn now(&mut self) -> Instant {
            self.base + self.step * u32::try_from(self.reads).unwrap()
        }
    }

    fn release(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        })
    }

    fn press(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    fn mouse(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn key(c: char) -> Input {
        Input::Key(Key {
            code: KeyCode::Char(c),
            mods: KeyModifiers::NONE,
        })
    }

    fn policy(max_events: usize, max_duration: Duration) -> RawInputPolicy {
        RawInputPolicy {
            max_events,
            max_duration,
        }
    }

    fn budget(source: &mut Script, policy: RawInputPolicy) -> RawBudget {
        RawBudget::new(policy, source.now())
    }

    #[derive(Default)]
    struct Recorder {
        policy: Option<RawInputPolicy>,
        seen: Vec<Input>,
    }

    impl Application for Recorder {
        fn handle(&mut self, input: Input) -> Outcome {
            self.seen.push(input);
            Outcome::Ignored
        }
        fn render(&mut self, _frame: &mut Frame) {}
        fn should_quit(&self) -> bool {
            false
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }
        fn coalesce(&self) -> Coalesce {
            Coalesce::OFF
        }
        fn raw_input_policy(&self) -> RawInputPolicy {
            self.policy.unwrap_or_default()
        }
    }

    struct Turns {
        app: Recorder,
        pump: InputPump,
        sizes: SizeWatch,
        raw_pending: bool,
    }

    impl Turns {
        fn new(policy: RawInputPolicy) -> Self {
            Self {
                app: Recorder {
                    policy: Some(policy),
                    seen: Vec::new(),
                },
                pump: InputPump::new(Coalesce::OFF),
                sizes: SizeWatch::default(),
                raw_pending: false,
            }
        }

        fn turn(&mut self, source: &mut Script) -> std::io::Result<bool> {
            input_turn(
                &mut self.app,
                &mut self.pump,
                source,
                &mut self.sizes,
                &mut self.raw_pending,
            )
        }
    }

    #[test]
    fn raw_input_filtered_stream_yields_at_budget() {
        let mut source = Script::releases_then(10_000, [press('a')]);
        let mut budget = budget(&mut source, policy(8, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 8);
        assert_eq!((budget.read(), budget.filtered()), (8, 8));
        assert_eq!(source.queue.len(), 10_001 - 8);
    }

    #[test]
    fn raw_input_budget_is_shared_across_accepted_inputs() {
        let queue = (b'a'..=b'h').flat_map(|c| [release('r'), press(char::from(c))]);
        let mut source = Script::new(queue);
        let mut turns = Turns::new(policy(8, Duration::from_secs(60)));
        turns.turn(&mut source).unwrap();
        assert_eq!(source.reads, 8);
        assert_eq!(turns.app.seen, [key('a'), key('b'), key('c'), key('d')]);
        assert!(turns.raw_pending);
    }

    #[test]
    fn raw_input_fifo_survives_yield() {
        let mut source = Script::releases_then(5, [press('A'), press('B')]);
        let mut turns = Turns::new(policy(3, Duration::from_secs(60)));
        turns.turn(&mut source).unwrap();
        assert!(turns.app.seen.is_empty());
        assert!(turns.raw_pending);
        turns.turn(&mut source).unwrap();
        assert_eq!(turns.app.seen, [key('A')]);
        assert!(turns.raw_pending);
        turns.turn(&mut source).unwrap();
        assert_eq!(turns.app.seen, [key('A'), key('B')]);
        assert!(!turns.raw_pending);
        assert_eq!(source.reads, 7);
    }

    #[test]
    fn raw_input_flood_allows_tick_external_resize() {
        let mut source = Script::endless(release('r'));
        let mut turns = Turns::new(policy(16, Duration::from_secs(60)));
        let mut checks = Vec::new();
        for turn in 0..5 {
            let before = source.reads;
            turns.turn(&mut source).unwrap();
            assert_eq!(
                source.reads - before,
                16,
                "turn {turn} read past its budget"
            );
            assert!(turns.raw_pending);
            let wait = turn_wait(Duration::from_millis(100), turns.raw_pending);
            assert_eq!(wait, Duration::ZERO);
            checks.extend(["external", "resize", "tick"]);
        }
        assert!(turns.app.seen.is_empty());
        assert_eq!(checks.len(), 15);
        assert_eq!(checks[..3], ["external", "resize", "tick"]);

        let mut drained = Script::new([press('a')]);
        turns.turn(&mut drained).unwrap();
        assert!(!turns.raw_pending);
        assert_eq!(
            turn_wait(Duration::from_millis(100), turns.raw_pending),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn raw_input_zero_duration_processes_one_then_yields() {
        let mut source = Script::releases_then(100, [press('a')]);
        let mut budget = budget(&mut source, policy(1000, Duration::ZERO));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 1);
    }

    #[test]
    fn raw_input_duration_is_charged_from_turn_start_between_reads() {
        let mut source = Script::releases_then(100, [press('a')]);
        source.step = Duration::from_millis(1);
        let mut budget = budget(&mut source, policy(1000, Duration::from_millis(3)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 3);
    }

    #[test]
    fn raw_input_zero_event_budget_normalizes_to_one() {
        let mut source = Script::releases_then(10, [press('a')]);
        let mut budget = budget(&mut source, policy(0, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 1);
    }

    #[test]
    fn raw_input_charges_guard_drops_and_unsupported_events() {
        let mut source = Script::new([
            Event::FocusGained,
            mouse(MouseEventKind::Down(MouseButton::Left)),
            mouse(MouseEventKind::Up(MouseButton::Left)),
            mouse(MouseEventKind::Drag(MouseButton::Middle)),
            release('r'),
            Event::FocusLost,
            press('a'),
        ]);
        let mut budget = budget(&mut source, policy(6, Duration::from_secs(60)));
        let first = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(first, RawRead::Yielded);
        assert_eq!((budget.read(), budget.filtered()), (6, 6));
        let mut next = RawBudget::new(policy(6, Duration::from_secs(60)), source.now());
        let second = scan_next(&mut source, &mut next, InputConfig::default()).unwrap();
        assert_eq!(second, RawRead::Input(key('a')));
    }

    #[test]
    fn raw_input_accepted_input_ends_scan_without_lookahead() {
        let mut source = Script::new([press('a'), press('b')]);
        let mut budget = budget(&mut source, policy(8, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Input(key('a')));
        assert_eq!(source.reads, 1);
        assert_eq!(budget.filtered(), 0);
    }

    #[test]
    fn raw_input_empty_at_exact_budget_is_not_a_yield() {
        let mut source = Script::releases_then(4, []);
        let mut budget = budget(&mut source, policy(4, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Empty);
        assert_eq!(source.reads, 4);
    }

    #[test]
    fn raw_input_empty_and_read_error_are_distinct() {
        let mut turns = Turns::new(RawInputPolicy::default());
        turns.raw_pending = true;
        turns.turn(&mut Script::new([])).unwrap();
        assert!(!turns.raw_pending);

        let mut failing = Script::releases_then(3, [press('a')]);
        failing.fail_read_at = Some(2);
        turns.raw_pending = true;
        let error = turns.turn(&mut failing).unwrap_err();
        assert_eq!(error.to_string(), "read failed");
        assert!(turns.raw_pending);
        assert!(turns.app.seen.is_empty());
    }

    #[derive(Default)]
    struct Mode {
        raw: bool,
        fail_disable: bool,
        calls: Vec<&'static str>,
    }

    impl RawModeControl for Mode {
        fn enabled(&mut self) -> std::io::Result<bool> {
            self.calls.push("enabled");
            Ok(self.raw)
        }
        fn enable(&mut self) -> std::io::Result<()> {
            self.calls.push("enable");
            self.raw = true;
            Ok(())
        }
        fn disable(&mut self) -> std::io::Result<()> {
            self.calls.push("disable");
            if self.fail_disable {
                return Err(std::io::Error::other("disable failed"));
            }
            self.raw = false;
            Ok(())
        }
    }

    #[test]
    fn startup_discard_is_bounded_and_restores_raw_mode() {
        let mut flood = Script::endless(release('r'));
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut flood, &mut mode, 256).unwrap(), 256);
        assert_eq!(flood.reads, 256);
        assert!(!mode.raw);
        assert_eq!(mode.calls, ["enabled", "enable", "disable"]);

        let mut queued = Script::new((0..15).map(|_| release('r')).chain([press('a')]));
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut queued, &mut mode, 10).unwrap(), 10);
        assert_eq!(queued.queue.len(), 6);

        let mut short = Script::new([Event::FocusGained, press('a')]);
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut short, &mut mode, 10).unwrap(), 2);

        let mut raw = Script::endless(release('r'));
        let mut mode = Mode {
            raw: true,
            ..Mode::default()
        };
        assert_eq!(discard_pending(&mut raw, &mut mode, 5).unwrap(), 5);
        assert!(mode.raw);
        assert_eq!(mode.calls, ["enabled"]);
    }

    #[test]
    fn startup_discard_zero_bound_touches_nothing() {
        let mut flood = Script::endless(release('r'));
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut flood, &mut mode, 0).unwrap(), 0);
        assert_eq!(flood.reads, 0);
        assert!(mode.calls.is_empty());
    }

    #[test]
    fn startup_discard_restores_raw_mode_on_scan_error_and_unwind() {
        let mut failing = Script::releases_then(3, []);
        failing.fail_read_at = Some(1);
        let mut mode = Mode::default();
        let error = discard_pending(&mut failing, &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "read failed");
        assert!(!mode.raw);

        struct Panicking;
        impl RawSource for Panicking {
            fn poll(&mut self) -> std::io::Result<bool> {
                panic!("poll panicked")
            }
            fn read(&mut self) -> std::io::Result<Event> {
                unreachable!()
            }
            fn dropped(&mut self, _event: &Event) -> bool {
                false
            }
            fn now(&mut self) -> Instant {
                Instant::now()
            }
        }
        let mut mode = Mode::default();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = discard_pending(&mut Panicking, &mut mode, 10);
        }));
        assert!(unwound.is_err());
        assert!(!mode.raw);
        assert_eq!(mode.calls, ["enabled", "enable", "disable"]);
    }

    #[test]
    fn startup_discard_error_precedence() {
        let scan_fails = || {
            let mut source = Script::releases_then(3, []);
            source.fail_read_at = Some(1);
            source
        };

        let mut mode = Mode::default();
        let error = discard_pending(&mut scan_fails(), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "read failed");

        let mut mode = Mode {
            fail_disable: true,
            ..Mode::default()
        };
        let error = discard_pending(&mut Script::releases_then(3, []), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "disable failed");

        let mut mode = Mode {
            fail_disable: true,
            ..Mode::default()
        };
        let error = discard_pending(&mut scan_fails(), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "disable failed");

        let mut mode = Mode {
            raw: true,
            fail_disable: true,
            ..Mode::default()
        };
        assert_eq!(
            discard_pending(&mut Script::releases_then(3, []), &mut mode, 10).unwrap(),
            3
        );
        let error = discard_pending(&mut scan_fails(), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "read failed");
        assert!(!mode.calls.contains(&"disable"));
    }

    #[test]
    fn raw_input_policy_default_and_normalization() {
        let default = RawInputPolicy::default();
        assert_eq!(default.max_events, 256);
        assert_eq!(default.max_duration, Duration::from_millis(2));
        assert_eq!(policy(0, Duration::ZERO).event_limit(), 1);
    }
}
