# Decisions

## Shared authority

The design reference governs exact product behavior, color, layout, keyboard and pointer interaction, and visual acceptance. Main governs the intended modular architecture. These authorities answer different questions: main's crate boundaries do not override the design, and design's current implementation does not prove that main or a WIP has completed the architecture or product.

## Compatibility and migration

Preserve public APIs and documented runtime behavior unless a deliberate architecture decision authorizes a breaking change. Retire old paths through an intentional migration with consumer updates and verifiable acceptance. Do not use historical status reports, old staffing plans, or frozen baseline captures as current readiness evidence.

## Evidence limits

Build success demonstrates compilation for the selected workspace and configuration. Tests demonstrate only their exercised cases. Captures demonstrate only their named states and dimensions. Historical source comments and safety rationale are evidence of prior reasoning, not fresh safety certification. Documentation link and cross-branch content checks certify documentation integrity. Operational proof validators certify only their named artifacts and source snapshots. A versioned packaging migration must identify retired prose explicitly without rewriting measured evidence or weakening operational checks.

## Scope discipline

Future proposals remain deferred until a consumer and acceptance contract are identified. Upstream work remains an external dependency until released, pinned where appropriate, and verified in this repository. Do not silently broaden supported platforms, host operations, or interaction abstractions without isolated evidence.

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
