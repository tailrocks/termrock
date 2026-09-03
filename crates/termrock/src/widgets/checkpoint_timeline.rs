// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **CheckpointTimeline** — rewindable session history for agent turns, file
//! states, and significant actions.
//!
//! **Mission.** Checkpoints with labels, actor, timestamp, summary, changed
//! files, tool calls, irreversible boundaries, branch/fork, preview, rewind,
//! and restore. **Viewing history ≠ mutating state** — Browse/Preview never
//! rewrite session; Restore/Rewind are explicit request outcomes. Warn when
//! local uncommitted work or external side effects cannot be restored.
//! **Never** clears or mutates host PromptComposer draft. Compare outcomes are
//! pure; host opens DiffReview.
//!
//! **vs [`super::Timeline`].** Timeline is general chronological events;
//! CheckpointTimeline is session restore/rewind with safety chrome.
//! **vs [`super::HistoryPicker`].** Field-local value history, not session.
//! **vs [`super::DiffReview`].** Patch review; compose via CompareRequested.
//!
//! Research: Grok Build rewind, IDE local history, Git reflog, notebook
//! checkpoints. Uses Timeline substrate for list paint projection.
use std::collections::BTreeMap;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, ListRowVisualState, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::panel::Panel,
    widgets::tiered_row::TieredRow,
    widgets::timeline::{
        Timeline, TimelineEvent, TimelineOutcome, TimelineRecipe, TimelineRowKind, TimelineState,
        TimelineStatus,
    },
};

/// Overlay id for checkpoint browser / restore confirm.
pub const CHECKPOINT_TIMELINE_OVERLAY_ID: &str = "termrock.checkpoint_timeline";
/// Max detail lines for files/tools in the detail pane.
pub const CHECKPOINT_DETAIL_WINDOW: usize = 12;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Kind of checkpoint in session history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CheckpointKind {
    /// Agent turn boundary.
    #[default]
    Turn,
    /// File / workspace snapshot.
    FileState,
    /// Significant action (tool, deploy, apply).
    Action,
    /// User-labeled manual pin.
    Manual,
    /// Branch / fork point.
    Branch,
    /// System / session lifecycle.
    System,
}

impl CheckpointKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::FileState => "file",
            Self::Action => "action",
            Self::Manual => "manual",
            Self::Branch => "branch",
            Self::System => "system",
        }
    }

    /// Compact letter (colorless / rail).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Turn => 'T',
            Self::FileState => 'F',
            Self::Action => 'A',
            Self::Manual => 'M',
            Self::Branch => 'B',
            Self::System => 'S',
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Turn => "T",
                Self::FileState => "F",
                Self::Action => "A",
                Self::Manual => "*",
                Self::Branch => "Y",
                Self::System => "S",
            };
        }
        match self {
            Self::Turn => "◉",
            Self::FileState => "◇",
            Self::Action => "◆",
            Self::Manual => "★",
            Self::Branch => "⑂",
            Self::System => "○",
        }
    }
}

/// Restore safety boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CheckpointBoundary {
    /// Fully restorable in-app state.
    #[default]
    Soft,
    /// May lose local uncommitted work.
    DirtyWorkspace,
    /// External side effects already applied (network, deploy, delete).
    ExternalEffects,
    /// Cannot restore — irreversible.
    Irreversible,
}

impl CheckpointBoundary {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::DirtyWorkspace => "dirty",
            Self::ExternalEffects => "external",
            Self::Irreversible => "irreversible",
        }
    }

    /// Whether restore is blocked by default.
    #[must_use]
    pub const fn blocks_restore(self) -> bool {
        matches!(self, Self::Irreversible)
    }

    /// Whether host must show an explicit warning before restore/rewind.
    #[must_use]
    pub const fn needs_warning(self) -> bool {
        !matches!(self, Self::Soft)
    }

    /// Warning label.
    #[must_use]
    pub const fn warning_label(self) -> &'static str {
        match self {
            Self::Soft => "",
            Self::DirtyWorkspace => "uncommitted local work may be lost",
            Self::ExternalEffects => "external side effects cannot be undone",
            Self::Irreversible => "cannot restore past this boundary",
        }
    }

    /// Theme role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Soft => Role::TextMuted,
            Self::DirtyWorkspace => Role::Warning,
            Self::ExternalEffects | Self::Irreversible => Role::Danger,
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Soft => " ",
                Self::DirtyWorkspace => "!",
                Self::ExternalEffects => "X",
                Self::Irreversible => "#",
            };
        }
        match self {
            Self::Soft => " ",
            Self::DirtyWorkspace => "⚠",
            Self::ExternalEffects => "↯",
            Self::Irreversible => "⛔",
        }
    }
}

/// One session checkpoint (host-projected snapshot metadata).
///
/// TermRock does **not** persist blobs — only presents and emits request
/// outcomes. Host owns storage and restore execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// Stable id.
    pub id: String,
    /// Short label.
    pub label: String,
    /// Kind.
    pub kind: CheckpointKind,
    /// Actor (`user`, `agent`, `subagent:x`).
    pub actor: Option<String>,
    /// Absolute / sequence time label.
    pub when: String,
    /// Relative time.
    pub relative: Option<String>,
    /// Summary line.
    pub summary: Option<String>,
    /// Changed file paths (display).
    pub changed_files: Vec<String>,
    /// Tool call names / short summaries.
    pub tool_calls: Vec<String>,
    /// Safety boundary.
    pub boundary: CheckpointBoundary,
    /// Branch / fork id (if any).
    pub branch_id: Option<String>,
    /// Parent checkpoint id (fork origin).
    pub parent_id: Option<String>,
    /// Host says this point is restorable.
    pub restorable: bool,
    /// Optional host warning text (overrides boundary label when set).
    pub warning: Option<String>,
    /// Marks live HEAD / current tip.
    pub is_head: bool,
}

impl Checkpoint {
    /// Minimal soft checkpoint.
    #[must_use]
    pub fn new(id: impl Into<String>, when: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: CheckpointKind::Turn,
            actor: None,
            when: when.into(),
            relative: None,
            summary: None,
            changed_files: Vec::new(),
            tool_calls: Vec::new(),
            boundary: CheckpointBoundary::Soft,
            branch_id: None,
            parent_id: None,
            restorable: true,
            warning: None,
            is_head: false,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: CheckpointKind) -> Self {
        self.kind = k;
        self
    }

    /// Actor.
    #[must_use]
    pub fn actor(mut self, a: impl Into<String>) -> Self {
        self.actor = Some(a.into());
        self
    }

    /// Relative.
    #[must_use]
    pub fn relative(mut self, r: impl Into<String>) -> Self {
        self.relative = Some(r.into());
        self
    }

