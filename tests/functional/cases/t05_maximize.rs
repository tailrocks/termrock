//! TablePro explorer and pane layout before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_maximize/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{
    App, Launch, Point, TABLEPRO, find, find_all, flash_settle, row_text, text, try_launch,
};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY_1: &str = "Query 1";
const QUERY_2: &str = "Query 2";
const PLACEHOLDER: &str = "Type SQL";
const EXPLORER: &str = "Filter objects";
const RESULTS: &str = "No results yet";
const ONE: &str = "SELECT 't05_max_one'";
const TWO: &str = "SELECT 't05_max_two'";
const WIDE: &str = "T05.maximize.layout.120x40";
const NARROW: &str = "T05.maximize.layout.narrow";
const ACCENT: SnapshotColor = SnapshotColor::Rgb(72, 224, 84);
const THIN_V: char = '│';
const THIN_H: char = '─';
const HEAVY_V: char = '┃';
const HEAVY_H: char = '━';

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_maximize/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    app
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
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

fn click_listed(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "list hit {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_connect(app: &App) {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.x >= 40
                && row.contains("Edit")
                && row.contains("Delete")
                && !row.contains("Reconnect")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn connect_development(app: &App) {
    click_listed(app, "Development");
    let detail = app.wait_text(HOST);
    see(
        &detail,
        &[HOST, DATABASE, "Development"],
        &["Authentication failed", "Could not reach the host"],
    );
    click_connect(app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY_1)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[
            CONNECTED,
            "acme_prod › public",
            QUERY_1,
            PLACEHOLDER,
            EXPLORER,
        ],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Execute query?",
            "No open tabs",
        ],
    );
}

fn glyph(screen: &TerminalSnapshot, x: u16, y: u16) -> char {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .and_then(|cell| cell.text().chars().next())
        .unwrap_or(' ')
}

fn ink(screen: &TerminalSnapshot, x: u16, y: u16) -> SnapshotColor {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|cell| cell.fg())
        .unwrap_or(SnapshotColor::Default)
}

fn statement(screen: &TerminalSnapshot, sql: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        let Some(at) = line.find(sql) else {
            return false;
        };
        match line[at + sql.len()..].chars().next() {
            None => true,
            Some(next) => !next.is_ascii_alphanumeric(),
        }
    })
}

fn both_tabs(screen: &TerminalSnapshot) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains(QUERY_1) && line.contains(QUERY_2)
    })
}

fn editor_max(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    both_tabs(screen)
        && statement(screen, TWO)
        && !visible.contains(EXPLORER)
        && !visible.contains(RESULTS)
        && !visible.contains("No open tabs")
        && !visible.contains("two'z")
        && !visible.contains("t05_max_twoz")
}

fn restored_layout(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    both_tabs(screen)
        && statement(screen, TWO)
        && visible.contains(EXPLORER)
        && visible.contains(RESULTS)
        && !visible.contains("No open tabs")
        && !visible.contains("two'z")
}

fn click_sql(app: &App, sql: &str) {
    let screen = app.wait_screen("sql to focus", |screen| statement(screen, sql));
    let hits = find_all(&screen, sql);
    assert_eq!(hits.len(), 1, "sql hit {sql:?}\n{}", text(&screen));
    app.click(hits[0]);
    app.wait_text("Ctrl+Space");
}

fn leave_editor(app: &App, sql: &str) {
    app.key("esc");
    app.wait_screen("editor released", |screen| {
        !text(screen).contains("Ctrl+Space") && statement(screen, sql)
    });
}

fn query_seam(screen: &TerminalSnapshot) -> Point {
    let sql = find(screen, TWO);
    let results = find(screen, RESULTS);
    assert!(
        results.y > sql.y + 1,
        "results are not below the editor\n{}",
        text(screen)
    );
    for y in (sql.y + 1)..results.y {
        let mut dashes = Vec::new();
        for x in 0..screen.cols() {
            if glyph(screen, x as u16, y) == THIN_H {
                dashes.push(x as u16);
            }
        }
        if dashes.len() > 8 {
            return Point {
                x: dashes[dashes.len() / 2],
                y,
            };
        }
    }
    panic!(
        "no query seam between the sql and the results\n{}",
        text(screen)
    );
}

