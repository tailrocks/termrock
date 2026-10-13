# Plan 006: One selection language for every collection — recipe adoption, shared gutter, table convergence

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 002/004/005 must be DONE in
> `plans/README.md`. For every site below, re-locate with the given search
> before editing; a site that moved is fine (edit it where it lives), a site
> that vanished gets recorded, not improvised.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/004 (SelectionChrome default + overrides deleted), plans/005 (underline gone)
- **Category**: tech-debt / design
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

After plans 004/005, `resolve_list_row` is the selection authority — but only
~14 widgets call it. Nine widgets hand-roll the same
`match system.selection {…}` and repaint whole selected rows in `Role::Focus`
(destroying severity roles); five container widgets hand-roll selection with
five different "current row" glyphs (`▌ › > • ▸`); the three table widgets
disagree on header paint, sort markers, and gutters; VirtualGrid's cursor is
invisible when unfocused and its ranges are solid accent; VirtualList sticky
rows silently drop selection chrome. One language, everywhere collections
paint — that is design-language principle 5 and cohesion rule §6.3.

## Current state (leads verified by audit at `605217aa`; anchors `tokens.rs:776-831` verified first-hand)

- Hand-rolled `match system.selection { Fill => style(Role::Selection), Tint|Gutter => style(Role::Focus) }`
  (whole-row Focus repaint): `diff.rs:1421-1424`, `hex_viewer.rs:1449-1452`,
  `log_stream.rs:1130-1136`, `terminal_output.rs:1436-1439`,
  `object_inspector.rs:1595-1600`, `event_stream.rs:1057-1062`,
  `diagnostic.rs:1550-1553`, `key_value_table.rs:1258-1261`,
  `timeline.rs:980-983`. Find: `rg -n "SelectionChrome::Fill" crates/termrock/src/widgets/`.
- Hand-rolled container selection (no recipe at all): `sidebar.rs:1218-1246`
  (route = full-row `Role::Selection`+BOLD; focus = `Role::Focus`+REVERSED;
  `›`/`•` markers), `detail_table.rs:17,585-617` (`▸ ` marker const),
  `virtual_grid.rs:961-1052` (cursor==range==`Role::Accent`; unfocused cursor
  == plain text), `tree_navigation.rs:955-1030` (two full-row slabs; single
  style for the whole composed string), `theme_picker.rs:266-284`
  (`›` + `Role::Selection` fill + unconditional `PanelChrome::Focused` at
  `:251-253`).
- Gutter glyph divergence: canonical `glyphs.selection_gutter()` = `▌` used by
  `table.rs:1182`, `panel.rs:1000-1008`, recipe consumers. Literals to
  replace: `tree_table.rs:1297-1301` (`▌` = *checked*, `›` = selected —
  inverted), `key_value_table.rs:1248-1254`, `timeline.rs:998-1002`,
  `sidebar.rs:1240-1246`, `detail_table.rs:17`, `review.rs:1698-1702`,
  `command_palette.rs:1655-1659` (resolves recipe then ignores
  `recipe.gutter`), `object_inspector.rs:1543`, `event_stream.rs:1067`,
  `diagnostic.rs:1527`, `diff.rs:1451,1627`, `log_stream.rs:1214`,
  `history_picker.rs:1256`, `transcript.rs:796`, `search_results.rs:1261`,
  `checkpoint_timeline.rs:1221`, `menu_nav.rs`, `mention.rs:1505`,
  `model_mode_selectors.rs:1157,1640`. Find: `rg -n '"[›>] ?"' crates/termrock/src/widgets/`.
- Raw `Role::Selection` row/label paint still live after plan 004:
  `completion_menu.rs:1491`, `menu_nav.rs:335`, `progress_steps.rs:769`,
  `notification_center.rs:1288`, `command_palette.rs:1688`,
  `keyboard_help.rs:1208`, `tag_chip.rs:532`, `menu_bar.rs:1406-1409`,
  `quick_open.rs:1558-1561`, `sidebar.rs:1223-1226`, `theme_picker.rs:273-284`,
  `data_table.rs:1529-1532` (cell), `fullscreen_viewer.rs:1247`,
  `prompt_composer.rs:2006,2009`, `tree_navigation.rs:959`.
  Find: `rg -n "Role::Selection" crates/termrock/src/widgets/`.
