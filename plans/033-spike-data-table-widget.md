# Plan 033 (spike): Design the columnar `Table` widget — headers, alignment, width policy, sort indicators

> **Executor instructions**: DESIGN SPIKE. Deliverable = design doc + a
> compile-proven column-layout prototype + a lookbook story sketch + a
> recommendation. Honor STOP conditions. Update the plans/README.md row when
> done.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/widgets/detail_table.rs crates/termrock/src/widgets/list.rs`
> Design against the POST-011/013 API. Verify those rows DONE.

## Status

- **Priority**: P3
- **Effort**: L (coarse — the column model is the hard part)
- **Risk**: MED (new flagship widget; a wrong column projection shape is expensive under forward-only rules)
- **Depends on**: plans/011-event-model-convergence.md, plans/013-construction-idiom-and-widget-traits.md; read plans/007's characterization notes on DetailTable (its machinery is the interaction template)
- **Category**: direction
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

Nothing in the library can render tabular data with columns. `DetailRow` is strictly `{ label, value, href, capability }` joined by a fixed `" : "` separator — a two-slot key:value widget; `List` rows are a single `Line`. Any consumer surfacing real tables (process lists, resource listings, transaction rows, anything with >2 columns) hand-rolls column layout, width negotiation, headers, alignment, and sort indicators — exactly the "neutral rendering body" AGENTS.md forbids consumers to re-implement. shadcn ships DataTable; for the "best possible Rust TUI components library" this is the largest widget-set gap. It's a spike because the column model (borrowed cells, width policy, huge-row-count posture) has real design risk.

## Current state

- `crates/termrock/src/widgets/detail_table.rs` — the closest relative and the interaction TEMPLATE to reuse, not extend: `const SEPARATOR: &str = " : "`;

```rust
pub struct DetailRow<'a, Id> {
    pub id: Id,
    pub label: &'a str,
    pub value: &'a str,
    pub href: Option<&'a str>,
    pub capability: DetailCapability,
    pub emphasis: bool,
    pub style: Option<Style>,
}
```

  `DetailTableState` owns selection/hover/scroll/painted regions (`DetailRegion { row_area, action_area, value_area, capability }`) — the region/outcome machinery a Table needs, already proven. (Post-016 it also has the measurement cache.)
- `List`/`ListState` — stable-Id selection/viewport/reconciliation patterns (`for_count`, `reconcile_count` …).
- Width primitives: `text::display_cols`, `display_cols_slice`, `take_display_cols` (grapheme-safe measure/slice) — the cell-truncation toolbox.
- Ownership doctrine (COMPONENTS.md pattern for Tree): "caller-flattened borrowed projection with stable IDs … callers retain hierarchy, filtering, lazy loading" — for Table this translates to: caller owns the data, ORDERING (sorting is caller-executed), and cell text; the widget owns layout, headers, selection, scroll, hit regions, and sort-indicator RENDERING.
- Post-011 contract: state-owned `handle_key(data, key) -> Outcome`, `hover`/`click`; post-013 construction idiom; post-016 O(visible) paint discipline + hot-path proof pattern (`tree_hot_path.rs`).
- Perf posture: 10k-row tables must render O(viewport) — same bar as Tree/List.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Gallery | `cargo run -p termrock-lookbook` | prototype story renders |

## Scope

**In scope (spike)**:
- Design doc `plans/033-table-design.md`
- Prototype of the COLUMN LAYOUT SOLVER (pure function: `&[Column] + available width → resolved widths`) with table-driven tests — this is the risk core; prototype it standalone (lookbook-local or `#[cfg(test)]`)
- One lookbook-local story sketch rendering a static table via the prototype

**Out of scope**:
- The full widget build (follow-up plan from the doc).
- Sorting EXECUTION (caller re-orders rows; widget renders `▲`/`▼` on the sorted column and emits a `SortRequested(column_id)` outcome).
- Cell editing, column resize-by-drag, column reorder (recorded future tiers).
- Row virtualization beyond the O(viewport) slice (windowed data fetch = future).

## Steps

### Step 1: Specify the data model

Draft in the doc (refine against POST-013 idiom):

