# Pre-1.0 public API redesign

**Status:** deliberate breaking-change proposal (not yet implemented)  
**Scope:** every public module / major type in `termrock` + `termrock-lookbook`  
**Authority:** Agents.md forward-only; hybrid kernel + source-owned registry  
**Inventory basis:** `docs/api/public-api.txt` (~13.8k lines), crate modules as of migrations `0001`–`0051`  
**Non-goal:** compatibility facades, deprecated aliases, parallel old/new public APIs

---

## 0. Diagnosis (why break hard)

The public surface grew by **accretion**: each v0.12 slice added a better abstraction **beside** the older one.

| Symptom | Evidence |
|---------|----------|
| Dual authorities | `FocusRing` + `InteractionScene`; `Theme` + `DesignTokens` + `DesignSystem`; `OverlayHost` (private) + `OverlayStack`; `ModalStack` still public |
| Dual product widgets | `PromptBox` vs `PromptComposer`; `ApprovalCard` vs `PermissionPrompt`; `StreamView` vs `Transcript` |
| Quad data grids | `Table`, `VirtualGrid`, `DetailTable`, `data_view::*` models with no single consumer path |
| Crate-root dump | `lib.rs` re-exports interaction/layout/capability/perf/style wholesale → 13k-line flat namespace, no ownership signal |
| Scroll free-fn sprawl | ~25 public scroll helpers + near-duplicates in `scroll::render` |
| Chrome enum clones | `PanelEmphasis` (widget) vs `PanelChrome` (tokens) |
| Focus ownership split | Widget `*State.focused` **and** scene focus — two truths |
| Architecture doc lag | `architecture-foundation.md` still cites `OverlayHost` + `FocusRing` as kernel contracts |

**Principle:** one authority per concern. Weak public abstractions die even if lookbook still uses them.

**Distribution rule (binding):**

| Lives in crate (kernel) | Source-installed (registry / app-owned) |
|-------------------------|----------------------------------------|
| Interaction scene, overlay stack, intents | Agent chrome skins, approval wording layouts |
| Design tokens / roles / recipes | Brand themes, phosphor preset packaging |
| Unicode text, scroll math, input, session | App keymaps, vim collection maps |
| Neutral primitives: Panel, List, Tree, Text*, Dialog shell | Patterns (`AgentShell`, Workbench, OpsDashboard, …) |
| Data presentation models + one DataTable | ToolCard / Timeline product chrome |
| Capability detection + doctor | Showcase / Studio app code |

---

## 1. Inventory matrix

Legend: **P** reserve · **R** ename · **M** ove · **S** plit · **D** esign · **X** remove · **I** nstall (source) · **Dep** deprecate-then-remove (one milestone only, no long shim)

### 1.1 Crate root (`lib.rs`)

| Surface | Action | Target |
|---------|--------|--------|
| `pub mod {ansi_text, capability, input, interaction, keymap, layout, osc, patterns, perf, runtime, scroll, style, text, widgets, crossterm}` | **P** (modules) | Keep module tree; tighten interiors |
| Blanket `pub use interaction::{…}` / `layout::{…}` / `capability::{…}` / `perf::{…}` / `style::{…}` | **X** | Consumers import from modules. Root keeps **only**: `DesignSystem`, `Event` (via `input`), `run` (via `runtime`) if desired — preferably none |
| Doc: “Entry point: Theme” | **R** | Entry: `DesignSystem` + `InteractionScene` |

### 1.2 `style`

| Surface | Action | Target |
|---------|--------|--------|
| `Role` | **P** (+ small **S** if needed) | Kernel semantic roles; keep exhaustiveness tests |
| `Theme` | **R** + **D** | → `RolePalette` (Role → Style map only). Not frame authority |
| `DesignTokens` | **M** into `DesignSystem` | Collapse nested wrapper |
| `DesignSystem` | **D** | **Single** paint authority: palette + density + motion + glyphs + spacing + selection + capability |
| `Density`, `Motion`, `GlyphSet`, `SelectionChrome`, `SpacingScale` | **P** | Nested under `DesignSystem` / `style::tokens` |
| `PanelChrome`, `PanelRecipe`, `ListRowRecipe` | **P** | Recipes stay; only chrome enum for panels |
| `Rgb`, `color()`, phosphor constants (`PHOSPHOR_*`, `PREVIEW_CARD`, …) | **M** / **I** | `style::palette` module; brand constants → source theme pack (phosphor default stays in crate as `DesignSystem::phosphor()`) |
| `faded` | **P** | Utility |
| `ColorCapability`, `quantize_*` | **P** | Called via `DesignSystem::quantize` |
| `Appearance`, `AppearanceThemeMap`, `theme_for_appearance` | **R** | `Appearance` **P**; map returns `DesignSystem`; rename fn |
| `CapabilityPreviewHost` + preview types | **M** | `capability::preview` (not style) |

### 1.3 `interaction`

| Surface | Action | Target |
|---------|--------|--------|
| `InteractionScene`, layers, elements, outcomes, `SceneError` | **P** + **D** (absorb focus) | **Sole** focus + hit + layer + Esc authority |
| `UiIntent`, `NavigationMove`, `PageMove` | **P** | Expand as needed; widgets take intents |
| `default_list_intent` / `_table_` / `_tree_` | **R** | `keymap::defaults::{list,table,tree}` or `intent::defaults` |
| `OverlayStack`, `OverlaySpec`, `OverlayPolicy`, placement types, `place_overlay` | **P** + **S** | Placement math → `layout::overlay`; stack owns law |
| `OverlayId`, `OverlayKind` (stack) | **P** | Single public definition |
| Private `overlay::{OverlayHost, OverlayId, OverlayLayer}` | **X** | Delete after stack migration complete |
| `FocusRing`, `FocusTarget`, `FocusOutcome` | **X** public | Logic folded into scene; lookbook migrates first |
| `ModalStack`, `classify_click`, `render_backdrop` | **X** / **M** | Backdrop → `OverlayStack` / `layout`; ModalStack **X** |
| `EscCascade`, `OverlayController` | **X** public (already private) | Stay internal until deleted |
| `SemanticScene`, `SemanticElement`, `SemanticRole` | **S** / **D** | Projection/query API on `InteractionScene` (or `scene.semantic_view()`), not parallel stack |
| `HitRegion`, `HoverState` | **P** | Hover may move to scene later; keep for widget local use |
| `Outcome<T>` | **R** | `widgets::Outcome` or `interaction::WidgetOutcome` — stop dual name with scene outcomes |
| `dispatch_keymap_action` | **M** | `keymap::dispatch` |

