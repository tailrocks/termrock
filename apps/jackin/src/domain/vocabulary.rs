use super::agent::Provider;
use super::docker::EngineEndpoint;
use super::doctor::DoctorFailure;
use super::instance::InstanceStatus;
use super::reason::{Found, IssueReason, RefreshOutcome, UsageReason};
use crate::arbiter::DiscoveryError;
use crate::backend::OpError;
use crate::panes::{FailNext, LinkFault, PaneRefusal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusWords {
    pub running: String,
    pub clean_exited: String,
    pub crashed: String,
    pub preserved_dirty: String,
    pub preserved_unpushed: String,
    pub restore_available: String,
    pub superseded: String,
    pub purged: String,
    pub failed_setup: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonWords {
    pub inspect_label: String,
    pub inspect_unavailable: String,
    pub no_tabs: String,
    pub unavailable_running: String,
    pub unavailable_stopped: String,
    pub topology_source: String,
    pub topology_unavailable: String,
    pub card_unavailable: String,
    pub card_no_tabs: String,
    pub list_tabs: String,
    pub list_no_tabs: String,
    pub list_unavailable: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleWords {
    pub restored: String,
    pub purge_refused_next: Option<String>,
    pub stop_body: String,
    pub purge_action: String,
    pub purge_scope: String,
    pub copy_dirty: String,
    pub copy_clean: String,
    pub risk_dirty: String,
    pub risk_clean: String,
    pub purged: String,
}

/// What a build cannot do yet. A backend that lacks one names it, and the
/// vocabulary words the refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unbuilt {
    SaveWorkspace,
    SaveSettings,
    SaveAccounts,
    DeleteWorkspace,
    RestoreSandbox,
    TrustRole,
    PrewarmImage,
    StopSandbox,
    PurgeSandbox,
    RefreshUsage,
    ValidateAccount,
    LoadRole,
    CheckRole,
    SaveRole,
    BindRole,
    AddRole,
    RemoveRole,
    Doctor,
    Launch,
    Pane,
    AddGitRole,
}

impl Unbuilt {
    fn subject(self, noun: &str) -> String {
        match self {
            Unbuilt::SaveWorkspace => "Saving a workspace".to_owned(),
            Unbuilt::SaveSettings => "Saving settings".to_owned(),
            Unbuilt::SaveAccounts => "Saving accounts".to_owned(),
            Unbuilt::DeleteWorkspace => "Deleting a workspace".to_owned(),
            Unbuilt::RestoreSandbox => format!("Restoring a {noun}"),
            Unbuilt::TrustRole => "Trusting a role source".to_owned(),
            Unbuilt::PrewarmImage => "Building an image".to_owned(),
            Unbuilt::StopSandbox => format!("Stopping a {noun}"),
            Unbuilt::PurgeSandbox => format!("Purging a {noun}"),
            Unbuilt::RefreshUsage => "Reading usage".to_owned(),
            Unbuilt::ValidateAccount => "Validating an account".to_owned(),
            Unbuilt::LoadRole => "Loading a role".to_owned(),
            Unbuilt::CheckRole => "Checking a role".to_owned(),
            Unbuilt::SaveRole => "Saving a role".to_owned(),
            Unbuilt::BindRole => "Binding a role".to_owned(),
            Unbuilt::AddRole => "Adding a role".to_owned(),
            Unbuilt::RemoveRole => "Removing a role".to_owned(),
            Unbuilt::Doctor => "The doctor".to_owned(),
            Unbuilt::Launch => format!("Launching a {noun}"),
            Unbuilt::Pane => "A terminal pane".to_owned(),
            Unbuilt::AddGitRole => "Adding a role from a Git URL".to_owned(),
        }
    }
}

/// A fact a refresh found that is not an error: something the console lists
/// but could not read in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    GatewayUnreachable { gateway: String, reason: String },
    UnreadableLabels { count: usize },
}

/// A field of a record that a backend cannot store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveField {
    Workdir,
    Mounts,
    KeepAwake,
    GitPull,
    DirtyPolicy,
    DisabledDefaults,
    EnabledAccounts,
    RoleAccounts,
    CoauthorTrailer,
    DcoSignoff,
    GlobalMounts,
    GlobalRoleVariables,
    AgentModes,
    Trust,
    AccountName,
    AccountProvider,
    Purpose,
    Endpoint,
    EnvSource,
    ApiKey,
    Agent,
    RoleSource,
    RoleName,
}

impl SaveField {
    pub fn label(self) -> &'static str {
        match self {
            SaveField::Workdir => "workdir",
            SaveField::Mounts => "mounts",
            SaveField::KeepAwake => "keep_awake",
            SaveField::GitPull => "git_pull",
            SaveField::DirtyPolicy => "dirty_policy",
            SaveField::DisabledDefaults => "disabled_defaults",
            SaveField::EnabledAccounts => "enabled accounts",
            SaveField::RoleAccounts => "role accounts",
            SaveField::CoauthorTrailer => "coauthor_trailer",
            SaveField::DcoSignoff => "dco_signoff",
            SaveField::GlobalMounts => "global mounts",
            SaveField::GlobalRoleVariables => "global role variables",
            SaveField::AgentModes => "agent modes",
            SaveField::Trust => "trust",
            SaveField::AccountName => "account name",
            SaveField::AccountProvider => "account provider",
            SaveField::Purpose => "purpose",
            SaveField::Endpoint => "endpoint",
            SaveField::EnvSource => "env source",
            SaveField::ApiKey => "api key",
            SaveField::Agent => "agent",
            SaveField::RoleSource => "role source",
            SaveField::RoleName => "role name",
        }
    }
}

/// Why a backend would not store or find something. The backend names the
/// case, and the vocabulary words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    CannotSave(SaveField),
    NoProject,
    NotAFolder { folder: String },
    NoRoleLeft,
    NotInRoster { instance: String },
    UnknownRole { role: String },
    RoleNotLoaded,
}

/// What a terminal pane says about itself. Templates take `{agent}` (the
/// agent's label, or `shell_label`), `{code}`, `{reason}`, `{lines}`,
/// `{bytes}`, `{source}` and `{chars}`. `failed_next` joins the failed note
/// (`{failed}`) to the typed next step (`{next}`); `main_open` is the new-tab
/// picker's reason for a refused agent (`{agent}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneWords {
    pub waiting_title: String,
    pub waiting_hint: String,
    pub attaching_title: String,
    pub attaching_hint: String,
    pub unattached_title: String,
    pub too_small: String,
    pub viewing_suffix: String,
    pub viewing: String,
    pub detached: String,
    pub connection_lost: String,
    pub exited: String,
    pub exited_unknown_code: String,
    pub failed: String,
    pub failed_next: String,
    pub next_doctor: String,
    pub stream_closed: String,
    pub shell_label: String,
    pub paste_confirm_title: String,
    pub paste_confirm_body: String,
    pub paste_confirm_button: String,
    pub paste_too_large: String,
    pub paste_full: String,
    pub pasted: String,
    pub send_prefix: String,
    pub title_label: String,
    pub copied_from: String,
    pub clip_write_blocked: String,
    pub clip_read_blocked: String,
    pub clip_too_large: String,
    pub main_open: String,
}

