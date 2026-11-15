//! TablePro connection form, Basic fields, before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t01_basic/` and each checkpoint's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, TABLEPRO, find_all, flash_settle_stepped as flash_settle, row_text, text,
    try_launch,
};

const HOST: &str = "prod-db-1.acme.io";
const HOST_PORT: &str = "prod-db-1.acme.io:5432";
const NAME: &str = "Production";
const DATABASE: &str = "acme_prod";
const USER: &str = "acme_ops";
const PORT: &str = "5432";
const REQUIRED: &str = "Required";
/// Drawn form of the port validator. The source string is
/// "Port must be 1–65535"; at 120×40 `reveal::fit` clips the port column to
/// this prefix plus U+2026. At 80×24 the same clip is `Port mus…`.
const PORT_ERROR: &str = "Port must…";
const NARROW_PORT_ERROR: &str = "Port mus…";
const STATUS: &str = "Fix the highlighted fields";
const SAVED: &str = "Connection saved";
const TITLE: &str = "Edit connection";
const BOUND_PORT: &str = "65535";
const BOUND_HOST: &str = "prod-db-1.acme.io:65535";
const ENGINE: &str = "PostgreSQL";
const ENV_LABEL: &str = "production";
const SAFE_DESC: &str = "deliberate acknowledgement";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t01_basic/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open() -> App {
    let mut launch = Launch::new("t01-basic", TABLEPRO);
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

fn line_after(screen: &TerminalSnapshot, label: &str) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let (index, line) = lines
        .iter()
        .enumerate()
        .find(|(_, line)| line.contains(label))
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    let byte = line.find(label).expect("label byte");
    let value = lines.get(index + 1).map(String::as_str).unwrap_or("");
    let start = value
        .char_indices()
        .find(|(index, _)| *index >= byte)
        .map(|(index, _)| index)
        .unwrap_or(value.len());
    value[start..].to_string()
}

fn name_value(screen: &TerminalSnapshot) -> String {
    let below = line_after(screen, "Name *");
    let head: String = below.chars().take(24).collect();
    head.split_whitespace()
        .map(|token| token.trim_start_matches(['▎', '│']))
        .find(|token| !token.is_empty())
        .unwrap_or("")
        .to_string()
}

fn port_value(screen: &TerminalSnapshot) -> String {
    let line = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(HOST))
        .unwrap_or_else(|| panic!("host line missing\n{}", text(screen)));
    let rest = &line[line.find(HOST).expect("host") + HOST.len()..];
    rest.split_whitespace()
        .map(|token| token.trim_start_matches(['▎', '│', '┃']))
        .find(|token| !token.is_empty() && *token != "Acme" && !token.starts_with('('))
        .unwrap_or("")
        .to_string()
}

/// Field error "Required", not the Database help "Required for PostgreSQL".
fn shows_required(screen: &TerminalSnapshot) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains(REQUIRED) && !line.contains("PostgreSQL")
    })
}

fn assert_required(screen: &TerminalSnapshot) {
    assert!(
        shows_required(screen),
        "name error {REQUIRED:?} missing\n{}",
        text(screen)
    );
}

fn list_row(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| point.x < 40)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "list row {label:?} got {hits:?}\n{}",
        text(&screen)
    );
    hits[0]
}

fn form_hit(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| point.x >= 40)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "form text {label:?} got {hits:?}\n{}",
        text(&screen)
    );
    hits[0]
}

fn click_cancel(app: &App) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, "Cancel")
        .into_iter()
        .filter(|point| row_text(&screen, u32::from(point.y)).contains("Save"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "Cancel on the form action row got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    flash_settle(app);
}

fn replace_focused(app: &App, value: &str) {
    app.key("ctrl+l");
    app.key("backspace");
    if !value.is_empty() {
        app.type_text(value);
    }
}

fn wait_name(app: &App, value: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("name {value:?}"), |screen| {
        name_value(screen) == value
    })
}

fn wait_port(app: &App, value: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("port {value:?}"), |screen| {
        port_value(screen) == value
    })
}

fn set_name(app: &App, value: &str) -> TerminalSnapshot {
    replace_focused(app, value);
    wait_name(app, value)
}

fn set_port(app: &App, value: &str) -> TerminalSnapshot {
    replace_focused(app, value);
    wait_port(app, value)
}

fn submit(app: &App) -> TerminalSnapshot {
    app.key("ctrl+s");
    app.settle()
}

/// Resize, then wait for the app to draw at the new size. The emulator
/// reports the new size at once but still holds the old frame, so the header
/// readout of the size is the sign that the new frame has landed.
fn wait_size(app: &App, cols: u16, rows: u16) -> TerminalSnapshot {
    app.resize(cols, rows);
    let readout = format!("{cols}×{rows}");
    app.wait_screen(&format!("size {cols}x{rows}"), |screen| {
        let visible = text(screen);
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && visible.contains(&readout)
            && (visible.contains("TablePro")
                || visible.contains(TITLE)
                || visible.contains("Filter connections"))
    })
}

