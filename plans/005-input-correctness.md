# Plan 005: Unknown keys stop dismissing modals; all widgets ignore key-release events consistently

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/input/ crates/termrock/src/keymap.rs crates/termrock/src/widgets/text_input.rs crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/dialog.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (touches a public enum matched across the crate; the compiler surfaces every site)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The crossterm→neutral key adapter maps every unhandled key (`F1`–`F12`, `Insert`, `CapsLock`, media keys, …) to `KeyCode::Esc`. `Esc` is a meaningful action — text inputs return `Cancelled`, lists cancel, dialogs dismiss — so pressing `F5` while a dialog is focused closes it. The keymap module's documentation even promises the opposite behavior ("Unknown key codes … map to `LogicalKey::Char('\0')` which will never match a real binding") — a promise no code implements. Separately, only three of six key-handling widgets filter key-release events; on terminals using the kitty keyboard protocol (which reports releases), the unguarded widgets fire twice per keystroke — text inputs would insert every character twice.

## Current state

- `crates/termrock/src/input/event.rs` — neutral `KeyCode` (15 variants: `Backspace, Enter, Left, Right, Up, Down, Home, End, PageUp, PageDown, Tab, BackTab, Delete, Esc, Char(char)`), and the faulty adapter arm (inside `#[cfg(feature = "crossterm")]`):

```rust
                crossterm::event::KeyCode::Esc => Self::Esc,
                crossterm::event::KeyCode::Char(c) => Self::Char(c),
                _ => Self::Esc,          // <- the bug: unknown keys become Esc
```

- `crates/termrock/src/input/event.rs` — `KeyEventKind` already models the distinction: `pub enum KeyEventKind { #[default] Press, Repeat, Release }`, and `KeyEvent { code, modifiers, kind, state }`.
- Widgets **with** the release guard (the pattern to copy):
  - `crates/termrock/src/widgets/form.rs:184` — `if !self.active || key.kind == KeyEventKind::Release {`
  - `crates/termrock/src/widgets/split_pane.rs:211`, `crates/termrock/src/widgets/tree.rs:152` — same shape.
- Widgets **without** the guard:
  - `crates/termrock/src/widgets/text_input.rs:135`:

```rust
    pub fn handle_key(&mut self, key: KeyEvent) -> TextInputOutcome {
        match key.code {
            KeyCode::Enter => self.submit(),
```

  - `crates/termrock/src/widgets/list.rs:78`:

```rust
    pub fn handle_key(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> ListOutcome<Id> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.select_relative(rows, -1),
```

  - `crates/termrock/src/widgets/dialog.rs:238`:

```rust
    pub fn handle_key(&mut self, key: KeyEvent, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        match key.code {
            KeyCode::Esc => Outcome::Cancelled,
```

- `crates/termrock/src/keymap.rs:138-139` — the stale doc comment on `impl From<crate::input::KeyEvent> for KeyChord`: "Unknown key codes (function keys, media keys, …) map to `LogicalKey::Char('\0')` which will never match a real binding."
- The two `From` impls in `keymap.rs` (`From<input::KeyEvent>` and `From<input::KeyCode>`) both `match` exhaustively over the current 15 `KeyCode` variants — adding a variant makes them non-exhaustive, and the compiler will point at them.
- There are **zero tests** for the crossterm adapter (`input/event.rs` crossterm impls, `crates/termrock/src/crossterm/event.rs` helpers `key`/`mouse_kind`).
- Repo conventions: breaking public changes are welcome (AGENTS.md "Modern-first, pre-stable API") but need a migration file in the same commit; Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Compile all features | `cargo check --workspace --all-features --locked` | exit 0 |
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Crossterm-feature tests | `cargo test -p termrock --features crossterm --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/input/event.rs`
- `crates/termrock/src/keymap.rs` (the two `From` impls + the stale doc comment)
- `crates/termrock/src/widgets/text_input.rs`, `list.rs`, `dialog.rs` (release guards only)
- Any other file the compiler flags for the new enum variant (follow the errors; each fix is a one-line non-actionable arm)
- New test module for the adapter (Step 4)
- `migrations/000N-*.md` + `MIGRATING.md` (new public enum variant = breaking for exhaustive matchers)

**Out of scope**:
- Any handler-signature changes (arg order, naming) — Plan 011 owns the event-model convergence; keep signatures exactly as they are.
- `Repeat` handling policy beyond what Step 3 specifies.
- The `Mods`/`KeyModifiers` duplication — Plan 006.

## Git workflow

- Directly on `main`; commit as `fix(input)!: stop collapsing unknown keys into Esc` with sign-off; migration file in the same commit.

## Steps

### Step 1: Add `KeyCode::Unknown`

In `crates/termrock/src/input/event.rs`, add a variant to `KeyCode`:

```rust
    /// A key the neutral vocabulary does not model (function keys, media
    /// keys, lock keys, …). Widgets and keymaps must treat it as
    /// non-actionable: it never matches a binding and never triggers an
    /// outcome.
    Unknown,
```

Change the crossterm adapter's fallthrough from `_ => Self::Esc` to `_ => Self::Unknown`.

**Verify**: `cargo check --workspace --all-features --locked` → fails with non-exhaustive-match errors listing every `match` on `KeyCode` that needs an arm. Record the list; that's Step 2's worklist.

### Step 2: Give every flagged match a non-actionable arm

