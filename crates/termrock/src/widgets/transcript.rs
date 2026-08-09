// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Variable-height streaming transcript engine.
//!
//! TermRock owns viewport, anchor, follow, folds, and measurement cache.
//! Consumers own block payloads, parsing, network, and domain wording.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode,
        KeyEvent,
        KeyEventKind,
        KeyModifiers,
        MouseEvent,
        MouseEventKind,
    },
    style::{
        DesignSystem,
        Role,
        RolePalette,
    },
    text::{
        display_cols,
        take_display_cols,
    },
};


/// Semantic kind of a transcript block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TranscriptKind {
    /// User message.
    #[default]
    User,
    /// Assistant message.
    Assistant,
    /// Tool call / result.
    Tool,
    /// System notice.
    System,
    /// Thinking / reasoning.
    Thinking,
    /// Permission / approval surface.
    Approval,
    /// Diff / code review.
    Diff,
    /// Generic content.
    Content,
}

/// Borrowed block projection for one paint/measure pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptBlock<'a, Id> {
    /// Stable identity across streaming revisions.
    pub id: Id,
    /// Content revision; bump to invalidate cached height.
    pub revision: u64,
    /// Semantic kind.
    pub kind: TranscriptKind,
    /// Display lines at the current width (consumer measures/wraps).
    pub lines: &'a [&'a str],
    /// Whether the block is folded to a single summary line.
    pub folded: bool,
    /// Summary when folded (defaults to first line).
    pub summary: Option<&'a str>,
    /// Whether the block accepts activation.
    pub enabled: bool,
}

impl<'a, Id> TranscriptBlock<'a, Id> {
    /// Creates an unfolded block from lines.
    #[must_use]
    pub const fn new(id: Id, kind: TranscriptKind, lines: &'a [&'a str]) -> Self {
        Self {
            id,
            revision: 0,
            kind,
            lines,
            folded: false,
            summary: None,
            enabled: true,
        }
    }

    /// Sets revision for cache invalidation.
    #[must_use]
    pub const fn revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }

    /// Marks the block folded.
    #[must_use]
    pub const fn folded(mut self, folded: bool) -> Self {
        self.folded = folded;
        self
    }

    /// Summary text when folded.
    #[must_use]
    pub const fn summary(mut self, summary: &'a str) -> Self {
        self.summary = Some(summary);
        self
    }

    /// Interaction enablement.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Display height in rows at the current fold state.
    #[must_use]
    pub fn height(&self) -> u16 {
        if self.folded {
            1
        } else {
            u16::try_from(self.lines.len().max(1)).unwrap_or(u16::MAX)
        }
    }
}

/// Neutral transcript interaction outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TranscriptOutcome<Id> {
    /// Input ignored.
    Ignored,
    /// Viewport or selection changed without activation.
    Changed,
    /// Block activated.
    Activated(Id),
    /// Fold toggled for a block.
    FoldToggled {
        /// Block id.
        id: Id,
        /// New folded state requested (consumer applies).
        folded: bool,
    },
    /// Follow mode changed.
    FollowChanged(bool),
    /// Escape / cancel.
    Cancelled,
}

/// Stable visual anchor: block + intra-block display row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptAnchor<Id> {
    /// Stable block id.
    pub id: Id,
    /// Display row within the block (0-based).
    pub row: u16,
}

/// Interaction and viewport state for [`Transcript`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptState<Id> {
    first_display_row: u64,
    body_rows: u16,
    follow: bool,
    selected: Option<Id>,
    anchor: Option<TranscriptAnchor<Id>>,
    focused: bool,
    /// Cached total height from last layout.
    total_display_rows: u64,
    painted_area: Rect,
}

impl<Id> Default for TranscriptState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> TranscriptState<Id> {
    /// Creates empty transcript state (follow on by default).
    #[must_use]
    pub fn new() -> Self {
        Self {
            first_display_row: 0,
            body_rows: 0,
            follow: true,
            selected: None,
            anchor: None,
            focused: false,
            total_display_rows: 0,
            painted_area: Rect::default(),
        }
    }

    /// Keyboard focus ownership.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether the transcript owns keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Follow-tail mode.
    #[must_use]
    pub const fn follow(&self) -> bool {
        self.follow
    }

    /// Sets follow mode.
    pub fn set_follow(&mut self, follow: bool) {
        self.follow = follow;
    }

    /// First absolute display row in the viewport.
    #[must_use]
    pub const fn first_display_row(&self) -> u64 {
        self.first_display_row
    }