fn has_token(screen: &TerminalSnapshot, token: &str) -> bool {
    text(screen).split_whitespace().any(|word| word == token)
}

fn open_edit(app: &App) {
    app.click(list_row(app, NAME));
    flash_settle(app);
    app.wait_text(HOST);
    app.key("e");
    app.wait_text(TITLE);
}

fn assert_stored_form(screen: &TerminalSnapshot, status_clear: bool) {
    see(
        screen,
        &[
            TITLE, "Basic", "Name", "Host", "Port", "Database", "Username", NAME, HOST, PORT,
            DATABASE, USER,
        ],
        &[SAVED, PORT_ERROR],
    );
    if status_clear {
        assert!(
            !text(screen).contains(STATUS),
            "fresh form kept {STATUS:?}\n{}",
            text(screen)
        );
    }
    assert_eq!(name_value(screen), NAME, "name field\n{}", text(screen));
    assert_eq!(port_value(screen), PORT, "port field\n{}", text(screen));
    assert!(
        !shows_required(screen),
        "stored form showed a name error\n{}",
        text(screen)
    );
}

fn assert_rejected_stays(screen: &TerminalSnapshot) {
    see(screen, &[TITLE, STATUS, NAME], &[SAVED]);
    assert!(
        text(screen).contains(NAME),
        "the tree dropped {NAME}\n{}",
        text(screen)
    );
}

fn assert_original_detail(screen: &TerminalSnapshot) {
    see(
        screen,
        &[NAME, HOST_PORT, "Edit", "Filter connections"],
        &[TITLE, SAVED, PORT_ERROR, "65536"],
    );
    assert!(
        !text(screen).contains("abc"),
        "rejected port abc stayed on the detail\n{}",
        text(screen)
    );
    assert!(
        text(screen).contains(STATUS),
        "cancel cleared {STATUS:?}\n{}",
        text(screen)
    );
}

