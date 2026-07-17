# Plan 004: Harden the OSC encoders — no control byte or unvetted scheme can escape into the terminal stream

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/osc/ crates/termrock/src/geometry.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: security
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The OSC 8 hyperlink encoder interpolates the caller's `url` and `id` into a raw escape sequence with no neutralization. A URL containing an escape terminator (`ESC \`, `BEL`) or other control bytes ends the OSC sequence early and the remaining bytes are interpreted by the terminal as fresh control input — the classic terminal-escape-injection class. Hyperlink URLs are exactly the kind of data that derives from file names, process output, or remote content (the `DetailTable` widget forwards `DetailRow.href` values into these encoders). The same gap applies to the OSC 52 `selection` field, which is a free string instead of the closed set of selection letters the protocol defines. The library already treats control-byte neutralization as its own responsibility for terminal titles (`sanitize_terminal_title`); the OSC 8/52 paths are an unsanitized asymmetry.

This is a security-relevant change: keep the implementation defensive (reject/encode), and never add test fixtures that could function as working attack strings against real terminals — synthetic control bytes are enough.

## Current state

- `crates/termrock/src/osc/encode.rs` — the three encoders (verbatim, current):

```rust
#[must_use]
pub fn encode_pointer(shape: PointerShape) -> Vec<u8> {
    format!("\x1b]22;{}\x1b\\", shape.name()).into_bytes()
}
#[must_use]
pub fn encode_hyperlink_open(id: Option<&str>, url: &str) -> Vec<u8> {
    format!(
        "\x1b]8;{};{url}\x1b\\",
        id.map_or(String::new(), |id| format!("id={id}"))
    )
    .into_bytes()
}
#[must_use]
pub fn encode_hyperlink_close() -> Vec<u8> {
    b"\x1b]8;;\x1b\\".to_vec()
}
#[must_use]
pub fn encode_clipboard(request: ClipboardWrite<'_>) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(request.text.as_bytes());
    format!("\x1b]52;{};{encoded}\x07", request.selection).into_bytes()
}
```

