//! Session ownership and terminal restoration shared by `Drop` and the
//! process panic hook.
//!
//! One [`RestoreGate`] decides which actor runs terminal cleanup for an
//! active interval, and who may claim the terminal next. One atomic word
//! holds the owner, the active interval, the phase, the setup progress and a
//! deferred-release bit, so the panic hook and `Drop` never take a lock and never wait.
//!
//! [`install_once`] puts one permanent bridge in the process panic hook. It
//! restores only a session that is active at that moment, then calls the hook
//! that was installed before it. Later sessions never touch the hook again,
//! and nothing here mutates the hook from `Drop` or while panicking.

use std::io;
use std::panic;
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};

const COUNTER_BITS: u32 = 29;
const COUNTER_MAX: u64 = (1 << COUNTER_BITS) - 1;
const PHASE_MASK: u64 = 0b11;
const RELEASE_PENDING: u64 = 0b100;
const SETUP_SHIFT: u32 = 3;
const SETUP_MASK: u64 = 0b11;
const INTERVAL_SHIFT: u32 = 5;
const OWNER_SHIFT: u32 = INTERVAL_SHIFT + COUNTER_BITS;
const ATTEMPTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Unowned,
    Inactive,
    Active,
    Restoring,
}

/// Whether the interval's terminal setup may still enable modes after a
/// cleanup already ran. Such a cleanup leaves the interval owing another one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setup {
    Settled,
    Running,
    /// Setup finished while a cleanup was still running.
    FinishedDuringCleanup,
}

/// The decoded gate word. `Unowned` keeps the last counters so exhaustion is
/// checked without wrapping into a stale token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    owner: u64,
    interval: u64,
    phase: Phase,
    setup: Setup,
    release_pending: bool,
}

impl State {
    fn pack(self) -> u64 {
        let phase = match self.phase {
            Phase::Unowned => 0,
            Phase::Inactive => 1,
            Phase::Active => 2,
            Phase::Restoring => 3,
        };
        let setup = match self.setup {
            Setup::Settled => 0,
            Setup::Running => 1,
            Setup::FinishedDuringCleanup => 2,
        };
        let pending = if self.release_pending {
            RELEASE_PENDING
        } else {
            0
        };
        (self.owner << OWNER_SHIFT)
            | (self.interval << INTERVAL_SHIFT)
            | (setup << SETUP_SHIFT)
            | pending
            | phase
    }

    fn unpack(word: u64) -> Self {
        let phase = match word & PHASE_MASK {
            0 => Phase::Unowned,
            1 => Phase::Inactive,
            2 => Phase::Active,
            _ => Phase::Restoring,
        };
        let setup = match (word >> SETUP_SHIFT) & SETUP_MASK {
            0 => Setup::Settled,
            1 => Setup::Running,
            _ => Setup::FinishedDuringCleanup,
        };
        Self {
            owner: word >> OWNER_SHIFT,
            interval: (word >> INTERVAL_SHIFT) & COUNTER_MAX,
            phase,
            setup,
            release_pending: word & RELEASE_PENDING != 0,
        }
    }

    fn in_phase(self, phase: Phase) -> Self {
        Self { phase, ..self }
    }
}

/// Proof of one claimed session. Not `Clone`: only the owner releases it.
#[derive(Debug)]
pub(super) struct SessionToken {
    owner: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RestoreResult {
    Restored,
    Inactive,
    InProgress,
    NotOwner,
}

pub(super) struct RestoreGate {
    word: AtomicU64,
    restore: fn(),
}

fn exhausted() -> io::Error {
    io::Error::other("terminal lifecycle generation exhausted")
}

impl RestoreGate {
    pub(super) const fn new(restore: fn()) -> Self {
        Self {
            word: AtomicU64::new(0),
            restore,
        }
    }

    fn load(&self) -> State {
        State::unpack(self.word.load(Ordering::SeqCst))
    }

