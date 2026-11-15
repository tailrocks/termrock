# Guard TablePro sort, filter, refresh and preview replacement (#12)

`TableTab::load` always called `DataGrid::set_rows`, which resets the draft, the
undo history, the selection and any editor. Header sort and Clear filters reloaded
at once, the filter editor, chips, Open Filtered and the plain discard did the
same, and Refresh asked only when it went through a different path than the rest.
`open_table` replaced the first preview tab without testing it for a draft. A
sort, a filter change or the next preview therefore dropped unsaved edits with no
question and no way back. On a populated tab a failed query also replaced the
rows with an empty set.

## Decision

- **One request type.** A gesture builds a `TableChange`: the table's never-reused
  tab key, the `view_revision` it was made against, the complete next
  `TableViewState` (sort, filters, match mode) and a reason (`Refresh`, `Sort`,
  `Filters`). It never carries a position to replay. `Act::TableChange` hands it to
  the app, which stamps the connection generation and calls
  `request_table_transition`. Keyboard and pointer chips share
  `TableChange::from_chip`.
- **One gate.** The gate resolves the table by key, generation and view revision,
  refuses a saving table, finishes a live editor once (a rejected input stops the
  request with the input kept), treats an equal non-Refresh view as a no-op, then
  either applies a clean change or opens the library `LeaveGuard` with
  `Stakes::Unsaved`. A second request while one waits is refused with a status.
  Plain discard keeps its approved dialog and goes through the same gate.
- **Read, then replace.** `TableTab::prepare_load` reads the rows for a view and
  touches nothing. `install_load` replaces the rows once through the grid's checked
  setter (which refuses while a save owns them), then moves the sort marker, filter
  marks, chips and view together and counts one view revision. A failed read or a
  refused install leaves the draft and the shown controls as they were. The
  replacement itself drops the draft, so there is no separate discard.
- **Save continues, exactly.** Choosing Save reviews that table's own save. The
  transition waits on that save's `GridCommitToken`. It runs only when that token
  completes; a cancelled review, a refused save, a failed receipt or a reconnect
  drops it. After a successful save, a failed reload keeps the saved, clean rows
  and the old controls and says so.
- **The filter editor keeps its owner.** Opening it captures the tab key,
  connection generation and view revision. Its answer is rejected when any of them
  changed, and applies to that table when they did not, whichever tab is active.
- **Previews.** The first edit that leaves a draft ends the preview state for
  good. `open_table` replaces only a clean preview with no open editor and no save,
  and promotes any other preview before opening the new one beside it.
- **Showcase.** The Grid page's Reload asks the library `LeaveGuard` before it
  drops a draft, with an app-owned `SampleTransition`.

## Behavior changes

- Sort, filter, refresh and open-filtered on a table with unsaved edits now ask
  first (Unsaved changes: Cancel, Discard, Save). Before, they dropped the edits.
- A failed read no longer empties a populated grid.
- A schema switch with a hidden explorer shows it, since it focuses it.
- Discard on a table with unsaved edits still opens the plain discard dialog.
  Discard on a clean table no longer asks. It reloads the rows at once, where it
  used to ask with "0 pending". Every approved capture is unchanged.

## Limits

The simulator has one data source, so `prepare_load` reads the in-memory catalog.
When #51 adds the session store, `prepare_load` reads it and keeps the same
read-then-install contract. A guard left open by a layer that closes without an
answer is cleared when the connection ends.
