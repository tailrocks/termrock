// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Permission and trust surfaces for terminal agents.
//!
//! TermRock owns presentation, focus, queue ordering, and stale-response
//! protection. Consumers own policy stores, process execution, and network I/O.
//!
//! **Law:** default focus is never a destructive Allow. Esc cancels without
//! granting. Confirming a stale (superseded) request is ignored.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode,
        KeyEvent,
        KeyEventKind,
        KeyModifiers,
        MouseButton,
        MouseEvent,
        MouseEventKind,
    },
    interaction::{
        OverlayId,
        OverlayKind,
        OverlayOutcome,
        OverlaySize,
        OverlaySpec,
        OverlayStack,
    },
    style::{
        Density,
        DesignTokens,
        Role,
        Theme,
    },
    text::{
        display_cols,
        take_display_cols,
    },
    widgets::{
        Panel,
        PanelEmphasis,
    },
};

/// Overlay id for agent permission / trust surfaces (`OverlayStack`).
pub const PERMISSION_OVERLAY_ID: &str = "termrock.permission";

// ── Provenance ──────────────────────────────────────────────────────────────

/// Who initiated the permission-gated action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InitiatorKind {
    /// Primary session agent.
    #[default]
    MainAgent,
    /// Nested subagent run.
    Subagent,
    /// Installed plugin / extension.
    Plugin,
    /// MCP server tool.
    McpServer,
    /// User-triggered explicit action.
    User,
    /// System / scheduler.
    System,
}

impl InitiatorKind {
    /// Short label for chrome.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MainAgent => "agent",
            Self::Subagent => "subagent",
            Self::Plugin => "plugin",
            Self::McpServer => "mcp",
            Self::User => "user",
            Self::System => "system",
        }
    }
}

/// One hop in the provenance chain (outer → inner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceHop {
    /// Kind of actor.
    pub kind: InitiatorKind,
    /// Stable id (agent run, subagent id, server name).
    pub id: String,
    /// Human label.
    pub label: String,
}

impl ProvenanceHop {
    /// Convenience constructor.
    #[must_use]
    pub fn new(kind: InitiatorKind, id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Full provenance: who asked, nested through subagents/MCP.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PermissionProvenance {
    /// Chain from outermost (main) to innermost (leaf initiator).
    pub chain: Vec<ProvenanceHop>,
}

impl PermissionProvenance {
    /// Single main-agent hop.
    #[must_use]
    pub fn main_agent(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            chain: vec![ProvenanceHop::new(InitiatorKind::MainAgent, id, label)],
        }
    }

    /// Append a hop (e.g. subagent, then MCP).
    #[must_use]
    pub fn push(mut self, hop: ProvenanceHop) -> Self {
        self.chain.push(hop);
        self
    }

    /// Leaf initiator (last hop), if any.
    #[must_use]
    pub fn leaf(&self) -> Option<&ProvenanceHop> {
        self.chain.last()
    }

    /// Whether any hop is a subagent.
    #[must_use]
    pub fn has_subagent(&self) -> bool {
        self.chain.iter().any(|h| h.kind == InitiatorKind::Subagent)
    }

    /// Compact display: `agent > sub:review > mcp:fs`.
    #[must_use]
    pub fn display_path(&self) -> String {
        self.chain
            .iter()
            .map(|h| format!("{}:{}", h.kind.label(), h.label))
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

// ── Risk & action ───────────────────────────────────────────────────────────

/// Risk classification for trust chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum PermissionRisk {
    /// Read-only / low impact.
    #[default]
    Low,
    /// May modify local state carefully.
    Medium,
    /// Destructive or hard to reverse.
    High,
    /// Data egress / secret exposure / irreversible external effects.
    Critical,
}

impl PermissionRisk {
    /// Theme role for risk chrome.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Low => Role::Info,
            Self::Medium => Role::Warning,
            Self::High | Self::Critical => Role::Danger,
        }
    }

    /// ASCII-friendly risk marker.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Low => "i",
            Self::Medium => "!",
            Self::High => "!!",
            Self::Critical => "X",
        }
    }

    /// Human risk label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "low risk",
            Self::Medium => "medium risk",
            Self::High => "high risk",
            Self::Critical => "critical",
        }
    }

    /// Whether Allow-class actions need extra confirmation language.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }

    /// Default safe focus decision for this risk.
    #[must_use]
    pub const fn default_focus(self) -> PermissionAction {
        match self {
            Self::Low | Self::Medium => PermissionAction::Deny,
            Self::High | Self::Critical => PermissionAction::Deny,
        }
    }
}

/// Kind of gated operation (domain-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PermissionActionKind {
    /// Unspecified action.
    #[default]
    Unknown,
    /// Read file/path.
    FileRead,
    /// Write/create file.
    FileWrite,
    /// Delete path.
    FileDelete,
    /// Shell / process execution.
    Shell,
    /// Outbound network.
    Network,
    /// MCP tool invocation.
    McpTool,
    /// Secrets / credentials access.
    Secrets,
    /// Other consumer-defined action.
    Other,
}

impl PermissionActionKind {
    /// Short action-kind label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "action",
            Self::FileRead => "read",
            Self::FileWrite => "write",
            Self::FileDelete => "delete",
            Self::Shell => "shell",
            Self::Network => "network",
            Self::McpTool => "mcp tool",
            Self::Secrets => "secrets",
            Self::Other => "other",
        }
    }
}

// ── Scope & outcomes ────────────────────────────────────────────────────────

/// How long a grant should last (consumer enforces).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PermissionScope {
    /// This single invocation.
    #[default]
    Once,
    /// Remainder of the session.
    Session,
    /// This project / workspace.
    Project,
    /// Persist always (consumer policy store).
    Always,
}

impl PermissionScope {
    /// All scopes in cycle order.
    pub const ALL: [Self; 4] = [Self::Once, Self::Session, Self::Project, Self::Always];

    #[must_use]
    /// Display label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Once => "Once",
            Self::Session => "Session",
            Self::Project => "Project",
            Self::Always => "Always",
        }
    }
}

/// Focusable actions on the trust surface (not all shown at every risk).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PermissionAction {
    /// Allow with current scope.
    Allow,
    /// Deny.
    Deny,
    /// Allow after consumer edits the command/target.
    AllowEdited,
    /// Allow with a restricted scope (consumer applies restriction).
    AllowRestricted,
    /// Ask the agent to change the request.
    RequestChanges,
    /// Expand/inspect full details (does not grant).
    InspectDetails,
}

