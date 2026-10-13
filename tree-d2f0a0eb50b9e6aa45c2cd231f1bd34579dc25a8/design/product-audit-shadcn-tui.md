# TermRock complete product audit — path to shadcn of TUI

**Status:** audit SoT (living)  
**HEAD inspected:** `274a5ce` (re-verify before executing PRs)  
**Policy:** quality over compatibility; breaking preferred  
**Related:** `shadcn-tui-strategic-brief.md`, `pre-1.0-api-redesign.md`, `experience-research-2026.md`, `source-owned-registry.md`, `termrock-studio.md`, `termrock-agent.md`

**Priority legend**

| Rank | Meaning |
|------|---------|
| **P0** | Foundational and blocking |
| **P1** | Critical to product quality |
| **P2** | Major improvement |
| **P3** | Valuable refinement |

---

## 1. What TermRock already does exceptionally well

| Strength | Evidence | Rank to protect |
|----------|----------|-----------------|
| **Ownership model** | Borrowed render data + stable IDs; domain/effects stay consumer (`COMPONENTS.md`) | P0 keep |
| **Session lifecycle** | `crossterm::Session` raw/alt-screen/mouse/paste/wrap/cursor with rollback and idempotent Drop | P0 keep |
| **Interaction kernel direction** | `InteractionScene`, `OverlayStack`, `UiIntent`, layer/dismiss policies, placement | P0 keep & sole-path |
| **Focus + hit geometry culture** | Per-frame registration patterns; hover ≠ keyboard steal; painted hit regions on states | P0 keep |
| **Unicode / grapheme editing** | `text/*`, TextInput/TextArea, unicode stories, clip/sanitize | P0 keep |
| **Narrow / responsive grammar** | `ViewportClass`, `ContractionStage`, anatomy priorities, narrow stories (~222 SVGs) | P1 keep |
| **Data virtualization** | VirtualGrid, Tree/Table/LogPane hot-path budgets (`perf` module, `*_hot_path` tests) | P1 keep |
| **Design token *types*** | `DesignSystem`, `SelectionChrome::Gutter`, `ListRowRecipe`, `PanelRecipe`, density/motion/glyphs | P1 keep & enforce |
| **Agent surface inventory** | PromptComposer, PermissionPrompt, PlanReview, TaskRail, Transcript, ToolCard, ModeRibbon… | P1 keep & unify |
| **Lookbook discipline** | Real public API stories, SVG gate, contracts JSON, knobs/inspector seed | P1 keep → Studio |
| **Registry CLI seed** | `termrock-cli` install plan/apply/diff, path/symlink safety, fixtures (6 packs) | P1 keep → gravity |
| **Migration culture** | Numbered `migrations/` + `MIGRATING.md` through `0056` | P0 keep |
| **Capability architecture** | Profiles, doctor, quantize hooks, `NO_COLOR` path | P1 keep & story-enforce |
| **Product direction clarity** | README line + strategic brief SoT | P0 keep |

**Bottom line:** TermRock’s invisible terminal correctness (lifecycle, IDs, geometry, unicode, budgets, catalog gates) is already **category-leadership material**. The failure mode is dual authorities and incomplete distribution/agent cohesion—not empty crates.

---

## 2. What prevents a coherent, premium design system feel

