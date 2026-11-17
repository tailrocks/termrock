#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::domain::instance::{DaemonSnapshot, InstanceId, InstanceStatus};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::MountSource;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::screens::Go;
use jackin_console::sim;
use jackin_console::world::{DaemonHealth, Msg};
use termrock::motion::Motion;
use termrock::theme::{ColorLevel, Theme, Tone};

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn id(label: &str) -> InstanceId {
    InstanceId::simulated(label)
}

fn live_world(scenario: Scenario) -> jackin_console::world::World {
    let mut world = world_for(scenario);
    world.surfaces.vocabulary = Vocabulary::openshell();
    world
}

fn worlds(scenario: Scenario) -> [jackin_console::world::World; 2] {
    [world_for(scenario), live_world(scenario)]
}

fn pair(scenario: Scenario, keys: &str) -> (String, String) {
    let mut demo = app(scenario);
    press(&mut demo, keys);
    let mut world = world_for(scenario);
    world.surfaces.vocabulary = Vocabulary::openshell();
    let mut live = app_with(scenario, world);
    press(&mut live, keys);
    (screen(&mut demo), screen(&mut live))
}

#[test]
fn the_openshell_words_avoid_the_false_strings() {
    for (name, text) in Vocabulary::openshell().fields() {
        for banned in [
            "on the host",
            "Opened",
            "Revealed",
            "Capsule",
            "Construct",
            "daemon",
            "container",
            "Docker",
            "pushed",
            "discard",
            "{state}",
        ] {
            if banned == "Capsule" && (name == "capsule" || name.starts_with("daemon.")) {
                continue;
            }
            assert!(!text.contains(banned), "{name} says {banned}: {text}");
        }
    }
}

#[test]
fn the_openshell_vocabulary_renames_the_demo_words() {
    const SAME: [&str; 3] = ["status.superseded", "capsule", "not_in_roster"];
    const PANE_RENAMED: [&str; 3] = ["pane.viewing", "pane.connection_lost", "pane.exited"];
    let demo = Vocabulary::demo();
    let openshell = Vocabulary::openshell();
    let (demo, live) = (demo.fields(), openshell.fields());
    assert_eq!(
        demo.iter().map(|f| f.0).collect::<Vec<_>>(),
        live.iter().map(|f| f.0).collect::<Vec<_>>(),
    );
    for ((name, was), (_, now)) in demo.iter().zip(&live) {
        if SAME.contains(name)
            || name.starts_with("reason.")
            || name.starts_with("source.")
            || (name.starts_with("pane.") && !PANE_RENAMED.contains(name))
        {
            assert_eq!(was, now, "{name}");
        } else {
            assert_ne!(was, now, "{name}");
        }
    }
}

#[test]
fn the_manager_card_names_the_project() {
    let (demo, live) = pair(Scenario::Returning, "down");
    assert!(demo.contains("Mounts") && !demo.contains("Project "));
    assert!(demo.contains("~/src/payments-platform · rw "));
    assert!(live.contains("Project ") && !live.contains("Mounts"));
    assert!(live.contains("~/src/payments-platform · uploaded"));
}

#[test]
fn the_status_strip_names_the_world() {
    let (demo, live) = pair(Scenario::Returning, "");
    assert!(demo.contains("inside the Construct"));
    assert!(live.contains("inside the sandbox") && !live.contains("Construct"));
}

#[test]
fn the_first_use_hint_follows_the_words() {
    let (demo, live) = pair(Scenario::FirstUse, "");
    assert!(demo.contains("is mounted at its own path inside the Construct"));
    assert!(live.contains("is uploaded to /sandbox/project"));
    assert!(!live.contains("mounted"));
}

#[test]
fn the_editor_tab_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down e");
    assert!(demo.contains("General    Mounts    Roles"));
    assert!(live.contains("General    Project    Roles") && !live.contains("Mounts"));
}

#[test]
fn the_github_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down o");
    assert!(demo.contains("Opened https://github.com/chainargos/payments-platform on the host"));
    assert!(live.contains("GitHub · https://github.com/chainargos/payments-platform"));
    assert!(!live.contains("Opened"));
}

#[test]
fn the_instance_status_text_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down right down");
    assert!(demo.contains("running · Container and Capsule daemon are live."));
    assert!(live.contains("running · The sandbox is running."));
    assert!(!live.contains("Capsule"));
}

