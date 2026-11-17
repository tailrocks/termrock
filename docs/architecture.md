# Architecture and layering

The repository is moving from one package to a workspace of focused
`termrock-*` crates, preserving the observable behaviour, the visual language
and the `termrock::` public paths of the current code (see
[current state](current-state.md) and [product](product.md)). Epic #439 tracks
the split. A crate boundary is accepted only when real consumers use it and the
gates below pass.

## Where the code is today

One package holds all library code: `junie-tui`, the repository root
(`Cargo.toml`, library `junie_tui`, `src/`). Its modules are `core`, `theme`,
`ui`, `widgets`, `shell`, `runtime` (with `clock`), `terminal`, `cli`,
`metrics`, `motion`, `catalog` and `docs`. `crates/termrock` is the facade the
four applications import. `apps/showcase`, `apps/tablepro`, `apps/holla` and
`apps/jackin` are the applications. No other library crate exists yet.

Because everything is one crate, the compiler allows any module to use any
other, and `ui` and `widgets` form one cycle. [`docs/layering.toml`](layering.toml)
and the `layering` regression test make the target direction a checked
artifact first, so the split cannot regress while it is in flight and cannot
drift afterwards.

## Target crates

Every row is a package under `crates/`. Names are the real ones the programme
creates; "Built from" names the current source. The exact crate each source
file will land in is `[files]` in `docs/layering.toml`.

