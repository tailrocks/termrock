#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::{Backend, Completion, Effect};
use jackin_console::domain::account::LaunchBlock;
use jackin_console::domain::account::{
    Account, CredentialSource, Lifecycle, ValidationLevel, ValidationState,
};
use jackin_console::domain::agent::{Agent, AgentBlocked, AuthMode, Provider};
use jackin_console::domain::instance::{
    DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus,
};
use jackin_console::domain::launch::{LaunchContent, RefusalReason};
use jackin_console::domain::openshell;
use jackin_console::domain::role::AccountOrder;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::tuple::{
    AccountRequirement, InstanceChoice, TupleIdentity, TupleOffer, TupleState,
};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{EnvAnswer, EnvQuestion, LaunchEvent, LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::screens::Go;
use jackin_console::sim::launch::validate_tuples;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, Snapshot, World};
use termrock::core::event::Input;
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

const INFRA: u32 = 2;
const PORTAL: u32 = 4;

fn available(entry: &InstanceEntry) -> &Instance {
    entry
        .as_available()
        .expect("simulated instance has verified metadata")
}

fn started(world: World) -> App {
    app_with_world(
        Scenario::LaunchChecks,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..600 {
        if done(app) {
            return;
        }
        Application::handle(app, Input::Tick);
    }
    panic!("never reached: {what}\n{}", screen(app));
}

fn shows(text: &'static str) -> impl Fn(&mut App) -> bool {
    move |app| screen(app).contains(text)
}

fn supported_tuple(agent: Agent, provider: Provider, model: &str) -> TupleOffer {
    TupleOffer::supported(agent, provider, &[model])
}

fn unavailable_tuple(agent: Agent, provider: Provider, model: &str) -> TupleOffer {
    let mut offer = supported_tuple(agent, provider, model);
    offer.state = TupleState::Unavailable;
    offer.reason = Some("provider unavailable".into());
    offer
}

fn remove_provider_accounts(world: &mut World, provider: Provider) {
    world
        .accounts
        .accounts
        .retain(|account| account.provider != provider);
}

fn daemon_tab_counts(world: &World) -> Vec<(InstanceId, usize)> {
    world
        .daemons
        .iter()
        .map(|(id, daemon)| (id.clone(), daemon.tabs.len()))
        .collect()
}

fn newly_active_daemon(world: &World, before: &[(InstanceId, usize)]) -> (InstanceId, usize) {
    let mut changed: Vec<(InstanceId, usize)> = world
        .daemons
        .iter()
        .filter_map(|(id, daemon)| {
            let previous = before
                .iter()
                .find(|(previous_id, _)| previous_id == id)
                .map_or(0, |(_, tabs)| *tabs);
            (daemon.tabs.len() > previous).then(|| (id.clone(), daemon.tabs.len()))
        })
        .collect();
    assert_eq!(changed.len(), 1, "expected one session daemon to open");
    changed.remove(0)
}

fn ready_account(id: &str, name: &str, provider: Provider) -> Account {
    let mut account = Account::registered(
        id,
        name,
        provider,
        CredentialSource::PlainApiKey {
            fingerprint: format!("fixture-{id}"),
            tail: "test".into(),
        },
    );
    account.lifecycle = Lifecycle::Available;
    account.default_for_provider = true;
    account.validation = ValidationState::Valid(ValidationLevel::IdentityAuthenticated);
    account
}

fn tuple_world(tuples: Vec<TupleOffer>, accounts: Vec<Account>) -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.tuples = tuples;
    world.surfaces.agent_blocked = None;
    world.global.agent_modes.insert(Agent::Omp, AuthMode::Sync);
    let providers: Vec<Provider> = accounts.iter().map(|account| account.provider).collect();
    world
        .accounts
        .accounts
        .retain(|existing| !providers.contains(&existing.provider));
    for account in accounts {
        world.accounts.insert(account);
    }
    world.backend = SimBackend::shared();
    world
}

fn live_tuple_world(tuples: Vec<TupleOffer>, accounts: Vec<Account>) -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces = Surfaces::openshell(LiveFacts {
        version: "test".into(),
        config_path: "/tmp/jackin-test.toml".into(),
        gateway: None,
    });
    world.surfaces.tuples = tuples;
    openshell::global(&mut world.global);
    let providers: Vec<Provider> = accounts.iter().map(|account| account.provider).collect();
    world
        .accounts
        .accounts
        .retain(|existing| !providers.contains(&existing.provider));
    for account in accounts {
        world.accounts.insert(account);
    }
    world.backend = SimBackend::shared();
    world
}

fn bind_role(world: &mut World, bindings: &[(Agent, &str)]) {
    let mut admin = world_for(Scenario::RoleBinding)
        .surfaces
        .roles
        .expect("role admin fixture");
    admin.agents = Agent::ALL.to_vec();
    admin.order = AccountOrder::Jackin;
    for record in &mut admin.roles {
        record.bindings.clear();
    }
    let record = admin.role_mut("the-architect").expect("launch role");
    for (agent, account) in bindings {
        record.bindings.insert(*agent, (*account).into());
    }
    world.surfaces.roles = Some(admin);
}

#[derive(Debug)]
struct LaunchRecorder {
    inner: Arc<dyn Backend>,
    targets: Mutex<Vec<LaunchTarget>>,
}

impl LaunchRecorder {
    fn install(world: &mut World) -> Arc<Self> {
        let recorder = Arc::new(Self {
            inner: SimBackend::shared(),
            targets: Mutex::new(vec![]),
        });
        world.backend = recorder.clone();
        recorder
    }

    fn count(&self) -> usize {
        self.targets.lock().expect("launch records").len()
    }
}

impl Backend for LaunchRecorder {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.inner.perform(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn account_requirement(&self, tuple: TupleIdentity) -> AccountRequirement {
        self.inner.account_requirement(tuple)
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.targets
            .lock()
            .expect("launch records")
            .push(target.clone());
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }
}

fn running_instance() -> Instance {
    Instance {
        id: InstanceId::simulated("jk-session-test"),
        container: "jackin-payments-platform".into(),
        workspace: Some(1),
        workdir: Some("/workspace/payments-platform".into()),
        role: Some("the-architect".into()),
        agent: Agent::Codex,
        status: InstanceStatus::Running,
        created_secs: 0,
        last_seen_secs: 0,
        run_id: "run-session-test".into(),
        sessions: Ok(vec![]),
        daemon: DaemonSnapshot::Unavailable,
        branch: None,
        pr: None,
        default_branch: "main".into(),
        github: None,
        uncommitted: 0,
        unpushed: 0,
        account: None,
        accounts: vec![],
        agents: vec![],
        task: None,
    }
}

fn manager_session_picker(mut world: World) -> App {
    world
        .instances
        .push(InstanceEntry::available(running_instance()));
    let mut app = started(world);
    press(&mut app, "down");
    press(&mut app, "right");
    press(&mut app, "down");
    press(&mut app, "n");
    assert!(screen(&mut app).contains("New session · choose Agent"));
    app
}

fn capsule_new_tab_picker(world: World) -> App {
    let mut app = manager_session_picker(world);
    click_picker_option(&mut app, "New session · choose Agent", "Codex");
    assert_eq!(app.route(), Route::Capsule);
    press(&mut app, "ctrl+b c");
    assert!(screen(&mut app).contains("New tab"));
    app
}

fn accountless_tuple_world() -> World {
    let tuple = TupleIdentity::new(Agent::Omp, Provider::OpenCode);
    let mut world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::OpenCode,
            "opencode-default",
        )],
        vec![],
    );
    world.backend = SimBackend::guarded_accountless(tuple);
    world
}

