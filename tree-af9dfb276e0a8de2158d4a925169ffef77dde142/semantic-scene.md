# SemanticScene (frame-local semantic tree)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0079-v0.13.0-semantic-scene.md` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `InteractionScene` focus/layers/Esc/mouse | **Preserve** — sole input authority |
| `InteractionElement` + actions + layers | **Preserve** — interaction registration |
| `SemanticRole` (Content/Control/Overlay/Chrome) | **Extend** with richer roles |
| Private thin `SemanticScene` / `SemanticElement` | **Replace** with full public tree API |
| `HitRegion` / `HoverState` | **Preserve** — low-level geometry helpers |
| `JumpOverlay` + `assign_jump_badges` | **Migrate** — optional build from semantic tree |
| `DesignInspector` panels | **Extend** — Semantics panel + snapshot lines |
| Retained widget DOM / a11y server | **Out of scope** — frame-local only |

## Mission

Rebuild a lightweight semantic tree every frame alongside paint. Use it for
hit discovery, focus navigation aids, generated help, jump mode, Studio
inspection, semantic snapshots, remote clients, and AI-readable UI state —
without replacing Ratatui immediate mode.

## API sketch

```rust
pub enum SemanticRole {
    Content, Control, Overlay, Chrome,
    // extended:
    Button, Input, List, ListItem, Tree, Table, Tab,
    Dialog, Menu, Status, Heading, Image, Progress, Custom,
}

pub struct SemanticState {
    pub selected: bool,
    pub expanded: bool,
    pub checked: bool,
    pub busy: bool,
    pub invalid: bool,
    pub pressed: bool,
}

pub struct SemanticNode<Id, Action = ()> {
    pub id: Id,
    pub parent: Option<Id>,
    pub role: SemanticRole,
    pub label: Option<String>,       // or Cow / &'static in hot path helpers
    pub description: Option<String>,
    pub area: Rect,
    pub focusable: bool,
    pub disabled: bool,
    pub hidden: bool,
    pub state: SemanticState,
    pub actions: Vec<Action>,
}

pub struct SemanticScene<Id, Action = ()> {
    // flat Vec + parent links; rebuild each frame
}

impl SemanticScene {
    pub fn begin_frame(&mut self);
    pub fn register(&mut self, node) -> Result<(), SemanticError>;
    pub fn register_child(&mut self, parent, node) -> Result<(), SemanticError>;
    pub fn hit_test(&self, pos) -> Option<&SemanticNode>; // includes disabled (inspection)
    pub fn hit_test_interactive(&self, pos) -> Option<&SemanticNode>;
    pub fn focus_order(&self) -> impl Iterator;
    pub fn children_of(&self, id) -> impl Iterator;
    pub fn path_to(&self, id) -> Vec<&Id>;
    pub fn help_lines(&self) -> Vec<String>; // label + actions
    pub fn jump_regions(&self) -> Vec<HitRegion<Id>>; // focusable visible
    pub fn snapshot(&self) -> SemanticSnapshot; // string ids for remote/AI
    pub fn diagnostics(&self) -> &[SemanticDiagnostic];
    pub fn from_interaction(scene: &InteractionScene) -> Self; // adapter
}

pub struct SemanticSnapshot {
    pub nodes: Vec<SemanticSnapshotNode>,
    pub collisions: Vec<String>,
}

pub enum SemanticError {
    DuplicateId,
    UnknownParent,
    SelfParent,
}
```

## Laws

1. **Frame-local:** `begin_frame` clears nodes; no retained DOM across frames.
2. **Stable ids:** host owns identity; scene reports collisions, does not invent ids.
3. **InteractionScene remains sole focus authority** — SemanticScene does not own focus.
4. **Cheap:** O(n) register/hit; virtualized views register only painted rows.
5. **Adapters:** `from_interaction` projects interaction elements into semantic nodes.

## Studio

`DesignInspector` gains `InspectorPanel::Semantics` and accepts optional
`SemanticSnapshot` summary lines (counts, collisions, focusable path).
