# High-frequency, streaming, and large-data performance

| Field | Value |
|-------|-------|
| **Status** | Design SoT + foundation (`termrock::perf`) |
| **Principle** | Optimize **perceived** jank. Do not micro-opt cold / trivial paths. |
| **Code** | `crates/termrock/src/perf/{budget,follow,stream}.rs` |
| **Related** | `data-presentation.md`, Transcript, LogPane, OverlayStack, FrameTick, `*_hot_path` tests |

---

## 1. Target workloads

| Workload | Pressure | Primary strategy |
|----------|----------|------------------|
| AI responses token-by-token | High-frequency small appends, wrap/remeasure | StreamCoalescer + last-block height cache |
| Tool output | Bursty lines | Coalesce + follow mode |
| Logs | Sustained append, follow-tail | Virtual window + pause/indicator |
| Process / terminal output | High volume, ANSI-ish lines | Same as logs; never paint full scrollback |
| Large tables | Huge logical N, small viewport | VirtualWindow + project(start..end) |
| Large trees | Deep flatten, expand | Viewport-only paint; zero-alloc steady |
| Diffs | Multi-hunk, streaming patch | Hunk window; incremental append |
| Background tasks | Concurrent id updates | Stable slots; tagged batches |
| Multiple concurrent agents | Multi-stream | Per-stream coalescer or tagged global |
| Rapid terminal resize | Full reflow storms | Coalesce resize → one layout/frame |

---

## 2. Investigation map (current TermRock)

| Area | Current state | Risk if ignored | Direction |
|------|---------------|-----------------|-----------|
| **Rendering complexity** | Hot paths bound viewport (tree/table/log) | Full-history paint thrash | Enforce MaxRowsTouched / O(viewport) |
| **Layout recalculation** | Workspace/responsive on area change | Resize storms | Coalesce Resize; dirty chrome+body once |
| **Text wrapping** | `display_cols` / grapheme-safe edit_core | Re-wrap full transcript per token | Incremental last-block invalidate only |
| **Grapheme processing** | UnicodeSegmentation on edit paths | Hot token path re-scanning all history | Cache measure by `(id, rev, width)` |
| **Allocation** | tree_hot_path **0 alloc** warmed | Silent clones on stream | CI zero-alloc budgets |
| **Cloning** | Some Theme/token clones remain | Steady paint cost | Prefer borrows; flag in Studio later |
| **Buffer writes** | Full widget region each frame | OK if viewport-sized | Skip work when DirtyFlags clean |
| **Visible-window** | VirtualWindow, VirtualGrid, DataTable | 1M row alloc | Only path for large N |
| **Scroll anchoring** | `ScrollAnchor` kit | Lost place on resize | ContentId preferred |
| **Follow-tail** | LogPane + `FollowMode` kit | Spam jump / lost context | Paused + NewContentIndicator |
| **Overlay cost** | OverlayStack reflow | Rebuild world on open | Isolate overlay paint; budget open/close |
| **Semantic scene** | Immediate-mode register/frame | Unbounded element growth | Soft cap 256 workbench |
| **Animation cadence** | Motion + FrameTick | Idle battery spin | Off ⇒ no idle redraw |
| **Event coalescing** | Host-owned poll | Token flood | StreamCoalescer on UI thread |
| **Cross-thread** | Not in widgets | Paint off UI thread | Channel + UI-only apply |

---

## 3. Strategies (user-visible)

### 3.1 Stable scroll anchors

```text
ScrollAnchorKind: Index | ContentId | FromEnd
ScrollAnchor { kind, index, content_id?, row_bias }
```

- After resize/reproject: resolve anchor → `VirtualWindow::reveal`.  
- Streaming inserts **above** anchor: prefer `ContentId`.  
- Follow uses `FromEnd(0)`.

### 3.2 Paused follow-tail + “new content”