#[test]
fn the_simulator_accepts_each_allowed_model_and_refuses_unknown_models() {
    let model_world = || {
        let mut offer = supported_tuple(Agent::Codex, Provider::OpenAi, "model-a");
        offer.allowed_models = vec!["model-a".into(), "model-b".into()];
        tuple_world(
            vec![offer],
            vec![
                ready_account("fixture-model-account", "Model account", Provider::OpenAi),
                ready_account(
                    "fixture-wrong-provider",
                    "Wrong provider account",
                    Provider::Moonshot,
                ),
            ],
        )
    };
    let choice = |model: &str| InstanceChoice {
        agent: Agent::Codex,
        provider: Provider::OpenAi,
        account: Some("fixture-model-account".into()),
        model: model.into(),
    };

    for model in ["model-a", "model-b"] {
        let mut app = started(model_world());
        let before = app.world.instances.len();
        app.go(Go::LaunchMany {
            workspace: None,
            role: "the-architect".into(),
            instances: vec![choice(model)],
            task: None,
        });
        tick_until(&mut app, "model launch result", |app| {
            app.world.instances.len() > before || screen(app).contains("Tuple not launchable")
        });
        assert!(
            app.world.instances.len() > before,
            "allowed model {model} did not launch\n{}",
            screen(&mut app)
        );
        let launched = available(&app.world.instances[before]);
        let [recorded] = launched.agents.as_slice() else {
            panic!("one selected tuple expected");
        };
        assert_eq!(recorded.model, model);
        assert_eq!(recorded.account.as_deref(), Some("fixture-model-account"));
        assert_eq!(recorded.provider, Provider::OpenAi);
    }

    let mut app = started(model_world());
    let before = app.world.instances.clone();
    app.go(Go::LaunchMany {
        workspace: None,
        role: "the-architect".into(),
        instances: vec![choice("unknown-model")],
        task: None,
    });
    tick_until(&mut app, "unknown model refusal", |app| {
        screen(app).contains("Tuple not launchable")
    });
    assert_eq!(app.world.instances, before);

    for account in ["fixture-wrong-provider", "fixture-nonexistent"] {
        let mut app = started(model_world());
        let before = app.world.instances.clone();
        app.go(Go::LaunchMany {
            workspace: None,
            role: "the-architect".into(),
            instances: vec![InstanceChoice {
                account: Some(account.into()),
                ..choice("model-b")
            }],
            task: None,
        });
        tick_until(&mut app, "invalid allowed-model account refusal", |app| {
            screen(app).contains("Tuple not launchable")
        });
        assert_eq!(app.world.instances, before, "account {account}");
    }

    let mut app = started(model_world());
    let before = app.world.instances.clone();
    app.go(Go::LaunchMany {
        workspace: None,
        role: "the-architect".into(),
        instances: vec![choice("")],
        task: None,
    });
    tick_until(&mut app, "empty model refusal", |app| {
        screen(app).contains("Tuple not launchable")
    });
    assert_eq!(app.world.instances, before);

    let unrestricted_world = || {
        let mut offer = supported_tuple(Agent::Codex, Provider::OpenAi, "model-a");
        offer.allowed_models.clear();
        tuple_world(
            vec![offer],
            vec![ready_account(
                "fixture-model-account",
                "Model account",
                Provider::OpenAi,
            )],
        )
    };
    for model in ["", "unlisted-model"] {
        let mut app = started(unrestricted_world());
        let before = app.world.instances.len();
        app.go(Go::LaunchMany {
            workspace: None,
            role: "the-architect".into(),
            instances: vec![choice(model)],
            task: None,
        });
        tick_until(&mut app, "unrestricted model result", |app| {
            app.world.instances.len() > before || screen(app).contains("Tuple not launchable")
        });
        assert!(app.world.instances.len() > before, "model {model:?}");
        assert_eq!(
            available(&app.world.instances[before]).agents[0].model,
            model
        );
    }
}

