//! Holla file find, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h05_find/` and each checkpoint's dots become underscores.
//!
//! Catalog item `find.files` is labeled `Find files under home…` and launches
//! `Launch::Find`. The page crumb is `Files › Find`. `Mode::Find` Esc clears a
//! non-empty query and pops when the query is empty. Enter calls
//! `open_actions`. With no hit, `selected_path` is `None` and the status is
//! `Select a file or folder first`. `Preview here` is the `_` arm of
//! `file_action`: a directory becomes `Mode::Browse` and `navigate`s to that
//! path. `Location::short` paints the home prefix as `~`. `render_find_list`
//! paints the file name and the parent on the hit row, not one full path.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const LABEL: &str = "Find files under home…";
const CRUMB: &str = "Files › Find";
const TYPE_HINT: &str = "Type to search under home";
const QUERY: &str = "src";
const PARENT: &str = "~/work/app";
const HIT: &str = "~/work/app/src";
const FULL: &str = "/Users/alex/work/app/src";
const SITE: &str = "/Users/alex/work/site";
const NOTES: &str = "Files · ~/work/notes";
const BROWSE: &str = "Files · ~/work/app/src";
const BROWSE_CRUMB: &str = "Files › src";
const ABSENT: &str = "zztyped-absent";
const NO_MATCH: &str = "No matches for “zztyped-absent”";
const EMPTY_STATUS: &str = "Select a file or folder first";
const OPEN_OS: &str = "Open in the OS";
const REVEAL: &str = "Reveal in the file manager";
const COPY: &str = "Copy full path";
const ANALYZE: &str = "Analyze disk usage";
const PREVIEW: &str = "Preview here";
const QUIT: &str = "Quit holla";
const FINDER: &str = "Search actions";
const WIDE: &str = "H05.find.find.120x40";
const NARROW: &str = "H05.find.find.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h05_find/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec!["--scenario".to_string(), "parity-files".to_string()];
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

/// The status bar paints `· {cols}×{rows}` from the frame Holla just drew.
/// The PTY grid changes first, and the previous frame still shows the selected row.
fn painted(screen: &TerminalSnapshot, cols: u16, rows: u16) -> bool {
    text(screen).contains(&format!("· {cols}×{rows}"))
}

fn src_selected(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    if !visible.contains(CRUMB)
        || visible.contains("indexing")
        || visible.contains(SITE)
        || visible.contains("work/site")
        || visible.contains(OPEN_OS)
        || visible.contains(QUIT)
    {
        return false;
    }
    visible.lines().any(|line| {
        let Some(mark) = line.find('▸') else {
            return false;
        };
        let Some(name) = line.find(QUERY) else {
            return false;
        };
        name > mark && name - mark <= 4 && line.contains(PARENT)
    })
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && painted(screen, nudged, rows)
            && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && painted(screen, cols, rows)
            && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn open_find(app: &App) {
    app.paste(LABEL);
    app.wait_text(&format!("▎ {LABEL}"));
    app.key("enter");
    let page = app.wait_screen("find page", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB)
            && visible.contains(LABEL)
            && visible.contains(TYPE_HINT)
            && !visible.contains(QUIT)
            && !visible.contains(NOTES)
    });
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(&page, &[CRUMB, LABEL, TYPE_HINT], &[QUIT, NOTES, SITE]);
}

fn search_src(app: &App) -> TerminalSnapshot {
    app.paste(QUERY);
    app.wait_screen("src directory selected", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && painted(screen, 120, 40)
            && src_selected(screen)
    })
}

#[test]
fn h05_find_route_returns_to_the_finder() {
    let app = launch("h05-find-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[FINDER], &[CRUMB, QUIT]);

    open_find(&app);
    app.paste(QUERY);
    let typed = app.wait_screen("non-empty query", src_selected);
    see(&typed, &[CRUMB, QUERY, PARENT], &[QUIT, SITE]);

    app.key("escape");
    let cleared = app.wait_screen("query cleared", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB)
            && visible.contains(TYPE_HINT)
            && visible.contains("0 results")
            && !visible.contains(QUIT)
            && !src_selected(screen)
    });
    see(
        &cleared,
        &[CRUMB, TYPE_HINT, "0 results"],
        &[QUIT, SITE, NOTES],
    );

    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        !visible.contains(CRUMB) && visible.contains(LABEL) && !visible.contains(QUIT)
    });
    see(&finder, &[LABEL], &[CRUMB, QUIT, BROWSE]);
    app.finish();
}

