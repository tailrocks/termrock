//! Holla cleanup checklist, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h06_cleanup/` and each checkpoint's dots become underscores.
//! Catalog item `cleanup.review-all` is labeled `Review cleanup candidates`
//! and launches `Launch::Cleanup { category: None }`. The crumb is `Cleanup`.
//! Categories start collapsed. Esc pops only when none are open; an open
//! category collapses first. `d` and Backspace push the cleanup gate, so
//! this leaf never sends them.
//!
//! `parity-delete-safety` paints no `Eligibility::Ineligible` row. The empty
//! Cargo registry category is the refusal (`No eligible candidate in this
//! category`, mark `[–]`). Protected fixture paths are not candidates.
//! The heading is clipped to the list width, so the test locks the visible
//! prefix, including `dry run: nothing is removed`.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const LABEL: &str = "Review cleanup candidates";
const CRUMB: &str = "Here › Cleanup";
const TITLE: &str = "Cleanup review";
const LIVE_HEADING: &str = "mode Trash (recoverable until emptied) · live · m toggles the mode";
const DRY_HEADING: &str = "dry run: nothing is removed";
const PERMANENT_HEADING: &str = "mode PERMANENT (cannot be undone)";
const EMPTY_STATUS: &str = "No eligible candidate in this category";
const SELECT_ALL: &str = "Every eligible candidate selected · 2.1 GiB";
const UNSELECT_ALL: &str = "Everything unselected";
const TRASH_STATUS: &str = "Mode: Trash · recoverable until the Trash is emptied";
const PERMANENT_STATUS: &str = "Mode: PERMANENT · cannot be undone · any earlier review is void";
const DRY_STATUS: &str = "Dry run: paths are validated and sized, the log records would-remove, nothing is deleted and no daemon";
const LIVE_STATUS: &str = "Live run";
const DRY_METHOD: &str = "dry run · would be removed";
const DRY_METHOD_REST: &str = "nothing changes";
const CARGO: &str = "Cargo registry cache";
const CACHES: &str = "User caches";
const LOGS: &str = "User logs";
const ARTIFACTS: &str = "Project artifacts";
const GOOGLE: &str = "~/Library/Caches/Google";
const TARGET: &str = "~/Projects/safe/target";
const LOCKED: &str = "~/Projects/safe/locked";
const HOME: &str = "~/Projects/safe";
const EMPTY_MARK: &str = "[–]";
const OPEN_MARK: &str = "[ ]";
const CHECK: &str = "[✓]";
const QUIT: &str = "Quit holla";
const GATE: &str = "Review · ";
const WIDE: &str = "H06.cleanup.select.120x40";
const NARROW: &str = "H06.cleanup.select.narrow";

const FOREIGN: &[&str] = &["vm.img", "~/work/big", "sparse.img", "842 of 926"];
const PROTECTED: &[&str] = &[
    "~/.Trash",
    "Application Support",
    "Mobile Documents",
    "com.apple.Safari",
    "~/work/other",
    "safe/linked",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h06_cleanup/{}",
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
    launch.args = vec!["--scenario".to_string(), "parity-delete-safety".to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions");
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
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn row_has(screen: &TerminalSnapshot, mark: &str, label: &str) -> bool {
    text(screen)
        .lines()
        .any(|line| line.contains(mark) && line.contains(label))
}

fn focus_down(app: &App, needle: &str) {
    for _ in 0..20 {
        if focused_line(&app.screen()).contains(needle) {
            return;
        }
        let before = focused_line(&app.screen());
        app.key("down");
        app.wait_screen(needle, |screen| {
            let line = focused_line(screen);
            line.contains(needle) || line != before
        });
    }
    panic!("never focused {needle}\n{}", text(&app.screen()));
}

fn expand(app: &App) {
    app.key("right");
    app.wait_screen("expanded", |screen| focused_line(screen).contains('▾'));
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

fn checklist(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB)
        && visible.contains(CHECK)
        && visible.contains(DRY_HEADING)
        && visible.contains(TARGET)
        && row_has(screen, EMPTY_MARK, CARGO)
        && !visible.contains(GATE)
        && !visible.contains(QUIT)
        && FOREIGN.iter().all(|item| !visible.contains(item))
}

/// The expanded list hides this heading at 80×24, so the shot collapses one category.
fn narrow_checklist(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB)
        && visible.contains(CHECK)
        && visible.contains(DRY_HEADING)
        && row_has(screen, EMPTY_MARK, CARGO)
        && !visible.contains(GATE)
        && !visible.contains(QUIT)
        && FOREIGN.iter().all(|item| !visible.contains(item))
}

fn open_cleanup(app: &App) -> TerminalSnapshot {
    app.paste(LABEL);
    app.wait_text(&format!("▎ {LABEL}"));
    app.key("enter");
    app.wait_screen("cleanup checklist", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB)
            && visible.contains(TITLE)
            && visible.contains(LIVE_HEADING)
            && visible.contains("1 selected (44.0 KiB)")
            && row_has(screen, CHECK, LOGS)
            && row_has(screen, EMPTY_MARK, CARGO)
            && !visible.contains(QUIT)
            && !visible.contains(GATE)
    })
}

