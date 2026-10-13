# DismissableLayer (reusable dismissal)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0089-v0.13.0-dismissable-layer.md` |
| **Module** | `interaction::dismissable` |
| **Studio** | `overlay/nested-escape`, `dismissable/gestures` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `LayerDismissPolicy` (scene) | **Preserve** — map via `DismissAction::from_layer` |
| `OverlayStack` Esc / outside handlers | **Migrate** internals → `DismissableLayer` |
| Public `ModalStack` click classify | **Already private** — use DismissableLayer |
| Ad-hoc Esc in widgets | **Forbidden** — host peels stack first |
| Dense per-widget dismiss code | **Delete** when migrated |

## Mission

Extract Radix-style **DismissableLayer** semantics for the terminal:

| Trigger | Terminal mapping |
|---------|------------------|
| Escape | Esc / `UiIntent::Cancel` |
| Outside pointer | Press **and** release outside (drag cancel) |
| Focus leave | Host focus graph leaves scope |
| Parent closed | Nested overlay parent dismissed |
| Explicit | `dismiss` / action button |
| Critical | `DismissAction::Trap` (alert / high permission) |

## API

```rust
DismissReason::{Escape, OutsidePointer, FocusLeave, ParentClosed, Explicit}
DismissAction::{Dismiss, Trap, Bubble}
DismissPolicy { escape, outside, focus_leave, parent_closed, explicit }
DismissDecision::{None, Dismiss{reason}, Consumed, Bubble}
DismissGuard + DismissEventId   // one dismiss per event
PointerGesture::{Idle, PressOutside, PressInside}
DismissableLayer { policy, rect, gesture }
  on_escape / on_pointer_down / on_pointer_up / on_outside_click
  on_focus_leave / on_parent_closed / on_explicit
evaluate_escape_stack(layers, guard, event, DismissPhase::CaptureTopFirst)
```

## Laws

1. **One conceptual dismiss per input event** (`DismissGuard`).
2. Nested Esc is **top-first capture**; Trap stops the peel.
3. Outside dismiss requires **press outside + release outside**.
4. Drag from inside → outside does **not** dismiss.
5. Parent closed cascades children even when critical (parent_closed = Dismiss).
6. Hosts (`OverlayStack`, scene) own geometry/z-order; DismissableLayer owns policy math.

## OverlayStack integration

- `handle_escape` / `handle_outside_click` / `handle_pointer_down` / `handle_pointer_up`
- Top entry → `top_dismiss` policy+rect sync
