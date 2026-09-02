# Parallel migration coordination

Two agents. One tree. Claim before write. Never revert the other agent's files.

## Owners

| Agent | Owns | Does not own |
|---|---|---|
| **Core** (this agent) | `crates/termrock/src/**` tokens, widgets, layout, interaction, public APIs, `crates/termrock/tests/**` | lookbook stories, showcase, catalog, verify/junie, PNG/SVG goldens |
| **Presentation** (worktree `presentation/junie-showcase-2026-09-02`) | `crates/termrock-catalog/**` (canonical one catalog app), `crates/termrock-lookbook-web/**` as thin host, `verify/junie/**` parity model, workspace `default-members`. Lookbook/showcase crates scheduled for deletion after extract. | widget paint, token resolvers, Table/DataTable/VirtualGrid layout |

## Core claimed (this commit)

Landed reusable APIs (catalog/preview must use these; no page-local forks):

- `crates/termrock/src/widgets/table.rs` — `TableState::cell_nav`; `ColumnWidth::Min` grows leftover (junie `Constraint::Min`)
- `crates/termrock/src/widgets/data_table.rs` / `data_view.rs` / `tree_table.rs` — column gap = `spacing.column_gap` (2)
- `crates/termrock/src/widgets/virtual_grid.rs` — same gap 2 (was 1)
- `crates/termrock/src/widgets/panel.rs` — border title/footer ellipsis (was Clip); `Panel::vertical_scroll` + title-track reserve + overflow gutter
- `crates/termrock/src/scroll/mod.rs` — `overflow_thumb` (junie `ScrollState::thumb`); `paint_overflow_scrollbar`
- `crates/termrock/src/widgets/picker.rs` — query underline is editing D5; hint chord `A-↵` not `Alt+Enter`; overflow gutter uses `paint_overflow_scrollbar`
- `crates/termrock/src/widgets/list.rs` — overflow gutter uses `paint_overflow_scrollbar` / `overflow_thumb` (was `render_scrollbar` / `full_cell_thumb`)
- `crates/termrock/src/style/junie.rs` — `gutter()` colour-only; BOLD from row fill merge
- `crates/termrock/tests/text_area_hot_path.rs` — insert test enters editing first
- `crates/termrock/tests/design_gate.rs` — picker underline class

## Public API for presentation

- **Cell cursor:** `TableState::set_cell_nav(true)` + `set_focused_column(Some(id))` + `Table::focused(true)`. Default `cell_nav=false` keeps row-select `›` + tint. Left/Right do not auto-enter cell-nav.
- **Reverse cell:** `DesignSystem::reversed()` = canvas on `text_primary` + BOLD. Never `Modifier::REVERSED`.
- **Table columns:** `ColumnWidth::Min(n)` now absorbs leftover after Fixed (junie Min). Drop Status via `.priority(20)` and render the **same** story at 52×6: Task grows 24→33. Do not mint a second Fixed-33 story.
- **DataTable gap:** `resolve_paint_widths_with_gap(budget, system.spacing.column_gap, out)`. Id cells fill the column with `TextSecondary` (padding included).
- **Grid header `⚷`:** `DataColumn::primary()`. Header origin is faint `⚷` (junie overdraw of `"▪ "`). Body Id cells stay `TextSecondary`. Catalog can crop the header.
- **Framed pane scroll:** `Panel::vertical_scroll(content_len)` + optional `.scroll_offset(n)`. Title row paints two faint blanks before `─╮` (junie empty `meta` `"  "`). Body gutter uses `scroll::paint_overflow_scrollbar` / `overflow_thumb` (`len = (viewport * track) / content`). Host wraps copy at `Panel::scrolled_content_area(body)` (`width - 2`). `full_cell_thumb` stays the subcell `tui-scrollbar` rounding; line widgets (panel, picker, list) must not use it for junie thumbs.

## Handoff / do not duplicate

- Table/grid/editable lookbook stories + `verify/junie` crops in this commit are **API consumers** of `cell_nav` / Min leftover / DataTable gap-2. Presentation owns the unified catalog and may relocate those stories; do not reimplement the widgets.
- PNG bless (`dialog/confirm-run`, `panel/framed-pane` missing from Jackin subset) is presentation. List overflow now uses `overflow_thumb`; `list/scroll-rows` PNG rewrite is the intended junie thumb (verify 0/0).
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

## Presentation claimed (blocking widget gap)

