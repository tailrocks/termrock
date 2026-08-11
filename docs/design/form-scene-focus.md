# Form scene-owned field focus (Break F remainder)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0067-v0.13.0-form-scene-focus.md` |
| **Related** | pre-1.0 Break F §2.2, M3 interim |

## Problem

`FormState` owns `focused: Option<Id>` and emits `FocusChanged`. That is a **second focus authority** vs `InteractionScene`.

## Decisions

1. **Remove** `FormOutcome::FocusChanged`.
2. **Field focus paint** from `Form::focused_field(Option<&Id>)` (host = `scene.focused()`).
3. **`FormState` does not mutate field focus on Tab/arrows/click-to-focus.**
4. **`handle_key` / `handle_intent`:** Activate (Enter) + scroll/page only; requires host-passed focused id for activate.
5. **`click`:** Activated only when hit id equals host-focused field; otherwise `Ignored` (host calls `scene.focus`).
6. **`active` → `accepts_input`** (rename).
7. **Viewport follow:** host may call `ensure_visible(id)` after scene focus change.
8. **Intents:** `default_form_intent` — Activate, Page, Cancel; Move reserved for host scene field cycle.

## Forbidden

- Private `focused` that Tab still mutates while scene also owns focus.
