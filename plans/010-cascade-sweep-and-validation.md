# Plan 010: Cascade sweep — remaining widgets, consistency audit, success-criteria enforcement

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: this plan runs **after** 001–009; drift from
> them is the point. Re-derive the work lists with the greps in Step 1 —
> never from stale lists.

## Status

- **Priority**: P3
- **Effort**: L
- **Risk**: LOW–MED (broad but mechanical; each item follows a proven pattern)
- **Depends on**: plans/001–009 (all)
- **Category**: tech-debt (consistency law)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

Repo law: "Inconsistency is a defect. A local win that invents a second way
to do the same terminal job is incomplete work." Plans 001–009 proved the
patterns on the highest-value surfaces; this plan sweeps the remaining ~100
widgets and the patterns tree onto them, then turns the series' success
criteria into repeatable checks so the library cannot silently regress into
the old flat look.

## Current state

At `539e7d03` the box-literal carriers beyond plans 005/009 were:
`completion_menu.rs` (005), `callout.rs`, `image_surface.rs`,
`fullscreen_viewer.rs`, `preview_card.rs`, `log_pane.rs`,
`notification_center.rs` (005), `tests.rs`, `toast.rs` (005) — plus any the
greps in Step 1 reveal now. Widget-count claims are re-derived, not assumed.

Authorities established by prior plans (the sweep targets route through
them):

- Fill/border/pad shells → `Surface` (+ `BorderShape` token).
- Row styling → `resolve_list_row` / `ListRowRecipe`.
- Controls → `button_recipe` / `input_recipe`.
- Labeled rows → `FieldRow`; block accents → `AccentRail`; stacked panels →
  `panel_stack`; hints → HintBar v2.
- Motion → `style/motion.rs`; glyphs → catalog v2; truncation →
  `text::truncate_cols`.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Contracts | `mise run contracts` | exit 0 |
| Docs quality | `mise run docs-quality` | exit 0 |

## Scope

**In scope**: all remaining `crates/termrock/src/widgets/*.rs` and
`crates/termrock/src/patterns/*.rs` not upgraded by 004/005/008/009;
lookbook stories and harness; `crates/termrock/src/lib.rs` policy tests;
`migrations/0279-*.md` + `MIGRATING.md`; generated catalog/API/frame outputs;
responsive `agent-browser` evidence under `artifacts/visual-qa/plan-010/`;
`plans/README.md`.

**Out of scope**: kernel modules (`interaction/`, `runtime/`, `keymap/`,
`input/`), `termrock-cli`, hand-authored docs-site features. Generated docs
outputs and browser validation remain required cross-surface evidence.

## Git workflow

`feat/component-visual-richness-series`, `git commit -s`, commit per coherent
cluster. Each
commit independently green under `mise run check` (repo law).

## Steps

### Step 1: Derive the work lists (fresh greps)

```
grep -rln '"┌"\|"╭"' crates/termrock/src/widgets crates/termrock/src/patterns   # shells to Surface
grep -rln "bg = None" crates/termrock/src                                        # must already be 0; fix any stragglers
grep -rLn "resolve_list_row" $(grep -rln "RowRole\|selected" crates/termrock/src/widgets) # rowed widgets off-recipe (triage manually)
grep -rn "fn .*truncat\|chars().take(" crates/termrock/src/widgets              # local truncation to replace with truncate_cols
grep -rn "░\|▁▂▃▄▅▆▇" crates/termrock/src/widgets | grep -v glyph               # ramp/track drift outside the catalog
```

Record the resulting lists in the PR/commit description; they are the sweep
checklist.

**Verify**: lists produced; count reported.

### Step 2: Sweep shells and rows

For each file on the lists, apply the matching authority (Surface shell /
recipe rows / catalog glyphs / truncate_cols), following the exact patterns
from plans 004/005/009 (read one already-migrated widget of the same shape
first — e.g. `toast.rs` for shells, `data_table.rs` for rows — and match
it). Mechanical rule: no new visual decisions in this plan; anything that
needs a decision goes to the report, not into code.

**Verify**: after each cluster commit: `mise run check` → 0; re-run the
Step 1 greps → shrinking lists.

### Step 3: Patterns cascade

`crates/termrock/src/patterns/*.rs` (36 composites): compose the upgraded
widgets; delete any pattern-local chrome that now duplicates a widget
authority (patterns must not carry forked paint — boundary law). Update
every pattern's lookbook story.

**Verify**: `cargo nextest run -p termrock patterns` (or the crate's pattern
test filter) → pass; `mise run contracts` → 0.

### Step 4: Success-criteria enforcement tests

