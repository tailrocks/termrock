# Plan 034 (spike): Design `TextArea` — multi-line, grapheme-safe editing with a shared edit core

> **Executor instructions**: DESIGN SPIKE. Deliverable = design doc + a
> prototype of the line-buffer/cursor core with table-driven tests + a
> recommendation. Honor STOP conditions. Update the plans/README.md row when
> done.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/widgets/text_input.rs crates/termrock/src/text/ crates/termrock/src/geometry.rs`
> Design against POST-011/013 (and post-012 module homes). Verify rows DONE.

> **Reconcile note (2026-07-16, round 3, HEAD `5c4758b`)**: `geometry.rs` was
> dissolved by plan 012 — the width/grapheme primitives now live in `text/`
> (`display_cols`, `display_cols_slice`, `take_display_cols`, …); drift-check
> paths updated accordingly. `text_input.rs` gained construction-idiom changes
> (013); plan 035 adds `insert_str` paste handling whose newline-truncation
> rule this spike's editor supersedes for multi-line consumers. Re-read live
> signatures.

## Status

- **Priority**: P3
- **Effort**: L (coarse — grapheme-correct multi-line cursor math is the hard part)
- **Risk**: MED (a shared edit core must not degrade `TextInput`; wrap math is subtle)
- **Depends on**: plans/011-event-model-convergence.md, plans/013-construction-idiom-and-widget-traits.md; plans/017 (geometry test tables — the width primitives this builds on must be pinned first)
- **Category**: direction
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

The library cannot edit multi-line text. `TextInputState` holds one `String` + one byte-offset cursor + a scalar horizontal viewport; `EditAction` has no vertical motion; and `Enter` is hard-wired to `submit()` — a newline is unrepresentable. Commit messages, notes, annotations, config blocks, prompt/chat composers: none can be built, and composing around `TextInput` is impossible by construction. Every mature toolkit ships a text area; under the shadcn-of-TUI ambition this is the second-biggest widget gap (after Table). It's a spike because grapheme-safe two-axis cursor movement, soft-wrap interaction with display columns, and the shared-core question (no duplicated edit logic — AGENTS.md forbids parallel implementations) all carry design risk.

## Current state

- `crates/termrock/src/widgets/text_input.rs` (verbatim cores):

```rust
pub enum EditAction { Insert(char), Backspace, Delete, MoveLeft, MoveRight, Home, End }

pub struct TextInputState {
    value: String,
    cursor: usize,          // byte offset
    viewport: usize,
    max_graphemes: Option<usize>,
    forbidden: Vec<String>,
    allow_empty: bool,
}
```

  `handle_key`: `KeyCode::Enter => self.submit()`; grapheme-safe editing is the module's documented contract ("grapheme-safe editing", components.mdx); `reveal_cursor` (~:225) computes one-axis horizontal reveal. Existing tests cover grapheme boundaries (`widgets/tests.rs` + inline).
- Width/grapheme primitives (post-012 in `text::`): `display_cols`, `display_cols_slice`, `take_display_cols`, `leading_space_cols` — pinned by Plan 017's test tables. `unicode-segmentation` is already a dependency (grapheme iteration).
- Scroll state: `DialogScroll` (post-024 the single two-axis type) for the viewport.
- Post-011 contract + post-013 idiom as for every widget. Contract axes incl. narrow-terminal/unicode stories (Plan 023 gate).
- Doctrine: consumers own validation/wording/submission (TextInput's `forbidden`/`allow_empty` pattern shows the line: mechanical constraints in-widget, semantic validation consumer-side).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| TextInput regression | `cargo test -p termrock text_input` | all pass |

## Scope

**In scope (spike)**:
- Design doc `plans/034-textarea-design.md`
- Prototype: the line-buffer + 2-axis cursor core as a standalone module with table-driven tests (lookbook-local or `#[cfg(test)]` — zero public surface)
- The shared-core analysis: exactly what `TextInput` and `TextArea` share, and the refactor shape

**Out of scope**:
- The full widget build.
- Syntax highlighting, undo/redo, selections/clipboard (recorded future tiers — undo/redo likely tier 2; note the buffer design must not preclude it: that constraint IS in scope for the doc).
- IME/composition input (research note only).
- Vim modes (consumer keymap territory).

## Steps

### Step 1: Decide the buffer model

Evaluate exactly three, with the repo's constraints (borrowed-data doctrine does NOT apply here — like `TextInputState.value: String`, an EDITOR owns its buffer):

1. **`Vec<String>` lines** — simple; newline = split/join; O(line) edits; matches how rendering consumes lines. Undo = snapshot or op-log later.
2. **Single `String` + line-start index cache** — TextInput-compatible representation; every newline edit rebuilds indices; cursor math stays byte-offset like today.
3. **Rope (external crate)** — scales to huge documents; new dependency (`ropey`-class); overkill for form-sized text, and dependency posture here is deliberately lean (workspace has 11 deps).

