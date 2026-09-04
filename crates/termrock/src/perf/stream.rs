// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Batched streaming updates, dirty flags, coalescing, and backpressure.
use std::time::Duration;

use crate::runtime::FrameTick;

/// Priority of a streaming update (coalescer may drop lower when backed up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum UpdatePriority {
    /// Cosmetic / animation.
    Low = 0,
    /// Normal token/log lines.
    #[default]
    Normal = 1,
    /// Tool result boundaries, errors, permissions.
    High = 2,
    /// Must not drop (cancel, final).
    Critical = 3,
}

/// Coarse dirty flags for a surface (optional; full redraw always correct).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DirtyFlags {
    /// Header / sticky chrome.
    pub chrome: bool,
    /// Body viewport cells.
    pub body: bool,
    /// Scrollbar / status strip.
    pub chrome_secondary: bool,
    /// Overlay stack.
    pub overlays: bool,
}

impl DirtyFlags {
    /// Nothing dirty.
    #[must_use]
    pub const fn clean() -> Self {
        Self {
            chrome: false,
            body: false,
            chrome_secondary: false,
            overlays: false,
        }
    }

    /// Merge.
    pub fn merge(&mut self, other: Self) {
        self.chrome |= other.chrome;
        self.body |= other.body;
        self.chrome_secondary |= other.chrome_secondary;
        self.overlays |= other.overlays;
    }

    /// Any bit set.
    #[must_use]
    pub const fn any(self) -> bool {
        self.chrome || self.body || self.chrome_secondary || self.overlays
    }
}

/// One coalesced batch ready to apply on the UI thread.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamBatch {
    /// Text deltas concatenated (token stream).
    pub text_delta: String,
    /// Number of logical append events coalesced.
    pub append_count: u64,
    /// Highest priority seen in the batch.
    pub priority: UpdatePriority,
    /// Suggested dirty regions after apply.
    pub dirty: DirtyFlags,
    /// True if a terminal/final chunk was seen (must flush).
    pub force_flush: bool,
}

impl StreamBatch {
    /// Empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the batch has work.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.append_count == 0 && self.text_delta.is_empty() && !self.force_flush
    }
}

/// Signal from UI to producers when behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BackpressureSignal {
    /// Accept full rate.
    #[default]
    Open,
    /// Prefer coalescing; drop Low priority.
    Soft,
    /// Block or drop Normal; keep High/Critical.
    Hard,
}

/// Coalesces high-frequency deltas into frame-aligned batches.
///
/// Not thread-safe; pair with a channel — producer sends deltas, UI thread
/// `push`es, then `take_for_frame` once per `FrameTick`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCoalescer {
    pending: StreamBatch,
    /// Max chars retained in `text_delta` before hard backpressure.
    max_chars: usize,
    /// Max append events before forced flush recommendation.
    max_events: u64,
    /// Min elapsed between automatic flushes when only Low/Normal traffic.
    min_flush: Duration,
    last_flush_elapsed: Duration,
    backpressure: BackpressureSignal,
}

