# Layout lane review decisions (#14)

Independent review of `land/14-layout` (356 proofs over 26 scenarios, no
exceptions). The review found four proofs that only checked that a builder
stores a value although the library shows an observable effect
(`Stack::track_click`, `Notice::kind`, `PaneTree::hit_pad`,
`PaneTree::split_keys`) and one (`ToolItem::reason`) that never showed the
reason. The scenarios now observe each effect. The three questions the
review had to decide are recorded here.

## File size

`src/bin/showcase/app_tests_catalog/layout.rs` is about 3,100 lines. No rule
limits file size: `AGENTS.md`, `docs/`, `hk.pkl`, `scripts/` and the clippy
configuration have none, and larger files already exist (`src/catalog.rs`
5,500 lines, `src/bin/showcase/app_tests_catalog.rs` 5,000). The file is not
split. The gate also pins a lane's scenario file to the path
`app_tests_catalog/<lane>.rs` wired by `#[path = ...] mod lane_<lane>;`, and a
submodule directory would need a change to `src/catalog_contract.rs`, which a
lane must not touch.

## StackScroll and the track state after a page click

`StackScroll::pointer` does not call `ScrollState::release_track` on
`Click`, `Release` or `DragEnd`, unlike `widgets::scrollbar::pointer`. A page
click leaves `paging` set and a thumb drag leaves `grab` set until the next
press. No bug is observable:

- Every press on the bar re-initialises both fields (`press_track` clears
  `paging`; the page branch of `press_track_with` clears `grab`; the thumb
  branch sets `grab`, which wins over `paging`).
- A `Drag` reaches `StackScroll` only through the router's held capture, which
  starts with a `Press` on the same bar. A drag cannot start from stale state.
- Nothing else reads `grab` or `paging`: the thumb's pressed colour comes from
  the router's pressed state, not from `ScrollState`.

A throwaway test replayed 512 press and drag sequences (both `TrackClick`
policies) with and without `release_track` between presses and found no
difference in the resulting offsets, so there is no failing-first test to
write. The duplicate of the shared bar routing stays a candidate for a later
clean-up (route `StackScroll::pointer` through `scrollbar::pointer`).

## ChipBar overflow tooltip

The hover tip on the `+N` marker is drawn by the shared tooltip overlay
(`ui::tooltip`, placed by `popup::place_with`, below the anchor and flipped
above when there is no room below). Rendering a bar at the top, middle and
bottom row of a taller screen shows the tip on the row next to the marker and
never over it. It covers the marker only when the screen has no spare row at
all (a one row screen), which is true of every tooltip in the library and has
no better placement. The tip stays dismissible with Esc and hoverable, so it
meets WCAG 2.2 content on hover. It is not a defect and needs no library
change.
