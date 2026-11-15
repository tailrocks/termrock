# DataGrid drafts and undo are keyed by row identity (#33)

`DataGrid` kept its pending cells, deleted and inserted sets and undo records
by source row index. Deleting an existing row erased its staged cells and
stored only the previous delete flag, so Undo brought back the loaded value
instead of the edit made just before. Removing an inserted row renumbered the
stored rows and the pending cells but left the undo stack, the selection, the
anchors and the error maps on the old numbers, so undoing a later edit wrote to
a stale index and created a phantom pending cell. Deleting an inserted row
pushed no inverse at all.

## Decision

- Every row has an `ItemKey` (the existing identity type, no second one).
  `set_rows` and `append_rows` give rows never-reused local `ItemKey::Num`
  values from a monotonic counter that also skips keys already present.
  `try_set_rows_keyed` and `try_set_model_keyed` take provider keys and reject
  a wrong count or a duplicate (`GridKeyError`) before they touch the grid.
- `PendingChanges` is read-only outside the grid and keyed:
  `cells()`, `inserted()`, `deleted()`, `is_inserted`, `is_deleted`, `value`,
  `is_dirty`, `dirty_rows`, `counts`, `total`, `is_empty`. `DataGrid::pending()`
  borrows it. `dirty_rows` and the update count leave out inserted and deleted
  rows.
- Deleting an existing row only sets its flag. The staged cells stay in the one
  pending map, so undelete, Undo and a second Delete show them again. A
  deleted row is never part of an update.
- Every undo record names a key. Removing an inserted row records a private
  inverse that carries the base cells, the pending cells, the cell and row
  errors, the source position, and a snapshot of the selection, cursor, cell
  anchor and scroll offset. Undo puts the same key back at that position. Rows
  appended in between stay, with their data, errors and selection. The display
  order is rebuilt from the sort policy: a local sort puts base rows first and
  drafts last.
- `undo()` returns `Option<GridUndo>` (`key`, `GridUndoKind`) and
  `GridAction::Undone` carries it, so a host learns which record changed, not
  where the cursor happens to be.
- `record_cell` returns whether the draft changed. An edit equal to the current
  value, a row or column out of range, and a `toggle_delete` of a missing row
  leave history and revisions alone.
- `GridRevision { source, draft }` counts source replacement, append and commit
  and each draft change. Counters never reset. View-only changes (cursor,
  sort, selection, errors) do not move them. #50 builds its change snapshots on
  these and on the keys.
- Marked-row Delete resolves each marked row by key, so removing an inserted
  row no longer shifts the next marked index.
- A successful commit keeps the keys and the display order of surviving rows and
  moves the selection and cursor with them. It no longer resets the order to
  source order.

## Behaviour changes for hosts

- `DataGrid::pending` is a method. `pending.cells`, `pending.inserted` and
  `pending.deleted` are no longer public fields; use the borrowed iterators and
  `DataGrid::source_index` to reach a source row.
- `UndoAction` is private. `DataGrid::undo` returns `Option<GridUndo>`.
- `DataGrid::record_cell` returns `bool`.
- Local sort puts inserted rows after the sorted base rows.

## Limits

- `GridKeyError` stands alone because `GridDataError` (#42) is not on `main`.
  #42 wraps it as `GridDataError::Keys` and turns `try_set_model_keyed` into a
  thin wrapper over its `try_set_model`.
- Equality for the unchanged-edit check is `CellValue::eq`. #48 owns the
  canonical comparison.
- The editor session guard here closes both the text editor and the choice
  editor on every structural change. #32 and #43 own the shared helper.
- Redo, persistence across reloads and merging drafts into a reloaded source are
  not part of this change.
