//! Text areas route, sections, options, and multiline editing, driven through the showcase PTY.
//!
//! Tuiscotti state ids allow only `[a-z0-9_-]` segments. Dotted case ids are stored as
//! `pre_refactor/s02_textareas/<case id with '.' replaced by '_'>`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Inputs & forms › Text areas";
const BLURB: &str = "Multi-line editing, Unicode cursor motion, horizontal and vertical scrolling";
const OVERVIEW: &str = "Tokens and principles behind every component";
const PLAY_CAPTION: &str = "the live widget";
const OPT_CAPTION: &str = "one control per option";
const STATE_CAPTION: &str = "every state, drawn by the widget";
const TOP: &str = "1–8 of 28";
const END: &str = "21–28 of 28";
const MID: &str = "13–20 of 28";
const TASK: &str = "1. Read src/api/billing.rs before touching invoices.";
const FIX: &str = "fix stuff";
const MOOD: &str = "Use the imperative mood and explain why";
const JUNIE: &str = "Junie: Reading 14 files";
const PLAN: &str = "Junie: Plan ready. 3 steps.";
const PLACEHOLDER: &str = "should know";
const EDITING: &str = "Newline";
const IDLE: &str = "Enter Edit";

fn launch(name: &str, args: &[&str], cols: u16, rows: u16) -> AppSession {
    let mut spec = Launch::new(name, SHOWCASE);
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.cols = cols;
    spec.rows = rows;
    spec.color = Some("truecolor".to_string());
    spec.motion = Some("paused".to_string());
    let app = try_launch(spec).unwrap_or_else(|error| panic!("{error}"));
    AppSession { app }
}

struct AppSession {
    app: crate::support::App,
}

fn open(name: &str, args: &[&str]) -> AppSession {
    let session = launch(name, args, 120, 40);
    session.app.wait_text(CRUMB);
    session
}

fn expect(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let raw = text(screen);
    for needle in present {
        assert!(
            raw.contains(needle),
            "visible result missing {needle:?}\n{raw}"
        );
    }
    for needle in absent {
        assert!(
            !raw.contains(needle),
            "forbidden effect still visible {needle:?}\n{raw}"
        );
    }
}

fn test_id(case_id: &str) -> String {
    format!("pre_refactor/s02_textareas/{}", case_id.replace('.', "_"))
}

fn see(session: &AppSession, case_id: &str, _precondition: &str) {
    session.app.visual(&test_id(case_id));
}

fn check_frame(app: &crate::support::App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
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
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

/// The first paint after keyboard navigation clips the commit message.
/// `fix stuff` is on the following paint.
fn wait_commit(app: &crate::support::App) -> TerminalSnapshot {
    app.wait_screen("commit message", |screen| {
        let raw = text(screen);
        raw.contains(TASK) && raw.contains(FIX) && raw.contains(JUNIE)
    })
}

/// The accepted label-hit frame still paints Playground with the press flash
/// and parks the cursor on the hint row. A later paint locks the released tab.
fn label_shot(screen: &TerminalSnapshot) -> bool {
    let raw = text(screen);
    if !raw.contains(PLACEHOLDER)
        || !raw.contains("Anything the agent")
        || !raw.contains(FIX)
        || !raw.contains(TOP)
        || raw.contains(EDITING)
        || raw.contains("ZZ")
    {
        return false;
    }
    let cursor = screen.cursor();
    if cursor.col() != 53 || cursor.row() != 39 || cursor.visible() {
        return false;
    }
    let Some(hit) = find_all(screen, "Playground")
        .into_iter()
        .find(|hit| hit.y <= 5)
    else {
        return false;
    };
    screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .is_some_and(|cell| cell.bg() == SnapshotColor::Rgb(0x4a, 0x4a, 0x52))
}

/// A press paints the gutter and the edited glyphs on the `#3a3a40` bold
/// flash fill. The shared tick drops that fill on its next run, within 400 ms
/// while motion is paused, so the settled frame keeps the glyphs on the
/// resting input fill.
fn focus_settled(
    screen: &TerminalSnapshot,
    row_matches: impl Fn(&str) -> bool,
    glyphs: &[&str],
) -> bool {
    let flash = SnapshotColor::Rgb(0x3a, 0x3a, 0x40);
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        if !row_matches(&line) {
            return false;
        }
        let Some(cells) = screen.row(row) else {
            return false;
        };
        glyphs.iter().all(|glyph| {
            cells
                .iter()
                .any(|cell| cell.text() == *glyph && cell.bg() != flash)
        })
    })
}

