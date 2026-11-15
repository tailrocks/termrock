#![cfg(feature = "demo")]

mod capsule_support;
mod common;
mod pane_support;
mod product_support;

use jackin_console::arbiter::{Arbiter, DiscoveryError};
use jackin_console::domain::surfaces::Surfaces;
use jackin_console::fixtures::{CLOCK, HOME, Scenario, world_for};
use jackin_console::sim::SimBackend;
use jackin_console::sim::onepassword::SimOnePassword;
use jackin_console::world::{DaemonHealth, World, WorldParts};

fn parts() -> WorldParts {
    WorldParts::new(
        CLOCK,
        HOME.into(),
        format!("{HOME}/src/demo"),
        Box::new(SimOnePassword::fixture(CLOCK.now_secs())),
        SimBackend::shared(),
        Surfaces::demo(Scenario::Returning),
    )
}

#[test]
fn every_scenario_builds_a_world_that_names_it() {
    for scenario in Scenario::ALL {
        let world = world_for(scenario);
        let named = world
            .surfaces
            .about
            .iter()
            .find(|(label, _)| label == "Scenario")
            .map(|(_, value)| value.as_str());
        assert_eq!(named, Some(scenario.name()), "{}", scenario.name());
    }
    assert!(world_for(Scenario::FirstUse).workspaces.is_empty());
    assert!(world_for(Scenario::FirstUse).discovery_pending);
    assert!(!world_for(Scenario::Returning).workspaces.is_empty());
}

#[test]
fn world_new_fills_what_the_parts_leave_out() {
    let world = World::new(parts());
    assert!(world.workspaces.is_empty() && world.roles.is_empty() && world.instances.is_empty());
    assert!(world.daemons.is_empty() && world.jobs.is_empty() && world.fs.is_empty());
    assert_eq!(world.clipboard, None);
    assert_eq!(world.daemon_health, DaemonHealth::Healthy);
    assert!(!world.discovery_pending);
    assert!(world.session_accounts.is_empty());
    assert_eq!(world.last_refresh_secs, CLOCK.now_secs());
    assert_eq!(world.cwd, format!("{HOME}/src/demo"));
}

#[test]
fn world_new_keeps_the_parts_it_is_given() {
    let source = world_for(Scenario::Returning);
    let world = World::new(WorldParts {
        workspaces: source.workspaces.clone(),
        last_refresh_secs: 5,
        ..parts()
    });
    assert_eq!(world.workspaces, source.workspaces);
    assert_eq!(world.last_refresh_secs, 5);
}

#[test]
fn world_new_counts_the_running_instances_in_the_arbiter() {
    let source = world_for(Scenario::Returning);
    let running = source.running_count();
    assert!(running > 0);
    let world = World::new(WorldParts {
        instances: source.instances.clone(),
        ..parts()
    });
    assert_eq!(world.arbiter.running(), Ok(running));
}

#[test]
fn world_new_keeps_an_unreadable_instance_index() {
    let source = world_for(Scenario::Returning);
    let mut arbiter = Arbiter::new(0);
    arbiter.discovery = Err(DiscoveryError::IndexUnreadable);
    let world = World::new(WorldParts {
        arbiter,
        instances: source.instances.clone(),
        ..parts()
    });
    assert_eq!(
        world.arbiter.running(),
        Err(DiscoveryError::IndexUnreadable)
    );
}

#[test]
fn a_path_under_a_longer_home_name_is_not_abbreviated() {
    let mut world = World::new(parts());
    world.home = "/home/dev".into();
    assert_eq!(
        world.tilde("/home/developer/project"),
        "/home/developer/project"
    );
    assert_eq!(world.tilde("/home/dev/project"), "~/project");
    assert_eq!(world.tilde("/home/dev"), "~");
    assert_eq!(world.mask_path("/home/developer/project/src"), "…/src");
    assert_eq!(world.mask_path("/home/dev/project/src"), "~/…/src");
    assert_eq!(world.mask_path("/home/dev"), "~");
}