#[test]
fn the_trailer_sample_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s");
    assert!(demo.contains("Signed-off-by: Alexey Zhokhov <alexey@chainargos.com>"));
    assert!(live.contains("None · jackin adds no trailers to commits"));
    assert!(!live.contains("Alexey"));
}

#[test]
fn the_trust_open_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s 5 tab o");
    assert!(demo.contains("Opened https://github.com/chainargos/roles in the browser"));
    assert!(live.contains("Source · github.com/chainargos/roles"));
    assert!(!live.contains("Opened"));
}

#[test]
fn the_env_form_wording_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s 3 tab a");
    assert!(
        demo.contains("Plain text  (masked)") && demo.contains("typed once · masked afterwards")
    );
    assert!(live.contains("(●) Plain text ") && live.contains("value is stored as plain text"));
    assert!(demo.contains("typed once · masked afterwards"));
    assert!(live.contains("type the value"));
    assert!(!live.contains("masked"));
}

#[test]
fn the_wizard_source_step_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "n");
    assert!(demo.contains("[ ] Mount read-only"));
    assert!(live.contains("[ ] Read-only") && !live.contains("Mount read-only"));
}

#[test]
fn the_git_url_help_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "n g");
    assert!(demo.contains("The repository is cloned into the"));
    assert!(live.contains("jackin uploads a folder from this machine."));
    assert!(live.contains("Clone the repository first, then choose its folder."));
}

#[test]
fn the_session_choice_reason_follows_the_words() {
    let provider = jackin_console::domain::agent::Agent::Codex.provider();
    let reasons = worlds(Scenario::Returning).map(|world| {
        let session = world
            .accounts
            .by_provider(provider)
            .next()
            .map(|a| a.id.clone())
            .expect("a Codex account");
        world
            .account_for(
                jackin_console::domain::tuple::TupleIdentity::new(
                    jackin_console::domain::agent::Agent::Codex,
                    provider,
                ),
                None,
                None,
                Some(&session),
            )
            .why
    });
    assert_eq!(
        reasons[0],
        "Session choice · picked for this session in Capsule · overrides provider default Primary"
    );
    assert_eq!(
        reasons[1],
        "Session choice · picked for this session only · overrides provider default Primary"
    );
}

#[test]
fn the_mount_open_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down down down down e 2 down o");
    assert!(demo.contains("Opened ~/src/customer-portal on GitHub"));
    assert!(live.contains("no GitHub URL for this mount"));
    assert!(!live.contains("GitHub · ~/src/customer-portal") && !live.contains("Opened"));
}

#[test]
fn the_editor_workdir_hint_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down e");
    assert!(demo.contains("Inside the Construct"));
    assert!(live.contains("Inside the sandbox") && !live.contains("Construct"));
}

fn go_pair(scenario: Scenario, go: impl Fn() -> Go) -> (String, String) {
    let [demo, live] = worlds(scenario).map(|world| {
        let mut app = app_with(scenario, world);
        screen(&mut app);
        app.go(go());
        screen(&mut app)
    });
    (demo, live)
}

#[test]
fn the_detach_status_follows_the_words() {
    let (demo, live) = go_pair(Scenario::CapsuleMulti, || Go::Detach);
    assert!(demo.contains("keeps running · 2 instances in the Construct"));
    assert!(live.contains("keeps running · 2 instances on the gateway"));
    assert!(!live.contains("Construct"));
}

#[test]
fn the_launch_failure_status_follows_the_words() {
    let (demo, live) = go_pair(Scenario::Returning, || Go::LaunchFailedAck {
        instance: None,
    });
    assert!(demo.contains("Launch failed · 2 instances still running in the Construct"));
    assert!(live.contains("Launch failed · 2 instances still running on the gateway"));
    assert!(!live.contains("Construct"));
}

#[test]
fn the_still_inside_status_follows_the_words() {
    let (demo, live) = go_pair(Scenario::Returning, || Go::InstanceEnded {
        instance: id("jk-7f3a"),
        purge: false,
    });
    assert!(demo.contains("Still inside the Construct · 1 instance remains"));
    assert!(live.contains("Still running on the gateway · 1 instance remains"));
    assert!(!live.contains("Construct"));
}