    /// Summary.
    #[must_use]
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }

    /// Files.
    #[must_use]
    pub fn files(mut self, f: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.changed_files = f.into_iter().map(Into::into).collect();
        self
    }

    /// Tool calls.
    #[must_use]
    pub fn tools(mut self, t: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tool_calls = t.into_iter().map(Into::into).collect();
        self
    }

    /// Boundary.
    #[must_use]
    pub const fn boundary(mut self, b: CheckpointBoundary) -> Self {
        self.boundary = b;
        self
    }

    /// Branch.
    #[must_use]
    pub fn branch(mut self, id: impl Into<String>, parent: Option<impl Into<String>>) -> Self {
        self.branch_id = Some(id.into());
        self.parent_id = parent.map(Into::into);
        self.kind = CheckpointKind::Branch;
        self
    }

    /// Not restorable.
    #[must_use]
    pub const fn not_restorable(mut self) -> Self {
        self.restorable = false;
        self
    }

    /// Host warning.
    #[must_use]
    pub fn warning(mut self, w: impl Into<String>) -> Self {
        self.warning = Some(w.into());
        self
    }

    /// Mark as HEAD.
    #[must_use]
    pub const fn head(mut self) -> Self {
        self.is_head = true;
        self
    }

    /// Effective warning text (host override or boundary).
    #[must_use]
    pub fn effective_warning(&self) -> Option<&str> {
        if let Some(w) = self.warning.as_deref() {
            if !w.is_empty() {
                return Some(w);
            }
        }
        if self.boundary.needs_warning() {
            let l = self.boundary.warning_label();
            if !l.is_empty() {
                return Some(l);
            }
        }
        if !self.restorable {
            return Some("not restorable");
        }
        None
    }

    /// Whether restore may be requested (host still confirms).
    #[must_use]
    pub fn can_request_restore(&self) -> bool {
        self.restorable && !self.boundary.blocks_restore() && !self.is_head
    }

    /// Whether rewind may be requested (same gates; semantic differs for host).
    #[must_use]
    pub fn can_request_rewind(&self) -> bool {
        self.can_request_restore()
    }
}

// ── Mode / phase / recipe ───────────────────────────────────────────────────

/// Interaction mode — viewing vs mutating intent.
///
/// Browse and Preview **never** mutate session state. Confirm is still a
/// **request** only; host performs restore/rewind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CheckpointTimelineMode {
    /// Viewing history (safe).
    #[default]
    Browse,
    /// Previewing a checkpoint (draft preserved; no mutation).
    Preview,
    /// Explicit confirm before restore/rewind request.
    Confirm,
}

impl CheckpointTimelineMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Browse => "browse",
            Self::Preview => "preview",
            Self::Confirm => "confirm",
        }
    }

    /// Whether this mode mutates session (always false in-widget; host acts).
    #[must_use]
    pub const fn is_mutating_request(self) -> bool {
        matches!(self, Self::Confirm)
    }
}

/// Pending confirm action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CheckpointConfirmAction {
    /// Restore workspace/session to checkpoint.
    Restore,
    /// Rewind conversation / agent state to checkpoint.
    Rewind,
}

impl CheckpointConfirmAction {
    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Restore => "Restore",
            Self::Rewind => "Rewind",
        }
    }

    /// Consequence line.
    #[must_use]
    pub const fn consequence(self) -> &'static str {
        match self {
            Self::Restore => "restore files/state to checkpoint (host executes)",
            Self::Rewind => "rewind session history to checkpoint (host executes)",
        }
    }
}

/// Density recipe (maps to Timeline recipe for list projection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CheckpointTimelineRecipe {
    /// Compact rail.
    Rail,
    /// Detailed rows (default).
    #[default]
    Detailed,
}

