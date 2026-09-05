# Shared live preview host

## Accepted architecture

Component docs, application-pattern docs, and the native catalog execute one
backend-neutral Rust demo runtime. Ratatui remains the paint engine; the web
host only translates browser events and paints returned cells.

```text
                  stable demo id
                       │
              termrock-catalog library
              CatalogSession + application host
               │                    │
        native catalog        wasm-bindgen adapter
               │                    │
      crossterm events       browser events → Event
               └──── same Rust state/paint ────┘
                                      │
                              Ratatui Buffer cells
                                      │
                         TerminalPreview canvas painter
```

| Piece | Location | Responsibility |
|---|---|---|
| Shared session | `crates/termrock-catalog/src/host.rs` | Mount, event dispatch, reset, render, hints, outcomes |
| Public-API pages | `crates/termrock-catalog/src/pages/` | Deterministic sample state using real widget/pattern APIs |
| Catalog | `crates/termrock-catalog/src/catalog.rs` | Stable IDs, dimensions, factories, classifications |
| WASM adapter | `crates/termrock-catalog-web/src/lib.rs` | Handle lifecycle and JSON boundary |
| Web host | `docs/src/components/TerminalPreview.tsx` | Lazy module load, DOM event translation, cell paint, status chrome |
| Poster export | `docs/scripts/export-preview-posters.ts` | One initial frame per embedded demo for fallback |

## Session contract

One mount owns one long-lived Rust value. `dispatch` accepts normalized key
press/repeat/release, pointer move/down/up/drag, wheel, paste, resize, focus,
and tick events. It returns whether paint changed, the latest visible outcome,
current action hints, interactivity, and the next time deadline. `reset`
recreates initial state; `unmount` invalidates the handle.

Unknown handles and malformed events return errors without panicking. Multiple
handles remain isolated. Both hosts use the same factory; no React or native
shell component behavior exists.

## Browser behavior

- Passive paint uses `role="img"`, is removed from tab order, and does not trap
  page keys or advertise actions.
- Interactive demos use `role="application"` and capture only keys supported
  by their interaction family.
- Pointer coordinates are mapped to exact terminal cells. Drag uses pointer
  capture. Hover does not steal focus.
- `beforeinput` and paste preserve Unicode input. Key lifecycle and modifiers
  cross the WASM boundary.
- `ResizeObserver` computes an arbitrary live cell grid and dispatches resize;
  it never chooses a pre-rendered size.
- Timed demos receive host elapsed time while visible. Reduced motion stops
  automatic animation.
- Full preview expands the same session. Reset recreates that session's state.
- The status bar shows only current Rust hints and the latest Rust outcome.

## Paint and cursor law

The retained canvas painter draws each Ratatui cell with 24-bit foreground and
background color, fixed monospaced grid geometry, continuous underlines, wide
glyph handling, and vector box/block geometry. This is a deterministic
Ghostty-styled documentation surface, not a claim that Ghostty's VT engine runs
in the browser.

The web host never infers selection or a cursor from unrelated glyphs. It
never creates a synthetic block cursor. Editable widgets paint their own caret
into the Ratatui buffer; non-editable and passive widgets show none.

## Static fallback

`docs/public/preview-posters/<demo>.json` contains one deterministic initial
frame for every embedded demo. It supports no-JS/WASM-failure rendering and
visual review only. Multi-step frame packs, sibling-story tours, probe keys,
idle cycling, and step scrollbars are forbidden.

Regenerate and validate:

```sh
rtk bun --cwd docs run build:preview-runtime
rtk bun --cwd docs run build:preview-posters
rtk bun --cwd docs run check:preview
rtk cargo test -p termrock-catalog --lib --locked
```

## Acceptance families

Deterministic traces cover activation, choice/disclosure, editor/form,
selection/navigation, scrolling/virtualization, drag/continuous value, timed
state, reset, resize, focus, paste, and invalid handles. Browser suites mirror
representative traces for components and patterns. A new active component is
incomplete until its primary demo accepts its real events and reports an
observable outcome.
