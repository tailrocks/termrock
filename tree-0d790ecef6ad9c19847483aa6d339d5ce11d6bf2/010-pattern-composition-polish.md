# Plan 010: Patterns compose, never hand-roll — the four priority composites plus the outliers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 004-009 DONE in `plans/README.md`.
> Re-locate every site with `rg` before editing.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/007, plans/008, plans/009
- **Category**: design
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

Patterns are the recipes users copy — the goal's four priority composites
(Setup wizard, Settings screen, Metrics dashboard, Auth entry) must be the
proof that the widget layer is sufficient. Today: zero hardcoded colors
(good — audit verified `rg 'Style::new\(\)|Color::Rgb'` over `patterns/`
returns nothing), but metrics_dashboard hand-draws tile chrome and never
touches the ChartSeries roles; auth_entry hand-stacks fields instead of
composing `Form`, shows nothing during a pending submit, and prints errors
twice without glyphs; setup_wizard's cancel confirm is a reversed warning
slab that names no scope; four patterns paint full-row REVERSED selection
slabs; the `Hint*` roles are used by zero patterns; the flagship
`agent_workbench`/`app_shell` (0 raw paint calls) prove the right shape.

## Current state (leads verified by audit at `605217aa`)

Priority four:
- `metrics_dashboard.rs`: `:1358-1375` manual top-edge `─` loop (no Panel);
  `:1471,1485,1501` Sparkline/Gauge use `tile.health.role()` (healthy = mint
  everywhere; ChartSeries* unused — `rg "ChartSeries" patterns/` → 0);
  `:1429-1433` deltas Success/Danger; `:1188-1199,1240-1244,1326-1337` focus
  recolors whole lines `Role::Focus`, overwriting severity; `:1267-1281`
  flat footer hints; `:1301-1305` `›` markers.
- `auth_entry.rs`: `:787-854` manual y-cursor field stack (Form unused;
  settings_screen `:758-763` + setup_wizard `:645-651` compose `Form`);
  `:341-344,475,505,705-895` `pending` blocks input but paints NOTHING;
  `:743-753` host error no glyph; `:757-769` first field error duplicated
  (summary + inline) and leaks `field.id()`; `:856-873` flat hints.
- `setup_wizard.rs`: `:710-730` cancel strip = full-width
  `Warning + REVERSED`, no scope text, Dialog/AlertDialog unused;
  `:733-794` capability list single-role rows (`Danger` or `Text`), OK glyph
  never Success, dead identical `ascii` ternaries `:743-747,:806`;
  `:796-841` summary flat `Text` (KeyValueList unused); `:698`
  `let _ = state.colorless;` (flag advertised, ignored);
  `:266` `wizard.set_focused(true)` permanent.
- `settings_screen.rs`: `:718-741` banner = full-width Warning string
  (Callout unused); `:797-817` two footer idioms in one fn (StatusBar vs
  hand-built string); `:842` `let _ = state.colorless;`.
- ThemePicker double-border: `theme_picker.rs` fixed in plan 006 (builder);
  wire `settings_screen.rs:768-776` + `setup_wizard.rs:671-679` to pass
  focus so only one bright border per scene.

Other patterns:
- Full-row REVERSED/accent selection slabs: `connection_manager.rs:1998-2005`,
  `git_workbench.rs:1396-1398`, `activity_shelf.rs:1082-1092`,
  `session_picker.rs:1417-1418` → gutter + tint + `TextStrong` (colorless
  keeps REVERSED).
- Whole-row status paint: `connection_manager.rs:1986-2005`,
  `session_picker.rs:1414-1426`, `prompt_queue.rs:853`,
  `background_task_panel.rs:1260`, `integration_status.rs:1141`,
  `working_state_card.rs:802,873`, `subagent_card.rs:912` → the
  `process_table.rs:1332-1365` idiom (status letter cell only).
- Selection markers: `rg '"›"|">"' patterns/` sites →
  `glyphs.selection_gutter()` (`▌` is currently used in patterns ONLY as a
  text caret: `connection_manager.rs:2131`, `prompt_queue.rs:789`,
  `plan_review.rs:1663`, `session_picker.rs:1230-1231` — leave carets,
  switch them to the caret glyph chosen in plan 006 Step 4).
- Hints: `rg "Role::Hint" patterns/` → 0; flat footers listed above plus
  `app_dashboard.rs:463-473`.
- `app_dashboard.rs:410-413` hardcodes `.ascii(true)`; `:428-436` KPI
  placeholder string `"host: charts / KPIs"` (embed `MetricsDashboard` like
  `observability_dashboard.rs:1013-1017`).
- `sidebar.rs:1386` OR-sticky nav focus (`state.nav.focused = self.focused
  || state.nav.focused;`) + builder default `true` (`:1325-1338`) — callers
  can never clear it.
- `agent_workbench.rs:1074` `let _ = system.style(Role::BorderFocused);`
  dead statement.
- Raw-paint outliers to sweep with the same rules: `plan_review.rs` (22 raw
  paint calls), `integration_status.rs` (15), `connection_manager.rs` (13),
  `session_picker.rs` (12), `query_editor.rs` (10).

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `crates/termrock/src/patterns/*` listed above;
`crates/termrock/src/widgets/sidebar.rs` (the OR-sticky line + default);
`crates/termrock/src/widgets/theme_picker.rs` ONLY if plan 006's `focused`
builder needs a signature touch-up; `design_gate.rs` (pattern render
checks); `migrations/0293-*.md` + `MIGRATING.md`.