#[test]
fn h05_find_find_opens_the_selected_directory() {
    let app = launch("h05-find-find");
    open_find(&app);
    let found = search_src(&app);
    see(
        &found,
        &[CRUMB, QUERY, PARENT, HIT, "1 result"],
        &[SITE, "work/site", OPEN_OS, QUIT, "indexing", NOTES],
    );

    nudge(&app, src_selected);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("narrow find results", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && painted(screen, 80, 24)
            && src_selected(screen)
    });
    see(
        &narrowed,
        &[CRUMB, QUERY, PARENT],
        &[SITE, "work/site", OPEN_OS, QUIT, "indexing"],
    );
    nudge(&app, src_selected);
    shot(&app, NARROW);

    app.resize(120, 40);
    let restored = app.wait_screen("find results restored", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && painted(screen, 120, 40)
            && src_selected(screen)
    });
    see(
        &restored,
        &[CRUMB, PARENT, HIT, "1 result"],
        &[SITE, OPEN_OS],
    );

    app.key("delete");
    app.wait_screen("src query cleared", |screen| {
        text(screen).contains(TYPE_HINT) && text(screen).contains(CRUMB) && !src_selected(screen)
    });
    app.paste(ABSENT);
    let empty = app.wait_screen("no matches", |screen| {
        let visible = text(screen);
        visible.contains(NO_MATCH)
            && visible.contains("Esc clears the query")
            && visible.contains(CRUMB)
            && !visible.contains(NOTES)
    });
    see(
        &empty,
        &[NO_MATCH, "Esc clears the query", CRUMB],
        &[SITE, NOTES, BROWSE, OPEN_OS],
    );
    app.key("enter");
    let stayed = app.wait_screen("empty enter stays on find", |screen| {
        let visible = text(screen);
        visible.contains(EMPTY_STATUS)
            && visible.contains(NO_MATCH)
            && visible.contains(CRUMB)
            && !visible.contains(NOTES)
            && !visible.contains(BROWSE)
            && !visible.contains(OPEN_OS)
    });
    see(
        &stayed,
        &[EMPTY_STATUS, NO_MATCH, CRUMB],
        &[NOTES, BROWSE, OPEN_OS, SITE, "todo.md"],
    );

    app.key("delete");
    app.wait_screen("absent query cleared", |screen| {
        text(screen).contains(TYPE_HINT) && text(screen).contains(CRUMB)
    });
    let again = search_src(&app);
    see(&again, &[PARENT, "1 result"], &[SITE, OPEN_OS]);

    app.key("enter");
    let menu = app.wait_screen("file actions", |screen| {
        let visible = text(screen);
        visible.contains(OPEN_OS)
            && visible.contains(REVEAL)
            && visible.contains(COPY)
            && visible.contains(ANALYZE)
            && visible.contains(PREVIEW)
            && visible.contains(FULL)
            && visible.contains(CRUMB)
    });
    see(
        &menu,
        &[OPEN_OS, REVEAL, COPY, ANALYZE, PREVIEW, FULL, CRUMB],
        &[NOTES, BROWSE, QUIT],
    );

    app.key("end");
    app.key("enter");
    let opened = app.wait_screen("previewed directory", |screen| {
        let visible = text(screen);
        visible.contains(BROWSE)
            && visible.contains(BROWSE_CRUMB)
            && visible.contains("main.rs")
            && !visible.contains(CRUMB)
            && !visible.contains(NOTES)
            && !visible.contains(OPEN_OS)
    });
    see(
        &opened,
        &[BROWSE, BROWSE_CRUMB, "main.rs"],
        &[
            CRUMB, NOTES, OPEN_OS, REVEAL, COPY, ANALYZE, PREVIEW, "todo.md", SITE,
        ],
    );
    app.finish();
}