#[test]
fn the_cockpit_aggregates_credentials_for_every_choice_and_selected_provider() {
    let aggregate_world = |workspace: bool| {
        let accounts = vec![
            ready_account("fixture-codex", "Codex selected", Provider::OpenAi),
            ready_account("fixture-codex-extra", "Codex extra", Provider::OpenAi),
            ready_account("fixture-kimi", "Kimi selected", Provider::Moonshot),
            ready_account("fixture-kimi-extra", "Kimi extra", Provider::Moonshot),
            ready_account("fixture-unrelated", "Unrelated account", Provider::Mimo),
        ];
        let tuples = vec![
            supported_tuple(Agent::OpenCode, Provider::OpenCode, "opencode-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "omp-moonshot"),
            supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
            supported_tuple(Agent::KimiCode, Provider::Moonshot, "kimi-default"),
        ];
        let mut world = tuple_world(tuples, accounts);
        remove_provider_accounts(&mut world, Provider::OpenCode);
        world
            .global
            .agent_modes
            .insert(Agent::KimiCode, AuthMode::Sync);
        world.backend = SimBackend::guarded_accountless(TupleIdentity::new(
            Agent::OpenCode,
            Provider::OpenCode,
        ));
        if workspace {
            for id in [
                "fixture-codex",
                "fixture-codex-extra",
                "fixture-kimi",
                "fixture-kimi-extra",
                "fixture-unrelated",
            ] {
                world
                    .accounts
                    .get_mut(id)
                    .expect("workspace account")
                    .default_for_provider = false;
            }
            let policy = &mut world.workspace_mut(1).expect("workspace").accounts;
            policy.disabled_defaults.clear();
            policy.enabled = [
                "fixture-codex".into(),
                "fixture-codex-extra".into(),
                "fixture-kimi".into(),
                "fixture-kimi-extra".into(),
                "fixture-unrelated".into(),
            ]
            .into();
        }
        world
    };
    let choices = vec![
        InstanceChoice {
            agent: Agent::OpenCode,
            provider: Provider::OpenCode,
            account: None,
            model: "opencode-default".into(),
        },
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Moonshot,
            account: Some("fixture-kimi".into()),
            model: "omp-moonshot".into(),
        },
        InstanceChoice {
            agent: Agent::Codex,
            provider: Provider::OpenAi,
            account: Some("fixture-codex".into()),
            model: "codex-default".into(),
        },
        InstanceChoice {
            agent: Agent::KimiCode,
            provider: Provider::Moonshot,
            account: Some("fixture-kimi".into()),
            model: "kimi-default".into(),
        },
    ];

    for (workspace, expected) in [
        (None, vec!["fixture-kimi", "fixture-codex"]),
        (
            Some(1),
            vec![
                "fixture-codex",
                "fixture-codex-extra",
                "fixture-kimi",
                "fixture-kimi-extra",
            ],
        ),
    ] {
        let launch = || {
            let mut app = started(aggregate_world(workspace.is_some()));
            app.go(Go::LaunchMany {
                workspace,
                role: "the-architect".into(),
                instances: choices.clone(),
                task: None,
            });
            app
        };
        let mut summary_app = launch();
        tick_until(&mut summary_app, "credential summary", |app| {
            screen(app).contains("credentials  ")
        });
        let summary = screen_at(&mut summary_app, 240, 40);
        let credential_line = summary
            .lines()
            .find(|line| line.contains("credentials  "))
            .expect("credential summary line");
        assert!(credential_line.contains("Codex selected"), "{summary}");
        assert!(credential_line.contains("Kimi selected"), "{summary}");
        assert!(
            credential_line.contains("API key"),
            "selected account origin missing\n{summary}"
        );
        assert!(
            summary.contains("identity authenticated"),
            "selected account validation missing\n{summary}"
        );
        assert!(!credential_line.contains("host profile"), "{summary}");
        assert!(
            !credential_line.contains("no registered account"),
            "{summary}"
        );
        assert!(
            !credential_line.contains("Unrelated account"),
            "provider-only launch must omit unrelated credentials\n{summary}"
        );

        let mut app = launch();
        let before = app.world.instances.len();
        tick_until(&mut app, "multi-choice instance record", |app| {
            app.world.instances.len() > before
        });
        assert_eq!(available(&app.world.instances[before]).accounts, expected);
        assert_eq!(
            available(&app.world.instances[before])
                .agents
                .iter()
                .map(|choice| choice.account.as_deref())
                .collect::<Vec<_>>(),
            [
                None,
                Some("fixture-kimi"),
                Some("fixture-codex"),
                Some("fixture-kimi"),
            ]
        );
    }
}

#[test]
fn manager_new_session_rejects_empty_account_offers_and_keeps_valid_rows_usable() {
    let tuple = TupleIdentity::new(Agent::Omp, Provider::OpenCode);
    let mut blocked = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::OpenCode,
            "omp-default",
        )],
        vec![ready_account(
            "fixture-omp-blocked",
            "Blocked OMP account",
            Provider::OpenCode,
        )],
    );
    blocked
        .accounts
        .get_mut("fixture-omp-blocked")
        .expect("blocked OMP account")
        .launch_blocked = Some(LaunchBlock::UsageOnlySource);
    blocked.backend = SimBackend::guarded_accountless(tuple);
    let mut app = manager_session_picker(blocked);
    let shown = screen(&mut app);
    assert!(
        shown.contains("OMP"),
        "blocked supported tuple must be shown\n{shown}"
    );
    click_picker_option(&mut app, "New session · choose Agent", "OMP");
    let after_blocked_click = screen(&mut app);
    assert!(
        after_blocked_click.contains("New session · choose Agent"),
        "blocked tuple must remain disabled\n{after_blocked_click}"
    );
    assert!(
        !after_blocked_click.contains("Account for OMP"),
        "an empty account picker must not open\n{after_blocked_click}"
    );

    let mut empty_world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::OpenCode,
            "omp-default",
        )],
        vec![],
    );
    remove_provider_accounts(&mut empty_world, Provider::OpenCode);
    empty_world.backend = SimBackend::guarded_accountless(tuple);
    let mut empty = manager_session_picker(empty_world);
    let shown = screen(&mut empty);
    assert!(
        shown.contains("Shell"),
        "session options remain available\n{shown}"
    );
    assert!(
        !shown.contains("OMP"),
        "accountless launch capability must not enable a new session\n{shown}"
    );

    let valid = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::OpenCode,
            "omp-default",
        )],
        vec![ready_account(
            "fixture-omp-ready",
            "Ready OMP account",
            Provider::OpenCode,
        )],
    );
    let mut app = manager_session_picker(valid);
    click_picker_option(&mut app, "New session · choose Agent", "OMP");
    assert_eq!(
        app.route(),
        Route::Capsule,
        "a valid account-backed session remains usable"
    );

    let alternate_only = tuple_world(
        vec![
            supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
            unavailable_tuple(Agent::Omp, Provider::OpenCode, "omp-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "omp-moonshot"),
        ],
        vec![
            ready_account("fixture-cross-codex", "Codex", Provider::OpenAi),
            ready_account("fixture-cross-omp", "OMP OpenCode", Provider::OpenCode),
        ],
    );
    let mut alternate = manager_session_picker(alternate_only);
    let shown = screen(&mut alternate);
    assert!(shown.contains("OMP"), "default tuple is listed\n{shown}");
    click_picker_option(&mut alternate, "New session · choose Agent", "OMP");
    assert!(screen(&mut alternate).contains("New session · choose Agent"));
    click_picker_option(&mut alternate, "New session · choose Agent", "Codex");
    assert_eq!(alternate.route(), Route::Capsule);
    press(&mut alternate, "ctrl+b c");
    let capsule_shown = screen(&mut alternate);
    assert!(!capsule_shown.contains("OMP"), "{capsule_shown}");

    let mut blocked_capsule_world = tuple_world(
        vec![
            supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "omp-default"),
        ],
        vec![
            ready_account("fixture-capsule-codex", "Codex", Provider::OpenAi),
            ready_account("fixture-capsule-omp", "Blocked OMP", Provider::OpenCode),
        ],
    );
    blocked_capsule_world
        .accounts
        .get_mut("fixture-capsule-omp")
        .expect("blocked Capsule OMP account")
        .launch_blocked = Some(LaunchBlock::UsageOnlySource);
    let mut blocked_capsule = capsule_new_tab_picker(blocked_capsule_world);
    let shown = screen(&mut blocked_capsule);
    assert!(
        shown.contains("OMP"),
        "blocked Capsule offer is visible\n{shown}"
    );
    click_picker_option(&mut blocked_capsule, "New tab", "OMP");
    let after_blocked_click = screen(&mut blocked_capsule);
    assert!(
        after_blocked_click.contains("New tab"),
        "{after_blocked_click}"
    );
    assert!(
        !after_blocked_click.contains("Account for OMP"),
        "blocked Capsule offer must not open an account picker\n{after_blocked_click}"
    );

    let mut empty_capsule_world = tuple_world(
        vec![
            supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "omp-default"),
        ],
        vec![ready_account(
            "fixture-capsule-codex",
            "Codex",
            Provider::OpenAi,
        )],
    );
    remove_provider_accounts(&mut empty_capsule_world, Provider::OpenCode);
    empty_capsule_world.backend = SimBackend::guarded_accountless(tuple);
    let mut empty_capsule = capsule_new_tab_picker(empty_capsule_world);
    let shown = screen(&mut empty_capsule);
    assert!(
        !shown.contains("OMP"),
        "empty accountless Capsule offer\n{shown}"
    );

    let mut valid_capsule = manager_session_picker(tuple_world(
        vec![
            supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "omp-default"),
            unavailable_tuple(Agent::Omp, Provider::Moonshot, "omp-moonshot"),
        ],
        vec![
            ready_account("fixture-capsule-codex", "Codex", Provider::OpenAi),
            ready_account("fixture-capsule-omp", "Ready OMP", Provider::OpenCode),
        ],
    ));
    let before_session = daemon_tab_counts(&valid_capsule.world);
    click_picker_option(&mut valid_capsule, "New session · choose Agent", "Codex");
    assert_eq!(valid_capsule.route(), Route::Capsule);
    let (capsule_instance, before_tabs) =
        newly_active_daemon(&valid_capsule.world, &before_session);
    press(&mut valid_capsule, "ctrl+b c");
    click_picker_option(&mut valid_capsule, "New tab", "OMP");
    let daemon = valid_capsule
        .world
        .daemons
        .get(&capsule_instance)
        .expect("active Capsule daemon");
    assert_eq!(daemon.tabs.len(), before_tabs + 1);
    let pane = daemon
        .pane(daemon.active_tab().expect("active tab").focused)
        .expect("new OMP pane");
    assert_eq!(pane.agent, Some(Agent::Omp));
    assert_eq!(pane.account.as_deref(), Some("fixture-capsule-omp"));
}

