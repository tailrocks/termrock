//! Session-level cases. Each runs in a child process through [`child_case`]
//! because entry installs the process panic bridge and claims SIGTSTP.
//! Terminal setup is a closure over a fixed-size terminal, so no case needs a
//! TTY or writes a terminal mode.

use std::io::{self, ErrorKind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, Mutex};
use std::thread;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{DefaultTerminal, Terminal, TerminalOptions, Viewport};

use super::TerminalSession;
use super::isolated::child_case;
use super::job_control;
use super::panic_bridge::RestoreGate;

fn fixed_terminal() -> io::Result<DefaultTerminal> {
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 10, 3)),
        },
    )
}

fn runs(count: &AtomicUsize) -> usize {
    count.load(Ordering::SeqCst)
}

#[test]
fn terminal_session_second_owner_is_rejected() {
    child_case(
        module_path!(),
        "terminal_session_second_owner_is_rejected",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static SETUPS: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });
            let open = || {
                SETUPS.fetch_add(1, Ordering::SeqCst);
                fixed_terminal()
            };

            let mut live = TerminalSession::enter_on(&GATE, open).expect("first session");
            let refused = TerminalSession::enter_on(&GATE, open)
                .map(|_| ())
                .expect_err("nested entry");
            assert_eq!(refused.kind(), ErrorKind::AlreadyExists);
            assert_eq!(runs(&SETUPS), 1, "the refused entry ran terminal setup");
            assert_eq!(
                runs(&RESTORES),
                0,
                "the refused entry tore down the live session"
            );

            assert!(live.job_control.is_some(), "the live session lost SIGTSTP");
            live.try_leave().expect("leave");
            assert_eq!(runs(&RESTORES), 1);
            let still_owned = TerminalSession::enter_on(&GATE, open)
                .map(|_| ())
                .expect_err("a left session still owns the terminal until it drops");
            assert_eq!(still_owned.kind(), ErrorKind::AlreadyExists);

            drop(live);
            assert_eq!(runs(&RESTORES), 1, "drop repeated the cleanup");
            TerminalSession::enter_on(&GATE, open).expect("entry after the session dropped");
        },
    );
}

#[test]
fn terminal_failed_setup_deactivates_gate() {
    child_case(
        module_path!(),
        "terminal_failed_setup_deactivates_gate",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });

            let failed =
                TerminalSession::enter_on(&GATE, || Err(io::Error::other("raw mode failed")))
                    .map(|_| ())
                    .expect_err("setup failure");
            assert_eq!(failed.to_string(), "raw mode failed");
            assert_eq!(runs(&RESTORES), 1, "failed setup attempts cleanup once");

            let session = TerminalSession::enter_on(&GATE, fixed_terminal)
                .expect("the failed entry released ownership");
            drop(session);
            assert_eq!(runs(&RESTORES), 2);
        },
    );
}

#[test]
fn terminal_session_install_failure_cleans_up_and_releases() {
    child_case(
        module_path!(),
        "terminal_session_install_failure_cleans_up_and_releases",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });

            job_control::fail_next_install();
            TerminalSession::enter_on(&GATE, fixed_terminal)
                .map(|_| ())
                .expect_err("injected SIGTSTP install failure");
            assert_eq!(runs(&RESTORES), 1, "terminal restored once");

            let session = TerminalSession::enter_on(&GATE, fixed_terminal)
                .expect("ownership and SIGTSTP were released");
            drop(session);
            assert_eq!(runs(&RESTORES), 2);
        },
    );
}

#[test]
fn terminal_session_try_leave_failure_keeps_the_guard() {
    child_case(
        module_path!(),
        "terminal_session_try_leave_failure_keeps_the_guard",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });
            let mut session = TerminalSession::enter_on(&GATE, fixed_terminal).expect("enter");

            job_control::fail_next_restore();
            session
                .try_leave()
                .expect_err("injected SIGTSTP restore failure");
            assert_eq!(runs(&RESTORES), 1, "terminal restored before the signal");
            assert!(
                session.job_control.is_some(),
                "failed restore dropped the guard"
            );
            let owned = job_control::install().map(|_| ()).expect_err("claim kept");
            assert_eq!(owned.kind(), ErrorKind::AlreadyExists);

            session.try_leave().expect("retry");
            assert!(session.job_control.is_none());
            assert_eq!(runs(&RESTORES), 1, "retry repeated the terminal cleanup");
            drop(job_control::install().expect("SIGTSTP released by the retry"));
        },
    );
}

#[test]
fn panic_cleanup_try_leave_is_rejected_while_restoring() {
    child_case(
        module_path!(),
        "panic_cleanup_try_leave_is_rejected_while_restoring",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static PAUSE: Barrier = Barrier::new(2);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
                PAUSE.wait();
                PAUSE.wait();
            });
            let mut session = TerminalSession::enter_on(&GATE, fixed_terminal).expect("enter");

            let hook = thread::spawn(|| GATE.restore_current_for_panic());
            PAUSE.wait();
            let busy = session
                .try_leave()
                .expect_err("leave while the hook restores");
            assert_eq!(busy.kind(), ErrorKind::WouldBlock);
            assert!(session.job_control.is_some(), "guard released mid-cleanup");

            PAUSE.wait();
            hook.join().expect("hook thread");
            session.try_leave().expect("leave after the hook finished");
            assert_eq!(runs(&RESTORES), 1, "leave repeated the hook's cleanup");
            assert!(session.job_control.is_none());
        },
    );
}

