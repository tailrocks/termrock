//! Cross-application Unicode and ASCII glyph treatment, before the refactor.
//!
//! `CommonArgs::glyphs` is `--glyphs unicode` (alias `utf8`) or `--glyphs
//! ascii`. An explicit flag wins over detection. `JUNIE_TUI_GLYPHS` is the
//! env override; these launches do not set it (`src/cli.rs`).
//!
//! `Glyphs::UNICODE` focus_bar is `▎`, divider is `│`, scroll_thumb is `┃`,
//! scroll_track is `┆`, and ellipsis is `…`. `Glyphs::ASCII` focus_bar is
//! `|`, divider is `|`, scroll_thumb is `#`, scroll_track is `:`, and
//! ellipsis is `~`. ASCII must not paint `▎` or `┃` (`src/theme.rs`).
//! `render_vertical` paints `g.scroll_thumb` and `g.scroll_track`
//! (`src/widgets/scrollbar.rs`). `Splitter::render_held` paints `divider`
//! at rest and `scroll_thumb` while hovered or pressed
//! (`src/widgets/splitter.rs`).
//!
//! The text-area fixture line is `Review 日本語 · 👩‍💻 · cafe\u{301}` inside
//! the numbered Unicode line (`src/bin/showcase/pages/textareas.rs`). Those
//! are document characters. The sidebar's selected marker is a hardcoded
//! `›` (`TreeView::selected_marker` in `src/bin/showcase/app.rs`), not
//! `Glyphs::current`.

use std::path::PathBuf;

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, SHOWCASE, TABLEPRO, row_text, text, try_launch};

const TITLE: &str = "Text areas";
const BLURB: &str = "Multi-line editing, Unicode cursor motion, horizontal and vertical scrolling";
const SPLITS_TITLE: &str = "Splits & stacks";
const LINE: &str = "4. Run the integration suite and attach the report.";
const HEAVY: &str = "┃";
const BAR: &str = "▎";
const RULE: &str = "│";
const GUTTER: &str = "|";
const THUMB: &str = "#";
const TRACK: &str = ":";
const ELLIPSIS: &str = "~";
const MARK: &str = "›";

const WIDE_ID: &str = "pre_refactor/x03_glyphs/x03_glyphs_glyphs_120x40";
const NARROW_ID: &str = "pre_refactor/x03_glyphs/x03_glyphs_glyphs_narrow";

fn rgb(r: u8, g: u8, b: u8) -> tui_test::SnapshotColor {
    tui_test::SnapshotColor::Rgb(r, g, b)
}

fn green() -> tui_test::SnapshotColor {
    rgb(0x48, 0xe0, 0x54)
}

fn white30() -> tui_test::SnapshotColor {
    rgb(0x4d, 0x4d, 0x4d)
}

fn white50() -> tui_test::SnapshotColor {
    rgb(0x80, 0x80, 0x80)
}

fn surface() -> tui_test::SnapshotColor {
    rgb(0x11, 0x11, 0x11)
}

fn field() -> tui_test::SnapshotColor {
    rgb(0x1e, 0x1e, 0x22)
}

fn nav() -> tui_test::SnapshotColor {
    rgb(0x17, 0x3f, 0x1c)
}

fn canvas() -> tui_test::SnapshotColor {
    tui_test::SnapshotColor::Rgb(0, 0, 0)
}

fn production() -> tui_test::SnapshotColor {
    rgb(0xff, 0x6b, 0x6b)
}

fn open(name: &str, bin: &str, args: &[&str]) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = args.iter().copied().map(str::to_string).collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn kill(app: &App) {
    assert!(app.child_running(), "child exited before the assertion");
    app.kill_child();
    assert!(!app.child_running(), "child still running");
}

