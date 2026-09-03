// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ConnectionManager** — reusable inventory for database, SSH, API, and
//! service connections.
//!
//! **Mission.** List with per-row status, search, groups, recent, favorites,
//! clear **target + environment** identity, add/edit/test, credentials with
//! **safe secret entry/redaction**, reconnect, and gated delete. Compact
//! **launcher** and **full** management presentations share one state/outcome
//! model. Offline connectivity projects into [`super::ReconnectingState`];
//! connection errors project into diagnostic-shaped summaries.
//!
//! **Host owns** protocol clients, sockets, credential vaults, and persistence.
//! Outcomes are **requests only** — no network or secret-store I/O inside this
//! surface.
//!
//! **vs [`super::IntegrationStatus`].** MCP/plugins lifecycle; not DB/SSH/API
//! connection inventory.
//! **vs [`super::SessionPicker`].** Agent sessions; not service connections.
//! **vs [`super::SchemaBrowser`].** Schema tree nodes; not connection catalog.
//! **vs SetupWizard connection step.** First-run form fields; optional host
//! may compose this block later.
//!
//! Research: TablePlus, SSH managers, cloud CLIs, service dashboards.
//!
//! Teaches: how to compose reusable inventory for database, SSH, API, and
//! service connections.
//!
//! Composes: [`crate::widgets::ConnectivityPhase`],
//! [`crate::widgets::Panel`], [`crate::widgets::PasswordInput`],
//! [`crate::widgets::PasswordInputState`],
//! [`crate::widgets::ReconnectingState`], [`crate::widgets::RevealPolicy`],
//! [`crate::widgets::StatefulWidget`], [`crate::widgets::Widget`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use std::fmt;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, ListRowVisualState, PanelChrome, Role},
    text::display_cols,
    widgets::{
        ConnectivityPhase, Panel, PasswordInput, PasswordInputState, ReconnectingState,
        RevealPolicy, SemanticStatus, StatusIndicator,
    },
};

/// Overlay id for full connection manager.
pub const CONNECTION_MANAGER_OVERLAY_ID: &str = "termrock.connection_manager";
/// Overlay id for compact launcher.
pub const CONNECTION_MANAGER_LAUNCHER_OVERLAY_ID: &str = "termrock.connection_manager_launcher";
/// Visible list window for large catalogs.
pub const CONNECTION_MANAGER_WINDOW: usize = 64;
/// Max recent entries surfaced in Recent view (by host `recent_rank`).
pub const CONNECTION_MANAGER_RECENT_CAP: usize = 32;
/// Redacted paint marker for masked secrets / list chrome (never the secret).
pub const CONNECTION_SECRET_REDACTED: &str = "••••";
/// ASCII redacted marker.
pub const CONNECTION_SECRET_REDACTED_ASCII: &str = "****";

// ── Domain ──────────────────────────────────────────────────────────────────

/// Protocol / connection family (product-neutral; host supplies protocol label).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectionKind {
    /// Database (Postgres, MySQL, …).
    #[default]
    Database,
    /// SSH / remote shell.
    Ssh,
    /// HTTP / REST / GraphQL API.
    Api,
    /// Generic service endpoint.
    Service,
    /// Host-defined.
    Custom,
}

impl ConnectionKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Ssh => "ssh",
            Self::Api => "api",
            Self::Service => "service",
            Self::Custom => "custom",
        }
    }

    /// Short label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Database => "DB",
            Self::Ssh => "SSH",
            Self::Api => "API",
            Self::Service => "Svc",
            Self::Custom => "Custom",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Database => "D",
                Self::Ssh => "S",
                Self::Api => "A",
                Self::Service => "V",
                Self::Custom => "C",
            };
        }
        match self {
            Self::Database => "▣",
            Self::Ssh => "⌘",
            Self::Api => "⇄",
            Self::Service => "⬡",
            Self::Custom => "◆",
        }
    }
}

/// Live connection health (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectionStatus {
    /// Actively connected.
    Connected,
    /// Known but offline / not connected.
    #[default]
    Disconnected,
    /// Connect attempt in flight.
    Connecting,
    /// Reconnect attempt in flight.
    Reconnecting,
    /// Failed (see `last_error`).
    Error,
    /// Needs credentials / auth refresh.
    AuthRequired,
    /// Cached / offline mode (may still browse local meta).
    Offline,
}

impl ConnectionStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Reconnecting => "reconnecting",
            Self::Error => "error",
            Self::AuthRequired => "auth_required",
            Self::Offline => "offline",
        }
    }

    /// Human label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Reconnecting => "reconnecting",
            Self::Error => "error",
            Self::AuthRequired => "auth required",
            Self::Offline => "offline",
        }
    }

    /// Letter (colorless).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Connected => 'C',
            Self::Disconnected => 'D',
            Self::Connecting => '~',
            Self::Reconnecting => 'R',
            Self::Error => 'E',
            Self::AuthRequired => '!',
            Self::Offline => 'O',
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Connected => "*",
                Self::Disconnected => "o",
                Self::Connecting => "~",
                Self::Reconnecting => "~",
                Self::Error => "x",
                Self::AuthRequired => "!",
                Self::Offline => "-",
            };
        }
        match self {
            Self::Connected => "●",
            Self::Disconnected => "○",
            Self::Connecting => "◌",
            Self::Reconnecting => "↻",
            Self::Error => "✗",
            Self::AuthRequired => "⚠",
            Self::Offline => "◌",
        }
    }

    /// Shared lifecycle projection for status recipes.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Connected => SemanticStatus::Online,
            Self::Disconnected | Self::Offline => SemanticStatus::Offline,
            Self::Connecting | Self::Reconnecting => SemanticStatus::Running,
            Self::Error => SemanticStatus::Failed,
            Self::AuthRequired => SemanticStatus::Warning,
        }
    }

    /// Maps to connectivity phase for Offline* projection.
    #[must_use]
    pub const fn to_connectivity_phase(self) -> ConnectivityPhase {
        match self {
            Self::Connected => ConnectivityPhase::Online,
            Self::Disconnected | Self::Offline => ConnectivityPhase::Disconnected,
            Self::Connecting | Self::Reconnecting => ConnectivityPhase::Reconnecting,
            Self::AuthRequired => ConnectivityPhase::AuthRequired,
            Self::Error => ConnectivityPhase::ServerUnavailable,
        }
    }

    /// Offline-like (banner-worthy).
    #[must_use]
    pub const fn is_offline_like(self) -> bool {
        matches!(
            self,
            Self::Disconnected
                | Self::Offline
                | Self::Error
                | Self::AuthRequired
                | Self::Reconnecting
                | Self::Connecting
        )
    }

    /// Whether connect is a sensible request.
    #[must_use]
    pub const fn can_connect(self) -> bool {
        matches!(
            self,
            Self::Disconnected | Self::Offline | Self::Error | Self::AuthRequired
        )
    }

    /// Whether reconnect is a sensible request.
    #[must_use]
    pub const fn can_reconnect(self) -> bool {
        matches!(
            self,
            Self::Error
                | Self::Offline
                | Self::Disconnected
                | Self::AuthRequired
                | Self::Reconnecting
        )
    }

    /// Whether test is allowed (not mid-flight).
    #[must_use]
    pub const fn can_test(self) -> bool {
        !matches!(self, Self::Connecting | Self::Reconnecting)
    }
}

/// Host-projected credential meta — **never** holds the raw secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectionCredentialMeta {
    /// Kind label (`password`, `token`, `key`, `none`).
    pub kind_label: String,
    /// Whether a secret is stored by the host.
    pub has_secret: bool,
    /// Display marker only (`••••`, `set`, `missing`).
    pub redacted_marker: String,
}

impl fmt::Debug for ConnectionCredentialMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionCredentialMeta")
            .field("kind_label", &self.kind_label)
            .field("has_secret", &self.has_secret)
            .field("redacted_marker", &self.redacted_marker)
            .finish()
    }
}

impl Default for ConnectionCredentialMeta {
    fn default() -> Self {
        Self {
            kind_label: "none".into(),
            has_secret: false,
            redacted_marker: CONNECTION_SECRET_REDACTED.into(),
        }
    }
}

impl ConnectionCredentialMeta {
    /// No credential.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Secret present (host vault); paint shows marker only.
    #[must_use]
    pub fn present(kind_label: impl Into<String>) -> Self {
        Self {
            kind_label: kind_label.into(),
            has_secret: true,
            redacted_marker: CONNECTION_SECRET_REDACTED.into(),
        }
    }

    /// Missing required secret.
    #[must_use]
    pub fn missing(kind_label: impl Into<String>) -> Self {
        Self {
            kind_label: kind_label.into(),
            has_secret: false,
            redacted_marker: "missing".into(),
        }
    }

    /// Custom marker.
    #[must_use]
    pub fn marker(mut self, m: impl Into<String>) -> Self {
        self.redacted_marker = m.into();
        self
    }
}

/// One connection projection (host-owned data; no sockets / vault).
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectionEntry {
    /// Stable id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Kind family.
    pub kind: ConnectionKind,
    /// Host-supplied protocol label (`postgres`, `ssh`, `https`, …).
    pub protocol_label: String,
    /// Target identity (host:port, DSN short form, URL host path).
    pub target: String,
    /// Environment identity (`prod`, `staging`, `local`).
    pub environment: String,
    /// Group / folder.
    pub group: Option<String>,
    /// Status.
    pub status: ConnectionStatus,
    /// Favorite.
    pub favorite: bool,
    /// Higher = more recent (host clock ordinal; used for Recent sort).
    pub recent_rank: u64,
    /// Recency label (`2m ago`).
    pub recency: Option<String>,
    /// Last error message (display).
    pub last_error: Option<String>,
    /// Credential meta (redacted only).
    pub credential: ConnectionCredentialMeta,
    /// One-line summary.
    pub summary: Option<String>,
    /// Disabled (cannot connect).
    pub enabled: bool,
}

