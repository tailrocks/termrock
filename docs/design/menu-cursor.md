# Menu / Sidebar cursor vs scene focus

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0069-v0.13.0-menu-cursor.md` |

## Problem

`MenuOutcome::FocusChanged` + `focus_index` conflate in-menu **cursor** with **scene focus**. Same pattern as DataTable before `0068`.

## Decisions

1. `FocusChanged` → `CursorMoved`; `focus_index` → `cursor_index`.
2. Cursor is menu-local state when `accepts_input`; host owns surface focus via `Menu::focused(bool)`.
3. `handle_intent` + `default_menu_intent` (j/k, arrows, enter, space, esc).
4. `handle_mouse` click-to-cursor / activate.
5. Non-color cursor `›`/`>`, disabled dim + `⊘`, checked `✓`/`[x]`/`[ ]` ascii.
6. `system` paint param; `ascii` / `colorless`.
7. Same renames on **Sidebar** (parallel surface in same module).

## Not doing

- Putting every menu item on InteractionScene by default (overlay hosts still register hit rects if needed).
