//! Breadcrumbs & rails, driven through the showcase PTY.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so a dotted case id cannot be
//! a path under `tests/functional/baselines`. This file checks against the
//! owned directory and uses the case id with dots turned into underscores.

use std::path::PathBuf;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find, find_all, row_text, text, try_launch};

fn baseline_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/functional/baselines/pre_refactor/S01.breadcrumbsrails")
}

fn visual_id(case_id: &str) -> String {
    case_id.replace('.', "_")
}

fn launch(name: &str, program_args: &[&str], cols: u16, rows: u16) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = program_args.iter().map(|arg| (*arg).to_string()).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn page(name: &str, extra: &[&str], cols: u16, rows: u16) -> App {
    let mut args = vec!["--page", "breadcrumbsrails", "--frame", "40"];
    args.extend_from_slice(extra);
    launch(name, &args, cols, rows)
}

fn freeze(app: &App, case_id: &str, precondition: &str) {
    let id = visual_id(case_id);
    let outcome = app.check_visual_with_precondition(&baseline_root(), &id, precondition);
    // Same parked-cursor rule as `App::visual`: equal cells, cursor column differs.
    if outcome.is_pass() || App::cursor_only(&outcome) {
        return;
    }
    assert!(
        outcome.is_pass(),
        "visual mismatch for {case_id}: {outcome:?}"
    );
}

fn assert_has(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "expected {needle:?}\n{}",
        text(screen)
    );
}

fn assert_lacks(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "forbidden {needle:?}\n{}",
        text(screen)
    );
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

fn rail_row(screen: &TerminalSnapshot) -> Option<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("Last RailAction"))
}

fn first(screen: &TerminalSnapshot, needle: &str) -> Point {
    let mut matches = find_all(screen, needle);
    matches.sort_by_key(|point| (point.y, point.x));
    matches
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("missing {needle:?}\n{}", text(screen)))
}

fn click_first(app: &App, needle: &str) {
    let screen = app.settle();
    app.click(first(&screen, needle));
}

fn enter_page(app: &App) {
    app.key("right");
    let focused = app.wait_text("←→ Section");
    assert_has(&focused, "the live widget");
    assert_lacks(&focused, "one control per option");
}

fn playground_ready(screen: &TerminalSnapshot) -> bool {
    let body = text(screen);
    body.contains("Desktop") && body.contains("Disk › Users › alex")
}

fn playground_visible(screen: &TerminalSnapshot) {
    assert_has(screen, "Breadcrumbs & rails");
    assert_has(screen, "Explorer");
    assert_has(screen, "Desktop");
    assert_has(screen, "Disk › Users › alex");
    assert_lacks(screen, "Page options");
    assert_lacks(screen, "Compare levels");
    assert_lacks(screen, "one control per option");
    assert_lacks(screen, "every state, drawn by the widget");
}

fn options_visible(screen: &TerminalSnapshot) {
    assert_has(screen, "Page options");
    assert_has(screen, "Links");
    assert_has(screen, "Deep trail width");
    assert_has(screen, "Sidebar collapsed");
    assert_has(screen, "Collapsed rail");
    assert_lacks(screen, "Explorer");
    assert_lacks(screen, "Compare levels");
    assert_lacks(screen, "the live widget");
    assert_lacks(screen, "every state, drawn by the widget");
}

fn states_visible(screen: &TerminalSnapshot) {
    assert_has(screen, "Compare levels");
    assert_has(screen, "rest");
    assert_has(screen, "Clickable");
    assert_lacks(screen, "Page options");
    assert_lacks(screen, "Explorer");
    assert_lacks(screen, "the live widget");
    assert_lacks(screen, "one control per option");
}

fn still_this_page(screen: &TerminalSnapshot) {
    assert_has(screen, "App shell › Breadcrumbs & rails");
    assert_has(screen, "› Breadcrumbs");
    assert_lacks(screen, "App shell › Help & keys");
    assert_lacks(screen, "App shell › Status & hints");
}