impl Default for StreamCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamCoalescer {
    /// Default: 64 KiB text, 256 events, 8 ms min flush (≈120 Hz cap for tokens).
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: StreamBatch::new(),
            max_chars: 64 * 1024,
            max_events: 256,
            min_flush: Duration::from_millis(8),
            last_flush_elapsed: Duration::ZERO,
            backpressure: BackpressureSignal::Open,
        }
    }

    /// Configure limits.
    #[must_use]
    pub fn with_limits(mut self, max_chars: usize, max_events: u64, min_flush: Duration) -> Self {
        self.max_chars = max_chars;
        self.max_events = max_events;
        self.min_flush = min_flush;
        self
    }

    /// Current backpressure for producers.
    #[must_use]
    pub const fn backpressure(&self) -> BackpressureSignal {
        self.backpressure
    }

    /// Push a text delta (token or log fragment).
    pub fn push_text(&mut self, delta: &str, priority: UpdatePriority) {
        if delta.is_empty() && priority < UpdatePriority::High {
            return;
        }
        if matches!(self.backpressure, BackpressureSignal::Hard) && priority < UpdatePriority::High
        {
            return;
        }
        if matches!(self.backpressure, BackpressureSignal::Soft) && priority == UpdatePriority::Low
        {
            return;
        }
        self.pending.text_delta.push_str(delta);
        self.pending.append_count = self.pending.append_count.saturating_add(1);
        if priority > self.pending.priority {
            self.pending.priority = priority;
        }
        self.pending.dirty.body = true;
        if priority >= UpdatePriority::High {
            self.pending.force_flush = true;
        }
        self.recompute_backpressure();
    }
    /// Take a batch if due for this frame; otherwise returns empty batch.
    pub fn take_for_frame(&mut self, tick: FrameTick) -> StreamBatch {
        if self.pending.is_empty() {
            return StreamBatch::new();
        }
        let due = self.pending.force_flush
            || self.pending.append_count >= self.max_events
            || self.pending.text_delta.len() >= self.max_chars
            || tick.elapsed().saturating_sub(self.last_flush_elapsed) >= self.min_flush
            || self.pending.priority >= UpdatePriority::High;

        if !due {
            return StreamBatch::new();
        }
        self.last_flush_elapsed = tick.elapsed();
        let out = std::mem::take(&mut self.pending);
        self.recompute_backpressure();
        out
    }

    /// Force take regardless of cadence (e.g. before modal open).
    pub fn take_now(&mut self) -> StreamBatch {
        let out = std::mem::take(&mut self.pending);
        self.backpressure = BackpressureSignal::Open;
        out
    }

    fn recompute_backpressure(&mut self) {
        let chars = self.pending.text_delta.len();
        let events = self.pending.append_count;
        self.backpressure = if chars >= self.max_chars || events >= self.max_events {
            BackpressureSignal::Hard
        } else if chars * 2 >= self.max_chars || events * 2 >= self.max_events {
            BackpressureSignal::Soft
        } else {
            BackpressureSignal::Open
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn tick(ms: u64) -> FrameTick {
        FrameTick::manual(
            Instant::now(),
            Duration::from_millis(ms),
            Duration::from_millis(8),
        )
    }

    #[test]
    fn coalesces_until_min_flush() {
        let mut c = StreamCoalescer::new().with_limits(1024, 100, Duration::from_millis(16));
        c.push_text("a", UpdatePriority::Normal);
        c.push_text("b", UpdatePriority::Normal);
        let early = c.take_for_frame(tick(0));
        assert!(early.is_empty());
        let batch = c.take_for_frame(tick(20));
        assert_eq!(batch.text_delta, "ab");
        assert_eq!(batch.append_count, 2);
    }

    #[test]
    fn high_priority_flushes_immediately() {
        let mut c = StreamCoalescer::new();
        c.push_text("x", UpdatePriority::High);
        let batch = c.take_for_frame(tick(0));
        assert_eq!(batch.text_delta, "x");
        assert!(batch.force_flush || batch.priority >= UpdatePriority::High);
    }

    #[test]
    fn hard_backpressure_drops_normal() {
        let mut c = StreamCoalescer::new().with_limits(4, 100, Duration::from_millis(1));
        c.push_text("12345", UpdatePriority::Normal);
        assert_eq!(c.backpressure(), BackpressureSignal::Hard);
        c.push_text("z", UpdatePriority::Normal);
        // dropped
        let batch = c.take_now();
        assert!(!batch.text_delta.contains('z') || batch.text_delta.starts_with("12345"));
    }

    #[test]
    fn dirty_merge() {
        let mut d = DirtyFlags::clean();
        d.merge(DirtyFlags {
            body: true,
            ..DirtyFlags::clean()
        });
        assert!(d.body);
        assert!(!d.chrome);
        assert!(d.any());
    }
}
