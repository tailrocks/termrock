//! TablePro explain plan: the tree, the raw plan, and analysis are distinct.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_plan/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, SessionMode, TABLEPRO, file_sha256, find_all, reference_binary_from,
    reference_digest_from, row_text, text, try_launch,
};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const SQL: &str = "SELECT sku FROM products";
const PLACEHOLDER: &str = "Type SQL";
const PLANNING: &str = "Planning 0.210 ms · r Raw";
const PLANNING_TIME: &str = "Planning Time: 0.210 ms";
const RAW_TITLE: &str = "EXPLAIN · raw";
const COST: &str = "Seq Scan on public.products  (cost=0.00..15.90 rows=312 width=64)";
const EXECUTION: &str = "Planning 0.210 ms · Execution 0.466 ms";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_plan/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let capture = std::env::var_os("T03_CAPTURE").is_some_and(|value| value == "1");
    let mut launch = if capture {
        let manifest =
            crate::support::workspace_root().join("docs/testing/reference-manifest-t03-pr2.json");
        let binary = reference_binary_from(&manifest, "tablepro-pr2");
        assert_eq!(
            file_sha256(&binary),
            reference_digest_from(&manifest, "tablepro-pr2"),
            "T03 reference binary digest"
        );
        let mut launch = Launch::new(name, binary.to_str().expect("reference path"));
        launch.mode = SessionMode::Reference;
        launch.reference_manifest = Some(manifest);
        launch
    } else {
        Launch::new(name, TABLEPRO)
    };
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
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
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Execute query?",
        ],
    );
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn capture_reference_frame(app: &App, test_id: &str, screen: &TerminalSnapshot) {
    let session_path = app.evidence_dir().join("session.json");
    let session: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&session_path).expect("reference session metadata"))
            .expect("parse reference session metadata");
    assert_eq!(session["mode"], "reference");
    assert_eq!(
        session["reference_sha"],
        "3cbe13ce922f7d0902bf42af418f3d012d8b537f"
    );
    assert_eq!(
        session["lockfile_sha256"],
        "9d9859e69e1de0b3306231a25a381c865079ca7ab533420f34dceb6eafe4d39a"
    );
    assert_eq!(
        session["binary_sha256"],
        "ebcc660347e420a9ea1879d1a54537ab08f5cbb750d88701ed5956899492e133"
    );
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let directory = app.evidence_dir().join("reference-candidates");
    std::fs::create_dir_all(&directory).expect("create reference candidate directory");
    let candidate = directory.join(format!("{}.tui.pb", test_id.replace('/', "_")));
    std::fs::write(&candidate, &bytes).expect("write reference candidate sample");
    let digest = file_sha256(&candidate);
    let index = serde_json::json!({
        "test_id": test_id,
        "candidate_path": candidate,
        "candidate_sha256": digest,
        "session_path": session_path,
        "session_sha256": file_sha256(&session_path),
        "binary_path": session["binary_path"],
        "binary_sha256": session["binary_sha256"],
        "reference_sha": session["reference_sha"],
        "lockfile_sha256": session["lockfile_sha256"],
    });
    use std::io::Write as _;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("index.jsonl"))
        .expect("open reference capture index");
    serde_json::to_writer(&mut log, &index).expect("write reference capture index");
    log.write_all(b"\n")
        .expect("finish reference capture index");
    eprintln!(
        "T03 reference candidate {test_id} sha256 {digest} at {}",
        candidate.display()
    );
}

fn finished(screen: &TerminalSnapshot) -> bool {
    !text(screen).contains("Esc cancels")
}

fn tree_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    finished(screen)
        && visible.contains("Operation")
        && visible.contains("Seq Scan")
        && visible.contains(PLANNING)
        && visible.contains(SQL)
        && !visible.contains(RAW_TITLE)
        && !visible.contains("Execution")
        && !visible.contains("Execute query?")
        && !visible.contains("Gather")
        && !visible.contains("Parallel")
        && !visible.contains("Sort")
}

