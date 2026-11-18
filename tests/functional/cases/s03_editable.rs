//! Editable tables route, sections, specimens, compare levels, cell edit, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s03_editable/` and each case id's dots become underscores.

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{Launch, Point, SHOWCASE, cell_bg, find_all, row_text, text, try_launch};

const TITLE: &str = "Editable tables";
const BLURB: &str = "Navigation is reversed cell; editing is a cursor. They never look alike.";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const RANGE: &str = "1–13 of 14";
const TASK: &str = "Add rate limiting";
const GRID_ABSENT: &[&str] = &["focus + hover", "busy", "loading", "selected"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s03_editable/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Components");
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(
            !visible.contains(item),
            "forbidden {item:?} is visible\n{visible}"
        );
    }
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
}

fn lines_of(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect()
}

fn line_after(screen: &TerminalSnapshot, label: &str) -> String {
    let lines = lines_of(screen);
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    lines
        .get(index + 1)
        .unwrap_or_else(|| panic!("no value under {label:?}\n{}", text(screen)))
        .clone()
}

fn block_from(lines: &[String], label: &str, len: usize) -> String {
    let index = lines
        .iter()
        .position(|line| line.contains(label) && line.contains("ID"))
        .unwrap_or_else(|| panic!("block {label:?} missing\n{}", lines.join("\n")));
    lines
        .iter()
        .skip(index)
        .take(len)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn locate(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    hits[0]
}

fn click_where(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let hit = locate(app, label, pred);
    app.click(hit);
    hit
}

fn click_tab(app: &crate::support::App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &crate::support::App, label: &str) {
    click_where(app, label, |hit| hit.x < 24);
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn grid_hit(screen: &TerminalSnapshot, needle: &str) -> Point {
    find_all(screen, needle)
        .into_iter()
        .filter(|point| point.x > 24 && point.y >= 8)
        .min_by_key(|point| (point.y, point.x))
        .unwrap_or_else(|| panic!("no grid cell {needle}\n{}", text(screen)))
}

/// The accepted frame keeps one data row in the pressed fill after the
/// cursor has moved. Under load the click release clears that fill before
/// the shot. Hold the row so the fill is present for the capture.
fn hold_pressed(app: &crate::support::App, needle: &str, other: &str) -> Point {
    let screen = app.wait_text(needle);
    let hit = grid_hit(&screen, needle);
    let other_hit = grid_hit(&screen, other);
    app.down(hit);
    app.wait_screen("pressed row", |screen| {
        let hot = cell_bg(screen, hit);
        let rest = cell_bg(screen, other_hit);
        row_text(screen, u32::from(hit.y)).contains(needle) && hot != rest
    });
    hit
}

fn row_fill_pressed(screen: &TerminalSnapshot, needle: &str) -> bool {
    let Some(hit) = find_all(screen, needle)
        .into_iter()
        .find(|point| point.x > 24 && point.y >= 8)
    else {
        return false;
    };
    cell_bg(screen, hit) == SnapshotColor::Rgb(0x38, 0x38, 0x3e)
        && row_text(screen, u32::from(hit.y)).contains(needle)
}

fn check_frame(app: &crate::support::App, screen: &TerminalSnapshot, test_id: &str) {
    let root = crate::support::oracle_baselines();
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        let fields = failure.changed_fields();
        // The press fill is the accepted paint. A hidden cursor is not drawn;
        // the terminal parks it on different cells of otherwise equal frames.
        let parked = failure.changed_cell_total() == 0
            && !screen.cursor().visible()
            && !fields.is_empty()
            && fields
                .iter()
                .all(|field| field == "cursor.col" || field == "cursor.row");
        if parked {
            return;
        }
        panic!(
            "{test_id} {} sha {} path {} cells {} fields {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total(),
            fields.join(",")
        );
    }
}

/// `visual` waits out the press. Hold the data row and compare that frame.
fn shot_pressed(app: &crate::support::App, needle: &str, case_id: &str) {
    let screen = app.wait_text(needle);
    let hit = grid_hit(&screen, needle);
    app.down(hit);
    let held = app.wait_screen("pressed row fill", |screen| {
        row_fill_pressed(screen, needle)
    });
    check_frame(app, &held, &visual_id(case_id));
    app.up(hit);
}

fn task_point(app: &crate::support::App) -> Point {
    locate(app, TASK, |hit| hit.x > 24 && hit.y < 16)
}

fn focus_task(app: &crate::support::App) {
    let hit = task_point(app);
    app.click(hit);
    app.wait_text("↑↓←→");
    app.move_to(Point { x: 110, y: 1 });
}

fn option_page(app: &crate::support::App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options")
}

fn header_row(screen: &TerminalSnapshot) -> String {
    lines_of(screen)
        .into_iter()
        .find(|line| line.contains("Task") && line.contains("Owner"))
        .unwrap_or_else(|| panic!("header missing\n{}", text(screen)))
}

fn column_bars(line: &str) -> bool {
    let mut groups = 0;
    let mut in_bar = false;
    for c in line.chars() {
        if c == '━' || c == '┄' {
            if !in_bar {
                groups += 1;
                in_bar = true;
            }
        } else if c == ' ' {
            in_bar = false;
        }
    }
    groups >= 2
}

fn narrow_block(visible: &str) -> Option<String> {
    let lines: Vec<String> = visible.lines().map(str::to_string).collect();
    let index = lines
        .iter()
        .position(|line| line.contains("narrow") && line.contains("ID"))?;
    if index + 3 >= lines.len() {
        return None;
    }
    let block = lines[index..=index + 3].join("\n");
    let complete = block.contains("#1040")
        && block.contains("#1041")
        && block.contains("Add rate limit")
        && !block.contains("#1042")
        && !block.contains("limiting")
        && column_bars(&lines[index + 3]);
    complete.then_some(block)
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "editable"];
    let app = open("s03-route-page", 120, 40, &args);
    let screen = app.wait_text(TITLE);
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "#1040",
            TASK,
            "0 edits",
            RANGE,
            "fix/checkout flake",
        ],
        &[
            OVERVIEW,
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "pending-change queue",
            "No tasks yet",
        ],
    );
    shot(
        &app,
        "sc.editable.route",
        "--page editable opens the playground with the sidebar focus on Editable",
        &args,
    );
    app.finish();

    let app = open("s03-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    for _ in 0..16 {
        app.key("]");
    }
    let entered = app.wait_text(TITLE);
    see(
        &entered,
        &[TITLE, BLURB, "#1040", CAP_PLAY],
        &[OVERVIEW, "Page options", "pending-change queue"],
    );
    app.key("]");
    let next = app.wait_text("pending-change queue");
    see(
        &next,
        &["Data grid", "Typed cells, a pending-change queue"],
        &[TITLE, BLURB],
    );
    app.key("[");
    let back = app.wait_text(TITLE);
    see(
        &back,
        &[TITLE, "#1040", BLURB],
        &["pending-change queue", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[TITLE, "#1040"]);
    let sidebar = text(&overview);
    if !sidebar.contains("Editable") {
        click_side(&app, "Tables");
        app.wait_text("Editable");
    }
    click_side(&app, "Editable");
    let pointed = app.wait_text(TITLE);
    see(
        &pointed,
        &[TITLE, BLURB, "#1040", CAP_PLAY],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "editable"];
    let app = open("s03-sections", 80, 24, &args);
    let narrow_play = app.wait_text(TITLE);
    see(
        &narrow_play,
        &[TITLE, "Playground", "#1040", TASK, "0 edits", RANGE, "mira"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "No tasks yet",
        ],
    );
    shot(
        &app,
        "sc.editable.playground.80x24",
        "playground at 80 by 24 before the terminal grows, with no section caption",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Rows",
            "tasks",
            "Type-ahead",
            "off",
            "focused",
            "Overflow marks on",
            "Numeric ID and Changes on",
            "Vim keys off",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "#1040",
            WIDE,
            "empty",
            "always",
            "Overflow marks off",
            "Vim keys on",
        ],
    );
    assert!(
        line_after(&narrow_opt, "Rows").contains("tasks"),
        "narrow Rows value changed\n{}",
        text(&narrow_opt)
    );
    assert!(
        !text(&narrow_opt).contains("Track click"),
        "80 by 24 options showed controls from below the fold\n{}",
        text(&narrow_opt)
    );
    shot(
        &app,
        "sc.editable.options.80x24",
        "options at 80 by 24 before the terminal grows, with the lower controls still below the fold",
        &args,
    );

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            "States",
            "○ Compare levels",
            "rest",
            "hover",
            "#1040",
            "focused sample",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            WIDE,
            "truecolor",
            "narrow",
            "No rows",
            "● Compare levels",
            "focus + hover",
        ],
    );
    shot(
        &app,
        "sc.editable.states.80x24",
        "states at 80 by 24 before the terminal grows, showing rest and clipping the later specimens",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_text(CAP_STATES);
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "editing",
            "error",
            "Add rate limiting to!",
            "… to auth endpoints",
            "Text cells",
            "Constrained (▾ always)",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "No rows",
            "narrow",
            "focus + hover",
            "busy",
            "loading",
        ],
    );
    assert!(
        !text(&wide_states).contains("selected"),
        "states grid drew a selected specimen\n{}",
        text(&wide_states)
    );
    shot(
        &app,
        "sc.editable.states.120x40",
        "states caption and the leading specimens at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.editable.states",
        "states is the visible Editable section",
        &args,
    );
    let top = [
        (
            "sc.editable.specimen.rest",
            "rest",
            "the rest specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.hover",
            "hover",
            "the hover specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.pressed",
            "pressed",
            "the pressed specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.focused",
            "focused",
            "the focused specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.editing",
            "editing",
            "the editing specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.error",
            "error",
            "the error specimen is fully visible at 120 by 40 and compare levels is off",
        ),
    ];
    for (case_id, label, precondition) in top {
        let screen = app.screen();
        let block = block_from(&lines_of(&screen), label, 4);
        assert!(
            block.contains("#1040") && block.contains("#1042"),
            "{label} specimen is clipped\n{block}"
        );
        see(
            &screen,
            &[label, "○ Compare levels", CAP_STATES],
            GRID_ABSENT,
        );
        shot(&app, case_id, precondition, &args);
    }

    let body = Point { x: 60, y: 20 };
    app.wheel(body, true, 3);
    app.move_to(Point { x: 2, y: 39 });
    app.settle();
    let paged = app.wait_text("No rows");
    let paged_lines = lines_of(&paged);
    let disabled = block_from(&paged_lines, "disabled", 4);
    assert!(
        disabled.contains("#1040") && disabled.contains("#1041") && disabled.contains("#1042"),
        "disabled specimen is clipped\n{disabled}"
    );
    let empty = block_from(&paged_lines, "empty", 2);
    assert!(
        empty.contains("No rows"),
        "empty specimen is clipped\n{empty}"
    );
    let overflow = block_from(&paged_lines, "overflow", 4);
    assert!(
        overflow.contains("#1040") && overflow.contains("#1042"),
        "overflow specimen is clipped\n{overflow}"
    );
    see(
        &paged,
        &["disabled", "empty", "overflow", "No rows"],
        &[
            "focus + hover",
            "busy",
            "loading",
            "● Compare levels",
            "Page options",
        ],
    );
    let lower = [
        (
            "sc.editable.specimen.disabled",
            "the disabled specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.empty",
            "the empty specimen shows No rows at 120 by 40 and compare levels is off",
        ),
        (
            "sc.editable.specimen.overflow",
            "the overflow specimen is fully visible at 120 by 40 and compare levels is off",
        ),
    ];
    for (case_id, precondition) in lower {
        shot(&app, case_id, precondition, &args);
    }

    app.wheel_until(body, true, 1, 4, "the narrow specimen", |screen| {
        narrow_block(&text(screen)).is_some()
    });
    app.move_to(Point { x: 2, y: 39 });
    let settled = app.settle();
    let block = narrow_block(&text(&settled))
        .unwrap_or_else(|| panic!("narrow specimen is clipped\n{}", text(&settled)));
    assert!(
        block.contains("narrow") && block.contains("#1040") && block.contains("#1041"),
        "narrow specimen missing a row\n{block}"
    );
    see(
        &settled,
        &["narrow", "Add rate limit"],
        &[
            "focus + hover",
            "busy",
            "loading",
            "● Compare levels",
            "Page options",
        ],
    );
    shot(
        &app,
        "sc.editable.specimen.narrow",
        "the narrow specimen is fully visible at 120 by 40 and compare levels is off",
        &args,
    );

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "Page options",
            "tasks",
            "off",
            "focused",
            "Overflow marks on",
            "Numeric ID and Changes on",
            "Vim keys off",
            "default",
            "auto",
            "Reveal cut cells on",
            "shell",
            "commit",
            "Header menu labels",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "#1040",
            "empty",
            "always",
            "Vim keys on",
            "custom",
            "jump",
            "cancel",
        ],
    );
    assert!(line_after(&wide_opt, "Rows").contains("tasks"));
    assert!(line_after(&wide_opt, "Type-ahead").contains("off"));
    assert!(line_after(&wide_opt, "▾ affordance").contains("focused"));
    assert!(line_after(&wide_opt, "Key map").contains("default"));
    assert!(line_after(&wide_opt, "Horizontal bar").contains("auto"));
    assert!(line_after(&wide_opt, "Track click").contains("shell"));
    assert!(line_after(&wide_opt, "On blur").contains("commit"));
    assert!(line_after(&wide_opt, "Header menu labels").contains("default"));
    shot(
        &app,
        "sc.editable.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.editable.options",
        "options is the visible Editable section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "#1040",
            TASK,
            "0 edits",
            RANGE,
            "fix/checkout flake",
            "!",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "No tasks yet",
            "Cell saved",
            "Edit cancelled",
        ],
    );
    shot(
        &app,
        "sc.editable.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.editable.playground",
        "playground is the visible Editable section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "editable"];
    let app = open("s03-compare", 120, 40, &args);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", CAP_STATES],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 12);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut bands = app.screen();
    for _ in 0..8 {
        if text(&bands).contains("no color") {
            break;
        }
        app.key("pagedown");
        bands = app.settle();
    }
    assert!(
        text(&bands).contains("no color"),
        "no color band stayed below the fold\n{}",
        text(&bands)
    );
    see(
        &bands,
        &[TITLE, "no color", WIDE, "Compare levels"],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.editable.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_text("● Compare levels");
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor", "rest"],
        &["Page options", CAP_PLAY, WIDE, "120×40", "○ Compare levels"],
    );
    shot(
        &app,
        "sc.editable.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &["● Compare levels", "no color", "Page options"],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    app.finish();
}

#[test]
fn cell_edit_shows_the_typed_character_before_commit() {
    let args = ["--page", "editable"];
    let app = open("s03-cells", 120, 40, &args);
    let before = app.wait_text(TASK);
    see(
        &before,
        &[TITLE, "0 edits", RANGE, "Enter Open"],
        &["Enter Apply", "Cell saved", "Edit cancelled", "endpointsQ"],
    );
    focus_task(&app);
    let focused = app.wait_text("Enter Edit");
    see(
        &focused,
        &["↑↓←→ Cell", "Enter Edit", "s Sort", "0 edits", TASK],
        &["Enter Apply", "Cell saved", "Edit cancelled"],
    );
    app.key("enter");
    let editing = app.wait_text("Enter Apply");
    see(
        &editing,
        &["Enter Apply", "Esc Cancel", "Tab Next cell", "0 edits"],
        &["Cell saved", "Edit cancelled", "1 edits"],
    );
    app.type_text("Q");
    let typed = app.wait_screen("typed Q before commit", |screen| {
        let visible = text(screen);
        visible.contains("endpointsQ")
            && visible.contains("Enter Apply")
            && visible.contains("0 edits")
            && visible.contains(RANGE)
            && visible.contains("#1040")
            && !visible.contains("Cell saved")
            && !visible.contains("1 edits")
    });
    see(
        &typed,
        &[
            "endpointsQ",
            "Enter Apply",
            "Esc Cancel",
            "0 edits",
            RANGE,
            "#1040",
        ],
        &["Cell saved", "Edit cancelled", "1 edits", "No tasks yet"],
    );
    // A same-size resize does not force a frame. Wait out the 140ms click
    // flash, then resize away and back so the settled edit is drawn.
    app.flash_expired();
    app.resize(121, 40);
    app.wait_text("121×40");
    app.resize(120, 40);
    let settled = app.wait_screen("settled typed cell", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains("endpointsQ")
            && visible.contains("Enter Apply")
            && visible.contains("0 edits")
            && visible.contains(RANGE)
            && visible.contains("#1040")
            && !visible.contains("Cell saved")
            && !visible.contains("1 edits")
    });
    see(
        &settled,
        &[
            "endpointsQ",
            "Enter Apply",
            "Esc Cancel",
            "0 edits",
            RANGE,
            "#1040",
        ],
        &["Cell saved", "Edit cancelled", "1 edits", "No tasks yet"],
    );
    shot(
        &app,
        "sc.editable.cells",
        "the task cell shows the typed Q while the edit is still open",
        &args,
    );
    app.key("esc");
    let cancelled = app.wait_text("Edit cancelled");
    see(
        &cancelled,
        &["Edit cancelled", "0 edits", TASK],
        &["endpointsQ", "Cell saved", "1 edits", "Enter Apply"],
    );
    app.finish();
}