fn count(screen: &TerminalSnapshot, glyph: &str) -> usize {
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

fn assert_absent(screen: &TerminalSnapshot, glyphs: &[&str]) {
    for glyph in glyphs {
        let found = count(screen, glyph);
        assert_eq!(found, 0, "painted {glyph} {found} times\n{}", text(screen));
    }
}

fn assert_ink(
    screen: &TerminalSnapshot,
    x: u16,
    y: u16,
    glyph: &str,
    fg: tui_test::SnapshotColor,
    bg: tui_test::SnapshotColor,
) {
    let cell = screen.cell_at(u32::from(x), u32::from(y)).expect("cell");
    assert_eq!(
        (cell.text(), cell.fg(), cell.bg()),
        (glyph, fg, bg),
        "cell {x},{y} is {:?} fg {:?} bg {:?}\n{}",
        cell.text(),
        cell.fg(),
        cell.bg(),
        text(screen)
    );
}

fn assert_column(
    screen: &TerminalSnapshot,
    x: u16,
    top: u16,
    bottom: u16,
    glyph: &str,
    fg: tui_test::SnapshotColor,
    bg: tui_test::SnapshotColor,
) {
    for y in top..=bottom {
        assert_ink(screen, x, y, glyph, fg, bg);
    }
}

fn assert_row(
    screen: &TerminalSnapshot,
    y: u16,
    left: u16,
    right: u16,
    glyph: &str,
    fg: tui_test::SnapshotColor,
    bg: tui_test::SnapshotColor,
) {
    for x in left..=right {
        assert_ink(screen, x, y, glyph, fg, bg);
    }
}

fn row_containing(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle}\n{}", text(screen)))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

/// The fixture's wide text, emoji, and combining sequence sit past the
/// textarea viewport. Their absence is the clip, not ASCII stripping.
fn assert_document_clipped(screen: &TerminalSnapshot) {
    require(screen, LINE);
    let visible = text(screen);
    assert!(
        !visible.contains("日本"),
        "wide text is inside the viewport\n{visible}"
    );
    assert!(
        !visible.contains('👩'),
        "emoji is inside the viewport\n{visible}"
    );
    assert!(
        !visible.contains('\u{0301}'),
        "combining sequence is inside the viewport\n{visible}"
    );
    assert!(
        !visible.contains("cafe"),
        "combining word is inside the viewport\n{visible}"
    );
}

fn assert_ascii_forbidden(screen: &TerminalSnapshot) {
    assert_absent(screen, &[HEAVY, BAR]);
}

fn wide_ascii(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120
        && screen.rows() == 40
        && text(screen).contains(TITLE)
        && text(screen).contains(BLURB)
        && text(screen).contains("Task description")
        && text(screen).contains(LINE)
        && count(screen, HEAVY) == 0
        && count(screen, BAR) == 0
        && count(screen, GUTTER) == 1
        && count(screen, THUMB) == 2
}

fn narrow_ascii(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 80
        && screen.rows() == 24
        && text(screen).contains(TITLE)
        && text(screen).contains("Multi-line")
        && text(screen).contains(LINE)
        && !text(screen).contains("120×40")
        && count(screen, HEAVY) == 0
        && count(screen, BAR) == 0
        && count(screen, GUTTER) == 1
        && count(screen, THUMB) == 57
}

fn assert_wide_ascii(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    require(screen, TITLE);
    require(screen, BLURB);
    assert_ascii_forbidden(screen);
    assert_eq!(count(screen, RULE), 0, "unicode divider\n{}", text(screen));
    assert_eq!(count(screen, GUTTER), 1, "gutter count\n{}", text(screen));
    assert_eq!(count(screen, THUMB), 2, "thumb count\n{}", text(screen));
    assert_ink(screen, 1, 9, GUTTER, green(), nav());
    assert_ink(screen, 4, 9, MARK, green(), nav());
    assert_column(screen, 85, 10, 11, THUMB, white50(), field());
    assert_column(screen, 85, 12, 17, TRACK, rgb(0x47, 0x47, 0x47), field());
    assert_ink(screen, 83, 13, ELLIPSIS, rgb(0x8a, 0x8a, 0x8a), field());
    assert_ink(screen, 83, 17, ELLIPSIS, rgb(0x59, 0x59, 0x5b), field());
    let line = row_containing(screen, LINE);
    assert!(line.contains(ELLIPSIS), "line ellipsis is not ~: {line}");
    assert!(!line.contains('…'), "line still uses …: {line}");
    assert_document_clipped(screen);
}

fn assert_narrow_ascii(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (80, 24));
    require(screen, TITLE);
    require(screen, "Multi-line");
    assert!(
        !text(screen).contains("120×40"),
        "narrow frame still says 120×40\n{}",
        text(screen)
    );
    assert_ascii_forbidden(screen);
    assert_eq!(count(screen, RULE), 0, "unicode divider\n{}", text(screen));
    assert_eq!(count(screen, GUTTER), 1, "gutter count\n{}", text(screen));
    assert_ink(screen, 1, 9, GUTTER, green(), nav());
    assert_ink(screen, 4, 9, MARK, green(), nav());
    assert_column(screen, 79, 7, 10, THUMB, white50(), canvas());
    assert_row(screen, 21, 21, 73, THUMB, white50(), canvas());
    assert_eq!(count(screen, THUMB), 57, "thumb count\n{}", text(screen));
    assert_document_clipped(screen);
    let line = row_containing(screen, LINE);
    assert!(
        !line.contains("日本") && !line.contains('👩'),
        "narrow line shows the fixture tail: {line}"
    );
}

