# Archived direction brief (executed)

> Historical research that drove migrations `0029` and `0030` and the experience-layer
> widgets. **Plans 001–038 were verified done and removed from `plans/`** — this file
> remains as design history. Open product work continues via new migrations only.

---

# TermRock as shadcn/ui for TUI (direction history)

**Status:** RESEARCH (not an implementation plan)  
**Written against:** `855a049` (2026-08-09 session)  
**Kind:** Product/architecture direction from landscape research  
**Breaking changes:** Explicitly allowed; quality over compatibility  

This document is the durable brief from a deep landscape + codebase survey.
It is **not** executable by itself. Promote selected items into sequential
implementation/spike plans (`039+`) before coding.

---

## 1. Executive verdict

TermRock is already a **serious component kit**: semantic `Role`/`Theme`,
stable-ID widgets, FocusRing + modal scopes, lookbook/SVG contracts, hot-path
tests, migration discipline, forward-only API. That is rarer than most Ratatui
ecosystem crates.

It is **not yet** the shadcn of TUI because shadcn won on:

1. A complete **experience language** (not only buttons/tables)
2. **Blocks/recipes** people copy to ship product screens
3. Identity-level **defaults** people screenshot
4. Distribution that makes adoption feel inevitable

**Win condition:** *“Built on TermRock”* becomes the same flex as *“uses
shadcn.”* White space in Rust is real: Ratatui owns the engine; nobody owns the
product component system + agent-era kit + lookbook + contracts.

---

## 2. TermRock inventory (baseline)

### Public widgets (25)

`ActionBar`, `Backdrop`, `ChoiceDialog`, `CompletionMenu`, `DetailTable`,
`Dialog`, `DiffView`, `Form`, `HintBar`, `List`, `LogPane`, `MessageDialog`,
`Panel`, `Picker`, `Progress`, `SplitPane`, `StatusBar`, `Table`, `Tabs`,
`TextArea`, `TextInput`, `Toast`, `Tree`, `Viewport`, `VirtualGrid`.

### Foundations already shipped (plans 001–037 largely DONE)

- Semantic theme (`Role` + `Theme::tailrocks_phosphor` / `Theme::slate`)
- Neutral input events, runtime `FrameTick` runner, configurable `Keymap`
- FocusRing with scoped modal restoration
- Catalog: contracts JSON, SVG previews, docs site, public-api inventory
- Performance gates on heavy widgets

### Structural strengths to keep

| Strength | Why it maps to shadcn |
|----------|----------------------|
| Consumer owns domain; library owns chrome | Same as shadcn ownership split |
| Stable IDs + borrowed render data | Composable, no hidden state soup |
| Focus-visible = theme role, not double borders | Coherent design system |
| Lookbook + contracts | Storybook equivalent |
| Breaking migrations documented | Forward-only craft culture |

### Structural gaps (product, not polish)

| Gap | Symptom |
|-----|---------|
| No **experience OS** | Overlay z-order, Esc cascade, jump-mode missing as first-class |
| No **agent kit** | Stream, tool cards, approvals, prompt chrome reinvented per app |
| No **rich text** | Markdown/syntax as product requirement elsewhere |
| No **multi-pane shell** | Only `SplitPane`; lazygit/k9s need WorkSurface |
| No **density/motion tokens** | One spacing feel |
| Theme 2.0 incomplete vs Grok Build | No live preview, quantize ladder, OS auto (optional) |
| No **blocks/recipes** layer | Components yes; product screens no |
| Charts / media | btop/yazi class density not expressible |

---

## 3. What “shadcn for TUI” means (port principles, not React)

| shadcn principle | TermRock expression |
|------------------|---------------------|
| Open, inspectable source | Crate stays open; optional later registry |
| Composable primitives | Keep widgets thin; add composites carefully |
| Strong defaults, full override | Phosphor default forever; full retheme |
| Blocks (page recipes) | `patterns/` or documented compositions |
| Storybook | Lookbook + docs site (extend) |
| Breaking OK until “stable” | Already law in AGENTS.md |

**Do not** mirror React hooks/JSX. **Do** mirror craft: one way to build
beautiful apps without reinventing focus, hints, empty states, overlays.

---

## 4. Landscape: apps people call “another level”

Consensus sources: rothgar/awesome-tuis, awesometui.com, HN/Reddit praise,
agent-era writeups.

