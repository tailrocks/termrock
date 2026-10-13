# termrock-lookbook

Interactive component lookbook for `termrock` — the reference rendering of every shared TUI component in its real state. This is where a developer (or agent) copies the canonical API call for a component, and where **Ghostty-class truecolor frame packs** for the docs site are exported.

## What this crate owns

- A `story_*()` per component variant, each calling the **public** `render_*()` helper or `Widget::render` exactly as the real surfaces do.
- Interactive `*Interactor` structs that drive a real component `*State` through `handle_key`, matching real-app usage.
- Frame-pack export (`export-frames`) that feeds `docs/public/preview-frames/` for the docs `TerminalPreview` host.
- Optional SVG render (`render` / `check --dir`) for offline paint-determinism — **not** the docs product path.

## Architecture tier and allowed dependencies

**Presentation / dev-tool crate.** Allowed workspace deps: `termrock`. It depends on nothing else because it must call only `termrock`'s public API — that is its whole purpose.

## Structure

| Module | Owns | Tests |
|---|---|---|
| [`main.rs`](src/main.rs) | lookbook runner + frame export + optional SVG check | — |
| [`stories.rs`](src/stories.rs) | one story per component variant | [`tests.rs`](src/tests.rs) |
| [`interactors.rs`](src/interactors.rs) | interactive state drivers | — |
| [`frame.rs`](src/frame.rs) | truecolor frame encode / size helpers for docs | unit tests in module |
| [`svg.rs`](src/svg.rs) | optional SVG render (CI determinism under `target/`) | — |
| [`tests.rs`](src/tests.rs) | tests | — |

## Public API

None consumed; this crate is a consumer of `termrock`. Docs packs:

```sh
cargo run -p termrock-lookbook -- export-frames --out docs/public/preview-frames --story list/selection
# or mise run export-preview-frames
```

## How to verify

```sh
cargo nextest run -p termrock-lookbook
# docs catalog (one Ghostty focus + packs for embeds):
(cd docs && bun run check:catalog)
# optional SVG determinism (not docs/public):
cargo run -p termrock-lookbook -- render --out target/render-check
cargo run -p termrock-lookbook -- check --dir target/render-check
```

The hard rule — *use only `termrock` public API* — applies to every story and interactor.
