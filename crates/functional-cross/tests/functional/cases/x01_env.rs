//! Cross-application colour, glyph, and motion precedence.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/x01_env/`. One showcase session captures 120×40, resizes to
//! 80×24, then resizes back. `Launch::new` leaves the clock off.
//!
//! `--color truecolor` with `NO_COLOR=1` selects `ColorLevel::TrueColor` in
//! `CommonArgs::level`, so the chrome reads `truecolor`. Crossterm 0.29 writes
//! no SGR while `NO_COLOR` is non-empty, and this crate does not call
//! `force_color_output`, so the captured cells stay `SnapshotColor::Default`.
//! The accent is not RGB. That is the reference paint.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{
    App, HOLLA, Launch, SHOWCASE, TABLEPRO, find_all, row_text, text, try_launch,
};

const IDENTITY: &str = "Tokens and principles behind every component";
const OVERVIEW: &str = "Overview";
const UNICODE_ROW: &str = "▎› Overview";
const ASCII_ROW: &str = "|› Overview";
const FOCUS: &str = "▎";
const ASCII_FOCUS: &str = "|";
const ACCENT: SnapshotColor = SnapshotColor::Rgb(72, 224, 84);
const INDEXED_256: SnapshotColor = SnapshotColor::Indexed(78);
const INDEXED_16: SnapshotColor = SnapshotColor::Indexed(10);
const INDEXED_NONE: SnapshotColor = SnapshotColor::Indexed(15);
const SPINNER: &str = "⠋";
const RUNNER: &str = "Waiting for the test runner";
const DISCOVERING: &str = "discovering";
const HOLLA_READY: &str = "Search actions";
const HOST: &str = "dev-db.internal.acme.io";
const PHASES: [&str; 3] = ["Opening SSH tunnel", "Authenticating", "Loading schema"];
const WIDE_ID: &str = "pre_refactor/x01_env/x01_env_env_120x40";
const NARROW_ID: &str = "pre_refactor/x01_env/x01_env_env_narrow";

