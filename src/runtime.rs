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

/// Bounds one runtime turn's external-message work.
///
/// The runtime hands the application at most `max_messages` external messages
/// per turn and stops early once `max_duration` has elapsed, measured from the
/// start of the turn and checked between messages. A turn always handles one
/// message when one is queued, so a zero duration or zero count still makes
/// progress. Queued messages stay in order and are not lost: the next turn
/// resumes where this one stopped, after the runtime has checked terminal
/// input, resize and tick.
///
/// The bound counts messages and time between handlers. It cannot interrupt
/// one handler that blocks or runs for a long time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPolicy {
    /// Messages handled per turn. Zero is treated as one.
    pub max_messages: usize,
    /// Time charged from the turn start, including handler time.
    pub max_duration: Duration,
}

impl ExternalPolicy {
    pub const DEFAULT_MAX_MESSAGES: usize = 64;
    pub const DEFAULT_MAX_DURATION: Duration = Duration::from_millis(2);

    fn normalized(self) -> Self {
        Self {
            max_messages: self.max_messages.max(1),
            ..self
        }
    }
}

impl Default for ExternalPolicy {
    fn default() -> Self {
        Self {
            max_messages: Self::DEFAULT_MAX_MESSAGES,
            max_duration: Self::DEFAULT_MAX_DURATION,
        }
    }
}

/// A rejected [`ExternalSender::try_send`]. Both variants return the message,
/// so the producer can retry it or merge it into its own pending state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSendError {
    /// The bounded queue is at capacity. Retry later.
    Full(External),
    /// The receiver was dropped. No retry will succeed.
    Closed(External),
}

impl std::fmt::Display for ExternalSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Full(_) => "external queue is full",
            Self::Closed(_) => "external queue is closed",
        })
    }
}

impl std::error::Error for ExternalSendError {}

#[derive(Debug, Clone)]
enum ExternalQueue {
    Unbounded(std::sync::mpsc::Sender<External>),
    Bounded(std::sync::mpsc::SyncSender<External>),
}

/// The producer half of an external-message channel. Clone it for each
/// producer thread.
#[derive(Debug, Clone)]
pub struct ExternalSender(ExternalQueue);

impl ExternalSender {
    /// Queues `message` without blocking.
    ///
    /// A bounded channel answers [`ExternalSendError::Full`] at capacity. An
    /// unbounded channel never reports `Full`. Either way the rejected message
    /// comes back in the error, so a retry sends the same payload.
    pub fn try_send(&self, message: External) -> Result<(), ExternalSendError> {
        use std::sync::mpsc::TrySendError;
        match &self.0 {
            ExternalQueue::Unbounded(tx) => {
                tx.send(message).map_err(|e| ExternalSendError::Closed(e.0))
            }
            ExternalQueue::Bounded(tx) => tx.try_send(message).map_err(|e| match e {
                TrySendError::Full(m) => ExternalSendError::Full(m),
                TrySendError::Disconnected(m) => ExternalSendError::Closed(m),
            }),
        }
    }

    /// Wraps `message` and queues it, returning whether it was accepted.
    ///
    /// This is the lossy convenience: on a full or closed queue the message is
    /// dropped and the caller learns only `false`. A producer that must keep
    /// the payload calls [`try_send`](Self::try_send) with an [`External`] it
    /// retains and retries on [`ExternalSendError::Full`].
    pub fn send<T: std::any::Any + Send + Sync>(&self, message: T) -> bool {
        self.try_send(External::new(message)).is_ok()
    }
}

/// The runtime half of an external-message channel, owned by the application
/// and returned from [`Application::external`].
#[derive(Debug)]
pub struct ExternalReceiver {
    queue: std::sync::mpsc::Receiver<External>,
    latency: Duration,
    policy: ExternalPolicy,
}

impl ExternalReceiver {
    pub const DEFAULT_LATENCY: Duration = Duration::from_millis(10);

    fn new(queue: std::sync::mpsc::Receiver<External>, policy: ExternalPolicy) -> Self {
        Self {
            queue,
            latency: Self::DEFAULT_LATENCY,
            policy: policy.normalized(),
        }
    }

