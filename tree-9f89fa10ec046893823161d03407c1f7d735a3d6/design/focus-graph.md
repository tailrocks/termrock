# FocusGraph

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0081-v0.13.0-focus-graph.md` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `InteractionScene` focus + layer input ownership | **Preserve** — layers + Esc + hit still scene-owned |
| Crate-private `FocusRing` | **Preserve private** (tests / legacy); not public API |
| Public focus API | **`FocusGraph` sole public focus graph authority** |
| `FocusOutcome` | **Public** (via FocusGraph module) |
| Collection selection (`ListState` cursor) | **Preserve** — not focus |
| `accepts_input` host gates | **Preserve** — input grant ≠ graph focus |
| `EventResult::FocusRequest` | **Wire** via `FocusGraph::apply_request` |

## Focus vs selection vs pointer

| Concept | Owner | Meaning |
|---------|-------|---------|
| **Focus** | `FocusGraph` | Which surface owns keyboard routing |
| **Selection / cursor** | Widget state | Row/cell inside a roving collection |
| **Pointer hit** | Geometry + `focus_at` | May move focus; does not imply Activate |
| **accepts_input** | Host/overlay | Whether widget may handle keys at all |

**Roving:** Graph focuses the collection id once (`roving: true`). Internal Move intents update selection only while that id is focused. Tab/`FocusNext` leaves the collection.

## API sketch

```rust
FocusNavMode::{Linear, Spatial, Hybrid}
FocusNode { id, parent, zone, area, enabled, focusable, roving, tab_index }
FocusGraph::begin_frame / register / attach_area / reconcile
focus_next / previous / spatial(dir) / request_focus / focus_at
push_trap / pop_trap (opener restore + history)
debug_snapshot / FocusLens paint
from_interaction(&InteractionScene) adapter
```
