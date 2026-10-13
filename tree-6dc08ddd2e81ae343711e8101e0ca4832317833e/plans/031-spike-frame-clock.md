# Plan 031 (spike): Design the frame-clock/tick primitive — time for spinners, toast TTL, and blink without coupling to a scheduler

> **Executor instructions**: DESIGN SPIKE. Deliverable = design doc + a
> prototype validating two clients (Spinner advancement, Toast TTL) + a
> recommendation. Honor STOP conditions. Update the plans/README.md row when
> done.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/runtime/ crates/termrock-lookbook/src/main.rs`
> Plan 018's runner spike reshapes the lookbook loop and rules on `runtime` —
> its verdict is REQUIRED input. Verify its row is DONE and read
> `plans/018-runner-design.md` first.

## Status

- **Priority**: P3
- **Effort**: M (coarse — spike)
- **Risk**: MED for the eventual build (a wrong abstraction couples the library to a scheduler); LOW for the spike
- **Depends on**: plans/018-spike-runtime-disposition-app-runner.md (the runner owns the loop; the clock threads through it); plans/030 (Spinner exists as client #1 — soft dependency, can prototype against a branch-local spinner)
- **Category**: direction
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

No time exists anywhere in the library: `runtime`'s `Subscription`/`drive_frame` carry no elapsed/`dt`; grep for `Instant|Duration|elapsed|tick` across `runtime/` and `crossterm/` returns nothing (not even poll-with-timeout). Consequences: `Toast` has no TTL/auto-dismiss, `Spinner` (Plan 030) needs a hand-rolled counter, cursor blink is impossible, and the donor's digital-rain animation had to live entirely consumer-side. Every time-based behavior forces each consumer to build its own clock and loop-timeout plumbing. One neutral tick primitive unlocks the whole class — but it must stay executor-neutral and immediate-mode-friendly, or it becomes the scheduler coupling AGENTS.md forbids (consumers own "effects, process policy, executor choice").

## Current state

- `crates/termrock/src/runtime/subscription.rs` (verbatim core):

```rust
pub enum SubscriptionPoll<Event> { Ready(Event), Pending, Closed }
pub trait Subscription { type Output; fn poll_next(&mut self) -> SubscriptionPoll<Self::Output>; }
pub struct ClosureSubscription<F>(pub F);
pub struct StdSubscription<Event>(pub Receiver<Event>);
```

- `runtime/frame.rs` — `drive_frame(terminal, view, model, area, overlay)`: no time parameter.
- `Toast` (widgets/toast.rs): pure render, no TTL field. `Spinner` (post-030): `frame(n)`, caller-advanced.
- Plan 018's design doc (`plans/018-runner-design.md`) — the runner's verdicts on `Subscription`/`drive_frame` and its loop cadence (poll timeout) are the substrate this clock threads through. READ IT FIRST; if 018 deleted `Subscription`, the clock's delivery mechanism is whatever the runner uses instead.
- Date/time constraint: the library forbids nothing here, but determinism gates (lookbook SVG double-render) mean any story involving time must take the instant/frame as INPUT — same rule as `Spinner::frame(n)`.
- MSRV 1.95; `std::time::Instant` is fine; no async runtimes (executor-neutral).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Lookbook | `cargo run -p termrock-lookbook` | prototype behaviors visible |

## Scope

**In scope (spike)**:
- Design doc `plans/031-frame-clock-design.md`
- Prototype in the lookbook (and/or `#[cfg(test)]` modules) — Toast auto-dismiss + Spinner advancement running in the gallery loop
- API recommendation + build-plan stub

**Out of scope**:
- Shipping library changes (follow-up build plan).
- Animation easing/tween utilities (a later tier; note as future option only).
- Async timer integration (tokio etc. — consumer-owned forever).

## Steps

### Step 1: Design the time model — evaluate exactly three shapes

Write up with sketched Rust (compile the winner in the prototype):

