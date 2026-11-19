//! Splits & stacks route, sections, specimens, compare levels, seams, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s07_splitsstacks/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Layout › Splits & stacks";
const TITLE: &str = "Splits & stacks";
const BLURB: &str = "Split panes with draggable seams and zoom, stacks that shrink";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "splitsstacks"];
const META: &str = "30% · min 16 · drag a seam, double-click resets";
const EDITOR_MARK: &str = "pub fn toggle_max";
const OUTPUT_MARK: &str = "Compiling junie-tui";
const GUTTER: char = '▎';

const OMITTED: &[&str] = &[
    "focused",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "error",
    "editing",
    "empty",
    "loading",
    "overflow",
    "narrow",
];

const DRAWN: &[&str] = &["rest", "hover", "pressed"];

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s07_splitsstacks/{}",
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

fn page(name: &str, opt: &str, cols: u16, rows: u16) -> App {
    let args = ["--page", "splitsstacks", "--opt", opt];
    open(name, cols, rows, &args)
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
            "forbidden {item:?} is visible\n{}",
            text(screen)
        );
    }
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
}

fn value_under(screen: &TerminalSnapshot, label: &str) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = lines
        .iter()
        .enumerate()
        .find(|(row, line)| *row >= 6 && line.contains(label))
        .map(|(row, _)| row)
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    let column = lines[index].find(label).expect("label column");
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).map(String::as_str).unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    rest.split_whitespace()
        .find(|token| *token != "▎")
        .unwrap_or("")
        .to_string()
}

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str, others: &[&str]) {
    let got = value_under(screen, label);
    assert_eq!(got, value, "{label} shows {got:?}, expected {value}");
    for other in others {
        assert_ne!(got, *other, "{label} shows the other value {other}");
    }
}

/// Grid labels sit under the column titles. The legend's "focused" is higher.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (20..48).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn content_hit(screen: &TerminalSnapshot, label: &str) -> Point {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 26)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one content {label:?}, got {hits:?}\n{}",
        text(screen)
    );
    hits[0]
}

fn click_where(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
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
    app.click(hits[0]);
    hits[0]
}

fn click_tab(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &App, label: &str) {
    reveal_side(app, label);
    click_where(app, label, |hit| hit.x < 24);
}

fn click_button(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 4 && hit.x >= 26);
}

fn reveal_side(app: &App, label: &str) {
    let point = Point { x: 8, y: 16 };
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel_applied(point, false, 3);
    }
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
}

fn reveal_text(app: &App, needle: &str) {
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel_applied(point, false, 3);
    }
    app.wait_text(needle);
}

fn assert_size(app: &App, cols: u16, rows: u16) {
    let screen = app.settle();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (u32::from(cols), u32::from(rows)),
        "terminal size\n{}",
        text(&screen)
    );
}

fn gutter_count(screen: &TerminalSnapshot) -> usize {
    (0..screen.rows())
        .filter(|row| row_text(screen, *row).contains(GUTTER))
        .count()
}

fn shot(app: &App, case_id: &str) {
    let root = crate::support::oracle_baselines();
    let id = visual_id(case_id);
    let outcome = app.check_visual(&root, &id);
    assert!(
        app.accepts(&outcome),
        "visual mismatch for {case_id}: {outcome:?}"
    );
}

fn glyph_at(screen: &TerminalSnapshot, point: Point) -> char {
    row_text(screen, u32::from(point.y))
        .chars()
        .nth(point.x as usize)
        .unwrap_or('?')
}

fn vertical_seam(screen: &TerminalSnapshot) -> Point {
    let editor = content_hit(screen, "Editor");
    Point {
        x: editor.x - 4,
        y: editor.y + 2,
    }
}

