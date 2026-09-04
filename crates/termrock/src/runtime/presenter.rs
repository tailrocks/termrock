// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The single seam that owns the wire.
//!
//! `docs/design/tui-motion-system.md` §2 is the law this module implements:
//! dirty coalescing, in-flight backpressure, a minimum draw interval, cursor
//! de-duplication, a demand-driven frame-rate ladder, and a scroll flush clock
//! that does **not** ride the animation tick.
//!
//! Nothing here touches a terminal — the presenter decides *whether* to draw
//! and *when to wake*; the backend adapter performs the draw. That keeps the
//! policy testable without a TTY and backend-neutral by construction.
use std::time::Duration;

use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};

use super::Instant;

/// Frame-rate rungs (motion SoT §2 rule 4).
///
/// The rate is recomputed every frame from what actually animates, so an idle
/// screen costs zero frames and zero bytes. Ordering is meaningful:
/// [`TickLadder::request`] keeps the highest rung asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum FrameRate {
    /// Nothing animates: no timed wakeup at all.
    #[default]
    Idle,
    /// Slow ambient loops (breathing, heartbeat) — 12 fps.
    Ambient,
    /// Spinners, streaming, transitions — 30 fps.
    Active,
    /// Reserved for input-driven bursts that must not drop frames — 60 fps.
    Ceiling,
}

impl FrameRate {
    /// Frames per second for this rung (`0` for [`Self::Idle`]).
    #[must_use]
    pub const fn fps(self) -> u16 {
        match self {
            Self::Idle => 0,
            Self::Ambient => 12,
            Self::Active => 30,
            Self::Ceiling => 60,
        }
    }

    /// Interval between frames, or `None` when nothing should wake the loop.
    #[must_use]
    pub const fn interval(self) -> Option<Duration> {
        match self {
            Self::Idle => None,
            Self::Ambient => Some(Duration::from_millis(1000 / 12)),
            Self::Active => Some(Duration::from_millis(1000 / 30)),
            Self::Ceiling => Some(Duration::from_millis(1000 / 60)),
        }
    }
}

/// Per-frame accumulator of animation demand.
///
/// Widgets and hosts request a rung; the ladder keeps the highest. It is reset
/// each frame so a finished animation immediately stops costing frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TickLadder {
    rate: FrameRate,
}

impl TickLadder {
    /// Nothing registered.
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            rate: FrameRate::Idle,
        }
    }

    /// Register demand; the highest rung this frame wins.
    pub const fn request(&mut self, rate: FrameRate) {
        if (rate as u8) > (self.rate as u8) {
            self.rate = rate;
        }
    }

    /// Highest rung registered.
    #[must_use]
    pub const fn rate(self) -> FrameRate {
        self.rate
    }

    /// Interval for the registered rung.
    #[must_use]
    pub const fn interval(self) -> Option<Duration> {
        self.rate.interval()
    }

    /// Drop back to idle (start of a frame).
    pub const fn reset(&mut self) {
        self.rate = FrameRate::Idle;
    }
}

/// Scroll flush cadence, deliberately independent of the animation ladder.
///
/// Riding the ambient tick makes scrolling visibly step (motion SoT §1, a
/// documented bug in the reference implementation).
pub const SCROLL_FLUSH_INTERVAL: Duration = Duration::from_millis(16);

/// Tracks whether a scroll flush is owed and when it is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollClock {
    pending: bool,
    last_flush: Option<Instant>,
}

impl ScrollClock {
    /// No scroll pending.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pending: false,
            last_flush: None,
        }
    }

    /// Record scroll input; many events between flushes coalesce into one.
    pub const fn request(&mut self) {
        self.pending = true;
    }

    /// Whether a scroll flush is owed now.
    #[must_use]
    pub fn due(&self, now: Instant) -> bool {
        self.pending
            && self
                .last_flush
                .is_none_or(|last| now.saturating_duration_since(last) >= SCROLL_FLUSH_INTERVAL)
    }

    /// Record that the owed flush was painted.
    pub const fn flushed(&mut self, now: Instant) {
        self.pending = false;
        self.last_flush = Some(now);
    }

    /// When the owed flush becomes due (`None` when nothing is pending).
    #[must_use]
    pub fn next_deadline(&self, now: Instant) -> Option<Instant> {
        if !self.pending {
            return None;
        }
        Some(match self.last_flush {
            Some(last) => (last + SCROLL_FLUSH_INTERVAL).max(now),
            None => now,
        })
    }
}