fn launch_tuple(world: World, label: &str, expected: InstanceChoice) -> (App, String) {
    let mut app = started(world);
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(picker.contains(label), "{label}\n{picker}");
    click_text(&mut app, label);
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    let cockpit = screen(&mut app);
    let tuple_label = TupleIdentity::new(expected.agent, expected.provider).label();
    assert!(cockpit.contains(&tuple_label), "{tuple_label}\n{cockpit}");
    tick_until(&mut app, "the selected tuple is recorded", |app| {
        app.world
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .any(|instance| {
                instance.agents.iter().any(|choice| {
                    choice.agent == expected.agent && choice.provider == expected.provider
                })
            })
    });
    let launched = app
        .world
        .instances
        .iter()
        .filter_map(InstanceEntry::as_available)
        .find(|instance| {
            instance.agents.iter().any(|choice| {
                choice.agent == expected.agent && choice.provider == expected.provider
            })
        })
        .expect("the selected tuple launched");
    let [actual] = launched.agents.as_slice() else {
        panic!("one chosen tuple expected");
    };
    assert_eq!(actual.agent, expected.agent);
    assert_eq!(actual.provider, expected.provider);
    assert_eq!(actual.account, expected.account);
    assert_eq!(actual.model, expected.model);
    (app, picker)
}

fn blocked_tuple_stays_unlaunchable(world: World, label: &str, reason: &str) {
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(picker.contains(label), "{label}\n{picker}");
    assert!(picker.contains(reason), "{reason}\n{picker}");
    click_text(&mut app, label);
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Manager);
    assert!(screen(&mut app).contains("Launch · choose Agent"));
    assert_eq!(app.world.instances, before);
}

fn click_picker_option(app: &mut App, title: &str, label: &str) {
    let shown = screen(app);
    let lines: Vec<&str> = shown.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.contains(title))
        .unwrap_or_else(|| panic!("{title} not shown:\n{shown}"));
    let (index, line) = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.contains(label))
        .unwrap_or_else(|| panic!("{label} not shown below {title}:\n{shown}"));
    let x = line
        .split_once(label)
        .expect("option label")
        .0
        .chars()
        .count() as u16;
    click(app, x + 1, index as u16);
}

fn two_choice_world() -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.tuples = vec![
        supported_tuple(Agent::Codex, Provider::OpenAi, "codex-model"),
        supported_tuple(Agent::ClaudeCode, Provider::Anthropic, "claude-model"),
    ];
    world.backend = SimBackend::shared();
    world
        .global
        .agent_modes
        .insert(Agent::Codex, AuthMode::Sync);
    world
        .global
        .agent_modes
        .insert(Agent::ClaudeCode, AuthMode::Sync);
    for id in ["acct-codex-primary", "acct-codex-experiments"] {
        let account = world.accounts.get_mut(id).expect("Codex account");
        account.lifecycle = Lifecycle::Available;
        account.default_for_provider = false;
    }
    for id in ["acct-claude-personal", "acct-claude-work"] {
        let account = world.accounts.get_mut(id).expect("Claude account");
        account.lifecycle = Lifecycle::Available;
        account.default_for_provider = false;
    }
    let workspace = world.workspace_mut(1).expect("payments-platform");
    workspace.accounts.disabled_defaults.clear();
    workspace.accounts.enabled = [
        "acct-codex-primary".into(),
        "acct-codex-experiments".into(),
        "acct-claude-personal".into(),
        "acct-claude-work".into(),
    ]
    .into();
    workspace.accounts.preferred.clear();
    workspace.accounts.role_preferred.clear();
    world
}

fn automatic_first_account_world() -> World {
    let mut world = two_choice_world();
    world
        .workspace_mut(1)
        .expect("payments-platform")
        .accounts
        .preferred
        .insert(Provider::OpenAi, "acct-codex-primary".into());
    world
}

