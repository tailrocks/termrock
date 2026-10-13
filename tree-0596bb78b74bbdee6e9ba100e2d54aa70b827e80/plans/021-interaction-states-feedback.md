# Plan 021: Every state paints, every action answers — hover, pressed, reveal, and feedback

> **Executor instructions**: Follow this plan step by step. Run every
> verification command. STOP conditions binding. Update `plans/README.md`.
>
> **Drift check (run first)**: plans 004/005/007/008 DONE (this plan builds
> the states those sweeps left as gaps; underline focus/hover cues are gone
> by now). Re-locate every cited site with `rg` — line numbers are leads.

## Status

- **Priority**: P1 (designer micro-detail directive)
- **Effort**: XL (widest per-widget sweep in the suite; steps are
  independent commits)
- **Risk**: MED (golden churn; two behavior bugs fixed)
- **Depends on**: plans/004, 005, 007, 008; plan 014 Step 2 for the
  feedback-flash timing (use static 1-frame flash pre-014)
- **Category**: design / correctness
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

The state-coverage matrix (micro-interaction audit) shows: pressed/active
paints distinctly NOWHERE; hover is hard-coded `false` in the nine
menu/picker widgets that pass it to the shared recipe, write-only in five
more, absent in 61 of 86 mouse-handling widgets and in all patterns;
row actions are always-on (P6 inverted); copy/activation produce zero
acknowledgment across the library; and the lookbook physically cannot
demonstrate hover/pressed/loading for 41 of 46 interactors — which is HOW
all of this accumulated unseen. Plus two real bugs: Button's Space arms
forever on terminals without key-release reporting (Space never activates),
and `data_table` forces every column sortable via `|| true`.

## Current state — verified leads (re-locate before editing)

- Recipe inputs hard-coded: `dropdown_menu.rs:1264-1266`, `menu_bar.rs:1608`,
  `select.rs:1067`, `multi_select.rs:1131`, `command_palette.rs:1643`,
  `quick_open.rs:1646`, `history_picker.rs:1234`, `file_picker.rs:1445`,
  `data_table.rs:1453` — all `hovered: false, loading: false` while several
  already build per-row `hits`.
- No `MouseEventKind::Moved` arm at all: `select.rs:711`, `multi_select.rs:705`,
  `dropdown_menu.rs:952`, `file_picker.rs:1131`, `combobox.rs:822`; zero
  `Moved` in all of `patterns/`.
- Write-only hover: `slider.rs:909,1523`, `panel.rs:340`,
  `code_block.rs:1392`, `table.rs:1122` (hovered_column vs flat header at
  `:1217`).
- Hover collapses to idle after underline deletion: `controls.rs:476-481`
  (Checkbox), `:1304-1309` (Radio), `:2091-2096` (Switch), `toggle.rs:488`,
  `segmented_control.rs:539-545`; Switch `track_style` (`:2039-2073`) has
  no hover branch at all.
- Button hover gated on focus: `primitives.rs:658-660`
  (`hovered && surface`); IconButton branches directly (`:1278`);
  ButtonGroup wires Moved (`button_group.rs:885-899`) and it's discarded.
- Pressed modeled, never painted: `SwitchState.pointer_armed`
  (`controls.rs:1719,2284`), `slider.rs:780` drag==focus, RangeSlider
  ignores `dragging`, split/RPG `is_dragging` not a paint input
  (`split_pane.rs:381-393`, `resizable_panel_group.rs:889`), IconButton
  `toggled || armed` identical (`primitives.rs:1291`);
  `button_recipe` Pressed = BOLD only (`tokens.rs:708-710`).
- Space correctness bug: `primitives.rs:204-216` arms on Press, fires ONLY
  on Release; zero `KeyboardEnhancementFlags` negotiation repo-wide → on
  ordinary terminals Space never fires and the button sticks Pressed.
- Enter fires with no armed frame: `primitives.rs:186-192`.
- Actions always-on: `list.rs:1260-1262` (width-gated only);
  `ListRowRecipe` has no reveal term (`tokens.rs:805,833`);
  `composed_row.rs:55` projection is width-only.
- Sort: markers only when already sorted (`data_table.rs:1389-1400`,
  `table.rs:1245`); header hover tracked, never painted; BUG
  `data_table.rs:1409` `sortable: col.sortable || true`.
- Copy feedback zero across 9 outcome-emitting widgets
  (`attachment_chips.rs:745`, `citation.rs:551`, `code_block.rs:1224`,
  `data_view.rs:842`, `data_table.rs:140,569`, `detail_table.rs:78,227`,
  `diagnostic.rs:1097`, `error_state.rs:978`).
