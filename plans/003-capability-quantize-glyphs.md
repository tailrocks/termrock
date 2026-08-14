# Plan 003: Make the capability ladder honest — quantize fixes, mono survival, glyph de-collision

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 605217aa..HEAD -- crates/termrock/src/style/quantize.rs crates/termrock/src/style/glyph.rs crates/termrock/src/style/preview_host.rs crates/termrock/src/capability/`
> On drift, compare "Current state" excerpts to live code; mismatch = STOP.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW-MED
- **Depends on**: plans/002-role-palette-foundation.md (retune against new values)
- **Category**: bug / design-foundation
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

TermRock's degraded-terminal story is currently false advertising. The ANSI-16
quantizer maps phosphor green, danger red, warning yellow AND success mint all
to `Color::White`; the 256-color path collapses the entire surface ladder to
xterm index 16 (pure black); the monochrome path erases selection fills with
no `REVERSED` substitute; and the ASCII glyph set uses `>` for selection,
disclosure, chevron, folder, play, and prompt simultaneously. On exactly the
terminals where color can't carry meaning, TermRock deletes the non-color
meaning too. Design law: "color is never the only cue" and "capability is
progressive … first-class projections, not afterthoughts"
(`docs/design/terminal-design-system.md:30,33`).

## Current state (verified excerpts at `605217aa`)

All in `crates/termrock/src/`.

- `style/quantize.rs:32-51` — `detect_from_env()`: `NO_COLOR`→Monochrome;
  `COLORTERM` truecolor/24bit→Truecolor; `TERM` contains `256color`→Indexed256;
  `TERM==dumb`→Monochrome; else Ansi16. (So plain `TERM=xterm` = Ansi16 — a
  common real path.)
- `style/quantize.rs:59-62` — Monochrome arm: `Color::Reset => Color::Reset,
  _ => Color::Reset` (both arms identical; bg+fg both erased; no modifier
  substitution).
- `style/quantize.rs:97-113` — `rgb_to_xterm256`: grayscale branch requires
  exact `r==g&&g==b`; otherwise 6×6×6 cube via `v*5/255`. Phosphor ladder
  values (Canvas `(10,12,10)` … Elevated `(30,38,32)`) all floor to cube
  index 16.
- `style/quantize.rs:115-165` — `rgb_to_ansi16`: match arms like
  `(1, true, false, true, false) => LightGreen` require the *other* channels
  ≤40. `PHOSPHOR_GREEN (0,255,65)` has b=65 → falls to `_` → `White`. Same for
  `DANGER_RED (255,94,122)`, `WARNING_YELLOW (255,216,94)`,
  `SUCCESS_GREEN (61,220,90)`.
- `style/quantize.rs:92` — `let _ = Role::Text; // keep Role imported …` and
  `style/quantize.rs:6` + `style/mod.rs:10` — module-wide
  `#![allow(unused_variables, unused_mut)]` hiding dead branches.
- `style/preview_host.rs:164-173` — projections quantize palette but never
  downgrade `glyphs` or `selection`; `capability/boundary.rs:127-138` same.
  Only `DesignSystem::no_color()` (`style/tokens.rs:589-593`) forces ASCII.
- `style/glyph.rs:493` — `SelectionGutter => ("▌", ">", "▌")`; ASCII `>` also
  used by `DisclosureClosed`(:458), `ChevronRight`(:460), `ArrowRight`(:464),
  `Folder`(:479), `Play`(:486), `Prompt`(:503). Unicode dupes:
  `Folder "▸"`==`DisclosureClosed "▸"`, `FolderOpen "▾"`==`DisclosureOpen "▾"`,
  `Busy`/`Connection`/`Token`/`StatusDotTarget` all `"◉"`,
  `SelectionMark "▣"`==`CheckMixed "▣"`.
- `style/glyph.rs:514-520` — `unicode_cols` returns 1 via three identical
  match arms (dead branches).
- Guard tests that exist: `quantize.rs:228-234`
  `quantize_ansi_preserves_structure` (asserts nothing about values);
  `glyph.rs:599-612` width test is self-referential; `glyph.rs:645,652` id
  round-trip/partition only.

## Commands you will need

