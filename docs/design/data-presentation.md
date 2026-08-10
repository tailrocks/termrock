# Data presentation system

| Field | Value |
|-------|-------|
| **Status** | Design SoT + kits (`data_view`) + evolving surfaces |
| **Audience** | Database tools, infrastructure/k8s, observability, developer workbenches |
| **Code** | `crates/termrock/src/widgets/data_view.rs`, `data_table.rs`, `virtual_grid.rs`, `review.rs`, `detail_table.rs`, `charts.rs`, `diff.rs`, `agent.rs` (Timeline), `log_pane.rs` |
| **Rule** | Shared **kits**, specialized **surfaces** — no mega-trait |

---

## 1. Goals

| Domain | Needs |
|--------|--------|
| **DB tools** | Wide tables, sort/filter/search, cell copy, 1M logical rows virtualized |
| **Infra / k8s** | Tree+columns, status, rapid updates, responsive drop |
| **Observability** | Log streams, timelines, sparklines, diagnostics |
| **Dev workbenches** | Diff review, object inspectors, terminal output, KV details |

### Non-goals

- Embed SQL / query engines or network fetch.
- Force every surface through `DataTable`.
- Own clipboard I/O (emit `CopyPayload` only).
- Own PTY lifecycle (TerminalOutput is presentation only).

---

## 2. Architecture: kits compose; surfaces specialize

```
┌──────────────────────────────────────────────────────────────┐
│ data_view kits (compose; no mega-trait)                      │
│  VirtualWindow · ColumnModel · SelectionModel · LoadState    │
│  DataDensity · SortSpec · FilterSpec · CopyPayload           │
│  ExpandState · GroupHeader · CellCoord · DataViewOutcome     │
│  bench::{ROWS_10, ROWS_10K, ROWS_1M, COLS_WIDE, …}           │
└────────────────────────────┬─────────────────────────────────┘
                             │ used by
┌────────────────────────────▼─────────────────────────────────┐
│ Surfaces                                                     │
│  DataTable · TreeTable · KeyValueTable · ObjectInspector     │
│  VirtualGrid · LogStream · Timeline · DiffReview             │
│  Diagnostic · Metrics/Sparkline · TerminalOutput             │
└──────────────────────────────────────────────────────────────┘
```

**Why not one trait?** Logs need follow-tail, not column pin. Diffs need hunks, not multi-row select. Sparklines need rings, not sticky headers. Shared **value types + small machines** scale; `trait DataSurface` does not.

### Mapping from today → target

| Today | Target |
|-------|--------|
| `Table` | Evolve / alias → **DataTable** (virtualization + kits) |
| `VirtualGrid` | Keep 2-axis resident projection; share Column/Selection kits |
| `DetailTable` | **KeyValueTable** (+ ObjectInspector composition) |
| `LogPane` / `LogStream` | **LogStream** (follow, levels, virtualize) |
| `Timeline` | Keep; `LoadState` + density |
| `DiffView` / `DiffReview` | **DiffReview** hunk nav + stage outcomes |
| `Sparkline` / meters | **Metrics** pack |
| `ObjectInspector` | Nested inspect (exists seed) |
| (gap → ship) | TreeTable, Diagnostic, TerminalOutput as first-class |

---

## 3. Common kits (`termrock::widgets::data_view`)

### 3.1 `VirtualWindow`

| Field | Role |
|-------|------|
| `offset` | First visible logical index |
| `viewport` | Visible slots |
| `logical_len` | Universe size (`u64`; supports 1e6+) |

**API:** `scroll_by`, `reveal`, `visible_range`, `clamp`, `max_offset`  
**Invariant:** paint **never** iterates `0..logical_len`. Cost **O(viewport)**.

### 3.2 `ColumnModel` / `DataColumn`

| Field | Role |
|-------|------|
| `width` | `Fixed` / `Min` / `Fill(weight)` |
| `visible` | Show/hide |
| `pin` | `None` / `Start` / `End` |
| `priority` | Responsive: **lower drops first** |
| `sortable` / `editable` | Chrome affordances |

`contract_to_budget(budget, keep_min_priority)` drops unpinned low-priority columns.

### 3.3 `SelectionModel`

| Mode | Use |
|------|-----|
| `None` | Read-only chrome |
| `Row` | Single row |
| `MultiRow` | `BTreeSet<RowId>` + Space toggle |
| `Cell` | Single cell |
| `CellRange` | Anchor + active `CellCoord` |

Focus row/col **separate** from selection set. Multi never invents unloaded ids (Select-all is a **request**).

### 3.4 `LoadState`

`Idle` · `Loading` · `Partial { resident, total? }` · `Ready { count }` · `Empty` · `Error { retryable }`

