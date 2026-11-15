//! TablePro write confirmation, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_confirm/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections. Development starts
//! in Silent, which runs an ordinary UPDATE with no dialog.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, TABLEPRO, cell_bg, find_all, flash_settle, row_text, text, try_launch,
};

const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const TITLE: &str = "Execute write query?";
const CANCELLED: &str = "Cancelled · nothing was executed";
const ALERT_FACT: &str = "Alert · confirm before running";
const SAFE_FACT: &str = "Safe Mode · type to confirm";
const PICKER: &str = "Safe Mode · this connection";
const ALERT_STATUS: &str = "Safety level set to Alert · saved to the connection";
const SAFE_STATUS: &str = "Safety level set to Safe Mode · saved to the connection";
const HISTORY: &str = "Query history";
const NONE: &str = "No matching queries";
const MISMATCH: &str = "Does not match";
const RUNNING: &str = "Esc cancels";
const AFFECTED: &str = "8 rows affected";
const CANCEL_MARKER: &str = "CancelMarkerT03";
const EXACT_MARKER: &str = "ExactMarkerT03";
const FOCUS: &str = "▎";
const LOST: &str = "1 unsaved query will be lost.";

fn cancel_sql() -> String {
    format!("UPDATE organizations SET name = '{CANCEL_MARKER}' WHERE slug = 'northwind-labs'")
}

fn exact_sql() -> String {
    format!("UPDATE organizations SET name = '{EXACT_MARKER}' WHERE slug = 'northwind-labs'")
}

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_confirm/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
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

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(FOCUS))
        .unwrap_or_else(|| panic!("no focus bar\n{}", text(screen)))
}

fn rows_with(screen: &TerminalSnapshot, needle: &str) -> usize {
    (0..screen.rows())
        .filter(|row| row_text(screen, *row).contains(needle))
        .count()
}

fn confirm_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE) && visible.contains("Type ") && visible.contains("Execute")
}

fn footer_cancelled(screen: &TerminalSnapshot) -> bool {
    let last = screen.rows().saturating_sub(1);
    row_text(screen, last).contains(CANCELLED)
}

fn settled_confirm(app: &App, label: &str, cols: u32, rows: u32) -> TerminalSnapshot {
    app.wait_screen(label, |screen| {
        screen.cols() == cols && screen.rows() == rows && confirm_body(screen)
    })
}

fn shot(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    // visual() keeps the first quiet frame. The cancel toast clears during
    // that wait, and the baseline still shows it.
    app.visual_while(&visual_id(checkpoint), |screen| {
        screen.cols() == cols
            && screen.rows() == rows
            && confirm_body(screen)
            && footer_cancelled(screen)
    });
}

fn connect_development(app: &App) {
    for _ in 0..8 {
        let line = focused_line(&app.screen());
        if line.contains("Development") && line.contains("pg") {
            break;
        }
        let before = line;
        app.key("down");
        app.wait_screen("connection cursor moved", |screen| {
            focused_line(screen) != before
        });
    }
    let line = focused_line(&app.screen());
    assert!(
        line.contains("Development") && line.contains("pg"),
        "Enter would not connect Development\n{line}\n{}",
        text(&app.screen())
    );
    app.key("enter");
    // Query 1 is drawn during "Loading schema…", before the connected status.
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, QUERY, "acme_prod › public"],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Loading schema",
        ],
    );
}

fn release_editor(app: &App) {
    if text(&app.screen()).contains("Ctrl+Space") {
        app.key("esc");
        app.wait_screen("editor stopped editing", |screen| {
            !text(screen).contains("Ctrl+Space")
        });
    }
}

/// Each level's description is unique. The picker cursor is a fill, and `›`
/// marks the saved level, so the highlighted row is the one whose fill differs.
const LEVELS: &[(&str, &str)] = &[
    ("Silent", "without asking"),
    ("Alert", "Every write asks"),
    ("Alert (Full)", "reads included"),
    ("Safe Mode", "Writes ask for confirmation and a deliberate"),
    ("Safe Mode (Full)", "Every statement asks"),
    ("Read-Only", "Writes are refused"),
];