- `crates/termrock/src/widgets/field_message.rs` — help/error row clip used `take_display_cols` (hard clip). Source `TextInput`/`TextArea` help uses `truncate(..., width)` with `…`. Core current claim list does not include this file. Smallest reusable fix: `truncate_cols` + `GlyphSet::ellipsis` in `paint_field_message`. Evidence: Inputs 120×40 L8 C117 `checkou` vs `checko…`.
- `crates/termrock/src/widgets/text_area.rs` — empty-document placeholder used `display_cols_slice_into` (hard clip). Source `TextArea` placeholder uses `truncate(..., inner.width)` with `…`. Core current claim list does not include this file. Also switched body gutter from `paint_scrolled_region` to `paint_overflow_scrollbar`. Evidence: Forms 120×40 L11 C68 `don` vs `do…`.
- `crates/termrock/src/widgets/text_input.rs` — empty-field placeholder used `take_display_cols` (same source `truncate` contract as TextArea). Core current claim list does not include this file.
- `crates/termrock/src/widgets/list.rs` — primary label used `Buffer::set_line` (hard clip). Source `fit(label, lw)` truncates with `…`. Core claimed this file for overflow gutter (`paint_overflow_scrollbar`); that path is not touched. Evidence: Lists 120×40 L8 C75 `auth.r` vs `auth…`.
- `crates/termrock/src/widgets/table.rs` — column budget was `width - 3` (gutter only). Source `cols_area` is `width - 5 - scrollbar` and paints header `…` plus body overflow gutter. Min leftover solver / `cell_nav` untouched. Evidence: Tables 120×40 L6 C64 Task ate the chrome cells (`Owner` shifted).
- `crates/termrock/src/widgets/data_view.rs` — `DataColumnWidth::Min` did not absorb leftover (treated as Fixed). Source `Constraint::Min` grows. Same solver as `table.rs` Min leftover. Core claimed this file for gap=2; solver path is additive.
- `crates/termrock/src/widgets/data_table.rs` — column budget missing junie trailing 2; header overflow painted `›` not `…`. Evidence: Editable tables 120×40 L6 C64. Numeric cells/headers used left `set_stringn`; source Number is `fit_right`. Evidence: Data grid 120×40 L7 C86 `3` vs space. Body overflow gutter missing; source DataGrid paints `scrollbar::render_vertical` on the body track only. Evidence: Data grid 120×40 L7 C117 `┃`.
- `crates/termrock/src/widgets/progress.rs` — running suffix `"  "` was trimmed so `suffix_w=0`; source always reserves `pct_w+2`. Evidence: Progress 120×40 L6 C91 `─` vs space.
- `crates/termrock/src/widgets/code_block.rs` — no overflow gutter. Source CodeEditor `has_sb` shrinks text and paints line thumb. Evidence: Code editor 120×40 L6 C81 `┃`.
- `crates/termrock/src/widgets/tag_chip.rs` — `Chip::measure_width` was gutter+label+remove+1 (label+4 removable). Source ChipBar is `1+label+1+removable 2+1` (label+5). `TokenStrip` scroll reserved add-button width and clipped the last chip; source paints full chips then add only in leftover. Evidence: Chips & selects 120×40 L6 C65 then C102.
- `crates/termrock/src/widgets/select.rs` — form label/value at `area.x`; source Select is `area.x + 2`, chevron at `right-2`, help at `x+2`. Evidence: Chips & selects 120×40 L13 C28 `S` vs space.
- `crates/termrock/src/widgets/status_strip.rs` — `StatusStrip` is ` · ` + semantic glyphs, not source `segments::render` (2-cell gap, left/right). Added `LineSegment` / `paint_line_segments`. Evidence: Chips & selects 120×40 L22 C28 `✓` vs space.
- `crates/termrock/src/widgets/tabs.rs` — active rule painted `x+1..right` (w-1). Source is `x+1..x+w-1` (w-2). Evidence: Settings 120×40 L5 C35 `━` vs `─`.
- `crates/termrock/tests/design_gate.rs` — `tab_active_underline_is_an_accent_rule` expected 7 `━` (old `right` exclusive). Source-faithful count is 6.

## Presentation claimed (in flight)

- `crates/termrock-catalog/**` — one Junie-style catalog (`termrock-catalog` + `tablepro` bins). Workspace `default-members = ["crates/termrock-catalog"]` so `cargo run --release` launches it.
- `verify/junie/parity-manifest.json`, `verify/junie/delta-manifest.json`, `verify/junie/source-headless/`
- `crates/termrock-lookbook-web/**` — thin WASM host over `termrock_catalog::host`
- `crates/termrock-lookbook/**` — bin is the canonical catalog (`termrock_catalog::run`). Lib stories remain for raster unit tests only; studio CLI/goldens/PNG baselines deleted.
- `crates/termrock-showcase/**` — thin `termrock_catalog::run()` alias so `cargo run -p termrock-showcase --release` is the same shell (criterion 1). Agent-workbench deleted. Not a second UX.

Source refs: `main` `e43cf670d6cb793e5761819e8778600797bbf1aa`; `jackin` `f0c262173c74459e21774783c7f4b0ff7e4fe8de`. Rebased onto `60c3d44a` (core overflow_thumb / panel scroll).

## Presentation status