#[test]
fn basic_fields_reject_blank_names_and_invalid_ports() {
    let app = open();
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", NAME, "Filter connections", "Edit"],
        &[TITLE, SAVED],
    );

    open_edit(&app);
    let form = app.settle();
    assert_stored_form(&form, true);

    set_name(&app, "");
    let blank = submit(&app);
    assert_required(&blank);
    assert_eq!(name_value(&blank), "", "blank name\n{}", text(&blank));
    assert_eq!(
        port_value(&blank),
        PORT,
        "blank name changed the port\n{}",
        text(&blank)
    );
    assert!(
        !text(&blank).contains(PORT_ERROR),
        "a valid port showed {PORT_ERROR:?}\n{}",
        text(&blank)
    );
    assert_rejected_stays(&blank);

    let marked = set_name(&app, "x");
    let mark_col = marked.cursor().col();
    replace_focused(&app, " ");
    let spaced = app.wait_screen("whitespace name", |screen| {
        name_value(screen).is_empty() && screen.cursor().col() == mark_col
    });
    assert_eq!(
        spaced.cursor().col(),
        mark_col,
        "a space did not keep the caret of a one-character name\n{}",
        text(&spaced)
    );
    let spaces = submit(&app);
    assert_required(&spaces);
    assert_eq!(
        name_value(&spaces),
        "",
        "whitespace name\n{}",
        text(&spaces)
    );
    assert_eq!(port_value(&spaces), PORT);
    assert!(!text(&spaces).contains(PORT_ERROR));
    assert_rejected_stays(&spaces);

    set_name(&app, NAME);
    app.click(form_hit(&app, PORT));
    flash_settle(&app);
    set_port(&app, "65536");
    let high = submit(&app);
    assert_eq!(port_value(&high), "65536", "high port\n{}", text(&high));
    assert_eq!(name_value(&high), NAME);
    see(&high, &[TITLE, STATUS, PORT_ERROR], &[SAVED]);
    assert!(
        !shows_required(&high),
        "a non-blank name showed {REQUIRED:?}\n{}",
        text(&high)
    );

    set_port(&app, "abc");
    let letters = submit(&app);
    assert_eq!(
        port_value(&letters),
        "abc",
        "letter port\n{}",
        text(&letters)
    );
    see(
        &letters,
        &[TITLE, STATUS, PORT_ERROR, NAME],
        &[SAVED, "65536"],
    );

    set_port(&app, "0");
    let zero = submit(&app);
    assert_eq!(port_value(&zero), "0", "zero port\n{}", text(&zero));
    see(
        &zero,
        &[TITLE, STATUS, PORT_ERROR, NAME],
        &[SAVED, "abc", "65536"],
    );
    assert!(!shows_required(&zero));

    app.click(form_hit(&app, NAME));
    flash_settle(&app);
    set_name(&app, "");
    let failed = submit(&app);
    assert_required(&failed);
    assert_eq!(name_value(&failed), "");
    assert_eq!(
        port_value(&failed),
        "0",
        "blank name cleared the bad port\n{}",
        text(&failed)
    );
    see(
        &failed,
        &[TITLE, STATUS, PORT_ERROR, "Basic", HOST, DATABASE, USER],
        &[SAVED, "65536", "abc"],
    );
    assert_rejected_stays(&failed);
    assert_eq!((failed.cols(), failed.rows()), (120, 40));
    app.visual(&visual_id("T01.basic.fields.120x40"));

    wait_size(&app, 80, 24);
    // The resized frame is drawn in more than one pass, and the status line
    // comes last. Wait for it along with the clipped port error.
    let fields_narrow = app.wait_screen("narrow field errors", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(STATUS)
            && visible.contains(NARROW_PORT_ERROR)
    });
    assert_eq!((fields_narrow.cols(), fields_narrow.rows()), (80, 24));
    assert_required(&fields_narrow);
    assert_eq!(
        name_value(&fields_narrow),
        "",
        "narrow blank name\n{}",
        text(&fields_narrow)
    );
    assert_eq!(
        port_value(&fields_narrow),
        "0",
        "narrow port\n{}",
        text(&fields_narrow)
    );
    see(
        &fields_narrow,
        &[TITLE, STATUS, NARROW_PORT_ERROR, HOST],
        &[SAVED, "65536", "abc"],
    );
    app.visual(&visual_id("T01.basic.fields.narrow"));
    let fields_back = wait_size(&app, 120, 40);
    assert_eq!((fields_back.cols(), fields_back.rows()), (120, 40));

    click_cancel(&app);
    let closed = app.wait_absent(TITLE);
    assert_original_detail(&closed);

    app.key("e");
    let reopened = app.wait_screen("reopened form keeps status", |screen| {
        let visible = text(screen);
        visible.contains(TITLE) && visible.contains(STATUS)
    });
    assert_stored_form(&reopened, false);
    assert!(
        text(&reopened).contains(STATUS),
        "reopen cleared {STATUS:?}\n{}",
        text(&reopened)
    );
    assert_eq!((reopened.cols(), reopened.rows()), (120, 40));
    app.visual(&visual_id("T01.basic.reopen.120x40"));

    let reopen_narrow = wait_size(&app, 80, 24);
    assert_eq!((reopen_narrow.cols(), reopen_narrow.rows()), (80, 24));
    see(&reopen_narrow, &[TITLE, NAME, HOST, PORT], &[SAVED]);
    assert_eq!(
        name_value(&reopen_narrow),
        NAME,
        "narrow name\n{}",
        text(&reopen_narrow)
    );
    assert_eq!(
        port_value(&reopen_narrow),
        PORT,
        "narrow port\n{}",
        text(&reopen_narrow)
    );
    app.visual(&visual_id("T01.basic.reopen.narrow"));
    let reopen_back = wait_size(&app, 120, 40);
    assert_eq!((reopen_back.cols(), reopen_back.rows()), (120, 40));

    app.click(form_hit(&app, PORT));
    flash_settle(&app);
    set_port(&app, BOUND_PORT);
    let saved = submit(&app);
    see(
        &saved,
        &[
            SAVED,
            NAME,
            BOUND_HOST,
            ENGINE,
            ENV_LABEL,
            "Safe Mode",
            SAFE_DESC,
            DATABASE,
            USER,
        ],
        &[TITLE, "65536"],
    );
    assert!(
        (0..saved.rows()).any(|row| {
            let line = row_text(&saved, row);
            line.contains("Environment") && line.contains(ENV_LABEL)
        }),
        "environment label\n{}",
        text(&saved)
    );
    assert!(
        (0..saved.rows()).any(|row| {
            let line = row_text(&saved, row);
            line.contains("Engine") && line.contains(ENGINE)
        }),
        "engine label\n{}",
        text(&saved)
    );
    assert!(
        !text(&saved).contains("abc"),
        "rejected port abc\n{}",
        text(&saved)
    );
    assert!(!has_token(&saved, "0"), "rejected port 0\n{}", text(&saved));
    assert!(!has_token(&saved, "65536"));
    assert!(!has_token(&saved, "abc"));

    app.key("e");
    let again = app.wait_text(TITLE);
    assert_eq!(name_value(&again), NAME, "saved name\n{}", text(&again));
    assert_eq!(
        port_value(&again),
        BOUND_PORT,
        "saved port\n{}",
        text(&again)
    );
    click_cancel(&app);
    let back = app.wait_absent(TITLE);
    see(&back, &[NAME, BOUND_HOST, ENGINE], &[TITLE, "65536"]);
    app.finish();
}
