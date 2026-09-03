// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ProjectLauncher** — fast project/session launcher for developer tools from
//! **public** TermRock widgets only (IDE welcome screens, zoxide/fzf, agent
//! session launchers as interaction *references* — not a clone API).
//!
//! **Mission.** Layout + focus + typed messages for recent projects, favorites,
//! workspaces, branches, sessions, remote/local status, search, grouping,
//! preview, open/new/import, and errors. Inline quick launcher and full-screen
//! home variants. Responsive density. **Discovery and persistence stay
//! host-owned** (no project scanners, history DBs, git processes, or network
//! probes inside this surface).
//!
//! **vs standalone [`QuickOpen`] / [`SessionPicker`] / [`EmptyState`].**
//! Composed, not re-painted.
//! **vs [`ConnectionManager`].** Uses product-neutral [`ConnectionStatus`] for
//! chrome; does not embed full connection inventory.
//!
//! Research: IDE welcome screens, zoxide/fzf workflows, agent session launchers.
//!
//! Teaches: how to compose fast project/session launcher for developer tools
//! from.
//!
//! Composes: [`crate::widgets::EmptyAction`], [`crate::widgets::EmptyKind`],
//! [`crate::widgets::EmptyState`], [`crate::widgets::EmptyStateOutcome`],
//! [`crate::widgets::EmptyStateState`], [`crate::widgets::List`],
//! [`crate::widgets::ListRow`], [`crate::widgets::ListState`], and 21 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::Outcome,
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    patterns::{
        ConnectionStatus, SessionEntry, SessionPicker, SessionPickerOutcome, SessionPickerState,
    },
    style::{DesignSystem, PanelChrome},
    widgets::{
        EmptyAction, EmptyKind, EmptyState, EmptyStateState, List, ListRow, ListState, Panel,
        PreviewCard, PreviewCardContent, PreviewCardState, PreviewLoadState, PreviewMetadata,
        PreviewResourceKind, QuickOpen, QuickOpenItem, QuickOpenMatch, QuickOpenOutcome,
        QuickOpenProvider, QuickOpenState, SearchInput, SearchInputOutcome, SearchInputState,
        SemanticStatus, StatusBar, StatusBarState, StatusRegion, StatusSlot,
    },
};

// ── Panes, mode, density ────────────────────────────────────────────────────

/// Named panes of the project launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProjectLauncherPane {
    /// Search / filter bar.
    Search,
    /// Grouped project list (favorites / recent / workspaces).
    Projects,
    /// Session picker (home mode).
    Sessions,
    /// Selection preview.
    Preview,
    /// Onboarding / empty callout.
    Onboarding,
    /// Status strip.
    Status,
}

impl ProjectLauncherPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Projects => "projects",
            Self::Sessions => "sessions",
            Self::Preview => "preview",
            Self::Onboarding => "onboarding",
            Self::Status => "status",
        }
    }

    /// Default Tab focus cycle (status is chrome-only).
    #[must_use]
    pub fn focus_order() -> &'static [ProjectLauncherPane] {
        &[
            Self::Search,
            Self::Projects,
            Self::Sessions,
            Self::Preview,
            Self::Onboarding,
        ]
    }
}

/// Presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProjectLauncherMode {
    /// Full-screen home (projects + sessions + preview + onboarding).
    #[default]
    Home,
    /// Compact inline quick launcher (search + projects + status).
    Inline,
}

impl ProjectLauncherMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Inline => "inline",
        }
    }
}

/// Responsive density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProjectLauncherDensity {
    /// Full multi-pane home / expanded inline.
    #[default]
    Normal,
    /// Collapse preview; keep projects + sessions when home.
    Narrow,
    /// Search + projects + status only.
    Tiny,
}

impl ProjectLauncherDensity {
    /// From terminal width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 52 {
            Self::Tiny
        } else if width < 96 {
            Self::Narrow
        } else {
            Self::Normal
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Narrow => "narrow",
            Self::Tiny => "tiny",
        }
    }
}

// ── Domain projections (host-owned) ─────────────────────────────────────────

/// List group for project rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProjectGroup {
    /// Pinned favorites.
    Favorite,
    /// Recent opens.
    #[default]
    Recent,
    /// Multi-root / workspace bundle.
    Workspace,
}

impl ProjectGroup {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Favorite => "favorite",
            Self::Recent => "recent",
            Self::Workspace => "workspace",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Favorite => "Favorites",
            Self::Recent => "Recent",
            Self::Workspace => "Workspaces",
        }
    }

    /// Sort order (favorites first).
    #[must_use]
    pub const fn sort_key(self) -> u8 {
        match self {
            Self::Favorite => 0,
            Self::Workspace => 1,
            Self::Recent => 2,
        }
    }
}

/// Path / location health (host-projected; no FS probe here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProjectPathStatus {
    /// Path present and ready.
    #[default]
    Ok,
    /// Missing / moved.
    Missing,
    /// Stale index / outdated metadata.
    Stale,
    /// Host error (permission, I/O).
    Error,
}

impl ProjectPathStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Error => "error",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Error => "error",
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Ok => SemanticStatus::Success,
            Self::Missing | Self::Error => SemanticStatus::Failed,
            Self::Stale => SemanticStatus::Warning,
        }
    }

    /// Whether chrome should warn.
    #[must_use]
    pub const fn is_problem(self) -> bool {
        !matches!(self, Self::Ok)
    }
}

/// Local vs remote project placement (host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProjectLocation {
    /// Local filesystem.
    #[default]
    Local,
    /// Remote / cloud workspace.
    Remote,
}

impl ProjectLocation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// Host-projected project / workspace row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEntry {
    /// Stable id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Path or URI (host truth).
    pub path: String,
    /// Group.
    pub group: ProjectGroup,
    /// Optional branch label.
    pub branch: Option<String>,
    /// Local / remote.
    pub location: ProjectLocation,
    /// Path health.
    pub path_status: ProjectPathStatus,
    /// Favorite pin.
    pub favorite: bool,
    /// Optional remote connection chrome.
    pub connection: ConnectionStatus,
    /// Optional recency label (`2h ago`).
    pub recency: Option<String>,
    /// Optional host error message.
    pub error: Option<String>,
}