fn commit_q_focused(screen: &TerminalSnapshot) -> bool {
    focus_settled(
        screen,
        |line| line.contains('▎') && line.contains('Q') && !line.contains(FIX),
        &["▎", "Q"],
    )
}

fn task_z_focused(screen: &TerminalSnapshot) -> bool {
    focus_settled(
        screen,
        |line| line.contains('▎') && line.contains("Z 1. Read"),
        &["▎", "Z"],
    )
}

fn notes_zz_focused(screen: &TerminalSnapshot) -> bool {
    focus_settled(
        screen,
        |line| line.contains('▎') && line.contains("ZZ") && !line.contains("QQ"),
        &["▎", "Z"],
    )
}

fn click_filtered(session: &AppSession, needle: &str, pred: impl Fn(&Point) -> bool) {
    let screen = session.app.wait_text(needle);
    let hits: Vec<_> = find_all(&screen, needle)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {needle:?}, got {hits:?}\n{}",
        text(&screen)
    );
    session.app.click(hits[0]);
}

fn click_left(session: &AppSession, needle: &str) {
    click_filtered(session, needle, |hit| hit.x < 22);
}

fn click_tab(session: &AppSession, title: &str) {
    click_filtered(session, title, |hit| hit.y <= 5 && hit.x > 24);
}

fn click_body(session: &AppSession, needle: &str) {
    click_filtered(session, needle, |hit| hit.x > 24);
}

fn value_row(screen: &TerminalSnapshot, label: &str) -> String {
    let rows: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = rows
        .iter()
        .position(|row| row.contains(label))
        .unwrap_or_else(|| panic!("{label} missing\n{}", text(screen)));
    if rows[index].contains("▾") || rows[index].contains("●") || rows[index].contains("○") {
        return rows[index].clone();
    }
    rows.get(index + 1).cloned().unwrap_or_default()
}

fn assert_value(screen: &TerminalSnapshot, label: &str, value: &str, forbidden: &[&str]) {
    let shown = value_row(screen, label);
    assert!(
        shown.contains(value),
        "{label} shows {shown:?}, want {value}"
    );
    for needle in forbidden {
        assert!(
            !shown.contains(needle),
            "forbidden {needle} on the {label} row {shown:?}"
        );
    }
}

fn assert_defaults(screen: &TerminalSnapshot, changed_label: &str, changed_value: &str) {
    let checks = [
        (
            "Edit entry",
            "theme",
            &["explicit", "immediate", "type-to-edit"][..],
        ),
        (
            "Focus selects",
            "caret-end",
            &["select-all", "caret-start"][..],
        ),
        ("Esc reverts", "off", &["on"][..]),
        ("Field keys", "theme", &["emacs", "desktop"][..]),
        ("Text key map", "preset", &["custom"][..]),
        ("Scroll keys", "default", &["vim", "no-page"][..]),
        (
            "Pointer selects",
            "default",
            &["off", "no-drag", "by-word"][..],
        ),
        ("On blur", "commit", &["cancel", "keep"][..]),
        ("Horizontal scroll", "theme", &["off"][..]),
        ("Label click focuses", "on", &["off"][..]),
    ];
    for (label, value, forbidden) in checks {
        if label == changed_label {
            assert_value(screen, label, changed_value, &[]);
        } else {
            assert_value(screen, label, value, forbidden);
        }
    }
}

fn band(screen: &TerminalSnapshot, label: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains(label) && !line.contains("120×40") && !line.contains("Inspector")
    })
}

fn wheel_until(
    session: &AppSession,
    down: bool,
    pred: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    for _ in 0..30 {
        let screen = session.app.settle();
        if pred(&screen) {
            return screen;
        }
        session.app.wheel(Point { x: 50, y: 18 }, down, 2);
    }
    let screen = session.app.settle();
    panic!("wheel target missing\n{}", text(&screen));
}

fn focus_task_from_tab(session: &AppSession) {
    session.app.key("tab");
}

