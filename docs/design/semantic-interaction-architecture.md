# Semantic interaction architecture

**Status:** design SoT (target architecture; partially implemented on HEAD)  
**Supersedes:** treating per-widget `handle_key(KeyEvent)` and dual `FocusRing`/`InteractionScene` as complete  
**Builds on:** `UiIntent`, `Keymap`, `InteractionScene`, `OverlayStack`, `dispatch_keymap_action`, `default_*_intent`  
**Related:** [`pre-1.0-api-redesign.md`](./pre-1.0-api-redesign.md) Breaks C–F, AGENTS cross-surface consistency  
**Constraint:** terminal-native — cells, chords, CSI/kitty protocols, mouse hits, no DOM focus model

---

## 0. HEAD gap (what exists vs target)

| Piece | HEAD today | Target |
|-------|------------|--------|
| Semantic intents | `UiIntent` with Move/Page/Activate/Toggle/Open/Close/Cancel/Submit/Expand/Collapse | Full intent catalog below + axis-aware Move |
| Key → intent | Free fns `default_list/table/tree_intent` | Layered `KeymapStack` presets (default/vim/emacs/app) |
| Keymap | `Keymap<A>` with remap, conflicts, hint spans | `Keymap<UiIntent>` + app command maps + context merge |
| Scene | `InteractionScene` elements/layers/tab/esc | Scene owns **FocusGraph** + intent dispatch |
| Focus | Dual: public `FocusRing` + scene focus | **FocusGraph** sole public focus authority |
| Mouse | Hit regions + widget `click`/`hover` | Mouse → same intents/outcomes where meaningful |
| Help/hints | `Keymap::hint_spans` | Context-filtered generated footer + palette |
| Widgets | Mix of `handle_intent` + raw keys | **Intent-first**; `handle_key` = documented default bridge only |

---

## 1. Target architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│ Terminal / backend (crossterm feature)                          │
│  KeyEvent | MouseEvent | Paste | Resize | Enhanced keyboard     │
└────────────────────────────┬────────────────────────────────────┘
                             │ normalize
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│ InputNormalizer                                                 │
│  • KeyEvent → KeyChord (modifier policy)                        │
│  • raw CSI / kitty → KeyChord (enhanced protocol)               │
│  • conventional fallbacks when protocol absent                  │
│  • MouseEvent → PointerGesture { kind, position, modifiers }    │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│ InteractionRuntime                                              │
│  ┌──────────────┐  ┌─────────────────┐  ┌────────────────────┐  │
│  │ FocusGraph   │  │ OverlayStack    │  │ KeymapStack        │  │
│  │ zones,roving │  │ modal layers    │  │ context merge      │  │
│  │ traps,history│  │ esc peel        │  │ conflict detect    │  │
│  └──────┬───────┘  └────────┬────────┘  └─────────┬──────────┘  │
│         └───────────────────┼──────────────────────┘            │
│                             ▼                                   │
│                   IntentRouter                                  │
│         chord + context + focus leaf → UiIntent | AppCommand    │
└────────────────────────────┬────────────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        FocusGraph     WidgetState    AppCommandHandler
        (Tab/spatial)  handle_intent  (palette, help, quit…)
                             │
                             ▼
                      Typed Outcomes
              (no I/O inside components)
