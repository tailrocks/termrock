# SelectionModel

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0084-v0.13.0-selection-model.md` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `widgets::Selection` ordered multi-check | **Migrate** → `interaction::SelectionModel` (alias kept short-term) |
| `data_view::SelectionModel` row/cell | **Extend** / row ops use shared model; cell API kept |
| `CollectionState` active/current | **Preserve** — cursor ≠ selection |
| `FocusGraph` focus | **Preserve** — focus ≠ selection |
| `SelectionChrome` (paint) | **Preserve** — visual recipe only |
| Color-only selection meaning | **Forbidden** |

## Separation of concerns

| Concept | Owner |
|---------|-------|
| Focus | `FocusGraph` |
| Active cursor | `CollectionState` / roving |
| Current item | Usually active (single-cursor) |
| Checked / multi-selected | **SelectionModel** |
| Selected (single) | SelectionModel Single **or** active |

## API sketch

```rust
SelectionKind::{None, Single, Multiple, Range}
SelectionDelta::{Cleared, Replaced, Added, Removed, Toggled, RangeApplied}
SelectionModel { kind, ordered ids, anchor }
  select / deselect / toggle / clear / select_all(ids) / invert(visible)
  extend_range(anchor_order, to)  // filtered-visible order
  is_selected / selected() / anchor
  reconcile(visible_enabled) // drop missing; keep valid under filter
SelectionVisual::{Gutter, Fill, Tint, Check} // non-color cues
```
