# Documentation terminal experience

Status: implemented architecture; continuing catalog coverage is enforced by CI.

## Product decision

The documentation is an executable catalog, not a screenshot gallery. Every
component and application-pattern route mounts a stable demo from the same Rust
catalog used by the native catalog. Interactive surfaces receive real events and
mutate persistent state; passive surfaces remain honest paint.

The existing Ghostty-styled canvas rendering is accepted. Ghostty VT,
selection, scrollback, and alternate web renderers remain independent future
experiments. They are not prerequisites and must not reintroduce a parallel
behavior model.

## Shipped information architecture

```text
/docs/components          canonical building-block catalog
/docs/components/<slug>   one page + one live primary demo
/docs/patterns            application/composite catalog
/docs/patterns/<slug>      one classified pattern + shared live demo
/docs/runtime             shared event/session model
/docs/interaction         kernel contracts
```

The former Component Handbook is removed. Its 84 files have an explicit
migration destination into canonical component pages, pattern pages, or shared
interaction guidance.

## Shipped runtime

- `termrock-catalog` is the canonical library plus native crossterm host.
- `termrock-catalog-web` compiles the same catalog/session runtime to WASM.
- `TerminalPreview` mounts one handle, forwards real browser events, resizes
  the live Ratatui grid, shows current hints/outcomes, supports Reset and Full
  preview, and pauses host time offscreen.
- One poster per embedded demo is the only static fallback.
- Multi-frame packs, inferred cursors, tours, and slide navigation are removed.

Current checked inventory is generated from the Rust authority: 210 public UI
entries, 35 patterns, and 227 canonical catalog scenarios. Native and browser
hosts consume those same IDs; inventory checks fail on drift.

## Experience rules

1. A disclosure surface begins with a real trigger and appears/disappears from
   accepted events.
2. A text editor accepts Unicode, paste, movement, and real caret placement.
3. Selection, scrolling, drag, and continuous values mutate real widget state.
4. Timed paint receives deterministic host time and respects reduced motion.
5. Hints describe only currently supported actions; outcomes make accepted
   actions visible.
6. Passive widgets never trap input or show a cursor.
7. Pattern demos use public `patterns`/`widgets` APIs and deterministic local
   fixtures. They perform no network, process, credential, or file effect.
8. Website and native catalog never own separate demo implementations.

## Verification

```sh
rtk cargo test -p termrock-catalog --lib --locked
rtk bun --cwd docs run build:preview-runtime
rtk bun --cwd docs run check:components
rtk bun --cwd docs run check:patterns
rtk bun --cwd docs run check:catalog
rtk bun --cwd docs run build
rtk mise run gate
```

Browser acceptance covers representative activation, editor, overlay, choice,
drag, navigation, virtualization, timed, and pattern workflows. The shared
Rust traces remain the authoritative parity proof when a browser runner is not
available in a local agent session.

## References

- [Shared live preview host](interactive-preview-host.md)
- [Canonical component documentation standard](component-documentation-standard.md)
- [Building block versus example composite](building-block-vs-example-composite.md)
- [Phosphor/obsidian visual direction](phosphor-obsidian-visual-direction.md)
