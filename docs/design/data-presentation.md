# Data presentation system

**Status:** design SoT + foundation module `termrock::widgets::data_view`  
**Audience:** database tools, infra/observability apps, developer workbenches  
**Rule:** useful shared abstractions **without** one excessively generic trait.

---

## 1. Goals

| Domain | Needs |
|--------|--------|
| DB tools | Wide tables, sort/filter, cell copy, virtualize millions of logical rows |
| Infra / k8s | Tree of resources, status columns, rapid updates |
| Observability | Log streams, timelines, metrics sparklines, diagnostics |
| Dev workbenches | Diff review, object inspectors, terminal output, KV details |

**Non-goals:** embed SQL engines, own network fetch, force every surface through `DataTable`.

---

## 2. Architecture: shared kits, specialized surfaces

```
┌─────────────────────────────────────────────────────────────┐
│ data_view kits (compose, don’t inherit a mega-trait)        │
│  VirtualWindow · ColumnModel · SelectionModel · LoadState   │
│  DataDensity · SortSpec · FilterSpec · CopyPayload · Expand │
└───────────────┬─────────────────────────────────────────────┘
                │ used by
┌───────────────▼─────────────────────────────────────────────┐
│ Surfaces (specialized widgets)                              │
│  DataTable · TreeTable · KeyValueTable · ObjectInspector    │
│  VirtualGrid · LogStream · Timeline · DiffReview            │
│  Diagnostic · Metrics/Sparkline · TerminalOutput            │
└─────────────────────────────────────────────────────────────┘
```

**Why not one trait?** Logs don’t need column pin; diffs don’t need multi-row select; sparklines don’t need sticky headers. Shared **value types + small state machines** scale better than `trait DataSurface`.

### Mapping from today

| Today | Target |
|-------|--------|
| `Table` | Evolve → **DataTable** (or alias); gain virtualization hooks |
| `VirtualGrid` | Stay 2-axis resident projection; align column/selection kits |
| `DetailTable` | **KeyValueTable** (+ ObjectInspector composition) |
| `LogPane` | **LogStream** (follow, levels, virtualize) |
| `Timeline` | Keep; adopt `LoadState` / density |
| `DiffView` | **DiffReview** (hunk nav already sketched) |
| `Sparkline` / meters | **Metrics** pack |
| (gap) | TreeTable, Diagnostic, TerminalOutput, ObjectInspector |

---

## 3. Common kits (`data_view`)

### 3.1 `VirtualWindow`

- `offset`, `viewport`, `logical_len` (u64 — supports 1e6+).  
- `scroll_by`, `reveal`, `visible_range` → **O(1)** math.  
- **Invariant:** paint never iterates `0..logical_len`.

### 3.2 `ColumnModel` / `DataColumn`

- Width: Fixed / Min / Fill.  
- `visible`, `pin` (None/Start/End), `priority` (responsive drop).  
- `contract_to_budget` drops low priority first; primary cols high priority.  
- Resize overrides optional.

### 3.3 `SelectionModel`

- Modes: None, Row, MultiRow, Cell, CellRange.  
- Focus row/col separate from selection set.  
- Multi via `BTreeSet<RowId>`.

### 3.4 `LoadState`

Idle · Loading · Partial · Ready · Empty · Error(retryable).

### 3.5 `DataDensity`

Compact vs Comfortable (row pad); maps from design `Density`.

### 3.6 Sort / Filter / Copy / Expand

- `SortSpec`, `FilterSpec` — **chrome emits; consumer re-projects data**.  
- `CopyPayload` — consumer clipboard/OSC 52.  
- `ExpandState` — detail rows / tree expand.

### 3.7 `DataViewOutcome`

Shared outcome vocabulary; surfaces add domain-specific variants via their own enums (prefer composition over inheritance).

---

## 4. Surface designs

### 4.1 DataTable (flagship)

**Purpose:** Primary grid for DB/infra tables.

**Anatomy**
```
┌ sticky header (pinned cols | scrollable cols) ─────────────┐
│ [sel] colA▲ | colB | …          ← horizontal scroll body  │
│ body rows (virtual window)                                  │
│ optional group headers                                      │
│ expandable detail under row                                 │
├ footer: count · load · selection summary                    │
└─────────────────────────────────────────────────────────────┘
```

