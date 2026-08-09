# TermRock complete product audit

**Goal:** become the shadcn/ui of TUI/CLI for Ratatui.  
**Inspected:** `feat/experience-layer-shadcn-tui` (re-verified HEAD family `fec49b1`+), 46 public widgets, 74 lookbook stories, migrations through `0031`, plans `039`–`044` (all still TODO — no P0 code fixes landed yet).  
**Method:** source, tests, contracts, SVG previews, mise gate, COMPONENTS ownership model—not README claims alone.  
**Live probes (still true):** ApprovalCard `selected: 0` → AllowOnce; List has `handle_intent` but no `DesignTokens` paint; `list_row_recipe` unused outside tokens.rs; `dismiss_top_esc` uses `rposition`; SemanticScene has no layers/actions; only 1 widget implements `handle_intent`.  
**Policy:** quality over compatibility; breaking redesigns preferred.

**Priority legend**

| Rank | Meaning |
|------|---------|
| **P0** | Foundational and blocking |
| **P1** | Critical to product quality |
| **P2** | Major improvement |
| **P3** | Valuable refinement |

---

## 1. What TermRock already does exceptionally well

| Strength | Evidence | Why it matters |
|----------|----------|----------------|
| **Ownership model** | Borrowed render data + stable IDs; domain/effects stay consumer-owned (`COMPONENTS.md`) | Correct shadcn-adjacent boundary: library owns chrome, app owns product |
| **Terminal lifecycle** | `crossterm::Session` raw mode, alt screen, mouse, paste, wrap, cursor, rollback, Drop idempotent | Rare, hard correctness moat |
| **Focus system** | `FocusRing`: per-frame register, scopes, modal open/pop with opener restore, Tab, pointer focus, panel emphasis from focus | Stronger than most TUI kits |
| **Hit geometry** | State-owned painted regions; hover ≠ keyboard focus steal | Pointer + keyboard coexistence |
| **Unicode / grapheme editing** | `text/*`, TextInput/TextArea, hot-path tests, unicode stories | Production-grade editing |
| **Narrow-terminal contracts** | Contract matrix + narrow stories for core widgets | Explicit quality axis |
| **Virtualization direction** | VirtualGrid, Tree/Table/LogPane hot-path budgets, stats_alloc | Performance as culture |
| **Keymap as single table** | Dispatch + hints + conflicts + runtime remap | Right abstraction for remapping |
| **Lookbook discipline** | Real public API stories, SVG gate, contracts JSON, public-api inventory, docs gen | Design-system studio seed |
| **Migration discipline** | Numbered `migrations/` + `MIGRATING.md` | Forward-only culture fits pre-stable ambition |
| **Experience-layer inventory** | Agent widgets, charts, patterns, quantize/appearance, UiIntent (List), DesignTokens types | Breadth for agent/ops apps |

**Bottom line:** TermRock’s kernel instincts (lifecycle, focus, IDs, borrowed data, catalog gates) are already **category-leadership material**. The gap is integration, safety contracts, visual system depth, and source distribution—not “lack of widgets.”

---

## 2. What prevents a coherent, premium design system feel

| Finding | Rank | Root cause in tree |
|---------|------|--------------------|
| **Parallel product truths** | P0 | New kernel types (`UiIntent`, `DesignTokens`, `SemanticScene`, `OverlayController`) vs widgets that ignore them |
| **Phosphor is one-dimensional** | P1 | Canvas/Surface/Elevated/Backdrop = empty styles; Selection/Focus/Accent share `#00ff41` — SVG `list-selection` is full-row green fill |
| **Tokens don’t paint** | P0 | `DesignTokens` / `list_row_recipe` unused outside `style/tokens.rs` + re-exports |
| **Thin anatomy** | P1 | `Panel` = title + border + 3-way emphasis; `ListRow` = label + optional trailing |
| **Patterns are geometry stubs** | P1 | `agent_shell` / ops / resource return rects only—no state, focus, or input ownership |
| **Agent pack is disconnected** | P1 | Stream one-line, ToolCard/Approval/Prompt not one workbench contract |
| **Inconsistent input model** | P0 | Only List has `handle_intent`; 15+ widgets still raw `KeyCode` |
| **No enforced design language** | P1 | Lookbook doesn’t fail on hierarchy/token misuse; only inventory/contracts |