| Layer | Crate | Owns | Built from (today) |
| --- | --- | --- | --- |
| Foundations | `termrock-metrics` | Work counters, `Metric`, `record_work!`, measurement. | `src/metrics.rs`, `src/metrics/{measurement,state}.rs` |
| Foundations | `termrock-text` | Cell width, truncate, wrap, `TextBuffer`, text index. | `ui/text.rs`, `core/text.rs`, `core/text_index.rs` |
| Foundations | `termrock-core` | IDs, events, intents, response, hit testing, focus, key maps, list and keyed sources, scroll state, selection, edit grammar, motion, line-edit substrate. | `src/core/*`, `motion.rs`, `widgets/{edit,field_common,text_select}.rs` |
| Foundations | `termrock-theme` | `Theme`, palettes, glyphs, `VisualState`, `CardHeading`. | `src/theme.rs` |
| Runtime | `termrock-runtime` | `RenderCtx`, reveal, tooltip, popup, measure, row, fade, specimen trait, column solver. | `ui/{ctx,popup,reveal,tooltip,measure,row,fade,fitting,columns,specimen}.rs` |
| Runtime | `termrock-session` | Run loop, `TerminalSession`, external channel, panic bridge, clock. | `src/runtime.rs`, `src/runtime/*`, `clock.rs` |
| Components | `termrock-controls` | Key hints, hint bar, key reference, button, icon button, choice, action row, brand, scrollbar, segments. | `widgets/{keyhint,hintbar,keyref,button,icon_button,choice,action_row,brand,scrollbar,segments,widget_hints}.rs` |
| Components | `termrock-feedback` | Progress, notice, empty, too-small, meter list, status bar, page header. | `widgets/{progress,notice,empty,too_small,meter_list,statusbar,page_header}.rs` |
| Components | `termrock-overlays` | Layer stack, modal, menu, option list, completion, row menu, context menu, edit menu, option editor, picker. | `ui/{layer,modal,context,edit_menu}.rs`, `widgets/{menu,option_list,completion,row_menu,option_editor,picker}.rs` |
| Components | `termrock-viewport` | Viewport and its operations, panel, scroll view. | `ui/scroll_view.rs`, `widgets/{viewport,viewport_ops,panel}.rs` |
| Components | `termrock-fields` | Input, select, chips. | `widgets/{input,select,chips}.rs` |
| Components | `termrock-editors` | Text area, code editor, diff. | `widgets/{textarea,code,diff}.rs` |
| Components | `termrock-layout` | Split and stack layout, splitter, master-detail, pane tree. | `ui/{layout,master_detail,pane_tree}.rs`, `widgets/splitter.rs` |
| Components | `termrock-collections` | List, tree, props, list core, content, list-detail, search-detail, detail card. | `ui/{content,list_core}.rs`, `widgets/{list,tree,props,list_detail,search_detail,detail_card}.rs` |
| Components | `termrock-navigation` | Breadcrumb, rail, nav list, tabs, toolbar, steps, document workspace. | `widgets/{breadcrumb,rail,nav_list,tabs,toolbar,steps,document_workspace}.rs` |
| Components | `termrock-grid` | Table, grid, grid changes and submission. | `widgets/{table,grid,grid_changes,grid_submission}.rs` |
| Components | `termrock-dialogs` | Dialog, choice and info dialogs, help, form dialog, file browser, inspect changes, leave guard, `LayerResult`. | `ui/{layers,leave}.rs`, `widgets/{dialog,choice_dialog,info_dialog,help,form_dialog,file_browser,inspect_changes}.rs` |
| Application host | `termrock-shell` | `Shell`, `App`, `Page`, router, chrome, footer, clipboard, reach harness. | `src/shell/*` |
| Optional | `termrock-terminal` | Terminal emulator pane, viewport, session controller (feature `terminal`). | `src/terminal.rs`, `src/terminal/*` |
| Optional | `termrock-cli` | `clap` argument types (feature `cli`). | `src/cli.rs` |
| Proof | `termrock-catalog` | Declared-API catalog, docs tables, inventory gate (#14). | `catalog.rs`, `docs.rs`, `catalog_contract.rs`, `tests/support/api_inventory.rs` |
| Proof | `termrock-conformance` | Work-metrics harness and report, allocation counter, cross-crate regression tests. | `metrics/{harness,report,tests}.rs`, `test_alloc.rs`, the cross-module `*_tests.rs` files |
| Facade | `termrock` | The public surface applications depend on: `pub use` of the crates above. | `crates/termrock` |
| Applications | `apps/*` | Routes, demo or simulated data, composition. They import `termrock::` only and hold no shared widget behaviour. | `apps/showcase`, `apps/tablepro`, `apps/holla`, `apps/jackin` |

`termrock-functional` (the PTY journeys of `tests/functional`) and
`functional-cross` are test-only crates; nothing depends on them. `junie-tui`
stays as a transitional deprecated shim once the split ends (#452).

## Dependency direction

Dependency direction is one-way: apps, then the facade, then components, then
runtime, then foundations. An owner crate never depends on application
internals, and no library crate names a jackin, showcase, tablepro or holla
concept.

[`docs/layering.toml`](layering.toml) is the machine-readable table:

- `order` lists the crates bottom to top.
- `[allow]` lists the direct dependencies each crate may declare. A crate also
  may use whatever its dependencies already use.
- `[layers]` puts each crate in a layer; a dependency may not point to a higher
  one.
- `[files]` maps each source file of the single package to the crate that will
  own it. It exists only until #452.

The `layering` test (`tests/regressions/layering.rs`) enforces it:

1. Stage A, modules. It parses every `.rs` file under `src/`, resolves each
   `crate::`, `self::` and `super::` path to the module that defines it, maps
   both ends to crates through `[files]` and fails on an edge `[allow]` does
   not permit. The message names the file, the importing crate, the imported
   crate and the fix: move the item down or invert the dependency; do not edit
   the allowlist.
2. Stage B, crates. It reads `cargo metadata` and checks every `termrock-*`
   package's normal and dev dependencies against `[allow]`, rejects cycles,
   rejects a `termrock-*` package missing from the table, and requires each
   application to depend on `termrock` and on no library crate. It stays off
   until the first `crates/termrock-*` directory exists (#444).

The dependencies that already point up are recorded in
`tests/regressions/layering-allowlist.tsv`. It may only shrink: #441 removes
the `core` and `theme` rows, #442 the `ui` and `widgets` rows and the
`ui::layers` test rows, and #449 the `test_alloc` rows. A line that no longer
matches a dependency fails as stale. This check complements `tests/regressions/facade_contract.rs`
(applications import the facade only) and does not replace it.

### Extending the check

An extraction PR moves files with `git mv`, deletes the `[files]` entries of the
files it moved (they now belong to a crate of their own) and adds the crate to
`order`, `layers` and `allow` if the row is not there yet. While the shrinking
package keeps a transitional `pub use termrock_x::module as name;`, stage A
attributes a path through that shim to `termrock-x`.

## Where the target differs from the earlier plan

The earlier version of this document grouped crates as `layout` in
Foundations, `terminal` in the runtime layer, one `overlays` crate, `forms`,
and a `runtime` crate that owned focus, hit testing and key maps. The measured module graph (#439) does not
support those boundaries, so the crates above differ in four ways (`terminal`
is now an optional crate above the components it draws with). The reasons
are recorded in [decisions](decisions.md).

1. Layout is a component-layer crate. `Split` uses `scrollbar` and `keyhint`,
   and `Measure` takes `RenderCtx`, so `termrock-layout` sits above
   `termrock-controls` and `termrock-runtime`.
2. Overlays are split into `termrock-overlays` (menus, popups, layers) and
   `termrock-dialogs` (dialogs, forms, file browser). `input` needs a context
   menu and the dialogs need `input`, so one crate would be a cycle. There is
   no `forms` crate; `form_dialog` lives in `termrock-dialogs`.
3. Session and runtime are separate. `termrock-session` holds the run loop and
   terminal lifecycle; `termrock-runtime` holds `RenderCtx` and its plumbing.
   The facade module `termrock::runtime` therefore comes from the crate named
   `session`.
4. Focus, hit testing and key maps stay in `termrock-core`, not in runtime.
   `list`, `keyed` and `intent` depend on them, and `core::{hit, intent,
   pointer, scroll, shape}` and `core::{keymap, list}` form a five-module cycle
   that only one crate can hold.

## Ownership contracts (must survive the move)

- Fallible state changes validate before commit, or leave prior state
  intact.
- Selection, cursor and focus identity stay stable across sort,
  filter, resize and data replacement while the item remains present.
- Disabled controls reject interaction; read-only controls stay
  navigable and visually distinct.
- Text editing and clipboard respect grapheme boundaries; modal input
  routes before background content; popup dismissal resolves to the
  configured cancel action via the popup's own backdrop.
- Empty/unavailable content renders deterministically and stays
  keyboard-reachable.
- A scrollbar `PointerEvent` carries the pointer position and the
  full `Bar` track in the owner's coordinate space, however deeply the
  owner is nested or clipped. `HitRegistry::bar` and hit testing stay in
  screen space, and a press freezes its screen-to-owner transform for
  the whole gesture, so drag and release stay valid after the bar
  leaves the visible window.
- `ScrollView` has two paint paths with different costs. `render`/`begin`/`end`
  stay the arbitrary-composition path: when the content overflows they
  allocate, fill and copy a canvas as large as the content on every frame
  (`ScrollView::last_work` reports the cells), because the callback can
  register hidden focus stops and position popups anywhere in it.
  `ScrollView::render_window` is the virtual path for rows a model can
  supply on demand. The callback runs once, paints in screen coordinates
  into a scratch buffer the size of the viewport, and its hits, autoscroll
  areas, cursor and cuts are clipped to the viewport; nothing is
  translated afterwards, so nested windows apply one projection. The host
  passes the root `screen` unchanged through nested windows and renders
  popups against the real buffer after the body returns. Offscreen rows
  have no focus stops: the model supplies a `focus_target` in padded-canvas
  coordinates and a container focus stop. The legacy consumers (Showcase
  `pages/template.rs`, TablePro `tabs.rs` result-facts and detail views,
  Holla `screens/review.rs` and `screens/snapshot.rs`, Jackin settings
  `agents_view`) compose arbitrary children with hidden
  focus stops and popups, so they stay on the legacy path and keep its cost.
- Public API behaviour stays compatible unless a deliberate migration
  updates consumers and records the decision in
  [decisions](decisions.md).

## Programme (distilled from the 73-task plan)

The retired per-task bundles are summarised here; the bundles
themselves were deleted (see
[audit](audits/2026-11-01-refactoring-tasks-retirement.md)).

| Wave | Scope | Tasks |
| --- | --- | --- |
| B — Baseline & proof | Comparator/host core, oracle capture per app and per component family, test inventory and disposition, runner/accounting/architecture verifiers, conformance foundation. | TASK-001–008, 070–073 |
| C — Core library | Session, runtime, theme, layout, text, scroll, rows, fields, layers, choices/forms, collections, output, grid, dialogs/menus, pickers, code, diff, chrome, status, motion, discovery, conformance closure. | TASK-009–031 |
| S — Showcase | Shell and all routes live on shared components; fields/forms, data grids, settings, output, collections, overlays; oracle closure. | TASK-032–039 |
| H — Holla | Shell/finder/files/review gates/activity/disk/cleanup/build tasks/services/config; 34-world oracle closure. | TASK-040–050 |
| J — Jackin | Rituals/manager, config forms, accounts/secrets, launch cockpit, capsule panes, inspect overlays; world/motion closure. | TASK-051–057 |
| T — TablePro | Connections, workbench, data grid, query editor, safety overlays, history/help; identity closure. | TASK-058–064 |
| X — Closure | Duplicate-painter rejection, test preservation/relocation, performance bounds, public API/docs, final integration and merge readiness. | TASK-065–069 |

## Migration gates

1. Establish crate ownership and dependency direction (`docs/layering.toml`
   and the `layering` test; #440).
2. Move one component or runtime contract at a time, with its tests
   and real consumers.
3. Demonstrate it in Showcase and validate full app journeys with the
   functional suite ([functional testing](functional-testing.md)).
4. Qualify visual states, colour levels, portability, performance
   and public API compatibility.

Product parity and architecture completion are independent gates: a
completed crate move does not close behaviour or visual work, and a
passing app journey does not prove a boundary is clean.
