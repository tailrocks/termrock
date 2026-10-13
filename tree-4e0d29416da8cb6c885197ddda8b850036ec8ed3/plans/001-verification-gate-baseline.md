# Plan 001: Make the documented verification gates real — one local command, CI that enforces what TESTING.md claims

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- mise.toml TESTING.md .github/workflows/rust.yml Cargo.toml rust-toolchain.toml`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (enabling real gates may surface currently-hidden breakage — that is the point; see STOP conditions)
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

`TESTING.md` tells contributors the gates are "formatting, clippy with warnings denied, all-feature nextest, dependency policy, REUSE, deterministic lookbook checks, feature powerset, and MSRV gates" — but CI runs none of: nextest, feature powerset, REUSE, cargo-shear, or any MSRV build. The advertised Rust 1.95 floor (`Cargo.toml` `rust-version = "1.95"`, README compatibility table) has zero automated protection while every toolchain in the repo is 1.97. There is also no single local command to run "the gate"; the true chain exists only as data inside `compatibility.toml`. This repo is trunk-only (commits go straight to `main`), so a contributor who cannot run the full gate locally pushes red commits. This plan is the verification baseline every other plan's "Done criteria" relies on.

## Current state

- `mise.toml` — tool pins only, **no `[tasks]` table**. Pinned but never used in CI: `cargo-nextest 0.9.136`, `cargo-hack 0.6.45`, `cargo-shear 1.13.0` (also `cargo-deny`, `gitleaks`, `actionlint`, `cargo-public-api` — handled by Plans 002/003):

```toml
[tools]
actionlint = "1.7.12"
bun = "1.3.14"
cargo-binstall = "1.21.0"
rust = "1.97.0"
"cargo:cargo-deny" = "0.20.0"
"cargo:cargo-hack" = "0.6.45"
"cargo:cargo-nextest" = "0.9.136"
"cargo:cargo-shear" = "1.13.0"
"cargo:cargo-public-api" = "0.52.0"
"cargo:cargo-semver-checks" = "0.46.0"
"ubi:gitleaks/gitleaks" = "8.28.0"
```

- `.config/nextest.toml` exists (`[profile.default]` with `retries = 0`, `fail-fast = false`) but nextest is never invoked anywhere.
- `.github/workflows/rust.yml` `rust-required` job steps (the entire current PR gate):

```yaml
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - run: cargo test --workspace --all-features --locked
      - run: cargo check -p termrock --no-default-features --locked
      - run: cargo check --workspace --examples --locked
      - run: cargo check --workspace --examples --features crossterm --locked
      - run: RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
      - run: cargo package -p termrock --locked --allow-dirty
```

- The authoritative full gate chain the project already documents (as *data*, in `compatibility.toml` line 77 — a `command =` string): fmt-check, clippy `-D warnings`, all-feature tests, no-default-features check, examples checks (both feature sets), `cargo doc` with `-D warnings`, `cargo package`, `cargo semver-checks --baseline-rev v0.6.0`, `cargo deny check advisories bans licenses sources`, `cargo shear --deny-warnings`, `gitleaks detect`, lookbook preview check, docs `bun run types:check && bun run build`. And `compatibility.toml:17,23` show the nextest/powerset form: `cargo nextest run --workspace --all-features --locked && cargo test --doc --workspace --locked` and `cargo hack check --workspace --feature-powerset --all-targets --locked`.
- `rust-toolchain.toml` pins `1.97.0`; no CI job builds with 1.95. MSRV claim: `Cargo.toml:8` `rust-version = "1.95"`.
- `TESTING.md` (entire file):

```markdown
# Testing

