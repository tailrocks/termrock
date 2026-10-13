# Plan 039: Restore fail-safe approval and bounded virtual-grid interaction

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update this plan's status row in
> `plans/README.md` unless a reviewer says they maintain the index.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets/agent.rs crates/termrock/src/widgets/virtual_grid.rs crates/termrock-lookbook/src/stories.rs crates/termrock-lookbook/src/interactors.rs docs/api/component-contracts.json docs/api/public-api.txt docs/content/docs/components/approval-card.mdx crates/termrock/COMPONENTS.md MIGRATING.md migrations`
>
> If any in-scope file changed, compare the live code with the excerpts below.
> Any semantic mismatch is a STOP condition. Do not begin while unrelated
> source work is uncommitted; plans may coexist, source changes may not.

## Status

- **Priority**: P0
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: none
- **Category**: security, bug, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — fail-safe ApprovalCard + VirtualGrid bounds (migrations 0032–0033)

## Why this matters

`ApprovalCard` currently makes accidental approval the default, including for
high-risk actions. `VirtualGrid` paints and activates nonexistent rows, ignores
disabled-row policy, paints the whole viewport as selected for any range, and
loses resident stable IDs. These are contract failures in security-sensitive
and data-heavy primitives; no later experience work should build on them.

## Current state

- `crates/termrock/src/widgets/agent.rs:360-400` stores a public numeric
  `selected` index. `new()` selects index `0`, and Enter maps that to
  `AllowOnce`:

  ```rust
  pub struct ApprovalCardState {
      pub selected: usize,
  }

  pub const fn new() -> Self {
      Self { selected: 0 }
  }
  ```

- `crates/termrock/src/widgets/agent.rs:453-465` stops painting decisions when
  width runs out. State can therefore select an invisible decision.
- `crates/termrock/src/widgets/virtual_grid.rs:100-130` exposes `GridRow.enabled`,
  but interaction and rendering never read it.
- `crates/termrock/src/widgets/virtual_grid.rs:623-636` always returns
  `row_id: None` for keyboard cursor outcomes.
- `crates/termrock/src/widgets/virtual_grid.rs:789-859` paints all body slots
  even when `total_rows` is zero or smaller than the viewport. Those phantom
  rows receive hit regions.
- `crates/termrock/src/widgets/virtual_grid.rs:817-829` computes each painted
  cell's range against that same cell instead of against the cursor endpoint.
- `docs/api/component-contracts.json:10-17` incorrectly calls ApprovalCard
  narrow-terminal behavior `not-applicable`.
- At planned HEAD, `rtk proxy mise run check` fails `cargo fmt --check` across
  the committed experience layer. Direct all-target/all-feature clippy reports
  26 errors and one must-use warning (mostly needless borrows, collapsed-if
  suggestions, and an ignored FocusOutcome). The baseline is not executable
  until these committed bootstrap failures are fixed. The matching workspace
  test run is green: 388 tests across 18 suites.
- Widget state owns only domain-neutral interaction state. Consumers retain
  permission policy and effects. Keep that boundary: TermRock may make safe
  presentation/input defaults, but it must never execute an approval.
- Stable IDs and borrowed projections are core conventions. Follow `ListState`
  in `crates/termrock/src/widgets/list.rs` for selection reconciliation and
  typed neutral outcomes.
- Every public break needs the next migration file and `MIGRATING.md` entry.
  Migration `0031` is committed; this plan owns `0032`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| RTK version | `rtk --version` | version printed |
| RTK path | `rtk proxy which rtk` | executable path printed |
| Approval tests | `rtk cargo test -p termrock approval --all-features --locked` | exit 0 |
| Grid tests | `rtk cargo test -p termrock virtual_grid --all-features --locked` | exit 0 |
| Catalog | `rtk cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0; previews fresh |
| Format | `rtk cargo fmt --all -- --check` | exit 0 |
| Clippy | `rtk cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Fast gate | `rtk proxy mise run check` | exit 0; fmt, clippy, nextest green |
| Full gate | `rtk proxy mise run gate` | exit 0 |

## Scope

**In scope**:

- Mechanical `cargo fmt` output across the committed 0029–0031 experience
  layer, plus the minimal lint-correctness edits reported by the documented
  clippy command. No broad `allow` attributes.
- `crates/termrock/src/widgets/agent.rs`
- `crates/termrock/src/widgets/virtual_grid.rs`
- focused tests beside those modules; one new hot-path test under
  `crates/termrock/tests/` if needed for the grid projection solver
- `crates/termrock-lookbook/src/stories.rs`
- `crates/termrock-lookbook/src/interactors.rs`
- `docs/api/component-contracts.json`
- generated catalog/docs artifacts required by the repository gate
- `docs/content/docs/components/approval-card.mdx`
- `crates/termrock/COMPONENTS.md`
- `migrations/0032-v0.12.0-safe-interaction-baseline.md`
- `MIGRATING.md`
- `plans/README.md` status only

**Out of scope**:

- Permission execution, allowlists, process policy, secrets, or persistence.
- Transcript redesign; that is Plan 041.
- Global interaction-scene redesign; that is Plan 040.
- Compatibility aliases for old approval state/outcomes.
- Semantic cleanup outside the exact formatter/clippy findings and the two P0
  widget contracts.

## Git workflow

- Binding repository rule: work directly on `main`; no feature branch or PR.
  If the operator has not reconciled the current feature branch onto `main`,
  STOP rather than switching or moving their work.
- Conventional Commit, DCO sign-off via `rtk git commit -s`.
- Add `Co-authored-by: Codex <codex@openai.com>`.
- Each commit must pass `rtk proxy mise run check`; push `main` only after `rtk proxy mise run gate`.
- First land one independently green mechanical bootstrap commit. Then prefer
  one independently green breaking commit because code, migration, docs,
  catalog, and tests describe one public boundary.

## Steps

### Step 0: Restore a trustworthy green bootstrap baseline

Run `rtk cargo fmt --all`, then inspect the diff. It may contain formatting only
across committed experience-layer Rust files. Resolve the 26 clippy errors and
one warning reported at planned HEAD with the smallest idiomatic edits:

- collapse nested conditions without changing environment/capability policy;
- remove needless generic borrows;
- explicitly consume or bind must-use FocusOutcome in tests/callers;
- never add broad `allow` attributes or mix P0 behavior changes into this
  preparatory commit.

Run format check, the exact clippy command, all workspace tests, then
`rtk proxy mise run check`. Commit this independently as a non-breaking
bootstrap repair with DCO and the Codex co-author trailer.

**Verify**:

- `rtk cargo fmt --all -- --check` → exit 0.
- `rtk cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → exit 0.
- `rtk cargo test --workspace --all-features --locked --no-fail-fast` → exit 0.
- `rtk proxy mise run check` → exit 0.