impl PaneWords {
    pub fn openshell() -> Self {
        Self {
            waiting_title: "Waiting for room".into(),
            waiting_hint: "{agent} starts when the pane is big enough".into(),
            attaching_title: "Attaching to {agent}…".into(),
            attaching_hint: "Ctrl+B x cancels".into(),
            unattached_title: "{agent} is not attached".into(),
            too_small: "Pane too small · zoom with Ctrl+B z".into(),
            viewing_suffix: "(viewing)".into(),
            viewing: "Read-only · another client controls this pane".into(),
            detached: "Detached from {agent} · press Enter to reattach".into(),
            connection_lost: "Gateway connection lost · press Enter to reattach".into(),
            exited:
                "{agent} exited (code {code}) · press Enter to restart · the sandbox keeps running"
                    .into(),
            exited_unknown_code: "unknown".into(),
            failed: "Press Enter to retry · could not attach to {agent}: {reason}".into(),
            failed_next: "{failed} · {next}".into(),
            next_doctor: "run jackin doctor".into(),
            stream_closed: "the connection closed before the pane attached".into(),
            shell_label: "Shell".into(),
            paste_confirm_title: "Paste {lines}?".into(),
            paste_confirm_body:
                "This pane has no paste protection, so each line runs as you paste it.".into(),
            paste_confirm_button: "Paste".into(),
            paste_too_large: "Paste too large · {bytes} bytes".into(),
            paste_full: "Paste not sent · the pane is busy, try again".into(),
            pasted: "Pasted into the focused pane".into(),
            send_prefix: "Send Ctrl+B".into(),
            title_label: "Title".into(),
            copied_from: "from {source} · {chars}".into(),
            clip_write_blocked: "Clipboard write from {source} blocked · select text to copy it"
                .into(),
            clip_read_blocked: "Clipboard read from {source} blocked".into(),
            clip_too_large: "Clipboard write from {source} blocked · too large".into(),
            main_open: "One {agent} pane per sandbox".into(),
        }
    }

    /// The failed note: the key first, then the host's reason, then the next
    /// step when the fault names one.
    pub fn failure(&self, agent: &str, fault: &LinkFault) -> String {
        let reason = if fault.reason.is_empty() {
            &self.stream_closed
        } else {
            &fault.reason
        };
        let failed = fill_in(&self.failed, &[("agent", agent), ("reason", reason)]);
        match fault.next {
            FailNext::None => failed,
            FailNext::RunDoctor => fill_in(
                &self.failed_next,
                &[("failed", &failed), ("next", &self.next_doctor)],
            ),
        }
    }

    pub fn refusal(&self, refusal: &PaneRefusal) -> String {
        match refusal {
            PaneRefusal::MainProcessOpen { agent } => {
                fill_in(&self.main_open, &[("agent", agent.label())])
            }
        }
    }

    pub fn fields(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("pane.waiting_title", &self.waiting_title),
            ("pane.waiting_hint", &self.waiting_hint),
            ("pane.attaching_title", &self.attaching_title),
            ("pane.attaching_hint", &self.attaching_hint),
            ("pane.unattached_title", &self.unattached_title),
            ("pane.too_small", &self.too_small),
            ("pane.viewing_suffix", &self.viewing_suffix),
            ("pane.viewing", &self.viewing),
            ("pane.detached", &self.detached),
            ("pane.connection_lost", &self.connection_lost),
            ("pane.exited", &self.exited),
            ("pane.exited_unknown_code", &self.exited_unknown_code),
            ("pane.failed", &self.failed),
            ("pane.failed_next", &self.failed_next),
            ("pane.next_doctor", &self.next_doctor),
            ("pane.stream_closed", &self.stream_closed),
            ("pane.shell_label", &self.shell_label),
            ("pane.paste_confirm_title", &self.paste_confirm_title),
            ("pane.paste_confirm_body", &self.paste_confirm_body),
            ("pane.paste_confirm_button", &self.paste_confirm_button),
            ("pane.paste_too_large", &self.paste_too_large),
            ("pane.paste_full", &self.paste_full),
            ("pane.pasted", &self.pasted),
            ("pane.send_prefix", &self.send_prefix),
            ("pane.title_label", &self.title_label),
            ("pane.copied_from", &self.copied_from),
            ("pane.clip_write_blocked", &self.clip_write_blocked),
            ("pane.clip_read_blocked", &self.clip_read_blocked),
            ("pane.clip_too_large", &self.clip_too_large),
            ("pane.main_open", &self.main_open),
        ]
    }
}

/// One reason in two places: the line of an issue and the text of a check
/// row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    pub issue: String,
    pub check: String,
}

fn phrase(issue: &str, check: &str) -> Phrase {
    Phrase {
        issue: issue.into(),
        check: check.into(),
    }
}

/// What the console says about a credential or a usage reading that did not
/// work. The templates take the facts of the matching `IssueReason`, `Found`
/// or `RefreshOutcome`: `{name}`, `{path}`, `{file}`, `{detail}`, `{provider}`,
/// `{found}`, `{expected}`, `{status}`, `{age}`, `{wait}`, `{account}`,
/// `{vault}`, `{item}`, `{field}`, `{reference}`, `{kind}`, `{line}`, `{who}`,
/// `{plan}` and `{count}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonWords {
    pub unknown_provider: String,
    pub skipped: Phrase,
    pub env_invalid: Phrase,
    pub env_missing: Phrase,
    pub env_empty: Phrase,
    pub bad_reference: Phrase,
    pub file_missing: Phrase,
    pub file_not_regular: Phrase,
    pub folder_missing: Phrase,
    pub folder_unreadable: Phrase,
    pub login_missing: Phrase,
    pub login_invalid: Phrase,
    pub wrong_provider: Phrase,
    pub key_empty: Phrase,
    pub key_rejected: Phrase,
    pub key_revoked: Phrase,
    pub key_wrong_provider: Phrase,
    pub quota_hidden: Phrase,
    pub quota_denied: Phrase,
    pub rejected: Phrase,
    pub provider_down: Phrase,
    pub broker_down: Phrase,
    pub stale: Phrase,
    pub identity_unresolved: Phrase,
    pub presence_only: Phrase,
    pub usage_no_credentials: Phrase,
    pub usage_expired_login: Phrase,
    pub usage_unsupported: Phrase,
    pub usage_rate_limited: Phrase,
    pub usage_rate_limited_open: Phrase,
    pub usage_network: Phrase,
    pub usage_provider: Phrase,
    pub op_locked: String,
    pub op_not_available: String,
    pub op_not_signed_in: String,
    pub op_access_denied: String,
    pub op_account_missing: String,
    pub op_vault_missing: String,
    pub op_item_missing: String,
    pub op_field_missing: String,
    pub op_field_empty: String,
    pub op_field_plain: String,
    pub found_reference_resolved: String,
    pub found_key_present: String,
    pub found_detected: String,
    pub found_unparsable: String,
    pub found_authenticated: String,
    pub found_authenticated_bare: String,
    pub found_windows: String,
    pub found_env: String,
    pub found_file: String,
    pub found_folder: String,
    pub found_login: String,
    pub outcome_current: String,
    pub outcome_stale: String,
    pub outcome_failed: String,
    pub outcome_still_rate_limited: String,
    pub outcome_provider_unavailable: String,
    pub outcome_needs_attention: String,
    pub outcome_broker_unreachable: String,
    pub outcome_op_locked: String,
}

