# Migrating to TermRock

Pin a reviewed full Git revision and commit the Cargo lockfile. TermRock keeps executor, output, validation, wording, and application models consumer-owned.

TermRock optimizes for the best forward design rather than API compatibility.
Breaking releases expose only the new model: no deprecated aliases,
compatibility facades, or transition shims. Each breaking release section must
provide an old-to-new map, identify changes in state or rendering ownership,
and call out local consumer implementations that should be removed. Signature
or composition changes include before/after Rust examples when the mapping is
not obvious. Consumers should read every section between their pinned revision
and the target revision before updating the full Git revision and lockfile.

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

`v0.8.0` completes the first multi-surface consumer migration. `DialogSpec`
uses independent horizontal and vertical margins, `Dialog` requires a theme
and semantic emphasis, `Toast` is constructed with a theme/message/severity and
owns anchored placement, and `ListState<usize>` owns indexed-picker selection.
Consumers should delete local geometry, toast, table, status-footer, and picker
selection implementations when adopting these contracts; no compatibility
facade is provided.
