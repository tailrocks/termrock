// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **PlanReview** — interactive review surface for agent-generated plans.
//!
//! **Mission.** Render Markdown plan body, sections, source refs, tasks, risks,
//! assumptions, and affected files. Support line/range comments, selection,
//! approve / approve-with-conditions / request-revision / edit-feedback /
//! abandon. Preserve comments across plan updates when anchors stay stable.
//! Show version changes between revisions. **Safe focus** and explicit
//! consequences — Approve never initial focus.
//!
//! **vs [`super::PermissionPrompt`].** Trust gate for a single action; PlanReview
//! is multi-section plan document review before execution.
//! **vs [`super::DiffReview`].** File hunk review; PlanReview is plan-level.
//! **vs [`super::QuestionFlow`].** Interview Q&A, not plan approval.
//!
//! Research: Grok Build plan approval, code review workflows, document annotation.

use std::collections::BTreeMap;

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
    style::{DesignSystem, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::panel::Panel,
    widgets::permission::PermissionRisk,
};

/// Overlay id for fullscreen plan review.
pub const PLAN_REVIEW_FULLSCREEN_OVERLAY_ID: &str = "termrock.plan_review_fullscreen";
/// Overlay id for plan review dialog.
pub const PLAN_REVIEW_OVERLAY_ID: &str = "termrock.plan";
/// Max body lines painted in the document window.
pub const PLAN_REVIEW_BODY_WINDOW: usize = 48;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Kind of file change projected by the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PlanFileChange {
    /// New file.
    #[default]
    Create,
    /// Existing file modified.
    Modify,
    /// File removed.
    Delete,
    /// Rename / move.
    Rename,
}

impl PlanFileChange {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Modify => "modify",
            Self::Delete => "delete",
            Self::Rename => "rename",
        }
    }

    /// Short glyph.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Create => "+",
            Self::Modify => "~",
            Self::Delete => "-",
            Self::Rename => ">",
        }
    }
}

/// Task status in the plan (planning, not execution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PlanTaskStatus {
    /// Not started.
    #[default]
    Pending,
    /// In progress in the plan narrative.
    InProgress,
    /// Marked done by the agent plan.
    Done,
    /// Blocked / deferred.
    Blocked,
}

impl PlanTaskStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Pending => "·",
            Self::InProgress => "›",
            Self::Done => "✓",
            Self::Blocked => "!",
        }
    }
}

/// Named section of the plan body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSection {
    /// Stable section id (anchor key).
    pub id: String,
    /// Heading.
    pub title: String,
    /// Body (plain / markdown fragment).
    pub body: String,
    /// Optional 0-based line start in markdown_body.
    pub line_start: Option<usize>,
    /// Optional exclusive line end.
    pub line_end: Option<usize>,
}

impl PlanSection {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            line_start: None,
            line_end: None,
        }
    }

    /// Line range in the full markdown body.
    #[must_use]
    pub const fn lines(mut self, start: usize, end: usize) -> Self {
        self.line_start = Some(start);
        self.line_end = Some(end);
        self
    }
}

/// One plan task / step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTask {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Detail.
    pub detail: Option<String>,
    /// Status.
    pub status: PlanTaskStatus,
}

impl PlanTask {
    /// Pending task.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            status: PlanTaskStatus::Pending,
        }
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: PlanTaskStatus) -> Self {
        self.status = s;
        self
    }
}

/// Risk callout inside the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRiskItem {
    /// Stable id.
    pub id: String,
    /// Text.
    pub text: String,
    /// Severity.
    pub severity: PermissionRisk,
}

impl PlanRiskItem {
    /// Construct.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        text: impl Into<String>,
        severity: PermissionRisk,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            severity,
        }
    }
}

/// Explicit assumption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanAssumption {
    /// Stable id.
    pub id: String,
    /// Text.
    pub text: String,
}

impl PlanAssumption {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

/// Affected file projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanAffectedFile {
    /// Path (display).
    pub path: String,
    /// Change kind.
    pub change: PlanFileChange,
    /// Optional rename target.
    pub rename_to: Option<String>,
}

impl PlanAffectedFile {
    /// Construct.
    #[must_use]
    pub fn new(path: impl Into<String>, change: PlanFileChange) -> Self {
        Self {
            path: path.into(),
            change,
            rename_to: None,
        }
    }

    /// Rename target.
    #[must_use]
    pub fn rename_to(mut self, p: impl Into<String>) -> Self {
        self.rename_to = Some(p.into());
        self
    }
}

/// Source / citation reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSourceRef {
    /// Stable id.
    pub id: String,
    /// Label.
    pub label: String,
    /// Location hint (path:line, url, …).
    pub location: Option<String>,
}

impl PlanSourceRef {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            location: None,
        }
    }

    /// Location.
    #[must_use]
    pub fn location(mut self, loc: impl Into<String>) -> Self {
        self.location = Some(loc.into());
        self
    }
}

/// Full plan document snapshot (host-projected, immutable for paint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDocument {
    /// Plan id (stable across revisions).
    pub id: String,
    /// Monotonic version.
    pub version: u32,
    /// Title.
    pub title: String,
    /// Short summary.
    pub summary: Option<String>,
    /// Full Markdown body (source of line anchors).
    pub markdown_body: String,
    /// Named sections.
    pub sections: Vec<PlanSection>,
    /// Tasks / steps.
    pub tasks: Vec<PlanTask>,
    /// Risk items.
    pub risks: Vec<PlanRiskItem>,
    /// Assumptions.
    pub assumptions: Vec<PlanAssumption>,
    /// Affected files.
    pub affected_files: Vec<PlanAffectedFile>,
    /// Source references.
    pub source_refs: Vec<PlanSourceRef>,
    /// Aggregate risk (required for action focus).
    pub risk: PermissionRisk,
    /// Previous revision for version-diff pane (optional).
    pub previous: Option<Box<PlanDocument>>,
}

impl PlanDocument {
    /// Minimal plan.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        version: u32,
        title: impl Into<String>,
        markdown_body: impl Into<String>,
        risk: PermissionRisk,
    ) -> Self {
        Self {
            id: id.into(),
            version,
            title: title.into(),
            summary: None,
            markdown_body: markdown_body.into(),
            sections: Vec::new(),
            tasks: Vec::new(),
            risks: Vec::new(),
            assumptions: Vec::new(),
            affected_files: Vec::new(),
            source_refs: Vec::new(),
            risk,
            previous: None,
        }
    }

    /// Summary.
    #[must_use]
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }

    /// Sections.
    #[must_use]
    pub fn sections(mut self, s: Vec<PlanSection>) -> Self {
        self.sections = s;
        self
    }

    /// Tasks.
    #[must_use]
    pub fn tasks(mut self, t: Vec<PlanTask>) -> Self {
        self.tasks = t;
        self
    }

    /// Risks.
    #[must_use]
    pub fn risks(mut self, r: Vec<PlanRiskItem>) -> Self {
        self.risks = r;
        self
    }

    /// Assumptions.
    #[must_use]
    pub fn assumptions(mut self, a: Vec<PlanAssumption>) -> Self {
        self.assumptions = a;
        self
    }

    /// Files.
    #[must_use]
    pub fn affected_files(mut self, f: Vec<PlanAffectedFile>) -> Self {
        self.affected_files = f;
        self
    }

    /// Source refs.
    #[must_use]
    pub fn source_refs(mut self, r: Vec<PlanSourceRef>) -> Self {
        self.source_refs = r;
        self
    }

    /// Previous revision.
    #[must_use]
    pub fn previous(mut self, prev: PlanDocument) -> Self {
        self.previous = Some(Box::new(prev));
        self
    }

    /// Body lines (split once).
    #[must_use]
    pub fn body_lines(&self) -> Vec<&str> {
        if self.markdown_body.is_empty() {
            Vec::new()
        } else {
            self.markdown_body.lines().collect()
        }
    }

    /// Number of body lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        if self.markdown_body.is_empty() {
            0
        } else {
            self.markdown_body.lines().count()
        }
    }

    /// Whether Approve is allowed (non-empty plan content).
    #[must_use]
    pub fn can_approve(&self) -> bool {
        !self.markdown_body.trim().is_empty()
            || !self.tasks.is_empty()
            || !self.sections.is_empty()
    }
}

