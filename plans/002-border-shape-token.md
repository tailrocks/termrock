# Plan 002: Make border shape a theme token (`BorderShape`)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/style crates/termrock/src/widgets/panel.rs crates/termrock/src/widgets/surface.rs`
> On mismatch with "Current state" excerpts, STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW (additive token; default unchanged)
- **Depends on**: plans/001-surface-ladder-and-role-expansion.md (token file churn; execute after to avoid conflicts)
- **Category**: tech-debt (theming capability)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

TermRock hardcodes square single-line borders everywhere. The phosphor brand
mandates square caps, but TermRock is a product-neutral library: Grok Build —
the premium reference TUI on the same stack — uses rounded borders at every
surface (`BorderType::Rounded`), and consumers with different brands must be
able to choose. Border shape must become a theme token with Square as the
default, without touching the focus law (focus = border color, never weight
or glyph).

## Current state

- `crates/termrock/src/style/tokens.rs` — `DesignSystem` struct (fields
  around line 390–410: `selection: SelectionChrome`, `capability`,
  `breakpoints`, `spacing`, `glyphs`, `density`…). Builder-style methods like
  `.selection(…)`, `.capability(…)`, `.glyphs(…)` exist (see
  `phosphor()`/`ansi()` at `tokens.rs:421-448` chaining them).
- `crates/termrock/src/widgets/panel.rs:873` (verified) — Panel borders come
  from Ratatui `Block::bordered().border_style(border)`; no `BorderType` is
  ever set (defaults to plain/square).
- Hand-drawn corners exist in several widgets (e.g. verified
  `widgets/toast.rs:1045-1049`:
  `("┌", "┐", "└", "┘", "─", "│")` with ASCII fallback `("+", …)`). Those
  widgets migrate to `Surface` in plans 004/005/010 — do NOT convert them
  here; this plan only makes the token exist and wires the two chrome
  authorities (`Panel`, `Surface`).
- `crates/termrock/src/style/glyph.rs` — glyph catalog; `GlyphSet` enum in
  `tokens.rs` (`Ascii`/`Unicode`/`Enhanced`) controls ASCII fallback.
- Repo law (`AGENTS.md`, "Focus-visible panel hierarchy"): "Every panel and
  dialog uses the same single-line border geometry. Border weight never
  communicates focus." Rounded corners keep single-line weight — corner
  glyphs change (`╭╮╰╯`), edges stay `─│` — so the law is preserved; only
  the corner glyph family becomes theme-selectable.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |
| Targeted | `cargo nextest run -p termrock panel` | pass |

## Scope

**In scope**:

- `crates/termrock/src/style/tokens.rs` (add `BorderShape`, `DesignSystem.border_shape` + builder)
- `crates/termrock/src/style/mod.rs` (re-export)
- `crates/termrock/src/widgets/panel.rs` (Block border type from token)
- `crates/termrock/src/widgets/surface.rs` (border glyphs from token)
- `crates/termrock/src/widgets/dialog.rs` (only the `panel.block()` path if it sets borders independently — it reuses `Panel`, so likely no change)
- `crates/termrock-lookbook/src/knobs.rs` + `stories.rs` (add a border-shape knob to the panel story)
- `crates/termrock-lookbook/src/interactors.rs` (panel story's live knob owner)
- `docs/api/public-api.txt` (generated public inventory required by the full gate)
- `docs/public/preview-frames/` (generated packs affected by shared ASCII border resolution)
- `artifacts/visual-qa/plan-002/` (required browser screenshots and review record)
- `migrations/0262-*.md` + `MIGRATING.md`
- `plans/README.md`

**Out of scope**:

- Every widget that hand-draws `┌`-corners (toast, callout, drawer, menus…) —
  they adopt `Surface` in plans 004/005/010 and inherit the token there.
- Any change to focus semantics or border colors.
- `GlyphSet` restructuring (plan 006).

## Git workflow

Same as plan 001: work on `main`, `git commit -s`, Conventional Commits.
Suggested: `feat(style): add BorderShape theme token (square default, rounded opt-in)`.

## Steps

### Step 1: Add the token

In `style/tokens.rs`:

```rust
/// Corner-glyph family for single-line borders. Focus stays color-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BorderShape {
    /// Square corners `┌┐└┘` (phosphor identity default).
    #[default]
    Square,
    /// Rounded corners `╭╮╰╯` (Grok-Build-class themes).
    Rounded,
}
```

Add field `border_shape: BorderShape` to `DesignSystem`, a
`#[must_use] pub const fn border_shape(mut self, shape: BorderShape) -> Self`
builder (match the style of `.selection(…)`), default `Square` in all
presets. Re-export `BorderShape` from `style/mod.rs` alongside the existing
`tokens::` re-exports (line 40). Doc comments on everything.

**Verify**: `cargo check -p termrock` → compiles.

### Step 2: Wire `Panel` and `Surface`

- `panel.rs:873` area: after `Block::bordered().border_style(border)`, add
  `.border_type(...)` mapping `BorderShape::Square → BorderType::Plain`,
  `Rounded → BorderType::Rounded` (import from `ratatui_widgets`/`ratatui`
  blocks module — find the existing `Block` import at the top of `panel.rs`
  and extend it). When `GlyphSet::Ascii` is active, keep the existing ASCII
  border path unchanged (both shapes render `+`).
- `surface.rs`: locate its border-glyph selection (search `"┌"` in the file);
  make corner glyphs resolve through the token the same way (square vs
  rounded corners; edges unchanged).

**Verify**: `cargo nextest run -p termrock panel surface` → pass.

### Step 3: Tests + lookbook knob

- Test in `panel.rs`: `rounded_shape_changes_corners_only` — render a
  bordered panel with `border_shape(BorderShape::Rounded)`, assert corner
  cells are `╭╮╰╯`, edge cells still `─`/`│`, and border style (color) equals
  the Square render's border style.
- Test: `ascii_maps_both_shapes_to_plus` — with `GlyphSet::Ascii`, Square and
  Rounded renders are identical.
- Lookbook: add a `border shape` knob (Square/Rounded) to the panel story in
  `termrock-lookbook` (`knobs.rs` pattern — copy an existing enum knob).

**Verify**: `cargo nextest run -p termrock panel` → pass incl. 2 new tests;
`mise run check` → exit 0.

### Step 4: Migration + gate

`migrations/0262-v0.13.0-border-shape-token.md` (next free number — verify
with `ls migrations | tail -1`; if plan 001 landed 0261, this is 0262):
records the new token, the `DesignSystem` field addition (struct-literal
consumers must add the field or use builders), default unchanged. Link from
`MIGRATING.md`. Run `mise run gate`, commit.

**Verify**: `mise run gate` → exit 0.

## Test plan

Two new tests (Step 3) in `panel.rs`'s existing test module (model after
`panel_recipe_focus_uses_border_focused_not_weight`, `panel.rs:1140`).

