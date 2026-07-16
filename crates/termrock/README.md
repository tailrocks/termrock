# termrock

Product-neutral terminal UI primitives and components for Ratatui applications.

Applications keep domain state and policy while composing stable-ID widgets,
semantic styles, backend-neutral input, scroll/layout helpers, and typed terminal
requests.

TermRock is pre-stable. Pin an exact Git revision:

```toml
[dependencies]
termrock = { git = "https://github.com/tailrocks/termrock", rev = "<commit>" }
```

## Quick start

```rust
use ratatui_core::text::Line;
use termrock::{Theme, widgets::{List, ListRow, ListState, RowRole}};

let theme = Theme::default();
let rows = [ListRow {
    id: "inbox",
    label: Line::from("Inbox"),
    trailing: Some(Line::from("3")),
    role: RowRole::Item,
    enabled: true,
}];
let list = List::new(&rows, &theme);
let mut state = ListState::new(Some("inbox"));
# let _ = (list, &mut state);
```

## Theming

The default is the phosphor design language. `Theme::slate()` is a complete
rebranding reference with a deliberately different cool-gray palette. Override
individual semantic roles from either preset:

```rust
use ratatui_core::style::Style;
use termrock::{Theme, style::Role};

let theme = Theme::slate().with_role(Role::Accent, Style::new().underlined());
```

Run the interactive showcase with
`cargo run -p termrock --example showcase --features crossterm`.

See the [migration guide](../../MIGRATING.md) for exact consumer edits after
breaking releases. The public API is always allowed to change. TermRock is
deliberately not stable yet and provides no backward-compatibility guarantees
of any kind.
