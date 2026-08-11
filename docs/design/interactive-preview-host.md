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

Primary embeds: **List** (`list/selection`), **Button** (`button/activation`),
**AgentWorkbench** handbook (`agent-workbench/basic`).

## Fidelity notes

- 24-bit RGB cells from Ratatui `Color::Rgb` / named phosphor greens.
- ~9×18 cell metrics (matches SVG export); JetBrains Mono stack.
- ResizeObserver re-paints; full col/row remap is a follow-up (re-export or WASM).
- Interactive packs step pre-painted interactor states (real TermRock paint), not mock HTML lists.
