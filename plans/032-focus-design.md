# Cross-widget focus design: per-frame registry with scoped restoration

## Recommendation

Graduate a per-frame `FocusRing<Id, ScopeId>` modeled after painted hit-region
registration. Consumers register stable focus identities, rectangles, and
enabled state in traversal order every frame. The ring persists only the
focused identity, previous order needed for reconciliation, current painted
registrations, and a scope/restore stack. It never retains widgets or a widget
tree.

The lookbook prototype rewires sidebar, preview, and dynamic controls through
this registry. Tab and BackTab have one owner. A synthetic `ChoiceDialog`
opens with `m`, pushes a modal scope coordinated with `ModalStack`, skips a
disabled middle action, traps traversal, and restores the exact opener on
close. All focused container chrome is projected through
`PanelEmphasis::Focused`; border glyphs never change.

## Requirements from real surfaces

| Requirement | Lookbook gallery | Form screen | Dialog over content | Decision |
|---|---|---|---|---|
| Stable identity | Sidebar, preview, controls survive layout changes | Form has one screen-level identity; field IDs remain Form-owned | Dialog actions need stable IDs | Generic caller identity; never index-only state |
| Registration | Controls appear only for interactive stories | Whole Form registers once | Active modal actions register in top scope | Per-frame `id + Rect + enabled + scope` |
| Order | Sidebar → preview → controls | Screen order places Form among sibling controls | Action declaration order | Registration order is canonical traversal order |
| Disabled/removed targets | Missing controls must disappear cleanly | Form internally skips disabled fields | Disabled action is never focused | Filter disabled; reconcile removed focus to nearest surviving order slot |
| Traversal | Tab/BackTab cycle and wrap | Form consumes its internal field navigation while focused | Tab/BackTab wrap only within modal | Ring owns inter-widget Tab; widget state owns intra-widget navigation |
| Directional movement | Rectangles exist, but gallery does not need it | Form already owns arrow policy | Action row already has local left/right behavior | Defer spatial navigation; preserve `Rect` evidence |
| Scopes | One screen scope | One screen scope | Nested modal scopes trap input | Stack; only top scope is eligible |
| Restore | Dynamic target removal chooses nearest | Leaving/re-entering preserves stable Form ID | Close restores opener; sub-modal close restores parent action | Each pushed scope saves parent focus |
| Pointer focus | Clicking a painted pane transfers focus | Form handles its internal regions | Modal pointer never leaks to background | Registry hit-tests enabled targets in active scope |
| Projection | Exactly one interactive panel has bright semantic border | Form receives `active`; surrounding Panel receives emphasis | Active dialog uses focused role; background panels inactive | `is_focused` + `panel_emphasis_for`; no glyph/weight change |

`Rect` is registered even though first release supports only ordered traversal.
That preserves evidence for a later spatial-navigation design without choosing
distance heuristics now.

## Exactly two evaluated shapes

Scores are 1–5; higher is better.

| Shape | Dynamic sets | Scope/trap | Borrowed immediate mode | API simplicity | Per-frame cost | Total |
|---|---:|---:|---:|---:|---:|---:|
| 1. Per-frame registry | 5 | 5 | 5 | 4 | 3 | 22 |
| 2. Declared static rings | 2 | 4 | 4 | 5 | 5 | 20 |

### 1. Per-frame focus registry

The registry mirrors the proven `HitRegion` pattern. Dynamic controls,
permissions, responsive layouts, and modal actions express their current truth
directly. Reconciliation sees disappearance and disabled state instead of
forcing every consumer to patch indices. The cost is one small registration
vector per frame; registration count is screen-sized, not data-row-sized.

This shape won the prototype. The lookbook controls target appears and
disappears with story capability without adding a special traversal branch.

### 2. Declared-order static rings

```rust
const SCREEN_RING: &[FocusId] = &[
    FocusId::Sidebar,
    FocusId::Preview,
    FocusId::Controls,
];
```

Static rings are excellent for fixed button strips but cannot represent a
temporarily absent controls pane or disabled runtime action without parallel
filtering and restore glue. That recreates today's fragmented ownership.
Reject as the state model. Keep a convenience `register_order` helper that
registers const identities with per-ID area/enabled tuples.

## Winner API specification

