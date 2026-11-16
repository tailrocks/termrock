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

### Terminal output boundary

`TerminalSession::terminal()` returns a `SessionTerminal`. In a default build
that is `ratatui::DefaultTerminal`, exactly as before, and nothing is counted.
With `work-metrics` it is a `ratatui::Terminal` over a `MeteredBackend<Stdout>`
that the crate does not export. The backend wraps `ratatui`'s crossterm backend
over a counting writer, so the runtime owns the one place where the diff's
output and the final bytes can be counted, and it draws and writes exactly what
`CrosstermBackend` does (`metered_backend_writes_the_bytes_crossterm_writes`).
Limits that are not hidden:

- `DiffCellsVisited` is the number of cells the diff handed to the backend, not
  the number of cells it compared. `ratatui` compares the two buffers inside
  its own `Terminal::draw`, and that comparison is not counted.
- The synchronized-update markers that `draw_frame` writes around `paint` are
  counted in `TerminalBytesWritten` by a real session but not by the contract,
  which calls `paint` directly.
- `SourceRowsRead` is counted only at the sites above. Other widgets that read
  row payloads (list, table, tree) are not in it.
- A terminal that is not a `TerminalSession` (a `TestBackend`, a third-party
  backend) is not metered: its diff and bytes are unavailable, not zero.

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
| `AnnotationQueries` | calls | `CodeEditor` highlighter and block segmenter, once per cache miss (a new text revision or provider generation) |
| `IdentityComparisons` | comparisons | linear key scans; none are left in `Picker`, which restores its cursor through a key map |
| `SourceRowsRead` | rows | `ScrollPanel` source lines read while it wraps (one per line per build) or measures its widest line; terminal rows read by `TerminalView` and `TerminalViewport` paint (`paint_view`, one per painted row) and by `TerminalViewport` row lookups (`locate`: selection, copy, blank and soft-wrap checks) |
| `TerminalBytesFed` | bytes | `TerminalState::feed`, before the feed guard filters the input; every `TerminalViewport`, session controller and pane feed passes through it |
| `DiffCellsVisited` | cells | `MeteredBackend::draw`: the cells the frame diff handed to the runtime's backend |
| `TerminalBytesWritten` | bytes | `MeteredWriter::write`: bytes the runtime's terminal writer accepted, escape sequences included |
| `RawEventsRead`, `RawEventsFiltered` | events | the runtime raw reader (`scan_next`): every event an input turn takes, and those dropped by the focus guard or as an unsupported kind. The startup discard of stale events (`discard_ready`) is not counted |
| `InputDispatches` | dispatches | every `Application::handle` call the runtime makes: input drains, settled pointer moves, external messages, resize, tick and the resume resize |

`TextViewport`, `ScrollView` and the width helper are shared, so every app that
uses them is counted. `VisibleRowsPainted` does not count `TextViewport` or
`TerminalViewport` paint; it does count `ScrollPanel` rows. `PayloadBytesCloned` counts text bytes only: a
`Select` or `Completion` overlay also clones its `OptionList`, and the count
does not include that. `IdentityComparisons` counts linear key scans. `Picker`
has none: `set_visible_items`, `set_visible_keys` and `replace_items` find the
cursor item through the key index or the new projection's key map, so the count
is 0 at any cursor position. Other key scans (`Picker` `retain`, `TreeView`,
`ViewportOps`) are not counted.

## Budgets

Library budgets live in `tests/regressions/work_budgets.rs`. Each test builds a
fixture at 1,000, 10,000 and 100,000 seeded rows or lines (60,000 for
`ScrollView`, whose extent is `u16`) in a fixed 100x30 viewport, warms it,
performs one operation and checks the delta. App scenarios live beside their
apps:

| Test | File |
| --- | --- |
| `work_contract_scroll_panel_flat_measures_once_and_appends_incrementally`, `work_contract_scroll_panel_wrapped_builds_once_per_content_change` | `tests/regressions/work_budgets/scroll_panel.rs` |
| `work_contract_terminal_prepared_paint`, `work_contract_terminal_feed_and_copy_units` | `tests/regressions/work_budgets/terminal.rs` |
| `work_contract_runtime_raw_filtered_events_yield`, `work_contract_runtime_flood_services_input`, `work_contract_runtime_frame_diff_and_bytes_written` | `src/runtime/raw_input_tests/work.rs` (lib unit tests: they drive the private `scan_next`, `service_turn`, `drain_ready_inputs` and `paint`) |
| `work_contract_jackin_scripted_output_budget` | `apps/jackin/src/sim/link/tests/work.rs` (lib unit test over the real `SimLink` and `TerminalPane`) |
| `work_contract_holla_idle_and_source_update` | `src/bin/holla/work_metrics_tests.rs` |
| `work_contract_tablepro_idle_and_cell_edit` | `src/bin/tablepro/work_metrics_tests.rs` |
| `work_contract_showcase_idle_and_switch` | `src/bin/showcase/work_metrics_tests.rs` |
| `work_contract_jackin_idle_and_roster_update`, `work_contract_jackin_capsule_idle_frame` | `apps/jackin/tests/regressions/work_budgets.rs` |

