#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{
    Backend, ChangeSet, ChangedFile, CheckRow, Completion, DiffLine, DiffStatus, Effect,
    FileSource, FolderProbe, Hunk, OpAccountRow, OpAccountState, OpError, OpListing, OpScope,
    Refresh, ValidationOutcome,
};
use jackin_console::domain::account::{
    Account, AccountIdentity, AccountOrigin, CredentialSource, DetectedKind, IssueCode,
    Recoverability, RecoverableIssue, ValidationLevel, ValidationState,
};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::reason::{Found, IssueReason, RefreshOutcome};
use jackin_console::domain::usage::{Freshness, FreshnessInfo};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::screens::Go;
use jackin_console::sim::{SimBackend, provider};
use jackin_console::world::{Absorbed, Msg, Snapshot, World, job};
use termrock::core::event::{Input, Outcome};
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

/// The simulator, except that the four requests this file is about get no
/// answer until the test delivers one, the way a slow provider, a slow disk
/// or a slow `op` would leave them.
#[derive(Debug)]
struct Held {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
    hold_saves: bool,
}

impl Held {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
            hold_saves: false,
        })
    }

    fn holding_saves() -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
            hold_saves: true,
        })
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

    fn refreshes(&self) -> Vec<Effect> {
        self.seen()
            .into_iter()
            .filter(|e| matches!(e, Effect::RefreshAccount { .. }))
            .collect()
    }

    fn saves(&self) -> usize {
        self.seen()
            .iter()
            .filter(|e| matches!(e, Effect::SaveAccounts { .. }))
            .count()
    }

    fn last_ticket(&self, want: impl Fn(&Effect) -> Option<u64>) -> u64 {
        self.seen().iter().rev().find_map(want).unwrap_or_else(|| {
            panic!(
                "the request was not sent; the backend saw {:?}",
                self.seen()
            )
        })
    }

    fn check_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::CheckSource { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn probe_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::ProbeFolder { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn read_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::ReadChanges { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn op_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::LoadOp { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn op_scopes(&self) -> Vec<OpScope> {
        self.seen()
            .iter()
            .filter_map(|e| match e {
                Effect::LoadOp { scope, .. } => Some(scope.clone()),
                _ => None,
            })
            .collect()
    }
}

impl Backend for Held {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        match effect {
            Effect::CheckSource { .. }
            | Effect::ProbeFolder { .. }
            | Effect::ReadChanges { .. }
            | Effect::LoadOp { .. }
            | Effect::RefreshAccount { .. }
            | Effect::ValidateAccount { .. } => None,
            Effect::SaveAccounts { .. } if self.hold_saves => None,
            _ => self.inner.perform(effect, world),
        }
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

fn held(scenario: Scenario) -> (App, Arc<Held>) {
    let backend = Held::new();
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
}

fn held_saves(scenario: Scenario) -> (App, Arc<Held>) {
    let backend = Held::holding_saves();
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
}

fn timed(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        Application::handle(app, Input::Tick);
    }
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

/// "Spare", provider Claude, the plain-text source and a key the simulator
/// takes as valid. The cursor ends on the key field's tail.
const PLAIN_KEY: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t - v a l i d - a b c d e f 1 2 3 4 tab";

const SPARE_CANARY: &str = "reply-from-the-backend";

/// What the simulator would answer for the typed key, with one row changed so
/// that a form showing it can only have taken it from the reply.
fn key_outcome(app: &App) -> ValidationOutcome {
    let mut outcome = provider::validate(
        Provider::Anthropic,
        &CredentialSource::PlainApiKey {
            fingerprint: "a605d51c".into(),
            tail: "1234".into(),
        },
        Some("sk-ant-valid-abcdef1234"),
        app.world.op.as_ref(),
        app.world.now_secs(),
    );
    outcome.material = CheckRow::Ok(Found::FileFound {
        path: SPARE_CANARY.into(),
    });
    outcome
}

fn checked(ticket: u64, outcome: ValidationOutcome) -> Msg {
    Msg::SourceChecked {
        ticket,
        result: Ok(Box::new(outcome)),
    }
}

#[test]
fn validate_leaves_the_form_pending_until_the_backend_answers() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
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
    assert!(matches!(source, CredentialSource::PlainApiKey { tail, .. } if tail == "1234"));
    assert!(key.is_some());

    let pending = screen(&mut app);
    for label in ["material", "identity", "quota"] {
        assert!(
            pending.contains(&format!("… {label}")) && pending.contains("checking"),
            "the {label} row waits:\n{pending}"
        );
    }
    assert!(!pending.contains("✓ material"), "{pending}");
    assert_eq!(status(&app), "Validating the source…");
    assert!(pending.contains("New account"), "the form stays open");

    click_text(&mut app, "Validate");
    assert_eq!(
        backend.checks().len(),
        1,
        "a pending check is not asked twice"
    );

    let ticket = backend.check_ticket();
    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let done = screen(&mut app);
    assert!(
        done.contains(&format!("✓ material   File found at {SPARE_CANARY}")),
        "{done}"
    );
    assert!(!done.contains("checking"), "{done}");
    assert_eq!(
        status(&app),
        "Validated · Save stores the reference, never the secret"
    );
}

#[test]
fn a_failed_check_shows_the_reply_error_in_the_form() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();
    app.deliver(&Msg::SourceChecked {
        ticket,
        result: Err("the provider did not answer".into()),
    });

    let shown = screen(&mut app);
    assert!(shown.contains("the provider did not answer"), "{shown}");
    assert!(!shown.contains("checking"), "{shown}");
    assert!(shown.contains("New account"), "{shown}");
    click_text(&mut app, "Validate");
    assert_eq!(
        backend.checks().len(),
        2,
        "the button works again after a failed check"
    );
}

#[test]
fn editing_the_form_drops_the_pending_check() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();

    click_text(&mut app, "Local agent folder");
    let edited = screen(&mut app);
    assert!(!edited.contains("checking"), "{edited}");

    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let late = screen(&mut app);
    assert!(
        !late.contains(SPARE_CANARY),
        "a reply to a check the form gave up is not shown:\n{late}"
    );
}

#[test]
fn the_typed_key_never_reaches_a_debug_print() {
    const KEY: &str = "sk-ant-valid-abcdef1234";
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();

    let effects = backend.checks();
    let [
        Effect::CheckSource {
            key: Some(sent), ..
        },
    ] = effects.as_slice()
    else {
        panic!("one check with the typed key expected, got {effects:?}");
    };
    assert_eq!(sent.expose(), KEY, "the backend gets the key itself");
    for printed in [format!("{effects:?}"), format!("{effects:#?}")] {
        assert!(
            !printed.contains(KEY),
            "the effect prints the key: {printed}"
        );
        assert!(!printed.contains("abcdef"), "{printed}");
    }

    let reply = checked(ticket, key_outcome(&app));
    for printed in [format!("{reply:?}"), format!("{reply:#?}")] {
        assert!(
            !printed.contains(KEY),
            "the reply prints the key: {printed}"
        );
    }
}

#[test]
fn a_reply_to_an_earlier_check_is_not_the_forms() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let first = backend.check_ticket();
    click_text(&mut app, "Local agent folder");
    click_text(&mut app, "Plain-text API key");
    press(&mut app, "tab tab tab tab tab");
    click_text(&mut app, "Validate");
    let second = backend.check_ticket();
    assert_ne!(first, second);

    let outcome = key_outcome(&app);
    app.deliver(&checked(first, outcome.clone()));
    let waiting = screen(&mut app);
    assert!(
        waiting.contains("… material   checking") && !waiting.contains(SPARE_CANARY),
        "the first reply does not end the second check:\n{waiting}"
    );

    app.deliver(&checked(second, outcome));
    assert!(screen(&mut app).contains(SPARE_CANARY));
}

