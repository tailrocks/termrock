# Competitive research: TermRock as a TUI design system

**Status:** research SoT (living document)  
**Date:** 2026-08-09  
**Method:** product/architecture analysis of frameworks, toolkits, and highly regarded TUI apps; no source copying  
**TermRock position:** hybrid **interaction kernel + product-neutral widgets + source-owned blocks** on Ratatui, with Studio, quality contracts, agent surfaces, and capability profiles

---

## 0. Executive summary

Most TUI “ecosystems” are either:

1. **Immediate-mode widget kits** (Ratatui, FTXUI-ish) — fast, flexible, weak product UX contracts, or  
2. **App frameworks** (Bubble Tea, Textual, Ink) — strong architecture/docs, weaker terminal-native depth (focus law, unicode, virtualization, agent trust), or  
3. **Hero applications** (lazygit, k9s, btop, yazi) — exceptional *product* interaction that is rarely extracted as reusable design systems.

**TermRock’s open gap is not “more widgets.”** It is being the **shadcn of the terminal**: open, owned, contract-tested components with studio-grade evidence, agent-native trust/input, and performance/capability ladders — while staying product-neutral.

---

## 1. Framework & toolkit analysis

### 1.1 Ratatui ecosystem (Rust)

| Lens | Analysis |
|------|----------|
| **Praise** | Performance, control, backend flexibility, community widgets, “no magic runtime” |
| **Unusually good** | Immediate-mode buffer model; low-level correctness culture |
| **Visual** | Depends entirely on the app; no opinionated design system |
| **Architecture** | Widget trait + Buffer; app owns loop/state |
| **Doesn’t generalize** | Each app reimplements focus, modals, themes, agent UX |
| **Extract** | Remain the **render substrate** (`ratatui-core`); don’t reimplement layout kernels |
| **Avoid** | Becoming “another widget dump” on crates.io without contracts |
| **Home** | **Core** (integration only) |

**Gap vs TermRock:** Ratatui is paint. TermRock must own **interaction law + design tokens + quality gate**.

### 1.2 Textual (Python)

| Lens | Analysis |
|------|----------|
| **Praise** | CSS, hot-reload, docs, widget gallery, web export |
| **Unusually good** | Retained-mode DOM + stylesheet iteration; RAD velocity |
| **Visual** | Theme presets, consistent widget chrome |
| **Architecture** | Message pump, reactive attributes, CSS layout |
| **Doesn’t generalize** | Python runtime cost; web metaphors (DOM) can fight cell-grid truth |
| **Extract** | Studio hot-reload *of tokens/stories*; gallery-as-docs; reactive *projections* not full DOM |
| **Avoid** | Full CSS cascade / browser DOM in the terminal |
| **Home** | **Studio** (dev UX), **recipe** (token themes), not core layout |

### 1.3 Bubble Tea + Bubbles + Lip Gloss + Huh (Go / Charm)

| Lens | Analysis |
|------|----------|
| **Praise** | Elm architecture clarity; stylish defaults; composable “bubbles”; forms (Huh); brand coherence |
| **Unusually good** | **Cmd** for effects; pure update; Lip Gloss adaptive styling; “glamour” culture |
| **Visual** | Soft borders, adaptive colors, polished CLI *and* full TUI |
| **Architecture** | TEA (model/update/view); components implement Model interface |
| **Doesn’t generalize** | App-framework lock-in; Go GC; weaker large-table/agent standards |
| **Extract** | Explicit effect boundary (TermRock already: outcomes only); style tokens as “lipgloss for semantic roles”; form flows as blocks |
| **Avoid** | Forcing TEA as the only app shape; Charm-branded look as default identity |
| **Home** | **Core** (outcome purity), **component** (forms/lists), **recipe** (phosphor vs neutral themes) |

### 1.4 Gum + Charm applications (Glow, etc.)

| Lens | Analysis |
|------|----------|
| **Praise** | Scriptable glamorous CLI; Markdown beauty (Glow) |
| **Unusually good** | **Composable shell UX** without writing a full TUI |
| **Doesn’t generalize** | Not a design system for large apps |
| **Extract** | Registry/CLI copy-own path; small “huh-like” interactive primitives as installable blocks |
| **Avoid** | One-off CLI tools as the only distribution story |
| **Home** | **Registry/CLI**, **block**, **Studio** stories |

