# Plan 017: Test the untested core math (geometry, ANSI parser, focus ring, diff) and fix the small correctness gaps found beside them

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/geometry.rs crates/termrock/src/text/ crates/termrock/src/ansi_text.rs crates/termrock/src/interaction/focus_owner.rs crates/termrock/src/widgets/diff.rs crates/termrock/src/layout/dialog.rs`
> Plan 012 may have MOVED geometry functions into `text/` — that's fine;
> test the functions at their current home. STOP only if a function named
> below no longer exists anywhere (`grep -rn "fn <name>" crates/`).

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW (tests + two S-sized fixes)
- **Depends on**: none (runs any time; file paths assume pre-012 layout, adapt if 012 landed)
- **Category**: tests
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

`geometry.rs` — 329 lines of display-width, hit-testing, and scroll-segment math consumed by every widget — has **zero tests**; its own doc comments warn that wide/combining chars drift hit-tests, and nothing asserts they don't. The ANSI SGR parser has a full dispatcher (8/bright/256-color/truecolor, modifiers, resets) covered by exactly two happy-path tests. The focus ring (`focus_owner.rs`, modular ring arithmetic — a classic off-by-one site) and the `DiffView` renderer have zero tests; `DiffView` additionally renders text without control-byte sanitization and never clamps `state.offset`. And `DialogBodyScroll` clamps with truncating `as u16` casts where the rest of the crate saturates. This plan adds the regression net and fixes the two small defects it exposes.

## Current state

- Geometry functions to cover (pre-012 home `crates/termrock/src/geometry.rs`; post-012 split across `text/`, `layout`, `widgets::tabs`): `display_cols` (~106), `take_display_cols` (~121), `display_cols_slice` (~142), `leading_space_cols` (~166), `padded_line_display_cols` (~188), `fixed_prefix_scroll_segments` (~212; zero-width combining-char coalescing at ~234-240), `sanitize_terminal_title` (~270), `is_terminal_control_char` (~96), `centered_rect` (~56; uses `saturating_sub(2)`), `lay_out_tabs` (~301), `tab_at_column` (~325), `hint_row_cols` (~87). Zero `#[test]` in the file (verified).
- `sanitize_terminal_title` (verbatim core):

```rust
    for ch in title.chars() {
        if ch.is_control() || ch == '\u{7f}' || ch.is_whitespace() {
            if !prev_space { out.push(' '); prev_space = true; }
        } else { out.push(ch); prev_space = false; }
    }
```

- ANSI parser (`crates/termrock/src/ansi_text.rs`): SGR dispatch handles `0,1,2,22,30-37,39,40-47,49,90-97,100-107,38/48 (5;n and 2;r;g;b via parse_extended_color with .min(255) clamps)`, unknown codes fall through `_ => {}`, malformed extended (`38` with no params) yields `None` and is skipped. Current tests (entire file `ansi_text/tests.rs`): `strips_ansi_sequences_from_bytes` (one red sequence) and `converts_sgr_to_styled_spans` (one 31m span).
- `focus_owner.rs` ring (verbatim):

```rust
    fn next(self) -> Self {
        let ring = Self::RING;
        if ring.is_empty() { return self; }
        let idx = self.index();
        ring[(idx + 1) % ring.len()]
    }
```

  plus `prev` (`(idx + ring.len() - 1) % ring.len()`), `panel_emphasis_for`, `show_cursor_for`. All `pub` items in a 95-line module; zero tests.
- `widgets/diff.rs` (50 lines, zero tests) — render loop (verbatim):

