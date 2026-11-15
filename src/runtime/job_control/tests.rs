//! SIGTSTP is process-global state, so no case here touches the signal
//! disposition of the process that runs the test. Each `#[test]` is a
//! parent that re-executes this test binary, filtered to itself, with
//! [`MARKER`] set to its own name. Only that child process installs
//! handlers, raises signals or arms the fault injector. nextest already gives
//! every test its own process. The parent/child split keeps the guarantee
//! for any other runner that shares one process between tests.

use std::ffi::c_int;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use super::{OWNED, REQUESTED, Step, install, on_tstp, take_request};

const MARKER: &str = "JUNIE_TUI_JOB_CONTROL_CHILD";
const DONE: &str = "job-control-child-done";

static FAULT: AtomicU8 = AtomicU8::new(0);
static HOST_COUNT: AtomicUsize = AtomicUsize::new(0);

pub(super) fn fault_armed(step: Step) -> bool {
    let armed = step as u8 + 1;
    FAULT
        .compare_exchange(armed, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub(super) fn arm(step: Step) {
    FAULT.store(step as u8 + 1, Ordering::SeqCst);
}

fn is_child(name: &str) -> bool {
    std::env::var(MARKER).as_deref() == Ok(name)
}

fn child_command(name: &str) -> Command {
    let module = module_path!().split_once("::").expect("crate prefix").1;
    let mut command = Command::new(std::env::current_exe().expect("test binary path"));
    command
        .args([
            "--exact",
            &format!("{module}::{name}"),
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MARKER, name)
        .stdin(Stdio::null());
    command
}

fn assert_child_passed(name: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains(DONE),
        "child {name} failed: {}\n--- stdout\n{stdout}\n--- stderr\n{stderr}",
        output.status
    );
}

fn child_case(name: &str, body: impl FnOnce()) {
    if is_child(name) {
        body();
        println!("{DONE}");
        return;
    }
    let output = child_command(name).output().expect("spawn child");
    assert_child_passed(name, &output);
}

extern "C" fn host_handler(_sig: c_int) {
    HOST_COUNT.fetch_add(1, Ordering::SeqCst);
}

fn host_handler_address() -> libc::sighandler_t {
    host_handler as *const () as libc::sighandler_t
}

fn query() -> libc::sigaction {
    let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::sigaction(libc::SIGTSTP, std::ptr::null(), &mut current) };
    assert_eq!(rc, 0, "query SIGTSTP action");
    current
}

fn install_host(handler: libc::sighandler_t, flags: c_int, masked: &[c_int]) -> libc::sigaction {
    let mut act: libc::sigaction = unsafe { std::mem::zeroed() };
    act.sa_sigaction = handler;
    act.sa_flags = flags;
    unsafe { libc::sigemptyset(&mut act.sa_mask) };
    for signal in masked {
        unsafe { libc::sigaddset(&mut act.sa_mask, *signal) };
    }
    let rc = unsafe { libc::sigaction(libc::SIGTSTP, &act, std::ptr::null_mut()) };
    assert_eq!(rc, 0, "install host SIGTSTP action");
    query()
}

fn masked(action: &libc::sigaction) -> Vec<c_int> {
    (1..32)
        .filter(|signal| unsafe { libc::sigismember(&action.sa_mask, *signal) } == 1)
        .collect()
}

fn assert_same_action(actual: &libc::sigaction, expected: &libc::sigaction) {
    assert_eq!(actual.sa_sigaction, expected.sa_sigaction, "handler");
    assert_eq!(actual.sa_flags, expected.sa_flags, "flags");
    assert_eq!(masked(actual), masked(expected), "mask");
}

fn assert_library_action(actual: &libc::sigaction) {
    assert_eq!(
        actual.sa_sigaction,
        on_tstp as *const () as libc::sighandler_t
    );
    assert_eq!(actual.sa_flags & libc::SA_RESTART, 0, "no SA_RESTART");
    assert!(masked(actual).is_empty(), "empty mask");
}

fn raise_tstp() {
    assert_eq!(unsafe { libc::raise(libc::SIGTSTP) }, 0, "raise SIGTSTP");
}

fn tstp_set() -> libc::sigset_t {
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTSTP);
    }
    set
}