- `crates/termrock/src/osc/request.rs` — the request types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardWrite<'a> {
    pub selection: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request<'a> {
    Pointer(PointerShape),
    Clipboard(ClipboardWrite<'a>),
    HyperlinkOpen { id: Option<&'a str>, url: &'a str },
    HyperlinkClose,
}
```

- The existing in-crate primitive to reuse, `crates/termrock/src/geometry.rs` (~line 96):

```rust
#[must_use]
pub fn is_terminal_control_char(c: char) -> bool {
    let code = c as u32;
    code < 0x20 || c == '\x7f' || (0x80..0xa0).contains(&code)
}
```

- `base64` (0.22) and `PointerShape.name()` paths are already safe — fixed vocabularies / encoded payloads. `encode_hyperlink_close` is a constant — safe.
- The data path that makes `url` consumer-controlled: `crates/termrock/src/widgets/detail_table.rs` exposes `DetailRow.href: Option<&str>` which flows into `HyperlinkRegion.url` and from there to `encode_hyperlink_open`.
- Existing test in `osc/encode.rs` `mod tests`: `encodes_known_requests_exactly` asserts exact output bytes — follow its style.
- Repo conventions: forward-only breaking changes are welcome (AGENTS.md "Forward-only design" / "Modern-first, pre-stable API"); public behavior changes need docs updated in the same commit; `unsafe` is forbidden workspace-wide.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests (module) | `cargo test -p termrock osc -- --nocapture` | all pass |
| Tests (workspace) | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Full gate (if Plan 001 landed) | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/osc/encode.rs`
- `crates/termrock/src/osc/request.rs`
- `crates/termrock/src/osc/mod.rs` (re-exports, module docs)
- `migrations/0003-*.md` + `MIGRATING.md` (only if Step 2's enum change lands — it is a breaking public change)
- `docs/api/public-api.txt` regeneration if the public surface changes and Plan 003's gate is active

**Out of scope**:
- `geometry.rs` — reuse `is_terminal_control_char` as-is; do not move it (Plan 012 owns module layout).
- `widgets/detail_table.rs` — no href validation at the widget layer; the encoder is the single choke point.
- Emission policy — consumers still decide *when* to emit (documented tradeoff); this plan only guarantees *what* is emitted is well-formed.

## Git workflow

- Directly on `main`; Conventional Commits + DCO sign-off. This plan is a breaking change if Step 2 lands: commit as `feat(osc)!: sanitize hyperlink and clipboard encoding` and include the migration file in the same commit (repo rule).

## Steps

### Step 1: Sanitize `encode_hyperlink_open`

In `osc/encode.rs`, before formatting:

1. **URL**: percent-encode every byte for which `is_terminal_control_char` is true (encode conservatively: bytes < 0x21, 0x7F, and 0x80–0x9F become `%XX`; leave all other bytes untouched so already-percent-encoded URLs survive).
2. **Scheme allowlist**: parse the scheme (the prefix before the first `:`; compare ASCII-case-insensitively). Allow `http`, `https`, `mailto`, `file`. For any other scheme — or a URL with no scheme — return the same bytes as `encode_hyperlink_close()` would *not* be right; instead return an empty `Vec<u8>` and document: "an empty vec means the request was rejected; emit nothing." Add `#[must_use]` doc text stating this.
3. **id**: strip (drop) any character where `is_terminal_control_char` is true, plus `;` and `:` (parameter separators in the OSC 8 params field).

Keep the function signature unchanged.

**Verify**: `cargo test -p termrock osc` → existing `encodes_known_requests_exactly` still passes (its fixtures use clean http-style URLs; if it used a bare path without a scheme, adjust the fixture to `file:///...` and note it).

### Step 2: Type the OSC 52 selection

In `osc/request.rs`, replace `pub selection: &'a str` with a closed enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardSelection {
    /// The system clipboard (`c`) — the default and the one terminals honor most.
    Clipboard,
    /// The primary selection (`p`).
    Primary,
}

impl ClipboardSelection {
    pub(crate) const fn letter(self) -> &'static str {
        match self { Self::Clipboard => "c", Self::Primary => "p" }
    }
}
```

Update `ClipboardWrite` to `pub selection: ClipboardSelection`, and `encode_clipboard` to use `request.selection.letter()`. (OSC 52 also defines `s`, `0`–`7`; add them only if a caller in this repo needs them — the closed enum can grow.)

Also add a size bound in `encode_clipboard`: if `request.text.len() > 100_000` (pre-encoding bytes), return an empty vec (rejected), documented the same way as Step 1's rejection. This makes the many-terminals-silently-drop-large-writes failure mode explicit.

**Verify**: `cargo check --workspace --all-features --locked` → exit 0 after fixing all in-repo callers (grep `ClipboardWrite` to find them; expected: osc tests and possibly lookbook).

### Step 3: Tests

Add to the existing `mod tests` in `osc/encode.rs`, following `encodes_known_requests_exactly`'s exact-bytes style:

- `hyperlink_url_control_bytes_are_percent_encoded`: a URL containing `\x1b` and `\x07` produces output whose byte string contains `%1B`/`%07` and contains exactly one `\x1b]8;` introducer and one `\x1b\\` terminator.
- `hyperlink_disallowed_scheme_is_rejected`: a `javascript:`-schemed input returns an empty vec. (Use the scheme name only; no payload after the colon.)
- `hyperlink_id_strips_separators_and_controls`: id with `;`, `:`, `\x1b` produces params without them.
- `clipboard_selection_is_typed`: `ClipboardSelection::Clipboard` encodes `\x1b]52;c;...`.
- `clipboard_oversized_write_is_rejected`: text > bound returns empty vec.

**Verify**: `cargo test -p termrock osc` → all pass, including 5 new tests.

### Step 4: Migration file (breaking change from Step 2)

Create `migrations/0003-v0.9.0-typed-osc-requests.md` following the exact structure of `migrations/0002-v0.8.0-canonical-widget-contracts.md` (sections: Boundary, Old to new table, before/after code blocks, Removed concepts, Validation). Content: `ClipboardWrite.selection: &str` → `ClipboardSelection` enum; rejected-request semantics (empty vec) for hyperlink scheme violations and oversized clipboard writes. Add the row to `MIGRATING.md`'s table in the same commit. (Version label: match however the maintainer versions the next boundary; if unclear, title it `0003-unreleased-typed-osc-requests.md` and STOP-note it.)

**Verify**: `grep -n "0003" MIGRATING.md` → row present.

## Test plan

- New tests: 5 named in Step 3, in `crates/termrock/src/osc/encode.rs` `mod tests`.
- Pattern to follow: existing `encodes_known_requests_exactly` (exact byte-string assertions).
- Verification: `cargo test --workspace --all-features --locked` → all pass; count increases by 5.

## Done criteria

- [x] `cargo test --workspace --all-features --locked` → all pass, 5 new osc tests present
- [x] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → exit 0
- [x] `grep -n "selection: &" crates/termrock/src/osc/request.rs` → no match (field is now the enum)
- [x] No raw interpolation of caller strings remains: manual read of `encode_hyperlink_open` confirms url passes through percent-encoding and scheme check before `format!`
- [x] Migration file `migrations/0004-v0.9.0-typed-osc-requests.md` exists and is linked from `MIGRATING.md` (`0003` was already claimed)
- [x] `git status` → no unrelated files attributable to Plan 004
- [x] `plans/README.md` status row updated

## STOP conditions

- The existing exact-bytes test fixture depends on an unschemed URL and changing it breaks a downstream contract you can see (grep `component-contracts.json` and lookbook stories for hyperlink fixtures) — report before changing fixtures.
- You find additional raw-interpolation sinks in `osc/` beyond the three encoders shown (the module may have grown) — report them; extend the same treatment only if they are string-typed.
- Any test would require embedding a realistic multi-step attack payload — don't; synthetic single control bytes are sufficient, report if you believe more is needed.

## Maintenance notes

- Future OSC additions (e.g. OSC 9 notifications) must route caller strings through the same percent-encode/reject helpers — reviewers should look for `format!("\x1b]"` patterns touching `&str` parameters.
- The empty-vec rejection contract is load-bearing: emitting code must treat empty as "skip write", never as "write empty sequence". If a `Result`-based API is preferred later, that's a compatible follow-up breaking change (forward-only policy).
- Plan 012 will move/dedupe `PointerShape`; it does not touch these encoders' sanitization.
