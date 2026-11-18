# Decisions

## Shared authority

The design reference governs exact product behavior, color, layout, keyboard and pointer interaction, and visual acceptance. Main governs the intended modular architecture. These authorities answer different questions: main's crate boundaries do not override the design, and design's current implementation does not prove that main or a WIP has completed the architecture or product.

## Compatibility and migration

Preserve public APIs and documented runtime behavior unless a deliberate architecture decision authorizes a breaking change. Retire old paths through an intentional migration with consumer updates and verifiable acceptance. Do not use historical status reports, old staffing plans, or frozen baseline captures as current readiness evidence.

## Evidence limits

Build success demonstrates compilation for the selected workspace and configuration. Tests demonstrate only their exercised cases. Captures demonstrate only their named states and dimensions. Historical source comments and safety rationale are evidence of prior reasoning, not fresh safety certification. Documentation link and cross-branch content checks certify documentation integrity. Operational proof validators certify only their named artifacts and source snapshots. A versioned packaging migration must identify retired prose explicitly without rewriting measured evidence or weakening operational checks.

## Scope discipline

Future proposals remain deferred until a consumer and acceptance contract are identified. Upstream work remains an external dependency until released, pinned where appropriate, and verified in this repository. Do not silently broaden supported platforms, host operations, or interaction abstractions without isolated evidence.

## 2026-11-18: A render that changes widget state may take a third paint pass (#17, #176)

Context: #17 asks for at most two paint passes per draw, with an unstable
second pass reported through `render_pending`. #176 then found that
`TextInput`, `TextArea` and `CodeEditor` start their focus edit inside their own
`render`, after widgets drawn earlier in the frame (a page footer under a modal)
already read the old state. The shell's second pass is the first pass that
paints the newly focused field: focus is only assigned by upkeep after pass
one, so the edit starts while pass two paints and the widgets drawn before it
in pass two still show the old state. #176 offered two fixes: move the
transition out of render, or add a render-time "state changed after it was
read" signal. The signal route shipped (79147b6ea, `RenderCtx::note_render_mutation`,
`MUTATION_PASSES` in `src/shell/app.rs`).

Decision: keep `RENDER_PASSES = 2` for every frame whose settlement is a focus,
hover or blur change. A frame in which a widget reports that it changed its own
state during render gets one more pass (`MUTATION_PASSES = 3`). Two passes
cannot meet #176's acceptance ("a library regression test in which a widget's
render changes edit state must produce a settled frame through the real
`Shell::render` path"), because the mutation first happens in pass two. Moving
the transition into upkeep would remove the third pass but changes when every
text widget starts editing and who may observe `editing`; that migration is
larger than the defect and is not planned. The bound stays: a widget that
mutates on every render stops after the third pass, the frame stays pending
and `Application::take_render_invalidated` returns it once, so the runtime
draws again on its next fair turn instead of looping.

Pinned by `tests/regressions/focus_edit.rs`: the three focus-edit cases draw
exactly three times and settle; stable frames draw once;
`a_widget_that_mutates_on_every_render_stops_at_the_bound_and_stays_pending`
shows the stop at three draws with `render_pending` set and consumed once.
`shell_unstable_second_pass_requests_next_turn` in `tests/shell.rs` keeps the
two-pass bound for focus oscillation.

## 2026-11-17: Remove public items nothing produces or consumes (#353)

Context: the #14 declared-API burn-down found public items with no producer
and no consumer anywhere in the workspace. Each was kept as a `Nonvisual`
`CapabilityException`, because "Compatibility and migration" above forbids a
breaking change without a recorded decision. The human delegated the decision
for #353. It was re-checked on `origin/main` (836bd0f8c) by searching `src`,
`apps/{showcase,tablepro,holla,jackin}`, `crates/termrock`, `tests` (including
`tests/consumers/*`), `examples` and `docs`. The facade re-exports whole
modules only, so no facade item or consumer fixture names any of them. Their
only references were the catalog, the exception lines and the tests written to
satisfy those lines.

