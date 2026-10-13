# Runner design: graduate the loop, remove speculative contracts

## Evidence

The lookbook prototype now runs through `crates/termrock-lookbook/src/runner.rs`.
It uses `crossterm::Session`, converts every backend event to
`termrock::input::Event`, draws through a callback, and exits through
`ControlFlow`. The app model, rendering, and routing live separately in
`app.rs`; `run_terminal` is construction plus one runner call. The duplicated
`TerminalGuard` and both long-function lint exceptions are gone.

The old loop redrew before every 120 ms poll, including timeouts. It contained
no timer, animation frame, external receiver, effect queue, or call to any
existing `runtime` API. The prototype preserves that cadence.

## Responsibility inventory

| Existing loop responsibility | Owner | Evidence / boundary |
|---|---|---|
| Raw mode, alternate screen, mouse, paste, cursor, line wrap, rollback | Runner | `Session` already owns acquisition and reverse-order restoration. |
| `event::poll` at 120 ms and `event::read` | Runner | Backend pump policy, identical for every Crossterm app. |
| Crossterm-to-neutral event conversion | Runner | Conversion is infrastructure; app code now sees only `input::Event`. |
| Frame draw and redraw cadence | Runner | One draw callback before each poll preserves current behavior. |
| Quit propagation | App callback + runner | App returns `ControlFlow::Break`; runner restores the session. |
| Resize receipt | Runner pump, app callback | Ratatui handles dimensions; app may invalidate layout state. |
| Focus routing between sidebar and preview | App callback | Lookbook-specific panes and policy. |
| Sidebar selection and preview scrolling | App callback | Product/catalog model state. |
| Story interactor dispatch | App callback | Story-specific behavior; now consumes neutral key/mouse events. |
| Hint selection and assembly | App render | Derived from app focus and its keymaps, not runner policy. |

## Chosen API

Graduate a closure runner under `termrock::runtime`, enabled by `crossterm`:

```rust
pub fn run<Model>(
    model: &mut Model,
    options: RunOptions,
    render: impl FnMut(&mut Model, &mut Frame<'_>),
    update: impl FnMut(&mut Model, input::Event) -> ControlFlow<()>,
) -> io::Result<()>;

pub struct RunOptions {
    pub session: crossterm::SessionOptions,
    pub poll_timeout: Duration,
}
```

`render` receives `&mut Model` because Ratatui stateful widgets update painted
hit geometry during rendering. Forcing `View<&Model>` would require interior
mutability or split render state without improving ownership. Closures keep the
runner compatible with direct, Elm-style, and component-shaped applications.

`ControlFlow` is sufficient for loop lifetime. Effects, executors, process
policy, and domain messages remain consumer-owned. `RunOptions` makes session
and cadence explicit without creating an async runtime abstraction.

## Runtime verdicts

| Existing item | Verdict | Prototype evidence |
|---|---|---|
| `drive_frame` | Delete | The runner owns `Terminal::draw`; a second frame driver would duplicate the only draw boundary. |
| `Component` / `View` | Delete | Plain callbacks expressed the real mutable-render and update requirements without adapter types. No widget or app used the traits. |
| `Dirty` / `UpdateResult` / `NoEffect` | Delete | The real loop has unconditional cadence and only needs continue/quit. Effect typing would claim ownership TermRock explicitly leaves to consumers. |
| `Subscription`, `ClosureSubscription`, `StdSubscription` | Delete | The real loop has no external source. A future multi-source runner should begin from demonstrated wakeup/fairness requirements, not retain an unused receiver wrapper. |

The runner belongs in the existing crate behind `crossterm`, not a new crate.
Its feature graph already contains Session, backend, and event conversion; a
runner crate would add packaging without isolating another dependency.

## Manual prototype checklist

- Terminal opened in a PTY and rendered the gallery.
- Sidebar navigation, focus transfer, and neutral interactor routing remain in
  the app layer and compile against existing tests.
- Resize events are accepted and trigger the next draw; Ratatui reads the new
  area from the terminal.
- `q` returned `ControlFlow::Break` and exited.
- Session output restored cursor, line wrap, bracketed paste, mouse capture,
  alternate screen, and raw mode in reverse order.
- SVG generation/check is unchanged because it does not use the terminal loop.

## Follow-up build-plan stub

1. Add `RunOptions` and `runtime::run` behind `crossterm`, based on the local
   prototype; add fake-pump/session tests for draw, continue, quit, errors, and
   restoration.
2. Move the lookbook from its local runner to the library runner and delete the
   prototype file.
3. Remove `Component`, `View`, `Dirty`, `UpdateResult`, `NoEffect`,
   `drive_frame`, and all subscription types in one forward-only redesign.
4. Add the next migration with exact import/call replacements. Regenerate the
   public API inventory and catalog checks.
5. Correct crate/README claims that downstream loops use `drive_frame`; document
   the closure runner and consumer-owned effects instead.
6. Run the full gate and the PTY restoration checklist before pushing.

## Open questions

- Should the first library runner preserve unconditional 120 ms redraws, or
  expose `RedrawPolicy::{Always, OnEvent}` immediately? Defaulting to `Always`
  preserves observed behavior; `OnEvent` is safe only after animated stories
  gain an explicit tick requirement.
- If a frame clock lands, should ticks arrive as a separate callback or a new
  neutral event? Decide with Plan 031 evidence; do not overload `Subscription`.
- Should `RunOptions` expose an event-pump callback for deterministic tests, or
  keep that seam crate-private? Library tests need injection, consumers do not
  yet demonstrate a need.