| Purpose | Command | Expected |
|---------|---------|----------|
| Fast gate | `mise run check` | exit 0 |
| Targeted | `cargo nextest run -p termrock quantize` / `glyph` / `capability` | all pass |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `crates/termrock/src/style/quantize.rs`,
`crates/termrock/src/style/glyph.rs`,
`crates/termrock/src/style/preview_host.rs`,
`crates/termrock/src/capability/boundary.rs`,
`crates/termrock/src/style/mod.rs` (ONLY the `#![allow…]` line and any test
additions), `crates/termrock/src/style/tokens.rs` (ONLY if `no_color()`
helpers need sharing), `migrations/0282-*.md` + `MIGRATING.md`.

**Out of scope**: role palette values (plan 002), widget paint code, recipes
(plan 004), lookbook SVGs (plan 011). Widgets' own hardcoded non-ASCII
literals (plan 006/007 fix the sites; this plan only fixes the catalog).

## Git workflow

Directly on `main`; one commit;
`git commit -s -m "fix(style)!: honest capability ladder — ansi16 nearest color, 256 gray ramp, mono reverse, glyph de-collision"`.

## Steps

### Step 1: Nearest-color ANSI-16 mapping

Replace `rgb_to_ansi16`'s heuristic match with a luminance-weighted
nearest-neighbor search over the 16 standard ANSI reference RGBs (use the
xterm defaults; keep the function signature). Table-test:
phosphor Accent→`LightGreen`, Danger `(255,94,122)`→`LightRed`, Warning
`(255,216,94)`→`LightYellow`, Info/CYAN→`Cyan`, Success mint→`LightGreen` or
`Green` (assert not White/Gray), Border graphite→`DarkGray`.

**Verify**: `cargo nextest run -p termrock quantize` → new table test passes.

### Step 2: Near-gray branch for xterm-256

In `rgb_to_xterm256`, before the cube fallback: if `max(r,g,b)-min(r,g,b) <= 12`,
map the average channel onto the 232-255 gray ramp (keep exact-gray branch).
Add test `surface_ladder_survives_256_quantization`: quantizing the phosphor
`Canvas/Surface/Raised/Elevated/Sunken` bgs yields 5 distinct indices, ordered
by the ramp.

**Verify**: `cargo nextest run -p termrock quantize` → passes.

### Step 3: Monochrome keeps structure

In `quantize_style` (the caller of `quantize_color` — locate via
`rg -n "fn quantize_style" crates/termrock/src/style/quantize.rs`): when
capability is Monochrome and the source style has `bg.is_some()` with a
non-Reset bg, add `Modifier::REVERSED` to the output. Remove the dead
`Color::Reset => Color::Reset, _ => Color::Reset` double-arm in favor of a
single `_ => Color::Reset` with a comment. Test: phosphor `Role::Selection`
and `Role::ActionFocused` quantized to Monochrome carry `REVERSED`;
`Role::Text` does not.

**Verify**: `cargo nextest run -p termrock quantize` → passes.

### Step 4: Projections force degraded selection + glyphs

In `preview_host.rs` `projected_theme`/`projected_tokens` and
`capability/boundary.rs` `project_system`: when target capability is
`Monochrome` (or glyph profile is ASCII), also set
`selection = SelectionChrome::Gutter` and `glyphs = GlyphSet::Ascii`
(mirroring `DesignSystem::no_color()`). Add a test in `capability/` asserting
a projected-to-mono system has Gutter + Ascii.

**Verify**: `cargo nextest run -p termrock capability` → passes.

### Step 5: Glyph catalog de-collision

In `glyph.rs`:
- `SelectionGutter` ASCII → `"|"` IF `"|"` is not already claimed by a rule
  glyph in the same group set — otherwise `"*"`; whichever you pick, remove
  that character from any other glyph's ASCII slot in collision.