### 4.1 Multi-panel masters

**lazygit, gitui, k9s, lazydocker**

Design DNA:

- Fixed spatial layout → muscle memory
- One focused pane; chrome consistent
- Context keybindings always visible
- Power without hiding the underlying tool (lazygit command log)

**Port concepts**

- `WorkSurface` (named regions, ratios, collapse, focus order)
- Scoped keymap → auto `HintBar`
- `ActivityTrail` / command log strip

### 4.2 Density art

**btop, bottom, nvtop, glances**

Design DNA:

- Graphs as language
- Every cell earns keep
- Color = signal

**Port concepts**

- `Sparkline`, `BarSeries`, `SegmentedMeter`, `Heatmap`, `MetricTile`

### 4.3 Modern fluid file UX

**yazi, superfile, nnn, lf, broot**

Design DNA:

- Async feel (UI never blocks)
- Preview pane protocol
- Image/media in terminal (yazi)
- Dual/triple column orthodoxy

**Port concepts**

- `PreviewHost` + content provider trait
- Optional Kitty/Sixel/iTerm image surface (feature-gated)
- Loading skeleton states

### 4.4 IDE-in-terminal

**posting, harlequin, rainfrog, ATAC, euporie**

Design DNA:

- Jump-mode (posting): letter targets on UI regions
- `$EDITOR` handoff
- Tree-sitter / syntax as non-negotiable
- Familiar GUI layouts translated, not dumbed down

**Port concepts**

- `JumpOverlay`
- `EditorHandoff`
- `CodeBlock` + pluggable syntax trait
- `MarkdownView`

### 4.5 Pure focus / charm

**fzf, gum, huh, glow, crush (Charm AI agent)**

Design DNA:

- One job, zero chrome waste
- Instant filter loop
- Aesthetic identity people share screenshots of

**Port concepts**

- Shared filter/score protocol across List/Picker/CommandPalette
- Theme presets worth photographing
- Micro-motion (spinners, soft progress) on `FrameTick`

### 4.6 Bling craft

**notcurses, Textual gallery, charm apps**

Design DNA:

- Modern terminal features first-class
- Micro-animation
- Separated style systems (Textual CSS)

**Port carefully**

- Motion tokens, not full CSS layout (fights Ratatui)
- Braille/block canvas primitive optional
- Live theme tweak in lookbook

### 4.7 AI agent TUI leaders (2025–2026 bar)

These define the new “incredible” standard for developer TUIs.

#### Grok Build (primary harness reference)

Documented UX to productize as **neutral** components:

| Pattern | Behavior | TermRock gap |
|---------|----------|--------------|
| Theme system | Live `/theme` preview, auto OS light/dark, RGB→256/16 quantize | Theme exists; no picker widget, no quantize, no OS auto |
| Scrollback blocks | Fold/expand turns, thinking, raw md, copy block | No stream/fold model |
| Tool cards | Mutable streaming cards | No `ToolCard` |
| Permission cards | Blocking allow/deny + focus trap | Partial (`ChoiceDialog`) |
| Dual input modes | Simple vs Vim scrollback; Esc cascade | Keymap yes; mode profiles + Esc stack no |
| Slash + @ menus | Nested completion | `CompletionMenu` needs command protocol |
| Agent dashboard | Multi-session roster, peek, dispatch | No multi-entity status list pattern |
| Minimal vs fullscreen | Two chrome modes | Session options only |
| Density chrome | Tokens, activity, focus hints | Status/Hint bars exist; no density presets |

#### Amp (ampcode.com)

Public signal:

- TUI often cited as convert-grade (Tim Culverhouse / rockorager lineage;
  Ghostty subsystem; libvaxis author)
- Double-buffered redraw, high FPS claims (~60)
- Thread as first-class UI object
- Plugin UI sync web ↔ TUI
- Subagent personas with distinct chrome
- Remote runners without breaking interaction model

**Port concepts:** `ThreadTimeline`, identity chips, extension slots,
runner status strip, smooth partial redraw discipline (already hot-path
culture — extend to streaming cards).

#### OpenCode, Crush, Claude Code, Codex

Community comparisons (not endorsements):

- **OpenCode:** structured tool calls + readable diffs; Plan vs Build modes;
  Bubble Tea; “sees state of session” vs scroll spam
- **Crush:** “pretty terminal” Charm aesthetic
- **Claude Code:** conversational depth; Ink/TS redraw criticism common;
  streaming markdown + approvals repeatedly reimplemented in clones
