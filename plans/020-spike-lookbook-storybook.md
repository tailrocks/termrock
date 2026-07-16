# Plan 020 (spike): Grow the lookbook into a terminal storybook — knobs, theme switching, installable gallery

> **Executor instructions**: DESIGN SPIKE with a thin working slice. Deliverable
> = design doc + one story running with interactive knobs + recommendation.
> Honor STOP conditions. Update the plans/README.md row when done.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock-lookbook/`
> Plans 010 (slate preset) and 018 (runner prototype rewrote run_terminal)
> should be DONE first — check their rows; this spike builds on both.

> **Reconcile note (2026-07-16, round 3, HEAD `5c4758b`)**: lookbook internals
> evolved heavily since planning (stories/interactors/svg/main reshaped by the
> 001-015 wave; 20 widgets now incl. Progress/LogPane). The approach stands;
> treat all "Current state" excerpts as leads and re-read the live files. Plan
> 022 (SVG fix) should land first so knob-driven re-renders inherit visible
> colors.

## Status

- **Priority**: P3
- **Effort**: M (coarse — spike scope)
- **Risk**: LOW (dev tooling; the SVG determinism gate protects the catalog)
- **Depends on**: plans/010-second-theme-preset.md, plans/018-spike-runtime-disposition-app-runner.md
- **Category**: direction
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

TermRock is explicitly shadcn-inspired and already has the storybook skeleton: 18 typed stories with metadata, interactive interactors, a deterministic SVG preview pipeline gated in CI, and a TUI gallery. What it lacks are the three features that make a storybook a design/QA surface people actually adopt: **knobs** (vary a component's props at runtime instead of one frozen variant per story), a **theme switcher** (now meaningful — two presets exist post-010), and **installability** (today it's a workspace-internal dev crate; an outside team evaluating TermRock can't run the gallery against their own theme). All three are adjacent-possible on existing infrastructure.

## Current state

- `crates/termrock-lookbook/src/lib.rs` — hardcoded 18-entry `StoryMetadata` list (ids like `text-input/filter` titled "Filter composition"); `stories.rs` — 18 `Story::new(...)` fixed-render functions (~20 KB); `interactors.rs` (~14 KB) — per-widget interactive drivers; `svg.rs` — deterministic SVG writer; `main.rs` — subcommands `render --out`, `check --dir`, `list --format json` + the interactive gallery loop (post-018: runner-based).
- Story constructions are fixed: e.g. stories construct `Theme::default()` internally with fixed rows/labels — no parameterization hook exists.
- CI integration: `docs/scripts/check-catalog.ts` consumes `list --format json` (id + component per story) and cross-checks against `public-api.txt` + `component-contracts.json` + previews; `docs.yml` renders twice + `diff -r` for determinism. ANY knob/theme feature must keep `render`/`check` deterministic and the `list` JSON schema stable (or update check-catalog.ts in the same commit).
- Post-010: `--theme <phosphor|slate>` exists on `render`; `Theme::slate()` is public.
- The crate is `publish`-less workspace member; consumers pin the `termrock` crate by git rev (README) — the same mechanism could `cargo install --git ... termrock-lookbook`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Gallery | `cargo run -p termrock-lookbook` | interactive gallery |
| Determinism | `cargo run -p termrock-lookbook -- render --out target/r-a && cargo run -p termrock-lookbook -- render --out target/r-b && diff -r target/r-a target/r-b` | no diff |
| Catalog gate | `cd docs && bun run build` | exit 0 |
| Tests | `cargo test --workspace --all-features --locked` | all pass |

## Scope

**In scope (spike)**:
- Design doc `plans/020-storybook-design.md`
- Thin slice: knob support for ONE story (recommend `toast/basic`-class story — severity enum + anchor enum + message string = three knob types) + the global theme toggle in the gallery
- Installability assessment (no publishing action)

**Out of scope**:
- Knobifying all 18 stories (build plan).
- Web/registry distribution of the gallery (AGENTS.md keeps future registry ideas from constraining today's crate).
- Changing the SVG golden set semantics.

## Steps

### Step 1: Design the knob model

In the doc, define a `Knob` descriptor that stories declare and the gallery renders as a control column. Sketch:

```rust
pub enum KnobValue { Bool(bool), Choice(usize), Text(String), Number(i64) }
pub struct Knob { pub id: &'static str, pub label: &'static str,
                  pub value: KnobValue, pub choices: &'static [&'static str] /* for Choice */ }
```

Story render signature evolves from `fn(&Theme, area, &mut Buffer)`-shaped to also receiving `&[Knob]` (read the ACTUAL current `Story` type in `stories.rs` first and evolve from it — forward-only, don't wrap). Constraints to answer: knobs must have deterministic defaults (the `render`/`check` pipeline uses defaults ⇒ goldens unchanged); the knob panel itself should be built from TermRock widgets (dogfooding: `Form` or `List` + `TextInput` — note which gaps this exposes; that friction list is a spike deliverable, it feeds Plan 021-class decisions).

### Step 2: Build the thin slice

Implement knobs for the chosen story: control column in the gallery (toggle focus between story pane and knob pane — the post-018 runner's focus routing shows how), Choice knobs cycle with arrows, Text knob edits via `TextInputState`, story re-renders live per change. Add the global theme toggle (`t` key: phosphor ⇄ slate) applying to story + gallery chrome.

**Verify**: manual checklist (change severity knob → toast restyles; toggle theme → whole gallery restyles); determinism command → no diff (defaults untouched); `cd docs && bun run build` → exit 0 (list JSON unchanged or check-catalog updated in same commit); workspace tests green.

### Step 3: Installability assessment

Answer in the doc: does `cargo install --git https://github.com/tailrocks/termrock termrock-lookbook` work today (try it in a temp CARGO_HOME if network allows; otherwise reason from the manifest — workspace deps with `path`? `termrock` dep is a path dep inside the workspace, which install resolves fine from git)? What would a themed external run look like (`--theme` flag exists; a `--theme-file <toml>`? — only sketch, ties into Plan 019's serde direction). Binary name collision/rename question (`termrock-lookbook` vs `termrock lookbook` cargo alias).

### Step 4: Design doc + build-plan stub

`plans/020-storybook-design.md`: knob model, story-signature migration table (all 18 stories), theme-switcher interaction spec, gallery layout sketch (text diagram), the dogfooding friction list from Step 2, installability answer, effort estimate for full rollout, open questions (knob persistence? per-story vs global theme?).

**Verify**: doc exists; README row updated.

## Done criteria

- [ ] One story drives real knobs in the gallery; theme toggle works globally
- [ ] SVG determinism + catalog gate + workspace tests all green
- [ ] `plans/020-storybook-design.md` complete incl. friction list + installability verdict
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plan 018's runner rewrite hasn't landed and `run_terminal` is still the 490-line original — building knob focus-routing on top of it doubles the mess; stop, do 018 first.
- Keeping `list --format json` stable proves impossible with the evolved Story type — update `check-catalog.ts` in the same commit ONLY if the change is additive fields; otherwise report.

## Maintenance notes

- The dogfooding friction list is first-class output — every awkwardness building the knob panel out of TermRock widgets is a library gap with a real reproduction.
- Full knob rollout across 18 stories is the follow-up build plan; keep per-story knob declarations next to the story fns so the catalog check can eventually require them.
