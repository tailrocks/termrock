# Plan 001: Fill the surface ladder and expand the semantic role system

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/style crates/termrock/src/widgets/surface.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED (visible default change across every widget; mitigated by lookbook + full gate)
- **Depends on**: none (this is the foundation plan — everything else builds on it)
- **Category**: tech-debt (visual foundation)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

TermRock's default phosphor theme ships its four surface roles (`Canvas`,
`Surface`, `Elevated`, `Backdrop`) and `StatusBar` as **empty styles**. The
surface fill path deliberately drops empty styles, so every panel, card,
dialog, toast, and menu fill in the library is a runtime no-op — the whole
library renders as bare borders on the raw terminal background and reads
"cheap CLI", not "rich application". Additionally eight+ roles all resolve to
the identical phosphor green, so focus, accent, success, hints, scrollbar, tab
underline, and chart series are visually indistinguishable. The design SoT
(`docs/design/phosphor-obsidian-visual-direction.md`) already specifies the
correct layered values; this plan makes the palette adopt them and adds the
missing semantic roles that consumers (Jackin) and reference TUIs (Grok
Build) prove are needed. Nearly every later plan in `plans/` depends on the
roles introduced here.

## Current state

Files:

- `crates/termrock/src/style/mod.rs` — `Role` enum (line 114, `#[non_exhaustive]`),
  `ROLE_COUNT = 49` (line 216), `every_role!` macro (line 218),
  `RolePalette::tailrocks_phosphor()` (line 313), palette constants (lines 52–93).
- `crates/termrock/src/style/tokens.rs` — `DesignSystem` presets
  (`phosphor()` line 421, `slate()`, `paper()`, `ansi()`, `high_contrast()`,
  `adaptive()`), `resolve_list_row` (line 727).
- `crates/termrock/src/style/palette.rs` — RGB constants (`PHOSPHOR_GREEN_RGB` etc.).
- `crates/termrock/src/widgets/surface.rs` — `nonempty_fill` (line 414),
  `surface_recipe` resolution (line 426), pinning test (line 596).
- `crates/termrock/src/style/quantize.rs` — palette quantization (works; nothing to change, but new colors must pass through it — it maps over all roles generically).

Verified excerpts (as of `539e7d03`):

`style/mod.rs:313-319` — the first four roles (Canvas, Surface, Elevated,
Backdrop, in `Role` declaration order) are empty:

```rust
pub fn tailrocks_phosphor() -> Self {
    Self {
        roles: [
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new().fg(WHITE),
```

`style/mod.rs:357` — `StatusBar` role (index 35) is `Style::new()` (between
`ActionDisabled` at 356 and `DiffAdded` at 358).

`style/mod.rs:325-329,343,346` — `BorderFocused`, `Focus`, `Accent`,
`Success`, `TabUnderlineFocused`, `HintText` are all the shared `GREEN`
const (`Style::new().fg(PHOSPHOR_GREEN)`, defined line 91). `ChartSeries1`
(line 367) is `Style::new().fg(PHOSPHOR_GREEN)` too.

`style/mod.rs:326` — `Selection` is a solid neon slab:
`Style::new().bg(PHOSPHOR_GREEN).fg(INK)`.

`widgets/surface.rs:414-420` — fills drop empty styles:

```rust
fn nonempty_fill(style: Style) -> Option<Style> {
    if style.bg.is_some() || style.fg.is_some() || style.add_modifier != Modifier::empty() {
        Some(style)
    } else {
        None
    }
}
```

`widgets/surface.rs:596-604` — a test pins the emptiness:

```rust
#[test]
fn phosphor_raised_skips_empty_elevated_fill() {
    let system = DesignSystem::default();
    let plan = system.surface_recipe(SurfaceRecipe::Raised);
    // Phosphor Elevated is intentionally empty (terminal-default compatible).
    assert!(system.style(Role::Elevated).bg.is_none());
    assert!(plan.fill.is_none());
    assert!(plan.border.is_some());
}
```

`style/tokens.rs:765-767` — selection tint and hover are wired to fg-only
roles, so `SelectionChrome::Tint` can never wash a row background:

```rust
focus: self.style(Role::Focus),
hover: self.style(Role::LinkHover),
tint: self.style(Role::Focus),
```

Design constraints to honor (from `docs/design/phosphor-obsidian-visual-direction.md`
and repo `AGENTS.md`):

- Focus is communicated by border **color** (`Role::BorderFocused`), never
  border weight or glyph changes. Do not alter that law.
