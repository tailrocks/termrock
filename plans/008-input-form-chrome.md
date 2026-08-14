# Plan 008: Inputs get real chrome — sunken wells, one focus cue, recipe adoption, honest states

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 002/004/005 DONE in `plans/README.md`.
> Re-locate every site with `rg` before editing.

## Status

- **Priority**: P1 (named-48 priority set)
- **Effort**: L
- **Risk**: MED (field geometry changes ripple through 6 delegating widgets)
- **Depends on**: plans/004 (input_recipe fixed), plans/005 (label underlines gone)
- **Category**: design / tech-debt
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

After plan 005 removes the label underline, a focused TextInput is
pixel-identical to an unfocused one except the 1-cell caret — the underline
was (wrongly) carrying the entire focus signal, on the wrong element.
`InputRecipe` has zero widget consumers; `Role::Sunken` has zero input
consumers; errors are color-only with inconsistent row placement; disabled is
color-only; required is unimplementable; six widgets keep private
`colorless`/`ascii` flags that ignore the `DesignSystem` capability. The
binding form grammar (design-language §5.3): field = Sunken well + quiet
border; focus = `BorderFocused` on the ACTIVE field's box (+ optional `›`
prompt); cursor = block/reverse; invalid = `InputInvalid` + `!` glyph.

## Current state (anchors verified first-hand at `605217aa`)

- `style/tokens.rs:732-759` — `input_recipe(state, invalid)` exists; plan 004
  re-pointed `fill` at `Role::Sunken`, hover at Border+HoverTint. Zero widget
  callers (`rg -n "input_recipe" crates/termrock/src/widgets/` → 0).
- `widgets/text_input.rs:1076-1147` — field fills `Role::Input` identically
  focused/unfocused; caret = 1 reversed cell; no border/marker anywhere.
- `widgets/select.rs:902-912` — focused trigger swaps to `Role::Focus`
  (fg-only) and thereby LOSES its well bg.
- TextInput is the substrate for `number_input.rs:1133`, `search_input.rs:908`,
  `token_field.rs:1101`, `password_input.rs:785`, `input_group.rs:267`,
  `path_input.rs`, `combobox.rs` — chrome changes land once, ripple seven times.
- Errors color-only + row drift: `text_input.rs:1180-1192` (y+2),
  `number_input.rs:1142-1167`, `token_field.rs:1107-1117`,
  `search_input.rs:914-926`, `select.rs:863-874` (bottom-1),
  `controls.rs:542-551`.
- Disabled color-only: `GlyphSet::disabled_mark()` (`style/tokens.rs:122-126`)
  consumed only by `badge.rs:404`, `label.rs:345`. Tabs disabled =
  `TextMuted` (`tabs.rs:1180-1182`) — same tone as inactive.
- Required: `rg -n "required" widgets/{text_input,text_area,password_input,number_input,search_input,token_field,select,controls}.rs`
  → one unrelated comment (`controls.rs:2024`).
- Capability flags: good pattern `controls.rs:403-409` / `slider.rs:187-191`
  (`mono() = colorless || glyphs.is_ascii() || capability == Monochrome`);
  raw-flag widgets: `text_area.rs:1446`, `picker.rs:415`, `quick_open.rs:1335`,
  `stepper.rs:807`, `toolbar.rs:413`, `menu_bar.rs:1318`; `ascii: bool`
  never seeded from `system.glyphs.is_ascii()` in `tabs.rs:812`,
  `spinner.rs:387,401,606`, `status_indicator.rs:218`, and ~28 sites in
  `menu_bar.rs`. TextInput/Select/SearchInput/NumberInput/TokenField have no
  mono branch at all.
- Slider family: `slider.rs:786-789` vs `:1364-1370` — Slider and RangeSlider
  disagree on thumb focus chrome; both flood the track `Role::Accent` when
  focused (`:766-772`, `:1378-1383`).
- Stepper: `stepper.rs:869-874` current step = `Role::Focus + BOLD|REVERSED`
  slab (underline cursor removed by plan 005).
