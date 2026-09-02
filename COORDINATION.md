# Parallel migration coordination

Two agents. One tree. Claim before write. Never revert the other agent's files.

## Owners

| Agent | Owns | Does not own |
|---|---|---|
| **Core** (this agent) | `crates/termrock/src/**` tokens, widgets, layout, interaction, public APIs, `crates/termrock/tests/**` | lookbook stories, showcase, catalog, verify/junie, PNG/SVG goldens |
| **Presentation** | `crates/termrock-lookbook/**`, `crates/termrock-showcase/**`, `verify/junie/**`, catalog/docs posters | widget paint, token resolvers, Table/DataTable/VirtualGrid layout |

## Core claimed (this commit)

- `crates/termrock/src/widgets/toggle.rs` — drop `[inner]` / `[B]` / `[~x]` wells; ToggleGroup pressed face is reverse+label (junie glyph catalog), not brackets; standalone Toggle stays `▎──●`/`○──`
- `crates/termrock/src/widgets/badge.rs` — drop default `[{inner}]` wells; status uses Glyph catalog + label; Count stays `(n)`
- `crates/termrock/src/style/tokens.rs` + `widgets/text_input.rs` + `tests/design_gate.rs` — underline only while editing (accent); idle invalid is bold `!` + message, not error underline (junie `input.rs`)
- `crates/termrock/src/widgets/markdown.rs` — task items paint `Glyph::CheckOn`/`CheckOff` (`[✓]`/`[ ]`)
- `crates/termrock/src/widgets/{combobox,token_field,search_input,path_input,password_input,date_time_picker,input_group,number_input}.rs` — `new()` idle (`editing: false`); Enter/`begin_edit`/`with_editing()` start the insert session; underline is `draft.is_editing()`, not focus; NumberInput no longer `begin_edit` on focus
- `crates/termrock/src/widgets/token_field.rs` — `input_recipe` third arg is `draft.is_editing()` in Draft zone
- `crates/termrock/src/widgets/surface.rs` — monochrome Inset/Sunken fill `Role::Canvas`, never `Color::Reset`
- `crates/termrock-lookbook/baselines/png/text-input-invalid.png` — idle invalid is `!` not error underline (intended junie `input.rs`)
- `crates/termrock-lookbook/src/interactors.rs` + `interactors/{extended,workflows}.rs` — live field interactors call `with_editing()`
- `crates/termrock/src/widgets/progress_steps.rs` — status marks from Glyph catalog (`✓ › ! − …`), not `[›]` wells


- `crates/termrock/src/widgets/surface.rs` — `SurfaceRecipe::Canvas` / `SurfaceFill::TerminalDefault` fill `Role::Canvas` `#000000`, never `Color::Reset`
- `crates/termrock/src/widgets/{tree,text_area,select}.rs` — overflow gutter uses `paint_overflow_scrollbar` / `overflow_thumb`
- `crates/termrock/src/widgets/{tabs,controls,text_input,data_table,picker}.rs` — junie keymap/state defaults


Landed reusable APIs (catalog/preview must use these; no page-local forks):

- `crates/termrock/src/widgets/table.rs` — `TableState::cell_nav`; `ColumnWidth::Min` grows leftover (junie `Constraint::Min`)
- `crates/termrock/src/widgets/data_table.rs` / `data_view.rs` / `tree_table.rs` — column gap = `spacing.column_gap` (2)
- `crates/termrock/src/widgets/virtual_grid.rs` — same gap 2 (was 1)
- `crates/termrock/src/widgets/panel.rs` — border title/footer ellipsis (was Clip); `Panel::vertical_scroll` + title-track reserve + overflow gutter
- `crates/termrock/src/scroll/mod.rs` — `overflow_thumb` (junie `ScrollState::thumb`); `paint_overflow_scrollbar`
- `crates/termrock/src/widgets/picker.rs` — query underline is editing D5; search footer spells junie `Alt+Enter`; searchable `j`/`k`/Space edit the query; Tab `NextScope`; Alt+Enter `ActivatedAlt`; overflow gutter uses `paint_overflow_scrollbar`
- `crates/termrock/src/widgets/table.rs` — reverse cell requires `cell_nav`; `focused_column` alone is not a cursor; `cell_nav` with no column seeds the first visible column on paint
- `crates/termrock/src/widgets/list.rs` — overflow gutter uses `paint_overflow_scrollbar` / `overflow_thumb` (was `render_scrollbar` / `full_cell_thumb`)
- `crates/termrock/src/style/junie.rs` — `gutter()` colour-only; BOLD from row fill merge
- `crates/termrock/tests/text_area_hot_path.rs` — insert test enters editing first
- `crates/termrock/tests/design_gate.rs` — picker underline class

