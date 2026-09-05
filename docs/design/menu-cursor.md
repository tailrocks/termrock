# Dropdown menu / Sidebar cursor vs scene focus

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0069-v0.13.0-menu-cursor.md` |

## Problem

The removed flat menu conflated in-menu **cursor** with **scene focus**. `DropdownMenuOutcome` and `DropdownMenuState` keep the replacement cascade's cursor local while the scene owns surface focus.

## Decisions

1. `DropdownMenuOutcome::CursorMoved` reports local navigation; `cursor_index` exposes the root panel cursor.
2. Cursor is menu-local state when `accepts_input`; host owns surface focus via `DropdownMenuState::set_focused`.
3. `handle_intent` + `default_menu_intent` (j/k, arrows, enter, space, esc).
4. `handle_mouse` click-to-cursor / activate.
5. Non-color cursor `›`/`>`, disabled dim + `⊘`, checked `✓`/`[x]`/`[ ]` ascii.
6. `system` paint param; `ascii` / `colorless`.
7. Same renames on **Sidebar** (parallel surface in same module).

## Not doing

- Putting every menu item on InteractionScene by default (overlay hosts still register hit rects if needed).
