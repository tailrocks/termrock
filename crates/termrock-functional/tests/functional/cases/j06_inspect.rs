//! Jackin change inspector, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_inspect/`. The shots are the open diff of the selected
//! change. They do not nudge. `Launch::new` leaves the clock off. `App::new`
//! would enable it for jackin.
//!
//! `Daemon::touched_files` sorts the Claude boot touches
//! `src/settlement/mod.rs` and `src/settlement/retry.rs`. `changes_for` keeps
//! that order because `uncommitted` is already 2, so the compact cursor starts
//! on `mod.rs`.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, cell_bg, find, row_text, text, try_launch};

const INSTANCE: &str = "7f3a";
const OTHER: &str = "9b02";
const WORKSPACE: &str = "payments-platform";
const IDENTITY: &str = "payments-platform › the-architect";
const ATTACHED: &str = "Attached to payments-platform · tabs and panes restored";
const PAGE: &str = "Workspaces";
const CURRENT: &str = "Current directory";
const OUTRO: &str = "You were in the Construct";
const PERMISSION: &str = "Allow edit to src/settlement/config.rs? (y/n)";
const TITLE: &str = "Inspect changes · payments-platform";
const MOD: &str = "src/settlement/mod.rs";
const RETRY: &str = "src/settlement/retry.rs";
const RAND: &str = "use rand::Rng;";
const BACKOFF: &str = "pub mod backoff;";
const RETRY_HUNKS: &str = "3 hunks";
const MOD_HUNK: &str = "1 hunk";
const FILES: &str = "2 files";
const COPY: &str = "Copy";
const BACK_LIST: &str = "Back to list";
const WIDE: &str = "J06.inspect.inspect.120x40";
const NARROW: &str = "J06.inspect.inspect.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_inspect/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "returning".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn wait_settled(
    app: &App,
    reason: &str,
    hold: Duration,
    budget: Duration,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    let mut last: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let same = last.as_ref().is_some_and(|prev| prev == &screen);
            if same {
                if since.expect("stability start").elapsed() >= hold {
                    return screen;
                }
            } else {
                last = Some(screen);
                since = Some(Instant::now());
            }
        } else {
            last = None;
            since = None;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} frame not seen\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn instance_row(screen: &TerminalSnapshot) -> bool {
    let line = focused_line(screen);
    line.contains(INSTANCE) && !line.contains(OTHER)
}

fn manager_up(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(CURRENT)
        && visible.contains(WORKSPACE)
        && !visible.contains(OUTRO)
        && !visible.contains(ATTACHED)
}

/// `ManagerScreen::new` selects Current directory and leaves workspaces collapsed.
fn select_7f3a(app: &App) {
    let first = app.wait_screen("manager", manager_up);
    let line = focused_line(&first);
    if !instance_row(&first) {
        assert!(
            line.contains(CURRENT) && !line.contains(INSTANCE) && !line.contains(OTHER),
            "initial selection is not Current directory\n{line}\n{}",
            text(&first)
        );
    }
    move_to_instance(app);
}

fn move_to_instance(app: &App) {
    if instance_row(&app.screen()) {
        return;
    }
    if focused_line(&app.screen()).contains(CURRENT) {
        app.key("down");
        app.wait_screen("payments-platform row", |screen| {
            let line = focused_line(screen);
            line.contains(WORKSPACE) && !line.contains(INSTANCE) && !line.contains(OTHER)
        });
    }
    if focused_line(&app.screen()).contains(WORKSPACE) && !focused_line(&app.screen()).contains('▾')
    {
        app.key("right");
        app.wait_screen("expanded payments-platform", |screen| {
            let line = focused_line(screen);
            line.contains('▾')
                && line.contains(WORKSPACE)
                && text(screen).contains(INSTANCE)
                && !line.contains(OTHER)
        });
    }
    for _ in 0..4 {
        if instance_row(&app.screen()) {
            return;
        }
        let before = focused_line(&app.screen());
        assert!(
            !before.contains(OTHER),
            "refusing to attach jk-{OTHER}\n{before}\n{}",
            text(&app.screen())
        );
        app.key("down");
        app.wait_screen("next row", |screen| focused_line(screen) != before);
    }
    assert!(
        instance_row(&app.screen()),
        "jk-{INSTANCE} row not selected\n{}",
        text(&app.screen())
    );
}

