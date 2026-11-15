//! Holla Alternatives, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h01_alternatives/` and each checkpoint's dots become underscores.
//!
//! `rust-dirty` suggests "Inspect 12 GB of generated artifacts" first, then
//! Pull, then "Review 5 modified files". Down selects Pull, then Review.
//! Pull's alternatives include "Fetch only". Review's include
//! "Open in a Git TUI (lazygit)". Alt+Enter is the finder binding
//! (`finder.rs`, and the help line in `main.rs`).
//! The preview freshness line advances one simulated second at a time. The wide
//! baseline reads `live · 1 s ago`. `visual` waits 700ms for a quiet frame and
//! locks the next second, so the wide shot compares the last menu frame that
//! still reads 1.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const SCENARIO: &str = "rust-dirty";
const SUGGESTED: &str = "Suggested here";
const FIRST: &str = "Inspect 12 GB";
const ROW_A: &str = "Pull";
const ROW_A_ALT: &str = "Fetch only";
const ROW_B: &str = "Review 5 modified files";
const ROW_B_ALT: &str = "Open in a Git TUI (lazygit)";
const MENU: &str = "Why is this here?";
const WIDE: &str = "H01.alternatives.alternatives.120x40";
const NARROW: &str = "H01.alternatives.alternatives.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h01_alternatives/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec![
        "--scenario".to_string(),
        SCENARIO.to_string(),
        "--frame".to_string(),
        "1".to_string(),
    ];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(SUGGESTED);
    app
}

fn flat(screen: &TerminalSnapshot) -> String {
    text(screen)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = flat(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{}", text(screen));
    }
    for item in absent {
        assert!(
            !visible.contains(item),
            "forbidden {item:?}\n{}",
            text(screen)
        );
    }
}

fn on_first(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(SUGGESTED)
        && visible.contains(FIRST)
        && visible.contains(ROW_B)
        && !visible.contains(ROW_A_ALT)
        && !visible.contains(ROW_B_ALT)
        && !visible.contains(MENU)
}

fn on_pull(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(SUGGESTED)
        && visible.contains(ROW_A_ALT)
        && visible.contains(ROW_B)
        && !visible.contains(ROW_B_ALT)
        && !visible.contains(MENU)
}

fn on_review(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(ROW_B)
        && visible.contains(ROW_B_ALT)
        && visible.contains(SUGGESTED)
        && !visible.contains(ROW_A_ALT)
        && !visible.contains(MENU)
}

fn menu_for_review(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(MENU)
        && visible.contains(ROW_B)
        && visible.contains(ROW_B_ALT)
        && visible.contains("Pin here")
        && !visible.contains(ROW_A_ALT)
        && !visible.contains("Pull with rebase")
}

fn select_review(app: &App) {
    let first = app.wait_screen("artifact row is selected", on_first);
    see(
        &first,
        &[FIRST, ROW_A, ROW_B],
        &[ROW_A_ALT, ROW_B_ALT, MENU],
    );
    app.key("down");
    let pull = app.wait_screen("Pull is the selected row", on_pull);
    see(&pull, &[ROW_A, ROW_A_ALT, ROW_B], &[ROW_B_ALT, MENU]);
    app.key("down");
    let review = app.wait_screen("Review is the selected row", on_review);
    see(&review, &[ROW_B, ROW_B_ALT], &[ROW_A_ALT, MENU]);
}

fn open_alternatives(app: &App) {
    app.key("alt+enter");
    let menu = app.wait_screen("alternatives for Review", menu_for_review);
    see(
        &menu,
        &[MENU, ROW_B, ROW_B_ALT, "Pin here", "Set alias…"],
        &[ROW_A_ALT, "Pull with rebase"],
    );
}

fn close_alternatives(app: &App) {
    app.key("escape");
    let back = app.wait_screen("alternatives closed on Review", on_review);
    see(&back, &[ROW_B, ROW_B_ALT, SUGGESTED], &[MENU, ROW_A_ALT]);
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. Below about 100 columns the header omits the size chip.
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
    app.visual(&visual_id(checkpoint));
}

/// Compare the last 120×40 alternatives frame whose freshness digit is still 1.
fn shot_wide_fresh(app: &App) {
    let id = visual_id(WIDE);
    app.visual_while(&id, |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && menu_for_review(screen)
            && text(screen).contains("live · 1 s ago")
    });
}

#[test]
fn h01_alternatives_route_returns_to_the_finder_row() {
    let app = open("h01-alternatives-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    select_review(&app);
    open_alternatives(&app);
    close_alternatives(&app);
    let returned = app.screen();
    assert_eq!((returned.cols(), returned.rows()), (120, 40));
    see(
        &returned,
        &[SUGGESTED, ROW_B, ROW_B_ALT],
        &[MENU, ROW_A_ALT],
    );
    app.finish();
}

#[test]
fn h01_alternatives_lists_the_selected_rows_action() {
    let app = open("h01-alternatives-list");
    select_review(&app);
    open_alternatives(&app);
    flash_settle(&app);
    shot_wide_fresh(&app);
    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(menu_for_review(&wide), "wide menu\n{}", text(&wide));

    app.resize(80, 24);
    let narrowed = app.wait_screen("alternatives at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && menu_for_review(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    assert!(
        menu_for_review(&narrowed),
        "narrow menu\n{}",
        text(&narrowed)
    );
    // The menu covers the preview at 80×24, so the freshness line is not drawn.
    flash_settle(&app);
    shot(&app, NARROW);
    app.resize(120, 40);

    let restored = app.wait_screen("alternatives back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && menu_for_review(screen)
    });
    assert!(
        menu_for_review(&restored),
        "restored menu\n{}",
        text(&restored)
    );

    close_alternatives(&app);
    let closed = app.screen();
    see(
        &closed,
        &[ROW_B, ROW_B_ALT, SUGGESTED],
        &[MENU, ROW_A_ALT, "Pull with rebase"],
    );
    app.finish();
}
