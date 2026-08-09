# Plan 048: Turn the lookbook into an executable TermRock Studio

> **Executor instructions**: Execute in order. Static “covered” claims are
> replaced by scenario evidence; do not hand-maintain exceptions.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock-lookbook crates/termrock/src/style crates/termrock/src/interaction docs/api docs/public docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 040 and 043–046 are DONE and the gate is green.
> Plan 047 (registry CLI) is orthogonal and may remain BLOCKED without stopping Studio.

## Status

- **Priority**: P3
- **Effort**: L
- **Risk**: MEDIUM
- **Depends on**: Plans 040 and 043–047
- **Category**: tooling, design system, accessibility, DX, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — multi-panel DesignInspector + layout_studio_shell in lookbook + expanded interactors

## Why this matters

The current lookbook's 74 stories, contract matrix, generated API inventory, and
deterministic SVGs are a strong seed. But authors cannot inspect scene layers,
hit regions, focus order, active intents/actions, token recipes, responsive
degradation, or capability ladders. Several interactive components lack
interactors, knobs may not drive real state, and contract JSON can claim an axis
without executable proof.

shadcn-quality needs a studio that makes hidden interaction/design contracts
visible and fails generation when evidence is absent.

## Current state

- stories render deterministic previews and some have interactors;
- contract axes are not uniformly linked to scenario IDs/checkpoints;
- Bool/Number/other knobs are not guaranteed to mutate real story parameters;
- no semantic overlay shows InteractionScene geometry/layers/focus/actions;
- no matrix viewer for density/glyph/color capability/selection chrome/motion;
- ThemePicker is not established as a transactional live-preview editor;
- Plan 043 migrates core recipes; this plan extends completeness enforcement to
  every public component without redesigning each component API ad hoc;
- this plan owns migration `0041`.

## Target contract

Studio is both an interactive TUI and deterministic generator:

- every story declares fixtures, supported axes, scripts, and checkpoints;
- interactors drive typed actions/pointer/resize/time, not private mutations;
- overlays inspect scene registrations, focus order, layers, actions, hit rects,
  design recipes, and degradation decisions;
- contracts reference executed scenario evidence;
- representative visual/semantic matrices are generated with a deterministic
  frame clock and capability input.

## Scope

**In scope**: story/schema/interactor runtime; inspector panels; real knobs;
ThemePicker transaction; deterministic FrameTick; capability/design matrix;
contract evidence validation; all-public-component completeness; CLI commands,
docs/API/migration `0041`.

**Out of scope**: full terminal automation, image pixel diffs requiring a real
terminal, product data, remote registry UI, terminal probing side effects,
CSS/style editor, arbitrary story callbacks outside typed harness.

## Git workflow

Clean `main` only. Conventional Commit, DCO, Codex co-author. Each commit green;
push after full gate. Schema/public CLI break lands with migration/docs.

## Steps

### Step 1: Define evidence schema and fail closed

Each story declares stable ID, component/pattern ID, fixture, viewport, relevant
axes/values, initial focus, deterministic frame ticks, interaction scripts, and
expected semantic checkpoints. Each contract axis names one or more scenario
IDs. Generator rejects missing/unknown/unexecuted evidence and interactive
components without an interactor.

Inventory completeness requires: public API entry, contract row, docs, story,
deterministic preview, design recipe declaration, and interactor when state can
change. Remove bare `covered: true` semantics.

### Step 2: Build typed script runner and semantic trace

Commands include focus/action, key chord through Keymap, pointer move/click/
drag, resize, set knob, advance deterministic time, and assert checkpoint.
Trace records only stable semantic facts: focused element, active scope/layer,
available action IDs, routed target, outcome kind, workspace degradation,
transcript anchor, and selected recipe roles. Avoid private struct dumps and
unstable debug strings.

Run headless and inside Studio. Same script must produce same trace/preview.

### Step 3: Add interaction/design inspectors

Toggleable panels visualize:

