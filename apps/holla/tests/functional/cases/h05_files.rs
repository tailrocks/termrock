//! Holla file navigation, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h05_files/` and each checkpoint's dots become underscores.
//! The checkpoint is the open `/Users/alex/work/app` directory, not the
//! `link-to-app` symlink. `FilesPage::open_current` follows a symlink to its
//! target (`src/bin/holla/screens/files.rs`), so that row is the wrong one.
//! A same-size resize does not force a frame, and the 140ms press flash stays
//! painted under reduced motion, so each shot nudges the width and comes back.
//! The waits follow the reported size and `Files · ~/work/app`, not the
//! status-bar size text.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const NOTES_LABEL: &str = "Browse ~/work/notes";
const SITE_LABEL: &str = "Browse ~/work/site";
const NOTES: &str = "~/work/notes";
const WORK: &str = "~/work";
const APP: &str = "~/work/app";
const SITE: &str = "~/work/site";
const ROOT: &str = "/";
const HOME: &str = "~";
const WIDE: &str = "H05.files.nav.120x40";
const NARROW: &str = "H05.files.nav.narrow";
const ROOT_STATUS: &str = "Already at the root";
const QUIT_DIALOG: &str = "Quit holla";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h05_files/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Type Search");
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

/// Browser directory from the page title `Files · {path}` (`FilesPage::render`).
fn title_path(screen: &TerminalSnapshot) -> String {
    let marker = "Files · ";
    for line in text(screen).lines() {
        if let Some(rest) = line.split(marker).nth(1) {
            return rest.split_whitespace().next().unwrap_or("").to_string();
        }
    }
    String::new()
}

/// Browser directory from the page status `{path} · hidden off`.
/// The shell chip stays on the scenario cwd, so this is the segment before
/// ` · hidden`, not the leftmost path.
fn status_path(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let Some(line) = visible.lines().find(|line| line.contains(" · hidden")) else {
        return String::new();
    };
    let Some(at) = line.find(" · hidden") else {
        return String::new();
    };
    line[..at]
        .split_whitespace()
        .next_back()
        .unwrap_or("")
        .to_string()
}

fn browser_path(screen: &TerminalSnapshot) -> String {
    let title = title_path(screen);
    let status = status_path(screen);
    assert_eq!(title, status, "title and status disagree\n{}", text(screen));
    title
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn entry_name(line: &str) -> String {
    let Some(at) = line.find('▎') else {
        return String::new();
    };
    let body = line[at + '▎'.len_utf8()..].trim_start();
    let body = body.trim_start_matches(['▸', '→', '◇']).trim_start();
    body.split_whitespace().next().unwrap_or("").to_string()
}

fn finder_row(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains('›') && !line.contains("scope ‹"))
        .unwrap_or("")
        .to_string()
}

fn at_directory(screen: &TerminalSnapshot, path: &str) -> bool {
    title_path(screen) == path && status_path(screen) == path
}

fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        at_directory(&screen, APP),
        "shot left the app directory\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && at_directory(screen, APP)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && at_directory(screen, APP)
    });
}

fn shot(app: &App, checkpoint: &str) {
    nudge(app);
    app.visual(&visual_id(checkpoint));
}

fn query_has(screen: &TerminalSnapshot, label: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains('▎') && line.contains(label)
    })
}

fn paste_browse(app: &App, label: &str) {
    app.paste(label);
    app.wait_screen("browse row", |screen| {
        query_has(screen, label) && finder_row(screen).contains(label)
    });
    app.key("enter");
}

fn open_notes(app: &App) -> TerminalSnapshot {
    paste_browse(app, NOTES_LABEL);
    let page = app.wait_screen("notes directory", |screen| at_directory(screen, NOTES));
    assert_notes(&page);
    page
}

fn open_site(app: &App) -> TerminalSnapshot {
    paste_browse(app, SITE_LABEL);
    let page = app.wait_screen("site directory", |screen| at_directory(screen, SITE));
    assert_site(&page);
    page
}

fn assert_notes(screen: &TerminalSnapshot) {
    assert_eq!(browser_path(screen), NOTES, "{}", text(screen));
    see(
        screen,
        &["todo.md", "README.md", "2 entries"],
        &[SITE, "index.html", "assets", QUIT_DIALOG],
    );
}

fn assert_work(screen: &TerminalSnapshot) {
    assert_eq!(browser_path(screen), WORK, "{}", text(screen));
    assert_ne!(browser_path(screen), ROOT);
    assert_ne!(browser_path(screen), SITE);
    assert_ne!(browser_path(screen), NOTES);
    see(
        screen,
        &["app", "notes", "link-to-app"],
        &[SITE, "index.html", "todo.md", QUIT_DIALOG],
    );
}

fn assert_app(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "{}",
        text(screen)
    );
    assert_eq!(browser_path(screen), APP, "{}", text(screen));
    see(
        screen,
        &["README.md", "src"],
        &["index.html", "todo.md", "link-to-app", SITE, QUIT_DIALOG],
    );
}

fn assert_site(screen: &TerminalSnapshot) {
    assert_eq!(browser_path(screen), SITE, "{}", text(screen));
    see(
        screen,
        &["index.html", "assets", "16 entries"],
        &[
            "todo.md",
            "/Users/alex/work/notes",
            NOTES,
            ".env",
            QUIT_DIALOG,
        ],
    );
}

