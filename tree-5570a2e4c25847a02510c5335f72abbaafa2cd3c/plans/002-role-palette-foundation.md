# Plan 002: Rebuild the role palette — text ladder, accent de-collapse, per-name preset construction

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 605217aa..HEAD -- crates/termrock/src/style/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P0
- **Effort**: L
- **Risk**: MED (every visual snapshot moves — intended)
- **Depends on**: plans/001-underline-grammar-doc-sot.md (doc precedence; can run in parallel)
- **Category**: tech-debt / design-foundation
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

The phosphor palette assigns the identical `#00ff41` to `BorderFocused`,
`Focus`, `Accent`, `ScrollThumb`, `TabUnderlineFocused`, `DiffAdded` fg, the
`Selection` fill bg, and `ActionFocused` bg — 13 of 63 role slots are
phosphor-green. The text ladder is pure white body + bold-white strong +
*green* muted, and there is no faint tier at all. The design SoT
(`docs/design/termrock-design-language.md` §4.1) specifies a graphite
surface/foreground ladder with rare jewel accent; none of the widget-level
polish in later plans can read correctly until the tokens stop shouting.
Additionally, auto light-mode maps to the **dark** slate palette (real bug),
and the five 63-slot positional theme arrays are a standing mis-paint hazard.

## Current state (verified excerpts at `605217aa`)

All in `crates/termrock/src/style/`.

- `palette.rs:24-59` — constants (`pub(crate) const`): `PHOSPHOR_GREEN=(0,255,65)`,
  `PHOSPHOR_DIM=(0,140,30)`, `DIALOG_SCROLL_THUMB = PHOSPHOR_GREEN`,
  `WHITE=(255,255,255)`, `INPUT_BG_DIM=(20,24,22)`, `SURFACE=(18,22,18)`,
  `SUNKEN=(13,16,13)`, `BORDER_GRAY=(80,80,80)`, `SUCCESS_GREEN=(61,220,90)`,
  `TAB_BG_INACTIVE=(30,30,30)`, `TAB_BG_ACTIVE=(42,42,42)` (+hover variants),
  `LINK_FG=(0,200,200)`, `CYAN=(0,180,180)`.
- `mod.rs:361-437` — `RolePalette::tailrocks_phosphor()` is a positional
  63-entry array. Slots (in `Role` order, enum at `mod.rs:120-247`):
  `Text` = `fg(WHITE)`, `TextStrong` = `BOLD_WHITE`, `TextMuted` = `DIM`
  (= `fg(PHOSPHOR_DIM)` green), `TextDisabled` = `fg(BORDER_GRAY)`;
  `Border` = `BORDER`; `BorderFocused`/`Focus`/`Accent` = `GREEN`;
  `Selection` = `bg(PHOSPHOR_GREEN).fg(INK)`;
  `Link` = `fg(LINK_FG)` (NOT underlined), `LinkHover` = `fg(LINK_FG_HOVER)`;
  `TabActive..TabInactiveHovered` = `fg(WHITE).bg(TAB_BG_*)`;
  `TabUnderlineFocused` = `GREEN`; `ActionFocused` = `fg(INK).bg(PHOSPHOR_GREEN).bold()`;
  `DiffAdded` = `fg(DIFF_ADDED_FG /* = PHOSPHOR_GREEN, mod.rs:99 */).bg(DIFF_ADDED_BG)`.
- `mod.rs:250` — `pub const ROLE_COUNT: usize = 63;`
- `mod.rs:352-356` — `pub struct RolePalette { roles: [Style; ROLE_COUNT] }`.
- The other four presets are positional arrays too: slate `mod.rs:~464-552`,
  paper `:556-637`, ansi `:641-709`, high_contrast `:713-784`. `from_fn`
  helper already exists (`mod.rs:~825-833`).
- `appearance.rs:28-35` — `AppearanceThemeMap::default { dark: "phosphor", light: "slate" }`.
- `appearance.rs:88-93` — `palette_for_appearance(Appearance::Light) => RolePalette::slate()`
  — slate canvas is dark navy `Rgb(15,23,42)`: light terminals get a dark theme.
- `appearance.rs:97-111` — `read_macos_interface_style()` spawns `defaults`
  per call; `detect()` has no memoization.
