//! Overview route, sections, specimens, compare levels, tokens, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s01_overview/` and each case id's dots become underscores.
//! A dotted id such as `S01.overview/sc.overview.playground.120x40` is rejected
//! and, where the grammar allowed the dots, `set_extension` would also drop
//! the final segment and collide the two sizes.

use crate::support::{App, Launch, Point, SHOWCASE, find_all, text, try_launch};

const BLURB: &str = "Tokens and principles behind every component";
const CHROME: &str = "App shell ›";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const NARROW: &str = "80×24";
const EXPLORER_DEFAULT: &str = "plane Card · kind Secondary";
const LADDER_DEFAULT: &str = "ladder theme · modifiers level";

const SHOWN: &[(&str, &str)] = &[
    ("sc.overview.specimen.rest", "rest"),
    ("sc.overview.specimen.hover", "hover"),
    ("sc.overview.specimen.pressed", "pressed"),
    ("sc.overview.specimen.focused", "focused"),
    ("sc.overview.specimen.focused_hover", "focus + hover"),
    ("sc.overview.specimen.selected", "selected"),
    ("sc.overview.specimen.disabled", "disabled"),
    ("sc.overview.specimen.error", "error"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s01_overview/{}", case_id.replace('.', "_"))
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
    if args.contains(&"chrome") {
        app.wait_text("Brand lockup");
    } else {
        app.wait_text(BLURB);
    }
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

fn shot(app: &App, case_id: &str, _precondition: &str, _fixture_args: &[String]) {
    app.visual(&visual_id(case_id));
}

fn page_until(app: &App, needle: &str) {
    if text(&app.screen()).contains(needle) {
        return;
    }
    app.key("home");
    for _ in 0..8 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.key("pagedown");
    }
    let point = Point { x: 40, y: 16 };
    for _ in 0..24 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 1);
    }
    panic!("missing {needle:?}\n{}", text(&app.settle()));
}

fn show_effect(app: &App, line: &str, also: &str) {
    click_first(app, "Playground");
    require(app, &[CAP_PLAY, BLURB], &[CAP_OPT, CAP_STATES, CHROME]);
    page_until(app, line);
    if !text(&app.screen()).contains(also) {
        let point = Point { x: 40, y: 16 };
        for _ in 0..8 {
            app.wheel(point, true, 1);
            let visible = text(&app.settle());
            if visible.contains(line) && visible.contains(also) {
                break;
            }
        }
    }
    require(
        app,
        &[line, also, BLURB, CAP_PLAY],
        &[CHROME, CAP_OPT, CAP_STATES],
    );
}

