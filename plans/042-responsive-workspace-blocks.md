# Plan 042: Replace flat rectangles with a responsive workspace tree

> **Executor instructions**: Execute in order. This is a forward-only public
> redesign. Verify each step; STOP instead of adding a compatibility layout.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/layout crates/termrock/src/patterns crates/termrock/src/widgets/panel.rs crates/termrock-lookbook docs/api docs/content/docs MIGRATING.md migrations`
>
> Compare drift with "Current state." Start only when Plans 040 and 041 are
> DONE and `rtk proxy mise run gate` is green.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plans 040 and 041
- **Category**: architecture, feature, UX, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

High-end terminal apps behave like responsive workspaces: panes collapse by
meaning, tabs preserve context, focus follows layout, dividers resize, a pane
can zoom, and tiny terminals remain contained. TermRock currently returns flat
rectangle recipes. Consumers must invent pane trees and frequently create
invalid geometry under pressure, so the shared component layer cannot guarantee
its own narrow-terminal contract.

## Current state

- `layout/work_surface.rs` allocates fixed/flexible tracks independently and
  can emit children outside its parent when fixed sizes plus gaps exceed a
  narrow area.
- WorkSurface is flat; it has no stable pane identity, nesting, stack/tab,
  collapse priority, zoom, divider interaction, or ratio memory.
- AgentShell, OpsDashboard, and ResourceBrowser are rectangle-returning pattern
  functions. Their state/focus/hit behavior remains consumer glue.
- Overlay/floating surfaces are not constrained by the same layout and input
  ownership model.
- Panel already owns border geometry and semantic focus roles. Preserve its
  single-line border law.
- Domain data, routing, processes, and effects remain consumer-owned.
- This plan owns migration `0035`.

## Target contract

Introduce a controlled workspace tree with stable IDs:

```text
WorkspaceNode
├── Leaf(id, constraints, collapse priority)
├── Split(axis, ratio, first, second)
├── Stack(id, tabs/leaves)
├── Dock(edge, size policy, child)
└── Float(id, anchor/constraints, child)
```

`WorkspaceState` owns only domain-neutral interaction state: active tabs,
focused pane, split ratios, collapsed panes, zoom target, divider drag, and
remembered ratios. Layout returns exact pane/divider/tab geometry plus semantic
registrations. Consumers render domain content into named leaves.

Every returned rectangle must be contained by the input area. Under pressure,
the solver deterministically reduces gaps, honors minimums where possible, then
collapses lower-priority panes into stacks/docks. It never relies on saturating
math that silently paints outside the parent.

## Scope

**In scope**:

- `layout/work_surface.rs` and related layout exports;
- new workspace tree/state/solver types;
- InteractionScene integration for panes, tabs, dividers, zoom, and floats;
- controlled product-neutral pattern components for AgentShell, OpsDashboard,
  ResourceBrowser, and a multi-step Wizard;
- exhaustive tiny-area/property-style tests, hot-path tests, lookbook scripts,
  docs/catalog/API inventory, migration `0035`.

**Out of scope**:

- application routing, files, processes, metrics, agent state, or wording;
- terminal window management outside the app viewport;
- a CSS/Flexbox/Grid clone or retained view tree;
- compatibility wrappers around flat WorkSurface/pattern functions;
- graphics protocol lifecycle (Plan 044).

## Git workflow

- Work directly on `main`; STOP otherwise.
- Conventional Commit + `rtk git commit -s` +
  `Co-authored-by: Codex <codex@openai.com>`.
- Every commit passes `rtk proxy mise run check`; push only after `rtk proxy mise run gate`.
- Land the solver privately with tests if useful, then expose the coherent
  public tree/state/pattern break with migration and generated docs together.

## Steps

### Step 1: Lock containment and pressure semantics

Build a slow reference solver/test oracle and exhaust dimensions `0..=32` on
both axes for representative nested trees. Assert:

1. every pane, tab, divider, and float rect is contained by the parent;
2. siblings do not overlap unless explicitly a Float;
3. collapsed nodes consume neither cell size nor orphan gaps;
4. output is deterministic for identical tree/state/area;
5. total partition size plus gaps never exceeds parent extent;
6. focus order follows visible semantic order, not stale tree position;
7. dragging/resizing cannot violate child minimums;
8. width/height restoration recovers remembered ratios and active tabs;
9. zoom produces exactly one visible content leaf and restores prior state;
10. all arithmetic is overflow-safe at `u16` extremes.

Include the exact current fixed-track-plus-gap overflow as a regression test.

**Verify**: focused layout tests expose the current containment failure before
the new solver replaces it.

### Step 2: Implement the tree and deterministic pressure solver

Define borrowed, stable-ID nodes and explicit constraints:

- minimum/preferred/maximum along relevant axis;
- flex weight or split ratio;
- collapse priority and permitted collapse destination;
- optional breakpoint policy expressed in cell geometry, not terminal brand;
- visible/enabled state.

Solver order must be documented and tested:

1. remove invisible nodes;
2. compute child pressure and legal gaps;
3. satisfy minimums when possible;
4. reduce optional gaps/chrome;
5. collapse lower-priority children by stable policy;
6. distribute remainder by ratios/weights with deterministic rounding;
7. clamp all geometry to the parent and report degradation metadata.

Return a `WorkspaceLayout<Id>` containing pane/tab/divider/float rectangles and
degradation decisions. Reuse caller-provided scratch/capacity; warmed layout
must not allocate per frame for an unchanged tree.

### Step 3: Add controlled workspace interaction state

`WorkspaceState<Id>` handles typed scene actions for:

- focus next/previous/directional pane;
- activate next/previous/specific tab;
- resize focused divider with keyboard or pointer drag;
- zoom/unzoom focused pane;
- collapse/reopen permitted pane;
- dismiss/activate top float according to InteractionScene policy.

State reconciles by stable ID when nodes reorder/disappear. It returns neutral
outcomes including changed ratios/tabs/focus so consumers may persist them.
TermRock must not persist anything itself.

Floats register as explicit scene layers. They must not bypass top-layer Escape,
outside-click, or focus ownership.

### Step 4: Replace rectangle recipes with real pattern blocks

Rebuild these as controlled, product-neutral compositions:

- `AgentShell`: main transcript, composer, optional activity/context rail;
- `OpsDashboard`: summary strip, primary data view, optional inspector/log;
- `ResourceBrowser`: tree/list, preview, optional metadata/actions;
- `Wizard`: step rail, current content, validation/action footer.

Each pattern accepts borrowed content projections or render closures only at
the leaf boundary if existing Ratatui ownership demands it. It owns layout,
chrome, semantic theme selection, focus navigation, collapse behavior, and hit
geometry. It returns typed neutral outcomes. It never gains branded modes or
domain data.

At documented widths, patterns degrade to stack/tabs or one focused leaf rather
than returning unusable panes. Reuse Transcript from Plan 041 in AgentShell.

### Step 5: Prove responsive behavior in the lookbook

Add deterministic scripts for:

- resize 120→80→40→20→120 and verify collapse/restoration;
- keyboard focus traversal and directional navigation;
- tab activation and disabled tab;
- divider keyboard and pointer resize;
- zoom/unzoom;
- float over workspace with top-layer Escape;
- Unicode labels and no-color/non-color focus cues.

Generate previews at every breakpoint and semantic traces with visible pane IDs,
focus, active tabs, ratios, and degradation decisions. Add public pattern
inventory/contracts/stories; patterns are public components and need the same
evidence standard as widgets.

### Step 6: Migrate docs and run gates

Write `migrations/0035-v0.12.0-responsive-workspace.md` with removed flat APIs,
tree/state replacements, exact pattern consumer changes, breakpoints,
ownership, before/after examples, and commands. Update `MIGRATING.md`, layout
and pattern docs, API/contracts/inventory, and previews.

**Verify**:

- focused layout/pattern/scene tests → pass;
- workspace hot-path test → warmed unchanged tree has zero allocations;
- `rtk cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` → pass;
- `rtk proxy mise run check` and `rtk proxy mise run gate` → both exit 0.

## Test plan

- Exhaustive small-area reference comparisons and `u16` extreme cases.
- Stable-ID state reconciliation across reorder/remove/disable.
- Keyboard/mouse divider, tab, focus, zoom, and float interaction tests.
- Render/semantic snapshots at 20/40/80/120 columns.
- Warmed allocation/local-work regression tests.
- Full lookbook scripts and catalog evidence.

## Done criteria

- [ ] Workspace tree supports leaf, split, stack, dock, and float.
- [ ] No layout rectangle escapes its parent at any tested size.
- [ ] Pressure/collapse/degradation order is deterministic and documented.
- [ ] Stable state survives resize, reorder, collapse, and zoom.
- [ ] InteractionScene owns all pane/tab/divider/float input geometry.
- [ ] Four reusable patterns are controlled components, not rect recipes.
- [ ] Pattern inventory, docs, contracts, stories, previews, and migration
      `0035` are current; old flat public paths are gone.
- [ ] Full gates pass.

## STOP conditions

Stop and report if:

- Plans 040/041 are not DONE, branch is not `main`, tree is dirty, or `0035`
  is claimed.
- A pattern requires product-domain state, wording, effects, or persistence.
- Solver correctness would require rectangles outside the caller area.
- Proposed callbacks create a retained widget tree or hide interaction state.
- Pattern public inventory cannot be generated by the catalog without changing
  its generic inventory mechanism; split that generator foundation explicitly,
  do not hand-maintain a second catalog.
- Any verification fails twice after a reasonable correction.

## Maintenance notes

- New patterns should be workspace compositions, not independent layout
  algorithms.
- Preserve Panel's single-line border geometry and semantic focus role.
- Plan 044 preview/media placement must consume workspace leaf geometry and
  scene lifecycle rather than emitting protocol output during render.
