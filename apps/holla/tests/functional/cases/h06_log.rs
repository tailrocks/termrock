//! Holla cleanup history and the operation log, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h06_log/` and each checkpoint's dots become underscores.
//! `parity-idea` sets `ops_log.write_failure` to `EROFS`. Catalog item
//! `idea.clean` is labeled `Clean IntelliJ metadata under ~/work/ide` and
//! launches `Launch::Cleanup { category: Some("idea.clean") }`. `disk.history`
//! is absent until a report exists. `d` opens gate 1. Gate 2's phrase is
//! `TRASH {n} UNDER /Users/alex/work/ide ON mbp`, read from the screen.
//! `OpsLog::append` drops the line when the write fails, so the history page
//! shows the log failure and no new JSONL row.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const IDEA: &str = "Clean IntelliJ metadata under ~/work/ide";
const HISTORY: &str = "Show cleanup history";
const TITLE: &str = "Cleanup history";
const CLEANUP: &str = "Cleanup · IntelliJ metadata";
const FAILURE: &str = "EROFS · deletions still ran · nothing rolled back";
const LOG_FACT: &str = "Log failure";
const NO_ROWS: &str = "0 · 0 operation log records";
const LOG_SECTION: &str = "operation log · newest last";
const MISMATCH: &str = "Does not match";
const STARTED: &str = "cleanup started";
const GATE1: &str = "gate 1 of 2";
const GATE2: &str = "gate 2 of 2";
const CONTINUE: &str = "Continue…";
const PHRASE_TAIL: &str = "UNDER /Users/alex/work/ide ON mbp";
const HOME: &str = "~/work/ide";
const QUIT: &str = "Quit holla";
const FINDER: &str = "Search actions";
const WIDE: &str = "H06.log.log.120x40";
const NARROW: &str = "H06.log.log.narrow";

