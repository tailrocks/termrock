# Plan 030: Build the progress family — determinate `Gauge`, caller-clocked `Spinner`

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/widgets/`
> Written against the POST-008/013 API (themed, new-idiom construction).
> Verify those rows; design against LIVE signatures.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW (additive widgets)
- **Depends on**: plans/008 (theme roles), plans/013 (construction idiom); pairs with plans/031 (FrameClock spike — Spinner's frame source; NOT a blocker, see Step 3)
- **Category**: direction
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

The library has no way to show progress. The only artifact is `TreeNodeStatus::Loading`, which renders the literal string " loading" on a tree row — no determinate bar, no indeterminate activity indicator, anywhere (verified: no progress/gauge/spinner/meter symbol in the crate or story catalog). Long-running operations are the bread and butter of this ecosystem's consumers. A `Gauge` (determinate fraction) and `Spinner` (indeterminate, caller-advanced frame) are standard component-library staples, cheap on the existing foundations, and force zero new infrastructure if the spinner's clock stays caller-owned (Plan 031 spikes the shared clock separately).

## Current state

- `crates/termrock/src/widgets/tree.rs:19` — `TreeNodeStatus::Loading`; ~:412 renders `" loading"`. That's the entire progress story today.
- Theme system (post-008): `theme: &Theme` + `Role` per widget; add roles if needed (`GaugeFilled`, `GaugeEmpty` — or reuse `Accent`/`TextMuted`; prefer REUSE first, new roles only if the lookbook proves the defaults wrong).
- Construction idiom (post-013): private fields, `const fn new(required)`, const builders, `#[non_exhaustive]` enums, owned+ref `Widget` impls.
- Non-color mandate (AGENTS.md: non-color cues are TermRock-owned): a gauge must not be color-only — filled cells use a distinct GLYPH (`█` vs `░`), and percentage text is available.
- Unicode-width discipline: label truncation must go through the crate's display-width helpers (`text::display_cols_slice` post-012, `crate::display_cols_slice` pre-012 — check live paths).
- New-widget catalog rule: contract row + story + preview + docs in the same change.
- Exemplar for a tiny render-only widget: `widgets/diff.rs` (50 lines) pre-024, or `widgets/panel.rs` for the builder shape.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews && cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Catalog gate | `cd docs && bun run build` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/widgets/{gauge.rs, spinner.rs}` (new) + `widgets/mod.rs` exports
- Lookbook stories (`gauge/fractions`, `spinner/frames` — deterministic: fixed fraction set / fixed frame index) + previews
- `docs/api/component-contracts.json` rows; `public-api.txt` regen; post-028 component-page entries

**Out of scope**:
- Any internal clock/timer (Plan 031). `Spinner` renders frame `n`; the caller advances `n`.
- Animated SVG previews (stills at fixed frames).
- ETA/rate computation (consumer domain — they own the numbers; Gauge renders a fraction + optional label string).
- Embedding gauges into Tree/List rows (future composition; keep the widgets standalone).

## Git workflow

- Directly on `main`; `git commit -s -m "feat(widgets): gauge and spinner progress family"` — additive, no migration file needed unless a `Role` addition lands (role additions are additive too; note them in the commit body).

## Steps

### Step 1: `Gauge`

```rust
pub struct Gauge<'a> { /* private: fraction: f32 (clamped 0..=1), label: Option<&'a str>, theme: &'a Theme, show_percent: bool */ }
impl<'a> Gauge<'a> {
    pub fn new(fraction: f32, theme: &'a Theme) -> Self;   // clamps; NaN -> 0.0 (document + test)
    pub const fn label(self, l: &'a str) -> Self;
    pub const fn percent(self, show: bool) -> Self;        // default true
}
```

Render (single row): filled cells `█` in the filled style, empty cells `░` in the empty style (glyph difference = the non-color cue), centered `label / NN%` overlaid with contrast-correct styles on both zones. Width math: `filled = (fraction * width).round()` with 0-width/1-width guards and the invariant `0 ≤ filled ≤ width`. Truncate labels via the display-width helpers (grapheme-safe).

Tests (buffer-assertion style): 0%, 50% (exact cell split), 100%, fraction > 1 clamps, NaN → 0, width 0/1 no panic, wide-char label truncation, label+percent overlay cell content, and the glyph (not just color) differs between zones.

**Verify**: ≥8 gauge tests green.

### Step 2: `Spinner`

```rust
pub struct Spinner<'a> { /* private: frames: &'a [&'a str] (default: braille set), frame: usize, label: Option<&'a str>, theme: &'a Theme */ }
impl<'a> Spinner<'a> {
    pub const fn new(theme: &'a Theme) -> Self;            // default braille frames ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏
    pub const fn frame(self, n: usize) -> Self;            // renders frames[n % frames.len()]
    pub const fn frames(self, custom: &'a [&'a str]) -> Self;
    pub const fn label(self, l: &'a str) -> Self;
}
```

Pure function of `frame` — deterministic by construction (stories/previews pin a frame). Guard: empty custom `frames` renders nothing (documented; test).

Tests: frame wrap-around (`frame(11)` on 10 frames == `frame(1)`), custom frames, empty-frames guard, label rendering, wide-glyph frame width handling.

**Verify**: ≥5 spinner tests green.

### Step 3: Frame-advancement contract note

Document on `Spinner::frame`: "Advance `n` from your event loop's tick — see `runtime` clock (Plan 031) once available; until then, any monotonic counter works (e.g. events received, or a 80–120ms timer)." This keeps 030 shippable before/without 031 and gives 031 its first validating client.

### Step 4: Stories, contracts, catalog

Stories: `gauge/fractions` (three gauges at 12%/50%/93% stacked — shows glyph cue + labels), `spinner/frames` (a row of spinners at frames 0..4 + one custom-frame variant). Contract rows: keyboard/mouse/focus `not-applicable`, nonColor `covered`, unicode `covered` (braille + wide-char label in story), narrowTerminal `covered` (add the narrow variant per Plan 023's gate). Previews + `public-api.txt` regen + (post-028) `component-docs.ts` entries in the same change.

**Verify**: previews deterministic + visible; `cd docs && bun run build` green; `mise run gate` green.

## Test plan

Steps 1–2 (~13 unit tests) + story goldens + gates. No hot-path test needed (O(width) single-row renders).

## Done criteria

- [ ] `Gauge`/`Spinner` exported, post-013 idiom, themed via roles (reused or added — noted)
- [ ] Non-color cue verified by a test asserting glyph difference between filled/empty
- [ ] ~13 unit tests green; stories/previews/contract rows/API report in the same change
- [ ] `mise run gate` → exit 0
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plans 008/013 not landed — stop, dependency.
- Reused roles look wrong in the story (e.g. `Accent` unreadable as a fill) and a new `Role` is needed — adding roles is fine (append-only per Plan 008's invariant) but note it and extend BOTH presets (phosphor + slate) in the same commit; if slate doesn't exist yet, phosphor only + a README note.
- Braille glyphs render 0-width under the crate's width helpers (would break layout math) — report; pick ASCII fallback frames `|/-\` as default instead and document why.

## Maintenance notes

- Plan 031's FrameClock retrofits `Spinner` as its first client — the `frame(n)` API is deliberately clock-agnostic so that retrofit is additive.
- A future `Gauge`-in-`ListRow`/`TreeNode` composition (per-row progress) should reuse this render body via a shared cell-painting helper — extract at that point.
- Toast auto-dismiss (the other timer client) is Plan 031's scope, not this family's.