impl fmt::Debug for ConnectionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionEntry")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("protocol_label", &self.protocol_label)
            .field("target", &self.target)
            .field("environment", &self.environment)
            .field("group", &self.group)
            .field("status", &self.status)
            .field("favorite", &self.favorite)
            .field("recent_rank", &self.recent_rank)
            .field("recency", &self.recency)
            .field("last_error", &self.last_error)
            .field("credential", &self.credential)
            .field("summary", &self.summary)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl ConnectionEntry {
    /// Minimal disconnected entry.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ConnectionKind,
        protocol_label: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            protocol_label: protocol_label.into(),
            target: target.into(),
            environment: "local".into(),
            group: None,
            status: ConnectionStatus::Disconnected,
            favorite: false,
            recent_rank: 0,
            recency: None,
            last_error: None,
            credential: ConnectionCredentialMeta::none(),
            summary: None,
            enabled: true,
        }
    }

    /// Environment.
    #[must_use]
    pub fn environment(mut self, e: impl Into<String>) -> Self {
        self.environment = e.into();
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: ConnectionStatus) -> Self {
        self.status = s;
        self
    }

    /// Favorite.
    #[must_use]
    pub const fn favorite(mut self, on: bool) -> Self {
        self.favorite = on;
        self
    }

    /// Recent rank + label.
    #[must_use]
    pub fn recent(mut self, rank: u64, label: impl Into<String>) -> Self {
        self.recent_rank = rank;
        self.recency = Some(label.into());
        self
    }

    /// Last error.
    #[must_use]
    pub fn last_error(mut self, e: impl Into<String>) -> Self {
        self.last_error = Some(e.into());
        self
    }

    /// Credential meta.
    #[must_use]
    pub fn credential(mut self, c: ConnectionCredentialMeta) -> Self {
        self.credential = c;
        self
    }

    /// Summary.
    #[must_use]
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Target + environment chrome line.
    #[must_use]
    pub fn identity_line(&self) -> String {
        format!("{} · {}", self.target, self.environment)
    }

    /// Semantic / scene-safe credential label (never raw secret).
    #[must_use]
    pub fn credential_scene_label(&self) -> String {
        if self.credential.has_secret {
            format!(
                "{} {}",
                self.credential.kind_label, self.credential.redacted_marker
            )
        } else if self.credential.kind_label == "none" {
            "credential: none".into()
        } else {
            format!("{} missing", self.credential.kind_label)
        }
    }

    /// Case-insensitive query match.
    #[must_use]
    pub fn matches_query(&self, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        let q = q.to_ascii_lowercase();
        let hit = |s: &str| crate::text::contains_lower(&s, &q);
        hit(&self.name)
            || hit(&self.target)
            || hit(&self.environment)
            || hit(&self.protocol_label)
            || hit(&self.id)
            || self.group.as_deref().is_some_and(hit)
            || self.summary.as_deref().is_some_and(hit)
            || hit(self.kind.label())
            || hit(self.status.label())
    }
}

// ── Presentation / phase / view ────────────────────────────────────────────

/// Chrome form: compact launcher vs full management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectionManagerPresentation {
    /// Compact launcher (quick connect / favorites).
    Launcher,
    /// Full management surface.
    #[default]
    Full,
}

impl ConnectionManagerPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Launcher => "launcher",
            Self::Full => "full",
        }
    }
}

/// List scope filter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectionListView {
    /// All (minus empty query).
    #[default]
    All,
    /// Favorites only.
    Favorites,
    /// Recent (ranked).
    Recent,
    /// Single group name.
    Group(String),
}

impl ConnectionListView {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::All => "all".into(),
            Self::Favorites => "favorites".into(),
            Self::Recent => "recent".into(),
            Self::Group(g) => format!("group:{g}"),
        }
    }

    /// Chrome label.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::All => "all".into(),
            Self::Favorites => "favorites".into(),
            Self::Recent => "recent".into(),
            Self::Group(g) => format!("group:{g}"),
        }
    }
}

/// Interaction phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectionManagerPhase {
    /// Browse / search list.
    #[default]
    Browse,
    /// Add new connection form.
    Add,
    /// Edit selected connection form.
    Edit,
    /// Host test in flight (UI busy chrome).
    TestBusy,
    /// Confirm delete (destructive; Cancel default).
    ConfirmDelete,
}

impl ConnectionManagerPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Browse => "browse",
            Self::Add => "add",
            Self::Edit => "edit",
            Self::TestBusy => "test_busy",
            Self::ConfirmDelete => "confirm_delete",
        }
    }
}

/// Which form field is focused in add/edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectionFormField {
    /// Display name.
    #[default]
    Name,
    /// Protocol label.
    Protocol,
    /// Target.
    Target,
    /// Environment.
    Environment,
    /// Group.
    Group,
    /// Secret (PasswordInput).
    Secret,
}

impl ConnectionFormField {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Protocol => "protocol",
            Self::Target => "target",
            Self::Environment => "environment",
            Self::Group => "group",
            Self::Secret => "secret",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Name => Self::Protocol,
            Self::Protocol => Self::Target,
            Self::Target => Self::Environment,
            Self::Environment => Self::Group,
            Self::Group => Self::Secret,
            Self::Secret => Self::Name,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Name => Self::Secret,
            Self::Protocol => Self::Name,
            Self::Target => Self::Protocol,
            Self::Environment => Self::Target,
            Self::Group => Self::Environment,
            Self::Secret => Self::Group,
        }
    }
}

/// Non-secret form draft for add/edit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectionFormDraft {
    /// Name.
    pub name: String,
    /// Kind.
    pub kind: ConnectionKind,
    /// Protocol label.
    pub protocol_label: String,
    /// Target.
    pub target: String,
    /// Environment.
    pub environment: String,
    /// Group.
    pub group: String,
}

impl ConnectionFormDraft {
    /// From entry (no secret).
    #[must_use]
    pub fn from_entry(e: &ConnectionEntry) -> Self {
        Self {
            name: e.name.clone(),
            kind: e.kind,
            protocol_label: e.protocol_label.clone(),
            target: e.target.clone(),
            environment: e.environment.clone(),
            group: e.group.clone().unwrap_or_default(),
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — requests only; host owns I/O, protocol, and vault.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConnectionManagerOutcome {
    /// Ignored.
    Ignored,
    /// Search query changed.
    QueryChanged {
        /// Query.
        query: String,
    },
    /// List view changed.
    ViewChanged(ConnectionListView),
    /// Cursor selection moved.
    Selected {
        /// Connection id.
        id: String,
    },
    /// Connect / open.
    ConnectRequested {
        /// Id.
        id: String,
    },
    /// Test connection.
    TestRequested {
        /// Id.
        id: String,
    },
    /// Reconnect.
    ReconnectRequested {
        /// Id.
        id: String,
    },
    /// Save add/edit. Host reads secret via [`ConnectionManagerState::take_form_secret`]
    /// when `has_secret_draft` is true — **secret never embedded here**.
    SaveRequested {
        /// Existing id for edit; `None` for add.
        id: Option<String>,
        /// Non-secret draft.
        draft: ConnectionFormDraft,
        /// Whether a secret draft is available for host take.
        has_secret_draft: bool,
    },
    /// Delete after confirm.
    DeleteRequested {
        /// Id.
        id: String,
    },
    /// Favorite toggled (optimistic local flip; host persists).
    FavoriteToggled {
        /// Id.
        id: String,
        /// New favorite.
        favorite: bool,
    },
    /// Confirm dialog opened.
    ConfirmOpened {
        /// Id.
        id: String,
    },
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Phase changed.
    PhaseChanged(ConnectionManagerPhase),
    /// Presentation promote to full management.
    FullRequested,
    /// Compact launcher requested.
    LauncherRequested,
    /// Cancelled (Esc from browse).
    Cancelled,
}

// ── Diagnostic projection (owned strings; host may re-borrow into Diagnostic) ─

/// Owned diagnostic-shaped error projection for a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDiagnosticSummary {
    /// Stable diagnostic id (`conn:{id}`).
    pub id: String,
    /// Severity letter path uses Error for failures.
    pub severity_id: &'static str,
    /// Message.
    pub message: String,
    /// Source subsystem.
    pub source: &'static str,
    /// Connection id.
    pub connection_id: String,
    /// Target for labels.
    pub target: String,
    /// Environment.
    pub environment: String,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive connection manager state.
pub struct ConnectionManagerState {
    /// Catalog (may be a window into a larger set).
    pub connections: Vec<ConnectionEntry>,
    /// Search query.
    pub query: String,
    /// Filtered indices into `connections`.
    filtered: Vec<usize>,
    /// Cursor into `filtered`.
    pub cursor: usize,
    /// Scroll offset.
    pub scroll: usize,
    /// Phase.
    pub phase: ConnectionManagerPhase,
    /// Presentation.
    pub presentation: ConnectionManagerPresentation,
    /// List view filter.
    pub list_view: ConnectionListView,
    /// Confirm: false = Cancel (safe default), true = Delete.
    pub confirm_proceed_focused: bool,
    /// Form draft (non-secret).
    pub form: ConnectionFormDraft,
    /// Focused form field.
    pub form_field: ConnectionFormField,
    /// Secret draft — masked; never in Debug/outcomes/scene labels.
    secret: PasswordInputState,
    /// Edit target id (when phase Edit).
    pub edit_id: Option<String>,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Browse search entry mode (`/` activates; Esc leaves).
    ///
    /// When false, letter hotkeys (`t` test, `n` add, …) apply. When true,
    /// printable keys feed the query (including letters that are hotkeys).
    search_mode: bool,
    /// Row hit regions.
    pub row_hits: Vec<(String, Rect)>,
    /// Confirm hits (proceed?).
    pub confirm_hits: Vec<(bool, Rect)>,
}

impl fmt::Debug for ConnectionManagerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionManagerState")
            .field("connections", &self.connections)
            .field("query", &self.query)
            .field("filtered_len", &self.filtered.len())
            .field("cursor", &self.cursor)
            .field("scroll", &self.scroll)
            .field("phase", &self.phase)
            .field("presentation", &self.presentation)
            .field("list_view", &self.list_view)
            .field("confirm_proceed_focused", &self.confirm_proceed_focused)
            .field("form", &self.form)
            .field("form_field", &self.form_field)
            .field("secret", &self.secret) // PasswordInputState redacts
            .field("edit_id", &self.edit_id)
            .field("focused", &self.focused)
            .field("search_mode", &self.search_mode)
            .finish()
    }
}

impl Clone for ConnectionManagerState {
    fn clone(&self) -> Self {
        // PasswordInputState is not Clone; re-create empty secret (safe).
        let mut secret = PasswordInputState::new().with_reveal_policy(RevealPolicy::Never);
        secret.set_focused(false);
        Self {
            connections: self.connections.clone(),
            query: self.query.clone(),
            filtered: self.filtered.clone(),
            cursor: self.cursor,
            scroll: self.scroll,
            phase: self.phase,
            presentation: self.presentation,
            list_view: self.list_view.clone(),
            confirm_proceed_focused: self.confirm_proceed_focused,
            form: self.form.clone(),
            form_field: self.form_field,
            secret,
            edit_id: self.edit_id.clone(),
            focused: self.focused,
            accepts_input: self.accepts_input,
            search_mode: self.search_mode,
            row_hits: self.row_hits.clone(),
            confirm_hits: self.confirm_hits.clone(),
        }
    }
}

impl PartialEq for ConnectionManagerState {
    fn eq(&self, other: &Self) -> bool {
        self.connections == other.connections
            && self.query == other.query
            && self.filtered == other.filtered
            && self.cursor == other.cursor
            && self.scroll == other.scroll
            && self.phase == other.phase
            && self.presentation == other.presentation
            && self.list_view == other.list_view
            && self.confirm_proceed_focused == other.confirm_proceed_focused
            && self.form == other.form
            && self.form_field == other.form_field
            && self.edit_id == other.edit_id
            && self.focused == other.focused
            && self.accepts_input == other.accepts_input
            && self.search_mode == other.search_mode
        // secret intentionally omitted from equality (never compare secrets)
    }
}

impl Eq for ConnectionManagerState {}

impl Default for ConnectionManagerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManagerState {
    /// Empty ready state.
    #[must_use]
    pub fn new() -> Self {
        let mut secret = PasswordInputState::new().with_reveal_policy(RevealPolicy::Never);
        secret.set_focused(false);
        Self {
            connections: Vec::new(),
            query: String::new(),
            filtered: Vec::new(),
            cursor: 0,
            scroll: 0,
            phase: ConnectionManagerPhase::Browse,
            presentation: ConnectionManagerPresentation::Full,
            list_view: ConnectionListView::All,
            confirm_proceed_focused: false,
            form: ConnectionFormDraft::default(),
            form_field: ConnectionFormField::Name,
            secret,
            edit_id: None,
            focused: true,
            accepts_input: true,
            search_mode: false,
            row_hits: Vec::new(),
            confirm_hits: Vec::new(),
        }
    }

