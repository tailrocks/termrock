//! Step BG4: the gaps the jackin account-source step (J3) found. Each test
//! states one gap in terms of what the operator sees or what the baseline
//! words. The gaps and their API are in `docs/P9.3-red-green.md`, section BG4.
#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{
    Backend, ChangeSet, ChangedFile, CheckRow, Completion, DiffLine, DiffStatus, Effect,
    FieldDescriptor, FileSource, Hunk, KeyOutcome, OnePassword, OpAccount, OpAccountRow,
    OpAccountState, OpError, OpField, OpItem, OpItemRow, OpListing, OpScope, OpSession, OpVault,
    OpVaultRow, Refresh, SecretClass, UnreadFile, ValidationOutcome, VaultAccess,
};
use jackin_console::domain::account::{
    AccountForm, AccountIdentity, Confidence, CredentialSource, IssueCode, Lifecycle,
    Recoverability, RecoverableIssue, ValidationLevel,
};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::onepassword::OpReference;
use jackin_console::domain::reason::{Found, IssueReason, RefreshOutcome, UsageReason};
use jackin_console::domain::usage::Freshness;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::onepassword::SimOnePassword;
use jackin_console::sim::{SimBackend, provider};
use jackin_console::world::{Msg, World};
use termrock::core::event::Input;
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

type Answer = fn(&Effect, &World) -> Option<Msg>;

#[test]
fn active_rate_limit_survives_a_broker_outage_refresh() {
    let world = world_for(Scenario::AccountsMixed);
    let account = world
        .accounts
        .get("acct-opencode-go")
        .expect("fixture account");
    let now = world.clock.now_secs();
    let refreshed = provider::refresh(account, now, false, true);

    assert_eq!(refreshed.outcome, RefreshOutcome::StillRateLimited);
    assert_eq!(
        refreshed.issue.as_ref().map(|issue| issue.code),
        Some(IssueCode::RateLimited)
    );
    assert_eq!(
        refreshed.issue.as_ref().and_then(|issue| issue.retry_secs),
        account.issue.as_ref().and_then(|issue| issue.retry_secs)
    );
    assert_eq!(refreshed.usage.freshness.phase, Freshness::Failed);
    assert_eq!(
        refreshed.usage.freshness.last_good_secs,
        account.usage.freshness.last_good_secs
    );
    assert_eq!(refreshed.usage.windows, account.usage.windows);
}

#[test]
fn expired_rate_limit_exposes_broker_outage_without_losing_last_good_values() {
    let world = world_for(Scenario::AccountsMixed);
    let mut account = world
        .accounts
        .get("acct-opencode-go")
        .expect("fixture account")
        .clone();
    let now = world.clock.now_secs();
    account.issue = Some(
        account
            .issue
            .take()
            .expect("rate limited issue")
            .retry(now - 1),
    );

    let refreshed = provider::refresh(&account, now, false, true);

    assert_eq!(refreshed.outcome, RefreshOutcome::BrokerUnreachable);
    assert_eq!(
        refreshed.issue.as_ref().map(|issue| issue.code),
        Some(IssueCode::ProviderUnavailable)
    );
    assert_eq!(
        refreshed.issue.as_ref().map(|issue| &issue.reason),
        Some(&IssueReason::BrokerDown)
    );
    assert_eq!(refreshed.usage.freshness.phase, Freshness::Stale);
    assert_eq!(
        refreshed.usage.freshness.last_good_secs,
        account.usage.freshness.last_good_secs
    );
    assert_eq!(refreshed.usage.windows, account.usage.windows);
}

/// The simulator, except that the requests this file is about get no answer
/// until the test delivers one, and one function may answer an effect first.
/// Every effect is recorded.
#[derive(Debug)]
struct Held {
    inner: Arc<dyn Backend>,
    answer: Answer,
    seen: Mutex<Vec<Effect>>,
}

impl Held {
    fn new(answer: Answer) -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            answer,
            seen: Mutex::new(vec![]),
        })
    }

    fn seen(&self) -> Vec<Effect> {
        let mut all = self.seen.lock().expect("seen").clone();
        all.retain(|e| !matches!(e, Effect::Refresh { .. }));
        all
    }

    fn last_ticket(&self, want: fn(&Effect) -> Option<u64>) -> u64 {
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

    fn staggers(&self) -> Vec<i64> {
        self.seen()
            .iter()
            .filter_map(|e| match e {
                Effect::RefreshAccount { stagger_ms, .. } => Some(*stagger_ms),
                _ => None,
            })
            .collect()
    }
}

