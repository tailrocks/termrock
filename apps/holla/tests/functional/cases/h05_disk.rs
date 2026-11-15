//! Holla disk tree, driven through the real binary.
//!
//! `parity-disk-navigation` does not open the disk page. Paste
//! `Analyze disk usage` and Enter. `~/work/big/target` is a noise name, and
//! fold starts on, so that directory is a leaf until `f`. Overview is not a
//! tree key: Go, Disk, then `Disk overview` pushes a page above the tree, and
//! Esc returns to that same tree. `t` pushes Top files the same way.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const ANALYZE: &str = "Analyze disk usage";
const OVERVIEW: &str = "Disk overview";
const TITLE: &str = "Disk usage · ~/work/big";
const WEB_TITLE: &str = "Disk usage · ~/Projects/web";
const CRUMB: &str = "Disk › Usage";
const OVERVIEW_CRUMB: &str = "Disk › Overview";
const TOP_TITLE: &str = "Top files · Spotlight";
const TOP_CRUMB: &str = "Usage › Top files";
const COMPLETE: &str = "scan complete";
const CANCELLED: &str = "partial · scan cancelled";
const QUIT: &str = "Quit holla";
const FINDER: &str = "Search actions";
const ROOT: &str = "~/work/big";
const TARGET: &str = "~/work/big/target";
const DEBUG: &str = "~/work/big/target/debug";
const DEPS: &str = "~/work/big/target/debug/deps";
const RLIB: &str = "~/work/big/target/debug/deps/lib.rlib";
const RELEASE: &str = "~/work/big/target/release";
const BIN: &str = "~/work/big/target/release/bin";
const NOISE: &str = "~/work/big/noise";
const DOCS: &str = "~/work/big/docs";
const GUIDE: &str = "~/work/big/docs/guide.md";
const SRC: &str = "~/work/big/src";
const MAIN: &str = "~/work/big/src/main.rs";
const CARGO: &str = "~/work/big/Cargo.toml";
const VM: &str = "~/work/big/vm.img";
const FOLDED: &str = "largest first · allocated · noise folded";
const APPARENT: &str = "largest first · apparent · noise folded";
const SORTED_APPARENT: &str = "Sorted by apparent size · rows still show allocated bytes";
const SORTED_ALLOCATED: &str = "Sorted by allocated size";
const UNFOLDED: &str = "Noise folders unfolded";
const REFOLDED: &str = "Noise folders folded · sizes unchanged";
const SELECTED_ALL: &str = "Every visible entry selected";
const SELECTED_NONE: &str = "Everything unselected";
const WIDE: &str = "H05.disk.tree.120x40";
const NARROW: &str = "H05.disk.tree.narrow";

const NOISE_NAMES: [&str; 14] = [
    "node_modules",
    ".git",
    "target",
    "build",
    "dist",
    ".venv",
    "venv",
    "__pycache__",
    "DerivedData",
    "Pods",
    ".gradle",
    ".next",
    ".turbo",
    ".cache",
];

const ABSENT: [&str; 6] = [
    "sparse.img",
    "evicted.bin",
    "~/work/data",
    "842 of 926",
    "/Volumes/Backup",
    "~/Projects/web",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h05_disk/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(FINDER);
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn row_cells(screen: &TerminalSnapshot, row: u32) -> Vec<String> {
    screen
        .row(row)
        .expect("row")
        .iter()
        .map(|cell| cell.text().to_owned())
        .collect()
}

fn cells_at(cells: &[String], at: usize, label: &str) -> bool {
    let chars: Vec<String> = label.chars().map(|ch| ch.to_string()).collect();
    cells.get(at..at + chars.len()) == Some(chars.as_slice())
}

fn find_cells(cells: &[String], label: &str) -> Option<usize> {
    (0..cells.len()).find(|&at| cells_at(cells, at, label))
}

fn path_chars(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '~' | '-'))
}

fn take_path(cells: &[String], col: usize) -> String {
    let mut out = String::new();
    for cell in cells.iter().skip(col) {
        if cell == " " {
            if out.is_empty() {
                continue;
            }
            break;
        }
        if !path_chars(cell) {
            break;
        }
        out.push_str(cell);
    }
    out
}

