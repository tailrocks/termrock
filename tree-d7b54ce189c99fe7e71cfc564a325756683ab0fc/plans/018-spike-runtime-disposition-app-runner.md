# Plan 018 (spike): Prove or remove `runtime` — design the app-runner that absorbs the event-loop boilerplate

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan. The
> deliverable is a written design + a working prototype in the lookbook, and a
> recommendation. Follow steps in order; honor STOP conditions. When done,
> update the status row in `plans/README.md` and file the design doc as
> specified in Step 5.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/runtime/ crates/termrock-lookbook/src/main.rs crates/termrock/src/crossterm/`
> Plan 011 added neutral `input::Event` — REQUIRED before this spike. Verify
> its status row is DONE.

## Status

- **Priority**: P3
- **Effort**: M-L (coarse — spike scope)
- **Risk**: LOW (prototype in dev tooling; library changes only on accept)
- **Depends on**: plans/011-event-model-convergence.md
- **Category**: direction
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The `runtime` module (`Component`, `View`, `UpdateResult`, `Dirty`, `Subscription`, `drive_frame`) is public API with **zero real consumers**: no widget implements `Component`, nothing calls `drive_frame`, the `Subscription` impls are unused, and the crate's own AGENTS.md claims "downstream loops render through `runtime::drive_frame`" — a stale statement. Meanwhile every real consumer hand-writes the same loop: the lookbook's `run_terminal` is ~490 lines carrying `#[allow(clippy::too_many_lines)]`, with a hand-rolled `TerminalGuard` re-implementing raw-mode/alt-screen teardown even though `crossterm::Session` exists for exactly that, manual `event::poll/read`, manual focus routing, and manual hint-bar assembly from the keymap. Either the runtime contracts get proven by powering a real loop, or they get deleted (forward-only design forbids keeping unverified speculative surface). This spike decides — by building.

## Current state

- `runtime/contract.rs` (78 lines): `Dirty { Clean, Redraw }` + `merge`; `enum NoEffect {}`; `UpdateResult<Effect = NoEffect> { dirty, effects }` with `clean()/redraw()` constructors; `trait Component<Event, Message> { fn handle_event(&mut self, &Event) -> Option<Message>; }`; `trait View<Model>`.
- `runtime/frame.rs` (35 lines):

```rust
pub fn drive_frame<'a, B, Model, V, F>(terminal: &'a mut Terminal<B>, view: &V, model: &Model, area: Rect, overlay: F)
    -> Result<CompletedFrame<'a>, B::Error>
where B: Backend, V: View<Model>, F: FnOnce(&mut Frame<'_>),
{
    terminal.draw(|frame| { view.render(model, frame, area); overlay(frame); })
}
```

- `runtime/subscription.rs` (58 lines): `Subscription` trait + `ClosureSubscription`/`StdSubscription` — zero consumers (migration 0001 told consumers to wrap foreign receivers with `ClosureSubscription`; whether any external consumer does is unverifiable from here).
- `crates/termrock-lookbook/src/main.rs`: `run_terminal` ~lines 255-687 (the god function), `TerminalGuard` ~696-741 (duplicate lifecycle handling). This file is the prototype target.
- `crossterm::Session` (feature-gated): sole terminal lifecycle owner — raw mode, alt screen, mouse capture, bracketed paste, rollback on failed entry, idempotent restore (per `COMPONENTS.md`).
- Post-011: neutral `input::Event` exists (Key/Mouse/Resize/Focus), so a runner can be expressed backend-neutrally with a crossterm adapter behind the feature.
- Design constraint (root AGENTS.md): consumers own "effects, process policy, executor choice" — the runner must not own async runtimes or process management; it pumps events and frames, nothing else.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Lookbook runs | `cargo run -p termrock-lookbook` | interactive gallery works as before |
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope (spike)**:
- A design doc: `plans/018-runner-design.md` (created by this spike)
- Prototype changes inside `crates/termrock-lookbook/` only
- A go/no-go recommendation on `runtime`'s three parts (contracts, subscriptions, drive_frame)

**Out of scope (this spike)**:
- Changing `crates/termrock/src/` — the accepted design becomes a follow-up build plan.
- Async executors, tokio integration (consumer-owned per AGENTS.md).
- The lookbook knobs/theme-switcher features (Plan 020 — coordinate: both touch `main.rs`; run this first, 020 builds on the cleaned loop).

## Git workflow

- Directly on `main`; prototype commits like `spike(lookbook): drive the gallery through a runner prototype`. The design doc commits with `docs(plans)`.

## Steps

### Step 1: Inventory what a real loop needs