// ── Comments & anchors ──────────────────────────────────────────────────────

/// Where a comment attaches.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanCommentAnchor {
    /// Single body line (0-based).
    Line {
        /// Line.
        line: usize,
    },
    /// Inclusive line range.
    Range {
        /// Start.
        start: usize,
        /// End inclusive.
        end: usize,
    },
    /// Section id.
    Section {
        /// Section id.
        section_id: String,
    },
    /// Task id.
    Task {
        /// Task id.
        task_id: String,
    },
    /// Affected file path.
    File {
        /// Path.
        path: String,
    },
    /// Anchor lost after remap.
    Orphan {
        /// Reason.
        reason: String,
    },
}

impl PlanCommentAnchor {
    /// Whether this is orphaned.
    #[must_use]
    pub const fn is_orphan(&self) -> bool {
        matches!(self, Self::Orphan { .. })
    }
}

/// Inline review comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanComment {
    /// Stable comment id.
    pub id: String,
    /// Body text.
    pub body: String,
    /// Optional author label.
    pub author: Option<String>,
    /// Anchor.
    pub anchor: PlanCommentAnchor,
    /// Plan version when created.
    pub created_at_version: u32,
    /// Survived remapping (false if orphaned).
    pub preserved: bool,
}

impl PlanComment {
    /// Construct.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        body: impl Into<String>,
        anchor: PlanCommentAnchor,
        version: u32,
    ) -> Self {
        Self {
            id: id.into(),
            body: body.into(),
            author: None,
            anchor,
            created_at_version: version,
            preserved: true,
        }
    }

    /// Author.
    #[must_use]
    pub fn author(mut self, a: impl Into<String>) -> Self {
        self.author = Some(a.into());
        self
    }
}

/// Remap comments onto an updated plan. Stable section/task/file anchors keep
/// comments; line anchors keep only when the line still exists (clamped range);
/// otherwise mark orphan with `preserved = false`.
#[must_use]
pub fn remap_plan_comments(
    comments: &[PlanComment],
    plan: &PlanDocument,
) -> Vec<PlanComment> {
    let section_ids: BTreeMap<&str, ()> = plan
        .sections
        .iter()
        .map(|s| (s.id.as_str(), ()))
        .collect();
    let task_ids: BTreeMap<&str, ()> = plan.tasks.iter().map(|t| (t.id.as_str(), ())).collect();
    let file_paths: BTreeMap<&str, ()> = plan
        .affected_files
        .iter()
        .map(|f| (f.path.as_str(), ()))
        .collect();
    let line_count = plan.line_count();

    comments
        .iter()
        .map(|c| {
            let mut out = c.clone();
            match &c.anchor {
                PlanCommentAnchor::Section { section_id } => {
                    if !section_ids.contains_key(section_id.as_str()) {
                        out.anchor = PlanCommentAnchor::Orphan {
                            reason: format!("section:{section_id}"),
                        };
                        out.preserved = false;
                    } else {
                        out.preserved = true;
                    }
                }
                PlanCommentAnchor::Task { task_id } => {
                    if !task_ids.contains_key(task_id.as_str()) {
                        out.anchor = PlanCommentAnchor::Orphan {
                            reason: format!("task:{task_id}"),
                        };
                        out.preserved = false;
                    } else {
                        out.preserved = true;
                    }
                }
                PlanCommentAnchor::File { path } => {
                    if !file_paths.contains_key(path.as_str()) {
                        out.anchor = PlanCommentAnchor::Orphan {
                            reason: format!("file:{path}"),
                        };
                        out.preserved = false;
                    } else {
                        out.preserved = true;
                    }
                }
                PlanCommentAnchor::Line { line } => {
                    if line_count == 0 || *line >= line_count {
                        out.anchor = PlanCommentAnchor::Orphan {
                            reason: format!("line:{line}"),
                        };
                        out.preserved = false;
                    } else {
                        out.preserved = true;
                    }
                }
                PlanCommentAnchor::Range { start, end } => {
                    if line_count == 0 || *start >= line_count {
                        out.anchor = PlanCommentAnchor::Orphan {
                            reason: format!("range:{start}-{end}"),
                        };
                        out.preserved = false;
                    } else {
                        let s = (*start).min(line_count.saturating_sub(1));
                        let e = (*end).min(line_count.saturating_sub(1)).max(s);
                        out.anchor = PlanCommentAnchor::Range { start: s, end: e };
                        out.preserved = true;
                    }
                }
                PlanCommentAnchor::Orphan { .. } => {
                    out.preserved = false;
                }
            }
            out
        })
        .collect()
}

// ── Actions / panes / phases ────────────────────────────────────────────────

/// Decision actions — Approve is grant-class and never default focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PlanAction {
    /// Abandon plan (safe default for high risk).
    #[default]
    Abandon,
    /// Request revision with feedback.
    RequestRevision,
    /// Edit free-form feedback draft (does not resolve).
    EditFeedback,
    /// Approve with explicit conditions text.
    ApproveWithConditions,
    /// Approve plan (grant — never initial focus).
    Approve,
}

impl PlanAction {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Abandon => "abandon",
            Self::RequestRevision => "request_revision",
            Self::EditFeedback => "edit_feedback",
            Self::ApproveWithConditions => "approve_conditions",
            Self::Approve => "approve",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Abandon => "Abandon",
            Self::RequestRevision => "Revise",
            Self::EditFeedback => "Feedback",
            Self::ApproveWithConditions => "Approve…",
            Self::Approve => "Approve",
        }
    }

    /// Whether this grants execution authority.
    #[must_use]
    pub const fn grants(self) -> bool {
        matches!(self, Self::Approve | Self::ApproveWithConditions)
    }

    /// Consequence hint for chrome.
    #[must_use]
    pub const fn consequence(self) -> &'static str {
        match self {
            Self::Abandon => "discard plan; no execution",
            Self::RequestRevision => "send feedback; agent revises",
            Self::EditFeedback => "edit draft feedback",
            Self::ApproveWithConditions => "execute under stated conditions",
            Self::Approve => "execute plan as written",
        }
    }

    /// Default safe focus for risk — **never Approve**.
    #[must_use]
    pub const fn default_for_risk(risk: PermissionRisk) -> Self {
        match risk {
            PermissionRisk::Low | PermissionRisk::Medium => Self::RequestRevision,
            PermissionRisk::High | PermissionRisk::Critical => Self::Abandon,
        }
    }

    /// Ordered action strip.
    #[must_use]
    pub fn strip() -> &'static [PlanAction] {
        &[
            Self::Abandon,
            Self::RequestRevision,
            Self::EditFeedback,
            Self::ApproveWithConditions,
            Self::Approve,
        ]
    }
}

/// Content pane focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PlanReviewPane {
    /// Markdown body + sections.
    #[default]
    Document,
    /// Tasks.
    Tasks,
    /// Risks + assumptions.
    Risks,
    /// Affected files.
    Files,
    /// Comments.
    Comments,
    /// Version diff vs previous.
    Diff,
}

impl PlanReviewPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Tasks => "tasks",
            Self::Risks => "risks",
            Self::Files => "files",
            Self::Comments => "comments",
            Self::Diff => "diff",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Document => "Doc",
            Self::Tasks => "Tasks",
            Self::Risks => "Risks",
            Self::Files => "Files",
            Self::Comments => "Notes",
            Self::Diff => "Diff",
        }
    }

    /// Cycle order.
    #[must_use]
    pub fn cycle() -> &'static [PlanReviewPane] {
        &[
            Self::Document,
            Self::Tasks,
            Self::Risks,
            Self::Files,
            Self::Comments,
            Self::Diff,
        ]
    }
}

