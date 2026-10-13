# ObjectInspector cursor vs scene focus (premium bar)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0070-v0.13.0-object-inspector-cursor.md` |

## Problem

`ObjectInspectorOutcome::FocusChanged` and `ObjectInspectorState::focus` conflate
**field cursor** with **scene surface focus**. Hosts already own surface focus
via InteractionScene / OverlayStack.

The prior API also lacked intent routing, mouse hit geometry, accepts_input
gating, and non-color cursor marks — below the premium component bar set by
List / Menu / DataTable.

## Decisions

1. Rename **FocusChanged → CursorMoved**; **focus → cursor** (`focus()` deprecated).
2. Field cursor is **valid state** owned by `ObjectInspectorState` when the host
   grants input via `accepts_input`.
3. Paint: `ObjectInspector::focused(bool)` for scene ownership chrome; cursor
   gutter (`›` / `>` / unfocused `·` / `.`) separate from surface focus.
4. `handle_intent` + `default_inspector_intent` (list nav minus Esc cancel).
5. `handle_mouse` wheel + click-to-cursor; second click activates.
6. Empty state: `∅` / `[ ]` + muted copy.
7. `ascii` + `colorless` paint flags.
8. Responsive: narrow (`key=value`), tiny (key or value only).
9. Cursor-follow scroll via `ScrollAreaState::set_offset_y`.
10. `tokens` constructor param → `system` naming (DesignSystem sole paint).

## Not doing

- Moving field cursor into InteractionScene (would bloat scene ids for large
  projections).
- Nested expand/collapse inside the widget (consumer owns projection of depth).
- Clipboard / open effects (Activate outcome only; consumer executes).
