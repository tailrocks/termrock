# Plan 012: One canonical home and one public path per concept — split the geometry junk drawer, kill duplicate exports

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/lib.rs crates/termrock/src/geometry.rs crates/termrock/src/text/ crates/termrock/src/osc/ crates/termrock/src/scroll/`
> Plans 004/009 legitimately touched osc/lib.rs. Other mismatches with
> "Current state" = STOP.

## Status

- **Priority**: P2
- **Effort**: M-L
- **Risk**: MED (large mechanical import churn; zero behavior change intended)
- **Depends on**: plans/009-brand-constant-purge.md (lib.rs palette already gone), plans/011-event-model-convergence.md (avoid double-churning the same migration doc space — order matters, not code)
- **Category**: tech-debt
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The same function is publicly reachable at up to three paths — `termrock::display_cols`, `termrock::geometry::display_cols`, and `termrock::text::display_cols` — because `lib.rs` re-exports ~15 geometry items at the crate root while `text/mod.rs` is a pure re-export facade whose doc claims ownership ("Product-neutral terminal text measurement, sanitization…") of functions `geometry.rs` actually implements. `geometry.rs` itself is a junk drawer of five unrelated concerns (display-width measurement, tab layout, hint-row math, title sanitization, dialog centering). The crate also carries a *duplicate* `PointerShape` at the root — a second enum with a different variant set from `osc::request::PointerShape`, plus a duplicate OSC-22 encoder producing the same bytes as `osc::encode::encode_pointer`. The result: 3,336 public items for an 18-widget library, unpredictable navigation for humans and agents, and more surface to break on every forward-only change.

## Current state

- `crates/termrock/src/lib.rs:25-31` — the crate-root re-export block:

```rust
pub use style as theme;
pub use style::Theme;

pub use geometry::{
    FixedPrefixSegment, HintSpan, TAB_GAP, TabCell, centered_rect, display_cols,
    display_cols_slice, fixed_prefix_scroll_segments, hint_row_cols, is_terminal_control_char,
    lay_out_tabs, leading_space_cols, padded_line_display_cols, sanitize_terminal_title,
    tab_at_column, take_display_cols,
};
pub use scroll::{TailScroll, is_scrollable, max_line_width, max_offset};
```

- `crates/termrock/src/text/mod.rs` (entire file — a facade owning nothing):

```rust
//! Product-neutral terminal text measurement, sanitization, and windows.

pub use crate::ansi_text::{strip_bytes, styled_spans};
pub use crate::geometry::{
    FixedPrefixSegment, display_cols, display_cols_slice, fixed_prefix_scroll_segments,
    is_terminal_control_char, leading_space_cols, padded_line_display_cols,
    sanitize_terminal_title, take_display_cols,
};
```

- `geometry.rs` (~329 lines) concern inventory: display-width measurement (`display_cols` ~106, `take_display_cols` ~121, `display_cols_slice` ~142, `leading_space_cols` ~166, `padded_line_display_cols` ~188), scroll segments (`fixed_prefix_scroll_segments` ~212), sanitization (`is_terminal_control_char` ~96, `sanitize_terminal_title` ~270), tab layout (`TabCell` ~13, `TAB_GAP` ~24, `lay_out_tabs` ~301, `tab_at_column` ~325), hint rows (`HintSpan` ~36, `hint_row_cols` ~87), dialog centering (`centered_rect` ~56).
- Duplicate pointer machinery at the crate root (`lib.rs:208-244`): `enum PointerShape { Default, Pointer, Text, EwResize, NsResize, Grabbing }` + `as_osc22_name()` + `clickable_pointer_shape()` + `osc22_pointer_shape() -> String`. The canonical one: `osc/request.rs` `enum PointerShape { Default, Pointer, Text, Crosshair }` + `osc/encode.rs` `encode_pointer` emitting the identical `\x1b]22;{};\x1b\\` bytes. **Variant sets differ** — the union is `Default, Pointer, Text, Crosshair, EwResize, NsResize, Grabbing`. Only in-repo references to the root copy are doc links in `interaction/hover_tracker.rs:93-94`.
- `pub use style as theme` — two module names for one module (`termrock::style` and `termrock::theme`); in-repo code imports via `termrock::theme::Role` in tests (`tests/form.rs:8`) and `crate::style::` internally.
- Surface size: `docs/api/public-api.txt` ≈ 3,336 pub items.
- Repo conventions: forward-only + migration file; regenerate `public-api.txt` (Plan 003 gate).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Docs | `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` | exit 0 |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/lib.rs`, `geometry.rs` (dissolved), `text/mod.rs`, `layout/` (receives geometry items), `osc/request.rs` (variant union), `interaction/hover_tracker.rs` (doc links), `widgets/` + `scroll/` + lookbook **import lines only**
- `migrations/000N-*.md` + `MIGRATING.md`; `public-api.txt` regen

**Out of scope**:
- Function bodies — this is a *move* refactor; any behavior change is a defect.
- `scroll` module internals (its root re-exports get removed; module stays).
- Construction idioms / `#[non_exhaustive]` — Plan 013.

## Git workflow

- Directly on `main`; `git commit -s -m "refactor(api)!: canonical module homes and single public paths"`; migration file same commit. Keep each intermediate commit green if splitting.

## Steps

### Step 1: Give `text` its real body

Move from `geometry.rs` into `text/` (real definitions, not re-exports): `display_cols`, `take_display_cols`, `display_cols_slice`, `leading_space_cols`, `padded_line_display_cols`, `fixed_prefix_scroll_segments`, `FixedPrefixSegment`, `is_terminal_control_char`, `sanitize_terminal_title`. Keep `text`'s existing `ansi_text` re-exports. Update every internal `crate::geometry::X` / root-path caller to `crate::text::X` (compiler-guided).

