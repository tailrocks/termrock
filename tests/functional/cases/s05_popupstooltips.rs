//! Popups & tooltips route, sections, specimens, compare levels, options, and overlays.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s05_popupstooltips/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Popups & tooltips";
const BLURB: &str = "Where popups go and how they close";
const FILE_BLURB: &str = "FileBrowser:";
const DIALOG_BLURB: &str = "Forms, info, choices and help";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const FRAME: &str = "╭─ Frame ─────────────── meta ─ × ─╮";
const FRAME_NO_CLOSE: &str = "╭─ Frame ─────────────────── meta ─╮";
const PLACE_BELOW: &str = "Below · place_with → 24×8 at 28,10";
const PLACE_ABOVE: &str = "Above · place_with → 24×8 at 28,1";
const PLACE_LINE: &str = "margin 1 · vertical margin as margin · Center · Shrink · text inset 0";
const DISMISS_LINE: &str =
    "Esc on · outside click on · pass through off · focus out on · refused Flash";
const TIPS_LINE: &str = "on · after 600 ms · wrap at 48 cells";
const MENU_HINT: &str = "↑↓ Move  Enter Choose  Esc Close  Tab Next";
const CLOSED_HINT: &str = "Enter / Space Open popover";
const FULL_PATH: &str = "src/widgets/dialog.rs · the modal dialog and its typed acknowledgement";
const CUT_PATH: &str = "src/widgets/dialog.rs · the modal dialog…";

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s05_popupstooltips/{}",
        case_id.replace('.', "_")
    )
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
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

fn with_opt(name: &str, opt: &str) -> App {
    let pair = format!("popupstooltips.{opt}");
    let args = ["--page", "popupstooltips", "--opt", pair.as_str()];
    let app = open(name, 120, 40, &args);
    app.wait_text("Open popover");
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

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str) {
    let line = line_after(screen, label);
    assert!(
        line.contains(value),
        "{label} shows {line:?}, expected {value}\n{}",
        text(screen)
    );
}

fn locate(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
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

fn click_where(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let hit = locate(app, label, pred);
    app.click(hit);
    hit
}

fn click_tab(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x < 24);
}

/// Paused motion keeps a released click's pressed color until something else redraws.
fn clear_flash(app: &App, cols: u16, rows: u16) {
    std::thread::sleep(Duration::from_millis(200));
    app.resize(121, rows);
    let hopped = format!("121×{rows}");
    app.wait_text(&hopped);
    app.resize(cols, rows);
    if cols >= 100 {
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow size", |screen| {
            let visible = text(screen);
            !visible.contains(&hopped) && visible.contains(TITLE)
        });
    }
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn open_menu(app: &App) -> TerminalSnapshot {
    click_where(app, "Open popover", |hit| hit.x > 24);
    app.wait_text("Copy link")
}

fn settle_menu(app: &App) -> TerminalSnapshot {
    clear_flash(app, 120, 40);
    app.wait_text("Copy link")
}

fn scroll_policies(app: &App) -> TerminalSnapshot {
    click_where(app, "Save draft", |hit| hit.x > 24);
    clear_flash(app, 120, 40);
    app.key("end");
    app.wait_text("wheel passes")
}

fn options_until(app: &App, needle: &str) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options");
    clear_flash(app, 120, 40);
    for _ in 0..8 {
        if text(&app.screen()).contains(needle) {
            return app.wait_text(needle);
        }
        app.key("pagedown");
        app.settle();
    }
    app.wait_text(needle)
}

