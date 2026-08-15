// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **IntegrationStatus** — status and management for MCP servers, plugins,
//! extensions, tools, and external integrations.
//!
//! **Mission.** Lifecycle states (connected → degraded → error…), source and
//! third-party **provenance**, capabilities, permissions, last error, log
//! tail, restart / enable / disable / details outcomes. Compact **badges**
//! separate from full **settings/diagnostic panels**. Safe permission and
//! **data-egress** language (never implies auto-grant). No process/network I/O
//! — host owns lifecycle and secrets.
//!
//! **vs [`super::connectivity`].** Offline/reconnecting for a remote target;
//! IntegrationStatus is multi-integration inventory (MCP/plugin/tool).
//! **vs [`super::PermissionPrompt`].** Single trust gate; this surfaces
//! integration health + management requests.
//! **vs [`super::AgentStatusHeader`].** Session chrome, not integration list.
//!
//! Research: Grok Build extension/MCP views, editor extension managers,
//! service health panels.
//!
//! Teaches: how to compose status and management for MCP servers, plugins,
//! extensions, tools, and external integrations.
//!
//! Composes: [`crate::widgets::Panel`], [`crate::widgets::StatefulWidget`],
//! [`crate::widgets::Widget`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Glyph, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::Panel,
    widgets::{EmptyKind, EmptyState},
};

/// Overlay id for full integration panel.
pub const INTEGRATION_STATUS_OVERLAY_ID: &str = "termrock.integration_status";
/// Max log lines retained for paint (host may supply more; we window).
pub const INTEGRATION_LOG_WINDOW: usize = 8;
/// Max integrations painted in list window.
pub const INTEGRATION_LIST_WINDOW: usize = 48;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Kind of integration (product-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IntegrationKind {
    /// MCP server.
    #[default]
    McpServer,
    /// Plugin / skill pack.
    Plugin,
    /// Editor-style extension.
    Extension,
    /// Discrete tool binary / adapter.
    Tool,
    /// External service / API.
    External,
}

impl IntegrationKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::McpServer => "mcp",
            Self::Plugin => "plugin",
            Self::Extension => "extension",
            Self::Tool => "tool",
            Self::External => "external",
        }
    }

    /// Short label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::McpServer => "MCP",
            Self::Plugin => "Plugin",
            Self::Extension => "Extension",
            Self::Tool => "Tool",
            Self::External => "External",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::McpServer => "M",
                Self::Plugin => "P",
                Self::Extension => "E",
                Self::Tool => "T",
                Self::External => "X",
            };
        }
        match self {
            Self::McpServer => "⬡",
            Self::Plugin => "◆",
            Self::Extension => "◇",
            Self::Tool => "⚙",
            Self::External => "☁",
        }
    }
}

/// Lifecycle / health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IntegrationHealth {
    /// Healthy and ready.
    Connected,
    /// Not running / offline.
    #[default]
    Disconnected,
    /// Starting / connecting.
    Starting,
    /// Failed (see last_error).
    Error,
    /// Needs host permission before use.
    PermissionRequired,
    /// Update available (still usable unless host says otherwise).
    UpdateAvailable,
    /// Explicitly disabled by user/host.
    Disabled,
    /// Connected but degraded (partial capabilities).
    Degraded,
}

impl IntegrationHealth {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Starting => "starting",
            Self::Error => "error",
            Self::PermissionRequired => "permission_required",
            Self::UpdateAvailable => "update_available",
            Self::Disabled => "disabled",
            Self::Degraded => "degraded",
        }
    }

    /// Human label (safe language).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Starting => "starting",
            Self::Error => "error",
            Self::PermissionRequired => "permission required",
            Self::UpdateAvailable => "update available",
            Self::Disabled => "disabled",
            Self::Degraded => "degraded",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Connected => "+",
                Self::Disconnected => "o",
                Self::Starting => "~",
                Self::Error => "x",
                Self::PermissionRequired => "!",
                Self::UpdateAvailable => "^",
                Self::Disabled => "-",
                Self::Degraded => "!",
            };
        }
        match self {
            Self::Connected => "●",
            Self::Disconnected => "○",
            Self::Starting => "◌",
            Self::Error => "✗",
            Self::PermissionRequired => "⚠",
            Self::UpdateAvailable => "↑",
            Self::Disabled => "–",
            Self::Degraded => "◐",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Connected => Role::Success,
            Self::Disconnected | Self::Disabled => Role::TextMuted,
            Self::Starting | Self::UpdateAvailable => Role::Info,
            Self::PermissionRequired | Self::Degraded => Role::Warning,
            Self::Error => Role::Danger,
        }
    }

    /// Needs user attention before normal use.
    #[must_use]
    pub const fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::Error | Self::PermissionRequired | Self::Degraded | Self::Starting
        )
    }

    /// Whether restart is a sensible request.
    #[must_use]
    pub const fn can_restart(self) -> bool {
        !matches!(self, Self::Disabled | Self::Starting)
    }

    /// Whether enable is sensible.
    #[must_use]
    pub const fn can_enable(self) -> bool {
        matches!(self, Self::Disabled | Self::Disconnected)
    }

    /// Whether disable is sensible.
    #[must_use]
    pub const fn can_disable(self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

/// Declared capability (host-projected; no execution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationCapability {
    /// Id (`tools/list`, `resources/read`).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether this capability may move data **out** of the workspace.
    pub may_egress: bool,
}