#[test]
fn route_opens_by_page_keyboard_and_pointer() {
    let keys = launch("s01-route-keys", &["--frame", "40"], 120, 40);
    // The overview blurb paints while the breadcrumb trails are still on screen.
    let overview = keys.wait_screen("overview without the live trail", |screen| {
        let raw = text(screen);
        raw.contains("Tokens · planes and text") && !raw.contains("Disk › Users › alex")
    });
    assert_has(&overview, "Overview");
    assert_lacks(&overview, "Disk › Users › alex");
    keys.key("down");
    keys.key("right");
    keys.key("down");
    keys.key("down");
    keys.key("down");
    keys.key("right");
    let opened = keys.wait_text("Disk › Users › alex");
    assert_size(&opened, 120, 40);
    still_this_page(&opened);
    playground_visible(&opened);
    enter_page(&keys);
    keys.key("]");
    let left = keys.wait_screen("help page without the live trail", |screen| {
        let raw = text(screen);
        raw.contains("App shell › Help & keys")
            && raw.contains("KeyReference")
            && !raw.contains("Disk › Users › alex")
    });
    assert_has(&left, "KeyReference");
    assert_has(&left, "› Help");
    assert_lacks(&left, "Disk › Users › alex");
    assert_lacks(&left, "Breadcrumb trails");
    keys.key("[");
    let returned = keys.wait_text("Disk › Users › alex");
    still_this_page(&returned);
    playground_visible(&returned);
    assert_has(&returned, "the live widget");
    click_first(&keys, "Overview");
    let away = keys.wait_screen("overview again without the live trail", |screen| {
        let raw = text(screen);
        raw.contains("Tokens · planes and text")
            && !raw.contains("Disk › Users › alex")
            && !raw.contains("App shell › Breadcrumbs & rails")
    });
    assert_has(&away, "Overview");
    assert_lacks(&away, "Disk › Users › alex");
    assert_lacks(&away, "App shell › Breadcrumbs & rails");
    let shell = keys.settle();
    if find_all(&shell, "Breadcrumbs & ra").is_empty() {
        keys.click(find(&shell, "App shell"));
    }
    click_first(&keys, "Breadcrumbs & ra");
    let back = keys.wait_text("Disk › Users › alex");
    still_this_page(&back);
    playground_visible(&back);
    assert_has(&back, "the live widget");
    keys.finish();

    let direct = page("s01-route-page", &[], 120, 40);
    let screen = direct.wait_screen("route page", |screen| {
        playground_ready(screen) && text(screen).contains("the live widget")
    });
    assert_size(&screen, 120, 40);
    assert_has(&screen, "120×40");
    still_this_page(&screen);
    playground_visible(&screen);
    let location = row_containing(&screen, "Location");
    assert!(
        location.contains("clickable") && !location.contains("clickable(false)"),
        "default links stay on: {location}"
    );
    freeze(
        &direct,
        "sc.breadcrumbsrails.route",
        "Breadcrumbs & rails playground is open from --page breadcrumbsrails at 120 by 40, with caption the live widget",
    );
    direct.finish();
}

#[test]
fn playground_sections_at_both_sizes() {
    let wide = page("s01-play-120", &[], 120, 40);
    let screen = wide.wait_screen("playground body", playground_ready);
    assert_size(&screen, 120, 40);
    assert_has(&screen, "120×40");
    playground_visible(&screen);
    assert_has(&screen, "the live widget");
    freeze(
        &wide,
        "sc.breadcrumbsrails.playground.120x40",
        "Playground caption the live widget is active at 120 by 40 and Options and States captions are absent",
    );
    wide.key("right");
    let focused = wide.wait_text("←→ Section");
    playground_visible(&focused);
    assert_has(&focused, "the live widget");
    assert!(
        row_containing(&focused, "Playground").contains('▎'),
        "keyboard focus sits on Playground"
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.playground",
        "Playground tab has the keyboard focus gutter and caption the live widget",
    );
    wide.finish();

    let narrow = page("s01-play-80", &[], 80, 24);
    let screen = narrow.wait_screen("narrow playground", |screen| {
        screen.cols() == 80 && playground_ready(screen)
    });
    assert_size(&screen, 80, 24);
    playground_visible(&screen);
    assert_lacks(&screen, "the live widget");
    freeze(
        &narrow,
        "sc.breadcrumbsrails.playground.80x24",
        "Narrow playground shows Explorer and Disk › Users › alex before the terminal grows, with no section caption",
    );
    narrow.resize(120, 40);
    let grown = narrow.wait_screen("grown playground", |screen| {
        screen.cols() == 120 && text(screen).contains("the live widget") && playground_ready(screen)
    });
    assert_size(&grown, 120, 40);
    playground_visible(&grown);
    assert_has(&grown, "120×40");
    narrow.finish();
}