```

### Layer responsibilities

| Layer | Owns | Does not own |
|-------|------|--------------|
| **InputNormalizer** | Chord/protocol/paste bytes | Domain policy |
| **KeymapStack** | Chord→intent/command bindings, hints, conflicts | Widget state |
| **FocusGraph** | Who may receive keys/pointer, traps, restore | Selection inside List |
| **OverlayStack** | Modal geometry layers, esc/outside peel | Paint of dialog body |
| **IntentRouter** | Ordered resolve: jump mode → overlay → focused leaf map → app global | Effects |
| **Widget** | Selection, scroll, edit buffer, typed outcomes | Hardcoded product chords |
| **App** | Domain, effects, which command means quit | Re-implementing focus |

### Design principles

1. **Intents are the component contract.** Widgets implement `handle_intent`, not product chords.  
2. **Keymaps are data.** Vim/Emacs/app packs are tables, not `if key == 'j'` in widgets.  
3. **One focus authority.** FocusGraph lives under InteractionScene; delete public FocusRing dual.  
4. **Mouse is parallel input**, not a second behavior tree: click maps to Activate/Select/Toggle where hit geometry says so.  
5. **Help is generated from the same tables** that dispatch (no divergent docs).  
6. **Forward-only:** better unified APIs replace dual paths; migrate with numbered files.

---

## 2. Rust types and traits

### 2.1 Expanded semantic intents

```rust
/// Axis-sensitive navigation (terminal collections + spatial focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NavAxis {
    Vertical,
    Horizontal,
    /// Both axes (2D grids).
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NavigationMove {
    Previous, // up or left depending on axis context
    Next,
    First,
    Last,
    /// Explicit 2D / spatial (FocusGraph).
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PageMove {
    Backward,
    Forward,
}

/// Semantic UI intention — what the user wants, not which key was pressed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UiIntent {
    // ── Navigation ───────────────────────────────────────────
    Move(NavigationMove),
    Page(PageMove),
    /// FocusGraph: Tab / BackTab linear traversal (not list selection).
    FocusNext,
    FocusPrevious,
    /// Jump-to-region mode (letter labels).
    JumpStart,
    JumpLabel(char),

    // ── Activation ───────────────────────────────────────────
    Activate,
    Toggle,
    Open,
    Close,
    Cancel,
    Submit,

    // ── Editing (TextInput / TextArea / composer) ─────────────
    Edit,           // enter edit mode if needed
    Delete,         // delete selection / forward delete
    Backspace,
    /// Insert grapheme cluster (printable input after keymap miss).
    InsertText(String),
    CursorLeft,
    CursorRight,
    CursorLineStart,
    CursorLineEnd,
    CursorDocStart,
    CursorDocEnd,
    SelectAll,
    ClipboardCopy,
    ClipboardPaste,
    ClipboardCut,
    Undo,
    Redo,
    Newline,        // explicit newline (vs Submit)

    // ── Hierarchy ────────────────────────────────────────────
    Expand,
    Collapse,

    // ── Discovery / chrome ───────────────────────────────────
    Search,              // open find / focus filter
    ShowHelp,
    OpenCommandPalette,
    PromoteFullscreen,   // overlay narrow → full
    /// Application-level command id (palette / global map).
    AppCommand(AppCommandId),
}

