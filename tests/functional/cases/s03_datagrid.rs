//! Data grid route, sections, specimens, compare levels, pending edits, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s03_datagrid/` and each case id's dots become underscores.

use std::path::PathBuf;

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{Launch, Point, SHOWCASE, cell_bg, find_all, row_text, text, try_launch};

const TITLE: &str = "Tables › Data grid";
const BLURB: &str = "Typed cells, a pending-change queue, paging and local sort";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LOADED: &str = "rows 1–21 of 40 loaded · ~4,812 total";
const QUEUED: &str = "Row queued for deletion";
const DELETED: &str = "−";

const TOP: &[(&str, &str)] = &[
    ("sc.datagrid.specimen.rest", "rest"),
    ("sc.datagrid.specimen.hover", "hover"),
    ("sc.datagrid.specimen.pressed", "pressed"),
    ("sc.datagrid.specimen.focused", "focused"),
    ("sc.datagrid.specimen.focused_hover", "focus + hover"),
];

const MID: &[(&str, &str)] = &[
    ("sc.datagrid.specimen.selected", "selected"),
    ("sc.datagrid.specimen.editing", "editing"),
    ("sc.datagrid.specimen.error", "error"),
    ("sc.datagrid.specimen.empty", "empty"),
    ("sc.datagrid.specimen.loading", "loading"),
];

const BOTTOM: &[(&str, &str)] = &[
    ("sc.datagrid.specimen.overflow", "overflow"),
    ("sc.datagrid.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s03_datagrid/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
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

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
}

fn line_after(screen: &TerminalSnapshot, label: &str) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    lines
        .get(index + 1)
        .unwrap_or_else(|| panic!("no value under {label:?}\n{}", text(screen)))
        .clone()
}

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str, absent: &[&str]) {
    let line = line_after(screen, label);
    assert!(
        line.contains(value),
        "{label} shows {line:?}, expected {value}"
    );
    for item in absent {
        assert!(
            !line.contains(item),
            "{label} line {line:?} still has {item}"
        );
    }
}

fn click_first(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    hits.sort_by_key(|hit| (hit.y, hit.x));
    assert!(!hits.is_empty(), "missing {label}\n{}", text(&screen));
    app.click(hits[0]);
}

fn click_tab(app: &crate::support::App, label: &str) {
    click_first(app, label, |hit| hit.y == 4);
}

fn click_side(app: &crate::support::App, label: &str) {
    click_first(app, label, |hit| hit.x < 24);
}

fn click_grid(app: &crate::support::App) {
    click_first(app, "Northwind", |hit| hit.x > 24 && hit.y < 20);
}

fn show_options(app: &crate::support::App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options")
}

fn page_until(app: &crate::support::App, needle: &str) {
    for _ in 0..12 {
        if text(&app.screen()).contains(needle) {
            return;
        }
        app.key("pagedown");
        app.settle();
    }
    app.wait_text(needle);
}

fn page_up_until(app: &crate::support::App, needle: &str) {
    for _ in 0..12 {
        if text(&app.screen()).contains(needle) {
            return;
        }
        app.key("pageup");
        app.settle();
    }
    app.wait_text(needle);
}

fn opt_args(pair: &str) -> [&str; 4] {
    ["--page", "datagrid", "--opt", pair]
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn check_frame(app: &crate::support::App, screen: &TerminalSnapshot, test_id: &str) {
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
        let fields = failure.changed_fields();
        let parked = failure.changed_cell_total() == 0
            && !screen.cursor().visible()
            && !fields.is_empty()
            && fields
                .iter()
                .all(|field| field == "cursor.col" || field == "cursor.row");
        if parked {
            return;
        }
        panic!(
            "{test_id} {} sha {} path {} cells {} fields {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total(),
            fields.join(",")
        );
    }
}

