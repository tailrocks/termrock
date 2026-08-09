# TermRock architecture foundation

**Status:** binding product direction (pre-stable)  
**Date:** 2026-08-09  
**Related:** [shadcn-tui-direction.md](./shadcn-tui-direction.md), migrations `0029`–`0031`

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
3. **Overlay stack:** `OverlayHost` + `EscCascade` peel one layer; focus scopes restore via `FocusRing`.
4. **Semantic scene:** `SemanticScene` registers id + rect + role for hit/focus discovery without replacing Ratatui.
5. **Ownership:** domain state, effects, secrets, and process policy stay application-owned.

## Forward-only API

TermRock is pre-stable. Breaking redesigns are preferred over compatibility facades. Every public break records a sequential file under `migrations/` and an index row in `MIGRATING.md`.

## Non-goals of this foundation slice

Full registry CLI (`termrock add`), multi-crate split, complete agent product pack, Workbench app, Windows/ConPTY, RTL/BiDi, and rewriting every widget onto intents—those follow once the kernel contracts below are stable.
