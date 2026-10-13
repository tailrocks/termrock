# DataTable cursor vs scene focus (premium bar)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0068-v0.13.0-data-table-cursor.md` |

## Problem

`DataTableOutcome::FocusChanged` and `focus_row`/`focus_col` conflate **table-internal cursor** with **scene focus**. Hosts already own surface focus via InteractionScene.

## Decisions

1. Rename **FocusChanged → CursorMoved**; **focus_row/col → cursor_row/col**.
2. Table cursor is **valid state** owned by DataTableState (like list selection) when the **host** has given the surface input via `accepts_input`.
3. Paint: `DataTable::focused(bool)` for scene ownership chrome; cursor gutter separate from selection.
4. `handle_intent` + `default_data_table_intent`.
5. `handle_mouse` wheel + click-to-cursor + double-activate path.
6. Empty / loading / error: non-color glyphs + RetryLoad.
7. `ascii` + `colorless` paint flags.
8. tokens param → `system` naming.

## Not doing

- Moving row cursor to InteractionScene (would explode scene ids for 1M virtual rows).
- Selecting unloaded rows.