impl ReasonWords {
    pub fn openshell() -> Self {
        Self {
            unknown_provider: "unknown".into(),
            skipped: phrase("not attempted", "not attempted"),
            env_invalid: phrase(
                "Not a valid variable name · enter the name, not its value",
                "Not a valid variable name",
            ),
            env_missing: phrase(
                "That variable is not set in this environment",
                "Not set in this environment",
            ),
            env_empty: phrase("That variable is set but empty", "Set but empty"),
            bad_reference: phrase(
                "{reference} is not a 1Password field reference",
                "Not a field reference",
            ),
            file_missing: phrase("File not found: {path}", "No file at the path"),
            file_not_regular: phrase("Not a regular file: {path}", "Not a regular file"),
            folder_missing: phrase("Folder not found: {path}", "Nothing exists at the path"),
            folder_unreadable: phrase(
                "Folder not readable: permission denied for {path}",
                "{detail}",
            ),
            login_missing: phrase(
                "No credential found: {path} has no {file}",
                "Run the agent login first",
            ),
            login_invalid: phrase(
                "Credential unreadable: {file} is not valid JSON",
                "{file} is not valid JSON",
            ),
            wrong_provider: phrase(
                "Folder belongs to another provider: found a {found} profile, expected {expected}",
                "Found a {found} profile",
            ),
            key_empty: phrase("API key required: the field is empty", "Field is empty"),
            key_rejected: phrase(
                "Key rejected: the provider returned 401",
                "Not authenticated (401)",
            ),
            key_revoked: phrase(
                "Key rejected: the provider returned 401",
                "Not authenticated (401) · key rotated or revoked",
            ),
            key_wrong_provider: phrase(
                "Provider mismatch: the key belongs to {found}, expected {expected}",
                "Key class is {found}",
            ),
            quota_hidden: phrase(
                "Quota not visible: {provider} does not expose usage for API keys",
                "Quota not visible for API keys",
            ),
            quota_denied: phrase(
                "Quota not visible: {provider} returned {status}",
                "Quota not visible",
            ),
            rejected: phrase(
                "Not authorized: {provider} rejected the credential",
                "Not authenticated",
            ),
            provider_down: phrase(
                "Provider unavailable: {provider} did not respond",
                "{detail}",
            ),
            broker_down: phrase(
                "Usage broker unreachable: last good values kept",
                "Usage broker unreachable",
            ),
            stale: phrase(
                "Usage stale · last good {age} ago",
                "Stale · last good {age} ago",
            ),
            identity_unresolved: phrase(
                "Identity unresolved · showing usage without a public handle",
                "No stable public identity",
            ),
            presence_only: phrase("Confidence: presence only", "Confidence: presence only"),
            usage_no_credentials: phrase(
                "No credentials: nothing to read usage with",
                "No credentials",
            ),
            usage_expired_login: phrase("Login expired: sign in again", "Login expired"),
            usage_unsupported: phrase("Usage not supported for this account", "Not supported"),
            usage_rate_limited: phrase(
                "Rate limited: retry after {wait}",
                "Rate limited (429) · retry after {wait}",
            ),
            usage_rate_limited_open: phrase("Rate limited: retry later", "Rate limited (429)"),
            usage_network: phrase("Network error: {detail}", "{detail}"),
            usage_provider: phrase("Provider error: {detail}", "{detail}"),
            op_locked: "1Password locked: unlock the app and retry".into(),
            op_not_available: "1Password not available: the 1Password CLI could not be run".into(),
            op_not_signed_in: "1Password authorization required: sign in to {account}".into(),
            op_access_denied: "1Password access denied: no permission for vault {vault}".into(),
            op_account_missing: "1Password account not found: {account}".into(),
            op_vault_missing: "1Password vault not found: {vault}".into(),
            op_item_missing: "1Password item not found: {item} in {vault}".into(),
            op_field_missing: "1Password field not found: {field} in {item}".into(),
            op_field_empty: "API key required: field {field} is empty".into(),
            op_field_plain: "Field {field} is not a credential: choose a concealed field".into(),
            found_reference_resolved: "1Password reference resolved".into(),
            found_key_present: "Key material present".into(),
            found_detected: "Detected {kind}".into(),
            found_unparsable: "{file} found · parse failed at line {line}".into(),
            found_authenticated: "Authenticated as {who} · {plan}".into(),
            found_authenticated_bare: "Authenticated as {who}".into(),
            found_windows: "{count} windows readable".into(),
            found_env: "Set in this environment".into(),
            found_file: "File found at {path}".into(),
            found_folder: "Folder found at {path}".into(),
            found_login: "ChatGPT login for {account}".into(),
            outcome_current: "current".into(),
            outcome_stale: "stale".into(),
            outcome_failed: "failed".into(),
            outcome_still_rate_limited: "still rate limited".into(),
            outcome_provider_unavailable: "provider unavailable".into(),
            outcome_needs_attention: "needs attention".into(),
            outcome_broker_unreachable: "broker unreachable · last good kept".into(),
            outcome_op_locked: "failed · 1Password locked".into(),
        }
    }

    pub fn op_error(&self, error: &OpError) -> String {
        let r = self;
        match error {
            OpError::Locked => r.op_locked.clone(),
            OpError::NotAvailable => r.op_not_available.clone(),
            OpError::AuthorizationRequired { account } => {
                fill_in(&r.op_not_signed_in, &[("account", account)])
            }
            OpError::PermissionDenied { vault } => {
                fill_in(&r.op_access_denied, &[("vault", vault)])
            }
            OpError::MissingAccount { account } => {
                fill_in(&r.op_account_missing, &[("account", account)])
            }
            OpError::MissingVault { vault } => fill_in(&r.op_vault_missing, &[("vault", vault)]),
            OpError::MissingItem { item, vault } => {
                fill_in(&r.op_item_missing, &[("item", item), ("vault", vault)])
            }
            OpError::MissingField { field, item } => {
                fill_in(&r.op_field_missing, &[("field", field), ("item", item)])
            }
            OpError::EmptyMaterial { field } => fill_in(&r.op_field_empty, &[("field", field)]),
            OpError::WrongFieldShape { field } => fill_in(&r.op_field_plain, &[("field", field)]),
        }
    }

