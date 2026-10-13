# Plan 001: Build the termrock-raster crate — deterministic ratatui Buffer → PNG rendering

> **Executor instructions**: Follow this plan step by step. Run the
> preconditions first. Run every verification command and confirm the
> expected result before moving on. If anything in "STOP conditions"
> occurs, stop and report — do not improvise. When done, update this
> plan's status row in `plans/jackin-termrock-parity/README.md`.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Covers**: spec/render-pipeline.md (all requirements) · F5(engine), B2, D9, N3
- **Guardrails**: N3
- **Research basis**: research/tui-png-baselines/06-rasterizer-facts-and-archtest.md,
  research/tui-png-baselines/04-determinism-ci-storage.md (§1, §2),
  research/tui-png-baselines/05-ci-placement-and-commands.md (Q3)
- **Planned at**: commit `41cf3d0b`, 2026-08-16

## Why this matters

Every PNG baseline, comparison pair, and CI design gate in the
jackin-termrock-parity item consumes one engine: a pure-Rust rasterizer that
turns a painted ratatui `Buffer` into a PNG. This plan builds that engine as a
new workspace crate, `termrock-raster`, with a vendored, hash-pinned
JetBrains Mono 2.304 font set, full modifier fidelity (including ITALIC and
CROSSED_OUT, which the existing lossy `FrameCell` JSON seam drops), and a
measured-deterministic swash + tiny-skia stack. Because the pipeline has zero
OS-text-stack inputs (no fontconfig, no FreeType, no CoreText), its output is
identical across machines by construction — that is what makes macOS-blessed
baselines verifiable on Linux CI in later plans. After this lands, plan 002 can
generate the committed baseline set and plan 003 can wire the bless-required
gate; neither is touched here.

## Preconditions — run before anything else

This plan depends on no other plan. Verify the environment and the starting
state:

- Toolchain present: `cargo --version` → prints a version (workspace pins
  `rust-version = "1.97.1"`; any >= 1.97.1 toolchain is fine).
- Task runner present: `mise --version` → prints a version.
- Workspace green at start: `mise run test` → all tests pass, exit 0.
- cargo-deny present and green at start:
  `cargo deny check advisories bans licenses sources` → exit 0.
- Font source reachable:
  `curl -sL -o /dev/null -w '%{http_code}' https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip`
  → `200` (final status after redirects).
- Drift check (this plan edits pre-existing files):
  `git diff --stat 41cf3d0b..HEAD -- Cargo.toml deny.toml REUSE.toml LICENSES/ crates/termrock-lookbook/src/frame.rs crates/termrock-lookbook/src/palette256.rs`
  → empty output. On any change, compare the "Starting state" excerpts below
  against the live files; a mismatch is a STOP.

Any failed precondition is a STOP.

## Spec contract

Inlined **verbatim** from `plans/jackin-termrock-parity/spec/render-pipeline.md`
(do not re-read the spec; this is the contract):

> ### Requirement: Buffer-to-PNG rasterization
> A new `termrock-raster` library crate (workspace member) SHALL render a
> ratatui `Buffer` plus a `RolePalette` to an RGBA pixmap and encode it as PNG,
> using `swash` (glyph scaling/hinting/rasterization via `charmap.map` per
> glyph — no shaper) and `tiny-skia` (compositing + `Pixmap::encode_png`),
> with cell geometry 9×18 px, font size 14 px, baseline 14 px from cell top —
> matching the existing seam constants (`frame.rs:345-348`,
> `TerminalPreview.tsx:67-68`, ch. 06 §3). Color resolution SHALL reuse the
> same named-color→RGB and xterm-256 mapping as `frame.rs:201-228`/
> `palette256.rs`.
> Covers: F5, B2, D9 · Evidence: ch. 06 §1, §3, §4
>
> #### Scenario: Panel story renders
> - **GIVEN** the `panel/focused` lookbook story painted into a Buffer at its story size with the phosphor `RolePalette`
> - **WHEN** `termrock-raster` renders it
> - **THEN** a PNG of exactly (cols×9) × (rows×18) px is produced
> - **AND** the phosphor border color appears in the border cell pixels
>
> #### Scenario: Wide grapheme spans two cells
> - **GIVEN** a Buffer containing a double-width grapheme
> - **WHEN** rendered
> - **THEN** the glyph is drawn across two cell widths and the following cell paints no duplicate glyph
>
> ### Requirement: Full modifier fidelity
> The rasterizer SHALL consume the unresolved `Buffer` (not the lossy
> `FrameCell` JSON) and render every modifier the paint system uses: BOLD via
> the vendored Bold face, ITALIC via the vendored Italic face, UNDERLINED as a
> row span (thickness `max(1, round(18*0.1))` at offset consistent with the
> web path), CROSSED_OUT as a mid-cell span, REVERSED by fg/bg swap, and DIM
> by a single 0.6 fg darken — the Rust-side resolution from
> `frame.rs:180-198`, applied exactly once.
> Covers: F5, B2 · Evidence: ch. 03 Q1/Q2 (frame JSON drops ITALIC/CROSSED_OUT; Buffer is the full-fidelity seam), ch. 06 §3
>
> #### Scenario: Italic survives
> - **GIVEN** a Buffer cell styled ITALIC (in real use, e.g. `stories.rs:22190`)
> - **WHEN** rendered
> - **THEN** the glyph comes from the Italic face, not a slanted Regular
>
> #### Scenario: Dim darkens once
> - **GIVEN** a cell with DIM and fg (0,255,65)
> - **WHEN** rendered
> - **THEN** the drawn fg is (0,153,39) — 0.6×, single application
>
> ### Requirement: Vendored pinned fonts
> The crate SHALL embed JetBrains Mono v2.304 Regular, Bold, and Italic TTFs
> (official release artifact; Regular sha256
> `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`) via
> `include_bytes!`, never system font discovery — no fontdb, no fontconfig, no
> CoreText. A unit test SHALL assert each embedded font's sha256.
> Covers: F5, B2, D9 · Evidence: ch. 06 §2 (license + sha), ch. 04 §1 (OS-stack variance)
>
> #### Scenario: Font tamper detected
> - **GIVEN** the crate's font-hash test
> - **WHEN** an embedded TTF byte differs from the pinned sha256
> - **THEN** the test fails naming the font file
>
> ### Requirement: License compliance for the raster stack
> The change SHALL extend `deny.toml:6`'s allow list with `BSD-3-Clause` and
> `BSD-2-Clause` (tiny-skia, arrayref — ch. 06 §2), add `LICENSES/OFL-1.1.txt`,
> and add a `REUSE.toml` annotation scoping the vendored font files to
> `OFL-1.1` so the `**` = Apache-2.0 annotation no longer misdeclares them.
> `cargo deny check` MUST pass afterwards.
> Covers: F5 · Evidence: ch. 06 §2
>
> #### Scenario: cargo deny passes with the raster stack
> - **GIVEN** termrock-raster with swash + tiny-skia in the workspace graph
> - **WHEN** `cargo deny check advisories bans licenses sources` runs
> - **THEN** it exits 0
>
> ### Requirement: Determinism self-test
> The crate SHALL ship a test that renders a fixture Buffer twice and asserts
> byte-identical raw RGBA and byte-identical PNG output, and a pixel-compare
> helper that decodes two PNGs and asserts pixel equality at zero tolerance —
> the only comparison predicate (N3: never gate on undecoded PNG bytes).
> Covers: F5, B2, N3 · Evidence: ch. 06 §4 (measured identity), ch. 04 §2 (pixel-compare norm)
>
> #### Scenario: Double render identical
> - **WHEN** the same Buffer renders twice in one process
> - **THEN** raw RGBA sha256 and PNG sha256 are equal
>
> #### Scenario: One-pixel difference caught
> - **GIVEN** two PNGs differing in one pixel's fg by one bit
> - **WHEN** the pixel-compare helper runs
> - **THEN** it reports inequality naming the first differing coordinate

