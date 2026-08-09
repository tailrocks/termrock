# Plan 040: Make one interaction scene own focus, actions, and overlays

> **Executor instructions**: Execute in order. Run each verification before
> continuing. STOP on any condition listed below; do not preserve old APIs as
> a fallback. Update only this plan's status row when complete.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/interaction crates/termrock/src/keymap.rs crates/termrock/src/widgets/command_palette.rs crates/termrock/src/widgets/jump_overlay.rs crates/termrock/src/widgets/completion_menu.rs crates/termrock/src/widgets/agent.rs crates/termrock-lookbook docs/api docs/content/docs MIGRATING.md migrations`
>
> Compare any changed file against "Current state." Semantic drift is a STOP.
> Begin only after Plan 039 is committed and `rtk proxy mise run gate` is green.

## Status

- **Priority**: P0
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plan 039
- **Category**: architecture, UX, bug, tests
- **Planned at**: commit `16b0ee8`, 2026-08-09

## Why this matters

TermRock currently has separate truths for focus, semantic hit geometry,
overlay order, Escape peeling, raw-key interpretation, and hint discovery.
Those truths can disagree. A non-dismissible top overlay already allows the
host to dismiss a lower layer, and consumers must manually keep focus scopes,
overlay markers, enabled actions, and hints synchronized. A component library
cannot deliver shadcn-like composability while each consumer writes that glue.

## Current state

- `interaction/scene.rs` registers only `id`, `Rect`, `focusable`, `enabled`,
  and a broad role. It has no layers, scopes, actions, or input ownership.
- `interaction/overlay_controller.rs` wraps `OverlayHost` and `EscCascade` but
  those remain separately public and mutable.
- `OverlayHost::dismiss_top_esc` searches backward for any dismissible layer.
  Therefore Esc may remove a lower menu behind a non-dismissible top dialog.
- `FocusRing` independently rebuilds focus targets and scopes each frame.
- `Keymap` already maps chords to typed actions and can derive hints; preserve
  that strength rather than creating a second key-binding registry.
- `UiIntent` is a small list vocabulary. It is useful as a default action
  family, but cannot represent every component action or application command.
- `CompletionMenu::handle_key` can move then commit on the same Down/Up key;
  routing ownership must make activation explicit.
- Product policy remains caller-owned. The scene reports actions such as
  dismiss, submit, or activate; it never quits, executes, or persists.
- Migration `0032` belongs to Plan 039; this plan owns `0033`.

## Target contract

Build one immediate-mode `InteractionScene<Id, ScopeId, Action>` per frame.
Registration order plus explicit layer establishes paint/input order. Each
element supplies stable identity, rect, focus scope, enabled/focusable state,
semantic role, input ownership, and currently available typed actions.

The scene must:

- reconcile focus from the same registrations used for hit testing;
- route pointer events from topmost eligible element downward;
- route keyboard actions to the focused element inside the top input-owning
  scope;
- dismiss only the top layer when that layer's policy permits it;
- return unhandled Escape at the bottom so consumers own quit policy;
- derive discoverable actions for hint bars and command palettes;
- preserve focus when elements reorder, disappear, disable, or close;
- rebuild per frame; it is not a retained widget tree or callback graph.

Use borrowed registrations and caller-owned action payloads where practical.
Do not store closures, effects, widget state, or product domain models.

## Scope

**In scope**:

- all modules under `crates/termrock/src/interaction/`;
- `crates/termrock/src/keymap.rs` only where action dispatch/discovery needs a
  canonical bridge;
- vertical integrations for JumpOverlay, CommandPalette, CompletionMenu, and
  ApprovalCard;
- HintBar/action discovery integration;
- lookbook runtime/interactors and deterministic interaction traces;
- architecture/component docs, generated API inventory, migration `0033`.

**Out of scope**:

- terminal event-loop ownership or async effects;
- application quit/draft/work policy;
- a retained DOM, callbacks, global singleton, or runtime widget tree;
- workspace layout (Plan 042) and transcript virtualization (Plan 041);
- compatibility exports for `EscCascade`, `OverlayController`, or parallel old
  scene contracts.

## Git workflow

- Work directly on `main`; STOP on any other branch.
- Use Conventional Commits and `rtk git commit -s`.
- Add `Co-authored-by: Codex <codex@openai.com>`.
- Every commit must independently pass `rtk proxy mise run check`; push `main` only after
  `rtk proxy mise run gate` passes.
- Prefer one breaking commit so source, migration, docs, catalog, and generated
  API inventory cannot describe different interaction models.

## Steps

### Step 1: Specify invariants as failing model tests

Add table/model tests for:

1. duplicate stable IDs reject registration deterministically;
2. disabled/hidden elements never receive focus or pointer actions;
3. later/top layers win hit testing, including overlapping rectangles;
4. non-dismissible top overlay blocks Escape from all lower overlays;
5. dismissing a top overlay restores the prior valid focus target;
6. nested modal/menu stacks peel exactly one layer per Escape;
7. outside click applies only to the top layer's policy;
8. removed/reordered targets preserve focus by stable ID;
9. active-scope actions exactly match generated hints and palette entries;
10. Press/Repeat/Release policy produces one activation, never two;
11. resize followed by pointer routing uses current-frame geometry only;
12. unhandled Escape reaches the caller without inventing Quit.

Use a pure model harness, not terminal timing. These tests should expose the
current lower-overlay dismissal bug before implementation.