impl ProjectEntry {
    /// Construct ok local project.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            group: ProjectGroup::Recent,
            branch: None,
            location: ProjectLocation::Local,
            path_status: ProjectPathStatus::Ok,
            favorite: false,
            connection: ConnectionStatus::Disconnected,
            recency: None,
            error: None,
        }
    }

    /// Group.
    #[must_use]
    pub const fn group(mut self, g: ProjectGroup) -> Self {
        self.group = g;
        self
    }

    /// Branch.
    #[must_use]
    pub fn branch(mut self, b: impl Into<String>) -> Self {
        self.branch = Some(b.into());
        self
    }

    /// Location.
    #[must_use]
    pub const fn location(mut self, loc: ProjectLocation) -> Self {
        self.location = loc;
        self
    }

    /// Path status.
    #[must_use]
    pub const fn path_status(mut self, s: ProjectPathStatus) -> Self {
        self.path_status = s;
        self
    }

    /// Favorite.
    #[must_use]
    pub const fn favorite(mut self, on: bool) -> Self {
        self.favorite = on;
        if on {
            self.group = ProjectGroup::Favorite;
        }
        self
    }

    /// Connection.
    #[must_use]
    pub const fn connection(mut self, c: ConnectionStatus) -> Self {
        self.connection = c;
        self
    }

    /// Recency.
    #[must_use]
    pub fn recency(mut self, r: impl Into<String>) -> Self {
        self.recency = Some(r.into());
        self
    }

    /// Error message (+ error status).
    #[must_use]
    pub fn error_msg(mut self, m: impl Into<String>) -> Self {
        self.error = Some(m.into());
        self.path_status = ProjectPathStatus::Error;
        self
    }

    /// Query match (name / path / branch).
    #[must_use]
    pub fn matches_query(&self, q: &str) -> bool {
        let q = q.trim().to_ascii_lowercase();
        if q.is_empty() {
            return true;
        }
        crate::text::contains_lower_all(
            &[
                &self.name,
                &self.path,
                self.branch.as_deref().unwrap_or(""),
                self.group.id(),
            ],
            &q,
        )
    }
}

/// Filter projects by query; preserve group order (favorites → workspaces → recent).
#[must_use]
pub fn filter_project_entries<'a>(
    entries: &'a [ProjectEntry],
    query: &str,
) -> Vec<&'a ProjectEntry> {
    let mut v: Vec<&ProjectEntry> = entries.iter().filter(|e| e.matches_query(query)).collect();
    v.sort_by(|a, b| {
        a.group
            .sort_key()
            .cmp(&b.group.sort_key())
            .then_with(|| a.name.cmp(&b.name))
    });
    v
}

/// Build list rows with group headers.
#[must_use]
pub fn project_list_rows<'a>(entries: &[&'a ProjectEntry]) -> Vec<ListRow<'a, String>> {
    let mut rows = Vec::new();
    let mut last_group: Option<ProjectGroup> = None;
    for e in entries {
        if last_group != Some(e.group) {
            rows.push(ListRow::group_header(
                format!("g-{}", e.group.id()),
                Line::from(e.group.label()),
            ));
            last_group = Some(e.group);
        }
        let mut label = e.name.clone();
        if let Some(b) = &e.branch {
            label = format!("{label} · {b}");
        }
        if e.path_status.is_problem() {
            let status = e.path_status.semantic();
            label = format!(
                "| {} {} · {label}",
                status.glyph_ascii(),
                e.path_status.label()
            );
        }
        let mut row = ListRow::item(e.id.clone(), Line::from(label));
        let mut secondary = e.path.clone();
        if let Some(r) = &e.recency {
            secondary = format!("{secondary} · {r}");
        }
        if e.location == ProjectLocation::Remote {
            secondary = format!("{secondary} · {}", e.connection.label());
        }
        if let Some(err) = &e.error {
            secondary = format!("{secondary} · {err}");
        }
        row = row.secondary(Line::from(secondary));
        rows.push(row);
    }
    rows
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Launcher outcomes — requests only; host owns discovery/persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectLauncherOutcome {
    /// Ignored.
    Ignored,
    /// Focus pane changed.
    FocusChanged(&'static str),
    /// Presentation mode changed.
    ModeChanged(ProjectLauncherMode),
    /// Open / resume project.
    OpenRequested {
        /// Project id.
        id: String,
    },
    /// Create new project.
    NewRequested,
    /// Import / open folder / clone entry.
    ImportRequested,
    /// Favorite pin toggled (host persists).
    FavoriteToggled {
        /// Project id.
        id: String,
        /// On after toggle.
        on: bool,
    },
    /// Session resume.
    SessionResume {
        /// Session id.
        id: String,
    },
    /// Session create.
    SessionCreate {
        /// Title draft.
        title: String,
    },
    /// Search / filter changed.
    FilterChanged {
        /// Query.
        query: String,
    },
    /// Selection moved in project list.
    SelectionChanged {
        /// Project id.
        id: String,
    },
    /// Quick open palette opened.
    QuickOpenOpened,
    /// Quick open closed.
    QuickOpenClosed,
    /// Quick open item activated.
    QuickOpenActivated {
        /// Id.
        id: String,
    },
    /// Onboarding / setup wizard entry.
    OnboardingRequested,
    /// Stale/missing chrome acknowledged.
    StaleAck {
        /// Project id if scoped.
        id: Option<String>,
    },
    /// Retry host discovery / reload.
    ReloadRequested,
    /// Esc / cancel.
    Cancelled,
    /// Child residual kind.
    Child {
        /// Kind label.
        kind: String,
    },
}

// ── Surfaces ────────────────────────────────────────────────────────────────

/// Borrowed surfaces for one paint frame.
pub struct ProjectLauncherSurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut ProjectLauncherState,
    /// Host-projected projects.
    pub projects: &'a [ProjectEntry],
    /// Host-projected sessions (home mode).
    pub sessions: &'a [SessionEntry],
    /// Host-projected preview for current selection.
    pub preview: Option<PreviewCardContent<'a>>,
    /// Quick-open items (when palette open).
    pub quick_open_items: &'a [QuickOpenMatch<'a, String>],
}

// ── State ───────────────────────────────────────────────────────────────────

/// Persistent project launcher state.
#[derive(Debug)]
pub struct ProjectLauncherState {
    /// Workspace collapse.
    pub workspace: WorkspaceState,
    /// Global search / filter.
    pub search: SearchInputState,
    /// Project list.
    pub projects: ListState<String>,
    /// Session picker (home).
    pub sessions: SessionPickerState,
    /// Preview card.
    pub preview: PreviewCardState,
    /// Onboarding empty state interaction.
    pub onboarding: EmptyStateState,
    /// Quick open palette (inline fast path or home C-o).
    pub quick_open: QuickOpenState<String>,
    /// Status bar.
    pub status: StatusBarState<&'static str>,
    /// Presentation mode.
    pub mode: ProjectLauncherMode,
    /// Whether quick open overlay is open.
    pub quick_open_open: bool,
    /// Show onboarding pane (host: empty catalog or first-run).
    pub show_onboarding: bool,
    /// Host aggregate connection chrome.
    pub connection: ConnectionStatus,
    /// Host error banner (discovery failure).
    pub host_error: Option<String>,
    /// Focused pane id.
    pub focus: &'static str,
    /// Density override (`None` = width-derived).
    pub density: Option<ProjectLauncherDensity>,
    /// Selected project id.
    pub selected_id: Option<String>,
    /// Project count chrome.
    /// Stale/missing count chrome.
    pub problem_count: u64,
    /// Colorless.
    pub colorless: bool,
    /// Last panes.
    last_panes: Vec<PaneGeom>,
    /// Last paint width for density=None.
    last_area_width: Option<u16>,
}

