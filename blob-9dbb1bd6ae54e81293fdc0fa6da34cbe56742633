# Comparison and verdicts

## Purpose

Flow W2 end to end: render each jackin-used widget at the Old rev (Side
harness) and at HEAD through the same rasterizer, publish per-widget
comparison reports, collect the user's per-component verdicts as dated item
Decisions, and apply merge/restore verdicts as termrock design changes.
Anchors: F3, F4, F8, W2, B1, D1, D3, D8, D10, D12 · Evidence: research/tui-png-baselines/03-termrock-seams-and-old-rev.md

## Requirements

### Requirement: Side harness renders the Old rev
A standalone cargo project `tools/oldrev-harness/` (in-repo, NOT a workspace
member — the workspace build must never compile the Old rev) SHALL depend on
termrock pinned by git rev `5ff94ee117fd4a1b72fdd0d1b1847815055a93ac` and on
`termrock-raster` by path, constructing each jackin-used widget through the
Old rev's public constructors (all public at the pin — ch. 03 Q4 sampled 13
families) and rendering PNGs with the identical cell geometry and fonts.
States with a HEAD story but no Old-rev construction path SHALL be emitted
into the report as `uncomparable`, never skipped (W2 failure point a; only
25 of the subset's 87 HEAD stories have Old-rev story counterparts).
Covers: F3, D3, D10, W2 · Evidence: ch. 03 Q3, Q4, Q6

#### Scenario: Old rev builds and renders
- **GIVEN** the harness with the pinned git dependency
- **WHEN** it runs
- **THEN** it emits one PNG per comparable widget state at the Old rev (the pin built clean on 2026-08-16 — assumption A2)

#### Scenario: Uncomparable state surfaces
- **GIVEN** `text-input/basic`, which has no Old-rev counterpart (ch. 03 Q6)
- **WHEN** the harness cannot construct an equivalent Old-rev state
- **THEN** the comparison report lists it under `uncomparable` with the reason

### Requirement: Per-widget comparison reports
For each of the 16 subset families, a report
`roadmap/jackin-termrock-parity/comparisons/<widget>.md` SHALL present
Old-rev and HEAD PNGs side by side per state (images committed next to the
report), with every visible difference named and classified
`palette-level` (global theme drift) or `widget-level` (behavior/structure)
— W2 failure point b — and a verdict slot per widget: `merge` (expected
default per D12), `restore`, or `accept`, empty until the user rules.
Covers: F3, F8, D8, W2 · Evidence: ch. 03 Q6 (state coverage per family), ch. 03 open unknown (palette drift expected large)

#### Scenario: Report separates drift classes
- **GIVEN** an Old-vs-HEAD pair differing in border color (theme) and in gutter glyph (widget)
- **WHEN** the report is written
- **THEN** the border difference is listed palette-level and the glyph difference widget-level, each named

#### Scenario: One subagent per widget verification
- **WHEN** reports are produced
- **THEN** each widget's comparison is produced by its own subagent run (F3), and the report records which states it covered

### Requirement: Verdict recording and application
Each user verdict SHALL be recorded as a dated Decision in the roadmap item
(D8) before application. `merge` verdicts SHALL apply the jackin-era visual
base with the current widget's improvements (hover states, interaction
refinements, new state coverage) kept on top — never discarded (D12);
`restore` applies the Old-rev look; `accept` records the divergence.
Applications are termrock design changes that re-bless the affected PNG
baselines in the same commit, keeping N1 (zero unreviewed divergence)
checkable: after all verdicts are applied, no subset baseline may differ
from its verdict's recorded outcome.
Covers: F4, B1, D1, D11, D12, N1, W2 · Evidence: item §Decisions; ch. 05 Q5 (bless mechanics)

#### Scenario: Merge keeps the improvement
- **GIVEN** a verdict `merge` on a widget whose HEAD version added a hover state absent at the Old rev
- **WHEN** the verdict is applied
- **THEN** the widget renders the jackin-era base look
- **AND** the hover state remains functional and gains a baseline

#### Scenario: No application without a recorded verdict
- **GIVEN** a widget whose comparison report's verdict slot is empty
- **WHEN** an executor reaches the application step
- **THEN** it stops and reports that user verdicts are pending — it never invents one (D1: the user decides)