fn attach(app: &App) -> TerminalSnapshot {
    assert!(
        instance_row(&app.screen()),
        "Enter would not attach jk-{INSTANCE}\n{}",
        text(&app.screen())
    );
    app.key("enter");
    app.wait_screen("attached", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(ATTACHED)
            && visible.contains(IDENTITY)
            && visible.contains(PERMISSION)
            && !visible.contains(PAGE)
            && !visible.contains(OUTRO)
            && !visible.contains(OTHER)
    })
}

const VIEW_ITEMS: [&str; 6] = [
    "Zoom pane",
    "Redraw",
    "Usage",
    "Container info",
    "GitHub context",
    "Inspect changes",
];

fn filled_label(
    screen: &TerminalSnapshot,
    cursor_bg: &tui_test::SnapshotColor,
) -> Option<&'static str> {
    let mut found = None;
    for label in VIEW_ITEMS {
        if text(screen).contains(label) && cell_bg(screen, find(screen, label)) == *cursor_bg {
            assert!(found.is_none(), "two view rows share the cursor fill");
            found = Some(label);
        }
    }
    found
}

/// F10 opens File. Right reaches View. Down moves the fill cursor onto
/// Inspect changes, one item per key, and waits until that fill is painted.
/// The item has no shortcut. A following Down is not sent until the fill moves,
/// because a second Down in the same frame is dropped.
fn open_inspector(app: &App) -> TerminalSnapshot {
    app.key("f10");
    app.wait_screen("file menu", |screen| {
        let visible = text(screen);
        visible.contains("New tab") && visible.contains("Split right")
    });
    app.key("right");
    app.wait_screen("edit menu", |screen| {
        text(screen).contains("Copy selection") && text(screen).contains("Paste clipboard")
    });
    app.key("right");
    let view = app.wait_screen("view menu", |screen| {
        let visible = text(screen);
        visible.contains("Zoom pane")
            && visible.contains("Inspect changes")
            && visible.contains("GitHub context")
            && !visible.contains("New tab")
    });
    let cursor_bg = cell_bg(&view, find(&view, "Zoom pane"));
    let idle_bg = cell_bg(&view, find(&view, "Inspect changes"));
    assert_ne!(
        cursor_bg,
        idle_bg,
        "view menu cursor fill matches the idle row\n{}",
        text(&view)
    );
    assert_eq!(
        filled_label(&view, &cursor_bg),
        Some("Zoom pane"),
        "view menu did not open on Zoom pane\n{}",
        text(&view)
    );
    for label in &VIEW_ITEMS[1..] {
        app.key("down");
        app.wait_screen(label, |screen| {
            filled_label(screen, &cursor_bg) == Some(*label)
        });
    }
    app.key("enter");
    app.wait_screen("inspector", |screen| {
        let visible = text(screen);
        let line = focused_line(screen);
        visible.contains(TITLE)
            && visible.contains(FILES)
            && visible.contains(MOD)
            && visible.contains(RETRY)
            && line.contains(MOD)
            && !line.contains(RETRY)
            && !visible.contains(PAGE)
            && !visible.contains(OUTRO)
            && !visible.contains("Nothing changed in this instance")
    })
}

fn list_on(screen: &TerminalSnapshot, path: &str, other: &str) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(TITLE)
        && visible.contains(FILES)
        && visible.contains(MOD)
        && visible.contains(RETRY)
        && line.contains(path)
        && !line.contains(other)
        && !visible.contains(BACK_LIST)
}

fn retry_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(RETRY)
        && visible.contains(RAND)
        && visible.contains(RETRY_HUNKS)
        && !visible.contains(MOD)
        && !visible.contains(BACKOFF)
        && !visible.contains(MOD_HUNK)
        && !visible.contains(PAGE)
        && !visible.contains(OUTRO)
}

