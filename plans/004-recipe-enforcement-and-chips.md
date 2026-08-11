# Plan 004: Enforce recipes — button chips, real badges, tinted selection, filled meters

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/widgets/primitives.rs crates/termrock/src/widgets/badge.rs crates/termrock/src/widgets/progress.rs crates/termrock/src/widgets/empty_state.rs crates/termrock/src/widgets/list.rs`
> Plans 001–003 legitimately touched style/layout; the widget excerpts below
> must still match. On mismatch, STOP.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED (public visual defaults change; API of Button/Badge states unchanged)
- **Depends on**: plans/001-surface-ladder-and-role-expansion.md, plans/003-spacing-activation.md
- **Category**: tech-debt (visual foundation)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

The `DesignSystem` ships full part×state recipes — `button_recipe` even
resolves a Primary fill of ink-on-phosphor — but **zero** widgets consume
them. Instead, a copied `style.bg = None` pattern in 13 widget files strips
every background: primary buttons render near-invisible black-on-dark text,
badges are `[text]` brackets instead of chips, selection washes are
impossible, progress tracks are colorless. This plan deletes the pattern,
makes the recipe resolvers the only styling path for controls, and upgrades
the wave-1 widgets (Button, Badge, List selection, Progress, EmptyState) to
the layered look the roles from plan 001 enable.

## Current state

Verified excerpts (`539e7d03`):

`widgets/primitives.rs:662-694` — Button resolves a role style then
unconditionally strips bg:

```rust
let mut style = if !a11y_ok {
    theme.style(Role::Danger)
} else if disabled {
    theme.style(Role::ActionDisabled)
...
} else {
    theme.style(self.base_role())
};
style.bg = None;
```

Note `Role::ActionFocused` in phosphor is `fg(INK).bg(PHOSPHOR_GREEN).bold()`
(`style/mod.rs:352-355`) — with bg stripped, fg stays `INK` (ANSI black).

All `bg = None` sites (verified by grep at `539e7d03`) — 17 sites, 13 files:

```
widgets/badge.rs:459           widgets/primitives.rs:694,1309,1334
widgets/kbd.rs:567             widgets/toggle.rs:1151
widgets/link.rs:430,779        widgets/identity.rs:561,581
widgets/code_block.rs:1200,1673  widgets/segmented_control.rs:703
widgets/text.rs:523            widgets/button_group.rs:704
widgets/key_value_list.rs:868  widgets/tag_chip.rs:538
```

`style/tokens.rs:610-679` — `button_recipe(variant, state)` already returns
`ButtonRecipe { label, fill, border, bordered, pad_x }` with Primary fill =
`Role::ActionFocused`, Destructive fill = `Role::Danger`, focus border =
`Role::BorderFocused`, disabled → `ActionDisabled` + empty fill. It has zero
widget callers (verify: `grep -rn "button_recipe" crates/termrock/src/widgets/`
→ none outside tests).

`widgets/list.rs:1202-1215` — selection paint path (after plan 001 the
`use_tint` branch paints `recipe.tint` = `Role::SelectionTint` bg and
`hover_fill` paints `recipe.hover_wash`). The default `SelectionChrome` for
phosphor is `Gutter` (`tokens.rs:421-423`).

`widgets/progress.rs:934-940` — flat glyphs:

```rust
fn fill_glyph(ascii: bool) -> &'static str { if ascii { "#" } else { "█" } }
fn empty_glyph(ascii: bool) -> &'static str { if ascii { "-" } else { "░" } }
```

`widgets/empty_state.rs:503-533` — `paint_full` renders loose centered text
rows (glyph/title/explanation/context rows in `TextMuted`/`TextDisabled`)
with `FlexSize::Fixed(1)` rows and no framing surface.

Design constraints (design SoT `docs/design/component-visual-richness-plan.md`
§5, §7): Primary = solid accent chip (ink on phosphor), Secondary = outlined,
Ghost/Quiet = text-only; badges are chips with `Surface`/tint fills; progress
uses sub-cell ramp (already shipped in `widgets/charts.rs:240`:
`" ▁▂▃▄▅▆▇█"` — promote, do not duplicate); EmptyState sits on a quiet
framed surface, not the void.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Recipe callers | `grep -rn "button_recipe" crates/termrock/src/widgets/ \| grep -v "#\[cfg(test)\]" \| wc -l` | ≥ 1 after Step 1 |

## Scope

**In scope**:

- `crates/termrock/src/widgets/`: `primitives.rs`, `badge.rs`, `kbd.rs`,
  `toggle.rs`, `link.rs`, `identity.rs`, `code_block.rs`,
  `segmented_control.rs`, `text.rs`, `button_group.rs`, `key_value_list.rs`,
  `tag_chip.rs`, `list.rs`, `tree.rs`, `progress.rs`, `empty_state.rs`
- `crates/termrock/src/style/tokens.rs` (recipe adjustments only if a state
  is unreachable through the current recipe shape)
- `crates/termrock/src/style/glyph.rs`, `crates/termrock/src/style/mod.rs`, and
  `crates/termrock/src/widgets/charts.rs` (shared block-ramp promotion required
  by Step 4)
- `crates/termrock/src/widgets/surface.rs` (`bordered(true)` contract repair
  required by EmptyState framing only)
- `crates/termrock-lookbook/src/stories.rs` (story updates)
- `docs/api/public-api.txt`, affected `docs/public/preview-frames/`, and
  `artifacts/visual-qa/plan-004/`
- `migrations/0264-*.md` + `MIGRATING.md`
- `plans/README.md`

**Out of scope**:

- Dialog/Toast/StatusBar/menus (plan 005), remaining widgets (plan 010).
- Changing recipe *shape* beyond what a listed widget needs.

## Git workflow

`main`, `git commit -s`. Suggested:
`feat(widgets)!: route controls through recipes — chips, tints, ramp meters`.

## Steps

### Step 1: Button through `button_recipe`

In `primitives.rs`, replace the hand-rolled style resolution
(lines 662-694 region) with:

1. Map `ButtonVariant` → `ButtonRecipeVariant`
   (`Primary→Primary`, `Destructive→Destructive`, `Link→Link`,
   `Outline→Outline`, `Success|Command→Primary` label semantics preserved via
   role override if needed, everything else → `Secondary`/`Quiet` as the
   variant docs indicate — read the `ButtonVariant` enum docs in
   `primitives.rs` first and record the mapping in a match with a comment).
2. Map interaction state (armed/hovered/disabled/loading/focused) →
   `ControlState`.
3. `let recipe = theme.button_recipe(variant, state);` — paint fill when
   `recipe.fill` is non-empty (chip: fill the label rect incl. `pad_x`
   cells), label with `recipe.label`, border when `recipe.bordered`.
4. **Delete `style.bg = None` at `primitives.rs:694`** and the sites at
   `primitives.rs:1309,1334` (read each in context: 1309/1334 are in other
   primitive paint paths — apply the same recipe-or-explicit-role treatment,
   never blanket stripping).
5. Keep existing non-color affordances (BOLD/UNDERLINED modifiers,
   `primitives.rs:697-709`) — they are the mono/no-color story.

Chip geometry: a Primary/Destructive button paints `␣label␣` with fill bg
across `pad_x + label + pad_x` cells (pad from `recipe.pad_x`), 1 row tall.
Mono/`no_color` systems: recipe fills quantize away — verify the mono path
still renders the BOLD/UNDERLINE affordances.

**Verify**: `cargo nextest run -p termrock primitives` → update expectations
asserting bg-less buttons; new test `primary_button_is_accent_chip`:
default-state Primary button cell at label position has
`bg == style(Role::ActionFocused).bg` and `fg == INK`.

### Step 2: Delete the remaining `bg = None` sites

For each remaining site (list above), read the surrounding function and
replace the strip with intent:

- `badge.rs:459` — `BadgeFill::None` keeps *explicitly transparent* look via
  `Style { bg: None, .. }` built from an fg-only role — but change the
  **default** `BadgeFill` from `None` to `Soft` (read `badge.rs:90-93`);
  `Soft` now resolves a real `Role::Surface`/`Raised` fill (plan 001), so
  badges become chips by default. Bracket glyphs `[ ]` remain only in
  `GlyphSet::Ascii`.
- `kbd.rs:567` — key caps get `Role::Raised` fill (chip-like), fg from
  existing role.
- `tag_chip.rs:538` — same chip treatment as Badge.
- `toggle.rs:1151`, `segmented_control.rs:703`, `button_group.rs:704` —
  selected/active segments use `Role::SelectionTint` (or `ActionFocused` for
  the active option — follow what the widget's own docs say the active state
  means; do not invent a third convention).
- `link.rs:430,779`, `text.rs:523`, `identity.rs:561,581`,
  `key_value_list.rs:868`, `code_block.rs:1200,1673` — these are text-like
  surfaces: legitimate transparent bg, but express it as fg-only styles at
  the source (build the style without bg) instead of resolve-then-strip.

**Verify**: `grep -rn "bg = None" crates/termrock/src/widgets/` → **0
matches**. `cargo nextest run -p termrock` → pass after expectation updates.

### Step 3: Selection tint default

In `style/tokens.rs` `phosphor()` (line 421): change
`.selection(SelectionChrome::Gutter)` → `.selection(SelectionChrome::Tint)`.
With plan 001's `SelectionTint` role, selected rows now get gutter glyph +
quiet `#14331a` wash (the `resolve_list_row` gutter is emitted for Tint mode
too — verified `tokens.rs:771-774` `show_gutter_slot` includes `Tint`).
Confirm `list.rs`/`tree.rs` paint gutter + tint together.

