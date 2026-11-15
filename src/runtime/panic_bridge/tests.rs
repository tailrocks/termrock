//! The gate cases use local gates and counting callbacks, so they run in the
//! shared test process. The hook cases replace the process panic hook and
//! each run in a child process through [`child_case`].

use std::io::{self, ErrorKind};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Barrier, Mutex};
use std::thread;

use super::{
    COUNTER_MAX, Phase, RestoreGate, RestoreResult, SessionOwner, Setup, State, install_once,
};
use crate::runtime::isolated::child_case;

fn leak(restore: fn()) -> &'static RestoreGate {
    Box::leak(Box::new(RestoreGate::new(restore)))
}

macro_rules! counting_gate {
    () => {{
        static RESTORES: AtomicUsize = AtomicUsize::new(0);
        fn restore() {
            RESTORES.fetch_add(1, Ordering::SeqCst);
        }
        (leak(restore), &RESTORES)
    }};
}

macro_rules! paused_gate {
    () => {{
        static RESTORES: AtomicUsize = AtomicUsize::new(0);
        static PAUSE: Barrier = Barrier::new(2);
        static PAUSING: AtomicBool = AtomicBool::new(false);
        fn restore() {
            RESTORES.fetch_add(1, Ordering::SeqCst);
            if PAUSING.load(Ordering::SeqCst) {
                PAUSE.wait();
                PAUSE.wait();
            }
        }
        (leak(restore), &RESTORES, &PAUSE, &PAUSING)
    }};
}

fn runs(count: &AtomicUsize) -> usize {
    count.load(Ordering::SeqCst)
}

#[test]
fn restore_gate_runs_once_per_active_interval() {
    let (gate, restores) = counting_gate!();
    let token = gate.claim().expect("claim");
    gate.activate(&token).expect("activate");
    assert_eq!(gate.restore_once(&token), RestoreResult::Restored);
    assert_eq!(gate.restore_once(&token), RestoreResult::Inactive);
    gate.request_release(&token);
    assert_eq!(runs(restores), 1, "leave then drop cleans up once");

    let token = gate.claim().expect("claim again");
    gate.activate(&token).expect("activate");
    assert_eq!(gate.restore_once(&token), RestoreResult::Restored);
    gate.activate(&token).expect("reactivate");
    assert_eq!(gate.restore_once(&token), RestoreResult::Restored);
    assert_eq!(runs(restores), 3, "reactivation allows one more cleanup");
    gate.request_release(&token);
    assert_eq!(runs(restores), 3);
}

#[test]
fn restore_gate_release_cleans_up_an_active_interval_once() {
    let (gate, restores) = counting_gate!();
    let token = gate.claim().expect("claim");
    gate.activate(&token).expect("activate");
    gate.request_release(&token);
    gate.request_release(&token);
    assert_eq!(runs(restores), 1);
    gate.claim().expect("released owner frees the gate");
}

#[test]
fn restore_gate_inactive_release_runs_no_cleanup() {
    let (gate, restores) = counting_gate!();
    let token = gate.claim().expect("claim");
    gate.request_release(&token);
    assert_eq!(runs(restores), 0);
    gate.claim().expect("claim after release");
}

#[test]
fn restore_gate_second_owner_is_rejected_until_release() {
    let (gate, _) = counting_gate!();
    let first = gate.claim().expect("first owner");
    let refused = gate.claim().expect_err("second owner");
    assert_eq!(refused.kind(), ErrorKind::AlreadyExists);
    gate.request_release(&first);
    gate.claim().expect("claim after release");
}

#[test]
fn restore_gate_stale_token_is_not_the_owner() {
    let (gate, restores) = counting_gate!();
    let old = gate.claim().expect("old owner");
    gate.request_release(&old);
    let current = gate.claim().expect("new owner");
    gate.activate(&current).expect("activate");

    assert_eq!(gate.restore_once(&old), RestoreResult::NotOwner);
    gate.request_release(&old);
    assert!(gate.activate(&old).is_err());
    assert_eq!(runs(restores), 0, "a stale token cleaned up the new owner");
    assert_eq!(gate.load().phase, Phase::Active);
}

