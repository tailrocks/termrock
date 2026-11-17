//! Quit & leave route, sections, specimens, compare levels, options, and guard.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s01_quitleave/` and each case id's dots become underscores.

use crate::support::{App, Launch, SHOWCASE, find_all, row_text, text, try_launch};

const BLURB: &str = "QuitKeys, the leave guard";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const TITLE: &str = "Q3 settlement report";
const WIDE: &str = "120×40";

const SHOWN: &[(&str, &str)] = &[
    ("sc.quitleave.specimen.rest", "rest"),
    ("sc.quitleave.specimen.hover", "hover"),
    ("sc.quitleave.specimen.pressed", "pressed"),
    ("sc.quitleave.specimen.focused", "focused"),
    ("sc.quitleave.specimen.disabled", "disabled"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s01_quitleave/{}", case_id.replace('.', "_"))
}

fn fixture(args: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = args.iter().copied().map(str::to_string).collect();
    out.extend(["--color", "truecolor", "--motion", "paused"].map(str::to_string));
    out
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Quit & leave");
    app
}

fn require(app: &App, present: &[&str], absent: &[&str]) {
    let visible = text(&app.settle());
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

fn click_first(app: &App, needle: &str) {
    let screen = app.settle();
    let hits = find_all(&screen, needle);
    assert!(
        !hits.is_empty(),
        "no click target {needle:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_row(app: &App, needle: &str, row_has: &str) {
    let screen = app.settle();
    let point = find_all(&screen, needle)
        .into_iter()
        .find(|point| row_text(&screen, u32::from(point.y)).contains(row_has))
        .unwrap_or_else(|| {
            panic!(
                "no {needle:?} on a row containing {row_has:?}\n{}",
                text(&screen)
            )
        });
    app.click(point);
}

fn shot(app: &App, case_id: &str, _precondition: &str, _fixture_args: &[String]) {
    app.visual(&visual_id(case_id));
}

fn size(app: &App, cols: u32, rows: u32) {
    let screen = app.screen();
    assert_eq!(screen.cols(), cols, "terminal columns");
    assert_eq!(screen.rows(), rows, "terminal rows");
}

fn line<'a>(visible: &'a str, label: &str) -> &'a str {
    visible
        .lines()
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing line {label:?}\n{visible}"))
}

fn next_line<'a>(visible: &'a str, label: &str) -> &'a str {
    let lines: Vec<_> = visible.lines().collect();
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label:?}\n{visible}"));
    lines[index + 1]
}

fn footer(visible: &str) -> &str {
    visible
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(visible)
}

fn tokens(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

fn classify<'a>(visible: &'a str, label: &str) -> Vec<&'a str> {
    tokens(line(visible, label))
}

#[test]
fn route_opens_by_page_keyboard_and_pointer() {
    let args = ["--page", "quitleave"];
    let app = open("s01-route-page", 120, 40, &args);
    require(
        &app,
        &[
            "App shell › Quit & leave",
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            TITLE,
            "Leave for Overview",
            "Run export",
            "Edits",
            "Clean",
            "▎",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Tokens and principles",
            "KeyReference",
            "Ada Lovelace",
            "Unsaved ·",
            "Stop export",
        ],
    );
    size(&app, 120, 40);
    shot(
        &app,
        "sc.quitleave.route",
        "--page quitleave opens the playground with the sidebar focused",
        &fixture(&args),
    );
    app.key("]");
    require(
        &app,
        &[
            "App shell › Focus & navigation",
            "Ada Lovelace",
            "Focus ring",
        ],
        &[TITLE, "Leave for Overview", "QuitKeys, the leave guard"],
    );
    app.key("[");
    require(
        &app,
        &["App shell › Quit & leave", TITLE, "Run export", CAP_PLAY],
        &["Ada Lovelace", "KeyReference", CAP_OPT],
    );
    app.key("[");
    require(
        &app,
        &["App shell › Help & keys", "KeyReference"],
        &[TITLE, "Leave for Overview", "QuitKeys, the leave guard"],
    );
    app.key("]");
    require(
        &app,
        &["App shell › Quit & leave", TITLE, "Clean", "Run export"],
        &["KeyReference", "Ada Lovelace", "Page options"],
    );
    app.finish();

    let app = open("s01-route-pointer", 120, 40, &["--page", "helpkeys"]);
    require(&app, &["KeyReference", "Help & keys"], &[TITLE]);
    click_first(&app, "Quit & leave");
    require(
        &app,
        &["App shell › Quit & leave", TITLE, CAP_PLAY, "Run export"],
        &["KeyReference", "Tokens and principles", CAP_OPT],
    );
    click_first(&app, "Overview");
    require(
        &app,
        &["Tokens and principles", "Overview", CAP_PLAY],
        &[TITLE, "Leave for Overview", "QuitKeys, the leave guard"],
    );
    click_first(&app, "App shell");
    click_first(&app, "Quit & leave");
    require(
        &app,
        &[
            "App shell › Quit & leave",
            TITLE,
            "Clean",
            "Run export",
            WIDE,
        ],
        &[
            "Tokens and principles",
            "KeyReference",
            "Ada Lovelace",
            "Page options",
        ],
    );
    app.finish();
}

