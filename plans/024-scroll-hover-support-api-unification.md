# Plan 024: One scroll type, one hover primitive, no colliding helper names — retire the orphaned render family

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/scroll/ crates/termrock/src/layout/ crates/termrock/src/interaction/`
> Plans 012/015/016/017 legitimately touch these files — check their status
> rows and read live code per excerpt. Unexplained structural mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED (breaking public removals/renames; forward-only policy sanctions them; migration file required)
- **Depends on**: plans/012-module-structure-and-surface-trim.md (same surface-trim philosophy; do 012's moves first so paths are final), plans/015 + 016 (they edit the same render functions — land perf first so this plan deletes/renames the post-perf shapes)
- **Category**: tech-debt
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

The support layer ships duplicate machinery at every level. Two two-axis dialog-scroll types exist in different modules — `scroll::DialogScroll` (live: `Viewport`, `DetailTable` use it) and `layout::DialogBodyScroll` (zero consumers, fields in a different order, and it's the copy carrying the truncating-cast bug its twin fixed). Two `max_line_width` functions return *different numbers* (raw vs padded width), disambiguated only by a `rendered_` re-export alias — an in-code comment in `layout/dialog.rs` warns picking the wrong one over-scrolls the body. Two `max_offset`, two `cursor_follow_offset`, duplicated `is_scrollable`/`effective_offset` wrappers. Two hover abstractions (`interaction::HoverState` + `HitRegion` vs `interaction::hover_tracker::HoverTracker` + `ClickableRect`) — BOTH with zero consumers — and `HoverTracker` recomputes the hovered element per query (O(n²)/frame). And the whole `render_*`/`ScrollableList` free-function family lost its only callers when the v0.7 "components facade" was removed (`git log`: `135e108 refactor(api)!: remove component facade`); `ScrollableList` duplicates `widgets::List` and hardcodes `PHOSPHOR_GREEN` highlight. One concept, one name, one implementation.

## Current state

- `scroll/mod.rs:131-135` (live twin):

```rust
pub struct DialogScroll { pub scroll_x: u16, pub scroll_y: u16 }
```

  Consumers: `widgets/viewport.rs:24` (`type State = DialogScroll`), `widgets/detail_table.rs:73`. Its Down-clamp uses saturating `max_offset_u16` (~scroll/mod.rs:185).
- `layout/dialog.rs:48-52` (dead twin): `pub struct DialogBodyScroll { pub scroll_y: u16, pub scroll_x: u16 }` with `handle_key`, `handle_key_for_axes`, `on_mouse_scroll{,_for_axes,_for_size}`, `handle_raw_key_for_axes`, `on_sgr_wheel_button_for_axes`, `render_scrollbars`. Only consumer: `layout/mod.rs:6` re-export + its own tests. Its Down-clamp uses the truncating `as u16` (~:99) — the exact bug Plan 017 Step 4 targets (COORDINATE: if this plan deletes the type first, 017's dialog fix is moot; if 017 ran first, the fix gets deleted here — both fine, note in README).
- The collision alias block (`scroll/mod.rs:16-23`):

```rust
pub use render::{
    SCROLLBAR_HORIZONTAL_THUMB, SCROLLBAR_TRACK, ScrollableList, ScrollbarStyle,
    apply_scroll_delta, apply_scroll_delta_unclamped, apply_term_width_scroll_delta,
    clamp_scroll_offset, cursor_follow_offset as rendered_cursor_follow_offset, effective_offset,
    horizontal_scrollbar_area, line_width, max_line_width as rendered_max_line_width,
    max_offset as rendered_max_offset, render_horizontal_scrollbar,
    render_line_with_fixed_prefix_scroll, render_lines_with_offset_in_area,
    render_scrollable_block, render_scrollable_block_at, render_selected_lines_in_area,
    render_vertical_scrollbar, render_vertical_scrollbar_in_area,
```

  Divergent pairs: `scroll/mod.rs:75` `max_line_width` (raw `Line::width`) vs `scroll/render.rs:202` `max_line_width` (padded via `padded_line_display_cols`); `mod.rs:85` `max_offset(usize)` vs `render.rs:69` `max_offset(u16)`; `mod.rs:519` vs `render.rs:85` `cursor_follow_offset`.
- Hover duo:
  - `interaction/mod.rs:34-58`: `HitRegion<Id> { id, area }` + `HoverState<Id>` (caches on `update(position, regions)`). `HitRegion` is heavily used by widgets; `HoverState` itself: zero consumers.
  - `interaction/hover_tracker.rs`: `HoverTracker<K>` + `pub(crate) ClickableRect` — `hovered()` linear scan per query; `is_hovered`/`pick_style`/`any_hovered` each call it again. Zero consumers outside tests.
- Orphaned render family (zero non-test consumers, verified by grep): `render_scrollable_block`, `render_scrollable_block_at`, `render_lines_with_offset_in_area`, `render_selected_lines_in_area`, `ScrollableList` (scroll/render.rs), `render_scrollable_dialog_body`, `dialog_inner_chunks`, `render_dialog_shell`, `dialog_scroll_axes`, `dialog_inner_height` (layout/dialog.rs). Widgets consume the primitives directly (`render_vertical_scrollbar_to_buffer`, `full_cell_thumb`, `is_scrollable` — these ARE live, keep them). `ScrollableList::new` hardcodes `.bg(crate::theme::PHOSPHOR_GREEN)` highlight (scroll/render.rs ~413).
- `interaction/modal.rs:118-121`: `dismiss_current` sets `current = None`, keeps parents (doc says "callers … manage parent restoration themselves"); `open_sub` (~105) `take()`s None and pushes nothing, silently stacking a new modal over stale parents; a later `pop()` restores an unrelated ancestor.
- Note: F-A (hardcoded `Theme::default()` in `render_scrollable_block_at`, also at `layout/dialog.rs:398`) — resolved by this plan via deletion of those functions.
- Repo conventions: forward-only + migration file; regenerate `public-api.txt`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Scroll tests | `cargo test -p termrock scroll` | all pass |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0, zero diffs |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/scroll/{mod.rs, render.rs}`
- `crates/termrock/src/layout/{mod.rs, dialog.rs}` (+ its tests dir)
- `crates/termrock/src/interaction/{mod.rs, hover_tracker.rs, modal.rs}`
- Call-site updates (widgets/lookbook, compiler-guided)
- `migrations/000N-*.md` + `MIGRATING.md`; `public-api.txt` regen

**Out of scope**:
- Widget behavior/rendering (deletions must be render-neutral — preview gate proves it).
- The focus system (`FocusState`/`FocusOwner`) — Plan 032's spike owns focus; only modal STATE-consistency is fixed here, focus-restore goes to 032.
- Perf changes beyond what deletion removes (Plans 015/016 own perf).

## Git workflow

- Directly on `main`; commit series, each green: `refactor(scroll)!: single dialog-scroll type`, `refactor(scroll)!: intent-revealing width/offset names, drop orphaned render family`, `refactor(interaction)!: one hover primitive; fix modal dismiss state`.

## Steps

### Step 1: Delete `DialogBodyScroll`

Port any capability the dead twin has that the live one lacks onto `scroll::DialogScroll` FIRST — inspect: `render_scrollbars`, `handle_raw_key_for_axes`, `on_sgr_wheel_button_for_axes`, `on_mouse_scroll_for_size` have no `DialogScroll` equivalents. For each, check for in-repo need: zero consumers means port NOTHING — delete the type, its impl, and its tests wholesale; keep `ScrollAxes`/`ScrollAxis` if `DialogScroll` or live code uses them (grep). Remove the `layout/mod.rs:6` re-export entries.

**Verify**: `cargo test --workspace --all-features --locked` → all pass; `grep -rn "DialogBodyScroll" crates/` → empty.

### Step 2: Retire the orphaned render family

Delete (with their tests): `render_scrollable_block`, `render_scrollable_block_at`, `render_lines_with_offset_in_area`, `render_selected_lines_in_area`, `ScrollableList`, `ScrollbarStyle` (if only ScrollableList used it — grep), `render_scrollable_dialog_body`, `dialog_inner_chunks`, `render_dialog_shell`, `dialog_scroll_axes`, `dialog_inner_height`, and helper-only-for-them internals (`add_trailing_padding` if orphaned after — grep). KEEP the live primitives: `render_vertical_scrollbar*`, `render_horizontal_scrollbar*`, `full_cell_thumb`, `tail_vertical_thumb`, `is_scrollable`, `effective_offset` (one copy), `apply_scroll_delta*`, `clamp_scroll_offset`, `render_line_with_fixed_prefix_scroll` (grep each for widget consumers before deciding; anything with a live consumer stays).

IMPORTANT interaction: Plans 015/016 EDIT some of these functions. If their rows are DONE, delete the post-perf versions (the perf work inside deleted functions is not wasted — Viewport/dialog-body keep their fixes; only the orphaned free functions go). If 015/016 are TODO, coordinate: deleting first shrinks their scope — note it in their rows.

**Verify**: workspace tests + preview check green (previews must be byte-identical — nothing live was removed); `cargo doc` builds (no dangling links).

### Step 3: Collapse the name collisions

With render.rs thinned, rename survivors to intent-revealing names and drop the alias band-aid: keep `scroll::max_line_width` (raw) as-is; the padded variant (render.rs:202) — if it survived Step 2 (used by the mounts-panel path — grep `padded_line_display_cols` consumers) rename to `padded_max_line_width`; delete `rendered_max_line_width`/`rendered_max_offset`/`rendered_cursor_follow_offset` aliases; keep exactly one `max_offset` (usize, mod.rs:85) + the explicit `max_offset_u16`; one `cursor_follow_offset`; one `is_scrollable`/`effective_offset`.

**Verify**: `grep -rn "rendered_" crates/termrock/src/scroll/mod.rs` → empty; workspace green.

### Step 4: One hover primitive

Decision: keep `HitRegion` (widgets' shared rect+id record, heavily used) + ONE query type. `HoverState` is the smaller, caching, `HitRegion`-native design — keep `HoverState`, delete `hover_tracker.rs` entirely (`HoverTracker`, `ClickableRect`; zero consumers; its doc-link value to OSC pointer helpers moves onto `HoverState`'s docs). This also erases the O(n²) issue. Fix the module doc links that referenced hover_tracker (Plan 012 already re-pointed some — check).

**Verify**: `grep -rn "HoverTracker\|ClickableRect\|hover_tracker" crates/ --include="*.rs"` → empty; workspace green.

### Step 5: Fix the `ModalStack` state hole

Make the orphaned-parents state unrepresentable-by-default: change `open_sub` to debug-panic or (better, forward-only) change `dismiss_current` semantics: it now ALSO clears parents (`self.parents.clear()`), and rename it `dismiss_all_keeping_nothing`? No — simplest coherent contract: `dismiss_current` becomes exactly `pop()`-without-restore is confusing. Decide: delete `dismiss_current` (its documented use case — "callers manage parents themselves" — has zero consumers) and let callers use `take_current()` + explicit parent management via `parents_mut()`. Add tests: `open_sub` after `take_current` behaves sanely; `pop` after `open → open_sub → pop → pop` restores in order; the deleted-method migration note tells consumers to use `take_current`.

**Verify**: `cargo test -p termrock interaction` → new tests pass.

### Step 6: Migration file + regen

Next-numbered migration: removed `DialogBodyScroll` (→ `scroll::DialogScroll`), removed render family + `ScrollableList` (→ `widgets::List` / scrollbar primitives), removed `rendered_*` aliases (→ new names table), removed `HoverTracker`/`ClickableRect` (→ `HoverState`+`HitRegion`), removed `ModalStack::dismiss_current` (→ `take_current`). Regenerate `public-api.txt`.

**Verify**: `mise run gate` → exit 0; migration indexed.

## Test plan

- Deletions ride the existing 51-test scroll suite + previews (byte-identical requirement).
- New: ModalStack state tests (Step 5, ≥3 cases); a `HoverState` unit test (hit/miss/cache) since it's now THE primitive and had none.

## Done criteria

- [x] Greps empty: `DialogBodyScroll`, `ScrollableList`, `render_scrollable_block`, `rendered_`, `HoverTracker`, `dismiss_current`
- [x] Exactly one `max_line_width`/`max_offset`/`cursor_follow_offset`/`is_scrollable`/`effective_offset` in `scroll`
- [x] Preview SVGs byte-identical; full workspace + gate green
- [x] Migration file indexed; `public-api.txt` regenerated
- [x] plans/README.md updated (incl. F-A marked resolved-by-deletion, 017-coordination note)

## STOP conditions

- Grep reveals a live consumer for anything listed as orphaned (external evidence beats the audit) — keep that item, report it.
- `padded_line_display_cols`/padded-width path turns out to feed a LIVE widget (mounts-panel heritage) — keep the padded width under its new name and verify with the preview gate.
- Preview SVGs change at all — a deletion touched live rendering; revert that hunk and report.

## Maintenance notes

- The deleted dialog-shell functions embodied a "dialog layout via free functions" idea — if a future consumer wants it, the right shape is a widget (per the construction idiom of Plan 013), not resurrecting free functions.
- `HoverState` is now canonical; new widgets expose `HitRegion`s and let consumers drive `HoverState` OR take state-owned hover methods (the Plan 011 contract) — document both in its rustdoc.
- Plan 032 (focus spike) adds focus-restore to `ModalStack` — the Step 5 cleanup is its precondition.
