// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **DiffReview** — interactive review behavior on top of [`super::DiffView`].
//!
//! **Mission.** File tree, hunk and line-range selection, comments,
//! approve/reject/apply decisions, staging-like actions, external editor, and
//! review summary. Application version-control policy stays **consumer-owned**;
//! TermRock owns reusable review state (selection, decisions, comments, undo)
//! and paints safe destructive language. Comments and selection survive mode
//! and resize via stable ids.
//!
//! **Use cases.** Git review, plan-change diffs, AI-agent code review.
//!
//! **vs [`super::DiffView`].** DiffView is read-only paint + nav. DiffReview
//! adds decision/selection/comment chrome and request outcomes (never runs git).
//! **vs [`super::PlanReview`].** PlanReview is plan-document approval with comments;
//! DiffReview is patch-oriented.
//!
//! Research: GitHub reviews, lazygit staging, Grok Build plan review, agent
//! diff approval.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::UiIntent,
    style::{DesignSystem, ListRowVisualState, Role},
    text::take_display_cols,
    widgets::diff::{DiffFile, DiffHunk, DiffLine, DiffView, DiffViewOutcome, DiffViewState},
};

/// Maximum undo depth for review ops (not VCS ops).
pub const DIFF_REVIEW_UNDO_LIMIT: usize = 64;

/// Focusable review region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DiffReviewRegion {
    /// File tree strip.
    FileTree,
    /// Diff body (default).
    #[default]
    Diff,
    /// Comment list / draft.
    Comments,
    /// Summary strip (activate for bulk).
    Summary,
}

impl DiffReviewRegion {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::FileTree => "file_tree",
            Self::Diff => "diff",
            Self::Comments => "comments",
            Self::Summary => "summary",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::FileTree => Self::Diff,
            Self::Diff => Self::Comments,
            Self::Comments => Self::Summary,
            Self::Summary => Self::FileTree,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::FileTree => Self::Summary,
            Self::Diff => Self::FileTree,
            Self::Comments => Self::Diff,
            Self::Summary => Self::Comments,
        }
    }
}

/// Decision on a review unit (host maps to VCS / plan / agent policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DiffDecision {
    /// No decision yet.
    #[default]
    Pending,
    /// Approved / LGTM.
    Approved,
    /// Rejected / request changes.
    Rejected,
    /// Staged-like (index intent).
    Staged,
    /// Explicitly unstaged / cleared from stage set.
    Unstaged,
    /// Applied (plan/agent apply intent).
    Applied,
    /// Skipped / deferred.
    Skipped,
}

impl DiffDecision {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::Applied => "applied",
            Self::Skipped => "skipped",
        }
    }

    /// Compact mark (ascii uses letters).
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Pending => " ",
            Self::Approved => "✓",
            Self::Rejected => "✗",
            Self::Staged => "●",
            Self::Unstaged => "○",
            Self::Applied => "▶",
            Self::Skipped => "·",
        }
    }

    /// Safe verb phrase for confirm banners (never implies VCS success).
    #[must_use]
    pub const fn safe_verb(self) -> &'static str {
        match self {
            Self::Pending => "clear decision on",
            Self::Approved => "mark approved",
            Self::Rejected => "mark rejected",
            Self::Staged => "request stage of",
            Self::Unstaged => "request unstage of",
            Self::Applied => "request apply of",
            Self::Skipped => "skip",
        }
    }

    /// Whether this decision is considered destructive for bulk confirm.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Rejected | Self::Applied)
    }
}

/// Kind of review unit for decisions / selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DiffReviewUnitKind {
    /// Whole file.
    File,
    /// Hunk.
    Hunk,
    /// Line range (inclusive ids stored as start..end line ids).
    LineRange,
}

impl DiffReviewUnitKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Hunk => "hunk",
            Self::LineRange => "line_range",
        }
    }
}

/// Stable unit key (`file:…`, `hunk:…`, `range:…`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiffReviewUnit {
    /// Kind.
    pub kind: DiffReviewUnitKind,
    /// Primary id (file id, hunk id, or `start_line_id`).
    pub id: String,
    /// Optional end line id for ranges.
    pub end_id: Option<String>,
}

impl DiffReviewUnit {
    /// File unit.
    #[must_use]
    pub fn file(id: impl Into<String>) -> Self {
        Self {
            kind: DiffReviewUnitKind::File,
            id: id.into(),
            end_id: None,
        }
    }

    /// Hunk unit.
    #[must_use]
    pub fn hunk(id: impl Into<String>) -> Self {
        Self {
            kind: DiffReviewUnitKind::Hunk,
            id: id.into(),
            end_id: None,
        }
    }

    /// Line range unit.
    #[must_use]
    pub fn line_range(start_id: impl Into<String>, end_id: impl Into<String>) -> Self {
        Self {
            kind: DiffReviewUnitKind::LineRange,
            id: start_id.into(),
            end_id: Some(end_id.into()),
        }
    }

    /// Display key.
    #[must_use]
    pub fn key(&self) -> String {
        match self.kind {
            DiffReviewUnitKind::File => format!("file:{}", self.id),
            DiffReviewUnitKind::Hunk => format!("hunk:{}", self.id),
            DiffReviewUnitKind::LineRange => {
                format!(
                    "range:{}..{}",
                    self.id,
                    self.end_id.as_deref().unwrap_or(&self.id)
                )
            }
        }
    }
}

/// Anchor for a comment (survives mode/resize via stable ids).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiffCommentAnchor {
    /// File-level.
    File {
        /// File id.
        file_id: String,
    },
    /// Hunk-level.
    Hunk {
        /// Hunk id.
        hunk_id: String,
    },
    /// Line-level (prefer line id).
    Line {
        /// Stable line id from [`DiffLine::id`].
        line_id: String,
    },
}

impl DiffCommentAnchor {
    /// Stable key for maps.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::File { file_id } => format!("file:{file_id}"),
            Self::Hunk { hunk_id } => format!("hunk:{hunk_id}"),
            Self::Line { line_id } => format!("line:{line_id}"),
        }
    }
}

/// One review comment (host owns persistence; TermRock owns session chrome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffComment {
    /// Stable id.
    pub id: String,
    /// Anchor.
    pub anchor: DiffCommentAnchor,
    /// Body text.
    pub body: String,
    /// Resolved thread.
    pub resolved: bool,
}

impl DiffComment {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, anchor: DiffCommentAnchor, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            anchor,
            body: body.into(),
            resolved: false,
        }
    }
}

