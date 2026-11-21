# API contract

This is the supported surface of Termrock for a consumer. It states what a
consumer may rely on and what it must not. Rules that a test holds name
the test; the others name the code.

Termrock is not published to crates.io. Depend on it by path or by git:

```toml
[dependencies]
termrock = { git = "https://github.com/tailrocks/termrock", default-features = false }
# or, in a checkout:
# termrock = { path = "../terminal-components-baseline/crates/termrock", default-features = false }
```

## Packages and imports

| Package | Import | Role |
| --- | --- | --- |
| `termrock` (`crates/termrock`) | `termrock::` | The supported import path. A re-export facade over the `termrock-*` library crates, with a small prelude. |
| `termrock-showcase`, `termrock-tablepro`, `termrock-holla` (`apps/*`) | none | Applications. They import `termrock` only. |
| `jackin-console` (`apps/jackin`) | `jackin_console::` | The Jackin host library and its `jackin-preview` demo. It imports `termrock` only. |

Every module of the facade is a `pub use` of the module of the crate that owns
it (`termrock-core`, `termrock-runtime`, `termrock-controls`, and so on), and
`core`, `metrics`, `ui` and `widgets` are assembled from several crates with
explicit lists, so a type has one identity under `termrock::` and under the owning
crate. `facade_basic_types_preserve_identity` in
`crates/termrock/tests/regressions/type_identity.rs` passes a facade `Response`,
`ItemKey`, `WidgetId` and `RowSource` to functions typed against the owning
crates. `facade_module_paths_resolve` fails when a module path that
the single package exposed before the split stops resolving. A consumer that
mixes the paths gets no conversion and no second type. Depend on `termrock`,
not on the `termrock-*` crates: their names and boundaries are not part of this
contract.

The facade exports `clock`, `core`, `metrics`, `motion`, `record_work`,
`runtime`, `shell`, `theme`, `ui` and `widgets` always; `cli` with the `cli`
feature; `terminal` with the `terminal` feature; and `catalog` and `docs`,
which are tooling surfaces for Showcase and are not part of an application's
contract. `RenderCtx` is `termrock::ui::ctx::RenderCtx`; `Page`, `PageTypes`,
`Shell` and `App` are in `termrock::shell`. There are no wrapper types.

The prelude holds `ItemKey`, `WidgetId`, `RowSource`, `KeyedRowSource`,
`SourceStamp`, `Actions`, `Flow`, `Invalidate` and `Response`.

## Features

| Facade feature | Enables | Notes |
| --- | --- | --- |
| none (the default is empty) | The modules above that carry no optional dependency. | No `clap`, no `termpane`. |
| `cli` | `termrock::cli`, `clap` | Command-line argument types, including `Motion` as a `clap::ValueEnum`. |
| `terminal` | `termrock::terminal`, `termpane` | The terminal emulator, viewport and session controller. |
| `product` | `terminal` | The shape of a shipped product: no demo code. |
| `work-metrics` | The work counters behind `record_work!` | Opt-in: off unless a build enables it, whatever the profile. See [performance contracts](performance-contracts.md). |

The facade default is empty: it does not inherit the old `junie-tui` defaults
(`cli` and `terminal`).