| Finding | Rank | Evidence |
|---------|------|----------|
| Dual interaction authorities | **P0** | Public `FocusRing` + `InteractionScene`; `ModalStack` + `OverlayStack` |
| Dual paint authorities | **P0** | Public `Theme` + `DesignSystem`; lib docs still “Entry point: Theme” |
| Phosphor hierarchy empty | **P1** | `Theme::tailrocks_phosphor` roles Canvas/Surface/Elevated/Backdrop = `Style::new()`; Selection = full green fill |
| Recipe not universal | **P1** | List/Tree use `list_row_recipe`; Table/Menu/Completion often `Role::Selection` fill |
| Intent adoption incomplete | **P0** | `handle_intent` only on List/Tree/Table/Picker/Completion/VirtualGrid; many surfaces raw `handle_key` |
| Dual agent chrome | **P1** | `PromptBox`/`ApprovalCard` + `PromptComposer`/`PermissionPrompt` |
| Quad data grids | **P1** | `Table`, `VirtualGrid`, `DetailTable`, `DataTable` without single consumer path |
| Patterns are geometry stubs | **P1** | `patterns/*` layout helpers, not installable owned blocks with state/focus |
| Crate-root export dump | **P2** | Wholesale `pub use` of interaction/layout/capability/perf/style → ownership signal lost (~27k public-api lines) |
| Widget-local focus flags | **P1** | `*State.focused` parallel to scene focus |

Until one authority per concern ships, chrome cannot feel premium even with more widgets.

---

## 3. What prevents shadcn/ui equivalence for Ratatui

| shadcn pillar | TermRock today | Rank |
|---------------|----------------|------|
| Own the source (`add` into your tree) | Git crate pin default; CLI install exists but not inevitable | **P0** |
| Manifest + provenance + dirty-safe update | Partial digests/diff; not full three-way product | **P1** |
| Blocks (page recipes) | Fixtures + patterns geometry | **P1** |
| Token-driven composition | Types exist; Theme path still dominates docs/examples | **P0** |
| Storybook as product | Lookbook strong seed; not Studio | **P1** |
| Coherent primitives → compounds | Anatomy uneven (Panel thin; rows partial) | **P1** |
| AI-friendly inspectable surface | Open source yes; semantic scene not productized as DevTools | **P2** |

**shadcn is a distribution + ownership system**, not a Button. TermRock must complete hybrid **kernel crate + source registry** or it remains “dependency with good bones.”

---

## 4. APIs that should be preserved

| API / subsystem | Why |
|-----------------|-----|
| Borrowed data + stable IDs | Composition without state soup |
| `Session` lifecycle semantics | Correctness moat |
| Neutral `input::Event` | Backend independence |
| `InteractionScene` (as sole path) | Focus/hit/layer/action discovery |
| `OverlayStack` (as sole path) | Esc/z-order law |
| `UiIntent` + navigation enums | Remappable chrome |
| `Keymap` tables | Dispatch + hints + conflicts |
| `DesignSystem` + recipes + density/motion/glyphs | Paint authority target |
| `Role` exhaustiveness | Semantic styling |
| Unicode `text` / `ansi_text` | Terminal truth |
| Scroll math + paint split | Perf + reuse |
| Responsive layout grammar | Narrow contracts |
| `perf` budgets / stream coalescer / follow | Production streaming |
| Capability resolve + doctor | Progressive enhancement |
| PromptComposer / PermissionPrompt / Transcript / PlanReview / TaskRail | Agent pack cores (after dual kill) |
| DataTable + VirtualGrid guts | Scale |
| Lookbook story/SVG/contract gates | Quality machine |
| Registry CLI path validation | Security |

---

## 5. APIs to redesign or remove (breaking OK)

| Surface | Action | Rank |
|---------|--------|------|
| Public `FocusRing` | **X** fold into Scene | P0 |
| Public `ModalStack` | **X** → OverlayStack | P0 |
| `Theme` as paint entry | **R** → internal RolePalette under DesignSystem | P0 |
| Crate-root blanket re-exports | **X** module-scoped imports | P1 |
| `PromptBox` | **X** → PromptComposer only | P1 |
| `ApprovalCard` | **X** → PermissionPrompt only | P1 |
| `StreamView` | **X** → Transcript only | P1 |
| Parallel Table/DetailTable public mega-APIs | **S** one DataTable path | P1 |
| Widget-local `focused: bool` as authority | **D** scene owns focus | P1 |
| Public raw `handle_key` on collections | **R** adapter or private; intent is law | P0 |
| `patterns` as crate public product | **I** source-owned blocks | P1 |
| lib rustdoc “Entry point: Theme” | **R** DesignSystem + InteractionScene | P2 |