## Public API for presentation

- **Cell cursor:** `TableState::set_cell_nav(true)` + `set_focused_column(Some(id))` + `Table::focused(true)`. Default `cell_nav=false` keeps row-select `›` + tint. Left/Right do not auto-enter cell-nav. If `cell_nav` is on and `focused_column` is `None`, first paint seeds the first visible column (junie `cursor_col = 0`).
- **Picker keys:** searchable (default) types into the query (`j`/`k`/Space included). `PickerOutcome::NextScope` (Tab) and `PickerOutcome::ActivatedAlt` (Alt+Enter) are host-owned; lookbook may ignore until scope/new-tab wiring exists. `PickerState::set_searchable(false)` restores `j`/`k` list motion. `Picker::searchable` copies onto state at render.
- **Reverse cell:** `DesignSystem::reversed()` = canvas on `text_primary` + BOLD. Never `Modifier::REVERSED`.
- **Table columns:** `ColumnWidth::Min(n)` now absorbs leftover after Fixed (junie Min). Drop Status via `.priority(20)` and render the **same** story at 52×6: Task grows 24→33. Do not mint a second Fixed-33 story.
- **DataTable gap:** `resolve_paint_widths_with_gap(budget, system.spacing.column_gap, out)`. Id cells fill the column with `TextSecondary` (padding included).
- **Grid header `⚷`:** `DataColumn::primary()`. Header origin is faint `⚷` (junie overdraw of `"▪ "`). Body Id cells stay `TextSecondary`. Catalog can crop the header.
- **Framed pane scroll:** `Panel::vertical_scroll(content_len)` + optional `.scroll_offset(n)`. Title row paints two faint blanks before `─╮` (junie empty `meta` `"  "`). Body gutter uses `scroll::paint_overflow_scrollbar` / `overflow_thumb` (`len = (viewport * track) / content`). Host wraps copy at `Panel::scrolled_content_area(body)` (`width - 2`). `full_cell_thumb` stays the subcell `tui-scrollbar` rounding for `ScrollArea`/dialog/`paint_scrolled_region`. Line widgets (panel, picker, list, tree, text area, select) use `overflow_thumb`.
- **Tabs keys:** `h`/`l` move like arrows. Space activates (same as Enter).
- **Radio Tab:** ignored; scene owns Tab. Arrows/`j`/`k` still move.
- **Closed Select:** Down/Right and Up/Left cycle the value without opening. Enter/Space open.
- **TextInput:** `new` starts `editing: false`. Enter begins edit. Picker query and open Select search call `set_editing(true)`.
- **DataTable:** default `DataTableNavMode::Row` (junie `cell_nav: false`). Reverse cell requires Cell/Range **and** `DataTable::focused(true)`.

## Handoff / do not duplicate