```rust
        for (visible, line) in self.lines.iter().skip(state.offset).take(area.height as usize).enumerate() {
            ...
            buffer.set_stringn(area.x, area.y.saturating_add(visible as u16), line.text, area.width as usize, style);
```

  `state.offset` is `pub usize`, never clamped (offset past end renders nothing — silent); `line.text` goes to `set_stringn` raw. NOTE on sanitization: ratatui buffers treat cell content as text (control chars don't execute as escapes through the cell path) — so the unsanitized text is a rendering-correctness issue (zero-width/control chars corrupt column math), not an injection sink; test width behavior, don't add sanitization.
- `layout/dialog.rs:99-120` — the truncating casts (verbatim, two of four sites):

```rust
            KeyCode::Down | KeyCode::Char('j' | 'J') if axes.vertical => {
                let max = content_height.saturating_sub(viewport_height) as u16;
                self.scroll_y = self.scroll_y.saturating_add(1).min(max);
```

  Contrast the crate convention: `scroll/mod.rs` (~448) uses `max_offset_u16`, which saturates to `u16::MAX` above range. With >65,535 content rows the `as u16` wraps and caps scrolling far short of the bottom.
- Test-style exemplars: `widgets/tests.rs` (buffer cell assertions), `scroll/tests.rs` (table-driven pure-fn tests). No proptest/quickcheck in the workspace (deliberate — TODO.md defers fuzz; use table-driven cases, not property testing deps).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Module tests | `cargo test -p termrock geometry` (or `text` post-012) | new tests pass |
| ANSI tests | `cargo test -p termrock ansi` | all pass |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope**:
- New test modules: `geometry` (or its post-012 homes), `ansi_text/tests.rs` (extend), `interaction/focus_owner.rs` (add `mod tests`), `widgets/diff.rs` (add `mod tests`)
- Fix 1: offset clamp in `widgets/diff.rs`
- Fix 2: saturating conversions in `layout/dialog.rs` `handle_key_for_axes`

**Out of scope**:
- Behavior changes to geometry/ANSI functions — if a test exposes a wrong result, pin it with `// CHARACTERIZATION: BUG` and report (fixing width math ripples into every widget; that's its own decision).
- Property-testing dependencies (deferred by TODO.md).

## Git workflow

- Directly on `main`; suggested: `test(core): cover geometry, ansi, focus ring` then `fix(scroll): saturate dialog-body clamps; clamp diff offset`.

## Steps

### Step 1: Geometry test module

Add `#[cfg(test)] mod tests` (in `geometry.rs`, or per-home post-012) with table-driven cases:

- `display_cols`/`take_display_cols`/`display_cols_slice`: ASCII, CJK (`"日本語"` = 6 cols), emoji (`"🧪"` = 2), combining char (`"e\u{301}"` = 1), control byte input, empty; slice windows: start mid-wide-char (pin behavior: does the wide char get dropped or replaced?), width 0, width beyond content. Invariant asserted per case: result's display width ≤ requested width.
- `fixed_prefix_scroll_segments`: no-scroll fit, scroll past end, combining-char coalescing at the boundary (the ~234-240 logic), zero-width prefix.
- `sanitize_terminal_title`: embedded ESC/BEL/newline/tab collapse to single spaces, C1 range (`\u{9b}`), leading/trailing trim, all-control input → empty.
- `is_terminal_control_char`: boundary table — 0x1F yes, 0x20 no, 0x7E no, 0x7F yes, 0x80 yes, 0x9F yes, 0xA0 no.
- `centered_rect`: width 0, 1, 2, huge; result always ⊆ input rect (assert containment, not exact values, for the tiny cases; exact for a nominal case).
- `lay_out_tabs`/`tab_at_column`: two tabs with a wide-char label; click at first cell, last cell, the gap, past end — returned index/None per position. This directly tests the documented wide-char drift risk.
- `hint_row_cols`: nominal + empty.

**Verify**: `cargo test -p termrock geometry` (or new homes) → ~20 cases pass. Any `CHARACTERIZATION: BUG` pins reported.

### Step 2: ANSI parser cases

Extend `ansi_text/tests.rs`, table-driven over `styled_spans` + `strip_bytes`:

- indexed color `\x1b[38;5;196m`, truecolor `\x1b[38;2;1;2;3m`, out-of-range clamp `38;5;300` → Indexed(255), background variants `48;5;…`/`41`/`104`, bright fg `92`, modifiers `1`/`2`/`22` (bold+dim then clear), defaults `39`/`49` restore the passed default style, multi-code sequence `\x1b[1;31;44m`, malformed `\x1b[38m` (skipped, no panic), truncated escape at end-of-input `"text\x1b["` (no panic; pin output), empty param `\x1b[m` (= reset).
- `strip_bytes` invariant across all the above inputs: output contains no `\x1b`.

**Verify**: `cargo test -p termrock ansi` → ~12 new cases pass.

### Step 3: Focus ring + diff tests

- `focus_owner.rs` `mod tests`: `next`/`prev` full cycle returns to start; `prev` from first wraps to last; `panel_emphasis_for` returns `Focused` only for the owned tab; `show_cursor_for` mirror. (The `RING is_empty` branch is dead if `RING` is a non-empty const — check; if dead, note it rather than contriving a test.)
- `diff.rs` `mod tests` (buffer-assertion style from `widgets/tests.rs`): renders kind-styles at correct cells; `offset` beyond `lines.len()` renders empty without panic; width-1/height-0 areas no panic; a line containing a control char — pin the cell result.

**Verify**: `cargo test -p termrock --lib` → new tests pass.

### Step 4: The two fixes

- `diff.rs`: clamp at render top: `let offset = state.offset.min(self.lines.len().saturating_sub(1));` — decide: clamp to `len-1` (keep last line visible) or `len.saturating_sub(area.height)` (classic max-offset)? Match the crate convention: `scroll::max_offset` exists — use `state.offset.min(max_offset(self.lines.len(), area.height as usize))` (grep its exact signature in `scroll/mod.rs` first). Write the test BEFORE the fix (red→green).
- `layout/dialog.rs` `handle_key_for_axes`: replace all four `... as u16` max/step computations with the saturating path used by `DialogScroll` (`scroll::max_offset_u16` — check exact name/signature at `scroll/mod.rs` ~448) and saturating `u16::try_from(...).unwrap_or(u16::MAX)` for the page steps. Add one test with `content_height = 70_000`, viewport 10: Down/PageDown reach the true bottom (`scroll_y == max_offset_u16(70_000, 10)`), not a wrapped small cap.

**Verify**: both new tests red before fix, green after; `cargo test --workspace --all-features --locked` → all pass.

## Test plan

Steps 1–4 are the test plan (~35 new tests). Patterns: `scroll/tests.rs` (tables), `widgets/tests.rs` (buffers). The two fixes each land with their red→green test.

## Done criteria

- [x] Geometry/text, ANSI, focus, and diff coverage landed in `4b79273`; the later shared `FocusRing` supersedes `focus_owner` with a richer suite
- [x] The four dialog clamp casts were replaced with saturating forms; Plan 024 later deleted that duplicate dialog-scroll implementation entirely
- [x] Diff over-scroll clamped, tested
- [x] `cargo test --workspace --all-features --locked` → all pass; clippy clean
- [x] No `CHARACTERIZATION: BUG` findings were pinned; the README finding slot remains empty
- [x] `plans/README.md` status row updated

## STOP conditions

- A geometry test exposes an actual wrong width/hit-test result (not just an unpinned edge) that existing widgets visibly depend on — pin + report; do NOT fix the math.
- `scroll::max_offset`/`max_offset_u16` signatures don't match what diff/dialog need without conversion gymnastics — report the exact mismatch instead of inventing a third clamp helper.

## Maintenance notes

- These tables are the place future Unicode edge reports get their regression case — keep them table-driven so a new case is one line.
- If fuzz targets land later (TODO.md defers them), `styled_spans`, `strip_bytes`, `display_cols_slice`, and `raw_bytes_to_chord` are the four candidate entry points — the tables here seed the corpora.
