use super::agent::Agent;
use super::workspace::WorkspaceId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId {
    pub gateway: String,
    pub sandbox_name: String,
}

impl InstanceId {
    pub fn new(gateway: impl Into<String>, sandbox_name: impl Into<String>) -> Self {
        Self {
            gateway: gateway.into(),
            sandbox_name: sandbox_name.into(),
        }
    }

    pub fn short_label(&self) -> String {
        let split = self.sandbox_name.len().saturating_sub(4);
        let suffix = self.sandbox_name.get(split..).unwrap_or(&self.sandbox_name);
        format!("{ID_PREFIX}{suffix}")
    }

    pub fn short_suffix(&self) -> String {
        self.short_label()
            .strip_prefix(ID_PREFIX)
            .unwrap_or(&self.sandbox_name)
            .to_owned()
    }

    pub fn row_key(&self) -> String {
        format!(
            "{}:{}{}:{}",
            self.gateway.len(),
            self.gateway,
            self.sandbox_name.len(),
            self.sandbox_name
        )
    }

    pub(crate) fn from_row_key(key: &str) -> Option<Self> {
        let gateway_length_end = key.find(':')?;
        let gateway_length = key[..gateway_length_end].parse::<usize>().ok()?;
        let gateway_start = gateway_length_end.checked_add(1)?;
        let gateway_end = gateway_start.checked_add(gateway_length)?;
        let gateway = key.get(gateway_start..gateway_end)?;
        let sandbox_length_end = key
            .get(gateway_end..)?
            .find(':')?
            .checked_add(gateway_end)?;
        let sandbox_length = key[gateway_end..sandbox_length_end].parse::<usize>().ok()?;
        let sandbox_start = sandbox_length_end.checked_add(1)?;
        let sandbox_end = sandbox_start.checked_add(sandbox_length)?;
        if sandbox_end != key.len() {
            return None;
        }
        Some(Self::new(gateway, key.get(sandbox_start..sandbox_end)?))
    }

    pub fn simulated(label: impl Into<String>) -> Self {
        Self::new("jackin-local", label)
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.short_label())
    }
}

/// What every instance id starts with. The console shows ids without it.
pub const ID_PREFIX: &str = "jk-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxPhase {
    Provisioning,
    Starting,
    Ready,
    Stopping,
    Stopped,
    Completed { exit_code: i32 },
    Error { exit_code: Option<i32> },
    Deleting,
    Suspending,
    Suspended,
    Unknown,
}

impl SandboxPhase {
    pub fn label(&self) -> String {
        match self {
            Self::Provisioning => "provisioning".into(),
            Self::Starting => "starting".into(),
            Self::Ready => "ready".into(),
            Self::Stopping => "stopping".into(),
            Self::Stopped => "stopped".into(),
            Self::Completed { exit_code } => format!("completed · exit {exit_code}"),
            Self::Error {
                exit_code: Some(exit_code),
            } => format!("error · exit {exit_code}"),
            Self::Error { exit_code: None } => "error".into(),
            Self::Deleting => "deleting".into(),
            Self::Suspending => "suspending".into(),
            Self::Suspended => "suspended".into(),
            Self::Unknown => "unknown".into(),
        }
    }

