# Plan 006: Motion, text-effect, and glyph-catalog kit

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/style crates/termrock/src/text crates/termrock/src/widgets/spinner.rs crates/termrock/src/widgets/charts.rs`
> style/ churn from 001–005 expected; on structural mismatch (files moved,
> modules renamed), STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW (almost entirely additive API)
- **Depends on**: plans/001-surface-ladder-and-role-expansion.md (roles used in examples/tests)
- **Category**: tech-debt (missing primitives)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

Consumers hand-build the presence/motion layer that makes premium TUIs feel
alive: Jackin implemented cell-run coalescing, smoothstep edge fades, a
brightness ripple, alpha fade-in, and duplicated TermRock's spinner frames
byte-for-byte; Grok Build's signature "alive while working" cues are sin²
wave/pulse helpers over accent rails, a width-invariant glyph catalog with
per-glyph legacy fallback, and quiet dot-pulse spinners. None of this exists
as reusable TermRock API. This plan ships the neutral kit; plans 007–009
consume it.

## Current state

- `crates/termrock/src/style/mod.rs:95-109` (verified) — `faded(color, alpha)`
  exists: multiplies RGB toward black. It fades toward **black**, not toward
  the canvas color — fine for phosphor, wrong for light themes; the new
  helpers must blend toward an explicit target color.
- `crates/termrock/src/widgets/status_bar.rs:797-800` (verified) — a private
  `fade_style(style, alpha)` exists in `status_bar.rs`; generalize it (do
  not leave two implementations — find it: `grep -n "fn fade_style" crates/termrock/src/widgets/status_bar.rs`).
- `crates/termrock/src/widgets/charts.rs:238-244` — block ramp
  (`" ▁▂▃▄▅▆▇█"`-class) and braille ramp constants live inside charts (plan
  004 Step 4 moves the block ramp to `style/glyph.rs`; if plan 004 already
  landed, extend the shared location instead of re-moving).
- `crates/termrock/src/widgets/spinner.rs` — `SPINNER_BRAILLE_FRAMES` const
  (Jackin duplicates it because it isn't reusable inline; check its
  visibility: `grep -n "SPINNER_BRAILLE_FRAMES" crates/termrock/src/widgets/spinner.rs`).
- `crates/termrock/src/style/glyph.rs` — existing glyph catalog (~44 glyphs,
  `Glyph` enum + `GlyphSet` resolution).
- `crates/termrock/src/text/` — text measurement helpers
  (`display_cols`, `display_cols_slice_into` used by status_bar, verified).
  Three consumer-side truncation reimplementations justify
  `truncate_cols` here.
- Motion reduction: `Motion` type exists in the capability system (README:
  "Motion reduction" is a supported progressive-enhancement axis) — find it:
  `grep -rn "enum Motion" crates/termrock/src`.

Reference formulas to implement (from Grok Build, Apache-2.0, studied from
source — reimplement, do not copy code):

- `pulse_brightness(tick, period) = sin²(π · (tick % period) / period)` —
  temporal pulse for a single glyph/icon.
- `wave_brightness(tick, row, wave_rows, speed)` — spatial sin² band flowing
  along a rail: phase = `(row − tick·speed) / wave_rows`, brightness =
  `sin²(π · phase)` clamped 0..1, ~32 rows per cycle default.
- Edge fade (Jackin): smoothstep `r²(3−2r)` over the outer N columns.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Targeted | `cargo nextest run -p termrock motion glyph text spinner` | pass |

## Scope

**In scope**:

- `crates/termrock/src/style/motion.rs` (new), `style/mod.rs` (module decl + re-exports)
- `crates/termrock/src/style/glyph.rs` (catalog v2)
- `crates/termrock/src/text/` (truncate_cols; effects helpers if text-shaped)
- `crates/termrock/src/widgets/spinner.rs` (frame reuse + dot-pulse tier)
- `crates/termrock/src/widgets/status_bar.rs` (delete private `fade_style`, use shared)
- `crates/termrock/src/widgets/charts.rs` (consume shared ramps only)
- `crates/termrock/src/perf/` — only if cell-run coalescing best lives there; decide by reading `perf/` module docs first
- `crates/termrock-lookbook/src/stories.rs` (motion demo story)
- `registry/` + docs inventory regen if the build checks demand it (`mise run contracts` — see mise.toml task)
- `docs/api/public-api.txt`, `docs/content/docs/components/spinner.mdx`, affected
  `docs/public/preview-frames/`, and `artifacts/visual-qa/plan-006/`
- `migrations/0266-*.md` + `MIGRATING.md` (only the `faded`/`fade_style` consolidation is breaking; otherwise additive note)
- `plans/README.md`

**Out of scope**:

- Consuming widgets (AccentRail etc. — plan 007; agent surfaces — plan 008).
- Frame-clock/timing changes in `runtime::run` — helpers are pure functions
  of a tick the host already owns (`FrameTick` is the existing vocabulary —
  grep `FrameTick` and match its field names in signatures).

## Git workflow

`main`, `git commit -s`. Suggested:
`feat(style): motion kit, text effects, glyph catalog v2, truncate_cols`.

## Steps

### Step 1: `style/motion.rs`

Pure functions, no state, documented units (ticks, not ms — derive from
`FrameTick` at call sites):

```rust
pub fn pulse_brightness(tick: u64, period: u64) -> f32;          // sin², 0..=1
pub fn wave_brightness(tick: u64, row: u16, wave_rows: u16, speed: f32) -> f32;
pub fn smoothstep(t: f32) -> f32;                                 // t²(3−2t), clamped
pub fn edge_fade(col: u16, width: u16, fade_cols: u16) -> f32;    // smoothstep at both edges
pub fn blend_toward(from: Color, to: Color, t: f32) -> Color;     // RGB lerp; non-RGB passthrough
pub fn fade_style(style: Style, alpha: f32, canvas: Color) -> Style; // fg/bg blend toward canvas
```

`Motion` reduction contract in module docs + helper:
`effective_alpha(motion: Motion, animated: f32) -> f32` returning the static
fallback (1.0) when motion is reduced. Delete the private `fade_style` in
`status_bar.rs` and route it through the shared one (canvas =
`style(Role::Canvas).bg` fallback black). Keep `style::faded` working but
implement it via `blend_toward(color, BLACK, …)` and doc-link the new API.

**Verify**: unit tests — `pulse_brightness(0, 8) == 0.0` (approx),
`pulse_brightness(4, 8) == 1.0` (approx), wave periodicity
(`wave_brightness(t, r, w, s) == wave_brightness(t, r + w as-cycle …)` at
cycle boundary), `smoothstep(0.5) == 0.5`, `edge_fade` symmetric;
`cargo nextest run -p termrock motion` → pass.

### Step 2: Glyph catalog v2

In `style/glyph.rs`:

- Add glyphs (each with Unicode + ASCII fallback, following the existing
  `Glyph` pattern in the file): diamond family `◆ ◇ ◈` (fallbacks `* o #`),
  status dots `● ○ ◉ ◎` (`* o (*) (o)`→ pick 1-char fallbacks: `* o @ O`),
  chevrons if missing `› ‹` (`> <`), copy `⧉` (`&`— choose a sane 1-col
  ASCII; document choice), pulse frames `⋅ : ⸬ ⁙` (ASCII `. : :: ::`
  → must stay 1 col: use `. : + *`).
