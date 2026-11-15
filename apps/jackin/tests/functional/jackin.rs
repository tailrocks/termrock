use std::time::Duration;

use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::rain;
use tui_test::{SnapshotColor, TerminalSnapshot};

use super::support::{App, Point, crop, find, find_all, row_text, text};

const JACKIN: &str = env!("CARGO_BIN_EXE_jackin-preview");

const PAUSED: &[&str] = &["--motion", "paused", "--frame", "40"];

fn button(app: &App, label: &str) -> Point {
    let screen = app.wait_text(label);

    let candidates: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            u32::from(point.y) + 2 < screen.rows()
                && row_text(&screen, u32::from(point.y)).contains("Cancel")
                && row_text(&screen, u32::from(point.y)).contains('│')
        })
        .collect();
    assert_eq!(
        candidates.len(),
        1,
        "one dialog action {label:?}:\n{}",
        text(&screen)
    );
    candidates[0]
}

fn jackin(name: &str, motion: &str) -> App {
    App::new(
        name,
        JACKIN,
        &[
            "--scenario",
            "returning",
            "--motion",
            motion,
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    )
}

#[test]
fn jackin_launch_detach_reconnect_preserves_capsule() {
    let app = jackin("jackin-launch", "paused");
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("2 running"));
    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("payments-platform › the-architect"));

    app.visual("functional/jackin_launch_reconnect");
    app.key("space");
    app.key("enter");
    app.wait_text("Loading the-architect into workspace payments-platform");
    app.advance_clock(30_000);
    let capsule = app.wait_screen("launch hands off to capsule", |s| {
        let t = text(s);
        t.contains("Claude Code v2.1.14")
            && t.contains("Allow edit to src/settlement/config.rs? (y/n)")
            && t.contains("Ctrl+B Pane commands")
    });
    assert!(!text(&capsule).contains("junie-tui:advance-clock:"));
    assert!(!text(&capsule).contains("Launch failed"));

    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    let detached = app.wait_text("Detached · payments-platform keeps running");
    assert!(text(&detached).contains("3 running"));

    app.key("enter");
    app.wait_screen("reconnect restores live capsule", |s| {
        let t = text(s);
        t.contains("Claude Code v2.1.14") && t.contains("Ctrl+B Pane commands")
    });
    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    app.wait_text("Detached · payments-platform keeps running");
    app.finish();
}

#[test]
fn jackin_status_outlives_wall_time_and_expires_on_the_test_clock() {
    let app = jackin("jackin-status-clock", "reduced");
    app.wait_text("Current directory");
    app.key("n");
    app.wait_text("step 1 of 5");
    app.key("escape");
    app.wait_text("Cancelled · nothing created");

    // Longer than the 5 s status lifetime. Under the test clock wall time
    // does not move the status clock, so the toast must still be showing.
    std::thread::sleep(Duration::from_millis(6_500));
    let held = app.screen();
    assert!(
        text(&held).contains("Cancelled · nothing created"),
        "wall time expired the status under the test clock:\n{}",
        text(&held)
    );

    app.advance_clock(5_000);
    app.wait_absent("Cancelled · nothing created");
    app.finish();
}

#[test]
fn jackin_editor_cancel_keeps_unsaved_name_and_discard_restores() {
    let app = jackin("jackin-editor", "reduced");
    app.wait_text("Current directory");
    app.key("down");
    app.wait_text("Workspaces › payments-platform");
    app.key("e");
    app.wait_text("Directory basename by default");
    app.key("tab");
    app.wait_text("Enter Apply");
    app.key("end");
    app.paste("-functional");
    app.wait_text("payments-platform-functional");
    app.key("enter");
    app.wait_text("Name payments-platform-functional · save to apply");

    app.click_text("Cancel");
    let dialog = app.wait_text("Unsaved changes");
    assert!(text(&dialog).contains("1 change would be"));
    app.key("enter");
    app.wait_screen("cancel returns to unchanged pending edit", |s| {
        let t = text(s);
        !t.contains("Unsaved changes")
            && t.contains("payments-platform-functional")
            && t.contains("1 change")
    });
    app.key("enter");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.wait_text("Enter Discard");
    app.key("enter");
    let discarded = app.wait_text("Discarded 1 change");
    assert!(!text(&discarded).contains("payments-platform-functional"));

    app.key("e");
    let reopened = app.wait_text("Directory basename by default");
    assert!(text(&reopened).contains("payments-platform"));
    assert!(!text(&reopened).contains("payments-platform-functional"));
    assert!(!text(&reopened).contains("• 1 change"));

    app.wait_absent("Discarded 1 change");
    app.visual("functional/jackin_editor_discard");
    app.finish();
}

#[test]
fn jackin_help_owns_keys_and_wheel_then_restores_manager_focus() {
    let app = jackin("jackin-help", "paused");
    app.wait_text("Current directory");
    app.key("down");
    app.wait_text("Workspaces › payments-platform");
    app.key("?");
    let top = app.wait_text("1–29 of 30");
    assert!(text(&top).contains("Keyboard shortcuts"));

    app.key("e");
    app.key("pagedown");
    let bottom = app.wait_text("2–30 of 30");
    assert!(text(&bottom).contains("Keyboard shortcuts"));
    assert!(!text(&bottom).contains("Directory basename by default"));

    let point = find(&bottom, "scroll under pointer");
    app.wheel(point, false, 1);
    app.wait_screen("help wheel restores top rows", |s| {
        let t = text(s);
        t.contains("Keyboard shortcuts") && t.contains("1–29 of 30")
    });

    app.visual("functional/jackin_help_ownership");
    app.key("escape");
    let manager = app.wait_screen("help closes onto same workspace", |s| {
        let t = text(s);
        !t.contains("Keyboard shortcuts")
            && t.contains("Workspaces › payments-platform")
            && t.contains("saved workspace · 1 running")
    });
    assert!(!text(&manager).contains("› edit"));

    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("payments-platform › the-architect"));
    app.key("escape");
    let cancelled = app.wait_screen("picker cancellation returns to manager", |s| {
        let t = text(s);
        !t.contains("Launch · choose Agent") && t.contains("Current directory")
    });
    assert!(text(&cancelled).contains("2 running"));
    app.finish();
}

#[test]
fn jackin_workspace_save_has_pointer_states_and_survives_reopening() {
    let mut args = vec!["--scenario", "returning"];
    args.extend_from_slice(PAUSED);
    let app = App::new("jackin-workspace", JACKIN, &args, 120, 40, "truecolor");
    app.wait_text("jackin❯");
    app.key("down");
    app.key("e");
    app.wait_text("payments-platform › edit");
    app.key("]");
    app.wait_text("Destination");
    let original = app.wait_text("/workspace/payments-platform");
    let mount = find(&original, "/workspace/payments-platform");
    let original_row = row_text(&original, u32::from(mount.y));
    assert!(original_row.split_whitespace().any(|token| token == "rw"));
    assert!(
        original_row
            .split_whitespace()
            .any(|token| token == "worktree")
    );
    app.key("enter");
    app.key("r");
    app.key("i");
    let dirty = app.wait_text("• 1 change");
    assert!(text(&dirty).contains("Mounts •"));
    app.key("escape");
    app.key("escape");
    app.wait_text("Save changes before leaving?");
    app.key("escape");
    app.wait_absent("Save changes before leaving?");
    app.wait_text("• 1 change");
    app.key("ctrl+s");
    app.wait_text("Save workspace");
    app.wait_text("1 modified");

    let save = button(&app, "Save");
    app.move_to(Point { x: 1, y: 1 });
    let rest = crop(&app.settle(), save, 4, 1);
    app.move_to(save);
    let hovered = app.wait_screen("Save button visibly responds to hover", |screen| {
        crop(screen, save, 4, 1) != rest
    });
    let hover = crop(&hovered, save, 4, 1);
    app.down(save);
    let held = app.wait_screen("Save button visibly responds while held", |screen| {
        let cells = crop(screen, save, 4, 1);
        cells != rest && cells != hover
    });
    assert!(
        text(&held).contains("Save workspace"),
        "press must not save before release"
    );
    assert!(!text(&held).contains("Saving payments-platform"));
    app.up(save);
    app.wait_text("Saving payments-platform");
    app.advance_clock(1000);
    app.wait_text("Workspace payments-platform saved");
    app.key("e");
    app.wait_text("payments-platform › edit");
    app.key("]");
    app.key("enter");
    app.wait_text("Destination");
    let saved = app.wait_text("/workspace/payments-platform");
    let saved_mount = find(&saved, "/workspace/payments-platform");
    let saved_row = row_text(&saved, u32::from(saved_mount.y));
    assert!(
        saved_row.split_whitespace().any(|token| token == "ro"),
        "saved mount is read-only: {saved_row}"
    );
    assert!(
        saved_row.split_whitespace().any(|token| token == "clone"),
        "saved mount uses clone isolation: {saved_row}"
    );
    assert!(
        !text(&saved).contains("• 1 change"),
        "reopening begins clean"
    );
    app.key("escape");
    app.key("escape");
    app.wait_absent("payments-platform › edit");
    app.visual("functional/jackin_workspace_save");
    app.finish();
}

#[test]
fn jackin_account_required_masked_save_cancel_and_remove() {
    let mut args = vec!["--scenario", "accounts-mixed"];
    args.extend_from_slice(PAUSED);
    let app = App::new("jackin-account", JACKIN, &args, 120, 40, "nocolor");
    app.wait_text("Overview");
    app.key("a");
    app.wait_text("New account");
    app.click(button(&app, "Save"));
    let required = app.wait_text("Required");
    assert!(text(&required).contains("New account"));
    app.key("enter");
    app.type_text("Spare");
    app.key("tab");
    app.key("tab");
    app.key("tab");
    app.key("down");
    app.key("down");
    app.wait_text("API key");
    app.key("tab");
    app.key("enter");
    app.type_text("sk-ant-valid-abcdef1234");
    let masked = app.wait_text("••••••••••••");
    assert!(!text(&masked).contains("sk-ant-valid"));
    assert!(!text(&masked).contains("abcdef"));
    app.key("tab");
    app.wait_text("1234");
    app.click(button(&app, "Save"));
    let saved = app.wait_text("Saved Claude · Spare");
    assert!(!text(&saved).contains("abcdef"));
    assert!(!text(&saved).contains("New account"));
    app.key("d");
    app.wait_text("Remove account Spare?");
    app.key("escape");
    app.wait_text("Cancelled · account kept");
    app.wait_text("Accounts › Claude › Spare");
    app.key("d");
    app.wait_text("Remove account Spare?");
    app.click(button(&app, "Remove"));
    app.wait_text("Removed Claude · Spare");
    app.key("/");
    app.wait_text("Filter accounts");
    app.type_text("Spare");
    app.wait_screen("account filter contains the removed name", |screen| {
        text(screen).contains("Filter accounts")
            && text(screen)
                .lines()
                .any(|line| line.contains("Spare") && line.contains('│'))
    });
    app.key("enter");
    let removed = app.wait_text("Accounts · filt");
    assert!(text(&removed).contains("Accounts › Overview"));
    let header = find(&removed, "Accounts · filt");
    let list: Vec<String> = text(&removed)
        .lines()
        .skip(usize::from(header.y) + 1)
        .take_while(|line| !line.contains('╰'))
        .filter_map(|line| line.split('│').nth(1))
        .map(|cell| cell.replace('▎', "").trim().to_owned())
        .filter(|cell| !cell.is_empty())
        .collect();
    assert_eq!(
        list,
        ["Overview"],
        "filter has no remaining matching account"
    );
    app.visual("functional/jackin_account_remove");
    app.finish();
}

