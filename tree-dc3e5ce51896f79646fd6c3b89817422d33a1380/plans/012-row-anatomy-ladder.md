# Plan 012: Rows get anatomy — part×tone painting for the ten flat data widgets, column tones for tables

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 006/007 DONE in `plans/README.md`.
> Re-locate every site with `rg` before editing.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED (wrap/h-scroll math moves from joined strings to parts)
- **Depends on**: plans/006 (row_chrome helper), plans/007 (status-glyph split)
- **Category**: design / tech-debt
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

The single largest "looks bad" cause in the non-priority widgets: ten data
widgets build each row as ONE `format!`/`join(" ")` string painted with ONE
style — timestamp, source, level, message, counts all the same tone. The
text ladder (primary / muted secondary / faint meta) is structurally
unreachable there, and `ComposedRow` — the repo's own row anatomy — takes a
single `Style` for all five parts, so even its three consumers (list, table,
tree) cannot tone badge/shortcut/secondary down. Tables additionally have no
per-column tone, so the design-language table spec (numeric faint, status
letter column) is unimplementable by hosts.

## Current state (anchors verified first-hand at `605217aa`)

- `widgets/composed_row.rs:14-33` — `ComposedRow { leading, primary,
  secondary, badge, shortcut, enabled, loading }`.
- `widgets/composed_row.rs:145-237` — `ComposedRowParts::paint(&self,
  buffer, area, style: Style)`: ONE style parameter applied to leading
  (`set_style` after `set_line`), primary, secondary, badge, and shortcut
  alike. No gutter slot, no per-part tones.
- `widgets/log_stream.rs:1158-1240` — Detailed recipe builds
  `parts.join(" ")` (marker, timestamp, source, glyph, letter, message,
  batch) and paints the joined string with one `style`; wrap and `h_offset`
  operate on the joined string; styled spans flattened to plain text
  (`:1158-1167`).
- Same single-string shape (audit-verified leads): `diagnostic.rs:1542-1558`,
  `trace_waterfall.rs:1157-1177`, `hex_viewer.rs:1438-1462`,
  `checkpoint_timeline.rs:1235-1245`, `terminal_output.rs:1425-1442`,
  `event_stream.rs:1092-1130`, `object_inspector.rs:1584-1620`,
  `search_results.rs:1280-1290`, `connectivity.rs:589`.
  Find each: `rg -n "join\(\" \"\)|format!\(\"\{.*\}.*\{" crates/termrock/src/widgets/<file>`.
- Tables: `table.rs:135-150` — `Column { id, title, width, alignment,
  sortable, sort, priority }`, no tone/kind; one style per row's data cells
  (`table.rs:1350`); `data_table.rs:1529` same.
- Consumers of `ComposedRowParts::paint` today: `list.rs`, `table.rs`,
  `tree.rs` (`rg -n "ComposedRow" crates/termrock/src/widgets/` → 4 files
  incl. the definition).
- Plan 007 already split the status GLYPH tone at these sites; this plan
  finishes the job: full part anatomy + per-part tones + drop order.

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `composed_row.rs`; the ten widgets listed; `table.rs`,
`data_table.rs`, `tree_table.rs` (column tones); `list.rs`/`tree.rs` (only
the call-site change to the new paint API); `design_gate.rs`;
`migrations/0296-*.md` + `MIGRATING.md`.

**Out of scope**: patterns (they compose these widgets and inherit),
overlay pickers (plan 009 fixed their columns), any wrap-engine rewrite
beyond what part-wise painting requires.

## Git workflow

`main`; commit per cluster; `git commit -s`.

## Steps

### Step 1: `ComposedRowParts::paint_with(recipe: &ListRowRecipe)`

Add a part×tone paint method: leading/gutter slot from recipe, primary =
`recipe.label`, secondary = `recipe.secondary`, badge = `recipe.trailing`,
shortcut = `recipe.shortcut`. Keep the old single-style `paint` as a thin
wrapper (calls `paint_with` with a uniform pseudo-recipe) — public API note
in the migration. Unit tests: five parts, five distinct styles land.

