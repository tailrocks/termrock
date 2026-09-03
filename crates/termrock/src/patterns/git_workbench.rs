// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **GitWorkbench** — modern source-owned Git workflow composition from
//! **public** TermRock widgets only (lazygit / GitUI / delta as interaction
//! *references*; distinct TermRock design — not a clone).
//!
//! **Mission.** Layout + focus + typed application messages for repository
//! status, file tree (git badges), diff review, history timeline, branches,
//! commits, command output, conflict diagnostics, contextual actions, and
//! keyboard help. Fullscreen diff promotion. Responsive density. **All Git
//! execution stays host-owned** (outcomes/requests only — no `git` process,
//! libgit2, or worktree mutation inside this pattern).
//!
//! **vs [`super::agent_workbench`].** Agent chrome; this is SCM workflow.
//! **vs [`super::database_workbench`].** Data IDE; this is Git.
//! **vs standalone [`DiffReview`] / [`FileTree`].** Composed, not re-painted.
//!
//! Research: lazygit, GitUI, delta, IDE source-control panels.
//!
//! Teaches: how to compose modern source-owned Git workflow composition from.
//!
//! Composes: [`crate::widgets::Checkpoint`],
//! [`crate::widgets::CheckpointTimeline`],
//! [`crate::widgets::CheckpointTimelineOutcome`],
//! [`crate::widgets::CheckpointTimelineState`],
//! [`crate::widgets::ConfirmFocus`], [`crate::widgets::ConfirmPrompt`],
//! [`crate::widgets::Diagnostic`], [`crate::widgets::DiagnosticSeverity`],
//! and 31 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    layout::{
        ModalSpec, PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode,
        WorkspaceState, modal_rect,
    },
    style::{DesignSystem, Glyph, ListRowVisualState, PanelChrome, Role},
    widgets::{
        Checkpoint, CheckpointTimeline, CheckpointTimelineOutcome, CheckpointTimelineState,
        ConfirmFocus, ConfirmPrompt, Diagnostic, DiagnosticSeverity, DiagnosticState,
        DiagnosticView, DiffHunk, DiffLine, DiffReview, DiffReviewFileRow, DiffReviewOutcome,
        DiffReviewState, DiffReviewUnit, FileGitStatus, FileTree, FileTreeEntry, FileTreeOutcome,
        FileTreeState, HelpEntry, KeyboardHelp, KeyboardHelpOutcome, KeyboardHelpState, Panel,
        StatusBar, StatusBarState, StatusRegion, StatusSlot, TerminalCommandMeta, TerminalLine,
        TerminalOutput, TerminalOutputState, TerminalRunStatus, example_checkpoints,
        example_help_entries,
    },
};

// ── Panes & density ─────────────────────────────────────────────────────────

/// Named panes of the Git workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GitWorkbenchPane {
    /// File tree / status list.
    Files,
    /// Diff review (hunks / lines).
    Diff,
    /// Commit / checkpoint history.
    History,
    /// Branch list.
    Branches,
    /// Command output strip.
    Output,
    /// Conflict diagnostics (when dirty-conflict).
    Diagnostics,
    /// Status strip.
    Status,
}

impl GitWorkbenchPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Diff => "diff",
            Self::History => "history",
            Self::Branches => "branches",
            Self::Output => "output",
            Self::Diagnostics => "diagnostics",
            Self::Status => "status",
        }
    }

    /// Default Tab focus cycle (status is chrome-only).
    #[must_use]
    pub fn focus_order() -> &'static [GitWorkbenchPane] {
        &[
            Self::Files,
            Self::Diff,
            Self::History,
            Self::Branches,
            Self::Output,
            Self::Diagnostics,
        ]
    }
}

/// Responsive density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum GitWorkbenchDensity {
    /// Full multi-pane workbench.
    #[default]
    Normal,
    /// Collapse history/branches/output; keep files + diff.
    Narrow,
    /// Diff + status (files optional strip) — or fullscreen diff.
    Tiny,
}

impl GitWorkbenchDensity {
    /// From terminal width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 48 {
            Self::Tiny
        } else if width < 90 {
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

// ── Domain projections (host-owned truth) ───────────────────────────────────

/// High-level repository chrome status (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum GitRepoStatus {
    /// Clean worktree.
    #[default]
    Clean,
    /// Dirty (modified / untracked).
    Dirty,
    /// Merge/rebase conflict.
    Conflict,
    /// Detached HEAD.
    Detached,
    /// Merge in progress (no conflict markers yet).
    Merging,
    /// Rebase in progress.
    Rebasing,
}

impl GitRepoStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Dirty => "dirty",
            Self::Conflict => "conflict",
            Self::Detached => "detached",
            Self::Merging => "merging",
            Self::Rebasing => "rebasing",
        }
    }

    /// Status label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Dirty => "dirty",
            Self::Conflict => "conflict",
            Self::Detached => "detached",
            Self::Merging => "merging",
            Self::Rebasing => "rebasing",
        }
    }

    /// Shared lifecycle projection for recipe-owned status chrome.
    #[must_use]
    pub const fn semantic(self) -> crate::widgets::SemanticStatus {
        match self {
            Self::Clean => crate::widgets::SemanticStatus::Success,
            Self::Dirty | Self::Detached => crate::widgets::SemanticStatus::Warning,
            Self::Conflict => crate::widgets::SemanticStatus::Failed,
            Self::Merging | Self::Rebasing => crate::widgets::SemanticStatus::Running,
        }
    }
}

/// Branch list row (host-projected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranch {
    /// Name (`main`, `feature/x`).
    pub name: String,
    /// Currently checked out.
    pub current: bool,
    /// Upstream label.
    pub upstream: Option<String>,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
}

impl GitBranch {
    /// Construct.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            current: false,
            upstream: None,
            ahead: 0,
            behind: 0,
        }
    }

    /// Mark current.
    #[must_use]
    pub const fn current(mut self) -> Self {
        self.current = true;
        self
    }

    /// Upstream.
    #[must_use]
    pub fn upstream(mut self, u: impl Into<String>) -> Self {
        self.upstream = Some(u.into());
        self
    }

    /// Ahead/behind.
    #[must_use]
    pub const fn tracking(mut self, ahead: u32, behind: u32) -> Self {
        self.ahead = ahead;
        self.behind = behind;
        self
    }
}

/// Destructive confirm kind (workbench-owned chrome; host executes).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GitDestructiveKind {
    /// Discard worktree changes for paths.
    Discard {
        /// Paths.
        paths: Vec<String>,
    },
    /// Hard reset request (host policy).
    ResetHard {
        /// Target ref.
        target: String,
    },
    /// Checkout branch (may lose uncommitted — host decides).
    Checkout {
        /// Branch name.
        branch: String,
    },
    /// Delete branch.
    DeleteBranch {
        /// Branch name.
        branch: String,
    },
}