fn focus_task_from_sidebar(session: &AppSession) {
    session.app.key("tab");
    session.app.key("tab");
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let keys = launch("s02-textareas-route-keys", &[], 120, 40);
    let overview = keys.app.wait_text(OVERVIEW);
    expect(&overview, &[OVERVIEW, PLAY_CAPTION], &[CRUMB, FIX, TASK]);
    for _ in 0..9 {
        keys.app.key("]");
    }
    let entered = wait_commit(&keys.app);
    expect(
        &entered,
        &[CRUMB, BLURB, PLAY_CAPTION, TASK, FIX, TOP],
        &[
            OPT_CAPTION,
            STATE_CAPTION,
            "Page options",
            "payments-gateway",
        ],
    );
    keys.app.key("[");
    let inputs = keys.app.wait_text("payments-gateway");
    expect(
        &inputs,
        &["Inputs & forms › Inputs", "payments-gateway", "Owner email"],
        &[TASK, FIX, "Task description"],
    );
    keys.app.key("]");
    let again = wait_commit(&keys.app);
    expect(&again, &[CRUMB, TASK, FIX], &["payments-gateway"]);
    click_left(&keys, "Overview");
    let pointed_back = keys.app.wait_text(OVERVIEW);
    expect(&pointed_back, &[OVERVIEW], &[TASK, FIX]);
    click_left(&keys, "Inputs & forms");
    click_left(&keys, "Text areas");
    let pointed = wait_commit(&keys.app);
    expect(
        &pointed,
        &[CRUMB, PLAY_CAPTION, TASK, FIX, "1–8 of 28"],
        &[OPT_CAPTION, "Page options", STATE_CAPTION],
    );

    let page = open("s02-textareas-route-page", &["--page", "textareas"]);
    let screen = page.app.wait_text(FIX);
    expect(
        &screen,
        &[
            CRUMB,
            BLURB,
            PLAY_CAPTION,
            "Task description",
            TASK,
            FIX,
            JUNIE,
            MOOD,
            TOP,
        ],
        &[OPT_CAPTION, STATE_CAPTION, "Page options", "Compare levels"],
    );
    see(
        &page,
        "sc.textareas.route",
        "--page textareas opens the playground with the sidebar focus on Text areas",
    );
}

#[test]
fn sections_narrow_then_wide() {
    let session = launch("s02-textareas-sections", &["--page", "textareas"], 80, 24);
    let narrow_play = session.app.wait_text("Task description");
    expect(
        &narrow_play,
        &[CRUMB, "Task description", TASK, "Notes"],
        &[
            PLAY_CAPTION,
            OPT_CAPTION,
            STATE_CAPTION,
            "Page options",
            "Compare levels",
            "120×40",
            "payments-gateway",
        ],
    );
    see(
        &session,
        "sc.textareas.playground.80x24",
        "playground tab activated at 80 by 24, before the terminal grows",
    );

    click_tab(&session, "Options");
    let narrow_opt = session.app.wait_text("Page options");
    expect(
        &narrow_opt,
        &[
            CRUMB,
            "Page options",
            "Edit entry",
            "caret-end",
            "Esc reverts off",
        ],
        &[
            PLAY_CAPTION,
            OPT_CAPTION,
            STATE_CAPTION,
            "Compare levels",
            "120×40",
            "Task description",
        ],
    );
    see(
        &session,
        "sc.textareas.options.80x24",
        "options tab activated at 80 by 24, before the terminal grows",
    );

    click_tab(&session, "States");
    let narrow_states = session.app.wait_text("Compare levels");
    expect(
        &narrow_states,
        &[CRUMB, "○ Compare levels", "rest", "hover", "focused"],
        &[
            PLAY_CAPTION,
            OPT_CAPTION,
            STATE_CAPTION,
            "Page options",
            "120×40",
            "pressed",
            "busy",
            "loading",
            "focus + hover",
            "selected",
        ],
    );
    see(
        &session,
        "sc.textareas.states.80x24",
        "states tab activated at 80 by 24, before the terminal grows",
    );

    session.app.resize(120, 40);
    let wide_states = session.app.wait_text("Anything to add");
    expect(
        &wide_states,
        &[
            CRUMB,
            STATE_CAPTION,
            "○ Compare levels",
            "▏ depicts a focused sample",
            "rest",
            "hover",
            "focused",
            "editing",
            "error",
            "disabled",
            "empty",
            "Anything to add",
            "Explain why, in the imperative",
            "120×40",
        ],
        &[
            PLAY_CAPTION,
            OPT_CAPTION,
            "Page options",
            "pressed",
            "busy",
            "loading",
            "focus + hover",
            "selected",
            "overflow",
            "narrow",
        ],
    );
    see(
        &session,
        "sc.textareas.states.120x40",
        "states tab still active after growing from 80 by 24 to 120 by 40",
    );
    session.app.key("pagedown");
    let low = session.app.wait_text("overflow");
    expect(
        &low,
        &[
            "overflow",
            "narrow",
            "Read src/api/billing.rs first. and a l",
            "Read src/api/billin",
            "Keep the public API",
            STATE_CAPTION,
        ],
        &[
            "pressed",
            "busy",
            "loading",
            "focus + hover",
            "selected",
            "Page options",
        ],
    );

    click_tab(&session, "Options");
    let wide_opt = session.app.wait_text(OPT_CAPTION);
    expect(
        &wide_opt,
        &[
            CRUMB,
            OPT_CAPTION,
            "Page options",
            "120×40",
            "Label click focuses",
        ],
        &[PLAY_CAPTION, STATE_CAPTION, "Compare levels", TASK],
    );
    assert_defaults(&wide_opt, "Edit entry", "theme");
    see(
        &session,
        "sc.textareas.options.120x40",
        "options tab at 120 by 40 after the narrow capture",
    );

    click_tab(&session, "Playground");
    let wide_play = session.app.wait_text(PLAY_CAPTION);
    expect(
        &wide_play,
        &[CRUMB, PLAY_CAPTION, TASK, FIX, JUNIE, MOOD, TOP, "120×40"],
        &[OPT_CAPTION, STATE_CAPTION, "Page options", "Compare levels"],
    );
    see(
        &session,
        "sc.textareas.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
    );
}

