//! Forms route, sections, specimens, compare levels, options, and submit.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s02_forms/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const PAGE: &str = "Inputs & forms › Forms";
const BLURB: &str = "Sections, required fields, validation, submission";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const NAME_PLACEHOLDER: &str = "Short imperative summary";
const DESC_A: &str = "What should Junie do, and what does";
const DESC_B: &str = "done look like?";
const HELP: &str = "Optional · Markdown";
const REVIEWER_HINT: &str = "name@company.com";
const MANAGED: &str = "Managed by your organization";
const SUBMITTED_NAME: &str = "Ship the forms page";
const SUBMITTED_EMAIL: &str = "ana@company.com";

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s02_forms/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
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

fn assert_size(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(screen.cols(), cols, "terminal columns");
    assert_eq!(screen.rows(), rows, "terminal rows");
}

fn row_containing(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing row {needle:?}\n{}", text(screen)))
}

fn compare_marked(screen: &TerminalSnapshot, on: bool) -> bool {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .any(|line| {
            line.contains("Compare levels")
                && if on {
                    line.contains('●')
                } else {
                    line.contains('○') && !line.contains('●')
                }
        })
}

fn click_filtered(app: &App, needle: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.settle();
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
    app.click(hits[0]);
}

fn click_tab(app: &App, label: &str) {
    click_filtered(app, label, |hit| hit.y == 4);
}

fn click_sidebar(app: &App, label: &str) {
    click_filtered(app, label, |hit| hit.x < 24 && hit.y >= 4);
}

fn see(app: &App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn open_forms_from_sidebar(app: &App) {
    let screen = app.settle();
    let visible = find_all(&screen, "Forms")
        .into_iter()
        .any(|point| point.x < 24 && point.y > 4);
    if !visible {
        click_sidebar(app, "Inputs & forms");
        app.wait_text("Forms");
    }
    click_sidebar(app, "Forms");
}

fn assert_toggle(screen: &TerminalSnapshot, label: &str, on: bool) {
    let line = row_containing(screen, label);
    let at = line.find(label).expect(label);
    let after = &line[at + label.len()..];
    let is_on = after.contains(" on");
    let is_off = after.contains(" off");
    assert!(is_on ^ is_off, "{label} is neither on nor off: {line:?}");
    assert_eq!(is_on, on, "{label} line {line:?}");
}

fn select_value(screen: &TerminalSnapshot, label: &str) -> String {
    let visible = text(screen);
    let lines: Vec<&str> = visible.lines().collect();
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label:?}\n{visible}"));
    let column = lines[index].find(label).expect("label column");
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).copied().unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    rest.split_whitespace()
        .find(|token| *token != "▎" && *token != "▾")
        .unwrap_or("")
        .to_string()
}

fn assert_option_defaults(screen: &TerminalSnapshot) {
    assert_toggle(screen, "Task name required", true);
    assert_eq!(select_value(screen, "Validate"), "after-error");
    assert_eq!(select_value(screen, "On blur"), "commit");
    assert_eq!(select_value(screen, "Description rows"), "4");
    assert_toggle(screen, "Help text", true);
    assert_toggle(screen, "Placeholders", true);
    assert_toggle(screen, "Managed toggle", true);
}

fn assert_option_except(
    screen: &TerminalSnapshot,
    changed: &str,
    value: &str,
    toggle: Option<bool>,
) {
    if changed != "Task name required" {
        assert_toggle(screen, "Task name required", true);
    }
    if changed != "Validate" {
        assert_eq!(select_value(screen, "Validate"), "after-error");
    }
    if changed != "On blur" {
        assert_eq!(select_value(screen, "On blur"), "commit");
    }
    if changed != "Description rows" {
        assert_eq!(select_value(screen, "Description rows"), "4");
    }
    if changed != "Help text" {
        assert_toggle(screen, "Help text", true);
    }
    if changed != "Placeholders" {
        assert_toggle(screen, "Placeholders", true);
    }
    if changed != "Managed toggle" {
        assert_toggle(screen, "Managed toggle", true);
    }
    match toggle {
        Some(on) => assert_toggle(screen, changed, on),
        None => assert_eq!(select_value(screen, changed), value, "{changed}"),
    }
}

