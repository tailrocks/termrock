# Dialog / ChoiceDialog action cursor vs scene focus

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0073-v0.13.0-dialog-action-cursor.md` |

## Problem

`ChoiceDialogState.focused` and `ActionBarState.focused` use **scene-focus
vocabulary** for **action-bar cursor**. Hosts that register action ids on
`InteractionScene` (lookbook focus trap) already own Tab; the dialog still
looks like a second focus authority. Missing: accepts_input gate, shared
intent map, ASCII/colorless action chrome, narrow action stacking.

## Decisions

1. Rename **focused → cursor** on ChoiceDialogState and ActionBarState
   (`focused` field deprecated alias via methods if needed; field rename is break).
2. Action cursor is **valid local state**; hosts may **project scene focus into
   `cursor` each frame** (lookbook pattern) without dialog owning Tab.
3. **Tab is not handled by ChoiceDialog** when host uses scene — only Left/Right
   (or j/k) move the local cursor; `default_choice_dialog_intent` maps those.
4. Esc → Cancel; Enter → Activate; gated by `accepts_input` + loading.
5. Overlay open/place APIs stay; surface chrome via `Dialog::emphasis` /
   variant; host sets Focused when overlay is top.
6. Narrow: stack actions vertically when width &lt; 28 or row overflow.
7. ASCII / colorless: ActionBar `[label]` cursor marks when requested.
8. MessageDialog / Backdrop: keep; handbook + tokens naming cleanup.

## Not doing

- Moving every action into InteractionScene mandatorily (hosts may still do so).
- Owning app loop / open policy.
- Multi-step wizards (FormWizard).