### Step 1: Lock the unsafe and phantom-row regressions with tests

Add focused tests before changing behavior. They must cover:

1. Default approval state cannot confirm `AllowOnce`, `AllowSession`, or
   `Always` on untouched Enter.
2. Escape yields a cancellation outcome, not a fabricated Deny decision.
3. Tab and BackTab wrap across the complete visible decision set.
4. At widths from `0..=48`, the selected decision is either visibly rendered
   or the card renders a deterministic selected-only fallback.
5. A grid with `total_rows == 0` has no body hit regions and Enter is ignored.
6. A grid with two total rows in a ten-row viewport paints/hit-tests only rows
   zero and one.
7. Disabled resident rows cannot be selected, clicked, ranged, or activated.
8. Range paint uses the rectangle between anchor and cursor only.
9. Keyboard and pointer outcomes include `row_id: Some(id)` for resident rows
   and `None` only for in-bounds non-resident rows.
10. Unknown totals still allow pending in-viewport rows without allocating the
    full dataset.

Render assertions must inspect both symbols/styles and published hit geometry.
Model terminal rendering after existing `VirtualGrid` `TestBackend` tests.

**Verify**:

- `rtk cargo test -p termrock approval --all-features --locked` → new approval
  tests fail for the documented old behavior only.
- `rtk cargo test -p termrock virtual_grid --all-features --locked` → new grid
  tests fail for the documented old behavior only.

Do not commit this deliberately red intermediate state.

### Step 2: Replace numeric approval selection with a typed fail-safe contract

Redesign the public state and outcome coherently:

- Store the selected `ApprovalDecision`, not a public `usize`.
- Default to `Deny`. Provide an explicit constructor/builder for a consumer
  that intentionally chooses another initial decision.
- Introduce a typed `ApprovalCardOutcome` with at least `Ignored`,
  `SelectionChanged`, `Confirmed(ApprovalDecision)`, and `Cancelled`.
- Escape returns `Cancelled`. It must not silently convert dismissal into a
  denial decision. `y` may explicitly confirm `AllowOnce`; `n` may explicitly
  confirm `Deny`.
- Accept Press and Repeat for navigation; reject Release. Confirmation must be
  Press-only to prevent held Enter from confirming repeatedly.
- Support BackTab and reverse navigation. Wrap rather than clamp so focus never
  becomes trapped at one edge.
- Keep the canonical ordered decision list in one place used by navigation,
  rendering, and decision lookup.

Do not add deprecated fields, index adapters, or parallel outcomes. Document
exact before/after consumer edits in migration `0032`.

**Verify**: `rtk cargo test -p termrock approval --all-features --locked` → all
approval tests pass.

### Step 3: Make ApprovalCard responsive and fully observable

Use deterministic breakpoints based on measured display width:

- Wide: all decisions on one horizontal row.
- Medium: wrap decisions to multiple rows without changing logical order.
- Tiny height/width: paint the selected decision plus a non-color navigation
  cue; never leave selected state invisible.
- Publish exact decision hit regions in state and handle pointer hover/click
  through them. A normal click confirms once on the repository's canonical
  activation edge, not both Down and Up.
- Preserve `Panel` single-line borders and semantic focused/danger roles.

Add interactive lookbook state for ApprovalCard. Change contract axes to real
coverage and add narrow/unicode stories rather than `not-applicable` claims.

**Verify**:

- `rtk cargo test -p termrock approval --all-features --locked` → pass.
- `rtk cargo run -p termrock-lookbook -- check --dir docs/public/component-previews`
  → pass with regenerated deterministic previews.

### Step 4: Unify VirtualGrid's dataset boundary

Change `VirtualGridState` input methods so resident rows are available when an
outcome needs identity or enabled state. Prefer passing the current borrowed
row projection to keyboard routing over cloning a second hidden dataset into
state. One canonical resolver must answer:

- Is absolute row in known bounds?
- Is it resident?
- If resident, what is its stable ID and enabled state?
- If non-resident but in bounds, is pending navigation allowed?

Required behavior:

- Known total is authoritative. Zero total has no cursor activation. Rendering
  stops at total.
- Resident disabled rows are skipped by keyboard movement and rejected by
  pointer activation/range selection.
- Pending, non-resident rows remain representable only inside known bounds or
  the visible window for unknown totals.
- Range endpoints are `anchor` and `cursor`; normalize those once, then test
  each cell against the normalized row/column bounds.
- Every resident cursor/activation outcome carries its stable row ID.
- Mouse handling clamps/reconciles after changing the cursor.
- Iterate an ordered resident projection once or use binary search; remove the
  current visible-row × resident-row linear search. Do not add a per-frame hash
  allocation.

If the current API cannot express this without a breaking signature change,
make the break. Do not preserve the wrong path.

**Verify**: `rtk cargo test -p termrock virtual_grid --all-features --locked` →
all old and new grid tests pass.

### Step 5: Update migration, docs, catalog, and API inventory

Write migration `0032` with:

- removed approval index/outcome surface;
- typed replacement and exact consumer changes;
- safe default explanation;
- VirtualGrid handler signature changes;
- before/after examples;
- validation commands.

Update `MIGRATING.md`, component docs, contract evidence, stories, previews,
`COMPONENTS.md`, and `docs/api/public-api.txt` in the same change.

**Verify**:

- `rtk proxy rg -n 'selected: usize|Option<ApprovalDecision>' crates/termrock/src/widgets/agent.rs`
  → no old approval-state/outcome contract remains.
- `rtk proxy rg -n '"ApprovalCard"' docs/api/component-contracts.json` → one entry whose
  narrow and keyboard axes are covered.
- Catalog command → pass.

### Step 6: Restore the complete repository gate

Run formatting only after the unrelated active work is reconciled. Fix clippy
causes; do not silence them with broad `allow` attributes.

**Verify**:

- `rtk proxy mise run check` → exit 0.
- `rtk proxy mise run gate` → exit 0.
- `rtk git status --short` → only the intended Plan 039 files before commit;
  clean after commit.

## Test plan

- Unit tests beside `agent.rs`: safe initial state, explicit override, wrapped
  navigation, confirmation edge, cancel semantics, tiny-area visibility.
- Unit/render tests beside `virtual_grid.rs`: zero/short/unknown totals,
  disabled rows, resident IDs, exact range styles, pointer bounds, resize.
- Add a warmed hot-path test only if the new resident resolver allocates or
  regresses viewport complexity; model it after
  `crates/termrock/tests/table_hot_path.rs`.
- Lookbook interactive scenario: navigate, reverse, cancel, confirm, resize.
- Full all-feature and catalog gates remain mandatory.

## Done criteria

- [ ] Untouched Enter can never approve an action.
- [ ] Format, clippy, and bootstrap check are green before behavior work begins.
- [ ] Escape emits cancellation, not Deny.
- [ ] Selected approval choice is visible at every non-empty render size.
- [ ] Known-total grids never paint or hit-test rows outside total.
- [ ] Disabled rows cannot become cursor/range/activation targets.
- [ ] Resident grid outcomes preserve stable row IDs.
- [ ] Range paint matches anchor-to-cursor rectangle exactly.
- [ ] Migration `0032`, index, docs, stories, contracts, previews, and public API
      inventory are fresh.
- [ ] `rtk proxy mise run check` and `rtk proxy mise run gate` exit 0.
- [ ] No compatibility facade or unrelated source edit exists.
- [ ] `plans/README.md` marks Plan 039 DONE.

## STOP conditions

Stop and report if:

- Source worktree is dirty beyond committed plan files.
- Formatter/clippy cleanup requires semantic redesign outside the reported
  experience-layer findings; split and report rather than hiding it.
- Current branch is not `main` and the operator has not explicitly reconciled
  repository policy.
- Migration `0032` is already claimed by unrelated work.
- Approval decisions are no longer the five variants shown above.
- VirtualGrid no longer accepts caller-projected resident rows.
- Correct identity/enabled behavior would require grid-owned fetching or domain
  storage; that violates ownership.
- Any verification fails twice after a reasonable correction.

## Maintenance notes

- Plan 040 should consume `ApprovalCardOutcome::Cancelled` through the unified
  scene. Do not reintroduce raw Escape-to-Deny mapping there.
- Plan 041 may render approvals inside transcript blocks; it must reuse this
  state/outcome contract rather than copy it.
- Reviewers should scrutinize tiny areas, held-key Repeat behavior, stable IDs,
  and whether pending grid rows remain bounded without hidden allocations.