fn states_page(app: &App) -> TerminalSnapshot {
    click_tab(app, "States");
    app.wait_text("Compare levels");
    clear_flash(app, 120, 40);
    app.wait_text("○ Compare levels")
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "popupstooltips"];
    let app = open("s05-route-page", 120, 40, &args);
    let screen = app.wait_text(TITLE);
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "Open popover",
            "Save draft",
            CUT_PATH,
            PLACE_BELOW,
            "› Popups & tooltips",
        ],
        &[
            OVERVIEW,
            FILE_BLURB,
            DIALOG_BLURB,
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
        ],
    );
    shot(&app, "sc.popupstooltips.route");

    let app = open("s05-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    for _ in 0..28 {
        app.key("]");
    }
    let entered = app.wait_text(BLURB);
    see(
        &entered,
        &[TITLE, BLURB, "Open popover", CAP_PLAY],
        &[OVERVIEW, FILE_BLURB, "Page options"],
    );
    app.key("]");
    let next = app.wait_text(FILE_BLURB);
    see(
        &next,
        &["File browser", FILE_BLURB],
        &[BLURB, "Open popover"],
    );
    app.key("[");
    let back = app.wait_text(BLURB);
    see(
        &back,
        &[TITLE, BLURB, "Open popover"],
        &[FILE_BLURB, OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[BLURB, "Open popover"]);
    if !text(&overview).contains("Popups & tooltips") {
        click_side(&app, "Overlays");
        app.wait_text("Popups & tooltips");
    }
    click_side(&app, "Popups & tooltips");
    let pointed = app.wait_text(BLURB);
    see(
        &pointed,
        &[
            TITLE,
            BLURB,
            "Open popover",
            CAP_PLAY,
            "› Popups & tooltips",
        ],
        &[OVERVIEW, FILE_BLURB, "Page options", "Compare levels"],
    );
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "popupstooltips"];
    let app = open("s05-sections", 80, 24, &args);
    let narrow_play = app.wait_text("Open popover");
    see(
        &narrow_play,
        &[
            TITLE,
            "Where popups go",
            "Playground",
            "Open popover",
            "Save draft",
            "Popups & to",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "place_with",
        ],
    );
    shot(&app, "sc.popupstooltips.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    clear_flash(&app, 80, 24);
    let narrow_opt = app.wait_text("Popover placement");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Options",
            "Page options",
            "Popover placement",
            "Esc closes on",
            "Outside click closes on",
            "Popups & to",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Open popover",
            WIDE,
            "truecolor",
            "Frame dismiss",
            "Reveal on hover",
        ],
    );
    assert_select(&narrow_opt, "Popover placement", "below");
    assert_select(&narrow_opt, "Edge margin", "1");
    shot(&app, "sc.popupstooltips.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    clear_flash(&app, 80, 24);
    let narrow_states = app.wait_text("○ Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            "States",
            "○ Compare levels",
            "rest",
            "Frame",
            "Popups & to",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            WIDE,
            "truecolor",
            "● Compare levels",
            "hover",
            "pressed",
        ],
    );
    shot(&app, "sc.popupstooltips.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_text(WIDE);
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "truecolor",
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            FRAME,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "disabled",
            "selected",
            "busy",
            "empty",
        ],
    );
    assert!(
        text(&wide_states).contains("▏ depicts a focused sample"),
        "focus legend missing\n{}",
        text(&wide_states)
    );
    shot(&app, "sc.popupstooltips.states.120x40");
    shot(&app, "sc.popupstooltips.states");
    for case_id in [
        "sc.popupstooltips.specimen.rest",
        "sc.popupstooltips.specimen.hover",
    ] {
        let screen = app.screen();
        see(
            &screen,
            &["rest", "hover", "○ Compare levels", CAP_STATES, WIDE, FRAME],
            &["● Compare levels", "Page options", "disabled", "selected"],
        );
        shot(&app, case_id);
    }

    app.key("pagedown");
    let pressed = app.wait_text("pressed");
    see(
        &pressed,
        &["pressed", "╭─ Frame", "╰", "×", "○ Compare levels", WIDE],
        &["Page options", "● Compare levels", "Open popover"],
    );
    assert!(
        lines_of(&pressed)
            .iter()
            .any(|line| line.contains("pressed"))
            && lines_of(&pressed).iter().any(|line| line.contains('╰')),
        "pressed frame is clipped\n{}",
        text(&pressed)
    );
    shot(&app, "sc.popupstooltips.specimen.pressed");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    clear_flash(&app, 120, 40);
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "Page options",
            "Esc closes on",
            "Outside click closes on",
            "Click passes through off",
            "Focus out closes on",
            "Frame dims on",
            "Rows kept live",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Open popover",
            "Esc closes off",
            "Outside click closes off",
            "Click passes through on",
            "Focus out closes off",
            "Frame dims off",
        ],
    );
    assert_select(&wide_opt, "Popover placement", "below");
    assert_select(&wide_opt, "Edge margin", "1");
    assert_select(&wide_opt, "Vertical margin", "as-margin");
    assert_select(&wide_opt, "Centred at", "center");
    assert_select(&wide_opt, "No room", "shrink");
    assert_select(&wide_opt, "Text inset", "0");
    assert_select(&wide_opt, "Refused click", "flash");
    assert_select(&wide_opt, "Frame placement", "center");
    assert_select(&wide_opt, "Frame fit", "default");
    assert_select(&wide_opt, "Frame padding", "3x1");
    assert_select(&wide_opt, "Frame min size", "20x5");
    assert_select(&wide_opt, "Frame chrome rows", "1");
    shot(&app, "sc.popupstooltips.options.120x40");
    shot(&app, "sc.popupstooltips.options");

    click_tab(&app, "Playground");
    app.wait_text("Open popover");
    clear_flash(&app, 120, 40);
    let wide_play = app.wait_text(WIDE);
    see(
        &wide_play,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Open popover",
            "Save draft",
            "New tab",
            CUT_PATH,
            PLACE_BELOW,
            PLACE_LINE,
            DISMISS_LINE,
            TIPS_LINE,
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            OVERVIEW,
            FILE_BLURB,
            "Above · place_with",
        ],
    );
    shot(&app, "sc.popupstooltips.playground.120x40");
    shot(&app, "sc.popupstooltips.playground");
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "popupstooltips"];
    let app = open("s05-compare", 120, 40, &args);
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
    for _ in 0..8 {
        if text(&app.screen()).contains("no color") {
            break;
        }
        app.key("pagedown");
        app.settle();
    }
    clear_flash(&app, 120, 40);
    for _ in 0..8 {
        if text(&app.screen()).contains("no color") {
            break;
        }
        app.key("pagedown");
        app.settle();
    }
    let bands = app.wait_text("no color");
    see(
        &bands,
        &[TITLE, "no color", WIDE, "Compare levels"],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(&app, "sc.popupstooltips.compare-levels.120x40");
    app.resize(80, 24);
    let narrow = app.wait_screen("compare at 80", |screen| {
        let visible = text(screen);
        visible.contains("● Compare levels")
            && visible.contains("truecolor")
            && !visible.contains(WIDE)
    });
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.popupstooltips.compare-levels.80x24");
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &["● Compare levels", "no color", "Page options"],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Popover placement");
    assert_select(&options, "Popover placement", "below");
    see(
        &options,
        &[TITLE, "› Popups & tooltips", "below"],
        &["no color", "● Compare levels", FILE_BLURB],
    );
}