#[test]
fn option_rows_empty() {
    let args = ["--page", "editable", "--opt", "editable.rows=empty"];
    let app = open("s03-rows", 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "Rows").contains("empty"));
    assert!(line_after(&options, "Type-ahead").contains("off"));
    see(
        &options,
        &["empty", "Overflow marks on", "Vim keys off", "commit"],
        &["always", "jump", "cancel", "custom"],
    );
    click_tab(&app, "Playground");
    let play = app.wait_text("No tasks yet");
    see(
        &play,
        &[
            TITLE,
            "No tasks yet",
            "Tasks you create show up here",
            "0 edits",
            CAP_PLAY,
        ],
        &["#1040", TASK, RANGE, "1 edits", "Cell saved"],
    );
    shot(
        &app,
        "opt.editable.rows",
        "rows empty draws No tasks yet and leaves the other options at their defaults",
        &args,
    );
    app.finish();
}

#[test]
fn option_type_ahead_task() {
    let args = ["--page", "editable", "--opt", "editable.type_ahead=task"];
    let app = open("s03-type", 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "Type-ahead").contains("task"));
    assert!(line_after(&options, "Rows").contains("tasks"));
    see(
        &options,
        &["task", "focused", "Overflow marks on"],
        &["empty", "always", "cancel"],
    );
    click_tab(&app, "Playground");
    focus_task(&app);
    app.key("w");
    let jumped = app.wait_screen("type-ahead on Write release", |screen| {
        lines_of(screen)
            .iter()
            .any(|line| line.contains("Write release notes for 3.2") && line.contains('▎'))
    });
    let row = row_with(&jumped, "Write release notes for 3.2");
    assert!(
        row.contains('▎'),
        "type-ahead did not move the cursor onto Write release: {row}"
    );
    let stayed = row_with(&jumped, "#1040");
    assert!(
        !stayed.contains('▎'),
        "type-ahead left the cursor on the first row: {stayed}"
    );
    see(
        &jumped,
        &["Write release notes for 3.2", "0 edits", "#1043"],
        &[
            "Add rate limiting to auth endpoints",
            "Cell saved",
            "1 edits",
        ],
    );
    shot_pressed(&app, "#1040", "opt.editable.type_ahead");
    app.finish();
}