fn detail_path(screen: &TerminalSnapshot) -> Option<String> {
    for row in 0..screen.rows() {
        let cells = row_cells(screen, row);
        let Some(at) = find_cells(&cells, "Path") else {
            continue;
        };
        let col = at + "Path".len();
        let mut value = take_path(&cells, col);
        if !value.starts_with('~') && !value.starts_with('/') {
            continue;
        }
        for next in row + 1..screen.rows() {
            let next_cells = row_cells(screen, next);
            if find_cells(&next_cells, "Allocated").is_some()
                || find_cells(&next_cells, "Apparent").is_some()
                || find_cells(&next_cells, "Entries").is_some()
            {
                break;
            }
            let frag = take_path(&next_cells, col);
            if frag.is_empty() {
                break;
            }
            value.push_str(&frag);
        }
        return Some(value);
    }
    None
}

fn path_of(screen: &TerminalSnapshot) -> String {
    detail_path(screen).unwrap_or_else(|| format!("no Path\n{}", text(screen)))
}

fn at(app: &App) -> String {
    path_of(&app.screen())
}

fn until(
    app: &App,
    key: &str,
    pred: impl Fn(&TerminalSnapshot) -> bool,
    why: &str,
) -> TerminalSnapshot {
    app.key(key);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let screen = app.screen();
        if pred(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "{why}\npath {}\n{}",
            path_of(&screen),
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn go(app: &App, key: &str, path: &str) {
    let screen = until(
        app,
        key,
        |screen| detail_path(screen).as_deref() == Some(path),
        path,
    );
    assert_eq!(path_of(&screen), path, "{}", text(&screen));
}

fn expand(app: &App, marker: &str) {
    let path = at(app);
    let screen = until(
        app,
        "right",
        |screen| {
            detail_path(screen).as_deref() == Some(path.as_str()) && shows_name(screen, marker)
        },
        marker,
    );
    assert_eq!(path_of(&screen), path, "expand\n{}", text(&screen));
    assert!(
        shows_name(&screen, marker),
        "missing {marker}\n{}",
        text(&screen)
    );
}

fn collapse(app: &App, marker: &str) {
    let path = at(app);
    let screen = until(
        app,
        "left",
        |screen| {
            detail_path(screen).as_deref() == Some(path.as_str()) && !shows_name(screen, marker)
        },
        marker,
    );
    assert_eq!(path_of(&screen), path, "collapse\n{}", text(&screen));
    assert!(
        !shows_name(&screen, marker),
        "still showing {marker}\n{}",
        text(&screen)
    );
}

fn status(app: &App, key: &str, message: &str) -> TerminalSnapshot {
    until(app, key, |screen| text(screen).contains(message), message)
}

fn selected_total(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let Some(line) = visible.lines().find(|line| line.contains("selected (")) else {
        panic!("no selected total\n{visible}");
    };
    let rest = line.split("selected (").nth(1).expect("selected");
    let Some(end) = rest.find(')') else {
        panic!("open selected total\n{visible}");
    };
    rest[..end].to_string()
}

fn row_name(line: &str, name: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find(name) {
        let before = if at == 0 {
            None
        } else {
            rest[..at].chars().next_back()
        };
        let after = rest[at + name.len()..].chars().next();
        let boundary =
            |ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '.' && ch != '-';
        let before_ok = before.is_none_or(boundary);
        let after_ok = after.is_none_or(boundary);
        if before_ok && after_ok {
            return true;
        }
        rest = &rest[at + 1..];
    }
    false
}

fn shows_name(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| row_name(line, name))
}

fn walk_noise(app: &App) -> Vec<String> {
    expand(app, ".git");
    let mut names = Vec::new();
    loop {
        go_step(app, "down");
        let path = at(app);
        if let Some(name) = path.strip_prefix("~/work/big/noise/") {
            assert!(!name.contains('/'), "{path}");
            names.push(name.to_string());
            continue;
        }
        break;
    }
    go(
        app,
        "up",
        &format!("~/work/big/noise/{}", names.last().expect("noise child")),
    );
    while at(app) != NOISE {
        let before = at(app);
        let screen = until(
            app,
            "left",
            |screen| detail_path(screen).as_deref() != Some(before.as_str()),
            "climb noise",
        );
        assert_ne!(path_of(&screen), before);
    }
    collapse(app, "node_modules");
    names
}

fn go_step(app: &App, key: &str) {
    let before = at(app);
    let screen = until(
        app,
        key,
        |screen| detail_path(screen).as_deref() != Some(before.as_str()),
        key,
    );
    assert_ne!(path_of(&screen), before, "{key}");
}

fn open_analyze(app: &App) {
    app.paste(ANALYZE);
    app.wait_text(&format!("▎ {ANALYZE}"));
    app.key("enter");
}

fn wait_complete(app: &App) -> TerminalSnapshot {
    app.wait_screen("scan complete", |screen| {
        let visible = text(screen);
        visible.contains(COMPLETE)
            && visible.contains(TITLE)
            && visible.contains(CRUMB)
            && visible.contains(FOLDED)
            && !visible.contains(CANCELLED)
            && !visible.contains(QUIT)
            && detail_path(screen).as_deref() == Some(ROOT)
    })
}

fn tree_checkpoint(screen: &TerminalSnapshot, total: &str) -> bool {
    let visible = text(screen);
    visible.contains(COMPLETE)
        && visible.contains(TITLE)
        && visible.contains(&format!("selected ({total})"))
        && visible.contains(SORTED_APPARENT)
        && visible.contains(APPARENT)
        && !visible.contains(TOP_TITLE)
        && !visible.contains(CANCELLED)
        && !visible.contains("Disk › Top files")
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn open_overview(app: &App) {
    app.key("f10");
    app.wait_screen("file menu", |screen| text(screen).contains("Close tab"));
    app.key("right");
    app.wait_screen("go menu", |screen| {
        text(screen).contains("Parent scope") && text(screen).contains("Disk")
    });
    for _ in 0..8 {
        app.key("down");
    }
    app.key("d");
    app.key("enter");
    app.wait_screen("disk catalog", |screen| {
        text(screen).contains("Search disk…") && text(screen).contains(OVERVIEW)
    });
    app.paste(OVERVIEW);
    app.wait_text(&format!("▎ {OVERVIEW}"));
    app.key("enter");
    app.wait_screen("disk overview", |screen| {
        let visible = text(screen);
        visible.contains("cached sizes are hints with their age")
            && visible.contains(OVERVIEW)
            && visible.contains(OVERVIEW_CRUMB)
            && !visible.contains(TITLE)
    });
}

fn back_to_tree(app: &App) {
    for _ in 0..6 {
        let screen = app.screen();
        if text(&screen).contains(TITLE)
            && detail_path(&screen).as_deref() == Some(VM)
            && !text(&screen).contains(TOP_TITLE)
            && !text(&screen).contains("cached sizes are hints with their age")
        {
            return;
        }
        app.key("escape");
        let deadline = Instant::now() + Duration::from_millis(800);
        while Instant::now() < deadline {
            let screen = app.screen();
            if text(&screen).contains(TITLE) && detail_path(&screen).as_deref() == Some(VM) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    let screen = app.screen();
    panic!(
        "tree not restored\npath {}\n{}",
        path_of(&screen),
        text(&screen)
    );
}

#[test]
fn h05_disk_route_returns_to_the_finder() {
    let app = launch("h05-disk-route", "parity-disk-navigation");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[FINDER], &[TITLE, QUIT]);

    open_analyze(&app);
    let usage = app.wait_screen("disk usage", |screen| {
        let visible = text(screen);
        visible.contains(TITLE) && visible.contains(CRUMB) && !visible.contains(QUIT)
    });
    assert_eq!((usage.cols(), usage.rows()), (120, 40));
    see(&usage, &[TITLE, CRUMB, ROOT], &[QUIT, CANCELLED]);

    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        visible.contains(ANALYZE)
            && visible.contains("scope ‹ here ›")
            && !visible.contains(TITLE)
            && !visible.contains(COMPLETE)
            && !visible.contains(QUIT)
    });
    see(
        &finder,
        &[ANALYZE, "scope ‹ here ›"],
        &[TITLE, COMPLETE, QUIT],
    );
    app.finish();
}

#[test]
fn h05_disk_tree_sort_selection_and_return() {
    let app = launch("h05-disk-tree", "parity-disk-navigation");
    open_analyze(&app);
    let done = wait_complete(&app);
    see(&done, &[TITLE, COMPLETE, CRUMB, FOLDED, ROOT], &ABSENT);

    go(&app, "down", TARGET);
    go(&app, "down", NOISE);
    let names = walk_noise(&app);
    for name in NOISE_NAMES {
        assert!(
            names.iter().any(|got| got == name),
            "missing noise {name}: {names:?}"
        );
    }
    assert!(names.iter().any(|name| name == "target"));
    assert_eq!(at(&app), NOISE);

    go(&app, "down", VM);
    let vm = app.screen();
    see(&vm, &[VM, "sparse"], &ABSENT);
    assert!(text(&vm).contains("Apparent") || text(&vm).contains("sparse"));

    go(&app, "down", DOCS);
    expand(&app, "guide.md");
    go(&app, "down", GUIDE);
    go(&app, "left", DOCS);
    collapse(&app, "guide.md");
    go(&app, "down", SRC);
    expand(&app, "main.rs");
    go(&app, "down", MAIN);
    go(&app, "left", SRC);
    collapse(&app, "main.rs");
    go(&app, "down", CARGO);
    assert_eq!(at(&app), CARGO);

    status(&app, "f", UNFOLDED);
    assert!(
        text(&app.screen()).contains("unfolded"),
        "{}",
        text(&app.screen())
    );
    go(&app, "up", SRC);
    go(&app, "up", DOCS);
    go(&app, "up", VM);
    go(&app, "up", NOISE);
    go(&app, "up", TARGET);
    expand(&app, "debug");
    go(&app, "down", DEBUG);
    expand(&app, "deps");
    go(&app, "down", DEPS);
    expand(&app, "lib.rlib");
    go(&app, "down", RLIB);
    assert_eq!(at(&app), RLIB);
    go(&app, "down", RELEASE);
    expand(&app, "bin");
    go(&app, "down", BIN);
    assert_eq!(at(&app), BIN);
    go(&app, "left", RELEASE);
    collapse(&app, "bin");
    go(&app, "left", TARGET);
    collapse(&app, "debug");

    let folded = status(&app, "f", REFOLDED);
    assert_eq!(selected_total(&folded), "0 B", "{}", text(&folded));
    assert!(text(&folded).contains(FOLDED), "{}", text(&folded));

    go(&app, "down", NOISE);
    go(&app, "down", VM);
    let ready = app.screen();
    see(
        &ready,
        &[VM, "sparse", COMPLETE, "0 selected (0 B)"],
        &ABSENT,
    );
    assert_eq!(selected_total(&ready), "0 B");

    let selected = status(&app, "space", "1 item selected");
    let total = selected_total(&selected);
    assert_ne!(total, "0 B", "{}", text(&selected));
    let sorted = status(&app, "s", SORTED_APPARENT);
    assert_eq!(
        selected_total(&sorted),
        total,
        "sort changed the selected total\n{}",
        text(&sorted)
    );
    see(
        &sorted,
        &[
            COMPLETE,
            TITLE,
            APPARENT,
            SORTED_APPARENT,
            &format!("selected ({total})"),
        ],
        &[CANCELLED, "Disk › Top files", TOP_TITLE],
    );
    assert_eq!(path_of(&sorted), VM, "{}", text(&sorted));

    let total_for_wide = total.clone();
    nudge(&app, |screen| {
        tree_checkpoint(screen, &total_for_wide) && detail_path(screen).as_deref() == Some(VM)
    });
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrow = app.wait_screen("narrow tree", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && tree_checkpoint(screen, &total)
    });
    see(
        &narrow,
        &[COMPLETE, TITLE, &format!("selected ({total})")],
        &[CANCELLED, TOP_TITLE, "Disk › Top files"],
    );
    let total_for_narrow = total.clone();
    nudge(&app, |screen| tree_checkpoint(screen, &total_for_narrow));
    shot(&app, NARROW);

    app.resize(120, 40);
    let restored = app.wait_screen("tree restored", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && tree_checkpoint(screen, &total)
            && detail_path(screen).as_deref() == Some(VM)
    });
    see(
        &restored,
        &[COMPLETE, TITLE, VM, "sparse"],
        &[TOP_TITLE, CANCELLED],
    );

    let cleared = status(&app, "space", "Unselected");
    assert_eq!(selected_total(&cleared), "0 B", "{}", text(&cleared));
    let again = status(&app, "space", "1 item selected");
    assert_eq!(selected_total(&again), total, "{}", text(&again));

    let allocated = status(&app, "s", SORTED_ALLOCATED);
    assert_eq!(
        selected_total(&allocated),
        total,
        "sort changed the selected total\n{}",
        text(&allocated)
    );
    assert!(
        text(&allocated).contains("largest first · allocated · noise folded"),
        "{}",
        text(&allocated)
    );

    let unfolded = status(&app, "f", UNFOLDED);
    assert_eq!(
        selected_total(&unfolded),
        total,
        "fold changed the selected total\n{}",
        text(&unfolded)
    );
    let refolded = status(&app, "f", REFOLDED);
    assert_eq!(
        selected_total(&refolded),
        total,
        "fold changed the selected total\n{}",
        text(&refolded)
    );
    assert_eq!(path_of(&refolded), VM, "{}", text(&refolded));

    open_overview(&app);
    let overview = app.screen();
    see(
        &overview,
        &[
            "cached sizes are hints with their age",
            OVERVIEW,
            OVERVIEW_CRUMB,
        ],
        &[TOP_TITLE, CANCELLED],
    );

    back_to_tree(&app);
    let returned = app.screen();
    see(
        &returned,
        &[TITLE, COMPLETE, VM],
        &[TOP_TITLE, OVERVIEW_CRUMB, CANCELLED],
    );
    assert_eq!(path_of(&returned), VM, "{}", text(&returned));
    assert_eq!(selected_total(&returned), total, "{}", text(&returned));

    let top = status(&app, "t", TOP_TITLE);
    see(
        &top,
        &[
            TOP_TITLE,
            TOP_CRUMB,
            "iso.iso",
            "big.mov",
            "vanished.bin",
            "stat failed",
        ],
        &["small.bin", TITLE, CANCELLED],
    );
    let iso = text(&top).matches("iso.iso").count();
    assert_eq!(
        iso,
        3,
        "iso.iso is the list row, the detail title, and the detail path\n{}",
        text(&top)
    );
    app.key("escape");
    let after_top = app.wait_screen("tree after top files", |screen| {
        text(screen).contains(TITLE)
            && detail_path(screen).as_deref() == Some(VM)
            && !text(screen).contains(TOP_TITLE)
    });
    see(&after_top, &[TITLE, COMPLETE, VM], &[TOP_TITLE, CANCELLED]);
    assert_eq!(path_of(&after_top), VM);
    assert_eq!(selected_total(&after_top), total, "{}", text(&after_top));

    let every = status(&app, "a", SELECTED_ALL);
    let every_total = selected_total(&every);
    assert_ne!(
        every_total,
        total,
        "select-all left the total unchanged\n{}",
        text(&every)
    );
    let none = status(&app, "a", SELECTED_NONE);
    assert_eq!(selected_total(&none), "0 B", "{}", text(&none));
    see(&none, &[TITLE, COMPLETE, SELECTED_NONE], &ABSENT);
    app.finish();
}

#[test]
fn h05_disk_scenario_parity_insights() {
    let app = launch("h05-disk-insights", "parity-insights");
    open_analyze(&app);
    let done = app.wait_screen("insights tree", |screen| {
        let visible = text(screen);
        visible.contains(COMPLETE) && visible.contains(WEB_TITLE) && !visible.contains(QUIT)
    });
    let visible = text(&done);
    assert!(
        visible.contains("package.json") || visible.contains("node_modules"),
        "missing web identity\n{visible}"
    );
    see(
        &done,
        &[WEB_TITLE, COMPLETE, CRUMB],
        &["vm.img", "~/work/big", QUIT],
    );
    app.finish();
}