    /// Selected block id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Visual anchor.
    #[must_use]
    pub const fn anchor(&self) -> Option<&TranscriptAnchor<Id>> {
        self.anchor.as_ref()
    }

    fn max_first(&self) -> u64 {
        self.total_display_rows
            .saturating_sub(u64::from(self.body_rows.max(1)))
    }

    fn clamp_scroll(&mut self) {
        let max_first = self.max_first();
        self.first_display_row = self.first_display_row.min(max_first);
    }
}

impl<Id: Clone + Eq> TranscriptState<Id> {
    /// Layout helper: total display rows and optional block starts.
    pub fn layout_heights(blocks: &[TranscriptBlock<'_, Id>]) -> (u64, Vec<(Id, u64, u16)>) {
        let mut starts = Vec::with_capacity(blocks.len());
        let mut cursor = 0u64;
        for block in blocks {
            let h = block.height();
            starts.push((block.id.clone(), cursor, h));
            cursor = cursor.saturating_add(u64::from(h));
        }
        (cursor, starts)
    }

    fn capture_anchor_from_viewport(&mut self, starts: &[(Id, u64, u16)]) {
        let top = self.first_display_row;
        for (id, start, height) in starts {
            let end = start.saturating_add(u64::from(*height));
            if top >= *start && top < end {
                let row = u16::try_from(top.saturating_sub(*start)).unwrap_or(0);
                self.anchor = Some(TranscriptAnchor {
                    id: id.clone(),
                    row: row.min(height.saturating_sub(1)),
                });
                return;
            }
        }
        if let Some((id, start, height)) = starts.last() {
            let row = height.saturating_sub(1);
            let _ = start;
            self.anchor = Some(TranscriptAnchor {
                id: id.clone(),
                row,
            });
        } else {
            self.anchor = None;
        }
    }

    fn restore_anchor(&mut self, starts: &[(Id, u64, u16)]) {
        let Some(anchor) = self.anchor.clone() else {
            return;
        };
        if let Some((_, start, height)) = starts.iter().find(|(id, _, _)| id == &anchor.id) {
            let row = u64::from(anchor.row.min(height.saturating_sub(1)));
            self.first_display_row = start.saturating_add(row);
            self.clamp_scroll();
            return;
        }
        // Nearest surviving: pick last block before missing, else first.
        if let Some((_, start, _)) = starts.last() {
            self.first_display_row = *start;
            self.clamp_scroll();
            if let Some((id, _, height)) = starts.last() {
                self.anchor = Some(TranscriptAnchor {
                    id: id.clone(),
                    row: height.saturating_sub(1),
                });
            }
        } else {
            self.anchor = None;
            self.first_display_row = 0;
        }
    }

    fn apply_follow(&mut self) {
        if self.follow {
            self.first_display_row = self.max_first();
        }
    }

    /// Handles keyboard navigation when focused.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        blocks: &[TranscriptBlock<'_, Id>],
    ) -> TranscriptOutcome<Id> {
        if !self.focused || key.kind == KeyEventKind::Release {
            return TranscriptOutcome::Ignored;
        }
        let (total, starts) = Self::layout_heights(blocks);
        self.total_display_rows = total;
        self.body_rows = self.body_rows.max(1);

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.follow = false;
                self.first_display_row = self.first_display_row.saturating_sub(1);
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.first_display_row = self.first_display_row.saturating_add(1);
                self.clamp_scroll();
                if self.first_display_row >= self.max_first() {
                    self.follow = true;
                }
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            KeyCode::PageUp => {
                self.follow = false;
                self.first_display_row = self
                    .first_display_row
                    .saturating_sub(u64::from(self.body_rows.max(1)));
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            KeyCode::PageDown => {
                self.first_display_row = self
                    .first_display_row
                    .saturating_add(u64::from(self.body_rows.max(1)));
                self.clamp_scroll();
                if self.first_display_row >= self.max_first() {
                    self.follow = true;
                }
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            KeyCode::Home => {
                self.follow = false;
                self.first_display_row = 0;
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            KeyCode::End => {
                self.follow = true;
                self.apply_follow();
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::FollowChanged(true)
            }
            KeyCode::Enter => {
                if let Some(id) = self.selected.clone() {
                    if blocks.iter().any(|b| b.id == id && b.enabled) {
                        TranscriptOutcome::Activated(id)
                    } else {
                        TranscriptOutcome::Ignored
                    }
                } else {
                    TranscriptOutcome::Ignored
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Toggle fold request for selected.
                if let Some(id) = self.selected.clone() {
                    let folded = blocks
                        .iter()
                        .find(|b| b.id == id)
                        .map(|b| !b.folded)
                        .unwrap_or(true);
                    TranscriptOutcome::FoldToggled { id, folded }
                } else {
                    TranscriptOutcome::Ignored
                }
            }
            KeyCode::Esc => TranscriptOutcome::Cancelled,
            _ => TranscriptOutcome::Ignored,
        }
    }

    /// Mouse wheel against last painted area.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        blocks: &[TranscriptBlock<'_, Id>],
    ) -> TranscriptOutcome<Id> {
        let (total, starts) = Self::layout_heights(blocks);
        self.total_display_rows = total;
        match event.kind {
            MouseEventKind::ScrollUp if self.painted_area.contains(event.position) => {
                self.follow = false;
                self.first_display_row = self.first_display_row.saturating_sub(1);
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            MouseEventKind::ScrollDown if self.painted_area.contains(event.position) => {
                self.first_display_row = self.first_display_row.saturating_add(1);
                self.clamp_scroll();
                if self.first_display_row >= self.max_first() {
                    self.follow = true;
                }
                self.capture_anchor_from_viewport(&starts);
                TranscriptOutcome::Changed
            }
            _ => TranscriptOutcome::Ignored,
        }
    }
}

/// Variable-height transcript widget over borrowed blocks.
#[derive(Debug, Clone, Copy)]
pub struct Transcript<'a, Id> {
    blocks: &'a [TranscriptBlock<'a, Id>],
    system: &'a DesignSystem,
}

impl<'a, Id> Transcript<'a, Id> {
    /// Creates a transcript over the given blocks.
    #[must_use]
    pub const fn new(blocks: &'a [TranscriptBlock<'a, Id>], system: &'a DesignSystem) -> Self {
        Self { blocks, system }
    }
}