#[test]
fn playground_sections_at_both_sizes() {
    let args = ["--page", "quitleave"];
    let fixed = fixture(&args);
    let app = open("s01-playground", 80, 24, &args);
    require(
        &app,
        &[
            "App shell › Quit & leave",
            BLURB,
            "Playground",
            TITLE,
            "Leave for Overview",
            "Run export",
            "Edits",
            "Clean",
            "save: Cancel Discard Save",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            WIDE,
            "Page options",
            "Compare levels",
            "Stop export",
            "Unsaved ·",
            "no save:",
        ],
    );
    size(&app, 80, 24);
    shot(
        &app,
        "sc.quitleave.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &fixed,
    );
    app.resize(120, 40);
    require(
        &app,
        &[
            CAP_PLAY,
            WIDE,
            TITLE,
            "QuitKeys::classify",
            "clean",
            "Run export",
        ],
        &[CAP_OPT, CAP_STATES, "Page options", "Compare levels"],
    );
    size(&app, 120, 40);
    app.key("tab");
    require(
        &app,
        &[CAP_PLAY, "▎Playground", TITLE],
        &[CAP_OPT, CAP_STATES],
    );
    app.key("2");
    require(
        &app,
        &[
            CAP_OPT,
            "Page options",
            "Stakes at start",
            "Guard can save on",
            "Quit letter",
            "Esc quits off",
            "Ctrl+C quits on",
            "Ctrl+Q quits on",
        ],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "running", "none"],
    );
    app.key("3");
    require(
        &app,
        &[
            CAP_STATES,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "disabled",
            "Cancel",
            "Discard",
            "Save",
            "Stay",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "●",
            "Page options",
            "focus + hover",
            "busy",
            "editing",
            "empty",
            "loading",
        ],
    );
    app.key("1");
    require(
        &app,
        &[CAP_PLAY, WIDE, "▎Playground", TITLE, "QuitKeys::classify"],
        &[CAP_OPT, CAP_STATES, "Page options", "Compare levels"],
    );
    shot(
        &app,
        "sc.quitleave.playground.120x40",
        "playground tab focused at 120 by 40 after keyboard section jumps",
        &fixed,
    );
    shot(
        &app,
        "sc.quitleave.playground",
        "playground is the visible Quit & leave section",
        &fixed,
    );
    app.finish();
}