#[test]
fn overlays_open_then_escape_dismisses_without_choosing() {
    let args = ["--page", "popupstooltips"];
    let app = open("s05-overlays", 120, 40, &args);
    let opened = open_menu(&app);
    see(
        &opened,
        &["Copy link", "report.pdf", "Share…", "Remove", MENU_HINT],
        &[CLOSED_HINT, "Chosen(0)"],
    );
    let settled = settle_menu(&app);
    see(
        &settled,
        &[
            "Copy link",
            "report.pdf",
            MENU_HINT,
            PLACE_BELOW,
            "Open popover",
        ],
        &[CLOSED_HINT, "Chosen(0)", FILE_BLURB],
    );
    shot(&app, "sc.popupstooltips.overlays.before");
    app.key("esc");
    let closed = app.wait_text(CLOSED_HINT);
    see(
        &closed,
        &[CLOSED_HINT, "Open popover ▾", TITLE],
        &["Copy link", "report.pdf", MENU_HINT, "Chosen(0)"],
    );
    app.key("end");
    let last = app.wait_text("Dismissed");
    see(
        &last,
        &["Last", "Dismissed", CLOSED_HINT, TITLE],
        &["Copy link", "report.pdf", "Chosen(0)", MENU_HINT],
    );
    shot(&app, "sc.popupstooltips.overlays.after");
}

