# High-frequency, streaming, and large-data performance

**Status:** design SoT + foundation (`termrock::perf`)  
**Principle:** Optimize **perceived** jank. Do not micro-opt cold paths.  
**Related:** `data_view` virtualization, Transcript, LogPane, OverlayStack, FrameTick, hot_path tests.

---

## 1. Target workloads

| Workload | Pressure |
|----------|----------|
| AI token stream | High-frequency small appends, wrap/remeasure |
| Tool / process / logs | Bursty lines, follow-tail |
| Large tables / trees / diffs | Huge logical N, small viewport |
| Multi-agent + background tasks | Concurrent updates, overlays |
| Rapid resize | Full reflow storms |

---

## 2. Investigation findings (current TermRock)

| Area | State | Risk if ignored |
|------|--------|-----------------|
| **Rendering** | Hot paths exist (tree/table/log/…) with viewport bounds | Full-dataset paint will thrash |
| **Layout** | Workspace/responsive recompute on area change | Resize storms without dirty coalescing |
| **Wrapping / graphemes** | `display_cols` / edit_core grapheme-safe | Re-wrap entire transcript every token = jank |
| **Allocation** | tree_hot_path asserts **0 alloc** warmed | Silent clones on stream paths |
| **Cloning** | Some chrome still clones Theme/tokens per render | Steady paint cost |
| **Buffer writes** | Full widget `Buffer` region each frame | OK if viewport-sized; bad if full scrollback |
| **Visible window** | `VirtualWindow`, VirtualGrid, Transcript measure cache | Must be the only path for 1M rows |
| **Scroll / follow** | Log/transcript follow exist; kits in `perf::follow` unify | Lost place + spam “jump to end” |
| **Overlays** | OverlayStack reflow | Open/close should not rebuild world |
| **Semantic scene** | Immediate-mode register each frame | Cap elements; don’t re-alloc actions carelessly |
| **Animation** | `Motion` + FrameTick | Idle spin without dirty = battery jank |
| **Events** | Crossterm poll loop consumer-owned | Need coalesce before paint |
| **Cross-thread** | Not in crate | Channel + UI-thread batch apply |

---

## 3. Strategies (user-visible)

### 3.1 Stable scroll anchors

`ScrollAnchor` kinds: `Index` · `ContentId` · `FromEnd`.

- After resize/reproject, resolve anchor → `VirtualWindow::reveal`.  
- Streaming inserts **above** anchor: prefer ContentId.  
- Follow uses FromEnd(0).

### 3.2 Paused follow-tail + new content

```
Following + append → stick to end, clear indicator
Paused + append → keep offset, NewContentIndicator.unseen += n
User wheel/drag → FollowMode::Paused
Jump/resume → Following + clear indicator
```

API: `FollowMode`, `NewContentIndicator`, `apply_follow_after_append`.

### 3.3 Batched streaming updates

`StreamCoalescer` on UI thread:

- `push_text` / `push_event` / `push_terminal` from channel drain  
- `take_for_frame(FrameTick)` respects min flush period (default 8 ms)  
- High/Critical priority flushes immediately  
- `BackpressureSignal::{Open,Soft,Hard}` for producers  

**Consumer pattern**

```
worker → channel(delta)
ui: while try_recv { coalescer.push_* }
batch = coalescer.take_for_frame(tick)
apply batch to model once → one paint
```

### 3.4 Dirty-region awareness (practical, optional)

`DirtyFlags { chrome, body, chrome_secondary, overlays }`.

- Always correct to full-redraw.  
- Prefer dirty only when host supports partial terminal damage (future).  
- Today: use flags to **skip work** (e.g. don’t remeasure headers if only body).

### 3.5 Virtualized rows/cells

Mandatory for large data (`VirtualWindow`, VirtualGrid, DataTable plan).

- Paint ≤ viewport ±1.  
- CI: `MaxRowsTouched` budgets.

### 3.6 Incremental text layout

- Transcript: height cache keyed by `(id, revision, width)`.  
- Token stream: append to **last block** only; invalidate that block’s cache.  
- Do not re-wrap entire history on each token.

### 3.7 Cancellation

- UI emits `Cancel` outcomes; worker checks token between chunks.  
- Coalescer `push_terminal` forces flush so cancel UI is prompt.

### 3.8 Backpressure

| Signal | Producer |
|--------|----------|
| Open | Full rate |
| Soft | Drop Low; coalesce Normal |
| Hard | Drop Normal; keep High/Critical |

Never drop permission/tool boundaries.

### 3.9 Deterministic frame clocks

- `FrameTick` only for time-dependent UI.  
- Tests use `FrameTick::manual`.  
- No `Instant::now()` inside widgets.

