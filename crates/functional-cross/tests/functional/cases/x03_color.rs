//! Cross-application colour treatment, before the refactor.
//!
//! `CommonArgs::level` in `src/cli.rs` uses an explicit `--color` before
//! `ColorLevel::detect`. A non-empty `NO_COLOR` selects `Mono`. The harness
//! sets `COLORTERM=truecolor`, so omitting `--color` is truecolor unless
//! `NO_COLOR` is set. `Button::render` clears hover and pressed when
//! disabled, then asks `Theme::interactive` for the style. `RenderCtx::state`
//! is what supplies hovered, pressed, and focused. The test reads the cells
//! the binary paints.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::terminal::cell::Attrs;
use tui_test::{SnapshotColor, SnapshotUnderline, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, Point, SHOWCASE, TABLEPRO, find_all, text, try_launch};

const BOLD: u8 = 1;
const DIM: u8 = 2;
const INVERSE: u8 = 4;
const UNDER: u8 = 8;

const PAGE: &str = "Inputs & forms › Buttons";
const BLURB: &str = "Primary, secondary, subtle, danger, toggle, disabled, busy";
const RUN: &str = "Run task";
const PREVIEW: &str = "Preview";
const DISABLED: &str = "Disabled primary";
const VERBOSE: &str = "Verbose";
const CONNECT: &str = "Connect";
const STAY: &str = "Stay";

const WIDE_ID: &str = "pre_refactor/x03_color/x03_color_matrix_120x40";
const NARROW_ID: &str = "pre_refactor/x03_color/x03_color_matrix_narrow";

