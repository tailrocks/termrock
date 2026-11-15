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
| `PayloadBytesCloned` | bytes | `TextViewport` deep clone on an unprepared `ViewportOps::draw`; `Picker` item copies; `Select` and `Completion` overlay copies; `CodeEditor` per-frame text copy; every undo snapshot and edit-entry snapshot of `CodeEditor` and `TextArea` |
| `AllocatedBufferCells` | cells | every scratch `Buffer`, through `ui::ctx::scratch_buffer` |
| `BufferCellsFilled`, `BufferCellsCopied` | cells | `ScrollView` canvas fill and `copy_cells` |
| `VisibleRowsPainted` | rows | `ListBox`, `DataTable`, `DataGrid`, `TreeView`, `OptionList` (the rows of `Picker`, `Select` and `Completion`), `CodeEditor` and `TextArea` paint loops, summed over a frame |
| `IntrinsicWidthCalls`, `TextBytesMeasured` | calls, bytes | `ui::text::width` |
| `FitVisits`, `FitCompares` | visits, comparisons | Responsive fitters (`ChipBar`, `Breadcrumb`, `segments`, `StatusBar`, `HintBar`, `Toolbar`): loop visits and removal-order sort comparisons |
| `LabelsNormalized`, `LabelBytesNormalized` | labels, bytes | `TreeView` filter |
| `ProjectionRebuilds`, `RankRowsVisited` | rebuilds, items | Holla Finder rebuild and ranking pass |
| `SchedulingRecordsVisited`, `SchedulingPolls` | records, polls | Jackin daemon traversal and pane polls, Holla activity and plan scheduling |
| `TextBytesHashed` | bytes | `CodeEditor` change-detection hash, once per frame |
| `AnnotationQueries` | calls | `CodeEditor` highlighter (on a changed hash) and block segmenter (every frame) |
| `IdentityComparisons` | comparisons | `Picker::set_visible_items` key lookup and cursor re-find, `Picker::replace_items` cursor re-find |

`TextViewport`, `ScrollView` and the width helper are shared, so every app that
uses them is counted. `VisibleRowsPainted` does not count `TextViewport` or
`TerminalViewport` paint. `PayloadBytesCloned` counts text bytes only: a
`Select` or `Completion` overlay also clones its `OptionList`, and the count
does not include that. `IdentityComparisons` counts only the `Picker` lookups
above; other key scans (`Picker` `retain`, `TreeView`, `ViewportOps`) are not
counted.

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
`Picker`, `Select`, `Completion`, `CodeEditor` and `TextArea` tests in
`tests/regressions/work_budgets.rs` use `assert_exact` too. The older viewport,
list, table, tree and scroll tests there use ceilings
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
| `Picker` | `set_items` | 1k / 10k / 100k items of 17 text bytes | none cloned |
| `Picker` | settled frame | 1k / 10k / 100k | rows painted 12, width calls 3n + 264, width bytes 17n + 264 |
| `Picker` | `set_visible_items` of 20 of n items | 1k / 10k / 100k | nothing cloned, no identity comparison |
| `Picker` | frame over those 20 items | 1k / 10k / 100k | rows painted 12, width calls 324, width bytes 604, independent of n |
| `Picker` | `replace_items` | 1k / 10k / 100k | nothing cloned, no identity comparison |
| `Select` | closed frame | 1k / 10k / 100k options | width calls 1, width bytes 6 |
| `Select` | open frame | 1k / 10k / 100k | payload bytes cloned 104, rows painted 8, width calls 1, independent of n |
| `Completion` | open frame | 1k / 10k / 100k items of 15 text bytes | payload bytes cloned 120, rows painted 8, width calls 240, width bytes 360, independent of n |
| `CodeEditor` | first focused frame | 1k / 10k / 100k lines | nothing cloned or hashed, annotation queries 2, rows painted 29, width calls 30, width bytes 67 / 68 / 69 |
| `CodeEditor` | settled frame | same | rows painted 29, width calls 30, no annotation query |
| `CodeEditor` | first keystroke, then second of the run | same | payload bytes cloned L, then none |
| `CodeEditor` | frame after the edit | same | annotation queries 2, rows painted 29, nothing cloned or hashed |
| `TextArea` | first focused frame | 1k / 10k / 100k lines | payload bytes cloned L, rows painted 10, width calls 3 (L is 32n - 1) |
| `TextArea` | settled frame | same | no clone, rows painted 10, width calls 3 |
| `TextArea` | first keystroke, then second of the run | same | payload bytes cloned L, then none |
| Holla Finder | settled tick | 106 items, 5 activities, 0 plans | no projection rebuild, scheduling 10, width calls 706 |
| Holla Finder | unrelated world field, then tick | same | same as the settled tick |
| Holla Finder | query change | same | projection rebuilds 1, ranked 106, width calls 1,621 |
| TablePro grid | settled tick | 312 rows | rows painted 41, width calls 233 |
| TablePro grid | one Down key | 312 | same as the settled tick |
| TablePro grid | one keyed draft edit | 312 | rows painted 39, width calls 268 |
| TablePro grid | unrelated history entry, then tick | 312 | rows painted 39, width calls 268 |
| Showcase Query workbench | settled tick | 2 documents | rows painted 54, width calls 222 |
| Showcase Query workbench | click the second tab (two frames) | 2 | rows painted 91, width calls 382 |
| Showcase Finder | settled tick | 12 actions | allocated 0, rows painted 21, width calls 422 |
| Showcase Finder | other page's option, then tick | 12 | same as the settled tick |
| Jackin roster | settled tick | 9 instances, 4 workspaces, 2 daemons | scheduling records 2, polls 6, width calls 205 |
| Jackin roster | unrelated workspace policy, then tick | same | same as the settled tick |
| Jackin roster | workspace rename, then tick | same | width bytes 1,498 instead of 1,466 |
| Jackin capsule | settled tick | 2 daemons | scheduling records 2, polls 6, width calls 109, width bytes 786; segmented 0, reflowed 0, index rebuilds 0, payload bytes cloned 0 |
| Jackin capsule | one typed key, then the frame | same | width calls 97, width bytes 726; segmented 0, reflowed 0, index rebuilds 0, payload bytes cloned 0 |