#[test]
fn option_affordance_always() {
    let args = ["--page", "editable", "--opt", "editable.affordance=always"];
    let app = open("s03-affordance", 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "▾ affordance").contains("always"));
    assert!(line_after(&options, "Rows").contains("tasks"));
    see(
        &options,
        &["always", "Type-ahead", "off", "Overflow marks on"],
        &["empty", "focused", "cancel"],
    );
    click_tab(&app, "Playground");
    let play = app.wait_text("mira");
    let first = row_with(&play, "#1040");
    let second = row_with(&play, "#1041");
    assert!(
        first.contains('▾') && second.contains('▾'),
        "always affordance missed an owner chevron\n{first}\n{second}"
    );
    see(
        &play,
        &[TITLE, "#1040", "0 edits", RANGE, CAP_PLAY],
        &["No tasks yet", "Cell saved", "empty"],
    );
    shot(
        &app,
        "opt.editable.affordance",
        "affordance always draws an owner chevron on every row and leaves the other options at their defaults",
        &args,
    );
    app.finish();
}

#[test]
fn option_overflow_marks_off() {
    let args = ["--page", "editable", "--opt", "editable.overflow_marks=off"];
    let app = open("s03-marks", 80, 24, &args);
    let options = option_page(&app);
    see(
        &options,
        &["Overflow marks off", "tasks", "Numeric ID and Changes on"],
        &["Overflow marks on", "empty", "Vim keys on"],
    );
    click_tab(&app, "Playground");
    let play = app.wait_text("#1040");
    let header = header_row(&play);
    assert!(
        !header.contains('…'),
        "overflow marks still drew a header ellipsis: {header}"
    );
    assert!(
        header.contains("Owner"),
        "header lost Owner when marks were off: {header}"
    );
    see(
        &play,
        &[TITLE, "#1040", "0 edits", RANGE, "end…"],
        &["No tasks yet", "Cell saved"],
    );
    shot(
        &app,
        "opt.editable.overflow_marks",
        "overflow marks off leaves the header without an ellipsis at 80 by 24",
        &args,
    );
    app.finish();
}

