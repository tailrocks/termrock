// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Variable-height streaming transcript — **sole** agent conversation surface.
//!
//! TermRock owns viewport, anchor, follow, folds, kind chrome, and measurement.
//! Consumers own block payloads, parsing, network, and domain wording.
//!
//! **Break J:** the one-row `StreamView` paint shell was deleted; project
//! one-line turns into single-line [`TranscriptBlock`]s.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
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

impl TranscriptKind {
    /// Non-color prefix glyph (Unicode).
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::User => "› ",
            Self::Assistant => "◦ ",
            Self::Tool => "⚙ ",
            Self::System => "· ",
            Self::Thinking => "… ",
            Self::Approval => "! ",
            Self::Diff => "± ",
            Self::Content => "  ",
        }
    }

    /// ASCII-safe prefix (no combining / wide symbols).
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::User => "> ",
            Self::Assistant => "o ",
            Self::Tool => "# ",
            Self::System => ". ",
            Self::Thinking => "~ ",
            Self::Approval => "! ",
            Self::Diff => "+ ",
            Self::Content => "  ",
        }
    }

    /// Semantic role for the kind chrome.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::User => Role::TextStrong,
            Self::Assistant => Role::Text,
            Self::Tool => Role::Info,
            Self::System => Role::Warning,
            Self::Thinking => Role::TextMuted,
            Self::Approval => Role::Danger,
            Self::Diff => Role::Text,
            Self::Content => Role::Text,
        }
    }
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
///
/// `focused` / [`Self::set_focused`] control **accepts-input chrome** and
/// selection emphasis only. Hosts must not call [`Self::handle_key`] unless
/// this surface owns input — handlers do not early-return on focus (sole
/// authority: host / scene).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptState<Id> {
    first_display_row: u64,
    body_rows: u16,
    follow: bool,
    selected: Option<Id>,
    anchor: Option<TranscriptAnchor<Id>>,
    /// Accepts-input chrome (not keyboard authority).
    focused: bool,
    /// Cached total height from last layout.
    total_display_rows: u64,
    painted_area: Rect,
    /// Hit regions: (block id, painted row rect) from last paint.
    pub block_regions: Vec<(Id, Rect)>,
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
            block_regions: Vec::new(),
        }
    }

    /// Accepts-input chrome flag (host/scene owns focus authority).
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether accepts-input chrome is active.
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

    /// Sets selection by stable id.
    pub fn select(&mut self, id: Option<Id>) {
        self.selected = id;
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

    fn select_visible_block(
        &mut self,
        starts: &[(Id, u64, u16)],
        blocks: &[TranscriptBlock<'_, Id>],
    ) {
        let top = self.first_display_row;
        for (id, start, height) in starts {
            let end = start.saturating_add(u64::from(*height));
            if top >= *start && top < end {
                if blocks.iter().any(|b| &b.id == id && b.enabled) {
                    self.selected = Some(id.clone());
                }
                return;
            }
        }
    }

    fn move_selection(
        &mut self,
        blocks: &[TranscriptBlock<'_, Id>],
        delta: isize,
    ) -> TranscriptOutcome<Id> {
        let selectable: Vec<_> = blocks
            .iter()
            .filter(|b| b.enabled)
            .map(|b| b.id.clone())
            .collect();
        if selectable.is_empty() {
            return TranscriptOutcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|id| selectable.iter().position(|s| s == id));
        let next = match current {
            Some(i) => {
                let n = selectable.len() as isize;
                let idx = (i as isize + delta).rem_euclid(n) as usize;
                selectable[idx].clone()
            }
            None => {
                if delta >= 0 {
                    selectable[0].clone()
                } else {
                    selectable[selectable.len() - 1].clone()
                }
            }
        };
        if self.selected.as_ref() == Some(&next) {
            return TranscriptOutcome::Ignored;
        }
        self.selected = Some(next);
        // Ensure selected block is brought into view when possible.
        let (_, starts) = Self::layout_heights(blocks);
        if let Some(sel) = &self.selected
            && let Some((_, start, height)) = starts.iter().find(|(id, _, _)| id == sel)
        {
            let end = start.saturating_add(u64::from(*height));
            let view_end = self
                .first_display_row
                .saturating_add(u64::from(self.body_rows.max(1)));
            if *start < self.first_display_row {
                self.follow = false;
                self.first_display_row = *start;
            } else if end > view_end {
                self.follow = false;
                self.first_display_row = end.saturating_sub(u64::from(self.body_rows.max(1)));
                self.clamp_scroll();
            }
            self.capture_anchor_from_viewport(&starts);
        }
        TranscriptOutcome::Changed
    }

    /// Handles keyboard navigation. Host must gate ownership before calling.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        blocks: &[TranscriptBlock<'_, Id>],
    ) -> TranscriptOutcome<Id> {
        if key.kind == KeyEventKind::Release {
            return TranscriptOutcome::Ignored;
        }
        if let Some(intent) = crate::interaction::default_transcript_intent(key) {
            return self.handle_intent(intent, blocks);
        }
        // Product fold chord (Ctrl+F) — not in generic intent map.
        if matches!(key.code, KeyCode::Char('f' | 'F'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && key.kind == KeyEventKind::Press
        {
            if let Some(id) = self.selected.clone() {
                let folded = blocks
                    .iter()
                    .find(|b| b.id == id)
                    .map(|b| !b.folded)
                    .unwrap_or(true);
                return TranscriptOutcome::FoldToggled { id, folded };
            }
            return TranscriptOutcome::Ignored;
        }
        TranscriptOutcome::Ignored
    }

    /// Semantic intent routing (keymap-friendly).
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        blocks: &[TranscriptBlock<'_, Id>],
    ) -> TranscriptOutcome<Id> {
        let (total, starts) = Self::layout_heights(blocks);
        self.total_display_rows = total;
        self.body_rows = self.body_rows.max(1);

        match intent {
            UiIntent::Move(NavigationMove::Previous) => {
                // Prefer selection step when a selection exists; else scroll.
                if self.selected.is_some() {
                    return self.move_selection(blocks, -1);
                }
                self.follow = false;
                self.first_display_row = self.first_display_row.saturating_sub(1);
                self.capture_anchor_from_viewport(&starts);
                self.select_visible_block(&starts, blocks);
                TranscriptOutcome::Changed
            }
            UiIntent::Move(NavigationMove::Next) => {
                if self.selected.is_some() {
                    return self.move_selection(blocks, 1);
                }
                self.first_display_row = self.first_display_row.saturating_add(1);
                self.clamp_scroll();
                if self.first_display_row >= self.max_first() {
                    self.follow = true;
                }
                self.capture_anchor_from_viewport(&starts);
                self.select_visible_block(&starts, blocks);
                TranscriptOutcome::Changed
            }
            UiIntent::Move(NavigationMove::First) => {
                self.follow = false;
                self.first_display_row = 0;
                self.capture_anchor_from_viewport(&starts);
                self.select_visible_block(&starts, blocks);
                TranscriptOutcome::Changed
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.follow = true;
                self.apply_follow();
                self.capture_anchor_from_viewport(&starts);
                self.select_visible_block(&starts, blocks);
                TranscriptOutcome::FollowChanged(true)
            }
            UiIntent::Page(PageMove::Backward) => {
                self.follow = false;
                self.first_display_row = self
                    .first_display_row
                    .saturating_sub(u64::from(self.body_rows.max(1)));
                self.capture_anchor_from_viewport(&starts);
                self.select_visible_block(&starts, blocks);
                TranscriptOutcome::Changed
            }
            UiIntent::Page(PageMove::Forward) => {
                self.first_display_row = self
                    .first_display_row
                    .saturating_add(u64::from(self.body_rows.max(1)));
                self.clamp_scroll();
                if self.first_display_row >= self.max_first() {
                    self.follow = true;
                }
                self.capture_anchor_from_viewport(&starts);
                self.select_visible_block(&starts, blocks);
                TranscriptOutcome::Changed
            }
            UiIntent::Activate | UiIntent::Submit => {
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
            UiIntent::Expand => {
                if let Some(id) = self.selected.clone() {
                    if blocks.iter().any(|b| b.id == id && b.folded) {
                        return TranscriptOutcome::FoldToggled { id, folded: false };
                    }
                }
                TranscriptOutcome::Ignored
            }
            UiIntent::Collapse => {
                if let Some(id) = self.selected.clone() {
                    if blocks.iter().any(|b| b.id == id && !b.folded) {
                        return TranscriptOutcome::FoldToggled { id, folded: true };
                    }
                }
                TranscriptOutcome::Ignored
            }
            UiIntent::Toggle => {
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
            UiIntent::Cancel | UiIntent::Close => TranscriptOutcome::Cancelled,
            UiIntent::Open => TranscriptOutcome::Ignored,
        }
    }

    /// Mouse wheel / click against last painted regions.
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
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((id, _)) = self
                    .block_regions
                    .iter()
                    .find(|(_, r)| r.contains(event.position))
                {
                    let id = id.clone();
                    if self.selected.as_ref() == Some(&id) {
                        if blocks.iter().any(|b| b.id == id && b.enabled) {
                            return TranscriptOutcome::Activated(id);
                        }
                        return TranscriptOutcome::Ignored;
                    }
                    self.selected = Some(id);
                    self.follow = false;
                    return TranscriptOutcome::Changed;
                }
                TranscriptOutcome::Ignored
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
    /// Accepts-input chrome (selection gutter emphasis).
    focused: bool,
    /// Prefer ASCII kind prefixes.
    ascii: bool,
    /// Suppress chromatic roles (use Text / TextMuted only).
    colorless: bool,
    /// Empty-state copy when `blocks` is empty.
    empty_label: &'a str,
}

impl<'a, Id> Transcript<'a, Id> {
    /// Creates a transcript over the given blocks.
    #[must_use]
    pub const fn new(blocks: &'a [TranscriptBlock<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            blocks,
            system,
            focused: false,
            ascii: false,
            colorless: false,
            empty_label: "(empty transcript)",
        }
    }

    /// Paint-time accepts-input emphasis (host passes scene ownership).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII kind prefixes and fold markers.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced / no-color paint path.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Empty-state label.
    #[must_use]
    pub const fn empty_label(mut self, label: &'a str) -> Self {
        self.empty_label = label;
        self
    }
}

fn kind_prefix(kind: TranscriptKind, ascii: bool) -> &'static str {
    if ascii {
        kind.glyph_ascii()
    } else {
        kind.glyph()
    }
}

fn kind_style(system: &DesignSystem, kind: TranscriptKind, colorless: bool) -> Style {
    if colorless {
        return match kind {
            TranscriptKind::Thinking | TranscriptKind::Tool => system.style(Role::TextMuted),
            TranscriptKind::Approval => system.style(Role::TextStrong),
            _ => system.style(Role::Text),
        };
    }
    system.style(kind.role())
}

impl<Id: Clone + Eq> StatefulWidget for &Transcript<'_, Id> {
    type State = TranscriptState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.painted_area = area;
        state.block_regions.clear();
        // Keep state chrome flag aligned with paint-time ownership when host sets both.
        if self.focused {
            state.focused = true;
        }
        if area.width == 0 || area.height == 0 {
            state.body_rows = 0;
            return;
        }
        state.body_rows = area.height;

        if self.blocks.is_empty() {
            state.total_display_rows = 0;
            state.first_display_row = 0;
            let style = if self.colorless {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::TextDisabled)
            };
            let label = if self.ascii {
                "(empty)"
            } else {
                self.empty_label
            };
            let clipped = take_display_cols(label, usize::from(area.width));
            buffer.set_stringn(area.x, area.y, &clipped, usize::from(area.width), style);
            return;
        }

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
        let accepts = self.focused || state.focused;
        let fold_open = if self.ascii { "v " } else { "▾ " };
        let fold_closed = if self.ascii { "> " } else { "▸ " };
        let sel_gutter = if self.ascii { ">" } else { "›" };

        for (block_index, block) in self.blocks.iter().enumerate() {
            let Some((_, start, height)) = starts.get(block_index) else {
                continue;
            };
            let end = start.saturating_add(u64::from(*height));
            if end <= view_start || *start >= view_end {
                continue;
            }
            let selected = state.selected.as_ref() == Some(&block.id);
            let style = if selected && accepts {
                if self.colorless {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Accent)
                }
            } else {
                kind_style(self.system, block.kind, self.colorless)
            };

            let prefix = kind_prefix(block.kind, self.ascii);
            let mut region_y0: Option<u16> = None;
            let mut region_y1: u16 = area.y;

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
                let gutter = if selected && accepts { sel_gutter } else { " " };
                let label = format!(
                    "{gutter}{fold_closed}{prefix}{}",
                    take_display_cols(text, usize::from(area.width.saturating_sub(6)))
                );
                let clipped = take_display_cols(&label, usize::from(area.width));
                buffer.set_stringn(area.x, y, &clipped, usize::from(area.width), style);
                state.block_regions.push((
                    block.id.clone(),
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                ));
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
                region_y0.get_or_insert(y);
                region_y1 = y.saturating_add(1);
                let gutter = if selected && accepts && line_idx == 0 {
                    sel_gutter
                } else {
                    " "
                };
                let head = if line_idx == 0 {
                    format!("{gutter}{fold_open}{prefix}")
                } else {
                    format!("{gutter}  ")
                };
                let budget = usize::from(area.width).saturating_sub(display_cols(&head));
                let body = take_display_cols(line, budget);
                let full = format!("{head}{body}");
                let clipped = take_display_cols(&full, usize::from(area.width));
                buffer.set_stringn(area.x, y, &clipped, usize::from(area.width), style);
            }

            if let Some(y0) = region_y0 {
                state.block_regions.push((
                    block.id.clone(),
                    Rect {
                        x: area.x,
                        y: y0,
                        width: area.width,
                        height: region_y1.saturating_sub(y0).max(1),
                    },
                ));
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

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn zero_area_and_empty_are_safe() {
        let system = system();
        let blocks: [TranscriptBlock<'_, u32>; 0] = [];
        let mut state = TranscriptState::new();
        let t = Transcript::new(&blocks, &system);
        let mut terminal = Terminal::new(TestBackend::new(0, 0)).unwrap();
        terminal
            .draw(|f| f.render_stateful_widget(&t, Rect::default(), &mut state))
            .unwrap();
    }

    #[test]
    fn empty_state_paints_label() {
        let system = system();
        let blocks: [TranscriptBlock<'_, u32>; 0] = [];
        let mut state = TranscriptState::new();
        let mut terminal = Terminal::new(TestBackend::new(24, 3)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks, &system).empty_label("(no messages)"),
                    Rect::new(0, 0, 24, 3),
                    &mut state,
                );
            })
            .unwrap();
        let text = buffer_text(&terminal);
        assert!(
            text.contains("no messages") || text.contains("(empty"),
            "{text:?}"
        );
    }

    #[test]
    fn variable_height_viewport_maps_across_blocks() {
        let system = system();
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
        state.set_follow(false);
        state.first_display_row = 3;
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks, &system).focused(true),
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
        state.body_rows = 3;
        let (total, _) = TranscriptState::<u32>::layout_heights(&blocks);
        state.total_display_rows = total;
        state.set_follow(true);
        state.apply_follow();
        assert_eq!(state.first_display_row(), 5);

        // Host-gate: handle_key works without set_focused (authority is host).
        let outcome = state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &blocks);
        assert_eq!(outcome, TranscriptOutcome::Changed);
        assert!(!state.follow());
    }

    #[test]
    fn handle_key_does_not_require_focused_flag() {
        let lines = ["x"];
        let blocks = [TranscriptBlock::new(1u32, TranscriptKind::User, &lines)];
        let mut state = TranscriptState::new();
        assert!(!state.is_focused());
        let out = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &blocks);
        assert_eq!(out, TranscriptOutcome::Changed);
    }

    #[test]
    fn intent_activate_selected_enabled_block() {
        let lines = ["go"];
        let blocks = [TranscriptBlock::new(
            7u32,
            TranscriptKind::Assistant,
            &lines,
        )];
        let mut state = TranscriptState::new();
        state.select(Some(7));
        let out = state.handle_intent(UiIntent::Activate, &blocks);
        assert_eq!(out, TranscriptOutcome::Activated(7));
    }

    #[test]
    fn anchor_survives_append_and_resize_height() {
        let system = system();
        let a = ["a0", "a1"];
        let b = ["b0", "b1", "b2"];
        let blocks = [
            TranscriptBlock::new(1u32, TranscriptKind::User, &a),
            TranscriptBlock::new(2, TranscriptKind::Assistant, &b),
        ];
        let mut state = TranscriptState::new();
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
        state.select(Some(9));
        let outcome = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &blocks);
        assert_eq!(outcome, TranscriptOutcome::Ignored);
    }

    #[test]
    fn kind_glyphs_are_non_color() {
        assert!(!TranscriptKind::User.glyph().is_empty());
        assert_eq!(TranscriptKind::User.glyph_ascii(), "> ");
        assert_eq!(TranscriptKind::Tool.glyph_ascii(), "# ");
    }

    #[test]
    fn ascii_and_unicode_paint_kind_prefix() {
        let system = system();
        let lines = ["hello"];
        let blocks = [TranscriptBlock::new(1u32, TranscriptKind::User, &lines)];
        let mut state = TranscriptState::new();
        let mut terminal = Terminal::new(TestBackend::new(24, 2)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks, &system).ascii(true),
                    Rect::new(0, 0, 24, 2),
                    &mut state,
                );
            })
            .unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains('>') || text.contains("hello"), "{text:?}");
    }

    #[test]
    fn narrow_and_tiny_geometry_safe() {
        let system = system();
        let lines = ["こんにちは 🔧", "line two"];
        let blocks = [
            TranscriptBlock::new(1u32, TranscriptKind::User, &lines[..1]),
            TranscriptBlock::new(2, TranscriptKind::Assistant, &lines[1..]).folded(true),
        ];
        for (w, h) in [(40, 12), (22, 6), (12, 4), (8, 3), (1, 1)] {
            let mut state = TranscriptState::new();
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|f| {
                    f.render_stateful_widget(
                        &Transcript::new(&blocks, &system)
                            .focused(true)
                            .ascii(w < 16)
                            .colorless(w < 12),
                        Rect::new(0, 0, w, h),
                        &mut state,
                    );
                })
                .unwrap();
            assert!(state.painted_area.width <= w);
        }
    }

    #[test]
    fn mouse_click_selects_block() {
        let system = system();
        let a = ["one"];
        let b = ["two"];
        let blocks = [
            TranscriptBlock::new(1u32, TranscriptKind::User, &a),
            TranscriptBlock::new(2, TranscriptKind::Assistant, &b),
        ];
        let mut state = TranscriptState::new();
        let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
        terminal
            .draw(|f| {
                f.render_stateful_widget(
                    &Transcript::new(&blocks, &system).focused(true),
                    Rect::new(0, 0, 30, 4),
                    &mut state,
                );
            })
            .unwrap();
        assert!(!state.block_regions.is_empty());
        let (id, rect) = state.block_regions[0].clone();
        assert_eq!(id, 1);
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: ratatui_core::layout::Position {
                x: rect.x,
                y: rect.y,
            },
            modifiers: KeyModifiers::NONE,
        };
        let out = state.handle_mouse(event, &blocks);
        assert_eq!(out, TranscriptOutcome::Changed);
        assert_eq!(state.selected(), Some(&1));
    }

    #[test]
    fn no_stream_view_types_in_module() {
        let src = include_str!("transcript.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        let banned = ["StreamView", "StreamItem"];
        for b in banned {
            assert!(
                !code.lines().any(|l| {
                    let t = l.trim_start();
                    !t.starts_with("//") && t.contains(b)
                }),
                "transcript must not reintroduce {b}"
            );
        }
    }
}
