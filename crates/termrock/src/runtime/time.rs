use std::time::{Duration, Instant};

/// Immutable monotonic time sampled once for one application frame.
///
/// Hosts inject time via [`FrameTick::manual`] / [`FrameClock::tick_at`] for
/// tests, Studio replay, and deterministic snapshots. Never sample clocks
/// inside widgets during paint — pass the frame tick from the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTick {
    now: Instant,
    elapsed: Duration,
    delta: Duration,
}

impl FrameTick {
    /// Creates an injectable frame-time value for tests and alternative runners.
    pub const fn manual(now: Instant, elapsed: Duration, delta: Duration) -> Self {
        Self {
            now,
            elapsed,
            delta,
        }
    }

    /// Returns this frame's monotonic timestamp.
    pub const fn now(self) -> Instant {
        self.now
    }

    /// Returns monotonic time elapsed since the runner started.
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Returns monotonic time elapsed since the previous frame.
    pub const fn delta(self) -> Duration {
        self.delta
    }

    /// Elapsed as whole milliseconds (saturating).
    #[must_use]
    pub fn elapsed_ms(self) -> u64 {
        self.elapsed.as_millis() as u64
    }

    /// Delta as whole milliseconds (saturating).
    #[must_use]
    pub fn delta_ms(self) -> u64 {
        self.delta.as_millis() as u64
    }
}

/// Samples monotonic time once per frame for the application loop.
///
/// Public for hosts that run their own loop (or Studio record/replay). Prefer
/// [`FrameTick::manual`] in pure unit tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameClock {
    started_at: Instant,
    previous_at: Instant,
}

impl FrameClock {
    /// Start sampling from wall clock.
    #[must_use]
    pub fn start() -> Self {
        Self::from_start(Instant::now())
    }

    /// Start from an injected instant (tests / replay).
    #[must_use]
    pub const fn from_start(now: Instant) -> Self {
        Self {
            started_at: now,
            previous_at: now,
        }
    }

    /// Sample wall clock.
    pub fn tick(&mut self) -> FrameTick {
        self.tick_at(Instant::now())
    }

    /// Sample at `now` (monotonic clamp if clock steps backward).
    pub fn tick_at(&mut self, now: Instant) -> FrameTick {
        let now = now.max(self.previous_at);
        let tick = FrameTick::manual(
            now,
            now.saturating_duration_since(self.started_at),
            now.saturating_duration_since(self.previous_at),
        );
        self.previous_at = now;
        tick
    }

    /// Runner start instant.
    #[must_use]
    pub const fn started_at(&self) -> Instant {
        self.started_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_emits_monotonic_injectable_ticks() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let first = clock.tick_at(start);
        let second = clock.tick_at(start + Duration::from_millis(120));

        assert_eq!(first.elapsed(), Duration::ZERO);
        assert_eq!(first.delta(), Duration::ZERO);
        assert_eq!(second.elapsed(), Duration::from_millis(120));
        assert_eq!(second.delta(), Duration::from_millis(120));
    }

    #[test]
    fn clock_clamps_backward_samples_without_inflating_next_delta() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let forward = clock.tick_at(start + Duration::from_millis(120));
        let backward = clock.tick_at(start + Duration::from_millis(60));
        let resumed = clock.tick_at(start + Duration::from_millis(200));

        assert_eq!(backward.now(), forward.now());
        assert_eq!(backward.delta(), Duration::ZERO);
        assert_eq!(resumed.delta(), Duration::from_millis(80));
    }
}
