# Share document identity, preview, close and task ownership (#54)

Showcase's Query workbench and TablePro's Workbench each kept their own tab
vector, active index, preview rule, close target and delayed-task target. #11,
#12, #52 and #53 fixed the wrong-target and draft-loss paths one at a time, with
app-private keys and tokens. Nothing shared them, so a third consumer would have
repeated the work and the two apps could drift apart again.

## Decision

- **One controller.** `widgets::document_workspace::DocumentWorkspace<D>` owns the
  documents, their order, the active key and the preview slot. `D` is the app's
  document and brings its title, a dirty flag, a focus target and a restricted
  view facet. The controller holds no SQL, connection, account or command
  concept, and it reuses `ItemKey` and `Response`.
- **Keys are never reused.** A process-wide counter allocates them, so a token
  issued for a closed document cannot match a document opened later, even under
  the same title or in a recreated controller. Render ids are scoped by the key,
  never by position.
- **One operation per document.** `begin_operation` supersedes the previous one
  and reports it as `CancelOperation`. `finish_operation` runs its closure only
  for the generation still running, so a completion changes the document that
  asked and no other.
- **Content and view are separate.** `update` is the only way to change a
  document. It refuses while a close-save holds the document, advances the
  content revision and pins a preview that became dirty. `view_mut` returns the
  document's view facet, never advances the revision, and works during a
  close-save. Showcase's `DocView` and TablePro's `WorkTabView` have private
  fields and expose painting, scroll, caret and cursor movement only.
- **Close is a token.** A dirty or running document answers `request_close` with
  a `CloseToken` (document, content revision, request). Resolving it validates
  all three, so an edit between the request and the answer makes it stale and
  nothing closes. Discard emits `CancelOperation`, `Closed` and the successor's
  `Activated`, in that order. `Closed` hands the document back so the host
  finalizes its resources.
- **Save-and-close is one lock.** `begin_close_save` installs the save lock and
  its operation atomically. `validate_close_save` is the preflight for fallible
  backend and grid-receipt checks. `complete_close_save` marks the exact document
  saved and closes it, or returns `StillDirty` and keeps it. `fail_close_save`
  keeps the document and its prompt. `cancel_close_save` clears exactly that pair.
- **Tabs is a projection.** `sync_tabs` and `sync_tabs_with` rebuild the strip
  from the ordered keys, and `key_at` turns a strip position back into a key from
  the same projection.

## Consumers

- **Showcase.** `Doc` owns its editor, result and opening text. Run, stop and
  finish capture an `OperationToken` and the SQL, under #53's one-run-at-a-time
  rule. New options let the page open tables as previews and ask before a close
  that would drop edits. The strip shows no dirty mark, as before.
- **TablePro.** `WorkTab` is the document. The workbench's tab list, active index
  and preview flag are gone. A dirty query tab confirms through a prompt that
  names a close token. A dirty table offers Cancel, Discard and Save, and Save
  runs the table's own review and save as one close-save. A running query is one
  operation of its tab. Every tab mutation goes through `Workbench::modify`, and
  painting and wheel input go through `WorkTabView`.

## Behavior changes

- A preview open appends the new preview and closes the clean old one, instead of
  replacing it in place. A dirty, running, editing or saving preview is pinned
  first, as before.
- Closing a dirty table now offers Save beside Cancel and Discard. The query
  tab's confirmation and every approved capture of it are unchanged.
- An edit made after a close was requested makes that request stale: the tab
  stays open and the user is asked again.
- Tab keys come from one counter for the whole process, so they no longer start
  at zero for each workbench.
- Showcase's Query workbench Options tab lists two new rows, "Explorer opens" and
  "Close with edits", both off by default. The affected captures are re-accepted
  with receipts under `docs/testing/acceptance-receipts/document-workspace-54-20261115`.

## Limits

The controller runs one background operation per document. An app that needs
independent lanes keeps those tasks outside it. The tokens are logical
identities, not capabilities: they reject stale and mismatched values and do not
defend against a caller that forges a current one. Hosts still run their own
stricter policies first, such as TablePro refusing to close a table whose save is
submitted.
