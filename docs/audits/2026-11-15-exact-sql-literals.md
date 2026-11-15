# Exact SQL literals in TablePro and Showcase (#49)

TablePro rendered every number in its mutation SQL with two decimals
(`format!("{n:.2}")`). A staged `1.2345` saved as `1.23`, `0.001` as `0.00`,
and the fallback all-column predicate of a table without a primary key
described a different row than the one loaded. The Showcase grid preview reused
the two-decimal display text.

## Decision

- `model::sql_literal` returns `Result<String, SqlLiteralError>`. A finite
  `f64` prints through `to_string` (shortest text that parses to the same
  bits, `-0.0` included) and gets `.0` when the text has no `.`, `e` or `E`.
  Integers stay exact decimal and never pass through `f64`. A NaN or infinite
  number is `SqlLiteralError::NonFiniteNumber`.
- `model::preview_sql` returns `Result<Vec<String>, PreviewSqlError>`. The
  error carries `PreviewRow::Source(row)` (since #50 it is `PreviewRow::Key(ItemKey)` and `preview_sql` takes a `GridChangeSet`), the column name and a typed
  `PreviewSqlCause` (`Literal`, `MissingColumn`, `InvalidDefaultPredicate`).
  The first error aborts the preview: no partial statement list exists.
- Original-row predicates read base values only. A null key or column uses
  `IS NULL` (the key path used `= NULL` before). A key column missing from the
  tab was silently read from column 0 and is now an error.
- `begin_commit` and `open_preview` show the rejection in the status bar and
  open no dialog. `finish_commit` records no history and applies nothing if the
  serialization fails. The draft stays pending in every case.
- The Showcase `GridPage::statements` and `changes_dialog` return the same
  complete-or-error result and report the failing row and column.

## Limits

- `f64::to_string` recovers the stored binary value, not decimal digits the
  model discarded earlier.
- The numeric formatting rule exists once in TablePro and once in the Showcase
  demo projection. #48 adds a lossless number text for the grid editor inside
  the library; the two copies can converge on it when that lands.
- `PreviewRow::Key`, `PreviewSqlCause::InvalidSchema`, `DEFAULT` in an UPDATE
  assignment and identifier quoting belong to #50.
- `finish_commit` still regenerates the statements it records. #11 replaces
  that with the reviewed payload.