- Source prefix pages 1–20 live. `junie-reference` hides Applications/TablePro; default TermRock identity shows them.
- TablePro is the shared workbench: `cargo run --bin tablepro`, `--connect Production`, Applications mount, same `db`/`sql`.
- Fail-on-first cell compare: `crates/termrock-catalog/tests/parity.rs` vs `verify/junie/source-headless/`.
- Event-driven 63-shot capture: `crates/termrock-catalog/src/scenarios.rs` + `capture::replay` + `tests/shots.rs::fail_first_shots_five_artifacts` vs immutable source `shots/` (txt, cursor flag, ansi/html cell grid, termrock-raster PNG of both grids). Native `run()` now calls `on_tick`.
- Fail-first 120×40 MATCH through Task runner (source prefix 1–20). `junie_reference_120x40_matches_source_headless_fail_first` green.
- TermRock extras pages Feedback/Overlays/Charts/Structure + `public_ui_inventory` coverage test. Lookbook bin is catalog; competing studio CLI/goldens removed.
- v4 workspace gates (fmt/clippy/test/nextest/release/doc) pass in this worktree. Launch probes: `cargo run --release -- --help` twice MATCH; lookbook/showcase same stdout; tablepro `--help` twice MATCH; tmux 120×40 Overview + `tablepro --connect Production` workbench. Evidence: `/var/folders/8p/h376l_nn3375kyj72czdq2x80000gn/T/grok-goal-f4a73375df19/implementer/final-report.md`.
- Known remaining: extras title-cards for most leftover PublicUiIds; lookbook `stories.rs` still compiled for raster tests; unfiltered fail-first still dies on `f_80x24_taskrunner` (16-page vs 20-page nav); remaining `s_editor_complete`/`s_*`/`t_*` TablePro (Safe Mode ack / EXPLAIN / filter chips / many chords). PNG vs source Python/FreeType is not NC.
- Five-artifact MATCH: idle 120×40 source prefix; `s_chips`, `s_chips_80`, `s_chips_select` (Sort popup `╭` at (28,15); strip paints after select so `╰` sits under Segment strip); `s_editor` 120×40 after Tab (editor focused).
- `Select` popover list is source `popup::surface` (border-only, `BorderFocused` no bold) + `▎› label` rows. OverlayFocused BOLD was the (28,15) ansi miss.
- `CodeBlock` gutter: always `▎`; numbers right-aligned at x+2; off-block numbers muted; field fill includes scrollbar column.

## Public API gaps (presentation)

- Overview swatch `info` `#8787ff`: source lists it; `JunieTheme` omits the dormant token. Page paints the source hex as fixture data.
- `TextArea::handle_key` Tab/BackTab indents/outdents; source TextArea Tab commits and yields `CommittedTab` so the page can move focus. Catalog intercepts Tab, ends editing, returns `Route::Ignored` so the shell ring advances.
- `TextArea` paint treats `accepts_input` as focused. Blur must clear the input gate or an unfocused area still paints the focus gutter; that also ends an in-flight edit.
- `TextArea` placeholder used hard clip; source truncates with `…`. Same class as `paint_field_message` / TextInput placeholder.
- `TextInputState` has no `fn(&str) -> Option<String>` validator. Catalog pages own email/name checks and pass `Validation::Invalid`.
- `Form` is field-chrome (label/status/summary), not a host that paints live `TextInput`/`TextArea`/`RadioGroup`/`Checkbox`/`Switch`. The Forms page composes those widgets directly, matching source.
- `List` hover is pointer-position (`ListState::hover`); catalog `RenderCtx` exposes hover id, not pointer. Row hover lift is weaker than source until List accepts a hovered id.
- `List` label paint uses `set_line` clip, not `fit`/`truncate` with `…`.
- `PageCtx` has no `focus.next(ring)`. Tab while editing commits then `Route::Ignored` so the shell ring moves.
- No block-aware `CodeEditor`: catalog Code editor page composes `TextAreaState` + `CodeBlock` + `RoleTokenSyntax` + `CompletionMenu`.
- `CompletionMenu` is overlay-stack oriented; catalog paints it in-page. No `Ctrl+Space` intent in `TextArea`.
- `PageCtx` has only `Request::Status`. Source Grid `cx.open(Dialog::facts…)` / Pickers `ctx.begin_modal()` have no catalog equivalent.
- `PickerOutcome` is `{QueryChanged, CursorMoved, Activated, Cancelled}`. Source `ChosenAlt` / `NextScope` / `Secondary` are page-owned.
- `TokenStrip::lead` paints source ChipBar `" {lead} "` in `TextMuted` on `Surface`, then a one-cell gap. Catalog Filters uses that; clickable `lead` region is still page-owned.
- `StatusStrip` is one priority-dropped row, not left/right `segments::render`.
- `DataTable` has no typed `CellValue` / pending-change queue / SQL preview / `apply_commit_result`.
- `DataTable` Enter is `Activate`; source Enter edits. Page intercepts Enter to start `editing`.
- `ScrollAreaState::handle_mouse` is wheel-only. Thumb click uses `termrock::scroll::offset_for_track_position_u16`.
- `ProgressBar`/`Spinner` need `FrameTick`; catalog `RenderCtx` only has `interaction.tick: u64`.
- `Select` popup is not an `OverlayStack` modal; catalog paints the list into the card.
- `RenderCtx` has no `begin_modal`; picker/completion/preview do not trap shell Tab unless the page consumes it.

