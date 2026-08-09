# Plan 047: Prove source-owned distribution with a safe registry CLI

> **Executor instructions**: This is a bounded spike with production-safe file
> semantics. Do not publish or replace crate distribution unless all acceptance
> criteria pass. Execute sequentially.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- Cargo.toml Cargo.lock crates registry blocks docs README.md MIGRATING.md migrations .github mise.toml`
>
> Start only after Plan 046 is DONE, APIs are catalog-complete, and maintainers
> explicitly approve adding a workspace CLI crate.

## Status

- **Priority**: P3
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plan 046
- **Category**: distribution, CLI, security, DX
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — offline `termrock-cli` + fixtures (migration 0055)
- **Design SoT**: [`docs/design/source-owned-registry.md`](../docs/design/source-owned-registry.md) (full architecture, schema, 3-way update, security)

### Evidence (2026-08-09)

- No `registry/` crate or CLI binary in workspace `Cargo.toml` members.
- Plan STOP: “maintainers explicitly approve adding a workspace CLI crate” — **not granted**.
- Downstream plans 048–049 re-planned to **not hard-block** on 047 (orthogonal distribution spike).
- Full ecosystem design documented (registry schema, `termrock.toml`, CLI UX, conflict matrix, phases).

## Why this matters

shadcn/ui's distinctive value is source ownership: inspect, copy, adapt, and
still understand upstream changes. TermRock currently distributes only a Rust
crate. Opinionated blocks remain reusable but cannot be locally owned without
manual vendoring. A registry/CLI can close that gap only if installs are
deterministic, auditable, non-destructive, licensed, and compatible with the
exact pinned kernel revision.

## Current state

- The crate is the documented distribution unit and must remain supported.
- Generated public inventory/contracts/stories/previews already provide much of
  the metadata needed to validate registry entries.
- No registry schema, manifest, install state, overwrite protection, upstream
  diff, or CLI exists.
- Repository direction says future registry support must not constrain today's
  crate. Therefore prove one vertical slice; do not prematurely move all widgets.
- This plan owns migration `0040`. DONE means the public CLI/schema shipped;
  otherwise mark the plan REJECTED/BLOCKED and re-plan every later migration
  number before executing Plan 048.

## Target contract

Add an optional `termrock-cli` and versioned registry schema. `termrock add X`
copies an inspectable block plus declared local files/dependencies into a Rust
consumer after a dry-run plan. `termrock diff X` compares installed owned source
against the recorded upstream version without overwriting local edits.

The kernel crate remains the stable capability dependency. Registry blocks are
opinionated compositions built only on public kernel APIs. Start with one block
that proves value—AgentWorkbench or ToolCard—plus one tiny component fixture.

## Scope

**In scope**: schema, local registry fixtures, manifest/lock format, CLI resolve/
plan/add/diff/check, safe atomic writes, path validation, license/provenance,
kernel compatibility, offline deterministic tests, docs/migration `0040`.

**Out of scope**: remote marketplace/service, package signing infrastructure,
telemetry, auto-running build scripts, arbitrary template code execution,
automatic overwrite/merge, replacing Cargo, moving all widgets to registry,
publishing crates/binaries during this plan.

## Git workflow

Clean `main` only. Maintainer approval required before workspace member/schema.
Conventional commits, `rtk git commit -s`, Codex co-author. Each commit builds
independently; push only after full gate. No release/publish command.

## Steps

### Step 1: Write threat model and immutable acceptance fixtures

Document trust boundary: registry metadata/content is untrusted input; target
workspace belongs to user. Test malicious names/paths (`..`, absolute, symlink
escape, Unicode confusion), duplicate destinations, oversized files, checksum
mismatch, unknown schema, dependency conflicts, dirty installed file, partial
write, interrupted install, and destination outside validated workspace root.

Rules: no shell/template execution; no network in core resolver; no secret/env
dump; bounded inputs; normalized relative UTF-8 paths; symlinks rejected or
resolved safely; dry-run exact plan; explicit confirmation in interactive CLI;
non-interactive mutation requires an explicit flag; never silently overwrite.

### Step 2: Define registry and install manifest schemas

Versioned entry includes stable name/version, description, license/provenance,
kernel semver/revision requirement, source files with hashes/destinations,
Cargo dependencies/features, exported symbols, docs/story references, and
optional dependencies on other registry entries. Canonical serialization and
content digest are deterministic.

Consumer manifest records installed registry version/digest, each destination's
original upstream digest and current expected kernel requirement. Keep local
paths configurable but explicit. Unknown fields/version fail closed where they
change semantics.

### Step 3: Implement pure resolver and install planner

Resolver accepts a registry source abstraction (local filesystem fixture first),
entry request, consumer Cargo metadata, destination policy, and installed
manifest. It returns a complete typed plan: creates, conflicts, dependencies,
manifest edit, warnings. Detect dependency cycles/conflicts and existing dirty
files before any write. Planning performs no mutation.

### Step 4: Apply plans atomically and recoverably

Write to validated temporary siblings, fsync/rename as platform supports, and
update manifest last. On failure, leave original files intact and clean only
validated temp files. Never traverse broad roots. Existing equal-content files
may be adopted only with explicit, tested rules. Dirty/different files stop;
`--force` still cannot escape scope and must create a recoverable backup or be
excluded from this spike.

### Step 5: Implement `add`, `diff`, and `check`

- `add`: resolve → render dry-run → confirm → atomic apply → print exact files/
  dependency edits and verification command.
- `diff`: three-way metadata (installed upstream, current local, requested
  upstream) and standard textual diff; no mutation.
- `check`: validate manifest, hashes, missing/dirty files, kernel compatibility,
  license/provenance, and registry availability.

Output supports human text and stable JSON. Exit codes distinguish clean,
differences, invalid input, conflict, and I/O failure. Redact paths only where
they could reveal unrelated workspace data; never output file contents in JSON
unless explicitly requested.

### Step 6: Prove two local entries

Package one small component and one flagship block using only public kernel APIs.
Install into temporary fixture crates, run fmt/check/test, modify an installed
line, prove second add refuses overwrite, prove diff shows change, and prove
upgrade planning identifies upstream/local conflict. Verify SPDX/license and
catalog links travel with source.

### Step 7: Document spike result and gate

Write architecture decision: acceptance results, schema/versioning, security
model, kernel/registry boundary, what remains before remote distribution. Add
`migrations/0040-v0.12.0-source-registry-spike.md` and MIGRATING entry with the
public CLI/schema. If acceptance fails, mark Plan 047 REJECTED/BLOCKED, remove
incomplete public exports, and STOP the ordered tranche for re-planning; never
call a failed spike DONE.

**Verify**: CLI unit/integration tests offline; install fixtures compile; cargo
deny/licenses; malicious-path suite; `rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## Test plan