#[test]
fn running_entries_keep_ready_unavailable_sandboxes_identifiable() {
    use jackin_console::domain::instance::{
        InstanceEntry, InstanceId, SandboxPhase, UnavailableInstance,
    };
    let mut world = World::new(parts());
    let ready = InstanceId::new("gw-a", "box-ready");
    let stopped = InstanceId::new("gw-a", "box-stopped");
    world.admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
        ready.clone(),
        None,
        None,
        Some("api".into()),
        SandboxPhase::Ready,
        None,
        "metadata degraded",
    )));
    world.admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
        stopped.clone(),
        None,
        None,
        Some("api".into()),
        SandboxPhase::Stopped,
        None,
        "sandbox stopped",
    )));
    let running: Vec<InstanceId> = world
        .running_entries()
        .iter()
        .map(|entry| entry.id().clone())
        .collect();
    assert!(
        running.contains(&ready),
        "a ready sandbox with degraded metadata must stay in the running view: {running:?}"
    );
    assert!(
        !running.contains(&stopped),
        "a stopped sandbox must not be in the running view: {running:?}"
    );
    assert_eq!(
        world.running_entries().len(),
        world.running_count(),
        "the running view must agree with the running count"
    );
}

mod projection_work {
    use crate::capsule_support::{Rig, instance};
    use crate::pane_support::INSTANCE;
    use jackin_console::app::Remote;
    use jackin_console::domain::instance::{
        DaemonSnapshot, InstanceEntry, InstanceId, InstanceStatus, SandboxPhase,
        UnavailableInstance,
    };
    use jackin_console::domain::workspace::Workspace;
    use jackin_console::fixtures::{CLOCK, HOME, Scenario, world_for};
    use jackin_console::pages::Pages;
    use jackin_console::panes::{Daemon, Lease, LinkEvent, PaneId};
    use jackin_console::world::{Msg, WorkSnapshot, World, WorldParts, refresh_snapshot};
    use junie_tui::core::event::{External, Input, Outcome};
    use junie_tui::runtime::Application;

    use super::parts;

    fn bulk(world: &mut World, workspaces: u32, per_workspace: usize) {
        let template = instance();
        for ws in 1..=workspaces {
            world
                .workspaces
                .push(Workspace::new(1000 + ws, &format!("bulk-{ws}"), "/work"));
            for n in 0..per_workspace {
                let mut i = template.clone();
                i.id = InstanceId::simulated(format!("jk-bulk-{ws}-{n}"));
                i.workspace = Some(1000 + ws);
                i.status = match n % 5 {
                    0 => InstanceStatus::Running,
                    1 => InstanceStatus::Purged,
                    2 => InstanceStatus::Crashed,
                    _ => InstanceStatus::CleanExited,
                };
                world.instances.push(InstanceEntry::available(i));
            }
        }
        world.mark_membership_changed();
    }

    fn spent(world: &World, since: WorkSnapshot) -> WorkSnapshot {
        world.work().since(since)
    }

    fn tick(rig: &mut Rig) -> Outcome {
        let out = Application::handle(&mut rig.app, Input::Tick);
        rig.screen();
        out
    }

    fn wake(rig: &mut Rig, pane: PaneId) -> Outcome {
        let msg = Msg::PaneWake {
            instance: InstanceId::simulated(INSTANCE),
            pane,
        };
        Application::handle(
            &mut rig.app,
            Input::External(External::new(Remote::Msg(msg))),
        )
    }

    fn pane(rig: &Rig) -> PaneId {
        rig.daemon().panes[0].id
    }

    fn manager_over_quiet_pane(prepare: impl FnOnce(&mut World)) -> Rig {
        let mut rig = Rig::with(Default::default(), prepare);
        rig.attach();
        rig.press("ctrl+b d");
        let now = rig.app.world.now_secs();
        rig.app.world.refresh_from = Some(rig.app.world.snapshots_applied);
        rig.app.world.refresh_started_secs = now;
        rig.screen();
        rig
    }

    #[test]
    fn quiet_panes_do_not_rebuild_roster_or_accounts() {
        let mut rig = manager_over_quiet_pane(|w| bulk(w, 1000, 10));
        assert_eq!(rig.app.world.instances.len(), 10_001);
        assert!(
            !rig.app.world.daemons.is_empty() && !rig.daemon().panes.is_empty(),
            "a retained pane is still polled"
        );
        let epochs = rig.app.world.epochs();
        let before = rig.app.world.work();
        for _ in 0..100 {
            tick(&mut rig);
        }
        let used = spent(&rig.app.world, before);
        assert_eq!(rig.app.world.epochs(), epochs, "no tick is an edit");
        assert_eq!(used.projection_work(), 0, "{used:?}");
        assert_eq!(used.membership_visits, 0, "{used:?}");
        assert_eq!(used.link_polls, 100, "the link is still polled every tick");
    }

