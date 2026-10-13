# TermRock shadcn-quality roadmap

**Status:** design recommendations (executable via plans 039+)  
**Inspected:** branch `feat/experience-layer-shadcn-tui` @ `3ec3ea8` family  
**Sources:** `crates/termrock/**`, lookbook (74 stories), `docs/design/*`, migrations 0029–0031, plans 039–040 drafts

---

## 0. Reality check (not README assumptions)

### What the tree actually has

| Asset | Evidence | Quality |
|-------|----------|---------|
| Interaction kernel | `FocusRing`, `ModalStack`, `OverlayHost`, `EscCascade`, `OverlayController`, `SemanticScene`, `UiIntent` | **Strong foundation, incomplete integration** |
| Design tokens | `DesignTokens`, `SpacingScale`, `GlyphSet`, `SelectionChrome`, `list_row_recipe` | **Defined but barely consumed** — List paint still hardcodes `Role::Selection` + `"▸ "` |
| Capability reduction | `ColorCapability`, `Theme::quantized`, `Appearance`, `Motion` | **API exists**; lookbook/docs still default truecolor-only stories |
| Data widgets | List, Table, Tree, VirtualGrid, LogPane, Form, Picker | **Production-grade bones**; VirtualGrid has contract bugs (see plan 039) |
| Agent pack | StreamView, ToolCard, ApprovalCard, PromptBox, Timeline… | **Present as paint shells**; ApprovalCard safety is wrong-by-default (plan 039) |
| Patterns | agent_shell, ops_dashboard, resource_browser | **Geometry only** — not installable source blocks |
| Lookbook | 74 stories, contracts, SVG gate | **Serious studio seed**, not yet inspector/registry |
| Distribution | git crate pin only | **Not shadcn** — no CLI, manifest, or copy-own path |

### The single most important diagnosis

TermRock has **two parallel truths**:

1. **New kernel types** (`UiIntent`, `DesignTokens`, `SemanticScene`, `OverlayController`).
2. **Old widget behavior** (raw `KeyCode` in Tree/Table/Picker, Theme-only paint, ad-hoc Esc, Approval/VirtualGrid hazards).

Until those converge, TermRock remains a **strong Ratatui crate with aspirational APIs**, not a design system users experience as cohesive.

---

## 1. Recommendation R1 — Fail-safe interaction contracts first

### 1. Problem

`ApprovalCardState::new()` selects index `0` (`AllowOnce`). Enter therefore **approves by default**, including when risk is `High`. Narrow layouts can leave state pointing at a decision that is not painted (`agent.rs` option paint loop clips by width).

`VirtualGrid` exposes `enabled` but never consults it; paints phantom rows when `total_rows` is 0; range highlight compares a cell to itself; keyboard outcomes drop stable row IDs.

These are not polish issues. They poison trust for every agent and data app.

### 2. UX improvement

High-risk prompts default to the **safest visible** decision (Deny/Defer). Invisible options are not selectable. Grids never invent rows; disabled cells never activate; selection outcomes always carry stable IDs when resident.

### 3. Modules

- `crates/termrock/src/widgets/agent.rs` (`ApprovalCard*`)
- `crates/termrock/src/widgets/virtual_grid.rs`
- contracts, lookbook stories, migration `0032`
- **Plan 039** already specifies this work

### 4. API sketch

```rust
pub struct ApprovalCardState {
    selected: ApprovalDecision, // not raw index
}

impl ApprovalCardState {
    pub const fn new_for_risk(risk: ApprovalRisk) -> Self {
        Self {
            selected: match risk {
                ApprovalRisk::High => ApprovalDecision::Deny,
                ApprovalRisk::Medium => ApprovalDecision::Defer,
                ApprovalRisk::Low => ApprovalDecision::AllowOnce,
            },
        }
    }
}

// VirtualGridOutcome always prefers:
// CursorMoved { row_id: Option<Id>, absolute_row, col_id }
// Activate { row_id: Id, ... } only for resident + enabled cells
```

### 5. Foundation vs visual

**Foundational safety/correctness** — do before any chrome work.

### 6. Order / tests / stories / acceptance

1. Implement Approval defaults + visible-option clamp + navigation tests.
2. VirtualGrid enabled policy, no phantom paint, range vs cursor endpoint, stable IDs on outcomes.
3. Stories: `approval-card/high-risk-default`, `virtual-grid/empty`, `virtual-grid/disabled`.
4. Contracts: ApprovalCard `narrowTerminal: covered` with narrow story.
5. **Accept:** `cargo test -p termrock approval virtual_grid`; high-risk Enter does not emit `AllowOnce` without explicit selection move; empty grid has zero hit regions.

