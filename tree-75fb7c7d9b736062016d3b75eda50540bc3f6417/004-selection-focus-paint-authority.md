# Plan 004: One paint authority for selection, focus, and elevation — recipes become mandatory

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 605217aa..HEAD -- crates/termrock/src/style/tokens.rs crates/termrock/src/widgets/surface.rs crates/termrock/src/widgets/panel.rs crates/termrock/src/widgets/dialog.rs`
> On drift, compare "Current state" excerpts to live code; mismatch = STOP.

## Status

- **Priority**: P0
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/002-role-palette-foundation.md
- **Category**: tech-debt / design-foundation
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

The token layer has the right ideas (`SelectionChrome`, `resolve_list_row`,
surface/panel/button/input recipes, elevation), but widgets can — and ~28 do —
bypass them: reading `Role::Selection` raw (neon slab), overriding the theme's
`SelectionChrome` locally (10 widgets force `Tint`), or re-deriving focus
styles ad hoc. Meanwhile the recipes themselves contain policy bugs
(`SelectionChrome` defaults to `Fill`; hover resolves to the same style as
focus; primary buttons are neon in every state; overlays built on `Panel` fill
with `Surface` instead of `Elevated`; `Role::Backdrop` has zero production
paint sites). This plan fixes the authority layer and adds the enforcement
seams; plans 005–010 then migrate widgets onto it.

## Current state (verified excerpts at `605217aa`)

- `crates/termrock/src/style/tokens.rs:129-140` — `SelectionChrome` derives
  `Default` with `#[default] Fill`.
- `tokens.rs:434-435` — `DesignSystem::phosphor()` doc says "(quiet gutter
  selection)" but sets `.selection(SelectionChrome::Tint)`; `:459` paper =
  `Fill`; `:474` high_contrast = `Fill`; `:498` `DesignSystem::new()` uses
  `SelectionChrome::default()`.
- `tokens.rs:776-831` — `resolve_list_row`: on `Tint`, label =
  `TextStrong.patch(SelectionTint)`; `gutter` =
  `Some((glyphs.selection_gutter(), style(Accent)))` whenever selected;
  `show_focus_underline: state.focused && state.selected && !disabled`
  (`:816`); `hover: self.style(Role::LinkHover)` (`:818`).
- `tokens.rs:678-724` — `button_recipe`: `Primary` → fill AND label
  `Role::ActionFocused` (neon chip in every state);
  `Destructive` → `fill = style(Role::Danger)` (fg-only role: no bg fill);
  `ControlState::Hovered => label = self.style(Role::LinkHover)` (hyperlink
  color as button hover); `Loading` folded into `Disabled`.
- `tokens.rs:732-759` — `input_recipe`: `Hovered => border = style(Role::Focus)`
  vs `Focused => border = style(Role::BorderFocused)` (same green pre-plan-002,
  confusable after); `value`/`fill` both from `Role::Input`; `InputRecipe.border`
  field doc at `tokens.rs:349` says "Border / underline style".
- `tokens.rs:640-655` — `panel_recipe`: `surface: self.style(Role::Surface)`
  hard-coded; emphasis changes border+nothing else; no elevation input.
- `crates/termrock/src/widgets/surface.rs:443-454` — `surface_recipe` map:
  `Focused => (Some(Role::Surface), Some(Role::BorderFocused), true)`;
  `Selected => (Some(Role::Selection), Some(Role::Border), true)` (whole-panel
  neon slab); `Raised`/`Overlay` → `Role::Elevated`.
- `crates/termrock/src/widgets/panel.rs:617-632` — `Panel::surface_recipe()`
  maps `PanelChrome::Focused → SurfaceRecipe::Focused`,
  `Danger → Destructive`, `PanelVariant::Selected → SurfaceRecipe::Selected`.
  So every overlay built from a focused `Panel` (permission, file_picker,
  history_picker, multi_select, form_wizard, question_flow, date_time_picker)
  fills with `Surface`, not `Elevated` — zero elevation contrast.
- `crates/termrock/src/widgets/dialog.rs:526-539` — `Backdrop::reset()`/
  `dim_wash()` hardcode `fg(Color::DarkGray)` + `bg(crate::style::DIALOG_BACKDROP)`
  (a `Color::Reset` const), and `from_tokens` (`:548-552`) also overrides the
  palette Backdrop bg with `DIALOG_BACKDROP`. The only `Backdrop` construction
  outside tests is none — `dialog.rs:2242` is `#[cfg(test)]`. `drawer.rs:140`
  and `popover.rs:81` declare `BackdropPolicy::Dim`; nothing consumes it.