#[test]
fn options_sections_at_both_sizes() {
    let wide = page("s01-opt-120", &[], 120, 40);
    wide.wait_text("the live widget");
    wide.key("right");
    wide.key("2");
    let screen = wide.wait_text("Links on");
    assert_size(&screen, 120, 40);
    options_visible(&screen);
    assert_has(&screen, "Links on");
    assert_has(&screen, "full");
    assert_has(&screen, "Sidebar collapsed off");
    assert_has(&screen, "Collapsed rail on");
    freeze(
        &wide,
        "sc.breadcrumbsrails.options.120x40",
        "Options caption one control per option is active at 120 by 40",
    );
    wide.finish();

    let pointer = page("s01-opt-click", &[], 120, 40);
    let initial = pointer.wait_text("Playground");
    pointer.click(find(&initial, "Options"));
    let screen = pointer.wait_text("Links on");
    options_visible(&screen);
    assert_has(&screen, "Links on");
    freeze(
        &pointer,
        "sc.breadcrumbsrails.options",
        "Options opened by a pointer click shows one control per option",
    );
    pointer.finish();

    let narrow = page("s01-opt-80", &[], 80, 24);
    narrow.wait_text("Breadcrumbs & rails");
    narrow.key("right");
    narrow.key("2");
    let screen = narrow.wait_text("Links on");
    assert_size(&screen, 80, 24);
    options_visible(&screen);
    assert_has(&screen, "Links on");
    assert_has(&screen, "Deep trail width");
    assert_lacks(&screen, "one control per option");
    freeze(
        &narrow,
        "sc.breadcrumbsrails.options.80x24",
        "Narrow options show Page options and Links on before the terminal grows",
    );
    narrow.resize(120, 40);
    let grown = narrow.wait_screen("grown options", |screen| {
        let body = text(screen);
        screen.cols() == 120 && body.contains("one control per option") && body.contains("Links on")
    });
    assert_size(&grown, 120, 40);
    options_visible(&grown);
    narrow.finish();
}

#[test]
fn states_sections_and_specimens() {
    let wide = page("s01-states-120", &[], 120, 40);
    wide.wait_text("the live widget");
    wide.key("right");
    wide.key("3");
    let screen = wide.wait_text("October");
    assert_size(&screen, 120, 40);
    states_visible(&screen);
    assert_has(&screen, "hover");
    assert_has(&screen, "pressed");
    assert_has(&screen, "overflow");
    assert_has(&screen, "narrow");
    assert_has(&screen, "October");
    assert_has(&screen, "… › alex › Projects");
    assert_lacks(&screen, "focus + hover");
    assert_lacks(&screen, "busy");
    assert_lacks(&screen, "editing");
    assert_lacks(&screen, "loading");
    let plain_hover = row_containing(&screen, "hover");
    assert!(
        plain_hover.contains('–'),
        "plain breadcrumb draws no hover specimen: {plain_hover}"
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.states.120x40",
        "States caption every state, drawn by the widget is active at 120 by 40",
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.specimen.rest",
        "States grid draws the rest row as Disk › Users › alex › Projects",
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.specimen.hover",
        "States grid draws the hover row and a dash for the plain column",
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.specimen.pressed",
        "States grid draws the pressed row",
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.specimen.overflow",
        "States grid draws the overflow row with October",
    );
    freeze(
        &wide,
        "sc.breadcrumbsrails.specimen.narrow",
        "States grid draws the narrow row as … › alex › Projects",
    );
    wide.finish();

    let pointer = page("s01-states-click", &[], 120, 40);
    let initial = pointer.wait_text("Playground");
    pointer.click(find(&initial, "States"));
    let screen = pointer.wait_text("October");
    states_visible(&screen);
    freeze(
        &pointer,
        "sc.breadcrumbsrails.states",
        "States opened by a pointer click shows every state, drawn by the widget",
    );
    pointer.finish();

    let narrow = page("s01-states-80", &[], 80, 24);
    narrow.wait_text("Breadcrumbs & rails");
    narrow.key("right");
    narrow.key("3");
    let screen = narrow.wait_text("rest");
    assert_size(&screen, 80, 24);
    states_visible(&screen);
    assert_lacks(&screen, "every state, drawn by the widget");
    freeze(
        &narrow,
        "sc.breadcrumbsrails.states.80x24",
        "Narrow states show Compare levels and the rest row before the terminal grows",
    );
    narrow.resize(120, 40);
    let grown = narrow.wait_screen("grown states", |screen| {
        let body = text(screen);
        screen.cols() == 120
            && body.contains("every state, drawn by the widget")
            && body.contains("October")
    });
    assert_size(&grown, 120, 40);
    states_visible(&grown);
    narrow.finish();
}

