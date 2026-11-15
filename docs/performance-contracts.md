# Performance contracts

Issue #56 makes the cost of a small update an executable contract. A contract
counts the work an operation does. It never measures elapsed time, and this
repository makes no latency claim from it.

## Counters

`junie_tui::metrics` defines `Metric`, one variant per unit of work, and the
macro `record_work!(Metric, amount)`. Production code marks the work site with
the macro.

- Without the `work-metrics` feature the macro expands to an empty block and
  does not evaluate `amount`. A disabled build pays nothing.
- With `work-metrics`, counts live in a thread-local table. A test reads it with
  `metrics::measure(|| ...)` or with `snapshot()` and `WorkSnapshot::delta`.
- A delta is refused when its snapshots come from different threads or
  different `reset` epochs, or when a counter went backwards. A counter that
  would overflow `u64` panics. Nothing wraps or saturates.
- Counts cover the calling thread only. Work on a worker thread or in a child
  process is not in the delta. Collect the worker's own `WorkDelta` and combine
  it with `WorkDelta::merged`.
- Only metrics that have a production work site are listed. A metric that is
  not listed is not measured. That is different from zero.

`jackin-console` forwards its `work-metrics` feature to `junie-tui`.

## What each metric counts

| Metric | Unit | Work site |
| --- | --- | --- |
| `SegmentedLines`, `ReflowedLines`, `IndexRebuilds`, `IndexExtends`, `IndexShifts` | lines or passes | `TextViewport` layout, next to its own `WorkCounters` |
| `PayloadBytesCloned` | bytes | `TextViewport` deep clone on an unprepared `ViewportOps::draw` |
| `AllocatedBufferCells` | cells | every scratch `Buffer`, through `ui::ctx::scratch_buffer` |
| `BufferCellsFilled`, `BufferCellsCopied` | cells | `ScrollView` canvas fill and `copy_cells` |
| `VisibleRowsPainted` | rows | `ListBox`, `DataTable`, `DataGrid` and `TreeView` paint loops, summed over a frame |
| `IntrinsicWidthCalls`, `TextBytesMeasured` | calls, bytes | `ui::text::width` |
| `LabelsNormalized`, `LabelBytesNormalized` | labels, bytes | `TreeView` filter |
| `ProjectionRebuilds`, `RankRowsVisited` | rebuilds, items | Holla Finder rebuild and ranking pass |
| `SchedulingRecordsVisited`, `SchedulingPolls` | records, polls | Jackin daemon traversal and pane polls, Holla activity and plan scheduling |

`TextViewport`, `ScrollView` and the width helper are shared, so every app that
uses them is counted. `VisibleRowsPainted` does not count `TextViewport`,
`CodeEditor` or `TerminalViewport` paint.

## Budgets

Library budgets live in `tests/regressions/work_budgets.rs`. Each test builds a
fixture at 1,000, 10,000 and 100,000 seeded rows or lines (60,000 for
`ScrollView`, whose extent is `u16`) in a fixed 100x30 viewport, warms it,
performs one operation and checks the delta. App scenarios live beside their
apps:

| Test | File |
| --- | --- |
| `work_contract_holla_idle_and_source_update` | `src/bin/holla/work_metrics_tests.rs` |
| `work_contract_tablepro_idle_and_cell_edit` | `src/bin/tablepro/work_metrics_tests.rs` |
| `work_contract_showcase_idle_and_switch` | `src/bin/showcase/work_metrics_tests.rs` |
| `work_contract_jackin_idle_and_roster_update`, `work_contract_jackin_capsule_idle_frame` | `apps/jackin/tests/regressions/work_budgets.rs` |

The app tests check each scenario with `assert_exact`: every listed metric must
equal its measured count and every unlisted metric must be zero. A ceiling
cannot tell a deleted counter from cheap work, so an exact count is the default
for a measured fact, and a counter that stops recording fails its test. The
library tests in `tests/regressions/work_budgets.rs` use ceilings
(`WorkDelta::check_budget`) where a number is a bound. A count that records
today's behaviour of an unfixed path says so beside it. Lowering it when the
owning issue lands is the intended change.

The app tests share `junie_tui::metrics::harness::Harness`, which drives an
`Application` through its real input and render path and writes the report.

### Measured counts

Per operation, one frame includes the draw. Rows painted is summed over every
list, table, grid and tree widget in the frame.

