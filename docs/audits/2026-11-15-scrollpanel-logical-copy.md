<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# ScrollPanel logical copy (#45)

Defect: `ScrollPanel::selected_text` copied wrapped display rows. It joined rows of one source line with a literal space and trimmed trailing spaces from each slice, so a long token (`abcdefghijkl` at text width four) copied as `abcd efgh ijkl`, and meaningful trailing spaces were dropped.

Fix, in `src/widgets/panel.rs` and `src/ui/text.rs`:

- The selection is stored as `SourcePos { line, byte }`, a grapheme boundary in one entry of `lines()`, through the existing `TextSelect<P>`. `selected_text` copies the original slice with `copy_source`: entries are joined by one `\n`, newline bytes inside an entry stay, nothing is trimmed and no space is invented at a soft wrap.
- `text::wrap_mapped` is the one line-break emitter. `wrap` and `wrap_with` discard its provenance, so the string output is unchanged (`ui/text/wrap_golden.txt` records the pre-change output for a corpus of widths and options). Each wrapped row carries `MappedCell { columns, bytes }` per displayed grapheme. Continuation indent maps to a zero-length boundary. A glyph shown as an ellipsis maps to the whole original grapheme. A dropped soft-wrap gap stays a gap between adjacent source ranges, so selecting across it copies the real spaces.
- The wrap cache from #39 holds the row maps beside the rows, so they are rebuilt only for a content revision or a width change. Unwrapped rows map graphemes to columns on demand for the row under the pointer or the painted row.
- A pointer cell maps to the grapheme start; either half of a wide glyph snaps to the nearer edge, ties toward the end; a column past the row maps to the row end. Double click selects the word in the original entry, triple click the whole entry, so a word split across wrapped rows copies whole.
- Resize and wrap toggles no longer clear the selection: anchors are source positions and only the painted projection changes. Replacement and `clear` still reset it (#39). An append keeps it and does not extend a select-all range. `select_all` works before the first measure.
- `selection()` stays the visual adapter: absolute wrapped-row cells, `None` before a measure or at zero text width. At a boundary shared by two soft-wrapped rows a start endpoint is the following row's start and an end endpoint is the preceding row's end. `source_selection()` returns the `(entry, byte)` endpoints for consumers that must outlive a resize.

Consumers: Showcase, TablePro, Holla and Jackin never read the visual selection or the copied text of a `ScrollPanel`, so none changed. The Panels page "Log content" option gains `long token`, which appends an unbroken hash to the prose panel; turn Wrap on, select it and copy.

Not changed: wrapping output, colours, focus language, captures, the `visual-baseline` tag.

Limits: rows of one source entry that contains `\n` are separate paragraphs and copy with that newline. The unwrapped path recomputes graphemes for the painted row while a selection exists instead of caching a per-entry table.
