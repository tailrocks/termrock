# Render pipeline

## Purpose

The pure-Rust rasterizer that turns a painted ratatui `Buffer` into a PNG —
the engine every baseline, comparison pair, and CI check consumes. Chosen by
decision D9 after libghostty was ruled out; realism comes from real glyph
scaling/hinting and rasterization of the true cell grid.
Anchors: F5(engine), B2, D9, N3 · Evidence: research/tui-png-baselines/04-determinism-ci-storage.md, research/tui-png-baselines/06-rasterizer-facts-and-archtest.md

## Requirements

### Requirement: Buffer-to-PNG rasterization
A new `termrock-raster` library crate (workspace member) SHALL render a
ratatui `Buffer` plus a `RolePalette` to an RGBA pixmap and encode it as PNG,
using `swash` (glyph scaling/hinting/rasterization via `charmap.map` per
glyph — no shaper) and `tiny-skia` (compositing + `Pixmap::encode_png`),
with cell geometry 9×18 px, font size 14 px, baseline 14 px from cell top —
matching the existing seam constants (`frame.rs:345-348`,
`TerminalPreview.tsx:67-68`, ch. 06 §3). Color resolution SHALL reuse the
same named-color→RGB and xterm-256 mapping as `frame.rs:201-228`/
`palette256.rs`.
Covers: F5, B2, D9 · Evidence: ch. 06 §1, §3, §4

#### Scenario: Panel story renders
- **GIVEN** the `panel/focused` lookbook story painted into a Buffer at its story size with the phosphor `RolePalette`
- **WHEN** `termrock-raster` renders it
- **THEN** a PNG of exactly (cols×9) × (rows×18) px is produced
- **AND** the phosphor border color appears in the border cell pixels

#### Scenario: Wide grapheme spans two cells
- **GIVEN** a Buffer containing a double-width grapheme
- **WHEN** rendered
- **THEN** the glyph is drawn across two cell widths and the following cell paints no duplicate glyph

### Requirement: Full modifier fidelity
The rasterizer SHALL consume the unresolved `Buffer` (not the lossy
`FrameCell` JSON) and render every modifier the paint system uses: BOLD via
the vendored Bold face, ITALIC via the vendored Italic face, UNDERLINED as a
row span (thickness `max(1, round(18*0.1))` at offset consistent with the
web path), CROSSED_OUT as a mid-cell span, REVERSED by fg/bg swap, and DIM
by a single 0.6 fg darken — the Rust-side resolution from
`frame.rs:180-198`, applied exactly once.
Covers: F5, B2 · Evidence: ch. 03 Q1/Q2 (frame JSON drops ITALIC/CROSSED_OUT; Buffer is the full-fidelity seam), ch. 06 §3

#### Scenario: Italic survives
- **GIVEN** a Buffer cell styled ITALIC (in real use, e.g. `stories.rs:22190`)
- **WHEN** rendered
- **THEN** the glyph comes from the Italic face, not a slanted Regular

#### Scenario: Dim darkens once
- **GIVEN** a cell with DIM and fg (0,255,65)
- **WHEN** rendered
- **THEN** the drawn fg is (0,153,39) — 0.6×, single application

### Requirement: Vendored pinned fonts
The crate SHALL embed JetBrains Mono v2.304 Regular, Bold, and Italic TTFs
(official release artifact; Regular sha256
`a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`) via
`include_bytes!`, never system font discovery — no fontdb, no fontconfig, no
CoreText. A unit test SHALL assert each embedded font's sha256.
Covers: F5, B2, D9 · Evidence: ch. 06 §2 (license + sha), ch. 04 §1 (OS-stack variance)

#### Scenario: Font tamper detected
- **GIVEN** the crate's font-hash test
- **WHEN** an embedded TTF byte differs from the pinned sha256
- **THEN** the test fails naming the font file

### Requirement: License compliance for the raster stack
The change SHALL extend `deny.toml:6`'s allow list with `BSD-3-Clause` and
`BSD-2-Clause` (tiny-skia, arrayref — ch. 06 §2), add `LICENSES/OFL-1.1.txt`,
and add a `REUSE.toml` annotation scoping the vendored font files to
`OFL-1.1` so the `**` = Apache-2.0 annotation no longer misdeclares them.
`cargo deny check` MUST pass afterwards.
Covers: F5 · Evidence: ch. 06 §2

#### Scenario: cargo deny passes with the raster stack
- **GIVEN** termrock-raster with swash + tiny-skia in the workspace graph
- **WHEN** `cargo deny check advisories bans licenses sources` runs
- **THEN** it exits 0

### Requirement: Determinism self-test
The crate SHALL ship a test that renders a fixture Buffer twice and asserts
byte-identical raw RGBA and byte-identical PNG output, and a pixel-compare
helper that decodes two PNGs and asserts pixel equality at zero tolerance —
the only comparison predicate (N3: never gate on undecoded PNG bytes).
Covers: F5, B2, N3 · Evidence: ch. 06 §4 (measured identity), ch. 04 §2 (pixel-compare norm)

#### Scenario: Double render identical
- **WHEN** the same Buffer renders twice in one process
- **THEN** raw RGBA sha256 and PNG sha256 are equal

#### Scenario: One-pixel difference caught
- **GIVEN** two PNGs differing in one pixel's fg by one bit
- **WHEN** the pixel-compare helper runs
- **THEN** it reports inequality naming the first differing coordinate