- Ladder values specified by the design doc: Canvas `#0a0c0a`, Surface
  `#121612`, Raised `#1a1f1a`, Elevated `#1e2620`, Sunken `#0d100d`,
  selection tint `#14331a`, hover tint `#1a221c`. Selection is "gutter +
  optional tint bg — not neon".
- The repo treats inconsistency as a defect: when you add roles, every
  palette preset (`slate`, `paper`, `ansi`, `high_contrast`) must define
  them — no preset may leave a new role accidentally empty.
- Breaking/dramatic public changes require the next sequential file under
  `migrations/` plus an entry in `MIGRATING.md` **in the same commit**
  (repo `AGENTS.md`, "Breaking-change documentation").

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Fast gate (fmt, clippy -D warnings, nextest, doctests) | `mise run check` | exit 0 |
| Full trunk gate (before push) | `mise run gate` | exit 0 |
| Targeted tests | `cargo nextest run -p termrock style::` | all pass |
| Lookbook preview regen (deterministic check uses it) | see `mise.toml` task `export-preview-frames` | exit 0 |

## Scope

**In scope** (the only files you should modify):

- `crates/termrock/src/style/mod.rs`
- `crates/termrock/src/style/tokens.rs`
- `crates/termrock/src/style/palette.rs`
- `crates/termrock/src/style/quantize.rs` (only if a test reveals new colors bypass it)
- `crates/termrock/src/widgets/surface.rs` (tests only)
- `migrations/0261-*.md` (create; number = last existing + 1 — verify with `ls migrations | tail -1`)
- `MIGRATING.md` (add index entry)
- `crates/termrock-lookbook/src/stories.rs` (only if theme-role stories fail to compile after role additions)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):

- Widget paint code (`widgets/*.rs` other than `surface.rs` tests). Widgets
  begin consuming the new roles in plans 003–005. If a widget test fails
  because a default color changed (expected — fills now exist), update the
  **test expectation**, not the widget.
- `docs/` site, `registry/` — regenerated in later plans.
- Border glyphs / `BorderShape` — that is plan 002.

## Git workflow

- Repo law: all work directly on `main`, no feature branches (repo
  `AGENTS.md`). The whole plans series executes sequentially on `main`.
- Conventional Commits with DCO sign-off: `git commit -s`. Example from log:
  `fix(ci): verify rendered docs route`. Suggested message for this plan:
  `feat(style)!: fill phosphor surface ladder and expand semantic roles`.
- Commit only when `mise run check` is green; push only when `mise run gate`
  is green.

## Steps

### Step 1: Add the new `Role` variants

In `crates/termrock/src/style/mod.rs`, extend the `Role` enum (line 114).
Insert `Raised` after `Surface` and `Sunken` after `Elevated` so the ladder
reads Canvas → Surface → Raised → Elevated → Sunken; append the remaining new
roles **at the end of the enum** (before the chart roles is also acceptable —
pick one placement and keep the `roles:` array, `every_role!` macro, and
`ROLE_COUNT` in exact declaration order):

- `Raised` — "Hover/section surface between Surface and Elevated."
- `Sunken` — "Recessed well surface (inputs, wells)."
- `SelectionTint` — "Quiet selected-row background wash."
- `HoverTint` — "Pointer-hover row background wash."
- `ActionConstructive` — "Creation/additive action rows (`+ Add …`)."
- `DisclosureHeader` — "Expand/collapse group header accent."
- `InfoStrong` / `InfoDim` — "Live-status two-tier pair."
- `ActorUser`, `ActorAssistant`, `ActorThinking`, `ActorTool`, `ActorPlan`,
  `ActorSystem` — "Agent-surface actor accents (transcript rails, tool
  cards, plan review)."

Every variant needs a doc comment (workspace denies `missing_docs`). Update
`ROLE_COUNT` (49 → 64) and the `every_role!` macro list (line 218) to match
declaration order exactly.

**Verify**: `cargo check -p termrock` → compiles; any non-exhaustive-match
errors show you every site that indexes the palette array — fix those in the
same declaration order.

### Step 2: Fill the phosphor palette

In `tailrocks_phosphor()` (`style/mod.rs:313`), replace the four empty
surface styles and `StatusBar`, and give the collapsed greens distinct
values. Define new colors in `style/palette.rs` as RGB consts following the
existing naming (`PHOSPHOR_GREEN_RGB` pattern) and lift them in `mod.rs` via
the existing `color()` helper:

| Role | Style |
|---|---|
| `Canvas` | `bg #0a0c0a` |
| `Surface` | `bg #121612` |
| `Raised` | `bg #1a1f1a` |
| `Elevated` | `bg #1e2620` |
| `Sunken` | `bg #0d100d` |
| `Backdrop` | `fg` dark gray `#3a443a`, no bg (the dim-wash glyph color; dialog backdrop keeps terminal-default bg per the `DIALOG_BACKDROP` comment at `mod.rs:58-63`) |
| `StatusBar` | `bg #121612`, `fg` = existing `WHITE` |
| `SelectionTint` | `bg #14331a` |
| `HoverTint` | `bg #1a221c` |
| `Selection` | keep `bg(PHOSPHOR_GREEN).fg(INK)` (used only by `SelectionChrome::Fill`) |
| `Focus` | keep green fg |
| `BorderFocused` | keep `PHOSPHOR_GREEN` (identity law) |
| `Accent` | keep `PHOSPHOR_GREEN` (brand accent) |
| `Success` | distinct: `fg #3ddc5a` (calmer than accent green) |
| `HintText` | `fg` = `PHOSPHOR_DIM` (hints are quiet, not accent) |
| `TabUnderlineFocused` | keep green |
| `ScrollThumb` | keep current |
| `ChartSeries1` | distinct from Accent: `fg #2bd968` |
| `ActionConstructive` | `fg #b4ffb4` (mint — Jackin `ACTION_ACCENT`) |
| `DisclosureHeader` | `fg #ffd066` (amber — Jackin `DISCLOSURE_ACCENT`) |
| `InfoStrong` | `fg` = existing `CYAN` |
| `InfoDim` | `fg #007878` |
| `ActorUser` | `fg #c8c8c8` |
| `ActorAssistant` | `fg #bb9af7` |
| `ActorThinking` | `fg #9a7fd1` |
| `ActorTool` | `fg #787878` |
| `ActorPlan` | `fg #ffdb8d` (golden plan accent) |
| `ActorSystem` | `fg #7aa2f7` |

Then update **every other preset** in the same file so no new role is
accidentally empty: `slate()`, `paper()` (light equivalents — pick lighter
surface values, e.g. paper surfaces are light grays and tints are light
green washes), `ansi()` (map to the nearest of the 16 ANSI colors),
`high_contrast()` (high-contrast values). Every palette constructor builds
the same 64-length array.

**Verify**: `cargo nextest run -p termrock style::` → pass, plus write and
run the Step 4 tests.

### Step 3: Rewire selection/hover tints and delete the pinning test

- In `style/tokens.rs` `resolve_list_row` (lines 765-767): change
  `tint: self.style(Role::Focus)` → `tint: self.style(Role::SelectionTint)`
  and `hover: self.style(Role::LinkHover)` → keep `hover` label style as-is
  BUT add a new field or reuse: the row wash for `hover_fill` must come from
  `Role::HoverTint`. Look at `widgets/list.rs:1209-1215` (verified):

  ```rust
  if recipe.use_fill {
      buffer.set_style(rect, style);
  } else if recipe.use_tint {
      buffer.set_style(rect, recipe.tint);
  } else if recipe.hover_fill {
      buffer.set_style(rect, recipe.hover);
  }
  ```

  `recipe.hover` is also used for hovered **label** styling
  (`list.rs:1202-1203`). Split the concerns: add `hover_wash: Style` to
  `ListRowRecipe` (populated from `Role::HoverTint`), switch the
  `hover_fill` branch in `list.rs` (and the equivalent in
  `widgets/tree.rs` near line 1042) to `recipe.hover_wash`. Keep
  `recipe.hover` for label fg.
- In `widgets/surface.rs`, replace the pinning test
  `phosphor_raised_skips_empty_elevated_fill` (line 596) with:
  - `phosphor_surface_ladder_is_populated`: assert `Canvas`, `Surface`,
    `Raised`, `Elevated`, `Sunken`, `StatusBar`, `SelectionTint`,
    `HoverTint` all have `bg.is_some()` in `DesignSystem::default()`.
  - `phosphor_raised_fill_is_painted`: `surface_recipe(SurfaceRecipe::Raised)`
    returns `plan.fill == Some(style(Role::Elevated))` (note: the recipe maps
    Raised → `Role::Elevated` per `surface.rs:430`; that mapping is fine to
    keep in this plan).
  - Keep `nonempty_fill` itself — it still protects the
    `terminal_native` palette (Step 5).

**Verify**: `cargo nextest run -p termrock surface` → pass;
`cargo nextest run -p termrock list` → pass (update snapshot-style
assertions that expected no tint).

### Step 4: Add ladder invariants and contrast tests

In `style/mod.rs` tests (or the existing style test module), add:

- `ladder_is_monotonic`: for phosphor, luminance(Canvas) < luminance(Surface)
  < luminance(Raised) < luminance(Elevated); Sunken < Surface. Compute
  luminance from the `Color::Rgb` channels with the standard
  `0.2126r+0.7152g+0.0722b` formula in the test.