- Known guard tests to update: `mod.rs:~913` `ladder_is_monotonic`,
  `mod.rs:~933` `accents_are_distinct`, `mod.rs:~1002`
  `default_separates_ordinary_and_strong_text`, `mod.rs:~1048`
  `phosphor_preset_pins_load_bearing_role_values`, `mod.rs:~1101` slate pins,
  `appearance.rs:118-133` appearance tests.

Repo conventions: no hardcoded RGB in widgets (all color changes live in
`style/`); every visible default change ships a numbered `migrations/` file +
`MIGRATING.md` row in the same commit; work directly on `main`; commit with
`git commit -s`, Conventional Commits.

## Target values (from `docs/design/termrock-design-language.md` §4.1 — quote for reference)

| Role | New value |
|------|-----------|
| `Text` | `#d6e0d6` |
| `TextStrong` | `#f0f5f0` + bold |
| `TextMuted` | `#7a8a7a` (neutral gray-green, NOT phosphor) |
| `TextDisabled` | `#4a574a` |
| `TextFaint` (NEW role) | `#4a574a` dim — meta/timestamps tier |
| `Border` | `#2a332c` |
| `BorderFocused` | `#00ff41` (owner only — keeps brand) |
| `Accent` | `#00ff41` (kept; budget enforced at widget layer) |
| `Focus` | `#33ff6a` (distinct from BorderFocused; non-border focus cue) |
| `Success` | `#5dffa0` (soft mint, split from brand) |
| `Selection` | keep fill pair but treat as opt-in only (recipe plan 004 stops default use) |
| `SelectionTint` | `#14331a` bg (keep) |
| `HoverTint` | `#1a221c` bg (slightly stronger than today's `(26,34,28)`) |
| `ScrollThumb` | `#2a332c` graphite; `ScrollTrack` `#161b16` |
| `TabActive` | `fg #f0f5f0` bold, **no bg** |
| `TabInactive` | `fg #7a8a7a`, **no bg** |
| `TabActiveHovered`/`TabInactiveHovered` | same fg + `HoverTint` bg |
| `TabAccent` (rename of `TabUnderlineFocused`) | `#00ff41` |
| `TabAccentQuiet` (rename of `TabUnderlineUnfocused`) | `#2a332c` |
| `Input` | bg = Sunken `#0d100d` (well recesses; delete `INPUT_BG_DIM` divergence) |
| `Link` | `fg #5ec8ff` (blue family — separates from `Info` teal), no underline (grammar §5.7) |
| `DiffAdded` | `fg` Success mint on the existing dark-green bg |

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Fast gate | `mise run check` | exit 0 (fmt, clippy -D warnings, nextest) |
| Style tests only | `cargo nextest run -p termrock style::` | all pass |
| Full gate (end) | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/style/palette.rs`
- `crates/termrock/src/style/mod.rs`
- `crates/termrock/src/style/appearance.rs`
- `crates/termrock/src/widgets/tabs.rs` (mechanical: role rename only)
- Any test file asserting the old values (update pins to new values)
- `migrations/0281-*.md` (new) + `MIGRATING.md` (index row)

**Out of scope** (do NOT touch):
- `crates/termrock/src/style/tokens.rs` recipes and `SelectionChrome` default
  (plan 004 owns them).
- `quantize.rs` (plan 003).
- Widget paint logic beyond the mechanical `Role::TabUnderline*` rename.
- Lookbook SVG regeneration (plan 011) — but run whatever preview gate
  `mise run check` includes and update inline expectations it flags.

## Git workflow

- Directly on `main`; one commit; `git commit -s -m "feat(style)!: graphite text ladder, accent de-collapse, named-role presets"`.
- Do NOT push unless the operator instructed it.

## Steps

### Step 1: Convert all five presets from positional arrays to `from_fn` name-matched construction

For each of `tailrocks_phosphor()`, `slate()`, `paper()`, `ansi()`,
`high_contrast()` in `mod.rs`: replace the `roles: [ ... 63 entries ... ]`
literal with `Self::from_fn(|role| match role { Role::Canvas => ..., ... })`
using an exhaustive match (no `_` arm for the value mapping; the enum is
`#[non_exhaustive]` but this module is in-crate so exhaustive matching
compiles). Preserve today's values exactly in this step — this step is a
pure refactor so the diff of behavior is empty.