    pub fn fields(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("reason.unknown_provider", &self.unknown_provider),
            ("reason.skipped.issue", &self.skipped.issue),
            ("reason.skipped.check", &self.skipped.check),
            ("reason.env_invalid.issue", &self.env_invalid.issue),
            ("reason.env_invalid.check", &self.env_invalid.check),
            ("reason.env_missing.issue", &self.env_missing.issue),
            ("reason.env_missing.check", &self.env_missing.check),
            ("reason.env_empty.issue", &self.env_empty.issue),
            ("reason.env_empty.check", &self.env_empty.check),
            ("reason.bad_reference.issue", &self.bad_reference.issue),
            ("reason.bad_reference.check", &self.bad_reference.check),
            ("reason.file_missing.issue", &self.file_missing.issue),
            ("reason.file_missing.check", &self.file_missing.check),
            (
                "reason.file_not_regular.issue",
                &self.file_not_regular.issue,
            ),
            (
                "reason.file_not_regular.check",
                &self.file_not_regular.check,
            ),
            ("reason.folder_missing.issue", &self.folder_missing.issue),
            ("reason.folder_missing.check", &self.folder_missing.check),
            (
                "reason.folder_unreadable.issue",
                &self.folder_unreadable.issue,
            ),
            (
                "reason.folder_unreadable.check",
                &self.folder_unreadable.check,
            ),
            ("reason.login_missing.issue", &self.login_missing.issue),
            ("reason.login_missing.check", &self.login_missing.check),
            ("reason.login_invalid.issue", &self.login_invalid.issue),
            ("reason.login_invalid.check", &self.login_invalid.check),
            ("reason.wrong_provider.issue", &self.wrong_provider.issue),
            ("reason.wrong_provider.check", &self.wrong_provider.check),
            ("reason.key_empty.issue", &self.key_empty.issue),
            ("reason.key_empty.check", &self.key_empty.check),
            ("reason.key_rejected.issue", &self.key_rejected.issue),
            ("reason.key_rejected.check", &self.key_rejected.check),
            ("reason.key_revoked.issue", &self.key_revoked.issue),
            ("reason.key_revoked.check", &self.key_revoked.check),
            (
                "reason.key_wrong_provider.issue",
                &self.key_wrong_provider.issue,
            ),
            (
                "reason.key_wrong_provider.check",
                &self.key_wrong_provider.check,
            ),
            ("reason.quota_hidden.issue", &self.quota_hidden.issue),
            ("reason.quota_hidden.check", &self.quota_hidden.check),
            ("reason.quota_denied.issue", &self.quota_denied.issue),
            ("reason.quota_denied.check", &self.quota_denied.check),
            ("reason.rejected.issue", &self.rejected.issue),
            ("reason.rejected.check", &self.rejected.check),
            ("reason.provider_down.issue", &self.provider_down.issue),
            ("reason.provider_down.check", &self.provider_down.check),
            ("reason.broker_down.issue", &self.broker_down.issue),
            ("reason.broker_down.check", &self.broker_down.check),
            ("reason.stale.issue", &self.stale.issue),
            ("reason.stale.check", &self.stale.check),
            (
                "reason.identity_unresolved.issue",
                &self.identity_unresolved.issue,
            ),
            (
                "reason.identity_unresolved.check",
                &self.identity_unresolved.check,
            ),
            ("reason.presence_only.issue", &self.presence_only.issue),
            ("reason.presence_only.check", &self.presence_only.check),
            (
                "reason.usage_no_credentials.issue",
                &self.usage_no_credentials.issue,
            ),
            (
                "reason.usage_no_credentials.check",
                &self.usage_no_credentials.check,
            ),
            (
                "reason.usage_expired_login.issue",
                &self.usage_expired_login.issue,
            ),
            (
                "reason.usage_expired_login.check",
                &self.usage_expired_login.check,
            ),
            (
                "reason.usage_unsupported.issue",
                &self.usage_unsupported.issue,
            ),
            (
                "reason.usage_unsupported.check",
                &self.usage_unsupported.check,
            ),
            (
                "reason.usage_rate_limited.issue",
                &self.usage_rate_limited.issue,
            ),
            (
                "reason.usage_rate_limited.check",
                &self.usage_rate_limited.check,
            ),
            (
                "reason.usage_rate_limited_open.issue",
                &self.usage_rate_limited_open.issue,
            ),
            (
                "reason.usage_rate_limited_open.check",
                &self.usage_rate_limited_open.check,
            ),
            ("reason.usage_network.issue", &self.usage_network.issue),
            ("reason.usage_network.check", &self.usage_network.check),
            ("reason.usage_provider.issue", &self.usage_provider.issue),
            ("reason.usage_provider.check", &self.usage_provider.check),
            ("reason.op_locked", &self.op_locked),
            ("reason.op_not_available", &self.op_not_available),
            ("reason.op_not_signed_in", &self.op_not_signed_in),
            ("reason.op_access_denied", &self.op_access_denied),
            ("reason.op_account_missing", &self.op_account_missing),
            ("reason.op_vault_missing", &self.op_vault_missing),
            ("reason.op_item_missing", &self.op_item_missing),
            ("reason.op_field_missing", &self.op_field_missing),
            ("reason.op_field_empty", &self.op_field_empty),
            ("reason.op_field_plain", &self.op_field_plain),
            (
                "reason.found_reference_resolved",
                &self.found_reference_resolved,
            ),
            ("reason.found_key_present", &self.found_key_present),
            ("reason.found_detected", &self.found_detected),
            ("reason.found_unparsable", &self.found_unparsable),
            ("reason.found_authenticated", &self.found_authenticated),
            (
                "reason.found_authenticated_bare",
                &self.found_authenticated_bare,
            ),
            ("reason.found_windows", &self.found_windows),
            ("reason.found_env", &self.found_env),
            ("reason.found_file", &self.found_file),
            ("reason.found_folder", &self.found_folder),
            ("reason.found_login", &self.found_login),
            ("reason.outcome_current", &self.outcome_current),
            ("reason.outcome_stale", &self.outcome_stale),
            ("reason.outcome_failed", &self.outcome_failed),
            (
                "reason.outcome_still_rate_limited",
                &self.outcome_still_rate_limited,
            ),
            (
                "reason.outcome_provider_unavailable",
                &self.outcome_provider_unavailable,
            ),
            (
                "reason.outcome_needs_attention",
                &self.outcome_needs_attention,
            ),
            (
                "reason.outcome_broker_unreachable",
                &self.outcome_broker_unreachable,
            ),
            ("reason.outcome_op_locked", &self.outcome_op_locked),
        ]
    }
}

/// The words of the environment-variable source of the account form. The demo
/// form has no such source, so the demo shares these words with OpenShell, as
/// it shares `ReasonWords` and the `PaneWords` it does not rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWords {
    /// The radio choice and the field label.
    pub env_label: String,
    pub env_placeholder: String,
    pub env_help: String,
    /// The form error when the name is empty.
    pub env_empty: String,
}

impl SourceWords {
    pub fn openshell() -> Self {
        Self {
            env_label: "Environment variable".into(),
            env_placeholder: "variable name · never its value".into(),
            env_help: "Only the name is saved; the value stays in your environment".into(),
            env_empty: "Enter the name of the environment variable".into(),
        }
    }