- `dialog.rs:1224-1231` — `resolved_emphasis`: `DialogVariant::Info =>
  PanelChrome::Focused` unconditionally (an unfocused info dialog wears the
  focus border).
- Local `SelectionChrome` overrides (all `self.system.clone().selection(SelectionChrome::Tint)`):
  `select.rs:1060`, `quick_open.rs:1639`, `menu_bar.rs:1601`,
  `dropdown_menu.rs:1256`, `command_palette.rs:1636`, `file_picker.rs:1436`,
  `history_picker.rs:1226`, `multi_select.rs:1124`, `data_table.rs:1446-1449`
  (per painted row!), `tree_table.rs:1244-1252` (per painted row).
- Raw `Role::Selection` consumers in widgets (28 files; the load-bearing ones):
  `completion_menu.rs:1491`, `menu_nav.rs:335`, `progress_steps.rs:769`,
  `notification_center.rs:1288`, `command_palette.rs:1688`,
  `sidebar.rs:1223-1226`, `keyboard_help.rs:1208`, `tag_chip.rs:532`,
  `theme_picker.rs:273-284`, `menu_bar.rs:1406-1409`, `quick_open.rs:1558-1561`,
  `data_table.rs:1529-1532` (cell selection), `markdown.rs:1626-1639`
  (`underline_row` uses Selection fg), `surface.rs:451`.

Repo conventions: recipes live in `style/tokens.rs`; `mise run check` before
commit; migration file for visible default changes; work on `main` with
`git commit -s`.

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Fast gate | `mise run check` | exit 0 |
| Style tests | `cargo nextest run -p termrock style::` | pass |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/style/tokens.rs`
- `crates/termrock/src/widgets/surface.rs`, `panel.rs`, `dialog.rs`,
  `drawer.rs`, `popover.rs` (backdrop consumption + Info-emphasis fix only)
- The 10 files with `.selection(SelectionChrome::Tint)` overrides (delete the
  override ONLY — cosmetic row-paint rework belongs to plans 005/006)
- A new cross-widget paint gate test file `crates/termrock/tests/design_gate.rs`
- `migrations/0283-*.md` + `MIGRATING.md`

**Out of scope**: per-widget underline removal (plan 005), row-anatomy rework
(plan 006/007), pattern files (plan 010), lookbook SVGs (plan 011). Do not
migrate the 28 raw `Role::Selection` sites here EXCEPT `surface.rs:451` —
they're plan 005/006/009 work; the gate test added here is `#[ignore]`d until
those plans land.

## Git workflow

Directly on `main`; one commit;
`git commit -s -m "feat(style)!: selection/focus/elevation paint authority — gutter default, honest recipes, live backdrop"`.

## Steps

### Step 1: SelectionChrome default → Gutter; align presets

- `tokens.rs`: `#[default]` moves from `Fill` to `Gutter`.
- `phosphor()`: `.selection(SelectionChrome::Gutter)` (doc comment already
  says gutter); `paper()` → `Tint`; `high_contrast()` → `Tint`.
- `Fill` stays as explicit opt-in; add doc: "opt-in only; never a default".

**Verify**: `cargo nextest run -p termrock style::` → update the preset tests that pinned Fill/Tint (`tokens.rs:892,976` area); then pass.

### Step 2: Fix `resolve_list_row` cue vocabulary

- Delete `show_focus_underline` from `ListRowRecipe` and its producer
  (`tokens.rs:816,855-857`). Fix the two consumers now so the build stays
  green: `widgets/list.rs:1350-1355` — delete the underline repaint block
  entirely (the gutter + label carry selection; do NOT repaint the label with
  `recipe.focus`); `widgets/tree.rs:1146-1153` — delete the three
  `UNDERLINED` modifications (keep BOLD on focused+selected; hover handled by
  `recipe.hover_wash` in plan 005).
- `hover` field: resolve from `Role::HoverTint`-based wash semantics — change
  `hover: self.style(Role::LinkHover)` to `hover: self.style(Role::TextStrong)`
  and keep `hover_wash` as the bg carrier; document that hover never uses link
  styling.