### 3.5 `DataDensity`

`Compact` / `Comfortable` — cell pad; maps from design `Density`.

### 3.6 Sort / Filter / Copy / Expand

| Type | Law |
|------|-----|
| `SortSpec` | Chrome emits; **consumer re-projects** |
| `FilterSpec` | Query + opaque clauses; consumer executes |
| `CopyPayload` | Cell / Row / Range(TSV); consumer clipboard |
| `ExpandState` | Detail / tree expand ids |
| `GroupHeader` | Group rows in projected stream |

### 3.7 `DataViewOutcome`

Shared vocabulary (`Scrolled`, `SortRequested`, `Copy`, `EditStarted`, …). Surfaces may use their own enums that **map** to these; do not force one enum on all widgets.

### 3.8 Bench constants (`data_view::bench`)

| Const | Value | Meaning |
|-------|------:|---------|
| `ROWS_10` | 10 | Correctness stories |
| `ROWS_10K` | 10_000 | Interactive feel |
| `ROWS_1M` | 1_000_000 | Virtualization only |
| `COLS_WIDE` | 64 | H-scroll + pin |
| `VIEWPORT_ROWS` | 40 | Large terminal body |
| `MAX_PAINT_CELLS` | 40×64 | Paint cell budget |

---

## 4. Surface designs

### 4.1 DataTable (flagship)

**Purpose:** Primary grid for DB/infra tables.

**Anatomy**
```
┌ toolbar (optional) ─────────────────────────────────────────┐
├ sticky header: [pin-start | scrollable cols | pin-end] ─────┤
│ body: virtual rows · selection gutter · striped optional    │
│   group headers · expandable detail under row               │
├ footer: count · load · selection · filter chip ─────────────┤
└─────────────────────────────────────────────────────────────┘
```

| Capability | Design |
|------------|--------|
| Large virtualized data | `VirtualWindow` + consumer `project(start..end)` |
| Sticky headers | Header outside vertical scroll; redraw each frame |
| Sorting | Header click / `s` / sort key → `SortRequested` |
| Filtering / search | `/` or filter bar → `FilterChanged` |
| Column resize | Drag header edge → `ColumnResized` |
| Column visibility | Menu → `ColumnVisibility` |
| Column pinning | `ColumnPin::Start/End` strips + center scroll |
| Row / cell / multi select | `SelectionModel` modes |
| Keyboard | Arrows, Page, Home/End, Space multi, Enter activate, `/` search, `c` copy, `e` edit |
| Mouse | Click, drag range, wheel, header sort, context click |
| Contextual actions | `ContextMenu` → OverlayStack |
| Inline editing | `EditStarted` / `EditCommitted` / `EditCancelled` |
| Loading / partial | Skeleton / Partial footer |
| Empty / error | Centered panel + Retry |
| Grouping | `GroupHeader` rows in projection |
| Expandable detail | `ExpandState` + nested height |
| Horizontal scroll | Second `VirtualWindow` on columns |
| Responsive priority | `contract_to_budget` |
| Copy cells/ranges | `CopyPayload` |
| Colorless | Gutter `›`/`*`; sort `^`/`v`; no color-only select |
| Density | `DataDensity` |

**Consumer projection (mandatory for large sets)**
```rust
let (start, end) = state.window.visible_range();
let rows = source.project(start, end); // ONLY this slice
// paint DataTable with projected slice — never Vec of logical_len
```

**Select-all law:** `SelectAllRequested` applies to **projected/visible scope only** — never silent full-scan of unloaded universe.

### 4.2 TreeTable

Hierarchical rows + columns (k8s, process trees, schemas).

- Kits: `ColumnModel` + `SelectionModel` + `ExpandState` + `VirtualWindow` on **flattened** visible rows.  
- First column: indent + expand glyph (`▸`/`▾` or `>`/`v` ASCII).  
- Keys: Left collapse / Right expand (or intents).  
- Rapid updates: stable `RowId`; preserve expand set by id.

### 4.3 KeyValueTable

Dense interactive detail (migration **0191**): key · value · type · source ·
status · copy · edit · secret · validation · nested groups · compare mode.

- Single focus per row; chords (`c`/`e`/`r`/`d`/`/`).  
- Columns → stacked under width pressure.  
- `LoadState` for async object load.  
- `DetailTable` / `KeyValueList` remain for dialog and light summary use.

### 4.4 ObjectInspector

Nested structure (JSON/YAML-like).

- Compose tree navigation + KeyValueTable leaves.  
- Path expand; optional breadcrumb.  
- Outcomes: `PathActivated`, `Copy`, `ExpandToggled`.  
- Not a generic table.