fn assert_only_column(before: &TerminalSnapshot, after: &TerminalSnapshot, column: u16) {
    assert_eq!((before.cols(), before.rows()), (after.cols(), after.rows()));
    let mut changed = false;
    for y in 0..before.rows() {
        for x in 0..before.cols() {
            let left = before.cell_at(x, y);
            let right = after.cell_at(x, y);
            if left != right {
                assert_eq!(
                    x,
                    u32::from(column),
                    "cell {x},{y} changed outside the seam\n{}",
                    text(after)
                );
                changed = true;
            }
        }
    }
    assert!(changed, "the seam column did not restyle");
}

fn assert_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Three panes",
            META,
            "Zoom editor",
            "Zoom output",
            "Restore",
            "Explorer",
            "Editor",
            "splitter.rs",
            OUTPUT_MARK,
            EDITOR_MARK,
            "Stack",
            "15 rows wanted, 8 given",
            "Connection · 3 rows",
            "Credentials · 2 rows",
            "Network · 1 row",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "editor zoomed",
            "output zoomed",
            "not in view",
            "min 40",
            "min 3",
        ],
    );
    let editor = content_hit(screen, "Editor");
    let seam = vertical_seam(screen);
    assert_eq!(
        glyph_at(screen, seam),
        '│',
        "rest seam is not the thin rule"
    );
    assert_eq!(editor.x, 58, "default editor origin");
    assert_eq!(gutter_count(screen), 1, "more than one focus gutter");
}

fn assert_omitted(screen: &TerminalSnapshot) {
    for label in DRAWN {
        assert!(
            grid_label(screen, label),
            "states grid clipped {label}\n{}",
            text(screen)
        );
    }
    for label in OMITTED {
        assert!(
            !grid_label(screen, label),
            "{label} is drawn, but states_grid lines 408-423 only list Rest, Hover, and Pressed\n{}",
            text(screen)
        );
    }
}

