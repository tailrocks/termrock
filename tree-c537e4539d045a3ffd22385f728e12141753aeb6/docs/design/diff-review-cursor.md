# DiffReview hunk cursor vs scene focus (premium bar)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0072-v0.13.0-diff-review-cursor.md` |

## Problem

`DiffReviewOutcome::HunkFocused` and private `hunk_index` conflate **hunk cursor**
with **scene focus**. Keys are ungated, paint ignores the current hunk, there is
no intent/mouse path, no accepts_input, and no ASCII/colorless/narrow recipes.

## Decisions

1. Rename **HunkFocused → HunkCursorMoved**; **hunk_index → hunk_cursor** (`hunk_index()` deprecated).
2. Hunk cursor is stream-local; scene owns surface focus (`focused` + `accepts_input`).
3. `handle_key` / `handle_intent` take `&[DiffHunk]` (not bare count) so nav can scroll to hunk start.
4. **n/p** product chords = hunk step; **j/k / arrows / page** = line scroll (`Scrolled`).
5. Enter → `HunkActivated`; **s** / Toggle → `ToggleMode` (split preference; narrow forces unified paint).
6. `handle_mouse` wheel = line scroll; click line maps to hunk containing that line.
7. Paint: hunk gutter on active hunk lines; empty state; `ascii` / `colorless`; tiny shows header strip.
8. `render(..., &mut state)` stores hit geometry + cursor-follow scroll.
9. Multi-file model stays out of scope (agent A5); single projected file only.

## Not doing

- Owning patch text or git stage (Activate/Stage are outcomes only).
- Split two-column layout math (preference flag only until wide split lands).
- File tabs (host/composition).
