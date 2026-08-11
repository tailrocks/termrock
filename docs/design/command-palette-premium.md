# CommandPalette / Picker premium redesign

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0074-v0.13.0-command-palette-premium.md` |

## Problem

`CommandPalette` is a thin always-Focused `Panel` shell over `Picker` with no
accepts_input gate, no surface-focus paint flag, no ASCII/colorless recipes,
no footer/empty premium cues, and `SelectionChanged` naming that confuses
list-local cursor with scene focus. Filtering remains correctly caller-owned.

## Decisions

1. **Picker substrate** (also used standalone):
   - `SelectionChanged` → `CursorMoved`
   - `accepts_input` / `set_accepts_input`
   - `Picker::{focused, ascii, colorless}` paint
   - empty mark `∅` / `[ ]` + message
2. **CommandPalette** composes OverlayStack helpers + Panel + Picker:
   - surface `focused` → `PanelChrome::Focused` vs `Normal`
   - `system` param naming; footer hint row when height allows
   - narrow/tiny: drop footer; tiny may query-only when height &lt; 4
3. Outcomes stay palette-named mirrors of picker (CursorMoved, QueryChanged, …).
4. Intents: list intents for results; query editing stays TextInput path;
   Esc two-stage clear then cancel unchanged.
5. No new generic abstraction beyond shared Picker upgrades (2 consumers).

## Not doing

- Owning fuzzy match / ranking / command registry.
- Async loading model (consumer swaps rows / empty message).
- Global key chord to open (host).
