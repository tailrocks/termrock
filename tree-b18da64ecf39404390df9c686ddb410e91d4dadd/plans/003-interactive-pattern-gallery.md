# Plan 003: Publish every application pattern as the same live demo in web and Lookbook

> **Executor instructions:** Complete Plans 001 and 002 first. Use the shared
> demo catalog; do not build a second pattern runner. Run every verification and
> update `plans/README.md` when done.
>
> **Drift check (run first):**
> `rtk git diff --stat 26457206..HEAD -- crates/termrock/src/patterns crates/termrock-lookbook/src docs/content/docs/application-patterns.mdx docs/content/docs/patterns docs/content/docs/meta.json docs/scripts`
> Changes made by Plans 001/002 are expected. Compare their final contracts to
> this plan; stop on incompatible assumptions rather than reviving old stories.

## Status

- **Execution:** DONE on `feat/live-interactive-docs`
- **Priority:** P1
- **Effort:** L
- **Risk:** MED; broad demo coverage but no external side effects
- **Depends on:** `plans/001-live-preview-runtime.md`,
  `plans/002-unified-component-documentation.md`
- **Category:** direction / docs
- **Planned at:** commit `26457206`, 2026-08-12

## Why this matters

TermRock already contains real application-shaped compositions, but the website
does not expose them as applications. Native Lookbook and website should offer
the same runnable sample: identical state, events, hints, outcomes, time, and
paint. This proves how building blocks compose into production-like TUIs without
turning product recipes into first-class widgets.

## Current state

- `crates/termrock/src/patterns/mod.rs:4-19` defines patterns as example
  compositions and names AppShell as the canonical shell.
- The public `termrock::patterns` surface is implemented by 35 pattern source
  modules. Only 20 have exact-name handbook pages. Missing are `agent-shell`,
  `app-dashboard`, `app-shell`, `auth-entry`, `connection-manager`,
  `database-workbench`, `error-recovery`, `file-manager`, `git-workbench`,
  `help-center`, `observability-dashboard`, `ops-dashboard`,
  `project-launcher`, `resource-browser`, and `studio-shell`.
- `docs/content/docs/application-patterns.mdx:1-86` documents the showcase,
  keymaps, and Crossterm runner. It is not an application gallery.
- Many missing applications already have static Lookbook stories in
  `crates/termrock-lookbook/src/stories.rs`, but a story variant is not a
  persistent application session.
- Native Lookbook and the website currently present different subsets and
  interaction semantics. Plan 001 supplies the common host contract.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Pattern inventory | `rtk bun docs/scripts/check-pattern-pages.ts` | 35/35 classified, documented, and demo-linked |
| Shared demos | `rtk cargo test -p termrock-lookbook --lib pattern --locked` | all pattern traces pass |
| Browser flows | `rtk bun --cwd docs run test:patterns` | all application flows pass |
| Site build | `rtk bun --cwd docs run build` | pattern index/pages prerender |
| Full gate | `rtk mise run gate` | exit 0 |

## Scope

**In scope:**

- demo implementations/registrations in the backend-neutral lookbook library
- native Lookbook catalog grouping, hints, outcomes, and full-preview mode
- `docs/content/docs/application-patterns.mdx`, new
  `docs/content/docs/patterns/*.mdx`, metadata, and root navigation
- a pattern manifest/checker under `docs/api/` and `docs/scripts/`
- browser acceptance tests and deterministic demo fixtures
- pattern/lookbook documentation affected by the new catalog

**Out of scope:**

- Moving product composites into `termrock::widgets`
- Real network, filesystem mutation, subprocesses, credentials, or persistence
- Product-branded behavior in generic widgets
- Separate web and terminal implementations of an application
- Replacing the accepted terminal renderer

## Git workflow

The user's execution instruction supersedes the original workflow: all three
plans ship from `feat/live-interactive-docs` in one PR to `main`. Commits use
Conventional Commits, DCO sign-off, and
`Co-authored-by: Codex <codex@openai.com>`.

## Steps

### Step 1: Define complete pattern coverage and honest classifications

Generate/check a manifest against every public module exported from
`termrock::patterns`. Each entry has a stable pattern ID, title, source path,
demo ID, composed building blocks, default dimensions, and one class:

- `application`: full stateful sample with navigation and multiple surfaces
- `composite`: reusable multi-widget recipe embedded in a larger host
- `layout-helper`: geometry/slot helper best demonstrated inside its canonical
  parent application

Every entry gets a page. Layout helpers do not get fake standalone behavior;
their page mounts the parent live demo with the relevant region highlighted and
shows the exact helper code.

