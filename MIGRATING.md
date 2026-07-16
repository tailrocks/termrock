# Migrating to TermRock

Pin a reviewed full Git revision and commit the Cargo lockfile. TermRock keeps executor, output, validation, wording, and application models consumer-owned.

## `v0.7.0` canonical namespace migration

`v0.7.0` is the intentional pre-1.0 breaking release that removes the donor-shaped `components` facade. Consumers must move to the canonical namespaces described below before updating their exact revision pin.

| Donor concept | TermRock path |
|---|---|
| button strip | `termrock::widgets::ActionBar` |
| select list | `termrock::widgets::List` |
| tab strip | `termrock::widgets::Tabs` |
| text field/filter | `termrock::widgets::TextInput` composition |
| panel and hint row | `termrock::widgets::{Panel,HintBar}` |
| status footer | `termrock::widgets::{StatusBar,StatusBarState}` with caller-owned slot meaning |
| detail table | `termrock::widgets::DetailTable` with caller-owned stable row IDs and payloads |
| confirm/save-discard | `termrock::widgets::{ChoiceDialog,ChoiceDialogState}` with caller actions and outcomes |
| error/status popup | `termrock::widgets::{MessageDialog,DetailTableState,Toast}` |
| diff view | `termrock::widgets::DiffView` |
| modal geometry and scrolling | `termrock::{layout,interaction,scroll}` |
| terminal escapes | `termrock::osc` typed requests and pure encoders |

The canonical namespaces own their complete implementation. Import dialog geometry from `termrock::layout`, focus/hover/modal lifecycle from `termrock::interaction`, scroll geometry and rendering from `termrock::scroll`, and backdrop policy from `termrock::widgets::Backdrop`. `Backdrop::default()` provides the opaque terminal-background policy. The entire `termrock::components` facade is removed; consumers must compose the canonical widgets instead of retaining donor-shaped render helpers or imports.

Wrap foreign receivers with `runtime::ClosureSubscription`; TermRock deliberately does not depend on Tokio. Brand headers, row construction, lifecycle stacks, output policy, and application-specific runtime helpers remain in the consumer.

## `v0.8.0` canonical contract completion

`v0.8.0` completes the first multi-surface consumer migration. No compatibility
facade is provided.

| Removed or changed `v0.6.0` surface | Canonical `v0.8.0` surface | Required consumer edit |
|---|---|---|
| `DialogSpec { margin, .. }` | `DialogSpec { horizontal_margin, vertical_margin, .. }` | Replace `margin` with both axis values. Keep them equal for identical geometry or choose them independently. |
| `Dialog { title, body: Line, style }` | `Dialog { title, body: Text, style, theme, emphasis }` | Convert the body to `Text`, pass the shared `Theme`, and select a semantic `PanelEmphasis`. Remove local border/focus styling. |
| Public-field `Toast { message, severity, anchor, style }` rendered into a caller-computed rectangle | `Toast::new(theme, message, severity).anchor(...).margins(...).style(...)` rendered over the full available area | Delete local toast sizing and placement. Pass the full area; `Toast` computes and clears its anchored rectangle. |
| Application-owned picker index and wrap/clamp helpers | `ListState::<usize>::for_count`, `cycle_index`, `move_index`, `reconcile_count`, and `selected_item` | Replace the parallel index with `ListState<usize>` and route keyboard/pointer changes through its methods. |
| Stateless `List { rows }` with consumer styling | `List { rows, theme }` plus state-owned keyboard, hover, scroll, activation, and painted regions | Pass the shared theme; delete duplicate selection, scrolling, hover, and hit-testing helpers. |
| Stateless detail/status/dialog interaction | `DetailTableState`, `StatusBarState`, `ChoiceDialogState`, and typed outcomes | Keep domain meaning in the application, but route navigation, hover, activation, copying, and painted hit regions through canonical state. |

### Dialog layout

Before:

```rust
DialogSpec { margin: 4, /* size and placement */ }
```

After:

```rust
DialogSpec {
    horizontal_margin: 4,
    vertical_margin: 4,
    /* size and placement */
}
```

### Toast placement

Before:

```rust
let toast = Toast { message, severity, anchor, style };
frame.render_widget(&toast, application_computed_rect);
```

After:

```rust
let toast = Toast::new(&theme, message, severity)
    .anchor(anchor)
    .margins(2, 1)
    .style(style);
frame.render_widget(&toast, frame.area());
```

### Indexed picker state

Before:

```rust
selected = wrap_index(selected, items.len(), direction);
```

After:

```rust
let mut state = ListState::<usize>::for_count(items.len());
state.cycle_index(items.len(), direction);
state.reconcile_count(items.len());
let selected = state.selected_item(&items);
```

After updating the exact revision pin, run:

```text
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
```

Delete consumer-local compatibility wrappers after migration. Retaining the old
and new paths together defeats the canonical-state contract and is unsupported.