Score: edit complexity, wrap/render integration (the renderer wants `&[line]` views), undo-readiness, TextInput-core sharing, dependency cost. Expected winner: 1 for the editor + the shared core extracted at the SINGLE-LINE level (see Step 3) — but let the prototype tests decide; record honestly.

### Step 2: Prototype the cursor/edit core

Implement the winner's core with table-driven tests (the spike's hard deliverable). Cursor = `(line, byte_offset)` + a REMEMBERED goal column for vertical motion (the classic editor invariant: Up/Down preserve the target display column across short lines — pin it). Operations: insert char, insert newline, backspace at line start (join), delete at line end (join), Left/Right across line boundaries, Up/Down with goal column, Home/End, PageUp/PageDown given a viewport height. Grapheme cases in every table: CJK (2-col), emoji, combining marks (`e\u{301}`), empty lines, cursor at buffer start/end.

≥20 table-driven cases; every mutation asserts BOTH buffer content and cursor position.

**Verify**: prototype tests green; zero public-surface change (`cargo test --workspace` green untouched elsewhere).

### Step 3: The shared-core and API analysis

In the doc: diff `TextInputState`'s edit paths against the prototype — what is genuinely shared (grapheme-boundary insert/delete/motion WITHIN a line) vs input-only (`forbidden`, `max_graphemes`, submit-on-Enter, single-axis reveal) vs area-only (line ops, goal column, 2-axis viewport). Recommend the refactor shape: extract `edit_core` (private module) consumed by both states — NOT a facade, a genuine shared implementation (AGENTS.md-compliant). Sketch the public API:

```rust
pub struct TextArea<'a> { /* theme, title?, placeholder? — post-013 idiom */ }
pub struct TextAreaState { /* buffer, cursor, goal_col, scroll: DialogScroll, dirty/measured caches per Plan 016 */ }
impl TextAreaState {
    pub fn handle_key(&mut self, key: KeyEvent) -> TextAreaOutcome;   // Enter = newline; submit is CONSUMER-bound (e.g. Ctrl+Enter via keymap) — decide + record
    pub fn lines(&self) -> impl Iterator<Item = &str>;                // consumer extraction
    pub fn set_text(&mut self, text: &str);
}
```

Pin the submit question explicitly (Enter-inserts-newline is the editor default; submission belongs to the consumer's keymap — cite TextInput's Ctrl+M/Enter split as the precedent to invert). Soft-wrap: recommend OFF for v1 (horizontal scroll via `DialogScroll`, same as Viewport) with wrap as tier 2 — wrap × goal-column × grapheme math triples the risk; say so.

### Step 4: Design doc + build-plan stub

`plans/034-textarea-design.md`: buffer-model evaluation + winner, the cursor invariant table (from Step 2's tests), shared-core refactor spec (with the TextInput regression-safety requirement: its existing tests must pass unchanged), public API + outcome enum, viewport/reveal spec (cursor-follow on both axes — `cursor_follow_offset` exists in scroll), undo-readiness constraint statement, future tiers (wrap, selection/clipboard via OSC 52 write path, undo/redo, IME research), build checklist (widget + narrow/unicode stories + contract row + hot-path test + component page).

**Verify**: doc exists; README row updated.

## Done criteria

- [ ] `plans/034-textarea-design.md`: buffer evaluation, cursor invariants, shared-core spec, API, deferred-tier rationale
- [ ] Edit-core prototype with ≥20 passing table-driven cases (grapheme/boundary/goal-column all covered)
- [ ] TextInput's existing tests untouched and green
- [ ] Zero public-surface changes; gates green
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plans 011/013/017 not DONE — stop, dependencies (017's pinned width primitives are the foundation).
- The shared-core extraction would change `TextInputState`'s observable behavior (any existing test needs edits) — that's a build-plan-scoped breaking decision; record it, don't do it in the spike.
- Goal-column semantics prove ambiguous with 2-col graphemes at line ends (the classic wide-char-at-boundary question) — pin ONE behavior in the test table with a rationale; ambiguity documented beats ambiguity shipped.

## Maintenance notes

- The edit-core module becomes the single home of grapheme-edit logic — future TextInput fixes land there once, serving both widgets.
- Selection + clipboard (tier 2) will compose with `osc::encode_clipboard` (hardened by Plan 004) — the buffer design must expose byte-range extraction for it (note in the doc).
- Wrap (tier 2) should reuse whatever line-window helper emerges from LogView/Table work — three widgets slicing line windows is the extraction trigger.
