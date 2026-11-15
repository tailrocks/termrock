//! TablePro commit flow before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_commit/` and each checkpoint's dots become underscores.
//!
//! `finish_commit` always applies `Ok(())`. This leaf does not invent a failure.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, TABLEPRO, find_all, row_text, text, try_launch};

const QUERY: &str = "Query 1";
const TABLE: &str = "organizations";
const PANEL: &str = "public › organizations";
const VALUE: &str = "AcmeNorth";
const PREVIEW: &str = "SQL preview";
const SAVE_TITLE: &str = "Save changes?";
const SCOPE: &str = "1 update · 0 inserts · 0 deletes";
const SAFE: &str = "runs after this review";
const KEPT: &str = "Changes kept pending";
const SAVED: &str = "Saved 1 change to public.organizations";
const HISTORY: &str = "Query history";
const UPDATE: &str = r#"UPDATE "public"."organizations""#;
const UPDATE_HEAD: &str = r#"UPDATE "public"."or"#;
const NAME_SET: &str = r#""name" = 'AcmeNorth'"#;

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t02_commit/{}",
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

/// The 140ms press flash stays painted while motion is paused, so a shot after
/// a released click has to redraw once the deadline has passed. Below about 100
/// columns the header omits the size chip, and the narrow nudge waits on the
/// terminal size instead.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    if cols >= 100 {
        app.wait_text(&format!("{nudged}×{rows}"));
        app.resize(cols, rows);
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow nudge", |screen| {
            screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
        });
        app.resize(cols, rows);
        app.wait_screen("narrow restored", |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
        });
    }
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
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

fn click_connect(app: &App) {
    let focused = focused_line(&app.screen());
    assert!(
        focused.contains("Development"),
        "Enter would not connect Development\n{focused}"
    );
    app.key("enter");
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_else(|| panic!("no focus bar\n{}", text(screen)))
}

fn connect_development(app: &App) {
    app.click(list_row(app, "Development"));
    app.wait_text("dev-db.internal.acme.io");
    click_connect(app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains("Connected to Development")
            && !visible.contains("Loading schema")
    });
}

fn open_organizations(app: &App) {
    app.wait_text(TABLE);
    for _ in 0..16 {
        let before = focused_line(&app.screen());
        if before.contains(TABLE) {
            app.key("enter");
            app.wait_text(PANEL);
            return;
        }
        app.key("down");
        app.wait_screen("explorer cursor moved", |screen| {
            focused_line(screen) != before
        });
    }
    panic!("explorer never focused {TABLE}\n{}", text(&app.screen()));
}

fn edit_name(app: &App) {
    app.key("right");
    app.key("enter");
    app.key("ctrl+l");
    app.type_text(VALUE);
    let editing = app.wait_text(VALUE);
    assert!(
        text(&editing).contains(VALUE),
        "cell editor did not show {VALUE}\n{}",
        text(&editing)
    );
    app.key("enter");
    let pending = app.wait_text("1 pending");
    see(&pending, &[VALUE, "1 pending"], &[SAVE_TITLE, PREVIEW]);
}

fn box_drawing(ch: char) -> bool {
    matches!(
        ch,
        '│' | '┃'
            | '║'
            | '╭'
            | '╮'
            | '╰'
            | '╯'
            | '┌'
            | '┐'
            | '└'
            | '┘'
            | '─'
            | '━'
            | '├'
            | '┤'
            | '┬'
            | '┴'
            | '┼'
    )
}

/// Visible `UPDATE` fragments, cut at the first ellipsis or rule.
fn update_fragments(screen: &TerminalSnapshot) -> Vec<String> {
    let mut found = Vec::new();
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        let mut rest = line.as_str();
        while let Some(start) = rest.find(UPDATE_HEAD) {
            let tail = &rest[start..];
            let mut end = tail.len();
            for (index, ch) in tail.char_indices().skip(UPDATE_HEAD.len()) {
                if ch == '…' || box_drawing(ch) {
                    end = index;
                    break;
                }
            }
            let fragment = tail[..end].trim().to_string();
            if !fragment.is_empty() {
                found.push(fragment);
            }
            rest = &tail[end.max(1)..];
        }
    }
    found
}