| Scenario | Operation | Records | Counts |
| --- | --- | --- | --- |
| viewport (`TextViewport`) | first `set_area` | 1k / 10k / 100k lines | segmented = n, reflowed = 2n, index rebuilds = 2 |
| viewport | prepared draw | 1k / 10k / 100k | none counted |
| viewport | append one line, then `set_area` | 1k / 10k / 100k | segmented 1, reflowed 1, index extends 1, width bytes 13, independent of n |
| viewport adapter | draw after a push, without `set_area` | 1k / 10k / 100k | payload bytes cloned = whole history (33,013 / 330,013 / 3,300,013) |
| `ScrollView` | render overflowing content | 1,000 / 10,000 / 60,000 rows of width 100 | allocated = filled = 100 x rows; copied 5,842 for every size |
| `ListBox` | settled frame | 1k / 10k / 100k | rows painted 30 |
| `DataTable` | settled frame | 1k / 10k / 100k | rows painted 29, width calls 62, width bytes 108 |
| `TreeView` | settled frame | 1k / 10k / 100k | rows painted 30, width calls 30, width bytes 200 |
| `TreeView` | filter change | 1k / 10k / 100k | labels normalized = n, label bytes 7,890 / 88,890 / 988,890 |
| Holla Finder | settled tick | 106 items, 5 activities, 0 plans | projection rebuilds 1, ranked 106, scheduling 10, width calls 716 |
| Holla Finder | unrelated world field, then tick | same | same as the settled tick |
| Holla Finder | query change | same | projection rebuilds 1, ranked 106, width calls 1,639 |
| TablePro grid | settled tick | 312 rows | rows painted 41, width calls 249 |
| TablePro grid | one Down key | 312 | same as the settled tick |
| TablePro grid | one keyed draft edit | 312 | rows painted 39, width calls 257 |
| TablePro grid | unrelated history entry, then tick | 312 | rows painted 39, width calls 257 |
| Showcase Query workbench | settled tick | 2 documents | rows painted 47, width calls 389 |
| Showcase Query workbench | click the second tab (two frames) | 2 | rows painted 82, width calls 593 |
| Showcase Finder | settled tick | 12 actions | allocated 1,440, rows painted 24, width calls 208 |
| Showcase Finder | other page's option, then tick | 12 | same as the settled tick |
| Jackin roster | settled tick | 9 instances, 4 workspaces, 2 daemons | scheduling records 2, polls 6, width calls 178 |
| Jackin roster | unrelated workspace policy, then tick | same | same as the settled tick |
| Jackin roster | workspace rename, then tick | same | width bytes 1,571 instead of 1,531 |
| Jackin capsule | settled tick | 2 daemons | scheduling records 2, polls 6, width calls 158, width bytes 1,476; segmented 0, reflowed 0, index rebuilds 0, payload bytes cloned 0 |
| Jackin capsule | one typed key, then the frame | same | width calls 146, width bytes 1,416; segmented 0, reflowed 0, index rebuilds 0, payload bytes cloned 0 |

### What these counts say

- Prepared `TextViewport` draws, and `ListBox`, `DataTable` and `TreeView`
  frames, do work that does not depend on the number of records.
- `ScrollView::render` allocates and fills a canvas as large as its content
  (#20). Only the copy back to the screen is bounded by the window.
- Drawing a `TextViewport` through a shared reference after a change, without
  preparing it first, clones the whole history.
- A Holla Finder tick rebuilds and re-ranks the whole catalog whether or not
  anything relevant changed (#62). Holla Finder has no source revision yet, so
  a relevant source revision change has no scenario here.
- The Jackin capsule primes every pane's viewport after each tick and input
  (`CapsuleScreen::sync_all`), so its idle frame draws prepared views. Before
  #249 the sync skipped script panes, and every idle frame cloned 80,059
  payload bytes and segmented 2,021 lines.

### Not covered

These are listed in the PR as unmet acceptance items: the `performance` nextest
profile and its required-suite inventory gate, ignored release measurements
and their elapsed-time report schema, terminal output, raw-event and
simulated-transport budgets, `Picker`, `Select`, `Completion`, `CodeEditor`,
`TextArea`, `ScrollPanel` and `TerminalView` families, and the
`SourceRowsRead`, `IdentityComparisons` and `AnnotationQueries` family of
metrics.

Also unmet, and absent from every scenario above:

- No test shows that a background tab or page projection that is irrelevant to
  a change does not rebuild. The idle and unrelated-change scenarios measure
  the focused page only.
- Jackin has no account-update scenario.
- Holla's batch summary loop and `tick_plan` are not counted. A Holla
  scheduling count covers the activity and plan traversal only, so a zero or a
  steady count there says nothing about batches or the work inside a plan.

## Reports

Each budget test also writes a count-only JSON report to
`<target dir>/performance/<scenario>-<operation>-<records>.json`. A report
lists every metric, zeros included, and states `instrumentation_enabled`, the
calling-thread scope, `"timing": "not measured"` and the correctness oracle the
test checked. It has no timing fields. A report is written to a temp file in
the same directory and renamed into place, so concurrent writers of one
scenario never leave a truncated file.

## Commands

```sh
cargo nextest run -p junie-tui --features work-metrics --test regressions \
    --no-tests fail -E 'test(/^work_budgets::/)'
cargo nextest run -p junie-tui --features work-metrics --bin holla --bin tablepro --bin showcase \
    --no-tests fail -E 'test(/^work_metrics_tests::/)'
cargo nextest run -p jackin-console --features demo,work-metrics --test regressions \
    --no-tests fail -E 'test(/^work_budgets::/)'
cargo nextest run -p junie-tui --features work-metrics --lib --no-tests fail -E 'test(/metrics/)'
```

CI builds both Rust jobs with `--all-features`, so these tests already run in
the existing `Rust / junie-tui` and `Rust / jackin-console` jobs. The product
shape builds without `work-metrics`, where the tests compile away and
`metrics_disabled_does_not_evaluate_arguments` runs instead.