Decision, by group. All are option (a), removal, except where noted.

- `core::intent` vocabulary. `Intent::{Key, Binding, Paste, FocusIn, FocusOut,
  Layer, Cancel}`, `FocusVia`, `LayerEvent`, `ActionKey` (with `CONFIRM`,
  `CANCEL`, `CLOSE`, `custom`, `raw`) are removed. Hosts receive input through
  `Response<A>` and key maps (`core::keymap`); no host dispatch path takes an
  `Intent`, so there is no concrete host use to document (option b) and no
  producer worth adding (option c). `Intent` keeps `Pointer` and `Wheel`, the
  two variants `PointerRouter` builds. With `Paste(&str)` gone, `Intent` has no
  lifetime parameter. `DismissReason` stays: `Layer::on_dismiss` and
  `ShellLayers::dismiss` use it in every app.
- `Phase::Move` and `Phase::SecondaryRelease` are removed. Wiring them
  (option c) is not a small change. Hover is already complete through
  `PointerState::on_move`; delivering a `Move` event to widgets adds a dispatch
  per mouse motion that no widget reads. A secondary release already completes
  as a `Click` on the row the secondary press started on, and a second event
  for it would double-deliver to every consumer of that route. Tests that
  used `Phase::Move` only as "a phase that is not a scroll gesture" now use
  `Phase::Middle`.
- `core::id::Part`: `ACTION`, `BADGE`, `BODY`, `BORDER`, `CHANGE`, `DETAIL`,
  `EMPTY`, `GUTTER`, `HELP`, `LABEL`, `META`, `PLACEHOLDER`, `PREFIX`, `RULE`,
  `SUMMARY` and `TEXT` are removed. No widget registers them as a hit region,
  and `Part::custom(name)` already lets a host name any region of its own. The
  numbers of the remaining parts are unchanged, so no `WidgetId` derived from a
  part changes. `Part::ALL` shrinks from 36 to 20 entries.
- `core::response::StateFlags` (the type, its 16 flags and its 11 methods) is
  removed. Widgets style state through `VisualState` and
  `Theme::interactive`, and `Response` carries no flag set.
- `theme::downgrade`, Mono branch: the audit's claim did not hold, so this
  group is a repair and a deletion rather than a removal of dead code.
  `downgrade_all` runs for `ColorLevel::Mono` before `apply_mono`, and
  `apply_mono` did not name seven colours (`warning`, `info`, `link`,
  `syntax_keyword`, `syntax_string`, `syntax_literal`, `selection_inactive`).
  Those came from the RGB band, so the branch was live. A test run before the
  change showed the mono theme differing from a theme built by `apply_mono`
  alone in exactly those fields. `apply_mono` now assigns them, with the values
  the band produced (`Gray` ×5, `White`, `DarkGray`), so every colour of the
  mono theme keeps its value and the band is deleted. `downgrade` is private;
  the change is not an API change.

Consequences: the declared-API catalog drops 68 entries, and
`apps/showcase/src/coverage/lane_routing.rs` and `lane_state.rs` drop 24 and 44
exception lines. The gate stays strict: nothing is allowlisted in their place.
`catalog_contract.rs` still fails when an API has no disposition, and the
scenarios `IntentVocabulary` and `StateIdentity` keep proving what remains.
`Intent::part` and `Intent::pos` still return `Option`, because `Intent` is
`#[non_exhaustive]` and a later variant may carry neither.

Migration: see "Breaking changes in v0.1 and migration" in
[the API contract](api-contract.md). A host that needs one of these ideas again
adds it with a producer and a test through a real route in the same change,
which is the bar the removed items did not meet.

## 2026-11-18: Hiding a dirty FormDialog input follows its blur policy (#87)