    /// Set search query and refilter.
    pub fn set_query(&mut self, q: impl Into<String>) {
        self.query = q.into();
        if !self.query.is_empty() {
            self.search_mode = true;
        }
        self.refilter();
    }

    /// Whether browse search mode is active (`/` entry).
    #[must_use]
    pub const fn search_mode(&self) -> bool {
        self.search_mode
    }

    /// Replace catalog and refilter.
    pub fn set_connections(&mut self, connections: Vec<ConnectionEntry>) {
        let keep = self.current_id();
        self.connections = connections;
        self.refilter();
        if let Some(id) = keep {
            if let Some(fi) = self
                .filtered
                .iter()
                .position(|&si| self.connections.get(si).is_some_and(|c| c.id == id))
            {
                self.cursor = fi;
            }
        }
        self.clamp_cursor();
    }

    /// List view.
    pub fn set_list_view(&mut self, view: ConnectionListView) {
        self.list_view = view;
        self.refilter();
    }

    /// Presentation.
    pub const fn set_presentation(&mut self, p: ConnectionManagerPresentation) {
        self.presentation = p;
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Mark host test finished (leave TestBusy).
    pub fn clear_test_busy(&mut self) {
        if matches!(self.phase, ConnectionManagerPhase::TestBusy) {
            self.phase = ConnectionManagerPhase::Browse;
        }
    }

    /// Current connection id.
    #[must_use]
    pub fn current_id(&self) -> Option<String> {
        self.current().map(|c| c.id.clone())
    }

    /// Current connection.
    #[must_use]
    pub fn current(&self) -> Option<&ConnectionEntry> {
        let si = *self.filtered.get(self.cursor)?;
        self.connections.get(si)
    }

    /// Filtered count.
    #[must_use]
    pub fn filtered_len(&self) -> usize {
        self.filtered.len()
    }

    /// Filtered entries (indices into catalog).
    #[must_use]
    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered
    }

    /// Distinct groups present in catalog (sorted).
    #[must_use]
    pub fn groups(&self) -> Vec<String> {
        let mut g: Vec<String> = self
            .connections
            .iter()
            .filter_map(|c| c.group.clone())
            .collect();
        g.sort();
        g.dedup();
        g
    }

    /// Whether form secret draft is non-empty (does not expose secret).
    #[must_use]
    pub fn has_secret_draft(&self) -> bool {
        !self.secret.is_empty()
    }

    /// Host takes secret after [`ConnectionManagerOutcome::SaveRequested`].
    /// Clears the field. Prefer only on save path.
    pub fn take_form_secret(&mut self) -> String {
        self.secret.take_secret()
    }

    /// Clear secret draft without returning it.
    pub fn clear_form_secret(&mut self) {
        let _ = self.secret.clear();
    }

    /// Seed secret for tests only (production: user entry).
    #[cfg(test)]
    pub fn test_set_secret(&mut self, secret: impl Into<String>) {
        self.secret =
            PasswordInputState::with_secret(secret).with_reveal_policy(RevealPolicy::Never);
        self.secret.set_focused(false);
    }

    /// Project selected (or named) connection into [`ReconnectingState`].
    #[must_use]
    pub fn reconnecting_state_for(&self, id: Option<&str>) -> Option<ReconnectingState> {
        let entry = if let Some(id) = id {
            self.connections.iter().find(|c| c.id == id)
        } else {
            self.current()
        }?;
        Some(connection_to_reconnecting_state(entry))
    }

    /// Diagnostic summary for selected connection error (if any).
    #[must_use]
    pub fn diagnostic_for_current(&self) -> Option<ConnectionDiagnosticSummary> {
        connection_error_diagnostic(self.current()?)
    }

    fn refilter(&mut self) {
        let mut idxs: Vec<usize> = self
            .connections
            .iter()
            .enumerate()
            .filter(|(_, c)| match &self.list_view {
                ConnectionListView::All => true,
                ConnectionListView::Favorites => c.favorite,
                ConnectionListView::Recent => c.recent_rank > 0,
                ConnectionListView::Group(g) => c.group.as_deref() == Some(g.as_str()),
            })
            .filter(|(_, c)| c.matches_query(&self.query))
            .map(|(i, _)| i)
            .collect();

        match &self.list_view {
            ConnectionListView::Recent => {
                idxs.sort_by_key(|&i| {
                    let c = &self.connections[i];
                    // higher rank first; favorites first as tiebreak
                    (
                        std::cmp::Reverse(c.recent_rank),
                        if c.favorite { 0u8 } else { 1 },
                        i,
                    )
                });
                idxs.truncate(CONNECTION_MANAGER_RECENT_CAP);
            }
            ConnectionListView::Favorites => {
                idxs.sort_by_key(|&i| {
                    let c = &self.connections[i];
                    (std::cmp::Reverse(c.recent_rank), i)
                });
            }
            _ => {
                // favorites first, then recent, then original order
                idxs.sort_by_key(|&i| {
                    let c = &self.connections[i];
                    (
                        if c.favorite { 0u8 } else { 1 },
                        std::cmp::Reverse(c.recent_rank),
                        i,
                    )
                });
            }
        }
        self.filtered = idxs;
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        if self.filtered.is_empty() {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }
        self.cursor = self.cursor.min(self.filtered.len() - 1);
        let window = CONNECTION_MANAGER_WINDOW;
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + window {
            self.scroll = self.cursor + 1 - window;
        }
    }

    fn select_cursor(&mut self) -> ConnectionManagerOutcome {
        if let Some(c) = self.current() {
            ConnectionManagerOutcome::Selected { id: c.id.clone() }
        } else {
            ConnectionManagerOutcome::Ignored
        }
    }

    fn move_cursor(&mut self, delta: isize) -> ConnectionManagerOutcome {
        if self.filtered.is_empty() {
            return ConnectionManagerOutcome::Ignored;
        }
        let n = self.filtered.len() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, n - 1) as usize;
        self.clamp_cursor();
        self.select_cursor()
    }

    fn open_confirm_delete(&mut self) -> ConnectionManagerOutcome {
        let Some(c) = self.current() else {
            return ConnectionManagerOutcome::Ignored;
        };
        if !c.enabled {
            return ConnectionManagerOutcome::Ignored;
        }
        let id = c.id.clone();
        self.phase = ConnectionManagerPhase::ConfirmDelete;
        self.confirm_proceed_focused = false; // Cancel default
        ConnectionManagerOutcome::ConfirmOpened { id }
    }