| Capability | Design |
|------------|--------|
| Large virtualized data | `VirtualWindow` + consumer `project(start..end) -> rows` |
| Sticky headers | Header outside vertical scroll; redraw each frame |
| Sorting | Header click / keys → `SortRequested`; consumer sorts |
| Filtering / search | Filter bar chrome → `FilterChanged` |
| Column resize | Drag header edge → `ColumnResized` |
| Column visibility | Menu / keys → `ColumnVisibility` |
| Column pinning | `ColumnPin::Start/End`; pin strips + center scroll |
| Row / cell / multi select | `SelectionModel` modes |
| Keyboard | Arrows, Page, Home/End, Space toggle multi, Enter activate, `/` search, `c` copy |
| Mouse | Click select, drag range, wheel, header sort, context click |
| Contextual actions | `ContextMenu` outcome → OverlayStack menu |
| Inline editing | `EditStarted/Committed` on editable columns |
| Loading / partial | `LoadState::Loading/Partial` skeleton rows |
| Empty / error | Centered empty/error panels + Retry |
| Grouping | Group header rows in projection stream |
| Expandable detail | `ExpandState` + nested pane height |
| Horizontal scroll | Second `VirtualWindow` on columns |
| Responsive priority | `ColumnModel::contract_to_budget` |
| Copy cells/ranges | `CopyPayload` |
| Colorless | Selection gutter glyph; sort ▲/▼; status letters |
| Density | `DataDensity` |

**State (sketch)**
```rust
pub struct DataTableState<RowId, ColId> {
    pub rows: VirtualWindow,
    pub cols: VirtualWindow,
    pub columns: ColumnModel<ColId>,
    pub selection: SelectionModel<RowId>,
    pub expand: ExpandState<RowId>,
    pub load: LoadState,
    pub density: DataDensity,
    pub sort: Option<SortSpec<ColId>>,
    pub filter: FilterSpec,
    // edit buffer, hit regions, …
}
```

**Consumer projection contract**
```rust
// Called each frame with visible range only
fn project_rows(start: u64, end: u64) -> Vec<DataTableRow<'_, RowId>>;
```

### 4.2 TreeTable

Hierarchical rows + columns (k8s resources, file+meta).

- Reuses `ColumnModel` + `SelectionModel` + `ExpandState`.  
- Indent + expand glyph in first column.  
- Keyboard: Left collapse / Right expand (intents).  
- Virtualize **flattened** visible rows only.

### 4.3 KeyValueTable

Evolved `DetailTable`: label/value, copy, link, search filter on keys.

- No column pin; selection is row-only.  
- Uses `LoadState`, `CopyPayload`, density.

### 4.4 ObjectInspector

Nested structure (JSON-like) via tree of KV / sections.

- Compose Tree + KeyValueTable.  
- Expand paths; breadcrumb optional.  
- Not a generic table — specialized navigation.

### 4.5 VirtualGrid

Keep 2D resident/pending cells (spreadsheet-ish).

- Align with `CellCoord`, `SelectionModel::Cell`, column width kit.  
- Pending cells already exist (`GridCell::pending`).  
- Benchmark: viewport-only page requests.

### 4.6 LogStream

Evolved `LogPane`:

- Follow-tail, level filter, search highlight.  
- Virtualize by line index; append O(1) ring.  
- Streaming: `LoadState::Partial`.  
- Colorless: level prefix `E/W/I/D`.

### 4.7 Timeline

Keep event list; add `LoadState`, multi-select optional, density.

### 4.8 DiffReview

Unified/split, hunk nav, streaming patch, stage outcomes (see agent design).

- Colorless `+`/`-`.  
- Narrow forces unified (`ResponsiveSurface::DiffReview`).

### 4.9 Diagnostic

Problem list (LSP-style): severity, source, message, location.

- Row select + jump outcome `Activated { path, line }`.  
- Group by file optional.  
- Colorless severity letters.

### 4.10 Metrics & sparklines

Keep `Sparkline`, `BarSeries`, `SegmentedMeter`.

- Live window of points (ring buffer consumer-owned).  
- No selection required.  
- Colorless: density glyphs.

### 4.11 TerminalOutput

PTY-like scrollback presentation (consumer owns PTY).

- Virtualized lines, search, copy range.  
- Optional CSI-lite style runs as borrowed spans.  
- Follow + scrollback break on wheel (LogStream pattern).

---

## 5. DataTable feature matrix (acceptance)

| Feature | Kit / surface | Acceptance |
|---------|---------------|------------|
| 1M logical rows | VirtualWindow | Paint ≤ viewport cells; no `Vec` of 1M |
| Sticky header | layout | Header y fixed while body scrolls |
| Sort | SortSpec | Outcome only; icon on header |
| Filter/search | FilterSpec | Reproject ≤ 16ms story target for 10k resident |
| Col resize/vis/pin | ColumnModel | State round-trip tests |
| Selection modes | SelectionModel | Keyboard + mouse tests |
| Multi-select | BTreeSet | Space toggles; shift-range later |
| Inline edit | outcomes | Esc cancel; Enter commit |
| Grouping/expand | ExpandState | Toggle keeps window stable |
| H-scroll | col VirtualWindow | Pin start still visible |
| Responsive cols | priority | 40-col terminal drops low priority |
| Copy | CopyPayload | TSV range shape |
| Empty/error/load | LoadState | Stories |
| Density | DataDensity | Comfortable pad ≥ compact |
| CJK / combining | text display_cols | Snapshot stories |
| Streaming | Partial load | Append doesn’t reset selection id |