**Verify**: `rtk cargo test -p termrock interaction --all-features --locked` →
new invariant tests fail only for documented old behavior.

### Step 2: Replace parallel registries with one scene model

Define cohesive types, names adjusted only for Rust ergonomics:

- `InteractionElement<Id, ScopeId, Action>`: identity, rect, scope, layer,
  role, state flags, input policy, borrowed available actions.
- `InteractionLayer<LayerId>`: stable identity, modal/input ownership, Escape
  and outside-click policies, optional focus-return target.
- `InteractionScene`: per-frame registrations plus focus/layer state needed
  across frames.
- `InteractionOutcome`: ignored, focus changed, action dispatched, layer
  dismissed, or caller-unhandled.

Use one begin/register/reconcile/route lifecycle. Registration must fail loudly
in debug/tests on duplicate IDs or unknown scopes/layers; release behavior must
remain deterministic. Rectangles are current-frame data. Stable focus and
layer history may persist across frames.

Fold useful `FocusRing`, `OverlayHost`, `OverlayController`, and `EscCascade`
behavior into this model, then remove their public parallel APIs and exports.
Do not add deprecated aliases.

**Verify**: interaction unit/model tests pass; `rtk proxy rg -n 'pub use .*EscCascade|pub use .*OverlayController' crates/termrock/src` returns no legacy exports.

### Step 3: Make Keymap the single chord-to-action source

Add a bridge that asks a context's `Keymap<Action>` to resolve a key, then asks
the scene whether that action is available in the active scope. The same
availability projection must feed HintBar and CommandPalette.

Rules:

- Release is ignored unless an explicit text/input contract requires it.
- navigation may accept Repeat; destructive/confirm actions are Press-only;
- unavailable actions are neither dispatched nor advertised;
- conflicts are detected when composing keymaps, not resolved by registration
  order;
- `UiIntent` may remain a standard action vocabulary/adaptor, never a second
  raw-key parser beside Keymap.

Add tests proving a disabled action disappears from dispatch, hints, and
palette together.

### Step 4: Migrate one complete overlay vertical slice

Migrate JumpOverlay, CommandPalette, CompletionMenu, and ApprovalCard so each:

- registers semantic geometry/actions during render;
- consumes routed typed actions, not its own competing Escape stack;
- returns product-neutral outcomes;
- restores focus through the scene when closed;
- publishes only actions actually valid for current state.

Fix CompletionMenu's Down/Up move-and-commit ambiguity structurally: movement
never commits; explicit activation does. Add nested scenarios such as palette
over completion and confirmation over palette.

### Step 5: Make the lookbook prove interaction contracts

Extend stories with deterministic scripts: focus traversal, pointer overlap,
nested overlays, non-dismissible top layer, disabled action, resize, and Escape
to caller. Record semantic traces containing focused ID, active layer, action,
and outcome; do not snapshot opaque internal structs.

Update contract evidence so keyboard, pointer, focus, narrow, and non-color
axes reference named scenario IDs. Regenerate previews and API inventory.

**Verify**:

- `rtk cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` → pass.
- `rtk cargo test -p termrock-lookbook --all-features --locked` → pass.

### Step 6: Document the break and run the full gate

Write `migrations/0033-v0.12.0-unified-interaction-scene.md` with removed
types, replacements, exact consumer lifecycle, keymap bridge, before/after
overlay example, ownership table, and validation commands. Update
`MIGRATING.md`, architecture docs, public API, and component docs.

**Verify**:

- `rtk proxy mise run check` → exit 0.
- `rtk proxy mise run gate` → exit 0.
- `rtk git status --short` → only Plan 040 files before commit; clean after.

## Test plan

- Pure scene model tests for focus/layer/action invariants and random operation
  sequences.
- Widget tests for the four migrated components.
- Lookbook semantic traces for keyboard, mouse, nesting, and resize.
- One warmed hot-path test asserting route/reconcile complexity is linear in
  registered visible elements and performs no per-event heap allocation after
  capacity warmup.
- Miri/proptest only if already available in repository tooling; do not add a
  test framework solely for this plan.

## Done criteria

- [ ] One scene is authoritative for focus, hit geometry, layers, and actions.
- [ ] Non-dismissible top overlays protect every lower layer.
- [ ] Escape peels one eligible top concern or returns unhandled.
- [ ] Keymap dispatch, hints, and palette use identical availability.
- [ ] Stable IDs preserve focus across reorder/resize/removal.
- [ ] Four vertical-slice components use the new contract.
- [ ] No public parallel focus/overlay/Escape compatibility facade remains.
- [ ] Migration `0033`, docs, stories, traces, contracts, previews, and API
      inventory are fresh.
- [ ] `rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## STOP conditions

Stop and report if:

- Plan 039 is not DONE or the baseline gate is red.
- Branch is not `main`, source worktree is dirty, or migration `0033` is taken.
- Current Keymap no longer provides typed action lookup and hint derivation.
- A proposed design requires stored callbacks, application effects, or domain
  policy inside TermRock.
- A retained DOM is required to satisfy routing; the agreed model is
  immediate-mode registration.
- Any verification fails twice after a reasonable correction.

## Maintenance notes

- Plans 041 and 042 must register transcript/workspace surfaces through this
  scene, not invent local hit/focus stacks.
- Plan 043 will attach design recipes to semantic roles and needs deterministic
  focused/disabled/action state from this scene.
- Keep layer order explicit. Registration order may break ties inside a layer;
  it must never silently become the modal policy.
