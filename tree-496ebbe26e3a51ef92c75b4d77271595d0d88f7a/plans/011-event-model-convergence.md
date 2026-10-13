# Plan 011: One event contract — neutral events, state-owned handlers, one outcome vocabulary

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/widgets/ crates/termrock/src/input/ crates/termrock/src/interaction/ crates/termrock/src/lib.rs`
> Plans 005/006/007 legitimately touched input, keymap, guards, and tests.
> Verify their status rows in plans/README.md are DONE before starting. Other
> unexplained mismatches with "Current state" = STOP.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED-HIGH (touches every interactive widget's public methods; characterization tests from Plan 007 are the net)
- **Depends on**: plans/005-input-correctness.md, plans/006-key-vocabulary-unification.md, plans/007-characterization-tests-form-detail-table.md
- **Category**: tech-debt
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

Every widget invents its own interaction API. Argument order flips between widgets (`list.handle_key(rows, key)` vs `dialog.handle_key(key, actions)`); pointer methods answer to three naming families (`click`, `activate_at`, `pointer_down`); receivers split between state-owned (`ListState::handle_key(&mut self, …)`) and widget-owned (`SplitPane::handle_key(&self, state, …)`); scroll deltas are `isize` in one widget and `i32` in two others with different return types; and there are **eight** outcome enums, two of them (`interaction::Outcome<T>` and `ListOutcome<Id>`) variant-for-variant identical, plus a crate-root `ModalOutcome<T>` that nothing uses at all. There is also no backend-neutral way to express a mouse click: neutral `MouseEventKind` has only `ScrollUp/ScrollDown/ScrollLeft/ScrollRight/Moved`, so button presses can only be represented by crossterm's own types — breaking the crate's backend-neutrality claim. A consumer cannot learn one convention and apply it; an alternate-backend consumer cannot even feed input. This plan converges on one contract.

## Current state

Verbatim signatures (all under `crates/termrock/src/widgets/`):

```rust
// list.rs:78  (state-owned, data-then-key)
pub fn handle_key(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> ListOutcome<Id>
// list.rs:174/184
pub fn hover(&mut self, position: Position) -> Option<&Id>
pub fn click(&mut self, position: Position) -> ListOutcome<Id>
// list.rs:196
pub fn scroll_by(&mut self, delta: isize, rows_len: usize) -> bool   // (confirm exact param name)

// tree.rs:151/173/182 — same family as list; tree.rs:116 scroll_by(delta: i32, node_count) -> usize

// dialog.rs:238  (state-owned, KEY-then-data — inconsistent)
pub fn handle_key(&mut self, key: KeyEvent, actions: &[Action<'_, Id>]) -> Outcome<Id>
// dialog.rs:292
pub fn activate_at(&mut self, position: ratatui_core::layout::Position) -> Outcome<Id>

// form.rs:179/203/212 — handle_key(...) multi-line, hover(Position), click(Position) -> FormOutcome<Id>
// form.rs:149 scroll_by(&mut self, delta: i32) -> usize

// split_pane.rs:210+  (WIDGET-owned: state passed in)
pub fn handle_key(&self, state: &mut SplitPaneState, key: KeyEvent) -> SplitPaneOutcome
pub fn pointer_down(&self, state: &mut SplitPaneState, position: Position) -> SplitPaneOutcome
pub fn pointer_move(&self, state: &mut SplitPaneState, position: Position) -> bool
pub fn pointer_drag(&self, state: &mut SplitPaneState, position: Position) -> SplitPaneOutcome

// detail_table.rs — NO handle_key; state-owned select_next/select_previous(rows),
// hover_at(Position), activate_at(Position), activate_link_at(Position);
// widget-owned outcome_at(...), activate_selected(&DetailTableState)

// status_bar.rs:42/52 — hover(&mut self, Position), activate_at(&self, Position) -> Outcome<Id>

// text_input.rs:135 — handle_key(&mut self, key: KeyEvent) -> TextInputOutcome
```

Outcome enums:

```rust
// interaction/mod.rs:60 — the generic one
pub enum Outcome<T> { Ignored, Changed, Activated(T), Cancelled }
// list.rs:29 — byte-identical duplicate
pub enum ListOutcome<Id> { Ignored, Changed, Activated(Id), Cancelled }
// form.rs:39
pub enum FormOutcome<Id> { Ignored, FocusChanged(Id), Activated(Id) }
// detail_table.rs:52
pub enum DetailTableOutcome<Id> { Ignored, Selected(Id), Copy(Id), ActivateLink(Id) }
// plus TextInputOutcome (text_input.rs:35), TreeOutcome (tree.rs:34), SplitPaneOutcome (split_pane.rs:69)
// lib.rs:39 — ZERO consumers (verified):
pub enum ModalOutcome<T> { Continue, Commit(T), Cancel }
```

Neutral mouse gap (`input/event.rs`):

```rust
pub enum MouseEventKind { ScrollUp, ScrollDown, ScrollLeft, ScrollRight, Moved }
// crossterm adapter: MouseEventKind::Down/Up/Drag(_) all collapse to `_ => Self::Moved`
```

And `crates/termrock/src/crossterm/event.rs:1` re-exports concrete crossterm types into the public API: `pub use crossterm::event::{Event, KeyEvent, MouseEvent};`.

Design constraints to honor (from repo docs, verbatim):
- `crates/termrock/AGENTS.md`: "Backend-neutral by design: token types, component state, and render helpers stay free of a specific backend."
- Migration 0002 established the direction: "state-owned keyboard, hover, scroll, activation, and painted regions" — `List`/`Tree` are the canonical winners; `SplitPane`/`DetailTable`'s widget-owned methods are the stragglers.
- Root `AGENTS.md` "Forward-only design": one coherent breaking redesign, no aliases.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Crossterm tests | `cargo test -p termrock --features crossterm --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Full gate | `mise run gate` (if Plan 001 landed) | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/input/event.rs` (neutral `MouseButton`, extended `MouseEventKind`, new `MouseEvent`, new top-level `Event`)
- `crates/termrock/src/interaction/mod.rs` (`Outcome` docs)
- All files in `crates/termrock/src/widgets/` with interaction methods
- `crates/termrock/src/lib.rs` (delete `ModalOutcome`)
- `crates/termrock/src/crossterm/event.rs` (adapters produce neutral types; stop re-exporting crossterm types)
- Lookbook + examples + tests call sites (compiler-guided)
- `migrations/000N-*.md` + `MIGRATING.md`; `docs/api/public-api.txt` regen; `docs/api/component-contracts.json` if method names it lists change

**Out of scope**:
- `runtime` module (`Component`/`drive_frame`) — Plan 018 decides its fate; do not implement `Component` for widgets here.
- Theming, rendering, layout logic — signatures only, behavior pinned by Plan 007's tests.
- Keymap internals (Plan 006 already unified vocabulary).

## Git workflow

- Directly on `main`. This is the largest breaking change in the series: land as ONE commit (`feat(widgets)!: converge on the canonical event contract`) with the migration file, or as a short series where each commit compiles and tests green (preferred; e.g. commit A = neutral mouse events, commit B = receiver/naming convergence, commit C = outcome consolidation). Never leave `main` red between commits.

## Steps

### Step 1: Complete the neutral event vocabulary

In `input/event.rs` add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle }

// extend MouseEventKind (append variants):
pub enum MouseEventKind {
    ScrollUp, ScrollDown, ScrollLeft, ScrollRight, Moved,
    Down(MouseButton), Up(MouseButton), Drag(MouseButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub position: ratatui_core::layout::Position,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste,        // marker; paste text stays consumer-owned
    Resize { width: u16, height: u16 },
    FocusGained, FocusLost,
}
```

Update the crossterm `From<crossterm::event::MouseEventKind>` impl to map `Down/Up/Drag(button)` to the new variants (map crossterm's `MouseButton` correspondingly) and add `From<crossterm::event::MouseEvent> for input::MouseEvent` and `From<crossterm::event::Event> for input::Event` (unknown crossterm event kinds → a non-actionable mapping; follow the `KeyCode::Unknown` precedent from Plan 005 — add `Event::Unknown` if needed).

In `crossterm/event.rs`, delete `pub use crossterm::event::{Event, KeyEvent, MouseEvent};` and re-point the helper fns (`key`, `mouse_kind`) or delete them if the `From` impls make them redundant (check callers first: `grep -rn "crossterm::event::\|crossterm::{" crates/termrock-lookbook crates/termrock/examples`).

**Verify**: `cargo test -p termrock --features crossterm --locked` → all pass (Plan 005's adapter tests must be extended for the button mappings — extend them now).

### Step 2: Converge receivers and names

Canonical contract (winner = migration-0002 direction, `List`/`Tree` shape), applied to every interactive widget:

- Methods live on the **State** type, `&mut self`.
- Signature order: **data slice first, event last** — `handle_key(&mut self, data, key: KeyEvent) -> XOutcome`.
- Pointer family names: `hover(&mut self, Position) -> Option<&Id>`, `click(&mut self, Position) -> XOutcome`, plus widget-specific extras keep the `click`-family naming (`click_link` replaces `activate_link_at`; `SplitPane`'s drag family becomes `drag_start`/`drag_move`/`drag_end` on `SplitPaneState`).
- `scroll_by(&mut self, delta: isize, content_len: usize) -> bool` everywhere (changed-or-not; `Form`'s no-len variant gains the len param for consistency — it clamps against content length like the others).

Concrete renames:
- `dialog.rs`: `handle_key(key, actions)` → `handle_key(actions, key)`; `activate_at` → `click`.
- `status_bar.rs`: `activate_at` → `click` (and make it `&mut self` for symmetry).
- `detail_table.rs`: move `outcome_at`/`activate_selected` from the widget to `DetailTableState` (they read widget data — pass `rows: &[DetailRow<…>]` as the data-first param); `hover_at` → `hover`; `activate_at` → `click`; `activate_link_at` → `click_link`; ADD `handle_key(&mut self, rows, key)` wrapping `select_next`/`select_previous` + Enter→`activate_selected` (closing the no-keyboard asymmetry — contract change: update `docs/api/component-contracts.json`'s `DetailTable.keyboard` from `caller-owned` to `covered`).
- `split_pane.rs`: `handle_key(&self, state, key)` → `SplitPaneState::handle_key(&mut self, spec: &SplitPane<'_>, key)` (data-first: the widget carries direction/minima the state math needs — pass what the current implementation actually reads; inspect the bodies and pass the minimal borrowed struct).
- `tree.rs`: `scroll_by(delta: i32, node_count) -> usize` → the canonical `isize`/`bool` shape; `form.rs:149` likewise.

Update all call sites (lookbook `interactors.rs`/`main.rs`, tests). Plan 007's characterization tests are intentionally affected: update them mechanically (same behavior, new names); any *assertion* change is a semantic change and must be listed in the migration file.

**Verify**: `cargo test --workspace --all-features --locked` → all pass. Preview check → zero diffs (no render changes).

### Step 3: Consolidate outcomes

- Delete `ModalOutcome` from `lib.rs` (zero consumers, verified).
- Delete `ListOutcome`; `ListState` returns `interaction::Outcome<Id>` (variant-identical — pure rename for consumers).
- Keep genuinely-distinct enums but align variant naming with `Outcome`'s vocabulary: every enum uses `Ignored` for no-op (they already do), `Cancelled` where escape applies. `DetailTableOutcome`/`TreeOutcome`/`SplitPaneOutcome`/`FormOutcome`/`TextInputOutcome` stay (their extra variants are real) — re-export them all from `widgets::mod` alongside `interaction::Outcome` so consumers have one import surface (`grep -n "pub use" crates/termrock/src/widgets/mod.rs` to match the existing re-export style).

**Verify**: `grep -rn "ModalOutcome\|ListOutcome" crates/ --include="*.rs"` → no matches. Full suite green.

### Step 4: Migration file + docs

Next-numbered migration file with the complete old→new method table (every rename from Step 2, the deleted enums, the new neutral `Event`/`MouseEvent`, the crossterm re-export removal — consumers now write `termrock::input::Event::from(crossterm_event)`), before/after for one keyboard and one mouse wiring, and the `DetailTable` keyboard-contract change. Regenerate `public-api.txt`; update `component-contracts.json` (`DetailTable.keyboard: covered`).

**Verify**: `cd docs && bun run build` → exit 0 (catalog gate consumes the contracts file); `mise run gate` → exit 0.

## Test plan

- Extend Plan 005's adapter tests: mouse button mapping table (crossterm Down/Up/Drag × 3 buttons → neutral variants).
- New: one `input::Event` end-to-end test — build a synthetic `crossterm::event::Event::Mouse(...)`, convert, feed `ListState::click` via the position, assert `Outcome::Activated`.
- Plan 007's characterization suites re-pass with mechanical renames only (report any assertion that had to change semantically).

## Done criteria

- [x] Every interactive widget's methods live on its State with data-first/event-last order (manual grep sweep: `grep -n "pub fn handle_key" crates/termrock/src/widgets/*.rs` shows uniform shapes)
- [x] `grep -rn "activate_at\|hover_at\|pointer_down\|pointer_drag\|ModalOutcome\|ListOutcome" crates/ --include="*.rs"` → no matches
- [x] Neutral `Event`/`MouseEvent`/`MouseButton` exist; crossterm types no longer re-exported (`grep -n "pub use crossterm" crates/termrock/src/` → empty)
- [x] `cargo test --workspace --all-features --locked` + crossterm-feature tests → all pass
- [x] Preview check → zero diffs; migration file indexed; `public-api.txt` + `component-contracts.json` updated
- [x] `plans/README.md` status row updated

## STOP conditions

- Plan 007's tests are not present/passing at start (`cargo test -p termrock --test detail_table`) — the net is missing; run 007 first.
- `SplitPane`'s key/drag math turns out to need owned widget state that cannot be passed as a borrowed param without redesigning `SplitPaneState` internals — report with the specific fields; a targeted design note beats an improvised restructure.
- Any characterization assertion must change to make the new shape work (semantic change) and you cannot tell whether it's intended — list it and stop rather than guessing.
- The diff exceeds ~25 files outside tests/lookbook — scope has grown beyond the plan; report progress and remaining sites.

## Maintenance notes

- This establishes THE widget interaction contract: State-owned, data-first, `Outcome`-vocabulary, neutral events. New widgets must match; reviewers should treat deviations as defects. Consider encoding it in `docs/api/component-contracts.json` review notes.
- Plan 018's runtime/app-runner spike consumes the neutral `Event` — it becomes viable only after this lands.
- `Paste` carrying its text (`Paste(String)`) was deliberately deferred — neutral `Event` is `Copy` today; revisit when a consumer needs paste routing through widgets.
