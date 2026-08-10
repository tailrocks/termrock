// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! DiffReview — interactive hunk review veneer over [`super::diff::DiffView`].
//!
//! Prefer [`super::DiffView`] for pure read-only viewing. DiffReview keeps the
//! activate/stage-oriented API surface used by product review flows.

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyEvent, MouseEvent},
    interaction::UiIntent,
    style::DesignSystem,
    widgets::diff::{
        DiffHunk, DiffLine, DiffMode, DiffView, DiffViewOutcome, DiffViewState,
    },
};

/// Review outcomes (activate/stage/open stay consumer-owned).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffReviewOutcome {
    /// No change.
    Ignored,
    /// Hunk cursor moved.
    HunkCursorMoved {
        /// Hunk index.
        index: usize,
    },
    /// Activate hunk (stage/copy/open).
    HunkActivated {
        /// Hunk index.
        index: usize,
    },
    /// Viewport scrolled.
    Scrolled {
        /// Offset.
        offset: u16,
    },
    /// Split/unified preference flip.
    ToggleMode,
    /// Cursor line moved.
    CursorMoved {
        /// Filtered index.
        index: usize,
    },
    /// Search changed.
    SearchChanged(String),
    /// Fold toggled.
    FoldToggled {
        /// Id.
        id: String,
        /// Folded after.
        folded: bool,
    },
    /// Cancelled.
    Cancelled,
    /// File navigated.
    FileNavigated {
        /// File id.
        id: String,
    },
}

impl From<DiffViewOutcome> for DiffReviewOutcome {
    fn from(value: DiffViewOutcome) -> Self {
        match value {
            DiffViewOutcome::Ignored => Self::Ignored,
            DiffViewOutcome::Scrolled { offset } => Self::Scrolled { offset },
            DiffViewOutcome::CursorMoved { index } => Self::CursorMoved { index },
            DiffViewOutcome::HunkCursorMoved { index } => Self::HunkCursorMoved { index },
            DiffViewOutcome::HunkActivated { index } => Self::HunkActivated { index },
            DiffViewOutcome::FileNavigated { id } => Self::FileNavigated { id },
            DiffViewOutcome::ModeChanged(_) => Self::ToggleMode,
            DiffViewOutcome::SearchChanged(q) => Self::SearchChanged(q),
            DiffViewOutcome::FoldToggled { id, folded } => Self::FoldToggled { id, folded },
            DiffViewOutcome::Cancelled => Self::Cancelled,
        }
    }
}

/// Review state — thin wrapper over [`DiffViewState`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffReviewState {
    /// Shared viewer state.
    pub view: DiffViewState,
}

impl DiffReviewState {
    /// Fresh review (Auto mode).
    #[must_use]
    pub fn new() -> Self {
        Self {
            view: DiffViewState::new(),
        }
    }

    /// Hunk cursor.
    #[must_use]
    pub const fn hunk_cursor(&self) -> usize {
        self.view.hunk_cursor
    }

    /// Deprecated name for [`Self::hunk_cursor`].
    #[deprecated(note = "use hunk_cursor")]
    #[must_use]
    pub const fn hunk_index(&self) -> usize {
        self.view.hunk_cursor
    }

    /// Programmatic hunk cursor.
    pub fn set_hunk_cursor(&mut self, index: usize) {
        self.view.hunk_cursor = index;
    }

    /// Vertical offset.
    #[must_use]
    pub fn offset_y(&self) -> u16 {
        self.view.offset()
    }

    /// Whether mode is currently Split (not Auto/Unified).
    #[must_use]
    pub const fn is_split(&self) -> bool {
        matches!(self.view.mode, DiffMode::Split)
    }

    /// Whether mode preference is split (including Auto preferring split when wide).
    #[must_use]
    pub const fn prefers_split(&self) -> bool {
        self.view.prefers_split()
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.view.set_accepts_input(accepts);
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.view.accepts_input()
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        // Review callers historically omit lines — scroll/hunk-only path.
        self.view
            .handle_key(key, &[], hunks)
            .into()
    }