/// Default floor between draws — one 60 fps frame.
pub const DEFAULT_MIN_DRAW_INTERVAL: Duration = Duration::from_millis(1000 / 60);

/// Owns the decision to draw: coalescing, backpressure, throttle, cursor de-dup.
///
/// One presenter per application loop. Per-widget writes bypass every guarantee
/// here and are a defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presenter {
    dirty: bool,
    in_flight: bool,
    last_draw: Option<Instant>,
    min_interval: Duration,
    cursor: Option<(u16, u16)>,
    cursor_known: bool,
    scroll: ScrollClock,
    ladder: TickLadder,
}

impl Default for Presenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Presenter {
    /// A presenter owing its first frame.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            // The first frame is always owed: nothing has been painted yet.
            dirty: true,
            in_flight: false,
            last_draw: None,
            min_interval: DEFAULT_MIN_DRAW_INTERVAL,
            cursor: None,
            cursor_known: false,
            scroll: ScrollClock::new(),
            ladder: TickLadder::idle(),
        }
    }

    /// Override the minimum interval between draws.
    #[must_use]
    pub const fn min_draw_interval(mut self, interval: Duration) -> Self {
        self.min_interval = interval;
        self
    }

    /// Content changed; a frame is owed.
    pub const fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Scroll input arrived; the 16 ms clock owes a flush.
    pub const fn mark_scrolled(&mut self) {
        self.scroll.request();
    }

    /// Register animation demand for the coming frame.
    pub const fn request_rate(&mut self, rate: FrameRate) {
        self.ladder.request(rate);
    }

    /// Registered rung.
    #[must_use]
    pub const fn rate(&self) -> FrameRate {
        self.ladder.rate()
    }

    /// Whether a frame is owed right now.
    ///
    /// Backpressure first: while frame N is unflushed, N+1 is never queued.
    /// Then the throttle, then the three reasons to paint — dirty content, an
    /// owed scroll flush, or an animation interval that has elapsed.
    #[must_use]
    pub fn should_draw(&self, now: Instant) -> bool {
        if self.in_flight {
            return false;
        }
        let Some(last) = self.last_draw else {
            return true;
        };
        let since = now.saturating_duration_since(last);
        if since < self.min_interval {
            return false;
        }
        if self.dirty || self.scroll.due(now) {
            return true;
        }
        self.ladder
            .interval()
            .is_some_and(|interval| since >= interval)
    }

    /// Take ownership of the frame: clears dirty, marks in-flight.
    pub const fn begin_draw(&mut self, now: Instant) {
        self.dirty = false;
        self.in_flight = true;
        self.last_draw = Some(now);
    }

    /// Frame reached the wire; release backpressure and settle the scroll clock.
    pub const fn end_draw(&mut self, now: Instant) {
        self.in_flight = false;
        self.scroll.flushed(now);
        self.ladder.reset();
    }

    /// Whether moving the cursor to `position` needs to emit bytes.
    ///
    /// Repeating the same position costs bytes and restarts the hardware blink,
    /// which reads as a flickering caret.
    pub fn cursor_needs_move(&mut self, position: Option<(u16, u16)>) -> bool {
        if self.cursor_known && self.cursor == position {
            return false;
        }
        self.cursor_known = true;
        self.cursor = position;
        true
    }

    /// How long the loop may block before the next owed frame.
    ///
    /// `None` means "wait for input" — an idle screen schedules no wakeup at
    /// all, which is the whole point of the ladder.
    #[must_use]
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        if self.dirty {
            // Wait out the throttle rather than spinning on a zero timeout.
            return Some(self.last_draw.map_or(Duration::ZERO, |last| {
                self.min_interval
                    .saturating_sub(now.saturating_duration_since(last))
            }));
        }
        let scroll = self
            .scroll
            .next_deadline(now)
            .map(|at| at.saturating_duration_since(now));
        let animation = self.ladder.interval().map(|interval| match self.last_draw {
            Some(last) => interval.saturating_sub(now.saturating_duration_since(last)),
            None => Duration::ZERO,
        });
        match (scroll, animation) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(only), None) | (None, Some(only)) => Some(only),
            (None, None) => None,
        }
    }
}

