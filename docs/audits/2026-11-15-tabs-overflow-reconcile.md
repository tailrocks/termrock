# Tabs overflow menu reconciles with the tab collection (2026-11-15)

Issue #44. The overflow menu of `widgets::tabs::Tabs` took the tab indices
it showed when it opened. A structural change between opening and choosing
moved those indices, so a choice could activate a different tab, and an
index past the end could be emitted as `Activated(i)`.

## Decision

Every successful structural change closes an open overflow menu, and the
menu does not reconcile positions. The changes are `set_items` with a
different vector, `push`, a valid `remove`, a valid `reorder`, and an
`update_item` that changes a tab's metadata. The menu shows that metadata,
so a label or flag change closes it too. Invalid indices and equal
replacements change nothing and keep the menu.

Until the next render, the strip ignores pointer hits from the old frame
for tabs, close buttons, More, the scroll arrows and middle-click. Keyboard
navigation still works on the current collection.

## Relation to other work

Keyed collection identity (#41) and host workspace identity (#52, #53) are
separate. Closing a stale menu does not identify a host document.

## Evidence

`tests/tabs_overflow_mutation.rs` (the `issue44_` tests). On the code before
this change, the four tests that compile against the old API fail at their
first assertion that a structural change closes the menu. Tests that call
`update_item`, `reorder`, `push` or `items()` cannot compile there.