fn open_two_choice_later_account_picker(world: World, explicit_first: Option<&str>) -> App {
    let mut app = started(world);
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    click_picker_option(&mut app, "Launch · choose Agent", "Claude Code");
    press(&mut app, "enter");
    if screen(&mut app).contains("Account for Codex") {
        let account = explicit_first.expect("an unresolved first choice needs an explicit account");
        click_picker_option(&mut app, "Account for Codex", account);
    } else {
        assert!(
            explicit_first.is_none(),
            "the first choice was selected automatically"
        );
    }
    assert!(screen(&mut app).contains("Account for Claude Code"));
    app
}

fn snapshot_after(app: &mut App, change: impl FnOnce(&mut Snapshot)) -> Msg {
    let mut snapshot = sim::snapshot(&app.world);
    change(&mut snapshot);
    roster_reply(app, snapshot)
}

fn select_later_claude_account(app: &mut App, label: &str) {
    click_picker_option(app, "Account for Claude Code", label);
}

fn assert_launch_invalidated(app: &mut App, before: &[InstanceEntry]) {
    let shown = screen(app);
    assert!(shown.contains("The launch options changed"), "{shown}");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances.as_slice(), before);
}

/// Opens the launch picker of the workspace `downs` rows below the current
/// directory and launches its first agent.
fn launch(app: &mut App, downs: usize, scope: &str) {
    for _ in 0..downs {
        press(app, "down");
    }
    press(app, "enter");
    let picker = screen(app);
    assert!(
        picker.contains(scope),
        "{scope:?} not in the picker\n{picker}"
    );
    press(app, "space enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(app));
}

#[test]
fn the_sim_refuses_a_role_the_workspace_does_not_allow() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    let others = app.world.instances.clone();
    let workspace = app.world.workspace(PORTAL).expect("customer-portal");
    assert!(!workspace.roles.allows("writer"), "the fixture's premise");

    launch(&mut app, 4, "customer-portal › writer");
    tick_until(&mut app, "the refusal", shows("Role not allowed"));
    let shown = screen(&mut app);
    for text in [
        "Workspace customer-portal does not allow role writer.",
        "nothing was created",
        "Allow it in the workspace's Roles tab, or load another role.",
    ] {
        assert!(shown.contains(text), "{text:?} missing\n{shown}");
    }
    assert_eq!(app.world.instances, others);

    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances, others);
}

/// The refusal and the questions sit over the cockpit of the launch jackin
/// can make: Codex, the seven stages of the OpenShell plan.
#[test]
fn the_checks_sit_over_the_codex_cockpit_of_seven_stages() {
    for (downs, scope, shown_when) in [
        (
            PORTAL as usize,
            "customer-portal › writer",
            "Role not allowed",
        ),
        (
            INFRA as usize,
            "infra-control-plane › sentinel",
            "Fail the preflight hook on purpose",
        ),
    ] {
        let mut app = started(world_for(Scenario::LaunchChecks));
        launch(&mut app, downs, scope);
        tick_until(&mut app, "the check", shows(shown_when));
        let shown = screen(&mut app);
        assert!(shown.contains("Codex · OpenAI"), "{scope}:\n{shown}");
        assert!(shown.contains("0/7 stages"), "{scope}:\n{shown}");
        assert!(
            !shown.contains("Claude Code") && !shown.contains("/11 stages"),
            "{scope}:\n{shown}"
        );
    }
}

#[test]
fn the_sim_launches_a_role_the_workspace_allows_and_asks_nothing() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    launch(&mut app, 1, "payments-platform › the-architect");
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert!(!screen(&mut app).contains("Role not allowed"));
}

#[test]
fn the_launch_picker_accepts_a_supported_accountless_tuple() {
    let mut world = world_for(Scenario::LaunchChecks);
    world.accounts.accounts.clear();
    world.surfaces.tuples = vec![TupleOffer::supported(
        Agent::Omp,
        Provider::OpenCode,
        &["opencode-default"],
    )];
    world.surfaces.agent_blocked = None;
    world.backend =
        SimBackend::guarded_accountless(TupleIdentity::new(Agent::Omp, Provider::OpenCode));
    let mut app = started(world);
    for _ in 0..2 {
        press(&mut app, "down");
    }
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(
        picker.contains("infra-control-plane › sentinel"),
        "{picker}"
    );
    press(&mut app, "space enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));

    tick_until(
        &mut app,
        "the OMP launch questions",
        shows("Fail the preflight hook on purpose"),
    );
    press(&mut app, "tab enter h i enter");
    click_text(&mut app, "Start launch");
    tick_until(&mut app, "the OMP launch record", |app| {
        app.world
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .any(|instance| {
                instance
                    .agents
                    .iter()
                    .any(|agent| agent.agent == Agent::Omp)
            })
    });
    let launched = app
        .world
        .instances
        .iter()
        .filter_map(InstanceEntry::as_available)
        .find(|instance| {
            instance
                .agents
                .iter()
                .any(|agent| agent.agent == Agent::Omp)
        })
        .expect("the tuple launched");
    let [agent] = launched.agents.as_slice() else {
        panic!("one chosen agent expected");
    };
    assert_eq!(agent.provider, Provider::OpenCode);
    assert_eq!(agent.account, None);
    assert_eq!(agent.model, "opencode-default");
}

#[test]
fn a_supported_tuple_and_zero_accounts_do_not_enable_accountless_by_default() {
    let mut world = world_for(Scenario::Returning);
    world.accounts.accounts.clear();
    world.surfaces.tuples = vec![supported_tuple(
        Agent::Omp,
        Provider::OpenCode,
        "opencode-default",
    )];
    world.backend = SimBackend::shared();
    let identity = TupleIdentity::new(Agent::Omp, Provider::OpenCode);
    assert_eq!(
        world.backend.account_requirement(identity),
        AccountRequirement::Required
    );
    let target = LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances: vec![InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::OpenCode,
            account: None,
            model: "opencode-default".into(),
        }],
        task: None,
    };
    let refusal = validate_tuples(&world, &target).expect("credentials are required");
    assert!(format!("{refusal:?}").contains("eligible provider account is required"));
}

#[test]
fn an_accountless_tuple_does_not_bypass_an_explicit_agent_block() {
    let mut world = accountless_tuple_world();
    world.surfaces.agent_blocked = Some(AgentBlocked {
        launchable: Agent::Codex,
        text: "only Codex can launch".into(),
    });
    blocked_tuple_stays_unlaunchable(world, "OMP", "only Codex can launch");
}

#[test]
fn an_accountless_tuple_does_not_bypass_an_ignored_runtime_mode() {
    let mut world = accountless_tuple_world();
    world
        .global
        .agent_modes
        .insert(Agent::Omp, AuthMode::Ignore);
    blocked_tuple_stays_unlaunchable(world, "OMP", "runtime mode");
}