impl PermissionAction {
    /// Action button label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Allow => "Allow",
            Self::Deny => "Deny",
            Self::AllowEdited => "Edit&Allow",
            Self::AllowRestricted => "Restrict",
            Self::RequestChanges => "Change",
            Self::InspectDetails => "Details",
        }
    }

    /// Whether this action grants authority.
    #[must_use]
    pub const fn grants(self) -> bool {
        matches!(
            self,
            Self::Allow | Self::AllowEdited | Self::AllowRestricted
        )
    }
}

/// Typed outcome (no side effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PermissionOutcome {
    /// Not handled.
    Ignored,
    /// Focus moved among actions or scope.
    SelectionChanged,
    /// Details expanded/collapsed.
    DetailsToggled {
        /// Expanded?
        expanded: bool,
    },
    /// Entered command/pattern edit mode.
    EditStarted {
        /// Which field.
        field: EditField,
    },
    /// Edit buffer changed.
    EditChanged,
    /// Left edit mode without confirming.
    EditCancelled,
    /// User confirmed a decision for a specific request generation.
    Decided {
        /// Request id.
        request_id: String,
        /// Generation at decide time (stale if ≠ current).
        generation: u64,
        /// Action chosen.
        action: PermissionAction,
        /// Scope for grants.
        scope: PermissionScope,
        /// Edited command/pattern when applicable.
        edited: Option<String>,
    },
    /// Esc / dismiss without decide.
    Cancelled {
        /// Request id if a request was showing.
        request_id: Option<String>,
        /// Generation at cancel time.
        generation: u64,
    },
    /// Queue advanced (request dismissed as stale/resolved externally).
    QueueChanged,
    /// Attempted to confirm a stale generation.
    StaleIgnored {
        /// Generation that was attempted.
        generation: u64,
    },
}

/// Editable field on the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EditField {
    /// Shell command or tool args preview.
    Command,
    /// Path/pattern restriction.
    Pattern,
}

// ── Request model ───────────────────────────────────────────────────────────

/// Data subject of the permission (what is touched).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PermissionTarget {
    /// Primary path, URL, or resource id.
    pub path: String,
    /// Optional secondary descriptor.
    pub detail: Option<String>,
}

/// Where execution would happen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionLocation {
    /// e.g. local workspace, sandbox, remote orb.
    pub label: String,
    /// Optional host/cwd.
    pub detail: Option<String>,
}

/// Data movement for egress warnings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataMovement {
    /// What data is accessed.
    pub accessed: Option<String>,
    /// Destination if transmitted.
    pub destination: Option<String>,
    /// True when data may leave the machine/project.
    pub egress: bool,
}

/// Prior grant hint (consumer supplies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorGrant {
    /// Scope of prior grant.
    pub scope: PermissionScope,
    /// Human summary.
    pub summary: String,
}

/// One permission request (immutable snapshot for display).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// Stable request id (consumer-minted).
    pub id: String,
    /// Monotonic generation for stale protection (set by queue on push).
    pub generation: u64,
    /// Provenance chain.
    pub provenance: PermissionProvenance,
    /// Action class.
    pub action_kind: PermissionActionKind,
    /// Exact action label (e.g. `bash`, `write_file`).
    pub action: String,
    /// Target resource.
    pub target: PermissionTarget,
    /// Where it runs.
    pub location: ExecutionLocation,
    /// Data movement / egress.
    pub data: DataMovement,
    /// Expected result summary.
    pub expected_result: String,
    /// Risk.
    pub risk: PermissionRisk,
    /// Whether consumer believes the op is reversible.
    pub reversible: bool,
    /// Requested default scope (user may change before allow).
    pub requested_scope: PermissionScope,
    /// Prior similar grant, if any.
    pub prior_grant: Option<PriorGrant>,
    /// Editable command/args preview (shell).
    pub command_preview: Option<String>,
    /// Editable allow-pattern (path glob etc.).
    pub pattern_preview: Option<String>,
    /// Extra detail lines (expanded view).
    pub detail_lines: Vec<String>,
    /// Destructive language override (defaults from risk).
    pub destructive_notice: Option<String>,
}