fn assert_unicode_textareas(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    require(screen, TITLE);
    require(screen, BLURB);
    assert_eq!(count(screen, BAR), 1, "unicode gutter\n{}", text(screen));
    assert_eq!(
        count(screen, THUMB),
        0,
        "ascii thumb on unicode\n{}",
        text(screen)
    );
    assert_ink(screen, 1, 9, BAR, green(), nav());
    assert_ink(screen, 4, 9, MARK, green(), nav());
    assert_column(screen, 85, 10, 11, HEAVY, white50(), field());
    assert_column(screen, 85, 12, 17, "┆", rgb(0x47, 0x47, 0x47), field());
    assert_ink(screen, 83, 13, "…", rgb(0x8a, 0x8a, 0x8a), field());
    assert_document_clipped(screen);
}

fn seam_ready(screen: &TerminalSnapshot, glyph: &str) -> bool {
    text(screen).contains("splitter.rs")
        && text(screen).contains(SPLITS_TITLE)
        && (11u16..=26).all(|y| screen.cell_at(54, u32::from(y)).expect("cell").text() == glyph)
}

fn assert_seam(screen: &TerminalSnapshot, glyph: &str) {
    require(screen, "splitter.rs");
    require(screen, SPLITS_TITLE);
    assert_column(screen, 54, 11, 26, glyph, white30(), surface());
    for y in 11u16..=26 {
        let text = screen.cell_at(54, u32::from(y)).expect("cell").text();
        assert_ne!(text, HEAVY, "seam painted {HEAVY}");
        if glyph == GUTTER {
            assert_ne!(text, RULE, "seam painted {RULE}");
        }
    }
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
        "x03-glyphs-route",
        SHOWCASE,
        &["--page", "textareas", "--glyphs", "unicode"],
    );
    let page = app.wait_text(BLURB);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    require(&page, TITLE);
    let header = row_containing(&page, BLURB);
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
        "the text areas page exited before Ctrl+Q"
    );
    app.finish();
}

fn unicode_textareas() {
    let app = open(
        "x03-glyphs-unicode",
        SHOWCASE,
        &["--page", "textareas", "--glyphs", "unicode"],
    );
    let page = app.wait_screen("unicode text areas", |screen| {
        text(screen).contains(BLURB)
            && screen.cell_at(1, 9).is_some_and(|cell| cell.text() == BAR)
            && screen
                .cell_at(85, 10)
                .is_some_and(|cell| cell.text() == HEAVY)
    });
    assert_unicode_textareas(&page);
    kill(&app);
}