#[test]
fn option_placement_above_draws_the_menu_over_the_header() {
    let app = with_opt("s05-placement", "placement=above");
    let opened = open_menu(&app);
    see(
        &opened,
        &[
            PLACE_ABOVE,
            "Copy link",
            "report.pdf",
            MENU_HINT,
            PLACE_LINE,
        ],
        &["Below · place_with", CLOSED_HINT, "Chosen(0)"],
    );
    let settled = settle_menu(&app);
    see(
        &settled,
        &[PLACE_ABOVE, "Copy link", MENU_HINT],
        &["Below · place_with", CLOSED_HINT],
    );
    shot(&app, "opt.popupstooltips.placement");
}

#[test]
fn option_margin_zero_rewrites_the_place_line() {
    let app = with_opt("s05-margin", "margin=0");
    let screen = app.wait_text("margin 0");
    see(
        &screen,
        &[
            "margin 0 · vertical margin as margin · Center · Shrink · text inset 0",
            PLACE_BELOW,
            TIPS_LINE,
        ],
        &["margin 1", "Above · place_with"],
    );
    shot(&app, "opt.popupstooltips.margin");
}

#[test]
fn option_vertical_margin_two_is_its_own_number() {
    let app = with_opt("s05-vmargin", "vertical_margin=2");
    let screen = app.wait_text("vertical margin 2");
    see(
        &screen,
        &[
            "margin 1 · vertical margin 2 · Center · Shrink · text inset 0",
            PLACE_BELOW,
        ],
        &["vertical margin as margin", "margin 0"],
    );
    shot(&app, "opt.popupstooltips.vertical_margin");
}

#[test]
fn option_v_align_third_replaces_center() {
    let app = with_opt("s05-valign", "v_align=third");
    let screen = app.wait_text("Third");
    see(
        &screen,
        &["margin 1 · vertical margin as margin · Third · Shrink · text inset 0"],
        &["· Center ·", "· Slide ·"],
    );
    shot(&app, "opt.popupstooltips.v_align");
}

#[test]
fn option_overflow_slide_replaces_shrink() {
    let app = with_opt("s05-overflow", "overflow=slide");
    let screen = app.wait_text("Slide");
    see(
        &screen,
        &["margin 1 · vertical margin as margin · Center · Slide · text inset 0"],
        &["· Shrink ·"],
    );
    shot(&app, "opt.popupstooltips.overflow");
}

#[test]
fn option_text_inset_two_rewrites_the_place_line() {
    let app = with_opt("s05-inset", "text_inset=2");
    let screen = app.wait_text("text inset 2");
    see(
        &screen,
        &["margin 1 · vertical margin as margin · Center · Shrink · text inset 2"],
        &["text inset 0"],
    );
    shot(&app, "opt.popupstooltips.text_inset");
}

#[test]
fn option_escape_off_keeps_the_menu() {
    let app = with_opt("s05-escape", "escape=off");
    open_menu(&app);
    let settled = settle_menu(&app);
    see(
        &settled,
        &["Esc off", "Copy link", MENU_HINT],
        &[CLOSED_HINT],
    );
    app.key("esc");
    let stayed = app.wait_text("Copy link");
    see(
        &stayed,
        &["Copy link", "report.pdf", MENU_HINT, "Esc off"],
        &[CLOSED_HINT, "Chosen(0)", "Dismissed"],
    );
    shot(&app, "opt.popupstooltips.escape");
}

