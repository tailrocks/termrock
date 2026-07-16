# Picker design spike

> **Graduated:** `Picker`, `PickerState`, and `PickerOutcome` now ship from
> `termrock::widgets`. The lookbook consumes that public implementation; three
> catalog stories, contract axes, docs, previews, tests, and API inventory ship
> with it. The local prototype was removed.

> Ownership boundary: callers retain filtering, matching, scoring, ordering,
> candidate lifecycle, and labels. Picker owns query mechanics, selection
> reconciliation, default layout, empty state, and semantic outcomes.

## Verdict

Graduate both `PickerState<Id>` and a strongly defaulted `Picker` widget into
`termrock::widgets::picker`. The local prototype removes repeated event routing
and selection repair while leaving the simple `contains` projection visibly in
the lookbook consumer. State-only would preserve too much layout glue; a
widget-only API could not express caller-owned filtering cleanly.

## Normative contract

```rust
pub struct PickerState<Id> {
    pub query: TextInputState,
    pub list: ListState<Id>,
    // private previous selectable ID order for index fallback
}

impl<Id: Clone + PartialEq> PickerState<Id> {
    pub fn handle_key(
        &mut self,
        visible: &[ListRow<'_, Id>],
        key: KeyEvent,
    ) -> PickerOutcome<Id>;
    pub fn query_text(&self) -> &str;
    pub fn reconcile(&mut self, visible: &[ListRow<'_, Id>]);
}

pub enum PickerOutcome<Id> {
    Ignored,
    QueryChanged,
    SelectionChanged,
    Activated(Id),
    Cancelled,
}
```

The caller computes `visible` for the current query and order. After
`QueryChanged`, it recomputes that projection and calls `reconcile` before
rendering or sending another list action.

### Key routing

| Key | Owner | Outcome |
|---|---|---|
| printable character, Backspace, Delete | query | `QueryChanged` when edited |
| Left, Right, Home, End | query cursor | `QueryChanged` only when state changes |
| Up, Down, PageUp, PageDown | list | `SelectionChanged` when moved |
| Enter | selected visible row | `Activated(Id)`; `Ignored` when empty |
| Escape with non-empty query | picker | clears query, `QueryChanged` |
| Escape with empty query | picker | `Cancelled` |
| release/unknown/modified unrelated key | neither | `Ignored` |

Application-global printable shortcuts must yield while the picker edits text;
the lookbook therefore uses `Ctrl+t` for its theme switch while Picker is
active and keeps plain `t` as input.

There is no query/list focus toggle. Navigation keys and editing keys have
disjoint ownership, matching command-palette muscle memory without requiring a
new event concept. Dialog hosts consume `Cancelled`; the lookbook lets the
first Escape clear and the second return to its story list.

### Selection stability

Reconciliation considers enabled `RowRole::Item` rows only:

1. Empty visible set clears selection.
2. If the selected stable ID survives, keep it regardless of reorder.
3. Otherwise, find its index in the previous selectable projection and select
   `min(old_index, new_len - 1)` in the new projection.
4. With no prior selected index, select the first selectable row.

This is ID-sticky, then index-fallback. It avoids jumping to the top when a
middle result disappears while remaining deterministic. Disabled rows and
separators never become fallbacks.

The default widget renders a one-row `TextInput`, then a `List` filling the
remaining area. Empty results render the product-neutral cue `No matches`;
the builder may override that text. Count/chrome are optional builder details,
not required state. A modal palette composes `Backdrop` + `Dialog` + `Picker`;
Picker owns no overlay or dismissal policy.

## Graduation findings

The `picker/basic` lookbook story now uses the public `PickerState`, a
caller-owned case-insensitive `contains` projection, four stable IDs, and the
default query/list layout. Library reconciliation/routing tests cover surviving
IDs, filtered-out fallback, tail clamp, empty results, query edits, arrows,
two-stage Escape, activation, modified/release events, Unicode, tiny areas, and
painted pointer geometry. The catalog replaces the frozen one-line filter story
with the actual composition.

No private `ListState` access was needed: its semantic selection, routing,
scroll, hover, and click methods supply the seam. `TextInputState::clear`
preserves configured validation and maximum length, removing the prototype's
state-reconstruction workaround.

## Completed library build

1. Added `widgets/picker.rs` with public `Picker`, `PickerState`, and
   `PickerOutcome`; export once from `widgets`.
2. Added `TextInputState::clear` and use it for
   two-stage Escape.
3. Tested routing, ID-sticky/index-fallback reconciliation, disabled/separator
   rows, Unicode queries, empty/tiny areas, and mouse behavior delegated to
   `ListState`.
4. Added `picker/basic`, `picker/empty`, and `picker/narrow-unicode` stories,
   deterministic previews, API inventory, component contract axes, and docs in
   the same change. Update the existing filter story rather than retain two
   parallel compositions.
5. This is additive, so no migration file is needed. Picker now owns the former
   filter-composition catalog identity; TextInput keeps its Unicode story.

## Deferred options

- Multi-select can later delegate to `Selection<Id>`, but needs evidence for
  whether Enter activates or confirms the set and should follow Plan 037's
  aligned outcome shape.
- A telescope-style preview pane belongs to caller composition until multiple
  consumers demonstrate shared focus/layout behavior.
- Async/streamed candidates remain caller-owned. Reconcile each stable snapshot;
  Picker must not own tasks, debouncing, cancellation, or loading policy.
- Fuzzy scoring is permanently outside the component. A future helper library
  may project/scored rows, but Picker accepts only caller-ordered visible rows.
