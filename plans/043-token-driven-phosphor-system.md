# Plan 043: Make design tokens drive a quiet phosphor hierarchy

> **Executor instructions**: Execute sequentially. This plan removes Theme-only
> paint paths for its component families; do not retain dual canonical APIs.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- AGENTS.md README.md crates/termrock/src/style crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/panel.rs crates/termrock/src/widgets/tree.rs crates/termrock/src/widgets/table.rs crates/termrock-lookbook docs/design docs/api docs/content/docs migrations MIGRATING.md`
>
> Compare drift with "Current state." Start only after Plan 040 is DONE and
> the full gate is green.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plan 040
- **Category**: design system, UX, accessibility, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

TermRock defines Density, MotionPreference, GlyphSet, SelectionChrome, and a
list-row recipe, but public paint still mostly consumes Theme directly. In the
default palette, surface hierarchy is quiet because several backgrounds are
empty, while Selection/Focus/Accent/BorderFocused often converge on phosphor
green. Tokens are aspirational and intent becomes visual noise.

The target is “quiet canvas, bright intent”: subtle structural elevation,
calm selection, rare phosphor emphasis for current intent/live action, and
non-color cues that survive capability reduction.

## Current state

- `style/tokens.rs` contains good primitives but only List has an initial recipe.
- `docs/design/terminal-design-system.md` is the accepted target specification:
  complete cell-native color/type/space/chrome/glyph/motion/breakpoint/viz/
  syntax/dimension tokens, recipes, presets, patches, and capability ladder.
- List still has paint branches that hard-code role/gutter behavior outside one
  universally consumed recipe.
- Panel owns correct single-line focus border geometry but lacks a token recipe
  for surface/chrome/spacing.
- Tree and Table independently paint row selection; they must use the same row
  grammar without sharing widget bodies.
- `Theme::quantized`/Appearance and progressive capability docs landed in 0030–
  0031. `docs/design/terminal-design-system.md` now makes truecolor the design
  target and 256/ANSI/monochrome first-class deterministic projections.
- Phosphor remains mandatory default; full re-theming remains mandatory.
- This plan owns migration `0036`.

## Scope

**In scope**: the complete terminal-native `DesignSystem` taxonomy specified in
`docs/design/terminal-design-system.md`; named presets/patches/capability
projection; canonical borrowed render context for List, Tree, Table, and Panel;
row/panel recipes; phosphor semantic-role adjustments; validators; stories/
contracts/API inventory; migration `0036`.

**Out of scope**: all-widget migration, motion choreography, CSS/cascade,
component anatomy (Plan 045), Studio inspector (Plan 048), media (Plan 049),
product branding, terminal probing effects.

## Git workflow

- Work on clean `main`; STOP otherwise.
- Conventional Commit, `rtk git commit -s`, and
  `Co-authored-by: Codex <codex@openai.com>`.
- Every commit passes `rtk proxy mise run check`; push only after `rtk proxy mise run gate`.
- Migration/docs/catalog land with the breaking public export.

## Steps

### Step 1: Reconcile capability policy and lock visual invariants

Make the accepted progressive policy consistent across AGENTS/README/design
docs: truecolor is authored target; 256/ANSI/monochrome are first-class
projections. Detection remains caller-owned; widgets consume resolved
capability. Treat a contradictory binding document as STOP.

Then add failing tests asserting:

- default phosphor Selection is visually distinct from Accent/BorderFocused;
- Canvas, Surface, and Elevated remain distinguishable at intended tiers;
- focused Panel uses `Role::BorderFocused` with the same single-line glyphs;
- row selection remains identifiable in no-color/ASCII mode;
- compact/cozy density changes geometry, not meaning;
- Fill/Gutter/Marker selection chrome changes actual cells painted;
- disabled + selected + focused combinations stay distinguishable.

### Step 2: Make one canonical design input

Implement the documented `DesignSystem` root containing resolved capability,
appearance, density, motion, glyph catalog, cell spacing, breakpoints,
terminal typography, color/chrome/dimension/viz/syntax tokens, and RecipeBook.
Named presets are complete; partial patches are surgical; capability projection
runs last. No ambient/global system. Define typed recipe inputs/outputs:

- `RowRecipeInput { focused, selected, enabled, hovered, depth }`;
- `RowRecipe { base, primary, secondary, marker, leading, trailing, padding }`;
- `PanelRecipeInput { focused, elevated, disabled, danger }`;
- `PanelRecipe { border, surface, title, chrome, padding }`.

Recipe lookup must allocate nothing and must not inspect product data. Remove
Theme-only paint entry points for migrated families; no deprecated aliases.
If an internal Theme projection is temporarily needed while migrating families,
keep it non-public and remove it before the breaking commit/gate.

### Step 3: Tune phosphor through semantic roles

Adjust the default theme only through documented semantic roles:

- Canvas near-black/terminal base;
- Surface subtle lift; Elevated clearer lift for dialogs;
- Selection calm tint/gutter/marker rather than universal bright fill;
- Accent/BorderFocused preserve rare phosphor emphasis;
- Success/Warning/Danger remain distinct and have glyph/text cues.

Validate all resolved color tiers and no-color mode. Avoid hard-coded RGB inside
widgets. Preserve user theme override completeness.

Implement Phosphor Obsidian, Phosphor Day, Slate, and high-contrast presets from
the design specification. Test partial patch isolation, density monotonicity,
glyph fallback completeness, motion-off determinism, and capability projection
of every color token. Wire viz/syntax tokens into their existing consumers only
where doing so removes raw palette literals without expanding component scope.

### Step 4: Migrate List, Tree, Table, and Panel

Route every cell of these component families through row/panel recipes. Keep
behavior and stable-ID outcomes intact. Use one shared recipe grammar, not
copied paint bodies. Density affects spacing/row height only where the widget
can honor it without hiding the focused element. GlyphSet owns markers/tree
connectors/sort cues with ASCII fallbacks.

Add an inventory test that rejects a migrated component exposing a Theme-only
public path or bypassing its declared recipe.

### Step 5: Prove the system in lookbook

Add deterministic stories at 20/40/80/120 columns for default/alternate theme,
compact/cozy, Fill/Gutter/Marker, Unicode/ASCII, truecolor/resolved lower tier,
focused/disabled/danger. Include Panel-over-canvas/elevated-dialog comparisons.
Contract axes must reference story IDs, not bare coverage booleans.

### Step 6: Migrate and gate

Write `migrations/0036-v0.12.0-token-driven-phosphor.md`: removed Theme-only
surface, canonical design input, exact consumer changes, override recipes,
capability policy, before/after examples, commands. Update all generated docs,
contracts, previews, inventory, and `MIGRATING.md`.

**Verify**:

- focused style/list/tree/table/panel tests pass;
- lookbook check passes;
- warmed recipe/render tests allocate zero;
- `rtk proxy mise run check` and `rtk proxy mise run gate` exit 0.

## Test plan

- Pure recipe/validator tests across all state combinations and tiers.
- Buffer-cell assertions for selection chrome and surface hierarchy.
- Narrow/ASCII/no-color render tests.
- Generated API/contract/story completeness tests.
- Warmed allocation regression.

## Done criteria

- [ ] Complete documented DesignSystem taxonomy/presets/patches are implemented.
- [ ] One canonical design input drives List, Tree, Table, and Panel.
- [ ] Phosphor is rare intent emphasis, not universal selection paint.
- [ ] Surface elevation, focus, selection, disabled, and danger remain clear.
- [ ] Density, glyph, selection chrome, and capability change real output.
- [ ] Panel border law and re-themeability hold.
- [ ] Migration `0036`, docs, contracts, stories, previews, inventory are fresh.
- [ ] No Theme-only compatibility path remains for migrated families.
- [ ] Full gates pass.

## STOP conditions

- Plan 040 not DONE; branch not `main`; dirty tree; migration `0036` claimed.
- A binding document contradicts the accepted progressive capability policy.
- Proposed defaults violate phosphor, re-themeability, Panel-border, or
  non-color-cue rules.
- Recipe design needs product domain data or retained style tree.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Plan 045 builds richer row/panel anatomy on these recipes. Plan 048 extends the
same canonical system and executable evidence to the full catalog.
