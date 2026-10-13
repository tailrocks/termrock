# Plan 015: Scrollable bodies clone only the visible window, never the whole content

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/widgets/viewport.rs crates/termrock/src/layout/dialog.rs crates/termrock/src/scroll/render.rs`
> On mismatch with "Current state" excerpts (beyond Plan 008's theme-field
> threading in viewport.rs), STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (scroll/wrap interaction; the 51-test scroll suite is the net)
- **Depends on**: none (coordinate with Plan 008 if concurrent — both touch viewport.rs)
- **Category**: perf
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The scrollable line-body renderers deep-clone the entire content on every frame even though only the ~viewport rows are visible. A 5,000-line log in a `Viewport` allocates ~5,000 `Line` clones (each with its span `Vec`) per keystroke repaint, per frame. The whole point of a scrollable body is that content is large; per-frame cost must scale with the viewport, not the content. The fix pattern already exists in this repo (`render_lines_with_offset_in_area` slices before collecting) — the shipped `Viewport` widget and the dialog-body path just don't use it.

## Current state

- `crates/termrock/src/widgets/viewport.rs:45` — the full clone (inside `StatefulWidget for &Viewport`):

```rust
        Paragraph::new(self.lines.to_vec())
            .block(block)
            .style(self.content_style)
            .scroll((state.scroll_y, state.scroll_x))
            .render(area, buffer);
```

  `state.clamp(...)` above it already computes `viewport_height`/`content_width`; `state` is a `DialogScroll` (from `crate::scroll`).
- `crates/termrock/src/layout/dialog.rs:298` — same pattern in `render_scrollable_dialog_body`:

```rust
    Paragraph::new(lines.to_vec())
        .scroll((eff_y, eff_x))
        .render(content_area, frame.buffer_mut());
    scroll.render_scrollbars(frame, block_area, content_height, content_width);
```

  (Above it: `eff_x`/`eff_y` from `effective_offset(...)`; the function returns `(content_width, content_height)`. Note the load-bearing comment about `line_width` vs `max_line_width` — do not change the width computation here; Plan 016 owns measurement.)
- `crates/termrock/src/scroll/render.rs:376` — `render_selected_lines_in_area` materializes ALL lines: `let items = lines.into_iter().map(ListItem::new).collect();` then relies on `ScrollableList ... .offset(offset)` to skip.
- `crates/termrock/src/scroll/render.rs:685` (`render_scrollable_block_at`) — passes the whole padded vec: `Paragraph::new(add_trailing_padding(lines)) ... .scroll((eff_y, eff_x))`.
- **The in-repo exemplar** — `scroll/render.rs:554-558` (`render_lines_with_offset_in_area`):

```rust
    let visible: Text<'_> = lines
        .into_iter()
        .skip(usize::from(clamped))
        .take(viewport)
        .collect();
    frame.render_widget(Paragraph::new(visible), area);
