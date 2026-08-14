# TUI motion system — transitions and effects that don't feel like a terminal

**Status:** design SoT (binding architecture for motion; partially implemented)
**Audience:** implementers, design
**Related:** [`frame-clock-presence.md`](./frame-clock-presence.md) (FrameClock),
[`tui-app-deep-analysis.md`](./tui-app-deep-analysis.md),
[`streaming-performance.md`](./streaming-performance.md)
**Existing code:** `style::motion` (wave/pulse/smoothstep/edge_fade/blend/fade),
`runtime::{time,motion}`, `perf::budget`. This doc specs the missing layers.
**Reference implementations:** Grok Build (`xai-org/grok-build`, verified from
source), [tachyonfx](https://github.com/ratatui/tachyonfx) (from source),
Textual `_animator.py`, Charm harmonica, OpenTUI timeline.

---

## 1. What "Amp/Grok smooth" actually is

Amp's *Look Ma, No Flicker* (Sept 2025) names the feeling without the mechanics.
Grok Build is open source; the mechanics are verified:

| Mechanism | Ground truth |
|---|---|
| Frame atomicity | Every frame wrapped in crossterm `Begin/EndSynchronizedUpdate` (mode 2026); critical in tmux/zellij |
| Demand-driven fps | Tick scheduled only when something animates: idle 0 → ambient 12 fps → active 30 fps cap (configurable 1–60) |
| Separate scroll clock | Scroll flushes at 16 ms cadence **independent of animation fps**; riding the ambient tick caused visible jumps (documented bug) |
| Wall-clock ambient phases | Record-dot pulse (0.7 s sine), shimmer sweep (1.3 s raised-cosine band, 4 s cycle), 5 s breathing at 6% amplitude — none coupled to tick rate |
| Peak restraint | Shine amplitude 0.33, pulse 6% — ambient motion whispers |
| Presenter backpressure | Dirty-flag coalescing, in-flight frame sequencing (never queue N+1 while N unflushed), min-draw-interval throttle; regression tests `wheel_flood_paints_no_ghost_frames` |
| Cursor de-dup | Zero cursor bytes on empty diffs so hardware blink survives |
| Layout-stable animation | Hard-tested invariant: every spinner frame exactly 1 column |
| Scroll input normalization | Wheel vs trackpad auto-detect per stream, sub-line fractional accumulation, per-terminal event normalization, 80 ms stream-gap finalize |

**Conclusion:** the fluidity is 80% render-pipeline discipline + 20% tasteful
effects. Effects on a flickering pipeline read as jank, not polish.

---

## 2. Render pipeline law (P0 — prerequisites, not optional)

1. **Double-buffer diff always.** Zero-diff frames emit zero bytes. Never bypass
   with direct writes.
2. **Synchronized output (mode 2026) on every frame.** BSU/ESU span kept short
   (ConPTY latency bug, rio#1753). Detect via DECRQM; degrade silently.
3. **Presenter owns the wire**: dirty coalescing, in-flight backpressure,
   min-interval throttle, cursor de-dup. One `Presenter` in `runtime::runner`,
   not per-widget.
4. **Demand-driven tick ladder** (kernel-owned): `None → Ambient(12fps) →
   Active(30fps) → Ceiling(60fps)`, computed from registered animations each
   frame. Idle CPU = 0 is a quality signal.
5. **Scroll clock separate** at 16 ms; never rides the ambient tick.
6. **Phase order per frame** (OpenTUI model): `animations → layout → render →
   effects → synchronized flush`.
7. **No animation when not a TTY**; no blocking input for any tween (Ctrl-C and
   keys act immediately; transitions skippable).

## 3. Motion tiers (reduced-motion story)

`MotionPolicy { Full | Basic | Off }`, resolved from env
(`TERMROCK_ANIMATIONS=full|basic|none`, honor `REDUCE_MOTION` if set), overridable
per app. Precedent: Textual `TEXTUAL_ANIMATIONS`.

| Tier | Behavior |
|------|----------|
| Full | Everything below |
| Basic | Transitions ≤ 120 ms fades only; ambient loops static at bright end; spinners static `…` + verb text |
| Off | Instant state changes; status still legible (glyph/verb/bold channels) |

Reduced ≠ frozen: status must stay readable. Swap wave → static bright rail,
spinner → `…`, never remove information.

## 4. Two-layer motion architecture

```text
Layer 1 — value animation (typed, kernel-owned):
    Tween::to(value, (ms, Easing))          // easing enum, not strings
    Spring::new(freq, damping)              // retarget-safe (harmonica model)
    Animator::tick(dt) -> impl Iterator<(AnimId, Value)>   // sleeps when empty

Layer 2 — buffer effects (post-render, tachyonfx-shaped):
    fx::fade_to(role, (200, Easing::CubicOut)).with_filter(CellFilter::Text)
    sequence / parallel / delay / ping_pong trees
    Pattern::{Sweep, Radial, Wave, Dissolve, …} for per-cell alpha fields
```

- **Layer 1** animates numbers: scroll offsets, progress values, widths, counter
  tickers. Springs for anything that retargets mid-flight (streaming progress).
- **Layer 2** mutates cells after widgets render: fades, sweeps, dissolves,
  glyph-morphs. Zero widget intrusion; works on any rendered buffer.
- **Glue**: widgets take `Motion` config (durations/easings from theme tokens);
  kernel owns the single clock; ambient loops use **wall-clock phase**,
  transitions use **tick counts** — never the reverse.

Build vs depend: tachyonfx (now ratatui-org) is the reference. Evaluate depending
on it for Layer 2 vs porting `Shader`/`CellFilter`/`Pattern` natively. Decision
inputs: our `MotionPolicy` must gate all effects, and our recipes need role-based
color targets. Either way the *shapes* above are binding; grok's welcome shimmer /
accent-rail wave are `patterns/` composites, not kernel.

## 5. Easing & duration table (binding)

| Use | Easing | Duration | Cell-grid rationale |
|---|---|---|---|
| Micro feedback (press, check) | `Linear`/`SineOut` | 60–120 ms | < 150 ms only presence registers |
| Overlay/dialog **in** | `CubicOut`/`QuadOut` | 150–250 ms | Fast start, stable integer tail |
| Overlay/dialog **out** | `CubicIn`/`SineIn` | 100–180 ms | Exits faster than entrances or UI feels sticky |
| Screen/panel transition | `SineInOut`/`QuadInOut` | 200–350 ms | Symmetric curves survive quantization |
| Scroll easing | `CubicOut` | 80–200 ms, distance-scaled | Short hops instant, long jumps capped |
| Retargeting values (progress, counters) | `Spring(freq≈18, damping=1)` | physics | No restart pops mid-stream |
| Ambient loops | sin² / raised cosine, wall-clock | 0.7 s–5 s periods | sin² ∈ [0,1] = pure breathing |
| **Forbidden** | `Elastic*`, `Bounce*`, strong `Back*` | — | Overshoot quantizes to row pops + color flicker |

**Duration law:** < 100 ms reads instant, 150–300 ms fluid, > 400 ms sluggish.
Transitions live in 100–300 ms; ambient loops are slow and quiet.

Per-character stagger recipes: typewriter = reveal 8–20 ms/char; decrypt ripple =
glyph-morph with diagonal pattern; number ticker = spring the value, format per
frame (or per-digit 30 ms delays).

## 6. Widget motion contracts (per family)

| Family | Motion contract |
|---|---|
| Overlays (dialog, palette, popover, toast) | In: fade + 1–2 cell rise (`CubicOut`, 180 ms); out: fade (`SineIn`, 120 ms); backdrop dims with the same alpha |
| Collections (list, table, picker) | Selection gutter **snaps** (no tween); row content cross-fades 80 ms on filter change; match spans pop bold instantly |
| Tabs | Active fill cross-fade 100 ms; no sliding indicator on cell grids (reads as jitter) |
| Progress/meters | Spring the value; block-element 1/8-cell resolution; braille graphs scroll 1 col/frame at data rate, not fps rate |
| Spinners | 1-column frames, braille 80 ms interval; verb text rotates on word boundaries; `esc to interrupt` hint fades in after 2 s |
| Skeletons | `raised` shimmer sweep (raised-cosine band, ~1.5 s period, ≤ 0.3 amplitude) or static under Basic/Off |
| Toasts | Slide ≤ 2 cells + fade in; stack reflows with 100 ms translate; exit fades, siblings close ranks |
| Focus change | Border recolor cross-fades 80 ms (fg blend); never animate border geometry |
| Diff/review | Added/removed tint fades in 120 ms after paint (settles attention); no per-line stagger on large hunks |
| Mode change | Statusline block + frame highlight cross-fade 100 ms (zellij teaching chrome) |

## 7. Anti-patterns (hard rules)

1. Fixed-rate ticking regardless of activity — idle must be 0 fps.
2. Frames outside mode 2026 — tearing in muxes.
3. Layout-shifting animation (variable-width spinner frames, moving neighbors).
4. Motion without information content — every animation answers "what is
   happening"; decoration-only loops are bugs (clig.dev law).
5. Animating non-TTY output.
6. Overshoot easings on full-cell geometry.
7. Tick-coupled ambient loops (phase must survive fps changes).
8. Repaint storms: one draw per input event under wheel/token flood.
9. High-amplitude always-on motion (btop-class gaudy/CPU burn; ambient ≤ ~1/3
   amplitude).
10. Blocking input for animation; unskippable transitions.
11. No reduced-motion path.
12. Scroll flush riding the ambient tick.

## 8. Implementation deltas

| Priority | Work |
|---|---|
| P0 | `runtime` Presenter: 2026 wrap, dirty coalescing, in-flight backpressure, cursor de-dup, demand-driven tick ladder, separate 16 ms scroll clock |
| P0 | `MotionPolicy` (Full/Basic/Off) env+config resolution; gates every animation path |
| P1 | `motion` layer 1: typed `Easing`, `Tween`, `Spring`, `Animator` (kernel clock) |
| P1 | Layer 2: evaluate tachyonfx dep vs port (`Shader`/`CellFilter`/`Pattern`/`EffectTimer` shapes binding either way) |
| P1 | Widget `Motion` configs from theme tokens; per-family contracts in §6 wired into recipes |
| P2 | Ambient kit in `patterns`: accent-rail sin² wave, raised-cosine shimmer, record-dot pulse, dot-pulse background spinner |
| P2 | Braille sub-cell canvas for gauges/sparklines (dotmax prior art) |
| P2 | Studio motion stories: every animated widget has a reduced-motion snapshot + a frame-timing assertion |

## 9. Acceptance criteria

- Idle full-screen app: 0 fps tick, 0 bytes emitted, CPU ≈ 0.
- Wheel-flood test: no ghost frames, no queue growth (grok's two regression tests
  ported).
- In tmux and zellij: no visible tearing during streaming + overlay transitions.
- Every animated surface honors Basic/Off without losing status legibility.
- Transition durations all within §5 budgets; no Elastic/Bounce/Back anywhere.
