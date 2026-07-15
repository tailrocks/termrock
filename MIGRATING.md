# Migrating to TermRock

Pin a reviewed full Git revision and commit the Cargo lockfile. TermRock keeps executor, output, validation, wording, and application models consumer-owned.

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

Wrap foreign receivers with `runtime::ClosureSubscription`; TermRock deliberately does not depend on Tokio. Brand headers, row construction, lifecycle stacks, output policy, and application-specific runtime helpers remain in the consumer.
