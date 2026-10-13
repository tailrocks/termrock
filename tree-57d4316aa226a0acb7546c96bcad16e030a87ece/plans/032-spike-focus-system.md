# Plan 032 (spike): Design the cross-widget focus system — traversal order, modal focus trap, restore-on-close

> **Executor instructions**: DESIGN SPIKE. Deliverable = design doc + a
> lookbook prototype proving traversal + trap + restore + a recommendation.
> Honor STOP conditions. Update the plans/README.md row when done.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/interaction/ crates/termrock/src/widgets/`
> Requires POST-011 event contract and POST-024 interaction cleanup (ModalStack
> state fix, single hover primitive). Verify both rows DONE.

## Status

- **Priority**: P3
- **Effort**: L (coarse — design-heavy spike)
- **Risk**: MED-HIGH for the eventual build (an over-framework-y focus tree would clash with the immediate-mode, borrowed-data grain); LOW for the spike
- **Depends on**: plans/011-event-model-convergence.md, plans/024-scroll-hover-support-api-unification.md
- **Category**: direction
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

Focus is a contract axis TermRock claims to own (AGENTS.md: TermRock owns "focus and navigation behavior") — but what ships is fragmentary: `FocusState<Id>` is a single-slot setter/getter; `FocusOwner<Tab>` is a ring hard-coded to ONE screen layout ("tab bars + content blocks"); `ButtonFocus` is a closed ring for one dialog strip; the contract matrix marks focus "caller-owned" on Dialog/DiffView/MessageDialog/Viewport; and `ModalStack` manages modal lifecycle with zero focus trapping or restore. Zero traversal code exists (no focus_next/tab_order/trap anywhere). Every consumer hand-wires Tab/Shift-Tab across heterogeneous widgets and re-implements "focus stays inside the modal, returns to the opener on close" — the most drift-prone glue in any TUI app. The design challenge: a focus registry that fits per-frame borrowed data (no retained widget tree) — that's why this is a spike.

## Current state

- `crates/termrock/src/interaction/mod.rs:13-32` (verbatim):

```rust
pub struct FocusState<Id> { focused: Option<Id> }
impl<Id> FocusState<Id> {
    pub const fn new(focused: Option<Id>) -> Self { ... }
    pub const fn focused(&self) -> Option<&Id> { ... }
    pub fn set(&mut self, focused: Option<Id>) { ... }
}
```

- `interaction/focus_owner.rs`: `FocusOwner<Tab>` — per-screen owner for the tab-bar+content layout; `ButtonFocus` ring via `RING` const + modular `next`/`prev`; `panel_emphasis_for`/`show_cursor_for` project focus into `PanelEmphasis`.
- `interaction/modal.rs` (post-024): `ModalStack<M>` — `open`/`open_sub`/`pop`/`clear_chain`/`take_current`; no focus knowledge. Plan 024 deliberately deferred focus-restore here.
- Per-widget focus facts: widgets carry `focused: bool` flags or focus in their state (`TreeState`, `FormState.active`, `TabsState.focused`, `SplitPaneState` divider focus) — set by the CONSUMER. `HitRegion<Id>`/`HoverState` (post-024 canonical) show the per-frame-registration pattern that traversal can mirror.
- Post-011: neutral `Event`, state-owned `handle_key(data, key) -> Outcome`; widgets do NOT implement a common focusable trait.
- Contract matrix rows with `focus: "caller-owned"`: Dialog, DiffView, MessageDialog, Viewport (`docs/api/component-contracts.json`).
- The lookbook gallery (post-018 runner) hand-routes focus between sidebar and preview panes — the in-repo consumer to prototype against.
- Design doctrine (AGENTS.md): borrowed/projected data, state types own interaction facts, no retained tree, no consumer-specific modes; forward-only.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Gallery | `cargo run -p termrock-lookbook` | prototype traversal visible |

## Scope

**In scope (spike)**:
- Design doc `plans/032-focus-design.md`
- Prototype INSIDE the lookbook (its sidebar/preview/interactor focus rewired through the prototype registry; a synthetic modal to prove trap/restore)
- Recommendation: the focus API + the migration story for `FocusState`/`FocusOwner`/`ButtonFocus`

**Out of scope**:
- Library changes (build plan follows).
- Screen-reader/AT integration (terminal a11y is cues-based here; note as future research only).
- Changing widget-internal focus flags — the registry coordinates BETWEEN widgets; widgets keep their `focused: bool` inputs.

## Steps

### Step 1: Requirements inventory from the three real surfaces

Document what focus actually must do, from: (a) the lookbook gallery (2-pane + interactors), (b) a Form screen (fields ring, per COMPONENTS.md Form owns intra-form focus — where's the boundary between intra-widget and inter-widget traversal?), (c) a dialog-over-content flow (trap + restore). Produce the requirement table: registration (what identifies a focusable — stable Id + Rect + enabled?), order (registration order? geometric? explicit indices?), traversal ops (next/prev/first/directional?), scopes (screen scope + modal scope stack), restore semantics, and the projection back into widgets (`focused: bool` inputs + `PanelEmphasis` — generalize `FocusOwner::panel_emphasis_for`).

### Step 2: Design — evaluate exactly two shapes

1. **Per-frame focus registry** (mirrors `HitRegion` registration): each frame, the consumer registers focusables `registry.register(id, rect, enabled)`; `FocusRing` state persists `focused: Option<Id>` across frames; `handle_key` on the ring consumes Tab/BackTab (+ optional arrows) → `Outcome`; reconciliation like `ListState::reconcile_count` when the registered set changes (focused id vanished → nearest). `FocusScope` = a stack pushed by modal open (trap = traversal restricted to top scope; restore = pop returns the saved id to the parent scope). Fits immediate-mode; per-frame Vec churn is the cost.
2. **Declared-order static rings** (generalize `ButtonFocus`): consumers declare `const` focus rings per screen (ids in order), the registry only tracks position + scope stack. Zero per-frame cost; cannot express dynamic sets (list rows appearing, disabled fields) without consumer glue — which is the status quo's weakness.

Score against Step 1's table + the doctrine (borrowed data, no retained tree). Expected winner: shape 1 with shape 2's `const` ergonomics as a convenience constructor — but let the prototype decide; record honestly.

### Step 3: Prototype in the lookbook

Rewire the gallery: sidebar, preview, and (when an interactor is active) the interactor's widget as three registered focusables; Tab cycles, the focused pane gets `PanelEmphasis::Focused` via the registry's projection helper; add a synthetic ChoiceDialog modal (open with `m`) proving: Tab is trapped to the dialog's actions while open; closing restores focus to whichever pane had it. Integrate with `ModalStack` (the scope push/pop rides `open`/`pop` — this is the focus-restore Plan 024 deferred).

**Verify**: manual checklist in the doc (cycle, trap, restore, disabled-skip); `cargo test --workspace` green; preview/determinism gates untouched (`check` green).

### Step 4: Design doc + build-plan stub

`plans/032-focus-design.md`: requirement table, two-shape evaluation + winner, full API spec (`FocusRing`/`FocusScope`/registration/reconciliation/projection), the `ModalStack` integration contract, the migration story (FocusState → absorbed? FocusOwner/ButtonFocus → reimplemented as registry instances or deleted; contract matrix rows flipping `caller-owned` → `covered` and WHICH widgets), intra-vs-inter widget boundary rule (Form keeps its internal field ring; the registry treats the whole Form as ONE focusable — or not; decide from the prototype), and open questions (directional/spatial navigation? focus-visible styling role?).

**Verify**: doc exists; README row updated with the winner one-liner.

## Done criteria

- [ ] `plans/032-focus-design.md`: requirements, two-shape evaluation, winner API spec, ModalStack contract, migration story
- [ ] Lookbook prototype demonstrates cycle + trap + restore + disabled-skip
- [ ] Zero `crates/termrock/src/` changes; all gates green
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plans 011/024 not DONE — stop, dependencies.
- The prototype forces widget-internal changes to be usable (a widget can't express "I'm focused" from outside) — record exactly which widget/flag as build-plan input and stop extending the prototype around it.
- Scope semantics conflict with `ModalStack`'s post-024 contract — report; don't fork a second modal concept.

## Maintenance notes

- The eventual build flips contract-matrix focus rows — each flip needs story evidence (a focus story per affected widget), same pattern as Plan 023's axis rule.
- `FocusOwner`/`ButtonFocus` deletion is a breaking change with a migration file — the build plan, not the spike.
- Directional (spatial) navigation is the recorded future tier; the registry's Rect registration deliberately keeps the door open.