```
Following + append → stick to end, clear indicator
Paused + append    → keep offset, NewContentIndicator.unseen += n
User wheel/drag    → FollowMode::Paused
Jump / resume      → Following + clear indicator
```

APIs: `FollowMode`, `NewContentIndicator`, `apply_follow_after_append`, `pause_follow_on_user_scroll`.

### 3.3 Batched streaming updates

`StreamCoalescer` (UI thread only):

| Method | Role |
|--------|------|
| `push_text` / `push_event` / `push_terminal` | From channel drain |
| `take_for_frame(FrameTick)` | Min flush period (default 8 ms) |
| High/Critical | Immediate flush |
| `backpressure()` | Open / Soft / Hard |

**Host loop**

```
worker → channel(delta)
ui: while try_recv { coalescer.push_* }
batch = coalescer.take_for_frame(tick)
apply batch once → one paint
if !dirty && !motion_needs_frame { park on event }
```

### 3.4 Dirty-region awareness (practical)

`DirtyFlags { chrome, body, chrome_secondary, overlays }`

- Full redraw always correct.  
- Use flags to **skip work** (don’t remeasure sticky headers if only body dirty).  
- Terminal CSI damage maps = later (not required for v1 budgets).

### 3.5 Virtualized rows / cells

Mandatory for large data:

- `VirtualWindow`, VirtualGrid, DataTable projection.  
- Paint ≤ viewport ±1.  
- CI: `MaxRowsTouched` / viewport region count.

### 3.6 Incremental text layout

- Transcript height cache: `(id, revision, width)`.  
- Token stream: append **last block** only; invalidate that entry.  
- Never re-wrap entire history on each token.

### 3.7 Cancellation

- UI emits cancel outcomes; worker checks token between chunks.  
- `push_terminal` / Critical forces flush so stop chrome is prompt.

### 3.8 Backpressure

| Signal | Producer behavior |
|--------|-------------------|
| Open | Full rate |
| Soft | Drop Low; coalesce Normal |
| Hard | Drop Normal; keep High/Critical |

Never drop permission / tool boundary events.

### 3.9 Deterministic frame clocks

- Time-dependent UI uses `FrameTick` only.  
- Tests: `FrameTick::manual`.  
- **No `Instant::now()` inside widgets.**

### 3.10 Reduced redraw when idle

| Motion | Cadence |
|--------|---------|
| Full | ≤ ~30 Hz idle (`idle_motion_cadence` ≥ 33 ms) |
| Reduced | ÷4 or slower |
| Off | **no** idle redraw; paint only on input/model dirty |

---

## 4. APIs (`termrock::perf`)

| API | Role |
|-----|------|
| `ComponentBudget` / `budgets()` / `budget_for` | Named CI budgets |
| `check_batch_budget` | Wall-time batch assert |
| `check_zero_alloc_steady` | Zero alloc assert |
| `check_max_rows_touched` | Virtualization / alloc-per-render cap |
| `FollowMode` / `ScrollAnchor` / `NewContentIndicator` | Follow UX |
| `apply_follow_after_append` | One-call follow update |
| `StreamCoalescer` / `StreamBatch` | Token/log batching |
| `DirtyFlags` / `UpdatePriority` / `BackpressureSignal` | Pipeline control |

Widgets remain free of threads; hosts wire channels.

---

## 5. Performance budgets (CI-fail on regression)

