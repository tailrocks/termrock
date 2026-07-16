# Plan 009: Purge donor-product color constants from the public API; the palette lives inside the theme

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/lib.rs crates/termrock/src/style/`
> Plan 008's Theme/Role changes to `style/mod.rs` are expected prerequisites,
> not drift. On other mismatches with "Current state", STOP.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (deletes public constants; forward-only policy sanctions it, migration file required)
- **Depends on**: plans/008-theme-constructor-and-role-threading.md
- **Category**: tech-debt
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The crate root exports ~38 `pub const` RGB tokens, many named for one donor product's features: `RAIN_HEAD/FRESH/BODY/MID/DIM/DARK` ("launch digital-rain animation"), `MENU_IDLE_BG`/`MENU_AWAITING_*` ("Menu button background"), `DEBUG_AMBER` ("inside a `--debug` run"), `STATUS_BLOCKED_RED` ("Stuck tab glyph"), `BRAND_BLOCK` ("brand pill"), with doc comments naming "`--jk-brand`", "the console", "launch cockpit", and "the editor's running-instance status badge". Grep confirms the product-specific ones have **zero consumers** in this workspace — they are pure donor leakage carried as public API, in direct violation of AGENTS.md ("Do not add product-branded widgets… product-neutral"). After Plan 008, widgets read the `Theme`, so the raw palette no longer needs to be public at all. The phosphor design remains the beloved default (per the "Modern-first, pre-stable API" section) — it just lives inside `Theme::tailrocks_phosphor()` instead of polluting the neutral surface.

## Current state

- `crates/termrock/src/lib.rs:48-206` — `Rgb` struct + ~38 `pub const` tokens. Categories:
  - **Product-specific, zero in-repo consumers** (delete): `RAIN_HEAD`, `RAIN_FRESH`, `RAIN_BODY`, `RAIN_MID`, `RAIN_DIM`, `RAIN_DARK`, `MENU_IDLE_BG`, `MENU_IDLE_HOVER_BG`, `MENU_AWAITING_BG`, `MENU_AWAITING_HOVER_BG`, `DEBUG_AMBER`, `STATUS_BLOCKED_RED`, `BRAND_BLOCK`, `LINK_BLUE`.
  - **Palette feeding the phosphor theme** (move into `style/`, demote visibility): `PHOSPHOR_GREEN`, `PHOSPHOR_DIM`, `PHOSPHOR_DARK`, `BLACK`, `WHITE`, `INPUT_BG_DIM`, `TAB_BG_*` (4), `LINK_FG`, `LINK_FG_HOVER`, `AMBER`, `BORDER_GRAY`, `BORDER_GRAY_LIGHT`, `DANGER_RED`, `CYAN`, `CYAN_DIM`, `ACTION_ACCENT`, `DISCLOSURE_ACCENT`, `WARNING_YELLOW`, `DIALOG_SCROLL_THUMB`, `DIALOG_SCROLL_TRACK`, `PREVIEW_CARD`.
- `crates/termrock/src/style/mod.rs:12-27` — imports ~30 of those RGB constants from the crate root (`use crate::{ACTION_ACCENT as ACTION_ACCENT_RGB, …}`) and re-derives `Color` constants from them — the semantic layer depending "up" on raw data in `lib.rs`.
- Product-flavored doc comments to strip regardless of what survives: `lib.rs:63` ("`--jk-brand`"), `:91` ("launch digital-rain animation"), `:152` ("--debug run"), `:171` ("Stuck tab glyph"), `:185-186` ("editor's running-instance status badge"); `style/mod.rs:35-36` ("the rest of the CLI and the digital rain", "brand pill").
- In-repo consumers of the palette constants (verified by grep): `style/mod.rs` (the import block), lookbook uses `PHOSPHOR_GREEN`/`PHOSPHOR_DARK`/`PREVIEW_CARD` (via `crate::style::` or root — grep `PREVIEW_CARD` in `crates/termrock-lookbook/` to locate), and `widgets/tests.rs:162` asserts `crate::style::PHOSPHOR_GREEN`.
- `Rgb` type itself is used by `style::color(rgb)` and the OSC/ANSI side — keep the type, relocate it.
- Repo conventions: forward-only breaking change + migration file same commit; `docs/api/public-api.txt` regeneration if Plan 003's gate is live.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Find every consumer | `grep -rn "RAIN_\|MENU_\|DEBUG_AMBER\|STATUS_BLOCKED\|BRAND_BLOCK\|LINK_BLUE" crates/ --include="*.rs"` | before: hits only in lib.rs + style/mod.rs; after: none |
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Preview check | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/lib.rs` (constant/type removal + doc rewrite)
- `crates/termrock/src/style/mod.rs` (palette becomes module-private data here)
- Lookbook files the compiler flags
- `migrations/000N-*.md` + `MIGRATING.md`
- `docs/api/public-api.txt` regeneration