fn explorer_seam(screen: &TerminalSnapshot) -> Point {
    let sql = find(screen, TWO);
    let mut x = sql.x;
    while x >= 2 {
        x -= 1;
        if glyph(screen, x, sql.y) == THIN_V
            && glyph(screen, x - 1, sql.y) == THIN_V
            && glyph(screen, x - 2, sql.y) == THIN_V
        {
            return Point { x: x - 1, y: sql.y };
        }
    }
    panic!("no explorer seam left of the sql\n{}", text(screen));
}

fn row_has(screen: &TerminalSnapshot, y: u16, glyph_at: char) -> bool {
    (0..screen.cols()).any(|x| glyph(screen, x as u16, y) == glyph_at)
}

/// Accent heavy rules that belong to a pane seam. The active tab underline is
/// also an accent `━`, and that row has no panel border.
fn accent_cells(screen: &TerminalSnapshot, heavy: char) -> Vec<Point> {
    let mut cells = Vec::new();
    for y in 0..screen.rows() {
        let y = y as u16;
        if heavy == HEAVY_H && !row_has(screen, y, THIN_V) {
            continue;
        }
        for x in 0..screen.cols() {
            let x = x as u16;
            if glyph(screen, x, y) == heavy && ink(screen, x, y) == ACCENT {
                cells.push(Point { x, y });
            }
        }
    }
    cells
}

fn one_line(cells: &[Point], heavy: char, screen: &TerminalSnapshot) -> Point {
    assert!(!cells.is_empty(), "no accent {heavy}\n{}", text(screen));
    let vertical = cells.iter().all(|cell| cell.x == cells[0].x);
    let horizontal = cells.iter().all(|cell| cell.y == cells[0].y);
    assert!(
        vertical || horizontal,
        "accent {heavy} is not one seam\n{}",
        text(screen)
    );
    cells[cells.len() / 2]
}

/// `on_up` leaves hover on the last hit, and a hovered seam stays the heavy
/// rule. The pointer has to leave the padded hit before the thin rest glyph
/// comes back. Neighbour rules stay thin the whole time.
fn drag_seam(app: &App, seam: Point, delta: (i16, i16), thin: char, heavy: char) {
    let before = app.screen();
    assert_eq!(
        glyph(&before, seam.x, seam.y),
        thin,
        "seam did not start thin at {seam:?}\n{}",
        text(&before)
    );
    let (left, right) = seam_neighbours(seam, delta);
    assert_ne!(
        glyph(&before, left.0, left.1),
        heavy,
        "neighbour already heavy"
    );
    assert_ne!(
        glyph(&before, right.0, right.1),
        heavy,
        "neighbour already heavy"
    );

    app.move_to(seam);
    let hovered = app.wait_screen("seam hover", |screen| {
        glyph(screen, seam.x, seam.y) == heavy
    });
    assert_ne!(
        ink(&hovered, seam.x, seam.y),
        ACCENT,
        "hover painted the accent"
    );
    assert_ne!(glyph(&hovered, left.0, left.1), heavy);
    assert_ne!(glyph(&hovered, right.0, right.1), heavy);

    app.down(seam);
    let end = Point {
        x: seam.x.checked_add_signed(delta.0).expect("drag x"),
        y: seam.y.checked_add_signed(delta.1).expect("drag y"),
    };
    app.drag_to(end);
    let held = app.wait_screen("seam held", |screen| {
        !accent_cells(screen, heavy).is_empty()
    });
    let at = one_line(&accent_cells(&held, heavy), heavy, &held);
    let (n1, n2) = seam_neighbours(at, delta);
    assert_ne!(
        glyph(&held, n1.0, n1.1),
        heavy,
        "drag restyled a neighbour\n{}",
        text(&held)
    );
    assert_ne!(
        glyph(&held, n2.0, n2.1),
        heavy,
        "drag restyled a neighbour\n{}",
        text(&held)
    );
    assert!(
        statement(&held, TWO) && both_tabs(&held) && text(&held).contains(EXPLORER),
        "drag changed the tabs or the sql\n{}",
        text(&held)
    );

    app.up(end);
    let released = app.settle();
    assert!(
        accent_cells(&released, heavy).is_empty(),
        "release kept the accent seam\n{}",
        text(&released)
    );
    // on_up leaves hover on the last hit, and the statement can sit inside
    // that pad after a short drag. The header is outside every seam.
    app.move_to(Point { x: 2, y: 0 });
    let vertical_line = delta.0 != 0;
    let rested = app.wait_screen("seam at rest", |screen| {
        statement(screen, TWO)
            && both_tabs(screen)
            && seam_released(screen, thin, heavy, vertical_line)
    });
    assert!(
        seam_released(&rested, thin, heavy, vertical_line),
        "seam stayed heavy after the pointer left {at:?}\n{}",
        text(&rested)
    );
    assert!(statement(&rested, TWO) && both_tabs(&rested));
}

