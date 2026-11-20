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

> Superseded for CI shape by 2026-11-20 (velnor-gen): the `performance` suite
> is now the all-features test lane. The reasoning below about the generator
> giving a suite no feature field describes velnor-actions.

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

## 2026-11-18: Relocate CI policy scripts, consumer fixtures and the example first (#450)

#450 depends on #448, which has not landed. Three groups of its move list do
not import any extracted crate, so they land first as a stack
(`split/450-a`, `-b`, `-c`). Every root test target that imports a crate
extracted by #444 to #448 waits.

- Policy scripts, Python tests, fixture pack and mutants move to
  `scripts/policy/`. A flat move into `scripts/` would collide with the
  existing `scripts/check-generated-workflows.sh`. Each shell script resolves
  the repository root with `../..` and each Python script with `parents[2]`;
  the issue lists the files but not this edit. `test_build_app_bins.py` also
  moves, although the issue's list of 15 omits it. Reviewed digests are
  unchanged: no pinned digest covers a moved file, and no pinned file changed.
- Consumer fixtures move to `crates/termrock/tests/consumers/<name>/`. The
  issue's manifest paths are one level short. From the new directory the
  facade is `../../..` and `jackin-console` is `../../../../../apps/jackin`.
  The four lockfiles are unchanged and `scripts/check-consumers.sh --locked`
  passes.
- `examples/document_workspace.rs` moves to `crates/termrock/examples/` and
  imports `termrock::`. It carries no `#[test]`, so it is not one of the six
  contract examples.
- Not weakened: no check was removed or loosened, and the same commands run
  from the new locations.
- SonarCloud classified the moved scripts as main code and failed the Security
  Rating on New Code at E (path traversal, argument injection and publicly
  writable directory findings in the guard harnesses). They raised none of
  these findings under `tests/`, where SonarCloud reads them as test code, so `.sonarcloud.properties` now sets `sonar.test.inclusions`
  to `scripts/policy/**` plus every `tests/` directory. That keeps the
  classification they had; no finding is suppressed in the scripts.

## 2026-11-18: The functional suite moves to `termrock-functional`, and the frozen oracle moves with it (#451)

`crates/termrock-functional` owns `tests/functional`: the Showcase, TablePro and
Holla journeys, the 121 `functional/` captures, the ten reference binaries and
the 1,905-file `pre_refactor` oracle. The three options of #451 were weighed
against evidence in this repository, not by opening a trial PR.

- **What SonarCloud counts.** Its Automatic Analysis crashes on the number of
  files in a language it analyses, not on binary captures. PR #326 changed 1,786
  files, almost all `.pb`, and passed; the probe PRs #327, #328, #329 and #331
  (1,100 to 1,740 modified baselines) all passed; PR #285 changed 1,172 files
  (867 of them `.pb` or `.json`) and passed. #326 states it: "Sonar analyses
  `.pb` baselines fine; what crashed it was the number of JSON receipts in one
  PR". The files that count are `.rs`, `.py`, `.sh`, `.toml`, `.json`, `.md`,
  `.yml` and `.tsv`. The oracle holds only `.pb` captures, two
  `.tuiscotti-approval.lock` files and ten executables, so moving it adds no
  analysed file.
- **Option B (one rename-only data move): chosen.** The "about 50 files" rule is
  a proxy for analysed files. Option A would be about 40 PRs of 48 `.pb` renames
  each, every one waiting for a full CI cycle, plus a dual lookup in `support.rs`
  that exists only to be deleted. Neither buys any Sonar safety.
- **Option C (leave the oracle in place behind one path): rejected.** The
  `g05_reference` receipt tests, `scripts/accept-changed-baselines.py` and the
  cross-binary cases all treat `functional/`, `showcase/` and `pre_refactor/`
  as one baseline tree under one root. Splitting that tree across two roots
  forces every one of them to carry two roots, and it leaves a root `tests/`
  directory that #452 must remove. All lookups now go through `BASELINES` in
  `support.rs` (`oracle_baselines()`, `baseline_root()`), so a later relocation
  is one constant and one `git mv`.
- **Receipts stay as written.** `docs/testing/**` (about 300 JSON and TSV
  receipts, 7,000 recorded paths) names `tests/functional/baselines/...` and
  stays unchanged: rewriting it would add hundreds of analysed JSON files to a
  PR, the failure #326 recorded. `g05_reference` strips the recorded
  `tests/functional/baselines` prefix and resolves it under the new root, so the
  receipts still prove every capture's digest chain. The three live
  `docs/testing/reference-manifest*.json` files carry the new binary paths.
- **Captures are byte-identical.** Every capture, receipt and binary moves with
  `git mv`; `git ls-tree` blob ids at `origin/main` and at the head match for
  every moved file that is not Rust source.
- **The suite needs nothing from #448.** The only library items it names are
  `clock::TestRequest`, `core::pointer::PointerConfig` and `shell::StatusLine`
  (plus `metrics`, `runtime` and `core::event` in two cases). It now imports them
  as `termrock::` through the facade, so the crate follows whatever layout #444
  to #448 give the facade. #451 does not need to wait for #448.
- **Shared harness.** `support.rs` and `bins.rs` are included by `apps/jackin`
  and `crates/functional-cross` through `#[path]`, and the 37 `j*` cases by
  `apps/jackin`. Those paths point into `crates/termrock-functional` once the
  sources move.
- **Staging.** One branch per step, each at about 40 analysed files:
  `-a` routes every baseline lookup through `support.rs` (no move), `-b` moves
  the captures and reference binaries with the guard pins, `-c` creates the
  crate and moves the nine top-level files, `-d` to `-f` move the 124 non-`j`
  cases, `-g` moves the 37 `j` cases. The moved `.pb` files and executables
  are not analysed.

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

## 2026-11-18: Where the ui and widgets cycle fixes landed (#442)

Context. #442 breaks every upward edge between `ui` and `widgets`. Three of its
instructions did not survive contact with the layering scan, so the code
follows the evidence and this entry records the differences.

- `ui::columns` is a re-export shim. #442 moves `HeaderAction`, `HeaderLabels`
  and `header_menu` into `widgets/table.rs` and says to re-export them from
  `ui::columns`. A re-export is a reference, so a runtime file naming a grid
  item is the same upward edge the move removes. The solver now lives in
  `ui/column_solver.rs` (`pub(crate)`, runtime) and `ui/columns.rs` holds only
  `pub use` lines for both halves, mapped to the facade in `[files]` like the
  other shim modules. Every `ui::columns::*` path and catalog id is unchanged.
  #445 and #447 extract the two halves into different crates and keep this
  shim as the one place the old module is composed; #452 deletes it with the
  rest of `[files]`.
- `PickerAction` stays in `widgets/picker.rs`. #442 moves it beside
  `ui/layer.rs`, but `picker` and `layer` are both in `termrock-overlays`, so
  `LayerResult::Picker(PickerAction)` is not an edge. Only `FormAction`,
  `InfoResult` and `FileBrowserResult`, which are produced by
  `termrock-dialogs` types, moved, into `ui/layer.rs` together with
  `LayerResult`. `ui::layers` and the three widget modules re-export them.
- Specimen impls and the other moved impls went into the owning widget file,
  not into new `*_specimen.rs` files, so no `[files]` row was added. The
  acceptance line "`ui/layer.rs` has no production reference to `widgets`" is
  read as "to a higher crate": `ui/layer.rs` still names `icon_button`,
  `keyhint`, `menu` and `picker`, all in its own or a lower crate.

- The #443 PR A test `catalog_the_catalog_spelling_leaves_the_repository_ids_unchanged`
  compared the pinned inventory with the bare walk, which only holds while no
  definition has moved. Moving `LayerResult`, `FormAction`, `InfoResult`,
  `FileBrowserResult`, `HeaderAction`, `HeaderLabels`, `header_menu`,
  `OUTER_MARGIN` and `body_bounds` makes the bare walk spell them at their new
  home. The test keeps its name and its strength: `MOVED_KEEPING_SPELLING`
  lists each moved definition with the spelling the catalog keeps, and the
  pinned id set must equal the bare id set after exactly that respelling. A
  moved id that is missing from the table, or an id that vanishes or appears,
  fails the test.

