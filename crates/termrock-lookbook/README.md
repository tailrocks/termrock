# termrock-lookbook

Interactive component lookbook for `termrock` — the reference rendering of every shared TUI component in its real state. This is where a developer (or agent) copies the canonical API call for a component, and where SVG previews for the docs are generated.

## What this crate owns

- A `story_*()` per component variant, each calling the **public** `render_*()` helper or `Widget::render` exactly as the real surfaces do.
- Interactive `*Interactor` structs that drive a real component `*State` through `handle_key`, matching real-app usage.
- The SVG preview generator that feeds `docs/public/tui-lookbook/`.

## Architecture tier and allowed dependencies

**Presentation / dev-tool crate.** Allowed workspace deps: `termrock`. It depends on nothing else because it must call only `termrock`'s public API — that is its whole purpose.

## Structure

| Module | Owns | Tests |
|---|---|---|
| [`main.rs`](src/main.rs) | lookbook runner + `check --dir` drift gate | — |
| [`stories.rs`](src/stories.rs) · [`stories/`](src/stories) | one story per component variant | [`tests.rs`](src/stories/tests.rs) |
| [`interactors.rs`](src/interactors.rs) | interactive state drivers | — |
| [`svg.rs`](src/svg.rs) | SVG preview rendering for the docs | — |
| [`tests.rs`](src/tests.rs) | tests | — |

## Public API

None consumed; this crate is a consumer of `termrock`. Its own surface is the lookbook binary (`cargo run -p termrock-lookbook -- render --out docs/public/component-previews` to regenerate, `check --dir …` to verify no drift).

## How to verify

```sh
cargo nextest run -p termrock-lookbook
cargo run -p termrock-lookbook -- check --dir docs/public/component-previews
```

The hard rule — *use only `termrock` public API* — applies to every story and interactor.