#[test]
fn the_outro_caption_follows_the_words() {
    let [demo, live] = worlds(Scenario::OutroLast).map(|world| {
        let mut app = app_with_world(
            Scenario::OutroLast,
            world,
            Motion::Reduced,
            1,
            Theme::for_level(ColorLevel::TrueColor),
        );
        screen(&mut app)
    });
    assert!(demo.contains("You were in the Construct for 2 hours 14 minutes"));
    assert!(live.contains("You were in the sandbox for 2 hours 14 minutes"));
}

#[test]
fn the_about_dialog_follows_the_data() {
    let rows = vec![
        ("Product".to_owned(), "jackin 1.4.0".to_owned()),
        ("Gateway".to_owned(), "local-gateway".to_owned()),
        (
            "Mode".to_owned(),
            "live · changes real sandboxes".to_owned(),
        ),
    ];
    let open_host = ["f10", "right", "right", "down", "enter"].join(" ");
    let (host_demo, _) = pair(Scenario::Returning, &open_host);
    let brand = |world| {
        let mut app = app_with(Scenario::CapsuleMulti, world);
        screen(&mut app);
        click(&mut app, 3, 0);
        press(&mut app, "enter");
        app
    };
    let capsule_demo = screen(&mut brand(world_for(Scenario::CapsuleMulti)));
    let mut world = live_world(Scenario::Returning);
    world.surfaces.about = rows.clone();
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, &open_host);
    let host_live = screen(&mut app);
    let mut world = live_world(Scenario::CapsuleMulti);
    world.surfaces.about = rows;
    let capsule_live = screen(&mut brand(world));
    for demo in [&host_demo, &capsule_demo] {
        assert!(demo.contains("simulated · nothing here touches a real"));
        assert!(demo.contains("Design system"));
    }
    for live in [&host_live, &capsule_live] {
        let live = &squash(live);
        assert!(live.contains("Product jackin 1.4.0"));
        assert!(live.contains("Gateway local-gateway"));
        assert!(live.contains("Mode live · changes real sandboxes"));
        assert!(!live.contains("simulated") && !live.contains("Scenario"));
    }
}

#[test]
fn the_debug_info_names_the_sandbox_and_the_version() {
    let mut live = live_world(Scenario::CapsuleMulti);
    live.surfaces.version = "1.4.0".into();
    let mut demo = app(Scenario::CapsuleMulti);
    let mut live = app_with(Scenario::CapsuleMulti, live);
    for app in [&mut demo, &mut live] {
        press(app, "ctrl+b i");
    }
    let (demo, live) = (screen(&mut demo), screen(&mut live));
    assert!(demo.contains("Container ID") && squash(&demo).contains("Capsule 0.9.2"));
    assert!(squash(&live).contains("Sandbox jackin-payments-platform-7f3a"));
    assert!(squash(&live).contains("jackin 1.4.0"));
    assert!(!live.contains("Container") && !live.contains("Capsule"));
}

#[test]
fn the_view_menu_names_the_sandbox() {
    let keys = "f10 right right";
    let (demo, live) = pair(Scenario::CapsuleMulti, keys);
    assert!(demo.contains("Container info"));
    assert!(live.contains("Sandbox info") && !live.contains("Container info"));
    let (_, live) = pair(
        Scenario::CapsuleMulti,
        "f10 right right down down down enter",
    );
    assert!(live.contains("Debug info"));
}

#[test]
fn the_help_rows_name_the_sandbox() {
    let (demo, live) = pair(Scenario::Returning, "?");
    assert!(demo.contains("inspect container"));
    assert!(live.contains("inspect sandbox") && !live.contains("inspect container"));
}

#[test]
fn the_inspect_dialog_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down right down i");
    assert!(demo.contains("Container 7f3a") && demo.contains("Daemon     attached"));
    assert!(live.contains("Sandbox 7f3a") && live.contains("Capsule    attached"));
    assert!(!live.contains("Container"));
    let (demo, live) = pair(Scenario::Returning, "down right down down i");
    assert!(demo.contains("Daemon     unavailable"));
    assert!(live.contains("Capsule    gateway unreachable"));
}

#[test]
fn the_instance_detail_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down right down");
    assert!(demo.contains("Container     jackin-payments-platform-7f3a"));
    assert!(demo.contains("daemon · 3 s ago"));
    assert!(live.contains("Sandbox       jackin-payments-platform-7f3a"));
    assert!(live.contains("gateway · 3 s ago"));
    let (demo, live) = pair(Scenario::Returning, "down right down down");
    assert!(demo.contains("No live daemon (instance not running)"));
    assert!(live.contains("No live Capsule (sandbox not running)"));
    assert!(demo.contains("Live topology") && demo.contains("  unavailable"));
    assert!(live.contains("  unreachable"));
}