fn absent_others(screen: &TerminalSnapshot) {
    see(screen, &[], [FOREIGN, PROTECTED].concat().as_slice());
}

#[test]
fn h06_cleanup_route_returns_to_the_finder() {
    let app = launch("h06-cleanup-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[HOME, "mbp"], &[TITLE, QUIT, GATE]);
    absent_others(&landing);

    let page = open_cleanup(&app);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(&page, &[CRUMB, TITLE, LIVE_HEADING, HOME], &[QUIT, GATE]);

    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        visible.contains(LABEL)
            && visible.contains(HOME)
            && !visible.contains(TITLE)
            && !visible.contains(LIVE_HEADING)
            && !visible.contains(QUIT)
    });
    see(&finder, &[LABEL, HOME], &[TITLE, LIVE_HEADING, QUIT, GATE]);
    app.finish();
}

#[test]
fn h06_cleanup_select_marks_only_eligible_candidates() {
    let app = launch("h06-cleanup-select");
    let page = open_cleanup(&app);
    see(
        &page,
        &[
            CRUMB,
            TITLE,
            LIVE_HEADING,
            HOME,
            "4 categories · 2.1 GiB",
            "Trash",
        ],
        &[QUIT, GATE, "ineligible ·", "Cannot select"],
    );
    absent_others(&page);
    assert!(
        row_has(&page, CHECK, LOGS),
        "preselected logs\n{}",
        text(&page)
    );
    assert!(
        row_has(&page, EMPTY_MARK, CARGO),
        "empty cargo\n{}",
        text(&page)
    );

    expand(&app);
    let empty = app.wait_screen("cargo has no candidates", |screen| {
        text(screen).contains("no candidates · nothing to remove")
    });
    see(
        &empty,
        &["no candidates · nothing to remove"],
        &["ineligible ·"],
    );
    app.key("space");
    let refused = app.wait_screen("empty category refuses space", |screen| {
        text(screen).contains(EMPTY_STATUS) && row_has(screen, EMPTY_MARK, CARGO)
    });
    see(
        &refused,
        &[EMPTY_STATUS, "1 selected (44.0 KiB)"],
        &["Cannot select", "ineligible ·", QUIT],
    );

    focus_down(&app, CACHES);
    expand(&app);
    focus_down(&app, GOOGLE);
    let google = app.wait_screen("google cache", |screen| {
        row_has(screen, OPEN_MARK, GOOGLE)
            && text(screen).contains("eligible · review first")
            && text(screen).contains("never preselected")
            && text(screen).contains("~/Library/Caches")
    });
    see(
        &google,
        &[
            GOOGLE,
            "~/Library/Caches/com.app",
            "eligible · review first",
        ],
        &["ineligible ·", "Denied"],
    );
    app.key("space");
    let picked = app.wait_screen("google selected", |screen| {
        row_has(screen, CHECK, GOOGLE) && text(screen).contains("2 items selected")
    });
    see(
        &picked,
        &[CHECK, "2 items selected", "selected"],
        &["Cannot select"],
    );

    focus_down(&app, ARTIFACTS);
    let category = app.wait_screen("project artifacts", |screen| {
        let line = focused_line(screen);
        line.contains(ARTIFACTS)
            && !line.contains(TARGET)
            && text(screen).contains(LOCKED)
            && text(screen).contains("Unreadable")
            && text(screen).contains("1 folder skipped")
            && text(screen).contains("2 of 2 candidates eligible")
    });
    see(
        &category,
        &[LOCKED, "Unreadable", "1 folder skipped", HOME],
        &["ineligible ·", "Denied", "[✓] ~/Projects/safe/locked"],
    );
    assert!(
        !text(&category)
            .lines()
            .any(|line| line.contains(LOCKED) && line.contains(CHECK)),
        "locked is not a checked row\n{}",
        text(&category)
    );

    expand(&app);
    focus_down(&app, TARGET);
    app.key("space");
    let target = app.wait_screen("target selected", |screen| {
        row_has(screen, CHECK, TARGET) && text(screen).contains("3 items selected")
    });
    see(
        &target,
        &[TARGET, "3 items selected", "selected", "ünï"],
        &["ineligible ·", "Denied", "target/debug"],
    );

    app.key("a");
    let all = app.wait_screen("every eligible selected", |screen| {
        text(screen).contains(SELECT_ALL)
            && text(screen).contains("6 selected (2.1 GiB)")
            && row_has(screen, EMPTY_MARK, CARGO)
            && row_has(screen, CHECK, TARGET)
            && row_has(screen, CHECK, LOGS)
    });
    see(&all, &[SELECT_ALL, "6 selected (2.1 GiB)"], &[GATE, QUIT]);
    absent_others(&all);
    assert!(
        !text(&all).contains("7 selected"),
        "protected path counted\n{}",
        text(&all)
    );

    app.key("a");
    let none = app.wait_screen("everything unselected", |screen| {
        text(screen).contains(UNSELECT_ALL)
            && text(screen).contains("0 selected (0 B)")
            && row_has(screen, OPEN_MARK, TARGET)
            && row_has(screen, OPEN_MARK, LOGS)
            && row_has(screen, EMPTY_MARK, CARGO)
    });
    see(&none, &[UNSELECT_ALL, "0 selected (0 B)"], &[GATE]);

    app.key("a");
    app.wait_screen("eligible selected again", |screen| {
        text(screen).contains(SELECT_ALL) && row_has(screen, CHECK, TARGET)
    });

    app.key("m");
    let permanent = app.wait_screen("permanent mode", |screen| {
        text(screen).contains(PERMANENT_STATUS)
            && text(screen).contains(PERMANENT_HEADING)
            && text(screen).contains("· PERMANENT")
            && text(screen).contains("permanent · cannot be")
    });
    see(
        &permanent,
        &[PERMANENT_STATUS, PERMANENT_HEADING, "permanent · cannot be"],
        &[DRY_HEADING, GATE],
    );

    app.key("m");
    let trash = app.wait_screen("trash mode", |screen| {
        text(screen).contains(TRASH_STATUS) && text(screen).contains(LIVE_HEADING)
    });
    see(
        &trash,
        &[TRASH_STATUS, LIVE_HEADING],
        &[PERMANENT_HEADING, DRY_HEADING],
    );

    app.key("n");
    let dry = app.wait_screen("dry run", |screen| {
        text(screen).contains(DRY_STATUS)
            && text(screen).contains(DRY_HEADING)
            && text(screen).contains(DRY_METHOD)
            && text(screen).contains(DRY_METHOD_REST)
            && text(screen).contains("· Trash · dry run")
            && checklist(screen)
    });
    see(
        &dry,
        &[DRY_STATUS, DRY_HEADING, DRY_METHOD, DRY_METHOD_REST, CHECK],
        &[GATE, QUIT, LIVE_STATUS],
    );

    app.key("n");
    let live = app.wait_screen("live run", |screen| {
        text(screen).contains(LIVE_STATUS) && text(screen).contains(LIVE_HEADING)
    });
    see(&live, &[LIVE_STATUS, LIVE_HEADING], &[DRY_HEADING, GATE]);

    app.key("n");
    app.wait_screen("dry run for the shot", checklist);

    nudge(&app, checklist);
    shot(&app, WIDE);

    app.resize(80, 24);
    app.key("left");
    let narrowed = app.wait_screen("narrow checklist", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && narrow_checklist(screen)
    });
    see(&narrowed, &[CRUMB, CHECK, DRY_HEADING], &[GATE, QUIT]);
    nudge(&app, narrow_checklist);
    shot(&app, NARROW);

    app.key("right");
    focus_down(&app, TARGET);
    app.resize(120, 40);
    let restored = app.wait_screen("checklist restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && checklist(screen)
    });
    see(
        &restored,
        &[CRUMB, DRY_HEADING, "6 selected (2.1 GiB)", TARGET],
        &[GATE, QUIT],
    );
    absent_others(&restored);
    app.finish();
}