### 3.10 Reduced redraw when idle

| Motion | Cadence |
|--------|---------|
| Full | ≤ ~30 Hz idle animation (`idle_motion_cadence` budget 33 ms) |
| Reduced | ÷4 or slower |
| Off | **no** idle redraw; paint only on input/model dirty |

Host loop: `if !dirty && !motion_needs_frame { park on event }`.

---

## 4. APIs (`termrock::perf`)

| API | Role |
|-----|------|
| `ComponentBudget` / `budgets()` / `budget_for` | Named CI budgets |
| `check_batch_budget` / `check_zero_alloc_steady` | Hot path asserts |
| `FollowMode` / `ScrollAnchor` / `NewContentIndicator` | Follow UX |
| `apply_follow_after_append` | One-call follow update |
| `StreamCoalescer` / `StreamBatch` | Token/log batching |
| `DirtyFlags` / `UpdatePriority` / `BackpressureSignal` | Pipeline control |

Widgets remain free of threads; hosts wire channels.

---

## 5. Performance budgets (CI)

| Id | Component | Limit (debug) |
|----|-----------|----------------|
| `tree_viewport_10k` | Tree | 100 paints / 250 ms + 0 alloc |
| `table_viewport_10k` | Table | 100 / 250 ms |
| `log_append_follow` | LogPane | 100 / 200 ms |
| `transcript_10k_blocks` | Transcript | 50 / 300 ms |
| `virtual_grid_million_window` | VirtualGrid | ≤48 rows touched |
| `datatable_million_window` | DataTable | ≤48 rows touched |
| `scene_register_workbench` | Scene | ≤256 elements soft |
| `overlay_open_close` | OverlayStack | 200 / 100 ms |
| `workbench_composite_frame` | Workbench | 30 / 300 ms (~10 fps floor) |
| `idle_motion_cadence` | Motion | ≥33 ms between idle frames |
| `stream_coalesce_batch` | Coalescer | 1000 push / 50 ms |

**Policy:** raising a budget requires new measurement note + intentional PR (same as COMPONENTS.md tree budget).

**Enforcement:**

1. Integration tests (`*_hot_path.rs`) call `check_batch_budget` / zero-alloc.  
2. `cargo test -p termrock --test tree_hot_path` already in compatibility gate.  
3. Expand gate as each budget gains a test.  
4. Optional: release-profile benches later (TODO.md); **debug budgets stay the CI floor**.

---

## 6. Cross-thread update model

```
[agent workers] ──mpsc/delta──► [UI coalescer] ──batch──► [model]
                                      ▲
                               FrameTick / event
                                      │
                                   [paint]
```

- Never paint from worker threads.  
- Prefer bounded channels; on full → Soft/Hard backpressure.  
- Cancellation: shared `AtomicBool` / generation token.

---

## 7. Resize strategy

1. Coalesce resize events to one per frame.  
2. Mark dirty chrome+body.  
3. Recompute layout once; reflow overlays (`OverlayStack::reflow`).  
4. Re-resolve `ScrollAnchor` (ContentId preferred).  
5. Invalidate width-keyed measure caches only.

---

## 8. Multi-agent / background tasks

- One coalescer **per stream** (agent id) or one global with tagged batches.  
- Task rail: update by id; don’t rebuild full list alloc if using stable slots.  
- Overlays for permission: High priority flush so prompts aren’t delayed behind tokens.

---

## 9. What not to optimize (yet)

- Cold start of one-shot dialogs.  
- Lookbook catalog generation frequency.  
- Perfect dirty-terminal CSI damage maps (nice later).  
- Sub-microsecond grapheme iterators without a failing budget.

---

## 10. Implementation plan

| Phase | Work |
|-------|------|
| **P0** ✅ | Design + `perf` kits + budget table + tree_hot_path wired |
| **P1** | Wire table/log/transcript hot_path to named budgets |
| **P2** | Transcript append uses coalescer + last-block cache invalidation only |
| **P3** | LogStream follow + NewContentIndicator UI |
| **P4** | Workbench frame budget test |
| **P5** | Scene element soft telemetry in Studio |
| **P6** | Release benches optional; keep debug CI |

---

## 11. Decision summary

1. **Viewport-only paint** for large data.  
2. **Batch streams** to frame cadence; backpressure producers.  
3. **Follow is a mode**, not an accident of offset.  
4. **Anchors** survive resize/reproject.  
5. **Budgets are named and CI-owned**; regressions fail tests.  
6. **Motion off ⇒ no idle redraw.**  
7. **UI thread owns paint**; workers only queue deltas.