#[test]
fn a_reply_after_the_form_closed_does_nothing() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();
    press(&mut app, "esc");

    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    press(&mut app, "a");
    let fresh = screen(&mut app);
    assert!(fresh.contains("New account"), "{fresh}");
    assert!(
        !fresh.contains(SPARE_CANARY) && !fresh.contains("checking"),
        "a new form starts clean:\n{fresh}"
    );
    assert_eq!(backend.saves(), 0);
}

#[test]
fn save_waits_for_the_check_and_then_saves() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Save");

    assert_eq!(backend.checks().len(), 1);
    assert_eq!(backend.saves(), 0, "nothing is stored before the check");
    let waiting = screen(&mut app);
    assert!(waiting.contains("New account"), "{waiting}");
    assert!(waiting.contains("checking"), "{waiting}");

    let ticket = backend.check_ticket();
    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    ticks(&mut app, 4);
    assert_eq!(backend.saves(), 1);
    assert_eq!(status(&app), "Saved Claude · Spare");
    assert!(!screen(&mut app).contains("New account"));
}

#[test]
fn save_during_a_validate_saves_when_the_check_returns() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    click_text(&mut app, "Save");
    assert_eq!(backend.checks().len(), 1, "one check serves both");
    assert_eq!(backend.saves(), 0);

    let ticket = backend.check_ticket();
    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    ticks(&mut app, 4);
    assert_eq!(backend.saves(), 1);
    assert_eq!(status(&app), "Saved Claude · Spare");
}

#[test]
fn a_rejected_check_keeps_the_form_open_and_stores_nothing() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();
    let mut rejected = key_outcome(&app);
    rejected.level = None;
    rejected.issue = Some(RecoverableIssue::new(
        IssueCode::ApiKeyEmpty,
        IssueReason::KeyEmpty,
        Recoverability::ActionRequired,
    ));
    rejected.material = CheckRow::Failed(IssueReason::KeyEmpty);
    app.deliver(&checked(ticket, rejected));
    ticks(&mut app, 4);

    let shown = screen(&mut app);
    assert!(shown.contains("New account"), "{shown}");
    assert!(!shown.contains("checking"), "{shown}");
    assert!(shown.contains("! material   Field is empty"), "{shown}");
    assert!(
        shown.contains("API key required: the field is empty"),
        "{shown}"
    );
    assert_eq!(backend.saves(), 0);
}

/// "Spare", provider Claude, the local folder source and `~/.claude-spare`.
const FOLDER: &str =
    "c a enter S p a r e tab tab tab down tab enter ~ / . c l a u d e - s p a r e tab";

fn folder_outcome(app: &App) -> ValidationOutcome {
    let mut outcome = provider::validate(
        Provider::Anthropic,
        &CredentialSource::LocalFolder {
            path: "~/.claude-work".into(),
            detected: DetectedKind::Unknown,
        },
        None,
        app.world.op.as_ref(),
        app.world.now_secs(),
    );
    assert!(outcome.level.is_some(), "the simulator accepts this folder");
    outcome.identity = AccountIdentity::default();
    outcome
}

#[test]
fn saving_a_folder_asks_the_backend_what_is_in_it() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");

    let ticket = backend.check_ticket();
    assert!(
        backend
            .seen()
            .iter()
            .all(|e| !matches!(e, Effect::ProbeFolder { .. })),
        "the folder is probed after the check, not with it"
    );
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));

    let probes: Vec<_> = backend
        .seen()
        .into_iter()
        .filter(|e| matches!(e, Effect::ProbeFolder { .. }))
        .collect();
    let [Effect::ProbeFolder { path, .. }] = probes.as_slice() else {
        panic!("one ProbeFolder expected, got {:?}", backend.seen());
    };
    assert_eq!(path, "~/.claude-spare");
    assert_eq!(backend.saves(), 0, "nothing is stored before the probe");
    assert!(screen(&mut app).contains("New account"));

    let ticket = backend.probe_ticket();
    app.deliver(&Msg::FolderProbed {
        ticket,
        result: Ok(FolderProbe::Found(DetectedKind::CodexAuthJson)),
    });
    ticks(&mut app, 4);

    let Some(Effect::SaveAccounts { registry, .. }) = backend
        .seen()
        .into_iter()
        .find(|e| matches!(e, Effect::SaveAccounts { .. }))
    else {
        panic!("the account is stored once the folder is probed");
    };
    let spare = registry
        .accounts
        .iter()
        .find(|a| a.display_name == "Spare")
        .expect("the new account");
    assert!(
        matches!(
            &spare.source,
            CredentialSource::LocalFolder { path, detected }
                if path == "~/.claude-spare" && *detected == DetectedKind::CodexAuthJson
        ),
        "the kind comes from the probe reply: {:?}",
        spare.source
    );
}

