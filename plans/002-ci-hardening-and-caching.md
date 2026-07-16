# Plan 002: Enforce full dependency policy, secret scanning, and workflow linting on every push; cache CI builds

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- .github/workflows/ deny.toml`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition. (If Plan 001 already landed, its
> rust.yml edits are expected — only treat *other* differences as drift.)

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/001-verification-gate-baseline.md (same workflow file; execute 001 first)
- **Category**: dx
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

`deny.toml` configures four policy families (advisories, licenses allowlist, wildcard bans, unknown sources) but CI runs only `cargo deny check advisories`, and only on a weekly cron — a disallowed license or unknown git source is never caught automatically, and a fresh advisory waits up to 7 days. `gitleaks` and `actionlint` are version-pinned in `mise.toml` but never executed, so there is no continuous secret scanning after the one-time bootstrap history scan. Finally, no workflow has any cargo caching: all four jobs cold-compile the entire dependency graph on every push, and the `semver-candidate` job compiles `cargo-semver-checks` from source each run.

## Current state

- `deny.toml` (entire file):

```toml
[advisories]
yanked = "deny"

[licenses]
confidence-threshold = 0.8
allow = ["Apache-2.0", "MIT", "Unicode-3.0", "Zlib"]

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

- `.github/workflows/hygiene.yml` (entire file):

```yaml
name: Hygiene
on:
  schedule: [{ cron: "17 5 * * 1" }]
  workflow_dispatch:
permissions: { contents: read }
jobs:
  hygiene:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: jdx/mise-action@v2
      - run: cargo deny check advisories
      - run: cargo tree --workspace --locked
      - run: cargo run -p termrock-lookbook -- check --dir docs/public/component-previews
```

- `.github/workflows/rust.yml` `semver-candidate` job builds the checker from source every run:

```yaml
      - run: mise install cargo-binstall
      - run: mise install cargo:cargo-semver-checks
```

(Note: mise's `cargo:` backend uses cargo-binstall when available, so this may already binstall — the real cost is the absent cache, not this line. Verify rather than assume: check the job's timing after caching lands.)

- No workflow contains any `cache`/`rust-cache` step (verified by grep across `.github/workflows/`).
- `mise.toml` pins `"ubi:gitleaks/gitleaks" = "8.28.0"` and `actionlint = "1.7.12"`; neither appears in any workflow.
- `compatibility.toml:47` documents the intended form: `cargo deny check advisories bans licenses sources` and `gitleaks detect --no-banner --redact --source .`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Deny full policy | `mise x -- cargo deny check advisories bans licenses sources` | exit 0 ("advisories ok, bans ok, licenses ok, sources ok") |
| Secret scan | `mise x -- gitleaks detect --no-banner --redact --source .` | exit 0, "no leaks found" |
| Workflow lint | `mise x -- actionlint` | exit 0 |

## Scope

**In scope**:
- `.github/workflows/rust.yml`
- `.github/workflows/hygiene.yml`
- `.github/workflows/docs.yml` (cache step only)

**Out of scope**:
- `deny.toml` policy content — the policy itself is fine; only its enforcement is missing.
- `.github/workflows/release.yml` — release flow untouched.
- `mise.toml` (Plan 001 owns it).
- Any secret remediation: if gitleaks finds a leak, STOP — never copy the value anywhere; report the file:line and credential type only, and recommend rotation.

## Git workflow

- Directly on `main`, Conventional Commits + DCO sign-off, e.g. `git commit -s -m "ci: run full dependency policy and secret scan on push"`.

## Steps

### Step 1: Promote full cargo-deny to the push path

In `rust.yml` `rust-required`, add after the clippy step:

```yaml
      - run: cargo deny check advisories bans licenses sources
```

In `hygiene.yml`, change `- run: cargo deny check advisories` to the same four-check form (the weekly run stays as a advisory-freshness safety net).

**Verify**: `mise x -- cargo deny check advisories bans licenses sources` → exit 0 locally; `mise x -- actionlint` → exit 0.

### Step 2: Add gitleaks and actionlint to CI

In `rust.yml` `rust-required`, add near the top (right after checkout + mise setup — change `mise install rust` to `mise install` if pinned tools beyond rust are needed and not installed):

```yaml
      - run: gitleaks detect --no-banner --redact --source .
      - run: actionlint
```

**Verify**: both commands exit 0 locally; `mise x -- actionlint` → exit 0.

### Step 3: Add cargo caching to all Rust jobs

In `rust.yml` (jobs `rust-required`, `crossterm-platform`, `semver-candidate`, and the `msrv` job if Plan 001 added it) and `hygiene.yml`/`docs.yml` (they run `cargo run -p termrock-lookbook`), insert after the toolchain setup step:

```yaml
      - uses: Swatinem/rust-cache@v2
```

**Verify**: `mise x -- actionlint` → exit 0. `grep -c "Swatinem/rust-cache" .github/workflows/rust.yml` → ≥ 3.

### Step 4: Confirm semver-checks is not compiled from source

After caching lands, inspect one CI run of `semver-candidate` (or run `mise install cargo:cargo-semver-checks` locally with timing). If it still compiles from source, replace the two `mise install` lines with a direct binstall:

```yaml
      - run: mise x -- cargo binstall --no-confirm cargo-semver-checks
```

**Verify**: job wall time drops; the step log shows a binary download, not a build. (If you cannot observe CI, leave this step as a note in your report rather than guessing.)

## Test plan

No Rust tests. Verification = the three commands in "Commands you will need" all exit 0, plus actionlint on every modified workflow.

## Done criteria

- [ ] `grep -n "advisories bans licenses sources" .github/workflows/rust.yml .github/workflows/hygiene.yml` → one hit in each
- [ ] `grep -n "gitleaks detect" .github/workflows/rust.yml` → present
- [ ] `grep -n "actionlint" .github/workflows/rust.yml` → present as a run step
- [ ] `grep -c "Swatinem/rust-cache" .github/workflows/*.yml` ≥ 4
- [ ] `mise x -- actionlint` → exit 0
- [ ] `mise x -- gitleaks detect --no-banner --redact --source .` → exit 0
- [ ] `git status` → no files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

- `cargo deny check ... licenses` or `... sources` fails locally: a dependency violates the documented policy today. Report which crate/license; changing `deny.toml` policy is a maintainer decision.
- `gitleaks` reports a finding: STOP immediately. Report only file:line + credential type + "rotate". Never include the matched string in any output, commit, or plan.
- `actionlint` flags pre-existing errors in workflows you did not touch: report, don't fix silently.

## Maintenance notes

- `Swatinem/rust-cache` keys on `Cargo.lock` — nothing to maintain, but large `target/` caches can evict; if CI slows again, check cache-hit rates before adding more steps.
- The weekly `hygiene.yml` run is now redundant with the push-path deny check except as an advisory-freshness net for quiet weeks — keep it.
- TODO.md defers "compiler cache" decisions until 20 CI runs are measured; `rust-cache` is a dependency cache, not sccache — it does not contradict that note, but record post-landing timings to inform it.