fn block_tstp() {
    let set = tstp_set();
    let rc = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) };
    assert_eq!(rc, 0, "block SIGTSTP");
}

fn unblock_tstp() {
    let set = tstp_set();
    let rc = unsafe { libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut()) };
    assert_eq!(rc, 0, "unblock SIGTSTP");
}

fn host_with_state() -> libc::sigaction {
    let host = install_host(
        host_handler_address(),
        libc::SA_RESTART,
        &[libc::SIGUSR1, libc::SIGUSR2],
    );
    assert_ne!(host.sa_flags & libc::SA_RESTART, 0, "fixture keeps flags");
    assert_eq!(
        masked(&host),
        [libc::SIGUSR1, libc::SIGUSR2],
        "fixture mask"
    );
    host
}

#[test]
fn job_control_restores_exact_host_action() {
    child_case("job_control_restores_exact_host_action", || {
        let host = host_with_state();
        let guard = install().expect("install");
        assert_library_action(&query());
        drop(guard);
        assert_same_action(&query(), &host);
        raise_tstp();
        assert_eq!(HOST_COUNT.load(Ordering::SeqCst), 1, "host handler runs");
        assert!(!take_request(), "library flag stays untouched");
    });
}

fn failing_scope() -> io::Result<()> {
    let _guard = install()?;
    assert_library_action(&query());
    Err(io::Error::other("event loop failed"))
}

#[test]
fn job_control_restores_on_error_and_unwind() {
    child_case("job_control_restores_on_error_and_unwind", || {
        let host = host_with_state();
        assert!(failing_scope().is_err());
        assert_same_action(&query(), &host);

        let unwound = std::panic::catch_unwind(|| {
            let _guard = install().expect("install");
            panic!("unwind through the guard");
        });
        assert!(unwound.is_err());
        assert_same_action(&query(), &host);
        assert!(install().is_ok(), "claim released after unwind");
    });
}

#[test]
fn job_control_repeated_sessions_clear_pending() {
    child_case("job_control_repeated_sessions_clear_pending", || {
        host_with_state();
        let first = install().expect("first owner");
        raise_tstp();
        drop(first);
        let second = install().expect("second owner");
        assert!(!take_request(), "request from the first owner crossed over");
        drop(second);

        REQUESTED.store(true, Ordering::SeqCst);
        let third = install().expect("third owner");
        assert!(!take_request(), "stale request survived the claim");
        drop(third);
    });
}

#[test]
fn job_control_rejects_second_owner() {
    child_case("job_control_rejects_second_owner", || {
        let host = host_with_state();
        let first = install().expect("first owner");
        raise_tstp();
        let before = query();

        let refused = install().expect_err("second owner");
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert_same_action(&query(), &before);
        assert!(
            take_request(),
            "refused install must keep the pending request"
        );

        drop(first);
        assert_same_action(&query(), &host);
        assert!(install().is_ok(), "claim released with the first guard");
    });
}

#[test]
fn job_control_preserves_ignored_action() {
    child_case("job_control_preserves_ignored_action", || {
        let host = install_host(
            libc::SIG_IGN,
            libc::SA_RESTART | libc::SA_NOCLDSTOP,
            &[libc::SIGUSR1],
        );
        assert_eq!(host.sa_sigaction, libc::SIG_IGN);
        drop(install().expect("install"));
        assert_same_action(&query(), &host);
        raise_tstp();
        assert_eq!(HOST_COUNT.load(Ordering::SeqCst), 0);
        assert!(!take_request());
    });
}