### 1.4 `input` / `keymap`

| Surface | Action | Target |
|---------|--------|--------|
| Neutral `Event`, key/mouse types | **P** | Kernel |
| `Keymap`, `KeyBinding`, `KeyChord`, `Visibility` | **P** | Kernel |
| `keymap::glyph` | **P** | Hint glyphs |
| App-default agent/vim maps | **I** | Registry packages |

### 1.5 `layout`

| Surface | Action | Target |
|---------|--------|--------|
| Responsive: `ViewportClass`, `ContractionStage`, `ResponsiveSurface`, … | **P** | Kernel grammar |
| `WorkSurface`, `RegionSpec`, workspace tree | **P** | Kernel |
| `DialogSpec`, `resolve_dialog`, `centered_rect` | **M** / unify with overlay placement | One placement API |
| `Slots` | **R** | → `ShellSlots` or `VerticalSlots` (clash with `PanelSlots`) |
| `bottom_rows` | **P** | |
| `render_dialog_shell`, `render_scrollable_dialog_body` | **M** | `widgets::dialog` paint helpers or `layout::dialog` |

### 1.6 `scroll`

| Surface | Action | Target |
|---------|--------|--------|
| Math: max offset, apply delta, track position, thumb geometry | **S** | `scroll::math` — single implementations |
| Paint: `render_scrollbar`, line offset render | **S** | `scroll::paint` |
| Policies: `TailScroll`, `DialogScroll`, follow/anchor | **S** | `scroll::policy` |
| Duplicate `apply_scroll_delta` / `apply_delta_u16` / render variants | **X** merge | One typed API |
| `scroll_selectable_list`, `scroll_hint_spans` | **M** | Widget or keymap defaults |
| `Measured`, `ScrollAxes`, `ScrollAxis`, `ScrollDelta`, `ScrollSpan` | **P** after cleanup | |

### 1.7 `runtime` / `crossterm`

| Surface | Action | Target |
|---------|--------|--------|
| `run`, `RunOptions`, `FrameTick` | **P** + minor **D** | Integrate `CapabilitySet` resolve at enter; keep closure runner |
| `Session`, `SessionOptions` | **P** | Independent options stay |
| `CrosstermBackend` re-export | **P** | Feature-gated |

### 1.8 `capability` / `perf`

| Surface | Action | Target |
|---------|--------|--------|
| Profiles, doctor, `resolve_capabilities`, env hints | **P** | Kernel |
| Stream coalescer, budgets, follow mode, dirty flags | **P** | Kernel kits; widgets **must** use them on hot paths |
| `data_view_bench` public bench module | **M** | `#[cfg(feature = "bench")]` or tests only |

### 1.9 `text` / `ansi_text` / `osc`

| Surface | Action | Target |
|---------|--------|--------|
| Display cols, clip, sanitize, fixed-prefix segments | **P** | Unicode kernel |
| `ansi_text` | **M** | Under `text::ansi` (drop top-level mod if desired) |
| OSC encode + request types | **P** | |

### 1.10 `patterns`

| Surface | Action | Target |
|---------|--------|--------|
| `layout_agent_shell`, `layout_agent_workbench`, ops, resource, studio | **I** | Source-owned blocks; crate may keep **one release** as `patterns` then remove from public crate or feature `patterns` deprecated |
| Interim | **Dep** | Feature-gate `patterns` immediately after Studio has copies |

### 1.11 Widgets — primitives (kernel)

| Surface | Action | Target |
|---------|--------|--------|
| `Panel`, `PanelSlots` | **P** + **R** chrome | Drop `PanelEmphasis`; use `PanelChrome` only |
| `List`, `ListRow`, `ListState`, `RowRole` | **P** + **D** state | Selection/scroll only; focus from scene |
| `Tree`, `Tabs`, `SplitPane`, `Progress` | **P** | |
| `TextInput`, `TextArea`, edit core | **P** | |
| `Dialog`, size, open/dismiss helpers | **P** + stack-only | No non-stack modal path |
| `Picker`, `CommandPalette`, `CompletionMenu` | **P** | OverlayStack clients |
| `HintBar` + hint helpers | **P** | Driven by `Keymap` |
| `ActionBar`, `StatusBar` | **P** | |
| `Toast` | **P** | |
| `Viewport` | **R** / **D** | Clarify vs `Panel` content area; or fold into Panel content helper |
| `Selection` | **P** | Shared multi-select model |
| `EmptyState`, `LoadingView`, `ErrorView`, `Skeleton`, `Banner` | **P** | View-state chrome |
| `JumpOverlay` | **P** or **I** | Prefer overlay stack story; keep if domain-neutral |
| `ImageSurface` | **P** | Capability-gated |
| `CodeBlock`, `MarkdownView`, `DiffView` | **P** | |
| `LogPane` | **P** | Bound scrollback stays |
| `ComposedRow` | **P** | Anatomy |
| `Sparkline`, meters, bar charts | **P** | Neutral viz |

### 1.12 Widgets — data presentation

| Surface | Action | Target |
|---------|--------|--------|
| `data_view::{VirtualWindow, ColumnModel, SelectionModel, LoadState, …}` | **P** + **M** | Module `termrock::data` (not buried only under widgets) |
| `Table` + `TableState` + width solver | **D** → **`DataTable`** | Absorbs interactive grid |
| `VirtualGrid` | **X** as public name | Implementation detail / merge into DataTable virtualization |
| `DetailTable` | **I** or thin `DataTable` mode | Product detail layout → registry |
| `resolve_widths` | **M** | `data::resolve_widths` |

