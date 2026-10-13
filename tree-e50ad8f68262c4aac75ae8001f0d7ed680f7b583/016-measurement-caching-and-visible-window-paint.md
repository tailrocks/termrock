# Plan 016: Stop re-measuring unchanged content every frame; paint only the visible window

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/widgets/detail_table.rs crates/termrock/src/widgets/form.rs crates/termrock/src/widgets/status_bar.rs crates/termrock/src/scroll/ crates/termrock/src/widgets/viewport.rs`
> Plans 008/011/013/015 legitimately touched these files. Read the live code
> for each excerpt before editing; STOP only on structural surprises (missing
> functions, different algorithms).

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (cache invalidation; measurement must never go stale)
- **Depends on**: plans/015-viewport-slice-before-clone.md (do the slice fix first; this builds on it)
- **Category**: perf
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

Even after Plan 015, every frame still pays O(total content) in *measurement*: `Viewport` calls `max_line_width(self.lines)` (unicode-width over every span of every line) to size the scrollbar; the dialog body does `lines.iter().map(line_width).max()`; `DetailTable` makes three full passes over all rows per frame (content width, height/selection scan, paint loop) and its paint path allocates several `String`s per visible row (`format!("{}{}", value, affordance)` plus a `display_cols_slice` String, plus per-segment `String`s in `paint_segment`); `Form` computes its `dimensions()` up to twice per frame; `StatusBar` rebuilds `placements()` (≈5 Vecs + 3 sorts) every render. On cursor-only repaints of large content, all of this is recomputed for identical input. `DetailTableState` even carries `content_width`/`content_height` fields that render recomputes instead of reusing.

## Current state

- `crates/termrock/src/widgets/viewport.rs:29` — `let content_width = max_line_width(self.lines);` every render (`max_line_width` walks every line: `scroll/render.rs` ~202).
- `crates/termrock/src/layout/dialog.rs:289` — `let content_width = lines.iter().map(line_width).max().unwrap_or(0);` per dialog-body frame. Load-bearing comment above it: dialog body deliberately uses `line_width`, NOT `max_line_width` (padding semantics) — preserve that distinction in whatever caching lands.
- `crates/termrock/src/widgets/detail_table.rs:289-294` — full-pass width:

```rust
        state.content_width = self
            .rows
            .iter()
            .map(|row| self.row_width(row, label_width))
            .max()
            .unwrap_or(0);
```

  then a second full pass for height/selection (`for row in self.rows { let row_height = self.row_height(row, value_width); ... }`), then the paint loop iterates ALL rows calling `row_height` even for off-screen rows. Per visible row (~line 430): `let value_and_affordance = format!("{}{}", row.value, affordance(row, copied));` + `crate::display_cols_slice(&value_and_affordance, chunk_start, chunk_width)` (returns `String`) + `paint_segment` calls each allocating another `String` (via `geometry.rs` `display_cols_slice` ~142-160).
- `crates/termrock/src/widgets/form.rs:318-325` — `dimensions(self.sections, area.width)` computed, and if a scrollbar is needed, computed AGAIN at reduced width:

```rust
        let (initial_columns, initial_height) = dimensions(self.sections, area.width);
        let show_scrollbar = initial_height > usize::from(area.height) && area.width > 1;
        ...
        let (columns, content_height) = if show_scrollbar {
            dimensions(self.sections, content_area.width)
        } else { (initial_columns, initial_height) };
