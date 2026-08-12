# Plan 009: Data + overlay wave — tables, trees, menus, palette, pickers, diff, forms

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/widgets/data_table.rs crates/termrock/src/widgets/command_palette.rs crates/termrock/src/widgets/diff.rs crates/termrock/src/widgets/form.rs crates/termrock/src/widgets/menu_bar.rs`
> Churn from 001–008 expected in shared modules; the widgets above must
> exist under these names. On rename, STOP.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/001, 003, 004, 005, 006, 007
- **Category**: tech-debt (visual quality wave)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

After the foundation (001–005) and primitives (006–007), the data-heavy and
overlay-heavy widgets are the next-most-visible surfaces: tables and trees
carry most real app content; command palette and pickers are the flagship
interaction moments; diff is the flagship agent-adjacent read surface. They
must express the same layered language (surface fills, tinted selection,
padded cells, muted chrome) instead of their current flat, flush,
hand-drawn styling.

## Current state

Widgets in this wave (`crates/termrock/src/widgets/`): `data_table.rs`,
`table.rs`, `tree.rs`/`tree_table.rs` (tiers landed in 007 — this wave
applies them to tree_table), `menu_bar.rs`, `dropdown_menu.rs` (shell done
in 005 — rows here), `command_palette.rs`, `quick_open.rs`, `picker.rs`,
`history_picker.rs`, `file_picker.rs`, `diff.rs`, `form.rs`,
`form_wizard.rs`, `multi_select.rs`, `select.rs`, `combobox.rs`.

Verified facts still binding:

- `menu_bar.rs` and `command_palette.rs` carry hand-drawn `"┌"` literals at
  `539e7d03` (grep-verified) — their shells move to `Surface`.
- Diff roles exist: `DiffAdded` = `fg PHOSPHOR_GREEN bg (20,50,20)`,
  `DiffRemoved` = `fg DANGER_RED bg (60,20,20)` (`style/mod.rs:83-86,358-359`,
  verified) — diff line backgrounds already exist as roles; this wave makes
  `diff.rs` paint **full-row** tinted backgrounds (not per-glyph) and adds
  the hunk-separator + collapsed `+N/-M` summary row styling.
- Selection/hover washes: `ListRowRecipe.tint`/`hover_wash` (001);
  `resolve_list_row` is already consumed by `list.rs`/`tree.rs` only —
  table/menu/palette rows currently style ad hoc.
- `FieldRow` (007) is the canonical labeled-row primitive for forms.

Read each widget's paint path before editing; line numbers from the audit
era are stale by now — the constraints below are binding, not excerpts.

Design constraints (design SoT §7, §8 wave 3):

- Row selection everywhere = gutter + `SelectionTint` wash + `TextStrong`
  label (never neon slab, never fg-only).
- Header rows (tables): `TextMuted` labels on `Role::Raised` band;
  separators from glyph catalog rules, not ad-hoc `─` strings.
- Command palette: input on `Sunken`, results as recipe rows, matched-substring
  highlight in `Accent`, shell = `Overlay` surface (005 conventions).
- Forms: label/value rows through `FieldRow`; validation messages `Danger`
  fg + field border `InputInvalid` (existing roles).
- Diff: full-row tints tuned to quantize to 256-color red/green (existing
  role bgs qualify — verify via the quantize tests), hunk separator glyph
  row, collapsed summary `+N/-M` with `DiffAdded`/`DiffRemoved` fgs.
- Zebra striping option for wide tables: alternate `Canvas`/`Surface` row
  bgs behind a builder flag (off by default).

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |

## Scope

**In scope**: the widget files listed above + the shared
`crates/termrock/src/style/tokens.rs` list-row recipe + their lookbook stories +
`crates/termrock/src/widgets/field_row.rs` canonical value/validation recipe +
`crates/termrock/src/patterns/settings_screen.rs` same-commit Form consumer migration +
sequential Plan 009 migration files beginning at `migrations/0274-*.md` +
`MIGRATING.md` + generated component/API/frame outputs + responsive
`agent-browser` evidence under `artifacts/visual-qa/plan-009/` +
`plans/README.md`.

**Out of scope**: charts (already the richest module), remaining sweep
widgets (010), kernel, patterns other than story updates.

## Git workflow

`main`, `git commit -s`, commit per widget cluster (tables / trees / menus+palette+pickers / diff / forms).

## Steps

### Step 1: Tables (`data_table.rs`, `table.rs`)

Rows through `resolve_list_row` (selection wash + hover + gutter); header
band on `Raised` with `TextMuted` labels; cell padding from
`DesignSystem.spacing.pad_x` (columns currently flush); optional
`.zebra(true)` builder. Sort indicators from glyph catalog (`▾`/`▴`
fallbacks `v`/`^`) if the widgets have sorting — check their APIs first.

**Verify**: `cargo nextest run -p termrock data_table table` → pass; new
tests `table_selection_is_tinted`, `header_band_is_raised`.

### Step 2: Tree table

Apply 007's tone tiers + hover wash to `tree_table.rs` rows (shares row
paint with `tree.rs` where possible — do not fork; extract a shared row
painter if the two diverge).

**Verify**: `cargo nextest run -p termrock tree_table` → pass.

### Step 3: Menus, palette, pickers

- `menu_bar.rs`: shell → `Surface` Overlay (kill its `"┌"` literals);
  menu rows through `resolve_list_row`; shortcut column `TextMuted`,
  separators = catalog rule glyph spanning border-to-border.
- `command_palette.rs`: shell → `Surface`; input region `Sunken`; result
  rows recipe-styled with `Accent` match highlight; footer hint row via
  HintBar conventions.
- `picker.rs`, `quick_open.rs`, `history_picker.rs`, `file_picker.rs`,
  `multi_select.rs`, `select.rs`, `combobox.rs`: same row + shell
  conventions (most already delegate rows to `List` — verify per widget and
  only touch the ones styling ad hoc).

**Verify**: `grep -c '"┌"' crates/termrock/src/widgets/{menu_bar,command_palette}.rs`
→ 0 each; targeted nextest per widget → pass.

### Step 4: Diff

Full-row tinted backgrounds via `DiffAdded`/`DiffRemoved` role bgs (row
width, not glyph width); gutter line numbers `TextDisabled`; hunk separator
row (catalog ellipsis/rule glyph, `TextMuted`); collapsed hunk summary row
`+N` in `DiffAdded` fg / `−M` in `DiffRemoved` fg. Confirm quantization:
`cargo nextest run -p termrock quantize` (role bgs must map to 256-color
red/green — a test should assert the quantized bg is a red/green cube entry,
not gray).

**Verify**: `cargo nextest run -p termrock diff` → pass incl.
`diff_rows_are_row-tinted` + quantize assertion.

### Step 5: Forms

`form.rs`/`form_wizard.rs` label/value rows via `FieldRow` (Plain/Masked/
Unset value kinds); validation row styling per constraints; wizard step
header via `progress_steps` conventions (no new chrome inventions).

**Verify**: `cargo nextest run -p termrock form form_wizard` → pass.

### Step 6: Stories, migration, gate

Stories updated per widget (states: selected/hovered/zebra/narrow/mono).
`migrations/0269-v0.13.0-data-overlay-wave.md` with per-widget before/after.
Link from `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0.

