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
| status footer | `termrock::widgets::StatusBar` |
| detail table | `termrock::widgets::DetailTable` |
| confirm/save-discard | `termrock::widgets::ChoiceDialog` with caller actions |
| error/status popup | `termrock::widgets::{MessageDialog,Toast}` |
| diff view | `termrock::widgets::DiffView` |
| modal geometry and scrolling | `termrock::{layout,interaction,scroll}` |
| terminal escapes | `termrock::osc` typed requests and pure encoders |

The canonical namespaces own their complete implementation. Import dialog geometry from `termrock::layout`, focus/hover/modal lifecycle from `termrock::interaction`, scroll geometry and rendering from `termrock::scroll`, and backdrop policy from `termrock::widgets::Backdrop`. `Backdrop::default()` provides the opaque terminal-background policy. The entire `termrock::components` facade is removed; consumers must compose the canonical widgets instead of retaining donor-shaped render helpers or imports.

Wrap foreign receivers with `runtime::ClosureSubscription`; TermRock deliberately does not depend on Tokio. Brand headers, row construction, lifecycle stacks, output policy, and application-specific runtime helpers remain in the consumer.