fn preview_statement(screen: &TerminalSnapshot) -> String {
    let fragments = update_fragments(screen);
    assert_eq!(
        fragments.len(),
        1,
        "preview UPDATE fragments {fragments:?}\n{}",
        text(screen)
    );
    let statement = &fragments[0];
    assert!(
        statement.starts_with(UPDATE),
        "preview {statement:?}\n{}",
        text(screen)
    );
    assert!(
        statement.contains(r#"SET "name" ="#),
        "preview changed a different column {statement:?}\n{}",
        text(screen)
    );
    assert!(
        text(screen).contains(VALUE),
        "preview omitted {VALUE}\n{}",
        text(screen)
    );
    statement.clone()
}

/// `wrapped` is for the history detail pane, which wraps the statement at word
/// boundaries, so `SET` and the assignment land on different lines.
fn same_statement(preview: &str, later: &TerminalSnapshot, where_: &str, wrapped: bool) {
    let visible = text(later);
    let fragments = update_fragments(later);
    assert!(
        !fragments.is_empty(),
        "{where_} showed no UPDATE\n{visible}"
    );
    for fragment in &fragments {
        let same = preview.starts_with(fragment) || fragment.starts_with(preview);
        assert!(
            same,
            "{where_} statement {fragment:?} is not the preview {preview:?}\n{}",
            text(later)
        );
    }
    let assignment = format!(r#"SET "name" = '{VALUE}'"#);
    assert!(
        preview.contains(&assignment),
        "preview {preview:?} did not set {VALUE}"
    );
    let shown: &[&str] = if wrapped {
        &[r#""public"."organizations" SET"#, NAME_SET]
    } else {
        &[assignment.as_str()]
    };
    for part in shown {
        assert!(
            visible.contains(part),
            "{where_} did not show {part}\n{visible}"
        );
    }
}

/// First eight hex characters of the preview's `"id" = '…` predicate.
fn id_prefix(preview: &str) -> String {
    let marker = r#""id" = '"#;
    let start = preview
        .find(marker)
        .unwrap_or_else(|| panic!("preview had no id predicate: {preview}"));
    let rest = &preview[start + marker.len()..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_hexdigit() && ch != '-')
        .unwrap_or(rest.len());
    let id = &rest[..end];
    assert!(
        id.len() >= 8,
        "preview id {id:?} is too short to identify the row in {preview:?}"
    );
    id.chars().take(8).collect()
}

fn wait_size(app: &App, cols: u16, rows: u16) -> TerminalSnapshot {
    app.resize(cols, rows);
    app.wait_screen(&format!("size {cols}x{rows}"), |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
    })
}

#[test]
fn route_closes_the_organizations_tab() {
    let app = open("t02-commit-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[QUERY, PANEL, "1 pending"],
    );
    connect_development(&app);
    open_organizations(&app);
    let opened = app.screen();
    see(&opened, &[QUERY, PANEL, TABLE], &["1 pending", SAVE_TITLE]);
    app.key("ctrl+w");
    let closed = app.wait_absent(PANEL);
    see(
        &closed,
        &[QUERY, "Filter objects"],
        &["1 pending", PANEL, SAVE_TITLE],
    );
    let tabs = (0..closed.rows())
        .map(|row| row_text(&closed, row))
        .find(|line| line.contains(QUERY))
        .unwrap_or_else(|| panic!("tab strip missing {QUERY}\n{}", text(&closed)));
    assert!(
        !tabs.contains(TABLE),
        "organizations tab stayed on {tabs:?}\n{}",
        text(&closed)
    );
    app.finish();
}

#[test]
fn commit_cancel_keeps_pending_and_confirm_records_the_preview() {
    let app = open("t02-commit");
    connect_development(&app);
    open_organizations(&app);
    edit_name(&app);

    app.key("p");
    let preview = app.wait_text(PREVIEW);
    see(
        &preview,
        &[PREVIEW, "1 statement", VALUE, "Close"],
        &[SAVE_TITLE, SAVED],
    );
    let statement = preview_statement(&preview);
    app.key("escape");
    let closed = app.wait_absent(PREVIEW);
    see(
        &closed,
        &[VALUE, "1 pending", PANEL],
        &[PREVIEW, SAVED, SAVE_TITLE],
    );

    app.key("ctrl+s");
    let dialog = app.wait_text(SAVE_TITLE);
    see(
        &dialog,
        &[SAVE_TITLE, SCOPE, SAFE, "Cancel", "Save", VALUE],
        &[PREVIEW, SAVED, KEPT],
    );
    same_statement(&statement, &dialog, "save dialog", false);
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    shot(&app, "T02.commit.commit.120x40");

    // The first 80 by 24 frame clips the wide dialog, and the next paints the
    // title before the statement. Both have to be on screen together.
    wait_size(&app, 80, 24);
    let narrow = app.wait_screen("narrow save dialog", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(SAVE_TITLE)
            && visible.contains(r#"UPDATE "public"."organizations" SET"#)
    });
    see(&narrow, &[SAVE_TITLE, SCOPE, SAFE], &[SAVED, KEPT, PREVIEW]);
    same_statement(&statement, &narrow, "narrow save dialog", false);
    shot(&app, "T02.commit.commit.narrow");

    wait_size(&app, 120, 40);
    let restored = app.wait_screen("save dialog restored", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(SAVE_TITLE)
            && visible.contains(SCOPE)
    });
    see(&restored, &[SAVE_TITLE, SCOPE], &[SAVED, KEPT]);
    app.key("escape");
    let kept = app.wait_text(KEPT);
    see(
        &kept,
        &[KEPT, VALUE, "1 pending", PANEL],
        &[SAVE_TITLE, SAVED, PREVIEW],
    );

    app.key("ctrl+s");
    app.wait_text(SAVE_TITLE);
    app.key("enter");
    let saved = app.wait_text(SAVED);
    see(
        &saved,
        &[SAVED, VALUE, PANEL],
        &["1 pending", SAVE_TITLE, KEPT, "Saving…"],
    );

    app.key("ctrl+y");
    let history = app.wait_text(HISTORY);
    see(
        &history,
        &[HISTORY, UPDATE_HEAD, VALUE],
        &["1 pending", SAVE_TITLE],
    );
    same_statement(&statement, &history, "history", true);
    let prefix = id_prefix(&statement);
    assert!(
        text(&history).contains(&prefix),
        "history id is not the preview id {prefix}\n{}",
        text(&history)
    );
    app.finish();
}

/// The tab strip row that lists the open tabs.
fn strip_row(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(QUERY) && line.contains(TABLE))
        .unwrap_or_else(|| panic!("no tab strip row\n{}", text(screen)))
}