    pub fn fields(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("source.env_label", &self.env_label),
            ("source.env_placeholder", &self.env_placeholder),
            ("source.env_help", &self.env_help),
            ("source.env_empty", &self.env_empty),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    pub construct: String,
    pub instances_where: String,
    pub still_inside: String,
    pub health_noun: String,
    pub capsule: String,
    pub version_label: String,
    pub container_noun: String,
    pub quit_lines: [String; 2],
    pub unexported_word: String,
    pub quit_dirty_keep: String,
    pub quit_dirty_discard: String,
    pub quit_discard_title: String,
    pub quit_discard_action: String,
    pub quit_discard_button: String,
    pub last_session_ended: String,
    pub never_reached: String,
    pub session_choice: String,
    pub status: StatusWords,
    pub daemon: DaemonWords,
    pub lifecycle: LifecycleWords,

    pub project_tab: String,
    pub project_one: String,
    pub project_many: String,
    pub project_card: String,
    pub first_use: String,
    pub role_access: String,
    pub destination_heading: String,
    pub git_url_help: [String; 2],
    pub read_only_label: String,

    pub github_workspace: String,
    pub github_git_mount: String,
    pub github_repo_mount: Option<String>,
    pub github_copies_url: bool,
    pub github_open_label: String,
    pub github_open_meta: String,
    pub github_copied: String,
    pub github_no_url: String,
    pub github_menu: String,
    pub trust_browser: String,
    pub trust_reveal: String,
    pub export_placeholder: String,
    pub trailer_sample: String,

    pub env_plain_label: String,
    pub env_plain_placeholder: String,
    pub env_plain_help: Option<String>,
    pub env_op_label: String,
    pub trust_hint: String,
    pub agent_no_credentials: String,

    pub op_reference_note: String,

    pub about_label: String,
    pub paste_label: String,
    pub paste_empty: String,
    pub paste_empty_reason: String,
    pub detail_unavailable: String,
    pub pane_starting: String,
    pub pane_cleared: String,
    pub close_tab_body: String,
    pub close_agent_body: String,
    pub close_shell_body: String,
    pub export_done: String,
    pub export_revealed: Option<String>,
    pub export_opened: Option<String>,
    pub host_log: Option<String>,
    pub workspace_config: Option<String>,
    pub global_config: Option<String>,
    pub save_error_workspace: String,
    pub save_error_global: String,
    pub save_error_next: String,
    pub sandbox_id: Option<String>,
    pub invocation_id: Option<String>,
    pub attached_status: String,

    pub not_built: String,
    pub gateway_unreachable: String,
    pub labels_unreadable: String,
    pub about_product: String,
    pub about_gateway: String,
    pub about_config: String,
    pub discovery_unreadable: String,
    pub engine_tls_port: String,
    pub doctor_failed: String,
    pub doctor_timed_out: String,
    pub doctor_no_gateway: String,

    pub cannot_save: String,
    pub no_project: String,
    pub not_a_folder: String,
    pub no_role_left: String,
    pub not_in_roster: String,
    pub unknown_role: String,
    pub role_not_loaded: String,

    pub changes_omitted: String,
    pub changes_unread: String,
    pub pane: PaneWords,
    pub reason: ReasonWords,
    pub source: SourceWords,
}

impl Vocabulary {
    pub fn openshell() -> Self {
        Self {
            construct: "the sandbox".into(),
            instances_where: "on the gateway".into(),
            still_inside: "Still running on the gateway".into(),
            health_noun: "gateway".into(),
            capsule: "Capsule".into(),
            version_label: "jackin".into(),
            container_noun: "Sandbox".into(),
            quit_lines: [
                "! Quitting detaches; the sandbox keeps running.".into(),
                "! Work stays in the sandbox until you export it.".into(),
            ],
            unexported_word: "unexported".into(),
            quit_dirty_keep: "Quit & keep the sandbox".into(),
            quit_dirty_discard: "Quit & delete the sandbox".into(),
            quit_discard_title: "Delete the sandbox and quit?".into(),
            quit_discard_action: "delete the sandbox and all work in it".into(),
            quit_discard_button: "Delete".into(),
            last_session_ended: "Last session ended · the sandbox keeps running".into(),
            never_reached: "never got ready".into(),
            session_choice: "Session choice · picked for this session only".into(),
            status: StatusWords {
                running: "The sandbox is running.".into(),
                clean_exited: "The final session ended cleanly; nothing to attach to.".into(),
                crashed: "The agent exited with an error; the sandbox is kept for inspection."
                    .into(),
                preserved_dirty: "Has uncommitted changes in the sandbox; kept until you decide."
                    .into(),
                preserved_unpushed: "Has commits that were never exported; kept until you decide."
                    .into(),
                restore_available: "Stopped cleanly; restore it to continue.".into(),
                superseded: "Replaced by a newer instance for the same Workspace.".into(),
                purged: "The sandbox was deleted.".into(),
                failed_setup: "Launch failed before the sandbox was ready; nothing to attach to."
                    .into(),
            },
            daemon: DaemonWords {
                inspect_label: "Capsule".into(),
                inspect_unavailable: "gateway unreachable".into(),
                no_tabs: "No Capsule tabs open".into(),
                unavailable_running: "Gateway unreachable — showing saved sessions".into(),
                unavailable_stopped: "No live Capsule (sandbox not running)".into(),
                topology_source: "gateway · {ago}".into(),
                topology_unavailable: "unreachable".into(),
                card_unavailable: "running {duration} · gateway unreachable".into(),
                card_no_tabs: "running {duration} · no Capsule tabs open".into(),
                list_tabs: "capsule · {tabs} · {panes}".into(),
                list_no_tabs: "capsule · no tabs".into(),
                list_unavailable: "gateway unreachable".into(),
            },
            lifecycle: LifecycleWords {
                restored: "Restored {id} · sandbox restarted".into(),
                purge_refused_next: Some("to delete it anyway, run jackin purge --force".into()),
                stop_body: "The sandbox stops and every session in it ends. You can restore it later.".into(),
                purge_action: "delete the sandbox".into(),
                purge_scope: "{sessions} · {copy}".into(),
                copy_dirty: "changed project copy".into(),
                copy_clean: "clean project copy".into(),
                risk_dirty: "uncommitted or unexported work is lost".into(),
                risk_clean: "the sandbox and its files are deleted".into(),
                purged: "Purged {id} · the sandbox was deleted".into(),
            },

            project_tab: "Project".into(),
            project_one: "project".into(),
            project_many: "projects".into(),
            project_card: "{source} · uploaded".into(),
            first_use: "{path} is uploaded to /sandbox/project. Enter launches with defaults; n creates a saved workspace.".into(),
            role_access: "The role can access the uploaded project files.".into(),
            destination_heading: "Project destination".into(),
            git_url_help: [
                "jackin uploads a folder from this machine.".into(),
                "Clone the repository first, then choose its folder.".into(),
            ],
            read_only_label: "Read-only".into(),

            github_workspace: "GitHub · {url}".into(),
            github_git_mount: "GitHub · https://{url}".into(),
            github_repo_mount: None,
            github_copies_url: true,
            github_open_label: "Copy URL".into(),
            github_open_meta: "Enter copies the row · y copies".into(),
            github_copied: "Copied the GitHub URL".into(),
            github_no_url: "No GitHub URL for this sandbox".into(),
            github_menu: "Show GitHub URL".into(),
            trust_browser: "Source · {source}".into(),
            trust_reveal: "Source · {source}".into(),
            export_placeholder: "path in /sandbox/project".into(),
            trailer_sample: "None · jackin adds no trailers to commits".into(),

            env_plain_label: "Plain text".into(),
            env_plain_placeholder: "type the value".into(),
            env_plain_help: Some("value is stored as plain text".into()),
            env_op_label: "1Password item / field".into(),
            trust_hint: "Show source".into(),
            agent_no_credentials: "no credentials handed to the sandbox".into(),

            op_reference_note:
                "only the reference is saved · read here for usage only".into(),

            about_label: "About jackin".into(),
            paste_label: "Paste last copy".into(),
            paste_empty: "Nothing copied in this console yet".into(),
            paste_empty_reason: "Nothing copied yet".into(),
            detail_unavailable: "Not supported".into(),
            pane_starting: "Starting…".into(),
            pane_cleared: "Scrollback cleared".into(),
            close_tab_body: "Closes every pane in this tab. Codex panes keep running; shells end."
                .into(),
            close_agent_body: "Codex keeps running in the sandbox. This pane detaches.".into(),
            close_shell_body: "The shell ends. Unsaved state in it is lost.".into(),
            export_done: "Exported to ~/Downloads/jackin/{workspace}/{path}".into(),
            export_revealed: None,
            export_opened: None,
            host_log: None,
            workspace_config: Some("{config}".into()),
            global_config: Some("{config}".into()),
            save_error_workspace: "Could not write {config}.".into(),
            save_error_global: "Could not write {config}.".into(),
            save_error_next: "check disk space and permissions, then Save again".into(),
            sandbox_id: None,
            invocation_id: None,
            attached_status: "Attached to {name}".into(),

            not_built: "{what} is not available in this build yet".into(),
            gateway_unreachable: "Could not list sandboxes on {gateway}: {reason}".into(),
            labels_unreadable: "{count} sandboxes have labels jackin cannot read".into(),
            about_product: "Product".into(),
            about_gateway: "Gateway".into(),
            about_config: "Config".into(),
            discovery_unreadable: "sandbox list unreadable".into(),
            engine_tls_port: "tls port {port}".into(),
            doctor_failed: "The checks could not run: {reason}".into(),
            doctor_timed_out: "no answer in time".into(),
            doctor_no_gateway: "no usable gateway".into(),

            cannot_save: "{field} cannot be saved".into(),
            no_project: "a workspace needs a project folder".into(),
            not_a_folder: "{folder} is not a folder jackin can use".into(),
            no_role_left: "no allowed role is left".into(),
            not_in_roster: "{id} is not in the roster".into(),
            unknown_role: "unknown role {role}".into(),
            role_not_loaded: "the role manifest is read when the role is loaded".into(),

            changes_omitted: "{count} more untracked files not listed".into(),
            changes_unread: "content not read".into(),
            pane: PaneWords::openshell(),
            reason: ReasonWords::openshell(),
            source: SourceWords::openshell(),
        }
    }

