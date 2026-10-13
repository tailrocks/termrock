# Plan 037: Align the List/Tree multi-select contract — one outcome shape, one state-visibility model

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5c4758b..HEAD -- crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/tree.rs crates/termrock/src/widgets/selection.rs`
> Concurrent executors active — re-locate excerpts by symbol; STOP on missing
> symbols only.

## Status

- **Priority**: P2
- **Effort**: S-M
- **Risk**: LOW-MED (breaking: ListState field privacy + a List outcome variant; forward-only policy sanctions it)
- **Depends on**: none (013 landed)
- **Category**: tech-debt
- **Planned at**: commit `5c4758b`, 2026-07-16

## Why this matters

The multi-select feature (migration 0011) landed with two sibling widgets disagreeing twice on the same concept. Visibility: `TreeState` keeps `selection`/`check_regions` **private** behind `selection()`/`selection_mut()` accessors, while `ListState` exposes every field `pub` (including `selection: Option<Selection<Id>>` and the per-frame `check_regions: Vec<HitRegion<Id>>` hit-geometry cache) *and* carries the same redundant accessor pair — two mutation paths, and consumers can corrupt frame-internal geometry. Outcome shape: toggling a checkbox returns `TreeOutcome::CheckToggled(id)` on Tree but bare `Outcome::Changed` (no id) on List, forcing List consumers to re-scan `selection().checked()` after every toggle. Migration 0011 documents the split, which makes it deliberate-looking — but a widget family whose identical gesture reports differently per widget fails the "learn one convention" bar the event-model convergence (migration 0009) established.

## Current state

- `crates/termrock/src/widgets/list.rs` — `ListState` (all fields `pub`; placeholder docs — plan 036 fixes wording, THIS plan fixes visibility):

```rust
pub struct ListState<Id> {
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
    pub focused: bool,
    pub offset: usize,
    pub viewport_height: usize,
    pub regions: Vec<HitRegion<Id>>,
    pub selection: Option<Selection<Id>>,
    pub check_regions: Vec<HitRegion<Id>>,
}
```

  plus `selection()`/`selection_mut()` accessors, and a `toggle_selected` returning `Outcome::Changed` (locate by symbol; the check-region click branch likewise returns `Changed`).
- `crates/termrock/src/widgets/tree.rs` — the exemplar: `check_regions` private (`tree.rs:79`), `pub const fn selection(&self)` / `pub fn selection_mut(&mut self)` (`tree.rs:167-172`), space key → `toggle_selected` → `TreeOutcome::CheckToggled(node.id.clone())` (`tree.rs:236-253`), check-region click → `CheckToggled(id)` (`tree.rs:276-285`).
- `crates/termrock/src/widgets/selection.rs` — `Selection<Id>` ordered check-set (`toggle(&Id) -> bool`, `checked() -> &[Id]`, `is_checked`, `clear`); sound, untouched by this plan.
- `interaction::Outcome<T>` is `#[non_exhaustive]` post-013 (verify) — a variant CAN be added.
- Consumers to fix compiler-guided: lookbook stories/interactors, `widgets/tests.rs`, characterization tests (`tests/form.rs` pattern files may construct `ListState { .. }` literals — post-013 they should already use methods; verify).
- `migrations/0011-v0.10.0-trailing-metadata-cells.md` documents the current split — this plan's migration file supersedes that aspect (never edit 0011 itself; historical).
- Conventions: breaking → next-numbered migration + MIGRATING row; regenerate `public-api.txt`; Conventional Commits + DCO; `missing_docs = deny` (real doc comments on any new accessor).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Fast check | `mise run check` | exit 0 |
| List/Tree tests | `cargo test -p termrock list` / `cargo test -p termrock tree` | all pass |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0, zero diffs |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/widgets/list.rs` (state visibility + outcome)
- `crates/termrock/src/widgets/tree.rs` ONLY if Step 2's decision requires touching it (target: it shouldn't — Tree is the exemplar)
- Compiler-flagged call sites (lookbook, tests)
- `migrations/00NN-*.md` + `MIGRATING.md`; `public-api.txt` regen

**Out of scope**:
- `Selection<Id>` internals.
- The naming-collision cleanup (`Selection` multi-check vs `ListState.selected` cursor vs selection-following scroll — three meanings of "select") — recorded as an open naming question for a future pass; renaming `selected`→`cursor` is too churn-heavy to ride this plan.
- DetailTable (no multi-select there).

## Git workflow

- Directly on `main`; `git commit -s -m "refactor(list)!: align multi-select contract with tree"`; migration file same commit.

## Steps

### Step 1: ListState visibility parity

Make `ListState`'s fields private; keep/add the accessor surface Tree has, PLUS what List consumers legitimately need (survey call sites first: `selected`/`select(Option<Id>)` exist as methods already — check; `hovered()`, `focused`/`set_focused`, `offset()` read-only if any test needs it; `regions()`/`field-regions` read-only slices). `check_regions` gets NO public accessor (frame-internal; Tree exposes none — confirm). `selection()`/`selection_mut()` stay as the single mutation path.

**Verify**: `cargo test --workspace --all-features --locked` → all pass after fixing flagged sites; previews zero-diff.

### Step 2: One outcome for the check gesture

Align on Tree's shape (id-carrying — the information-preserving choice, consistent with `Activated(Id)`): add `Outcome::` variant? NO — `Outcome<T>` is the shared generic enum; adding `CheckToggled(T)` to it imposes the variant on every widget using `Outcome`. Decide by reading how List's outcomes are typed post-011: List returns `interaction::Outcome<Id>`. Options: (a) add `CheckToggled(T)` to `interaction::Outcome` (non_exhaustive, consumers with `_` arms unaffected; semantically fine — any checkable widget reuses it); (b) give List its own `ListOutcome` again (regression against 011's consolidation — reject). Choose (a). List's `toggle_selected` and check-click branch return `Outcome::CheckToggled(id)`; Tree keeps `TreeOutcome::CheckToggled(id)` unchanged.

**Verify**: `cargo test -p termrock list` — update the multi-select tests to assert the id-carrying outcome; full suite green.

### Step 3: Tests + migration

Add/extend tests: `list_check_toggle_reports_id` (keyboard space + check-region click both yield `CheckToggled(expected_id)`), `list_state_geometry_not_publicly_mutable` (compile-time by construction — instead assert accessor behavior: `selection_mut` toggles reflect in `selection().checked()`). Migration file: `ListState` fields now private (before/after literal→methods example), `Outcome` gained `CheckToggled(T)`, List check gesture now id-carrying; note it supersedes migration 0011's "List reports `Changed`" contract. Regenerate `public-api.txt`.

**Verify**: `mise run gate` → exit 0; migration indexed.

## Test plan

Step 3's two List tests + existing list/tree suites + preview zero-diff (no render change intended).

## Done criteria

- [x] `grep -n "pub selected\|pub selection\|pub check_regions" crates/termrock/src/widgets/list.rs` → no matches
- [x] List and Tree both report an id-carrying check-toggle outcome; tests assert it
- [x] Previews byte-identical; full gate green
- [x] Migration indexed; `public-api.txt` regenerated
- [x] `plans/README.md` status row updated (naming-collision question recorded as open)

## STOP conditions

- Call-site survey reveals a consumer pattern that NEEDS direct field mutation which no reasonable accessor covers — report the pattern before inventing setters ad hoc.
- `interaction::Outcome` turns out NOT to be non_exhaustive (013 gap) — adding the variant is then a wider break; still correct under forward-only, but note it in the migration and report the 013 gap.
- A concurrent executor already aligned these (check `git log --oneline -10` for a list/tree contract commit) — reconcile scope with what remains.

## Maintenance notes

- Rule this establishes: sibling widgets expose the SAME gesture with the SAME outcome shape and the SAME state-visibility model — reviewers should treat divergence as a defect (this is the second occurrence; migration 0009 fixed the first family-wide).
- The select-vocabulary naming question (`selected` cursor vs `Selection` checks) stays open in plans/README.md — worth resolving before Table (Plan 033) adds a third selection consumer.