- **Codex:** snappy Rust feel praised vs heavy harnesses

**Cross-agent UI primitives everyone reimplements badly:**

1. Streaming markdown / message stream  
2. Mutable tool call cards  
3. Inline permission / approval  
4. Multiline prompt with slash/@ completion  
5. Plan-mode visual chrome  
6. Subagent / parallel work visibility  
7. Token/cost chrome  

**TermRock should own these once**, product-neutral.

---

## 5. Cross-language libraries — steal / skip

| Library | Steal | Skip / adapt |
|---------|-------|--------------|
| Charm Bubbletea/Lipgloss/Bubbles/Huh | Aesthetic, form fluency, style builders | Don’t force Elm model on consumers |
| Textual | Look vs logic separation, widget gallery, live design | Full CSS layout |
| tview | Pages, flex, rich widgets | Inheritance heavy APIs |
| FTXUI | Functional composition | — |
| Spectre / Rich / pterm | Beautiful CLI formatting patterns | Not full TUI |
| notcurses | Media, modern terminal features | Complexity for all consumers |
| iocraft | Declarative DX ideas | Competing framework vs Ratatui layer |
| OpenTUI / Ink | Agent/host patterns in TS ecosystem | JS runtime assumptions |
| libvaxis (Zig) | Modern terminal protocol fidelity | Different language |
| prompt-toolkit | Wizard/readline quality | — |

**Positioning sentence:**  
Ratatui = canvas. TermRock = shadcn layer (tokens, widgets, contracts, lookbook,
patterns). Never become another Ratatui.

---

## 6. Design laws (propose adding to AGENTS.md)

1. Focus is semantic color/role, never border weight (existing).  
2. **Density modes:** Comfortable / Compact / Dashboard.  
3. Non-color always carries state (icons, underlines, markers).  
4. Keyboard primary, mouse complete, jump-mode optional power.  
5. **Streaming-first:** incremental updates without full rebuild thrash.  
6. **Esc cascade** is product: overlay → clear → cancel work → quit.  
7. Empty / loading / error / success are first-class view states.  
8. Hint bar always truthful for focused scope.  
9. Theme is identity; phosphor default; full retheme.  
10. Performance is UX; hot-path budgets stay catalog gates.  
11. Prefer one coherent breaking redesign over aliases.  

---

## 7. Target module map (breaking OK)

```
termrock
├── style/         Role, Theme, Density, Motion, optional quantize
├── input/
├── interaction/   FocusRing, ModalStack, JumpOverlay, EscCascade, OverlayHost
├── keymap/        Scoped maps + hint projection
├── layout/        Split, WorkSurface, Dock
├── scroll/
├── text/          Graphemes, ANSI, markdown spans, syntax traits
├── motion/        FrameTick-driven transitions
├── widgets/
│   ├── chrome/    Panel, StatusBar, HintBar, Tabs, Breadcrumb, Title
│   ├── data/      List, Table, Tree, VirtualGrid, DetailTable
│   ├── input/     TextInput, TextArea, Form, Picker, Completion, PromptBox
│   ├── feedback/  Toast, Progress, Skeleton, EmptyState, Banner
│   ├── media/     Diff, CodeBlock, Markdown, ImageSurface, charts
│   ├── agent/     Stream, ToolCard, ApprovalCard, Timeline, TokenBar, Thinking
│   └── overlay/   Dialog, Menu, ContextMenu, CommandPalette, Jump
├── patterns/      AgentShell, OpsDashboard, ResourceBrowser, Wizard (recipes)
└── runtime/
```

`patterns/` = shadcn blocks: opinionated, still domain-neutral.

---

## 8. Capability roadmap

### Tier 0 — Experience OS

| ID | Capability | Effort | Depends |
|----|------------|--------|---------|
| D2 | Overlay host + z-order + Esc cascade | M–L | FocusRing |
| D3 | Theme 2.0: density, motion, live preview; optional quantize/OS auto | L | Theme |
| D6 | JumpOverlay + CommandPalette pattern | M | D2 |
| D0 | Empty/Loading/Error primitives | S–M | Theme |

### Tier 1 — Agent-era kit