impl Backend for Held {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        if let Some(msg) = (self.answer)(effect, world) {
            return Some(Completion::ready(0, msg));
        }
        match effect {
            Effect::CheckSource { .. }
            | Effect::ReadChanges { .. }
            | Effect::LoadOp { .. }
            | Effect::RefreshAccount { .. } => None,
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

fn no_answer(_: &Effect, _: &World) -> Option<Msg> {
    None
}

fn held(world: World, scenario: Scenario) -> (App, Arc<Held>) {
    held_answering(world, scenario, no_answer)
}

fn held_answering(mut world: World, scenario: Scenario, answer: Answer) -> (App, Arc<Held>) {
    let backend = Held::new(answer);
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

const PERSONAL: &str = "acct-claude-personal";

/// "Spare", provider Claude, the plain-text source and a key the simulator
/// takes as valid. The cursor ends on the key field's tail.
const PLAIN_KEY: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t - v a l i d - a b c d e f 1 2 3 4 tab";

fn words() -> Vocabulary {
    Vocabulary::openshell()
}

// Gap 1: an issue, a check row and a refresh outcome are typed values, and
// the Vocabulary words them.

fn issue_of(reason: IssueReason) -> RecoverableIssue {
    RecoverableIssue::new(
        IssueCode::OpFieldMissing,
        reason,
        Recoverability::ActionRequired,
    )
}

fn open_account_detail(app: &mut App) -> String {
    press(app, "c down down");
    screen(app)
}

#[test]
fn an_issue_is_worded_by_the_vocabulary() {
    let reason = IssueReason::OnePassword(OpError::MissingField {
        field: "credential".into(),
        item: "Anthropic".into(),
    });
    let mut world = world_for(Scenario::AccountsMixed);
    world.accounts.get_mut(PERSONAL).expect("the account").issue = Some(issue_of(reason.clone()));
    let mut app = app_with(Scenario::AccountsMixed, world);
    let shown = open_account_detail(&mut app);
    assert!(
        shown.contains("1Password field not found: credential in Anthropic"),
        "the standard words:\n{shown}"
    );

    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.vocabulary.reason.op_field_missing = "CANARY field {field} of {item}".into();
    world.accounts.get_mut(PERSONAL).expect("the account").issue = Some(issue_of(reason));
    let mut app = app_with(Scenario::AccountsMixed, world);
    let shown = open_account_detail(&mut app);
    assert!(
        shown.contains("CANARY field credential of Anthropic"),
        "the text comes from the vocabulary:\n{shown}"
    );
}

fn rows_outcome(app: &App) -> ValidationOutcome {
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
    outcome.level = None;
    outcome.identity = AccountIdentity::default();
    outcome.confidence = Confidence::None;
    outcome.lifecycle = Lifecycle::NeedsSecret;
    outcome.issue = Some(RecoverableIssue::new(
        IssueCode::CredentialFileMissing,
        IssueReason::EnvMissing {
            name: "CI_TOKEN".into(),
        },
        Recoverability::ActionRequired,
    ));
    outcome.usage = None;
    outcome.material = CheckRow::Failed(IssueReason::EnvMissing {
        name: "CI_TOKEN".into(),
    });
    outcome.identity_row = CheckRow::Skipped(IssueReason::Skipped);
    outcome.quota_row = CheckRow::Skipped(IssueReason::Usage(UsageReason::Unsupported));
    outcome
}

fn check_rows_screen(vocabulary: Vocabulary) -> String {
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.vocabulary = vocabulary;
    let (mut app, backend) = held(world, Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();
    let outcome = rows_outcome(&app);
    app.deliver(&Msg::SourceChecked {
        ticket,
        result: Ok(Box::new(outcome)),
    });
    screen(&mut app)
}

#[test]
fn check_rows_are_worded_by_the_vocabulary() {
    let shown = check_rows_screen(words());
    assert!(
        shown.contains("! material   Not set in this environment"),
        "a failed row:\n{shown}"
    );
    assert!(
        shown.contains("— identity   not attempted"),
        "a skipped row:\n{shown}"
    );
    assert!(shown.contains("— quota      Not supported"), "{shown}");
    assert!(
        shown.contains("That variable is not set in this environment"),
        "the issue line:\n{shown}"
    );

    let mut vocabulary = words();
    vocabulary.reason.env_missing.check = "CANARY row {name}".into();
    vocabulary.reason.env_missing.issue = "CANARY issue {name}".into();
    vocabulary.reason.skipped.check = "CANARY skipped".into();
    let shown = check_rows_screen(vocabulary);
    assert!(
        shown.contains("! material   CANARY row CI_TOKEN"),
        "{shown}"
    );
    assert!(shown.contains("CANARY issue CI_TOKEN"), "{shown}");
    assert!(shown.contains("— identity   CANARY skipped"), "{shown}");
}

fn refresh_still_limited(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::RefreshAccount {
        ticket,
        account,
        source,
        ..
    } = effect
    else {
        return None;
    };
    let now = world.now_secs();
    Some(Msg::AccountRefreshed {
        ticket: *ticket,
        account: account.clone(),
        source: Box::new(source.clone()),
        result: Ok(Box::new(Refresh {
            usage: world.accounts.get(account)?.usage.clone(),
            issue: None,
            last_refresh_secs: now,
            outcome: RefreshOutcome::StillRateLimited,
        })),
    })
}

#[test]
fn a_refresh_outcome_is_worded_by_the_vocabulary() {
    let (mut app, _) = held_answering(
        world_for(Scenario::AccountsMixed),
        Scenario::AccountsMixed,
        refresh_still_limited,
    );
    press(&mut app, "c down down r");
    ticks(&mut app, 40);
    assert!(
        status(&app).starts_with("Refreshed Claude · Personal · still rate limited · "),
        "{}",
        status(&app)
    );

    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.vocabulary.reason.outcome_still_rate_limited = "CANARY outcome".into();
    let (mut app, _) = held_answering(world, Scenario::AccountsMixed, refresh_still_limited);
    press(&mut app, "c down down r");
    ticks(&mut app, 40);
    assert!(
        status(&app).starts_with("Refreshed Claude · Personal · CANARY outcome · "),
        "{}",
        status(&app)
    );
}

fn texts(vocabulary: &Vocabulary, reason: &IssueReason) -> (String, String) {
    (vocabulary.reason(reason), vocabulary.check(reason))
}

fn row_text(vocabulary: &Vocabulary, row: &CheckRow) -> String {
    match row {
        CheckRow::Ok(found) => format!("ok {}", vocabulary.found(found)),
        CheckRow::Failed(reason) => format!("failed {}", vocabulary.check(reason)),
        CheckRow::Skipped(reason) => format!("skipped {}", vocabulary.check(reason)),
    }
}

fn the_rows(outcome: &ValidationOutcome) -> [String; 3] {
    let vocabulary = Vocabulary::demo();
    [
        row_text(&vocabulary, &outcome.material),
        row_text(&vocabulary, &outcome.identity_row),
        row_text(&vocabulary, &outcome.quota_row),
    ]
}

fn issue_text(outcome: &ValidationOutcome) -> Option<String> {
    let vocabulary = Vocabulary::demo();
    outcome
        .issue
        .as_ref()
        .map(|issue| vocabulary.reason(&issue.reason))
}

fn validated(provider: Provider, source: CredentialSource, key: Option<&str>) -> ValidationOutcome {
    let op = SimOnePassword::fixture(0);
    provider::validate(provider, &source, key, &op, 1_000_000)
}

fn folder(path: &str) -> CredentialSource {
    CredentialSource::LocalFolder {
        path: path.into(),
        detected: jackin_console::domain::account::DetectedKind::Unknown,
    }
}

fn plain() -> CredentialSource {
    CredentialSource::PlainApiKey {
        fingerprint: "00000000".into(),
        tail: "0000".into(),
    }
}

fn strings<const N: usize>(rows: [&str; N]) -> [String; N] {
    rows.map(str::to_owned)
}

#[test]
fn the_demo_keeps_every_text_the_simulator_wrote() {
    let missing = validated(Provider::Anthropic, folder("~/nowhere"), None);
    assert_eq!(
        issue_text(&missing).as_deref(),
        Some("Folder not found: ~/nowhere")
    );
    assert_eq!(
        the_rows(&missing),
        strings([
            "failed Nothing exists at the path",
            "skipped not attempted",
            "skipped not attempted"
        ])
    );

    let locked = validated(Provider::OpenAi, folder("~/.codex-locked"), None);
    assert_eq!(
        issue_text(&locked).as_deref(),
        Some("Folder not readable: permission denied for ~/.codex-locked")
    );
    assert_eq!(the_rows(&locked)[0], "failed mode 000 · owner root");

    let no_login = validated(Provider::XAi, folder("~/.grok"), None);
    assert_eq!(
        issue_text(&no_login).as_deref(),
        Some("No credential found: ~/.grok has no auth.json")
    );
    assert_eq!(the_rows(&no_login)[0], "failed Run the agent login first");

    let broken = validated(Provider::OpenCode, folder("~/.opencode-broken"), None);
    assert_eq!(
        issue_text(&broken).as_deref(),
        Some("Credential unreadable: auth.json is not valid JSON")
    );
    assert_eq!(
        the_rows(&broken)[0],
        "ok auth.json found · parse failed at line 1"
    );

    let other = validated(Provider::Anthropic, folder("~/.codex"), None);
    assert_eq!(
        issue_text(&other).as_deref(),
        Some("Folder belongs to another provider: found a OpenAI profile, expected Anthropic")
    );
    assert_eq!(the_rows(&other)[0], "ok Detected Codex auth.json");

    let presence = validated(Provider::Anthropic, folder("~/.claude-work"), None);
    assert_eq!(
        issue_text(&presence).as_deref(),
        Some("Identity unresolved · showing usage without a public handle")
    );
    assert_eq!(
        the_rows(&presence),
        strings([
            "ok Detected Claude API key env",
            "skipped No stable public identity",
            "skipped Confidence: presence only"
        ])
    );

    let good = validated(Provider::Anthropic, folder("~/.claude"), None);
    let windows = good.usage.as_ref().expect("usage").windows.len();
    assert_eq!(issue_text(&good), None);
    assert_eq!(
        the_rows(&good),
        [
            "ok Detected Claude OAuth profile".to_owned(),
            "ok Authenticated as alexey@donbeave.dev · Max 5x".to_owned(),
            format!("ok {windows} windows readable"),
        ]
    );
}

#[test]
fn the_demo_keeps_every_key_text_the_simulator_wrote() {
    let key = |provider, value| validated(provider, plain(), Some(value));

    let empty = key(Provider::Anthropic, "");
    assert_eq!(
        issue_text(&empty).as_deref(),
        Some("API key required: the field is empty")
    );
    assert_eq!(
        the_rows(&empty),
        strings([
            "failed Field is empty",
            "skipped not attempted",
            "skipped not attempted"
        ])
    );

    let rejected = key(Provider::Anthropic, "bogus");
    assert_eq!(
        issue_text(&rejected).as_deref(),
        Some("Key rejected: the provider returned 401")
    );
    assert_eq!(
        the_rows(&rejected),
        strings([
            "ok Key material present",
            "failed Not authenticated (401) · key rotated or revoked",
            "skipped not attempted"
        ])
    );

    let mismatch = key(Provider::Anthropic, "sk-valid-x");
    assert_eq!(
        issue_text(&mismatch).as_deref(),
        Some("Provider mismatch: the key belongs to OpenAI, expected Anthropic")
    );
    assert_eq!(the_rows(&mismatch)[1], "failed Key class is OpenAI");

    let limited = key(Provider::Anthropic, "sk-ant-throttled");
    assert_eq!(
        issue_text(&limited).as_deref(),
        Some("Rate limited: retry after 25 min")
    );
    assert_eq!(
        the_rows(&limited),
        strings([
            "ok Key material present",
            "ok Authenticated as alexey@chainargos.com",
            "failed Rate limited (429) · retry after 25 min"
        ])
    );

    let down = key(Provider::Anthropic, "sk-ant-outage");
    assert_eq!(
        issue_text(&down).as_deref(),
        Some("Provider unavailable: Anthropic did not respond")
    );
    assert_eq!(the_rows(&down)[1], "failed Timed out after 8 s");

    let hidden = key(Provider::Gemini, "valid-key");
    assert_eq!(
        issue_text(&hidden).as_deref(),
        Some("Quota not visible: Gemini does not expose usage for API keys")
    );
    assert_eq!(
        the_rows(&hidden),
        strings([
            "ok Key material present",
            "ok Authenticated as unknown · unknown",
            "skipped Quota not visible for API keys"
        ])
    );
}

#[test]
fn the_demo_worlds_keep_every_issue_text() {
    let vocabulary = Vocabulary::demo();
    let mut seen = std::collections::BTreeSet::new();
    for scenario in Scenario::ALL {
        let world = world_for(scenario);
        for account in &world.accounts.accounts {
            for issue in account.issue.iter() {
                seen.insert(vocabulary.reason(&issue.reason));
            }
            if let jackin_console::domain::account::ValidationState::Invalid(issue) =
                &account.validation
            {
                seen.insert(vocabulary.reason(&issue.reason));
            }
        }
    }
    let was: std::collections::BTreeSet<String> = [
        "Usage stale · last good 47 min ago",
        "Quota not visible: OpenAI does not expose usage for API keys",
        "Rate limited: retry after 25 min",
        "Identity unresolved · showing usage without a public handle",
        "Usage stale · last good 2 h ago",
        "No credential found: ~/.kimi has no api key",
        "Provider unavailable: MiniMax did not respond",
        "Quota not visible: OpenCode returned 403",
        "Not authorized: xAI rejected the credential",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(seen, was);
}

#[test]
fn the_words_of_the_other_reasons_name_their_facts() {
    let vocabulary = words();
    let named = |reason: IssueReason, issue: &str, check: &str| {
        assert_eq!(
            texts(&vocabulary, &reason),
            (issue.to_owned(), check.to_owned()),
            "{reason:?}"
        );
    };
    named(
        IssueReason::OnePassword(OpError::AuthorizationRequired {
            account: "acme.1password.com".into(),
        }),
        "1Password authorization required: sign in to acme.1password.com",
        "1Password authorization required: sign in to acme.1password.com",
    );
    named(
        IssueReason::OnePassword(OpError::NotAvailable),
        "1Password not available: the 1Password CLI could not be run",
        "1Password not available: the 1Password CLI could not be run",
    );
    named(
        IssueReason::OnePassword(OpError::MissingItem {
            item: "Anthropic".into(),
            vault: "Engineering".into(),
        }),
        "1Password item not found: Anthropic in Engineering",
        "1Password item not found: Anthropic in Engineering",
    );
    named(
        IssueReason::OnePassword(OpError::MissingField {
            field: "credential".into(),
            item: "Anthropic".into(),
        }),
        "1Password field not found: credential in Anthropic",
        "1Password field not found: credential in Anthropic",
    );
    named(
        IssueReason::EnvMissing {
            name: "CI_TOKEN".into(),
        },
        "That variable is not set in this environment",
        "Not set in this environment",
    );
    named(
        IssueReason::FileMissing {
            path: "/etc/key".into(),
        },
        "File not found: /etc/key",
        "No file at the path",
    );
    named(
        IssueReason::FileNotRegular {
            path: "/etc".into(),
        },
        "Not a regular file: /etc",
        "Not a regular file",
    );
    named(
        IssueReason::FolderUnreadable {
            path: "~/.codex".into(),
            detail: "Permission denied".into(),
        },
        "Folder not readable: permission denied for ~/.codex",
        "Permission denied",
    );
    named(
        IssueReason::LoginInvalid {
            file: "auth.json".into(),
        },
        "Credential unreadable: auth.json is not valid JSON",
        "auth.json is not valid JSON",
    );
    named(
        IssueReason::WrongProvider {
            found: None,
            expected: Provider::OpenAi,
        },
        "Folder belongs to another provider: found a unknown profile, expected OpenAI",
        "Found a unknown profile",
    );
    named(
        IssueReason::Usage(UsageReason::ExpiredLogin),
        "Login expired: sign in again",
        "Login expired",
    );
    named(
        IssueReason::Usage(UsageReason::NoCredentials),
        "No credentials: nothing to read usage with",
        "No credentials",
    );
    named(
        IssueReason::Usage(UsageReason::RateLimited {
            retry_in_secs: None,
        }),
        "Rate limited: retry later",
        "Rate limited (429)",
    );
    named(
        IssueReason::Usage(UsageReason::Network {
            detail: "connection reset".into(),
        }),
        "Network error: connection reset",
        "connection reset",
    );
    named(
        IssueReason::Usage(UsageReason::Provider {
            detail: "HTTP 502".into(),
        }),
        "Provider error: HTTP 502",
        "HTTP 502",
    );
    named(IssueReason::Skipped, "not attempted", "not attempted");

    assert_eq!(
        vocabulary.found(&Found::Login {
            account: "org_7Hq2".into()
        }),
        "ChatGPT login for org_7Hq2"
    );
    assert_eq!(
        vocabulary.found(&Found::Level(ValidationLevel::QuotaReadable)),
        "quota readable"
    );
    for (outcome, text) in [
        (RefreshOutcome::Current, "current"),
        (RefreshOutcome::Stale, "stale"),
        (RefreshOutcome::Failed, "failed"),
        (RefreshOutcome::StillRateLimited, "still rate limited"),
        (RefreshOutcome::ProviderUnavailable, "provider unavailable"),
        (RefreshOutcome::NeedsAttention, "needs attention"),
        (
            RefreshOutcome::BrokerUnreachable,
            "broker unreachable · last good kept",
        ),
        (RefreshOutcome::OpLocked, "failed · 1Password locked"),
    ] {
        assert_eq!(vocabulary.refresh_outcome(outcome), text);
    }
}

// Gap 2: the account form lists every provider the Provider enum has.

fn provider_slot(p: Provider) -> usize {
    match p {
        Provider::Anthropic => 0,
        Provider::OpenAi => 1,
        Provider::Amp => 2,
        Provider::XAi => 3,
        Provider::OpenCode => 4,
        Provider::Moonshot => 5,
        Provider::Zai => 6,
        Provider::MiniMax => 7,
        Provider::Cursor => 8,
        Provider::Gemini => 9,
        Provider::Openrouter => 10,
        Provider::Antigravity => 11,
        Provider::Hermes => 12,
        Provider::Muse => 13,
        Provider::Mimo => 14,
    }
}

#[test]
fn the_account_form_lists_every_provider() {
    for (at, provider) in Provider::ALL.iter().enumerate() {
        assert_eq!(provider_slot(*provider), at, "{provider:?}");
    }
    let form = AccountForm::openshell();
    let listed: Vec<Provider> = form.providers.iter().map(|c| c.provider).collect();
    assert_eq!(listed, Provider::ALL);
    let labels: std::collections::BTreeSet<&str> =
        form.providers.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels.len(), Provider::ALL.len(), "every label differs");
    assert!(labels.contains("Claude Code · Anthropic / Claude"));
    assert!(labels.contains("Z.AI"));
    assert!(labels.contains("Google Gemini"));

    let form = AccountForm::default();
    let demo: Vec<&str> = form.providers.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        demo,
        [
            "Claude Code · Anthropic / Claude",
            "Codex · OpenAI",
            "Grok Build · xAI / Grok",
            "OpenCode · OpenCode",
        ]
    );
}

fn provider_chosen_last(form: AccountForm) -> String {
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = form;
    let mut app = app_with(Scenario::AccountsMixed, world);
    press(&mut app, "c a tab tab");
    for _ in 0..=Provider::ALL.len() {
        press(&mut app, "down");
    }
    press(&mut app, "enter");
    screen_at(&mut app, 140, 60)
}

#[test]
fn the_form_offers_the_providers_its_data_lists() {
    let demo = provider_chosen_last(AccountForm::default());
    assert!(demo.contains("OpenCode · OpenCode"), "{demo}");
    assert!(!demo.contains("Muse"), "{demo}");

    let live = provider_chosen_last(AccountForm::openshell());
    assert!(
        live.contains("Mimo"),
        "the last provider is chosen:\n{live}"
    );
    assert!(!live.contains("OpenCode · OpenCode"), "{live}");
}

// Gap 3: a picker row with no field count or no access draws neither.

fn open_picker(app: &mut App) {
    press(app, "a");
    click_text(app, "Choose…");
}

fn deliver(app: &mut App, backend: &Held, listing: OpListing) {
    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Ok(listing),
    });
}

fn line_of<'a>(shown: &'a str, text: &str) -> &'a str {
    shown
        .lines()
        .find(|l| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{shown}"))
}