#[test]
fn option_outside_click_off_keeps_the_menu() {
    let on = open("s05-outside-on", 120, 40, &["--page", "popupstooltips"]);
    open_menu(&on);
    click_where(&on, "eu-west-1", |hit| hit.x > 40);
    let closed = on.wait_screen("menu closed", |screen| !text(screen).contains("Copy link"));
    see(
        &closed,
        &["Open popover", "eu-west-1"],
        &["Copy link", "report.pdf", MENU_HINT, "Chosen(0)"],
    );
    on.key("end");
    let last = on.wait_text("Dismissed");
    see(
        &last,
        &["Dismissed", CLOSED_HINT],
        &["Chosen(0)", "Copy link", MENU_HINT],
    );

    let app = with_opt("s05-outside-off", "outside_click=off");
    open_menu(&app);
    let settled = settle_menu(&app);
    see(
        &settled,
        &["outside click off", "Copy link", MENU_HINT],
        &[CLOSED_HINT],
    );
    click_where(&app, "eu-west-1", |hit| hit.x > 40);
    clear_flash(&app, 120, 40);
    let stayed = app.wait_text("Copy link");
    see(
        &stayed,
        &["Copy link", "report.pdf", MENU_HINT, "outside click off"],
        &[CLOSED_HINT, "Chosen(0)", "Dismissed"],
    );
    shot(&app, "opt.popupstooltips.outside_click");
}

