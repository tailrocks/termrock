# Plan 007: Pin current Form and DetailTable behavior with characterization tests before the event-model refactor

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/widgets/form.rs crates/termrock/src/widgets/detail_table.rs crates/termrock/tests/`
> Plan 005 legitimately adds a release guard to other widgets; if `form.rs`
> or `detail_table.rs` themselves changed beyond that, compare excerpts and
> STOP on mismatch.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (additive tests only)
- **Depends on**: none (but MUST land before plans/011-event-model-convergence.md)
- **Category**: tests
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

`form.rs` (712 lines, the largest widget) and `detail_table.rs` (671 lines) have the thinnest behavioral coverage relative to their size: Form has 6 integration tests, DetailTable 3 inline tests, while both carry complex focus/scroll/hit-region state machines. Plan 011 rewrites every widget's event-handling surface; without characterization tests pinning today's outcomes, that refactor can silently change behavior. These tests describe *current* behavior — they are the safety net, not a spec of ideal behavior.

## Current state

- Form's public interaction surface (`crates/termrock/src/widgets/form.rs`):
  - `FormState::focus(Option<Id>)` (line ~124), `scroll_by(delta: i32) -> usize` (~149), `scroll_to_position(Position) -> bool` (~160), `handle_key(...)` (~179; multi-line signature — read it first), `hover(Position) -> Option<&Id>` (~203), `click(Position) -> FormOutcome<Id>` (~212), `regions()`, `field_regions()`.
  - `FormOutcome<Id> { Ignored, FocusChanged(Id), Activated(Id) }`.
  - `handle_key` begins `if !self.active || key.kind == KeyEventKind::Release` (line ~184).
- DetailTable's surface (`crates/termrock/src/widgets/detail_table.rs`):
  - `DetailTableState::select_next/select_previous(rows)` (~96/100), `hover_at(Position)` (~128), `activate_at(Position)` (~138), `activate_link_at(Position)` (~158), `mark_copied(Option<Id>)` (~171), `clamp_scroll()` (~175); widget-side `outcome_at(...)` (~201), `activate_selected(&DetailTableState)` (~210), `hyperlink_regions(...)` (~227). Note: **no `handle_key`** — keyboard is `select_next/previous` only.
  - `DetailTableOutcome<Id> { Ignored, Selected(Id), Copy(Id), ActivateLink(Id) }`.
- Test pattern to copy — `crates/termrock/tests/form.rs` opens with borrowed fixture builders:

```rust
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    widgets::{Form, FormField, FormOutcome, FormSection, FormState},
};