    fn emit_delete(&mut self) -> ConnectionManagerOutcome {
        let Some(id) = self.current_id() else {
            return ConnectionManagerOutcome::Ignored;
        };
        self.phase = ConnectionManagerPhase::Browse;
        self.confirm_proceed_focused = false;
        ConnectionManagerOutcome::DeleteRequested { id }
    }

    fn begin_add(&mut self) -> ConnectionManagerOutcome {
        self.search_mode = false;
        self.phase = ConnectionManagerPhase::Add;
        self.edit_id = None;
        self.form = ConnectionFormDraft {
            kind: ConnectionKind::Database,
            protocol_label: "postgres".into(),
            environment: "local".into(),
            ..ConnectionFormDraft::default()
        };
        self.form_field = ConnectionFormField::Name;
        self.clear_form_secret();
        self.sync_secret_focus();
        ConnectionManagerOutcome::PhaseChanged(ConnectionManagerPhase::Add)
    }

    fn begin_edit(&mut self) -> ConnectionManagerOutcome {
        let Some(e) = self.current().cloned() else {
            return ConnectionManagerOutcome::Ignored;
        };
        if !e.enabled {
            return ConnectionManagerOutcome::Ignored;
        }
        self.search_mode = false;
        self.phase = ConnectionManagerPhase::Edit;
        self.edit_id = Some(e.id.clone());
        self.form = ConnectionFormDraft::from_entry(&e);
        self.form_field = ConnectionFormField::Name;
        self.clear_form_secret(); // host may re-prompt; never prefill secret into state
        self.sync_secret_focus();
        ConnectionManagerOutcome::PhaseChanged(ConnectionManagerPhase::Edit)
    }

    fn sync_secret_focus(&mut self) {
        let on = matches!(
            self.phase,
            ConnectionManagerPhase::Add | ConnectionManagerPhase::Edit
        ) && matches!(self.form_field, ConnectionFormField::Secret);
        self.secret.set_focused(on);
    }