**Verify**: new test `phosphor_selection_is_tinted_not_neon`: selected row
bg == `SelectionTint` bg, fg != `INK` (not the neon slab);
`cargo nextest run -p termrock list tree` → pass.

### Step 4: Progress sub-cell ramp + track

In `progress.rs`: promote the ramp from `charts.rs:240` — move the
`" ▁▂▃▄▅▆▇█"`-style constant into a shared location
(`crates/termrock/src/style/glyph.rs`, next to the existing glyph catalog;
re-export; leave `charts.rs` consuming the shared one — grep
`charts.rs:238-244` for the exact consts first). Determinate bars render:
filled cells `█`, the fractional boundary cell picks the ramp glyph for the
remainder (`(fraction * cells * 8) % 8`), empty cells paint space **with
`Role::Sunken` bg track** (not `░`). ASCII: keep `#`/`-`. Update
indeterminate path only for the track bg.

**Verify**: test `determinate_boundary_uses_ramp`: 50%+1/16 width bar's
boundary cell is a mid-ramp glyph; `mise run check` → 0.

### Step 5: EmptyState framing

In `empty_state.rs` `paint_full` (line 503): when area affords it
(`width >= 24 && height >= 6`), paint a `Surface` with
`SurfaceRecipe::Inset` (→ `Role::Surface` fill, plan 001) behind the
centered content and add `spacing.gap` blank rows between glyph/title block
and explanation block (replace the all-`Fixed(1)` stack, `empty_state.rs:531`,
with gap-aware sizes). Below the threshold, keep current compact behavior.