impl GitDestructiveKind {
    /// Label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Discard { .. } => "Discard",
            Self::ResetHard { .. } => "Reset hard",
            Self::Checkout { .. } => "Checkout",
            Self::DeleteBranch { .. } => "Delete branch",
        }
    }

    /// Consequence line (safe language).
    #[must_use]
    pub fn consequence(&self) -> String {
        match self {
            Self::Discard { paths } => {
                format!(
                    "discard local changes for {} path(s) (host executes)",
                    paths.len()
                )
            }
            Self::ResetHard { target } => {
                format!("reset hard to {target} (host executes; irreversible)")
            }
            Self::Checkout { branch } => {
                format!("checkout {branch} (host executes)")
            }
            Self::DeleteBranch { branch } => {
                format!("delete branch {branch} (host executes)")
            }
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Workbench outcomes — requests only; host owns Git I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GitWorkbenchOutcome {
    /// Ignored.
    Ignored,
    /// Focus pane changed.
    FocusChanged(&'static str),
    /// Stage request (units from DiffReview or files).
    StageRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Unstage request.
    UnstageRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Discard after confirm.
    DiscardRequested {
        /// Paths.
        paths: Vec<String>,
    },
    /// Commit request (message draft; host runs `git commit`).
    CommitRequested {
        /// Message.
        message: String,
    },
    /// Checkout branch (after confirm when dirty).
    CheckoutRequested {
        /// Branch.
        branch: String,
    },
    /// Delete branch after confirm.
    DeleteBranchRequested {
        /// Branch.
        branch: String,
    },
    /// Hard reset after confirm.
    ResetHardRequested {
        /// Target.
        target: String,
    },
    /// Refresh status / diff (host re-projects).
    RefreshRequested,
    /// Open keyboard help.
    OpenHelp,
    /// Help closed.
    HelpClosed,
    /// Fullscreen diff toggled.
    FullscreenDiff {
        /// On.
        on: bool,
    },
    /// Confirm dialog opened (Cancel default).
    ConfirmOpened(GitDestructiveKind),
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Diff review child (non-stage/destructive already mapped when possible).
    Diff(DiffReviewOutcome),
    /// File tree child.
    Files {
        /// Kind label.
        kind: String,
        /// Id when known.
        id: Option<String>,
    },
    /// History timeline child.
    History {
        /// Kind label.
        kind: String,
    },
    /// Branch selection moved.
    BranchSelected {
        /// Name.
        name: String,
    },
    /// Diagnostics child.
    Diagnostics {
        /// Kind label.
        kind: String,
    },
    /// Output pane child.
    Output {
        /// Kind label.
        kind: String,
    },
    /// Esc root cancel.
    Cancelled,
}

// ── Surfaces ────────────────────────────────────────────────────────────────

/// Borrowed surfaces for one paint frame.
pub struct GitWorkbenchSurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut GitWorkbenchState,
    /// File tree projection.
    pub files: &'a [FileTreeEntry<'a, &'static str>],
    /// Diff lines.
    pub diff_lines: &'a [DiffLine<'a>],
    /// Diff hunks.
    pub hunks: &'a [DiffHunk],
    /// DiffReview file rows.
    pub diff_files: &'a [DiffReviewFileRow<'a>],
    /// History checkpoints (commits).
    pub commits: &'a [Checkpoint],
    /// Conflict diagnostics.
    pub diagnostics: &'a [Diagnostic<'a>],
    /// Terminal meta for output pane.
    pub terminal_meta: &'a TerminalCommandMeta<'a>,
    /// Terminal lines.
    pub terminal_lines: &'a [TerminalLine<'a>],
    /// Keyboard help entries (host-filtered references).
    pub help_entries: &'a [&'a HelpEntry],
}

// ── State ───────────────────────────────────────────────────────────────────

/// Persistent Git workbench state.
#[derive(Debug)]
pub struct GitWorkbenchState {
    /// Workspace collapse.
    pub workspace: WorkspaceState,
    /// File tree.
    pub files: FileTreeState<&'static str>,
    /// Diff review.
    pub diff: DiffReviewState,
    /// History timeline.
    pub history: CheckpointTimelineState,
    /// Output pane.
    pub output: TerminalOutputState,
    /// Diagnostics.
    pub diagnostics: DiagnosticState,
    /// Keyboard help.
    pub help: KeyboardHelpState,
    /// Status bar.
    pub status: StatusBarState<&'static str>,
    /// Branches (host-projected).
    pub branches: Vec<GitBranch>,
    /// Branch cursor.
    pub branch_cursor: usize,
    /// Repo status chrome.
    pub repo_status: GitRepoStatus,
    /// Commit message draft.
    pub commit_message: String,
    /// Focused pane id.
    pub focus: &'static str,
    /// Density override.
    pub density: Option<GitWorkbenchDensity>,
    /// Fullscreen diff promotion.
    pub fullscreen_diff: bool,
    /// Pending destructive confirm (`None` = none). Cancel is default focus.
    pub confirm: Option<GitDestructiveKind>,
    /// Confirm: false = Cancel (safe default), true = proceed.
    pub confirm_proceed_focused: bool,
    /// Help modal open (mirrors KeyboardHelp modal).
    pub help_open: bool,
    /// Colorless.
    pub colorless: bool,
    /// Last panes.
    last_panes: Vec<PaneGeom>,
    /// Last paint width for density=None.
    last_area_width: Option<u16>,
}

impl Default for GitWorkbenchState {
    fn default() -> Self {
        Self::new()
    }
}

impl GitWorkbenchState {
    /// Fresh workbench with demo branch list.
    #[must_use]
    pub fn new() -> Self {
        let mut files = FileTreeState::new();
        files.select(Some("src/main.rs"));
        let mut history = CheckpointTimelineState::new();
        history.set_checkpoints(example_git_commits());
        Self {
            workspace: WorkspaceState::new(),
            files,
            diff: DiffReviewState::new(),
            history,
            output: TerminalOutputState::new(),
            diagnostics: DiagnosticState::new(),
            help: KeyboardHelpState::new(),
            status: StatusBarState::new(),
            branches: example_git_branches(),
            branch_cursor: 0,
            repo_status: GitRepoStatus::Dirty,
            commit_message: String::new(),
            focus: GitWorkbenchPane::Files.id(),
            density: None,
            fullscreen_diff: false,
            confirm: None,
            confirm_proceed_focused: false,
            help_open: false,
            colorless: false,
            last_panes: Vec::new(),
            last_area_width: None,
        }
    }

    /// Last panes.
    #[must_use]
    pub fn last_panes(&self) -> &[PaneGeom] {
        &self.last_panes
    }

    /// Help modal open.
    #[must_use]
    pub const fn help_is_open(&self) -> bool {
        self.help_open
    }

    /// Effective density (override → last width → last_panes → Normal).
    #[must_use]
    pub fn effective_density(&self) -> GitWorkbenchDensity {
        if self.fullscreen_diff {
            return GitWorkbenchDensity::Tiny;
        }
        if let Some(d) = self.density {
            return d;
        }
        if let Some(w) = self.last_area_width {
            return GitWorkbenchDensity::for_width(w);
        }
        if !self.last_panes.is_empty() {
            let has = |id: &str| {
                self.last_panes.iter().any(|p| {
                    p.id.0.as_str() == id && !p.collapsed && p.area.width > 0 && p.area.height > 0
                })
            };
            if !has("files") && !has("history") {
                return GitWorkbenchDensity::Tiny;
            }
            if !has("history") && !has("branches") {
                return GitWorkbenchDensity::Narrow;
            }
            return GitWorkbenchDensity::Normal;
        }
        GitWorkbenchDensity::Normal
    }

