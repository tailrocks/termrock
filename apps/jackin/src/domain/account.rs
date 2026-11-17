use termrock::core::refusal::Reason;

use super::agent::{Agent, Provider, UsageSurface};
use super::fixed::Fixed;
use super::onepassword::OpReference;
use super::reason::IssueReason;
use super::usage::{AccountUsage, Freshness};
use super::vocabulary::{SourceWords, Vocabulary};
use super::workspace::{Usability, usability_of};

pub type AccountId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditBlocked {
    pub text: String,
    pub short: String,
}

impl EditBlocked {
    pub fn new(text: &str, short: &str) -> Self {
        Self {
            text: text.to_owned(),
            short: short.to_owned(),
        }
    }

    pub fn reason(&self) -> Reason {
        Reason::new(&self.text).short(&self.short)
    }
}

/// Why an account cannot launch. The words are `Fixed` data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchBlock {
    UsageOnlySource,
}

impl LaunchBlock {
    pub const ALL: [LaunchBlock; 1] = [Self::UsageOnlySource];

    pub fn text(self, fixed: Option<&Fixed>) -> String {
        match (self, fixed) {
            (Self::UsageOnlySource, Some(f)) => f.launch_usage_only.clone(),
            (Self::UsageOnlySource, None) => "unavailable".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountOrigin {
    Registered,

    Discovered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    OnePassword(OpReference),
    LocalFolder {
        path: String,
        detected: DetectedKind,
    },
    PlainApiKey {
        fingerprint: String,

        tail: String,
    },
    HostEnv {
        var: String,
        detected: DetectedKind,
    },
}

impl CredentialSource {
    pub fn origin_label(&self) -> &'static str {
        match self {
            CredentialSource::OnePassword(_) => "1Password",
            CredentialSource::LocalFolder { .. } => "Local folder",
            CredentialSource::PlainApiKey { .. } => "API key",
            CredentialSource::HostEnv { .. } => "Host env",
        }
    }

    pub fn safe_detail(&self) -> String {
        match self {
            CredentialSource::OnePassword(r) => r.display_path(),
            CredentialSource::LocalFolder { path, .. } => path.clone(),
            CredentialSource::PlainApiKey { fingerprint, tail } => {
                format!("…{tail} · fingerprint {fingerprint}")
            }
            CredentialSource::HostEnv { var, .. } => format!("${var}"),
        }
    }
}

pub fn masked(tail: &str) -> String {
    format!("••••••••…{tail}")
}

pub fn fingerprint(value: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in value.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:08x}", (h >> 32) as u32 ^ h as u32)
}

pub fn tail_of(value: &str) -> String {
    let n = value.chars().count();
    value.chars().skip(n.saturating_sub(4)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedKind {
    ClaudeOAuthProfile,
    ClaudeApiKeyEnv,
    CodexAuthJson,
    GrokAuthJson,
    OpenCodeGoAuthJson,
    AmpSecrets,
    ZaiApiKeyEnv,
    KimiApiKeyEnv,
    MinimaxTokenEnv,
    Unknown,
}

impl DetectedKind {
    pub fn label(self) -> &'static str {
        match self {
            DetectedKind::ClaudeOAuthProfile => "Claude OAuth profile",
            DetectedKind::ClaudeApiKeyEnv => "Claude API key env",
            DetectedKind::CodexAuthJson => "Codex auth.json",
            DetectedKind::GrokAuthJson => "Grok auth.json",
            DetectedKind::OpenCodeGoAuthJson => "OpenCode Go auth.json",
            DetectedKind::AmpSecrets => "Amp secrets.json",
            DetectedKind::ZaiApiKeyEnv => "Z.AI API key env",
            DetectedKind::KimiApiKeyEnv => "Kimi API key env",
            DetectedKind::MinimaxTokenEnv => "MiniMax token env",
            DetectedKind::Unknown => "unrecognised",
        }
    }

    pub fn provider(self) -> Option<Provider> {
        match self {
            DetectedKind::ClaudeOAuthProfile | DetectedKind::ClaudeApiKeyEnv => {
                Some(Provider::Anthropic)
            }
            DetectedKind::CodexAuthJson => Some(Provider::OpenAi),
            DetectedKind::GrokAuthJson => Some(Provider::XAi),
            DetectedKind::OpenCodeGoAuthJson => Some(Provider::OpenCode),
            DetectedKind::AmpSecrets => Some(Provider::Amp),
            DetectedKind::ZaiApiKeyEnv => Some(Provider::Zai),
            DetectedKind::KimiApiKeyEnv => Some(Provider::Moonshot),
            DetectedKind::MinimaxTokenEnv => Some(Provider::MiniMax),
            DetectedKind::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySubject {
    Handle(String),
}

impl IdentitySubject {
    pub fn label(&self) -> &str {
        match self {
            IdentitySubject::Handle(s) => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountIdentity {
    pub subject: Option<IdentitySubject>,
    pub plan: Option<String>,
}

impl AccountIdentity {
    pub fn label(&self) -> String {
        match &self.subject {
            Some(s) => s.label().to_owned(),
            None => "unresolved identity".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Provenance {
    ConfiguredSource,
    LiveHost,
}

impl Provenance {
    pub fn label(self) -> &'static str {
        match self {
            Provenance::ConfiguredSource => "configured source",
            Provenance::LiveHost => "live host",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Authoritative,
    Estimated,
    PresenceOnly,
    None,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Authoritative => "authoritative",
            Confidence::Estimated => "estimated",
            Confidence::PresenceOnly => "presence only",
            Confidence::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Available,
    AgentUninitialized,
    NeedsLogin,
    NeedsSecret,
    Unsupported,
    Unavailable,
    Error,
}

impl Lifecycle {
    pub fn label(self) -> &'static str {
        match self {
            Lifecycle::Available => "available",
            Lifecycle::AgentUninitialized => "agent uninitialized",
            Lifecycle::NeedsLogin => "needs login",
            Lifecycle::NeedsSecret => "needs secret",
            Lifecycle::Unsupported => "unsupported",
            Lifecycle::Unavailable => "unavailable",
            Lifecycle::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationLevel {
    MaterialDiscovered,
    IdentityAuthenticated,
    QuotaReadable,
}

impl ValidationLevel {
    pub fn label(self) -> &'static str {
        match self {
            ValidationLevel::MaterialDiscovered => "material discovered",
            ValidationLevel::IdentityAuthenticated => "identity authenticated",
            ValidationLevel::QuotaReadable => "quota readable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationState {
    NeverValidated,
    Validating { started_tick: u64 },
    Valid(ValidationLevel),
    Invalid(RecoverableIssue),
}

impl ValidationState {
    pub fn label(&self, vocabulary: &Vocabulary) -> String {
        match self {
            ValidationState::NeverValidated => "never validated".into(),
            ValidationState::Validating { .. } => "validating…".into(),
            ValidationState::Valid(l) => l.label().to_owned(),
            ValidationState::Invalid(i) => vocabulary.reason(&i.reason),
        }
    }

    pub fn level(&self) -> Option<ValidationLevel> {
        match self {
            ValidationState::Valid(l) => Some(*l),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueCode {
    QuotaUnsupported,
    FolderMissing,
    FolderUnreadable,
    FolderWrongProvider,
    CredentialFileMissing,
    CredentialMalformed,
    ApiKeyEmpty,
    ApiKeyInvalid,
    OpLocked,
    OpAuthorizationRequired,
    OpPermissionDenied,
    OpItemMissing,
    OpFieldMissing,
    OpProviderMismatch,
    Unauthorized,
    RateLimited,
    ProviderUnavailable,
    Stale,
    IdentityUnresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recoverability {
    Retryable,
    ActionRequired,
    Unsupported,
}

impl Recoverability {
    pub fn label(self) -> &'static str {
        match self {
            Recoverability::Retryable => "retryable",
            Recoverability::ActionRequired => "action required",
            Recoverability::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverableIssue {
    pub code: IssueCode,
    pub reason: IssueReason,
    pub detail: Option<String>,
    pub recoverability: Recoverability,
    pub retry_secs: Option<i64>,
}

impl RecoverableIssue {
    pub fn new(code: IssueCode, reason: IssueReason, rec: Recoverability) -> Self {
        Self {
            code,
            reason,
            detail: None,
            recoverability: rec,
            retry_secs: None,
        }
    }
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
    pub fn retry(mut self, secs: i64) -> Self {
        self.retry_secs = Some(secs);
        self
    }

    pub fn is_informational(&self) -> bool {
        matches!(self.code, IssueCode::Stale | IssueCode::IdentityUnresolved)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub label: String,
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub origin: AccountOrigin,
    pub display_name: String,
    pub agent: Option<Agent>,
    pub provider: Provider,
    pub surface: UsageSurface,
    pub source: CredentialSource,
    pub identity: AccountIdentity,
    pub provenance: Vec<Provenance>,
    pub confidence: Confidence,
    pub lifecycle: Lifecycle,
    pub purpose: Option<String>,
    pub enabled: bool,
    pub default_for_provider: bool,
    pub validation: ValidationState,
    pub last_refresh_secs: Option<i64>,
    pub issue: Option<RecoverableIssue>,

    pub endpoint: Option<Endpoint>,
    pub usage: AccountUsage,

    pub launch_blocked: Option<LaunchBlock>,
    pub edit_blocked: Option<EditBlocked>,
}

/// The id of an account the backend has not stored yet. The backend gives
/// the account its id and names it in the reply.
pub const UNSAVED_ID: &str = "";

impl Account {
    pub fn is_unsaved(&self) -> bool {
        self.id == UNSAVED_ID
    }

    pub fn registered(id: &str, name: &str, provider: Provider, source: CredentialSource) -> Self {
        Self {
            id: id.to_owned(),
            origin: AccountOrigin::Registered,
            display_name: name.to_owned(),
            agent: provider.agent(),
            provider,
            surface: provider.usage_surface(),
            source,
            identity: AccountIdentity::default(),
            provenance: vec![Provenance::ConfiguredSource],
            confidence: Confidence::None,
            lifecycle: Lifecycle::AgentUninitialized,
            purpose: None,
            enabled: true,
            default_for_provider: false,
            validation: ValidationState::NeverValidated,
            last_refresh_secs: None,
            issue: None,
            endpoint: None,
            usage: AccountUsage::none(),
            launch_blocked: None,
            edit_blocked: None,
        }
    }

    pub fn discovered(id: &str, name: &str, provider: Provider, source: CredentialSource) -> Self {
        let mut a = Self::registered(id, name, provider, source);
        a.origin = AccountOrigin::Discovered;
        a.provenance = vec![Provenance::LiveHost];
        a
    }

    pub fn with_endpoint(mut self, label: &str, host: &str) -> Self {
        if self.provider.supports_endpoint() {
            self.endpoint = Some(Endpoint {
                label: label.to_owned(),
                host: host.to_owned(),
            });
        }
        self
    }

    pub fn mutations_allowed(&self) -> bool {
        self.origin == AccountOrigin::Registered
    }

    pub fn edit_allowed(&self) -> bool {
        self.mutations_allowed() && self.edit_blocked.is_none()
    }

    pub fn title(&self) -> String {
        format!("{} · {}", self.surface.surface_name(), self.display_name)
    }

    pub fn status_word(&self) -> &'static str {
        if !self.enabled {
            return "disabled";
        }
        match self.usage.freshness.phase {
            Freshness::Refreshing => return "refreshing",
            Freshness::Failed if self.issue.is_some() => {}
            _ => {}
        }
        match self.lifecycle {
            Lifecycle::NeedsLogin => return "needs login",
            Lifecycle::NeedsSecret => return "needs secret",
            Lifecycle::Unsupported => return "unsupported",
            Lifecycle::Unavailable => return "unavailable",
            Lifecycle::Error => return "error",
            Lifecycle::AgentUninitialized => return "uninitialized",
            Lifecycle::Available => {}
        }
        match self.usage.worst_status() {
            Some(super::usage::QuotaStatus::Exhausted) => "exhausted",
            Some(super::usage::QuotaStatus::Warning) => "warning",
            _ => match self.usage.freshness.phase {
                Freshness::Stale => "stale",
                Freshness::Failed => "failed",
                _ => "ok",
            },
        }
    }

    pub fn is_error_state(&self) -> bool {
        matches!(
            self.lifecycle,
            Lifecycle::Error
                | Lifecycle::NeedsLogin
                | Lifecycle::NeedsSecret
                | Lifecycle::Unavailable
        ) || self.usage.freshness.phase == Freshness::Failed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateProbe {
    Folder {
        provider: Provider,
        path: String,
    },
    OpReference {
        canonical: String,
        account: String,
    },
    KeyFingerprint {
        provider: Provider,
        fingerprint: String,
    },
    HostEnv {
        provider: Provider,
        var: String,
    },
    Identity {
        surface: UsageSurface,
        subject: IdentitySubject,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountRegistry {
    pub accounts: Vec<Account>,
    pub revision: u64,
}

impl AccountRegistry {
    pub fn get(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    pub fn by_provider(&self, p: Provider) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(move |a| a.provider == p)
    }

    pub fn default_for(&self, p: Provider) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|a| a.provider == p && a.default_for_provider && a.enabled)
    }

    pub fn discovered_current(&self, p: Provider) -> Option<&Account> {
        self.accounts.iter().find(|a| {
            a.provider == p
                && a.origin == AccountOrigin::Discovered
                && a.enabled
                && a.lifecycle == Lifecycle::Available
        })
    }

    /// The account of the provider that is ready, when exactly one is: D26's
    /// last rule, "the only account".
    pub fn only_ready(&self, p: Provider) -> Option<&Account> {
        let mut ready = self
            .by_provider(p)
            .filter(|a| usability_of(a) == Usability::Ready);
        let only = ready.next()?;
        ready.next().is_none().then_some(only)
    }

    pub fn find_duplicate(&self, probe: &DuplicateProbe) -> Option<&Account> {
        self.accounts.iter().find(|a| match probe {
            DuplicateProbe::Folder { provider, path } => {
                a.provider == *provider
                    && a.origin == AccountOrigin::Registered
                    && matches!(&a.source, CredentialSource::LocalFolder { path: p, .. } if p == path)
            }
            DuplicateProbe::OpReference { canonical, account } => {
                matches!(&a.source, CredentialSource::OnePassword(r) if r.canonical() == *canonical && r.account == *account)
            }
            DuplicateProbe::KeyFingerprint {
                provider,
                fingerprint,
            } => {
                a.provider == *provider
                    && matches!(&a.source, CredentialSource::PlainApiKey { fingerprint: f, .. } if f == fingerprint)
            }
            DuplicateProbe::HostEnv { provider, var } => {
                a.provider == *provider
                    && matches!(&a.source, CredentialSource::HostEnv { var: v, .. } if v == var)
            }
            DuplicateProbe::Identity { surface, subject } => {
                a.surface == *surface && a.identity.subject.as_ref() == Some(subject)
            }
        })
    }

    pub fn name_taken(&self, provider: Provider, name: &str, except: Option<&str>) -> bool {
        self.accounts.iter().any(|a| {
            a.provider == provider
                && a.display_name.eq_ignore_ascii_case(name)
                && Some(a.id.as_str()) != except
        })
    }

    pub fn set_default(&mut self, id: &str) -> Result<(), String> {
        let Some(target) = self.get(id) else {
            return Err("account not found".into());
        };
        if !target.enabled {
            return Err("a disabled account cannot be the provider default".into());
        }
        let p = target.provider;
        for a in &mut self.accounts {
            if a.provider == p {
                a.default_for_provider = a.id == id;
            }
        }
        self.revision += 1;
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Option<Account> {
        let i = self.accounts.iter().position(|a| a.id == id)?;
        self.revision += 1;
        Some(self.accounts.remove(i))
    }

    pub fn insert(&mut self, account: Account) {
        self.accounts.push(account);
        self.revision += 1;
    }

    pub fn sorted(&self) -> Vec<&Account> {
        let mut v: Vec<&Account> = self.accounts.iter().collect();
        v.sort_by(|a, b| {
            a.surface
                .cmp(&b.surface)
                .then(b.default_for_provider.cmp(&a.default_for_provider))
                .then(a.display_name.cmp(&b.display_name))
        });
        v
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    OnePassword,
    LocalFolder,
    HostEnv,
    PlainApiKey,
}

impl CredentialSource {
    pub fn kind(&self) -> Option<SourceKind> {
        match self {
            CredentialSource::OnePassword(_) => Some(SourceKind::OnePassword),
            CredentialSource::LocalFolder { .. } => Some(SourceKind::LocalFolder),
            CredentialSource::PlainApiKey { .. } => Some(SourceKind::PlainApiKey),
            CredentialSource::HostEnv { .. } => Some(SourceKind::HostEnv),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChoice {
    pub provider: Provider,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChoice {
    pub kind: SourceKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountForm {
    pub providers: Vec<ProviderChoice>,
    pub sources: Vec<SourceChoice>,
    pub default_source: Option<usize>,
    pub op_note: Option<String>,
}

fn provider_choices(providers: &[Provider]) -> Vec<ProviderChoice> {
    providers
        .iter()
        .map(|&provider| ProviderChoice {
            provider,
            label: match provider.agent() {
                Some(agent) => format!("{} · {}", agent.label(), provider.label()),
                None => provider.label().to_owned(),
            },
        })
        .collect()
}

fn source_choices(sources: &[(SourceKind, &str)]) -> Vec<SourceChoice> {
    sources
        .iter()
        .map(|&(kind, label)| SourceChoice {
            kind,
            label: label.into(),
        })
        .collect()
}

/// The form of the demo: four providers.
impl Default for AccountForm {
    fn default() -> Self {
        Self {
            providers: provider_choices(&[
                Provider::Anthropic,
                Provider::OpenAi,
                Provider::XAi,
                Provider::OpenCode,
            ]),
            sources: source_choices(&[
                (
                    SourceKind::OnePassword,
                    "1Password item / field  (recommended)",
                ),
                (SourceKind::LocalFolder, "Local agent folder"),
                (SourceKind::PlainApiKey, "Plain-text API key"),
            ]),
            default_source: None,
            op_note: None,
        }
    }
}

impl AccountForm {
    /// Only a local Codex login can launch, so the local folder is the
    /// recommended source and the preselected one. A host variable is the way
    /// to add a key the console may not type.
    pub fn openshell() -> Self {
        Self {
            providers: provider_choices(&Provider::ALL),
            sources: source_choices(&[
                (
                    SourceKind::OnePassword,
                    "1Password item / field  (usage only)",
                ),
                (SourceKind::LocalFolder, "Local agent folder  (recommended)"),
                (SourceKind::PlainApiKey, "Plain-text API key"),
                (
                    SourceKind::HostEnv,
                    &format!("{}  (usage only)", SourceWords::openshell().env_label),
                ),
            ]),
            default_source: Some(1),
            op_note: Some("Usage only · launching Codex needs a local login".into()),
        }
    }
}