- TextArea soft-wrap selection: `text_area.rs:1713-1727` full-visual-row
  `Role::Focus + REVERSED`; hard-wrap path (`:1786-1808`) does real columns.
- Form rows: `form.rs:318-323` reserves 3-4 rows; paints label row + help at
  `content_y+2`; `FieldRow` renders label+value on ONE line
  (`field_row.rs:174-176`), so `Stacked` == `Inline` + blank rows; hit-region
  `FormFieldRegion.value` registered on the blank row (`form.rs:915-921`);
  labels use `recipe.secondary` (muted) while values use `recipe.label` —
  inverted vs form grammar.
- Date picker: `date_time_picker.rs:2069-2136` — `num.trim()` breaks the
  4-col grid; five bracket decorations; header hints inline in the title;
  green `REVERSED` cursor blocks; day/time lists bypass recipes.
- OTP: `input_otp.rs:385-418` — no well fill; focused slot triple-cue
  (plan 005 reduces to REVERSED; well added here).
- Password: `password_input.rs:793-830` — strength line always `TextMuted`
  (no glyph/meter); reveal toggle hardcodes `"o"/"*"` + `Role::Focus`.
- Dead/no-op: `picker.rs:462-466` identical ternary arms;
  `input_group.rs` addon accent handled by plan 007.

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `text_input.rs`, `text_area.rs`, `password_input.rs`,
`number_input.rs`, `search_input.rs`, `token_field.rs`, `path_input.rs`,
`select.rs`, `combobox.rs`, `multi_select.rs`, `input_group.rs`,
`input_otp.rs`, `controls.rs`, `slider.rs`, `stepper.rs`, `picker.rs`,
`quick_open.rs`, `toolbar.rs`, `menu_bar.rs`, `tabs.rs`, `spinner.rs`,
`status_indicator.rs`, `form.rs`, `field_row.rs`, `date_time_picker.rs`;
`style/tokens.rs` (mono() promotion + InputRecipe extension only);
`design_gate.rs`; `migrations/0290-*.md` + `MIGRATING.md`.

**Out of scope**: overlay popups of select/combobox (plan 006/009 already
unify rows), status colors (plan 007), patterns (plan 010).

## Git workflow

`main`; commits per step; `git commit -s`.

## Steps

### Step 1: Promote `mono()` and seed `ascii` from the system

Add `DesignSystem::mono(&self) -> bool` (capability == Monochrome ||
glyphs.is_ascii()) and `DesignSystem::ascii_glyphs()`. Every widget-local
`colorless`/`ascii` flag becomes a force-override defaulting to the system
value (`self.colorless || system.mono()`); fix the raw-flag sites listed
above; delete `picker.rs:462-466` dead ternary.

**Verify**: `rg -n "self\.colorless(?!\s*\|\|)" -P crates/termrock/src/widgets/` — remaining hits reviewed as force-overrides; `mise run check` green.

### Step 2: TextInput adopts `input_recipe` — the one field-chrome authority

`TextInput::paint` resolves `input_recipe(state, invalid)`:
- well = `fill` (Sunken) painted every state; focused adds a field-local
  cue: leading `›` prompt cell in `BorderFocused` color (1 reserved col)
  when the field is height-1/borderless, or `BorderFocused` box when the
  widget owns a border. Extend `InputRecipe` with `prompt: (&'static str, Style)`
  if needed.