    pub fn ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn startable(&self) -> bool {
        matches!(self, Self::Stopped)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstanceStatus {
    Running,
    CleanExited,
    Crashed,
    PreservedDirty,
    PreservedUnpushed,
    RestoreAvailable,
    Superseded,
    Purged,
    FailedSetup,
}

impl InstanceStatus {
    pub fn label(self) -> &'static str {
        match self {
            InstanceStatus::Running => "running",
            InstanceStatus::CleanExited => "exited",
            InstanceStatus::Crashed => "crashed",
            InstanceStatus::PreservedDirty => "preserved · dirty",
            InstanceStatus::PreservedUnpushed => "preserved · unpushed",
            InstanceStatus::RestoreAvailable => "restore available",
            InstanceStatus::Superseded => "superseded",
            InstanceStatus::Purged => "purged",
            InstanceStatus::FailedSetup => "failed setup",
        }
    }

    pub fn hidden(self) -> bool {
        matches!(self, InstanceStatus::Superseded | InstanceStatus::Purged)
    }

    pub fn is_live(self) -> bool {
        self == InstanceStatus::Running
    }

    pub fn reconnectable(self) -> bool {
        matches!(
            self,
            InstanceStatus::Running
                | InstanceStatus::RestoreAvailable
                | InstanceStatus::PreservedDirty
                | InstanceStatus::PreservedUnpushed
                | InstanceStatus::Crashed
        )
    }

    pub fn stoppable(self) -> bool {
        self == InstanceStatus::Running
    }

    pub fn dirty(self) -> bool {
        matches!(
            self,
            InstanceStatus::PreservedDirty | InstanceStatus::PreservedUnpushed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub full_name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub id: InstanceId,

    pub container: String,
    pub workspace: Option<WorkspaceId>,
    pub workdir: Option<String>,
    pub role: Option<String>,
    pub agent: Agent,
    pub status: InstanceStatus,
    pub created_secs: i64,
    pub last_seen_secs: i64,
    pub run_id: String,

    pub sessions: Result<Vec<SessionRecord>, ManifestError>,

    pub daemon: DaemonSnapshot,

    pub branch: Option<String>,
    pub pr: Option<(u32, String)>,
    pub default_branch: String,
    pub github: Option<RepoRef>,

    pub uncommitted: usize,
    pub unpushed: usize,

    pub account: Option<super::account::AccountId>,
    pub accounts: Vec<super::account::AccountId>,
    pub agents: Vec<AgentInstanceView>,
    pub task: Option<TaskView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableInstance {
    pub id: InstanceId,
    pub workspace: Option<WorkspaceId>,
    pub workspace_name: Option<String>,
    pub role: Option<String>,
    pub phase: SandboxPhase,
    pub created_secs: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableDisplay {
    pub sandbox_name: String,
    pub short_suffix: String,
    pub gateway: String,
    pub workspace_name: String,
    pub role: String,
    pub reason: String,
}

impl UnavailableInstance {
    pub fn new(
        id: InstanceId,
        workspace: Option<WorkspaceId>,
        workspace_name: Option<String>,
        role: Option<String>,
        phase: SandboxPhase,
        created_secs: Option<i64>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id,
            workspace,
            workspace_name,
            role,
            phase,
            created_secs,
            reason: reason.into(),
        }
    }

    pub fn display(&self) -> UnavailableDisplay {
        UnavailableDisplay {
            sandbox_name: display_context(&self.id.sandbox_name, 64, "sandbox name unavailable"),
            short_suffix: display_context(&self.id.short_suffix(), 16, "sandbox unavailable"),
            gateway: display_context(&self.id.gateway, 40, "gateway unavailable"),
            workspace_name: display_context(
                self.workspace_name.as_deref().unwrap_or_default(),
                48,
                "workspace unavailable",
            ),
            role: display_context(
                self.role.as_deref().unwrap_or_default(),
                40,
                "role unavailable",
            ),
            reason: display_context(&self.reason, 112, "agent metadata could not be verified"),
        }
    }
}

fn display_context(value: &str, max_cells: usize, fallback: &str) -> String {
    use unicode_segmentation::UnicodeSegmentation;

    let mut safe = String::new();
    let mut cells = 0usize;
    let mut codepoints = 0;
    let mut pending_space = false;
    let mut truncated = false;
    let codepoint_limit = max_cells.saturating_mul(8).saturating_add(32);
    for grapheme in value.graphemes(true) {
        let mut piece = String::new();
        for ch in grapheme.chars() {
            codepoints += 1;
            if codepoints > codepoint_limit {
                truncated = true;
                break;
            }
            if is_display_control(ch) {
                continue;
            }
            if ch.is_control() || ch.is_whitespace() {
                pending_space = true;
                continue;
            }
            if pending_space && (!safe.is_empty() || !piece.is_empty()) {
                piece.push(' ');
            }
            pending_space = false;
            piece.push(ch);
        }
        if codepoints > codepoint_limit {
            break;
        }
        if piece.is_empty() {
            continue;
        }
        let piece_cells = termrock::ui::text::width(&piece);
        if cells.saturating_add(piece_cells) > max_cells {
            truncated = true;
            break;
        }
        safe.push_str(&piece);
        cells += piece_cells;
    }
    if safe.is_empty() {
        return fallback.to_owned();
    }
    if contains_sensitive_marker(&safe) {
        return fallback.to_owned();
    }
    if truncated {
        let prefix = termrock::ui::text::clip(&safe, max_cells.saturating_sub(1));
        if prefix.is_empty() {
            fallback.to_owned()
        } else {
            format!("{prefix}…")
        }
    } else {
        safe.trim_end().to_owned()
    }
}

fn is_display_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn contains_sensitive_marker(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["sk-", "ghp_", "github_pat_", "xoxb-", "op://"]
        .iter()
        .any(|marker| {
            value.match_indices(marker).any(|(index, _)| {
                index == 0
                    || !value.as_bytes()[index - 1].is_ascii_alphanumeric()
                        && value.as_bytes()[index - 1] != b'_'
            })
        })
        || ["api_key=", "api-key=", "token=", "password=", "secret="]
            .iter()
            .any(|marker| value.contains(marker))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceEntry {
    Available(Box<Instance>),
    Unavailable(UnavailableInstance),
}

impl InstanceEntry {
    pub fn available(instance: Instance) -> Self {
        Self::Available(Box::new(instance))
    }

    pub fn id(&self) -> &InstanceId {
        match self {
            Self::Available(instance) => &instance.id,
            Self::Unavailable(instance) => &instance.id,
        }
    }

    pub fn workspace_id(&self) -> Option<WorkspaceId> {
        match self {
            Self::Available(instance) => instance.workspace,
            Self::Unavailable(instance) => instance.workspace,
        }
    }

    pub fn as_available(&self) -> Option<&Instance> {
        match self {
            Self::Available(instance) => Some(instance),
            Self::Unavailable(_) => None,
        }
    }

    pub fn as_available_mut(&mut self) -> Option<&mut Instance> {
        match self {
            Self::Available(instance) => Some(instance),
            Self::Unavailable(_) => None,
        }
    }

    pub fn short_label(&self) -> String {
        match self {
            Self::Available(instance) => instance.id.short_label(),
            Self::Unavailable(instance) => instance.display().sandbox_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInstanceView {
    pub agent: Agent,
    pub provider: super::agent::Provider,
    pub account: Option<super::account::AccountId>,
    pub model: String,
    pub branch: Option<String>,
    pub state: AgentState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    pub text: String,
    pub statuses: Vec<InstanceStatusView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceStatusView {
    pub agent: Agent,
    pub text: String,
}

impl Instance {
    pub fn role_label(&self) -> &str {
        self.role.as_deref().unwrap_or("role unavailable")
    }

    pub fn workdir_label(&self) -> &str {
        self.workdir.as_deref().unwrap_or("workdir unavailable")
    }

    pub fn container_id(&self) -> String {
        format!("{}-{}", self.container, self.id.short_suffix())
    }

    pub fn is_dirty(&self) -> bool {
        self.uncommitted > 0 || self.unpushed > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    ReadError,
}

impl ManifestError {
    pub fn label(self) -> &'static str {
        match self {
            ManifestError::ReadError => "Sessions unavailable (manifest read error)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub agent: Option<Agent>,
    pub label: String,
    pub status: SessionStatus,
    pub started_secs: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Exited(i32),
    Crashed,
}

impl SessionStatus {
    pub fn label(self) -> String {
        match self {
            SessionStatus::Active => "active".into(),
            SessionStatus::Exited(0) => "exited".into(),
            SessionStatus::Exited(code) => format!("exited {code}"),
            SessionStatus::Crashed => "crashed".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonSnapshot {
    Unavailable,

    NoTabs,
    Tabs(Vec<TabSnapshot>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSnapshot {
    pub label: String,
    pub active: bool,
    pub panes: Vec<PaneSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSnapshot {
    pub label: String,
    pub agent: Option<Agent>,
    pub state: AgentState,
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    Idle,
    Working,
    Done,
    Blocked,
    Unknown,
}

impl AgentState {
    pub fn label(self) -> &'static str {
        match self {
            AgentState::Idle => "idle",
            AgentState::Working => "working",
            AgentState::Done => "done",
            AgentState::Blocked => "blocked",
            AgentState::Unknown => "unknown",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            AgentState::Blocked => 4,
            AgentState::Done => 3,
            AgentState::Working => 2,
            AgentState::Idle => 1,
            AgentState::Unknown => 0,
        }
    }
}