- Promote ramps (if not already via plan 004): block ramp + braille ramp as
  named constants here; charts and progress consume them.
- Spinner frames: make `SPINNER_BRAILLE_FRAMES` public API on the catalog
  (re-export from `widgets/spinner.rs` for continuity) and add
  `SPINNER_DOT_PULSE_FRAMES` (the `⋅ : ⸬ ⁙` tier) consumed by `Spinner` via
  a new variant/constructor (read `spinner.rs` builder API first and match
  it).
- **Width test**: a new test iterates every glyph in the catalog and asserts
  `unicode_width` display width == 1 (or == declared width for declared-wide
  glyphs), for both Unicode and ASCII fallbacks. This is the width-invariant
  law from Grok Build.

**Verify**: `cargo nextest run -p termrock glyph spinner` → pass incl. width
test.

### Step 3: `text::truncate_cols`

`pub fn truncate_cols(s: &str, max_cols: usize) -> Cow<'_, str>` —
display-column-correct (unicode-width based, reuse the existing
`display_cols` machinery in `crates/termrock/src/text/`), appends `…` (or
`...` under ASCII — take a `GlyphSet` or an `ellipsis: &str` param; pick the
simpler: `truncate_cols(s, max_cols, ellipsis)`), never splits a grapheme,
result display width ≤ max_cols. Property tests with wide glyphs (CJK),
combining marks, and exact-fit strings.

