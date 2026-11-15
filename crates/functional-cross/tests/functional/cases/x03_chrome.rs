//! Cross-application scrollbar and seam treatment, before the refactor.
//!
//! `scrollbar::THUMB` is `Glyphs::UNICODE.scroll_thumb` (`┃`).
//! `render_vertical` writes that glyph on every thumb cell and
//! `scroll_track` (`┆`) on the rest of the column. `thumb_style` uses
//! `Theme::interactive(Kind::Handle)`. `Splitter::render_held` paints
//! `divider` (`│`) or `h_divider` (`─`) at rest, and `scroll_thumb` (`┃`)
//! or `scroll_h_thumb` (`━`) while hovered or pressed. The style is
//! `Theme::interactive(Kind::Seam)` then `style.bg(bg)`, so the seam
//! background stays the pane background. Only `area` is painted.
//! `hit_pad` widens the hit region and does not paint padding cells.
//!
//! Truecolor `ladders.handle` is `Ladder::new(WHITE_50, WHITE_70, GREEN)`
//! and `ladders.seam` is `Ladder::new(WHITE_30, WHITE_50, GREEN)`.
//! `StateModifiers::MONO` sets `thumb_hover` to `BOLD` and `thumb_pressed`
//! to `REVERSED`. `ColorLevel::detect` returns `Mono` when `NO_COLOR` is
//! non-empty. Launch color `nocolor` sets `NO_COLOR=1` and passes no
//! `--color` flag.
//!
//! `--page scrolling` is `ScrollingPage`. `--page splits` is the alias of
//! `PageId::SplitsStacks` (`PAGE_ALIASES`). The outer seam is
//! `Splitter::new(splitsstacks/outer, SplitDir::Horizontal)` with the
//! default `hit_pad` of 1, rendered on the card background. Jackin's
//! hard-cases screen is `ManagerScreen`, whose `manager.seam` is drawn by
//! `MasterDetail::layout` on `t.canvas`.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use tui_test::terminal::cell::Attrs;
use tui_test::{SnapshotColor, SnapshotUnderline, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, Point, SHOWCASE, TABLEPRO, find_all, text, try_launch};

const BOLD: u8 = 1;
const DIM: u8 = 2;
const INVERSE: u8 = 4;
const UNDER: u8 = 8;

const TITLE: &str = "Scrolling";
const BLURB: &str = "Wheel under the pointer, keys on the focused container";
const SPLITS_TITLE: &str = "Splits & stacks";
const ROW: &str = "Row 001";
const THUMB: &str = "┃";
const TRACK: &str = "┆";
const RULE: &str = "│";

const WIDE_ID: &str = "pre_refactor/x03_chrome/x03_chrome_chrome_120x40";
const NARROW_ID: &str = "pre_refactor/x03_chrome/x03_chrome_chrome_narrow";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Paint {
    text: String,
    fg: SnapshotColor,
    bg: SnapshotColor,
    mods: u8,
}

#[derive(Clone, Copy, Debug)]
struct Run {
    x: u16,
    y: u16,
    len: u16,
}

fn rgb(r: u8, g: u8, b: u8) -> SnapshotColor {
    SnapshotColor::Rgb(r, g, b)
}

fn white30() -> SnapshotColor {
    rgb(0x4d, 0x4d, 0x4d)
}

fn white50() -> SnapshotColor {
    rgb(0x80, 0x80, 0x80)
}

fn white70() -> SnapshotColor {
    rgb(0xb3, 0xb3, 0xb3)
}

fn white28() -> SnapshotColor {
    rgb(0x47, 0x47, 0x47)
}

fn green() -> SnapshotColor {
    rgb(0x48, 0xe0, 0x54)
}

fn surface() -> SnapshotColor {
    rgb(0x11, 0x11, 0x11)
}

fn canvas() -> SnapshotColor {
    SnapshotColor::Rgb(0, 0, 0)
}

fn plain() -> SnapshotColor {
    SnapshotColor::Default
}