#[test]
fn h06_cleanup_scenario_parity_delete_safety() {
    let app = launch("h06-cleanup-scenario");
    let landing = app.screen();
    see(
        &landing,
        &[HOME, "mbp", "safe · Rust project"],
        &[TITLE, QUIT],
    );
    absent_others(&landing);

    let page = open_cleanup(&app);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(
        &page,
        &[CRUMB, TITLE, LIVE_HEADING, HOME, "4 categories"],
        &[QUIT, GATE, "ineligible ·"],
    );
    absent_others(&page);

    focus_down(&app, CACHES);
    let caches = app.wait_screen("user caches", |screen| {
        focused_line(screen).contains(CACHES) && text(screen).contains("~/Library/Caches")
    });
    see(&caches, &["~/Library/Caches", CACHES], &["ineligible ·"]);

    focus_down(&app, ARTIFACTS);
    expand(&app);
    let artifacts = app.wait_screen("scenario artifacts", |screen| {
        text(screen).contains(TARGET)
            && text(screen).contains(LOCKED)
            && text(screen).contains("Unreadable")
            && text(screen).contains("ünï")
    });
    see(
        &artifacts,
        &[TARGET, LOCKED, "Unreadable", HOME],
        &[QUIT, GATE, "target/debug"],
    );
    absent_others(&artifacts);
    app.finish();
}