#[test]
fn a_folder_probe_that_fails_keeps_the_form_open() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let ticket = backend.probe_ticket();
    app.deliver(&Msg::FolderProbed {
        ticket,
        result: Err("the disk is not answering".into()),
    });

    let shown = screen(&mut app);
    assert!(shown.contains("the disk is not answering"), "{shown}");
    assert!(shown.contains("New account"), "{shown}");
    assert_eq!(backend.saves(), 0);
}

#[test]
fn a_probe_reply_for_a_path_the_user_has_since_edited_saves_nothing() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let probe = backend.probe_ticket();

    click_text(&mut app, "~/.claude-spare");
    press(&mut app, "x y z");
    assert!(screen(&mut app).contains("xyz"), "the field shows the edit");

    app.deliver(&Msg::FolderProbed {
        ticket: probe,
        result: Ok(FolderProbe::Found(DetectedKind::CodexAuthJson)),
    });
    ticks(&mut app, 4);

    assert_eq!(backend.saves(), 0, "the old path is not stored");
    let shown = screen(&mut app);
    assert!(
        shown.contains("New account"),
        "the form stays open:\n{shown}"
    );
    assert!(shown.contains("xyz"), "the typed text is kept:\n{shown}");
    assert!(!shown.contains("checking"), "{shown}");
}

#[test]
fn a_check_reply_for_a_key_the_user_has_since_edited_is_not_shown() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();

    click_text(&mut app, "1234");
    press(&mut app, "x y z");

    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let shown = screen(&mut app);
    assert!(
        !shown.contains(SPARE_CANARY),
        "the rows describe the old key:\n{shown}"
    );
    assert!(!shown.contains("checking"), "{shown}");
    assert!(shown.contains("New account"), "{shown}");
}

#[test]
fn a_save_whose_check_reply_finds_the_form_edited_stores_nothing() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();

    click_text(&mut app, "~/.claude-spare");
    press(&mut app, "x y z");
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    ticks(&mut app, 4);

    assert!(
        backend
            .seen()
            .iter()
            .all(|e| !matches!(e, Effect::ProbeFolder { .. })),
        "a reply for the old path starts no probe"
    );
    assert_eq!(backend.saves(), 0);
    assert!(screen(&mut app).contains("xyz"));
}

#[test]
fn browsing_for_a_folder_drops_the_pending_check() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();

    click_text(&mut app, "Browse…");
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    ticks(&mut app, 4);

    assert!(
        backend
            .seen()
            .iter()
            .all(|e| !matches!(e, Effect::ProbeFolder { .. })),
        "the chooser buttons drop the pending check"
    );
    assert_eq!(backend.saves(), 0);
}

fn choose_a_reference(app: &mut App) {
    open_picker(app);
    until_loaded(app);
    press(app, "enter");
    until_loaded(app);
    press(app, "enter");
    until_loaded(app);
    press(app, "W o r k");
    press(app, "enter");
    until_loaded(app);
    press(app, "enter");
    assert!(screen(app).contains("Anthropic · Work"));
}

#[test]
fn the_reference_note_follows_the_vocabulary() {
    let mut demo = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    choose_a_reference(&mut demo);
    assert!(
        screen(&mut demo).contains("only the reference is saved · the value is resolved at launch")
    );

    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.vocabulary = Vocabulary::openshell();
    let mut live = timed(Scenario::AccountsMixed, world);
    choose_a_reference(&mut live);
    let shown = screen(&mut live);
    assert!(
        shown.contains("only the reference is saved · read here for usage only"),
        "{shown}"
    );
    assert!(!shown.contains("resolved at launch"), "{shown}");
}

#[test]
fn choosing_a_reference_drops_the_pending_check() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    choose_a_reference(&mut app);
    click_text(&mut app, "Validate");
    assert!(screen(&mut app).contains("… material   checking"));

    click_text(&mut app, "Choose…");
    ticks(&mut app, 14);

    let said = status(&app);
    assert!(
        !said.starts_with("Validat"),
        "the reply to a dropped check says nothing: {said}"
    );
}

#[test]
fn entering_plain_text_drops_the_pending_check() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    choose_a_reference(&mut app);
    click_text(&mut app, "Validate");
    assert!(screen(&mut app).contains("… material   checking"));

    click_text(&mut app, "Enter plain text instead");
    let shown = screen(&mut app);
    assert!(
        !shown.contains("checking"),
        "the pending rows are gone:\n{shown}"
    );
}

fn view_changes(app: &mut App) {
    press(app, "f10");
    click_text(app, "View");
    click_text(app, "Inspect changes");
}

fn canary_changes() -> ChangeSet {
    ChangeSet {
        files: vec![ChangedFile {
            path: "src/canary.rs".into(),
            status: DiffStatus::Modified,
            hunks: vec![Hunk {
                old_start: 1,
                new_start: 1,
                lines: vec![DiffLine::add("let canary = 1;")],
            }],
        }],
        unpushed: 0,
        omitted: 0,
        unread: vec![],
    }
}

