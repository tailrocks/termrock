# FrameClock, Presence, and motion

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0092-v0.13.0-frame-clock-presence.md` |
| **Module** | `runtime::{time,motion}` |
| **Studio** | `motion/presence-spinner` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `FrameTick` | **Preserve** + helpers |
| `FrameClock` | **Public** (was crate-private) |
| `Motion` (style) | **Preserve** Full / Reduced / Off |
| Spinner / Progress / Toast | **Migrate** to motion helpers / Presence |
| TooltipState hover_ms | **Migrate** → Presence + FrameTick |
| Wall-clock sampling inside widgets | **Forbidden** |

## Mission

Deterministic animation and timed presence **without** gratuitous motion:

- Injectable time (tests, Studio replay)
- Spinner cadence / pulses honor `Motion`
- Toast TTL + tooltip delay via `Presence`
- Idle work must **not** demand redraw for decoration alone
- Hidden / pending / exiting surfaces are **not focusable**

## API

```rust
FrameTick::manual(now, elapsed, delta)
FrameClock::from_start(now).tick_at(now)
tick.spinner_step(n, period_ms, motion)
tick.pulse_fraction(period_ms, motion)

Presence::tooltip(delay) / ::toast(ttl) / ::persistent()
presence.request_show(tick) / request_hide(tick, motion) / advance(tick, motion)
presence.is_visible() / is_focusable() / next_deadline()

AnimationDemand { needs_redraw, next_deadline }
spinner_demand(tick, motion, active)
earliest_deadline([toast.next_deadline(), spinner.deadline])

ProgressKind::indeterminate_from(tick, motion)
```

## Laws

1. Sample time **once per frame** in the host; pass `FrameTick` in.
2. `Motion::Off` → static glyphs; no spinner deadline.
3. `Presence` pending/exiting → not focusable.
4. Host poll: `next_deadline` from active Presence + spinner_demand only when work active.