---

## 2. Recommendation R2 — One interaction scene owns focus, hits, overlays, actions

### 1. Problem

Today consumers must keep four stacks coherent by hand:

- `FocusRing` scopes
- `SemanticScene` rects (no layers/actions)
- `OverlayHost` / `EscCascade` / `OverlayController` (separately mutable)
- widget-local `handle_key` tables

`OverlayHost::dismiss_top_esc` can dismiss a **lower** dismissible layer while a non-dismissible layer sits on top. Esc and focus can disagree. Hints and command palettes cannot discover “what can I do now?” from one source.

### 2. UX improvement

Every frame has one truth for: what is under the pointer, who owns keys, what Esc peels, what hints to show, what the palette lists. Apps stop inventing glue; overlays stop fighting each other.

### 3. Modules

- Entire `crates/termrock/src/interaction/`
- Bridge from `keymap.rs`
- Vertical integrations: JumpOverlay, CommandPalette, CompletionMenu, ApprovalCard
- **Plan 040** targets this; migration `0033`

### 4. API sketch

```rust
pub struct InteractionScene<Id, ScopeId, Action> { /* per-frame */ }

pub struct ElementReg<'a, Id, ScopeId, Action> {
    pub id: Id,
    pub scope: ScopeId,
    pub area: Rect,
    pub layer: u8,                 // paint/input order
    pub focusable: bool,
    pub enabled: bool,
    pub input_owner: bool,         // topmost input owner traps keys
    pub esc_policy: EscPolicy,     // Dismiss | Bubble | Trap
    pub actions: &'a [BoundAction<Action>],
}

pub enum SceneEvent<Id, Action> {
    Focus(FocusOutcome<Id>),
    Action { target: Id, action: Action },
    OverlayDismissed { id: Id },
    UnhandledEsc,
}

impl InteractionScene<Id, ScopeId, Action> {
    pub fn begin_frame(&mut self);
    pub fn register(&mut self, ElementReg<'_, Id, ScopeId, Action>);
    pub fn reconcile_focus(&mut self) -> FocusOutcome<Id>;
    pub fn handle_key(&mut self, key: KeyEvent, map: &Keymap<Action>) -> SceneEvent<Id, Action>;
    pub fn handle_pointer(&mut self, e: MouseEvent) -> SceneEvent<Id, Action>;
    pub fn discoverable_actions(&self) -> Vec<&BoundAction<Action>>;
}
```

### 5. Foundation vs visual

**Foundational architecture.** Replaces parallel stacks; may break public overlay types by folding them into the scene.

### 6. Order / tests / stories / acceptance

1. Land after R1.
2. Scene registration + hit order (topmost wins) + Esc only peels top layer if policy allows.
3. Focus restore on dismiss (existing FocusRing opener semantics).
4. Dogfood: CommandPalette + ApprovalCard as scene participants.
5. Story: multi-layer shell (dialog over list) with Esc peel + focus return.
6. **Accept:** non-dismissible top layer never peels lower menu; one Esc → one layer; hints derived from `discoverable_actions()` match key routing.

---

## 3. Recommendation R3 — Design tokens drive paint (quiet canvas, bright intent)

### 1. Problem

Default phosphor theme maps Canvas/Surface/Elevated/Backdrop to empty styles; Selection, Focus, Accent, BorderFocused often share the same green (`style/mod.rs` phosphor array). List render ignores `DesignTokens` entirely and always full-fills selection.

Result: flat hierarchy, selection screams, tokens are dead code, density/glyphs don’t change UX.

### 2. UX improvement

Surfaces stack quietly; phosphor is reserved for **intent** (cursor, primary action, live/running). Compact density tightens padding; gutter selection reads calmer in long lists; ASCII glyph set keeps structure without Unicode.

### 3. Modules

- `style/mod.rs` (phosphor role values)
- `style/tokens.rs` (expand recipes: `panel_recipe`, `menu_item_recipe`)
- `widgets/list.rs`, `tree.rs`, `table.rs`, `panel.rs` paint paths
- lookbook dual-theme + density stories

### 4. API sketch