    /// Focus order for density.
    #[must_use]
    pub fn focus_order_for(&self, density: GitWorkbenchDensity) -> Vec<&'static str> {
        if self.fullscreen_diff {
            return vec![GitWorkbenchPane::Diff.id()];
        }
        let mut order = match density {
            GitWorkbenchDensity::Normal => vec![
                GitWorkbenchPane::Files.id(),
                GitWorkbenchPane::Diff.id(),
                GitWorkbenchPane::History.id(),
                GitWorkbenchPane::Branches.id(),
                GitWorkbenchPane::Output.id(),
            ],
            GitWorkbenchDensity::Narrow => vec![
                GitWorkbenchPane::Files.id(),
                GitWorkbenchPane::Diff.id(),
                GitWorkbenchPane::Output.id(),
            ],
            GitWorkbenchDensity::Tiny => {
                vec![GitWorkbenchPane::Files.id(), GitWorkbenchPane::Diff.id()]
            }
        };
        if matches!(self.repo_status, GitRepoStatus::Conflict) {
            order.push(GitWorkbenchPane::Diagnostics.id());
        }
        order
    }

    /// Clamp focus to visible panes.
    pub fn clamp_focus_to_density(&mut self, density: GitWorkbenchDensity) {
        let order = self.focus_order_for(density);
        if !order.contains(&self.focus) {
            self.focus = order.first().copied().unwrap_or("diff");
            self.apply_focus_gates();
        }
    }

    fn apply_focus_gates(&mut self) {
        let f = self.focus;
        let live = self.confirm.is_none();
        self.files.set_accepts_input(f == "files" && live);
        self.diff.set_accepts_input(f == "diff" && live);
        // CheckpointTimeline paints PanelChrome from state.focused (not painter flag).
        self.history.set_focused(f == "history");
        self.history.set_accepts_input(f == "history" && live);
        self.output.set_accepts_input(f == "output" && live);
        self.diagnostics
            .set_accepts_input(f == "diagnostics" && live);
    }

    /// Set focus.
    pub fn set_focus(&mut self, pane: GitWorkbenchPane) -> GitWorkbenchOutcome {
        self.focus = pane.id();
        self.apply_focus_gates();
        GitWorkbenchOutcome::FocusChanged(self.focus)
    }

    /// Focus next.
    pub fn focus_next(&mut self, density: GitWorkbenchDensity) -> GitWorkbenchOutcome {
        let order = self.focus_order_for(density);
        if order.is_empty() {
            return GitWorkbenchOutcome::Ignored;
        }
        let i = order.iter().position(|id| *id == self.focus).unwrap_or(0);
        self.focus = order[(i + 1) % order.len()];
        self.apply_focus_gates();
        GitWorkbenchOutcome::FocusChanged(self.focus)
    }

    /// Focus prev.
    pub fn focus_prev(&mut self, density: GitWorkbenchDensity) -> GitWorkbenchOutcome {
        let order = self.focus_order_for(density);
        if order.is_empty() {
            return GitWorkbenchOutcome::Ignored;
        }
        let i = order.iter().position(|id| *id == self.focus).unwrap_or(0);
        self.focus = order[(i + order.len() - 1) % order.len()];
        self.apply_focus_gates();
        GitWorkbenchOutcome::FocusChanged(self.focus)
    }

    /// Whether conflict diagnostics pane should be allocated.
    #[must_use]
    pub const fn include_diagnostics(&self) -> bool {
        matches!(self.repo_status, GitRepoStatus::Conflict)
    }

    /// Layout.
    pub fn layout(&mut self, area: Rect) -> Vec<PaneGeom> {
        self.last_area_width = Some(area.width);
        let density = self.effective_density();
        let panes = git_workbench_layout_density(
            area,
            &self.workspace,
            density,
            self.fullscreen_diff,
            self.include_diagnostics(),
        );
        self.last_panes = panes.clone();
        self.clamp_focus_to_density(density);
        panes
    }

    /// Toggle fullscreen diff.
    pub fn set_fullscreen_diff(&mut self, on: bool) -> GitWorkbenchOutcome {
        self.fullscreen_diff = on;
        if on {
            self.focus = GitWorkbenchPane::Diff.id();
        }
        self.apply_focus_gates();
        GitWorkbenchOutcome::FullscreenDiff { on }
    }

    /// Open discard confirm for paths (Cancel default).
    pub fn open_discard_confirm(&mut self, paths: Vec<String>) -> GitWorkbenchOutcome {
        if paths.is_empty() {
            return GitWorkbenchOutcome::Ignored;
        }
        let kind = GitDestructiveKind::Discard { paths };
        self.confirm = Some(kind.clone());
        self.confirm_proceed_focused = false;
        GitWorkbenchOutcome::ConfirmOpened(kind)
    }

    /// Open checkout confirm when dirty.
    pub fn open_checkout_confirm(&mut self, branch: String) -> GitWorkbenchOutcome {
        let kind = GitDestructiveKind::Checkout { branch };
        self.confirm = Some(kind.clone());
        self.confirm_proceed_focused = false;
        GitWorkbenchOutcome::ConfirmOpened(kind)
    }

    fn emit_confirm(&mut self) -> GitWorkbenchOutcome {
        let Some(kind) = self.confirm.take() else {
            return GitWorkbenchOutcome::Ignored;
        };
        self.confirm_proceed_focused = false;
        match kind {
            GitDestructiveKind::Discard { paths } => {
                GitWorkbenchOutcome::DiscardRequested { paths }
            }
            GitDestructiveKind::ResetHard { target } => {
                GitWorkbenchOutcome::ResetHardRequested { target }
            }
            GitDestructiveKind::Checkout { branch } => {
                GitWorkbenchOutcome::CheckoutRequested { branch }
            }
            GitDestructiveKind::DeleteBranch { branch } => {
                GitWorkbenchOutcome::DeleteBranchRequested { branch }
            }
        }
    }

    fn map_diff_outcome(&mut self, out: DiffReviewOutcome) -> GitWorkbenchOutcome {
        match out {
            DiffReviewOutcome::Ignored => GitWorkbenchOutcome::Ignored,
            DiffReviewOutcome::StageRequested { units } => {
                GitWorkbenchOutcome::StageRequested { units }
            }
            DiffReviewOutcome::UnstageRequested { units } => {
                GitWorkbenchOutcome::UnstageRequested { units }
            }
            DiffReviewOutcome::ConfirmRequired(c) => {
                // Map DiffReview destructive confirm into workbench confirm when discard-like
                // DiffReview uses Reject/Apply as destructive — surface as Diff outcome.
                GitWorkbenchOutcome::Diff(DiffReviewOutcome::ConfirmRequired(c))
            }
            DiffReviewOutcome::ConfirmCancelled => GitWorkbenchOutcome::ConfirmCancelled,
            other => GitWorkbenchOutcome::Diff(other),
        }
    }

    /// Keyboard routing.
    ///
    /// Diff path uses [`DiffReviewState::handle_key_lines`] with the same
    /// lines/hunks/files projection as paint so line/file/hunk selection and
    /// stage (`t`) / unstage (`T`) resolve real unit ids.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        files: &[FileTreeEntry<'_, &'static str>],
        hunks: &[DiffHunk],
        diff_lines: &[DiffLine<'_>],
        diff_files: &[DiffReviewFileRow<'_>],
        help_entries: &[&HelpEntry],
        diagnostics: &[Diagnostic<'_>],
        terminal_lines: &[TerminalLine<'_>],
        terminal_meta: &TerminalCommandMeta<'_>,
    ) -> GitWorkbenchOutcome {
        if !key.is_press() {
            return GitWorkbenchOutcome::Ignored;
        }

        let density = self.effective_density();
        self.clamp_focus_to_density(density);

        // Help modal
        if self.help_open {
            let out = self.help.handle_key(key, help_entries);
            match out {
                KeyboardHelpOutcome::Closed => {
                    self.help_open = false;
                    let _ = self.help.close_modal();
                    return GitWorkbenchOutcome::HelpClosed;
                }
                KeyboardHelpOutcome::Ignored if matches!(key.code, KeyCode::Esc) => {
                    self.help_open = false;
                    let _ = self.help.close_modal();
                    return GitWorkbenchOutcome::HelpClosed;
                }
                _ => {
                    if matches!(key.code, KeyCode::Esc) {
                        self.help_open = false;
                        let _ = self.help.close_modal();
                        return GitWorkbenchOutcome::HelpClosed;
                    }
                    return GitWorkbenchOutcome::Ignored;
                }
            }
        }

        // Destructive confirm (Cancel default; y unbound)
        if self.confirm.is_some() {
            return self.handle_confirm_key(key);
        }

        // Global chords
        match key.code {
            KeyCode::Char('?') if key.modifiers.is_empty() => {
                let _ = self.help.open_modal();
                self.help_open = true;
                return GitWorkbenchOutcome::OpenHelp;
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.set_fullscreen_diff(!self.fullscreen_diff);
            }
            KeyCode::Char('r')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || (key.modifiers.is_empty()
                        && self.focus != "files"
                        && self.focus != "diff") =>
            {
                // Ctrl+R always refresh; plain r only when not in typing panes
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || matches!(
                        self.focus,
                        "history" | "branches" | "output" | "diagnostics"
                    )
                {
                    return GitWorkbenchOutcome::RefreshRequested;
                }
            }
            KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                return self.focus_next(density);
            }
            KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                return self.focus_prev(density);
            }
            KeyCode::Esc => {
                if self.fullscreen_diff {
                    return self.set_fullscreen_diff(false);
                }
                if self.focus != GitWorkbenchPane::Files.id() {
                    return self.set_focus(GitWorkbenchPane::Files);
                }
                return GitWorkbenchOutcome::Cancelled;
            }
            KeyCode::Char('c')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                // Ctrl+Shift+C commit with draft
                let msg = self.commit_message.trim().to_string();
                if msg.is_empty() {
                    return GitWorkbenchOutcome::Ignored;
                }
                return GitWorkbenchOutcome::CommitRequested { message: msg };
            }
            _ => {}
        }

        // Discard: `x` on files selection opens confirm
        if matches!(self.focus, "files")
            && matches!(key.code, KeyCode::Char('x'))
            && key.modifiers.is_empty()
        {
            if let Some(id) = self.files.selected() {
                return self.open_discard_confirm(vec![(*id).to_string()]);
            }
        }

        match self.focus {
            "files" => {
                let out = self.files.handle_key(files, key);
                match out {
                    FileTreeOutcome::Ignored => GitWorkbenchOutcome::Ignored,
                    FileTreeOutcome::SelectionChanged(id)
                    | FileTreeOutcome::OpenRequested(id)
                    | FileTreeOutcome::Toggle(id)
                    | FileTreeOutcome::LoadChildrenRequested(id) => GitWorkbenchOutcome::Files {
                        kind: "selection".into(),
                        id: Some(id.to_string()),
                    },
                    other => {
                        let kind = format!("{other:?}")
                            .split(|c: char| c == '(' || c == ' ')
                            .next()
                            .unwrap_or("files")
                            .to_string();
                        GitWorkbenchOutcome::Files { kind, id: None }
                    }
                }
            }
            "diff" => {
                let out = self
                    .diff
                    .handle_key_lines(key, diff_lines, hunks, diff_files);
                self.map_diff_outcome(out)
            }
            "history" => {
                let out = self.history.handle_key(key);
                match out {
                    CheckpointTimelineOutcome::Ignored => GitWorkbenchOutcome::Ignored,
                    other => {
                        let kind = format!("{other:?}")
                            .split(|c: char| c == '(' || c == ' ')
                            .next()
                            .unwrap_or("history")
                            .to_string();
                        GitWorkbenchOutcome::History { kind }
                    }
                }
            }
            "branches" => self.handle_branch_key(key),
            "output" => {
                let out = self.output.handle_key(key, terminal_lines, terminal_meta);
                let kind = format!("{out:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("output")
                    .to_string();
                if kind == "Ignored" {
                    GitWorkbenchOutcome::Ignored
                } else {
                    GitWorkbenchOutcome::Output { kind }
                }
            }
            "diagnostics" => {
                let out = self.diagnostics.handle_key(key, diagnostics);
                let kind = format!("{out:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("diagnostics")
                    .to_string();
                if kind == "Ignored" {
                    GitWorkbenchOutcome::Ignored
                } else {
                    GitWorkbenchOutcome::Diagnostics { kind }
                }
            }
            _ => GitWorkbenchOutcome::Ignored,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> GitWorkbenchOutcome {
        match key.code {
            KeyCode::Esc => {
                self.confirm = None;
                self.confirm_proceed_focused = false;
                GitWorkbenchOutcome::ConfirmCancelled
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.confirm_proceed_focused = false;
                GitWorkbenchOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.confirm_proceed_focused = true;
                GitWorkbenchOutcome::Ignored
            }
            KeyCode::Tab => {
                self.confirm_proceed_focused = !self.confirm_proceed_focused;
                GitWorkbenchOutcome::Ignored
            }
            KeyCode::Enter => {
                if self.confirm_proceed_focused {
                    self.emit_confirm()
                } else {
                    self.confirm = None;
                    self.confirm_proceed_focused = false;
                    GitWorkbenchOutcome::ConfirmCancelled
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => GitWorkbenchOutcome::Ignored,
            _ => GitWorkbenchOutcome::Ignored,
        }
    }

    fn handle_branch_key(&mut self, key: KeyEvent) -> GitWorkbenchOutcome {
        if self.branches.is_empty() {
            return GitWorkbenchOutcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.branch_cursor = self.branch_cursor.saturating_sub(1);
                let name = self.branches[self.branch_cursor].name.clone();
                GitWorkbenchOutcome::BranchSelected { name }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.branch_cursor = (self.branch_cursor + 1).min(self.branches.len() - 1);
                let name = self.branches[self.branch_cursor].name.clone();
                GitWorkbenchOutcome::BranchSelected { name }
            }
            KeyCode::Enter => {
                let name = self.branches[self.branch_cursor].name.clone();
                if self.branches[self.branch_cursor].current {
                    return GitWorkbenchOutcome::Ignored;
                }
                if matches!(
                    self.repo_status,
                    GitRepoStatus::Dirty | GitRepoStatus::Conflict
                ) {
                    return self.open_checkout_confirm(name);
                }
                GitWorkbenchOutcome::CheckoutRequested { branch: name }
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                let name = self.branches[self.branch_cursor].name.clone();
                if self.branches[self.branch_cursor].current {
                    return GitWorkbenchOutcome::Ignored;
                }
                let kind = GitDestructiveKind::DeleteBranch { branch: name };
                self.confirm = Some(kind.clone());
                self.confirm_proceed_focused = false;
                GitWorkbenchOutcome::ConfirmOpened(kind)
            }
            _ => GitWorkbenchOutcome::Ignored,
        }
    }

    /// Status slots.
    #[must_use]
    pub fn status_slots(&self) -> Vec<StatusSlot<'static, &'static str>> {
        let status = self.repo_status.label();
        let mut slots = vec![
            StatusSlot::connection("repo", status)
                .semantic(self.repo_status.semantic())
                .priority(90),
            StatusSlot::mode("branch", "branch").priority(50),
            StatusSlot::focus_zone("focus", self.focus).priority(70),
            StatusSlot::shortcut(
                "keys",
                "t stage · T unstage · x discard · C-f full · ? help",
            )
            .priority(10),
        ];
        if matches!(self.repo_status, GitRepoStatus::Conflict) {
            slots.insert(
                0,
                StatusSlot::new("conflict", "conflict")
                    .semantic(crate::widgets::SemanticStatus::Failed)
                    .region(StatusRegion::Left)
                    .priority(100),
            );
        }
        slots
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

fn south_stack(include_diagnostics: bool, include_output: bool) -> WorkspaceNode {
    let status = WorkspaceNode::Leaf {
        id: PaneId::from_static(GitWorkbenchPane::Status.id()),
        constraint: PaneConstraint::Fixed(1),
        collapse_priority: 3,
    };
    let output = WorkspaceNode::Leaf {
        id: PaneId::from_static(GitWorkbenchPane::Output.id()),
        constraint: PaneConstraint::Min(3),
        collapse_priority: 2,
    };
    let diagnostics = WorkspaceNode::Leaf {
        id: PaneId::from_static(GitWorkbenchPane::Diagnostics.id()),
        constraint: PaneConstraint::Min(3),
        collapse_priority: 1,
    };

    match (include_diagnostics, include_output) {
        (true, true) => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 55,
            first: Box::new(diagnostics),
            second: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 70,
                first: Box::new(output),
                second: Box::new(status),
            }),
        },
        (true, false) => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 75,
            first: Box::new(diagnostics),
            second: Box::new(status),
        },
        (false, true) => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 75,
            first: Box::new(output),
            second: Box::new(status),
        },
        (false, false) => status,
    }
}

/// Layout with density, fullscreen, and optional conflict diagnostics pane.
#[must_use]
pub fn git_workbench_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: GitWorkbenchDensity,
    fullscreen_diff: bool,
    include_diagnostics: bool,
) -> Vec<PaneGeom> {
    if fullscreen_diff {
        // Fullscreen keeps status; surface conflicts as diagnostics strip when needed.
        return Workspace::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: if include_diagnostics { 88 } else { 96 },
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(GitWorkbenchPane::Diff.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 1,
            }),
            second: Box::new(south_stack(include_diagnostics, false)),
        })
        .layout(area, state);
    }

    let root = match density {
        GitWorkbenchDensity::Tiny => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 90,
            first: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Horizontal,
                ratio_percent: 30,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(GitWorkbenchPane::Files.id()),
                    constraint: PaneConstraint::Min(10),
                    collapse_priority: 0,
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(GitWorkbenchPane::Diff.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
            }),
            second: Box::new(south_stack(include_diagnostics, false)),
        },
        GitWorkbenchDensity::Narrow => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 85,
            first: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Horizontal,
                ratio_percent: 28,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(GitWorkbenchPane::Files.id()),
                    constraint: PaneConstraint::Min(12),
                    collapse_priority: 0,
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(GitWorkbenchPane::Diff.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
            }),
            second: Box::new(south_stack(include_diagnostics, true)),
        },
        GitWorkbenchDensity::Normal => {
            // west files | center diff | east history+branches | south diagnostics?/output | status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 80,
                first: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 22,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(GitWorkbenchPane::Files.id()),
                        constraint: PaneConstraint::Min(14),
                        collapse_priority: 0,
                    }),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Horizontal,
                        ratio_percent: 70,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(GitWorkbenchPane::Diff.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                        second: Box::new(WorkspaceNode::Split {
                            axis: WorkspaceAxis::Vertical,
                            ratio_percent: 55,
                            first: Box::new(WorkspaceNode::Leaf {
                                id: PaneId::from_static(GitWorkbenchPane::History.id()),
                                constraint: PaneConstraint::Weight(1),
                                collapse_priority: 0,
                            }),
                            second: Box::new(WorkspaceNode::Leaf {
                                id: PaneId::from_static(GitWorkbenchPane::Branches.id()),
                                constraint: PaneConstraint::Min(4),
                                collapse_priority: 0,
                            }),
                        }),
                    }),
                }),
                second: Box::new(south_stack(include_diagnostics, true)),
            }
        }
    };
    Workspace::new(root).layout(area, state)
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