fn splits(glyphs: &str) {
    let app = open(
        &format!("x03-glyphs-splits-{glyphs}"),
        SHOWCASE,
        &["--page", "splits", "--glyphs", glyphs],
    );
    let rest = if glyphs == "ascii" { GUTTER } else { RULE };
    let page = app.wait_screen(&format!("splits {glyphs}"), |screen| {
        seam_ready(screen, rest)
    });
    assert_seam(&page, rest);
    if glyphs == "ascii" {
        assert_absent(&page, &[HEAVY, BAR, RULE]);
        assert!(
            count(&page, THUMB) > 0,
            "ascii splits painted no thumb\n{}",
            text(&page)
        );
        assert_ink(&page, 1, 14, GUTTER, green(), nav());
    } else {
        assert_eq!(
            count(&page, BAR),
            1,
            "unicode splits gutter\n{}",
            text(&page)
        );
        assert_ink(&page, 1, 14, BAR, green(), nav());
        assert!(
            count(&page, HEAVY) > 0,
            "unicode splits painted no thumb\n{}",
            text(&page)
        );
    }
    kill(&app);
}

fn tablepro() {
    let app = open("x03-glyphs-tablepro", TABLEPRO, &["--glyphs", "ascii"]);
    let page = app.wait_screen("tablepro ascii", |screen| {
        text(screen).contains("Filter connections")
            && count(screen, HEAVY) == 0
            && count(screen, BAR) == 0
            && screen
                .cell_at(3, 5)
                .is_some_and(|cell| cell.text() == GUTTER)
    });
    require(&page, "Filter connections");
    require(&page, "Analytics");
    require(&page, "Production");
    assert_ascii_forbidden(&page);
    assert_eq!(count(&page, RULE), 0, "unicode rule\n{}", text(&page));
    assert_ink(&page, 3, 5, GUTTER, green(), canvas());
    assert_ink(&page, 8, 11, THUMB, production(), canvas());
    assert_ink(&page, 8, 12, THUMB, production(), canvas());
    assert_eq!(count(&page, THUMB), 2, "unexpected thumb\n{}", text(&page));
    assert_eq!(count(&page, GUTTER), 69, "border pipes\n{}", text(&page));
    kill(&app);
}

fn holla() {
    let app = open(
        "x03-glyphs-holla",
        HOLLA,
        &["--scenario", "activities-multi", "--glyphs", "ascii"],
    );
    let page = app.wait_screen("holla ascii", |screen| {
        text(screen).contains("Search actions")
            && count(screen, HEAVY) == 0
            && count(screen, BAR) == 0
            && screen
                .cell_at(1, 4)
                .is_some_and(|cell| cell.text() == GUTTER)
    });
    require(&page, "Search actions");
    assert_ascii_forbidden(&page);
    assert_eq!(
        count(&page, THUMB),
        0,
        "holla painted a thumb\n{}",
        text(&page)
    );
    assert_ink(&page, 1, 4, GUTTER, green(), field());
    assert_eq!(count(&page, GUTTER), 5, "holla pipes\n{}", text(&page));
    kill(&app);
}

fn checkpoint() {
    let app = open(
        "x03-glyphs-checkpoint",
        SHOWCASE,
        &["--page", "textareas", "--glyphs", "ascii"],
    );
    let wide = app.wait_screen("ascii text areas 120x40", wide_ascii);
    assert_wide_ascii(&wide);
    app.resize(80, 24);
    let narrow = app.wait_screen("ascii text areas 80x24", narrow_ascii);
    assert_narrow_ascii(&narrow);
    app.resize(120, 40);
    let _back = app.wait_screen("ascii text areas back at 120x40", wide_ascii);
    assert!(app.child_running(), "text areas exited before Ctrl+Q");
    app.finish();
    check_stored(&app, &wide, WIDE_ID);
    check_stored(&app, &narrow, NARROW_ID);
}

#[test]
fn x03_glyphs_route() {
    route();
}

#[test]
fn x03_glyphs_glyphs() {
    unicode_textareas();
    splits("ascii");
    splits("unicode");
    tablepro();
    holla();
    checkpoint();
}