#[test]
fn options_section_at_both_sizes() {
    let args = ["--page", "quitleave"];
    let fixed = fixture(&args);
    let app = open("s01-options", 80, 24, &args);
    click_first(&app, "Options");
    let visible = text(&app.settle());
    require(
        &app,
        &[
            "App shell › Quit & leave",
            "Page options",
            "Stakes at start",
            "Guard can save on",
            "Quit letter",
            "Esc quits off",
            "Ctrl+C quits on",
            "Ctrl+Q quits on",
            "▎Options",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            WIDE,
            "Compare levels",
            TITLE,
            "running",
            "unsaved",
            "none",
            "no save:",
        ],
    );
    assert!(
        next_line(&visible, "Stakes at start").contains("clean"),
        "stakes default is not clean\n{visible}"
    );
    assert!(
        next_line(&visible, "Quit letter").contains('q')
            && !next_line(&visible, "Quit letter").contains("none"),
        "quit letter default is not q\n{visible}"
    );
    size(&app, 80, 24);
    shot(
        &app,
        "sc.quitleave.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &fixed,
    );
    app.resize(120, 40);
    require(
        &app,
        &[
            CAP_OPT,
            WIDE,
            "Page options",
            "Guard can save on",
            "Esc quits off",
            "Ctrl+C quits on",
            "Ctrl+Q quits on",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            TITLE,
            "running",
            "○── Guard can save",
        ],
    );
    size(&app, 120, 40);
    shot(
        &app,
        "sc.quitleave.options.120x40",
        "options caption visible at 120 by 40",
        &fixed,
    );
    shot(
        &app,
        "sc.quitleave.options",
        "options is the visible Quit & leave section",
        &fixed,
    );
    app.finish();
}

#[test]
fn states_section_and_specimens() {
    let args = ["--page", "quitleave"];
    let fixed = fixture(&args);
    let app = open("s01-states", 80, 24, &args);
    click_first(&app, "States");
    require(
        &app,
        &[
            "App shell › Quit & leave",
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "disabled",
            "Cancel",
            "Discard",
            "Save",
            "▎States",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            WIDE,
            "●",
            "Page options",
            "focus + hover",
            "selected",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "overflow",
            TITLE,
        ],
    );
    size(&app, 80, 24);
    shot(
        &app,
        "sc.quitleave.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &fixed,
    );
    app.resize(120, 40);
    require(
        &app,
        &[
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "disabled",
            "Stay",
            "depicts a focused sample",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "●",
            "Page options",
            "focus + hover",
            "selected",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "overflow",
            "256 colors",
        ],
    );
    size(&app, 120, 40);
    shot(
        &app,
        "sc.quitleave.states.120x40",
        "states caption and the five grid rows at 120 by 40",
        &fixed,
    );
    shot(
        &app,
        "sc.quitleave.states",
        "states is the visible Quit & leave section",
        &fixed,
    );
    for (case_id, label) in SHOWN {
        require(
            &app,
            &[label, "○ Compare levels", CAP_STATES, "Cancel", "Stay"],
            &["●", "256 colors", "Page options"],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &fixed,
        );
    }
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "quitleave"];
    let fixed = fixture(&args);
    let app = open("s01-compare", 120, 40, &args);
    click_first(&app, "States");
    require(
        &app,
        &["○ Compare levels", "rest", "disabled", CAP_STATES],
        &["256 colors", "16 colors", "no color", "●"],
    );
    click_first(&app, "Compare levels");
    require(
        &app,
        &["●", "truecolor", "256 colors", CAP_STATES, "Quit & leave"],
        &["○", CAP_PLAY, CAP_OPT, "Page options"],
    );
    for _ in 0..6 {
        app.key("pagedown");
    }
    require(
        &app,
        &[
            "Quit & leave",
            "truecolor",
            "256 colors",
            "16 colors",
            "no color",
            "disabled",
            WIDE,
        ],
        &[CAP_PLAY, "Page options", "Tokens and principles"],
    );
    shot(
        &app,
        "sc.quitleave.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &fixed,
    );
    app.resize(80, 24);
    let narrow = text(&app.settle());
    let bands = narrow.contains("truecolor")
        || narrow.contains("256 colors")
        || narrow.contains("16 colors")
        || narrow.contains("no color");
    assert!(
        bands && narrow.contains("Quit & leave") && !narrow.contains("Tokens and principles"),
        "narrow compare bands missing\n{narrow}"
    );
    size(&app, 80, 24);
    shot(
        &app,
        "sc.quitleave.compare-levels.80x24",
        "compare levels still on at 80 by 24 before growing and before turning it off",
        &fixed,
    );
    app.key("space");
    for _ in 0..8 {
        let visible = text(&app.settle());
        if visible.contains("○")
            && visible.contains("Compare levels")
            && !visible.contains("256 colors")
        {
            break;
        }
        app.key("pageup");
    }
    require(
        &app,
        &["Quit & leave", "○", "Compare levels"],
        &[
            "●",
            "256 colors",
            "16 colors",
            "no color",
            "Tokens and principles",
        ],
    );
    app.resize(120, 40);
    click_first(&app, "Options");
    let visible = text(&app.settle());
    require(
        &app,
        &[
            "App shell › Quit & leave",
            CAP_OPT,
            "Page options",
            "Guard can save on",
            "Esc quits off",
            "Ctrl+C quits on",
            "Ctrl+Q quits on",
        ],
        &[
            "running",
            "unsaved",
            "none",
            "no save:",
            "256 colors",
            "Focus & navigation",
        ],
    );
    assert!(
        next_line(&visible, "Stakes at start").contains("clean"),
        "compare levels changed stakes\n{visible}"
    );
    assert!(
        next_line(&visible, "Quit letter").contains('q')
            && !next_line(&visible, "Quit letter").contains("none"),
        "compare levels changed the quit letter\n{visible}"
    );
    app.finish();
}

