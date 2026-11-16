# API contract

This is the supported surface of Termrock for a consumer. It states what a
consumer may rely on and what it must not. Rules that a test holds name
the test; the others name the code.

Termrock is not published to crates.io. Depend on it by path or by git:

```toml
[dependencies]
termrock = { git = "https://github.com/tailrocks/terminal-components-baseline", default-features = false }
# or, in a checkout:
# termrock = { path = "../terminal-components-baseline/crates/termrock", default-features = false }
```

## Packages and imports

| Package | Import | Role |
| --- | --- | --- |
| `termrock` (`crates/termrock`) | `termrock::` | The supported import path. A re-export facade with a small prelude. |
| `junie-tui` (repository root) | `junie_tui::` | The implementation package. It keeps its name during this migration. Existing consumers keep working. |
| `termrock-showcase`, `termrock-tablepro`, `termrock-holla` (`apps/*`) | none | Applications. They import `termrock` only. |
| `jackin-console` (`apps/jackin`) | `jackin_console::` | The Jackin host library and its `jackin-preview` demo. It imports `termrock` only. |

Every module of the facade is a `pub use` of the implementation module, so a
type has one identity under both paths. `facade_basic_types_preserve_identity`
in `crates/termrock/tests/regressions/type_identity.rs` passes a facade
`Response`, `ItemKey`, `WidgetId` and `RowSource` to functions typed against
`junie_tui`. A consumer that mixes both paths gets no conversion and no second
type.

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
| `product` | `terminal` and the implementation's `product` | The shape of a shipped product: no demo code. |
| `work-metrics` | The work counters behind `record_work!` | Opt-in: off unless a build enables it, whatever the profile. See [performance contracts](performance-contracts.md). |

The implementation package keeps its old defaults (`cli` and `terminal`) so
that an existing `junie-tui` consumer does not change silently. The facade
does not inherit them.

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
  pointer press by key in `KeyedState` (`src/core/keyed.rs`). Indices are
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

## Edit errors

A fallible change validates before it commits, or leaves the prior state
intact. A provider that breaks the keyed contract is rejected with a
`KeyedSourceError` and the collection keeps its previous state. When a record
disappears under an open edit, reconcile cancels the edit and reports
`edit_cancelled`; a pointer press whose row left is cancelled the same way. A
refused control reports through the refusal channel instead of mutating.

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
handler that blocks. `TerminalSession` enters raw mode and restores the
terminal on drop and on panic.

## Breaking changes in v0.1 and migration

Version 0.1 is not a compatibility release. A consumer of an older checkout:

1. Replaces `junie_tui::` with `termrock::` and depends on `crates/termrock`.
   The facade names are the implementation names, so the replacement is
   mechanical. The old path keeps compiling.
2. Declares the features it needs. The facade default is empty; add `cli`
   and `terminal` explicitly if the code used them through the old defaults.
3. Reads collection selection by key (`ListBox::keyed`) and iterates
   `Response::actions` instead of reading a single outcome.
4. Moves any use of the three demonstration applications to their packages
   (`cargo run -p termrock-showcase`, `-p termrock-tablepro`, `-p termrock-holla`).
   The binary names `showcase`, `tablepro`, `holla` and `jackin-preview` and
   their command lines are unchanged.

A rename or removal of the implementation names is a separate, versioned
decision.

## Fixtures that hold this contract

Each fixture is a package with its own `[workspace]`, a committed `Cargo.lock`
and its own tests. `scripts/check-consumers.sh` builds and runs all four in
separate Cargo invocations with separate target directories, using `--locked`,
and prints each normal feature tree.

| Fixture | Features | Proves |
| --- | --- | --- |
| [`tests/consumers/minimal`](../tests/consumers/minimal/src/lib.rs) | none | `consumer_minimal_renders_input_and_list`; the examples `consumer_minimal_validated_input_reports_and_clears_errors`, `consumer_minimal_typed_grid_actions_arrive_in_order`, `consumer_minimal_nested_scroll_views_bubble_what_they_cannot_scroll` and `consumer_minimal_bounded_external_messages_return_the_overflow`. The normal tree has neither `clap` nor `termpane`. |
| [`tests/consumers/keyed-modal`](../tests/consumers/keyed-modal/src/lib.rs) | none | `consumer_keyed_modal_preserves_selection`: a keyed list borrowed from a provider, under a modal layer, keeps its selected key when the provider is reordered, and focus returns. It is the borrowed-keyed-list and the modal-focus example. |
| [`tests/consumers/terminal`](../tests/consumers/terminal/src/lib.rs) | `terminal` | `consumer_terminal_constructs_public_viewport`. No `clap`. |
| [`tests/consumers/jackin-host`](../tests/consumers/jackin-host/src/lib.rs) | none, on `jackin-console` | `consumer_jackin_host_resolves_the_host_api`. The host keeps its intentional terminal dependency and resolves no `clap`; `tests/library_has_no_demo.rs` of `jackin-console` keeps demo names out of the host modules. |

`tests/regressions/facade_contract.rs` runs in the ordinary test suite: it
scans the four application packages for any manifest entry or Rust path that
names `junie_tui` (`facade_apps_have_no_implementation_imports`) and checks the
committed lockfile of each fixture, which holds exactly its normal graph
(`consumer_minimal_excludes_clap_and_termpane`,
`consumer_keyed_modal_excludes_clap_and_termpane`,
`consumer_terminal_has_termpane_without_clap`,
`consumer_jackin_host_excludes_demo`).