Run formatting, clippy with warnings denied, all-feature nextest, dependency
policy, REUSE, deterministic lookbook checks, feature powerset, and MSRV gates.
```

- REUSE: `REUSE.toml` + `LICENSES/` exist; no `reuse` tool pinned in mise, no CI step. (The `reuse` linter is a Python tool; mise can pin it via `pipx:reuse`.)
- Repo conventions: Conventional Commits with DCO sign-off (`git commit -s`), trunk-only on `main`, no PRs (`AGENTS.md` "All TermRock work happens directly on `main`").

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tool install | `mise install` | exit 0 |
| Format check | `mise x -- cargo fmt --all -- --check` | exit 0 |
| Clippy | `mise x -- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Tests (current) | `mise x -- cargo test --workspace --all-features --locked` | 174+ pass, 0 fail |
| Nextest | `mise x -- cargo nextest run --workspace --all-features --locked` | all pass |
| Doctests | `mise x -- cargo test --doc --workspace --locked` | exit 0 (nextest skips doctests — keep both) |
| Powerset | `mise x -- cargo hack check --workspace --feature-powerset --all-targets --locked` | exit 0 |
| Shear | `mise x -- cargo shear` | exit 0 (add `--deny-warnings`? see Step 4) |
| Workflow lint | `mise x -- actionlint` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `mise.toml`
- `TESTING.md`
- `.github/workflows/rust.yml`
- `CONTRIBUTING.md` (one sentence pointing at the new task)

**Out of scope** (do NOT touch):
- `compatibility.toml` — append-only verification log owned by the compatibility process; do not edit or "fix" its header (Plan 003 documents its semantics).
- `.github/workflows/hygiene.yml`, `docs.yml` — Plan 002 owns CI hardening there.
- `deny.toml`, gitleaks wiring — Plan 002.
- Fixing any code the new gates flag beyond what Step 5/6 explicitly allows. If MSRV or powerset reveal real breakage, STOP and report (that is a finding, not a drive-by fix).

## Git workflow

- Work directly on `main` (repo rule: no feature branches, no PRs).
- Conventional Commits with sign-off, e.g. `git commit -s -m "ci: enforce documented verification gates"`.
- One commit per step is fine; do not push until all done criteria pass locally.

## Steps

### Step 1: Add mise tasks encoding the gate

Append to `mise.toml`:

```toml
[tasks.check]
description = "Fast pre-commit check: fmt, clippy, all-feature nextest"
run = """
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo test --doc --workspace --locked
"""

[tasks.gate]
description = "Full pre-push gate mirroring rust-required + policy checks"
run = """
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo test --doc --workspace --locked
cargo check -p termrock --no-default-features --locked
cargo check --workspace --examples --locked
cargo check --workspace --examples --features crossterm --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
cargo hack check --workspace --feature-powerset --all-targets --locked
cargo deny check advisories bans licenses sources
cargo shear
cargo package -p termrock --locked --allow-dirty
cargo run -p termrock-lookbook -- check --dir docs/public/component-previews
"""
```

**Verify**: `mise run check` → exit 0. Then `mise run gate` → exit 0. If `cargo shear` or `cargo hack` fail, see STOP conditions.

### Step 2: Switch the CI test step to nextest + doctests

In `.github/workflows/rust.yml` `rust-required`, replace
`- run: cargo test --workspace --all-features --locked` with:

```yaml
      - run: cargo nextest run --workspace --all-features --locked
      - run: cargo test --doc --workspace --locked
```

(The mise-action step already installs pinned tools, so `cargo nextest` resolves. Confirm `jdx/mise-action@v2` + `mise install rust` steps precede it; if only `mise install rust` is run, change that line to `mise install` so all pinned tools install.)

**Verify**: `mise x -- actionlint` → exit 0.

### Step 3: Add feature-powerset and shear steps to rust-required

After the examples-check steps in `rust.yml`, add:

```yaml
      - run: cargo hack check --workspace --feature-powerset --all-targets --locked
      - run: cargo shear
```

**Verify**: `mise x -- actionlint` → exit 0; run both commands locally → exit 0.

### Step 4: Add an MSRV job

