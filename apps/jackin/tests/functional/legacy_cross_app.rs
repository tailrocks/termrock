use std::thread;
use std::time::{Duration, Instant};

use jackin_console::app::Route;
use jackin_console::fixtures::{Scenario, app_for};
use jackin_console::screens::manager::TREE;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use termrock::core::event::{Input, Key, Outcome};
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::motion::Motion;
use termrock::shell::App as ShellApp;
use termrock::theme::Theme;
use termrock::ui::ctx::{Interaction, RenderCtx};
use tui_test::terminal::cell::Attrs;
use tui_test::{SnapshotColor, SnapshotUnderline, TerminalSnapshot};

use super::support::{App, JACKIN, Launch, Point, text, try_launch};

#[test]
fn intro_enter_is_consumed_for_exactly_the_first_three_ticks() {
    let mut app = app_for(Scenario::FirstUse, Motion::Reduced, 0, Theme::junie());
    let enter = Input::Key(Key {
        code: KeyCode::Enter,
        mods: KeyModifiers::NONE,
    });

    assert_eq!(app.route(), Route::Intro);
    assert_eq!(
        ShellApp::on_input(&mut app, enter.clone()),
        Outcome::Consumed
    );
    for _ in 0..3 {
        assert_eq!(app.route(), Route::Intro);
        assert_eq!(
            ShellApp::on_input(&mut app, enter.clone()),
            Outcome::Consumed
        );
        ShellApp::on_input(&mut app, Input::Tick);
    }
    assert_eq!(app.route(), Route::Intro);
    assert_eq!(ShellApp::on_input(&mut app, enter), Outcome::Changed);
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn failed_refresh_keeps_selection_moved_during_pending_frame() {
    let mut app = app_for(Scenario::HardCases, Motion::Reduced, 0, Theme::junie());
    app.shell_mut().focus.focus(TREE);
    let area = Rect::new(0, 0, 120, 40);

    let draw = |app: &mut jackin_console::app::App| {
        let mut buffer = Buffer::empty(area);
        let theme = Theme::junie();
        let mut hits = HitRegistry::default();
        let mut focus = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut focus);
        ShellApp::draw(app, area, &mut buffer, &mut ctx);
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let key = |code| {
        Input::Key(Key {
            code,
            mods: KeyModifiers::NONE,
        })
    };

    let initial = draw(&mut app);
    assert!(
        initial.contains("Workspaces"),
        "manager did not render\n{initial}"
    );
    assert_eq!(
        ShellApp::on_input(&mut app, key(KeyCode::Down)),
        Outcome::Changed
    );
    assert_eq!(
        ShellApp::on_input(&mut app, key(KeyCode::Right)),
        Outcome::Changed
    );
    let expanded = draw(&mut app);
    assert!(
        expanded.contains("7f3a"),
        "payments did not expand\n{expanded}"
    );

    assert_eq!(
        ShellApp::on_input(&mut app, key(KeyCode::Down)),
        Outcome::Changed
    );
    let running = draw(&mut app);
    assert!(running.contains("Container and Capsule daemon are live."));

    assert_eq!(
        ShellApp::on_input(&mut app, key(KeyCode::F(5))),
        Outcome::Changed
    );
    let pending = draw(&mut app);
    assert!(
        pending.contains("Workspaces"),
        "refresh left the manager\n{pending}"
    );
    assert!(
        !pending.contains("no instances"),
        "refresh cleared the list\n{pending}"
    );
    assert!(
        pending.contains("Refreshing…"),
        "pending status missing\n{pending}"
    );
    assert!(
        !pending.contains("Refresh failed:"),
        "refresh failed before tick\n{pending}"
    );

    assert_eq!(
        ShellApp::on_input(&mut app, key(KeyCode::Down)),
        Outcome::Changed
    );
    let moved = draw(&mut app);
    assert!(
        moved.contains("Workspaces"),
        "refresh left the manager\n{moved}"
    );
    assert!(
        !moved.contains("no instances"),
        "refresh cleared the list\n{moved}"
    );
    assert!(
        moved.contains("Refreshing…"),
        "pending status disappeared\n{moved}"
    );
    assert!(moved.contains("c41e"), "dirty instance missing\n{moved}");
    assert!(
        moved.contains("Stopped with uncommitted changes"),
        "dirty selection details missing\n{moved}"
    );
    assert!(
        !moved.contains("Refresh failed:"),
        "failure replaced the pending refresh\n{moved}"
    );

    ShellApp::on_input(&mut app, Input::Tick);
    let failed = draw(&mut app);
    assert!(
        failed.contains("Refresh failed: instance index unreadable"),
        "failure missing\n{failed}"
    );
    assert!(
        failed.contains("c41e"),
        "selection was lost after failure\n{failed}"
    );
    assert!(
        failed.contains("Stopped with uncommitted changes"),
        "dirty details were lost\n{failed}"
    );
    assert!(
        failed.contains("7f3a"),
        "last-good rows were lost\n{failed}"
    );
}

fn configured_launch(
    name: &str,
    args: &[&str],
    color: Option<&str>,
    motion: Option<&str>,
    env: &[(&str, &str)],
) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.args = args.iter().map(|arg| (*arg).to_owned()).collect();
    launch.color = color.map(str::to_owned);
    launch.motion = motion.map(str::to_owned);
    launch.env.extend(
        env.iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
    );
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn leading_foreground(screen: &TerminalSnapshot, needle: &str) -> (String, SnapshotColor) {
    let hit = super::support::find_all(screen, needle)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("missing {needle}\n{}", text(screen)));
    let cell = screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .expect("caption cell");
    (cell.text().to_owned(), cell.fg())
}