fn open(
    name: &str,
    bin: &str,
    args: &[&str],
    color: Option<&str>,
    motion: Option<&str>,
    env: &[(&str, &str)],
) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.color = color.map(str::to_string);
    launch.motion = motion.map(str::to_string);
    launch.env = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn wait_for(app: &App, needle: &str) -> TerminalSnapshot {
    app.wait_screen(needle, |screen| text(screen).contains(needle))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn glyph_fg(screen: &TerminalSnapshot, glyph: &str) -> SnapshotColor {
    for row in 0..screen.rows() {
        for cell in screen.row(row).expect("row") {
            if cell.text() == glyph {
                return cell.fg();
            }
        }
    }
    panic!("missing cell {glyph:?}\n{}", text(screen));
}

fn has_cell(screen: &TerminalSnapshot, glyph: &str) -> bool {
    (0..screen.rows()).any(|row| {
        screen
            .row(row)
            .expect("row")
            .iter()
            .any(|cell| cell.text() == glyph)
    })
}

fn leading_fg(screen: &TerminalSnapshot, needle: &str) -> (String, SnapshotColor) {
    let hit = find_all(screen, needle)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("missing {needle}\n{}", text(screen)));
    let cell = screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .expect("cell");
    (cell.text().to_string(), cell.fg())
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

fn showcase_screen(
    name: &str,
    args: &[&str],
    color: Option<&str>,
    motion: Option<&str>,
    env: &[(&str, &str)],
    needle: &str,
) -> TerminalSnapshot {
    let app = open(name, SHOWCASE, args, color, motion, env);
    let screen = wait_for(&app, needle);
    app.kill_child();
    screen
}

fn assert_color_levels() {
    let expected = [
        ("truecolor", ACCENT, true),
        ("256", INDEXED_256, false),
        ("16", INDEXED_16, false),
        ("none", INDEXED_NONE, false),
    ];
    for (level, focus, rgb) in expected {
        let screen = showcase_screen(
            &format!("x01-env-color-{level}"),
            &[],
            Some(level),
            Some("reduced"),
            &[],
            OVERVIEW,
        );
        assert_eq!(glyph_fg(&screen, FOCUS), focus, "--color {level}");
        assert_eq!(rgb_cells(&screen) > 0, rgb, "--color {level} rgb");
        assert!(has_cell(&screen, FOCUS), "--color {level} focus glyph");
    }
}

fn assert_glyphs() {
    let unicode = showcase_screen(
        "x01-env-glyphs-unicode",
        &["--glyphs", "unicode"],
        Some("truecolor"),
        Some("reduced"),
        &[],
        OVERVIEW,
    );
    require(&unicode, UNICODE_ROW);
    assert!(!has_cell(&unicode, ASCII_FOCUS));
    assert_eq!(glyph_fg(&unicode, FOCUS), ACCENT);

    let ascii = showcase_screen(
        "x01-env-glyphs-ascii",
        &["--glyphs", "ascii"],
        Some("truecolor"),
        Some("reduced"),
        &[],
        OVERVIEW,
    );
    require(&ascii, ASCII_ROW);
    refuse(&ascii, UNICODE_ROW);
    assert_eq!(glyph_fg(&ascii, ASCII_FOCUS), ACCENT);

    let from_env = showcase_screen(
        "x01-env-glyphs-env",
        &[],
        Some("truecolor"),
        Some("reduced"),
        &[("JUNIE_TUI_GLYPHS", "ascii")],
        OVERVIEW,
    );
    require(&from_env, ASCII_ROW);
    refuse(&from_env, UNICODE_ROW);
    assert_eq!(glyph_fg(&from_env, ASCII_FOCUS), ACCENT);

    let flag = showcase_screen(
        "x01-env-glyphs-flag",
        &["--glyphs", "unicode"],
        Some("truecolor"),
        Some("reduced"),
        &[("JUNIE_TUI_GLYPHS", "ascii")],
        OVERVIEW,
    );
    require(&flag, UNICODE_ROW);
    assert!(!has_cell(&flag, ASCII_FOCUS));
    assert_eq!(glyph_fg(&flag, FOCUS), ACCENT);
}

fn assert_plain_accent(name: &str, bin: &str, ready: &str, accent: SnapshotColor, at: &str) {
    let plain = open(
        &format!("x01-env-nocolor-{name}"),
        bin,
        &[],
        Some("nocolor"),
        Some("reduced"),
        &[],
    );
    let plain_screen = wait_for(&plain, ready);
    let (plain_text, plain_fg) = leading_fg(&plain_screen, at);
    assert_eq!(rgb_cells(&plain_screen), 0, "{name} NO_COLOR rgb");
    assert_eq!(plain_fg, SnapshotColor::Default, "{name} NO_COLOR {at}");
    plain.kill_child();

    let color = open(
        &format!("x01-env-truecolor-{name}"),
        bin,
        &[],
        Some("truecolor"),
        Some("reduced"),
        &[],
    );
    let color_screen = wait_for(&color, ready);
    let (color_text, color_fg) = leading_fg(&color_screen, at);
    assert_eq!(color_text, plain_text, "{name} accent cell");
    assert_eq!(color_fg, accent, "{name} truecolor {at}");
    assert!(rgb_cells(&color_screen) > 0, "{name} truecolor rgb");
    color.kill_child();
}

fn assert_nocolor_each_app() {
    assert_plain_accent("showcase", SHOWCASE, OVERVIEW, ACCENT, FOCUS);
    assert_plain_accent("holla", HOLLA, HOLLA_READY, ACCENT, FOCUS);
    assert_plain_accent("tablepro", TABLEPRO, "Filter connections", ACCENT, FOCUS);
}

fn assert_color_flag_with_nocolor() {
    let screen = showcase_screen(
        "x01-env-color-flag",
        &[],
        Some("truecolor"),
        Some("reduced"),
        &[("NO_COLOR", "1")],
        OVERVIEW,
    );
    require(&screen, "truecolor ·");
    refuse(&screen, "no color");
    assert_eq!(glyph_fg(&screen, FOCUS), SnapshotColor::Default);
    assert_eq!(rgb_cells(&screen), 0);
    assert_ne!(glyph_fg(&screen, FOCUS), ACCENT);
}

fn motion_env<'a>(key: &'a str, value: Option<&'a str>) -> Vec<(&'a str, &'a str)> {
    match value {
        Some(value) => vec![(key, value)],
        None => Vec::new(),
    }
}

fn spinner_held(name: &str, motion: Option<&str>, value: Option<&str>) -> bool {
    let env = motion_env("SHOWCASE_NO_MOTION", value);
    let app = open(
        name,
        SHOWCASE,
        &["--page", "progress"],
        Some("truecolor"),
        motion,
        &env,
    );
    let appeared = wait_for(&app, RUNNER);
    require(&appeared, RUNNER);
    std::thread::sleep(Duration::from_millis(500));
    let screen = app.screen();
    require(&screen, RUNNER);
    let held = text(&screen).contains(SPINNER);
    app.kill_child();
    held
}

fn holla_discovering(name: &str, motion: Option<&str>, value: Option<&str>) -> bool {
    let env = motion_env("HOLLA_NO_MOTION", value);
    let app = open(name, HOLLA, &[], Some("truecolor"), motion, &env);
    let screen = wait_for(&app, HOLLA_READY);
    let discovering = text(&screen).contains(DISCOVERING);
    app.kill_child();
    discovering
}

fn spinner_near(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let hit = find_all(screen, label).into_iter().find(|hit| hit.x >= 2)?;
    let row = screen.row(u32::from(hit.y))?;
    let cell = row.get(usize::from(hit.x - 2))?;
    Some(cell.text().to_string())
}

