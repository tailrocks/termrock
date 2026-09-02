// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical ScrollArea — shared scrolling primitive for TermRock.
//!
//! Every scrollable surface (logs, transcripts, lists, dialogs, nested panes)
//! should drive offsets through [`ScrollAreaState`]. Specialized helpers in
//! `termrock::scroll` (scrollbar paint, `TailScroll`, dialog dual-axis) remain
//! available; this module owns **policy**: follow/pause, anchors, chaining,
//! visible ranges, and new-content indication.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    perf::{
        FollowMode, NewContentIndicator, ScrollAnchor, ScrollAnchorKind,
        pause_follow_on_user_scroll,
    },
    scroll::{ScrollAxis, apply_delta_u16, max_offset},
    style::{DesignSystem, Role},
    text::take_display_cols,
};

/// Scrollbar visibility policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ScrollBarVisibility {
    /// Show when content overflows.
    #[default]
    Auto,
    /// Always show track.
    Always,
    /// Never show.
    Never,
}

/// Nested scroll chaining when the child hits an edge under wheel input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ScrollChain {
    /// Child always consumes wheel (default for modal bodies).
    #[default]
    Capture,
    /// Always forward wheel to parent (rare; parent is sole scroller).
    Parent,
    /// Child scrolls until edge; further wheel may go to parent.
    NestedPreferChild,
}

/// Outcome of a scroll operation (deterministic for tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ScrollOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Offset and/or follow state changed.
    Scrolled,
    /// Follow resumed or paused without offset change.
    FollowChanged,
    /// Wheel not applied here (parent may chain).
    ChainToParent,
}

impl ScrollOutcome {
    /// Offset moved (not follow-only or chain).
    #[must_use]
    pub const fn is_scrolled(self) -> bool {
        matches!(self, Self::Scrolled)
    }

    /// Event was handled here (not ignored / not chain).
    #[must_use]
    pub const fn consumed(self) -> bool {
        matches!(self, Self::Scrolled | Self::FollowChanged)
    }

    /// Parent should try chaining.
    #[must_use]
    pub const fn chains(self) -> bool {
        matches!(self, Self::ChainToParent)
    }
}

/// Inclusive half-open visible range on one axis: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VisibleRange {
    /// First visible unit (row or column).
    pub start: u64,
    /// One past last visible unit.
    pub end: u64,
}

impl VisibleRange {
    /// Empty range.
    #[must_use]
    pub const fn empty() -> Self {
        Self { start: 0, end: 0 }
    }

    /// Length in units.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Whether empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// Controlled scroll offsets, follow policy, anchors, and nesting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollAreaState {
    offset_y: u16,
    offset_x: u16,
    content_h: u16,
    content_w: u16,
    viewport_h: u16,
    viewport_w: u16,
    follow: FollowMode,
    indicator: NewContentIndicator,
    /// Optional stable anchor for restore after reflow/resize.
    anchor: Option<ScrollAnchor>,
    chain: ScrollChain,
    /// Vertical wheel lines per notch.
    wheel_step_y: u16,
    /// Horizontal wheel columns per notch.
    wheel_step_x: u16,
    /// When true, vertical axis accepts input.
    axis_y: bool,
    /// When true, horizontal axis accepts input.
    axis_x: bool,
}

