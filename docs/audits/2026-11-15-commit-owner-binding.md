# Bind saves to stable owners and the exact reviewed changes (#11)

TablePro stored a save as `committing: Option<u32>`, a countdown with no owner.
`finish_commit` then read `active_tab` and regenerated the SQL from that tab's
current draft. Switching tabs during the countdown saved the wrong table, an edit
made after the review joined the save, and a reconnect let the old timer commit a
new connection's draft. The Showcase Grid page had the same hole: `commit_ticks`
reviewed current strings, waited four ticks and read whatever pending cells
existed, and `set_option` replaced the grid while the timer ran. A tab id derived
from `tabs.len() + query_counter + 1000` could repeat after a close.

## Decision

- **One lock in the grid.** `DataGrid::begin_submission(snapshot)` accepts only the
  grid's own current capture: same `GridInstanceId`, same revision, and a
  bit-exact match of `snapshot_changes()` through `GridChangeSet::same_capture`
  (a `NaN` payload does not reject spuriously). It refuses an open text or choice
  session with `Editing` and never cancels or commits it; the host calls
  `finish_edit_session` first. It returns a `GridCommitToken` (origin, nonce,
  revision) and stores the snapshot privately.
- **Busy everywhere.** While locked, `record_cell`, `try_edit_text`,
  `toggle_delete`, `insert_row`, `duplicate_row`, `undo`, `discard`, `set_rows`,
  `append_rows`, `try_set_rows_keyed` and `try_set_model_keyed` refuse with
  `GridEditError::Busy` or `GridDataError::Busy` before changing anything. Key,
  context and pointer paths turn the refusal into the existing
  `GridAction::ReadOnly` with its reason, never a data-change action. A remote
  sort request is refused too, because the host would reload. Navigation,
  selection and copy still work. `read_only_reason_at_cursor` and
  `cell_editable` report the lock, and the context menu disables the mutating
  commands.
- **Receipt validated before it applies.** `validate_submission_receipt` checks
  origin, revision, exact coverage of inserted and updated keys, duplicates,
  schema width, unchanged keys for updates, committed-key collisions and revision
  headroom, runs no parser and changes nothing. `complete_submission` runs it
  again, then applies only the captured rows: updated and inserted rows take the
  receipt values, inserted rows take their committed keys, deleted rows leave, and
  the draft and undo history clear. A failure (`GridCommitFailure`) unlocks and
  keeps the draft, marking the named row or setting `commit_failure()`.
- **Clone starts unlocked.** The submission state is a private type whose `Clone`
  yields the default, like `GridOrigin`, so a clone cannot inherit or consume a
  token.
- **`apply_commit_result` is gone.** It applied whatever happened to be pending.
  `GridChangeSet::identity_receipt` is the acknowledgement of a host with no
  authoritative store.
- **Never-reused tab keys.** Each `TableTab`, `QueryTab` and `HistoryTab` carries
  an `ItemKey` from a private checked counter on `Workbench`. `WorkTab::key`,
  `Workbench::tab_mut` and `active_key` resolve by key; a reopened table is a new
  key and a new render id. A preview replacement skips a tab that is saving, and
  `close_tab` refuses a saving table.
- **One request in the app.** `CommitRequest` (tab key, connection generation,
  names, `GridChangeSet`, SQL) and `CommitState` (`Reviewing`, `Submitted`,
  `FailedReceipt`) replace the timer. The review dialog, the completion, the
  history entries and the status all read the request. Completion finds the grid
  by tab key under the request's connection generation; reconnect and disconnect
  cancel the lock and bump the generation, so a late callback finds nothing. A
  completed request is retired before the success is recorded, so a repeated
  delivery has no second effect. A receipt that does not validate leaves the draft
  locked and shows a dialog whose Cancel releases it.
- **Strip marks follow the save.** The tab strip is rebuilt after a save, so the
  saved tab stops showing a dirty mark even when another tab is active.
- **Showcase.** `DemoCommit { snapshot, token, stage }` replaces `commit_ticks`.
  The review is built from one capture, the plan-limit check reads the snapshot
  and `set_option` cancels the request before it replaces the grid.

## Migrating

`record_cell`, `toggle_delete`, `insert_row`, `duplicate_row`, `undo` and
`discard` return `Result<_, GridEditError>`; `set_rows`, `append_rows` and the
keyed setters return `Result<(), GridDataError>`. `try_set_*_keyed` no longer
return a bare `GridKeyError`; it is `GridDataError::Keys`. `TableTab::load`
returns `Result<(), GridDataError>`. Replace `apply_commit_result(Ok(()))` with
`begin_submission` then `complete_submission`.

## Limits

- `DataGrid::columns`, `edit`, `choice`, `editable` and the error maps are still
  public fields. They are display and configuration, not the draft, and moving
  them behind accessors belongs with the encapsulation work in #33 and #42.
  `DataGrid::update_column` (#42) checks the same lock and returns `Busy`.
- Sort, filter, refresh and discard on a saving table are refused with a status;
  the guarded view transition that lets Save continue into them is #12.
- Preview promotion of an accepted live edit is #12. A store-backed receipt and
  expected table revision are #51. This simulator makes no I/O promise.
- A review dialog is modal, so closing the reviewed tab cannot happen while it is
  open; `close_tab` refuses it anyway.
