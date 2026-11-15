//! Holla configuration reload and trust revocation, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h02_reload/` and each checkpoint's dots become underscores.
//!
//! `Go::Reload` (`src/bin/holla/app.rs`) re-parses the same simulated file and
//! has no error path. `TrustStore::save_failure` is only read from `approve`
//! (`src/bin/holla/domain/custom.rs`), and this scenario never sets it. The
//! drawn re-read is the status line. The action ids do not change, and nothing
//! starts. The header age is `ago(now - 1s)`, so it stays `1 s ago` without
//! `--frame`.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const QUERY: &str = "Custom action configuration";
// The 120-column finder cuts the label. The drawn prefix is what `›` marks.
const ROW: &str = "Custom action configurat";
const TITLE: &str = "Custom actions";
const DEPLOY: &str = "deploy.preview";
const WIPE: &str = "wipe";
const MULTI: &str = "multi";
const MOVED: &str = "Moved";
const UNTRUSTED: &str = "Untrusted";
const SHARED: &str = "id \"shared\" is already defined by the global configuration";
const BAD_ID: &str = "invalid id \"Bad Id\"";
const REREAD: &str = "Re-reading the configuration files · trust is re-checked by digest";
// At 80 columns the footer and the detail pane cut these with an ellipsis.
const REREAD_MARK: &str = "Re-reading the configuration files";
const SHARED_MARK: &str = "id \"shared\"";
const REVOKED: &str = "Trust revoked · the file must be reviewed again before anything runs";
const FILE: &str = "~/work/team/.holla.toml";
const WIDE: &str = "H02.reload.reload.120x40";
const NARROW: &str = "H02.reload.reload.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h02_reload/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec![
        "--scenario".to_string(),
        "parity-custom-actions".to_string(),
    ];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions");
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

fn selected(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('›') else {
            return false;
        };
        let Some(at) = line.find(label) else {
            return false;
        };
        at > mark && at - mark <= 4
    })
}

fn same_actions(visible: &str) -> bool {
    visible.contains(TITLE)
        && visible.contains(DEPLOY)
        && visible.contains(WIPE)
        && visible.contains(MULTI)
        && visible.contains(SHARED_MARK)
        && visible.contains(BAD_ID)
        && visible.contains(FILE)
        && !visible.contains(" running")
        && !visible.contains("discovering")
        && !visible.contains(QUERY)
}

/// The status bar paints `· {cols}×{rows}` from the frame Holla just drew.
/// The PTY grid changes first, and the previous frame still matches `reloaded`.
fn painted(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    text(screen).contains(&format!("· {cols}×{rows}"))
}

fn reloaded(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    same_actions(&visible)
        && visible.contains(REREAD_MARK)
        && visible.contains(MOVED)
        && !visible.contains(REVOKED)
}

fn revoked(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    same_actions(&visible)
        && visible.contains(REVOKED)
        && visible.contains(UNTRUSTED)
        && !visible.contains(MOVED)
        && !visible.contains(REREAD)
}

fn open_config(app: &App) -> TerminalSnapshot {
    app.paste(QUERY);
    let row = app.wait_screen("config row selected", |screen| {
        selected(screen, ROW) && text(screen).contains("Results · 1")
    });
    assert_eq!((row.cols(), row.rows()), (120, 40));
    see(
        &row,
        &[QUERY, ROW, "Results · 1"],
        &[TITLE, REREAD, REVOKED],
    );
    app.key("enter");
    let page = app.wait_screen("custom actions open", |screen| {
        let visible = text(screen);
        same_actions(&visible) && visible.contains(MOVED) && !visible.contains(REREAD)
    });
    see(
        &page,
        &[TITLE, DEPLOY, WIPE, MULTI, MOVED, SHARED, BAD_ID, FILE],
        &[REREAD, REVOKED, QUERY, " running", "discovering"],
    );
    page
}

fn back_to_finder(app: &App) {
    app.key("escape");
    // Esc paints the finder list while the file preview still holds
    // `id = "wipe"`. The restored row is the frame after that preview closes.
    let finder = app.wait_screen("finder row restored", |screen| {
        let visible = text(screen);
        selected(screen, ROW)
            && visible.contains(QUERY)
            && visible.contains("Results · 1")
            && !visible.contains(TITLE)
            && !visible.contains(DEPLOY)
            && !visible.contains(WIPE)
            && !visible.contains(MULTI)
            && !visible.contains(REREAD)
            && !visible.contains("discovering")
    });
    see(
        &finder,
        &[QUERY, ROW, "Results · 1"],
        &[TITLE, DEPLOY, WIPE, MULTI, REREAD, "discovering"],
    );
    assert!(
        selected(&finder, ROW),
        "finder left the row that opened the page\n{}",
        text(&finder)
    );
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back. The config page stays open.
fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && painted(screen, u32::from(nudged), u32::from(rows))
            && reloaded(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && painted(screen, u32::from(cols), u32::from(rows))
            && reloaded(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

#[test]
fn holla_reload_route_returns_to_the_finder() {
    let app = launch("h02-reload-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Search actions", "~/work/team"],
        &["discovering", TITLE, REREAD],
    );
    open_config(&app);
    back_to_finder(&app);
    app.finish();
}

#[test]
fn holla_reload_rereads_and_revokes_without_running() {
    let app = launch("h02-reload-reload");
    open_config(&app);
    app.key("r");
    let reread = app.wait_screen("configuration re-read", reloaded);
    assert_eq!((reread.cols(), reread.rows()), (120, 40));
    see(
        &reread,
        &[REREAD, TITLE, DEPLOY, WIPE, MULTI, MOVED, SHARED, BAD_ID],
        &[REVOKED, QUERY, " running", "discovering"],
    );
    nudge(&app);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("config at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && painted(screen, 80, 24) && reloaded(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(
        &narrowed,
        &[
            REREAD_MARK,
            TITLE,
            DEPLOY,
            WIPE,
            MULTI,
            MOVED,
            SHARED_MARK,
            BAD_ID,
        ],
        &[REVOKED, " running"],
    );
    nudge(&app);
    shot(&app, NARROW);
    app.resize(120, 40);

    let restored = app.wait_screen("config back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && painted(screen, 120, 40) && reloaded(screen)
    });
    see(&restored, &[REREAD, DEPLOY, MOVED], &[REVOKED, " running"]);

    app.key("u");
    let revoked_page = app.wait_screen("trust revoked", revoked);
    assert_eq!((revoked_page.cols(), revoked_page.rows()), (120, 40));
    see(
        &revoked_page,
        &[
            REVOKED, TITLE, DEPLOY, WIPE, MULTI, UNTRUSTED, SHARED, BAD_ID,
        ],
        &[REREAD, MOVED, QUERY, " running", "discovering"],
    );
    back_to_finder(&app);
    app.finish();
}
