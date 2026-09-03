// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared index cursor + scroll-window follower for list-shaped patterns.
//!
//! Five list patterns (prompt queue, session picker, approval queue,
//! integration status, connection manager) carried byte-identical
//! cursor-clamp + window-follow logic — and the paint layer re-derived it a
//! second time per pattern, already drifted. [`CursorWindow`] is the single
//! definition. Id-based roving / typeahead / virtual windows live in
//! [`CollectionState`](super::CollectionState); this is the raw index form
//! for patterns that own their projection and emit domain outcomes.

/// Index cursor with a follow-scroll window (`[scroll, scroll + viewport)`).
///
/// Every mutation clamps the cursor into `0..len` and scrolls just enough to
/// keep it visible, so state is never stale for paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorWindow {
    cursor: usize,
    scroll: usize,
}

impl CursorWindow {
    /// Cursor and scroll at the top.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cursor: 0,
            scroll: 0,
        }
    }

    /// Keyboard / selection cursor index.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// First visible row index.
    #[must_use]
    pub const fn scroll(&self) -> usize {
        self.scroll
    }

    /// Clamps the cursor into `0..len` and keeps it inside the window.
    ///
    /// Call after any list mutation (insert, remove, refilter, reorder).
    pub fn clamp(&mut self, len: usize, viewport: usize) {
        if len == 0 {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }
        self.cursor = self.cursor.min(len - 1);
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + viewport {
            self.scroll = self.cursor + 1 - viewport;
        }
    }

    /// Moves the cursor by `delta` (clamped to `0..len`) and keeps it visible.
    ///
    /// Returns whether the cursor index changed; movement at an edge still
    /// clamps, and callers decide whether a no-op move emits an outcome.
    pub fn move_by(&mut self, delta: isize, len: usize, viewport: usize) -> bool {
        if len == 0 {
            return false;
        }
        let before = self.cursor;
        self.cursor = (self.cursor as isize + delta).clamp(0, len as isize - 1) as usize;
        self.clamp(len, viewport);
        self.cursor != before
    }

    /// Points the cursor at an absolute index and keeps it visible.
    pub fn set_cursor(&mut self, cursor: usize, len: usize, viewport: usize) {
        self.cursor = cursor;
        self.clamp(len, viewport);
    }

    /// Jumps to the first item (cursor and scroll to the top).
    pub fn move_first(&mut self, len: usize, viewport: usize) {
        self.cursor = 0;
        self.clamp(len, viewport);
    }

    /// Jumps to the last item.
    pub fn move_last(&mut self, len: usize, viewport: usize) {
        if len > 0 {
            self.cursor = len - 1;
        }
        self.clamp(len, viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_resets_on_empty_and_caps_cursor() {
        let mut w = CursorWindow::new();
        w.clamp(0, 4);
        assert_eq!((w.cursor(), w.scroll()), (0, 0));

        w.set_cursor(9, 3, 4);
        assert_eq!(w.cursor(), 2);
        assert_eq!(w.scroll(), 0);
    }

    #[test]
    fn scroll_follows_cursor_out_of_window() {
        let mut w = CursorWindow::new();
        w.set_cursor(7, 10, 4);
        assert_eq!(w.scroll(), 4);
        w.move_by(-1, 10, 4);
        // Still inside [4, 8): scroll does not move.
        assert_eq!((w.cursor(), w.scroll()), (6, 4));
        w.move_by(-3, 10, 4);
        assert_eq!((w.cursor(), w.scroll()), (3, 3));
        w.move_by(-10, 10, 4);
        assert_eq!((w.cursor(), w.scroll()), (0, 0));
        w.move_by(10, 10, 4);
        assert_eq!((w.cursor(), w.scroll()), (9, 6));
    }

    #[test]
    fn move_by_reports_change_and_edges_clamp() {
        let mut w = CursorWindow::new();
        w.set_cursor(2, 3, 4);
        assert!(w.move_by(-5, 3, 4)); // clamps to first
        assert_eq!(w.cursor(), 0);
        assert!(!w.move_by(-5, 3, 4));
        assert!(w.move_by(2, 3, 4)); // clamps to last
        assert_eq!(w.cursor(), 2);
        assert!(!w.move_by(5, 3, 4));
        assert!(!w.move_by(1, 0, 4));
    }

    #[test]
    fn move_first_and_last() {
        let mut w = CursorWindow::new();
        w.set_cursor(4, 20, 4);
        w.move_first(20, 4);
        assert_eq!((w.cursor(), w.scroll()), (0, 0));
        w.move_last(20, 4);
        assert_eq!((w.cursor(), w.scroll()), (19, 16));
        w.move_last(0, 4);
        assert_eq!(w.cursor(), 0);
    }
}