#[test]
fn inspect_changes_waits_for_the_backend() {
    let (mut app, backend) = held(Scenario::CapsuleMulti);
    view_changes(&mut app);

    let reads: Vec<_> = backend
        .seen()
        .into_iter()
        .filter(|e| matches!(e, Effect::ReadChanges { .. }))
        .collect();
    let [Effect::ReadChanges { instance, .. }] = reads.as_slice() else {
        panic!("one ReadChanges expected, got {:?}", backend.seen());
    };
    assert!(instance.sandbox_name.starts_with("jk-"), "{instance}");
    assert_eq!(status(&app), "Reading changes…");
    assert!(!screen(&mut app).contains("Inspect changes ·"));

    view_changes(&mut app);
    let asked = backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::ReadChanges { .. }))
        .count();
    assert_eq!(asked, 1, "a pending read is not asked twice");

    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Ok(canary_changes()),
    });
    let shown = screen(&mut app);
    assert!(shown.contains("Inspect changes ·"), "{shown}");
    assert!(shown.contains("canary.rs"), "{shown}");
}

#[test]
fn an_empty_changes_reply_says_nothing_changed() {
    let (mut app, backend) = held(Scenario::CapsuleMulti);
    view_changes(&mut app);
    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Ok(ChangeSet {
            files: vec![],
            unpushed: 0,
            omitted: 0,
            unread: vec![],
        }),
    });
    assert_eq!(status(&app), "Nothing changed in this instance");
    assert!(!screen(&mut app).contains("Inspect changes ·"));
}

#[test]
fn a_failed_changes_read_shows_the_reply_error() {
    let (mut app, backend) = held(Scenario::CapsuleMulti);
    view_changes(&mut app);
    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Err("git is not answering".into()),
    });
    assert_eq!(status(&app), "git is not answering");
    assert!(!screen(&mut app).contains("Inspect changes ·"));
    view_changes(&mut app);
    assert_eq!(
        backend
            .seen()
            .iter()
            .filter(|e| matches!(e, Effect::ReadChanges { .. }))
            .count(),
        2,
        "the next try asks again"
    );
}

fn open_picker(app: &mut App) {
    press(app, "a");
    click_text(app, "Choose…");
}

fn accounts(rows: &[(&str, &str, OpAccountState)]) -> OpListing {
    OpListing::Accounts(
        rows.iter()
            .map(|(id, email, state)| OpAccountRow {
                id: (*id).into(),
                email: (*email).into(),
                state: *state,
            })
            .collect(),
    )
}

#[test]
fn the_picker_loads_through_the_backend() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);

    assert_eq!(backend.op_scopes(), [OpScope::Accounts]);
    ticks(&mut app, 12);
    let waiting = screen(&mut app);
    assert!(
        waiting.contains("loading accounts…"),
        "the picker waits for the reply, not for the clock:\n{waiting}"
    );

    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Ok(accounts(&[(
            "canary.1password.com",
            "canary@example.test",
            OpAccountState::Available,
        )])),
    });
    let listed = screen(&mut app);
    assert!(listed.contains("canary.1password.com"), "{listed}");
    assert!(!listed.contains("loading accounts"), "{listed}");
    assert!(!listed.contains("chainargos"), "{listed}");
}

#[test]
fn choosing_an_account_asks_for_its_vaults() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);
    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Ok(accounts(&[(
            "canary.1password.com",
            "canary@example.test",
            OpAccountState::Available,
        )])),
    });
    press(&mut app, "enter");

    assert_eq!(
        backend.op_scopes(),
        [
            OpScope::Accounts,
            OpScope::Vaults {
                account: "canary.1password.com".into()
            }
        ]
    );
    let waiting = screen(&mut app);
    assert!(waiting.contains("loading vaults…"), "{waiting}");
    press(&mut app, "down");
    assert_eq!(
        backend.op_scopes().len(),
        2,
        "keys do not start another load while one is out"
    );
}

#[test]
fn a_locked_listing_offers_a_retry_that_asks_again() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);
    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Err(OpError::Locked),
    });
    let locked = screen(&mut app);
    assert!(locked.contains("1Password is locked"), "{locked}");

    press(&mut app, "r");
    assert_eq!(backend.op_scopes(), [OpScope::Accounts, OpScope::Accounts]);
    assert!(screen(&mut app).contains("loading accounts…"));
}

#[test]
fn a_listing_the_picker_has_left_behind_is_not_shown() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);
    let first = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket: first,
        result: Ok(accounts(&[(
            "canary.1password.com",
            "canary@example.test",
            OpAccountState::Available,
        )])),
    });
    press(&mut app, "enter");
    let vaults = backend.op_ticket();
    press(&mut app, "esc");
    let accounts_again = backend.op_ticket();
    assert_ne!(vaults, accounts_again);

    app.deliver(&Msg::OpLoaded {
        ticket: vaults,
        result: Ok(OpListing::Vaults(vec![])),
    });
    let shown = screen(&mut app);
    assert!(
        shown.contains("loading accounts…"),
        "the old reply must not end the new load:\n{shown}"
    );
}

#[test]
fn the_sim_still_lists_the_picker_after_260_ms() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    open_picker(&mut app);
    let start = app.world.clock.now_ms;
    let mut elapsed = 0;
    while screen(&mut app).contains("loading accounts…") && elapsed < 2_000 {
        ticks(&mut app, 1);
        elapsed = app.world.clock.now_ms - start;
    }
    assert_eq!(
        elapsed, 320,
        "260 ms of latency, delivered on the tick after"
    );
    let listed = screen(&mut app);
    assert!(listed.contains("chainargos.1password.com"), "{listed}");
    assert!(listed.contains("acme.1password.com"), "{listed}");
}

#[test]
fn the_sim_checks_a_typed_key_on_the_spot() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let done = screen(&mut app);
    assert!(done.contains("✓ material   Key material present"), "{done}");
    assert!(!done.contains("checking"), "{done}");
    assert_eq!(
        status(&app),
        "Validated · Save stores the reference, never the secret"
    );
}

#[test]
fn the_sim_saves_a_folder_on_the_spot_with_the_kind_it_probes() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    press(
        &mut app,
        "c a enter S p a r e tab tab tab down tab enter ~ / . c l a u d e - w o r k tab",
    );
    click_text(&mut app, "Save");
    assert_eq!(status(&app), "Saved Claude · Spare");
    let spare = app
        .world
        .accounts
        .accounts
        .iter()
        .find(|a| a.display_name == "Spare")
        .expect("the saved account");
    assert!(
        matches!(
            &spare.source,
            CredentialSource::LocalFolder { detected, .. }
                if *detected == DetectedKind::ClaudeApiKeyEnv
        ),
        "{:?}",
        spare.source
    );
}

