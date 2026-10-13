# Plan 015: Design-law v2 residuals — one chip recipe, tabs cue model, focus vocabulary, breathing rows

> **Executor instructions**: Follow this plan step by step. Read
> `docs/design/web-premium-tui-law.md` (§3-§5) and
> `docs/design/termrock-component-audit-2026-08.md` (§1 F2/F4/F6, §3
> decisions) before starting. Run every verification command. STOP
> conditions binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: plans 002-008 DONE. Re-locate cited sites
> with `rg` before editing.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/005, plans/006, plans/008
- **Category**: design
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

The 2026-08-14 PM SoTs (`web-premium-tui-law.md`,
`termrock-component-audit-2026-08.md`) add contracts the 001-013 suite
doesn't yet own: ONE chip recipe across Tag/Chip/token/kbd (today Tag and
Chip are near-duplicate paint bodies, `tag_chip.rs:293` vs `:768`, and kbd
has two divergent keycap renderings, `kbd.rs:535/557` vs `:779`); a named
`FocusEmphasis` vocabulary instead of per-widget cue improvisation; the
tabs active-cue variant model (rule row already retired in plan 002); the
four-level ladder's "max ONE BOLD run per row" and breathing-row laws; and
the shape-before-color glyph ladder `○→◎→●` (the `◎` preview pip is defined
in RadioGroup but never painted, `controls.rs:1037-1057`).

## Scope

**In scope**: `style/tokens.rs` (FocusEmphasis, TokenRecipe, list breathing),
`widgets/{tag_chip,kbd,tabs,controls,list,input_otp,token_field}.rs`,
consumers of chip paint (attachment_chips, search filter chips via plan 007
helper), `design_gate.rs`, `migrations/` + `MIGRATING.md`.

**Out of scope**: patterns (plan 016), motion (plan 014), decisions D5-D23
not listed below (recorded in README as open until the operator rules —
each plan that hits one applies the audit's recommendation and records it).

## Steps

### Step 1: `FocusEmphasis` vocabulary (audit F2)

`FocusEmphasis { BrightBorder, SelectionFill, FocusTint, Reversed, BoldKey,
PillGlyph }` on `DesignSystem` with per-family defaults; recipes resolve
their focus cue through it (inputs=BrightBorder, rows=FocusTint+gutter,
cells=Reversed, chips=PillGlyph/bracket). This names what plans 005-008
already implemented — wire the existing cue code through the enum so themes
can override per family.

**Verify**: `rg -n "FocusEmphasis" crates/termrock/src | wc -l` ≥ 6 files;
`mise run check` green.

### Step 2: `TokenRecipe` — one chip family (audit F4; law P13)

`TokenRecipe { bracket: BracketStyle{Angle,Square}, mark: Option<Glyph>,
label, removable, status, state }`:
- Tag = angle `⟨ label × ⟩` (neutral; ASCII `< >`),
- Chip = square `[● label ×]` (interactive; shape-ladder mark `○→●→◉`),
- kbd = `[ C-s ]` space-padded, brackets `Border` faint, label
  `HintKey`+BOLD, chord separator = space inside the chip (keep `+` in
  prose), focused keycap = REVERSED,
- TokenField tokens + attachment chips + ThemePicker `[~ tc]` chip consume
  the same recipe.
Collapse the duplicate Tag/Chip paint bodies into one; kbd's two renderings
into one. Remove-region = invert (Danger fg + Surface bg + BOLD; REVERSED on
ANSI-16) — plan 005 already deleted the underline there. Selection ramp
glyph>weight>fill>color; audit D18: bracket carries focus, mark stays `●`.

**Verify**: `rg -n "fn paint" crates/termrock/src/widgets/tag_chip.rs` shows
one shared body; kbd single renderer; chip gate test: Tag/Chip/kbd/token
renders share bracket faintness + label roles.

### Step 3: Tabs active-cue model (audit D2)

`TabsActiveCue { AccentPill (default), Connected, Marker, Rule }`:
- AccentPill = `SelectionTint` bg + `TextStrong` + BOLD on the active label
  (plan 002's interim cue becomes the named default),
- Connected = active tab on `Surface` fill with open bottom edge (app-shell
  variant per `tui-design-research-2026-08.md` §5.3),
- Marker = `▸` + bold (the design-language fallback),
- Rule = opt-in bottom accent rule row (off by default).
Roving-not-selected = leading-edge `BorderFocused` brightening (no REVERSED
block); hovered = `HoverTint`. Update `docs/design/termrock-design-language.md`
§5.2 to name AccentPill as default (one-line errata, same commit).

**Verify**: tabs tests cover all four cues; default render shows pill; zero
`UNDERLINED`/rule-row remnants.

### Step 4: Ladder discipline — one BOLD run per row + breathing rows (law P4/P1; audit F6)

- `resolve_list_row`: assert-in-test that recipe styles yield ≤1 BOLD part
  per row (label may be bold; secondary/trailing never).
- Breathing rows: 1 blank Canvas spacer row between list sections under
  Comfortable density, driven by `Density→SpacingScale` (audit D8:
  default-on Comfortable; migration documents it), off in Compact/Dashboard.
- Trailing-meta slot: verify plan 012's `TextFaint` trailing column is
  independent of badge/status (audit F6 notes `list.rs:1234` made them
  mutually exclusive — fix if plan 012 didn't).

**Verify**: list story under Comfortable shows spacer bands; bold-budget
test green.

### Step 5: Shape-before-color ladder + `◎` preview pip

RadioGroup paints the full `○→◎(preview)→●(committed)→◉(focused+on)` ladder
(the `◎` preview state exists in code but is never painted); Checkbox/
Switch/Select adopt the same preview-pip semantics where a roving cursor
previews a choice before commit. Two-bg-cue: SelectionTint (selected) +
HoverTint (cursor) may coexist.

**Verify**: radio group render at cursor-on-unselected shows `◎`; controls
tests updated.

### Step 6: Gate + migration

`design_gate.rs`: `one_chip_recipe` (source scan: no second bracket-paint
body), `bold_budget_per_row`. Migration file + `MIGRATING.md` (chip API,
tabs cue enum, breathing default, FocusEmphasis).

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] One chip paint body; one keycap renderer.
- [ ] `TabsActiveCue` shipped, AccentPill default, docs errata in same commit.
- [ ] Breathing rows on Comfortable; bold-budget test green.
- [ ] Migration + `MIGRATING.md`; README row updated.

## STOP conditions

- TokenRecipe cannot express an existing chip consumer without losing a
  state — report the state matrix.
- Breathing rows break virtualization row-math (`virtual_list` offsets) —
  report; do not fake it with padding.
- `◎` preview semantics conflict with an existing outcome contract — report.

## Maintenance notes

- Open decisions applied here by audit recommendation: D2 (AccentPill),
  D8 (breathing default-on Comfortable), D14 (`◇` now-edge / Info `·`,
  landed via plan 003), D18 (bracket carries chip focus). The operator can
  overturn any by editing this plan before execution; each is one isolated
  step.