/// Backend wrapper that makes an unchanged frame cost zero bytes.
///
/// Two leaks, both from `Terminal::draw` doing the same work every frame
/// regardless of the diff (motion SoT §2 rule 1, and the cursor de-dup clause
/// in rule 3):
///
/// 1. **Empty diffs still reach the backend**, and the crossterm backend closes
///    every `draw` with a style-reset epilogue — 19 bytes for a screen that did
///    not change. This wrapper drops the call when the content iterator is
///    empty.
/// 2. **Cursor commands are re-issued** every frame. Those bytes restart the
///    hardware blink, so the caret visibly stutters. This wrapper drops
///    hide/show/position commands the terminal is already obeying.
///
/// Wrap any backend once, at the top of the loop:
///
/// ```rust,ignore
/// let backend = QuietBackend::new(CrosstermBackend::new(writer));
/// let mut terminal = Terminal::new(backend)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuietBackend<B> {
    inner: B,
    /// `None` until the first command tells us what the terminal is doing.
    hidden: Option<bool>,
    position: Option<Position>,
}

impl<B> QuietBackend<B> {
    /// Wrap a backend.
    pub const fn new(inner: B) -> Self {
        Self {
            inner,
            hidden: None,
            position: None,
        }
    }

    /// Borrow the wrapped backend.
    pub const fn inner(&self) -> &B {
        &self.inner
    }

    /// Mutably borrow the wrapped backend.
    pub const fn inner_mut(&mut self) -> &mut B {
        &mut self.inner
    }

    /// Unwrap.
    pub fn into_inner(self) -> B {
        self.inner
    }

    /// Forget what we believe about the terminal's cursor.
    ///
    /// Call after anything that moves the cursor behind our back (a resize, a
    /// suspend/resume, host-emitted escape sequences).
    pub const fn invalidate(&mut self) {
        self.hidden = None;
        self.position = None;
    }
}

impl<B: Backend> Backend for QuietBackend<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut content = content.peekable();
        if content.peek().is_none() {
            // Zero-diff frame: not even the backend's style-reset epilogue.
            return Ok(());
        }
        // Painting cells leaves the cursor wherever the last cell was.
        self.position = None;
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.position = None;
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        if self.hidden == Some(true) {
            return Ok(());
        }
        self.inner.hide_cursor()?;
        self.hidden = Some(true);
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        if self.hidden == Some(false) {
            return Ok(());
        }
        self.inner.show_cursor()?;
        self.hidden = Some(false);
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        let position = self.inner.get_cursor_position()?;
        self.position = Some(position);
        Ok(position)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        if self.position == Some(position) {
            return Ok(());
        }
        self.inner.set_cursor_position(position)?;
        self.position = Some(position);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.invalidate();
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.invalidate();
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