Add a `crates/termrock/src/widgets/tests.rs` (or extend it) policy test
module that enforces the series' end state mechanically where possible:

- `no_bg_none_policy`: compile-time-adjacent guard is impossible; instead
  add a repo check: extend an existing policy test pattern (see
  `lib.rs` `root_export_policy` tests, verified at `539e7d03`
  `crates/termrock/src/lib.rs:37-44`, which grep-style pin repo law) with a
  test that reads the widgets source dir at test time and asserts zero
  `bg = None` matches and zero `"┌"` literals outside `surface.rs`/glyph
  catalog. Follow the existing policy-test implementation style in
  `lib.rs`.
- `every_preset_survives_capability_ladder`: for each preset × each
  `ColorCapability` (+ `NO_COLOR` mono + `GlyphSet::Ascii`), render a
  representative story set and assert non-panic + non-empty distinct output
  (leverage existing lookbook test harness — see
  `crates/termrock-lookbook/src/tests.rs`).

**Verify**: policy tests pass; deliberately reintroducing a `bg = None` in a
scratch build fails the policy test (manual spot check, then revert).

### Step 5: Consistency audit vs contract matrix

Run `mise run contracts` + `mise run docs-quality`; walk the generated
contract matrix for every public widget: preview present, states story
present, mono/narrow/unicode coverage rows true. Fix gaps (stories or
matrix rows). This is the repo's own cross-surface consistency law applied
end-to-end.

**Verify**: both tasks exit 0; no matrix row regressions vs `main` before
this plan (diff the generated matrix file if it is checked in).

### Step 6: Final migration + gate

`migrations/0270-v0.13.0-visual-cascade-completion.md` (next free number):
records the sweep completion, the policy tests as the new invariant, and a
consolidated consumer-facing summary of the whole 0261–0270 visual series
(one table: surface → what changed → opt-out). Link from `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0. Push per repo law.

## Test plan

Policy tests (Step 4), capability-ladder sweep test, plus per-widget
expectation updates. No new visual-decision tests — decisions belong to
001–009.

## Done criteria

- [ ] `mise run check` + `mise run gate` + `mise run contracts` + `mise run docs-quality` exit 0
- [ ] `grep -rn "bg = None" crates/termrock/src` → 0
- [ ] `grep -rln '"┌"' crates/termrock/src/widgets crates/termrock/src/patterns` → only `surface.rs` / glyph catalog (and `tests.rs` fixtures if any — justify each)
- [ ] Policy tests exist and pass
- [ ] Contract matrix complete for all public widgets
- [ ] `migrations/0270-*.md` exists, linked
- [ ] `plans/README.md` updated — series complete

## STOP conditions

- A sweep item genuinely cannot use an authority without a new visual
  decision → list it in the report with the decision needed; do not decide.
- Policy test approach conflicts with how `lib.rs` policy tests are actually
  built (they may be AST/grep-shaped differently) → match the existing
  mechanism; if none fits, report.
- The patterns cascade uncovers a widgets↔patterns boundary violation
  predating this series → report separately; don't fix ad hoc.

## Maintenance notes

- The policy tests are the durable enforcement — future contributors get an
  immediate red test instead of silent visual drift.
- Follow-up (deferred, own decision): docs-site live previews of the new
  system (`docs/design/docs-site-terminal-experience-plan.md`) and the
  Jackin adoption pass (consumer-side, tracked in Jackin's repo — the
  workaround-deletion checklist lives in
  `docs/design/component-visual-richness-plan.md` §9).

## Amendments

- 2026-08-12: Added `lib.rs`, lookbook harness, generated outputs, and
  responsive `agent-browser` evidence to Scope. Step 4 explicitly requires
  repo policy tests and lookbook capability rendering, while repo
  cross-surface law and standing browser rules require generated/docs-visible
  proof; their omission was a plan defect.
- 2026-08-12: Replaced the stale `main` workflow with the operator-required
  single `feat/component-visual-richness-series` branch and one PR. This is a
  direct instruction override; DCO, independent green commits, and immediate
  push remain unchanged.
- 2026-08-12: Allocated migration 0279 because Plans 007–009 truthfully used
  0267–0278 across independently visible boundaries. Rewriting immutable
  history would violate repo migration law.
- 2026-08-12: Scoped `bg = None` enforcement to widget/pattern paint sources.
  The sole repo-wide match is `ansi_text::filter_style`, which must erase ANSI
  color under `NO_COLOR`; forbidding it conflicts with accessibility and the
  design SoT. Options were weakening NO_COLOR, string-exempting that line, or
  testing the intended widget boundary. Boundary-scoped policy best advances
  the goal without sacrificing capability sanitation.
