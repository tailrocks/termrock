# DataTable and DataGrid row shape at the data boundary (2026-11-15)

Source: audit issue #42.

## What was wrong

`DataTable` accepted any `Vec<Vec<Cell>>` through `new`, `set_rows` and `set_row`
and later indexed every visible column of every row. A short row panicked in
render and in sort, and a table with no columns but one empty row panicked in
`start_edit`. `DataGrid` tolerated short rows in `value` but indexed directly in
its local sorter.

## Decision

Both widgets store a rectangular model: every row has exactly one cell per
column, and a schema with no columns holds no rows. The input contracts differ.

- `DataTable` rejects a short or long row (`DataError::Shape`) and rows under an
  empty schema (`DataError::EmptySchemaWithRows`). It never pads or truncates.
- `DataGrid` pads missing trailing cells with `CellValue::Null` on ingestion,
  because a result set can arrive sparse, and rejects extra cells.
- Every checked entry point validates the whole candidate before it touches the
  live model, so a rejection keeps rows, schema, sort, cursor, pending changes
  and open editors as they were.
- The cursor, sort column and edit state stay public. Input and render clamp or
  ignore a stale index instead of indexing with it.

## Schema replacement

`DataTable::try_set_model` keeps a numeric flag, and a sort, only for a column
that is unchanged at the same index, so a replaced column never inherits the
ordering of the one it replaced. `DataGrid::try_set_model` also rejects a column
whose `max_width` is zero or below its `min_width` before it installs anything.

## Showcase demonstration

The Grid and Tables pages have a `Malformed input` option, off by default so the
approved captures do not change. On Tables, a short or long row sent to the empty
Checks table is rejected and the `DataError` text replaces the empty message. On
Grid, a long row rejects the whole batch and the error shows as the empty state,
while a short row is padded with `NULL` and displayed.

## Left out on purpose

- `DataGrid::try_set_model_keyed` and `try_set_rows_keyed` move to #33. The tree
  has no row keys, and the #78 register gives key storage and never-reused local
  keys to #33. #33 adds `GridDataError::Keys` and runs its count and uniqueness
  check in the same validation phase as the shape check. This corrects the
  unconditional statement in #42 section 5.
- The Busy half of the column-mutation lock belongs to #11.
- `GridRevision.source` assertions on a column update belong to #33, which
  introduces the revision counters.
- `DataGrid::new` keeps its infallible signature, so only `try_set_model`
  validates column widths.

The consumer migration is in `docs/current-state.md`.