```rust
impl List<'a, Id> {
    pub const fn tokens(mut self, tokens: &'a DesignTokens) -> Self { ... }
}

// Panel anatomy (breaking evolve of Panel)
pub struct PanelSlots<'a> {
    pub title: Option<&'a str>,
    pub subtitle: Option<&'a str>,
    pub leading: Option<&'a str>,   // status glyph
    pub trailing: Option<&'a str>,  // badge / shortcut
    pub footer_hint: Option<&'a str>,
}
```

Phosphor defaults (intent rarity):

| Role | Proposed |
|------|----------|
| Canvas | near-black / terminal default |
| Surface | subtle lift (RGB dark gray) |
| Elevated | stronger lift for dialogs |
| Selection | dim tint or gutter, not full PHOSPHOR fill |
| Accent / BorderFocused | PHOSPHOR only |
| Success/Warning/Danger | keep distinct non-green hues |

### 5. Foundation vs visual

**Both:** token plumbing is foundation; phosphor role values are visual system.

### 6. Order / tests / stories / acceptance

1. After R1 (can parallel R2 carefully).
2. List uses `list_row_recipe`; snapshot buffer cells for Fill vs Gutter.
3. Phosphor canary: `Role::Selection` ≠ `Role::Accent` in default theme.
4. Stories: `list/selection-gutter`, `list/density-compact`, `panel/anatomy`.
5. **Accept:** changing `SelectionChrome` visibly changes List paint without app code; elevated dialog bg differs from canvas in phosphor.

---

## 4. Recommendation R4 — Universal intent + keymap bridge

### 1. Problem

Only List routes through `UiIntent`. Tree/Table/VirtualGrid/Picker/CompletionMenu still match `KeyCode` directly. Global Vim vs Simple modes require N rewrites. Keymap actions and widget keys can diverge from HintBar ads.

### 2. UX improvement

One remapping surface changes every collection. Palette and footer always match real bindings. Agent shells can switch Simple/Vim without forking widgets.

### 3. Modules

- `interaction/intent.rs` (expand families: TreeExpand/Collapse, Edit*, Scroll*)
- `keymap.rs` bridge `Keymap<Action> → UiIntent` or generic `Action`
- `tree.rs`, `table.rs`, `virtual_grid.rs`, `picker.rs`, `completion_menu.rs`

### 4. API sketch

```rust
pub trait IntentHandler {
    type Outcome;
    fn handle_intent(&mut self, intent: UiIntent) -> Self::Outcome;
}

// Keymap remains Action-typed for apps:
pub fn intent_from_list_action(action: ListAction) -> UiIntent { ... }

// Picker split intents: query edit vs results navigation
pub enum PickerIntent {
    Query(EditIntent),
    Results(UiIntent),
}
```

### 5. Foundation vs visual

**Foundational interaction.**

### 6. Order / tests / stories / acceptance

1. After R2 scene (or tightly with it).
2. Tree expand/collapse as intents; Table sort-request remains outcome, not effect.
3. Picker: printable → query; vertical → results (already partially true — formalize via intents).
4. **Accept:** single test keymap remaps Down→Up for List and Tree; CompletionMenu never activate-on-move.

---

## 5. Recommendation R5 — Row/panel anatomy (composition, not strings)

### 1. Problem

`ListRow` is `{ label: Line, trailing: Option<Line> }`. Production UIs need leading icon, secondary text, badge, shortcut, status with **priority drop order** on narrow widths. Panel is title + border only.

### 2. UX improvement

Rows stay legible at 40 columns by dropping low-priority parts before truncating primary text. Panels carry status and actions without each app inventing chrome.

### 3. Modules

- `widgets/list.rs` (or new `widgets/row.rs`)
- `widgets/panel.rs`, dialogs, agent cards

### 4. API sketch

```rust
pub struct RowPart<'a> {
    pub text: Line<'a>,
    pub priority: u8, // 0 = drop last
    pub role: RowPartRole, // Leading, Primary, Secondary, Badge, Shortcut, Status
}

pub struct ComposedRow<'a, Id> {
    pub id: Id,
    pub parts: &'a [RowPart<'a>],
    pub enabled: bool,
}
```

### 5. Foundation vs visual

**Foundational composition API** with visual layout solver.

### 6. Order / tests / stories / acceptance

After R3 tokens. Narrow story must prove Secondary drops before Primary truncates.

---

## 6. Recommendation R6 — Agent pack as flagship, not paint demos

