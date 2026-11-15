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

## Left out on purpose

- `DataGrid::update_column`, `try_set_model_keyed` and the Busy guards belong to
  #11, #33 and #43. `DataGrid::columns` is read-only until those land.
- The grid constructor does not validate `min_width <= max_width`.

The consumer migration is in `docs/current-state.md`.