const FOREIGN: &[&str] = &[
    "~/Projects/safe",
    "sparse.img",
    "build.gradle.kts",
    "842 of 926",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h06_log/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    launch.platform = "macos".to_string();
    launch.args = vec!["--scenario".to_string(), "parity-idea".to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(FINDER);
    app
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

fn collapsed(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join(" ")
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn phrase_on(screen: &TerminalSnapshot) -> String {
    let flat = collapsed(screen);
    let tail = flat
        .find(PHRASE_TAIL)
        .unwrap_or_else(|| panic!("phrase tail missing\n{flat}"));
    let start = flat[..tail]
        .rfind("TRASH ")
        .unwrap_or_else(|| panic!("phrase head missing\n{flat}"));
    let phrase = flat[start..tail + PHRASE_TAIL.len()]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut parts = phrase.split_whitespace();
    assert_eq!(parts.next(), Some("TRASH"), "phrase {phrase}");
    let count = parts.next().unwrap_or("");
    assert!(
        !count.is_empty() && count.chars().all(|c| c.is_ascii_digit()) && count != "0",
        "phrase count {phrase}"
    );
    assert_eq!(
        parts.collect::<Vec<_>>().join(" "),
        PHRASE_TAIL,
        "phrase {phrase}"
    );
    phrase
}

fn dated_row(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.windows(5).any(|window| {
        window[0].is_ascii_digit()
            && window[1].is_ascii_digit()
            && window[2].is_ascii_digit()
            && window[3].is_ascii_digit()
            && window[4] == b'-'
    })
}

/// A stored JSONL row is the only thing that can paint a history-table entry.
/// Failed and skipped outcomes free nothing. Trashed and removed are the only
/// outcomes that may show a size, and this fixture stored neither.
fn assert_no_invented_entry(screen: &TerminalSnapshot) {
    let visible = text(screen);
    let flat = collapsed(screen);
    assert!(flat.contains(FAILURE), "log failure text\n{visible}");
    assert!(visible.contains(NO_ROWS), "operation log count\n{visible}");
    assert!(
        !flat.contains(LOG_SECTION),
        "invented operation log section\n{visible}"
    );
    assert!(
        !visible.contains("\"outcome\""),
        "invented JSONL row\n{visible}"
    );
    for line in visible.lines() {
        if !dated_row(line) {
            continue;
        }
        let failed = line.contains("failed") || line.contains("skipped");
        let deleted = line.contains("trashed") || line.contains("removed");
        assert!(
            !(failed && !line.contains("0 B")),
            "failed or skipped row freed space\n{line}"
        );
        assert!(
            !(deleted && line.contains("0 B") && !line.contains("KiB") && !line.contains("MiB")),
            "deleted row hid its size\n{line}"
        );
        panic!("history row for an entry the log did not store\n{line}\n{visible}");
    }
    for line in visible.lines() {
        let report = line.contains("Report") || line.contains("log write failures");
        let outcome = line.contains("trashed") || line.contains("removed");
        assert!(
            !(outcome && !report),
            "log failure painted as a successful delete\n{line}\n{visible}"
        );
    }
}

fn history_page(screen: &TerminalSnapshot) -> bool {
    let flat = collapsed(screen);
    flat.contains(TITLE)
        && flat.contains(LOG_FACT)
        && flat.contains(FAILURE)
        && flat.contains(NO_ROWS)
        && flat.contains(HOME)
        && !flat.contains(LOG_SECTION)
        && !flat.contains(QUIT)
        && !flat.contains("\"outcome\"")
        && FOREIGN.iter().all(|item| !flat.contains(item))
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn open_cleanup(app: &App) -> TerminalSnapshot {
    app.paste(IDEA);
    app.wait_screen("idea cleanup row", |screen| {
        focused_line(screen).contains(IDEA)
    });
    app.key("enter");
    app.wait_screen("idea cleanup", |screen| {
        let visible = text(screen);
        visible.contains(CLEANUP)
            && visible.contains(HOME)
            && !visible.contains(GATE1)
            && !visible.contains(QUIT)
    })
}

fn confirm_cleanup(app: &App) {
    app.key("d");
    let gate1 = app.wait_screen("cleanup gate 1", |screen| {
        let visible = text(screen);
        visible.contains(GATE1)
            && visible.contains(CONTINUE)
            && focused_line(screen).contains("Cancel")
            && !visible.contains(GATE2)
    });
    see(
        &gate1,
        &[GATE1, CONTINUE, "Cancel"],
        &[GATE2, MISMATCH, STARTED, QUIT],
    );
    assert!(
        focused_line(&gate1).find('▎') < focused_line(&gate1).find("Cancel"),
        "Cancel is not the focused action\n{}",
        focused_line(&gate1)
    );

    app.key("right");
    app.wait_screen("continue focused", |screen| {
        focused_line(screen).contains(CONTINUE)
    });
    app.key("enter");
    let gate2 = app.wait_screen("cleanup gate 2", |screen| {
        let flat = collapsed(screen);
        flat.contains(GATE2) && flat.contains("TRASH ") && flat.contains(PHRASE_TAIL)
    });
    let phrase = phrase_on(&gate2);
    // Gate 1 stays under the dialog, so its Continue… button is still painted.
    see(
        &gate2,
        &[GATE2, &phrase, "Type "],
        &[MISMATCH, STARTED, QUIT],
    );
    assert!(
        collapsed(&gate2).contains(&format!("Type {phrase} to confirm")),
        "phrase label\n{}",
        text(&gate2)
    );

    app.type_text("TRASH 0");
    app.key("enter");
    let rejected = app.wait_screen("phrase rejected", |screen| {
        let visible = text(screen);
        visible.contains(MISMATCH) && visible.contains(GATE2) && visible.contains("TRASH 0")
    });
    see(
        &rejected,
        &[MISMATCH, GATE2, "TRASH 0", &phrase],
        &[STARTED, QUIT],
    );

    app.key("ctrl+u");
    app.wait_absent("TRASH 0");
    app.paste(&phrase);
    app.wait_text(&phrase);
    app.key("enter");
    app.wait_screen("cleanup started", |screen| {
        let visible = text(screen);
        visible.contains(STARTED) && !visible.contains(GATE2) && !visible.contains(MISMATCH)
    });
    app.wait_screen("log writes failed", |screen| {
        text(screen).contains("log write failures")
    });
}

fn open_history(app: &App) -> TerminalSnapshot {
    open_cleanup(app);
    confirm_cleanup(app);
    app.key("escape");
    app.wait_screen("finder after the report", |screen| {
        text(screen).contains(IDEA) && !text(screen).contains(TITLE)
    });
    app.key("escape");
    app.wait_text(FINDER);
    app.paste(HISTORY);
    app.wait_screen("history row", |screen| {
        focused_line(screen).contains(HISTORY)
    });
    app.key("enter");
    app.wait_screen("cleanup history", history_page)
}

fn assert_history(screen: &TerminalSnapshot) {
    see(
        screen,
        &[TITLE, LOG_FACT, FAILURE, NO_ROWS, HOME],
        &[LOG_SECTION, QUIT, GATE1, GATE2],
    );
    see(screen, &[], FOREIGN);
    assert_no_invented_entry(screen);
    let visible = text(screen);
    if visible.contains(".idea") || visible.contains("app.iml") {
        assert!(
            visible.contains(HOME),
            "idea path without its root\n{visible}"
        );
    }
}

#[test]
fn h06_log_route_returns_from_history() {
    let app = launch("h06-log-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[HOME, "mbp"], &[TITLE, QUIT, CLEANUP]);
    see(&landing, &[], FOREIGN);

    let page = open_history(&app);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    assert_history(&page);

    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY)
            && visible.contains(HOME)
            && !visible.contains(TITLE)
            && !visible.contains(LOG_FACT)
            && !visible.contains(QUIT)
    });
    see(
        &finder,
        &[HISTORY, HOME],
        &[TITLE, LOG_FACT, FAILURE, QUIT, CLEANUP],
    );
    app.finish();
}

#[test]
fn h06_log_failure_does_not_invent_an_entry() {
    let app = launch("h06-log-failure");
    let page = open_history(&app);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    assert_history(&page);

    nudge(&app, history_page);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrow = app.wait_screen("narrow history", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && history_page(screen)
    });
    assert_history(&narrow);
    nudge(&app, history_page);
    shot(&app, NARROW);

    app.resize(120, 40);
    let restored = app.wait_screen("history restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && history_page(screen)
    });
    assert_history(&restored);

    app.key("escape");
    let finder = app.wait_screen("left history", |screen| {
        text(screen).contains(HISTORY) && !text(screen).contains(TITLE)
    });
    see(&finder, &[HISTORY], &[TITLE, LOG_FACT, FAILURE]);

    app.key("escape");
    app.wait_text(FINDER);
    app.paste(HISTORY);
    app.wait_screen("history row again", |screen| {
        focused_line(screen).contains(HISTORY)
    });
    app.key("enter");
    let again = app.wait_screen("history reopened", history_page);
    assert_history(&again);
    assert_eq!((again.cols(), again.rows()), (120, 40));
    app.finish();
}