impl Default for ProjectLauncherState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectLauncherState {
    /// Fresh home launcher.
    #[must_use]
    pub fn new() -> Self {
        let mut search = SearchInputState::new();
        search.set_focused(false);
        let mut sessions = SessionPickerState::new();
        sessions.set_accepts_input(false);
        let mut quick_open = QuickOpenState::new();
        quick_open.set_accepts_input(false);
        quick_open.set_focused(false);
        let mut onboarding = EmptyStateState::new();
        onboarding.focus_primary();
        Self {
            workspace: WorkspaceState::new(),
            search,
            projects: ListState::new(None),
            sessions,
            preview: PreviewCardState::new(),
            onboarding,
            quick_open,
            status: StatusBarState::new(),
            mode: ProjectLauncherMode::Home,
            quick_open_open: false,
            show_onboarding: false,
            connection: ConnectionStatus::Disconnected,
            host_error: None,
            focus: ProjectLauncherPane::Projects.id(),
            density: None,
            selected_id: None,
            problem_count: 0,
            colorless: false,
            last_panes: Vec::new(),
            last_area_width: None,
        }
    }

    /// Inline quick launcher factory.
    #[must_use]
    pub fn inline() -> Self {
        let mut s = Self::new();
        s.mode = ProjectLauncherMode::Inline;
        s.focus = ProjectLauncherPane::Search.id();
        s
    }

    /// Last panes.
    #[must_use]
    pub fn last_panes(&self) -> &[PaneGeom] {
        &self.last_panes
    }

    /// Effective density.
    #[must_use]
    pub fn effective_density(&self) -> ProjectLauncherDensity {
        self.density.unwrap_or_else(|| {
            ProjectLauncherDensity::for_width(self.last_area_width.unwrap_or(120))
        })
    }

    /// Visible focusable panes for density + mode.
    #[must_use]
    pub fn visible_focus_panes(&self, density: ProjectLauncherDensity) -> Vec<ProjectLauncherPane> {
        match (self.mode, density) {
            (ProjectLauncherMode::Inline, _) => {
                vec![ProjectLauncherPane::Search, ProjectLauncherPane::Projects]
            }
            (ProjectLauncherMode::Home, ProjectLauncherDensity::Tiny) => {
                vec![ProjectLauncherPane::Search, ProjectLauncherPane::Projects]
            }
            (ProjectLauncherMode::Home, ProjectLauncherDensity::Narrow) => {
                let mut v = vec![
                    ProjectLauncherPane::Search,
                    ProjectLauncherPane::Projects,
                    ProjectLauncherPane::Sessions,
                ];
                if self.show_onboarding {
                    v.push(ProjectLauncherPane::Onboarding);
                }
                v
            }
            (ProjectLauncherMode::Home, ProjectLauncherDensity::Normal) => {
                let mut v = vec![
                    ProjectLauncherPane::Search,
                    ProjectLauncherPane::Projects,
                    ProjectLauncherPane::Sessions,
                    ProjectLauncherPane::Preview,
                ];
                if self.show_onboarding {
                    v.push(ProjectLauncherPane::Onboarding);
                }
                v
            }
        }
    }

    /// Clamp focus to density-visible panes.
    pub fn clamp_focus_to_density(&mut self, density: ProjectLauncherDensity) {
        let visible = self.visible_focus_panes(density);
        if !visible.iter().any(|p| p.id() == self.focus) {
            self.focus = visible
                .first()
                .map(|p| p.id())
                .unwrap_or(ProjectLauncherPane::Projects.id());
        }
    }

    /// Sync child accept/focus gates.
    pub fn apply_focus_gates(&mut self) {
        let f = self.focus;
        let qo = self.quick_open_open;
        self.search.set_focused(f == "search" && !qo);
        // List has no set_focused; paint uses focused flag
        let sessions_on = f == "sessions" && !qo;
        self.sessions.set_accepts_input(sessions_on);
        self.sessions.set_focused(sessions_on);
        self.preview.set_focus_within(f == "preview" && !qo);
        self.quick_open.set_accepts_input(qo);
        self.quick_open.set_focused(qo);
        if f == "onboarding" && !qo {
            self.onboarding.focus_primary();
        }
    }