fn confirm_options(app: &App, playground: &str, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text(playground);
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s07-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    flash_settle(&app);
    let settled = app.settle();
    assert_playground(&settled);
    shot(&app, "sc.splitsstacks.route");
    app.finish();

    let app = open("s07-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    let stops = [
        "App shell › Chrome",
        "App shell › Status & hints",
        "App shell › Breadcrumbs & rails",
        "App shell › Help & keys",
        "App shell › Quit & leave",
        "App shell › Focus & navigation",
        "Inputs & forms › Buttons",
        "Inputs & forms › Inputs",
        "Inputs & forms › Text areas",
        "Inputs & forms › Chips & selects",
        "Inputs & forms › Forms",
        "Inputs & forms › Checks & radios",
        "Inputs & forms › Toolbars",
        "Inputs & forms › Fixed controls",
        "Tables › Basic table",
        "Tables › Editable",
        "Tables › Data grid",
        "Tables › Selection",
        "Tables › Cell editors",
        "Tables › Columns",
        "Navigation › Lists",
        "Navigation › Trees",
        "Navigation › Sidebars",
        "Navigation › Tabs",
        "Navigation › Menus",
        "Overlays › Dialogs",
        "Overlays › Pickers",
        "Overlays › Dialog kinds",
        "Overlays › Popups & tooltips",
        "Overlays › File browser",
        "Overlays › Fixed dialogs",
        "Feedback › Progress",
        "Feedback › Empty & error",
        "Feedback › Meters",
        "Feedback › Notices",
        "Layout › Panels",
        "Layout › Scrolling",
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Three panes");
    see(
        &entered,
        &[CRUMB, BLURB, "Zoom editor", META],
        &[OVERVIEW, "Page options", "Layout › Scrolling"],
    );
    app.key("[");
    let left = app.wait_text("Layout › Scrolling");
    see(
        &left,
        &["Layout › Scrolling"],
        &[CRUMB, "Three panes", BLURB],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Zoom editor", "Explorer"],
        &["Layout › Scrolling", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Three panes", BLURB]);
    click_side(&app, "Layout");
    app.wait_text("Panels");
    click_side(&app, "Splits & stacks");
    let pointed = app.wait_text("Three panes");
    see(
        &pointed,
        &[CRUMB, BLURB, "Zoom editor", META, "Connection · 3 rows"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s07-sections", 80, 24, PAGE_ARGS);
    app.wait_text("Zoom editor");
    flash_settle(&app);
    let narrow_play = app.settle();
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "draggable seams",
            "Playground",
            "Zoom editor",
            "30%",
            "min 16",
            "Explorer",
            "Editor",
            "splitter.rs",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "scroll or clip",
            OUTPUT_MARK,
            "Connection",
            "not in view",
        ],
    );
    assert_eq!(gutter_count(&narrow_play), 1, "narrow playground gutters");
    shot(&app, "sc.splitsstacks.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_select(&narrow_opt, "Split direction", "horizontal", &["vertical"]);
    assert_select(&narrow_opt, "First pane %", "30", &["50", "70"]);
    assert_select(&narrow_opt, "First pane minimum", "16", &["40"]);
    assert_select(&narrow_opt, "Zoomed pane", "none", &["first", "second"]);
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Split direction",
            "Seam double-click",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Three panes",
            WIDE,
            "truecolor",
            "Stack child minimum",
        ],
    );
    shot(&app, "sc.splitsstacks.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert_omitted(&narrow_states);
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "Vertical seam",
            "Horizontal seam",
            "rest",
            "hover",
            "pressed",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Three panes",
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    let rest = row_with(&narrow_states, "rest");
    assert!(
        rest.contains('│') && rest.contains('─') && !rest.contains('┃'),
        "narrow rest seam row {rest:?}"
    );
    let hover = row_with(&narrow_states, "hover");
    assert!(
        hover.contains('┃') && hover.contains('━'),
        "narrow hover seam row {hover:?}"
    );
    shot(&app, "sc.splitsstacks.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "pressed")
    });
    assert_size(&app, 120, 40);
    assert_omitted(&wide_states);
    see(
        &wide_states,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "Vertical seam",
            "Horizontal seam",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Three panes",
            "256 colors",
        ],
    );
    assert_eq!(gutter_count(&wide_states), 1, "states focus gutter");
    let rest = row_with(&wide_states, "rest");
    assert!(
        rest.contains('│') && rest.contains("────────────") && !rest.contains('┃'),
        "rest specimen is not a thin seam: {rest:?}"
    );
    let hover = row_with(&wide_states, "hover");
    assert!(
        hover.contains('┃') && hover.contains("━━━━━━━━━━━━"),
        "hover specimen is not a heavy seam: {hover:?}"
    );
    let pressed = row_with(&wide_states, "pressed");
    assert!(
        pressed.contains('┃') && pressed.contains("━━━━━━━━━━━━"),
        "pressed specimen is not a heavy seam: {pressed:?}"
    );
    for (case_id, label) in [
        ("sc.splitsstacks.specimen.rest", "rest"),
        ("sc.splitsstacks.specimen.hover", "hover"),
        ("sc.splitsstacks.specimen.pressed", "pressed"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.splitsstacks.states.120x40");
    shot(&app, "sc.splitsstacks.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_select(&wide_opt, "Split direction", "horizontal", &["vertical"]);
    assert_select(&wide_opt, "First pane %", "30", &["50", "70"]);
    assert_select(&wide_opt, "First pane minimum", "16", &["40"]);
    assert_select(&wide_opt, "Zoomed pane", "none", &["first", "second"]);
    assert_select(&wide_opt, "Seam double-click", "reset", &["ignore"]);
    assert_select(&wide_opt, "Seam hit padding", "1", &["0", "3"]);
    assert_select(&wide_opt, "Stack overflow", "scroll", &["clip"]);
    assert_select(&wide_opt, "Stack child minimum", "1", &["3"]);
    assert_select(&wide_opt, "Stack gap", "0", &["1"]);
    assert_select(&wide_opt, "Stack scrollbar gap", "1", &["0", "2"]);
    assert_select(&wide_opt, "Stack track click", "page", &["jump"]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Three panes"],
    );
    shot(&app, "sc.splitsstacks.options.120x40");
    shot(&app, "sc.splitsstacks.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Three panes");
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.splitsstacks.playground.120x40");
    shot(&app, "sc.splitsstacks.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s07-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[
            CRUMB,
            "○ Compare levels",
            "rest",
            "Vertical seam",
            CAP_STATES,
        ],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    app.wait_text("● Compare levels");
    flash_settle(&app);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[CRUMB, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel_applied(point, true, 3);
    }
    let bands = app.settle();
    assert_size(&app, 120, 40);
    see(
        &bands,
        &[CRUMB, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    assert!(
        saw_256 && saw_16,
        "a colour band was skipped before no color\n{}",
        text(&bands)
    );
    shot(&app, "sc.splitsstacks.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    // The first paint after a shrink can still show the wide scroll offset.
    // Decide from the settled frame, then use the same wheel as the wide reveal.
    let mut narrow = app.settle();
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
        narrow = app.settle();
    }
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.splitsstacks.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    let top = app.settle();
    see(
        &top,
        &[TITLE, "truecolor", "● Compare levels"],
        &["no color", "Page options"],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest", "Vertical seam"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "truecolor",
            "Page options",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_select(&options, "Split direction", "horizontal", &["vertical"]);
    assert_select(&options, "First pane %", "30", &["50", "70"]);
    assert_select(&options, "Zoomed pane", "none", &["first", "second"]);
    assert_select(&options, "Stack overflow", "scroll", &["clip"]);
    see(
        &options,
        &["horizontal", "Page options", CAP_OPT, CRUMB],
        &["vertical", "Compare levels", OVERVIEW],
    );
    app.finish();
}

/// Leave the pointer on a blank header cell so the next comparison is the seam.
fn park(app: &App) {
    app.move_to(Point { x: 40, y: 1 });
    app.settle();
}

fn drag_vertical(app: &App, from: Point, dx: i16) -> Point {
    let target = Point {
        x: from.x.saturating_add_signed(dx),
        y: from.y,
    };
    app.down(from);
    app.drag_to(target);
    target
}

#[test]
fn seam_drag_minimum_double_click_and_zoom() {
    let app = open("s07-splits", 120, 40, PAGE_ARGS);
    let rest = app.wait_text("Three panes");
    assert_playground(&rest);
    flash_settle(&app);
    let rest = app.settle();
    assert_playground(&rest);
    shot(&app, "sc.splitsstacks.splits.before");

    let seam = vertical_seam(&rest);
    park(&app);
    let parked = app.settle();
    app.move_to(seam);
    let hovered = app.settle();
    assert_eq!(
        glyph_at(&hovered, seam),
        '┃',
        "hover did not thicken the seam"
    );
    assert_only_column(&parked, &hovered, seam.x);
    let output = content_hit(&hovered, "Output");
    let across = row_text(&hovered, u32::from(output.y - 1));
    assert!(
        across.contains('─') && !across.contains('━'),
        "vertical hover restyled the horizontal seam: {across:?}"
    );

    let held_at = drag_vertical(&app, seam, 10);
    let held = app.settle();
    let editor = content_hit(&held, "Editor");
    assert_eq!(editor.x, 68, "drag did not move the editor by 10");
    assert_eq!(
        glyph_at(&held, held_at),
        '┃',
        "held seam is not the heavy rule"
    );
    see(
        &held,
        &["41%", "splitter.rs", OUTPUT_MARK, EDITOR_MARK],
        &["30%", "editor zoomed", "Page options"],
    );
    let output = content_hit(&held, "Output");
    let across = row_text(&held, u32::from(output.y - 1));
    assert!(
        across.contains('─') && !across.contains('━'),
        "the drag restyled the other seam: {across:?}"
    );
    shot(&app, "sc.splitsstacks.splits.after");
    app.up(held_at);
    flash_settle(&app);
    let released = app.settle();
    assert_eq!(
        content_hit(&released, "Editor").x,
        68,
        "release snapped back"
    );
    see(&released, &["41%"], &["30%", "editor zoomed"]);

    let seam = vertical_seam(&released);
    app.click(seam);
    app.click(seam);
    flash_settle(&app);
    park(&app);
    let reset = app.settle();
    assert_playground(&reset);

    let output = content_hit(&reset, "Output");
    let across = Point {
        x: output.x,
        y: output.y - 1,
    };
    assert_eq!(
        glyph_at(&reset, across),
        '─',
        "horizontal seam was not at rest"
    );
    app.down(across);
    let lower = Point {
        x: across.x,
        y: across.y + 2,
    };
    app.drag_to(lower);
    let dragging = app.settle();
    let moved = content_hit(&dragging, "Output");
    assert!(
        moved.y > output.y,
        "horizontal drag left Output at {moved:?}, was {output:?}"
    );
    assert_eq!(
        glyph_at(&dragging, vertical_seam(&dragging)),
        '│',
        "horizontal drag thickened the vertical seam"
    );
    let heavy = row_text(&dragging, u32::from(lower.y));
    assert!(
        heavy.contains('━'),
        "held horizontal seam is not heavy: {heavy:?}"
    );
    app.up(lower);
    flash_settle(&app);
    let after = app.settle();
    let seam = Point {
        x: content_hit(&after, "Output").x,
        y: content_hit(&after, "Output").y - 1,
    };
    app.click(seam);
    app.click(seam);
    flash_settle(&app);
    let reset = app.settle();
    assert_eq!(
        content_hit(&reset, "Output").y,
        output.y,
        "double-click did not restore the horizontal seam"
    );

    let seam = vertical_seam(&reset);
    app.down(seam);
    // x=32 is left of the 16-column minimum and still inside the pane, so the
    // drag clamps without crossing into the sidebar.
    app.drag_to(Point { x: 32, y: seam.y });
    let clamped = app.settle();
    let editor = content_hit(&clamped, "Editor");
    assert!(
        editor.x > 44 && editor.x < 58,
        "seam ignored its minimum or did not move: editor {editor:?}"
    );
    see(
        &clamped,
        &["Cargo.toml", EDITOR_MARK, "Explorer", "min 16"],
        &["editor zoomed", "30%"],
    );
    app.up(Point {
        x: editor.x - 4,
        y: seam.y,
    });
    flash_settle(&app);
    click_button(&app, "Restore");
    flash_settle(&app);
    let restored = app.settle();
    assert_playground(&restored);

    click_button(&app, "Zoom editor");
    app.wait_text("editor zoomed");
    flash_settle(&app);
    let zoomed = app.settle();
    see(
        &zoomed,
        &["editor zoomed", EDITOR_MARK, "Explorer", "30%"],
        &[
            OUTPUT_MARK,
            "output zoomed",
            "drag a seam, double-click resets",
        ],
    );
    click_button(&app, "Restore");
    app.wait_text(OUTPUT_MARK);
    flash_settle(&app);
    let back = app.settle();
    see(
        &back,
        &[EDITOR_MARK, OUTPUT_MARK, META],
        &["editor zoomed", "output zoomed"],
    );

    click_button(&app, "Zoom output");
    app.wait_text("output zoomed");
    flash_settle(&app);
    let zoomed = app.settle();
    see(
        &zoomed,
        &["output zoomed", OUTPUT_MARK, "Explorer"],
        &[
            EDITOR_MARK,
            "editor zoomed",
            "drag a seam, double-click resets",
        ],
    );
    click_button(&app, "Restore");
    app.wait_text(EDITOR_MARK);
    flash_settle(&app);
    let back = app.settle();
    assert_playground(&back);
    app.finish();
}

#[test]
fn option_dir_vertical() {
    let app = page("s07-dir", "splitsstacks.dir=vertical", 120, 40);
    app.wait_text("Explorer");
    confirm_options(&app, "min 3", |screen| {
        assert_select(screen, "Split direction", "vertical", &["horizontal"]);
        assert_select(screen, "First pane %", "30", &["50", "70"]);
        assert_select(screen, "First pane minimum", "16", &["40"]);
        assert_select(screen, "Zoomed pane", "none", &["first", "second"]);
    });
    finish_option(&app, "opt.splitsstacks.dir", |screen| {
        let explorer = content_hit(screen, "Explorer");
        let editor = content_hit(screen, "Editor");
        let output = content_hit(screen, "Output");
        assert!(
            editor.y > explorer.y,
            "vertical direction kept Editor beside Explorer: editor {editor:?} explorer {explorer:?}"
        );
        assert_eq!(
            output.y, editor.y,
            "the inner pair did not stay side by side"
        );
        see(
            screen,
            &["min 3", "drag a seam", EDITOR_MARK, OUTPUT_MARK],
            &["min 16", "editor zoomed", "Page options", "Connection"],
        );
    });
}

#[test]
fn option_percent_50() {
    let app = page("s07-percent", "splitsstacks.percent=50", 120, 40);
    app.wait_text("50%");
    confirm_options(&app, "50%", |screen| {
        assert_select(screen, "First pane %", "50", &["30", "70"]);
        assert_select(screen, "Split direction", "horizontal", &["vertical"]);
        assert_select(screen, "First pane minimum", "16", &["40"]);
    });
    finish_option(&app, "opt.splitsstacks.percent", |screen| {
        let editor = content_hit(screen, "Editor");
        assert!(
            editor.x >= 80,
            "50% did not widen the explorer: editor {editor:?}"
        );
        see(
            screen,
            &["50%", "min 16", "splitter.rs", OUTPUT_MARK],
            &["30%", "70%", "editor zoomed", "Page options"],
        );
    });
}

#[test]
fn option_min_40() {
    let app = page("s07-min", "splitsstacks.min=40", 120, 40);
    app.wait_text("min 40");
    confirm_options(&app, "min 40", |screen| {
        assert_select(screen, "First pane minimum", "40", &["16"]);
        assert_select(screen, "First pane %", "30", &["50", "70"]);
        assert_select(screen, "Split direction", "horizontal", &["vertical"]);
    });
    finish_option(&app, "opt.splitsstacks.min", |screen| {
        let editor = content_hit(screen, "Editor");
        assert!(
            editor.x > 58,
            "minimum 40 did not push the editor: {editor:?}"
        );
        see(
            screen,
            &["min 40", "30%", "splitter.rs"],
            &["min 16", "editor zoomed", "Page options"],
        );
    });
}

#[test]
fn option_maximized_first() {
    let app = page("s07-max", "splitsstacks.maximized=first", 120, 40);
    app.wait_text("editor zoomed");
    confirm_options(&app, "editor zoomed", |screen| {
        assert_select(screen, "Zoomed pane", "first", &["none", "second"]);
        assert_select(screen, "Split direction", "horizontal", &["vertical"]);
        assert_select(screen, "First pane %", "30", &["50", "70"]);
    });
    finish_option(&app, "opt.splitsstacks.maximized", |screen| {
        see(
            screen,
            &[
                "editor zoomed",
                EDITOR_MARK,
                "Explorer",
                "Danger zone · 1 row",
            ],
            &[
                OUTPUT_MARK,
                "output zoomed",
                "drag a seam, double-click resets",
                "Page options",
            ],
        );
    });
}

#[test]
fn option_double_click_ignore() {
    let app = page("s07-double", "splitsstacks.double_click=ignore", 120, 40);
    app.wait_text("Editor");
    confirm_options(&app, "Editor", |screen| {
        assert_select(screen, "Seam double-click", "ignore", &["reset"]);
        assert_select(screen, "Split direction", "horizontal", &["vertical"]);
        assert_select(screen, "Seam hit padding", "1", &["0", "3"]);
    });
    let start = content_hit(&app.settle(), "Editor");
    let seam = vertical_seam(&app.screen());
    let held_at = drag_vertical(&app, seam, 10);
    app.up(held_at);
    flash_settle(&app);
    let dragged = app.settle();
    assert_eq!(content_hit(&dragged, "Editor").x, start.x + 10);
    see(&dragged, &["41%"], &["30%"]);
    let seam = vertical_seam(&dragged);
    app.click(seam);
    app.click(seam);
    flash_settle(&app);
    finish_option(&app, "opt.splitsstacks.double_click", |screen| {
        assert_eq!(
            content_hit(screen, "Editor").x,
            start.x + 10,
            "ignore still reset the seam"
        );
        see(
            screen,
            &["41%", "splitter.rs", OUTPUT_MARK],
            &["30%", "editor zoomed", "Page options"],
        );
    });
}

#[test]
fn option_hit_pad_0() {
    let app = page("s07-pad", "splitsstacks.hit_pad=0", 120, 40);
    app.wait_text("Editor");
    confirm_options(&app, "Editor", |screen| {
        assert_select(screen, "Seam hit padding", "0", &["1", "3"]);
        assert_select(screen, "Seam double-click", "reset", &["ignore"]);
        assert_select(screen, "Split direction", "horizontal", &["vertical"]);
    });
    park(&app);
    let rest = app.settle();
    let seam = vertical_seam(&rest);
    app.move_to(seam);
    let on_seam = app.settle();
    assert_eq!(glyph_at(&on_seam, seam), '┃', "pad 0 missed the seam cell");
    assert_only_column(&rest, &on_seam, seam.x);
    let beside = Point {
        x: seam.x + 1,
        y: seam.y,
    };
    app.move_to(beside);
    let off = app.settle();
    assert_eq!(
        glyph_at(&off, seam),
        '│',
        "pad 0 still hovered the neighbouring cell"
    );
    finish_option(&app, "opt.splitsstacks.hit_pad", |screen| {
        assert_eq!(content_hit(screen, "Editor").x, 58);
        see(
            screen,
            &[META, "splitter.rs"],
            &["editor zoomed", "Page options", "min 40"],
        );
    });
}

#[test]
fn option_overflow_clip() {
    let app = page("s07-clip", "splitsstacks.overflow=clip", 120, 40);
    app.wait_text("Credentials");
    confirm_options(&app, "Credentials", |screen| {
        assert_select(screen, "Stack overflow", "clip", &["scroll"]);
        assert_select(screen, "Stack child minimum", "1", &["3"]);
        assert_select(screen, "Stack gap", "0", &["1"]);
    });
    finish_option(&app, "opt.splitsstacks.overflow", |screen| {
        see(
            screen,
            &[
                "Credentials · 2 rows",
                "Network · 1 row",
                META,
                "15 rows wanted, 8 given",
            ],
            &[
                "not in view",
                "Credentials · 3 rows",
                "Page options",
                "min 3",
            ],
        );
    });
}

#[test]
fn option_min_child_rows_3() {
    let app = page("s07-minrows", "splitsstacks.min_child_rows=3", 120, 40);
    app.wait_text("not in view");
    confirm_options(&app, "not in view", |screen| {
        assert_select(screen, "Stack child minimum", "3", &["1"]);
        assert_select(screen, "Stack overflow", "scroll", &["clip"]);
        assert_select(screen, "Stack gap", "0", &["1"]);
        assert_select(screen, "First pane %", "30", &["50", "70"]);
    });
    finish_option(&app, "opt.splitsstacks.min_child_rows", |screen| {
        see(
            screen,
            &["Credentials · 3 rows", "3 not in view", META],
            &[
                "Credentials · 2 rows",
                "Network · 1 row",
                "Page options",
                "min 40",
            ],
        );
    });
}

#[test]
fn option_gap_1() {
    let app = page("s07-gap", "splitsstacks.gap=1", 120, 40);
    app.wait_text("not in view");
    confirm_options(&app, "not in view", |screen| {
        assert_select(screen, "Stack gap", "1", &["0"]);
        assert_select(screen, "Stack child minimum", "1", &["3"]);
        assert_select(screen, "Stack overflow", "scroll", &["clip"]);
    });
    finish_option(&app, "opt.splitsstacks.gap", |screen| {
        let connection = content_hit(screen, "Connection");
        let credentials = content_hit(screen, "Credentials");
        assert!(
            credentials.y >= connection.y + 4,
            "gap 1 did not separate the cards: connection {connection:?} credentials {credentials:?}"
        );
        see(
            screen,
            &["3 not in view", "Credentials · 3 rows", META],
            &["Credentials · 2 rows", "Page options", "min 3"],
        );
    });
}

#[test]
fn option_scrollbar_gap_0() {
    let app = page("s07-sgap", "splitsstacks.scrollbar_gap=0", 120, 40);
    app.wait_text("Credentials");
    confirm_options(&app, "Credentials", |screen| {
        assert_select(screen, "Stack scrollbar gap", "0", &["1", "2"]);
        assert_select(screen, "Stack track click", "page", &["jump"]);
        assert_select(screen, "Stack overflow", "scroll", &["clip"]);
        assert_select(screen, "Stack child minimum", "1", &["3"]);
    });
    finish_option(&app, "opt.splitsstacks.scrollbar_gap", |screen| {
        see(
            screen,
            &["Credentials · 2 rows", "Network · 1 row", META],
            &["not in view", "Credentials · 3 rows", "Page options"],
        );
    });
}

#[test]
fn option_track_click_jump() {
    let app = page("s07-jump", "splitsstacks.track_click=jump", 120, 40);
    app.wait_text("Credentials");
    confirm_options(&app, "Credentials", |screen| {
        assert_select(screen, "Stack track click", "jump", &["page"]);
        assert_select(screen, "Stack scrollbar gap", "1", &["0", "2"]);
        assert_select(screen, "Stack overflow", "scroll", &["clip"]);
    });
    finish_option(&app, "opt.splitsstacks.track_click", |screen| {
        see(
            screen,
            &["Credentials · 2 rows", "Network · 1 row", META],
            &["not in view", "Credentials · 3 rows", "Page options"],
        );
    });
}

#[test]
fn escape_cancels_a_seam_drag_and_keeps_the_size_it_reached() {
    let app = open("s07-splits-esc", 120, 40, PAGE_ARGS);
    app.wait_text("Three panes");
    flash_settle(&app);
    let rest = app.settle();
    let seam = vertical_seam(&rest);
    let start = content_hit(&rest, "Editor").x;
    park(&app);

    let held_at = drag_vertical(&app, seam, 10);
    let held = app.settle();
    assert_eq!(content_hit(&held, "Editor").x, start + 10);

    app.key("escape");
    let cancelled = app.wait_screen("drag cancelled", |screen| {
        text(screen).contains("Drag cancelled: the seam stays where it is")
    });
    assert_eq!(
        content_hit(&cancelled, "Editor").x,
        start + 10,
        "cancelling kept the size the drag reached"
    );
    see(&cancelled, &["Splits & stacks"], &[]);

    app.drag_to(Point {
        x: held_at.x + 8,
        y: held_at.y,
    });
    let still = app.settle();
    assert_eq!(
        content_hit(&still, "Editor").x,
        start + 10,
        "the pointer still holds the seam but the grab is dropped"
    );
    app.up(held_at);
    flash_settle(&app);
    let released = app.settle();
    assert_eq!(content_hit(&released, "Editor").x, start + 10);

    let seam = vertical_seam(&released);
    let again = drag_vertical(&app, seam, -4);
    let moved = app.settle();
    assert_eq!(
        content_hit(&moved, "Editor").x,
        start + 6,
        "a fresh press grabs the seam again"
    );
    app.up(again);
    app.finish();
}