```

- Key subtlety: pre-slicing vertically means `Paragraph::scroll` must receive `(0, eff_x)` — horizontal scroll stays on Paragraph, vertical is the slice. Trailing-padding (`add_trailing_padding`) exists to keep horizontal scroll bounded on the *mounts panel* path — when slicing, padding must apply to the visible slice only.
- Also present at `render.rs:685` region (record, don't fix here): `let theme = crate::Theme::default();` hardcoded inside the render helper — a theming bypass outside Plan 008's widget scope; tracked in plans/README.md as follow-up F-A.
- Safety net: `scroll` has 51 tests (`scroll/tests.rs` + `scroll/render/tests.rs`) covering clamping, thumbs, overflow; `widgets/tests.rs` asserts viewport thumb glyphs and content cells.
- Perf contract context: `performance-baseline.md` + `COMPONENTS.md` document the tree hot path as allocation-free with a stats_alloc test (`tests/tree_hot_path.rs`) — use the same technique to prove this fix.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Scroll tests | `cargo test -p termrock scroll` | all pass |
| Widget tests | `cargo test -p termrock --lib` | all pass |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0, zero diffs |

## Scope

**In scope**:
- `crates/termrock/src/widgets/viewport.rs`
- `crates/termrock/src/layout/dialog.rs` (`render_scrollable_dialog_body` only)
- `crates/termrock/src/scroll/render.rs` (`render_selected_lines_in_area`, `render_scrollable_block_at`)
- New allocation-budget test file

**Out of scope**:
- Width measurement/caching (`max_line_width` per frame) — Plan 016.
- Signature changes to public helpers — slice internally; callers unchanged.
- The `Theme::default()` hardcode at render.rs:685 (recorded follow-up).

## Git workflow

- Directly on `main`; `git commit -s -m "perf(scroll): render the visible slice instead of cloning full content"`.

## Steps

### Step 1: `Viewport`

Replace the full `to_vec()` with the exemplar pattern: slice `self.lines[usize::from(state.scroll_y).min(len)..]`, `take(viewport_height)`, clone only those `Line`s into the `Paragraph`, pass `.scroll((0, state.scroll_x))`. The scrollbar math (`is_scrollable(self.lines.len(), viewport_height)` etc.) keeps using TOTAL content length — only the paragraph body is sliced.

**Verify**: `cargo test -p termrock --lib` → viewport tests pass (they assert visible cells + thumb positions — sliced rendering must produce identical buffers).

### Step 2: `render_scrollable_dialog_body`

Same transformation on `dialog.rs:298`: slice by `eff_y`, take `vp_h`, `.scroll((0, eff_x))`. The returned `(content_width, content_height)` and scrollbar rendering keep total dimensions.

**Verify**: `cargo test -p termrock layout` and `cargo test -p termrock --lib` → dialog tests pass.

### Step 3: `render_selected_lines_in_area` and `render_scrollable_block_at`

- `render_selected_lines_in_area` (render.rs:376): compute `offset` first (it already does), then map ONLY `lines.into_iter().skip(offset).take(viewport)` to `ListItem`s, and pass `.offset(0)` to `ScrollableList` (offset already applied). Check `ScrollableList`'s highlight logic: `selected` index must be rebased (`selected - offset`) — read the widget's API (tui-scrollbar / ratatui-widgets `List`) and rebase accordingly.
- `render_scrollable_block_at` (render.rs:685): slice by `eff_y`/`take(vp_h)` before `add_trailing_padding`, `.scroll((0, eff_x))`.

**Verify**: `cargo test -p termrock scroll` → all 51 pass unchanged.

### Step 4: Prove it with an allocation budget test

Add `crates/termrock/tests/viewport_hot_path.rs` modeled directly on `crates/termrock/tests/tree_hot_path.rs` (read it first; it uses `stats_alloc` as a dev-dependency, warms up, then asserts zero allocations across N renders): build a `Viewport` over 10,000 lines, render a 40-row area 100 times after warm-up, assert allocation count is bounded by O(viewport) per render — a fixed generous budget (e.g. < 200 allocations/render) rather than zero, since `Line` clones of the visible slice still allocate. Assert also a wall-clock budget in the spirit of tree_hot_path only if stable; allocation count is the primary assertion.

**Verify**: `cargo test -p termrock --test viewport_hot_path` → passes; temporarily revert Step 1 locally to confirm the test FAILS against the old code (then re-apply) — this proves the test bites.

## Test plan

- Existing 51 scroll tests + widget render tests (buffer-identical output requirement).
- New `viewport_hot_path.rs` allocation-budget test (Step 4), verified to fail against pre-fix code.
- Preview SVGs must not change at all.

## Done criteria

- [x] `grep -n "to_vec()" crates/termrock/src/widgets/viewport.rs crates/termrock/src/layout/dialog.rs` → no full-content clones remain in render paths
- [x] `cargo test --workspace --all-features --locked` → all pass, including the new hot-path test
- [x] Preview check → zero diffs
- [x] The hot-path test demonstrably fails against `d960b6b^`: 1,000,200 allocations / 88,051,200 bytes across 100 renders versus the <20,000-allocation budget
- [x] `plans/README.md` status row updated

## STOP conditions

- Sliced rendering changes any buffer assertion in existing tests — a subtle interaction (wrap, padding, highlight rebase) differs; investigate up to two attempts, then report the exact test + cell diff.
- `ScrollableList` cannot express a rebased selection without visual difference (highlight spacing) — report; that path may need the offset left as-is (partial fix is acceptable, note it).
- stats_alloc interferes with test parallelism (it's a global allocator) — mirror whatever isolation `tree_hot_path.rs` uses (separate integration-test binary = separate process, which is why the new test gets its own file).

## Maintenance notes

- Rule for future scrollable renderers: slice vertically, `Paragraph::scroll` horizontally only. The exemplar comment should live on `render_lines_with_offset_in_area`.
- Plan 016 (width caching) compounds this win: after both, per-frame cost is O(viewport) fully.
- Follow-up F-A (plans/README.md): `render_scrollable_block_at` hardcodes `Theme::default()` — needs a theme parameter (breaking) when someone next touches this signature.