Detailed matrix: `pre-1.0-api-redesign.md`.

---

## 6. Missing foundational primitives

| Primitive | Rank | Notes |
|-----------|------|-------|
| Sole `UiContext` per frame | P0 | design + caps + keymap + scene + overlays + clock |
| `FocusGraph` (zones + roving + spatial) | P1 | evolve Scene beyond linear tab |
| Universal `EventResult<M>` | P1 | consumed/message/redraw/focus/overlay requests |
| Complete `ComponentRecipes` map | P1 | Button/MenuItem/Tab/Dialog parts |
| SpacingScale enforcement in paint | P1 | not optional metadata |
| Semantic scene query API | P1 | jump, hints, Studio, a11y export |
| Headless behavior cores | P1 | CollectionState, ChoiceState, SelectionModel separated from chrome |
| Inline / Static render modes | P2 | not only alternate-screen apps |
| Image protocol behind capability | P2 | PreviewHost complete ladder |

### Sketches

```rust
pub struct UiContext<'a> {
    pub design: &'a DesignSystem,
    pub capabilities: &'a CapabilitySet,
    pub keymap: &'a Keymap<AppAction>,
    pub scene: &'a mut InteractionScene<Id, LayerId, AppAction>,
    pub overlays: &'a mut OverlayStack<Id>,
    pub clock: FrameClock,
}

pub struct EventResult<M> {
    pub consumed: bool,
    pub message: Option<M>,
    pub redraw: Redraw,
    pub focus: Option<FocusRequest<Id>>,
    pub overlay: Option<OverlayRequest<Id>>,
}
```

---

## 7. Missing high-level components

Many names exist in inventory; **missing as contract-complete, recipe-driven, intent-driven products**:

| Component | Rank |
|-----------|------|
| Button / IconButton with loading+danger anatomy | P1 |
| SegmentedControl / Switch (if partial, finish contracts) | P2 |
| Breadcrumbs, Pagination | P2 |
| ContextMenu / HoverCard | P1 |
| Tooltip (non-hover-only; keyboard peer) | P1 |
| Drawer/Sheet complete overlay registration | P1 |
| TreeTable | P2 |
| ObjectInspector (JSON/YAML) | P2 |
| StreamingMarkdown quality bar | P1 |
| MetricTile / Heatmap (ops density) | P2 |
| Skeleton/Empty/Error universal contracts | P1 |
| KeyboardHelp generated from keymap | P1 |

---

## 8. Missing application blocks

| Block | Rank | Inspired by |
|-------|------|-------------|
| `agent-workbench` (full session contract) | P0 | Grok, Amp, OpenCode |
| `agent-shell` (minimal) | P1 | Claude-density |
| `settings-shell` (MCP/tools as panels) | P1 | Amp |
| `ops-dashboard` (metrics+tables+log) | P2 | btop + k9s |
| `resource-browser` | P2 | k9s, lazydocker |
| `dual-pane-preview` | P2 | yazi |
| `review-diff` | P1 | Grok plan/diff |
| `form-wizard` (already fixture—productize) | P1 | Huh |
| `studio-shell` | P1 | Storybook |
| `palette-host` | P1 | Amp Ctrl+O |

Fixtures under `registry/fixtures/*` prove packaging; not yet flagship product experience.

---

## 9. Missing developer tooling

| Tool | Rank |
|------|------|
| `termrock init` excellent TUI | P1 |
| `termrock add/search/view` product gravity | P0 |
| Three-way `update` preserving local ownership | P1 |
| Studio inspector (focus/hits/layers/tokens/events) | P1 |
| Capability emulator knobs | P1 |
| Record/replay traces (`.trock`) | P2 |
| Design linter (color-only state, missing gutter, clipped primary) | P1 |
| Public-api ownership budget (forbid dual types) | P0 |
| Compilable handbook examples CI | P1 |

