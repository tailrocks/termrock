# Plan 008: Agent-surface wave — actor accents, rails, presence motion

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/widgets/transcript.rs crates/termrock/src/widgets/tool_call_card.rs crates/termrock/src/widgets/subagent_card.rs crates/termrock/src/widgets/prompt_composer.rs crates/termrock/src/widgets/permission.rs crates/termrock/src/widgets/terminal_run_card.rs crates/termrock/src/patterns/plan_review.rs`
> Expect churn from plans 001–007 in shared modules; the widgets above must
> still exist under these names. On rename/移动, STOP.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED (flagship surfaces; visual contract changes)
- **Depends on**: plans/001, 004, 005, 006, 007
- **Category**: tech-debt (flagship visual quality)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

Agent tooling is TermRock's flagship use case, and per-actor accent systems
are the proven premium pattern for it (Grok Build: every scrollback block
carries a semantic accent rail — user gray, assistant magenta, plan golden,
error red — with a sin² wave flowing down active rails; collapsed tool rows
read `◆ Verb (dim details)`). TermRock's agent widgets predate the actor
roles (plan 001), the AccentRail primitive (plan 007), and the motion kit
(plan 006). This wave makes the agent surfaces express them, and proves the
public API is sufficient by recreating three Grok Build signature surfaces
as lookbook stories.

## Current state

Widgets in this wave (all under `crates/termrock/src/widgets/` unless
noted):

- `transcript.rs` — conversation blocks (`TranscriptBlock`,
  `TranscriptKind` — verified names via lookbook imports at
  `crates/termrock-lookbook/src/interactors.rs:20-25`).
- `tool_call_card.rs`, `subagent_card.rs`, `terminal_run_card.rs`,
  `working_state_card.rs` (in `patterns/`? verify:
  `ls crates/termrock/src/widgets | grep card` — `tool_call_card.rs` is a
  widget; `subagent_card.rs`, `terminal_run_card.rs`,
  `working_state_card.rs` live in `patterns/` per the tree at `539e7d03` —
  check both dirs and treat patterns files as patterns-side work).
- `prompt_composer.rs`, `permission.rs` — widgets.
- `patterns/plan_review.rs` — plan approval composite.
- Roles available after plan 001: `ActorUser`, `ActorAssistant`,
  `ActorThinking`, `ActorTool`, `ActorPlan`, `ActorSystem`, plus
  `Success`/`Danger`.
- Primitives after 006/007: `AccentRail` (wave-capable), dot-pulse spinner
  tier, `blend_toward`, glyph catalog (`◆`, status dots).

Read each widget's paint path + its lookbook story before editing — this
plan intentionally does not inline their excerpts (they are large and
plans 001–007 will have shifted line numbers); the binding references are
the design constraints below.

Design constraints (design SoT `docs/design/component-visual-richness-plan.md`
§4.2, §8 wave 2; Grok Build evidence in §3.1):

- Actor accent mapping: user→`ActorUser`, assistant→`ActorAssistant`,
  thinking→`ActorThinking`, tool→`ActorTool`, plan→`ActorPlan`,
  system→`ActorSystem`, error→`Danger`, success→`Success`.
- Rail replaces box for transcript blocks: 1-col rail + content, not a
  bordered panel per message.
- Tool rows: `◆ Verb (dim parenthetical)` one-line collapsed form; expanded
  keeps the rail.
- Thinking content: fg blended ~70% toward canvas (`blend_toward`),
  collapsible with 3-line preview.
- Presence: active blocks animate the rail (wave); background/subagent
  activity uses the quiet dot-pulse spinner, not braille.
- Plan approval: `ActorPlan` golden accent on rail + title.
- Reduced motion (`Motion` reduction): static rail, no wave, spinners drop
  to a static glyph — must be provably identical across ticks.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Contracts | `mise run contracts` | exit 0 |

## Scope

**In scope**:

- `crates/termrock/src/widgets/`: `transcript.rs`, `tool_call_card.rs`,
  `prompt_composer.rs`, `permission.rs`, `streaming_markdown.rs` (only if
  transcript embeds it for thinking blocks — check imports)
- `crates/termrock/src/patterns/`: `subagent_card.rs`,
  `terminal_run_card.rs`, `working_state_card.rs`, `plan_review.rs`,
  `agent_workbench.rs` (story-level integration only — patterns compose
  widgets)
- Lookbook stories for every touched surface + 3 new Grok-parity stories
- `migrations/0268-*.md` + `MIGRATING.md`
- `plans/README.md`

**Out of scope**:

- Kernel/interaction changes (focus, overlays, intents).
- Non-agent widgets (plan 009/010).
- New roles or primitives — if one is missing, that's a STOP (gap goes back
  to plans 001/006/007), not an inline invention.

## Git workflow

`main`, `git commit -s`; commit per widget/surface. Suggested prefix:
`feat(widgets)!: transcript actor rails` etc.

## Steps

### Step 1: Transcript actor rails

Map `TranscriptKind` → actor role; render each block through
`AccentRail::layout` (rail + content) instead of any current per-block
border/prefix chrome. Active (streaming) block: `.active(true).tick(t)` —
tick must come from the widget's existing frame/tick input (find how
transcript stories animate today; if transcript has no tick input, add one
following the `FrameTick` vocabulary). Thinking blocks: content styles
blended 70% toward `Role::Canvas` bg color; collapsed shows 3-line preview +
disclosure glyph.

**Verify**: `cargo nextest run -p termrock transcript` → pass with updated
expectations; new tests: `blocks_carry_actor_rails` (rail cell fg per kind),
`reduced_motion_rail_is_static` (two ticks, identical buffers).

### Step 2: Tool-call cards

Collapsed row: `◆` (catalog diamond, `ActorTool` color) + verb
(`TextStrong`) + parenthetical details (`TextMuted`), disclosure affordance
from the catalog. Expanded: rail continues down the card; output region on
`Role::Sunken` fill (code-block-like well). Running state: rail wave +
dot-pulse in the row.

**Verify**: `cargo nextest run -p termrock tool_call` → pass; test
`collapsed_row_shape` asserts diamond + dim details.

### Step 3: Subagent / terminal-run / working-state cards (patterns)

Same vocabulary: dot-pulse while running, `Success`/`Danger` status dot when
done, one-line collapsed summary, rail on expanded detail. These are
patterns — compose public widgets/primitives only (boundary law: patterns
may use widgets; never fork widget paint inside patterns).

**Verify**: `cargo nextest run -p termrock subagent terminal_run working_state`
→ pass.

### Step 4: PromptComposer + Permission

- PromptComposer: input region on `Role::Sunken` fill with `❯` prompt prefix
  glyph (catalog), focus border law unchanged; dim info line below in
  `TextMuted` (only touch paint, not editing logic).
- Permission: trust chrome adopts rail + severity icon per plan 005 toast
  conventions (muted frame, accent rail carries severity); approval actions
  are plan-004 chips.

**Verify**: `cargo nextest run -p termrock prompt_composer permission` → pass.

### Step 5: Plan review golden accent

`patterns/plan_review.rs`: `ActorPlan` rail + title accent; per-step rows
via `FieldRow` where they are label/value shaped; approve/reject actions as
chips.

**Verify**: `cargo nextest run -p termrock plan_review` → pass.

### Step 6: Grok-parity stories (acceptance)

Three new lookbook stories composed **only from public TermRock APIs** (no
private helpers — they prove API sufficiency):

1. `agent/tool-block-parity` — tool-call block with rail + wave while
   running, collapsed/expanded toggle.
2. `agent/plan-approval-parity` — golden plan block with inline step list +
   action chips.
3. `agent/turn-status-parity` — one-line turn status: spinner + activity
   verb + elapsed (dim) + token count glyph + stop chip.

If any story needs a hack (hardcoded color, private import), that is a STOP
— the missing capability gets reported, not worked around.

**Verify**: stories render in lookbook; deterministic preview check passes
(`mise run gate` includes it).

### Step 7: Migration + gate

`migrations/0268-v0.13.0-agent-surfaces-actor-accents.md`: transcript block
chrome change (border→rail), tool-card row shape, composer/permission
chrome, reduced-motion contract; before/after per surface. Link from
`MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0.

