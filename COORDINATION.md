# Parallel migration coordination

Two agents. One tree. Claim before write. Never revert the other agent's files.

## Owners

| Agent | Owns | Does not own |
|---|---|---|
| **Core** (this agent) | `crates/termrock/src/**` tokens, widgets, layout, interaction, public APIs, `crates/termrock/tests/**` | lookbook stories, showcase, catalog, verify/junie, PNG/SVG goldens |
| **Presentation** | `crates/termrock-lookbook/**`, `crates/termrock-showcase/**`, `verify/junie/**`, catalog/docs posters | widget paint, token resolvers, Table/DataTable/VirtualGrid layout |

## Core claimed (this commit)

Landed reusable APIs (catalog/preview must use these; no page-local forks):

- `crates/termrock/src/widgets/table.rs` — `TableState::cell_nav`; `ColumnWidth::Min` grows leftover (junie `Constraint::Min`)
- `crates/termrock/src/widgets/data_table.rs` / `data_view.rs` / `tree_table.rs` — column gap = `spacing.column_gap` (2)
- `crates/termrock/src/widgets/virtual_grid.rs` — same gap 2 (was 1)
- `crates/termrock/src/widgets/panel.rs` — border title/footer ellipsis (was Clip); `Panel::vertical_scroll` + title-track reserve + overflow gutter
- `crates/termrock/src/scroll/mod.rs` — `overflow_thumb` (junie `ScrollState::thumb`); `paint_overflow_scrollbar`
- `crates/termrock/src/widgets/picker.rs` — query underline is editing D5; hint chord `A-↵` not `Alt+Enter`
- `crates/termrock/src/style/junie.rs` — `gutter()` colour-only; BOLD from row fill merge
- `crates/termrock/tests/text_area_hot_path.rs` — insert test enters editing first
- `crates/termrock/tests/design_gate.rs` — picker underline class

## Public API for presentation

- **Cell cursor:** `TableState::set_cell_nav(true)` + `set_focused_column(Some(id))` + `Table::focused(true)`. Default `cell_nav=false` keeps row-select `›` + tint. Left/Right do not auto-enter cell-nav.
- **Reverse cell:** `DesignSystem::reversed()` = canvas on `text_primary` + BOLD. Never `Modifier::REVERSED`.
- **Table columns:** `ColumnWidth::Min(n)` now absorbs leftover after Fixed (junie Min). Drop Status via `.priority(20)` and render the **same** story at 52×6: Task grows 24→33. Do not mint a second Fixed-33 story.
- **DataTable gap:** `resolve_paint_widths_with_gap(budget, system.spacing.column_gap, out)`. Id cells fill the column with `TextSecondary` (padding included).
- **Grid header `⚷`:** `DataColumn::primary()`. Header origin is faint `⚷` (junie overdraw of `"▪ "`). Body Id cells stay `TextSecondary`. Catalog can crop the header.
- **Framed pane scroll:** `Panel::vertical_scroll(content_len)` + optional `.scroll_offset(n)`. Title row paints two faint blanks before `─╮` (junie empty `meta` `"  "`). Body gutter uses `scroll::paint_overflow_scrollbar` / `overflow_thumb` (`len = (viewport * track) / content`). Host wraps copy at `Panel::scrolled_content_area(body)` (`width - 2`). `full_cell_thumb` stays the subcell `tui-scrollbar` rounding; line widgets must not use it for junie thumbs.

## Handoff / do not duplicate

- Table/grid/editable lookbook stories + `verify/junie` crops in this commit are **API consumers** of `cell_nav` / Min leftover / DataTable gap-2. Presentation owns the unified catalog and may relocate those stories; do not reimplement the widgets.
- PNG bless (`dialog/confirm-run`, `panel/framed-pane` missing from Jackin subset) is presentation.
- Workspace `png_baselines` red is presentation (subset stories without baselines / intended Junie paint drift). Core will not run `mise run bless-pngs`.
- Pre-existing panel ellipsis tests are core defects; this agent fixes the painter.

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

Remaining SKIP (`verify/junie`, 11 product shells):
- showcase overview / settings / taskrunner — 120×40 and 80×24
- tablepro default 120×40 and 80×24, local 120×40, production 120×40, help 120×40

Panel scrollbar API landed (`vertical_scroll` / `scrolled_content_area` / `overflow_thumb`). `panel/framed-pane` consumes it; no story-local thumb/title overdraw.

Lookbook+showcase merge: not started; not done in this slice. No commit (orchestrator).