---

## 3. What prevents shadcn/ui equivalence for Ratatui

shadcn wins on **owned, inspectable source** + registry/CLI + blocks + coherent tokens—not on “has a Button.”

| shadcn pillar | TermRock today | Gap rank |
|---------------|----------------|----------|
| Install/copy components you own | Git crate pin only | **P0** |
| Manifest + provenance + upstream diff | Absent | **P0** |
| Blocks (page recipes) | Flat rect patterns | **P1** |
| Token-driven composition | Theme roles + unused DesignTokens | **P0** |
| CLI (`add` / `diff`) | Absent | **P1** |
| Open registry | Absent | **P2** |
| Storybook/studio | Lookbook (strong seed) | **P1** (needs inspector) |

Hybrid is correct: **kernel stays a crate**; **opinionated chrome becomes source-owned blocks**. Today everything is the crate—so you get version coupling without ownership, the worst of both worlds for shadcn-class DX.

---

## 4. APIs that should be preserved

Keep and treat as **stable kernel contracts** (evolve carefully, don’t replace casually):

| API | Why preserve |
|-----|----------------|
| Borrowed data + stable IDs | Core composition model |
| `Session` lifecycle semantics | Correctness moat |
| `FocusRing` scope/modal/opener model | Real multi-widget focus |
| `HitRegion` / painted geometry | Mouse truth |
| `Keymap` chord→action + hint projection | Remapping single source |
| Neutral `input::Event` / `KeyEvent` | Backend independence |
| `FrameTick` immutable time | Testable animation/TTL |
| `Outcome` / typed widget outcomes | Effects stay consumer-owned |
| Catalog gates (public-api, contracts, SVG) | Design-system enforcement |
| Migration numbering + DCO + gate | Engineering culture |
| `Role` semantic naming (not values) | Vocabulary for themes |
| Hot-path test *idea* | Performance culture |

---

## 5. APIs to redesign or remove (breaking OK)

| Surface | Action | Rank | Reason |
|---------|--------|------|--------|
| `ApprovalCardState { selected: usize }` default `0` → AllowOnce | **Redesign** | P0 | Unsafe default |
| `OverlayHost::dismiss_top_esc` rposition any dismissible | **Redesign** | P0 | Peels under non-dismissible tops |
| Separate public `OverlayHost` + `EscCascade` + thin `SemanticScene` + `FocusRing` as app glue | **Consolidate** into InteractionScene | P0 | Parallel truths |
| Raw `KeyCode` in Tree/Table/Picker/… | **Replace** with intent/action routing | P0 | Blocks global remaps |
| List paint ignoring `DesignTokens` | **Replace** paint path | P0 | Dead design system |
| Phosphor role array (empty surfaces, green everywhere) | **Redesign values** | P1 | Flat luxury failure |
| `StreamView` one-row model | **Replace** with transcript engine | P1 | Agent flagship blocker |
| Flat `WorkSurface` only | **Replace/extend** workspace tree | P1 | Invalid narrow geometry risk |
| `Panel` title-only | **Expand** anatomy (breaking ok) | P1 | Too thin for design system |
| `ListRow` label+trailing only | **Expand** composed parts | P2 | Priority drop for narrow |
| Parallel public stacks without deprecation | **Remove** after scene lands | P1 | No compatibility facades |

---

## 6. Missing foundational primitives