**Verify**: `cargo test --workspace --all-features --locked` → all pass.

### Step 2: Rehome the remaining geometry concerns

- `centered_rect` → `layout` (dialog geometry lives there; `layout/dialog.rs` and `layout/mod.rs` exist).
- Tab layout (`TabCell`, `TAB_GAP`, `lay_out_tabs`, `tab_at_column`) → `widgets::tabs` (only Tabs consumes them; check external uses first: `grep -rn "lay_out_tabs\|tab_at_column\|TabCell" crates/ --include="*.rs"` — if the lookbook or another widget uses them, `widgets::tabs` is still right, just keep them `pub`).
- Hint rows (`HintSpan`, `hint_row_cols`) → `widgets::hint_bar` (which already owns `Hint`, `styled_hint_spans`, `wrapped_hint_lines` — ending the HintSpan/Hint split across modules).
- Delete `geometry.rs` and `pub mod geometry` once empty.

**Verify**: `cargo test --workspace --all-features --locked` → all pass; `grep -rn "geometry" crates/termrock/src/ --include="*.rs"` → no module references remain.

### Step 3: Kill the crate-root re-exports and the theme alias

In `lib.rs`, delete the `pub use geometry::{...}` and `pub use scroll::{...}` blocks. Decide the alias direction: keep **`style`** as the module name (matches the directory) and delete `pub use style as theme`; keep `pub use style::Theme;` (the single ergonomic root export — `Theme` is the crate's entry-point type per lib.rs docs). Update in-repo `termrock::theme::` users (tests/form.rs and any lookbook imports) to `termrock::style::`.

**Verify**: `cargo test --workspace --all-features --locked` → all pass; `grep -rn "termrock::theme\|crate::theme" crates/ --include="*.rs"` → empty.

### Step 4: Unify `PointerShape`

In `osc/request.rs`, extend the canonical enum with the root copy's extra variants: `EwResize, NsResize, Grabbing` (name strings `"ew-resize"`, `"ns-resize"`, `"grabbing"` — the root copy's `as_osc22_name` shows them). Delete from `lib.rs`: `PointerShape`, `as_osc22_name`, `clickable_pointer_shape`, `osc22_pointer_shape`. Re-point `interaction/hover_tracker.rs:93-94` doc links at `crate::osc::{PointerShape, encode_pointer}` (add a `clickable_pointer_shape`-equivalent helper in `osc` only if a caller needs it — in-repo there is none; document the one-liner instead).

**Verify**: `cargo test -p termrock osc` → all pass (extend the exact-bytes test with one new variant); `grep -rn "osc22_pointer_shape\|clickable_pointer_shape" crates/ --include="*.rs"` → empty; docs build clean (the doc links resolve).

### Step 5: Surface audit pass

Generate the fresh API report (Plan 003 command). Review the diff: everything that vanished should be the moved/deleted paths. Then run a quick accidental-surface sweep: for each `pub fn` in `scroll/render.rs` and `text/` that no in-repo consumer or documented story uses (`grep` each), demote to `pub(crate)` **only** when clearly internal plumbing (e.g. helpers whose docs reference internal widget behavior). When in doubt, leave `pub` — this step trims obvious accidents, not the philosophy of open primitives (AGENTS.md wants open, inspectable building blocks).

**Verify**: `cargo test --workspace --all-features --locked` → all pass; `cd docs && bun run build` → exit 0 (catalog regex reads `impl ... Widget for &termrock::widgets::X` lines — widget impls unaffected by this plan).

### Step 6: Migration file

Old→new path table for every moved item (three-path items list all removed paths), the `theme`-alias removal, the `PointerShape` merge (variant union; `osc22_pointer_shape(shape)` → `String::from_utf8(encode_pointer(shape)).unwrap()` or the new helper). Before/after import block example. Link from `MIGRATING.md`.

**Verify**: migration indexed; `mise run gate` → exit 0.

## Test plan

- Move-refactor: the existing suite is the net (tests themselves get mechanical import updates).
- New: one osc test variant for `EwResize` exact bytes.
- After Step 5, `cargo doc` builds warning-free proving no dangling intra-doc links.

## Done criteria

- [ ] `geometry.rs` deleted; `text/` owns measurement/sanitization bodies; tab/hint/centering items live with their consumers
- [ ] Exactly one public path per moved item (`grep -n "pub use geometry\|pub use scroll" crates/termrock/src/lib.rs` → empty; `pub use style as theme` gone)
- [ ] One `PointerShape` (union variants) in `osc`; root copy + 3 helpers deleted
- [ ] `cargo test --workspace --all-features --locked` + preview check + docs build → all green
- [ ] Migration file indexed; `public-api.txt` regenerated
- [ ] `plans/README.md` status row updated

## STOP conditions

- A moved function turns out to have callers in both `widgets` and `scroll` layers that would create a dependency cycle (e.g. `text` needing `widgets` types) — report the cycle; do not invert layering ad hoc.
- The catalog gate (`bun run build`) fails after regen — a widget's public path changed in a way the regex tracks; report before adjusting the regex.
- More than ~40 files need import edits — pause, commit the green subset, report scope.

## Maintenance notes

- Rule going forward: new helpers land in the module that owns the concern; crate-root exports are reserved for `Theme` (and nothing else without a migration-documented decision).
- Plan 013 (construction/non_exhaustive) and Plan 014 (missing_docs) assume these final paths — run them after.
- The `hover_tracker` doc links are the only place that referenced the deleted root helpers; future doc links should target `osc` directly.
