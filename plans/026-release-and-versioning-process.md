# Plan 026: A real release process — honest semver gate, version tags, git-cliff changelog, documented flow

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`. NOTE: Step 2 (tag backfill) pushes tags — that is an
> outward-facing action; it is explicitly authorized by this plan ONLY after
> its verification passes, and only the listed tags.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- .github/workflows/release.yml .github/workflows/rust.yml CONTRIBUTING.md MIGRATING.md`
> Plans 001/002 legitimately edit rust.yml. Other mismatches = STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW-MED (tag backfill is permanent; everything else is config/docs)
- **Depends on**: plans/001-verification-gate-baseline.md (rust.yml conventions), coordinate with 002 (same file)
- **Category**: dx
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

Four connected process gaps. (1) The `semver-candidate` CI job compares against baseline tag `v0.6.0` while the workspace is at 0.9.0 — under 0.x semver rules a 0.6→0.9 minor delta pre-authorizes unlimited breaking changes, so the gate is structurally incapable of failing: false assurance plus a full baseline build per push. (2) Migration files name boundaries v0.7.0/v0.8.0/v0.9.0, but the only git tag is `v0.6.0`, and `release.yml` asserts tag == current Cargo version — so intermediate versions can never be tagged; "pin an exact revision and walk migrations by version" points at labels that resolve to nothing checkoutable. (3) No CHANGELOG exists; `gh release create --generate-notes` in a no-PR trunk-only repo produces a raw commit dump, and the carefully-authored `migrations/` files never surface in releases. (4) The bump-version+migration+release flow is tribal knowledge — no RELEASING.md.

## Current state

- `.github/workflows/rust.yml` `semver-candidate` job (committed state): runs `cargo semver-checks check-release -p termrock --baseline-rev v0.6.0` (guarded by tag existence).
- `git tag -l` → `v0.6.0` only. `Cargo.toml:6` → `version = "0.9.0"`. Version-bump commits: `bb8ff31` bumped 0.8.0→0.9.0 with `migrations/0003-v0.9.0-styled-tab-glyphs.md`. Find the 0.7.0 and 0.8.0 bump commits: `git log --oneline -S 'version = "0.7.0"' -- Cargo.toml` and same for 0.8.0 (also cross-check the commits that added `migrations/0001-*.md` / `0002-*.md`).
- `.github/workflows/release.yml` (entire flow): workflow_dispatch with `tag` input → assert `v$(cargo version) == tag` → test → package → docs build → `git tag -a` + push → `gh release create "$tag" --generate-notes`.
- `MIGRATING.md` table maps sequence → version → file; `AGENTS.md` tells consumers to pin revisions and migrate forward.
- No `CHANGELOG.md`, no `cliff.toml`. Conventional Commits are repo law (CONTRIBUTING.md), which is exactly git-cliff's input format. `mise` can pin git-cliff (`"cargo:git-cliff"` or ubi).
- CONTRIBUTING.md: "After `v0.6.0`, `semver-candidate`, `rust-required`, and `docs-required` are release gates; incompatible changes require an intentional version decision."
- Repo conventions: trunk-only, DCO, Conventional Commits.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Find bump commits | `git log --format='%h %s' -S 'version = "0.7.0"' -- Cargo.toml` | one add-commit |
| Changelog gen | `mise x -- git-cliff --unreleased` (after config) | grouped conventional sections |
| Workflow lint | `mise x -- actionlint` | exit 0 |
| Tag verify | `git tag -l && git ls-remote --tags origin` | expected tag set |

## Scope

**In scope**:
- `.github/workflows/{rust.yml (semver job only), release.yml}`
- `cliff.toml` (new), `CHANGELOG.md` (generated), `mise.toml` (git-cliff pin)
- `RELEASING.md` (new), `CONTRIBUTING.md` (pointer sentence), `MIGRATING.md` (tag-mapping note)
- Git tags `v0.7.0`, `v0.8.0`, `v0.9.0` (annotated, backfilled — Step 2)

**Out of scope**:
- crates.io publication (deferred by decision).
- Rewriting any migration file content.
- `compatibility.toml` (append-only).

## Git workflow

- Directly on `main`; commits: `ci: make the semver gate compare adjacent releases`, `chore(release): backfill version boundary tags` (tags themselves), `docs: add release procedure and changelog generation`.

## Steps

### Step 1: Repoint the semver gate at the previous release

Change the `semver-candidate` job to derive the baseline dynamically: latest tag preceding HEAD (`git describe --tags --abbrev=0`) instead of hardcoded `v0.6.0`:

```yaml
      - name: Compare with the previous release tag
        run: |
          baseline=$(git describe --tags --abbrev=0 2>/dev/null || true)
          if [ -n "$baseline" ]; then
            cargo semver-checks check-release -p termrock --baseline-rev "$baseline"
          else
            echo 'no baseline tag yet'
          fi
```