### 1.13 Widgets — agent / permission / prompt (dual stacks)

| Surface | Action | Target |
|---------|--------|--------|
| `PromptComposer` + state/outcome | **P** | Flagship |
| `PromptBox`, `PromptBoxState`, `PromptBoxOutcome` | **X** | Stories migrate to Composer |
| `PermissionPrompt`, queue, request model | **P** | Trust authority |
| `ApprovalCard*` | **X** crate public → **I** optional skin on PermissionPrompt |
| `Transcript` | **P** | Variable-height stream authority |
| `StreamView`, `StreamItem*` | **X** or **I** | Migrate to Transcript + item projection |
| `ToolCard`, `ThinkingBlock`, `TokenMeter`, `Timeline*` | **I** | Product chrome; kernel may keep minimal if truly neutral — default **I** |
| `ModeRibbon`, `PlanReview`, `QuestionFlow`, `SessionPicker`, `TaskRail`, `WorkbenchMode` | **I** | Agent blocks → `@termrock/agent` / registry |
| `session_picker_handle_key` free fn | **X** / absorb | Block-local |

### 1.14 Widgets — forms / misc

| Surface | Action | Target |
|---------|--------|--------|
| `Form`, `FormField`, `FormSection`, `FormState`, `FormOutcome` | **D** | Composition of labeled controls + scene focus order + validation display — not a second List |
| `ThemePicker`, presets | **I** | Theme pack + Studio; kernel keeps `DesignSystem` constructors |
| `DesignInspector` | **M** | Studio crate (`termrock-studio`), not general widgets |

### 1.15 Lookbook / Studio

| Surface | Action | Target |
|---------|--------|--------|
| `termrock-lookbook` story/interactor/svg | **D** → **Studio** | Story contract versioned; only public TermRock APIs |
| Focus helpers in lookbook | **X** FocusRing | Scene-only |
| Dual stories for PromptBox/ApprovalCard | **X** | Composer + Permission only |

### 1.16 Docs / contracts

| Surface | Action | Target |
|---------|--------|--------|
| `architecture-foundation.md` overlay/focus wording | **Update** | Scene + OverlayStack only |
| Component quality / handbook | **P** | Bind to new names in same milestones |
| `public-api.txt` | Regen each milestone | CI gate |

---

## 2. Break packages (detailed)

Each package is shippable as **one or more migrations** (see §4). No package leaves two public authorities for the same concern.

---

### Break A — Module boundaries and crate-root hygiene

#### What is structurally wrong
Root re-exports teach consumers that TermRock is a bag of types. Ownership (who owns focus? paint? placement?) is invisible. Public-api.txt becomes unreadable; accidental coupling thrives.

#### New API

```rust
// lib.rs — modules only (illustrative)
pub mod capability;
pub mod data;        // was widgets::data_view models
pub mod input;
pub mod interaction;
pub mod keymap;
pub mod layout;
pub mod osc;
pub mod perf;
pub mod runtime;
pub mod scroll;
pub mod style;
pub mod text;
pub mod widgets;

#[cfg(feature = "crossterm")]
pub mod crossterm;

// No blanket pub use.
```

#### Before / after

```rust
// Before
use termrock::{Theme, OverlayStack, UiIntent, DesignTokens, List};

// After
use termrock::style::{DesignSystem, Role};
use termrock::interaction::{OverlayStack, UiIntent};
use termrock::widgets::List;
```

#### Migration path
1. Stop adding root re-exports.  
2. One migration: delete root `pub use`; fix in-tree lookbook/tests/docs.  
3. Consumer migration file: search-replace import paths.

#### What becomes simpler
One import path per type; rustdoc modules match mental model.

#### New constraints
No `termrock::Theme` at root. CI may fail on new root re-exports (`rg` gate).

#### Required tests
- `public_api_no_root_reexports` (or snapshot of `lib.rs` exports).  
- Lookbook + lib tests compile with module paths only.

---

### Break B — Design system is the only paint authority

#### What is structurally wrong
Three nested “systems” (`Theme` ⊂ `DesignTokens` ⊂ `DesignSystem`) and widgets still take bare `&Theme`. Recipes duplicate chrome enums (`PanelEmphasis` / `PanelChrome`). Phosphor RGB constants leak as public API.

#### New API

```rust
pub struct RolePalette { /* Role → Style */ }

pub struct DesignSystem {
    pub palette: RolePalette,
    pub density: Density,
    pub motion: Motion,
    pub glyphs: GlyphSet,
    pub spacing: SpacingScale,
    pub selection: SelectionChrome,
    pub capability: ColorCapability, // resolved for this frame/session
}

impl DesignSystem {
    pub fn phosphor() -> Self { /* … */ }
    pub fn style(&self, role: Role) -> Style { self.palette.style(role) }
    pub fn panel_recipe(&self, chrome: PanelChrome) -> PanelRecipe { /* … */ }
    pub fn list_row_recipe(&self, selected: bool, focused: bool, enabled: bool) -> ListRowRecipe { /* … */ }
    pub fn quantize(self, cap: ColorCapability) -> Self { /* … */ }
}

// Widgets
impl<'a> Panel<'a> {
    pub fn new(system: &'a DesignSystem) -> Self { /* … */ }
    pub fn chrome(self, chrome: PanelChrome) -> Self { /* … */ }
}
```

**Removed public:** `DesignTokens` (merged), `PanelEmphasis`, crate-root color soup (moved to `style::palette` private or theme pack).

**Renamed:** `Theme` → `RolePalette` (or keep `Theme` as type alias **one** milestone then delete alias — prefer hard rename).

#### Before / after

```rust
// Before
let theme = Theme::default();
let tokens = DesignTokens::new(theme.clone(), Density::Comfortable);
let sys = tokens.design_system();
Panel::new(&theme).emphasis(PanelEmphasis::Focused);

// After
let system = DesignSystem::phosphor().density(Density::Comfortable);
Panel::new(&system).chrome(PanelChrome::Focused);
```