#[test]
fn the_manager_records_credentialed_and_accountless_choices_in_order() {
    let mut world = world_for(Scenario::Returning);
    world.accounts.accounts.clear();
    world.surfaces.tuples = vec![
        supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
        supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
    ];
    world.surfaces.agent_blocked = None;
    world
        .global
        .agent_modes
        .insert(Agent::Codex, AuthMode::Sync);
    world.global.agent_modes.insert(Agent::Omp, AuthMode::Sync);
    world.accounts.insert(ready_account(
        "fixture-codex",
        "Codex account",
        Provider::OpenAi,
    ));
    world.backend =
        SimBackend::guarded_accountless(TupleIdentity::new(Agent::Omp, Provider::OpenCode));

    let mut app = started(world);
    let before = app.world.instances.len();
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    click_picker_option(&mut app, "Launch · choose Agent", "OMP");
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(&mut app, "the new launch instance is recorded", |app| {
        app.world.instances.len() > before
    });

    let instance = app
        .world
        .instances
        .get(before)
        .expect("the two-choice launch record");
    let instance = available(instance);
    let actual: Vec<_> = instance
        .agents
        .iter()
        .map(|choice| {
            (
                choice.agent,
                choice.provider,
                choice.account.clone(),
                choice.model.as_str(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            (
                Agent::Codex,
                Provider::OpenAi,
                Some("fixture-codex".into()),
                "codex-default"
            ),
            (Agent::Omp, Provider::OpenCode, None, "opencode-default"),
        ]
    );
}

#[test]
fn the_picker_routes_omp_kimi_to_its_moonshot_account_with_reversed_offers() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let mimo = ready_account("fixture-mimo", "MiMo account", Provider::Mimo);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
        ],
        vec![kimi, mimo, opencode],
    );
    launch_tuple(
        world,
        "OMP · Moonshot / Kimi",
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Moonshot,
            account: Some("fixture-kimi".into()),
            model: "kimi-default".into(),
        },
    );
}

#[test]
fn the_picker_routes_omp_mimo_to_its_mimo_account_with_reversed_offers() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let mimo = ready_account("fixture-mimo", "MiMo account", Provider::Mimo);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
        ],
        vec![kimi, mimo, opencode],
    );
    launch_tuple(
        world,
        "OMP · Mimo",
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Mimo,
            account: Some("fixture-mimo".into()),
            model: "mimo-default".into(),
        },
    );
}

#[test]
fn the_picker_keeps_omp_opencode_on_the_opencode_account() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let mimo = ready_account("fixture-mimo", "MiMo account", Provider::Mimo);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
        ],
        vec![kimi, mimo, opencode],
    );
    launch_tuple(
        world,
        "OMP · OpenCode",
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::OpenCode,
            account: Some("fixture-opencode".into()),
            model: "opencode-default".into(),
        },
    );
}

fn assert_live_zai_launch(agent: Agent, docker: bool) {
    let id = format!("zai-{}", agent.short());
    let model = format!("zai-{}", agent.short());
    let mut world = live_tuple_world(
        vec![supported_tuple(agent, Provider::Zai, &model)],
        vec![
            ready_account(&id, &format!("ZAI {}", agent.label()), Provider::Zai),
            ready_account("unrelated-openai", "Unrelated OpenAI", Provider::OpenAi),
        ],
    );
    bind_role(&mut world, &[(agent, &id)]);
    if docker {
        world.surfaces.docker = world_for(Scenario::RoleEditor).surfaces.docker;
        world.workspace_mut(1).expect("workspace").docker = true;
    }
    let label = TupleIdentity::new(agent, Provider::Zai).label();
    let (_app, picker) = launch_tuple(
        world,
        &label,
        InstanceChoice {
            agent,
            provider: Provider::Zai,
            account: Some(id.clone()),
            model,
        },
    );
    assert!(picker.contains("Role choice"), "{picker}");
    assert_eq!(picker.contains("Docker is on"), docker, "{picker}");
}

#[test]
fn t09_interactive_launch_uses_the_zai_tuple_and_agent_role_binding() {
    assert_live_zai_launch(Agent::Omp, false);
}

#[test]
fn t14_interactive_launch_uses_the_zai_tuple_and_agent_role_binding() {
    assert_live_zai_launch(Agent::ClaudeCode, false);
}

#[test]
fn t18_interactive_docker_launch_uses_the_zai_tuple_and_agent_role_binding() {
    assert_live_zai_launch(Agent::Codex, true);
}

#[test]
fn ambiguous_zai_accounts_require_and_launch_the_explicit_choice() {
    let mut one = ready_account("zai-one", "ZAI one", Provider::Zai);
    one.default_for_provider = false;
    let mut two = ready_account("zai-two", "ZAI two", Provider::Zai);
    two.default_for_provider = false;
    let mut world = live_tuple_world(
        vec![supported_tuple(Agent::Codex, Provider::Zai, "zai-codex")],
        vec![
            one,
            two,
            ready_account("unrelated-openai", "Unrelated OpenAI", Provider::OpenAi),
        ],
    );
    bind_role(&mut world, &[]);
    let workspace = world.workspace_mut(1).expect("workspace");
    workspace.accounts.enabled = ["zai-one".into(), "zai-two".into()].into();
    workspace.accounts.preferred.clear();
    let recorder = LaunchRecorder::install(&mut world);
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex · Z.AI");
    press(&mut app, "enter");
    let choices = screen(&mut app);
    assert!(choices.contains("Account for Codex"), "{choices}");
    assert!(
        choices.contains("ZAI one") && choices.contains("ZAI two"),
        "{choices}"
    );
    assert!(
        !choices
            .lines()
            .any(|line| line.contains("Unrelated OpenAI") && line.contains("API key ·")),
        "{choices}"
    );
    click_picker_option(&mut app, "Account for Codex", "ZAI two");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    assert!(screen(&mut app).contains("Codex · Z.AI"));
    tick_until(&mut app, "the explicit ZAI account is recorded", |app| {
        app.world.instances.len() > before.len()
    });
    let launched = available(&app.world.instances[before.len()]);
    assert_eq!(launched.agents[0].account.as_deref(), Some("zai-two"));
    assert_eq!(recorder.count(), 1);
}