Done means these scenarios hold; the test plan below exercises them.

## Must NOT

Guardrail inlined verbatim from the must-not registry
(`plans/jackin-termrock-parity/spec/README.md`). It overrides anything a step
seems to imply:

- **N3**: "CI MUST NOT gate on PNG byte equality; the predicate is
  decoded-pixel equality at zero tolerance" — reason: "encoder-version churn
  rewrites bytes without pixel change (research ch. 04 §2)". For this plan
  that means: the **exported pixel-compare helper decodes both PNGs and
  compares pixels**; it never compares encoded bytes. (The crate's *internal*
  determinism self-test additionally hashes raw RGBA and PNG bytes — that is a
  self-check of the fixed in-process stack, explicitly required by the spec,
  not a CI comparison predicate. Nothing exported for comparison may be
  byte-based.)

Additional hard boundaries for this plan:

- Do NOT modify `crates/termrock-lookbook/**` (its color mapping is
  duplicated, not moved — see "Starting state / Chosen shape").
- Do NOT add `fontdb`, `cosmic-text`, any shaper, any system font discovery,
  or the `image` crate. The dependency set for rendering is exactly
  `swash` + `tiny-skia` (+ already-present workspace deps).
- Do NOT commit baseline PNG files or a CI gate test — those are plans 002
  and 003.

## Inputs to provide

- `FONT_ZIP` — the official JetBrains Mono v2.304 release artifact,
  `https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip`.
  Needed by step 1.
  - If absent (URL unreachable): there is NO substitute — the Regular TTF must
    hash to the pinned sha256, so no placeholder can stand in. This is a STOP
    condition, not a proceed-with-placeholder input.
- `OFL_TEXT` — the canonical SPDX text of OFL-1.1. Needed by step 2.
  - Preferred source: `reuse download OFL-1.1` (the `reuse` tool 6.2.0 is
    pinned in `mise.toml:20`; run via `mise install` first if missing).
  - Fallback: `curl -fsSL -o LICENSES/OFL-1.1.txt https://raw.githubusercontent.com/spdx/license-list-data/main/text/OFL-1.1.txt`.
  - If both fail: STOP (license file is a spec requirement).

No secrets are involved anywhere in this plan.

## Starting state

Verified at commit `41cf3d0b` (2026-08-16). All excerpts re-read from the live
files at planning time.

### Workspace layout

`/Users/donbeave/Projects/tailrocks/termrock/Cargo.toml:1-9` — members list:

```toml
[workspace]
members = [
    "crates/termrock",
    "crates/termrock-lookbook",
    "crates/termrock-lookbook-web",
    "crates/termrock-cli",
    "crates/termrock-showcase",
]
resolver = "3"
```

`Cargo.toml:19-36` — `[workspace.dependencies]` is the convention: every crate
inherits with `foo.workspace = true`. Already present and relevant here:
`unicode-width = "0.2.2"` (line 33), `sha2 = "0.11.0"` (line 35),
`hex = "0.4.3"` (line 36), `ratatui = { version = "0.30.2", default-features = false }`
(line 23). `swash` and `tiny-skia` are NOT present yet.

`Cargo.toml:38-58` — workspace lints apply to every crate:
`missing_docs = "deny"`, `unsafe_code = "forbid"`. Every public item in the
new crate needs a doc comment; no `unsafe` anywhere.

Small-crate manifest exemplar — `crates/termrock-lookbook/Cargo.toml:1-12`
opens with an SPDX header comment and inherits workspace package fields:

```toml
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0

[package]
name = "termrock-lookbook"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "Interactive and generated component catalog for TermRock"
publish = false
```

and ends with:

```toml
[lints]
workspace = true
```

Every `.rs` file in the repo opens with the two-line SPDX comment header shown
above. Match both conventions in every new file.

### The seam constants and paint resolution to replicate

`crates/termrock-lookbook/src/frame.rs:345-348`:

```rust
/// Ghostty-class default cell width in CSS pixels (matches SVG export).
pub const CELL_WIDTH_PX: u16 = 9;
/// Ghostty-class default cell height in CSS pixels (matches SVG export).
pub const CELL_HEIGHT_PX: u16 = 18;
```

Derived text metrics (research ch. 06 §3, from `docs/src/components/preview-metrics.ts:7-14`
and `TerminalPreview.tsx:141-142`): font size `max(11, floor(18*0.78)) = 14 px`,
baseline `floor(18*0.78) = 14 px` from cell top. Web underline: thickness
`max(1, round(h*0.1))` (= 2 px at h=18) at offset `h - thickness - 1` (= 15)
from cell top (`preview-metrics.ts:63-71`). "Offset consistent with the web
path" in the spec therefore means **top offset 15, thickness 2** at the fixed
18 px cell height.

`crates/termrock-lookbook/src/frame.rs:173-199` — the Rust-side modifier
resolution the spec mandates ("applied exactly once"):

```rust
fn resolve_cell_paint(
    fg: Color,
    bg: Color,
    modifier: Modifier,
) -> ([u8; 3], [u8; 3], bool, bool, bool, bool) {
    let mut fg_rgb = color_to_rgb(fg, true);
    let mut bg_rgb = color_to_rgb(bg, false);
    let reversed = modifier.contains(Modifier::REVERSED);
    if reversed {
        std::mem::swap(&mut fg_rgb, &mut bg_rgb);
    }
    if modifier.contains(Modifier::DIM) {
        fg_rgb = [
            (u16::from(fg_rgb[0]) * 6 / 10) as u8,
            (u16::from(fg_rgb[1]) * 6 / 10) as u8,
            (u16::from(fg_rgb[2]) * 6 / 10) as u8,
        ];
    }
    (
        fg_rgb,
        bg_rgb,
        modifier.contains(Modifier::BOLD),
        modifier.contains(Modifier::DIM),
        modifier.contains(Modifier::UNDERLINED),
        reversed,
    )
}
```