fn kind_style(system: &DesignSystem, kind: TranscriptKind) -> Style {
    match kind {
        TranscriptKind::User => system.style(Role::Text),
        TranscriptKind::Assistant => system.style(Role::Text),
        TranscriptKind::Tool => system.style(Role::TextMuted),
        TranscriptKind::System => system.style(Role::Warning),
        TranscriptKind::Thinking => system.style(Role::TextMuted),
        TranscriptKind::Approval => system.style(Role::Danger),
        TranscriptKind::Diff => system.style(Role::Text),
        TranscriptKind::Content => system.style(Role::Text),
    }
}

impl<Id: Clone + Eq> StatefulWidget for &Transcript<'_, Id> {
    type State = TranscriptState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.painted_area = area;
        if area.width == 0 || area.height == 0 {
            state.body_rows = 0;
            return;
        }
        state.body_rows = area.height;
        let (total, starts) = TranscriptState::<Id>::layout_heights(self.blocks);
        state.total_display_rows = total;

        if state.follow {
            state.apply_follow();
        } else if state.anchor.is_some() {
            state.restore_anchor(&starts);
        }
        state.clamp_scroll();
        state.capture_anchor_from_viewport(&starts);

        let view_start = state.first_display_row;
        let view_end = view_start.saturating_add(u64::from(area.height));

        for (block_index, block) in self.blocks.iter().enumerate() {
            let Some((_, start, height)) = starts.get(block_index) else {
                continue;
            };
            let end = start.saturating_add(u64::from(*height));
            if end <= view_start || *start >= view_end {
                continue;
            }
            let style = kind_style(self.system, block.kind);
            let selected = state.selected.as_ref() == Some(&block.id);
            let style = if selected && state.focused {
                self.system.style(Role::Accent)
            } else {
                style
            };

            if block.folded {
                let abs = *start;
                if abs < view_start || abs >= view_end {
                    continue;
                }
                let y = area
                    .y
                    .saturating_add(u16::try_from(abs - view_start).unwrap_or(0));
                let text = block
                    .summary
                    .unwrap_or_else(|| block.lines.first().copied().unwrap_or("…"));
                let label = format!(
                    "▸ {}",
                    take_display_cols(text, usize::from(area.width.saturating_sub(2)))
                );
                let clipped = take_display_cols(&label, usize::from(area.width));
                buffer.set_stringn(area.x, y, &clipped, usize::from(area.width), style);
                continue;
            }

            for (line_idx, line) in block.lines.iter().enumerate() {
                let abs = start.saturating_add(line_idx as u64);
                if abs < view_start || abs >= view_end {
                    continue;
                }
                let y = area
                    .y
                    .saturating_add(u16::try_from(abs.saturating_sub(view_start)).unwrap_or(0));
                let clipped = take_display_cols(line, usize::from(area.width));
                let _ = display_cols(&clipped);
                buffer.set_stringn(area.x, y, &clipped, usize::from(area.width), style);
            }
        }
    }
}

