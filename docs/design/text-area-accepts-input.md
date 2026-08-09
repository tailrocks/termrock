# TextArea accepts_input + premium editor chrome

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0078-v0.13.0-text-area-accepts-input.md` |

## Problem

`TextAreaState::set_focused` is residual dual-focus vocabulary (0062 deferred).
PromptComposer embeds TextArea and never promoted editor focus, so key routing
relied on weak tests. Need: accepts_input gate, intent map for nav/cancel,
surface chrome recipes, Scrolled vs Changed, read-only, ASCII scroll glyphs.

## Decisions

1. `focused` → `accepts_input` (+ deprecated aliases).
2. `default_text_area_intent` for arrows/page/Home/End/Esc (not char insert).
3. `handle_intent` for navigation; chars stay on `handle_key`.
4. Wheel-only scroll → `TextAreaOutcome::Scrolled`; caret move → `Changed`.
5. `read_only` blocks edit, allows scroll when accepts_input.
6. Widget: `ascii` (scroll glyphs), `colorless` caret, surface panel from accepts_input.
7. PromptComposer propagates accepts_input to embedded editor.

## Not doing

- Full selection model (composer owns selection overlay).
- IME / multi-cursor.