**Verify**: `cargo nextest run -p termrock text` → pass incl. new cases
(`"日本語", 5` → width ≤ 5 with ellipsis; exact fit → unchanged borrow).

### Step 4: Cell-run coalescing

`coalesce_cells(cells: impl Iterator<Item = (char, Style)>) -> Vec<(String, Style)>`
— merges adjacent equal-style cells into span runs (makes per-character
gradients affordable). Place next to the motion kit (`style/motion.rs`) or
`perf/` per module-doc fit; write one benchmark-shaped test (1000 cells, 3
style changes → 3 runs).

**Verify**: `cargo nextest run -p termrock coalesce` → pass.

### Step 5: Lookbook story + registry + gate

- Add a lookbook story "motion/presence" demonstrating pulse, wave along a
  vertical rail, edge fade on a long label, dot-pulse spinner (stories are
  deterministic — drive from the story's tick parameter, not wall clock;
  follow how existing spinner stories inject ticks).
- Regenerate whatever `mise run contracts` / registry checks require for new
  public API (run and follow errors).
- `migrations/0266-v0.13.0-motion-text-glyph-kit.md` — mostly additive;
  breaking bits: `status_bar.rs` private fade removed (internal), any
  signature change to `style::faded` (avoid — keep compatible). Link from
  `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0. Commit.

## Test plan

Named per step: motion math (4+), glyph width invariant (1 sweeping),
truncate properties (3+), coalesce (1+). Model on existing pure-function
tests in `style/` modules.

## Done criteria

- [x] `mise run check` + `mise run gate` exit 0
- [x] `grep -n "fn fade_style" crates/termrock/src/widgets/status_bar.rs` → 0 matches (moved to shared kit)
- [x] Glyph width test sweeps the whole catalog and passes
- [x] `truncate_cols` exists in `crates/termrock/src/text/` with passing property tests
- [x] Motion story renders deterministically in lookbook
- [x] `migrations/0266-*.md` exists, linked
- [x] `plans/README.md` updated

## Visual QA

- Spinner and deterministic motion/presence story: **pass** across dark/paper
  and mobile, tablet, and desktop viewports; keyboard interaction, console,
  requests, and horizontal overflow all clean.
- Designer-eye verdict: **pass** — shared ramps and restrained dot-pulse cues
  add presence without competing with the primary status hierarchy.
- Evidence: [`artifacts/visual-qa/plan-006/README.md`](../artifacts/visual-qa/plan-006/README.md).

## STOP conditions

- `FrameTick`/`Motion` vocabulary differs materially from what this plan
  assumes (no tick counter available to stories) → report the actual shape.
- Lookbook determinism check fails because motion helpers got wired to wall
  clock anywhere → fix by threading ticks; if impossible, STOP.
- Adding glyphs breaks `GlyphSet::Enhanced`/`Ascii` resolution exhaustiveness
  in ways that require redesigning `Glyph` — report first.

## Maintenance notes

- Plans 007–009 consume every helper here; keep signatures stable through
  the series (coordinate edits via plans/README if a later plan needs a
  change).
- Reviewers: verify no `Instant::now()`/wall-clock sneaks into paint paths —
  determinism is a repo-level contract for previews.

## Amendments

- 2026-08-12: Added generated public API/frame output and agent-browser QA
  artifacts omitted from Scope. The new kit is public and its deterministic
  story is docs-visible; repo catalog law and the standing responsive browser
  gate require these direct outputs in the same green commit.
- 2026-08-12: Added generated Spinner component MDX after the docs gate found
  it stale. This is a plan defect: the goal requires public component coverage
  and repo cross-surface law requires docs to track story/API changes. Options
  were suppressing the generator check, omitting the story, or regenerating the
  canonical page; regeneration best matches the goal/design authorities, has
  the smallest blast radius, and keeps the commit independently green.