#### Migration path
1. Add `DesignSystem` methods covering all Theme uses.  
2. Convert every widget constructor/render from `&Theme` → `&DesignSystem` (or `&RolePalette` only where recipes unneeded — prefer always `DesignSystem`).  
3. Delete `DesignTokens`, `PanelEmphasis`, dual constructors.  
4. Update handbook + recipes.

#### What becomes simpler
One object to pass down the tree; density/glyphs affect list rows without extra params.

#### New constraints
Widgets **must not** hardcode phosphor RGB. Custom brands build `RolePalette` then `DesignSystem::from_palette`.

#### Required tests
- Role exhaustiveness + phosphor snapshot of roles.  
- `panel_recipe_focus_uses_border_focused_not_heavy_glyphs`.  
- `list_row_recipe_gutter_default_phosphor`.  
- Quantize path preserves non-color cues (selection gutter still present in mono).  
- Widget contract: render with `DesignSystem` only (no Theme param in public fn signatures).

---

### Break C — InteractionScene absorbs FocusRing; kill ModalStack public path

#### What is structurally wrong
Two focus stacks. Lookbook and older widgets still use `FocusRing`. Architecture docs still name `OverlayHost` + `FocusRing`. `ModalStack` is a third modal metaphor beside `OverlayStack`. `SemanticScene` parallels element registration.

#### New API

```rust
pub struct InteractionScene<Id, LayerId, Action> { /* focus + layers + elements */ }

impl<Id, LayerId, Action> InteractionScene<Id, LayerId, Action> {
    pub fn begin_frame(&mut self);
    pub fn register_layer(&mut self, layer: InteractionLayer<LayerId, Id>) -> Result<(), SceneError>;
    pub fn register(&mut self, el: InteractionElement<Id, LayerId, Action>) -> Result<(), SceneError>;
    pub fn focused(&self) -> Option<&Id>;
    pub fn set_focused(&mut self, id: Option<Id>);
    pub fn handle_key(&mut self, key: KeyEvent) -> InteractionOutcome<Id, LayerId, Action>;
    pub fn handle_pointer(&mut self, pos: Position, kind: PointerKind) -> InteractionOutcome<…>;
    /// Query projection (replaces SemanticScene as separate owner).
    pub fn semantics(&self) -> impl Iterator<Item = SemanticHit<'_, Id>>;
}

// OverlayStack remains for modal/popup policy + placement + esc law.
// App loop: scene + overlay stack cooperate; Esc: stack first, then scene layers.
```

**Removed public:** `FocusRing`, `FocusTarget`, `FocusOutcome`, `ModalStack`, public `SemanticScene` as independent authority (types may remain as view structs).

#### Before / after

```rust
// Before
let mut ring = FocusRing::new(Scope::Root, Some(id));
ring.begin_frame();
ring.register(FocusTarget { id, scope, area: Some(r), enabled: true });
ring.handle_key(key);

// After
let mut scene = InteractionScene::new();
scene.begin_frame();
scene.register_layer(root_layer)?;
scene.register(InteractionElement::new(id, root).area(r))?;
scene.handle_key(key);
```

#### Migration path
1. Lookbook `focus.rs` / interactors → scene.  
2. Any widget docs mentioning FocusRing → scene.  
3. Delete FocusRing module public exports; keep code private only if OverlayStack still needs a slice — prefer delete.  
4. ModalStack callers → OverlayStack.  
5. SemanticScene: implement `scene.semantics()`; remove dual registration APIs.

#### What becomes simpler
One `begin_frame` / register / handle path. Esc and focus restore share layer stack.

#### New constraints
Widgets **do not** store `focused: bool` as source of truth; they read `scene.is_focused(&id)` or receive `focused: bool` **from** the consumer for the frame. Consumer owns scene.

#### Required tests
- Scene tab order + pointer hit (existing + FocusRing parity cases).  
- Layer push/pop restores focus (parity with old FocusRing scopes).  
- Esc: overlay stack peels before scene UnhandledEscape.  
- No public FocusRing in `public-api.txt`.  
- Lookbook builds without focus.rs ring.

---

### Break D — Overlay single stack; placement lives in layout

#### What is structurally wrong
Historical `OverlayHost` types still in tree; placement split across `place_overlay`, `layout::resolve_dialog`, per-widget geometry. Easy to open dialogs without stack (Esc law bypass).

#### New API

```rust
// interaction
pub struct OverlayStack<FocusId> { /* … */ }
pub struct OverlaySpec<FocusId> { /* id, kind, policy, size, … */ }

// layout
pub fn place(spec: &PlacementSpec, viewport: Rect) -> Rect;

// widgets — only stack-backed openers
pub fn open_dialog(stack: &mut OverlayStack<F>, /* … */) -> OverlayOutcome<F>;
```

**Removed:** any public path that renders modal chrome without `OverlayStack` entry. Private host/controller deleted when unused.

#### Before / after

```rust
// Before — possible to use ModalStack + manual backdrop
modal.push(MyModal);
render_backdrop(frame, area);

// After
stack.push(OverlaySpec {
    id: OverlayId::from_static("confirm"),
    kind: OverlayKind::Dialog,
    policy: OverlayPolicy::modal_default(),
    ..
})?;
// render: for entry in stack.iter() { place + paint }
```

#### Migration path
Migrate remaining ModalStack/Host tests → OverlayStack (already mostly done for dialog/palette/completion). Delete dead modules.

#### What becomes simpler
One Esc law, one z-order, one focus trap policy.

#### New constraints
Every floating UI registers an `OverlayId`. Duplicate ids rejected.

#### Required tests
- Stack: push/replace/dismiss/esc peel (existing 0043).  
- Dialog/palette/completion/prompt fullscreen openers only via stack.  
- `rg OverlayHost|ModalStack` zero in public API.

---

### Break E — Events: intents first; keymaps dispatch intents