/// Interaction phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PlanReviewPhase {
    /// Browsing / deciding.
    #[default]
    Review,
    /// Typing feedback for revision request.
    Feedback,
    /// Typing conditions for conditional approve.
    Conditions,
    /// Typing a new comment.
    Comment,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — requests only; host owns execution policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanReviewOutcome {
    /// Ignored.
    Ignored,
    /// Pane changed.
    PaneChanged(PlanReviewPane),
    /// Body line selected.
    LineSelected {
        /// 0-based line.
        line: usize,
    },
    /// Line range selection.
    SelectionChanged {
        /// Start.
        start: usize,
        /// End inclusive.
        end: usize,
    },
    /// Section focused.
    SectionSelected {
        /// Id.
        id: String,
    },
    /// Task focused.
    TaskSelected {
        /// Id.
        id: String,
    },
    /// File focused.
    FileSelected {
        /// Path.
        path: String,
    },
    /// Action cursor moved (does **not** confirm).
    ActionFocused(PlanAction),
    /// Plan approved as written.
    Approved,
    /// Plan approved with conditions text.
    ApprovedWithConditions {
        /// Conditions.
        conditions: String,
    },
    /// Revision requested with feedback.
    RevisionRequested {
        /// Feedback.
        feedback: String,
    },
    /// Feedback draft edited (not submitted).
    FeedbackEdited {
        /// Draft.
        text: String,
    },
    /// Plan abandoned.
    Abandoned,
    /// Overlay cancelled (≠ Abandoned); no approve.
    Cancelled,
    /// Comment added.
    CommentAdded(PlanComment),
    /// Comment removed.
    CommentRemoved {
        /// Id.
        id: String,
    },
    /// Version diff toggled.
    VersionDiffToggled {
        /// On.
        show: bool,
    },
    /// Phase entered.
    PhaseChanged(PlanReviewPhase),
    /// Fullscreen promote.
    FullscreenRequested,
    /// Plan document replaced / remapped.
    PlanUpdated {
        /// Version.
        version: u32,
        /// Comments preserved count.
        preserved_comments: usize,
        /// Orphaned comments count.
        orphaned_comments: usize,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive plan review state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanReviewState {
    /// Open plan.
    pub plan: Option<PlanDocument>,
    /// Comments (host may seed; remapped on update).
    pub comments: Vec<PlanComment>,
    /// Phase.
    pub phase: PlanReviewPhase,
    /// Content pane.
    pub pane: PlanReviewPane,
    /// Focused action (safe default).
    pub action_cursor: PlanAction,
    /// Selected body line.
    pub selected_line: usize,
    /// Selection end (range); None = single line.
    pub selection_end: Option<usize>,
    /// Selected section index.
    pub selected_section: usize,
    /// Selected task index.
    pub selected_task: usize,
    /// Selected risk index.
    pub selected_risk: usize,
    /// Selected file index.
    pub selected_file: usize,
    /// Selected comment index.
    pub selected_comment: usize,
    /// Document scroll offset.
    pub scroll: usize,
    /// Feedback draft (revision / edit).
    pub draft_feedback: String,
    /// Conditions draft.
    pub draft_conditions: String,
    /// Comment draft.
    pub draft_comment: String,
    /// Show version diff when previous exists.
    pub show_version_diff: bool,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Next comment id counter.
    comment_seq: u32,
    /// Action hit regions.
    pub action_regions: Vec<(PlanAction, Rect)>,
    /// Pane tab hits.
    pub pane_hits: Vec<(PlanReviewPane, Rect)>,
}

impl Default for PlanReviewState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanReviewState {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self {
            plan: None,
            comments: Vec::new(),
            phase: PlanReviewPhase::Review,
            pane: PlanReviewPane::Document,
            action_cursor: PlanAction::RequestRevision,
            selected_line: 0,
            selection_end: None,
            selected_section: 0,
            selected_task: 0,
            selected_risk: 0,
            selected_file: 0,
            selected_comment: 0,
            scroll: 0,
            draft_feedback: String::new(),
            draft_conditions: String::new(),
            draft_comment: String::new(),
            show_version_diff: false,
            focused: true,
            accepts_input: true,
            comment_seq: 0,
            action_regions: Vec::new(),
            pane_hits: Vec::new(),
        }
    }

    /// Open plan with safe action focus from risk.
    pub fn open(&mut self, plan: PlanDocument) {
        let risk = plan.risk;
        self.action_cursor = PlanAction::default_for_risk(risk);
        self.phase = PlanReviewPhase::Review;
        self.pane = PlanReviewPane::Document;
        self.selected_line = 0;
        self.selection_end = None;
        self.selected_section = 0;
        self.selected_task = 0;
        self.selected_risk = 0;
        self.selected_file = 0;
        self.selected_comment = 0;
        self.scroll = 0;
        self.draft_feedback.clear();
        self.draft_conditions.clear();
        self.draft_comment.clear();
        self.show_version_diff = false;
        self.comments.clear();
        self.plan = Some(plan);
        self.focused = true;
        self.accepts_input = true;
    }

    /// Replace plan and remap comments (preserve stable anchors).
    pub fn update_plan(&mut self, plan: PlanDocument) -> PlanReviewOutcome {
        let remapped = remap_plan_comments(&self.comments, &plan);
        let preserved = remapped.iter().filter(|c| c.preserved).count();
        let orphaned = remapped.len().saturating_sub(preserved);
        let version = plan.version;
        let risk = plan.risk;
        // Risk upgrade: snap off grant focus.
        if risk.is_destructive() && self.action_cursor.grants() {
            self.action_cursor = PlanAction::default_for_risk(risk);
        } else if !plan.can_approve() && self.action_cursor.grants() {
            self.action_cursor = PlanAction::default_for_risk(risk);
        }
        let line_count = plan.line_count();
        if line_count > 0 {
            self.selected_line = self.selected_line.min(line_count.saturating_sub(1));
            if let Some(end) = self.selection_end {
                self.selection_end = Some(end.min(line_count.saturating_sub(1)));
            }
        } else {
            self.selected_line = 0;
            self.selection_end = None;
        }
        self.comments = remapped;
        self.plan = Some(plan);
        PlanReviewOutcome::PlanUpdated {
            version,
            preserved_comments: preserved,
            orphaned_comments: orphaned,
        }
    }

    /// Seed comments (host).
    pub fn set_comments(&mut self, comments: Vec<PlanComment>) {
        if let Some(plan) = self.plan.as_ref() {
            self.comments = remap_plan_comments(&comments, plan);
        } else {
            self.comments = comments;
        }
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.plan.is_some()
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Action cursor.
    #[must_use]
    pub const fn action_cursor(&self) -> PlanAction {
        self.action_cursor
    }

    /// Aggregate risk.
    #[must_use]
    pub fn risk(&self) -> PermissionRisk {
        self.plan
            .as_ref()
            .map(|p| p.risk)
            .unwrap_or(PermissionRisk::Medium)
    }

    /// Selection range (start, end inclusive).
    #[must_use]
    pub fn selection_range(&self) -> (usize, usize) {
        let start = self.selected_line;
        let end = self.selection_end.unwrap_or(start).max(start);
        let start = start.min(end);
        (start, end)
    }

    fn available_actions(&self) -> Vec<PlanAction> {
        let mut actions = PlanAction::strip().to_vec();
        let can = self.plan.as_ref().is_some_and(PlanDocument::can_approve);
        if !can {
            actions.retain(|a| !a.grants());
        }
        actions
    }

    fn clamp_action_cursor(&mut self) {
        let avail = self.available_actions();
        if !avail.contains(&self.action_cursor) {
            self.action_cursor = avail
                .first()
                .copied()
                .unwrap_or_else(|| PlanAction::default_for_risk(self.risk()));
        }
    }

    fn move_action(&mut self, delta: isize) -> PlanReviewOutcome {
        let avail = self.available_actions();
        if avail.is_empty() {
            return PlanReviewOutcome::Ignored;
        }
        let idx = avail
            .iter()
            .position(|a| *a == self.action_cursor)
            .unwrap_or(0);
        let n = avail.len() as isize;
        let next = (idx as isize + delta).rem_euclid(n) as usize;
        self.action_cursor = avail[next];
        PlanReviewOutcome::ActionFocused(self.action_cursor)
    }

    fn cycle_pane(&mut self, delta: isize) -> PlanReviewOutcome {
        let panes = PlanReviewPane::cycle();
        let idx = panes
            .iter()
            .position(|p| *p == self.pane)
            .unwrap_or(0);
        let n = panes.len() as isize;
        let next = (idx as isize + delta).rem_euclid(n) as usize;
        self.pane = panes[next];
        if self.pane == PlanReviewPane::Diff {
            self.show_version_diff = true;
        }
        PlanReviewOutcome::PaneChanged(self.pane)
    }

    fn confirm_action(&mut self) -> PlanReviewOutcome {
        self.clamp_action_cursor();
        match self.action_cursor {
            PlanAction::Abandon => {
                self.plan = None;
                PlanReviewOutcome::Abandoned
            }
            PlanAction::RequestRevision => {
                if self.draft_feedback.trim().is_empty() {
                    self.phase = PlanReviewPhase::Feedback;
                    PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Feedback)
                } else {
                    let feedback = self.draft_feedback.clone();
                    self.plan = None;
                    PlanReviewOutcome::RevisionRequested { feedback }
                }
            }
            PlanAction::EditFeedback => {
                self.phase = PlanReviewPhase::Feedback;
                PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Feedback)
            }
            PlanAction::ApproveWithConditions => {
                if self.draft_conditions.trim().is_empty() {
                    self.phase = PlanReviewPhase::Conditions;
                    PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Conditions)
                } else {
                    let conditions = self.draft_conditions.clone();
                    self.plan = None;
                    PlanReviewOutcome::ApprovedWithConditions { conditions }
                }
            }
            PlanAction::Approve => {
                if !self.plan.as_ref().is_some_and(PlanDocument::can_approve) {
                    return PlanReviewOutcome::Ignored;
                }
                self.plan = None;
                PlanReviewOutcome::Approved
            }
        }
    }

    fn nav_list(&mut self, len: usize, selected: &mut usize, down: bool) {
        if len == 0 {
            return;
        }
        if down {
            *selected = (*selected + 1).min(len - 1);
        } else {
            *selected = selected.saturating_sub(1);
        }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> PlanReviewOutcome {
        if !self.focused || !self.accepts_input || key.kind != KeyEventKind::Press {
            return PlanReviewOutcome::Ignored;
        }
        if self.plan.is_none() {
            return PlanReviewOutcome::Ignored;
        }

        // Typing phases
        match self.phase {
            PlanReviewPhase::Feedback => return self.handle_text_phase(key, TextTarget::Feedback),
            PlanReviewPhase::Conditions => {
                return self.handle_text_phase(key, TextTarget::Conditions)
            }
            PlanReviewPhase::Comment => return self.handle_text_phase(key, TextTarget::Comment),
            PlanReviewPhase::Review => {}
        }

        match key.code {
            KeyCode::Esc => PlanReviewOutcome::Cancelled,
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.cycle_pane(-1),
            KeyCode::Tab => self.cycle_pane(1),
            KeyCode::Left | KeyCode::Char('h') => self.move_action(-1),
            KeyCode::Right | KeyCode::Char('l')
                if !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.move_action(1)
            }
            KeyCode::Enter => self.confirm_action(),
            // Focus-move only — never bare grant.
            KeyCode::Char('a') => {
                if self.available_actions().contains(&PlanAction::Approve) {
                    self.action_cursor = PlanAction::Approve;
                    PlanReviewOutcome::ActionFocused(PlanAction::Approve)
                } else {
                    PlanReviewOutcome::Ignored
                }
            }
            KeyCode::Char('r') => {
                self.action_cursor = PlanAction::RequestRevision;
                PlanReviewOutcome::ActionFocused(PlanAction::RequestRevision)
            }
            KeyCode::Char('x') => {
                self.action_cursor = PlanAction::Abandon;
                PlanReviewOutcome::ActionFocused(PlanAction::Abandon)
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+C reserved — ignore as grant
                PlanReviewOutcome::Ignored
            }
            KeyCode::Char('c') => {
                if self.available_actions().contains(&PlanAction::ApproveWithConditions)
                {
                    self.action_cursor = PlanAction::ApproveWithConditions;
                    PlanReviewOutcome::ActionFocused(PlanAction::ApproveWithConditions)
                } else {
                    PlanReviewOutcome::Ignored
                }
            }
            KeyCode::Char('y') => PlanReviewOutcome::Ignored, // unbound — parity PermissionPrompt
            KeyCode::Char('m') => {
                self.phase = PlanReviewPhase::Comment;
                self.draft_comment.clear();
                PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Comment)
            }
            KeyCode::Char('d') => {
                let has_prev = self
                    .plan
                    .as_ref()
                    .is_some_and(|p| p.previous.is_some());
                if !has_prev {
                    return PlanReviewOutcome::Ignored;
                }
                self.show_version_diff = !self.show_version_diff;
                if self.show_version_diff {
                    self.pane = PlanReviewPane::Diff;
                }
                PlanReviewOutcome::VersionDiffToggled {
                    show: self.show_version_diff,
                }
            }
            KeyCode::Char('f') => PlanReviewOutcome::FullscreenRequested,
            KeyCode::Char('[') => self.nav_section(-1),
            KeyCode::Char(']') => self.nav_section(1),
            KeyCode::Up | KeyCode::Char('k') => self.nav_content(false),
            KeyCode::Down | KeyCode::Char('j') => self.nav_content(true),
            KeyCode::Char('J') | KeyCode::Char('K') => {
                // Extend selection on document
                if self.pane != PlanReviewPane::Document {
                    return PlanReviewOutcome::Ignored;
                }
                let down = key.code == KeyCode::Char('J');
                let lines = self.plan.as_ref().map(PlanDocument::line_count).unwrap_or(0);
                if lines == 0 {
                    return PlanReviewOutcome::Ignored;
                }
                let (start, mut end) = self.selection_range();
                if down {
                    end = (end + 1).min(lines - 1);
                } else {
                    end = end.saturating_sub(1).max(start);
                }
                self.selected_line = start;
                self.selection_end = Some(end);
                PlanReviewOutcome::SelectionChanged { start, end }
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(8);
                PlanReviewOutcome::Ignored
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(8);
                PlanReviewOutcome::Ignored
            }
            KeyCode::Delete | KeyCode::Backspace
                if self.pane == PlanReviewPane::Comments && !self.comments.is_empty() =>
            {
                let idx = self.selected_comment.min(self.comments.len() - 1);
                let id = self.comments[idx].id.clone();
                self.comments.remove(idx);
                if self.selected_comment >= self.comments.len() && !self.comments.is_empty() {
                    self.selected_comment = self.comments.len() - 1;
                }
                PlanReviewOutcome::CommentRemoved { id }
            }
            _ => PlanReviewOutcome::Ignored,
        }
    }

    fn nav_section(&mut self, delta: isize) -> PlanReviewOutcome {
        let Some(plan) = self.plan.as_ref() else {
            return PlanReviewOutcome::Ignored;
        };
        if plan.sections.is_empty() {
            return PlanReviewOutcome::Ignored;
        }
        let n = plan.sections.len() as isize;
        let next = (self.selected_section as isize + delta).rem_euclid(n) as usize;
        self.selected_section = next;
        self.pane = PlanReviewPane::Document;
        if let Some(ls) = plan.sections[next].line_start {
            self.selected_line = ls;
            self.selection_end = None;
            self.scroll = ls.saturating_sub(2);
        }
        let id = plan.sections[next].id.clone();
        PlanReviewOutcome::SectionSelected { id }
    }

    fn nav_content(&mut self, down: bool) -> PlanReviewOutcome {
        match self.pane {
            PlanReviewPane::Document => {
                let lines = self.plan.as_ref().map(PlanDocument::line_count).unwrap_or(0);
                if lines == 0 {
                    return PlanReviewOutcome::Ignored;
                }
                if down {
                    self.selected_line = (self.selected_line + 1).min(lines - 1);
                } else {
                    self.selected_line = self.selected_line.saturating_sub(1);
                }
                self.selection_end = None;
                if self.selected_line < self.scroll {
                    self.scroll = self.selected_line;
                } else if self.selected_line >= self.scroll + PLAN_REVIEW_BODY_WINDOW {
                    self.scroll = self
                        .selected_line
                        .saturating_sub(PLAN_REVIEW_BODY_WINDOW.saturating_sub(1));
                }
                PlanReviewOutcome::LineSelected {
                    line: self.selected_line,
                }
            }
            PlanReviewPane::Tasks => {
                let len = self.plan.as_ref().map(|p| p.tasks.len()).unwrap_or(0);
                let mut sel = self.selected_task;
                self.nav_list(len, &mut sel, down);
                self.selected_task = sel;
                if let Some(t) = self.plan.as_ref().and_then(|p| p.tasks.get(sel)) {
                    PlanReviewOutcome::TaskSelected { id: t.id.clone() }
                } else {
                    PlanReviewOutcome::Ignored
                }
            }
            PlanReviewPane::Risks => {
                let len = self.plan.as_ref().map(|p| p.risks.len()).unwrap_or(0);
                let mut sel = self.selected_risk;
                self.nav_list(len, &mut sel, down);
                self.selected_risk = sel;
                PlanReviewOutcome::Ignored
            }
            PlanReviewPane::Files => {
                let len = self
                    .plan
                    .as_ref()
                    .map(|p| p.affected_files.len())
                    .unwrap_or(0);
                let mut sel = self.selected_file;
                self.nav_list(len, &mut sel, down);
                self.selected_file = sel;
                if let Some(f) = self
                    .plan
                    .as_ref()
                    .and_then(|p| p.affected_files.get(sel))
                {
                    PlanReviewOutcome::FileSelected {
                        path: f.path.clone(),
                    }
                } else {
                    PlanReviewOutcome::Ignored
                }
            }
            PlanReviewPane::Comments => {
                let len = self.comments.len();
                let mut sel = self.selected_comment;
                self.nav_list(len, &mut sel, down);
                self.selected_comment = sel;
                PlanReviewOutcome::Ignored
            }
            PlanReviewPane::Diff => {
                // scroll only
                if down {
                    self.scroll = self.scroll.saturating_add(1);
                } else {
                    self.scroll = self.scroll.saturating_sub(1);
                }
                PlanReviewOutcome::Ignored
            }
        }
    }

    fn handle_text_phase(&mut self, key: KeyEvent, target: TextTarget) -> PlanReviewOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = PlanReviewPhase::Review;
                PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Review)
            }
            KeyCode::Enter => match target {
                TextTarget::Feedback => {
                    if self.draft_feedback.trim().is_empty() {
                        return PlanReviewOutcome::Ignored;
                    }
                    // If action is RequestRevision, submit; else keep draft
                    if self.action_cursor == PlanAction::RequestRevision
                        || self.action_cursor == PlanAction::EditFeedback
                    {
                        let feedback = self.draft_feedback.clone();
                        if self.action_cursor == PlanAction::RequestRevision {
                            self.plan = None;
                            self.phase = PlanReviewPhase::Review;
                            return PlanReviewOutcome::RevisionRequested { feedback };
                        }
                        self.phase = PlanReviewPhase::Review;
                        return PlanReviewOutcome::FeedbackEdited { text: feedback };
                    }
                    self.phase = PlanReviewPhase::Review;
                    PlanReviewOutcome::FeedbackEdited {
                        text: self.draft_feedback.clone(),
                    }
                }
                TextTarget::Conditions => {
                    if self.draft_conditions.trim().is_empty() {
                        return PlanReviewOutcome::Ignored;
                    }
                    let conditions = self.draft_conditions.clone();
                    self.plan = None;
                    self.phase = PlanReviewPhase::Review;
                    PlanReviewOutcome::ApprovedWithConditions { conditions }
                }
                TextTarget::Comment => {
                    if self.draft_comment.trim().is_empty() {
                        return PlanReviewOutcome::Ignored;
                    }
                    let version = self.plan.as_ref().map(|p| p.version).unwrap_or(0);
                    self.comment_seq = self.comment_seq.saturating_add(1);
                    let (start, end) = self.selection_range();
                    let anchor = if start == end {
                        PlanCommentAnchor::Line { line: start }
                    } else {
                        PlanCommentAnchor::Range { start, end }
                    };
                    let comment = PlanComment::new(
                        format!("c{}", self.comment_seq),
                        self.draft_comment.clone(),
                        anchor,
                        version,
                    );
                    self.comments.push(comment.clone());
                    self.draft_comment.clear();
                    self.phase = PlanReviewPhase::Review;
                    PlanReviewOutcome::CommentAdded(comment)
                }
            },
            KeyCode::Backspace => {
                match target {
                    TextTarget::Feedback => {
                        self.draft_feedback.pop();
                    }
                    TextTarget::Conditions => {
                        self.draft_conditions.pop();
                    }
                    TextTarget::Comment => {
                        self.draft_comment.pop();
                    }
                }
                PlanReviewOutcome::Ignored
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                match target {
                    TextTarget::Feedback => self.draft_feedback.push(ch),
                    TextTarget::Conditions => self.draft_conditions.push(ch),
                    TextTarget::Comment => self.draft_comment.push(ch),
                }
                PlanReviewOutcome::Ignored
            }
            _ => PlanReviewOutcome::Ignored,
        }
    }

    /// Mouse: click actions / panes.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> PlanReviewOutcome {
        if !self.focused || !self.accepts_input || self.plan.is_none() {
            return PlanReviewOutcome::Ignored;
        }
        if !matches!(
            ev.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
        ) {
            return PlanReviewOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return PlanReviewOutcome::Ignored;
        }
        let pos = ev.position;
        for (pane, r) in &self.pane_hits {
            if r.contains(pos) {
                self.pane = *pane;
                if *pane == PlanReviewPane::Diff {
                    self.show_version_diff = true;
                }
                return PlanReviewOutcome::PaneChanged(*pane);
            }
        }
        for (action, r) in &self.action_regions {
            if r.contains(pos) {
                self.action_cursor = *action;
                // Click confirms action (explicit)
                return self.confirm_action();
            }
        }
        PlanReviewOutcome::Ignored
    }
}