fn retry_wide(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120
        && screen.rows() == 40
        && retry_body(screen)
        && text(screen).contains(COPY)
        && text(screen).contains(BACK_LIST)
}

fn retry_narrow(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 80 && screen.rows() == 24 && retry_body(screen)
}

fn mod_diff(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(TITLE)
        && visible.contains(MOD)
        && visible.contains(BACKOFF)
        && visible.contains(MOD_HUNK)
        && visible.contains(BACK_LIST)
        && !visible.contains(RETRY)
        && !visible.contains(RAND)
        && !visible.contains(RETRY_HUNKS)
        && !visible.contains(PAGE)
        && !visible.contains(OUTRO)
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let screen = wait_settled(
        app,
        &test_id,
        Duration::from_millis(200),
        Duration::from_secs(8),
        accept,
    );
    crate::support::assert_frame(app, &screen, &test_id);
}

/// `TextViewport::select_all` paints `Theme::selection` over the open diff.
/// `y` is `ViewportCmd::Copy` of `selected_text`. `InspectChanges::compact_key`
/// drops that action, so the painted selection is the visible copy result.
fn select_diff(app: &App, needle: &str) -> tui_test::SnapshotColor {
    let before = app.screen();
    let idle = cell_bg(&before, find(&before, needle));
    app.key("ctrl+a");
    let selected = app.wait_screen(&format!("selected {needle}"), |screen| {
        text(screen).contains(needle) && cell_bg(screen, find(screen, needle)) != idle
    });
    let bg = cell_bg(&selected, find(&selected, needle));
    assert_ne!(bg, idle, "selection did not restyle {needle}");
    bg
}