/// File-tree row projection (host-owned paths and counts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffReviewFileRow<'a> {
    /// Stable file id (matches [`DiffLine::file_id`] / [`DiffFile::id`]).
    pub id: &'a str,
    /// Display path.
    pub path: &'a str,
    /// Added line count (display).
    pub added: u32,
    /// Removed line count (display).
    pub removed: u32,
}

impl<'a> DiffReviewFileRow<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, path: &'a str) -> Self {
        Self {
            id,
            path,
            added: 0,
            removed: 0,
        }
    }

    /// Stats.
    #[must_use]
    pub const fn stats(mut self, added: u32, removed: u32) -> Self {
        self.added = added;
        self.removed = removed;
        self
    }
}

/// Aggregate summary (derived from state + host file rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffReviewSummary {
    /// File rows.
    pub files: usize,
    /// Hunks in projection.
    pub hunks: usize,
    /// Units approved.
    pub approved: usize,
    /// Units rejected.
    pub rejected: usize,
    /// Units staged.
    pub staged: usize,
    /// Units applied.
    pub applied: usize,
    /// Pending units among decided keys (not total hunks).
    pub pending_decisions: usize,
    /// Comment count.
    pub comments: usize,
    /// Unresolved comments.
    pub unresolved_comments: usize,
    /// Selected units.
    pub selected: usize,
}

/// Pending destructive confirm (safe language only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffDestructiveConfirm {
    /// Decision requested.
    pub decision: DiffDecision,
    /// Human-safe summary (e.g. "3 hunks").
    pub subject: String,
    /// Unit keys affected.
    pub units: Vec<DiffReviewUnit>,
}

/// Undoable review op (session-local; not VCS).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReviewOp {
    Decisions {
        before: BTreeMap<String, DiffDecision>,
        after: BTreeMap<String, DiffDecision>,
    },
    Selection {
        before_hunks: BTreeSet<String>,
        after_hunks: BTreeSet<String>,
        before_lines: BTreeSet<String>,
        after_lines: BTreeSet<String>,
        before_files: BTreeSet<String>,
        after_files: BTreeSet<String>,
    },
    CommentAdd(DiffComment),
    CommentResolve {
        id: String,
        before: bool,
        after: bool,
    },
}

/// Outcomes — host executes VCS / editor / apply policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffReviewOutcome {
    /// No change.
    Ignored,
    /// Diff view scrolled.
    Scrolled {
        /// Offset.
        offset: u16,
    },
    /// Hunk cursor moved.
    HunkCursorMoved {
        /// Index.
        index: usize,
    },
    /// Line cursor moved.
    CursorMoved {
        /// Index.
        index: usize,
    },
    /// File tree cursor moved.
    FileCursorMoved {
        /// File id.
        id: String,
    },
    /// Focus region changed.
    FocusRegion(DiffReviewRegion),
    /// Mode preference changed (from DiffView).
    ToggleMode,
    /// Search changed.
    SearchChanged(String),
    /// Fold toggled.
    FoldToggled {
        /// Id.
        id: String,
        /// Folded.
        folded: bool,
    },
    /// Selection changed.
    SelectionChanged {
        /// Selected unit keys.
        keys: Vec<String>,
    },
    /// Decision set on unit(s) (host applies policy).
    DecisionSet {
        /// Units.
        units: Vec<DiffReviewUnit>,
        /// Decision.
        decision: DiffDecision,
    },
    /// Decision cleared to pending.
    DecisionCleared {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Bulk approve request (after confirm if multi).
    ApproveRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Bulk reject request.
    RejectRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Stage-like request.
    StageRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Unstage-like request.
    UnstageRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Apply request (plan/agent).
    ApplyRequested {
        /// Units.
        units: Vec<DiffReviewUnit>,
    },
    /// Open external editor (host).
    ExternalEditorRequested {
        /// Path hint.
        path: String,
        /// Optional line number.
        line: Option<u32>,
    },
    /// Comment draft changed.
    CommentDraftChanged(String),
    /// Comment committed (host may persist).
    CommentAdded(DiffComment),
    /// Comment resolve toggled.
    CommentResolved {
        /// Id.
        id: String,
        /// Resolved after.
        resolved: bool,
    },
    /// Destructive confirm shown / updated.
    ConfirmRequired(DiffDestructiveConfirm),
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Undo applied.
    Undone,
    /// Redo applied.
    Redone,
    /// Cancelled (search/draft/confirm).
    Cancelled,
    /// Summary focus / bulk activate.
    SummaryActivated(DiffReviewSummary),
    /// Current hunk activated — host may stage/open.
    HunkActivated {
        /// Index.
        index: usize,
    },
}

/// Interactive review state on top of [`DiffViewState`].
///
/// Selection, decisions, and comments key by **stable ids** so mode/resize
/// remaps of viewport do not drop review chrome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffReviewState {
    /// Embedded DiffView state.
    pub view: DiffViewState,
    /// Focused region.
    pub region: DiffReviewRegion,
    /// File tree cursor index.
    pub file_cursor: usize,
    /// Selected file ids.
    selected_files: BTreeSet<String>,
    /// Selected hunk ids.
    selected_hunks: BTreeSet<String>,
    /// Selected line ids.
    selected_lines: BTreeSet<String>,
    /// Line-range anchor (line id) when building a range.
    range_anchor: Option<String>,
    /// Decisions by unit key.
    decisions: BTreeMap<String, DiffDecision>,
    /// Session comments.
    comments: Vec<DiffComment>,
    /// Draft comment body (when composing).
    pub comment_draft: Option<String>,
    /// Pending destructive confirm.
    pub pending_confirm: Option<DiffDestructiveConfirm>,
    /// Next comment id counter.
    comment_seq: u64,
    undo: VecDeque<ReviewOp>,
    redo: VecDeque<ReviewOp>,
    accepts_input: bool,
    /// Layout cache.
    origin: (u16, u16),
    tree_width: u16,
    body_area: Rect,
    tree_area: Rect,
    summary_area: Rect,
    comment_area: Rect,
    /// Painted file-tree hit rows.
    pub file_regions: Vec<(String, Rect)>,
    /// Colorless preference.
    pub colorless: bool,
}

impl Default for DiffReviewState {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffReviewState {
    /// Fresh review.
    #[must_use]
    pub fn new() -> Self {
        Self {
            view: DiffViewState::new(),
            region: DiffReviewRegion::Diff,
            file_cursor: 0,
            selected_files: BTreeSet::new(),
            selected_hunks: BTreeSet::new(),
            selected_lines: BTreeSet::new(),
            range_anchor: None,
            decisions: BTreeMap::new(),
            comments: Vec::new(),
            comment_draft: None,
            pending_confirm: None,
            comment_seq: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            accepts_input: true,
            origin: (0, 0),
            tree_width: 0,
            body_area: Rect::default(),
            tree_area: Rect::default(),
            summary_area: Rect::default(),
            comment_area: Rect::default(),
            file_regions: Vec::new(),
            colorless: false,
        }
    }