#### What is structurally wrong
Every widget has bespoke `handle_key(KeyEvent)`. `UiIntent` exists but is optional. Keymaps map to arbitrary `Action` enums per surface without a shared intent layer. Defaults are free functions at interaction root.

#### New API

```rust
// Preferred widget input
impl ListState<Id> {
    pub fn handle_intent(&mut self, intent: UiIntent, rows: &[Id]) -> ListOutcome<Id>;
    // Optional bridge for apps that have not adopted keymaps yet:
    pub fn handle_key(&mut self, key: KeyEvent, rows: &[Id]) -> ListOutcome<Id> {
        match default_list_intent(key) {
            Some(i) => self.handle_intent(i, rows),
            None => ListOutcome::Ignored,
        }
    }
}

// keymap
impl Keymap<UiIntent> {
    pub fn resolve(&self, key: KeyEvent) -> Option<UiIntent>;
}
pub mod defaults {
    pub fn list_map() -> Keymap<UiIntent>;
    pub fn table_map() -> Keymap<UiIntent>;
    pub fn tree_map() -> Keymap<UiIntent>;
}
```

#### Before / after

```rust
// Before
list_state.handle_key(key, &ids);

// After
let intent = app_keymap.resolve(key).or_else(|| defaults::list_map().resolve(key));
if let Some(intent) = intent {
    list_state.handle_intent(intent, &ids);
}
```

#### Migration path
1. Add `handle_intent` everywhere collections exist; keep `handle_key` as thin default bridge for one milestone.  
2. Next milestone: remove public `handle_key` from widgets that fully intent-cover (or keep as convenience forever — prefer keep thin bridge, **documented** as default map only).  
3. Move `default_*_intent` under `keymap::defaults` / `intent::defaults`.

#### What becomes simpler
Rebinding, testing (inject intents), vim packs as data.

#### New constraints
New interactive widgets must define intent coverage in contract matrix.

#### Required tests
- Intent tables for list/table/tree (existing 0038) expanded.  
- Keymap round-trip: Shown bindings ⊆ handled intents.  
- Unknown key → Ignored, never panic.

---

### Break F — Widget state: selection/scroll vs scene focus

#### What is structurally wrong
`ListState.focused`, hover, and hit regions duplicate scene/hover. Consumers must sync focus flags manually.

#### New API

```rust
pub struct ListState<Id> {
    // selection, multi-select, scroll offset, viewport metrics
    // NOT focused authority
}

impl ListState<Id> {
    pub fn set_pointer(&mut self, pos: Option<Position>); // local hover ok
    pub fn regions(&self) -> &[HitRegion<Id>];            // for scene register after paint
}

// Render
List { focused: bool, /* from scene */, .. }
// or
List::new(...).focused(scene.focused() == Some(&id))
```

#### Before / after

```rust
// Before
state.set_focused(true);
state.handle_key(key, ids);

// After
let focused = scene.focused() == Some(&list_id);
if focused {
    state.handle_intent(intent, ids);
}
List::new(rows).focused(focused).render(...);
// after paint: scene.attach_area(list_id, state.area());
```

#### Migration path
Deprecate `set_focused` / `is_focused` on widget state → remove. Document consumer pattern in handbook.

#### What becomes simpler
No double focus bugs; scene is inspector-visible truth.

#### New constraints
Stateless render param `focused: bool` required for chrome (BorderFocused).

#### Required tests
- List chrome role depends on passed `focused`, not internal flag.  
- Multi-widget app test: one BorderFocused at a time via scene.

---

### Break G — Panel / surface APIs

#### What is structurally wrong
`PanelEmphasis` vs `PanelChrome`. `layout::Slots` vs `PanelSlots`. `Viewport` role unclear vs Panel content. Multiple “surface” types (`WorkSurface`, `ResponsiveSurface`) without a doc map.

#### New API

```rust
// widgets
Panel::new(&system).chrome(PanelChrome::Focused).slots(PanelSlots { … });

// layout
pub struct VerticalSlots { /* renamed from Slots */ }
pub struct WorkSurface { /* … */ }
pub struct ResponsiveSurface { /* … */ }
// docs: WorkSurface = multi-region app chrome; ResponsiveSurface = contraction policy; Panel = single bordered container
```

**Remove:** `PanelEmphasis`, ambiguous `Viewport` if it only wraps Block — or rename to `ContentViewport` with scroll coupling.

#### Before / after

```rust
// Before
Panel::new(&theme).emphasis(PanelEmphasis::Focused).title("Files");

// After
Panel::new(&system)
    .chrome(PanelChrome::Focused)
    .slots(PanelSlots { title: Some("Files"), ..Default::default() });
```

#### Migration path
Rename + delete dual enum; fix patterns/lookbook; document surface taxonomy in handbook.

#### What becomes simpler
One chrome enum; slots naming no longer collides.

#### New constraints
Border weight never encodes focus (unchanged law).

#### Required tests
- Focused vs normal border roles (phosphor green vs gray).  
- Narrow `PanelSlots` drop order (existing).  
- Rename compile surface for `VerticalSlots`.

---

### Break H — Data presentation: one grid, one model kit

#### What is structurally wrong
Four public grids. Consumers cannot know which is canonical. `data_view` models are excellent but not wired as **the** Table path. `VirtualGrid` vs `Table` duplicate region/outcome shapes.

#### New API

```rust
// termrock::data
pub struct VirtualWindow { … }
pub struct ColumnModel<Id> { … }
pub struct SelectionModel<RowId> { … }
pub enum LoadState { … }
// sort/filter/copy/expand as today

// termrock::widgets
pub struct DataTable<'a, RowId, ColId> { /* virtualized grid */ }
pub struct DataTableState<RowId, ColId> { … }

// List remains for 1-D collections
// Tree remains hierarchical
```

**Remove public:** `Table`, `VirtualGrid`, `DetailTable` names (merge behaviors into DataTable modes: `plain`, `virtual`, `detail_projection` via row templates — not separate widgets).

#### Before / after

