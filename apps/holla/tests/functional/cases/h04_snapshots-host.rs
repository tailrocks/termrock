//! Holla host snapshots, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h04_snapshots_host/` and each checkpoint's dots become
//! underscores. `first-use` is the shipped scenario that can open every host
//! family: `system_pressure` listens on 5173, `disk_full` has cleanup history,
//! and `ssh_personal` resolves `devbox`. The shots are the open system page.
//! Reduced motion still paints the press flash, and a same-size resize does
//! not force a frame, so each shot nudges the width and comes back.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, cell_bg, find_all, row_text, text, try_launch};

const SYSTEM: &str = "Show system resources";
const PROCESSES: &str = "Show process tree";
const PORT: &str = "Find the process on a port…";
/// The finder label column cuts this row before the ellipsis.
const PORT_ROW: &str = "Find the process on a po";
const HISTORY: &str = "Show cleanup history";
const CONNECT: &str = "Connect to devbox";
const RESOLVE: &str = "Show the resolved configuration";
const SYSTEM_TITLE: &str = "System resources";
const PROCESS_TITLE: &str = "Process tree";
const PORT_TITLE: &str = "Port 5173";
const LSOF: &str = "$ lsof -nP -iTCP:5173 -sTCP:LISTEN";
const HISTORY_TITLE: &str = "Cleanup history";
const HISTORY_ROW: &str = "~/work/legacy/node_modules";
const SSH_TITLE: &str = "ssh devbox · resolved";
const SSH_MARK: &str = "ssh -G";
const MEMORY: &str = "12.4 of 16 GB";
const FINDER: &str = "Search actions and resources";
const WIDE: &str = "H04.snapshots-host.host.120x40";
const NARROW: &str = "H04.snapshots-host.host.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h04_snapshots_host/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = ["--scenario", "first-use"]
        .into_iter()
        .map(str::to_string)
        .collect();
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    let finder = app.wait_text(FINDER);
    assert_eq!((finder.cols(), finder.rows()), (120, 40));
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

fn menu_point(screen: &TerminalSnapshot, label: &str) -> crate::support::Point {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| row_text(screen, u32::from(hit.y)).contains('│'))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "menu hit {label:?}: {hits:?}\n{}",
        text(screen)
    );
    hits[0]
}

/// The alternatives row paints a fill and no gutter. The focused item is the
/// one whose background differs from every other action row.
fn menu_focused(screen: &TerminalSnapshot, label: &str) -> bool {
    let focus = cell_bg(screen, menu_point(screen, label));
    let rest: Vec<_> = [
        "Run now",
        "Copy command",
        "Insert into shell",
        RESOLVE,
        "Pin here",
    ]
    .into_iter()
    .filter(|other| *other != label)
    .map(|other| cell_bg(screen, menu_point(screen, other)))
    .collect();
    rest.iter().all(|bg| *bg != focus) && rest.windows(2).all(|pair| pair[0] == pair[1])
}

fn query_empty(screen: &TerminalSnapshot) -> bool {
    text(screen).contains(FINDER) && !text(screen).contains("snapshot · read-only")
}

fn clear_query(app: &App) {
    for _ in 0..3 {
        if query_empty(&app.screen()) {
            return;
        }
        app.key("escape");
    }
    app.wait_screen("finder query cleared", query_empty);
}

fn search(app: &App, query: &str, row: &str) {
    clear_query(app);
    app.paste(query);
    app.wait_screen("row selected", |screen| {
        selected(screen, row) && text(screen).contains(query)
    });
}

fn system_drawn(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(SYSTEM_TITLE)
        && visible.contains("pid")
        && visible.contains("rust-analyzer")
        && visible.contains("Open btm")
}

fn open_system(app: &App) -> TerminalSnapshot {
    search(app, SYSTEM, SYSTEM);
    app.key("enter");
    let page = app.wait_screen("system snapshot", system_drawn);
    assert_system(&page);
    page
}

fn assert_system(screen: &TerminalSnapshot) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (120, 40),
        "{}",
        text(screen)
    );
    see(
        screen,
        &[SYSTEM_TITLE, "pid", MEMORY, "CPU", "Open btm"],
        &[
            PROCESS_TITLE,
            HISTORY_TITLE,
            LSOF,
            SSH_MARK,
            "btm 0.11.1",
            "Enter attaches",
        ],
    );
}

fn assert_processes(screen: &TerminalSnapshot) {
    see(
        screen,
        &[PROCESS_TITLE, "pid", "cargo"],
        &[SYSTEM_TITLE, HISTORY_TITLE, LSOF, SSH_MARK],
    );
}

fn assert_port(screen: &TerminalSnapshot) {
    see(
        screen,
        &[PORT_TITLE, LSOF, "node (vite)"],
        &[
            "nothing listens",
            HISTORY_TITLE,
            SSH_MARK,
            SYSTEM_TITLE,
            "· arguments",
        ],
    );
}

fn assert_history(screen: &TerminalSnapshot) {
    see(
        screen,
        &[HISTORY_TITLE, HISTORY_ROW, "3.9 GB", "Trash"],
        &[SYSTEM_TITLE, LSOF, SSH_MARK, PORT_TITLE],
    );
}

fn assert_ssh(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            SSH_TITLE,
            SSH_MARK,
            "$ ssh -G devbox",
            "alex@devbox.internal:22",
            "hostname devbox.internal",
        ],
        &[LSOF, HISTORY_TITLE, SYSTEM_TITLE, "Enter attaches"],
    );
}

/// The waits follow the reported size and the system title, not the size chip.
fn settle_system(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        text(&screen).contains(SYSTEM_TITLE),
        "shot left the system snapshot\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(SYSTEM_TITLE)
    });
    app.resize(cols, rows);
    app.wait_screen("restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(SYSTEM_TITLE)
    });
}

