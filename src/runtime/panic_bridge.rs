//! Session ownership and terminal restoration shared by `Drop` and the
//! process panic hook.
//!
//! One [`RestoreGate`] decides which actor runs terminal cleanup for an
//! active interval, and who may claim the terminal next. One atomic word
//! holds the owner, the active interval, the phase and a deferred-release
//! bit, so the panic hook and `Drop` never take a lock and never wait.
//!
//! [`install_once`] puts one permanent bridge in the process panic hook. It
//! restores only a session that is active at that moment, then calls the hook
//! that was installed before it. Later sessions never touch the hook again,
//! and nothing here mutates the hook from `Drop` or while panicking.

use std::io;
use std::panic;
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};

const COUNTER_BITS: u32 = 30;
const COUNTER_MAX: u64 = (1 << COUNTER_BITS) - 1;
const PHASE_MASK: u64 = 0b11;
const RELEASE_PENDING: u64 = 0b100;
const INTERVAL_SHIFT: u32 = 4;
const OWNER_SHIFT: u32 = INTERVAL_SHIFT + COUNTER_BITS;
const ATTEMPTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Unowned,
    Inactive,
    Active,
    Restoring,
}

/// The decoded gate word. `Unowned` keeps the last counters so exhaustion is
/// checked without wrapping into a stale token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    owner: u64,
    interval: u64,
    phase: Phase,
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
        let pending = if self.release_pending {
            RELEASE_PENDING
        } else {
            0
        };
        (self.owner << OWNER_SHIFT) | (self.interval << INTERVAL_SHIFT) | pending | phase
    }

    fn unpack(word: u64) -> Self {
        let phase = match word & PHASE_MASK {
            0 => Phase::Unowned,
            1 => Phase::Inactive,
            2 => Phase::Active,
            _ => Phase::Restoring,
        };
        Self {
            owner: word >> OWNER_SHIFT,
            interval: (word >> INTERVAL_SHIFT) & COUNTER_MAX,
            phase,
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
    pub(super) fn activate(&self, token: &SessionToken) -> io::Result<()> {
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

    /// Cleans up when the gate still holds exactly the observed active
    /// interval. A later interval or owner makes the exchange fail.
    fn restore_observed(&self, observed: State) {
        if observed.phase != Phase::Active {
            return;
        }
        let restoring = observed.in_phase(Phase::Restoring);
        if self.swap(observed, restoring) {
            self.run_cleanup(restoring);
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

    fn run_cleanup(&self, restoring: State) {
        let _finish = Finish {
            gate: self,
            restoring,
        };
        (self.restore)();
    }

    fn finish(&self, restoring: State) {
        for _ in 0..ATTEMPTS {
            let current = self.load();
            debug_assert_eq!(current.phase, Phase::Restoring);
            debug_assert_eq!(
                (current.owner, current.interval),
                (restoring.owner, restoring.interval)
            );
            let next = State {
                phase: if current.release_pending {
                    Phase::Unowned
                } else {
                    Phase::Inactive
                },
                release_pending: false,
                ..current
            };
            if self.swap(current, next) {
                return;
            }
        }
    }
}

/// Publishes the end of a cleanup even when the restore callback unwinds.
struct Finish<'a> {
    gate: &'a RestoreGate,
    restoring: State,
}

impl Drop for Finish<'_> {
    fn drop(&mut self) {
        self.gate.finish(self.restoring);
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

    pub(super) fn activate(&self) -> io::Result<()> {
        self.gate.activate(&self.token)
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

static BRIDGE: Once = Once::new();

/// Installs the permanent panic-hook bridge for `gate` the first time it is
/// called in the process, and does nothing afterwards.
///
/// The bridge restores the active session of `gate`, then calls the hook that
/// was installed at that first call. A host that calls `set_hook` later
/// replaces the bridge, and the library does not reinstall it. The hook is
/// never touched from `Drop`, and a thread that is already panicking gets an
/// error instead of a hook mutation.
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