## Test plan

New tests named in steps (5+), plus expectation updates. Mono/ASCII
capability stories re-verified per widget.

## Done criteria

- [x] `mise run check` + `mise run gate` exit 0
- [x] `grep -c '"┌"' crates/termrock/src/widgets/{menu_bar,command_palette}.rs` → 0 each
- [x] Row owners route through `resolve_list_row`; delegating wrappers (`Picker`
  → `List`, `Combobox` → `CompletionMenu`) retain one canonical paint owner
- [x] Diff full-row tint + quantize tests pass
- [x] Sequential migrations `0274`–`0278` exist and are linked
- [x] `plans/README.md` updated

## STOP conditions

- A widget's row model can't express recipe rows without behavior changes
  (e.g. its own selection state machine conflicts) → report; don't fork the
  recipe.
- Shared row painter extraction (Step 2) grows beyond tree/tree_table →
  scope question, report.

## Maintenance notes

- After this wave, `resolve_list_row` is the single row-styling authority
  for every rowed widget — reviewers reject new ad-hoc row styling.
- Zebra flag interacts with future virtualization work — note in review if
  `virtual_grid`/`virtual_list` need the same flag later (deferred to 010
  sweep or beyond).

## Completion summary

Completed all six steps in independently green commits. Table, DataTable,
TreeTable, menu and picker row owners now share `resolve_list_row`; Picker and
Combobox retain canonical delegation to List and CompletionMenu. Diff paints
full-row semantic added/removed tints. Form projects typed Plain, Masked, and
Unset values through FieldRow, including SettingsScreen's non-leaking search
migration. `mise run check`, `mise run gate`, targeted diff/quantize/form tests,
literal-shell greps, generated docs, and browser validation all passed.