fn centered_modal(area: Rect) -> Rect {
    modal_rect(area, ModalSpec::new(3, 5, 28).height(1, 2, 8))
}

// ── Render ──────────────────────────────────────────────────────────────────

/// Paint composed Git workbench (public child widgets only).
pub fn paint_git_workbench(buffer: &mut Buffer, area: Rect, surfaces: GitWorkbenchSurfaces<'_>) {
    let GitWorkbenchSurfaces {
        system,
        state,
        files,
        diff_lines,
        hunks,
        diff_files,
        commits,
        diagnostics,
        terminal_meta,
        terminal_lines,
        help_entries,
    } = surfaces;

    if area.is_empty() {
        return;
    }

    state.last_area_width = Some(area.width);
    let density = state.effective_density();
    let panes = git_workbench_layout_density(
        area,
        &state.workspace,
        density,
        state.fullscreen_diff,
        state.include_diagnostics(),
    );
    state.last_panes = panes.clone();
    state.clamp_focus_to_density(density);
    state.apply_focus_gates();

    // Sync history projection if host passed commits and state empty
    if state.history.current().is_none() && !commits.is_empty() {
        state.history.set_checkpoints(commits.to_vec());
    }

    if let Some(r) = pane_area(&panes, "files") {
        let focused = state.focus == "files";
        let panel = Panel::new(system).title("Files").emphasis(if focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        });
        let inner = panel.inner(r);
        panel.paint(r, buffer, None);
        FileTree::new(files, system)
            .title("Changes")
            .focused(focused)
            .paint(inner, buffer, &mut state.files);
    }

    if let Some(r) = pane_area(&panes, "diff") {
        let focused = state.focus == "diff" || state.fullscreen_diff;
        DiffReview::new(diff_lines, system)
            .hunks(hunks)
            .files(diff_files)
            .title(if state.fullscreen_diff {
                "Diff · fullscreen"
            } else {
                "Diff"
            })
            .focused(focused)
            .colorless(state.colorless)
            .show_tree(false) // FileTree is west pane
            .render(r, buffer, &mut state.diff);
    }

    if let Some(r) = pane_area(&panes, "history") {
        // state.history.focused is the paint authority (PanelChrome); keep in sync.
        state.history.set_focused(state.focus == "history");
        CheckpointTimeline::new(system).paint(r, buffer, &mut state.history);
    }

    if let Some(r) = pane_area(&panes, "branches") {
        paint_branch_list(system, r, buffer, state);
    }

    if let Some(r) = pane_area(&panes, "output") {
        let focused = state.focus == "output";
        TerminalOutput::new(terminal_meta, terminal_lines, system)
            .title("Git output")
            .focused(focused)
            .colorless(state.colorless)
            .render(r, buffer, &mut state.output);
    }

    if let Some(r) = pane_area(&panes, "diagnostics") {
        let focused = state.focus == "diagnostics";
        DiagnosticView::new(diagnostics, system)
            .title("Conflicts")
            .focused(focused)
            .colorless(state.colorless)
            .render(r, buffer, &mut state.diagnostics);
    }

    if let Some(r) = pane_area(&panes, "status") {
        let slots = state.status_slots();
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], system),
            r,
            buffer,
            &mut state.status,
        );
    }

    // Confirm banner (bottom of full area)
    if let Some(kind) = &state.confirm {
        paint_confirm_banner(system, area, buffer, kind, state.confirm_proceed_focused);
    }

    // Help modal
    if state.help_open {
        let m = centered_modal(area);
        KeyboardHelp::new(help_entries, system)
            .title("Git workbench help")
            .colorless(state.colorless)
            .paint(m, buffer, &mut state.help);
    }
}