    /// Keys with full line projection (preferred).
    pub fn handle_key_lines(
        &mut self,
        key: KeyEvent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffReviewOutcome {
        self.view.handle_key(key, lines, hunks).into()
    }

    /// Intent.
    pub fn handle_intent(&mut self, intent: UiIntent, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        self.view.handle_intent(intent, &[], hunks).into()
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        hunks: &[DiffHunk],
        _line_count: usize,
    ) -> DiffReviewOutcome {
        self.view.handle_mouse(event, &[], hunks).into()
    }

    /// Mouse with lines.
    pub fn handle_mouse_lines(
        &mut self,
        event: MouseEvent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffReviewOutcome {
        self.view.handle_mouse(event, lines, hunks).into()
    }
}

/// Interactive review chrome over [`DiffView`].
#[derive(Debug, Clone)]
pub struct DiffReview<'a> {
    lines: &'a [DiffLine<'a>],
    hunks: &'a [DiffHunk],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
    title: Option<&'a str>,
}

impl<'a> DiffReview<'a> {
    /// Lines + design system.
    #[must_use]
    pub const fn new(lines: &'a [DiffLine<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            hunks: &[],
            system,
            focused: true,
            ascii: false,
            colorless: false,
            title: None,
        }
    }

    /// Hunks.
    #[must_use]
    pub const fn hunks(mut self, hunks: &'a [DiffHunk]) -> Self {
        self.hunks = hunks;
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
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint via DiffView.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut DiffReviewState) {
        // Prefer split when review toggled split via `s` (maps to DiffMode cycle).
        let mut view = DiffView::new(self.lines, self.system)
            .hunks(self.hunks)
            .focused(self.focused)
            .ascii(self.ascii)
            .colorless(self.colorless);
        if let Some(t) = self.title {
            view = view.title(t);
        }
        view.render(area, buffer, &mut state.view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyEvent, KeyModifiers};
    use crate::widgets::diff::{DiffKind, DiffLine};

    fn sample_hunks() -> [DiffHunk; 3] {
        [
            DiffHunk::new(0, 3, "@@ -1,3 +1,3 @@").id("h0"),
            DiffHunk::new(3, 2, "@@ -10,2 +10,2 @@").id("h1"),
            DiffHunk::new(5, 3, "@@ -20,3 +20,4 @@").id("h2"),
        ]
    }

    fn sample_lines() -> [DiffLine<'static>; 8] {
        [
            DiffLine::hunk_header("0", "@@ -1,3 +1,3 @@").hunk_id("h0"),
            DiffLine::context("1", "context").hunk_id("h0"),
            DiffLine::removed("2", "old").hunk_id("h0"),
            DiffLine::added("3", "new 東京").hunk_id("h0"),
            DiffLine::hunk_header("4", "@@ -10,2 +10,2 @@").hunk_id("h1"),
            DiffLine::removed("5", "gone").hunk_id("h1"),
            DiffLine::hunk_header("6", "@@ -20,3 +20,4 @@").hunk_id("h2"),
            DiffLine::added("7", "ready 🧪").hunk_id("h2"),
        ]
    }

    #[test]
    fn diff_hunk_cursor_moved() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::HunkCursorMoved { index: 1 }
        ));
        assert_eq!(state.hunk_cursor(), 1);
    }

    #[test]
    fn diff_activate_toggle_and_scroll() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &hunks),
            DiffReviewOutcome::HunkActivated { index: 0 }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::ToggleMode
        ));
        assert!(state.prefers_split());
    }

    #[test]
    fn accepts_input_gate() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::Ignored
        ));
    }

    #[test]
    fn paint_via_diff_view() {
        let system = DesignSystem::default();
        let lines = sample_lines();
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        let area = Rect::new(0, 0, 48, 10);
        let mut buf = Buffer::empty(area);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .render(area, &mut buf, &mut state);
        assert!(!state.view.regions.is_empty());
        let _ = DiffKind::Added;
    }
}