- State gaps: TextArea read-only unpaintable (`text_area.rs:504` vs paint
  `:1601`), InputOtp disabled unpaintable (`input_otp.rs:150` vs `:368-420`),
  Collapsible Section focus==idle (`collapsible.rs:514-523`, inherited by
  3/4 Accordion recipes), tabs focus cue needs height>1 (`tabs.rs:1175,1228`),
  tag_chip status shadows selection/focus (`tag_chip.rs:530-534`),
  ActionBar no hover/pressed/busy (`action_bar.rs:184-203`) while
  `DialogState.loading` spins above pressable-looking buttons
  (`dialog.rs:598,1240`), citation hover latches (`citation.rs:802-806`),
  radio hover ignores disabled (`controls.rs:1463` vs `:1474`), scroll
  thumb painted but not a hit target (`scroll_area.rs:602-642,713-750`),
  semantic `pressed` overloaded (`toolbar.rs:608`, `toggle.rs:604`,
  `controls.rs:645`), AccentRail unwired at `permission.rs:1482` +
  `plan_review.rs:1598`, pagination/stepper/date-picker no hover
  (`pagination.rs:788`, `stepper.rs:690`).
- Lookbook: `PointerTarget::hover` default no-op, 5 of 46 implement
  (`interactors.rs:71,546,621,733,780,877`); no story exercises
  Hovered/Pressed/Loading (`stories.rs:10392` area); the all-states shape
  exists once (`design_system_button_recipes_story`, `stories.rs:10414`).

## Scope

**In scope**: files above; `style/tokens.rs` (ListRowVisualState/recipe
fields; ControlState::Pressed token); `crossterm/session.rs` + `input/`
(keyboard-enhancement detection only); lookbook interactors/stories;
`design_gate.rs`; migrations + `MIGRATING.md`.
**Out of scope**: tween timing (plan 014 owns durations — this plan's
flashes are 1-2 frame static until 014 lands), spacing/truncation
(plan 022), microcopy (plan 020).

## Steps (each a commit; each ends `mise run check` green)

### Step 1: Hover infrastructure

Standard shape: `hovered: Option<Id>`/`bool` assigned UNCONDITIONALLY from
hit-test on `Moved` (`state.hovered = area.contains(pos)` — the
`panel.rs:340` idiom). Apply: add Moved arms + state to the five no-Moved
widgets; populate the nine hard-coded `hovered: false` recipe calls from
their existing `hits`; fix the citation latch; mirror `!disabled` into the
radio Moved arm; drop `&& surface` from Button hover (keep
`!disabled && !loading`); consume the five write-only hover fields
(slider handle wash, panel header wash, code_block line wash, table header
cell hover, whichever `HoverTint`-based); Switch track hover branch; the
five idle+underline hover branches → `HoverTint` wash; patterns: one Moved
arm per hit-owning pattern (approval_queue, connection_manager,
plan_review, session_picker) styling row/action hover; ActionBar
`hovered: Option<Id>`; pagination/stepper/date-picker/breadcrumbs/toolbar/
tag_chip/attachment_chips hover per the same shape.

**Verify**: `rg -n "hovered: false" crates/termrock/src/widgets/` → 0 at
recipe call sites; per-widget hover tests (render with hover set → style
differs from idle).

### Step 2: Pressed/armed + the Space bug

- Session capability: detect keyboard-enhancement (release reporting)
  support in `crossterm/session.rs`; expose on the input layer. When
  absent, Button Space FIRES ON PRESS (fallback) — kills the stuck-armed
  bug; when present, keep press-arm/release-fire.
- `fire_or_confirm` stamps `fired_at`; `ControlState::Pressed` resolves for
  the next 1-2 frames (AnimationDemand wakes one repaint) — Enter/click get
  an armed flash.
- `button_recipe` Pressed gets a real token (REVERSED or press-tint bg —
  pick one, record in migration).
- Paint the modeled-but-invisible pressed states: Switch `pointer_armed`,
  Slider vs RangeSlider drag (one shared chrome — plan 008 Step 4's helper
  gains a `dragging` input), split/RPG `dragging` in the divider match +
  live `%` readout while dragging (data source `ratio()` exists),
  IconButton toggled (REVERSED) ≠ armed (flash).
- Shared `ActionFlash { fired_at }` helper; copy sites flash `✓` in their
  affordance slot ~1s (pilots: code_block, citation; then the other 7).
- Semantic truth: `pressed` = armed only; route checked/selected through
  the existing fields at the three overload sites.

