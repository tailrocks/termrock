# Plan 014: Motion system — pipeline discipline first, then MotionChannel + shimmer

> **Executor instructions**: Follow this plan step by step. READ
> `docs/design/tui-motion-system.md` IN FULL before starting — it is the
> binding SoT this plan implements; its §5 easing/duration table and §7
> anti-patterns are law. Run every verification command. STOP conditions are
> binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: plans 002/007 DONE. Re-locate cited sites with
> `rg`; the motion/runtime modules may have moved — the SoT doc §8 table is
> the contract, file paths are leads.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED (touches the runtime presenter — regression tests mandatory)
- **Depends on**: plans/002 (roles), plans/007 (status vocabulary)
- **Category**: design-foundation / runtime
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

"The fluidity is 80% render-pipeline discipline + 20% tasteful effects.
Effects on a flickering pipeline read as jank, not polish"
(`tui-motion-system.md` §1). Flicker is the #1 cheap-TUI tell
(`web-premium-tui-law.md` P14). Current gaps (audit F3 +
`tui-motion-system.md` §8): `style/motion.rs` has only
`pulse_brightness`/`wave_brightness` — no `shimmer_cells` traveling sweep;
Skeleton pulses the whole block (`skeleton.rs:498-504`) instead of sweeping;
`LoadingView` takes a static frame; `Spinner` has no Stream/Done phases and
its reduced-motion fallback `●` collides with "done"; `StatusIndicator` has
no motion at all; no `MotionChannel` vocabulary, no `MotionPolicy` env
resolution, and the presenter-level guarantees (mode-2026 wrap, demand-driven
tick, separate scroll clock, backpressure) are unverified.

## Scope

**In scope**: `crates/termrock/src/style/motion.rs`,
`crates/termrock/src/runtime/` (presenter/tick/scroll clock),
`crates/termrock/src/perf/budget*`, widget consumers named in Step 4
(`skeleton.rs`, `spinner.rs`, `status_indicator.rs`, `view_state.rs`,
`loading_overlay.rs`, `toast.rs`, `timeline.rs`, `log_stream.rs`,
`log_pane.rs`), `design_gate.rs`, `migrations/` + `MIGRATING.md`.

**Out of scope**: Layer-2 buffer-effects engine (tachyonfx dep-vs-port —
Step 6 produces the decision memo, not the implementation); ambient garnish
composites (accent-rail wave etc. — patterns work, plan 016); braille
sub-cell canvas (P2 in the SoT).

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Git workflow

`main`; commit per step; `git commit -s`.

## Steps

### Step 1: Pipeline audit against §2 law (verify, then fix gaps)

For each of the seven §2 rules, locate the implementing code in `runtime/`
(rg leads: `SynchronizedUpdate`, `tick`, `scroll`, `Presenter`, `dirty`,
`min_draw`) and record PRESENT/ABSENT in the commit body. Fix the ABSENT
ones: (a) every frame wrapped in Begin/EndSynchronizedUpdate with DECRQM
detect + silent degrade; (b) demand-driven tick ladder
`None → Ambient(12) → Active(30) → Ceiling(60)` computed from registered
animations (idle = 0 fps); (c) scroll flush on its own 16 ms cadence;
(d) dirty coalescing + in-flight backpressure + cursor de-dup in one
Presenter. Port the two named regression tests
(`wheel_flood_paints_no_ghost_frames`, idle-zero-bytes).

**Verify**: new runtime tests pass; idle test asserts 0 emitted bytes on a
static frame; `mise run check` green.

### Step 2: `MotionPolicy` + `MotionChannel` + `shimmer_cells`

- `MotionPolicy { Full, Basic, Off }` resolved from
  `TERMROCK_ANIMATIONS=full|basic|none` + `REDUCE_MOTION`, overridable on
  `DesignSystem` (reconcile with the existing `Motion` type — one enum
  survives, migration documents the mapping).