| Primitive | Rank | Sketch |
|-----------|------|--------|
| **InteractionScene** | P0 | See §13 |
| **Intent/action bridge for all collections** | P0 | `IntentSurface` / scene actions |
| **Token-driven paint (`DesignSystem`)** | P0 | `DesignTokens` required by chrome widgets |
| **Esc policy on layers** (Trap/Dismiss/Bubble) | P0 | Top-only dismiss |
| **Transcript measure/viewport anchor** | P1 | `(block_id, row_in_block)` |
| **Workspace pane tree** | P1 | Nested split/tabs/stack + collapse priority |
| **Composed row parts + priority** | P1 | Radix-like slots |
| **Capability profile** (color, glyph, motion) threaded to render | P1 | Already partial; not enforced |
| **Safe decision control** | P0 | Decision enum selection, not index |
| **Focus graph spatial nav** | P2 | After scene |
| **Text measurement pipeline for wraps** | P1 | Transcript + markdown |

---

## 7. Missing high-level components

| Component | Rank | Notes |
|-----------|------|--------|
| **Button / IconButton** (or Action control with variants) | P1 | Today only ActionBar chips |
| **Menu / ContextMenu / Menubar** | P1 | CompletionMenu ≠ general menu |
| **Popover / Tooltip** | P2 | Overlay geometry |
| **Checkbox / Switch / Radio** (standalone) | P2 | Only embedded list checks |
| **Select / Combobox** | P1 | Picker is close; formalize |
| **Accordion / Collapsible** | P2 | Agent thinking/tool expand |
| **Breadcrumb** | P3 | Resource browser |
| **Stepper / Wizard** | P2 | Onboarding/forms |
| **DataTable v2** (column pin, multi-sort chrome) | P2 | Table is good base |
| **ScrollArea primitive** | P1 | Unify LogPane/Viewport/TextArea scroll chrome |
| **Alert / Callout** | P2 | Banner is thin |
| **Avatar / Identity chip** | P3 | Agent personas |
| **Keyboard shortcut display** | P2 | Keymap already has glyphs—promote widget |

---

## 8. Missing application blocks

| Block | Rank | Reference |
|-------|------|-----------|
| **AgentWorkbench** | P0 product | Grok Build / Amp / OpenCode |
| **Permission ledger + risk approval** | P0 | Safe agent UX |
| **Ops dashboard** (live, not rects) | P1 | btop density |
| **Resource browser** (tree+detail+preview) | P1 | Yazi / k9s |
| **Git workbench** patterns | P2 | lazygit |
| **HTTP client workbench** | P2 | Posting |
| **Markdown reader** | P2 | Glow |
| **Multiplexer chrome** (mode ribbon) | P3 | Zellij |

Patterns today (`layout_agent_shell` etc.) are **layout sketches**, not blocks.

---

## 9. Missing developer tooling

| Tool | Rank |
|------|------|
| **`termrock` CLI** (`init`, `add`, `diff`, `check`) | P0 for shadcn goal |
| **Component manifest** (`termrock.toml`) | P0 |
| **Block source packages** under `blocks/` or registry | P1 |
| **Studio inspector** (focus/scene/tokens/capability) | P1 |
| **Capability doctor** in showcase | P2 |
| **Visual regression CI** beyond SVG hash | P2 |
| **Public API changelog generator** | P3 |
| **Fuzz/mutation** (deferred in TODO—revisit post-kernel) | P3 |

---

## 10. Missing quality contracts

Existing axes (keyboard/mouse/focus/nonColor/unicode/narrow) are good. Missing:

| Contract | Rank |
|----------|------|
| **Safety defaults** (destructive/confirm) | P0 |
| **Esc layer policy** | P0 |
| **Intent-routable** (or exempt) | P0 |
| **Token-themed** (uses DesignTokens/recipes) | P0 |
| **Stable-ID outcomes** when data resident | P0 |
| **No phantom interactables** | P0 |
| **Streaming/anchor stability** (transcript) | P1 |
| **Density/capability variants** for chrome | P1 |
| **Performance budget** class (hot-path yes/no) | P2 |
| **A11y: non-color always sufficient** | P1 (partial) |

---

## 11. Visual-design weaknesses (from SVG previews)

Evidence from committed previews:

- **`list-selection.svg`:** Selected row is solid `#00ff41` with black text across the full width—selection **is** the brand, not a quiet cue. Unselected checked row also `#00ff41` text. Hierarchy collapses to black / gray / neon green / white (~5 fills).
- **`panel-focused` / `table-basic` / `toast-success`:** Same palette poverty; success toast still dominated by phosphor, not a calm surface stack.
- **`stream-view-basic`:** Adds cyan but still flat; no elevation cards for turns.
- **`approval-card-basic`:** Danger pink helps, but chrome doesn’t read as “blocking permission,” just another bordered box.
- **Padding ring** is uniform charcoal; components don’t demonstrate Surface vs Elevated.
- **No density comparison stories** (comfortable vs dashboard) in previews.
- **No monochrome / 16-color story** despite quantize API.

Principle to adopt: **Quiet canvas, bright intent.** Phosphor for cursor, primary action, live/running—not every selected cell.

---

## 12. Interaction weaknesses screenshots hide

| Issue | Rank | Location |
|-------|------|----------|
| Approval Enter defaults AllowOnce | P0 | `ApprovalCardState::new` |
| Esc peels non-top dismissible | P0 | `OverlayHost::dismiss_top_esc` |
| VirtualGrid phantom rows / enabled ignored / `row_id: None` | P0 | `virtual_grid.rs` |
| DesignTokens unused in paint | P0 | list/tree/table |
| Intent only on List | P0 | widgets/* |
| Scene has no layers/actions/input ownership | P0 | `scene.rs` |
| Stream not variable-height / no id anchor | P1 | `StreamView` |
| WorkSurface can overflow narrow parents | P1 | `work_surface.rs` |
| CompletionMenu move/activate coupling risk | P1 | completion_menu |
| PromptBox key contract docs vs code drift | P1 | agent PromptBox |
| Hover/focus/selection styles overloaded (LinkHover for hover) | P2 | list paint |
| No global Simple/Vim mode profile | P2 | keymap |

---

## 13. Proposed target architecture

```text
┌─────────────────────────────────────────────────────────────┐
│  termrock-cli / registry (source-owned blocks)     [later]  │
├─────────────────────────────────────────────────────────────┤
│  blocks/ patterns (AgentWorkbench, OpsDashboard, …)         │
├─────────────────────────────────────────────────────────────┤
│  components (crate + eventually copyable)                   │
│    chrome · data · input · feedback · agent · media         │
├─────────────────────────────────────────────────────────────┤
│  KERNEL crate (stable, compiled)                            │
│    Session · FrameTick · Keymap                             │
│    DesignSystem (Theme + Density + Motion + Glyphs + …)     │
│    InteractionScene (focus + hit + layer + esc + actions)   │
│    text/unicode · scroll · layout workspace tree            │
│    input events · capability profile                        │
├─────────────────────────────────────────────────────────────┤
│  Ratatui / terminal backend                                 │
└─────────────────────────────────────────────────────────────┘
```

### Core type sketches

```rust
// --- Design system ---
pub struct DesignSystem {
    pub theme: Theme,
    pub density: Density,
    pub motion: Motion,
    pub glyphs: GlyphSet,
    pub spacing: SpacingScale,
    pub selection: SelectionChrome,
    pub capability: ColorCapability,
}

impl DesignSystem {
    pub fn list_row(&self, st: RowVisualState) -> ListRowRecipe { … }
    pub fn panel(&self, emphasis: PanelEmphasis) -> PanelRecipe { … }
    pub fn quantized(self) -> Self { /* apply capability to theme */ }
}

// --- Interaction scene (immediate mode) ---
pub struct InteractionScene<Id, ScopeId, Action> { … }

pub struct ElementReg<'a, Id, ScopeId, Action> {
    pub id: Id,
    pub scope: ScopeId,
    pub area: Rect,
    pub layer: LayerId,
    pub focusable: bool,
    pub enabled: bool,
    pub input_owner: bool,
    pub esc: EscPolicy, // Dismiss | Trap | Bubble
    pub actions: &'a [BoundAction<Action>],
}

