# Plan 006: One key vocabulary — collapse `keymap::{LogicalKey, Mods}` into `input::{KeyCode, KeyModifiers}`

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/keymap.rs crates/termrock/src/input/`
> If these files changed since this plan was written (Plan 005 legitimately
> adds `KeyCode::Unknown` and touches the `From` impls — that exact change is
> expected, not drift), compare the "Current state" excerpts against live code
> before proceeding; on any *other* mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (touches key dispatch; keymap has a strong test suite to catch regressions)
- **Depends on**: plans/005-input-correctness.md (do 005 first so `Unknown` exists before the merge)
- **Category**: tech-debt
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The crate has two parallel vocabularies for the same concept. `input::KeyCode` (15 variants) and `keymap::LogicalKey` (the same 15, minus naming) are near-identical enums kept in lockstep by hand-written `From` impls. Worse, the two modifier bitflag types disagree on bit layout — `input::KeyModifiers` puts SHIFT at bit 0 and CONTROL at bit 1; `keymap::Mods` puts CTRL at bit 0, ALT at bit 1, SHIFT at bit 2. Any future code that compares or serializes raw bits across the boundary is a latent correctness bug, and consumers wiring both the widget layer (`KeyEvent`) and the keymap/hint layer (`KeyChord`) juggle two enums that mean the same thing. One vocabulary, one bit layout, one conversion boundary.

## Current state

- `crates/termrock/src/input/event.rs`:

```rust
pub struct KeyModifiers(u8);
impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const ALT: Self = Self(4);
```

- `crates/termrock/src/keymap.rs` (~line 39):

```rust
/// Modifier flags packed into a `u8`. Bit 0 = Ctrl, bit 1 = Alt, bit 2 = Shift.
pub struct Mods(u8);
impl Mods {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const ALT: Self = Self(2);
    pub const SHIFT: Self = Self(4);
```

  `Mods` also has `with_ctrl()/with_alt()/with_shift()/contains()/is_empty()` (all `const fn`).
- `keymap::LogicalKey` — enum with `Char(char), Enter, Esc, Tab, BackTab, Up, Down, Left, Right, Home, End, PageUp, PageDown, Backspace, Delete` — a 1:1 mirror of `input::KeyCode` (which lists the same set; after Plan 005 it additionally has `Unknown`).
- `keymap::KeyChord { key: LogicalKey, mods: Mods }` with const constructors `plain/ctrl/alt/shift/alt_shift`.
- Two conversion impls in `keymap.rs`: `From<crate::input::KeyEvent> for KeyChord` and `From<crate::input::KeyCode> for KeyChord`, each containing a 15-arm mirror `match`.
- A parallel raw-bytes parser `keymap::raw_bytes_to_chord(bytes: &[u8]) -> Option<KeyChord>` (~line 384) that decodes terminal bytes (CR/LF, ESC, Tab, Backspace, printable ASCII, Ctrl+A..Z, CSI arrows) straight to `KeyChord`, bypassing `input` entirely.
- `KeyChord` constructors are used pervasively in `static` binding tables (e.g. `crates/termrock-lookbook/src/main.rs`, `keymap.rs` test tables) — the `const fn` property must survive.
- Shift-tracking policy (keep it): "Shift is intrinsic to Char casing; only track it for non-Char keys" (comment in the `From<KeyEvent>` impl).
- Tests: `keymap` has 17 tests in `crates/termrock/src/keymap.rs` `mod tests` plus 13 in `crates/termrock-lookbook/src/tests.rs` — your safety net.
- Repo conventions: forward-only breaking changes with a migration file (AGENTS.md); Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Keymap tests | `cargo test -p termrock keymap` | all pass |
| Lookbook tests | `cargo test -p termrock-lookbook` | all pass |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/keymap.rs`
- `crates/termrock/src/input/event.rs` (add the few `const` helpers `Mods` had that `KeyModifiers` lacks)
- Callers of `LogicalKey`/`Mods` across the workspace (compiler-guided; primarily `crates/termrock-lookbook/src/main.rs`, `stories.rs`, `interactors.rs`, `keymap.rs` tests, `widgets/hint_bar.rs` if it names them)
- `migrations/000N-*.md` + `MIGRATING.md`

**Out of scope**:
- Handler signatures and outcome enums — Plan 011.
- Making `Keymap` runtime-configurable (`&'static` stays) — Plan 019 spikes that.
- `raw_bytes_to_chord`'s decode *coverage* (which sequences it understands) — only its output vocabulary changes here.

## Git workflow

- Directly on `main`; `git commit -s -m "refactor(keymap)!: unify key vocabulary on input::KeyCode"` + migration file in the same commit.

## Steps

### Step 1: Port the missing const helpers to `KeyModifiers`

In `input/event.rs`, add to `KeyModifiers` the `const fn` builders `with_ctrl`, `with_alt`, `with_shift` (same shapes as `Mods`'s versions), and `is_empty`. Keep the existing bit layout (SHIFT=1, CONTROL=2, ALT=4) — it is the layout the crossterm adapter already writes.

**Verify**: `cargo check --workspace --all-features --locked` → exit 0.

### Step 2: Replace `LogicalKey` and `Mods` with type aliases, then delete

In `keymap.rs`:

1. Change `KeyChord` to `{ key: crate::input::KeyCode, mods: crate::input::KeyModifiers }`.
2. Update the const constructors (`plain/ctrl/alt/shift/alt_shift`) to build `KeyModifiers` values.
3. Delete the `LogicalKey` enum and `Mods` struct.
4. Delete the mirror `match`es inside `From<KeyEvent> for KeyChord` and `From<KeyCode> for KeyChord` — the key now passes through unchanged; the impls shrink to modifier normalization only (keep the Shift-is-intrinsic-to-Char rule and the Plan-005 `Unknown → inert` doc note; with a unified vocabulary "inert" now means `KeyCode::Unknown` itself flows into `KeyChord` and simply never appears in binding tables — update the doc comment accordingly, replacing the `'\0'` mechanism).
5. Update `raw_bytes_to_chord` internals to produce `KeyCode` variants.
6. Fix all compiler-flagged call sites mechanically (`LogicalKey::X` → `KeyCode::X`; `Mods::CTRL` → `KeyModifiers::CONTROL` etc.). Watch for bit-literal uses: `grep -n "Mods(" crates/ --include="*.rs" -r` — any code constructing `Mods` from a raw integer must be re-expressed with named constants (raw bits would silently change meaning given the different layouts).

**Verify**: `cargo test --workspace --all-features --locked` → all pass (the 30 keymap tests are the regression net). `cargo clippy ... -- -D warnings` → exit 0.

### Step 3: Kill the duplicate glyph/name tables if present

`keymap.rs` derives hint glyphs from chords (`chord_glyph` mentioned in `KeyBinding` docs). Search for any `match` over the old `LogicalKey` used for glyph rendering and re-point it at `KeyCode`. Ensure `KeyCode::Unknown` renders as an empty/„?" glyph and — more importantly — never appears in a binding table (add a debug assertion in `Keymap::new` is NOT possible in `const fn`; instead add a unit test iterating a representative binding table asserting no chord uses `Unknown`).

**Verify**: `cargo test -p termrock keymap` → all pass, including the new no-Unknown-in-bindings test.

### Step 4: Migration file

Next-numbered `migrations/` file: removed `keymap::LogicalKey` (→ `input::KeyCode`), removed `keymap::Mods` (→ `input::KeyModifiers`, note the **bit-layout change** for anyone who serialized raw bits: CTRL was bit 0, is now bit 1; SHIFT was bit 2, is now bit 0), `KeyChord` field types changed. Before/after example:

```rust
// Before
use termrock::keymap::{KeyChord, LogicalKey, Mods};
const QUIT: KeyChord = KeyChord::ctrl(LogicalKey::Char('q'));

// After
use termrock::keymap::KeyChord;
use termrock::input::KeyCode;
const QUIT: KeyChord = KeyChord::ctrl(KeyCode::Char('q'));
```

Link from `MIGRATING.md` in the same commit.

**Verify**: migration file exists, indexed; `mise run gate` (if Plan 001 landed) or the full command chain → exit 0.

## Test plan

- Existing 30 keymap tests must pass unchanged in behavior (mechanical renames inside them are fine).
- New test (Step 3): binding tables contain no `Unknown` chords.
- New test: `KeyChord::from(KeyEvent)` preserves modifier semantics — Ctrl+X with the *new* unified type still dispatches the same action as before (adapt one existing dispatch test to assert via the public `Keymap::dispatch` API).

## Done criteria

- [ ] `grep -rn "LogicalKey\|struct Mods" crates/ --include="*.rs"` → no matches
- [ ] `cargo test --workspace --all-features --locked` → all pass
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → exit 0
- [ ] `KeyChord` constructors still `const fn` (a `static` binding table still compiles — the lookbook's tables prove it)
- [ ] Migration file exists and is indexed
- [ ] `plans/README.md` status row updated

## STOP conditions

- Any code (or test fixture) is found serializing/persisting raw `Mods` bits — the layout change would corrupt it silently; report before proceeding.
- `KeyChord` constructors cannot stay `const fn` for some reason (e.g. a needed conversion isn't const) — report; the `&'static` binding tables depend on const construction.
- Plan 005 has not landed and `KeyCode` lacks `Unknown` — execute 005 first (dependency).

## Maintenance notes

- Plan 019 (runtime-configurable keymaps) builds directly on this unification — do not start it before this lands.
- Reviewers: check that no `match` on the unified `KeyCode` in keymap glyph code treats `Unknown` as actionable.
- The `raw_bytes_to_chord` path still exists as a second entry point; Plan 011's neutral-event work may fold it into the crossterm adapter later — deliberately untouched here.
