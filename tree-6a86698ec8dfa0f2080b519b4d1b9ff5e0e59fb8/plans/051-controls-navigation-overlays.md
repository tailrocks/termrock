# Plan 051: Complete forms, selection, navigation, and overlays

> **Executor instructions**: Implement binding component contracts exactly;
> compose Plan 050 primitives and Plan 040 layers instead of local substitutes.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets crates/termrock/src/interaction crates/termrock/src/keymap.rs crates/termrock-lookbook docs/design/component-anatomy-spec.md docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 040, 043–045, 048, and 050 are DONE and gate is green.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plans 040, 043–045, 048, and 050
- **Category**: components, forms, navigation, overlays, accessibility
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

Applications cannot feel coherent if every settings form, dropdown, context
menu, tooltip, or sidebar is bespoke. These controls are the largest remaining
shadcn-style gap after kernel convergence. They also exercise the hardest shared
contracts: controlled values, roving focus, nested layers, outside click,
disabled state, narrow collapse, and exact key discovery.

## Current state

- Form/TextInput/TextArea/Picker/Tabs/CommandPalette exist with uneven contracts.
- Checkbox/Radio/Switch/Select/MultiSelect, Sidebar/Breadcrumbs/Menu/
  ContextMenu, Drawer/Popover/Tooltip are missing as standalone components.
- CompletionMenu is not a general Menu. Picker should become canonical Combobox.
- Plans 040/044 define scene/action/intent grammar; 043/045/050 define paint and
  anatomy. This plan owns migration `0044`.

## Scope

**In scope**:

- harden TextInput, TextArea, Form, Tabs, ActionBar, HintBar, StatusBar,
  CommandPalette;
- Checkbox, RadioGroup, Switch, Select, MultiSelect, Combobox;
- Sidebar, Breadcrumbs, Menu, ContextMenu;
- Drawer, Popover, Tooltip;
- shared controlled-value/outcome and overlay-placement contracts;
- stories/contracts/docs/API/migration `0044`.

**Out of scope**: data grids/logs (052), app routing, validation business rules,
focus/effect persistence, remote options fetching, compatibility aliases for
Picker/Banner-style renamed APIs.

## Git workflow

Clean `main`; Conventional Commit; `rtk git commit -s`; Codex co-author. Each
family independently green. Migration/docs/catalog ship with breaking exports.

## Steps

### Step 1: Lock shared control/layer laws

Generate reusable tests: controlled value changes only through outcomes;
disabled/loading/read-only never mutate; one key/pointer gesture emits once;
roving focus skips disabled; scene availability equals hints; top-layer
Trap/Dismiss/Bubble; outside click only top; focus restores; placements flip/
clamp within viewport; narrow/tiny keep value/label/focus cue; ASCII/no-color.

### Step 2: Finish form controls

Checkbox/RadioGroup/Switch use controlled projections and stable-ID outcomes.
Select composes trigger + scene Menu. MultiSelect composes checks/Tags and
membership outcomes without owning caller dataset. Evolve Picker into Combobox
with select-only/free-text variants and explicit query-vs-results intents.
Harden TextInput/TextArea batch paste, repeat/release, cursor, read-only, and
submit/newline contracts from Plan 041. Form owns layout/focus/validation chrome,
never validation rules.

### Step 3: Finish navigation

Tabs use stable IDs/roving focus/close outcomes/priority parts. Sidebar supports
expanded/rail with controlled selection. Breadcrumbs collapse middle items to
overflow Menu. Formalize ActionBar as Buttons, HintBar as Keymap projection,
StatusBar as priority slots. All use DesignSystem and InteractionScene.

### Step 4: Build one Menu model and anchored overlays

Menu supports separators, checked/disabled items, shortcuts, nested submenus,
one-level Escape, pointer/keyboard, and borrowed item tree. ContextMenu reuses
Menu at pointer anchor. Drawer uses scene modal layer and Workspace-aware area.
Popover is non-modal anchored placement. Tooltip is FrameTick-delayed focus/
hover help with no focus theft. CommandPalette reuses Combobox/List actions and
two-stage Escape. No overlay stores callbacks/effects.

### Step 5: Studio evidence and migration

Scripts cover every binding story plus nested menus, outside click, placement
flip, disabled roving focus, controlled value roundtrip, async loading
projection, compact/tiny, Unicode/ASCII/no-color. Trace focus/layer/action/
outcome IDs.

Write `migrations/0044-v0.12.0-controls-navigation-overlays.md`: removed/
renamed types, exact consumer changes, controlled state, scene lifecycle,
before/after Picker→Combobox and overlay examples, commands. Update all docs/
contracts/inventory/previews/traces/MIGRATING.

**Verify**: family/unit/integration tests; generated completeness; Studio check;
placement model; warmed visible-item allocation; check and gate pass separately.

## Test plan

- Shared controlled-control and overlay law suites.
- Form/input Unicode/paste/event tests.
- Roving-focus/menu/submenu/placement/focus-restore model tests.
- Narrow/tiny/capability render matrix and deterministic Tooltip clock.
- Inventory/scenario evidence and hot-path tests.

## Done criteria

- [ ] All listed controls/navigation/overlay contracts implemented.
- [ ] Values are controlled or explicitly state-owned; effects remain outcomes.
- [ ] One Menu/placement/layer truth serves Select/ContextMenu/Palette/etc.
- [ ] Focus, Escape, outside-click, disabled/loading are deterministic.
- [ ] Picker and thin/duplicate public paths have canonical replacements only.
- [ ] Migration `0044`, docs/evidence/previews/traces/API fresh; gates pass.

## STOP conditions

- Prerequisites not DONE; non-main/dirty tree; `0044` claimed.
- Correct UI requires domain validation/fetch/routing/effects inside TermRock.
- Overlay would bypass InteractionScene or copy Menu behavior.
- Terminal input cannot distinguish a claimed chord; document/change the shared
  contract rather than pretending support.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Future controls must reuse activation, Menu, overlay placement, controlled-value,
and Studio contract harnesses introduced here.