1. **Pass-the-instant**: widgets/states that need time take `now: Instant` (or a newtype `FrameTime`) as a method argument — e.g. `ToastState::is_expired(now)`, spinner frame derived by the caller from `now`. No new machinery; maximum neutrality; N call sites thread `now`.
2. **TickSubscription + elapsed in the loop**: a `Subscription`-shaped timer source (`TickSubscription::every(Duration)` yielding `Tick { elapsed, now }`) + the runner (Plan 018) threading `elapsed` into the update callback. Library stays passive; the RUNNER owns the only clock read.
3. **FrameClock state object**: `FrameClock::start()`, `clock.tick() -> FrameTick { dt, now, frame_index }` called once per loop iteration by the consumer/runner; widgets take `&FrameTick` where needed. One canonical read per frame; testable via a manual `FrameTick` constructor.

Evaluation criteria (score in the doc): determinism/testability (can a test inject time? — 1 and 3 trivially, 2 via a fake subscription), coupling (2 depends on 018's subscription verdict), API noise (1 threads args everywhere), story-compatibility (previews must pin time — all three OK if widgets take time as data), and the AGENTS.md executor-neutrality rule. Note: 1 and 3 compose (3 produces what 1 consumes) — the likely winner is a hybrid: `FrameTick` as the value type, produced by runner-or-consumer, consumed as an argument (never read inside widgets).

### Step 2: Prototype the two clients

In the lookbook loop (post-018 runner): (a) Toast TTL — show a toast on some interactor action, auto-dismiss after ~2s using the winning shape (prototype a `ToastState { shown_at }` + `is_expired(tick)` locally); (b) Spinner — advance the Plan-030 spinner's frame from the tick (`frame = tick.now_millis / 100 % len`, or `frame_index`-based). Confirm the loop's poll cadence supports it (does the event poll block forever today? The runner design records the timeout — if it blocks indefinitely, ticks starve: THAT finding goes in the doc as the runner-integration requirement: poll timeout = min(next-deadline, default)).

**Verify**: both behaviors work in the gallery; SVG render/check unaffected (`check` green — time never reaches the story path); `cargo test --workspace` green.

### Step 3: Design doc + build-plan stub

`plans/031-frame-clock-design.md`: the three-shape evaluation + winner, the `FrameTick` type spec, the runner integration contract (poll-timeout rule from Step 2), the injectable-time testing pattern (manual `FrameTick` in tests — with example), the Toast TTL API sketch (`Toast` stays a render widget; `ToastState`/consumer owns `shown_at` — decide and record WHERE TTL state lives, honoring "state types own interaction facts"), retrofit list (Toast, Spinner, cursor blink as future), and open questions (fixed-timestep vs elapsed for animations; does `frame_index` belong in the type?).

**Verify**: doc exists; README row updated with the winner one-liner.

## Done criteria

- [x] `plans/031-frame-clock-design.md` with three-shape evaluation, winner spec, runner poll-timeout contract, testing pattern
- [x] Toast TTL + Spinner advancement demonstrably working in the lookbook; the prototype has since graduated into `FrameTick`, the runner, and `ToastState`
- [x] The spike preserved zero library-source changes; its later graduated implementation is tested and the workspace + preview gates are green
- [x] `plans/README.md` status row updated

## STOP conditions

- Plan 018 not DONE (no runner, no loop to thread time through) — stop, dependency.
- The event loop cannot support a poll timeout without runner redesign — that's a Plan-018 amendment; document and stop rather than hacking a busy-loop.
- The prototype pushes toward widgets reading `Instant::now()` internally — forbidden direction (breaks determinism gates and testability); the doc must record time-as-argument as a hard invariant.

## Maintenance notes

- Invariant for all future time work: **widgets never read the clock; time arrives as data.** The lookbook determinism gate is the enforcement backstop.
- The digital-rain-class animations stay consumer-side, but they consume the same `FrameTick` — mention in the doc as the neutrality proof.
- Cursor blink (TextInput) is the third retrofit client — defer until a consumer asks; the design must merely not preclude it.