fn summary_pair(keys: &str) -> (String, String) {
    let [demo, live] = worlds(Scenario::Returning).map(|world| {
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, keys);
        screen_at(&mut app, 80, 24)
    });
    (demo, live)
}

#[test]
fn the_summary_lines_follow_the_words() {
    let (demo, live) = summary_pair("down right down");
    assert!(demo.contains("daemon · 3 tabs · 5 panes"));
    assert!(live.contains("capsule · 3 tabs · 5 panes"));
    let (demo, live) = summary_pair("down right down down");
    assert!(demo.contains("daemon unavailable"));
    assert!(live.contains("gateway unreachable"));
}

#[test]
fn the_summary_names_a_terminal_without_tabs() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        world.instance_mut(&id("jk-7f3a")).expect("instance").daemon = DaemonSnapshot::NoTabs;
        world.daemons.remove(&id("jk-7f3a"));
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, "down right down");
        screen_at(&mut app, 80, 24)
    });
    assert!(demo.contains("daemon reports no tabs"));
    assert!(live.contains("capsule · no tabs"));
}

#[test]
fn the_roster_card_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        let id = world.running_entries()[0].id().clone();
        world.daemons.remove(&id);
        world.instance_mut(&id).expect("instance").daemon = DaemonSnapshot::Unavailable;
        let mut app = app_with(Scenario::Returning, world);
        screen_at(&mut app, 170, 40)
    });
    assert!(demo.contains("· daemon unavailable"));
    assert!(live.contains("· gateway unreachable"));
}

#[test]
fn the_health_chip_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        world.daemon_health = DaemonHealth::Stale;
        let mut app = app_with(Scenario::Returning, world);
        screen(&mut app)
    });
    assert!(demo.contains("▲ daemon stale"));
    assert!(live.contains("▲ gateway stale") && !live.contains("daemon"));
}

#[test]
fn the_quit_dialog_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|world| {
        let mut app = app_with(Scenario::Returning, world);
        screen(&mut app);
        app.go(Go::Attach {
            instance: id("jk-9b02"),
            pane: None,
        });
        screen(&mut app);
        press(&mut app, "ctrl+q");
        screen(&mut app)
    });
    assert!(demo.contains("! Quitting force-stops the container immediately."));
    assert!(demo.contains("! Work not saved outside the container will be lost."));
    assert!(live.contains("! Quitting detaches; the sandbox keeps running."));
    assert!(live.contains("! Work stays in the sandbox until you export it."));
    assert!(!live.contains("force-stops"));
}

#[test]
fn the_mount_form_read_only_label_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down e 2 down a");
    assert!(demo.contains("[ ] Mount read-only"));
    assert!(live.contains("[ ] Read-only") && !live.contains("Mount read-only"));
}

#[test]
fn the_stop_dialog_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down right down t");
    assert!(demo.contains("The container stops and every session in it"));
    assert!(live.contains("The sandbox stops and every session in it"));
    assert!(live.contains("You can restore it later"));
    assert!(!live.contains("container") && !squash(&live).contains("later ("));
}

#[test]
fn the_purge_dialog_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down right down p");
    assert!(demo.contains("purge the instance record and its container"));
    assert!(demo.contains("1 container · 2 preserved sessions"));
    assert!(live.contains("delete the sandbox"));
    assert!(live.contains("2 preserved sessions · changed project copy"));
    assert!(live.contains("uncommitted or unexported work is lost"));
    assert!(
        !live.contains("container") && !live.contains("worktree") && !live.contains("1 sandbox")
    );
    let mut world = live_world(Scenario::Returning);
    let instance = world.instance_mut(&id("jk-7f3a")).expect("instance");
    let session = instance.sessions.as_ref().expect("sessions")[0].clone();
    instance.sessions = Ok(vec![session; 10]);
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down right down p");
    assert!(squash(&screen(&mut app)).contains("10 preserved sessions · changed project copy"));
}