    pub fn latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// The per-turn bound the runtime applies to this receiver.
    pub fn policy(&self) -> ExternalPolicy {
        self.policy
    }

    /// Replaces the per-turn bound, normalising a zero message count to one.
    pub fn set_policy(&mut self, policy: ExternalPolicy) {
        self.policy = policy.normalized();
    }

    pub fn with_policy(mut self, policy: ExternalPolicy) -> Self {
        self.set_policy(policy);
        self
    }

    /// Takes the oldest queued message, if any. An empty or disconnected
    /// queue both read as no message.
    pub fn try_recv(&self) -> Option<External> {
        self.queue.try_recv().ok()
    }

    /// Takes every queued message at once.
    ///
    /// The runtime never calls this. It is unbounded: a producer that keeps
    /// refilling the queue keeps it from returning, and the whole backlog is
    /// allocated at once. Read incrementally with [`try_recv`](Self::try_recv).
    #[deprecated(note = "unbounded; read incrementally with `try_recv`")]
    pub fn drain(&self) -> Vec<External> {
        self.queue.try_iter().collect()
    }
}

/// Creates an unbounded channel with the default [`ExternalPolicy`].
///
/// The queue can grow without limit while the application is busy, so this
/// suits a low-rate producer. The runtime still handles at most one policy's
/// worth of messages per turn. Prefer [`bounded_external_channel`] for a
/// producer that can outrun the application.
pub fn external_channel() -> (ExternalSender, ExternalReceiver) {
    let (tx, rx) = std::sync::mpsc::channel();
    (
        ExternalSender(ExternalQueue::Unbounded(tx)),
        ExternalReceiver::new(rx, ExternalPolicy::default()),
    )
}

