<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# Query workbench document keys (#52)

Defect: `Tabs` updates its active index before it emits `Activated`. The Query workbench `switch_to` returned early when the requested index equalled `tabs.active`, so a tab click or key navigation left the previous document's editor loaded. The next `store_editor` (Ctrl+R, new tab, close, option change) wrote that text into `docs[tabs.active]`, and Ctrl+R ran it.

Fix, in `src/bin/showcase/pages/query_workbench.rs`:

- `Doc { key, title, editor, result, results }` owns its `CodeEditor` and result `DataTable`; the parallel `sql` copy, `store_editor` and `load_editor` are gone.
- The page keeps `active_key` and a checked monotonic `next_doc_key`. `activate_document(key, focus)` is the single transition; `switch_to(index, focus)` is a bounds-checked adapter. Equal-key activation only syncs the strip cursor.
- Editor and result widget ids are scoped by document key (`editor_id`, `results_id`). Activation, and closing the active document, move focus on the departing document's editor or results to the same role of the new active document; closing the last document moves focus to the strip.
- `close_tab` removes exactly the requested document: an inactive close keeps `active_key`, closing the active one selects the next document at that position, then the previous.
- The run in flight is bound to the document key (`running: (ItemKey, ..)`), so closing another tab cannot retarget it. Capturing the SQL at run time is #53.
- `rebuild` configures retained editors in place (`read_only`). Only a real change of the Result option installs a result, on the active document.

Not changed: library widgets, `Tabs` notification order, layout, colours, the `visual-baseline` tag.

Limits: a `CodeEditor` clears its selection and edit mode when it is painted without focus, so a selection survives a switch only when focus follows the editor (programmatic or explorer activation), not a tab click that moves focus to the strip. Pointer capture of a drag in progress is owned by the shell and is not cancelled by the page; activation cannot happen mid-drag from the keyboard or strip.