/// Stable application command identity (not free-form strings at dispatch).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppCommandId(pub Cow<'static, str>);
```

**Context interpretation:** `Move(Previous)` in a vertical list = up; in a horizontal ActionBar = left; FocusGraph spatial maps `Up/Down/Left/Right` to nearest neighbor.

### 2.2 Keymap stack

```rust
/// Where a binding applies.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeymapContext {
    Global,
    /// Named zone: "sidebar", "editor", "dialog.footer"
    Zone(Cow<'static, str>),
    /// Widget kind contract: "list", "table", "text_input", "dialog"
    Surface(Cow<'static, str>),
    /// Overlay kind: dialog, palette, jump
    Overlay(OverlayKind),
    /// Modal trap: only this context + Global Escape policy
    Modal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeymapProfile {
    Default,
    Vim,
    Emacs,
    Custom,
}

/// One layer in the stack (later layers override earlier on conflict win rules).
pub struct KeymapLayer {
    pub id: Cow<'static, str>,
    pub context: KeymapContext,
    pub profile: KeymapProfile,
    pub map: Keymap<UiIntent>,
    pub enabled: bool,
    /// When true, chord match stops here (modal).
    pub captures: bool,
}

pub struct KeymapStack {
    layers: Vec<KeymapLayer>, // bottom → top
}

impl KeymapStack {
    pub fn push(&mut self, layer: KeymapLayer);
    pub fn pop(&mut self, id: &str) -> bool;
    pub fn set_enabled(&mut self, id: &str, enabled: bool);

    /// Resolve chord for the active context set (zone + surface + overlay).
    pub fn resolve(
        &self,
        chord: KeyChord,
        active: &ActiveKeyContexts,
    ) -> Option<ResolvedBinding>;

    pub fn conflicts(&self) -> Vec<KeyConflict>;
    pub fn hint_spans(&self, active: &ActiveKeyContexts) -> Vec<HintSpan<'static>>;
    pub fn palette_commands(&self, active: &ActiveKeyContexts) -> Vec<PaletteCommand>;
}

pub struct ActiveKeyContexts {
    pub zone: Option<Cow<'static, str>>,
    pub surface: Option<Cow<'static, str>>,
    pub overlay: Option<OverlayKind>,
    pub modal: bool,
    pub editing: bool, // text field owns insert
}

pub struct ResolvedBinding {
    pub intent: UiIntent,
    pub layer_id: Cow<'static, str>,
    pub chord: KeyChord,
    pub visibility: Visibility,
}
```

**Presets (data, not widget code):**

```rust
pub mod intent_maps {
    pub fn collection_default() -> Keymap<UiIntent>; // arrows, jk, home/end, page, enter, space, esc
    pub fn collection_vim() -> Keymap<UiIntent>;     // hjkl, gg/G as multi-chord later, ctrl-d/u
    pub fn text_default() -> Keymap<UiIntent>;
    pub fn text_emacs() -> Keymap<UiIntent>;          // C-a C-e C-k C-y …
    pub fn text_vim_insert() -> Keymap<UiIntent>;
    pub fn dialog_default() -> Keymap<UiIntent>;      // tab, enter, esc
    pub fn global_default() -> Keymap<UiIntent>;      // C-k palette, ?, help
}
```

**Multi-chord (vim `gg`):** phase-2 `Keymap` sequence state on `KeymapStack` (`pending: Option<SequenceState>`). Phase-1: single chords only; document sequences as follow-up without blocking architecture.

### 2.3 FocusGraph

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusNavMode {
    /// Tab / BackTab only.
    Linear,
    /// Arrow keys move to nearest neighbor by hit rect.
    Spatial,
    /// Linear outside; spatial or roving inside collections.
    Hybrid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusNode<Id> {
    pub id: Id,
    pub parent: Option<Id>,
    pub zone: Option<Cow<'static, str>>,
    pub area: Option<Rect>,
    pub enabled: bool,
    pub focusable: bool,
    /// Collection owns internal cursor; scene focuses the collection once.
    pub roving: bool,
    pub tab_index: i32, // lower first; stable id tie-break
}

pub struct FocusGraph<Id> {
    nodes: Vec<FocusNode<Id>>,
    focused: Option<Id>,
    /// Modal trap: only this subtree participates.
    trap_root: Option<Id>,
    /// Opener stack for traps / overlays.
    restore_stack: Vec<Id>,
    /// Ring history for “focus last”.
    history: VecDeque<Id>,
    mode: FocusNavMode,
}

impl<Id: Clone + Eq> FocusGraph<Id> {
    pub fn begin_frame(&mut self);
    pub fn register(&mut self, node: FocusNode<Id>);
    pub fn attach_area(&mut self, id: &Id, area: Rect);
    pub fn reconcile(&mut self) -> FocusOutcome<Id>;

    pub fn focus_next(&mut self) -> FocusOutcome<Id>;
    pub fn focus_previous(&mut self) -> FocusOutcome<Id>;
    pub fn focus_spatial(&mut self, dir: NavigationMove) -> FocusOutcome<Id>;
    pub fn request_focus(&mut self, id: Id) -> FocusOutcome<Id>;
    pub fn focus_at(&mut self, pos: Position) -> FocusOutcome<Id>;

    pub fn push_trap(&mut self, root: Id, opener: Option<Id>);
    pub fn pop_trap(&mut self) -> FocusOutcome<Id>;

    pub fn focused(&self) -> Option<&Id>;
    pub fn is_focused(&self, id: &Id) -> bool;
    /// For PanelEmphasis / border_focused.
    pub fn owns_keyboard(&self, id: &Id) -> bool;

    /// Jump labels for registered geometry.
    pub fn jump_labels(&self) -> Vec<(Id, char)>;
    pub fn debug_snapshot(&self) -> FocusDebugSnapshot<Id>;
}
```

**Roving focus:** Scene focuses `list_id` once. ListState keeps selected row. `FocusNext` leaves the list; `Move` intents go to ListState while list owns keyboard.

**Disabled reconciliation:** On `reconcile`, if focused id missing or disabled, move to nearest enabled sibling (same parent) then next in tab order — same spirit as FocusRing today.

### 2.4 Intent routing

```rust
pub struct IntentRouter {
    pub maps: KeymapStack,
    // multi-chord pending, jump mode buffer, etc.
}

pub enum RouteResult {
    Intent(UiIntent),
    /// Printable text for focused editor when no binding matched.
    TextInput(String),
    Unbound,
}

impl IntentRouter {
    pub fn route_key(
        &mut self,
        key: KeyEvent,
        focus: &FocusGraph<impl Eq>,
        active: &ActiveKeyContexts,
    ) -> RouteResult;

    pub fn route_pointer(
        &self,
        gesture: PointerGesture,
        hit: Option<HitTarget>,
    ) -> Option<UiIntent>;
}

pub struct PointerGesture {
    pub kind: PointerKind, // Move, PrimaryDown, SecondaryDown, Wheel { delta }
    pub position: Position,
    pub modifiers: KeyModifiers,
}
```

**Pointer → intent defaults:**

| Gesture | Intent |
|---------|--------|
| Primary click on row | Activate or Select (widget policy) |
| Click on check/disclosure | Toggle / Expand|Collapse |
| Wheel | Page Forward/Backward or Move Next/Previous (surface map) |
| Click outside modal | Cancel / Close per OverlayStack policy |

### 2.5 Widget integration trait

```rust
/// Contract every interactive TermRock widget implements for intents.
pub trait IntentHandler {
    type State;
    type Outcome;

    /// Surface name for KeymapContext::Surface ("list", "table", …).
    fn surface_id() -> &'static str;

    /// Intents this surface understands (for help + palette filtering).
    fn supported_intents() -> &'static [UiIntentKind];

    fn handle_intent(state: &mut Self::State, intent: UiIntent) -> Self::Outcome;
}

/// Discriminant-only for help tables (no payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiIntentKind { /* Move, Page, Activate, … */ }
```

Widgets may keep inherent methods; trait is for catalog/lint.

### 2.6 InteractionRuntime (app glue)

```rust
pub struct InteractionRuntime<Id, LayerId> {
    pub focus: FocusGraph<Id>,
    pub overlays: OverlayStack<Id>, // or unified with scene layers
    pub maps: KeymapStack,
    pub router: IntentRouter,
    pub jump: Option<JumpMode>,
}

impl<Id, LayerId> InteractionRuntime<Id, LayerId> {
    /// Single entry after backend event conversion.
    pub fn handle_event(&mut self, event: InputEvent) -> RuntimeOutcome<Id> { … }
}
```

HEAD may keep `InteractionScene` as the struct name and **grow** FocusGraph inside it rather than rename immediately — public dual FocusRing still dies.

---

## 3. Event flow

### 3.1 Keyboard (happy path)

```text
1. Backend KeyEvent (or raw bytes → chord)
2. Drop KeyEventKind::Release (unless protocol needs press/release pairs later)
3. If JumpMode active → JumpLabel / cancel
4. OverlayStack top esc policy: Esc may peel without widget
5. Build ActiveKeyContexts from focus leaf zone + surface + overlay
6. KeymapStack.resolve(chord, contexts) top-down:
     - modal layer captures?
     - surface map (list/table/text)
     - zone map
     - global map
7. If Unbound && focused is text editor → InsertText / Backspace raw policy
8. If intent is FocusNext/Previous/spatial → FocusGraph
9. Else if intent is AppCommand → app handler
10. Else → focused widget handle_intent
11. Collect Outcome → app update → next frame register focus/hits
```

### 3.2 Mouse

```text
1. MouseEvent → PointerGesture
2. hit_test(FocusGraph areas ∪ OverlayStack ∪ widget hit regions)
3. focus_at(position) if primary down on focusable
4. route_pointer → UiIntent
5. widget handle_intent or OverlayStack outside-click
```

### 3.3 Paste

```text
Paste payload → focused editor InsertText / multi-line policy
Never interpreted as chords unless app binds a paste command.
```

### 3.4 Enhanced keyboard protocols

| Source | Handling |
|--------|----------|
| Kitty keyboard / fixterms | Map to KeyCode + mods in InputNormalizer |
| CSI u | Same |
| Legacy xterm | Conventional arrows/F-keys via existing conversion |
| Unknown | `KeyCode::Unknown` → Unbound (never panic) |

---

## 4. Component integration examples

### App loop (sketch)

```rust
fn on_key(app: &mut App, key: KeyEvent) {
    let ctx = app.runtime.active_contexts(); // zone + surface from focus
    match app.runtime.router.route_key(key, &app.runtime.focus, &ctx) {
        RouteResult::Intent(UiIntent::FocusNext) => {
            app.runtime.focus.focus_next();
        }
        RouteResult::Intent(UiIntent::OpenCommandPalette) => {
            app.open_palette();
        }
        RouteResult::Intent(intent) => {
            if app.runtime.focus.is_focused(&app.list_id) {
                let _ = app.list_state.handle_intent(&app.rows, intent);
            } else if app.runtime.focus.is_focused(&app.input_id) {
                let _ = app.input_state.handle_intent(intent);
            }
        }
        RouteResult::TextInput(s) => {
            app.input_state.insert_str(&s);
        }
        RouteResult::Unbound => {}
    }
}
```

### Registering focus + hits (each frame)

```rust
app.runtime.focus.begin_frame();
app.runtime.focus.register(FocusNode {
    id: LIST,
    parent: Some(MAIN),
    zone: Some("main".into()),
    area: Some(list_rect),
    enabled: true,
    focusable: true,
    roving: true,
    tab_index: 10,
});
// after List render:
for region in list_state.regions() {
    // optional: debug / jump labels only; roving list keeps internal selection
}
app.runtime.focus.reconcile();
```

### Hint footer

```rust
let spans = app.runtime.maps.hint_spans(&ctx);
// HintBar::new(&spans, &theme)
```

### Command palette

```rust
// Palette rows = maps.palette_commands(&ctx) ∪ app commands
// Activation → UiIntent::AppCommand(id) or direct app dispatch
```

---

## 5. Migration examples

### 5.1 List

```rust
// BEFORE — keys inside widget path only
state.handle_key(&rows, key);

// AFTER — intent first
state.handle_intent(&rows, intent);

// Bridge (keep one milestone, documented as default map only)
state.handle_key(&rows, key); // calls collection_default() resolve internally

// App-owned vim
let map = intent_maps::collection_vim();
if let Some(i) = map.dispatch(KeyChord::from(key)) {
    state.handle_intent(&rows, i);
}
```

**List intent coverage:** Move*, Page*, Activate, Toggle, Cancel; ignore Expand/Collapse.  
**Click policy:** remains ListClickPolicy; pointer router emits Activate or synthetic select via intent if added (`Select` optional later).

### 5.2 Table

```rust
// BEFORE
table_state.handle_key(&rows, key);

// AFTER
table_state.handle_intent(intent, &model);
// Sort header click → app handles; or UiIntent::AppCommand("table.sort") from header keymap
```

**Coverage:** Move*, Page*, Activate, Cancel; no Toggle by default (matches `default_table_intent` today).  
**Horizontal:** optional Move(Left/Right) for cell navigation when VirtualGrid-like.

### 5.3 Dialog / ChoiceDialog

```rust
// BEFORE
choice_state.handle_key(&actions, key); // Esc/Enter/Tab raw

// AFTER
choice_state.handle_intent(&actions, intent);
// FocusGraph trap on dialog root; FocusNext cycles footer actions
// Esc → Cancel → OverlayStack dismiss OR Outcome::Cancelled
```

**Maps:** `dialog_default` — Enter Activate, Esc Cancel, Tab FocusNext, Shift-Tab FocusPrevious, arrows Move among actions.

### 5.4 TextInput

```rust
// BEFORE
input.handle_key(key); // many KeyCode matches inside widget

// AFTER
match intent {
    UiIntent::InsertText(s) => input.insert(&s),
    UiIntent::Backspace => input.backspace(),
    UiIntent::CursorLeft => input.move_left(),
    UiIntent::Submit => return TextInputOutcome::Submit,
    UiIntent::Cancel => return TextInputOutcome::Cancel,
    _ => {}
}
// Emacs pack: C-a → CursorLineStart, C-e → CursorLineEnd, C-k → delete to end, …
```

**Critical:** Printable characters are **not** list of bindings — unbound char → `InsertText` when `editing` context true.

### Migration sequence (repo)

1. Expand `UiIntent` + `intent_maps::*` as `Keymap<UiIntent>`.  
2. All interactive widgets: `handle_intent` complete; `handle_key` thin bridge.  
3. Fold FocusRing into FocusGraph inside InteractionScene; delete public FocusRing.  
4. KeymapStack + ActiveKeyContexts + hint/palette generation.  
5. Pointer → intent router.  
6. Contract matrix: `intents: covered` required.  
7. Migrations `0059+` for public breaks.

---

## 6. Focus and keymap debugging tools

### 6.1 Focus debug overlay (Studio / DesignInspector)

```text
FocusGraph debug
  trap: dialog.confirm
  focused: dialog.ok   zone=footer  roving=false
  history: list → composer → dialog.ok
  tab order:
    1 sidebar.tree
    2 main.list *
    3 main.composer
  spatial neighbors of main.list: up=tabs down=composer left=tree
```

API: `FocusGraph::debug_snapshot()` → inspector panels (existing DesignInspector recipes).

### 6.2 Keymap debugger

```text
Chord: Ctrl+K
  resolved: OpenCommandPalette  layer=global
  shadows: (none)
Conflicts:
  Space → Toggle (list) vs InsertText (editor)  [OK: context split]
  Esc → Cancel (list) vs Cancel (dialog)      [OK: modal capture]
```

API: `KeymapStack::explain(chord, &ActiveKeyContexts) -> ExplainResult`.

### 6.3 Lookbook stories

| ID | Purpose |
|----|---------|
| `interaction/intent-list-vim` | Same list, vim map |
| `interaction/focus-trap-dialog` | Tab stays in dialog; restore opener |
| `interaction/spatial-zones` | Arrow moves between panes |
| `interaction/keymap-conflicts` | Inspector shows conflict list |
| `interaction/jump-labels` | Jump mode letters |

### 6.4 Runtime flag

`TERMROCK_DEBUG_FOCUS=1` / app toggle: draw focus id + zone in status bar each frame.

---

## 7. Interaction tests

### Unit

| Test | Asserts |
|------|---------|
| `intent_list_default_arrows` | Up/Down → Move |
| `intent_list_vim_hjkl` | h/j/k/l maps |
| `intent_emacs_ctrl_a` | CursorLineStart |
| `keymap_stack_modal_captures_esc` | lower layer does not see Esc |
| `keymap_conflict_same_context` | `conflicts()` non-empty |
| `keymap_context_split_no_conflict` | Space list vs editor OK |
| `focus_reconcile_skips_disabled` | focus moves to next enabled |
| `focus_trap_restore_opener` | pop_trap returns opener |
| `focus_roving_tab_leaves_collection` | Tab from list → next zone |
| `focus_spatial_nearest` | Down from sidebar → main |
| `router_unbound_char_inserts_in_editor` | editing context |
| `router_unbound_char_ignored_in_list` | not editing |
| `pointer_click_maps_activate` | PrimaryDown + hit |
| `hint_spans_match_dispatchable` | Shown ⊆ resolve |
| `enhanced_unknown_key_ignored` | no panic |

### Integration

| Test | Asserts |
|------|---------|
| Dialog open → trap → Esc Cancelled + focus restore | |
| Command palette chord → OpenCommandPalette → layer | |
| List multi Toggle via intent only (no raw space in widget test) | |
| TextInput Submit vs Newline policy | |

### Contract matrix

Add axes: `intents`, `keymap`, `focusGraph` = covered | exempt.

---

## 8. Rules every TermRock component must follow

1. **No product-specific chords in widget paint/update code.** Only `UiIntent` (and documented default bridge).  
2. **`handle_intent` is the primary API**; raw `handle_key` may exist only as default-map bridge.  
3. **Declare `surface_id` + supported intents** for help, palette, and lint.  
4. **Typed outcomes only** — no process spawn, no I/O, no global keymap mutation inside widgets.  
5. **Stable ids** for focus, selection, and hits when identity survives reorder.  
6. **Register focus** with zone + area each frame when participating in app focus; composites are one scene target when roving.  
7. **Roving collections** keep internal cursor; do not register every row as a scene focus node unless Jump needs them.  
8. **Modal dialogs** push FocusGraph trap + OverlayStack layer with explicit esc policy.  
9. **Mouse paths** must have keyboard intent equivalents for the same outcomes.  
10. **Hints and help** come from KeymapStack for the active context — no hand-maintained parallel chord lists.  
11. **Disabled** elements: not focusable, not hittable, skipped by reconcile.  
12. **Colorless / mono:** intent routing unchanged; chrome uses non-color cues (existing design system).  
13. **Cross-surface consistency:** a new intent or map pattern in one widget is rolled to peers (AGENTS.md).  
14. **Breaking intent/focus API** ships migration file + MIGRATING index.  
15. **Tests inject intents** for behavior; key tests only cover map tables.

---

## 9. Implementation phasing

| Phase | Deliverable |
|-------|-------------|
| **A** | Expand `UiIntent`; `intent_maps` as `Keymap<UiIntent>`; widget `handle_intent` complete for List/Table/Tree/Dialog/Text* |
| **B** | FocusGraph inside InteractionScene; kill public FocusRing; traps + restore |
| **C** | KeymapStack + context merge + conflict explain + generated hints |
| **D** | Pointer → intent; Jump mode; palette integration |
| **E** | Vim/Emacs packs as data; multi-chord sequences; Studio focus/keymap panels |
| **F** | Contract lints; remove dual raw-key paths from contracts |

---

## 10. Success criteria

1. Rebind “Activate” globally without editing List/Table source.  
2. Vim collection pack is a keymap file/table, not forked widgets.  
3. Focus trap + opener restore works without apps reimplementing FocusRing.  
4. HintBar always matches what dispatch will do for the focused context.  
5. Unit tests for list selection never need `KeyCode::Down` — only `UiIntent::Move(Next)`.  
6. One public focus authority; zero public FocusRing.

---

## 11. Relationship to existing modules

| Keep / evolve | Role |
|---------------|------|
| `Keymap`, `KeyChord`, `Visibility`, `conflicts`, `remap` | Kernel of KeymapStack |
| `UiIntent` (expand) | Component contract |
| `InteractionScene` | Host FocusGraph + registration |
| `OverlayStack` | Modal geometry + peel |
| `dispatch_keymap_action` | Specialize to intent or AppCommand |
| `default_*_intent` | Move into `intent_maps` data |
| `FocusRing` | **Delete public** after FocusGraph parity |
| `JumpOverlay` | Jump mode on runtime |
| `HintBar` | Consumer of `hint_spans` |
| `CommandPalette` | Consumer of palette command projection |
| `DesignInspector` | Focus + keymap debug panels |