#[test]
fn compare_levels_then_restore() {
    let session = open("s02-textareas-compare", &["--page", "textareas"]);
    click_tab(&session, "States");
    let off = session.app.wait_text("○ Compare levels");
    expect(
        &off,
        &["○ Compare levels", CRUMB, "rest"],
        &["Page options", "256 colors", "● Compare levels"],
    );
    click_filtered(&session, "Compare levels", |hit| hit.y < 16);
    let on = session.app.wait_text("● Compare levels");
    expect(
        &on,
        &["● Compare levels", CRUMB],
        &["○ Compare levels", "Page options"],
    );

    let mut saw_true = false;
    let mut saw_16 = false;
    let mut captured = false;
    for _ in 0..24 {
        let screen = session.app.settle();
        let visible = text(&screen);
        if band(&screen, "truecolor") {
            saw_true = true;
        }
        if band(&screen, "16 colors") {
            saw_16 = true;
        }
        if !captured && visible.contains("256 colors") {
            expect(
                &screen,
                &["256 colors", CRUMB, "120×40", STATE_CAPTION],
                &["Page options", "○ Compare levels"],
            );
            see(
                &session,
                "sc.textareas.compare-levels.120x40",
                "compare levels on at 120 by 40 with the 256-color band visible",
            );
            captured = true;
        }
        if visible.contains("no color") {
            break;
        }
        session.app.wheel(Point { x: 50, y: 18 }, true, 2);
    }
    assert!(captured, "256-color band never became visible");
    assert!(saw_true, "truecolor band never became visible");
    assert!(saw_16, "16-color band never became visible");
    let wide_end = session.app.settle();
    expect(
        &wide_end,
        &["no color", CRUMB],
        &["Page options", "○ Compare levels"],
    );

    session.app.resize(80, 24);
    session.app.wait_absent("120×40");
    let narrow = wheel_until(&session, true, |screen| text(screen).contains("no color"));
    expect(
        &narrow,
        &["no color", CRUMB],
        &["Page options", "120×40", "○ Compare levels"],
    );
    // The hidden cursor sits on the resize clamp until the text area places
    // it. The accepted narrow frame has that caret at column 60.
    session.app.wait_screen("narrow caret", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains("no color")
            && screen.cursor_local() == Some((60, 20))
    });
    see(
        &session,
        "sc.textareas.compare-levels.80x24",
        "compare levels still on after narrowing to 80 by 24, before restoring 120 by 40",
    );

    wheel_until(&session, false, |screen| {
        text(screen).contains("Compare levels")
    });
    click_filtered(&session, "Compare levels", |hit| hit.y < 16);
    let restored = session.app.wait_text("○ Compare levels");
    expect(
        &restored,
        &["○ Compare levels", CRUMB],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    session.app.resize(120, 40);
    session.app.wait_text("Anything to add");
    click_tab(&session, "Options");
    let options = session.app.wait_text("Page options");
    assert_defaults(&options, "Edit entry", "theme");
    expect(
        &options,
        &[OPT_CAPTION, "120×40"],
        &[PLAY_CAPTION, "no color"],
    );
}