#[test]
fn job_control_install_failure_releases_claim() {
    child_case("job_control_install_failure_releases_claim", || {
        let host = host_with_state();
        arm(Step::Install);
        let failed = install().expect_err("injected install failure");
        assert_ne!(failed.kind(), io::ErrorKind::AlreadyExists);
        assert_same_action(&query(), &host);
        assert!(!OWNED.load(Ordering::SeqCst));
        drop(install().expect("retry after failed install"));
        assert_same_action(&query(), &host);
    });
}

#[test]
fn job_control_restore_failure_keeps_ownership() {
    child_case("job_control_restore_failure_keeps_ownership", || {
        let host = host_with_state();
        let mut guard = install().expect("install");
        arm(Step::Restore);
        guard.restore().expect_err("injected restore failure");
        assert_library_action(&query());
        let refused = install().expect_err("claim must survive a failed restore");
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);

        guard.restore().expect("explicit retry");
        assert_same_action(&query(), &host);
        drop(guard);
        assert_same_action(&query(), &host);

        let mut guard = install().expect("reinstall");
        arm(Step::Restore);
        guard.restore().expect_err("injected restore failure");
        drop(guard);
        assert_same_action(&query(), &host);
        assert!(install().is_ok(), "drop retried the restore");
    });
}

#[test]
fn job_control_stop_self_default_failure_does_not_raise() {
    child_case(
        "job_control_stop_self_default_failure_does_not_raise",
        || {
            host_with_state();
            let mut guard = install().expect("install");
            arm(Step::Default);
            guard.stop_self().expect_err("injected default failure");
            assert_library_action(&query());
            assert!(!take_request(), "signal was raised under an unknown action");
        },
    );
}

#[test]
fn job_control_stop_self_raise_failure_reinstates() {
    child_case("job_control_stop_self_raise_failure_reinstates", || {
        host_with_state();
        let mut guard = install().expect("install");
        arm(Step::Raise);
        let failed = guard.stop_self().expect_err("injected raise failure");
        assert!(failed.to_string().contains("Raise"), "original error kept");
        assert_library_action(&query());
        assert!(!take_request());
    });
}

#[test]
fn job_control_stop_self_reinstate_failure_keeps_host_for_drop() {
    child_case(
        "job_control_stop_self_reinstate_failure_keeps_host_for_drop",
        || {
            let host = host_with_state();
            let mut guard = install().expect("install");
            block_tstp();
            arm(Step::Reinstate);
            let failed = guard.stop_self().expect_err("injected reinstate failure");
            assert!(failed.to_string().contains("Reinstate"));
            assert_eq!(query().sa_sigaction, libc::SIG_DFL);

            drop(guard);
            assert_same_action(&query(), &host);
            unblock_tstp();
            assert_eq!(
                HOST_COUNT.load(Ordering::SeqCst),
                1,
                "the pending stop reaches the restored host action"
            );
        },
    );
}

#[test]
fn job_control_stop_self_reinstalls_owned_action() {
    child_case("job_control_stop_self_reinstalls_owned_action", || {
        let host = host_with_state();
        let mut guard = install().expect("install");
        block_tstp();
        guard.stop_self().expect("stop_self");
        assert_library_action(&query());
        drop(guard);
        assert_same_action(&query(), &host);
    });
}

#[test]
fn job_control_stop_self_stops_until_continued() {
    const NAME: &str = "job_control_stop_self_stops_until_continued";
    if is_child(NAME) {
        let host = host_with_state();
        let mut guard = install().expect("install");
        guard.stop_self().expect("stop_self");
        assert_library_action(&query());
        drop(guard);
        assert_same_action(&query(), &host);
        println!("{DONE}");
        return;
    }
    let child = child_command(NAME)
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn child");
    let pid = child.id() as libc::pid_t;
    let mut status = 0;
    let waited = unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) };
    let stopped = waited == pid && libc::WIFSTOPPED(status);
    unsafe { libc::kill(pid, libc::SIGCONT) };
    let output = child.wait_with_output().expect("child output");
    assert!(stopped, "child did not stop on SIGTSTP (status {status})");
    assert_child_passed(NAME, &output);
}
