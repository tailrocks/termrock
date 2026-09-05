# TermRock documentation redesign — terminal experience in the browser

> **Archived research snapshot.** This predates the generated catalog and the
> current site shell. Current counts and routes come only from
> `termrock-lookbook inventory --format json` via `docs/scripts/generate-catalog.ts`.

> Research deliverable for the directive: *redesign the docs to be "more terminal
> experience in the browser, modern, stylish, high-contrast, high-quality designer
> aesthetics."* Grounded in (a) a read of the current `docs/` site code,
> (b) [`terminal-website-design-directions-2026-08.md`](terminal-website-design-directions-2026-08.md),
> (c) [`terminal-aesthetics-landscape-2026-08.md`](terminal-aesthetics-landscape-2026-08.md),
> (d) [`web-premium-tui-law.md`](web-premium-tui-law.md).

## The one-sentence thesis

The docs site today is **a generic Fumadocs shell wrapping world-class terminal
islands.** The terminal aesthetic lives only inside `not-prose` components
(`TerminalPreview`, `PatternGallery`); the chrome around them — homepage,
navigation, MDX prose, code blocks — is the neutral docs preset. The redesign
extends the already-excellent terminal language of those islands to the **whole
site**, so the page reads as one terminal-derived *Operator Environment*, not a
docs site with embedded terminals.

## Current-state audit (grounded in code)

### What is already excellent — keep, generalize

| Surface | File | Why it is the gold standard |
|---------|------|----------------------------|
| Live preview host | `docs/src/components/TerminalPreview.tsx` | Ghostty-class canvas: truecolor RGB24 cell paint, vector-stroked box/block glyphs, continuous underlines, integer cell metrics, `imageRendering:pixelated`. Real Rust WASM demos with key/pointer/wheel/paste/resize/tick dispatch, focus ring (`#39ff14`), traffic-light titlebar ("Ghostty · TermRock — {story}"), live status footer (`● live`/`◐ timed`/`○ static`), outcome readout, hover cell-probe, hints, zen, reset, preview↔code toggle, variant switcher, poster fallback. This is direction-3 (ASCII as material) + direction-4 (control room) executed at award-winner quality. |
| Pattern gallery | `docs/src/components/PatternGallery.tsx` | Phosphor poster cards, search filter, grouped sections (Applications / Composites / Layout helpers), `{n} of {m} patterns` count. Control-room language. |
| Content depth | generated catalog | Exact component and pattern pages come from the typed Rust inventory; interaction/runtime/migrations share that authority. |
| Catalog component | `crates/termrock/.../CommandPalette` | "Flagship universal command surface: fuzzy search, groups, recent/contextual, nested pages, args, async gates, history, fullscreen." **The docs site does not dogfood it.** |

### What is generic — the redesign targets

| Surface | File | Current | Problem |
|---------|------|---------|---------|
| Homepage | `docs/src/routes/index.tsx:7-27` | Bare `<HomeLayout>`: centered "TermRock", one tagline, one button. | Zero terminal character. No hero, no live preview, no status bar. The landing page of a terminal-component library looks like a generic SaaS. |
| Root shell | `docs/src/routes/__root.tsx:24-41` | Fumadocs `RootProvider`, search **disabled**, system theme **off**, forced `.dark`. | No custom chrome: no status bar, no telemetry rail, no command-palette hook. The nav is Fumadocs default. |
| Theme tokens | `docs/src/styles/app.css:6-24` | Only **5 custom tokens**; rest is `fumadocs-ui/css/neutral.css` preset. Light tokens (`#2563eb` blue, `#faf8f5` cream) are **dead** (site is dark-locked) and are not even terminal-derived. | No shared design-token layer. Hex is hardcoded inline in every component (see below). |
| Inline hex | `PatternGallery.tsx`, `TerminalPreview.tsx` | `#39ff14`, `#090c09`, `#d8ffd8`, `#91a091`, `#334033` repeated as string literals. | Palette cannot change in one place. Every terminal island reinvents the same 8 colors. |
| Search | archived shell snapshot | Search was disabled. | Historical defect; the current generated catalog owns search documents. |
| Typography | `app.css:1,12` | JetBrains Mono only (mono). Prose = Fumadocs default sans. | No typographic hierarchy between "terminal metadata" register and "prose" register (the mono+grotesk split the website-directions research demands). |
| Light theme | `app.css:6-15` | Blue primary, cream bg — unused, and not terminal. | No coherent second register; if a light mode ever ships it would be a non-terminal blue. |

### The gap, precisely

The terminal identity is a **leak through `not-prose` holes** in a conventional
docs page. Outside those holes (homepage, nav, prose paragraphs, code fences,
footer) the site is indistinguishable from any Fumadocs deployment. The
redesign closes the holes by making the *shell itself* terminal-derived.