## Done criteria

- [x] `mise run check` and `mise run gate` exit 0
- [x] `grep -n "BorderShape" crates/termrock/src/style/tokens.rs` → enum + field + builder
- [x] New tests pass; Square remains the default (`DesignSystem::default().border_shape == BorderShape::Square` asserted in a test)
- [x] `migrations/0262-*.md` exists, linked from `MIGRATING.md`
- [x] No out-of-scope files modified (`git status`)
- [x] `plans/README.md` updated

## Visual QA

- Panel / bordered Surface: **pass** — square-default hierarchy, rhythm,
  layering, focus color, state distinction, and responsive contraction passed
  the designer review; see
  [`artifacts/visual-qa/plan-002/README.md`](../artifacts/visual-qa/plan-002/README.md).

## STOP conditions

- `Block` in the pinned `ratatui-widgets 0.3.2` has no `border_type`/rounded
  support → report; do not hand-roll corner overrides in `Panel`.
- `Surface` turns out not to own any border glyphs (pure fill) → wire only
  `Panel` and note it in the migration.
- Drift vs excerpts.

## Maintenance notes

- Plans 004/005/010 route more widgets through `Surface`; they inherit the
  token automatically — reviewers of those plans should confirm no widget
  reads corner glyphs from anywhere else.
- Future `BorderShape` variants (e.g. `Heavy`) are intentionally excluded:
  weight must never communicate focus or hierarchy (repo law).

## Amendments

- 2026-08-12: Reframed the Surface wiring from hand-selected corner glyphs to
  its live Ratatui `Block` border path. Options were to skip Surface (leaves
  two chrome authorities inconsistent), hand-paint corners (duplicates
  Ratatui), or expose one `DesignSystem::border_set()` resolver consumed by
  Panel and Surface. The shared resolver best matches the goal, Ratatui-first
  law, ASCII fallback, and smallest coherent blast radius.
- 2026-08-12: Added `interactors.rs`, generated API/frame output, and visual-QA
  artifacts to Scope. The live lookbook architecture owns mutable knobs in a
  story interactor, while the original Scope named only knob data and story
  registration. The added files are the minimum path to the required live
  Square/Rounded control and mandatory browser proof.
- 2026-08-12: Widened generated-frame Scope from `panel-focused` to all
  affected generated packs. Live code had no claimed pre-existing ASCII Panel path;
  enforcing the plan's explicit `+/-/|` test correctly updates every story
  composed from Panel/Surface under ASCII capability. Options were to weaken
  the test (contradicts accessibility intent), keep Unicode in ASCII mode
  (violates glyph policy), or accept deterministic generated cascade. The
  cascade is the only cross-surface-consistent resolution and contains no
  additional handwritten widget change. The repository tracks only its
  primary pack subset; the full gate's other reproducible packs remain
  untracked build output.