Cargo unifies the features of one package across a whole build, so a workspace
build with `--all-features` shows nothing about what a single consumer gets.
The consumer fixtures below are separate workspaces with their own lockfiles,
and each runs in its own Cargo invocation and target directory. See the
[Cargo feature reference](https://doc.rust-lang.org/cargo/reference/features.html).

## Ownership and keys

- A record is identified by an `ItemKey`, never by its position. `ItemKey::Index`
  exists for positional convenience constructors; a keyed provider must not
  return it, and `KeyedState::reconcile` rejects a provider that does.
- A collection keeps its cursor, anchor, selection, expansion, edit target and
  pointer press by key in `KeyedState` (`crates/termrock-core/src/keyed.rs`). Indices are
  transient view positions. A consumer does not mirror selection in its own
  index fields.
- A keyed provider implements `KeyedRowSource`: `stamp`, `source_len` and
  `source_key` describe every authoritative record, and the inherited
  `RowSource` methods describe the visible ordered projection. Every visible
  key must be authoritative, and keys must be unique.
- `KeyedVec<T>` is the owned adapter. `ListBox::render_source` and
  `ListBox::on_key_source` borrow any `ListSource` for one call.
- `WidgetId` names a widget instance for focus, hit testing and layers.
  `WidgetId::of` is a constant function of its text; `sub` derives a child.
- Example: [`keyed_list`](../crates/termrock/examples/keyed_list.rs) keeps the
  cursor on its record while the host reorders the rows.

## Source revisions

`SourceStamp { instance, data, view }` is the revision of a provider.
`instance` identifies the provider's lifetime, never a reusable label. `data`
changes when authoritative membership, payload or enablement changes. `view`
changes when the visible order, filter or match metadata changes. A reconcile
reports which of these it saw in `Reconcile` and prunes keys that left. A
consumer must bump `data` or `view` when it changes the provider; an unchanged
stamp is a promise that nothing changed.

## Response ordering

An input handler returns `Response<A>`: a `Flow` (`Ignored` or `Consumed`), an
`Invalidate` (`None`, `Paint` or `Layout`) and the ordered semantic actions the
input produced. `Response::actions` iterates them in the order they were
pushed, and a host handles them in that order. Zero or one action is stored
inline; a second allocates. A host must not rely on one action standing in for
a batch: the fixture `consumer_keyed_modal_preserves_selection` reads the
activation and the selection change of one Enter as two actions, in order.
Example: [`grid_effects`](../crates/termrock/examples/grid_effects.rs) turns the
typed actions of a grid edit into host effects, in order.
`termrock::core::keymap::Actions` is a different, key-map concept.

## Focus and layers

Exactly one keyboard-focus indicator is visible, in the topmost layer. A modal
layer is opened with `LayerSpec::modal(Dismiss::…)`; its `initial_focus` takes
focus, and `restore_focus(true)` returns focus to the widget that had it when
the layer closes, however it closes (result, dismissal or clear). Light
dismissal resolves to the dialog's cancel action; an application never checks
for "click elsewhere". Floating content goes through the overlay layer
(`RenderCtx::overlay`, flushed by `flush_overlays` and `begin_modal`) and never
depends on draw order inside a page.

Example: [`modal_focus`](../crates/termrock/examples/modal_focus.rs) routes keys
to the modal before the page and restores focus when it closes.

## Nested scrolling

A `ScrollView` handles a scroll key only when it has something to scroll, and
answers `None` otherwise. The host offers a key to the innermost view first and
to the next outer view when the answer is `None`, so the inner view consumes
before the outer one. Example:
[`nested_scroll`](../crates/termrock/examples/nested_scroll.rs).

## Edit errors

A fallible change validates before it commits, or leaves the prior state
intact. A provider that breaks the keyed contract is rejected with a
`KeyedSourceError` and the collection keeps its previous state. When a record
disappears under an open edit, reconcile cancels the edit and reports
`edit_cancelled`; a pointer press whose row left is cancelled the same way. A
refused control reports through the refusal channel instead of mutating. Example:
[`validated_input`](../crates/termrock/examples/validated_input.rs) refuses an
invalid value and keeps the one the host saved.

## Render lifetime

A borrowed provider is used for one call (`render_source`, `on_key_source`,
`on_pointer_source`) and is never stored. Layout state captured at render, such
as hit regions and a layout generation, is valid until the next render; a
pointer release completes only on the row its press started on. A consumer
renders from its current state on every frame and holds no widget borrow across
frames.

## Threads and lifecycle

The widget tree, `Shell` and the runtime loop run on one thread. Other threads
reach the application only through the external channel.
`bounded_external_channel(capacity, ExternalPolicy)` takes a queue capacity and
a per-turn budget of messages and time. `ExternalSender::try_send` never blocks:
a producer that finds the queue full or closed gets its message back in
`ExternalSendError`, and decides whether to retry, merge or drop it. A turn of the loop reads at most `RawInputPolicy::max_events` raw
terminal events and hands the application at most `ExternalPolicy::max_messages`
external messages, within `max_duration`, before it checks resize and tick
again, so a flood of either cannot starve input. The bound cannot interrupt one
handler that blocks. The turn is the public `runtime::pump_external(app, policy,
elapsed)`, which the event loop calls and returns an `ExternalTurn` whose
`exhausted` flag says the budget, not an empty queue, ended it. `elapsed` is
the time since the turn began, so a test supplies a fake clock. Example:
[`bounded_messages`](../crates/termrock/examples/bounded_messages.rs) runs
`pump_external` itself over a bounded channel, shows the count budget and the
time budget (on a fake clock, with no sleep) and retries what a full queue
returned.
`TerminalSession` enters raw mode and restores the
terminal on drop and on panic.

## Breaking changes in v0.1 and migration

Version 0.1 is not a compatibility release. A consumer of an older checkout:

1. Replaces `junie_tui::` with `termrock::` and depends on `crates/termrock`.
   The facade names are the old implementation names, so the replacement is
   mechanical. There is no shim: `junie_tui::` paths no longer resolve (#565).
2. Declares the features it needs. The facade default is empty; add `cli`
   and `terminal` explicitly if the code used them through the old defaults.
3. Reads collection selection by key (`ListBox::keyed`) and iterates
   `Response::actions` instead of reading a single outcome.
4. Moves any use of the three demonstration applications to their packages
   (`cargo run -p termrock-showcase`, `-p termrock-tablepro`, `-p termrock-holla`).
   The binary names `showcase`, `tablepro`, `holla` and `jackin-preview` and
   their command lines are unchanged.

5. Stops using the public items that nothing in the library produced or
   consumed (decision 2026-11-17 in [decisions](decisions.md)):

   | Removed | Use instead |
   | --- | --- |
   | `Intent::{Key, Binding, Paste, FocusIn, FocusOut, Layer, Cancel}`, `FocusVia`, `LayerEvent`, `ActionKey` | Handle key input through `core::keymap` and `Response` actions. Keep a host-owned enum for host-level events. |
   | `Intent<'a>` | `Intent` has no lifetime parameter; `PointerEvent::intent` is an `Intent`. |
   | `Phase::Move`, `Phase::SecondaryRelease` | Hover is `PointerState::on_move`. A secondary release arrives as `Phase::Click`. |
   | `Part::{ACTION, BADGE, BODY, BORDER, CHANGE, DETAIL, EMPTY, GUTTER, HELP, LABEL, META, PLACEHOLDER, PREFIX, RULE, SUMMARY, TEXT}` | `Part::custom("name")` for a region of the host's own. The numbers of the remaining parts are unchanged. |
   | `StateFlags` | A host-owned `bitflags` type, or the `VisualState` and `Theme` modifiers the widgets already use. |

6. If it depends on `junie-tui` by git, it must move. The package `junie-tui`
   and its `junie_tui::` shim were removed (#565). Replace `junie-tui` by
   `termrock` (`crates/termrock`, features declared explicitly) and
   `junie_tui::` by `termrock::`; the paths are identical.

## Examples and fixtures that hold this contract

Six runnable examples in [`crates/termrock/examples`](../crates/termrock/examples)
(`cargo run -p termrock --example <name>`, listed in the
[README](../README.md#examples)) show each contract above, and each carries a
`#[test]` for its outcome. The fixtures below hold the feature and dependency
boundaries.

Each fixture is a package with its own `[workspace]`, a committed `Cargo.lock`
and its own tests. `scripts/check-consumers.sh` builds and runs all four in
separate Cargo invocations with separate target directories, using `--locked`,
and prints each normal feature tree.

| Fixture | Features | Proves |
| --- | --- | --- |
| [`crates/termrock/tests/consumers/minimal`](../crates/termrock/tests/consumers/minimal/src/lib.rs) | none | `consumer_minimal_renders_input_and_list`; the examples `consumer_minimal_validated_input_reports_and_clears_errors`, `consumer_minimal_typed_grid_actions_arrive_in_order`, `consumer_minimal_nested_scroll_views_bubble_what_they_cannot_scroll` and `consumer_minimal_bounded_external_messages_return_the_overflow`. The normal tree has neither `clap` nor `termpane`. |
| [`crates/termrock/tests/consumers/keyed-modal`](../crates/termrock/tests/consumers/keyed-modal/src/lib.rs) | none | `consumer_keyed_modal_preserves_selection`: a keyed list borrowed from a provider, under a modal layer, keeps its selected key when the provider is reordered, and focus returns. It is the borrowed-keyed-list and the modal-focus example. |
| [`crates/termrock/tests/consumers/terminal`](../crates/termrock/tests/consumers/terminal/src/lib.rs) | `terminal` | `consumer_terminal_constructs_public_viewport`. No `clap`. |
| [`crates/termrock/tests/consumers/jackin-host`](../crates/termrock/tests/consumers/jackin-host/src/lib.rs) | none, on `jackin-console` | `consumer_jackin_host_resolves_the_host_api`. The host keeps its intentional terminal dependency and resolves no `clap`; `apps/jackin/tests/library_has_no_demo.rs` of `jackin-console` keeps demo names out of the host modules. |

`crates/termrock/tests/regressions/facade_contract.rs` runs in the ordinary test suite: it
scans the four application packages for any manifest entry or Rust path that
names `junie_tui` (`facade_apps_have_no_implementation_imports`), checks that the
shim is gone (`the_junie_tui_shim_is_gone`), and checks the
committed lockfile of each fixture, which holds exactly its normal graph
(`consumer_minimal_excludes_clap_and_termpane`,
`consumer_keyed_modal_excludes_clap_and_termpane`,
`consumer_terminal_has_termpane_without_clap`,
`consumer_jackin_host_excludes_demo`).