Context: #87 selected the contract that hiding a field ends its transient
interaction, "Input ends editing while retaining current text as programmatic
state (no implicit parse/commit/Changed)". PR #185 shipped that. Train fix
`e005e7c77` (the paint-form train, #262, a codex review finding) then changed
it: hiding a dirty input now ends the edit through `TextInput::on_blur`.
`BlurAction::Cancel` reverts to the committed text and reports nothing;
`Commit` and `Keep` retain the typed text and the form reports
`FormAction::Changed` once. The independent audit of main `fb5c9ed63` flagged
the difference from #87's text, and no record explained it.

Decision: keep the shipped behaviour. The first contract was wrong for two
reasons the review found:

- It ignored the input's own `BlurAction`. A `Cancel` input promises that
  uncommitted text never survives losing the edit. Adopting the typed text as
  the new baseline on hide broke that promise for the one blur path (hiding)
  that no user gesture visibly triggers.
- It changed the values in `FormValues` with no event. A consumer that
  tracks `Changed` (the dirty mark, the quit guard, Jackin's `form_changed`
  hook that drops a pending validation) never learned that the retained text
  was the user's edit, so the form held text its consumer had not been told
  about.

The rest of #87 is unchanged: hiding adds no step of its own beyond the
input's blur (a `Commit` blur runs the input's validator, as any blur does),
hidden values stay in `FormValues`, reveal restores
value and configuration but never an editing session, and a Select closes and
cancels its capture. Hiding an already hidden field reports nothing a second
time.

Pinned by `form_hiding_a_dirty_input_follows_its_blur_policy` in
`src/widgets/form_dialog/tests.rs` (`Commit`, `Keep` and `Cancel`). The
contract is documented on `FormDialog::set_visible`. Consumers: Jackin's
Accounts and Config forms and Showcase's connection form call `set_visible`
when a source, kind or driver field changes, so the input being hidden is not
mid-edit there. Their tests pass unchanged.

## 2026-11-18: Showcase keeps its own quoting helper (#181)

Context: #181 asked for one shared identifier-quoting helper owned by
TablePro's SQL mapping. Showcase and TablePro are separate binaries with no
shared module, and the library stays SQL-free, so the acceptance line cannot
be met literally. The scope note on #181 (from the review of PR #193) records
the split.

Decision: TablePro keeps `quote_ident` and `table_ident` in
`apps/tablepro/src/model.rs`, which the DDL view reuses. Showcase keeps a
3-line demo helper in `apps/showcase/src/pages/grid.rs`. Both double an
embedded quote the same way, and both have tests for it. The helper is not
moved into the library because the library holds no SQL concept, and it is
not shared between the apps because that would make one demo depend on the
other's binary. Revisit only if the library gains a SQL-neutral
identifier-quoting capability with a consumer in each app.

TablePro's filter SQL (`apps/tablepro/src/sql.rs`) keeps one policy of its
own: plain lowercase names print bare so a previewed filter reads as typed.
Review found that its reserved-word list was incomplete (a column named
`current_date`, `fetch` or `natural` printed bare and no longer meant the
column). It now lists every PostgreSQL reserved word and quotes through
`model::quote_ident`, so TablePro has one place that doubles an embedded
quote. Pinned by `issue181_every_postgres_reserved_word_is_quoted_in_filter_sql`.

## 2026-11-18: The performance CI suite and app-page timing (#56)

Context: the #56 audit found no `performance` suite in `.velnor/config.toml`
and no timing scenario for an app page or for `CodeEditor` and `TextArea`.
The suite is now declared (id `performance`, profile `performance`, four
partitions, `required = true`, pull request, push and manual triggers), and
`CodeEditor`, `TextArea` and `ScrollView::render_window` have timing
scenarios. Two parts of #56 differ from its text, on purpose.

Decision 1, the suite builds without `work-metrics`. #56 asks that the
configuration selected for the suite enables `work-metrics`. A suite in
`velnor-actions` (0.1.78 pinned, checked again against 0.1.80) has no feature
or `all_features` field: its commands are fixed as
`mbx nextest run --workspace ... --profile performance`, so the counters are
compiled out of the suite build and the `#![cfg(feature = "work-metrics")]`
contracts do not run there. Enabling the feature for the whole workspace some
other way (a default feature, or a workspace member that turns it on for
everyone) would make every default build, the functional PTY journeys included,
carry the counters, which #56 forbids ("default-off") and which the
`s06_meters` journey and the timing tests tell apart by build. What ships instead:

