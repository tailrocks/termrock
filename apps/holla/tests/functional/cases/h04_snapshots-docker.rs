//! Holla Docker snapshots, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h04_snapshots_docker/` and each checkpoint's dots become
//! underscores. The checkpoint is the open `docker-unavailable` page. A
//! same-size resize does not force a frame, so each shot nudges the width and
//! comes back. The PTY grid changes before Holla paints, so each wait also
//! requires the status-bar size from that paint.
//!
//! `docker-cleanup` starts with the query `docker clean` selected
//! (`App::start` in `src/bin/holla/app.rs`). Esc clears a selection before it
//! clears the text (`FinderPage` in `src/bin/holla/screens/finder.rs`).

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const SOCK: &str = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock";
const INFO: &str = "$ docker info";
const UNREACHABLE: &str = "daemon unreachable";
const CLEAN: &str = "Clean Docker completely";
const LIST: &str = "List containers";
const DISK: &str = "Show Docker disk usage";
const COMPOSE: &str = "Show Compose services";
const DOCKER: &str = "Docker";
const PLACEHOLDER: &str = "Search actions and resources";
const WIDE: &str = "H04.snapshots-docker.docker.120x40";
const NARROW: &str = "H04.snapshots-docker.docker.narrow";

const NAMES: &[&str] = &[
    "acme-api-1",
    "acme-worker-1",
    "acme-scheduler-1",
    "acme-db-1",
    "acme-redis-1",
    "build-cache-7f2",
    "registry-mirror",
    "tmp-shell ",
    "tmp-shell-2",
    "pgadmin",
    "grafana",
    "loki",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h04_snapshots_docker/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = ["--scenario", scenario]
        .into_iter()
        .map(str::to_string)
        .collect();
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

fn line_with(screen: &TerminalSnapshot, needle: &str) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or("")
        .to_string()
}

fn query_line(screen: &TerminalSnapshot) -> String {
    line_with(screen, "scope ‹")
}

fn selected(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains('›') && !line.contains("scope ‹"))
        .unwrap_or("")
        .to_string()
}

fn live_line(screen: &TerminalSnapshot) -> String {
    line_with(screen, "· live ·")
}

fn unavailable(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("docker.sock") || visible.contains(INFO)
}

/// The status bar paints `· {cols}×{rows}` from the frame Holla just drew.
fn painted(screen: &TerminalSnapshot, cols: u16, rows: u16) -> bool {
    text(screen).contains(&format!("· {cols}×{rows}"))
}

fn containers_page(screen: &TerminalSnapshot) -> bool {
    let title = live_line(screen);
    title.contains("Containers")
        && !title.contains("Docker disk usage")
        && !title.contains("Compose ·")
        && NAMES.iter().all(|name| text(screen).contains(name))
}

fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        unavailable(&screen),
        "shot left the unavailable Docker page\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && painted(screen, nudged, rows)
            && unavailable(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && painted(screen, cols, rows)
            && unavailable(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    nudge(app);
    app.visual(&visual_id(checkpoint));
}

/// Two Esc presses: a fully selected query only drops the selection on the first.
fn clear_query(app: &App) {
    app.key("esc");
    app.key("esc");
    app.wait_text(PLACEHOLDER);
}

fn paste_row(app: &App, label: &str) {
    app.paste(label);
    app.wait_screen("row selected", |screen| {
        query_line(screen).contains(label) && selected(screen).contains(label)
    });
    app.key("enter");
}

fn back(app: &App) {
    app.key("esc");
    app.wait_text("Type Search");
}

fn assert_running_containers(screen: &TerminalSnapshot) {
    let title = live_line(screen);
    assert!(
        title.contains("Containers") && !title.contains("Compose ·"),
        "containers page title\n{title}\n{}",
        text(screen)
    );
    see(screen, NAMES, &[SOCK, UNREACHABLE, "docker.sock"]);
    assert!(
        row_text_has(screen, &["grafana", "monitoring"]),
        "grafana is not in project monitoring\n{}",
        text(screen)
    );
    assert!(
        row_text_has(screen, &["acme-api-1", "healthy", "acme"]),
        "acme-api-1 is not the running acme container\n{}",
        text(screen)
    );
    assert!(
        row_text_has(screen, &["build-cache-7f2", "exited"]),
        "build-cache-7f2 is not exited\n{}",
        text(screen)
    );
}

fn row_text_has(screen: &TerminalSnapshot, parts: &[&str]) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        parts.iter().all(|part| line.contains(part))
    })
}