/// Creates a channel that holds at most `capacity` queued messages.
///
/// [`ExternalSender::try_send`] never blocks: at capacity it returns
/// [`ExternalSendError::Full`] with the message, and the producer decides
/// whether to retry, merge or drop. 256 is a reasonable capacity for a UI
/// background feed.
pub fn bounded_external_channel(
    capacity: std::num::NonZeroUsize,
    policy: ExternalPolicy,
) -> (ExternalSender, ExternalReceiver) {
    let (tx, rx) = std::sync::mpsc::sync_channel(capacity.get());
    (
        ExternalSender(ExternalQueue::Bounded(tx)),
        ExternalReceiver::new(rx, policy),
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ExternalTurn {
    changed: bool,
    exhausted: bool,
}

/// Hands the application one bounded group of queued external messages.
///
/// The receiver is read again before every message, so a handler may replace
/// or remove it. `exhausted` means the bound stopped the turn and more work
/// may be queued, so the caller schedules another turn without waiting.
fn pump_external(
    app: &mut impl Application,
    policy: ExternalPolicy,
    elapsed: impl Fn() -> Duration,
) -> ExternalTurn {
    let mut turn = ExternalTurn::default();
    for count in 0..policy.normalized().max_messages {
        if app.should_quit() {
            return turn;
        }
        if count > 0 && elapsed() >= policy.max_duration {
            turn.exhausted = true;
            return turn;
        }
        let Some(message) = app.external().and_then(ExternalReceiver::try_recv) else {
            return turn;
        };
        turn.changed |= app.handle(Input::External(message)) == Outcome::Changed;
        if app.should_quit() {
            return turn;
        }
    }
    turn.exhausted = true;
    turn
}

/// What one loop iteration does after terminal input: a bounded external
/// group, then the resize check, then the tick. The order and the quit checks
/// between them are the fairness contract between those sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ServiceTurn {
    changed: bool,
    quit: bool,
    ticked: bool,
}

fn service_turn(
    app: &mut impl Application,
    external_pending: &mut bool,
    elapsed: impl Fn() -> Duration,
    resize: impl FnOnce() -> Option<Input>,
    tick_due: impl FnOnce() -> bool,
) -> ServiceTurn {
    let mut turn = ServiceTurn::default();
    match app.external().map(ExternalReceiver::policy) {
        Some(policy) => {
            let external = pump_external(app, policy, elapsed);
            turn.changed |= external.changed;
            *external_pending = external.exhausted;
        }
        None => *external_pending = false,
    }
    if app.should_quit() {
        turn.quit = true;
        return turn;
    }
    if let Some(resize) = resize() {
        turn.changed |= app.handle(resize) == Outcome::Changed;
    }
    if tick_due() {
        turn.ticked = true;
        turn.changed |= app.handle(Input::Tick) == Outcome::Changed;
        turn.quit = app.should_quit();
    }
    turn
}

fn external_wait(wait: Duration, external_pending: bool) -> Duration {
    if external_pending {
        Duration::ZERO
    } else {
        wait
    }
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
    let mut external_pending = false;
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
        wait = external_wait(wait, external_pending);

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

        let started = Instant::now();
        let turn = service_turn(
            app,
            &mut external_pending,
            || started.elapsed(),
            || terminal_size(terminal).and_then(|size| sizes.check(size)),
            || {
                let due = last_tick.elapsed() >= interval;
                if due {
                    last_tick = Instant::now();
                }
                due
            },
        );
        dirty |= turn.changed;
        if turn.quit {
            if turn.ticked && dirty {
                draw_frame(terminal, app, suppress_colors)?;
            }
            return Ok(());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    const LONG: Duration = Duration::from_secs(3600);

    fn policy(max_messages: usize) -> ExternalPolicy {
        ExternalPolicy {
            max_messages,
            max_duration: LONG,
        }
    }

    fn zero_clock() -> Duration {
        Duration::ZERO
    }

    #[derive(Default)]
    struct Fixture {
        receiver: Option<ExternalReceiver>,
        seen: Vec<String>,
        quit_on: Option<u32>,
        refill: Option<ExternalSender>,
        remove_on: Option<u32>,
        quit: bool,
    }

    impl Fixture {
        fn over(receiver: ExternalReceiver) -> Self {
            Self {
                receiver: Some(receiver),
                ..Self::default()
            }
        }

        fn numbers(&self) -> Vec<u32> {
            self.seen
                .iter()
                .filter_map(|s| s.strip_prefix('e')?.parse().ok())
                .collect()
        }
    }

    impl Application for Fixture {
        fn handle(&mut self, input: Input) -> Outcome {
            match input {
                Input::External(message) => {
                    let n = *message.downcast_ref::<u32>().expect("u32 payload");
                    self.seen.push(format!("e{n}"));
                    if self.quit_on == Some(n) {
                        self.quit = true;
                    }
                    if self.remove_on == Some(n) {
                        self.receiver = None;
                    }
                    if let Some(tx) = &self.refill {
                        assert!(tx.send(n + 1000));
                    }
                    Outcome::Changed
                }
                Input::Resize(..) => {
                    self.seen.push("r".into());
                    Outcome::Changed
                }
                Input::Tick => {
                    self.seen.push("t".into());
                    Outcome::Ignored
                }
                Input::Key(_) => {
                    self.seen.push("k".into());
                    Outcome::Changed
                }
                _ => Outcome::Ignored,
            }
        }
        fn render(&mut self, _frame: &mut Frame) {}
        fn should_quit(&self) -> bool {
            self.quit
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }
        fn external(&self) -> Option<&ExternalReceiver> {
            self.receiver.as_ref()
        }
    }

    fn loaded(count: u32, policy: ExternalPolicy) -> (Fixture, ExternalSender) {
        let (tx, rx) = external_channel();
        for n in 0..count {
            assert!(tx.send(n));
        }
        (Fixture::over(rx.with_policy(policy)), tx)
    }

    fn pump(app: &mut Fixture) -> ExternalTurn {
        let policy = app.external().expect("receiver").policy();
        pump_external(app, policy, zero_clock)
    }

    fn rest(app: &Fixture) -> Vec<u32> {
        std::iter::from_fn(|| app.external()?.try_recv())
            .map(|m| *m.downcast_ref::<u32>().expect("u32 payload"))
            .collect()
    }

    #[test]
    fn external_turn_stops_at_count_and_keeps_fifo() {
        let (mut app, _tx) = loaded(10, policy(3));

        let first = pump(&mut app);
        assert_eq!(app.numbers(), [0, 1, 2]);
        assert!(first.changed && first.exhausted);

        let second = pump(&mut app);
        assert_eq!(app.numbers(), [0, 1, 2, 3, 4, 5]);
        assert!(second.exhausted);

        assert_eq!(rest(&app), [6, 7, 8, 9], "no extra dequeue, order intact");
    }

    #[test]
    fn external_turn_normalizes_a_zero_count_to_one_message() {
        let (mut app, _tx) = loaded(4, policy(0));
        assert_eq!(app.external().unwrap().policy().max_messages, 1);
        pump(&mut app);
        assert_eq!(app.numbers(), [0]);
    }

    #[test]
    fn external_turn_charges_handler_time() {
        let (mut app, _tx) = loaded(
            5,
            ExternalPolicy {
                max_messages: 64,
                max_duration: Duration::from_millis(2),
            },
        );
        let policy = app.external().unwrap().policy();
        let spent = pump_external(&mut app, policy, || Duration::from_millis(3));
        assert_eq!(app.numbers(), [0], "one handler consumed the budget");
        assert!(spent.exhausted);
        assert_eq!(rest(&app), [1, 2, 3, 4]);
    }

    #[test]
    fn external_turn_zero_duration_handles_one_then_yields() {
        let (mut app, _tx) = loaded(
            3,
            ExternalPolicy {
                max_messages: 64,
                max_duration: Duration::ZERO,
            },
        );
        let turn = pump(&mut app);
        assert_eq!(app.numbers(), [0]);
        assert!(turn.exhausted);
    }

    #[test]
    fn external_turn_quit_stops_immediately() {
        let (mut app, _tx) = loaded(6, policy(64));
        app.quit_on = Some(0);
        let turn = pump(&mut app);
        assert_eq!(app.numbers(), [0]);
        assert!(!turn.exhausted);
        assert_eq!(rest(&app), [1, 2, 3, 4, 5], "later messages stay queued");
    }

    #[test]
    fn external_turn_replenished_queue_yields() {
        let (mut app, tx) = loaded(1, policy(5));
        app.refill = Some(tx);
        let turn = pump(&mut app);
        assert_eq!(
            app.numbers().len(),
            5,
            "a refilling queue cannot hold the turn"
        );
        assert!(turn.exhausted);
    }

    #[test]
    fn external_turn_receiver_removed_by_handler() {
        let (mut app, _tx) = loaded(4, policy(64));
        app.remove_on = Some(1);
        let turn = pump(&mut app);
        assert_eq!(app.numbers(), [0, 1]);
        assert!(!turn.exhausted);
        assert!(app.external().is_none());
        let mut pending = true;
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert!(!pending);
    }

    #[test]
    fn external_turn_empty_queue_is_not_pending() {
        let (mut app, _tx) = loaded(2, policy(8));
        let mut pending = true;
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert_eq!(app.numbers(), [0, 1]);
        assert!(!pending, "the queue emptied inside the bound");
    }

    #[test]
    fn external_turn_exact_count_costs_one_empty_turn() {
        let (mut app, _tx) = loaded(3, policy(3));
        let mut pending = false;
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert!(pending, "conservatively pending at the bound");
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert!(!pending);
        assert_eq!(app.numbers(), [0, 1, 2]);
    }

    #[test]
    fn external_backlog_allows_input_resize_and_tick() {
        let (mut app, _tx) = loaded(10, policy(3));
        let mut pending = false;

        let mut keys = vec![Input::Key(Key {
            code: KeyCode::Char('a'),
            mods: KeyModifiers::NONE,
        })]
        .into_iter();
        drain_ready_inputs(&mut app, || Ok(keys.next())).expect("input drains");
        let first = service_turn(
            &mut app,
            &mut pending,
            zero_clock,
            || Some(Input::Resize(80, 24)),
            || true,
        );
        assert!(first.ticked && !first.quit);
        assert_eq!(app.seen, ["k", "e0", "e1", "e2", "r", "t"]);
        assert_eq!(
            external_wait(Duration::from_millis(50), pending),
            Duration::ZERO
        );

        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert_eq!(app.seen[6..], ["e3", "e4", "e5"]);
    }

    #[test]
    fn external_backlog_quit_skips_resize_and_tick() {
        let (mut app, _tx) = loaded(5, policy(64));
        app.quit_on = Some(1);
        let mut pending = false;
        let turn = service_turn(
            &mut app,
            &mut pending,
            zero_clock,
            || Some(Input::Resize(80, 24)),
            || true,
        );
        assert!(turn.quit && !turn.ticked);
        assert_eq!(app.seen, ["e0", "e1"]);
    }

    #[test]
    fn external_wait_is_immediate_only_while_pending() {
        let wait = Duration::from_millis(40);
        assert_eq!(external_wait(wait, false), wait);
        assert_eq!(external_wait(wait, true), Duration::ZERO);
    }

    fn capacity(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).expect("nonzero")
    }

    #[test]
    fn bounded_external_full_and_closed_preserve_payload() {
        let (tx, rx) = bounded_external_channel(capacity(1), ExternalPolicy::default());
        assert!(tx.send(1_u32));
        let second = External::new(2_u32);
        let Err(ExternalSendError::Full(back)) = tx.try_send(second.clone()) else {
            panic!("a full queue answers Full");
        };
        assert_eq!(back, second, "the same payload comes back");
        assert!(!tx.send(3_u32), "send reports rejection as false");

        let first = rx.try_recv().expect("queued first");
        assert_eq!(first.downcast_ref::<u32>(), Some(&1));
        assert_eq!(tx.try_send(back), Ok(()), "the retry is accepted");
        let again = rx.try_recv().expect("queued retry");
        assert_eq!(again, second);

        let kept = External::new(4_u32);
        drop(rx);
        assert_eq!(
            tx.try_send(kept.clone()),
            Err(ExternalSendError::Closed(kept)),
        );
        assert!(!tx.send(5_u32));
    }

    #[test]
    fn bounded_external_unbounded_sender_reports_closed_with_payload() {
        let (tx, rx) = external_channel();
        drop(rx);
        let kept = External::new(7_u32);
        assert_eq!(
            tx.try_send(kept.clone()),
            Err(ExternalSendError::Closed(kept))
        );
    }

    #[test]
    fn bounded_external_empty_and_dropped_sender_read_as_no_message() {
        let (tx, rx) = bounded_external_channel(capacity(2), ExternalPolicy::default());
        assert!(rx.try_recv().is_none());
        drop(tx);
        assert!(rx.try_recv().is_none());
    }

    #[test]
    fn bounded_external_producers_share_one_capacity_and_keep_order() {
        let (tx, rx) = bounded_external_channel(capacity(3), ExternalPolicy::default());
        let other = tx.clone();
        assert!(tx.send(0_u32));
        assert!(other.send(1_u32));
        assert!(tx.send(2_u32));
        assert!(matches!(
            other.try_send(External::new(3_u32)),
            Err(ExternalSendError::Full(_))
        ));
        let order: Vec<u32> = std::iter::from_fn(|| rx.try_recv())
            .map(|m| *m.downcast_ref::<u32>().expect("u32"))
            .collect();
        assert_eq!(order, [0, 1, 2]);
    }

    #[test]
    fn bounded_external_policy_is_receiver_local_and_normalized() {
        let (_tx, rx) = bounded_external_channel(capacity(4), policy(0));
        assert_eq!(rx.policy().max_messages, 1);
        let mut rx = rx.with_policy(policy(9));
        assert_eq!(rx.policy(), policy(9));
        rx.set_policy(ExternalPolicy::default());
        assert_eq!(rx.policy(), ExternalPolicy::default());
        assert_eq!(ExternalPolicy::default().max_messages, 64);
        assert_eq!(
            ExternalPolicy::default().max_duration,
            Duration::from_millis(2)
        );
    }
}