- Table/grid/editable lookbook stories + `verify/junie` crops in this commit are **API consumers** of `cell_nav` / Min leftover / DataTable gap-2. Presentation owns the unified catalog and may relocate those stories; do not reimplement the widgets.
- PNG bless (`dialog/confirm-run`, `panel/framed-pane` missing from Jackin subset) is presentation. List overflow now uses `overflow_thumb`; `list/scroll-rows` PNG rewrite is the intended junie thumb (verify 0/0).
- Workspace `png_baselines` red is presentation (subset stories without baselines / intended Junie paint drift). Core will not run `mise run bless-pngs`.
- `TextInputState::new` is idle (`editing: false`). Lookbook toast message knob and `text-input/basic` interactor use `with_editing()` for live typing. Paint-only `text-input/{focused,prefix,secret}` stay idle; PNG bless is that field-plane paint.
- Pre-existing panel ellipsis tests are core defects; this agent fixes the painter.

## Core claimed (lookbook consume, this commit)

- `crates/termrock-lookbook/src/interactors.rs` — toast message knob `with_editing()` (live typing)
- `crates/termrock-lookbook/src/app.rs` — toast control test Enter/`with_editing` live type
- `crates/termrock-lookbook/src/stories.rs` — text-input stories idle vs focused (no implicit edit)
- `crates/termrock-lookbook/baselines/png/` — bless idle-field text-input PNGs (intended junie `editing: false`)

## Presentation claimed (this slice)

- `crates/termrock-lookbook/src/stories.rs` + `junie_screens.rs` — Overview Tokens / Settings General / Task runner Targets stories from public APIs
- `verify/junie/scenarios.json5` — un-SKIP those six showcase scenes with equivalent crops

## Presentation claimed (spawned)

- `crates/termrock-lookbook/baselines/png/` — bless Jackin subset including `dialog-confirm-run.png` and `panel-framed-pane.png`
- Unified catalog: consume `TableState::cell_nav`, `ColumnWidth::Min` leftover, `DataColumn::primary`; do not fork table paint
- `verify/junie` remaining SKIP are product shells (overview/settings/taskrunner/tablepro) unless catalog adds equivalent public-API demos

## Protocol

1. Edit only claimed paths. Add a claim line before touching a new file.
2. API gap: write it under **Public API for presentation**; core implements in the library, never a showcase-local workaround.
3. Integrate `git pull --rebase` before push. Never `git reset --hard` the other's commits.

## Presentation status

PNG bless: PASS. `TERMROCK_BLESS_PNGS=1 cargo nextest run -p termrock-lookbook --all-features --test png_baselines --no-capture --locked` then confirm without bless: PASS. Render-twice determinism held (no raster bug).

- New baselines: `dialog-confirm-run.png`, `panel-framed-pane.png`.
- Rewrote 14 subset goldens to HEAD Junie paint (same geometry). Notable: `collection-state-headless` dropped a stray light footer band.

API consume (no Table/DataTable paint forks):
- `table/tasks` — `ColumnWidth::Min(24)` + Status `.priority(20)` (54 cols keeps Status; 52×6 drops Status, Task 24→33).
- `table/editable-cursor` — recast off Fixed(33) onto the same columns + `set_cell_nav(true)` at 52×2; junie crop `[11, 2, 33, 1]`.
- `data-table/grid-ids` — `DataColumn::primary()`.
- `table/editable-80` — still ID Fixed(5)+Task Fixed(32) for the 44-col 80×24 crop (leftover-growth lives on `table/tasks` @ 52×6).

Remaining SKIP (`verify/junie`, 5 TablePro product shells):
- tablepro default 120×40 and 80×24, local 120×40, production 120×40, help 120×40

Panel scrollbar API landed (`vertical_scroll` / `scrolled_content_area` / `overflow_thumb`). `panel/framed-pane` consumes it; no story-local thumb/title overdraw.

Junie screen stories (public APIs, equivalent crops, 0/0):
- `overview/tokens` — Tokens card (swatch + name + note)
- `settings/general` — Tabs + TextInput + RadioGroup + Toggle + Button
- `taskrunner/targets` — Tree of payments-gateway / shared-libs

SKIP remaining: 5 tablepro product shells (default/local/production/help). Do not map `database-workbench/*` pattern demos onto those.
