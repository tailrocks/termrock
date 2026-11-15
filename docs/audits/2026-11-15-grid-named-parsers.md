# DataGrid named types validate their whole grammar (#47)

`default_validator` advertised JSON object/array, UUID and `YYYY-MM-DD`, but
checked shape only: `{not json}` passed as Json, 36 hyphens passed as an Id and
`2026-99-99` passed as a Timestamp. Because the grid commits what the parser
returns, a cell could hold text its column type says is impossible, and the
`Number` kind accepted `NaN` and `inf`.

## Decision

- New public helpers `widgets::edit::parsers::{json_container, canonical_uuid,
  date_or_timestamp}`, each `&str -> Result<(), FieldError>`. They validate the
  exact text they are given (no trimming) and never rewrite it, so a form can
  reuse them without a `DataGrid`. `default_validator` trims, calls them and
  keeps the trimmed spelling.
- Json is one complete `serde_json::Value` with an object or array root. The
  default recursion limit applies and is documented; no recursion limit is
  disabled and no tree walk follows the parse. Valid JSON is never
  reserialised, because key order and whitespace are user data.
- Id is the canonical 8-4-4-4-12 grouping, either hex case, no version or
  variant check, no case folding. `canonical_uuid` names the grouping.
- Timestamp is `YYYY-MM-DD` plus an optional time, fraction (1 to 9 digits) and
  fixed offset (see `docs/current-state.md`). It checks Gregorian dates, rejects
  leap seconds, and does not convert or assume a zone. The TablePro fixture
  forms `2025-06-01 12:30:00+00` and date-only values stay valid, and a test
  walks every generated fixture of those kinds through the default parser.
- Number keeps int-first parsing and accepts a float only when it is finite.
  An imported non-finite cell can therefore not be committed back by an edit.
- A permissive app supplies its own `cell_validator`; there is no global
  permissive flag. `CellKind::Id` means this UUID form.
- `serde_json` moves from the `cli` feature to a normal dependency of the
  library. The helpers are the only library code that uses it.

## Not changed

JSON scalar support, timezone handling, database schema checks and the display
format of numbers and timestamps. The commit path is #43's single gateway; the
parsers plug into it and add no second validation path.
