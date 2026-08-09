# TermRock experience research — becoming the shadcn of TUI

**Status:** research SoT (living)  
**Date:** 2026-08-09  
**Goal:** TermRock as the shadcn/ui-class design system for console TUI/CLI on Ratatui  
**Policy:** quality over compatibility; breaking changes preferred; think big  
**Method:** TermRock tree audit + agent product analysis + hero-app consensus
([rothgar/awesome-tuis](https://github.com/rothgar/awesome-tuis), awesometui.com) +
cross-language library patterns + design-standard research (Monospace Design TUI et al.)  
**Related:** `competitive-tui-research.md`, `pre-1.0-api-redesign.md`,
`termrock-agent.md`, `termrock-studio.md`, `source-owned-registry.md`,
`shadcn-tui-direction.md`

---

## 0. Thesis

Most TUI ecosystems are one of four things:

| Class | Examples | Strength | Failure mode |
|-------|----------|----------|--------------|
| **Paint kits** | Ratatui, FTXUI | Control, speed | Every app reinvents focus/theme/overlays |
| **App frameworks** | Bubble Tea, Textual, Ink | Architecture, docs | Framework lock-in; weak agent/data law |
| **Hero products** | lazygit, k9s, btop, yazi | Incredible product feel | Interaction never extracted as reusable system |
| **Agent OS products** | Grok Build, Amp, OpenCode, Crush, Claude Code | Trust, sessions, inspectable work | Product-specific chrome; not a design system |

**Nobody owns the missing middle:** a product-neutral **interaction kernel + design
tokens + contract-tested widgets + source-owned blocks + Studio** that makes
“built with X” the same flex as “uses shadcn.”

That middle is TermRock’s category.

**TermRock’s gap is not “more widgets.”** Inventory is already wide (70+ named
surfaces in `COMPONENTS.md`). The gap is:

1. **One authority per concern** (dual stacks still public — pre-1.0 kill)  
2. **Tokens and intents that actually paint and drive all widgets**  
3. **shadcn distribution** (own the source: CLI + registry + blocks)  
4. **Studio gravity** (Storybook + DevTools for terminals)  
5. **Experience packs** that encode Amp/Grok/lazygit grammar as reusable law  

**Win condition:** *“Built with TermRock”* means spatial memory, trust visibility,
inspectable work, composer continuity, screenshot-worthy defaults, and graceful
degrade — as **library law**, not app folklore.

---

## 1. TermRock baseline (truth from the tree)

### 1.1 Strengths already rare in the ecosystem

| Strength | Evidence |
|----------|----------|
| Ownership split | Borrowed paint data + stable IDs; domain/effects stay consumer |
| Lifecycle moat | `Session` alt-screen/raw/mouse/paste/rollback/Drop |
| Interaction direction | `InteractionScene`, `OverlayStack`, intents, keymap |
| Design tokens | Roles, phosphor default, density/motion/glyphs, quantize |
| Agent surfaces | Composer, Permission, Transcript, PlanReview, TaskRail, ToolCard… |
| Catalog culture | Lookbook, SVG gate, contracts, Fumadocs, migrations, hot-path budgets |
| Blocks seed | Registry fixtures: form-wizard, ops-dashboard, resource-browser, settings-shell |
| Patterns seed | agent_shell, agent_workbench, ops_dashboard, resource_browser, studio_shell |

### 1.2 What still prevents “incredible design system feel”

| Finding | Why it hurts premium feel |
|---------|---------------------------|
| Dual authorities (FocusRing + Scene, Theme + DesignSystem, multi-grid, dual composers) | Consumers feel API soup; chrome inconsistent |
| Tokens not fully driving paint | Visual language is aspirational, not enforced |
| Patterns often geometry-first | Blocks not yet shippable product screens |
| Agent pack not one workbench contract | Apps re-glue ToolCard/Permission/Composer |
| Quality gates uneven | Inventory strong; hierarchy/token misuse not always ship-fail |

**Pre-1.0 redesign is correct.** Kill dual truths. No compatibility facades.

### 1.3 Hybrid model (keep)

| Crate (kernel) | Source-owned (registry) |
|----------------|-------------------------|
| Scene, overlays, intents, tokens | Agent chrome skins, brand themes |
| Unicode, scroll math, session, capability | Keymap collections, vim packs |
| Neutral primitives (Panel, List, Tree, Text*, Dialog) | Workbench / ops / settings blocks |
| One DataTable path + presentation models | Product tool cards / timelines |

This *is* the shadcn hybrid: stable foundation + copy-own experience.

---

## 2. What shadcn/ui actually taught (port principles, not React)

| shadcn win | Terminal expression |
|------------|---------------------|
| You **own** the component source | `termrock add panel` → files in *your* tree + digest lock |
| Strong defaults, full override | Phosphor default forever; full DesignSystem retheme |
| Primitives + **blocks** | Kernel widgets + installable screen recipes |
| Storybook / docs as product | Studio + Fumadocs lookbook-true previews |
| AI-friendly inspectable code | Open Rust, contracts JSON, deterministic SVG |
| Ecosystem of registries | TermRock core registry + community packs later |
| Breaking OK pre-1.0 | Already law — execute dual-kill |

**Do not** port JSX, Tailwind, or VDOM.  
**Do** port craft: one beautiful way to build apps without reinventing focus, Esc,
hints, empty states, permissions, virtualization.

Hybrid truth for Rust: **kernel crate stays imported**; **opinionated chrome is
source-installed**. Pure crate-only = version coupling without ownership. Pure
copy-only = no shared interaction law.

---

## 3. Agent-era TUIs (the new quality bar)

### 3.1 Grok Build (primary reference)

Fullscreen, mouse-interactive coding agent TUI.

| Pattern | Why people call it another level | TermRock concept |
|---------|----------------------------------|------------------|
| Transcript as document | Fold/expand, turn nav, raw toggle, sticky folds | Variable-height `Transcript` + scroll policy |
| Dual input modes | Simple (letter→composer) vs Vim (scrollback as text object) | Intent collections + keymap packs |
| Composer never dies | Queue while busy; overlays don’t destroy draft | **Composer continuity contract** |
| Overlay law | Slash, @, plan, permissions peel one layer | `OverlayStack` sole authority |
| Live theme preview | `/theme` stage preview + OS auto + quantize-safe defaults | Studio + ThemePicker |
| Minimal mode | Terminal-native palette for SSH/mux | Capability ladder + mono recipe |
| Plan + subagent + diff | Work is inspectable | PlanReview, TaskRail, DiffView blocks |
| Extensibility surface | Skills, plugins, hooks, MCP, ACP | UI projects agent OS — consumer owns policy |

### 3.2 Amp (ampcode.com) — leader signals

Amp is repeatedly cited as TUI-class agent UX (full TUI CLI, not thin REPL).

| Pattern | Product signal | TermRock concept |
|---------|----------------|------------------|
| **Threads as objects** | Server-backed, resume, share, team visibility | SessionPicker + timeline + projection trait |
| **Modes as dials** | `low` / `medium` / `high` / `ultra` (capability presets) | ModeRibbon always visible; not hidden in settings |
| **Oracle / specialist lanes** | Dedicated reasoning / research personas | Task/agent lane cards (domain-neutral labels) |
| **Command palette** | `Ctrl+O` → mode and actions | CommandPalette + discoverable scene actions |
| **MCP/tools as panels** | Setup is UX, not YAML folklore | Settings form/tree blocks |
| **Headless twin** | Streaming JSON / `-x` / pipes | Same projections for TUI and CI |
| **Plugin UI hooks** | Plugins register modes/commands/tools | Extension points in blocks only |
| Ghostty-class fidelity | Truecolor modern VT assumed | Capability doctor; enhance don’t require |

**Users convert from IDE to TUI when the TUI feels like an app** (spatial chrome,
modes, threads, not a chat log with ANSI).

### 3.3 OpenCode, Crush, Claude Code, Codex

| Product | DNA | Steal | Avoid |
|---------|-----|-------|-------|
| OpenCode | Plan vs Build; multi-session; client/server survives SSH | Plan chrome; session rail; reconnect | Provider chrome |
| Crush | Charm glamour; screenshot magnet | Token recipes worth photographing | Pretty over trust |
| Claude Code | Dense tools; permission interrupts | Tool card density; interrupt UX | Brand lock-in |
| Codex-class | Autonomy ladder | Visible dial | Allow-by-default |

### 3.4 Agent UX consensus (non-negotiable laws)

1. **Composer continuity** — draft/queue survive overlays and streaming  
2. **Trust visibility** — mode, risk, pending permission always readable  
3. **Inspectable work** — tools, diffs, plans, subagents are first-class rows  
4. **Durable session** — resume/share/multi is UI, not afterthought  
5. **Modern terminal is enhancement** — mono/narrow/mux still usable  
6. **Default-deny trust** — high risk never Enter-approves by accident  

---

## 4. Hero apps people agree are “rare and amazing”

Sources: [rothgar/awesome-tuis](https://github.com/rothgar/awesome-tuis),
[awesometui.com](https://awesometui.com), terminal-apps.dev, community roundups.

There is no formal “TUI Oscar.” The award is **unprompted recommendation +
screenshots that make people install**.

### 4.1 Cluster DNA

| Cluster | Exemplars | Interaction DNA | TermRock extract |
|---------|-----------|-----------------|------------------|
| Multipane muscle memory | lazygit, GitUI, lazydocker | Fixed regions, one focus, contextual keys | WorkSurface, HintBar, focus graph |
| Live ops | k9s, dtop, ctop | Stream + filter + drill | DataTable + log dual-pane blocks |
| Density art | btop, bottom, nvtop | Every cell earns keep; graphs as language | Sparkline, BarSeries, MetricTile, density tokens |
| Async FM | yazi, superfile | Never-block; preview; media | PreviewHost, ImageSurface, Skeleton |
| IDE-in-terminal | posting, harlequin, rainfrog, ATAC | Jump mode, $EDITOR, syntax, collections | JumpOverlay, CodeBlock, Form density |
| Single-job glamour | fzf, gum, huh, glow | Zero waste; instant loop | Shared filter protocol; theme photography |
| Mux modernity | zellij | Layout-as-product; discoverable modes | Responsive workspace + ModeRibbon |
| Data sheets | visidata | Cell-native huge tables | Cell/range selection on one DataTable |
| GitHub flow | gh-dash | PR/issue without browser | Review list + detail split |
| Markdown beauty | glow, frogmouth | Readable dense prose | MarkdownView quality bar |

### 4.2 Seven laws of incredible TUI feel

1. **Spatial memory** — layout stable  
2. **One keyboard owner** — Esc/focus never ambiguous  
3. **Hints at point of action**  
4. **Speed as honesty** — async + virtualize  
5. **Screenshot identity** — default theme worth sharing  
6. **Degrade with dignity** — mono / NO_COLOR / narrow  
7. **Power under chrome** — activity trail / raw escape hatch  

### 4.3 Cross-standard insight (Monospace Design TUI)

External design standards (e.g. Monospace Design TUI) confirm what hero apps
already practice: **shared keyboard conventions, focus rules, master-detail
archetypes, footer command bars, selection grammar**. TermRock should encode
these as **falsifiable contracts** (Studio lint / CI), not blog posts.

Archetypes to own as blocks:

| Archetype | Screen shape |
|-----------|--------------|
| Master–detail | List/tree west + detail east |
| Dual-pane | Two equal work surfaces (FM, diff) |
| Transcript + composer | Agent shell |
| Ops dashboard | Metrics + tables + log |
| Wizard / form flow | Linear steps + validation |
| Palette / jump | Ephemeral overlay over any surface |

---

## 5. Cross-language libraries — port map

| Stack | Language | Steal for TermRock | Reject |
|-------|----------|--------------------|--------|
| Bubble Tea + Bubbles + Lip Gloss + Huh | Go | Outcome purity; form kits; adaptive style | Framework monopoly |
| Gum | Go | Scriptable primitives → CLI path | One-shot only |
| Textual | Python | Gallery, hot-reload, docs gravity | Full CSS DOM |
| Ink | TS | Composition, Storybook culture, Static logs | VDOM as cell truth |
| prompt_toolkit + Rich | Python | Edit excellence; density presentation | Pretty without focus law |
| Cursive | Rust | Dialog toolkit expectations | Dual retained ecosystem |
| FTXUI / ImTui | C++ | Immediate-mode discipline | ImGui aesthetic as product |
| notcurses | C | Media/capability ambition | Low-level public API |
| nocterm (Flutter-like Dart) | Dart | Hot reload + component catalog culture | Non-Rust runtime |
| iocraft / tui-realm / yeehaw | Rust | Declarative / batteries lessons | Competing paint model |
| tachyonfx / ratatui-image / tui-textarea | Rust | Effects, images, rich editors | Uncontracted soup |
| php-tui / TUI4J / Consolonia | other | Cross-ecosystem component catalogs | Language lock-in |

**Port rule:** ideas become tokens, intents, blocks, Studio evidence — not a second
layout engine on Ratatui.

---

## 6. Concept catalog — improve TermRock with these ideas

### 6.1 Kernel concepts (crate)

| Concept | Description | Inspired by |
|---------|-------------|-------------|
| **Single InteractionScene** | Sole focus/hit/layer/Esc/action authority | Grok overlays, posting jump |
| **DesignSystem paint law** | All chrome through roles + density + motion + glyphs | Lip Gloss, Textual tokens |
| **UiIntent everywhere** | No raw KeyCode in public widget handlers | Bubble Tea messages |
| **OverlayStack law** | One peel per Esc; non-dismissible trap | Agent permissions |
| **Responsive algebra** | ViewportClass + ContractionStage everywhere | zellij, TermRock responsive design |
| **Perf budgets public** | Named budgets CI-fail | TermRock hot paths |
| **Capability doctor** | Modern/Compatible/Minimal/Inline/Headless | Grok minimal, ncurses humility |
| **Composer continuity** | Tested guarantee: draft+queue survive takeover | Amp, Grok |
| **Permission provenance** | Risk + egress + stale + default-deny | Agent trust research |
| **DataTable one path** | 1M logical rows, cell/range, stream coalesce | visidata, k9s |

### 6.2 Component concepts (crate or thin wrappers)

| Concept | Job |
|---------|-----|
| **ComposedRow anatomy** | Leading / main / meta / trailing slots — not label-only rows |
| **ModeRibbon** | Always-visible autonomy/mode dial |
| **TokenMeter** | Context window as honest meter (not fake “thinking”) |
| **ToolCard** | Collapsible invocation + status + result |
| **ThinkingBlock** | Foldable reasoning; never color-only state |
| **QuestionFlow** | Multi-step agent questions as overlay |
| **PlanReview** | Read-only plan → approve → execute layers |
| **TaskRail** | Background/subagent list with cancel |
| **SessionPicker** | Thread/session as navigable object |
| **JumpOverlay** | Letter targets on dense UIs |
| **ActivityTrail** | “What did the UI just do” strip |
| **PreviewHost** | Async content provider for east pane |
| **Skeleton / LoadingView** | Async honesty |
| **EmptyState / ErrorView / Callout** | Non-color status language |
| **MetricTile / Sparkline / BarSeries / Heatmap** | Density art kit |
| **Drawer / Popover** | Overlay family consistency |

### 6.3 Block concepts (source-owned registry)

| Block | Screen | Inspired by |
|-------|--------|-------------|
| **agent-workbench** | Transcript + composer + rail + overlays | Grok, Amp, OpenCode |
| **agent-shell** | Minimal south-composer agent | Claude Code density |
| **ops-dashboard** | Metrics + tables + log | btop + k9s hybrid |
| **resource-browser** | Tree/table + detail + actions | k9s, lazydocker |
| **settings-shell** | Nav tree + form + apply | Amp MCP panels |
| **form-wizard** | Multi-step flow | Huh, posting |
| **master-detail** | Generic list+detail | Monospace archetype |
| **dual-pane-preview** | FM-class west list + east preview | yazi, superfile |
| **review-diff** | Hunk nav + accept/reject | Grok plan/diff |
| **studio-shell** | Library + stage + inspector | Storybook |
| **palette-host** | Global command palette shell | Amp Ctrl+O, Grok / |

### 6.4 Studio concepts

| Concept | Why |
|---------|-----|
| Live token edit + quantize matrix | Textual RAD without CSS |
| Scene inspector (focus/hits/layers) | Terminal DevTools |
| Contract PASS/FAIL per component | shadcn quality + a11y gates |
| Record/replay frames | CI twin of headless |
| Theme photography export | Marketing + handbook truth |
| Capability simulation knobs | Ghostty → dumb mux proof |

### 6.5 Distribution concepts (shadcn path)

| Concept | Why |
|---------|-----|
| `termrock add <block>` | Own the source |
| Lock digests + dirty-safe update | Provenance without npm chaos |
| Three-way merge on upstream update | Real ownership |
| Kernel version peer constraint | Blocks compile against public API only |
| Community registry later | Ecosystem gravity |

---

## 7. Experience language — visual & interaction rules to enforce

These should become **contracts**, not taste:

1. **Focus = theme role, never double-line borders** (Agents.md law)  
2. **Color is never the only status channel** (glyphs, text, position)  
3. **One focused interactive container** (BorderFocused vs Border)  
4. **Hints near actions; global chord map secondary**  
5. **Destructive defaults: safest visible option**  
6. **Narrow contraction is designed, not clipped chaos**  
7. **Unicode width correctness or it is a bug**  
8. **Motion respects reduced-motion**  
9. **Empty/loading/error are first-class states, not missing paint**  
10. **Every public widget: story + contract + SVG + handbook when/why**  

Phosphor remains the **loved default identity**. Neutrality means others can
retheme fully — not that defaults are bland.

---

## 8. Think-big roadmap (breaking preferred)

### Phase A — Authority (P0)

Execute `pre-1.0-api-redesign.md`:

- Scene-only focus/hits/Esc  
- DesignSystem-only paint  
- One composer, one permission, one DataTable path  
- Patterns demoted to registry blocks  
- Public API shrink; no dual shims  

### Phase B — Distribution + Studio (P0/P1)

- CLI install path real for all flagship blocks  
- Studio inspector v1 (scene + tokens + contracts)  
- Lookbook-true docs for every public surface  

### Phase C — Agent OS pack (P1)

- Workbench as installable block with ModeRibbon + SessionPicker + TaskRail  
- Composer continuity + permission provenance as **named CI contracts**  
- PlanReview + QuestionFlow + ToolCard density pass Amp/Grok feel tests  

### Phase D — Data + density + media (P1/P2)

- DataTable cell/range + stream coalesce  
- Metrics kit (btop-class recipes)  
- PreviewHost + ImageSurface capability ladder  
- JumpOverlay + ActivityTrail  

### Phase E — Ecosystem gravity (P2)

- Theme photography set (5 themes × quantize × mono × narrow)  
- External “built with TermRock” dogfood apps  
- Community block registry design  
- Optional DevTools attach protocol on InteractionScene  

### Explicit non-goals

- Terminal emulator / multiplexer product  
- Models, secrets, provider SDKs in crate  
- Compatibility facades for inferior dual APIs  
- Full CSS layout engine  
- Cloning Amp/Claude/Grok chrome/brands  

---

## 9. Positioning

> **TermRock is the design system and interaction kernel for serious terminal
> products — agent-native, contract-tested, source-ownable — on Ratatui.**

| Competitor class | They own | TermRock owns |
|------------------|----------|---------------|
| Ratatui | Paint | Interaction law + design system on top |
| Charm | Glamour culture (Go) | Neutral, contract-tested Rust system |
| Textual | RAD docs (Python) | Studio + kernel without DOM tax |
| Amp / Grok Build | Product agent UX | Reusable agent/data grammar as blocks |
| Hero apps | Domain excellence | Extracted archetypes, not domains |

---

## 10. Decision summary

1. White space is real: paint kits / frameworks / heroes / agent products — no
   neutral design system in the middle.  
2. TermRock inventory is already broad; **integration, authority uniqueness,
   distribution, and experience packs** unlock “incredible.”  
3. Steal **patterns** from Grok Build, Amp, lazygit, k9s, btop, yazi, posting,
   Charm, Textual — never brands or product domains.  
4. shadcn for TUI = **kernel crate + source-owned blocks + Studio evidence**.  
5. Pre-1.0 dual-kill is the critical path; research is not the bottleneck.  
6. Think bigger than widgets: own **trust, continuity, sessions, density, and
   distribution**.

---

## 11. References (non-exhaustive)

- TermRock: `COMPONENTS.md`, `pre-1.0-api-redesign.md`, `termrock-agent.md`,
  `termrock-studio.md`, `source-owned-registry.md`, plans 039–053  
- [rothgar/awesome-tuis](https://github.com/rothgar/awesome-tuis),
  [awesometui.com](https://awesometui.com)  
- [shadcn/ui](https://ui.shadcn.com) distribution model  
- Grok Build (xAI): fullscreen TUI, themes, plan, subagents, vim/simple modes  
- Amp (ampcode.com): threads, modes, Oracle, command palette, full TUI CLI  
- OpenCode, Crush, Claude Code, Codex CLI  
- lazygit, k9s, btop, yazi, superfile, posting, harlequin, gh-dash, zellij, visidata  
- Charm stack, Textual, Ink, Cursive, FTXUI, notcurses, prompt_toolkit/Rich  
- Monospace Design TUI (design standard / pattern library for terminal apps)  
- Ratatui ecosystem widgets (image, textarea, tachyonfx)

*(Architectural and product analysis only. No proprietary source reuse.)*