    /// Set focus pane.
    pub fn set_focus(&mut self, pane: ProjectLauncherPane) -> ProjectLauncherOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if !visible.contains(&pane) {
            return ProjectLauncherOutcome::Ignored;
        }
        if self.focus == pane.id() {
            self.apply_focus_gates();
            return ProjectLauncherOutcome::Ignored;
        }
        self.focus = pane.id();
        self.apply_focus_gates();
        ProjectLauncherOutcome::FocusChanged(self.focus)
    }

    /// Cycle Tab focus.
    pub fn cycle_focus(&mut self, reverse: bool) -> ProjectLauncherOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if visible.is_empty() {
            return ProjectLauncherOutcome::Ignored;
        }
        let cur = visible
            .iter()
            .position(|p| p.id() == self.focus)
            .unwrap_or(0);
        let next = if reverse {
            if cur == 0 { visible.len() - 1 } else { cur - 1 }
        } else {
            (cur + 1) % visible.len()
        };
        self.focus = visible[next].id();
        self.apply_focus_gates();
        ProjectLauncherOutcome::FocusChanged(self.focus)
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: ProjectLauncherMode) -> ProjectLauncherOutcome {
        if self.mode == mode {
            return ProjectLauncherOutcome::Ignored;
        }
        self.mode = mode;
        let density = self.effective_density();
        self.clamp_focus_to_density(density);
        self.apply_focus_gates();
        ProjectLauncherOutcome::ModeChanged(mode)
    }

    /// Open quick open palette.
    pub fn open_quick_open(&mut self) -> ProjectLauncherOutcome {
        self.quick_open_open = true;
        self.quick_open.set_accepts_input(true);
        self.quick_open.set_focused(true);
        ProjectLauncherOutcome::QuickOpenOpened
    }

    /// Close quick open.
    pub fn close_quick_open(&mut self) {
        self.quick_open_open = false;
        self.quick_open.set_accepts_input(false);
        self.quick_open.set_focused(false);
        self.apply_focus_gates();
    }

    /// Status slots.
    #[must_use]
    pub fn status_slots(&self) -> Vec<StatusSlot<'static, &'static str>> {
        let mut slots = vec![
            StatusSlot::connection("conn", self.connection.label())
                .semantic(self.connection.semantic())
                .priority(90),
            StatusSlot::context("mode", self.mode.id()).priority(50),
            StatusSlot::focus_zone("focus", self.focus).priority(70),
            StatusSlot::shortcut("keys", "enter open · n new · i import · f fav · C-o quick")
                .priority(10),
        ];
        if self.problem_count > 0 {
            slots.push(
                StatusSlot::new("problems", "stale/missing")
                    .semantic(crate::widgets::SemanticStatus::Warning)
                    .region(StatusRegion::Left)
                    .priority(95),
            );
        }
        if self.host_error.is_some() {
            slots.push(
                StatusSlot::new("err", "error")
                    .semantic(crate::widgets::SemanticStatus::Failed)
                    .region(StatusRegion::Left)
                    .priority(100),
            );
        }
        slots
    }

    /// Keys — real launcher path.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        projects: &[ProjectEntry],
        sessions: &[SessionEntry],
        quick_open_items: &[QuickOpenMatch<'_, String>],
    ) -> ProjectLauncherOutcome {
        if key.is_release() {
            return ProjectLauncherOutcome::Ignored;
        }
        let is_press = key.is_press();

        // Quick open overlay first
        if self.quick_open_open {
            if is_press && key.code == KeyCode::Esc {
                self.close_quick_open();
                return ProjectLauncherOutcome::QuickOpenClosed;
            }
            let providers = default_project_quick_open_providers();
            let out = self
                .quick_open
                .handle_key(key, &providers, quick_open_items);
            return match out {
                QuickOpenOutcome::Ignored => ProjectLauncherOutcome::Ignored,
                QuickOpenOutcome::Activated { id, .. } => {
                    self.close_quick_open();
                    ProjectLauncherOutcome::QuickOpenActivated { id }
                }
                QuickOpenOutcome::Cancelled => {
                    self.close_quick_open();
                    ProjectLauncherOutcome::QuickOpenClosed
                }
                other => {
                    let kind = format!("{other:?}")
                        .split(|c: char| c == '(' || c == ' ')
                        .next()
                        .unwrap_or("quick-open")
                        .to_string();
                    ProjectLauncherOutcome::Child { kind }
                }
            };
        }

        if is_press {
            match key.code {
                KeyCode::Tab if key.modifiers.is_empty() => {
                    return self.cycle_focus(false);
                }
                KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.cycle_focus(true);
                }
                KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return self.open_quick_open();
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // New session shortcut (global when not typing search)
                    if self.focus != "search" {
                        return ProjectLauncherOutcome::SessionCreate {
                            title: String::new(),
                        };
                    }
                }
                KeyCode::Esc => {
                    return ProjectLauncherOutcome::Cancelled;
                }
                _ => {}
            }
        }

        match self.focus {
            "search" => self.handle_search_key(key),
            "projects" => self.handle_projects_key(key, projects),
            "sessions" => self.handle_sessions_key(key, sessions),
            "preview" => ProjectLauncherOutcome::Ignored,
            "onboarding" => self.handle_onboarding_key(key),
            _ => ProjectLauncherOutcome::Ignored,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> ProjectLauncherOutcome {
        let out = self.search.handle_key(key);
        match out {
            SearchInputOutcome::Ignored => ProjectLauncherOutcome::Ignored,
            SearchInputOutcome::DebouncedQuery { query }
            | SearchInputOutcome::Submitted { query } => {
                ProjectLauncherOutcome::FilterChanged { query }
            }
            SearchInputOutcome::Changed | SearchInputOutcome::HistoryRecalled { .. } => {
                let query = self.search.query().to_string();
                ProjectLauncherOutcome::FilterChanged { query }
            }
            SearchInputOutcome::Cleared => ProjectLauncherOutcome::FilterChanged {
                query: String::new(),
            },
            SearchInputOutcome::Cancelled => ProjectLauncherOutcome::Cancelled,
            other => {
                let kind = format!("{other:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("search")
                    .to_string();
                ProjectLauncherOutcome::Child { kind }
            }
        }
    }

    fn handle_projects_key(
        &mut self,
        key: KeyEvent,
        projects: &[ProjectEntry],
    ) -> ProjectLauncherOutcome {
        let is_press = key.is_press();
        let query = self.search.query().to_string();
        let filtered = filter_project_entries(projects, &query);
        let rows = project_list_rows(&filtered);

        if is_press && key.modifiers.is_empty() {
            match key.code {
                KeyCode::Char('n') => {
                    return ProjectLauncherOutcome::NewRequested;
                }
                KeyCode::Char('i') => {
                    return ProjectLauncherOutcome::ImportRequested;
                }
                KeyCode::Char('f') => {
                    if let Some(id) = self
                        .projects
                        .selected()
                        .cloned()
                        .or_else(|| filtered.first().map(|e| e.id.clone()))
                    {
                        let on = filtered
                            .iter()
                            .find(|e| e.id == id)
                            .map(|e| !e.favorite)
                            .unwrap_or(true);
                        return ProjectLauncherOutcome::FavoriteToggled { id, on };
                    }
                }
                KeyCode::Char('a') => {
                    // Ack stale/missing for selection
                    let id = self.projects.selected().cloned();
                    return ProjectLauncherOutcome::StaleAck { id };
                }
                KeyCode::Char('r') if key.modifiers.is_empty() => {
                    return ProjectLauncherOutcome::ReloadRequested;
                }
                KeyCode::Enter => {
                    if let Some(id) = self
                        .projects
                        .selected()
                        .cloned()
                        .or_else(|| filtered.first().map(|e| e.id.clone()))
                    {
                        self.selected_id = Some(id.clone());
                        return ProjectLauncherOutcome::OpenRequested { id };
                    }
                }
                _ => {}
            }
        }

        let out = self.projects.handle_key(&rows, key);
        match out {
            Outcome::Ignored => ProjectLauncherOutcome::Ignored,
            Outcome::Changed => {
                let id = self.projects.selected().cloned().unwrap_or_default();
                if id.is_empty() || id.starts_with("g-") {
                    return ProjectLauncherOutcome::Ignored;
                }
                self.selected_id = Some(id.clone());
                let _ = self.preview.set_selection(id.clone());
                ProjectLauncherOutcome::SelectionChanged { id }
            }
            Outcome::Activated(id) => {
                if id.starts_with("g-") {
                    return ProjectLauncherOutcome::Ignored;
                }
                self.selected_id = Some(id.clone());
                let _ = self.preview.set_selection(id.clone());
                ProjectLauncherOutcome::OpenRequested { id }
            }
            Outcome::Cancelled => ProjectLauncherOutcome::Cancelled,
            Outcome::CheckToggled(id) => ProjectLauncherOutcome::SelectionChanged { id },
        }
    }

    fn handle_sessions_key(
        &mut self,
        key: KeyEvent,
        sessions: &[SessionEntry],
    ) -> ProjectLauncherOutcome {
        // Keep session catalog projected
        if self.sessions.sessions.is_empty() && !sessions.is_empty() {
            self.sessions.set_sessions(sessions.to_vec());
        }
        let out = self.sessions.handle_key(key);
        match out {
            SessionPickerOutcome::Ignored => ProjectLauncherOutcome::Ignored,
            SessionPickerOutcome::Opened { id } => ProjectLauncherOutcome::SessionResume { id },
            SessionPickerOutcome::CreateRequested { title } => {
                ProjectLauncherOutcome::SessionCreate { title }
            }
            SessionPickerOutcome::QueryChanged { query, .. } => {
                ProjectLauncherOutcome::FilterChanged { query }
            }
            SessionPickerOutcome::Selected { id } => {
                ProjectLauncherOutcome::SelectionChanged { id }
            }
            SessionPickerOutcome::Cancelled => ProjectLauncherOutcome::Cancelled,
            other => {
                let kind = format!("{other:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("sessions")
                    .to_string();
                ProjectLauncherOutcome::Child { kind }
            }
        }
    }

    fn handle_onboarding_key(&mut self, key: KeyEvent) -> ProjectLauncherOutcome {
        // EmptyState needs the widget for handle_key — use a minimal system-free path:
        // Enter/primary → onboarding or new; secondary → import
        if !key.is_press() {
            return ProjectLauncherOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char('n') => ProjectLauncherOutcome::NewRequested,
            KeyCode::Char('i') | KeyCode::Char('o') => ProjectLauncherOutcome::ImportRequested,
            KeyCode::Char('s') => ProjectLauncherOutcome::OnboardingRequested,
            KeyCode::Tab => {
                // cycle primary/secondary inside empty
                ProjectLauncherOutcome::Ignored
            }
            _ => ProjectLauncherOutcome::Ignored,
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Search strip height.
pub const PROJECT_LAUNCHER_SEARCH_HEIGHT: u16 = 3;

/// Explicit density + mode layout.
#[must_use]
pub fn project_launcher_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: ProjectLauncherDensity,
    mode: ProjectLauncherMode,
    show_onboarding: bool,
) -> Vec<PaneGeom> {
    let mut panes = Vec::new();
    let mut y = area.y;
    let mut remain = area.height;

    // Search strip
    let search_h = if remain >= 3 {
        PROJECT_LAUNCHER_SEARCH_HEIGHT.min(remain.saturating_sub(2))
    } else if remain >= 1 {
        1
    } else {
        0
    };
    panes.push(PaneGeom {
        id: PaneId::from_static(ProjectLauncherPane::Search.id()),
        area: if search_h == 0 {
            Rect::new(area.x, y, 0, 0)
        } else {
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: search_h,
            }
        },
        collapsed: search_h == 0,
    });
    y = y.saturating_add(search_h);
    remain = remain.saturating_sub(search_h);

    let body = Rect {
        x: area.x,
        y,
        width: area.width,
        height: remain,
    };

    let root = match (mode, density) {
        (ProjectLauncherMode::Inline, _) | (_, ProjectLauncherDensity::Tiny) => {
            // projects | status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 92,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ProjectLauncherPane::Projects.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ProjectLauncherPane::Status.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 3,
                }),
            }
        }
        (ProjectLauncherMode::Home, ProjectLauncherDensity::Narrow) => {
            // projects | sessions | (optional onboarding) | status — no preview
            if show_onboarding {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 88,
                    first: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Horizontal,
                        ratio_percent: 50,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ProjectLauncherPane::Projects.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                        second: Box::new(WorkspaceNode::Split {
                            axis: WorkspaceAxis::Vertical,
                            ratio_percent: 70,
                            first: Box::new(WorkspaceNode::Leaf {
                                id: PaneId::from_static(ProjectLauncherPane::Sessions.id()),
                                constraint: PaneConstraint::Weight(1),
                                collapse_priority: 1,
                            }),
                            second: Box::new(WorkspaceNode::Leaf {
                                id: PaneId::from_static(ProjectLauncherPane::Onboarding.id()),
                                constraint: PaneConstraint::Min(4),
                                collapse_priority: 0,
                            }),
                        }),
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ProjectLauncherPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
            } else {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 92,
                    first: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Horizontal,
                        ratio_percent: 50,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ProjectLauncherPane::Projects.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ProjectLauncherPane::Sessions.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ProjectLauncherPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
            }
        }
        (ProjectLauncherMode::Home, ProjectLauncherDensity::Normal) => {
            // (projects | sessions | preview) / optional onboarding / status
            let main = WorkspaceNode::Split {
                axis: WorkspaceAxis::Horizontal,
                ratio_percent: 40,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ProjectLauncherPane::Projects.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 55,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ProjectLauncherPane::Sessions.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ProjectLauncherPane::Preview.id()),
                        constraint: PaneConstraint::Min(18),
                        collapse_priority: 0,
                    }),
                }),
            };
            if show_onboarding {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 78,
                    first: Box::new(main),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 70,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ProjectLauncherPane::Onboarding.id()),
                            constraint: PaneConstraint::Min(4),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ProjectLauncherPane::Status.id()),
                            constraint: PaneConstraint::Fixed(1),
                            collapse_priority: 3,
                        }),
                    }),
                }
            } else {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 92,
                    first: Box::new(main),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ProjectLauncherPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
            }
        }
    };

    panes.extend(Workspace::new(root).layout(body, state));
    panes
}

