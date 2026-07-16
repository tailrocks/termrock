# TermRock component inventory

The public widget set is derived from the reviewed API report and currently contains `ActionBar`, `Backdrop`, `ChoiceDialog`, `DetailTable`, `Dialog`, `DiffView`, `Form`, `HintBar`, `List`, `MessageDialog`, `Panel`, `SplitPane`, `StatusBar`, `Tabs`, `TextInput`, `Toast`, `Tree`, and `Viewport`.

With the optional `crossterm` feature, `Session` is the sole terminal lifecycle
owner. Its forward default acquires raw mode, alternate screen, mouse capture,
bracketed paste, disabled line wrapping, and hidden cursor state. Failed entry
rolls back every acquired mode; explicit restore and `Drop` restore in reverse
order and remain idempotent. Cursor hiding and line-wrap disabling are
independent options that default on; inline integrations may enable either
without owning the alternate screen, while alternate-screen integrations may
disable either. Screens and widgets never emit lifecycle commands.

`SplitPane` maps an integer remembered ratio, horizontal/vertical direction,
and caller minimums into bounded first/divider/second rectangles. Tiny areas
degrade proportionally without escaping the input rectangle. `SplitPaneState`
owns ratio, divider focus/hover/drag, collapse side, and last painted geometry;
render alone publishes direction-tagged pointer hit geometry. Keyboard resize
and pointer methods emit semantic ratio/focus outcomes; explicit
`collapse`/`expand` methods preserve the remembered ratio. The caller maps
collapse bindings and owns pane content, persistence, focus routing, and
collapse policy.

`Form` consumes caller-owned borrowed sections and stable-ID fields. It renders
required, disabled, help, and validation-error states in responsive one/two
column layouts. `FormState` owns only active focus, hover, viewport offset,
column count, and painted field/scrollbar geometry. Partially clipped fields
retain a union hit region plus optional visible label/value/support subregions.
Required and disabled states reserve the neutral non-color markers `*` and `⊘`.
Keyboard, click, wheel, and scrollbar-position methods expose semantic
focus/activation or bounded scroll; callers retain field values, wording,
editing, validation, submission, and lifecycle.

`Tree` consumes a caller-flattened borrowed node projection with stable IDs,
depth, disclosure, enabled, and loading/error facts. `TreeState` owns only
focus, selection, hover, viewport offset, and painted row/disclosure/scrollbar
hit regions. Keyboard, wheel, click, and scrollbar-position methods return or
apply semantic selection/toggle/activation/scroll outcomes; callers retain
hierarchy, filtering, lazy loading, and expansion policy.

`List` owns selection, hover, focus, viewport offset, painted regions, and a
reserved proportional-scrollbar gutter. Stable-ID rows use the general
navigation contract. Index-addressed pickers use the `ListState<usize>` count,
wrap-navigation, bounded-gesture, reconciliation, and selected-item methods so
consumers do not retain a second list-state crate or generic picker helpers.

The `tree_hot_path` evidence renders a warmed 40-row viewport over 10,000
borrowed nodes 100 times in the Cargo test/debug profile, asserts bounded
painted regions, rejects allocator or reallocator calls, and enforces a 250 ms
batch budget (2.5 ms/render). The 2026-07-16 baseline was 45.09 ms on an Apple
M1 Max with 64 GiB, macOS 26.5.2, and Rust 1.97.0. The 250 ms threshold is the
cross-run/CI tolerance; a slower result blocks the component gate until measured
and deliberately revised with new environment evidence.

Every component uses borrowed render data and stable IDs where interaction identity is required. Consumers own labels, validation, filtering, lifecycle, output, and domain models. Canonical neutral stories and SVG previews are maintained by `termrock-lookbook`; the catalog coverage check derives the component inventory from `docs/api/public-api.txt`, requires at least one typed story, documented story ID, and deterministic preview per public widget, and requires an exact keyboard/mouse/focus/non-color/Unicode/narrow-terminal classification in `docs/api/component-contracts.json`.