## Test plan

Per-step tests named above (6+ new). Every touched surface: mono/ASCII +
reduced-motion story variants re-verified (capability ladder law). Model
paint tests on the widget's existing test module.

## Done criteria

- [ ] `mise run check` + `mise run gate` exit 0
- [ ] `grep -rn "AccentRail" crates/termrock/src/widgets/transcript.rs crates/termrock/src/widgets/tool_call_card.rs` → present
- [ ] 3 parity stories exist and use only public API (`grep -n "use termrock::" crates/termrock-lookbook/src/stories.rs` style imports only)
- [ ] Reduced-motion determinism test passes
- [ ] `migrations/0268-*.md` exists, linked
- [ ] `plans/README.md` updated

## STOP conditions

- A needed capability is missing from plans 001/006/007 outputs (role,
  glyph, helper) → report the gap; do not hardcode.
- Transcript has no tick/frame input and adding one requires runtime/kernel
  changes → report design question.
- A patterns file would need widget-internal access → boundary violation;
  report.

## Maintenance notes

- The 3 parity stories become permanent regression anchors — reviewers
  should treat any later diff to them as a visual-contract change.
- Consumer note for the migration: Jackin's transcript-adjacent surfaces
  should adopt rails on next pin; its spinner-frame copy becomes deletable
  after plan 006.