### 4.5 VirtualGrid

2D resident/pending cells (spreadsheet / matrix).

- Align `CellCoord`, cell selection, column widths.  
- Pending cells (`GridCell::pending`) for async pages.  
- Viewport-only page requests; million-row story already in lookbook.

### 4.6 LogStream

Follow-tail log surface.

- Virtualize by line index; append O(1).  
- Follow until wheel/scroll breaks.  
- Level filter + search highlight (outcomes).  
- Colorless: `E`/`W`/`I`/`D` prefixes.  
- Streaming: `LoadState::Partial`.

### 4.7 Timeline

Ordered events (deploy, incidents, agent steps).

- Vertical list; density; optional multi-select.  
- `LoadState` for page-back history.  
- Activate → jump outcome.

### 4.8 DiffReview

Unified/split patches, hunk nav, accept/reject outcomes.

- Colorless mandatory `+`/`-`.  
- Narrow forces unified (`ResponsiveSurface`).  
- Streaming patch append mid-review.  
- See agent pack for multi-file model.

### 4.9 Diagnostic

LSP-style problem list.

| Col | Content |
|-----|---------|
| Severity | `E`/`W`/`I`/`H` letters |
| Source | linter / compiler id |
| Message | text |
| Location | path:line:col |

- Group by file optional.  
- `Activated { path, line, col }` jump.  
- Colorless severity letters only (no red-only).

### 4.10 Metrics & sparklines

`Sparkline`, `BarSeries`, `SegmentedMeter`, token meters.

- Consumer-owned ring of points.  
- No selection required.  
- Colorless: glyph density, not hue alone.  
- Live stream: replace window, no full history paint.

### 4.11 TerminalOutput

PTY-like scrollback **presentation** (consumer owns process).

- Virtualized lines; search; copy range.  
- Optional style runs as borrowed spans.  
- Follow + break on wheel (LogStream pattern).  
- Not a shell emulator core.

---

## 5. DataTable feature matrix (acceptance)

| Feature | Acceptance test / story |
|---------|-------------------------|
| 1M logical rows | Window math only; paint ≤ viewport; no `Vec` of 1M |
| Sticky header | Header y fixed while body scrolls |
| Sort | Outcome + header marker |
| Filter/search | `FilterChanged`; 10k reproject story |
| Col resize / vis / pin | State round-trip |
| Selection modes | Keyboard + mouse |
| Multi-select | Space; Ctrl+A = request visible only |
| Inline edit | Esc cancel; Enter commit |
| Group / expand | Toggle keeps window stable |
| H-scroll | Pin start visible |
| Responsive cols | 22-col drops low priority |
| Copy | TSV range shape |
| Empty / error / load | Stories |
| Density | Comfortable pad ≥ compact |
| CJK / combining | display_cols stories |
| Streaming partial | Append keeps selection **ids** |

---

## 6. Performance strategy

| Scale | Strategy |
|------:|----------|
| **10 rows** | Full project fine; correctness stories |
| **10_000 rows** | Resident OK; still virtualize paint |
| **1_000_000 logical** | Index window only; page fetch by range; zero per-row alloc in TermRock |
| **Very wide (64 cols)** | Column window + pin strips |
| **Rapid stream** | Append path; reuse buffers; don’t rebuild selection from scratch |
| **Narrow** | `contract_to_budget` before measure |

**Targets**

| Metric | Target |
|--------|--------|
| Paint cells | ≤ `MAX_PAINT_CELLS` (40×64) typical large viewport |
| `visible_range` on 1M | O(1) |
| Focus move on 10k projected | Bound to projected slice length |
| Select-all | Never O(logical_len) allocation |

Hot-path tests: assert no full logical scans (pattern of `*_hot_path.rs`).

---

## 7. Stories (lookbook / Studio)

| Story id | Proves |
|----------|--------|
| `data-table/rows-10` | Baseline chrome |
| `data-table/rows-10k` | Scroll focus bound |
| `data-table/rows-1m-virtual` | Only viewport projected |
| `data-table/wide-64` | H-scroll + pins |
| `data-table/cjk` | Width measure |
| `data-table/combining` | Grapheme safety |
| `data-table/stream-partial` | Rapid append / Partial load |
| `data-table/narrow-priority` | Column drop |
| `data-table/empty` · `loading` · `error` | LoadState |
| `data-table/multi-select` | Selection |
| `data-table/toolbar` | Actions |
| `virtual-grid/million` | 2D virtual |
| `logstream/follow` | Follow break |
| `diff/narrow-unified` | Responsive |
| `diagnostic/severity` | Colorless letters |
| `metrics/sparkline-live` | Stream points |
| `terminal-output/scrollback` | Virtual lines |
| `treetable/expand` | Hierarchy |
| `kv/copy` | KeyValue |
| `object-inspector/nested` | Nested expand |