impl Default for ScrollAreaState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollAreaState {
    /// Zero offsets, follow paused, both axes enabled.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            offset_y: 0,
            offset_x: 0,
            content_h: 0,
            content_w: 0,
            viewport_h: 0,
            viewport_w: 0,
            follow: FollowMode::Paused,
            indicator: NewContentIndicator {
                unseen: 0,
                visible: false,
            },
            anchor: None,
            chain: ScrollChain::Capture,
            wheel_step_y: 3,
            wheel_step_x: 4,
            axis_y: true,
            axis_x: true,
        }
    }

    /// Enable/disable axes.
    #[must_use]
    pub const fn axes(mut self, vertical: bool, horizontal: bool) -> Self {
        self.axis_y = vertical;
        self.axis_x = horizontal;
        self
    }

    /// Nesting policy.
    #[must_use]
    pub const fn chain(mut self, chain: ScrollChain) -> Self {
        self.chain = chain;
        self
    }

    /// Wheel steps.
    #[must_use]
    pub fn wheel_steps(mut self, vertical: u16, horizontal: u16) -> Self {
        self.wheel_step_y = vertical.max(1);
        self.wheel_step_x = horizontal.max(1);
        self
    }

    /// Project content size (caller-owned measurement; may grow on stream).
    pub fn set_content_size(&mut self, width: u16, height: u16) {
        let grew = height > self.content_h;
        let appended = height.saturating_sub(self.content_h);
        self.content_w = width;
        self.content_h = height;
        if grew && appended > 0 {
            self.on_content_grown(u64::from(appended));
        } else {
            self.clamp();
        }
    }

    /// Viewport size from last layout (resize).
    pub fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport_w = width;
        self.viewport_h = height;
        self.restore_or_clamp_after_geometry();
    }

    /// After content growth of `appended` units on the vertical axis.
    pub fn on_content_grown(&mut self, appended: u64) {
        self.indicator.note_appended(appended, self.follow);
        match self.follow {
            FollowMode::Following => {
                self.stick_to_tail_y();
                self.indicator.clear();
            }
            FollowMode::Paused => {
                // Preserve top offset (insert above would need host anchor map).
                self.clamp();
            }
        }
    }

    /// Notify horizontal content growth (no follow-tail semantics by default).
    pub fn on_content_grown_x(&mut self, _appended: u64) {
        self.clamp();
    }

    fn stick_to_tail_y(&mut self) {
        self.offset_y = max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
        self.anchor = Some(ScrollAnchor::from_end(0));
    }

    fn restore_or_clamp_after_geometry(&mut self) {
        if matches!(self.follow, FollowMode::Following) {
            self.stick_to_tail_y();
            return;
        }
        if let Some(anchor) = self.anchor.clone() {
            self.apply_anchor(&anchor, |_| None);
        }
        self.clamp();
    }

    /// Apply a stable anchor. `resolve_id` maps content ids → row index.
    pub fn apply_anchor(
        &mut self,
        anchor: &ScrollAnchor,
        resolve_id: impl FnOnce(&str) -> Option<u64>,
    ) {
        match anchor.kind {
            ScrollAnchorKind::Index => {
                let max = max_offset(self.content_h as usize, self.viewport_h as usize) as u64;
                self.offset_y = (anchor.index.min(max)) as u16;
            }
            ScrollAnchorKind::FromEnd => {
                let max = max_offset(self.content_h as usize, self.viewport_h as usize) as u64;
                self.offset_y = max.saturating_sub(anchor.index) as u16;
            }
            ScrollAnchorKind::ContentId => {
                if let Some(id) = anchor.content_id.as_deref()
                    && let Some(idx) = resolve_id(id)
                {
                    let max = max_offset(self.content_h as usize, self.viewport_h as usize) as u64;
                    self.offset_y = idx.min(max) as u16;
                }
            }
        }
        self.anchor = Some(anchor.clone());
        self.clamp();
    }

    /// Snapshot an index anchor at the current top.
    pub fn capture_index_anchor(&mut self) {
        self.anchor = Some(ScrollAnchor::at_index(u64::from(self.offset_y)));
    }

    /// Snapshot a from-end anchor (for paused logs near tail).
    pub fn capture_from_end_anchor(&mut self) {
        let max = max_offset(self.content_h as usize, self.viewport_h as usize) as u64;
        let dist = max.saturating_sub(u64::from(self.offset_y));
        self.anchor = Some(ScrollAnchor::from_end(dist));
    }

    #[must_use]
    /// Vertical offset.
    pub const fn offset_y(&self) -> u16 {
        self.offset_y
    }

    #[must_use]
    /// Horizontal offset.
    pub const fn offset_x(&self) -> u16 {
        self.offset_x
    }

    #[must_use]
    /// Content height.
    pub const fn content_h(&self) -> u16 {
        self.content_h
    }

    #[must_use]
    /// Content width.
    pub const fn content_w(&self) -> u16 {
        self.content_w
    }

    #[must_use]
    /// Viewport height.
    pub const fn viewport_h(&self) -> u16 {
        self.viewport_h
    }

    #[must_use]
    /// Viewport width.
    pub const fn viewport_w(&self) -> u16 {
        self.viewport_w
    }

    #[must_use]
    /// Follow mode.
    pub const fn follow_mode(&self) -> FollowMode {
        self.follow
    }

    #[must_use]
    /// New-content indicator (paused + unseen).
    pub const fn new_content(&self) -> NewContentIndicator {
        self.indicator
    }

    #[must_use]
    /// Nesting chain policy.
    pub const fn chain_policy(&self) -> ScrollChain {
        self.chain
    }

    #[must_use]
    /// Current anchor, if any.
    pub const fn anchor(&self) -> Option<&ScrollAnchor> {
        self.anchor.as_ref()
    }

    /// Visible vertical half-open range `[start, end)` in content rows.
    #[must_use]
    pub fn visible_range_y(&self) -> VisibleRange {
        if self.viewport_h == 0 || self.content_h == 0 {
            return VisibleRange::empty();
        }
        let start = u64::from(self.offset_y);
        let end = start
            .saturating_add(u64::from(self.viewport_h))
            .min(u64::from(self.content_h));
        VisibleRange { start, end }
    }

    /// Visible horizontal half-open range in content columns.
    #[must_use]
    pub fn visible_range_x(&self) -> VisibleRange {
        if self.viewport_w == 0 || self.content_w == 0 {
            return VisibleRange::empty();
        }
        let start = u64::from(self.offset_x);
        let end = start
            .saturating_add(u64::from(self.viewport_w))
            .min(u64::from(self.content_w));
        VisibleRange { start, end }
    }

    /// Whether vertical content overflows the viewport.
    #[must_use]
    pub fn overflows_y(&self) -> bool {
        self.viewport_h > 0 && self.content_h > self.viewport_h
    }

    /// Whether horizontal content overflows.
    #[must_use]
    pub fn overflows_x(&self) -> bool {
        self.viewport_w > 0 && self.content_w > self.viewport_w
    }

    /// At top (vertical).
    #[must_use]
    pub const fn at_top(&self) -> bool {
        self.offset_y == 0
    }

    /// At bottom (vertical).
    #[must_use]
    pub fn at_bottom(&self) -> bool {
        let max = max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
        self.offset_y >= max
    }

    /// At left (horizontal).
    #[must_use]
    pub const fn at_left(&self) -> bool {
        self.offset_x == 0
    }

    /// At right (horizontal).
    #[must_use]
    pub fn at_right(&self) -> bool {
        let max = max_offset(self.content_w as usize, self.viewport_w as usize) as u16;
        self.offset_x >= max
    }

    /// Set vertical offset (clamped). Pauses follow (user-driven position).
    pub fn set_offset_y(&mut self, y: u16) {
        self.pause_follow_user();
        self.offset_y = y;
        self.clamp();
        self.capture_index_anchor();
    }

    /// Programmatic vertical offset (cursor reveal). Does **not** pause follow.
    pub fn set_offset_y_quiet(&mut self, y: u16) {
        self.offset_y = y;
        self.clamp();
        if !self.is_following() {
            self.capture_index_anchor();
        }
    }

    /// Set horizontal offset (clamped).
    pub fn set_offset_x(&mut self, x: u16) {
        self.offset_x = x;
        self.clamp();
    }

    /// Clamp offsets to content.
    pub fn clamp(&mut self) {
        let max_y = max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
        let max_x = max_offset(self.content_w as usize, self.viewport_w as usize) as u16;
        self.offset_y = self.offset_y.min(max_y);
        self.offset_x = self.offset_x.min(max_x);
    }

    fn pause_follow_user(&mut self) {
        self.follow = pause_follow_on_user_scroll(self.follow);
    }

    /// Scroll by signed deltas (negative = up/left). Pauses follow on Y motion.
    pub fn scroll_by(&mut self, dy: isize, dx: isize) -> ScrollOutcome {
        let before = (self.offset_y, self.offset_x);
        if dy != 0 && self.axis_y {
            self.pause_follow_user();
            apply_delta_u16(
                self.content_h as usize,
                self.viewport_h as usize,
                &mut self.offset_y,
                dy,
            );
        }
        if dx != 0 && self.axis_x {
            apply_delta_u16(
                self.content_w as usize,
                self.viewport_w as usize,
                &mut self.offset_x,
                dx,
            );
        }
        self.clamp();
        if dy != 0 {
            self.capture_index_anchor();
        }
        if (self.offset_y, self.offset_x) != before {
            ScrollOutcome::Scrolled
        } else {
            ScrollOutcome::Ignored
        }
    }

    /// Page vertically by viewport height.
    pub fn page(&mut self, forward: bool) -> ScrollOutcome {
        if !self.axis_y {
            return ScrollOutcome::Ignored;
        }
        let step = self.viewport_h.max(1) as isize;
        self.scroll_by(if forward { step } else { -step }, 0)
    }

    /// Jump to top.
    pub fn home(&mut self) -> ScrollOutcome {
        if !self.axis_y {
            return ScrollOutcome::Ignored;
        }
        let before = self.offset_y;
        self.set_offset_y(0);
        if self.offset_y != before {
            ScrollOutcome::Scrolled
        } else {
            ScrollOutcome::Ignored
        }
    }

    /// Jump to bottom (does not enable follow).
    pub fn end(&mut self) -> ScrollOutcome {
        if !self.axis_y {
            return ScrollOutcome::Ignored;
        }
        let before = self.offset_y;
        self.pause_follow_user();
        self.offset_y = max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
        self.clamp();
        self.capture_from_end_anchor();
        if self.offset_y != before {
            ScrollOutcome::Scrolled
        } else {
            ScrollOutcome::Ignored
        }
    }

    /// Attach to live tail and clear new-content badge.
    pub fn follow_tail(&mut self) {
        self.follow = FollowMode::Following;
        self.stick_to_tail_y();
        self.indicator.clear();
    }

    /// Pause follow without moving offset.
    pub fn pause_follow(&mut self) {
        self.follow = FollowMode::Paused;
        self.capture_from_end_anchor();
    }

    /// Resume follow (jumps to tail).
    pub fn resume_follow(&mut self) {
        self.follow_tail();
    }

    #[must_use]
    /// Following tail.
    pub const fn is_following(&self) -> bool {
        matches!(self.follow, FollowMode::Following)
    }

    /// Jump to end and resume follow (user action on new-content indicator).
    pub fn jump_to_new_content(&mut self) {
        self.follow_tail();
    }

    /// Page / line / home / end / arrows.
    pub fn handle_key(&mut self, key: KeyEvent) -> ScrollOutcome {
        if key.kind == KeyEventKind::Release {
            return ScrollOutcome::Ignored;
        }
        if !key.modifiers.is_empty()
            && key.modifiers != KeyModifiers::SHIFT
            && !matches!(key.code, KeyCode::Char(_))
        {
            // Leave Ctrl+ combos to host (e.g. select-all).
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT)
            {
                return ScrollOutcome::Ignored;
            }
        }
        match key.code {
            KeyCode::Up if self.axis_y => self.scroll_by(-1, 0),
            KeyCode::Down if self.axis_y => self.scroll_by(1, 0),
            KeyCode::PageUp if self.axis_y => self.page(false),
            KeyCode::PageDown if self.axis_y => self.page(true),
            KeyCode::Home if self.axis_y => self.home(),
            KeyCode::End if self.axis_y => self.end(),
            KeyCode::Left if self.axis_x => self.scroll_by(0, -1),
            KeyCode::Right if self.axis_x => self.scroll_by(0, 1),
            _ => ScrollOutcome::Ignored,
        }
    }

    /// Semantic intents (Move/Page map to scroll when this surface owns input).
    pub fn handle_intent(&mut self, intent: UiIntent) -> ScrollOutcome {
        match intent {
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) if self.axis_y => {
                self.scroll_by(-1, 0)
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) if self.axis_y => {
                self.scroll_by(1, 0)
            }
            UiIntent::Move(NavigationMove::Left) if self.axis_x => self.scroll_by(0, -1),
            UiIntent::Move(NavigationMove::Right) if self.axis_x => self.scroll_by(0, 1),
            UiIntent::Move(NavigationMove::First) if self.axis_y => self.home(),
            UiIntent::Move(NavigationMove::Last) if self.axis_y => self.end(),
            UiIntent::Page(PageMove::Backward) if self.axis_y => self.page(false),
            UiIntent::Page(PageMove::Forward) if self.axis_y => self.page(true),
            _ => ScrollOutcome::Ignored,
        }
    }

    /// Wheel scroll with nesting policy.
    ///
    /// Returns [`ScrollOutcome::ChainToParent`] when the child is at edge under
    /// [`ScrollChain::NestedPreferChild`] and the wheel would not move.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> ScrollOutcome {
        use crate::scroll::{ScrollAxes, ScrollDelta, mouse_scroll_delta_with_step};

        let axes = ScrollAxes {
            vertical: self.axis_y,
            horizontal: self.axis_x,
        };
        let Some(delta) =
            mouse_scroll_delta_with_step(event.kind, event.modifiers, axes, self.wheel_step_x)
        else {
            return ScrollOutcome::Ignored;
        };
        let (dy, dx) = match delta {
            ScrollDelta {
                axis: ScrollAxis::Vertical,
                amount,
            } => (isize::from(amount) * self.wheel_step_y as isize, 0),
            ScrollDelta {
                axis: ScrollAxis::Horizontal,
                amount,
            } => (0, isize::from(amount)),
        };
        // amount for vertical is ±1 from helper; scale by wheel_step_y above.
        // For horizontal, amount already includes step.
        match self.chain {
            ScrollChain::Parent => ScrollOutcome::ChainToParent,
            ScrollChain::Capture => self.scroll_by(dy, dx),
            ScrollChain::NestedPreferChild => {
                let at_edge = (dy < 0 && self.at_top())
                    || (dy > 0 && self.at_bottom())
                    || (dx < 0 && self.at_left())
                    || (dx > 0 && self.at_right());
                let out = self.scroll_by(dy, dx);
                if matches!(out, ScrollOutcome::Ignored) && at_edge {
                    ScrollOutcome::ChainToParent
                } else {
                    out
                }
            }
        }
    }
}