fn shot(app: &App, checkpoint: &str) {
    settle_system(app);
    app.visual(&visual_id(checkpoint));
}

fn back(app: &App) {
    app.key("escape");
    // A query replaces the empty-field placeholder, so the finder hint is the signal.
    app.wait_screen("back on the finder", |screen| {
        text(screen).contains("Type Search") && !text(screen).contains("snapshot · read-only")
    });
}

fn open_port(app: &App) -> TerminalSnapshot {
    search(app, PORT, PORT_ROW);
    app.key("enter");
    let form = app.wait_screen("port arguments", |screen| {
        let visible = text(screen);
        visible.contains("· arguments") && visible.contains("lsof -nP '-iTCP:5173'")
    });
    see(
        &form,
        &[PORT, "5173", "lsof -nP '-iTCP:5173'"],
        &[PORT_TITLE, LSOF, "uid="],
    );
    app.key("ctrl+s");
    let page = app.wait_screen("port snapshot", |screen| {
        let visible = text(screen);
        visible.contains(PORT_TITLE) && visible.contains(LSOF)
    });
    assert_port(&page);
    page
}

fn open_resolved(app: &App) -> TerminalSnapshot {
    search(app, CONNECT, CONNECT);
    app.key("alt+enter");
    let menu = app.wait_text(RESOLVE);
    see(
        &menu,
        &[RESOLVE, "Run now", "Pin here", CONNECT],
        &[SSH_TITLE, "Enter attaches"],
    );
    assert!(
        menu_focused(&menu, "Run now"),
        "Connect was not the focused alternative\n{}",
        text(&menu)
    );
    assert!(
        !menu_focused(&menu, RESOLVE),
        "the resolve row started focused\n{}",
        text(&menu)
    );
    app.key("down");
    app.wait_screen("copy command focused", |screen| {
        menu_focused(screen, "Copy command")
    });
    app.key("down");
    app.wait_screen("insert focused", |screen| {
        menu_focused(screen, "Insert into shell")
    });
    app.key("down");
    let chosen = app.wait_screen("resolved configuration focused", |screen| {
        menu_focused(screen, RESOLVE)
    });
    assert!(
        !menu_focused(&chosen, "Run now"),
        "Run now stayed focused\n{}",
        text(&chosen)
    );
    app.key("enter");
    let page = app.wait_text(SSH_TITLE);
    assert_ssh(&page);
    page
}

fn quit(app: &App) {
    app.key("ctrl+q");
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut confirmed = false;
    while app.child_running() {
        let visible = text(&app.screen());
        if visible.contains("Quit holla") && !confirmed {
            let stay = visible
                .lines()
                .any(|line| line.contains('▎') && line.contains("Stay"));
            let confirm = visible.lines().any(|line| {
                line.contains('▎') && (line.contains("Stop and quit") || line.contains("Quit"))
            });
            assert!(
                stay || confirm,
                "quit dialog focused control was not Stay or the confirm button\n{visible}"
            );
            if stay {
                app.key("right");
                app.wait_screen("quit confirm focused", |screen| {
                    text(screen).lines().any(|line| {
                        line.contains('▎')
                            && !line.contains("Stay")
                            && (line.contains("Stop and quit") || line.contains("Quit"))
                    })
                });
            }
            confirmed = true;
            app.key("enter");
        }
        assert!(
            Instant::now() < deadline,
            "quit did not exit\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

#[test]
fn h04_snapshots_host_route() {
    let app = open("h04-snapshots-host-route");
    open_system(&app);
    app.key("escape");
    let back = app.wait_screen("finder keeps the system row", |screen| {
        selected(screen, SYSTEM)
            && text(screen).contains("Type Search")
            && text(screen).contains(SYSTEM)
            && !text(screen).contains("snapshot · read-only")
    });
    assert_eq!((back.cols(), back.rows()), (120, 40));
    see(
        &back,
        &[SYSTEM, "Type Search", "Results · 1"],
        &[
            "snapshot · read-only",
            LSOF,
            HISTORY_TITLE,
            SSH_TITLE,
            "Enter attaches",
        ],
    );
    assert!(
        selected(&back, SYSTEM),
        "the system row lost selection\n{}",
        text(&back)
    );
    quit(&app);
}

#[test]
fn h04_snapshots_host_host() {
    let app = open("h04-snapshots-host-host");
    let system = open_system(&app);
    assert_eq!((system.cols(), system.rows()), (120, 40));
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("system at 80x24", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(SYSTEM_TITLE)
            && visible.contains("pid")
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(
        &narrowed,
        &[SYSTEM_TITLE, "pid"],
        &[HISTORY_TITLE, LSOF, SSH_MARK, "Enter attaches"],
    );
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("system back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && system_drawn(screen)
    });
    assert_system(&restored);

    back(&app);
    search(&app, PROCESSES, PROCESSES);
    app.key("enter");
    let processes = app.wait_screen("process tree", |screen| {
        let visible = text(screen);
        visible.contains(PROCESS_TITLE) && visible.contains("pid") && visible.contains("cargo")
    });
    assert_processes(&processes);
    back(&app);

    open_port(&app);
    back(&app);

    search(&app, HISTORY, HISTORY);
    app.key("enter");
    let history = app.wait_screen("cleanup history", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY_TITLE) && visible.contains(HISTORY_ROW)
    });
    assert_history(&history);
    back(&app);

    open_resolved(&app);
    back(&app);
    let finder = app.screen();
    see(
        &finder,
        &[CONNECT, "Type Search"],
        &[SSH_TITLE, LSOF, "Enter attaches", "snapshot · read-only"],
    );
    quit(&app);
}