/// The accepted Options page still has the tab pressed, gutter included.
/// `visual` returns after that fill drops back to the overlay rest colour.
fn shot_options_held(app: &crate::support::App, case_id: &str) {
    let screen = app.screen();
    let tab = find_all(&screen, "Options")
        .into_iter()
        .find(|hit| hit.y == 4)
        .unwrap_or_else(|| panic!("Options tab\n{}", text(&screen)));
    app.down(tab);
    let held = app.wait_screen("options tab held", |screen| {
        let Some(hit) = find_all(screen, "Options")
            .into_iter()
            .find(|hit| hit.y == 4)
        else {
            return false;
        };
        cell_bg(screen, hit) == SnapshotColor::Rgb(0x4a, 0x4a, 0x52)
            && row_text(screen, 4).contains("▎Options")
            && text(screen).contains("cancel")
    });
    check_frame(app, &held, &visual_id(case_id));
    app.up(tab);
}

fn see_loaded(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            TITLE,
            CAP_PLAY,
            WIDE,
            "Northwind Traders",
            "1 1001",
            "3›",
            LOADED,
            "no pending changes",
        ],
        &[
            "Page options",
            "Compare levels",
            "read-only",
            "Could not load",
            "Review SQL",
            "0 rows",
            QUEUED,
        ],
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "datagrid"];
    let app = open("s03-route-page", 120, 40, &args);
    let screen = app.wait_text("Northwind");
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "customers",
            LOADED,
            "1 1001",
            "Northwind Traders",
            "3›",
            "no pending changes",
            "↑↓ Move",
        ],
        &[
            "Page options",
            "Compare levels",
            CAP_OPT,
            CAP_STATES,
            OVERVIEW,
            "Could not load",
            "←→ Section",
            "0 rows",
        ],
    );
    shot(
        &app,
        "sc.datagrid.route",
        "--page datagrid opens the playground with the sidebar on Data grid",
        &args,
    );
    app.finish();

    let app = open("s03-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    for _ in 0..16 {
        app.key("]");
    }
    // The destination blurb paints before the previous page's grid leaves.
    let entered = app.wait_screen("datagrid from overview", |screen| {
        let visible = text(screen);
        visible.contains(TITLE)
            && visible.contains(BLURB)
            && visible.contains("Northwind Traders")
            && visible.contains(CAP_PLAY)
            && !visible.contains(OVERVIEW)
            && !visible.contains("Page options")
            && !visible.contains("Editable tables")
    });
    see(
        &entered,
        &[TITLE, BLURB, "Northwind Traders", CAP_PLAY],
        &[OVERVIEW, "Page options", "Editable tables"],
    );
    app.key("[");
    let left = app.wait_screen("editable after datagrid", |screen| {
        let visible = text(screen);
        visible.contains("Editable tables")
            && visible.contains("Navigation is reversed cell")
            && !visible.contains(BLURB)
            && !visible.contains("no pending changes")
    });
    see(
        &left,
        &["Editable tables", "Navigation is reversed cell"],
        &[BLURB, "no pending changes"],
    );
    app.key("]");
    let again = app.wait_screen("datagrid again", |screen| {
        let visible = text(screen);
        visible.contains(TITLE)
            && visible.contains(BLURB)
            && visible.contains("Northwind Traders")
            && !visible.contains("Editable tables")
            && !visible.contains(OVERVIEW)
    });
    see(
        &again,
        &[TITLE, BLURB, "Northwind Traders"],
        &["Editable tables", OVERVIEW],
    );
    app.finish();

    let app = open("s03-route-pointer", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    click_side(&app, "Tables");
    app.wait_text("Data grid");
    click_side(&app, "Data grid");
    let pointed = app.wait_text(BLURB);
    see(
        &pointed,
        &[TITLE, BLURB, "Northwind Traders", "customers", CAP_PLAY],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            "Editable tables",
        ],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "datagrid"];
    let app = open("s03-sections", 80, 24, &args);
    let narrow_play = app.wait_text("Northwind");
    see(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "Typed cells",
            "customers",
            "Northwind Traders",
            "1001",
            "↑↓ Move",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "local sort",
            "Could not load",
        ],
    );
    shot(
        &app,
        "sc.datagrid.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "editable",
            "loaded",
            "estimated",
            "Row numbers on",
            "Overflow badges on",
            "default",
            "focused",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Northwind",
            WIDE,
            "query",
            "Could not load",
            "truecolor",
        ],
    );
    assert!(line_after(&narrow_opt, "Access").contains("editable"));
    assert!(line_after(&narrow_opt, "Rows").contains("loaded"));
    shot(
        &app,
        "sc.datagrid.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "Northwind Traders",
            "←→ Section",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "pressed",
            "selected",
            "editing",
            "overflow",
            "narrow",
            "disabled",
            "busy",
            WIDE,
            "truecolor",
        ],
    );
    shot(
        &app,
        "sc.datagrid.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    // The states caption paints before the specimen rows. "hover" is one of
    // those later rows.
    let wide_states = app.wait_screen("wide states", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(CAP_STATES)
            && visible.contains("hover")
            && visible.contains("pressed")
            && visible.contains("focused")
            && visible.contains("focus + hover")
    });
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "▏ depicts a focused sample; it is not where your focus is",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "Northwind Traders",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "selected",
            "editing",
            "overflow",
            "narrow",
            "disabled",
            "busy",
            "256 colors",
        ],
    );
    shot(
        &app,
        "sc.datagrid.states.120x40",
        "states caption and the top grid rows at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.datagrid.states",
        "states is the visible Data grid section",
        &args,
    );
    for (case_id, label) in TOP {
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES, "rest"],
            &[
                "● Compare levels",
                "Page options",
                "overflow",
                "narrow",
                "disabled",
                "busy",
            ],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &args,
        );
    }

    page_until(&app, "selected");
    let middle = app.wait_text("editing");
    see(
        &middle,
        &[
            "selected", "editing", "error", "empty", "loading", "No rows", TITLE,
        ],
        &[
            "rest",
            "pressed",
            "overflow",
            "narrow",
            "○ Compare levels",
            "● Compare levels",
            "disabled",
            "busy",
            "Page options",
        ],
    );
    for (case_id, label) in MID {
        see(
            &app.screen(),
            &[label, "selected", "No rows"],
            &[
                "overflow",
                "narrow",
                "rest",
                "disabled",
                "busy",
                "● Compare levels",
            ],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible below the first screen",
            &args,
        );
    }

    page_until(&app, "narrow");
    let bottom = app.wait_text("overflow");
    see(
        &bottom,
        &[
            "overflow",
            "narrow",
            "and a value",
            "No rows",
            "Northwin",
            TITLE,
        ],
        &[
            "rest",
            "selected",
            "editing",
            "Compare levels",
            "disabled",
            "busy",
            "Page options",
        ],
    );
    for (case_id, label) in BOTTOM {
        see(
            &app.screen(),
            &[label, "and a value", "narrow", "overflow"],
            &[
                "rest",
                "selected",
                "editing",
                "Compare levels",
                "disabled",
                "busy",
            ],
        );
        shot(
            &app,
            case_id,
            "overflow and narrow rows are on screen",
            &args,
        );
    }

    click_tab(&app, "Options");
    // The options caption paints before Horizontal bar's "auto", while the
    // states grid can still cover that row.
    let wide_opt = app.wait_screen("wide options", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(CAP_OPT)
            && visible.contains("auto")
            && visible.contains("Page options")
            && visible.contains("shell")
            && visible.contains("ctrl-d")
            && !visible.contains("Northwind")
            && !visible.contains("Compare levels")
            && !visible.contains(CAP_STATES)
            && !visible.contains("never")
    });
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "Page options",
            "editable",
            "loaded",
            "estimated",
            "Row numbers on",
            "Overflow badges on",
            "ctrl-d",
            "Vim keys off",
            "200",
            "shell",
            "auto",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Northwind",
            "query",
            "shift-d",
            "customer",
            "never",
            "jump",
            "strict",
            "wide",
            "Could not load",
        ],
    );
    assert_select(&wide_opt, "Access", "editable", &["query", "view"]);
    assert_select(&wide_opt, "Rows", "loaded", &["empty", "error"]);
    assert_select(&wide_opt, "Row total", "estimated", &["exact", "unknown"]);
    assert_select(&wide_opt, "Type-ahead", "off", &["any", "customer"]);
    assert_select(&wide_opt, "Key map", "default", &["custom"]);
    assert_select(&wide_opt, "Column seams", "on", &["off", "wide"]);
    shot(
        &app,
        "sc.datagrid.options.120x40",
        "options caption and the default values at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.datagrid.options",
        "options is the visible Data grid section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    see_loaded(&wide_play);
    see(
        &wide_play,
        &[BLURB, "←→ Section", "Change marks"],
        &["↑↓ Move", "Tailspin Toys 2"],
    );
    shot(
        &app,
        "sc.datagrid.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.datagrid.playground",
        "playground is the visible Data grid section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "datagrid"];
    let app = open("s03-compare", 120, 40, &args);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", CAP_STATES, WIDE],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_first(&app, "Compare levels", |hit| hit.y < 12 && hit.x > 24);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    for _ in 0..12 {
        let visible = text(&app.screen());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.key("pagedown");
        app.settle();
    }
    let bands = app.wait_text("no color");
    assert!(
        saw_256 && saw_16,
        "compare bands did not pass 256 and 16\n{}",
        text(&bands)
    );
    see(
        &bands,
        &[TITLE, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.datagrid.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    // The title is in the header on the first narrow frame. The compare
    // bands paint after it.
    let narrow = app.wait_screen("narrow compare", |screen| {
        let visible = text(screen);
        let band = visible.contains("truecolor")
            || visible.contains("256 colors")
            || visible.contains("16 colors")
            || visible.contains("no color");
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(TITLE)
            && !visible.contains(WIDE)
            && band
    });
    let visible = text(&narrow);
    let band = visible.contains("truecolor")
        || visible.contains("256 colors")
        || visible.contains("16 colors")
        || visible.contains("no color");
    assert!(
        band && !visible.contains(WIDE),
        "narrow compare bands missing\n{visible}"
    );
    see(
        &narrow,
        &[TITLE],
        &["Page options", CAP_PLAY, WIDE, "120×40"],
    );
    shot(
        &app,
        "sc.datagrid.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    page_up_until(&app, "Compare levels");
    click_first(&app, "Compare levels", |hit| hit.x > 20);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("editable");
    see(
        &options,
        &[TITLE, "editable", "loaded", "estimated", "Row numbers on"],
        &["query", "Could not load", "● Compare levels", "unknown"],
    );
    assert_select(&options, "Access", "editable", &["query"]);
    assert_select(&options, "Rows", "loaded", &["error"]);
    app.finish();
}

#[test]
fn pending_narrow_then_wide() {
    let args = ["--page", "datagrid"];
    let app = open("s03-pending", 80, 24, &args);
    app.wait_text("Northwind");
    click_grid(&app);
    app.key("-");
    let queued = app.wait_text(QUEUED);
    let row = row_with(&queued, "1001");
    assert!(
        row.contains(DELETED) && row.contains("Northwind"),
        "delete mark missing: {row}"
    );
    see(
        &queued,
        &[TITLE, QUEUED, "Northwind Traders", "u undoes"],
        &[
            "no pending changes",
            "Changes discarded",
            "Saved",
            "Nothing to save",
            "Review SQL",
            WIDE,
            "truecolor",
            "local sort",
            "Page options",
        ],
    );
    shot(
        &app,
        "sc.datagrid.pending.80x24",
        "a delete is queued at 80 by 24 before the terminal grows",
        &args,
    );
    app.resize(120, 40);
    let wide = app.wait_text(WIDE);
    let row = row_with(&wide, "1001");
    assert!(
        row.contains(DELETED) && row.contains("Northwind"),
        "delete mark lost after grow: {row}"
    );
    see(
        &wide,
        &[TITLE, BLURB, CAP_PLAY, WIDE, QUEUED, "Northwind Traders"],
        &[
            "no pending changes",
            "Changes discarded",
            "Saved",
            "Nothing to save",
            "Review SQL",
            "Page options",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.datagrid.pending.120x40",
        "the same queued delete is still uncommitted at 120 by 40",
        &args,
    );
    app.key("U");
    let cleared = app.wait_text("no pending changes");
    see(
        &cleared,
        &["no pending changes", "Northwind Traders", TITLE],
        &[QUEUED, "Saved", "Review SQL"],
    );
    let row = row_with(&cleared, "1 1001");
    assert!(
        !row.contains(DELETED),
        "discard left the delete mark: {row}"
    );
    app.finish();
}

#[test]
fn option_access_query() {
    let args = opt_args("datagrid.access=query");
    let app = open("s03-access", 120, 40, &args);
    let play = app.wait_text("read-only");
    see(
        &play,
        &[
            TITLE,
            "read-only",
            "Northwind Traders",
            "1 1001",
            "3›",
            "~4,812",
            CAP_PLAY,
        ],
        &["Could not load", "0 rows", "Page options", QUEUED],
    );
    click_grid(&app);
    app.key("-");
    let refused = app.settle();
    let row = row_with(&refused, "1 1001");
    assert!(
        !row.contains(DELETED),
        "read-only grid accepted a delete: {row}"
    );
    assert!(
        !text(&refused).contains(QUEUED),
        "read-only grid queued a delete\n{}",
        text(&refused)
    );
    let options = show_options(&app);
    assert_select(&options, "Access", "query", &["editable", "view", "no-key"]);
    assert_select(&options, "Rows", "loaded", &["empty", "error"]);
    assert!(row_with(&options, "Row numbers").contains("Row numbers on"));
    click_tab(&app, "Playground");
    let again = app.wait_text("read-only");
    see(
        &again,
        &["read-only", "Northwind Traders", "1 1001", "~4,812"],
        &[
            QUEUED,
            "Could not load",
            "Page options",
            "Changes discarded",
        ],
    );
    shot(
        &app,
        "opt.datagrid.access",
        "query access reads as read-only and a delete key does not queue a row",
        &args,
    );
    app.finish();
}

#[test]
fn option_rows_error() {
    let args = opt_args("datagrid.rows=error");
    let app = open("s03-rows", 120, 40, &args);
    let play = app.wait_text("Could not load customers");
    see(
        &play,
        &[
            TITLE,
            "Could not load customers",
            "connection reset by peer",
            "0 rows",
            CAP_PLAY,
        ],
        &["Northwind", "~4,812", "1 1001", "Page options"],
    );
    let options = show_options(&app);
    assert_select(&options, "Rows", "error", &["loaded", "empty"]);
    assert_select(&options, "Access", "editable", &["query"]);
    click_tab(&app, "Playground");
    app.wait_text("Could not load customers");
    shot(
        &app,
        "opt.datagrid.rows",
        "rows error shows Could not load customers and access stays editable",
        &args,
    );
    app.finish();
}

#[test]
fn option_total_unknown() {
    let args = opt_args("datagrid.total=unknown");
    let app = open("s03-total", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see(
        &play,
        &[
            TITLE,
            "rows 1–21 of 40 · cols 1–6 of 9",
            "Northwind Traders",
            "1 1001",
            "3›",
            CAP_PLAY,
        ],
        &["~4,812", "loaded ·", "Could not load", "Page options"],
    );
    let options = show_options(&app);
    assert_select(&options, "Row total", "unknown", &["estimated", "exact"]);
    assert_select(&options, "Rows", "loaded", &["error"]);
    click_tab(&app, "Playground");
    app.wait_text("rows 1–21 of 40 · cols");
    shot(
        &app,
        "opt.datagrid.total",
        "an unknown row total drops the estimated count and rows stay loaded",
        &args,
    );
    app.finish();
}

#[test]
fn option_row_numbers_off() {
    let args = opt_args("datagrid.row_numbers=off");
    let app = open("s03-numbers", 120, 40, &args);
    let play = app.wait_text("Northwind");
    let row = row_with(&play, "Northwind");
    assert!(
        row.contains("1001") && !row.contains("1 1001"),
        "row number gutter stayed: {row}"
    );
    see(
        &play,
        &[TITLE, "3›", "~4,812", "Northwind Traders", CAP_PLAY],
        &["Could not load", "Page options", "read-only"],
    );
    let options = show_options(&app);
    let numbers = row_with(&options, "Row numbers");
    assert!(
        numbers.contains("Row numbers off") && !numbers.contains("Row numbers on"),
        "{numbers}"
    );
    assert!(row_with(&options, "Overflow badges").contains("Overflow badges on"));
    click_tab(&app, "Playground");
    app.wait_text("Northwind");
    shot(
        &app,
        "opt.datagrid.row_numbers",
        "row numbers off drops the gutter and overflow badges stay on",
        &args,
    );
    app.finish();
}

#[test]
fn option_overflow_badges_off() {
    let args = opt_args("datagrid.overflow_badges=off");
    let app = open("s03-badges", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see(
        &play,
        &[TITLE, "1 1001", "Northwind Traders", "~4,812", CAP_PLAY],
        &["3›", "Could not load", "Page options"],
    );
    let options = show_options(&app);
    let badges = row_with(&options, "Overflow badges");
    assert!(
        badges.contains("Overflow badges off") && !badges.contains("Overflow badges on"),
        "{badges}"
    );
    assert!(row_with(&options, "Row numbers").contains("Row numbers on"));
    click_tab(&app, "Playground");
    app.wait_text("1 1001");
    shot(
        &app,
        "opt.datagrid.overflow_badges",
        "overflow badges off hides 3› and row numbers stay on",
        &args,
    );
    app.finish();
}

#[test]
fn option_labels_custom() {
    let args = opt_args("datagrid.labels=custom");
    let app = open("s03-labels", 120, 40, &args);
    let options = show_options(&app);
    assert_select(&options, "Bar and menu labels", "custom", &["default"]);
    assert_select(&options, "Type-ahead", "off", &["customer", "any"]);
    click_tab(&app, "Playground");
    app.wait_text("Northwind");
    click_grid(&app);
    app.key("-");
    let queued = app.wait_text("Review SQL");
    see(
        &queued,
        &[TITLE, "Review SQL", "Revert", QUEUED, "Northwind Traders"],
        &[
            "Preview SQL",
            "no pending changes",
            "Changes discarded",
            "Saved",
        ],
    );
    shot(
        &app,
        "opt.datagrid.labels",
        "custom bar labels read Review SQL and Revert while a delete is queued",
        &args,
    );
    app.key("U");
    app.wait_text("no pending changes");
    app.finish();
}

#[test]
fn option_type_ahead_customer() {
    let args = opt_args("datagrid.type_ahead=customer");
    let app = open("s03-type", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "Type-ahead", "customer", &["off", "any"]);
    assert_select(&options, "Rows", "loaded", &["error"]);
    see(
        &options,
        &[CAP_OPT, "customer", "Row numbers on"],
        &[CAP_PLAY, "Compare levels", "any"],
    );
    shot(
        &app,
        "opt.datagrid.type_ahead",
        "type-ahead customer is selected and rows stay loaded",
        &args,
    );
    app.finish();
}

#[test]
fn option_affordance_never() {
    let args = opt_args("datagrid.affordance=never");
    let app = open("s03-affordance", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "▾ affordance", "never", &["focused", "always"]);
    assert_select(&options, "Type-ahead", "off", &["customer"]);
    see(
        &options,
        &[CAP_OPT, "never", "editable"],
        &[CAP_PLAY, "Compare levels", "always"],
    );
    shot(
        &app,
        "opt.datagrid.affordance",
        "the editor affordance is never and type-ahead stays off",
        &args,
    );
    app.finish();
}

#[test]
fn option_keymap_custom() {
    let args = opt_args("datagrid.keymap=custom");
    let app = open("s03-keymap", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "Key map", "custom", &["default"]);
    assert_select(&options, "Duplicate row keys", "ctrl-d", &["shift-d"]);
    see(
        &options,
        &[CAP_OPT, "custom", "ctrl-d"],
        &[CAP_PLAY, "Compare levels", "shift-d"],
    );
    shot(
        &app,
        "opt.datagrid.keymap",
        "the key map is custom and duplicate row keys stay ctrl-d",
        &args,
    );
    app.finish();
}

#[test]
fn option_duplicate_shift_d() {
    let args = opt_args("datagrid.duplicate_keys=shift-d");
    let app = open("s03-duplicate", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "Duplicate row keys", "shift-d", &["ctrl-d"]);
    assert_select(&options, "Key map", "default", &["custom"]);
    see(
        &options,
        &[CAP_OPT, "shift-d", "default"],
        &[CAP_PLAY, "Compare levels", "ctrl-d"],
    );
    shot(
        &app,
        "opt.datagrid.duplicate_keys",
        "duplicate row keys are shift-d and the key map stays default",
        &args,
    );
    app.finish();
}

#[test]
fn option_nav_on() {
    let args = opt_args("datagrid.nav=on");
    let app = open("s03-nav", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    let vim = row_with(&options, "Vim keys");
    assert!(
        vim.contains("Vim keys on") && !vim.contains("Vim keys off"),
        "{vim}"
    );
    assert!(row_with(&options, "Row numbers").contains("Row numbers on"));
    see(
        &options,
        &[CAP_OPT, "Vim keys on", "editable"],
        &[CAP_PLAY, "Compare levels", "Vim keys off"],
    );
    shot(
        &app,
        "opt.datagrid.nav",
        "vim keys are on and row numbers stay on",
        &args,
    );
    app.finish();
}

#[test]
fn option_width_sample_eight() {
    let args = opt_args("datagrid.width_sample_rows=8");
    let app = open("s03-width", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see(
        &play,
        &[
            TITLE,
            "Northwind Traders",
            "enterprise",
            "1 1001",
            "~4,812",
            CAP_PLAY,
        ],
        &["Could not load", "Page options", "read-only"],
    );
    let options = show_options(&app);
    assert_select(&options, "Width sample", "8", &["200"]);
    assert_select(&options, "Column seams", "on", &["off", "wide"]);
    see(
        &options,
        &[CAP_OPT, "8"],
        &[CAP_PLAY, "Compare levels", "200"],
    );
    shot(
        &app,
        "opt.datagrid.width_sample_rows",
        "the width sample is 8 rows and column seams stay on",
        &args,
    );
    app.finish();
}

#[test]
fn option_seams_wide() {
    let args = opt_args("datagrid.seams=wide");
    let app = open("s03-seams", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "Column seams", "wide", &["off"]);
    assert_select(&options, "Horizontal bar", "auto", &["never"]);
    see(
        &options,
        &[CAP_OPT, "wide", "auto"],
        &[CAP_PLAY, "Compare levels", "never"],
    );
    shot(
        &app,
        "opt.datagrid.seams",
        "column seams are wide and the horizontal bar stays auto",
        &args,
    );
    app.finish();
}

#[test]
fn option_track_click_jump() {
    let args = opt_args("datagrid.track_click=jump");
    let app = open("s03-track", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "Track click", "jump", &["shell", "page"]);
    assert_select(&options, "Horizontal bar", "auto", &["never"]);
    see(
        &options,
        &[CAP_OPT, "jump", "auto"],
        &[CAP_PLAY, "Compare levels"],
    );
    shot(
        &app,
        "opt.datagrid.track_click",
        "track click is jump and the horizontal bar stays auto",
        &args,
    );
    app.finish();
}

#[test]
fn option_horizontal_never() {
    let args = opt_args("datagrid.horizontal=never");
    let app = open("s03-horizontal", 120, 40, &args);
    let play = app.wait_text("Tailspin Toys 2");
    see(
        &play,
        &[
            TITLE,
            "Tailspin Toys 2",
            "1–22 of 40",
            "Northwind Traders",
            CAP_PLAY,
        ],
        &["1–21 of 40", "Could not load", "Page options"],
    );
    let options = show_options(&app);
    assert_select(&options, "Horizontal bar", "never", &["auto"]);
    assert_select(&options, "Track click", "shell", &["jump", "page"]);
    click_tab(&app, "Playground");
    app.wait_text("Tailspin Toys 2");
    shot(
        &app,
        "opt.datagrid.horizontal",
        "the horizontal bar never hides and row 22 Tailspin Toys 2 is in view",
        &args,
    );
    app.finish();
}

#[test]
fn option_validator_strict() {
    let args = opt_args("datagrid.validator=strict");
    let app = open("s03-validator", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    let options = show_options(&app);
    assert_select(&options, "Validator", "strict", &["default"]);
    assert_select(&options, "Horizontal bar", "auto", &["never"]);
    see(
        &options,
        &[CAP_OPT, "strict", "auto"],
        &[CAP_PLAY, "Compare levels", "cancel"],
    );
    shot(
        &app,
        "opt.datagrid.validator",
        "the validator is strict and the horizontal bar stays auto",
        &args,
    );
    app.finish();
}

#[test]
fn option_blur_cancel() {
    let args = opt_args("datagrid.blur_action=cancel");
    let app = open("s03-blur", 120, 40, &args);
    let play = app.wait_text("Northwind");
    see_loaded(&play);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    page_until(&app, "cancel");
    let options = app.wait_text("cancel");
    assert_select(&options, "On blur", "cancel", &["commit"]);
    assert_select(&options, "Validator", "default", &["strict"]);
    see(
        &options,
        &[TITLE, CAP_OPT, "cancel", "default"],
        &[CAP_PLAY, "Compare levels", "strict", "commit"],
    );
    shot_options_held(&app, "opt.datagrid.blur_action");
    app.finish();
}

fn click_grid_row(app: &crate::support::App, label: &str) {
    click_first(app, label, |hit| hit.x > 24 && hit.y < 20);
}

fn strict_grid_at(name: &str, right: usize) -> crate::support::App {
    let args = opt_args("datagrid.validator=editors");
    let app = open(name, 120, 40, &args);
    app.wait_text("Northwind");
    click_grid(&app);
    for _ in 0..right {
        app.key("right");
    }
    app.settle();
    app
}

#[test]
fn issue43_strict_grid_rejects_the_active_toggle() {
    const WHY: &str = "Accounts are deactivated by support";
    let app = strict_grid_at("s03-strict-active", 4);
    app.key("enter");
    let rejected = app.wait_text(WHY);
    let row = row_with(&rejected, "Northwind");
    assert!(
        row.contains("true") && !row.contains("false"),
        "the rejected toggle must keep true: {row}"
    );
    see(&rejected, &["no pending changes"], &["1 pending"]);
    app.key("down");
    let marked = app.settle();
    let row = row_with(&marked, "Northwind");
    assert!(row.contains('!'), "the rejected cell keeps its mark: {row}");
    app.finish();
}

#[test]
fn issue43_strict_grid_rejects_the_empty_tier_choice() {
    const WHY: &str = "A support tier is required: pick one";
    let app = strict_grid_at("s03-strict-tier", 8);
    app.key("enter");
    app.wait_text("(none)");
    app.key("up");
    app.settle();
    app.key("enter");
    let rejected = app.wait_text(WHY);
    see(&rejected, &["(none)", "standard"], &["1 pending"]);
    assert!(
        text(&rejected).contains("dedicated"),
        "the popup closed on the rejected choice\n{}",
        text(&rejected)
    );
    app.key("esc");
    let closed = app.settle();
    see(&closed, &["no pending changes"], &[WHY, "(none)"]);
    app.finish();
}

#[test]
fn issue43_strict_grid_bool_cycle_reaches_null() {
    let app = strict_grid_at("s03-strict-paused", 9);
    let start = app.settle();
    assert!(
        row_with(&start, "▎").contains("true"),
        "paused starts true\n{}",
        text(&start)
    );
    app.key("enter");
    let off = app.wait_text("1 pending");
    assert!(
        row_with(&off, "▎").contains("false"),
        "first toggle sets false\n{}",
        text(&off)
    );
    app.key("enter");
    let null = app.settle();
    assert!(
        row_with(&null, "▎").contains("NULL"),
        "second toggle sets NULL\n{}",
        text(&null)
    );
    app.key("U");
    app.wait_text("no pending changes");
    app.finish();
}

#[test]
fn issue43_save_resolves_the_open_edit_first() {
    const WHY: &str = "A customer name has at least 3 letters";
    let app = strict_grid_at("s03-strict-save", 0);
    app.key("down");
    app.key("-");
    app.wait_text(QUEUED);
    app.key("up");
    app.key("enter");
    for _ in 0.."Northwind Traders".len() {
        app.key("backspace");
    }
    app.key("a");
    app.key("b");
    app.settle();
    click_first(&app, "Save", |hit| hit.y > 30);
    app.wait_text("Fix the open cell first");
    let blocked = app.settle();
    see(
        &blocked,
        &["EDIT", WHY],
        &["Save changes", "UPDATE customers"],
    );

    click_grid_row(&app, "Northwind");
    app.key("c");
    app.settle();
    click_first(&app, "Save", |hit| hit.y > 30);
    let review = app.wait_text("UPDATE customers");
    see(
        &review,
        &["Save changes", "SET customer = 'cab' WHERE id = 1001"],
        &["Fix the open cell first"],
    );
    app.key("esc");
    click_first(&app, "Discard", |hit| hit.y > 30);
    app.wait_text("no pending changes");
    app.finish();
}
