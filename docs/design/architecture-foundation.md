# TermRock architecture foundation

**Status:** binding product direction (pre-stable)  
**Date:** 2026-08-09 (north-star stack law reinforced 2026-08-10)  
**Contributor law:** root [`AGENTS.md`](../../AGENTS.md) — *North star*  
**Related:** [shadcn-tui-direction.md](./shadcn-tui-direction.md),
[terminal-design-system.md](./terminal-design-system.md),
[product-audit.md](./product-audit.md), migrations `0029`–`0031`

## North star

TermRock is the **de facto base layer for modern Rust terminal applications**:
simple defaults, advanced power, modern APIs, shadcn-class ownership and quality
**on Ratatui**. The emotional bar is intentional: a developer who builds a new
TUI should think *this is the best foundation I have used*. Breaking redesigns
are free; excellence is not. Full contributor statement: **AGENTS.md → North
star**.

### Stack law

| Layer | Role |
|-------|------|
| **Ratatui** | Mandatory paint engine (`Buffer`, `Frame`, layout cells, widgets). TermRock never replaces Ratatui with a retained UI DOM. |
| **crossterm** (feature) | Preferred session / backend / event adapter while it remains the best Ratatui-ecosystem choice. Kernel events stay backend-neutral. |
| **TermRock kernel** | Design system, intents, focus, overlays, semantic scene, capabilities — product-grade contracts on top of paint. |

## Category definition

TermRock is **not** only a crate of Ratatui widgets. The product category is:

> **A hybrid terminal design system:** a **stable interaction kernel** (compiled crate) + **source-ownable components/blocks** (registry/CLI direction) + **design tokens and recipes** for high-quality TUI applications.

| Layer | Distribution | Owns |
|-------|--------------|------|
| **Kernel** | Rust crate (`termrock`) | Session lifecycle, focus, hit geometry, scroll, Unicode safety, overlay stack, semantic intents, per-frame scene registration, design tokens |
| **Components** | Crate today; source-copy registry later | Styled widgets with stable IDs and borrowed data |
| **Blocks / patterns** | Crate recipes today; installable sources later | Agent shell, ops dashboard, resource browser layouts |
| **Studio** | `termrock-lookbook` | Stories, contracts, SVG previews; path to component studio |

shadcn/ui’s defining advantage is **owned, inspectable source**—not React buttons. TermRock aims at the same for the terminal: developers pin the kernel, own application chrome, and can later install blocks without losing interaction contracts.

### Product line

> **Beautiful, inspectable terminal components you own.**

## Progressive capability reduction

**Modern-first, not modern-only.** Quality means progressive enhancement:

| Capability | Behavior |
|------------|----------|
| Truecolor | Full RGB themes |
| 256-color | `Theme::quantized(ColorCapability::Indexed256)` |
| 16-color ANSI | `Theme::quantized(ColorCapability::Ansi16)` |
| `NO_COLOR` / monochrome | `ColorCapability::Monochrome` + non-color glyphs (markers, underlines) |
| ASCII glyph fallback | `GlyphSet::Ascii` vs Unicode |
| Reduced motion | `Motion::Reduced` / `Motion::Off` |

Detection helpers (`ColorCapability::detect_from_env`, `Appearance::detect`) are best-effort. Applications may override. Ghostty-class truecolor remains the **design** baseline for previews; runtime must not assume it.

This supersedes the README line that claimed “no reduced-color or `NO_COLOR` degradation path.” That path is intentional infrastructure as of migration `0031`.

## Interaction kernel contracts

1. **Immediate mode:** per-frame registration; no retained widget DOM.
2. **Semantic intents:** widgets consume `UiIntent` where practical; raw keys map via `default_list_intents` / application keymaps.
3. **Overlay stack:** `OverlayStack` peels one layer; focus traps restore openers via `FocusGraph` (not public `FocusRing`).
4. **Semantic scene:** `SemanticScene` rebuilds a parented tree each frame (id, parent, role, label, description, state, actions, rect, focusable/disabled) for hit discovery, help, jump, Studio snapshots, and AI-readable UI — without replacing Ratatui or owning focus (`InteractionScene` remains sole input layer authority). See `semantic-scene.md` / migration `0079`.
5. **Focus graph:** `FocusGraph` is the sole public focus-graph authority (tab, spatial, zones, traps, history, roving). Collection **selection** stays widget-local. See `focus-graph.md` / migration `0081`.
6. **Ownership:** domain state, effects, secrets, and process policy stay application-owned.

## Forward-only API

TermRock is pre-stable. Breaking redesigns are preferred over compatibility facades. Every public break records a sequential file under `migrations/` and an index row in `MIGRATING.md`.

## Non-goals of this foundation slice

Full registry CLI (`termrock add`), multi-crate split, complete agent product pack, Workbench app, Windows/ConPTY, RTL/BiDi, and rewriting every widget onto intents—those follow once the kernel contracts below are stable.