pub enum EscPolicy { Dismiss, Trap, Bubble }

pub enum SceneEvent<Id, Action> {
    Focus(FocusOutcome<Id>),
    Action { target: Id, action: Action },
    LayerDismissed { id: Id },
    UnhandledEsc,
}

impl<Id, ScopeId, Action> InteractionScene<Id, ScopeId, Action> {
    pub fn begin_frame(&mut self);
    pub fn register(&mut self, ElementReg<'_, Id, ScopeId, Action>);
    pub fn reconcile(&mut self) -> FocusOutcome<Id>;
    pub fn handle_key(&mut self, key: KeyEvent, map: &Keymap<Action>) -> SceneEvent<Id, Action>;
    pub fn handle_pointer(&mut self, e: MouseEvent) -> SceneEvent<Id, Action>;
    pub fn discoverable_actions(&self) -> impl Iterator<Item = &BoundAction<Action>>;
}

// --- Intents as default action family ---
pub enum UiIntent { Move(NavigationMove), Page(PageMove), Activate, Toggle, Open, Close, Cancel, Submit, Expand, Collapse }

pub trait IntentSurface {
    type Outcome;
    fn handle_intent(&mut self, intent: UiIntent) -> Self::Outcome;
}

// --- Workspace ---
pub enum PaneNode<Id> {
    Leaf { id: Id, min: Size },
    Split { id: Id, axis: Axis, ratio: u16, a: Box<Self>, b: Box<Self> },
    Tabs { id: Id, active: usize, children: Vec<(Id, Box<Self>)> },
    Stack { id: Id, front: usize, children: Vec<Box<Self>> },
}

// --- Transcript ---
pub struct TranscriptBlock<'a, Id> {
    pub id: Id,
    pub kind: BlockKind,
    pub folded: bool,
    pub content: BlockContent<'a>,
}
pub struct TranscriptAnchor<Id> { pub id: Id, pub row: u32 }
```

**Immediate mode stays:** scene rebuilds each frame; no retained widget tree or callbacks in the kernel.

---

## 14. Proposed component taxonomy

```text
kernel/
  session, time, input, keymap, design, scene, scroll, text, layout

chrome/
  Panel, Tabs, StatusBar, HintBar, ActionBar, Breadcrumb, ModeRibbon

content/
  Typography, MarkdownView, CodeBlock, DiffView, ImageSurface, Empty/Loading/Error

data/
  List, Tree, Table, VirtualGrid, DetailTable, LogPane, ScrollArea

input/
  TextInput, TextArea, Form, Checkbox, Select, Picker, CompletionMenu, CommandPalette

feedback/
  Toast, Banner, Progress, Skeleton, Tooltip

overlay/
  Dialog, MessageDialog, ChoiceDialog, Menu, ContextMenu, Popover, JumpOverlay

agent/   (flagship pack)
  Transcript, ToolCard, ApprovalCard, PromptBox, Timeline, TokenMeter, ThinkingBlock, ActivityRail

patterns/blocks/
  AgentWorkbench, OpsDashboard, ResourceBrowser, FormWizard, SettingsShell

charts/
  Sparkline, BarSeries, SegmentedMeter, …
