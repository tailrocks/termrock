# Frame-clock design: immutable time data, one read per frame

## Recommendation

Graduate a hybrid of pass-the-instant and a frame-clock value. The runner or
consumer owns a `FrameClock`, reads the monotonic clock exactly once per frame,
and passes one immutable `FrameTick` to update and render callbacks. Widgets and
widget states consume that value as an argument. They never call
`Instant::now()`.

The lookbook prototype implements this shape locally in `runner.rs`. Its live
header advances `ProgressKind::Indeterminate` from elapsed time. Changing a
Toast story knob starts a lookbook-owned two-second notification whose state is
tested with manual ticks. Static SVG stories receive no clock and remain byte
deterministic.

## Exactly three evaluated shapes

Scores are 1–5; higher is better.

| Shape | Determinism | Low coupling | Low API noise | Story compatibility | Executor neutrality | Total |
|---|---:|---:|---:|---:|---:|---:|
| 1. Pass `Instant`/`FrameTime` directly | 5 | 5 | 2 | 5 | 5 | 22 |
| 2. `TickSubscription` yielding elapsed time | 3 | 2 | 3 | 5 | 4 | 17 |
| 3. Consumer/runner-owned `FrameClock` producing `FrameTick` | 5 | 4 | 4 | 5 | 5 | 23 |

### 1. Pass the instant

```rust
fn is_expired(&self, now: Instant) -> bool;
fn spinner_step(now: Instant, started_at: Instant) -> u64;
```

This is maximally passive and trivially injectable. It makes every client own
start-time and delta bookkeeping, repeats clock-threading conventions, and
cannot guarantee one read per rendered frame. Keep its core rule—time is an
argument—but centralize the sampled value.

### 2. Tick subscription

```rust
TickSubscription::every(Duration::from_millis(100));
// yields Tick { now, elapsed }
```

Plan 018 rejected and scheduled deletion of the unused `Subscription` family.
Reintroducing it for a timer would couple a pure time value to source polling,
fairness, wakeups, closure/receiver adapters, and runner scheduling. Fake
subscriptions can test it, but with more machinery than a manual value. Reject.

### 3. Frame clock and value

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTick {
    now: Instant,
    elapsed: Duration,
    delta: Duration,
}

impl FrameTick {
    pub const fn manual(
        now: Instant,
        elapsed: Duration,
        delta: Duration,
    ) -> Self;
    pub const fn now(self) -> Instant;
    pub const fn elapsed(self) -> Duration;
    pub const fn delta(self) -> Duration;
}
```

`FrameClock` owns `started_at` and `previous_at`. `tick()` reads
`Instant::now()` once and computes saturating elapsed/delta durations.
`tick_at(now)` is the test seam and may remain crate-private. The public value
constructor is deliberately manual so tests, replay tools, and alternative
executors can inject time without a clock object.

`tick_at` clamps an out-of-order sample to `previous_at`. A backward source
therefore yields zero delta and cannot rewind elapsed time or inflate the next
forward delta.

Do not include `frame_index`. Render count changes with terminal input,
backpressure, redraw policy, and machine speed. Animations derived from it
speed up or slow down accidentally. Discrete frames derive from elapsed time:

```rust
let spinner_tick = u64::try_from(tick.elapsed().as_millis() / 100)
    .unwrap_or(u64::MAX);