#[test]
fn a_picker_row_with_no_count_or_access_draws_nothing() {
    let (mut app, backend) = held(world_for(Scenario::AccountsMixed), Scenario::AccountsMixed);
    open_picker(&mut app);
    deliver(
        &mut app,
        &backend,
        OpListing::Accounts(vec![
            OpAccountRow {
                id: "one.1password.com".into(),
                email: "one@example.test".into(),
                state: OpAccountState::Available,
            },
            OpAccountRow {
                id: "two.1password.com".into(),
                email: "two@example.test".into(),
                state: OpAccountState::Available,
            },
        ]),
    );
    press(&mut app, "enter");
    deliver(
        &mut app,
        &backend,
        OpListing::Vaults(vec![
            OpVaultRow {
                id: "v1".into(),
                name: "Engineering".into(),
                items: 12,
                access: None,
            },
            OpVaultRow {
                id: "v2".into(),
                name: "Cold Storage".into(),
                items: 3,
                access: Some(VaultAccess::ReadOnly),
            },
        ]),
    );
    let vaults = screen(&mut app);
    assert!(
        !line_of(&vaults, "Engineering").contains("read-only"),
        "{vaults}"
    );
    assert!(
        line_of(&vaults, "Engineering").contains("12 items"),
        "{vaults}"
    );
    assert!(
        line_of(&vaults, "Cold Storage").contains("read-only"),
        "{vaults}"
    );

    press(&mut app, "enter");
    deliver(
        &mut app,
        &backend,
        OpListing::Items(vec![
            OpItemRow {
                id: "i1".into(),
                title: "Zeta Credential".into(),
                category: "API Credential".into(),
                fields: None,
            },
            OpItemRow {
                id: "i2".into(),
                title: "Spare Login".into(),
                category: "Login".into(),
                fields: Some(0),
            },
        ]),
    );
    let items = screen(&mut app);
    let unknown = line_of(&items, "Zeta Credential");
    assert!(unknown.contains("API Credential"), "{items}");
    assert!(!unknown.contains("fields"), "{items}");
    assert!(
        line_of(&items, "Spare Login").contains("Login · 0 fields"),
        "{items}"
    );
}

