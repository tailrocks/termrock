# Plan 003: Activate the spacing system (padding, gaps, dialog rhythm)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. On any STOP condition, stop and report. Update your row in
> `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/widgets/panel.rs crates/termrock/src/widgets/dialog.rs crates/termrock/src/layout crates/termrock/src/style/density.rs`
> Excerpts below were verified at `539e7d03`; plans 001–002 will have touched
> `style/` — that is expected. On mismatch in the widget/layout excerpts, STOP.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED (layout geometry changes ripple into many widget tests and stories)
- **Depends on**: plans/001-surface-ladder-and-role-expansion.md
- **Category**: tech-debt (visual foundation)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

The spacing system exists (`SpacingScale`, `Density::padding_x/y/gap`) but is
dead code: 2 of 141 widget files consume it, `Panel` zeroes its padding
explicitly, and `Stack`/`Grid` default to gap 0 with their density-aware
constructors called only from tests. Body text sits flush against border
glyphs across the whole library — a large share of the "cheap CLI" read.
Dialogs additionally lost their interior rhythm (leading/mid/trailing
spacers) and rescale with every terminal resize; the primary consumer
(Jackin) rebuilt both product-side, which is the strongest evidence the
library should own them.

## Current state

Verified excerpts (`539e7d03`):

`widgets/panel.rs:858-863` — Panel zeroes Surface padding:

```rust
let _ = Surface::new(self.tokens)
    .recipe(surface_recipe)
    .bordered(false)
    .fill(fill_policy)
    .padding(0, 0)
    .paint(area, buffer);
```

`widgets/panel.rs:711-715` — `Panel::layout` insets by border only, no pad
term:

```rust
pub fn layout(&self, area: Rect, state: Option<&PanelState>) -> PanelParts {
    let collapsed = state.is_some_and(|s| s.collapsed && self.collapsible);
    let has_border = self.has_box_border();
    let border_cells: u16 = if has_border { 1 } else { 0 };
    let inner = shrink(area, border_cells, border_cells, border_cells, border_cells);
```

`layout/stack.rs:235-247` — `StackSpec::default()` → `gap: 0, pad_x: 0,
pad_y: 0`; `vertical()`/`horizontal()` same; `with_density`
(`stack.rs:283-288`) and `with_spacing` (`stack.rs:292-297`) exist and are
correct but unused by widgets.

`style/density.rs:22-47` — Comfortable = pad_x 2, pad_y 1, gap 1; Compact =
1/0/0; Dashboard = 0/0/0. `SpacingScale::from_density` (`tokens.rs:158-168`)
already resolves these.

`widgets/dialog.rs:1292-1300` — dialog body starts directly at `inner.y`
with no leading spacer; description/validation/footer rows are packed
adjacent (lines 1283-1300 compute `has_desc`/`has_validation`/`has_footer`
as adjacent 1-row bands).

Widget-local density enums that shadow the global model (verified locations):
`widgets/list.rs:75` (`ListDensity`), `widgets/empty_state.rs:124`
(`EmptyDensity`), `widgets/key_value_list.rs:37` (`KvDensity`),
`widgets/data_view.rs:29` (`DataDensity`).

Design constraints:

- `docs/design/component-anatomy-spec.md` specifies panel inset `(2,1)` at
  Comfortable density; that is exactly `Density::Comfortable`'s
  `padding_x/padding_y` — no new constants needed.
- Jackin evidence (design SoT `docs/design/component-visual-richness-plan.md`
  §3.2/§4.4): five-slot dialog rhythm (leading spacer / body / mid spacer /
  actions / trailing spacer) and reference-width dialog sizing
  (`REFERENCE_COLS = 160`).

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Targeted | `cargo nextest run -p termrock panel dialog list stack grid` | pass |

## Scope

**In scope**:

- `crates/termrock/src/widgets/panel.rs`
- `crates/termrock/src/widgets/dialog.rs`
- `crates/termrock/src/widgets/surface.rs` (only if plumbing requires)
- `crates/termrock/src/layout/stack.rs`, `crates/termrock/src/layout/grid.rs`
- `crates/termrock/src/widgets/list.rs`, `empty_state.rs`,
  `key_value_list.rs`, `data_view.rs` (density-enum collapse only)
- `crates/termrock/src/layout/mod.rs` (`DialogSpec` — locate `resolve_dialog`)
- `crates/termrock-lookbook/src/stories.rs` (story updates for new geometry)
- `migrations/0263-*.md` + `MIGRATING.md`
- `plans/README.md`

**Out of scope**:

- Colors/roles (done in 001), chips/fills in individual widgets (004/005).
- Widgets not listed — the cascade to remaining widgets is plan 010.

## Git workflow

`main`, `git commit -s`, suggested message:
`feat(layout)!: activate density-driven padding, dialog rhythm, reference-width dialogs`.

## Steps

### Step 1: Panel padding

- Remove `.padding(0, 0)` at `panel.rs:862`; pass
  `.padding(recipe.pad_x, recipe.pad_y)` where `recipe` is
  `self.tokens.panel_recipe(...)` (already computed at `panel.rs:871`; hoist
  it above the Surface call). `PanelRecipe.pad_x/pad_y` already carry the
  density values (`tokens.rs:602-603`, verified).
