# Complete DataGrid change snapshots (#50)

`DataGrid::duplicate_row` copies the effective cells into the new row's stored
base cells and records only the row key as inserted. `PendingChanges::cells`
therefore held none of the copies. TablePro and the Showcase preview built their
INSERT from `pending.cells` alone, so an untouched duplicate saved as
`INSERT ... DEFAULT VALUES` and a duplicate with one edit saved only that one
column, while the grid showed the whole row.

## Decision

- `DataGrid::snapshot_changes()` is the only place that normalizes a draft. It
  returns an owned `GridChangeSet` of keyed rows and never mutates the grid.
  - `InsertedRow` holds every effective value at the schema width. `Default` and
    `Null` stay distinct.
  - `UpdatedRow` holds the complete base row and the changed `CellChange`s in
    ascending column order. A cell whose value equals the base under
    `same_edit_value` is dropped.
  - `DeletedRow` holds the complete base row. Staged cells of a deleted row never
    become an update. A removed inserted row is absent.
- Rows are listed in source order, so the result does not depend on hash order,
  sorting, selection or the cursor. The three classes are disjoint.
- The pass sorts the pending cells by row once and clones payload only for rows it
  emits. A clean grid copies nothing.
- A snapshot carries a `GridInstanceId` (`origin()`) and the `GridRevision`. A
  revision means something only with its origin. Cloning a `DataGrid` gives the
  clone a fresh id, because the clone is an independent grid with the same
  draft, not a second handle to an in-flight request. The id lives in a private
  field type whose `Clone` allocates, so a field added later cannot be copied by
  mistake.
- `GridChangeSet` derives `PartialEq`, which keeps `f64` rules (`NaN != NaN`).
  `same_capture` is the admission comparison: same origin, same revision and
  bit-identical rows, so a `NaN` payload or the sign of zero counts. The two are
  different contracts and must not be merged.
- `CommittedRow` and `GridCommitReceipt` are data shapes only. A valid receipt has
  one `CommittedRow` per inserted and updated row of its snapshot, keyed by the
  snapshot key, with complete accepted values and the final `committed_key` (the
  same key for an update, possibly a new one for a generated insert). Deleted rows
  have none. `GridCommitReceipt::new(&snapshot, rows)` copies the snapshot's origin
  and revision, so a consumer can reject a receipt for another grid or capture.
  The submission code that checks coverage and applies it is separate work.

## TablePro and Showcase

- `model::preview_sql(table, columns, &GridChangeSet)` cannot see a live grid.
  INSERT lists every column except an explicit `Default` and a schema-generated
  column; an explicit `Null` is `NULL`. UPDATE sets only the changed columns,
  writes an explicit `Default` as `DEFAULT`, and matches on the snapshot's base
  row (primary key when the projection has it, else every projected column). A
  change to a generated column is `PreviewSqlCause::GeneratedColumn`, never a
  dropped statement. A snapshot whose width differs from the projection is
  `PreviewSqlCause::RowShape`. `PreviewRow::Key(ItemKey)` names the row, and the
  app turns it into a row number for the status line.
- `begin_commit`, `open_preview` and the temporary `finish_commit` capture a
  snapshot before they render SQL.
- The Showcase `GridPage::statements` takes a `GridChangeSet`; its dialogs are
  built from one capture.
- A fresh insert still starts with `Default` for key and read-only cells and
  `Null` elsewhere. Its SQL now lists the untouched cells as `NULL`, matching
  what the grid displays.

## Migrating

Do not read `PendingChanges::cells` as a complete inserted row. It is the draft's
overrides. Take a `GridChangeSet` and read `InsertedRow::values`.

## Limits

- Identifiers are still written unquoted, so the accepted TablePro and Showcase
  captures keep their text. Quoting them is a visible change that needs its own
  baseline refresh, tracked in #181.
- A base row shorter than the schema reads as `Null` in its missing tail until
  #42 canonicalizes stored rows.
- Delayed save, submission locks and completion validation are #11. Resolved
  database defaults and receipts from a store are #51.