/// The held accent midpoint is not the committed rule. Percent snapping can
/// move the split by more than one cell, and the tab underline is a heavy
/// `━` on a different row. A resting query seam is the long thin rule between
/// the statement and the results. A resting explorer seam is the triple thin
/// bars on the statement row. Either one still carrying the heavy glyph fails.
fn seam_released(screen: &TerminalSnapshot, thin: char, heavy: char, vertical_line: bool) -> bool {
    if vertical_line {
        explorer_resting(screen, thin, heavy)
    } else {
        query_resting(screen, thin, heavy)
    }
}

fn query_resting(screen: &TerminalSnapshot, thin: char, heavy: char) -> bool {
    let sql = find_all(screen, TWO);
    let results = find_all(screen, RESULTS);
    if sql.len() != 1 || results.len() != 1 || results[0].y <= sql[0].y + 1 {
        return false;
    }
    for y in (sql[0].y + 1)..results[0].y {
        let mut thin_run = 0u16;
        let mut heavy_run = 0u16;
        for x in 0..screen.cols() {
            let cell = glyph(screen, x as u16, y);
            if cell == thin {
                thin_run += 1;
            } else if cell == heavy {
                heavy_run += 1;
            }
        }
        if thin_run > 8 && heavy_run == 0 {
            return true;
        }
    }
    false
}

fn explorer_resting(screen: &TerminalSnapshot, thin: char, heavy: char) -> bool {
    let sql = find_all(screen, TWO);
    if sql.len() != 1 || sql[0].x < 2 {
        return false;
    }
    let y = sql[0].y;
    let mut x = sql[0].x;
    let mut seam_x = None;
    while x >= 2 {
        x -= 1;
        if x >= 2
            && glyph(screen, x, y) == thin
            && glyph(screen, x - 1, y) == thin
            && glyph(screen, x - 2, y) == thin
        {
            seam_x = Some(x - 1);
            break;
        }
    }
    let Some(seam_x) = seam_x else {
        return false;
    };
    !(0..screen.rows()).any(|row| glyph(screen, seam_x, row as u16) == heavy)
}

fn seam_neighbours(seam: Point, delta: (i16, i16)) -> ((u16, u16), (u16, u16)) {
    if delta.0 != 0 {
        ((seam.x - 1, seam.y), (seam.x + 1, seam.y))
    } else {
        ((seam.x, seam.y - 1), (seam.x, seam.y + 1))
    }
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &["Query history"]);
    app.key("right");
    app.key("enter");
    app.wait_exit("tablepro stayed up after confirming quit");
    app.expect_exited(0);
}

#[test]
fn tablepro_maximize_route_reaches_the_workbench() {
    let app = open("t05-maximize-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Filter connections", "Development"],
        &[QUERY_1, EXPLORER],
    );
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY_1, PLACEHOLDER, CONNECTED, EXPLORER, "Development"],
        &["Loading schema", "Query history", QUERY_2, "No open tabs"],
    );
    app.finish();
}

