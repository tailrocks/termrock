# 0001 — Canonical namespaces (`v0.7.0`)

## Before

Consumers imported donor-shaped helpers through `termrock::components`. The
facade mixed widget rendering, geometry, interaction, terminal escapes, and
consumer-oriented names. Some consumers kept local copies of those neutral
implementations.

## After

The canonical namespaces own the complete reusable implementation. Consumers
compose widgets directly and retain only product state, wording, effects, and
projections into TermRock data.

| Before | After |
|---|---|
| button strip | `termrock::widgets::ActionBar` |
| select list | `termrock::widgets::List` |
| tab strip | `termrock::widgets::Tabs` |
| text field or filter helper | `termrock::widgets::TextInput` composition |
| panel and hint-row helpers | `termrock::widgets::{Panel, HintBar}` |
| status footer | `termrock::widgets::{StatusBar, StatusBarState}` with consumer-owned slot meaning |
| detail renderer | `termrock::widgets::DetailTable` with consumer-owned stable row IDs and payloads |
| confirm or save/discard helper | `termrock::widgets::{ChoiceDialog, ChoiceDialogState}` with consumer actions and outcomes |
| error or status popup | `termrock::widgets::{MessageDialog, DetailTableState, Toast}` |
| diff helper | `termrock::widgets::DiffView` |
| modal geometry and scrolling | `termrock::{layout, interaction, scroll}` |
| raw terminal escapes | `termrock::osc` typed requests and pure encoders |

## Consumer actions

1. Remove every `termrock::components` import; the facade no longer exists.
2. Delete copied neutral rendering, geometry, focus, hover, scroll, backdrop,
   and terminal-escape implementations.
3. Import dialog geometry from `termrock::layout`, focus/hover/modal lifecycle
   from `termrock::interaction`, and scrolling from `termrock::scroll`.
4. Use `termrock::widgets::Backdrop::default()` for the opaque
   terminal-background policy.
5. Wrap foreign receivers with `runtime::ClosureSubscription`; TermRock does
   not depend on Tokio.
6. Keep brand headers, row construction, lifecycle stacks, output policy, and
   application-specific runtime helpers in the consumer.