`crates/termrock-lookbook/src/frame.rs:201-228` — the named-color→RGB mapping
the spec says to reuse:

```rust
fn color_to_rgb(color: Color, is_fg: bool) -> [u8; 3] {
    match color {
        Color::Reset => {
            if is_fg {
                [0xff, 0xff, 0xff]
            } else {
                [0x00, 0x00, 0x00]
            }
        }
        Color::Black => [0x00, 0x00, 0x00],
        Color::Red => [0xff, 0x00, 0x00],
        Color::Green => [0x00, 0xff, 0x41],
        Color::Yellow => [0xff, 0xd8, 0x5e],
        Color::Blue => [0x00, 0x50, 0xb4],
        Color::Magenta => [0xff, 0x00, 0xff],
        Color::Cyan => [0x00, 0xff, 0xff],
        Color::Gray | Color::DarkGray => [0x80, 0x80, 0x80],
        Color::LightRed => [0xff, 0x5e, 0x7a],
        Color::LightGreen => [0x00, 0xff, 0x41],
        Color::LightYellow => [0xff, 0xd8, 0x5e],
        Color::LightBlue => [0x7a, 0xa2, 0xff],
        Color::LightMagenta => [0xff, 0x7a, 0xff],
        Color::LightCyan => [0x7a, 0xff, 0xff],
        Color::White => [0xff, 0xff, 0xff],
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(index) => crate::palette256::xterm256_to_rgb(index),
    }
}
```

`crates/termrock-lookbook/src/palette256.rs:12-47` — the xterm-256 table
(public function `pub fn xterm256_to_rgb(index: u8) -> [u8; 3]`): indices
0–15 are the phosphor-slot system colors
(`0 → [0,0,0]`, `1 → [0xff,0,0]`, `2 → [0,0xff,0x41]`, `3 → [0xff,0xd8,0x5e]`,
`4 → [0,0x50,0xb4]`, `5 → [0xff,0,0xff]`, `6 → [0,0xff,0xff]`,
`7 → [0xc0,0xc0,0xc0]`, `8 → [0x80,0x80,0x80]`, `9 → [0xff,0x5e,0x7a]`,
`10 → [0,0xff,0x41]`, `11 → [0xff,0xd8,0x5e]`, `12 → [0x7a,0xa2,0xff]`,
`13 → [0xff,0x7a,0xff]`, `14 → [0x7a,0xff,0xff]`, `15 → [0xff,0xff,0xff]`);
16–231 the 6×6×6 cube with channel steps `[0, 95, 135, 175, 215, 255]`
(`i = index - 16`; r step `i / 36`, g step `(i % 36) / 6`, b step `i % 6`);
232–255 the grey ramp `level = 8 + (index - 232) * 10`.

### Chosen shape for the color mapping (importability checked)

`termrock-lookbook` IS a lib+bin crate (`src/lib.rs` exports `pub mod frame`
and `pub mod palette256`; `termrock-lookbook-web/Cargo.toml:18` already
imports it with `default-features = false`). However:

- `color_to_rgb` and `resolve_cell_paint` are **private** (`fn`, not
  `pub fn`) in `frame.rs` — not importable without modifying lookbook, which
  is out of scope for this plan.
- A normal dependency from `termrock-raster` on `termrock-lookbook` would
  also invert the dependency direction plans 002/003 need (lookbook-side code
  rendering through the rasterizer).