#[test]
fn option_numeric_off_sorts_changes_as_text() {
    let args = ["--page", "editable", "--opt", "editable.numeric=off"];
    let app = open("s03-numeric", 120, 40, &args);
    let options = option_page(&app);
    see(
        &options,
        &[
            "Numeric ID and Changes off",
            "tasks",
            "Overflow marks on",
            "Vim keys off",
        ],
        &["Numeric ID and Changes on", "empty", "jump"],
    );
    click_tab(&app, "Playground");
    focus_task(&app);
    for _ in 0..4 {
        app.key("right");
    }
    app.key("s");
    // Four Rights already reveal 118. The ascending marker is painted on
    // the next frame, so the sorted header is the one that carries it.
    let sorted = app.wait_screen("changes sorted ascending", |screen| {
        text(screen).contains("118")
            && lines_of(screen)
                .iter()
                .any(|line| line.contains("Task") && line.contains("Owner") && line.contains('▴'))
    });
    see(
        &sorted,
        &[
            "Generate API",
            "118",
            "Write release notes for 3.2",
            "Chang",
            "0 edits",
        ],
        &["Cell saved", "1 edits", "No tasks yet"],
    );
    let header = header_row(&sorted);
    assert!(
        header.contains('▴'),
        "Changes did not sort ascending: {header}"
    );
    let held = hold_pressed(&app, "Write release notes for 3.2", "Add rate limiting");
    // The accepted cells are this held row. A hidden cursor parks on a
    // different cell of an otherwise equal frame, which `visual` rejects.
    let pressed = app.screen();
    check_frame(&app, &pressed, &visual_id("opt.editable.numeric"));
    app.up(held);
    app.finish();
}