No `termrock::` path, type identity or catalog id changed. The allowlist lost
every `src/ui/**` and `src/widgets/**` row and the two `ui::layers` test rows;
the core and theme rows (#441) and the two `test_alloc` rows (#449) remain.

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

## 2026-11-18: Three coherence breaks of `termrock-core`, and how they close (#444)

Context: an experiment that moved `core` into its own crate failed in
`junie-tui` with three errors that are not visibility problems. Rust's
coherence rules forbid, across a crate boundary, an inherent `impl` on a
foreign type (E0116), a foreign trait implemented for a slice of local types
(E0117), and an exhaustive `match` on a foreign `#[non_exhaustive]` enum
(E0004). Each one would have hit every later extraction that moves a type away
from its trait or its inherent impl, so the answer is a rule, not a patch.

Rule: a type, the traits whose impls are blanket over it, and its inherent
methods stay in the crate that owns the type. What the lower crate cannot name
(theme, `RenderCtx`, widget item types) reaches it through a `#[doc(hidden)]`
trait that the higher crate implements.

### ScrollPane `draw_*` (E0116)

`impl ScrollPane { draw_v, draw_h, draw_divider, draw_with_fade }` sat in
`widgets/scrollbar.rs`. `ScrollPane` stays in `termrock-core`. The four methods
now sit next to it, generic over a hidden `core::scroll::ScrollPaint` that
`RenderCtx` implements in `widgets/scrollbar.rs`:

```rust
pub fn draw_v<C: ScrollPaint>(&mut self, area, buf, ctx: &mut C, owner, focused)
```

`ScrollPaint` carries the five things the methods ask of the context (paint a
vertical bar, a divider, a horizontal bar, the fade-and-bar pair; register a
scroll-under region). The bodies call the same `render_*` functions as before,
so nothing is painted differently. Every call site, in `src/`, `apps/showcase`
and the tests, is unchanged (`pane.draw_v(..)`), needs no import and passes a
`&mut RenderCtx` as before.

Alternatives, with the evidence that decided it:

- Relocate `ScrollPane` to the crate that draws it. Rejected. Core needs it:
  `Scrollable::pane() -> &mut ScrollPane` and `pointer::scroll_input` are core,
  and about 400 lines of `ScrollPane` are pointer, track and column logic that
  reads private `ScrollState` fields. Moving it takes `Scrollable`,
  `scroll_input` and the tests with it and leaves the five-module core cycle
  (`hit`, `intent`, `pointer`, `scroll`, `shape`) cut in two.
- An extension trait in the drawing crate. Rejected. The four methods stop being
  `core::scroll::ScrollPane::draw_*`: four ids change and a trait id appears,
  the digest of `tests/regressions/api-ids.sha256` changes, and the 23 call
  sites (apps and tests included) each need `use ...::ScrollPaneDraw`. A
  published method would become a method that needs an import. The constraint
  was "unchanged unless unavoidable"; it is avoidable.
- Move the drawing into core. Rejected: it paints with the theme and
  `Interaction`, which sit above core.

Consequence for #445 and #446: `impl ScrollPaint for RenderCtx` must live in
the crate that owns `RenderCtx` (the orphan rule), and `termrock-runtime` is
below `termrock-controls`, where `scrollbar.rs` goes. When `RenderCtx` moves to
`termrock-runtime`, the painting functions `render_vertical`,
`render_horizontal_with`, `render_divider` and `render_with_fade_except`, with
their `thumb_style` helper, move with it and `widgets::scrollbar` re-exports
them. They use only `RenderCtx`, the theme and `ui::fade`, all in runtime or
below. The ids keep their spelling through the canonical-spelling pin of #443.

### Row slices (E0117)

`impl RowSource for [ListItem]`, `for [FlatRow]` and `impl<T: AsItem> RowSource
for [T]` cannot stay in the crates of `ListItem`, `FlatRow` and `AsItem`: a
slice is never local. They become one blanket impl in core,
`impl<T: RowItem> RowSource for [T]`, where `RowItem` is a hidden trait with
`label`, `enabled` and `key`. `ListItem` and `FlatRow` implement `RowItem` in
their own files, with the bodies the slice impls had.

`AsItem` is documented API with a Showcase proof ("any row type feeds the shared
row source"), so a user type that implements `AsItem` must keep getting
`[T]: RowSource`. For that, `AsItem` and the `Item` it returns move into core
(`core::item`, a pure move of the struct and its builders), `impl<T: AsItem>
RowItem for T` lives there, and `widgets::option_list` re-exports both.
`MenuItem` still implements `AsItem` in `menu.rs`. `RowItem` is not public API.

Alternative rejected: keep `AsItem` in the widget crate and give it `RowItem` as
a supertrait. It compiles every in-tree use, but a user type that implements
`AsItem` today would stop compiling until it also implemented a hidden trait.
The coherence of the three-impl layout (core blanket over `AsItem` and
`ListItem: RowItem` in a downstream crate) was checked by the compiler; a
downstream crate may implement a trait for its own type next to an upstream
blanket because it owns the type.

### DismissReason (E0004)

`DismissReason` stays `#[non_exhaustive]`, which is a published promise.
`Dismiss::allows` matched it exhaustively in `ui/popup.rs`. The match moves into
core as a hidden `const fn DismissReason::permitted_by(escape, outside_click,
focus_out)` and `Dismiss::allows` calls it, so a new variant still fails to
compile, now in the crate that owns the enum. A wildcard arm would have given a
future variant a silent default; dropping `#[non_exhaustive]` would have changed
the public contract.

### `PinnedKeys` and `CheckGlyphs` (the #441 note)

Both are now plain `#[doc(hidden)] pub` traits in core with no private module,
as the #441 decision said they must be once core and theme are separate crates.
The inventory walker skips `#[doc(hidden)]` items (#444, `tests/support`), so
the declared-API id set does not change.

### Everything else that crossed the boundary

Items that `junie-tui` reaches in core became `#[doc(hidden)] pub` (compiler
driven, no other edit): the rest are `pub(crate)` helpers of `hit`, `keyed`,
`pointer`, `event`, `scroll` and `Item::payload_bytes`. None joins the
declared API.

The test module `core/pointer/nested_scrollbar_tests.rs` imports `ScrollView`,
the scrollbar and the theme, so it cannot sit in core. It stays at its path,
declared from `src/core/mod.rs`, until #446 moves it to `termrock-viewport`.

## 2026-11-18: The line-edit fields sit in `termrock-core::field` (#444)

`widgets::{edit, field_common}` and the crate-private `text_select` move into
`termrock-core` as the `field` module, as the issue says. `widgets::edit` and
`widgets::field_common` keep their paths through a `pub use` in
`src/widgets/mod.rs`. `text_select` was `pub(crate)`, so `field::text_select`
is `#[doc(hidden)]` and `junie-tui` re-exports it `pub(crate)`; its items that
the panel, viewport and terminal views reach are `#[doc(hidden)] pub` and listed
in `tests/regressions/doc-hidden-items.txt`.

The test `text_revision_follows_undo_and_redo` left `termrock-text` for lack of
`core::edit`. It is now `crates/termrock-core/tests/regressions/text_buffer.rs`,
a `regressions` target of `termrock-core` that nothing else in the workspace
needs to know about (`main.rs` holds one `mod` line per file, as the root
target does). The `[files]` entry of `src/core/text.rs` is gone: the file is a
one-line `pub use` of `termrock-text`.

## 2026-11-18: `termrock-theme` takes `RowState` with `Theme::change_glyph` (#444)

`impl Theme { change_glyph(&self, RowState) }` sat in `widgets/grid.rs`: an
inherent impl on a type of another crate once `Theme` is its own crate (the same
E0116 as `ScrollPane::draw_*`, see "Three coherence breaks of `termrock-core`").
`RowState` is a five-variant enum with no behaviour of its own, so it moves into
`termrock-theme` beside `change_glyph`, following the earlier move of
`VisualState` and `CardHeading` (#441). `widgets::grid::RowState` re-exports it,
and the ids `widgets::grid::RowState*` and `theme::Theme::change_glyph` keep
their spelling through the canonical-spelling pin of #443; the id digest does
not change. A trait would have changed the id and needed an import at every
call (`t.change_glyph(row)`).

`termrock-theme` depends on `termrock-core` and `ratatui` only; it implements
core's hidden `PinnedKeys` (for `HintBarStyle`) and `CheckGlyphs` (for `Glyphs`),
which is the reason those traits are `#[doc(hidden)] pub` and not private
(#441). `cargo tree -p termrock-theme -e normal` lists `termrock-core`,
`termrock-metrics` and `termrock-text` and nothing above them.



## 2026-11-18: `termrock-runtime` and the scroll-bar painters (#445)

`termrock-runtime` holds `RenderCtx`, `reveal`, `tooltip`, `popup`, `measure`,
`row`, `fade`, the `Specimen` trait and the column solver. It depends on
`termrock-{metrics,text,core,theme}` and `ratatui`, and on no other package.
Choices that differ from the issue's wording or settle what it left open:

- **The column solver is `termrock_runtime::columns`.** #442 left the solver in
  `ui/column_solver.rs` and turned `ui/columns.rs` into a shim over it plus the
  three table items. The solver file moves as `columns.rs`, as the move map
  says; `ui/columns.rs` stays in `junie-tui` and now re-exports
  `termrock_runtime::columns`. `widgets/{grid,table,picker}.rs` import
  `crate::ui::columns` instead of the old `pub(crate)` `column_solver`.
- **`impl ScrollPaint for RenderCtx` moves with `RenderCtx`.** The orphan rule
  (see "Three coherence breaks of `termrock-core`") puts the impl in the crate
  that owns `RenderCtx`. The impl calls the `render_*` painters, so those move
  too: `id_for`, `h_id_for`, `render_divider`, `render_vertical`,
  `render_horizontal`, `render_horizontal_with`, `render_with_fade` and
  `render_with_fade_except` now live in `termrock_runtime::scrollbar`, and
  `widgets::scrollbar` re-exports each at its old path. The geometry, hit-test
  and `Bars` code stays in `widgets/scrollbar.rs` (`controls`). Nothing is
  painted differently; every catalog id keeps its spelling and the digest does
  not change.
- **`ui::fitting` stays crate-private to `junie-tui`.** The issue says to make
  its items `pub` + `#[doc(hidden)]`. `removal_order` and the module are, but
  `ui/mod.rs` re-exports the module with `pub(crate) use`, so no new path
  appears under `termrock::ui`.
- **Hidden plumbing.** Everything the moved code kept `pub(crate)` and a
  higher crate still uses is `#[doc(hidden)] pub` and listed in
  `tests/regressions/doc-hidden-items.txt`: `RenderCtx::{push_shift,
  pop_shift, note_render_mutation, render_mutated}`, `ctx::{Shift,
  total_shift, scratch_buffer, copy_cells}`, `measure::terminal_extent`,
  `fitting::removal_order`, and the reveal helpers `OVERFLOW`, `overflow_id`,
  `overflow`, `overflowed`, `cuts_len`, `scroll_cuts_since` and
  `clip_cuts_since`. `RenderCtx::shifts` stays private; `ui/scroll_view.rs`
  pushes and pops through `push_shift` and `pop_shift`, which keeps the field
  and the `Vec` out of the interface. No item of the facade gains an id.
- **No other coherence break.** `RenderCtx`, `Reveal`, `Tooltips`, `Placement`
  and the other moved types have no inherent impl and no foreign-trait impl
  outside the moved files. The only `impl` for a moved type outside it is
  `impl From<FramePlacement> for Placement` in `ui/modal.rs`, which stays legal
  because `FramePlacement` is local there.

## 2026-11-19: The generated-workflow snapshot entry cap rises to 16384 (#505)

Context: `scripts/check-generated-workflows.sh` rejects a snapshot tree of more
than `maximum_snapshot_entries` entries. The cap was 8192. After the crate
split (#451) moved about 2,000 baselines and reference binaries into
`crates/termrock-functional`, main tracks 11179 files, so the local
`pre-commit` guard failed on every commit made against main. CI does not run
the guard, so nothing else caught it.

Options: (a) leave the frozen baselines and reference binaries out of the
snapshot; (b) raise the cap.

Decision: (b). The cap is 16384, and the trusted-seed metadata bound is now the
literal 16384 instead of `2 * maximum_snapshot_entries`.

Why not (a): the guard does not count the snapshot as a convenience. It lists
the whole tree and validates every entry (path safety, no gitlinks, supported
modes, symlink targets, blob sizes, the approved large-blob allowlist, which
already names the reference binaries under `crates/termrock-functional`)
before it materializes anything. Skipping subtrees would make that validation
and the materialized tree depend on a path list, which is a new thing to keep
honest and a way to hide an unsafe entry under an excluded prefix. The
generator also reads the snapshot as one tree.

Why (b) is safe: the entry cap is only a resource bound. It sits beside the
8 MiB listing-byte cap, the 16 MiB per-blob cap and the 512 MiB total cap, and
those stay as they are. Main's listing is about 1.9 MB and its blobs total
about 238 MB, so 16384 entries (1.47 times main) fits well inside the other
three. The trusted-seed bound protects a reviewed, fixed pack of 539 objects
and would have been loosened as a side effect of the old derived formula, so
it keeps its previous value of 16384 as an explicit number.

Pins: the checker digest moved in `scripts/install-generated-workflow-guard.sh`
(`reviewed_checker_sha256`), `scripts/policy/check-generated-workflow-auth.py`
(`qualified_checker_sha256`) and
`scripts/policy/check-generated-workflow-auth-mutants.py` (`expected_checker`).

Tests: `scripts/policy/check-generated-workflows.sh snapshot-limits` now
builds a 16385-entry tree and expects the entry-count rejection, so the new
cap is pinned at its upper edge. A positive test (a tree between 8193 and 16384
entries reaches the Rustup boundary) was tried and dropped: it takes about
six minutes because the guard materializes every entry, and it cannot pass
until the separate jactionlint support-policy mismatch below is fixed.

Not fixed here: with the cap raised, `pre-commit` on main still fails at
"snapshot tool tuple is outside the reviewed support policy". `mise.toml` pins
jactionlint 2.6.0 (Renovate, #464 era) but the reviewed tuple table in the
checker only admits jactionlint 2.4.0 for Velnor 0.1.78. That table is a
reviewed allowlist, so it needs its own qualification and issue.

## 2026-11-18: `termrock-session` holds the session, not the run loop (#445)

The issue puts `src/runtime.rs` whole into `termrock-session`. That cannot build.
`src/shell/app.rs:463` has `impl<A: App> Application for A`, so every `App` is
an `Application` and `Shell` runs through `runtime::run`. With `Application` in
`termrock-session` and `App` in `termrock-shell` the impl is neither local to
the trait's crate nor to the type parameter's, and rustc rejects it with E0210
(an uncovered type parameter). Two attempts failed: the direct blanket impl,
and a blanket impl over a hidden marker trait in a lower crate.

Decision: `Application` and the code that is generic over it stay in
`junie-tui`, in `src/runtime.rs`, until #448 moves them into `termrock-shell`
next to `App`. Both traits are then in one crate and the blanket impl is legal.
`termrock-session` takes everything that does not name `Application`:

- `clock`; `TerminalSession`, `SessionTerminal` and the terminal-mode restore;
  `job_control`; the panic bridge;
- the external channel (`ExternalPolicy`, `ExternalSender`, `ExternalReceiver`,
  `ExternalSendError`, `ExternalTurn`, `external_channel`,
  `bounded_external_channel`);
- the raw-input side: `RawInputPolicy`, `Coalesce`, `SizeWatch`,
  `FocusClickGuard`, the raw-event sources and budget, `scan_next`,
  `drain_pending_input` and `drain_pending_input_bounded`.

`junie-tui` keeps `Application`, `run`, `pump_external`, `InputPump`,
`drain_ready_inputs`, `DRAIN_BUDGET`, and the private loop (`event_loop`,
`loop_turn`, `service_turn`, `input_turn`, the frame painter). `junie-tui`
re-exports every moved item at its old `runtime::` path, so no
`termrock::runtime::*`, `junie_tui::runtime::*` or `termrock::clock::*` path,
type identity or API id changes, and no `MOVED_KEEPING_SPELLING` row is needed:
the catalog spells each moved item by the same `runtime::` path. The layering
table maps `src/runtime.rs` to `shell`, its future crate, and the architecture
doc says the run loop belongs to `termrock-shell`.

Consequences:

- `termrock-session` does not depend on `termrock-runtime`: the three calls the
  loop makes into `reveal` and `tooltip` are in the loop code, which stays up.
  The `session` row of `docs/layering.toml` keeps `runtime` in its allowed list.
- Items the loop and its tests need from the session crate were private. They
  are `#[doc(hidden)] pub` and listed in `tests/regressions/doc-hidden-items.txt`:
  `RawBudget` (with `new`, `read`, `filtered`), `RawRead`, `RawSource`,
  `CrosstermSource`, `scan_next`, `RawModeControl`, `discard_pending`,
  `discard_ready`, `poll_uninterrupted`, `RawInputPolicy::event_limit`,
  `ExternalPolicy::normalized`, `ExternalReceiver::wait_latency` and the
  `metered` module (feature `work-metrics`, used by the #56 tests). #448 makes
  the loop-only ones private again where it can.
- The #56 tests `runtime::raw_input_tests::work::*` drive the loop through
  `super::*` and `crate::metrics::harness`, which stays in `junie-tui` until
  #449, so they stay with the loop in `src/runtime/raw_input_tests/work.rs`.
  The `performance` nextest profile and `performance-inventory.tsv` select the
  same 168 tests as before. The rest of `raw_input_tests`, `external_tests`,
  `render_pending_tests` and `draw_lag_tests` also stay: each drives an
  `Application`. The panic-bridge, job-control and session tests
  (`runtime/tests.rs`, `job_control/tests.rs`, `panic_bridge/tests.rs`,
  `isolated.rs`) move with the code and run in the `termrock-session` binary.

Rejected options:

- **Move the whole of `runtime.rs` and `shell` together now.** It builds only
  after #448 has moved `shell`, so `split/445-b` would carry #448's content and
  could not be reviewed or merged alone, and the stack would no longer build at
  each branch.
- **Require an explicit `impl Application for T` for every app.** It would
  satisfy the orphan rule, but it breaks every call site that relies on
  `App` implying `Application`, in `Shell`, the four applications and the
  consumer fixtures, which the epic forbids.

## 2026-11-18: The base component crates (#446)

`termrock-controls`, `-feedback`, `-overlays` and `-viewport` are extracted in a
stack of branches, one crate each (overlays in two). Each crate depends only on
the crates the layering table allows, and `junie-tui` re-exports each module at
its old path (`widgets::*`, `ui::*`), so no `termrock::` path, type identity or
catalog id changes. The rules from "Three coherence breaks of `termrock-core`" and
the #445 entries were applied up front: every `impl` in the moved files and every
`impl` of a moved trait or type in the files that stay was listed before the move.

- **No coherence break arose.** The scan found no inherent `impl` on a moved type
  outside its file (E0116), no foreign trait implemented for a slice or other
  foreign type (E0117), no exhaustive match on a foreign `#[non_exhaustive]` enum
  (E0004), and no blanket impl over a trait that sits in a different crate
  (E0210, the #445 case). The impls that cross a boundary are the legal kind: a
  type of `junie-tui` (`Dialog`, `Select`, `ListBox`, `TextInput`, ...) implements
  a trait of a lower crate (`Overlay`, `Layer`, `ContextSource`, `EditMenuHost`,
  `ViewportOps`). The one blanket impl, `impl<O: Overlay> Modal for O`, has both
  traits in `termrock-overlays`. The compiler agreed; no coherence fix was
  needed, so no trait had to be hidden or moved. (The one `MOVED_KEEPING_SPELLING`
  row below comes from how the catalog spells a re-exported id, not from
  coherence.)
- **Hidden plumbing, not new API.** Items that stayed `pub(crate)` and that a
  higher crate or a `junie-tui` test still reaches are `#[doc(hidden)] pub` and
  listed in `tests/regressions/doc-hidden-items.txt`. Every one was found by the
  compiler; none is re-exported by the facade.
- **`termrock-controls`.** `widgets::scrollbar` keeps its geometry, hit-testing
  and `Bars`; the painters and `impl ScrollPaint for RenderCtx` are in
  `termrock-runtime` since #445, and the file re-exports them. `record_work!`
  needs no feature of its own beyond `termrock-metrics/work-metrics`, which the
  crate's `work-metrics` feature forwards.
- **The #443 scratch-crate fixtures moved a file that has moved.** Two catalog
  tests (`catalog_a_scratch_move_into_a_crate_...`, `catalog_a_move_that_drops_its_
  reexport_...`) put `src/widgets/brand.rs` into a scratch crate and leave a
  `pub use` behind. `brand.rs` now lives in `termrock-controls`, so the fixture
  moves it from there, and the facade reaches it as `widgets::brand` through the
  re-export, which is the chain a real extraction produces. The tests assert the
  same things.
- **`termrock-feedback`.** It depends on `termrock-controls` for `empty -> button`
  and on nothing above it. `Button::intrinsic_width` and `button::place_lines`,
  which `empty` calls, became hidden `pub` in `termrock-controls`. The
  `pub(crate) use termrock_runtime::fitting` line of `src/ui/mod.rs` had one
  user left, `statusbar`, which now imports `termrock_runtime::fitting` itself,
  so the line is gone (the module stays `#[doc(hidden)]` in `termrock-runtime`).
- **`termrock-overlays` holds `picker` with the layer stack.** The issue cuts
  `layer`, `modal`, `context`, `edit_menu`, `option_list`, `menu`, `completion`
  and `row_menu` from `option_editor` and `picker`. `layer.rs` names
  `PickerAction` in `LayerResult::Picker`, so a crate with `layer` and without
  `picker` would depend on `junie-tui`. The cut is now: this branch moves
  everything but `option_editor` (nothing in the crate uses it), the next moves
  `option_editor`. The duplication risk of `picker.rs` (3.4k lines) moves from
  the second PR to the first; run Sonar on a draft of this branch.
- **`menu` and `option_list` stay one crate.** `option_list` calls
  `menu::reason_room` and `menu` uses `option_list`; they are both here.
- **Paths that named moved files.** `scripts/rewrite-regression-paths.py`
  rewrote the 26 `regression_test:` paths of the picker tests. The catalog tests
  `catalog_regression_paths_follow_a_moved_test_file` and `..._left_stale_fails`
  and the `UNRELATED_TEST` constant name `crates/termrock-overlays/src/{picker,
  layer}.rs`, and `tests/picker_catalog.rs` reads the two sources from there.
  The Showcase page `popups_tooltips` prints `src/ui/modal.rs` and
  `src/ui/popup.rs` as screen text; it is left alone, because a changed string
  would change the baselines.
- **One respelled id.** `LayerTag` is defined in `layer.rs` and also re-exported
  as `shell::LayerTag`. In another crate the bare walk prefers that item
  re-export to the `ui::layer` module re-export, so
  `catalog_the_catalog_spelling_leaves_the_repository_ids_unchanged` needed the
  row `("shell::LayerTag", "ui::layer::LayerTag")` in `MOVED_KEEPING_SPELLING`.
  The catalog line and the id digest keep `ui::layer::LayerTag`.
- **Hidden plumbing.** `context::row_rect`, `layer::layer_out`,
  `ContextMenu::contains_id` and `modal::Frame::inner_width` are used by the
  dialogs, lists, tables and chips that stay in `junie-tui` until #447 and #448.
- **`option_editor`** is the last file of `termrock-overlays`. It reads
  `option_list`, `edit` and the layer stack, all in the crate or below.
  `ChoiceEditor::contains_id` is hidden `pub` for the grid.
- **`termrock-viewport` and the tests that cross it.** `viewport` and `panel`
  embed an `EditMenu`, so the crate sits above `termrock-overlays`, as the table
  says.
  - `core/pointer/nested_scrollbar_tests.rs` moved with the crate and is a
    `#[cfg(test)]` module of its `lib.rs`; the `#[path]` declaration in
    `src/core/mod.rs` is gone. Its nextest names lose the `core::` prefix. No
    inventory, nextest profile or catalog line names it.
  - `ui/scroll_view/window_tests.rs` drives a `Select`, which sits above
    `termrock-viewport`, so it stays in `junie-tui` for #449. `src/ui/mod.rs`
    declares it by `#[path]` as `scroll_view_window_tests` and it imports from
    `crate::ui::scroll_view`. `VisibleWindow::content_rows` became a hidden
    `pub` field, because its test builds the struct by literal; consumers cannot
    use it without naming a listed hidden item.
  - `viewport_specimen` is a hidden `pub` module, because `widgets::diff` reads
    `SAMPLE_ROWS` from it until #447 moves `diff`. `widgets/mod.rs` re-exports it
    `pub(crate)`; no new path appears under `termrock::widgets`.

## 2026-11-18: The component family crates (#447)

`termrock-fields`, `-editors`, `-layout`, `-collections`, `-navigation` and `-grid`
are extracted in a stack of branches, one crate each, in dependency order. Each
crate depends only on the crates the layering table allows, and `junie-tui`
re-exports each module at its old path (`widgets::*`, `ui::*`), so no
`termrock::` path, type identity or catalog id changes. The rules of "Three
coherence breaks of `termrock-core`" were applied up front: every `impl` in the
moved files, and every `impl` of a moved type or trait in the files that stay,
was listed before the move.

- **`termrock-fields`** (`chips`, `input`, `select`). No coherence break:
  `Select` implements `Overlay` (the trait is in `termrock-overlays`, the type
  is local), which is the legal direction, and nothing implements a trait for a
  moved type from outside. `ChipBar::intrinsic_width`, `chips::overflow_run_cells`
  and `Select::contains_id` were `pub(crate)` and are used by the intrinsic-width
  tests and by the lists and dialogs that stay in `junie-tui`, so they are
  `#[doc(hidden)] pub` and listed in `tests/regressions/doc-hidden-items.txt`.
- **Features forward to every dependency that has them.** The root
  `work-metrics` feature enables each crate's feature, and a workspace build
  unifies them, which hid a gap in #446: a crate's `work-metrics` that names only
  `termrock-metrics/work-metrics` fails alone (`cargo check -p <crate>
  --all-features`) once a dependency such as `termrock-core` needs its own
  counters. Each #447 crate therefore forwards `work-metrics` to
  `termrock-metrics` and to every `termrock-*` dependency that has the feature
  (`termrock-layout` has none: it records no work), and
  `cargo check -p <crate>` passes with `--all-features`, `--no-default-features`
  and `--all-targets` on every branch. `termrock-editors` reads the line-index
  counters of `termrock-text` in its tests, so it enables `termrock-text/test-support`
  in its own `[dev-dependencies]` (see #444); nothing else in the stack does.
- **`termrock-editors` and the allocation counter.** `code/tests.rs` and
  `textarea/tests.rs` call `crate::test_alloc::measure` (seven call sites), and
  `test_alloc.rs` installs a `#[global_allocator]` that the move map assigned to
  `termrock-conformance` (#449). Editors sits below conformance, so it cannot
  depend on it, and a library cannot carry a global allocator for other crates'
  tests. Three ways were weighed against the code:
  1. *A feature-gated allocator in a lower crate* (the `termrock-text`
     `test-support` pattern of #444). Rejected: that feature only exposes
     counters, while this one installs the process allocator, and a workspace
     test build unifies the feature, so every test binary that links the crate
     would count every allocation, and a `--all-targets` build would put it into
     binaries that are not tests.
  2. *Cut the allocation-budget tests into `termrock-conformance`*, as the issue
     says. Rejected: both files are `mod tests;` of their module and import
     private items (`super::index::{BracketMap, DiagnosticIndex, ProviderKey,
     RunIndex, SegmentSnapshot, work}`, `Highlighter`, `Segmenter` and the shared
     fixtures of the file), so the cut needs several hidden `pub` items and a
     copy of the helpers; it would also leave a half-split file for #449.
  3. *Move the counter with its only users.* Chosen. `test_alloc.rs` is used by
     nothing else (`git grep test_alloc`), so it moves to
     `crates/termrock-editors/src/test_alloc.rs` as a `#[cfg(test)] mod` of the
     crate. The allocator is then installed only in the `termrock-editors` unit
     test binary; no feature, no unification, no non-test build is affected. The
     `junie-tui` test binary no longer installs it, and no test there asserts an
     allocation count. Both test files move whole, as the table in #447 lists.
  Consequences: #449 has nothing left to do for `test_alloc.rs` (its row, and the
  planned `alloc_budgets.rs`, are void); the `src/test_alloc.rs` row of
  `docs/layering.toml [files]` and the two `test_alloc` rows of
  `layering-allowlist.tsv` are gone, so the allowlist is empty and may only
  stay so. The nextest `performance` profile selects the same 168 tests as
  before (`cargo nextest list --profile performance --workspace --all-features`,
  compared before and after): none of them is an editors test.
  `viewport_specimen` stays a hidden `pub` module of `termrock-viewport`,
  because `diff` reads `SAMPLE_ROWS` from it across the crate boundary; the
  `pub(crate)` re-export in `widgets/mod.rs` is gone.
- **`termrock-layout`** (`ui::{layout, master_detail, pane_tree}`,
  `widgets::splitter`). It depends on `termrock-{controls, overlays, viewport}`
  and the foundations. Every `impl` in the four files is a foreign trait
  (`Actions`, `Specimen`) on a local type, and nothing outside them implements a
  trait for a moved type, so there is no coherence break.
  `MasterDetail::forget_geometry` is called by `list_detail`, which stays in
  `junie-tui` until the collections branches, so it is `#[doc(hidden)] pub` and
  listed in `tests/regressions/doc-hidden-items.txt`. The crate has no
  `work-metrics` feature because none of its code records work, so the root
  `work-metrics` feature does not forward to it.
- **`termrock-collections`** (`list`, `tree`, `props`, `detail_card`,
  `list_detail`, `search_detail/*`, `ui::{content, list_core}`). It depends on
  `termrock-{layout, fields, viewport, overlays, feedback, controls}` and the
  foundations, as the layering table allows. Every `impl` in the moved files is
  either an impl of a trait defined in the same file (`TreeRows for [FlatRow]`,
  `ListRows for [ListItem]`, `ListSource for KeyedVec<ListItem>`) or a foreign
  trait on a local type, so there is no coherence break; the `RowItem` impls of
  `ListItem` and `FlatRow` sit next to their types, as #444 arranged.
  - `search_detail/cost_tests.rs` stays in `junie-tui` for #449. Its
    `mod cost_tests;` declaration left `search_detail.rs`; `src/widgets/mod.rs`
    declares it by `#[path]` as `search_detail_cost_tests` (work-metrics tests
    only) and it imports from `termrock_collections::{list_detail,
    search_detail}`. Everything it reaches there was already `pub`. Its nextest
    names change from `widgets::search_detail::cost_tests::` to
    `widgets::search_detail_cost_tests::`, so the `performance` profile and
    `performance-inventory.tsv` follow that spelling (the set of 168 tests is
    unchanged).
  - `PropsList::{wrapping, row_of, height_at}` are hidden `pub` for
    `info_dialog`.
- **`termrock-navigation`** (`breadcrumb`, `rail`, `nav_list`, `tabs`, `toolbar`,
  `steps`, `document_workspace`). It depends on `termrock-{collections, overlays,
  controls}` and the foundations; `steps` reads `content` and `list_core` from
  collections. Every `impl` in the moved files is a foreign trait on a local
  type, so there is no coherence break. `Breadcrumb::{intrinsic_width,
  first_shown}` and `Toolbar::intrinsic_width` are hidden `pub` for the
  intrinsic-width and overflow-parity tests that wait for #449.
  `Breadcrumb` is defined in `breadcrumb.rs` and also re-exported as
  `shell::Breadcrumb`; in another crate the bare walk prefers the shell item
  re-export, as it did for `LayerTag` (#446), so
  `MOVED_KEEPING_SPELLING` gained `("shell::Breadcrumb",
  "widgets::breadcrumb::Breadcrumb")`. The catalog line and the id digest keep
  `widgets::breadcrumb::Breadcrumb`.
- **`termrock-grid`** (`table`, `grid`, `grid_changes`, `grid_submission`,
  `grid_specimen`, `table_specimen` and five test files), in one branch.
  `table` calls `grid::header_style` and `grid` uses `table::SortDir`, and
  `grid.rs` declares `grid_changes.rs` and `grid_submission.rs` by `#[path]`
  (and those two declare their test files the same way), so the cycle and the
  relative layout are kept inside the crate; splitting it across branches would
  not build. The crate depends on `termrock-{overlays, feedback, controls}` and
  the foundations, as the layering table allows. Its `impl`s are the trait
  families of the files (`TableRows` for the row sources of `table.rs`, foreign
  traits on local types) and the test-only `F64` helper in `table.rs`; there is
  no coherence break. The `ui::columns` shim keeps re-exporting
  `HeaderAction`, `HeaderLabels` and `header_menu` through `widgets::table`,
  which is now a re-export of `termrock_grid::table`; the ids and their
  `MOVED_KEEPING_SPELLING` rows did not change. No `pub(crate)` item needed to
  become hidden `pub`. The 34 `regression_test:` strings of the Showcase
  coverage tables that named the moved `grid_*_tests.rs` files were rewritten
  with `scripts/rewrite-regression-paths.py`. The crate carries the
  `work-metrics` feature because its code records work.

## 2026-11-19: `PropsList::wrapping`, `PropsList::height_at` and `MasterDetail::forget_geometry` stay out of the public API (#494)

The #447 split made three `pub(crate)` methods cross a crate, so they are
`#[doc(hidden)] pub` and listed in `tests/regressions/doc-hidden-items.txt`.
That needed no public-API decision and made none. #494 asked for one. Each
stays hidden. The reasons are below; promoting later needs a consumer, a catalog
line, a Showcase proof that calls it, and an id-digest update with its
justification here.

- **`PropsList::wrapping` and `PropsList::height_at`: no app builds a
  `PropsList`.** Holla, jackin and tablepro never name it. They draw property
  sheets with `props::render` and `props::measure`, which already wrap
  `Prop::wrap` values, and they show a property dialog with `InfoDialog`. The
  only callers are `InfoDialog` (`wrapping()` when it builds its list,
  `height_at` when it sizes the dialog) and one saturation test. The Showcase
  has seven `PropsList` pages and none calls either. Public items already cover
  the need that is left. A list that keeps one prop per row shows a long value
  through the public `reveal` (`RevealMode::InlineWrap` shows it on wrapped
  rows). A sheet's wrapped height is `props::measure_with(&props, width,
  sheet)`. A list that does not wrap is `props.len()` rows tall, so `height_at`
  adds nothing there.
- **`height_at` is not a contract worth freezing.** It measures a list with no
  scrollbar, and `InfoDialog` passes it `inner_w + 1`, an offset a public
  version would have to explain. `wrapping`
  changes how the cursor, the viewport and the reveal treat a prop that takes
  several rows, and only `InfoDialog` exercises that path. Making either public
  without a consumer is the shape #353 removes.
- **`Prop::wrap` now says where it applies.** The Showcase's own details list
  (`disk_cleanup`) carries a `Prop::wrap` value that a plain `PropsList` keeps on
  one row. That is the intended list behavior, and the doc comment of
  `Prop::wrap` now says so (`render`, `measure` and `InfoDialog` wrap; a
  `PropsList` reveals).
- **`MasterDetail::forget_geometry`: `layout` already does the job for an
  app.** Its comment said "a host calls this". The host is `ListDetail`, which
  skips `layout` when its area is empty. All seven app call sites (holla
  `disk`, `cleanup`, `files`; jackin `config`, `accounts`, `manager`; tablepro
  `connections`) call `layout` every frame with whatever area they have. A
  probe laid out a 120-column split, laid it out again into `Rect::ZERO`, a
  zero-width area and a zero-height area, with the default `collapse_below` and
  with `0`, then sent the resize chord. The split never moved. Before the empty
  `layout`, the same chord moved it. `forget_geometry` gave the same result.
  `master_detail_layout_into_no_space_leaves_nothing_to_resize` pins that, so
  this reason stays true. The two calls are not identical (`last()` reads
  `Master` after the first and the default `Split` after the second, and the
  chord is ignored or consumed), and a second public way to reach almost the
  same state is a trap, not a need.
- **The doc comments no longer address apps.** `forget_geometry`, `wrapping`
  and `height_at` say they are library plumbing and name the public route. The
  hidden-item rule from #444 stands: apps and examples may not use a listed
  item.
- **No id digest change.** Nothing here becomes public, so `api-ids.sha256`,
  the catalog and `doc-hidden-items.txt` are untouched. #447 already listed the
  three items in `doc-hidden-items.txt`, and `catalog_doc_hidden_items_are_a_reviewed_list`
  checks that list.

## 2026-11-18: `termrock-catalog` holds the catalog and the #14 gate (#449)

`termrock-catalog` is the catalog half of #449 (the conformance half waits for the
shell crate of #448). `junie-tui` re-exports it as `pub use
termrock_catalog::{catalog, docs}`, so `termrock::{catalog, docs}` and every id
are unchanged (`catalog_api_ids_match_the_committed_digest` passes on the
unchanged `tests/regressions/api-ids.sha256`).

- **What moved.** `src/catalog.rs`, `src/docs.rs` and `src/catalog_contract.rs`
  keep their names; `tests/support/api_inventory.rs` is `src/inventory.rs` and
  `tests/support/scenario_reach.rs` is `src/scenario_reach.rs`. The move map had no
  row for `scenario_reach.rs`; `docs/layering.toml [files]` already assigned it to
  `catalog`, and `catalog_contract` was its only user. `inventory`,
  `scenario_reach` and `catalog_contract` are `#[cfg(test)]` modules, so `syn`,
  `toml` and `sha2` are dev-dependencies and the library has none of them. The
  `#[path = "../tests/support/..."]` includes are `use crate::inventory` and
  `use crate::scenario_reach`. `tests/picker_catalog.rs` and
  `tests/terminal/session_catalog.rs` are the modules of the crate's single
  `tests/regressions` target (`termrock-catalog::regressions`); the integration
  target includes `src/inventory.rs` by `#[path]`, as `tests/picker_catalog.rs`
  did. `picker_catalog` stops being a top-level target of `junie-tui`.
- **Dependencies.** `docs.rs` is the only library code and names `core`,
  `controls`, `grid`, `collections`, `fields` and `navigation`, all inside the
  `catalog` row of `docs/layering.toml`; `catalog.rs` has no `crate::` path. The
  test-only `termrock-metrics` dev-dependency (for `Metric::ALL`) is allowed by the
  closure of `core`. No `catalog` feature exists on the crate. It declares
  `work-metrics`, which forwards to the six dependencies that declare it (the
  forwarding guard of stage B), and `junie-tui`'s `work-metrics` forwards to it.
- **The repository root.** The gate reads `docs/`, `src/`, `apps/` and
  `tests/regressions/` of the repository, so `root()` is the third ancestor of
  `CARGO_MANIFEST_DIR` (`crates/termrock-catalog` is two levels down), not
  `../..` appended to it, which would leave `..` inside every path the inventory
  keys on. The facade is still chosen as before: the root package while
  `src/lib.rs` exists, `crates/termrock/src` after it is gone. #452 must switch
  that rule to `crates/termrock` unconditionally in the same PR that rewires the
  facade (finding 2 of its plan); this PR keeps the rule so the digest is
  computed from the tree it was computed from.
- **`session_catalog` needs no `terminal` feature.** The issue asked to confirm
  it. The test parses `src/terminal/session.rs` with `include_str!` and reads the
  catalog; it links no terminal code, so it runs in the catalog crate under every
  feature set. The `include_str!` path is the one thing #448 must update when it
  moves `src/terminal/session.rs` into `termrock-terminal`; the compiler fails
  loudly until it does.
- **Layering.** The `src/catalog*.rs`, `src/docs.rs` and `tests/support/*` rows of
  `[files]` are gone, and stage A no longer scans `tests/support` (the directory
  no longer exists, and stage B checks the crate's dependencies from
  `cargo metadata`).
- **SonarCloud.** `.sonarcloud.properties` already excluded
  `crates/termrock-catalog/src/catalog.rs` from copy-paste detection (#443); it
  matches the moved file, and the old `src/catalog.rs` pattern is removed because
  the file is gone. `catalog_contract.rs` (2.6k lines), `inventory.rs` (1.7k) and
  `docs.rs` are not excluded and count as new code as renames do; they are not
  tables of alike lines, but the new-code duplication gate is the thing to watch on
  the draft.
- **Command.** The catalog command is `cargo nextest run -p termrock-catalog -p
  termrock-showcase --all-features --no-tests fail -E 'test(catalog_)'`
  (`AGENTS.md`, `docs/ci.md`). The generated `Rust / termrock-catalog` jobs run the
  crate under `--profile ci --all-features`; the source gate is slow on a loaded
  machine (several `catalog_` tests take over a minute each because every one
  rebuilds the inventory), so the crate's job is the one to watch against the
  15-minute timeout.
- **Void items of #449.** `test_alloc.rs` and `work.rs` do not move (decided in
  #447 and the #449 design); nothing of the run loop became `pub`.

## 2026-11-18: `termrock-terminal` (#448)

`termrock-terminal` holds the termpane emulator, `TerminalViewport` and the
session controller (`src/terminal.rs` is its `lib.rs`; `forward`, `guard`,
`session` and `viewport` keep their names). `junie-tui` keeps
`#[cfg(feature = "terminal")] pub use termrock_terminal as terminal;`, and its
`terminal` feature is `dep:termrock-terminal`, so `termrock::terminal::*` and
the consumer trees (`termpane` present only with the feature, never `clap`) are
unchanged. `termpane` moved to the crate as a normal dependency; the root
package keeps it as a dev-dependency because `tests/terminal_guard.rs` builds a
`termpane::DamageGrid`. The crate depends only on what the layering table
allows (`viewport`, `controls`, `runtime`, `theme`, `core`, `text`, `metrics`),
plus `unicode-width`; `libc` is not used by the moved files.

- **No hidden items, no coherence break.** Every path the files used from
  `junie-tui` already resolves in a lower crate
  (`termrock_core::field::{field_common, text_select}`,
  `termrock_viewport::{viewport::CellPos, viewport_ops}`,
  `termrock_runtime::{ctx, fade}`, `termrock_text::cells::thousands`,
  `termrock_controls::scrollbar`), so no `pub(crate)` item became `pub`.
- **`widgets::text_select`.** The `pub(crate) use termrock_core::field::text_select`
  line of `src/widgets/mod.rs` had one user left, `terminal/viewport.rs`, so the
  line is gone.
- **The facade is untouched.** `#448` planned to add `dep:termrock-terminal`
  under the facade's features. `termrock`'s `terminal` feature forwards to
  `junie-tui/terminal`, which now pulls the crate, so the facade needs no edit
  until #452 re-points its re-exports.
- **Path edits.** `tests/terminal/session_catalog.rs` reads
  `crates/termrock-terminal/src/session.rs`. The `terminal` and `jackin-host`
  consumer lockfiles gain the crate; `minimal` and `keyed-modal` do not resolve
  it.
- **`work-metrics`.** The root feature forwards as
  `termrock-terminal?/work-metrics`, so it does not enable the optional crate.
- **Feature forwarding.** `termrock-terminal/work-metrics` forwards to every
  dependency that has the feature (`controls`, `core`, `metrics`, `runtime`,
  `viewport`), so `cargo check -p termrock-terminal --all-features` builds on its
  own and does not rely on workspace feature unification.

## 2026-11-18: `termrock-cli` (#448)

`termrock-cli` holds `ColorArg`, `GlyphsArg`, `CommonArgs`, `motion_help` and
`parse` (`src/cli.rs` is its `lib.rs`). It depends on `clap`, `termrock-core`
with `features = ["cli"]` (for `Motion: ValueEnum`) and `termrock-theme`, which
is what the layering table allows (`core`, `theme`). `junie-tui` keeps
`#[cfg(feature = "cli")] pub use termrock_cli as cli;`, and its `cli` feature is
`dep:termrock-cli`, which now carries both `clap` and `termrock-core/cli`. The
root package has no `clap` dependency left. The facade and the consumer
lockfiles need no edit: no fixture resolves the optional crate, and
`termrock::cli::*` still resolves through `junie-tui`.

## 2026-11-18: `termrock-dialogs` (#448)

`termrock-dialogs` holds `dialog`, `choice_dialog`, `info_dialog`, `help`,
`form_dialog` (with its `tests.rs`), `file_browser`, `inspect_changes`, and the
two `ui` modules `layers` and `leave`. It is cut in two branches, as the issue
planned: part 1 creates the crate with `dialog`, `choice_dialog`, `info_dialog`,
`help`, `leave` and `layers`; part 2 adds `form_dialog`, `file_browser` and
`inspect_changes`. `junie-tui` re-exports each module at its old path
(`widgets::*`, `ui::{layers, leave}`), so no `termrock::` path, type identity or
API id changes.

- **One crate, no hidden items.** Every item the moved files use from a lower
  crate was already `pub` (or hidden `pub` from #446 and #447), so no
  `pub(crate)` item became hidden `pub` and `doc-hidden-items.txt` is unchanged.
  No `impl` crosses the crate boundary: each dialog implements `Layer` for a
  local type.
- **`ui/layers.rs`** is the one line `pub use termrock_overlays::layer::LayerResult`;
  it moves with the dialogs because the issue maps it there, and the spelling
  `ui::layers::LayerResult` is kept.
- **Dependencies.** `ratatui` and the twelve termrock crates the sources name
  (`collections`, `controls`, `core`, `editors`, `fields`, `layout`,
  `navigation`, `overlays`, `runtime`, `text`, `theme`, `viewport`), all
  allowed for `dialogs` by `docs/layering.toml`.
- **No `work-metrics` feature.** No file of the crate records work or reads a
  `work-metrics` counter, so the crate has no feature and the root feature does
  not forward to it; the guard added in #446 only applies to a crate that
  declares the feature.

## 2026-11-18: `termrock-shell` takes the run loop (#448)

`termrock-shell` holds `Shell`, `App`, `Page`, the router, chrome, footer,
clipboard and the reach harness (`src/shell/*`, with `mod.rs` as `lib.rs`), and
`Application` with the run loop that is generic over it (`src/runtime.rs`).
`App` and `Application` are now in one crate, so `impl<A: App> Application for A`
is legal, as #445 planned (decision "`termrock-session` holds the session, not
the run loop"). Nothing about the blanket impl changed.

- **Where the loop is exposed.** `termrock_shell::runtime` is the module of the
  old `runtime.rs`, unchanged: it declares `Application`, `run`,
  `pump_external`, `InputPump`, `drain_ready_inputs` and `DRAIN_BUDGET`, and it
  still re-exports the public `termrock-session` items by an explicit list (never
  a glob, so the hidden `termrock_session::runtime` items stay unlisted). The
  crate root keeps the shell items at their old `shell::` spelling. `junie-tui`
  has `pub use termrock_shell as shell;`, `pub use termrock_shell::runtime;` and
  `pub use termrock_shell::reach_tests;`, so `junie_tui::runtime::*` shows the
  session items and the loop items at the same paths as before. #452 can export
  `termrock::runtime` as `pub use termrock_shell::runtime;`; it needs no
  assembled module, and the facade `runtime` and `shell` paths and every API id
  are unchanged (the digest test passes).
- **`reach_tests!`** names `$crate::reach::{check, dump, SIZES}`, because
  `$crate` is `termrock-shell` and `reach` is a root module there. `check` and
  `dump` stay `pub`.
- **The #56 tests stay with the loop.** `runtime::raw_input_tests::work::*`
  (six `work_contract_runtime_*` tests) drive `service_turn`, `input_turn`,
  `paint`, `deliver_resume_resize` and the inline test fixtures, so they stay a
  child of `raw_input_tests` in `termrock-shell` as unit tests, with no hidden
  item. Their only use of the harness was `assert_exact`, a four-line pure
  function over `WorkDelta::check_exact`; `work.rs` has a local copy. The
  conformance `harness::assert_exact` is untouched. `termrock-shell` must not
  depend on `termrock-conformance`, even as a dev-dependency: conformance
  depends on shell, and a unit test that passes a shell type to the harness
  would see two copies of the crate (E0277). #449 drops `work.rs` from its list.
- **The performance profile.** The operand is now
  `binary_id(=termrock-shell) & test(/^runtime::raw_input_tests::work::/)`, the
  inventory row names the same binary id and the new source path, and
  `docs/performance-contracts.md` follows. `cargo nextest list --profile
  performance` selects the same 168 tests; only the binary id of those six
  changed. The required `rust-termrock-shell` job (generated) runs them under
  `--all-features`, which `performance_contracts_run_in_required_all_features_ci_jobs`
  demands.
- **The session plumbing stays hidden `pub`.** The issue asked to make the
  loop-only hidden items of `termrock-session` private again where possible. None
  can be: the loop is still in a different crate, and every one of `RawBudget`
  (`new`, `read`, `filtered`), `RawRead`, `RawSource`, `CrosstermSource`,
  `scan_next`, `RawModeControl`, `discard_pending`, `discard_ready`,
  `poll_uninterrupted`, `RawInputPolicy::event_limit`,
  `ExternalPolicy::normalized`, `ExternalReceiver::wait_latency` and `metered` is
  named by `termrock-shell` (the loop, `shell::run`, or the #56 tests). No hidden
  item is added or removed.
- **Dependencies and features.** `termrock-shell` depends on `dialogs`,
  `navigation`, `overlays`, `feedback`, `controls`, `session` and `runtime`
  (`reveal::hover_wait`, `reveal::on_pointer_wheel` and `tooltip::on_input` are
  called by the loop), plus the foundations. Its `work-metrics` feature forwards
  to every dependency that declares one (`controls`, `core`, `feedback`,
  `metrics`, `navigation`, `overlays`, `runtime`, `session`); `dialogs` declares
  none. The root feature gains `termrock-shell/work-metrics`.
- **`x02_suspend.rs`.** Its header named `src/runtime.rs` for job control; it
  now names `crates/termrock-session/src/runtime.rs` (where `job_control` is) and
  the loop's new file.

## 2026-11-19: `termrock-conformance` holds the harness and report (#449)

`termrock-conformance` is the second half of #449. This part moves
`metrics/harness.rs`, `metrics/report.rs` and `metrics/tests.rs`; the cross-crate
regression tests follow in the next part. The crate is `crates/termrock-conformance`
with `harness`, `report` and a `#[cfg(test)] mod metrics_tests` (the old
`metrics/tests.rs`), all under `work-metrics`. `Harness<A: Application>` needs
`termrock-shell`, so it cannot sit with the counters in `termrock-metrics`.

- **The facade keeps its spelling.** `src/metrics.rs` is now
  `#[cfg(feature = "work-metrics")] pub use termrock_conformance::{harness, report};`
  beside the `termrock_metrics` re-exports, so `termrock::metrics::harness::*`
  and `termrock::metrics::report::*` resolve as before and every API id is
  unchanged. `junie-tui` declares the dependency as optional and enables it from
  `work-metrics` (`dep:termrock-conformance`, `termrock-conformance/work-metrics`);
  no library crate depends on `termrock-conformance`, and `termrock` reaches it
  only through `junie-tui`'s `work-metrics`. `docs/layering.toml` gains
  `conformance` in the `junie-tui` row of `[allow]`: `junie-tui` is the
  transitional implementation of the facade, whose row already lists it, and
  #452 deletes the row with the package. `src/metrics.rs` moves from `conformance`
  to `termrock` in `[files]`, because it only re-exports now.
- **Dependencies.** Normal: `ratatui`, `termrock-core`, `termrock-metrics` and
  `termrock-shell`, which is all the harness names. No other library crate is a
  normal dependency; the regression tests that follow add the rest as
  dev-dependencies. `work-metrics` forwards to the three that declare it (the
  forwarding guard of stage B).
- **No allocation counter.** As decided in #447 the counter is a `#[cfg(test)]`
  module of `termrock-editors`, so `termrock-conformance` has no allocator and no
  `alloc_budgets.rs`. Never install a `#[global_allocator]` in this crate's
  shared `regressions` target.
- **The performance profile.** The counter-core operand is
  `binary_id(=termrock-conformance) & test(/^metrics_tests::/)`; the inventory row
  and its six `T` lines follow, and the source file is
  `crates/termrock-conformance/src/metrics_tests.rs`. Same 19 tests; only the
  binary id and the module prefix changed.
- **Coverage strings.** Ten `regression_test:` strings of
  `apps/showcase/src/coverage.rs` (the `metrics::harness`, `metrics::measure`,
  `metrics::reset`, `metrics::snapshot` and `metrics::suspended` exceptions) named
  `src/metrics/tests.rs`; `scripts/rewrite-regression-paths.py` rewrote them.
- **Tests.** The unit tests implement `Application` for local types
  (`LabelApp`, `CountingApp`), which stays legal; never add `impl App for` them
  (the blanket impl of `termrock-shell` would then overlap).

## 2026-11-19: the cross-crate cost and intrinsic tests move to `termrock-conformance::regressions` (#449)

Seven test files of `junie-tui` moved to `crates/termrock-conformance/tests/regressions/`
with `git mv`, as modules of one shared `regressions` target:
`ui/intrinsic_tests.rs` is `intrinsic.rs`, `ui/intrinsic_wide_tests.rs` is
`intrinsic_wide.rs`, `ui/overflow_parity_tests.rs` is `overflow_parity.rs`,
`ui/overflow_work_tests.rs` is `overflow_work.rs`,
`ui/scroll_view/window_tests.rs` is `scroll_view_window.rs`,
`widgets/keyed_cost_tests.rs` is `keyed_cost.rs` and
`widgets/search_detail/cost_tests.rs` is `search_detail_cost.rs`. The three that
count work (`overflow_work`, `keyed_cost`, `search_detail_cost`) are
`#[cfg(feature = "work-metrics")]`, as their `mod` lines were.

- **No hidden items.** Every item the files use from another crate was already
  `pub`, so nothing became `#[doc(hidden)] pub` and `doc-hidden-items.txt` is
  unchanged. The `crate::...` paths of the facade became the owner crate's own path
  (`crate::widgets::select` is `termrock_fields::select`, `crate::ui::ctx` is
  `termrock_runtime::ctx`, and so on); no file reaches the facade, so
  `termrock-conformance` does not depend on `junie-tui` or `termrock`, which
  would close a cycle with the facade's `work-metrics` edge.
  `overflow_parity.rs` writes `termrock_metrics::record_work!`; the macro was
  already at the root of `termrock-metrics`.
- **Dev-dependencies.** The regression target needs the component crates
  (`collections`, `controls`, `feedback`, `fields`, `layout`, `navigation`,
  `overlays`, `runtime`, `text`, `theme`, `viewport`), all of which `conformance`
  may depend on in `docs/layering.toml`. They are dev-dependencies, so the
  library of `termrock-conformance` still holds only the harness and the report.
  The crate's `work-metrics` forwards to every one of them that declares the
  feature, so `cargo nextest run -p termrock-conformance --all-features` (the
  generated `Rust / termrock-conformance` job) counts work in all of them
  without relying on workspace feature unification.
- **The performance profile.** The three operands are now
  `binary_id(=termrock-conformance::regressions) & test(/^overflow_work::/)`,
  `... & test(/^keyed_cost::/)` and `... & test(/^search_detail_cost::/)`; the
  inventory rows and their `T` lines carry the module names without the old
  `ui::` and `widgets::` prefixes and the new source paths. The same 16 tests
  run. Together with the metrics rows of the previous part, `cargo nextest list
  --profile performance` selects the same 168 tests; only their binary ids and
  module prefixes changed.
- **Coverage strings.** Three `regression_test:` strings of
  `apps/showcase/src/coverage.rs` (the `ScrollView` window helpers) named
  `src/ui/scroll_view/window_tests.rs`; `scripts/rewrite-regression-paths.py`
  rewrote them.
- **No allocator.** The shared target installs no `#[global_allocator]`; these tests
  assert counted work, not bytes (see the #447 decision).

## 2026-11-19: the #56 gate, the budgets and the default-branch guard move to `termrock-conformance` (#449)

The root tests that #450 mapped to `conformance` ("450-g") move here, with the
rest of the crate they belong to, so the #56 gate runs from one crate:
`tests/regressions/{performance_inventory,work_budgets,work_measurements}.rs`,
`work_budgets/{scroll_panel,terminal}.rs`, `performance-inventory.tsv` and
`tests/ci_default_branch.rs` are now modules of
`crates/termrock-conformance/tests/regressions/` (`ci_default_branch` stops being
a top-level target of `junie-tui`, so the generated workflows lose one test
target). Every file is `git mv`'d; `work_budgets*.rs` and `work_measurements.rs`
name the owner crates instead of `junie_tui::`.

- **`performance_inventory.rs` is parametrised by one constant each.** Its
  `root()` is the third ancestor of `CARGO_MANIFEST_DIR` (the crate sits two
  levels below the repository), `INVENTORY_FILE` and `MEASUREMENT_FILE` name the
  files in the crate, and the `performance-measure` profile check, which hardcoded
  `binary_id(=junie-tui::regressions)`, now names
  `binary_id(=termrock-conformance::regressions)`. The synthetic cases of
  `gate_logic` follow: the stale-job case removes `rust-termrock-conformance`
  from the workflow, and the "filter does not select" case edits the
  `termrock-conformance::regressions` binary.
- **The inventory.** Every `junie-tui::regressions` row (the library budgets, the
  two `ScrollPanel` and terminal budget files, the measurement-sampling check and
  this gate) is `termrock-conformance::regressions` now, and the source column
  points into the crate. The `T` lines keep their names, so no required test
  changed. `junie-tui` no longer owns a required test, so
  `performance_contracts_run_in_required_all_features_ci_jobs` no longer demands
  `rust-junie-tui`; it demands `rust-termrock-conformance`, which the generator
  produces (`nextest run --profile ci --package termrock-conformance
  --all-features`) and `Required` waits for. Under that job the contracts run with
  the counters on, because the crate's `work-metrics` forwards to every library
  crate the tests drive.
- **The nextest profiles.** Every `binary_id(=junie-tui::regressions)` operand of
  `[profile.performance]`, and the filter of `[profile.performance-measure]`, name
  `termrock-conformance::regressions`. `cargo nextest list --profile performance
  --workspace --all-features` selects the same 168 tests as the base of this
  stack; the set differs only in the binary ids and in the module prefixes of the
  tests that moved in the two earlier parts of #449.
- **Coverage strings.** Five `regression_test:` strings of
  `apps/showcase/src/coverage.rs` (the `metrics::report::*` exceptions) named
  `tests/regressions/work_budgets.rs`; `scripts/rewrite-regression-paths.py`
  rewrote them, and `catalog_` is green.
- **Terminal budgets.** `work_budgets/terminal.rs` drives `termrock-terminal`,
  which is a dev-dependency here without a feature gate: it is a library crate
  like the others and the facade's `terminal` feature has no role in this crate.
- **Timing reports.** `work_measurements.rs` falls back to
  `<workspace>/target` when `CARGO_TARGET_DIR` is unset (the crate is two levels
  down), and `scripts/measure-performance.sh` runs `-p termrock-conformance --test
  regressions`. `docs/performance-contracts.md`, `docs/ci.md`,
  `docs/functional-testing.md` and `docs/dependencies.md` carry the new paths and
  commands.
- **Dev-dependencies.** `serde_json`, `syn` and `toml` (the gate reads the nextest
  config, the generated manifest and `.velnor/config.toml`) are dev-dependencies
  of the crate at the pins `junie-tui` used; `junie-tui` keeps `syn` and `toml` for
  `layering`.

## 2026-11-19: The root becomes a virtual workspace and `junie-tui` a re-export shim (#439, #452)

Context: the root package `junie-tui` held the library. After #444 to #451 every module lives in a
`crates/termrock-*` crate, the tests, the consumer fixtures and the functional suite live under
`crates/`, and the root `src/` held only `pub use` lines. `docs/api-contract.md` promises that
`junie_tui::` paths keep compiling and that removing the implementation names is a versioned decision.
Two repositories depend on `junie-tui` by git rev (`jackin-project/jackin`, whose `xtask` baseline check
requires the `junie-tui` and `jackin-console` pins to match, and `donbeave/essential-mise`). Cargo
refuses two packages named `junie-tui` in one workspace, so the shim and the virtual root cannot land
apart.

Decision:

- **The root `Cargo.toml` is a virtual manifest** with `resolver = "3"` set explicitly (a virtual manifest
  does not infer it from an edition). It has no `[package]`, `[lib]`, `[features]` or `[dependencies]`, and
  the repository has no `src/`, `tests/` or `examples/`. `layering` asserts it.
- **`crates/termrock` depends on the 22 library crates itself** and carries the features the root package
  held (`cli`, `terminal`, `product`, `work-metrics`; default empty). `core`, `metrics`, `ui` and `widgets`
  are explicit `pub use` lists, never a glob over a crate, so the `#[doc(hidden)]` items listed in
  `crates/termrock-catalog/tests/regressions/doc-hidden-items.txt` (and `termrock-core::{field, item}`)
  stay out of the facade. `core::text` is `pub use termrock_text::buffer as text`. The id digest
  `api-ids.sha256` is unchanged by this: the file only moved.
- **`junie-tui` stays as `crates/junie-tui`**: `pub use termrock::*;`, the old defaults `cli` and
  `terminal`, and every feature forwarded to `termrock`. It holds no code and has no `#[deprecated]`
  attribute (an attribute on a glob does nothing, and wrapping each module would break consumers built with
  `-D warnings`). Nothing in this workspace depends on it, and `layering` fails an edge to it. Its
  `regressions` target holds the identity test. Removing it, or adding the attribute, is a later versioned
  decision.
- **The `junie-tui:fence` and `junie-tui:advance-clock:` protocol strings, and the screen text that says
  `junie-tui`, are not renamed.** They are runtime protocol and baseline content.
- **The #14 walker takes `crates/termrock/src` as the facade always.** The earlier rule read the root
  `src/lib.rs` while it existed; with a leftover root `src/lib.rs` it would walk the wrong tree.
- **The facade's `prelude` is spelled by the catalog where the items are defined** (`core::id::ItemKey`
  and the eight others). The prelude's paths now resolve inside the walk, so the bare walk would spell
  them `prelude::`; `MOVED_KEEPING_SPELLING` names the nine.
- **Stage A of `layering` is deleted with the last module of the single package**, together with `[files]`
  and `layering-allowlist.tsv`. Stage B stays, and now also covers the shim (nothing depends on it; it must
  forward every feature of `termrock`).

Root tests (#450). The tests that were left at the root move to the crate they import: `clipboard` to
`termrock-shell`, `file_source` to `termrock-dialogs`, the six terminal files to `termrock-terminal`, and
everything that includes the shared `common` harness or reads the facade paths to
`crates/termrock/tests/regressions`. `deferred_api_calls` imports a widget of `termrock-grid` and one of
`termrock-navigation`, which are siblings that may not depend on each other even as dev-dependencies
(`layering` checks them), so it goes to the facade rather than to `termrock-grid` as #450's table said.
`terminal_guard` installs a counting `#[global_allocator]`, which now applies to the whole `termrock-terminal`
`regressions` binary; its peak measurements assume one test per process, which is how `cargo nextest` runs
them. The #14 digest and the hidden-item list moved to `crates/termrock-catalog/tests/regressions`, beside
the gate that reads them. Test names gain the module prefix of the file that held them (`terminal::` and so
on); the sorted lists differ only by that prefix and the binary id.

Consequences: the generated job set is unchanged (the same 68 `Rust /` names; the shim keeps
`Rust / junie-tui` and its product job); the shim's task key moves from `root` to `crates/junie-tui`. The
`nextest` filters that named `junie-tui` binary ids already name `termrock-conformance`,
`termrock-shell` and `termrock-functional` (#449, #451).

Rejected: removing `junie-tui` now (it breaks both git consumers at their next bump and needs consumer pull
requests that do not exist); keeping the package at the root next to a virtual manifest (impossible, a
virtual manifest has no package); a `[patch]` or `package =` alias in the facade (a second name for one
crate); `#[deprecated]` on the shim (see above).
## 2026-11-20: CI moves from velnor-actions to velnor-gen; Rust moves to 1.100.0

Context: Rust 1.100.0 (Renovate PR #484) was held because velnor-actions
generated the CI and qualified Rust only up to 1.99.1. velnor-actions is
end-of-life (0.1.81 is its last release; tailrocks/velnor-new#1625 was closed
as obsolete). velnor-gen (tag `tools-v0.1.106`, helper `velnor-ci` 0.1.106)
supports 1.100.0. The owner decided that this repository moves to it.

Decision: `.velnor/config.toml`, `.velnor/release-manifest.json`, the four
velnor-actions workflows (`ci.yml`, `state-seal.yml`, `suites.yml`,
`cache-prune.yml`), `.github/actions/`, `.github/scripts/`, `.github/velnor/`
and `.github/AGENTS.md` are removed. `.velnor/rust-ci.toml`,
`.velnor/rust-ci.lock`, `.velnor/rust-metadata.json` and
`.github/workflows/rust-ci.yml` replace them. The Renovate
`allowedVersions: "<1.100.0"` cap is gone and `rust-toolchain.toml` and
`mise.toml` pin 1.100.0. The crate `rust-version` stays `1.99`: it is the
MSRV, and raising it was not asked for. Rust 1.100.0 raised two new
findings, both fixed rather than allowed: clippy `needless_bool` in
`s03_columns.rs` (the guard now returns its condition), and cargo's new
`cargo::unused_dependencies` lint, which flagged a `sha2` dev-dependency in
`apps/holla`, `apps/showcase` and `apps/tablepro` that no code uses (removed,
`Cargo.lock` follows).

### What changed in the shape of CI

The job-by-job mapping is in [CI](ci.md). Points that change behaviour:

- One job set per Cargo root (five roots), not one per crate, and no `product`
  configuration. velnor-gen has no `--no-default-features`. The apps declare
  `default = []`, so their default build is the product shape; the library
  shapes without default features are built by the four consumer roots. The
  junie-tui shape `terminal` without `cli` has no direct job any more except
  through `termrock-consumer-terminal` and `termrock-consumer-jackin-host`.
- `--all-features` is a hand-listed `[[test_lane]]` (`cli`, `demo`, `product`,
  `terminal`, `test-support`, `work-metrics`). A crate that adds a feature must
  add it to the list.
- The functional suite is four `[[test_groups]]` (`functional-cases-a`, `-b`,
  `-c`, `-apps`) of 317 to 384 tests each, disjoint by construction (checked
  here against `cargo nextest list`: union 1,371 = every `binary(functional)`
  test, no overlap, and `velnor-ci test-groups check` repeats the overlap check
  in CI). They replace four hash partitions with the same eight test threads;
  the limit is 30 minutes per group instead of 15. There is no `functional`
  nextest profile, no failure-evidence artifact upload (`target/functional`,
  `target/nextest/functional`, 14 days), and the group step passes
  `--no-tests=pass` instead of `--no-tests fail`, so a vanished `functional`
  binary no longer fails by itself. `scripts/check-refactor-coverage.py` and
  the coverage inventory still pin the cases. All reported upstream.
- The performance suite is the all-features lane (it runs every test except
  the `functional` binaries, which have their groups). The old `suite-performance`
  built default features, so the counter-gated contracts compiled out of it and
  ran in the per-crate all-features jobs (decision of 2026-11-18, "Decision 1").
  They still run in an all-features job, now the lane. The inventory proof (the
  nested `cargo nextest list --profile performance --all-features`) used to run
  only under `--profile performance`; generated CI always runs `--profile ci`,
  so it now also runs under `ci` when `work-metrics` is compiled in, which is
  the lane. Verified locally with the lane's feature list: 24 inventory tests
  pass, the nested list included (8 s). The lane's `not binary(functional)`
  filter satisfies `velnor-ci lane-guard`: of the 82 tests that exist only with
  the lane's features, none is in a `functional` binary.
- `suites.yml` (`workflow_dispatch` with `suite` and `tier` inputs) is gone. The
  generated workflow has a plain `workflow_dispatch` that runs everything.
- `state-seal.yml` (the compiled-state producer and seal) has no equivalent
  workflow. The default-branch runs of `rust-ci.yml` save the Tools, Sources and
  Target caches instead. That save is refused while `Cargo.lock` holds a git
  source (`termpane`, `tuiscotti`, both public), so until velnor-new makes
  public git sources cacheable (announced for `tools-v0.1.108`) every run is
  cold. `timeout_minutes` is 45 (default 30) for that reason; lower it after
  `0.1.108`. `cache-prune.yml` is
  replaced by the `rust-trim` job.
  Dropping `state-seal.yml` is accepted: it produced and sealed compiled state
  for cache integrity only and shipped no artifact, so no release trust rested
  on it. Re-check that conclusion if a second workflow is ever added, and when
  `tools-v0.1.108` re-enables the cache saves.
- The `Required` check keeps its name, so a ruleset that requires it keeps
  working. No such ruleset exists today: server-side enforcement is a ruleset
  on `main` that requires `Required`, which the coordinator creates through
  github-terraform at rename time.

### Contracts of the removed tests and scripts

| Removed or changed | Contract | Disposition |
| --- | --- | --- |
| `tests/ci_default_branch.rs` (`[workflow].default_branch` of `config.toml`) | The configured default branch is `main` and has a sane TOML shape | Kept, rewritten for the top-level `default_branch` of `rust-ci.toml` (absent means `main`). New: the generated workflow pushes only that branch. Dropped: the `[workflow]` table-shape cases, which no longer exist |
| `performance_inventory.rs`: `performance_suite_is_a_required_generated_ci_job` | The suite is declared, required, four partitions, three triggers, job generated and awaited by `Required` | Replaced by `performance_contracts_run_in_a_required_all_features_ci_lane`: the lane exists in `rust-ci.toml`, is unfiltered, and `Required` waits for the job that runs it. Dropped: partitions (not expressible) and the triggers (the workflow always runs on pull request, push and dispatch) |
| `performance_inventory.rs`: `performance_contracts_run_in_required_all_features_ci_jobs` | Each package that owns a required test has a generated `--all-features` task and a `rust-<package>` job that `Required` waits for | Replaced by the same lane test. Dropped: the per-package job, which velnor-gen does not generate |
| `scripts/check-generated-workflows.sh` and the installer, config, seed and mise/Python inventory | See the guard decision below | Replaced |
| `scripts/policy/check-generated-workflows.sh` regression matrix, `check-installed-generated-workflow-guard.sh`, install-recovery, mutants, auth checker, Python inventory and seed-fixture tests | The guard's own behaviour | Dropped with the guard; they tested code that no longer exists |
| `scripts/validate-snapshot-blob-sizes.py`, `snapshot-large-blob-allowlist.tsv`, `test_snapshot_blob_sizes.py` | Bounded blob size in the guard's snapshot, with the reference binaries allowlisted | Dropped: a resource bound for the snapshot reader, not a repository policy. Nothing else reads the allowlist |
| `scripts/materialize-git-tree.py`, `test_materialize_git_tree.py` | Hardened snapshot materialization | Dropped: no snapshot is materialized |
| Decision 2026-11-19 (#505) snapshot cap 16384 and the unfixed jactionlint tuple mismatch | Local `pre-commit` failed on main | Obsolete: the failure source is removed |
| `scripts/generate-execution-ledger.py` | Names `.velnor/config.toml` and `velnor_actions` in recorded task data | Unchanged: it regenerates a historical ledger, and those are facts about the run it records |
| `docs/testing/cases/*.json`, `docs/testing/known-gaps.md`, acceptance receipts, `docs/P9.3-red-green.md` | Mention velnor-actions | Unchanged: dated evidence, not live instructions |

### The generated-workflow guard

The old guard (issue #509 and its trusted seed) was a 20,000-line subsystem
whose only job was to prove that the committed workflow files equal what
velnor-actions generates from the staged tree, using a checker, a generator
pin, Mise and Python copied out of the checkout into Git metadata so that the
branch under test could not replace them. It is bound to velnor-actions end to
end (the seed is a velnor-actions tree, the support-policy table lists
velnor-actions tuples, the comparison runs `velnor-actions generate`), and it
cannot run on velnor-gen. It is replaced, not ported, by three smaller checks:

1. `velnor-gen check` in the hk `pre-commit` and `pre-push` hooks (`hk.pkl`).
2. The same command in CI as the `lint-generated-workflows` job, a `[[lint]]`
   `mise-task` that `Required` waits for. Before, the equivalent was the Plan
   job's "files differ" failure; the old guard docs already said the CI check
   establishes reproducibility and does not authorize a change.
3. `velnor-ci check-metadata` in every Rust job, which fails when
   `.velnor/rust-metadata.json` differs from `cargo metadata`.

Why this is enough:

- The guard's value was local feedback plus a reproducibility proof. Both
  survive: the hook gives the feedback, the CI job gives the proof server-side
  where a bypassed hook cannot hide it. Hook-only enforcement would be a
  regression, so CI is the part that matters.
- The trust-anchor design (a checker outside the checkout) defended against a
  branch that changes its own generator. In CI that defence never held, since a
  pull request edits its own workflow file. Locally the hook is a convenience
  and was documented as bypassable. The remaining anchor is smaller and
  reviewable: `.velnor/rust-ci.lock` holds the digest of every helper asset,
  and the generator and helper versions are visible lines in `mise.toml` and
  the lock. Server-side authorization is still the open owner action: a ruleset
  that requires a trusted workflow, unchanged by this migration.
- The hermetic snapshot machinery (no network, scrubbed environment, path and
  symlink validation, blob bounds, GitHub blob API hydration) existed because
  velnor-actions was run on an untrusted materialized snapshot. velnor-gen reads
  the working tree and runs `cargo metadata --no-deps --offline`; there is no
  snapshot to harden.

Weakened, accepted, and recorded here so a reviewer can object:

- The hook checks the working tree, not the staged index, and the pre-push hook
  checks the tree at `HEAD` instead of every pushed commit. A partial stage or
  an intermediate commit can pass locally. The CI job checks each pull request
  head, and the merge queue checks the merge result.
- The checker and generator come from the checkout's `mise.toml`, so a branch
  can change what the local hook runs.
- The mise binary is no longer sha256-pinned. The old `jdx/mise-action` step
  carried `sha256:`; the generated step (`Install tools` in `rust-ci.yml`, the
  `version: '2026.11.10'` and `cache: false` lines) pins the action by commit
  and the mise version by number only.
- Restored mise and rustup tool caches are not re-verified. The generated
  `Restore the tools cache` step (`~/.local/share/mise`, `~/.rustup`, keyed by
  `mise.lock`) is followed by `mise-action` and nothing checks what was
  restored. The old workflow ran verifier actions after the restore (restored
  Rust, the Rust channel manifest, jactionlint).
- Cold setup is `mise install` (a source build of `velnor-gen` on macOS, pinned
  to the release commit `1f9762b239202d138ceec484da17fa5b27f2c3a3` rather than
  the movable tag). The
  old explicit trust action (`install-generated-workflow-guard.sh`) is gone;
  `mise exec -- hk install --mise` registers the hooks.

### Gaps reported to velnor-new (velnor-new-12)

- Public git sources disable every base cache; `tools-v0.1.108` is announced to
  fix it.
- The two provenance regressions above (mise binary digest, restored tool-cache
  verification) are sent upstream as gaps: velnor-gen should pin the mise
  digest and verify restored tool caches the way velnor-actions did.
- velnor-gen does not compare `.velnor/rust-ci.lock` with `mise.toml` or
  `mise.lock`, so `[velnor-gen]` in the lock was unenforced. `tests/ci_default_branch.rs`
  now pins its Linux digest, `version` and `commit` to the mise pins.
- No macOS asset for `velnor-gen` and `velnor-ci`; announced for `0.1.108`.
  Until then, `mise.toml` has two pins, `cargo:` for macOS and `github:` for
  Linux. velnor-gen refuses any tool that applies on the runner OS without a
  prebuilt binary, so `tuiscotti` is also `os = ["macos"]`.
- `tools.postinstall` is refused, so `mbx setup --yes` is no longer run by
  mise. velnor-new says the wrapper and `mr_boxington = true` already route
  cargo through mbx; run `mise exec -- mbx setup --yes` by hand if needed.
- Requests sent: `no_default_features` and `all_features` on `[[test_lane]]`;
  `partitions`, failure artifacts and a no-tests policy on `[[test_groups]]`;
  a per-root job timeout; cache restore for a group job; a key for
  `velnor-ci check-configurations`.

Revisit when `tools-v0.1.108` lands: use one `github:` pin for velnor-gen, check
that the base caches save, and then lower `timeout_minutes`.
