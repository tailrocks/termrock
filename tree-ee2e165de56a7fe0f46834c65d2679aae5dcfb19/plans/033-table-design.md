# Table design spike

## Recommendation

Build `Table` as a new borrowed, stable-ID, stateful widget. Keep `DetailTable`
as the ergonomic label/value surface with copy and hyperlink affordances.
`Table` owns column layout, header and row rendering, selection, viewport state,
painted hit geometry, and sort-request outcomes. The caller owns data, row
ordering, sorting execution, cell wording, and domain effects.

The lookbook prototype proves the risky seam: one deterministic solver maps a
cell-width budget to every resolved column width. It also renders a five-column,
twelve-row app-only gallery story with a header, `▼`, three alignments, a
selection bar, and display-column-safe CJK/emoji clipping. `stories()` remains
the public catalog source; `gallery_stories()` appends the prototype only for
the interactive gallery. Generated inventory and previews therefore remain
unchanged.

## Drift and dependencies

Plans 011 and 013 are DONE. Since the planning baseline, `DetailTable` moved
keyboard, pointer, selection, scroll, and painted-region behavior into
`DetailTableState`; `List` gained stable-ID paging, disabled-row skipping, and
canonical private widget construction. Those are the interaction and API
templates. No pre-011 widget-owned outcome methods or public widget fields
should return.

## Public model

```rust
use std::num::NonZeroU16;
use ratatui_core::{style::Style, text::Line};

pub struct Column<'a, ColId> {
    pub id: ColId,
    pub title: Line<'a>,
    pub width: ColumnWidth,
    pub alignment: CellAlignment,
    pub sortable: bool,
    pub sort: Option<SortDirection>,
}

pub enum ColumnWidth {
    Fixed(u16),
    Min(u16),
    Fill(NonZeroU16),
}

pub enum CellAlignment { Left, Center, Right }
pub enum SortDirection { Ascending, Descending }

pub struct TableRow<'a, Id> {
    pub id: Id,
    pub cells: &'a [Line<'a>],
    pub enabled: bool,
    pub emphasis: bool,
    pub style: Option<Style>,
}

#[non_exhaustive]
pub enum TableOutcome<Id, ColId> {
    Ignored,
    Selected(Id),
    Activated(Id),
    SortRequested(ColId),
    Cancelled,
}

pub struct Table<'a, Id, ColId> { /* private borrowed fields */ }

impl<'a, Id, ColId> Table<'a, Id, ColId> {
    pub const fn new(
        columns: &'a [Column<'a, ColId>],
        rows: &'a [TableRow<'a, Id>],
        theme: &'a Theme,
    ) -> Self;
    pub const fn column_gap(self, gap: u16) -> Self;
}
```

`Line<'a>` wins over `&str` and over a second cell wrapper. It already supports
borrowed or owned text and per-span styling, matches `ListRow`, and avoids a
parallel style field. Alignment applies to the measured visible line as a
whole. Links, editing, and other cell capabilities need a deliberate future
cell-interaction model; they must not be smuggled into layout data now.

`sortable` is separate from `sort`: `sortable: true, sort: None` means an
unsorted header can emit `SortRequested`; `sortable: false` is inert. At most
one column may carry `sort: Some`, and a sorted column must be sortable. Debug
builds assert both invariants. Release rendering ignores `sort` on inert
columns and shows only the first valid sorted column, preventing ambiguous
indicators without making render fail. `Ascending` always paints `▲`;
`Descending` always paints `▼`.

### Row and identity invariants

- Row and column IDs are stable and unique in their respective slices. Debug
  builds reject duplicates; release behavior is first-ID-wins for hit routing.
- `cells.len() == columns.len()` is debug-asserted. Release rendering treats
  missing cells as empty and ignores excess cells. Geometry always follows the
  column slice, never malformed row length.
- Selection is ID-sticky. After caller sorting or filtering,
  `reconcile(rows)` keeps the selected ID when it remains enabled; otherwise it
  selects the nearest enabled row by the previous projected index.
- Disabled rows render with `Role::TextDisabled`, expose no row hit region, and
  are skipped by all navigation and activation.
- The header is exactly one terminal row. It remains visible while body rows
  scroll vertically.
- The default separator is a two-cell blank gap. It is quiet, theme-neutral,
  and leaves every painted cell unambiguous. A future visible-rule option must
  use `Role::Border`; it is not the default.
- The two-cell `▸ ` / `  ` row marker is outside the solver's cell budget and
  is the non-color selection cue.

## Width solver

The public-testable pure function is:

```rust
pub fn resolve_widths(columns: &[ColumnWidth], available: u16) -> Vec<u16>;
```

`available` is the **cell budget**. The joint layout resolver owns conversion
from viewport width to that budget; this keeps the pure width solver independent
of visual separator policy.

Algorithm:

