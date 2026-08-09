# Virtualizer (1D / 2D large collections)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0086-v0.13.0-virtualizer.md` |
| **Module** | `widgets::virtualizer` |
| **Studio** | `virtualizer/million-fixed` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `data_view::VirtualWindow` | **Preserve** as fixed-extent (1 unit/item) facade; methods shared with Virtualizer |
| `VirtualGridState` first_row/body_rows math | **Migrate** body window through `Virtualizer` |
| `DataTableState::{window,col_window}` | **Preserve** `VirtualWindow` fields; constructible from/into Virtualizer |
| `CollectionState` offset | **Preserve** (list window); can adopt Virtualizer later |
| Full logical `Vec` of extents for 1M rows | **Forbidden** — sparse measure map only |
| SemanticScene nodes for every logical row | **Forbidden** — register **visible (+ sticky)** only |

## Mission

Reusable one- and two-dimensional virtualizer for large collections and grids:
fixed/variable extents, overscan, sticky regions, stable IDs, visible-range
queries, resize/insert/delete/filter + anchor preservation, O(viewport) measure
and paint, million-row logical datasets without per-row allocation.

## Research anchors

| Source | Takeaway |
|--------|----------|
| TermRock VirtualGrid | Resident projection only; absolute indices; no phantom hits |
| VirtualWindow | O(1) fixed slot math for 1M rows |
| Web virtual lists | Overscan prefetch; sticky headers; content-id anchors |
| VisiData / Textual tables | Sparse materialization; viewport-bound paint |

## API (shipped)

```rust
ExtentPolicy::{Fixed(u16), Variable { estimated: u16 }}
StickyRegion { leading, trailing }  // always-in-set counts
VirtSlice { start, end, measure_start, measure_end }  // half-open
Virtualizer {
  logical_len, viewport_extent, offset (item index),
  overscan, policy, sticky, measured: BTreeMap (variable only),
  anchor: Option<ScrollAnchor>,
}
  set_len / set_viewport_extent / set_overscan / set_sticky
  scroll_by / reveal / clamp / apply_anchor
  note_measured(index, extent)  // sparse; never O(n) store
  forget_measured_outside(pad)  // drop cold measures after filter/reflow
  on_items_changed(new_len)     // clamp + re-resolve anchor
  visible_slice() / measure_slice() / semantic_count()
  total_extent_estimate()
Virtualizer2D { rows: Virtualizer, cols: Virtualizer }
  visible_cells_budget() → row_len * col_len (cap checks)
```

## Laws

1. Paint and semantic registration are **O(visible + sticky + overscan measure)**, never O(logical_len).
2. Variable extents: host measures only `measure_slice`; Virtualizer stores sparse overrides.
3. Sticky leading/trailing are always part of the semantic set; body virtualizes the middle.
4. Anchors prefer `ContentId` when host resolves; else Index / FromEnd.
5. Insert/delete/filter → `on_items_changed` + optional `apply_anchor`; no full remeasure.