fn omitted_states() -> &'static [&'static str] {
    &[
        "pressed",
        "focus + hover",
        "selected",
        "busy",
        "loading",
        "overflow",
    ]
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "forms"];
    let app = open("s02-forms-route", 120, 40, &args);
    let screen = app.wait_text("Task name *");
    assert_size(&screen, 120, 40);
    require(
        &app,
        &[
            PAGE,
            BLURB,
            CAP_PLAY,
            "120×40",
            "New task",
            "Ctrl+S Submit",
            "Task name *",
            NAME_PLACEHOLDER,
            "(●) Balanced",
            "Create task",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Inputs & forms › Checks & radios",
            "Inputs & forms › Chips & selects",
            "Task name  optional",
        ],
    );
    let selected = (0..screen.rows())
        .map(|row| row_text(&screen, row))
        .find(|line| line.contains('▎') && line.contains("Forms") && line.contains('›'))
        .unwrap_or_else(|| panic!("sidebar focus is not on Forms\n{}", text(&screen)));
    assert!(
        selected.contains("Forms"),
        "sidebar focus stays on Forms: {selected}"
    );
    see(
        &app,
        "sc.forms.route",
        "--page forms opens the playground with the sidebar focus on Forms",
        &args,
    );
    app.key("]");
    let checks = app.wait_text("Inputs & forms › Checks & radios");
    assert!(
        !text(&checks).contains("New task") && !text(&checks).contains("Task name *"),
        "] should leave Forms for Checks & radios\n{}",
        text(&checks)
    );
    app.key("[");
    let returned = app.wait_text("Task name *");
    assert!(
        text(&returned).contains(PAGE)
            && text(&returned).contains(CAP_PLAY)
            && !text(&returned).contains("Inputs & forms › Checks & radios"),
        "[ should re-enter Forms\n{}",
        text(&returned)
    );
    click_sidebar(&app, "Overview");
    let overview = app.wait_text("Tokens and principles");
    assert!(
        !text(&overview).contains(PAGE) && !text(&overview).contains("Task name *"),
        "sidebar Overview should leave Forms\n{}",
        text(&overview)
    );
    open_forms_from_sidebar(&app);
    require(
        &app,
        &[PAGE, CAP_PLAY, "Task name *", NAME_PLACEHOLDER],
        &[
            "Tokens and principles",
            CAP_OPT,
            CAP_STATES,
            "Inputs & forms › Checks & radios",
        ],
    );
    app.finish();

    let neighbor = ["--page", "chipsselects"];
    let app = open("s02-forms-keys", 120, 40, &neighbor);
    app.wait_text("Inputs & forms › Chips & selects");
    app.key("]");
    let opened = app.wait_text("Task name *");
    assert!(
        text(&opened).contains(PAGE)
            && text(&opened).contains(CAP_PLAY)
            && !text(&opened).contains("Inputs & forms › Chips & selects"),
        "] from Chips & selects should open Forms\n{}",
        text(&opened)
    );
    app.key("[");
    let chips = app.wait_text("Inputs & forms › Chips & selects");
    assert!(
        !text(&chips).contains("Task name *"),
        "[ should leave Forms\n{}",
        text(&chips)
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "forms"];
    let app = open("s02-forms-sections", 80, 24, &args);
    let narrow_play = app.wait_text("Task name *");
    assert_size(&narrow_play, 80, 24);
    require(
        &app,
        &[
            PAGE,
            BLURB,
            "New task",
            "Ctrl+S Submit",
            "Task name *",
            NAME_PLACEHOLDER,
            DESC_A,
            "like?",
            HELP,
            "Reviewer  optional",
            REVIEWER_HINT,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "120×40",
            "Task name  optional",
            "Create task",
        ],
    );
    see(
        &app,
        "sc.forms.playground.80x24",
        "playground at 80 by 24 before the terminal grows, with no section caption",
        &args,
    );
    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    assert_size(&narrow_opt, 80, 24);
    require(
        &app,
        &[
            PAGE,
            "Page options",
            "Task name required",
            "Description rows",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "New task",
            "Compare levels",
            "120×40",
            NAME_PLACEHOLDER,
        ],
    );
    assert_option_defaults(&app.screen());
    see(
        &app,
        "sc.forms.options.80x24",
        "options at 80 by 24 before the terminal grows, with every control at its default",
        &args,
    );
    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    assert_size(&narrow_states, 80, 24);
    require(
        &app,
        &[
            PAGE,
            "○ Compare levels",
            "Required field",
            "rest",
            "hover",
            "focused",
            "Fix the login flow",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "New task",
            "120×40",
            "● Compare levels",
        ],
    );
    let visible = text(&app.screen());
    for omitted in omitted_states() {
        assert!(
            !visible.contains(omitted),
            "forbidden specimen {omitted:?} is visible\n{visible}"
        );
    }
    see(
        &app,
        "sc.forms.states.80x24",
        "states at 80 by 24 before the terminal grows, showing rest, hover, and focused",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_screen("grown states", |screen| {
        screen.cols() == 120 && text(screen).contains(CAP_STATES) && text(screen).contains("empty")
    });
    assert_size(&wide_states, 120, 40);
    require(
        &app,
        &[
            PAGE,
            CAP_STATES,
            "120×40",
            "○ Compare levels",
            "Required field",
            "Optional field",
            "Description",
            "rest",
            "hover",
            "focused",
            "editing",
            "error",
            "disabled",
            "empty",
            "Fix the login flow",
            "ana@company.com",
            "Enter a valid value",
            "Explain why, in the im",
            "Short imperative summ",
            "What should Junie do?",
            "ln 1/2",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "New task",
            "● Compare levels",
            "pressed",
            "focus + hover",
            "selected",
            "busy",
            "loading",
            "overflow",
        ],
    );
    let rest_at = (0..wide_states.rows())
        .position(|row| row_text(&wide_states, row).contains("rest"))
        .expect("rest label");
    let rest = row_text(&wide_states, rest_at as u32 + 1);
    assert!(
        rest.contains("Fix the login flow")
            && rest.contains("ana@company.com")
            && !rest.contains("Enter a valid value")
            && !rest.contains("Short imperative")
            && !rest.contains('▏'),
        "rest row keeps the filled samples: {rest}"
    );
    let focused = row_containing(&wide_states, "▏ Fix the login flow");
    assert!(
        focused.contains("ana@company.com"),
        "focused row draws the focus bar on the samples: {focused}"
    );
    assert!(
        text(&wide_states).contains("editing") && text(&wide_states).contains("ln 1/2"),
        "editing row is missing the caret line\n{}",
        text(&wide_states)
    );
    let error = row_containing(&wide_states, "Enter a valid value");
    assert!(
        error.contains("Enter a valid value") && !error.contains("Short imperative"),
        "error row shows the specimen message: {error}"
    );
    assert!(
        text(&wide_states).contains("disabled") && text(&wide_states).contains("empty"),
        "disabled and empty rows are not on the wide states grid\n{}",
        text(&wide_states)
    );
    see(
        &app,
        "sc.forms.states.120x40",
        "states caption every state, drawn by the widget is active at 120 by 40",
        &args,
    );
    for _ in 0..4 {
        if text(&app.screen()).contains("narrow") && text(&app.screen()).contains("Fix the logi") {
            break;
        }
        app.key("pagedown");
        let _ = app.settle();
    }
    require(
        &app,
        &[
            PAGE,
            "narrow",
            "Fix the logi",
            "ana@company.",
            "Reproduce t",
        ],
        &[
            "Page options",
            "pressed",
            "focus + hover",
            "selected",
            "busy",
            "loading",
            "overflow",
            "New task",
        ],
    );
    see(
        &app,
        "sc.forms.specimen.narrow",
        "states grid paged to the narrow row at 120 by 40",
        &args,
    );

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(CAP_OPT);
    assert_size(&wide_opt, 120, 40);
    require(
        &app,
        &[PAGE, CAP_OPT, "120×40", "Page options"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "New task",
            "Compare levels",
            NAME_PLACEHOLDER,
        ],
    );
    assert_option_defaults(&app.screen());
    see(
        &app,
        "sc.forms.options.120x40",
        "options caption one control per option is active at 120 by 40 and every control is at its default",
        &args,
    );
    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    assert_size(&wide_play, 120, 40);
    require(
        &app,
        &[
            PAGE,
            CAP_PLAY,
            "120×40",
            "truecolor",
            "New task",
            "Ctrl+S Submit",
            "Task name *",
            NAME_PLACEHOLDER,
            HELP,
            "Reviewer  optional",
            REVIEWER_HINT,
            "( ) Fast",
            "(●) Balanced",
            "( ) Thorough",
            "[✓] Run tests before opening a PR",
            "[ ] Open a pull request when done",
            "Auto-approve changes off",
            "Notify on completion on",
            MANAGED,
            "Create task",
            "Reset",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Task name  optional",
            "Help text off",
            "Creating task",
            "Fix the highlighted fields",
        ],
    );
    see(
        &app,
        "sc.forms.playground.120x40",
        "playground caption the live widget is active at 120 by 40 after the narrow capture",
        &args,
    );
    app.key("2");
    require(
        &app,
        &[CAP_OPT, "Page options", "Task name required"],
        &[CAP_PLAY, CAP_STATES, "New task", "Compare levels"],
    );
    app.key("3");
    require(
        &app,
        &[CAP_STATES, "○ Compare levels", "rest"],
        &[CAP_PLAY, CAP_OPT, "Page options", "● Compare levels"],
    );
    app.key("1");
    require(
        &app,
        &[CAP_PLAY, "Task name *", "Create task"],
        &[CAP_OPT, CAP_STATES, "Page options"],
    );
    app.finish();
}