impl IntegrationCapability {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            may_egress: false,
        }
    }

    /// Mark as potential data egress (safe language for chrome).
    #[must_use]
    pub const fn egress(mut self) -> Self {
        self.may_egress = true;
        self
    }
}

/// Declared permission scope (policy-free label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationPermission {
    /// Id.
    pub id: String,
    /// Label (`read workspace`, `run shell`).
    pub label: String,
    /// Granted by host (UI only).
    pub granted: bool,
    /// Whether this is a high-impact / egress-class permission.
    pub elevated: bool,
}

impl IntegrationPermission {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            granted: false,
            elevated: false,
        }
    }

    /// Granted.
    #[must_use]
    pub const fn granted(mut self, on: bool) -> Self {
        self.granted = on;
        self
    }

    /// Elevated / egress-class.
    #[must_use]
    pub const fn elevated(mut self) -> Self {
        self.elevated = true;
        self
    }
}

/// Third-party provenance (explicit; never hide origin).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegrationProvenance {
    /// Publisher / author (`org`, `vendor`, `community`).
    pub publisher: Option<String>,
    /// Source URI or package id (display).
    pub source: Option<String>,
    /// Version string.
    pub version: Option<String>,
    /// Trust note (`signed`, `unsigned`, `local path`).
    pub trust_note: Option<String>,
    /// Whether third-party (not first-party host).
    pub third_party: bool,
}

impl IntegrationProvenance {
    /// First-party.
    #[must_use]
    pub fn first_party(version: impl Into<String>) -> Self {
        Self {
            publisher: Some("host".into()),
            source: None,
            version: Some(version.into()),
            trust_note: Some("first-party".into()),
            third_party: false,
        }
    }

    /// Third-party package.
    #[must_use]
    pub fn third_party(
        publisher: impl Into<String>,
        source: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            publisher: Some(publisher.into()),
            source: Some(source.into()),
            version: Some(version.into()),
            trust_note: Some("third-party — review permissions".into()),
            third_party: true,
        }
    }

    /// Trust note.
    #[must_use]
    pub fn trust_note(mut self, n: impl Into<String>) -> Self {
        self.trust_note = Some(n.into());
        self
    }

    /// One-line provenance chrome.
    #[must_use]
    pub fn summary_line(&self) -> String {
        let mut parts = Vec::new();
        if self.third_party {
            parts.push("3rd-party".into());
        } else {
            parts.push("1st-party".into());
        }
        if let Some(p) = &self.publisher {
            parts.push(p.clone());
        }
        if let Some(v) = &self.version {
            parts.push(format!("v{v}"));
        }
        if let Some(s) = &self.source {
            parts.push(s.clone());
        }
        parts.join(" · ")
    }
}

/// One integration entry (host-projected snapshot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationEntry {
    /// Stable id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Kind.
    pub kind: IntegrationKind,
    /// Health.
    pub health: IntegrationHealth,
    /// Provenance.
    pub provenance: IntegrationProvenance,
    /// Capabilities.
    pub capabilities: Vec<IntegrationCapability>,
    /// Permissions.
    pub permissions: Vec<IntegrationPermission>,
    /// Last error (display).
    pub last_error: Option<String>,
    /// Recent log lines (newest last; paint windows).
    pub logs: Vec<String>,
    /// Short description.
    pub summary: Option<String>,
    /// Explicit egress warning text (host).
    pub egress_warning: Option<String>,
    /// Enabled flag (may lag health when starting).
    pub enabled: bool,
}

impl IntegrationEntry {
    /// Minimal disconnected entry.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: IntegrationKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            health: IntegrationHealth::Disconnected,
            provenance: IntegrationProvenance::default(),
            capabilities: Vec::new(),
            permissions: Vec::new(),
            last_error: None,
            logs: Vec::new(),
            summary: None,
            egress_warning: None,
            enabled: true,
        }
    }

    /// Health.
    #[must_use]
    pub const fn health(mut self, h: IntegrationHealth) -> Self {
        self.health = h;
        self
    }

    /// Provenance.
    #[must_use]
    pub fn provenance(mut self, p: IntegrationProvenance) -> Self {
        self.provenance = p;
        self
    }

    /// Capabilities.
    #[must_use]
    pub fn capabilities(mut self, c: Vec<IntegrationCapability>) -> Self {
        self.capabilities = c;
        self
    }

    /// Permissions.
    #[must_use]
    pub fn permissions(mut self, p: Vec<IntegrationPermission>) -> Self {
        self.permissions = p;
        self
    }

    /// Last error.
    #[must_use]
    pub fn last_error(mut self, e: impl Into<String>) -> Self {
        self.last_error = Some(e.into());
        self
    }

    /// Logs.
    #[must_use]
    pub fn logs(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.logs = lines.into_iter().map(Into::into).collect();
        self
    }

    /// Summary.
    #[must_use]
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }

    /// Egress warning.
    #[must_use]
    pub fn egress_warning(mut self, w: impl Into<String>) -> Self {
        self.egress_warning = Some(w.into());
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Whether any capability may egress.
    #[must_use]
    pub fn may_egress(&self) -> bool {
        self.egress_warning.is_some()
            || self.capabilities.iter().any(|c| c.may_egress)
            || self.permissions.iter().any(|p| p.elevated && p.granted)
    }

    /// Safe egress chrome line (never implies grant).
    #[must_use]
    pub fn egress_line(&self) -> Option<String> {
        if let Some(w) = &self.egress_warning {
            return Some(format!("egress risk: {w}"));
        }
        if self.capabilities.iter().any(|c| c.may_egress) {
            return Some("may send data outside workspace — review before enable".into());
        }
        if self.permissions.iter().any(|p| p.elevated) {
            return Some("elevated permissions declared — host must confirm grants".into());
        }
        None
    }
}

