//! Step BG6: the account form offers an environment-variable source under the
//! openshell surfaces. Each test states one case in terms of what the
//! operator sees or what the backend is asked. The cases are in
//! `docs/P9.3-red-green.md`, section BG6.
#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{
    Backend, CheckRow, Completion, Effect, FileSource, ValidationOutcome,
};
use jackin_console::domain::account::{
    Account, AccountForm, CredentialSource, DetectedKind, IssueCode, Recoverability,
    RecoverableIssue,
};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::{DaemonSnapshot, Instance};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::reason::{Found, IssueReason};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::provider;
use jackin_console::world::{Msg, World};
use termrock::core::event::Input;
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

/// The simulator, with every effect recorded.
#[derive(Debug)]
struct Recording {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
    /// A check gets no answer until the test delivers one.
    hold: bool,
}

impl Recording {
    fn check_ticket(&self) -> u64 {
        self.checks()
            .iter()
            .find_map(|e| match e {
                Effect::CheckSource { ticket, .. } => Some(*ticket),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no check was sent: {:?}", self.seen()))
    }

    fn seen(&self) -> Vec<Effect> {
        let mut all = self.seen.lock().expect("seen").clone();
        all.retain(|e| !matches!(e, Effect::Refresh { .. }));
        all
    }

    fn checks(&self) -> Vec<Effect> {
        self.seen()
            .into_iter()
            .filter(|e| matches!(e, Effect::CheckSource { .. }))
            .collect()
    }

    fn saved(&self) -> Vec<Account> {
        self.seen()
            .iter()
            .filter_map(|e| match e {
                Effect::SaveAccounts { registry, .. } => Some(registry.accounts.clone()),
                _ => None,
            })
            .next_back()
            .unwrap_or_default()
    }
}

impl Backend for Recording {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        if self.hold && matches!(effect, Effect::CheckSource { .. }) {
            return None;
        }
        self.inner.perform(effect, world)
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

fn live_world() -> World {
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = AccountForm::openshell();
    world.surfaces.vocabulary = Vocabulary::openshell();
    world
}

fn recorded(world: World) -> (App, Arc<Recording>) {
    recorded_with(Scenario::AccountsMixed, world, false)
}

fn recorded_with(scenario: Scenario, mut world: World, hold: bool) -> (App, Arc<Recording>) {
    let backend = Arc::new(Recording {
        inner: world.backend.clone(),
        seen: Mutex::new(vec![]),
        hold,
    });
    world.backend = backend.clone();
    let app = app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    (app, backend)
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        Application::handle(app, Input::Tick);
    }
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn word(name: &str) -> String {
    Vocabulary::openshell()
        .fields()
        .into_iter()
        .find(|(field, _)| *field == name)
        .unwrap_or_else(|| panic!("the vocabulary has no word {name}"))
        .1
        .to_owned()
}

fn row_of<'a>(shown: &'a str, text: &str) -> &'a str {
    shown
        .lines()
        .find(|l| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{shown}"))
}

/// The new-account form of the openshell surfaces, with the environment
/// variable source chosen and the cursor in its name field.
fn env_form(app: &mut App) {
    press(app, "c a");
    click_text(app, "Environment variable");
    press(app, "tab");
}

fn type_name(app: &mut App, name: &str) {
    press(app, "enter");
    for c in name.chars() {
        let key = if c == ' ' {
            "space".into()
        } else {
            c.to_string()
        };
        press(app, &key);
    }
    press(app, "tab");
}

fn name_the_account(app: &mut App, name: &str) {
    click_text(app, "Display name");
    press(app, "enter");
    for c in name.chars() {
        press(app, &c.to_string());
    }
    press(app, "tab");
}

#[test]
fn the_openshell_form_offers_an_environment_variable_source() {
    let mut app = app_with_world(
        Scenario::AccountsMixed,
        live_world(),
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "c a");
    let shown = screen(&mut app);
    assert!(
        row_of(&shown, "Environment variable").contains("( ) Environment variable"),
        "{shown}"
    );
    let rows: Vec<&str> = shown.lines().collect();
    let at = |text: &str| rows.iter().position(|r| r.contains(text)).expect(text);
    assert!(
        at("Plain-text API key") < at("Environment variable"),
        "the new source follows the plain-text key:\n{shown}"
    );
}

#[test]
fn the_demo_form_keeps_its_three_sources() {
    let mut app = app(Scenario::AccountsMixed);
    press(&mut app, "c a");
    let shown = screen(&mut app);
    assert!(!shown.contains("Environment variable"), "{shown}");
    for label in [
        "1Password item / field  (recommended)",
        "Local agent folder",
        "Plain-text API key",
    ] {
        assert!(shown.contains(label), "{label}:\n{shown}");
    }
    let form = AccountForm::default();
    assert_eq!(form.sources.len(), 3);
}

#[test]
fn choosing_it_asks_for_a_name_and_hides_the_other_sources() {
    let mut app = app_with_world(
        Scenario::AccountsMixed,
        live_world(),
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "c a");
    let before = screen(&mut app);
    assert!(
        !before.contains(&word("source.env_placeholder")),
        "{before}"
    );
    click_text(&mut app, "Environment variable");
    let shown = screen(&mut app);
    assert!(
        row_of(&shown, "Environment variable").contains("(●) Environment variable"),
        "{shown}"
    );
    assert!(shown.contains(&word("source.env_placeholder")), "{shown}");
    assert!(!shown.contains("Browse…"), "{shown}");
    assert!(!shown.contains("paste the key"), "{shown}");
    assert!(!shown.contains("Choose…"), "{shown}");
}

#[test]
fn validate_asks_the_backend_to_check_the_name_and_sends_no_value() {
    let (mut app, backend) = recorded(live_world());
    env_form(&mut app);
    type_name(&mut app, " ZAI_API_KEY ");
    click_text(&mut app, "Validate");

    let checks = backend.checks();
    let [
        Effect::CheckSource {
            provider,
            source,
            key,
            ..
        },
    ] = checks.as_slice()
    else {
        panic!("one CheckSource expected, got {:?}", backend.seen());
    };
    assert_eq!(*provider, Provider::Anthropic);
    assert_eq!(
        *source,
        CredentialSource::HostEnv {
            var: "ZAI_API_KEY".into(),
            detected: DetectedKind::Unknown,
        }
    );
    assert!(key.is_none(), "a name is checked, never a typed value");
}

#[test]
fn an_empty_name_is_refused_before_the_backend_is_asked() {
    let (mut app, backend) = recorded(live_world());
    env_form(&mut app);
    click_text(&mut app, "Validate");
    let shown = screen(&mut app);
    assert!(shown.contains(&word("source.env_empty")), "{shown}");
    assert!(backend.checks().is_empty(), "{:?}", backend.seen());
    assert!(shown.contains("New account"), "the form stays open");
}

fn checked_row(name: &str) -> String {
    let (mut app, _) = recorded(live_world());
    env_form(&mut app);
    type_name(&mut app, name);
    click_text(&mut app, "Validate");
    ticks(&mut app, 4);
    screen(&mut app)
}

#[test]
fn the_check_words_a_set_missing_empty_and_invalid_variable() {
    let set = checked_row("ZAI_API_KEY");
    assert!(
        set.contains("✓ material   Set in this environment"),
        "a set variable passes:\n{set}"
    );
    let missing = checked_row("NOT_SET_ANYWHERE");
    assert!(
        missing.contains("! material   Not set in this environment"),
        "{missing}"
    );
    let empty = checked_row("JACKIN_EMPTY_KEY");
    assert!(empty.contains("! material   Set but empty"), "{empty}");
}

const TYPED: &str = "sk-ant-secret-1234";

#[test]
fn an_invalid_name_is_worded_without_the_name() {
    let words = Vocabulary::openshell();
    let reason = IssueReason::EnvInvalid { name: TYPED.into() };
    assert!(
        !words.reason(&reason).contains(TYPED),
        "{}",
        words.reason(&reason)
    );
    assert!(
        !words.check(&reason).contains(TYPED),
        "{}",
        words.check(&reason)
    );
    assert_eq!(
        words.reason(&reason),
        "Not a valid variable name · enter the name, not its value"
    );
}

#[test]
fn a_pasted_key_is_not_echoed_by_the_check() {
    let invalid = checked_row(TYPED);
    assert!(
        invalid.contains(&format!(
            "! material   {}",
            word("reason.env_invalid.check")
        )),
        "{invalid}"
    );
    assert_eq!(
        invalid.matches(TYPED).count(),
        1,
        "only the field the operator typed in shows it:\n{invalid}"
    );
}

#[test]
fn saving_stores_the_name_as_a_host_variable_account() {
    let (mut app, backend) = recorded(live_world());
    env_form(&mut app);
    type_name(&mut app, "ANTHROPIC_API_KEY");
    name_the_account(&mut app, "Envy");
    click_text(&mut app, "Save");
    ticks(&mut app, 6);

    assert_eq!(status(&app), "Saved Claude · Envy", "{}", screen(&mut app));
    let saved = backend.saved();
    let account = saved
        .iter()
        .find(|a| a.display_name == "Envy")
        .unwrap_or_else(|| panic!("no account Envy was saved: {:?}", backend.seen()));
    assert_eq!(
        account.source,
        CredentialSource::HostEnv {
            var: "ANTHROPIC_API_KEY".into(),
            detected: DetectedKind::Unknown,
        }
    );
    assert!(
        !backend
            .seen()
            .iter()
            .any(|e| matches!(e, Effect::ProbeFolder { .. })),
        "a variable has no folder to probe"
    );
}

#[test]
fn a_variable_another_account_of_the_provider_reads_is_a_duplicate() {
    let mut world = live_world();
    world.accounts.insert(Account::registered(
        "reg-env",
        "Taken",
        Provider::Anthropic,
        CredentialSource::HostEnv {
            var: "ANTHROPIC_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
    ));
    let (mut app, backend) = recorded(world);
    env_form(&mut app);
    type_name(&mut app, "ANTHROPIC_API_KEY");
    name_the_account(&mut app, "Envy");
    click_text(&mut app, "Save");
    ticks(&mut app, 6);

    let shown = screen(&mut app);
    assert!(
        shown.contains("Already registered: this source is used by"),
        "{shown}"
    );
    assert!(
        backend.saved().iter().all(|a| a.display_name != "Envy"),
        "nothing is stored"
    );
}

#[test]
fn editing_a_host_variable_account_opens_on_its_source_and_name() {
    let mut world = live_world();
    world.accounts.insert(Account::registered(
        "reg-env",
        "Taken",
        Provider::Anthropic,
        CredentialSource::HostEnv {
            var: "ANTHROPIC_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
    ));
    let (mut app, _) = recorded(world);
    press(&mut app, "c");
    click_text(&mut app, "Taken");
    press(&mut app, "e");
    let shown = screen(&mut app);
    assert!(shown.contains("Edit account"), "{shown}");
    assert!(
        row_of(&shown, "Environment variable").contains("(●) Environment variable"),
        "{shown}"
    );
    assert!(shown.contains("ANTHROPIC_API_KEY"), "{shown}");
}

fn roster_card(vocabulary: Vocabulary, daemon: DaemonSnapshot) -> String {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary = vocabulary;
    let id = world.running_entries()[0].id().clone();
    world.daemons.remove(&id);
    world.instance_mut(&id).expect("instance").daemon = daemon;
    let mut app = app_with(Scenario::Returning, world);
    screen_at(&mut app, 170, 40)
}

#[test]
fn a_running_sandbox_without_tabs_is_not_called_unreachable() {
    let live = roster_card(Vocabulary::openshell(), DaemonSnapshot::NoTabs);
    assert!(live.contains("· no Capsule tabs open"), "{live}");
    assert!(!live.contains("· gateway unreachable"), "{live}");
}

#[test]
fn an_unreachable_gateway_still_reads_unreachable_on_the_card() {
    let live = roster_card(Vocabulary::openshell(), DaemonSnapshot::Unavailable);
    assert!(live.contains("· gateway unreachable"), "{live}");
    assert!(!live.contains("no Capsule tabs open"), "{live}");
}

#[test]
fn the_demo_card_reads_as_it_did_for_a_terminal_without_tabs() {
    let demo = roster_card(Vocabulary::demo(), DaemonSnapshot::NoTabs);
    assert!(demo.contains("· daemon unavailable"), "{demo}");
}

#[test]
fn the_environment_variable_source_says_it_is_usage_only() {
    let mut app = app_with_world(
        Scenario::AccountsMixed,
        live_world(),
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "c a");
    let shown = screen(&mut app);
    assert!(
        row_of(&shown, "Environment variable").contains("( ) Environment variable  (usage only)"),
        "the radio row is a usage account like the 1Password row:\n{shown}"
    );
    click_text(&mut app, "Environment variable");
    let chosen = screen(&mut app);
    assert!(
        chosen.contains("Environment variable *"),
        "the field label stays plain:\n{chosen}"
    );
    assert!(
        !chosen.contains("Environment variable *  (usage only)"),
        "{chosen}"
    );
}

const SECRET: &str = "sk_ant_secret_1234";

/// What the simulator answers for a set variable, with the one row and the
/// issue of the case under test swapped in.
fn env_outcome(app: &App, case: &str) -> ValidationOutcome {
    let mut outcome = provider::validate(
        Provider::Anthropic,
        &CredentialSource::HostEnv {
            var: "ZAI_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
        None,
        app.world.op.as_ref(),
        app.world.now_secs(),
    );
    let name = SECRET.to_owned();
    let failed = match case {
        "missing" => Some(IssueReason::EnvMissing { name }),
        "empty" => Some(IssueReason::EnvEmpty { name }),
        _ => {
            outcome.material = CheckRow::Ok(Found::EnvSet { name });
            None
        }
    };
    if let Some(reason) = failed {
        outcome.level = None;
        outcome.issue = Some(RecoverableIssue::new(
            IssueCode::ApiKeyEmpty,
            reason.clone(),
            Recoverability::ActionRequired,
        ));
        outcome.material = CheckRow::Failed(reason);
    }
    outcome
}

#[test]
fn a_name_shaped_secret_is_never_printed_by_the_check() {
    for case in ["missing", "empty", "set"] {
        let (mut app, backend) = recorded_with(Scenario::AccountsMixed, live_world(), true);
        env_form(&mut app);
        type_name(&mut app, SECRET);
        click_text(&mut app, "Validate");
        app.deliver(&Msg::SourceChecked {
            ticket: backend.check_ticket(),
            result: Ok(Box::new(env_outcome(&app, case))),
        });
        let shown = screen(&mut app);
        assert!(
            shown.contains("material"),
            "{case}: the reply shows:\n{shown}"
        );
        assert!(!shown.contains("checking"), "{case}:\n{shown}");
        assert_eq!(
            shown.matches(SECRET).count(),
            1,
            "{case}: only the field the operator typed in shows it:\n{shown}"
        );
    }
    let words = Vocabulary::openshell();
    for reason in [
        IssueReason::EnvMissing {
            name: SECRET.into(),
        },
        IssueReason::EnvEmpty {
            name: SECRET.into(),
        },
    ] {
        assert!(!words.reason(&reason).contains(SECRET));
        assert!(!words.check(&reason).contains(SECRET));
    }
    assert!(
        !words
            .found(&Found::EnvSet {
                name: SECRET.into()
            })
            .contains(SECRET)
    );
}

/// The 1Password source with a reference chosen in the picker: the form with
/// the most rows, a note about the chosen reference included.
fn reference_form(app: &mut App) {
    press(app, "c a");
    click_text(app, "1Password item");
    click_text(app, "Choose");
    for _ in 0..4 {
        ticks(app, 12);
        press(app, "enter");
    }
    ticks(app, 12);
}

#[test]
fn a_pending_check_brings_its_rows_into_view() {
    let (mut app, backend) = recorded_with(
        Scenario::OpenshellFixed,
        world_for(Scenario::OpenshellFixed),
        true,
    );
    reference_form(&mut app);
    let before = screen_at(&mut app, 120, 40);
    assert!(
        !before.contains("Validating") && before.contains("Validate"),
        "{before}"
    );
    click_text(&mut app, "Validate");
    let pending = screen_at(&mut app, 120, 40);
    for label in ["material", "identity", "quota"] {
        assert!(
            pending.contains(&format!("… {label}")),
            "the {label} row is below the fold:\n{pending}"
        );
    }

    let outcome = env_outcome(&app, "missing");
    app.deliver(&Msg::SourceChecked {
        ticket: backend.check_ticket(),
        result: Ok(Box::new(outcome)),
    });
    let done = screen_at(&mut app, 120, 40);
    assert!(!done.contains("checking"), "{done}");
    for label in ["material", "identity", "quota"] {
        assert!(
            done.lines()
                .any(|l| l.contains(label)
                    && (l.contains('✓') || l.contains('!') || l.contains('—'))),
            "the {label} row is below the fold after the reply:\n{done}"
        );
    }
    assert!(
        done.contains(&word("reason.env_missing.issue")),
        "the issue line under the rows is below the fold:\n{done}"
    );
}

/// The card sits in the roster column, which the manager draws from 152
/// columns on; at 120 columns the detail panel lists the instance without its
/// live line, so there is no card to cut. At 152 the line is cut after
/// `no Capsule tabs`, so the point of the card is still there.
#[test]
fn the_no_tabs_card_keeps_its_point_at_the_narrowest_width_that_draws_it() {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary = Vocabulary::openshell();
    let id = world.running_entries()[0].id().clone();
    world.daemons.remove(&id);
    world.instance_mut(&id).expect("instance").daemon = DaemonSnapshot::NoTabs;
    let mut app = app_with(Scenario::Returning, world);
    let narrow = screen_at(&mut app, 152, 40);
    assert!(narrow.contains("· no Capsule tabs"), "{narrow}");
    assert!(!narrow.contains("gateway unreachable"), "{narrow}");
    let at_120 = screen_at(&mut app, 120, 40);
    assert!(!at_120.contains("gateway unreachable"), "{at_120}");
}