- `MotionChannel { Work, Wait, Stream, Live, Static }` + the period table
  (Work ~80ms / Wait ~240ms / Stream ~120ms / Live ~2s breathe / heartbeat
  ~5s). Errors/done = Static (gravity — never animate).
- `shimmer_cells(tick, cols, period)` raised-cosine traveling band in
  `motion.rs` (amplitude ≤ 0.33 per §1 peak-restraint).
- Ambient loops phase on WALL CLOCK, transitions on tick counts (§4 glue
  rule) — expose both bases in the motion API.

**Verify**: unit tests: channel→period mapping; shimmer band travels
(distinct outputs across ticks) and is static under `Basic`/`Off`;
`mise run check` green.

### Step 3: Layer-1 value animation

Typed `Easing` enum (no `Elastic/Bounce/Back*` variants — forbidden by §5),
`Tween::to(value, (ms, easing))`, `Spring::new(freq≈18, damping=1)` for
retargeting values, `Animator::tick(dt)` iterator that sleeps when empty.
Wire scroll easing (`CubicOut`, 80-200ms distance-scaled) through it.

**Verify**: spring retarget test (no restart pop); animator-empty = no tick
demand (feeds Step 1's ladder).

### Step 3b: Transition seams (the audit's inventory — build these before Step 4)

The micro-interaction audit mapped exactly where time fails to reach paint;
wire these seams (each verified lead; re-locate with rg):
- `DesignSystem::at(tick: FrameTick)` — motion policy already rides
  `DesignSystem` (`style/tokens.rs:410`) into all ~143 widgets but only 6
  sites read it; adding the tick here gives every widget time with zero
  signature churn. Snapshot tests keep determinism via a frozen default
  tick. Remove `DrawerState`'s private `motion` copy (`drawer.rs:445,609`).
- `Presence` exit: all three constructors zero `exit_duration`
  (`runtime/motion.rs:168,179,190`) so `Exiting` is unreachable; add
  `with_exit(Duration)` + `phase_fraction(tick) -> f32`; stop gating exits
  on `animate_spinners()` (`:235`).
- `Backdrop::alpha(f32)` (`dialog.rs:503-513`) blending toward Canvas via
  the existing `fade_style`; `BackdropPolicy::Dim` carries target alpha;
  `OverlayStack` entries record opened-at.
- `StatusBar` mode cross-fade: `alpha` seam exists unused
  (`status_bar.rs:414,463,943-948`) — drive it from a new
  `mode_changed_at`.
- Collections filter fade: `revision: u64` + `revision_changed_at` on
  `CollectionState` (precedent: `combobox.rs:186` generation); row content
  fades 80ms on revision change; gutter still SNAPS (per §6 — current snap
  behavior at `tokens.rs:798` is correct, keep).
- Tabs: `TabsState` records `(previous, changed_at)`; active fill blends
  100ms via `blend_toward`; emit `Changed { previous }` so hosts can fade
  panels.
- Diff/review settle: per-view `painted_at`; tint blends in over 120ms at
  the two tint call sites (`diff.rs:1411,1448`; `review.rs:1810`). No
  per-line stagger.
- Focus border cross-fade: `focus_changed_at` on border-owning states;
  blend `Border → BorderFocused` inside `panel_recipe`/`input_recipe`
  (16 consumer files — recipe-level change, one place).
- `Motion` gains `allows_transitions()` + `clamp_duration(Duration)` (today
  it is spinner-shaped only, `density.rs:81-97`, forcing misuse).
- Determinate progress: keep `value` (target) + add sprung `displayed`;
  `is_active()` true while they differ (today determinate is excluded from
  animation, `progress.rs:341-356`); eighth-block ramp comes from plan 003.
- Scroll: split `target_offset` (integer, windowing) from `display_offset`
  (eased, paint origin) in `scroll_area`/`virtualizer`; `WheelAccumulator`
  (fractional lines + 80ms stream-gap) in `scroll/`; adopt the unused
  `edge_fade` (`style/motion.rs:42`, zero consumers) for top/bottom
  continuation cues. Requires Step 1's 16ms scroll clock FIRST (single
  poll loop today, `runner.rs:82-118`).
