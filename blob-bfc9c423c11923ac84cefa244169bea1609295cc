# Plan 053: Ship the full source-ownable application block collection

> **Executor instructions**: Compose public components only. Blocks own reusable
> experience state/chrome; consumers own every domain projection and effect.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/patterns blocks registry crates/termrock-cli crates/termrock-lookbook docs/design/component-anatomy-spec.md docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 042, 046, 047, and 049–052 are DONE and gate is green.

## Status

- **Execution**: DONE — migration 0056

- **Priority**: P2
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plans 042, 046, 047, and 049–052
- **Category**: blocks, distribution, flagship UX, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

shadcn-level adoption comes from blocks that show how primitives become real
applications while remaining owned and adaptable. TermRock's OpsDashboard and
ResourceBrowser are geometry sketches; SettingsShell/FormWizard are absent.
AgentWorkbench alone does not demonstrate breadth. This plan turns the catalog
into source-ownable, full-state compositions with premium terminal behavior.

## Current state

- Plan 042 creates controlled workspace/pattern foundations.
- Plan 046 ships AgentWorkbench; 047 ships source registry/CLI; 049 PreviewHost;
  050–052 complete component families.
- binding spec names OpsDashboard, ResourceBrowser, SettingsShell, FormWizard.
- consumers must own metrics/log/file/settings/validation/routing/process policy,
  effects, persistence, secrets, and wording.
- this plan owns migration `0046`.

## Target blocks

- `OpsDashboard`: summary/actions, DataTable/charts, LogStream, inspector, alerts.
- `ResourceBrowser`: Sidebar/Tree, Breadcrumbs, list/table, PreviewHost,
  ObjectInspector/actions.
- `SettingsShell`: Sidebar/sections/forms, search, unsaved-state projection,
  responsive drawer/rail.
- `FormWizard`: stepper/progress, per-step borrowed form content, validation
  projection, back/next/review/submit-request outcomes.

Each block has kernel-crate API plus source-registry package when appropriate.
Installed source uses only public kernel contracts.

## Scope

**In scope**: controlled state/config/outcomes; workspace/scene/design/responsive
composition; registry entries/manifests/provenance; deterministic Studio stories;
docs/contracts/API/migration `0046`.

**Out of scope**: real ops/file/settings/form engines, I/O, routing, persistence,
validation rules, executors, secrets, branded copy, hidden dependencies, local
render substitutes, automated registry overwrite.

## Git workflow

Clean `main`; Conventional Commit; `rtk git commit -s`; Codex co-author. Each
block lands independently green but migration/catalog stays coherent. Registry
fixtures use Plan 047 safety workflow. Push after full gate.

## Steps

### Step 1: Lock a universal block boundary

Define generated checks: borrowed domain projections; stable IDs; controlled
domain values; domain-neutral state only; typed outcomes; no I/O/callbacks;
one WorkspaceState/InteractionScene/DesignSystem; narrow/tiny degradation;
registry package compiles only against public API; docs/story/preview/trace/
contract/provenance all required.

### Step 2: Build OpsDashboard

Project summary metrics, series, rows, logs, alerts, and inspector details.
Compose charts/DataTable/LogStream/ObjectInspector/Callout and workspace. Outcomes
request time range, sort/filter/action, row inspect, log follow, retry—never
perform. Wide/medium/narrow layouts preserve current focus and priority data.

### Step 3: Build ResourceBrowser

Project path crumbs, tree/list resources, metadata, actions, and preview content.
Compose Breadcrumbs/Tree/DataTable/PreviewHost/ObjectInspector/Menu. Selection
generation drives caller load requests; stale preview guarded by Plan 049.
Narrow collapses preview/metadata to tabs/drawer. No filesystem/URL/MIME policy.

### Step 4: Build SettingsShell and FormWizard

SettingsShell projects section IDs/labels/descriptions/form slots, dirty/error
counts, and search results. Outcomes request select/save/reset/discard; caller
owns values/persistence. FormWizard projects steps/current content/validation;
state owns navigation/focus only. It cannot advance past invalid projection
unless caller policy allows; SubmitRequested never submits itself.

### Step 5: Package owned source

Add each block to registry schema with hashes, license/provenance, kernel
requirement, files/dependencies, docs/story links. End-to-end `termrock add`,
compile fixture, dirty reinstall refusal, and `termrock diff` for each. Avoid
copying kernel components into blocks.

### Step 6: Definitive block gallery

Studio scripts cover live projection updates, loading/empty/error, nested
menus/dialogs, keyboard/mouse, widths 160→120→80→40→20→120, density/capability/
ASCII/no-color/reduced motion, stable focus restoration, typed outcomes, and
registry-installed fixture render equivalence. No external I/O.

### Step 7: Migrate and gate

Write `migrations/0046-v0.12.0-application-blocks.md` with removed rect-pattern
APIs, controlled replacements, ownership tables, consumer mapping examples,
registry install/diff commands, validation. Update MIGRATING/docs/contracts/
inventory/stories/previews/traces/registry.

**Verify**: block model/integration tests; registry security/install fixtures;
Studio completeness; allocation/local-work checks; separate check/gate pass.

## Test plan

- Generic block ownership/completeness tests.
- Each block state/outcome/responsive/overlay integration suite.
- Loading/error/stale projection cases.
- Registry add/check/diff/dirty/compile fixtures.
- Studio matrix and warmed allocation tests.

## Done criteria

- [x] Four full controlled blocks replace geometry sketches/missing recipes.
- [x] Blocks use public components only and execute no domain effects.
- [x] Responsive/focus/overlay/projection behavior is deterministic.
- [x] Every block installs as owned source safely and compiles.
- [x] Studio/catalog evidence covers all quality axes.
- [x] Migration `0046`, docs/registry/contracts/previews/traces/API fresh.
- [x] Old rect-pattern public APIs removed; full gates pass.

## STOP conditions

- Prerequisites not DONE; non-main/dirty tree; `0046` claimed.
- Any block needs domain policy/data ownership/effects in TermRock.
- Registry package needs private kernel API or duplicates a component body.
- Responsive composition requires invalid/out-of-parent geometry.
- Any security/verification fails twice after reasonable correction.

## Maintenance notes

Future blocks follow the same source-owned/public-kernel/Studio evidence contract.
Product-branded variants belong in consumer registries, not TermRock core.


### Registry packaging (post-skeptic)

- `registry/fixtures/{ops-dashboard,resource-browser,settings-shell,form-wizard}`
- Install+compile: `install_blocks_compile` integration test.