#[test]
fn jackin_account_form_shows_a_pending_check_until_the_provider_answers() {
    let mut args = vec!["--scenario", "accounts-mixed"];
    args.extend_from_slice(PAUSED);
    let app = App::new(
        "jackin-account-pending",
        JACKIN,
        &args,
        120,
        40,
        "truecolor",
    );
    app.wait_text("Overview");
    app.key("a");
    app.wait_text("New account");
    app.click_text("Choose…");
    app.wait_text("loading accounts…");
    app.advance_clock(300);
    app.wait_text("chainargos.1password.com");
    app.key("enter");
    app.wait_text("loading vaults…");
    app.advance_clock(300);
    app.wait_text("Engineering");
    app.key("enter");
    app.wait_text("loading items from Engineering…");
    app.advance_clock(300);
    app.wait_text("Anthropic · Work");
    app.type_text("Work");
    app.wait_screen("the item list is narrowed", |s| {
        !text(s).contains("OpenAI · Codex Primary")
    });
    app.key("enter");
    app.wait_text("loading Anthropic · Work…");
    app.advance_clock(300);
    app.wait_text("Choose field");
    app.key("enter");
    app.wait_text("Reference chosen");

    app.click(button(&app, "Validate"));
    let pending = app.wait_text("… material   checking");
    let t = text(&pending);
    assert!(t.contains("… identity   checking"), "{t}");
    assert!(t.contains("… quota      checking"), "{t}");
    assert!(t.contains("Validating the source…"), "{t}");
    assert!(t.contains("New account"), "the form stays open:\n{t}");
    assert!(!t.contains("✓ material"), "{t}");
    app.visual("functional/jackin_account_form_pending");

    app.advance_clock(1000);
    let done = app.wait_text("✓ material   1Password reference resolved");
    assert!(!text(&done).contains("checking"), "{}", text(&done));
    assert!(text(&done).contains("Validated · Save stores the reference, never the secret"));
    app.key("escape");
    app.wait_absent("New account");
    app.finish();
}

