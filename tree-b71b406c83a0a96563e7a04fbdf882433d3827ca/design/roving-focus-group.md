# RovingFocusGroup

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0082-v0.13.0-roving-focus-group.md` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `FocusGraph` + roving **node** flag | **Preserve** — external keyboard surface |
| `ListState` selection | **Preserve** (can adopt later) |
| `MenuState` index cursor | **Migrate** internals → `RovingFocusGroup<usize>` |
| `RadioState` focus_index | **Migrate** → `RovingFocusGroup<Id>` |
| `TabsState` | **Preserve** selection; optional roving helper later |
| Dual public FocusRing | **Still deleted** |

## Mission

Reusable **active descendant** behavior for menus, radios, tabs, toolbars,
segmented controls, and collections — separate from external focus ownership
(`FocusGraph` / scene).

Radix RovingFocusGroup as behavioral reference; terminal immediate-mode registration.

## Laws

1. **External focus** = one collection id on `FocusGraph` (`roving: true`).
2. **Active descendant** = `RovingFocusGroup::active` (cursor inside collection).
3. Tab leaves the collection (graph); arrows/Home/End/typeahead stay local.
4. Disabled items skipped; reconcile after insert/remove/virtual window change.
5. Activate/Select is host/widget outcome — roving only moves active id.

## API sketch

```rust
Orientation::{Horizontal, Vertical, Both}
RovingEntry { id, enabled, label /* typeahead */ }
RovingFocusGroup { active, orientation, wrap, typeahead_buf }
  reconcile(entries) -> bool
  move_next/prev/first/last
  handle_intent(UiIntent)
  handle_key(KeyEvent) // orientation-aware
  typeahead_char
  hint_chords / semantic_active_id
```