#[test]
fn restore_gate_stale_snapshot_cannot_restore_a_later_interval() {
    let (gate, restores) = counting_gate!();
    let token = gate.claim().expect("claim");
    gate.activate(&token).expect("activate");
    let observed = gate.load();
    assert_eq!(observed.phase, Phase::Active);

    assert_eq!(gate.restore_once(&token), RestoreResult::Restored);
    gate.activate(&token).expect("reenter");
    gate.restore_observed(observed);

    assert_eq!(runs(restores), 1, "the stale hook restored a live terminal");
    assert_eq!(gate.load().phase, Phase::Active);
    assert_eq!(gate.restore_once(&token), RestoreResult::Restored);
}

#[test]
fn restore_gate_exhaustion_is_an_error_not_a_wrap() {
    let (gate, _) = counting_gate!();
    gate.word.store(
        State {
            owner: COUNTER_MAX,
            interval: 3,
            phase: Phase::Unowned,
            setup: Setup::Settled,
            release_pending: false,
        }
        .pack(),
        Ordering::SeqCst,
    );
    let err = gate.claim().expect_err("owner counter exhausted");
    assert!(err.to_string().contains("exhausted"), "{err}");

    let (gate, restores) = counting_gate!();
    let token = gate.claim().expect("claim");
    gate.word.store(
        State {
            owner: token.owner,
            interval: COUNTER_MAX,
            phase: Phase::Inactive,
            setup: Setup::Settled,
            release_pending: false,
        }
        .pack(),
        Ordering::SeqCst,
    );
    let err = gate
        .activate(&token)
        .expect_err("interval counter exhausted");
    assert!(err.to_string().contains("exhausted"), "{err}");
    assert_eq!(runs(restores), 0, "no cleanup before a refused activation");
}

#[test]
fn restore_gate_publishes_inactive_when_cleanup_panics() {
    child_case(
        module_path!(),
        "restore_gate_publishes_inactive_when_cleanup_panics",
        || {
            static CLEANUPS: AtomicUsize = AtomicUsize::new(0);
            fn failing() {
                CLEANUPS.fetch_add(1, Ordering::SeqCst);
                panic!("cleanup failed");
            }
            panic::set_hook(Box::new(|_| {}));
            let gate = leak(failing);
            let token = gate.claim().expect("claim");
            gate.activate(&token).expect("activate");
            let unwound = panic::catch_unwind(AssertUnwindSafe(|| gate.restore_once(&token)));

            assert!(unwound.is_err());
            assert_eq!(gate.load().phase, Phase::Inactive, "stuck in Restoring");
            assert_eq!(gate.restore_once(&token), RestoreResult::Inactive);
            assert_eq!(runs(&CLEANUPS), 1);
        },
    );
}

#[test]
fn panic_cleanup_blocks_new_owner_until_restore_finishes() {
    let (gate, restores, pause, pausing) = paused_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    owner.activate().expect("activate");

    pausing.store(true, Ordering::SeqCst);
    let hook = thread::spawn(move || gate.restore_current_for_panic());
    pause.wait();
    assert_eq!(gate.load().phase, Phase::Restoring);

    drop(owner);
    let refused = SessionOwner::claim(gate)
        .map(|_| ())
        .expect_err("claim while the hook is still restoring");
    assert_eq!(refused.kind(), ErrorKind::AlreadyExists);
    assert_eq!(gate.load().phase, Phase::Restoring, "drop released early");

    pause.wait();
    hook.join().expect("hook thread");
    pausing.store(false, Ordering::SeqCst);
    assert_eq!(runs(restores), 1, "drop repeated the cleanup");
    let next = SessionOwner::claim(gate).expect("claim after cleanup finished");
    next.activate().expect("activate the new owner");
    assert_eq!(gate.load().phase, Phase::Active);
}

#[test]
fn panic_cleanup_release_after_completion_publishes_unowned() {
    let (gate, restores) = counting_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    owner.activate().expect("activate");
    gate.restore_current_for_panic();
    assert_eq!(gate.load().phase, Phase::Inactive);
    drop(owner);
    assert_eq!(
        runs(restores),
        1,
        "drop after the hook repeated the cleanup"
    );
    SessionOwner::claim(gate).expect("claim after release");
}

