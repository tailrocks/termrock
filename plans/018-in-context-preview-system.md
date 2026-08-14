# Plan 018: Components shown living in real applications — the in-context preview system

> **Executor instructions**: Follow this plan step by step. Read
> `docs/design/component-documentation-standard.md` and
> `docs/design/interactive-preview-host.md` before starting; the docs build
> is gate-heavy — every gate you must touch is named below. Run every
> verification command. STOP conditions binding. Update `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d09bd2fe..HEAD -- docs/ crates/termrock-lookbook*`
> On mismatch with "Current state", verify before editing.

## Status

- **Priority**: P1 (goal: "components documentation and previews in kind of
  real-work TUI applications")
- **Effort**: L
- **Risk**: MED (touches CI-gated docs contracts; every gate change is enumerated)
- **Depends on**: plans/011 (catalog truth — in-context scenes must show the
  post-redesign paint, not today's); plans/016 (pattern charter)
- **Category**: dx / design-infrastructure
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

Today every one of the 165 component pages shows its widget in isolation —
the "widget demo" failure mode the design SoTs name explicitly. The pieces
for in-context display already exist and are simply never composed: pattern
catalog entries carry `buildingBlocks: string[]` (e.g. `agent-workbench`
lists 23 blocks) but the reference is only rendered forward;
`PatternGallery.tsx:21-24` already defines a working "shown inside its
canonical parent application" mechanism for 2 layout-helper pages; the
`TerminalPreview` Variant `<select>` remounts sibling stories by
`descriptor.component` — which means in-context application scenes can ship
as VARIANTS of the existing single preview without violating the
one-preview-per-page CI gates.

## Current state (from the docs-pipeline digest; verify with rg)

- One preview per page is HARD-GATED thrice: `docs/scripts/check-catalog.ts:63`,
  `check-component-pages.ts:134-137`, `check-pattern-pages.ts:95-98`.
- Component page section order is byte-checked (`check-component-pages.ts:67-78`);
  the section list lives in `component-documentation-standard.md:32-67`.
- Inventory literals are asserted: 135 widgets / 165 component routes / 35
  patterns / 183 embedded demos (`check-catalog.ts:28,85`,
  `check-component-pages.ts:38,53,187`, `check-pattern-pages.ts:39`).
- Variant select: `TerminalPreview.tsx` filters sibling stories by
  `descriptor.component`; Code view via `docs/public/demo-code.json`;
  posters auto-export for story ids referenced in MDX
  (`docs/scripts/export-preview-posters.ts`).
- App-scene hosts that already exist as interactive stories:
  `app-shell/{workbench,dashboard,master-detail,minimal,narrow-drawer,offline}`
  (`stories.rs:7058-7111`), `agent-workbench/*` (11 variants),
  `PATTERN_DEMO_IDS` 31 ids (`stories.rs:239-271`).
- wasm blob 4.6 MB monolith; posters 14 MB/183 files — size is a budget.
- `pattern-catalog.json` classification: 16 application + 17 composite +
  2 layout-helper.

## Step 1: "Seen in" reverse index (cheap, all 165 pages)

Generate the reverse mapping `component → [patterns whose buildingBlocks
contain it]` from `docs/api/pattern-catalog.json` at build time
(`docs/scripts/` new step or inside the MDX loader). Render on every
component page as a new final-section block: `## Seen in applications` —
pattern links + poster thumbnails (reuse `PatternGallery` thumbnail
component). Update in lockstep (same commit):
- `component-documentation-standard.md` section list (+1 section, position
  after `## Source and related material` or merged into it — pick after,
  document),
- `check-component-pages.ts:67-78` expected-section array,
- any inventory literals the new generated content trips.
Components used by zero patterns render the section with a "not yet
composed in a shipped example" line — that list is itself a coverage
signal; emit it in the build log.

**Verify**: `cd docs && bun run build` → exit 0; List's page links
agent-workbench + others; build log prints the zero-usage component list.

## Step 2: In-context scene variants for the priority components

Mechanism: new lookbook stories `"<component>/in-app"` whose render mounts
an EXISTING application interactor (reuse — no new state machines) with the
target component's pane focused/highlighted, `descriptor.component` set to
the component so the Variant select picks it up. The page still has exactly
one live preview → no gate changes needed for this step.

Coverage: the 48 priority components, mapped to their canonical host:
- collections/inputs/chrome (List, Tree*, Table*, TextInput, Select,
  Tabs, StatusBar, Toolbar, Panel, Sidebar…) → `app-shell/workbench` or
  `agent-workbench/basic` scene focusing that pane;
- feedback (Toast, Spinner, LoadingView, StatusIndicator) →
  `observability-dashboard` scene;
- forms (Form, PasswordInput, Checkbox…) → `settings-screen`/`auth-entry`;
- overlays (Dialog, QuickOpen, CommandPalette…) → `app-shell/workbench`
  with the overlay open.
Implementation is a data table (component id → host story + focus target) +
one generic `in_app_scene(host, focus)` story builder in `stories.rs`.
Budget: reusing interactors keeps wasm growth to the story-table only;
posters grow ONLY for ids referenced in MDX (variants are runtime — no new
posters). Story-count literals (`~1,065`) update where asserted.

**Verify**: component page for List shows Variant options including
"In application"; selecting it mounts the workbench scene with the list
focused; `check:preview-metrics` + Playwright suites green.

## Step 3: WOW surfaces — landing hero + galleries

- Docs landing (`index.mdx`): live `agent-workbench/basic` hero preview
  (falls back to poster) + one-line pitch; components index page (165-row
  flat table today) becomes a grouped gallery (family groups from
  COVERAGE.md clusters) with poster thumbnails.
- Patterns index: keep `PatternGallery`, add classification blurbs and a
  "start here" ordering (workbench → dashboard → auth → settings).
- These pages are not under the component/pattern page contracts — only
  `check-catalog.ts` totals apply; update literals if page count changes.

**Verify**: `bun run build` green; landing renders the hero; visual spec
passes.

## Step 4: Documentation-standard + registry alignment

- `component-documentation-standard.md`: codify the "Seen in" section, the
  in-app variant naming (`<component>/in-app`), and the rule that a
  building-block claim in `pattern-catalog.json` is the SINGLE source for
  the reverse index (no hand-written lists).
- `source-owned-registry.md` note: story packs for installed blocks include
  their in-app scenes (the registry model already anticipates story packs).

**Verify**: `mise run contracts` green.

## Done criteria

- [ ] `cd docs && bun run build` exit 0 (all gates), `mise run gate` exit 0.
- [ ] All 165 component pages render "Seen in applications" (generated).
- [ ] ≥48 priority components have an "In application" variant scene.
- [ ] Landing hero live; components index grouped with thumbnails.
- [ ] Standard doc updated in the same commits as the gate changes.
- [ ] Migration file ONLY if crate-public API changed (story ids are not
      API); otherwise note in commit body. README row updated.

## STOP conditions

- The Variant select's `descriptor.component` filter can't host scenes
  without UI changes (e.g. sort order buries them) — small TSX change is in
  scope; a second live preview per page is NOT (that gate stays).
- wasm blob exceeds 6 MB after the story table — report size delta and
  stop adding scenes until a splitting strategy is decided.
- A host interactor can't focus the target pane via public state — report
  the missing hook (widgets-plan item), don't fork the interactor.

## Maintenance notes

- New components must ship an in-app scene when any pattern composes them;
  the zero-usage build-log list is the tracker.
- Plan 019 (showcase binary) supersedes none of this: the docs scenes are
  the browsing surface, the binary is the end-to-end proof (SKD-1 and the
  explicit rejection of "lookbook-only" both stand).