---

## 10. Missing quality contracts

Existing: keyboard/mouse/focus/narrow/unicode/non-color axes in contracts JSON — **good**.

**Expand (P1–P2):**

| Axis | Cases |
|------|-------|
| Color | none, 16, 256, truecolor, light/dark |
| Glyphs | ASCII, Unicode, CJK, emoji, ambiguous width |
| Input | paste, enhanced keyboard, drag |
| Env | SSH, tmux, inline vs alt-screen |
| Streaming | burst, pause follow, coalesce |
| Overlay | nested Esc, non-dismissible trap, opener restore |
| Agent | continuity, default-deny, queue while busy |
| Perf | named budgets fail CI (extend to composer/transcript) |

---

## 11. Visual-design weaknesses (screenshots / SVG previews)

From phosphor defaults + SVG catalog behavior:

| Weakness | Rank |
|----------|------|
| Empty Canvas/Surface/Elevated/Backdrop in phosphor | **P1** |
| Selection full-row green fill dominates | **P1** |
| Accent/Focus/Success converge on same green | **P1** |
| Box soup risk (many bordered panels) | **P2** |
| Disabled/focused distinction historically SVG-fragile (partially fixed with RGB ActionFocused/Disabled) | **P2** |
| Elevation hierarchy invisible on default theme | **P1** |
| Selection chrome recipe not applied universally | **P1** |

**Principle:** quiet canvas, bright intent; borders = ownership/focus/security, not every section.

---

## 12. Interaction weaknesses screenshots hide

| Weakness | Rank |
|----------|------|
| Dual focus authorities | P0 |
| Esc may not peel single conceptual layer if stack bypassed | P0 |
| Raw keys in agent/form prevent remapping | P0 |
| Composer continuity not a named CI guarantee | P1 |
| Permission dual implementations risk wrong defaults | P0 |
| Widget `focused` vs scene focus dual truth | P1 |
| Hover-only discovery without keyboard peer | P1 |
| Patterns without input ownership | P1 |
| Incomplete intent on DetailTable/LogPane/Menu | P1 |
| No Focus Lens / jump from semantic scene productized | P2 |

---

## 13. Proposed target architecture

```text
┌─ Application ─────────────────────────────────────────────┐
│  src/ui/* (owned components)  src/blocks/*  src/themes/*   │
└──────────────────────────▲────────────────────────────────┘
                           │ termrock add | diff | update
┌──────────────────────────┴────────────────────────────────┐
│  Registries: primitives · components · blocks · themes     │
└──────────────────────────▲────────────────────────────────┘
                           │ depends
┌──────────────────────────┴────────────────────────────────┐
│  termrock kernel crates (compiled)                         │
│  core: ids, geometry, intent, scene, overlay, scroll, text │
│  runtime: session, tick, capability                        │
│  style: DesignSystem                                       │
│  widgets: neutral primitives + headless behaviors          │
│  perf, keymap                                              │
└────────────────────────────────────────────────────────────┘
```

### Package sketch

```rust
// termrock (umbrella) re-exports kernel only — no patterns, no brand packs

// Kernel public surface (tight)
pub mod interaction {
    pub struct InteractionScene<Id, Layer, Action> { /* sole focus/hit/layer */ }
    pub struct OverlayStack<Id> { /* sole float law */ }
    pub enum UiIntent { /* … */ }
}
pub mod style {
    pub struct DesignSystem { /* sole paint */ }
    pub enum Role { /* … */ }
}
pub struct UiContext<'a, Id, Layer, Action> { /* per frame */ }
```

**Law:** widgets emit `EventResult` / typed outcomes; apps own effects.

---

## 14. Proposed component taxonomy