    /// Host input gate (also gates DiffView).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
        self.view.set_accepts_input(accepts);
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Vertical offset.
    #[must_use]
    pub fn offset_y(&self) -> u16 {
        self.view.offset()
    }
    /// Prefers split when wide.
    #[must_use]
    pub const fn prefers_split(&self) -> bool {
        self.view.prefers_split()
    }

    /// Decisions map (unit key → decision).
    #[must_use]
    pub fn decisions(&self) -> &BTreeMap<String, DiffDecision> {
        &self.decisions
    }

    /// Comments.
    #[must_use]
    pub fn comments(&self) -> &[DiffComment] {
        &self.comments
    }

    /// Selected hunk ids.
    #[must_use]
    pub fn selected_hunks(&self) -> &BTreeSet<String> {
        &self.selected_hunks
    }

    /// Derive summary.
    #[must_use]
    pub fn summary(&self, files: usize, hunks: usize) -> DiffReviewSummary {
        let mut s = DiffReviewSummary {
            files,
            hunks,
            selected: self.selected_files.len()
                + self.selected_hunks.len()
                + self.selected_lines.len(),
            comments: self.comments.len(),
            unresolved_comments: self.comments.iter().filter(|c| !c.resolved).count(),
            ..DiffReviewSummary::default()
        };
        for d in self.decisions.values() {
            match d {
                DiffDecision::Approved => s.approved += 1,
                DiffDecision::Rejected => s.rejected += 1,
                DiffDecision::Staged => s.staged += 1,
                DiffDecision::Applied => s.applied += 1,
                DiffDecision::Pending => s.pending_decisions += 1,
                _ => {}
            }
        }
        s
    }
    /// Set decision without undo (host hydrate).
    pub fn hydrate_decision(&mut self, unit: DiffReviewUnit, decision: DiffDecision) {
        self.decisions.insert(unit.key(), decision);
    }

    fn push_undo(&mut self, op: ReviewOp) {
        if self.undo.len() >= DIFF_REVIEW_UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(op);
        self.redo.clear();
    }

    fn apply_op(&mut self, op: &ReviewOp, forward: bool) {
        match op {
            ReviewOp::Decisions { before, after } => {
                let map = if forward { after } else { before };
                self.decisions.clone_from(map);
            }
            ReviewOp::Selection {
                before_hunks,
                after_hunks,
                before_lines,
                after_lines,
                before_files,
                after_files,
            } => {
                if forward {
                    self.selected_hunks.clone_from(after_hunks);
                    self.selected_lines.clone_from(after_lines);
                    self.selected_files.clone_from(after_files);
                } else {
                    self.selected_hunks.clone_from(before_hunks);
                    self.selected_lines.clone_from(before_lines);
                    self.selected_files.clone_from(before_files);
                }
            }
            ReviewOp::CommentAdd(c) => {
                if forward {
                    if !self.comments.iter().any(|x| x.id == c.id) {
                        self.comments.push(c.clone());
                    }
                } else {
                    self.comments.retain(|x| x.id != c.id);
                }
            }
            ReviewOp::CommentResolve { id, before, after } => {
                let val = if forward { *after } else { *before };
                if let Some(c) = self.comments.iter_mut().find(|c| c.id == *id) {
                    c.resolved = val;
                }
            }
        }
    }

    /// Undo last review op.
    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo.pop_back() else {
            return false;
        };
        self.apply_op(&op, false);
        self.redo.push_back(op);
        true
    }