Indicative public surface:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTarget<Id, ScopeId> {
    pub id: Id,
    pub scope: ScopeId,
    pub area: Option<Rect>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusScope<Id, ScopeId> {
    pub id: ScopeId,
    restore: Option<Id>,
    restore_index: Option<usize>,
}

pub struct FocusRing<Id, ScopeId> {
    focused: Option<Id>,
    targets: Vec<FocusTarget<Id, ScopeId>>,
    previous_targets: Vec<FocusTarget<Id, ScopeId>>,
    scopes: Vec<FocusScope<Id, ScopeId>>,
    pending_restore_index: Option<usize>,
}
```

Core operations:

```rust
impl<Id: Clone + Eq, ScopeId: Clone + Eq> FocusRing<Id, ScopeId> {
    pub fn new(root_scope: ScopeId, focused: Option<Id>) -> Self;
    pub fn begin_frame(&mut self);
    pub fn register(&mut self, target: FocusTarget<Id, ScopeId>);
    pub fn register_order(
        &mut self,
        scope: ScopeId,
        targets: impl IntoIterator<Item = (Id, Option<Rect>, bool)>,
    );
    pub fn attach_region(&mut self, scope: &ScopeId, id: &Id, area: Rect) -> bool;
    pub fn reconcile(&mut self) -> FocusOutcome<Id>;
    pub fn handle_key(&mut self, key: KeyEvent) -> FocusOutcome<Id>;
    pub fn request_focus(&mut self, id: Id) -> FocusOutcome<Id>;
    pub fn focus_at(&mut self, position: Position) -> FocusOutcome<Id>;
    pub fn focused(&self) -> Option<&Id>;
    pub fn is_focused(&self, id: &Id) -> bool;
    pub fn panel_emphasis_for(&self, id: &Id) -> PanelEmphasis;
    pub fn active_scope(&self) -> &ScopeId;
}
```

`begin_frame` preserves the previous registrations, then clears current
targets. Reconciliation filters both current and previous registrations by the
active scope, so repeated IDs or targets in another scope cannot distort the
nearest index. A pushed scope also captures the opener's parent-scope ordinal;
that durable hint survives any number of modal frames and reconciles a removed
opener to its nearest sibling. `register` diagnoses duplicate `(scope, id)`
pairs in debug builds and keeps the first registration in every build.
`register_order` requires a distinct optional painted rectangle and enabled
value for every ID; shared geometry cannot make only the first target
pointer-reachable. Composite widgets may register ordered eligibility before
render with `None`, then attach their canonical state-produced hit regions
after render. `focus_at` ignores targets whose region is not attached.
`reconcile` follows these rules:

1. Ignore every target outside the top scope and every disabled target.
2. Preserve the focused stable ID when still eligible.
3. If it disappeared, take its previous order index and clamp that index into
   the new eligible set. This selects the nearest survivor deterministically.
4. If no previous target existed, select the first eligible target.
5. If the active scope has no eligible target, focus is `None`; Tab is consumed
   but changes nothing so it cannot escape a modal trap.

`FocusOutcome` should distinguish `Ignored`, `Changed { from, to }`, and
`Unchanged`; callers can request redraw without comparing state manually. Only
Tab and BackTab belong in the initial `handle_key`. Arrow/directional behavior
is intentionally absent.

### Focus-visible projection

`panel_emphasis_for` is the general replacement for
`FocusOwner::panel_emphasis_for`. It returns only `Normal` or `Focused`.
`Panel` then selects `Role::Border` or `Role::BorderFocused` using the same
single-line glyphs. The registry never returns border glyphs, weights, styles,
or theme colors. Widget flags such as `FormState::active`,
`TabsState::focused`, and `SplitPaneState` divider focus are assigned from
`is_focused`.

## ModalStack integration contract

Focus and modal lifecycle must change atomically. The library build should add
operations on `FocusRing` that accept the existing `ModalStack`; it must not
create a second modal stack:

```rust
ring.open_modal(&mut modals, modal, modal_scope);
ring.open_submodal(&mut modals, child, child_scope);
ring.pop_modal(&mut modals);
ring.clear_modals(&mut modals);
```

- Root open saves the current screen ID and pushes the modal scope.
- Sub-modal open saves the current parent-modal ID and pushes another scope.
- Pop closes exactly the current modal and restores the saved parent ID.
- Clear closes the chain and restores the root opener, not an intermediate
  modal action.
- While a scope is active, traversal, programmatic requests, and pointer focus
  reject targets outside it.
- After the next registration pass, reconciliation handles an opener that no
  longer exists by choosing its nearest surviving sibling.

The lookbook prototype pairs `ModalStack::open/pop` and scope `push/pop` in
single private methods. It registers ChoiceDialog action order/enabled state
before render, then attaches the exact `ChoiceDialogState::regions` produced by
the widget's canonical ActionBar layout. It routes left-button activation
through the top scope and swallows background clicks. Consumers must never
reimplement label-width, gap, or action-row geometry. The build makes lifecycle
pairing impossible to forget via the atomic API above.

## Intra-widget versus inter-widget boundary

The focus ring treats a composite widget as one screen target. Form continues
to own its field ring, validation-aware skips, scrolling, and activation while
`FormState::active` is projected from the screen ring. ChoiceDialog is the
exception only in the sense that its actions are the modal scope's direct
targets; it already exposes stable action IDs.

List/Tree rows are selection, not screen focus. TextInput owns cursor/editing
only while its containing screen target is focused. Tabs may keep arrow-key
tab selection while the tab strip is one inter-widget focus target. This rule
prevents a retained cross-widget tree and avoids a common focusable trait on
every widget.

## Migration story

The build is one forward-only redesign with the next migration file:

- `FocusState<Id>` is absorbed by `FocusRing<Id, ScopeId>`; consumers replace
  `set` calls with registration plus `request_focus`.
- `FocusOwner<Tab>` is deleted. Consumers define stable IDs for tab strip and
  content, register them, and use `panel_emphasis_for`/`is_focused`.
- `ButtonFocus` is deleted. Fixed button strips use `register_order`; dynamic
  or disabled actions register individually.
- `ModalStack` remains the sole modal lifecycle type and gains atomic scoped
  focus coordination.
- No deprecated aliases, parallel rings, or compatibility facade remain.

The live contract matrix marks focus caller-owned for passive render surfaces
(`Dialog`, `DiffView`, `LogPane`, `MessageDialog`, and `Viewport`). Graduation
deliberately retains that classification: `FocusRing` removes bespoke routing,
but the consumer still registers screen composition and projects focus into
those widgets. Flipping them to `covered` would falsely claim widget-owned
interaction. Existing `ChoiceDialog`, Form, List, SplitPane, Tabs, TextInput,
and Tree focus claims remain backed by their widget-state stories; the lookbook
app deterministically tests shared-ring traversal, scoped trapping, canonical
ChoiceDialog regions, and restoration.

## Prototype verification checklist

- Sidebar → preview → controls cycles and wraps through Tab/BackTab.
- A story with no controls registers only sidebar and preview; traversal skips
  the absent target and reconciliation moves removed controls focus to preview.
- `m` opens a ChoiceDialog and pushes the modal scope.
- Modal Tab moves Continue → Cancel, skipping the visible disabled Unavailable
  action; another Tab wraps to Continue.
- Background pane focus is inactive while the modal owns focus.
- Esc pops the modal and restores the exact sidebar/preview/controls opener.
- Sidebar, preview, controls, and dialog all retain single-line Panel geometry;
  only semantic border role changes.
- Pointer focus uses registered painted rectangles and cannot cross the active
  modal scope.
- SVG render/check is unchanged because the terminal app prototype is outside
  static story rendering.

## Build-plan stub

1. Add generic `FocusTarget`, `FocusScope`, `FocusRing`, and typed outcomes to
   `interaction`; test wrap, reverse, dynamic reconcile, duplicates, empty
   scopes, disabled targets, pointer focus, and nested restoration.
2. Add two-phase canonical-region attachment for composite widgets and atomic
   `ModalStack` coordination; test Unicode/custom-gap action geometry plus
   nested open/pop/clear.
3. Migrate first-party focus projections; delete `FocusState`, `FocusOwner`,
   and `ButtonFocus` in the same breaking change.
4. Move lookbook from local prototype to library API; retain caller-owned
   contract rows where screen-level registration remains consumer-owned.
5. Add migration documentation with before/after edits, regenerate public API
   and component docs/previews, then run full gate and PTY checklist.

## Open questions

- Spatial navigation needs measured evidence for geometry scoring, overlap,
  RTL, and wrapped layouts. `Rect` registration preserves the seam; do not ship
  arrows yet.
- Screen-reader/AT integration is future research. Current terminal
  accessibility remains visible semantic focus plus non-color cues.
- If profiling shows registration allocation material, reuse vector capacity
  inside `FocusRing`; do not replace dynamic truth with static declarations.