**Chosen shape: duplicate the mapping into `termrock-raster`** (module
`src/color.rs`), byte-for-byte the same values, with a doc comment naming
`crates/termrock-lookbook/src/frame.rs:201-228` and
`crates/termrock-lookbook/src/palette256.rs` as the peer copies. Unit tests
pin the table values against the constants inlined in this plan (independent
source), and plan 003's pixel gate catches any future divergence in practice.
Deferred root cause (named per repo consistency law, scheduled — not silently
dropped): two copies of one mapping exist after this plan; the unification
(moving lookbook's `frame.rs`/`svg.rs` onto `termrock-raster::color`) is
follow-up work recorded in "Maintenance notes", because touching lookbook is
out of this plan's scope.

`termrock-lookbook` is used by this plan **only as a dev-dependency** (for the
"Panel story renders" test). Cargo supports that shape even after later plans
make lookbook depend on termrock-raster (dev-dependency cycles through one dev
edge are allowed).

### Public API facts needed by the tests

- `termrock::style::RolePalette` (`crates/termrock/src/style/mod.rs:355`),
  with `RolePalette::default()` = `tailrocks_phosphor()`
  (`style/mod.rs:839-843`) and public `style(Role) -> Style`.
  `Role::Canvas` and `Role::BorderFocused` are public roles; phosphor
  `Role::BorderFocused` fg is PHOSPHOR_GREEN = RGB (0, 255, 65)
  (`crates/termrock/src/style/palette.rs:68`).
- `termrock_lookbook::frame::story_by_id(id) -> Option<Story>` is public;
  `Story` exposes `pub id`, `pub width`, `pub height`, and
  `pub fn make_interactor(&self) -> Box<dyn StoryInteraction>`
  (`stories.rs:168-220`). `StoryInteraction` (public trait,
  `interactors.rs:102`) has `fn render(&mut self, frame: &mut Frame<'_>, area: Rect)`
  and `fn set_theme(&mut self, theme: RolePalette)`. The story id
  `"panel/focused"` exists (`stories.rs:1053`).
- `ratatui::backend::TestBackend` + `ratatui::Terminal` are how lookbook
  paints stories into a `Buffer` off-screen (`frame.rs:248-274`).

### Design and vocabulary constraints from research (quoted)

- Minimal crate set (ch. 06 §1): "Empirical minimal set for grid → PNG:
  `swash` + `tiny-skia` (its default `png-format` feature bundles `png` for
  `Pixmap::encode_png`) — 38 crates total in the resolved graph, no fontdb,
  no shaper, no image crate." Current stable pins: swash 0.2.10,
  tiny-skia 0.12.0.
- Per-glyph rendering without shaping is proven (ch. 06 §1): render with only
  "`FontRef::from_index(bytes, 0)` → `font.charmap().map(ch)` →
  `ScaleContext`/`Scaler` →
  `Render::new(&[Source::Outline]).format(Format::Alpha).render(&mut scaler, glyph_id)`".
- Determinism measured (ch. 06 §4): the archtest with this exact stack
  (JBM Regular v2.304 at 14 px, hinted and unhinted, box/block glyphs
  U+250C…U+2593 rendered **from the font**, tiny-skia AA fills, integer-only
  source-over compositing) produced bit-identical raw RGBA and PNG output
  across aarch64-native and x86_64-under-Rosetta, double-run each.
- Box-drawing note: unlike the web canvas path (which strokes box glyphs as
  vectors), the rasterizer draws box/block glyphs **from the JBM font** — the
  archtest confirmed JBM covers U+250C…U+2593 and renders them correctly.
- Fonts (ch. 06 §2): official artifact
  `https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip`;
  license "SIL Open Font License, Version 1.1"; copyright line
  "Copyright 2020 The JetBrains Mono Project Authors";
  `JetBrainsMono-Regular.ttf` sha256
  `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`.
- Recorded cross-surface defect, binding note from the spec README (in scope
  only as a note): "dim is darkened 0.6× in `frame.rs:184-189` and again 0.7×
  in `preview-metrics.ts:167-172` (ch. 06 §3). The rasterizer follows the
  Rust-side single 0.6 resolution; reconciling the web path is separate
  cleanup, not this item."

### License machinery to edit

`deny.toml` (entire current file):

```toml
[advisories]
yanked = "deny"

[licenses]
confidence-threshold = 0.8
allow = ["Apache-2.0", "MIT", "Unicode-3.0", "Zlib"]

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

cargo-deny covers dev-dependency graphs too (no `[graph] exclude-dev` is set),
so tiny-skia (BSD-3-Clause) and arrayref (BSD-2-Clause, via swash) fail the
licenses check until the allow list is extended (ch. 06 §2).

`REUSE.toml` (entire current file):

```toml
version = 1
SPDX-PackageName = "termrock"
SPDX-PackageSupplier = "Alexey Zhokhov <alexey@zhokhov.com>"
SPDX-PackageDownloadLocation = "https://github.com/tailrocks/termrock"

[[annotations]]
path = ["**"]
precedence = "aggregate"
SPDX-FileCopyrightText = "2026 Alexey Zhokhov"
SPDX-License-Identifier = "Apache-2.0"
```

`LICENSES/` currently contains only `Apache-2.0.txt`. No `.ttf`/`.otf`/`.woff*`
exists anywhere in the repo yet. No `reuse lint` runs in CI (only the tool is
pinned in mise.toml), so REUSE verification here is by inspection.

## Commands you will need

Proven by research/tui-png-baselines/05-ci-placement-and-commands.md (Q3):

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Tests (workspace) | `mise run test` (= `cargo nextest run --workspace --all-features --locked`) | all pass, exit 0 |
| Lint | `mise run lint` (= `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`) | exit 0 |
| Format | `mise run fmt` (= `cargo fmt --all -- --check`) | exit 0 |
| License/dep policy | `cargo deny check advisories bans licenses sources` | exit 0 |
| Fast per-step check | `cargo check -p termrock-raster` | exit 0 |
| Targeted tests | `cargo nextest run -p termrock-raster` | all pass |

Note: `--locked` commands fail until `Cargo.lock` is updated; run a plain
`cargo check` first after any manifest edit (cargo updates the lockfile), and
commit `Cargo.lock` with the manifest change.

## Scope

**In scope** (the only files to create or modify):

- `crates/termrock-raster/**` — new crate (manifest, sources, tests, vendored
  font assets)
- `Cargo.toml` (workspace root) — member registration + pinned workspace deps
- `Cargo.lock` — regenerated by cargo
- `deny.toml` — license allow-list extension
- `REUSE.toml` — font annotation block
- `LICENSES/OFL-1.1.txt` — new license text file

**Out of scope** (do NOT touch, even though related):

- `crates/termrock-lookbook/**` — the color mapping is duplicated, not moved;
  lookbook is referenced only as a dev-dependency. Unifying the mapping is a
  named deferred follow-up.
- Baseline PNG files, bless tooling, story→PNG enumeration — plan 002's
  territory.
- The CI gate test and any `mise.toml` / `.github/workflows/**` change —
  plan 003's territory.
- `crates/termrock/**`, `docs/**`, `migrations/**`, `MIGRATING.md` — this
  plan is purely additive (a new crate + policy files); no public surface is
  removed or changed, so no migration file is required.

Protocol write: the hub `plans/jackin-termrock-parity/README.md` status row,
updated and staged in the same commit as this plan's final work; roadmap
item + index writes are owned by the hub's Executor protocol and happen only
when this plan is the package's first started plan (item → IN EXECUTION) —
follow the hub protocol, not this plan, for those. The hub file is never
listed in scope.

## Git workflow

Repo law (root `CLAUDE.md`):

- All work happens **directly on `main`** — no feature branches, no PRs.
- Conventional Commits, one commit per step or logical unit, each building
  independently. Example subjects for this plan:
  `feat(raster): vendor JetBrains Mono 2.304 with pinned hashes`,
  `chore(license): allow BSD licenses and declare OFL-1.1 font assets`,
  `feat(raster): add termrock-raster Buffer→PNG rasterizer`.
- Every commit carries DCO sign-off: `git commit -s`.
- Commit to `main` per step; push only after `mise run gate` exits 0 — the
  repo's documented pre-push gate ("Full pre-push gate", mise.toml:44-67).
  The per-step commands (`mise run ci`/`test`/`lint`/`fmt`) verify work but
  do not authorize a push.

## Steps

### Step 1: Vendor the pinned JetBrains Mono 2.304 faces

Download the official artifact into a temp dir (use the session scratchpad or
`mktemp -d`, never the repo tree), verify the Regular hash, copy three faces:

```bash
WORK=$(mktemp -d)
curl -fsSL -o "$WORK/jbm.zip" \
  https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip
unzip -q "$WORK/jbm.zip" -d "$WORK/jbm"
# Release-zip layout is fonts/ttf/JetBrainsMono-<Face>.ttf; if it differs, locate with:
#   find "$WORK/jbm" -name 'JetBrainsMono-Regular.ttf'
shasum -a 256 "$WORK/jbm/fonts/ttf/JetBrainsMono-Regular.ttf"
```

The Regular hash MUST be exactly
`a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f` — any
other value is a STOP (wrong or tampered artifact).

Then:

```bash
mkdir -p crates/termrock-raster/assets/fonts
cp "$WORK"/jbm/fonts/ttf/JetBrainsMono-{Regular,Bold,Italic}.ttf \
   crates/termrock-raster/assets/fonts/
shasum -a 256 crates/termrock-raster/assets/fonts/JetBrainsMono-Bold.ttf
shasum -a 256 crates/termrock-raster/assets/fonts/JetBrainsMono-Italic.ttf
```

**Record the Bold and Italic sha256 values now** — they are pinned as string
constants in the font-hash test in step 5. (Only Regular's hash was
pre-verified by research; Bold/Italic are pinned at execution time from the
same hash-verified artifact.)

**Verify**:
`shasum -a 256 crates/termrock-raster/assets/fonts/JetBrainsMono-Regular.ttf`
→ `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`; and
`ls crates/termrock-raster/assets/fonts/` → exactly three `.ttf` files.

### Step 2: License compliance (deny.toml, LICENSES, REUSE.toml)

1. `deny.toml` line 6 — extend the allow list to exactly:

   ```toml
   allow = ["Apache-2.0", "MIT", "Unicode-3.0", "Zlib", "BSD-3-Clause", "BSD-2-Clause"]
   ```

2. Add `LICENSES/OFL-1.1.txt` with the canonical SPDX text (see "Inputs to
   provide": prefer `reuse download OFL-1.1` run from the repo root, which
   writes exactly that path; else the SPDX license-list-data curl fallback).

3. Append to `REUSE.toml`, **after** the existing `**` block:

   ```toml
   [[annotations]]
   path = ["crates/termrock-raster/assets/fonts/**"]
   precedence = "aggregate"
   SPDX-FileCopyrightText = "2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)"
   SPDX-License-Identifier = "OFL-1.1"
   ```

**Verify**: `cargo deny check advisories bans licenses sources` → exit 0
(graph unchanged so far; proves the edited config parses);
`grep -c 'SIL OPEN FONT LICENSE' LICENSES/OFL-1.1.txt` → at least 1
(the SPDX license text never contains the literal token "OFL-1.1", so grep
for the license's title line instead); `grep -c 'OFL-1.1' REUSE.toml` → at
least 1; and `grep -A3 'JetBrainsMono' REUSE.toml` → shows the appended
annotation block including its `SPDX-License-Identifier = "OFL-1.1"` line
(the concrete REUSE check — inspection alone is not enough, since no
`reuse lint` runs in CI).

### Step 3: Register the crate — workspace member, pinned deps, manifest, lib skeleton

1. Root `Cargo.toml`: add `"crates/termrock-raster",` to `[workspace]`
   `members` (keep the existing ordering style — append after
   `"crates/termrock-showcase",`). Add to `[workspace.dependencies]` (exact
   pins per research ch. 06 §1; `=` requirements satisfy `wildcards = "deny"`):

   ```toml
   swash = "=0.2.10"
   tiny-skia = "=0.12.0"
   ```

   (No direct `png` dependency: tiny-skia's default `png-format` feature
   provides `Pixmap::encode_png`/`Pixmap::decode_png`.)

2. Create `crates/termrock-raster/Cargo.toml`:

   ```toml
   # SPDX-FileCopyrightText: 2026 Alexey Zhokhov
   # SPDX-License-Identifier: Apache-2.0

   [package]
   name = "termrock-raster"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true
   description = "Deterministic ratatui Buffer + RolePalette to PNG rasterizer for TermRock baselines"
   publish = false

   [dependencies]
   termrock = { version = "0.11.0", path = "../termrock" }
   ratatui = { workspace = true, default-features = false }
   swash.workspace = true
   tiny-skia.workspace = true
   unicode-width.workspace = true

   [dev-dependencies]
   termrock-lookbook = { version = "0.11.0", path = "../termrock-lookbook", default-features = false }
   sha2.workspace = true
   hex.workspace = true

   [lints]
   workspace = true
   ```

   (The `default-features = false` lookbook dev-dep mirrors
   `termrock-lookbook-web/Cargo.toml:18` and avoids the crossterm-bearing
   `native` feature. Version-plus-path dependency syntax matches the existing
   crates.)

3. Create `crates/termrock-raster/src/lib.rs` with the SPDX header, a crate
   doc comment (`//!`) stating purpose and determinism contract, module
   declarations (`mod color; mod fonts; mod render; mod compare;` with
   `pub use` re-exports added as the modules land in later steps), and the
   public geometry constants:

   ```rust
   /// Cell width in pixels (matches the frame/preview seam).
   pub const CELL_WIDTH_PX: u32 = 9;
   /// Cell height in pixels (matches the frame/preview seam).
   pub const CELL_HEIGHT_PX: u32 = 18;
   /// Font size in pixels: max(11, floor(18 * 0.78)) = 14.
   pub const FONT_SIZE_PX: f32 = 14.0;
   /// Baseline offset from cell top in pixels: floor(18 * 0.78) = 14.
   pub const BASELINE_PX: u32 = 14;
   ```

   For this step, declare only the modules that exist (create empty-but-doc'd
   `color.rs` etc. as they land; the crate must compile at every step —
   start with just the constants if preferred).

**Verify**: `cargo check -p termrock-raster` → exit 0;
`cargo deny check advisories bans licenses sources` → exit 0 (now WITH swash +
tiny-skia + arrayref in the graph — this is the spec scenario "cargo deny
passes with the raster stack"). Commit `Cargo.lock` with this step.

### Step 4: Color mapping module (`src/color.rs`)

Implement, duplicated verbatim from the excerpts in "Starting state" (values
must match exactly):

- `pub(crate) fn xterm256_to_rgb(index: u8) -> [u8; 3]` — the full 256-entry
  mapping (16 system slots, 6×6×6 cube with steps `[0,95,135,175,215,255]`,
  grey ramp `8 + (index-232)*10`).
- `pub(crate) fn color_to_rgb(color: ratatui::style::Color, is_fg: bool) -> [u8; 3]`
  — the exact `frame.rs:201-228` match arms, with `Color::Indexed` routed to
  the local `xterm256_to_rgb`.

Doc-comment both with the peer pointers
(`crates/termrock-lookbook/src/frame.rs:201-228`,
`crates/termrock-lookbook/src/palette256.rs`) and the sentence "Duplicated by
decision in plan 001; keep in lockstep until unified." Add unit tests pinning
spot values from this plan (independent of the implementation shape):
`Color::Green → [0x00,0xff,0x41]`, `Color::Reset` fg → `[0xff,0xff,0xff]` /
bg → `[0x00,0x00,0x00]`, `Indexed(21) → [0,0,255]`, `Indexed(196) → [255,0,0]`,
`Indexed(232) → [8,8,8]`, `Indexed(255) → [238,238,238]`.

**Verify**: `cargo nextest run -p termrock-raster` → new color tests pass.

### Step 5: Fonts module (`src/fonts.rs`) + font-hash test

- Embed the three faces:

  ```rust
  pub(crate) const FONT_REGULAR: &[u8] =
      include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
  pub(crate) const FONT_BOLD: &[u8] =
      include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");
  pub(crate) const FONT_ITALIC: &[u8] =
      include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf");
  ```

- Face selection, one function, this exact rule: BOLD set → Bold face;
  else ITALIC set → Italic face; else Regular. (BOLD|ITALIC maps to the Bold
  face: only three faces are vendored per the spec; this precedence is this
  plan's determination — record it in the function's doc comment.)
- Fonts parse via `swash::FontRef::from_index(bytes, 0)` — no fontdb, no
  system discovery of any kind.
- Font-hash unit test (spec scenario "Font tamper detected"): for each of the
  three `(name, bytes, pinned_hex)` triples, compute
  `hex::encode(sha2::Sha256::digest(bytes))` and assert equality with the
  pinned constant, with the font file name in the assertion message, e.g.
  `assert_eq!(actual, PINNED, "vendored font hash mismatch: JetBrainsMono-Bold.ttf")`.
  Regular's pin is
  `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`;
  Bold and Italic pins are the values recorded in step 1.

**Verify**: `cargo nextest run -p termrock-raster` → font-hash test passes.
Then prove the tamper detection once: temporarily flip one hex digit of the
Bold pin, re-run (test MUST fail naming `JetBrainsMono-Bold.ttf`), restore the
correct pin, re-run green.

### Step 6: Render core (`src/render.rs`) — Buffer + RolePalette → Pixmap → PNG

Public API (doc-comment everything; `missing_docs` is deny):

```rust
/// Errors from rendering or encoding.
pub enum RenderError { EmptyBuffer, Encode(String) }

pub fn render_pixmap(buffer: &ratatui::buffer::Buffer,
                     palette: &termrock::style::RolePalette)
    -> Result<tiny_skia::Pixmap, RenderError>;

pub fn render_png(buffer: &ratatui::buffer::Buffer,
                  palette: &termrock::style::RolePalette)
    -> Result<Vec<u8>, RenderError>;   // Pixmap::encode_png
```

Algorithm (deterministic by construction — integer-only pixel writes, the
archtest-proven approach; tiny-skia supplies the `Pixmap` surface and PNG
codec):

1. **Pixmap**: size `(area.width × 9, area.height × 18)`;
   `Pixmap::new` failure (zero area) → `RenderError::EmptyBuffer`.
2. **Ground**: fill the whole pixmap with the palette's canvas —
   `palette.style(Role::Canvas).bg` resolved through `color_to_rgb(bg, false)`;
   when the role has no bg, fall back to `[0,0,0]`. (Every buffer cell paints
   its own bg over this, so the ground is normally invisible; this is the
   `RolePalette` parameter's defined use in this plan.)
3. **Cell walk**: for `y in 0..area.height`, walk `x` from 0 while
   `x < area.width` over `&buffer[(area.x + x, area.y + y)]`:
   - Resolve paint exactly per `resolve_cell_paint` (Starting state):
     map fg with `color_to_rgb(fg, true)`, bg with `color_to_rgb(bg, false)`;
     if `REVERSED`, swap; if `DIM`, per-channel `(u16::from(c) * 6 / 10) as u8`
     — once, after the swap, exactly as `frame.rs:180-198`. All other
     modifiers except the six the spec names are ignored.
   - **Span**: `span = UnicodeWidthStr::width(cell.symbol()).clamp(1, 2)`
     (unicode-width is the workspace dep), clamped to the remaining columns.
   - **Background**: write bg RGBA (`[r, g, b, 255]`, opaque so premultiplied
     equals straight) over the rect `x*9 .. (x+span)*9` × `y*18 .. (y+1)*18`
     directly into `pixmap.data_mut()`.
   - **Glyphs**: skip when the symbol is empty or all-whitespace. Otherwise,
     for each `ch` in `cell.symbol().chars()`: `glyph = face.charmap().map(ch)`
     on the selected face (step 5 rule); scale via one `ScaleContext`
     (created per `render_pixmap` call) with
     `ctx.builder(font).size(FONT_SIZE_PX).hint(true).build()`; rasterize with
     `Render::new(&[Source::Outline]).format(Format::Alpha).render(&mut scaler, glyph)`.
     Import paths: `Format` and `Placement` are the re-exported zeno types —
     `swash::zeno::Format`, `swash::zeno::Placement`.
     `Render::render(&mut scaler, glyph)` returns `Option<Image>`; on `None`,
     draw nothing for that glyph (deterministic skip — bg, underline, and
     strikethrough spans still paint).
     Composite the alpha image at pen `(x*9 + placement.left, y*18 + 14 - placement.top)`
     using integer source-over per coverage byte `a`:
     `out = ((u16::from(fg_c) * u16::from(a) + u16::from(dst_c) * u16::from(255 - a)) / 255) as u8`,
     alpha stays 255. Clip all glyph pixels to the cell's span rect (prevents
     neighbor-cell bleed; keeps per-cell pixel assertions exact). Unmapped
     chars produce glyph id 0 (`.notdef`) and render as the font's tofu box —
     acceptable. Combining marks (extra chars in one symbol) draw at the same
     pen origin.
   - **UNDERLINED**: fill fg over rows `y*18 + 15` and `y*18 + 16`
     (thickness `max(1, round(18*0.1)) = 2`, top offset `18 - 2 - 1 = 15` —
     the web-path-consistent offset), full span width.
   - **CROSSED_OUT**: same thickness, mid-cell: rows `y*18 + 8` and
     `y*18 + 9` (top offset `(18 - 2) / 2 = 8`; the exact mid-cell offset is
     this plan's determination — the spec fixes only "mid-cell span").
   - Advance `x += span` (a width-2 symbol consumes its shadow cell, so the
     following cell never paints a duplicate glyph — ratatui leaves it as a
     skip/blank cell and the walk never visits it).

Optional within-call glyph-bitmap memoization (`HashMap<(face, char), image>`)
is allowed; iteration order must not affect output (it cannot, with per-pixel
deterministic compositing).

**Verify**: `cargo check -p termrock-raster` → exit 0; `mise run lint` →
exit 0.

### Step 7: Pixel-compare helper (`src/compare.rs`)

The only exported comparison predicate (guardrail N3 — decoded pixels, zero
tolerance, never encoded bytes):

```rust
/// Outcome of a zero-tolerance decoded-pixel comparison.
#[derive(Debug, PartialEq, Eq)]
pub enum PixelDiff {
    /// The two decoded images have different dimensions.
    DimensionMismatch {
        /// (width, height) of the first image.
        a: (u32, u32),
        /// (width, height) of the second image.
        b: (u32, u32),
    },
    /// First differing pixel in row-major scan order.
    FirstDifference {
        /// X coordinate of the first differing pixel.
        x: u32,
        /// Y coordinate of the first differing pixel.
        y: u32,
        /// RGBA of that pixel in the first image.
        a: [u8; 4],
        /// RGBA of that pixel in the second image.
        b: [u8; 4],
    },
}

/// Decode two PNGs and compare every pixel at zero tolerance.
pub fn compare_png_pixels(a: &[u8], b: &[u8]) -> Result<(), PixelDiff>;
```

`PixelDiff` MUST carry `#[derive(Debug, PartialEq, Eq)]` (the test plan
compares results with `==` and pattern matches) and doc comments on the
type, every variant, and every field as sketched — the workspace lints deny
`missing_docs`.

Decode with `tiny_skia::Pixmap::decode_png`; on dimension mismatch return
`DimensionMismatch`; else scan row-major and return the **first** differing
coordinate with both RGBA values. Implement `Display` (or derive `Debug` and
provide a formatted message) so a failure names the coordinate. Decode
failures may map into `PixelDiff`/panic-with-message — pick one and document
it; a corrupt PNG must not be reported as "equal".

**Verify**: `cargo check -p termrock-raster` → exit 0.

### Step 8: Test suite — every spec scenario

Create `crates/termrock-raster/tests/raster.rs` (integration tests; unit
tests from steps 4–5 stay in their modules). Tests specified in "Test plan"
below.

**Verify**: `cargo nextest run -p termrock-raster` → all pass.

### Step 9: Full gates and status row

Run the workspace gates, then update this plan's row in
`plans/jackin-termrock-parity/README.md`: set the Status cell to the exact
text `DONE`. The final commit stages the in-scope files PLUS the hub README
status-row change together — one commit. Roadmap item + index writes are
owned by the hub's Executor protocol and happen only when this plan is the
package's first started plan (item → IN EXECUTION) — follow the hub
protocol, not this plan, for those.

**Verify**: `mise run test` → exit 0; `mise run lint` → exit 0;
`mise run fmt` → exit 0; `cargo deny check advisories bans licenses sources`
→ exit 0; then, after staging both the in-scope files and the hub README
status-row change, `git status` shows nothing modified or untracked outside
the Scope list plus `plans/jackin-termrock-parity/README.md`.

## Test plan

All in `crates/termrock-raster/tests/raster.rs` unless noted. Expected values
come from this plan's inlined seam constants and spec scenarios — an
independent source from the implementation. Structural model: the existing
integration-test layout of `crates/termrock-lookbook/tests/goldens.rs`
(plain `#[test]` fns in `tests/`).

1. `panel_story_png_has_exact_size_and_phosphor_border` (spec: "Panel story
   renders"): look up `story_by_id("panel/focused")`, paint it at its story
   size with the phosphor palette through the public lookbook API — bind
   `let palette = RolePalette::default();`, then
   `TestBackend::new(story.width, story.height)` + `Terminal::new` +
   `story.make_interactor()` + `interactor.set_theme(palette.clone())`
   (`set_theme` takes the palette by value; clone the binding) +
   `terminal.draw(|f| interactor.render(f, f.area()))`. Clone the painted
   buffer, then force-set one interior cell of the clone (e.g. (1, 1)) to
   symbol `"█"` (U+2588) with fg `Color::Rgb(0, 255, 65)` — the phosphor
   `Role::BorderFocused` green in a full-block glyph: full blocks guarantee
   full-coverage (alpha 255) pixels, so the exact-value assertion is robust
   where an anti-aliased box-drawing border stroke would not be. Then
   `render_png(&buffer, &palette)`. Decode with `Pixmap::decode_png` and
   assert width `== story.width as u32 * 9`, height
   `== story.height as u32 * 18`, and that at least one pixel equals RGBA
   `(0, 255, 65, 255)` (the phosphor border color present in the rendered
   pixels).
2. `wide_grapheme_spans_two_cells_and_shadow_cell_is_ignored` (spec: "Wide
   grapheme spans two cells"): buffer 4×1; `buffer.set_string(0, 0, "界", style)`
   with green fg (unicode-width of `界` is 2; JBM has no CJK coverage so the
   glyph is the font's `.notdef` box — the scenario tests span logic, not
   CJK). Render A. Build buffer B identical except the shadow cell (1,0) is
   force-set to garbage (e.g. symbol `"X"` via direct cell mutation,
   `buf[(1, 0)].set_symbol("X")`). Render B. Assert
   `compare_png_pixels(&png_a, &png_b) == Ok(())` — pixel-identical — which
   proves the shadow cell is ignored: step 6's span walk advances
   `x += span` and never visits cell (1,0), so nothing it contains can paint
   (this is the mechanism by which "the following cell paints no duplicate
   glyph"). Also assert the pixel region of cells 0–1 (x 0..18) in render A
   is not two identical 9-px halves — the glyph is drawn once across two
   cell widths, not stamped per cell.
3. `italic_selects_italic_face` (spec: "Italic survives"): 1×1 buffer, symbol
   `"a"`, green fg; render once plain, once with `Modifier::ITALIC`. Assert
   the two PNGs differ (`compare_png_pixels` → `Err`). Face provenance: the
   pipeline contains no slant/skew transform (step 6 has none), so a
   difference can only come from the Italic face; the font-hash test pins that
   face as genuine JBM Italic 2.304.
4. `bold_selects_bold_face`: same shape as (3) with `Modifier::BOLD` — PNGs
   differ.
5. `dim_darkens_exactly_once` (spec: "Dim darkens once"): 1×1 buffer, symbol
   `"█"` (U+2588, covered by JBM), fg `Color::Rgb(0, 255, 65)`, bg black,
   `Modifier::DIM`. Decode; assert at least one pixel equals
   `(0, 153, 39, 255)` (the spec's 0.6× value: 255·6/10 = 153, 65·6/10 = 39)
   and **no** pixel equals `(0, 255, 65, 255)` (undarkened) — the full-block
   glyph `█` guarantees full-coverage (alpha 255) pixels, which are exact
   under the integer compositing formula, making the exact-value assertions
   robust.
6. `underline_paints_web_consistent_rows`: 1×1 buffer, symbol `" "`, fg
   `Color::Rgb(0, 255, 65)`, `Modifier::UNDERLINED`. Assert pixels at
   `(x, 15)` and `(x, 16)` for all `x in 0..9` equal `(0, 255, 65, 255)` and
   row 14 and row 17 stay bg.
7. `crossed_out_paints_mid_cell_rows`: same with `Modifier::CROSSED_OUT`;
   rows 8 and 9 are fg, rows 7 and 10 are bg.
8. `reversed_swaps_fg_bg`: 1×1 buffer, symbol `" "`, fg
   `Color::Rgb(0, 255, 65)`, bg `Color::Rgb(0, 0, 0)`, `Modifier::REVERSED`
   → every pixel `(0, 255, 65, 255)`.
9. `double_render_identical` (spec: "Double render identical"; determinism
   self-test): build one fixture buffer exercising text + box glyphs
   (`┌`, `─`, `█`) + all six modifiers across a few cells; render twice in
   one process. Assert `hex::encode(Sha256::digest(pixmap_a.data())) ==
   hex::encode(Sha256::digest(pixmap_b.data()))` AND equal sha256 of the two
   PNG byte vectors AND `compare_png_pixels(&png_a, &png_b) == Ok(())`.
10. `pixel_compare_names_first_differing_coordinate` (spec: "One-pixel
    difference caught"): build two 3×2-px `Pixmap`s directly, both filled
    entirely with opaque pixels (alpha 255) BEFORE the flip — transparent
    (alpha 0) pixels are destroyed by premultiplication on encode, which
    would erase the difference — then flip one bit in one channel of pixel
    (2, 1) in one of them; encode both via `encode_png`; assert
    `compare_png_pixels` returns `FirstDifference { x: 2, y: 1, .. }`.
11. `pixel_compare_dimension_mismatch`: two different-sized pixmaps →
    `DimensionMismatch`.
12. Module unit tests from steps 4–5: color-table spot values; three font
    hashes (failure message names the font file).

**Verify**: `cargo nextest run -p termrock-raster` → all pass, including the
12 groups above; then `mise run test` → full workspace green.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo check -p termrock-raster` exits 0
- [ ] `mise run test` exits 0; a test exists and passes for every spec
      scenario: Panel story renders, Wide grapheme spans two cells, Italic
      survives, Dim darkens once, Font tamper detected, cargo deny passes,
      Double render identical, One-pixel difference caught
- [ ] `mise run lint` exits 0 and `mise run fmt` exits 0
- [ ] `cargo deny check advisories bans licenses sources` exits 0 with swash +
      tiny-skia in `Cargo.lock`
- [ ] `crates/termrock-raster/assets/fonts/` contains exactly
      JetBrainsMono-{Regular,Bold,Italic}.ttf; the Regular file hashes to
      `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`
- [ ] `LICENSES/OFL-1.1.txt` exists; `REUSE.toml` contains an `OFL-1.1`
      annotation for `crates/termrock-raster/assets/fonts/**`; `deny.toml`
      allow list contains `BSD-3-Clause` and `BSD-2-Clause`
- [ ] `grep -n 'fontdb\|cosmic-text\|ab_glyph\|fontdue' crates/termrock-raster/Cargo.toml`
      → no matches, and
      `grep -rn '^use \(fontdb\|cosmic_text\|ab_glyph\|fontdue\)' crates/termrock-raster/src/`
      → no matches (no shaper/discovery crates in the manifest or code
      imports; prose/doc mentions of these names are allowed)
- [ ] No files outside the in-scope list modified (`git status`) — excluding
      the protocol write: the hub `plans/jackin-termrock-parity/README.md`
      status row, updated and staged in the same commit as this plan's final
      work; roadmap item + index writes are owned by the hub's Executor
      protocol and happen only when this plan is the package's first started
      plan (item → IN EXECUTION) — follow the hub protocol, not this plan,
      for those
- [ ] The hub `plans/jackin-termrock-parity/README.md` status row for this
      plan reads exactly `DONE`, staged in the same commit as this plan's
      final work

## STOP conditions

Stop and report back (do not improvise) if:

- Any precondition fails, or "Starting state" does not match reality (drift
  in `Cargo.toml`, `deny.toml`, `REUSE.toml`, `frame.rs:173-228`, or
  `palette256.rs` against the inlined excerpts).
- The downloaded `JetBrainsMono-Regular.ttf` does not hash to
  `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`, or the
  font zip / OFL text is unreachable.
- **The determinism self-test fails** (double render not byte-identical in raw
  RGBA or PNG). This falsifies ledger assumption **A1** ("`png` crate emits
  deterministic bytes at a fixed version with fixed options" — falsified by
  "double-encode diff in the determinism self-test failing"). Report; do not
  paper over with tolerances or by comparing less.
- The work requires touching any file owned by plans 002 (baseline files,
  bless tooling) or 003 (gate test, mise/CI wiring), or any other
  out-of-scope file, or violating the Must NOT section.
- `swash =0.2.10` or `tiny-skia =0.12.0` fails to resolve/compile on the
  pinned toolchain, or `Pixmap::decode_png` is unavailable — the pinned-stack
  facts would then be stale; report rather than floating the versions.
- A step's verification fails twice after a reasonable fix attempt.

## Maintenance notes

- **Plan 002** consumes `render_png` to generate the committed baseline set;
  **plan 003** consumes `compare_png_pixels` as the gate predicate and may add
  a normal dependency from lookbook-side code onto `termrock-raster`. This
  crate's dev-dependency on `termrock-lookbook` is compatible with that
  direction (dev edges may close a cycle); keep `termrock-raster`'s *normal*
  dependency set free of lookbook.
- **Deferred root cause (scheduled, not dropped)**: the named-color/xterm-256
  mapping now exists in two places (`termrock-raster::color` and lookbook's
  private `frame.rs`/`palette256.rs`). Unify by making lookbook consume
  `termrock-raster::color` (or a shared home) in a follow-up that may touch
  lookbook; plan 003's pixel gate plus this plan's spot-value tests guard the
  interim. A reviewer should scrutinize that the duplicated values match the
  excerpts in this plan exactly.
- **Plan-level determinations to re-examine if the spec evolves**: CROSSED_OUT
  offset (rows 8–9 of 18), BOLD|ITALIC → Bold face (no BoldItalic vendored),
  glyph clipping to the span rect, `hint(true)`, and the `RolePalette` ground
  fill (`Role::Canvas` bg, normally overpainted by cells).
- **Ledger A3** (macOS-blessed PNGs match Linux CI renders) remains open until
  plan 003's first cross-OS run; this crate's zero-OS-input construction and
  the measured cross-arch identity are the basis. If A3 falsifies there, the
  documented fallback is bless-in-container or CI-side bless — not a change to
  this crate's determinism contract.
- The web preview path double-darkens DIM (0.6 Rust-side × 0.7 web-side);
  this crate intentionally matches the Rust-side single 0.6. PNGs will differ
  from web-canvas pixels on dim cells until that separate cleanup lands —
  expected, recorded in the spec README.
