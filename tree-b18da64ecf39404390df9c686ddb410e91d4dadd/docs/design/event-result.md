# EventResult + typed component outcomes

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0080-v0.13.0-event-result.md` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| Per-widget `*Outcome` enums | **Preserve** as **domain messages** (`M`) |
| `interaction::Outcome<T>` (List shared) | **Preserve**; add `into_event_result()` |
| `InteractionOutcome` (scene routing) | **Preserve** — scene-level, not widget envelope |
| `OverlayOutcome` | **Preserve** — stack ops, not widget envelope |
| Ad-hoc `bool` / bare `Ignored` only | **Migrate** reps → `EventResult` |
| Global Elm/Bubble Tea runtime | **Out of scope** — no forced app architecture |
| Side effects inside widgets | **Forbidden** (unchanged law) |

## Mission

Standard envelope for “what happened after input” without owning the app loop:

- **Domain message `M`** — product/widget meaning (activated id, query changed, …)
- **Framework coordination** — consume/bubble, redraw, focus request, overlay request

Inspired by Elm messages, Bubble Tea cmds, Textual messages, Radix contracts — **not** a full TEAU runtime.

## API sketch

```rust
pub enum Redraw { None, Now }

pub enum Propagation {
    Bubble,  // not handled → parent may try
    Stop,    // consumed; stop bubble
}

pub enum FocusRequest<Id = ()> {
    Set(Id),
    Clear,
    Next,
    Previous,
}

pub enum OverlayRequest {
    DismissTop,
    Dismiss(OverlayId),
    OpenJump,
    OpenCommandPalette,
    OpenNamed { id: OverlayId, kind: OverlayKind },
}

pub struct EventResult<M, FocusId = ()> {
    // fields private or pub for inspectability
}

impl EventResult {
    ignored() / stop() / changed() / emit(m)
    with_redraw / with_focus / with_overlay
    map(f) / map_message
    merge(other)           // composite: Stop wins; messages prefer child
    or_else(|| …)          // if Bubble, run fallback
    is_consumed / message / take_message
}

// Bubble (child first): compose_bubble(child, || parent)
// Capture (parent first): compose_capture(parent, || child)
```

## Laws

1. Widgets never run effects; host applies `EventResult` + domain `M`.
2. `Propagation::Bubble` ⇔ not consumed; `Stop` ⇔ consumed.
3. Domain enums should not encode consume/redraw (prefer pure messages); transitional `Outcome::Ignored` maps to `ignored()`.
4. Deterministic: pure functions of state + input → `EventResult`.