```

  (The double-compute is an intrinsic two-pass — acceptable; the target here is the paint loop at ~352-428 iterating every section/field with four `visible_rect` computations per field regardless of visibility.)
- `crates/termrock/src/widgets/status_bar.rs:208+` — `for placement in self.placements(area)` each render; `placements()` (~106-195) allocates several Vecs and sorts; per slot `crate::display_cols_slice(slot.content, 0, width)` allocates a String.
- Exemplar for visible-window iteration: `widgets/tree.rs` render (~364-458) slices with `skip/take` and is allocation-free under `tests/tree_hot_path.rs`'s stats_alloc proof.
- Widget data is borrowed per frame (`&[DetailRow]`, `&[Line]`) — the library cannot hash-cache content cheaply without consumer cooperation. The states DO own scroll/dimensions fields across frames.
- Repo conventions: no interaction/rendering contract may regress for speed (`performance-baseline.md:15`).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Detail-table char tests | `cargo test -p termrock --test detail_table` | all pass (from Plan 007) |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0, zero diffs |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/widgets/{detail_table,form,status_bar,viewport}.rs`
- `crates/termrock/src/scroll/mod.rs` or a new small `measure` seam (Step 1's revision type)
- `crates/termrock/src/text/` (post-012 home of `display_cols_slice`) or `geometry.rs` (pre-012): add the non-allocating variant
- `migrations/000N-*.md` only if public state fields change shape

**Out of scope**:
- Consumer-side caching APIs beyond the minimal revision hook of Step 1.
- The dialog-body `line_width` vs `max_line_width` semantics.
- tabs/action_bar/hint_bar per-frame `format!` — small-N chrome, explicitly rejected as micro-optimization in the audit.

## Git workflow

- Directly on `main`; `git commit -s -m "perf(widgets): cache content measurement and clip paint loops to the viewport"`.

## Steps

### Step 1: A minimal content-revision seam

Add to the scroll/measure layer a tiny opt-in cache carried by the state types that already persist across frames:

```rust
/// Cached content measurement, invalidated by (len, revision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Measured {
    len: usize,
    revision: u64,
    pub width: usize,
    pub height: usize,
    valid: bool,
}
impl Measured {
    /// Returns cached dimensions when `len`+`revision` match the last
    /// computation; otherwise recomputes via `f` and stores.
    pub fn get_or_measure(&mut self, len: usize, revision: u64,
                          f: impl FnOnce() -> (usize, usize)) -> (usize, usize) { ... }
}
```

`revision` is consumer-supplied: widgets gain an optional builder field `content_revision(u64)` (post-013 idiom) defaulting to a sentinel meaning "no caching — measure every frame" (today's behavior, zero risk for consumers who ignore it). `len` changes always invalidate even under a stale revision, catching the most common mutation (append) automatically.

**Verify**: unit tests for `Measured` (hit, miss-on-len, miss-on-revision, sentinel-always-miss) → pass.

### Step 2: Apply to Viewport + DetailTable width/height

- `Viewport`: `DialogScroll` gains (or is wrapped with) a `Measured`; `max_line_width` runs only on miss.
- `DetailTable`: reuse the existing `DetailTableState.content_width/content_height` fields through `Measured`; the height/selection full pass also collapses on cache hit — the selected-row range must then come from a cheaper path: keep a per-frame scan ONLY over the visible window + the selected row's stored offset (store `selected_range` in state on miss; recompute on selection change — selection changes already run through state methods, set a dirty flag there).

**Verify**: `cargo test -p termrock --test detail_table` (Plan 007 suite) + `cargo test --workspace --all-features --locked` → all pass; previews zero-diff.

### Step 3: Clip the paint loops to the visible window

- `detail_table.rs` paint loop: before iterating, skip directly to the first visible visual row (`scroll_y`), iterate until the window is filled, exactly as `tree.rs` does with `skip/take` (wrap-mode continuation rows: entering mid-row is the tricky case — the loop must find the row containing visual line `scroll_y` and its continuation index; the existing `continuation`/`chunk_start` math at ~430 supports this).
- `form.rs` paint loop: skip fields whose `visible_rect` is empty *before* computing the other three rects — compute row bounds first, `continue` when the field's row range is entirely outside `[offset, offset+viewport)`.
- `status_bar.rs`: single row, small-N — leave `placements()` per frame, but stop re-sorting when slot slices are unchanged is NOT verifiable without hashing; skip (record as not-worth-it).

**Verify**: Plan 007 characterization tests (they pin hit regions including clipped/wrap cases) + full suite → pass; previews zero-diff.

### Step 4: Non-allocating segment painting

Add alongside `display_cols_slice(s, start, width) -> String` a writer variant `display_cols_slice_into(s, start, width, out: &mut String)` (clears and reuses `out`). Thread a scratch `String` through `detail_table`'s per-row paint (one scratch in the render fn, reused across rows) replacing the `format!` + slice allocations where the borrow checker allows; same for `status_bar`'s slot content. Keep the allocating variant for external callers.

**Verify**: full suite green. Optional but preferred: extend Plan 015's `viewport_hot_path.rs` approach with a `detail_table_hot_path.rs` (10k rows, 40-row viewport, warmed, assert bounded allocations/render + the existing behavior assertions) — verified to fail pre-fix.

## Test plan

- `Measured` unit tests (4 cases).
- `detail_table_hot_path.rs` allocation-budget test modeled on `tree_hot_path.rs`, failing against pre-fix code (state the observed pre-fix number in your report).
- Plan 007 characterization suites + 51 scroll tests + previews as the behavior net.

## Done criteria

- [ ] Cursor-only repaint of an unchanged 10k-row DetailTable performs no full-content width/height pass (hot-path test proves allocation/time bound)
- [ ] Paint loops in detail_table/form skip off-window items (code inspection + wrap-window characterization tests pass)
- [ ] `display_cols_slice_into` exists; detail_table/status_bar use scratch buffers
- [ ] `cargo test --workspace --all-features --locked` → all pass; previews zero-diff
- [ ] `plans/README.md` status row updated (including the status_bar sort note as considered/rejected)

## STOP conditions

- The wrap-mode mid-row window entry in detail_table cannot reuse the existing continuation math without behavior change — report with the failing characterization test name; do not weaken the test.
- The `Measured` seam forces a public State field change that breaks serde derives from Plan 013 — coordinate: report which fields.
- Borrowed-lifetime issues make the scratch-buffer threading require `unsafe` or self-referential contortions — STOP (`unsafe` is forbidden); the allocating path stays for that call site, note it.

## Maintenance notes

- The `content_revision` contract must be documented loudly on the builder method: "bump when row content changes; length changes are auto-detected." A consumer who forgets gets a stale scrollbar, not corruption — say so in the doc.
- If a future virtualized/windowed data API lands (rows fetched on demand), `Measured` becomes obsolete — it's deliberately tiny to be deletable.
- Follow-up F-A from Plan 015 (Theme::default() in render_scrollable_block_at) still open.