    fn swap(&self, from: State, to: State) -> bool {
        self.word
            .compare_exchange(from.pack(), to.pack(), Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Takes exclusive ownership without touching the terminal.
    ///
    /// Fails with `AlreadyExists` while another session owns the gate,
    /// including while that session's cleanup is still running.
    pub(super) fn claim(&self) -> io::Result<SessionToken> {
        let current = self.load();
        if current.phase != Phase::Unowned {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a terminal session is already active",
            ));
        }
        if current.owner == COUNTER_MAX {
            return Err(exhausted());
        }
        let next = State {
            owner: current.owner + 1,
            interval: 0,
            phase: Phase::Inactive,
            setup: Setup::Settled,
            release_pending: false,
        };
        if self.swap(current, next) {
            Ok(SessionToken { owner: next.owner })
        } else {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a terminal session is already active",
            ))
        }
    }

    /// Starts a new active interval. Call it before the first terminal
    /// operation that needs cleanup.
    ///
    /// Fails with `WouldBlock` while a cleanup of this owner is running.
    #[cfg(test)]
    pub(super) fn activate(&self, token: &SessionToken) -> io::Result<()> {
        self.activate_with(token, Setup::Settled)
    }

    /// [`activate`](Self::activate) for an interval whose terminal setup is
    /// still to run. A cleanup that runs before
    /// [`finish_setup`](Self::finish_setup) leaves the interval active, so
    /// leave or drop cleans up what setup enables afterwards.
    pub(super) fn activate_setup(&self, token: &SessionToken) -> io::Result<()> {
        self.activate_with(token, Setup::Running)
    }

    fn activate_with(&self, token: &SessionToken, setup: Setup) -> io::Result<()> {
        let current = self.load();
        if current.owner != token.owner {
            return Err(io::Error::other("terminal session token is stale"));
        }
        match (current.phase, current.release_pending) {
            (Phase::Inactive, false) => {}
            (Phase::Restoring, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "terminal cleanup is still in progress",
                ));
            }
            _ => return Err(io::Error::other("terminal session is not inactive")),
        }
        if current.interval == COUNTER_MAX {
            return Err(exhausted());
        }
        let next = State {
            interval: current.interval + 1,
            phase: Phase::Active,
            setup,
            ..current
        };
        if self.swap(current, next) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "terminal cleanup is still in progress",
            ))
        }
    }

    /// Ends the setup that [`activate_setup`](Self::activate_setup) began.
    /// Call it once setup stops enabling modes, whether it succeeded or not.
    ///
    /// Retries until the exchange succeeds or the setup is already over. A
    /// failed exchange means another actor changed the word, and each actor's
    /// transitions are finite, so the loop never waits on a thread. A bounded
    /// loop could give up and leave `Running` set, which keeps every later
    /// cleanup of the interval owing another one.
    pub(super) fn finish_setup(&self, token: &SessionToken) {
        loop {
            let current = self.load();
            if current.owner != token.owner {
                return;
            }
            let setup = match (current.phase, current.setup) {
                (_, Setup::Settled | Setup::FinishedDuringCleanup) | (Phase::Unowned, _) => return,
                (Phase::Restoring, Setup::Running) => Setup::FinishedDuringCleanup,
                (_, Setup::Running) => Setup::Settled,
            };
            if self.swap(current, State { setup, ..current }) {
                return;
            }
        }
    }

    /// Runs the cleanup once for the owner's active interval.
    pub(super) fn restore_once(&self, token: &SessionToken) -> RestoreResult {
        for _ in 0..ATTEMPTS {
            let current = self.load();
            if current.owner != token.owner {
                return RestoreResult::NotOwner;
            }
            match current.phase {
                Phase::Unowned => return RestoreResult::NotOwner,
                Phase::Inactive => return RestoreResult::Inactive,
                Phase::Restoring => return RestoreResult::InProgress,
                Phase::Active => {
                    let restoring = current.in_phase(Phase::Restoring);
                    if self.swap(current, restoring) {
                        self.run_cleanup(restoring);
                        return RestoreResult::Restored;
                    }
                }
            }
        }
        RestoreResult::InProgress
    }

    /// The panic hook entry: cleans up only a session active right now.
    pub(super) fn restore_current_for_panic(&self) {
        self.restore_observed(self.load());
    }

    /// Cleans up the observed active interval. Another actor may change the
    /// word's setup bits or release flag first, such as `finish_setup`, so a
    /// failed exchange reloads and retries while the gate still holds the same
    /// owner and interval in the active phase. A later interval or owner, or
    /// a cleanup another actor already started, ends the retries.
    fn restore_observed(&self, observed: State) {
        let mut current = observed;
        for _ in 0..ATTEMPTS {
            if current.phase != Phase::Active {
                return;
            }
            let restoring = current.in_phase(Phase::Restoring);
            if self.swap(current, restoring) {
                self.run_cleanup(restoring);
                return;
            }
            current = self.load();
            if (current.owner, current.interval) != (observed.owner, observed.interval) {
                return;
            }
        }
    }

    /// Releases the owner. Never publishes `Unowned` while cleanup runs: an
    /// active interval is cleaned up here, and a cleanup already running
    /// releases when it finishes.
    pub(super) fn request_release(&self, token: &SessionToken) {
        for _ in 0..ATTEMPTS {
            let current = self.load();
            if current.owner != token.owner {
                return;
            }
            match current.phase {
                Phase::Unowned => return,
                Phase::Inactive => {
                    if self.swap(current, current.in_phase(Phase::Unowned)) {
                        return;
                    }
                }
                Phase::Active => {
                    let restoring = State {
                        phase: Phase::Restoring,
                        setup: Setup::Settled,
                        release_pending: true,
                        ..current
                    };
                    if self.swap(current, restoring) {
                        self.run_cleanup(restoring);
                        return;
                    }
                }
                Phase::Restoring => {
                    if current.release_pending {
                        return;
                    }
                    let pending = State {
                        release_pending: true,
                        ..current
                    };
                    if self.swap(current, pending) {
                        return;
                    }
                }
            }
        }
    }

    fn run_cleanup(&self, mut restoring: State) {
        loop {
            let mut finish = Finish {
                gate: self,
                restoring: Some(restoring),
            };
            (self.restore)();
            match finish.complete() {
                Some(again) => restoring = again,
                None => return,
            }
        }
    }

    /// Publishes the end of a cleanup. Returns the state of one more cleanup
    /// when setup may have enabled modes after this one began and the owner
    /// is already gone, so nobody else will clean them up.
    fn finish(&self, restoring: State) -> Option<State> {
        for _ in 0..ATTEMPTS {
            let current = self.load();
            debug_assert_eq!(current.phase, Phase::Restoring);
            debug_assert_eq!(
                (current.owner, current.interval),
                (restoring.owner, restoring.interval)
            );
            let (phase, setup) = match (current.release_pending, current.setup) {
                (true, Setup::Settled) => (Phase::Unowned, Setup::Settled),
                (true, _) => (Phase::Restoring, Setup::Settled),
                (false, Setup::Settled) => (Phase::Inactive, Setup::Settled),
                (false, Setup::Running) => (Phase::Active, Setup::Running),
                (false, Setup::FinishedDuringCleanup) => (Phase::Active, Setup::Settled),
            };
            let next = State {
                phase,
                setup,
                release_pending: phase == Phase::Restoring,
                ..current
            };
            if self.swap(current, next) {
                return (phase == Phase::Restoring).then_some(next);
            }
        }
        None
    }
}