| ID | Capability | Effort |
|----|------------|--------|
| D1 | StreamView, ToolCard, ApprovalCard, PromptBox, Timeline, TokenMeter, ThinkingBlock | L |
| D5 | MarkdownView, CodeBlock, syntax trait, richer LogPane | L |

### Tier 2 — Multi-panel products

| ID | Capability | Effort |
|----|------------|--------|
| D4 | WorkSurface + scoped keymap/hints | L |
| D7 | Sparkline/Bar/Heat/Metric tiles | M |

### Tier 3 — Spatial + polish

Context menu, menubar, wizard/stepper, accordion, skeleton, badge/chip,
breadcrumb, editor handoff.

### Tier 4 — Distribution endgame

Lookbook as public design system; optional `termrock add <pattern>` later;
**do not** constrain today’s APIs for future registry.

### Explicit non-goals (near term)

- Windows support (separate decision)
- Full Textual CSS layout
- Compatibility facades
- Becoming an agent product (TermRock stays product-neutral)

---

## 9. API concept sketches (neutral)

### ToolCard

- Consumer: tool name, args summary, live output slice, status enum, IDs  
- TermRock: layout, spinner frames, expand/collapse, copy affordance, focus,
  narrow wrap, non-color status glyphs  

### ApprovalCard

- Outcomes: `AllowOnce | AllowSession | Always | Deny | Defer`  
- Risk via `Role::Warning` / `Danger`  
- Focus trap via existing scopes  

### StreamView

- Items: `User | Assistant | Tool | System | Thinking` with stable IDs  
- Fold policy consumer-owned; virtualized paint TermRock-owned  

### WorkSurface

- Named regions + ratio memory + collapse + focus registration  
- lazygit geometry without domain  

### JumpOverlay

- Letter badges on registered focus rects; key activates; Esc dismisses  

### Density

```text
Comfortable = agent chat
Compact     = ops tools
Dashboard   = btop-class
```

Same widgets; spacing/glyph/chrome scale changes.

---

## 10. Suggested execution phases

```
Phase A — Foundations of delight
  D2 Overlay/Esc → D3 Theme density/motion → D0 states → D6 Jump/CommandPalette

Phase B — Own the agent era
  D5 text intelligence → D1 agent kit
  Showcase: “mini agent shell” (no real LLM required — mock stream)

Phase C — Own multi-panel products
  D4 WorkSurface → D7 charts → patterns (AgentShell, OpsDashboard)

Phase D — Optional media
  Image protocols behind features; Ghostty baseline remains
```

Each phase must ship: lookbook stories, contract matrix rows, migration file,
showcase that *feels* like a daily driver.

---

## 11. Competitive map

| Layer | Owner today | TermRock role |
|-------|-------------|---------------|
| Terminal engine | Ratatui | Compose on it |
| Pretty one-shot CLI | Spectre/Rich | Adjacent only |
| Full app framework | Textual/Bubbletea | Stay library |
| Widget soup | ratatui-widgets/bubbles | Too low-level |
| **Product design system + agent kit + lookbook** | **Empty in Rust** | **Own this** |
| Agent UX | Proprietary per agent | Open neutral kit |

---

## 12. Recommended first plan promotions

When maintainers select work, spawn **separate** plans:

1. `039` — Overlay host + Esc cascade design/spike  
2. `040` — Density + motion tokens + Theme 2.0  
3. `041` — Agent kit spike (ToolCard + StreamView + ApprovalCard)  
4. `042` — WorkSurface + scoped hints  
5. `043` — Markdown/CodeBlock text intelligence  

Default order: **039 → 040 → 041 → 043 → 042** if agent consumers lead;  
swap 042 earlier if ops/dashboard consumers lead.

---

## 13. Research sources (session)

- Local TermRock tree (README, COMPONENTS, style, widgets, plans 001–037, AGENTS)
- rothgar/awesome-tuis (libraries, dashboards, file managers, productivity)
- Grok Build user-guide (theming, shortcuts, dashboard)
- Public Amp notes (TUI convert praise, double-buffer, Culverhouse/libvaxis)
- Community comparisons: OpenCode, Crush, Claude Code, Codex UX
- shadcn/ui conceptual model (not API)
- Charm, Textual, tview, FTXUI, notcurses, iocraft, posting, lazygit, btop, yazi

---

## 14. Out of scope of this research

Full correctness/security audit of every widget; crates.io packaging strategy;
Windows; real Amp/Grok binary reverse engineering; formal UX testing with users.

---