### 1. Problem

Agent widgets exist but are independent paint helpers. No shared stream virtualization policy, no permission ledger, no mode ribbon (plan/build), no activity rail integration with overlays/scene.

Grok Build / Amp / OpenCode win by treating **modes, permissions, tasks, and history** as first-class.

### 2. UX improvement

Building an agent TUI becomes composing TermRock blocks: stream + tool cards + approval + prompt + status, already scene-aware and token-aware.

### 3. Modules

- `widgets/agent.rs` split into `widgets/agent/*`
- `patterns/agent_shell.rs` (wire real widgets, not only rects)
- lookbook “AgentWorkbench” multi-story

### 4. API sketch

```rust
// patterns become stateful recipes:
pub struct AgentShell<'a> {
    pub stream: StreamView<'a, Id>,
    pub prompt: PromptBox<'a>,
    pub mode: AgentMode, // Plan | Build
    pub tokens: &'a DesignTokens,
}
```

### 5. Foundation vs visual

**Product pack** on top of R1–R4.

### 6. Order / tests / stories / acceptance

After R1–R4. Story: mock agent session with approval default-deny, Esc dismiss, token meter, stream fold.

---

## 7. Recommendation R7 — Source-owned distribution (true shadcn gap)

### 1. Problem

Users depend on the crate. Opinionated blocks cannot be forked without vendoring the monorepo. No `components.json`, no install, no upstream diff.

### 2. UX improvement

`termrock add agent/tool-card` copies inspectable source into the app; kernel stays a versioned crate; `termrock diff` shows upstream changes without clobbering local edits.

### 3. Modules

- New crate `termrock-cli` (later)
- `registry/` or `blocks/` source packages
- manifest schema

### 4. API sketch

```toml
# termrock.toml
kernel = { git = "...", rev = "..." }
[components]
tool-card = { path = "src/ui/tool_card.rs", version = "0.12.0" }
```

### 5. Foundation vs visual

**Distribution architecture** — after kernel APIs stabilize (R2–R4).

### 6. Order / tests / stories / acceptance

Spike CLI install of one block that compiles against published kernel API. Golden: second install refuses silent overwrite of dirty files.

---

## 8. Recommendation R8 — Lookbook → TermRock Studio

### 1. Problem

Lookbook validates contracts and SVGs but does not inspect hit regions, focus order, intent routing, token recipes, or capability ladders interactively.

### 2. UX improvement

Authors see focus ring, scene layers, active intents, and 16-color/NO_COLOR previews live — quality bar becomes visible.

### 3. Modules

- `termrock-lookbook` app, knobs, interactors
- optional doctor command for capability detection

### 4. Order / acceptance

After R2–R3. Studio mode toggles: Density, GlyphSet, ColorCapability, SelectionChrome; overlay inspector lists registered elements.

---

## Program sequence (do not reorder casually)

```
P0  R1  Safety: ApprovalCard + VirtualGrid contracts     (plan 039 / mig 0032)
P0  R2  InteractionScene unifies stacks                  (plan 040 / mig 0033)
P1  R3  Tokens drive List/Panel paint + phosphor rarity
P1  R4  Intent-ify Tree/Table/Picker/Completion
P2  R5  Composed rows + panel anatomy
P2  R6  Agent workbench flagship stories
P3  R7  Registry/CLI source ownership
P3  R8  Studio inspector + capability previews
```

---

## What not to do

- Add more widgets before R1–R2 (increases surface on broken contracts).
- Keep dual APIs (`handle_key` forever + unused `DesignTokens`).
- Copy web shadcn components literally (Cards with heavy chrome everywhere).
- Treat phosphor green as the universal “selected” paint.
- Build CLI registry before scene/token convergence (copies would freeze bad patterns).

---

## Immediate next execution targets

1. Execute **plan 039** on PR #6 (or main per policy).
2. Execute **plan 040** InteractionScene.
3. Open plan **041**: List/Panel consume DesignTokens + phosphor role redesign.
4. Open plan **042**: Tree/Table/Picker `handle_intent`.

---

## Success metric

A new consumer can build an agent-style TUI by:

1. depending on the kernel crate;
2. composing AgentShell + scene + tokens;
3. remapping keys once;
4. getting safe approvals, correct grids, quiet hierarchy, and coherent Esc;

without reimplementing focus, overlays, or selection chrome—and later can **own** block source via registry without losing those contracts.