#[test]
fn terminal_session_drop_restores_terminal_before_signal() {
    child_case(
        module_path!(),
        "terminal_session_drop_restores_terminal_before_signal",
        || {
            static ORDER: Mutex<Vec<&str>> = Mutex::new(Vec::new());
            static GATE: RestoreGate = RestoreGate::new(|| {
                let held = job_control::install().is_err();
                ORDER.lock().expect("order").push(if held {
                    "terminal while SIGTSTP owned"
                } else {
                    "terminal after SIGTSTP released"
                });
            });
            drop(TerminalSession::enter_on(&GATE, fixed_terminal).expect("enter"));
            assert_eq!(
                *ORDER.lock().expect("order"),
                ["terminal while SIGTSTP owned"]
            );
            drop(job_control::install().expect("SIGTSTP released after the terminal"));
        },
    );
}

fn caught_panic_on_another_thread() {
    let worker = thread::spawn(|| std::panic::catch_unwind(|| panic!("worker panic during setup")));
    assert!(worker.join().expect("worker").is_err());
}

#[test]
fn terminal_panic_on_another_thread_during_setup_still_restores_at_leave() {
    child_case(
        module_path!(),
        "terminal_panic_on_another_thread_during_setup_still_restores_at_leave",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });

            let mut session = TerminalSession::enter_on(&GATE, || {
                caught_panic_on_another_thread();
                fixed_terminal()
            })
            .expect("enter");
            assert_eq!(
                runs(&RESTORES),
                1,
                "the hook restored the half-set-up terminal"
            );

            session.try_leave().expect("leave");
            assert_eq!(
                runs(&RESTORES),
                2,
                "leave skipped the modes setup enabled after the hook ran"
            );
            drop(session);
            assert_eq!(runs(&RESTORES), 2, "drop repeated the cleanup");
        },
    );
}

#[test]
fn terminal_panic_on_another_thread_during_setup_still_restores_at_drop() {
    child_case(
        module_path!(),
        "terminal_panic_on_another_thread_during_setup_still_restores_at_drop",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });

            let session = TerminalSession::enter_on(&GATE, || {
                caught_panic_on_another_thread();
                fixed_terminal()
            })
            .expect("enter");
            drop(session);
            assert_eq!(runs(&RESTORES), 2, "drop skipped the post-hook modes");
        },
    );
}

#[test]
fn terminal_failed_setup_after_a_panic_on_another_thread_still_restores() {
    child_case(
        module_path!(),
        "terminal_failed_setup_after_a_panic_on_another_thread_still_restores",
        || {
            static RESTORES: AtomicUsize = AtomicUsize::new(0);
            static GATE: RestoreGate = RestoreGate::new(|| {
                RESTORES.fetch_add(1, Ordering::SeqCst);
            });

            TerminalSession::enter_on(&GATE, || {
                caught_panic_on_another_thread();
                Err(io::Error::other("raw mode failed"))
            })
            .map(|_| ())
            .expect_err("setup failure");
            assert_eq!(runs(&RESTORES), 2, "the failed setup skipped its cleanup");
            TerminalSession::enter_on(&GATE, fixed_terminal).expect("ownership released");
        },
    );
}

#[cfg(unix)]
#[test]
fn input_queued_behind_a_resize_event_is_delivered() {
    use std::time::{Duration, Instant};

    use ratatui::crossterm::event::{self, Event, KeyCode};
    use ratatui::crossterm::terminal::enable_raw_mode;

    use super::poll_input;

    child_case(
        module_path!(),
        "input_queued_behind_a_resize_event_is_delivered",
        || {
            let master = unsafe {
                let (mut master, mut slave) = (0, 0);
                let (name, attrs, size) = (
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                assert_eq!(libc::openpty(&mut master, &mut slave, name, attrs, size), 0);
                assert!(libc::setsid() > 0, "own session");
                assert_eq!(libc::ioctl(slave, libc::TIOCSCTTY as _, 0), 0);
                assert_eq!(libc::dup2(slave, libc::STDIN_FILENO), libc::STDIN_FILENO);
                master
            };
            enable_raw_mode().expect("raw mode");
            let _ = ratatui::crossterm::terminal::size();
            assert!(
                !poll_input(Duration::ZERO).expect("prime"),
                "queue is empty"
            );

            // SIGWINCH is ready first, then the key. crossterm returns the
            // resize from that batch and drops the readiness of the tty.
            unsafe { assert_eq!(libc::raise(libc::SIGWINCH), 0) };
            std::thread::sleep(Duration::from_millis(20));
            assert_eq!(unsafe { libc::write(master, b"r".as_ptr().cast(), 1) }, 1);
            std::thread::sleep(Duration::from_millis(50));

            let deadline = Instant::now() + Duration::from_secs(2);
            let mut seen = Vec::new();
            while Instant::now() < deadline {
                if poll_input(Duration::from_millis(200)).expect("poll") {
                    let event = event::read().expect("read");
                    let key = matches!(&event, Event::Key(k) if k.code == KeyCode::Char('r'));
                    seen.push(event);
                    if key {
                        return;
                    }
                }
            }
            panic!("the queued `r` never arrived; events seen: {seen:?}");
        },
    );
}
