# Docs Site: Terminal Experience Redesign Plan

Status: approved direction, pending implementation
Scope: `docs/` site (fumadocs + TanStack Start), `crates/termrock-lookbook` (preview
export/host), new WASM preview runtime. No public `termrock` crate API change is
required by this plan; lookbook and docs tooling changes are internal.

## 1. North star for the documentation site

**The docs are not a website about a terminal library. The docs are a terminal.**

Opening the site should produce the same wow moment the library promises: you are
looking at a real, live, phosphor-on-obsidian terminal experience rendered in the
browser — and every component preview is the real Rust widget running, not a
picture of it. shadcn/ui is the structural reference (preview center-stage, code
one tab away, copy-paste-first); the visual and interaction language is TermRock's
own phosphor design system, applied to the site chrome itself.

Three commitments, in priority order:

1. **Live, truly interactive previews.** Real `termrock` code compiled to WASM,
   receiving real keyboard/mouse events, rendering every frame in the browser.
   No baked frame packs, no step slideshows.
2. **Ghostty-grade realism.** What you see in the preview window is what Ghostty
   renders: same VT semantics, same font, same cell metrics, same cursor
   behavior, faithful macOS window chrome around it.
3. **Terminal-native site design.** Navigation, typography, layout, and motion
   all speak terminal: command-palette navigation, monospace grid, tmux-class
   status chrome, boot-sequence landing, keyboard-first everything.

## 2. Current state audit (what is wrong today)

### 2.1 Preview pipeline: pseudo-interactive frame packs

- `termrock-lookbook export-frames` bakes every story into JSON cell dumps under
  `docs/public/preview-frames/` — per story × per size (`28x6`…`80x24`) × per
  step. The directory is currently **477 MB** of static JSON.
- `docs/src/components/TerminalPreview.tsx` paints those cells on a canvas and
  maps keys/wheel/click to **step scrubbing through pre-baked frames**. It looks
  interactive but is a slideshow: you cannot type into a `TextInput`, drive a
  `CommandPalette` query, or take any path the exporter did not record.
- The lookbook already has everything needed for real interactivity: stories
  implement `render(frame, area)`, `handle_key(KeyEvent)`,
  `handle_mouse(MouseEvent, area)`, knobs, and theme switching
  (`crates/termrock-lookbook/src/interactors.rs`, `stories.rs`). The baked-frame
  layer throws that away at the docs boundary.

### 2.2 Realism gap vs Ghostty

- Cell size is hardcoded `9×18 px` instead of being derived from real font
  metrics; glyph advance mismatches show up at some zoom/DPR combinations.
- Canvas painter imitates a terminal per-glyph: no text shaping, ligatures
  force-disabled, approximated box-glyph strokes, custom underline/cursor logic.
  It is a good imitation ("Ghostty-class"), but it drifts from what Ghostty
  actually renders — which users then see when they run the same widget locally.
- Window chrome is a hand-styled div with traffic lights; padding, titlebar
  typography, focus states, and shadows do not match a real Ghostty macOS window.
- No real cursor pipeline (cursor is inferred from frame contents), no
  selection, no scrollback, no real resize (resizing swaps between 5 baked size
  packs instead of reflowing at arbitrary cols×rows).

### 2.3 Site design: default docs-framework look

- fumadocs default UI: web sidebar, web typography, web search. Nothing about
  the chrome says "terminal" except the preview cards.
- Component pages are generated (`docs/scripts/gen-component-pages.ts`) with a
  reasonable order (preview → usage → contract → stories) but the preview is not
  center-stage the way shadcn/ui makes it, secondary stories are a table instead
  of explorable states, and knobs/themes are not exposed at all.
- Application patterns (`patterns/`, 36 composites) share one page; there is no
  one-page-per-application, full-screen, live experience.

## 3. Site design concept: "the docs are a terminal"

### 3.1 Design language