#[test]
fn the_purge_dialog_names_a_clean_copy() {
    let clean = |live: bool| {
        let mut world = if live {
            live_world(Scenario::Returning)
        } else {
            world_for(Scenario::Returning)
        };
        let instance = world.instance_mut(&id("jk-7f3a")).expect("instance");
        instance.status = InstanceStatus::RestoreAvailable;
        instance.uncommitted = 0;
        instance.unpushed = 0;
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, "down right down p");
        squash(&screen(&mut app))
    };
    let (demo, live) = (clean(false), clean(true));
    assert!(demo.contains("2 preserved sessions"), "{demo}");
    assert!(
        live.contains("Scope 2 preserved sessions · clean project copy"),
        "{live}"
    );
    assert!(
        live.contains("Risk the sandbox and its files are deleted"),
        "{live}"
    );
    assert!(!live.contains("changed project copy") && !live.contains("unexported work"));
}

#[test]
fn the_purge_status_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|world| {
        let mut app = app_with(Scenario::Returning, world);
        screen(&mut app);
        let mut snapshot = sim::snapshot(&app.world);
        for entry in snapshot
            .instances
            .iter_mut()
            .filter(|entry| entry.id() == &id("jk-c41e"))
        {
            if let Some(i) = entry.as_available_mut() {
                i.status = InstanceStatus::Purged;
            }
        }
        app.deliver(&Msg::Purged {
            instance: id("jk-c41e"),
            result: Ok(Box::new(snapshot)),
        });
        screen(&mut app)
    });
    assert!(demo.contains("Purged c41e · container, sidecar, volume and recovery state removed"));
    assert!(live.contains("Purged c41e · the sandbox was deleted"));
}

#[test]
fn the_github_dialog_follows_the_words() {
    let open = "f10 right right down down down down enter";
    let (demo, live) = pair(Scenario::CapsuleMulti, open);
    assert!(demo.contains("open uses a gated host action") && demo.contains("Open PR…"));
    assert!(live.contains("Enter copies the row · y copies") && live.contains("Copy URL"));
    let (demo, live) = pair(Scenario::CapsuleMulti, &format!("{open} tab enter"));
    assert!(demo.contains("Opened the pull request on the host (gated host action)"));
    assert!(live.contains("Copied the GitHub URL"));
}

#[test]
fn the_role_trust_dialog_follows_the_words() {
    let trust = |world| {
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, "down e");
        click_text(&mut app, "Roles");
        press(&mut app, "down down down down down enter");
        screen(&mut app)
    };
    let [demo, live] = worlds(Scenario::Returning).map(trust);
    assert!(demo.contains("Trust this role source?"));
    assert!(demo.contains("The role can access mounted workspace files."));
    assert!(live.contains("The role can access the uploaded project files."));
    assert!(!live.contains("mounted workspace files"));
}

#[test]
fn the_workspace_menu_names_the_github_url() {
    let (demo, live) = pair(Scenario::Returning, "down shift+f10");
    assert!(demo.contains("Open on GitHub"));
    assert!(live.contains("Show GitHub URL") && !live.contains("Open on GitHub"));
    let (demo, live) = pair(Scenario::Returning, "shift+f10");
    assert!(demo.contains("Open on GitHub"));
    assert!(live.contains("Show GitHub URL") && !live.contains("Open on GitHub"));
}

#[test]
fn the_failed_setup_instance_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        world.instance_mut(&id("jk-7f3a")).expect("instance").status = InstanceStatus::FailedSetup;
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, "down right down enter");
        let status = screen(&mut app);
        press(&mut app, "esc shift+f10");
        let menu = screen(&mut app);
        format!("{status}\n{menu}")
    });
    assert!(demo.contains("7f3a never reached the Capsule · launch again"));
    assert!(live.contains("7f3a never got ready · launch again"));
    assert!(!live.contains("never reached"));
}

#[test]
fn the_last_session_status_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        world.instance_mut(&id("jk-7f3a")).expect("instance").status = InstanceStatus::CleanExited;
        world.daemons.remove(&id("jk-7f3a"));
        let mut app = app_with(Scenario::Returning, world);
        screen(&mut app);
        app.go(Go::Attach {
            instance: id("jk-9b02"),
            pane: None,
        });
        screen(&mut app);
        press(&mut app, "ctrl+b x right enter");
        assert_eq!(app.route(), Route::Outro);
        app.shell.status.text().unwrap_or_default().to_owned()
    });
    assert_eq!(demo, "Last session ended · the Capsule shuts down");
    assert_eq!(live, "Last session ended · the sandbox keeps running");
}