fn raw_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    finished(screen)
        && visible.contains(RAW_TITLE)
        && visible.contains(PLANNING_TIME)
        && visible.contains(COST)
        && visible.contains(SQL)
        && !visible.contains("productsr")
        && !visible.contains("Operation")
        && !visible.contains("Execution")
        && !visible.contains("Execute query?")
}

fn raw_narrow(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    finished(screen)
        && visible.contains(RAW_TITLE)
        && visible.contains("Planning Time:")
        && visible.contains(SQL)
        && !visible.contains("Operation")
        && !visible.contains("productsr")
}

fn type_statement(app: &App) {
    let editor = app.wait_text(PLACEHOLDER);
    let hits = find_all(&editor, PLACEHOLDER);
    assert_eq!(
        hits.len(),
        1,
        "placeholder hits {hits:?}\n{}",
        text(&editor)
    );
    app.click(hits[0]);
    app.wait_screen("editor focused", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains('▎') && line.contains(PLACEHOLDER)
        })
    });
    app.type_text(SQL);
    app.wait_text(SQL);
}

fn focus_plan(app: &App) {
    let screen = app.wait_text("Seq Scan");
    let mut hits = find_all(&screen, "Seq Scan");
    hits.sort_by_key(|hit| (hit.x, hit.y));
    assert!(!hits.is_empty(), "no Seq Scan row\n{}", text(&screen));
    app.click(hits[0]);
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
fn tablepro_plan_route_reaches_query_1() {
    let app = open("t03-plan-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &["Loading schema", RAW_TITLE, "Execute query?"],
    );
    app.finish();
}

#[test]
fn tablepro_plan_raw_is_distinct_from_the_tree_and_analysis() {
    let app = open("t03-plan-bodies");
    connect_development(&app);
    type_statement(&app);

    app.key("ctrl+x");
    let tree = app.wait_screen("explain tree", tree_body);
    assert_eq!((tree.cols(), tree.rows()), (120, 40));
    see(
        &tree,
        &["Operation", "Seq Scan", PLANNING, SQL],
        &[
            RAW_TITLE,
            "Execution",
            "Esc cancels",
            "Gather",
            "Parallel",
            "Sort",
            "Execute query?",
        ],
    );

    focus_plan(&app);
    app.key("r");
    let raw = app.wait_screen("raw plan", raw_body);
    assert_eq!((raw.cols(), raw.rows()), (120, 40));
    see(
        &raw,
        &[RAW_TITLE, PLANNING_TIME, COST, SQL],
        &["Operation", "Execution", "productsr", "Execute query?"],
    );
    shot(&app, "T03.plan.plan.120x40");

    app.expire_status();
    app.resize(80, 24);
    let narrow = app.wait_screen("raw plan at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && raw_narrow(screen)
            && !text(screen).contains(CONNECTED)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[RAW_TITLE, "Planning Time:", SQL],
        &["Operation", "productsr"],
    );
    let test_id = visual_id("T03.plan.plan.narrow");
    if std::env::var_os("T03_CAPTURE").is_some_and(|value| value == "1") {
        capture_reference_frame(&app, &test_id, &narrow);
    } else {
        app.visual(&test_id);
    }

    app.resize(120, 40);
    app.wait_screen("raw plan back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && raw_body(screen)
    });

    app.key("alt+x");
    // Analysis replaces the raw card. "Execution" is the distinct body;
    // the tree header "Operation" is not that proof.
    let analyzed = app.wait_screen("explain analyze", |screen| {
        let visible = text(screen);
        finished(screen)
            && visible.contains("Execution")
            && visible.contains(EXECUTION)
            && visible.contains(SQL)
            && !visible.contains("Execute query?")
    });
    see(
        &analyzed,
        &["Execution", EXECUTION, "0.466 ms", SQL, "EXPLAIN ANALYZE"],
        &["r Raw", "Execute query?", "Esc cancels", "productsr"],
    );
    quit_unsaved(&app);
}