| Tier | Contents |
|------|----------|
| **Kernel** | Scene, Overlay, Intent, Keymap, DesignSystem, Session, text, scroll, capability, perf |
| **Primitives** | Text, Surface, Separator, Badge, Kbd, Spinner, Progress, Skeleton, Empty/Error |
| **Controls** | Button, Checkbox, Switch, TextInput, TextArea, Select, Tabs, Menu |
| **Collections** | List, Tree, DataTable, VirtualGrid, Picker, CommandPalette |
| **Overlays** | Dialog, Popover, Tooltip, Drawer, CompletionMenu, JumpOverlay |
| **Data/Dev** | LogStream, DiffReview, Timeline, Metrics, CodeBlock, Markdown |
| **Agent (registry)** | Composer, Transcript, ToolCard, Permission, PlanReview, TaskRail, SessionPicker |
| **Blocks (registry)** | Workbench, OpsDashboard, ResourceBrowser, SettingsShell, FormWizard, StudioShell |

---

## 15. Phased roadmap

| Phase | Outcome | Rank focus |
|-------|---------|------------|
| **A** | Dual-authority kill; public API shrink | P0 |
| **B** | UiIntent universal; OverlayStack sole | P0 |
| **C** | Phosphor Obsidian; recipes everywhere | P1 |
| **D** | Registry gravity (`init/add/diff/update`) | P0–P1 |
| **E** | Agent workbench contracts + continuity | P1 |
| **F** | Studio inspector + capability stories | P1–P2 |
| **G** | Ops/data blocks; metrics density | P2 |
| **H** | Inline/static modes; Windows/ConPTY | P2–P3 |

Do **not** start by adding 40 widgets.

---

## 16. First 10 pull requests

| # | PR title | Scope | Rank |
|---|----------|-------|------|
| **1** | `feat!: InteractionScene sole focus authority` | Delete public FocusRing; lookbook migrate; migration | P0 |
| **2** | `feat!: OverlayStack sole float law` | Delete public ModalStack; all floaters register; Esc tests | P0 |
| **3** | `feat!: DesignSystem sole paint authority` | Theme internal; lib entry redesign; public-api regen | P0 |
| **4** | `feat!: UiIntent on Form, agent chrome, menus` | Remove public raw-key dependency; keymap stories | P0 |
| **5** | `feat!: Phosphor Obsidian + universal list/table recipes` | Elevation RGB; gutter selection; SVG stories | P1 |
| **6** | `feat!: single agent chrome path` | Remove PromptBox/ApprovalCard; Permission+Composer only | P1 |
| **7** | `feat!: one DataTable consumer path` | Demote parallel grids from public “pick any” | P1 |
| **8** | `feat(cli): termrock init + add gravity` | termrock.toml; install agent-workbench pack | P1 |
| **9** | `feat(agent): continuity + default-deny contracts` | CI tests; workbench story | P1 |
| **10** | `feat(studio): scene inspector v1` | Focus/hits/layers/tokens on public APIs | P1 |

Each PR: Conventional Commit + DCO (`-s`), migration file if breaking, lookbook/contracts green, push `main` per Agents.md.

---

## Finding index (ranked)

### P0
- Dual FocusRing/Scene, ModalStack/OverlayStack, Theme/DesignSystem  
- Incomplete UiIntent adoption  
- Esc/overlay law incomplete  
- Registry not default DX  
- Permission dual path risk  

### P1
- Phosphor flat hierarchy / selection fill  
- Dual agent widgets  
- Quad grids  
- Patterns not blocks  
- Composer continuity / trust contracts  
- Studio inspector  
- Capability story enforcement  
- Anatomy (Panel/row slots)  

### P2
- Crate-root export soup  
- Metrics/ops density pack  
- Record/replay  
- Inline/static render modes  
- TreeTable / ObjectInspector  

### P3
- Community registries  
- Windows/ConPTY polish  
- Focus Lens productization  

---

## Success metric

Not widget count.  
**Yes:** one authority per concern · owned installable blocks · quiet phosphor · remappable intents · Esc law · agent continuity + default-deny · Studio evidence · profile degrade.

> TermRock is the source-owned design system for building exceptional terminal software on Ratatui.
