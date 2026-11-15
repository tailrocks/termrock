//! Status & hints leaf. Drives the showcase binary through the PTY.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, SHOWCASE, find, find_all, row_text, text, try_launch};

fn open(name: &str, args: &[&str], cols: u16, rows: u16, motion: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    try_launch(launch).expect("launch showcase")
}

fn page(name: &str, opts: &[&str], cols: u16, rows: u16, motion: &str) -> App {
    let mut args = vec!["--page", "statushints"];
    for opt in opts {
        args.push("--opt");
        args.push(opt);
    }
    open(name, &args, cols, rows, motion)
}

fn visual_id(id: &str) -> String {
    // sample.rs validate_test_id_grammar: a segment must match [a-z0-9_-].
    let leaf = id.replace('.', "_");
    format!("pre_refactor/s01_statushints/{leaf}")
}

fn shot(app: &App, id: &str, _precondition: &str) {
    app.visual(&visual_id(id));
}

fn see(screen: &TerminalSnapshot, have: &[&str], lack: &[&str]) {
    let visible = text(screen);
    for needle in have {
        assert!(
            visible.contains(needle),
            "expected {needle:?} in\n{visible}"
        );
    }
    for needle in lack {
        assert!(
            !visible.contains(needle),
            "forbidden {needle:?} in\n{visible}"
        );
    }
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no row {needle:?}\n{}", text(screen)))
}

fn activate(app: &App, label: &str) {
    app.click_text(label);
    app.key("enter");
}

fn header(screen: &TerminalSnapshot) -> String {
    row_text(screen, 0)
}

fn wheel_to(app: &App, anchor: &str, needle: &str) -> TerminalSnapshot {
    let point = find(&app.wait_text(anchor), anchor);
    for _ in 0..16 {
        let screen = app.screen();
        if text(&screen).contains(needle) {
            let settled = app.settle();
            if text(&settled).contains(needle) {
                return settled;
            }
        }
        app.wheel(point, true, 3);
    }
    app.wait_text(needle)
}

fn reveal_right(app: &App, needle: &str) -> TerminalSnapshot {
    app.key("tab");
    for _ in 0..6 {
        let screen = app.screen();
        if text(&screen).contains(needle) {
            return screen;
        }
        app.key("alt+pagedown");
    }
    app.wait_text(needle)
}

fn choose_select(app: &App, current: &str, listed: &str) {
    let field = find(&app.screen(), current);
    app.click(field);
    let open = app.wait_text(listed);
    let option = find_all(&open, listed)
        .into_iter()
        .find(|point| point.y != field.y)
        .unwrap_or_else(|| panic!("popup row {listed:?} is not on screen\n{}", text(&open)));
    app.click(option);
    let closed = app.wait_screen("chosen value replaces the open list", |screen| {
        row_text(screen, u32::from(field.y)).contains(listed) && !text(screen).contains("Enter Set")
    });
    assert!(
        row_text(&closed, u32::from(field.y)).contains(listed),
        "field shows {listed}"
    );
    assert!(
        !text(&closed).contains("Enter Set"),
        "forbidden open-list footer after choosing {listed}"
    );
}