The app tests check each scenario with `assert_exact`: every listed metric must
equal its measured count and every unlisted metric must be zero. A ceiling
cannot tell a deleted counter from cheap work, so an exact count is the default
for a measured fact, and a counter that stops recording fails its test. The
`Picker`, `Select`, `Completion`, `CodeEditor` and `TextArea` tests in
`tests/regressions/work_budgets.rs`, and every test in the rows above that
covers `ScrollPanel`, terminal, runtime or transport work, use `assert_exact`
too. The older viewport,
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
| `Picker` | `set_visible_items` of 20 of n items | 1k / 10k / 100k | nothing cloned, identity comparisons 0 |
| `Picker` | frame over those 20 items | 1k / 10k / 100k | rows painted 12, width calls 324, width bytes 604, independent of n |
| `Picker` | `replace_items` | 1k / 10k / 100k | nothing cloned, identity comparisons 0 |
| `Picker` | `set_visible_items` or `replace_items` with the cursor on item 0, item 19 or the middle item | 1k / 10k / 100k | nothing cloned, identity comparisons 0 at every cursor position |
| `Select` | closed frame | 1k / 10k / 100k options | width calls 1, width bytes 6 |
| `Select` | open frame | 1k / 10k / 100k | payload bytes cloned 104, rows painted 8, width calls 1, independent of n |
| `Completion` | open frame | 1k / 10k / 100k items of 15 text bytes | payload bytes cloned 120, rows painted 8, width calls 240, width bytes 360, independent of n |
| `CodeEditor` | first focused frame | 1k / 10k / 100k lines | nothing cloned, annotation queries 2, rows painted 29, width calls 30, width bytes 67 / 68 / 69 |
| `CodeEditor` | settled frame | same | rows painted 29, width calls 30, no annotation query |
| `CodeEditor` | first keystroke, then second of the run | same | payload bytes cloned L, then none |
| `CodeEditor` | frame after the edit | same | annotation queries 2, rows painted 29, nothing cloned |
| `TextArea` | first focused frame | 1k / 10k / 100k lines | payload bytes cloned L, rows painted 10, width calls 3 (L is 32n - 1) |
| `TextArea` | settled frame | same | no clone, rows painted 10, width calls 3 |
| `TextArea` | first keystroke, then second of the run | same | payload bytes cloned L, then none |
| `ScrollPanel` (flat, horizontal scroll) | first frame | 1k / 10k / 100k lines of 33 bytes | source rows read = n, rows painted 30, width calls n + 30, width bytes 33n + 990 |
| `ScrollPanel` (flat) | settled frame, or scroll one row, then frame | 1k / 10k / 100k | rows painted 30, width calls 30, width bytes 990; no source row read |
| `ScrollPanel` (flat) | push one 13-byte line, then frame | 1k / 10k / 100k | source rows read 1, rows painted 30, width calls 31, width bytes 1,003, independent of n |
| `ScrollPanel` (wrapping) | first frame | 1k / 10k / 100k | source rows read = n, rows painted 30, width calls 39n + 30, width bytes 61n + 990 |
| `ScrollPanel` (wrapping) | settled frame | same | rows painted 30, width calls 30, width bytes 990; no source row read |
| `ScrollPanel` (wrapping) | push one line, then frame | same | source rows read n + 1, width calls 39n + 46, width bytes 61n + 1,014: the append drops the wrapped rows and the frame wraps every line again (a known gap, #291, see below) |
| `TerminalViewport` / `TerminalView` | draw or render at the live tail, or at the oldest history | 30x99 grid over 1k / 10k / 100k lines of scrollback | source rows read 30 for each, independent of the scrollback |
| `TerminalViewport` | feed 100 lines (3,500 bytes) | same | terminal bytes fed 3,500, no row read |
| `TerminalViewport` | press, then drag, then copy ten selected rows | same | source rows read 5, then 1, then 19 (one lookup per copied row and one soft-wrap lookup per row boundary), independent of the scrollback |
| Runtime raw events | one turn over n queued filtered events, budget 16 | 1k / 10k / 100k | raw events read 16, filtered 16, dispatches 0; the rest stays queued and the loop is told to return |
| Runtime raw events | one turn over alternating filtered and accepted events, budget 8 | 1k / 10k / 100k pairs | read 8, filtered 4, dispatches 4 |
| Runtime flood | one `service_turn` over n queued external messages (count policy 3), a resize and a tick | 1k / 10k / 100k | dispatches 5 |
| Runtime flood | one `drain_ready_inputs` over n ignored inputs | 1k / 10k / 100k | dispatches 256 (`DRAIN_BUDGET`), inputs offered 256 |
| Runtime frame output (`paint` into a fixed terminal of `#`) | first frame | 40x10 / 80x24 / 200x60 | diff cells = every cell (400 / 1,920 / 12,000), bytes written 486 / 2,104 / 12,436 |
| Runtime frame output | unchanged frame | same | diff cells 0, bytes written 25 (the style reset) |
| Runtime frame output | one cell changed | same | diff cells 1, bytes written 32, for every size |
| Jackin `SimLink` into a `TerminalPane` | one 200,000-byte output chunk, feed budget 1,024 / 4,096 / 65,536 | 196 / 49 / 4 ticks | terminal bytes fed per tick = min(budget, remaining), 200,000 in all; nothing fed after the stream |
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
  frame copies and segments nothing. A `TextArea` frame measures only
  its visible lines. The first keystroke of an undo run copies the whole text
  in both. Later keystrokes of the run do not.

- A `ScrollPanel` append invalidates every wrapped row, so a wrapping panel
  rewraps its whole history on the next frame (n + 1 source rows). The flat
  panel extends its widest-line cache by the added lines only. The wrapping
  count is today's behaviour of an unfixed path; an incremental wrap lowers it
  to n-independent work. Tracked in
  [#291](https://github.com/tailrocks/terminal-components-baseline/issues/291).
- Copying ten terminal rows reads 19 rows: each row is looked up to cut its
  text and once more to ask whether the next row continues it. The count is
  bounded by the selection, not the scrollback.
- A frame that changes nothing still writes 25 bytes (the style reset that
  ends every `draw`); it hands the backend no cell.
- A pane feed counts the bytes it passes to the emulator. The `FeedGuard`
  filters inside `feed`, so a guarded sequence is counted before it is dropped.
  Control events (attach, lease, notice) cost no fed byte and have no counter.
- A scripted `SimLink` obeys the feed budget on its first oversized chunk and
  across ticks.

### Not covered

`CodeEditor` bracket matching, `Picker`
`retain` scans and the `CodeEditor` find state are not counted.

Also unmet, and absent from every scenario above:

- No editor or app-page measurement scenario exists. The release measurements
  time `TextViewport`, `ListBox`, `TreeView` and `ScrollView` only. There is no
  `CodeEditor` or `TextArea` timing, and none for a Holla, TablePro, Showcase
  or Jackin page.
- The timing oracle runs after each operation only. #56 asks for a check
  before and after every operation, so the first round has no pre-check and a
  state that was already wrong before round 0 would go unnoticed.
- No `performance` suite exists in `.velnor/config.toml`, so the profile
  does not run as its own CI job. The same tests run in the `Rust / <crate>`
  jobs through the `ci` profile. Adding the suite changes the generated
  workflows and needs the owner's decision.

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

## The performance profile and its inventory gate

`cargo nextest run --profile performance --workspace --all-features --no-tests fail`
runs every deterministic cost contract in one command: the counter core, the
fit linearity tests, the library budgets, the four apps' scenarios, Holla's
and Jackin's projection-reuse tests, the measurement sampling check and the
gate below. The profile is `[profile.performance]` in `.config/nextest.toml`.
It needs `--all-features` because the counters compile away without
`work-metrics`; without it the budgets would not run at all.

`tests/regressions/performance_inventory.rs` (the `performance_inventory`
module of the shared `regressions` target) is the required-suite inventory. It
lists each required suite with its nextest binary, the filter that selects it,
its source file and the tests it must hold, and fails when

- nextest does not list a required test, lists it `#[ignore]`d, or the profile
  filter does not match it. This is checked on the real output of
  `cargo nextest list --profile performance --workspace --all-features
  --message-format json`, which the gate runs itself, so a removed `mod` line,
  an `#[ignore]`, a `#[cfg]` that drops the test or a filter that no longer
  selects it all fail;
- the profile no longer has the operand for a suite, so nextest would skip it;
- the operand would not select one of the suite's required tests, for example
  after a module rename or a narrowed regex;
- a required test is no longer a `#[test]` function in its file.

The list check runs only under `--profile performance`: there the nested list
is a cache hit on the build the run just made. Under any other profile it
prints that it is skipped, because a list in the wrong feature shape would
rebuild the workspace. Run the profile with the documented command; a run with
other `-p` or feature flags still works but rebuilds for the nested list.
The gate cannot vouch for itself: an `#[ignore]` on the gate's own tests
removes the check, and only `--no-tests fail` and review notice that.

The profile filter is read as text, not evaluated, so it must stay a plain
union of `binary_id(=ID) & test(MATCHER)` terms. The gate refuses `not` and set
difference, and any matcher richer than `/^prefix/`, `/substring/` or `=exact`,
instead of guessing. The inventory is a floor: a new test in an existing
suite needs no edit while the operand still selects it. A new suite needs an
inventory entry and an operand together. `gate_logic::*` runs the same checks
on synthetic input, including a removed suite and a narrowed matcher.

Both measurement files live in the shared `regressions` target
(`tests/regressions/work_measurements.rs`, `performance_inventory.rs`), as
`docs/ci.md` prescribes, so the generated workflows list no new test target.

`[profile.performance-measure]` selects the ignored timing tests and sets
`test-threads = 1`, so that parallel tests do not time each other. The gate
checks both, and that each timing test is `#[ignore]`d.

## Timing reports

A timing report is a different file from a count report. It is
`<target dir>/performance/<scenario>-<operation>-<records>.timing.json`, schema
version 2, written by `junie_tui::metrics::measurement::MeasurementReport`.
Fields:

| Field | Meaning |
| --- | --- |
| `schema_version`, `scenario`, `operation`, `record_count`, `viewport` | As in a count report. `record_count` is the fixture's real size. |
| `warmups`, `samples` | 20 warm-ups, then 200 timed samples. Warm-ups are not in the statistics. |
| `median_ns`, `p95_ns` | `sorted[(n - 1) / 2]` and `sorted[ceil(0.95 n) - 1]` over the samples. `min_ns` and `max_ns` come with them. |
| `work_delta` | The operation's counts, or `null`. Timing runs have the counters off, so it is `null` there: unavailable, not zero. Counts come from the count reports. |
| `allocations_or_null` | Always `null`: no allocation instrumentation exists. |
| `rustc`, `profile`, `target`, `os`, `cpu` | Read from `rustc`, the build, `uname` and the CPU on the machine that ran. A value that could not be read is `"unknown"`. `target` is the host triple, so cross builds are not supported. |
| `backend` | What was drawn into: the in-memory ratatui `Buffer`. |
| `terminal_io_included` | `false`: no terminal, child process or backend I/O is in any interval. |
| `instrumentation_enabled` | Whether `work-metrics` was on. The measuring test refuses to run when it is. |
| `correctness_oracle` | What each round checked, outside the timed interval. |
| `setup_ns_or_null` | Fixture build and warm-up cost, kept out of the samples. |
| `commit`, `commit_dirty` | `git rev-parse HEAD` and whether tracked files differed from it. |
| `load_average` | `uptime`, read when that scenario's test starts. |

`measurement::REQUIRED_FIELDS` lists them, and `measurement_required_fields_are_all_present`
fails when one goes missing. Nothing gates on a time: no wall-clock threshold is
enabled until repeatable baselines exist.

## Release measurements

`scripts/measure-performance.sh` is the procedure. It builds `tests/regressions/work_measurements.rs`
in the release profile without `work-metrics`, runs the ignored tests one at a
time through `performance-measure`, and prints a summary table of the reports
with the commit, machine, CPU, compiler and the load average before and after.

```sh
scripts/measure-performance.sh
```

The measured operations, each on the real library path at 1,000 / 10,000 /
100,000 records (1,000 / 10,000 / 60,000 rows for `ScrollView`):

| Scenario | Operation timed | Oracle, outside the interval |
| --- | --- | --- |
| `viewport_prepared` | `push` of one new line, then `set_area` | the last row shows that round's line and the row above shows the previous round's |
| `viewport_prepared` | one prepared `draw` into a fresh 100x30 `Buffer` | the frame shows the last 30 lines |
| `list_box` | one settled `ListBox` frame | the frame equals the warm frame |
| `tree_view` | `set_filter`, alternating `node 5` and `node 55` | the visible rows equal the matching labels and differ from the previous round |
| `scroll_view` | `render` of overflowing content (the content-sized canvas of #20) | row 0 shows the first content row |

Rules the procedure follows:

- Run it on an otherwise idle machine and read the load average it prints. It
  does not refuse a busy machine. A report from a loaded machine says so
  through `load_average` and the printed load, and should be repeated.
- Fixtures are built before the timer. An append grows the history by one line
  per round, 220 rounds in all, so a sample appends to a different history
  each time. The filter alternates two queries so no round is an equal write.
  `measurement_edit_samples_are_real_changes` runs both at a tiny size in the
  `performance` profile and fails if a round does not change the state.
- Each draw allocates its own 100x30 `Buffer`, `HitRegistry` and `FocusRing`
  inside the interval, as the budgets' frames do. A timing here is the cost
  of that frame, not of the widget's paint loop alone.
- Counters are off in the timing build. Do not compare these numbers with a
  build that has `work-metrics`, and do not read them as a speedup between
  commits unless both ran on the same machine under comparable load.
- A debug build refuses to measure.

Results are not committed. The report files and the summary stay in
`<target dir>/performance`.

## Commands

```sh
cargo nextest run -p junie-tui --features work-metrics --test regressions \
    --no-tests fail -E 'test(/^work_budgets::/)'
cargo nextest run -p junie-tui --features work-metrics --bin holla --bin tablepro --bin showcase \
    --no-tests fail -E 'test(/^work_metrics_tests::/)'
cargo nextest run -p jackin-console --features demo,work-metrics --test regressions \
    --no-tests fail -E 'test(/^work_budgets::/)'
cargo nextest run -p junie-tui --features work-metrics --lib --no-tests fail \
    -E 'test(/metrics/) | test(/work_contract_/)'
cargo nextest run -p jackin-console --features demo,work-metrics --lib --no-tests fail \
    -E 'test(/work_contract_/)'
cargo nextest run --profile performance --workspace --all-features --no-tests fail
scripts/measure-performance.sh
```

CI builds both Rust jobs with `--all-features`, so these tests already run in
the existing `Rust / junie-tui` and `Rust / jackin-console` jobs. The product
shape builds without `work-metrics`, where the tests compile away and
`metrics_disabled_does_not_evaluate_arguments` runs instead.
