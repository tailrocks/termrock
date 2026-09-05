# PROGRESS — Junie showcase golden parity loop

Updated: 2026-09-05, immediately after commit `e8c53407` (pushed to
`origin/wip/junie-showcase-loop-2026-09-05`). Full context and operating
manual: `HANDOFF.md`.

## Done

- All 63 scenarios in `crates/termrock-catalog/src/scenarios.rs` rebuilt from
  the source manifest (`/tmp/newscenes/manifest.json`) as the event authority.
  Deleted the entire `seed_sql` / `run_ticks_left` seeding machinery (no-legacy
  rule): `Scenario.seed_sql`, `Scenario.run_ticks_left`, `tp_sql*`,
  `tp_table*` helpers, `TableStateSeed`/`TableFocusSeed`,
  `set_next_run_ticks_left`, `set_active_query_run_ticks_left`,
  `set_input_cursor` + its test. Constructors are now `cat(...)` and
  `tp(id, cols, rows, connect, steps)` only.
- Mouse replay: tmux one-based → crossterm zero-based translation.
- Cursor artifact semantics derived and implemented (persistent print-run
  position; no cursor movement on mouse events).
- Source-true behaviors implemented and verified green through the loop:
  scroll badges only after scrolling, nav hints when sidebar focused,
  sorted-only header brightening, scrollbar focus propagation, lists empty-pane
  inset, `truncate_middle_cols` in `data_view` cell ellipsis policy
  (`CellEllipsisPolicy { Clip, End, Middle }`), `DataColumn::filtered`,
  datagrid viewport column accounting (`viewport_columns`,
  `painted_columns_right`), status TTL emulation for `t_80`.
- `reference_nav_for_scene` returns the full 20-entry `SOURCE_NAV`
  unconditionally (no `f_*` fixture filter).
- All work committed and pushed: `e8c53407 wip(catalog): replay the junie
  showcase golden loop` on branch `wip/junie-showcase-loop-2026-09-05`
  (12 files, +347/−717).

## Loop status

Last run: `/tmp/shots_run33.log`. Failing scene: `f_settings_members`,
artifact `ansi`, first mismatch at cell (109,11): cursor-row "Last active"
Numeric digit must be `bold: true` (gray `[128,128,128]` on `[17,17,17]`);
ours paints it without BOLD.

Scenes green through run33 (alphabetically up to the failure, full pass):
`f_80x24*`, `f_buttons_hover`, `f_dialogs*`, `f_editor*`, `f_grid*`,
`f_inputs_edit`, `f_lists_hover`, `f_overview`, `f_panels`, `f_progress`,
`f_scrolling`, plus all `s_*` earlier in the key order and the TablePro
`t_*` scenes verified in prior runs.

## Next step

Fix the focused-row BOLD propagation in
`crates/termrock/src/widgets/paint_data_row` (preferred: carry BOLD in
`RowChrome`'s focused `label_style`, deleting the redundant gutter
`add_modifier(BOLD)` hack; verify against green datagrid scenes first), then
rerun the loop — exact command in `HANDOFF.md`.