### 1.5 Ink (TypeScript / React)

| Lens | Analysis |
|------|----------|
| **Praise** | React mental model; Yoga flexbox; testability |
| **Unusually good** | Component composition; Static for append-only logs |
| **Doesn’t generalize** | JS event loop; flexbox ≠ terminal cell physics fully |
| **Extract** | Storybook-like Studio; component docs; composition over inheritance |
| **Avoid** | VDOM reconciliation as the only truth over buffer cells |
| **Home** | **Studio**, **docs**, composition **patterns** |

### 1.6 Blessed / Blessed-contrib (Node)

| Lens | Analysis |
|------|----------|
| **Praise** | Full-screen dashboards; many widgets historically |
| **Unusually good** | Dashboard density demos |
| **Doesn’t generalize** | Aging stack; accessibility/unicode inconsistency |
| **Extract** | Dashboard **block** recipes (ops) |
| **Avoid** | Widget soup without contracts |
| **Home** | **Block** (OpsDashboard) |

### 1.7 Cursive (Rust)

| Lens | Analysis |
|------|----------|
| **Praise** | Retained widgets; event callbacks; higher-level than raw Ratatui |
| **Unusually good** | Traditional UI toolkit feel for forms/dialogs |
| **Doesn’t generalize** | Different paradigm from Ratatui immediate mode; dual ecosystem split |
| **Extract** | Dialog/focus expectations users bring from “real UI toolkits” |
| **Avoid** | Forking retained vs immediate as two TermRock modes |
| **Home** | **Core** focus/dialog law only |

### 1.8 Iced (desktop) / terminal experiments

| Lens | Analysis |
|------|----------|
| **Praise** | Elm-ish pure UI; polish aspirations |
| **Extract** | Message-driven UI purity (aligns with outcomes) |
| **Avoid** | Pixel GUI assumptions in terminal |
| **Home** | Philosophy only |

### 1.9 ImTui / FTXUI (C++)

| Lens | Analysis |
|------|----------|
| **Praise** | ImGui-like / functional C++ TUI; performance |
| **Unusually good** | Immediate-mode familiarity for game/tooling engineers |
| **Doesn’t generalize** | C++ ecosystem isolation; weaker design-system docs |
| **Extract** | Immediate-mode discipline; low-level control |
| **Avoid** | ImGui visual language (dense debug chrome) as product default |
| **Home** | **Core** paint philosophy |

### 1.10 Ncurses / Notcurses ecosystems

| Lens | Analysis |
|------|----------|
| **Praise** | Ubiquity; control |
| **Unusually good** | Lowest common denominator survival |
| **Doesn’t generalize** | Painful modern UX; capability hell |
| **Extract** | Capability ladder humility; graceful degrade |
| **Avoid** | Building *on* ncurses as the public API |
| **Home** | **Capability** architecture |

### 1.11 Prompt-toolkit + Rich (Python)

| Lens | Analysis |
|------|----------|
| **Praise** | Prompt-toolkit: line editing excellence; Rich: pretty output/tables/markdown |
| **Unusually good** | **Editing** (prompt-toolkit); **presentation density** (Rich) |
| **Doesn’t generalize** | Not a full interaction design system for multi-panel agents |
| **Extract** | PromptComposer bar (editing); markdown/table presentation quality |
| **Avoid** | Pretty-print only without focus/overlay law |
| **Home** | **Component** (composer, markdown, tables) |

### 1.12 Zig / other

Sparse mature design systems; apps often roll custom. **Extract:** zero-cost abstraction mindset; **avoid:** NIH without contracts.

---

## 2. Application analysis (product heroes)

### 2.1 Agent TUIs: Claude Code, Codex CLI, OpenCode, Amp, Grok Build–class