fn assert_unavailable(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "{}",
        text(screen)
    );
    let title = live_line(screen);
    assert!(
        title.contains("Docker")
            && !title.contains("disk usage")
            && !title.contains("Containers")
            && !title.contains("Compose ·"),
        "unavailable title\n{title}\n{}",
        text(screen)
    );
    see(
        screen,
        &[INFO, SOCK, "Daemon", UNREACHABLE],
        &[
            CLEAN,
            "acme-api-1",
            "acme-worker-1",
            "grafana",
            "loki",
            "build-cache-7f2",
            "registry-mirror",
            "tmp-shell",
            "pgadmin",
            "healthy",
            "8080→8080",
        ],
    );
}

#[test]
fn h04_snapshots_docker_route() {
    let app = open("h04-snapshots-docker-route", "docker-cleanup");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[CLEAN, "devbox", "docker clean"],
        &[SOCK, UNREACHABLE],
    );
    clear_query(&app);
    paste_row(&app, LIST);
    let page = app.wait_screen("containers", containers_page);
    assert_running_containers(&page);
    back(&app);
    let finder = app.wait_screen("finder keeps the row", |screen| {
        text(screen).contains("Type Search")
            && selected(screen).contains(LIST)
            && query_line(screen).contains(LIST)
            && !text(screen).contains("acme/api:dev")
    });
    see(&finder, &[LIST, "devbox"], &[SOCK, "acme/api:dev"]);
    app.finish();
}

#[test]
fn h04_snapshots_docker_docker() {
    let app = open("h04-snapshots-docker-docker", "docker-cleanup");
    clear_query(&app);
    paste_row(&app, LIST);
    let containers = app.wait_screen("containers", containers_page);
    assert_running_containers(&containers);
    back(&app);
    clear_query(&app);
    paste_row(&app, DISK);
    let disk = app.wait_screen("disk usage", |screen| {
        let title = live_line(screen);
        title.contains("Docker disk usage") && !title.contains("Containers")
    });
    let disk_title = live_line(&disk);
    assert!(
        disk_title.contains("Docker disk usage") && !disk_title.contains("Containers"),
        "disk page is still titled Containers\n{disk_title}"
    );
    see(
        &disk,
        &["9.8 GB", "~18.4", "acme-api-1", "monitoring"],
        &[SOCK, UNREACHABLE, "docker.sock"],
    );
    back(&app);
    clear_query(&app);
    // docker_devbox leaves compose unset, so the catalog never adds this row.
    app.paste(COMPOSE);
    let unmatched = app.wait_text("No matches for");
    see(
        &unmatched,
        &[
            "No matches for “Show Compose services”",
            "Nothing to preview",
        ],
        &["Compose ·", SOCK, UNREACHABLE, CLEAN],
    );
    app.key("enter");
    let stayed = app.wait_screen("compose row is absent", |screen| {
        text(screen).contains("No matches for “Show Compose services”")
            && !text(screen).contains("Compose ·")
            && !text(screen).contains(INFO)
    });
    see(&stayed, &["Nothing to preview"], &[CLEAN, "acme-api-1"]);
    app.finish();

    let app = open("h04-snapshots-docker-unavailable", "parity-discovery");
    clear_query(&app);
    paste_row(&app, DOCKER);
    let wide = app.wait_screen("docker unavailable", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && painted(screen, 120, 40)
            && unavailable(screen)
    });
    assert_unavailable(&wide, 120, 40);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("unavailable at 80 columns", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && painted(screen, 80, 24) && unavailable(screen)
    });
    assert_unavailable(&narrow, 80, 24);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("unavailable back at 120 columns", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && painted(screen, 120, 40)
            && unavailable(screen)
    });
    assert_unavailable(&restored, 120, 40);
    back(&app);
    let finder = app.wait_text("Type Search");
    assert!(
        selected(&finder).contains(DOCKER) || query_line(&finder).contains(DOCKER),
        "finder left the Docker row\n{}",
        text(&finder)
    );
    app.finish();
}

#[test]
fn h04_snapshots_docker_scenario_docker_cleanup() {
    let app = open("h04-snapshots-docker-scenario", "docker-cleanup");
    let landing = app.screen();
    see(
        &landing,
        &[CLEAN, "devbox", "18.4 GB", "docker clean"],
        &[SOCK, "docker.sock"],
    );
    clear_query(&app);
    paste_row(&app, LIST);
    let page = app.wait_screen("scenario containers", containers_page);
    assert_running_containers(&page);
    see(
        &page,
        &["devbox", "acme/api:dev", "monitoring", "18.4"],
        &[SOCK, UNREACHABLE, "mbp · live"],
    );
    app.finish();
}