fn pane_area(panes: &[PaneGeom], id: &str) -> Option<Rect> {
    panes.iter().find_map(|p| {
        if p.id.0.as_str() == id && !p.collapsed && p.area.width > 0 && p.area.height > 0 {
            Some(p.area)
        } else {
            None
        }
    })
}

/// Default QuickOpen providers for project palette.
#[must_use]
pub fn default_project_quick_open_providers() -> Vec<QuickOpenProvider> {
    vec![
        QuickOpenProvider::new("projects", "Projects"),
        QuickOpenProvider::new("sessions", "Sessions"),
    ]
}

/// Quick open rect (center).
#[must_use]
pub fn project_quick_open_rect(area: Rect) -> Rect {
    let w = area.width.min(72).max(32.min(area.width));
    let h = area.height.min(18).max(8.min(area.height));
    let x = area.x.saturating_add(area.width.saturating_sub(w) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}

// ── Render ──────────────────────────────────────────────────────────────────

/// Paint composed project launcher (public child widgets only).
pub fn paint_project_launcher(
    buffer: &mut Buffer,
    area: Rect,
    surfaces: ProjectLauncherSurfaces<'_>,
) {
    let ProjectLauncherSurfaces {
        system,
        state,
        projects,
        sessions,
        preview,
        quick_open_items,
    } = surfaces;

    if area.is_empty() {
        return;
    }

    state.last_area_width = Some(area.width);
    let density = state.effective_density();
    // Host keeps full control of show_onboarding; the block never auto-opens.
    let panes = project_launcher_layout_density(
        area,
        &state.workspace,
        density,
        state.mode,
        state.show_onboarding,
    );
    state.last_panes = panes.clone();
    state.clamp_focus_to_density(density);
    state.apply_focus_gates();
    state.problem_count = projects
        .iter()
        .filter(|p| p.path_status.is_problem())
        .count() as u64;

    // Sync sessions into picker when home
    if matches!(state.mode, ProjectLauncherMode::Home) && !sessions.is_empty() {
        if state.sessions.sessions.len() != sessions.len() {
            state.sessions.set_sessions(sessions.to_vec());
        }
    }

    let query = state.search.query().to_string();
    let filtered = filter_project_entries(projects, &query);

    // Search
    if let Some(r) = pane_area(&panes, "search") {
        let focused = state.focus == "search" && !state.quick_open_open;
        state.search.set_focused(focused);
        if r.height >= 3 {
            let inner = Panel::new(system)
                .title(match state.mode {
                    ProjectLauncherMode::Home => "Projects · home",
                    ProjectLauncherMode::Inline => "Quick open · inline",
                })
                .emphasis(PanelChrome::for_focus(focused))
                .paint(r, buffer, None);
            if !inner.is_empty() {
                SearchInput::new(system)
                    .placeholder("filter projects…")
                    .paint(inner, buffer, &mut state.search);
            }
        } else if !r.is_empty() {
            SearchInput::new(system)
                .placeholder("filter projects…")
                .paint(r, buffer, &mut state.search);
        }
    }

    // Projects list
    if let Some(r) = pane_area(&panes, "projects") {
        let focused = state.focus == "projects" && !state.quick_open_open;
        let inner = Panel::new(system)
            .title("Projects")
            .emphasis(PanelChrome::for_focus(focused))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            let rows = project_list_rows(&filtered);
            if rows.is_empty() {
                // An empty pane says what is missing and what to do next
                // (plans/013 Step 4).
                let empty = if projects.is_empty() {
                    EmptyState::new("No projects", system)
                        .kind(EmptyKind::NoData)
                        .explanation("n new, i import")
                } else {
                    EmptyState::new("No matches", system).kind(EmptyKind::FilteredOut)
                };
                let mut empty_state = crate::widgets::EmptyStateState::new();
                empty.paint(inner, buffer, &mut empty_state);
            } else {
                let list = List::new(&rows, system).focused(focused);
                StatefulWidget::render(&list, inner, buffer, &mut state.projects);
            }
        }
    }

    // Sessions
    if let Some(r) = pane_area(&panes, "sessions") {
        let focused = state.focus == "sessions" && !state.quick_open_open;
        state.sessions.set_accepts_input(focused);
        state.sessions.set_focused(focused);
        SessionPicker::new(system)
            .colorless(state.colorless)
            .list_only(true)
            .paint(r, buffer, &mut state.sessions);
    }

    // Preview
    if let Some(r) = pane_area(&panes, "preview") {
        let focused = state.focus == "preview" && !state.quick_open_open;
        state.preview.set_focus_within(focused);
        let content = preview.unwrap_or_else(|| {
            PreviewCardContent::title("(select a project)", PreviewResourceKind::File)
                .load(PreviewLoadState::Idle)
                .essential_elsewhere(true)
        });
        let inner = Panel::new(system)
            .title("Preview")
            .emphasis(PanelChrome::for_focus(focused))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            PreviewCard::new(content, system).paint(inner, buffer, &mut state.preview);
        }
    }

    // Onboarding
    if let Some(r) = pane_area(&panes, "onboarding") {
        let focused = state.focus == "onboarding" && !state.quick_open_open;
        if focused {
            state.onboarding.focus_primary();
        }
        let empty = EmptyState::new("Get started", system)
            .kind(EmptyKind::FirstUse)
            .explanation("Create a project, import a folder, or open Setup.")
            .primary(EmptyAction::with_shortcut("New project", "n"))
            .secondary(EmptyAction::with_shortcut("Import…", "i"))
            .shortcut("n new · i import · s setup");
        empty.paint(r, buffer, &mut state.onboarding);
    }

    // Status
    if let Some(r) = pane_area(&panes, "status") {
        if let Some(err) = &state.host_error {
            // Severity is the status role's job; the line spends its cells on
            // what actually failed.
            state.status.transient = Some(err.clone());
        } else if state.problem_count > 0 {
            state.status.transient = Some(format!(
                "{} stale/missing · a ack · r reload",
                state.problem_count
            ));
        } else {
            state.status.transient = None;
        }
        let slots = state.status_slots();
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], system),
            r,
            buffer,
            &mut state.status,
        );
    }

    // Quick open overlay
    if state.quick_open_open {
        let qo = project_quick_open_rect(area);
        if !qo.is_empty() {
            let providers = default_project_quick_open_providers();
            QuickOpen::new(&providers, quick_open_items, system).paint(
                qo,
                buffer,
                &mut state.quick_open,
            );
        }
    }
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Example project catalog.
#[must_use]
pub fn example_projects() -> Vec<ProjectEntry> {
    vec![
        ProjectEntry::new("p-termrock", "termrock", "~/src/termrock")
            .favorite(true)
            .branch("main")
            .recency("now")
            .connection(ConnectionStatus::Connected),
        ProjectEntry::new("p-ws", "workspace: tailrocks", "~/src/tailrocks")
            .group(ProjectGroup::Workspace)
            .branch("develop")
            .recency("1h"),
        ProjectEntry::new("p-api", "api-service", "~/src/api-service")
            .branch("feat/auth")
            .recency("2d"),
        ProjectEntry::new("p-gone", "old-repo", "~/src/old-repo")
            .path_status(ProjectPathStatus::Missing)
            .recency("3w"),
        ProjectEntry::new("p-stale", "stale-index", "~/src/stale")
            .path_status(ProjectPathStatus::Stale)
            .recency("1w"),
        ProjectEntry::new("p-remote", "cloud-app", "ssh://dev/cloud-app")
            .location(ProjectLocation::Remote)
            .connection(ConnectionStatus::Offline)
            .branch("main")
            .recency("5d"),
        ProjectEntry::new("p-jp", "Sample project", "~/src/sample")
            .branch("main")
            .recency("today"),
    ]
}

