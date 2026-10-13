# CI gate

## Purpose

The bless-required gate (D5) realizing flow W1: every PR regenerates the
subset PNGs and fails on divergence unless the PR commits the regenerated
files. Placement rides the proven goldens pattern — a workspace test that
`mise run ci`/`test` (the only commands the pinned velnor-actions
`ci-code.yml` runs) executes on every PR; no workflow change needed.
Anchors: F6, W1, B4, D5, N2, N3 · Evidence: research/tui-png-baselines/05-ci-placement-and-commands.md

## Requirements

### Requirement: PNG baseline gate test
`crates/termrock-lookbook` SHALL gain an integration test (goldens-style,
alongside `tests/goldens.rs`) that renders every subset story via
`termrock-raster` and pixel-compares (N3: decoded pixels, zero tolerance —
never PNG bytes) against the committed baseline; on any mismatch or missing
baseline it fails naming the drifted story ids and instructing
`mise run bless-pngs`. Because it runs under
`cargo nextest run --workspace`, it is PR-gated through `mise run ci`/`test`
with no workflow edit (ch. 05 Q2/Q3: the five fixed gates are ci-code.yml's
only extension point).
Covers: F6, W1, B4, D5, N3 · Evidence: ch. 05 Q2, Q3, Q5 (goldens precedent `goldens.rs:79-133`)

#### Scenario: Unchanged rendering passes
- **GIVEN** baselines committed on the current commit
- **WHEN** the gate test runs
- **THEN** it passes

#### Scenario: Pixel drift fails with bless instruction
- **GIVEN** a code change that alters one subset story's rendered pixels
- **WHEN** the gate test runs without blessing
- **THEN** it fails, names the drifted story id, and prints the `mise run bless-pngs` instruction

#### Scenario: Missing baseline fails
- **GIVEN** a newly registered subset story with no committed PNG
- **WHEN** the gate test runs
- **THEN** it fails instructing bless — never silently skips (W1 failure point a: full-subset render every run dissolves affected-file mapping)

#### Scenario: Bless rewrites and the PR carries the diff
- **GIVEN** an intended visual change
- **WHEN** `TERMROCK_BLESS_PNGS=1` runs the test (via `mise run bless-pngs`) and the rewritten PNGs are committed in the same PR
- **THEN** the gate passes and the reviewer sees GitHub's image diff (2-up/swipe/onion) on the PR

### Requirement: Mise task wiring
`mise.toml` SHALL gain `bless-pngs` (env-var bless run, mirroring
`bless-previews` at `mise.toml:69-71`) and `png-baselines` (locked diff run,
mirroring `preview-goldens` at `mise.toml:73-75`), and `gate` SHALL invoke
`png-baselines` next to its existing `preview-goldens` step (`mise.toml:51`).
Covers: F6, D5 · Evidence: ch. 05 Q3

#### Scenario: Local gate covers PNGs
- **WHEN** `mise run gate` runs
- **THEN** the PNG baseline diff executes as one of its steps

### Requirement: Determinism guard in CI
The gate test SHALL include a render-twice determinism assertion (raw RGBA
equality) so a non-deterministic pipeline fails as a pipeline bug — W1
failure point (b): such a failure MUST NOT be resolved by blessing. The
failure message SHALL say so explicitly. If a macOS-blessed baseline ever
diverges on the Linux runners, that falsifies ledger assumption A3 and the
recorded fallback (bless in a pinned Linux container or CI-produced bless
artifact) activates — the failure message SHALL name A3.
Covers: W1, B4 · Evidence: ch. 06 §4 (measured identity + untested cross-OS axis), ch. 05 Q4 (no macOS lane exists; runners Linux-inferred)

#### Scenario: Nondeterminism is not blessable
- **GIVEN** a render-twice mismatch in one process
- **WHEN** the gate test fails
- **THEN** the message classifies it as a pipeline bug referencing A3, not as design drift to bless
