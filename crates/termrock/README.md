# termrock

Product-neutral terminal UI primitives and components for Ratatui applications.

Applications keep domain state and policy while composing stable-ID widgets, semantic styles, backend-neutral input, scroll/layout helpers, and typed terminal requests.

The `runtime` module owns the shared update-loop contracts (`Subscription`, `UpdateResult`, `Component`, `View`) and the shared frame driver `runtime::drive_frame`, which runs a `View<Model>` render plus a frame-scoped overlay closure inside one `Terminal::draw` call. Downstream surfaces dispatch their per-tick rendering through that single driver.

The catalog and migration guide live in the repository documentation.

## Theming

The default is the phosphor design language. `Theme::slate()` is a complete
rebranding reference with a deliberately different cool-gray palette. Override
individual semantic roles from either preset:

```rust
use ratatui_core::style::Style;
use termrock::{Theme, style::Role};

let theme = Theme::slate().with_role(Role::Accent, Style::new().underlined());
```