**Verify**: button test: Enter → next frame paints Pressed → frame after
clears; Space activates on a session without enhancement flags (unit test
with the capability stubbed); `rg -n "\|\| true" crates/termrock/src/widgets/data_table.rs` → 0 (delete it here with the sort work below).

### Step 3: Hover-reveal (law P6) + sort affordance

- `ListRowVisualState.revealed = hovered || selected || focused`;
  `ListRowRecipe.show_actions`; `list.rs:1260` consumes it (width ladder
  becomes the fallback cap, not the gate). `ComposedRow` gains the reveal
  input (coordinate with plan 012's `paint_with`).
- Default flips: actions hidden on idle rows → revealed rows only.
  Visible-affordance rule (plan 017 law): rows with actions show a faint
  `…` marker cell when unrevealed so discoverability survives.
- Sort: neutral `⇅` faint on sortable-unsorted headers, header hover paint
  (uses Step 1's hovered_column), delete `data_table.rs:1409` `|| true`
  (ColumnSpec::sortable becomes real — behavior fix, migration note).

**Verify**: list story: idle rows show `…` marker, hovered/selected row
shows actions; sortable-unsorted header shows `⇅`; non-sortable column
click emits no SortSpec (test).

### Step 4: State completeness sweep

TextArea read-only (dim body + gutter, keep fg legible — read-only ≠
disabled: normal fg, no caret; document the distinction in the law doc's
state vocabulary); InputOtp disabled → `TextDisabled`; Collapsible Section
focus arm distinct (accent gutter `▎` + TextStrong — inherits to
Accordion); tabs: focused+selected cue independent of strip height;
tag_chip match reorder (status composes with selected/focused via patch);
ActionBar `busy` per action (dialog submit → actions dim + spinner glyph on
the confirming action; consumes `DialogState.loading`); AccentRail wired at
permission + plan_review (tick+active) + `animation_demand()`; scroll
thumb: register as hit region with hover + drag-to-scroll (it looks
draggable; make it real — else strip it to a 1-cell indicator; pick
draggable, it's a hit-region + offset math); disabled controls skip the
focus cycle (quality-standard rule — wire in the focus graph, verify with a
3-control cycle test).

**Verify**: per-widget state tests; matrix spot-render: read-only TextArea
≠ disabled ≠ editable (three distinct buffers).

### Step 5: Lookbook proves it + the gate

- `PointerTarget::hover` becomes required (each interactor implements or
  explicitly opts out with a comment).
- Generalize the `button_recipes` all-states story shape: per-family
  "state matrix" story (idle/hover/pressed/focus/selected/disabled/loading/
  invalid side by side) for Button, Checkbox family, TextInput, List row,
  Tabs, Chip, ActionBar.
- `design_gate.rs::state_matrix_distinct`: for each matrix story, assert
  the supported state cells render pairwise-distinct buffers (the S1
  evidence gate).

**Verify**: gate green; corrupting one hover branch back to idle fails it.

### Step 6: Toast behavior + migration

Toast TTL 4s default + dismiss on `esc`/`×` + pause-when-unfocused IF the
input layer exposes terminal focus events (mode 1004) — check
`input/event.rs` for FocusGained/Lost; if absent, wire crossterm's focus
events through (small input addition) or record the gap and ship TTL +
dismiss only. Migration file (next free) + `MIGRATING.md`: hover/reveal
default changes, Space semantics, pressed token, sortable fix, toast
behavior.

## Done criteria

- [ ] `mise run gate` exits 0; `state_matrix_distinct` green.
- [ ] Coverage matrix deltas: zero `hovered: false` recipe literals; zero
      write-only hover fields (each consumed or removed); pressed paints on
      Button/IconButton/Switch/sliders/dividers; disabled paints on
      TextArea(ro)/InputOtp.
- [ ] Space activates a Button in a plain-terminal test.
- [ ] Actions hidden-until-revealed with `…` affordance; sort `⇅` + `|| true` gone.
- [ ] Copy flash on the 9 sites; semantic `pressed` truthful.
- [ ] Migration + `MIGRATING.md`; README row updated.

## STOP conditions

- Keyboard-enhancement detection isn't knowable from the session layer —
  report the crossterm surface available; do NOT ship release-only Space.
- Hover-reveal breaks a pattern's fixed row width math twice — report.
- Focus-cycle skip for disabled conflicts with the focus graph's contract —
  report (kernel change is its own plan).
- Scroll-thumb drag requires hit-region plumbing the widget lacks — fall
  back to documented non-interactive indicator + note.

## Maintenance notes

- New interactive widgets must ship a state-matrix story + PointerTarget
  impl — the gate enforces distinctness.
- Flash timings are placeholder-static until plan 014; revisit durations
  there (§5 table).