- Gutter distinction per design-language §5.5: when
  `selected && !state.focused`, gutter style = `Role::TextMuted`; when
  `selected && state.focused`, gutter style = `Role::Accent` (today it is
  always Accent, `tokens.rs:798-802`).

**Verify**: `mise run check` → exit 0; existing list/tree snapshot tests updated in the same step.

### Step 3: Elevation-honest surface + panel recipes

- `surface.rs` map: `Focused => (Some(Role::Surface), BorderFocused, true)`
  stays for in-flow panels, but add `SurfaceRecipe::OverlayFocused =>
  (Some(Role::Elevated), Some(Role::BorderFocused), true)` and
  `SurfaceRecipe::OverlayDanger => (Some(Role::Elevated), Some(Role::Danger), true)`.
- Ladder honesty (audit F1): add `SurfaceRecipe::Sunken => (Some(Role::Sunken),
  None, false)` (input wells / code wells become expressible), and SPLIT
  `Raised` to `Role::Raised` — today `Raised` and `Overlay` both resolve
  `Role::Elevated` (`surface.rs:447-448`), so the 5-step ladder physically
  can't render. Reserve `Elevated` for `Overlay*` only.
- `SurfaceRecipe::Selected` fill changes `Role::Selection` →
  `Role::SelectionTint` (kills the whole-panel neon slab; `panel.rs:1000`'s
  `▌` cue stays).
- `Panel` gains `.overlay(bool)` (default false). `Panel::surface_recipe()`
  maps `overlay && Focused → OverlayFocused`, `overlay && Danger →
  OverlayDanger`, `overlay otherwise → Overlay`.
- `panel_recipe` (`tokens.rs:640-655`) gains an `elevation: Elevation`
  parameter; `surface: self.style(elevation.role())`; danger emphasis sets
  `title_prefix: Some(Glyph::Warning)` on a new `PanelRecipe.title_prefix`
  field (consumed by `Panel::block()` title assembly).
- Flip the seven Panel-based overlays to `.overlay(true)`:
  `permission.rs:1438,1484`, `file_picker.rs:1274`, `history_picker.rs:1043`,
  `multi_select.rs:999`, `form_wizard.rs:970`, `question_flow.rs:1091,1120`,
  `date_time_picker.rs:2028`.

**Verify**: `mise run check`; a quick behavior test: render a focused overlay Panel and assert its fill bg == `Role::Elevated` bg.

### Step 4: Backdrop becomes real

- `dialog.rs`: delete `DIALOG_BACKDROP` overrides — `Backdrop::from_tokens`
  uses `tokens.style(Role::BackdropWash)` (new bg-carrying role from plan 002:
  Canvas blended ~60%; the `░` stipple + `DIM` + Reset-bg approach dies — it
  gave non-black terminals no dim at all); keep `reset()` for hosts that opt
  out.
- Consume `BackdropPolicy::Dim`: in `dialog.rs` (before the `Clear` +
  `Surface` at `:1264-1269`), and in `drawer.rs`/`popover.rs` render paths,
  paint `Backdrop::from_tokens` across the full overlay layer area when the
  policy is `Dim`. The overlay host (scene layer) area = the `area` passed to
  the modal render minus nothing — if only the dialog rect is available, STOP
  and report (the backdrop needs the layer rect; check how
  `OverlayStack`/scene passes area).
- `dialog.rs:1228` — delete `DialogVariant::Info => PanelChrome::Focused`;
  Info resolves to `self.emphasis` like `Default`.

**Verify**: dialog snapshot tests show dimmed cells outside the dialog rect; `mise run check` green.

### Step 5: Button/input recipe honesty

In `tokens.rs`:
- `button_recipe`: `Primary` label = `Role::ActionFocused` ONLY when
  `state == Focused || state == Pressed`; otherwise label = `TextStrong` +
  bold with `fill = Role::ActionFocused`-bg… NO — simpler, per design doc
  "solid accent chip only when default confirm": add
  `ButtonRecipeVariant::PrimaryDefaultConfirm` guidance is overreach; instead:
  keep Primary = chip, but `Hovered => label.patch(HoverTint bg) + BOLD`
  (remove `Role::LinkHover`); `Destructive` gains a real bg: use
  `Role::Danger` fg on `Role::SelectionTint`-style dark red — add palette
  const `DANGER_TINT` bg in plan-002 style if missing; if plan 002 didn't add
  one, use `Role::Danger` fg + `REVERSED` for Pressed only and border Danger
  (no fake fill). `Loading` gets its own arm: keep the variant label role,
  add `Modifier::DIM` (verb/spinner handled by widgets).
