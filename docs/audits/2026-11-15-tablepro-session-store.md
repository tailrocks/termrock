# Persist TablePro simulated saves in a session store (#51)

A successful save only told the grid it was clean. Every SELECT regenerated the
deterministic fixture, so a saved edit vanished on refresh, sort, filter, close
and reopen or another query, a saved insert disappeared, and a saved delete came
back.

## Decision

- **One store per connection session.** `Workbench` owns a `SessionStore` built
  with the connection generation. It ends with the connection, so reconnecting
  starts from the seed again. Closing and reopening a table keeps it.
- **Sparse overlays over a lazy seed.** A table holds `updated`, `deleted` and
  `inserted` maps and counters, nothing else. A baseline row is read by
  `db::rows(table, ordinal, 1)` and then overlaid. The bounded 2,000 row sample
  a query scans is the same one as before, with saved changes applied, plus every
  inserted row.
- **Keys name rows, not values.** A baseline row is `ItemKey::Num(ordinal * 2)`
  and an inserted row is `Num(counter * 2 + 1)`. Keys flow through filtering,
  sorting and projection in `ResultSet::row_keys`, into `try_set_rows_keyed`, and
  back through the commit receipt. Editing a primary key never changes one.
- **Revisions.** Each table counts accepted saves. A `TableTab` records the
  revision it read (`table_revision`), separately from the grid's own revision,
  and a save carries it in `CommitRequest::expected_table_revision`.
- **Prepare, then apply.** `prepare_commit` is pure. It checks the revision, the
  column list, that every updated and deleted row still holds the values the grid
  read, value kinds, finiteness, NULL against nullability, enum membership and
  generated columns, then resolves defaults and builds the complete receipt.
  `apply_prepared` installs the plan with nothing left that can fail. The plan
  names its own table, store instance and connection generation, so it cannot be
  retargeted, and two stores at the same generation still reject each other's
  plans.
- **Grid first, then store.** The app asks the grid to validate the receipt
  (`validate_submission_receipt`) before the store changes, then applies, then
  completes the submission with the same receipt. A refusal at either step calls
  `complete_submission` with a failure (or keeps the lock for a bad receipt), so
  the draft, the undo history and the grid lock behave as #11 defined.
- **Defaults.** `gen_random_uuid()` is `00000000-0000-8000-8000-{counter:012x}`,
  a version 8 id that cannot collide with the version 4 seed ids. `identity` and
  `nextval('orders_number_seq')` continue the seed's sequence with checked
  arithmetic. `now()` is one captured `CommitContext` timestamp for the whole
  save, from a frozen simulator clock (`2025-06-01 12:30:00+00`). Quoted text,
  boolean and numeric literals parse by column type. Any other expression is
  `UnsupportedDefault`. An explicit NULL never invokes a default.
- **Peers.** After a save the workbench refreshes every other table view that has
  no draft, no open editor and no save in flight, through the same
  `prepare_load` and `install_load` as #12. Any other view keeps its draft and its
  exact editor input, shows `source changed` in the tab strip, and fails its own
  save with `Source changed; save requires reload/review` until it reloads
  through the #12 gate. A query result is a snapshot: it is marked, never rerun.
- **Live counts.** The explorer, the completion list, the risk text of a write,
  EXPLAIN and the totals of the saving tab read the store's exact count. The
  catalog's `row_count` stays the seed size.

## Behavior changes

- Saved updates, inserts and deletes persist for the connection session.
- A new row saved with a NULL in a required column, an unknown enum value or a
  value of the wrong kind is refused with a row error, as a database would. Use
  `DEFAULT` for a column that has one.
- Filtered totals remain estimates. Unfiltered totals are exact.
- A second view's save after another view saved the same table fails as a
  conflict instead of overwriting it.

## Limits

This is a simulator. Nothing is written to disk. Foreign keys, unique indexes and
CHECK constraints are not enforced, `numeric(12,2)` is not rounded, and
non-SELECT statements typed into the query editor still only report a status.
The explorer opens one tab per table, so the multi-view conflict paths are
exercised through `Workbench::open_second_view`, which exists for tests.