- **Tokens:** the site consumes TermRock's own phosphor-obsidian palette
  (`docs/design/phosphor-obsidian-visual-direction.md`) as CSS custom
  properties, generated from the Rust `DesignSystem` source of truth so docs
  chrome and previews can never drift apart. Semantic roles map 1:1
  (`Role::BorderFocused` → focused UI chrome on the site too).
- **Typography:** monospace-first. JetBrains Mono (Ghostty's default font) for
  all headings, navigation, labels, and code; a character-grid layout discipline
  in the spirit of [the-monospace-web](https://github.com/owickstrom/the-monospace-web).
  Body prose may use a humanist mono or stay mono — decide with an A/B of
  reading comfort on the longest handbook page.
- **Structure chrome:** box-drawing characters and single-line borders for
  section frames, `$`-prompt headers, `ls`-style component index, `man`-page
  conventions for API reference sections (NAME / SYNOPSIS / OPTIONS rhythm).
- **Motion:** typewriter/boot animations only where they carry meaning; all
  motion behind `prefers-reduced-motion`. Subtle CRT effects (scanline, glow,
  slight bloom on phosphor accents) as a **toggleable** "CRT" mode in the status
  bar, default off for readability, remembered per visitor.
- **Themes:** phosphor dark is the identity default. A "paper terminal" light
  theme (amber-on-paper or ink-on-paper) proves the neutrality claim, and the
  theme switch is itself a TermRock `ThemePicker` story running live.
- Design references to study, not copy: [SRCL / Sacred Computer](https://sacred.computer)
  (terminal aesthetics as a complete web component system),
  [terminal.shop](https://terminal.shop) (commerce inside a terminal — commitment
  to the bit), [charm.sh](https://charm.sh) (playful CLI branding),
  [ghostty.org](https://ghostty.org) (restrained, credible terminal-product
  design), Berkeley Mono / US Graphics visual language.

### 3.2 Navigation and interaction

- **Command palette is the primary navigation.** `Ctrl+K` / `/` opens an
  fzf-style fuzzy finder over components, patterns, handbook pages, and design
  docs — and it *is* the TermRock `CommandPalette` widget running live in WASM.
  The site navigates itself with the library. This is the single highest-wow,
  highest-honesty feature: search results, keyboard behavior, and paint all come
  from the crate being documented.
- **Keyboard-first:** `j`/`k` scroll, `g g`/`G` top/bottom, `[`/`]`
  previous/next page, `?` opens a keyboard-help overlay (the TermRock
  `KeyboardHelp` widget, live). Mouse everything still works.
- **Breadcrumbs as shell paths:** `~/docs/components/panel` with a blinking
  block cursor at the end of the active segment.
- **tmux-class status bar** fixed at the viewport bottom: current section,
  theme, CRT toggle, palette hint, build revision. Doubles as the home of site
  settings.
- **Landing page = boot sequence + live hero.** A short motd-style boot
  (`termrock v0.11.0 · ratatui kernel · phosphor theme loaded`) that resolves
  into a full-width Ghostty window running a real TermRock application
  (the lookbook shell or a purpose-built showcase app) that visitors can
  immediately drive with the keyboard. Skippable, cached after first visit,
  static poster under reduced-motion.

## 4. Preview architecture: live WASM, Ghostty-faithful

### 4.1 Rendering ladder (decision)

Two live paths were evaluated; ship **Path A as the product experience** with
**Path B retained as the deterministic test/SSR path**:

- **Path A — true terminal pipeline via `ghostty-web` (primary).**
  [`ghostty-web`](https://github.com/coder/ghostty-web) compiles Ghostty's own
  VT core ([libghostty-vt](https://mitchellh.com/writing/libghostty-is-coming))
  to a ~400 KB WASM module with an xterm.js-compatible API (MIT, pre-1.0,
  actively developed; also distributed as [`ghostty-web` on npm](https://www.npmjs.com/package/ghostty-web)).
  The story host compiles to `wasm32-unknown-unknown` with a Ratatui backend
  that emits ANSI (truecolor SGR, cursor addressing — the same byte stream the
  crossterm backend produces) into `term.write()`; browser key/mouse events flow
  back through `term.onData()`/DOM listeners into `termrock::input::Event`.
  Realism is inherited, not imitated: the exact VT parser that powers Ghostty
  interprets our output. Grapheme clusters, SGR edge cases, cursor semantics —
  all real.
- **Path B — cell-direct canvas (secondary).** The story host exports the
  Ratatui `Buffer` cells straight to the existing TypeScript painter
  (`TerminalPreview.tsx` already implements Ghostty-class cell paint). Kept
  because it is fully deterministic (pixel-stable for CI screenshots and the
  SSR poster frames) and it de-risks `ghostty-web`'s pre-1.0 status. Same WASM
  module, two frame sinks.
- **Prior art:** [Ratzilla](https://github.com/ratatui/ratzilla) proves
  Ratatui-in-the-browser with DOM/Canvas/WebGL2 backends. Evaluate reusing its
  event plumbing; its renderers are not Ghostty-faithful, so it is a reference,
  not the shipping path. Version-check against our pinned `ratatui-core 0.1.2`.

### 4.2 New crate: `termrock-lookbook-web`

- `crates/termrock-lookbook-web`: `wasm32-unknown-unknown` cdylib wrapping the
  existing story registry and interactor trait. Exports (via `wasm-bindgen`):
  `list_stories()`, `mount(story_id, cols, rows)`, `key(event)`,
  `mouse(event)`, `resize(cols, rows)`, `set_theme(name)`,
  `set_knob(id, value)`, `frame_ansi() -> Vec<u8>` (Path A) and
  `frame_cells() -> JsValue` (Path B).
- The `crossterm` feature stays off for wasm; input events are constructed
  directly as backend-neutral `termrock::input` types (the kernel's
  backend-neutral event vocabulary pays off here — no adapter needed).
- **Real resize:** dragging the Ghostty window's resize handle recomputes
  cols×rows from real cell metrics and calls `resize()`; the widget reflows
  live at arbitrary sizes. Delete the 5-size pack system.
- **Binary size budget:** one module containing all stories is acceptable if
  gzipped wasm ≤ ~2.5 MB after `wasm-opt -Oz` + `lto = "fat"` + panic=abort;
  otherwise split into per-category modules (widgets-core, data, overlays,
  patterns) loaded lazily per page. Measure first, split only on evidence.
- Determinism: stories must not read wall-clock time directly; frame-clock
  ticks come from the host (`requestAnimationFrame` → `tick(ms)` export), so
  the same story remains reproducible in tests.

### 4.3 The Ghostty window component

One React component, `<GhosttyWindow>`, wraps every preview and the hero:

- Faithful macOS Ghostty chrome: titlebar proportions, traffic lights with
  hover glyphs, title string (`story — termrock`), window shadow/radius,
  focused/unfocused states (traffic lights grey out, title dims — exactly like
  macOS), default Ghostty window padding.
- Content area is *only* the terminal surface — nothing web inside the glass.
- Font: JetBrains Mono, self-hosted, with cell metrics **measured from the
  loaded font** (advance width, ascent/descent) instead of hardcoded 9×18.
- Cursor: Ghostty defaults — blinking block, correct blink cadence, hidden when
  unfocused; rendered by the VT layer (Path A), not inferred from cell contents.
- Focus model: click or hover-to-focus (configurable), visible focus ring using
  `Role::BorderFocused` phosphor green; `Esc Esc` releases focus back to the
  page so keyboard users are never trapped.
- Extras that sell realism: text selection with copy, wheel scrollback where
  the story supports it, OSC 8 hyperlink hover underlines in stories that emit
  them.

### 4.4 Static fallback and performance

- **Tier 2 poster frames:** each story pre-renders one SVG/PNG poster frame at
  its default size for SSR/prerender, no-JS visitors, social cards, and LCP.
  Hydrate to the live WASM surface on interaction or on viewport entry.
- Replace the 477 MB `preview-frames/` tree with: one wasm module (a few MB)
  plus one poster frame per story (a few KB each). This is also a CI/deploy
  win (site artifact currently dominated by frame JSON).
- Lazy-mount previews via IntersectionObserver; at most N live terminals
  running simultaneously (pause off-screen instances' tick loop).

## 5. Page anatomy (shadcn/ui-derived)

### 5.1 Component page (one per widget)

Order, top to bottom — preview is the hero, everything else supports it:

1. **Title + one-line contract** (`Panel — composable container; focus ≠ selection`).
2. **Live Ghostty window, center-stage, large.** Default story mounted,
   focused hint bar (`click to focus · ? for keys`). Tabs directly on the
   window frame: **Preview / Code** — Code shows the exact Rust snippet that
   produces what the preview renders, copy button, `termrock = { git, rev }`
   pin snippet one keystroke away.
3. **States row:** the story's states (focused, disabled, loading, error,
   narrow, unicode …) as small live thumbnails; clicking one swaps the main
   window's story. This replaces the current stories table — states are
   *explored*, not listed.
4. **Knobs panel:** the lookbook knob system surfaced in the browser —
   variant, density, theme, glyph set (`Ascii` proof), color capability
   (`NO_COLOR` proof), motion. Changing a knob mutates the live instance.
   This demonstrates the capability ladder better than any prose.
5. **Keyboard map:** the widget's real key bindings, rendered with the `Kbd`
   widget style; pressing a listed key while the preview is focused visibly
   fires it (binding rows flash on use).
6. **Usage:** minimal → composed Rust examples, kept compile-checked by the
   existing snippet checker.
7. **API / anatomy / interaction contract:** generated from the registry as
   today, restyled to man-page rhythm.

### 5.2 Application pattern pages (one page per application)

- Each `patterns/` composite (Connection Manager, Agent Workbench, Git
  Workbench, Plan Review, …) gets **its own page** whose hero is a
  **full-width, tall Ghostty window running the entire application recipe
  live** — multi-panel focus traversal, overlays, dialogs, the works.
- Below the hero: what the recipe demonstrates, which building blocks compose
  it (each linked, with mini live thumbnails), the full recipe source, and the
  building-block-vs-composite classification note.
- A **"Zen mode"** control expands any pattern to a full-viewport terminal
  (site chrome hides except the status bar) — the closest a browser gets to
  running the app.
- Index page `/patterns`: grid of live poster windows (idle animation on
  hover), one card per application.

### 5.3 Information architecture

```
/                     boot + live hero + value proposition + quick start
/docs                 handbook (concepts: kernel, focus, overlays, intents,
                      capability ladder, theming) — man-page styled
/docs/components      catalog index (ls-style, grouped, searchable)
/docs/components/*    164 component pages per §5.1
/patterns             application gallery
/patterns/*           one live application per page per §5.2
/themes               theme playground: ThemePicker live, palette tokens,
                      NO_COLOR / ASCII / density matrix on a live story
/lookbook             full-screen web lookbook: every story, knobs, themes —
                      the Studio experience in the browser
```

## 6. Implementation phases

Each phase lands independently on `main` with its validation gate green.

### Phase 1 — WASM story host (foundation)
- Create `termrock-lookbook-web`; compile story registry to wasm32; Path B
  cell export wired into the existing canvas painter behind a feature flag on
  `TerminalPreview` (`live=true`).
- Gate: any story mounts live; typing into `text-input/basic` edits text;
  wasm size measured and recorded in `performance-baseline.md`.

### Phase 2 — Ghostty fidelity
- Integrate `ghostty-web` (Path A): ANSI backend in the wasm host, VT-driven
  rendering, real cursor/selection/scrollback; metrics-derived cell size.
- Build `<GhosttyWindow>` chrome to visual parity with real Ghostty on macOS
  (side-by-side screenshot comparison is the review artifact).
- Gate: side-by-side of ≥5 representative stories (panel, data-table,
  command-palette, diff, prompt-composer) vs the same stories in real Ghostty;
  reviewer signs off on parity. Path B retained for CI determinism.

### Phase 3 — Component page redesign
- Rework `gen-component-pages.ts` to emit the §5.1 anatomy (states row, knobs,
  keyboard map, Preview/Code tabs). Delete frame-pack export from the build;
  add poster-frame export (one frame per story).
- Gate: all 164 pages regenerate; `check:site` passes; `preview-frames/`
  removed; site artifact size drop recorded.

### Phase 4 — Terminal site chrome
- Token bridge (Rust `DesignSystem` → CSS custom properties, generated),
  monospace layout system, command-palette navigation (live TermRock
  CommandPalette), keyboard-first bindings, shell-path breadcrumbs, tmux status
  bar, CRT toggle, light "paper" theme, boot-sequence landing with live hero.
- Gate: Lighthouse a11y ≥ 95, reduced-motion audit, keyboard-only walkthrough
  of the entire site, palette navigation covers 100% of routes.

### Phase 5 — Application pages + lookbook
- One page per pattern with full live app + Zen mode; `/patterns` gallery;
  `/lookbook` full-screen story browser; `/themes` playground.
- Gate: every `patterns/` composite reachable and drivable end-to-end in the
  browser (scripted Playwright traversal per pattern).

## 7. Risks and mitigations

| Risk | Mitigation |
|---|---|
| wasm module too large (312k-line crate) | `wasm-opt -Oz`, fat LTO, panic=abort, strip `std` fat; split per-category modules lazily loaded; poster frames keep first paint instant regardless |
| `ghostty-web` is pre-1.0 | Path B (cell-direct) is a complete fallback renderer sharing the same host module; pin exact version; upstream issues as found |
| Ratatui version skew (`ratatui-core 0.1.2` vs helper crates) | We own the backend shim; Ratzilla is reference-only |
| Determinism loss (live tick vs baked frames) in CI | Host-driven `tick(ms)`, seedless stories, Path B pixel snapshots stay the CI oracle |
| Keyboard capture conflicts (page vs terminal) | Explicit focus model, `Esc Esc` release, focus ring, scoped `window` capture only while focused (pattern already proven in `TerminalPreview`) |
| Font metric drift across platforms/DPR | Metrics measured from loaded font at runtime; integer cell snapping; DPR-aware canvas (existing `paintDpr` logic carries over) |
| Docs build time regression | wasm built once in CI, cached by crate hash; poster frames trivially cheap vs current 477 MB export |

## 8. Success criteria (definition of wow)

1. A visitor types into a TextInput preview and sees their own text — within
   the first 10 seconds on the landing page.
2. Pixel-parity screenshot of a preview vs real Ghostty running the same story
   is hard to tell apart in review.
3. `Ctrl+K` navigation is visibly the library's own CommandPalette.
4. Every one of the 164 components and 36 patterns is live-drivable; zero
   baked frame packs remain in the repo.
5. Site works keyboard-only, respects reduced motion, and holds a11y ≥ 95 —
   terminal aesthetic never costs accessibility.

## 9. References

- shadcn/ui docs structure: https://ui.shadcn.com/docs
- ghostty-web (Ghostty VT in the browser, xterm.js-compatible): https://github.com/coder/ghostty-web · https://www.npmjs.com/package/ghostty-web
- libghostty direction: https://mitchellh.com/writing/libghostty-is-coming
- wterm / @wterm/ghostty (libghostty packaging reference): https://github.com/vercel-labs/wterm
- Ratzilla (Ratatui web backends, prior art): https://github.com/ratatui/ratzilla
- The Monospace Web (grid-disciplined mono layout): https://github.com/owickstrom/the-monospace-web
- SRCL / Sacred Computer (terminal web component aesthetics): https://sacred.computer
- terminal.shop, charm.sh, ghostty.org — tone and commitment references
- Internal: `docs/design/phosphor-obsidian-visual-direction.md`,
  `docs/design/lookbook-host-frame.md`, `docs/design/interactive-preview-host.md`,
  `docs/design/component-documentation-standard.md`