1. Seed `Fixed(n)` and `Min(n)` with `n`; seed `Fill(_)` with zero.
2. If the seed fits, distribute all remaining cells among `Fill` columns in
   weight proportion. Integer quotient is assigned first. Remainder cells go
   to eligible columns from left to right. `NonZeroU16` makes an inert fill
   impossible.
3. If the seed overflows, Fill is already zero. Reduce `Min` columns from
   right to left until the deficit is gone, then reduce `Fixed` columns from
   right to left as the last-resort clipping tier.

Thus Fixed is exact whenever the viewport can honor the mandatory widths. At
impossible widths, rightmost content disappears first and left identity columns
survive longest. For any input, every output is bounded by `u16`, total width
never exceeds `available`, and a layout containing Fill consumes the full
budget. A layout containing only Fixed/Min intentionally leaves slack when its
declared widths are smaller than the budget.

### Executable behavior table

The table maps 1:1 to the prototype's table-driven test.

| Input widths | Available | Resolved | Contract |
|---|---:|---:|---|
| `[]` | 10 | `[]` | empty |
| `[Fill(1)]` | 0 | `[0]` | zero budget |
| `[Fill(1)]` | 7 | `[7]` | single fill |
| `[Fill(1), Fill(1)]` | 5 | `[3, 2]` | tie remainder goes left |
| `[Fill(1), Fill(2)]` | 9 | `[3, 6]` | weighted fill |
| `[Fixed(4), Min(3)]` | 7 | `[4, 3]` | exact mandatory fit |
| `[Fixed(4), Fill(1), Min(3)]` | 12 | `[4, 5, 3]` | mixed remainder |
| `[Fixed(4), Min(3)]` | 6 | `[4, 2]` | Min shrinks first |
| `[Fixed(4), Min(3)]` | 3 | `[3, 0]` | Fixed clips last |
| `[Min(100)]` | 9 | `[9]` | huge Min |
| `[Min(2), Min(3), Min(4)]` | 6 | `[2, 3, 1]` | rightmost Min first |
| `[Fixed(0), Fill(2)]` | 7 | `[0, 7]` | zero-width column |
| `[Fixed(2), Fixed(3), Fixed(4)]` | 4 | `[2, 2, 0]` | rightmost Fixed first |

No determinism or gap-free contradiction exists. Arithmetic uses widened
integers for weighted multiplication and converts only bounded shares back to
`u16`.

### Joint visibility and gap resolution

Naively removing gaps after a column reaches zero can oscillate: with
`[Fixed(4), Min(3)]`, viewport 5, and gap 2, reserving the gap yields `[3, 0]`,
while reclaiming it yields `[4, 1]`, which asks for the gap again. The canonical
resolver uses a one-way tie-break:

1. Remove inherently invisible `Fixed(0)` and `Min(0)` columns.
2. For the remaining active columns, reserve gaps between all active columns,
   then call `resolve_widths` with the remaining cell budget.
3. Permanently hide every column resolved to zero for this frame.
4. If gaps consume the whole nonzero viewport and every active column resolves
   to zero, preserve only the first viable column, remove every gap, and solve
   it again.
5. Re-solve the survivors once with only their gaps. Paint gaps only between
   survivors. Removing zero-width policies and gaps only increases the budget
   available to each survivor, so no further elimination pass is needed.

The two-pass algorithm is O(columns), terminates, and never oscillates. Removed
columns do not revive after their gaps are reclaimed. This order-preserving
rule makes earlier identity columns survive narrow widths, eliminates phantom
gaps, and paints at least one viable column whenever the post-marker viewport
is nonzero. Tests pin the oscillating example, two Fill columns in two cells,
zero viewport, and an inherently hidden leading column.

## Rendering and interaction contract

`TableState<Id, ColId>` owns selected and hovered row IDs, the hovered column
ID, focus, body offset, viewport height, last projected index, row hit regions,
header hit regions, and resolved widths. Painted geometry is replaced on every
render; pointer input never predicts layout.

| Input | Outcome/state |
|---|---|
| Up/Down, j/k | previous/next enabled row, wrapping |
| Home/End | first/last enabled row |
| PageUp/PageDown | move by painted body height and clamp |
| Enter | `Activated(selected_id)` only when enabled |
| Esc | `Cancelled` |
| pointer move | hover only a painted enabled row/header |
| row click | select row and return `Selected(id)` |
| any row double-click | no special behavior; the neutral input vocabulary has no double-click event |
| sortable header click | `SortRequested(column_id)` |
| inert header/gap click | `Ignored` |
| wheel / `scroll_by` | bounded body offset; header remains fixed |

Header regions contain stable column IDs and exact painted rectangles. Row
regions contain stable row IDs and span only the painted body row. If future
cell outcomes arrive, per-cell regions are derived from the same solved column
geometry, not recomputed in input handlers.