**Out of scope**:
- `Role`/`Theme` shape (Plan 008 owns it; assume landed).
- Widget bodies (after Plan 008 they reference roles, not constants; if any still references a purged constant, that is a Plan-008 gap — STOP and report rather than patching here).
- `ModalOutcome`, `PointerShape` and other crate-root non-color items — Plan 012.

## Git workflow

- Directly on `main`; `git commit -s -m "refactor(style)!: remove donor-branded palette from the public API"`; migration file in the same commit.

## Steps

### Step 1: Delete the product-specific constants

Remove the 14 zero-consumer constants listed above from `lib.rs`, and their `Color` re-derivations in `style/mod.rs` (`BRAND_BLOCK`, `DEBUG_AMBER`, `STATUS_BLOCKED_RED`, `LINK_BLUE`, `MENU_*`). Delete the corresponding import lines from the `use crate::{...}` block.

**Verify**: the grep in "Commands" returns no matches; `cargo check --workspace --all-features --locked` → exit 0 (zero consumers means zero breakage in-repo).

### Step 2: Move the surviving palette into `style/`

Relocate `Rgb` and the surviving RGB constants from `lib.rs` into `style/mod.rs` (or a `style/palette.rs` submodule), demoting the RGB constants to `pub(crate)`. Keep `pub` on: `Rgb` (used in public signatures — check with `grep -rn "Rgb" crates/termrock/src/ --include="*.rs" | grep "pub fn\|pub struct\|pub const fn"` — `style::color(rgb: Rgb)` is public), and any `Color` constant still consumed by the lookbook or tests via a public path (`PHOSPHOR_GREEN`, `PHOSPHOR_DARK`, `PREVIEW_CARD` — these three may stay `pub` inside `style::` as the documented phosphor palette; everything else goes `pub(crate)`).

Add `pub use style::Rgb;`? No — keep one canonical path (`termrock::style::Rgb`); update in-repo callers.

**Verify**: `cargo test --workspace --all-features --locked` → all pass; `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` → exit 0, zero SVG diffs.

### Step 3: Strip product-flavored language

Rewrite the surviving doc comments in neutral, semantic terms. Every occurrence of: "jk-brand", "digital rain", "rain", "launch cockpit", "the console", "brand pill", "--debug run", "Stuck tab", "editor's running-instance", "Dyn footer" → describe the semantic function instead ("high-emphasis accent", "focus highlight", …). Sweep: `grep -rni "jk-brand\|digital.rain\|cockpit\|brand pill\|stuck tab\|running-instance" crates/termrock/src/` must end empty.

**Verify**: the grep sweep returns nothing; `cargo doc --workspace --all-features --no-deps --locked` with `RUSTDOCFLAGS='-D warnings'` → exit 0.

### Step 4: Migration file + API report

Next-numbered migration file. Old-to-new table: each deleted constant → "consumer-owned: define the value in your application if you used it" (they were product tokens; the donor app owns them now), each moved constant → `termrock::style::` path or "use `Theme`/`Role` instead". Include the before/after:

```rust
// Before
use termrock::{PHOSPHOR_GREEN, RAIN_BODY};

// After
use termrock::style::PHOSPHOR_GREEN;   // palette preset
// RAIN_* were donor-product tokens: define them in the consuming app.
```

Regenerate `docs/api/public-api.txt` per Plan 003 Step 1's command.

**Verify**: migration indexed; `cd docs && bun run build` → exit 0.

## Test plan

- No new behavior; net = existing suite + preview determinism. One new test worth adding: `style` unit test asserting `Theme::default()` equals `Theme::tailrocks_phosphor()` (pins the default-design promise from AGENTS.md).

## Done criteria

- [ ] `grep -rn "RAIN_\|MENU_IDLE\|MENU_AWAITING\|DEBUG_AMBER\|STATUS_BLOCKED_RED\|BRAND_BLOCK\|LINK_BLUE" crates/ --include="*.rs"` → no matches
- [ ] `grep -rni "jk-brand\|digital.rain\|cockpit\|brand pill\|stuck tab" crates/termrock/src/` → no matches
- [ ] `lib.rs` no longer defines `Rgb` or color constants (moved to `style/`)
- [ ] `cargo test --workspace --all-features --locked` → all pass; preview check → zero diffs
- [ ] Migration file exists and is indexed; `public-api.txt` regenerated
- [ ] `plans/README.md` status row updated

## STOP conditions

- A widget body still references a purged/moved constant → Plan 008 gap; report, don't patch around it.
- The lookbook uses a product constant for its own chrome (e.g. `PREVIEW_CARD`) in a way that suggests it should move to the lookbook crate instead of surviving in `style::` — acceptable either way, but if moving it requires touching lookbook rendering logic beyond an import path, report first.
- `Rgb` appears in a public signature outside `style`/`osc` that would force keeping a root re-export — report the signature.

## Maintenance notes

- Future rule for reviewers (matches AGENTS.md): no product-named identifiers in `crates/termrock` — semantic names only. Product palettes live in consumer apps as `Theme` overrides.
- Plan 010 adds a second preset to *prove* the neutrality this plan creates.
- Plan 012 finishes the crate-root cleanup (non-color items).