#[test]
fn the_export_prompt_follows_the_words() {
    let (demo, live) = pair(Scenario::CapsuleMulti, "ctrl+\\ E x p o r t enter");
    assert!(demo.contains("workspace path or /jackin/run/…"));
    assert!(live.contains("path in /sandbox/project") && !live.contains("/jackin/run"));
}

#[test]
fn the_terminal_screen_names_itself_by_the_words() {
    let (demo, live) = pair(Scenario::CapsuleMulti, "f10 right right right right enter");
    assert!(demo.contains("Capsule › payments") && demo.contains("  Capsule  "));
    assert!(live.contains("Capsule › payments") && live.contains("  Capsule  "));
    assert!(!live.contains("Terminal"));
}

#[test]
fn the_terminal_without_tabs_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        world.instance_mut(&id("jk-7f3a")).expect("instance").daemon = DaemonSnapshot::NoTabs;
        world.daemons.remove(&id("jk-7f3a"));
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, "down right down");
        let detail = screen(&mut app);
        press(&mut app, "i");
        format!("{detail}\n{}", screen(&mut app))
    });
    assert!(demo.contains("Daemon reports no tabs"));
    assert!(squash(&demo).contains("Daemon attached · no tabs"));
    assert!(live.contains("No Capsule tabs open"));
    assert!(squash(&live).contains("Capsule attached · no tabs"));
}

#[test]
fn the_unreachable_gateway_follows_the_words() {
    let [demo, live] = worlds(Scenario::Returning).map(|mut world| {
        world.instance_mut(&id("jk-7f3a")).expect("instance").daemon = DaemonSnapshot::Unavailable;
        world.daemons.remove(&id("jk-7f3a"));
        let mut app = app_with(Scenario::Returning, world);
        press(&mut app, "down right down");
        screen(&mut app)
    });
    assert!(demo.contains("Daemon unavailable — showing manifest sessions"));
    assert!(live.contains("Gateway unreachable — showing saved sessions"));
}

fn run(scenario: Scenario, world: jackin_console::world::World, keys: &str) -> App {
    let mut app = app_with(scenario, world);
    press(&mut app, keys);
    app
}

const GITHUB_OPEN: &str = "f10 right right down down down down enter";
const GITHUB_COPY: &str = "tab enter";

fn capsule_world(live: bool) -> jackin_console::world::World {
    if live {
        live_world(Scenario::CapsuleMulti)
    } else {
        world_for(Scenario::CapsuleMulti)
    }
}

#[test]
fn the_github_dialog_copies_the_pull_request_url() {
    let keys = format!("{GITHUB_OPEN} {GITHUB_COPY}");
    let live = run(Scenario::CapsuleMulti, capsule_world(true), &keys);
    assert_eq!(
        live.world.clipboard.as_deref(),
        Some("https://github.com/chainargos/payments-platform/pull/482")
    );
    assert_eq!(live.shell.status.text(), Some("Copied the GitHub URL"));
    let demo = run(Scenario::CapsuleMulti, capsule_world(false), &keys);
    assert_eq!(demo.world.clipboard, None);
}

#[test]
fn the_github_dialog_copies_the_branch_url_without_a_pull_request() {
    let mut world = capsule_world(true);
    for instance in &mut world.instances {
        if let Some(instance) = instance.as_available_mut() {
            instance.pr = None;
        }
    }
    let live = run(
        Scenario::CapsuleMulti,
        world,
        &format!("{GITHUB_OPEN} {GITHUB_COPY}"),
    );
    assert_eq!(
        live.world.clipboard.as_deref(),
        Some("https://github.com/chainargos/payments-platform/tree/feature/settlement-backoff")
    );
}

#[test]
fn the_github_dialog_copies_nothing_without_a_matching_repository() {
    let keys = format!("{GITHUB_OPEN} {GITHUB_COPY}");
    let unrelated = |live: bool| {
        let mut world = capsule_world(live);
        for instance in &mut world.instances {
            if let Some(instance) = instance.as_available_mut() {
                instance.github = None;
            }
        }
        run(Scenario::CapsuleMulti, world, &keys)
    };
    let live = unrelated(true);
    assert_eq!(live.world.clipboard, None);
    assert_eq!(
        live.shell.status.get(),
        Some(("No GitHub URL for this sandbox", Tone::Error))
    );
    let demo = unrelated(false);
    assert_eq!(demo.world.clipboard, None);
    assert_eq!(
        demo.shell.status.get(),
        Some(("No GitHub URL to open for this instance", Tone::Error))
    );
}