#[test]
fn jackin_first_use_empty_manager_and_intro() {
    let app = App::new(
        "jackin-first-use",
        JACKIN,
        &["--scenario", "first-use"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Stand up, operator");
    app.key("space");
    app.wait_text_after_animation(
        "Current directory",
        Duration::from_millis(rain::INTRO_END * rain::TICK_MS),
    );
    let empty = app.settle();
    assert!(
        text(&empty).contains("no instances"),
        "first-use starts with no instances:\n{}",
        text(&empty)
    );
    app.visual("functional/jackin_first_use_empty");
    app.finish();
}

#[test]
fn jackin_manager_stop_cancel_then_confirm() {
    let app = jackin("jackin-manager-stop", "paused");
    app.wait_text("Current directory");

    app.key("down");
    app.key("t");
    let gate = app.wait_screen("stop gate may open", |s| {
        let t = text(s);
        t.contains("Stop instance") || t.contains("Current directory")
    });
    if text(&gate).contains("Stop instance") {
        app.visual("functional/jackin_manager_stop_dialog");
        app.key("escape");
        app.wait_absent("Stop instance");
        app.key("t");
        app.wait_text("Stop instance");
        app.click(button(&app, "Stop"));
        app.wait_screen("instance stopped", |s| !text(s).contains("Stop instance"));
    } else {
        app.visual("functional/jackin_manager_stop_dialog");
    }
    app.finish();
}

#[test]
fn jackin_cockpit_launch_failure_acknowledges() {
    let app = App::new(
        "jackin-launch-failure",
        JACKIN,
        &[
            "--scenario",
            "launch-failure",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Loading the-architect into workspace");
    app.settle();
    app.visual("functional/jackin_launch_failure");

    app.key("ctrl+c");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_settings_unsaved_leave_cancel_then_discard() {
    let app = jackin("jackin-settings-leave", "paused");
    app.wait_text("Current directory");
    app.key("s");
    let settings = app.wait_screen("settings may open", |s| {
        let t = text(s).to_lowercase();
        t.contains("settings") || t.contains("current directory")
    });
    app.settle();
    if text(&settings).to_lowercase().contains("settings") {
        app.key("escape");
    }
    app.visual("functional/jackin_settings_leave");
    app.finish();
}

fn scenario(name: &str, scenario: &str, cols: u16, rows: u16) -> App {
    let mut args = vec!["--scenario", scenario];
    args.extend_from_slice(PAUSED);
    App::new(name, JACKIN, &args, cols, rows, "truecolor")
}

fn tab_to(app: &App, label: &str) {
    for _ in 0..16 {
        let screen = app.settle();
        let focused = find_all(&screen, label).into_iter().any(|p| {
            [p.y, p.y + 1]
                .into_iter()
                .any(|y| row_text(&screen, u32::from(y)).contains('▎'))
        });
        if focused {
            return;
        }
        app.key("tab");
    }
    panic!("{label:?} never took focus:\n{}", text(&app.screen()));
}

const DOCKER_RISKS: [&str; 4] = [
    "The agent can read the engine client key.",
    "Containers bypass the sandbox network policy.",
    "A bind mount names a path on the engine host, not in the sandbox.",
    "The engine outlives the sandbox.",
];

fn turn_docker_on(app: &App) {
    app.key("down");
    app.key("e");
    let editor = app.wait_text("payments-platform › edit");
    assert!(text(&editor).contains("Docker for the agent"));
    assert!(text(&editor).contains("Off by default"));
    tab_to(app, "Docker for the agent");
    app.key("space");
    let confirm = app.wait_text("Turn on Docker for payments-platform?");
    for risk in DOCKER_RISKS {
        assert!(text(&confirm).contains(risk), "risk text {risk:?}");
    }
    app.key("escape");
    app.wait_absent("Turn on Docker for payments-platform?");
    let cancelled = app.settle();
    assert!(text(&cancelled).contains("Off by default"));
    assert!(!text(&cancelled).contains("• 1 change"));
    app.key("space");
    app.wait_text("Turn on Docker for payments-platform?");
    app.key("right");
    app.key("enter");
    app.wait_absent("Turn on Docker for payments-platform?");
}

#[test]
fn jackin_docker_setting_confirm_and_save() {
    let app = scenario("jackin-docker-setting", "docker-setting", 120, 40);
    app.wait_text("Current directory");
    turn_docker_on(&app);
    let on = app.wait_text("4 risks");
    assert!(text(&on).contains("On · jackin-local · host.openshell.internal:18476"));
    assert!(text(&on).contains("• 1 change"));

    app.key("ctrl+s");
    let preview = app.wait_text("Save workspace");
    assert!(text(&preview).contains("Docker"));
    assert!(text(&preview).contains("off → on"));
    app.key("right");
    app.key("enter");
    app.wait_text("Saving payments-platform");
    app.advance_clock(1000);
    app.wait_text("Workspace payments-platform saved");
    let manager = app.wait_text("docker on");
    assert!(text(&manager).contains("dirty exit"));

    app.key("e");
    app.wait_text("payments-platform › edit");
    tab_to(&app, "Docker for the agent");
    let reopened = app.settle();
    assert!(text(&reopened).contains("On · jackin-local"));
    assert!(text(&reopened).contains("Space Toggle  i Risks"));
    assert!(!text(&reopened).contains("• 1 change"));
    app.visual("functional/jackin_docker_setting");
    app.finish();
}

#[test]
fn jackin_cockpit_docker_names_key_and_egress_bypass() {
    let app = scenario("jackin-cockpit-docker", "docker-setting", 120, 40);
    app.wait_text("Current directory");
    turn_docker_on(&app);
    app.key("ctrl+s");
    app.wait_text("Save workspace");
    app.key("right");
    app.key("enter");
    app.wait_text("Saving payments-platform");
    app.advance_clock(1000);
    app.wait_text("Workspace payments-platform saved");
    app.key("escape");
    app.wait_text("Current directory");
    app.key("enter");
    app.wait_text("Launch · choose Agent");
    app.key("space");
    app.key("enter");
    let cockpit = app.wait_text("Loading the-architect");
    let screen = text(&cockpit);
    assert!(screen.contains("the agent can read the engine key"));
    assert!(screen.contains("containers bypass the sandbox"));
    assert!(screen.contains("network policy · i shows all 4 risks"));
    app.key("ctrl+c");
    app.finish();
}

#[test]
fn jackin_docker_setting_no_engine() {
    let app = scenario("jackin-docker-no-engine", "docker-no-engine", 120, 40);
    app.wait_text("Current directory");
    turn_docker_on(&app);
    let on = app.wait_text("has no Docker engine");
    assert!(text(&on).contains("jackin gateway add"));
    assert!(text(&on).contains("4 risks"));
    assert!(text(&on).contains("Space Toggle  i Risks"));
    app.visual("functional/jackin_docker_no_engine");
    app.key("escape");
    app.key("escape");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.key("enter");
    app.wait_text("Current directory");
    app.finish();
}

const DOCTOR_CHECKS: [&str; 7] = [
    "OpenShell binary",
    "SSH binary",
    "Git binary",
    "Gateway registered",
    "Gateway health",
    "Pinned version",
    "Engine socket",
];

fn list_row(screen: &tui_test::TerminalSnapshot, label: &str) -> String {
    let rows: Vec<String> = find_all(screen, label)
        .into_iter()
        .map(|p| row_text(screen, u32::from(p.y)))
        .filter(|row| row.contains('│'))
        .collect();
    assert_eq!(rows.len(), 1, "one list row {label:?}:\n{}", text(screen));
    rows[0].clone()
}

#[test]
fn jackin_doctor_failure_and_rerun() {
    let app = scenario("jackin-doctor", "doctor", 120, 40);
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("gateway failing"));
    app.type_text("D");
    let doctor = app.wait_text("Engine socket");
    for label in DOCTOR_CHECKS {
        assert!(text(&doctor).contains(label), "check {label:?}");
    }
    assert!(text(&doctor).contains("1 failing"));
    assert!(text(&doctor).contains("1 warning"));
    assert!(list_row(&doctor, "Engine socket").contains("fail"));

    for _ in 0..6 {
        app.key("down");
    }
    let engine = app.wait_text("Start Docker or OrbStack");
    assert!(text(&engine).contains("Doctor › ") || text(&engine).contains("Doctor"));

    app.key("r");
    app.wait_text("Running checks…");
    app.advance_clock(900);
    let passing = app.wait_text("all checks pass");
    assert!(!text(&passing).contains("Start Docker or OrbStack"));
    app.key("escape");
    let healthy = app.wait_screen("manager after the passing run", |s| {
        let t = text(s);
        t.contains("Current directory") && !t.contains("gateway failing")
    });
    assert!(text(&healthy).contains("gateway"));
    app.type_text("D");
    app.wait_text("all checks pass");

    app.key("r");
    app.wait_text("Running checks…");
    app.advance_clock(900);
    let failed = app.wait_text("connection refused");
    assert!(list_row(&failed, "Gateway health").contains("fail"));
    assert!(list_row(&failed, "Engine socket").contains("ok"));
    assert!(list_row(&failed, "Git binary").contains("ok"));
    app.key("g");
    for _ in 0..4 {
        app.key("down");
    }
    let detail = app.wait_text("Expected");
    for needle in ["Gateway health", "Status", "Found", "Expected"] {
        assert!(
            text(&detail).contains(needle),
            "{needle:?}:\n{}",
            text(&detail)
        );
    }
    app.visual("functional/jackin_doctor");

    app.key("escape");
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("gateway failing"));

    app.resize(80, 24);
    app.type_text("D");
    app.wait_text("Engine socket");
    app.key("enter");
    let detail = app.wait_text("Expected");
    assert!(text(&detail).contains("Gateway"));
    app.finish();
}

fn binding_row(screen: &tui_test::TerminalSnapshot, agent: &str) -> String {
    let rows: Vec<String> = text(screen)
        .lines()
        .filter_map(|line| line.rsplit('│').next())
        .map(|cell| cell.replace('▎', "").trim().to_owned())
        .filter(|cell| cell.starts_with(agent) && !cell.starts_with("Codex model"))
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "one binding row {agent:?}:\n{}",
        text(screen)
    );
    rows[0].clone()
}

#[test]
fn jackin_role_binding_pick_and_clear() {
    let app = scenario("jackin-role-binding", "role-binding", 120, 40);
    app.wait_text("Account bindings");
    let roles = app.settle();
    for role in [
        "sentinel",
        "the-architect",
        "reviewer",
        "data-eng",
        "writer",
    ] {
        assert!(text(&roles).contains(role), "role {role:?}");
    }
    assert!(text(&roles).contains("5 roles"));
    assert!(text(&roles).contains("A launch uses --account"));
    for (role, chip) in [
        ("reviewer", "git"),
        ("data-eng", "path"),
        ("writer", "path"),
    ] {
        let row = list_row(&roles, role);
        assert!(
            row.contains(chip),
            "the {role} row keeps its source chip: {row}"
        );
    }
    let binding = binding_row(&roles, "Codex");
    assert!(binding.contains("Codex · Primary"), "{binding}");
    assert!(binding.contains("role binding"), "{binding}");

    app.key("tab");
    app.key("enter");
    let picker = app.wait_text("Bind account · sentinel · Codex");
    assert!(text(&picker).contains("Primary"));
    assert!(text(&picker).contains("Experiments"));
    app.key("escape");
    app.wait_absent("Bind account · sentinel · Codex");
    assert!(binding_row(&app.settle(), "Codex").contains("Codex · Primary"));

    app.key("enter");
    app.wait_text("Bind account · sentinel · Codex");
    app.key("down");
    app.key("enter");
    app.wait_text("Binding sentinel…");
    app.advance_clock(900);
    let failed = app.wait_text("Could not save the binding");
    assert!(text(&failed).contains("Nothing changed"));
    let unchanged = app.settle();
    let row = binding_row(&unchanged, "Codex");
    assert!(
        row.contains("Codex · Primary") && !row.contains("Experiments"),
        "the failed write must not change the row: {row}"
    );

    app.key("enter");
    app.wait_text("Bind account · sentinel · Codex");
    app.key("down");
    app.key("enter");
    app.advance_clock(900);
    app.wait_text("Bound Codex · Experiments to sentinel");
    let bound = app.settle();
    assert!(binding_row(&bound, "Codex").contains("Codex · Experiments"));

    app.key("d");
    app.advance_clock(900);
    let cleared = app.wait_text("u undoes");
    assert!(text(&cleared).contains("Cleared the Codex binding of sentinel"));
    assert!(
        text(&cleared).contains("Enter Pick  d Clear  u Undo"),
        "the status text leaves the key hints whole:\n{}",
        text(&cleared)
    );
    assert!(text(&cleared).contains("none · uses Codex · Primary ★"));
    app.key("u");
    app.advance_clock(900);
    app.wait_text("Restored the Codex binding of sentinel");
    assert!(binding_row(&app.settle(), "Codex").contains("Codex · Experiments"));
    app.key("d");
    app.advance_clock(900);
    app.wait_text("none · uses Codex · Primary ★");
    app.visual("functional/jackin_role_binding");

    app.resize(80, 24);
    app.key("escape");
    app.wait_absent("Account bindings");
    app.key("enter");
    app.wait_text("Account bindings");
    app.finish();
}

#[test]
fn jackin_launch_openshell_stages() {
    let mut args = vec!["--scenario", "launch-openshell", "--motion", "paused"];
    args.extend_from_slice(&["--frame", "90"]);
    let app = App::new(
        "jackin-launch-openshell",
        JACKIN,
        &args,
        120,
        40,
        "truecolor",
    );
    app.wait_text("stage 5 of 7 · Upload");
    let screen = app.settle();
    for label in [
        "Image build",
        "OpenShell provider",
        "Sandbox",
        "Credentials",
        "Upload",
        "Hooks",
        "Codex",
    ] {
        assert!(text(&screen).contains(label), "stage {label:?}");
    }
    assert!(text(&screen).contains("4/7 stages"));
    let sandbox = text(&screen)
        .split_whitespace()
        .find(|word| word.starts_with("jackin-"))
        .map(str::to_owned)
        .expect("a sandbox chip");
    assert_eq!(
        sandbox, "jackin-payment-b5df",
        "D07: jackin-<slug7>-<suffix4>"
    );
    assert!(sandbox.len() <= 19);
    let screen_text = text(&screen);
    let credentials = screen_text
        .lines()
        .skip_while(|l| !l.contains("credentials "))
        .take(2)
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(credentials.contains("Codex"), "{credentials}");
    assert!(!credentials.contains("Claude"), "{credentials}");
    assert!(!credentials.contains('…'), "clipped: {credentials}");
    let body: String = text(&screen).lines().skip(1).collect::<Vec<_>>().join("\n");
    for word in ["Docker", "Capsule", "Construct", "Derived Image"] {
        assert!(!body.contains(word), "demo word {word:?} leaked:\n{body}");
    }
    app.key("b");
    let log = app.wait_text("12 lines · following");
    assert!(!text(&log).contains("Docker build"));
    app.key("escape");
    app.visual("functional/jackin_launch_openshell");
    app.key("ctrl+c");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_role_editor_check_and_save() {
    let app = scenario("jackin-role-editor", "role-editor", 120, 40);
    app.wait_text("Roles › sentinel › edit");
    let editor = app.settle();
    for needle in [
        "sentinel",
        "policy.yaml",
        "Check passed",
        "Manifest",
        "Hooks",
        "Environment 2",
    ] {
        assert!(
            text(&editor).contains(needle),
            "{needle:?}:\n{}",
            text(&editor)
        );
    }

    app.key("enter");
    tab_to(&app, "Codex model");
    app.type_text("codex-fixture");
    app.key("tab");
    app.key("space");
    let confirm = app.wait_text("Turn on Docker for role sentinel?");
    for risk in DOCKER_RISKS {
        assert!(text(&confirm).contains(risk), "risk text {risk:?}");
    }
    app.key("right");
    app.key("enter");
    app.wait_absent("Turn on Docker for role sentinel?");
    let dirty = app.wait_text("2 changes");
    assert!(text(&dirty).contains("Manifest •"), "{}", text(&dirty));

    assert!(
        text(&dirty).contains("Not checked since your last edit"),
        "an edit makes the check stale:\n{}",
        text(&dirty)
    );
    assert!(!text(&dirty).contains("Check passed"));
    app.key("ctrl+s");
    app.advance_clock(600);
    let preview = app.wait_text("Save role");
    assert!(text(&preview).contains("none → codex-fixture"));
    assert!(text(&preview).contains("off → on"));
    app.key("escape");
    app.wait_absent("Save role");
    let kept = app.settle();
    assert!(!text(&kept).contains("Role sentinel saved"));
    assert!(text(&kept).contains("2 changes"));

    tab_to(&app, "Dockerfile");
    for _ in 0..10 {
        app.key("backspace");
    }
    app.type_text("Dockerfile.old");
    app.key("enter");
    app.key("ctrl+s");
    app.advance_clock(600);
    let unchecked = app.wait_text("Check failed · sentinel");
    assert!(text(&unchecked).contains("Dockerfile.old:12"));
    assert!(text(&unchecked).contains("nothing was written"));
    assert!(!text(&unchecked).contains("Save role"));
    app.key("escape");
    app.wait_absent("Check failed · sentinel");
    app.key("f5");
    app.advance_clock(600);
    let failed = app.wait_text("final stage must start FROM construct");
    assert!(text(&failed).contains("Dockerfile.old"));
    assert!(text(&failed).contains("Manifest !") || text(&failed).contains("Manifest •!"));

    app.key("ctrl+s");
    app.advance_clock(600);
    let blocked = app.wait_text("Check failed · sentinel");
    assert!(text(&blocked).contains("nothing was written"));
    app.key("escape");
    app.wait_absent("Check failed · sentinel");
    app.visual("functional/jackin_role_editor");

    app.resize(80, 24);
    let narrow = app.wait_text("1 error");
    assert!(text(&narrow).contains("Dockerfile"));
    app.key("escape");
    app.key("escape");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.key("enter");
    app.wait_text("Discarded 3 changes");
    app.finish();
}

fn submit_location(app: &App, location: &str) {
    app.type_text(location);
    app.key("enter");
    app.key("tab");
    app.key("tab");
    app.key("enter");
}

#[test]
fn jackin_roles_add_role() {
    let app = scenario("jackin-role-add", "role-binding", 120, 40);
    app.wait_text("Account bindings");
    assert!(!text(&app.settle()).contains("new-role"));
    app.key("a");
    app.wait_text("register a role");

    submit_location(&app, "roles/new-role");
    let invalid = app.settle();
    assert!(
        text(&invalid).contains("Use a directory such as"),
        "{}",
        text(&invalid)
    );
    assert!(
        text(&invalid).contains("register a role"),
        "the form stays open"
    );
    app.key("escape");
    app.wait_absent("register a role");

    app.key("a");
    app.wait_text("register a role");
    submit_location(&app, "~/roles/sentinel");
    app.wait_text("Adding role…");
    app.advance_clock(700);
    let taken = app.wait_text("Could not add role");
    assert!(text(&taken).contains("A role named sentinel is already registered."));
    assert!(text(&taken).contains("no role was registered"));
    app.key("escape");
    let again = app.wait_text("register a role");
    assert!(
        text(&again).contains("~/roles/sentinel"),
        "the typed location stays"
    );
    app.key("escape");
    app.wait_absent("register a role");

    app.key("a");
    app.wait_text("register a role");
    submit_location(&app, "~/roles/new-role");
    app.wait_text("Adding role…");
    app.advance_clock(700);
    let added = app.wait_text("Role new-role added");
    assert!(text(&added).contains("Roles › new-role") || text(&added).contains("Role · new-role"));
    assert!(text(&added).contains("6 roles"));
    assert!(!text(&added).contains("Not in this preview"));
    app.finish();
}

fn fixed_app(name: &str) -> App {
    scenario(name, "openshell-fixed", 120, 40)
}

fn open_fixed_editor(app: &App, downs: usize) -> TerminalSnapshot {
    app.wait_text("Current directory");
    for _ in 0..downs {
        app.key("down");
    }
    app.key("e");
    app.wait_text("Directory basename by default")
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows() - 1)
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    row_text(screen, u32::from(find(screen, needle).y))
}

fn unchecked_rows(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows() - 1)
        .map(|row| row_text(screen, row))
        .filter(|row| row.contains("[ ]"))
        .collect()
}

fn contrast(screen: &TerminalSnapshot, at: Point) -> f64 {
    let cell = screen
        .cell_at(u32::from(at.x), u32::from(at.y))
        .expect("cell in viewport");
    let luminance = |color: SnapshotColor| {
        let SnapshotColor::Rgb(r, g, b) = color else {
            panic!("a truecolor cell, got {color:?}");
        };
        let linear = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    };
    let (a, b) = (luminance(cell.fg()), luminance(cell.bg()));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn role_row(screen: &TerminalSnapshot, role: &str) -> String {
    row_with(screen, &format!("] {role}"))
}

fn inside(point: Point) -> Point {
    Point {
        x: point.x + 2,
        ..point
    }
}

fn refused_with(app: &App, reason: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("status reads {reason}"), |s| {
        footer(s).contains(reason)
    })
}

fn status_expired(app: &App, reason: &str) -> TerminalSnapshot {
    app.advance_clock(6_000);
    app.wait_screen("status expired", |s| !footer(s).contains(reason))
}

fn nothing_changed(screen: &TerminalSnapshot) {
    assert!(
        !text(screen).contains("change"),
        "a refusal marks no change:\n{}",
        text(screen)
    );
}

fn no_change_marker(screen: &TerminalSnapshot) {
    let t = text(screen);
    assert!(
        !t.contains("1 change") && !t.contains(" changes"),
        "a refusal marks no change:\n{t}"
    );
}

fn leave_without_a_prompt(app: &App) {
    app.key("ctrl+s");
    let nothing = app.wait_text("Nothing to save");
    assert!(!text(&nothing).contains("Save workspace"));
    app.key("esc");
    app.key("esc");
    let manager = app.wait_text("Current directory");
    assert!(!text(&manager).contains("Unsaved changes"));
}

#[test]
fn jackin_fixed_keep_awake_refuses_by_click_and_key() {
    let reason = Fixed::openshell().keep_awake.text;
    let app = fixed_app("jackin-fixed-keep-awake");
    let editor = open_fixed_editor(&app, 1);
    let row = row_with(&editor, "Keep awake");
    assert!(
        row.contains("[ ] Keep awake") && row.contains(&reason),
        "{row}"
    );
    assert!(!text(&editor).contains("macOS only"));
    app.visual("functional/jackin_fixed_editor_general");

    app.click(inside(find(&editor, "Keep awake")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Keep awake").contains("[ ] Keep awake"));
    assert!(row_with(&clicked, "Keep awake").contains('▎'));
    assert!(!footer(&clicked).contains("Space Toggle"));
    nothing_changed(&clicked);
    status_expired(&app, &reason);

    app.key("space");
    let keyed = refused_with(&app, &reason);
    assert!(row_with(&keyed, "Keep awake").contains("[ ] Keep awake"));
    nothing_changed(&keyed);
    let gone = status_expired(&app, &reason);
    app.key("up");
    app.click(find(&gone, "Roles"));
    let later = app.wait_text("Allowed roles");
    assert!(!footer(&later).contains(&reason));
    app.key("esc");
    let manager = app.wait_text("Current directory");
    assert!(!text(&manager).contains("Unsaved changes"));
    app.finish();
}

#[test]
fn jackin_fixed_git_pull_refuses_by_key_and_click() {
    let reason = Fixed::openshell().git_pull.text;
    let app = fixed_app("jackin-fixed-git-pull");
    let editor = open_fixed_editor(&app, 1);
    let row = row_with(&editor, "Git pull before launch");
    assert!(
        row.contains("[ ] Git pull before launch") && row.contains(&reason),
        "{row}"
    );

    for _ in 0..4 {
        app.key("tab");
    }
    app.wait_screen("focus reaches git pull", |s| {
        row_with(s, "Git pull before launch").contains('▎')
    });
    app.key("space");
    let keyed = refused_with(&app, &reason);
    assert!(row_with(&keyed, "Git pull before launch").contains("[ ] Git pull"));
    assert!(row_with(&keyed, "Git pull before launch").contains('▎'));
    nothing_changed(&keyed);
    app.visual("functional/jackin_fixed_editor_general_refused");
    status_expired(&app, &reason);

    app.click(inside(find(&app.screen(), "Git pull before launch")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Git pull before launch").contains("[ ] Git pull"));
    nothing_changed(&clicked);
    status_expired(&app, &reason);
    leave_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_dirty_exit_refuses_the_list_by_key_and_click() {
    let reason = Fixed::openshell().dirty_exit.text;
    let app = fixed_app("jackin-fixed-dirty-exit");
    let editor = open_fixed_editor(&app, 1);
    assert!(text(&editor).contains(&reason));
    let value = "ask · show the exit dialog";
    assert!(text(&editor).contains(value));

    let field = find(&editor, value);
    app.click(Point {
        x: field.x + 2,
        y: field.y,
    });
    let clicked = refused_with(&app, &reason);
    assert!(!text(&clicked).contains("keep · preserve changes silently"));
    assert!(!footer(&clicked).contains("Space Toggle"));
    nothing_changed(&clicked);
    status_expired(&app, &reason);

    for key in ["enter", "space", "down"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(text(&keyed).contains(value), "{key}");
        assert!(
            !text(&keyed).contains("keep · preserve changes silently"),
            "{key}"
        );
        nothing_changed(&keyed);
        status_expired(&app, &reason);
    }
    leave_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_working_directory_refuses_the_picker_by_key_and_click() {
    let reason = Fixed::openshell().workdir.text;
    let app = fixed_app("jackin-fixed-workdir");
    let editor = open_fixed_editor(&app, 1);
    assert!(row_with(&editor, "Choose…").contains("/sandbox/project"));
    assert!(text(&editor).contains(&reason));
    assert!(!text(&editor).contains("Inside the sandbox"));

    app.click(inside(find(&editor, "Choose…")));
    let clicked = refused_with(&app, &reason);
    assert!(!text(&clicked).contains("mount · "), "no picker opens");
    assert!(!footer(&clicked).contains("Enter Choose…"));
    nothing_changed(&clicked);
    status_expired(&app, &reason);

    app.key("enter");
    let keyed = refused_with(&app, &reason);
    assert!(!text(&keyed).contains("mount · "), "no picker opens");
    assert!(row_with(&keyed, "Choose…").contains("/sandbox/project"));
    nothing_changed(&keyed);
    status_expired(&app, &reason);
    leave_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_editor_refusals_carry_the_full_reason_at_80_by_24() {
    let fixed = Fixed::openshell();
    let app = scenario("jackin-fixed-editor-narrow", "openshell-fixed", 80, 24);
    let editor = open_fixed_editor(&app, 1);

    for (target, reason) in [
        ("Choose…", fixed.workdir.text.clone()),
        ("Keep awake", fixed.keep_awake.text.clone()),
        ("Git pull before launch", fixed.git_pull.text.clone()),
        ("ask · show the exit dialog", fixed.dirty_exit.text.clone()),
    ] {
        let at = find(&editor, target);
        app.click(inside(at));
        let refused = refused_with(&app, &reason);
        assert!(
            footer(&refused).contains(&reason),
            "{target}: the status carries the whole reason:\n{}",
            text(&refused)
        );
        nothing_changed(&refused);
        status_expired(&app, &reason);
    }
    leave_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_project_row_keeps_its_paint_and_refuses_every_edit() {
    let reason = Fixed::openshell().mounts.text;
    let app = fixed_app("jackin-fixed-project");
    open_fixed_editor(&app, 1);
    app.key("]");
    app.key("enter");
    let project = app.wait_screen("the Project list has focus", |s| {
        text(s).contains("Destination") && row_with(s, "~/src/payments-platform").contains('▎')
    });
    let row = row_with(&project, "~/src/payments-platform");
    for part in ["/sandbox/project", "rw", "clone", "host"] {
        assert!(row.contains(part), "{part} in {row}");
    }
    assert!(!text(&project).contains("shared") && !text(&project).contains("worktree"));
    assert!(text(&project).contains(&reason));
    assert!(!text(&project).contains("Read-only") && !footer(&project).contains("Isolation"));
    assert!(!footer(&project).contains("Remove") && !footer(&project).contains("Add mount"));
    assert!(!footer(&project).contains("Open source"));

    let add = contrast(&project, find(&project, "+ Add mount"));
    assert!(add < 4.5, "the Add row is drawn fixed, contrast {add:.2}");
    app.visual("functional/jackin_fixed_editor_project");

    for key in [
        "enter", "e", "n", "r", "i", "1", "2", "3", "d", "delete", "u", "a",
    ] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(
            row_with(&keyed, "~/src/payments-platform").contains("rw"),
            "{key}"
        );
        assert!(
            row_with(&keyed, "~/src/payments-platform").contains("clone"),
            "{key}"
        );
        assert!(!text(&keyed).contains("Add mount…"), "{key}");
        nothing_changed(&keyed);
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "/sandbox/project")));
    refused_with(&app, &reason);
    status_expired(&app, &reason);

    app.key("down");
    let added = app.wait_screen("cursor reaches the Add row", |s| {
        row_with(s, "+ Add mount").contains('▎')
    });
    assert!(!footer(&added).contains("Add mount"));
    for key in ["enter", "a"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(!text(&keyed).contains("Add mount…"), "{key}");
        nothing_changed(&keyed);
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "+ Add mount")));
    refused_with(&app, &reason);
    status_expired(&app, &reason);
    leave_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_workspace_account_toggle_refuses_and_prefer_stays_live() {
    let reason = Fixed::openshell().enable_workspace.text;
    let app = fixed_app("jackin-fixed-accounts");
    open_fixed_editor(&app, 1);
    for _ in 0..4 {
        app.key("]");
    }
    app.key("enter");
    let accounts = app.wait_screen("the account list has focus", |s| {
        text(s).contains("Active accounts") && footer(s).contains("Prefer")
    });
    assert!(
        unchecked_rows(&accounts).is_empty(),
        "every account is active"
    );
    assert!(text(&accounts).contains("[✓]"));
    assert!(!footer(&accounts).contains("Enable / disable"));
    assert!(footer(&accounts).contains("Prefer"));
    app.visual("functional/jackin_fixed_editor_accounts");

    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(unchecked_rows(&keyed).is_empty(), "{key}");
        nothing_changed(&keyed);
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "Personal")));
    refused_with(&app, &reason);
    status_expired(&app, &reason);
    app.key("ctrl+s");
    app.wait_text("Nothing to save");

    app.key("down");
    app.key("down");
    app.key("p");
    let preferred = app.wait_text("Preferred for");
    assert!(text(&preferred).contains("1 change"));
    assert!(!footer(&preferred).contains(&reason));
    app.key("esc");
    app.key("esc");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.wait_text("Enter Discard");
    app.key("enter");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_fixed_last_allowed_role_cannot_be_unchecked() {
    let reason = Fixed::openshell().last_role;
    let app = fixed_app("jackin-fixed-last-role");
    open_fixed_editor(&app, 3);
    app.key("]");
    app.key("]");
    app.key("enter");
    let roles = app.wait_text("Allowed roles");
    assert!(role_row(&roles, "backend").contains("[✓]"));
    assert!(role_row(&roles, "the-architect").contains("[ ]"));

    app.key("end");
    let stale = app.wait_text("] ghost-role");
    let ghost = role_row(&stale, "ghost-role");
    assert!(
        ghost.contains("[✓]") && ghost.contains("not in registry"),
        "{ghost}"
    );
    app.key("home");
    app.wait_text("] the-architect");

    app.wait_screen("cursor on the first role", |s| {
        role_row(s, "the-architect").contains('▎')
    });
    app.key("down");
    app.wait_screen("cursor on backend", |s| {
        role_row(s, "backend").contains('▎')
    });
    app.key("space");
    let refused = refused_with(&app, &reason);
    assert!(role_row(&refused, "backend").contains("[✓]"));
    nothing_changed(&refused);
    status_expired(&app, &reason);

    app.key("up");
    app.key("space");
    let added = app.wait_text("the-architect · allowed");
    assert!(role_row(&added, "the-architect").contains("[✓]"));
    assert!(text(&added).contains("1 change"));
    app.key("down");
    app.key("space");
    let removed = app.wait_text("backend · not allowed");
    assert!(role_row(&removed, "backend").contains("[ ]"));
    assert!(!footer(&removed).contains(&reason));

    app.key("up");
    app.key("space");
    let last = refused_with(&app, &reason);
    assert!(role_row(&last, "the-architect").contains("[✓]"));
    app.key("esc");
    app.key("esc");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.wait_text("Enter Discard");
    app.key("enter");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_fixed_last_allowed_role_refuses_a_click() {
    let reason = Fixed::openshell().last_role;
    let app = fixed_app("jackin-fixed-last-role-click");
    open_fixed_editor(&app, 3);
    app.key("]");
    app.key("]");
    app.key("enter");
    let roles = app.wait_text("Allowed roles");
    assert!(role_row(&roles, "backend").contains("[✓]"));

    app.click(inside(find(&roles, "] backend")));
    app.wait_screen("the first click moves the cursor", |s| {
        role_row(s, "backend").contains('▎')
    });
    app.click(inside(find(&app.screen(), "] backend")));
    let refused = refused_with(&app, &reason);
    assert!(role_row(&refused, "backend").contains("[✓]"));
    nothing_changed(&refused);
    status_expired(&app, &reason);
    leave_without_a_prompt(&app);
    app.finish();
}

fn open_fixed_settings(app: &App, tab: &str, presses: usize) -> TerminalSnapshot {
    app.wait_text("Current directory");
    app.key("s");
    app.wait_text("Settings › global › General");
    for _ in 0..presses {
        app.key("]");
    }
    app.wait_text(&format!("Settings › global › {tab}"));
    app.key("enter");
    app.wait_screen("the tab body has focus", |s| {
        !footer(s).contains("Enter Edit fields")
    })
}

fn leave_settings_without_a_prompt(app: &App) {
    app.key("ctrl+s");
    let nothing = app.wait_text("Nothing to save");
    assert!(!text(&nothing).contains("Save settings"));
    app.key("esc");
    app.key("esc");
    let manager = app.wait_text("Current directory");
    assert!(!text(&manager).contains("Unsaved changes"));
}

#[test]
fn jackin_fixed_settings_commits_refuse_by_key_and_click() {
    let reason = Fixed::openshell().commits.text;
    let app = fixed_app("jackin-fixed-commits");
    let general = open_fixed_settings(&app, "General", 0);
    for label in ["Add Co-authored-by trailer", "Sign off commits (DCO)"] {
        assert!(row_with(&general, label).contains("[ ]"), "{label}");
    }
    assert!(text(&general).contains(&reason));
    assert!(text(&general).contains("None · jackin adds no trailers to commits"));
    assert!(!text(&general).contains("Two independent flags"));
    assert!(!footer(&general).contains("Space Toggle"));
    app.visual("functional/jackin_fixed_settings_commits");

    app.key("space");
    let keyed = refused_with(&app, &reason);
    assert!(row_with(&keyed, "Add Co-authored-by trailer").contains("[ ]"));
    assert!(row_with(&keyed, "Add Co-authored-by trailer").contains('▎'));
    no_change_marker(&keyed);
    status_expired(&app, &reason);

    app.key("down");
    app.wait_screen("focus reaches the DCO box", |s| {
        row_with(s, "Sign off commits (DCO)").contains('▎')
    });
    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(
            row_with(&keyed, "Sign off commits (DCO)").contains("[ ]"),
            "{key}"
        );
        no_change_marker(&keyed);
        status_expired(&app, &reason);
    }

    app.click(inside(find(&app.screen(), "Add Co-authored-by trailer")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Add Co-authored-by trailer").contains("[ ]"));
    assert!(row_with(&clicked, "Add Co-authored-by trailer").contains('▎'));
    no_change_marker(&clicked);
    status_expired(&app, &reason);
    app.click(inside(find(&app.screen(), "Sign off commits (DCO)")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Sign off commits (DCO)").contains("[ ]"));
    no_change_marker(&clicked);
    status_expired(&app, &reason);
    leave_settings_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_settings_global_mounts_refuse_every_add() {
    let reason = Fixed::openshell().mounts.text;
    let app = fixed_app("jackin-fixed-global-mounts");
    let mounts = open_fixed_settings(&app, "Mounts", 1);
    let add = row_with(&mounts, "+ Add mount");
    assert!(add.contains('▎'), "{add}");
    assert!(text(&mounts).contains(&reason));
    assert!(
        !text(&mounts).contains("/home/agent"),
        "the list holds only the Add row:\n{}",
        text(&mounts)
    );
    assert!(!footer(&mounts).contains("Add mount"));
    let ratio = contrast(&mounts, find(&mounts, "+ Add mount"));
    assert!(
        ratio < 4.5,
        "the Add row is drawn fixed, contrast {ratio:.2}"
    );
    app.visual("functional/jackin_fixed_settings_mounts");

    for key in ["enter", "a"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(!text(&keyed).contains("Add mount…"), "{key}");
        assert!(!text(&keyed).contains("Host directory"), "{key}");
        no_change_marker(&keyed);
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "+ Add mount")));
    let clicked = refused_with(&app, &reason);
    assert!(!text(&clicked).contains("Host directory"));
    no_change_marker(&clicked);
    status_expired(&app, &reason);
    leave_settings_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_settings_trust_refuses_the_toggle_and_shows_the_source() {
    let reason = Fixed::openshell().trust.text;
    let app = fixed_app("jackin-fixed-trust");
    let trust = open_fixed_settings(&app, "Trust", 4);
    let rows: Vec<String> = (0..trust.rows() - 1)
        .map(|row| row_text(&trust, row))
        .filter(|row| row.contains("[✓] trusted") || row.contains("[ ] untrusted"))
        .collect();
    assert_eq!(rows.len(), 4, "{}", text(&trust));
    assert!(
        rows.iter().all(|row| row.contains("[✓] trusted")),
        "{rows:?}"
    );
    assert!(!text(&trust).contains("untrusted ["));
    assert!(text(&trust).contains(&reason));
    assert!(!text(&trust).contains("blocks + Load role"));
    assert!(!footer(&trust).contains("Toggle trust"));
    assert!(footer(&trust).contains("o Show source"));
    assert!(!footer(&trust).contains("Open source"));
    for cell in ["github.com/chainargos/roles", "[✓] trusted", "4 roles"] {
        let at = find_all(&trust, cell)[0];
        let ratio = contrast(&trust, at);
        assert!(ratio >= 4.5, "{cell} is legible, contrast {ratio:.2}");
    }
    app.visual("functional/jackin_fixed_settings_trust");

    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(!text(&keyed).contains("[ ] untrusted"), "{key}");
        no_change_marker(&keyed);
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "github.com/chainargos/roles")));
    let clicked = refused_with(&app, &reason);
    assert!(!text(&clicked).contains("[ ] untrusted"));
    no_change_marker(&clicked);
    status_expired(&app, &reason);

    app.key("down");
    app.key("o");
    let shown = app.wait_text("Source · github.com/acme-labs/roles-experimental");
    assert!(!text(&shown).contains("Opened") && !text(&shown).contains("Revealed"));
    no_change_marker(&shown);
    app.key("down");
    app.key("down");
    app.key("o");
    app.wait_text("Source · git@corp:infra/roles");
    leave_settings_without_a_prompt(&app);
    app.finish();
}

fn agent_at(screen: &TerminalSnapshot, label: &str) -> Point {
    find_all(screen, label)
        .into_iter()
        .find(|point| point.x < 12)
        .unwrap_or_else(|| panic!("agent row {label}:\n{}", text(screen)))
}

fn agent_row(screen: &TerminalSnapshot, label: &str) -> String {
    row_text(screen, u32::from(agent_at(screen, label).y))
}

#[test]
fn jackin_fixed_settings_agents_refuse_every_row() {
    let fixed = Fixed::openshell();
    let app = fixed_app("jackin-fixed-agents");
    let agents = open_fixed_settings(&app, "Agents", 3);
    assert!(agent_row(&agents, "Codex").contains("sync"));
    for agent in ["Claude Code", "Amp", "Kimi Code", "OpenCode", "Grok Build"] {
        assert!(agent_row(&agents, agent).contains("ignore"), "{agent}");
    }
    for note in &fixed.agent_notes {
        assert!(text(&agents).contains(note.as_str()), "{note}");
    }
    assert!(!text(&agents).contains("sync mirrors the host login"));
    assert!(
        !footer(&agents).contains("Cycle mode") && !footer(&agents).contains("Remove override")
    );
    assert!(footer(&agents).contains("Manage accounts"));
    for cell in ["Claude Code", "Codex", "Amp"] {
        let ratio = contrast(&agents, agent_at(&agents, cell));
        assert!(ratio >= 4.5, "{cell} is legible, contrast {ratio:.2}");
    }

    for key in ["space", "enter", "d"] {
        app.key(key);
        let keyed = refused_with(&app, &fixed.agent_other.text);
        assert!(agent_row(&keyed, "Claude Code").contains("ignore"), "{key}");
        no_change_marker(&keyed);
        status_expired(&app, &fixed.agent_other.text);
    }
    app.click(inside(agent_at(&app.screen(), "Claude Code")));
    refused_with(&app, &fixed.agent_other.text);
    status_expired(&app, &fixed.agent_other.text);

    app.key("down");
    app.wait_screen("cursor on Codex", |s| agent_row(s, "Codex").contains('▎'));
    for key in ["enter", "d", "space"] {
        app.key(key);
        let keyed = refused_with(&app, &fixed.agent_launchable.text);
        assert!(agent_row(&keyed, "Codex").contains("sync"), "{key}");
        no_change_marker(&keyed);
        if key != "space" {
            status_expired(&app, &fixed.agent_launchable.text);
        }
    }
    app.visual("functional/jackin_fixed_settings_agents");
    status_expired(&app, &fixed.agent_launchable.text);

    app.click(inside(agent_at(&app.screen(), "Codex")));
    let clicked = refused_with(&app, &fixed.agent_launchable.text);
    assert!(agent_row(&clicked, "Codex").contains("sync"));
    no_change_marker(&clicked);
    status_expired(&app, &fixed.agent_launchable.text);
    for _ in 0..4 {
        app.key("down");
    }
    app.wait_screen("cursor on Grok Build", |s| {
        agent_row(s, "Grok Build").contains('▎')
    });
    app.key("space");
    refused_with(&app, &fixed.agent_other.text);

    app.key("ctrl+s");
    let nothing = app.wait_text("Nothing to save");
    assert!(!text(&nothing).contains("Save settings"));
    app.key("c");
    let accounts = app.wait_screen("c opens the Accounts page", |s| {
        text(s).contains("Accounts ›") && !text(s).contains("Settings ›")
    });
    assert!(!text(&accounts).contains("Unsaved changes"));
    app.key("esc");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_fixed_settings_role_overrides_refuse_and_the_scope_lists_global() {
    let reason = Fixed::openshell().role_env.text;
    let app = fixed_app("jackin-fixed-role-env");
    let env = open_fixed_settings(&app, "Environments", 2);
    assert!(row_with(&env, "EDITOR").contains("plain"));
    assert!(text(&env).contains("Role overrides"));
    assert!(text(&env).contains(&reason));
    for hint in ["Scope…", "1Password…"] {
        assert!(!footer(&env).contains(hint), "{hint}");
    }

    for row in ["CARGO_NET_GIT_FET", "+ Add environment variable"] {
        app.key("down");
        app.wait_screen("the cursor steps down", |s| {
            (0..s.rows())
                .map(|r| row_text(s, r))
                .any(|r| r.contains(row) && r.contains('▎'))
        });
    }
    app.key("down");
    let add = app.wait_screen("cursor on the Add role row", |s| {
        row_with(s, "+ Add role override…").contains('▎')
    });
    assert!(!footer(&add).contains("Add role override"));
    let ratio = contrast(&add, find(&add, "+ Add role override…"));
    assert!(
        ratio < 4.5,
        "the Add row is drawn fixed, contrast {ratio:.2}"
    );
    let ratio = contrast(&add, find(&add, "EDITOR"));
    assert!(
        ratio >= 4.5,
        "the data row stays legible, contrast {ratio:.2}"
    );
    app.visual("functional/jackin_fixed_settings_env");

    for key in ["enter", "a"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(!text(&keyed).contains("no overrides yet"), "{key}");
        no_change_marker(&keyed);
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "+ Add role override…")));
    let clicked = refused_with(&app, &reason);
    assert!(!text(&clicked).contains("no overrides yet"));
    no_change_marker(&clicked);
    status_expired(&app, &reason);

    for _ in 0..3 {
        app.key("up");
    }
    app.wait_screen("cursor on EDITOR", |s| row_with(s, "EDITOR").contains('▎'));
    app.key("s");
    let scoped = refused_with(&app, &reason);
    assert!(!text(&scoped).contains("Scope for EDITOR"));
    no_change_marker(&scoped);
    status_expired(&app, &reason);

    app.key("a");
    let form = app.wait_screen("the form shows its Scope field and value", |s| {
        let labels = find_all(s, "Scope");
        text(s).contains("New global environment key")
            && labels.len() == 1
            && find_all(s, "global").iter().any(|p| p.y == labels[0].y + 1)
    });
    let label = find(&form, "Scope");
    let value = find_all(&form, "global")
        .into_iter()
        .find(|p| p.y == label.y + 1)
        .expect("the scope value sits under its label");
    app.click(value);
    let select = app.wait_screen("the scope list opens", |s| {
        find_all(s, "global")
            .into_iter()
            .filter(|p| p.y > label.y)
            .count()
            >= 2
    });
    for role in ["the-architect", "backend", "reviewer", "sre"] {
        assert!(
            !text(&select).contains(&format!("role {role}")),
            "the scope select lists only global:\n{}",
            text(&select)
        );
    }
    app.key("esc");
    app.key("esc");
    app.wait_text("Role overrides");
    leave_settings_without_a_prompt(&app);
    app.finish();
}

#[test]
fn jackin_fixed_env_sources_refuse_the_radio_options_and_the_p_key() {
    let reason = Fixed::openshell().env_sources.text;
    let app = fixed_app("jackin-fixed-env-sources");
    open_fixed_settings(&app, "Environments", 2);
    app.key("p");
    let p = refused_with(&app, &reason);
    assert!(!text(&p).contains("1Password"), "no 1Password flow opens");
    assert!(row_with(&p, "EDITOR").contains("plain"));
    no_change_marker(&p);
    status_expired(&app, &reason);

    app.key("a");
    app.wait_text("New global environment key");
    app.key("tab");
    let form = app.wait_screen("focus reaches the source radio", |s| {
        row_with(s, "Plain text").contains('▎')
    });
    assert!(row_with(&form, "Plain text").contains("(●)"));
    assert!(row_with(&form, "1Password item").contains("( )"));
    assert!(row_with(&form, "Host environment variable").contains("( )"));
    assert!(text(&form).contains(&reason));
    assert!(text(&form).contains("value is stored as plain text"));
    assert!(!text(&form).contains("recommended for secrets"));
    let ratio = contrast(&form, find(&form, &reason));
    assert!(ratio >= 4.5, "the reason is legible, contrast {ratio:.2}");

    app.key("down");
    app.wait_screen("cursor on the 1Password option", |s| {
        row_with(s, "1Password item").contains('▎')
    });
    app.visual("functional/jackin_fixed_env_form");
    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(row_with(&keyed, "Plain text").contains("(●)"), "{key}");
        assert!(row_with(&keyed, "1Password item").contains("( )"), "{key}");
        assert!(text(&keyed).contains("Value for the key"), "{key}");
        assert!(!text(&keyed).contains("not chosen"), "{key}");
        status_expired(&app, &reason);
    }
    app.key("down");
    app.wait_screen("cursor on the host option", |s| {
        row_with(s, "Host environment variable").contains('▎')
    });
    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(row_with(&keyed, "Plain text").contains("(●)"), "{key}");
        assert!(
            row_with(&keyed, "Host environment variable").contains("( )"),
            "{key}"
        );
        assert!(
            !text(&keyed).contains("Forwarded from the host shell"),
            "{key}"
        );
        status_expired(&app, &reason);
    }

    app.click(inside(find(&app.screen(), "1Password item")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Plain text").contains("(●)"));
    assert!(!text(&clicked).contains("not chosen"));
    status_expired(&app, &reason);
    app.click(inside(find(&app.screen(), "Host environment variable")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Plain text").contains("(●)"));
    assert!(!text(&clicked).contains("Forwarded from the host shell"));
    status_expired(&app, &reason);

    app.key("esc");
    let closed = app.wait_absent("New global environment key");
    no_change_marker(&closed);
    leave_settings_without_a_prompt(&app);
    app.finish();
}

fn type_key_and_value(app: &App, key: &str, value: &str) {
    app.key("a");
    app.wait_text("New global environment key");
    app.paste(key);
    app.key("tab");
    app.key("tab");
    app.wait_screen("the value is being edited", |s| {
        footer(s).contains("Enter Apply")
    });
    app.paste(value);
    app.key("enter");
    for stop in ["▾", "▎Cancel", "▎Save"] {
        app.key("tab");
        app.wait_screen("focus walks to the Save button", |s| {
            (0..s.rows())
                .map(|r| row_text(s, r))
                .any(|r| r.contains('│') && r.contains(stop) && (stop != "▾" || r.contains('▎')))
        });
    }
    app.key("enter");
}

#[test]
fn jackin_fixed_env_form_refuses_a_credential_reference_and_notes_a_secret_name() {
    let fixed = Fixed::openshell();
    let app = fixed_app("jackin-fixed-env-reference");
    open_fixed_settings(&app, "Environments", 2);

    app.key("a");
    let form = app.wait_text("New global environment key");
    assert!(!text(&form).contains(&fixed.env_secret_note[0]));
    app.paste("EDITOR_THEME");
    app.key("tab");
    let plain = app.wait_screen("the key is committed", |s| {
        row_with(s, "Plain text").contains('▎')
    });
    assert!(!text(&plain).contains(&fixed.env_secret_note[0]));
    app.key("shift+tab");
    app.key("enter");
    app.key("ctrl+u");
    app.paste("DEPLOY_TOKEN");
    app.key("tab");
    let noted = app.wait_screen("the form shows both note lines", |s| {
        text(s).contains(&fixed.env_secret_note[0]) && text(s).contains(&fixed.env_secret_note[1])
    });
    assert!(text(&noted).contains("DEPLOY_TOKEN"), "the input is kept");
    app.key("esc");
    app.wait_absent("New global environment key");

    for reference in [
        "op://vault/item/field",
        "OP://V/I/F",
        "openshell:resolve:env:X",
    ] {
        type_key_and_value(&app, "DEPLOY_TOKEN", reference);
        let refused = app.wait_text(&format!("! {}", fixed.env_reference));
        assert!(
            text(&refused).contains("New global environment key"),
            "{reference}"
        );
        assert!(text(&refused).contains("DEPLOY_TOKEN"), "{reference}");
        app.key("esc");
        let closed = app.wait_absent("New global environment key");
        assert!(
            text(&closed).contains("2 vars"),
            "{reference} was not stored"
        );
        assert!(
            !text(&closed).contains("DEPLOY_TOKEN"),
            "{reference} was not stored"
        );
    }

    type_key_and_value(&app, "DEPLOY_TOKEN", "ghp_abc");
    let saved = app.wait_text("DEPLOY_TOKEN · added · save to apply");
    assert!(text(&saved).contains("3 vars"));
    let added = find_all(&saved, "DEPLOY_TOKEN")[0];
    assert!(row_text(&saved, u32::from(added.y)).contains("plain"));
    app.key("esc");
    app.key("esc");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.wait_text("Enter Discard");
    app.key("enter");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_fixed_workspace_env_keeps_the_role_scope_live_and_refuses_the_sources() {
    let reason = Fixed::openshell().env_sources.text;
    let app = fixed_app("jackin-fixed-workspace-env");
    open_fixed_editor(&app, 1);
    for _ in 0..3 {
        app.key("]");
    }
    app.key("enter");
    let env = app.wait_screen("the env list has focus", |s| {
        text(s).contains("Role overrides") && footer(s).contains("m Show")
    });
    assert!(footer(&env).contains("Scope…"));
    assert!(!footer(&env).contains("1Password…"));

    app.key("p");
    let keyed = refused_with(&app, &reason);
    assert!(
        !text(&keyed).contains("1Password"),
        "no 1Password flow opens"
    );
    no_change_marker(&keyed);
    status_expired(&app, &reason);

    app.key("s");
    let scope = app.wait_text("Scope for DATABASE_URL");
    assert!(text(&scope).contains("role the-architect"));
    app.key("esc");
    app.wait_absent("Scope for DATABASE_URL");

    for _ in 0..3 {
        app.key("down");
    }
    app.wait_screen("cursor on the Add role row", |s| {
        row_with(s, "+ Add role override…").contains('▎')
    });
    assert!(footer(&app.screen()).contains("Add role override"));
    app.key("enter");
    let picker = app.wait_text("no overrides yet");
    assert!(!footer(&picker).contains(&Fixed::openshell().role_env.text));
    app.key("esc");
    app.wait_absent("no overrides yet");
    leave_without_a_prompt(&app);
    app.finish();
}

fn row_has(screen: &TerminalSnapshot, needle: &str, part: &str) -> bool {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .any(|row| row.contains(needle) && row.contains(part))
}

fn open_fixed_accounts(app: &App, downs: usize, crumb: &str) -> TerminalSnapshot {
    app.wait_text("Current directory");
    app.key("c");
    app.wait_text("Accounts › Overview");
    for _ in 0..downs {
        app.key("down");
    }
    app.wait_text(crumb)
}

fn leave_accounts(app: &App) {
    for _ in 0..3 {
        if text(&app.settle()).contains("Current directory") {
            break;
        }
        app.key("esc");
    }
    let manager = app.wait_text("Current directory");
    assert!(!text(&manager).contains("Unsaved changes"));
}

fn tab_until(
    app: &App,
    reason: &str,
    reached: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    for _ in 0..8 {
        let screen = app.settle();
        if reached(&screen) {
            return screen;
        }
        app.key("tab");
    }
    app.wait_screen(reason, reached)
}

fn quiet_after(app: &App, reason: &str, unrelated_key: &str) {
    status_expired(app, reason);
    app.key(unrelated_key);
    let after = app.settle();
    assert!(
        !footer(&after).contains(reason),
        "{unrelated_key} brought the reason back:\n{}",
        text(&after)
    );
}

#[test]
fn jackin_fixed_account_enable_refuses_by_key_button_and_menu() {
    let fixed = Fixed::openshell();
    let reason = fixed.enable_global.text;
    let short = fixed.enable_global.short.expect("a short reason");
    let app = fixed_app("jackin-fixed-account-enable");
    let account = open_fixed_accounts(&app, 7, "Accounts › Codex › Experiments");
    assert!(row_with(&account, "Enabled").contains("yes"));
    assert!(!footer(&account).contains("t Disable"));
    assert!(!footer(&account).contains("t Enable"));

    app.key("shift+f10");
    app.wait_text("Show usage");
    for _ in 0..5 {
        app.key("down");
    }
    app.wait_screen("the highlight reaches Disable", |s| {
        text(s).contains(&format!(" {short} "))
    });
    app.visual("functional/jackin_fixed_account_menu");
    app.key("enter");
    let fired = app.settle();
    assert!(text(&fired).contains(&short), "a disabled item stays shut");
    assert!(row_with(&fired, "Enabled").contains("yes"));
    assert!(!footer(&fired).contains(&reason));
    app.key("esc");
    app.wait_absent("Show usage");

    app.key("t");
    let keyed = refused_with(&app, &reason);
    assert!(row_with(&keyed, "Enabled").contains("yes"));
    assert!(!text(&keyed).contains("account · disabled"));
    assert!(
        row_has(&keyed, "Remove…", "Disable"),
        "the button still reads Disable"
    );
    quiet_after(&app, &reason, "down");
    app.key("up");

    app.key("tab");
    app.key("tab");
    for _ in 0..4 {
        app.key("right");
    }
    let button = app.wait_screen("focus reaches the Disable button", |s| {
        row_has(s, "Disable", "▎Disable")
    });
    assert!(!footer(&button).contains("Enter Disable"));
    app.key("enter");
    let pressed = refused_with(&app, &reason);
    assert!(row_with(&pressed, "Disable").contains("▎Disable"));
    assert!(row_with(&pressed, "Enabled").contains("yes"));
    status_expired(&app, &reason);

    app.click(inside(find(&app.screen(), "Disable")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Enabled").contains("yes"));
    assert!(row_with(&clicked, "Disable").contains("▎Disable"));
    app.visual("functional/jackin_fixed_account_disable_refused");
    quiet_after(&app, &reason, "left");
    leave_accounts(&app);
    app.finish();
}

#[test]
fn jackin_fixed_account_form_refuses_a_typed_key_and_its_button() {
    let fixed = Fixed::openshell();
    let reason = fixed.typed_key.text;
    let app = fixed_app("jackin-fixed-account-form");
    open_fixed_accounts(&app, 0, "Accounts › Overview");
    app.key("a");
    app.wait_text("New account");
    for _ in 0..3 {
        app.key("tab");
    }
    let form = app.wait_screen("focus reaches the source radio", |s| {
        row_has(s, "Local agent folder", "▎")
    });
    assert!(
        row_with(&form, "1Password item / field")
            .contains("( ) 1Password item / field  (usage only)")
    );
    assert!(
        row_with(&form, "Local agent folder").contains("(●) Local agent folder  (recommended)")
    );
    assert!(row_with(&form, "Plain-text API key").contains("( ) Plain-text API key"));
    assert!(text(&form).contains(&reason));
    assert!(!text(&form).contains("Usage only · launching Codex needs a local login"));
    assert!(!text(&form).contains("paste the key"));
    let ratio = contrast(&form, find(&form, &reason));
    assert!(ratio >= 4.5, "the reason is legible, contrast {ratio:.2}");
    app.visual("functional/jackin_fixed_account_form");

    app.key("down");
    app.wait_screen("the cursor reaches the typed key option", |s| {
        row_has(s, "Plain-text API key", "▎")
    });
    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(
            row_with(&keyed, "Local agent folder").contains("(●)"),
            "{key}"
        );
        assert!(
            row_with(&keyed, "Plain-text API key").contains("( )"),
            "{key}"
        );
        assert!(!text(&keyed).contains("paste the key"), "{key}");
        status_expired(&app, &reason);
    }
    app.key("space");
    refused_with(&app, &reason);
    app.visual("functional/jackin_fixed_account_form_refused");
    status_expired(&app, &reason);

    app.click(inside(find(&app.screen(), "Plain-text API key")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Local agent folder").contains("(●)"));
    assert!(!text(&clicked).contains("paste the key"));
    quiet_after(&app, &reason, "up");

    app.key("up");
    let one_password = app.wait_screen("1Password is chosen", |s| {
        row_has(s, "1Password item / field", "(●)")
    });
    assert!(text(&one_password).contains("Usage only · launching Codex needs a local login"));
    app.visual("functional/jackin_fixed_account_form_1password");
    let plain = "Enter plain text instead";
    let button = tab_until(&app, "focus reaches the plain text button", |s| {
        row_has(s, plain, &format!("▎{plain}"))
    });
    assert!(!(0..button.rows()).any(|r| row_text(&button, r).contains("Enter Enter plain")));
    assert!(!footer(&button).contains(&format!("Enter {plain}")));
    app.key("enter");
    let pressed = refused_with(&app, &reason);
    assert!(row_with(&pressed, "1Password item / field").contains("(●)"));
    assert!(!text(&pressed).contains("paste the key"));
    status_expired(&app, &reason);
    app.click(inside(find(&app.screen(), plain)));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "1Password item / field").contains("(●)"));
    assert!(!text(&clicked).contains("paste the key"));
    status_expired(&app, &reason);

    app.key("esc");
    app.wait_absent("New account");
    leave_accounts(&app);
    app.finish();
}

#[test]
fn jackin_fixed_account_form_offers_an_environment_variable() {
    let words = Vocabulary::openshell().source;
    let app = fixed_app("jackin-fixed-account-env");
    open_fixed_accounts(&app, 0, "Accounts › Overview");
    app.key("a");
    app.wait_text("New account");
    for _ in 0..3 {
        app.key("tab");
    }
    app.wait_screen("focus reaches the source radio", |s| {
        row_has(s, "Local agent folder", "▎")
    });
    assert!(!text(&app.screen()).contains(&words.env_placeholder));
    for _ in 0..2 {
        app.key("down");
    }
    let chosen = app.wait_screen("the environment variable is chosen", |s| {
        text(s).contains(&format!("(●) {}", words.env_label))
    });
    let shown = text(&chosen);
    assert!(shown.contains(&words.env_placeholder), "{shown}");
    assert!(!shown.contains("paste the key"), "{shown}");
    assert!(row_with(&chosen, "Plain-text API key").contains("( )"));

    app.key("tab");
    app.key("enter");
    app.type_text("ANTHROPIC_API_KEY");
    let typed = app.wait_text("ANTHROPIC_API_KEY");
    let shown = text(&typed);
    assert!(shown.contains(&words.env_help), "{shown}");
    assert!(!shown.contains(&words.env_placeholder), "{shown}");
    app.visual("functional/jackin_fixed_account_form_env");

    app.key("esc");
    app.key("esc");
    app.wait_absent("New account");
    leave_accounts(&app);
    app.finish();
}

#[test]
fn jackin_fixed_account_form_says_what_jackin_does_with_a_reference() {
    let app = fixed_app("jackin-fixed-account-pending");
    open_fixed_accounts(&app, 0, "Accounts › Overview");
    app.key("a");
    app.wait_text("New account");
    for _ in 0..3 {
        app.key("tab");
    }
    app.wait_screen("focus reaches the source radio", |s| {
        row_has(s, "Local agent folder", "▎")
    });
    app.key("up");
    app.wait_screen("1Password is chosen", |s| {
        row_has(s, "1Password item / field", "(●)")
    });
    app.click_text("Choose…");
    app.wait_text("loading accounts…");
    app.advance_clock(300);
    app.wait_text("chainargos.1password.com");
    app.key("enter");
    app.wait_text("loading vaults…");
    app.advance_clock(300);
    app.wait_text("Engineering");
    app.key("enter");
    app.wait_text("loading items from Engineering…");
    app.advance_clock(300);
    app.wait_text("Anthropic · Work");
    app.type_text("Work");
    app.wait_screen("the item list is narrowed", |s| {
        !text(s).contains("OpenAI · Codex Primary")
    });
    app.key("enter");
    app.wait_text("loading Anthropic · Work…");
    app.advance_clock(300);
    app.wait_text("Choose field");
    app.key("enter");
    let chosen = app.wait_text("Reference chosen");
    let t = text(&chosen);
    assert!(
        t.contains("only the reference is saved · read here for usage only"),
        "{t}"
    );
    assert!(!t.contains("resolved at launch"), "{t}");

    app.click(button(&app, "Validate"));
    let pending = app.wait_text("… material   checking");
    let t = text(&pending);
    assert!(t.contains("only the reference is saved"), "{t}");
    assert!(t.contains("Validating the source…"), "{t}");
    assert!(t.contains("New account"), "the form stays open:\n{t}");
    app.visual("functional/jackin_fixed_account_form_pending");

    app.key("esc");
    app.wait_absent("New account");
    leave_accounts(&app);
    app.finish();
}

#[test]
fn jackin_fixed_account_edit_refuses_a_host_variable_account() {
    let reason = "reads ZAI_API_KEY · remove it, then add an account";
    let app = fixed_app("jackin-fixed-account-edit");
    let account = open_fixed_accounts(&app, 13, "Accounts › Z.AI › host login");
    assert!(text(&account).contains("Host env · $ZAI_API_KEY"));
    assert!(!footer(&account).contains("e Edit…"));

    app.key("shift+f10");
    app.wait_text("Show usage");
    for _ in 0..3 {
        app.key("down");
    }
    app.wait_screen("the highlight reaches Edit…", |s| {
        text(s).contains(" env account ")
    });
    app.key("enter");
    let fired = app.settle();
    assert!(
        !text(&fired).contains("Edit account"),
        "a disabled item stays shut"
    );
    assert!(!footer(&fired).contains(reason));
    app.key("esc");
    app.wait_absent("Show usage");

    app.key("e");
    let keyed = refused_with(&app, reason);
    assert!(!text(&keyed).contains("Edit account"));
    app.visual("functional/jackin_fixed_account_edit_env");
    quiet_after(&app, reason, "up");
    app.key("down");

    app.key("tab");
    app.key("tab");
    app.key("right");
    app.key("right");
    let button = app.wait_screen("focus reaches the Edit… button", |s| {
        row_has(s, "Edit…", "▎Edit…")
    });
    assert!(!footer(&button).contains("Enter Edit…"));
    app.key("enter");
    let pressed = refused_with(&app, reason);
    assert!(!text(&pressed).contains("Edit account"));
    status_expired(&app, reason);
    app.click(inside(find(&app.screen(), "Edit…")));
    let clicked = refused_with(&app, reason);
    assert!(!text(&clicked).contains("Edit account"));
    status_expired(&app, reason);

    app.key("esc");
    app.key("d");
    let remove = app.wait_text("Remove");
    assert!(text(&remove).contains("host login"));
    app.key("esc");
    leave_accounts(&app);
    app.finish();
}

fn open_fixed_wizard(app: &App) -> TerminalSnapshot {
    app.wait_text("Current directory");
    app.key("n");
    app.wait_text("New workspace · step 1 of 5 · Source")
}

fn cancel_wizard(app: &App) {
    if text(&app.settle()).contains("step 2 of 5") {
        app.key("esc");
        app.wait_text("step 1 of 5");
    }
    app.click(button(app, "Cancel"));
    let manager = app.wait_text("Current directory");
    assert!(footer(&manager).contains("Cancelled · nothing created"));
}

#[test]
fn jackin_fixed_browser_git_url_keeps_its_help_and_refuses_a_url() {
    let fixed = Fixed::openshell();
    let words = jackin_console::domain::vocabulary::Vocabulary::openshell();
    let app = fixed_app("jackin-fixed-browser-url");
    open_fixed_wizard(&app);
    app.key("g");
    let entry = app.wait_text(&words.git_url_help[0]);
    assert!(text(&entry).contains(&words.git_url_help[1]));
    assert!(text(&entry).contains(&fixed.wizard_ro.text));
    assert!(row_with(&entry, "Read-only").contains("[ ] Read-only"));
    assert!(
        !text(&entry).contains(&format!("! {}", fixed.git_url)),
        "no error before an attempt"
    );
    app.visual("functional/jackin_fixed_browser_url");

    app.paste("github.com/acme/payments");
    app.key("enter");
    let refused = app.wait_text(&format!("! {}", fixed.git_url));
    assert!(
        text(&refused).contains("github.com/acme/payments"),
        "the typed value is kept"
    );
    assert!(
        text(&refused).contains("step 1 of 5 · Source"),
        "the wizard does not advance"
    );
    assert!(!text(&refused).contains("repository not found"));
    assert!(
        !footer(&refused).contains(&fixed.git_url),
        "the error sits under the field, not in the status"
    );
    app.visual("functional/jackin_fixed_browser_url_refused");

    app.key("enter");
    let again = app.settle();
    assert!(text(&again).contains(&format!("! {}", fixed.git_url)));
    assert!(text(&again).contains("step 1 of 5 · Source"));
    cancel_wizard(&app);
    app.finish();
}

#[test]
fn jackin_fixed_browser_read_only_refuses_by_key_and_click() {
    let reason = Fixed::openshell().wizard_ro.text;
    let app = fixed_app("jackin-fixed-browser-read-only");
    let browser = open_fixed_wizard(&app);
    let row = row_with(&browser, "Read-only");
    assert!(
        row.contains("[ ] Read-only") && row.contains(&reason),
        "{row}"
    );
    let ratio = contrast(&browser, find(&browser, &reason));
    assert!(ratio >= 4.5, "the reason is legible, contrast {ratio:.2}");

    app.key("tab");
    let focused = app.wait_screen("focus reaches the Read-only box", |s| {
        row_has(s, "Read-only", "▎")
    });
    assert!(!footer(&focused).contains("Toggle"));
    for key in ["space", "enter"] {
        app.key(key);
        let keyed = refused_with(&app, &reason);
        assert!(
            row_with(&keyed, "Read-only").contains("[ ] Read-only"),
            "{key}"
        );
        assert!(row_with(&keyed, "Read-only").contains('▎'), "{key}");
        if key == "space" {
            app.visual("functional/jackin_fixed_browser_read_only");
        }
        status_expired(&app, &reason);
    }
    app.click(inside(find(&app.screen(), "Read-only")));
    let clicked = refused_with(&app, &reason);
    assert!(row_with(&clicked, "Read-only").contains("[ ] Read-only"));
    status_expired(&app, &reason);

    app.click(button(&app, "Choose"));
    let next = app.wait_text("step 2 of 5 · Destination");
    assert!(text(&next).contains("Source  ~/src/payments-platform"));
    assert!(
        !text(&next).contains("read-only"),
        "the source is not read-only"
    );
    cancel_wizard(&app);
    app.finish();
}

#[test]
fn jackin_fixed_wizard_destination_cannot_change_and_next_advances() {
    let fixed = Fixed::openshell();
    let app = fixed_app("jackin-fixed-wizard-destination");
    open_fixed_wizard(&app);
    app.key("space");
    let step = app.wait_text("step 2 of 5 · Destination");
    assert!(row_with(&step, "Project folder").contains("(●) Project folder /sandbox/project"));
    assert!(row_with(&step, "Edit destination…").contains("( ) Edit destination…"));
    assert!(text(&step).contains(&fixed.destination.text));
    let ratio = contrast(&step, find(&step, &fixed.destination.text));
    assert!(ratio >= 4.5, "the reason is legible, contrast {ratio:.2}");
    assert!(footer(&step).contains("Enter Next"), "{}", footer(&step));
    app.visual("functional/jackin_fixed_wizard_destination");

    app.key("down");
    let still = app.settle();
    assert!(row_with(&still, "Project folder").contains("(●)"));
    assert!(row_with(&still, "Edit destination…").contains("( )"));
    assert!(
        !footer(&still).contains(&fixed.destination.text),
        "no status is posted"
    );
    app.click(inside(find(&still, "Edit destination…")));
    let clicked = app.settle();
    assert!(row_with(&clicked, "Project folder").contains("(●)"));
    assert!(text(&clicked).contains("step 2 of 5 · Destination"));

    app.key("enter");
    let workdir = app.wait_text("step 4 of 5 · Working dir");
    assert!(text(&workdir).contains("▪ /sandbox/project"));
    assert!(!text(&workdir).contains("Edit destination"));
    assert!(
        !footer(&workdir).contains(&fixed.destination.text),
        "no status appears"
    );
    app.visual("functional/jackin_fixed_wizard_workdir");

    app.key("enter");
    let name = app.wait_text("step 5 of 5 · Name");
    assert!(text(&name).contains("▎ payments-platform"));
    assert!(!text(&name).contains("▎ project"));
    for _ in 0..6 {
        if text(&app.settle()).contains("step 1 of 5") {
            break;
        }
        app.key("esc");
    }
    cancel_wizard(&app);
    app.finish();
}

fn attach_capsule(app: &App) {
    app.wait_text("Current directory");
    app.key("down");
    app.key("right");
    app.key("down");
    app.key("enter");
    app.wait_text("Ctrl+B Pane commands");
}

fn detach_capsule(app: &App) {
    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    app.wait_text("Detached · payments-platform keeps running");
}

fn palette_row(screen: &TerminalSnapshot, label: &str) -> String {
    row_with(screen, label)
}

#[test]
fn jackin_fixed_palette_shows_reveal_and_open_as_unavailable() {
    let fixed = Fixed::openshell();
    let short = fixed.reveal_open.short.expect("a short reason");
    let app = fixed_app("jackin-fixed-palette-export");
    attach_capsule(&app);
    app.key("ctrl+\\");
    let palette = app.wait_text("Command palette");
    for label in ["Export file and reveal", "Export file and open"] {
        let row = palette_row(&palette, label);
        assert!(row.contains(&short), "{row}");
    }
    assert!(palette_row(&palette, "Export file  ").contains("host"));
    assert!(!palette_row(&palette, "Export file  ").contains(&short));
    app.visual("functional/jackin_fixed_palette_export");

    app.type_text("reveal");
    let found = app.wait_screen("the filter keeps the reveal rows", |s| {
        text(s).contains("3 of 20") || text(s).contains(&short)
    });
    assert!(text(&found).contains(&short));
    app.key("enter");
    let fired = app.settle();
    assert!(
        !text(&fired).contains("Copies to ~/Downloads/jackin/"),
        "a disabled row stays shut"
    );
    app.key("esc");
    app.key("esc");

    app.key("ctrl+\\");
    app.wait_text("Command palette");
    app.key("down");
    app.key("down");
    app.key("down");
    app.key("enter");
    let export = app.wait_text("Copies to ~/Downloads/jackin/");
    assert!(text(&export).contains("Export file"));
    app.key("esc");
    detach_capsule(&app);
    app.finish();
}

#[test]
fn jackin_fixed_file_menu_shows_reveal_as_unavailable() {
    let fixed = Fixed::openshell();
    let short = fixed.reveal_open.short.expect("a short reason");
    let app = fixed_app("jackin-fixed-file-menu-export");
    attach_capsule(&app);
    app.key("f10");
    app.wait_text("Export selected file and reveal…");
    for _ in 0..4 {
        app.key("down");
    }
    let menu = app.wait_screen("the highlight reaches the reveal item", |s| {
        text(s).contains(&format!(" {short} "))
    });
    assert!(row_with(&menu, "Export selected file and reveal…").contains("Export"));
    app.visual("functional/jackin_fixed_file_menu_export");
    app.key("enter");
    let fired = app.settle();
    assert!(text(&fired).contains(&short), "a disabled item stays shut");
    assert!(!text(&fired).contains("Select a path in a pane first"));
    app.key("up");
    let live = app.wait_screen("the plain export item shows no reason", |s| {
        !text(s).contains(&format!(" {short} "))
    });
    assert!(text(&live).contains("Export selected file…"));
    app.key("esc");
    detach_capsule(&app);
    app.finish();
}

#[test]
fn jackin_fixed_launch_picker_disables_explicitly_ignored_agents() {
    let app = fixed_app("jackin-fixed-launch-picker");
    app.wait_text("Current directory");
    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("payments-platform › the-architect"));
    let rows: Vec<String> = (0..picker.rows()).map(|r| row_text(&picker, r)).collect();
    for agent in ["Claude Code", "Amp", "Kimi Code", "OpenCode", "Grok Build"] {
        assert!(
            rows.iter().any(|row| row.contains(&format!(" {agent} "))
                && row.contains("agent runtime mode is ignore")
                && row.contains("unavailable")),
            "{agent} is a disabled row with its reason:\n{}",
            text(&picker)
        );
    }
    let codex = row_with(&picker, "▪ Codex");
    assert!(
        codex.contains("choose at start") && !codex.contains("unavailable"),
        "Codex stays available: {codex}"
    );
    app.visual("functional/jackin_fixed_launch_picker");

    app.click(find(&picker, "▪ Claude Code"));
    let after = app.settle();
    assert!(
        text(&after).contains("Launch · choose Agent") && !text(&after).contains("Loading"),
        "a disabled row starts nothing:\n{}",
        text(&after)
    );
    app.key("escape");
    app.wait_absent("Launch · choose Agent");
    app.finish();
}

fn launch_checks(name: &str) -> App {
    scenario(name, "launch-checks", 120, 40)
}

fn launch_first_agent(app: &App, downs: usize, scope: &str) {
    app.wait_text("Current directory");
    for _ in 0..downs {
        app.key("down");
    }
    app.key("enter");
    app.wait_text(scope);
    app.key("space");
    app.key("enter");
    app.wait_text("Cancel launch…");
}

#[test]
fn jackin_launch_refuses_a_role_the_workspace_does_not_allow() {
    let app = launch_checks("jackin-role-not-allowed");
    launch_first_agent(&app, 4, "customer-portal › writer");
    app.advance_clock(1_000);
    let refused = app.wait_text("Role not allowed");
    for line in [
        "Workspace customer-portal does not allow role writer.",
        "nothing was created",
        "Allow it in the workspace's Roles tab, or load another role.",
    ] {
        assert!(
            text(&refused).contains(line),
            "{line:?}:\n{}",
            text(&refused)
        );
    }
    app.settle();
    app.visual("functional/jackin_role_not_allowed");

    app.key("escape");
    let manager = app.wait_text("Current directory");
    assert!(
        !text(&manager).contains("customer-portal-"),
        "no instance of the refused launch exists"
    );
    app.finish();
}

#[test]
fn jackin_launch_asks_for_the_values_a_role_needs() {
    let app = launch_checks("jackin-launch-env-prompt");
    launch_first_agent(&app, 2, "infra-control-plane › sentinel");
    app.advance_clock(1_000);
    let asked = app.wait_text("Fail the preflight hook on purpose");
    for line in [
        "Note for the run",
        "SENTINEL_FAIL_PREFLIGHT",
        "SENTINEL_NOTE",
    ] {
        assert!(text(&asked).contains(line), "{line:?}:\n{}", text(&asked));
    }
    assert!(
        text(&asked).contains("Codex · OpenAI") && text(&asked).contains("0/7 stages"),
        "the questions sit over the Codex launch:\n{}",
        text(&asked)
    );
    assert!(row_with(&asked, "Fail the preflight hook on purpose").contains("optional"));
    let note = row_with(&asked, "Note for the run");
    assert!(
        note.contains('*') && !note.contains("optional"),
        "an entry with no default is required: {note}"
    );
    app.settle();
    app.visual("functional/jackin_launch_env_prompt");

    app.advance_clock(30_000);
    let held = app.settle();
    assert!(
        text(&held).contains("Fail the preflight hook on purpose"),
        "the launch waits for the answers"
    );
    app.click_text("Start launch");
    let refused = app.wait_text("Enter a value for SENTINEL_NOTE.");
    assert!(
        text(&refused).contains("Values for this launch"),
        "the form stays open:\n{}",
        text(&refused)
    );
    app.click_text("Note for the run");
    app.type_text("hello");
    app.key("enter");
    app.click_text("Start launch");
    app.advance_clock(30_000);
    app.wait_screen("the launch hands off to the capsule", |s| {
        text(s).contains("Ctrl+B Pane commands")
    });
    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    app.wait_text("Detached");
    app.finish();
}

#[test]
fn jackin_launch_questions_end_the_launch_on_escape() {
    let app = launch_checks("jackin-launch-env-escape");
    launch_first_agent(&app, 2, "infra-control-plane › sentinel");
    app.advance_clock(1_000);
    app.wait_text("Fail the preflight hook on purpose");
    app.key("escape");
    let manager = app.wait_text("Current directory");
    assert!(!text(&manager).contains("Fail the preflight hook on purpose"));
    assert!(text(&manager).contains("2 running"), "nothing was started");
    app.finish();
}

#[test]
fn jackin_manager_multi_launch_codex_and_omp_in_check_order() {
    let app = jackin("jackin-multi-launch", "paused");
    app.wait_text("Current directory");
    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("Space Toggle"));
    assert!(text(&picker).contains("Enter Confirm"));
    app.click_text("▪ Codex");
    app.key("end");
    app.key("space");
    app.key("enter");
    app.wait_text("Cancel launch…");
    app.visual("functional/jackin_manager_multi_launch");
    app.key("ctrl+c");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_manager_multi_launch_reverse_order_and_cancel_second_picker() {
    let app = jackin("jackin-multi-launch-reverse", "paused");
    app.wait_text("Current directory");
    app.key("enter");
    app.wait_text("Launch · choose Agent");
    app.key("end");
    app.key("space");
    app.key("home");
    app.click_text("▪ Codex");
    app.key("enter");
    let cockpit = app.wait_text("Cancel launch…");
    assert!(text(&cockpit).contains("OMP") || text(&cockpit).contains("Omp"));
    app.key("ctrl+c");
    app.wait_text("Current directory");
    app.finish();
}