Append to `rust.yml`:

```yaml
  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.95
      - run: cargo check -p termrock --all-features --locked
      - run: cargo check -p termrock --no-default-features --locked
```

Note: use `cargo check` (not test) for the MSRV job — dev-dependencies and the lookbook may legitimately need a newer toolchain; the public MSRV promise covers building the `termrock` crate.

**Verify**: `mise x -- actionlint` → exit 0. Locally, if you have rustup: `rustup toolchain install 1.95 --profile minimal && cargo +1.95 check -p termrock --all-features --locked` → exit 0. If 1.95 check fails, STOP (MSRV claim is false; that's a report, and the fix is either code or bumping `rust-version` — a maintainer decision).

### Step 5: Add REUSE lint

Add `"pipx:reuse" = "5.3.1"` (or current) to `mise.toml` `[tools]`, and a step in `rust.yml` `rust-required` (or a small separate job):

```yaml
      - run: reuse lint
```

**Verify**: `mise install && mise x -- reuse lint` → exit 0 ("Congratulations! Your project is compliant..."). If non-compliant files are reported, STOP and list them in your report (fixing license headers repo-wide is out of scope).

### Step 6: Reconcile TESTING.md and CONTRIBUTING.md

Rewrite `TESTING.md` to name the two entry points and what each runs:

```markdown
# Testing

`mise run check` — formatting, clippy (warnings denied), all-feature nextest,
doctests. Run before every commit.

`mise run gate` — the full trunk gate: everything in `check` plus
no-default-features and examples checks, rustdoc with warnings denied,
feature powerset (cargo-hack), dependency policy (cargo-deny: advisories,
bans, licenses, sources), unused-dependency check (cargo-shear), packaging,
and the deterministic lookbook preview check. CI additionally verifies the
Rust 1.95 MSRV and REUSE compliance. Run before every push.
```

In `CONTRIBUTING.md`, replace the phrase "run the relevant gates" with "run `mise run gate`".

**Verify**: `grep -n "mise run gate" TESTING.md CONTRIBUTING.md` → one hit in each.

## Test plan

No new Rust tests. The deliverable *is* verification infrastructure:
- `mise run check` and `mise run gate` exit 0 locally.
- `mise x -- actionlint` exits 0 on the edited workflow.

## Done criteria

- [ ] `mise run check` → exit 0
- [ ] `mise run gate` → exit 0
- [ ] `grep -c "nextest" .github/workflows/rust.yml` ≥ 1; `grep -c "feature-powerset" .github/workflows/rust.yml` ≥ 1; `grep -c "shear" .github/workflows/rust.yml` ≥ 1
- [ ] `grep -n "1.95" .github/workflows/rust.yml` → MSRV job present
- [ ] `grep -n "reuse lint" .github/workflows/rust.yml` → present
- [ ] `mise x -- actionlint` → exit 0
- [ ] `TESTING.md` names `mise run check` / `mise run gate`; no claim of an unenforced gate remains
- [ ] `git status` shows no modified files outside the in-scope list
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `cargo hack --feature-powerset` fails: a feature combination is broken. Report the failing combination and error; do not patch library code.
- `cargo +1.95 check` fails: the MSRV claim is false. Report the error; the 1.95-vs-bump decision belongs to the maintainer.
- `cargo shear` reports unused dependencies: report them; removing deps is out of scope here.
- `reuse lint` reports non-compliant files: report the list.
- `mise` cannot install a pinned tool (network/registry issue) after two attempts.

## Maintenance notes

- Every later plan in `plans/` uses `mise run gate` as its final verification — keep the task in sync with CI when either changes.
- Plan 002 adds cargo-deny scope/triggers, gitleaks, actionlint, and CI caching on top of this workflow file; execute it after this plan to avoid merge friction.
- If a future change bumps `rust-version` in `Cargo.toml`, the MSRV job's toolchain pin must be bumped in the same commit.
