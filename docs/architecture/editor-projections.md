# Editor text projections (issue #40)

`CodeEditor` and `TextArea` paint from projections of the text instead of
re-reading it every frame. This note records what is cached, what each cache
is keyed by, and the costs that remain on purpose.

## Content revision and line index

`core::text::TextBuffer` owns a process-unique content revision
(`TextBuffer::revision`). A real content change takes a fresh ticket. A caret
move, a selection change, an equal-text replacement and deleting nothing keep
the ticket. Cloning an unchanged buffer keeps the ticket, so a cache keyed by
the ticket is valid for the clone. Replacing `EditField::buffer` with another
buffer always gives a different ticket, so no app keeps a revision counter of
its own.

The buffer also owns a lazily built line index (`core::text_index`). It holds
the byte range and display width of every line, and a per-line grapheme table
built on first use. `line_range`, `line_width`, `max_line_width`,
`position_at`, `offset_at`, `line_count`, `selection_lines` and
`has_selection_lines` read it with binary searches, so caret events no longer
scan a document prefix. The text is shared behind an `Arc` and copied only
when a clone is edited, so `TextBuffer::clone` is O(1) and a clone shares the
already built index.

## Editor caches

`CodeEditor` keeps these caches, each rebuilt once per key change:

| Cache | Key | Rebuilt when |
| --- | --- | --- |
| Syntax runs (first provider span wins) | text revision, highlighter generation | text edit, `set_highlighter`, `invalidate_highlighter` |
| Segment snapshot (`current_block`, `blocks`, block navigation) | text revision, segmenter generation | text edit, `set_segmenter`, `invalidate_segmenter` |
| Diagnostic runs, exact offsets, gutter and nearest index | text revision, diagnostics revision | text edit, `set_diagnostics`, `push_diagnostic` |
| Bracket pairs | text revision | text edit |
| Find matches | text revision, query text | text edit, new query |
| Find runs | text revision, match scan | new match scan |

A provider setter always starts a new generation, even for the same function
pointer, because a callback may read configuration the editor cannot see. Call
`invalidate_highlighter` or `invalidate_segmenter` after changing that
configuration. The cache-owning fields are private; read them with
`highlighter_fn`, `segmenter_fn`, `diagnostics` and `find_state`.

## What a frame costs

An unchanged, hovered, focused or caret-only frame visits the visible rows and
graphemes and does binary-search lookups. It does not hash, copy or split the
document, rerun a provider, or rescan annotations.

## Costs that remain on purpose

- Any real edit rebuilds the line index, syntax runs, diagnostic index,
  bracket map and segment snapshot once, each O(document bytes). Provider
  callbacks may scan the whole text.
- The text is a flat `String`. Inserting into it moves the tail, so an
  arbitrary insertion is O(document bytes). When the buffer is shared with a
  clone (the undo "before" copy of an edit key), the first edit also copies the
  text.
- `UndoStack::record` snapshots the whole text per undo step, O(document
  bytes), up to `UndoStack::LIMIT` steps.
- `TextArea::begin_edit` snapshots the text for blur and Esc revert.
  `CodeEditor::begin_edit` snapshots it only when `esc_reverts` is on.
- The line index costs about one small record per line (range, width and an
  empty table slot). A grapheme table costs two machine words per grapheme and
  exists only for lines that were painted or positioned. Syntax and diagnostic
  indexes cost O(span count).
- Incremental ropes and incremental highlighters are outside this change.

The work counters that prove these claims live in the unit tests
(`core/text/tests.rs`, `widgets/code/tests.rs`, `widgets/textarea/tests.rs`).
Issue #56 owns the default-off counters that will replace them in the
real-path budgets.