fn brand_ink(screen: &TerminalSnapshot) -> tui_test::SnapshotColor {
    let points = find_all(screen, "junie showcase");
    assert!(!points.is_empty(), "brand missing\n{}", text(screen));
    let colors: Vec<_> = points
        .iter()
        .map(|point| {
            screen
                .cell_at(u32::from(point.x), u32::from(point.y))
                .expect("brand cell")
                .fg()
        })
        .collect();
    assert!(
        colors.iter().all(|color| *color == colors[0]),
        "brand ink disagrees across copies: {colors:?}\n{}",
        text(screen)
    );
    colors[0]
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("statushints-route", &[], 120, 40, "paused");
    let overview = app.wait_text("Overview");
    assert!(
        header(&overview).contains("Overview"),
        "overview header is Overview, not Status & hints: {}",
        header(&overview)
    );
    assert!(
        !header(&overview).contains("Status & hints"),
        "forbidden Status & hints crumb on Overview: {}",
        header(&overview)
    );

    // The sidebar consumes ], so Right moves focus onto the page first.
    app.key("right");
    let armed = app.settle();
    assert!(
        header(&armed).contains("Overview") && !header(&armed).contains("Status & hints"),
        "Right leaves the sidebar on Overview: {}",
        header(&armed)
    );

    app.key("]");
    app.key("]");
    let arrived = app.wait_text("modal layer  (none)");
    see(
        &arrived,
        &[
            "App shell › Status & hints",
            "the live widget",
            "modal layer  (none)",
            "Playground",
        ],
        &["one control per option", "every state, drawn by the widget"],
    );
    assert!(
        header(&arrived).contains("Status & hints"),
        "keyboard ]] opens Status & hints: {}",
        header(&arrived)
    );

    app.key("[");
    let chrome = app.wait_text("App shell › Chrome");
    assert!(
        header(&chrome).contains("› Chrome"),
        "keyboard [ leaves to Chrome: {}",
        header(&chrome)
    );
    assert!(
        !header(&chrome).contains("Status & hints"),
        "forbidden Status & hints crumb on Chrome: {}",
        header(&chrome)
    );
    assert!(
        !text(&chrome).contains("modal layer"),
        "forbidden Status & hints playground on Chrome"
    );

    app.key("]");
    let again = app.wait_text("modal layer  (none)");
    assert!(
        header(&again).contains("Status & hints"),
        "keyboard ] re-enters Status & hints: {}",
        header(&again)
    );

    app.click_text("Overview");
    let back = app.wait_text("Tokens and principles");
    assert!(
        header(&back).contains("Overview") && !header(&back).contains("Status & hints"),
        "sidebar Overview returns home: {}",
        header(&back)
    );
    assert!(
        !text(&back).contains("modal layer"),
        "forbidden Status & hints playground after returning to Overview"
    );

    if !text(&back).contains("Status & hints") {
        app.click_text("App shell");
        app.wait_text("Status & hints");
    }
    app.click_text("Status & hints");
    let pointed = app.wait_text("modal layer  (none)");
    see(
        &pointed,
        &[
            "App shell › Status & hints",
            "the live widget",
            "Playground",
            "modal layer  (none)",
        ],
        &["one control per option"],
    );
    shot(
        &app,
        "sc.statushints.route",
        "pointer opens Status & hints from the sidebar after keyboard leave and return",
    );
    app.finish();

    let app = page("statushints-page-arg", &[], 120, 40, "paused");
    let opened = app.wait_text("the live widget");
    see(
        &opened,
        &["App shell › Status & hints", "the live widget"],
        &["one control per option"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = page("statushints-sections", &[], 80, 24, "paused");
    let playground = app.wait_text("resolved");
    see(
        &playground,
        &[
            "Status & hints",
            "Playground",
            "Options",
            "States",
            "modal layer  (none)",
            "resolved",
            "align Start",
        ],
        &[
            "the live widget",
            "one control per option",
            "every state, drawn by the widget",
            "Page options",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.statushints.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
    );

    app.click_text("Options");
    let options = app.wait_text("Page options");
    see(
        &options,
        &["Page options", "Top layer", "screen", "Bar width", "full"],
        &[
            "one control per option",
            "the live widget",
            "Compare levels",
            "resolved",
        ],
    );
    shot(
        &app,
        "sc.statushints.options.80x24",
        "options at 80 by 24 before the terminal grows",
    );

    app.click_text("States");
    let states = app.wait_text("Compare levels");
    see(
        &states,
        &[
            "Compare levels",
            "rest",
            "overflow",
            "error",
            "editing",
            "narrow",
            "EDIT",
        ],
        &["Page options", "the live widget", "one control per option"],
    );
    shot(
        &app,
        "sc.statushints.states.80x24",
        "states at 80 by 24 before the terminal grows",
    );

    app.resize(120, 40);
    app.wait_text("120×40");
    app.click_text("Playground");
    let wide_play = app.wait_text("the live widget");
    see(
        &wide_play,
        &[
            "the live widget",
            "modal layer  (none)",
            "menu layer  (none)",
            "align Start",
            "status_min_share 50%",
            "compact off",
            "Ctrl+H Show hidden",
        ],
        &[
            "one control per option",
            "every state, drawn by the widget",
            "Enter Delete",
            "align Center",
            "compact on",
            "Hide hidden",
        ],
    );
    shot(
        &app,
        "sc.statushints.playground.120x40",
        "playground at 120 by 40 after the narrow capture",
    );

    app.click_text("Options");
    let wide_options = app.wait_text("one control per option");
    see(
        &wide_options,
        &[
            "one control per option",
            "Top layer",
            "screen",
            "Align",
            "start",
            "Bar width",
            "full",
            "Compact keys off",
            "Status share %",
            "F5 priority",
            "auto",
            "EDIT badge off",
            "Hidden files shown off",
            "72x20",
            "Too-small quit hint",
            "default",
            "Too-small brand",
            "title",
        ],
        &[
            "the live widget",
            "every state, drawn by the widget",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.statushints.options.120x40",
        "options at 120 by 40 after the narrow capture",
    );

    app.click_text("States");
    let wide_states = app.wait_text("every state, drawn by the widget");
    see(
        &wide_states,
        &[
            "every state, drawn by the widget",
            "Compare levels",
            "rest",
            "overflow",
            "error",
            "editing",
            "narrow",
            "EDIT",
            "Start",
            "Center",
        ],
        &["the live widget", "one control per option", "Page options"],
    );
    shot(
        &app,
        "sc.statushints.states.120x40",
        "states at 120 by 40 after the narrow capture",
    );
    app.finish();
}

#[test]
fn compare_levels_wide_then_narrow() {
    let app = page("statushints-compare", &[], 120, 40, "paused");
    app.wait_text("the live widget");
    app.click_text("States");
    let off = app.wait_text("Compare levels");
    see(
        &off,
        &["○ Compare levels", "rest", "editing"],
        &["256 colors", "one control per option"],
    );
    app.key("tab");
    app.key(" ");
    let on = app.wait_text("256 colors");
    see(
        &on,
        &[
            "● Compare levels",
            "truecolor",
            "256 colors",
            "16 colors",
            "no color",
            "rest",
            "editing",
            "Status & hints",
        ],
        &["○ Compare levels", "one control per option", "Page options"],
    );
    assert!(
        header(&on).contains("Status & hints"),
        "compare levels stays on Status & hints: {}",
        header(&on)
    );
    shot(
        &app,
        "sc.statushints.compare-levels.120x40",
        "Compare levels on at 120 by 40, before the terminal narrows",
    );

    app.resize(80, 24);
    let narrow = wheel_to(&app, "● Compare levels", "no color");
    see(
        &narrow,
        &["no color", "Status & hints", "rest"],
        &["one control per option", "Page options", "○ Compare levels"],
    );
    shot(
        &app,
        "sc.statushints.compare-levels.80x24",
        "Compare levels on at 80 by 24, captured before restoring the wide size",
    );

    let origin = find(&app.screen(), "no color");
    for _ in 0..10 {
        if text(&app.screen()).contains("● Compare levels") {
            break;
        }
        app.wheel(origin, false, 3);
    }
    app.key(" ");
    let mut closed = app.settle();
    if text(&closed).contains("● Compare levels") {
        app.key("tab");
        app.key(" ");
        closed = app.wait_text("○ Compare levels");
    }
    see(
        &closed,
        &["○ Compare levels", "rest", "Status & hints"],
        &["256 colors", "16 colors", "no color", "Page options"],
    );
    assert!(
        header(&closed).contains("Status & hints"),
        "turning Compare levels off keeps the page: {}",
        header(&closed)
    );
    app.resize(120, 40);
    app.finish();
}

#[test]
fn option_layer_from_the_options_control() {
    let app = page("statushints-layer", &[], 120, 40, "paused");
    app.wait_text("the live widget");
    app.click_text("Options");
    let options = app.wait_text("one control per option");
    see(
        &options,
        &["Top layer", "screen", "Align", "start"],
        &["modal"],
    );
    choose_select(&app, "screen", "modal");
    let chosen = app.screen();
    see(
        &chosen,
        &["modal", "start", "one control per option"],
        &["the live widget", "menu", "Enter Set"],
    );
    app.click_text("Playground");
    let playground = app.wait_text("Enter Delete");
    see(
        &playground,
        &[
            "Enter Delete  Esc Cancel",
            "Enter Choose  Esc Close",
            "align Start",
            "status_min_share 50%",
            "compact off",
        ],
        &[
            "modal layer  (none)",
            "menu layer  (none)",
            "align Center",
            "compact on",
        ],
    );
    shot(
        &app,
        "opt.statushints.layer",
        "Options control sets Top layer to modal; Align stays start",
    );
    app.finish();
}

#[test]
fn option_boundaries() {
    let align = page(
        "statushints-align",
        &["statushints.align=center"],
        120,
        40,
        "paused",
    );
    let screen = align.wait_text("align Center");
    see(
        &screen,
        &["align Center", "modal layer  (none)", "the live widget"],
        &["align Start", "Enter Delete"],
    );
    shot(
        &align,
        "opt.statushints.align",
        "Align center; Top layer stays screen",
    );
    align.finish();

    let width = page(
        "statushints-width",
        &["statushints.width=narrow"],
        120,
        40,
        "paused",
    );
    let screen = width.wait_text("? Help  …");
    let resolved = row_with(&screen, "resolved →");
    assert!(
        resolved.contains("Enter Open  Tab Next pane  ? Help  …"),
        "narrow resolved bar drops trailing hints: {resolved}"
    );
    assert!(
        !resolved.contains("F5"),
        "forbidden F5 on the narrow resolved bar: {resolved}"
    );
    see(
        &screen,
        &["align Start", "compact off", "the live widget"],
        &["compact on", "align Center", "Enter Delete"],
    );
    shot(
        &width,
        "opt.statushints.width",
        "Bar width narrow drops F5; Align stays start and Compact stays off",
    );
    width.finish();

    let compact = page(
        "statushints-compact",
        &["statushints.width=narrow", "statushints.compact=on"],
        120,
        40,
        "paused",
    );
    let screen = compact.wait_text("compact on");
    see(
        &screen,
        &[
            "compact on",
            "Enter  Tab  /",
            "align Start",
            "status_min_share 50%",
        ],
        &["compact off", "align Center", "status_min_share 30%"],
    );
    shot(
        &compact,
        "opt.statushints.compact",
        "Compact on with the required narrow width; share stays 50%",
    );
    compact.finish();

    let share = page(
        "statushints-share",
        &[
            "statushints.width=narrow",
            "statushints.status=plain",
            "statushints.share=30",
        ],
        120,
        40,
        "paused",
    );
    let screen = share.wait_text("status_min_share 30%");
    see(
        &screen,
        &[
            "status_min_share 30%",
            "3 rows selected",
            "align Start",
            "compact off",
        ],
        &["status_min_share 50%", "compact on", "align Center"],
    );
    shot(
        &share,
        "opt.statushints.share",
        "Status share 30% with narrow width and plain status",
    );
    share.finish();

    let priority = page(
        "statushints-priority",
        &["statushints.width=narrow", "statushints.priority=pinned"],
        120,
        40,
        "paused",
    );
    let screen = priority.wait_text("F5 Refresh");
    let resolved = row_with(&screen, "resolved →");
    assert!(
        resolved.contains("F5 Refresh"),
        "pinned F5 stays on the narrow resolved bar: {resolved}"
    );
    see(
        &screen,
        &["align Start", "compact off"],
        &["compact on", "align Center", "Hide hidden"],
    );
    shot(
        &priority,
        "opt.statushints.priority",
        "F5 pinned on a narrow bar; Compact stays off",
    );
    priority.finish();

    let status = page(
        "statushints-status",
        &["statushints.status=parts"],
        120,
        40,
        "paused",
    );
    let top = status.wait_text("align Start");
    see(
        &top,
        &["align Start", "compact off", "the live widget"],
        &["align Center", "3 rows selected", "2 unsaved changes"],
    );
    let screen = reveal_right(&status, "payments.settlements");
    see(
        &screen,
        &["payments.settlements", "compact off"],
        &["3 rows selected", "2 unsaved changes", "align Center"],
    );
    shot(
        &status,
        "opt.statushints.status",
        "Status parts shows payments.settlements; Align stays start",
    );
    status.finish();

    let badge = page(
        "statushints-badge",
        &["statushints.badge=on"],
        120,
        40,
        "paused",
    );
    let screen = badge.wait_text("EDIT");
    let resolved = row_with(&screen, "resolved →");
    assert!(
        resolved.contains("EDIT"),
        "EDIT badge is on the resolved bar: {resolved}"
    );
    see(
        &screen,
        &["align Start", "Ctrl+H Show hidden"],
        &["Hide hidden", "align Center"],
    );
    shot(
        &badge,
        "opt.statushints.badge",
        "EDIT badge on; hidden-files hint stays Show hidden",
    );
    badge.finish();

    let hidden = page(
        "statushints-hidden",
        &["statushints.hidden=on"],
        120,
        40,
        "paused",
    );
    let screen = hidden.wait_text("Ctrl+H Hide hidden");
    let resolved = row_with(&screen, "resolved →");
    assert!(
        resolved.contains("Ctrl+H Hide hidden"),
        "resolved bar hides files: {resolved}"
    );
    assert!(
        !resolved.contains("EDIT") && !resolved.contains("Show hidden"),
        "forbidden badge and Show hidden on the resolved bar: {resolved}"
    );
    see(
        &screen,
        &["align Start", "compact off"],
        &["Show hidden", "align Center"],
    );
    shot(
        &hidden,
        "opt.statushints.hidden",
        "Hidden files on changes the hint to Hide hidden; badge stays off",
    );
    hidden.finish();

    let defaults = page("statushints-too-small-default", &[], 120, 40, "paused");
    let preview = wheel_to(&defaults, "resolved", "Need 72×20, have 42×5");
    see(
        &preview,
        &[
            "junie showcase",
            "Terminal too small",
            "Need 72×20, have 42×5",
            "q Quit",
        ],
        &["Need 100×30", "resize to continue"],
    );
    let title_ink = brand_ink(&preview);
    defaults.finish();

    let small = page(
        "statushints-too-small",
        &["statushints.too_small_min=100x30"],
        120,
        40,
        "paused",
    );
    let preview = wheel_to(&small, "resolved", "Need 100×30, have 42×5");
    see(
        &preview,
        &["junie showcase", "Need 100×30, have 42×5", "q Quit"],
        &["Need 72×20", "resize to continue"],
    );
    shot(
        &small,
        "opt.statushints.too_small_min",
        "Too small minimum 100×30; quit hint stays q Quit",
    );
    small.finish();

    let quit = page(
        "statushints-quit-hint",
        &["statushints.quit_hint=custom"],
        120,
        40,
        "paused",
    );
    let preview = wheel_to(&quit, "resolved", "resize to continue");
    see(
        &preview,
        &[
            "q Quit · resize to continue",
            "Need 72×20, have 42×5",
            "junie showcase",
        ],
        &["Need 100×30"],
    );
    shot(
        &quit,
        "opt.statushints.quit_hint",
        "Custom too-small quit hint; minimum stays 72×20",
    );
    quit.finish();

    let brand = page(
        "statushints-brand",
        &["statushints.brand=accent"],
        120,
        40,
        "paused",
    );
    let preview = wheel_to(&brand, "resolved", "Need 72×20, have 42×5");
    see(
        &preview,
        &[
            "junie showcase",
            "Need 72×20, have 42×5",
            "Terminal too small",
        ],
        &["Need 100×30", "resize to continue"],
    );
    let accent_ink = brand_ink(&preview);
    assert_ne!(
        accent_ink, title_ink,
        "accent brand ink {accent_ink:?} must differ from title ink {title_ink:?}"
    );
    shot(
        &brand,
        "opt.statushints.brand",
        "Too-small brand uses the accent ink; the brand text stays junie showcase",
    );
    brand.finish();
}

#[test]
fn hints_priority_width_and_hidden() {
    let app = page("statushints-hints", &[], 120, 40, "paused");
    let before = app.wait_text("Ctrl+H Show hidden");
    see(
        &before,
        &["Ctrl+H Show hidden", "align Start", "the live widget"],
        &["Hide hidden", "Enter Delete"],
    );
    shot(
        &app,
        "sc.statushints.hints.before",
        "full-width hints show Ctrl+H Show hidden",
    );

    app.click_text("Options");
    app.wait_text("Hidden files shown off");
    app.click_text("Hidden files shown");
    app.wait_text("Hidden files shown on");
    app.click_text("Playground");
    let hidden = app.wait_text("Ctrl+H Hide hidden");
    see(
        &hidden,
        &["Ctrl+H Hide hidden", "align Start", "the live widget"],
        &["Show hidden", "align Center"],
    );

    app.click_text("Options");
    app.wait_text("one control per option");
    choose_select(&app, "full", "narrow");
    choose_select(&app, "auto", "pinned");
    app.click_text("Playground");
    let after = app.wait_text("F5 Refresh");
    see(
        &after,
        &["F5 Refresh", "align Start", "Status & hints"],
        &["Show hidden", "one control per option", "align Center"],
    );
    shot(
        &app,
        "sc.statushints.hints.after",
        "narrow pinned bar keeps F5 Refresh and Show hidden is gone",
    );
    let preview = wheel_to(&app, "resolved", "Need 72×20, have 42×5");
    see(
        &preview,
        &["Need 72×20, have 42×5", "junie showcase", "Status & hints"],
        &["Need 100×30", "resize to continue"],
    );
    app.finish();
}

#[test]
fn status_posts_expire_and_error_stays() {
    // Paused motion returns before the footer clock, so expiry cannot be seen.
    // Reduced motion advances ticks without moving the spinner. advance_clock is unused.
    let app = page("statushints-status", &[], 120, 40, "reduced");
    let before = app.wait_text("the live widget");
    see(
        &before,
        &["the live widget", "modal layer  (none)"],
        &["Indexing finished", "Saved 3 rows", "2 rows skipped"],
    );
    assert!(
        !footer(&before).contains("Connection lost"),
        "footer starts without Connection lost: {}",
        footer(&before)
    );
    shot(
        &app,
        "sc.statushints.status.before",
        "footer has no posted status before Post info",
    );

    wheel_to(&app, "resolved", "Post info");
    activate(&app, "Post info");
    let info = app.wait_screen("info footer", |screen| {
        footer(screen).contains("Saved 3 rows")
    });
    assert!(
        footer(&info).contains("Saved 3 rows"),
        "footer shows Saved 3 rows: {}",
        footer(&info)
    );
    assert!(
        !footer(&info).contains("Indexing finished"),
        "forbidden Indexing finished after Post info: {}",
        footer(&info)
    );

    activate(&app, "Post warning");
    let warning = app.wait_screen("warning footer", |screen| {
        footer(screen).contains("2 rows skipped")
    });
    assert!(
        footer(&warning).contains("2 rows skipped"),
        "footer shows 2 rows skipped: {}",
        footer(&warning)
    );
    assert!(
        !footer(&warning).contains("Saved 3 rows"),
        "forbidden Saved 3 rows after Post warning: {}",
        footer(&warning)
    );

    for _ in 0..4 {
        app.key("tab");
    }
    app.key("enter");
    let finished = app.wait_screen("job footer", |screen| {
        footer(screen).contains("Indexing finished")
    });
    assert!(
        footer(&finished).contains("Indexing finished"),
        "footer shows Indexing finished: {}",
        footer(&finished)
    );
    let expired = app.wait_absent("Indexing finished");
    assert!(
        !text(&expired).contains("Indexing finished"),
        "forbidden Indexing finished after its lifetime"
    );
    assert!(
        !footer(&expired).contains("Saved 3 rows") && !footer(&expired).contains("2 rows skipped"),
        "forbidden earlier status after the job: {}",
        footer(&expired)
    );

    activate(&app, "Post error");
    let error = app.wait_screen("error footer", |screen| {
        footer(screen).contains("Connection lost")
    });
    assert!(
        footer(&error).contains("Connection lost"),
        "footer keeps Connection lost: {}",
        footer(&error)
    );
    let held = app.stable_screen();
    assert!(
        footer(&held).contains("Connection lost"),
        "error stays through settle: {}",
        footer(&held)
    );
    assert!(
        !text(&held).contains("Indexing finished"),
        "forbidden expired Indexing finished while the error stays"
    );
    shot(
        &app,
        "sc.statushints.status.after",
        "Indexing finished has expired and Connection lost is still on the footer",
    );

    app.key("esc");
    let cleared = app.settle();
    assert!(
        header(&cleared).contains("Status & hints"),
        "the page stays Status & hints after Esc: {}",
        header(&cleared)
    );
    assert!(
        !footer(&cleared).contains("Connection lost"),
        "Esc clears the error footer: {}",
        footer(&cleared)
    );
    assert!(
        !text(&cleared).contains("Indexing finished"),
        "forbidden Indexing finished after the error is cleared"
    );
    app.finish();
}

#[test]
fn notices_replace_notices_and_yield_to_posts() {
    const SYNC: &str = "Sync paused: another client is entering";
    const REFRESH: &str = "Refresh failed, retrying in 30 s";
    let app = page("statushints-notice", &[], 120, 40, "reduced");
    let before = app.wait_text("the live widget");
    assert!(
        !footer(&before).contains("Sync paused") && !footer(&before).contains("Refresh failed"),
        "the footer starts without a notice: {}",
        footer(&before)
    );

    wheel_to(&app, "resolved", "Notice: refresh failed");
    activate(&app, "Notice: sync paused");
    let first = app.wait_screen("first notice", |screen| footer(screen).contains(SYNC));
    assert!(
        footer(&first).contains(SYNC),
        "a notice shows when the line is free: {}",
        footer(&first)
    );

    activate(&app, "Notice: refresh failed");
    let second = app.wait_screen("second notice", |screen| footer(screen).contains(REFRESH));
    assert!(
        !footer(&second).contains("Sync paused"),
        "a notice replaces an earlier notice: {}",
        footer(&second)
    );

    activate(&app, "Post info");
    let post = app.wait_screen("post over a notice", |screen| {
        footer(screen).contains("Saved 3 rows")
    });
    assert!(
        !footer(&post).contains("Refresh failed"),
        "a post replaces a notice: {}",
        footer(&post)
    );

    activate(&app, "Notice: sync paused");
    let dropped = app.settle();
    assert!(
        footer(&dropped).contains("Saved 3 rows") && !footer(&dropped).contains("Sync paused"),
        "a notice never hides a post: {}",
        footer(&dropped)
    );

    app.wait_absent("Saved 3 rows");
    activate(&app, "Notice: refresh failed");
    let free = app.wait_screen("notice on a free line", |screen| {
        footer(screen).contains(REFRESH)
    });
    assert!(
        !footer(&free).contains("Sync paused"),
        "the dropped notice did not come back: {}",
        footer(&free)
    );
    app.finish();
}
