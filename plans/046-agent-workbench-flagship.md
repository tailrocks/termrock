# Plan 046: Ship the Agent Workbench flagship composition

> **Executor instructions**: Integrate existing primitives; do not move model,
> process, permission, or persistence policy into TermRock. Execute in order.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets/agent.rs crates/termrock/src/widgets/transcript.rs crates/termrock/src/patterns/agent_shell.rs crates/termrock/src/interaction crates/termrock/src/style crates/termrock-lookbook docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 039–045 are DONE and the full gate is green.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plans 039–045
- **Category**: flagship feature, composition, UX, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — render_agent_workbench composes Transcript/Prompt/Status/Approval + scene (migration 0040)

## Why this matters

Amp, Grok Build, and standout agent TUIs feel coherent because transcript,
tools, permissions, plan/build mode, task progress, composer, context, status,
and overlays behave as one workspace. TermRock has most paint primitives but no
canonical composition proving the whole framework. A flagship is both a useful
block and the integration test for shadcn-quality contracts.

## Current state

- Agent widgets are concentrated in `agent.rs` and can render independently.
- Plan 041 supplies variable-height Transcript; Plan 042 supplies workspace;
  Plans 039/040/043–045 supply safe approval, scene, tokens, intents, anatomy.
- AgentShell is currently a geometry pattern rather than one controlled neutral
  experience.
- Consumers must own agent events, tool execution, permissions, plan semantics,
  persistence, secrets, and wording. TermRock owns interaction/presentation.
- This plan owns migration `0039`.

## Target contract

Create `AgentWorkbench` (or evolved AgentShell) as a controlled composition:

- central variable-height transcript with follow/fold/search;
- prompt composer with explicit submit/newline/cancel outcomes;
- mode ribbon/status projection (caller-defined modes, not fixed product enum);
- task/activity rail and context/token meter projections;
- tool/activity/detail blocks using composed anatomy;
- fail-safe ApprovalCard as top scene layer;
- standard product-neutral QuestionFlow, PlanReview, TaskRail, and SessionPicker
  components from `docs/design/component-anatomy-spec.md`;
- command/completion overlays and discoverable actions;
- responsive workspace degradation to focused stack at narrow widths.

State owns only focus/layout/folds/selection/drafts if the consumer delegates
them. Effects and product policy remain typed outcomes.

## Scope

**In scope**: split/reorganize agent module; workbench config/state/outcome;
standard neutral projections; scene/workspace/design integration; exhaustive
scripted flagship stories; docs/contracts/API/migration `0039`.

**Out of scope**: LLM SDK, agent runtime, shell/process execution, permission
policy, storage, clipboard, filesystem/network, product wording/branding,
compatibility facade for geometry-only AgentShell.

## Git workflow

Clean `main` only. Conventional Commit, DCO signoff, Codex co-author trailer.
Each commit independently green; push only after full gate.

## Steps

### Step 1: Define ownership and integration laws

Add compile/model tests proving workbench config borrows caller projections and
outcomes never execute effects. Lock: one InteractionScene, one WorkspaceState,
one Transcript scroll truth, safe approval default, top-layer Escape, stable IDs
through streamed reorder, no hidden action hints, and responsive focus survival.

### Step 2: Split agent components into coherent modules

Move transcript/tool/approval/prompt/timeline/activity components under
`widgets/agent/` or another clear public boundary. Remove duplicate neutral
render bodies superseded by Plans 039/041/045. Preserve product-neutral names
and generated component inventory for every public piece. No re-export aliases
for removed APIs. Adopt the binding names PromptComposer, PermissionPrompt, and
ToolCallCard where they replace thin legacy names; implement QuestionFlow,
PlanReview, TaskRail, and SessionPicker as reusable compositions.

### Step 3: Build controlled workbench state and projections

Define borrowed projections for transcript blocks, activity/task rows, modes,
context meter, prompt suggestions, and active approval. Define typed outcomes:
submit draft, cancel/dismiss, mode requested, task/tool activated, approval
confirmed/cancelled, context action, layout/follow/fold/search changed.

Consumers map their domain models into these projections and perform effects.
TermRock coordinates focus, overlays, layout, hit geometry, tokens, and state.

### Step 4: Implement responsive composition

Wide: transcript + optional activity/context rail + composer/status. Medium:
rail collapses to stack/tab. Narrow: focused surface and composer remain; mode/
status use priority anatomy. Approval/command/completion are scene layers and
never clipped behind workspace leaves. Panel focus law and design recipes apply.

### Step 5: Build the definitive scripted demo

Create deterministic fixtures and scenarios:

1. streamed assistant/tool blocks while following;
2. user scroll detaches, search/fold, End rejoins;
3. completion menu, command palette, nested approval;
4. high-risk untouched Enter cannot approve; explicit select + confirm can;
5. plan/build mode request and task/activity update projection;
6. widths 120→80→40→20→120 preserve stable focus/anchor;
7. compact/cozy, alternate theme, ASCII/no-color/reduced motion;
8. error/retry and interrupted/cancelled activities;
9. semantic trace shows focus, layer, available action, outcome, block IDs.

This story becomes a release canary and documentation centerpiece, not a
hard-coded product demo.

### Step 6: Migrate and gate

Write `migrations/0039-v0.12.0-agent-workbench.md` with removed AgentShell/agent
surfaces, projections/outcomes, exact consumer integration, ownership table,
before/after event loop, validation commands. Update MIGRATING, component/
pattern docs, contract evidence, previews/traces, public inventory.

**Verify**: focused workbench + all prerequisite regression tests; 10k transcript
fixture; lookbook check; `rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## Test plan

- Compile/ownership tests for borrowed projections and neutral outcomes.
- Full integration model tests for scene/workspace/transcript/design.
- Approval safety and overlay nesting regressions.
- Responsive/Unicode/capability render matrix.
- Deterministic flagship scripts and warmed hot-path allocation tests.

## Done criteria

- [x] One controlled flagship composes all shared experience layers.
- [x] No product policy/effect/domain state enters TermRock.
- [x] Transcript, prompt, tools, approvals, rail, modes, overlays compose.
- [x] Responsive widths preserve focus/anchor and expose valid actions only.
- [x] Flagship scripts prove safety, streaming, nesting, and design axes.
- [x] Migration `0039`, docs, contracts, stories, traces, previews/API fresh.
- [x] Geometry-only/duplicate agent paths removed; full gates pass.

## STOP conditions

- Any Plan 039–045 not DONE; branch not `main`; dirty tree; `0039` claimed.
- Required API stores consumer models, executes effects, or defines product
  permission/mode policy.
- Workbench needs a second scene/workspace/transcript state truth.
- Deterministic fixtures cannot represent the experience without external I/O.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Use the flagship as integration canary for future scene/design/transcript
changes. Product-specific agent packs belong in consumers or source registry.
