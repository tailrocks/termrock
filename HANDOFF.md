# HANDOFF — Junie showcase golden parity loop

Status snapshot lives in `PROGRESS.md`. This file is the complete operating
manual for whoever (or whatever session) picks the work up.

## Restart prompt

Paste this into a fresh session started inside the worktree
`/tmp/termrock-verify` (or start anywhere, then `cd /tmp/termrock-verify`):

> Work in the git worktree `/tmp/termrock-verify` on branch
> `wip/junie-showcase-loop-2026-09-05` (already pushed to origin; head
> `6b8ad9db`). Read `HANDOFF.md` and `PROGRESS.md` in that directory first —
> they are the authority. Resume task #4: drive the fail-first shots loop
> (`DUMP_SHOT=1 JUNIE_SHOTS=/tmp/newscenes/scenes cargo test -p
> termrock-catalog --test shots fail_first_shots_five_artifacts`) to green by
> fixing the current failure — cursor-row Numeric cell (109,11) in
> `f_settings_members` must paint BOLD — then follow `HANDOFF.md` "Remaining
> work, in order": delete the temp probes, promote `/tmp/newscenes` goldens
> into `verify/junie/reference/`, run the full gate, commit with `git commit
> -s`, push, and merge PR #50. Conventions: caveman ultra-terse replies, `rtk`
> command prefix (`rtk proxy` for raw output), DCO signoff, no legacy code.
> The loop command and every derived source-true behavior are documented in
> `HANDOFF.md`; do not re-derive them.

## Mission

Session goal (user, verbatim): *"Spawn subagents and commit and push everyting
and resolve any conflicts in terms of comparing which implementation is better.
Use reviewer and critical subagents to do verification for that decision."*
Plus: *"We should eventually merge all branches, all PRs and all worktrees
(+branches) to main branch."*

Active task: **green PR #50 and merge to main** — the Junie showcase golden
migration. Drive the fail-first shots loop through all 63 scenes fixing
revealed drift until green; then delete temp probes, promote the new goldens,
run the full gate, commit (`-s`), push, merge.

## Where everything is

| Thing | Location |
|---|---|
| Active worktree | `/tmp/termrock-verify` (branch `wip/junie-showcase-loop-2026-09-05`, pushed to origin, head `e8c53407`) |
| Session launch dir | `/Users/donbeave/Projects/tailrocks/termrock-presentation` (also a worktree — do NOT cd there to work) |
| Source checkout (junie-tui capture session) | `/tmp/jr-e43` |
| New goldens + manifest (authority) | `/tmp/newscenes/scenes/*.txt|.cursor|.ansi|.html|.png` and `/tmp/newscenes/manifest.json` |
| Old committed goldens | `verify/junie/reference/scenes/` + `verify/junie/reference/manifest.json` |
| Our replay frame dumps (probe) | `/tmp/dump/{id}.txt`, `/tmp/dump/{id}.ansi` |
| Shot run logs | `/tmp/shots_runNN.log` (run33 = latest) |

## How to run the loop

```bash
cd /tmp/termrock-verify && DUMP_SHOT=1 JUNIE_SHOTS=/tmp/newscenes/scenes \
  cargo test -p termrock-catalog --test shots fail_first_shots_five_artifacts \
  > /tmp/shots_runNN.log 2>&1; \
  rtk proxy grep -m1 "first mismatch\|test result" /tmp/shots_runNN.log
```

Gotchas:

- **`JUNIE_SHOTS` MUST be set**, otherwise the test reads the stale committed
  goldens from `verify/junie/reference/scenes/` and reports garbage failures.
- **`rtk grep` truncates output** — use `rtk proxy grep` for exact lines.
- One run takes ~2–2.5 minutes; the test panics at the FIRST failing artifact
  of the alphabetically-first failing scene. Artifacts compare in order
  txt → cursor → ansi → html → png.
- `DUMP_SHOT=1` also eprints per-step catalog focus (needs `-- --nocapture`)
  and dumps our frames to `/tmp/dump/`.
- To diff full-ansi vs golden offline there is a Python SGR tokenizer approach
  (parse both `.ansi` into per-cell `(ch, fg, attrs)` grids, diff) — used
  repeatedly; regenerate as needed.

## Established facts (do not re-derive)

