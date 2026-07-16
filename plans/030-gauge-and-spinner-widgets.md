# Plan 030 (rewritten 2026-07-16, round 3): Complete `Progress` — custom spinner frames, the mandated test matrix, axis-proving stories

> **SUPERSESSION NOTE**: The original Plan 030 specified separate `Gauge` and
> `Spinner` widgets. Commit `b5928dc` shipped `widgets/progress.rs` — one
> `Progress` widget with `ProgressKind::{Determinate { fraction }, Indeterminate { tick }}`
> — delivering both capabilities plus the caller-clocked model (story, preview,
> contract row included). The single-widget shape is accepted (forward-only
> unification). This rewrite covers ONLY the residual gaps.
>
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. STOP
> conditions are binding. Update the plans/README.md row when done.
>
> **Drift check (run first)**: `git diff --stat 5c4758b..HEAD -- crates/termrock/src/widgets/progress.rs crates/termrock-lookbook/src/stories.rs docs/api/component-contracts.json`
> Concurrent executors active — re-locate by symbol.

## Status

- **Priority**: P2
- **Effort**: S-M
- **Risk**: LOW
- **Depends on**: none
- **Category**: tests
- **Planned at**: commit `5c4758b`, 2026-07-16

## Why this matters

`Progress` shipped with 2 tests against a planned matrix of ~13, and the widget's defining accessibility property — the **glyph-based** non-color cue (`█` filled vs `░` empty) — is never actually asserted (the existing test checks `█` is present but never that the empty zone differs by glyph rather than color alone). Untested: 0%, the exact 50% cell split, width 0/1 guards, explicit NaN, wide-char label truncation through the display-width path (`progress.rs` uses `display_cols` at ~:51/:66 — the untested branch). The braille spinner frame set is hardcoded (8 frames, ~:104) with no custom-frames option and no empty-frames guard, blocking ASCII-only or branded spinners. And the contract row claims `unicode: covered` + `narrowTerminal: covered` while the single story renders ASCII labels at one generous size — self-attested axes, the exact anti-pattern the catalog exists to prevent.

## Current state

- `crates/termrock/src/widgets/progress.rs`: `ProgressKind::Determinate { fraction: f64 }` ("rendering clamps finite values to `0.0..=1.0`") and `Indeterminate { tick: u64 }` ("Caller-owned deterministic animation tick"); hardcoded braille frames (~:104); `is_finite` guard (~:38-95 render body); label rendering via `display_cols` helpers; **2 tests** (~:120-164): determinate clamp/cue-presence, indeterminate determinism + 0-size. Doc stubs on the variants (plan 036 fixes wording — don't collide: this plan touches code+tests, 036 touches doc comments; coordinate via drift-check if both run).
- Story: `progress/determinate` in `crates/termrock-lookbook/src/stories.rs` (renders both kinds, ASCII labels, one size). Preview: `docs/public/component-previews/progress-determinate.svg`.
- Contract row: `Progress` claims all axes covered.
- Test exemplar: buffer-assertion style in `widgets/tests.rs` and `progress.rs`'s own two tests.
- Conventions: additive API (custom frames) still needs `public-api.txt` regen; `missing_docs = deny` — real docs on new methods; story changes re-render previews; Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Widget tests | `cargo test -p termrock progress` | all pass |
| Previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews && cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Catalog gate | `cd docs && bun run build` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `progress.rs`, lookbook `stories.rs` (+interactors only if a story needs one), previews, `component-contracts.json` only if an axis must be downgraded, `public-api.txt` regen.

**Out of scope**: frame-clock integration (Plan 031 — `tick` stays caller-owned); embedding progress in List/Tree rows; renaming `Progress`/`ProgressKind`.

## Git workflow

- Directly on `main`; `git commit -s -m "test(progress): complete the coverage matrix and prove contract axes"` (+ `feat(progress): custom spinner frames` if split).

## Steps

### Step 1: Custom spinner frames

Add a frames override to the post-013 construction idiom (read the current `Progress` construction shape first — 013 landed after the widget; align with whatever `new`/builder form it now has): `frames(&'a [&'a str])` builder defaulting to the braille set; empty-slice guard renders nothing (documented + tested); frame index = `tick % frames.len()` (guard the modulo against the empty case before it).

**Verify**: `cargo check --workspace --all-features --locked` → exit 0.

### Step 2: The test matrix

Add buffer-assertion tests (names indicative): `zero_fraction_renders_all_empty_glyphs`, `half_fraction_splits_cells_exactly` (even width → exact split; assert the boundary cells), `full_fraction_renders_all_filled`, `nan_and_infinite_clamp_to_zero` (explicit NaN + ∞ + -∞), `width_zero_and_one_do_not_panic`, `filled_and_empty_zones_differ_by_glyph` (THE non-color cue assertion: symbols differ, not merely styles), `wide_char_label_truncates_on_grapheme_boundary` (CJK/emoji label in a narrow bar; assert no broken cell), `custom_frames_cycle_and_wrap`, `empty_frames_render_nothing`.

**Verify**: `cargo test -p termrock progress` → ≥11 tests pass total.

### Step 3: Prove the claimed axes with stories

Add `progress/narrow` (width ~14: bar + spinner squeezed; percentage elision behavior pinned) and extend the main story (or add `progress/unicode`) with a wide-char label. Re-render previews. If any axis still lacks demonstration after this, downgrade that contract-row value instead of leaving the claim.

**Verify**: previews deterministic + visible; `cd docs && bun run build` → exit 0 (new story ids mentioned in components.mdx per the gate); `mise run gate` → exit 0.

## Done criteria

- [ ] Custom frames + empty guard shipped, documented (non-stub), tested
- [ ] ≥9 new tests incl. the glyph-difference assertion and wide-char truncation
- [ ] Narrow + unicode story coverage committed (or contract row honestly downgraded)
- [ ] `public-api.txt` regenerated; `mise run gate` → exit 0
- [ ] `plans/README.md` row updated

## STOP conditions

- The 50%-split test reveals off-by-one rounding that changes the shipped preview — that's a real bug: pin the CORRECT behavior (round-half-up documented), fix the render math, note the preview diff in the commit.
- Braille glyphs measure 0-width through the crate's helpers (would corrupt layout) — report; switch default frames to ASCII `|/-\` and document.

## Maintenance notes

- Plan 031's frame clock adopts `Indeterminate.tick` as its first client — API here is already clock-agnostic; no change expected.
- Plan 023's axis-story gate (when it lands) will enforce what Step 3 does manually — this plan front-runs it for Progress.