#[test]
fn default_keys_scroll_and_field() {
    let session = open("s02-textareas-defaults", &["--page", "textareas"]);
    focus_task_from_sidebar(&session);
    let editing = session.app.wait_text("ln 28/28");
    expect(
        &editing,
        &[EDITING, "ln 28/28", "Unicode", FIX, CRUMB],
        &[IDLE, TOP, "Z 1. Read"],
    );
    session.app.key("end");
    let ended = session.app.wait_text("Unicode");
    expect(&ended, &["Unicode", "ln 28/28", EDITING], &[TOP, IDLE]);
    session.app.key("escape");
    let parked = session.app.wait_text(END);
    expect(&parked, &[END, IDLE, "Saved"], &[EDITING, TOP]);
    session.app.key("pageup");
    let paged = session.app.wait_text(MID);
    expect(&paged, &[MID, IDLE], &[END, EDITING, "no-page"]);

    click_body(&session, "Commit message");
    session.app.key("ctrl+a");
    session.app.type_text("Q");
    let emacs = session.app.wait_text("Qfix stuff");
    expect(
        &emacs,
        &["Qfix stuff", EDITING],
        &["fix stuffQ", "Qfix stuffQ"],
    );
    click_body(&session, "Notes");
    session.app.type_text("ab");
    session.app.key("alt+a");
    session.app.type_text("Z");
    let preset = session.app.wait_text("abZ");
    expect(
        &preset,
        &["abZ", EDITING, "Qfix stuff"],
        &[PLACEHOLDER, "Anything the agent"],
    );
}

#[test]
fn esc_reverts_from_the_options_control() {
    let session = open("s02-textareas-esc", &["--page", "textareas"]);
    click_body(&session, "Notes");
    session.app.type_text("ZZ");
    session.app.key("escape");
    let kept = session.app.wait_text("Saved");
    expect(
        &kept,
        &["ZZ", "Saved", IDLE, FIX],
        &[PLACEHOLDER, EDITING, "QQ"],
    );
    click_tab(&session, "Options");
    let before = session.app.wait_text("Esc reverts off");
    assert_defaults(&before, "Edit entry", "theme");
    click_body(&session, "Esc reverts");
    let toggled = session.app.wait_text("Esc reverts on");
    assert_defaults(&toggled, "Esc reverts", "on");
    expect(
        &toggled,
        &["Esc reverts on", OPT_CAPTION],
        &["Esc reverts off", PLAY_CAPTION],
    );
    click_tab(&session, "Playground");
    click_body(&session, "Notes");
    session.app.type_text("QQ");
    let dirty = session.app.wait_text("ZZQQ");
    expect(&dirty, &["ZZQQ", EDITING], &[IDLE]);
    // Click once more and Esc. The press flash expires on the next tick, so
    // the settled frame has the committed ZZ on the resting fill.
    click_body(&session, "Notes");
    session.app.key("escape");
    let reverted = session.app.wait_text(IDLE);
    expect(
        &reverted,
        &["ZZ", IDLE, FIX, CRUMB],
        &["QQ", "ZZQQ", EDITING, PLACEHOLDER],
    );
    session.app.wait_screen("notes settled", notes_zz_focused);
    see(
        &session,
        "opt.textareas.esc_reverts",
        "Esc reverts toggled on from the Options control drops QQ and keeps the committed ZZ",
    );
}

