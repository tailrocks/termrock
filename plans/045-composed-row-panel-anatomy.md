# Plan 045: Add priority-aware row and panel anatomy

> **Executor instructions**: Execute sequentially. Replace string-shaped chrome
> coherently; do not add richer parts beside unchanged legacy paint paths.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/panel.rs crates/termrock/src/widgets/tree.rs crates/termrock/src/widgets/table.rs crates/termrock/src/style crates/termrock-lookbook docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 041 and 043 are DONE and the full gate is green.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MEDIUM
- **Depends on**: Plans 041 and 043
- **Category**: component API, responsive UX, Unicode, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

Production terminal rows need leading status/icon, primary and secondary text,
badge, shortcut, trailing value, and live status. Narrow terminals must drop
low-priority parts before truncating the identity-bearing primary label.
`ListRow { label, trailing }` and title-only Panel force every consumer to
rebuild this anatomy, spacing, clipping, theme, and hit geometry.

## Current state

- ListRow exposes one main Line and optional trailing Line.
- Tree/Table/dialog/agent chrome solve similar leading/trailing layout locally.
- Panel owns border/focus semantics but has limited title/chrome slots.
- Shared display-column truncation must be grapheme-safe before parts can be
  reduced reliably; Plan 041 supplies that foundation.
- Plan 043 provides row/panel recipes; this plan adds composition atop them.
- This plan owns migration `0038`.

## Target contract

Provide borrowed, semantic parts rather than fixed strings:

- `RowPart`: role (leading/primary/secondary/badge/shortcut/value/status), Line,
  collapse priority, minimum width, optional action identity;
- `ComposedRow<Id>`: stable ID, borrowed parts, enabled/selected semantics;
- `PanelSlots`: title, subtitle, leading status, trailing actions/badge, footer
  hint/status, each with priority and semantic role;
- deterministic one-row/two-row reduction metadata and exact part hit regions.

The layout solver reserves mandatory parts, drops optional parts by explicit
priority, then grapheme-truncates primary content. It never hides the current
focus/selection cue or paints outside its area.

## Scope

**In scope**: generic part model/solver; List/Tree/Table row adapters; Panel
slots; token recipe integration; part hit/actions through InteractionScene;
tiny-area/Unicode tests; lookbook/docs/contracts/API; migration `0038`.

**Out of scope**: product-specific row kinds, arbitrary nested widgets,
multi-line rich transcript blocks, retained layout tree, compatibility structs,
consumer action effects.

## Git workflow

Clean `main` only. Conventional Commit, `rtk git commit -s`, Codex co-author
trailer. Each commit green; push only after `rtk proxy mise run gate`.

## Steps

### Step 1: Lock reduction invariants

Build fixture parts and exhaust widths `0..=80`. Assert containment, stable
order, no overlap, mandatory focus cue preservation, secondary/badge/shortcut
drop before primary truncation according to priorities, grapheme integrity,
trailing alignment, disabled action exclusion, deterministic duplicate-priority
tie break, and hit regions exactly matching painted surviving parts.

### Step 2: Implement one allocation-conscious part solver

Measure parts once by display width. Resolve mandatory widths and inter-part
gaps from DesignSystem density. Drop optional parts from lowest survival
priority; then truncate only parts that declare truncation. Return borrowed
placements/reduction reasons using caller scratch or a small reusable state
buffer. Warm unchanged row layout must allocate zero.

Reject impossible definitions (no primary, duplicate action IDs, min > max)
with typed validation in debug/tests and deterministic safe rendering in release.

### Step 3: Expose composed rows

Replace ListRow's label/trailing contract with ComposedRow. Add zero-copy
adapters for simple primary-only rows, but do not retain the old public struct.
Reuse the same anatomy in Tree and Table row/header surfaces where roles fit.
Register surviving actionable parts with InteractionScene; hidden parts are not
discoverable or focusable.

### Step 4: Expand Panel slots without breaking focus law

Panel lays out title/subtitle/leading/trailing/footer slots inside one-line
border geometry. `Role::BorderFocused` remains the only focused border signal;
actions use explicit part cues. Tiny width keeps focus/border and primary title,
then drops lower-priority chrome. Footer must not redefine scrollbar/border.

### Step 5: Catalog the anatomy

Stories cover full row/panel, progressive widths, custom drop priority,
focused/disabled/action parts, compact/cozy, Unicode/ASCII, no-color, long CJK/
emoji, and pointer action traces. Contract evidence references those scenarios.

### Step 6: Migrate and gate

Write `migrations/0038-v0.12.0-composed-anatomy.md` with removed row/panel
surface, exact construction edits, priority semantics, before/after examples,
ownership, commands. Update docs/contracts/previews/API/MIGRATING.

**Verify**: focused row/panel tests; allocation test; lookbook check; then
`rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## Test plan

- Exhaustive width solver tests and seeded part combinations.
- Grapheme/ASCII/no-color buffer tests.
- InteractionScene part availability/hit tests.
- List/Tree/Table/Panel adapter tests.
- Warmed zero-allocation layout test and deterministic story traces.

## Done criteria

- [ ] Rows/panels compose semantic borrowed parts with explicit priorities.
- [ ] Narrow reduction preserves primary identity/focus before optional chrome.
- [ ] Geometry is contained, grapheme-safe, deterministic, and hit-test exact.
- [ ] Hidden/disabled parts cannot advertise or trigger actions.
- [ ] List/Tree/Table/Panel reuse one anatomy/recipe grammar.
- [ ] Migration `0038`, docs, contracts, stories, previews, inventory fresh.
- [ ] Old string-shaped public structs/paths removed; full gates pass.

## STOP conditions

- Plan 041 or 043 not DONE; branch not `main`; dirty tree; `0038` claimed.
- Proposed part needs product domain semantics or arbitrary nested callbacks.
- Solver cannot preserve primary/focus cue inside non-empty available area.
- Unicode foundation from Plan 041 is absent/regressed.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Prefer new semantic part roles only when reuse across at least two components is
clear. Plan 046 builds workbench rows/cards from this anatomy.