    fn emit_save(&mut self) -> ConnectionManagerOutcome {
        let name = self.form.name.trim().to_string();
        let target = self.form.target.trim().to_string();
        if name.is_empty() || target.is_empty() {
            return ConnectionManagerOutcome::Ignored;
        }
        let mut draft = self.form.clone();
        draft.name = name;
        draft.target = target;
        draft.protocol_label = draft.protocol_label.trim().to_string();
        draft.environment = draft.environment.trim().to_string();
        if draft.environment.is_empty() {
            draft.environment = "local".into();
        }
        draft.group = draft.group.trim().to_string();
        let has_secret_draft = !self.secret.is_empty();
        let id = self.edit_id.clone();
        self.phase = ConnectionManagerPhase::Browse;
        self.form_field = ConnectionFormField::Name;
        self.secret.set_focused(false);
        ConnectionManagerOutcome::SaveRequested {
            id,
            draft,
            has_secret_draft,
        }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> ConnectionManagerOutcome {
        if !self.focused || !self.accepts_input || !key.is_press() {
            return ConnectionManagerOutcome::Ignored;
        }

        match self.phase {
            ConnectionManagerPhase::ConfirmDelete => return self.handle_confirm_key(key),
            ConnectionManagerPhase::Add | ConnectionManagerPhase::Edit => {
                return self.handle_form_key(key);
            }
            ConnectionManagerPhase::TestBusy => {
                if matches!(key.code, KeyCode::Esc) {
                    self.phase = ConnectionManagerPhase::Browse;
                    return ConnectionManagerOutcome::PhaseChanged(ConnectionManagerPhase::Browse);
                }
                return ConnectionManagerOutcome::Ignored;
            }
            ConnectionManagerPhase::Browse => {}
        }

        // Browse: `/` enters search mode; Esc leaves search (or cancels when idle).
        // Letter hotkeys only apply when !search_mode so typing "test" works.
        match key.code {
            KeyCode::Esc if self.search_mode => {
                let had_query = !self.query.is_empty();
                self.query.clear();
                self.search_mode = false;
                if had_query {
                    self.refilter();
                    ConnectionManagerOutcome::QueryChanged {
                        query: String::new(),
                    }
                } else {
                    ConnectionManagerOutcome::Ignored
                }
            }
            KeyCode::Esc => ConnectionManagerOutcome::Cancelled,
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            KeyCode::Char('k') if key.modifiers.is_empty() && !self.search_mode => {
                self.move_cursor(-1)
            }
            KeyCode::Char('j') if key.modifiers.is_empty() && !self.search_mode => {
                self.move_cursor(1)
            }
            KeyCode::Enter => {
                let Some(c) = self.current() else {
                    return ConnectionManagerOutcome::Ignored;
                };
                if !c.enabled {
                    return ConnectionManagerOutcome::Ignored;
                }
                ConnectionManagerOutcome::ConnectRequested { id: c.id.clone() }
            }
            KeyCode::Char('t') if key.modifiers.is_empty() && !self.search_mode => {
                let Some(c) = self.current() else {
                    return ConnectionManagerOutcome::Ignored;
                };
                if !c.enabled || !c.status.can_test() {
                    return ConnectionManagerOutcome::Ignored;
                }
                let id = c.id.clone();
                self.phase = ConnectionManagerPhase::TestBusy;
                ConnectionManagerOutcome::TestRequested { id }
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && !self.search_mode => {
                let Some(c) = self.current() else {
                    return ConnectionManagerOutcome::Ignored;
                };
                if !c.enabled || !c.status.can_reconnect() {
                    return ConnectionManagerOutcome::Ignored;
                }
                ConnectionManagerOutcome::ReconnectRequested { id: c.id.clone() }
            }
            KeyCode::Char('n') if key.modifiers.is_empty() && !self.search_mode => self.begin_add(),
            KeyCode::Char('e') if key.modifiers.is_empty() && !self.search_mode => {
                self.begin_edit()
            }
            KeyCode::Char('f') if key.modifiers.is_empty() && !self.search_mode => {
                let Some((id, fav)) = self.current().map(|c| (c.id.clone(), !c.favorite)) else {
                    return ConnectionManagerOutcome::Ignored;
                };
                if let Some(si) = self.filtered.get(self.cursor).copied() {
                    if let Some(e) = self.connections.get_mut(si) {
                        e.favorite = fav;
                    }
                }
                self.refilter();
                ConnectionManagerOutcome::FavoriteToggled { id, favorite: fav }
            }
            KeyCode::Char('F')
                if key.modifiers.contains(KeyModifiers::SHIFT) && !self.search_mode =>
            {
                self.presentation = ConnectionManagerPresentation::Full;
                ConnectionManagerOutcome::FullRequested
            }
            KeyCode::Char('L')
                if key.modifiers.contains(KeyModifiers::SHIFT) && !self.search_mode =>
            {
                self.presentation = ConnectionManagerPresentation::Launcher;
                ConnectionManagerOutcome::LauncherRequested
            }
            KeyCode::Delete if !self.search_mode => self.open_confirm_delete(),
            KeyCode::Char('d') if key.modifiers.is_empty() && !self.search_mode => {
                self.open_confirm_delete()
            }
            KeyCode::Char('1') if key.modifiers.is_empty() && !self.search_mode => {
                self.list_view = ConnectionListView::All;
                self.refilter();
                ConnectionManagerOutcome::ViewChanged(ConnectionListView::All)
            }
            KeyCode::Char('2') if key.modifiers.is_empty() && !self.search_mode => {
                self.list_view = ConnectionListView::Favorites;
                self.refilter();
                ConnectionManagerOutcome::ViewChanged(ConnectionListView::Favorites)
            }
            KeyCode::Char('3') if key.modifiers.is_empty() && !self.search_mode => {
                self.list_view = ConnectionListView::Recent;
                self.refilter();
                ConnectionManagerOutcome::ViewChanged(ConnectionListView::Recent)
            }
            KeyCode::Char('g') if key.modifiers.is_empty() && !self.search_mode => {
                let groups = self.groups();
                if groups.is_empty() {
                    return ConnectionManagerOutcome::Ignored;
                }
                let next = match &self.list_view {
                    ConnectionListView::Group(cur) => {
                        let pos = groups.iter().position(|g| g == cur);
                        match pos {
                            Some(i) if i + 1 < groups.len() => {
                                ConnectionListView::Group(groups[i + 1].clone())
                            }
                            _ => ConnectionListView::All,
                        }
                    }
                    _ => ConnectionListView::Group(groups[0].clone()),
                };
                self.list_view = next.clone();
                self.refilter();
                ConnectionManagerOutcome::ViewChanged(next)
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && !self.search_mode => {
                self.search_mode = true;
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Backspace => {
                if self.search_mode && !self.query.is_empty() {
                    self.query.pop();
                    self.refilter();
                    return ConnectionManagerOutcome::QueryChanged {
                        query: self.query.clone(),
                    };
                }
                if self.search_mode && self.query.is_empty() {
                    self.search_mode = false;
                    return ConnectionManagerOutcome::Ignored;
                }
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Char('y') | KeyCode::Char('Y') if !self.search_mode => {
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Char(c)
                if self.search_mode
                    && !c.is_control()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.query.push(c);
                self.refilter();
                ConnectionManagerOutcome::QueryChanged {
                    query: self.query.clone(),
                }
            }
            KeyCode::PageDown => self.move_cursor(8),
            KeyCode::PageUp => self.move_cursor(-8),
            KeyCode::Home => {
                self.cursor = 0;
                self.clamp_cursor();
                self.select_cursor()
            }
            KeyCode::End => {
                if !self.filtered.is_empty() {
                    self.cursor = self.filtered.len() - 1;
                    self.clamp_cursor();
                }
                self.select_cursor()
            }
            _ => ConnectionManagerOutcome::Ignored,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> ConnectionManagerOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = ConnectionManagerPhase::Browse;
                self.confirm_proceed_focused = false;
                ConnectionManagerOutcome::ConfirmCancelled
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.confirm_proceed_focused = false;
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.confirm_proceed_focused = true;
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Tab => {
                self.confirm_proceed_focused = !self.confirm_proceed_focused;
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Enter => {
                if self.confirm_proceed_focused {
                    self.emit_delete()
                } else {
                    self.phase = ConnectionManagerPhase::Browse;
                    self.confirm_proceed_focused = false;
                    ConnectionManagerOutcome::ConfirmCancelled
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => ConnectionManagerOutcome::Ignored,
            _ => ConnectionManagerOutcome::Ignored,
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> ConnectionManagerOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = ConnectionManagerPhase::Browse;
                self.clear_form_secret();
                self.edit_id = None;
                self.secret.set_focused(false);
                ConnectionManagerOutcome::PhaseChanged(ConnectionManagerPhase::Browse)
            }
            KeyCode::Enter => self.emit_save(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.form_field = self.form_field.prev();
                self.sync_secret_focus();
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Tab => {
                self.form_field = self.form_field.next();
                self.sync_secret_focus();
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Up => {
                self.form_field = self.form_field.prev();
                self.sync_secret_focus();
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Down => {
                self.form_field = self.form_field.next();
                self.sync_secret_focus();
                ConnectionManagerOutcome::Ignored
            }
            _ if matches!(self.form_field, ConnectionFormField::Secret) => {
                // Route to PasswordInput — outcomes never carry secret text
                let _ = self.secret.handle_key(key);
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Backspace => {
                self.pop_form_char();
                ConnectionManagerOutcome::Ignored
            }
            KeyCode::Char(c)
                if !c.is_control() && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.push_form_char(c);
                ConnectionManagerOutcome::Ignored
            }
            _ => ConnectionManagerOutcome::Ignored,
        }
    }

    fn push_form_char(&mut self, c: char) {
        match self.form_field {
            ConnectionFormField::Name => self.form.name.push(c),
            ConnectionFormField::Protocol => self.form.protocol_label.push(c),
            ConnectionFormField::Target => self.form.target.push(c),
            ConnectionFormField::Environment => self.form.environment.push(c),
            ConnectionFormField::Group => self.form.group.push(c),
            ConnectionFormField::Secret => {}
        }
    }

    fn pop_form_char(&mut self) {
        match self.form_field {
            ConnectionFormField::Name => {
                self.form.name.pop();
            }
            ConnectionFormField::Protocol => {
                self.form.protocol_label.pop();
            }
            ConnectionFormField::Target => {
                self.form.target.pop();
            }
            ConnectionFormField::Environment => {
                self.form.environment.pop();
            }
            ConnectionFormField::Group => {
                self.form.group.pop();
            }
            ConnectionFormField::Secret => {}
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> ConnectionManagerOutcome {
        if !self.focused || !self.accepts_input {
            return ConnectionManagerOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return ConnectionManagerOutcome::Ignored;
        }
        let pos = ev.position;
        if matches!(self.phase, ConnectionManagerPhase::ConfirmDelete) {
            let hit = self
                .confirm_hits
                .iter()
                .find(|(_, r)| r.contains(pos))
                .map(|(p, _)| *p);
            if let Some(proceed) = hit {
                self.confirm_proceed_focused = proceed;
                if proceed {
                    return self.emit_delete();
                }
                self.phase = ConnectionManagerPhase::Browse;
                return ConnectionManagerOutcome::ConfirmCancelled;
            }
            return ConnectionManagerOutcome::Ignored;
        }
        if !matches!(self.phase, ConnectionManagerPhase::Browse) {
            return ConnectionManagerOutcome::Ignored;
        }
        let hit = self
            .row_hits
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| id.clone());
        let Some(id) = hit else {
            return ConnectionManagerOutcome::Ignored;
        };
        if let Some(fi) = self
            .filtered
            .iter()
            .position(|&si| self.connections.get(si).is_some_and(|c| c.id == id))
        {
            let already = self.cursor == fi;
            self.cursor = fi;
            self.clamp_cursor();
            if already {
                if let Some(c) = self.current() {
                    if c.enabled {
                        return ConnectionManagerOutcome::ConnectRequested { id: c.id.clone() };
                    }
                }
            }
            return ConnectionManagerOutcome::Selected { id };
        }
        ConnectionManagerOutcome::Ignored
    }
}

// ── Projection helpers ──────────────────────────────────────────────────────

/// Build [`ReconnectingState`] for a connection (host paints Offline chrome).
#[must_use]
pub fn connection_to_reconnecting_state(entry: &ConnectionEntry) -> ReconnectingState {
    let target = format!("{} ({}) · {}", entry.name, entry.environment, entry.target);
    let mut st = ReconnectingState::new(target);
    match entry.status {
        ConnectionStatus::Connected => {
            st.mark_online(0);
        }
        ConnectionStatus::Connecting | ConnectionStatus::Reconnecting => {
            st.begin_reconnect(1);
        }
        ConnectionStatus::AuthRequired => {
            st.require_auth();
        }
        ConnectionStatus::Error => {
            st.mark_server_unavailable();
        }
        ConnectionStatus::Disconnected | ConnectionStatus::Offline => {
            st.mark_disconnected();
        }
    }
    st
}

/// Project connection error into a diagnostic summary (no raw secrets).
#[must_use]
pub fn connection_error_diagnostic(entry: &ConnectionEntry) -> Option<ConnectionDiagnosticSummary> {
    let msg = entry.last_error.as_ref()?;
    // Never include credential secrets — last_error is host display text.
    Some(ConnectionDiagnosticSummary {
        id: format!("conn:{}", entry.id),
        severity_id: "error",
        message: msg.clone(),
        source: "connection",
        connection_id: entry.id.clone(),
        target: entry.target.clone(),
        environment: entry.environment.clone(),
    })
}

/// Filter connections by query.
#[must_use]
pub fn filter_connections<'a>(
    connections: &'a [ConnectionEntry],
    query: &str,
) -> Vec<&'a ConnectionEntry> {
    connections
        .iter()
        .filter(|c| c.matches_query(query))
        .collect()
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Connection manager painter.
#[derive(Debug, Clone, Copy)]
pub struct ConnectionManager<'a> {
    system: &'a DesignSystem,
    colorless: bool,
    show_detail: bool,
}

impl<'a> ConnectionManager<'a> {
    /// System only — catalog lives in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
            show_detail: true,
        }
    }

    /// ASCII glyphs.
    #[must_use]
    /// Colorless roles.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Hide detail pane (list only).
    #[must_use]
    pub const fn list_only(mut self, on: bool) -> Self {
        self.show_detail = !on;
        self
    }

    fn role(&self, r: Role) -> Role {
        if self.colorless { Role::Text } else { r }
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ConnectionManagerState) {
        state.row_hits.clear();
        state.confirm_hits.clear();
        if area.is_empty() {
            return;
        }

        let title = match state.presentation {
            ConnectionManagerPresentation::Launcher => "Connections · launcher",
            ConnectionManagerPresentation::Full => "Connections",
        };
        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system).title(title).emphasis(emphasis);
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let _w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Identity / offline projection banner when selected is offline-like
        if let Some(c) = state.current() {
            if c.status.is_offline_like() && y < max_y {
                let offline = connection_to_reconnecting_state(c);
                let line = offline.status_bar_content();
                StatusIndicator::new(c.status.semantic(), self.system)
                    .label(&line)
                    .colorless(self.colorless)
                    .paint(Rect::new(inner.x, y, inner.width, 1), buffer, None);
                y = y.saturating_add(1);
            }
        }

        // Search / view / phase line
        if y < max_y {
            let line = match state.phase {
                ConnectionManagerPhase::Browse => {
                    let mode = if state.search_mode { "search" } else { "keys" };
                    format!(
                        "/{}  [{mode}·{}] ({})",
                        state.query,
                        state.list_view.label(),
                        state.filtered_len()
                    )
                }
                ConnectionManagerPhase::Add => "add connection…".into(),
                ConnectionManagerPhase::Edit => "edit connection…".into(),
                ConnectionManagerPhase::TestBusy => "testing…".into(),
                ConnectionManagerPhase::ConfirmDelete => "confirm delete…".into(),
            };
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                &line,
                self.system.style(self.role(Role::Text)),
            );
            y = y.saturating_add(1);
        }

        let footer = if matches!(state.phase, ConnectionManagerPhase::ConfirmDelete) {
            2u16
        } else {
            1u16
        };
        let content_bottom = max_y.saturating_sub(footer);
        let content_h = content_bottom.saturating_sub(y);
        let content = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: content_h,
        };

        match state.phase {
            ConnectionManagerPhase::Add | ConnectionManagerPhase::Edit => {
                self.paint_form(content, buffer, state);
            }
            ConnectionManagerPhase::TestBusy => {
                if !content.is_empty() {
                    self.system.paint_row(
                        buffer,
                        Rect::new(content.x, content.y, inner.width, 1),
                        "testing connection… host owns protocol I/O",
                        self.system.style(self.role(Role::TextSecondary)),
                    );
                }
            }
            ConnectionManagerPhase::Browse | ConnectionManagerPhase::ConfirmDelete => {
                if !content.is_empty() {
                    let launcher =
                        matches!(state.presentation, ConnectionManagerPresentation::Launcher);
                    let show_detail = self.show_detail && !launcher && content.width >= 52;
                    let (list_area, detail_area) = if show_detail {
                        let lw = (content.width * 6 / 10).max(24);
                        (
                            Rect {
                                x: content.x,
                                y: content.y,
                                width: lw,
                                height: content.height,
                            },
                            Some(Rect {
                                x: content.x.saturating_add(lw),
                                y: content.y,
                                width: content.width.saturating_sub(lw),
                                height: content.height,
                            }),
                        )
                    } else {
                        (content, None)
                    };
                    self.paint_list(list_area, buffer, state);
                    if let Some(da) = detail_area {
                        self.paint_detail(da, buffer, state);
                    }
                }
            }
        }

        if matches!(state.phase, ConnectionManagerPhase::ConfirmDelete) {
            self.paint_confirm(inner, buffer, state);
        } else if max_y > inner.y {
            let fy = max_y.saturating_sub(1);
            let hints = match state.presentation {
                ConnectionManagerPresentation::Launcher => {
                    "enter connect · f fav · t test · F full · esc"
                }
                ConnectionManagerPresentation::Full => {
                    "enter connect · n add · t test · 1/2/3 view · esc close"
                }
            };
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, fy, inner.width, 1),
                hints,
                self.system.style(self.role(Role::TextMuted)),
            );
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut ConnectionManagerState) {
        if area.is_empty() {
            return;
        }
        let _w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let viewport = max_y.saturating_sub(y) as usize;

        if state.filtered.is_empty() {
            let msg = if state.query.is_empty() {
                match state.list_view {
                    ConnectionListView::Favorites => "no favorites · f to star",
                    ConnectionListView::Recent => "no recent connections",
                    ConnectionListView::Group(_) => "empty group",
                    ConnectionListView::All => "no connections · n to add",
                }
            } else {
                "no matches"
            };
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                msg,
                self.system.style(self.role(Role::TextMuted)),
            );
            return;
        }

        let mut offset = state.scroll;
        if state.cursor < offset {
            offset = state.cursor;
        } else if viewport > 0 && state.cursor >= offset + viewport {
            offset = state.cursor + 1 - viewport;
        }
        state.scroll = offset;

        let narrow = area.width < 40;
        let tiny = area.width < 28;

        for (row_i, &si) in state
            .filtered
            .iter()
            .enumerate()
            .skip(offset)
            .take(viewport)
        {
            if y >= max_y {
                break;
            }
            let Some(c) = state.connections.get(si) else {
                continue;
            };
            let selected = row_i == state.cursor;
            let indicator = StatusIndicator::new(c.status.semantic(), self.system)
                .label(c.status.label())
                .colorless(self.colorless);
            let status_text = indicator.text(None);
            let fav = if c.favorite { "★" } else { " " };
            let line = if tiny {
                format!(
                    "{}{} {}",
                    if selected { ">" } else { " " },
                    status_text,
                    c.name
                )
            } else if narrow {
                // Drop meta columns first under narrow
                format!(
                    "{}{}{} {} · {}",
                    if selected { ">" } else { " " },
                    fav,
                    status_text,
                    c.name,
                    c.environment
                )
            } else {
                // Default frame carries identity only: status glyph, name, env.
                // Target, protocol, and kind live one keypress away in the
                // detail pane (information budget, plans/017 Part B).
                format!(
                    "{}{}{} {} · {}",
                    if selected { ">" } else { " " },
                    fav,
                    status_text,
                    c.name,
                    c.environment
                )
            };
            let mut style = self.system.style(Role::Text);
            if selected {
                // The gutter marks the row; weight and a tint carry it. A
                // reversed slab loses the connection's own status (plans/010).
                let recipe = self.system.resolve_list_row(ListRowVisualState {
                    selected: true,
                    focused: true,
                    enabled: c.enabled,
                    ..Default::default()
                });
                style = if self.colorless {
                    // Mono selection survives as the explicit reversal pair
                    // (D5); weight alone would not mark the row.
                    self.system.reversed()
                } else {
                    style.patch(recipe.tint).add_modifier(Modifier::BOLD)
                };
            }
            if !c.enabled {
                style = self.system.style(self.role(Role::TextMuted));
            }
            self.system
                .paint_row(buffer, Rect::new(area.x, y, area.width, 1), &line, style);
            let status_x = area.x.saturating_add(if tiny { 1 } else { 2 });
            if status_x < area.right() {
                indicator.paint(
                    Rect::new(status_x, y, area.right().saturating_sub(status_x), 1),
                    buffer,
                    None,
                );
            }
            state.row_hits.push((
                c.id.clone(),
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
            ));
            y = y.saturating_add(1);
        }
    }

    fn paint_detail(&self, area: Rect, buffer: &mut Buffer, state: &ConnectionManagerState) {
        if area.is_empty() {
            return;
        }
        let _w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let Some(c) = state.current() else {
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                "no selection",
                self.system.style(self.role(Role::TextMuted)),
            );
            return;
        };

        let lines: Vec<(String, Role, Option<(SemanticStatus, String)>)> = {
            let status = StatusIndicator::new(c.status.semantic(), self.system)
                .label(c.status.label())
                .colorless(self.colorless);
            let mut v = vec![
                (c.name.clone(), Role::TextStrong, None),
                (
                    format!("{} · {}", c.kind.label(), c.protocol_label),
                    Role::TextMuted,
                    None,
                ),
                (format!("target  {}", c.target), Role::Text, None),
                (format!("env     {}", c.environment), Role::Text, None),
                (
                    format!("group   {}", c.group.as_deref().unwrap_or("—")),
                    Role::TextMuted,
                    None,
                ),
                (
                    status.text(None),
                    Role::Text,
                    Some((c.status.semantic(), c.status.label().to_string())),
                ),
                (
                    format!("cred    {}", c.credential_scene_label()),
                    Role::TextMuted,
                    None,
                ),
            ];
            if let Some(r) = &c.recency {
                v.push((format!("recent  {r}"), Role::TextMuted, None));
            }
            if let Some(s) = &c.summary {
                v.push((s.clone(), Role::Text, None));
            }
            if let Some(err) = &c.last_error {
                let error = format!("error: {err}");
                let status = StatusIndicator::new(SemanticStatus::Failed, self.system)
                    .label(&error)
                    .colorless(self.colorless);
                v.push((
                    status.text(None),
                    Role::Text,
                    Some((SemanticStatus::Failed, error)),
                ));
                if let Some(d) = connection_error_diagnostic(c) {
                    v.push((
                        format!("diag    {} · {}", d.id, d.source),
                        Role::TextMuted,
                        None,
                    ));
                }
            }
            v
        };

        for (line, role, semantic) in lines {
            if y >= max_y {
                break;
            }
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &line,
                self.system.style(self.role(role)),
            );
            if let Some((semantic, label)) = semantic {
                StatusIndicator::new(semantic, self.system)
                    .label(&label)
                    .colorless(self.colorless)
                    .paint(Rect::new(area.x, y, area.width, 1), buffer, None);
            }
            y = y.saturating_add(1);
        }
    }

