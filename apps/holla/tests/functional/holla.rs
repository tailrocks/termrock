use super::support::{App, HOLLA, find, text};

fn holla(name: &str, scenario: &str) -> App {
    App::new(
        name,
        HOLLA,
        &[
            "--scenario",
            scenario,
            "--motion",
            "reduced",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    )
}

fn holla_paused(name: &str) -> App {
    App::new(
        name,
        HOLLA,
        &[
            "--scenario",
            "first-use",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    )
}

#[test]
fn holla_finder_search_runs_selected_task_and_records_result() {
    let app = holla("holla-finder-history", "parity-history");
    app.wait_text("Search actions and resources");
    app.paste("cargo check");
    app.wait_text("▎ cargo check");
    app.key("enter");
    let done = app.wait_screen("successful task result", |s| {
        let t = text(s);
        t.contains("succeeded") && t.contains("exit 0") && t.contains("Finished `dev` profile")
    });
    assert!(text(&done).contains("$ cargo check"));
    app.key("ctrl+g");
    let history = app.wait_screen("completed activity in picker", |s| {
        let t = text(s);
        t.contains("0 live") && t.contains("cargo check") && t.contains("succeeded")
    });
    assert!(!text(&history).contains("Nothing has been started yet"));
    app.key("escape");
    app.visual("functional/holla_finder_history");
    app.finish();
}

#[test]
fn holla_cancelling_second_gate_leaves_no_started_activity() {
    let app = holla("holla-second-gate", "docker-cleanup");
    app.wait_text("Clean Docker completely");
    app.key("enter");
    app.wait_text("Here › Clean Docker completely");
    app.key("c");
    app.wait_screen("first confirmation gate", |s| {
        let t = text(s);
        t.contains("gate 1 of 2") && t.contains("Sequence · 8 commands")
    });
    app.key("right");
    app.key("enter");
    app.wait_screen("second confirmation gate", |s| {
        let t = text(s);
        t.contains("gate 2 of 2")
            && t.contains("Type I UNDERSTAND: REMOVE ALL DOCKER DATA ON devbox")
    });
    app.key("escape");
    let cancelled = app.wait_text("Cancelled · nothing was executed");
    assert!(!text(&cancelled).contains("gate 2 of 2"));

    app.key("ctrl+g");
    let empty = app.wait_screen("empty activity picker", |s| {
        let t = text(s);
        t.contains("0 live") && t.contains("Nothing has been started yet")
    });
    assert!(!text(&empty).contains("gate 2 of 2"));
    app.key("escape");
    app.visual("functional/holla_second_gate_cancel");
    app.finish();
}

#[test]
fn holla_browser_pointer_selection_loads_preview_and_hidden_toggle() {
    let app = holla("holla-browser-pointer", "parity-browser");
    app.wait_text("Search actions and resources");
    app.paste("Browse ~/work/site");
    app.wait_text("▎ Browse ~/work/site");
    app.key("enter");
    let listing = app.wait_screen("loaded directory listing", |s| {
        let t = text(s);
        t.contains("Files · ~/work/site") && t.contains("16 entries") && t.contains("index.html")
    });
    assert!(!text(&listing).contains(".env"));

    let target = find(&listing, "index.html");
    app.click(target);
    let preview = app.wait_screen("selected HTML preview", |s| {
        let t = text(s);
        t.contains("<html>") && t.contains("hello")
    });
    assert!(text(&preview).contains("index.html"));

    app.key("ctrl+h");
    let shown = app.wait_text("18 entries · 2 hidden shown");
    assert!(text(&shown).contains(".env") && text(&shown).contains(".git"));
    app.key("ctrl+h");
    let hidden = app.wait_text("16 entries · 2 hidden hidden");
    assert!(!text(&hidden).contains(".env"));

    app.key("g");
    app.wait_text("Go to path");
    app.visual("functional/holla_browser_pointer");
    app.key("escape");
    app.finish();
}

#[test]
fn holla_trust_cancel_then_run_uses_the_child_directory() {
    let app = App::new(
        "holla-trust-run",
        HOLLA,
        &["--scenario", "monorepo-child", "--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("From acme · the parent root");
    app.type_text("test");
    let results = app.wait_text("alias test");
    assert!(text(&results).contains("Run tests"));
    app.key("enter");
    app.wait_text("Trust ~/work/acme/apps/frontend/mise.toml?");
    app.wait_text("[tasks.test]");
    app.key("escape");
    let cancelled = app.wait_absent("Trust ~/work/acme/apps/frontend/mise.toml?");
    assert!(text(&cancelled).contains("Run tests"));
    assert!(!text(&cancelled).contains("19 passed"));

    app.key("enter");
    app.wait_text("Trust ~/work/acme/apps/frontend/mise.toml?");
    app.key("right");
    app.key("enter");
    app.wait_text("arguments");
    app.key("ctrl+s");
    let launched = app.wait_screen("frontend test has its own activity and output", |screen| {
        let visible = text(screen);
        visible
            .lines()
            .any(|line| line.trim_start().starts_with("frontend test ·"))
            && visible.contains("╭─ output")
    });
    let launched_text = text(&launched);
    assert!(launched_text.lines().any(|line| {
        line.trim_start()
            .starts_with("$ mise run //apps/frontend:test")
    }));
    let activity_header = launched_text
        .lines()
        .find(|line| line.trim_start().starts_with("frontend test ·"))
        .expect("the activity header");
    let directory = activity_header
        .split("here ·")
        .nth(1)
        .and_then(|location| location.split('·').next())
        .expect("activity header states its working directory")
        .trim();
    assert_eq!(directory, "~/work/acme/apps/frontend");
    app.wait_text("19 passed (19)");
    app.wait_text("succeeded");
    app.visual("functional/holla_trust_cancel_then_run");
    app.finish();
}

#[test]
fn holla_browser_hidden_files_preview_scroll_and_bad_path_recovery() {
    let app = App::new(
        "holla-browser",
        HOLLA,
        &["--scenario", "parity-browser", "--motion", "reduced"],
        120,
        40,
        "nocolor",
    );
    app.wait_text("holla❯");
    app.type_text("Browse ~/work/site");
    app.key("enter");
    let initial = app.wait_text("16 entries");
    assert!(!text(&initial).contains(".env"));
    app.key("ctrl+h");
    app.wait_text("Hidden entries shown");
    app.wait_text(".env");
    app.key("ctrl+h");
    let hidden = app.wait_text("Hidden entries hidden");
    assert!(!text(&hidden).contains(".env"));
    app.key("home");
    app.key("down");
    app.key("down");
    app.key("down");
    app.wait_text("first 2000 lines");
    let original = app.wait_text("line 5");
    let preview = find(&original, "line 5");

    let preview_border = original.row(u32::from(preview.y)).expect("preview row")
        [..usize::from(preview.x)]
        .iter()
        .rposition(|cell| cell.text() == "│")
        .expect("preview has a visible left border");
    let segments = |screen: &tui_test::TerminalSnapshot| {
        let cells = screen.row(u32::from(preview.y)).expect("preview row");
        let split = usize::from(preview.x);
        let left: String = cells[..preview_border]
            .iter()
            .map(|cell| cell.text())
            .collect();
        let preview: String = cells[split..].iter().map(|cell| cell.text()).collect();
        (left, preview)
    };
    let (before_left, before_preview) = segments(&original);
    app.wheel(preview, true, 3);
    let scrolled = app.wait_screen("preview content advances after wheel", |screen| {
        segments(screen).1 != before_preview && text(screen).contains("first 2000 lines")
    });
    assert_eq!(
        segments(&scrolled).0,
        before_left,
        "wheel leaves the file list unchanged"
    );
    assert!(text(&scrolled).contains("16 entries"));
    app.wheel(preview, false, 3);
    let restored = app.wait_screen("preview wheel up restores original content", |screen| {
        segments(screen).1 == before_preview
    });
    assert_eq!(
        segments(&restored).0,
        before_left,
        "reverse wheel leaves the file list unchanged"
    );
    app.key("g");
    app.wait_text("type a path or a name");
    app.type_text("nope");
    app.key("enter");
    app.wait_text("no such file or directory");
    app.key("escape");
    app.wait_absent("no such file or directory");
    app.wait_text("type a path or a name");
    app.key("escape");
    app.wait_absent("type a path or a name");
    app.wait_text("16 entries");
    app.key("home");
    app.key("g");
    app.wait_text("type a path or a name");
    app.key("escape");
    app.wait_absent("type a path or a name");
    app.visual("functional/holla_browser_hidden_files");
    app.finish();
}

#[test]
fn holla_first_use_idle_and_no_results() {
    let app = holla_paused("holla-first-use-paused");
    app.wait_text("Search actions and resources");
    app.type_text("zzzzqq");
    let none = app.wait_text("No matches for “zzzzqq”");
    assert!(text(&none).contains("Esc clears the query"));
    assert!(!text(&none).contains("cargo check"));
    app.key("escape");
    app.wait_text("Search actions and resources");
    app.visual("functional/holla_first_use_idle");
    app.finish();
}

#[test]
fn holla_help_overlay() {
    let app = holla_paused("holla-help-paused");
    app.wait_text("Search actions and resources");
    app.key("f1");
    app.wait_text("Key reference");
    app.key("escape");
    app.wait_absent("Key reference");
    app.visual("functional/holla_help");
    app.finish();
}

#[test]
fn holla_second_gate_wrong_phrase_stays_then_reconfirm_executes() {
    const PHRASE: &str = "I UNDERSTAND: REMOVE ALL DOCKER DATA ON devbox";
    let app = holla("holla-second-gate-execute", "docker-cleanup");
    app.wait_text("Clean Docker completely");
    app.key("enter");
    app.wait_text("Here › Clean Docker completely");
    app.key("c");
    app.wait_screen("first confirmation gate", |s| {
        text(s).contains("gate 1 of 2")
    });
    app.key("right");
    app.key("enter");
    app.wait_screen("second confirmation gate", |s| {
        text(s).contains("gate 2 of 2")
    });
    app.type_text("I UNDERSTAND: WRONG");
    app.key("enter");
    let still = app.wait_screen("wrong phrase keeps the second gate", |s| {
        text(s).contains("gate 2 of 2")
    });
    assert!(!text(&still).contains("Cancelled"));
    assert!(!text(&still).contains("7 succeeded"));

    app.key("ctrl+u");
    app.paste(PHRASE);
    app.key("enter");
    let invalidated = app.wait_text("confirmation invalidated, review again");
    assert!(text(&invalidated).contains("gate 1 of 2"));
    assert!(text(&invalidated).contains("tmp-shell-3"));
    assert!(!text(&invalidated).contains("7 succeeded"));
    assert!(!text(&invalidated).contains("finished · 7 ok"));
    app.visual("functional/holla_second_gate_execute");

    app.key("right");
    app.key("enter");
    app.wait_screen("second gate after the plan change", |s| {
        text(s).contains("gate 2 of 2")
    });
    app.paste(PHRASE);
    app.key("enter");
    app.wait_text("Stop running containers");
    let executed = app.wait_screen_for(
        "exact phrase finishes the docker plan",
        std::time::Duration::from_secs(20),
        |s| {
            let t = text(s);
            t.contains("finished · 7 ok") && t.contains("7 succeeded")
        },
    );
    for step in [
        "Stop running containers",
        "Remove containers",
        "Remove unused images",
        "Prune networks",
        "Prune volumes",
        "Prune builder and Buildx c",
        "Rescan and report",
    ] {
        assert!(
            text(&executed).contains(step),
            "finished plan shows {step}:\n{}",
            text(&executed)
        );
    }
    assert!(text(&executed).contains("Docker disk usage"));
    assert!(!text(&executed).contains("Cancelled · nothing was executed"));
    assert!(!text(&executed).contains("gate 2 of 2"));
    app.finish();
}

#[test]
fn holla_disk_tree_and_plan_review_flows() {
    let app = holla("holla-disk-tree", "disk-cleanup");
    app.wait_screen("disk usage tree opens", |s| {
        text(s).contains("Disk › Usage") || text(s).contains("Disk")
    });
    app.settle();
    app.visual("functional/holla_disk_tree");
    app.finish();

    let plan = App::new(
        "holla-plan-review",
        HOLLA,
        &[
            "--scenario",
            "upgrade-plan",
            "--motion",
            "paused",
            "--frame",
            "48",
        ],
        120,
        40,
        "truecolor",
    );
    plan.wait_text("8 steps · 8 included");
    for _ in 0..5 {
        plan.key("down");
    }
    plan.key("space");
    plan.wait_text("7 included");
    plan.key("u");
    plan.wait_text("8 included");
    plan.key("escape");
    plan.wait_text("Search actions and resources");
    plan.visual("functional/holla_plan_review");
    plan.finish();
}

#[test]
fn holla_launch_failure_state_is_visible() {
    let app = holla("holla-launch-failure", "launch-failure");
    let failed = app.wait_text("frontend dev · failed");
    assert!(text(&failed).contains("exit 1"));
    assert!(text(&failed).contains("frontend dev"));
    assert!(!text(&failed).contains("Search actions and resources"));
    app.visual("functional/holla_launch_failure");
    app.finish();
}

#[test]
fn holla_resize_finder_recovers() {
    let app = App::new(
        "holla-resize",
        HOLLA,
        &[
            "--scenario",
            "parity-browser",
            "--motion",
            "reduced",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Search actions and resources");
    app.paste("Browse ~/work/site");
    app.wait_text("▎ Browse ~/work/site");
    app.key("enter");
    app.wait_text("16 entries");
    app.resize(80, 24);
    app.wait_screen("files still listed at 80x24", |s| {
        s.cols() == 80 && s.rows() == 24 && text(s).contains("16 entries")
    });
    app.visual("functional/holla_browser_80x24");
    app.resize(120, 40);
    app.wait_screen("files still listed at 120x40", |s| {
        s.cols() == 120 && text(s).contains("16 entries")
    });
    app.visual("functional/holla_resize_narrow");
    app.finish();
}