## Vision: the docs site is an Operator Environment

Borrowing the composite direction from the website research (Displace
structure + Aino live-ASCII + Terminal Industries maturity + Terminal.shop
authenticity), but specialized for **documentation**:

> A documentation environment that *is* a terminal control room: a live status
> bar, a command surface for navigation, terminal-derived chrome and typography,
> and the real Rust component runtime as the hero — where the page does not
> *describe* TermRock, it *is* a TermRock surface.

This is not slapping a green-on-black skin on Fumadocs. It is promoting the
design language already proven in `TerminalPreview` to the site layer, and
adding the two missing control-room primitives: a **status/telemetry bar** and a
**command palette**.

## Seven redesign directions

### D1 — Unify the visual system under shared tokens

**Principle.** One palette, one type scale, one surface ladder, expressed as
CSS custom properties — exactly the `Role`/recipe discipline TermRock enforces
in Rust, mirrored in the docs CSS.

**Concrete change.**
- Extract every hardcoded hex in `TerminalPreview.tsx` and `PatternGallery.tsx`
  into tokens in `app.css` (`--tr-accent`, `--tr-surface`, `--tr-surface-raised`,
  `--tr-border`, `--tr-border-focus`, `--tr-text`, `--tr-muted`, `--tr-faint`,
  `--tr-warn`, `--tr-danger`). The two components already agree on the palette;
  promote that agreement to the source of truth.
- Adopt the **surface ladder** from the component audit (Canvas → Sunken →
  Surface → Raised → Elevated) in docs CSS so cards, preview chrome, and prose
  containers share depth semantics.
- Map Fumadocs' `--color-fd-*` variables onto the `--tr-*` tokens so the preset
  shell inherits the terminal palette instead of fighting it.

**Why.** Right now the site has *two* design systems bolted together (Fumadocs
neutral + inline terminal hex). Cross-surface consistency (a TermRock
contributor rule) demands one. This is also the prerequisite for every other
direction — none of them land cleanly while the palette is duplicated as string
literals.

### D2 — Homepage becomes the wow moment

**Principle.** The landing page must produce the *"wait, this terminal IS the
site"* reaction — the inverse of the showcase category's *"wait, that runs in a
terminal?"*

**Concrete change.** Replace `index.tsx`'s bare centered block with:
- A **live reactive hero** rendered by the existing WASM paint path (reuse
  `paintCanvas` + the lookbook runtime) — an ASCII/glyph field or an animated
  TermRock surface, not a static gradient or stock illustration. This is
  direction-3 (ASCII as living material) and it is *free* because the runtime
  already exists.
- A **status bar** strip (D4) across the hero: `TermRock · Rust · Ratatui`,
  version/revision, build date, live clock, and generated component/pattern counts.