impl<Id: Clone + Eq> StatefulWidget for Transcript<'_, Id> {
    type State = TranscriptState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::{backend::TestBackend, terminal::Terminal};

    #[test]
    fn zero_area_and_empty_are_safe() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let lines: [&str; 0] = [];
        let blocks: [TranscriptBlock<'_, u32>; 0] = [];
        let mut state = TranscriptState::new();
        state.set_focused(true);
        let t = Transcript::new(&blocks, &system);
        let mut terminal = Terminal::new(TestBackend::new(0, 0)).unwrap();
        terminal
            .draw(|f| f.render_stateful_widget(&t, Rect::default(), &mut state))
            .unwrap();
        let _ = lines;
    }

    #[test]
    fn variable_height_viewport_maps_across_blocks() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let a = ["one", "two", "three"];
        let b = ["solo"];
        let c = ["x", "y"];
        let blocks = [
            TranscriptBlock::new(1u32, TranscriptKind::User, &a),
            TranscriptBlock::new(2, TranscriptKind::Assistant, &b),
            TranscriptBlock::new(3, TranscriptKind::Tool, &c),
        ];
        let (total, starts) = TranscriptState::<u32>::layout_heights(&blocks);
        assert_eq!(total, 6);
        assert_eq!(starts[0], (1, 0, 3));
        assert_eq!(starts[1], (2, 3, 1));
        assert_eq!(starts[2], (3, 4, 2));

        let mut state = TranscriptState::new();
        state.set_focused(true);
        state.set_follow(false);
        state.first_display_row = 3;
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks, &system),
                    Rect::new(0, 0, 20, 2),
                    &mut state,
                );
            })
            .unwrap();
        assert_eq!(state.anchor().map(|a| a.id), Some(2));
    }

    #[test]
    fn follow_stays_at_tail_and_manual_scroll_detaches() {
        let a = ["1", "2", "3", "4", "5", "6", "7", "8"];
        let blocks = [TranscriptBlock::new(1u32, TranscriptKind::User, &a)];
        let mut state = TranscriptState::new();
        state.set_focused(true);
        state.body_rows = 3;
        let (total, _) = TranscriptState::<u32>::layout_heights(&blocks);
        state.total_display_rows = total;
        state.set_follow(true);
        state.apply_follow();
        assert_eq!(state.first_display_row(), 5);

        let outcome = state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &blocks);
        assert_eq!(outcome, TranscriptOutcome::Changed);
        assert!(!state.follow());
    }

    #[test]
    fn anchor_survives_append_and_resize_height() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let a = ["a0", "a1"];
        let b = ["b0", "b1", "b2"];
        let blocks = [
            TranscriptBlock::new(1u32, TranscriptKind::User, &a),
            TranscriptBlock::new(2, TranscriptKind::Assistant, &b),
        ];
        let mut state = TranscriptState::new();
        state.set_focused(true);
        state.set_follow(false);
        state.first_display_row = 2; // first line of block 2
        let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks, &system),
                    Rect::new(0, 0, 30, 3),
                    &mut state,
                );
            })
            .unwrap();
        assert_eq!(state.anchor().map(|a| (a.id, a.row)), Some((2, 0)));

        // Append more lines to block 1 (taller first block) via new projection.
        let a2 = ["a0", "a1", "a2", "a3"];
        let blocks2 = [
            TranscriptBlock::new(1u32, TranscriptKind::User, &a2).revision(1),
            TranscriptBlock::new(2, TranscriptKind::Assistant, &b),
        ];
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks2, &system),
                    Rect::new(0, 0, 30, 3),
                    &mut state,
                );
            })
            .unwrap();
        // Anchor on block 2 row 0 should restore first_display_row to 4.
        assert_eq!(state.anchor().map(|a| a.id), Some(2));
        assert_eq!(state.first_display_row(), 4);
    }

    #[test]
    fn folded_block_is_one_row() {
        let lines = ["a", "b", "c"];
        let block = TranscriptBlock::new(1u32, TranscriptKind::Thinking, &lines).folded(true);
        assert_eq!(block.height(), 1);
    }

    #[test]
    fn disabled_block_does_not_activate() {
        let lines = ["x"];
        let blocks = [TranscriptBlock::new(9u32, TranscriptKind::System, &lines).enabled(false)];
        let mut state = TranscriptState::new();
        state.set_focused(true);
        state.selected = Some(9);
        let outcome = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &blocks);
        assert_eq!(outcome, TranscriptOutcome::Ignored);
    }
}