impl PermissionRequest {
    /// Builder-style minimal request.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        action: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            generation: 0,
            provenance: PermissionProvenance::default(),
            action_kind: PermissionActionKind::Unknown,
            action: action.into(),
            target: PermissionTarget {
                path: target.into(),
                detail: None,
            },
            location: ExecutionLocation {
                label: "local".into(),
                detail: None,
            },
            data: DataMovement::default(),
            expected_result: String::new(),
            risk: PermissionRisk::Medium,
            reversible: true,
            requested_scope: PermissionScope::Once,
            prior_grant: None,
            command_preview: None,
            pattern_preview: None,
            detail_lines: Vec::new(),
            destructive_notice: None,
        }
    }

    /// Sets risk tier.
    #[must_use]
    pub fn risk(mut self, risk: PermissionRisk) -> Self {
        self.risk = risk;
        self
    }

    /// Sets provenance chain.
    #[must_use]
    pub fn provenance(mut self, provenance: PermissionProvenance) -> Self {
        self.provenance = provenance;
        self
    }

    /// Sets action kind.
    #[must_use]
    pub fn action_kind(mut self, kind: PermissionActionKind) -> Self {
        self.action_kind = kind;
        self
    }

    /// Sets editable command preview.
    #[must_use]
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command_preview = Some(cmd.into());
        self
    }

    /// Marks data egress with destination and accessed summary.
    #[must_use]
    pub fn egress(mut self, destination: impl Into<String>, accessed: impl Into<String>) -> Self {
        self.data.egress = true;
        self.data.destination = Some(destination.into());
        self.data.accessed = Some(accessed.into());
        self
    }

    /// Marks the operation non-reversible.
    #[must_use]
    pub fn irreversible(mut self) -> Self {
        self.reversible = false;
        self
    }

    /// Expected result summary (what the agent intends to do with the outcome).
    #[must_use]
    pub fn expected(mut self, result: impl Into<String>) -> Self {
        self.expected_result = result.into();
        self
    }

    /// Execution location label (+ optional detail).
    #[must_use]
    pub fn location(mut self, label: impl Into<String>, detail: Option<String>) -> Self {
        self.location = ExecutionLocation {
            label: label.into(),
            detail,
        };
        self
    }

    /// Editable path/pattern restriction preview.
    #[must_use]
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern_preview = Some(pattern.into());
        self
    }

    /// Prior similar grant hint (consumer-supplied).
    #[must_use]
    pub fn prior(mut self, scope: PermissionScope, summary: impl Into<String>) -> Self {
        self.prior_grant = Some(PriorGrant {
            scope,
            summary: summary.into(),
        });
        self
    }

    /// Requested default grant scope.
    #[must_use]
    pub fn scope(mut self, scope: PermissionScope) -> Self {
        self.requested_scope = scope;
        self
    }

    /// Extra detail lines for expanded inspect view.
    #[must_use]
    pub fn details(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.detail_lines = lines.into_iter().map(Into::into).collect();
        self
    }

    /// Custom destructive / egress banner (overrides defaults).
    #[must_use]
    pub fn notice(mut self, notice: impl Into<String>) -> Self {
        self.destructive_notice = Some(notice.into());
        self
    }

    /// Default action strip for this risk.
    #[must_use]
    pub fn actions_for_risk(&self) -> Vec<PermissionAction> {
        let mut actions = vec![PermissionAction::Deny, PermissionAction::InspectDetails];
        if self.command_preview.is_some() || self.pattern_preview.is_some() {
            actions.push(PermissionAction::AllowEdited);
            actions.push(PermissionAction::AllowRestricted);
        }
        actions.push(PermissionAction::RequestChanges);
        // Allow last among grants so default focus (Deny) is first.
        actions.push(PermissionAction::Allow);
        // Order for left-to-right chrome: Deny first, Allow last for high risk.
        if self.risk.is_destructive() {
            // Deny | Details | Change | Edit | Restrict | Allow
            actions = vec![PermissionAction::Deny, PermissionAction::InspectDetails];
            if self.command_preview.is_some() {
                actions.push(PermissionAction::AllowEdited);
            }
            actions.push(PermissionAction::RequestChanges);
            actions.push(PermissionAction::AllowRestricted);
            actions.push(PermissionAction::Allow);
        }
        actions
    }

    /// Destructive banner text.
    #[must_use]
    pub fn warning_text(&self) -> Option<String> {
        if let Some(notice) = &self.destructive_notice {
            return Some(notice.clone());
        }
        if self.data.egress {
            return Some(format!(
                "DATA EGRESS: {} → {}",
                self.data.accessed.as_deref().unwrap_or("data"),
                self.data.destination.as_deref().unwrap_or("external")
            ));
        }
        if !self.reversible || self.risk.is_destructive() {
            return Some(format!(
                "DESTRUCTIVE: {} may be hard to undo",
                self.action_kind.label()
            ));
        }
        None
    }
}

// ── Audit ───────────────────────────────────────────────────────────────────

/// One audit log entry (consumer may persist).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionAuditEntry {
    /// Request id.
    pub request_id: String,
    /// Generation.
    pub generation: u64,
    /// Action taken.
    pub action: PermissionAction,
    /// Scope if grant.
    pub scope: PermissionScope,
    /// Optional edited payload.
    pub edited: Option<String>,
    /// Leaf provenance label.
    pub initiator: String,
}

// ── Queue ───────────────────────────────────────────────────────────────────

/// FIFO permission queue with generation-based stale protection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PermissionQueue {
    pending: Vec<PermissionRequest>,
    next_generation: u64,
    audit: Vec<PermissionAuditEntry>,
}

impl PermissionQueue {
    /// Empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a request; assigns a unique generation.
    pub fn push(&mut self, mut request: PermissionRequest) -> u64 {
        self.next_generation = self.next_generation.saturating_add(1);
        let generation = self.next_generation;
        request.generation = generation;
        self.pending.push(request);
        generation
    }

    /// Current head (oldest pending).
    #[must_use]
    pub fn head(&self) -> Option<&PermissionRequest> {
        self.pending.first()
    }

    /// Number of pending requests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether the queue has no pending requests.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Pending requests oldest-first.
    #[must_use]
    pub fn pending(&self) -> &[PermissionRequest] {
        &self.pending
    }

    /// Audit log (oldest first).
    #[must_use]
    pub fn audit(&self) -> &[PermissionAuditEntry] {
        &self.audit
    }

    /// Whether `generation` is still the live head.
    #[must_use]
    pub fn is_live(&self, generation: u64) -> bool {
        self.pending
            .first()
            .is_some_and(|r| r.generation == generation)
    }

    /// Resolve head if generation matches; records audit on success.
    pub fn resolve(
        &mut self,
        generation: u64,
        action: PermissionAction,
        scope: PermissionScope,
        edited: Option<String>,
    ) -> Result<PermissionRequest, StalePermission> {
        let head = self.pending.first().ok_or(StalePermission {
            generation,
            reason: StaleReason::EmptyQueue,
        })?;
        if head.generation != generation {
            return Err(StalePermission {
                generation,
                reason: StaleReason::Superseded {
                    live: head.generation,
                },
            });
        }
        let req = self.pending.remove(0);
        self.audit.push(PermissionAuditEntry {
            request_id: req.id.clone(),
            generation: req.generation,
            action,
            scope,
            edited,
            initiator: req
                .provenance
                .leaf()
                .map(|h| h.label.clone())
                .unwrap_or_else(|| "unknown".into()),
        });
        Ok(req)
    }

    /// Drop head without grant (cancel / external invalidate).
    pub fn dismiss_head(&mut self, generation: u64) -> Result<PermissionRequest, StalePermission> {
        let result = self.resolve(
            generation,
            PermissionAction::Deny,
            PermissionScope::Once,
            None,
        );
        if result.is_ok()
            && let Some(last) = self.audit.last_mut()
        {
            last.action = PermissionAction::Deny;
        }
        result
    }

    /// Remove any request by id (e.g. tool cancelled upstream).
    pub fn remove_id(&mut self, id: &str) -> bool {
        let before = self.pending.len();
        self.pending.retain(|r| r.id != id);
        before != self.pending.len()
    }
}

/// Stale confirmation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalePermission {
    /// Generation that failed.
    pub generation: u64,
    /// Why.
    pub reason: StaleReason,
}

/// Why a resolve failed as stale.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StaleReason {
    /// No pending requests.
    EmptyQueue,
    /// Head moved on.
    Superseded {
        /// Current head generation.
        live: u64,
    },
}

// ── Surface state ───────────────────────────────────────────────────────────