Read `run_terminal` end to end. Produce (in the design doc) the exact responsibility list it hand-rolls: session lifecycle, poll/read cadence (note the timeout it uses), event conversion, keymap dispatch, focus routing between panes, story-specific interactors, hint-bar assembly, redraw policy (does it redraw per event or per tick?), resize handling, quit path. Mark each as: (a) runner-owned, (b) app-callback, (c) stays app-specific.

### Step 2: Sketch the runner API against that inventory

Design target (adjust from evidence, don't force it): something like

```rust
// feature = "crossterm"
pub fn run<M>(
    session: SessionSpec,               // lifecycle config
    model: &mut M,
    view: impl Fn(&M, &mut Frame<'_>),  // or the View<M> trait — DECIDE with evidence
    update: impl FnMut(&mut M, input::Event) -> ControlFlow<()>,  // quit via Break
) -> io::Result<()>
```

Key design questions the doc must answer with evidence from Step 1: Does `Component<Event, Message>` earn its place, or is a plain `FnMut` closure the modern shape? Does `Dirty`/`UpdateResult` pay for itself (measure: does the lookbook redraw unconditionally today)? Do `Subscription`s model anything the loop needs (timers? external wakeups?) or is `event::poll(timeout)` sufficient? What is the minimal tick/timer story (the lookbook may animate — check)?

### Step 3: Prototype in the lookbook

Rewrite `run_terminal` on the sketched runner (implemented locally in the lookbook crate for now — e.g. `lookbook/src/runner.rs`), using `crossterm::Session` instead of `TerminalGuard`. Success = `run_terminal` body shrinks to construction + an `update` closure + story-specific interactor dispatch; the `#[allow(clippy::too_many_lines)]` comes OFF. Behavior must be identical (manual spot-check of navigation, interactors, quit, terminal restore; SVG render/check unaffected since it doesn't go through the loop).

**Verify**: `cargo run -p termrock-lookbook` behaves as before; `cargo test --workspace --all-features --locked` green; the too-many-lines allow is gone.

### Step 4: Render the verdict on `runtime`

With the prototype as evidence, recommend per item: `drive_frame` (subsumed by the runner? keep? delete?), `Component`/`View` (adopted by the runner or deleted), `UpdateResult`/`Dirty`/`NoEffect` (used or deleted), `Subscription` + impls (used or deleted). Deletion recommendations cite the prototype ("the runner needed X, never needed Y"). Also state where the runner lives: `termrock::runtime::run` behind `crossterm` feature (likely) vs a new `termrock-runner` crate (only if the feature graph demands it).

### Step 5: Write the design doc + follow-up plan stub

`plans/018-runner-design.md`: inventory table (Step 1), chosen API with signatures, the four verdicts (Step 4), migration sketch for the AGENTS.md stale claim (the crate doc must be corrected in the build plan), and open questions for the maintainer (e.g. tick/animation cadence policy). Keep the lookbook prototype committed — it IS the reference implementation the build plan graduates into the library.

**Verify**: doc exists; `plans/README.md` row updated to DONE with a one-line verdict summary.

## Test plan

- Spike-level: lookbook behaves identically (manual checklist in the design doc: navigate stories, run one interactor, resize, quit, terminal restored).
- The follow-up build plan (not this spike) owns library-level tests.

## Done criteria

- [x] `plans/018-runner-design.md` exists with inventory, API, four verdicts, open questions
- [x] Spike commit `2031822` moved the lookbook through its prototype runner; `TerminalGuard` and too-many-lines allowances were deleted
- [x] `cargo test --workspace --all-features --locked` → all pass
- [x] Spike commit `2031822` changed only lookbook and plan files; graduation later implemented the accepted design under `crates/termrock/src/runtime`
- [x] `plans/README.md` status row updated; the closure runner is now graduated and extended with immutable frame time

## STOP conditions

- Plan 011's neutral `Event` is not merged — stop, dependency.
- The lookbook loop turns out to need capabilities `Session` cannot provide (e.g. inline mode quirks) — document in the design doc and continue with `Session` extended-scope notes; do NOT fork another guard.
- The prototype grows past ~300 lines of runner code — the abstraction is fighting reality; write that finding down and stop prototyping (a negative result is a valid spike outcome).

## Maintenance notes

- Until the follow-up build plan lands, `runtime`'s public surface stays as-is — the spike changes nothing under `crates/termrock/src/`.
- Plan 020 (storybook) should start from the post-spike lookbook loop.
- The stale AGENTS.md claim ("downstream loops render through runtime::drive_frame") gets corrected by the build plan that implements the verdict — note it there explicitly.