fn assert_jackin_no_color_accent() {
    let plain = configured_launch(
        "x01-env-nocolor-jackin",
        &[],
        Some("nocolor"),
        Some("reduced"),
        &[],
    );
    let plain_screen = plain.wait_text(CAPTION);
    let (plain_text, plain_fg) = leading_foreground(&plain_screen, CAPTION);
    assert_eq!(plain_fg, SnapshotColor::Default, "Jackin NO_COLOR caption");
    assert_eq!(
        rgb_cell_count(&plain_screen),
        0,
        "Jackin NO_COLOR RGB cells"
    );
    plain.kill_child();

    let color = configured_launch(
        "x01-env-truecolor-jackin",
        &[],
        Some("truecolor"),
        Some("reduced"),
        &[],
    );
    let color_screen = color.wait_text(CAPTION);
    let (color_text, color_fg) = leading_foreground(&color_screen, CAPTION);
    assert_eq!(color_text, plain_text, "Jackin caption character");
    assert_eq!(color_fg, SnapshotColor::Rgb(255, 255, 255));
    assert!(
        rgb_cell_count(&color_screen) > 0,
        "Jackin truecolor RGB cells"
    );
    color.kill_child();
}

fn rgb_cell_count(screen: &TerminalSnapshot) -> usize {
    (0..screen.rows())
        .flat_map(|row| screen.row(row).expect("row").iter())
        .filter(|cell| {
            matches!(cell.fg(), SnapshotColor::Rgb(_, _, _))
                || matches!(cell.bg(), SnapshotColor::Rgb(_, _, _))
        })
        .count()
}

fn jackin_reduced_caption(name: &str, motion: Option<&str>, value: Option<&str>) -> bool {
    let env = value.map_or_else(Vec::new, |value| vec![("JACKIN_NO_MOTION", value)]);
    let app = configured_launch(name, &[], Some("truecolor"), motion, &env);
    let screen = app.wait_screen(name, |screen| {
        let visible = text(screen);
        visible.contains(CAPTION) || visible.contains(SKIP)
    });
    let reduced = text(&screen).contains(CAPTION) && text(&screen).contains(CONTINUE);
    if reduced {
        assert!(
            !text(&screen).contains(SKIP),
            "reduced intro shows Skip\n{}",
            text(&screen)
        );
    } else {
        assert!(
            text(&screen).contains(SKIP),
            "full intro omits Skip\n{}",
            text(&screen)
        );
        assert!(
            !text(&screen).contains(CAPTION),
            "full intro shows caption\n{}",
            text(&screen)
        );
    }
    app.kill_child();
    reduced
}

fn assert_jackin_motion_precedence() {
    assert!(jackin_reduced_caption(
        "x01-env-motion-jackin-reduced",
        Some("reduced"),
        None
    ));
    assert!(jackin_reduced_caption(
        "x01-env-motion-jackin-env",
        None,
        Some("1")
    ));
    assert!(!jackin_reduced_caption(
        "x01-env-motion-jackin-full",
        Some("full"),
        Some("1")
    ));
    assert!(!jackin_reduced_caption(
        "x01-env-motion-jackin-zero",
        None,
        Some("0")
    ));
}