fn picker_choice(screen: &TerminalSnapshot) -> Option<String> {
    let mut rows = Vec::new();
    for (label, mark) in LEVELS {
        let hits = find_all(screen, mark);
        if hits.len() == 1 {
            rows.push((*label, hits[0]));
        }
    }
    if rows.len() != LEVELS.len() {
        return None;
    }
    let colors: Vec<_> = rows
        .iter()
        .map(|(_, point)| cell_bg(screen, *point))
        .collect();
    let rest = colors
        .iter()
        .find(|color| colors.iter().filter(|other| other == color).count() >= 2)?;
    let mut hits: Vec<_> = rows
        .iter()
        .zip(&colors)
        .filter(|(_, color)| *color != rest)
        .map(|(row, _)| row.0)
        .collect();
    (hits.len() == 1).then(|| hits.pop().unwrap().to_string())
}

fn set_level(app: &App, label: &str, status: &str) {
    release_editor(app);
    app.key("ctrl+l");
    app.wait_text(PICKER);
    for _ in 0..6 {
        let Some(current) = picker_choice(&app.screen()) else {
            break;
        };
        if current == label {
            break;
        }
        app.key("down");
        app.wait_screen("safe mode cursor moved", |screen| {
            text(screen).contains(PICKER)
                && picker_choice(screen).as_deref() != Some(current.as_str())
        });
    }
    let current = picker_choice(&app.screen());
    assert_eq!(
        current.as_deref(),
        Some(label),
        "picker is not on {label}\n{}",
        text(&app.screen())
    );
    app.key("enter");
    let set = app.wait_screen("safety level saved", |screen| {
        let visible = text(screen);
        visible.contains(status) && !visible.contains(PICKER)
    });
    see(&set, &[status], &["(Full)", PICKER]);
}

fn hide_explorer(app: &App) {
    if text(&app.screen()).contains("Filter objects") {
        app.key("ctrl+b");
        app.wait_screen("explorer hidden", |screen| {
            !text(screen).contains("Filter objects")
        });
    }
}

fn ensure_editing(app: &App) {
    if !text(&app.screen()).contains("Ctrl+Space") {
        app.key("enter");
        app.expire_status();
        app.wait_text("Ctrl+Space");
    }
}

fn write_sql(app: &App, sql: &str, marker: &str, absent: &str) {
    hide_explorer(app);
    ensure_editing(app);
    app.key("ctrl+l");
    app.type_text(sql);
    let written = app.wait_text(marker);
    see(
        &written,
        &[marker, "UPDATE organizations"],
        &[absent, TITLE],
    );
}

fn open_history(app: &App) {
    release_editor(app);
    app.key("ctrl+y");
    let history = app.wait_text(HISTORY);
    see(&history, &[HISTORY, "Search history"], &[TITLE]);
    app.key("c");
    app.wait_text("all connections");
}

fn close_history(app: &App) {
    // Esc on an Immediate field clears the text and keeps editing, so Ctrl+W
    // would be eaten. Tab commits the search and leaves the field.
    app.key("tab");
    app.key("ctrl+w");
    app.wait_screen("history closed", |screen| {
        let visible = text(screen);
        visible.contains(QUERY) && !visible.contains(HISTORY)
    });
}

fn assert_marker_absent(app: &App, marker: &str) {
    open_history(app);
    app.key("/");
    app.type_text(marker);
    let empty = app.wait_screen("history has no marker", |screen| {
        let visible = text(screen);
        visible.contains(NONE) && visible.contains(marker)
    });
    assert_eq!(
        rows_with(&empty, "UPDATE organizations"),
        0,
        "history listed {marker}\n{}",
        text(&empty)
    );
    close_history(app);
}

fn assert_marker_once(app: &App, marker: &str) {
    open_history(app);
    app.key("/");
    app.type_text(marker);
    let found = app.wait_screen("history has the marker", |screen| {
        let visible = text(screen);
        visible.contains(marker) && visible.contains("just now") && !visible.contains(NONE)
    });
    // List meta is "{when} · {duration}". The detail header is "{source} · {when}".
    let hits = rows_with(&found, "just now ·");
    assert_eq!(
        hits,
        1,
        "expected one history row for {marker}, saw {hits}\n{}",
        text(&found)
    );
    close_history(app);
}