fn organizations_is_dirty(screen: &TerminalSnapshot) -> bool {
    let row = strip_row(screen);
    let after = row.split(TABLE).nth(1).unwrap_or("");
    after.trim_start().starts_with('•')
}

#[test]
fn issue11_save_switch_pty() {
    let mut launch = Launch::new("t02-commit-owner", TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.clock = true;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    connect_development(&app);
    open_organizations(&app);
    edit_name(&app);

    app.key("ctrl+s");
    let review = app.wait_text(SAVE_TITLE);
    let statement = preview_statement(&review);
    app.key("enter");
    app.wait_text("Saving");

    app.key("ctrl+pageup");
    let switched = app.wait_screen("the query tab is active", |screen| {
        let visible = text(screen);
        !visible.contains(PANEL) && visible.contains("Filter objects")
    });
    assert!(
        organizations_is_dirty(&switched),
        "the saving tab still shows its draft as dirty\n{}",
        text(&switched)
    );

    app.advance_clock(1_000);
    let saved = app.wait_text(SAVED);
    see(&saved, &[SAVED], &["Saving", SAVE_TITLE]);
    assert!(
        !organizations_is_dirty(&saved),
        "the saved tab still shows a dirty mark\n{}",
        text(&saved)
    );

    app.key("ctrl+y");
    let history = app.wait_text(HISTORY);
    see(&history, &[HISTORY, UPDATE, VALUE], &[SAVE_TITLE]);
    same_statement(&statement, &history, "history");
    app.finish();
}