---

## 6. Performance strategy

| Scale | Strategy |
|-------|----------|
| 10 rows | Full project fine |
| 10k rows | Resident projection OK; still virtualize paint |
| 1M logical | Index-only window; page fetch by range; no per-row alloc in TermRock |
| Very wide | Column window + pin strips; measure headers once |
| Rapid stream | Append path; reuse buffers; don’t rebuild selection sets from scratch |
| Hot path tests | Existing `*_hot_path.rs` style — assert no full scans |

**Benchmark targets (see `data_view::bench`)**

| Name | Target |
|------|--------|
| `ROWS_10` | Story correctness |
| `ROWS_10K` | Interactive sort/filter feel |
| `ROWS_1M` | Virtualization correctness |
| `COLS_WIDE` (64) | H-scroll + pin |
| `MAX_PAINT_CELLS` | 40×64 bound |

**CI benches (plan):** criterion or custom — paint 1M window < threshold on CI machine class; fail if paint walks logical_len.

---

## 7. Stories (Studio / lookbook)

| Story id | Proves |
|----------|--------|
| `datatable/rows-10` | Baseline chrome |
| `datatable/rows-10k` | Scroll performance |
| `datatable/rows-1m-virtual` | Only viewport projected |
| `datatable/wide-64` | H-scroll + pins |
| `datatable/cjk` | Width measure |
| `datatable/combining` | Grapheme safety |
| `datatable/stream-partial` | Rapid append |
| `datatable/narrow-priority` | Column drop order |
| `datatable/empty` / `error` / `loading` | LoadState |
| `datatable/multi-select` | Selection |
| `datatable/inline-edit` | Edit outcomes |
| `treetable/expand` | Hierarchy |
| `kv/copy` | KeyValue |
| `logstream/follow` | Follow break |
| `diff/narrow-unified` | Responsive |
| `diagnostic/severity` | Colorless letters |
| `metrics/sparkline-live` | Stream points |
| `terminal-output/scrollback` | Virtual lines |

---

## 8. Accessibility / colorless

- Selection: gutter `›` / `*` not color alone.  
- Sort: `▲`/`▼` or `^`/`v` ascii.  
- Severity: `E`/`W`/`I`.  
- Diff: `+`/`-`.  
- Focus-visible: `Role::BorderFocused` on panel.  
- Hit regions for all interactive headers/cells.

---

## 9. Implementation plan

| Phase | Deliverable |
|-------|-------------|
| **D0** ✅ | Design + `data_view` kits + unit tests |
| **D1** | DataTable state shell: virtual rows + sticky header + selection |
| **D2** | Column resize/visibility/pin + responsive contract |
| **D3** | Sort/filter chrome outcomes + empty/load/error |
| **D4** | Multi-select, copy range, expand detail |
| **D5** | Inline edit |
| **D6** | TreeTable |
| **D7** | LogStream / TerminalOutput virtualize |
| **D8** | ObjectInspector + Diagnostic |
| **D9** | 1M + stream stories/benches; migrate `Table` → DataTable |
| **D10** | Migration doc when `Table` public API renames |

`Table` / `VirtualGrid` / `DetailTable` keep working until D9; dual-run then fold.

---

## 10. API sketch (DataTable)

```rust
// Consumer each frame:
let (start, end) = state.rows.visible_range();
let rows = db.project(start, end); // only this slice

DataTable::new(&columns, &rows, &tokens)
  .density(DataDensity::Compact)
  .render(area, buf, &mut state);

match state.handle_key(key) {
    DataTableOutcome::SortRequested(spec) => db.set_sort(spec),
    DataTableOutcome::Copy(payload) => clipboard.write(payload),
    DataTableOutcome::RowActivated(id) => open(id),
    _ => {}
}
```

---

## 11. Decision summary

1. **Kits not mega-traits.**  
2. **Virtualization is mandatory** for large logical sets.  
3. **Consumer owns data + sort/filter execution.**  
4. **TermRock owns** geometry, selection chrome, contraction, hits, outcomes.  
5. **Specialized surfaces** for logs, diffs, metrics, trees, inspectors.  
6. **Stories scale** from 10 → 1M logical with the same API.