```rust
pub struct Column<'a, ColId> {
    pub id: ColId,                      // stable, for sort outcomes + width overrides
    pub title: &'a str,
    pub width: ColumnWidth,             // Fixed(u16) | Min(u16) | Fill(weight) — the solver's input
    pub align: Alignment,               // Left | Right | Center (numbers right)
    pub sort: Option<SortDirection>,    // renders the indicator; None = unsorted/unsortable per a flag
}
pub struct TableRow<'a, Id> {
    pub id: Id,
    pub cells: &'a [Cell<'a>],          // Cell = &str or Line? DECIDE: &str + per-cell Option<Style> keeps it flat; Line<'a> maximizes styling. Evaluate both against DetailRow precedent.
    pub enabled: bool,
    pub emphasis: bool,
}
pub enum TableOutcome<Id, ColId> { Ignored, Selected(Id), Activated(Id), SortRequested(ColId), Cancelled }
```

Pin the invariants: `cells.len()` vs `columns.len()` mismatch behavior (truncate + debug_assert? document), stable-Id selection surviving caller re-sort (Id-sticky — the reason sorting stays caller-side), header row height (1), separator policy (column gap `  ` vs `│` — theme-role question).

### Step 2: Prototype the width solver

Pure function + table-driven tests (the spike's hard deliverable):

`resolve_widths(columns: &[ColumnWidth], available: u16) -> Vec<u16>` — semantics: Fixed always exact; Min gets at least its floor; Fill divides the remainder by weight; overflow order when available < sum-of-mins (shrink Fill→Min-floors→then truncate rightmost Fixed? decide + document); zero-width and 1-column degenerates; deterministic rounding (leftover cells go left-to-right). ≥10 test cases including: all-Fill, mixed, overflow, exact-fit, available=0, single huge Min, weight ties.

**Verify**: solver tests green; behavior table in the doc matches the tests 1:1.

### Step 3: Story sketch + interaction walkthrough

Render a static 5-column × 12-row table in a lookbook-local story via the solver (header + alignment + a sorted column's `▼` + selection bar + truncated CJK/emoji cell proving `display_cols_slice` integration). Walk (on paper, in the doc) the full interaction contract against DetailTable's machinery: `handle_key` (Up/Down/Home/End/PageUp/PageDown/Enter → `Activated`), `click` on row → select, click on HEADER → `SortRequested(col)`, hover, `scroll_by`, painted regions (`row_area` + per-header `Rect`s), narrow-terminal behavior (which columns collapse first — tie to `ColumnWidth` policy: Fill columns shrink first, then a priority field? record the open question).

**Verify**: story renders in the gallery; unicode truncation visibly correct; `cargo test --workspace` green; committed previews untouched (lookbook-local story not registered in the catalog — keep it out of `stories()` or the catalog gate fires; note the mechanism used).

### Step 4: Design doc + build-plan stub

`plans/033-table-design.md`: data model + invariants, solver spec + its test table, outcome/interaction contract, theming roles needed (`TableHeader`, `TableSelection`? — reuse-first per Plan 030's rule), the O(viewport) + measurement-cache plan (Plan 016's `Measured` applies), narrow-collapse open question, future tiers (resize/reorder/editing/virtualized fetch), and the build-plan checklist (widget + stories incl. narrow/unicode variants + contract row + hot-path test + component page).

**Verify**: doc exists; README row updated.

## Done criteria

- [ ] `plans/033-table-design.md`: model, solver spec, interaction contract, roles, perf plan, open questions
- [ ] Width solver prototyped with ≥10 passing table-driven tests
- [ ] Static story sketch renders (unicode truncation proven)
- [ ] Zero `crates/termrock/src/` changes; gates green
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plans 011/013 not DONE — stop, dependencies.
- The solver can't be both deterministic and gap-free at extreme widths (design contradiction found) — that IS a spike result; document the contradiction + chosen tradeoff and stop.
- The `Cell` representation decision (str vs Line) deadlocks on a real constraint (e.g. per-cell links à la DetailRow's href) — present both in the doc with the constraint spelled out; maintainer picks.

## Maintenance notes

- DetailTable stays: it's the ergonomic 2-column special case (label:value + copy/link affordances). The doc must state the boundary so consumers know which to pick — and whether DetailTable eventually becomes a Table preset (future consolidation question, not now).
- Sorting stays caller-executed forever (ownership doctrine) — `SortRequested` is the entire widget-side contract.
- The width solver is deliberately a pure public-testable function — future column-resize drags recompute through the same solver.
