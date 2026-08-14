# Terminal aesthetics landscape — source research (2026-08)

> Companion source doc to [`web-premium-tui-law.md`](web-premium-tui-law.md) and
> [`tui-design-research-2026-08.md`](tui-design-research-2026-08.md). Where those
> documents derive the *web-premium* feel from chat UIs (Kimi, Grok), this one
> extracts signal from two terminal-native sources and maps it to TermRock's
> north star ("the shadcn/ui of TUI").

## Sources researched

| # | Source | Kind | Evidence captured |
|---|--------|------|-------------------|
| S1 | [Noam Lewis (@TheNoamLewis) post, 2026-08-13](https://x.com/TheNoamLewis/status/2087960987529826468) — reply to dax (@thdxr) on opencode2 | Architecture thesis | Text of both posts; two attached screenshots could **not** be machine-analyzed (format error on the media CDN, see Limitations) |
| S2 | [`kud/awesome-terminal-aesthetics`](https://github.com/kud/awesome-terminal-aesthetics) | Curated aesthetic index | Full README: ~60 entries across 8 categories |
| S3 | [opencode.ai/docs/plugins](https://opencode.ai/docs/plugins/) | Plugin architecture docs | Concrete plugin surface: event hooks, tool override, TUI events |
| S4 | [`sinelaw/fresh`](https://github.com/sinelaw/fresh) (Noam Lewis) | Product description | Design bar stated as product copy |

## S1 — "Less core, more plugins": the plugin-as-core model

### What the thread says

dax (@thdxr, SST / opencode) describes an architectural change in opencode2:

> "nearly everything is an internal plugin. there's 68 of them that cover our
> built in agents, integrations, config loading, etc. this means you can disable
> any behavior and we also properly dogfood our plugin apis."

Expanded elsewhere by the same author: *"Models, tools, skills, sessions,
sandboxes, filesystems, loops, orchestration, and UI are ALL implemented as
plugins, and can be mixed, matched, replaced."*

Noam Lewis (author of **Fresh**, S4) endorses the direction and pushes further:

> "Great design. I've been doing that with Fresh too. Would like to have less
> core and more plugins."

### The thesis, distilled

1. **Core is a plugin runtime, not a feature set.** Even built-in capability
   (agents, integrations, config loading, the *UI*) is a plugin. Nothing is
   privileged by being first-party.
2. **Disability is a first-class property.** "You can disable any behavior." A
   feature is only real if a user can turn it off without forking.
3. **Dogfood the public extension surface.** First-party features are built on
   the *same* plugin API third parties use. There is one API, not two.
4. **The UI is a plugin.** Rendering itself participates in the mix/match/
   replace model, not just data and behavior.

### Concrete plugin surface (S3, opencode docs)

The "everything is a plugin" claim is not rhetoric — opencode exposes event
hooks across the whole product, each a replacement/interception point:

- **Lifecycle domains:** command, file, LSP, message, permission, session,
  tool, **TUI**.
- **Tool override:** a plugin tool with a built-in's name *takes precedence*
  over the built-in.
- **Tool interception:** `tool.execute.before` / `tool.execute.after` let a
  plugin modify or block a call (e.g. block `.env` reads).
- **Environment injection:** `shell.env` injects into all shell execution.
- **Compaction replacement:** `experimental.session.compacting` can replace the
  entire compaction prompt.
- **TUI events:** `tui.prompt.append`, `tui.command.execute`, `tui.toast.show`
  — the UI surface is scriptable from the plugin layer.

### Why this matters to TermRock

TermRock is a *component library*, not an end-user product like opencode or
Fresh — so TermRock cannot be "a plugin runtime" literally. But the thesis maps
cleanly onto decisions TermRock's own contributor rules already force, and names
a direction they point toward:

- **CLAUDE.md already states the registry future:** TermRock's distribution unit
  is today the Rust crate, but must "preserve open, inspectable source and
  design APIs that can later support **registry or copy-and-adapt
  distribution**." The opencode/Fresh thesis is the strongest field evidence
  that *every capability being individually addressable, removable, and
  replaceable* is the shape serious terminal products converge on. TermRock's
  building-block/composite boundary is the component-library analogue of
  opencode's plugin boundary: nothing product-shaped is privileged inside
  `widgets`; everything composable lives behind a neutral contract.
- **"Dogfood the public surface" ↔ building-block law.** TermRock's rule that
  `widgets` must not depend on `patterns`, and that `patterns` composes only
  public `widgets`, is the same principle: there is one way to build a surface,
  and the first-party composites use exactly the public blocks a consumer would.
  No hidden internal path.
- **"Disable any behavior" ↔ theming and recipes.** TermRock already makes the
  whole paint layer replaceable via `DesignSystem` + `Role` + recipes. The
  plugin thesis argues for the same disability at the *behavior* and *capability*
  layer (intents, focus authority, overlay policy) — each should be an
  overridable seam, not a baked-in default. This reinforces the
  `web-premium-tui-law.md` stance that interaction kernel contracts are
  seams, not implementations.
- **The UI is a plugin ↔ Ratatui stays the paint engine.** TermRock's stack law
  ("Ratatui first; crossterm as session adapter") mirrors opencode treating UI
  as one replaceable layer among many. The interaction kernel owns contracts;
  the paint engine is swappable in principle, exactly as crossterm is replaceable
  "when a measured better adapter replaces it."

**Actionable implication (research, not a code change):** when TermRock later
introduces a registry/plugin distribution mechanism, the opencode/Fresh evidence
sets the target — *every* widget, recipe, and kernel behavior should be
addressable, removable, and replaceable through the same public surface a
third-party contributor uses. "Less core, more plugins" is the end-state to keep
the current API shape open toward.

## S2 — The aesthetic landscape: what "genuinely beautiful terminal" means in 2026

`awesome-terminal-aesthetics` is an opinionated index — entries "earn their
place through great DX, visual polish, or both," explicitly "not just
functional — *delightful*." That editorial bar is itself the signal: the
terminal community now treats visual polish as a first-class quality axis, the
same axis TermRock's north star is built on.

### The shape of the landscape

| Category | What it rewards | TermRock relevance |
|----------|-----------------|--------------------|
| **TUI frameworks** | Composable widgets, clean architecture, great docs | Ratatui is listed **first** — TermRock is positioned directly on the community's default foundation |
| **Prompt & input** | Beautiful defaults, onboarding "gold standard" (Clack), native-feeling forms (Huh) | Validates TermRock's input primitives (TextInput, PasswordInput, Form) as a primary "wow" surface |
| **Styling & output** | **Declarative styling** — borders, padding, color, alignment (Lip Gloss); token design systems (**shui**: "semantic components, themes, icon sets") | Direct validation of TermRock's recipe + token + Role model. `shui` is a philosophical cousin: a token-based design system for scripts |
| **CLI tools** | Syntax highlighting, icons, color-coded output, full TUI panels (lazygit, gitui, bottom) | The everyday-app benchmark TermRock widgets must look as good as |
| **Multiplexers & shells** | Discoverability + "stunning table output" (Nushell); beautiful defaults (Zellij) | Data-table + workspace/panel quality bar |
| **Emulators** | GPU rendering, ligatures, **"exceptional font rendering"** (Ghostty) | Sets the typography ceiling TermRock's glyph/Unicode choices must survive |
| **Fonts** | Ligatures, icon glyphs (Nerd Fonts), superfamily systems (Monaspace) | TermRock's glyph catalog must be Nerd-Font-safe and ligature-aware |
| **Colour schemes** | Catppuccin, Rosé Pine, Tokyo Night, Kanagawa, Gruvbox, Nord, Dracula, base16 | The palettes the ecosystem *expects* a themable library to ship or map to |
| **Showcases** | VHS, asciinema — "wait, that runs in a terminal?" | The exact wow-moment TermRock targets |

### Extracted design principles (from the index's own framing)

1. **Delightful, not just functional.** The bar is "genuinely beautiful to look
   at." This matches TermRock's "incredible — high-class, high-quality, the
   standard" north star verbatim in spirit.
2. **Declarative styling is the norm.** Lip Gloss ("declarative styling …
   borders, padding, colours, alignment") and `shui` ("token-based design system …
   semantic components, themes, icon sets") show the ecosystem has converged on
   *separate declaration from paint* — exactly TermRock's recipe/DesignSystem
   split. TermRock is on the correct side of history here; the
   `web-premium-tui-law.md` recipes are the same idea taken further.
3. **Themes that travel.** base16, Catppuccin, etc. are valued because they
   "travel across your whole terminal stack." A themable component library that
   *cannot* map to these palettes is fighting the ecosystem. TermRock's phosphor
   default must remain **one theme of many**, fully re-themable — already a
   CLAUDE.md requirement, reinforced here by field evidence.
4. **Typography carries weight.** Nerd Fonts, JetBrains Mono, Fira Code,
   Cascadia Code, Monaspace — icon glyphs and ligatures are assumed present.
   TermRock's glyph ladder and Unicode safety must assume a Nerd-Font-capable
   terminal and degrade gracefully where glyphs are absent (non-color /
   colorless cues, already a TermRock rule).
5. **"Wait, that runs in a terminal?"** is the showcase category's explicit
   reaction. TermRock's goal is to produce that reaction in every widget.

### Palette reality check for phosphor

The index's most-loved palettes are *muted, harmonious, multi-hue* (Catppuccin's
pastels, Rosé Pine's warm pines, Tokyo Night's neon-downtown, Kanagawa's
Hokusai, Gruvbox's warm retro). TermRock's default phosphor theme is
*single-hue, high-saturation green on near-black* — a deliberate, distinctive
choice, not a follower. **This is a feature, not a defect**, provided:

- Phosphor is one of N first-class themes, not the only one (CLAUDE.md already
  mandates full re-themability).
- TermRock ships or can map to the ecosystem palettes above, so a consumer
  adopting Catppuccin or Tokyo Night gets a coherent TermRock look without
  hand-rolling Role mappings.
- The phosphor accent budget (≤2 bright accents/viewport) is respected — the
  muted palettes succeed partly by restraint, and TermRock's accent-budget rule
  is the structural equivalent.

## Cross-surface consistency notes (per contributor rules)

This research surfaces two items that apply across the TermRock surface, not
just one widget:

1. **Palette portability.** If/when TermRock ships additional themes, they
   should include mappings to the S2 ecosystem palettes (Catppuccin, Tokyo
   Night, Gruvbox, Nord, Dracula, base16) so the library "travels" the way the
   index expects themes to. The `Role` enum is the contract that makes this
   cheap; the work is palette data, not architecture.
2. **Every-capability-is-addressable.** The S1 plugin thesis reinforces that
   interaction-kernel contracts (intents, focus, overlay policy, capability
   ladders) should remain *overridable seams*, not baked-in defaults — so a
   future registry/plugin model can replace any one without forking. Any
   current code that hard-wires a kernel behavior should be flagged as closing
   a future seam.

## Limitations

- **S1 images not analyzed.** Both attached screenshots (opencode2's 68-plugin
  list; Fresh's architecture) returned a format/parse error from the vision
  tool against the Twitter media CDN. The *text* of both posts, the expanded
  author statement, and the opencode plugin docs (S3) fully substantiate the
  architecture thesis; only the *visual* layout of the screenshots is
  unverified. No screenshot detail is claimed or inferred in this document.
- **S4 (Fresh) is product copy, not source.** No code-level plugin surface was
  inspected; only the stated design bar ("zero config, familiar keybindings,
  mouse support, IDE-level features, no learning curve") is used.

## Open questions for a future pass

1. Should TermRock define a **formal capability/plugin seam** in the
   interaction kernel now, or wait until a registry distribution is built? (S1
   argues the seam should exist before the distribution mechanism.)
2. Which S2 ecosystem palettes does TermRock commit to shipping as first-class
   themes alongside phosphor? (S2 argues Catppuccin + Tokyo Night + Gruvbox at
   minimum, for ecosystem fit.)
3. Does TermRock's glyph catalog need a **Nerd-Font feature-detection** path
   (prefer icon glyphs when present, fall back to shape-ladder when absent)?
   (S2 fonts category + existing colorless-cue rule argue yes.)

---

*Research only. English only. No code changed. Aligns with TermRock docs-only
design-research convention. Cross-references [`web-premium-tui-law.md`](web-premium-tui-law.md),
[`tui-design-research-2026-08.md`](tui-design-research-2026-08.md),
[`termrock-component-audit-2026-08.md`](termrock-component-audit-2026-08.md).*