#[test]
fn option_pass_through_on_is_on_the_dismiss_line() {
    let app = with_opt("s05-pass", "pass_through=on");
    let screen = app.wait_text("pass through on");
    see(
        &screen,
        &[
            "Esc on · outside click on · pass through on · focus out on · refused Flash",
            PLACE_BELOW,
            PLACE_LINE,
        ],
        &["pass through off", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.pass_through");
}

#[test]
fn option_refuse_feedback_none_rewrites_the_dismiss_line() {
    let app = with_opt("s05-refuse", "refuse_feedback=none");
    let screen = app.wait_text("refused None");
    see(
        &screen,
        &[
            "Esc on · outside click on · pass through off · focus out on · refused None",
            PLACE_LINE,
        ],
        &["refused Flash"],
    );
    shot(&app, "opt.popupstooltips.refuse_feedback");
}

#[test]
fn option_focus_out_off_is_on_the_dismiss_line() {
    let app = with_opt("s05-focus-off", "focus_out=off");
    let screen = app.wait_text("focus out off");
    see(
        &screen,
        &["focus out off", "pass through off", PLACE_BELOW, PLACE_LINE],
        &["focus out on", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.focus_out");
}

#[test]
fn option_frame_placement_below_marks_the_anchor() {
    let app = with_opt("s05-frame-place", "frame_placement=below");
    let screen = states_page(&app);
    see(
        &screen,
        &["▣ anchor", "○ Compare levels", "rest", FRAME, TITLE],
        &["Page options", "● Compare levels", "Open popover"],
    );
    shot(&app, "opt.popupstooltips.frame_placement");
}

#[test]
fn option_frame_fit_menu_selects_menu() {
    let app = with_opt("s05-frame-fit", "frame_fit=menu");
    let screen = options_until(&app, "Frame fit");
    assert_select(&screen, "Frame fit", "menu");
    assert_select(&screen, "Frame placement", "center");
    assert_select(&screen, "Popover placement", "below");
    see(
        &screen,
        &[TITLE, "Page options", "menu"],
        &["Open popover", "Compare levels"],
    );
    shot(&app, "opt.popupstooltips.frame_fit");
}

#[test]
fn option_frame_margin_1x0_selects_that_padding() {
    let app = with_opt("s05-frame-margin", "frame_margin=1x0");
    let screen = options_until(&app, "Frame padding");
    assert_select(&screen, "Frame padding", "1x0");
    assert_select(&screen, "Frame min size", "20x5");
    see(&screen, &[TITLE, "1x0"], &["3x1", "Open popover"]);
    shot(&app, "opt.popupstooltips.frame_margin");
}

#[test]
fn option_frame_min_40x8_selects_that_minimum() {
    let app = with_opt("s05-frame-min", "frame_min=40x8");
    let screen = options_until(&app, "Frame min size");
    assert_select(&screen, "Frame min size", "40x8");
    assert_select(&screen, "Frame padding", "3x1");
    see(&screen, &[TITLE, "40x8"], &["20x5", "Open popover"]);
    shot(&app, "opt.popupstooltips.frame_min");
}

#[test]
fn option_frame_chrome_zero_selects_zero_rows() {
    let app = with_opt("s05-frame-chrome", "frame_chrome=0");
    let screen = options_until(&app, "Frame chrome rows");
    assert_select(&screen, "Frame chrome rows", "0");
    see(
        &screen,
        &[TITLE, "Frame dims on"],
        &["Open popover", "Compare levels"],
    );
    shot(&app, "opt.popupstooltips.frame_chrome");
}

#[test]
fn option_frame_backdrop_off_clears_the_dim_toggle() {
    let app = with_opt("s05-frame-dim-off", "frame_backdrop=off");
    let screen = options_until(&app, "Frame dims off");
    see(
        &screen,
        &[
            TITLE,
            "Frame dims off",
            "Page options",
            "Focus out closes on",
        ],
        &["Frame dims on", "Open popover", "Compare levels"],
    );
    shot(&app, "opt.popupstooltips.frame_backdrop");
}

#[test]
fn option_frame_dim_keep_top_selects_that_value() {
    let app = with_opt("s05-frame-dim", "frame_dim=keep-top");
    let screen = options_until(&app, "keep-top");
    assert_select(&screen, "Rows kept live", "keep-top");
    see(
        &screen,
        &[TITLE, "keep-top"],
        &["Open popover", "Compare levels"],
    );
    shot(&app, "opt.popupstooltips.frame_dim");
}

#[test]
fn option_frame_dismiss_never_hides_the_close_button() {
    let app = with_opt("s05-frame-dismiss", "frame_dismiss=never");
    let options = options_until(&app, "never");
    assert_select(&options, "Frame dismiss", "never");
    click_tab(&app, "States");
    app.wait_text("Compare levels");
    clear_flash(&app, 120, 40);
    let screen = app.wait_text(FRAME_NO_CLOSE);
    see(
        &screen,
        &[FRAME_NO_CLOSE, "○ Compare levels", "rest", TITLE],
        &[FRAME, "Page options", "● Compare levels"],
    );
    shot(&app, "opt.popupstooltips.frame_dismiss");
}

#[test]
fn option_frame_close_off_draws_dashes_for_hover_and_pressed() {
    let app = with_opt("s05-frame-close", "frame_close=off");
    let screen = states_page(&app);
    see(
        &screen,
        &[FRAME_NO_CLOSE, "rest", "○ Compare levels", TITLE],
        &[FRAME, "Page options", "▣ anchor"],
    );
    shot(&app, "opt.popupstooltips.frame_close");
}

#[test]
fn option_frame_close_button_danger_draws_the_heavy_mark() {
    let app = with_opt("s05-frame-x", "frame_close_button=danger");
    let screen = states_page(&app);
    see(
        &screen,
        &["✕", "rest", "○ Compare levels", "Frame", TITLE],
        &[FRAME, "Page options", "▣ anchor"],
    );
    shot(&app, "opt.popupstooltips.frame_close_button");
}

#[test]
fn option_tooltips_off_hides_the_save_tip() {
    let app = with_opt("s05-tips-off", "tooltips=off");
    let save = locate(&app, "Save draft", |hit| hit.x > 24);
    app.move_to(save);
    let screen = app.wait_text("off · after 600 ms");
    see(
        &screen,
        &[
            "off · after 600 ms · wrap at 48 cells",
            "Save draft",
            PLACE_BELOW,
        ],
        &["Save the draft to this device", "on · after 600 ms"],
    );
    shot(&app, "opt.popupstooltips.tooltips");
}

#[test]
fn option_tip_delay_zero_rewrites_the_tooltips_line() {
    let app = with_opt("s05-tip-delay", "tip_delay=0");
    let screen = app.wait_text("after 0 ms");
    see(
        &screen,
        &["on · after 0 ms · wrap at 48 cells", PLACE_BELOW],
        &["after 600 ms", "after 1500 ms"],
    );
    shot(&app, "opt.popupstooltips.tip_delay");
}

#[test]
fn option_tip_width_24_rewrites_the_tooltips_line() {
    let app = with_opt("s05-tip-width", "tip_width=24");
    let screen = app.wait_text("wrap at 24 cells");
    see(
        &screen,
        &[
            "on · after 600 ms · wrap at 24 cells",
            "Save draft",
            PLACE_BELOW,
        ],
        &["wrap at 48 cells", "Save the draft to this device"],
    );
    shot(&app, "opt.popupstooltips.tip_width");
}

#[test]
fn option_reveal_hover_off_keeps_the_cut_row() {
    let app = with_opt("s05-reveal-hover", "reveal_hover=off");
    let row = locate(&app, "dialog.rs", |hit| hit.x > 24);
    app.move_to(row);
    let hovered = app.settle();
    see(
        &hovered,
        &[CUT_PATH, "README.md", "popup.rs"],
        &["typed acknowledgement"],
    );
    shot(&app, "opt.popupstooltips.reveal_hover");
    app.click(row);
    app.key("right");
    let opened = app.wait_text("← Collapse");
    see(
        &opened,
        &[FULL_PATH, "README.md", "popup.rs · placement, fitting and…"],
        &["→ Full text"],
    );
}

#[test]
fn option_reveal_key_r_uses_r_and_ignores_right() {
    let app = with_opt("s05-reveal-key", "reveal_key=r");
    click_where(&app, "dialog.rs", |hit| hit.x > 24);
    let focused = app.wait_text("r Full text");
    see(
        &focused,
        &["r Full text", "README.md", "popup.rs"],
        &["→ Full text", "← Collapse"],
    );
    // The ignored key repaints nothing. Let the flash expire first so its
    // order against the tick cannot change the settled frame.
    app.stable_screen();
    app.key("right");
    let stayed = app.settle();
    see(&stayed, &["r Full text", "README.md"], &["← Collapse"]);
    shot(&app, "opt.popupstooltips.reveal_key");
    app.key("r");
    let opened = app.wait_text("← Collapse");
    see(
        &opened,
        &[FULL_PATH, "README.md", "← Collapse"],
        &["→ Full text", "r Full text"],
    );
}

#[test]
fn option_reveal_mode_inline_wraps_the_same_path() {
    let app = with_opt("s05-reveal-inline", "reveal_mode=inline");
    click_where(&app, "dialog.rs", |hit| hit.x > 24);
    app.key("right");
    let opened = app.wait_text("← Collapse");
    see(
        &opened,
        &[
            "src/widgets/dialog.rs · the modal",
            "dialog and its typed acknowledgement",
            "← Collapse",
            "popup.rs",
        ],
        &["→ Full text"],
    );
    shot(&app, "opt.popupstooltips.reveal_mode");
}

#[test]
fn option_reveal_rows_three_is_on_the_reveal_line() {
    let app = with_opt("s05-reveal-rows", "reveal_rows=3");
    let screen = scroll_policies(&app);
    see(
        &screen,
        &[
            "rows 3",
            "scroll on",
            "wheel 3",
            "after 300",
            "wheel passes on",
        ],
        &["rows as fit", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.reveal_rows");
}

#[test]
fn option_reveal_scroll_off_is_on_the_reveal_line() {
    let app = with_opt("s05-reveal-scroll", "reveal_scroll=off");
    let screen = scroll_policies(&app);
    see(
        &screen,
        &["scroll off", "rows as fit", "wheel passes on"],
        &["scroll on", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.reveal_scroll");
}

#[test]
fn option_reveal_wheel_one_is_on_the_reveal_line() {
    let app = with_opt("s05-reveal-wheel", "reveal_wheel=1");
    let screen = scroll_policies(&app);
    see(
        &screen,
        &["wheel 1", "rows as fit", "scroll on"],
        &["wheel 3", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.reveal_wheel");
}

#[test]
fn option_reveal_delay_zero_is_on_the_reveal_line() {
    let app = with_opt("s05-reveal-delay", "reveal_delay=0");
    let screen = scroll_policies(&app);
    see(
        &screen,
        &["after 0 ms", "rows as fit", TIPS_LINE],
        &["after 300", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.reveal_delay");
}

#[test]
fn option_reveal_passes_off_is_on_the_reveal_line() {
    let app = with_opt("s05-reveal-passes", "reveal_passes=off");
    let screen = scroll_policies(&app);
    see(
        &screen,
        &["wheel passes off", "rows as fit", "scroll on", "wheel 3"],
        &["wheel passes on", "Copy link"],
    );
    shot(&app, "opt.popupstooltips.reveal_passes");
}
