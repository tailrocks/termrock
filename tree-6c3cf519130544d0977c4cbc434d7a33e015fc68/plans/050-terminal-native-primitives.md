# Plan 050: Complete terminal-native primitives, content, and feedback

> **Executor instructions**: Implement the binding contracts in
> `docs/design/component-anatomy-spec.md`; rendering alone is not completion.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets crates/termrock/src/style crates/termrock-lookbook docs/design/component-anatomy-spec.md docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 043, 045, and 048 are DONE. Strict ordered execution
> also requires Plan 049 DONE and migration `0042` present.

## Status

- **Execution**: DONE — migration 0052

- **Priority**: P2
- **Effort**: L
- **Risk**: MEDIUM
- **Depends on**: Plans 043, 045, 048, and operationally 049
- **Category**: component library, accessibility, UX, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

A premium design system needs small universal building blocks so applications
compose behavior and chrome instead of drawing ad hoc strings. TermRock lacks a
formal Button family, semantic content hierarchy, and consistent feedback
anatomy. Higher components otherwise keep duplicating activation, priority
reduction, loading, glyph fallback, focus, and recipes.

## Current state

- ActionBar chips are not standalone Button/IconButton contracts.
- Kbd/glyph/separator/spinner behavior is scattered.
- Heading/Paragraph/Surface/Section do not exist as shared primitives.
- Banner/EmptyState/ErrorView/LoadingView/Skeleton/Progress exist but do not all
  implement the binding anatomy/state/design/evidence contract.
- Plans 040/043/045/048 provide scene, recipes, priority parts, and Studio proof.
- This plan owns migration `0043`.

## Scope

**In scope**:

- Button, IconButton, Badge, Tag, Chip, Kbd, Separator, Spinner;
- Heading, Paragraph, Surface, Section, Callout, Alert;
- harden Skeleton, EmptyState, LoadingView, ErrorView, Progress;
- typed recipes/parts/outcomes, scene actions, frame-tick motion, ASCII/no-color,
  docs/contracts/stories/traces/API, migration `0043`.

**Out of scope**: form controls (051), data surfaces (052), product effects,
callback storage, arbitrary child tree, compatibility aliases for renamed
Banner/Progress paths.

## Git workflow

Clean `main` only. Conventional Commit, `rtk git commit -s`, Codex co-author.
Each component-family commit independently green; public rename/removal lands
with migration/docs. Push only after full gate.

## Steps

### Step 1: Generate contract tests from the binding catalog

For every in-scope component, require anatomy/recipe coverage, typed outcome or
explicit none, keyboard/mouse/focus or N/A, disabled/loading/error behavior,
narrow/tiny, grapheme/ASCII/no-color, story IDs, snapshots, interaction tests,
and performance classification. Inventory fails when any required cell is absent.

### Step 2: Implement activation primitives

Button/IconButton share one armed/activation model: Enter/Space and one pointer
gesture activate once; disabled/loading never activate; Press/Repeat/Release are
explicit. Badge is non-interactive. Tag removal and Chip toggle return typed
stable-ID outcomes. Kbd renders Keymap chords. All geometry/available actions
register through InteractionScene and use composed priority parts.

### Step 3: Implement pure terminal primitives

Separator uses glyph/design recipes without redefining focus borders. Spinner is
FrameTick-derived and respects Full/Reduced/Off. Heading/Paragraph use terminal
typography and grapheme-safe wrap. Surface implements documented elevation and
density insets. No primitive owns I/O/time/global theme.

### Step 4: Implement Section, Callout, and Alert

Section composes Surface + Heading + optional description/header actions and a
controlled collapse outcome. Callout/Alert use semantic tones plus non-color
glyphs; dismiss/acknowledge actions are scene-routed. Replace thin Banner paths
where semantics match; do not overload permission prompts.

### Step 5: Harden feedback surfaces

EmptyState/ErrorView optional actions reuse Button and expose stable action
outcomes. LoadingView reuses Spinner. Skeleton motion is deterministic/off-safe.
Progress separates determinate from Spinner-backed indeterminate behavior. All
tiny layouts retain primary meaning/action visibility.

### Step 6: Studio evidence and migration

Implement every story ID from the binding spec plus risk variants: activation
edge, disabled/loading, action/no-action, motion-off, narrow/tiny, Unicode/ASCII,
no-color, density/preset. Contract axes name executed scenarios.

Write `migrations/0043-v0.12.0-terminal-primitives.md` with renamed/removed
surfaces, canonical replacements, exact consumer edits, before/after code,
ownership, commands. Update MIGRATING/docs/API/inventory/previews/traces.

**Verify**: focused component tests; generated completeness; Studio/lookbook
check; warmed O(1)/visible-text allocation checks; separate
`rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## Test plan

- Shared activation law suite for Button/IconButton/Tag/Chip/actions.
- Recipe/render matrices across states, capability, density, and tiny widths.
- FrameTick/motion determinism.
- Grapheme wrapping and ASCII/non-color tests.
- Generated anatomy/evidence completeness and warmed allocation checks.

## Done criteria

- [x] All listed primitives/content/feedback contracts are implemented.
- [x] Interaction uses scene/actions and returns typed neutral outcomes.
- [x] All paint uses DesignSystem recipes and composed anatomy.
- [x] Disabled/loading/tiny/ASCII/no-color/motion behaviors are proven.
- [x] Existing feedback surfaces have one canonical contract; duplicates gone.
- [x] Migration `0043`, docs, evidence, previews/traces/API are fresh.
- [x] Full gates pass.

## STOP conditions

- Prerequisites not DONE; branch not `main`; dirty tree; `0043` claimed.
- Component requires product state/effects, stored callbacks, or global time/theme.
- Binding anatomy spec changed semantically without a corresponding design
  decision; re-plan rather than guessing.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Plans 051–053 must compose these primitives; they may not duplicate button,
surface, loading, empty, or feedback paint/interaction bodies.