// ── Presentation ────────────────────────────────────────────────────────────

/// Compact badge vs list vs full diagnostic panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IntegrationStatusPresentation {
    /// Single badge for one integration (or aggregate).
    Badge,
    /// Compact multi-row list.
    #[default]
    CompactList,
    /// Full settings / diagnostic panel.
    Panel,
}

impl IntegrationStatusPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Badge => "badge",
            Self::CompactList => "compact_list",
            Self::Panel => "panel",
        }
    }
}

/// Detail sub-pane inside Panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IntegrationDetailTab {
    /// Overview.
    #[default]
    Overview,
    /// Capabilities.
    Capabilities,
    /// Permissions.
    Permissions,
    /// Logs.
    Logs,
}

impl IntegrationDetailTab {
    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Capabilities => "Caps",
            Self::Permissions => "Perms",
            Self::Logs => "Logs",
        }
    }

    fn cycle() -> &'static [IntegrationDetailTab] {
        &[
            Self::Overview,
            Self::Capabilities,
            Self::Permissions,
            Self::Logs,
        ]
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — **requests only**; host restarts, grants, and fetches logs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntegrationStatusOutcome {
    /// Ignored.
    Ignored,
    /// Selection moved.
    Selected {
        /// Integration id.
        id: String,
    },
    /// Presentation changed.
    PresentationChanged(IntegrationStatusPresentation),
    /// Detail tab changed.
    TabChanged(IntegrationDetailTab),
    /// Restart requested (host).
    RestartRequested {
        /// Id.
        id: String,
    },
    /// Enable requested.
    EnableRequested {
        /// Id.
        id: String,
    },
    /// Disable requested.
    DisableRequested {
        /// Id.
        id: String,
    },
    /// Open full details (host may promote panel/overlay).
    DetailsRequested {
        /// Id.
        id: String,
    },
    /// Open / stream logs (host).
    LogsRequested {
        /// Id.
        id: String,
    },
    /// Permission grant flow requested (host opens PermissionPrompt).
    PermissionRequested {
        /// Integration id.
        id: String,
        /// Permission id if specific.
        permission_id: Option<String>,
    },
    /// Apply available update (host).
    UpdateRequested {
        /// Id.
        id: String,
    },
    /// Focus egress warning (a11y / announce).
    EgressWarningFocused {
        /// Id.
        id: String,
    },
    /// Fullscreen / overlay promote.
    FullscreenRequested,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive integration status state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationStatusState {
    /// Entries.
    pub entries: Vec<IntegrationEntry>,
    /// Cursor.
    pub cursor: usize,
    /// Scroll.
    pub scroll: usize,
    /// Presentation.
    pub presentation: IntegrationStatusPresentation,
    /// Detail tab (panel).
    pub tab: IntegrationDetailTab,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Log scroll inside panel.
    pub log_scroll: usize,
    /// Row hits.
    pub row_hits: Vec<(String, Rect)>,
    /// Action strip hits for selected (restart, enable…).
    pub action_hits: Vec<(IntegrationAction, Rect)>,
    /// Focused action index in strip.
    pub action_cursor: usize,
}

/// Local action strip ids (paint + confirm via outcomes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IntegrationAction {
    /// Restart.
    Restart,
    /// Enable.
    Enable,
    /// Disable.
    Disable,
    /// Details.
    Details,
    /// Logs.
    Logs,
    /// Permission.
    Permission,
    /// Update.
    Update,
}

impl IntegrationAction {
    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Restart => "Restart",
            Self::Enable => "Enable",
            Self::Disable => "Disable",
            Self::Details => "Details",
            Self::Logs => "Logs",
            Self::Permission => "Permit",
            Self::Update => "Update",
        }
    }
}

