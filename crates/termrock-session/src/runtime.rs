use std::io::{Write, stdout};
use std::time::{Duration, Instant};

use ratatui::crossterm::cursor::Show;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::EndSynchronizedUpdate;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use termrock_core::event::{External, Input, InputConfig, Key, MouseKind};
use termrock_core::keyboard::{self, KeyboardProtocol};

#[cfg(feature = "work-metrics")]
#[doc(hidden)]
pub mod metered;
mod panic_bridge;
#[cfg(feature = "work-metrics")]
use metered::MeteredBackend;
use panic_bridge::{RestoreGate, SessionOwner};

/// The terminal a [`TerminalSession`] draws into. It is `ratatui`'s
/// [`DefaultTerminal`](ratatui::DefaultTerminal), crossterm over stdout. With
/// the `work-metrics` feature the backend counts the cells and bytes of every
/// frame; the type is then a `ratatui::Terminal` over an unnameable metered
/// backend, and callers can only use it through `Terminal`'s own methods.
#[cfg(not(feature = "work-metrics"))]
pub type SessionTerminal = ratatui::DefaultTerminal;

/// The metered shape of [`SessionTerminal`] (feature `work-metrics`).
#[cfg(feature = "work-metrics")]
pub type SessionTerminal = ratatui::Terminal<MeteredBackend<std::io::Stdout>>;

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

    /// Plumbing for the run loop in `termrock-shell`: a zero message count read as one.
    #[doc(hidden)]
    pub fn normalized(self) -> Self {
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
/// and returned from `Application::external`.
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

    /// Plumbing for the run loop in `termrock-shell`: the longest the loop may wait for input.
    #[doc(hidden)]
    pub fn wait_latency(&self) -> Duration {
        self.latency
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

/// What one `pump_external` turn did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExternalTurn {
    /// At least one handled message returned [`Outcome::Changed`](termrock_core::event::Outcome::Changed).
    pub changed: bool,
    /// The count or time bound stopped the turn, so more messages may be
    /// queued. The runtime schedules the next turn without waiting for input.
    /// False when the queue ran dry or the application asked to quit.
    pub exhausted: bool,
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
/// One session owns the terminal at a time. [`enter`](Self::enter) and
/// [`enter_with`](Self::enter_with) claim that ownership before touching the
/// terminal, so a second entry while a session lives fails with
/// `ErrorKind::AlreadyExists` and has no side effects. The claim lasts until
/// the session is dropped, also after [`try_leave`](Self::try_leave).
///
/// Two cleanup contracts live here and stay separate. Terminal modes are
/// restored once per active interval by the session's `SessionOwner`, from
/// whichever of `try_leave`, drop or the panic hook gets there first. The
/// interval ends at [`suspend`](Self::suspend) and starts again when it
/// resumes. The SIGTSTP action is restored by [`job_control::JobControlGuard`],
/// only when the session ends ([`try_leave`](Self::try_leave),
/// [`leave`](Self::leave) or drop), never from the panic hook and never during
/// a suspend. Field order makes drop restore the terminal first and the signal
/// action last.
///
/// # Panic hook
///
/// The first entry in the process installs one permanent panic-hook bridge.
/// It restores the terminal when a panic happens while a session is active,
/// on any thread, then calls the hook that was installed before it. Later
/// entries add nothing to the chain, and a panic after the session ended
/// emits no terminal cleanup. The library never changes the hook again. A host
/// that calls `std::panic::set_hook` afterwards replaces the bridge and owns
/// terminal cleanup from then on. Unwinding still restores the terminal when
/// the session drops, but under `panic = "abort"` a replacing hook must
/// restore the terminal itself or wrap the hook it replaced.
///
/// A panic caught on another thread while entry or resume is still enabling
/// modes makes the hook restore a half-set-up terminal. Setup then finishes
/// and the interval stays owed one more cleanup, so leave or drop restores the
/// modes setup enabled after the hook ran.
pub struct TerminalSession {
    terminal: SessionTerminal,
    owner: SessionOwner,
    #[cfg(unix)]
    job_control: Option<job_control::JobControlGuard>,
}

static SESSION_GATE: RestoreGate = RestoreGate::new(restore_terminal);

/// Claims `gate`, installs the panic bridge, activates the first interval and
/// then runs `initialize` inside its setup window. A refused claim runs
/// nothing. Any later failure
/// cleans up once and releases the claim when the returned owner drops.
fn claim_and_initialize<T>(
    gate: &'static RestoreGate,
    initialize: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<(T, SessionOwner)> {
    let owner = SessionOwner::claim(gate)?;
    panic_bridge::install_once(gate)?;
    let value = owner.set_up(initialize)?;
    Ok((value, owner))
}

fn open_terminal(protocol: KeyboardProtocol) -> std::io::Result<SessionTerminal> {
    enable_raw_mode()?;
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
    #[cfg(feature = "work-metrics")]
    let backend = MeteredBackend::new(out);
    #[cfg(not(feature = "work-metrics"))]
    let backend = ratatui::backend::CrosstermBackend::new(out);
    ratatui::Terminal::new(backend)
}

const ENABLE_WRAP: &str = "\x1b[?7h";

impl TerminalSession {
    pub fn enter() -> std::io::Result<Self> {
        Self::enter_with(KeyboardProtocol::current())
    }

    /// Fails with `ErrorKind::AlreadyExists`, before any terminal change,
    /// while another session is alive.
    pub fn enter_with(protocol: KeyboardProtocol) -> std::io::Result<Self> {
        Self::enter_on(&SESSION_GATE, || open_terminal(protocol))
    }

    fn enter_on(
        gate: &'static RestoreGate,
        open: impl FnOnce() -> std::io::Result<SessionTerminal>,
    ) -> std::io::Result<Self> {
        let (terminal, owner) = claim_and_initialize(gate, open)?;
        Self::with_job_control(terminal, owner)
    }

    #[cfg(unix)]
    fn with_job_control(terminal: SessionTerminal, owner: SessionOwner) -> std::io::Result<Self> {
        let mut session = Self {
            terminal,
            owner,
            job_control: None,
        };
        session.job_control = Some(job_control::install()?);
        Ok(session)
    }

    #[cfg(not(unix))]
    fn with_job_control(terminal: SessionTerminal, owner: SessionOwner) -> std::io::Result<Self> {
        Ok(Self { terminal, owner })
    }

    pub fn terminal(&mut self) -> &mut SessionTerminal {
        &mut self.terminal
    }

    /// Restores the terminal modes, then, on Unix, the host's SIGTSTP action.
    ///
    /// Terminal cleanup stays best effort and runs once per active interval.
    /// It fails with `ErrorKind::WouldBlock` only while another thread is
    /// still cleaning up, and then nothing else is released. A failed SIGTSTP
    /// restoration is reported too. In both cases the session keeps its
    /// [`job_control`] claim so a later call or drop can retry.
    pub fn try_leave(&mut self) -> std::io::Result<()> {
        self.owner.restore()?;
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
        self.owner.restore()?;
        guard.stop_self()?;
        self.reenter()
    }

    #[cfg(unix)]
    fn reenter(&mut self) -> std::io::Result<(u16, u16)> {
        if let Err(e) = self.owner.set_up(Self::resume_modes) {
            let _ = self.owner.restore();
            return Err(e);
        }

        let mut out = stdout();
        execute!(out, Clear(ClearType::All))?;
        self.terminal.swap_buffers();
        self.terminal.swap_buffers();
        let size = self.terminal.size()?;
        Ok((size.width, size.height))
    }

    #[cfg(unix)]
    fn resume_modes() -> std::io::Result<()> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(
            out,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            EnableFocusChange
        )?;
        if let Some(push) = keyboard::session::resume() {
            out.write_all(push.as_bytes())?;
            out.flush()?;
        }
        Ok(())
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
    pub(in crate::runtime) fn fail_next_install() {
        tests::arm(Step::Install);
    }

    #[cfg(test)]
    pub(in crate::runtime) fn fail_next_restore() {
        tests::arm(Step::Restore);
    }

    #[cfg(test)]
    mod tests;
}

#[cfg(test)]
mod isolated;
#[cfg(test)]
mod tests;

fn restore_terminal() {
    let mut out = stdout();

    let _ = execute!(out, EndSynchronizedUpdate);
    let _ = out.write_all(termrock_core::shape::reset_sequence().as_bytes());
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

fn prime_input() -> std::io::Result<()> {
    poll_uninterrupted(Duration::ZERO).map(|_| ())
}

#[doc(hidden)]
pub fn poll_uninterrupted(wait: Duration) -> std::io::Result<bool> {
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

    /// crossterm reads the tty only when its edge-triggered poller reports it
    /// readable. When SIGWINCH and a key land in one poller batch it returns
    /// the resize and discards the tty's readiness, so the key waits unread
    /// until the next byte arrives. `queued` sees such input, and a termios
    /// write that changes a value makes the kernel signal the tty again. A
    /// write of unchanged attributes does not do that on macOS.
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
            let attrs = attrs.assume_init();
            let mut nudged = attrs;
            nudged.c_cc[libc::VMIN] = attrs.c_cc[libc::VMIN].wrapping_add(1);
            let changed = libc::tcsetattr(fd, libc::TCSANOW, &nudged) == 0;
            libc::tcsetattr(fd, libc::TCSANOW, &attrs) == 0 && changed
        }
    }
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
    /// Plumbing for the raw-input budget: `max_events`, with zero read as one.
    #[doc(hidden)]
    pub fn event_limit(&self) -> usize {
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

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct RawBudget {
    max_events: usize,
    max_duration: Duration,
    started: Instant,
    read: usize,
    filtered: usize,
}

impl RawBudget {
    #[doc(hidden)]
    pub fn new(policy: RawInputPolicy, started: Instant) -> Self {
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

    #[doc(hidden)]
    pub fn read(&self) -> usize {
        self.read
    }

    #[doc(hidden)]
    pub fn filtered(&self) -> usize {
        self.filtered
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawRead {
    Input(Input),
    Empty,
    Yielded,
}

#[doc(hidden)]
pub trait RawSource {
    fn poll(&mut self) -> std::io::Result<bool>;
    fn read(&mut self) -> std::io::Result<Event>;
    fn dropped(&mut self, event: &Event) -> bool;
    fn now(&mut self) -> Instant;
}

#[doc(hidden)]
pub struct CrosstermSource;

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

#[doc(hidden)]
pub fn scan_next(
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
        termrock_metrics::record_work!(RawEventsRead, 1usize);
        if source.dropped(&ev) {
            budget.filtered += 1;
            termrock_metrics::record_work!(RawEventsFiltered, 1usize);
            continue;
        }
        match Input::from_crossterm_with(ev, config) {
            Some(input) => {
                keyboard::observe(&input);
                return Ok(RawRead::Input(input));
            }
            None => {
                budget.filtered += 1;
                termrock_metrics::record_work!(RawEventsFiltered, 1usize);
            }
        }
    }
}

/// Discards every queued raw terminal event, however many arrive.
///
/// This never gives up while the queue keeps refilling. The shell's `run` does
/// not call it; use [`drain_pending_input_bounded`] at startup instead.
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

#[doc(hidden)]
pub trait RawModeControl {
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

#[doc(hidden)]
pub fn discard_pending(
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

#[doc(hidden)]
pub fn discard_ready(source: &mut impl RawSource, max_events: usize) -> std::io::Result<usize> {
    let mut discarded = 0;
    while discarded < max_events {
        match source.poll() {
            Ok(true) => {}
            Ok(false) => break,
            // A signal such as a resize's SIGWINCH can interrupt the zero-wait
            // poll. Nothing is ready then, as in `poll_uninterrupted`; it is
            // not a terminal failure, so startup must not abort on it.
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => break,
            Err(e) => return Err(e),
        }
        source.read()?;
        discarded += 1;
    }
    Ok(discarded)
}
