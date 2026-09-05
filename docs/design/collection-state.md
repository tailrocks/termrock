# CollectionState

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0083-v0.13.0-collection-state.md` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `RovingFocusGroup` | **Preserve** — composed inside CollectionState for active cursor |
| `ListState` paint regions / multi-select / hover | **Preserve**; cursor+offset via CollectionState |
| `DropdownMenuState` | **Preserve** — cascade frames compose CollectionState |
| `RadioState` | **Migrate** → CollectionState (flat, no virt) |
| `PickerState` | **Reuse** via ListState.collection |
| Tree hierarchy | **Optional** `parent` on frame items only — not forced |
| Borrowed `Line`/`&str` in long-lived state | **Forbidden** |

## Mission

Headless collection model: stable IDs, ordering, disabled, filtering window,
typeahead (frame labels), active/current item, virtualization metadata,
reconcile on appear/disappear/reorder/disable.

## API sketch

```rust
CollectionItem { id, enabled, label /* frame only */, parent: Option<Id> }
CollectionState { roving, offset, viewport_len, total_len }
  active / set_active / current (= active for single-cursor)
  reconcile(items) / apply_window(items, start, total, viewport)
  move_* / handle_intent / handle_key
  ensure_active_visible / scroll_by
  to_roving_entries()
```
