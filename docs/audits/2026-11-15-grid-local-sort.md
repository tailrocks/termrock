# DataGrid local sort after append and commit (2026-11-15)

Issue #46. PR #175.

## Finding

With `local_sort` on, an active sort was lost in two places. `append_rows`
added rows at the end, and a successful `apply_commit_result` rebuilt the
order from source order while the sort header stayed lit.

## Policy

- The display order is built by one helper, `DataGrid::rebuild_order`. It
  runs once per real append, once per successful commit that changed
  something, and once per `set_rows`.
- With `local_sort` on and a valid sort column, original rows are
  stable-sorted by their saved (committed) cell values. Asc puts Null last.
  Desc reverses the whole comparison, so Null comes first. Ties keep source
  order (Asc) or reverse it (Desc).
- Inserted drafts stay after the sorted originals, in insertion order, until
  commit.
- Pending edits do not move rows. Typing, no-op edits, and failed commits do
  not rebuild the order.
- With `local_sort` off, the order is the literal source order.
- The cursor and range anchor follow their record through each rebuild. If the
  cursor's record is deleted by a commit, the cursor moves to the next survivor
  in old display order, else the previous one.

## Tests

`src/widgets/grid_sort_tests.rs`, all named `issue46_*`. Run with
`cargo nextest run -p junie-tui --all-features --no-tests fail -E 'test(issue46_)'`.

Covered: append under asc and desc, sorting by saved values while a pending
edit exists, anchor following its record on append and commit, commit that
reorders the changed column, commit that reorders elsewhere, drafts staying
at the tail, deletion with cursor fallback (including delete and reorder in
one commit), remote and failed-commit paths, empty append, empty commit, and
a stale sort column.

Three mutations were checked by hand. Each one made a named test fail:
dropping the anchor remap in `restore_view`, sorting by pending values, and
removing the cursor fallback in `apply_commit_result`.

## Limits

- Identity is the source index, with an explicit old-to-new map on commit.
  This holds until #33 (keyed rows) lands. #166 then replaces the map with
  key capture.
- Append does not validate incoming rows. That belongs to #42 (checked
  ingress). The comparator reads cells with checked access, so a short row
  does not panic.