#[test]
fn route_opens_overview() {
    let args = ["--page", "overview"];
    let app = open("s01-route-page", 120, 40, &args);
    require(
        &app,
        &[
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Tokens · planes and text",
        ],
        &[
            CHROME,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.overview.route",
        "--page overview opens the playground with the sidebar focused",
        &fixture(&args),
    );
    app.finish();

    let chrome = ["--page", "chrome"];
    let app = open("s01-route-keys", 120, 40, &chrome);
    require(&app, &[CHROME, "Brand lockup"], &[BLURB]);
    app.key("[");
    require(&app, &[BLURB, CAP_PLAY, WIDE], &[CHROME]);
    app.key("]");
    require(&app, &[CHROME, "Brand lockup"], &[BLURB]);
    app.key("[");
    require(
        &app,
        &[BLURB, CAP_PLAY, "Tokens · planes and text"],
        &[CHROME, "Brand lockup", "Page options"],
    );
    app.finish();

    let app = open("s01-route-pointer", 120, 40, &chrome);
    require(&app, &[CHROME, "Brand lockup"], &[BLURB]);
    click_first(&app, "Overview");
    require(
        &app,
        &[BLURB, CAP_PLAY, "Tokens · planes and text", WIDE],
        &[CHROME, "Brand lockup", "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn playground_sections_at_both_sizes() {
    let args = ["--page", "overview"];
    let fixed = fixture(&args);
    let app = open("s01-playground", 80, 24, &args);
    require(
        &app,
        &[
            BLURB,
            NARROW,
            "Playground",
            "Tokens · planes and text",
            "canvas",
        ],
        &[
            CHROME,
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.overview.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &fixed,
    );
    app.resize(120, 40);
    require(
        &app,
        &[BLURB, CAP_PLAY, WIDE, "Tokens · planes and text"],
        &[CHROME, CAP_OPT, CAP_STATES, "Page options"],
    );
    app.key("tab");
    require(
        &app,
        &[CAP_PLAY, "▎Playground"],
        &[CAP_OPT, CAP_STATES, CHROME],
    );
    app.key("2");
    require(
        &app,
        &[
            CAP_OPT,
            "Page options",
            "card",
            "secondary",
            "theme",
            "level",
        ],
        &[CAP_PLAY, CAP_STATES, CHROME, "Compare levels"],
    );
    app.key("3");
    require(
        &app,
        &[
            CAP_STATES,
            "○ Compare levels",
            "rest",
            "focus + hover",
            "error",
        ],
        &[CAP_PLAY, CAP_OPT, CHROME, "Page options", "●"],
    );
    app.key("1");
    require(
        &app,
        &[CAP_PLAY, WIDE, "Tokens · planes and text", "#000000"],
        &[
            CAP_OPT,
            CAP_STATES,
            CHROME,
            "Page options",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.overview.playground.120x40",
        "playground tab focused at 120 by 40 after keyboard section jumps",
        &fixed,
    );
    shot(
        &app,
        "sc.overview.playground",
        "playground is the visible Overview section",
        &fixed,
    );
    app.finish();
}

#[test]
fn options_section_at_both_sizes() {
    let args = ["--page", "overview"];
    let fixed = fixture(&args);
    let app = open("s01-options", 80, 24, &args);
    require(&app, &[BLURB, NARROW, "Playground"], &[CHROME, CAP_PLAY]);
    click_first(&app, "Options");
    require(
        &app,
        &[
            BLURB,
            NARROW,
            "Page options",
            "Plane",
            "card",
            "Kind",
            "secondary",
            "Ladder",
            "theme",
            "State modifiers",
            "level",
        ],
        &[
            CHROME,
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "seam",
            "mono",
        ],
    );
    shot(
        &app,
        "sc.overview.options.80x24",
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
            "card",
            "secondary",
            "theme",
            "level",
        ],
        &[
            CHROME,
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "danger",
            "seam",
        ],
    );
    shot(
        &app,
        "sc.overview.options.120x40",
        "options caption visible at 120 by 40",
        &fixed,
    );
    shot(
        &app,
        "sc.overview.options",
        "options is the visible Overview section",
        &fixed,
    );
    app.finish();
}

#[test]
fn states_section_and_specimens() {
    let args = ["--page", "overview"];
    let fixed = fixture(&args);
    let app = open("s01-states", 80, 24, &args);
    click_first(&app, "States");
    require(
        &app,
        &[
            BLURB,
            NARROW,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "selected",
            "disabled",
            "error",
        ],
        &[
            CHROME, CAP_PLAY, CAP_OPT, CAP_STATES, "●", "busy", "editing", "empty", "loading",
            "overflow", "narrow",
        ],
    );
    shot(
        &app,
        "sc.overview.states.80x24",
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
            "focus + hover",
            "error",
            "Secondary",
        ],
        &[
            CHROME,
            CAP_PLAY,
            CAP_OPT,
            "●",
            "Page options",
            "busy",
            "editing",
            "empty",
            "loading",
            "overflow",
            "narrow",
        ],
    );
    shot(
        &app,
        "sc.overview.states.120x40",
        "states caption and the eight grid rows at 120 by 40",
        &fixed,
    );
    shot(
        &app,
        "sc.overview.states",
        "states is the visible Overview section",
        &fixed,
    );
    for (case_id, label) in SHOWN {
        require(
            &app,
            &[label, "○ Compare levels", CAP_STATES],
            &[CHROME, "●"],
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
    let args = ["--page", "overview"];
    let fixed = fixture(&args);
    let app = open("s01-compare", 120, 40, &args);
    click_first(&app, "States");
    require(
        &app,
        &[BLURB, "○ Compare levels", "rest", CAP_STATES],
        &[CHROME, "256 colors", "16 colors", "no color", "●"],
    );
    click_first(&app, "Compare levels");
    require(
        &app,
        &[BLURB, "●", "256 colors", CAP_STATES],
        &[CHROME, "○", CAP_PLAY, CAP_OPT],
    );
    for _ in 0..4 {
        if text(&app.screen()).contains("no color") && text(&app.screen()).contains("16 colors") {
            break;
        }
        app.key("pagedown");
    }
    require(
        &app,
        &[BLURB, "16 colors", "no color", WIDE, "Compare levels"],
        &[CHROME, CAP_PLAY, "Page options"],
    );
    shot(
        &app,
        "sc.overview.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &fixed,
    );
    app.resize(80, 24);
    let narrow = text(&app.settle());
    let bands = narrow.contains("256 colors")
        || narrow.contains("16 colors")
        || narrow.contains("no color");
    assert!(
        bands && narrow.contains(NARROW) && narrow.contains(BLURB) && !narrow.contains(CHROME),
        "narrow compare bands missing\n{narrow}"
    );
    shot(
        &app,
        "sc.overview.compare-levels.80x24",
        "compare levels still on at 80 by 24 before growing and before turning it off",
        &fixed,
    );
    app.key("space");
    require(
        &app,
        &[BLURB, NARROW, "○"],
        &[CHROME, "●", "256 colors", "16 colors", "no color"],
    );
    app.resize(120, 40);
    click_first(&app, "Options");
    require(
        &app,
        &[
            BLURB,
            CAP_OPT,
            WIDE,
            "Page options",
            "card",
            "secondary",
            "theme",
            "level",
        ],
        &[
            CHROME, CAP_PLAY, CAP_STATES, "seam", "mono", "danger", "custom",
        ],
    );
    app.finish();
}

#[test]
fn tokens_before_and_after() {
    let args = ["--page", "overview"];
    let fixed = fixture(&args);
    let app = open("s01-tokens", 120, 40, &args);
    require(
        &app,
        &[BLURB, CAP_PLAY, WIDE, "Tokens · planes and text"],
        &[CHROME],
    );
    let visible = text(&app.settle());
    let canvas = visible
        .lines()
        .find(|line| line.contains("canvas"))
        .unwrap_or("");
    assert!(
        canvas.contains("#000000"),
        "canvas line lacks #000000: {canvas:?}"
    );
    assert!(
        !canvas.contains("#48e054"),
        "forbidden accent swatch on the canvas line: {canvas:?}"
    );
    shot(
        &app,
        "sc.overview.tokens.before",
        "planes card is the visible token example before scrolling",
        &fixed,
    );
    app.key("tab");
    require(&app, &[CAP_PLAY], &[CAP_OPT, CAP_STATES, CHROME]);
    page_until(&app, "#48e054");
    require(
        &app,
        &["accent", "#48e054", CAP_PLAY],
        &[CHROME, "plane Danger"],
    );
    page_until(&app, "Ladders · theme.ladders");
    require(
        &app,
        &["Ladders · theme.ladders", CAP_PLAY],
        &[CHROME, CAP_OPT],
    );
    page_until(&app, "Contrast on the card, per level");
    require(
        &app,
        &[
            "Contrast on the card, per level",
            "contrast_ratio",
            CAP_PLAY,
        ],
        &[CHROME, "plane Danger"],
    );
    show_effect(&app, EXPLORER_DEFAULT, "State modifiers · theme.mods");
    require(
        &app,
        &[
            EXPLORER_DEFAULT,
            LADDER_DEFAULT,
            "State modifiers · theme.mods",
        ],
        &[CHROME, "plane Danger", CAP_OPT, CAP_STATES],
    );
    shot(
        &app,
        "sc.overview.tokens.after",
        "explorer and state modifiers are visible after the token examples",
        &fixed,
    );
    app.finish();
}

#[test]
fn plane_change_cancels_then_commits() {
    let args = ["--page", "overview"];
    let fixed = fixture(&args);
    let app = open("s01-plane", 120, 40, &args);
    click_first(&app, "Options");
    require(
        &app,
        &[
            CAP_OPT,
            "Page options",
            "card",
            "secondary",
            "theme",
            "level",
        ],
        &[CHROME, CAP_PLAY, "Esc Close", "seam", "mono"],
    );
    click_first(&app, "card");
    require(
        &app,
        &["canvas", "danger", "Esc Close"],
        &[CHROME, "plane Danger"],
    );
    app.key("escape");
    require(
        &app,
        &["card", "secondary", "theme", "level", CAP_OPT],
        &[CHROME, "Esc Close", "canvas", "highlight-danger"],
    );
    click_first(&app, "card");
    require(&app, &["danger", "Esc Close"], &[CHROME]);
    app.key("d");
    require(&app, &["danger", "Esc Close"], &[CHROME]);
    app.key("enter");
    require(
        &app,
        &["danger", "secondary", "theme", "level", CAP_OPT],
        &[
            CHROME,
            "Esc Close",
            "canvas",
            "highlight-danger",
            "seam",
            "mono",
        ],
    );
    show_effect(
        &app,
        "plane Danger · kind Secondary",
        "ladder theme · modifiers level",
    );
    require(
        &app,
        &[
            "plane Danger · kind Secondary",
            "ladder theme · modifiers level",
            CAP_PLAY,
        ],
        &[
            CHROME,
            "plane Card",
            "kind Seam",
            "modifiers MONO",
            "custom (with_ladder)",
            CAP_OPT,
        ],
    );
    shot(
        &app,
        "opt.overview.plane",
        "plane danger is committed and the other options stay at their defaults",
        &fixed,
    );
    app.finish();
}

fn boundary(
    name: &str,
    opt: &str,
    case_id: &str,
    on_options: (&[&str], &[&str]),
    effect: &str,
    forbid: &[&str],
) {
    let args = ["--page", "overview", "--opt", opt];
    let app = open(name, 120, 40, &args);
    click_first(&app, "Options");
    require(&app, on_options.0, on_options.1);
    show_effect(&app, effect, "State modifiers · theme.mods");
    require(&app, &[effect, CAP_PLAY, BLURB], forbid);
    shot(
        &app,
        case_id,
        "the option boundary is visible in the explorer and siblings stay default",
        &fixture(&args),
    );
    app.finish();
}

#[test]
fn kind_ladder_and_mods_boundaries() {
    let defaults = ["--page", "overview"];
    let app = open("s01-option-defaults", 120, 40, &defaults);
    click_first(&app, "Options");
    require(
        &app,
        &[
            "card",
            "secondary",
            "theme",
            "level",
            "Page options",
            CAP_OPT,
        ],
        &[CHROME, "seam", "mono", "custom", "danger", "Esc Close"],
    );
    app.finish();

    boundary(
        "s01-kind",
        "overview.kind=seam",
        "opt.overview.kind",
        (
            &["seam", "card", "theme", "level", CAP_OPT, "Page options"],
            &["secondary", "Secondary", "mono", "danger", CHROME],
        ),
        "plane Card · kind Seam",
        &[
            CHROME,
            "kind Secondary",
            "plane Danger",
            "modifiers MONO",
            "custom (with_ladder)",
            CAP_OPT,
        ],
    );
    boundary(
        "s01-ladder",
        "overview.ladder=custom",
        "opt.overview.ladder",
        (
            &[
                "custom",
                "card",
                "secondary",
                "level",
                CAP_OPT,
                "Page options",
            ],
            &["theme", "seam", "mono", CHROME],
        ),
        "ladder custom (with_ladder) · modifiers level",
        &[
            CHROME,
            "ladder theme",
            "plane Danger",
            "kind Seam",
            "modifiers MONO",
            CAP_OPT,
        ],
    );
    boundary(
        "s01-mods",
        "overview.mods=mono",
        "opt.overview.mods",
        (
            &[
                "mono",
                "card",
                "secondary",
                "theme",
                CAP_OPT,
                "Page options",
            ],
            &["level", "seam", "danger", "custom", CHROME],
        ),
        "ladder theme · modifiers MONO",
        &[
            CHROME,
            "modifiers level",
            "plane Danger",
            "kind Seam",
            "custom (with_ladder)",
            CAP_OPT,
        ],
    );
}