- the contracts execute in the `Rust / <crate>` jobs, which build with
  `--all-features` and whose `ci` profile selects every test but the
  `functional` binary;
- the suite runs `performance_nextest_list_runs_every_required_test`, which
  nextest sets `NEXTEST_PROFILE=performance` for and which runs
  `cargo nextest list --profile performance --workspace --all-features`. The
  inventory therefore no longer depends on someone running the profile by hand.
  A nonzero aggregate count is not enough: every family and every app must list
  each required test, not ignored and selected by the profile.
- `performance_suite_is_a_required_generated_ci_job` pins the declaration, the
  generated job and the `Required` dependency, so the suite cannot drop out.
- `performance_contracts_run_in_required_all_features_ci_jobs` pins, from the
  generated task manifest, that each package owning a required test runs
  `nextest run --profile ci --package P --all-features` in a job `Required`
  waits for, and the nested list is repeated with `--profile ci`, so every
  required contract is selected where the counters are on.

`allow_empty_test_suite` stays on: it is a stack-wide setting that other
legitimately empty shards need, and it only lets a shard with no test pass.
Revisit when `velnor-actions` can give a suite a feature selection: then the
suite should run the contracts themselves, and the list check can stay as the
inventory gate.

Decision 2, no app-page timing scenario. #56 step 9 asks for one actual report
per family and per app. Library families have count reports and, for
`TextViewport`, `ListBox`, `TreeView`, `ScrollView`, `CodeEditor` and
`TextArea`, timing reports. The four apps have count reports only. Timing an
app page needs a driver that runs the page's real input and render path without
the counters, and none exists: `termrock::metrics::harness::Harness`, the driver
every app scenario uses, is compiled only with `work-metrics`, and a timing run
refuses a build that has it (`instrumentation_enabled` must be `false`, and a
timing taken beside counters would not be comparable). The apps' models and
pages are private to their binaries, so a driver cannot live in the library
tests, and writing one per app would duplicate input and render driving four
times, which the component rules forbid. The right owner is one shared
off-screen driver in the library, which is a public API addition that needs its
catalog line and showcase disposition, and its own change. Until then
`scripts/performance-report.sh` prints each app's counts, and the timing
section carries the library families only. This is a stated gap, not a result:
no app time is claimed anywhere.

## 2026-11-18: Crate boundaries and the layering check (#439, #440)

Context: epic #439 splits the `junie-tui` monolith into `termrock-*` crates.
`docs/architecture.md` had a target table from before the split was measured.
A source scan of `origin/main` (`ecffc8e21`, then `bf62acd30`) showed that the
target table does not hold, and that nothing enforced any direction between
the modules of the single package.

Decision 1, crate boundaries. The crate table in `docs/architecture.md` and the
`[allow]` table in `docs/layering.toml` replace the earlier table. They differ
from it in four ways, each measured:

- `layout` is a component-layer crate, not a foundation. `Split` uses
  `scrollbar` and `keyhint`, and `Measure` takes `RenderCtx`.
- Overlays split into `termrock-overlays` (menus, popups, layers) and
  `termrock-dialogs` (dialogs, forms, file browser). `input` needs a context
  menu and the dialogs need `input`, so a single crate is a cycle. There is no
  `forms` crate.
- `termrock-session` holds the run loop and `termrock-runtime` holds
  `RenderCtx`. The facade module `termrock::runtime` comes from `session`.
- Focus, hit testing and key maps stay in `termrock-core`. `list`, `keyed` and
  `intent` depend on them, and `core::{hit, intent, pointer, scroll, shape}`
  with `core::{keymap, list}` form a five-module cycle.

