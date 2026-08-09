# PromptComposer accepts_input vs scene focus

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0075-v0.13.0-prompt-composer-accepts-input.md` |

## Problem

`PromptComposerState::set_focused` uses **scene-focus vocabulary** for an
**input gate**. Migration 0062 deferred this residual. Hosts that open
permission/plan/palette overlays must disable keys without clearing draft;
naming must match List/Menu/Picker/Dialog (`accepts_input`).

`chip_focus` similarly implies scene focus for attachment chips.

## Decisions

1. Rename **`focused` → `accepts_input`** (field + primary API).
2. **`set_focused` / `is_focused` deprecated** aliases for one cycle.
3. Rename **`chip_focus` → `chip_cursor`** (list-local chip highlight).
4. Panel chrome uses `accepts_input` (Focused border only when host grants input).
5. Draft still never cleared by accepts_input/blur alone.
6. Product chords (Ctrl+Z/Y, submit policy, history) stay on `handle_key`.
7. Thin `default_prompt_composer_intent`: Esc → Cancel, Enter → Activate
   (submit policy still applied inside handle_intent/handle_key).
8. Overlay helpers (completion / fullscreen) unchanged.

## Not doing

- Owning provider I/O or completion candidate search.
- Moving editor caret into InteractionScene.
- Full TextArea set_focused rename in this commit (separate residual).