// Gap 4: OnePassword::accounts() returns owned data, so a cache shared with
// the backend can lend it.

#[derive(Debug, Clone)]
struct Cached {
    inner: SimOnePassword,
    cache: Arc<Mutex<Vec<OpAccount>>>,
}

impl OnePassword for Cached {
    fn session(&self) -> OpSession {
        self.inner.session()
    }

    fn set_session(&mut self, session: OpSession) {
        self.inner.set_session(session);
    }

    fn accounts(&self) -> Vec<OpAccount> {
        self.cache.lock().expect("cache").clone()
    }

    fn latency_ms(&self) -> i64 {
        self.inner.latency_ms()
    }

    fn list_accounts(&self) -> Result<Vec<&OpAccount>, OpError> {
        self.inner.list_accounts()
    }

    fn list_vaults(&self, account: &str) -> Result<Vec<&OpVault>, OpError> {
        self.inner.list_vaults(account)
    }

    fn list_items(&self, account: &str, vault: &str) -> Result<Vec<&OpItem>, OpError> {
        self.inner.list_items(account, vault)
    }

    fn list_fields(
        &self,
        account: &str,
        vault: &str,
        item: &str,
    ) -> Result<Vec<&OpField>, OpError> {
        self.inner.list_fields(account, vault, item)
    }