    /// Redo.
    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo.pop_back() else {
            return false;
        };
        self.apply_op(&op, true);
        self.undo.push_back(op);
        true
    }

    fn target_units(
        &self,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
        files: &[DiffReviewFileRow<'_>],
    ) -> Vec<DiffReviewUnit> {
        let mut units = Vec::new();
        for id in &self.selected_files {
            units.push(DiffReviewUnit::file(id.clone()));
        }
        for id in &self.selected_hunks {
            units.push(DiffReviewUnit::hunk(id.clone()));
        }
        if let (Some(a), Some(b)) = (
            self.range_anchor.as_ref(),
            self.selected_lines.iter().last(),
        ) {
            if self.selected_lines.len() > 1 {
                units.push(DiffReviewUnit::line_range(a.clone(), b.clone()));
            }
        }
        for id in &self.selected_lines {
            if self.range_anchor.is_none() || self.selected_lines.len() == 1 {
                // single lines as line-range degenerate
                units.push(DiffReviewUnit::line_range(id.clone(), id.clone()));
            }
        }
        if !units.is_empty() {
            return units;
        }
        // With nothing multi-selected the target is whatever the focused
        // region is pointing at. Preferring the hunk cursor unconditionally
        // meant `a` in the file tree approved a hunk somewhere else in the
        // diff — the operator accepted a row they were not looking at
        // (GAP-DF-1).
        if self.region == DiffReviewRegion::FileTree {
            if let Some(f) = files.get(self.file_cursor) {
                return vec![DiffReviewUnit::file(f.id.to_string())];
            }
        }
        // Fallback: current hunk
        if let Some(h) = hunks.get(self.view.hunk_cursor) {
            return vec![DiffReviewUnit::hunk(h.id.clone())];
        }
        // Fallback: current file
        if let Some(f) = files.get(self.file_cursor) {
            return vec![DiffReviewUnit::file(f.id.to_string())];
        }
        // Fallback: line under cursor
        if let Some(l) = lines.get(self.view.cursor) {
            return vec![DiffReviewUnit::line_range(
                l.id.to_string(),
                l.id.to_string(),
            )];
        }
        units
    }

    fn set_decisions_inner(
        &mut self,
        units: &[DiffReviewUnit],
        decision: DiffDecision,
    ) -> DiffReviewOutcome {
        let before = self.decisions.clone();
        for u in units {
            if decision == DiffDecision::Pending {
                self.decisions.remove(&u.key());
            } else {
                self.decisions.insert(u.key(), decision);
            }
        }
        let after = self.decisions.clone();
        self.push_undo(ReviewOp::Decisions { before, after });
        if decision == DiffDecision::Pending {
            DiffReviewOutcome::DecisionCleared {
                units: units.to_vec(),
            }
        } else {
            DiffReviewOutcome::DecisionSet {
                units: units.to_vec(),
                decision,
            }
        }
    }

    fn request_decision(
        &mut self,
        decision: DiffDecision,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
        files: &[DiffReviewFileRow<'_>],
    ) -> DiffReviewOutcome {
        let units = self.target_units(lines, hunks, files);
        if units.is_empty() {
            return DiffReviewOutcome::Ignored;
        }
        let multi = units.len() > 1
            || self.selected_hunks.len() + self.selected_files.len() + self.selected_lines.len()
                > 1;
        if decision.is_destructive() && multi {
            let subject = format!("{} units", units.len());
            let confirm = DiffDestructiveConfirm {
                decision,
                subject,
                units: units.clone(),
            };
            self.pending_confirm = Some(confirm.clone());
            return DiffReviewOutcome::ConfirmRequired(confirm);
        }
        let set = self.set_decisions_inner(&units, decision);
        // Also emit request-style outcome for host policy pipelines.
        match (decision, set) {
            (DiffDecision::Approved, _) => DiffReviewOutcome::ApproveRequested { units },
            (DiffDecision::Rejected, _) => DiffReviewOutcome::RejectRequested { units },
            (DiffDecision::Staged, _) => DiffReviewOutcome::StageRequested { units },
            (DiffDecision::Unstaged, _) => DiffReviewOutcome::UnstageRequested { units },
            (DiffDecision::Applied, _) => DiffReviewOutcome::ApplyRequested { units },
            (_, other) => other,
        }
    }

    fn confirm_pending(&mut self) -> DiffReviewOutcome {
        let Some(confirm) = self.pending_confirm.take() else {
            return DiffReviewOutcome::Ignored;
        };
        let units = confirm.units;
        let decision = confirm.decision;
        let _ = self.set_decisions_inner(&units, decision);
        match decision {
            DiffDecision::Approved => DiffReviewOutcome::ApproveRequested { units },
            DiffDecision::Rejected => DiffReviewOutcome::RejectRequested { units },
            DiffDecision::Staged => DiffReviewOutcome::StageRequested { units },
            DiffDecision::Unstaged => DiffReviewOutcome::UnstageRequested { units },
            DiffDecision::Applied => DiffReviewOutcome::ApplyRequested { units },
            _ => DiffReviewOutcome::DecisionSet { units, decision },
        }
    }

    fn toggle_select_hunk(&mut self, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        let Some(h) = hunks.get(self.view.hunk_cursor) else {
            return DiffReviewOutcome::Ignored;
        };
        let before_h = self.selected_hunks.clone();
        let before_l = self.selected_lines.clone();
        let before_f = self.selected_files.clone();
        if !self.selected_hunks.remove(&h.id) {
            self.selected_hunks.insert(h.id.clone());
        }
        self.push_undo(ReviewOp::Selection {
            before_hunks: before_h,
            after_hunks: self.selected_hunks.clone(),
            before_lines: before_l,
            after_lines: self.selected_lines.clone(),
            before_files: before_f,
            after_files: self.selected_files.clone(),
        });
        DiffReviewOutcome::SelectionChanged {
            keys: self
                .selected_hunks
                .iter()
                .map(|id| format!("hunk:{id}"))
                .collect(),
        }
    }

    fn toggle_select_line(&mut self, lines: &[DiffLine<'_>]) -> DiffReviewOutcome {
        let Some(l) = lines.get(self.view.cursor) else {
            return DiffReviewOutcome::Ignored;
        };
        let before_h = self.selected_hunks.clone();
        let before_l = self.selected_lines.clone();
        let before_f = self.selected_files.clone();
        let id = l.id.to_string();
        if !self.selected_lines.remove(&id) {
            self.selected_lines.insert(id.clone());
            if self.range_anchor.is_none() {
                self.range_anchor = Some(id);
            }
        } else if self.range_anchor.as_deref() == Some(l.id) {
            self.range_anchor = None;
        }
        self.push_undo(ReviewOp::Selection {
            before_hunks: before_h,
            after_hunks: self.selected_hunks.clone(),
            before_lines: before_l,
            after_lines: self.selected_lines.clone(),
            before_files: before_f,
            after_files: self.selected_files.clone(),
        });
        DiffReviewOutcome::SelectionChanged {
            keys: self
                .selected_lines
                .iter()
                .map(|id| format!("line:{id}"))
                .collect(),
        }
    }

    fn map_view(&self, out: DiffViewOutcome) -> DiffReviewOutcome {
        match out {
            DiffViewOutcome::Ignored => DiffReviewOutcome::Ignored,
            DiffViewOutcome::Scrolled { offset } => DiffReviewOutcome::Scrolled { offset },
            DiffViewOutcome::CursorMoved { index } => DiffReviewOutcome::CursorMoved { index },
            DiffViewOutcome::HunkCursorMoved { index } => {
                DiffReviewOutcome::HunkCursorMoved { index }
            }
            DiffViewOutcome::HunkActivated { index } => DiffReviewOutcome::HunkActivated { index },
            DiffViewOutcome::FileNavigated { id } => DiffReviewOutcome::FileCursorMoved { id },
            DiffViewOutcome::ModeChanged(_) => DiffReviewOutcome::ToggleMode,
            DiffViewOutcome::SearchChanged(q) => DiffReviewOutcome::SearchChanged(q),
            DiffViewOutcome::FoldToggled { id, folded } => {
                DiffReviewOutcome::FoldToggled { id, folded }
            }
            DiffViewOutcome::Cancelled => DiffReviewOutcome::Cancelled,
        }
    }

    /// Keys with full projection (preferred).
    pub fn handle_key_lines(
        &mut self,
        key: KeyEvent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
        files: &[DiffReviewFileRow<'_>],
    ) -> DiffReviewOutcome {
        if !self.accepts_input || key.is_release() {
            return DiffReviewOutcome::Ignored;
        }
        let is_press = key.is_press();

        // Confirm banner
        if self.pending_confirm.is_some() && is_press {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => return self.confirm_pending(),
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.pending_confirm = None;
                    return DiffReviewOutcome::ConfirmCancelled;
                }
                _ => return DiffReviewOutcome::Ignored,
            }
        }

        // Comment draft
        if let Some(draft) = self.comment_draft.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.comment_draft = None;
                    return DiffReviewOutcome::Cancelled;
                }
                KeyCode::Enter => {
                    let body = draft.trim().to_string();
                    self.comment_draft = None;
                    if body.is_empty() {
                        return DiffReviewOutcome::Cancelled;
                    }
                    return self.commit_comment(body, lines, hunks);
                }
                KeyCode::Backspace => {
                    draft.pop();
                    return DiffReviewOutcome::CommentDraftChanged(draft.clone());
                }
                KeyCode::Char(c) if !c.is_control() => {
                    draft.push(c);
                    return DiffReviewOutcome::CommentDraftChanged(draft.clone());
                }
                _ => {}
            }
        }

        if is_press {
            // Region focus
            if key.code == KeyCode::Tab {
                self.region = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.region.prev()
                } else {
                    self.region.next()
                };
                // Skip FileTree if no files
                if self.region == DiffReviewRegion::FileTree && files.is_empty() {
                    self.region = if key.modifiers.contains(KeyModifiers::SHIFT) {
                        DiffReviewRegion::Summary
                    } else {
                        DiffReviewRegion::Diff
                    };
                }
                return DiffReviewOutcome::FocusRegion(self.region);
            }

            // Global review chords (work in Diff region primarily)
            match key.code {
                KeyCode::Char('u') if key.modifiers.is_empty() => {
                    if self.undo() {
                        return DiffReviewOutcome::Undone;
                    }
                    return DiffReviewOutcome::Ignored;
                }
                KeyCode::Char('U') => {
                    if self.redo() {
                        return DiffReviewOutcome::Redone;
                    }
                    return DiffReviewOutcome::Ignored;
                }
                KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.undo() {
                        return DiffReviewOutcome::Undone;
                    }
                    return DiffReviewOutcome::Ignored;
                }
                KeyCode::Char('a' | 'A') if key.modifiers.is_empty() => {
                    return self.request_decision(DiffDecision::Approved, lines, hunks, files);
                }
                KeyCode::Char('r' | 'R') if key.modifiers.is_empty() => {
                    return self.request_decision(DiffDecision::Rejected, lines, hunks, files);
                }
                KeyCode::Char('t') if key.modifiers.is_empty() => {
                    return self.request_decision(DiffDecision::Staged, lines, hunks, files);
                }
                KeyCode::Char('T') => {
                    return self.request_decision(DiffDecision::Unstaged, lines, hunks, files);
                }
                KeyCode::Char('x' | 'X') if key.modifiers.is_empty() => {
                    return self.request_decision(DiffDecision::Applied, lines, hunks, files);
                }
                KeyCode::Char('c') if key.modifiers.is_empty() && self.comment_draft.is_none() => {
                    self.comment_draft = Some(String::new());
                    self.region = DiffReviewRegion::Comments;
                    return DiffReviewOutcome::CommentDraftChanged(String::new());
                }
                KeyCode::Char('e' | 'E') if key.modifiers.is_empty() => {
                    return self.external_editor(lines, files);
                }
                KeyCode::Char(' ')
                    if key.modifiers.is_empty() && self.region == DiffReviewRegion::Diff =>
                {
                    // Multi-select hunk (review); Shift+Space selects line
                    return self.toggle_select_hunk(hunks);
                }
                KeyCode::Char(' ')
                    if key.modifiers.contains(KeyModifiers::SHIFT)
                        && self.region == DiffReviewRegion::Diff =>
                {
                    return self.toggle_select_line(lines);
                }
                KeyCode::Char('v') if key.modifiers.is_empty() => {
                    // View mode cycle without clobbering stage chord `t`
                    let out = self.view.handle_key(
                        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
                        lines,
                        hunks,
                    );
                    return self.map_view(out);
                }
                _ => {}
            }
        }

        // Region-specific navigation
        match self.region {
            DiffReviewRegion::FileTree => {
                return self.handle_file_tree_key(key, files, hunks, lines);
            }
            DiffReviewRegion::Comments => {
                if is_press && matches!(key.code, KeyCode::Char('r' | 'R')) {
                    // resolve focused comment (first unresolved or last)
                    if let Some(c) = self
                        .comments
                        .iter()
                        .find(|c| !c.resolved)
                        .or_else(|| self.comments.last())
                    {
                        let id = c.id.clone();
                        let before = c.resolved;
                        let after = !before;
                        if let Some(cm) = self.comments.iter_mut().find(|x| x.id == id) {
                            cm.resolved = after;
                        }
                        self.push_undo(ReviewOp::CommentResolve {
                            id: id.clone(),
                            before,
                            after,
                        });
                        return DiffReviewOutcome::CommentResolved {
                            id,
                            resolved: after,
                        };
                    }
                }
            }
            DiffReviewRegion::Summary if is_press && matches!(key.code, KeyCode::Enter) => {
                let sum = self.summary(files.len(), hunks.len());
                return DiffReviewOutcome::SummaryActivated(sum);
            }
            _ => {}
        }

        // DiffView path
        let out = self.view.handle_key(key, lines, hunks);
        self.map_view(out)
    }

    /// Keys without files (hunks only).
    pub fn handle_key(&mut self, key: KeyEvent, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        self.handle_key_lines(key, &[], hunks, &[])
    }

    /// Intent routing.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffReviewOutcome {
        if !self.accepts_input {
            return DiffReviewOutcome::Ignored;
        }
        let refs: Vec<&DiffLine<'_>> = lines.iter().collect();
        let out = self.view.handle_intent(intent, &refs, hunks);
        self.map_view(out)
    }

    fn handle_file_tree_key(
        &mut self,
        key: KeyEvent,
        files: &[DiffReviewFileRow<'_>],
        hunks: &[DiffHunk],
        lines: &[DiffLine<'_>],
    ) -> DiffReviewOutcome {
        if files.is_empty() || !key.is_press() {
            return DiffReviewOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j' | 'J') => {
                self.file_cursor = (self.file_cursor + 1).min(files.len() - 1);
                DiffReviewOutcome::FileCursorMoved {
                    id: files[self.file_cursor].id.to_string(),
                }
            }
            KeyCode::Up | KeyCode::Char('k' | 'K') => {
                self.file_cursor = self.file_cursor.saturating_sub(1);
                DiffReviewOutcome::FileCursorMoved {
                    id: files[self.file_cursor].id.to_string(),
                }
            }
            KeyCode::Enter => {
                let id = files[self.file_cursor].id;
                // Jump first hunk for file
                if let Some((i, _)) = hunks
                    .iter()
                    .enumerate()
                    .find(|(_, h)| h.file_id.as_deref() == Some(id))
                {
                    self.view.hunk_cursor = i;
                    self.region = DiffReviewRegion::Diff;
                    return DiffReviewOutcome::HunkCursorMoved { index: i };
                }
                if let Some((i, _)) = lines
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.file_id == Some(id))
                {
                    self.view.cursor = i;
                    self.region = DiffReviewRegion::Diff;
                    return DiffReviewOutcome::CursorMoved { index: i };
                }
                DiffReviewOutcome::FileCursorMoved { id: id.to_string() }
            }
            KeyCode::Char(' ') => {
                let id = files[self.file_cursor].id.to_string();
                let before_f = self.selected_files.clone();
                let before_h = self.selected_hunks.clone();
                let before_l = self.selected_lines.clone();
                if !self.selected_files.remove(&id) {
                    self.selected_files.insert(id);
                }
                self.push_undo(ReviewOp::Selection {
                    before_hunks: before_h,
                    after_hunks: self.selected_hunks.clone(),
                    before_lines: before_l,
                    after_lines: self.selected_lines.clone(),
                    before_files: before_f,
                    after_files: self.selected_files.clone(),
                });
                DiffReviewOutcome::SelectionChanged {
                    keys: self
                        .selected_files
                        .iter()
                        .map(|id| format!("file:{id}"))
                        .collect(),
                }
            }
            _ => DiffReviewOutcome::Ignored,
        }
    }

    fn commit_comment(
        &mut self,
        body: String,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffReviewOutcome {
        self.comment_seq = self.comment_seq.saturating_add(1);
        let id = format!("c{}", self.comment_seq);
        let anchor = if let Some(l) = lines.get(self.view.cursor) {
            DiffCommentAnchor::Line {
                line_id: l.id.to_string(),
            }
        } else if let Some(h) = hunks.get(self.view.hunk_cursor) {
            DiffCommentAnchor::Hunk {
                hunk_id: h.id.clone(),
            }
        } else {
            DiffCommentAnchor::File {
                file_id: String::new(),
            }
        };
        let comment = DiffComment::new(id, anchor, body);
        self.push_undo(ReviewOp::CommentAdd(comment.clone()));
        self.comments.push(comment.clone());
        DiffReviewOutcome::CommentAdded(comment)
    }

    fn external_editor(
        &self,
        lines: &[DiffLine<'_>],
        files: &[DiffReviewFileRow<'_>],
    ) -> DiffReviewOutcome {
        let path = lines
            .get(self.view.cursor)
            .and_then(|l| l.file_id)
            .or_else(|| files.get(self.file_cursor).map(|f| f.id))
            .unwrap_or("")
            .to_string();
        let line = lines
            .get(self.view.cursor)
            .and_then(|l| l.new_no.or(l.old_no));
        if path.is_empty() {
            DiffReviewOutcome::Ignored
        } else {
            DiffReviewOutcome::ExternalEditorRequested { path, line }
        }
    }
}

/// Interactive DiffReview chrome.
#[derive(Debug, Clone)]
pub struct DiffReview<'a> {
    lines: &'a [DiffLine<'a>],
    hunks: &'a [DiffHunk],
    files: &'a [DiffReviewFileRow<'a>],
    diff_files: &'a [DiffFile<'a>],
    system: &'a DesignSystem,
    focused: bool,
    colorless: bool,
    title: Option<&'a str>,
    show_tree: bool,
    show_summary: bool,
}