#[test]
fn the_sim_answers_a_changes_read_on_the_spot() {
    let mut app = timed(Scenario::CapsuleMulti, world_for(Scenario::CapsuleMulti));
    view_changes(&mut app);
    let shown = screen(&mut app);
    assert!(shown.contains("Inspect changes ·"), "{shown}");
}

fn until_loaded(app: &mut App) {
    for _ in 0..30 {
        if !screen(app).contains("loading ") {
            return;
        }
        ticks(app, 1);
    }
    panic!("the picker never loaded:\n{}", screen(app));
}

#[test]
fn the_sim_holds_a_1password_check_for_900_ms() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    open_picker(&mut app);
    until_loaded(&mut app);
    press(&mut app, "enter");
    until_loaded(&mut app);
    press(&mut app, "enter");
    until_loaded(&mut app);
    press(&mut app, "W o r k");
    press(&mut app, "enter");
    until_loaded(&mut app);
    press(&mut app, "enter");
    let chosen = screen(&mut app);
    assert!(chosen.contains("Anthropic · Work"), "{chosen}");

    click_text(&mut app, "Validate");
    let start = app.world.clock.now_ms;
    let pending = screen(&mut app);
    assert!(pending.contains("… material   checking"), "{pending}");
    ticks(&mut app, 5);
    assert!(
        screen(&mut app).contains("… material   checking"),
        "still waiting after 400 ms"
    );
    while screen(&mut app).contains("checking") && app.world.clock.now_ms - start < 3_000 {
        ticks(&mut app, 1);
    }
    let elapsed = app.world.clock.now_ms - start;
    assert!(
        (900..980).contains(&elapsed),
        "900 ms, delivered on the tick after: {elapsed}"
    );
    let done = screen(&mut app);
    assert!(
        done.contains("✓ material   1Password reference resolved"),
        "{done}"
    );
}

#[test]
fn a_change_made_while_the_registry_is_saving_is_refused_out_loud() {
    let (mut app, backend) = held_saves(Scenario::AccountsMixed);
    press(&mut app, "j j t");
    assert_eq!(backend.saves(), 1);

    press(&mut app, "j j space");

    assert_eq!(backend.saves(), 1, "the second change is not sent");
    let shown = status(&app);
    assert!(
        shown.contains("Saving the previous change") && shown.contains("try again"),
        "the refusal reaches the operator: {shown}"
    );
}

const DISCOVERED_FOLDER: &str = "~/.claude-spare";

#[test]
fn registering_a_copy_of_a_discovered_folder_replaces_the_discovered_account() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    let discovered = Account::discovered(
        "disc-claude-spare",
        "discovered",
        Provider::Anthropic,
        CredentialSource::LocalFolder {
            path: DISCOVERED_FOLDER.into(),
            detected: DetectedKind::Unknown,
        },
    );
    app.world.accounts.insert(discovered);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let ticket = backend.probe_ticket();
    app.deliver(&Msg::FolderProbed {
        ticket,
        result: Ok(FolderProbe::Found(DetectedKind::CodexAuthJson)),
    });
    ticks(&mut app, 4);

    let shown = screen(&mut app);
    assert!(!shown.contains("Already registered"), "{shown}");
    let Some(Effect::SaveAccounts { registry, .. }) = backend
        .seen()
        .into_iter()
        .find(|e| matches!(e, Effect::SaveAccounts { .. }))
    else {
        panic!("the copy is stored:\n{shown}");
    };
    let at_folder: Vec<_> = registry
        .accounts
        .iter()
        .filter(|a| {
            matches!(&a.source, CredentialSource::LocalFolder { path, .. } if path == DISCOVERED_FOLDER)
        })
        .map(|a| (a.display_name.as_str(), a.origin))
        .collect();
    assert_eq!(at_folder, vec![("Spare", AccountOrigin::Registered)]);
}

const KEY_FIELD: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t";

#[test]
fn a_key_with_a_direction_mark_is_refused_before_the_backend_sees_it() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, KEY_FIELD);
    Application::handle(&mut app, Input::Paste("\u{202e}-abc".into()));
    press(&mut app, "tab");
    click_text(&mut app, "Validate");
    ticks(&mut app, 4);

    assert_eq!(backend.checks(), [], "{:?}", backend.seen());
    let shown = screen(&mut app);
    assert!(
        shown.contains("The API key holds a control or direction character"),
        "{shown}"
    );
}

#[test]
fn a_key_of_blanks_is_refused() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(
        &mut app,
        "c a enter S p a r e tab tab tab down down tab enter space space tab",
    );
    click_text(&mut app, "Validate");
    ticks(&mut app, 4);
    assert_eq!(backend.checks(), []);
    assert!(
        screen(&mut app).contains("API key required: the field is empty"),
        "blanks are no key"
    );
}

#[test]
fn a_key_with_blanks_around_it_is_trimmed() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(
        &mut app,
        "c a enter S p a r e tab tab tab down down tab enter space s k - a n t space tab",
    );
    click_text(&mut app, "Validate");
    let checks = backend.checks();
    let [Effect::CheckSource { key: Some(key), .. }] = checks.as_slice() else {
        panic!("one check with a key expected, got {:?}", backend.seen());
    };
    assert_eq!(key.expose(), "sk-ant");
}

const PERSONAL: &str = "acct-claude-personal";

fn personal(world: &World) -> &Account {
    world.accounts.get(PERSONAL).expect("personal account")
}

/// The ticket of the newest refresh the backend was asked to run for `id`.
fn refresh_ticket_of(backend: &Held, id: &str) -> u64 {
    backend.last_ticket(|e| match e {
        Effect::RefreshAccount {
            ticket, account, ..
        } if account == id => Some(*ticket),
        _ => None,
    })
}