```rust
// Before
Table::new(&theme, &cols, &rows).render(area, buf, &mut table_state);
VirtualGrid::new(...).render(...);

// After
let columns = ColumnModel::new(cols);
DataTable::new(&system, &columns, rows)
    .window(&window)
    .render(area, buf, &mut state);
```

#### Migration path
1. Implement `DataTable` on top of VirtualGrid + Table best parts + data_view models.  
2. Migrate lookbook table stories.  
3. Delete old public types.  
4. Migration doc with type rename table.

#### What becomes simpler
One width solver, one selection model, one load/skeleton path, one copy payload.

#### New constraints
Large data **must** use `VirtualWindow` + perf budgets; no “load all rows into Table” public encouragement.

#### Required tests
- Width resolve parity with old `resolve_widths`.  
- Virtual window stability on append (streaming).  
- Selection model independent of scroll.  
- Contract matrix row for DataTable (quality standard).  
- Perf: steady-state zero-alloc check where promised.

---

### Break I — Form architecture

#### What is structurally wrong
`Form` is a navigable labeled list of pre-rendered `Line` values. It does not compose `TextInput` / `TextArea` / `Picker` / validation pipelines. Activation outcome forces apps to open editors out-of-band. Duplicates List navigation.

#### New API

```rust
pub enum FieldControl<'a, Id> {
    Display(Line<'a>),
    Text(&'a TextInputState),
    Area(&'a TextAreaState),
    // … custom via trait object or generic
}

pub struct FormField<'a, Id> {
    pub id: Id,
    pub label: Line<'a>,
    pub control: FieldControl<'a, Id>,
    pub help: Option<Line<'a>>,
    pub error: Option<Line<'a>>,
    pub required: bool,
    pub enabled: bool,
}

pub struct FormState {
    // scroll only; focus ids via scene registration of field ids
}

pub enum FormOutcome<Id> {
    Ignored,
    Submit,
    Cancel,
    FieldEdited(Id),
    // FocusChanged removed — scene owns it
}
```

#### Before / after

```rust
// Before
FormField::new(id, label, value_line).error(err);
// on Activated → app focuses external TextInput

// After
FormField {
    id,
    label,
    control: FieldControl::Text(&input_state),
    error: Some(err_line),
    ..
};
// scene registers each field id; focused field receives intents
```

#### Migration path
New Form beside old **not** allowed publicly. Replace in one milestone; migrate stories; apps that used Activated rebuild with composed controls.

#### What becomes simpler
Real forms; less glue; validation display unified.

#### New constraints
Domain validation stays app-owned; Form only displays error lines and routes input to child controls.

#### Required tests
- Tab order across fields via scene.  
- Disabled field skips focus.  
- Error role uses `InputInvalid` / danger text.  
- Narrow layout contraction of help/error.

---

### Break J — Kill dual agent surfaces

#### What is structurally wrong
Two prompts, two permissions, two streams. Quality and law (default-deny, composer policy) only on the new stack; old stack still exported and story-backed.

#### New API
- **Prompt:** only `PromptComposer`.  
- **Permission:** only `PermissionPrompt` + `PermissionQueue` + request model.  
- **Stream:** only `Transcript` (+ perf follow/coalesce kits).  
- **Product chrome:** `ToolCard`, `ThinkingBlock`, `Timeline`, `TokenMeter`, agent_blocks → **source-installed** `@termrock/agent` / registry.

#### Before / after

```rust
// Before
PromptBox::new(...).render(..., &mut prompt_box_state);
ApprovalCard::new(...).render(..., &mut approval_state);
StreamView::new(items).render(...);

// After
PromptComposer::new(&system, &mut composer_state).render(...);
PermissionPrompt::new(&system, &request, &mut perm_state).render(...);
Transcript::new(&system, blocks).render(..., &mut transcript_state);
// ToolCard: use installed source under src/ui/agent/tool_card.rs
```

#### Migration path
1. Lookbook: delete PromptBox/ApprovalCard/StreamView stories; add Composer/Permission/Transcript coverage if gaps.  
2. Remove types from `widgets/mod.rs`.  
3. Optionally publish registry items with last known source.  
4. Migration 00xx with replacements table.

#### What becomes simpler
One law for submit policy, one law for default-deny, one variable-height stream.

#### New constraints
No reintroduction of parallel “simple” prompt without Composer policy hooks.

#### Required tests
- Composer submit policy + large paste threshold (existing).  
- Permission default-deny + stale generation (existing).  
- Transcript anchor/follow (existing + StreamView parity cases ported).  
- `public-api.txt` free of PromptBox/ApprovalCard/StreamView.

---

### Break K — Keymaps packaging

#### What is structurally wrong
Kernel keymap is good; product maps and glyph tables risk bloating the crate. Bridge lives under interaction.

#### New API

```rust
// crate
termrock::keymap::{Keymap, KeyBinding, KeyChord, Visibility, dispatch};
termrock::keymap::defaults::{list, table, tree, dialog};

// source-installed
// termrock-keymap-agent, termrock-keymap-vim-collections
```

#### Migration path
Move `dispatch_keymap_action` → `keymap::dispatch`. Extract agent chords from widgets into install packs when agent blocks move.

#### Required tests
- Keymap shown ⊆ dispatch (property test).  
- Default list map matches `UiIntent` coverage.

---

### Break L — Scroll API consolidation

#### What is structurally wrong
~25 public entry points; `mod` vs `render` duplicates; widget-specific helpers mixed with pure math.

#### New API

```rust
pub mod scroll {
    pub mod math { /* max_offset, apply_delta, thumb, track_to_offset */ }
    pub mod paint { /* render_scrollbar, render_lines_with_offset */ }
    pub mod policy { /* TailScroll, DialogScroll, FollowMode glue with perf */ }
}
```

#### Before / after

```rust
// Before
scroll::apply_delta_u16(...);
scroll::render::apply_scroll_delta(...); // sibling duplicate

// After
scroll::math::apply_delta(...);
scroll::paint::scrollbar(...);
```

#### Migration path
Re-export old paths **not** allowed. Single rename migration + fix call sites in-tree.