fn paint_branch_list(
    system: &DesignSystem,
    area: Rect,
    buffer: &mut Buffer,
    state: &mut GitWorkbenchState,
) {
    if area.is_empty() {
        return;
    }
    let focused = state.focus == "branches";
    let panel = Panel::new(system).title("Branches").emphasis(if focused {
        PanelChrome::Focused
    } else {
        PanelChrome::Normal
    });
    let inner = panel.inner(area);
    panel.paint(area, buffer, None);
    if inner.is_empty() {
        return;
    }
    let _w = usize::from(inner.width);
    let mut y = inner.y;
    let max_y = inner.bottom();
    for (i, b) in state.branches.iter().enumerate() {
        if y >= max_y {
            break;
        }
        let cur = if b.current { "*" } else { " " };
        let sel = if i == state.branch_cursor && focused {
            ">"
        } else {
            " "
        };
        // Ahead / behind is stated with catalog arrows, so an ASCII terminal
        // gets ASCII instead of a box (plans/013 Step 2).
        let up = "↑";
        let down = system.glyphs.resolve(Glyph::ArrowDown).text;
        let track = match (b.ahead, b.behind) {
            (0, 0) => String::new(),
            (a, 0) => format!(" {up}{a}"),
            (0, be) => format!(" {down}{be}"),
            (a, be) => format!(" {up}{a}{down}{be}"),
        };
        let line = format!("{sel}{cur}{}{track}", b.name);
        let mut style = system.style(if b.current {
            Role::TextStrong
        } else {
            Role::Text
        });
        if i == state.branch_cursor && focused {
            // Selection is chrome: the gutter marks it and the weight carries
            // it. A reversed slab hides which branch is current (plans/010).
            style = system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD)
                .patch(
                    system
                        .resolve_list_row(ListRowVisualState {
                            selected: true,
                            focused: true,
                            enabled: true,
                            ..Default::default()
                        })
                        .tint,
                );
        }
        system.paint_row(buffer, Rect::new(inner.x, y, inner.width, 1), &line, style);
        y = y.saturating_add(1);
    }
}

