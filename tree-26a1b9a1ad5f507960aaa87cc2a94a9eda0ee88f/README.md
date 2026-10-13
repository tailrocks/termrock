# termrock-lookbook

Interactive component lookbook for `termrock` — the reference rendering and persistent state machine for every shared TUI demo. Native terminal and browser WebAssembly hosts instantiate the same stories and interactors.

## What this crate owns

- A `story_*()` per component variant, each calling the **public** `render_*()` helper or `Widget::render` exactly as the real surfaces do.
- Interactive `*Interactor` structs that drive a real component `*State` through `handle_key`, matching real-app usage.
- Shared backend-neutral `DemoSession` used by the native Lookbook and browser WebAssembly adapter.
- One-poster export (`export-posters`) used only when a browser cannot initialize WebAssembly.
- Optional SVG render (`render` / `check --dir`) for offline paint-determinism — **not** the docs product path.

## Architecture tier and allowed dependencies

**Presentation / dev-tool crate.** Allowed workspace deps: `termrock`. It depends on nothing else because it must call only `termrock`'s public API — that is its whole purpose.

## Structure

| Module | Owns | Tests |
|---|---|---|
| [`main.rs`](src/main.rs) | native runner + poster export + optional SVG check | — |
| [`stories.rs`](src/stories.rs) | one story per component variant | [`tests.rs`](src/tests.rs) |
| [`interactors.rs`](src/interactors.rs) | interactive state drivers | — |
| [`frame.rs`](src/frame.rs) | truecolor frame encode / size helpers for docs | unit tests in module |
| [`svg.rs`](src/svg.rs) | optional SVG render (CI determinism under `target/`) | — |
| [`tests.rs`](src/tests.rs) | tests | — |

## Public API

The docs host consumes the library API through `termrock-lookbook-web`. Static fallbacks:

```sh
cargo run -p termrock-lookbook -- export-posters --out docs/public/preview-posters --story list/selection
# or mise run export-preview-posters
```

## How to verify

```sh
cargo nextest run -p termrock-lookbook
# docs catalog and live runtime:
(cd docs && bun run check:catalog)
# optional SVG determinism (not docs/public):
cargo run -p termrock-lookbook -- render --out target/render-check
cargo run -p termrock-lookbook -- check --dir target/render-check
```

The hard rule — *use only `termrock` public API* — applies to every story and interactor.
