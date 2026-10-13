# TextArea design spike

## Recommendation

Build `TextArea` on an owned `Vec<String>` line buffer. Extract private,
allocation-free single-line grapheme operations into `widgets/edit_core.rs` and
make both `TextInputState` and `TextAreaState` use them. Keep line joining,
vertical goal-column motion, and two-axis viewport ownership in TextArea only.
This creates one implementation home without forcing a multi-line state model
onto TextInput.

The lookbook-local, test-only prototype proves 34 cursor/edit cases. It owns a
nonempty line vector, a `(line, byte)` cursor, and a remembered display-column
goal. Every mutation case asserts buffer and cursor together. No TermRock public
surface changed.

## Drift

Plans 011, 013, and 017 are DONE. Plan 012 removed `geometry.rs`; current width
helpers live in `text`. TextInput now uses the canonical private builder and
state-owned event handling. Its observable behavior remains the regression
contract: this spike changes none of its code or tests.

## Buffer model evaluation

Scores: 1 poor, 5 strong.

| Model | Edit simplicity | Render/wrap views | Undo readiness | TextInput sharing | Dependency posture | Total |
|---|---:|---:|---:|---:|---:|---:|
| `Vec<String>` lines | 5 | 5 | 4 | 4 | 5 | 23 |
| single `String` + line starts | 3 | 3 | 4 | 5 | 5 | 20 |
| rope crate | 3 | 3 | 5 | 2 | 1 | 14 |

`Vec<String>` wins. Newline is `split_off` plus insert; boundary deletion joins
adjacent lines; rendering borrows each line directly; future operation logs can
identify line and byte ranges. Form-sized editors do not justify a rope or its
dependency/iterator model. A single String shares TextInput storage shape but
requires line-index rebuilding and gives rendering worse views.

The state always contains at least one line. `set_text("")` produces one empty
line. Internal line strings never contain `\n` or `\r`; parsing recognizes
`\r\n`, `\n`, and lone `\r` as line breaks and normalizes extraction to `\n`.
Cursor bytes are always extended-grapheme boundaries in the selected line.

## Cursor and edit invariants

- Horizontal movement and deletion operate on Unicode extended grapheme
  clusters, never scalar values or display cells.
- Left at byte zero moves to the previous line end. Right at line end moves to
  the next line start.
- Backspace at line start and Delete at line end join adjacent lines. If the
  join merges graphemes (for example `"e" + "\u{301}"`), the cursor advances to
  the first **global** grapheme boundary at or after the logical join byte.
  Keeping the old byte would leave an invalid cursor inside the merged cluster.
- Enter inserts a newline. It never submits.
- Home/End target logical line boundaries. Ctrl+Home/Ctrl+End may become
  document-boundary bindings in the build.
- First vertical motion records the cursor's display column. Consecutive
  Up/Down/Page motions preserve it through short or empty lines. Any horizontal
  move or mutation clears it.
- Vertical placement chooses the greatest grapheme boundary whose display
  column does not exceed the goal. If the goal lands inside a two-cell grapheme,
  the cursor stays before that grapheme. It never overshoots or occupies a
  continuation cell. This pins the STOP-condition ambiguity.
- PageUp/PageDown move by `max(viewport_height, 1)` logical lines and clamp at
  buffer edges. Soft wrap is absent in v1, so logical and visual rows agree.

### Executable prototype table

The table-driven prototype covers these 34 cases:

| Group | Cases |
|---|---|
| insert | ASCII, CJK, emoji, combining mark, base before an existing mark, ZWJ joining emoji |
| newline | middle, start, end |
| backspace | ASCII, combined grapheme, ordinary join, combining-leading join, buffer start |
| delete | ASCII, emoji, ordinary join, combining-leading join, buffer end |
| horizontal | combined grapheme left, cross-line left, emoji right, cross-line right |
| edge | Home, End |
| vertical | same-column up/down, short line, remembered goal restoration, wide-grapheme boundary, empty-line up/down |
| page | down by viewport, upward clamp |

## Shared edit core

Extract only operations genuinely defined on one mutable line:

```rust
mod edit_core {
    pub(crate) fn is_boundary(line: &str, byte: usize) -> bool;
    pub(crate) fn previous_boundary(line: &str, byte: usize) -> Option<usize>;
    pub(crate) fn next_boundary(line: &str, byte: usize) -> Option<usize>;
    pub(crate) fn insert_char(line: &mut String, byte: &mut usize, ch: char) -> Option<LineDelta>;
    pub(crate) fn insert_inline(line: &mut String, byte: &mut usize, text: &str) -> Option<LineDelta>;
    pub(crate) fn backspace(line: &mut String, byte: &mut usize) -> Option<LineDelta>;
    pub(crate) fn delete(line: &mut String, byte: usize) -> Option<LineDelta>;
    pub(crate) fn move_left(line: &str, byte: &mut usize) -> bool;
    pub(crate) fn move_right(line: &str, byte: &mut usize) -> bool;
    pub(crate) fn byte_at_display_column(line: &str, goal: usize) -> usize;
}

pub(crate) enum LineDelta {
    Inserted { range: Range<usize> },
    Deleted { at: usize, text: String },
}

pub(crate) enum TextEditDelta {
    Line { line: usize, delta: LineDelta },
    Split { at: TextCursor },
    Joined { inverse_split: JoinPoint },
}

// Raw normalized buffer coordinate. Unlike TextCursor, a join seam may become
// interior to a grapheme after concatenation and is retained only for undo.
pub(crate) struct JoinPoint { line: usize, byte: usize }
```

TextInput keeps value validation, `max_graphemes`, forbidden values,
submit-on-Enter/Ctrl+M, and horizontal reveal. Its max-grapheme check happens
before shared insertion. TextArea owns line split/join, vertical movement,
goal column, text parsing/extraction, and `DialogScroll`. The extraction is a
real shared implementation, not a public facade or copied editing body.

Build order: first characterize current TextInput results at the private-core
boundary; extract; then add TextArea. Mutation helpers return inverse-ready
typed deltas: inserted ranges can be removed, deletions retain their exact
payload, and split/join records retain normalized coordinates. Normal editing
does not clone the document or diff snapshots.

One observable TextInput bug must intentionally change during extraction.
Current `TextInputState::apply(Insert)` advances by the inserted scalar's byte
length. Inserting `e` before an existing leading combining mark changes
`"\u{301}x"` to `"e\u{301}x"` but leaves cursor byte 1 inside the new grapheme.
The prototype instead finds the first boundary in the **globally segmented
post-edit line**, yielding byte 3. Suffix-only segmentation is also invalid for
ZWJ insertion, and old join bytes can become invalid after line joins. The
architecture allowed this bug because insertion, joining, and boundary repair
had no single primitive. The build must first add regression tests that expose
the current invalid cursor, then deliberately fix them through shared core,
write the next migration, and keep all unrelated TextInput expectations
unchanged. This is a recorded forward-only breaking correction, not spike work.

## Public API

```rust
pub struct TextArea<'a> { /* private: theme, title, placeholder */ }

impl<'a> TextArea<'a> {
    pub const fn new(theme: &'a Theme) -> Self;
    pub const fn title(self, title: &'a str) -> Self;
    pub const fn placeholder(self, placeholder: &'a str) -> Self;
}

pub struct TextAreaState {
    lines: Vec<String>,
    cursor: TextCursor,
    goal_column: Option<usize>,
    scroll: DialogScroll,
    focused: bool,
    // painted viewport and reusable render scratch
}

pub struct TextCursor { pub line: usize, pub byte: usize }

#[non_exhaustive]
pub enum TextAreaOutcome {
    Ignored,
    Changed,
    Cancelled,
}

impl TextAreaState {
    pub fn new(text: impl AsRef<str>) -> Self;
    pub fn handle_key(&mut self, key: KeyEvent) -> TextAreaOutcome;
    pub fn handle_event(&mut self, event: Event) -> TextAreaOutcome;
    pub fn insert_text(&mut self, text: &str) -> TextAreaOutcome;
    pub fn set_text(&mut self, text: &str);
    pub fn lines(&self) -> impl ExactSizeIterator<Item = &str>;
    pub fn text(&self) -> String;
    pub const fn cursor(&self) -> TextCursor;
    pub fn set_cursor(&mut self, cursor: TextCursor) -> bool;
    pub fn set_focused(&mut self, focused: bool);
    pub const fn is_focused(&self) -> bool;
    pub fn scroll(&self) -> &DialogScroll;
    pub fn scroll_by(&mut self, delta_x: isize, delta_y: isize) -> bool;
    pub fn scroll_to(&mut self, position: Position) -> bool;
}
```