## 15. Ranked opportunities (implementer-facing)

Quality-over-compat and **breaking changes allowed**. Effort: S hours / M ~1 day /
L multi-day. Order is leverage for “shadcn of TUI,” not bug severity.

| Rank | ID | Opportunity | Outcome for consumers | Effort | Sequence note |
|------|-----|-------------|----------------------|--------|---------------|
| 1 | D2 | Overlay host + z-order + **Esc cascade** | Menus, jump, toasts, permission cards coexist without fighting | M–L | Foundation for D1/D6 |
| 2 | D3 | **Theme 2.0**: Density + Motion tokens; live theme preview widget; optional quantize/OS auto | Screenshot identity; Comfortable/Compact/Dashboard | L | After or parallel docs stories |
| 3 | D0 | Empty / Loading / Error / Success view primitives | Every screen stops looking unfinished | S–M | Cheap polish tax |
| 4 | D6 | **JumpOverlay** + CommandPalette pattern | posting-tier power UX | M | Needs D2 |
| 5 | D5 | **MarkdownView** / CodeBlock / syntax trait / richer LogPane | Agent + IDE apps stop hand-rolling text | L | Feeds D1 |
| 6 | D1 | **Agent kit**: StreamView, ToolCard, ApprovalCard, PromptBox, Timeline, TokenMeter, ThinkingBlock | Amp/Grok/Claude-class shells without proprietary chrome | L | After D2+D5 |
| 7 | D4 | **WorkSurface** + scoped keymap → auto HintBar | lazygit/k9s multi-panel products | L | Ops consumers may promote earlier |
| 8 | D7 | Sparkline / BarSeries / SegmentedMeter / Heatmap / MetricTile | btop-class density dashboards | M | After WorkSurface optional |
| 9 | D8 | PreviewHost + optional Kitty/Sixel/iTerm image surface | yazi-class previews | L | Feature-gated; terminal variance |
| 10 | D9 | **patterns/** blocks (AgentShell, OpsDashboard, ResourceBrowser, Wizard) | shadcn “blocks” distribution story | M | After D1–D4 |

**Deferred / non-goals (do not start from this research alone):** Windows support;
full Textual CSS layout; becoming an AI agent product; pixel clones of Amp/Grok;
compatibility facades; exhaustive awesome-tuis catalog implementation.

---

## 16. Baseline verification (tree drift check)

### Research-session baseline (HEAD ~`855a049`)

| Claim at research time | Evidence then | Drift then |
|------------------------|---------------|------------|
| 25 public widgets in COMPONENTS.md | Inventory sentence | None |
| Widget modules under `crates/termrock/src/widgets/` | 22 `*.rs` excl. helpers; dialog composites shared file | None material |
| `Theme::tailrocks_phosphor` + `Theme::slate` | `style/mod.rs` | None |
| FocusRing | `interaction/focus.rs` | None |
| Lookbook + contracts | present | None |
| Plans 001–037 | DONE | None |

### Post-implementation note (PR #6, branch `feat/experience-layer-shadcn-tui`)

The ranked opportunities in §15 were **implemented additively** in the same PR
as this research brief (migration `0029-v0.12.0-experience-layer.md`):

| Research ID | Shipped surface (non-exhaustive) |
|-------------|----------------------------------|
| D2 | `EscCascade`, `OverlayHost` |
| D3 (partial) | `Density`, `Motion` tokens (live theme preview / quantize still open) |
| D0 | `EmptyState`, `LoadingView`, `ErrorView`, `Banner`, `Skeleton` |
| D6 | `JumpOverlay`, `CommandPalette` |
| D5 (partial) | `MarkdownView`, `CodeBlock`, `SyntaxHighlighter` |
| D1 | `StreamView`, `ToolCard`, `ApprovalCard`, `PromptBox`, `Timeline`, `TokenMeter`, `ThinkingBlock` |
| D4 | `WorkSurface` + `patterns::layout_agent_shell` |
| D7 | `Sparkline`, `BarSeries`, `SegmentedMeter` |

**Still open after 0029:** Theme live-preview + OS auto + quantize (D3 rest),
image/graphics protocols (D8), fuller `patterns/` blocks (D9), density-axis
story coverage, and deeper markdown/syntax backends. Re-rank those as follow-on
work on the **same** PR branch or a later sequential migration — do not fork
parallel PRs for the same initiative unless policy changes.