```

This is the winner. It composes shape 1's time-as-data contract with one
canonical producer and avoids shape 2's scheduler abstraction.

## Runner integration contract

The existing prototype runner uses `event::poll(Duration::from_millis(120))`;
it does not block indefinitely. A tick is sampled before each draw, then the
same value is passed to render and any event update from that poll cycle. The
maximum action timestamp skew is one poll interval and the next frame corrects
it. No busy-loop or Plan-018 redesign is required.

The graduated runner should accept a tick in both callbacks:

```rust
render: impl FnMut(&mut Model, &mut Frame<'_>, FrameTick),
update: impl FnMut(&mut Model, input::Event, FrameTick) -> ControlFlow<()>,
```

Polling must obey:

```text
timeout = min(configured_idle_timeout, time_until_next_deadline)
```

With no deadline, use the configured idle timeout. With an overdue deadline,
use zero once, render/update, then compute a new future deadline; never spin on
the same overdue value. The first build can preserve the proven 120 ms idle
timeout. Later on-event redraw policies must register animation/TTL deadlines,
otherwise time clients starve.

`FrameClock` is synchronous infrastructure, not an executor. Tokio, async
timers, threads, external event multiplexing, and process policy remain
consumer-owned. A consumer-supplied tick works identically.

## Prototype clients

### Spinner advancement

The interactive lookbook header renders the shipped unified `Progress` in
indeterminate mode. Its phase is `elapsed / 100 ms`; SVG story rendering does
not call the runner and stays pinned. The header also shows the sampled delta,
making stalled or irregular frames observable during the spike.

### Toast TTL

The local `PrototypeToastState` owns `shown_at: Option<Instant>`. A successful
Toast interactor knob edit calls `show(tick)`. Render checks
`tick.now() - shown_at < 2s` and overlays a success Toast while true. Tests pin
visibility at 1,999 ms and expiry at exactly 2,000 ms using manual ticks.

For the library build, `Toast` remains a borrowed render widget. A domain-
neutral `ToastState` should own visibility timing because `shown_at`, TTL, and
dismissal are interaction facts. Consumers still own message wording,
notification queues, trigger policy, and effects:

```rust
pub struct ToastState {
    shown_at: Option<Instant>,
    ttl: Option<Duration>,
}

impl ToastState {
    pub fn show(&mut self, tick: FrameTick);
    pub fn dismiss(&mut self);
    pub fn is_visible(&self, tick: FrameTick) -> bool;
    pub fn next_deadline(&self) -> Option<Instant>;
}
```

## Injectable testing pattern

```rust
let start = Instant::now();
let shown = FrameTick::manual(start, Duration::ZERO, Duration::ZERO);
let before = FrameTick::manual(
    start + Duration::from_millis(1_999),
    Duration::from_millis(1_999),
    Duration::from_millis(1_999),
);
let expired = FrameTick::manual(
    start + Duration::from_secs(2),
    Duration::from_secs(2),
    Duration::from_millis(1),
);

state.show(shown);
assert!(state.is_visible(before));
assert!(!state.is_visible(expired));
```

Tests never sleep. Golden stories use explicit manual ticks or remain timeless.

## Retrofit list

1. `ProgressKind::Indeterminate`: consumers derive its existing `tick` from
   `FrameTick::elapsed`; no widget API change required.
2. `Toast`: add state-owned optional TTL and deadline reporting; keep rendering
   pure.
3. `TextInput` cursor blink: future state can derive phase from elapsed time;
   defer until a consumer requires it.
4. Consumer-owned digital-rain/easing/tween systems: consume the same tick,
   proving the primitive does not pull product animation into TermRock.

## Build-plan stub

1. Add `runtime::FrameTick` and the runner-private `FrameClock`; test manual,
   first, subsequent, and saturating/out-of-order ticks.
2. Extend the crossterm runner callbacks with `FrameTick`; move the lookbook off
   its local clock prototype and document the 120 ms/default-deadline rule.
3. Add `ToastState` with explicit `show`, `dismiss`, `is_visible`, and
   `next_deadline`; add deterministic boundary tests and catalog interaction.
4. Document deriving `ProgressKind::Indeterminate::tick` from elapsed time.
5. Add the next migration for runner callback and Toast state changes,
   regenerate public API/docs/previews, run the full gate, and perform the PTY
   restoration checklist.

## Resolved and open questions

- **Resolved:** `frame_index` does not belong in `FrameTick`; elapsed time is
  stable across redraw policies.
- **Resolved:** widgets never read clocks. This is a hard invariant.
- **Resolved:** TTL state belongs in `ToastState`, not the render widget or
  runner.
- **Open:** animations may choose variable elapsed time or implement their own
  fixed-step accumulator from `delta`; the primitive should not impose either.
- **Open:** deadline aggregation across many states belongs in the runner build
  only after a real second deadline client demonstrates the minimal interface.
- **Open:** decide whether `FrameClock` itself is public after consumers prove a
  need; `FrameTick::manual` plus runner delivery is sufficient today.