// `Backend::scroll_region_up`/`_down` exist only when ratatui-core is built
// with its `scrolling-regions` feature. TermRock does not enable it, and the
// flag belongs to a dependency, so there is no honest `cfg` to gate a forward
// on from here; add the passthroughs (invalidating the cursor cache) together
// with a forwarding feature if TermRock ever adopts scrolling regions.

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn ladder_keeps_the_highest_rung_and_resets() {
        let mut ladder = TickLadder::idle();
        assert_eq!(ladder.rate(), FrameRate::Idle);
        assert_eq!(ladder.interval(), None);

        ladder.request(FrameRate::Ambient);
        ladder.request(FrameRate::Active);
        ladder.request(FrameRate::Ambient);
        assert_eq!(ladder.rate(), FrameRate::Active);
        assert_eq!(ladder.interval(), Some(Duration::from_millis(33)));

        ladder.reset();
        assert_eq!(ladder.rate(), FrameRate::Idle);
    }

    #[test]
    fn idle_presenter_schedules_no_wakeup_and_draws_nothing() {
        let start = Instant::now();
        let mut presenter = Presenter::new();

        // The first frame is owed, then the screen goes quiet.
        assert!(presenter.should_draw(start));
        presenter.begin_draw(start);
        presenter.end_draw(start);

        assert!(!presenter.should_draw(at(start, 1_000)));
        assert_eq!(presenter.next_wake(at(start, 1_000)), None);
        assert_eq!(presenter.rate(), FrameRate::Idle);
    }

    #[test]
    fn wheel_flood_coalesces_into_one_frame() {
        let start = Instant::now();
        let mut presenter = Presenter::new();
        presenter.begin_draw(start);
        presenter.end_draw(start);

        // A flood of wheel events inside one 16 ms window.
        for _ in 0..200 {
            presenter.mark_scrolled();
        }
        assert!(
            !presenter.should_draw(at(start, 4)),
            "scroll flush must wait for its own 16 ms cadence"
        );

        let due = at(start, 16);
        assert!(presenter.should_draw(due));
        presenter.begin_draw(due);
        presenter.end_draw(due);

        assert!(
            !presenter.should_draw(at(start, 32)),
            "one flush drains the whole flood — no ghost frames"
        );
    }

    #[test]
    fn in_flight_frame_blocks_the_next_one() {
        let start = Instant::now();
        let mut presenter = Presenter::new();
        presenter.begin_draw(start);
        presenter.mark_dirty();
        assert!(
            !presenter.should_draw(at(start, 100)),
            "N+1 must never queue while N is unflushed"
        );
        presenter.end_draw(at(start, 100));
        assert!(presenter.should_draw(at(start, 100)));
    }

    #[test]
    fn min_interval_throttles_a_dirty_storm() {
        let start = Instant::now();
        let mut presenter = Presenter::new().min_draw_interval(Duration::from_millis(16));
        presenter.begin_draw(start);
        presenter.end_draw(start);

        presenter.mark_dirty();
        assert!(!presenter.should_draw(at(start, 8)));
        assert!(presenter.should_draw(at(start, 16)));
    }

    #[test]
    fn throttled_dirty_frame_waits_instead_of_spinning() {
        let start = Instant::now();
        let mut presenter = Presenter::new().min_draw_interval(Duration::from_millis(16));
        presenter.begin_draw(start);
        presenter.end_draw(start);

        presenter.mark_dirty();
        assert_eq!(
            presenter.next_wake(at(start, 4)),
            Some(Duration::from_millis(12)),
            "a zero timeout here would busy-loop for the rest of the window"
        );
    }

    #[test]
    fn animation_rung_wakes_the_loop_on_its_own_interval() {
        let start = Instant::now();
        let mut presenter = Presenter::new();
        presenter.begin_draw(start);
        presenter.end_draw(start);

        presenter.request_rate(FrameRate::Ambient);
        assert_eq!(
            presenter.next_wake(start),
            Some(Duration::from_millis(1000 / 12))
        );
        assert!(!presenter.should_draw(at(start, 40)));
        assert!(presenter.should_draw(at(start, 83)));
    }

    #[test]
    fn scroll_clock_does_not_ride_the_animation_tick() {
        let start = Instant::now();
        let mut presenter = Presenter::new();
        presenter.begin_draw(start);
        presenter.end_draw(start);

        // Ambient animation alone would wake at ~83 ms; a scroll must not wait.
        presenter.request_rate(FrameRate::Ambient);
        presenter.mark_scrolled();
        assert_eq!(
            presenter.next_wake(start),
            Some(Duration::from_millis(16)),
            "scroll flush owns its own 16 ms cadence"
        );
    }

    #[test]
    fn cursor_moves_are_deduplicated() {
        let mut presenter = Presenter::new();
        assert!(presenter.cursor_needs_move(Some((4, 2))));
        assert!(!presenter.cursor_needs_move(Some((4, 2))));
        assert!(presenter.cursor_needs_move(Some((5, 2))));
        assert!(presenter.cursor_needs_move(None));
        assert!(!presenter.cursor_needs_move(None));
    }
}