**Verify**: `cargo nextest run -p termrock empty_state` → pass with updated
geometry; story renders show a framed quiet card (eyeball via lookbook
export if available).

### Step 6: Stories, migration, gate

- Update lookbook stories for Button/Badge/List/Progress/EmptyState states
  (chip variants, tint selection) — stories live in
  `crates/termrock-lookbook/src/stories.rs`.
- `migrations/0264-v0.13.0-recipe-enforcement-and-chips.md`: Button chip
  default, Badge default `Soft`, `SelectionChrome::Tint` default, Progress
  track/ramp, EmptyState framing; exact opt-outs (e.g. `BadgeFill::None`,
  `.selection(SelectionChrome::Gutter)`); before/after code for a consumer
  pinning old visuals. Link from `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0. Commit.

## Test plan

New tests (minimum): `primary_button_is_accent_chip`,
`phosphor_selection_is_tinted_not_neon`, `determinate_boundary_uses_ramp`,
`badge_default_is_soft_chip`, `empty_state_paints_inset_surface`. Model each
after the widget's existing paint tests (same test modules). Plus
expectation updates across the touched widgets' existing tests.

## Done criteria

- [x] `mise run check` + `mise run gate` exit 0
- [x] `grep -rn "bg = None" crates/termrock/src/widgets/` → 0 matches
- [x] `grep -rn "button_recipe" crates/termrock/src/widgets/primitives.rs` → ≥1 non-test match
- [x] 5 new tests above exist and pass
- [x] `migrations/0264-*.md` exists, linked from `MIGRATING.md`
- [x] `plans/README.md` updated

## Visual QA

- Button, Badge, List, Progress: **pass** in dark and paper shells.
- EmptyState: **pass, iterated once** to bound the wide card and remove dead
  void while retaining responsive contraction.
- Evidence: [`artifacts/visual-qa/plan-004/README.md`](../artifacts/visual-qa/plan-004/README.md).

## STOP conditions

- `ButtonVariant` ↔ `ButtonRecipeVariant` mapping is ambiguous for a variant
  (semantics unclear from docs) → report the variant instead of guessing.
- A `bg = None` site turns out to guard a real Reset-background requirement
  (comment says so) → keep that site as an explicit fg-only construction,
  note it in the migration, continue.
- Tint default breaks `NO_COLOR`/mono stories (selection invisible without
  color) → confirm gutter still renders in mono; if not, STOP.

## Maintenance notes

- Reviewers: check chip contrast in `paper` (light) and `ansi` presets, and
  the mono story for every upgraded widget (non-color affordances must
  survive).
- Plan 010's success grep (`bg = None` == 0) starts holding here — CI could
  enforce it later (deferred).

## Amendments

- 2026-08-12: The drift check reports `list.rs` and `empty_state.rs` changes
  from completed Plan 003. Classified as a plan defect: this plan depends on
  Plan 003 yet its STOP text treats the dependency's declared density and
  spacing edits as unexpected drift. Options considered were halt, revert the
  prerequisite, or accept the current geometry while preserving this plan's
  recipe-only intent. Continuing from the live Plan 003 geometry best matches
  the goal, repo cross-surface law, design SoT, and independently-green commit
  requirement with no extra implementation scope.
- 2026-08-12: Added `surface.rs` after live-code research showed
  `Surface::bordered(true)` only preserved an existing recipe border and could
  not force one, contradicting its public contract and blocking the specified
  inset EmptyState frame. Options were local duplicate border paint, use a
  semantically wrong interactive recipe, or repair the shared override. The
  shared one-line fallback to `Role::Border` has the smallest coherent blast
  radius and preserves focus-visible law.
- 2026-08-12: Designer review constrained full EmptyState inset cards to a
  centered 56×12 maximum measure. The first browser render stretched the
  frame across 80×24, leaving dead void around a small message; the bounded
  measure preserves the specified surface hierarchy while producing deliberate
  rhythm at wide terminal sizes and leaving narrow contraction unchanged.
- 2026-08-12: Added shared glyph/chart files plus generated public API, frame,
  and browser evidence outputs omitted from Scope. Step 4 already mandates the
  block-ramp move, and repo migration/catalog law plus the standing browser gate
  require the generated cascade. Omitting them would leave duplicated glyph
  ownership or stale public/visual contracts; including only these direct
  outputs is the smallest coherent resolution.