| Lens | Analysis |
|------|----------|
| **Praise** | Speed of flow; plan modes; tool visibility; permissions; session resume |
| **Unusually good** | **Autonomy dial**, plan→approve→execute, tool cards, nested agents, queue while busy |
| **Visual** | Dense transcript + south composer; status chips; mode badges |
| **Architecture** | Client/server or event stream; UI is a projection of agent runtime |
| **Doesn’t generalize** | Provider policy, branding, product wire formats |
| **Extract** | `@termrock/agent` surfaces already designed: PromptComposer, PermissionPrompt, MessageThread, TaskRail, Workbench |
| **Avoid** | Embedding models, secrets, or “yolo allow” defaults |
| **Home** | **Component + block** (source-owned agent pack); **core** (overlay, intents, perf) |

### 2.2 lazygit / GitUI

| Lens | Analysis |
|------|----------|
| **Praise** | Multi-panel muscle memory; staging UX; keyboard density |
| **Unusually good** | **Panel focus graph**; contextual key help; high-frequency git ops |
| **Architecture** | Domain-specific state machines |
| **Extract** | Workbench multi-pane focus; status/hint bars; selection chrome |
| **Avoid** | Git domain in core |
| **Home** | **Block** recipes; **core** workspace/focus |

### 2.3 k9s

| Lens | Analysis |
|------|----------|
| **Praise** | Cluster nav speed; resource tables; pulse of live data |
| **Unusually good** | **Live tables + filter + drill-down**; kubectl mental model |
| **Extract** | DataTable/TreeTable + streaming refresh; diagnostics list |
| **Avoid** | k8s types in library |
| **Home** | **Component** (data_view); **block** (ops) |

### 2.4 btop / htop-class

| Lens | Analysis |
|------|----------|
| **Praise** | Beautiful dense metrics; identity/brand of “terminal can look modern” |
| **Unusually good** | **Information density without chaos**; sparklines/graphs |
| **Extract** | Metrics components; palette discipline; reduced-motion |
| **Avoid** | One-off art that breaks colorless/a11y |
| **Home** | **Component** + **recipe** (themes) |

### 2.5 Yazi

| Lens | Analysis |
|------|----------|
| **Praise** | Async I/O speed; previews; keyboard FM |
| **Unusually good** | **Async work + preview pane** without freezing UI |
| **Extract** | Preview host, backpressure, dual-pane file block |
| **Avoid** | File-manager product scope in core |
| **Home** | **Block** + **perf** + **CapabilityPreviewHost** |

### 2.6 Zellij

| Lens | Analysis |
|------|----------|
| **Praise** | Multiplexer UX modernity; layouts; plugins |
| **Unusually good** | **Layout as first-class**; discoverable modes |
| **Extract** | Workspace tree; mode ribbons; capability under mux |
| **Avoid** | Competing as a multiplexer |
| **Home** | **Core** layout; **capability** mux detection |

### 2.7 Glow

| Lens | Analysis |
|------|----------|
| **Praise** | Markdown beauty |
| **Extract** | StreamingMarkdown quality bar |
| **Home** | **Component** |

### 2.8 Posting / HTTP clients (ATAC, etc.)

| Lens | Analysis |
|------|----------|
| **Praise** | Postman-in-terminal; request collections |
| **Unusually good** | Forms + panes + editors for API work |
| **Extract** | Form density; multi-pane request/response block |
| **Home** | **Block** (HTTP client shell), not core |

### 2.9 television / fuzzy finders

| Lens | Analysis |
|------|----------|
| **Praise** | Instant fuzzy pick |
| **Extract** | Picker/CommandPalette quality; viewport virtualization |
| **Home** | **Component** |

### 2.10 visidata

| Lens | Analysis |
|------|----------|
| **Praise** | Sheet operations; data exploration power |
| **Unusually good** | **Cell-native navigation** on large tables |
| **Extract** | DataTable cell selection, copy ranges, load states |
| **Avoid** | Spreadsheet engine in core |
| **Home** | **Component** data_view / DataTable |

### 2.11 Other notables

| App | Extract | Avoid | Home |
|-----|---------|-------|------|
| **lazydocker** | Resource panes + logs | Docker API | Block |
| **bottom/btm** | Metrics layout | — | Component |
| **newsboat/mutt** | Keyboard-first lists | Legacy chrome | Core focus law |
| **tmux** | Capability constraints | Competing | Capability |
| **helix/kakoune** | Modal editing lessons for composers | Full editor | PromptComposer only |