    #[test]
    fn roster_index_visits_each_instance_once() {
        for workspaces in [10, 100, 1000] {
            let mut world = World::new(parts());
            let per = 10_000 / workspaces as usize;
            bulk(&mut world, workspaces, per);
            for n in 0..25 {
                world.admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
                    InstanceId::new("gw", format!("sandbox-{n}")),
                    (n % 2 == 0).then_some(1001),
                    None,
                    Some("api".into()),
                    SandboxPhase::Ready,
                    None,
                    "metadata degraded",
                )));
            }
            let before = world.work();
            let roster = world.roster();
            let built = spent(&world, before);
            assert_eq!(built.source_builds, 1);
            assert_eq!(
                built.membership_visits,
                world.instances.len() as u64,
                "{workspaces} workspaces"
            );
            let again = world.roster();
            assert_eq!(spent(&world, before), built, "a second read is free");
            assert!(std::sync::Arc::ptr_eq(&roster, &again));

            for ws in &world.workspaces {
                let group = roster.group(Some(ws.id));
                let from_scan = world.entries_of(Some(ws.id));
                let from_index: Vec<_> = group
                    .map(|g| g.indices.iter().map(|at| &world.instances[*at]).collect())
                    .unwrap_or_default();
                assert_eq!(from_index, from_scan, "workspace {}", ws.id);
                assert_eq!(
                    group.map_or(0, |g| g.running),
                    world.running_count_of(Some(ws.id))
                );
            }
            assert_eq!(roster.running_len(), world.running_count());
            assert!(
                world
                    .instances
                    .iter()
                    .any(|entry| entry.as_available().is_some_and(|i| i.status.hidden())),
                "hidden instances are in the fixture"
            );
        }
    }

    #[test]
    fn one_epoch_change_builds_once_before_render() {
        let mut rig = manager_over_quiet_pane(|w| bulk(w, 50, 4));
        let selected = rig.app.router.manager().selected.clone();
        let before = rig.app.world.work();
        rig.app.world.workspace_mut(1001).expect("workspace").name = "renamed".into();
        tick(&mut rig);
        Application::handle(&mut rig.app, Input::Key(crate::pane_support::key("x")));
        let shown = rig.screen();
        let used = spent(&rig.app.world, before);
        assert_eq!(used.manager_builds, 1, "{used:?}");
        assert_eq!(used.source_builds, 0, "a label edit keeps the membership");
        assert!(shown.contains("renamed"));
        assert_eq!(
            rig.app.router.manager().selected,
            selected,
            "selection is stable"
        );
        let before = rig.app.world.work();
        rig.screen();
        assert_eq!(spent(&rig.app.world, before).manager_builds, 0);
    }

    #[test]
    fn account_sort_is_once_per_registry_epoch() {
        use crate::common::{app_with, press};
        let mut app = app_with(Scenario::Returning, world_for(Scenario::Returning));
        press(&mut app, "c");
        let surfaces = app
            .world
            .accounts
            .accounts
            .iter()
            .map(|a| a.surface)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(surfaces.len() > 1, "several usage surfaces share one sort");
        let sorts = |app: &jackin_console::app::App| app.world.work().account_sorts;
        let settled = sorts(&app);
        assert_eq!(settled, 1, "opening the page sorts the registry once");

        let before = app.world.work();
        for keys in ["down", "space", "down", "space", "up", "down", "up"] {
            press(&mut app, keys);
        }
        press(&mut app, "/");
        press(&mut app, "a");
        press(&mut app, "esc");
        let used = app.world.work().since(before);
        assert_eq!(used.account_sorts, 0, "fold, filter and cursor: {used:?}");
        assert!(used.accounts_builds >= 2, "fold and filter rebuild rows");

        let before = app.world.work();
        app.world.mark_accounts_changed();
        press(&mut app, "down");
        press(&mut app, "up");
        let used = app.world.work().since(before);
        assert_eq!(used.account_sorts, 1, "a registry change sorts once");
        assert_eq!(used.accounts_builds, 1, "and builds the rows once");
    }

    #[test]
    fn pane_output_replay_and_expiry_still_invalidate() {
        let mut rig = Rig::new();
        rig.advance(80);
        rig.bench.push(0, LinkEvent::Attached);
        rig.bench.push(0, LinkEvent::Lease(Lease::Owner));
        let window: Vec<Outcome> = (0..12).map(|_| tick(&mut rig)).collect();
        assert!(
            window.contains(&Outcome::Changed),
            "the replay window closing is drawn: {window:?}"
        );
        assert_eq!(
            &window[window.len() - 3..],
            [Outcome::Ignored; 3],
            "a quiet pane after the window asks for nothing: {window:?}"
        );

        rig.bench.output(0, b"visible output");
        assert_eq!(tick(&mut rig), Outcome::Changed, "visible output");
        assert_eq!(tick(&mut rig), Outcome::Ignored, "empty following tick");

        rig.app
            .router
            .capsule_mut()
            .expect("capsule")
            .prefix_timeout_ms = Some(500);
        rig.press("ctrl+b");
        let held: Vec<Outcome> = (0..150).map(|_| tick(&mut rig)).collect();
        assert!(
            held.contains(&Outcome::Changed),
            "the prefix timing out is drawn"
        );
        assert_eq!(
            &held[held.len() - 3..],
            [Outcome::Ignored; 3],
            "and then nothing is asked for"
        );

        rig.app.set_status("saved", junie_tui::theme::Tone::Normal);
        let shown: Vec<Outcome> = (0..120).map(|_| tick(&mut rig)).collect();
        assert!(shown.contains(&Outcome::Changed), "status expiry is drawn");
        assert_eq!(&shown[shown.len() - 3..], [Outcome::Ignored; 3]);
    }

    #[test]
    fn background_summary_invalidates_manager_once() {
        let mut rig = manager_over_quiet_pane(|_| {});
        let pane = pane(&rig);
        rig.press("up");
        assert_ne!(
            rig.app.router.manager().selected,
            jackin_console::screens::manager::RowKey::Instance(InstanceId::simulated(INSTANCE))
        );
        let before = rig.app.world.work();
        let epochs = rig.app.world.epochs();

        rig.bench.output(0, b"plain grid output");
        assert_eq!(
            wake(&mut rig, pane),
            Outcome::Ignored,
            "grid-only, not visible"
        );
        rig.screen();
        assert_eq!(
            rig.app.world.epochs(),
            epochs,
            "a grid never bumps an epoch"
        );
        let used = spent(&rig.app.world, before);
        assert_eq!(
            (used.source_builds, used.manager_builds),
            (0, 0),
            "{used:?}"
        );

        rig.bench.push(0, LinkEvent::Exited(Some(0)));
        assert_eq!(wake(&mut rig, pane), Outcome::Changed, "the summary moved");
        assert_eq!(rig.app.world.epochs().membership, epochs.membership);
        assert_eq!(rig.app.world.epochs().roster, epochs.roster + 1);
        rig.screen();
        rig.screen();
        let used = spent(&rig.app.world, before);
        assert_eq!(used.manager_builds, 1, "{used:?}");
        assert_eq!(used.source_builds, 0, "membership is untouched: {used:?}");
    }

    #[test]
    fn visible_age_labels_update_without_structural_projection() {
        let mut rig = Rig::with(Default::default(), |w| {
            let now = w.now_secs();
            w.workspaces.push(Workspace::new(5, "ages", "/work"));
            let i = w
                .instance_mut(&InstanceId::simulated(INSTANCE))
                .expect("instance");
            i.created_secs = now - 58;
            i.workspace = Some(5);
        });
        rig.attach();
        rig.press("ctrl+b d");
        rig.app.world.refresh_from = Some(rig.app.world.snapshots_applied);
        rig.app.world.refresh_started_secs = rig.app.world.now_secs();
        let first = rig.flat();
        assert!(first.contains("Started 59 s ago"), "{first}");
        let before = rig.app.world.work();
        let mut repainted = 0;
        for _ in 0..40 {
            if tick(&mut rig) == Outcome::Changed {
                repainted += 1;
            }
        }
        let later = rig.flat();
        assert!(
            !later.contains("Started 59 s ago") && later.contains("Started 1 min ago"),
            "the age moved on: {later}"
        );
        let used = spent(&rig.app.world, before);
        assert_eq!(used.projection_work(), 0, "{used:?}");
        assert_eq!(used.membership_visits, 0, "{used:?}");
        assert!(
            (3..=6).contains(&repainted),
            "one repaint per whole second, not per tick: {repainted}"
        );
    }

    #[test]
    fn quiet_many_daemons_do_not_scan_full_roster() {
        let mut world = World::new(WorldParts {
            instances: vec![],
            ..parts()
        });
        bulk(&mut world, 1000, 10);
        for ws in 1..=1000u32 {
            let id = InstanceId::simulated(format!("jk-bulk-{ws}-0"));
            world.daemons.insert(id, Daemon::new("bulk"));
        }
        world.clock.running = true;
        world.tick(80);
        let before = world.work();
        for _ in 0..100 {
            world.tick(80);
        }
        let used = spent(&world, before);
        assert_eq!(used.membership_visits, 0, "{used:?}");
        assert_eq!(used.source_builds, 0, "{used:?}");
        assert_eq!(used.running_lookups, 100 * 1000, "{used:?}");
        assert_eq!(
            used.link_polls,
            100 * 1000,
            "every running daemon is polled"
        );
    }

    fn open_second_tab(rig: &mut Rig) {
        rig.press("ctrl+b c");
        rig.typed("Shell");
        rig.press("enter");
        rig.advance(80);
        let n = rig.bench.opened() - 1;
        rig.bench.push(n, LinkEvent::Attached);
        rig.bench.push(n, LinkEvent::Lease(Lease::Owner));
        rig.advance(900);
    }

    #[test]
    fn pane_summary_does_not_rebuild_membership_index() {
        let mut rig = Rig::new();
        rig.attach();
        open_second_tab(&mut rig);
        rig.app.world.roster();
        let before = rig.app.world.work();
        let epochs = rig.app.world.epochs();

        rig.bench.output(0, b"plain output");
        let quiet = rig.app.world.tick_with_changes(80);
        assert_eq!(quiet.panes_changed.len(), 1, "the grid changed");
        assert!(quiet.summaries_changed.is_empty());
        assert_eq!(rig.app.world.epochs(), epochs);

        rig.bench.output(0, b"\x07");
        let bell = rig.app.world.tick_with_changes(80);
        assert_eq!(bell.summaries_changed.len(), 1);
        let after = rig.app.world.epochs();
        assert_eq!(after.membership, epochs.membership);
        assert_eq!(after.roster, epochs.roster + 1);
        rig.app.world.roster();
        assert_eq!(spent(&rig.app.world, before).source_builds, 0);
    }

    #[test]
    fn public_tick_compatibility_still_delivers_messages() {
        let hard = || {
            let source = world_for(Scenario::Returning);
            let mut world = World::new(WorldParts {
                instances: source.instances.clone(),
                backend: jackin_console::sim::SimBackend::hard_cases(),
                ..parts()
            });
            world.clock.running = true;
            world
        };
        let plain = hard().tick(45_000);
        assert!(
            plain.iter().any(|m| matches!(m, Msg::Takeover { .. })),
            "the backend's due message arrives: {plain:?}"
        );
        let rich = hard().tick_with_changes(45_000);
        assert_eq!(
            rich.messages, plain,
            "tick is the messages of the richer tick"
        );
        assert!(rich.panes_changed.is_empty() && rich.summaries_changed.is_empty());
    }

    #[test]
    fn every_real_mutation_moves_its_epoch_and_a_no_op_moves_none() {
        let mut world = world_for(Scenario::Returning);
        let id = world.instances[0].id().clone();
        let ws = world.workspaces[0].id;
        let account = world.accounts.accounts[0].id.clone();
        let e = |w: &World| w.epochs();

        let base = e(&world);
        assert_eq!(base, e(&world), "reads move nothing");
        world.roster();
        world.sorted_accounts();
        assert_eq!(base, e(&world), "building a view moves nothing");
        assert!(!refresh_snapshot(&mut world, &id), "no daemon, nothing new");
        assert_eq!(base, e(&world));

        world.admit_entry(world.instances[0].clone());
        let a = e(&world);
        assert_eq!(
            (a.membership, a.roster),
            (base.membership + 1, base.roster + 1)
        );
        assert_eq!(a.accounts, base.accounts);

        world.instance_view_mut(&id).expect("instance").uncommitted += 1;
        let b = e(&world);
        assert_eq!((b.membership, b.roster), (a.membership, a.roster + 1));

        world.workspace_mut(ws).expect("workspace").name = "other".into();
        let c = e(&world);
        assert_eq!((c.membership, c.roster), (b.membership, b.roster + 1));

        world.account_mut(&account).expect("account").enabled = false;
        let d = e(&world);
        assert_eq!((d.roster, d.accounts), (c.roster, c.accounts + 1));

        world.instance_mut(&id).expect("instance").status = InstanceStatus::CleanExited;
        let f = e(&world);
        assert_eq!((f.membership, f.roster), (d.membership + 1, d.roster + 1));

        let snapshot = jackin_console::world::Snapshot {
            workspaces: world.workspaces.clone(),
            roles: world.roles.clone(),
            instances: world.instances.clone(),
            global: world.global.clone(),
            accounts: world.accounts.clone(),
            surfaces: world.surfaces.clone(),
            github: world.github.clone(),
            index_error: None,
            daemon_health: world.daemon_health,
        };
        world.apply(snapshot);
        let g = e(&world);
        assert_eq!(
            (g.membership, g.roster, g.accounts),
            (f.membership + 1, f.roster + 1, f.accounts + 1),
            "a complete snapshot moves all three once"
        );
        let _ = (CLOCK, HOME, DaemonSnapshot::Unavailable);
    }

    #[test]
    fn account_replies_move_the_accounts_epoch_only_for_the_source_they_were_made_for() {
        use jackin_console::backend::{Refresh, Validation};
        use jackin_console::domain::reason::RefreshOutcome;
        use jackin_console::domain::usage::AccountUsage;

        let mut world = world_for(Scenario::Returning);
        assert!(
            world.accounts.accounts.len() > 1,
            "two sources to tell apart"
        );
        let account = world.accounts.accounts[0].id.clone();
        let source = world.accounts.accounts[0].source.clone();
        let other = world.accounts.accounts[1].source.clone();
        assert_ne!(source, other);
        let refresh_ticket = world
            .refresh_account(&account, 0)
            .expect("a refresh ticket");
        let base = world.epochs();

        let refreshed = |ticket, source: &_, result| Msg::AccountRefreshed {
            ticket,
            account: account.clone(),
            source: Box::new(Clone::clone(source)),
            result,
        };
        let validated = |source: &_, result| Msg::AccountValidated {
            account: account.clone(),
            source: Box::new(Clone::clone(source)),
            result,
        };
        let refresh = || {
            Ok(Box::new(Refresh {
                usage: AccountUsage::none(),
                issue: None,
                last_refresh_secs: 1,
                outcome: RefreshOutcome::Current,
            }))
        };

        world.absorb(&refreshed(refresh_ticket, &other, refresh()));
        world.absorb(&validated(&other, Err("late".into())));
        assert_eq!(
            world.epochs(),
            base,
            "a reply for a replaced source is dropped"
        );

        world.absorb(&refreshed(refresh_ticket, &source, refresh()));
        let a = world.epochs();
        assert_eq!(
            (a.membership, a.roster, a.accounts),
            (base.membership, base.roster, base.accounts + 1),
            "an accepted refresh"
        );

        let refresh_ticket = world
            .refresh_account(&account, 0)
            .expect("a refresh ticket");
        let started = world.epochs();
        assert_eq!(started.accounts, a.accounts + 1, "a refresh start");
        world.absorb(&refreshed(refresh_ticket, &source, Err("down".into())));
        let b = world.epochs();
        assert_eq!(
            b.accounts,
            started.accounts + 1,
            "a failed refresh still changes the account"
        );

        world.absorb(&validated(&source, Err("rejected".into())));
        let c = world.epochs();
        assert_eq!(c.accounts, b.accounts + 1, "an accepted validation");
        assert_eq!((c.membership, c.roster), (base.membership, base.roster));

        world.absorb(&validated(
            &source,
            Ok(Validation::Fingerprint {
                level: jackin_console::domain::account::ValidationLevel::MaterialDiscovered,
            }),
        ));
        assert_eq!(
            world.epochs().accounts,
            c.accounts + 1,
            "a fingerprint check"
        );
    }
}