    fn reference(
        &self,
        account: &str,
        vault: &str,
        item: &str,
        field_id: &str,
        section: Option<&str>,
    ) -> Result<OpReference, OpError> {
        self.inner
            .reference(account, vault, item, field_id, section)
    }

    fn describe(&self, reference: &OpReference) -> Result<FieldDescriptor, OpError> {
        self.inner.describe(reference)
    }

    fn endpoint_of(&self, reference: &OpReference) -> Option<String> {
        self.inner.endpoint_of(reference)
    }

    fn classify(&self, reference: &OpReference) -> Result<SecretClass, OpError> {
        self.inner.classify(reference)
    }

    fn box_clone(&self) -> Box<dyn OnePassword> {
        Box::new(self.clone())
    }
}

fn cached_account(id: &str) -> OpAccount {
    OpAccount {
        id: id.into(),
        email: format!("user@{id}"),
        state: OpAccountState::Available,
        vaults: vec![],
    }
}

#[test]
fn a_cache_shared_with_the_backend_lends_the_accounts_to_the_picker() {
    let cache = Arc::new(Mutex::new(vec![cached_account("cached.1password.com")]));
    let mut world = world_for(Scenario::AccountsMixed);
    world.op = Box::new(Cached {
        inner: SimOnePassword::fixture(0),
        cache: cache.clone(),
    });
    let (mut app, backend) = held(world, Scenario::AccountsMixed);

    open_picker(&mut app);
    assert_eq!(
        backend.op_scopes(),
        [OpScope::Vaults {
            account: "cached.1password.com".into()
        }],
        "one account in the cache: the picker starts at its vaults"
    );

    press(&mut app, "esc");
    press(&mut app, "esc");
    cache
        .lock()
        .expect("cache")
        .push(cached_account("later.1password.com"));
    open_picker(&mut app);
    assert_eq!(
        backend.op_scopes().last(),
        Some(&OpScope::Accounts),
        "the backend added an account: the picker starts at the accounts"
    );
}

