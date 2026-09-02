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
- `crates/termrock/src/widgets/panel.rs` — border title/footer ellipsis (was Clip)
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

## Handoff / do not duplicate

- Table/grid/editable lookbook stories + `verify/junie` crops in this commit are **API consumers** of `cell_nav` / Min leftover / DataTable gap-2. Presentation owns the unified catalog and may relocate those stories; do not reimplement the widgets.
- PNG bless (`dialog/confirm-run`, `panel/framed-pane` missing from Jackin subset) is presentation.
- Workspace `png_baselines` red is presentation (subset stories without baselines / intended Junie paint drift). Core will not run `mise run bless-pngs`.
- Pre-existing panel ellipsis tests are core defects; this agent fixes the painter.

## Protocol

1. Edit only claimed paths. Add a claim line before touching a new file.
2. API gap: write it under **Public API for presentation**; core implements in the library, never a showcase-local workaround.
3. Integrate `git pull --rebase` before push. Never `git reset --hard` the other's commits.
