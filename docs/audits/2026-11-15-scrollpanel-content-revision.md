<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# ScrollPanel content revision (#39)

Defect: `ScrollPanel` kept its wrapped rows under a width-only key and exposed `pub lines: Vec<String>`. Editing, replacing, truncating or clearing the vector at the same width left the old wrapped text painted and selectable until a resize or an unrelated `push`.

Fix, in `src/widgets/panel.rs`:

- `lines` is private. `lines()` borrows it. `set_lines`, `replace_line`, `extend_lines`, `clear` and `push` are the only writers and each bumps a checked `content_revision` once and drops the wrapped rows. `set_lines`, `replace_line` and `clear` also reset the selection, the pointer press and the word drag. `push` and `extend_lines` keep them, because an append never changes an existing row and the selection is stored as row and column cells. This keeps drag-selection and Select All alive in a log that appends every tick. Selection that follows logical source text across edits stays with #45.
- `set_lines`, `replace_line` (same text), `extend_lines` (empty) and `clear` (already empty) return `false` and change nothing: no revision bump, no selection reset, no rebuild. `replace_line` returns `Err(index)` for an index past the last line.
- `extend_lines` drains the iterator before it touches the content, so a panicking iterator leaves the lines, revision, selection and cache as they were.
- The wrapped cache is keyed by `(content_revision, text_width)`. Width zero is an ordinary key. The widest logical line (the sideways extent) is cached by revision alone, so a width-only reflow keeps it, and an append updates it from the new lines only. An unchanged frame does no wrapping, no width scan and no comparison of the content.
- On revision overflow the revision restarts at zero and both caches are dropped, so an equal old key cannot survive.
- `measure` clamps both scroll axes after a shrink and follows the tail only when `follow` was already set.

Consumers: the Showcase scrolling and taskrunner pages read `lines()` and `clear()`. The Panels page has a "Log content" option (`generated`, `edited`, `appended`, `replaced`, `cleared`) that drives `replace_line`, `extend_lines`, `set_lines` and `clear`. TablePro, Holla and Jackin only construct panels and `push`.

Breaking change: the public `lines` field is gone (v0.1).

Not changed: wrapping rules, line endings, colours, focus language, the `visual-baseline` tag. The Panels options list gains one row, so its approved captures were re-accepted through `scripts/accept-baseline.py`.

Limits: every real mutation, an append included, resets the selection. Keeping a selection across an append through logical anchors is #45. Measuring the unchanged-content cost across widgets is #56. The work counters here are test-only (`cfg(test)`).
