# Interactive Ghostty-class preview host

## Goal

Embed **live** TermRock story paint in documentation so widgets and composites
feel like a real TUI (Ghostty-class truecolor, monospaced cells, keyboard), not
only static SVG snapshots.

## Architecture

```
lookbook story + interactor  ──paint──► ratatui Buffer
        │                                  │
        │ keys (ArrowDown, …)              ▼
        └──────────────────────  encode_buffer → TerminalFrame JSON
                                                  │
                          docs/public/preview-frames/<story>/
                                                  │
                          TerminalPreview (canvas) ◄── ↑↓ / click
```

| Piece | Location | Role |
|-------|----------|------|
| Frame bridge | `crates/termrock-lookbook/src/frame.rs` | Truecolor encode, key decode, paint after keys |
| CLI | `termrock-lookbook frame` / `export-frames` | Export packs for docs |
| Host | `docs/src/components/TerminalPreview.tsx` | Ghostty chrome + canvas paint + input |
| Assets | `docs/public/preview-frames/*` | Deterministic step graphs |

**Docs catalog SoT is Ghostty frame packs only** (`docs/public/preview-frames/`).
Component MDX must not embed SVG or `component-previews/`. Lookbook `render`
SVG remains an optional offline tooling path, not the documentation surface.

## Export

```bash
cargo run -p termrock-lookbook -- export-frames --out docs/public/preview-frames
cargo run -p termrock-lookbook -- frame --story list/selection --keys ArrowDown
```

## Docs page shape (one focus)

Each component reference page embeds **exactly one** `TerminalPreview` for the
primary lookbook story. Other stories appear in a **Stories** markdown table.
Interactivity (keys / click / size remap) replaces the old multi-SVG gallery.

## Embed

```mdx
<TerminalPreview story="list/selection" interactive />
```

Primary embeds (widget + composite):

| Surface | Story pack |
|---------|------------|
| List | `list/selection` |
| Button | `button/activation` |
| Tabs | `tabs/status` |
| Tree | `tree/navigation` |
| Form | `form/responsive` |
| Picker | `picker/basic` |
| AgentWorkbench (handbook) | `agent-workbench/basic` |

Repeatable export: `mise run export-preview-frames` (or `termrock-lookbook export-frames`).

## Fidelity notes

- 24-bit RGB cells from Ratatui `Color::Rgb` / named phosphor greens; host paints
  **every** cell background (including pure black) — no ANSI 16 collapse.
- ~9×18 cell metrics (matches SVG export); JetBrains Mono stack; canvas keeps
  **fixed CSS pixel size** (no `max-width` stretch) so cell geometry stays integer.
- Ghostty window chrome: traffic lights, title, blinking caret when focused,
  status bar (`step n/m`, size key, RGB24, key hints).
- **Responsive remap (shipped):** host `ResizeObserver` → `storySizeForCssHost` /
  `colsForCssWidth` / `rowsForCssHeight` (mirrors Rust `frame.rs`) →
  `pickSizeKey` → load `preview-frames/<story>/<cols>x<rows>/<step>.json`
  re-painted at that story size. Not letterbox-only.
- Interactive packs step pre-painted interactor states (real TermRock paint), not mock HTML lists.
- **Step key probe:** export uses `preferred_step_key` (Down/Right/Left/Up/j/Tab)
  so horizontal widgets (Tabs) bake correct multi-step graphs.
- **Composite tour:** `agent-workbench/basic` packs multi-scene workbench stories
  (`tool-running`, `permission`, `plan`, `diff`, `session`) as interactive steps.
- **Variant tour (auto):** when a primary story has no keyboard interactor but the
  component has multiple lookbook stories, `resolve_export_tour` bakes up to 6
  sibling paints (narrow/unicode/empty/…) into one pack — one Ghostty surface
  cycles states instead of N static screens. Key-driven interactors still use
  ArrowDown/Right step graphs.
- **Paint fidelity:** canvas repaints after `document.fonts.ready` / JetBrains Mono
  load; glyphs centered in cells via measured mono advance; window-capture
  keydown while focused for reliable TUI nav under automation.
- **Snappy interaction:** adjacent step frames (and the full current size pack)
  are prefetched into an in-memory cache; wheel over the focused host steps
  state/tour; status bar pulses on step change; unfocused chrome dims slightly
  like a real Ghostty window.
- **Load races:** each `loadFrame` carries a generation id; stale async completes
  are ignored so rapid ArrowDown never rewinds the painted step.
- **Wide glyphs:** measured advances spanning ~2 cells paint across the
  continuation cell when it is empty (CJK/emoji grid fidelity).
- **Single-step keys:** window-capture + React handlers share a short dedupe
  window (`shouldAcceptKeyEvent`) so one ArrowDown advances one step, not two.
- **Overlay scrollbar (Ghostty 1.3-class):** multi-step packs show a thin right
  track + phosphor thumb; track click and thumb drag scrub steps via
  `stepFromScrollRatio` / `scrollThumbMetrics`. Canvas click uses pure
  `stepFromPointer` (row for lists, column for short wide strips).
- **Cell probe + paint-true cursor:** pointermove maps CSS → grid via
  `cellAtPointer`; status bar shows `col,row · ch · #fg/#bg` (`formatCellProbe`).
  Block cursor uses `inferCursorFromFrame`: underline / reverse, or leftmost-body
  `▌` only — never panel scrollbar `█` or decorative `›`/`❯` (form/workbench packs).
- **Resize warm:** ResizeObserver speculatively prefetches the pending size pack
  before the 50ms debounce applies the remap.
- **Block cursor:** when focused, a blinking phosphor block tracks the active
  step row (pad + step) so the surface reads like a live Ghostty caret, not a
  static screenshot. Ligatures disabled on canvas for terminal-true metrics.
- **Loading:** uncached pack fetches set `data-preview-loading` + status “loading”.

## Size packs

`export-frames` writes every `RESPONSIVE_STORY_SIZES` entry (`28x6` … `80x24`)
with interactive steps, plus a default root copy of `40x8`.
