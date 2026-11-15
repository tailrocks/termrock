# Future architecture (refactoring target)

This is the target state the refactoring programme moves toward: a
workspace of focused crates with thin applications, preserving the
observable behaviour and visual language of the current monolith (see
[current state](current-state.md) and [product](product.md)). Nothing
here exists yet; a crate boundary is accepted only when real consumers
use it and the gates below pass.

## Target structure

| Layer | Crates | Owns |
| --- | --- | --- |
| Foundations | `core`, `layout`, `theme`, `text` | IDs, events, intents, validation; measurement/clipping; semantic palettes and glyph policy; grapheme-correct text and editor grammar. |
| Runtime | `runtime`, `terminal`, `session` | Focus, hit testing, key maps, layers, capture, dispatch; terminal capability/input models; session enter/leave/run lifecycle. |
| Components | `controls`, `fields`, `feedback`, `navigation`, `viewport`, `overlays`, `grid`, `editors`, `forms`, `collections` | The reusable component families, each with the shared widget conventions (builders, events, outcomes, one focus gutter, selection ≠ focus). |
| Facade | `termrock` | The public library surface applications depend on. |
| Applications | Showcase, TablePro, Holla, Jackin app crates (domain/sim/UI split per app) | Routes, demo/simulated data, composition. No shared widget behaviour. |
| Proof | conformance and test tooling crates | Component registry/PARTS closure, test inventory, performance counters. |

Dependency direction is one-way: apps → facade → components →
runtime → foundations. An owner crate never depends on application
internals.

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

1. Establish crate ownership and dependency direction.
2. Move one component or runtime contract at a time, with its tests
   and real consumers.
3. Demonstrate it in Showcase and validate full app journeys with the
   functional suite ([functional testing](functional-testing.md)).
4. Qualify visual states, colour levels, portability, performance
   and public API compatibility.

Product parity and architecture completion are independent gates: a
completed crate move does not close behaviour or visual work, and a
passing app journey does not prove a boundary is clean.