### What these counts say

- Prepared `TextViewport` draws, and `ListBox`, `DataTable` and `TreeView`
  frames, do work that does not depend on the number of records.
- `ScrollView::render` allocates and fills a canvas as large as its content
  (#20). Only the copy back to the screen is bounded by the window.
- Drawing a `TextViewport` through a shared reference after a change, without
  preparing it first, clones the whole history.
- A settled Holla Finder tick rebuilds and re-ranks nothing: each projection
  layer rebuilds only for its own dependency (#62). A query change still
  re-ranks the whole catalog. A relevant source revision change has no
  scenario here.
- The Holla Finder counts are the product path only. In test builds, a settled
  `FinderPage::ensure_projection` also rebuilds a reference page and compares
  it (`assert_matches_reference`), which measures 26 width calls and bytes per
  run (the 26 `clip` calls of the custom-item digests in the catalog build).
  That oracle runs under `projection::uncounted`, which suspends the library
  counters as well, so none of it lands in these rows. Before this, the
  counters included it: 52 calls in a settled tick (two oracle runs) and 26 in
  the query-change frame, which became 52 once the Finder's `on_key` also ran
  `ensure_projection` (#283). That, not plan rows (the fixture has none), was
  the +26 of the 1,647 to 1,673 re-baseline. The rows above are 758 - 52 = 706
  and 1,673 - 52 = 1,621.
- The Jackin capsule primes every pane's viewport after each tick and input
  (`CapsuleScreen::sync_all`), so its idle frame draws prepared views. Before
  #249 the sync skipped script panes, and every idle frame cloned 80,059
  payload bytes and segmented 2,021 lines.
- A settled `Picker` frame measures the width of every item, so it grows with
  the hidden list. A frame over a 20-item filtered list does not.
  `Picker::set_visible_items` and `replace_items` resolve keys through the key
  index and clone nothing.
- An open `Select` or `Completion` frame clones only the painted rows into its
  overlay snapshot, and `Completion` sizes its popup from cached widths.
- A `CodeEditor` frame reads cached text and annotation results, so a settled
  frame copies, hashes and segments nothing. A `TextArea` frame measures only
  its visible lines. The first keystroke of an undo run copies the whole text
  in both. Later keystrokes of the run do not.

### Not covered

These are listed in the PR as unmet acceptance items: the `performance` nextest
profile and its required-suite inventory gate, ignored release measurements
and their elapsed-time report schema, terminal output, raw-event and
simulated-transport budgets, `ScrollPanel` and `TerminalView` families, and
the `SourceRowsRead` metric. `CodeEditor` bracket matching, `Picker`
`retain` scans and the `CodeEditor` find state are not counted.

Also unmet, and absent from every scenario above:

- No test shows that a background tab or page projection that is irrelevant to
  a change does not rebuild. The idle and unrelated-change scenarios measure
  the focused page only.
- Jackin has no account-update scenario.
- Holla's batch summary loop and `tick_plan` are not counted. A Holla
  scheduling count covers the activity and plan traversal only, so a zero or a
  steady count there says nothing about batches or the work inside a plan.
- `Tabs` paint work scales with the drawn tabs, not the item count. Painting a
  drawn tab re-measures its label, calls `close_button.width` once, and
  measures its prefix and suffix when set. `tabs_render_measures_each_tab_once`
  pins the plain-label case.

## Fitting work

`FitVisits` and `FitCompares` count the responsive fitters' loop visits and
removal-order comparisons. Together with `IntrinsicWidthCalls` they show that a
fit measures each label once and sorts once instead of rescanning survivors.
The Showcase scenarios `work_contract_stress_*` set each `Stress data` option
(Toolbars, Breadcrumbs, Chrome, Chips) to 20,000 items and to one 65,536-cell
label, and assert exact counts for the frame that applies the option and the
next settled frame. A fitter that goes quadratic changes these counts and
fails the scenario.

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