    pub fn fields(&self) -> Vec<(&'static str, &str)> {
        let (s, d, l) = (&self.status, &self.daemon, &self.lifecycle);
        vec![
            ("construct", self.construct.as_str()),
            ("instances_where", &self.instances_where),
            ("still_inside", &self.still_inside),
            ("health_noun", &self.health_noun),
            ("capsule", &self.capsule),
            ("version_label", &self.version_label),
            ("container_noun", &self.container_noun),
            ("quit_lines.0", &self.quit_lines[0]),
            ("quit_lines.1", &self.quit_lines[1]),
            ("unexported_word", &self.unexported_word),
            ("quit_dirty_keep", &self.quit_dirty_keep),
            ("quit_dirty_discard", &self.quit_dirty_discard),
            ("quit_discard_title", &self.quit_discard_title),
            ("quit_discard_action", &self.quit_discard_action),
            ("quit_discard_button", &self.quit_discard_button),
            ("last_session_ended", &self.last_session_ended),
            ("never_reached", &self.never_reached),
            ("session_choice", &self.session_choice),
            ("status.running", &s.running),
            ("status.clean_exited", &s.clean_exited),
            ("status.crashed", &s.crashed),
            ("status.preserved_dirty", &s.preserved_dirty),
            ("status.preserved_unpushed", &s.preserved_unpushed),
            ("status.restore_available", &s.restore_available),
            ("status.superseded", &s.superseded),
            ("status.purged", &s.purged),
            ("status.failed_setup", &s.failed_setup),
            ("daemon.inspect_label", &d.inspect_label),
            ("daemon.inspect_unavailable", &d.inspect_unavailable),
            ("daemon.no_tabs", &d.no_tabs),
            ("daemon.unavailable_running", &d.unavailable_running),
            ("daemon.unavailable_stopped", &d.unavailable_stopped),
            ("daemon.topology_source", &d.topology_source),
            ("daemon.topology_unavailable", &d.topology_unavailable),
            ("daemon.card_unavailable", &d.card_unavailable),
            ("daemon.card_no_tabs", &d.card_no_tabs),
            ("daemon.list_tabs", &d.list_tabs),
            ("daemon.list_no_tabs", &d.list_no_tabs),
            ("daemon.list_unavailable", &d.list_unavailable),
            ("lifecycle.restored", &l.restored),
            (
                "lifecycle.purge_refused_next",
                l.purge_refused_next.as_deref().unwrap_or(""),
            ),
            ("lifecycle.stop_body", &l.stop_body),
            ("lifecycle.purge_action", &l.purge_action),
            ("lifecycle.purge_scope", &l.purge_scope),
            ("lifecycle.copy_dirty", &l.copy_dirty),
            ("lifecycle.copy_clean", &l.copy_clean),
            ("lifecycle.risk_dirty", &l.risk_dirty),
            ("lifecycle.risk_clean", &l.risk_clean),
            ("lifecycle.purged", &l.purged),
            ("project_tab", &self.project_tab),
            ("project_one", &self.project_one),
            ("project_many", &self.project_many),
            ("project_card", &self.project_card),
            ("first_use", &self.first_use),
            ("role_access", &self.role_access),
            ("destination_heading", &self.destination_heading),
            ("git_url_help.0", &self.git_url_help[0]),
            ("git_url_help.1", &self.git_url_help[1]),
            ("read_only_label", &self.read_only_label),
            ("github_workspace", &self.github_workspace),
            ("github_git_mount", &self.github_git_mount),
            (
                "github_repo_mount",
                self.github_repo_mount.as_deref().unwrap_or(""),
            ),
            ("github_open_label", &self.github_open_label),
            ("github_open_meta", &self.github_open_meta),
            ("github_copied", &self.github_copied),
            ("github_no_url", &self.github_no_url),
            ("github_menu", &self.github_menu),
            ("trust_browser", &self.trust_browser),
            ("trust_reveal", &self.trust_reveal),
            ("export_placeholder", &self.export_placeholder),
            ("trailer_sample", &self.trailer_sample),
            ("env_plain_label", &self.env_plain_label),
            ("env_plain_placeholder", &self.env_plain_placeholder),
            (
                "env_plain_help",
                self.env_plain_help.as_deref().unwrap_or(""),
            ),
            ("env_op_label", &self.env_op_label),
            ("trust_hint", &self.trust_hint),
            ("agent_no_credentials", &self.agent_no_credentials),
            ("op_reference_note", &self.op_reference_note),
            ("about_label", &self.about_label),
            ("paste_label", &self.paste_label),
            ("paste_empty", &self.paste_empty),
            ("paste_empty_reason", &self.paste_empty_reason),
            ("detail_unavailable", &self.detail_unavailable),
            ("pane_starting", &self.pane_starting),
            ("pane_cleared", &self.pane_cleared),
            ("close_tab_body", &self.close_tab_body),
            ("close_agent_body", &self.close_agent_body),
            ("close_shell_body", &self.close_shell_body),
            ("export_done", &self.export_done),
            (
                "export_revealed",
                self.export_revealed.as_deref().unwrap_or(""),
            ),
            ("export_opened", self.export_opened.as_deref().unwrap_or("")),
            ("host_log", self.host_log.as_deref().unwrap_or("")),
            (
                "workspace_config",
                self.workspace_config.as_deref().unwrap_or(""),
            ),
            ("global_config", self.global_config.as_deref().unwrap_or("")),
            ("save_error_workspace", &self.save_error_workspace),
            ("save_error_global", &self.save_error_global),
            ("save_error_next", &self.save_error_next),
            ("sandbox_id", self.sandbox_id.as_deref().unwrap_or("")),
            ("invocation_id", self.invocation_id.as_deref().unwrap_or("")),
            ("attached_status", &self.attached_status),
            ("not_built", &self.not_built),
            ("gateway_unreachable", &self.gateway_unreachable),
            ("labels_unreadable", &self.labels_unreadable),
            ("about_product", &self.about_product),
            ("about_gateway", &self.about_gateway),
            ("about_config", &self.about_config),
            ("discovery_unreadable", &self.discovery_unreadable),
            ("engine_tls_port", &self.engine_tls_port),
            ("doctor_failed", &self.doctor_failed),
            ("doctor_timed_out", &self.doctor_timed_out),
            ("doctor_no_gateway", &self.doctor_no_gateway),
            ("cannot_save", &self.cannot_save),
            ("no_project", &self.no_project),
            ("not_a_folder", &self.not_a_folder),
            ("no_role_left", &self.no_role_left),
            ("not_in_roster", &self.not_in_roster),
            ("unknown_role", &self.unknown_role),
            ("role_not_loaded", &self.role_not_loaded),
            ("changes_omitted", &self.changes_omitted),
            ("changes_unread", &self.changes_unread),
        ]
        .into_iter()
        .chain(self.pane.fields())
        .chain(self.reason.fields())
        .chain(self.source.fields())
        .collect()
    }