fn hold_copy(app: &App, reason: &str, present: &[&str], absent: &[&str]) {
    let deadline = Instant::now() + Duration::from_millis(200);
    let mut frames = 0usize;
    while Instant::now() < deadline {
        assert!(app.child_running(), "{reason} quit the process");
        let visible = text(&app.screen());
        for needle in present {
            assert!(
                visible.contains(needle),
                "{reason} missing {needle}\n{visible}"
            );
        }
        for needle in absent {
            assert!(
                !visible.contains(needle),
                "{reason} shows {needle}\n{visible}"
            );
        }
        frames += 1;
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(frames > 1, "{reason} did not hold a frame");
}

#[test]
fn j06_inspect_route() {
    let app = open("j06-inspect-route");
    select_7f3a(&app);
    let attached = attach(&app);
    require(&attached, ATTACHED);
    require(&attached, IDENTITY);
    refuse(&attached, PAGE);
    refuse(&attached, OUTRO);

    let list = open_inspector(&app);
    let first = focused_line(&list);
    assert!(
        first.contains(MOD) && !first.contains(RETRY),
        "inspector did not open on {MOD}\n{first}\n{}",
        text(&list)
    );
    require(&list, RETRY);
    refuse(&list, OUTRO);

    app.key("down");
    let moved = app.wait_screen("retry row", |screen| list_on(screen, RETRY, MOD));
    let line = focused_line(&moved);
    assert!(
        line.contains(RETRY) && !line.contains(MOD),
        "selection did not move to {RETRY}\n{line}\n{}",
        text(&moved)
    );
    require(&moved, MOD);
    refuse(&moved, OUTRO);
    refuse(&moved, PAGE);

    app.key("esc");
    let back = app.wait_screen("capsule", |screen| {
        let visible = text(screen);
        visible.contains(IDENTITY)
            && visible.contains(PERMISSION)
            && !visible.contains(TITLE)
            && !visible.contains("Inspect changes")
            && !visible.contains(OUTRO)
            && !visible.contains(PAGE)
    });
    require(&back, IDENTITY);
    refuse(&back, OUTRO);
    assert!(app.child_running(), "the inspector quit the process");
}

#[test]
fn j06_inspect_inspect() {
    let app = open("j06-inspect-inspect");
    select_7f3a(&app);
    let attached = attach(&app);
    require(&attached, ATTACHED);
    refuse(&attached, OUTRO);

    let list = open_inspector(&app);
    assert!(
        focused_line(&list).contains(MOD),
        "list did not start on {MOD}\n{}",
        text(&list)
    );
    app.key("down");
    let selected = app.wait_screen("retry selected", |screen| list_on(screen, RETRY, MOD));
    assert!(
        focused_line(&selected).contains(RETRY),
        "selected row is not {RETRY}\n{}",
        text(&selected)
    );

    app.key("enter");
    let diff = app.wait_screen("retry diff", retry_wide);
    require(&diff, RETRY);
    require(&diff, RAND);
    require(&diff, RETRY_HUNKS);
    require(&diff, COPY);
    refuse(&diff, MOD);
    refuse(&diff, BACKOFF);
    refuse(&diff, PAGE);
    refuse(&diff, OUTRO);
    assert_eq!((diff.cols(), diff.rows()), (120, 40));

    shot(&app, WIDE, retry_wide);
    app.resize(80, 24);
    shot(&app, NARROW, retry_narrow);
    app.resize(120, 40);
    let restored = wait_settled(
        &app,
        "wide diff",
        Duration::from_millis(200),
        Duration::from_secs(8),
        retry_wide,
    );
    require(&restored, RETRY);
    require(&restored, RAND);
    refuse(&restored, MOD);
    refuse(&restored, BACKOFF);
    refuse(&restored, PAGE);

    app.key("v");
    let review = app.wait_screen("review mode after resize", |screen| {
        let visible = text(screen);
        retry_body(screen) && visible.contains("review") && visible.contains("Show unified")
    });
    require(&review, RAND);
    app.key("v");
    let unified = app.wait_screen("unified mode after resize", |screen| {
        let visible = text(screen);
        retry_wide(screen) && visible.contains("unified") && visible.contains("Show side by side")
    });
    require(&unified, RAND);

    let selected_bg = select_diff(&app, RAND);
    let chosen = app.screen();
    require(&chosen, COPY);
    assert_eq!(cell_bg(&chosen, find(&chosen, RAND)), selected_bg);
    app.key("y");
    let copied = app.wait_screen("copied retry", |screen| {
        retry_wide(screen) && cell_bg(screen, find(screen, RAND)) == selected_bg
    });
    require(&copied, RAND);
    require(&copied, RETRY);
    refuse(&copied, MOD);
    refuse(&copied, BACKOFF);
    hold_copy(
        &app,
        "retry copy",
        &[RETRY, RAND, RETRY_HUNKS, COPY],
        &[MOD, BACKOFF, PAGE, OUTRO],
    );

    app.key("esc");
    app.wait_screen("list again", |screen| list_on(screen, RETRY, MOD));
    app.key("up");
    let other = app.wait_screen("mod selected", |screen| list_on(screen, MOD, RETRY));
    assert!(
        focused_line(&other).contains(MOD) && !focused_line(&other).contains(RETRY),
        "other row is not {MOD}\n{}",
        text(&other)
    );
    app.key("enter");
    let other_diff = app.wait_screen("mod diff", mod_diff);
    require(&other_diff, MOD);
    require(&other_diff, BACKOFF);
    refuse(&other_diff, RETRY);
    refuse(&other_diff, RAND);

    let other_bg = select_diff(&app, BACKOFF);
    app.key("y");
    let other_copy = app.wait_screen("copied mod", |screen| {
        mod_diff(screen) && cell_bg(screen, find(screen, BACKOFF)) == other_bg
    });
    require(&other_copy, BACKOFF);
    require(&other_copy, MOD);
    refuse(&other_copy, RETRY);
    refuse(&other_copy, RAND);
    hold_copy(
        &app,
        "mod copy",
        &[MOD, BACKOFF, MOD_HUNK],
        &[RETRY, RAND, RETRY_HUNKS, PAGE, OUTRO],
    );
    assert!(app.child_running(), "the process quit");
}
