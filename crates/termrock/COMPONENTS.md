# TermRock component inventory

The public widget set is `ActionBar`, `Backdrop`, `ChoiceDialog`, `DetailTable`, `Dialog`, `DiffView`, `HintBar`, `List`, `MessageDialog`, `Panel`, `StatusBar`, `Tabs`, `TextInput`, `Toast`, and `Tree`.

`Tree` consumes a caller-flattened borrowed node projection with stable IDs,
depth, disclosure, enabled, and loading/error facts. `TreeState` owns only
focus, selection, hover, viewport offset, and painted row/disclosure/scrollbar
hit regions. Keyboard, wheel, click, and scrollbar-position methods return or
apply semantic selection/toggle/activation/scroll outcomes; callers retain
hierarchy, filtering, lazy loading, and expansion policy.

The `tree_hot_path` evidence renders a warmed 40-row viewport over 10,000
borrowed nodes 100 times in the Cargo test/debug profile, asserts bounded
painted regions, rejects allocator or reallocator calls, and enforces a 250 ms
batch budget (2.5 ms/render). The 2026-07-16 baseline was 45.09 ms on an Apple
M1 Max with 64 GiB, macOS 26.5.2, and Rust 1.97.0. The 250 ms threshold is the
cross-run/CI tolerance; a slower result blocks the component gate until measured
and deliberately revised with new environment evidence.

Every component uses borrowed render data and stable IDs where interaction identity is required. Consumers own labels, validation, filtering, lifecycle, output, and domain models. Canonical neutral stories and SVG previews are maintained by `termrock-lookbook`; the catalog coverage check binds story IDs to documentation and preview files.