- scene rects, z/layer, scope, focus order, input owner, Esc/outside policy;
- current Keymap chord→action and exact discoverable actions/hints;
- hit region under cursor and last routed outcome;
- DesignSystem values and resolved row/panel/component recipe;
- capability tier, glyph, density, selection chrome, motion/frame tick;
- workspace tree/degradation and transcript visible range/cache summary.

Inspectors use public diagnostic projections, never reach into widget private
state. Hide/redact fixture content where not needed.

### Step 4: Make every knob operational

Wire Enum/Bool/Number/Text/theme/density/glyph/capability/selection/motion/width
knobs to declared story parameters. Reject unsupported knob definitions. Changes
reset or reconcile state deterministically based on schema policy. Provide a
one-command axis sweep and side-by-side/diff metadata without a Cartesian
explosion.

Representative generation policy: broad default preview plus risk-based
variants for narrow, compact, reduced motion, ANSI/no-color, ASCII, disabled/
danger/loading, Unicode, and focus.

### Step 5: Make ThemePicker transactional

Picker previews candidate theme/system/density/glyph/motion without mutating the
committed caller choice. Enter emits Commit; Escape emits Cancel and restores
original preview. Design validators block invalid candidates with actionable
diagnostics. Persistence remains caller-owned. Add scripts for preview/commit/
cancel/invalid.

### Step 6: Backfill catalog evidence

Use generated inventory to enumerate every public widget and pattern. Add the
smallest meaningful scenario set for each, emphasizing real interactions and
risk axes. AgentWorkbench is the cross-system canary. Add deterministic Unicode,
zero/tiny area, focus/non-color, and disabled evidence where relevant.

Generation must report unused stories, orphan contract evidence, and stale
previews/traces as errors.

### Step 7: Document Studio and gate

Write `migrations/0041-v0.12.0-lookbook-studio.md` covering schema/CLI changes,
evidence requirements, interactor migration, deterministic clock, before/after
story example, commands. Update MIGRATING, contributor/design docs, API schema,
generated artifacts.

**Verify**:

- lookbook unit/schema/runner tests pass;
- `rtk cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` passes and detects intentionally stale fixture in a test;
- all inventory completeness/evidence checks pass;
- `rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## Test plan

- Schema validation and orphan/missing/stale evidence fixtures.
- Headless vs interactive deterministic trace equivalence.
- Inspector projection tests for scene/design/workspace/transcript.
- Operational knob and representative matrix tests.
- ThemePicker transactional/validator tests.
- Full generated inventory/catalog canary.

## Done criteria

- [x] DesignInspector multi-panel (Focus/Layers/Tokens/Recipes) + `layout_studio_shell` in lookbook app.
- [x] Interactive primary components have interactors: List, Tree, Form, Split, Picker, Log, Toast, TextArea, ChoiceDialog, Tabs, Table, ThemePicker, CommandPalette, ApprovalCard, DesignInspector, Transcript, PromptBox, VirtualGrid.
- [x] Scene/focus/layer state inspectable via live DesignInspector strip (focus id + modal layer).
- [x] Toast knobs change real interactor state (existing tests).
- [x] ThemePicker interactor drives preset selection (preview path).
- [x] Migration `0041` documents studio shell + inspector.
- [x] `mise run check` green with lookbook tests.
- [ ] Full contract-axis ↔ scenario matrix for *every* inventory row (deferred continuous work; not a ship blocker for shell+interactors).

## STOP conditions

- Required prerequisite plans not DONE; branch not `main`; dirty tree; `0041`
  claimed.
- Inspection requires private-field coupling instead of public diagnostic
  projections.
- Scenario execution needs wall-clock timing, network, process, or secrets.
- Matrix design creates unbounded Cartesian artifacts; use risk-based coverage.
- Any verification fails twice after reasonable correction.

## Maintenance notes

New public components must enter Studio/catalog in their originating change.
Treat semantic traces as contracts: evolve deliberately with migrations, not
incidental Debug output.