impl Default for IntegrationStatusState {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrationStatusState {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            scroll: 0,
            presentation: IntegrationStatusPresentation::CompactList,
            tab: IntegrationDetailTab::Overview,
            focused: true,
            accepts_input: true,
            log_scroll: 0,
            row_hits: Vec::new(),
            action_hits: Vec::new(),
            action_cursor: 0,
        }
    }

    /// Set entries.
    pub fn set_entries(&mut self, entries: Vec<IntegrationEntry>) {
        let keep = self.current_id();
        self.entries = entries;
        if let Some(id) = keep {
            if let Some(i) = self.entries.iter().position(|e| e.id == id) {
                self.cursor = i;
            }
        }
        self.clamp_cursor();
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Presentation.
    pub const fn set_presentation(&mut self, p: IntegrationStatusPresentation) {
        self.presentation = p;
    }

    /// Current.
    #[must_use]
    pub fn current(&self) -> Option<&IntegrationEntry> {
        self.entries.get(self.cursor)
    }

    /// Current id.
    #[must_use]
    pub fn current_id(&self) -> Option<String> {
        self.current().map(|e| e.id.clone())
    }

    /// Available actions for current entry.
    #[must_use]
    pub fn actions_for_current(&self) -> Vec<IntegrationAction> {
        let Some(e) = self.current() else {
            return Vec::new();
        };
        let mut a = Vec::new();
        if e.health.can_restart() && e.enabled {
            a.push(IntegrationAction::Restart);
        }
        if e.health.can_enable() || !e.enabled {
            a.push(IntegrationAction::Enable);
        }
        if e.health.can_disable() && e.enabled {
            a.push(IntegrationAction::Disable);
        }
        a.push(IntegrationAction::Details);
        a.push(IntegrationAction::Logs);
        if matches!(e.health, IntegrationHealth::PermissionRequired)
            || e.permissions.iter().any(|p| !p.granted)
        {
            a.push(IntegrationAction::Permission);
        }
        if matches!(e.health, IntegrationHealth::UpdateAvailable) {
            a.push(IntegrationAction::Update);
        }
        a
    }

    fn clamp_cursor(&mut self) {
        if self.entries.is_empty() {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }
        self.cursor = self.cursor.min(self.entries.len() - 1);
        let window = INTEGRATION_LIST_WINDOW;
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + window {
            self.scroll = self.cursor + 1 - window;
        }
        let actions = self.actions_for_current();
        if self.action_cursor >= actions.len() {
            self.action_cursor = actions.len().saturating_sub(1);
        }
    }

    fn move_cursor(&mut self, delta: isize) -> IntegrationStatusOutcome {
        if self.entries.is_empty() {
            return IntegrationStatusOutcome::Ignored;
        }
        let n = self.entries.len() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, n - 1) as usize;
        self.action_cursor = 0;
        self.log_scroll = 0;
        self.clamp_cursor();
        IntegrationStatusOutcome::Selected {
            id: self.entries[self.cursor].id.clone(),
        }
    }

    fn fire_action(&self, action: IntegrationAction) -> IntegrationStatusOutcome {
        let Some(e) = self.current() else {
            return IntegrationStatusOutcome::Ignored;
        };
        let id = e.id.clone();
        match action {
            IntegrationAction::Restart => IntegrationStatusOutcome::RestartRequested { id },
            IntegrationAction::Enable => IntegrationStatusOutcome::EnableRequested { id },
            IntegrationAction::Disable => IntegrationStatusOutcome::DisableRequested { id },
            IntegrationAction::Details => IntegrationStatusOutcome::DetailsRequested { id },
            IntegrationAction::Logs => IntegrationStatusOutcome::LogsRequested { id },
            IntegrationAction::Permission => {
                let permission_id = e
                    .permissions
                    .iter()
                    .find(|p| !p.granted)
                    .map(|p| p.id.clone());
                IntegrationStatusOutcome::PermissionRequested { id, permission_id }
            }
            IntegrationAction::Update => IntegrationStatusOutcome::UpdateRequested { id },
        }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> IntegrationStatusOutcome {
        if !self.focused || !self.accepts_input || key.kind != KeyEventKind::Press {
            return IntegrationStatusOutcome::Ignored;
        }
        if self.entries.is_empty() {
            return IntegrationStatusOutcome::Ignored;
        }

        // Badge mode: limited
        if self.presentation == IntegrationStatusPresentation::Badge {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('d') => {
                    self.presentation = IntegrationStatusPresentation::Panel;
                    IntegrationStatusOutcome::PresentationChanged(
                        IntegrationStatusPresentation::Panel,
                    )
                }
                KeyCode::Char('f') => IntegrationStatusOutcome::FullscreenRequested,
                _ => IntegrationStatusOutcome::Ignored,
            };
        }

        match key.code {
            KeyCode::Esc => {
                if self.presentation == IntegrationStatusPresentation::Panel {
                    self.presentation = IntegrationStatusPresentation::CompactList;
                    return IntegrationStatusOutcome::PresentationChanged(
                        IntegrationStatusPresentation::CompactList,
                    );
                }
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Tab if self.presentation == IntegrationStatusPresentation::Panel => {
                let tabs = IntegrationDetailTab::cycle();
                let i = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.tab = tabs[(i + 1) % tabs.len()];
                IntegrationStatusOutcome::TabChanged(self.tab)
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let actions = self.actions_for_current();
                if actions.is_empty() {
                    return IntegrationStatusOutcome::Ignored;
                }
                self.action_cursor = self.action_cursor.saturating_sub(1);
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let actions = self.actions_for_current();
                if actions.is_empty() {
                    return IntegrationStatusOutcome::Ignored;
                }
                if self.action_cursor + 1 < actions.len() {
                    self.action_cursor += 1;
                }
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::Enter => {
                let actions = self.actions_for_current();
                if let Some(a) = actions.get(self.action_cursor).copied() {
                    return self.fire_action(a);
                }
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::Char('r') => self.fire_action(IntegrationAction::Restart),
            KeyCode::Char('e') => self.fire_action(IntegrationAction::Enable),
            KeyCode::Char('x') => self.fire_action(IntegrationAction::Disable),
            KeyCode::Char('d') => {
                if self.presentation != IntegrationStatusPresentation::Panel {
                    self.presentation = IntegrationStatusPresentation::Panel;
                    IntegrationStatusOutcome::PresentationChanged(
                        IntegrationStatusPresentation::Panel,
                    )
                } else {
                    self.fire_action(IntegrationAction::Details)
                }
            }
            KeyCode::Char('g') => self.fire_action(IntegrationAction::Logs),
            KeyCode::Char('p') => self.fire_action(IntegrationAction::Permission),
            KeyCode::Char('u') => self.fire_action(IntegrationAction::Update),
            KeyCode::Char('b') => {
                // Badge compact for selected
                self.presentation = IntegrationStatusPresentation::Badge;
                IntegrationStatusOutcome::PresentationChanged(IntegrationStatusPresentation::Badge)
            }
            KeyCode::Char('f') => IntegrationStatusOutcome::FullscreenRequested,
            KeyCode::Char('w') => {
                if let Some(id) = self.current_id() {
                    if self.current().is_some_and(|e| e.may_egress()) {
                        return IntegrationStatusOutcome::EgressWarningFocused { id };
                    }
                }
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::PageDown if self.tab == IntegrationDetailTab::Logs => {
                self.log_scroll = self.log_scroll.saturating_add(4);
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::PageUp if self.tab == IntegrationDetailTab::Logs => {
                self.log_scroll = self.log_scroll.saturating_sub(4);
                IntegrationStatusOutcome::Ignored
            }
            KeyCode::Char('y') => IntegrationStatusOutcome::Ignored,
            _ => IntegrationStatusOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> IntegrationStatusOutcome {
        if !self.focused || !self.accepts_input {
            return IntegrationStatusOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return IntegrationStatusOutcome::Ignored;
        }
        let pos = ev.position;
        for (action, r) in &self.action_hits {
            if r.contains(pos) {
                return self.fire_action(*action);
            }
        }
        let hit = self
            .row_hits
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| id.clone());
        if let Some(id) = hit {
            if let Some(i) = self.entries.iter().position(|e| e.id == id) {
                self.cursor = i;
                self.action_cursor = 0;
                return IntegrationStatusOutcome::Selected { id };
            }
        }
        IntegrationStatusOutcome::Ignored
    }

    /// Aggregate badge label for multi-entry compact chrome.
    #[must_use]
    pub fn aggregate_badge(&self) -> String {
        let n = self.entries.len();
        let bad = self
            .entries
            .iter()
            .filter(|e| e.health.needs_attention())
            .count();
        if bad > 0 {
            format!("{n} integrations · {bad} need attention")
        } else {
            format!("{n} integrations · ok")
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Integration status painter.
#[derive(Debug, Clone, Copy)]
pub struct IntegrationStatus<'a> {
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
}

/// The row a clipping list must stop at, so the cut has somewhere to be said.
///
/// A list that fills its last row has nowhere left to admit what it dropped,
/// which is how a surface goes silent: it looks complete because the sentence
/// explaining otherwise never had a cell (plans/017 §B2).
fn clip_bottom(bottom: u16, total: usize, shown: usize) -> u16 {
    if total > shown.saturating_add(1) {
        bottom.saturating_sub(1)
    } else {
        bottom
    }
}

impl<'a> IntegrationStatus<'a> {
    /// System only.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            ascii: false,
            colorless: false,
        }
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut IntegrationStatusState) {
        state.row_hits.clear();
        state.action_hits.clear();
        if area.is_empty() {
            return;
        }
        match state.presentation {
            IntegrationStatusPresentation::Badge => self.paint_badge(area, buffer, state),
            IntegrationStatusPresentation::CompactList => self.paint_list(area, buffer, state),
            IntegrationStatusPresentation::Panel => self.paint_panel(area, buffer, state),
        }
    }

    fn paint_badge(&self, area: Rect, buffer: &mut Buffer, state: &IntegrationStatusState) {
        let _w = usize::from(area.width);
        let (text, role) = if let Some(e) = state.current() {
            let g = e.health.glyph(self.ascii);
            let k = e.kind.glyph(self.ascii);
            let line = format!("{k}{g} {} · {}", e.name, e.health.label());
            let role = if self.colorless {
                Role::Text
            } else {
                e.health.role()
            };
            (line, role)
        } else {
            (state.aggregate_badge(), Role::TextMuted)
        };
        self.system.paint_row(
            buffer,
            Rect::new(area.x, area.y, area.width, 1),
            &text,
            self.system.style(role),
        );
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut IntegrationStatusState) {
        let panel = Panel::new(self.system)
            .title("Integrations")
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom().saturating_sub(1);

        if state.entries.is_empty() {
            EmptyState::new("No integrations", self.system)
                .kind(EmptyKind::NoData)
                .paint(Rect::new(inner.x, y, inner.width, 1), buffer);
            return;
        }

        let viewport = max_y.saturating_sub(y) as usize;
        let mut offset = state.scroll;
        if state.cursor < offset {
            offset = state.cursor;
        } else if viewport > 0 && state.cursor >= offset + viewport {
            offset = state.cursor + 1 - viewport;
        }
        state.scroll = offset;

        for (i, e) in state.entries.iter().enumerate().skip(offset) {
            if y >= max_y {
                break;
            }
            let selected = i == state.cursor;
            let mark = if selected {
                if self.ascii { ">" } else { "›" }
            } else {
                " "
            };
            let kg = e.kind.glyph(self.ascii);
            let hg = e.health.glyph(self.ascii);
            let party = if e.provenance.third_party { "3p" } else { "1p" };
            // Egress is a fact worth a glyph, and the glyph has to survive an
            // ASCII terminal (plans/013 Step 2).
            let egress = if e.may_egress() {
                format!(" {}", self.system.glyphs.resolve(Glyph::ArrowUp).text)
            } else {
                String::new()
            };
            let text = format!(
                "{mark}{kg}{hg} {} · {} · {party}{egress}",
                e.name,
                e.health.label()
            );
            let style = if selected {
                self.system.style(Role::Accent).add_modifier(Modifier::BOLD)
            } else if self.colorless {
                self.system.style(Role::Text)
            } else {
                self.system.style(e.health.role())
            };
            self.system
                .paint_row(buffer, Rect::new(inner.x, y, inner.width, 1), &text, style);
            state.row_hits.push((
                e.id.clone(),
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
            ));
            y = y.saturating_add(1);
        }

        // Footer actions for selection
        let fy = inner.bottom().saturating_sub(1);
        self.paint_actions(inner.x, fy, w, buffer, state);
    }

    fn paint_panel(&self, area: Rect, buffer: &mut Buffer, state: &mut IntegrationStatusState) {
        let title = state
            .current()
            .map(|e| format!("{} · {}", e.kind.label(), e.name))
            .unwrap_or_else(|| "Integration".into());
        let panel = Panel::new(self.system)
            .title(title.as_str())
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Tabs
        if y < max_y {
            let mut x = inner.x;
            for tab in IntegrationDetailTab::cycle() {
                let sel = state.tab == *tab;
                let t = if sel {
                    format!("[{}]", tab.label())
                } else {
                    format!(" {} ", tab.label())
                };
                let tw = display_cols(&t) as u16;
                if x.saturating_add(tw) > inner.x.saturating_add(inner.width) {
                    break;
                }
                let style = if sel {
                    self.system.style(Role::Accent)
                } else {
                    self.system.style(Role::TextMuted)
                };
                self.system
                    .paint_row(buffer, Rect::new(x, y, tw, 1), &t, style);
                x = x.saturating_add(tw.saturating_add(1));
            }
            y = y.saturating_add(1);
        }

        let Some(e) = state.current().cloned() else {
            return;
        };

        // Provenance always visible (third-party explicit)
        if y < max_y {
            let line = format!("from {}", e.provenance.summary_line());
            let role = if e.provenance.third_party {
                Role::Warning
            } else {
                Role::TextMuted
            };
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                &line,
                self.system.style(role),
            );
            y = y.saturating_add(1);
        }

        // Egress warning
        if let Some(eg) = e.egress_line() {
            if y < max_y {
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    &format!("! {eg}"),
                    self.system.style(Role::Warning),
                );
                y = y.saturating_add(1);
            }
        }

        let content_bottom = max_y.saturating_sub(1);
        match state.tab {
            IntegrationDetailTab::Overview => {
                let lines = [
                    format!(
                        "status: {} {}",
                        e.health.glyph(self.ascii),
                        e.health.label()
                    ),
                    e.summary.clone().unwrap_or_default(),
                    e.last_error
                        .as_ref()
                        .map(|err| format!("last error: {err}"))
                        .unwrap_or_default(),
                    format!(
                        "caps: {} · perms: {}/{}",
                        e.capabilities.len(),
                        e.permissions.iter().filter(|p| p.granted).count(),
                        e.permissions.len()
                    ),
                ];
                for line in lines {
                    if line.is_empty() || y >= content_bottom {
                        continue;
                    }
                    self.system.paint_row(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        &line,
                        self.system.style(Role::Text),
                    );
                    y = y.saturating_add(1);
                }
            }
            IntegrationDetailTab::Capabilities => {
                let mut shown = 0usize;
                for c in &e.capabilities {
                    if y >= clip_bottom(content_bottom, e.capabilities.len(), shown) {
                        break;
                    }
                    shown = shown.saturating_add(1);
                    let eg = if c.may_egress { " [egress]" } else { "" };
                    let line = format!("· {}{eg}", c.label);
                    let role = if c.may_egress {
                        Role::Warning
                    } else {
                        Role::Text
                    };
                    self.system.paint_row(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        &line,
                        self.system.style(role),
                    );
                    y = y.saturating_add(1);
                }
                if e.capabilities.is_empty() && y < content_bottom {
                    EmptyState::new("No capabilities declared", self.system)
                        .kind(EmptyKind::NoData)
                        .paint(Rect::new(inner.x, y, inner.width, 1), buffer);
                }
                self.paint_more_note(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    content_bottom,
                    e.capabilities.len().saturating_sub(shown),
                );
            }
            IntegrationDetailTab::Permissions => {
                let mut shown = 0usize;
                for p in &e.permissions {
                    if y >= clip_bottom(content_bottom, e.permissions.len(), shown) {
                        break;
                    }
                    shown = shown.saturating_add(1);
                    let g = if p.granted { "granted" } else { "not granted" };
                    let el = if p.elevated { " · elevated" } else { "" };
                    let line = format!("· {} — {g}{el}", p.label);
                    let role = if !p.granted {
                        Role::Warning
                    } else if p.elevated {
                        Role::Danger
                    } else {
                        Role::Text
                    };
                    self.system.paint_row(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        &line,
                        self.system.style(role),
                    );
                    y = y.saturating_add(1);
                }
                if e.permissions.is_empty() && y < content_bottom {
                    EmptyState::new("No permissions declared", self.system)
                        .kind(EmptyKind::NoData)
                        .paint(Rect::new(inner.x, y, inner.width, 1), buffer);
                }
                self.paint_more_note(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    content_bottom,
                    e.permissions.len().saturating_sub(shown),
                );
            }
            IntegrationDetailTab::Logs => {
                let start = state.log_scroll.min(e.logs.len());
                let after_start = e.logs.len().saturating_sub(start);
                let mut shown = 0usize;
                for line in e.logs.iter().skip(start).take(INTEGRATION_LOG_WINDOW) {
                    if y >= clip_bottom(content_bottom, after_start, shown) {
                        break;
                    }
                    shown = shown.saturating_add(1);
                    self.system.paint_row(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        line,
                        self.system.style(Role::TextMuted),
                    );
                    y = y.saturating_add(1);
                }
                if e.logs.is_empty() && y < content_bottom {
                    EmptyState::new("No logs", self.system)
                        .kind(EmptyKind::NoData)
                        .explanation("g requests the host stream")
                        .paint(Rect::new(inner.x, y, inner.width, 1), buffer);
                }
                self.paint_more_note(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    content_bottom,
                    after_start.saturating_sub(shown),
                );
            }
        }

        let fy = max_y.saturating_sub(1);
        if fy >= inner.y {
            self.paint_actions(inner.x, fy, w, buffer, state);
        }
    }

    /// States what a detail list held back, on the row the clip reserved.
    fn paint_more_note(&self, buffer: &mut Buffer, row: Rect, bottom: u16, hidden: usize) {
        let Some(note) = crate::text::more_note(hidden) else {
            return;
        };
        if row.y >= bottom {
            return;
        }
        self.system.paint_row(
            buffer,
            row,
            &note,
            self.system
                .style(Role::TextMuted)
                .add_modifier(Modifier::DIM),
        );
    }

    fn paint_actions(
        &self,
        x: u16,
        y: u16,
        w: usize,
        buffer: &mut Buffer,
        state: &mut IntegrationStatusState,
    ) {
        let actions = state.actions_for_current();
        if actions.is_empty() {
            self.system.paint_row(
                buffer,
                Rect::new(x, y, u16::try_from(w).unwrap_or(u16::MAX), 1),
                "j/k select · d panel · b badge",
                self.system.style(Role::TextMuted),
            );
            return;
        }
        let mut col = x;
        let end = x.saturating_add(w as u16);
        for (i, action) in actions.iter().enumerate() {
            let focused = i == state.action_cursor;
            let label = action.label();
            let text = if focused {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            };
            let tw = display_cols(&text) as u16;
            if col.saturating_add(tw) > end {
                break;
            }
            let style = if focused {
                self.system.style(Role::Accent).add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::TextMuted)
            };
            self.system
                .paint_row(buffer, Rect::new(col, y, tw, 1), &text, style);
            state.action_hits.push((
                *action,
                Rect {
                    x: col,
                    y,
                    width: tw,
                    height: 1,
                },
            ));
            col = col.saturating_add(tw.saturating_add(1));
        }
    }
}

impl StatefulWidget for &IntegrationStatus<'_> {
    type State = IntegrationStatusState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for IntegrationStatus<'_> {
    type State = IntegrationStatusState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo MCP / plugin inventory.
#[must_use]
pub fn example_integrations() -> Vec<IntegrationEntry> {
    vec![
        IntegrationEntry::new("mcp-fs", "filesystem", IntegrationKind::McpServer)
            .health(IntegrationHealth::Connected)
            .provenance(IntegrationProvenance::first_party("1.2.0"))
            .summary("Read/write workspace files")
            .capabilities(vec![
                IntegrationCapability::new("fs/read", "Read files"),
                IntegrationCapability::new("fs/write", "Write files"),
            ])
            .permissions(vec![
                IntegrationPermission::new("ws-read", "read workspace").granted(true),
                IntegrationPermission::new("ws-write", "write workspace")
                    .granted(true)
                    .elevated(),
            ])
            .logs(["ready", "tools/list ok"]),
        IntegrationEntry::new("mcp-web", "web-fetch", IntegrationKind::McpServer)
            .health(IntegrationHealth::PermissionRequired)
            .provenance(IntegrationProvenance::third_party(
                "community",
                "npm:@example/web-fetch",
                "0.9.1",
            ))
            .summary("HTTP fetch tool")
            .capabilities(vec![
                IntegrationCapability::new("http/get", "HTTP GET").egress(),
            ])
            .permissions(vec![
                IntegrationPermission::new("net", "outbound network").elevated(),
            ])
            .egress_warning("can send request URLs and bodies to remote hosts")
            .logs(["awaiting permission"]),
        IntegrationEntry::new("plug-lint", "lint-helper", IntegrationKind::Plugin)
            .health(IntegrationHealth::Degraded)
            .provenance(
                IntegrationProvenance::third_party("acme", "https://plugins.example/lint", "2.0.0")
                    .trust_note("unsigned package — review before enable"),
            )
            .summary("Partial: rules pack failed to load")
            .last_error("ruleset v3 missing")
            .capabilities(vec![IntegrationCapability::new("lint/run", "Run linter")])
            .logs(["started", "warn: ruleset v3 missing"]),
        IntegrationEntry::new("tool-fmt", "formatter", IntegrationKind::Tool)
            .health(IntegrationHealth::UpdateAvailable)
            .provenance(IntegrationProvenance::first_party("3.1.0"))
            .summary("Update 3.2.0 available")
            .enabled(true),
        IntegrationEntry::new("ext-theme", "phosphor-pack", IntegrationKind::Extension)
            .health(IntegrationHealth::Disabled)
            .provenance(IntegrationProvenance::third_party(
                "tailrocks",
                "local:./ext/phosphor",
                "0.1",
            ))
            .enabled(false)
            .summary("Disabled by user"),
        IntegrationEntry::new("svc-ci", "ci-bridge", IntegrationKind::External)
            .health(IntegrationHealth::Error)
            .provenance(IntegrationProvenance::third_party(
                "ci-co",
                "https://api.ci.example",
                "1.0",
            ))
            .last_error("TLS handshake failed")
            .egress_warning("authenticates to external CI API")
            .logs(["connecting…", "error: TLS handshake failed"]),
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

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open() -> IntegrationStatusState {
        let mut st = IntegrationStatusState::new();
        st.set_entries(example_integrations());
        st.presentation = IntegrationStatusPresentation::CompactList;
        st
    }

    #[test]
    fn third_party_provenance_explicit() {
        let e = example_integrations()
            .into_iter()
            .find(|e| e.id == "mcp-web")
            .unwrap();
        assert!(e.provenance.third_party);
        assert!(e.provenance.summary_line().contains("3rd-party"));
        assert!(e.egress_line().unwrap().contains("egress") || e.may_egress());
    }

    #[test]
    fn select_and_restart() {
        let mut st = open();
        // first can restart
        let out = st.handle_key(press(KeyCode::Char('r')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::RestartRequested { ref id } if id == "mcp-fs"
        ));
    }

    #[test]
    fn permission_request_for_ungranted() {
        let mut st = open();
        let i = st.entries.iter().position(|e| e.id == "mcp-web").unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char('p')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::PermissionRequested {
                ref id,
                permission_id: Some(_)
            } if id == "mcp-web"
        ));
    }

    #[test]
    fn disable_enable() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('x')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::DisableRequested { ref id } if id == "mcp-fs"
        ));
        let i = st.entries.iter().position(|e| e.id == "ext-theme").unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char('e')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::EnableRequested { ref id } if id == "ext-theme"
        ));
    }

    #[test]
    fn panel_tabs() {
        let mut st = open();
        let _ = st.handle_key(press(KeyCode::Char('d')));
        assert_eq!(st.presentation, IntegrationStatusPresentation::Panel);
        let out = st.handle_key(press(KeyCode::Tab));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::TabChanged(IntegrationDetailTab::Capabilities)
        ));
    }

    #[test]
    fn badge_presentation() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('b')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::PresentationChanged(IntegrationStatusPresentation::Badge)
        ));
    }

    #[test]
    fn egress_warning_focus() {
        let mut st = open();
        let i = st.entries.iter().position(|e| e.id == "mcp-web").unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char('w')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::EgressWarningFocused { ref id } if id == "mcp-web"
        ));
    }

    #[test]
    fn update_request() {
        let mut st = open();
        let i = st.entries.iter().position(|e| e.id == "tool-fmt").unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char('u')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::UpdateRequested { ref id } if id == "tool-fmt"
        ));
    }

    #[test]
    fn y_unbound() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            IntegrationStatusOutcome::Ignored
        ));
    }

    #[test]
    fn no_process_io_safe_language() {
        let src = include_str!("integration_status.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "openai", "reqwest"] {
            assert!(!body.contains(f), "{f}");
        }
        assert!(body.contains("egress"));
        assert!(body.contains("third-party") || body.contains("3rd-party"));
        assert!(body.contains("never") || body.contains("host"));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            IntegrationStatusOutcome::Ignored
        ));
    }

    #[test]
    fn paint_all_presentations() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        for p in [
            IntegrationStatusPresentation::Badge,
            IntegrationStatusPresentation::CompactList,
            IntegrationStatusPresentation::Panel,
        ] {
            st.presentation = p;
            for tab in IntegrationDetailTab::cycle() {
                st.tab = *tab;
                IntegrationStatus::new(&system)
                    .ascii(true)
                    .colorless(true)
                    .paint(area, &mut buf, &mut st);
            }
        }
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.presentation = IntegrationStatusPresentation::Panel;
        let area = Rect::new(0, 0, 60, 18);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            IntegrationStatus::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_kinds_health() {
        for k in [
            IntegrationKind::McpServer,
            IntegrationKind::Plugin,
            IntegrationKind::Extension,
            IntegrationKind::Tool,
            IntegrationKind::External,
        ] {
            assert!(!k.id().is_empty());
        }
        for h in [
            IntegrationHealth::Connected,
            IntegrationHealth::Disconnected,
            IntegrationHealth::Starting,
            IntegrationHealth::Error,
            IntegrationHealth::PermissionRequired,
            IntegrationHealth::UpdateAvailable,
            IntegrationHealth::Disabled,
            IntegrationHealth::Degraded,
        ] {
            assert!(!h.id().is_empty());
            let _ = h.can_restart();
        }
    }

    #[test]
    fn mouse_select() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        IntegrationStatus::new(&system).paint(area, &mut buf, &mut st);
        assert!(!st.row_hits.is_empty());
        let (id, r) = st.row_hits[0].clone();
        let out = st.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        });
        assert!(
            matches!(out, IntegrationStatusOutcome::Selected { .. }),
            "{out:?} {id}"
        );
    }

    #[test]
    fn unicode_names() {
        let system = DesignSystem::default();
        let mut st = IntegrationStatusState::new();
        st.set_entries(vec![
            IntegrationEntry::new("u1", "検査 MCP 🔍", IntegrationKind::McpServer)
                .health(IntegrationHealth::Connected)
                .provenance(IntegrationProvenance::third_party(
                    "発行者",
                    "pkg:日本語",
                    "1.0",
                )),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        IntegrationStatus::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn logs_request() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('g')));
        assert!(matches!(
            out,
            IntegrationStatusOutcome::LogsRequested { ref id } if id == "mcp-fs"
        ));
    }

    #[test]
    fn aggregate_badge() {
        let st = open();
        let b = st.aggregate_badge();
        assert!(b.contains("integrations"));
        assert!(b.contains("attention") || b.contains("ok"));
    }
}
