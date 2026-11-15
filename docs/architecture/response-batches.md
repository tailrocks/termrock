# Response action batches

`core::response::Response<A>` carries one flow answer, one invalidation level and an
ordered batch of semantic actions. One input may commit a value, change a selection,
request a lazy expansion and hand focus on, so a response holds every one of those
actions exactly once, in the order they happened.

`Actions<A>` stores zero or one action inline and allocates from the second. It is
unrelated to the `core::keymap::Actions` key-map trait; import it as
`ResponseActions` when a module needs both.

## Consuming a response

```rust
// Before (v0.1 early): one optional action, a second one was silently lost.
let (outcome, action) = table.on_key(&key);
if let Some(TableAction::Committed { .. }) = action { save(); }

// After: drain the whole batch in order, fold the outcome once.
let (outcome, actions) = table.on_key(&key).into_parts();
for action in actions {
    match action {
        TableAction::Committed { .. } => save(),
        TableAction::LeaveForward => focus_next(),
        _ => {}
    }
}
```

| Need | API |
| --- | --- |
| Drain, keep the legacy outcome | `into_parts() -> (Outcome, Actions<A>)` (a `Layout` invalidation collapses to `Changed`) |
| Drain, keep flow and invalidation | `into_raw_parts() -> (Flow, Invalidate, Actions<A>)` |
| Inspect without taking | `actions()`, `has_actions()` |
| Take in place | `take_actions()` |
| Combine responses | `merge`, `\|`, `\|=`, `push_action` |
| Translate for a parent widget | `map_actions`, `on_actions` |
| A boundary that proves at most one action | `try_into_single() -> Result<(Outcome, Option<A>), Response<A>>` |
| Deliberately discard actions | `erase()` |

The singular `action`, `take_action`, `into_action`, `map_action`, `on_action` and
`with_action` were removed so the compiler finds every consumer.

## Intentional signature changes

These return `Response<Action>` instead of `(Outcome, Option<Action>)`:

- `DataTable::{on_key, begin_edit, start_edit, on_blur, on_click_editor, on_click_header, on_click_cell, on_click_cell_with, on_click_cell_count}`
- `DataGrid::{on_key, begin_edit, on_blur, on_click, on_click_with}`
- `TreeView::{toggle, toggle_recursive, run, on_key, on_click_row, on_click_row_count, on_click_toggle, on_click_with, on_click_at}`

Single-result helpers such as `DataTable::commit_edit` keep `Option`.

## Ordering rules

| Input | Ordered effects |
| --- | --- |
| Valid changed edit, then Tab off the last column (Shift+Tab off the first) | `Committed` / `CellChanged`, then `LeaveForward` / `LeaveBackward` |
| Unchanged valid text edit, then edge Tab | `LeaveForward` only; the editor closes and the cell error clears |
| Invalid edit, then Tab | error repaint; no commit and no movement |
| Tab into a text or choice cell | focus moves and the editor opens; nothing is toggled |
| Tab into a checkbox, bool, JSON or long-text cell | focus only; Enter, Space or click still activates |
| Tree `BranchAction::Both` on a collapsed lazy branch | `Expand`, then `Activate` for the same path |

### See it in Showcase

- **Editable tables, Options tab, "Tab at the last cell".** `commit`, `no-op` and `invalid`
  start the table on its last editable cell. The page lists the keys to press under the
  table. The table card shows the batch the last input produced (`last Committed →
  LeaveForward`, `last LeaveForward`, `last none`), and the hint bar names what Tab does.
- **Trees, Options tab, "Enter on a branch" = `both`.** Enter on a lazy connection reads
  `last TreeAction: Expand(orders-db) → Activate(orders-db)`, and the Enter hint says
  "Expand, then open".

## Layer result queue

`Picker` and `ContextMenu` queue every action one input emits and release them one per
`take_result`. Three rules keep that lossless:

- **One drain loop.** A host never takes once. It calls `ModalStack::settle`, which
  re-reads the stack before every take, hands each `Taken` to the host's handler and
  folds the outcomes. TablePro, Holla, Showcase and Jackin all call it.
- **The close is last.** Queued events are released first, in order, and the closing
  result (`Chosen`, `ChosenAlt`, `ChosenMany`, `Cancelled`, or a menu result) follows, so
  removing the layer cannot drop an event queued beside it. A second closing result in the
  same batch is redundant and is dropped with the layer.
- **Dismiss is Cancelled.** `ModalStack::dismiss` calls `Layer::on_dismiss`, which
  replaces the queue with the cancel result, so a dismissed picker always resolves to
  `Cancelled` and a menu to `Dismissed`. Results still queued when the user dismisses
  belong to the abandoned interaction and are discarded.