#### Required tests
- Math golden tests (existing) under new paths.  
- No duplicate symbols in public-api.

---

### Break M — Patterns → source-installed; Studio absorbs lookbook

#### What is structurally wrong
Patterns are product layouts in the kernel crate. Lookbook is a gallery, not the harness described in `termrock-studio.md`. DesignInspector is a Studio concern exported as a general widget.

#### New API

```text
crates/termrock              — kernel only
crates/termrock-studio       — renamed/evolved lookbook: stories, knobs, inspector, svg gate
registry://blocks/agent-workbench — source layout_agent_workbench
registry://blocks/ops-dashboard
```

Story contract (Studio):

```rust
pub trait Story {
    fn id(&self) -> &str;
    fn render(&mut self, ctx: &mut StoryContext);
    fn handle(&mut self, event: Event, ctx: &mut StoryContext) -> StoryControl;
}
pub struct StoryContext {
    pub system: DesignSystem,
    pub scene: InteractionScene<…>,
    pub overlays: OverlayStack<…>,
    pub caps: EffectiveCapabilities,
    // knobs, recording, …
}
```

#### Before / after

```rust
// Before
use termrock::patterns::layout_agent_shell;
use termrock::widgets::DesignInspector;

// After
// src/ui/blocks/agent_shell.rs (installed)
// termrock_studio::inspector::DesignInspector
```

#### Migration path
1. Copy patterns into Studio fixtures / registry draft.  
2. Feature-gate `patterns` then remove.  
3. Move DesignInspector to studio crate.  
4. Story API migration for all interactors (DesignSystem + Scene).

#### What becomes simpler
Kernel size; product layouts forkable; Studio dogfoods public API only.

#### New constraints
Studio **must not** use `pub(crate)` termrock hooks. Missing API → fix kernel.

#### Required tests
- Studio/lookbook compile on public API only (enforce via separate crate dependency).  
- SVG snapshot gate green.  
- Story contract: every catalog component has ≥1 story (inventory test).

---

### Break N — Runtime / session + capabilities

#### What is structurally wrong
`run` is solid but session enter does not establish capability profile. Capability module parallel to session. Perf kits optional discipline.

#### New API

```rust
pub struct RunOptions {
    pub session: SessionOptions,
    pub poll_timeout: Duration,
    pub capabilities: CapabilityOverrides, // default detect
}

pub fn run<Model>(…) -> io::Result<RunHandle> {
    // resolve EffectiveCapabilities once; pass via FrameTick or context
}

pub struct FrameTick {
    pub now: Instant,
    pub frame: u64,
    // optional: caps handle / generation
}
```

#### Before / after

```rust
// Before
run(&mut model, RunOptions::default(), render, update, deadline);

// After
run(&mut model, RunOptions {
    capabilities: CapabilityOverrides::from_env(),
    ..Default::default()
}, |m, f, tick| {
    let sys = DesignSystem::phosphor().quantize(tick.caps.color);
    // …
}, update, deadline);
```

#### Migration path
Extend `FrameTick` carefully (breaking field add is ok pre-1.0). Wire doctor recommendation into Studio.

#### What becomes simpler
One place to resolve caps; widgets receive already-quantized `DesignSystem`.

#### New constraints
Detection never silent-fail to Modern when env says NO_COLOR.

#### Required tests
- Doctor + resolve integration (existing 0050) via RunOptions.  
- FrameTick mono-time properties (existing).

---

### Break O — Convert to source-installed (summary list)

| Item | Why install |
|------|-------------|
| `patterns::*` | App chrome |
| `ApprovalCard` (if kept at all) | Product wording |
| `ToolCard`, `ThinkingBlock`, `Timeline`, `TokenMeter` | Agent brand |
| `agent_blocks::*` | Agent product pack |
| `ThemePicker` + preset table | Brand themes |
| Phosphor marketing constants as copy-paste theme pack | Identity |
| Agent/vim keymaps | Muscle memory |
| Showcase workbench app | Dogfood app, not library |
| Optional: `JumpOverlay` product skins | |

Kernel keeps **behavior engines** those skins call (Panel, List, OverlayStack, Permission model, Transcript, PromptComposer).

---

## 3. What we deliberately preserve

| Keep | Reason |
|------|--------|
| Immediate-mode per-frame registration | Core model |
| Semantic `Role` + non-color cues | Accessibility law |
| Single-line panel borders; focus via role not weight | Agents.md focus-visible hierarchy |
| Stable IDs on interactive rows/fields | Hit/focus |
| Borrowed/projected render data | Ownership law |
| Neutral `Event` vocabulary | Backend independence |
| `Keymap` as SoT for hints + dispatch | Structural anti-divergence |
| `OverlayStack` Esc peel + policies | Overlay law |
| `UiIntent` for collections | Rebindable nav |
| Unicode display-column text helpers | Correctness |
| `Session` independent options | Testability |
| Closure `runtime::run` | Simple apps |
| Capability profiles + doctor | Progressive enhancement |
| Perf coalescer / budgets / follow | Streaming honesty |
| Component quality standard + handbook | Process |
| Forward-only migrations under `migrations/` | Agent-migratable |

---

## 4. Migration milestones (buildable sequence)

Each milestone: **one logical break**, `cargo test` green, lookbook/studio green, `public-api` regen, **one** new `migrations/00xx-…md` + `MIGRATING.md` row, commit on `main` (or allowed feat branch + merge), DCO.