fn tablepro_spinners(name: &str, motion: Option<&str>, value: Option<&str>) -> Vec<String> {
    let env = motion_env("TABLEPRO_NO_MOTION", value);
    let app = open(name, TABLEPRO, &[], Some("truecolor"), motion, &env);
    let listed = app.wait_text("Filter connections");
    let hits: Vec<_> = find_all(&listed, "Development")
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .collect();
    assert_eq!(hits.len(), 1, "development {hits:?}\n{}", text(&listed));
    app.click(hits[0]);
    let detail = app.wait_text(HOST);
    let hits: Vec<_> = find_all(&detail, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&detail, u32::from(hit.y));
            hit.x >= 40 && row.contains("Edit")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect {hits:?}\n{}", text(&detail));
    app.click(hits[0]);

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut glyphs = Vec::new();
    while Instant::now() < deadline {
        let screen = app.screen();
        if text(&screen).contains("Connected to Development") {
            break;
        }
        for label in PHASES {
            if let Some(glyph) = spinner_near(&screen, label)
                && !glyphs.contains(&glyph)
            {
                glyphs.push(glyph);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    app.kill_child();
    glyphs
}

fn assert_motion() {
    assert!(spinner_held(
        "x01-env-motion-showcase-reduced",
        Some("reduced"),
        None
    ));
    assert!(spinner_held("x01-env-motion-showcase-env", None, Some("1")));
    assert!(!spinner_held(
        "x01-env-motion-showcase-full",
        Some("full"),
        Some("1")
    ));
    assert!(!spinner_held(
        "x01-env-motion-showcase-zero",
        None,
        Some("0")
    ));

    assert!(!holla_discovering(
        "x01-env-motion-holla-reduced",
        Some("reduced"),
        None
    ));
    assert!(!holla_discovering(
        "x01-env-motion-holla-env",
        None,
        Some("1")
    ));
    assert!(holla_discovering(
        "x01-env-motion-holla-full",
        Some("full"),
        Some("1")
    ));
    assert!(holla_discovering(
        "x01-env-motion-holla-zero",
        None,
        Some("0")
    ));

    for (name, motion, value, held) in [
        (
            "x01-env-motion-tablepro-reduced",
            Some("reduced"),
            None,
            true,
        ),
        ("x01-env-motion-tablepro-env", None, Some("1"), true),
        (
            "x01-env-motion-tablepro-full",
            Some("full"),
            Some("1"),
            false,
        ),
        ("x01-env-motion-tablepro-zero", None, Some("0"), false),
    ] {
        let glyphs = tablepro_spinners(name, motion, value);
        assert!(!glyphs.is_empty(), "{name} saw no connect spinner");
        assert_eq!(
            glyphs.iter().all(|glyph| glyph == SPINNER),
            held,
            "{name} spinners {glyphs:?}"
        );
    }
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
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

fn assert_checkpoints() {
    let app = open(
        "x01-env-checkpoint",
        SHOWCASE,
        &[],
        Some("truecolor"),
        Some("reduced"),
        &[],
    );
    let wide = app.wait_screen(WIDE_ID, |screen| {
        text(screen).contains(OVERVIEW) && screen.cols() == 120 && screen.rows() == 40
    });
    require(&wide, IDENTITY);
    assert_eq!(glyph_fg(&wide, FOCUS), ACCENT);
    assert_eq!(wide.cols(), 120);
    assert_eq!(wide.rows(), 40);
    check_frame(&app, &wide, WIDE_ID);

    app.resize(80, 24);
    let narrow = app.wait_screen(NARROW_ID, |screen| {
        text(screen).contains(OVERVIEW) && screen.cols() == 80 && screen.rows() == 24
    });
    require(&narrow, IDENTITY);
    assert_eq!(glyph_fg(&narrow, FOCUS), ACCENT);
    assert_eq!(narrow.cols(), 80);
    assert_eq!(narrow.rows(), 24);
    check_frame(&app, &narrow, NARROW_ID);

    app.resize(120, 40);
    let restored = app.wait_screen("restored", |screen| {
        text(screen).contains(IDENTITY) && screen.cols() == 120 && screen.rows() == 40
    });
    require(&restored, IDENTITY);
    app.finish();
}

#[test]
fn x01_env_route() {
    let app = open(
        "x01-env-route",
        SHOWCASE,
        &[],
        Some("truecolor"),
        Some("reduced"),
        &[],
    );
    let screen = wait_for(&app, IDENTITY);
    require(&screen, IDENTITY);
    require(&screen, OVERVIEW);
    app.finish();
}

#[test]
fn x01_env_env() {
    assert_color_levels();
    assert_glyphs();
    assert_nocolor_each_app();
    assert_color_flag_with_nocolor();
    assert_motion();
    assert_checkpoints();
}