const LEVELS: [&str; 5] = ["truecolor", "256", "16", "none", "nocolor"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Ink {
    fg: SnapshotColor,
    bg: SnapshotColor,
    mods: u8,
}

fn rgb(r: u8, g: u8, b: u8) -> SnapshotColor {
    SnapshotColor::Rgb(r, g, b)
}

fn idx(index: u8) -> SnapshotColor {
    SnapshotColor::Indexed(index)
}

fn ink(fg: SnapshotColor, bg: SnapshotColor, mods: u8) -> Ink {
    Ink { fg, bg, mods }
}

fn open(name: &str, bin: &str, args: &[&str], color: &str) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some(color.to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = args.iter().copied().map(str::to_string).collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn one(screen: &TerminalSnapshot, needle: &str) -> Point {
    let hits: Vec<_> = find_all(screen, needle)
        .into_iter()
        .filter(|hit| hit.y > 2 && u32::from(hit.y) + 2 < screen.rows())
        .collect();
    assert_eq!(hits.len(), 1, "{needle} hits {hits:?}\n{}", text(screen));
    hits[0]
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
    Ink {
        fg: cell.fg(),
        bg: cell.bg(),
        mods,
    }
}

fn wait_ink(app: &App, point: Point, expected: Ink) -> Ink {
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
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn hold_ink(app: &App, point: Point, expected: Ink) {
    let deadline = Instant::now() + Duration::from_millis(200);
    loop {
        let got = read_ink(&app.screen(), point);
        assert_eq!(got, expected, "cell {point:?} changed");
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn cancel_press(app: &App) {
    let away = Point { x: 1, y: 1 };
    app.drag_to(away);
    app.up(away);
}

fn rgb_cells(screen: &TerminalSnapshot) -> usize {
    let mut count = 0;
    for row in 0..screen.rows() {
        for cell in screen.row(row).expect("row") {
            if matches!(cell.fg(), SnapshotColor::Rgb(_, _, _))
                || matches!(cell.bg(), SnapshotColor::Rgb(_, _, _))
            {
                count += 1;
            }
        }
    }
    count
}

fn primary(color: &str) -> (Ink, Ink, Ink) {
    match color {
        "truecolor" => (
            ink(rgb(25, 25, 28), rgb(72, 224, 84), BOLD),
            ink(rgb(25, 25, 28), rgb(58, 179, 67), BOLD),
            ink(rgb(25, 25, 28), rgb(46, 154, 56), BOLD),
        ),
        "256" => (
            ink(idx(232), idx(78), BOLD),
            ink(idx(232), idx(71), BOLD),
            ink(idx(232), idx(65), BOLD),
        ),
        "16" => (
            ink(idx(0), idx(10), BOLD),
            ink(idx(0), idx(2), BOLD),
            ink(idx(0), idx(10), BOLD | INVERSE),
        ),
        "none" => (
            ink(idx(0), idx(15), BOLD | INVERSE),
            ink(idx(0), idx(15), BOLD | INVERSE | UNDER),
            ink(idx(0), idx(15), BOLD | UNDER),
        ),
        "nocolor" => {
            let plain = SnapshotColor::Default;
            (
                ink(plain, plain, BOLD | INVERSE),
                ink(plain, plain, BOLD | INVERSE | UNDER),
                ink(plain, plain, BOLD | UNDER),
            )
        }
        _ => unreachable!("colour {color}"),
    }
}

fn disabled_ink(color: &str) -> Ink {
    match color {
        "truecolor" => ink(rgb(77, 77, 77), rgb(39, 39, 42), 0),
        "256" => ink(idx(239), idx(235), 0),
        "16" | "none" => ink(idx(7), idx(8), DIM),
        "nocolor" => ink(SnapshotColor::Default, SnapshotColor::Default, DIM),
        _ => unreachable!("colour {color}"),
    }
}

fn selected_ink(color: &str) -> Ink {
    match color {
        "truecolor" => ink(rgb(72, 224, 84), rgb(39, 39, 42), 0),
        "256" => ink(idx(78), idx(235), 0),
        "16" => ink(idx(10), idx(8), 0),
        "none" => ink(idx(15), idx(8), 0),
        "nocolor" => ink(SnapshotColor::Default, SnapshotColor::Default, 0),
        _ => unreachable!("colour {color}"),
    }
}

fn distinct(name: &str, states: &[(&str, Ink)]) {
    for (i, (left_name, left)) in states.iter().enumerate() {
        for (right_name, right) in states.iter().skip(i + 1) {
            assert_ne!(
                left, right,
                "{name} {left_name} and {right_name} paint the same cell"
            );
        }
    }
}

fn chrome(color: &str) -> &'static str {
    match color {
        "truecolor" => "truecolor",
        "256" => "256 colors",
        "16" => "16 colors",
        "none" | "nocolor" => "no color",
        _ => unreachable!("colour {color}"),
    }
}

fn gesture(app: &App, point: Point, rest: Ink, hover: Ink, pressed: Ink) {
    assert_eq!(read_ink(&app.screen(), point), rest, "rest");
    app.move_to(point);
    assert_eq!(wait_ink(app, point, hover), hover, "hover");
    app.down(point);
    assert_eq!(wait_ink(app, point, pressed), pressed, "pressed");
    cancel_press(app);
}

fn showcase_states(color: &str) {
    let app = open(
        &format!("x03-color-showcase-{color}"),
        SHOWCASE,
        &["--page", "buttons"],
        color,
    );
    let screen = app.wait_text(DISABLED);
    require(&screen, PAGE);
    require(&screen, BLURB);
    require(&screen, chrome(color));
    require(&screen, RUN);
    require(&screen, VERBOSE);
    let run = one(&screen, RUN);
    let disabled = one(&screen, DISABLED);
    let verbose = one(&screen, VERBOSE);
    let mark = one(&screen, "●");
    assert_eq!(mark.y, verbose.y, "on-state mark is not on the Verbose row");
    assert!(mark.x < verbose.x, "on-state mark does not lead Verbose");
    let (rest, hover, pressed) = primary(color);
    let off = disabled_ink(color);
    let on = selected_ink(color);
    assert_eq!(read_ink(&screen, run), rest, "{color} rest");
    assert_eq!(read_ink(&screen, disabled), off, "{color} disabled");
    assert_eq!(read_ink(&screen, mark), on, "{color} selected");
    distinct(
        color,
        &[
            ("rest", rest),
            ("hover", hover),
            ("pressed", pressed),
            ("disabled", off),
            ("selected", on),
        ],
    );
    if color == "nocolor" {
        assert_eq!(rgb_cells(&screen), 0, "NO_COLOR rgb cells");
        assert_ne!(rest.fg, rgb(72, 224, 84), "NO_COLOR accent");
    }
    if color == "none" {
        assert_ne!(
            rest.fg,
            SnapshotColor::Default,
            "--color none stays indexed"
        );
    }
    app.move_to(disabled);
    hold_ink(&app, disabled, off);
    app.down(disabled);
    hold_ink(&app, disabled, off);
    app.up(disabled);
    app.move_to(Point { x: 1, y: 1 });
    gesture(&app, run, rest, hover, pressed);
    app.kill_child();
}

fn connect_states(color: &str) {
    let app = open(&format!("x03-color-tablepro-{color}"), TABLEPRO, &[], color);
    let screen = app.wait_text("Filter connections");
    require(&screen, chrome(color));
    require(&screen, "Edit");
    let connect = one(&screen, CONNECT);
    let (rest, hover, pressed) = primary(color);
    distinct(
        &format!("tablepro {color}"),
        &[("rest", rest), ("hover", hover), ("pressed", pressed)],
    );
    gesture(&app, connect, rest, hover, pressed);
    app.kill_child();
}

fn stay_ink(color: &str) -> (Ink, Ink, Ink) {
    match color {
        "truecolor" => (
            ink(rgb(255, 255, 255), rgb(39, 39, 42), BOLD),
            ink(rgb(255, 255, 255), rgb(55, 55, 61), BOLD),
            ink(rgb(255, 255, 255), rgb(74, 74, 82), BOLD),
        ),
        "256" => (
            ink(idx(231), idx(235), BOLD),
            ink(idx(231), idx(238), BOLD),
            ink(idx(231), idx(240), BOLD),
        ),
        "16" | "none" => (
            ink(idx(0), idx(8), BOLD | DIM),
            ink(idx(0), idx(8), BOLD | DIM | UNDER),
            ink(idx(0), idx(8), BOLD | DIM | INVERSE),
        ),
        "nocolor" => {
            let plain = SnapshotColor::Default;
            (
                ink(plain, plain, BOLD | DIM),
                ink(plain, plain, BOLD | DIM | UNDER),
                ink(plain, plain, BOLD | DIM | INVERSE),
            )
        }
        _ => unreachable!("colour {color}"),
    }
}

fn stay_states(color: &str) {
    let app = open(
        &format!("x03-color-holla-{color}"),
        HOLLA,
        &["--scenario", "activities-multi"],
        color,
    );
    app.wait_text("Search actions");
    app.key("ctrl+q");
    let screen = app.wait_text(STAY);
    require(&screen, "Quit holla");
    require(&screen, "Stop and quit");
    require(&screen, chrome(color));
    let stay = one(&screen, STAY);
    let mnemonic = read_ink(&screen, stay);
    let body = Point {
        x: stay.x + 1,
        y: stay.y,
    };
    assert_eq!(
        screen
            .cell_at(u32::from(body.x), u32::from(body.y))
            .expect("stay body")
            .text(),
        "t",
        "Stay body cell"
    );
    let (rest, hover, pressed) = stay_ink(color);
    distinct(
        &format!("holla {color}"),
        &[("rest", rest), ("hover", hover), ("pressed", pressed)],
    );
    if matches!(color, "16" | "none" | "nocolor") {
        assert_eq!(mnemonic.mods & UNDER, UNDER, "{color} mnemonic underline");
    }
    gesture(&app, body, rest, hover, pressed);
    if matches!(color, "16" | "none" | "nocolor") {
        let after = app.screen();
        let mark = one(&after, STAY);
        app.move_to(mark);
        let hovered = wait_ink(&app, mark, mnemonic);
        assert_eq!(
            hovered, mnemonic,
            "{color} Stay mnemonic cell does not show hover"
        );
    }
    app.kill_child();
}

fn buttons_up(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(PAGE)
        && visible.contains(RUN)
        && visible.contains(PREVIEW)
        && visible.contains(DISABLED)
        && visible.contains(VERBOSE)
        && !visible.contains("Tokens and principles behind every component")
}

fn frozen(app: &App, cols: u16, rows: u16) -> TerminalSnapshot {
    let _ = app.settle();
    let screen = app.stable_screen();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (u32::from(cols), u32::from(rows)),
        "stored frame size\n{}",
        text(&screen)
    );
    assert!(
        buttons_up(&screen),
        "buttons page left before the store\n{}",
        text(&screen)
    );
    let run = one(&screen, RUN);
    let (rest, _, _) = primary("truecolor");
    assert_eq!(
        read_ink(&screen, run),
        rest,
        "stored Run task is not at rest"
    );
    screen
}

fn sample_bytes(screen: &TerminalSnapshot, test_id: &str) -> Vec<u8> {
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    sample.to_bytes().expect("serialize Tuiscotti sample")
}

fn check_stored(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let bytes = sample_bytes(screen, test_id);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
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

fn checkpoint() {
    let app = open(
        "x03-color-checkpoint",
        SHOWCASE,
        &["--page", "buttons"],
        "truecolor",
    );
    let opened = app.wait_screen("buttons at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && buttons_up(screen)
    });
    require(&opened, "truecolor");
    let wide = frozen(&app, 120, 40);

    app.resize(80, 24);
    let _narrowed = app.wait_screen("buttons at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && buttons_up(screen)
    });
    let narrow = frozen(&app, 80, 24);

    app.resize(120, 40);
    let restored = app.wait_screen("buttons back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && buttons_up(screen)
    });
    require(&restored, RUN);
    assert!(app.child_running(), "the resize quit showcase");
    app.finish();
    check_stored(&app, &wide, WIDE_ID);
    check_stored(&app, &narrow, NARROW_ID);
}

#[test]
fn x03_color_route() {
    let app = open(
        "x03-color-route",
        SHOWCASE,
        &["--page", "buttons"],
        "truecolor",
    );
    let page = app.wait_text(BLURB);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    require(&page, PAGE);
    require(&page, RUN);
    require(&page, DISABLED);
    require(&page, "● Verbose");
    require(&page, "truecolor");
    assert!(
        !text(&page).contains("Tokens and principles behind every component"),
        "overview is showing"
    );
    assert!(app.child_running(), "the buttons page exited before Ctrl+Q");
    app.finish();
}

#[test]
fn x03_color_matrix() {
    for color in LEVELS {
        showcase_states(color);
        connect_states(color);
        stay_states(color);
    }
    checkpoint();
}
