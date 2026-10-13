# Plan 035: Complete the neutral input contract — carry paste text, degrade unknown backend events, pin the theme presets

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5c4758b..HEAD -- crates/termrock/src/input/event.rs crates/termrock/src/style/mod.rs crates/termrock/src/widgets/text_input.rs`
> The repo has concurrent executors — expect drift; re-locate every excerpt by
> symbol before editing. STOP only if a named symbol no longer exists.

## Status

- **Priority**: P1
- **Effort**: S-M
- **Risk**: LOW
- **Depends on**: none (011/013 already landed)
- **Category**: bug
- **Planned at**: commit `5c4758b`, 2026-07-16

## Why this matters

Three defects survived the (otherwise clean) event-contract and theming waves. (1) `Event::Paste` is a unit variant and the crossterm adapter writes `Event::Paste(_) => Self::Paste` — the pasted `String` is dropped, unrecoverable through the neutral path, even though the `Session` enables bracketed paste; a library that ships `TextInput` cannot deliver pasted text to it. (2) The `From<crossterm::event::Event>` impl matches all six crossterm variants exhaustively with no `_` arm, so `Event::Unknown` (documented as "a backend event outside the neutral vocabulary") is unreachable — and a future crossterm release adding a variant becomes a compile error instead of degrading, the opposite of what the doc promises and of the `KeyCode::Unknown` precedent. (3) The two theme presets are hand-ordered positional `[Style; 37]` arrays with no value-pinning test — swapping Success↔Warning entries compiles and passes the whole suite while shipping wrong colors; and if a `Role` variant is added without extending `roles()`, `style()`/`with_role()` index out of bounds at runtime.

## Current state

- `crates/termrock/src/input/event.rs` — the `Event` enum (~line 195; `#[non_exhaustive]`): variants `Key(KeyEvent), Mouse(MouseEvent), Paste, Resize { width, height }, FocusGained, FocusLost, Unknown`. The adapter (~line 317, inside the crossterm cfg module):

```rust
    impl From<crossterm::event::Event> for Event {
        fn from(value: crossterm::event::Event) -> Self {
            match value {
                crossterm::event::Event::Key(event) => Self::Key(event.into()),
                crossterm::event::Event::Mouse(event) => Self::Mouse(event.into()),
                crossterm::event::Event::Paste(_) => Self::Paste,
                crossterm::event::Event::Resize(width, height) => Self::Resize { width, height },
                crossterm::event::Event::FocusGained => Self::FocusGained,
                crossterm::event::Event::FocusLost => Self::FocusLost,
            }
        }
    }
```

- `Event` currently derives `Debug, Clone, Copy, PartialEq, Eq, Hash` — adding `Paste(String)` forces dropping `Copy` (and `Hash`? String is Hash — keep Hash, drop Copy). Check all in-repo uses of `Event` by value (grep `input::Event` — the adapter tests in `tests/input_adapter.rs` and possibly nothing else; the type has no runtime consumers yet).
- `crates/termrock/src/widgets/text_input.rs` — `TextInputState` edits via `EditAction::Insert(char)` per keystroke; no paste-insert API (`grep -n "fn paste\|fn insert_str" crates/termrock/src/widgets/text_input.rs` → confirm empty).
- `crates/termrock/src/style/mod.rs` — `tailrocks_phosphor()` and `slate()` are positional 37-element literals; `pub const fn roles() -> [Role; 37]` lists all variants in order; existing tests (~line 322+): `roles_cover_the_positional_theme_array` (checks discriminant order only) and `default_is_the_phosphor_preset`.
- Existing exemplar for adapter tests: `crates/termrock/tests/input_adapter.rs` (feature-gated `crossterm`, mapping tables).
- Repo conventions: Conventional Commits + DCO; trunk-only; breaking public change (Paste variant shape) → next-numbered `migrations/` file + `MIGRATING.md` row same commit (check `ls migrations/` — 0011 was last at planning time); `missing_docs = "deny"` is active — new/changed public items need real doc comments (NOT the placeholder style; see plan 036); regenerate `docs/api/public-api.txt` (CI gate diffs it).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Fast check | `mise run check` | exit 0 |
| Adapter tests | `cargo test -p termrock --features crossterm --test input_adapter` | all pass |
| Style tests | `cargo test -p termrock style` | all pass |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/input/event.rs`
- `crates/termrock/src/widgets/text_input.rs` (paste-insert method only)
- `crates/termrock/src/style/mod.rs` (tests only)
- `crates/termrock/tests/input_adapter.rs`
- `migrations/00NN-*.md` + `MIGRATING.md`; `docs/api/public-api.txt` regen

**Out of scope**:
- Porting the lookbook loop to `input::Event` — that is Plan 018's prototype step (the dogfooding gap is recorded there); do not wire consumers here.
- Multi-line paste semantics in TextInput (single-line widget: strip/stop at newline — pick strip-`\n`-to-space? NO: truncate at first newline and document; TextArea handles real multi-line later — Plan 034).
- Any Role additions.

## Git workflow

- Directly on `main`; `git commit -s -m "fix(input)!: carry paste text and degrade unknown backend events"` (+ the style-test commit may ride along or split: `test(style): pin preset role values`).

## Steps

### Step 1: `Paste(String)` end to end

Change the variant to `Paste(String)` (real doc comment: "Bracketed-paste text from the backend. Multi-line handling is the consumer's/widget's concern."). Drop `Copy` from `Event`'s derives (keep `Clone, Debug, PartialEq, Eq, Hash`). Adapter arm: `crossterm::event::Event::Paste(text) => Self::Paste(text)`. Fix compiler-flagged uses.

Add `TextInputState::insert_str(&mut self, text: &str) -> TextInputOutcome` (or `paste(&str)` — match the crate's naming register; check existing method names first): grapheme-safe insertion at the cursor honoring `max_graphemes`/`forbidden` exactly like repeated `EditAction::Insert`, truncating at the first `\n`/`\r` (documented).

**Verify**: `cargo test --workspace --all-features --locked` → all pass.

### Step 2: `_ => Self::Unknown` degradation arm

Add the trailing arm to the `From<crossterm::event::Event>` match so unmodeled/future backend variants map to `Event::Unknown` (mirroring the `KeyCode` adapter). Update `Unknown`'s doc to state it is the live degradation path. NOTE: with all six current variants still explicitly matched, the `_` arm is currently unreachable — the compiler may warn `unreachable_pattern`; if so, this is exactly the case for `#[allow(unreachable_patterns)]` with a comment "future-proofing against new backend variants" (crossterm's Event is non_exhaustive upstream? check: if crossterm marks it non_exhaustive the `_` arm is REQUIRED-or-allowed naturally and no warning fires — verify by compiling).

