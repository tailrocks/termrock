# Parity inventory

## Purpose

The evidence documents proving termrock can replace every termrock API and
custom TUI component jackin uses: a complete inventory (F2), an old→new API
parity map (F1), and a building-block-law classification of jackin's ~40
custom components with a promotion backlog for generic gaps (F7, D7).
Anchors: F1, F2, F7, D7 · Evidence: research/tui-png-baselines/03-termrock-seams-and-old-rev.md; roadmap item §References "Looked-up facts"

## Requirements

### Requirement: Jackin usage inventory
A document `roadmap/jackin-termrock-parity/parity/inventory.md` SHALL list
every termrock API jackin references (the 1067 references across 137 files:
modules `widgets`, `scroll`, `style`, `layout`, `keymap`, `input`,
`interaction`, `osc`, `text`, `ansi_text`, plus `Theme::default()` sites)
and every jackin-owned custom TUI component (the `Widget` impls and
function-style components inventoried in the item's Looked-up facts), each
with jackin `file:line` evidence.
Covers: F2 · Evidence: item §References Looked-up facts (jackin scout, 2026-08-16)

#### Scenario: Inventory is complete against a recount
- **GIVEN** the finished inventory
- **WHEN** `rg -c 'termrock::'` over jackin's crates is re-run and module names are extracted
- **THEN** no module or public-type family appears in the recount that is absent from the inventory

### Requirement: API parity map old-to-new
A document `roadmap/jackin-termrock-parity/parity/api-map.md` SHALL map
every inventoried old-rev API to its current-HEAD equivalent (e.g.
`termrock::Theme` → `style::RolePalette`, per
`migrations/0060-v0.13.0-root-reexport-purge.md` and the rename bound in
ch. 03 Q3), citing the migration file or current `file:line` for each; any
API with no current equivalent SHALL be flagged `GAP` with the missing
capability named. Every jackin-used widget family existing at the Old rev
under today's names (ch. 03 Q3) SHALL be confirmed against HEAD exports.
Covers: F1 · Evidence: ch. 03 Q3; MIGRATING.md; migrations/ (326 files)

#### Scenario: No unmapped API remains
- **WHEN** the map is complete
- **THEN** every inventory row is mapped or flagged GAP — no row is blank

#### Scenario: A GAP becomes work, not silence
- **GIVEN** an API flagged GAP whose capability is generic
- **WHEN** the map is finalized
- **THEN** the GAP appears in the promotion backlog with a proposed widget/module home

### Requirement: Custom-component classification and promotion backlog
A document `roadmap/jackin-termrock-parity/parity/classification.md` SHALL
classify every jackin-owned custom TUI component through the
building-block-vs-example-composite checklist
(`docs/design/building-block-vs-example-composite.md`; CLAUDE.md law):
verdict `generic building block` (promotion candidate), `example composite`
(patterns candidate), or `product-specific` (stays in jackin — e.g. digital
rain, BrandHeader per D7). Generic verdicts SHALL form a promotion backlog
naming each proposed termrock widget, its home (`widgets`/kernel module),
and the jackin evidence. Promotions themselves are follow-on implementation
slices; the classification document is the deliverable this capability owns.
Covers: F7, D7 · Evidence: item §References Looked-up facts (custom component list); CLAUDE.md building-block law

#### Scenario: Every custom component has exactly one verdict
- **WHEN** classification.md is complete
- **THEN** each of the inventoried custom components appears once with verdict + checklist rationale + evidence, and the three verdict sets partition the list

#### Scenario: Brand-specific stays put
- **GIVEN** the digital-rain animation and BrandHeader
- **WHEN** classified
- **THEN** both carry `product-specific` (D7 names them), with the checklist trace showing why