const MOUNT_OPEN: &str = "down down down down e 2 down o";

#[test]
fn the_mount_open_error_has_the_error_tone() {
    let live = run(
        Scenario::Returning,
        live_world(Scenario::Returning),
        MOUNT_OPEN,
    );
    assert_eq!(
        live.shell.status.get(),
        Some(("no GitHub URL for this mount", Tone::Error))
    );
    let demo = run(
        Scenario::Returning,
        world_for(Scenario::Returning),
        MOUNT_OPEN,
    );
    assert_eq!(
        demo.shell.status.get(),
        Some(("Opened ~/src/customer-portal on GitHub", Tone::Secondary))
    );
}

#[test]
fn the_git_mount_follows_the_words() {
    let git = |mut world: jackin_console::world::World| {
        let ws = world
            .workspaces
            .iter_mut()
            .find(|w| w.name == "customer-portal")
            .expect("workspace");
        ws.mounts[0].source = MountSource::Git("github.com/chainargos/payments-platform".into());
        world
    };
    let demo = run(
        Scenario::Returning,
        git(world_for(Scenario::Returning)),
        MOUNT_OPEN,
    );
    assert_eq!(
        demo.shell.status.text(),
        Some("Opened https://github.com/chainargos/payments-platform in the browser")
    );
    let live = run(
        Scenario::Returning,
        git(live_world(Scenario::Returning)),
        MOUNT_OPEN,
    );
    assert_eq!(
        live.shell.status.text(),
        Some("GitHub · https://github.com/chainargos/payments-platform")
    );
}

fn lifecycle_row(live: bool, status: InstanceStatus) -> String {
    let mut world = if live {
        live_world(Scenario::Returning)
    } else {
        world_for(Scenario::Returning)
    };
    let instance = world.instance_mut(&id("jk-7f3a")).expect("instance");
    instance.status = status;
    instance.unpushed = 2;
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down right down");
    let screen = screen(&mut app);
    let line = screen
        .lines()
        .find(|l| l.contains("Lifecycle  "))
        .unwrap_or_else(|| panic!("no Lifecycle row for {status:?}\n{screen}"));
    squash(&line[line.find("Lifecycle").expect("label")..])
}

#[test]
fn the_lifecycle_rows_follow_the_words() {
    let rows = [
        (
            InstanceStatus::CleanExited,
            "Lifecycle exited · The final session ended cleanly; the container is stopped.",
            "Lifecycle exited · The final session ended cleanly; nothing to attach to.",
        ),
        (
            InstanceStatus::Crashed,
            "Lifecycle crashed · The Capsule daemon stopped unexpectedly;",
            "Lifecycle crashed · The agent exited with an error; the sandbox is kept for",
        ),
        (
            InstanceStatus::PreservedDirty,
            "Lifecycle preserved · dirty · Stopped with uncommitted changes in the",
            "Lifecycle preserved · dirty · Has uncommitted changes in the sandbox; kept until you",
        ),
        (
            InstanceStatus::PreservedUnpushed,
            "Lifecycle preserved · unpushed · Stopped with commits that were never pushed;",
            "Lifecycle preserved · unexported · Has commits that were never exported; kept until",
        ),
        (
            InstanceStatus::RestoreAvailable,
            "Lifecycle restore available · Stopped cleanly with a restorable Capsule layout.",
            "Lifecycle restore available · Stopped cleanly; restore it to continue.",
        ),
        (
            InstanceStatus::FailedSetup,
            "Lifecycle failed setup · Launch failed before the Capsule was ready;",
            "Lifecycle failed setup · Launch failed before the sandbox was ready; nothing to",
        ),
    ];
    for (status, demo, live) in rows {
        let (shown_demo, shown_live) = (lifecycle_row(false, status), lifecycle_row(true, status));
        assert!(shown_demo.contains(demo), "{status:?} demo: {shown_demo}");
        assert!(shown_live.contains(live), "{status:?} live: {shown_live}");
    }
}