- `accents_are_distinct`: in phosphor, the style pairs that used to collapse
  are no longer equal: `Success != Accent`, `HintText != Accent`,
  `ChartSeries1 != Accent`.
- `every_preset_fills_new_roles`: for each preset in
  [`phosphor`, `slate`, `paper`, `ansi`, `high_contrast`], every new role
  resolves non-empty (fg or bg present).
- `tint_roles_carry_bg`: `SelectionTint` and `HoverTint` have `bg.is_some()`
  in every truecolor preset.

**Verify**: `cargo nextest run -p termrock style::` → all pass including the
4 new tests.

### Step 5: Add `RolePalette::terminal_native()`

New constructor in `style/mod.rs` next to `tailrocks_phosphor()`: identical
role array except all five surface-ladder roles + `StatusBar` use
`Color::Reset` backgrounds (i.e. `Style::new()` for surfaces — the existing
empty pattern), documented as "terminal-default background variant for hosts
that must inherit the operator's terminal theme (previous default
behavior)". Add `DesignSystem::terminal_native()` preset in `tokens.rs`
following the shape of `phosphor()` (`tokens.rs:421-423`). This preserves
the Jackin-documented `Color::Reset` policy as an explicit opt-in.

**Verify**: `cargo nextest run -p termrock style::` → pass; doctest on the
new constructor compiles.

### Step 6: Migration file + MIGRATING.md

Create `migrations/0261-v0.13.0-surface-ladder-and-role-expansion.md`
(confirm 0261 is still next: `ls migrations | tail -1` → currently `0260-…`).
Follow the structure of `migrations/0260-*.md` (read it first). Must record:

- Default visual change: phosphor now paints surface fills, status-bar band,
  selection/hover tints; screenshots/lookbook story names to compare.
- New `Role` variants (full list), `ROLE_COUNT` 49 → 64.
- Consumers matching exhaustively on `Role` (it is `#[non_exhaustive]`, so
  only crate-internal matches break) and consumers with custom `RolePalette`
  arrays must add the new roles.
- Opt-out: `DesignSystem::terminal_native()` restores terminal-default
  surfaces.
- `ListRowRecipe` field addition (`hover_wash`).
- Validation commands (`mise run check`).

Add the ordered index entry in `MIGRATING.md`.

**Verify**: `ls migrations | tail -1` → `0261-…`; `grep 0261 MIGRATING.md` → 1+ match.

### Step 7: Full gate and commit

**Verify**: `mise run check` → exit 0. Then `mise run gate` → exit 0.
Commit as described in Git workflow (single commit; migration file included).

## Test plan

- New tests listed in Steps 3–4 (6 new tests minimum), placed in the existing
  `#[cfg(test)]` modules of `style/mod.rs` / `widgets/surface.rs` — model
  after the existing tests in those modules.
- Expect existing widget tests to fail on changed default colors: each
  failure is reviewed — if it asserts the OLD empty-surface behavior, update
  the expectation; if it asserts something unrelated that broke, STOP.
- Verification: `mise run check` → exit 0.

## Done criteria

- [ ] `mise run check` exits 0; `mise run gate` exits 0
- [ ] `cargo nextest run -p termrock style::` passes with the new invariant tests
- [ ] Test `phosphor_raised_skips_empty_elevated_fill` no longer exists:
      `grep -rn "phosphor_raised_skips_empty_elevated_fill" crates/` → no matches
- [ ] `DesignSystem::default()` has `style(Role::Surface).bg.is_some()` (asserted by new test)
- [ ] `migrations/0261-*.md` exists and is linked from `MIGRATING.md`
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The excerpts above don't match the live code (drift since `539e7d03`).
- `Role` is matched exhaustively somewhere in a way that suggests palette
  arrays exist outside `style/mod.rs` (other than the presets listed) — the
  palette storage design has changed.
- More than ~15 widget tests fail after Step 2 for reasons that are NOT
  "expected color now present" — the fill path may be wired differently
  than this plan assumes.
- Quantization tests fail for the new colors and the fix would require
  changing `quantize.rs` semantics (not just adding coverage).

## Maintenance notes

- Plans 003–005 assume the roles and values introduced here; if you rename a
  role, update those plan files in the same commit.
- Reviewer should scrutinize: the declaration-order alignment of `Role`,
  `every_role!`, `ROLE_COUNT`, and all five palette arrays (an off-by-one
  silently shifts every color in a preset); and lookbook previews for the
  ladder actually reading as layered (Canvas vs Surface vs Elevated).
- Deferred: docs-site token bridge and per-widget adoption (later plans).