#[test]
fn option_nav_vim_moves_down() {
    let args = ["--page", "editable", "--opt", "editable.nav=on"];
    let app = open("s03-nav", 120, 40, &args);
    let options = option_page(&app);
    see(
        &options,
        &[
            "Vim keys on",
            "tasks",
            "Type-ahead",
            "off",
            "Overflow marks on",
        ],
        &["Vim keys off", "empty", "always", "cancel"],
    );
    click_tab(&app, "Playground");
    focus_task(&app);
    app.key("j");
    let moved = app.wait_text("Migrate sessions table to UUID keys");
    let row = row_with(&moved, "#1041");
    assert!(row.contains('▎'), "vim j left the first row focused: {row}");
    see(
        &moved,
        &["Migrate sessions table to UUID keys", "0 edits", "#1041"],
        &[
            "Add rate limiting to auth endpoints",
            "Cell saved",
            "1 edits",
        ],
    );
    shot_pressed(&app, "#1040", "opt.editable.nav");
    app.finish();
}

#[test]
fn option_keymap_custom_sorts_with_o() {
    let args = ["--page", "editable", "--opt", "editable.keymap=custom"];
    let app = open("s03-keymap", 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "Key map").contains("custom"));
    assert!(line_after(&options, "Rows").contains("tasks"));
    see(
        &options,
        &["custom", "Overflow marks on", "Vim keys off"],
        &["empty", "jump", "cancel"],
    );
    click_tab(&app, "Playground");
    focus_task(&app);
    let hints = app.wait_text("o Sort");
    see(&hints, &["o Sort", "0 edits"], &["s Sort", "Cell saved"]);
    app.key("o");
    let sorted = app.wait_text("Task ▴");
    see(
        &sorted,
        &["Task ▴", "Add OpenTelemetry", "#1047", "o Sort", "0 edits"],
        &["Task ▾", "s Sort", "Cell saved", "Sort A to Z"],
    );
    app.key("s");
    let stayed = app.settle();
    see(
        &stayed,
        &["Task ▴", "Add OpenTelemetry", "o Sort"],
        &["Task ▾", "s Sort", "Sort A to Z", "Cell saved"],
    );
    shot_pressed(&app, "#1047", "opt.editable.keymap");
    app.finish();
}