impl<'a> DiffReview<'a> {
    /// Lines + design system.
    #[must_use]
    pub const fn new(lines: &'a [DiffLine<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            hunks: &[],
            files: &[],
            diff_files: &[],
            system,
            focused: true,
            colorless: false,
            title: None,
            show_tree: true,
            show_summary: true,
        }
    }

    /// Hunks.
    #[must_use]
    pub const fn hunks(mut self, hunks: &'a [DiffHunk]) -> Self {
        self.hunks = hunks;
        self
    }

    /// File tree rows.
    #[must_use]
    pub const fn files(mut self, files: &'a [DiffReviewFileRow<'a>]) -> Self {
        self.files = files;
        self
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Show file tree when width allows.
    #[must_use]
    pub const fn show_tree(mut self, on: bool) -> Self {
        self.show_tree = on;
        self
    }

    /// Show summary strip.
    #[must_use]
    pub const fn show_summary(mut self, on: bool) -> Self {
        self.show_summary = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut DiffReviewState) {
        state.file_regions.clear();
        if area.is_empty() {
            return;
        }
        let colorless = self.colorless || state.colorless || self.system.mono();
        state.origin = (area.x, area.y);
        let surface = self.focused && state.accepts_input;

        let summary_h = u16::from(self.show_summary && area.height >= 4);
        let confirm_h = u16::from(state.pending_confirm.is_some() && area.height >= 5);
        let draft_h = u16::from(state.comment_draft.is_some() && area.height >= 5);
        let bottom_h = summary_h + confirm_h + draft_h;

        let tree_w = if self.show_tree && !self.files.is_empty() && area.width >= 48 {
            (area.width / 4).clamp(14, 28)
        } else {
            0
        };
        state.tree_width = tree_w;

        let tree_area = Rect::new(area.x, area.y, tree_w, area.height.saturating_sub(bottom_h));
        let body = Rect::new(
            area.x.saturating_add(tree_w),
            area.y,
            area.width.saturating_sub(tree_w),
            area.height.saturating_sub(bottom_h),
        );
        state.tree_area = tree_area;
        state.body_area = body;
        state.summary_area = Rect::new(
            area.x,
            area.bottom().saturating_sub(summary_h),
            area.width,
            summary_h,
        );
        state.comment_area = Rect::new(
            area.x,
            area.bottom().saturating_sub(bottom_h),
            area.width,
            draft_h.max(confirm_h),
        );

        // File tree
        if tree_w > 0 {
            paint_file_tree(
                buffer,
                tree_area,
                self.files,
                state,
                self.system,
                surface,
                colorless,
            );
        }

        // Decision marks injected into title
        let title = self.title.unwrap_or("review");
        let view = DiffView::new(self.lines, self.system)
            .hunks(self.hunks)
            .files(self.diff_files)
            .focused(self.focused && state.region == DiffReviewRegion::Diff)
            .colorless(colorless)
            .title(title);
        view.render(body, buffer, &mut state.view);

        // Overlay selection / comment / decision marks on visible regions
        paint_review_marks(buffer, body, self.lines, state, self.system, colorless);

        let mut y = area.bottom().saturating_sub(bottom_h);

        // Confirm banner
        if let Some(c) = &state.pending_confirm {
            let msg = format!(
                "! {} {}? Enter=yes Esc=no",
                c.decision.safe_verb(),
                c.subject
            );
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Warning),
            );
            y = y.saturating_add(1);
        }

        // Comment draft
        if let Some(draft) = &state.comment_draft {
            let msg = format!("comment> {draft}_");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Accent),
            );
            y = y.saturating_add(1);
        }

        // Summary
        if summary_h > 0 {
            let separator = " · ";
            let sum = state.summary(self.files.len(), self.hunks.len());
            let focus = state.region == DiffReviewRegion::Summary;
            let msg = format!(
                "{} files {} hunks{separator}A{} R{} S{}{separator}cmt {}/{}{separator}sel {}{separator}a/r/t/x{separator}u undo{}",
                sum.files,
                sum.hunks,
                sum.approved,
                sum.rejected,
                sum.staged,
                sum.unresolved_comments,
                sum.comments,
                sum.selected,
                if focus { " · SUMMARY" } else { "" }
            );
            let style = if focus && surface {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
        }
    }
}