#[test]
fn edit_entry_explicit() {
    let session = open(
        "s02-textareas-explicit",
        &[
            "--page",
            "textareas",
            "--opt",
            "textareas.edit_entry=explicit",
        ],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("explicit");
    assert_defaults(&options, "Edit entry", "explicit");
    click_tab(&session, "Playground");
    session.app.wait_text("Task description");
    focus_task_from_tab(&session);
    let idle = session.app.settle();
    expect(&idle, &[IDLE, TOP, TASK, FIX], &[EDITING, "Z 1. Read"]);
    session.app.type_text("Q");
    let ignored = session.app.settle();
    expect(
        &ignored,
        &[IDLE, TOP, TASK, FIX],
        &[EDITING, "Q1.", "Z 1. Read"],
    );
    let task_row = (0..ignored.rows())
        .map(|row| row_text(&ignored, row))
        .find(|row| row.contains("1. Read src/api/billing.rs"))
        .expect("task row");
    assert!(
        !task_row.contains('Q'),
        "explicit entry inserted Q before Enter: {task_row}"
    );
    session.app.key("enter");
    let started = session.app.wait_text("ln 1/28");
    expect(
        &started,
        &[EDITING, "ln 1/28", TASK],
        &[IDLE, TOP, "Z 1. Read"],
    );
    session.app.type_text("Z");
    let inserted = session.app.wait_text("Z 1. Read src/api/billing.rs");
    expect(
        &inserted,
        &["Z 1. Read src/api/billing.rs", EDITING, FIX, "ln 1/28"],
        &[IDLE, TOP, "fix stuffZ", "Q1."],
    );
    // The label click does not move the cursor. The press flash drops on the
    // next tick, so the settled frame keeps Z on the resting fill.
    click_body(&session, "Task description");
    session.app.wait_screen("task settled", task_z_focused);
    see(
        &session,
        "opt.textareas.edit_entry",
        "textareas.edit_entry=explicit ignores Q until Enter, then inserts Z at the start of the task",
    );
}

#[test]
fn focus_select_select_all() {
    let session = open(
        "s02-textareas-select-all",
        &[
            "--page",
            "textareas",
            "--opt",
            "textareas.focus_select=select-all",
        ],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("select-all");
    assert_defaults(&options, "Focus selects", "select-all");
    click_tab(&session, "Playground");
    click_body(&session, "Commit message");
    session.app.type_text("Q");
    let replaced = session.app.wait_absent(FIX);
    expect(
        &replaced,
        &[EDITING, "ln 1/1", CRUMB, "Q"],
        &[FIX, "stuff", "Qfix", "fix stuffQ", MOOD],
    );
    see(
        &session,
        "opt.textareas.focus_select",
        "textareas.focus_select=select-all replaces the commit message with Q",
    );
}

#[test]
fn field_keys_desktop() {
    let session = open(
        "s02-textareas-desktop",
        &[
            "--page",
            "textareas",
            "--opt",
            "textareas.field_keys=desktop",
        ],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("desktop");
    assert_defaults(&options, "Field keys", "desktop");
    click_tab(&session, "Playground");
    click_body(&session, "Commit message");
    session.app.key("ctrl+a");
    session.app.type_text("Q");
    let replaced = session.app.wait_absent(FIX);
    expect(
        &replaced,
        &[EDITING, "ln 1/1", "Q"],
        &[FIX, "stuff", "Qfix stuff", "fix stuffQ", MOOD],
    );
    session.app.wait_screen("commit settled", commit_q_focused);
    see(
        &session,
        "opt.textareas.field_keys",
        "textareas.field_keys=desktop selects the commit message on ctrl+a, so Q replaces it",
    );
}

#[test]
fn text_keymap_custom() {
    let session = open(
        "s02-textareas-custom",
        &[
            "--page",
            "textareas",
            "--opt",
            "textareas.text_keymap=custom",
        ],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("custom");
    assert_defaults(&options, "Text key map", "custom");
    click_tab(&session, "Playground");
    click_body(&session, "Notes");
    session.app.type_text("ab");
    session.app.key("alt+a");
    session.app.type_text("Z");
    let replaced = session.app.wait_text("Z");
    expect(
        &replaced,
        &[EDITING, CRUMB, FIX],
        &["abZ", PLACEHOLDER, "Anything the agent"],
    );
    let notes = (0..replaced.rows())
        .map(|row| row_text(&replaced, row))
        .find(|row| row.contains('Z'))
        .expect("Z row");
    assert!(
        notes.contains('Z') && !notes.contains("ab"),
        "custom alt+a left {notes:?}"
    );
    see(
        &session,
        "opt.textareas.text_keymap",
        "textareas.text_keymap=custom selects all on alt+a, so Z replaces ab",
    );
}

#[test]
fn nav_no_page() {
    let session = open(
        "s02-textareas-nopage",
        &["--page", "textareas", "--opt", "textareas.nav=no-page"],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("no-page");
    assert_defaults(&options, "Scroll keys", "no-page");
    click_tab(&session, "Playground");
    focus_task_from_tab(&session);
    let editing = session.app.wait_text("ln 28/28");
    expect(&editing, &["ln 28/28", EDITING], &[TOP, IDLE]);
    session.app.key("escape");
    let parked = session.app.wait_text(END);
    expect(&parked, &[END, IDLE], &[EDITING, MID]);
    session.app.key("pageup");
    let held = session.app.settle();
    expect(&held, &[END, IDLE, CRUMB], &[MID, TOP, EDITING, "vim"]);
    // The accepted Playground tab is the press color. A click's flash lasts
    // 140ms and the Page Up paint can land after it. Holding the button keeps
    // that color. The tab part is not a focus stop, so the list keeps its gutter.
    let tabs: Vec<_> = find_all(&held, "Playground")
        .into_iter()
        .filter(|hit| hit.y <= 5 && hit.x > 24)
        .collect();
    assert_eq!(
        tabs.len(),
        1,
        "expected one Playground tab, got {tabs:?}\n{}",
        text(&held)
    );
    session.app.down(tabs[0]);
    see(
        &session,
        "opt.textareas.nav",
        "textareas.nav=no-page leaves the task list on 21–28 of 28 after Page Up",
    );
}

#[test]
fn pointer_select_off() {
    let defaults = open("s02-textareas-drag", &["--page", "textareas"]);
    let screen = defaults.app.wait_text(FIX);
    let origin = find_all(&screen, FIX)
        .into_iter()
        .find(|hit| hit.x > 24)
        .expect("fix stuff");
    let end = Point {
        x: origin.x + 8,
        y: origin.y,
    };
    defaults.app.down(origin);
    defaults.app.drag_to(end);
    defaults.app.up(end);
    defaults.app.type_text("Q");
    let replaced = defaults.app.wait_text("Qf");
    expect(
        &replaced,
        &[EDITING, "ln 1/1", "Qf"],
        &[FIX, "stuf", "fix stufQf", "Qfix", MOOD],
    );

    let session = open(
        "s02-textareas-drag-off",
        &[
            "--page",
            "textareas",
            "--opt",
            "textareas.pointer_select=off",
        ],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("Pointer selects");
    assert_defaults(&options, "Pointer selects", "off");
    click_tab(&session, "Playground");
    let screen = session.app.wait_text(FIX);
    let origin = find_all(&screen, FIX)
        .into_iter()
        .find(|hit| hit.x > 24)
        .expect("fix stuff");
    let end = Point {
        x: origin.x + 8,
        y: origin.y,
    };
    session.app.down(origin);
    session.app.drag_to(end);
    session.app.up(end);
    session.app.type_text("Q");
    let inserted = session.app.wait_text("fix stufQf");
    expect(
        &inserted,
        &["fix stufQf", EDITING],
        &[FIX, "Qfix stuff", MOOD],
    );
    see(
        &session,
        "opt.textareas.pointer_select",
        "textareas.pointer_select=off inserts Q inside the dragged commit message as fix stufQf",
    );
}

#[test]
fn blur_action_cancel() {
    let defaults = open("s02-textareas-blur", &["--page", "textareas"]);
    click_body(&defaults, "Notes");
    defaults.app.type_text("ZZ");
    click_body(&defaults, "Commit message");
    let committed = defaults.app.wait_text("Saved");
    expect(
        &committed,
        &["ZZ", "Saved", FIX, EDITING],
        &[PLACEHOLDER, "Anything the agent"],
    );

    let session = open(
        "s02-textareas-blur-cancel",
        &[
            "--page",
            "textareas",
            "--opt",
            "textareas.blur_action=cancel",
        ],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("cancel");
    assert_defaults(&options, "On blur", "cancel");
    click_tab(&session, "Playground");
    click_body(&session, "Notes");
    session.app.type_text("ZZ");
    let dirty = session.app.wait_text("ZZ");
    expect(&dirty, &["ZZ", EDITING], &[PLACEHOLDER]);
    click_body(&session, "Commit message");
    let cancelled = session.app.wait_absent("ZZ");
    expect(
        &cancelled,
        &[FIX, EDITING, PLACEHOLDER, "Anything the agent", CRUMB],
        &["ZZ", "Saved", "ZZfix"],
    );
    see(
        &session,
        "opt.textareas.blur_action",
        "textareas.blur_action=cancel drops ZZ when focus leaves Notes and keeps fix stuff",
    );
}

#[test]
fn horizontal_off() {
    let session = open(
        "s02-textareas-horizontal",
        &["--page", "textareas", "--opt", "textareas.horizontal=off"],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("Horizontal scroll");
    assert_defaults(&options, "Horizontal scroll", "off");
    expect(
        &options,
        &["Horizontal scroll", OPT_CAPTION, "120×40"],
        &[PLAY_CAPTION],
    );
    see(
        &session,
        "opt.textareas.horizontal",
        "textareas.horizontal=off shows off on Horizontal scroll while Edit entry stays theme",
    );
    click_tab(&session, "Playground");
    session.app.wait_text("Task description");
    focus_task_from_tab(&session);
    session.app.key("end");
    let ended = session.app.wait_text("ln 28/28");
    expect(&ended, &[EDITING, "ln 28/28", CRUMB], &[IDLE, TOP]);
}

#[test]
fn label_hit_off() {
    let session = open(
        "s02-textareas-label",
        &["--page", "textareas", "--opt", "textareas.label_hit=off"],
    );
    click_tab(&session, "Options");
    let options = session.app.wait_text("Label click focuses");
    assert_defaults(&options, "Label click focuses", "off");
    click_tab(&session, "Playground");
    session.app.key("escape");
    let playground = session.app.wait_text("Notes");
    let tab = find_all(&playground, "Playground")
        .into_iter()
        .find(|hit| hit.y <= 5)
        .expect("Playground tab");
    let notes = find_all(&playground, "Notes")
        .into_iter()
        .find(|hit| hit.x > 24)
        .expect("Notes label");
    let mut ignored = None;
    for _ in 0..6 {
        session.app.click(tab);
        session.app.key("escape");
        session.app.click(notes);
        let started = Instant::now();
        let mut since: Option<Instant> = None;
        while started.elapsed() < Duration::from_millis(700) {
            let screen = session.app.screen();
            if label_shot(&screen) {
                let held = since.get_or_insert_with(Instant::now);
                if held.elapsed() >= Duration::from_millis(300) {
                    ignored = Some(screen);
                    break;
                }
            } else {
                since = None;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if ignored.is_some() {
            break;
        }
    }
    let ignored = ignored.unwrap_or_else(|| {
        panic!(
            "Notes click did not keep the Playground press flash\n{}",
            text(&session.app.screen())
        )
    });
    expect(
        &ignored,
        &[PLACEHOLDER, "Anything the agent", FIX, TOP],
        &[EDITING, "ZZ"],
    );
    check_frame(&session.app, &ignored, &test_id("opt.textareas.label_hit"));
    click_body(&session, PLACEHOLDER);
    let editing = session.app.wait_text(EDITING);
    expect(&editing, &[EDITING, PLACEHOLDER], &[IDLE, "ZZ"]);
}

#[test]
fn multiline_commit_cancel_and_readonly() {
    let session = open("s02-textareas-multiline", &["--page", "textareas"]);
    let before = session.app.wait_text(FIX);
    expect(
        &before,
        &[FIX, JUNIE, PLAN, PLACEHOLDER, TOP, "Tab Into page"],
        &["alpha", "beta", "gamma", "CANCELME", EDITING, IDLE],
    );
    see(
        &session,
        "sc.textareas.multiline.before",
        "multiline playground before paste, with the original commit message and read-only transcript",
    );

    click_body(&session, "Notes");
    session.app.paste("alpha\nbeta\ngamma");
    let pasted = session.app.wait_text("gamma");
    expect(
        &pasted,
        &["alpha", "beta", "gamma", EDITING, JUNIE, PLAN],
        &[PLACEHOLDER, "Anything the agent", "CANCELME"],
    );
    session.app.key("shift+up");
    let selected = session.app.settle();
    expect(
        &selected,
        &["alpha", "beta", "gamma", EDITING, JUNIE, PLAN],
        &[PLACEHOLDER, "CANCELME"],
    );
    session.app.key("escape");
    let committed = session.app.wait_text("Saved");
    expect(
        &committed,
        &["alpha", "beta", "gamma", "Saved", IDLE, JUNIE, PLAN],
        &[EDITING, PLACEHOLDER, "CANCELME"],
    );

    click_tab(&session, "Options");
    click_body(&session, "Esc reverts");
    let toggled = session.app.wait_text("Esc reverts on");
    assert_value(&toggled, "Esc reverts", "on", &["off"]);
    assert_value(&toggled, "Edit entry", "theme", &["explicit"]);
    assert_value(&toggled, "On blur", "commit", &["cancel"]);
    click_tab(&session, "Playground");
    click_body(&session, "Notes");
    session.app.type_text("CANCELME");
    let dirty = session.app.wait_text("CANCELME");
    expect(&dirty, &["CANCELME", "alpha", EDITING], &[IDLE]);
    session.app.key("escape");
    let reverted = session.app.wait_text(IDLE);
    expect(
        &reverted,
        &["alpha", "beta", "gamma", IDLE, JUNIE, PLAN, FIX],
        &["CANCELME", EDITING, PLACEHOLDER],
    );

    click_body(&session, "Junie: Reading");
    session.app.type_text("NO");
    let readonly = session.app.settle();
    let transcript = (0..readonly.rows())
        .map(|row| row_text(&readonly, row))
        .find(|row| row.contains("Junie: Reading"))
        .expect("transcript");
    assert!(
        !transcript.contains("NO"),
        "read-only transcript changed: {transcript}"
    );
    expect(
        &readonly,
        &[JUNIE, PLAN, "alpha", FIX],
        &["NOJunie", "CANCELME"],
    );
    session.app.key("tab");
    let tabbed = session.app.wait_text(EDITING);
    expect(
        &tabbed,
        &[EDITING, FIX, JUNIE, PLAN, "alpha"],
        &["NOJunie", "CANCELME", "fix stuffNO"],
    );
    click_body(&session, "Task description");
    let scrolled = session.app.wait_text("ln 28/28");
    expect(
        &scrolled,
        &["ln 28/28", EDITING, "alpha", JUNIE, PLAN, FIX],
        &[TOP, "CANCELME", "NOJunie"],
    );
    see(
        &session,
        "sc.textareas.multiline.after",
        "committed paste stays, cancelled CANCELME reverts, and the read-only transcript is unchanged",
    );
}