Amendment rationale: live migration history required allocations 0274–0278;
cross-surface law brought generated outputs and `agent-browser` evidence into
scope; TreeTable needed pointer-hover identity; wrapper ownership required
delegation-aware verification; browser proof moved selected-label tint into
the shared row recipe; and Form's string-only model required canonical
FieldRowValue plus its SettingsScreen migration. Each amendment removed a plan
defect against higher-authority repo/design contracts while keeping every
commit independently green.

Designer verdicts: Table/DataTable **pass**; TreeTable **iterated 1, pass**;
overlay/picker set **iterated 2, pass**; shared SelectionTint cascade
**iterated 2, pass**; DiffView/DiffReview **pass**; Form **iterated 1, pass**;
FormWizard **pass**; FieldRow **iterated 1, pass**. Responsive dark, paper,
reduced-motion, keyboard, console, network, and screenshot evidence is indexed
in `artifacts/visual-qa/plan-009/README.md`.

## Amendments

- 2026-08-12: Replaced the stale 0269 migration allocation with sequential
  Plan 009 migrations beginning at 0274. Plans 008 independently and
  truthfully consumed 0269–0273; rewriting them violates immutable migration
  history. Each visible cluster receives its same-commit boundary.
- 2026-08-12: Added generated catalog/API/frame outputs and mandatory
  responsive `agent-browser`/designer evidence to Scope. Cross-surface repo law
  and standing browser gates make these outputs part of every changed public
  surface; omitting them was a plan defect.
- 2026-08-12: Drift audit found all named widgets present. Only DataTable had
  direct churn since 539e7d03; CommandPalette's historic `"┌"` literal is
  already gone, while MenuBar still owns one literal tuple. Steps therefore
  test live paint paths instead of assuming both stale excerpts remain.
- 2026-08-12: Allocated migration 0274 to the table cluster. Table and
  DataTable share one recipe/header default and ship as one independently green
  boundary; later visible clusters advance sequentially.
- 2026-08-12: Allocated migration 0275 to TreeTable and added
  `TreeTableState::hovered`. Live code had no pointer-hover identity, so merely
  swapping styles could not satisfy the shared hover-wash contract. The
  additive state follows Tree's precedent and keeps paint in
  `resolve_list_row` rather than introducing a second row recipe.
- 2026-08-12: Replaced the Step 3/done-criteria demand for a literal
  `resolve_list_row` call in every wrapper with ownership-aware verification.
  Live `Picker` delegates rows to `List`, and `Combobox` delegates its popup to
  `CompletionMenu`; duplicating recipe calls would fork canonical paint. Direct
  row owners use the recipe, while wrappers are verified through delegation.
  Migration 0276 records the visible overlay/picker default change.
- 2026-08-12: Added the shared list-row recipe to Scope after browser/designer
  proof showed row text repainting erased `SelectionTint` backgrounds. Options
  were per-widget style patching, buffer-order tricks, or making the recipe's
  selected label carry its tint. The shared recipe fix best matches repo law,
  design intent, and smallest long-term blast radius; it removes the enabling
  condition for the defect across every consumer.
- 2026-08-12: Allocated migration 0277 to Diff's full-row semantic tint
  boundary. Live hunk headers and folded summaries already use the documented
  muted/added/removed roles, so the smallest correct change fills the complete
  added/removed row before existing span paint.
- 2026-08-12: Added `field_row.rs` to Scope for Step 5. Live `Field` stores
  only `&str`, so it cannot satisfy the plan's explicit Plain/Masked/Unset
  projection, while the canonical enum lives in FieldRow. Options considered:
  infer kinds from strings (ambiguous), add a duplicate Form-only enum
  (violates shared-abstraction law), or project Form through FieldRow's typed
  value. The canonical typed projection best matches repo law and design SoT.
- 2026-08-12: Added `patterns/settings_screen.rs` for the required same-commit
  consumer migration. Moving Form from a Copy string projection to canonical
  `FieldRowValue` necessarily changes filtering/cloning; repo migration law
  forbids leaving the in-repo composite on the removed shape.
- 2026-08-12: Allocated migration 0278 to the typed Form/FieldRow boundary.
  The migration records Copy→Clone projection changes and the non-leaking
  searchable-text contract for masked/composed values.
