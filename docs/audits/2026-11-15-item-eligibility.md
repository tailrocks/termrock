# Item eligibility for completion and table editing (#36)

`Completion` accepted its cursor item on Tab or Enter, and on a click, even
when the item was `Item::disabled`. `DataTable::start_edit` checked that the
column was editable but not that the row was enabled, so F2, Enter, a double
click and the editor affordance opened an editor (or flipped a checkbox) on a
disabled row.

## Decision

- A completion item is acceptable when it exists and `RowSource::enabled`
  says so. `Completion::open` puts the cursor on the first acceptable item.
  Tab and Enter, a click, and `Completion::apply` all use that one test.
  `apply` returns `bool` (true when an item was inserted). A popup whose items
  are all disabled stays visible, consumes Tab and Enter without an `Accept`
  action, and still closes on Esc or an outside click. A click on a disabled
  row neither moves the cursor nor accepts.
- `DataTable::editable_target(src, col)` (existing row and cell, editable
  column, row not disabled) guards `start_edit` and the final write in
  `apply_value`. Text `commit_edit` now writes through `apply_value`.
  `set_row_disabled(src, true)` closes a text or choice session on that row
  and re-enabling does not reopen it. A direct write to the public `disabled`
  set is caught at commit: the session closes, nothing is written, and the
  route returns `TableAction::Cancelled` at most once.
- Selection and activation of disabled rows are unchanged.

## Limits

- A stale `Accept(index)` is only checked against the current items. Replacing
  the items with a different enabled item at the same index is not detected
  (keyed sources are #41).
- `set_row_disabled` returns `()`, so a programmatic disable closes the editor
  without a `Cancelled` action; callers redraw.
- Showcase editor: the `url` completion candidate is disabled with a reason
  (type `ur`). The Editable page states grid already shows a disabled
  editable row.