#[test]
fn option_horizontal_never() {
    let args = ["--page", "editable", "--opt", "editable.horizontal=never"];
    let app = open("s03-horizontal", 80, 24, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "Horizontal bar").contains("never"));
    see(
        &options,
        &["never", "tasks", "Reveal cut cells on", "Overflow marks on"],
        &["empty", "Vim keys on", "Reveal cut cells off"],
    );
    click_tab(&app, "Playground");
    let play = app.wait_text("tracing spans");
    see(
        &play,
        &[TITLE, "tracing spans", "0 edits", "#1047", "auth endp…"],
        &[RANGE, "No tasks yet", "Cell saved", "1–13 of 14"],
    );
    shot(
        &app,
        "opt.editable.horizontal",
        "horizontal bar never drops the position label and shows the full tracing spans title at 80 by 24",
        &args,
    );
    app.finish();
}

#[test]
fn option_reveal_off_keeps_the_cut_cell() {
    let args = ["--page", "editable", "--opt", "editable.reveal=off"];
    let app = open("s03-reveal", 80, 24, &args);
    let options = option_page(&app);
    see(
        &options,
        &["Reveal cut cells off", "tasks", "Overflow marks on"],
        &["Reveal cut cells on", "empty", "Vim keys on"],
    );
    click_tab(&app, "Playground");
    let hit = task_point(&app);
    app.move_to(hit);
    let hovered = app.settle();
    let row = row_with(&hovered, "#1040");
    assert!(
        row.contains("mira") && row.contains("end…"),
        "reveal off expanded the cut cell: {row}"
    );
    see(
        &hovered,
        &[TITLE, "#1040", "0 edits", RANGE],
        &[
            "Add rate limiting to auth endpoints",
            "Cell saved",
            "No tasks yet",
        ],
    );
    shot(
        &app,
        "opt.editable.reveal",
        "reveal off keeps the hovered task truncated and the owner on the same row",
        &args,
    );
    app.finish();
}

