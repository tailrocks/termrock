# Plan 052: Complete scalable data, log, and review surfaces

> **Executor instructions**: Build opinionated components on existing kernel
> primitives; never fetch, sort, filter, or select unloaded data implicitly.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/scroll.rs crates/termrock/src/widgets/table.rs crates/termrock/src/widgets/detail_table.rs crates/termrock/src/widgets/log_pane.rs crates/termrock/src/widgets/diff.rs crates/termrock/src/widgets/charts.rs crates/termrock-lookbook docs/design/component-anatomy-spec.md docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 041, 043–045, 048, 050, and 051 are DONE.

## Status

- **Execution**: DONE — migration 0054

- **Priority**: P2
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plans 041, 043–045, 048, 050, and 051
- **Category**: data components, performance, UX, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

btop, k9s, Lazygit, Posting, and Yazi earn trust through dense data surfaces
that stay responsive and legible. TermRock has strong Table/DetailTable/LogPane/
DiffView/chart bones but lacks one ScrollArea truth, opinionated DataTable
chrome, navigable review surfaces, and executable performance contracts.

## Current state

- Viewport/scroll behavior is repeated across widgets.
- Table owns basic widths/sort outcomes but not DataTable toolbar/bulk/pinned
  chrome or the complete binding state matrix.
- DetailTable, LogPane, DiffView are paint/navigation bases without canonical
  ObjectInspector, LogStream follow, or DiffReview hunk contracts.
- Charts need DesignSystem viz/capability/no-color recipes.
- Plan 039 fixes VirtualGrid; Plan 041 supplies streaming anchors; this plan owns
  migration `0045`.

## Scope

**In scope**: ScrollArea evolution of Viewport; harden Table; DataTable;
ObjectInspector evolution of DetailTable; LogStream evolution of LogPane;
DiffReview evolution of DiffView; chart viz recipes; virtualization/performance;
Studio evidence/docs/API/migration `0045`.

**Out of scope**: data fetching, sorting/filter execution, unloaded select-all,
query engines, filesystem/process/log ingestion, application actions, copying
render bodies, compatibility aliases.

## Git workflow

Clean `main`; Conventional Commit; `rtk git commit -s`; Codex co-author. Land
private scroll/perf foundations green, then public breaks with migration/docs.

## Steps

### Step 1: Specify shared scroll/virtualization laws

Reference tests cover empty/zero area, bounds, page/wheel/drag, both axes,
bar auto-hide, follow detach/rejoin, resize anchor, stable IDs, known/unknown
total, disabled rows, no phantom regions, and O(visible) work. One ScrollState/
ScrollArea grammar must serve children without taking a second scroll truth.

### Step 2: Build ScrollArea

Evolve Viewport into controlled/internal ScrollState plus viewport and optional
horizontal/vertical bar geometry/actions. Content size is projected; child
rendering stays caller/component-owned. Bar pointer drag uses current geometry;
ASCII/no-color glyph recipes; tiny area clips without invalid regions.

### Step 3: Harden Table and add DataTable

Table uses composed rows, DesignSystem, universal intents, stable row IDs,
disabled/loading/empty states, identity-column-first narrow collapse, and sort
request outcomes only. DataTable composes toolbar Buttons, stripes, optional
bulk selection/actions, leading-column pin cue, and density presets. Ctrl-A is
`SelectAllRequested`, never silent enumeration of virtual/unloaded rows.

### Step 4: Evolve inspector/log/diff surfaces

ObjectInspector supports flat/nested projected fields and capability actions.
LogStream adds bounded append/follow/detach, structured level recipes, stable
anchor, and hot path. DiffReview adds unified/split responsive mode, hunk
navigation/activation, syntax/diff tokens, horizontal scroll, and selection/
copy outcomes. Consumers own parsing, logs, clipboard, review effects.

### Step 5: Tokenize visualization

Sparkline/BarSeries/SegmentedMeter consume viz tokens, density, glyph capability,
and non-color patterns. Normalize invalid/NaN/infinite values deterministically.
Paint visible cells only and avoid per-frame formatted/fill allocations.

### Step 6: Scale proof, Studio, migration

Benchmark-style allocation tests: 10k Table/DataTable/Log/Diff rows with 40-row
viewport; warmed unchanged frame zero allocations and visible-local work.
Stories cover sort/bulk request, pinned/narrow, follow, hunk review, inspector,
both-axis scroll, loading/empty/error composition, Unicode/ASCII/no-color.

Write `migrations/0045-v0.12.0-data-review-surfaces.md` with removed types,
ScrollArea/state, canonical replacements, exact consumer edits, performance/
ownership, before/after code, commands. Update all artifacts/MIGRATING.

**Verify**: reference/model/component tests; hot-path allocation suites; Studio
check; separate repository check and gate pass.

## Test plan

- Shared scroll/reference model and pointer geometry tests.
- Table/DataTable stable-ID/sort/bulk/disabled/narrow tests.
- Inspector/LogStream/DiffReview state/anchor/action tests.
- Invalid chart data and capability render tests.
- 10k-row allocation/local-work regressions and Studio evidence.

## Done criteria

- [x] One ScrollArea/state grammar serves scalable content.
- [x] Table/DataTable stay consumer-sorted/fetched and stable-ID correct.
- [x] ObjectInspector, LogStream, DiffReview have full binding contracts.
- [x] Charts use viz recipes and non-color/capability cues.
- [x] Hot paths are O(visible) and allocation-gated.
- [x] Migration `0045`, docs/evidence/previews/traces/API fresh; gates pass.

## STOP conditions

- Prerequisites not DONE; non-main/dirty tree; `0045` claimed.
- Component needs hidden full dataset scan, fetching, sorting, parsing, clipboard,
  or effect execution.
- Scroll composition introduces two competing offsets/anchors.
- Performance result depends on wall-clock/flaky terminal timing.
- Any verification fails twice after reasonable correction.

## Maintenance notes

New virtual/data components must use shared scroll/identity/performance contract
harnesses. Plan 053 composes these surfaces without local substitutes.