/// Interaction mode of the trust surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum SurfaceMode {
    #[default]
    Navigate,
    EditCommand,
    EditPattern,
}

/// Hit region for an action button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionActionRegion {
    /// Action.
    pub action: PermissionAction,
    /// Area.
    pub area: Rect,
}

/// Stateful permission / trust surface.
///
/// Holds a [`PermissionQueue`]. Renders and focuses the **head** request only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPromptState {
    /// Queue of pending requests.
    pub queue: PermissionQueue,
    /// Selected action.
    selected: PermissionAction,
    /// Selected grant scope.
    scope: PermissionScope,
    /// Details expanded.
    details_expanded: bool,
    /// Edit mode.
    mode: SurfaceMode,
    /// Edit buffer.
    edit_buffer: String,
    /// Hit regions from last paint.
    pub action_regions: Vec<PermissionActionRegion>,
    /// Available actions for current head (cached).
    available: Vec<PermissionAction>,
}

impl Default for PermissionPromptState {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionPromptState {
    /// Empty prompt state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: PermissionQueue::new(),
            selected: PermissionAction::Deny,
            scope: PermissionScope::Once,
            details_expanded: false,
            mode: SurfaceMode::Navigate,
            edit_buffer: String::new(),
            action_regions: Vec::new(),
            available: vec![PermissionAction::Deny, PermissionAction::Allow],
        }
    }

    /// Enqueue a request and focus safe default for its risk.
    pub fn enqueue(&mut self, request: PermissionRequest) -> u64 {
        let was_empty = self.queue.is_empty();
        let generation = self.queue.push(request);
        if was_empty {
            self.sync_from_head();
        }
        generation
    }

    /// Recompute available actions + default focus from head.
    pub fn sync_from_head(&mut self) {
        if let Some(head) = self.queue.head() {
            self.available = head.actions_for_risk();
            self.selected = head.risk.default_focus();
            if !self.available.contains(&self.selected) {
                self.selected = self
                    .available
                    .first()
                    .copied()
                    .unwrap_or(PermissionAction::Deny);
            }
            self.scope = head.requested_scope;
            self.details_expanded = false;
            self.mode = SurfaceMode::Navigate;
            self.edit_buffer.clear();
        } else {
            self.available = vec![PermissionAction::Deny];
            self.selected = PermissionAction::Deny;
        }
        self.action_regions.clear();
    }

    /// Currently focused action.
    #[must_use]
    pub fn selected(&self) -> PermissionAction {
        self.selected
    }

    /// Selected grant scope.
    #[must_use]
    pub const fn scope(&self) -> PermissionScope {
        self.scope
    }

    /// Whether detail lines are shown.
    #[must_use]
    pub const fn details_expanded(&self) -> bool {
        self.details_expanded
    }

    /// Current edit buffer contents.
    #[must_use]
    pub fn edit_buffer(&self) -> &str {
        &self.edit_buffer
    }

    /// Generation of the head request, if any.
    #[must_use]
    pub fn head_generation(&self) -> Option<u64> {
        self.queue.head().map(|r| r.generation)
    }

    /// Head request snapshot, if any.
    #[must_use]
    pub fn head(&self) -> Option<&PermissionRequest> {
        self.queue.head()
    }

    /// Whether there are pending requests.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Opens the permission overlay on the stack (Alert-class trap for High/Critical head).
    ///
    /// Host still owns paint + key routing into this state while the overlay is top.
    pub fn open_overlay<FocusId: Clone>(
        &self,
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        opener: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        let high_risk = self
            .queue
            .head()
            .is_some_and(|r| r.risk >= PermissionRisk::High);
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static(PERMISSION_OVERLAY_ID),
                kind: if high_risk {
                    OverlayKind::AlertDialog
                } else {
                    OverlayKind::Dialog
                },
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(
                    bounds.width.saturating_sub(4).clamp(28, 72),
                    bounds.height.saturating_sub(4).clamp(8, 18),
                ),
                opener_focus: opener,
                policy: None,
            },
        )
    }

    /// Dismisses the permission overlay (does **not** cancel the queue — host must
    /// still run gate-cancel / [`Self::handle_key`] Esc / dismiss_head per KD-26).
    pub fn dismiss_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.dismiss(&OverlayId::from_static(PERMISSION_OVERLAY_ID))
    }

    /// Keyboard routing.
    pub fn handle_key(&mut self, key: KeyEvent) -> PermissionOutcome {
        if key.kind == KeyEventKind::Release {
            return PermissionOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        if matches!(
            self.mode,
            SurfaceMode::EditCommand | SurfaceMode::EditPattern
        ) {
            return self.handle_edit_key(key, is_press);
        }

        let Some(head) = self.queue.head().cloned() else {
            return PermissionOutcome::Ignored;
        };

        match key.code {
            KeyCode::Left | KeyCode::Up => self.move_action(-1),
            KeyCode::Right | KeyCode::Down | KeyCode::Tab
                if !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.move_action(1)
            }
            KeyCode::BackTab => self.move_action(-1),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.move_action(-1),
            KeyCode::Char('[') if is_press => self.move_scope(-1),
            KeyCode::Char(']') if is_press => self.move_scope(1),
            KeyCode::Char('d' | 'D') if is_press => {
                self.details_expanded = !self.details_expanded;
                PermissionOutcome::DetailsToggled {
                    expanded: self.details_expanded,
                }
            }
            KeyCode::Char('e' | 'E') if is_press && head.command_preview.is_some() => {
                self.mode = SurfaceMode::EditCommand;
                self.edit_buffer = head.command_preview.clone().unwrap_or_default();
                PermissionOutcome::EditStarted {
                    field: EditField::Command,
                }
            }
            KeyCode::Char('p' | 'P') if is_press && head.pattern_preview.is_some() => {
                self.mode = SurfaceMode::EditPattern;
                self.edit_buffer = head.pattern_preview.clone().unwrap_or_default();
                PermissionOutcome::EditStarted {
                    field: EditField::Pattern,
                }
            }
            KeyCode::Char('n' | 'N') if is_press => {
                self.selected = PermissionAction::Deny;
                self.confirm(head.generation)
            }
            KeyCode::Enter if is_press => self.confirm(head.generation),
            KeyCode::Esc if is_press => {
                let generation = head.generation;
                let id = head.id.clone();
                let _ = self.queue.dismiss_head(generation);
                self.sync_from_head();
                PermissionOutcome::Cancelled {
                    request_id: Some(id),
                    generation,
                }
            }
            _ => PermissionOutcome::Ignored,
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent, is_press: bool) -> PermissionOutcome {
        match key.code {
            KeyCode::Esc if is_press => {
                self.mode = SurfaceMode::Navigate;
                self.edit_buffer.clear();
                PermissionOutcome::EditCancelled
            }
            KeyCode::Enter if is_press => {
                let generation = match self.queue.head() {
                    Some(h) => h.generation,
                    None => return PermissionOutcome::Ignored,
                };
                self.selected = if self.mode == SurfaceMode::EditCommand {
                    PermissionAction::AllowEdited
                } else {
                    PermissionAction::AllowRestricted
                };
                let edited = Some(self.edit_buffer.clone());
                self.mode = SurfaceMode::Navigate;
                self.confirm_with_edit(generation, edited)
            }
            KeyCode::Backspace if is_press || key.kind == KeyEventKind::Repeat => {
                self.edit_buffer.pop();
                PermissionOutcome::EditChanged
            }
            KeyCode::Char(c)
                if !c.is_control() && (is_press || key.kind == KeyEventKind::Repeat) =>
            {
                self.edit_buffer.push(c);
                PermissionOutcome::EditChanged
            }
            _ => PermissionOutcome::Ignored,
        }
    }

    fn move_action(&mut self, delta: isize) -> PermissionOutcome {
        if self.available.is_empty() {
            return PermissionOutcome::Ignored;
        }
        let cur = self
            .available
            .iter()
            .position(|a| *a == self.selected)
            .unwrap_or(0) as isize;
        let len = self.available.len() as isize;
        let next = (cur + delta).rem_euclid(len) as usize;
        let next_a = self.available[next];
        if next_a == self.selected {
            return PermissionOutcome::Ignored;
        }
        self.selected = next_a;
        PermissionOutcome::SelectionChanged
    }

    fn move_scope(&mut self, delta: isize) -> PermissionOutcome {
        let cur = PermissionScope::ALL
            .iter()
            .position(|s| *s == self.scope)
            .unwrap_or(0) as isize;
        let len = PermissionScope::ALL.len() as isize;
        let next = (cur + delta).rem_euclid(len) as usize;
        let next_s = PermissionScope::ALL[next];
        if next_s == self.scope {
            return PermissionOutcome::Ignored;
        }
        self.scope = next_s;
        PermissionOutcome::SelectionChanged
    }

    fn confirm(&mut self, generation: u64) -> PermissionOutcome {
        self.confirm_with_edit(generation, None)
    }

    fn confirm_with_edit(&mut self, generation: u64, edited: Option<String>) -> PermissionOutcome {
        if self.selected == PermissionAction::InspectDetails {
            self.details_expanded = !self.details_expanded;
            return PermissionOutcome::DetailsToggled {
                expanded: self.details_expanded,
            };
        }
        if !self.queue.is_live(generation) {
            return PermissionOutcome::StaleIgnored { generation };
        }
        let action = self.selected;
        let scope = self.scope;
        let request_id = self.queue.head().map(|r| r.id.clone()).unwrap_or_default();
        match self
            .queue
            .resolve(generation, action, scope, edited.clone())
        {
            Ok(_) => {
                self.sync_from_head();
                PermissionOutcome::Decided {
                    request_id,
                    generation,
                    action,
                    scope,
                    edited,
                }
            }
            Err(_) => PermissionOutcome::StaleIgnored { generation },
        }
    }

    /// Mouse against last action regions.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> PermissionOutcome {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(region) = self
                    .action_regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                    .cloned()
                else {
                    return PermissionOutcome::Ignored;
                };
                self.selected = region.action;
                let generation = match self.head_generation() {
                    Some(g) => g,
                    None => return PermissionOutcome::Ignored,
                };
                self.confirm(generation)
            }
            MouseEventKind::Moved => {
                let Some(region) = self
                    .action_regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                    .cloned()
                else {
                    return PermissionOutcome::Ignored;
                };
                if region.action == self.selected {
                    return PermissionOutcome::Ignored;
                }
                self.selected = region.action;
                PermissionOutcome::SelectionChanged
            }
            _ => PermissionOutcome::Ignored,
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Permission / trust surface widget (head of queue).
#[derive(Debug, Clone, Copy)]
pub struct PermissionPrompt<'a> {
    theme: &'a Theme,
    /// Use ASCII risk markers.
    ascii: bool,
}

