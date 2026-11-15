use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use junie_tui::runtime::TerminalSession;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};
use ratatui::widgets::Paragraph;

use super::bins::SHOWCASE;
use super::support::{App, Launch, Point, find, text, try_launch};

#[test]
#[ignore = "Tracked in docs/audits/2026-11-06-resize-fence-quarantine.md"]
fn showcase_help_survives_shrink_and_growth_and_returns_focus() {
    let app = App::new(
        "showcase-lifecycle",
        SHOWCASE,
        &["--page", "overview", "--motion", "paused", "--frame", "40"],
        120,
        40,
        "truecolor",
    );
    let screen = app.wait_text("120×40");
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    app.key("f1");
    let open = app.wait_text("Keyboard & mouse");
    let original_title = find(&open, "Keyboard & mouse");
    app.resize(80, 24);
    let narrow = app.wait_screen("help reflows at 80×24", |s| {
        s.cols() == 80
            && s.rows() == 24
            && text(s).contains("80×24")
            && text(s).contains("Keyboard & mouse")
    });
    assert_ne!(find(&narrow, "Keyboard & mouse"), original_title);
    app.visual("functional/showcase_help_80x24");
    app.resize(160, 50);
    app.wait_screen("help reflows at 160×50", |s| {
        s.cols() == 160
            && s.rows() == 50
            && text(s).contains("160×50")
            && text(s).contains("Keyboard & mouse")
    });
    app.key("escape");
    app.wait_absent("Keyboard & mouse");
    app.key("ctrl+pagedown");
    app.wait_text("App shell › Chrome");
    app.key("ctrl+pageup");
    app.wait_text("Tokens and principles behind every component");
    assert!(!text(&app.screen()).contains("App shell › Chrome"));
    app.visual("functional/showcase_help_resize");
    app.finish();
}

#[test]
fn showcase_popover_survives_resize_and_escape_returns_focus() {
    let app = App::new(
        "showcase-popover-resize",
        SHOWCASE,
        &["--page", "popups", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Open popover ▾");
    let trigger = Point {
        x: find(&app.screen(), "Open popover ▾").x + 2,
        y: find(&app.screen(), "Open popover ▾").y,
    };
    app.click(trigger);

    let open = app.wait_text("report.pdf");
    assert!(text(&open).contains("Open popover"));
    app.resize(80, 24);
    let narrow = app.wait_screen("popover still owned at 80x24", |s| {
        s.cols() == 80 && s.rows() == 24 && text(s).contains("Open popover")
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));

    let tab = find(&app.screen(), "Playground");
    app.wait_screen("selected tab neutral under popover", |s| {
        s.cell_at(u32::from(tab.x), u32::from(tab.y) + 1)
            .map(|c| c.fg())
            == Some(tui_test::SnapshotColor::Rgb(77, 77, 77))
    });

    assert!(
        app.try_sustained(std::time::Duration::from_millis(600), |s| {
            s.cell_at(u32::from(tab.x), u32::from(tab.y) + 1)
                .map(|c| c.fg())
                == Some(tui_test::SnapshotColor::Rgb(77, 77, 77))
        }),
        "selected tab must go neutral under the popover"
    );
    app.move_to(super::support::Point { x: 79, y: 0 });
    let trigger = find(&app.screen(), "Open popover \u{25be}");
    app.wait_screen("trigger unhovered under popover", |s| {
        super::support::cell_bg(s, trigger) == tui_test::SnapshotColor::Rgb(39, 39, 42)
    });
    app.visual("functional/showcase_popover_resize");
    app.key("escape");
    app.wait_screen("escape dismisses the popover after resize", |s| {
        !text(s).contains("report.pdf") && text(s).contains("Open popover ▾")
    });
    app.finish();
}

const SESSION_CHILD: &str = "JUNIE_TUI_LIFECYCLE_CHILD";
static HOST_PANICS: AtomicUsize = AtomicUsize::new(0);

fn await_enter() {
    loop {
        if let Event::Key(key) = event::read().expect("read key")
            && key.code == KeyCode::Enter
        {
            return;
        }
    }
}

fn show(session: &mut TerminalSession, line: &str) {
    session
        .terminal()
        .draw(|frame| frame.render_widget(Paragraph::new(line.to_string()), frame.area()))
        .expect("draw");
}

fn say(line: &str) {
    print!("\r\n{line}\r\n");
    std::io::stdout().flush().expect("flush");
}

fn raw() -> bool {
    is_raw_mode_enabled().expect("raw mode state")
}

fn caught_panic(message: &'static str) {
    assert!(std::panic::catch_unwind(|| panic!("{message}")).is_err());
}

/// Runs in the child process only. The child is this test binary, started
/// through the tuiscotti PTY, so the panic hook it replaces is its own.
fn session_child(mode: &str) {
    std::panic::set_hook(Box::new(|_| {
        HOST_PANICS.fetch_add(1, Ordering::SeqCst);
    }));
    let mut session = TerminalSession::enter().expect("enter session");
    match mode {
        "panic" => {
            show(&mut session, "session active, press Enter to panic");
            await_enter();
            caught_panic("active session panic");
            say(&format!(
                "active panic: host={} raw={}",
                HOST_PANICS.load(Ordering::SeqCst),
                raw()
            ));
            await_enter();
            drop(session);
            enable_raw_mode().expect("host raw mode");
            caught_panic("unrelated panic after the session");
            say(&format!(
                "after session: host={} raw={}",
                HOST_PANICS.load(Ordering::SeqCst),
                raw()
            ));
            await_enter();
            disable_raw_mode().expect("host raw mode off");
        }
        _ => {
            show(&mut session, "session active, press Enter to nest");
            await_enter();
            let refused = TerminalSession::enter()
                .map(|_| ())
                .expect_err("nested entry");
            show(
                &mut session,
                &format!("nested refused kind={:?} raw={}", refused.kind(), raw()),
            );
            await_enter();
        }
    }
}

fn launch_session_child(name: &str, test: &str, mode: &str) -> App {
    let exe = std::env::current_exe().expect("functional test binary");
    let mut launch = Launch::new(name, exe.to_str().expect("UTF-8 test binary path"));
    launch.args = [test, "--exact", "--nocapture", "--test-threads=1"]
        .into_iter()
        .map(str::to_string)
        .collect();
    launch.cols = 80;
    launch.rows = 24;
    launch.env = vec![(SESSION_CHILD.to_string(), mode.to_string())];
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn await_exit(app: &App) {
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

#[test]
fn active_panic_restores_terminal_once() {
    if let Ok(mode) = std::env::var(SESSION_CHILD) {
        session_child(&mode);
        return;
    }
    let app = launch_session_child(
        "lifecycle-active-panic",
        "lifecycle::active_panic_restores_terminal_once",
        "panic",
    );
    app.wait_text("session active, press Enter to panic");
    app.key("enter");
    app.wait_text("active panic: host=1 raw=false");
    app.key("enter");
    app.wait_text("after session: host=2 raw=true");
    app.key("enter");
    await_exit(&app);
}

#[test]
fn nested_session_entry_leaves_the_live_terminal_untouched() {
    if let Ok(mode) = std::env::var(SESSION_CHILD) {
        session_child(&mode);
        return;
    }
    let app = launch_session_child(
        "lifecycle-nested-entry",
        "lifecycle::nested_session_entry_leaves_the_live_terminal_untouched",
        "nested",
    );
    app.wait_text("session active, press Enter to nest");
    app.key("enter");
    app.wait_text("nested refused kind=AlreadyExists raw=true");
    app.key("enter");
    await_exit(&app);
}