fn refreshes_of(backend: &Held, id: &str) -> usize {
    backend
        .refreshes()
        .iter()
        .filter(|e| matches!(e, Effect::RefreshAccount { account, .. } if account == id))
        .count()
}

/// What a provider that read 61% used would answer for `ticket`.
fn refreshed(app: &App, ticket: u64, id: &str, pct: u8) -> Msg {
    let account = app.world.accounts.get(id).expect("account");
    let now = app.world.now_secs();
    let mut usage = account.usage.clone();
    usage.freshness = FreshnessInfo::current(now);
    usage.windows[0].used_pct = Some(pct);
    Msg::AccountRefreshed {
        ticket,
        account: id.to_owned(),
        source: Box::new(account.source.clone()),
        result: Ok(Box::new(Refresh {
            usage,
            issue: None,
            last_refresh_secs: now,
            outcome: RefreshOutcome::NeedsAttention,
        })),
    }
}

fn refresh_failed(app: &App, ticket: u64, id: &str, error: &str) -> Msg {
    Msg::AccountRefreshed {
        ticket,
        account: id.to_owned(),
        source: Box::new(app.world.accounts.get(id).expect("account").source.clone()),
        result: Err(error.into()),
    }
}

/// A roster arriving from the same backend: every account as it is now, the
/// credential sources untouched, and no account marked as refreshing.
fn idle_roster(app: &App) -> Snapshot {
    let mut snapshot = jackin_console::sim::snapshot(&app.world);
    for account in &mut snapshot.accounts.accounts {
        account.usage.freshness.phase = Freshness::Current;
    }
    snapshot
}

fn used_pct(app: &App) -> Option<u8> {
    personal(&app.world).usage.windows.first()?.used_pct
}

#[test]
fn refresh_snapshot_preserves_single_flight_across_pages() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down r");
    assert_eq!(refreshes_of(&backend, PERSONAL), 1, "{:?}", backend.seen());
    let a = refresh_ticket_of(&backend, PERSONAL);

    let idle = idle_roster(&app);
    let roster = roster_reply(&mut app, idle);
    app.deliver(&roster);
    assert_eq!(
        personal(&app.world).usage.freshness.phase,
        Freshness::Refreshing,
        "the roster does not end the refresh in flight"
    );

    app.go(Go::Usage {
        select: Some(PERSONAL.into()),
    });
    press(&mut app, "r");
    assert_eq!(
        refreshes_of(&backend, PERSONAL),
        1,
        "the refresh in flight owns the account: {:?}",
        backend.refreshes()
    );
    assert_eq!(app.world.refresh_pending(PERSONAL), Some(a));
    assert_eq!(
        personal(&app.world).usage.freshness.phase,
        Freshness::Refreshing
    );

    let reply = refreshed(&app, a, PERSONAL, 61);
    app.deliver(&reply);
    assert_eq!(app.world.refresh_pending(PERSONAL), None);
    assert_eq!(used_pct(&app), Some(61));
    assert_eq!(
        personal(&app.world).usage.freshness.phase,
        Freshness::Current
    );
}

#[test]
fn refresh_old_ticket_cannot_overwrite_new_result() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down r");
    let a = refresh_ticket_of(&backend, PERSONAL);
    let a_ok = refreshed(&app, a, PERSONAL, 61);
    let a_err = refresh_failed(&app, a, PERSONAL, "old failure");
    app.deliver(&a_ok);
    assert_eq!(used_pct(&app), Some(61));

    press(&mut app, "r");
    let b = refresh_ticket_of(&backend, PERSONAL);
    assert_ne!(a, b);
    assert_eq!(refreshes_of(&backend, PERSONAL), 2);
    let started = status(&app);

    for stale in [&a_ok, &a_err] {
        assert_eq!(app.world.absorb(stale), Absorbed::Stale);
        app.deliver(stale);
        assert_eq!(
            app.world.refresh_pending(PERSONAL),
            Some(b),
            "B keeps its slot"
        );
        assert_eq!(
            personal(&app.world).usage.freshness.phase,
            Freshness::Refreshing
        );
        assert_eq!(status(&app), started);
    }

    let b_ok = refreshed(&app, b, PERSONAL, 77);
    app.deliver(&b_ok);
    assert_eq!(used_pct(&app), Some(77));
    let finished = status(&app);
    assert!(
        finished.starts_with("Refreshed Claude · Personal"),
        "{finished}"
    );

    for stale in [&a_ok, &a_err, &b_ok] {
        app.deliver(stale);
        assert_eq!(used_pct(&app), Some(77));
        assert_eq!(
            personal(&app.world).usage.freshness.phase,
            Freshness::Current
        );
        assert_eq!(personal(&app.world).issue, None);
        assert_eq!(status(&app), finished);
    }
}

fn with_source(snapshot: &mut Snapshot, id: &str, source: CredentialSource) {
    snapshot.accounts.get_mut(id).expect("account").source = source;
}

#[test]
fn refresh_source_change_and_return_does_not_revive_old_request() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    let original = personal(&app.world).source.clone();
    let other = CredentialSource::HostEnv {
        var: "OTHER_TOKEN".into(),
        detected: DetectedKind::ClaudeApiKeyEnv,
    };
    press(&mut app, "c down down r");
    let a = refresh_ticket_of(&backend, PERSONAL);
    let a_ok = refreshed(&app, a, PERSONAL, 61);
    let a_err = refresh_failed(&app, a, PERSONAL, "old failure");

    let mut changed = idle_roster(&app);
    with_source(&mut changed, PERSONAL, other);
    let msg = roster_reply(&mut app, changed);
    app.deliver(&msg);
    assert_eq!(
        app.world.refresh_pending(PERSONAL),
        None,
        "the source changed"
    );
    assert_ne!(
        personal(&app.world).usage.freshness.phase,
        Freshness::Refreshing
    );

    let mut back = idle_roster(&app);
    with_source(&mut back, PERSONAL, original);
    let msg = roster_reply(&mut app, back);
    app.deliver(&msg);
    assert_eq!(app.world.refresh_pending(PERSONAL), None, "no revival");

    press(&mut app, "r");
    let b = refresh_ticket_of(&backend, PERSONAL);
    assert_ne!(a, b);
    for stale in [&a_ok, &a_err] {
        assert_eq!(app.world.absorb(stale), Absorbed::Stale);
        assert_eq!(app.world.refresh_pending(PERSONAL), Some(b));
        assert_eq!(
            personal(&app.world).usage.freshness.phase,
            Freshness::Refreshing
        );
    }
    app.deliver(&refreshed(&app, b, PERSONAL, 77));
    assert_eq!(used_pct(&app), Some(77));
}