#[test]
fn an_automatic_t18_role_binding_change_before_confirmation_launches_nothing() {
    let mut second = ready_account("zai-t18-second", "ZAI second", Provider::Zai);
    second.default_for_provider = false;
    let mut world = live_tuple_world(
        vec![supported_tuple(Agent::Codex, Provider::Zai, "zai-codex")],
        vec![
            ready_account("zai-t18-first", "ZAI first", Provider::Zai),
            second,
        ],
    );
    bind_role(&mut world, &[(Agent::Codex, "zai-t18-first")]);
    world
        .workspace_mut(1)
        .expect("workspace")
        .accounts
        .enabled
        .insert("zai-t18-second".into());
    let recorder = LaunchRecorder::install(&mut world);
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex · Z.AI");
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .surfaces
            .roles
            .as_mut()
            .expect("role admin")
            .role_mut("the-architect")
            .expect("launch role")
            .bindings
            .insert(Agent::Codex, "zai-t18-second".into());
    });
    app.deliver(&reload);
    press(&mut app, "enter");
    assert_launch_invalidated(&mut app, &before);
    assert_eq!(recorder.count(), 0);
}

#[test]
fn the_simulator_accepts_matching_credentials_and_refuses_a_mismatched_provider() {
    let world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![
            ready_account("fixture-kimi", "Kimi account", Provider::Moonshot),
            ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode),
        ],
    );
    let target = |account: &str| LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances: vec![InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Moonshot,
            account: Some(account.into()),
            model: "kimi-default".into(),
        }],
        task: None,
    };

    assert_eq!(validate_tuples(&world, &target("fixture-kimi")), None);
    let refusal = validate_tuples(&world, &target("fixture-opencode"))
        .expect("an account from another provider is refused");
    let RefusalReason::TupleNotLaunchable { reason, .. } = refusal else {
        panic!("provider mismatch must be a tuple refusal");
    };
    assert_eq!(
        reason.as_deref(),
        Some("account is not eligible for this provider")
    );
}

#[test]
fn the_picker_refuses_two_provider_choices_for_the_same_omp_agent() {
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
        ],
        vec![
            ready_account("fixture-kimi", "Kimi account", Provider::Moonshot),
            ready_account("fixture-mimo", "MiMo account", Provider::Mimo),
        ],
    );
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "OMP · Moonshot / Kimi");
    click_picker_option(&mut app, "Launch · choose Agent", "OMP · Mimo");
    let conflict = screen(&mut app);
    assert!(
        conflict.contains("Choose only one provider for each agent"),
        "{conflict}"
    );
    assert!(conflict.contains("Launch · choose Agent"));
    press(&mut app, "esc");
    assert_eq!(app.world.instances, before);
}

#[test]
fn the_picker_keeps_unsupported_omp_kimi_disabled_when_open_code_is_supported() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let mut unavailable = supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default");
    unavailable.state = jackin_console::domain::tuple::TupleState::Unavailable;
    unavailable.reason = Some("not offered".into());
    let mut world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
            unavailable,
        ],
        vec![kimi, opencode],
    );
    let recorder = LaunchRecorder::install(&mut world);
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    let picker = screen(&mut app);
    let row = picker
        .lines()
        .find(|line| line.contains("OMP · Moonshot / Kimi"))
        .unwrap_or_else(|| panic!("missing T10 row:\n{picker}"));
    assert!(row.contains("not offered"), "{row}");
    assert!(row.contains("unavailable"), "{row}");
    click_text(&mut app, "OMP · Moonshot / Kimi");
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Manager);
    assert!(screen(&mut app).contains("Launch · choose Agent"));
    assert_eq!(app.world.instances, before);
    assert_eq!(recorder.count(), 0);
    assert!(
        app.world
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .all(|instance| {
                instance
                    .agents
                    .iter()
                    .all(|choice| choice.agent != Agent::Omp)
            })
    );
}

#[test]
fn a_supported_omp_tuple_with_a_blocked_account_stays_disabled() {
    let mut blocked = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    blocked.lifecycle = Lifecycle::NeedsSecret;
    let world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![blocked],
    );
    blocked_tuple_stays_unlaunchable(world, "OMP · Moonshot / Kimi", "needs secret");
}

#[test]
fn a_supported_omp_tuple_with_ignored_runtime_stays_disabled() {
    let mut world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![ready_account(
            "fixture-kimi",
            "Kimi account",
            Provider::Moonshot,
        )],
    );
    world
        .global
        .agent_modes
        .insert(Agent::Omp, AuthMode::Ignore);
    blocked_tuple_stays_unlaunchable(world, "OMP · Moonshot / Kimi", "runtime mode");
}

#[test]
fn a_blocked_preferred_account_is_not_replaced_by_another_ready_account() {
    let mut blocked = ready_account("fixture-kimi-blocked", "Blocked Kimi", Provider::Moonshot);
    blocked.lifecycle = Lifecycle::NeedsSecret;
    let ready = ready_account("fixture-kimi-ready", "Ready Kimi", Provider::Moonshot);
    let mut world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![blocked, ready],
    );
    world
        .workspace_mut(1)
        .expect("payments-platform")
        .accounts
        .preferred
        .insert(Provider::Moonshot, "fixture-kimi-blocked".into());
    blocked_tuple_stays_unlaunchable(world, "OMP · Moonshot / Kimi", "needs secret");
}

#[test]
fn an_account_collected_before_a_later_picker_is_revalidated_before_dispatch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .accounts
            .get_mut("acct-codex-primary")
            .expect("first account")
            .lifecycle = Lifecycle::NeedsSecret;
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_collected_tuple_becoming_unsupported_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        let offer = snapshot
            .surfaces
            .tuples
            .iter_mut()
            .find(|offer| offer.agent == Agent::Codex && offer.provider == Provider::OpenAi)
            .expect("first tuple");
        offer.state = jackin_console::domain::tuple::TupleState::Unavailable;
        offer.reason = Some("Codex tuple became unavailable".into());
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_collected_provider_change_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .surfaces
            .tuples
            .retain(|offer| offer.agent != Agent::Codex || offer.provider != Provider::OpenAi);
        snapshot.surfaces.tuples.push(supported_tuple(
            Agent::Codex,
            Provider::OpenCode,
            "codex-opencode-model",
        ));
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_collected_model_change_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        let offer = snapshot
            .surfaces
            .tuples
            .iter_mut()
            .find(|offer| offer.agent == Agent::Codex && offer.provider == Provider::OpenAi)
            .expect("first tuple");
        offer.default_model = "codex-model-updated".into();
        offer.allowed_models = vec![offer.default_model.clone()];
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_later_account_becoming_unusable_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .accounts
            .get_mut("acct-claude-work")
            .expect("later account")
            .lifecycle = Lifecycle::NeedsSecret;
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn an_automatic_account_binding_change_is_refused_even_when_old_account_stays_ready() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == 1)
            .expect("workspace")
            .accounts
            .preferred
            .insert(Provider::OpenAi, "acct-codex-experiments".into());
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
    assert_eq!(
        app.world
            .accounts
            .get("acct-codex-primary")
            .expect("old automatic account")
            .lifecycle,
        Lifecycle::Available
    );
}