    fn paint_form(&self, area: Rect, buffer: &mut Buffer, state: &mut ConnectionManagerState) {
        if area.is_empty() {
            return;
        }
        let _w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();

        let fields: [(&str, ConnectionFormField, String); 5] = [
            ("name", ConnectionFormField::Name, state.form.name.clone()),
            (
                "protocol",
                ConnectionFormField::Protocol,
                state.form.protocol_label.clone(),
            ),
            (
                "target",
                ConnectionFormField::Target,
                state.form.target.clone(),
            ),
            (
                "env",
                ConnectionFormField::Environment,
                state.form.environment.clone(),
            ),
            (
                "group",
                ConnectionFormField::Group,
                state.form.group.clone(),
            ),
        ];

        for (label, field, value) in fields {
            if y >= max_y {
                return;
            }
            let focus = state.form_field == field;
            let marker = if focus {
                crate::style::Glyph::SelectionMarker.resolve().text
            } else {
                " "
            };
            let caret = if focus {
                crate::style::Glyph::SelectionGutter.resolve().text
            } else {
                ""
            };
            let line = format!("{marker}{label:8} {value}{caret}");
            let style = if focus {
                self.system
                    .style(self.role(Role::Accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(self.role(Role::Text))
            };
            self.system
                .paint_row(buffer, Rect::new(area.x, y, area.width, 1), &line, style);
            y = y.saturating_add(1);
        }

        // Secret field — PasswordInput paint (masked)
        if y < max_y {
            let focus = matches!(state.form_field, ConnectionFormField::Secret);
            let marker = if focus { ">" } else { " " };
            let label = format!("{marker}secret   ");
            let label_w = display_cols(&label) as u16;
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &label,
                if focus {
                    self.system
                        .style(self.role(Role::Accent))
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(self.role(Role::TextMuted))
                },
            );
            let secret_x = area.x.saturating_add(label_w.min(area.width));
            let secret_w = area.width.saturating_sub(label_w.min(area.width));
            if secret_w > 0 {
                let secret_area = Rect {
                    x: secret_x,
                    y,
                    width: secret_w,
                    height: 1,
                };
                state.secret.set_focused(focus);
                let _ = PasswordInput::new("", self.system).mask('•').paint(
                    secret_area,
                    buffer,
                    &mut state.secret,
                );
            }
            y = y.saturating_add(1);
        }

        if y < max_y {
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                "tab fields · enter save · esc cancel · secret never in outcomes",
                self.system.style(self.role(Role::TextMuted)),
            );
        }
    }