#[test]
fn refresh_removed_and_readded_account_does_not_revive_old_request() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down r");
    let a = refresh_ticket_of(&backend, PERSONAL);
    let a_ok = refreshed(&app, a, PERSONAL, 61);

    let full = idle_roster(&app);
    let mut removed = full.clone();
    removed.accounts.remove(PERSONAL);
    let msg = roster_reply(&mut app, removed);
    app.deliver(&msg);
    assert_eq!(app.world.refresh_pending(PERSONAL), None);
    assert_eq!(app.world.absorb(&a_ok), Absorbed::Stale);

    let msg = roster_reply(&mut app, full);
    app.deliver(&msg);
    assert_eq!(app.world.refresh_pending(PERSONAL), None);
    assert_eq!(app.world.absorb(&a_ok), Absorbed::Stale);
    assert_eq!(
        app.world.refresh_account(PERSONAL, 0).map(|t| t != a),
        Some(true)
    );
}

#[test]
fn refresh_distinct_accounts_progress_independently() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    app.go(Go::Usage {
        select: Some(PERSONAL.into()),
    });
    press(&mut app, "r");
    let x = PERSONAL;
    let y = "acct-claude-work";
    let (tx, ty) = (
        refresh_ticket_of(&backend, x),
        refresh_ticket_of(&backend, y),
    );
    assert_ne!(tx, ty);

    app.deliver(&refreshed(&app, tx, x, 61));
    assert_eq!(app.world.refresh_pending(x), None);
    assert_eq!(app.world.refresh_pending(y), Some(ty));
    assert_eq!(
        app.world.accounts.get(y).expect("y").usage.freshness.phase,
        Freshness::Refreshing
    );

    app.go(Go::Manager);
    app.go(Go::Usage { select: None });
    assert_eq!(app.world.refresh_pending(y), Some(ty));
    press(&mut app, "r");
    assert_eq!(refreshes_of(&backend, y), 1, "Y is still in flight");
    assert_eq!(refreshes_of(&backend, x), 2, "X is idle and starts again");

    app.deliver(&refreshed(&app, ty, y, 70));
    assert_eq!(app.world.refresh_pending(y), None);
    assert!(
        app.world.refresh_pending(x).is_some(),
        "X's new refresh stays"
    );
}

#[test]
fn refresh_stale_reply_has_no_ui_or_revision_effect() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down r");
    let a = refresh_ticket_of(&backend, PERSONAL);
    let ok = refreshed(&app, a, PERSONAL, 61);
    app.deliver(&ok);
    press(&mut app, "r");
    let b = refresh_ticket_of(&backend, PERSONAL);
    assert_ne!(a, b);

    let stale = refresh_failed(&app, a, PERSONAL, "late failure");
    let before = (
        screen(&mut app),
        status(&app),
        app.world.jobs.len(),
        app.world.accounts.revision,
        personal(&app.world).clone(),
    );
    assert_eq!(app.deliver(&stale), termrock::core::event::Outcome::Ignored);
    let after = (
        screen(&mut app),
        status(&app),
        app.world.jobs.len(),
        app.world.accounts.revision,
        personal(&app.world).clone(),
    );
    assert_eq!(before, after);
    assert!(!after.0.contains("late failure"), "{}", after.0);

    app.deliver(&refresh_failed(&app, b, PERSONAL, "provider is down"));
    assert!(
        screen(&mut app).contains("provider is down") || status(&app).contains("provider is down"),
        "the current reply still reaches the screen"
    );
    assert_eq!(
        personal(&app.world).usage.freshness.phase,
        Freshness::Stale,
        "the current error still ends the refresh"
    );
}

const NEXT: &str = "acct-claude-archive";

fn validation_ticket_of(backend: &Held, id: &str) -> u64 {
    *validation_tickets(&backend.seen(), id)
        .last()
        .unwrap_or_else(|| panic!("no validation of {id}: {:?}", backend.seen()))
}

fn validations_of(backend: &Held, id: &str) -> usize {
    validation_tickets(&backend.seen(), id).len()
}

fn validation_state(app: &App, id: &str) -> ValidationState {
    app.world
        .accounts
        .get(id)
        .expect("account")
        .validation
        .clone()
}

fn never_validated_roster(app: &App) -> Snapshot {
    let mut snapshot = idle_roster(app);
    for account in &mut snapshot.accounts.accounts {
        account.validation = ValidationState::NeverValidated;
    }
    snapshot
}

#[test]
fn validation_old_reply_does_not_clear_current_progress() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down v v");
    assert_eq!(validations_of(&backend, PERSONAL), 2);
    let tickets = validation_tickets(&backend.seen(), PERSONAL);
    let (a, b) = (tickets[0], tickets[1]);
    let progress = status(&app);
    assert!(progress.starts_with("Validating"), "{progress}");

    let old = validated(&app, a, PERSONAL, valid_fingerprint());
    let before = (screen(&mut app), personal(&app.world).clone());
    assert_eq!(app.deliver(&old), Outcome::Ignored);
    assert_eq!(
        (screen(&mut app), personal(&app.world).clone()),
        before,
        "A changes nothing on screen or in the account"
    );
    assert!(matches!(
        validation_state(&app, PERSONAL),
        ValidationState::Validating { .. }
    ));
    assert_eq!(app.world.validation_pending(PERSONAL), Some(b));
    assert_eq!(status(&app), progress, "B keeps the progress line");
    assert_eq!(app.shell.status.job(), Some(job::ACCOUNT_VALIDATE));

    app.deliver(&validated(&app, b, PERSONAL, valid_fingerprint()));
    assert_eq!(
        validation_state(&app, PERSONAL),
        ValidationState::Valid(ValidationLevel::QuotaReadable)
    );
    assert_eq!(app.shell.status.job(), None);
}

