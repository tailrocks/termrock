//! Cross-application suspend, before the refactor.
//!
//! Suspend is `job_control` in `src/runtime.rs`: the SIGTSTP handler only
//! sets a flag, and the event loop calls `TerminalSession::suspend`.
//! `tests/functional/support.rs` keeps `App.session` private. The only public
//! signal is `kill_child`, which sends `Operation::Signal` with name `KILL`.
//! `App::key("ctrl+z")` is `Operation::Key`. That key stays on Overview, and
//! a later `]` still changes the page, so the child was not stopped.
//! `Launch::new` leaves the clock off.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use junie_tui::runtime::{TerminalSession, job_control};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::Paragraph;
use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, SHOWCASE, text, try_launch};

const BLURB: &str = "Tokens and principles behind every component";
const CAPTION: &str = "the live widget";
const CHROME: &str = "App shell ›";
const BRAND: &str = "Brand lockup";

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = ["--page", "overview"]
        .into_iter()
        .map(str::to_string)
        .collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    assert!(visible.contains(needle), "missing {needle}\n{visible}");
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    assert!(!visible.contains(needle), "{needle} present\n{visible}");
}

fn overview(app: &App) -> TerminalSnapshot {
    app.wait_text(BLURB);
    let screen = app.settle();
    require(&screen, BLURB);
    require(&screen, CAPTION);
    require(&screen, "Playground");
    refuse(&screen, CHROME);
    assert_eq!((screen.cols(), screen.rows()), (120u32, 40u32));
    assert!(app.child_running(), "overview startup stopped showcase");
    screen
}

#[test]
fn x02_suspend_route() {
    let app = open("x02-suspend-route");
    overview(&app);
    app.finish();
}

#[test]
fn x02_suspend_ctrl_z_stays() {
    let app = open("x02-suspend-ctrl-z");
    overview(&app);
    app.key("ctrl+z");
    let stayed = app.try_sustained(Duration::from_millis(800), |screen| {
        let visible = text(screen);
        visible.contains(BLURB)
            && visible.contains(CAPTION)
            && !visible.contains(CHROME)
            && screen.cols() == 120u32
            && screen.rows() == 40u32
    });
    assert!(stayed, "ctrl+z left Overview\n{}", text(&app.screen()));
    assert!(app.child_running(), "ctrl+z stopped showcase");
    app.key("]");
    let chrome = app.wait_text(BRAND);
    require(&chrome, CHROME);
    refuse(&chrome, BLURB);
    assert!(
        app.child_running(),
        "the page key after ctrl+z did not reach a running process"
    );
    app.finish();
}

const HOST_HANDLER_CHILD: &str = "JUNIE_TUI_X02_HOST_HANDLER_CHILD";
const HOST_HANDLER_TEST: &str = "cases::x02_suspend::suspend_resume_restores_host_handler";

static HOST_SIGTSTP: AtomicUsize = AtomicUsize::new(0);

extern "C" fn host_sigtstp(_signal: libc::c_int) {
    HOST_SIGTSTP.fetch_add(1, Ordering::SeqCst);
}

fn sigtstp_action() -> libc::sigaction {
    let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::sigaction(libc::SIGTSTP, std::ptr::null(), &mut current) };
    assert_eq!(rc, 0, "query SIGTSTP");
    current
}

fn raise_sigtstp() {
    assert_eq!(unsafe { libc::raise(libc::SIGTSTP) }, 0, "raise SIGTSTP");
}

fn show(session: &mut TerminalSession, line: &str) {
    session
        .terminal()
        .draw(|frame| frame.render_widget(Paragraph::new(line.to_string()), frame.area()))
        .expect("draw");
}

fn await_enter() {
    loop {
        if let Event::Key(key) = event::read().expect("read key")
            && key.code == KeyCode::Enter
        {
            return;
        }
    }
}

/// Runs in the child process only. The child is this test binary, started
/// through the tuiscotti PTY, so the SIGTSTP handler it installs, raises and
/// restores is the child's own and never the invoking runner's.
fn host_handler_child() {
    let mut host: libc::sigaction = unsafe { std::mem::zeroed() };
    host.sa_sigaction = host_sigtstp as *const () as libc::sighandler_t;
    host.sa_flags = libc::SA_RESTART;
    unsafe {
        libc::sigemptyset(&mut host.sa_mask);
        libc::sigaddset(&mut host.sa_mask, libc::SIGUSR1);
        assert_eq!(
            libc::sigaction(libc::SIGTSTP, &host, std::ptr::null_mut()),
            0
        );
    }
    let before = sigtstp_action();

    let mut session = TerminalSession::enter().expect("enter session");
    show(&mut session, "session active, press Enter to suspend");
    await_enter();

    raise_sigtstp();
    assert!(job_control::take_request(), "the session owns SIGTSTP");
    assert_eq!(
        HOST_SIGTSTP.load(Ordering::SeqCst),
        0,
        "host handler is shadowed"
    );
    session.suspend().expect("suspend and resume");
    show(&mut session, "repainted after resume, press Enter to leave");
    raise_sigtstp();
    assert!(
        job_control::take_request(),
        "the session reowns SIGTSTP after resume"
    );
    assert_eq!(
        HOST_SIGTSTP.load(Ordering::SeqCst),
        0,
        "host handler is still shadowed"
    );
    await_enter();

    session.try_leave().expect("leave session");
    let after = sigtstp_action();
    assert_eq!(after.sa_sigaction, before.sa_sigaction, "handler");
    assert_eq!(after.sa_flags, before.sa_flags, "flags");
    assert_eq!(
        unsafe { libc::sigismember(&after.sa_mask, libc::SIGUSR1) },
        1,
        "mask"
    );
    raise_sigtstp();
    println!(
        "\nhost handler restored, count={}",
        HOST_SIGTSTP.load(Ordering::SeqCst)
    );
    await_enter();
}

#[test]
fn suspend_resume_restores_host_handler() {
    if std::env::var_os(HOST_HANDLER_CHILD).is_some() {
        host_handler_child();
        return;
    }
    let exe = std::env::current_exe().expect("functional test binary");
    let mut launch = Launch::new(
        "x02-suspend-host-handler",
        exe.to_str().expect("UTF-8 test binary path"),
    );
    launch.args = [
        HOST_HANDLER_TEST,
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    launch.cols = 120;
    launch.rows = 40;
    launch.env = vec![(HOST_HANDLER_CHILD.to_string(), "1".to_string())];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));

    app.wait_text("session active, press Enter to suspend");
    app.key("enter");
    app.wait_text("repainted after resume, press Enter to leave");
    app.key("enter");
    app.wait_text("host handler restored, count=1");
    app.key("enter");
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "child did not exit\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    app.assert_restored_exit();
}