- Collapsible/Accordion settle: `resolved_open` returns `reveal_rows: u16`
  driven by `toggled_at` (`collapsible.rs:425-466`, `accordion.rs:554-610`);
  Tree stays snap-documented (host-projected rows — record the exception).
- Drawer doc drift: `drawer.rs:19` says "terminals do not slide-animate" —
  rewrite to "no slide GEOMETRY; fade required" so it stops contradicting
  §6.

**Verify**: each seam has a unit test proving (a) it animates under Full,
(b) two ticks under `Off` render identical buffers.

### Step 4: Widget contracts (§6 table)

- Skeleton: inert shape + `shimmer_cells` sweep; NEVER spinner frames — add
  the `shimmer_implies_no_spinner_frames` test.
- Spinner: phase→channel map; `Streaming` → Stream shimmer glyphs `∻≈∿〜`;
  Done → static `✓` morph; reduced-motion fallback `○` (NOT `●`); 1-column
  frame invariant test.
- StatusIndicator: Running `◉` Live breathe; Waiting `◐` Wait pulse; Online
  heartbeat; failed/done static.
- LoadingView: `LoadingMode { Spinner(verb), Skeleton(shape), Optimistic }`
  (audit F9) — LoadingOverlay composes LoadingView.
- Toast: entrance ≤120ms fade, stack reflow 100ms retargetable row-offset
  tween, errors skip animation, success = one pulse then static (aligns
  with plan 007's quieting).
- Timeline/LogStream: running-rail wave / 1-tick arrival pulse via the
  channel API (replace any ad-hoc sweeps).
- Focus border changes: 80ms fg cross-fade; never geometry.

**Verify**: per-widget tests incl. reduced-motion snapshots (every animated
widget: `Basic`/`Off` render is static AND states remain distinguishable);
`mise run gate` green.

### Step 5: Gates — LAND THE TWO CORE GATES BEFORE Step 3b/4 (audit: no
motion test exists anywhere today; every seam must ship with its Off-proof)

`design_gate.rs`: `motion_policy_off_is_static` (render each animated widget
at two ticks under Off → identical buffers); `spinner_frames_one_column`
(width invariant across all frame sets).

### Step 6: Layer-2 decision memo (no code)

One-page memo appended to `plans/README.md` follow-ups: tachyonfx dependency
vs native port for `Shader`/`CellFilter`/`Pattern` — evaluate against: our
`MotionPolicy` must gate all effects; recipes need role-based color targets;
dependency health. Recommendation + rationale; implementation is a future
plan.

### Step 7: Migration

`migrations/` next free number + `MIGRATING.md`: MotionPolicy env vars,
Motion enum reconciliation, Spinner phase/fallback changes, Skeleton sweep,
LoadingMode consolidation, any Presenter-visible behavior (frame pacing).

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] §2 pipeline checklist recorded, all seven PRESENT.
- [ ] Idle = 0 fps/0 bytes test green; wheel-flood test green.
- [ ] `rg -n "Elastic|Bounce" crates/termrock/src/style/motion.rs` → 0.
- [ ] `shimmer_implies_no_spinner_frames` + reduced-motion snapshots green.
- [ ] Layer-2 memo written; migration + `MIGRATING.md`; README row updated.

## STOP conditions

- The runtime has no single Presenter seam (per-widget writes exist) —
  report the write sites; restructuring the runtime beyond §2's contract is
  its own plan.
- Mode-2026 wrap breaks a supported terminal in CI — report terminal + trace.
- `Motion` type reconciliation breaks public consumers beyond a mechanical
  rename — list them.

## Maintenance notes

- Every future animated widget declares a `MotionChannel`; reviewers reject
  raw tick math in widgets.
- The §5 duration table is binding; deviations need a design-doc change.