fn confirm_token(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let flat = visible.split_whitespace().collect::<Vec<_>>().join(" ");
    let head = "Type ";
    let start = flat
        .find(head)
        .unwrap_or_else(|| panic!("no type-to-confirm label\n{visible}"));
    let rest = &flat[start + head.len()..];
    let end = rest
        .find(" to confirm")
        .unwrap_or_else(|| panic!("type label did not close\n{flat}"));
    let token = rest[..end].to_string();
    assert!(
        !token.is_empty() && !token.contains(' '),
        "token {token:?} is not the drawn label\n{visible}"
    );
    assert!(
        flat.contains(&format!("Type {token} to confirm")),
        "refusing a token the label does not show\n{visible}"
    );
    token
}

fn assert_unarmed(screen: &TerminalSnapshot, token: &str) {
    see(
        screen,
        &[
            TITLE,
            &format!("Type {token} to confirm"),
            SAFE_FACT,
            EXACT_MARKER,
            "Cancel",
            "Execute",
        ],
        &[RUNNING, AFFECTED, MISMATCH, CANCEL_MARKER],
    );
}

fn clear_ack(app: &App) {
    app.key("ctrl+l");
    app.key("backspace");
    for _ in 0..8 {
        app.key("backspace");
    }
}

fn type_rejected(app: &App, value: &str) {
    clear_ack(app);
    app.type_text(value);
    app.wait_text(value);
    app.key("enter");
    let still = app.wait_text(MISMATCH);
    see(
        &still,
        &[TITLE, MISMATCH, value, EXACT_MARKER],
        &[RUNNING, AFFECTED, "rows affected"],
    );
}

fn run_statement(app: &App) {
    app.key("ctrl+r");
}

fn quit_dirty(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &[LOST, "Quit"], &[TITLE]);
    app.confirm_quit();
}

#[test]
fn tablepro_confirm_route_returns_to_query() {
    let app = open("t03-confirm-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Filter connections", "Development"],
        &[QUERY, TITLE, CONNECTED],
    );
    connect_development(&app);
    app.key("ctrl+y");
    let history = app.wait_text(HISTORY);
    see(
        &history,
        &[HISTORY, QUERY],
        &[TITLE, "Loading schema", CANCELLED],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("back on query 1", |screen| {
        let visible = text(screen);
        visible.contains("Type SQL") && visible.contains(QUERY) && !visible.contains(HISTORY)
    });
    see(
        &back,
        &[QUERY, "Type SQL", CONNECTED],
        &[
            TITLE,
            HISTORY,
            "Loading schema",
            CANCEL_MARKER,
            EXACT_MARKER,
        ],
    );
    app.finish();
}

