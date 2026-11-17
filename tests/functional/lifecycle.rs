use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use junie_tui::runtime::TerminalSession;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};
use ratatui::widgets::Paragraph;

use super::bins::showcase;
use super::support::{App, Launch, Point, find, text, try_launch};

#[test]
#[ignore = "Tracked in docs/audits/2026-11-06-resize-fence-quarantine.md"]
fn showcase_help_survives_shrink_and_growth_and_returns_focus() {
    let app = App::new(
        "showcase-lifecycle",
        showcase(),
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
        showcase(),
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
    app.wait_exit("child did not exit");
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

#[test]
fn a_session_gives_the_terminal_back_and_takes_it_again_through_its_own_calls() {
    if std::env::var(SESSION_CHILD).as_deref() == Ok("release") {
        let mut session = TerminalSession::enter().expect("enter session");
        session
            .terminal()
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new("session active, press Enter to release"),
                    frame.area(),
                )
            })
            .expect("draw through the session's terminal");
        await_enter();
        session.try_leave().expect("try_leave");
        say(&format!("released: raw={}", raw()));
        // The host owns the terminal now; a second leave must not undo that.
        enable_raw_mode().expect("host raw mode");
        session.leave();
        say(&format!("left twice: raw={}", raw()));
        await_enter();
        let (cols, rows) = session.suspend().expect("suspend and resume");
        session
            .terminal()
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new(format!("resumed: size={cols}x{rows} raw={}", raw())),
                    frame.area(),
                )
            })
            .expect("draw after the resume");
        await_enter();
        return;
    }
    let app = launch_session_child(
        "lifecycle-release-resume",
        "lifecycle::a_session_gives_the_terminal_back_and_takes_it_again_through_its_own_calls",
        "release",
    );
    app.wait_text("session active, press Enter to release");
    app.key("enter");
    app.wait_text("released: raw=false");
    app.wait_text("left twice: raw=true");
    app.key("enter");
    app.wait_text("resumed: size=80x24 raw=true");
    app.key("enter");
    await_exit(&app);
}

/// How long the event loop sleeps when nothing else wakes it, and so the
/// longest an external message can wait: the receiver's own latency.
struct LatencyProbe {
    start: std::time::Instant,
    receiver: junie_tui::runtime::ExternalReceiver,
    delivered_ms: Option<u128>,
}

impl junie_tui::runtime::Application for LatencyProbe {
    fn handle(&mut self, input: junie_tui::core::event::Input) -> junie_tui::core::event::Outcome {
        if matches!(input, junie_tui::core::event::Input::External(_)) {
            self.delivered_ms = Some(self.start.elapsed().as_millis());
        }
        junie_tui::core::event::Outcome::Changed
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        frame.render_widget(Paragraph::new("waiting for a message"), frame.area());
    }

    fn should_quit(&self) -> bool {
        self.delivered_ms.is_some()
    }

    // Far longer than any latency under test, so only the latency wakes the loop.
    fn tick_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }

    fn external(&self) -> Option<&junie_tui::runtime::ExternalReceiver> {
        Some(&self.receiver)
    }
}

fn delivery_ms(latency_ms: &str) -> u128 {
    let app = launch_session_child(
        "lifecycle-external-latency",
        "lifecycle::an_external_message_waits_for_the_receivers_latency",
        &format!("latency:{latency_ms}"),
    );
    let screen = app.wait_text("delivered after");
    let shown = text(&screen);
    let ms = shown
        .split("delivered after ")
        .nth(1)
        .and_then(|rest| rest.split(" ms").next())
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| panic!("no delivery time on the screen:\n{shown}"));
    app.key("enter");
    await_exit(&app);
    ms
}

#[test]
fn an_external_message_waits_for_the_receivers_latency() {
    if let Some(ms) = std::env::var(SESSION_CHILD)
        .ok()
        .and_then(|mode| mode.strip_prefix("latency:").map(str::to_owned))
    {
        let (tx, receiver) = junie_tui::runtime::external_channel();
        let receiver = receiver.latency(std::time::Duration::from_millis(
            ms.parse().expect("latency in ms"),
        ));
        let mut app = LatencyProbe {
            start: std::time::Instant::now(),
            receiver,
            delivered_ms: None,
        };
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            tx.send(1_u8);
        });
        junie_tui::runtime::run(&mut app).expect("the event loop runs");
        say(&format!(
            "delivered after {} ms",
            app.delivered_ms.expect("a message arrived")
        ));
        // The session is gone, so the terminal is back in line mode.
        std::io::stdin()
            .read_line(&mut String::new())
            .expect("read the line");
        return;
    }
    // The message is sent 100 ms in. A long latency leaves it queued until the
    // next wake-up, so it cannot arrive before 1000 ms. Only lower bounds are
    // asserted: an upper bound on the short latency would fail on a loaded host.
    let quick = delivery_ms("10");
    let slow = delivery_ms("1500");
    assert!(
        quick >= 100,
        "a 10 ms latency delivered after {quick} ms, before the send"
    );
    assert!(slow >= 1_000, "a 1500 ms latency delivered after {slow} ms");
}