fn duplicated_titles_account() -> OpAccount {
    let field = OpField {
        id: "f1".into(),
        label: "key".into(),
        section: None,
        kind: jackin_console::backend::FieldKind::Concealed,
        material: Some("anthropic:valid".into()),
        tail: "1234".into(),
    };
    let item = |id: &str| OpItem {
        id: id.into(),
        title: "api".into(),
        category: "API credential",
        fields: vec![field.clone()],
    };
    let vault = |id: &str| OpVault {
        id: id.into(),
        name: "Shared".into(),
        access: VaultAccess::ReadWrite,
        items: vec![item("i1"), item("i2")],
    };
    OpAccount {
        id: "dup.1password.com".into(),
        email: "dup@example.test".into(),
        state: OpAccountState::Available,
        vaults: vec![vault("v1"), vault("v2")],
    }
}

#[test]
fn the_picker_keeps_the_id_of_the_row_chosen_when_titles_repeat() {
    let mut world = world_for(Scenario::AccountsMixed);
    let mut op = SimOnePassword::fixture(0);
    op.accounts = vec![duplicated_titles_account()];
    world.op = Box::new(op);
    let (mut app, backend) = held(world, Scenario::AccountsMixed);

    open_picker(&mut app);
    deliver(
        &mut app,
        &backend,
        OpListing::Vaults(
            ["v1", "v2"]
                .map(|id| OpVaultRow {
                    id: id.into(),
                    name: "Shared".into(),
                    items: 2,
                    access: None,
                })
                .to_vec(),
        ),
    );
    press(&mut app, "down");
    press(&mut app, "enter");
    deliver(
        &mut app,
        &backend,
        OpListing::Items(
            ["i1", "i2"]
                .map(|id| OpItemRow {
                    id: id.into(),
                    title: "api".into(),
                    category: "API credential".into(),
                    fields: Some(1),
                })
                .to_vec(),
        ),
    );
    press(&mut app, "down");
    press(&mut app, "enter");
    assert_eq!(
        backend.op_scopes().last(),
        Some(&OpScope::Fields {
            account: "dup.1password.com".into(),
            vault: "v2".into(),
            item: "i2".into(),
        }),
        "the second vault and the second item, not the first ones with the same title"
    );
    deliver(
        &mut app,
        &backend,
        OpListing::Fields(vec![jackin_console::backend::OpFieldRow {
            id: "f1".into(),
            label: "key".into(),
            section: None,
            kind: jackin_console::backend::FieldKind::Concealed,
        }]),
    );
    press(&mut app, "enter");
    let shown = screen(&mut app);
    assert!(shown.contains("op://v2/i2/f1"), "{shown}");
}

