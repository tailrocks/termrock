//! Holla frecency before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h01_memory/` and each checkpoint's dots become underscores.
//!
//! `--frame 800` is only the binary pre-tick (`app.rs`). Reduced motion leaves
//! the clock running. Freshness age is `tick * 80ms`, and 800 ticks is 64s,
//! inside the one-minute label (`action.rs`), so a later redraw keeps it.
//!
//! `parity.rs` learns `pull` for `git.fetch` and `Ünï  Code` for `cargo.clippy`.
//! `ranking.rs` drops an item that misses the query words before the learned
//! boost, and neither label matches, so the PTY never draws `remembered for`.
//! The drawn difference is the seeded frecency. `seed` keeps at most 20 stamps
//! (`usage.rs`), so the 25-use `git.pull` row reads `used 20 times here`.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, text, try_launch};

const QUERY: &str = "pull";
const SEEDED: &str = "used 20 times here";
const RECENT: &str = "everything used here is suggested above";
const EMPTY: &str = "nothing used here yet";
const RESULTS: &str = "Results · 3";
const PLACEHOLDER: &str = "Search actions and resources";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h01_memory/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, history: bool) -> App {
    let mut launch = Launch::new(name, crate::support::HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = history;
    launch.args = vec![
        "--scenario".to_string(),
        "parity-history".to_string(),
        "--frame".to_string(),
        "800".to_string(),
    ];
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
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

fn before(screen: &TerminalSnapshot, earlier: &str, later: &str) {
    let visible = text(screen);
    let left = visible
        .find(earlier)
        .unwrap_or_else(|| panic!("missing {earlier:?}\n{visible}"));
    let right = visible
        .find(later)
        .unwrap_or_else(|| panic!("missing {later:?}\n{visible}"));
    assert!(
        left < right,
        "{earlier:?} should precede {later:?}\n{visible}"
    );
}

fn assert_seeded(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    see(
        screen,
        &[
            "Here",
            "Suggested here",
            "cargo build",
            "cargo test",
            "Pull",
            "cargo check",
            "used 3 times here",
            SEEDED,
            "used once here",
            "Recent here",
            RECENT,
            "~/work/holla",
            "mbp · local",
        ],
        &[
            EMPTY,
            "Run shots",
            "remembered for",
            "history not saved",
            "frecency.json",
            "gone.action",
            "system.resources",
        ],
    );
    before(screen, "cargo build", "cargo test");
    before(screen, "cargo test", "Pull");
    before(screen, "Pull", "cargo check");
    before(screen, "cargo check", "Run check");
}

fn assert_unseeded(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            PLACEHOLDER,
            "Suggested here",
            "Run check",
            "Run fmt",
            "Recent here",
            EMPTY,
            "~/work/holla",
        ],
        &[SEEDED, "used 3 times here", RECENT, "history not saved"],
    );
    before(screen, "Run check", "Run fmt");
    before(screen, "Run fmt", "nothing used here yet");
}

fn assert_foreign(screen: &TerminalSnapshot) {
    see(
        screen,
        &["holla · Rust project", "mbp · local"],
        &[
            "Clean Docker completely",
            "prod-eu-1",
            "From acme",
            "index.html",
            "café",
            "devbox",
        ],
    );
}

/// A same-size resize does not force a frame, and the freshness cell is part
/// of the hash (`cursor.col` is too). Nudge the width and come back. The
/// narrow footer may omit a size chip, so this waits on the terminal size.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged width", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
    });
    app.resize(cols, rows);
    app.wait_screen("restored size", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn search(app: &App, history: bool) -> TerminalSnapshot {
    app.paste(QUERY);
    app.wait_screen("pull search settled", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains(RESULTS)
            && !visible.contains(PLACEHOLDER)
            && visible.contains(SEEDED) == history
    })
}

#[test]
fn route_opens_memory_finder_and_quits() {
    let app = launch("h01-memory-route", true);
    // The placeholder paints before the seeded Recent section. The route
    // checkpoint is that section, not the first search field.
    let finder = app.wait_screen("seeded finder", |screen| {
        let visible = text(screen);
        visible.contains(PLACEHOLDER) && visible.contains("Recent here")
    });
    assert_eq!((finder.cols(), finder.rows()), (120, 40));
    see(
        &finder,
        &["Here", "Recent here", "~/work/holla", "mbp"],
        &["history not saved", "frecency.json"],
    );
    app.finish();
}

#[test]
fn memory_search_keeps_the_query() {
    let on = launch("h01-memory-on", true);
    let seeded = on.wait_text(RECENT);
    assert_seeded(&seeded);

    let found = search(&on, true);
    see(
        &found,
        &[QUERY, SEEDED, RESULTS, "Pull"],
        &[
            PLACEHOLDER,
            EMPTY,
            "history not saved",
            "frecency.json",
            "remembered for",
        ],
    );

    let off = launch("h01-memory-off", false);
    let empty = off.wait_text(EMPTY);
    assert_unseeded(&empty);
    let plain = search(&off, false);
    see(
        &plain,
        &[QUERY, RESULTS, "up to date", "Pull"],
        &[
            PLACEHOLDER,
            SEEDED,
            "used 3 times here",
            RECENT,
            "history not saved",
            "frecency.json",
        ],
    );
    off.finish();

    flash_settle(&on);
    shot(&on, "H01.memory.memory.120x40");

    on.resize(80, 24);
    let narrow = on.wait_screen("pull search at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(QUERY)
            && text(screen).contains(SEEDED)
            && text(screen).contains(RESULTS)
            && !text(screen).contains(PLACEHOLDER)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(&narrow, &[QUERY, SEEDED, RESULTS], &[PLACEHOLDER, EMPTY]);
    flash_settle(&on);
    shot(&on, "H01.memory.memory.narrow");
    on.resize(120, 40);
    let restored = on.wait_screen("pull search back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(QUERY)
            && text(screen).contains(SEEDED)
            && !text(screen).contains(PLACEHOLDER)
    });
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    see(
        &restored,
        &[QUERY, SEEDED],
        &[PLACEHOLDER, "history not saved"],
    );
    on.finish();
}

#[test]
fn scenario_shows_parity_history_memory() {
    let app = launch("h01-memory-scenario", true);
    let finder = app.wait_text(RECENT);
    assert_seeded(&finder);
    assert_foreign(&finder);
    app.finish();
}
