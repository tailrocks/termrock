# Plan 001: Replace preview slides with one live Rust demo runtime

> **Executor instructions:** Follow this plan step by step. Run every
> verification command before continuing. Update `plans/README.md` when done.
> Do not implement web-only widget behavior: the website and native Lookbook
> must execute the same Rust demo object.
>
> **Drift check (run first):**
> `rtk git diff --stat 26457206..HEAD -- Cargo.toml Cargo.lock crates/termrock-lookbook docs/src/components docs/scripts docs/package.json docs/public mise.toml`
> If current code no longer matches the evidence below, stop and reconcile the
> plan before editing.

## Status

- **Execution:** DONE on `feat/live-interactive-docs`
- **Priority:** P1
- **Effort:** L
- **Risk:** HIGH; changes the preview runtime and browser build pipeline
- **Depends on:** none
- **Category:** direction / architecture
- **Planned at:** commit `26457206`, 2026-08-12

## Why this matters

The current preview looks like a terminal but behaves like a slide deck. Mouse,
keyboard, wheel, and the synthetic cursor choose pre-rendered JSON frames; they
never reach a persistent widget state. This makes the documentation unable to
demonstrate the product's most important contracts. One backend-neutral demo
catalog must power both the browser and native Lookbook so they cannot drift.

## Current state

- `docs/src/components/TerminalPreview.tsx:397-499` fetches and caches numbered
  JSON frames. `step` is the only persistent preview state.
- `TerminalPreview.tsx:655-704` advances an idle tour and maps wheel motion to
  another frame. `TerminalPreview.tsx:706-754` maps keys to a frame index.
- `TerminalPreview.tsx:793-847` maps clicks and scrollbar drags to a frame index,
  not `termrock::input::MouseEvent`.
- `TerminalPreview.tsx:532-538` and `preview-metrics.ts:752-768` infer a cursor
  from underline/reverse paint, then fall back to the current slide number.
- `crates/termrock-lookbook/src/frame.rs:287-417` probes six navigation keys and
  builds tours from sibling stories. `frame.rs:419-471` replays keys offline.
- Current artifacts contain 201 manifests: 14 baked-key packs, 172 tours, and
  15 static packs. The 165 public component pages use 14 baked-key previews,
  136 tours, and 15 static previews. None is a live Rust session.
- `crates/termrock-lookbook/src/interactors.rs:64-85` exposes render, key, mouse,
  theme, and knobs, but only as a native-only trait returning `bool`. Only 18 of
  1,065 stories register an interactor in `stories.rs`.
- Native Lookbook already forwards real events to its selected interactor at
  `crates/termrock-lookbook/src/app.rs:470-624`. This is the behavior to share.
- `crates/termrock-lookbook/Cargo.toml:18-22` enables `termrock/crossterm` and the
  `ratatui` facade unconditionally. A web library cannot inherit that shape.
- `termrock::input::Event` is already backend-neutral and covers key press,
  repeat, release, mouse move/down/up/drag/wheel, paste, resize, and focus at
  `crates/termrock/src/input/event.rs:123-239`.
- The exact WASM target is not installed at planning time. Treat WASM support as
  a proof gate, not an assumption.

## Architectural decision

Keep the accepted `FrameCell[]` canvas painter and replace only its source and
event path. Extract a backend-neutral demo library from `termrock-lookbook`.
Both hosts instantiate a persistent demo by ID and call the same methods:

```rust
trait Demo {
    fn descriptor(&self) -> &DemoDescriptor;
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, tick: FrameTick)
        -> DemoPresentation;
    fn handle_event(&mut self, event: Event, tick: FrameTick) -> DemoUpdate;
    fn set_theme(&mut self, theme: RolePalette);
    fn reset(&mut self);
}
```

The exact ownership form may differ, but these contracts may not:

- `DemoDescriptor`: stable ID, component/pattern identity, default dimensions,
  interaction kind, current action hints, and capability tags.
- `DemoPresentation`: cells plus optional explicit editor caret, pointer shape,
  visible outcome/status text, and next animation deadline.
- `DemoUpdate`: whether paint changed and an optional typed/serialized outcome.
- Demos own deterministic sample application state and translate real widget
  outcomes into visible demo effects. Widgets still own rendering, hit geometry,
  focus behavior, and interaction. External I/O remains fake and deterministic.
- Passive demos advertise no input. Never invent clicks for a `Badge`, `Text`,
  separator, or other paint-only component.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tool check | `rtk --version && which rtk` | version and path printed |