| MS | Name | Breaks | Exit criteria |
|----|------|--------|----------------|
| **M0** | Inventory freeze | — | This doc merged; CI job lists dual-stack symbols as “scheduled delete” |
| **M1** | Root hygiene | A | No blanket root `pub use`; in-tree imports fixed |
| **M2** | DesignSystem paint | B (constructors) | All widgets take `&DesignSystem`; `Theme` renamed or type-alias phase; recipes use `PanelChrome` only |
| **M3** | Delete DesignTokens / PanelEmphasis | B (finish) | No DesignTokens/PanelEmphasis in public-api; handbook updated |
| **M4** | Scene focus authority | C | Lookbook on InteractionScene; FocusRing unexported; focus restore tests green |
| **M5** | Overlay-only modals | D | ModalStack/Host gone; all overlays on stack |
| **M6** | Intent-first collections | E | `handle_intent` on List/Tree/DataTable; defaults under keymap |
| **M7** | Widget focus fields removed | F | No `set_focused` on List/Table states; multi-panel scene test |
| **M8** | Panel/surface rename | G | `VerticalSlots`; Viewport clarified/removed |
| **M9** | DataTable land | H (add) | DataTable + data module public; old Table still present **only if** same milestone deletes — **prefer same MS delete** |
| **M10** | Remove Table/VirtualGrid/DetailTable | H (finish) | Single grid API; stories migrated |
| **M11** | Form redesign | I | New Form; old Activated-only form gone |
| **M12** | Agent dual removal | J | No PromptBox/ApprovalCard/StreamView; Composer/Permission/Transcript only |
| **M13** | Scroll modules | L | math/paint/policy split; duplicates gone |
| **M14** | Keymap home + packs | K | dispatch moved; agent maps extracted if blocks move |
| **M15** | Patterns feature-gate | M (start) | `patterns` behind feature; Studio copies layouts |
| **M16** | Studio crate | M (finish) | `termrock-studio`; DesignInspector moved; lookbook binary thin wrapper or renamed |
| **M17** | Runtime caps | N | RunOptions + FrameTick caps; doctor path documented |
| **M18** | Source-install pilot | O | ≥3 registry items (theme, keymap, agent block) installable per `source-owned-registry.md` |
| **M19** | Doc + architecture sync | — | foundation doc, COMPONENTS, handbook, competitive matrix names |
| **M20** | Public API budget | — | Size/goal gate: fail CI if new dual authorities reappear (`rg` denylist) |

**Parallelism note:** M1 free; M2–M3 sequential; M4–M5 sequential after M1; M6–M7 after M4; M9–M10 sequential; M12 after M5 (overlays) and independent of M9; M13 anytime after M1; M15–M18 after dual removals preferred.

**Never greenless:** do not merge a milestone that leaves both old and new **public** for the same concern. Internal `#[cfg]` during a branch is fine; `pub use old` is not.

---

## 5. Per-milestone test bundles (minimum)

| MS | Must pass |
|----|-----------|
| M1 | full lib + lookbook; public-api snapshot |
| M2–M3 | style token tests; all widget render smoke; phosphor role snapshots |
| M4–M5 | interaction scene + overlay_stack suites; esc law; lookbook interactors |
| M6–M7 | collection intent tests; focus chrome integration test |
| M8 | panel slots + layout rename tests |
| M9–M10 | data_view + DataTable contracts; virtual window; width solver |
| M11 | form scene focus + validation display |
| M12 | composer + permission + transcript suites; public-api denylist |
| M13 | scroll math goldens |
| M14 | keymap property tests |
| M15–M16 | studio SVG gate; public-API-only dependency |
| M17 | capability doctor + run options |
| M18 | registry install dry-run / digest verify (when CLI exists) |
| M20 | denylist: FocusRing, DesignTokens, PromptBox, ApprovalCard, StreamView, ModalStack, VirtualGrid, Table (old), PanelEmphasis |

---

## 6. Consumer migration playbook (summary)

1. Pin current rev.  
2. Walk `MIGRATING.md` from pin → head.  
3. Global import rewrites (module paths, DesignSystem).  
4. Replace FocusRing with InteractionScene.  
5. Replace Theme-only paint with DesignSystem.  
6. Replace PromptBox/ApprovalCard/StreamView/Table/VirtualGrid.  
7. Pass `focused` from scene into widgets; delete local focus flags.  
8. Move patterns into app tree or install from registry.  
9. Run app tests + `termrock doctor` mindset for caps.

---

## 7. Explicit non-goals of this redesign

- Stable 1.0 promise (still pre-stable after these breaks until a later freeze).  
- Multi-crate kernel split (`termrock-core` etc.) — optional later, not required for API clarity.  
- Windows/ConPTY completeness.  
- Keeping weak APIs because Tailrocks apps use them — apps pin and migrate.  
- Long deprecation windows or `#[deprecated]` forever aliases.

---

## 8. Decision log (short)

| Decision | Choice | Rejected |
|----------|--------|----------|
| Paint authority | `DesignSystem` only | Keep Theme as peer |
| Focus authority | `InteractionScene` only | FocusRing + Scene forever |
| Modal authority | `OverlayStack` only | ModalStack parallel |
| Grid | `DataTable` + `data::*` | Four widgets |
| Prompt | `PromptComposer` | PromptBox “simple” |
| Permission | `PermissionPrompt` | ApprovalCard dual |
| Stream | `Transcript` | StreamView dual |
| Patterns | Source-install | Eternal crate patterns |
| Root exports | Modules only | Convenience re-exports |
| Compatibility | Hard break + migration files | Facades |

---

## 9. Implementation order for the next agent

1. Land **this doc** (M0).  
2. Execute **M1** (root hygiene) — smallest, unlocks clarity.  
3. **M2–M3** DesignSystem (widest mechanical churn; do with automated rustc fix + tests).  
4. **M4–M5** interaction purity.  
5. **M12** dual agent delete (high confusion removal).  
6. **M9–M10** DataTable.  
7. Remaining milestones per dependencies in §4.

Do not start registry CLI before M12–M15 unless needed for pattern extraction.

---

## 10. Success metrics

| Metric | Target |
|--------|--------|
| Public dual authorities (listed denylist) | 0 |
| Crate-root re-exported types | 0 (or documented ≤3 entry points) |
| Interactive widgets with intent coverage | 100% of collections |
| Widgets taking `&Theme` only | 0 |
| Stories on FocusRing / PromptBox / ApprovalCard | 0 |
| `cargo test` + studio SVG gate | green every MS |
| Migration file per break | mandatory |

---

*End of proposal. Implementation requires separate commits per milestone; this file alone does not change the public API.*