**Verify**: `cargo nextest run -p termrock composed_row` → new tests pass.

### Step 2: Move list/table/tree to `paint_with`

The three existing consumers pass their already-resolved recipe instead of
a flattened style; visual result: badges/shortcuts/secondary drop to muted
tones in the flagship collections.

**Verify**: list/table/tree snapshots updated: secondary cells' style ≠ primary cells' style (assert in one test per widget).

### Step 3: Project the ten flat widgets onto ComposedRow

Per widget, replace the joined-string builder with a `ComposedRow`
projection: leading = marker+status glyph (status tone from plan 007),
primary = message/name/path, secondary = source/service/type, badge =
batch/count/size, shortcut = timestamp/duration/offset. Wrap mode
(`log_stream`, `terminal_output`): wrap the PRIMARY part only; meta parts
render on row 0 (drop under narrow per part priority — ComposedRow's
existing budget logic). `h_offset` applies to the primary budget window.
Preserve each widget's colorless letter injection. `log_stream` styled
spans: paint span-wise inside the primary slot (stop flattening);
`connectivity.rs:589` single site gets the same split inline (too small for
full projection — leading glyph + muted rest).

Order (one commit each, test between): log_stream → event_stream →
trace_waterfall → diagnostic → object_inspector → search_results →
checkpoint_timeline → terminal_output → hex_viewer (offset column =
`TextFaint`, hex = `Text`, ASCII pane = `TextMuted` — its "parts" are
columns; if ComposedRow doesn't fit, do the three-tone split directly and
say so) → connectivity.

**Verify** per widget: a rendered row shows ≥2 distinct tones (test asserts
timestamp cell style ≠ message cell style); `mise run check` green.

### Step 4: Column tones for the three tables

`Column` gains `kind: ColumnKind { Text (default), Numeric, Status }`
(or `tone: Option<Role>` — pick ColumnKind, it encodes the mono letter
behavior). Cell style = row base patched by kind: Numeric → `TextMuted`,
Status → status role on the letter/glyph only. Same field on
`data_table`/`tree_table` column types; header/sort untouched (plan 006 did
those).

**Verify**: table story with a numeric column renders it muted (test);
`rg -n "ColumnKind" crates/termrock/src/widgets/ | wc -l` ≥ 3 files.

### Step 5: Gate + migration

- `design_gate.rs::data_rows_have_ladder()` — render log_stream,
  event_stream, trace_waterfall with a detailed row; assert ≥2 distinct
  fg styles per row.
- `migrations/0297-*.md` (next free) + `MIGRATING.md`: `paint` →
  `paint_with` (old kept as wrapper), row-part tone changes per widget,
  `ColumnKind` addition, wrap-behavior note (meta no longer wraps).
- `mise run gate` → exit 0.

## Test plan

Per-step tests; snapshot churn re-blessed; ComposedRow budget/drop-order
tests extended for `paint_with`.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n 'parts.join\(" "\)' crates/termrock/src/widgets/log_stream.rs` → 0.
- [ ] Ten widgets render multi-tone rows (gate test covers three; per-widget tests cover the rest).
- [ ] `ColumnKind` live in three tables.
- [ ] Migration + `MIGRATING.md`; `plans/README.md` updated.

## STOP conditions

- Part-wise wrap changes a widget's row-count contract (tests assert total
  painted rows) twice — report the wrap semantics conflict.
- `hex_viewer`'s columnar layout genuinely doesn't fit ComposedRow — do the
  direct three-tone split (allowed above) but report if even that breaks
  its selection/h-scroll math.
- `ComposedRowParts::paint` signature is depended on by out-of-workspace
  consumers listed in `docs/api/public-api.txt` in a way the wrapper can't
  satisfy — report.

## Maintenance notes

- New data widgets must project into ComposedRow — reviewers reject new
  `join(" ")` row builders (extend the design gate's source scan if it
  recurs).
- This closes the "all other components look bad" root cause #Row-anatomy;
  with plans 002-011 it completes the audited defect set.
