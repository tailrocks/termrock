<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# Query workbench run identity (#53)

Defect: the Query workbench stored its run as a positional tuple and, at completion, read the target document's current SQL. After #52 the tuple already carried the document key, so closing an earlier tab no longer redirected the run. Three gaps remained: a late edit changed which query "ran", a cancelled or duplicate completion had no guard, and a result could attach a diagnostic range computed for text the editor no longer held.

Fix, in `src/bin/showcase/pages/query_workbench.rs`:

- `OperationToken { document: ItemKey, generation: u64 }` (Copy, Eq, Debug) names one accepted run. It reuses `ItemKey`; there is no second identity type. #54 moves this exact type into the generic document workspace.
- `RunRequest { token, sql, explain, ticks_left }` replaces `(ItemKey, bool, u8)`. `run` captures `editor.text().to_owned()` once, with exact whitespace, after the non-empty check. `tick` takes the whole request at zero and hands it to `finish`; nothing is rebuilt from the active tab or the current editor.
- `Doc` gains `run_generation`, `last_completed_generation`, `result_sql` and `result_token`. `run` and `cancel_running_for` bump `run_generation` (checked add, preflighted before the request is dropped). Busy is derived (`is_running`): the request's key and generation must match a live document.
- `finish` admits a completion only through `completion_index` (same key, same generation, not yet completed), marks the generation completed first, then installs the result and the executed SQL by key. A cancelled, closed, stale or duplicate completion changes nothing and posts no status.
- `Doc::install_for` places the error diagnostic only when the editor text equals the captured SQL byte for byte. A result from a run whose SQL differs from the editor shows `Result for earlier query text` in the Results meta; initial and Result-option demonstration results carry no run and show no note.
- Closing the target tab, and Stop, call `cancel_running_for`; closing any other tab leaves the request alone. A reopened title gets a fresh key.
- Policy kept: one running query at a time. Run and Explain stay disabled while one is pending and Ctrl+R now says `A query is already running in <title>`; Stop stays enabled from any tab. A completion for a document that is not active is prefixed with its title (`customers: ✓ 40 rows · 15 ms`), and Stop from another tab says `Query cancelled in <title>`. Completion never activates a tab or moves focus.

Not changed: library widgets, layout, colours, the `visual-baseline` tag. No approved capture changes; the new states appear only after an edit during a run or a completion on another tab.

Limits: the Query workbench simulates execution; there is no cancellation of real work. #54 owns the shared `DocumentWorkspace` lifecycle built on this token. Parallel queries remain out of scope.