For each compiler-flagged site: widgets' `handle_key` matches already end in a `_ => …Ignored` arm, so most will not be flagged. Expected flagged sites are the two `From` impls in `keymap.rs` — map `KeyCode::Unknown => LogicalKey::Char('\0')` (exactly what the doc comment always promised; `'\0'` never appears in binding tables — confirm with `grep -n "Char('\\\\0')" crates/`, expect no binding uses it). If other sites appear, choose the arm that makes `Unknown` inert (an `Ignored`-equivalent), never an action.

Also update the stale doc comment at `keymap.rs:138-139` so it now truthfully describes both hops: unknown crossterm keys → `KeyCode::Unknown` → `LogicalKey::Char('\0')`.

**Verify**: `cargo check --workspace --all-features --locked` → exit 0. `cargo test --workspace --all-features --locked` → all pass.

### Step 3: Add the release guard to the three unguarded widgets

At the top of each `handle_key` listed below, insert the guard, matching the exemplar `form.rs:184` (`if !self.active || key.kind == KeyEventKind::Release`) minus the activity flag where the widget has none:

- `text_input.rs` `handle_key`: `if key.kind == KeyEventKind::Release { return TextInputOutcome::Ignored; }`
- `list.rs` `handle_key`: `... return ListOutcome::Ignored;`
- `dialog.rs` `handle_key`: `... return Outcome::Ignored;`

Import `KeyEventKind` where missing. Do **not** filter `Repeat` — held-arrow-key repetition is desired in lists/inputs, and the guarded widgets (`form`/`tree`/`split_pane`) don't filter it either; consistency wins.

**Verify**: `cargo test --workspace --all-features --locked` → all pass.

### Step 4: Test the adapter and the guards

Create `crates/termrock/tests/input_adapter.rs` with `#![cfg(feature = "crossterm")]` at the top (the existing integration tests in `crates/termrock/tests/` show the layout; run under `--features crossterm` like CI's `crossterm-platform` job does):

- `every_mapped_crossterm_key_roundtrips`: table of `(crossterm::event::KeyCode, termrock::input::KeyCode)` pairs for all 15 mapped variants.
- `unmapped_keys_become_unknown`: `F(5)`, `Insert`, `CapsLock`, `Media(...)` all map to `KeyCode::Unknown`, and specifically NOT to `Esc`.
- `unknown_is_inert_in_widgets`: build a `TextInputState`, a `ListState`, a `ChoiceDialogState`; feed `KeyEvent::new(KeyCode::Unknown, KeyModifiers::NONE)`; assert the outcome is the `Ignored` variant of each (regression for the F5-closes-dialog bug).
- `release_events_are_ignored`: same three widgets; a `KeyEvent { code: KeyCode::Enter, kind: KeyEventKind::Release, .. }` yields `Ignored` (not submit/activate).

Note: the pure-neutral tests (`unknown_is_inert_in_widgets`, `release_events_are_ignored`) don't need crossterm — if you prefer, put those two in `crates/termrock/src/widgets/tests.rs` alongside the existing render tests, and keep only the adapter tests feature-gated.

**Verify**: `cargo test -p termrock --features crossterm --locked` → all pass, 4 new tests. `cargo test --workspace --all-features --locked` → all pass.

### Step 5: Migration file

Create the next-numbered `migrations/` file (check `ls migrations/` — next after `0002`, unless Plan 004 already took `0003`) documenting: `KeyCode` gained `Unknown` (breaking for exhaustive matches); unknown keys no longer produce `Esc`; consumers with exhaustive `match` on `KeyCode` must add an `Unknown => /* ignore */` arm. Follow `migrations/0002-v0.8.0-canonical-widget-contracts.md`'s structure. Link from `MIGRATING.md` in the same commit.

**Verify**: `grep -n "Unknown" migrations/000*.md` → present; MIGRATING.md table has the new row.

## Test plan

Covered in Step 4 — 4 new tests; pattern: existing `crates/termrock/tests/*.rs` integration style and `widgets/tests.rs` for neutral-layer tests.

## Done criteria

- [ ] `grep -n "_ => Self::Esc" crates/termrock/src/input/event.rs` → no match
- [ ] `grep -n "Unknown" crates/termrock/src/input/event.rs` → variant present with doc comment
- [ ] All three previously-unguarded `handle_key`s start with a `KeyEventKind::Release` guard (grep each file)
- [ ] `cargo test --workspace --all-features --locked` and `cargo test -p termrock --features crossterm --locked` → all pass, ≥4 new tests
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → exit 0
- [ ] Migration file exists and is indexed
- [ ] `plans/README.md` status row updated

## STOP conditions

- A `grep -rn "KeyCode::Esc" crates/` site turns out to *depend* on the unknown→Esc collapse (e.g. a test asserting F-keys cancel) — report it; that test encodes the bug.
- A binding table anywhere uses `LogicalKey::Char('\0')` (would collide with the inert mapping) — report; pick a different inert representation only after maintainer input.
- Plan 011 has already landed and changed handler signatures — the guard insertion points will differ; re-locate them by symbol name and note the drift in your report.

## Maintenance notes

- Plan 011 (event-model convergence) will fold these guards into a shared entry point; these one-line guards are still correct interim behavior and are trivially absorbed.
- Reviewers of future widgets: every new `handle_key` must start with the release guard — consider that part of the widget contract checklist in `docs/api/component-contracts.json`.
- The kitty-protocol `Repeat` semantics remain unfiltered by design; if a consumer reports double-activation on `Enter` repeat, that's a policy decision to make once, in the shared entry point, not per widget.