fn jackin_scenario(name: &str, scenario: &str) -> App {
    configured_launch(
        name,
        &["--scenario", scenario],
        Some("truecolor"),
        Some("reduced"),
        &[],
    )
}

fn jackin_focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| super::support::row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn jackin_action_focused(screen: &TerminalSnapshot, label: &str) -> bool {
    (0..screen.rows())
        .any(|row| super::support::row_text(screen, row).contains(&format!("▎{label}")))
}

fn select_clean_instance(app: &App) {
    app.wait_screen("Jackin manager", |screen| {
        text(screen).contains(CURRENT) && text(screen).contains(WORKSPACES)
    });
    app.key("down");
    app.wait_screen("payments workspace", |screen| {
        jackin_focused_line(screen).contains("payments-platform")
            && !jackin_focused_line(screen).contains('▾')
    });
    app.key("down");
    app.wait_screen("infra workspace", |screen| {
        jackin_focused_line(screen).contains("infra-control-plane")
            && !jackin_focused_line(screen).contains('▾')
    });
    app.key("right");
    app.wait_screen("infra expanded", |screen| {
        jackin_focused_line(screen).contains('▾')
            && jackin_focused_line(screen).contains("infra-control-plane")
            && text(screen).contains(CLEAN_ID)
    });
    app.key("down");
    app.wait_screen("clean instance selected", |screen| {
        let line = jackin_focused_line(screen);
        line.contains(CLEAN_ID) && line.contains("sre") && !line.contains(DIRTY_ID)
    });
}

fn clean_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CLEAN_IDENTITY)
        && visible.contains("Ctrl+Q")
        && !visible.contains(UNSAVED)
        && !visible.contains(CLEAN_QUIT)
        && !visible.contains(WORKSPACES)
}

fn jackin_quit(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CLEAN_QUIT)
        && visible.contains(FORCE_STOP)
        && visible.contains("Quit")
        && visible.contains("Cancel")
        && !visible.contains(UNSAVED)
        && !visible.contains(CLEAN_LIFE)
}

fn confirm_jackin_quit(app: &App, open: impl Fn(&TerminalSnapshot) -> bool) {
    app.key("right");
    app.wait_screen("Quit focused", |screen| {
        open(screen) && jackin_action_focused(screen, "Quit")
    });
    app.key("enter");
}