#[test]
fn an_automatic_binding_change_before_confirmation_refuses_the_selected_account() {
    let mut app = started(automatic_first_account_world());
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    let before = app.world.instances.clone();
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == 1)
            .expect("workspace")
            .accounts
            .preferred
            .insert(Provider::OpenAi, "acct-codex-experiments".into());
    });
    app.deliver(&reload);
    press(&mut app, "enter");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn an_unchanged_automatic_binding_launches_the_selected_account() {
    let mut app = started(automatic_first_account_world());
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(&mut app, "the automatic account is recorded", |app| {
        app.world
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .any(|instance| {
                instance.agents.iter().any(|choice| {
                    choice.agent == Agent::Codex
                        && choice.account.as_deref() == Some("acct-codex-primary")
                })
            })
    });
    let instance = app
        .world
        .instances
        .iter()
        .filter_map(InstanceEntry::as_available)
        .find(|instance| {
            instance.agents.iter().any(|choice| {
                choice.agent == Agent::Codex
                    && choice.account.as_deref() == Some("acct-codex-primary")
            })
        })
        .expect("the automatically selected account");
    let [choice] = instance.agents.as_slice() else {
        panic!("one selected tuple expected");
    };
    assert_eq!(choice.provider, Provider::OpenAi);
    assert_eq!(choice.model, "codex-model");
}

#[test]
fn an_explicit_account_remains_selected_after_the_automatic_preference_changes() {
    let mut app = open_two_choice_later_account_picker(two_choice_world(), Some("Primary"));
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == 1)
            .expect("workspace")
            .accounts
            .preferred
            .insert(Provider::OpenAi, "acct-codex-experiments".into());
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(
        &mut app,
        "both explicit and selected tuples launch",
        |app| {
            app.world
                .instances
                .iter()
                .filter_map(InstanceEntry::as_available)
                .any(|instance| {
                    instance.agents.len() == 2
                        && instance.agents.iter().any(|choice| {
                            choice.agent == Agent::Codex
                                && choice.provider == Provider::OpenAi
                                && choice.account.as_deref() == Some("acct-codex-primary")
                        })
                        && instance.agents.iter().any(|choice| {
                            choice.agent == Agent::ClaudeCode
                                && choice.provider == Provider::Anthropic
                                && choice.account.as_deref() == Some("acct-claude-work")
                        })
                })
        },
    );
}

#[test]
fn the_sim_asks_what_the_role_manifest_asks() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    launch(&mut app, INFRA as usize, "infra-control-plane › sentinel");
    tick_until(
        &mut app,
        "the questions",
        shows("Fail the preflight hook on purpose"),
    );
    let shown = screen(&mut app);
    for text in [
        "Note for the run",
        "SENTINEL_FAIL_PREFLIGHT",
        "SENTINEL_NOTE",
    ] {
        assert!(shown.contains(text), "{text:?} missing\n{shown}");
    }
    assert_eq!(app.route(), Route::Cockpit);

    press(&mut app, "tab enter h i enter");
    click_text(&mut app, "Start launch");
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
}

#[test]
fn escape_on_the_sim_questions_records_nothing() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    let others = app.world.instances.clone();
    launch(&mut app, INFRA as usize, "infra-control-plane › sentinel");
    tick_until(
        &mut app,
        "the questions",
        shows("Fail the preflight hook on purpose"),
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(
        app.world.instances, others,
        "nothing exists to record before the questions are answered"
    );
}

#[test]
fn a_backend_that_does_not_guard_neither_asks_nor_refuses() {
    for (downs, scope) in [
        (INFRA as usize, "infra-control-plane › sentinel"),
        (PORTAL as usize, "customer-portal › writer"),
    ] {
        let mut world = world_for(Scenario::LaunchChecks);
        world.backend = SimBackend::shared();
        let mut app = started(world);
        launch(&mut app, downs, scope);
        tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    }
}

fn target(world: &World, workspace: u32, role: &str) -> LaunchTarget {
    let account = world
        .offer_for_provider(
            Agent::Codex,
            Provider::OpenAi,
            world.workspace(workspace),
            Some(role),
        )
        .accounts
        .into_iter()
        .next()
        .expect("the fixture offers an eligible Codex account");
    LaunchTarget {
        workspace: Some(workspace),
        role: role.into(),
        instances: vec![jackin_console::domain::tuple::InstanceChoice {
            agent: Agent::Codex,
            provider: Provider::OpenAi,
            account: Some(account),
            model: "gpt-5-codex".into(),
        }],
        task: None,
    }
}

#[test]
fn the_guarded_sim_sends_the_refusal_before_any_stage() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, PORTAL, "writer"));
    let first = feed.poll(&world);
    assert_eq!(
        first,
        [LaunchEvent::Refused(RefusalReason::RoleNotAllowed {
            workspace: "customer-portal".into(),
            role: "writer".into(),
        })]
    );
    assert_eq!(feed.poll(&world), [], "a refused launch says nothing more");
}

#[test]
fn the_guarded_sim_holds_the_launch_until_the_questions_are_answered() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, INFRA, "sentinel"));
    let events = feed.poll(&world);
    let asked: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LaunchEvent::NeedsInput(q) => Some(q.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        [vec![
            EnvQuestion {
                name: "SENTINEL_FAIL_PREFLIGHT".into(),
                prompt: "Fail the preflight hook on purpose".into(),
                default: Some("0".into()),
            },
            EnvQuestion {
                name: "SENTINEL_NOTE".into(),
                prompt: "Note for the run".into(),
                default: None,
            },
        ]],
        "{events:?}"
    );
    for _ in 0..20 {
        assert_eq!(feed.poll(&world), [], "no stage starts before the answers");
    }

    feed.answer_env(vec![EnvAnswer {
        name: "SENTINEL_NOTE".into(),
        value: "hello".into(),
    }]);
    let resumed = feed.poll(&world);
    assert!(
        resumed
            .iter()
            .any(|e| matches!(e, LaunchEvent::StageChanged(0, _))),
        "{resumed:?}"
    );
}

#[test]
fn cancelling_the_guarded_sim_at_the_questions_keeps_no_record() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, INFRA, "sentinel"));
    feed.poll(&world);
    assert_eq!(feed.cancel(&world), None);
}

#[test]
fn a_role_without_questions_starts_at_once() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, 1, "the-architect"));
    let first = feed.poll(&world);
    assert!(
        first
            .iter()
            .any(|e| matches!(e, LaunchEvent::StageChanged(0, _))),
        "{first:?}"
    );
    assert!(
        !first
            .iter()
            .any(|e| matches!(e, LaunchEvent::NeedsInput(_) | LaunchEvent::Refused(_)))
    );
}
