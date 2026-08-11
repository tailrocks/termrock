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

SVG under `docs/public/component-previews/` remains the catalog snapshot SoT.

## Export

```bash
cargo run -p termrock-lookbook -- export-frames --out docs/public/preview-frames
cargo run -p termrock-lookbook -- frame --story list/selection --keys ArrowDown
```

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

## Size packs

`export-frames` writes every `RESPONSIVE_STORY_SIZES` entry (`28x6` … `80x24`)
with interactive steps, plus a default root copy of `40x8`.