#[test]
fn stakes_change_through_the_options_control() {
    let args = ["--page", "quitleave"];
    let fixed = fixture(&args);
    let app = open("s01-stakes", 120, 40, &args);
    click_first(&app, "Options");
    let visible = text(&app.settle());
    assert!(
        next_line(&visible, "Stakes at start").contains("clean")
            && !next_line(&visible, "Stakes at start").contains("running"),
        "default stakes are not clean\n{visible}"
    );
    require(
        &app,
        &[
            "Guard can save on",
            "Esc quits off",
            "Ctrl+C quits on",
            "Ctrl+Q quits on",
            CAP_OPT,
        ],
        &["running", "unsaved", "none", "Esc Close"],
    );
    click_first(&app, "clean");
    require(
        &app,
        &["unsaved", "running", "Esc Close", "Quit & leave"],
        &["Stop export", "Tokens and principles"],
    );
    app.key("down");
    app.key("down");
    app.key("enter");
    let chosen = text(&app.settle());
    assert!(
        next_line(&chosen, "Stakes at start").contains("running")
            && !next_line(&chosen, "Stakes at start").contains("clean"),
        "stakes control did not commit running\n{chosen}"
    );
    require(
        &app,
        &[
            "Guard can save on",
            "Esc quits off",
            "Ctrl+C quits on",
            "Ctrl+Q quits on",
            "Quit & leave",
        ],
        &["Esc Close", "unsaved", "none", "○── Guard can save"],
    );
    assert!(
        next_line(&chosen, "Quit letter").contains('q')
            && !next_line(&chosen, "Quit letter").contains("none"),
        "stakes change moved the quit letter\n{chosen}"
    );
    click_first(&app, "Playground");
    let effect = text(&app.settle());
    require(
        &app,
        &[
            "Stop export",
            "Running · the export",
            "exporting",
            TITLE,
            "save: Cancel Discard Save",
            CAP_PLAY,
        ],
        &[
            "Run export",
            "Unsaved ·",
            "no save:",
            "edited",
            "Page options",
        ],
    );
    assert_eq!(
        classify(&effect, "clean").as_slice(),
        ["clean", "Quit", "Quit", "Quit", "—"].as_slice(),
        "stakes change rewrote the quit-key table\n{effect}"
    );
    assert!(
        line(&effect, "Edits").contains("Clean") && !line(&effect, "Edits").contains("Unsaved"),
        "running stakes marked the title unsaved\n{effect}"
    );
    shot(
        &app,
        "opt.quitleave.stakes",
        "stakes running is committed and the other options stay at their defaults",
        &fixed,
    );
    click_row(&app, "Stop export", "Stop export");
    require(
        &app,
        &["Run export", "Clean", TITLE],
        &["Stop export", "Running · the export", "exporting"],
    );
    app.finish();
}