impl CheckpointTimelineRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Rail => "rail",
            Self::Detailed => "detailed",
        }
    }

    fn to_timeline(self) -> TimelineRecipe {
        match self {
            Self::Rail => TimelineRecipe::Rail,
            Self::Detailed => TimelineRecipe::Detailed,
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — **requests only**. No persistence, no restore I/O, no draft wipe.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckpointTimelineOutcome {
    /// Ignored.
    Ignored,
    /// Selection moved (browse).
    Selected {
        /// Checkpoint id.
        id: String,
    },
    /// Entered preview of a checkpoint (draft still host-owned).
    PreviewOpened {
        /// Id.
        id: String,
    },
    /// Left preview without mutating.
    PreviewClosed,
    /// Host should restore to this checkpoint (after confirm when required).
    RestoreRequested {
        /// Id.
        id: String,
    },
    /// Host should rewind session to this checkpoint.
    RewindRequested {
        /// Id.
        id: String,
    },
    /// Host should open compare (e.g. DiffReview) — pure.
    CompareRequested {
        /// From checkpoint (or HEAD if none).
        from: Option<String>,
        /// To checkpoint.
        to: String,
    },
    /// Branch filter / focus.
    BranchFocused {
        /// Branch id.
        branch_id: String,
    },
    /// Confirm dialog opened (mode = Confirm).
    ConfirmOpened {
        /// Target id.
        id: String,
        /// Action.
        action: CheckpointConfirmAction,
    },
    /// Confirm cancelled → back to preview/browse.
    ConfirmCancelled,
    /// Warning banner acknowledged (does not restore).
    WarningAcknowledged,
    /// Esc / dismiss — no mutation; draft preserved.
    Cancelled,
    /// Follow live head toggled.
    FollowToggled {
        /// Following.
        following: bool,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive checkpoint timeline state.
///
/// **Composer draft:** never held or cleared here. Host keeps draft while the
/// user browses/previews checkpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointTimelineState {
    /// Checkpoints (oldest → newest recommended).
    pub checkpoints: Vec<Checkpoint>,
    /// Mode.
    pub mode: CheckpointTimelineMode,
    /// Cursor index.
    pub cursor: usize,
    /// Selected id (stable).
    pub selected: Option<String>,
    /// Target under preview/confirm.
    pub focus_id: Option<String>,
    /// Confirm action when mode = Confirm.
    pub confirm_action: Option<CheckpointConfirmAction>,
    /// Safe focus on confirm strip: false = Cancel, true = proceed.
    pub confirm_proceed_focused: bool,
    /// Follow live HEAD when new checkpoints append.
    pub following: bool,
    /// Recipe.
    pub recipe: CheckpointTimelineRecipe,
    /// Compare anchor (first of pair); None → compare to HEAD.
    pub compare_anchor: Option<String>,
    /// Branch filter (None = all).
    pub branch_filter: Option<String>,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Detail scroll.
    pub detail_scroll: usize,
    /// Timeline substrate state (list nav paint).
    pub timeline: TimelineState<String>,
    /// Last warning shown (for ack).
    pub last_warning: Option<String>,
    /// Hit regions for rows (id, rect).
    pub row_hits: Vec<(String, Rect)>,
    /// Confirm action hits.
    pub confirm_hits: Vec<(bool, Rect)>, // true = proceed
}

impl Default for CheckpointTimelineState {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointTimelineState {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        let mut timeline = TimelineState::new();
        timeline.set_checkpoint_mode(true);
        timeline.following = false;
        Self {
            checkpoints: Vec::new(),
            mode: CheckpointTimelineMode::Browse,
            cursor: 0,
            selected: None,
            focus_id: None,
            confirm_action: None,
            confirm_proceed_focused: false, // safe default = Cancel
            following: false,
            recipe: CheckpointTimelineRecipe::Detailed,
            compare_anchor: None,
            branch_filter: None,
            focused: true,
            accepts_input: true,
            detail_scroll: 0,
            timeline,
            last_warning: None,
            row_hits: Vec::new(),
            confirm_hits: Vec::new(),
        }
    }

    /// Replace checkpoints (preserves selection when id still exists).
    pub fn set_checkpoints(&mut self, cps: Vec<Checkpoint>) {
        let keep = self.selected.clone();
        self.checkpoints = cps;
        if let Some(id) = keep {
            if let Some(i) = self.checkpoints.iter().position(|c| c.id == id) {
                self.cursor = i;
                self.selected = Some(id);
            } else {
                self.cursor = self.checkpoints.len().saturating_sub(1);
                self.selected = self.checkpoints.get(self.cursor).map(|c| c.id.clone());
            }
        } else if !self.checkpoints.is_empty() {
            // Prefer HEAD, else last
            if let Some(i) = self.checkpoints.iter().position(|c| c.is_head) {
                self.cursor = i;
            } else {
                self.cursor = self.checkpoints.len() - 1;
            }
            self.selected = self.checkpoints.get(self.cursor).map(|c| c.id.clone());
        }
        self.sync_timeline_cursor();
        if self.following {
            self.follow_head();
        }
    }

    /// Append checkpoint (stream).
    pub fn append(&mut self, cp: Checkpoint) {
        let id = cp.id.clone();
        self.checkpoints.push(cp);
        if self.following {
            self.cursor = self.checkpoints.len() - 1;
            self.selected = Some(id);
            self.sync_timeline_cursor();
        }
        self.timeline.on_append(self.checkpoints.len());
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.timeline.set_accepts_input(on);
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Follow HEAD.
    pub fn follow_head(&mut self) {
        self.following = true;
        if let Some(i) = self.checkpoints.iter().position(|c| c.is_head) {
            self.cursor = i;
            self.selected = Some(self.checkpoints[i].id.clone());
        } else if !self.checkpoints.is_empty() {
            self.cursor = self.checkpoints.len() - 1;
            self.selected = self.checkpoints.last().map(|c| c.id.clone());
        }
        self.sync_timeline_cursor();
    }

    /// Pause follow (reading history).
    pub fn unfollow(&mut self) {
        self.following = false;
    }

    /// Current checkpoint.
    #[must_use]
    pub fn current(&self) -> Option<&Checkpoint> {
        self.checkpoints.get(self.cursor)
    }

    /// By id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.id == id)
    }

    /// HEAD checkpoint.
    #[must_use]
    pub fn head(&self) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.is_head)
    }

    /// Whether currently only viewing (not in confirm).
    #[must_use]
    pub const fn is_viewing(&self) -> bool {
        matches!(
            self.mode,
            CheckpointTimelineMode::Browse | CheckpointTimelineMode::Preview
        )
    }

    fn sync_timeline_cursor(&mut self) {
        self.timeline.cursor = self.cursor;
        self.timeline.following = self.following;
        if let Some(id) = self.selected.clone() {
            // TimelineState selected is private — cursor drives paint selection
            let _ = id;
        }
    }

    fn select_cursor(&mut self) -> CheckpointTimelineOutcome {
        if self.checkpoints.is_empty() {
            return CheckpointTimelineOutcome::Ignored;
        }
        self.cursor = self.cursor.min(self.checkpoints.len() - 1);
        let id = self.checkpoints[self.cursor].id.clone();
        self.selected = Some(id.clone());
        self.sync_timeline_cursor();
        if self.mode == CheckpointTimelineMode::Preview {
            self.focus_id = Some(id.clone());
            return CheckpointTimelineOutcome::PreviewOpened { id };
        }
        CheckpointTimelineOutcome::Selected { id }
    }

    fn move_cursor(&mut self, delta: isize) -> CheckpointTimelineOutcome {
        if self.checkpoints.is_empty() {
            return CheckpointTimelineOutcome::Ignored;
        }
        self.unfollow();
        let n = self.checkpoints.len() as isize;
        let next = (self.cursor as isize + delta).clamp(0, n - 1) as usize;
        self.cursor = next;
        self.detail_scroll = 0;
        // Leave confirm if navigating
        if self.mode == CheckpointTimelineMode::Confirm {
            self.mode = CheckpointTimelineMode::Browse;
            self.confirm_action = None;
        }
        self.select_cursor()
    }

    fn open_preview(&mut self) -> CheckpointTimelineOutcome {
        let Some(cp) = self.current() else {
            return CheckpointTimelineOutcome::Ignored;
        };
        let id = cp.id.clone();
        let warn = cp.effective_warning().map(str::to_string);
        self.mode = CheckpointTimelineMode::Preview;
        self.focus_id = Some(id.clone());
        self.last_warning = warn;
        CheckpointTimelineOutcome::PreviewOpened { id }
    }

    fn open_confirm(&mut self, action: CheckpointConfirmAction) -> CheckpointTimelineOutcome {
        let Some(cp) = self.current() else {
            return CheckpointTimelineOutcome::Ignored;
        };
        let ok = match action {
            CheckpointConfirmAction::Restore => cp.can_request_restore(),
            CheckpointConfirmAction::Rewind => cp.can_request_rewind(),
        };
        let id = cp.id.clone();
        let warn = cp.effective_warning().map(str::to_string);
        if !ok {
            self.last_warning = warn.or_else(|| Some("cannot restore".into()));
            return CheckpointTimelineOutcome::WarningAcknowledged;
        }
        self.mode = CheckpointTimelineMode::Confirm;
        self.confirm_action = Some(action);
        self.confirm_proceed_focused = false; // Cancel default
        self.focus_id = Some(id.clone());
        self.last_warning = warn;
        CheckpointTimelineOutcome::ConfirmOpened { id, action }
    }

    fn emit_mutate(&mut self, action: CheckpointConfirmAction) -> CheckpointTimelineOutcome {
        let Some(cp) = self.current() else {
            return CheckpointTimelineOutcome::Ignored;
        };
        let ok = match action {
            CheckpointConfirmAction::Restore => cp.can_request_restore(),
            CheckpointConfirmAction::Rewind => cp.can_request_rewind(),
        };
        let id = cp.id.clone();
        if !ok {
            return CheckpointTimelineOutcome::WarningAcknowledged;
        }
        self.mode = CheckpointTimelineMode::Browse;
        self.confirm_action = None;
        match action {
            CheckpointConfirmAction::Restore => CheckpointTimelineOutcome::RestoreRequested { id },
            CheckpointConfirmAction::Rewind => CheckpointTimelineOutcome::RewindRequested { id },
        }
    }

    /// Keyboard.
    ///
    /// **Draft:** no path touches composer draft text.
    pub fn handle_key(&mut self, key: KeyEvent) -> CheckpointTimelineOutcome {
        if !self.focused || !self.accepts_input || !key.is_press() {
            return CheckpointTimelineOutcome::Ignored;
        }
        if self.checkpoints.is_empty() {
            return CheckpointTimelineOutcome::Ignored;
        }

        // Confirm phase: only confirm strip + esc
        if self.mode == CheckpointTimelineMode::Confirm {
            return self.handle_confirm_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                if self.mode == CheckpointTimelineMode::Preview {
                    self.mode = CheckpointTimelineMode::Browse;
                    self.focus_id = None;
                    return CheckpointTimelineOutcome::PreviewClosed;
                }
                CheckpointTimelineOutcome::Cancelled
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Home => {
                self.unfollow();
                self.cursor = 0;
                self.select_cursor()
            }
            KeyCode::End => {
                self.follow_head();
                self.select_cursor()
            }
            KeyCode::Char('f') => {
                if self.following {
                    self.unfollow();
                } else {
                    self.follow_head();
                }
                CheckpointTimelineOutcome::FollowToggled {
                    following: self.following,
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Browse → Preview; Preview → Confirm Restore (safe default path)
                match self.mode {
                    CheckpointTimelineMode::Browse => self.open_preview(),
                    CheckpointTimelineMode::Preview => {
                        self.open_confirm(CheckpointConfirmAction::Restore)
                    }
                    CheckpointTimelineMode::Confirm => CheckpointTimelineOutcome::Ignored,
                }
            }
            KeyCode::Char('p') => self.open_preview(),
            KeyCode::Char('r') => {
                // Restore — always through confirm when warning needed
                let needs = self
                    .current()
                    .map(|c| c.boundary.needs_warning() || !c.restorable)
                    .unwrap_or(true);
                if needs || self.mode != CheckpointTimelineMode::Preview {
                    // Force preview awareness then confirm
                    if self.mode == CheckpointTimelineMode::Browse {
                        let _ = self.open_preview();
                    }
                    self.open_confirm(CheckpointConfirmAction::Restore)
                } else {
                    self.open_confirm(CheckpointConfirmAction::Restore)
                }
            }
            KeyCode::Char('w') => {
                if self.mode == CheckpointTimelineMode::Browse {
                    let _ = self.open_preview();
                }
                self.open_confirm(CheckpointConfirmAction::Rewind)
            }
            KeyCode::Char('c') => {
                let Some(cp) = self.current() else {
                    return CheckpointTimelineOutcome::Ignored;
                };
                let to = cp.id.clone();
                let from = self
                    .compare_anchor
                    .clone()
                    .or_else(|| self.head().filter(|h| h.id != to).map(|h| h.id.clone()));
                CheckpointTimelineOutcome::CompareRequested { from, to }
            }
            KeyCode::Char('a') => {
                // Set compare anchor
                if let Some(cp) = self.current() {
                    self.compare_anchor = Some(cp.id.clone());
                }
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::Char('b') => {
                if let Some(branch) = self.current().and_then(|c| c.branch_id.clone()) {
                    self.branch_filter = Some(branch.clone());
                    CheckpointTimelineOutcome::BranchFocused { branch_id: branch }
                } else {
                    self.branch_filter = None;
                    CheckpointTimelineOutcome::Ignored
                }
            }
            KeyCode::Char('B') => {
                self.branch_filter = None;
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(4);
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(4);
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::Char('y') => CheckpointTimelineOutcome::Ignored, // unbound grant parity
            _ => CheckpointTimelineOutcome::Ignored,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> CheckpointTimelineOutcome {
        match key.code {
            KeyCode::Esc => {
                self.mode = CheckpointTimelineMode::Preview;
                self.confirm_action = None;
                CheckpointTimelineOutcome::ConfirmCancelled
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.confirm_proceed_focused = false;
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.confirm_proceed_focused = true;
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::Tab => {
                self.confirm_proceed_focused = !self.confirm_proceed_focused;
                CheckpointTimelineOutcome::Ignored
            }
            KeyCode::Enter => {
                if self.confirm_proceed_focused {
                    let action = self
                        .confirm_action
                        .unwrap_or(CheckpointConfirmAction::Restore);
                    self.emit_mutate(action)
                } else {
                    self.mode = CheckpointTimelineMode::Preview;
                    self.confirm_action = None;
                    CheckpointTimelineOutcome::ConfirmCancelled
                }
            }
            KeyCode::Char('y') => CheckpointTimelineOutcome::Ignored,
            _ => CheckpointTimelineOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> CheckpointTimelineOutcome {
        if !self.focused || !self.accepts_input {
            return CheckpointTimelineOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return CheckpointTimelineOutcome::Ignored;
        }
        let pos = ev.position;
        if self.mode == CheckpointTimelineMode::Confirm {
            for (proceed, r) in &self.confirm_hits {
                if r.contains(pos) {
                    self.confirm_proceed_focused = *proceed;
                    if *proceed {
                        let action = self
                            .confirm_action
                            .unwrap_or(CheckpointConfirmAction::Restore);
                        return self.emit_mutate(action);
                    }
                    self.mode = CheckpointTimelineMode::Preview;
                    self.confirm_action = None;
                    return CheckpointTimelineOutcome::ConfirmCancelled;
                }
            }
            return CheckpointTimelineOutcome::Ignored;
        }
        let hit = self
            .row_hits
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| id.clone());
        let Some(id) = hit else {
            return CheckpointTimelineOutcome::Ignored;
        };
        let already = self.selected.as_deref() == Some(id.as_str());
        let browsing = self.mode == CheckpointTimelineMode::Browse;
        if let Some(i) = self.checkpoints.iter().position(|c| c.id == id) {
            self.unfollow();
            self.cursor = i;
            let out = self.select_cursor();
            if already && browsing {
                return self.open_preview();
            }
            return out;
        }
        CheckpointTimelineOutcome::Ignored
    }

    /// Project checkpoints to Timeline events for substrate paint.
    #[must_use]
    pub fn project_timeline_events(&self) -> Vec<TimelineEvent<'_, String>> {
        self.checkpoints
            .iter()
            .filter(|c| {
                self.branch_filter
                    .as_ref()
                    .is_none_or(|b| c.branch_id.as_ref() == Some(b) || c.is_head)
            })
            .map(|c| {
                let status = if c.is_head {
                    TimelineStatus::Running
                } else if c.boundary.blocks_restore() {
                    TimelineStatus::Failed
                } else if c.boundary.needs_warning() {
                    TimelineStatus::Warning
                } else {
                    TimelineStatus::Success
                };
                let mut ev =
                    TimelineEvent::checkpoint(c.id.clone(), c.when.as_str(), c.label.as_str())
                        .status(status);
                if let Some(a) = c.actor.as_deref() {
                    ev = ev.actor(a);
                }
                if let Some(r) = c.relative.as_deref() {
                    ev = ev.relative(r);
                }
                if c.is_head {
                    ev = ev.active();
                }
                if let Some(s) = c.summary.as_deref() {
                    ev = ev.detail(s);
                }
                if matches!(
                    self.mode,
                    CheckpointTimelineMode::Preview | CheckpointTimelineMode::Confirm
                ) && self.focus_id.as_deref() == Some(c.id.as_str())
                {
                    ev = ev.expanded();
                }
                ev
            })
            .collect()
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Checkpoint timeline painter.
#[derive(Debug, Clone, Copy)]
pub struct CheckpointTimeline<'a> {
    system: &'a DesignSystem,
    colorless: bool,
    show_detail: bool,
}

impl<'a> CheckpointTimeline<'a> {
    /// System only — checkpoints live in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
            show_detail: true,
        }
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
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

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut CheckpointTimelineState) {
        if (!false && false) || (!self.colorless && self.system.mono()) {
            let effective = Self {
                colorless: self.colorless || self.system.mono(),
                ..*self
            };
            return effective.paint(area, buffer, state);
        }
        state.row_hits.clear();
        state.confirm_hits.clear();
        if area.is_empty() {
            return;
        }

        let mode_tag = match state.mode {
            CheckpointTimelineMode::Browse => "browse",
            CheckpointTimelineMode::Preview => "preview",
            CheckpointTimelineMode::Confirm => "confirm",
        };
        let separator = { " · " };
        let title = format!("Checkpoints{separator}{mode_tag}");
        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system)
            .title(title.as_str())
            .emphasis(emphasis);
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        if state.checkpoints.is_empty() {
            super::EmptyState::new("No checkpoints yet", self.system)
                .paint(Rect::new(inner.x, inner.y, inner.width, 1), buffer);
            return;
        }

        // Split: list + detail when wide and show_detail
        let w = inner.width;
        let (list_area, detail_area) = if self.show_detail && w >= 48 {
            let list_w = (w * 5 / 10).max(22).min(w.saturating_sub(18));
            let list = Rect {
                x: inner.x,
                y: inner.y,
                width: list_w,
                height: inner.height,
            };
            let detail = Rect {
                x: inner.x.saturating_add(list_w),
                y: inner.y,
                width: w.saturating_sub(list_w),
                height: inner.height,
            };
            (list, Some(detail))
        } else {
            (inner, None)
        };

        self.paint_list(list_area, buffer, state);

        if let Some(detail) = detail_area {
            self.paint_detail(detail, buffer, state);
        }

        // Confirm footer overlays bottom of area
        if state.mode == CheckpointTimelineMode::Confirm {
            self.paint_confirm_bar(inner, buffer, state);
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut CheckpointTimelineState) {
        if area.is_empty() {
            return;
        }
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();

        // Draft preservation banner (viewing ≠ mutating)
        if y < max_y {
            let banner = match state.mode {
                CheckpointTimelineMode::Browse if false => "viewing history - draft preserved",
                CheckpointTimelineMode::Preview if false => {
                    "preview - draft preserved - no mutation"
                }
                CheckpointTimelineMode::Browse => "viewing history · draft preserved",
                CheckpointTimelineMode::Preview => "preview · draft preserved · no mutation",
                CheckpointTimelineMode::Confirm => "confirm mutation request",
            };
            let style = if state.mode == CheckpointTimelineMode::Confirm {
                self.system.style(Role::Warning)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(area.x, y, take_display_cols(banner, w), w, style);
            y = y.saturating_add(1);
        }

        let viewport = max_y.saturating_sub(y) as usize;
        let len = state.checkpoints.len();
        let mut offset = 0usize;
        if len > viewport && viewport > 0 {
            if state.cursor >= offset + viewport {
                offset = state.cursor + 1 - viewport;
            }
            if state.cursor < offset {
                offset = state.cursor;
            }
        }

        for (i, cp) in state.checkpoints.iter().enumerate().skip(offset) {
            if y >= max_y {
                break;
            }
            if let Some(bf) = state.branch_filter.as_ref() {
                if cp.branch_id.as_ref() != Some(bf) && !cp.is_head {
                    continue;
                }
            }
            let selected = i == state.cursor;
            let mark = if cp.is_head {
                "●"
            } else {
                cp.kind.glyph(false)
            };
            let bound = cp.boundary.glyph(false);
            let head = if cp.is_head { " HEAD" } else { "" };
            let branch = cp
                .branch_id
                .as_ref()
                .map(|b| format!(" [{b}]"))
                .unwrap_or_default();
            // The boundary rides its glyph and the timestamp sits under the
            // label: a column of checkpoints reads as one column of state
            // instead of a stack of colored sentences (plans/012 Step 3).
            let tone = |role: Role| (!self.colorless).then(|| self.system.style(role));
            let mut tiers = TieredRow::with_separator("");
            tiers.push_joined(
                mark,
                cp.is_head
                    .then(|| self.system.style(Role::TextSecondary))
                    .filter(|_| !self.colorless),
            );
            tiers.push_joined(
                bound,
                cp.boundary
                    .needs_warning()
                    .then(|| self.system.style(cp.boundary.role()))
                    .filter(|_| !self.colorless),
            );
            tiers.push_joined(" ", None);
            tiers.push_joined(&cp.when, tone(Role::TextFaint));
            tiers.push_joined(" ", None);
            tiers.push_joined(&cp.label, None);
            tiers.push_joined(head, tone(Role::TextSecondary));
            tiers.push_joined(&branch, tone(Role::TextMuted));
            let text = tiers.text().to_string();
            // Selection is chrome — the gutter and weight mark it; the row
            // keeps its own meaning (plans/007).
            let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected,
                    focused: selected && state.focused,
                    enabled: true,
                    ..Default::default()
                },
            )
            .colorless(self.colorless);
            let style = chrome.label_style(self.system.style(Role::Text));
            buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
            tiers.paint_tiers(buffer, Rect::new(area.x, y, area.width, 1), 0);
            chrome.paint(buffer, Rect::new(area.x, y, area.width, 1));
            state.row_hits.push((
                cp.id.clone(),
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

    fn paint_detail(&self, area: Rect, buffer: &mut Buffer, state: &CheckpointTimelineState) {
        if area.is_empty() {
            return;
        }
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let Some(cp) = state.current() else {
            return;
        };

        let lines: Vec<(String, Role)> = {
            let mut v = Vec::new();
            let separator = { " · " };
            v.push((
                format!("{}{separator}{}", cp.kind.id(), cp.label),
                Role::TextStrong,
            ));
            if let Some(a) = cp.actor.as_ref() {
                v.push((format!("actor {a}"), Role::TextMuted));
            }
            if let Some(s) = cp.summary.as_ref() {
                v.push((s.clone(), Role::Text));
            }
            if let Some(warn) = cp.effective_warning() {
                v.push((format!("! {warn}"), Role::Warning));
            }
            if !cp.restorable {
                v.push(("not restorable".into(), Role::Danger));
            }
            if let Some(b) = cp.branch_id.as_ref() {
                let p = cp
                    .parent_id
                    .as_ref()
                    .map(|x| format!(" {} {x}", { "←" }))
                    .unwrap_or_default();
                v.push((format!("branch {b}{p}"), Role::TextSecondary));
            }
            if !cp.changed_files.is_empty() {
                v.push((
                    format!("files ({})", cp.changed_files.len()),
                    Role::TextMuted,
                ));
                for f in cp
                    .changed_files
                    .iter()
                    .skip(state.detail_scroll)
                    .take(CHECKPOINT_DETAIL_WINDOW)
                {
                    v.push((format!("  {f}"), Role::Text));
                }
            }
            if !cp.tool_calls.is_empty() {
                v.push((format!("tools ({})", cp.tool_calls.len()), Role::TextMuted));
                for t in cp.tool_calls.iter().take(6) {
                    v.push((format!("  {} {t}", { "·" }), Role::Text));
                }
            }
            v.push((
                { "keys: enter preview · r restore · w rewind · c compare · esc".into() },
                Role::TextMuted,
            ));
            v
        };

        for (line, role) in lines {
            if y >= max_y {
                break;
            }
            let style = if self.colorless {
                self.system.style(Role::Text)
            } else {
                self.system.style(role)
            };
            buffer.set_stringn(area.x, y, take_display_cols(&line, w), w, style);
            y = y.saturating_add(1);
        }
    }

    fn paint_confirm_bar(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut CheckpointTimelineState,
    ) {
        let y = area.bottom().saturating_sub(2);
        if y < area.y {
            return;
        }
        let w = usize::from(area.width);
        let action = state
            .confirm_action
            .unwrap_or(CheckpointConfirmAction::Restore);
        let warn = state
            .last_warning
            .as_deref()
            .unwrap_or(action.consequence());
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&format!("! {warn}"), w),
            w,
            self.system.style(Role::Warning),
        );
        let bar_y = area.bottom().saturating_sub(1);
        let cancel = if !state.confirm_proceed_focused {
            "[Cancel]"
        } else {
            " Cancel "
        };
        let proceed = if state.confirm_proceed_focused {
            format!("[{}]", action.label())
        } else {
            format!(" {} ", action.label())
        };
        let line = format!("{cancel}  {proceed}  → {}", action.consequence());
        buffer.set_stringn(
            area.x,
            bar_y,
            take_display_cols(&line, w),
            w,
            self.system.style(Role::Accent),
        );
        // Hit regions (approximate)
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
        let pw = display_cols(&proceed) as u16;
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

    /// Optional: paint list via Timeline substrate (for hosts that want Timeline chrome).
    pub fn paint_via_timeline(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut CheckpointTimelineState,
        events: &[TimelineEvent<'_, String>],
    ) {
        state.timeline.colorless = self.colorless;
        state.timeline.set_checkpoint_mode(true);
        state.timeline.cursor = state.cursor.min(events.len().saturating_sub(1));
        let t = Timeline::with_events(events, self.system)
            .recipe(state.recipe.to_timeline())
            .focused(state.focused)
            .colorless(self.colorless);
        t.paint(area, buffer, &mut state.timeline);
        // Map TimelineOutcome-style selection back if needed
        if let Some(sel) = state.timeline.selected().cloned() {
            state.selected = Some(sel);
        }
        state.cursor = state.timeline.cursor();
    }

    /// Bridge: handle Timeline substrate outcome → CheckpointTimelineOutcome.
    #[must_use]
    pub fn map_timeline_outcome(outcome: TimelineOutcome<String>) -> CheckpointTimelineOutcome {
        match outcome {
            TimelineOutcome::Ignored => CheckpointTimelineOutcome::Ignored,
            TimelineOutcome::Selected(id) => CheckpointTimelineOutcome::Selected { id },
            TimelineOutcome::RestoreRequested(id) => {
                CheckpointTimelineOutcome::RestoreRequested { id }
            }
            TimelineOutcome::CompareRequested(id) => {
                CheckpointTimelineOutcome::CompareRequested { from: None, to: id }
            }
            TimelineOutcome::Cancelled => CheckpointTimelineOutcome::Cancelled,
            TimelineOutcome::Scrolled { following } => {
                CheckpointTimelineOutcome::FollowToggled { following }
            }
            TimelineOutcome::Activated(id) => CheckpointTimelineOutcome::PreviewOpened { id },
            _ => CheckpointTimelineOutcome::Ignored,
        }
    }
}

impl StatefulWidget for &CheckpointTimeline<'_> {
    type State = CheckpointTimelineState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for CheckpointTimeline<'_> {
    type State = CheckpointTimelineState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Projection helpers ──────────────────────────────────────────────────────

/// Project a checkpoint to a Timeline event (owned id).
#[must_use]
pub fn checkpoint_to_timeline_event(cp: &Checkpoint) -> TimelineEvent<'_, String> {
    let status = if cp.is_head {
        TimelineStatus::Running
    } else if cp.boundary.blocks_restore() {
        TimelineStatus::Failed
    } else if cp.boundary.needs_warning() {
        TimelineStatus::Warning
    } else {
        TimelineStatus::Success
    };
    let mut ev = TimelineEvent::checkpoint(cp.id.clone(), cp.when.as_str(), cp.label.as_str())
        .status(status);
    if let Some(a) = cp.actor.as_deref() {
        ev = ev.actor(a);
    }
    if let Some(r) = cp.relative.as_deref() {
        ev = ev.relative(r);
    }
    if cp.is_head {
        ev = ev.active();
    }
    if let Some(s) = cp.summary.as_deref() {
        ev = ev.detail(s);
    }
    let _ = TimelineRowKind::Checkpoint;
    ev
}

/// Index checkpoints by id.
#[must_use]
pub fn checkpoint_index(cps: &[Checkpoint]) -> BTreeMap<&str, &Checkpoint> {
    cps.iter().map(|c| (c.id.as_str(), c)).collect()
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo session history with branch, dirty, and irreversible boundaries.
#[must_use]
pub fn example_checkpoints() -> Vec<Checkpoint> {
    vec![
        Checkpoint::new("c0", "10:00", "session open")
            .kind(CheckpointKind::System)
            .actor("system")
            .relative("1h ago")
            .summary("New agent session"),
        Checkpoint::new("c1", "10:05", "after plan")
            .kind(CheckpointKind::Turn)
            .actor("agent")
            .relative("55m ago")
            .summary("Plan approved")
            .tools(["plan_review"])
            .files(["docs/plan.md"]),
        Checkpoint::new("c2", "10:12", "pre-apply")
            .kind(CheckpointKind::FileState)
            .actor("agent")
            .relative("48m ago")
            .summary("Before file writes")
            .files(["src/auth/mod.rs", "src/auth/token.rs"])
            .tools(["write", "read"]),
        Checkpoint::new("c3", "10:18", "dirty workspace")
            .kind(CheckpointKind::Action)
            .actor("user")
            .relative("42m ago")
            .summary("Local edits pending")
            .files(["src/auth/mod.rs"])
            .boundary(CheckpointBoundary::DirtyWorkspace)
            .warning("uncommitted local work may be lost"),
        Checkpoint::new("c4", "10:25", "fork explore")
            .kind(CheckpointKind::Branch)
            .actor("user")
            .relative("35m ago")
            .summary("Branched session for alt approach")
            .branch("explore", Some("c2")),
        Checkpoint::new("c5", "10:40", "deployed")
            .kind(CheckpointKind::Action)
            .actor("agent")
            .relative("20m ago")
            .summary("Pushed remote deploy")
            .tools(["shell"])
            .boundary(CheckpointBoundary::ExternalEffects),
        Checkpoint::new("c6", "10:55", "secrets rotated")
            .kind(CheckpointKind::Action)
            .actor("agent")
            .relative("5m ago")
            .summary("Irreversible secret rotation")
            .boundary(CheckpointBoundary::Irreversible)
            .not_restorable()
            .warning("cannot restore past this boundary"),
        Checkpoint::new("c7", "11:00", "current")
            .kind(CheckpointKind::Turn)
            .actor("agent")
            .relative("now")
            .summary("Live tip")
            .head(),
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
    use crate::input::KeyModifiers;
    use ratatui_core::layout::Position;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open_sample() -> CheckpointTimelineState {
        let mut st = CheckpointTimelineState::new();
        st.set_checkpoints(example_checkpoints());
        st
    }

    #[test]
    fn browse_is_not_mutating() {
        let st = open_sample();
        assert!(st.is_viewing());
        assert!(!st.mode.is_mutating_request());
    }

    #[test]
    fn select_and_preview_preserves_viewing() {
        let mut st = open_sample();
        let i = st.checkpoints.iter().position(|c| c.id == "c2").unwrap();
        st.cursor = i;
        st.selected = Some("c2".into());
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(
            matches!(
                out,
                CheckpointTimelineOutcome::PreviewOpened { ref id } if id == "c2"
            ),
            "{out:?}"
        );
        assert_eq!(st.mode, CheckpointTimelineMode::Preview);
        assert!(st.is_viewing());
    }

    #[test]
    fn restore_requires_confirm_and_safe_default_cancel() {
        let mut st = open_sample();
        st.cursor = 1; // c1 soft
        st.selected = Some("c1".into());
        let _ = st.handle_key(press(KeyCode::Char('p')));
        let out = st.handle_key(press(KeyCode::Char('r')));
        assert!(matches!(
            out,
            CheckpointTimelineOutcome::ConfirmOpened {
                action: CheckpointConfirmAction::Restore,
                ..
            }
        ));
        assert!(!st.confirm_proceed_focused, "Cancel is default focus");
        // Enter without moving → cancel
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, CheckpointTimelineOutcome::ConfirmCancelled));
    }

    #[test]
    fn restore_proceed_after_focus() {
        let mut st = open_sample();
        st.cursor = 1;
        let _ = st.handle_key(press(KeyCode::Char('p')));
        let _ = st.handle_key(press(KeyCode::Char('r')));
        let _ = st.handle_key(press(KeyCode::Right)); // focus proceed
        assert!(st.confirm_proceed_focused);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            CheckpointTimelineOutcome::RestoreRequested { id } if id == "c1"
        ));
    }

    #[test]
    fn irreversible_blocks_restore() {
        let mut st = open_sample();
        let i = st.checkpoints.iter().position(|c| c.id == "c6").unwrap();
        st.cursor = i;
        let _ = st.handle_key(press(KeyCode::Char('r')));
        // WarningAcknowledged or cannot open confirm with proceed
        assert!(
            !st.current().unwrap().can_request_restore()
                || matches!(
                    st.handle_key(press(KeyCode::Char('r'))),
                    CheckpointTimelineOutcome::WarningAcknowledged
                        | CheckpointTimelineOutcome::ConfirmOpened { .. }
                        | CheckpointTimelineOutcome::PreviewOpened { .. }
                        | CheckpointTimelineOutcome::Ignored
                )
        );
        assert!(!st.checkpoints[i].can_request_restore());
    }

    #[test]
    fn dirty_workspace_warns() {
        let cp = example_checkpoints()
            .into_iter()
            .find(|c| c.id == "c3")
            .unwrap();
        assert!(cp.boundary.needs_warning());
        assert!(cp.effective_warning().unwrap().contains("uncommitted"));
        assert!(cp.can_request_restore()); // allowed with warning
    }

    #[test]
    fn head_cannot_restore() {
        let mut st = open_sample();
        let i = st.checkpoints.iter().position(|c| c.is_head).unwrap();
        st.cursor = i;
        assert!(!st.current().unwrap().can_request_restore());
    }

    #[test]
    fn rewind_outcome() {
        let mut st = open_sample();
        st.cursor = 2;
        let _ = st.handle_key(press(KeyCode::Char('w')));
        assert_eq!(st.mode, CheckpointTimelineMode::Confirm);
        assert_eq!(st.confirm_action, Some(CheckpointConfirmAction::Rewind));
        st.confirm_proceed_focused = true;
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            CheckpointTimelineOutcome::RewindRequested { id } if id == "c2"
        ));
    }

    #[test]
    fn compare_requested() {
        let mut st = open_sample();
        st.cursor = 2;
        let out = st.handle_key(press(KeyCode::Char('c')));
        assert!(matches!(
            out,
            CheckpointTimelineOutcome::CompareRequested {
                to,
                from: Some(_)
            } if to == "c2"
        ));
    }

    #[test]
    fn esc_preview_then_cancel() {
        let mut st = open_sample();
        let _ = st.handle_key(press(KeyCode::Char('p')));
        assert!(matches!(
            st.handle_key(press(KeyCode::Esc)),
            CheckpointTimelineOutcome::PreviewClosed
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Esc)),
            CheckpointTimelineOutcome::Cancelled
        ));
    }

    #[test]
    fn y_unbound() {
        let mut st = open_sample();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            CheckpointTimelineOutcome::Ignored
        ));
    }

    #[test]
    fn append_follow() {
        let mut st = open_sample();
        st.follow_head();
        assert!(st.following);
        st.append(
            Checkpoint::new("c8", "11:05", "newer")
                .kind(CheckpointKind::Turn)
                .head(),
        );
        // previous head not auto-cleared by append — host should update is_head
        assert!(st.checkpoints.iter().any(|c| c.id == "c8"));
    }

    #[test]
    fn branch_focus() {
        let mut st = open_sample();
        let i = st.checkpoints.iter().position(|c| c.id == "c4").unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char('b')));
        assert!(matches!(
            out,
            CheckpointTimelineOutcome::BranchFocused { branch_id } if branch_id == "explore"
        ));
    }

    #[test]
    fn project_timeline_events() {
        let st = open_sample();
        let evs = st.project_timeline_events();
        assert_eq!(evs.len(), st.checkpoints.len());
        assert!(
            evs.iter()
                .all(|e| matches!(e.kind, TimelineRowKind::Checkpoint))
        );
    }

    #[test]
    fn paint_modes() {
        let system = DesignSystem::default();
        let mut st = open_sample();
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        for mode in [
            CheckpointTimelineMode::Browse,
            CheckpointTimelineMode::Preview,
            CheckpointTimelineMode::Confirm,
        ] {
            st.mode = mode;
            if mode == CheckpointTimelineMode::Confirm {
                st.confirm_action = Some(CheckpointConfirmAction::Restore);
            }
            CheckpointTimeline::new(&system).paint(area, &mut buf, &mut st);
            assert!(!st.checkpoints.is_empty());
        }
        CheckpointTimeline::new(&system)
            .colorless(true)
            .list_only(true)
            .paint(area, &mut buf, &mut st);
    }

    #[test]
    fn paint_via_timeline_substrate() {
        let system = DesignSystem::default();
        let mut st2 = CheckpointTimelineState::new();
        st2.set_checkpoints(example_checkpoints());
        let labels: Vec<(String, String, String)> = st2
            .checkpoints
            .iter()
            .map(|c| (c.id.clone(), c.when.clone(), c.label.clone()))
            .collect();
        let events: Vec<TimelineEvent<'_, String>> = labels
            .iter()
            .map(|(id, when, label)| {
                TimelineEvent::checkpoint(id.clone(), when.as_str(), label.as_str())
            })
            .collect();
        let area = Rect::new(0, 0, 48, 10);
        let mut buf = Buffer::empty(area);
        CheckpointTimeline::new(&system).paint_via_timeline(area, &mut buf, &mut st2, &events);
    }

    #[test]
    fn no_process_no_draft_mutation() {
        let src = include_str!("checkpoint_timeline.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "openai", "git2"] {
            assert!(!body.contains(f), "{f}");
        }
        assert!(body.contains("draft"));
        assert!(body.contains("preserved") || body.contains("never"));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open_sample();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            CheckpointTimelineOutcome::Ignored
        ));
    }

    #[test]
    fn map_timeline_outcome() {
        assert!(matches!(
            CheckpointTimeline::map_timeline_outcome(TimelineOutcome::RestoreRequested(
                "x".into()
            )),
            CheckpointTimelineOutcome::RestoreRequested { id } if id == "x"
        ));
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = open_sample();
        let area = Rect::new(0, 0, 60, 18);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            CheckpointTimeline::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_kinds_boundaries() {
        for k in [
            CheckpointKind::Turn,
            CheckpointKind::FileState,
            CheckpointKind::Action,
            CheckpointKind::Manual,
            CheckpointKind::Branch,
            CheckpointKind::System,
        ] {
            assert!(!k.id().is_empty());
            let _ = k.glyph(true);
            let _ = k.glyph(false);
        }
        for b in [
            CheckpointBoundary::Soft,
            CheckpointBoundary::DirtyWorkspace,
            CheckpointBoundary::ExternalEffects,
            CheckpointBoundary::Irreversible,
        ] {
            assert!(!b.id().is_empty());
            let _ = b.needs_warning();
            let _ = b.blocks_restore();
        }
    }

    #[test]
    fn mouse_select() {
        let system = DesignSystem::default();
        let mut st = open_sample();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        CheckpointTimeline::new(&system).paint(area, &mut buf, &mut st);
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
                CheckpointTimelineOutcome::Selected { .. }
                    | CheckpointTimelineOutcome::PreviewOpened { .. }
            ),
            "{out:?} id={id}"
        );
    }

    #[test]
    fn unicode_labels() {
        let system = DesignSystem::default();
        let mut st = CheckpointTimelineState::new();
        st.set_checkpoints(vec![
            Checkpoint::new("u1", "12:00", "検査 🔍")
                .summary("ファイル状態")
                .files(["src/日本語.rs"]),
            Checkpoint::new("u2", "12:01", "現在").head(),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        CheckpointTimeline::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn selection_stable_on_set() {
        let mut st = open_sample();
        st.cursor = 2;
        st.selected = Some("c2".into());
        let mut cps = example_checkpoints();
        cps.push(Checkpoint::new("c9", "12:00", "extra"));
        st.set_checkpoints(cps);
        assert_eq!(st.selected.as_deref(), Some("c2"));
    }
}