- **Manifest is the event authority.** `/tmp/newscenes/manifest.json` lists the
  events per scene. All 63 scenarios in `crates/termrock-catalog/src/scenarios.rs`
  were rebuilt from it. `connect` is NOT recorded in the manifest; TablePro
  scenes keep `connect = Some("Production")` per golden evidence. Seeding SQL ≡
  typed queries: the manifest's typed text (e.g. `SELECT * FROM customers LIMIT
  20`) is what the goldens show.
- **Mouse coordinates are tmux one-based**; crossterm is zero-based. Replay
  translates `(x-1, y-1)` in `apply_step` for `Move`/`Click`/`WheelDown`
  (`crates/termrock-catalog/src/capture.rs`).
- **Cursor artifact** = terminal-persistent position after the LAST contiguous
  print run of the final frame. Initial full-frame paint ends bottom-right
  ("120 39 0" for eventless 120x40 scenes). Mouse events do NOT move the tmux
  text cursor (the `set_input_cursor` hack was removed for this reason).
  `f_80x24` ends "73 12 0" (page-specific last paint run).
- **DataGrid header rules**: only SORTED columns brighten their label (cursor
  column does NOT — `header_label_style(table.system, sorted, false,
  col.sortable)` in `data_table.rs`); suffixes " ∇" (filter) / " ▴" " ▾" (sort)
  are composed AFTER truncation; the primary column reserves a 2-cell "▪ "
  prefix and overdraws ⚷.
- **Scroll range badges** ("1–15 of 24", "cols m–n") appear only when actually
  scrolled away from top (`offset_y > 0`), and for columns only when hscroll
  overflows; viewport counts only FULL-FITTING columns
  (`DataTableState::viewport_columns`, `painted_columns_right`).
- **Shell hints**: `page_hints_when_nav()` default false → nav hints
  ("↑ ↓ Move / Enter Open / Tab Into page / q Quit") shown when sidebar
  focused. Panels hints merged into the default branch.
- **Scrollbar thumb** (`scrollbar_thumb(focused, hovered)`): focused →
  text_primary (white), hovered → text_secondary, else muted.
  `render_source_list` in `pages/lists.rs` passes real `focused`.
- **Status TTL**: 5 s source wall clock. Replay emulates with `Ticks(64)`
  (80 ms ticks) only for `t_80`.
- **Run simulation**: source `Run::new` = 6 ticks; `run_ticks_left` seeding
  machinery DELETED (no-legacy rule). `tabs.rs` `start()` = `self.running =
  Some((statements, 6, explain, 0))`.
- **Lists empty pane**: source insets the empty-pane area 2 cells per side
  inside the card (`pages/lists.rs` `empty_area`).

## Current failure (loop stop point, run33)

```
first mismatch scenario f_settings_members ansi 120x40: cell (109,11)
expected GridCell { ch: "2", fg: [128, 128, 128], bg: [17, 17, 17],
bold: true, ... } got ... bold: false, ...
```

`f_settings_members` = catalog Settings page, members `DataTable` with
`nav_mode = Cell`, steps `Tab, Right, Tab, Down`. The golden wants the
cursor-row "Last active" (Numeric) cell digit painted BOLD (gray on surface).

Analysis so far:

- `paint_data_row` in `crates/termrock/src/widgets/data_table.rs`
  (~line 2130) builds `style` from `RowChrome::resolve(...).label_style(base)`.
  The datagrid full-row fill (line ~2170) and the gutter (line ~2194, which
  adds `Modifier::BOLD` explicitly) carry focus chrome, but the body-cell
  `cell_style` in the **non-datagrid** branch goes through
  `col.kind.cell_style(style, quiet)` and loses BOLD for the Numeric quiet
  tone.
- Structural suspicion: `RowChrome`'s focused `label_style` should carry BOLD
  itself, which would make the explicit gutter `add_modifier(BOLD)` hack
  redundant — that removes the enabling condition instead of patching the one
  cell kind. Verify against the already-green datagrid scenes (`t_*`) and the
  green catalog table scenes before committing to that; if focused-label BOLD
  breaks them, the fallback is bolding the focused row's content `style` at the
  top of `paint_data_row` only.
- Check `/tmp/dump/f_settings_members.ansi` vs the golden
  `/tmp/newscenes/scenes/f_settings_members.ansi` with the SGR tokenizer diff
  to see the full extent (row-wide bold vs single cell) before editing.

## Remaining work, in order

1. **Finish the loop to green.** Iterate failure → fix → rerun until
   `test result: ok` for `fail_first_shots_five_artifacts`. Scenes already
   green this loop: everything alphabetically before `f_settings_members`
   including `f_inputs_edit`, `f_lists_hover`, `f_tables_hover`, `f_panels`
   (the test fails fast, so "before" = all earlier scenes in one full pass;
   after any fix, rerun from the top — it re-verifies everything).
2. **Delete temp probes** (no-legacy rule):
   - `Drive::debug_focus()` in `crates/termrock-catalog/src/capture.rs` and its
     `DUMP_SHOT` eprintln in `replay_catalog`.
   - The `DUMP_SHOT` dump block in `crates/termrock-catalog/tests/shots.rs`
     (`compare_one`).
   - The `.ansi` write in that block (added this loop).
3. **Promote goldens**: copy `/tmp/newscenes/scenes/*` (all 5 artifacts × 63
   scenes) and `/tmp/newscenes/manifest.json` into
   `verify/junie/reference/`.
4. **Full gate** (mise tasks; run all): fmt, clippy `-D warnings`, nextest,
   parity, shots **WITHOUT `JUNIE_SHOTS`** (now reads promoted goldens),
   types:check.
5. **Commit** everything with `git commit -s` (DCO), normal-English message.
6. **Push**, then **merge PR #50** to main.
7. Post-merge: report in the session, then handle the remaining sweep (below).

## Other pending tasks (after PR #50)

- **Send termrock-3f receipts via SendMessage**: their `31dd4330` parity FAILS
  (Tables sort), shots FAIL, and their clippy-clean claim is false — their push
  is superseded by this branch.
- **Task #6**: sweep remaining branches/worktrees to main (keep
  `wip/junie-showcase-workingtree-2026-09-05` as adjudicated-loser archive).
- **Verify termrock-3f's two reported deviations during final review**:
  `code_block` bang = unconditional overwrite of last number cell;
  `dialog.rs` GUARDED footer clear.
- Clean up probe tmux sessions (`junie_cap` etc.) when done.

## Conventions

- Responses: caveman ultra-terse (`/caveman` session style). File/commit/PR
  text: normal English prose.
- Shell: prefer `rtk` prefix (`rtk cargo test`, `rtk git status`); raw output
  via `rtk proxy <cmd>`.
- Commits: always `git commit -s` (DCO signoff).
- Project rules: no legacy code or compat shims — remove old paths completely;
  breaking changes preferred; fix the enabling condition, not the symptom;
  never defer known-wrong state.
- This is a git worktree. NEVER bare `git stash` / `git stash pop` (shared
  stash stack). Use temporary WIP commits instead.