Sorting execution always stays caller-owned. On `SortRequested`, the caller
chooses direction, reorders its rows, projects the new `sort` marker, and calls
render again. Stable row IDs preserve selection across that reorder.

## Narrow terminals

The current policy is deliberately order-based: Fill reaches zero first, then
rightmost Min, then rightmost Fixed. This proves a deterministic baseline but
does not express semantic importance. The build should add an explicit
`collapse_priority` only after real story evidence shows order is insufficient.
Do not overload weight: weight controls surplus distribution, not importance.
Columns resolved to zero consume no gap. The one-way joint resolver above
reclaims their separators without reviving them, so hidden columns cannot leave
blank separators or create an oscillation.

Open question for the build: whether a zero-width sorted column should expose
its sort state in an overflow indicator. Default recommendation: no hidden
header indicator; consumers should keep important sortable columns earlier or
use future priority metadata.

## Theme roles

Reuse existing roles first:

- header: `Role::TextStrong`
- ordinary/emphasized cell: `Role::Text` / `Role::Accent`
- disabled row: `Role::TextDisabled`
- selected row and marker: `Role::Selection`
- hovered row: `Role::Focus`
- focused outer chrome, when composed in a Panel: `Role::BorderFocused`
- optional future visible separators: `Role::Border`

No `TableHeader` or `TableSelection` role is justified: existing semantic roles
already express those meanings and both presets visibly support them. Add a role
only when a concrete independent theming requirement cannot be expressed here.

## Performance plan

- Render only `rows[offset..offset + body_height]`; work is O(columns × visible
  rows), never O(total rows). A 10,000-row input must produce at most
  `body_height` row regions.
- Joint layout is two O(columns) passes and needs no content-derived
  intrinsic measurement: Fixed/Min/Fill depends only on policy and viewport.
  Do not use `Measured` or scan offscreen cells. If profiling later justifies a
  geometry cache, key the resolved widths by exact column-width policies,
  viewport width, marker width, and gap; body offset is irrelevant to geometry.
- The public convenience `resolve_widths` may return `Vec<u16>` for direct
  testing and ad-hoc layout. Widget rendering uses an internal
  `resolve_widths_into(columns, available, &mut state.resolved_widths)` plus
  state-owned `visible_columns` and `solver_policies` scratch vectors. Render
  clears and reuses their capacity; it never calls the allocating convenience
  function after warmup.
- Do not clone cells or rows. Reuse scratch strings for grapheme-safe clipping
  through `display_cols_slice_into` in the final widget.
- Measure and clip only header plus visible-window cells. Keep stable-ID
  reconciliation separate from rendering. Caller sort
  changes order but not identity.
- Add a warmed 10,000-row hot-path test matching `tree_hot_path`: bounded
  regions, allocation-free render after warmup, and a measured tolerance.

## DetailTable boundary

Choose `DetailTable` for label/value facts with wrapping, horizontal scrolling,
copy confirmation, and hyperlink activation. Choose `Table` for a homogeneous
row set with named columns, alignment, sorting requests, and header interaction.
`DetailTable` should not become a Table preset yet: its label-width wrapping,
value hyperlink geometry, and copy affordance are materially different. A
future consolidation may share private measurement primitives, never a public
compatibility facade or duplicated rendering body.

## Future tiers

1. Drag resize: caller-owned width overrides feed the same solver; state owns
   only painted divider geometry and drag status.
2. Column reorder: caller reorders the borrowed column/cell projection using
   stable column IDs.
3. Cell editing: a separate editor/composition contract, not embedded domain
   mutation in Table.
4. Windowed fetch: caller supplies a total row count plus visible borrowed
   window; Table remains unaware of executors and data sources.
5. Cell capabilities: typed domain-neutral link/action projections with painted
   cell regions, designed from concrete requirements.

## Build-plan stub

- [ ] Add `widgets/table.rs` with the model, solver, canonical private builder,
  owned and borrowed `StatefulWidget` implementations.
- [ ] Add stable-ID state reconciliation, full keyboard outcomes, hover/click,
  body scrolling, header and row regions.
- [ ] Add exact solver tests plus visible-column gap tests at zero/tiny widths.
- [ ] Add Unicode alignment/truncation tests for `Line` spans and combining,
  CJK, emoji, control, and narrow-boundary cases.
- [ ] Add basic, narrow, Unicode, disabled, empty, and sorted stories with
  deterministic previews.
- [ ] Add API inventory, exact component contract row, docs page, catalog story
  coverage, and component inventory prose.
- [ ] Add the 10,000-row warmed hot-path proof, scratch-buffer reuse checks,
  and allocation-free warm-render assertion.
- [ ] Add the next numbered migration if implementation changes any existing
  public shared primitive; Table itself is additive.
- [ ] Validate with `mise run gate`, direct gallery walkthrough, preview check,
  package verification, and feature powerset.