- `input_recipe`: `Hovered => border = self.style(Role::Border)` patched with
  `HoverTint` bg (never `Role::Focus`); `fill = self.style(Role::Sunken)`;
  `value = self.style(Role::Text)` (patched `InputInvalid` when invalid);
  fix `InputRecipe.border` doc at `tokens.rs:349` → "Border style." (no
  underline mention).

**Verify**: `cargo nextest run -p termrock style::` — update `input_recipe`/`button_recipe` unit tests (`tokens.rs:985` area) to the new expectations; pass.

### Step 6: Delete local SelectionChrome overrides

In the 10 files listed in Current state: remove `.clone().selection(SelectionChrome::Tint)`
and resolve against the borrowed `&DesignSystem` (also hoists the per-row
`DesignSystem` clone out of `data_table.rs`/`tree_table.rs` hot loops). Do
not otherwise change their row paint here.

**Verify**: `rg -n "\.selection\(SelectionChrome" crates/termrock/src/widgets/` → 0 matches (style/ presets keep theirs); `mise run check` green (snapshot updates expected — these widgets now honor the theme's Gutter default).

### Step 7: Add the design gate (ignored until sweeps land)

New `crates/termrock/tests/design_gate.rs`:
- `#[test] #[ignore = "enable after plans 005-009"] fn no_widget_paints_selection_fill_by_default()`
  — render List, Tree, Table, DataTable, TreeTable, CompletionMenu,
  DropdownMenu, CommandPalette, NotificationCenter, ProgressSteps, MenuNav
  each with one selected row under `DesignSystem::phosphor()`, assert no cell
  bg equals the `Role::Selection` bg.
- `#[test] fn selection_chrome_not_overridden_in_widgets()` — reads source via
  `std::fs` over `crates/termrock/src/widgets/` asserting the string
  `.selection(SelectionChrome` does not appear (enabled immediately — Step 6
  makes it true).

**Verify**: `cargo nextest run -p termrock design_gate` → the source-scan test passes; the render gate is ignored.

### Step 8: Migration + gate

`migrations/0283-*.md` (next free number): SelectionChrome default
Fill→Gutter (+ preset changes), ListRowRecipe field removal
(`show_focus_underline` — public struct break), PanelRecipe signature change
(elevation param + title_prefix), new SurfaceRecipe variants,
SurfaceRecipe::Selected tint change, backdrop now dims by default under
`BackdropPolicy::Dim`, DialogVariant::Info border change, button/input recipe
behavior changes. Consumer edit recipe for each. `MIGRATING.md` row.

**Verify**: `mise run gate` → exit 0.

## Test plan

- Updated recipe unit tests (Steps 1,2,5); new elevation/backdrop assertions
  (Steps 3,4); design-gate file (Step 7). Model tests on existing
  `tokens.rs`/widget test modules.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "show_focus_underline" crates/` → 0 matches.
- [ ] `rg -n "\.selection\(SelectionChrome" crates/termrock/src/widgets/` → 0 matches.
- [ ] `rg -n "DIALOG_BACKDROP" crates/` → 0 matches.
- [ ] `rg -n "Role::LinkHover" crates/termrock/src/style/tokens.rs` → 0 matches.
- [ ] `design_gate.rs` exists; source-scan test green; render gate present (ignored).
- [ ] One migration file + `MIGRATING.md` row.
- [ ] `plans/README.md` updated.

## STOP conditions

- The overlay/scene layer rect is not reachable where backdrops must paint
  (Step 4) — report the actual overlay plumbing instead of painting the
  backdrop over the dialog rect only.
- Removing `show_focus_underline` breaks consumers outside `list.rs`/`tree.rs`
  — report the extra sites (there should be none; `rg` first).
- Plan 002 not landed (no `Role::TextFaint` / palette still all-green): Steps
  1-7 still apply; note in the migration that hue distinctions arrive with
  plan 002.

## Maintenance notes

- Plans 005-009 migrate the 28 raw `Role::Selection` sites and per-widget
  cues onto this authority, then un-`#[ignore]` the render gate — that flip
  is part of plan 009's done criteria.
- Reviewer focus: Step 5's Destructive-fill decision (fg+border vs tinted bg)
  — whichever lands, the migration file must state it.