#[test]
fn option_track_click_jump() {
    let args = ["--page", "editable", "--opt", "editable.track_click=jump"];
    let app = open("s03-track", 120, 40, &args);
    click_tab(&app, "Playground");
    let play = app.wait_text(RANGE);
    see(
        &play,
        &[TITLE, "#1040", TASK, "0 edits", RANGE, CAP_PLAY],
        &["No tasks yet", "Cell saved", "1 edits"],
    );
    let options = option_page(&app);
    assert!(line_after(&options, "Track click").contains("jump"));
    assert!(line_after(&options, "Rows").contains("tasks"));
    assert!(line_after(&options, "On blur").contains("commit"));
    see(
        &options,
        &["jump", "Overflow marks on", "Vim keys off"],
        &["empty", "cancel", "custom"],
    );
    shot(
        &app,
        "opt.editable.track_click",
        "track click jump is selected and the table still opens on the first rows",
        &args,
    );
    app.finish();
}

#[test]
fn option_blur_cancel_drops_the_edit() {
    let args = ["--page", "editable", "--opt", "editable.blur_action=cancel"];
    let app = open("s03-blur", 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "On blur").contains("cancel"));
    assert!(line_after(&options, "Rows").contains("tasks"));
    see(
        &options,
        &["cancel", "Overflow marks on", "Vim keys off"],
        &["empty", "jump", "custom"],
    );
    click_tab(&app, "Playground");
    focus_task(&app);
    app.key("enter");
    app.wait_text("Enter Apply");
    app.type_text("Q");
    app.wait_text("endpointsQ");
    click_tab(&app, "Options");
    let blurred = app.wait_text("Edit cancelled");
    see(
        &blurred,
        &["Edit cancelled", "cancel", "Page options", "tasks"],
        &["Cell saved", "1 edits", "endpointsQ"],
    );
    shot(
        &app,
        "opt.editable.blur_action",
        "blurring an open edit cancels it and the status says Edit cancelled",
        &args,
    );
    click_tab(&app, "Playground");
    let back = app.wait_text("0 edits");
    see(
        &back,
        &["0 edits", RANGE, TASK, "#1040"],
        &["1 edits", "endpointsQ", "Cell saved"],
    );
    app.finish();
}