Focus belongs only to state, matching Tree/TextInput interaction ownership; the
widget builder has no competing focus flag. `text()` allocates intentionally
for consumer extraction. A later `write_text(&mut impl fmt::Write)` can support
allocation-free serialization if evidence requires it.

`TextAreaOutcome` has no Submitted variant. Enter is the editor's newline.
Submission is consumer policy: a consumer keymap may intercept Ctrl+Enter (or
any chord), read `text()`, validate, and perform its effect. TextInput keeps its
single-line Enter/Ctrl+M submission contract.

After Plan 035, `handle_event(Event::Paste(text))` routes directly to
`insert_text`. TextArea normalizes CRLF/LF/CR into line splits and inserts the
entire multi-line payload in v1; unlike TextInput it never truncates at the
first newline. Paste is editing, not selection, and is not deferred.

## Viewport and rendering

V1 soft wrap is off. `DialogScroll.scroll_y` is the first visible logical line;
`scroll_x` is a display-column window shared by all visible lines. After every
edit or cursor move:

1. vertical cursor-follow uses body height and `cursor_follow_offset`;
2. horizontal reveal compares the cursor display column with the current
   `[scroll_x, scroll_x + body_width)` window;
3. offsets clamp after line count or maximum visible width changes;
4. rendering slices only visible logical lines through display-column-safe text
   helpers and paints a one-cell `Role::Focus` cursor.

The cursor remains visible at width/height zero without underflow. The title and
border use the same single-line Panel glyphs everywhere; state focus projects
only to `PanelEmphasis::Focused` / `Role::BorderFocused`. Placeholder appears
only for one empty line. Selection is absent, so no hidden selection state or
clipboard shortcut ships in v1. State also owns the last painted body and
scrollbar regions; wheel and drag methods consume only that canonical geometry.

Measurement is O(visible lines) for painting. Horizontal maximum width may use
a revision-keyed cached aggregate measured when text changes, never recomputed
on cursor-only frames. Scratch strings are state-owned and reused after warmup.

## Undo and range readiness

Mutations return the `LineDelta` / `TextEditDelta` values above: insert/delete
text at `(line, byte)`, split line, and join line. V1 need not retain them, but
tests apply each inverse to prove exact restoration without snapshot diffing.
The buffer exposes internal normalized byte-range extraction across lines;
selection and OSC 52 clipboard can consume it later. Do not bake snapshots,
rope positions, or UI commands into the buffer core.

## Deferred tiers

1. Undo/redo operation log with bounded history and edit coalescing.
2. Selection plus copy through hardened `osc::encode_clipboard`.
3. Soft wrap, with a shared visual-line window helper extracted only when
   TextArea, LogPane, and Table demonstrate the same need. Wrap must define goal
   columns in visual-row space separately.
4. Syntax styling as caller-projected spans over immutable render snapshots;
   editor state never owns language policy.
5. IME/composition research. Current terminal input events do not expose a
   portable composition lifecycle; do not pretend scalar key events solve it.
6. Vim modes remain consumer keymap/state policy.

## Build-plan stub

- [ ] Extract private `edit_core` and keep every existing TextInput test
  unrelated to boundary repair byte-for-byte unchanged and green; add the
  combining/ZWJ regression, fix it, document the breaking correction, and
  migrate consumers.
- [ ] Add TextArea model/state/outcomes, normalized line parsing, shared-core
  edits, line split/join, goal-column motion, page motion, and cursor setters.
- [ ] Add state-only focus, keyboard handling, two-axis reveal/clamp, painted
  viewport/scrollbar geometry, state-owned wheel/drag commands, and
  owned/borrowed `StatefulWidget` implementations.
- [ ] Route Plan-035 `Event::Paste(String)` through normalized multi-line
  `insert_text`; cover CRLF/LF/CR and grapheme-boundary repair.
- [ ] Add at least the prototype cursor table plus parsing, control-input,
  tiny-area, viewport, and invalid external-cursor tests.
- [ ] Add basic, narrow, Unicode/combining/wide, empty, and scrolled stories
  with deterministic previews and interaction support.
- [ ] Add API inventory, exact contract matrix row, component page, inventory
  prose, and catalog coverage.
- [ ] Add inverse-ready delta tests, visible-window hot-path proof, cached-width
  invalidation tests, and allocation-free warm render with no document clone.
- [ ] Add the next migration for any TextInput public behavior/API change; an
  additive TextArea alone needs none.
- [ ] Run `mise run gate`, TextInput regression tests, direct gallery
  walkthrough, preview check, feature powerset, and package verification.