---

## 3. Feature & quality matrix

Legend: ● strong · ◐ partial · ○ weak/absent · — N/A  

| Dimension | Ratatui | Charm stack | Textual | Ink | Cursive | TermRock (target/current) |
|-----------|:---:|:---:|:---:|:---:|:---:|:---:|
| Immediate-mode paint control | ● | ◐ | ○ | ◐ | ○ | ● (on Ratatui) |
| Design tokens / themes | ○ | ● | ● | ◐ | ◐ | ● |
| Focus / modal law | ○ | ◐ | ● | ◐ | ● | ● InteractionScene |
| Overlay stack / Esc-one-layer | ○ | ◐ | ● | ◐ | ● | ● OverlayStack |
| Source-owned install (shadcn-like) | ○ | ○ | ○ | ○ | ○ | ● design/CLI |
| Studio / storybook | ○ | ◐ | ● | ● | ○ | ● Studio design |
| Quality contracts + CI evidence | ○ | ○ | ◐ | ◐ | ○ | ● standard |
| Agent trust surfaces | ○ | ○ | ○ | ○ | ○ | ● PermissionPrompt |
| Agent composer (queue/chips) | ○ | ○ | ○ | ○ | ○ | ● PromptComposer |
| Large virtual tables | ◐ | ◐ | ● | ◐ | ◐ | ● kits + path |
| Streaming / coalesce / budgets | ○ | ○ | ◐ | ◐ | ○ | ● perf |
| Capability profiles + doctor | ○ | ◐ | ◐ | ○ | ○ | ● capability |
| Docs depth (shadcn-class) | ◐ | ● | ● | ● | ◐ | ● handbook |
| Product-neutral mandate | ● | ○ | ● | ● | ● | ● |
| Hero app polish | — | ● (apps) | ● | — | — | via consumers |

**Hero apps** score ● on product-specific interaction but ○ on reusable design-system extraction.

---

## 4. Cross-cutting patterns worth stealing (abstractly)

1. **TEA / pure update** (Bubble Tea, Iced) → outcomes, no effects in widgets.  
2. **CSS/token iteration** (Textual, Lip Gloss) → DesignTokens + Studio live edit.  
3. **Static append regions** (Ink Static) → log/transcript follow + virtual window.  
4. **Panel focus graphs** (lazygit) → Workspace + scene.  
5. **Live resource tables** (k9s) → DataTable stream + filter.  
6. **Density art** (btop) → metrics + phosphor recipe without color-only state.  
7. **Async previews** (yazi) → CapabilityPreviewHost + backpressure.  
8. **Trust dials** (agent TUIs) → mode selector + permission queue.  
9. **Cell-native data** (visidata) → selection model cell/range.  
10. **Capability humility** (ncurses world) → profiles + doctor.

---

## 5. What TermRock must not copy

| Anti-pattern | Source tendency | Why avoid |
|--------------|-----------------|-----------|
| Widget soup without contracts | Blessed, early tui | Unmaintainable quality |
| Framework monopoly (must use TEA/DOM) | Bubble Tea, Textual | Ratatui apps need choice |
| Product domain in core | Agent CLIs, k9s | Neutrality death |
| Color-as-only-state “pretty” | Many glam demos | A11y / mono failure |
| Approve-by-default | Historical agent UIs | Safety |
| Silent truecolor assumption | Many modern apps | Mux/SSH breakage |
| Docs that restate field names | Generated API dumps | No “when/why” |
| Performance theater without budgets | Everywhere | Regressions ship |

---

## 6. Placement taxonomy (idea → home)

| Idea | Core | Component | Block | Recipe | Studio |
|------|:---:|:---:|:---:|:---:|:---:|
| Buffer/widget traits | ● (Ratatui) | | | | |
| Focus, Esc, overlays, intents | ● | | | | |
| Tokens, density, quantize | ● | | | | |
| List/Table/Tree/Composer | | ● | | | |
| Permission/trust | | ● | | | |
| Agent workbench shell | | | ● | | |
| Ops dashboard layout | | | ● | | |
| Phosphor default theme | | | | ● | |
| Story/replay/inspect | | | | | ● |
| Registry copy-own | CLI | ● skins | ● | | ● |