    fn paint_confirm(&self, area: Rect, buffer: &mut Buffer, state: &mut ConnectionManagerState) {
        let y = area.bottom().saturating_sub(2);
        if y < area.y {
            return;
        }
        let _w = usize::from(area.width);
        let name = state
            .current()
            .map(|c| c.name.as_str())
            .unwrap_or("connection");
        let warning = format!("confirm delete “{name}” — irreversible");
        StatusIndicator::new(SemanticStatus::Warning, self.system)
            .label(&warning)
            .colorless(self.colorless)
            .paint(Rect::new(area.x, y, area.width, 1), buffer, None);
        let bar_y = area.bottom().saturating_sub(1);
        let cancel = if !state.confirm_proceed_focused {
            "[Cancel]"
        } else {
            " Cancel "
        };
        let proceed = if state.confirm_proceed_focused {
            "[Delete]"
        } else {
            " Delete "
        };
        let line = format!("{cancel}  {proceed}");
        self.system.paint_row(
            buffer,
            Rect::new(area.x, bar_y, area.width, 1),
            &line,
            self.system.style(self.role(Role::Accent)),
        );
        let cw = display_cols(cancel) as u16;
        state.confirm_hits.push((
            false,
            Rect {
                x: area.x,
                y: bar_y,
                width: cw,
                height: 1,
            },
        ));
        let px = area.x.saturating_add(cw.saturating_add(2));
        let pw = display_cols(proceed) as u16;
        state.confirm_hits.push((
            true,
            Rect {
                x: px,
                y: bar_y,
                width: pw,
                height: 1,
            },
        ));
    }
}

impl StatefulWidget for &ConnectionManager<'_> {
    type State = ConnectionManagerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for ConnectionManager<'_> {
    type State = ConnectionManagerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo catalog (TablePlus / SSH manager flavored).