/// ScrollArea: paints optional scrollbars + new-content strip.
#[derive(Debug, Clone, Copy)]
pub struct ScrollArea<'a> {
    tokens: &'a DesignSystem,
    bar: ScrollBarVisibility,
    show_new_content: bool,
    focused: bool,
    hovered: bool,
}

impl<'a> ScrollArea<'a> {
    /// Tokens.
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self {
            tokens,
            bar: ScrollBarVisibility::Auto,
            show_new_content: true,
            focused: false,
            hovered: false,
        }
    }

    /// Bar policy.
    #[must_use]
    pub const fn bar(mut self, bar: ScrollBarVisibility) -> Self {
        self.bar = bar;
        self
    }

    /// Whether to paint new-content indicator when paused+unseen.
    #[must_use]
    pub const fn show_new_content(mut self, show: bool) -> Self {
        self.show_new_content = show;
        self
    }

    /// Keyboard owner: the thumb uses the primary rung.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Pointer over the track: the thumb uses the secondary rung.
    #[must_use]
    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Content body rect after reserving scrollbar column/row when needed.
    #[must_use]
    pub fn body_area(&self, area: Rect, state: &ScrollAreaState) -> Rect {
        let need_v = self.need_bar_v(state);
        let need_h = self.need_bar_h(state);
        let w = if need_v && area.width > 1 {
            area.width - 1
        } else {
            area.width
        };
        let h = if need_h && area.height > 1 {
            area.height - 1
        } else {
            area.height
        };
        Rect::new(area.x, area.y, w, h)
    }

    fn need_bar_v(&self, state: &ScrollAreaState) -> bool {
        match self.bar {
            ScrollBarVisibility::Never => false,
            ScrollBarVisibility::Always => state.axis_y,
            ScrollBarVisibility::Auto => state.axis_y && state.overflows_y(),
        }
    }

    fn need_bar_h(&self, state: &ScrollAreaState) -> bool {
        match self.bar {
            ScrollBarVisibility::Never => false,
            ScrollBarVisibility::Always => state.axis_x,
            ScrollBarVisibility::Auto => state.axis_x && state.overflows_x(),
        }
    }

    /// Paint vertical/horizontal scrollbar gutters when policy requires.
    pub fn render_bars(&self, area: Rect, buffer: &mut Buffer, state: &ScrollAreaState) {
        if area.is_empty() {
            return;
        }
        let need_v = self.need_bar_v(state);
        let need_h = self.need_bar_h(state);

        // One scrollbar language: `│` track, `┃` / `━` thumb, overflow only.
        if need_v && area.width >= 1 {
            let bar_h = if need_h {
                area.height.saturating_sub(1)
            } else {
                area.height
            };
            crate::scroll::render_scrollbar(
                buffer,
                Rect::new(area.right().saturating_sub(1), area.y, 1, bar_h),
                crate::scroll::ScrollbarSpec::new(
                    crate::scroll::ScrollAxis::Vertical,
                    crate::scroll::ScrollbarGeometry::new(
                        usize::from(state.content_h),
                        usize::from(state.viewport_h),
                        state.offset_y,
                    ),
                )
                .focused(self.focused)
                .hovered(self.hovered),
                self.tokens,
            );
        }

        if need_h && area.height >= 1 {
            let bar_w = if need_v {
                area.width.saturating_sub(1)
            } else {
                area.width
            };
            crate::scroll::render_scrollbar(
                buffer,
                Rect::new(area.x, area.bottom().saturating_sub(1), bar_w, 1),
                crate::scroll::ScrollbarSpec::new(
                    crate::scroll::ScrollAxis::Horizontal,
                    crate::scroll::ScrollbarGeometry::new(
                        usize::from(state.content_w),
                        usize::from(state.viewport_w),
                        state.offset_x,
                    ),
                )
                .focused(self.focused)
                .hovered(self.hovered),
                self.tokens,
            );
        }
    }

    /// Paint new-content indicator with a structural down cue and warning role.
    pub fn render_new_content(&self, area: Rect, buffer: &mut Buffer, state: &ScrollAreaState) {
        if !self.show_new_content || !state.indicator.visible || area.height == 0 {
            return;
        }
        let style = self.tokens.style(Role::Warning);
        let marker = "↓";
        let label = format!("{marker} {} new", state.indicator.unseen);
        let y = area.bottom().saturating_sub(1);
        let text = take_display_cols(&label, usize::from(area.width));
        buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, KeyModifiers};
    use crate::scroll::max_line_width;
    use ratatui_core::text::Line;

    #[test]
    fn scroll_clamps_and_pages() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(10, 100);
        s.set_viewport(10, 10);
        assert_eq!(
            s.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            ScrollOutcome::Scrolled
        );
        assert_eq!(s.offset_y(), 10);
        assert_eq!(
            s.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            ScrollOutcome::Scrolled
        );
        assert_eq!(s.offset_y(), 90);
        s.follow_tail();
        assert!(s.is_following());
        assert_eq!(s.offset_y(), 90);
    }

    #[test]
    fn empty_content_zero_offset() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(0, 0);
        s.set_viewport(20, 10);
        s.clamp();
        assert_eq!(s.offset_y(), 0);
        assert!(s.visible_range_y().is_empty());
    }

    #[test]
    fn follow_and_pause_unseen() {
        let mut s = ScrollAreaState::new();
        s.set_viewport(10, 10);
        s.set_content_size(10, 50);
        s.follow_tail();
        assert_eq!(s.offset_y(), 40);
        s.scroll_by(-5, 0);
        assert!(!s.is_following());
        s.set_content_size(10, 80);
        assert!(s.new_content().visible);
        assert_eq!(s.new_content().unseen, 30);
        s.jump_to_new_content();
        assert!(s.is_following());
        assert!(!s.new_content().visible);
        assert_eq!(s.offset_y(), max_offset(80, 10) as u16);
    }

    #[test]
    fn nested_chain_at_edge() {
        let mut child = ScrollAreaState::new().chain(ScrollChain::NestedPreferChild);
        child.set_content_size(5, 20);
        child.set_viewport(5, 10);
        child.set_offset_y(0);
        let up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            position: ratatui_core::layout::Position::new(0, 0),
            modifiers: KeyModifiers::NONE,
        };
        // at top + scroll up → chain
        assert_eq!(child.handle_mouse(up), ScrollOutcome::ChainToParent);
        child.set_offset_y(5);
        assert_eq!(child.handle_mouse(up), ScrollOutcome::Scrolled);
    }

    #[test]
    fn anchor_index_survives_resize() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(10, 100);
        s.set_viewport(10, 20);
        s.set_offset_y(40);
        s.capture_index_anchor();
        s.set_viewport(10, 10);
        // restore_or_clamp applies index anchor
        assert_eq!(s.offset_y(), 40.min(max_offset(100, 10) as u16));
    }

    #[test]
    fn content_id_anchor_restore() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(10, 200);
        s.set_viewport(10, 10);
        let a = ScrollAnchor::content_id("msg-42");
        s.apply_anchor(&a, |id| if id == "msg-42" { Some(77) } else { None });
        assert_eq!(s.offset_y(), 77);
    }

    #[test]
    fn visible_range_for_virtualization() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(80, 1000);
        s.set_viewport(40, 25);
        s.set_offset_y(100);
        let r = s.visible_range_y();
        assert_eq!(r.start, 100);
        assert_eq!(r.end, 125);
        assert_eq!(r.len(), 25);
    }

    #[test]
    fn intent_page_and_move() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(10, 100);
        s.set_viewport(10, 10);
        assert_eq!(
            s.handle_intent(UiIntent::Page(PageMove::Forward)),
            ScrollOutcome::Scrolled
        );
        assert_eq!(s.offset_y(), 10);
    }

    #[test]
    fn unicode_wrapping_content_width_uses_display_cols() {
        // Wide graphemes must contribute display width for horizontal overflow.
        let lines = [
            Line::from("hello"),
            Line::from("日本語テスト"),
            Line::from("emoji 🧪 lab"),
        ];
        let w = max_line_width(&lines);
        assert!(w >= 8, "CJK/emoji wider than ascii hello: {w}");
        let mut s = ScrollAreaState::new().axes(true, true);
        s.set_content_size(w as u16, lines.len() as u16);
        s.set_viewport(5, 2);
        assert!(s.overflows_x());
        assert!(s.overflows_y());
    }

    #[test]
    fn huge_content_offset_ops_are_o1() {
        // Max u16 rows: only store scalars; scroll ops must not allocate per row.
        let mut s = ScrollAreaState::new();
        s.set_viewport(80, 40);
        s.set_content_size(80, u16::MAX);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = s.page(true);
            let _ = s.page(false);
            let _ = s.visible_range_y();
            let _ = s.handle_intent(UiIntent::Page(PageMove::Forward));
        }
        let elapsed = start.elapsed();
        assert!(s.offset_y() > 0);
        let r = s.visible_range_y();
        assert_eq!(r.len(), 40.min(u64::from(u16::MAX)));
        // Loose budget: pure scalar math on CI; fail only if catastrophic.
        assert!(
            elapsed.as_millis() < 2_000,
            "huge scroll ops too slow: {elapsed:?}"
        );
    }

    #[test]
    fn horizontal_wheel_and_keys() {
        let mut s = ScrollAreaState::new().axes(true, true);
        s.set_content_size(200, 20);
        s.set_viewport(40, 10);
        assert_eq!(
            s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            ScrollOutcome::Scrolled
        );
        assert_eq!(s.offset_x(), 1);
        let ev = MouseEvent {
            kind: MouseEventKind::ScrollRight,
            position: ratatui_core::layout::Position::new(0, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(s.handle_mouse(ev), ScrollOutcome::Scrolled);
        assert!(s.offset_x() > 1);
    }

    #[test]
    fn overflow_paints_one_column_box_track_and_thumb() {
        let system = DesignSystem::default();
        let mut state = ScrollAreaState::new();
        state.set_content_size(10, 40);
        state.set_viewport(10, 8);
        let area = Rect::new(0, 0, 10, 8);
        let mut buffer = Buffer::empty(area);
        ScrollArea::new(&system)
            .focused(true)
            .render_bars(area, &mut buffer, &state);
        let gutter: Vec<&str> = (0..area.height)
            .map(|y| buffer[(area.right() - 1, y)].symbol())
            .collect();
        assert!(gutter.contains(&"┃"), "{gutter:?}");
        assert!(gutter.contains(&"│"), "{gutter:?}");
        assert_eq!(
            buffer[(area.right() - 1, 0)].fg,
            system.scrollbar_thumb(true, false).fg.unwrap()
        );
        let track_y = gutter.iter().position(|g| *g == "│").unwrap() as u16;
        assert_eq!(
            buffer[(area.right() - 1, track_y)].fg,
            system.scrollbar_track().fg.unwrap()
        );
    }

    #[test]
    fn no_bar_when_content_fits() {
        let system = DesignSystem::default();
        let mut state = ScrollAreaState::new();
        state.set_content_size(10, 8);
        state.set_viewport(10, 8);
        let area = Rect::new(0, 0, 10, 8);
        let mut buffer = Buffer::empty(area);
        ScrollArea::new(&system).render_bars(area, &mut buffer, &state);
        for y in 0..area.height {
            assert_eq!(buffer[(area.right() - 1, y)].symbol(), " ");
        }
    }
}