Purpose shift documented in the job name/comment: it now catches breaking changes NOT declared by a version bump — if the workspace version equals the baseline tag's version (no bump yet) and semver-checks finds breakage, that's the failure this gate exists for; once the version is bumped (with its migration file), the same breakage is authorized and passes. That is a meaningful gate under forward-only rules. (After Step 2's backfill, baseline = v0.9.0, current = 0.9.0 ⇒ the gate becomes live immediately.)

**Verify**: `mise x -- actionlint` → exit 0.

### Step 2: Backfill the boundary tags

Locate the three bump commits (see Commands; each should be the commit introducing `migrations/000N-v0.X.0-*.md` + the Cargo bump — verify BOTH facts per commit; `bb8ff31` is the known v0.9.0 one). Create annotated tags at those commits: `git tag -a v0.7.0 <sha> -m "TermRock v0.7.0 — canonical namespaces"` (message from the migration title), same for v0.8.0, v0.9.0. Push: `git push origin v0.7.0 v0.8.0 v0.9.0`.

This is permanent and outward-facing: perform it only after the commit-identification verification below succeeds; tag ONLY these three.

**Verify (before pushing)**: for each candidate sha: `git show <sha>:Cargo.toml | grep 'version ='` shows the target version AND `git show <sha> --stat` includes the matching migration file. After push: `git ls-remote --tags origin` lists all four tags.

### Step 3: git-cliff changelog

Pin git-cliff in `mise.toml`. Add `cliff.toml`: conventional-commit grouping (feat/fix/refactor/perf/docs/test/chore), tag pattern `v[0-9]*`, breaking-change (`!`) section first, link template to `migrations/` when the release has one. Generate `CHANGELOG.md` covering v0.6.0..HEAD and commit it. Wire `release.yml`: replace `--generate-notes` with a git-cliff-generated notes file for the released range, prepending a "Migration: [migrations/000N-...](...)" line when a migration file exists for that version:

```yaml
      - run: mise x -- git-cliff --latest --output /tmp/notes.md
      - run: |
          mig=$(ls migrations/ | grep -F "${{ inputs.tag }}" || true)
          [ -n "$mig" ] && sed -i "1i **Migration guide:** [migrations/$mig](https://github.com/tailrocks/termrock/blob/main/migrations/$mig)\n" /tmp/notes.md
      - env: { GH_TOKEN: "${{ github.token }}" }
        run: gh release create "${{ inputs.tag }}" --notes-file /tmp/notes.md
```

**Verify**: `mise x -- git-cliff --unreleased` produces grouped output locally; `actionlint` → exit 0; `CHANGELOG.md` committed and reads sanely (spot-check the v0.9.0 section contains the styled-tab-glyphs feat).

### Step 4: RELEASING.md

Write the one-page procedure: (1) breaking change lands atomically = code + `migrations/000N-vX.Y.0-slug.md` + `MIGRATING.md` row + `Cargo.toml` minor bump, all one commit (the observed `bb8ff31` pattern — cite it as the exemplar); (2) rule: one migration file ⇔ one minor bump (state it as the inferred rule; flag for maintainer confirmation); (3) release = dispatch `release.yml` with `tag: vX.Y.0` matching the current Cargo version — it verifies, tests, packages, tags, and publishes cliff-generated notes with the migration link; (4) the semver gate's meaning (Step 1's declared-vs-undeclared breakage contract). Add a `MIGRATING.md` note: migration versions correspond to git tags from v0.7.0 onward. Point CONTRIBUTING.md's "intentional version decision" sentence at RELEASING.md.

**Verify**: `grep -n "RELEASING" CONTRIBUTING.md MIGRATING.md` → pointers present.

## Test plan

No Rust tests. Verification = actionlint, local git-cliff output, tag-set assertions, and the Step 2 per-commit identity checks.

## Done criteria

- [ ] semver job baselines `git describe --tags --abbrev=0`; no hardcoded v0.6.0
- [ ] Tags v0.7.0/v0.8.0/v0.9.0 exist locally and on origin, each on its verified bump commit
- [ ] `cliff.toml` + generated `CHANGELOG.md` committed; release.yml uses `--notes-file` with migration link injection
- [ ] `RELEASING.md` exists; CONTRIBUTING/MIGRATING point at it
- [ ] `mise x -- actionlint` → exit 0
- [ ] `plans/README.md` status row updated

## STOP conditions

- A version bump commit cannot be uniquely identified (squash/merge ambiguity, bump separate from migration) — STOP before tagging anything; report the candidates. Wrong tags are forever.
- The maintainer-inferred "one migration ⇔ one minor bump" rule is contradicted by history (e.g. a bump without migration) — document the actual observed rule in RELEASING.md and flag it.
- git-cliff's output on this history is garbage (merge-commit noise from the `chore(merge)` commits) — configure `skip` patterns for `chore(merge)` in cliff.toml; if still unusable, report with a sample.

## Maintenance notes

- Every future bump commit is a tag candidate — RELEASING.md's flow keeps tags, versions, migrations, and releases in lockstep; the semver gate now audits exactly that lockstep.
- If crates.io publication is ever un-deferred, this plan's artifacts (tags, changelog, notes) are the prerequisites — nothing here needs redoing.
- CHANGELOG.md regeneration cadence: `git-cliff --unreleased` locally anytime; the committed file updates at release time via the workflow (or add it to the release commit — maintainer taste, note in RELEASING.md).