#[test]
fn compare_levels_then_restore() {
    let args = ["--page", "forms"];
    let app = open("s02-forms-compare", 120, 40, &args);
    app.wait_text(CAP_PLAY);
    app.key("tab");
    app.wait_text("←→ Section");
    app.key("3");
    let states = app.wait_text(CAP_STATES);
    let before = row_containing(&states, "Compare levels");
    assert!(
        before.contains('○') && !before.contains('●'),
        "compare starts off: {before}"
    );
    require(
        &app,
        &[PAGE, "rest", "Fix the login flow"],
        &[
            "256 colors",
            "16 colors",
            "no color",
            CAP_PLAY,
            "Page options",
        ],
    );
    click_filtered(&app, "Compare levels", |hit| hit.y < 16);
    let on = app.wait_screen("compare on", |screen| compare_marked(screen, true));
    assert!(
        text(&on).contains(CAP_STATES)
            && text(&on).contains("rest")
            && text(&on).contains("truecolor"),
        "compare on shows the truecolor band\n{}",
        text(&on)
    );
    assert!(
        !text(&on).contains("Page options") && !text(&on).contains("New task"),
        "compare on left States\n{}",
        text(&on)
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    for _ in 0..8 {
        let visible = text(&app.screen());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.key("pagedown");
        let _ = app.settle();
    }
    let scrolled = app.settle();
    assert!(
        saw_256 && saw_16 && text(&scrolled).contains("no color") && text(&scrolled).contains(PAGE),
        "scrolled colour bands missing (256={saw_256}, 16={saw_16})\n{}",
        text(&scrolled)
    );
    assert!(
        !text(&scrolled).contains("Page options") && !text(&scrolled).contains("New task"),
        "scrolling compare changed the page\n{}",
        text(&scrolled)
    );
    assert_size(&scrolled, 120, 40);
    see(
        &app,
        "sc.forms.compare-levels.120x40",
        "compare levels is on at 120 by 40 and the no color band is visible",
        &args,
    );
    app.resize(80, 24);
    app.wait_screen("narrow compare", |screen| screen.cols() == 80);
    for _ in 0..8 {
        if text(&app.screen()).contains("Compare levels") {
            break;
        }
        app.key("pageup");
        let _ = app.settle();
    }
    let narrow = app.wait_screen("compare button after page up", |screen| {
        compare_marked(screen, true)
    });
    assert_size(&narrow, 80, 24);
    require(
        &app,
        &[PAGE, "●", "Compare levels"],
        &[
            CAP_STATES,
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "New task",
            "120×40",
        ],
    );
    see(
        &app,
        "sc.forms.compare-levels.80x24",
        "compare levels stays on at 80 by 24 before it is turned off",
        &args,
    );
    click_filtered(&app, "Compare levels", |hit| hit.y < 16);
    let off = app.wait_screen("compare off", |screen| compare_marked(screen, false));
    assert!(
        !text(&off).contains("256 colors")
            && !text(&off).contains("16 colors")
            && !text(&off).contains("no color")
            && text(&off).contains(PAGE),
        "compare off should clear the bands and stay on Forms\n{}",
        text(&off)
    );
    app.resize(120, 40);
    app.wait_screen("wide after compare", |screen| {
        screen.cols() == 120 && text(screen).contains("120×40")
    });
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert_option_defaults(&options);
    assert!(
        text(&options).contains(PAGE) && !text(&options).contains("Inputs & forms › Checks"),
        "compare levels changed the page\n{}",
        text(&options)
    );
    app.finish();
}

#[test]
fn submit_invalid_then_reset() {
    let args = ["--page", "forms"];
    let app = open("s02-forms-submit", 120, 40, &args);
    let fresh = app.wait_text("Create task");
    assert_size(&fresh, 120, 40);
    assert!(
        !text(&fresh).contains("Required") && !text(&fresh).contains("Fix the highlighted fields"),
        "the default form already shows a validation error\n{}",
        text(&fresh)
    );
    click_filtered(&app, "Create task", |hit| hit.x > 24);
    let invalid = app.wait_text("Fix the highlighted fields");
    require(
        &app,
        &[
            PAGE,
            CAP_PLAY,
            "Task name *",
            NAME_PLACEHOLDER,
            "Required",
            "Fix the highlighted fields",
            "Create task",
            "Reset",
            "(●) Balanced",
            MANAGED,
        ],
        &[
            "Creating task",
            "Task created",
            SUBMITTED_NAME,
            SUBMITTED_EMAIL,
            "Form reset",
            "At least 4 characters",
        ],
    );
    assert_size(&invalid, 120, 40);
    see(
        &app,
        "sc.forms.submit.before",
        "empty submit shows Required and Fix the highlighted fields and does not create a task",
        &args,
    );
    app.type_text(SUBMITTED_NAME);
    app.key("enter");
    let named = app.wait_text(SUBMITTED_NAME);
    assert!(
        !text(&named).contains("Required")
            && !text(&named).contains("Creating task")
            && !text(&named).contains("At least 4 characters"),
        "a committed valid name still shows an error\n{}",
        text(&named)
    );
    click_filtered(&app, REVIEWER_HINT, |hit| hit.x > 24);
    app.type_text(SUBMITTED_EMAIL);
    app.key("enter");
    app.wait_text(SUBMITTED_EMAIL);
    app.key("ctrl+s");
    let created = app.wait_text("Creating task");
    require(
        &app,
        &[
            PAGE,
            SUBMITTED_NAME,
            SUBMITTED_EMAIL,
            "Creating task",
            "(●) Balanced",
            "Task name *",
        ],
        &[
            "Task created",
            "Fix the highlighted fields",
            "Required",
            "Form reset",
            NAME_PLACEHOLDER,
            "At least 4 characters",
        ],
    );
    assert_size(&created, 120, 40);
    click_filtered(&app, "Reset", |hit| hit.x > 24);
    let reset = app.wait_text("Form reset");
    assert_size(&reset, 120, 40);
    require(
        &app,
        &[
            PAGE,
            CAP_PLAY,
            "Form reset",
            "Task name *",
            NAME_PLACEHOLDER,
            HELP,
            "Reviewer  optional",
            REVIEWER_HINT,
            "(●) Balanced",
            MANAGED,
            "Create task",
            "Reset",
        ],
        &[
            SUBMITTED_NAME,
            SUBMITTED_EMAIL,
            "Creating task",
            "Task created",
            "Fix the highlighted fields",
            "Required",
            "At least 4 characters",
        ],
    );
    see(
        &app,
        "sc.forms.submit.after",
        "reset restores the empty defaults and drops the submitted name and reviewer",
        &args,
    );
    app.finish();
}

#[test]
fn help_changes_through_the_options_control() {
    let args = ["--page", "forms"];
    let app = open("s02-forms-help", 120, 40, &args);
    app.wait_text("Task name *");
    click_tab(&app, "Options");
    let defaults = app.wait_text("Help text");
    assert_option_defaults(&defaults);
    click_filtered(&app, "Help text", |hit| hit.x > 24);
    let toggled = app.wait_text("Help text off");
    assert_option_except(&toggled, "Help text", "off", Some(false));
    click_tab(&app, "Playground");
    let effect = app.wait_text("Task name *");
    require(
        &app,
        &[
            PAGE,
            CAP_PLAY,
            "Task name *",
            NAME_PLACEHOLDER,
            DESC_A,
            "Reviewer  optional",
            REVIEWER_HINT,
            "(●) Balanced",
            MANAGED,
            "Notify on completion on",
            "Create task",
        ],
        &[
            HELP,
            "\nOptional\n",
            "Help text on",
            "Task name  optional",
            "Placeholders off",
            "Managed toggle off",
            CAP_OPT,
            "Page options",
        ],
    );
    assert!(
        !text(&effect).contains("Optional · Markdown"),
        "help text stayed on the description\n{}",
        text(&effect)
    );
    see(
        &app,
        "opt.forms.help",
        "Help text toggled off from the Options control removes Optional · Markdown and the reviewer help line",
        &args,
    );
    app.finish();
}

fn option_screen(name: &str, opt: &str) -> App {
    let args = ["--page", "forms", "--opt", opt];
    let app = open(name, 120, 40, &args);
    app.wait_text("Playground");
    click_tab(&app, "Options");
    app.wait_text("Page options");
    app
}

fn back_to_playground(app: &App) {
    click_tab(app, "Playground");
    app.wait_text(CAP_PLAY);
}

#[test]
fn option_boundaries_keep_siblings() {
    let app = option_screen("s02-forms-required", "forms.required=off");
    let options = app.screen();
    assert_option_except(&options, "Task name required", "off", Some(false));
    back_to_playground(&app);
    require(
        &app,
        &[
            PAGE,
            "Task name  optional",
            NAME_PLACEHOLDER,
            HELP,
            REVIEWER_HINT,
            MANAGED,
            "(●) Balanced",
        ],
        &[
            "Task name *",
            "Creating task",
            "Help text off",
            "Placeholders off",
        ],
    );
    see(
        &app,
        "opt.forms.required",
        "forms.required=off shows Task name optional and leaves help, placeholders, and managed on",
        &["--page", "forms", "--opt", "forms.required=off"],
    );
    app.finish();

    let app = option_screen("s02-forms-when", "forms.validate_when=live");
    let options = app.screen();
    assert_option_except(&options, "Validate", "live", None);
    back_to_playground(&app);
    click_filtered(&app, NAME_PLACEHOLDER, |hit| hit.x > 24);
    app.type_text("ab");
    let live = app.wait_text("At least 4 characters");
    require(
        &app,
        &[
            PAGE,
            "At least 4 characters",
            "Task name *",
            "ab",
            HELP,
            MANAGED,
            "(●) Balanced",
        ],
        &[
            "Creating task",
            "Fix the highlighted fields",
            "Required",
            "Task created",
            NAME_PLACEHOLDER,
        ],
    );
    assert_size(&live, 120, 40);
    see(
        &app,
        "opt.forms.validate_when",
        "forms.validate_when=live shows At least 4 characters while typing and does not submit",
        &["--page", "forms", "--opt", "forms.validate_when=live"],
    );
    app.finish();

    let app = option_screen("s02-forms-blur", "forms.blur=cancel");
    let options = app.screen();
    assert_option_except(&options, "On blur", "cancel", None);
    back_to_playground(&app);
    click_filtered(&app, NAME_PLACEHOLDER, |hit| hit.x > 24);
    app.type_text(SUBMITTED_NAME);
    app.wait_text(SUBMITTED_NAME);
    click_filtered(&app, DESC_A, |hit| hit.x > 24);
    let cancelled = app.wait_text(NAME_PLACEHOLDER);
    require(
        &app,
        &[PAGE, NAME_PLACEHOLDER, "Task name *", HELP, MANAGED],
        &[
            SUBMITTED_NAME,
            "Creating task",
            "At least 4 characters",
            "Form reset",
        ],
    );
    assert_size(&cancelled, 120, 40);
    see(
        &app,
        "opt.forms.blur",
        "forms.blur=cancel drops the typed task name when the description is clicked and restores the placeholder",
        &["--page", "forms", "--opt", "forms.blur=cancel"],
    );
    app.finish();

    let app = option_screen("s02-forms-rows", "forms.desc_rows=2");
    let options = app.screen();
    assert_option_except(&options, "Description rows", "2", None);
    back_to_playground(&app);
    let screen = app.wait_text(DESC_B);
    let visible = text(&screen);
    let lines: Vec<&str> = visible.lines().collect();
    let index = lines
        .iter()
        .position(|line| line.contains(DESC_B))
        .expect("description placeholder");
    let next = lines.get(index + 1).copied().unwrap_or("");
    assert!(
        next.contains(HELP),
        "two description rows should put the help on the next line, got {next:?}\n{}",
        text(&screen)
    );
    require(
        &app,
        &[
            PAGE,
            "Task name *",
            NAME_PLACEHOLDER,
            DESC_A,
            HELP,
            MANAGED,
            "(●) Balanced",
        ],
        &["Task name  optional", "Creating task", "Placeholders off"],
    );
    see(
        &app,
        "opt.forms.desc_rows",
        "forms.desc_rows=2 places Optional · Markdown on the line after the placeholder",
        &["--page", "forms", "--opt", "forms.desc_rows=2"],
    );
    app.finish();

    let app = option_screen("s02-forms-placeholders", "forms.placeholders=off");
    let options = app.screen();
    assert_option_except(&options, "Placeholders", "off", Some(false));
    back_to_playground(&app);
    require(
        &app,
        &[
            PAGE,
            "Task name *",
            HELP,
            "Reviewer  optional",
            MANAGED,
            "(●) Balanced",
            "Create task",
        ],
        &[
            NAME_PLACEHOLDER,
            DESC_A,
            DESC_B,
            REVIEWER_HINT,
            "Placeholders on",
            "Task name  optional",
            "Creating task",
        ],
    );
    see(
        &app,
        "opt.forms.placeholders",
        "forms.placeholders=off clears the three placeholders and leaves help and required on",
        &["--page", "forms", "--opt", "forms.placeholders=off"],
    );
    app.finish();

    let app = option_screen("s02-forms-managed", "forms.managed=off");
    let options = app.screen();
    assert_option_except(&options, "Managed toggle", "off", Some(false));
    back_to_playground(&app);
    require(
        &app,
        &[
            PAGE,
            "Task name *",
            NAME_PLACEHOLDER,
            HELP,
            REVIEWER_HINT,
            "Notify on completion on",
            "(●) Balanced",
            "Create task",
        ],
        &[
            MANAGED,
            "Task name  optional",
            "Help text off",
            "Placeholders off",
            "Creating task",
        ],
    );
    see(
        &app,
        "opt.forms.managed",
        "forms.managed=off removes Managed by your organization and leaves notify on",
        &["--page", "forms", "--opt", "forms.managed=off"],
    );
    app.finish();
}