#[test]
fn panic_cleanup_reenter_is_rejected_while_restoring() {
    let (gate, restores, pause, pausing) = paused_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    owner.activate().expect("activate");

    pausing.store(true, Ordering::SeqCst);
    let hook = thread::spawn(move || gate.restore_current_for_panic());
    pause.wait();
    let refused = owner.activate().expect_err("reenter while restoring");
    assert_eq!(refused.kind(), ErrorKind::WouldBlock);
    let refused = owner.restore().expect_err("leave while restoring");
    assert_eq!(refused.kind(), ErrorKind::WouldBlock);
    assert_eq!(runs(restores), 1, "interleaved cleanup");

    pause.wait();
    hook.join().expect("hook thread");
    pausing.store(false, Ordering::SeqCst);
    owner.activate().expect("reenter once cleanup finished");
    assert_eq!(gate.load().phase, Phase::Active);
}

#[test]
fn setup_window_keeps_the_interval_active_after_a_hook_restore() {
    let (gate, restores) = counting_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    let setup = owner.set_up(|| {
        gate.restore_current_for_panic();
        assert_eq!(
            gate.load().phase,
            Phase::Active,
            "hook ended a setup interval"
        );
        gate.restore_current_for_panic();
        Ok(())
    });
    setup.expect("setup");
    assert_eq!(runs(restores), 2, "the hook restores during setup");

    assert_eq!(gate.load().setup, Setup::Settled);
    owner.restore().expect("leave");
    assert_eq!(runs(restores), 3, "leave skipped the modes setup enabled");
    owner.restore().expect("second leave");
    assert_eq!(runs(restores), 3, "settled interval cleans up once");
}

#[test]
fn setup_window_ends_when_setup_fails() {
    let (gate, restores) = counting_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    owner
        .set_up(|| -> io::Result<()> {
            gate.restore_current_for_panic();
            Err(io::Error::other("setup failed"))
        })
        .expect_err("setup failed");
    assert_eq!(gate.load().setup, Setup::Settled);
    drop(owner);
    assert_eq!(runs(restores), 2, "failed setup skipped its cleanup");
}

#[test]
fn setup_window_ends_when_setup_unwinds() {
    let (gate, restores) = counting_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    let unwound = panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = owner.set_up(|| -> io::Result<()> {
            gate.restore_current_for_panic();
            panic!("setup panicked")
        });
    }));
    assert!(unwound.is_err());
    assert_eq!(gate.load().setup, Setup::Settled);
    drop(owner);
    assert_eq!(runs(restores), 2, "unwinding setup skipped its cleanup");
}

#[test]
fn setup_finished_during_a_cleanup_owes_another_cleanup() {
    let (gate, restores, pause, pausing) = paused_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    pausing.store(true, Ordering::SeqCst);
    let hook = thread::spawn(move || {
        pause.wait();
        gate.restore_current_for_panic();
    });
    let setup = owner.set_up(|| {
        pause.wait();
        pause.wait();
        assert_eq!(gate.load().phase, Phase::Restoring);
        Ok(())
    });
    setup.expect("setup");
    assert_eq!(gate.load().setup, Setup::FinishedDuringCleanup);
    pause.wait();
    hook.join().expect("hook thread");
    pausing.store(false, Ordering::SeqCst);

    assert_eq!(gate.load().phase, Phase::Active, "hook ended the interval");
    assert_eq!(gate.load().setup, Setup::Settled);
    owner.restore().expect("leave");
    assert_eq!(
        runs(restores),
        2,
        "leave skipped the cleanup setup still owed"
    );
}

#[test]
fn owner_dropped_during_a_setup_cleanup_cleans_up_again() {
    let (gate, restores, pause, pausing) = paused_gate!();
    let owner = SessionOwner::claim(gate).expect("claim");
    pausing.store(true, Ordering::SeqCst);
    let hook = thread::spawn(move || {
        pause.wait();
        gate.restore_current_for_panic();
    });
    let _ = owner.set_up(|| {
        pause.wait();
        pause.wait();
        Ok(())
    });
    drop(owner);
    assert_eq!(gate.load().phase, Phase::Restoring, "drop released early");
    pause.wait();
    pause.wait();
    pause.wait();
    hook.join().expect("hook thread");
    pausing.store(false, Ordering::SeqCst);

    assert_eq!(runs(restores), 2, "modes enabled after the hook stayed on");
    assert_eq!(gate.load().phase, Phase::Unowned);
    SessionOwner::claim(gate).expect("claim after release");
}

static HOST: AtomicUsize = AtomicUsize::new(0);
static RESTORES: AtomicUsize = AtomicUsize::new(0);