fn boundary(
    name: &str,
    opt: &str,
    case_id: &str,
    options: (&[&str], &[&str]),
    effect: (&[&str], &[&str]),
    rows: (&[&str], &[&str]),
) {
    let args = ["--page", "quitleave", "--opt", opt];
    let fixed = fixture(&args);
    let app = open(name, 120, 40, &args);
    click_first(&app, "Options");
    require(&app, options.0, options.1);
    click_first(&app, "Playground");
    let visible = text(&app.settle());
    require(&app, effect.0, effect.1);
    assert_eq!(
        classify(&visible, "clean").as_slice(),
        rows.0,
        "quit-key clean row\n{visible}"
    );
    assert_eq!(
        classify(&visible, "selection").as_slice(),
        rows.1,
        "quit-key selection row\n{visible}"
    );
    assert!(
        visible.contains("Quit & leave") && visible.contains(CAP_PLAY),
        "option left the page\n{visible}"
    );
    if opt.ends_with("letter=none") {
        let header = tokens(line(&visible, "Ctrl+C"));
        assert!(
            header.contains(&"letter") && !header.contains(&"q"),
            "quit letter none left the column header as q\n{visible}"
        );
    }
    if opt.ends_with("esc_quits=on") {
        assert_eq!(
            classify(&visible, "text focus").as_slice(),
            ["text", "focus", "—", "Quit", "Quit", "—"].as_slice(),
            "esc quits changed the text-focus row\n{visible}"
        );
    }
    shot(
        &app,
        case_id,
        &format!("{opt} is committed and the quit-key table shows that boundary"),
        &fixed,
    );
    if opt.ends_with("ctrl_q=off") {
        click_first(&app, "Options");
        click_first(&app, "Ctrl+Q quits");
        require(&app, &["Ctrl+Q quits on"], &["Ctrl+Q quits off"]);
    }
    app.finish();
}

#[test]
fn option_boundaries() {
    boundary(
        "s01-save",
        "quitleave.save=off",
        "opt.quitleave.save",
        (
            &[
                "Guard can save off",
                "Stakes at start",
                "Esc quits off",
                "Ctrl+C quits on",
                "Ctrl+Q quits on",
                CAP_OPT,
            ],
            &["Guard can save on", "running", "none", "──● Esc quits"],
        ),
        (
            &["no save: Cancel Discard", TITLE, "Run export", "Clean"],
            &[
                "save: Cancel Discard Save",
                "Stop export",
                "Unsaved ·",
                "Running ·",
                "Page options",
            ],
        ),
        (
            &["clean", "Quit", "Quit", "Quit", "—"],
            &["selection", "Quit", "Copy", "Quit", "—"],
        ),
    );
    boundary(
        "s01-letter",
        "quitleave.letter=none",
        "opt.quitleave.letter",
        (
            &[
                "none",
                "Guard can save on",
                "Esc quits off",
                "Ctrl+C quits on",
                "Ctrl+Q quits on",
            ],
            &["running", "unsaved"],
        ),
        (
            &[TITLE, "Run export", "save: Cancel Discard Save"],
            &["Stop export", "Unsaved ·", "no save:", "Page options"],
        ),
        (
            &["clean", "—", "Quit", "Quit", "—"],
            &["selection", "—", "Copy", "Quit", "—"],
        ),
    );
    boundary(
        "s01-esc",
        "quitleave.esc_quits=on",
        "opt.quitleave.esc_quits",
        (
            &[
                "Esc quits on",
                "Guard can save on",
                "Ctrl+C quits on",
                "Ctrl+Q quits on",
            ],
            &["Esc quits off", "running", "none", "○── Esc quits"],
        ),
        (
            &[TITLE, "Run export", "save: Cancel Discard Save", "Clean"],
            &["Stop export", "no save:", "Unsaved ·", "Page options"],
        ),
        (
            &["clean", "Quit", "Quit", "Quit", "Quit"],
            &["selection", "Quit", "Copy", "Quit", "Quit"],
        ),
    );
    boundary(
        "s01-ctrl-c",
        "quitleave.ctrl_c=off",
        "opt.quitleave.ctrl_c",
        (
            &[
                "Ctrl+C quits off",
                "Guard can save on",
                "Esc quits off",
                "Ctrl+Q quits on",
            ],
            &["Ctrl+C quits on", "running", "none"],
        ),
        (
            &[TITLE, "Run export", "Clean"],
            &["Stop export", "no save:", "Unsaved ·"],
        ),
        (
            &["clean", "Quit", "—", "Quit", "—"],
            &["selection", "Quit", "—", "Quit", "—"],
        ),
    );
    boundary(
        "s01-ctrl-q",
        "quitleave.ctrl_q=off",
        "opt.quitleave.ctrl_q",
        (
            &[
                "Ctrl+Q quits off",
                "Guard can save on",
                "Esc quits off",
                "Ctrl+C quits on",
            ],
            &["Ctrl+Q quits on", "running", "none"],
        ),
        (
            &[TITLE, "Run export", "Clean"],
            &["Stop export", "no save:", "Unsaved ·"],
        ),
        (
            &["clean", "Quit", "Quit", "—", "—"],
            &["selection", "Quit", "Copy", "—", "—"],
        ),
    );
}