    pub fn status_description(&self, status: InstanceStatus) -> &str {
        let s = &self.status;
        match status {
            InstanceStatus::Running => &s.running,
            InstanceStatus::CleanExited => &s.clean_exited,
            InstanceStatus::Crashed => &s.crashed,
            InstanceStatus::PreservedDirty => &s.preserved_dirty,
            InstanceStatus::PreservedUnpushed => &s.preserved_unpushed,
            InstanceStatus::RestoreAvailable => &s.restore_available,
            InstanceStatus::Superseded => &s.superseded,
            InstanceStatus::Purged => &s.purged,
            InstanceStatus::FailedSetup => &s.failed_setup,
        }
    }

    pub fn status_label(&self, status: InstanceStatus) -> String {
        match status {
            InstanceStatus::PreservedUnpushed => format!("preserved · {}", self.unexported_word),
            other => other.label().to_owned(),
        }
    }

    pub fn dirty_summary(&self, i: &super::instance::Instance) -> String {
        let word = &self.unexported_word;
        match (i.uncommitted, i.unpushed) {
            (0, 0) => "clean".into(),
            (u, 0) => format!("{u} uncommitted"),
            (0, p) => format!("{p} {word}"),
            (u, p) => format!("{u} uncommitted · {p} {word}"),
        }
    }

    pub fn count(&self, n: usize) -> String {
        termrock::ui::text::plural(n, &self.project_one, &self.project_many)
    }

    pub fn info_label(&self) -> String {
        format!("{} info", self.container_noun)
    }

    pub fn noun_lower(&self) -> String {
        self.container_noun.to_lowercase()
    }

    pub fn unbuilt(&self, what: Unbuilt) -> String {
        fill_in(
            &self.not_built,
            &[("what", &what.subject(&self.noun_lower()))],
        )
    }

    pub fn notice(&self, notice: &Notice) -> String {
        match notice {
            Notice::GatewayUnreachable { gateway, reason } => fill_in(
                &self.gateway_unreachable,
                &[("gateway", gateway), ("reason", reason)],
            ),
            Notice::UnreadableLabels { count } => {
                fill_in(&self.labels_unreadable, &[("count", &count.to_string())])
            }
        }
    }

