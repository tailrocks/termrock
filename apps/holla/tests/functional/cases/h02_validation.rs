//! Holla argument validation, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h02_validation/` and each checkpoint's dots become underscores.
//! The launch does not pass `--frame`. Reduced motion finishes discovery before
//! the first frame, so the shots are not a spinner.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const ROW: &str = "Find the process on a port…";
/// The finder label column cuts this row to `Find the process on a po…`.
const ROW_VISIBLE: &str = "Find the process on a po";
const TITLE: &str = "Find the process on a port… · arguments";
const QUERY: &str = "lsof";
const LITERAL: &str = "5173;$(id)";
const DEFAULT_PREVIEW: &str = "lsof -nP '-iTCP:5173' -sTCP:LISTEN";
const LITERAL_PREVIEW: &str = "lsof -nP '-iTCP:5173;$(id)' -sTCP:LISTEN";
const EMPTY_PREVIEW: &str = "lsof -nP '-iTCP:<port>' -sTCP:LISTEN";
const ERROR: &str = "Fill the required fields first";
const WIDE: &str = "H02.validation.literal.120x40";
const NARROW: &str = "H02.validation.literal.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h02_validation/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn selected(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('›') else {
            return false;
        };
        let Some(at) = line.find(label) else {
            return false;
        };
        at > mark && at - mark <= 4
    })
}

fn on_form(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(ROW) && visible.contains("· arguments")
}

fn literal_held(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    on_form(screen)
        && visible.contains(LITERAL)
        && !visible.contains("51735173")
        && !visible.contains("uid=")
        && !visible.contains("rm:")
        && !visible.contains("discovering")
}

fn literal_open(screen: &TerminalSnapshot) -> bool {
    literal_held(screen)
        && text(screen).contains(LITERAL_PREVIEW)
        && !text(screen).contains(DEFAULT_PREVIEW)
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec!["--scenario".to_string(), "first-use".to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    let finder = app.wait_text("Search actions and resources");
    assert_eq!((finder.cols(), finder.rows()), (120, 40));
    see(
        &finder,
        &["Search actions and resources"],
        &[TITLE, "discovering"],
    );
    app
}

fn open_port(app: &App) {
    app.paste(QUERY);
    let row = app.wait_screen("port row selected", |screen| {
        selected(screen, ROW_VISIBLE) && text(screen).contains(QUERY)
    });
    assert_eq!((row.cols(), row.rows()), (120, 40));
    see(
        &row,
        &[ROW_VISIBLE, QUERY, "Results · 1", "port = 5173"],
        &[TITLE, "uid=", "rm:"],
    );
    app.key("enter");
    let page = app.wait_screen("arguments form", |screen| {
        on_form(screen) && text(screen).contains(DEFAULT_PREVIEW)
    });
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(
        &page,
        &[TITLE, DEFAULT_PREVIEW, "Command"],
        &["uid=", "rm:", "discovering", LITERAL],
    );
}

fn back_on_port(screen: &TerminalSnapshot) -> bool {
    selected(screen, ROW_VISIBLE) && !on_form(screen) && text(screen).contains(QUERY)
}

/// A same-size resize does not force a new frame, so the shot nudges the width
/// and comes back. The size chip is not a wait target: at 80 columns it can be
/// omitted, and the arguments form must stay open with the literal text.
fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && literal_held(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && literal_held(screen)
    });
}

#[test]
fn h02_validation_route_returns_to_the_finder() {
    let app = launch("h02-validation-route");
    open_port(&app);
    app.key("escape");
    let back = app.wait_screen("finder keeps the port row", back_on_port);
    assert_eq!((back.cols(), back.rows()), (120, 40));
    see(
        &back,
        &[ROW_VISIBLE, QUERY, "port = 5173"],
        &[TITLE, "uid=", "rm:", ERROR],
    );
    assert!(
        selected(&back, ROW_VISIBLE),
        "the port action lost selection\n{}",
        text(&back)
    );
    app.finish();
}

#[test]
fn h02_validation_literal_keeps_shell_metacharacters() {
    let app = launch("h02-validation-literal");
    open_port(&app);

    // Focus selects the default, so paste replaces `5173` instead of appending.
    app.paste(LITERAL);
    let typed = app.wait_screen("literal port text", literal_open);
    assert_eq!((typed.cols(), typed.rows()), (120, 40));
    see(
        &typed,
        &[TITLE, LITERAL, LITERAL_PREVIEW, "Command"],
        &[
            DEFAULT_PREVIEW,
            EMPTY_PREVIEW,
            "51735173",
            "uid=",
            "rm:",
            "discovering",
            ERROR,
        ],
    );
    assert_eq!(
        text(&typed).matches("lsof").count(),
        1,
        "a second command appeared\n{}",
        text(&typed)
    );

    nudge(&app);
    app.visual(&visual_id(WIDE));

    app.resize(80, 24);
    let narrowed = app.wait_screen("arguments at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && on_form(screen)
            && text(screen).contains(LITERAL)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(
        &narrowed,
        &[TITLE, LITERAL],
        &["uid=", "rm:", "discovering", ERROR],
    );
    nudge(&app);
    app.visual(&visual_id(NARROW));
    app.resize(120, 40);

    let restored = app.wait_screen("arguments back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && literal_open(screen)
    });
    see(
        &restored,
        &[TITLE, LITERAL, LITERAL_PREVIEW, "Command"],
        &["uid=", "rm:", ERROR],
    );

    app.key("ctrl+u");
    let cleared = app.wait_screen("required port cleared", |screen| {
        let visible = text(screen);
        on_form(screen) && visible.contains(EMPTY_PREVIEW) && !visible.contains(LITERAL)
    });
    see(
        &cleared,
        &[TITLE, EMPTY_PREVIEW, "TCP port"],
        &[LITERAL, LITERAL_PREVIEW, "uid=", "rm:", ERROR],
    );

    app.key("ctrl+s");
    let rejected = app.wait_screen("empty submit stays on the form", |screen| {
        let visible = text(screen);
        on_form(screen) && visible.contains(ERROR) && visible.contains(EMPTY_PREVIEW)
    });
    assert_eq!((rejected.cols(), rejected.rows()), (120, 40));
    see(
        &rejected,
        &[TITLE, ERROR, EMPTY_PREVIEW, "Required"],
        &[LITERAL, LITERAL_PREVIEW, "uid=", "rm:", "discovering"],
    );
    assert!(
        !text(&rejected).contains("Port "),
        "empty submit opened a port snapshot\n{}",
        text(&rejected)
    );

    // The field is dirty, so the first Esc cancels the edit and the next leaves.
    app.key("escape");
    let cancelled = app.wait_screen("validation edit cancelled", |screen| {
        let visible = text(screen);
        on_form(screen)
            && visible.contains(DEFAULT_PREVIEW)
            && !visible.contains(LITERAL)
            && !visible.contains(ERROR)
    });
    see(
        &cancelled,
        &[TITLE, DEFAULT_PREVIEW],
        &[LITERAL, ERROR, "uid=", "rm:"],
    );

    app.key("escape");
    let back = app.wait_screen("finder keeps the port row", back_on_port);
    assert_eq!((back.cols(), back.rows()), (120, 40));
    see(
        &back,
        &[ROW_VISIBLE, QUERY, "port = 5173"],
        &[TITLE, ERROR, LITERAL, "uid=", "rm:"],
    );
    assert!(
        selected(&back, ROW_VISIBLE),
        "the port action changed after the error\n{}",
        text(&back)
    );
    app.finish();
}