#[test]
fn the_project_count_follows_the_words() {
    let (demo, live) = summary_pair("down");
    assert!(demo.contains("2 mounts"), "{demo}");
    assert!(
        live.contains("2 projects") && !live.contains("mounts"),
        "{live}"
    );
    let (demo, live) = summary_pair("down down");
    assert!(
        demo.contains("1 mount") && !demo.contains("1 mounts"),
        "{demo}"
    );
    assert!(
        live.contains("1 project") && !live.contains("1 projects"),
        "{live}"
    );
}

#[test]
fn the_trust_reveal_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s 5 tab down down o");
    assert!(demo.contains("Revealed ~/roles in Finder"), "{demo}");
    assert!(
        live.contains("Source · ~/roles") && !live.contains("Revealed"),
        "{live}"
    );
}

fn unexported_world(live: bool) -> jackin_console::world::World {
    let mut world = if live {
        live_world(Scenario::Returning)
    } else {
        world_for(Scenario::Returning)
    };
    let preserved = world.instance_mut(&id("jk-7f3a")).expect("instance");
    preserved.status = InstanceStatus::PreservedUnpushed;
    preserved.unpushed = 2;
    let capsule = world.instance_mut(&id("jk-9b02")).expect("instance");
    capsule.unpushed = 2;
    capsule.uncommitted = 3;
    world
}

fn unexported_captures(live: bool) -> Vec<(&'static str, String)> {
    let at = |keys: &str, width: u16, height: u16| {
        let mut app = app_with(Scenario::Returning, unexported_world(live));
        press(&mut app, keys);
        screen_at(&mut app, width, height)
    };
    let capsule = |keys: &str| {
        let mut app = app_with(Scenario::Returning, unexported_world(live));
        screen(&mut app);
        app.go(Go::Attach {
            instance: id("jk-9b02"),
            pane: None,
        });
        screen(&mut app);
        press(&mut app, keys);
        screen(&mut app)
    };
    vec![
        ("roster card", at("", 140, 40)),
        ("roster row", at("", 240, 60)),
        ("summary status", at("down right down", 80, 24)),
        ("summary working tree", at("down right down", 100, 30)),
        ("inspect dialog", at("down right down i", 140, 40)),
        ("detail", at("down right down", 140, 40)),
        ("capsule status bar", capsule("")),
        ("dirty quit line", capsule("ctrl+q")),
    ]
}

#[test]
fn the_unexported_word_replaces_unpushed() {
    let demo = unexported_captures(false);
    let live = unexported_captures(true);
    let mut wrong = vec![];
    for ((name, demo), (_, live)) in demo.iter().zip(&live) {
        if !demo.contains("unpushed") || !live.contains("unexported") || live.contains("unpushed") {
            wrong.push(*name);
        }
    }
    assert!(wrong.is_empty(), "captures with the wrong word: {wrong:?}");
}

#[test]
fn the_dirty_quit_dialog_follows_the_words() {
    let steps = |live: bool| {
        let mut app = app_with(Scenario::Returning, unexported_world(live));
        screen(&mut app);
        app.go(Go::Attach {
            instance: id("jk-9b02"),
            pane: None,
        });
        screen(&mut app);
        press(&mut app, "ctrl+q");
        let options = screen(&mut app);
        press(&mut app, "down down down enter");
        (options, screen(&mut app))
    };
    let ((demo_options, demo_confirm), (live_options, live_confirm)) = (steps(false), steps(true));
    assert!(demo_options.contains("Quit & keep changes"));
    assert!(demo_options.contains("Quit & discard changes"));
    assert!(
        live_options.contains("Quit & keep the sandbox"),
        "{live_options}"
    );
    assert!(
        live_options.contains("Quit & delete the sandbox"),
        "{live_options}"
    );
    assert!(
        demo_confirm.contains("Quit and discard changes?"),
        "{demo_confirm}"
    );
    assert!(
        demo_confirm.contains("Action      discard every uncommitted"),
        "{demo_confirm}"
    );
    assert!(
        live_confirm.contains("Delete the sandbox and quit?"),
        "{live_confirm}"
    );
    assert!(
        live_confirm.contains("delete the sandbox and all work in it"),
        "{live_confirm}"
    );
    let buttons = live_confirm
        .lines()
        .find(|l| l.contains("Cancel") && l.contains("Delete"));
    assert!(buttons.is_some(), "{live_confirm}");
}