fn two_sections_account() -> OpAccount {
    let field = |section: &str| OpField {
        id: "k".into(),
        label: "key".into(),
        section: Some(section.into()),
        kind: jackin_console::backend::FieldKind::Concealed,
        material: Some("anthropic:valid".into()),
        tail: "1234".into(),
    };
    OpAccount {
        id: "sections.1password.com".into(),
        email: "sections@example.test".into(),
        state: OpAccountState::Available,
        vaults: vec![OpVault {
            id: "v1".into(),
            name: "Shared".into(),
            access: VaultAccess::ReadWrite,
            items: vec![OpItem {
                id: "i1".into(),
                title: "api".into(),
                category: "API credential",
                fields: vec![field("Production"), field("Staging")],
            }],
        }],
    }
}

#[test]
fn the_picker_names_the_section_of_the_field_chosen_when_labels_repeat() {
    let mut world = world_for(Scenario::AccountsMixed);
    let mut op = SimOnePassword::fixture(0);
    op.accounts = vec![two_sections_account()];
    world.op = Box::new(op);
    let (mut app, backend) = held(world, Scenario::AccountsMixed);

    open_picker(&mut app);
    deliver(
        &mut app,
        &backend,
        OpListing::Vaults(vec![OpVaultRow {
            id: "v1".into(),
            name: "Shared".into(),
            items: 1,
            access: None,
        }]),
    );
    press(&mut app, "enter");
    deliver(
        &mut app,
        &backend,
        OpListing::Items(vec![OpItemRow {
            id: "i1".into(),
            title: "api".into(),
            category: "API credential".into(),
            fields: Some(2),
        }]),
    );
    press(&mut app, "enter");
    deliver(
        &mut app,
        &backend,
        OpListing::Fields(
            ["Production", "Staging"]
                .map(|section| jackin_console::backend::OpFieldRow {
                    id: "k".into(),
                    label: "key".into(),
                    section: Some(section.into()),
                    kind: jackin_console::backend::FieldKind::Concealed,
                })
                .to_vec(),
        ),
    );
    let rows = screen(&mut app);
    assert!(rows.contains("Production · concealed"), "{rows}");
    assert!(rows.contains("Staging · concealed"), "{rows}");
    press(&mut app, "down");
    press(&mut app, "enter");

    let shown = screen(&mut app);
    assert!(shown.contains("op://v1/i1/Staging/k"), "{shown}");
}