#[test]
fn validation_source_replacement_retires_ticket() {
    let replaced = CredentialSource::HostEnv {
        var: "OTHER_TOKEN".into(),
        detected: DetectedKind::ClaudeApiKeyEnv,
    };
    for removal in [false, true] {
        let (mut app, backend) = held(Scenario::AccountsMixed);
        press(&mut app, "c down down v");
        let a = validation_ticket_of(&backend, PERSONAL);
        let old_ok = validated(&app, a, PERSONAL, valid_fingerprint());
        let old_err = validated(&app, a, PERSONAL, Err("old failure".into()));
        assert!(status(&app).starts_with("Validating"));

        let mut roster = never_validated_roster(&app);
        if removal {
            roster.accounts.remove(PERSONAL);
        } else {
            with_source(&mut roster, PERSONAL, replaced.clone());
        }
        let msg = roster_reply(&mut app, roster);
        app.deliver(&msg);
        assert_eq!(app.world.validation_pending(PERSONAL), None);
        assert_eq!(app.shell.status.job(), None, "no job left spinning");
        assert_eq!(status(&app), "", "retiring prints no result");

        screen(&mut app);
        let before = (
            app.world.accounts.get(PERSONAL).cloned(),
            screen(&mut app),
            status(&app),
        );
        for stale in [&old_ok, &old_err] {
            assert_eq!(app.world.absorb(stale), Absorbed::Stale);
            assert_eq!(app.deliver(stale), Outcome::Ignored);
        }
        let after = (
            app.world.accounts.get(PERSONAL).cloned(),
            screen(&mut app),
            status(&app),
        );
        assert_eq!(before, after, "removal: {removal}");
        assert!(!after.2.contains("old failure"));
    }
}

#[test]
fn validation_duplicate_and_same_source_refresh() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down v");
    let a = validation_ticket_of(&backend, PERSONAL);
    let progress = status(&app);

    let roster = never_validated_roster(&app);
    let msg = roster_reply(&mut app, roster);
    app.deliver(&msg);
    assert_eq!(app.world.validation_pending(PERSONAL), Some(a));
    assert!(matches!(
        validation_state(&app, PERSONAL),
        ValidationState::Validating { .. }
    ));
    assert_eq!(status(&app), progress, "a roster does not end the job");
    assert_eq!(app.shell.status.job(), Some(job::ACCOUNT_VALIDATE));

    let reply = validated(&app, a, PERSONAL, valid_fingerprint());
    assert_eq!(app.world.absorb(&reply), Absorbed::Current);
    assert_eq!(app.world.absorb(&reply), Absorbed::Stale, "accepted once");

    app.world
        .accounts
        .get_mut(PERSONAL)
        .expect("account")
        .validation = ValidationState::NeverValidated;
    assert_eq!(app.deliver(&reply), Outcome::Ignored);
    assert_eq!(
        validation_state(&app, PERSONAL),
        ValidationState::NeverValidated,
        "the duplicate changes nothing"
    );
}

#[test]
fn validation_nonowner_completion_preserves_status_text_and_job() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down v down v");
    let x = validation_ticket_of(&backend, PERSONAL);
    let y = validation_ticket_of(&backend, NEXT);
    let progress = status(&app);
    assert!(progress.starts_with("Validating"), "{progress}");
    assert!(app.world.validation_owns_status(NEXT, y));

    app.deliver(&validated(&app, x, PERSONAL, valid_fingerprint()));
    assert_eq!(
        validation_state(&app, PERSONAL),
        ValidationState::Valid(ValidationLevel::QuotaReadable)
    );
    assert_eq!(status(&app), progress, "Y's progress text stays");
    assert_eq!(app.shell.status.job(), Some(job::ACCOUNT_VALIDATE));
    assert!(app.world.validation_owns_status(NEXT, y));

    app.deliver(&validated(&app, y, NEXT, valid_fingerprint()));
    assert_eq!(app.shell.status.job(), None);
    assert!(!app.world.validation_owns_status(NEXT, y));
    let done = status(&app);
    assert!(
        done.starts_with("Claude · Archived") && done.ends_with("quota readable"),
        "Y's own result replaces its progress: {done}"
    );
}

#[test]
fn validation_owner_source_replacement_ends_the_job() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down v down v");
    let x = validation_ticket_of(&backend, PERSONAL);
    let y = validation_ticket_of(&backend, NEXT);
    assert!(app.world.validation_owns_status(NEXT, y));

    let mut roster = never_validated_roster(&app);
    roster.accounts.remove(NEXT);
    let msg = roster_reply(&mut app, roster);
    app.deliver(&msg);
    assert_eq!(
        app.shell.status.job(),
        None,
        "no spinning job for a gone account"
    );
    assert_eq!(app.world.validation_pending(NEXT), None);
    assert_eq!(
        app.world.validation_pending(PERSONAL),
        Some(x),
        "X is unaffected"
    );
}

#[test]
fn validation_snapshot_preserves_original_started_tick() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, "c down down v");
    let a = validation_ticket_of(&backend, PERSONAL);
    let ValidationState::Validating { started_tick } = validation_state(&app, PERSONAL) else {
        panic!("the validation shows as running");
    };

    for _ in 0..3 {
        ticks(&mut app, 5);
        assert!(
            app.world.clock.now_ms as u64 > started_tick,
            "time moved on"
        );
        let roster = never_validated_roster(&app);
        let msg = roster_reply(&mut app, roster);
        app.deliver(&msg);
        assert_eq!(
            validation_state(&app, PERSONAL),
            ValidationState::Validating { started_tick },
            "elapsed time counts from the request, not from the roster"
        );
    }
    assert_eq!(app.world.validation_pending(PERSONAL), Some(a));
}