fn paint_confirm_banner(
    system: &DesignSystem,
    area: Rect,
    buffer: &mut Buffer,
    kind: &GitDestructiveKind,
    proceed: bool,
) {
    let consequence = kind.consequence();
    ConfirmPrompt::new(kind.label(), kind.label(), system)
        .detail(&consequence)
        .focus(if proceed {
            ConfirmFocus::Confirm
        } else {
            ConfirmFocus::Cancel
        })
        .paint(area, buffer);
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Demo branches.
#[must_use]
pub fn example_git_branches() -> Vec<GitBranch> {
    vec![
        GitBranch::new("main")
            .current()
            .upstream("origin/main")
            .tracking(0, 1),
        GitBranch::new("feature/auth")
            .upstream("origin/feature/auth")
            .tracking(2, 0),
        GitBranch::new("hotfix/login"),
    ]
}

/// Demo commits as timeline checkpoints.
#[must_use]
pub fn example_git_commits() -> Vec<Checkpoint> {
    // Reuse checkpoint shape with git-ish summaries
    let mut cps = example_checkpoints();
    if let Some(c) = cps.first_mut() {
        c.summary = Some("Initial import".into());
    }
    cps
}

/// Dirty file tree with git badges.
#[must_use]
pub fn example_git_files() -> Vec<FileTreeEntry<'static, &'static str>> {
    vec![
        FileTreeEntry::dir("src", "src", "src", 0).expanded(),
        FileTreeEntry::file("src/main.rs", "main.rs", "src/main.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Modified),
        FileTreeEntry::file("src/lib.rs", "lib.rs", "src/lib.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Added),
        FileTreeEntry::file("src/auth.rs", "auth.rs", "src/auth.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Conflict),
        FileTreeEntry::file("README.md", "README.md", "README.md", 0).git(FileGitStatus::Untracked),
        FileTreeEntry::file("gone.rs", "gone.rs", "gone.rs", 0).git(FileGitStatus::Deleted),
    ]
}

/// Demo diff lines.
#[must_use]
pub fn example_git_diff_lines() -> Vec<DiffLine<'static>> {
    vec![
        DiffLine::file_header("fh0", "diff --git a/src/main.rs b/src/main.rs")
            .file_id("src/main.rs"),
        DiffLine::hunk_header("hh0", "@@ -1,3 +1,4 @@")
            .hunk_id("h0")
            .file_id("src/main.rs"),
        DiffLine::context("c1", "fn main() {")
            .hunk_id("h0")
            .file_id("src/main.rs"),
        DiffLine::removed("r1", "    let x = 1;")
            .hunk_id("h0")
            .file_id("src/main.rs"),
        DiffLine::added("a1", "    let x = 2;")
            .hunk_id("h0")
            .file_id("src/main.rs"),
        DiffLine::context("c2", "}")
            .hunk_id("h0")
            .file_id("src/main.rs"),
        DiffLine::hunk_header("hh1", "@@ -10,2 +11,3 @@")
            .hunk_id("h1")
            .file_id("src/main.rs"),
        DiffLine::removed("r2", "    // todo")
            .hunk_id("h1")
            .file_id("src/main.rs"),
        DiffLine::added("a2", "    println!(\"ready\");")
            .hunk_id("h1")
            .file_id("src/main.rs"),
        DiffLine::added("a3", "    // sample")
            .hunk_id("h1")
            .file_id("src/main.rs"),
    ]
}

/// Demo hunks.
#[must_use]
pub fn example_git_hunks() -> Vec<DiffHunk> {
    vec![
        DiffHunk::new(1, 5, "@@ -1,3 +1,4 @@")
            .id("h0")
            .file_id("src/main.rs"),
        DiffHunk::new(6, 4, "@@ -10,2 +11,3 @@")
            .id("h1")
            .file_id("src/main.rs"),
    ]
}

/// Diff file rows.
#[must_use]
pub fn example_git_diff_files() -> Vec<DiffReviewFileRow<'static>> {
    vec![
        DiffReviewFileRow::new("src/main.rs", "src/main.rs").stats(3, 2),
        DiffReviewFileRow::new("src/lib.rs", "src/lib.rs").stats(12, 0),
    ]
}

/// Large multi-hunk projection for paint stress.
#[must_use]
pub fn large_git_diff(n_hunks: usize) -> (Vec<String>, Vec<DiffHunk>, Vec<DiffLine<'static>>) {
    // Note: DiffLine needs 'static text — we only return owned headers/bodies
    // for the host to leak or store; for tests use example lines + synthetic hunks.
    let mut hunks = Vec::with_capacity(n_hunks);
    for i in 0..n_hunks {
        let start = i * 4;
        hunks.push(
            DiffHunk::new(start, 4, format!("@@ -{i},3 +{i},4 @@"))
                .id(format!("H{i}"))
                .file_id("big.rs"),
        );
    }
    (Vec::new(), hunks, example_git_diff_lines())
}

/// Conflict diagnostics.
#[must_use]
pub fn example_conflict_diagnostics() -> Vec<Diagnostic<'static>> {
    vec![
        Diagnostic::new(
            "c1",
            DiagnosticSeverity::Error,
            "merge conflict in src/auth.rs",
        )
        .code("CONFLICT")
        .source("git")
        .file("src/auth.rs"),
        Diagnostic::new("c2", DiagnosticSeverity::Error, "both modified: Cargo.lock")
            .source("git")
            .file("Cargo.lock"),
    ]
}

/// Terminal meta for last git command.
#[must_use]
pub fn example_git_terminal_meta() -> TerminalCommandMeta<'static> {
    TerminalCommandMeta::new("git status -sb")
        .cwd("/repo")
        .status(TerminalRunStatus::Succeeded)
        .exit_code(0)
        .duration_ms(12)
}

/// Terminal lines.
#[must_use]
pub fn example_git_terminal_lines() -> Vec<TerminalLine<'static>> {
    vec![
        TerminalLine::stdout("1", "## main...origin/main [behind 1]"),
        TerminalLine::stdout("2", " M src/main.rs"),
        TerminalLine::stdout("3", "A  src/lib.rs"),
        TerminalLine::stderr("4", "warning: LF will be replaced by CRLF"),
    ]
}

/// Git-oriented help entries (extends generic help).
#[must_use]
pub fn example_git_help_entries() -> Vec<HelpEntry> {
    let mut e = example_help_entries();
    e.push(HelpEntry::new("stage", "Git", "t", "Stage selection (DiffReview)").priority(15));
    e.push(HelpEntry::new("unstage", "Git", "T", "Unstage selection").priority(15));
    e.push(HelpEntry::new("discard", "Git", "x", "Discard path (confirm)").priority(16));
    e.push(HelpEntry::new("full", "Git", "C-f", "Fullscreen diff").priority(17));
    e
}