- caret stays reversed cell (`cursor` style).
- placeholder = recipe.placeholder (`TextFaint` tone after plan 002).
Update the six delegating widgets' expectations (`number/search/token/
password/input_group/path`); `select.rs:902-912` trigger patches Focus onto
the well instead of replacing it; `combobox` inherits.

**Verify**: focused vs unfocused TextInput render differs by prompt cell +
border color in a new unit test; all input-family tests green.

### Step 3: Shared field messages — error/help/required/disabled

- One `paint_field_message(system, kind, msg, rect)` helper: error =
  `Glyph::Error` + Danger; warning = glyph + Warning; help = `TextFaint`.
  Fix the six error sites onto one row-offset policy (directly under the
  field).
- `required(bool)` builder on TextInput family + Select + controls; paints
  `*` in Danger after the label; expose in `SemanticState`
  (`text_input.rs:1250-1262`).
- Disabled: append `glyphs.disabled_mark()` in shared label paint; tabs
  disabled = `TextDisabled`.

**Verify**: mono render of an invalid field shows `!`; required shows `*`;
disabled shows mark — one test each.

### Step 4: Controls/slider/stepper chrome parity

- Radio/checkbox/switch focus = bright outline on the mark + bold label
  (already partly there post-005); hover = HoverTint.
- One `slider_chrome(state)` for Slider+RangeSlider: thumb = `█`/`◉` Accent
  glyph; track fill = `ChartSeries1`-family, NOT Accent; focus = bright
  border on the track box + bold value; identical for both widgets.
- Stepper current step = `●` glyph accent + `TextStrong` bold label (kill
  the `Focus+REVERSED` slab); roving cursor = gutter/bracket.

**Verify**: slider vs range_slider focused renders use identical cue set
(test comparing styles); stepper snapshot updated.

### Step 5: TextArea + OTP + Password + Picker polish

- `text_area.rs:1713-1727`: per-visual-row column math (reuse hard-wrap
  helper); selection = `SelectionTint` bg, not `Focus+REVERSED` full rows.
- `input_otp.rs`: slot rects get `Role::Sunken` fill; focused slot =
  REVERSED cell.
- `password_input.rs`: strength → `SemanticStatus`-mapped glyph + block-ramp
  meter (`▁▃▅`); reveal toggle via glyph catalog, `TextMuted` until targeted.
- Date picker (`date_time_picker.rs`): drop `.trim()` (fixed 4-col cells);
  two decorations only (`[..]` selected, `·..·` other-month); cursor =
  recipe gutter/tint (no green REVERSED); `[< > Pg]` moves to footer hints;
  day/time lists through `resolve_list_row`.

**Verify**: calendar renders aligned columns (test asserts every cell width
4); textarea selection covers exact columns; otp/password snapshots updated.

### Step 6: Form rows real

`form.rs` + `field_row.rs`: `Stacked`/`Responsive` = label row 0
(`TextStrong`), value row 1 in a Sunken well; `Inline`/`Compact` = one line,
height 2 reservation fixed to what paints; `FormFieldRegion.value` rect =
the row that carries the value; label/value tones per grammar (label strong,
value body, help faint). `FieldRow` gains a two-line mode consumed by Form.

**Verify**: form story renders with no blank filler rows; hit-region test
clicks the value row and hits the field.

### Step 7: Gate + migration

- `design_gate.rs::inputs_share_field_chrome()` — render TextInput,
  NumberInput, SearchInput, TokenField, PasswordInput, Select focused;
  assert all six paint the same well bg role and the same focused cue cell.
- `migrations/0290-*.md` (next free) + `MIGRATING.md`: field chrome change,
  form row-height semantics, date-picker decoration change, required/
  disabled additions, mono()/ascii seeding behavior change.
- `mise run gate` → exit 0.

## Test plan

Per-step tests above; churned snapshots re-blessed; helper unit tests.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "input_recipe" crates/termrock/src/widgets/text_input.rs` ≥ 1; six delegators inherit (no private well paint left: `rg -n "Role::Input" crates/termrock/src/widgets/` only via recipe consumption).
- [ ] Focused/unfocused field renders differ (gate test).
- [ ] `inputs_share_field_chrome` green.
- [ ] Migration + `MIGRATING.md`; `plans/README.md` updated.

## STOP conditions

- Reserving the `›` prompt column breaks caret-column math in a delegating
  widget's tests twice — report the offset chain (`TextInputParts.field`).
- `FieldRow` two-line mode conflicts with consumers outside Form (patterns
  pinned to one-line) — report the consumer list.
- Date-picker grid change breaks its hit-testing — report before reworking
  hit rects.

## Maintenance notes

- Patterns (plan 010) must re-render their forms after this; auth_entry
  composition lands there.
- The `InputRecipe.prompt` extension is the seam for future per-theme
  prompt glyphs.