    pub fn notices(&self, notices: &[Notice]) -> String {
        notices
            .iter()
            .map(|n| self.notice(n))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn discovery(&self, error: DiscoveryError) -> &str {
        match error {
            DiscoveryError::IndexUnreadable => &self.discovery_unreadable,
        }
    }

    pub fn engine_endpoint(&self, endpoint: &EngineEndpoint) -> String {
        match endpoint {
            EngineEndpoint::Address(address) => address.clone(),
            EngineEndpoint::TlsPort(port) => {
                fill_in(&self.engine_tls_port, &[("port", &port.to_string())])
            }
        }
    }

    pub fn rejection(&self, rejection: &Rejection) -> String {
        match rejection {
            Rejection::CannotSave(field) => fill_in(&self.cannot_save, &[("field", field.label())]),
            Rejection::NoProject => self.no_project.clone(),
            Rejection::NotAFolder { folder } => fill_in(&self.not_a_folder, &[("folder", folder)]),
            Rejection::NoRoleLeft => self.no_role_left.clone(),
            Rejection::NotInRoster { instance } => {
                fill_in(&self.not_in_roster, &[("id", instance)])
            }
            Rejection::UnknownRole { role } => fill_in(&self.unknown_role, &[("role", role)]),
            Rejection::RoleNotLoaded => self.role_not_loaded.clone(),
        }
    }

    pub fn op_error(&self, error: &OpError) -> String {
        self.reason.op_error(error)
    }

    pub fn reason(&self, reason: &IssueReason) -> String {
        self.worded(reason, |p| &p.issue)
    }

    pub fn check(&self, reason: &IssueReason) -> String {
        self.worded(reason, |p| &p.check)
    }

    fn provider_name(&self, provider: Option<Provider>) -> &str {
        match provider {
            Some(provider) => provider.short(),
            None => &self.reason.unknown_provider,
        }
    }

    fn worded(&self, reason: &IssueReason, side: fn(&Phrase) -> &String) -> String {
        let r = &self.reason;
        let say = |phrase: &Phrase, pairs: &[(&str, &str)]| fill_in(side(phrase), pairs);
        match reason {
            IssueReason::Skipped => say(&r.skipped, &[]),
            IssueReason::OnePassword(error) => self.op_error(error),
            IssueReason::EnvInvalid { name } => say(&r.env_invalid, &[("name", name)]),
            IssueReason::EnvMissing { name } => say(&r.env_missing, &[("name", name)]),
            IssueReason::EnvEmpty { name } => say(&r.env_empty, &[("name", name)]),
            IssueReason::BadReference { reference } => {
                say(&r.bad_reference, &[("reference", reference)])
            }
            IssueReason::FileMissing { path } => say(&r.file_missing, &[("path", path)]),
            IssueReason::FileNotRegular { path } => say(&r.file_not_regular, &[("path", path)]),
            IssueReason::FolderMissing { path } => say(&r.folder_missing, &[("path", path)]),
            IssueReason::FolderUnreadable { path, detail } => {
                say(&r.folder_unreadable, &[("path", path), ("detail", detail)])
            }
            IssueReason::LoginMissing { path, file } => {
                say(&r.login_missing, &[("path", path), ("file", file)])
            }
            IssueReason::LoginInvalid { file } => say(&r.login_invalid, &[("file", file)]),
            IssueReason::WrongProvider { found, expected } => say(
                &r.wrong_provider,
                &[
                    ("found", self.provider_name(*found)),
                    ("expected", expected.short()),
                ],
            ),
            IssueReason::KeyEmpty => say(&r.key_empty, &[]),
            IssueReason::KeyRejected => say(&r.key_rejected, &[]),
            IssueReason::KeyRevoked => say(&r.key_revoked, &[]),
            IssueReason::KeyWrongProvider { found, expected } => say(
                &r.key_wrong_provider,
                &[("found", found.short()), ("expected", expected.short())],
            ),
            IssueReason::Usage(usage) => self.usage_reason(usage, side),
            IssueReason::QuotaHidden { provider } => {
                say(&r.quota_hidden, &[("provider", provider.short())])
            }
            IssueReason::QuotaDenied { provider, status } => say(
                &r.quota_denied,
                &[
                    ("provider", provider.short()),
                    ("status", &status.to_string()),
                ],
            ),
            IssueReason::Rejected { provider } => {
                say(&r.rejected, &[("provider", provider.short())])
            }
            IssueReason::ProviderDown { provider, detail } => say(
                &r.provider_down,
                &[("provider", provider.short()), ("detail", detail)],
            ),
            IssueReason::BrokerDown => say(&r.broker_down, &[]),
            IssueReason::Stale { age_secs } => say(&r.stale, &[("age", &spoken(*age_secs))]),
            IssueReason::IdentityUnresolved => say(&r.identity_unresolved, &[]),
            IssueReason::PresenceOnly => say(&r.presence_only, &[]),
        }
    }

    fn usage_reason(&self, reason: &UsageReason, side: fn(&Phrase) -> &String) -> String {
        let r = &self.reason;
        let say = |phrase: &Phrase, pairs: &[(&str, &str)]| fill_in(side(phrase), pairs);
        match reason {
            UsageReason::NoCredentials => say(&r.usage_no_credentials, &[]),
            UsageReason::ExpiredLogin => say(&r.usage_expired_login, &[]),
            UsageReason::Unsupported => say(&r.usage_unsupported, &[]),
            UsageReason::RateLimited {
                retry_in_secs: Some(secs),
            } => say(&r.usage_rate_limited, &[("wait", &spoken(*secs))]),
            UsageReason::RateLimited {
                retry_in_secs: None,
            } => say(&r.usage_rate_limited_open, &[]),
            UsageReason::Network { detail } => say(&r.usage_network, &[("detail", detail)]),
            UsageReason::Provider { detail } => say(&r.usage_provider, &[("detail", detail)]),
        }
    }

    pub fn found(&self, found: &Found) -> String {
        let r = &self.reason;
        match found {
            Found::ReferenceResolved => r.found_reference_resolved.clone(),
            Found::KeyPresent => r.found_key_present.clone(),
            Found::Detected(kind) => fill_in(&r.found_detected, &[("kind", kind.label())]),
            Found::Unparsable { file, line } => fill_in(
                &r.found_unparsable,
                &[("file", file), ("line", &line.to_string())],
            ),
            Found::Authenticated {
                who,
                plan: Some(plan),
            } => fill_in(&r.found_authenticated, &[("who", who), ("plan", plan)]),
            Found::Authenticated { who, plan: None } => {
                fill_in(&r.found_authenticated_bare, &[("who", who)])
            }
            Found::WindowsReadable(count) => {
                fill_in(&r.found_windows, &[("count", &count.to_string())])
            }
            Found::EnvSet { name } => fill_in(&r.found_env, &[("name", name)]),
            Found::FileFound { path } => fill_in(&r.found_file, &[("path", path)]),
            Found::FolderFound { path } => fill_in(&r.found_folder, &[("path", path)]),
            Found::Login { account } => fill_in(&r.found_login, &[("account", account)]),
            Found::Level(level) => level.label().to_owned(),
        }
    }

    pub fn refresh_outcome(&self, outcome: RefreshOutcome) -> &str {
        let r = &self.reason;
        match outcome {
            RefreshOutcome::Current => &r.outcome_current,
            RefreshOutcome::Stale => &r.outcome_stale,
            RefreshOutcome::Failed => &r.outcome_failed,
            RefreshOutcome::StillRateLimited => &r.outcome_still_rate_limited,
            RefreshOutcome::ProviderUnavailable => &r.outcome_provider_unavailable,
            RefreshOutcome::NeedsAttention => &r.outcome_needs_attention,
            RefreshOutcome::BrokerUnreachable => &r.outcome_broker_unreachable,
            RefreshOutcome::OpLocked => &r.outcome_op_locked,
        }
    }

    pub fn doctor_failure(&self, failure: &DoctorFailure) -> String {
        match failure {
            DoctorFailure::TimedOut => self.doctor_timed_out.clone(),
            DoctorFailure::Failed(reason) => fill_in(&self.doctor_failed, &[("reason", reason)]),
        }
    }
}

fn spoken(secs: i64) -> String {
    match secs.max(0) {
        s @ 0..=59 => format!("{s} s"),
        s @ 60..=3_599 => format!("{} min", s / 60),
        s @ 3_600..=86_399 => format!("{} h", s / 3_600),
        s => format!("{} d", s / 86_400),
    }
}

pub fn fill_in(template: &str, pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .fold(template.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}