**Verify:** `check-pattern-pages.ts` reports 35/35 entries, no duplicate IDs,
valid source paths, valid shared demo IDs, and no pattern exported as a widget.

### Step 2: Build stateful sample applications from public APIs

Convert existing story fixtures into long-lived demos. Use public pattern and
widget APIs exactly as a consumer does. Each demo owns deterministic in-memory
domain data and visible host outcomes. Required flows include:

- App/Agent/Studio shells: focus traversal, sidebar toggle, responsive resize.
- AuthEntry: switch field, type, validate, submit/cancel feedback.
- ConnectionManager: select, open a modal, confirm/cancel, update visible status.
- Agent/Git/Database workbenches: change panes, open/close overlays, take one
  safe local action, and show the typed result.
- FileManager/ResourceBrowser: navigate, scroll, expand/collapse, activate.
- ProjectLauncher/HelpCenter: filter, select, activate, clear.
- Dashboards: navigate panels, inspect detail, change range/filter where public
  APIs support it.
- ErrorRecovery/SettingsScreen/SetupWizard: trigger retry or form transitions
  and show deterministic success/error outcomes.

No demo may pretend to execute a process, connect to a database, authenticate,
or write a file. It demonstrates UI contracts and consumer-owned effects only.

**Verify:** deterministic Rust event traces cover each `application` entry and
assert state, latest outcome, and meaningful frame changes.

### Step 3: Make native Lookbook the terminal host for the same demos

Group Components and Application patterns in the native catalog. Selecting a
pattern mounts the exact shared demo ID used by its docs page. Route keyboard,
mouse, paste, resize, focus, and time without Lookbook-specific behavior.
Display the demo's current hints and latest outcome. Add a full-preview mode so
multi-pane applications can use the whole terminal, with a documented escape
sequence that does not steal an application's first `Esc`.

**Verify:** for every manifest entry, native Lookbook can discover and mount the
demo. Replay a representative trace in native and web adapters and compare
cells, hints, outcomes, and caret policy.

### Step 4: Build the web application-pattern gallery

Replace the current single prose page with:

- `/docs/patterns`: grouped, searchable catalog using static posters for cards
- `/docs/patterns/<slug>`: one tall live application, Preview/Code, action hints,
  Reset, latest outcome, composed-component links, source, and classification
- a full-viewport/Zen control that resizes the same session rather than mounting
  an alternate implementation

The first viewport should tell a user what action opens the interesting state.
Dialogs, sidebars, toasts, menus, and wizards start from a realistic trigger,
not permanently open screenshots.

**Verify:** all 35 routes prerender, hydrate, mount their declared demo, and
link only to existing component/pattern/source destinations.

### Step 5: Add cross-host application acceptance tests

Browser tests must cover at least AuthEntry typing, ChoiceDialog decision,
ConnectionManager modal lifecycle, AppShell sidebar toggle and resize,
FileManager tree navigation, one workbench overlay, SetupWizard step transition,
and timed Toast feedback. For each, assert the same normalized trace against the
native adapter. Add a static check that docs and Lookbook resolve the same demo
ID for every pattern.

Run pattern checks, shared tests, browser tests, site build, then full gate.

## Done criteria

- [x] All 35 public patterns have explicit classification and a canonical page.
- [x] Every application/composite page mounts the same Rust demo as Lookbook.
- [x] Native and web hosts expose identical actions, outcomes, time, and paint.
- [x] Real app flows open/close overlays, type, select, drag, scroll, and resize
      wherever the public pattern supports them.
- [x] Layout-only helpers are demonstrated honestly inside a parent application.
- [x] No demo performs external effects or reimplements widget behavior.
- [x] Pattern inventory, parity, browser, site, and full gates pass.
- [x] `plans/README.md` marks Plan 003 `DONE`.

## STOP conditions

- Plans 001/002 do not provide one shared demo ID and event contract.
- A pattern's public API cannot support its required real interaction. Report
  the API gap and create the next numbered migration plan; do not reach private
  state or hard-code a fake transition.
- The same action produces different state/outcome between native and web hosts.
- A pattern cannot be classified under the mandatory building-block/composite law.
- A test would need real external credentials, processes, or destructive I/O.
- Any verification fails twice after a focused correction.

## Maintenance notes

Pattern coverage is part of the public catalog contract. New public pattern
exports must add one manifest entry, one canonical page, one shared demo (or a
declared parent demo for layout helpers), deterministic traces, and both-host
discoverability in the same change.