#[test]
fn jackin_attached_and_launched_sessions_quit_without_exiting_manager() {
    let app = jackin_scenario("x02-exit-jackin-attached", "returning");
    select_clean_instance(&app);
    app.key("enter");
    let capsule = app.wait_screen("clean capsule", clean_capsule);
    assert!(text(&capsule).contains(CLEAN_IDENTITY));
    assert!(!text(&capsule).contains(UNSAVED));
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit Jackin attached session", jackin_quit);
    for present in [CLEAN_QUIT, FORCE_STOP, "Cancel"] {
        assert!(
            text(&dialog).contains(present),
            "missing {present}\n{}",
            text(&dialog)
        );
    }
    assert!(!text(&dialog).contains(UNSAVED));
    assert!(jackin_action_focused(&dialog, "Cancel"));
    confirm_jackin_quit(&app, jackin_quit);
    let manager = app.wait_screen("clean session ended", |screen| {
        let visible = text(screen);
        visible.contains(CLEAN_LIFE)
            && visible.contains(STILL_INSIDE)
            && visible.contains(CLEAN_ID)
            && !visible.contains(CLEAN_QUIT)
            && !visible.contains(UNSAVED)
    });
    assert!(
        app.child_running(),
        "confirming the clean instance quit Jackin"
    );
    assert!(text(&manager).contains(WORKSPACES));
    app.finish();

    let app = jackin_scenario("x02-exit-jackin-launch", "launch-running");
    let capsule = app.wait_screen("launched capsule", |screen| {
        let visible = text(screen);
        visible.contains(LAUNCH_VERSION)
            && visible.contains(LAUNCH_PANE)
            && visible.contains(LAUNCH_TAB)
            && !visible.contains(CLEAN_QUIT)
            && !visible.contains(UNSAVED)
    });
    assert!(text(&capsule).contains(LAUNCH_VERSION));
    assert!(!text(&capsule).contains(UNSAVED));
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit launched capsule", jackin_quit);
    for present in [CLEAN_QUIT, FORCE_STOP] {
        assert!(
            text(&dialog).contains(present),
            "missing {present}\n{}",
            text(&dialog)
        );
    }
    for absent in [UNSAVED, CLEAN_LIFE, STILL_INSIDE] {
        assert!(
            !text(&dialog).contains(absent),
            "unexpected {absent}\n{}",
            text(&dialog)
        );
    }
    confirm_jackin_quit(&app, jackin_quit);
    let manager = app.wait_screen("launch returned", |screen| {
        let visible = text(screen);
        visible.contains(STILL_INSIDE)
            && visible.contains(WORKSPACES)
            && !visible.contains(CLEAN_QUIT)
            && !visible.contains(LAUNCH_PANE)
    });
    assert!(app.child_running(), "confirming the launch quit Jackin");
    assert!(text(&manager).contains(STILL_INSIDE));
    app.finish();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ink {
    fg: SnapshotColor,
    bg: SnapshotColor,
    mods: u8,
}

const BOLD: u8 = 1;
const DIM: u8 = 2;
const INVERSE: u8 = 4;
const UNDER: u8 = 8;
const HEAVY: &str = "┃";
const BAR: &str = "▎";
const RULE: &str = "│";
const GUTTER: &str = "|";
const THUMB: &str = "#";

fn cell_count(screen: &TerminalSnapshot, glyph: &str) -> usize {
    (0..screen.rows())
        .flat_map(|row| screen.row(row).expect("row").iter())
        .filter(|cell| cell.text() == glyph)
        .count()
}

fn ink(fg: SnapshotColor, bg: SnapshotColor, mods: u8) -> Ink {
    Ink { fg, bg, mods }
}

fn color_rgb(r: u8, g: u8, b: u8) -> SnapshotColor {
    SnapshotColor::Rgb(r, g, b)
}

fn color_idx(index: u8) -> SnapshotColor {
    SnapshotColor::Indexed(index)
}

fn read_ink(screen: &TerminalSnapshot, point: Point) -> Ink {
    let cell = screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell");
    let mut mods = 0;
    if cell.has(Attrs::BOLD) {
        mods |= BOLD;
    }
    if cell.has(Attrs::DIM) {
        mods |= DIM;
    }
    if cell.has(Attrs::INVERSE) {
        mods |= INVERSE;
    }
    if cell.underline() != SnapshotUnderline::None {
        mods |= UNDER;
    }
    ink(cell.fg(), cell.bg(), mods)
}

fn launch_button_ink(color: &str) -> (Ink, Ink, Ink) {
    match color {
        "truecolor" => (
            ink(color_rgb(255, 255, 255), color_rgb(39, 39, 42), 0),
            ink(color_rgb(255, 255, 255), color_rgb(55, 55, 61), 0),
            ink(color_rgb(255, 255, 255), color_rgb(74, 74, 82), BOLD),
        ),
        "256" => (
            ink(color_idx(231), color_idx(235), 0),
            ink(color_idx(231), color_idx(238), 0),
            ink(color_idx(231), color_idx(240), BOLD),
        ),
        "16" | "none" => (
            ink(color_idx(0), color_idx(8), 0),
            ink(color_idx(0), color_idx(8), UNDER),
            ink(color_idx(0), color_idx(8), BOLD | INVERSE),
        ),
        "nocolor" => {
            let plain = SnapshotColor::Default;
            (
                ink(plain, plain, 0),
                ink(plain, plain, UNDER),
                ink(plain, plain, BOLD | INVERSE),
            )
        }
        _ => unreachable!("color {color}"),
    }
}

fn launch_point(screen: &TerminalSnapshot) -> Point {
    let hits: Vec<_> = super::support::find_all(screen, "Launch")
        .into_iter()
        .filter(|hit| hit.y > 2 && u32::from(hit.y) + 2 < screen.rows())
        .collect();
    assert_eq!(hits.len(), 1, "Launch hits {hits:?}\n{}", text(screen));
    hits[0]
}

fn wait_for_ink(app: &App, point: Point, expected: Ink) -> Ink {
    let deadline = Instant::now() + Duration::from_millis(800);
    loop {
        let got = read_ink(&app.screen(), point);
        if got == expected {
            return got;
        }
        assert!(
            Instant::now() < deadline,
            "cell {point:?} is {got:?}, expected {expected:?}\n{}",
            text(&app.screen())
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn launch_button_gesture(app: &App, point: Point, rest: Ink, hover: Ink, pressed: Ink) {
    assert_eq!(read_ink(&app.screen(), point), rest, "Launch rest");
    app.move_to(point);
    assert_eq!(wait_for_ink(app, point, hover), hover, "Launch hover");
    app.down(point);
    assert_eq!(wait_for_ink(app, point, pressed), pressed, "Launch pressed");
    let away = Point { x: 1, y: 1 };
    app.drag_to(away);
    app.up(away);
}

#[test]
fn jackin_launch_button_paints_rest_hover_and_pressed_states() {
    for color in ["truecolor", "256", "16", "none", "nocolor"] {
        let app = configured_launch(
            &format!("x03-color-jackin-{color}"),
            &["--scenario", "hard-cases"],
            Some(color),
            Some("reduced"),
            &[],
        );
        let screen = app.wait_screen("workspace actions", |screen| {
            let visible = text(screen);
            visible.contains(CURRENT) && visible.contains("Launch") && visible.contains("Edit")
        });
        assert!(text(&screen).contains("Edit"));
        let point = launch_point(&screen);
        let (rest, hover, pressed) = launch_button_ink(color);
        assert_ne!(rest, hover, "{color}: rest and hover paint identically");
        assert_ne!(
            hover, pressed,
            "{color}: hover and pressed paint identically"
        );
        assert_ne!(rest, pressed, "{color}: rest and pressed paint identically");
        launch_button_gesture(&app, point, rest, hover, pressed);
        app.kill_child();
    }
}

fn manager_focus_ink(screen: &TerminalSnapshot, point: Point) -> (SnapshotColor, SnapshotColor) {
    let cell = screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("manager focus cell");
    (cell.fg(), cell.bg())
}

fn seam_run(screen: &TerminalSnapshot) -> (u16, u16, u16) {
    let rest = ink(color_rgb(0x4d, 0x4d, 0x4d), color_rgb(0, 0, 0), 0);
    let mut runs = Vec::new();
    for x in 0..screen.cols() {
        let mut y = 0;
        while y < screen.rows() {
            let point = Point {
                x: u16::try_from(x).expect("column"),
                y: u16::try_from(y).expect("row"),
            };
            if read_ink(screen, point) != rest || screen.cell_at(x, y).expect("cell").text() != RULE
            {
                y += 1;
                continue;
            }
            let start = y;
            y += 1;
            while y < screen.rows() {
                let point = Point {
                    x: u16::try_from(x).expect("column"),
                    y: u16::try_from(y).expect("row"),
                };
                if read_ink(screen, point) != rest
                    || screen.cell_at(x, y).expect("cell").text() != RULE
                {
                    break;
                }
                y += 1;
            }
            if y - start >= 8 {
                runs.push((
                    u16::try_from(x).expect("column"),
                    u16::try_from(start).expect("row"),
                    u16::try_from(y - start).expect("run"),
                ));
            }
        }
    }
    runs.into_iter()
        .filter(|(x, _, _)| u32::from(*x) > screen.cols() / 8 && u32::from(*x) < screen.cols() / 2)
        .max_by_key(|(_, _, len)| *len)
        .unwrap_or_else(|| panic!("manager seam not found\n{}", text(screen)))
}

fn seam_column_is(
    screen: &TerminalSnapshot,
    run: (u16, u16, u16),
    expected: Ink,
    glyph: &str,
) -> bool {
    (0..run.2).all(|offset| {
        let point = Point {
            x: run.0,
            y: run.1 + offset,
        };
        read_ink(screen, point) == expected
            && screen
                .cell_at(u32::from(point.x), u32::from(point.y))
                .is_some_and(|cell| cell.text() == glyph)
    })
}

fn assert_seam_sides(before: &TerminalSnapshot, after: &TerminalSnapshot, run: (u16, u16, u16)) {
    for offset in 0..run.2 {
        for dx in [-1i32, 1] {
            let x = i32::from(run.0) + dx;
            if x < 0 || x >= i32::try_from(before.cols()).expect("columns") {
                continue;
            }
            let point = Point {
                x: u16::try_from(x).expect("column"),
                y: run.1 + offset,
            };
            let old = read_ink(before, point);
            let now = read_ink(after, point);
            let glyph = after
                .cell_at(u32::from(point.x), u32::from(point.y))
                .expect("side cell")
                .text();
            let old_glyph = before
                .cell_at(u32::from(point.x), u32::from(point.y))
                .expect("rest side cell")
                .text();
            assert_eq!(now, old, "manager seam side cell {point:?} changed");
            assert_eq!(
                glyph, old_glyph,
                "manager seam side glyph {point:?} changed"
            );
            assert_ne!(now.fg, color_rgb(0x48, 0xe0, 0x54));
            assert_ne!(glyph, HEAVY);
        }
    }
}

fn assert_seam_padding_clear(screen: &TerminalSnapshot, run: (u16, u16, u16)) {
    for offset in 0..run.2 {
        for dx in [-1i32, 1] {
            let x = i32::from(run.0) + dx;
            if x < 0 || x >= i32::try_from(screen.cols()).expect("columns") {
                continue;
            }
            let point = Point {
                x: u16::try_from(x).expect("column"),
                y: run.1 + offset,
            };
            let cell = screen
                .cell_at(u32::from(point.x), u32::from(point.y))
                .expect("padding cell");
            assert_ne!(cell.text(), HEAVY, "padding cell {point:?} became a seam");
            assert_ne!(
                cell.fg(),
                color_rgb(0x48, 0xe0, 0x54),
                "padding cell {point:?} took the seam accent"
            );
        }
    }
}

#[test]
fn jackin_manager_focus_paints_phosphor_on_the_selected_row() {
    let app = jackin_scenario("x03-focus-jackin", "hard-cases");
    let screen = app.wait_text(CURRENT);
    let bar = super::support::find_all(&screen, BAR);
    assert_eq!(bar.len(), 1, "focus bars {bar:?}\n{}", text(&screen));
    let bar = bar[0];
    let row = super::support::find_all(&screen, CURRENT)
        .into_iter()
        .find(|point| point.y == bar.y)
        .unwrap_or_else(|| panic!("Current directory is not on focus row\n{}", text(&screen)));
    assert!(row.x > bar.x, "selected label precedes focus bar");
    assert_eq!(
        manager_focus_ink(&screen, row),
        (color_rgb(0x48, 0xe0, 0x54), color_rgb(0x17, 0x3f, 0x1c))
    );
    assert!(
        app.child_running(),
        "Jackin exited before manager paint assertion"
    );
    app.kill_child();
    assert!(!app.child_running(), "Jackin child still running");
}

#[test]
fn jackin_manager_seam_paints_rest_hover_and_pressed_states() {
    let app = jackin_scenario("x03-chrome-jackin", "hard-cases");
    let before = app.wait_text(CURRENT);
    let run = seam_run(&before);
    let rest = ink(color_rgb(0x4d, 0x4d, 0x4d), color_rgb(0, 0, 0), 0);
    let hover = ink(color_rgb(0x80, 0x80, 0x80), color_rgb(0, 0, 0), 0);
    let pressed = ink(color_rgb(0x48, 0xe0, 0x54), color_rgb(0, 0, 0), 0);
    assert!(
        seam_column_is(&before, run, rest, RULE),
        "rest seam\n{}",
        text(&before)
    );

    let anchor = Point { x: run.0, y: run.1 };
    app.move_to(anchor);
    let hovered = app.wait_screen("Jackin seam hover", |screen| {
        seam_column_is(screen, run, hover, HEAVY)
    });
    assert!(seam_column_is(&hovered, run, hover, HEAVY));
    assert_seam_sides(&before, &hovered, run);
    app.down(anchor);
    let _pressed = app.wait_screen("Jackin seam pressed", |screen| {
        seam_column_is(screen, run, pressed, HEAVY)
    });
    app.drag_to(anchor);
    let dragged = app.wait_screen("Jackin seam drag", |screen| {
        seam_column_is(screen, run, pressed, HEAVY)
    });
    assert!(seam_column_is(&dragged, run, pressed, HEAVY));
    assert_seam_sides(&before, &dragged, run);
    assert_seam_padding_clear(&dragged, run);
    assert_ne!(read_ink(&before, anchor), read_ink(&hovered, anchor));
    assert_ne!(read_ink(&hovered, anchor), read_ink(&dragged, anchor));
    assert_ne!(read_ink(&before, anchor), read_ink(&dragged, anchor));
    app.up(anchor);
    app.kill_child();
}

#[test]
fn jackin_no_color_caption_and_motion_precedence() {
    assert_jackin_no_color_accent();
    assert_jackin_motion_precedence();
}

const CAPTION: &str = "Knock, knock, operator.";
const CONTINUE: &str = "Continue";
const SKIP: &str = "Skip";
const JACKIN_ABOUT: &str =
    "Jackin-preview — Jackin redesigned on the Junie design system (deterministic preview)";
const JACKIN_HELP_DETAIL: &str =
    "Everything is simulated in memory; the real Jackin CLI is never touched.";
const JACKIN_UI: &str = "Knock, knock, operator.";
const CLEAN_ID: &str = "9b02";
const DIRTY_ID: &str = "7f3a";
const CURRENT: &str = "Current directory";
const WORKSPACES: &str = "Workspaces";
const CLEAN_IDENTITY: &str = "infra-control-plane › sre";
const CLEAN_QUIT: &str = "Quit jackin❯?";
const FORCE_STOP: &str = "force-stops the container";
const UNSAVED: &str = "Unsaved work — quit?";
const CLEAN_LIFE: &str = "The final session ended cleanly";
const STILL_INSIDE: &str = "Still inside the Construct";
const LAUNCH_VERSION: &str = "Claude Code v2.1.14";
const LAUNCH_PANE: &str = "Ctrl+B Pane commands";
const LAUNCH_TAB: &str = "Claude Code (Work)";

#[test]
fn jackin_cli_help_and_invalid_scenario() {
    let mut launch = Launch::new("jackin-help", JACKIN);
    launch.args = vec!["--help".into()];
    launch.cols = 200;
    launch.rows = 80;
    let help = try_launch(launch).expect("launch Jackin help");
    let help_screen = output_screen(&help, JACKIN_ABOUT);
    assert!(text(&help_screen).contains(JACKIN_ABOUT));
    assert!(text(&help_screen).contains(JACKIN_HELP_DETAIL));
    assert!(!text(&help_screen).contains(JACKIN_UI));
    wait_for_exit(&help, 0);

    let mut launch = Launch::new("jackin-invalid-scenario", JACKIN);
    launch.args = vec!["--scenario".into(), "not-a-scenario".into()];
    let invalid = try_launch(launch).expect("launch invalid Jackin scenario");
    let error = output_screen(&invalid, "unknown scenario \"not-a-scenario\"");
    let unwrapped = text(&error).lines().collect::<String>();
    assert!(unwrapped.contains("unknown scenario \"not-a-scenario\""));
    assert!(text(&error).contains("use one of first-use"));
    assert!(!text(&error).contains(JACKIN_UI));
    wait_for_exit(&invalid, 2);
}

/// Wait for `marker` on the screen of a process that prints and exits (a CLI
/// help or an argument error), or for the exit with the final screen.
///
/// The exit is read before the screen. The session publishes the exit only
/// after its reader has fed the last output to the terminal, so a screen
/// captured after the exit holds all of it. The other order captures first and
/// asks later: output and exit that land between the two calls (a slow capture
/// under load) return a screen from before the text.
fn output_screen(app: &App, marker: &str) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let exited = !app.child_running();
        let screen = app.screen();
        if text(&screen).contains(marker) || exited {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "output missing {marker}\n{}",
            text(&screen)
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(app: &App, code: i32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "process did not exit with code {code}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    app.expect_exited(code);
}

#[test]
fn jackin_copy_and_ctrl_c_leave_the_failure_dialog_open() {
    let mut launch = Launch::new("jackin-clipboard", JACKIN);
    launch.args = vec!["--scenario".into(), "launch-failure".into()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".into());
    launch.motion = Some("reduced".into());
    let app = try_launch(launch).expect("launch Jackin clipboard scenario");
    app.wait_text("Launch failed");
    app.wait_text("run-202609030914-b5df");
    app.key("down");
    app.key("y");
    let copied = app.wait_screen("run id copied", |screen| {
        text(screen).contains("Copied 21 characters · run-202609030914-b5df")
            && text(screen).contains("Launch failed")
    });
    assert!(app.child_running(), "clipboard action stopped Jackin");
    assert!(text(&copied).contains("Copied 21 characters · run-202609030914-b5df"));
    assert!(!text(&copied).contains("Not copied"));
    app.key("ctrl+c");
    thread::sleep(Duration::from_millis(200));
    assert!(
        app.child_running(),
        "ctrl+c quit Jackin with its dialog open"
    );
    app.wait_text("Launch failed");
    app.kill_child();
}

#[test]
fn jackin_resize_preserves_the_first_use_screen() {
    let app = App::new(
        "jackin-resize",
        JACKIN,
        &["--scenario", "first-use"],
        120,
        40,
        "truecolor",
    );
    app.wait_text(JACKIN_UI);
    app.resize(71, 20);
    let too_narrow = app.wait_screen("71x20 too small", |screen| {
        let visible = text(screen);
        screen.cols() == 71
            && screen.rows() == 20
            && visible.contains("Terminal too small")
            && visible.contains("Need 72×20, have 71×20")
            && !visible.contains(JACKIN_UI)
    });
    assert!(text(&too_narrow).contains("Need 72×20, have 71×20"));
    assert!(!text(&too_narrow).contains(JACKIN_UI));
    assert!(app.child_running(), "71×20 stopped Jackin");
    app.resize(72, 19);
    let too_short = app.wait_screen("72x19 too small", |screen| {
        let visible = text(screen);
        screen.cols() == 72
            && screen.rows() == 19
            && visible.contains("Terminal too small")
            && visible.contains("Need 72×20, have 72×19")
            && !visible.contains(JACKIN_UI)
    });
    assert!(text(&too_short).contains("Need 72×20, have 72×19"));
    assert!(!text(&too_short).contains(JACKIN_UI));
    assert!(app.child_running(), "72×19 stopped Jackin");
    app.resize(72, 20);
    let minimum = app.wait_screen("72x20 minimum restored", |screen| {
        screen.cols() == 72
            && screen.rows() == 20
            && text(screen).contains(JACKIN_UI)
            && !text(screen).contains("Terminal too small")
    });
    assert!(text(&minimum).contains(CAPTION));
    app.resize(120, 40);
    app.wait_screen("120x40 restored", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(JACKIN_UI)
            && !text(screen).contains("Terminal too small")
    });
    assert!(app.child_running(), "resize stopped Jackin");
    app.kill_child();
}

#[test]
fn jackin_first_key_keeps_the_intro_before_showing_workspace_details() {
    let app = configured_launch(
        "jackin-first-key",
        &["--scenario", "first-use"],
        Some("truecolor"),
        Some("reduced"),
        &[],
    );
    let start = app.wait_screen("intro content ready", |screen| {
        let visible = text(screen);
        visible.contains(JACKIN_UI) && visible.contains(CONTINUE)
    });
    assert!(text(&start).contains("Continue"));
    assert!(!text(&start).contains("Current directory"));
    app.key("down");
    let moved = app.wait_text("Continue");
    assert!(!text(&moved).contains("Current directory"));
    assert!(app.child_running(), "first key stopped Jackin");
    app.kill_child();
}

#[test]
fn jackin_ascii_glyph_mode_keeps_the_manager_readable() {
    let app = configured_launch(
        "x03-glyphs-jackin",
        &["--scenario", "hard-cases", "--glyphs", "ascii"],
        Some("truecolor"),
        Some("reduced"),
        &[],
    );
    let screen = app.wait_screen("Jackin ASCII glyphs", |screen| {
        text(screen).contains(CURRENT)
            && cell_count(screen, HEAVY) == 0
            && cell_count(screen, BAR) == 0
            && cell_count(screen, RULE) == 0
            && screen
                .cell_at(3, 3)
                .is_some_and(|cell| cell.text() == GUTTER)
    });
    assert!(!text(&screen).contains(HEAVY));
    assert!(!text(&screen).contains(BAR));
    assert_eq!(
        cell_count(&screen, RULE),
        0,
        "unicode seam\n{}",
        text(&screen)
    );
    assert_eq!(
        cell_count(&screen, THUMB),
        0,
        "Jackin painted a thumb\n{}",
        text(&screen)
    );
    let gutter = screen.cell_at(3, 3).expect("ASCII gutter");
    assert_eq!(gutter.text(), GUTTER);
    assert_eq!(gutter.fg(), color_rgb(0x48, 0xe0, 0x54));
    assert_eq!(gutter.bg(), color_rgb(0x17, 0x3f, 0x1c));
    assert!(
        cell_count(&screen, GUTTER) > 1,
        "Jackin painted no resting rule\n{}",
        text(&screen)
    );
    app.kill_child();
}

#[test]
fn jackin_test_clock_advances_the_launch_checkpoint_once() {
    let mut launch = Launch::new("jackin-clock", JACKIN);
    launch.args = vec!["--scenario".into(), "launch-failure".into()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".into());
    launch.motion = Some("paused".into());
    launch.clock = true;
    let app = try_launch(launch).expect("launch clock-controlled Jackin");
    let before = app.wait_text("Preparing launch");
    assert!(!text(&before).contains("Resolving launch identity"));
    app.advance_clock(33);
    let after = app.wait_text("Resolving launch identity");
    assert!(!text(&after).contains("Preparing launch"));
    thread::sleep(Duration::from_millis(200));
    assert_eq!(after, app.screen(), "launch advanced without a clock step");
    app.kill_child();
}