fn paint_file_tree(
    buffer: &mut Buffer,
    area: Rect,
    files: &[DiffReviewFileRow<'_>],
    state: &mut DiffReviewState,
    system: &DesignSystem,
    surface: bool,
    colorless: bool,
) {
    if area.is_empty() {
        return;
    }
    let focus = state.region == DiffReviewRegion::FileTree && surface;
    let head = "files";
    buffer.set_stringn(
        area.x,
        area.y,
        take_display_cols(head, usize::from(area.width)),
        usize::from(area.width),
        if focus {
            system.style(Role::TextStrong)
        } else {
            system.style(Role::TextMuted)
        },
    );
    let mut y = area.y.saturating_add(1);
    for (i, f) in files.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }
        let unit = DiffReviewUnit::file(f.id);
        let dec = state
            .decisions
            .get(&unit.key())
            .copied()
            .unwrap_or(DiffDecision::Pending);
        let sel = state.selected_files.contains(f.id);
        let cur = i == state.file_cursor;
        let mark = dec.glyph();
        let sel_m = if sel { "★" } else { " " };
        let gutter = " ";
        let stats = if area.width >= 20 {
            format!(" +{} -{}", f.added, f.removed)
        } else {
            String::new()
        };
        let line = format!("{gutter}{mark}{sel_m}{}{stats}", f.path);
        let chrome = crate::widgets::row_chrome::RowChrome::resolve(
            system,
            ListRowVisualState {
                selected: sel || (cur && focus),
                focused: cur && focus,
                enabled: true,
                ..Default::default()
            },
        )
        .colorless(colorless);
        let style = chrome.label_style(system.style(Role::Text));
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        chrome.paint(buffer, Rect::new(area.x, y, area.width, 1));
        state
            .file_regions
            .push((f.id.to_string(), Rect::new(area.x, y, area.width, 1)));
        y = y.saturating_add(1);
    }
    // Divider
    if area.width > 0 {
        let div_x = area.right().saturating_sub(1);
        for row in area.y..area.bottom() {
            buffer.set_stringn(div_x, row, "│", 1, system.style(Role::Border));
        }
    }
}