**Out of scope**: new pattern features; widget API changes beyond the
sidebar line; lookbook (plan 011).

## Git workflow

`main`; commit per pattern-cluster; `git commit -s`.

## Steps

### Step 1: Metrics dashboard

Tiles = `Panel` (borderless variant if the 2-cell cost breaks the grid —
then keep a `Section`-style header rule via `glyphs.rule()`, but through a
widget, not a manual loop); series = `ChartSeries1..4`; `health.role()`
confined to the status letter + threshold marker; deltas muted unless bad;
focus = gutter + `TextStrong` (severity preserved), container border carries
zone focus; footer via StatusBar/HintBar.

**Verify**: `rg -n "cell_mut|set_stringn" patterns/metrics_dashboard.rs | wc -l` drops to ≤3; `rg "ChartSeries" patterns/metrics_dashboard.rs` ≥ 1; healthy 12-tile render passes the accent-budget spirit (spot count in test).

### Step 2: Auth entry

Compose `Form` + `Fieldset` (identity/password/confirm/terms; keep
`sync_focus` ids); pending state: panel title "Signing in…" + spinner glyph,
fields dimmed, identity told `set_pending`; errors: glyph via the plan-008
`paint_field_message`; drop the duplicate summary (inline Validation only;
summary reserved for `host_error` with `✗`); hints via HintBar.

**Verify**: pending render differs from idle (test); `rg -n "saturating_add\(field_h" patterns/auth_entry.rs` → 0.

### Step 3: Setup wizard + settings screen

- Cancel confirm → `AlertDialog` naming scope ("Discard setup — N of M steps
  completed"); Warning on glyph/title, no REVERSED strip.
- Capability list + summary → `KeyValueList` (labels strong, values muted,
  problem rows glyph + Danger on glyph); delete dead `ascii` ternaries;
  implement `colorless` (or remove it from state — implement, per plan 008
  Step 1 system seeding).
- Settings banner → `Callout`; one footer idiom (StatusBar always; hand-built
  string deleted); ThemePicker focus threading in both patterns
  (one bright border per scene — verify the Theme step shows exactly one).
- `wizard.set_focused(true)` becomes scene-driven if the pattern exposes
  focus regions; otherwise leave and note.

**Verify**: Theme-step render: exactly 1 `BorderFocused`-styled border (test counts); `rg -n 'if ascii \{ "' patterns/setup_wizard.rs` → 0.

### Step 4: Collection patterns

Selection slabs → gutter+tint+strong (colorless keeps REVERSED); whole-row
status → status-letter idiom; markers → `selection_gutter()`; text carets →
plan-006 caret glyph. Apply to connection_manager, git_workbench,
activity_shelf, session_picker, prompt_queue, background_task_panel,
integration_status, working_state_card, subagent_card, plan_review,
query_editor, task_rail (verify already-correct sites, don't churn them).

**Verify**: `rg -n "REVERSED" patterns/ | rg -v colorless` — remaining hits reviewed (mono-only); pattern tests green.

### Step 5: Odds and ends

`app_dashboard`: thread `state.ascii`; optional `&[MetricTile]` → embedded
`MetricsDashboard`. `sidebar.rs:1386` assign instead of OR; builder default
`false`; update `settings_screen`/`app_dashboard` to pass
`.focused(region == Nav)`. Delete `agent_workbench.rs:1074` dead statement.

**Verify**: `rg -n "\|\| state\.nav\.focused" crates/termrock/src/widgets/sidebar.rs` → 0; sidebar consumers pass focus explicitly (compile forces it if the builder signature changes).

### Step 6: Gate + migration

- `design_gate.rs::patterns_compose()` — source scan: `patterns/` contains
  no `Role::Selection`, no `.add_modifier(Modifier::REVERSED)` outside
  colorless branches (regex heuristic ok), no `Role::Hint`-bypassing footer
  strings in the four priority patterns (assert `HintKey` usage ≥ 1 in each).
- `migrations/0294-*.md` (next free) + `MIGRATING.md`: pattern-visible
  changes (auth entry pending chrome, wizard cancel dialog, settings footer,
  metrics tile chrome, sidebar focus builder default).
- `mise run gate` → exit 0.

## Test plan

Per-step verifies; pattern snapshot updates; the new gate checks.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] Four priority patterns each: composes (no manual field stacks/tile
      loops), ≤2 accent regions and exactly ≤1 focused border per frame
      (tests), HintBar-rendered hints, glyph-carried status.
- [ ] `patterns_compose` gate green.
- [ ] Migration + `MIGRATING.md`; `plans/README.md` updated.

## STOP conditions

- Panel adoption in metrics tiles breaks the 2-4 column grid math and the
  borderless fallback also fails — report with the layout numbers.
- Form composition in auth_entry can't express the secrets flow without
  widening secret exposure — STOP (security-sensitive; report exactly what
  the Form API lacks).
- Sidebar builder default flip blanks focus cues in a consumer you can't
  see rendered — list consumers, report.

## Maintenance notes

- These four patterns are the catalog's hero recipes; plan 011 re-renders
  them as flagship stories.
- `process_table.rs` stays the reference row idiom; point new patterns at it.