#[test]
fn compare_levels_scrolls_then_narrows_before_growing() {
    let app = page("s01-compare", &[], 120, 40);
    app.wait_text("the live widget");
    app.key("right");
    app.key("3");
    let states = app.wait_text("every state, drawn by the widget");
    let before = row_containing(&states, "Compare levels");
    assert!(before.contains('○'), "compare starts off: {before}");
    click_first(&app, "Compare levels");
    // The toggle paints on before the 256-color band. Both are the on state.
    let on = app.wait_screen("compare levels on", |screen| {
        let visible = text(screen);
        visible.contains("● Compare levels") && visible.contains("256 colors")
    });
    assert_has(&on, "256 colors");
    assert_has(&on, "every state, drawn by the widget");
    assert_lacks(&on, "16 colors");
    let mut saw_sixteen = false;
    let mut saw_none = false;
    for _ in 0..12 {
        let current = text(&app.screen());
        saw_sixteen |= current.contains("16 colors");
        saw_none |= current.contains("no color");
        if saw_sixteen && saw_none {
            break;
        }
        app.wheel(Point { x: 50, y: 18 }, true, 3);
        let _ = app.settle();
    }
    let scrolled = app.settle();
    assert_has(&scrolled, "16 colors");
    assert_has(&scrolled, "no color");
    still_this_page(&scrolled);
    freeze(
        &app,
        "sc.breadcrumbsrails.compare-levels.120x40",
        "Compare levels is on at 120 by 40 and the truecolor, 256, 16, and no color bands were visible",
    );
    app.resize(80, 24);
    let narrow = app.wait_screen("narrow compare", |screen| {
        screen.cols() == 80 && text(screen).contains("● Compare levels")
    });
    assert_size(&narrow, 80, 24);
    assert_has(&narrow, "● Compare levels");
    assert_has(&narrow, "truecolor");
    assert_lacks(&narrow, "Page options");
    freeze(
        &app,
        "sc.breadcrumbsrails.compare-levels.80x24",
        "Narrow compare-levels layout is captured at 80 by 24 before the terminal grows",
    );
    app.resize(120, 40);
    let grown = app.wait_screen("grown compare", |screen| {
        screen.cols() == 120 && text(screen).contains("120×40")
    });
    assert_size(&grown, 120, 40);
    click_first(&app, "Compare levels");
    let off = app.wait_text("○ Compare levels");
    assert_lacks(&off, "256 colors");
    assert_lacks(&off, "16 colors");
    assert_lacks(&off, "no color");
    still_this_page(&off);
    click_first(&app, "Options");
    let options = app.wait_text("Links on");
    assert_has(&options, "Links on");
    assert_has(&options, "Sidebar collapsed off");
    assert_has(&options, "Collapsed rail on");
    let width = row_containing(&options, "Deep trail width");
    assert!(
        !width.contains("20") && !width.contains("32") && !width.contains("48"),
        "width stayed full: {width}"
    );
    app.finish();
}