/// Large mock history for paint stress (not real discovery).
#[must_use]
pub fn burst_project_entries(n: usize) -> Vec<ProjectEntry> {
    (0..n)
        .map(|i| {
            let g = match i % 5 {
                0 => ProjectGroup::Favorite,
                1 => ProjectGroup::Workspace,
                _ => ProjectGroup::Recent,
            };
            let mut e = ProjectEntry::new(
                format!("p-{i}"),
                format!("project-{i:04}"),
                format!("~/src/project-{i:04}"),
            )
            .group(g)
            .recency(format!("{i}m"));
            if i % 17 == 0 {
                e = e.path_status(ProjectPathStatus::Missing);
            } else if i % 23 == 0 {
                e = e.path_status(ProjectPathStatus::Stale);
            }
            if g == ProjectGroup::Favorite {
                e = e.favorite(true);
            }
            e
        })
        .collect()
}

/// Preview for selected project.
#[must_use]
pub fn example_project_preview() -> (
    PreviewCardContent<'static>,
    &'static [&'static str],
    &'static [PreviewMetadata<'static>],
) {
    const BODY: &[&str] = &[
        "termrock — Rust TUI components",
        "branch: main",
        "path: ~/src/termrock",
        "status: ok · local",
    ];
    const META: &[PreviewMetadata<'static>] = &[
        PreviewMetadata::new("branch", "main"),
        PreviewMetadata::new("path", "~/src/termrock"),
        PreviewMetadata::new("status", "ok"),
    ];
    let content = PreviewCardContent::title("termrock", PreviewResourceKind::File)
        .subtitle("~/src/termrock")
        .meta(META)
        .body(BODY)
        .load(PreviewLoadState::Ready)
        .essential_elsewhere(true);
    (content, BODY, META)
}

/// Quick open items from projects.
#[must_use]
pub fn example_project_quick_open(projects: &[ProjectEntry]) -> Vec<QuickOpenItem<String>> {
    projects
        .iter()
        .map(|p| {
            QuickOpenItem::new(p.id.clone(), p.name.clone())
                .detail(p.path.clone())
                .kind(p.group.id())
        })
        .collect()
}

/// Seed stale/missing chrome story.
pub fn seed_stale_state(state: &mut ProjectLauncherState) {
    state.host_error = None;
    state.connection = ConnectionStatus::Offline;
    state.mode = ProjectLauncherMode::Home;
}

