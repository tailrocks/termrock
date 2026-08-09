# Plan 044: Route every collection through universal intents

> **Executor instructions**: Execute in order. Raw-key compatibility paths are
> removed after migration; Keymap remains the chord source.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/interaction/intent.rs crates/termrock/src/keymap.rs crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/tree.rs crates/termrock/src/widgets/table.rs crates/termrock/src/widgets/virtual_grid.rs crates/termrock/src/widgets/picker.rs crates/termrock/src/widgets/completion_menu.rs crates/termrock-lookbook docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 039 and 040 are DONE and the gate is green.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: HIGH
- **Depends on**: Plans 039 and 040
- **Category**: architecture, interaction, accessibility, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — handle_intent on List/Tree/Table/Picker/CompletionMenu/VirtualGrid (migration 0038)

## Why this matters

List partially supports UiIntent; Tree, Table, VirtualGrid, Picker, and
CompletionMenu still interpret raw KeyCode locally. One application-wide Vim/
Simple remap therefore requires widget forks, and footer/palette hints can drift
from actual behavior. The InteractionScene cannot be authoritative while
widgets maintain hidden chord tables.

## Current state

- `default_list_intent` maps a fixed raw-key subset directly to UiIntent.
- List offers `handle_intent`, but raw `handle_key` remains another behavior
  truth.
- Tree/Table/Grid/Picker/CompletionMenu directly match keys and use different
  Press/Repeat/Release conventions.
- CompletionMenu may commit after movement; Plan 040 establishes action routing,
  this plan completes collection state APIs.
- Table sort and activation are caller effects/outcomes; keep that boundary.
- Plan 039 fixes Grid bounds/IDs/enabled policy; reuse it.
- This plan owns migration `0037`.

## Scope

**In scope**: typed intent families; Keymap→scene→intent bridge; List, Tree,
Table, VirtualGrid, Picker, CompletionMenu migration; discoverable action/hint
consistency; input-kind policy; stories/contracts/docs/API; migration `0037`.

**Out of scope**: text editor command system beyond Picker query split,
application keymaps, effects, product commands, old raw-key compatibility,
row visuals (043/045).

## Git workflow

Clean `main` only. Conventional Commit with `rtk git commit -s` and
`Co-authored-by: Codex <codex@openai.com>`. Each commit passes check; push only
after full gate.

## Steps

### Step 1: Lock cross-widget intent laws

Add one reusable behavior suite applied to every collection:

- Next/Previous/First/Last/Page move with empty, one, disabled, and reordered
  projections;
- Activate never occurs on Move;
- disabled targets never activate;
- Repeat moves/scrolls; Release is ignored; Activate is Press-only;
- remapping one test key from Next→Previous changes List and Tree identically;
- scene action availability, hint output, and handler acceptance match;
- stable IDs survive reorder and outcomes preserve resident IDs;
- Picker printable edits query while navigation targets results;
- Tree expand/collapse and Table sort emit neutral outcomes only.

### Step 2: Define composable intent families

Evolve the vocabulary without one giant product enum:

- navigation/page/scroll intents;
- activate/toggle/cancel;
- tree expand/collapse/toggle branch;
- table/grid sort/column/range intents;
- query edit vs results navigation for Picker;
- completion commit/cancel/details.

Use small typed enums/adapters or a generic `IntentHandler` trait. Keymap maps
chords to typed application/widget actions; adapters translate those actions
to intents. Do not add a second raw-key parser beside Keymap.

### Step 3: Migrate collections and delete raw-key truth

Migrate List first as contract canary, then Tree, Table, VirtualGrid, Picker,
CompletionMenu. Each state exposes `handle_intent(projection, intent)` and
returns typed neutral outcome. Geometry stays InteractionScene-owned.

Remove public `handle_key` methods that encode defaults. If convenience defaults
remain, they must be Keymap constructors/adapters outside widget state and feed
the exact intent path. No deprecated aliases.

### Step 4: Make discovery exact

For each component/state, project available actions based on focus, enabled
items, query state, expanded state, scroll/range capability. The same projection
feeds scene routing, HintBar, and CommandPalette. Add tests that unavailable
actions disappear and conflicts in composed keymaps fail deterministically.

### Step 5: Story matrix and migration

Scripts: default vs remapped keys, Repeat navigation, disabled rows, Tree branch,
Table sort request, Grid range, Picker edit/results split, Completion explicit
commit/cancel. Traces record action→intent→outcome and stable target ID.

Write `migrations/0037-v0.12.0-universal-intents.md` with removed raw handlers,
adapter construction, exact consumer event-loop changes, before/after code,
ownership, commands. Update docs/contracts/previews/API/MIGRATING.

**Verify**: focused collection/interaction tests, lookbook check, then
`rtk proxy mise run check` and `rtk proxy mise run gate` all pass.

## Test plan

- Shared intent law suite across six collection families.
- Component-specific expansion/sort/range/query/completion tests.
- Keymap conflict/availability/hint equivalence tests.
- Scripted semantic traces for default/remapped modes.
- Warmed event routing performs no per-event allocation.

## Done criteria

- [x] Every collection routes behavior through typed intents.
- [x] Keymap is the only chord source; scene owns availability/routing.
- [x] Move never activates; key-kind policy is coherent.
- [x] One remap works across component families.
- [x] Hints/palette exactly match accepted actions.
- [x] No public raw-key behavior path remains.
- [x] Migration `0037`, docs, scenarios, contracts, previews, inventory fresh.
- [x] Full gates pass.

## STOP conditions

- Plans 039/040 not DONE; branch not `main`; dirty tree; `0037` claimed.
- Required behavior needs product commands/effects in TermRock.
- Keymap or InteractionScene semantics drift from Plan 040.
- Migration would retain dual raw-key/intent behavior.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Future collections implement the shared intent law suite before catalog entry.
Plan 046 consumes one application keymap across the flagship workbench.