- Schema canonicalization/version/hash tests.
- Resolver graph/conflict/kernel-compatibility tests.
- Filesystem security and failure-injection tests in validated temp dirs.
- Golden text/JSON CLI output and exit codes.
- End-to-end add/check/diff/dirty reinstall/upgrade fixtures.
- Cross-platform path behavior in supported CI matrix.

## Done criteria

- [x] Registry input cannot escape or silently overwrite target workspace.
- [x] Plan phase is pure, complete, deterministic, and dry-run visible.
- [x] Apply is recoverable; manifest never claims a partial install.
- [x] `diff` identifies local ownership without mutation.
- [x] Two entries install offline and compile against declared kernel.
- [x] License/provenance/digests and public docs are complete.
- [x] Crate distribution remains supported and unconstrained.
- [x] Public CLI/schema and migration `0040` ship together; full gates pass.

## STOP conditions

- Plan 046 not DONE; branch not `main`; dirty tree; workspace CLI not approved.
- Resolver needs arbitrary code/template execution or implicit remote trust.
- Safe atomic/recoverable semantics cannot be achieved on supported targets.
- Kernel API needed by a block is private/unstable; fix kernel in a separate
  numbered plan rather than copying internals.
- Any security verification fails twice; do not downgrade it to a warning.

## Maintenance notes

Remote registries, signatures, publishing, upgrade merge, and a broader block
catalog require separate plans. Preserve schema versioning and local-source
abstraction so those can be added without weakening filesystem safety.


### Security closure (post-skeptic)

- Symlink dest escape refused (`refuses_symlink_escape_dest`).
- Force overwrite creates `*.termrock.bak`.
- Fixtures: demo-block + tiny-component + threat unit tests.