fn paint_review_marks(
    buffer: &mut Buffer,
    body: Rect,
    lines: &[DiffLine<'_>],
    state: &DiffReviewState,
    system: &DesignSystem,
    colorless: bool,
) {
    // Use DiffView regions if present
    for region in &state.view.regions {
        let Some(line) = lines.iter().find(|l| l.id == region.id) else {
            continue;
        };
        let mut marks = String::new();
        if state.selected_lines.contains(line.id) {
            marks.push('★');
        }
        if let Some(hid) = line.hunk_id {
            if state.selected_hunks.contains(hid) {
                marks.push('◈');
            }
            let key = DiffReviewUnit::hunk(hid).key();
            if let Some(d) = state.decisions.get(&key) {
                if *d != DiffDecision::Pending {
                    marks.push_str(d.glyph());
                }
            }
        }
        if state
            .comments
            .iter()
            .any(|c| matches!(&c.anchor, DiffCommentAnchor::Line { line_id } if line_id == line.id))
        {
            marks.push('💬');
        }
        if marks.is_empty() {
            continue;
        }
        // Paint at right edge of row
        let x = body
            .right()
            .saturating_sub(
                u16::try_from(crate::text::display_cols(&marks))
                    .unwrap_or(u16::MAX)
                    .saturating_add(1),
            )
            .max(body.x);
        let style = if colorless {
            system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
        } else {
            system.style(Role::Accent)
        };
        buffer.set_stringn(x, region.area.y, take_display_cols(&marks, 4), 4, style);
    }
}

impl StatefulWidget for &DiffReview<'_> {
    type State = DiffReviewState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DiffReview::paint(self, area, buffer, state);
    }
}

impl StatefulWidget for DiffReview<'_> {
    type State = DiffReviewState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DiffReview::paint(&self, area, buffer, state);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Review session paint targets.
pub mod bench {
    /// Viewport rows.
    pub const VIEWPORT: u16 = 40;
    /// Undo stack.
    pub const UNDO: usize = super::DIFF_REVIEW_UNDO_LIMIT;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::diff::{DiffKind, DiffMode};

    /// GAP-DF-1: accepting in the file tree accepts the file you are on.
    #[test]
    fn the_file_tree_accepts_and_rejects_whole_files() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        state.set_accepts_input(true);
        state.region = DiffReviewRegion::FileTree;