#[test]
fn navigate_folder_backspace_and_breadcrumb() {
    let app = page("s01-navigate", &[], 120, 40);
    // "the live widget" is the page caption. The detail pane paints Depth
    // on a later frame, and the first caption frame is still blank there.
    let before = app.wait_screen("explorer depth", |screen| {
        let visible = text(screen);
        visible.contains("the live widget") && visible.contains("Depth            3")
    });
    assert_has(&before, "Disk › Users › alex");
    assert_has(&before, "Desktop");
    assert_lacks(&before, "design-system");
    assert_has(&before, "Depth            3");
    still_this_page(&before);
    freeze(
        &app,
        "sc.breadcrumbsrails.navigate.before",
        "Path is Disk › Users › alex and the sidebar still selects Breadcrumbs & rails",
    );
    enter_page(&app);
    app.key("tab");
    app.wait_text("Enter Open");
    app.key("down");
    app.key("down");
    app.key("down");
    app.key("enter");
    let opened = app.wait_text("design-system");
    assert_has(&opened, "Disk › Users › alex › Projects");
    assert_has(&opened, "notes.md");
    assert_has(&opened, "Depth            4");
    assert_lacks(&opened, "Desktop");
    still_this_page(&opened);
    app.key("backspace");
    let up = app.wait_text("Backspace → level 2");
    assert_has(&up, "Disk › Users › alex");
    assert_has(&up, "Projects");
    assert_has(&up, "Desktop");
    assert_lacks(&up, "design-system");
    assert_has(&up, "Depth            3");
    still_this_page(&up);
    let crumb = app.settle();
    app.click(first(&crumb, "Disk › Users › alex"));
    let disk = app.wait_text("Disk (level 0)");
    assert_has(&disk, "Applications");
    assert_has(&disk, "Depth            1");
    assert_lacks(&disk, "Desktop");
    assert_lacks(&disk, "design-system");
    still_this_page(&disk);
    freeze(
        &app,
        "sc.breadcrumbsrails.navigate.after",
        "Breadcrumb click returns to Disk, Last crumb is Disk (level 0), and the sidebar selection is unchanged",
    );
    app.finish();
}

#[test]
fn rail_collapse_keeps_the_path() {
    let app = page("s01-rail", &[], 120, 40);
    // The first paint lists Desktop before the rail action row is drawn.
    let before = app.wait_screen("rail path painted", |screen| {
        let visible = text(screen);
        visible.contains("Desktop")
            && visible.contains("Disk › Users › alex")
            && visible.contains("the live widget")
            && rail_row(screen).is_some()
    });
    assert_has(&before, "Disk › Users › alex");
    assert_has(&before, "the live widget");
    let before_rail = row_containing(&before, "Last RailAction");
    assert!(
        before_rail.contains('—') && !before_rail.contains("Collapse"),
        "rail has not moved: {before_rail}"
    );
    still_this_page(&before);
    freeze(
        &app,
        "sc.breadcrumbsrails.rail.before",
        "Explorer is expanded on Disk › Users › alex before the rail collapses",
    );
    enter_page(&app);
    app.key("tab");
    app.wait_text("Enter Open");
    app.key("ctrl+b");
    let collapsed = app.wait_screen("explorer collapsed", |screen| {
        let Some(rail) = rail_row(screen) else {
            return false;
        };
        text(screen).contains("Disk › Users › alex")
            && !text(screen).contains("Desktop")
            && rail.contains("Collapse")
    });
    assert_has(&collapsed, "Depth            3");
    still_this_page(&collapsed);
    app.key("ctrl+b");
    let expanded = app.wait_screen("explorer expanded", |screen| {
        let Some(rail) = rail_row(screen) else {
            return false;
        };
        text(screen).contains("Desktop") && rail.contains("Expand")
    });
    assert_has(&expanded, "Disk › Users › alex");
    assert_has(&expanded, "Last RailAction  Expand");
    assert_has(&expanded, "Depth            3");
    assert_lacks(&expanded, "Applications");
    still_this_page(&expanded);
    freeze(
        &app,
        "sc.breadcrumbsrails.rail.after",
        "Expand restores Desktop under Disk › Users › alex and Last RailAction is Expand",
    );
    app.finish();
}