fn assert_named(screen: &TerminalSnapshot, name: &str) {
    let line = focused_line(screen);
    assert_eq!(
        entry_name(&line),
        name,
        "focused row is not {name}\n{line}\n{}",
        text(screen)
    );
}

fn select_named(app: &App, name: &str) {
    let started = entry_name(&focused_line(&app.screen()));
    app.key("home");
    let mut screen = app.wait_screen("home", |screen| {
        let got = entry_name(&focused_line(screen));
        !got.is_empty() && (got == name || got != started)
    });
    for step in 0..16 {
        let got = entry_name(&focused_line(&screen));
        if got == name {
            let line = focused_line(&screen);
            assert!(
                !line.contains("link-to-app"),
                "selected the symlink\n{line}"
            );
            assert!(
                line.contains("directory"),
                "selected row is not a directory\n{line}"
            );
            return;
        }
        app.key("down");
        let previous = got;
        screen = app.wait_screen(&format!("down toward {name} {step}"), |screen| {
            let got = entry_name(&focused_line(screen));
            !got.is_empty() && got != previous
        });
    }
    panic!("never selected {name}\n{}", text(&app.screen()));
}

fn go_parent(app: &App, key: &str, from: &str) -> TerminalSnapshot {
    app.key(key);
    app.wait_screen(key, |screen| {
        let path = title_path(screen);
        !path.is_empty() && path != from && status_path(screen) == path
    })
}

fn quiet_exit(app: &App) {
    assert!(
        !text(&app.screen()).contains(QUIT_DIALOG),
        "quit dialog before Ctrl+Q\n{}",
        text(&app.screen())
    );
    app.finish();
}

#[test]
fn h05_files_route() {
    let app = open("h05-files-route", "parity-files");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[NOTES, "mbp", "Type Search"],
        &[SITE, "index.html", QUIT_DIALOG],
    );
    let notes = open_notes(&app);
    assert_eq!((notes.cols(), notes.rows()), (120, 40));
    app.key("esc");
    let finder = app.wait_screen("finder after files", |screen| {
        text(screen).contains("Type Search")
            && finder_row(screen).contains(NOTES_LABEL)
            && title_path(screen).is_empty()
            && text(screen).contains(NOTES)
    });
    see(
        &finder,
        &[NOTES_LABEL, NOTES, "mbp"],
        &["Files · ", "todo.md", SITE, QUIT_DIALOG],
    );
    quiet_exit(&app);
}

#[test]
fn h05_files_nav() {
    let app = open("h05-files-nav", "parity-files");
    open_notes(&app);
    let work = go_parent(&app, "left", NOTES);
    assert_work(&work);
    assert_named(&work, "notes");
    select_named(&app, "app");
    app.key("enter");
    let wide = app.wait_screen("app directory", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && at_directory(screen, APP)
    });
    assert_app(&wide, 120, 40);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("app at 80 columns", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && at_directory(screen, APP)
    });
    assert_app(&narrow, 80, 24);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("app back at 120 columns", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && at_directory(screen, APP)
    });
    assert_app(&restored, 120, 40);
    let parent = go_parent(&app, "left", APP);
    assert_work(&parent);
    assert_named(&parent, "app");
    let mut path = browser_path(&parent);
    for _ in 0..6 {
        if path == ROOT {
            break;
        }
        let next = go_parent(&app, "left", &path);
        path = browser_path(&next);
        assert_ne!(path, SITE, "parent jumped to site\n{}", text(&next));
        assert_ne!(path, NOTES, "parent jumped to notes\n{}", text(&next));
        assert_ne!(path, APP, "parent jumped back to app\n{}", text(&next));
    }
    assert_eq!(path, ROOT, "stopped at {path}\n{}", text(&app.screen()));
    app.key("backspace");
    let rooted = app.wait_screen("root boundary", |screen| {
        text(screen).contains(ROOT_STATUS) && at_directory(screen, ROOT)
    });
    assert_eq!(browser_path(&rooted), ROOT);
    assert_ne!(browser_path(&rooted), HOME);
    assert_ne!(browser_path(&rooted), NOTES);
    see(&rooted, &[ROOT_STATUS], &[SITE, "todo.md", QUIT_DIALOG]);
    quiet_exit(&app);
}

#[test]
fn h05_files_scenario_parity_files() {
    let app = open("h05-files-scenario-files", "parity-files");
    let landing = app.screen();
    see(&landing, &[NOTES, "mbp"], &[SITE, "index.html", "todo.md"]);
    let notes = open_notes(&app);
    assert_eq!((notes.cols(), notes.rows()), (120, 40));
    assert_notes(&notes);
    quiet_exit(&app);
}

#[test]
fn h05_files_scenario_parity_browser() {
    let app = open("h05-files-scenario-browser", "parity-browser");
    let landing = app.screen();
    see(
        &landing,
        &[SITE, "mbp"],
        &["todo.md", "/Users/alex/work/notes", NOTES, "index.html"],
    );
    let site = open_site(&app);
    assert_eq!((site.cols(), site.rows()), (120, 40));
    assert_site(&site);
    quiet_exit(&app);
}