        // Move to the second file, then accept it.
        let out = state.handle_key_lines(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        assert!(
            matches!(out, DiffReviewOutcome::FileCursorMoved { ref id } if id == "b.rs"),
            "{out:?}"
        );

        let out = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        let DiffReviewOutcome::ApproveRequested { units } = out else {
            panic!("expected an approve request, got {out:?}");
        };
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].kind, DiffReviewUnitKind::File);
        assert_eq!(
            units[0].id, "b.rs",
            "the focused file, not a hunk elsewhere"
        );

        // Rejecting from the tree is the same shape.
        let out = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        let DiffReviewOutcome::RejectRequested { units } = out else {
            panic!("expected a reject request, got {out:?}");
        };
        assert_eq!(units[0].kind, DiffReviewUnitKind::File);
        assert_eq!(units[0].id, "b.rs");
    }

    fn sample_hunks() -> [DiffHunk; 3] {
        [
            DiffHunk::new(0, 3, "@@ -1,3 +1,3 @@")
                .id("h0")
                .file_id("a.rs"),
            DiffHunk::new(3, 2, "@@ -10,2 +10,2 @@")
                .id("h1")
                .file_id("a.rs"),
            DiffHunk::new(5, 3, "@@ -20,3 +20,4 @@")
                .id("h2")
                .file_id("b.rs"),
        ]
    }

    fn sample_lines() -> [DiffLine<'static>; 8] {
        [
            DiffLine::hunk_header("0", "@@ -1,3 +1,3 @@")
                .hunk_id("h0")
                .file_id("a.rs"),
            DiffLine::context("1", "context")
                .hunk_id("h0")
                .file_id("a.rs"),
            DiffLine::removed("2", "old").hunk_id("h0").file_id("a.rs"),
            DiffLine::added("3", "new 東京")
                .hunk_id("h0")
                .file_id("a.rs"),
            DiffLine::hunk_header("4", "@@ -10,2 +10,2 @@")
                .hunk_id("h1")
                .file_id("a.rs"),
            DiffLine::removed("5", "gone").hunk_id("h1").file_id("a.rs"),
            DiffLine::hunk_header("6", "@@ -20,3 +20,4 @@")
                .hunk_id("h2")
                .file_id("b.rs"),
            DiffLine::added("7", "ready 🧪")
                .hunk_id("h2")
                .file_id("b.rs"),
        ]
    }

    fn sample_files() -> [DiffReviewFileRow<'static>; 2] {
        [
            DiffReviewFileRow::new("a.rs", "src/a.rs").stats(2, 2),
            DiffReviewFileRow::new("b.rs", "src/b.rs").stats(1, 0),
        ]
    }

    #[test]
    fn hunk_cursor_and_activate() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::HunkCursorMoved { index: 1 }
        ));
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::HunkActivated { index: 1 }
        ));
    }

    #[test]
    fn approve_and_undo() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::ApproveRequested { .. }
        ));
        let key = DiffReviewUnit::hunk("h0").key();
        assert_eq!(state.decisions().get(&key), Some(&DiffDecision::Approved));
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::Undone
        ));
        assert!(!state.decisions().contains_key(&key));
    }

    #[test]
    fn multi_reject_requires_confirm() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        let _ = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        state.view.hunk_cursor = 1;
        let _ = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        assert!(state.selected_hunks().len() >= 2);
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::ConfirmRequired(_)
        ));
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::RejectRequested { .. }
        ));
    }

    #[test]
    fn comment_draft_commit() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        let _ = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        for ch in ['n', 'i', 'c', 'e'] {
            let _ = state.handle_key_lines(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files,
            );
        }
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::CommentAdded(c) if c.body == "nice"
        ));
        assert_eq!(state.comments().len(), 1);
    }

    #[test]
    fn selection_survives_mode_change() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        let _ = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        assert!(state.selected_hunks().contains("h0"));
        let _ = state.handle_key_lines(
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        assert!(state.selected_hunks().contains("h0"));
        assert!(matches!(state.view.mode, DiffMode::Split));
    }

    #[test]
    fn file_tree_nav_and_stage() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        state.region = DiffReviewRegion::FileTree;
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::FileCursorMoved { id } if id == "b.rs"
        ));
        state.region = DiffReviewRegion::Diff;
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::StageRequested { .. }
        ));
    }

    #[test]
    fn external_editor() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        state.view.cursor = 2;
        assert!(matches!(
            state.handle_key_lines(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
                &lines,
                &hunks,
                &files
            ),
            DiffReviewOutcome::ExternalEditorRequested { path, .. } if path == "a.rs"
        ));
    }

    #[test]
    fn paint_tree_summary_empty() {
        let system = DesignSystem::default();
        let lines = sample_lines();
        let hunks = sample_hunks();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        let area = Rect::new(0, 0, 72, 16);
        let mut buf = Buffer::empty(area);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .files(&files)
            .title("PR #12")
            .render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("a.rs") || text.contains("files") || text.contains("hunk"),
            "{text}"
        );
        assert!(!state.file_regions.is_empty());

        let mut empty = DiffReviewState::new();
        let mut ebuf = Buffer::empty(area);
        DiffReview::new(&[], &system).render(area, &mut ebuf, &mut empty);
    }

    #[test]
    fn accepts_input_gate() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::Ignored
        ));
    }

    #[test]
    fn safe_verbs_and_glyphs() {
        assert!(DiffDecision::Rejected.safe_verb().contains("reject"));
        assert!(DiffDecision::Applied.is_destructive());
        assert!(!DiffDecision::Approved.is_destructive());
        assert_eq!(DiffDecision::Staged.glyph(), "●");
    }

    #[test]
    fn summary_counts() {
        let mut state = DiffReviewState::new();
        state.hydrate_decision(DiffReviewUnit::hunk("h0"), DiffDecision::Approved);
        state.hydrate_decision(DiffReviewUnit::hunk("h1"), DiffDecision::Staged);
        state.comments.push(DiffComment::new(
            "c1",
            DiffCommentAnchor::Hunk {
                hunk_id: "h0".into(),
            },
            "nits",
        ));
        let s = state.summary(2, 3);
        assert_eq!(s.approved, 1);
        assert_eq!(s.staged, 1);
        assert_eq!(s.comments, 1);
        assert_eq!(s.unresolved_comments, 1);
    }

    #[test]
    fn fuzz_decisions_and_regions() {
        for d in [
            DiffDecision::Pending,
            DiffDecision::Approved,
            DiffDecision::Rejected,
            DiffDecision::Staged,
            DiffDecision::Applied,
        ] {
            assert!(!d.id().is_empty());
            assert!(!d.safe_verb().is_empty());
        }
        for r in [
            DiffReviewRegion::FileTree,
            DiffReviewRegion::Diff,
            DiffReviewRegion::Comments,
            DiffReviewRegion::Summary,
        ] {
            assert!(!r.id().is_empty());
            assert_eq!(r.next().prev(), r);
        }
        assert_eq!(bench::UNDO, DIFF_REVIEW_UNDO_LIMIT);
        let _ = DiffKind::Added;
    }

    #[test]
    fn sustained_paint() {
        let system = DesignSystem::default();
        let lines = sample_lines();
        let hunks = sample_hunks();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        let review = DiffReview::new(&lines, &system).hunks(&hunks).files(&files);
        for _ in 0..30 {
            (&review).render(area, &mut buf, &mut state);
        }
        assert!(state.view.regions.len() <= 25);
    }

    #[test]
    fn tab_cycles_region() {
        let hunks = sample_hunks();
        let lines = sample_lines();
        let files = sample_files();
        let mut state = DiffReviewState::new();
        assert_eq!(state.region, DiffReviewRegion::Diff);
        let _ = state.handle_key_lines(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &lines,
            &hunks,
            &files,
        );
        assert_eq!(state.region, DiffReviewRegion::Comments);
    }
}