fn fields() -> Vec<FormField<'static, &'static str>> {
    vec![
        FormField { id: "host", label: Line::from("Host"), value: Line::from("localhost"),
                    help: Some(Line::from("Server name or address")), error: None,
                    required: true, enabled: true },
        ...
```

  and renders into an in-memory `Buffer` via `StatefulWidget` — no terminal needed.
- Existing DetailTable inline tests live in `detail_table.rs` `mod tests` / `widgets/tests.rs`; read them before writing to avoid duplicating covered cases (mouse regions and basic render are covered; keyboard traversal, disabled rows, copy-affordance state, wrap-mode hit regions, scroll clamping largely are not).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Run new form tests | `cargo test -p termrock --test form` | all pass |
| Run detail-table tests | `cargo test -p termrock --test detail_table` | all pass (new file) |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/tests/form.rs` (extend)
- `crates/termrock/tests/detail_table.rs` (create)

**Out of scope**:
- ANY change to `form.rs` / `detail_table.rs` source. If a test reveals a behavior that looks like a bug, record it in the test with a `// CHARACTERIZATION:` comment stating "this pins current behavior, potentially wrong — see plans/README findings" and report it. Do not fix.
- Render-output assertions beyond what identifies a regression (hit regions and outcomes are the focus; pixel-exact SVG is the lookbook's job).

## Git workflow

- Directly on `main`; `git commit -s -m "test(widgets): characterize form and detail-table interaction"`.

## Steps

### Step 1: Extend `tests/form.rs` — keyboard state machine

Add tests (reusing the existing `fields()`/section fixtures; render once into a `Buffer` sized ~80×24 to populate regions where needed):

1. `tab_cycles_focus_across_sections_and_skips_disabled` — build a form with a disabled field (`enabled: false`); Tab from last field wraps (or stops — pin whatever happens); disabled fields never receive focus.
2. `enter_on_focused_field_activates` — expect `FormOutcome::Activated(id)`.
3. `inactive_form_ignores_keys` — after `active` is false (find the setter/constructor flag), every key yields `Ignored`.
4. `arrow_navigation_matches_tab_order_or_column_layout` — pin Up/Down behavior in the two-column responsive layout (narrow buffer → one column, wide → two; assert focus order in both). Use two renders with widths ~40 and ~100.
5. `scroll_by_clamps_at_bounds` — `scroll_by(-1)` at top returns 0-offset; `scroll_by(large)` clamps to max; record the returned `usize` meaning (it returns the new offset — pin it).

**Verify**: `cargo test -p termrock --test form` → all pass.

### Step 2: Extend `tests/form.rs` — pointer + regions

6. `click_on_field_focuses_and_reports` — click inside a field's `FormFieldRegion.area` → `FocusChanged` or `Activated` (pin actual); `hover` over the same region returns the id.
7. `partially_clipped_field_retains_union_hit_region` — small viewport so a field is half-scrolled off; per COMPONENTS.md, "Partially clipped fields retain a union hit region plus optional visible label/value/support subregions" — assert the region exists and subregion `Option`s are `None` when invisible.
8. `click_outside_any_region_is_ignored`.

**Verify**: `cargo test -p termrock --test form` → all pass.

### Step 3: Create `tests/detail_table.rs`

Model the file layout on `tests/form.rs` (imports, fixture fn returning `Vec<DetailRow<'static, &'static str>>` — read `DetailRow`'s fields in `detail_table.rs` first; it includes `id`, `label`, `value`, `href: Option<&str>`, and capability-related fields — copy a fixture shape from the existing inline tests). Tests:

1. `select_next_previous_traverse_and_clamp` — from `None`, `select_next` selects the first row; at last row it stays (or wraps — pin actual); `select_previous` mirror.
2. `select_skips_or_includes_disabled_rows` — if `DetailRow` has an enabled/capability flag affecting selection, pin it.
3. `activate_at_on_copyable_row_returns_copy` — render, take a `DetailRegion` with copy capability, click its `action_area` → `Copy(id)`; then `mark_copied(Some(id))` and assert a re-render changes the affordance cell (grab one cell symbol before/after).
4. `activate_link_at_returns_activate_link` — row with `href: Some("https://example.com")` → `ActivateLink(id)` when clicking `value_area`.
5. `hover_tracks_row_id` — `hover_at` inside/outside regions.
6. `clamp_scroll_after_rows_shrink` — scroll down, rebuild with fewer rows, `clamp_scroll()`, assert offset within new bounds (find the offset accessor; if none is public, assert indirectly via which rows render into the buffer).
7. `activate_selected_routes_by_capability` — keyboard path: select a row, `activate_selected` returns `Copy`/`ActivateLink` per the row's capability, `Ignored` when nothing selected.
8. `wrap_mode_regions_cover_continuation_rows` — if the widget exposes a wrap flag (grep `wrap` in `detail_table.rs` — it exists per the `continuation` logic around line ~430), render a long value in a narrow buffer and assert its region spans multiple visual rows.

**Verify**: `cargo test -p termrock --test detail_table` → all pass, ≥8 tests.

### Step 4: Full suite

**Verify**: `cargo test --workspace --all-features --locked` → all pass; `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → exit 0.

## Test plan

This plan *is* the test plan: ~13 new characterization tests across two files. Every assertion pins observed behavior; when observed behavior looks wrong, keep the pin + `// CHARACTERIZATION:` comment + report.

## Done criteria

- [ ] `cargo test -p termrock --test form` → passes with ≥8 new tests beyond the original 6
- [ ] `crates/termrock/tests/detail_table.rs` exists; `cargo test -p termrock --test detail_table` → ≥8 tests pass
- [ ] `git diff --stat crates/termrock/src/` → empty (no source changes)
- [ ] `cargo test --workspace --all-features --locked` → all pass
- [ ] `plans/README.md` status row updated (and any suspected-bug notes recorded there)

## STOP conditions

- A fixture cannot be built because `DetailRow`/`FormField` fields differ from the excerpts (drift) — recheck against live code, report.
- A test cannot observe needed state (no public accessor for scroll offset, etc.) and the only way to assert would be modifying source — report the missing accessor instead of adding one.
- You find outright panics (index out of bounds etc.) while exercising edge cases — that's a bug discovery: write the test as `#[should_panic]` with a `// CHARACTERIZATION: BUG` comment ONLY if the panic is deterministic, and report it prominently.

## Maintenance notes

- Plan 011 will change these APIs; these tests are the contract it must consciously update — every assertion it changes should be an intended semantic change, called out in 011's migration file.
- After Plan 011 lands, remove the `// CHARACTERIZATION:` markers from tests whose behavior was deliberately redesigned.