#[derive(Clone, Copy)]
enum TextTarget {
    Feedback,
    Conditions,
    Comment,
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Plan review painter.
#[derive(Debug, Clone, Copy)]
pub struct PlanReview<'a> {
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
}

impl<'a> PlanReview<'a> {
    /// System only — plan lives in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            ascii: false,
            colorless: false,
        }
    }

    /// ASCII glyphs.
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
    ///
    /// Temporarily takes `state.plan` so content paint can hold `&PlanDocument`
    /// while recording mutable hit regions — plan is always restored.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut PlanReviewState) {
        state.action_regions.clear();
        state.pane_hits.clear();
        if area.is_empty() {
            return;
        }
        let Some(plan) = state.plan.take() else {
            let panel = Panel::new(self.system)
                .title("Plan")
                .emphasis(PanelChrome::Normal);
            let inner = panel.inner(area);
            use ratatui_core::widgets::Widget;
            Widget::render(&panel, area, buffer);
            if !inner.is_empty() {
                let m = if self.ascii { "[ ] no plan" } else { "∅ no plan" };
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    m,
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
            return;
        };

        let risk = plan.risk;
        let title = format!(
            "Plan v{} · {} · {}",
            plan.version,
            plan.title,
            risk.label()
        );
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
            state.plan = Some(plan);
            return;
        }

        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom();
        let has_previous = plan.previous.is_some();
        let can_approve = plan.can_approve();

        // Summary + risk
        if let Some(sum) = plan.summary.as_ref() {
            if y < max_y {
                let style = if self.colorless {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(risk.role())
                };
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(sum, w),
                    w,
                    style,
                );
                y = y.saturating_add(1);
            }
        }

        // Pane tabs
        if y < max_y {
            y = self.paint_pane_tabs(inner.x, y, w, buffer, state, has_previous);
        }

        // Phase banner
        if !matches!(state.phase, PlanReviewPhase::Review) && y < max_y {
            let label = match state.phase {
                PlanReviewPhase::Feedback => "feedback › ",
                PlanReviewPhase::Conditions => "conditions › ",
                PlanReviewPhase::Comment => "comment › ",
                PlanReviewPhase::Review => "",
            };
            let draft = match state.phase {
                PlanReviewPhase::Feedback => state.draft_feedback.as_str(),
                PlanReviewPhase::Conditions => state.draft_conditions.as_str(),
                PlanReviewPhase::Comment => state.draft_comment.as_str(),
                PlanReviewPhase::Review => "",
            };
            let line = format!("{label}{draft}▌");
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&line, w),
                w,
                self.system.style(Role::Accent),
            );
            y = y.saturating_add(1);
        }

        // Content region leaves room for actions + consequence
        let action_rows: u16 = 2;
        let content_bottom = max_y.saturating_sub(action_rows);
        let content = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: content_bottom.saturating_sub(y),
        };
        if !content.is_empty() {
            match state.pane {
                PlanReviewPane::Document => self.paint_document(content, buffer, state, &plan),
                PlanReviewPane::Tasks => self.paint_tasks(content, buffer, state, &plan),
                PlanReviewPane::Risks => self.paint_risks(content, buffer, state, &plan),
                PlanReviewPane::Files => self.paint_files(content, buffer, state, &plan),
                PlanReviewPane::Comments => self.paint_comments(content, buffer, state),
                PlanReviewPane::Diff => self.paint_diff(content, buffer, state, &plan),
            }
        }

        // Actions + consequence
        if max_y > inner.y {
            let action_y = max_y.saturating_sub(2);
            self.paint_actions(inner.x, action_y, w, buffer, state, can_approve);
            let cons_y = max_y.saturating_sub(1);
            let cons = state.action_cursor.consequence();
            let line = format!("→ {cons}");
            let style = if state.action_cursor.grants() && risk.is_destructive() {
                self.system.style(Role::Danger)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                inner.x,
                cons_y,
                take_display_cols(&line, w),
                w,
                style,
            );
        }

        state.plan = Some(plan);
    }

    fn paint_pane_tabs(
        &self,
        x: u16,
        y: u16,
        w: usize,
        buffer: &mut Buffer,
        state: &mut PlanReviewState,
        has_previous: bool,
    ) -> u16 {
        let mut col = x;
        let end = x.saturating_add(w as u16);
        for pane in PlanReviewPane::cycle() {
            if *pane == PlanReviewPane::Diff && !has_previous {
                continue;
            }
            let label = pane.label();
            let selected = state.pane == *pane;
            let text = if selected {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            };
            let tw = display_cols(&text) as u16;
            if col.saturating_add(tw) > end {
                break;
            }
            let style = if selected {
                self.system
                    .style(Role::Accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(col, y, &text, usize::from(tw), style);
            state.pane_hits.push((
                *pane,
                Rect {
                    x: col,
                    y,
                    width: tw,
                    height: 1,
                },
            ));
            col = col.saturating_add(tw.saturating_add(1));
        }
        y.saturating_add(1)
    }

    fn paint_document(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &PlanReviewState,
        plan: &PlanDocument,
    ) {
        let lines = plan.body_lines();
        let w = usize::from(area.width);
        let (sel_start, sel_end) = state.selection_range();
        let mut y = area.y;
        let max_y = area.bottom();
        let start = state.scroll.min(lines.len());
        for (i, line) in lines.iter().enumerate().skip(start) {
            if y >= max_y {
                break;
            }
            let selected = i >= sel_start && i <= sel_end;
            let has_comment = state.comments.iter().any(|c| match &c.anchor {
                PlanCommentAnchor::Line { line } => *line == i,
                PlanCommentAnchor::Range { start, end } => i >= *start && i <= *end,
                _ => false,
            });
            let mark = if selected {
                if self.ascii { ">" } else { "›" }
            } else if has_comment {
                if self.ascii { "*" } else { "·" }
            } else {
                " "
            };
            let text = format!("{mark}{i:>3}│{line}");
            let style = if selected {
                self.system
                    .style(Role::Accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
            y = y.saturating_add(1);
        }
        if lines.is_empty() && y < max_y {
            // Fall back to sections
            for (si, sec) in plan.sections.iter().enumerate() {
                if y >= max_y {
                    break;
                }
                let selected = si == state.selected_section;
                let mark = if selected { "›" } else { " " };
                let text = format!("{mark}# {} — {}", sec.title, sec.body);
                let style = if selected {
                    self.system.style(Role::Accent)
                } else {
                    self.system.style(Role::Text)
                };
                buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
                y = y.saturating_add(1);
            }
        }
    }

    fn paint_tasks(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &PlanReviewState,
        plan: &PlanDocument,
    ) {
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        for (i, task) in plan.tasks.iter().enumerate() {
            if y >= max_y {
                break;
            }
            let selected = i == state.selected_task;
            let g = if self.ascii {
                match task.status {
                    PlanTaskStatus::Done => "[x]",
                    PlanTaskStatus::Blocked => "[!]",
                    PlanTaskStatus::InProgress => "[>]",
                    PlanTaskStatus::Pending => "[ ]",
                }
            } else {
                task.status.glyph()
            };
            let detail = task
                .detail
                .as_ref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default();
            let text = format!(
                "{}{} {}{}",
                if selected { "›" } else { " " },
                g,
                task.title,
                detail
            );
            let style = if selected {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
            y = y.saturating_add(1);
        }
        if plan.tasks.is_empty() && y < max_y {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("(no tasks)", w),
                w,
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_risks(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &PlanReviewState,
        plan: &PlanDocument,
    ) {
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        if y < max_y {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("Risks", w),
                w,
                self.system.style(Role::Warning),
            );
            y = y.saturating_add(1);
        }
        for (i, risk) in plan.risks.iter().enumerate() {
            if y >= max_y {
                break;
            }
            let selected = i == state.selected_risk;
            let text = format!(
                "{}[{}] {}",
                if selected { "›" } else { " " },
                risk.severity.glyph(),
                risk.text
            );
            let style = if selected {
                self.system.style(Role::Accent)
            } else if self.colorless {
                self.system.style(Role::Text)
            } else {
                self.system.style(risk.severity.role())
            };
            buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
            y = y.saturating_add(1);
        }
        if y < max_y {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("Assumptions", w),
                w,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }
        for a in &plan.assumptions {
            if y >= max_y {
                break;
            }
            let text = format!(" · {}", a.text);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&text, w),
                w,
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }
        if !plan.source_refs.is_empty() && y < max_y {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("Sources", w),
                w,
                self.system.style(Role::Info),
            );
            y = y.saturating_add(1);
            for s in &plan.source_refs {
                if y >= max_y {
                    break;
                }
                let loc = s
                    .location
                    .as_ref()
                    .map(|l| format!(" ({l})"))
                    .unwrap_or_default();
                let text = format!(" · {}{loc}", s.label);
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&text, w),
                    w,
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }
    }

    fn paint_files(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &PlanReviewState,
        plan: &PlanDocument,
    ) {
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        for (i, f) in plan.affected_files.iter().enumerate() {
            if y >= max_y {
                break;
            }
            let selected = i == state.selected_file;
            let rename = f
                .rename_to
                .as_ref()
                .map(|t| format!(" → {t}"))
                .unwrap_or_default();
            let text = format!(
                "{}{} {}{}",
                if selected { "›" } else { " " },
                f.change.glyph(),
                f.path,
                rename
            );
            let style = if selected {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
            y = y.saturating_add(1);
        }
        if plan.affected_files.is_empty() && y < max_y {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("(no files)", w),
                w,
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_comments(&self, area: Rect, buffer: &mut Buffer, state: &PlanReviewState) {
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        if state.comments.is_empty() {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("No comments · m to annotate", w),
                w,
                self.system.style(Role::TextMuted),
            );
            return;
        }
        for (i, c) in state.comments.iter().enumerate() {
            if y >= max_y {
                break;
            }
            let selected = i == state.selected_comment;
            let anchor = match &c.anchor {
                PlanCommentAnchor::Line { line } => format!("L{line}"),
                PlanCommentAnchor::Range { start, end } => format!("L{start}-{end}"),
                PlanCommentAnchor::Section { section_id } => format!("§{section_id}"),
                PlanCommentAnchor::Task { task_id } => format!("T{task_id}"),
                PlanCommentAnchor::File { path } => format!("F{path}"),
                PlanCommentAnchor::Orphan { reason } => format!("?{reason}"),
            };
            let orphan = if c.preserved { "" } else { " !" };
            let text = format!(
                "{}[{}] {}{} — {}",
                if selected { "›" } else { " " },
                anchor,
                c.body,
                orphan,
                c.created_at_version
            );
            let style = if !c.preserved {
                self.system.style(Role::Warning)
            } else if selected {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(area.x, y, take_display_cols(&text, w), w, style);
            y = y.saturating_add(1);
        }
    }

    fn paint_diff(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &PlanReviewState,
        plan: &PlanDocument,
    ) {
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let Some(prev) = plan.previous.as_ref() else {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols("No previous revision", w),
                w,
                self.system.style(Role::TextMuted),
            );
            return;
        };
        if y < max_y {
            let hdr = format!("v{} → v{}", prev.version, plan.version);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&hdr, w),
                w,
                self.system.style(Role::Info),
            );
            y = y.saturating_add(1);
        }
        // Task set diff
        let prev_tasks: BTreeMap<&str, &str> = prev
            .tasks
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
            .collect();
        let cur_tasks: BTreeMap<&str, &str> = plan
            .tasks
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
            .collect();
        let mut rows: Vec<String> = Vec::new();
        for (id, title) in &cur_tasks {
            if !prev_tasks.contains_key(id) {
                rows.push(format!("+ task {id}: {title}"));
            } else if prev_tasks.get(id) != Some(title) {
                rows.push(format!("~ task {id}: {title}"));
            }
        }
        for (id, title) in &prev_tasks {
            if !cur_tasks.contains_key(id) {
                rows.push(format!("- task {id}: {title}"));
            }
        }
        // File set diff
        let prev_files: BTreeMap<&str, PlanFileChange> = prev
            .affected_files
            .iter()
            .map(|f| (f.path.as_str(), f.change))
            .collect();
        for f in &plan.affected_files {
            match prev_files.get(f.path.as_str()) {
                None => rows.push(format!("+ file {} {}", f.change.glyph(), f.path)),
                Some(c) if *c != f.change => {
                    rows.push(format!("~ file {} {}", f.change.glyph(), f.path));
                }
                _ => {}
            }
        }
        for (path, _) in &prev_files {
            if !plan.affected_files.iter().any(|f| f.path == *path) {
                rows.push(format!("- file {path}"));
            }
        }
        // Body line count delta
        let pl = prev.line_count();
        let cl = plan.line_count();
        if pl != cl {
            rows.push(format!("~ body lines {pl} → {cl}"));
        }
        if plan.summary != prev.summary {
            rows.push("~ summary changed".into());
        }
        if rows.is_empty() {
            rows.push("no structural changes".into());
        }
        let start = state.scroll.min(rows.len());
        for row in rows.iter().skip(start) {
            if y >= max_y {
                break;
            }
            let style = if row.starts_with('+') {
                self.system.style(Role::Success)
            } else if row.starts_with('-') {
                self.system.style(Role::Danger)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(area.x, y, take_display_cols(row, w), w, style);
            y = y.saturating_add(1);
        }
    }

    fn paint_actions(
        &self,
        x: u16,
        y: u16,
        w: usize,
        buffer: &mut Buffer,
        state: &mut PlanReviewState,
        can_approve: bool,
    ) {
        let actions = {
            let mut a = PlanAction::strip().to_vec();
            if !can_approve {
                a.retain(|x| !x.grants());
            }
            a
        };
        let mut col = x;
        let end = x.saturating_add(w as u16);
        for action in actions {
            let focused = state.action_cursor == action;
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
                if action.grants() {
                    self.system
                        .style(Role::Danger)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.system
                        .style(Role::Accent)
                        .add_modifier(Modifier::BOLD)
                }
            } else if action.grants() {
                self.system.style(Role::Warning)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(col, y, &text, usize::from(tw), style);
            state.action_regions.push((
                action,
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

impl StatefulWidget for &PlanReview<'_> {
    type State = PlanReviewState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for PlanReview<'_> {
    type State = PlanReviewState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo plan document.
#[must_use]
pub fn example_plan_document() -> PlanDocument {
    let body = "\
# Migrate auth module

## Steps
1. Extract token validation
2. Add session store
3. Update callers

## Notes
Preserves existing API surface.
";
    PlanDocument::new("plan-auth", 2, "Migrate auth", body, PermissionRisk::Medium)
        .summary("Refactor auth into module without breaking callers")
        .sections(vec![
            PlanSection::new("steps", "Steps", "Extract, store, update").lines(3, 7),
            PlanSection::new("notes", "Notes", "API preserved").lines(8, 10),
        ])
        .tasks(vec![
            PlanTask::new("t1", "Extract token validation")
                .detail("src/auth/token.rs")
                .status(PlanTaskStatus::Pending),
            PlanTask::new("t2", "Add session store").detail("src/auth/session.rs"),
            PlanTask::new("t3", "Update callers").status(PlanTaskStatus::Blocked),
        ])
        .risks(vec![PlanRiskItem::new(
            "r1",
            "Session store may drop in-flight tokens",
            PermissionRisk::Medium,
        )])
        .assumptions(vec![PlanAssumption::new(
            "a1",
            "Callers use public auth::validate only",
        )])
        .affected_files(vec![
            PlanAffectedFile::new("src/auth/token.rs", PlanFileChange::Create),
            PlanAffectedFile::new("src/auth/mod.rs", PlanFileChange::Modify),
            PlanAffectedFile::new("src/legacy_auth.rs", PlanFileChange::Delete),
        ])
        .source_refs(vec![PlanSourceRef::new("s1", "ADR-12 Auth")
            .location("docs/adr/12.md:14")])
        .previous(
            PlanDocument::new(
                "plan-auth",
                1,
                "Migrate auth",
                "# Migrate auth\n\n1. Extract\n",
                PermissionRisk::Low,
            )
            .tasks(vec![PlanTask::new("t1", "Extract tokens")])
            .affected_files(vec![PlanAffectedFile::new(
                "src/auth/token.rs",
                PlanFileChange::Create,
            )]),
        )
}

/// High-risk plan for safe-focus demos.
#[must_use]
pub fn example_high_risk_plan() -> PlanDocument {
    PlanDocument::new(
        "plan-drop",
        1,
        "Drop production tables",
        "# DROP\n\nDROP TABLE users;\n",
        PermissionRisk::Critical,
    )
    .summary("Irreversible data loss")
    .tasks(vec![PlanTask::new("t1", "Drop users table")])
    .risks(vec![PlanRiskItem::new(
        "r1",
        "No backup gate",
        PermissionRisk::Critical,
    )])
    .affected_files(vec![PlanAffectedFile::new(
        "db/users",
        PlanFileChange::Delete,
    )])
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

    #[test]
    fn default_focus_never_grants() {
        for risk in [
            PermissionRisk::Low,
            PermissionRisk::Medium,
            PermissionRisk::High,
            PermissionRisk::Critical,
        ] {
            let focus = PlanAction::default_for_risk(risk);
            assert!(
                !focus.grants(),
                "risk {risk:?} default {:?} grants",
                focus
            );
        }
    }

    #[test]
    fn open_medium_focuses_revision() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        assert_eq!(st.action_cursor(), PlanAction::RequestRevision);
        assert!(!st.action_cursor().grants());
    }

    #[test]
    fn open_high_focuses_abandon() {
        let mut st = PlanReviewState::new();
        st.open(example_high_risk_plan());
        assert_eq!(st.action_cursor(), PlanAction::Abandon);
    }

    #[test]
    fn bare_a_only_focuses_not_approve() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let out = st.handle_key(press(KeyCode::Char('a')));
        assert!(matches!(
            out,
            PlanReviewOutcome::ActionFocused(PlanAction::Approve)
        ));
        assert!(st.is_open(), "plan still open — not approved");
        // Enter would approve after focus move
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, PlanReviewOutcome::Approved));
        assert!(!st.is_open());
    }

    #[test]
    fn y_unbound() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            PlanReviewOutcome::Ignored
        ));
        assert!(st.is_open());
    }

    #[test]
    fn esc_cancels_not_abandon() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        assert!(matches!(
            st.handle_key(press(KeyCode::Esc)),
            PlanReviewOutcome::Cancelled
        ));
        assert!(st.is_open(), "cancel keeps plan; host peels overlay");
    }

    #[test]
    fn enter_default_revision_opens_feedback() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        assert_eq!(st.action_cursor(), PlanAction::RequestRevision);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Feedback)
        ));
        for c in "need tests".chars() {
            let _ = st.handle_key(press(KeyCode::Char(c)));
        }
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PlanReviewOutcome::RevisionRequested { feedback } if feedback == "need tests"
        ));
    }

    #[test]
    fn approve_with_conditions() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let _ = st.handle_key(press(KeyCode::Char('c')));
        assert_eq!(st.action_cursor(), PlanAction::ApproveWithConditions);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PlanReviewOutcome::PhaseChanged(PlanReviewPhase::Conditions)
        ));
        for c in "behind flag".chars() {
            let _ = st.handle_key(press(KeyCode::Char(c)));
        }
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PlanReviewOutcome::ApprovedWithConditions { conditions }
                if conditions == "behind flag"
        ));
    }

    #[test]
    fn abandon_via_enter() {
        let mut st = PlanReviewState::new();
        st.open(example_high_risk_plan());
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            PlanReviewOutcome::Abandoned
        ));
        assert!(!st.is_open());
    }

    #[test]
    fn comment_on_line_and_preserve_on_update() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        // select line 1
        let _ = st.handle_key(press(KeyCode::Down));
        let _ = st.handle_key(press(KeyCode::Char('m')));
        for c in "check this".chars() {
            let _ = st.handle_key(press(KeyCode::Char(c)));
        }
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, PlanReviewOutcome::CommentAdded(_)));
        assert_eq!(st.comments.len(), 1);
        assert!(st.comments[0].preserved);

        // Update plan — same body lines keep line comment; section task anchors too
        let mut next = example_plan_document();
        next.version = 3;
        next.markdown_body.push_str("\nextra line\n");
        let out = st.update_plan(next);
        assert!(matches!(
            out,
            PlanReviewOutcome::PlanUpdated {
                preserved_comments: 1,
                orphaned_comments: 0,
                ..
            }
        ));
        assert!(st.comments[0].preserved);
    }

    #[test]
    fn section_anchor_survives_body_rewrite() {
        let plan = example_plan_document();
        let comments = vec![PlanComment::new(
            "c1",
            "section note",
            PlanCommentAnchor::Section {
                section_id: "steps".into(),
            },
            1,
        )];
        let mut rewritten = plan.clone();
        rewritten.markdown_body = "# totally new body\n".into();
        rewritten.version = 9;
        let remapped = remap_plan_comments(&comments, &rewritten);
        assert!(remapped[0].preserved);
        assert!(matches!(
            remapped[0].anchor,
            PlanCommentAnchor::Section { .. }
        ));
    }

    #[test]
    fn missing_section_orphans_comment() {
        let plan = example_plan_document();
        let comments = vec![PlanComment::new(
            "c1",
            "gone",
            PlanCommentAnchor::Section {
                section_id: "missing".into(),
            },
            1,
        )];
        let remapped = remap_plan_comments(&comments, &plan);
        assert!(!remapped[0].preserved);
        assert!(remapped[0].anchor.is_orphan());
    }

    #[test]
    fn empty_plan_cannot_approve() {
        let mut st = PlanReviewState::new();
        st.open(PlanDocument::new("e", 1, "Empty", "", PermissionRisk::Low));
        assert!(!st.plan.as_ref().unwrap().can_approve());
        let _ = st.handle_key(press(KeyCode::Char('a')));
        // Approve not available
        assert!(!st.action_cursor().grants() || !st.available_actions().contains(&PlanAction::Approve));
        st.action_cursor = PlanAction::Approve;
        st.clamp_action_cursor();
        assert!(!st.action_cursor().grants());
    }

    #[test]
    fn risk_upgrade_snaps_off_approve() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        st.action_cursor = PlanAction::Approve;
        let mut next = example_high_risk_plan();
        next.id = "plan-auth".into();
        next.version = 4;
        let _ = st.update_plan(next);
        assert!(!st.action_cursor().grants());
        assert_eq!(st.action_cursor(), PlanAction::Abandon);
    }

    #[test]
    fn version_diff_toggle() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let out = st.handle_key(press(KeyCode::Char('d')));
        assert!(matches!(
            out,
            PlanReviewOutcome::VersionDiffToggled { show: true }
        ));
        assert_eq!(st.pane, PlanReviewPane::Diff);
    }

    #[test]
    fn pane_tab_cycle() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let out = st.handle_key(press(KeyCode::Tab));
        assert!(matches!(
            out,
            PlanReviewOutcome::PaneChanged(PlanReviewPane::Tasks)
        ));
    }

    #[test]
    fn task_nav() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        st.pane = PlanReviewPane::Tasks;
        let out = st.handle_key(press(KeyCode::Down));
        assert!(matches!(
            out,
            PlanReviewOutcome::TaskSelected { id } if id == "t2"
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            PlanReviewOutcome::Ignored
        ));
    }

    #[test]
    fn paint_all_panes() {
        let system = DesignSystem::default();
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let area = Rect::new(0, 0, 56, 18);
        let mut buf = Buffer::empty(area);
        for pane in PlanReviewPane::cycle() {
            st.pane = *pane;
            PlanReview::new(&system).paint(area, &mut buf, &mut st);
            assert!(st.is_open(), "paint restores plan after pane {:?}", pane);
        }
        PlanReview::new(&system).ascii(true).colorless(true).paint(area, &mut buf, &mut st);
        assert!(st.is_open());
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            PlanReview::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn no_process_policy() {
        let src = include_str!("plan_review.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "workflow::", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
        assert!(body.contains("grants"));
        assert!(body.contains("never"));
    }

    #[test]
    fn selection_range_extend() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        let _ = st.handle_key(press(KeyCode::Char('J')));
        let (s, e) = st.selection_range();
        assert!(e >= s);
    }

    #[test]
    fn fullscreen_request() {
        let mut st = PlanReviewState::new();
        st.open(example_plan_document());
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('f'))),
            PlanReviewOutcome::FullscreenRequested
        ));
    }

    #[test]
    fn fuzz_actions_and_panes() {
        for a in PlanAction::strip() {
            assert!(!a.id().is_empty());
            assert!(!a.label().is_empty());
            assert!(!a.consequence().is_empty());
        }
        for p in PlanReviewPane::cycle() {
            assert!(!p.id().is_empty());
        }
        for c in [
            PlanFileChange::Create,
            PlanFileChange::Modify,
            PlanFileChange::Delete,
            PlanFileChange::Rename,
        ] {
            assert!(!c.id().is_empty());
        }
    }

    #[test]
    fn mouse_action_hit() {
        let system = DesignSystem::default();
        let mut st = PlanReviewState::new();
        st.open(example_high_risk_plan());
        let area = Rect::new(0, 0, 64, 14);
        let mut buf = Buffer::empty(area);
        PlanReview::new(&system).paint(area, &mut buf, &mut st);
        assert!(!st.action_regions.is_empty());
        let (action, r) = st.action_regions[0];
        assert_eq!(action, PlanAction::Abandon);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        };
        let out = st.handle_mouse(ev);
        assert!(matches!(out, PlanReviewOutcome::Abandoned));
    }

    #[test]
    fn unicode_title_paint() {
        let system = DesignSystem::default();
        let mut st = PlanReviewState::new();
        st.open(
            PlanDocument::new(
                "u",
                1,
                "计划 🚀",
                "# 你好\n\n- 步骤一\n",
                PermissionRisk::Low,
            )
            .tasks(vec![PlanTask::new("t1", "步骤")]),
        );
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        PlanReview::new(&system).paint(area, &mut buf, &mut st);
    }
}