- A single phosphor CTA ("Browse the catalog") + a `⌘K` hint ("press ⌘K to
  command").
- The live preview of one flagship component (CommandPalette or a workbench)
  mounted and interactive, not a screenshot.

**Why.** The homepage is the single highest-leverage surface for the north-star
"wow moment of clarity, beauty, and power." It is currently the weakest surface.
A live WASM hero is the most authentic possible statement: the library renders
its own landing page.

### D3 — Command palette for site navigation (dogfood CommandPalette)

**Principle.** Direction-2 (command-native) made optional via direction-5
(escape route): typing is an *enhancement*, clicking always works. TermRock
ships `CommandPalette`; the docs should use the concept for navigation.

**Concrete change.**
- A `⌘K` / `Ctrl+K` palette over the whole site: fuzzy over components,
  patterns, doc pages, plus **actions** (copy `cargo` snippet for a component,
  jump to a component's contract, toggle raw view, switch theme).
- Re-enable a visible search affordance in the nav (`searchToggle` is currently
  disabled) wired to the same palette — the palette *is* the search.
- Every palette result is also a clickable nav entry; the palette never replaces
  conventional navigation, it accelerates it.

**Why.** A large generated catalog without search has a
real findability problem. The command palette solves it *and* demonstrates the
flagship widget dogfooding-style. This is the "less core, more plugins /
dogfood the public surface" thesis (plugin research, S1) applied to the docs.

### D4 — Status / telemetry bar as site chrome

**Principle.** Direction-4 (industrial control room): status labels,
coordinates, timestamps, telemetry, operational language, monospaced metadata.

**Concrete change.** A slim persistent bar (top or bottom) carrying:
- Version / git revision / build date (static, from build).
- Current section ("components · command-palette") as location telemetry.
- Live clock (the runtime already has time events; trivial in JS).
- Density/theme indicators + the `⌘K` hint.
- On component pages: the component's contract one-liner as a status readout.

**Why.** This is the single cheapest change that converts "docs site" into
"control room." It also gives every page a consistent terminal-derived frame,
closing the biggest chrome gap. It must stay **restrained** (one row, muted
text, phosphor only for the live/accent elements) — the control-room aesthetic
fails the moment it becomes busy.

### D5 — Typographic hierarchy: mono metadata + grotesk prose

**Principle.** The website-directions research is explicit: monospace
everywhere reads as documentation; the strong pattern combines mono (commands,
metadata, telemetry, labels) + a modern grotesk (nav, body) + a display face
(statements) + oversized numbers/symbols as composition.

**Concrete change.**
- Keep **JetBrains Mono** as the *terminal-metadata register* (status bar,
  component IDs, code, telemetry, labels, the `demo` slug).
- Add a **modern grotesk** (Inter / Geist / similar) for prose body and nav —
  the register Fumadocs already uses, but made deliberate, not default.
- Reserve **oversized ASCII / glyph numerals** for section openers and the hero
  (composition), painted via the WASM path so they are true terminal glyphs.
- Encode the hierarchy as a type-token scale, mirroring the TUI
  typographic-hierarchy principle (brightness/glyph-weight/accent-spend, not
  font family) from the website-directions doc — adapted to the web where font
  family *is* available, so the split is family + weight + color.

**Why.** Today the only loaded face is mono; prose falls back to Fumadocs'
default sans with no stated intent. The result is no hierarchy between
"documentation about a terminal" and "terminal output." The split makes the
terminal register feel like a deliberate material inside a broader identity.

### D6 — Dual register: dark control-room + optional paper-terminal

**Principle.** Two independent sources flag green-on-black as a cliché.
TermRock's defense (phosphor is the deliberate default, accent budget, full
re-themability) holds — but the docs should offer the **escape route**
(direction-5): a coherent second register.

**Concrete change.**
- **Dark control-room** = primary (matches the terminal, matches TermRock's
  identity, matches the current forced `.dark`).
- **Paper-terminal** = optional light register, *not* the current dead blue.
  Use the editorial palettes from the website-directions research: warm
  paper/cream surface, charcoal ink, one restrained accent (a muted green or a
  non-green editorial accent). This is the Lumena "expensive, contemporary,
  not-cyberpunk" register.
- Toggle via the status bar + the command palette + respecting
  `prefers-color-scheme` (currently `enableSystem:false`).

**Why.** The dead `#2563eb` light tokens prove a light register was started and
abandoned because blue is not terminal. A real paper-terminal register lets the
site serve readers who want high-contrast light without breaking the
terminal-derived identity, and it satisfies the "escape route" usability rule.
If the team prefers committing dark-only, that is defensible — but then the dead
light tokens must be removed, not left as a non-terminal trap.

### D7 — Signature interactions that reuse the WASM runtime

**Principle.** The composite direction's "one feature that could only belong to
this project." TermRock's unique asset is the live Rust runtime in the browser —
signature interactions should lean on it, not on CSS tricks.

**Candidates (pick 1–2):**
- **Raw toggle on doc pages** (direction-5): rendered prose ⇄ raw MDX/AST/source
  view, mirroring the per-widget `preview ⇄ code` toggle already in
  `TerminalPreview`.
- **Live install simulation**: a `cargo add` / `cargo run -p termrock-lookbook`
  flow rendered as streaming terminal output via the runtime.
- **ASCII section dividers / live headers**: section openers painted as live
  glyphs (breathing/shimmer via the motion channels), not static images.
- **Component "tear-off"**: from a component page, pop the live demo into a
  fullscreen terminal surface (the existing `zen` mode), making the page
  itself become the terminal.
- **Copy-as-cargo**: command-palette action that emits the exact pinned-rev
  snippet for the viewed component.

**Why.** These are impossible to fake with a static docs generator — only
TermRock has its own runtime in the page. They are the authenticity argument
(Terminal.shop: "at least one genuinely terminal-native action") translated to
docs.

## Information architecture (unchanged content, reframed shell)

The content tree is already correct — do not restructure it. Reframe *how* it is
entered and framed:

```
Operator Environment (site shell: status bar + ⌘K palette + terminal chrome)
├── /                         live WASM hero + status + flagship demo
├── /docs                     index (existing) — framed as "system overview"
├── /docs/components          catalog (generated) — each page = one "process" with
│   └── /:component             live preview + contract + API + recipe
├── /docs/patterns            gallery (35) — each pattern = one "application"
│   └── /patterns/:slug         assembled from building blocks
├── /docs/interaction         scene/overlays/lifecycle — "kernel reference"
├── /docs/quality-migrations  contracts + breaking-change index — "changelog"
└── ⌘K everywhere             fuzzy nav + actions (the control-room command surface)
```

The metaphor (pages = processes, components = processes, patterns =
applications, categories = directories) is the website-directions
"terminal-affects-behavior" mapping applied to docs IA. It is mostly a framing
and labeling change — the routes stay.

## Proposed token + palette starter (dark control-room)

Derived from the colors the two islands already agree on, promoted to tokens:

```
--tr-accent:        #39ff14   /* phosphor — focus, live, ONE cta, restrained */
--tr-accent-soft:   #b4e8b4   /* outcome / positive metadata */
--tr-text:          #d8e8d8   /* primary prose-on-dark */
--tr-muted:         #91a091   /* secondary metadata */
--tr-faint:         #6a7a6a   /* hints, placeholders */
--tr-surface:       #0a0a0a   /* canvas (matches SURFACE_BG) */
--tr-surface-sunken:#050505
--tr-surface-raised:#090c09   /* cards (PatternGallery) */
--tr-surface-elevated:#121512
--tr-border:        #1e261e   /* resting border (preview unfocused) */
--tr-border-soft:   #334033   /* control borders */
--tr-border-focus:  #39ff14   /* = accent; one bright border per viewport */
--tr-warn:          #febc2e   /* traffic-light amber */
--tr-danger:        #ff5f57   /* traffic-light red / error */
```

**Accent discipline (non-negotiable):** phosphor green is an *accent*, not a
fill. The control-room look survives the green-on-black cliché only if green
occupies ≤~5% of pixels — focus rings, the live-status glyph, one CTA, active
labels. The bulk is neutral charcoal. `TerminalPreview` already obeys this; the
rest of the site must too.

## Phasing (what to do first — research recommendation, not a plan to execute)

1. **D1 tokens** — extract hex to `--tr-*`, map Fumadocs vars onto them. Zero
   visual risk, unblocks everything. Remove the dead blue/cream light tokens.
2. **D4 status bar** — cheapest chrome conversion; instant control-room feel.
3. **D3 command palette** — re-enable findability; dogfood the flagship widget.
4. **D2 homepage hero** — the wow moment; reuses WASM runtime.
5. **D5 typography split** — add grotesk, codify registers.
6. **D7 one signature interaction** (raw toggle or tear-off).
7. **D6 paper-terminal light register** — only if dark-only is rejected.

D1→D4→D3 is the highest-leverage sequence and is achievable without touching the
Rust runtime at all.

## Cross-surface implications (contributor rules)

- **Token parity:** the docs `--tr-*` tokens should be the *web projection* of
  the same `Role` semantics TermRock uses in Rust. A future task is generating
  the docs palette from the Rust `DesignTokens`/`Role` source so the site can
  never drift from the library's theme.
- **Dogfooding:** the docs command palette (D3) should be recognizably the same
  interaction model as the Rust `CommandPalette` widget — same grouping, same
  fuzzy feel, same escape semantics — even though one is React and one is
  Ratatui. Cross-surface consistency applies to the docs site too.
- **Accessibility:** the escape-route rule (D6 light register, D3 click-always-
  works, D4 restraint, D7 raw toggle) is not optional decoration — it is the
  usability mitigation every terminal-aesthetic source demands. `prefers-
  reduced-motion` must gate the live hero and ASCII motion (the runtime already
  checks it for timed demos).

## Open questions

1. **Dark-only or dual?** Commit to the dark control-room and delete the dead
   light tokens, or invest in a real paper-terminal light register (D6)?
2. **Homepage hero source:** render the hero through the existing lookbook WASM
   runtime (authentic, heavier), or a lightweight dedicated canvas reusing
   `paintCanvas` with a synthetic frame (lighter, less "live")?
3. **Command palette implementation:** port the Rust `CommandPalette` *behavior*
   to React for visual/interaction parity, or use Fumadocs' search plumbing
   reskinned? The former dogfoods harder; the latter ships faster.
4. **Token generation:** generate docs `--tr-*` from Rust `DesignTokens` at
   build time (single source of truth), or maintain them by hand initially?

## Limitations

- This is a design-research doc, not an implementation. No code changed. Phase
  ordering is a recommendation for the user to accept or reorder.
- Token values above are lifted from the colors the two islands already use;
  final values should be tuned against contrast (WCAG) and the accent-budget
  rule before implementation.
- The website/aesthetic research it builds on is partly ChatGPT-sourced (see
  provenance in `terminal-website-design-directions-2026-08.md`); principles
  are portable, individual project attributions are secondary.

---

*Research only. English only. No code changed. Cross-references
[`terminal-website-design-directions-2026-08.md`](terminal-website-design-directions-2026-08.md),
[`terminal-aesthetics-landscape-2026-08.md`](terminal-aesthetics-landscape-2026-08.md),
[`web-premium-tui-law.md`](web-premium-tui-law.md),
[`termrock-component-audit-2026-08.md`](termrock-component-audit-2026-08.md).*