| Id | Component | Limit (debug) | Test |
|----|-----------|---------------|------|
| `tree_viewport_10k` | Tree | 100 paints / 250 ms | `tree_hot_path` |
| `tree_viewport_10k_alloc` | Tree | 0 alloc steady | `tree_hot_path` |
| `table_viewport_10k` | Table | 100 / 250 ms | `table_hot_path` |
| `table_viewport_10k_alloc` | Table | 0 alloc steady | `table_hot_path` |
| `log_append_follow` | LogPane | 100 / 300 ms | `log_pane_hot_path` |
| `log_append_follow_alloc` | LogPane | ≤64 allocs/render avg | `log_pane_hot_path` |
| `transcript_10k_blocks` | Transcript | 50 / 300 ms | (wire next) |
| `virtual_grid_million_window` | VirtualGrid | ≤48 rows touched | lookbook + unit |
| `datatable_million_window` | DataTable | ≤48 rows touched | data_table / data_view tests |
| `scene_register_workbench` | Scene | ≤256 elements soft | Studio later |
| `overlay_open_close` | OverlayStack | 200 / 100 ms | (wire next) |
| `workbench_composite_frame` | AgentWorkbench | 30 / 300 ms (~10 fps floor) | (wire next) |
| `idle_motion_cadence` | Motion | ≥33 ms idle period | policy |
| `stream_coalesce_batch` | StreamCoalescer | 1000 push / 50 ms | unit in `budget` tests |

**Policy:** raising a budget requires intentional PR + measurement note. No silent loosen.

**Enforcement:**

```bash
cargo test -p termrock --test tree_hot_path --locked
cargo test -p termrock --test table_hot_path --locked
cargo test -p termrock --test log_pane_hot_path --locked
cargo test -p termrock --lib perf --locked
# compatibility / gate should keep expanding as budgets gain tests
```

Debug budgets are the **CI floor**. Release benches optional later.

---

## 6. Cross-thread update model

```
[agent / tool / log workers] ──mpsc deltas──► [UI StreamCoalescer]
                                                    │
                                              FrameTick / event
                                                    │
                                              apply batch → model
                                                    │
                                                 [paint]
```

- Never paint from worker threads.  
- Bounded channels; on full → Soft/Hard backpressure.  
- Cancellation: generation / `AtomicBool` shared with workers.

---

## 7. Resize strategy

1. Coalesce resize events → one per frame.  
2. Mark dirty chrome+body.  
3. Layout once; `OverlayStack::reflow`.  
4. Re-resolve `ScrollAnchor` (ContentId preferred).  
5. Invalidate **width-keyed** measure caches only.

---

## 8. Multi-agent / background tasks

- One coalescer **per stream** (agent id) or one global with tagged batches.  
- Task rail: update by id; stable slots avoid full list rebuild.  
- Permission overlays: **High** priority flush — never starve behind tokens.

---

## 9. What not to optimize (yet)

- Cold start of one-shot dialogs.  
- Lookbook catalog generation frequency.  
- Perfect CSI dirty-terminal damage maps.  
- Sub-microsecond grapheme iterators without a failing budget.  
- Premature micro-opts on paths users never feel.

---

## 10. Implementation plan

| Phase | Work | Status |
|-------|------|--------|
| **P0** | Design + `perf` kits + budget table + tree_hot_path | **Done** |
| **P1** | Wire table/log hot_path to named budgets + max_rows helper | **Done** (this pass) |
| **P2** | Transcript append: coalescer + last-block cache only | Next |
| **P3** | LogStream UI: follow + NewContentIndicator chrome | Next |
| **P4** | Workbench composite frame budget test | Next |
| **P5** | Scene element soft telemetry in Studio | Later |
| **P6** | Release-profile benches optional | Later |

---

## 11. Decision summary

1. **Viewport-only paint** for large data.  
2. **Batch streams** to frame cadence; backpressure producers.  
3. **Follow is a mode**, not an accident of offset.  
4. **Anchors** survive resize/reproject.  
5. **Named budgets are CI-owned**; regressions fail tests.  
6. **Motion off ⇒ no idle redraw.**  
7. **UI thread owns paint**; workers only queue deltas.  
8. **Perceived jank first** — skip trivial cold-path micro-opts.

---

## 12. References

- `crates/termrock/src/perf/`  
- `crates/termrock/tests/{tree,table,log_pane}_hot_path.rs`  
- `docs/design/data-presentation.md`  
- `performance-baseline.md` (historical + pointer to `budgets()`)  
- `docs/design/component-quality-standard.md` (large_data / streaming axes)