/// Seed empty + onboarding.
pub fn seed_onboarding_state(state: &mut ProjectLauncherState) {
    state.show_onboarding = true;
    state.mode = ProjectLauncherMode::Home;
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress targets (mock scale).
pub mod bench {
    /// Mock history size.
    pub const BURST_ENTRIES: usize = 2_000;
    /// Paint frames.
    pub const PAINT_FRAMES: usize = 8;
    /// Viewport.
    pub const VIEWPORT: (u16, u16) = (120, 40);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::example_sessions;
    use crate::style::DesignSystem;
    use crate::widgets::tests::press;

    fn press_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn open() -> ProjectLauncherState {
        let mut st = ProjectLauncherState::new();
        st.density = Some(ProjectLauncherDensity::Normal);
        st.mode = ProjectLauncherMode::Home;
        st
    }

    #[test]
    fn focus_cycle_visits_visible_panes_only() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        st.focus = "projects";
        let mut seen = vec![st.focus];
        for _ in 0..10 {
            let out = st.handle_key(press(KeyCode::Tab), &projects, &sessions, &[]);
            assert!(matches!(out, ProjectLauncherOutcome::FocusChanged(_)));
            seen.push(st.focus);
        }
        assert!(seen.contains(&"search"));
        assert!(seen.contains(&"projects"));
        assert!(seen.contains(&"sessions"));
        assert!(seen.contains(&"preview"));
        assert!(!seen.contains(&"status"));
    }

    #[test]
    fn narrow_tiny_collapse_and_tab_clamp() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        st.density = Some(ProjectLauncherDensity::Tiny);
        st.focus = "preview";
        st.clamp_focus_to_density(ProjectLauncherDensity::Tiny);
        assert_ne!(st.focus, "preview");
        assert_ne!(st.focus, "sessions");
        let vis = st.visible_focus_panes(ProjectLauncherDensity::Tiny);
        assert!(!vis.contains(&ProjectLauncherPane::Preview));
        assert!(!vis.contains(&ProjectLauncherPane::Sessions));

        // density=None from last paint width
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        st.density = None;
        paint_project_launcher(
            &mut buf,
            area,
            ProjectLauncherSurfaces {
                system: &system,
                state: &mut st,
                projects: &projects,
                sessions: &sessions,
                preview: None,
                quick_open_items: &[],
            },
        );
        assert_eq!(st.effective_density(), ProjectLauncherDensity::Tiny);
        for _ in 0..6 {
            let _ = st.handle_key(press(KeyCode::Tab), &projects, &sessions, &[]);
            assert!(st.focus == "search" || st.focus == "projects");
            assert_ne!(st.focus, "preview");
            assert_ne!(st.focus, "sessions");
        }

        st.density = Some(ProjectLauncherDensity::Narrow);
        st.clamp_focus_to_density(ProjectLauncherDensity::Narrow);
        let vis = st.visible_focus_panes(ProjectLauncherDensity::Narrow);
        assert!(!vis.contains(&ProjectLauncherPane::Preview));
        assert!(vis.contains(&ProjectLauncherPane::Sessions));
    }

    #[test]
    fn open_new_import_favorite_through_launcher() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        st.focus = "projects";
        st.projects = ListState::new(Some("p-termrock".into()));
        st.apply_focus_gates();

        let out = st.handle_key(press(KeyCode::Enter), &projects, &sessions, &[]);
        assert!(
            matches!(
                out,
                ProjectLauncherOutcome::OpenRequested { ref id } if id == "p-termrock"
            ),
            "got {out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('n')), &projects, &sessions, &[]);
        assert!(
            matches!(out, ProjectLauncherOutcome::NewRequested),
            "got {out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('i')), &projects, &sessions, &[]);
        assert!(
            matches!(out, ProjectLauncherOutcome::ImportRequested),
            "got {out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('f')), &projects, &sessions, &[]);
        assert!(
            matches!(
                out,
                ProjectLauncherOutcome::FavoriteToggled { ref id, .. } if id == "p-termrock"
            ),
            "got {out:?}"
        );
    }

    #[test]
    fn session_resume_and_create() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        st.sessions.set_sessions(sessions.clone());
        // When projects focused, sessions must not paint/accept as focused
        st.focus = "projects";
        st.apply_focus_gates();
        assert!(
            !st.sessions.focused,
            "sessions.focused must track focus zone (off when projects)"
        );

        st.focus = "sessions";
        st.apply_focus_gates();
        assert!(
            st.sessions.focused,
            "sessions.focused must be true when sessions zone active"
        );
        // Ensure cursor has a current session
        let expected = st
            .sessions
            .current()
            .map(|s| s.id.clone())
            .expect("session catalog must expose current after set_sessions");

        // Enter → real SessionPicker Opened → SessionResume { id }
        // (requires both focused + accepts_input; gates set both)
        let out = st.handle_key(press(KeyCode::Enter), &projects, &sessions, &[]);
        assert!(
            matches!(
                out,
                ProjectLauncherOutcome::SessionResume { ref id } if id == &expected
            ),
            "workbench path must emit SessionResume for current session {expected}, got {out:?}"
        );

        // Global C-n for session create when not in search
        st.focus = "projects";
        st.apply_focus_gates();
        assert!(!st.sessions.focused);
        let out = st.handle_key(
            press_mod(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &projects,
            &sessions,
            &[],
        );
        assert!(
            matches!(out, ProjectLauncherOutcome::SessionCreate { .. }),
            "got {out:?}"
        );
    }

    #[test]
    fn sessions_focused_tracks_focus_zone_on_paint() {
        let system = DesignSystem::default();
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        st.focus = "projects";
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        paint_project_launcher(
            &mut buf,
            area,
            ProjectLauncherSurfaces {
                system: &system,
                state: &mut st,
                projects: &projects,
                sessions: &sessions,
                preview: None,
                quick_open_items: &[],
            },
        );
        assert!(
            !st.sessions.focused,
            "paint must set_focused(false) when projects focused"
        );

        st.focus = "sessions";
        paint_project_launcher(
            &mut buf,
            area,
            ProjectLauncherSurfaces {
                system: &system,
                state: &mut st,
                projects: &projects,
                sessions: &sessions,
                preview: None,
                quick_open_items: &[],
            },
        );
        assert!(
            st.sessions.focused,
            "paint must set_focused(true) when sessions focused"
        );
    }

    #[test]
    fn filter_and_grouping() {
        let projects = example_projects();
        let filtered = filter_project_entries(&projects, "termrock");
        assert!(!filtered.is_empty());
        assert!(filtered.iter().all(|p| p.matches_query("termrock")));

        let all = filter_project_entries(&projects, "");
        let rows = project_list_rows(&all);
        // Group headers present
        assert!(
            rows.iter().any(|r| r.id.starts_with("g-")),
            "expected group headers"
        );
        // Favorites before recent
        let fav_idx = all.iter().position(|p| p.group == ProjectGroup::Favorite);
        let recent_idx = all.iter().position(|p| p.group == ProjectGroup::Recent);
        if let (Some(f), Some(r)) = (fav_idx, recent_idx) {
            assert!(f < r, "favorites sort before recent");
        }
    }

    #[test]
    fn stale_chrome_host_projectable_no_fs() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        seed_stale_state(&mut st);
        st.focus = "projects";
        st.projects = ListState::new(Some("p-gone".into()));
        st.apply_focus_gates();

        let out = st.handle_key(press(KeyCode::Char('a')), &projects, &sessions, &[]);
        assert!(
            matches!(
                out,
                ProjectLauncherOutcome::StaleAck {
                    id: Some(ref id)
                } if id == "p-gone"
            ),
            "got {out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('r')), &projects, &sessions, &[]);
        assert!(
            matches!(out, ProjectLauncherOutcome::ReloadRequested),
            "got {out:?}"
        );

        // Problems counted from host projection on paint
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 100, 28);
        let mut buf = Buffer::empty(area);
        paint_project_launcher(
            &mut buf,
            area,
            ProjectLauncherSurfaces {
                system: &system,
                state: &mut st,
                projects: &projects,
                sessions: &sessions,
                preview: None,
                quick_open_items: &[],
            },
        );
        assert!(st.problem_count >= 2, "missing+stale in fixtures");
        assert!(
            st.status
                .transient
                .as_ref()
                .is_some_and(|t| t.contains("stale"))
        );
    }

    #[test]
    fn inline_vs_home_mode() {
        let mut st = ProjectLauncherState::inline();
        assert_eq!(st.mode, ProjectLauncherMode::Inline);
        let vis = st.visible_focus_panes(ProjectLauncherDensity::Normal);
        assert!(!vis.contains(&ProjectLauncherPane::Sessions));
        assert!(!vis.contains(&ProjectLauncherPane::Preview));

        let out = st.set_mode(ProjectLauncherMode::Home);
        assert!(matches!(
            out,
            ProjectLauncherOutcome::ModeChanged(ProjectLauncherMode::Home)
        ));
        let vis = st.visible_focus_panes(ProjectLauncherDensity::Normal);
        assert!(vis.contains(&ProjectLauncherPane::Sessions));
    }

    #[test]
    fn quick_open_open_close() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        let qo_items = example_project_quick_open(&projects);
        let qo: Vec<QuickOpenMatch<'_, String>> = qo_items.iter().map(QuickOpenMatch::of).collect();
        let out = st.handle_key(
            press_mod(KeyCode::Char('o'), KeyModifiers::CONTROL),
            &projects,
            &sessions,
            &qo,
        );
        assert!(matches!(out, ProjectLauncherOutcome::QuickOpenOpened));
        assert!(st.quick_open_open);
        let out = st.handle_key(press(KeyCode::Esc), &projects, &sessions, &qo);
        assert!(matches!(out, ProjectLauncherOutcome::QuickOpenClosed));
        assert!(!st.quick_open_open);
    }

    #[test]
    fn onboarding_actions() {
        let mut st = open();
        let projects: Vec<ProjectEntry> = Vec::new();
        let sessions = example_sessions();
        seed_onboarding_state(&mut st);
        st.focus = "onboarding";
        st.apply_focus_gates();
        let out = st.handle_key(press(KeyCode::Enter), &projects, &sessions, &[]);
        assert!(matches!(out, ProjectLauncherOutcome::NewRequested));
        let out = st.handle_key(press(KeyCode::Char('s')), &projects, &sessions, &[]);
        assert!(matches!(out, ProjectLauncherOutcome::OnboardingRequested));
    }

    #[test]
    fn no_discovery_or_persistence_in_composition() {
        let body = include_str!("project_launcher.rs");
        let code = body
            .split("fn no_discovery_or_persistence_in_composition")
            .next()
            .unwrap_or(body);
        for forbidden in [
            "std::fs::",
            "walkdir",
            "sqlite",
            "rusqlite",
            "Command::new",
            "std::process",
            "TcpStream",
            "reqwest",
            "tokio::fs",
        ] {
            let hits: Vec<_> = code
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    !t.starts_with("//")
                        && !t.starts_with("//!")
                        && !t.starts_with('*')
                        && l.contains(forbidden)
                })
                .collect();
            assert!(hits.is_empty(), "forbidden {forbidden}: {hits:?}");
        }
    }

    #[test]
    fn layout_home_normal_has_preview_sessions() {
        let st = WorkspaceState::new();
        let panes = project_launcher_layout_density(
            Rect::new(0, 0, 120, 40),
            &st,
            ProjectLauncherDensity::Normal,
            ProjectLauncherMode::Home,
            false,
        );
        let ids: Vec<_> = panes
            .iter()
            .filter(|p| !p.collapsed && p.area.height > 0 && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        assert!(ids.contains(&"search"));
        assert!(ids.contains(&"projects"));
        assert!(ids.contains(&"sessions"));
        assert!(ids.contains(&"preview"));
        assert!(ids.contains(&"status"));
    }

    #[test]
    fn layout_inline_drops_sessions_preview() {
        let st = WorkspaceState::new();
        let panes = project_launcher_layout_density(
            Rect::new(0, 0, 80, 20),
            &st,
            ProjectLauncherDensity::Normal,
            ProjectLauncherMode::Inline,
            false,
        );
        let ids: Vec<_> = panes
            .iter()
            .filter(|p| !p.collapsed && p.area.height > 0 && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        assert!(ids.contains(&"projects"));
        assert!(!ids.contains(&"sessions"));
        assert!(!ids.contains(&"preview"));
    }

    #[test]
    fn paint_smoke_and_keyboard_paths() {
        let system = DesignSystem::default();
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        let (preview, _, _) = example_project_preview();
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        paint_project_launcher(
            &mut buf,
            area,
            ProjectLauncherSurfaces {
                system: &system,
                state: &mut st,
                projects: &projects,
                sessions: &sessions,
                preview: Some(preview),
                quick_open_items: &[],
            },
        );
        let search = st
            .last_panes()
            .iter()
            .find(|p| p.id.0.as_str() == "search")
            .expect("search");
        assert!(search.area.height >= 3, "search strip ≥3");
        let keys = st
            .status_slots()
            .iter()
            .find(|s| s.id == "keys")
            .map(|s| s.content)
            .unwrap_or("");
        for chord in ["enter open", "n new", "i import", "f fav"] {
            assert!(keys.contains(chord), "missing {chord} in {keys}");
        }
    }

    #[test]
    fn burst_paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.density = Some(ProjectLauncherDensity::Normal);
        let projects = burst_project_entries(bench::BURST_ENTRIES);
        let sessions = example_sessions();
        let area = Rect::new(0, 0, bench::VIEWPORT.0, bench::VIEWPORT.1);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            paint_project_launcher(
                &mut buf,
                area,
                ProjectLauncherSurfaces {
                    system: &system,
                    state: &mut st,
                    projects: &projects,
                    sessions: &sessions,
                    preview: None,
                    quick_open_items: &[],
                },
            );
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 5, "paint too slow: {elapsed:?}");
    }

    #[test]
    fn search_filter_projects_real_path() {
        let mut st = open();
        let projects = example_projects();
        let sessions = example_sessions();
        st.focus = "search";
        st.apply_focus_gates();
        st.search.set_query("missing");
        let out = st.handle_key(press(KeyCode::Enter), &projects, &sessions, &[]);
        assert!(
            matches!(
                out,
                ProjectLauncherOutcome::FilterChanged { ref query } if query.contains("missing")
            ) || st.search.query().contains("missing"),
            "got {out:?}"
        );
        let filtered = filter_project_entries(&projects, st.search.query());
        assert!(
            filtered
                .iter()
                .all(|p| p.path_status == ProjectPathStatus::Missing || p.matches_query("missing")),
            "filter should narrow"
        );
    }
}