**Verify**: `mise run check` → exit 0; `cargo nextest run -p termrock style::` → all pass with zero value changes.

### Step 2: Add `Role::TextFaint`, rename `TabUnderlineFocused`→`TabAccent`, `TabUnderlineUnfocused`→`TabAccentQuiet`

- Append `TextFaint` as a new LAST enum variant (`mod.rs` Role enum) with doc
  `/// Faint meta text (timestamps, counts) — below TextMuted.`; ALSO append
  `BackdropWash` (`/// Bg-carrying overlay dim: Canvas blended ~60% — painted
  under every modal layer.` — value: `bg` = blend_toward(Canvas, 0.6) per
  `web-premium-tui-law.md` P3/P12); bump `ROLE_COUNT` to 65; add both arms to
  all five `from_fn` matches and to the `every_role!` macro list (`mod.rs:252+`).
- RETIRE the two tab-underline roles entirely (supersedes the earlier rename
  idea — `termrock-component-audit-2026-08.md` D2 kills the rule-row model):
  delete `TabUnderlineFocused`/`TabUnderlineUnfocused` from the enum and all
  five presets; in `widgets/tabs.rs` delete the `━` rule row paint
  (~`:1229-1240`) and its test (`:1376`); as the minimal interim active cue,
  paint the selected tab label with `SelectionTint` bg + `TextStrong` + BOLD
  (the full `TabsActiveCue::{AccentPill, Connected, Marker, Rule}` variant
  model lands in plan 015). Net ROLE_COUNT change with the two additions
  above: 63 − 2 + 2 = 63... recount and pin whatever it is.

**Verify**: `rg -n "TabUnderline" crates/` → 0 matches; `mise run check` → exit 0.

### Step 3: Apply the phosphor target values

Edit `palette.rs` constants + the phosphor `from_fn` match to the Target
values table above. Notes:
- Keep `PHOSPHOR_GREEN` for `BorderFocused`, `Accent`, `ActionFocused`,
  `TabAccent` only. `Focus` gets a NEW constant `FOCUS_GREEN=(51,255,106)`.
- `ScrollThumb`/`ScrollTrack`: retire `DIALOG_SCROLL_THUMB = PHOSPHOR_GREEN`
  alias; graphite values above.
- `Input` bg becomes `SUNKEN`; delete `INPUT_BG_DIM` (also used by
  `InputInvalid` — that becomes `bg(SUNKEN).fg(DANGER_RED)`).
- `TextMuted` loses the green `PHOSPHOR_DIM`; keep `PHOSPHOR_DIM` only where
  syntax-comment/hint roles intentionally stay green — move `SyntaxComment`
  to `#7a8a7a` too; `HintText`/`HintDim`/`ActionDisabled` take the new
  muted/disabled grays, NOT `PHOSPHOR_DIM`.