#[test]
fn option_clickable_width_collapsed_and_rail() {
    let defaults = page("s01-opt-default", &[], 120, 40);
    let screen = defaults.wait_screen("default playground", |screen| {
        playground_ready(screen) && text(screen).contains("the live widget")
    });
    let location = row_containing(&screen, "Location");
    assert!(
        location.contains("clickable") && !location.contains("clickable(false)"),
        "default location is clickable: {location}"
    );
    assert_has(&screen, "deep, cut to fit · full width");
    assert_has(&screen, "settlement-backoff-retry-policy-v2.md");
    assert_has(&screen, "Desktop");
    assert_has(&screen, "on: a collapsed sidebar leaves");
    defaults.finish();

    let ui = page("s01-opt-links", &[], 120, 40);
    ui.wait_text("the live widget");
    ui.key("right");
    ui.key("2");
    ui.wait_text("Links on");
    ui.key("tab");
    ui.key(" ");
    let toggled = ui.wait_text("Links off");
    assert_has(&toggled, "Sidebar collapsed off");
    assert_has(&toggled, "Collapsed rail on");
    assert_lacks(&toggled, "Links on");
    let width = row_containing(&toggled, "Deep trail width");
    assert!(
        !width.contains("20") && !width.contains("32") && !width.contains("48"),
        "only Links changed: {width}"
    );
    click_first(&ui, "Playground");
    let effect = ui.wait_text("clickable(false)");
    let location = row_containing(&effect, "Location");
    assert!(
        location.contains("clickable(false)"),
        "location meta follows Links off: {location}"
    );
    assert_has(&effect, "deep, cut to fit · full width");
    assert_has(&effect, "Desktop");
    assert_has(&effect, "Disk › Users › alex");
    freeze(
        &ui,
        "opt.breadcrumbsrails.clickable",
        "Links off shows clickable(false) on Location and leaves the deep trail at full width",
    );
    ui.finish();

    let width_args = ["--opt", "breadcrumbsrails.width=20"];
    let width_app = page("s01-opt-width", &width_args, 120, 40);
    let cut = width_app.wait_text("width 20");
    assert_has(&cut, "deep, cut to fit · width 20");
    assert_has(&cut, "settlement…v2.md");
    assert_lacks(&cut, "settlement-backoff-retry-policy-v2.md");
    let location = row_containing(&cut, "Location");
    assert!(
        !location.contains("clickable(false)"),
        "width does not change links: {location}"
    );
    assert_has(&cut, "Desktop");
    freeze(
        &width_app,
        "opt.breadcrumbsrails.width",
        "Deep trail width 20 truncates the trail to settlement…v2.md and leaves Links clickable",
    );
    width_app.finish();

    let collapsed_args = ["--opt", "breadcrumbsrails.collapsed=on"];
    let collapsed = page("s01-opt-collapsed", &collapsed_args, 120, 40);
    let rail = collapsed.wait_text("Disk › Users › alex");
    assert_lacks(&rail, "Desktop");
    assert_has(&rail, "Depth            3");
    let location = row_containing(&rail, "Location");
    assert!(
        location.contains("clickable") && !location.contains("clickable(false)"),
        "collapse keeps links: {location}"
    );
    assert_has(&rail, "on: a collapsed sidebar leaves");
    freeze(
        &collapsed,
        "opt.breadcrumbsrails.collapsed",
        "Sidebar collapsed on hides Desktop and keeps Disk › Users › alex beside the rail",
    );
    collapsed.finish();

    let rail_args = [
        "--opt",
        "breadcrumbsrails.collapsed=on",
        "--opt",
        "breadcrumbsrails.collapsed_rail=off",
    ];
    let hidden = page("s01-opt-rail", &rail_args, 120, 40);
    let screen = hidden.wait_text("only the keyboard brings it back");
    assert_has(&screen, "Disk › Users › alex");
    assert_has(&screen, "Depth            3");
    assert_lacks(&screen, "Desktop");
    assert_lacks(&screen, "on: a collapsed sidebar leaves");
    let location = row_containing(&screen, "Location");
    assert!(
        location.contains("clickable") && !location.contains("clickable(false)"),
        "collapsed rail does not change links: {location}"
    );
    assert_has(&screen, "deep, cut to fit · full width");
    freeze(
        &hidden,
        "opt.breadcrumbsrails.collapsed_rail",
        "Collapsed rail off, with Sidebar collapsed on, drops the rail and keeps Disk › Users › alex",
    );
    hidden.finish();
}

#[test]
fn option_stress_crumbs_and_long_label() {
    let items = page(
        "s01-opt-stress-items",
        &["--opt", "breadcrumbsrails.stress=20k items"],
        120,
        40,
    );
    let screen = items.wait_text("Folder 20000");
    assert_has(&screen, "deep, cut to fit · full width");
    assert_lacks(&screen, "Folder 00001");
    assert_has(&screen, "Explorer");
    assert_has(&screen, "Disk › Users › alex");
    items.finish();

    let label = page(
        "s01-opt-stress-label",
        &["--opt", "breadcrumbsrails.stress=65k label"],
        120,
        40,
    );
    let screen = label.wait_text("long label");
    assert_has(&screen, "Explorer");
    assert_has(&screen, "Location");
    assert_has(&screen, "Desktop");
    label.finish();
}