#[test]
fn tablepro_confirm_gates_accept_only_the_exact_phrase_once() {
    let app = open("t03-confirm-gates");
    connect_development(&app);
    set_level(&app, "Alert", ALERT_STATUS);

    write_sql(&app, &cancel_sql(), CANCEL_MARKER, EXACT_MARKER);
    run_statement(&app);
    let ordinary = app.wait_text(TITLE);
    see(
        &ordinary,
        &[TITLE, ALERT_FACT, CANCEL_MARKER, "Cancel", "Execute"],
        &[
            "Type ",
            SAFE_FACT,
            RUNNING,
            AFFECTED,
            CANCELLED,
            EXACT_MARKER,
        ],
    );
    assert!(
        text(&ordinary).contains(&format!("{FOCUS}Execute")),
        "Execute is not the focused arm\n{}",
        text(&ordinary)
    );
    app.key("esc");
    let cancelled = app.wait_screen("ordinary confirm cancelled", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && !visible.contains(TITLE)
    });
    see(
        &cancelled,
        &[CANCELLED, CANCEL_MARKER],
        &[TITLE, AFFECTED, RUNNING, EXACT_MARKER],
    );
    assert_marker_absent(&app, CANCEL_MARKER);

    set_level(&app, "Safe Mode", SAFE_STATUS);
    write_sql(&app, &exact_sql(), EXACT_MARKER, CANCEL_MARKER);
    run_statement(&app);
    let typed = app.wait_text("Type ");
    let token = confirm_token(&typed);
    assert_unarmed(&typed, &token);
    app.key("esc");
    let typed_cancelled = app.wait_screen("typed confirm cancelled", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && !visible.contains(TITLE)
    });
    see(
        &typed_cancelled,
        &[CANCELLED, EXACT_MARKER],
        &[
            TITLE,
            AFFECTED,
            RUNNING,
            &format!("Type {token} to confirm"),
        ],
    );
    // The cancel toast lasts 5s. History searches outlive it, so the gate is
    // captured before those searches.
    run_statement(&app);
    let again = app.wait_screen("typed confirm reopened", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && confirm_body(screen)
            && footer_cancelled(screen)
            && visible.contains(&format!("Type {token} to confirm"))
            && !visible.contains(MISMATCH)
    });
    assert_unarmed(&again, &token);
    flash_settle(&app);
    shot(&app, "T03.confirm.gates.120x40", 120, 40);

    app.resize(80, 24);
    app.wait_screen("typed confirm at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && confirm_body(screen)
    });
    flash_settle(&app);
    let narrow = settled_confirm(&app, "typed confirm settled at 80x24", 80, 24);
    see(
        &narrow,
        &["Type ", "Execute", CANCELLED],
        &[RUNNING, AFFECTED, MISMATCH],
    );
    shot(&app, "T03.confirm.gates.narrow", 80, 24);

    app.resize(120, 40);
    let restored = app.wait_screen("typed confirm back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && confirm_body(screen)
            && text(screen).contains(&format!("Type {token} to confirm"))
    });
    assert_unarmed(&restored, &token);

    app.key("esc");
    app.wait_screen("gate closed before history", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && !visible.contains(TITLE)
    });
    assert_marker_absent(&app, EXACT_MARKER);
    assert_marker_absent(&app, CANCEL_MARKER);

    run_statement(&app);
    let typing = app.wait_screen("typed confirm ready to type", |screen| {
        let visible = text(screen);
        visible.contains(TITLE)
            && visible.contains(&format!("Type {token} to confirm"))
            && !visible.contains(MISMATCH)
    });
    assert_unarmed(&typing, &token);

    type_rejected(&app, "nope");
    let partial: String = token.chars().take(token.chars().count() - 1).collect();
    assert!(
        token.starts_with(&partial) && partial != token && !partial.is_empty(),
        "partial {partial:?} is not a proper prefix of {token:?}"
    );
    type_rejected(&app, &partial);

    clear_ack(&app);
    app.type_text(&token);
    app.wait_text(&token);
    app.key("enter");
    let done = app.wait_screen("exact phrase ran once", |screen| {
        let visible = text(screen);
        visible.contains(AFFECTED)
            && !visible.contains(TITLE)
            && !visible.contains(RUNNING)
            && !visible.contains(MISMATCH)
    });
    see(
        &done,
        &[AFFECTED, EXACT_MARKER, "UPDATE organizations"],
        &[TITLE, RUNNING, CANCEL_MARKER, MISMATCH],
    );
    assert_eq!(
        rows_with(&done, AFFECTED),
        1,
        "the exact phrase produced more than one result\n{}",
        text(&done)
    );
    assert_marker_once(&app, EXACT_MARKER);

    run_statement(&app);
    let second = app.wait_text(TITLE);
    see(
        &second,
        &[TITLE, &format!("Type {token} to confirm")],
        &[RUNNING, MISMATCH],
    );
    app.key("esc");
    let held = app.wait_screen("second gate cancelled", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && !visible.contains(TITLE) && visible.contains(AFFECTED)
    });
    see(&held, &[AFFECTED, CANCELLED], &[TITLE, RUNNING]);
    assert_marker_once(&app, EXACT_MARKER);
    quit_dirty(&app);
}