#[test]
fn tablepro_maximize_restores_the_same_layout() {
    let app = open("t05-maximize-layout");
    connect_development(&app);

    click_sql(&app, PLACEHOLDER);
    app.type_text(ONE);
    let first = app.wait_screen("first statement", |screen| statement(screen, ONE));
    see(&first, &[ONE, QUERY_1, EXPLORER], &[QUERY_2, "two'z"]);

    app.key("ctrl+t");
    app.wait_screen("second query", |screen| {
        both_tabs(screen)
            && text(screen).contains(PLACEHOLDER)
            && text(screen).contains("Ctrl+Space")
    });
    app.type_text(TWO);
    let second = app.wait_screen("second statement", |screen| statement(screen, TWO));
    see(
        &second,
        &[TWO, QUERY_1, QUERY_2],
        &["t05_max_twoz", "No open tabs"],
    );

    // Ctrl+B is a chord, so the capturing editor does not insert it.
    app.key("ctrl+b");
    let hidden = app.wait_screen("explorer hidden", |screen| {
        let visible = text(screen);
        !visible.contains(EXPLORER) && statement(screen, TWO) && both_tabs(screen)
    });
    see(
        &hidden,
        &[TWO, QUERY_1, QUERY_2],
        &[EXPLORER, "No open tabs", "twob"],
    );

    app.key("ctrl+b");
    let shown = app.wait_screen("explorer shown", |screen| {
        text(screen).contains(EXPLORER) && statement(screen, TWO) && both_tabs(screen)
    });
    see(
        &shown,
        &[EXPLORER, TWO, QUERY_1, QUERY_2],
        &["No open tabs"],
    );

    // Showing the explorer focuses it, so [ ] change tabs without typing.
    app.key("[");
    let query_one = app.wait_screen("query 1 still open", |screen| {
        statement(screen, ONE) && both_tabs(screen) && text(screen).contains(EXPLORER)
    });
    assert!(
        !statement(&query_one, TWO),
        "query 1 showed the other statement\n{}",
        text(&query_one)
    );
    app.key("]");
    app.wait_screen("query 2 still open", |screen| {
        statement(screen, TWO) && both_tabs(screen) && text(screen).contains(EXPLORER)
    });

    let seams = app.screen();
    assert_eq!((seams.cols(), seams.rows()), (120, 40));
    let query = query_seam(&seams);
    drag_seam(&app, query, (0, 3), THIN_H, HEAVY_H);
    let after_query = app.screen();
    let explorer = explorer_seam(&after_query);
    drag_seam(&app, explorer, (4, 0), THIN_V, HEAVY_V);

    // z is a letter. Esc commits and leaves the editor so the next z maximizes.
    click_sql(&app, TWO);
    leave_editor(&app, TWO);
    flash_settle(&app);
    app.key("z");
    let maximized = app.wait_screen("editor maximized", editor_max);
    assert_eq!((maximized.cols(), maximized.rows()), (120, 40));
    see(
        &maximized,
        &[TWO, QUERY_1, QUERY_2, CONNECTED],
        &[EXPLORER, RESULTS, "No open tabs", "two'z"],
    );
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("editor maximized at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && editor_max(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(
        &narrowed,
        &[TWO, QUERY_1, QUERY_2],
        &[EXPLORER, RESULTS, "No open tabs"],
    );
    shot(&app, NARROW);
    app.resize(120, 40);
    let wide_again = app.wait_screen("editor maximized back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && editor_max(screen)
    });
    assert!(statement(&wide_again, TWO) && both_tabs(&wide_again));

    app.key("z");
    let restored = app.wait_screen("layout restored", restored_layout);
    see(
        &restored,
        &[EXPLORER, TWO, QUERY_1, QUERY_2, RESULTS],
        &["No open tabs", "two'z"],
    );

    app.key("[");
    app.wait_screen("query 1 after restore", |screen| {
        statement(screen, ONE) && both_tabs(screen) && text(screen).contains("Ctrl+Space")
    });
    leave_editor(&app, ONE);
    app.key("]");
    app.wait_screen("query 2 after restore", |screen| statement(screen, TWO));
    leave_editor(&app, TWO);

    app.key("0");
    let explorer_focus = app.wait_screen("explorer focused", |screen| {
        text(screen).contains(EXPLORER) && statement(screen, TWO) && !text(screen).contains("two0")
    });
    assert!(
        statement(&explorer_focus, TWO),
        "0 was inserted\n{}",
        text(&explorer_focus)
    );
    app.key("z");
    let results = app.wait_screen("results maximized", |screen| {
        let visible = text(screen);
        both_tabs(screen)
            && visible.contains(RESULTS)
            && !statement(screen, TWO)
            && !visible.contains(EXPLORER)
            && !visible.contains("No open tabs")
    });
    see(
        &results,
        &[RESULTS, QUERY_1, QUERY_2],
        &[EXPLORER, TWO, "No open tabs"],
    );
    app.key("z");
    let again = app.wait_screen("results restored", restored_layout);
    see(
        &again,
        &[EXPLORER, TWO, QUERY_1, QUERY_2, RESULTS],
        &["No open tabs"],
    );

    quit_unsaved(&app);
}
