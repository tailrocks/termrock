// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared ScrollArea / ScrollState grammar (Plan 052).

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind},
    scroll::{apply_delta_u16, max_offset},
    style::{DesignSystem, Role},
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

/// Controlled scroll offsets + content metrics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScrollAreaState {
    offset_y: u16,
    offset_x: u16,
    content_h: u16,
    content_w: u16,
    viewport_h: u16,
    viewport_w: u16,
    follow_tail: bool,
}

impl ScrollAreaState {
    /// Zero offsets.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            offset_y: 0,
            offset_x: 0,
            content_h: 0,
            content_w: 0,
            viewport_h: 0,
            viewport_w: 0,
            follow_tail: false,
        }
    }

    /// Project content size (caller-owned measurement).
    pub const fn set_content_size(&mut self, width: u16, height: u16) {
        self.content_w = width;
        self.content_h = height;
    }

    /// Viewport size from last layout.
    pub const fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport_w = width;
        self.viewport_h = height;
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

    /// Set vertical offset (clamped). Used by cursor-follow painters.
    pub fn set_offset_y(&mut self, y: u16) {
        self.offset_y = y;
        self.clamp();
    }

    /// Clamp offsets to content.
    pub fn clamp(&mut self) {
        let max_y = max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
        let max_x = max_offset(self.content_w as usize, self.viewport_w as usize) as u16;
        self.offset_y = self.offset_y.min(max_y);
        self.offset_x = self.offset_x.min(max_x);
    }

    /// Page / line / home / end.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        let before = (self.offset_y, self.offset_x);
        match key.code {
            KeyCode::Up => {
                apply_delta_u16(
                    self.content_h as usize,
                    self.viewport_h as usize,
                    &mut self.offset_y,
                    -1,
                );
            }
            KeyCode::Down => {
                apply_delta_u16(
                    self.content_h as usize,
                    self.viewport_h as usize,
                    &mut self.offset_y,
                    1,
                );
            }
            KeyCode::PageUp => {
                apply_delta_u16(
                    self.content_h as usize,
                    self.viewport_h as usize,
                    &mut self.offset_y,
                    -(self.viewport_h as isize).max(1),
                );
            }
            KeyCode::PageDown => {
                apply_delta_u16(
                    self.content_h as usize,
                    self.viewport_h as usize,
                    &mut self.offset_y,
                    (self.viewport_h as isize).max(1),
                );
            }
            KeyCode::Home => self.offset_y = 0,
            KeyCode::End => {
                self.offset_y =
                    max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
            }
            KeyCode::Left => {
                apply_delta_u16(
                    self.content_w as usize,
                    self.viewport_w as usize,
                    &mut self.offset_x,
                    -1,
                );
            }
            KeyCode::Right => {
                apply_delta_u16(
                    self.content_w as usize,
                    self.viewport_w as usize,
                    &mut self.offset_x,
                    1,
                );
            }
            _ => return false,
        }
        self.follow_tail = false;
        self.clamp();
        (self.offset_y, self.offset_x) != before
    }

    /// Wheel scroll.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> bool {
        let before = self.offset_y;
        match event.kind {
            MouseEventKind::ScrollUp => {
                apply_delta_u16(
                    self.content_h as usize,
                    self.viewport_h as usize,
                    &mut self.offset_y,
                    -3,
                );
            }
            MouseEventKind::ScrollDown => {
                apply_delta_u16(
                    self.content_h as usize,
                    self.viewport_h as usize,
                    &mut self.offset_y,
                    3,
                );
            }
            _ => return false,
        }
        self.follow_tail = false;
        self.clamp();
        self.offset_y != before
    }

    /// Attach to tail (log follow).
    pub fn follow_tail(&mut self) {
        self.follow_tail = true;
        self.offset_y = max_offset(self.content_h as usize, self.viewport_h as usize) as u16;
    }

    #[must_use]
    /// Following tail.
    pub const fn is_following(&self) -> bool {
        self.follow_tail
    }
}

/// ScrollArea: paints optional scrollbar track for vertical axis.
#[derive(Debug, Clone, Copy)]
pub struct ScrollArea<'a> {
    tokens: &'a DesignSystem,
    bar: ScrollBarVisibility,
}

impl<'a> ScrollArea<'a> {
    /// Tokens.
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self {
            tokens,
            bar: ScrollBarVisibility::Auto,
        }
    }

    /// Bar policy.
    #[must_use]
    pub const fn bar(mut self, bar: ScrollBarVisibility) -> Self {
        self.bar = bar;
        self
    }

    /// Content body rect after reserving scrollbar column when needed.
    #[must_use]
    pub fn body_area(&self, area: Rect, state: &ScrollAreaState) -> Rect {
        let need_bar = match self.bar {
            ScrollBarVisibility::Never => false,
            ScrollBarVisibility::Always => true,
            ScrollBarVisibility::Auto => state.content_h > state.viewport_h && state.viewport_h > 0,
        };
        if need_bar && area.width > 1 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        }
    }

    /// Paint vertical scrollbar gutter when policy requires.
    pub fn render_bars(&self, area: Rect, buffer: &mut Buffer, state: &ScrollAreaState) {
        if area.is_empty() {
            return;
        }
        let need_bar = match self.bar {
            ScrollBarVisibility::Never => false,
            ScrollBarVisibility::Always => true,
            ScrollBarVisibility::Auto => state.content_h > state.viewport_h && state.viewport_h > 0,
        };
        if !need_bar || area.width < 1 {
            return;
        }
        let x = area.right().saturating_sub(1);
        let track = self.tokens.style(Role::ScrollTrack);
        let thumb = self.tokens.style(Role::ScrollThumb);
        for y in area.y..area.bottom() {
            buffer[(x, y)].set_char('│').set_style(track);
        }
        if state.content_h > 0 && state.viewport_h > 0 {
            let max_off = max_offset(state.content_h as usize, state.viewport_h as usize);
            let thumb_h = (state.viewport_h as usize)
                .saturating_mul(area.height as usize)
                .checked_div(state.content_h.max(1) as usize)
                .unwrap_or(1)
                .max(1)
                .min(area.height as usize);
            let travel = area.height as usize - thumb_h;
            let thumb_y = (travel.saturating_mul(state.offset_y as usize))
                .checked_div(max_off)
                .unwrap_or(0);
            for dy in 0..thumb_h {
                let y = area.y + thumb_y as u16 + dy as u16;
                if y < area.bottom() {
                    buffer[(x, y)].set_char('█').set_style(thumb);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, KeyModifiers};

    #[test]
    fn scroll_clamps_and_pages() {
        let mut s = ScrollAreaState::new();
        s.set_content_size(10, 100);
        s.set_viewport(10, 10);
        assert!(s.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)));
        assert_eq!(s.offset_y(), 10);
        assert!(s.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)));
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
    }
}