fn paint(text: &str, fg: SnapshotColor, bg: SnapshotColor, mods: u8) -> Paint {
    Paint {
        text: text.to_string(),
        fg,
        bg,
        mods,
    }
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

fn read_paint(screen: &TerminalSnapshot, point: Point) -> Paint {
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
    Paint {
        text: cell.text().to_string(),
        fg: cell.fg(),
        bg: cell.bg(),
        mods,
    }
}

fn assert_uncolored(screen: &TerminalSnapshot) {
    let mut rgb_count = 0;
    let mut indexed = 0;
    for y in 0..screen.rows() {
        for cell in screen.row(y).expect("row") {
            if matches!(cell.fg(), SnapshotColor::Rgb(_, _, _))
                || matches!(cell.bg(), SnapshotColor::Rgb(_, _, _))
            {
                rgb_count += 1;
            }
            if matches!(cell.fg(), SnapshotColor::Indexed(_))
                || matches!(cell.bg(), SnapshotColor::Indexed(_))
            {
                indexed += 1;
            }
        }
    }
    assert_eq!(rgb_count, 0, "NO_COLOR screen has RGB\n{}", text(screen));
    assert_eq!(
        indexed,
        0,
        "NO_COLOR screen has indexed colour\n{}",
        text(screen)
    );
}

fn colored(screen: &TerminalSnapshot) -> bool {
    screen.rows() > 0
        && screen.row(0).expect("row").iter().any(|cell| {
            matches!(cell.fg(), SnapshotColor::Rgb(_, _, _))
                || matches!(cell.bg(), SnapshotColor::Rgb(_, _, _))
        })
}

fn vertical_runs(
    screen: &TerminalSnapshot,
    glyph: &str,
    fg: SnapshotColor,
    bg: SnapshotColor,
) -> Vec<Run> {
    let mut runs = Vec::new();
    for x in 0..screen.cols() {
        let mut y = 0;
        while y < screen.rows() {
            let cell = screen.cell_at(x, y).expect("cell");
            if cell.text() == glyph && cell.fg() == fg && cell.bg() == bg {
                let start = y;
                y += 1;
                while y < screen.rows() {
                    let next = screen.cell_at(x, y).expect("cell");
                    if next.text() == glyph && next.fg() == fg && next.bg() == bg {
                        y += 1;
                    } else {
                        break;
                    }
                }
                let len = y - start;
                if len >= 8 {
                    runs.push(Run {
                        x: u16::try_from(x).expect("column"),
                        y: u16::try_from(start).expect("row"),
                        len: u16::try_from(len).expect("run"),
                    });
                }
            } else {
                y += 1;
            }
        }
    }
    runs
}

fn column_is(screen: &TerminalSnapshot, run: Run, expected: &Paint) -> bool {
    (0..run.len).all(|offset| {
        let point = Point {
            x: run.x,
            y: run.y + offset,
        };
        read_paint(screen, point) == *expected
    })
}

fn assert_column(screen: &TerminalSnapshot, run: Run, expected: &Paint) {
    assert!(
        column_is(screen, run, expected),
        "column {run:?} is not {expected:?}\n{}",
        text(screen)
    );
}

fn side_points(run: Run, cols: u32) -> Vec<Point> {
    let mut points = Vec::new();
    for offset in 0..run.len {
        let y = run.y + offset;
        for dx in [-1, 1] {
            let x = i32::from(run.x) + dx;
            if x >= 0 && u32::try_from(x).expect("x") < cols {
                points.push(Point {
                    x: u16::try_from(x).expect("x"),
                    y,
                });
            }
        }
    }
    points
}

fn assert_sides(before: &TerminalSnapshot, after: &TerminalSnapshot, run: Run) {
    for point in side_points(run, before.cols()) {
        let rest = read_paint(before, point);
        let now = read_paint(after, point);
        assert_eq!(now, rest, "padding cell {point:?} changed from {rest:?}");
        assert_ne!(now.fg, green(), "padding cell {point:?} took the accent");
        assert_ne!(
            now.text, THUMB,
            "padding cell {point:?} took the seam glyph"
        );
    }
}

fn assert_padding_clear(screen: &TerminalSnapshot, run: Run) {
    for point in side_points(run, screen.cols()) {
        let now = read_paint(screen, point);
        assert_ne!(
            now.fg,
            green(),
            "padding cell {point:?} took the accent\n{}",
            text(screen)
        );
        assert_ne!(
            now.text,
            THUMB,
            "padding cell {point:?} took the seam glyph\n{}",
            text(screen)
        );
    }
}

fn assert_only_column(before: &TerminalSnapshot, after: &TerminalSnapshot, run: Run) {
    let mut extra = Vec::new();
    for y in 0..before.rows() {
        for x in 0..before.cols() {
            let on_seam = x == u32::from(run.x)
                && y >= u32::from(run.y)
                && y < u32::from(run.y) + u32::from(run.len);
            if on_seam {
                continue;
            }
            let point = Point {
                x: u16::try_from(x).expect("column"),
                y: u16::try_from(y).expect("row"),
            };
            let left = read_paint(before, point);
            let right = read_paint(after, point);
            if left != right {
                extra.push(format!("{point:?} {left:?} -> {right:?}"));
            }
        }
    }
    assert!(
        extra.is_empty(),
        "cells outside the seam changed: {extra:?}\n{}",
        text(after)
    );
}

fn wait_glyph(app: &App, point: Point, glyph: &str) -> bool {
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        if read_paint(&app.screen(), point).text == glyph {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn column_that_thickens(
    app: &App,
    screen: &TerminalSnapshot,
    fg: SnapshotColor,
    bg: SnapshotColor,
) -> Run {
    let mut runs = vertical_runs(screen, RULE, fg, bg);
    assert!(
        !runs.is_empty(),
        "no {RULE} column at {fg:?}/{bg:?}\n{}",
        text(screen)
    );
    runs.sort_by(|left, right| right.len.cmp(&left.len).then(left.x.cmp(&right.x)));
    for run in runs {
        let anchor = Point { x: run.x, y: run.y };
        app.move_to(anchor);
        if wait_glyph(app, anchor, THUMB) {
            return run;
        }
    }
    panic!("no {RULE} column became {THUMB}\n{}", text(&app.screen()));
}

fn cycle_column(
    app: &App,
    before: &TerminalSnapshot,
    run: Run,
    rest: &Paint,
    hover: &Paint,
    pressed: &Paint,
    strict: bool,
) -> TerminalSnapshot {
    assert_ne!(rest, hover);
    assert_ne!(hover, pressed);
    assert_ne!(rest, pressed);
    assert_column(before, run, rest);
    let anchor = Point { x: run.x, y: run.y };
    app.move_to(anchor);
    let hovered = app.wait_screen("seam hover", |screen| column_is(screen, run, hover));
    assert_sides(before, &hovered, run);
    if strict {
        assert_only_column(before, &hovered, run);
    }
    app.down(anchor);
    let _down = app.wait_screen("seam press", |screen| column_is(screen, run, pressed));
    app.drag_to(anchor);
    let dragged = app.wait_screen("seam drag", |screen| column_is(screen, run, pressed));
    assert_sides(before, &dragged, run);
    assert_padding_clear(&dragged, run);
    if strict {
        assert_only_column(before, &dragged, run);
    }
    let measured = [
        read_paint(before, anchor),
        read_paint(&hovered, anchor),
        read_paint(&dragged, anchor),
    ];
    assert_ne!(measured[0], measured[1]);
    assert_ne!(measured[1], measured[2]);
    assert_ne!(measured[0], measured[2]);
    dragged
}

fn kill(app: &App) {
    assert!(app.child_running(), "child exited before the assertion");
    app.kill_child();
    assert!(!app.child_running(), "child still running");
}

fn thumb_states(color: &str) -> (Paint, Paint, Paint, Paint) {
    if color == "nocolor" {
        let plain = plain();
        (
            paint(THUMB, plain, plain, 0),
            paint(THUMB, plain, plain, BOLD),
            paint(THUMB, plain, plain, INVERSE),
            paint(TRACK, plain, plain, 0),
        )
    } else {
        (
            paint(THUMB, white50(), surface(), 0),
            paint(THUMB, white70(), surface(), 0),
            paint(THUMB, green(), surface(), 0),
            paint(TRACK, white28(), surface(), 0),
        )
    }
}

fn seam_states(color: &str, bg: SnapshotColor) -> (Paint, Paint, Paint) {
    if color == "nocolor" {
        let plain = plain();
        (
            paint(RULE, plain, plain, 0),
            paint(THUMB, plain, plain, BOLD),
            paint(THUMB, plain, plain, INVERSE),
        )
    } else {
        (
            paint(RULE, white30(), bg, 0),
            paint(THUMB, white50(), bg, 0),
            paint(THUMB, green(), bg, 0),
        )
    }
}

fn one_row(screen: &TerminalSnapshot) -> Point {
    let hits = find_all(screen, ROW);
    assert_eq!(hits.len(), 1, "{ROW} hits {hits:?}\n{}", text(screen));
    hits[0]
}

fn thumb_near(screen: &TerminalSnapshot) -> Option<Point> {
    if !text(screen).contains(ROW) {
        return None;
    }
    let row = one_row(screen);
    (row.x..row.x + 30)
        .flat_map(|x| (row.y..row.y + 3).map(move |y| Point { x, y }))
        .find(|point| read_paint(screen, *point).text == THUMB)
}

fn scrollbar(color: &str) {
    let app = open(
        &format!("x03-chrome-thumb-{color}"),
        SHOWCASE,
        &["--page", "scrolling"],
        color,
    );
    let screen = app.wait_screen("long list thumb", |screen| {
        thumb_near(screen).is_some() && text(screen).contains(BLURB)
    });
    if color == "nocolor" {
        assert_uncolored(&screen);
    }
    let (rest, hover, pressed, track) = thumb_states(color);
    let thumb = thumb_near(&screen).expect("thumb");
    assert_eq!(read_paint(&screen, thumb), rest);
    let below = Point {
        x: thumb.x,
        y: thumb.y + 1,
    };
    assert_eq!(read_paint(&screen, below), track);
    app.move_to(thumb);
    let hovered = app.wait_screen("thumb hover", |screen| read_paint(screen, thumb) == hover);
    assert_eq!(read_paint(&hovered, thumb).text, THUMB);
    app.down(thumb);
    let held = app.wait_screen("thumb press", |screen| read_paint(screen, thumb) == pressed);
    assert_eq!(read_paint(&held, thumb).text, THUMB);
    let dropped = Point {
        x: thumb.x,
        y: thumb.y + 2,
    };
    app.drag_to(dropped);
    let moved = app.wait_screen("thumb drag", |screen| {
        read_paint(screen, thumb) == track && read_paint(screen, dropped) == pressed
    });
    assert_eq!(read_paint(&moved, dropped).text, THUMB);
    assert_ne!(rest, hover);
    assert_ne!(hover, pressed);
    assert_ne!(rest, pressed);
    if color == "nocolor" {
        assert_uncolored(&moved);
        assert_eq!(hover.mods, BOLD);
        assert_eq!(pressed.mods, INVERSE);
        assert_eq!(rest.mods, 0);
    } else {
        assert_ne!(rest.fg, hover.fg);
        assert_ne!(hover.fg, pressed.fg);
        assert_ne!(rest.fg, pressed.fg);
    }
    app.up(dropped);
    kill(&app);
}

fn showcase_seam(color: &str) {
    let app = open(
        &format!("x03-chrome-seam-{color}"),
        SHOWCASE,
        &["--page", "splits"],
        color,
    );
    let before = app.wait_text("splitter.rs");
    require(&before, SPLITS_TITLE);
    if color == "nocolor" {
        assert_uncolored(&before);
    }
    let (fg, bg) = if color == "nocolor" {
        (plain(), plain())
    } else {
        (white30(), surface())
    };
    let (rest, hover, pressed) = seam_states(color, surface());
    let run = column_that_thickens(&app, &before, fg, bg);
    let dragged = cycle_column(&app, &before, run, &rest, &hover, &pressed, true);
    if color == "nocolor" {
        assert_uncolored(&dragged);
        assert_eq!(hover.mods, BOLD);
        assert_eq!(pressed.mods, INVERSE);
    }
    app.up(Point { x: run.x, y: run.y });
    kill(&app);
}

fn count_glyph(screen: &TerminalSnapshot, glyph: &str) -> usize {
    let mut count = 0;
    for y in 0..screen.rows() {
        for x in 0..screen.cols() {
            if screen.cell_at(x, y).expect("cell").text() == glyph {
                count += 1;
            }
        }
    }
    count
}

fn tablepro_borders() {
    let app = open("x03-chrome-tablepro", TABLEPRO, &[], "truecolor");
    let screen = app.wait_text("Filter connections");
    assert_eq!(
        count_glyph(&screen, THUMB),
        0,
        "tablepro painted a thumb\n{}",
        text(&screen)
    );
    assert_eq!(
        count_glyph(&screen, TRACK),
        0,
        "tablepro painted a track\n{}",
        text(&screen)
    );
    let borders = vertical_runs(&screen, RULE, white30(), canvas());
    assert!(
        !borders.is_empty(),
        "tablepro painted no panel border\n{}",
        text(&screen)
    );
    let border = borders[0];
    let point = Point {
        x: border.x,
        y: border.y + border.len / 2,
    };
    let rest = read_paint(&screen, point);
    app.move_to(point);
    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        assert_eq!(
            read_paint(&app.screen(), point),
            rest,
            "panel border changed on hover\n{}",
            text(&app.screen())
        );
        thread::sleep(Duration::from_millis(25));
    }
    kill(&app);
}

fn holla_tab_rule() {
    let app = open(
        "x03-chrome-holla",
        HOLLA,
        &["--scenario", "activities-multi"],
        "truecolor",
    );
    let screen = app.wait_text("Search actions");
    assert_eq!(
        count_glyph(&screen, THUMB),
        0,
        "holla painted a thumb\n{}",
        text(&screen)
    );
    assert_eq!(
        count_glyph(&screen, RULE),
        0,
        "holla painted a vertical rule\n{}",
        text(&screen)
    );
    assert_eq!(
        count_glyph(&screen, TRACK),
        0,
        "holla painted a track\n{}",
        text(&screen)
    );
    let rules = vertical_runs(&screen, "━", green(), canvas());
    assert!(
        rules.is_empty(),
        "holla painted a vertical heavy rule\n{}",
        text(&screen)
    );
    assert!(
        count_glyph(&screen, "━") > 0,
        "holla painted no tab rule\n{}",
        text(&screen)
    );
    kill(&app);
}

fn green_run(screen: &TerminalSnapshot) -> Option<Run> {
    let runs: Vec<_> = vertical_runs(screen, THUMB, green(), surface())
        .into_iter()
        .filter(|run| run.len >= 8)
        .collect();
    (runs.len() == 1).then(|| runs[0])
}

fn pressed_seam(screen: &TerminalSnapshot) -> Run {
    green_run(screen).unwrap_or_else(|| panic!("pressed seam missing\n{}", text(screen)))
}

fn settled_resize(screen: &TerminalSnapshot, cols: u32, rows: u32, absent: &str) -> Option<Run> {
    if screen.cols() != cols || screen.rows() != rows || !colored(screen) {
        return None;
    }
    let visible = text(screen);
    if !visible.contains("Junie")
        || !visible.contains("Explorer")
        || !visible.contains("splitter.rs")
        || !visible.contains("Move")
        || visible.contains(absent)
    {
        return None;
    }
    green_run(screen)
}

fn checkpoint_frames(app: &App) -> (TerminalSnapshot, TerminalSnapshot) {
    let before = app.wait_screen("splits page", |screen| {
        text(screen).contains("splitter.rs")
            && text(screen).contains(SPLITS_TITLE)
            && colored(screen)
    });
    let (rest, hover, pressed) = seam_states("truecolor", surface());
    let run = column_that_thickens(app, &before, white30(), surface());
    let wide = cycle_column(app, &before, run, &rest, &hover, &pressed, true);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    let wide_seam = pressed_seam(&wide);
    assert_padding_clear(&wide, wide_seam);
    app.resize(80, 24);
    let narrow_seen = std::cell::Cell::new(None);
    let narrow = app.wait_screen("pressed seam at 80x24", |screen| {
        let Some(run) = settled_resize(screen, 80, 24, "120×40") else {
            narrow_seen.set(None);
            return false;
        };
        let key = (run.x, run.y, run.len);
        if narrow_seen.get() == Some(key) {
            true
        } else {
            narrow_seen.set(Some(key));
            false
        }
    });
    let narrow_seam = pressed_seam(&narrow);
    assert_padding_clear(&narrow, narrow_seam);
    app.resize(120, 40);
    let back_seen = std::cell::Cell::new(None);
    let back = app.wait_screen("pressed seam back at 120x40", |screen| {
        let Some(run) = settled_resize(screen, 120, 40, "80×24") else {
            back_seen.set(None);
            return false;
        };
        let key = (run.x, run.y, run.len);
        if back_seen.get() == Some(key) {
            true
        } else {
            back_seen.set(Some(key));
            false
        }
    });
    let _back_seam = pressed_seam(&back);
    assert_padding_clear(&back, pressed_seam(&back));
    app.up(Point { x: run.x, y: run.y });
    assert!(app.child_running(), "splits page exited before Ctrl+Q");
    (wide, narrow)
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

fn route() {
    let app = open(
        "x03-chrome-route",
        SHOWCASE,
        &["--page", "scrolling"],
        "truecolor",
    );
    let page = app.wait_text(BLURB);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    require(&page, TITLE);
    let header = text(&page)
        .lines()
        .find(|line| line.contains(BLURB))
        .expect("blurb row")
        .to_string();
    assert!(
        header.contains(TITLE),
        "page identity is not {TITLE}: {header}"
    );
    assert!(
        !header.contains(SPLITS_TITLE),
        "page identity used {SPLITS_TITLE}: {header}"
    );
    assert!(
        app.child_running(),
        "the scrolling page exited before Ctrl+Q"
    );
    app.finish();
}

fn checkpoint() {
    let app = open(
        "x03-chrome-checkpoint",
        SHOWCASE,
        &["--page", "splits"],
        "truecolor",
    );
    let (wide, narrow) = checkpoint_frames(&app);
    app.finish();
    check_stored(&app, &wide, WIDE_ID);
    check_stored(&app, &narrow, NARROW_ID);
}

#[test]
fn x03_chrome_route() {
    route();
}

#[test]
fn x03_chrome_chrome() {
    scrollbar("truecolor");
    scrollbar("nocolor");
    showcase_seam("nocolor");
    tablepro_borders();
    holla_tab_rule();
    checkpoint();
}