- `Folder` Unicode → `"🗀"`-free option: use `"■"`? No — keep it simple:
  `Folder => ("▪", "F", "▪")` is ugly; prefer distinct existing pairs:
  `Folder => ("◆", "+", "◆")` conflicts with diamonds. DECISION RULE (apply
  mechanically): a glyph's Unicode encoding may not equal another glyph's
  encoding within the union of groups that can co-occur in one row (selection
  + disclosure + status). Resolve the four documented collisions:
  `Folder`/`DisclosureClosed`, `FolderOpen`/`DisclosureOpen`,
  `Busy|Connection|Token|StatusDotTarget` (keep `◉` for `StatusDotTarget`,
  give `Busy` `"◐"`, `Connection` `"◍"` or `"⧉"`, `Token` `"◈"`),
  `SelectionMark`/`CheckMixed` (give `SelectionMark` `"▮"`).
  All replacements must be 1-column East-Asian-Narrow characters.
- Collapse `unicode_cols`' three identical arms into one table; add doc note
  that EAW-Ambiguous glyphs are assumed narrow (a wider policy is deferred —
  record in Maintenance notes).
- CATALOG ADDITIONS (per `termrock-component-audit-2026-08.md` F5): left-half
  block ramp `▏▎▍▌▋▊▉` (Histogram 1/8 bar-tops, Slider sub-cell thumb,
  password strength meter); shade blocks `░▒▓` (mono multi-series hatch, soft
  fills — replaces inline literals at `charts.rs:1248,1758`); unified input
  mask `●` (ASCII `*`) replacing the 3-way `*`/`●`/`•` split
  (TextInput/PasswordInput/InputOtp); slider glyphs `Glyph::{SliderThumb,
  SliderFill, SliderRail, SliderTick}` (promote `slider.rs:187-203` literals);
  split-divider glyphs (promote `split_pane.rs:382-393`,
  `resizable_panel_group.rs:889-910` literals); checkbox `CheckOn/CheckOff`
  adoption at `multi_select.rs:1142-1146`, `menu_bar.rs:1641-1657`;
  reassign `◇` to now-edge/checkpoint (Info status glyph becomes `·` — audit
  decision D14).
- Add test: within each co-occurring group union, encodings are unique per
  `GlyphSet` (Unicode, Ascii, Enhanced).

**Verify**: `cargo nextest run -p termrock glyph` → uniqueness test passes.

### Step 6: Delete the lint blindfolds

Remove `#![allow(unused_variables, unused_mut)]` from `style/mod.rs:10` and
`style/quantize.rs:6`; delete `let _ = Role::Text;` (`quantize.rs:92`); fix or
remove any unused bindings clippy then flags in those files (test fixtures get
targeted `#[allow]` on the item, not the module).

**Verify**: `mise run check` → exit 0 (clippy denies warnings, so this proves the module is clean).

### Step 7: Migration + gate

`migrations/0282-*.md` (next free number): behavior changes on degraded
terminals (ANSI16 colors now map to nearest hue; 256-color surfaces now use
the gray ramp; mono adds REVERSED for filled styles; ASCII selection gutter
character changed; four Unicode glyph reassignments — list old→new exactly).
`MIGRATING.md` row.

**Verify**: `mise run gate` → exit 0.

## Test plan

New tests named in Steps 1-5; model after existing `#[test]` fns in
`quantize.rs`/`glyph.rs`. All `mise run check` green between steps.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "allow\(unused_variables" crates/termrock/src/style/` → 0 matches.
- [ ] Table test proves phosphor Danger/Warning/Accent/Info map to 4 distinct ANSI16 colors.
- [ ] 256 test proves 5 distinct surface indices.
- [ ] Mono test proves `Selection`→REVERSED.
- [ ] Glyph uniqueness test in place and passing.
- [ ] One new `migrations/` file + `MIGRATING.md` row.
- [ ] `plans/README.md` updated.

## STOP conditions

- `quantize_style` doesn't exist under that name / mono handling lives
  elsewhere — report the actual structure before rewriting.
- Plan 002 has not landed and palette values differ from the targets named in
  Step 1's table test — run the test against whatever values exist at HEAD
  and pin those hues instead; note it.
- Glyph replacements break the width test (a chosen character is not
  1-column) — pick another from the same family; if none fits, STOP and list
  candidates.

## Maintenance notes

- EAW-Ambiguous width policy (terminals rendering `✓●○◆` as 2 cells) is
  deferred: needs a capability flag + layout audit. Record as a follow-up in
  `plans/README.md` when closing this plan.
- Plan 011's lookbook capability-knob stories should show the new ANSI16/256
  projections; SVG churn lands there.