#[test]
fn leave_choices() {
    let args = ["--page", "quitleave"];
    let fixed = fixture(&args);
    let app = open("s01-leave", 120, 40, &args);
    require(
        &app,
        &[
            TITLE,
            "Run export",
            "save: Cancel Discard Save",
            "Last request",
            "Clean",
            CAP_PLAY,
        ],
        &[
            "Unsaved changes",
            "Still running",
            "Stop export",
            "Would go to Overview now",
            "Would quit now",
        ],
    );
    assert!(
        line(&text(&app.screen()), "Last request").contains('—'),
        "a request was already recorded"
    );
    shot(
        &app,
        "sc.quitleave.leave.before",
        "clean document before any leave choice",
        &fixed,
    );

    click_row(&app, "Leave for Overview", "Leave for Overview");
    let left = text(&app.settle());
    assert!(
        line(&left, "Last request").contains("Ok(Leave::To(Overview)): go now"),
        "clean leave did not resolve immediately\n{left}"
    );
    assert!(
        footer(&left).contains("Would go to Overview now"),
        "clean leave status missing\n{left}"
    );
    assert!(
        line(&left, "settlement report").contains(TITLE)
            && !line(&left, "settlement report").contains("XQ3")
            && line(&left, "Edits").contains("Clean"),
        "clean leave changed the title\n{left}"
    );
    require(
        &app,
        &["App shell › Quit & leave", "Run export"],
        &[
            "Save before leaving?",
            "Still running",
            "Tokens and principles",
            "Unsaved ·",
        ],
    );

    click_row(&app, "Quit", "Run export");
    let quit = text(&app.settle());
    assert!(
        line(&quit, "Last request").contains("Ok(Leave::Quit): go now")
            && footer(&quit).contains("Would quit now"),
        "clean quit did not resolve immediately\n{quit}"
    );
    require(
        &app,
        &["App shell › Quit & leave", TITLE, "Run export"],
        &["Save before leaving?", "Still running", "Stop export"],
    );

    click_row(&app, TITLE, "settlement");
    app.type_text("X");
    let edited = text(&app.settle());
    assert!(
        line(&edited, "settlement report").contains("XQ3 settlement report")
            && line(&edited, "Edits").contains("Unsaved · 1 change(s)"),
        "typing did not mark the title unsaved\n{edited}"
    );
    click_row(&app, "Leave for Overview", "Leave for Overview");
    require(
        &app,
        &[
            "Unsaved changes",
            "Save before leaving?",
            "1 unsaved change would be",
            "lost.",
            "Cancel",
            "Discard",
            "Save",
        ],
        &["Still running", "Stop and leave", "Tokens and principles"],
    );
    app.key("escape");
    let stayed = text(&app.settle());
    assert!(
        line(&stayed, "Last answer").contains("Resolved::Stay")
            && !line(&stayed, "Last answer").contains("Discard")
            && !line(&stayed, "Last answer").contains("Save")
            && footer(&stayed).contains("Stayed"),
        "cancel did not stay\n{stayed}"
    );
    assert!(
        line(&stayed, "settlement report").contains("XQ3 settlement report")
            && line(&stayed, "Edits").contains("Unsaved · 1 change(s)"),
        "cancel discarded the edit\n{stayed}"
    );
    require(
        &app,
        &["App shell › Quit & leave", "Run export"],
        &["Save before leaving?", "Discarded;"],
    );

    click_row(&app, "Leave for Overview", "Leave for Overview");
    app.wait_text("Save before leaving?");
    click_row(&app, "Discard", "Cancel");
    let discarded = text(&app.settle());
    assert!(
        line(&discarded, "Last answer").contains("Resolved::Discard(Leave::To(Overview))")
            && footer(&discarded).contains("Discarded; would go to Overview now"),
        "discard did not discard\n{discarded}"
    );
    assert!(
        line(&discarded, "settlement report").contains(TITLE)
            && !line(&discarded, "settlement report").contains('X')
            && line(&discarded, "Edits").contains("Clean"),
        "discard kept the edit\n{discarded}"
    );

    click_row(&app, TITLE, "settlement");
    app.type_text("Z");
    click_row(&app, "Leave for Overview", "Leave for Overview");
    app.wait_text("Save before leaving?");
    app.key("enter");
    let saved = text(&app.settle());
    assert!(
        line(&saved, "Last answer").contains("Resolved::Save(Leave::To(Overview))")
            && footer(&saved).contains("Saved; would go to Overview now"),
        "save did not save\n{saved}"
    );
    assert!(
        line(&saved, "settlement report").contains("ZQ3 settlement report")
            && line(&saved, "Edits").contains("Clean")
            && line(&saved, "Document").contains("saved"),
        "save dropped the edited title\n{saved}"
    );

    click_row(&app, "Run export", "Run export");
    require(
        &app,
        &[
            "Stop export",
            "Running · the export",
            "exporting",
            "ZQ3 settlement report",
            "Export running",
        ],
        &["Run export", "Unsaved ·"],
    );
    click_row(&app, "Leave for Overview", "Leave for Overview");
    require(
        &app,
        &[
            "Still running",
            "the export still running. Leaving stops it.",
            "Stop and leave",
            "Stay",
        ],
        &["Save before leaving?", "Tokens and principles"],
    );
    click_row(&app, "Stay", "Stop and leave");
    let running_stay = text(&app.settle());
    assert!(
        line(&running_stay, "Last answer").contains("Resolved::Stay")
            && footer(&running_stay).contains("Stayed")
            && line(&running_stay, "Work").contains("Running · the export")
            && line(&running_stay, "settlement report").contains("ZQ3 settlement report"),
        "stay stopped the export or dropped the saved title\n{running_stay}"
    );
    require(
        &app,
        &["Stop export", "exporting"],
        &[
            "Run export",
            "Save before leaving?",
            "the export still running",
        ],
    );

    click_row(&app, "Leave for Overview", "Leave for Overview");
    app.wait_text("Stop and leave");
    click_row(&app, "Stop and leave", "Stay");
    let stopped = text(&app.settle());
    assert!(
        line(&stopped, "Last answer").contains("Resolved::Discard(Leave::To(Overview))")
            && footer(&stopped).contains("Discarded; would go to Overview now")
            && line(&stopped, "Work").contains("Clean")
            && line(&stopped, "settlement report").contains("ZQ3 settlement report"),
        "stop did not stop, or it dropped the saved title\n{stopped}"
    );
    assert!(
        line(&stopped, "Document").contains("saved")
            && line(&stopped, "Last request").contains("Err: Still running"),
        "stop cleared the recorded running error or the document meta\n{stopped}"
    );
    require(
        &app,
        &["Run export", "App shell › Quit & leave"],
        &[
            "Stop export",
            "Running · the export",
            "exporting",
            "Stop and leave",
            "the export still running",
            "Save before leaving?",
        ],
    );
    shot(
        &app,
        "sc.quitleave.leave.after",
        "stop ended the export and the saved title is still the edited value",
        &fixed,
    );
    app.finish();
}