- Table divergence: headers — `table.rs:1217-1221` + `data_table.rs:1337-1341`
  (`TextMuted` on `Raised` band) vs `tree_table.rs:1162-1166` (`TextStrong`
  when focused, no band); sort markers — `table.rs:1683-1688` (`▲▼`, no ASCII)
  vs appended `^ v` in the other two.
- VirtualList sticky rows: `virtual_list.rs:730-787,864-887` —
  `paint_simple_row` has no gutter slot/selection states while the body
  delegates to `List`.
- Empty states: only `virtual_list.rs:697-707` has `empty_message`;
  `virtual_grid`, `tree_table`, `detail_table`, `key_value_table`, `timeline`,
  filtered `sidebar.rs:1176-1188` paint nothing when empty.
- Split dividers: `split_pane.rs:387-392` and
  `resizable_panel_group.rs:890-908` use heavy glyphs (`┃━┋┅`) for
  focus/drag — violates the border law (`AGENTS.md` focus-visible section).
- Per-row `DesignSystem` clones removed by plan 004 Step 6 — verify.

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Widget tests | `cargo nextest run -p termrock <widget>` | pass |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: files listed above; `crates/termrock/src/style/tokens.rs`
ONLY if a shared row-paint helper needs adding (preferred home:
`widgets/composed_row.rs` or a new `widgets/row_chrome.rs`);
`crates/termrock/tests/design_gate.rs` (extend); `migrations/0286-*.md` +
`MIGRATING.md`.

**Out of scope**: status/severity color discipline (plan 007 — here you only
stop *destroying* severity by whole-row repaints; you don't redesign status
paint), overlay chrome (plan 009), patterns (plan 010), input widgets
(plan 008).

## Git workflow

`main`; commit per step-cluster; `git commit -s`;
`refactor(widgets)!: unify collection selection chrome` style subjects.

## Steps

### Step 1: Shared row-chrome helper