fn count_restore() {
    RESTORES.fetch_add(1, Ordering::SeqCst);
}

static GATE: RestoreGate = RestoreGate::new(count_restore);

fn set_host_hook(counter: &'static AtomicUsize) {
    panic::set_hook(Box::new(move |info| {
        eprintln!("host hook saw: {info}");
        counter.fetch_add(1, Ordering::SeqCst);
    }));
}

fn caught_panic(message: &'static str) {
    assert!(panic::catch_unwind(|| panic!("{message}")).is_err());
}

#[test]
fn panic_bridge_after_many_sessions_is_inactive() {
    child_case(
        module_path!(),
        "panic_bridge_after_many_sessions_is_inactive",
        || {
            set_host_hook(&HOST);
            for _ in 0..100 {
                install_once(&GATE).expect("install");
                let owner = SessionOwner::claim(&GATE).expect("claim");
                owner.activate().expect("activate");
                owner.restore().expect("leave");
            }
            assert_eq!(runs(&RESTORES), 100, "one cleanup per leave");

            caught_panic("unrelated");
            assert_eq!(runs(&HOST), 1, "host hook runs once, not once per session");
            assert_eq!(runs(&RESTORES), 100, "a panic after leave cleaned up");
        },
    );
}

#[test]
fn panic_bridge_active_panic_restores_once() {
    child_case(
        module_path!(),
        "panic_bridge_active_panic_restores_once",
        || {
            set_host_hook(&HOST);
            install_once(&GATE).expect("install");
            let owner = SessionOwner::claim(&GATE).expect("claim");
            owner.activate().expect("activate");

            let unwound = panic::catch_unwind(AssertUnwindSafe(move || {
                let _owner = owner;
                panic!("active session panic");
            }));
            assert!(unwound.is_err());
            assert_eq!(runs(&RESTORES), 1, "hook plus unwind cleaned up twice");
            assert_eq!(runs(&HOST), 1, "host hook delegated once");
            SessionOwner::claim(&GATE).expect("owner released by unwind");
        },
    );
}

#[test]
fn panic_bridge_respects_replacement_host_hook() {
    child_case(
        module_path!(),
        "panic_bridge_respects_replacement_host_hook",
        || {
            static REPLACEMENT: AtomicUsize = AtomicUsize::new(0);
            set_host_hook(&HOST);
            install_once(&GATE).expect("install");

            set_host_hook(&REPLACEMENT);
            install_once(&GATE).expect("later entry");
            let owner = SessionOwner::claim(&GATE).expect("claim");
            owner.activate().expect("activate");

            let unwound = panic::catch_unwind(AssertUnwindSafe(move || {
                let _owner = owner;
                panic!("panic under a replacement hook");
            }));
            assert!(unwound.is_err());
            assert_eq!(runs(&REPLACEMENT), 1, "replacement hook runs once");
            assert_eq!(runs(&HOST), 0, "the old bridge was reinstalled");
            assert_eq!(
                runs(&RESTORES),
                1,
                "only the unwinding drop cleans up under a replacement hook"
            );
        },
    );
}

struct InstallWhileUnwinding;

static INSTALL_WHILE_UNWINDING: Mutex<Option<bool>> = Mutex::new(None);

impl Drop for InstallWhileUnwinding {
    fn drop(&mut self) {
        let refused = install_once(&GATE).is_err();
        *INSTALL_WHILE_UNWINDING.lock().expect("record") = Some(refused);
    }
}

#[test]
fn panic_bridge_first_install_panicking_thread_returns_error() {
    child_case(
        module_path!(),
        "panic_bridge_first_install_panicking_thread_returns_error",
        || {
            set_host_hook(&HOST);
            let unwound = panic::catch_unwind(|| {
                let _install = InstallWhileUnwinding;
                panic!("unwinding");
            });
            assert!(unwound.is_err());
            assert_eq!(
                *INSTALL_WHILE_UNWINDING.lock().expect("recorded"),
                Some(true),
                "install must fail while panicking"
            );
            assert_eq!(runs(&HOST), 1, "the hook was changed while panicking");

            install_once(&GATE).expect("ordinary entry stays compatible");
            caught_panic("after install");
            assert_eq!(runs(&HOST), 2, "the bridge delegates to the host hook");
        },
    );
}