/// Publishes the end of a cleanup even when the restore callback unwinds.
struct Finish<'a> {
    gate: &'a RestoreGate,
    restoring: Option<State>,
}

impl Finish<'_> {
    fn complete(&mut self) -> Option<State> {
        self.restoring
            .take()
            .and_then(|restoring| self.gate.finish(restoring))
    }
}

impl Drop for Finish<'_> {
    fn drop(&mut self) {
        let mut next = self.restoring.take();
        while let Some(restoring) = next {
            next = self.gate.finish(restoring);
        }
    }
}

/// The session's claim on a gate. Dropping it cleans up an active interval
/// once and releases the gate, deferring the release while another thread's
/// cleanup is still running.
pub(super) struct SessionOwner {
    gate: &'static RestoreGate,
    token: SessionToken,
}

impl SessionOwner {
    pub(super) fn claim(gate: &'static RestoreGate) -> io::Result<Self> {
        Ok(Self {
            gate,
            token: gate.claim()?,
        })
    }

    #[cfg(test)]
    pub(super) fn activate(&self) -> io::Result<()> {
        self.gate.activate(&self.token)
    }

    /// Activates an interval, runs `setup`, then ends the setup window
    /// whatever `setup` returned. A cleanup that ran in the meantime, such as
    /// a panic hook on another thread, no longer ends the interval: leave and
    /// drop still clean up what `setup` enabled after it.
    pub(super) fn set_up<T>(&self, setup: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        self.gate.activate_setup(&self.token)?;
        let _settle = SettleSetup(self);
        setup()
    }

    /// Ends the active interval, running the cleanup unless it already ran.
    /// `WouldBlock` means another thread is cleaning up right now.
    pub(super) fn restore(&self) -> io::Result<()> {
        match self.gate.restore_once(&self.token) {
            RestoreResult::Restored | RestoreResult::Inactive => Ok(()),
            RestoreResult::InProgress => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "terminal cleanup is still in progress",
            )),
            RestoreResult::NotOwner => Err(io::Error::other("terminal session token is stale")),
        }
    }
}

impl Drop for SessionOwner {
    fn drop(&mut self) {
        self.gate.request_release(&self.token);
    }
}

/// Ends the setup window when `setup` returns or unwinds.
struct SettleSetup<'a>(&'a SessionOwner);

impl Drop for SettleSetup<'_> {
    fn drop(&mut self) {
        self.0.gate.finish_setup(&self.0.token);
    }
}

static BRIDGE: Once = Once::new();

/// Installs the permanent panic-hook bridge for `gate` the first time it is
/// called in the process, and does nothing afterwards.
///
/// The bridge restores the active session of `gate`, then calls the hook that
/// was installed at that first call. A host that calls `set_hook` later
/// replaces the bridge, and the library does not reinstall it. The hook is
/// never touched from `Drop`, and a thread that is already panicking gets an
/// error instead of a hook mutation.
///
/// The bridge belongs to the first `gate` only. Later calls ignore their
/// `gate`, so a process has one gate for the library's sessions.
pub(super) fn install_once(gate: &'static RestoreGate) -> io::Result<()> {
    if BRIDGE.is_completed() {
        return Ok(());
    }
    if std::thread::panicking() {
        return Err(io::Error::other(
            "the panic hook cannot be installed while panicking",
        ));
    }
    BRIDGE.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            gate.restore_current_for_panic();
            previous(info);
        }));
    });
    Ok(())
}

#[cfg(test)]
mod tests;