#[test]
fn option_header_labels_custom_still_sorts_from_the_header() {
    let args = [
        "--page",
        "editable",
        "--opt",
        "editable.header_labels=custom",
    ];
    let app = open("s03-headers", 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "Header menu labels").contains("custom"));
    assert!(line_after(&options, "Rows").contains("tasks"));
    assert!(line_after(&options, "Key map").contains("default"));
    see(
        &options,
        &["custom", "Overflow marks on", "commit", "shell"],
        &["empty", "jump", "cancel", "Sort A to Z"],
    );
    shot(
        &app,
        "opt.editable.header_labels",
        "header menu labels custom is selected and the page does not draw a header menu",
        &args,
    );
    click_tab(&app, "Playground");
    click_where(&app, "Task", |hit| hit.y == 9 && hit.x > 24);
    let sorted = app.wait_text("Task ▴");
    see(
        &sorted,
        &["Task ▴", "Add OpenTelemetry", "#1047", "0 edits"],
        &[
            "Sort A to Z",
            "Sort Z to A",
            "Unsort",
            "Fit Width",
            "Cell saved",
        ],
    );
    app.finish();
}

/// Opens the walkthrough on the Options tab, then drives the last cell from the keyboard.
fn walkthrough(name: &str, value: &str) -> crate::support::App {
    let opt = format!("editable.tab_walkthrough={value}");
    let args = ["--page", "editable", "--opt", opt.as_str()];
    let app = open(name, 120, 40, &args);
    let options = option_page(&app);
    assert!(line_after(&options, "Tab at the last cell").contains(value));
    click_tab(&app, "Playground");
    let shown = app.wait_text(TASK);
    see(
        &shown,
        &["Try", "Shows", "last —", "0 edits"],
        &["Cell saved"],
    );
    for _ in 0..4 {
        if text(&app.settle()).contains("Enter Edit") {
            break;
        }
        app.key("tab");
    }
    app.wait_text("Enter Edit");
    app.key("enter");
    app.wait_text("Enter Apply");
    app
}

#[test]
fn option_tab_walkthrough_commit_then_leave() {
    let app = walkthrough("s03-tab-commit", "commit");
    let editing = app.screen();
    see(
        &editing,
        &["Tab Commit, then leave", "Enter, 7, Tab"],
        &["Cell saved"],
    );
    app.type_text("7");
    app.key("tab");
    let left = app.wait_text("Cell saved");
    see(
        &left,
        &[
            "last Committed → LeaveForward",
            "1 edits",
            "Cell saved",
            "Tab Into page",
        ],
        &["Enter Apply", "Edit cancelled", "last —"],
    );
    app.finish();
}

#[test]
fn option_tab_walkthrough_no_op_leaves_without_saving() {
    let app = walkthrough("s03-tab-noop", "no-op");
    see(
        &app.screen(),
        &["Tab Leave, nothing to save", "Enter, Tab"],
        &["Cell saved"],
    );
    app.key("tab");
    let left = app.wait_text("last LeaveForward");
    see(
        &left,
        &["last LeaveForward", "0 edits", "Tab Into page"],
        &["Committed", "Cell saved", "Enter Apply", "1 edits"],
    );
    app.finish();
}

#[test]
fn option_tab_walkthrough_invalid_blocks_the_move() {
    let app = walkthrough("s03-tab-invalid", "invalid");
    see(
        &app.screen(),
        &["Tab Blocked by an error", "Enter, x, Tab"],
        &["Cell saved"],
    );
    app.type_text("x");
    app.key("tab");
    let blocked = app.wait_text("Changes must be a whole number");
    see(
        &blocked,
        &[
            "Changes must be a whole number · last none",
            "Enter Apply",
            "Esc Cancel",
        ],
        &["Cell saved", "LeaveForward", "Committed", "Tab Into page"],
    );
    app.key("esc");
    let cancelled = app.wait_text("Edit cancelled");
    see(
        &cancelled,
        &["Edit cancelled", "last Cancelled", "0 edits"],
        &["Cell saved", "Enter Apply"],
    );
    app.finish();
}
