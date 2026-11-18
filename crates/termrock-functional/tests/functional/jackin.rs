use super::support::{App, JACKIN, Point, crop, find, find_all, row_text, text};

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

fn normalized_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    let app = App::new(
        "jackin-launch",
        JACKIN,
        &[
            "--scenario",
            "returning",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    );
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("2 running"));
    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("payments-platform › the-architect"));

    app.visual("functional/jackin_launch_reconnect");
    app.key("enter");
    app.wait_text("Loading the-architect into workspace payments-platform");
    app.visual("functional/jackin_launch_cockpit");
    app.advance_clock(30_000);
    let capsule = app.wait_screen("launch hands off to capsule", |s| {
        let t = text(s);
        t.contains("Claude Code v2.1.14")
            && t.contains("Allow edit to src/settlement/config.rs? (y/n)")
            && t.contains("Ctrl+B Pane commands")
    });
    assert!(!text(&capsule).contains("junie-tui:advance-clock:"));
    assert!(!text(&capsule).contains("Launch failed"));
    app.visual("functional/jackin_launch_capsule");

    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    let detached = app.wait_text("Detached · payments-platform keeps running");
    assert!(text(&detached).contains("3 running"));
    app.visual("functional/jackin_launch_detached");

    app.key("enter");
    let restored = app.wait_screen("reconnect restores live capsule", |s| {
        let t = text(s);
        t.contains("Claude Code v2.1.14") && t.contains("Ctrl+B Pane commands")
    });
    assert!(text(&restored).contains("Allow edit to src/settlement/config.rs? (y/n)"));
    app.visual("functional/jackin_launch_reconnected");
    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    app.wait_text("Detached · payments-platform keeps running");
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
    app.key("enter");
    app.wait_text("Current directory");
    let empty = app.settle();
    assert!(
        text(&empty).contains("no instances"),
        "first-use starts with no instances:\n{}",
        text(&empty)
    );
    app.visual("functional/jackin_first_use_empty");
    app.finish();
}

fn stop_action(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(&screen, u32::from(point.y));
            row.contains("Keep running") && row.contains('│')
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one stop-dialog action {label:?}:\n{}",
        text(&screen)
    );
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

#[test]
fn jackin_manager_stop_cancel_then_confirm() {
    let app = App::new(
        "jackin-manager-stop",
        JACKIN,
        &[
            "--scenario",
            "returning",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    );
    let manager = app.wait_text("2 running");
    assert!(text(&manager).contains("Current directory"));
    app.key("down");
    app.wait_text("payments-platform");
    app.key("right");
    app.key("down");
    app.wait_text("7f3a");
    app.key("t");
    let dialog = app.wait_text("Stop instance 7f3a?");
    assert!(text(&dialog).contains("Keep running"));
    assert!(text(&dialog).contains("preserved with uncommitted work"));
    app.visual("functional/jackin_stop_instance_dialog");
    app.click(stop_action(&app, "Keep running"));
    let kept = app.wait_screen("cancel leaves the instance running", |s| {
        !text(s).contains("Stop instance 7f3a?") && text(s).contains("2 running")
    });
    assert!(text(&kept).contains("7f3a"));
    assert!(text(&kept).contains("running"));
    app.key("t");
    app.wait_text("Stop instance 7f3a?");
    app.click(stop_action(&app, "Stop"));
    app.advance_clock(2_000);
    let stopped = app.wait_text("Instance 7f3a stopped");
    assert!(text(&stopped).contains("1 running"));
    assert!(!text(&stopped).contains("2 running"));
    assert!(text(&stopped).contains("preserved"));
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
    app.advance_clock(24_000);
    let failed = app.wait_screen("launch failure details", |screen| {
        let visible = text(screen);
        visible.contains("The Construct network could not be attached")
            && visible.contains("Network")
            && normalized_text(&visible).contains("network jackin-net not found")
    });
    assert!(text(&failed).contains("The Construct network could not be attached"));
    assert!(text(&failed).contains("Network"));
    assert!(normalized_text(&text(&failed)).contains("network jackin-net not found"));
    app.visual("functional/jackin_launch_failed");
    app.key("escape");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_cockpit_ctrl_c_returns_to_manager() {
    let app = App::new(
        "jackin-launch-abort",
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
    app.key("ctrl+c");
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("Launch failed ·"));
    assert!(text(&manager).contains("still running in the Construct"));
    assert!(!text(&manager).contains("Loading the-architect"));
    app.finish();
}

#[test]
fn jackin_settings_unsaved_leave_cancel_then_discard() {
    let app = App::new(
        "jackin-settings-leave",
        JACKIN,
        &[
            "--scenario",
            "returning",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Current directory");
    app.key("s");
    let settings = app.wait_text("Add Co-authored-by trailer");
    assert!(
        row_text(
            &settings,
            u32::from(find(&settings, "Add Co-authored-by trailer").y)
        )
        .contains("[✓]")
    );
    app.key("enter");
    app.key(" ");
    let dirty = app.wait_text("• 1 change");
    assert!(
        row_text(
            &dirty,
            u32::from(find(&dirty, "Add Co-authored-by trailer").y)
        )
        .contains("[ ]")
    );
    app.key("escape");
    app.key("escape");
    let leave = app.wait_text("Unsaved changes");
    assert!(text(&leave).contains("Save changes before leaving?"));
    assert!(text(&leave).contains("1 change would be"));
    assert!(text(&leave).contains("lost."));
    assert!(text(&leave).contains("Discard"));
    app.visual("functional/jackin_settings_dirty_leave");
    app.key("enter");
    let kept = app.wait_screen("cancel stays on the dirty settings", |s| {
        !text(s).contains("Unsaved changes") && text(s).contains("• 1 change")
    });
    assert!(text(&kept).contains("Add Co-authored-by trailer"));
    app.key("escape");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.key("enter");
    let discarded = app.wait_text("Discarded 1 change");
    assert!(text(&discarded).contains("Current directory"));
    app.key("s");
    let reopened = app.wait_text("Add Co-authored-by trailer");
    assert!(!text(&reopened).contains("• 1 change"));
    assert!(
        row_text(
            &reopened,
            u32::from(find(&reopened, "Add Co-authored-by trailer").y)
        )
        .contains("[✓]")
    );
    app.finish();
}