---

## 8. Accessibility / colorless

- Selection: gutter `›` / `*` (not fill color alone).  
- Sort: `▲`/`▼` or ASCII `^`/`v`.  
- Severity: `E`/`W`/`I`/`H`.  
- Diff: `+`/`-`.  
- Focus-visible: `Role::BorderFocused` on panel.  
- Hit regions for headers, cells, actions.  
- Screen-reader path: consumer may map outcomes to a11y bus later; labels always textual.

---

## 9. Keyboard (DataTable defaults)

| Key | Behavior |
|-----|----------|
| ↑/↓ | Focus row in projected slice / scroll window |
| ←/→ | Focus column / h-scroll |
| PgUp/PgDn | Page by viewport |
| Home/End | First/last in slice or window edge |
| Space | Toggle multi-select on focused row |
| Enter | Activate row / commit edit |
| Esc | Cancel edit / clear cell range |
| Ctrl+A | `SelectAllRequested` (visible only) |
| s | Sort on focused/header column |
| / | Start filter edit (outcome) |
| c | Copy focused cell/row |
| e | Inline edit if column editable |
| x / Context | Context menu outcome |

Consumer remaps via keymap; outcomes stay stable.

---

## 10. Implementation plan

| Phase | Deliverable | Status |
|-------|-------------|--------|
| **D0** | Design + `data_view` kits + unit tests + bench consts | **Done** |
| **D1** | DataTable: virtual rows, sticky header, selection, load states | **In progress** (shell) |
| **D2** | Column resize / visibility / pin + responsive contract | Partial (model) |
| **D3** | Sort / filter chrome outcomes + empty/load/error stories | Partial |
| **D4** | Multi-select, copy range, expand detail | Partial (multi) |
| **D5** | Inline edit | Planned |
| **D6** | TreeTable surface | **Done** (migration 0190) |
| **D7** | LogStream / TerminalOutput virtualize | Partial (LogStream seed) |
| **D8** | ObjectInspector + Diagnostic | Partial (inspector seed) |
| **D9** | 1M + stream stories/benches; migrate `Table` → DataTable | Partial (grid million story) |
| **D10** | Migration file when `Table` public API renames | When breaking |

`Table` / `VirtualGrid` / `DetailTable` keep working until D9 fold.

---

## 11. API sketch

```rust
use termrock::widgets::data_view::{
    ColumnModel, DataColumn, DataColumnWidth, LoadState, VirtualWindow, bench,
};
use termrock::widgets::{DataTable, DataTableState, DataTableOutcome};

// Once: describe columns
let columns = ColumnModel::new(vec![
    DataColumn::new("id", "ID", DataColumnWidth::Fixed(8)).priority(100).pin(ColumnPin::Start),
    DataColumn::new("name", "Name", DataColumnWidth::Min(16)).priority(80),
    // …
]);

// Each frame:
state.window.logical_len = total_rows; // may be 1_000_000
state.window.viewport = body_h;
let (start, end) = state.window.visible_range();
let projected = db.project(start, end); // Vec of only (end-start) rows

DataTable::new(&tokens, &columns, &projected)
    .render(area, buf, &mut state);

match state.handle_key(key, &ids, &columns) {
    DataTableOutcome::SortRequested(col) => db.set_sort(col),
    DataTableOutcome::SelectAllRequested => select_visible_only(&projected),
    DataTableOutcome::Activate(id) => open(id),
    DataTableOutcome::Copy(payload) => clipboard.write(payload),
    _ => {}
}
```

---

## 12. Decision summary

1. **Kits not mega-traits** — compose `VirtualWindow`, `ColumnModel`, `SelectionModel`, …  
2. **Virtualization is mandatory** for large logical sets.  
3. **Consumer owns data + sort/filter execution**; TermRock owns chrome/geometry/outcomes.  
4. **Select-all never scans unloaded universe.**  
5. **Specialized surfaces** for logs, diffs, metrics, trees, inspectors, terminal output.  
6. **Same API** scales 10 → 10k → 1M logical with different projection strategies.  
7. **Colorless first** — glyphs and words, not hue alone.  
8. **Benchmark constants** live in `data_view::bench` for stories and CI.

---

## 13. Related

- `docs/design/terminal-design-system.md` — density, roles  
- `docs/design/responsive-layout.md` — contraction  
- `docs/design/termrock-agent.md` — DiffReview in agent workbench  
- `docs/design/streaming-performance.md` — append budgets  
- Plan 052 (DataTable / data_view)