| Add target | `rtk proxy rustup target add wasm32-unknown-unknown` | target installed |
| Shared demo tests | `rtk cargo test -p termrock-lookbook --lib --locked` | all pass |
| WASM build | `rtk bun --cwd docs run build:preview-runtime` | web package generated, exit 0 |
| Browser interaction tests | `rtk bun --cwd docs run test:preview` | all named flows pass |
| Site build | `rtk bun --cwd docs run build` | static build and checks pass |
| Full gate | `rtk mise run gate` | exit 0 |

## Suggested executor toolkit

- Per repository instructions, use Context7 for current `wasm-bindgen`,
  `wasm-pack`, Vite, and browser-test setup before choosing dependency versions
  or configuration. If Context7 is unavailable, use only the projects' primary
  documentation.
- [`wasm-bindgen` animation-frame guide](https://wasm-bindgen.github.io/wasm-bindgen/examples/request-animation-frame.html)
  for host-injected animation scheduling.
- [`wasm-pack build` guide](https://rustwasm.github.io/docs/wasm-pack/commands/build.html)
  for `--target web` output. Confirm the maintained URL/version through
  Context7 before implementation.
- [shadcn Dialog](https://ui.shadcn.com/docs/components/radix/dialog) as the
  behavior reference: a trigger opens the mounted dialog and close mutates that
  same example, never a screenshot index.

## Scope

**In scope:**

- `Cargo.toml`, `Cargo.lock`
- `crates/termrock-lookbook/Cargo.toml`, `src/lib.rs` (new), and the existing
  `app.rs`, `frame.rs`, `interactors.rs`, `stories.rs`, `main.rs`
- `crates/termrock-lookbook-web/` (new WASM adapter crate)
- `docs/src/components/TerminalPreview.tsx`, `preview-metrics.ts`, and focused
  runtime/event adapter modules and tests under `docs/src/components/`
- `docs/tests/preview/` and `docs/playwright.config.ts` (new browser suite)
- `docs/scripts/build-preview-runtime.ts` and preview/poster checks
- `docs/package.json`, `docs/bun.lock`, `mise.toml`, relevant CI workflow setup
- replacement of `docs/public/preview-frames/` with one default poster per demo
- `crates/termrock-lookbook/{README.md,AGENTS.md}` and
  `docs/design/docs-site-terminal-experience-plan.md`

**Out of scope:**

- Replacing the current canvas painter, font, colors, or terminal chrome
- Ghostty VT, xterm.js, Ratzilla renderer adoption, selection, or scrollback
- New public TermRock behavior solely for the demo
- Component-page information architecture (Plan 002)
- Application-pattern coverage (Plan 003)

## Git workflow

The user's execution instruction supersedes the original workflow: all three
plans ship from `feat/live-interactive-docs` in one PR to `main`. Commits use
Conventional Commits, DCO sign-off, and
`Co-authored-by: Codex <codex@openai.com>`.

## Steps

### Step 1: Prove the backend-neutral slice on WASM

1. Install `wasm32-unknown-unknown` and add a minimal `cdylib` web crate.
2. Split `termrock-lookbook` into a library plus native binary. Move catalog,
   demo state, cell encoding, and test backend use into the library. Gate only
   terminal session/crossterm code behind a native feature.
3. Replace `ratatui` facade imports in the shared slice with pinned
   `ratatui-core`/`ratatui-widgets` imports.
4. Compile and execute a minimal session containing one button and one timed
   spinner. Prove that a host-supplied clock can construct the existing
   `FrameTick` without panic in a browser.

**Verify:** `rtk bun --cwd docs run build:preview-runtime` exits 0 and a focused
WASM test renders two different spinner frames at controlled timestamps.

### Step 2: Replace `StoryInteraction` with the shared demo contract

1. Implement the descriptor, event, presentation, outcome, and reset contract
   in the lookbook library. Keep it internal to the tooling crates.
2. Convert current interactors without changing their public-widget calls. A
   demo must call the same public `paint`, `handle_key`, `handle_mouse`, and
   outcome APIs copied by downstream applications.
3. Change native Lookbook to mount the demo factory, route all input through
   `handle_event`, render with the injected tick, and display descriptor hints
   plus the latest outcome. Keep its catalog/sidebar shell native.
4. Add trace tests that replay the same normalized event sequence against the
   native and web adapters and compare cells, outcome, hints, and caret metadata.

**Verify:** shared tests pass; native Lookbook can open/close ChoiceDialog,
edit TextArea, drag SplitPane, change Tabs, and trigger/dismiss Toast.

### Step 3: Expose persistent browser sessions

Create a handle-based WASM API: catalog, mount, event, tick, resize, theme,
reset, frame, and unmount. Each handle owns one Rust demo instance. Serialize
the existing cell format first; measure before inventing a packed format.
Reject unknown handles and malformed events without panicking.

Map browser input completely:

- `keydown` and `keyup` to press/repeat/release with modifiers
- `beforeinput`/paste to characters or `Event::Paste`
- pointer move/down/up/drag with pointer capture and exact terminal cell
- vertical/horizontal wheel, focus gained/lost, and arbitrary resize

Hover must work without changing slides or requiring keyboard focus. A click
may focus the preview, but pointer-enter must not steal focus from the page.

**Verify:** WASM tests prove session isolation, reset, resize, key lifecycle,
paste, hover, click, drag, wheel, focus, and invalid-handle behavior.

### Step 4: Turn `TerminalPreview` into a live host

1. Lazy-load one WASM module and mount one Rust session per visible preview.
2. Keep `paintCanvas` and its visual metrics. Remove `step`, tours, frame-pack
   prefetch, slide scrollbars, generic navigation interception, and idle cycling.
3. Remove `inferCursorFromFrame` and `cursorCellForStep`. Render no synthetic
   cursor. Use only caret metadata emitted by an editable demo; the widget's
   own buffer paint remains authoritative.
4. Render dynamic action hints and latest outcome beside the terminal. Examples:
   `Enter/click Open`, `Esc Close`, `drag divider`, `type to edit`, and
   `wheel to scroll`. Add Reset. Never show generic actions unsupported by the
   current demo state.
5. Drive timed demos from `requestAnimationFrame` only while visible and while
   the demo reports another deadline. Respect reduced motion and pause offscreen.
6. Resize the live Rust session at the measured cell grid; do not choose among
   five static size packs.

**Verify:** focused React/unit tests prove complete event translation and that
passive previews neither capture navigation keys nor expose a cursor.

### Step 5: Add browser acceptance tests and remove slide artifacts

Add a headless-browser suite covering these real flows:

1. ActionLink hover changes cells; click emits visible activation.
2. Button press/release enters loading, then deterministic completion.
3. Dialog trigger opens; close/Escape dismisses; opener focus is restored.
4. ChoiceDialog Continue and Cancel produce different visible outcomes.
5. TextInput accepts Unicode text, paste, cursor movement, and mouse placement.
6. Slider and SplitPane respond to keys and pointer drag.
7. Tabs change by click and their documented keys, never page-scroll shortcuts.
8. TreeTable expands/collapses; VirtualList consumes real wheel scrolling.
9. Toast appears, dismisses, and expires; Spinner advances from host time.
10. Passive component has no fake cursor or fake interaction label.

After the suite is green, export only one default poster per embedded demo for
SSR/no-JS fallback. Delete multi-step JSON packs, exporter tour/probe code, and
stale docs claiming `step`, `scene`, `live pack`, or `idle tour`. Update the
design plan so cell-direct live WASM is the accepted path and Ghostty VT remains
an independent future experiment.

**Verify:** browser suite, site build, then full gate all pass. Search commands:

```sh
rg -n "idle tour|live pack|stepKey|resolve_export_tour|inferCursorFromFrame|cursorCellForStep" docs crates/termrock-lookbook
find docs/public/preview-frames -type f 2>/dev/null | wc -l
```

Expected: no production slide/runtime matches; old multi-step tree absent.

## Done criteria

- [x] Website and native Lookbook mount the same Rust demo factory by stable ID.
- [x] Real browser events mutate persistent Rust state and typed outcomes.
- [x] Current canvas rendering quality and responsive paint remain unchanged.
- [x] Hints and visible feedback come from current demo state.
- [x] No synthetic cursor appears on non-editable components.
- [x] Timed animation uses injected time and stops offscreen.
- [x] One poster per demo is the only static fallback; slide packs are gone.
- [x] Ten browser acceptance flows and native/web parity tests pass.
- [x] `rtk mise run gate` passes and the plan row is `DONE`.

## STOP conditions

- The backend-neutral lookbook library cannot compile for WASM because a
  `termrock` dependency requires OS/crossterm behavior after feature cleanup.
- `FrameTick` cannot be safely constructed/executed in-browser. Stop and write a
  separate public clock redesign with the next migration file; do not sample
  time inside widgets or fake animation in JavaScript.
- A demo requires private TermRock state or duplicates widget hit/key logic.
  Stop and report the missing public API; any API fix needs its own migration.
- Matching native/web behavior requires two demo implementations.
- A proposed fix changes the accepted painter or visual design.
- Any verification fails twice after a focused correction.

## Maintenance notes

The shared demo is an executable usage example, not a testing backdoor. Review
every demo against `crates/termrock-lookbook/AGENTS.md`: public TermRock APIs
only, consumer-owned effects, typed outcomes. Future components add one demo;
both hosts receive it automatically.
