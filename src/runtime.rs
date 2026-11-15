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

/// An active terminal session: raw mode, alternate screen, mouse and paste
/// capture, plus (on Unix) ownership of the process SIGTSTP action.
///
/// Two cleanup contracts live here and stay separate. Terminal modes are
/// restored by `Restoration`, which also runs from the panic hook, and may
/// be left and re-entered during [`suspend`](Self::suspend). The SIGTSTP
/// action is restored by [`job_control::JobControlGuard`], only when the
/// session ends ([`try_leave`](Self::try_leave), [`leave`](Self::leave) or
/// drop), never from the panic hook and never during a suspend. Field order
/// makes drop restore the terminal first and the signal action last.
pub struct TerminalSession {
    terminal: ratatui::DefaultTerminal,
    restoration: Restoration,
    #[cfg(unix)]
    job_control: Option<job_control::JobControlGuard>,
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
        #[cfg(unix)]
        let job_control = Some(job_control::install()?);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            previous(info);
        }));
        Ok(Self {
            terminal,
            restoration,
            #[cfg(unix)]
            job_control,
        })
    }

    pub fn terminal(&mut self) -> &mut ratatui::DefaultTerminal {
        &mut self.terminal
    }

    /// Restores the terminal modes, then, on Unix, the host's SIGTSTP action.
    ///
    /// Terminal cleanup stays best effort. Only the SIGTSTP restoration is
    /// reported. When it fails the session keeps its [`job_control`] claim
    /// so a later call or drop can retry.
    pub fn try_leave(&mut self) -> std::io::Result<()> {
        self.restoration.leave();
        #[cfg(unix)]
        if let Some(guard) = self.job_control.as_mut() {
            guard.restore()?;
            self.job_control = None;
        }
        Ok(())
    }

    /// [`try_leave`](Self::try_leave) with the SIGTSTP error discarded.
    pub fn leave(&mut self) {
        let _ = self.try_leave();
    }

    #[cfg(unix)]
    pub fn suspend(&mut self) -> std::io::Result<(u16, u16)> {
        let guard = match &mut self.job_control {
            Some(guard) => guard,
            None => self.job_control.insert(job_control::install()?),
        };
        self.restoration.leave();
        guard.stop_self()?;
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
    //! Cooperative Ctrl+Z (SIGTSTP) handling for the runtime.
    //!
    //! [`install`] swaps the process SIGTSTP action for one that only sets a
    //! flag, which [`take_request`] consumes, and returns a
    //! [`JobControlGuard`]. Dropping the guard puts back the exact action
    //! that was there before: handler, signal mask and flags, `SIG_IGN`
    //! included. [`TerminalSession`](super::TerminalSession) holds one for
    //! its whole lifetime, so direct session users own SIGTSTP too.
    //!
    //! While a guard is alive the library owns SIGTSTP exclusively and a
    //! host must not replace the action. Unlike the panic hook, which a host
    //! may wrap, a second owner is refused. The guard is released on
    //! session end and on unwind, but not by the panic hook, so panic
    //! restoration never touches signal state.

    use std::ffi::c_int;
    use std::io;
    use std::sync::atomic::{AtomicBool, Ordering};

    static REQUESTED: AtomicBool = AtomicBool::new(false);
    static OWNED: AtomicBool = AtomicBool::new(false);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Step {
        Install,
        Restore,
        Default,
        Raise,
        Reinstate,
    }

    fn inject(step: Step) -> io::Result<()> {
        #[cfg(test)]
        if tests::fault_armed(step) {
            return Err(io::Error::other(format!("injected {step:?} failure")));
        }
        let _ = step;
        Ok(())
    }

    extern "C" fn on_tstp(_sig: c_int) {
        REQUESTED.store(true, Ordering::SeqCst);
    }

    fn action(handler: libc::sighandler_t) -> io::Result<libc::sigaction> {
        let mut act: libc::sigaction = unsafe { std::mem::zeroed() };
        act.sa_sigaction = handler;
        act.sa_flags = 0;
        if unsafe { libc::sigemptyset(&mut act.sa_mask) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(act)
    }

    fn set_action(
        step: Step,
        new: &libc::sigaction,
        old: Option<&mut libc::sigaction>,
    ) -> io::Result<()> {
        inject(step)?;
        let old = old.map_or(std::ptr::null_mut(), std::ptr::from_mut);
        if unsafe { libc::sigaction(libc::SIGTSTP, new, old) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Owns the SIGTSTP action installed by [`install`].
    ///
    /// Dropping it restores the previous action. A failed restore is not
    /// reported from `drop`.
    #[must_use = "dropping the guard immediately gives SIGTSTP back to the host"]
    pub struct JobControlGuard {
        previous: libc::sigaction,
        owned: libc::sigaction,
        active: bool,
    }

    impl std::fmt::Debug for JobControlGuard {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("JobControlGuard")
                .field("active", &self.active)
                .finish_non_exhaustive()
        }
    }

    /// Claims SIGTSTP and discards any request left by an earlier owner.
    ///
    /// Fails with [`io::ErrorKind::AlreadyExists`], before changing any
    /// handler or pending request, while another guard is alive. The
    /// installed action has an empty mask and no `SA_RESTART`, so a blocked
    /// terminal read wakes with `Interrupted`.
    pub fn install() -> io::Result<JobControlGuard> {
        if OWNED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "SIGTSTP job control is already owned",
            ));
        }
        let installed = action(on_tstp as *const () as libc::sighandler_t).and_then(|owned| {
            REQUESTED.store(false, Ordering::SeqCst);
            let mut previous = owned;
            set_action(Step::Install, &owned, Some(&mut previous))?;
            Ok(JobControlGuard {
                previous,
                owned,
                active: true,
            })
        });
        if installed.is_err() {
            OWNED.store(false, Ordering::SeqCst);
        }
        installed
    }

    /// Consumes a pending suspend request set by the SIGTSTP handler.
    pub fn take_request() -> bool {
        REQUESTED.swap(false, Ordering::SeqCst)
    }

    impl JobControlGuard {
        /// Stops the process as the shell expects and reinstalls the owned
        /// action after it continues.
        ///
        /// The signal is never raised under an action this guard did not
        /// set. The error of a failed raise, or else of a failed
        /// reinstatement, is returned. The guard keeps the saved host action
        /// for [`drop`](Drop::drop) in every case.
        pub fn stop_self(&mut self) -> io::Result<()> {
            if !self.active {
                return Err(io::Error::other("job control guard already restored"));
            }
            let default = action(libc::SIG_DFL)?;
            set_action(Step::Default, &default, None)?;
            let raised = inject(Step::Raise).and_then(|()| {
                if unsafe { libc::raise(libc::SIGTSTP) } != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
            let reinstated = set_action(Step::Reinstate, &self.owned, None);
            raised.and(reinstated)
        }

        pub(super) fn restore(&mut self) -> io::Result<()> {
            if !self.active {
                return Ok(());
            }
            set_action(Step::Restore, &self.previous, None)?;
            self.active = false;
            REQUESTED.store(false, Ordering::SeqCst);
            OWNED.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    impl Drop for JobControlGuard {
        fn drop(&mut self) {
            let _ = self.restore();
        }
    }

    #[cfg(test)]
    mod tests;
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
    let result = event_loop(&mut session, app);
    let left = session.try_leave();
    result.and(left)
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

        if pump.has_pending() || poll_uninterrupted(wait)? {
            pump.set_policy(app.coalesce());
            let config = effective_input_config(app.input_config(), keyboard::active());
            let changed = pump.drain(app, || {
                let input = next_ready_input(config)?;
                if let Some(input) = &input {
                    sizes.observe(input);
                }
                Ok(input)
            })?;
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

fn next_ready_input(config: InputConfig) -> std::io::Result<Option<Input>> {
    while poll_input(Duration::ZERO)? {
        let ev = event::read()?;
        let dropped = FOCUS_GUARD.with(|g| {
            let mut guard = g.get();
            let d = guard.drops(&ev, Instant::now());
            g.set(guard);
            d
        });
        if dropped {
            continue;
        }
        if let Some(input) = Input::from_crossterm_with(ev, config) {
            keyboard::observe(&input);
            return Ok(Some(input));
        }
    }
    Ok(None)
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

pub fn drain_pending_input() -> std::io::Result<usize> {
    let was_raw = ratatui::crossterm::terminal::is_raw_mode_enabled()?;
    if !was_raw {
        enable_raw_mode()?;
    }
    let mut n = 0;
    let drained = loop {
        match poll_input(Duration::ZERO) {
            Ok(false) => break Ok(n),
            Ok(true) => match event::read() {
                Ok(_) => n += 1,
                Err(e) => break Err(e),
            },
            Err(e) => break Err(e),
        }
    };
    if !was_raw {
        disable_raw_mode()?;
    }
    drained
}