**Verify**: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → exit 0.

### Step 3: Adapter + paste tests

Extend `tests/input_adapter.rs`: `paste_carries_text` (crossterm Paste("héllo🧪") → `Event::Paste(s)` with s intact); `text_input_accepts_pasted_text` (insert_str at mid-cursor, grapheme-correct cursor after; newline-truncation case; max_graphemes clamp case).

**Verify**: `cargo test -p termrock --features crossterm --test input_adapter` → all pass, ≥3 new.

### Step 4: Pin the presets

In `style/mod.rs` tests: (a) a table test per preset asserting the load-bearing role→style pairs (at minimum: `Text`, `Border`, `BorderFocused`, `Selection`, `Success`, `Warning`, `Danger`, `Link`, `Input`, `ScrollThumb`, `TabActive`, `HintKey`, `DiffAdded`, `DiffRemoved`) against their current concrete values — read the current literals and pin exactly what ships; (b) a guard that the LAST `Role` variant `as usize == Theme::roles().len() - 1` (so adding a Role without extending `roles()` and the arrays fails a test before it can panic at runtime). Prove the table bites: temporarily swap two entries locally → test fails → restore.

**Verify**: `cargo test -p termrock style` → new tests pass; the swap experiment result stated in your report.

### Step 5: Migration + regen

Migration file (next free number — check `ls migrations/`): `Event::Paste` → `Event::Paste(String)`; `Event` no longer `Copy`; before/after match-arm example. Regenerate `public-api.txt`.

**Verify**: `mise run gate` → exit 0.

## Test plan

Steps 3–4: ≥3 adapter/paste tests + 2 preset-pinning tests (with the bite-proof swap experiment).

## Done criteria

- [x] `grep -n "Paste(_)" crates/termrock/src/input/event.rs` → no match; `Paste(text) => Self::Paste(text)` present
- [x] `_ => Self::Unknown` (or non_exhaustive-forced equivalent) in the Event adapter; clippy clean
- [x] `TextInputState` paste-insert method exists, grapheme-tested
- [x] Preset value table + Role-length guard tests pass; swap experiment demonstrated
- [x] Migration indexed; `public-api.txt` regenerated; `mise run gate` → exit 0
- [x] `plans/README.md` status row updated

## STOP conditions

- Dropping `Copy` from `Event` breaks a consumer that spreads events by value in a way requiring design changes (unlikely — zero runtime consumers at planning time) — report it.
- crossterm's `Event` is NOT non_exhaustive and clippy forbids the unreachable arm even with allow — report; the degradation goal may need a different mechanism.

## Maintenance notes

- Plan 018's runner must consume `input::Event` (recorded there) — paste routing through widgets becomes real at that point.
- The preset pin table must be extended whenever a Role is added (the length guard forces acknowledgment).
- TextArea (Plan 034) supersedes the newline-truncation rule for multi-line consumers.