impl<'a> PermissionPrompt<'a> {
    /// Creates a prompt with the given theme.
    #[must_use]
    pub const fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            ascii: false,
        }
    }

    /// Prefer ASCII risk markers.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }
}

impl StatefulWidget for &PermissionPrompt<'_> {
    type State = PermissionPromptState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.action_regions.clear();
        if area.is_empty() {
            return;
        }
        let tokens = DesignTokens::new(self.theme.clone(), Density::Compact);
        let Some(req) = state.queue.head() else {
            let panel = Panel::new(&tokens)
                .title("Permission")
                .emphasis(PanelEmphasis::Normal);
            Widget::render(&panel, area, buffer);
            let inner = panel.inner(area);
            if !inner.is_empty() {
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    "No pending permissions",
                    usize::from(inner.width),
                    self.theme.style(Role::TextMuted),
                );
            }
            return;
        };
        // Clone fields we need so we can mutably use state later for regions.
        let risk = req.risk;
        let title = format!(
            "{} {} · {}",
            if self.ascii {
                risk.glyph()
            } else {
                match risk {
                    PermissionRisk::Low => "ℹ",
                    PermissionRisk::Medium => "!",
                    PermissionRisk::High => "⚠",
                    PermissionRisk::Critical => "⛔",
                }
            },
            risk.label(),
            req.action
        );
        let emphasis = if risk.is_destructive() {
            PanelEmphasis::Danger
        } else {
            PanelEmphasis::Focused
        };
        let panel = Panel::new(&tokens).title(title.as_str()).emphasis(emphasis);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let w = usize::from(inner.width);

        // Provenance
        let prov = format!("from {}", req.provenance.display_path());
        paint_line(
            buffer,
            inner.x,
            y,
            w,
            &prov,
            self.theme.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
        if y >= inner.bottom() {
            return;
        }

        // Action + target
        let line = format!(
            "{} → {}",
            req.action_kind.label(),
            take_display_cols(&req.target.path, w.saturating_sub(12))
        );
        paint_line(buffer, inner.x, y, w, &line, self.theme.style(Role::Text));
        y = y.saturating_add(1);

        if y < inner.bottom() {
            let loc = format!(
                "at {}",
                req.location
                    .detail
                    .as_deref()
                    .unwrap_or(req.location.label.as_str())
            );
            paint_line(
                buffer,
                inner.x,
                y,
                w,
                &loc,
                self.theme.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        if let Some(warn) = req.warning_text()
            && y < inner.bottom()
        {
            paint_line(buffer, inner.x, y, w, &warn, self.theme.style(risk.role()));
            y = y.saturating_add(1);
        }

        if !req.expected_result.is_empty() && y < inner.bottom() {
            let exp = format!("expect: {}", req.expected_result);
            paint_line(
                buffer,
                inner.x,
                y,
                w,
                &exp,
                self.theme.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        if let Some(prior) = &req.prior_grant
            && y < inner.bottom()
        {
            let p = format!("prior: {} ({})", prior.summary, prior.scope.label());
            paint_line(buffer, inner.x, y, w, &p, self.theme.style(Role::Info));
            y = y.saturating_add(1);
        }

        // Command / edit buffer
        if matches!(
            state.mode,
            SurfaceMode::EditCommand | SurfaceMode::EditPattern
        ) && y < inner.bottom()
        {
            let prefix = match state.mode {
                SurfaceMode::EditCommand => "edit cmd> ",
                SurfaceMode::EditPattern => "edit pat> ",
                _ => "> ",
            };
            let line = format!("{prefix}{}", state.edit_buffer);
            paint_line(buffer, inner.x, y, w, &line, self.theme.style(Role::Input));
            y = y.saturating_add(1);
        } else if let Some(cmd) = &req.command_preview
            && y < inner.bottom()
        {
            let line = format!("$ {}", take_display_cols(cmd, w.saturating_sub(2)));
            paint_line(
                buffer,
                inner.x,
                y,
                w,
                &line,
                self.theme.style(Role::TextStrong),
            );
            y = y.saturating_add(1);
        }

        if state.details_expanded {
            for detail in &req.detail_lines {
                if y >= inner.bottom().saturating_sub(2) {
                    break;
                }
                paint_line(
                    buffer,
                    inner.x,
                    y,
                    w,
                    detail,
                    self.theme.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }

        // Scope line
        if y < inner.bottom().saturating_sub(1) {
            let scope_line = format!(
                "scope: {} · [] keys · q:{}",
                state.scope.label(),
                state.queue.len()
            );
            paint_line(
                buffer,
                inner.x,
                y,
                w,
                &scope_line,
                self.theme.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Actions row on last line
        if y < inner.bottom() || inner.height >= 1 {
            let action_y = inner.bottom().saturating_sub(1).max(inner.y);
            let mut x = inner.x;
            for action in &state.available {
                let label = format!(" {} ", action.label());
                let width = (display_cols(&label) as u16).max(1);
                if x.saturating_add(width) > inner.right() {
                    break;
                }
                let rect = Rect::new(x, action_y, width, 1);
                let style = if *action == state.selected {
                    self.theme.style(Role::Selection)
                } else if action.grants() && risk.is_destructive() {
                    self.theme.style(Role::Danger)
                } else {
                    self.theme.style(Role::Text)
                };
                buffer.set_stringn(rect.x, rect.y, &label, usize::from(rect.width), style);
                state.action_regions.push(PermissionActionRegion {
                    action: *action,
                    area: rect,
                });
                x = x.saturating_add(width.saturating_add(1));
            }
        }
    }
}

impl StatefulWidget for PermissionPrompt<'_> {
    type State = PermissionPromptState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

fn paint_line(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    width: usize,
    text: &str,
    style: ratatui_core::style::Style,
) {
    let clipped = take_display_cols(text, width);
    buffer.set_stringn(x, y, &clipped, width, style);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn nested_provenance() -> PermissionProvenance {
        PermissionProvenance::main_agent("run-1", "main")
            .push(ProvenanceHop::new(
                InitiatorKind::Subagent,
                "sub-9",
                "reviewer",
            ))
            .push(ProvenanceHop::new(
                InitiatorKind::McpServer,
                "mcp-fs",
                "filesystem",
            ))
    }

    fn low_read() -> PermissionRequest {
        PermissionRequest::new("r1", "read_file", "src/lib.rs")
            .risk(PermissionRisk::Low)
            .action_kind(PermissionActionKind::FileRead)
            .provenance(PermissionProvenance::main_agent("a", "agent"))
    }

    fn destructive_shell() -> PermissionRequest {
        PermissionRequest::new("r2", "bash", "workspace")
            .risk(PermissionRisk::High)
            .action_kind(PermissionActionKind::Shell)
            .command("rm -rf build/")
            .irreversible()
            .provenance(nested_provenance())
    }

    fn egress_request() -> PermissionRequest {
        PermissionRequest::new("r3", "http_post", "api.example.com")
            .risk(PermissionRisk::Critical)
            .action_kind(PermissionActionKind::Network)
            .egress("https://api.example.com/v1", "src/** + .env")
            .provenance(nested_provenance())
    }

    #[test]
    fn default_focus_is_never_allow() {
        for risk in [
            PermissionRisk::Low,
            PermissionRisk::Medium,
            PermissionRisk::High,
            PermissionRisk::Critical,
        ] {
            assert!(
                !risk.default_focus().grants(),
                "{risk:?} default must not grant"
            );
        }
    }

    #[test]
    fn enqueue_defaults_selection_to_deny() {
        let mut state = PermissionPromptState::new();
        state.enqueue(destructive_shell());
        assert_eq!(state.selected(), PermissionAction::Deny);
        assert!(!state.selected().grants());
    }

    #[test]
    fn enter_on_default_denies_not_allows() {
        let mut state = PermissionPromptState::new();
        state.enqueue(destructive_shell());
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PermissionOutcome::Decided {
                action: PermissionAction::Deny,
                ..
            }
        ));
    }

    #[test]
    fn nested_provenance_display_includes_subagent_and_mcp() {
        let p = nested_provenance();
        let s = p.display_path();
        assert!(s.contains("subagent"));
        assert!(s.contains("mcp"));
        assert!(p.has_subagent());
        assert_eq!(p.leaf().unwrap().kind, InitiatorKind::McpServer);
    }

    #[test]
    fn queue_fifo_and_stale_protection() {
        let mut q = PermissionQueue::new();
        let g1 = q.push(low_read());
        let g2 = q.push(destructive_shell());
        assert_ne!(g1, g2);
        assert!(q.is_live(g1));
        assert!(!q.is_live(g2));
        // Confirming g2 while g1 is head → stale
        let err = q
            .resolve(g2, PermissionAction::Allow, PermissionScope::Once, None)
            .unwrap_err();
        assert!(matches!(
            err.reason,
            StaleReason::Superseded { live } if live == g1
        ));
        // Resolve g1 then g2 becomes live
        q.resolve(g1, PermissionAction::Deny, PermissionScope::Once, None)
            .unwrap();
        assert!(q.is_live(g2));
        q.resolve(g2, PermissionAction::Allow, PermissionScope::Session, None)
            .unwrap();
        assert!(q.is_empty());
        assert_eq!(q.audit().len(), 2);
    }

    #[test]
    fn surface_stale_confirm_after_external_dismiss() {
        let mut state = PermissionPromptState::new();
        let g1 = state.enqueue(low_read());
        let g2 = state.enqueue(destructive_shell());
        assert_eq!(state.head_generation(), Some(g1));
        // Externally resolve head
        state
            .queue
            .resolve(g1, PermissionAction::Deny, PermissionScope::Once, None)
            .unwrap();
        state.sync_from_head();
        assert_eq!(state.head_generation(), Some(g2));
        // Stale g1 confirm
        let out = state.confirm(g1);
        assert!(matches!(
            out,
            PermissionOutcome::StaleIgnored { generation } if generation == g1
        ));
    }

    #[test]
    fn queued_requests_advance_after_decide() {
        let mut state = PermissionPromptState::new();
        state.enqueue(low_read());
        state.enqueue(egress_request());
        assert_eq!(state.queue.len(), 2);
        let _ = state.handle_key(press(KeyCode::Enter)); // deny first
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.queue.head().unwrap().id, "r3");
        assert_eq!(state.selected(), PermissionAction::Deny);
    }

    #[test]
    fn esc_cancels_without_grant_and_advances_queue() {
        let mut state = PermissionPromptState::new();
        state.enqueue(low_read());
        state.enqueue(destructive_shell());
        let out = state.handle_key(press(KeyCode::Esc));
        assert!(matches!(
            out,
            PermissionOutcome::Cancelled {
                request_id: Some(ref id),
                ..
            } if id == "r1"
        ));
        assert_eq!(state.queue.head().unwrap().id, "r2");
    }

    #[test]
    fn y_is_not_bound_to_allow_on_permission_prompt() {
        // Unlike ApprovalCard shortcuts, trust surface does not grant on 'y'.
        let mut state = PermissionPromptState::new();
        state.enqueue(destructive_shell());
        let out = state.handle_key(press(KeyCode::Char('y')));
        assert_eq!(out, PermissionOutcome::Ignored);
        assert_eq!(state.queue.len(), 1);
    }

    #[test]
    fn n_confirms_deny() {
        let mut state = PermissionPromptState::new();
        state.enqueue(low_read());
        let out = state.handle_key(press(KeyCode::Char('n')));
        assert!(matches!(
            out,
            PermissionOutcome::Decided {
                action: PermissionAction::Deny,
                ..
            }
        ));
    }

    #[test]
    fn command_edit_allow_edited_outcome() {
        let mut state = PermissionPromptState::new();
        state.enqueue(destructive_shell());
        let _ = state.handle_key(press(KeyCode::Char('e')));
        assert!(matches!(
            state.handle_key(press(KeyCode::Char('x'))),
            PermissionOutcome::EditChanged
        ));
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PermissionOutcome::Decided {
                action: PermissionAction::AllowEdited,
                edited: Some(ref s),
                ..
            } if s.contains('x') || s.contains("rm")
        ));
    }

    #[test]
    fn scope_cycle() {
        let mut state = PermissionPromptState::new();
        state.enqueue(low_read());
        assert_eq!(state.scope(), PermissionScope::Once);
        let _ = state.handle_key(press(KeyCode::Char(']')));
        assert_eq!(state.scope(), PermissionScope::Session);
        let _ = state.handle_key(press(KeyCode::Char(']')));
        assert_eq!(state.scope(), PermissionScope::Project);
        let _ = state.handle_key(press(KeyCode::Char(']')));
        assert_eq!(state.scope(), PermissionScope::Always);
        let _ = state.handle_key(press(KeyCode::Char('[')));
        assert_eq!(state.scope(), PermissionScope::Project);
    }

    #[test]
    fn details_toggle() {
        let mut state = PermissionPromptState::new();
        let mut req = low_read();
        req.detail_lines = vec!["line1".into(), "line2".into()];
        state.enqueue(req);
        let out = state.handle_key(press(KeyCode::Char('d')));
        assert!(matches!(
            out,
            PermissionOutcome::DetailsToggled { expanded: true }
        ));
    }

    #[test]
    fn egress_warning_text() {
        let req = egress_request();
        let w = req.warning_text().unwrap();
        assert!(w.contains("EGRESS") || w.contains("egress") || w.contains("DATA"));
        assert!(req.data.egress);
    }

    #[test]
    fn mouse_confirm_uses_hit_regions() {
        use ratatui_core::{backend::TestBackend, terminal::Terminal};

        let theme = Theme::default();
        let prompt = PermissionPrompt::new(&theme);
        let mut state = PermissionPromptState::new();
        state.enqueue(low_read());
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(&prompt, f.area(), &mut state);
            })
            .unwrap();
        assert!(!state.action_regions.is_empty());
        // Click Deny region
        let deny = state
            .action_regions
            .iter()
            .find(|r| r.action == PermissionAction::Deny)
            .unwrap()
            .area;
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position {
                x: deny.x,
                y: deny.y,
            },
            modifiers: KeyModifiers::NONE,
        };
        let out = state.handle_mouse(event);
        assert!(matches!(
            out,
            PermissionOutcome::Decided {
                action: PermissionAction::Deny,
                ..
            }
        ));
    }

    #[test]
    fn high_risk_action_strip_puts_deny_before_allow() {
        let req = destructive_shell();
        let actions = req.actions_for_risk();
        let deny_i = actions.iter().position(|a| *a == PermissionAction::Deny);
        let allow_i = actions.iter().position(|a| *a == PermissionAction::Allow);
        assert!(deny_i.unwrap() < allow_i.unwrap());
    }

    #[test]
    fn audit_records_nested_initiator_label() {
        let mut q = PermissionQueue::new();
        let g = q.push(destructive_shell());
        q.resolve(g, PermissionAction::Deny, PermissionScope::Once, None)
            .unwrap();
        assert_eq!(q.audit()[0].initiator, "filesystem");
    }

    #[test]
    fn pattern_edit_allow_restricted() {
        let mut state = PermissionPromptState::new();
        let req = PermissionRequest::new("pat", "write_file", "src/")
            .risk(PermissionRisk::Medium)
            .action_kind(PermissionActionKind::FileWrite)
            .pattern("src/**")
            .provenance(PermissionProvenance::main_agent("a", "agent"));
        state.enqueue(req);
        let out = state.handle_key(press(KeyCode::Char('p')));
        assert!(matches!(
            out,
            PermissionOutcome::EditStarted {
                field: EditField::Pattern
            }
        ));
        let _ = state.handle_key(press(KeyCode::Char('!')));
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PermissionOutcome::Decided {
                action: PermissionAction::AllowRestricted,
                edited: Some(ref s),
                ..
            } if s.contains('!') || s.contains("src")
        ));
    }

    #[test]
    fn edit_esc_cancels_without_resolving() {
        let mut state = PermissionPromptState::new();
        state.enqueue(destructive_shell());
        let _ = state.handle_key(press(KeyCode::Char('e')));
        let out = state.handle_key(press(KeyCode::Esc));
        assert_eq!(out, PermissionOutcome::EditCancelled);
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.selected(), PermissionAction::Deny);
    }

    #[test]
    fn nested_subagent_queue_preserves_provenance_across_advance() {
        let mut state = PermissionPromptState::new();
        state.enqueue(low_read());
        state.enqueue(destructive_shell());
        state.enqueue(egress_request());
        assert_eq!(state.queue.len(), 3);
        // Deny low-risk head
        let _ = state.handle_key(press(KeyCode::Enter));
        let head = state.head().unwrap();
        assert_eq!(head.id, "r2");
        assert!(head.provenance.has_subagent());
        assert_eq!(
            head.provenance.leaf().unwrap().kind,
            InitiatorKind::McpServer
        );
        // Deny destructive → egress head still nested
        let _ = state.handle_key(press(KeyCode::Enter));
        let head = state.head().unwrap();
        assert_eq!(head.id, "r3");
        assert!(head.data.egress);
        assert_eq!(head.provenance.display_path().matches('>').count(), 2);
    }

    #[test]
    fn allow_with_project_scope_records_audit() {
        let mut state = PermissionPromptState::new();
        state.enqueue(
            low_read()
                .scope(PermissionScope::Once)
                .prior(PermissionScope::Session, "src/** previously Session"),
        );
        // Move to Allow (last in strip for low risk)
        for _ in 0..8 {
            let _ = state.handle_key(press(KeyCode::Right));
            if state.selected() == PermissionAction::Allow {
                break;
            }
        }
        assert_eq!(state.selected(), PermissionAction::Allow);
        // Scope Once → Session → Project
        let _ = state.handle_key(press(KeyCode::Char(']')));
        let _ = state.handle_key(press(KeyCode::Char(']')));
        assert_eq!(state.scope(), PermissionScope::Project);
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PermissionOutcome::Decided {
                action: PermissionAction::Allow,
                scope: PermissionScope::Project,
                ..
            }
        ));
        assert_eq!(state.queue.audit().len(), 1);
        assert_eq!(state.queue.audit()[0].scope, PermissionScope::Project);
    }

    #[test]
    fn remove_id_invalidates_non_head_without_stale_grant() {
        let mut state = PermissionPromptState::new();
        let g1 = state.enqueue(low_read());
        state.enqueue(destructive_shell());
        assert!(state.queue.remove_id("r2"));
        assert_eq!(state.queue.len(), 1);
        assert!(state.queue.is_live(g1));
        // Still cannot resolve a fake generation
        assert!(matches!(
            state.confirm(999),
            PermissionOutcome::StaleIgnored { generation: 999 }
        ));
    }

    #[test]
    fn request_fields_cover_trust_checklist() {
        let req = PermissionRequest::new("full", "bash", "workspace")
            .risk(PermissionRisk::High)
            .action_kind(PermissionActionKind::Shell)
            .provenance(nested_provenance())
            .command("rm -rf build/")
            .expected("remove build artifacts")
            .location("local", Some("sandbox:off".into()))
            .irreversible()
            .scope(PermissionScope::Once)
            .prior(PermissionScope::Once, "similar shell once")
            .details(["cwd=/tmp", "env=filtered"]);
        assert!(!req.provenance.chain.is_empty());
        assert_eq!(req.action, "bash");
        assert_eq!(req.target.path, "workspace");
        assert_eq!(req.location.label, "local");
        assert!(req.expected_result.contains("build"));
        assert!(!req.reversible);
        assert!(req.prior_grant.is_some());
        assert_eq!(req.detail_lines.len(), 2);
        assert!(req.warning_text().unwrap().contains("DESTRUCTIVE"));
    }

    #[test]
    fn permission_overlay_opens_alert_for_high_risk() {
        let mut state = PermissionPromptState::new();
        state.enqueue(destructive_shell());
        let mut stack = OverlayStack::<&'static str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let out = state.open_overlay(&mut stack, bounds, Some("composer"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::AlertDialog);
        assert_eq!(
            stack.top().unwrap().id.as_str(),
            PERMISSION_OVERLAY_ID
        );
    }

    #[test]
    fn three_queued_stale_after_head_resolved_externally() {
        let mut state = PermissionPromptState::new();
        let g1 = state.enqueue(low_read());
        let g2 = state.enqueue(destructive_shell());
        let g3 = state.enqueue(egress_request());
        // External host resolves g1
        state
            .queue
            .resolve(g1, PermissionAction::Allow, PermissionScope::Once, None)
            .unwrap();
        state.sync_from_head();
        assert_eq!(state.head_generation(), Some(g2));
        assert!(matches!(
            state.confirm(g1),
            PermissionOutcome::StaleIgnored { generation } if generation == g1
        ));
        // Resolve g2 via UI deny
        let _ = state.handle_key(press(KeyCode::Enter));
        assert_eq!(state.head_generation(), Some(g3));
        // g2 no longer live
        assert!(matches!(
            state.confirm(g2),
            PermissionOutcome::StaleIgnored { generation } if generation == g2
        ));
    }
}