#[test]
fn a_reference_is_described_and_classified_by_its_own_section() {
    let mut op = SimOnePassword::fixture(0);
    let mut account = two_sections_account();
    let [production, staging] = account.vaults[0].items[0].fields.as_mut_slice() else {
        panic!("two fields expected");
    };
    production.material = Some("anthropic:valid-PROD".into());
    production.tail = "PROD".into();
    staging.material = Some("anthropic:throttled-STAG".into());
    staging.tail = "STAG".into();
    op.accounts = vec![account];
    let op: &dyn OnePassword = &op;
    let reference = |section| {
        op.reference("sections.1password.com", "v1", "i1", "k", Some(section))
            .expect("a reference")
    };
    let (production, staging) = (reference("Production"), reference("Staging"));

    assert!(
        op.describe(&production)
            .expect("described")
            .masked
            .ends_with("PROD")
    );
    assert!(
        op.describe(&staging)
            .expect("described")
            .masked
            .ends_with("STAG")
    );
    let outcome = |reference| match op.classify(reference).expect("classified") {
        SecretClass::Key { outcome, .. } => outcome,
        other => panic!("a key expected, got {other:?}"),
    };
    assert_eq!(outcome(&production), KeyOutcome::Valid);
    assert_eq!(outcome(&staging), KeyOutcome::RateLimited);
}

// Gap 5: a change set says how many files it left out and which it did not
// read, and the Vocabulary words both.

fn view_changes(app: &mut App) {
    press(app, "f10");
    click_text(app, "View");
    click_text(app, "Inspect changes");
}

fn limited_changes() -> ChangeSet {
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
        omitted: 37,
        unread: vec![UnreadFile {
            path: "assets/big.bin".into(),
            status: DiffStatus::Added,
        }],
    }
}

fn changes_screen(vocabulary: Vocabulary) -> String {
    let mut world = world_for(Scenario::CapsuleMulti);
    world.surfaces.vocabulary = vocabulary;
    let (mut app, backend) = held(world, Scenario::CapsuleMulti);
    view_changes(&mut app);
    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Ok(limited_changes()),
    });
    screen(&mut app)
}

#[test]
fn the_changes_name_what_they_left_out_in_the_vocabulary_words() {
    let demo = changes_screen(Vocabulary::demo());
    assert!(demo.contains("canary.rs"), "{demo}");
    assert!(demo.contains("37 more not listed"), "{demo}");
    assert!(
        line_of(&demo, "big.bin").contains("not read"),
        "an unread file is listed with the word:\n{demo}"
    );

    let live = changes_screen(words());
    assert!(
        live.contains("37 more untracked files not listed"),
        "{live}"
    );
    assert!(
        line_of(&live, "big.bin").contains("content not read"),
        "{live}"
    );

    let mut canary = words();
    canary.changes_omitted = "CANARY {count} skipped".into();
    canary.changes_unread = "CANARY unread".into();
    let shown = changes_screen(canary);
    assert!(shown.contains("CANARY 37 skipped"), "{shown}");
    assert!(
        line_of(&shown, "big.bin").contains("CANARY unread"),
        "{shown}"
    );
}

#[test]
fn a_change_set_that_only_omits_or_leaves_unread_is_not_empty() {
    let none = ChangeSet {
        files: vec![],
        unpushed: 0,
        omitted: 0,
        unread: vec![],
    };
    assert!(none.is_empty());
    assert!(
        !ChangeSet {
            omitted: 5,
            ..none.clone()
        }
        .is_empty()
    );
    assert!(
        !ChangeSet {
            unread: vec![UnreadFile {
                path: "a.bin".into(),
                status: DiffStatus::Added,
            }],
            ..none
        }
        .is_empty()
    );
}

#[test]
fn the_sim_changes_omit_nothing_and_leave_nothing_unread() {
    let mut app = app(Scenario::CapsuleMulti);
    view_changes(&mut app);
    ticks(&mut app, 4);
    let shown = screen(&mut app);
    assert!(shown.contains("Inspect changes ·"), "{shown}");
    assert!(!shown.contains("not listed"), "{shown}");
    assert!(!shown.contains("not read"), "{shown}");
}

// Gap 6: the stagger of a refresh stays in the effect.

#[test]
fn refreshing_every_account_staggers_the_requests() {
    let (mut app, backend) = held(world_for(Scenario::AccountsMixed), Scenario::AccountsMixed);
    press(&mut app, "c r");
    let staggers = backend.staggers();
    assert!(staggers.len() > 2, "{staggers:?}");
    assert_eq!(staggers[0], 0);
    assert!(
        staggers.windows(2).all(|w| w[0] < w[1]) && staggers.iter().all(|s| s % 160 == 0),
        "{staggers:?}"
    );
}