/// Clean worktree file list (all tracked clean).
#[must_use]
pub fn example_clean_files() -> Vec<FileTreeEntry<'static, &'static str>> {
    vec![
        FileTreeEntry::dir("src", "src", "src", 0).expanded(),
        FileTreeEntry::file("src/main.rs", "main.rs", "src/main.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Clean),
        FileTreeEntry::file("src/lib.rs", "lib.rs", "src/lib.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Clean),
        FileTreeEntry::file("README.md", "README.md", "README.md", 0).git(FileGitStatus::Clean),
    ]
}

/// Empty repo / no paths projected.
#[must_use]
pub fn example_empty_files() -> Vec<FileTreeEntry<'static, &'static str>> {
    Vec::new()
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 20;
    /// Synthetic hunks for stress layout.
    pub const LARGE_HUNKS: usize = 80;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::DiffReviewUnitKind;
    use crate::widgets::tests::press;

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn meta_empty() -> TerminalCommandMeta<'static> {
        TerminalCommandMeta::new("git status")
    }

    fn hk(st: &mut GitWorkbenchState, key: KeyEvent) -> GitWorkbenchOutcome {
        st.handle_key(key, &[], &[], &[], &[], &[], &[], &[], &meta_empty())
    }

    fn hk_diff(
        st: &mut GitWorkbenchState,
        key: KeyEvent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
        files: &[DiffReviewFileRow<'_>],
    ) -> GitWorkbenchOutcome {
        st.handle_key(key, &[], hunks, lines, files, &[], &[], &[], &meta_empty())
    }

    fn hk_help(
        st: &mut GitWorkbenchState,
        key: KeyEvent,
        help: &[&HelpEntry],
    ) -> GitWorkbenchOutcome {
        st.handle_key(key, &[], &[], &[], &[], help, &[], &[], &meta_empty())
    }

    fn open() -> GitWorkbenchState {
        let mut st = GitWorkbenchState::new();
        st.repo_status = GitRepoStatus::Dirty;
        st
    }

    #[test]
    fn focus_cycle_visits_zones() {
        let mut st = open();
        st.density = Some(GitWorkbenchDensity::Normal);
        let order = st.focus_order_for(GitWorkbenchDensity::Normal);
        assert!(order.contains(&"files"));
        assert!(order.contains(&"diff"));
        assert!(order.contains(&"history"));
        st.focus = "files";
        for _ in 0..order.len() {
            let out = st.focus_next(GitWorkbenchDensity::Normal);
            assert!(matches!(out, GitWorkbenchOutcome::FocusChanged(_)));
        }
        assert_eq!(st.focus, "files");
    }

    #[test]
    fn history_focused_chrome_tracks_workbench_focus() {
        let mut st = open();
        st.density = Some(GitWorkbenchDensity::Normal);
        // Default CheckpointTimelineState.focused is true — workbench must clear it.
        let _ = st.set_focus(GitWorkbenchPane::Files);
        assert!(
            !st.history.focused,
            "history must not paint focused chrome when files own focus"
        );
        let _ = st.set_focus(GitWorkbenchPane::History);
        assert!(
            st.history.focused,
            "history must paint focused chrome when history owns focus"
        );
        // Paint path re-syncs from state.focus (not a dead local `focused` discard).
        let system = DesignSystem::default();
        let files = example_git_files();
        let lines = example_git_diff_lines();
        let hunks = example_git_hunks();
        let dfiles = example_git_diff_files();
        let commits = example_git_commits();
        let diags = example_conflict_diagnostics();
        let meta = example_git_terminal_meta();
        let tlines = example_git_terminal_lines();
        let help = example_git_help_entries();
        let help_refs: Vec<&HelpEntry> = help.iter().collect();
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        st.focus = "files";
        st.history.set_focused(true); // corrupt; paint must correct via set_focused
        paint_git_workbench(
            &mut buf,
            area,
            GitWorkbenchSurfaces {
                system: &system,
                state: &mut st,
                files: &files,
                diff_lines: &lines,
                hunks: &hunks,
                diff_files: &dfiles,
                commits: &commits,
                diagnostics: &diags,
                terminal_meta: &meta,
                terminal_lines: &tlines,
                help_entries: &help_refs,
            },
        );
        assert!(
            !st.history.focused,
            "paint_git_workbench must set history.focused from workbench focus"
        );
        st.focus = "history";
        paint_git_workbench(
            &mut buf,
            area,
            GitWorkbenchSurfaces {
                system: &system,
                state: &mut st,
                files: &files,
                diff_lines: &lines,
                hunks: &hunks,
                diff_files: &dfiles,
                commits: &commits,
                diagnostics: &diags,
                terminal_meta: &meta,
                terminal_lines: &tlines,
                help_entries: &help_refs,
            },
        );
        assert!(st.history.focused);
    }

    #[test]
    fn narrow_tiny_drop_panes_and_tab_clamps() {
        let ws = WorkspaceState::new();
        let normal = git_workbench_layout_density(
            Rect::new(0, 0, 120, 40),
            &ws,
            GitWorkbenchDensity::Normal,
            false,
            false,
        );
        let narrow = git_workbench_layout_density(
            Rect::new(0, 0, 70, 24),
            &ws,
            GitWorkbenchDensity::Narrow,
            false,
            false,
        );
        let tiny = git_workbench_layout_density(
            Rect::new(0, 0, 40, 16),
            &ws,
            GitWorkbenchDensity::Tiny,
            false,
            false,
        );
        let ids = |p: &[PaneGeom]| {
            p.iter()
                .filter(|g| !g.collapsed && g.area.width > 0 && g.area.height > 0)
                .map(|g| g.id.0.clone())
                .collect::<Vec<_>>()
        };
        let n = ids(&normal);
        let w = ids(&narrow);
        let t = ids(&tiny);
        assert!(n.iter().any(|i| i == "history"), "{n:?}");
        assert!(
            !w.iter().any(|i| i == "history"),
            "narrow drops history: {w:?}"
        );
        assert!(
            !t.iter().any(|i| i == "history") && !t.iter().any(|i| i == "branches"),
            "tiny drops side panes: {t:?}"
        );

        let mut st = open();
        st.density = None;
        let _ = st.layout(Rect::new(0, 0, 70, 24));
        assert_eq!(st.effective_density(), GitWorkbenchDensity::Narrow);
        st.focus = "history";
        let _ = hk(&mut st, press(KeyCode::Tab));
        assert_ne!(st.focus, "history");
        for _ in 0..10 {
            let _ = hk(&mut st, press(KeyCode::Tab));
            assert_ne!(st.focus, "history");
            assert_ne!(st.focus, "branches");
        }
    }

    #[test]
    fn fullscreen_diff_promotes() {
        let mut st = open();
        let out = st.set_fullscreen_diff(true);
        assert!(matches!(
            out,
            GitWorkbenchOutcome::FullscreenDiff { on: true }
        ));
        assert_eq!(st.focus, "diff");
        let panes = st.layout(Rect::new(0, 0, 100, 30));
        assert!(
            panes
                .iter()
                .filter(|p| !p.collapsed && p.area.width > 0)
                .all(|p| p.id.0 == "diff" || p.id.0 == "status"),
            "fullscreen only diff+status"
        );
        let _ = hk(&mut st, press(KeyCode::Esc));
        assert!(!st.fullscreen_diff);
    }

    #[test]
    fn stage_outcome_from_diff() {
        let mut st = open();
        st.focus = "diff";
        st.diff.set_accepts_input(true);
        let lines = example_git_diff_lines();
        let hunks = example_git_hunks();
        let dfiles = example_git_diff_files();
        // DiffReview stage chord is `t` (not `s`) — workbench forwards lines/files.
        let out = hk_diff(&mut st, press(KeyCode::Char('t')), &lines, &hunks, &dfiles);
        match out {
            GitWorkbenchOutcome::StageRequested { units } => {
                assert!(!units.is_empty(), "stage must target units");
                assert!(
                    units
                        .iter()
                        .any(|u| u.key().contains("h0") || u.key().contains("hunk")),
                    "expected hunk unit ids, got {:?}",
                    units.iter().map(|u| u.key()).collect::<Vec<_>>()
                );
            }
            other => panic!("expected StageRequested via t chord, got {other:?}"),
        }
        // Unstage chord T
        let out = hk_diff(&mut st, press(KeyCode::Char('T')), &lines, &hunks, &dfiles);
        assert!(
            matches!(out, GitWorkbenchOutcome::UnstageRequested { ref units } if !units.is_empty()),
            "{out:?}"
        );
        // `s` is not stage through workbench (DiffReview uses s for mode cycle)
        let out = hk_diff(&mut st, press(KeyCode::Char('s')), &lines, &hunks, &dfiles);
        assert!(
            !matches!(out, GitWorkbenchOutcome::StageRequested { .. }),
            "s must not stage: {out:?}"
        );
    }

    #[test]
    fn conflict_layout_emits_diagnostics_pane() {
        let ws = WorkspaceState::new();
        let panes = git_workbench_layout_density(
            Rect::new(0, 0, 120, 40),
            &ws,
            GitWorkbenchDensity::Normal,
            false,
            true,
        );
        assert!(
            panes
                .iter()
                .any(|p| p.id.0 == "diagnostics" && !p.collapsed && p.area.height > 0),
            "conflict layout must allocate diagnostics: {:?}",
            panes.iter().map(|p| p.id.0.as_str()).collect::<Vec<_>>()
        );
        let mut st = open();
        st.repo_status = GitRepoStatus::Conflict;
        let _ = st.layout(Rect::new(0, 0, 120, 36));
        assert!(st.include_diagnostics());
        assert!(
            st.last_panes()
                .iter()
                .any(|p| p.id.0 == "diagnostics" && p.area.height > 0),
            "state.layout must emit diagnostics when conflict"
        );
        // Tab can land on diagnostics only when pane exists
        st.focus = "output";
        let order = st.focus_order_for(GitWorkbenchDensity::Normal);
        assert!(order.contains(&"diagnostics"));
        let _ = st.focus_next(GitWorkbenchDensity::Normal);
        // From output, next may be diagnostics
        st.focus = "diagnostics";
        st.clamp_focus_to_density(GitWorkbenchDensity::Normal);
        assert_eq!(st.focus, "diagnostics");
    }

    #[test]
    fn clean_and_empty_fixtures() {
        let clean = example_clean_files();
        assert!(!clean.is_empty());
        assert!(clean.iter().all(|e| e.git == FileGitStatus::Clean));
        let empty = example_empty_files();
        assert!(empty.is_empty());

        let system = DesignSystem::default();
        let mut st = open();
        st.repo_status = GitRepoStatus::Clean;
        let lines = example_git_diff_lines();
        let hunks = example_git_hunks();
        let dfiles = example_git_diff_files();
        let commits = example_git_commits();
        let diags: Vec<Diagnostic<'static>> = Vec::new();
        let meta = example_git_terminal_meta();
        let tlines = example_git_terminal_lines();
        let help = example_git_help_entries();
        let help_refs: Vec<&HelpEntry> = help.iter().collect();
        let area = Rect::new(0, 0, 100, 28);
        let mut buf = Buffer::empty(area);
        // Clean paint
        paint_git_workbench(
            &mut buf,
            area,
            GitWorkbenchSurfaces {
                system: &system,
                state: &mut st,
                files: &clean,
                diff_lines: &lines,
                hunks: &hunks,
                diff_files: &dfiles,
                commits: &commits,
                diagnostics: &diags,
                terminal_meta: &meta,
                terminal_lines: &tlines,
                help_entries: &help_refs,
            },
        );
        assert!(!st.last_panes().is_empty());
        // Empty repo paint
        st.repo_status = GitRepoStatus::Clean;
        paint_git_workbench(
            &mut buf,
            area,
            GitWorkbenchSurfaces {
                system: &system,
                state: &mut st,
                files: &empty,
                diff_lines: &[],
                hunks: &[],
                diff_files: &[],
                commits: &[],
                diagnostics: &diags,
                terminal_meta: &meta,
                terminal_lines: &[],
                help_entries: &help_refs,
            },
        );
    }

    #[test]
    fn discard_confirm_cancel_default() {
        let mut st = open();
        let out = st.open_discard_confirm(vec!["src/main.rs".into()]);
        assert!(matches!(
            out,
            GitWorkbenchOutcome::ConfirmOpened(GitDestructiveKind::Discard { .. })
        ));
        assert!(!st.confirm_proceed_focused);
        // Enter on Cancel → cancelled, not discard
        let out = hk(&mut st, press(KeyCode::Enter));
        assert!(matches!(out, GitWorkbenchOutcome::ConfirmCancelled));
        // y unbound
        let _ = st.open_discard_confirm(vec!["src/main.rs".into()]);
        assert!(matches!(
            hk(&mut st, press(KeyCode::Char('y'))),
            GitWorkbenchOutcome::Ignored
        ));
        // Proceed
        let _ = st.open_discard_confirm(vec!["src/main.rs".into()]);
        let _ = hk(&mut st, press(KeyCode::Right));
        assert!(st.confirm_proceed_focused);
        let out = hk(&mut st, press(KeyCode::Enter));
        assert!(matches!(
            out,
            GitWorkbenchOutcome::DiscardRequested { ref paths } if paths == &["src/main.rs".to_string()]
        ));
    }

    #[test]
    fn discard_without_confirm_does_not_emit() {
        let mut st = open();
        st.focus = "files";
        // Opening confirm is not DiscardRequested
        let out = st.open_discard_confirm(vec!["a".into()]);
        assert!(!matches!(out, GitWorkbenchOutcome::DiscardRequested { .. }));
    }

    #[test]
    fn conflict_and_dirty_chrome() {
        let mut st = open();
        st.repo_status = GitRepoStatus::Conflict;
        let order = st.focus_order_for(GitWorkbenchDensity::Normal);
        assert!(order.contains(&"diagnostics"));
        let slots = st.status_slots();
        assert!(slots.iter().any(|s| s.id == "conflict" || s.id == "repo"));
        st.repo_status = GitRepoStatus::Dirty;
        assert_eq!(st.repo_status.label(), "dirty");
    }

    #[test]
    fn checkout_dirty_opens_confirm() {
        let mut st = open();
        st.repo_status = GitRepoStatus::Dirty;
        st.focus = "branches";
        st.branch_cursor = 1; // feature/auth
        let out = hk(&mut st, press(KeyCode::Enter));
        assert!(matches!(
            out,
            GitWorkbenchOutcome::ConfirmOpened(GitDestructiveKind::Checkout { .. })
        ));
    }

    #[test]
    fn paint_composes_children_and_no_git_io() {
        let system = DesignSystem::default();
        let mut st = open();
        let files = example_git_files();
        let lines = example_git_diff_lines();
        let hunks = example_git_hunks();
        let dfiles = example_git_diff_files();
        let commits = example_git_commits();
        let diags = example_conflict_diagnostics();
        let meta = example_git_terminal_meta();
        let tlines = example_git_terminal_lines();
        let help = example_git_help_entries();
        let help_refs: Vec<&HelpEntry> = help.iter().collect();

        for d in [
            GitWorkbenchDensity::Normal,
            GitWorkbenchDensity::Narrow,
            GitWorkbenchDensity::Tiny,
        ] {
            st.density = Some(d);
            st.fullscreen_diff = false;
            let area = match d {
                GitWorkbenchDensity::Normal => Rect::new(0, 0, 120, 36),
                GitWorkbenchDensity::Narrow => Rect::new(0, 0, 70, 24),
                GitWorkbenchDensity::Tiny => Rect::new(0, 0, 40, 16),
            };
            let mut buf = Buffer::empty(area);
            paint_git_workbench(
                &mut buf,
                area,
                GitWorkbenchSurfaces {
                    system: &system,
                    state: &mut st,
                    files: &files,
                    diff_lines: &lines,
                    hunks: &hunks,
                    diff_files: &dfiles,
                    commits: &commits,
                    diagnostics: &diags,
                    terminal_meta: &meta,
                    terminal_lines: &tlines,
                    help_entries: &help_refs,
                },
            );
            assert!(!st.last_panes().is_empty());
        }

        let src = include_str!("git_workbench.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for needle in [
            "DiffReview",
            "FileTree",
            "CheckpointTimeline",
            "TerminalOutput",
            "DiagnosticView",
            "StatusBar",
            "KeyboardHelp",
            "StageRequested",
            "DiscardRequested",
            "FullscreenDiff",
        ] {
            assert!(body.contains(needle), "missing: {needle}");
        }
        for forbidden in [
            "std::process::",
            "Command::new",
            "git2::",
            "std::fs::",
            "TcpStream",
            "std::net::",
        ] {
            assert!(!body.contains(forbidden), "forbidden I/O: {forbidden}");
        }
    }

    #[test]
    fn large_diff_paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.density = Some(GitWorkbenchDensity::Normal);
        let files = example_git_files();
        let lines = example_git_diff_lines();
        let (_, hunks, _) = large_git_diff(bench::LARGE_HUNKS);
        // Repeat lines for visual mass
        let mut many_lines = Vec::new();
        for _ in 0..20 {
            many_lines.extend(lines.iter().cloned());
        }
        // Fix hunk indices loosely for paint (DiffReview windows)
        let dfiles = example_git_diff_files();
        let commits = example_git_commits();
        let diags = example_conflict_diagnostics();
        let meta = example_git_terminal_meta();
        let tlines = example_git_terminal_lines();
        let help = example_git_help_entries();
        let help_refs: Vec<&HelpEntry> = help.iter().collect();
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            paint_git_workbench(
                &mut buf,
                area,
                GitWorkbenchSurfaces {
                    system: &system,
                    state: &mut st,
                    files: &files,
                    diff_lines: &many_lines,
                    hunks: &hunks,
                    diff_files: &dfiles,
                    commits: &commits,
                    diagnostics: &diags,
                    terminal_meta: &meta,
                    terminal_lines: &tlines,
                    help_entries: &help_refs,
                },
            );
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 5, "paint too slow: {elapsed:?}");
    }

    #[test]
    fn help_and_refresh() {
        let mut st = open();
        let help = example_git_help_entries();
        let help_refs: Vec<&HelpEntry> = help.iter().collect();
        let out = hk_help(&mut st, press(KeyCode::Char('?')), &help_refs);
        assert!(matches!(out, GitWorkbenchOutcome::OpenHelp));
        assert!(st.help_is_open());
        st.help_open = false;
        let _ = st.help.close_modal();
        let out = hk_help(&mut st, ctrl(KeyCode::Char('r')), &help_refs);
        assert!(
            matches!(out, GitWorkbenchOutcome::RefreshRequested),
            "{out:?}"
        );
    }

    #[test]
    fn fuzz_status_and_units() {
        for s in [
            GitRepoStatus::Clean,
            GitRepoStatus::Dirty,
            GitRepoStatus::Conflict,
            GitRepoStatus::Detached,
            GitRepoStatus::Merging,
            GitRepoStatus::Rebasing,
        ] {
            assert!(!s.id().is_empty());
        }
        assert!(!DiffReviewUnit::hunk("h").key().is_empty());
        assert_eq!(DiffReviewUnitKind::File, DiffReviewUnit::file("a").kind);
    }

    #[test]
    fn unit_ids_in_stage_path() {
        let unit = DiffReviewUnit::line_range("L1", "L5");
        let key = unit.key();
        assert!(
            key.contains("L1") || key.contains("line") || key.contains("range"),
            "{key}"
        );
        let file = DiffReviewUnit::file("src/main.rs");
        assert_eq!(file.key(), "file:src/main.rs");
    }
}