#[must_use]
pub fn example_connections() -> Vec<ConnectionEntry> {
    vec![
        ConnectionEntry::new(
            "c1",
            "Prod Postgres",
            ConnectionKind::Database,
            "postgres",
            "db.prod.internal:5432",
        )
        .environment("prod")
        .group("databases")
        .status(ConnectionStatus::Connected)
        .favorite(true)
        .recent(100, "2m ago")
        .credential(ConnectionCredentialMeta::present("password"))
        .summary("primary OLTP"),
        ConnectionEntry::new(
            "c2",
            "Staging API",
            ConnectionKind::Api,
            "https",
            "api.staging.example.com",
        )
        .environment("staging")
        .group("apis")
        .status(ConnectionStatus::Disconnected)
        .recent(80, "1h ago")
        .credential(ConnectionCredentialMeta::present("token"))
        .summary("REST v2"),
        ConnectionEntry::new(
            "c3",
            "Bastion SSH",
            ConnectionKind::Ssh,
            "ssh",
            "bastion.ops:22",
        )
        .environment("ops")
        .group("ssh")
        .status(ConnectionStatus::AuthRequired)
        .favorite(true)
        .recent(60, "yesterday")
        .credential(ConnectionCredentialMeta::present("key"))
        .last_error("agent key rejected")
        .summary("jump host"),
        ConnectionEntry::new(
            "c4",
            "Metrics service",
            ConnectionKind::Service,
            "grpc",
            "metrics.local:9090",
        )
        .environment("local")
        .group("services")
        .status(ConnectionStatus::Error)
        .recent(40, "3d ago")
        .last_error("connection refused")
        .credential(ConnectionCredentialMeta::none())
        .summary("otel collector"),
        ConnectionEntry::new(
            "c5",
            "Offline replica",
            ConnectionKind::Database,
            "postgres",
            "replica.cache:5432",
        )
        .environment("local")
        .group("databases")
        .status(ConnectionStatus::Offline)
        .recent(20, "1w ago")
        .credential(ConnectionCredentialMeta::present("password"))
        .summary("cached schema only"),
        ConnectionEntry::new(
            "c6",
            "Disabled lab",
            ConnectionKind::Custom,
            "custom",
            "lab.invalid:1",
        )
        .environment("lab")
        .status(ConnectionStatus::Disconnected)
        .disabled()
        .summary("cannot connect"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 30;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open() -> ConnectionManagerState {
        let mut st = ConnectionManagerState::new();
        st.set_connections(example_connections());
        st
    }

    fn buffer_contains(buf: &Buffer, area: Rect, needle: &str) -> bool {
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let cell = buf.cell((x, y)).unwrap();
                s.push_str(cell.symbol());
            }
            s.push('\n');
        }
        s.contains(needle)
    }

    #[test]
    fn filter_case_insensitive() {
        let mut st = open();
        st.set_query("POSTGRES");
        assert!(st.filtered_len() >= 1);
        assert!(
            st.filtered_indices()
                .iter()
                .any(|&i| st.connections[i].protocol_label == "postgres")
        );
    }

    #[test]
    fn favorites_view() {
        let mut st = open();
        st.set_list_view(ConnectionListView::Favorites);
        assert!(st.filtered_len() >= 1);
        assert!(
            st.filtered_indices()
                .iter()
                .all(|&i| st.connections[i].favorite)
        );
    }

    #[test]
    fn recent_view_orders_by_rank() {
        let mut st = open();
        st.set_list_view(ConnectionListView::Recent);
        assert!(st.filtered_len() >= 2);
        let ranks: Vec<u64> = st
            .filtered_indices()
            .iter()
            .map(|&i| st.connections[i].recent_rank)
            .collect();
        let mut sorted = ranks.clone();
        sorted.sort_by_key(|&r| std::cmp::Reverse(r));
        assert_eq!(ranks, sorted);
    }

    #[test]
    fn group_view() {
        let mut st = open();
        st.set_list_view(ConnectionListView::Group("databases".into()));
        assert!(st.filtered_len() >= 1);
        assert!(
            st.filtered_indices()
                .iter()
                .all(|&i| st.connections[i].group.as_deref() == Some("databases"))
        );
    }

    #[test]
    fn connect_outcome() {
        let mut st = open();
        // favorites first → c1
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::ConnectRequested { ref id } if id == "c1"
        ));
    }

    #[test]
    fn test_and_reconnect_outcomes() {
        let mut st = open();
        // c1 connected — test ok, reconnect may be false; pick error row
        let i = st
            .filtered
            .iter()
            .position(|&si| st.connections[si].id == "c4")
            .unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char('t')));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::TestRequested { ref id } if id == "c4"
        ));
        st.clear_test_busy();
        let out = st.handle_key(press(KeyCode::Char('r')));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::ReconnectRequested { ref id } if id == "c4"
        ));
    }

    #[test]
    fn favorite_toggle() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('f')));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::FavoriteToggled {
                ref id,
                favorite: false
            } if id == "c1"
        ));
    }

    #[test]
    fn delete_without_confirm_does_not_emit() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Delete));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::ConfirmOpened { ref id } if id == "c1"
        ));
        assert!(!st.confirm_proceed_focused);
        // Enter on Cancel default → cancelled, not delete
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, ConnectionManagerOutcome::ConfirmCancelled));
        // never DeleteRequested
    }

    #[test]
    fn delete_confirm_proceed() {
        let mut st = open();
        let _ = st.handle_key(press(KeyCode::Delete));
        let _ = st.handle_key(press(KeyCode::Right));
        assert!(st.confirm_proceed_focused);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::DeleteRequested { ref id } if id == "c1"
        ));
    }

    #[test]
    fn y_unbound_on_confirm() {
        let mut st = open();
        let _ = st.handle_key(press(KeyCode::Delete));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            ConnectionManagerOutcome::Ignored
        ));
    }

    #[test]
    fn add_edit_save_outcomes() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('n')));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::PhaseChanged(ConnectionManagerPhase::Add)
        ));
        for c in "Lab DB".chars() {
            let _ = st.handle_key(press(KeyCode::Char(c)));
        }
        // tab to target
        while !matches!(st.form_field, ConnectionFormField::Target) {
            let _ = st.handle_key(press(KeyCode::Tab));
        }
        for c in "localhost:5432".chars() {
            let _ = st.handle_key(press(KeyCode::Char(c)));
        }
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(
            matches!(
                out,
                ConnectionManagerOutcome::SaveRequested {
                    id: None,
                    ref draft,
                    has_secret_draft: false
                } if draft.name == "Lab DB" && draft.target == "localhost:5432"
            ),
            "{out:?}"
        );

        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('e')));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::PhaseChanged(ConnectionManagerPhase::Edit)
        ));
        st.form.name = "Renamed".into();
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::SaveRequested {
                id: Some(ref id),
                ref draft,
                ..
            } if id == "c1" && draft.name == "Renamed"
        ));
    }

    #[test]
    fn secret_not_in_save_outcome_or_debug() {
        let mut st = open();
        let _ = st.begin_add();
        st.form.name = "Secreted".into();
        st.form.target = "host:1".into();
        st.test_set_secret("s3cr3t-VALUE-never-leak");
        let out = st.handle_key(press(KeyCode::Enter));
        let dbg = format!("{out:?}");
        assert!(
            !dbg.contains("s3cr3t-VALUE-never-leak"),
            "secret leaked in outcome Debug: {dbg}"
        );
        assert!(matches!(
            out,
            ConnectionManagerOutcome::SaveRequested {
                has_secret_draft: true,
                ..
            }
        ));
        let state_dbg = format!("{st:?}");
        assert!(
            !state_dbg.contains("s3cr3t-VALUE-never-leak"),
            "secret leaked in state Debug: {state_dbg}"
        );
        // Secret retained after Save until host takes it (not embedded in outcome).
        assert!(
            st.has_secret_draft(),
            "secret draft must remain available for host take_form_secret after Save"
        );
        let taken = st.take_form_secret();
        assert_eq!(taken, "s3cr3t-VALUE-never-leak");
        assert!(
            !st.has_secret_draft(),
            "take_form_secret must clear the draft"
        );
    }

    #[test]
    fn search_mode_slash_then_test_types_query() {
        let mut st = open();
        // Without search mode, 't' is TestRequested
        let out = st.handle_key(press(KeyCode::Char('t')));
        assert!(
            matches!(out, ConnectionManagerOutcome::TestRequested { .. }),
            "empty browse: t is test hotkey, got {out:?}"
        );
        st.clear_test_busy();

        // '/' enters search mode; typing "test" is query, never TestRequested
        let out = st.handle_key(press(KeyCode::Char('/')));
        assert!(matches!(out, ConnectionManagerOutcome::Ignored));
        assert!(st.search_mode());

        let mut last = ConnectionManagerOutcome::Ignored;
        for c in "test".chars() {
            last = st.handle_key(press(KeyCode::Char(c)));
            assert!(
                matches!(last, ConnectionManagerOutcome::QueryChanged { .. }),
                "search_mode char {c:?} must QueryChanged, got {last:?}"
            );
            assert!(
                !matches!(last, ConnectionManagerOutcome::TestRequested { .. }),
                "must not fire TestRequested while searching"
            );
        }
        assert_eq!(st.query, "test");
        assert!(matches!(
            last,
            ConnectionManagerOutcome::QueryChanged { ref query } if query == "test"
        ));
        // Esc leaves search mode and clears query
        let out = st.handle_key(press(KeyCode::Esc));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::QueryChanged { ref query } if query.is_empty()
        ));
        assert!(!st.search_mode());
        assert!(st.query.is_empty());
    }

    #[test]
    fn masked_secret_not_in_paint_buffer() {
        let system = DesignSystem::default();
        let mut st = open();
        let _ = st.begin_add();
        st.form.name = "X".into();
        st.form.target = "t".into();
        st.form_field = ConnectionFormField::Secret;
        st.sync_secret_focus();
        st.test_set_secret("s3cr3t-VALUE-never-leak");
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        ConnectionManager::new(&system).paint(area, &mut buf, &mut st);
        assert!(
            !buffer_contains(&buf, area, "s3cr3t-VALUE-never-leak"),
            "raw secret appeared in paint buffer"
        );
        // Scene / credential labels on entries never hold raw secrets
        for c in &st.connections {
            let label = c.credential_scene_label();
            assert!(!label.contains("s3cr3t"));
            assert!(
                label.contains(CONNECTION_SECRET_REDACTED)
                    || label.contains("none")
                    || label.contains("missing")
                    || label.contains("****")
                    || label.contains("password")
                    || label.contains("token")
                    || label.contains("key")
            );
        }
    }

    #[test]
    fn entry_debug_has_no_secret_field() {
        let e = &example_connections()[0];
        let dbg = format!("{e:?}");
        assert!(dbg.contains("credential"));
        assert!(dbg.contains("has_secret"));
        // struct fields are meta only
        assert!(!dbg.contains("raw_secret"));
        assert!(!dbg.contains("password_value"));
    }

    #[test]
    fn offline_projection() {
        let mut st = open();
        let i = st
            .filtered
            .iter()
            .position(|&si| st.connections[si].id == "c5")
            .unwrap();
        st.cursor = i;
        let rs = st.reconnecting_state_for(None).unwrap();
        assert_eq!(rs.phase(), ConnectivityPhase::Disconnected);
        assert!(rs.target().contains("Offline replica"));
        assert!(rs.target().contains("replica.cache"));
    }

    #[test]
    fn diagnostic_projection() {
        let mut st = open();
        let i = st
            .filtered
            .iter()
            .position(|&si| st.connections[si].id == "c4")
            .unwrap();
        st.cursor = i;
        let d = st.diagnostic_for_current().unwrap();
        assert_eq!(d.connection_id, "c4");
        assert!(d.message.contains("refused"));
        assert_eq!(d.source, "connection");
        assert_eq!(d.severity_id, "error");
    }

    #[test]
    fn dual_presentation_paint() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        for p in [
            ConnectionManagerPresentation::Full,
            ConnectionManagerPresentation::Launcher,
        ] {
            st.presentation = p;
            ConnectionManager::new(&system).paint(area, &mut buf, &mut st);
        }
        st.phase = ConnectionManagerPhase::ConfirmDelete;
        ConnectionManager::new(&system)
            .colorless(true)
            .list_only(true)
            .paint(area, &mut buf, &mut st);
    }

    #[test]
    fn disabled_cannot_connect() {
        let mut st = open();
        let i = st
            .filtered
            .iter()
            .position(|&si| st.connections[si].id == "c6")
            .unwrap();
        st.cursor = i;
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            ConnectionManagerOutcome::Ignored
        ));
    }

    #[test]
    fn view_keys() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('2'))),
            ConnectionManagerOutcome::ViewChanged(ConnectionListView::Favorites)
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('3'))),
            ConnectionManagerOutcome::ViewChanged(ConnectionListView::Recent)
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('1'))),
            ConnectionManagerOutcome::ViewChanged(ConnectionListView::All)
        ));
        let out = st.handle_key(press(KeyCode::Char('g')));
        assert!(matches!(
            out,
            ConnectionManagerOutcome::ViewChanged(ConnectionListView::Group(_))
        ));
    }

    #[test]
    fn filter_connections_helper() {
        let c = example_connections();
        let hit = filter_connections(&c, "bastion");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "c3");
    }

    #[test]
    fn status_kind_fuzz() {
        for s in [
            ConnectionStatus::Connected,
            ConnectionStatus::Disconnected,
            ConnectionStatus::Connecting,
            ConnectionStatus::Reconnecting,
            ConnectionStatus::Error,
            ConnectionStatus::AuthRequired,
            ConnectionStatus::Offline,
        ] {
            assert!(!s.id().is_empty());
            let _ = s.glyph(true);
            let _ = s.glyph(true);
            let _ = s.to_connectivity_phase();
        }
        for k in [
            ConnectionKind::Database,
            ConnectionKind::Ssh,
            ConnectionKind::Api,
            ConnectionKind::Service,
            ConnectionKind::Custom,
        ] {
            assert!(!k.id().is_empty());
        }
    }

    #[test]
    fn no_network_or_secret_store() {
        let src = include_str!("connection_manager.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in [
            "std::process",
            "Command::new",
            "TcpStream",
            "std::fs::",
            "std::net::",
            "reqwest",
            "tokio::net",
            "keyring",
            "openssl",
        ] {
            assert!(!body.contains(f), "forbidden I/O surface: {f}");
        }
        // Boundary documented
        assert!(
            body.contains("host owns")
                || body.contains("Host owns")
                || body.contains("host-owned")
                || body.contains("requests only")
        );
        assert!(body.contains("ReconnectingState"));
        assert!(body.contains("Launcher") || body.contains("launcher"));
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = ConnectionManagerState::new();
        let many: Vec<_> = (0..200)
            .map(|i| {
                ConnectionEntry::new(
                    format!("id{i}"),
                    format!("Conn {i}"),
                    ConnectionKind::Database,
                    "postgres",
                    format!("host{i}:5432"),
                )
                .environment(if i % 2 == 0 { "prod" } else { "staging" })
                .group("g")
                .recent(i as u64, "now")
                .favorite(i % 5 == 0)
            })
            .collect();
        st.set_connections(many);
        let area = Rect::new(0, 0, 72, 24);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            ConnectionManager::new(&system).paint(area, &mut buf, &mut st);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 3, "{elapsed:?}");
        // Also record-friendly: always succeed if under budget
        let _ = elapsed;
    }

    #[test]
    fn mouse_connect() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        ConnectionManager::new(&system).paint(area, &mut buf, &mut st);
        assert!(!st.row_hits.is_empty());
        let (id, r) = st.row_hits[0].clone();
        let out = st.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        });
        assert!(
            matches!(
                out,
                ConnectionManagerOutcome::Selected { .. }
                    | ConnectionManagerOutcome::ConnectRequested { .. }
            ),
            "{out:?} {id}"
        );
        let out = st.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        });
        assert!(
            matches!(out, ConnectionManagerOutcome::ConnectRequested { .. }),
            "{out:?}"
        );
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let system = DesignSystem::default();
        for _ascii in [false, true] {
            for (width, height) in [(48, 14), (20, 5), (1, 1), (0, 0)] {
                let mut st = ConnectionManagerState::new();
                st.set_connections(vec![
                    ConnectionEntry::new(
                        "u1",
                        "本番DB Cafe\u{301}",
                        ConnectionKind::Database,
                        "postgres",
                        "db.東京:5432",
                    )
                    .environment("本番")
                    .group("データベース")
                    .favorite(true)
                    .credential(ConnectionCredentialMeta::present("パスワード")),
                    ConnectionEntry::new("u2", "堡垒机", ConnectionKind::Ssh, "ssh", "堡垒:22")
                        .environment("运维"),
                ]);
                let area = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(area);
                ConnectionManager::new(&system).paint(area, &mut buf, &mut st);
                if width == 48 {
                    let text: String = buf.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains('本'), "{text:?}");
                    assert!(text.contains("Cafe\u{301}"), "{text:?}");
                }
            }
        }
    }

    #[test]
    fn cancel_esc() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Esc)),
            ConnectionManagerOutcome::Cancelled
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            ConnectionManagerOutcome::Ignored
        ));
    }

    #[test]
    fn selection_stable_on_set() {
        let mut st = open();
        st.cursor = 1;
        let id = st.current_id().unwrap();
        let mut next = example_connections();
        next.push(ConnectionEntry::new(
            "c9",
            "Extra",
            ConnectionKind::Api,
            "https",
            "x",
        ));
        st.set_connections(next);
        assert_eq!(st.current_id().as_deref(), Some(id.as_str()));
    }

    #[test]
    fn presentation_requests() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT)),
            ConnectionManagerOutcome::FullRequested
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT)),
            ConnectionManagerOutcome::LauncherRequested
        ));
    }
}