- `Link` = `#5ec8ff`; `LinkHover` = `#8fd8ff` (still no underline in the
  role — link paint policy is plan 005's `LinkStyle`).

**Verify**: `cargo nextest run -p termrock style::` → failures ONLY in value-pinning tests; update those pins to the new values in the same step, then all pass.

### Step 4: Strengthen the guard tests

In `mod.rs` tests:
- Extend `accents_are_distinct` to assert pairwise distinctness of
  `{BorderFocused, Focus, Accent, Success, ScrollThumb, TabAccent, DiffAdded.fg, ChartSeries1}`
  foregrounds for phosphor.
- Extend `ladder_is_monotonic` to also assert `Input.bg == Sunken.bg` and a
  minimum luminance step ≥ 8 (channel-sum delta) between consecutive ladder
  surfaces Canvas→Surface→Raised→Elevated.
- Extend `default_separates_ordinary_and_strong_text` to assert
  `Text.fg != TextStrong.fg` (value step, not just bold) and
  `TextMuted.fg != TextStrong.fg != TextFaint.fg`.
- Add `hc_and_paper_have_text_ladders`: for `paper()` and `high_contrast()`,
  assert `Text != TextStrong` styles and `TextMuted != Text`.

**Verify**: `cargo nextest run -p termrock style::` → all pass.

### Step 5: Fix the non-phosphor presets (keep hierarchy rules, change hue only)

- `high_contrast()`: body `#e6e6e6` non-bold, strong `#ffffff` bold, muted
  `#c0c0c0`, disabled `#8a8a8a`, `TextFaint` `#9a9a9a` dim; `Sunken` `#0a0a0a`;
  `Backdrop` distinct from Canvas; `Selection` → `bg(#005050)` + bold (kill
  the white slab).
- `paper()`: `SelectionTint` → blue wash `#dbeafe`-family (distinct from
  `DiffAdded` bg `#dcfce7`); `HoverTint` → warm neutral `#f1efec`; `Input`
  bg → its `Sunken` `#eeebe6`.
- `ansi()`: `HoverTint` → `bg(Black)` (distinct from `SelectionTint`
  DarkGray); `Elevated` → `bg(Gray)`; `Accent` → `Green` (align with
  Focus/BorderFocused; `Info` keeps Cyan); `ActorThinking` → `LightMagenta`;
  `Input` → `bg(Black)`.
- `slate()`: apply `TextFaint` + tab-chip quieting consistent with Step 3
  (no bg on TabActive/TabInactive).

**Verify**: `cargo nextest run -p termrock style::` → all pass, including Step 4's new assertions run against phosphor+paper+HC.

### Step 6: Fix light-appearance mapping and memoize detection

In `appearance.rs`:
- `AppearanceThemeMap::default().light` → `"paper"`.
- `palette_for_appearance(Appearance::Light)` → `RolePalette::paper()`.
- Wrap `Appearance::detect()` in a `std::sync::OnceLock` memo (add
  `pub fn detect_fresh()` keeping the uncached path); doc-comment: call once
  at startup.
- Update tests at `appearance.rs:118-133`: `theme_key` maps light→"paper";
  add assertion that the light palette's `Canvas` bg luminance is greater
  than the dark palette's.

**Verify**: `cargo nextest run -p termrock style::appearance` → all pass.

### Step 7: Migration + index + gate

- Write `migrations/0281-v0.14.0-graphite-role-ladder.md` (use the next free
  number if 0281 is taken — check `ls migrations | tail -3`): list every role
  whose default value changed (table old→new), the `TabUnderline*`→`TabAccent*`
  rename with exact `rg`-able before/after identifiers, the new `TextFaint`
  role + `ROLE_COUNT` 63→64, the appearance light→paper change, and the
  consumer edit recipe ("if you pinned role values, re-pin; if you matched on
  `Role`, add `TextFaint` arm").
- Add the row to `MIGRATING.md` in numeric order.

**Verify**: `mise run gate` → exit 0. `git diff --stat` touches only in-scope files.

## Test plan

- Updated pin tests (Step 3), strengthened guards (Step 4), appearance tests
  (Step 6). Model new tests after the existing `mod.rs` test module style
  (plain `#[test]` fns asserting on `RolePalette` values).
- `mise run check` and `mise run gate` green.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "TabUnderline" crates/ docs/design/terminal-design-system.md` → 0 code matches (doc updated by plan 001).
- [ ] `rg -n "INPUT_BG_DIM" crates/` → 0 matches.
- [ ] `rg -c "Style::new\(\)" crates/termrock/src/style/mod.rs` shows the positional arrays are gone (presets constructed via `from_fn` name matches).
- [ ] `migrations/` gained exactly one new file; `MIGRATING.md` row added.
- [ ] `plans/README.md` status row updated.

## STOP conditions

- The positional arrays' current values do not match the "Current state"
  excerpts (drift) — report before refactoring.
- `ROLE_COUNT` bump breaks a public consumer inside this workspace other than
  the five presets and `every_role!` — report the file rather than patching it
  ad hoc.
- Any Step's `mise run check` fails twice after a reasonable fix attempt.
- You find widgets whose appearance depends on `Role::TabActive` having a bg
  (tab strip unreadable at height 1 after Step 3) — report; the follow-up cue
  lands in plan 008, but tabs must not become unreadable in this commit; if
  they do, keep `TabActiveHovered` bg treatment on `TabActive` temporarily and
  note it in the migration file.

## Maintenance notes

- Plan 003 retunes quantize against these values; plan 004 changes
  `SelectionChrome` default and recipes; plan 011 regenerates lookbook SVGs —
  expect large intentional SVG churn there, not here.
- Reviewer: check `from_fn` matches are exhaustive per preset (no `_` arm),
  and that no widget file except `tabs.rs` appears in the diff.