```

---

## 15. Phased implementation roadmap

### Phase 0 — Stop the bleeding (1–2 weeks) **P0**

1. ApprovalCard safe defaults + visible option clamp.  
2. VirtualGrid: no phantoms, honor enabled, stable IDs on outcomes.  
3. Esc: dismiss **top layer only** if policy allows.  
4. Contract tests + stories.

→ Plans **039**, overlay fix (may fold into 040).

### Phase 1 — One interaction truth (2–4 weeks) **P0**

5. `InteractionScene` replaces app-side glue.  
6. Dogfood CommandPalette, CompletionMenu, ApprovalCard, JumpOverlay.  
7. Keymap → scene actions; hints from discoverable actions.

→ Plan **040**.

### Phase 2 — Design system real (2–3 weeks) **P1**

8. Phosphor: surfaces + rare accent (“quiet canvas, bright intent”).  
9. All chrome lists/panels consume `DesignSystem` recipes.  
10. Lookbook density + capability stories; fail catalog if widget ignores tokens (policy).

→ Follow-on after 040; related plans **043–044**.

### Phase 3 — Flagship agent surface (3–5 weeks) **P1**

11. Variable-height transcript engine (plan **041**).  
12. Workspace tree (plan **042**).  
13. AgentWorkbench block story end-to-end.

### Phase 4 — Anatomy & collections (2–4 weeks) **P1–P2**

14. Composed rows / panel slots.  
15. Intent-ify Tree/Table/Picker.  
16. Menu/Select/ScrollArea primitives.

### Phase 5 — Distribution (shadcn moment) **P1**

17. `termrock` CLI + manifest + first copyable block.  
18. `diff` / dirty protection.  
19. Docs: “kernel crate + owned blocks.”

### Phase 6 — Studio & polish **P2–P3**

20. Scene/token/capability inspector.  
21. Broader component set; charts/media depth; Windows later if desired.

---

## 16. First 10 pull requests (in order)

> Note: project may prefer main-only or single long-lived PR; treat these as **logical PR slices** even if stacked on one branch.

| # | Title | Rank | Outcome |
|---|-------|------|---------|
| **PR1** | fix(security)!: ApprovalCard risk-aware defaults + visible decisions only | P0 | High-risk Enter ≠ AllowOnce |
| **PR2** | fix(grid)!: VirtualGrid enabled, no phantoms, stable row_id outcomes | P0 | Honest data grid |
| **PR3** | fix(overlay)!: Esc dismisses top layer only (EscPolicy) | P0 | No under-peel |
| **PR4** | feat(scene)!: InteractionScene focus+hit+layer+actions | P0 | One interaction truth |
| **PR5** | feat(scene): migrate CommandPalette + CompletionMenu + Jump + Approval onto scene | P0 | Dogfood |
| **PR6** | feat(style)!: DesignSystem drives List/Panel paint + phosphor redesign | P1 | Tokens live; quiet canvas |
| **PR7** | feat(intent)!: Tree/Table/Picker handle_intent + keymap bridge | P1 | Global remaps |
| **PR8** | feat(transcript)!: variable-height streaming transcript engine | P1 | Agent flagship core |
| **PR9** | feat(layout)!: Workspace pane tree + AgentShell as tree | P1 | Responsive workspaces |
| **PR10** | feat(dx): termrock CLI spike + one installable block + manifest | P1 | shadcn distribution path |

**Do not start PR10 before PR4–PR7** or copied sources freeze wrong patterns.

---

## Ranked finding rollup

### P0

- Approval default allow  
- VirtualGrid contract lies  
- Esc under-peel  
- Kernel types unused (tokens, scene, intents incomplete)  
- No source-owned distribution path (for *shadcn* goal)  
- Parallel focus/overlay/scene stacks  

### P1

- Phosphor flat hierarchy  
- Thin Panel/ListRow anatomy  
- StreamView not transcript-grade  
- Flat WorkSurface / pattern rects  
- Lookbook not studio  
- Incomplete capability stories  
- Agent pack not a workbench  

### P2

- Missing Menu/Select/ScrollArea/Button family  
- Spatial focus graph  
- Broader charts/media  
- Visual regression beyond SVG  

### P3

- Windows/RTL  
- Registry marketplace  
- Avatar/breadcrumb polish  

---

## Closing judgment

TermRock is **not** “behind” on random widgets. It is **ahead** on the hard terminal problems and **behind** on (1) making new kernel types **mandatory and coherent**, (2) **safety contracts**, (3) a **real visual system**, and (4) **source ownership**.

Execute **PR1–PR5** immediately; that turns TermRock from a premium crate into a **design-system kernel**. PR6–PR9 make it **feel premium**. PR10 makes it **shadcn**.