- In `Panel::layout` (`panel.rs:711`), after the border shrink, apply the
  same pad: `let inner = shrink(inner, pad_x, pad_y, pad_x, pad_y);` using
  `self.tokens.spacing`. `PanelParts` consumers (body, actions, footer
  rects) must derive from the padded inner. Guard degenerate areas: skip
  padding when `inner.width < pad_x * 2 + 4` or `inner.height < pad_y * 2 + 1`
  (narrow-terminal law — content beats padding).
- `PanelVariant::Quiet` and `Dashboard` density keep pad 0 automatically
  (Density::Dashboard → 0/0).

**Verify**: `cargo nextest run -p termrock panel` → failures only in tests
asserting old flush geometry; update those expectations. Then pass.

### Step 2: Dialog five-slot rhythm

In `dialog.rs` `paint` (line 1246): after `block.inner(area)`, lay out the
interior as: leading spacer (1 row) / description / body (flex) / validation
/ mid spacer (1 row) / actions (`action_rows`) / trailing spacer (1 row).
Spacers appear only when `inner.height` affords them (define the minimum:
rhythm applies when `inner.height >= 3 + action_rows + desc/validation rows`;
below that, fall back to current packed behavior — small-terminal
degradation must remain lossless). Body x-inset: `spacing.pad_x` columns each
side (dialog text currently starts at `inner.x`).

**Verify**: `cargo nextest run -p termrock dialog` → update geometry
expectations, then pass.

### Step 3: Reference-width dialog policy

Locate `DialogSpec`/`resolve_dialog` in `crates/termrock/src/layout/`
(search: `grep -rn "resolve_dialog\|DialogSpec" crates/termrock/src/layout`).
Add a sizing mode:

```rust
/// Width as percent of a virtual reference terminal (default 160 cols),
/// clamped to the actual area minus margins — dialogs hold stable width
/// across host resizes instead of rescaling continuously.
pub fn preferred_pct_of_reference(self, pct: u16) -> Self { … }
```

with `REFERENCE_COLS: u16 = 160` const. Existing min/preferred/max behavior
unchanged when the mode is unset.

**Verify**: unit test — at terminal widths 200, 160, 120, and 80, a
`preferred_pct_of_reference(50)` dialog resolves to 80, 80, 80, then clamps
(< 80 only when area demands); `cargo nextest run -p termrock layout` → pass.

### Step 4: Stack/Grid density defaults + enum collapse

- Add `StackSpec::from_system(&DesignSystem)` and
  `GridSpec::from_system(&DesignSystem)` (thin wrappers over the existing
  `with_spacing`). Do **not** change `Default` (gap 0) — bare `Stack` is a
  low-level primitive; density flows in where a `DesignSystem` is in hand.
- Replace the four widget-local density enums with the global
  `style::Density`:
  - `ListDensity` (`list.rs:75`) — map `Compact→Density::Compact`,
    `Comfortable→Density::Comfortable`; `row_height(secondary_below)` logic
    moves to a private helper reading `Density`.
  - Same for `EmptyDensity` (`empty_state.rs:124`), `KvDensity`
    (`key_value_list.rs:37`), `DataDensity` (`data_view.rs:29`).
  - Public API change: builder methods that took the local enums now take
    `Density`. Remove the local enums entirely (forward-only law — no
    aliases).

**Verify**: `cargo check -p termrock` → all internal call sites updated;
`cargo nextest run -p termrock list empty_state key_value data_view` → pass.

### Step 5: Lookbook, migration, gate

- Regenerate/adjust lookbook stories whose fixed sizes now clip padded
  content (stories declare cols×rows; bump sizes where content no longer
  fits — search failing story tests).
- `migrations/0263-v0.13.0-spacing-activation.md`: panel default padding
  `(2,1)` at Comfortable, dialog rhythm + minimum heights, density-enum
  removals with exact before/after for each removed enum, `DialogSpec`
  addition, opt-outs (`Density::Dashboard`, `PanelVariant::Quiet`). Link in
  `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0. Commit.

## Test plan

- New: panel padded-inner geometry test (Comfortable vs Dashboard), dialog
  five-slot layout test (rows land where expected at height 20; packed
  fallback at height 8), reference-width test (Step 3), density mapping
  tests for each collapsed enum. Model after existing geometry tests in
  `panel.rs`/`dialog.rs` test modules.
- Expect a broad but shallow wave of geometry expectation updates in
  existing tests — each must be reviewed as "old flush layout" before
  updating.

## Done criteria

- [ ] `mise run check` + `mise run gate` exit 0
- [ ] `grep -n "padding(0, 0)" crates/termrock/src/widgets/panel.rs` → no matches
- [ ] `grep -rn "enum ListDensity\|enum EmptyDensity\|enum KvDensity\|enum DataDensity" crates/` → no matches
- [ ] Dialog rhythm test passes; reference-width test passes
- [ ] `migrations/0263-*.md` exists, linked from `MIGRATING.md`
- [ ] `plans/README.md` updated

## STOP conditions

- `PanelParts` geometry is consumed by interaction hit-regions in a way that
  padding breaks (hit tests failing en masse after Step 1) → report with the
  failing test list.
- The density-enum collapse would force a `patterns/` rewrite beyond
  mechanical enum substitution → report scope.
- Story regeneration requires the docs-site pipeline (bun) and it is
  unavailable in the environment → note it, finish Rust-side, mark partial.

## Maintenance notes

- Plan 004/005 paint fills into the areas this plan padded — reviewers
  should check fills cover the padding cells (Surface paints the whole area,
  so they will).
- Any future widget must consume `DesignSystem.spacing`, never local
  constants — reviewers should reject new hardcoded pads.
