# Plan 025: Decouple cursor-hide and line-wrap from alternate-screen in `Session`

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/crossterm/session.rs`
> On mismatch with "Current state" excerpts, STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (additive option fields; restore path already tracks the two flags independently)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

`SessionOptions` gates BOTH `DisableLineWrap` and `Hide` (cursor) on the single `alternate_screen` flag. Two configurations are therefore impossible: an inline TUI (`alternate_screen: false`) cannot hide the cursor or disable wrapping — the blinking cursor and terminal line-wrap sit on top of the inline UI; and an alt-screen surface is force-fed a hidden cursor — an alt-screen editor wanting a visible terminal cursor can't have one. The struct offers `alternate_screen` as if inline mode were supported, but inline mode is only half-configured. COMPONENTS.md documents the intended contract ("Disabling alternate-screen ownership also omits its full-screen line-wrap and cursor changes for inline/non-interactive integrations") — that doc describes the current coupling as intentional for *omission*, but provides no way to opt IN independently, which is the gap.

## Current state

- `crates/termrock/src/crossterm/session.rs:13-19`:

```rust
pub struct SessionOptions {
    pub alternate_screen: bool,
    pub mouse_capture: bool,
    pub bracketed_paste: bool,
    pub raw_mode: bool,
}
```

  `Default` sets all four `true`.
- The coupling (~lines 70-77):

```rust
            if options.alternate_screen {
                session.line_wrap_disabled = true;
                execute!(&mut session.writer, DisableLineWrap)?;
            }
            if options.alternate_screen {
                session.cursor_hidden = true;
                execute!(&mut session.writer, Hide)?;
            }
```

- `restore()` (~87-120) already tracks `cursor_hidden` and `line_wrap_disabled` as independent flags and restores each in reverse order with first-error capture — teardown needs NO change.
- Session is feature-gated (`crossterm`); tests exist (`compatibility.toml` references `cargo test -p termrock --features crossterm --lib session::tests`) — read `session.rs`'s test module for the fixture pattern (it uses an in-memory writer; follow it).
- Repo conventions: public struct field additions are breaking for struct-literal constructors (SessionOptions has pub fields) → migration file needed; note `#[non_exhaustive]` interaction with Plan 013 (if 013 landed, `SessionOptions` may already be non_exhaustive with a builder — adapt: add builder methods instead of raw fields).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Session tests | `cargo test -p termrock --features crossterm session` | all pass |
| Crossterm suite | `cargo test -p termrock --features crossterm --locked` | all pass |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |

## Scope

**In scope**:
- `crates/termrock/src/crossterm/session.rs`
- `crates/termrock/COMPONENTS.md` (one sentence updating the Session contract)
- `migrations/000N-*.md` + `MIGRATING.md`; `public-api.txt` regen

**Out of scope**:
- Event handling, runtime, examples (Plan 014's showcase may later exercise the new options — not here).
- Any new Session capability beyond the two flags (e.g. title setting, focus reporting — separate decisions).

## Git workflow

- Directly on `main`; `git commit -s -m "feat(crossterm)!: independent cursor and line-wrap session options"`.

## Steps

### Step 1: Add the two options

Add `pub hide_cursor: bool` and `pub disable_line_wrap: bool` to `SessionOptions`; `Default` sets both `true` (preserving today's default-alt-screen behavior exactly). Change the two acquisition blocks to gate on the new fields instead of `alternate_screen`. Inline consumers get their opt-in; alt-screen consumers get their opt-out. (If Plan 013 already made this struct builder-based/non_exhaustive, add the fields privately + builder methods in the same style.)

**Verify**: `cargo test -p termrock --features crossterm session` → existing tests pass (default behavior unchanged — the flags default true).

### Step 2: Tests

Following the existing session test fixture (in-memory writer, asserting the escape byte stream):

1. `inline_session_can_hide_cursor`: `alternate_screen: false, hide_cursor: true, disable_line_wrap: false, ...` → acquisition stream contains the Hide sequence, no alt-screen enter, no DisableLineWrap; restore shows Show.
2. `alt_screen_with_visible_cursor`: `alternate_screen: true, hide_cursor: false` → no Hide in the stream; restore has no Show.
3. `defaults_unchanged`: `SessionOptions::default()` byte stream identical to the pre-change expectation (pin the full acquisition sequence).

**Verify**: `cargo test -p termrock --features crossterm session` → 3 new tests pass.

### Step 3: Docs + migration

Update the COMPONENTS.md Session paragraph: cursor and line-wrap are now independent options defaulting on; inline integrations may enable them without the alternate screen. Migration file: `SessionOptions` gained two fields (breaking for struct literals without `..Default::default()`); before/after literal example. Regenerate `public-api.txt`.

**Verify**: `mise run gate` → exit 0; migration indexed.

## Test plan

Step 2's three byte-stream tests; suite green.

## Done criteria

- [x] `grep -n "if options.alternate_screen" crates/termrock/src/crossterm/session.rs` → exactly one hit (the alt-screen block itself)
- [x] 3 new session tests pass; full crossterm + workspace suites green
- [x] COMPONENTS.md updated; migration file indexed; `public-api.txt` regenerated
- [x] `plans/README.md` status row updated

## STOP conditions

- The session test fixture can't observe the escape stream (writer abstraction differs from the excerpt's implication) — read the existing tests first; if they assert state flags only, assert flags + add stream assertions only if the writer is capturable; report if neither works.
- Restore ordering for the new combinations produces terminal-state anomalies in the existing partial-failure tests — report; do not reorder restore without understanding the reverse-order invariant.

## Maintenance notes

- Plan 014's showcase example should eventually demonstrate an inline-mode session (`alternate_screen: false, hide_cursor: true`) — the first real consumer of this fix.
- Any future Session option must follow this pattern: its own flag, its own tracked restore bit, reverse-order teardown.
