// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Follow-tail, scroll anchors, and new-content indicators.
//!
//! Used by transcripts, logs, process output, and agent threads.

use crate::widgets::VirtualWindow;

/// Whether the viewport sticks to the end as content grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FollowMode {
    /// Do not auto-scroll on append.
    #[default]
    Paused,
    /// Keep the end of content visible when content grows.
    Following,
}

/// Kind of stable scroll anchor (survives inserts above/below when possible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScrollAnchorKind {
    /// Logical row/line index in the current projection.
    Index,
    /// Stable content id (message/block id); consumer maps id → index.
    ContentId,
    /// Pixel/row offset from content end (for follow).
    FromEnd,
}

/// Stable anchor for resume after resize or partial reproject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollAnchor {
    /// Kind.
    pub kind: ScrollAnchorKind,
    /// Index when `kind == Index` or distance-from-end when `FromEnd`.
    pub index: u64,
    /// Content id when `kind == ContentId`.
    pub content_id: Option<String>,
    /// Fraction of the anchor row visible from the top (0.0–1.0), optional.
    pub row_bias: u16,
}

impl ScrollAnchor {
    /// Anchor at logical index.
    #[must_use]
    pub const fn at_index(index: u64) -> Self {
        Self {
            kind: ScrollAnchorKind::Index,
            index,
            content_id: None,
            row_bias: 0,
        }
    }

    /// Anchor for follow-tail (0 = stuck to end).
    #[must_use]
    pub const fn from_end(distance: u64) -> Self {
        Self {
            kind: ScrollAnchorKind::FromEnd,
            index: distance,
            content_id: None,
            row_bias: 0,
        }
    }

    /// Content-id anchor.
    #[must_use]
    pub fn content_id(id: impl Into<String>) -> Self {
        Self {
            kind: ScrollAnchorKind::ContentId,
            index: 0,
            content_id: Some(id.into()),
            row_bias: 0,
        }
    }
}

/// Badge/chip when follow is paused and new lines arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NewContentIndicator {
    /// Unseen lines/blocks since pause.
    pub unseen: u64,
    /// Whether the indicator should paint.
    pub visible: bool,
}

impl NewContentIndicator {
    /// Clear after user jumps to end or resumes follow.
    pub fn clear(&mut self) {
        self.unseen = 0;
        self.visible = false;
    }

    /// Record `n` new items while paused.
    pub fn note_appended(&mut self, n: u64, follow: FollowMode) {
        if n == 0 {
            return;
        }
        if matches!(follow, FollowMode::Paused) {
            self.unseen = self.unseen.saturating_add(n);
            self.visible = self.unseen > 0;
        } else {
            self.clear();
        }
    }
}

/// After appending `appended` items, update window + follow + indicator.
///
/// `logical_len` is the new total length. Returns whether offset changed.
pub fn apply_follow_after_append(
    window: &mut VirtualWindow,
    follow: FollowMode,
    indicator: &mut NewContentIndicator,
    logical_len: u64,
    appended: u64,
) -> bool {
    window.logical_len = logical_len;
    indicator.note_appended(appended, follow);
    match follow {
        FollowMode::Paused => {
            window.clamp();
            false
        }
        FollowMode::Following => {
            let max = window.max_offset();
            let changed = window.offset != max;
            window.offset = max;
            indicator.clear();
            changed
        }
    }
}

/// User scroll interaction: pause follow (does not clear indicator).
#[must_use]
pub const fn pause_follow_on_user_scroll(follow: FollowMode) -> FollowMode {
    let _ = follow;
    FollowMode::Paused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_jumps_to_end_on_append() {
        let mut w = VirtualWindow::new(100, 10);
        w.offset = 0;
        let mut ind = NewContentIndicator::default();
        let changed = apply_follow_after_append(&mut w, FollowMode::Following, &mut ind, 120, 20);
        assert!(changed);
        assert_eq!(w.offset, w.max_offset());
        assert!(!ind.visible);
    }

    #[test]
    fn paused_tracks_unseen() {
        let mut w = VirtualWindow::new(100, 10);
        w.offset = 50;
        let mut ind = NewContentIndicator::default();
        let changed = apply_follow_after_append(&mut w, FollowMode::Paused, &mut ind, 130, 30);
        assert!(!changed);
        assert_eq!(w.offset, 50);
        assert_eq!(ind.unseen, 30);
        assert!(ind.visible);
    }

    #[test]
    fn user_scroll_pauses() {
        assert_eq!(
            pause_follow_on_user_scroll(FollowMode::Following),
            FollowMode::Paused
        );
    }
}