---

## 7. Ten opportunities to **exceed** every competitor

Not parity features — **category-defining** gaps:

### 1. shadcn-class **source-owned** TUI distribution
No Charm/Textual/Ratatui stack owns “install component source + three-way update + lock digests.”  
**Exceed by:** shipping `termrock` CLI + registry with dirty-safe updates (design exists).

### 2. **Contract-complete** components with CI evidence
Web design systems have Storybook + a11y gates; TUI libs mostly don’t.  
**Exceed by:** quality standard v2 + Studio replay + lint errors as ship gates.

### 3. **Agent trust surface** as a reusable library
Agents reinvent Allow/Deny poorly.  
**Exceed by:** provenance chain, risk, egress copy, stale queue, default-deny — already foundational.

### 4. **Composer that survives overlay takeover**
Most agent UIs lose draft or dual-submit under load.  
**Exceed by:** PromptComposer queue + blur-without-clear + completion overlays.

### 5. **Unified overlay law** across every floating UI
Apps reimplement Esc/z-order inconsistently.  
**Exceed by:** OverlayStack + scene as single public authority (shipped direction).

### 6. **Performance budgets as public API**
Apps claim “fast”; few fail CI on alloc/paint.  
**Exceed by:** `termrock::perf` named budgets + hot_path enforcement.

### 7. **Capability profiles + doctor** as first-class
Users debug `TERM` folklore alone.  
**Exceed by:** Modern/Compatible/Minimal/Inline/Headless + doctor text + fallback table for every optional feature.

### 8. **Data presentation kits without mega-traits**
visidata power without product lock-in; k9s tables without k8s.  
**Exceed by:** `data_view` + DataTable path: 1M logical rows, cell selection, contraction priorities.

### 9. **Semantic scene as DevTools for TUI**
Browser has DOM inspectors; terminals have print debugging.  
**Exceed by:** Studio InspectionFrame (focus, hits, layers, messages, buffer digests).

### 10. **Handbook that teaches when/why**
Charm/Textual win on docs; Ratatui wins on control.  
**Exceed by:** handbook standard + compilable examples + ownership tables (started).

---

## 8. Strategic implications for TermRock roadmap

| Priority | Investment |
|----------|------------|
| P0 | Keep kernel interaction + safety (Esc, permission defaults) |
| P0 | Studio + quality contracts enforcement |
| P1 | Registry CLI spike (047) |
| P1 | DataTable D1–D5 on data_view |
| P1 | Agent workbench block on public APIs only |
| P2 | Doctor CLI + capability stories |
| P2 | Finish handbook coverage for all public widgets |
| P3 | Metrics/dashboard blocks (btop-class density as recipe, not core identity) |

**Positioning line:**

> TermRock is the design system and interaction kernel for serious terminal products — agent-native, contract-tested, source-ownable — on Ratatui.

---

## 9. References (non-exhaustive)

- Ratatui / awesome-ratatui  
- Charm: Bubble Tea, Bubbles, Lip Gloss, Huh, Gum, Glow  
- Textualize/Textual  
- Ink (vadimdemedes/ink)  
- Cursive, FTXUI, ImTui, ncurses/notcurses  
- prompt_toolkit, Rich  
- Apps: lazygit, GitUI, k9s, btop, yazi, zellij, posting, television, visidata, ATAC  
- Agent TUIs: Claude Code, Codex CLI, OpenCode, Amp, Grok Build–class experiences  

*(Analysis is architectural/product; no proprietary source reuse.)*

---

## 10. Decision summary

1. Competitors split into **paint kits**, **app frameworks**, and **hero apps**.  
2. TermRock should own the **missing middle**: design system + interaction law + agent/data quality.  
3. Steal **patterns**, never brands or product domains.  
4. Ten exceed-opportunities are mostly **already designed** — winning is **execution and enforcement**, not more research.