Decision 2, how the direction is enforced. `tests/regressions/layering.rs`
checks `docs/layering.toml` at module level now (stage A) and at crate level
from `cargo metadata` once `crates/termrock-*` exist (stage B, off until #444).
Known upward dependencies are recorded in
`tests/regressions/layering-allowlist.tsv`. The list may only shrink; a line
that no longer matches a dependency fails as stale. The allowlist is not a way
to land a new violation: fix the design.

Where the implementation differs from the plan in #440, and why:

- Counts. #440 expected 94 allowlist pairs (36 production, 58 test). The
  scanner finds 42: 38 production in 10 files and 4 test. The production
  difference of two pairs is not reconciled pair by pair: #439 counted with a
  regex over path text, this scan resolves syntax to the defining module and
  keeps one line per (file, target module). The test difference is the file map. #444 to #449
  say where each test file lands, mostly `termrock-conformance`, which the
  table allows to depend on every library crate, so those files are not
  upward edges; the 58 counted them against the module they sit in today.
  Test files that stay with a crate (`widgets/code/tests.rs`,
  `widgets/textarea/tests.rs`, `widgets/picker.rs`, `ui/layer.rs`) still
  record their upward edges.
- `[allow]` is checked together with what its dependencies use. #439 says "every
  row also allows the foundation crates that the row's dependencies already
  use", so the permitted set is the transitive closure of `[allow]`. `[allow]`
  itself is the #439 table verbatim.
- `tests/support/*` is scanned and mapped to `catalog`, because
  `src/catalog_contract.rs` includes it through `#[path]`.
- `[layers]`, `layer_order` and the `functional`, `junie-tui` and `termrock`
  rows are additions to the #439 table: the layer rule makes the sentence
  "apps, then facade, then components, then runtime, then foundations" a
  checked property, and stage B needs a row for every `termrock-*` package.
  `junie-tui` sits below `termrock` while it is the implementation package;
  #452 flips it.
- The scan covers `crate::`, `self::`, `super::` and child-module paths in
  code, types, `use` trees and macro arguments. Doc comments, string literals
  and `pub(crate)` are not dependencies. A `#[macro_export]` macro used as
  `crate::record_work!` counts as a dependency on the module that defines it
  (`metrics`).

- Stage B also fails a `crates/termrock-*` or `apps/*` directory that holds a
  `Cargo.toml` but is not a workspace package, because `cargo metadata` would
  not list it and its dependencies would go unchecked. Added at review.

Revisit when #452 removes `[files]`: from then on only stage B remains.

## 2026-11-18: The #14 inventory walks several crates and pins its ids (#443)

Context: the gate must keep one catalog line and one disposition per public API
while the module graph is spread over `crates/termrock-*`, and the id set must
not change when code moves.

Decision:

- Ids stay facade spellings. The walker takes a list of crates (the facade
  first, from `docs/layering.toml`), keeps the facade's modules under their plain
  paths and puts every other crate under `@<crate name>`. A module or item of
  another crate is exported only through a `pub use` the facade makes of it, and
  carries that spelling.
- When a definition has several public spellings, one that already has a catalog
  line wins (PR A, `preferred`). Without it a move changes the id: the
  definition's own path used to win whenever it was public. The old rule still
  applies when the catalog has none of the spellings.
- A `#[macro_export]` macro is a definition at the root of its own crate, so a
  macro that another crate re-exports gets an id; it used to be listed
  unconditionally. Before the split the output is identical.
- The facade is the root package while `src/lib.rs` exists, because `termrock`
  only re-exports `junie-tui` today; #452 switches it to `crates/termrock`.
- Deviation from #443: the golden check compares against a committed digest
  (`tests/regressions/api-ids.sha256`), not `git show origin/main:...`, so it needs
  no fetched `origin` in CI. The cost is that a PR that adds public API updates
  one file (`UPDATE_API_IDS=1`). The digest was computed from the walker before
  this change, and the refactored walker reproduces it.
- Feature gates are still evaluated once for the matrix row, not per crate;
  #444 maps the row's features onto each crate's features.
- `.sonarcloud.properties` excludes `crates/termrock-catalog/src/catalog.rs`
  from copy-paste detection now, ahead of the catalog's move in #449.

## 2026-11-18: Foundation back-edges use unnameable traits (#441)

`core::hints` now owns `Hint` and `core::selection` owns the check strings, so
`core` no longer names `theme` or `widgets`. `Hint::is_pinned` and
`TriState::glyph_in` take a `&impl` of a trait that `theme` implements
(`PinnedKeys` for `HintBarStyle`, `CheckGlyphs` for `Glyphs`). Every old call
site still compiles.

- The two traits are the issue's proposal, with one change: they are `pub`
  traits inside private modules, re-exported as `pub(crate)`. The declared-API
  inventory counts only items it can reach through a public path
  (`tests/support/api_inventory.rs` does not read `#[doc(hidden)]`), so a
  nameable trait would add two ids and need catalog and coverage lines. That
  contradicts "the inventory id set does not change".
- `TriState::glyph()` uses constants in `core::selection`, and
  `Glyphs::UNICODE` takes its three check strings from them, so the two cannot
  drift.
- `VisualState` and `CardHeading` moved into `theme`; `ui::ctx` and
  `widgets::panel` re-export them. Their catalog ids keep the old spelling
  through the canonical-spelling pin of #443.
- The B6 edge (`metrics/harness.rs` to `runtime`) needed no change: `[files]`
  already maps the work-metrics harness, its report and its tests to
  `conformance`.

Revisit in #444: once `core` and `theme` are separate crates the traits must be
visible across the boundary. Make them `#[doc(hidden)] pub` and have the
inventory skip hidden items in the same PR, so the id set still does not grow.

## 2026-11-18: `termrock-metrics` and `termrock-text`, and the hidden-item rule (#444)

The first two foundation crates are out. These choices differ from the issue's
wording or settle what it left open.

- **`#[doc(hidden)] pub` has no id, and is pinned.** An item that must cross a
  crate boundary is `pub` and `#[doc(hidden)]`, as #441 planned, and the
  inventory gives it no id, so the id digest does not grow. A hidden item can
  still be named by a consumer, so the attribute alone would let a new public
  item skip the catalog. `tests/regressions/doc-hidden-items.txt` therefore
  lists every hidden item of the walked crates (`<file>  <item>`), and
  `catalog_doc_hidden_items_are_a_reviewed_list` fails on any difference;
  `UPDATE_DOC_HIDDEN=1` rewrites it. The attribute is for plumbing that crosses
  a boundary, never for something an app should call: apps and examples may not
  use a listed item. The scan does not check that last part; the facade tests
  that name no implementation package and review of the list carry it.
- **A manifest macro may sit at another crate's root.** `metrics!` moved to the
  root of `termrock-metrics`, so its `EXPANSIONS` site is
  `@termrock_metrics::metrics`, reachable once the facade re-exports any item of
  that crate. A public re-export of an item such a macro writes
  (`pub use termrock_metrics::Metric`) is not an unresolved re-export.
- **`src/metrics.rs` maps to `conformance`.** It keeps the old paths and declares
  `harness`, `report` and `tests`, which are #449's files, so it goes to
  `conformance` with them until #449 moves the harness; the counters themselves
  are in `termrock-metrics` (`layering` stage B now runs).
- **`termrock-text` has a `test-support` feature.** The line-index work counters
  (`text_index::work`) were `cfg(test)` of the old package and the editors' cost
  tests in `junie-tui` read them. The feature is off by default and enabled only
  by `junie-tui`'s `[dev-dependencies]`, so `cargo tree -e normal,build` shows
  only `default` for it, and a normal build, the facade, the Showcase and the
  consumers never compile the counters. A workspace build with test targets
  unifies the feature across that build, so another package's test binary links
  the counters too (thread-local increments, no change to what any contract
  asserts); `junie-tui`'s own tests see exactly the set `cfg(test)` gave them.
- **`core/text/tests.rs` moves with the buffer** into `termrock-text` except
  `text_revision_follows_undo_and_redo`, which drives `core::edit` and stays in
  `junie-tui` until `termrock-core` exists (#444 core).