Add `pub(crate) fn paint_row_chrome(recipe: &ListRowRecipe, buffer, row_rect, base: Style) -> Style`
(home: new `widgets/row_chrome.rs`): applies tint/fill wash to the row rect,
paints `recipe.gutter` in the reserved slot honoring `show_gutter_slot`,
returns the label style = `base.patch(recipe.label)` (base carries the
widget's semantic role so severity survives selection). Unit-test it directly.

### Step 2: Migrate the nine `match SelectionChrome` copies

Each of the nine (diff, hex_viewer, log_stream, terminal_output,
object_inspector, event_stream, diagnostic, key_value_table, timeline):
build `ListRowVisualState { selected, focused, .. }`, call
`resolve_list_row`, use Step 1's helper. The row's semantic style (e.g. log
level role) becomes `base` — selected rows keep severity, gain gutter+tint.
Delete the local `match`.

**Verify**: `rg -n "SelectionChrome::Fill" crates/termrock/src/widgets/` → 0 matches outside `row_chrome.rs`/recipe internals; per-widget tests green.

### Step 3: Migrate the container hand-rollers

`sidebar.rs` (route = gutter `▌` + `TextStrong`, optional tint; cursor =
recipe focus treatment; kill both full-row slabs), `detail_table.rs`,
`tree_navigation.rs` (port `tree.rs`'s recipe usage; split the single-string
row into parts so badge/status can take `secondary` tone),
`theme_picker.rs` (recipe rows + `focused(bool)` builder driving
`PanelChrome` — default false), `keyboard_help.rs:1208`,
`progress_steps.rs:769`, `notification_center.rs:1288`, `menu_nav.rs:335`,
`completion_menu.rs:1491`, `command_palette.rs:1688` badges,
`quick_open.rs:1558-1561`, `menu_bar.rs:1406-1409`, `tag_chip.rs:532`
(selected chip → `SelectionTint` + `TextStrong`), `fullscreen_viewer.rs:1247`,
`prompt_composer.rs:2006,2009`, `data_table.rs:1529-1532`
(cell-selected → `recipe.tint`).

**Verify**: `rg -n "Role::Selection" crates/termrock/src/widgets/` → 0 matches (widgets); `mise run check` green.

### Step 4: Gutter vocabulary

Replace every hand-rolled `›`/`>`/`•`/`▸`-as-selection literal with
`recipe.gutter` / `glyphs.selection_gutter()`; `▸/▾/›` remain ONLY for
disclosure; checks come from `recipe.check_on/check_off`. Fix
`tree_table.rs:1297-1301` inversion (▌=selected, check glyph=checked).
`streaming_markdown.rs:826` caret stops using `▌` (pick a distinct caret
glyph from the catalog, e.g. `Glyph::Prompt`).

**Verify**: `rg -n '"› "|">" ?[,)].*select' crates/termrock/src/widgets/` — inspect remaining hits are disclosure-only; visual spot-check via lookbook list/table/palette stories.

### Step 5: Tables converge

Shared header + sort treatment (helper beside Step 1's): header =
`TextMuted` on `Raised` band, never focus-brightened (`tree_table.rs:1162-1166`
fix); sort marker = `▲/▼` with ASCII `^/v` from the glyph catalog, painted in
the header style, consistent slot across `table.rs`, `data_table.rs`,
`tree_table.rs`.

**Verify**: three table test suites green; headers identical across the three in a side-by-side lookbook render.

### Step 6: VirtualGrid per spec

`virtual_grid.rs`: cursor cell = `REVERSED` + `TextStrong` (one cell) +
gutter marker in the row-number column; range = `SelectionTint` bg only;
unfocused cursor keeps the gutter marker (position never invisible); headers
stay `TextMuted`. Keep O(visible) paint (the clear-then-paint loop at
`:1046-1052` keeps per-cell set_style).

**Verify**: virtual_grid tests + a new test: unfocused grid renders cursor gutter marker; range cells carry SelectionTint bg, not Accent.

### Step 7: VirtualList sticky parity + shared empty states

`paint_simple_row` resolves the same recipe path as `List` (gutter slot
reserved; selection states honored). Add `empty_message` (mirroring
`virtual_list.rs:697-707`) to `virtual_grid`, `tree_table`, `detail_table`,
`key_value_table`, `timeline`; `sidebar` filtered-empty paints a muted
"no matches" line under the query.

**Verify**: new empty-state renders in each widget's tests.

### Step 8: Split dividers obey the border law

`split_pane.rs:387-392`, `resizable_panel_group.rs:890-908`: one glyph
weight (`│`/`─` via `glyphs.rule_v()/rule()`); focus/drag = role change only
(`Role::Border` → `Role::BorderFocused`); ASCII gets a midpoint handle glyph
(`▍`→`=`/`|#|` style from catalog) instead of weight change.

**Verify**: `rg -n '"┃"|"━"|"┋"|"┅"|"║"' crates/termrock/src/widgets/` → 0 matches.

### Step 9: Gate + migration

- Extend `design_gate.rs`: un-`#[ignore]` nothing yet, but add
  `fn collections_share_gutter()` — render List/Table/DataTable/TreeTable/
  Sidebar/Timeline/KeyValueTable with a selected row; assert the first
  gutter cell symbol equals `glyphs.selection_gutter()` for all.
- `migrations/0287-*.md` (next free) + `MIGRATING.md`: selection paint
  changes per widget, gutter glyph unification, table header/sort
  convergence, divider glyph change, empty-state additions.
- `mise run gate` → exit 0.

## Test plan

Per-step verifications above; new gate test; update all snapshot/buffer
tests these repaints churn (expect many — each should show gutter+tint
instead of slabs).

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "Role::Selection" crates/termrock/src/widgets/` → 0.
- [ ] `rg -n "SelectionChrome::" crates/termrock/src/widgets/` → only `row_chrome.rs` (if it needs the enum) — no per-widget matches.
- [ ] `collections_share_gutter` gate green.
- [ ] Migration + `MIGRATING.md`.
- [ ] `plans/README.md` updated.

## STOP conditions

- A widget's selection semantics genuinely aren't row-shaped (e.g.
  `virtual_grid` ranges) and the recipe can't express them — use the
  documented spec cue (Step 6) or report; don't force `resolve_list_row`
  where it doesn't fit.
- Gutter slot insertion shifts hit-test rects a widget exposes publicly and
  tests outside that widget fail — report the coupling.

## Maintenance notes

- Plan 007 relies on Step 2's `base`-style pass-through for severity;
  reviewers should check no widget re-introduces whole-row severity repaint.
- After plans 007+009, plan 009 flips the `no_widget_paints_selection_fill_by_default` gate to active.
