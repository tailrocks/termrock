# 06 — Rasterizer implementation facts and cross-arch test

Questions: (1) current crate versions and the minimal pure-Rust set for monospaced grid → PNG, including whether per-glyph rendering suffices without shaping; (2) vendored-font candidates, JetBrains Mono license, and what this repo's deny.toml / REUSE / LICENSES machinery says about a bundled OFL asset; (3) exact pixel geometry, font, and attribute paint the existing frame/preview seam uses, so PNGs can match; (4) measured cross-arch bit-identity of the swash + tiny-skia stack (aarch64 native vs x86_64 under Rosetta, double-run identity); (5) Rust-side PNG decode/compare options for the CI gate and workspace dev-dependency policy.
Informs: jackin-termrock-parity
Method: web + codebase read + measured experiment (commands recorded)
Vetted: 2026-08-16

## Findings

### 1. Crate versions and the minimal set

Current stable versions, read 2026-08-16 from the crates.io API (`curl https://crates.io/api/v1/crates/<name>` → `max_stable_version`):

| crate | version | updated | license | docs |
|---|---|---|---|---|
| swash | 0.2.10 | 2026-07-17 | Apache-2.0 OR MIT | <https://docs.rs/swash/0.2.10> |
| tiny-skia | 0.12.0 | 2026-02-02 | **BSD-3-Clause** | <https://docs.rs/tiny-skia/0.12.0> |
| fontdb | 0.24.0 | 2026-07-29 | MIT | <https://docs.rs/fontdb/0.24.0> |
| png | 0.18.1 | 2026-02-14 | MIT OR Apache-2.0 | <https://docs.rs/png/0.18.1> |
| zeno | 0.3.3 | 2025-05-08 | Apache-2.0 OR MIT | <https://docs.rs/zeno/0.3.3> |
| cosmic-text | 0.19.0 | 2026-04-22 | MIT OR Apache-2.0 | <https://docs.rs/cosmic-text/0.19.0> |
| ab_glyph | 0.2.32 | 2025-09-28 | Apache-2.0 | <https://docs.rs/ab_glyph/0.2.32> |
| fontdue | 0.9.4 | 2026-07-29 | MIT OR Apache-2.0 OR Zlib | <https://docs.rs/fontdue/0.9.4> |
| unicode-width | 0.2.2 | 2025-10-06 | (already a workspace dep) | <https://docs.rs/unicode-width/0.2.2> |
| image | 0.25.10 | 2026-03-10 | MIT OR Apache-2.0 | <https://docs.rs/image/0.25.10> |

Licenses read from the vendored registry manifests after building the archtest (`grep '^license' ~/.cargo/registry/src/*/<crate>/Cargo.toml`) and from `crates.io/api/v1/crates/{image,png}/versions`. (confidence: HIGH)

- **Shaping is not required for per-glyph grid rendering with swash.** The archtest (§4) renders every cell with only `FontRef::from_index(bytes, 0)` → `font.charmap().map(ch)` → `ScaleContext`/`Scaler` → `Render::new(&[Source::Outline]).format(Format::Alpha).render(&mut scaler, glyph_id)`; no `shape` module import, compiles and runs on swash 0.2.10 — empirical, plus docs: the scale module is "Scaling, hinting and rasterization of visual glyph representations" and takes glyph ids directly — <https://docs.rs/swash/0.2.10/swash/scale/> (confidence: HIGH)
- swash 0.2.10 default features = `["std", "scale", "render"]`; `scale` pulls `zeno` + `yazi`, `render` adds `zeno/eval`; font parsing is delegated to Google fontations (`skrifa >=0.31.1,<=0.44`) — swash's own published manifest, `~/.cargo/registry/src/*/swash-0.2.10/Cargo.toml`. **zeno arrives automatically; it is not a direct dependency the repo would name.** Resolved in the archtest lock: skrifa 0.44.0, read-fonts 0.41.0, zeno 0.3.3, yazi 0.2.1. (confidence: HIGH)
- swash's scale module includes **hinting** ("Scaling, hinting and rasterization") — <https://docs.rs/swash/0.2.10/swash/scale/>. ab_glyph's docs describe "loading, scaling, positioning and rasterizing OpenType font glyphs" via `Font::outline_glyph` → `OutlinedGlyph::draw(|x,y,coverage|)` with **no shaping and no hinting mentioned** — <https://docs.rs/ab_glyph/0.2.32/ab_glyph/>. fontdue is "a font parser, rasterizer, and layout tool", no_std+alloc, `Font::rasterize(char, px)` → `(Metrics, Vec<u8> coverage)` — <https://docs.rs/fontdue/0.9.4/fontdue/>. All three can rasterize one glyph at a time; capability difference on record is hinting (swash yes; ab_glyph/fontdue docs do not claim it). (confidence: HIGH for swash/fontdue quotes, MED for ab_glyph hinting absence — argument from docs silence)
- cosmic-text 0.19.0 is the full-stack option: "abstractions for shaping, font discovery, font fallback, layout, rasterization, and editing" — shaping via harfrust, discovery via fontdb, raster via swash (`FontSystem`, `Buffer`, `SwashCache`) — <https://docs.rs/cosmic-text/0.19.0/cosmic_text/>. Strictly a superset of what a fixed-grid needs. (confidence: HIGH)
- fontdb is a font *database/discovery* layer; the archtest proves it is unnecessary for a vendored single font: `swash::FontRef::from_index(include_bytes!(...), 0)` parses the TTF directly. (confidence: HIGH, empirical)
- **Empirical minimal set for grid → PNG: `swash` + `tiny-skia` (its default `png-format` feature bundles `png` for `Pixmap::encode_png`) — 38 crates total in the resolved graph, no fontdb, no shaper, no image crate.** Archtest `Cargo.lock`, §4. `unicode-width 0.2.2` is already a workspace dependency for wide-cell logic — `/Users/donbeave/Projects/tailrocks/termrock/Cargo.toml:33`. (confidence: HIGH)
- tiny-skia 0.12.0 default features = `["std", "simd", "png-format"]` with `simd = []` (internal, no external SIMD crates such as `wide`/`safe_arch` in the graph) — published manifest `~/.cargo/registry/src/*/tiny-skia-0.12.0/Cargo.toml`; 0.12.0 is now under the linebender org and added WebAssembly relaxed-SIMD support — <https://github.com/linebender/tiny-skia/blob/master/CHANGELOG.md> (confidence: HIGH)

### 2. Vendored font candidates, license, and repo asset policy

- Existing previews assume **JetBrains Mono first**: `PREVIEW_MONO_STACK = '"JetBrains Mono", "SF Mono", "Cascadia Mono", ui-monospace, Menlo, Consolas, monospace'` — `docs/src/components/preview-metrics.ts:374-376`; docs load it from Google Fonts (weights 400;600) — `docs/src/styles/app.css:1` and `--font-mono` at `app.css:12`; the canvas host explicitly waits on `document.fonts.load('...px JetBrains Mono')` — `docs/src/components/TerminalPreview.tsx:523`. The offline SVG export instead uses a system stack, `font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="14"` — `crates/termrock-lookbook/src/svg.rs:195`. (confidence: HIGH)
- Ghostty ships JetBrains Mono as its built-in default: "the built-in JetBrains Mono now uses a variable font rather than 4 static ones" — Ghostty 1.2.0 release notes, <https://ghostty.org/docs/install/release-notes/1-2-0>. (confidence: HIGH)
- asciinema agg's default text `--font-family` is `"JetBrains Mono,Fira Code,SF Mono,Menlo,Consolas,DejaVu Sans Mono,Liberation Mono"` — but agg does **not** bundle JBM (it bundles Symbols Nerd Font and monochrome Noto Emoji; JBM is resolved from the system) — <https://docs.asciinema.org/manual/agg/usage/>. (confidence: HIGH)
- JetBrains Mono license: **SIL Open Font License 1.1**. Primary source: `OFL.txt` inside the official release artifact `https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip` (latest release tag v2.304, published 2023-01-14, per `api.github.com/repos/JetBrains/JetBrainsMono/releases/latest`): "This Font Software is licensed under the SIL Open Font License, Version 1.1." Copyright line: "Copyright 2020 The JetBrains Mono Project Authors". `JetBrainsMono-Regular.ttf` from that zip: sha256 `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`. (confidence: HIGH)
- `deny.toml` license allowlist is `allow = ["Apache-2.0", "MIT", "Unicode-3.0", "Zlib"]` — `/Users/donbeave/Projects/tailrocks/termrock/deny.toml:6`. **OFL-1.1 is not listed — but deny.toml does not govern non-crate assets at all**: cargo-deny evaluates "the license requirements specified by each crate" from the Cargo dependency graph (<https://embarkstudios.github.io/cargo-deny/checks/licenses/index.html>); a vendored `.ttf` is invisible to it. The check runs in the gate: `cargo deny check advisories bans licenses sources` — `mise.toml:62` and `.github/workflows/hygiene.yml:67`, cargo-deny pinned 0.20.2 at `mise.toml:13`. (confidence: HIGH)
- **The crate allowlist does bite the rasterizer crates**: tiny-skia and tiny-skia-path are `BSD-3-Clause`, and swash's graph carries `arrayref` (`BSD-2-Clause`) — neither ID is in `deny.toml:6`, so adding the stack (even as dev-dependency) fails `cargo deny check licenses` without an allowlist edit. cargo-deny includes dev-dependencies unless `[graph] exclude-dev = true` ("If set to `true`, all `dev-dependencies` … are not included in the crate graph used for any of the checks" — <https://embarkstudios.github.io/cargo-deny/checks/cfg.html>); this deny.toml has no `[graph]` section (`deny.toml:1-14`). (confidence: HIGH for licenses and config; MED that exclude-dev defaults to false — implied by the docs' phrasing)
- Repo-wide REUSE annotation claims **everything** is Apache-2.0: `[[annotations]] path = ["**"] … SPDX-License-Identifier = "Apache-2.0"` — `REUSE.toml` (annotation block, lines 6-11). A vendored OFL font would be misdeclared until a second annotation block is added. The `reuse` tool is pinned (`pipx:reuse = 6.2.0`, `mise.toml:20`) but no `reuse lint` invocation exists in mise tasks, workflows, or scripts (grep over `mise.toml`, `.github/workflows/*.yml`, `scripts/`). (confidence: HIGH for the annotation; MED that no lint runs anywhere)
- **No precedent for third-party binary assets**: `LICENSES/` contains only `Apache-2.0.txt`; `find` for `*.ttf|*.woff*|*.otf` over the repo returns nothing; the only JBM delivery today is the Google Fonts CDN import at `docs/src/styles/app.css:1`. (confidence: HIGH)

### 3. What the frame/render seam gives the rasterizer

- Cell geometry: `CELL_WIDTH_PX: u16 = 9` / `CELL_HEIGHT_PX: u16 = 18` — `crates/termrock-lookbook/src/frame.rs:346,348`; mirrored as `DEFAULT_CELL_W = 9` / `DEFAULT_CELL_H = 18` — `docs/src/components/TerminalPreview.tsx:67-68`; and `CELL_W/CELL_H = 9/18` in the SVG export — `crates/termrock-lookbook/src/svg.rs:180-181`. Surface under cells `#0a0a0a` — `TerminalPreview.tsx:71,139-140`. (confidence: HIGH)
- Derived text metrics at 9×18: font size `max(11, floor(18*0.78)) = 14 px` and baseline `floor(18*0.78) = 14 px` from cell top — `docs/src/components/preview-metrics.ts:7-14`, applied at `TerminalPreview.tsx:141-142`; SVG export hard-codes the same `font-size="14"` / `y = 14` (`svg.rs:195`, test at `svg.rs:370`). Canvas disables smoothing and ligatures (`imageSmoothingEnabled = false`, `font-feature-settings "liga" 0, "calt" 0`) — `TerminalPreview.tsx:133-138`. (confidence: HIGH)
- `encode_buffer` exports a flat truecolor grid: per-cell `ch, fg, bg, bold, dim, underline, reversed` — `frame.rs:149-171`; **REVERSED is resolved Rust-side by swapping fg/bg** (`frame.rs:180-183`) and **DIM is darkened Rust-side to 6/10** (`frame.rs:184-189`) while the flags are still exported (`frame.rs:191-198`). (confidence: HIGH)
- Web paint of attributes: **bold** → canvas `font-weight` '700' via `boldFontWeight` (`preview-metrics.ts:53-56`, used `TerminalPreview.tsx:181-182`; note app.css loads only weights 400;600 — `app.css:1` — so canvas 700 is browser-synthesized); **underline** → continuous per-row spans, thickness `max(1, round(h*0.1))` (=2 px at h=18) at offset `h-thickness-1` (=15) — `preview-metrics.ts:63-71,77-93`, stroked at `TerminalPreview.tsx:204-222`; **dim** → fg × 0.7 then Ghostty-style min-contrast ≥1.6 vs bg — `preview-metrics.ts:160-175,125-155`, applied via `paintFg` at `TerminalPreview.tsx:104-107,183`; **reversed** → carried on `FrameCell` (`TerminalPreview.tsx:51`) but never consulted by `paintCanvas` — the swap already happened in Rust. (confidence: HIGH)
- **Dim is applied twice on the web path**: `frame.rs:184-189` scales fg to 0.6 and still sets `dim: true`; `resolvePaintFg` scales by 0.7 again (net 0.42 before the contrast floor) — `preview-metrics.ts:160-175`. A PNG rasterizer that matches web pixels must replicate both stages (or the seam must be reconciled); one that matches only the Rust-resolved cells will differ on dim cells. (confidence: HIGH, code read of both stages)
- Box-drawing/block glyphs bypass the font entirely on the web path: vector strokes/fills per `boxStrokeForGlyph`/`boxStrokeGeometry` (light stroke `max(1, min(w,h)*0.12)`, heavy `max(light*1.6, min(w,h)*0.2)`, eighth-block fills, shade via alpha 0.25/0.5/0.75) — `preview-metrics.ts:204-372`, painted at `TerminalPreview.tsx:155-179`. Wide glyphs span 2 cells (`glyphCellSpan`, `preview-metrics.ts:382-389`; `TerminalPreview.tsx:186-200`). (confidence: HIGH)
- Existing baseline precedent is text, blessed in-repo: committed cell dumps per flagship story diffed on every run, blessed via `TERMROCK_BLESS_PREVIEWS` — `crates/termrock-lookbook/tests/goldens.rs:80-129`, tasks `bless-previews` / `preview-goldens` in `mise.toml`. SVG byte-identity was explicitly rejected as "platform-sensitive (font metrics / glyph widths)" — comment at `crates/termrock-lookbook/src/svg.rs:137-140`. (confidence: HIGH)

### 4. Measured: cross-arch bit-identity of swash + tiny-skia

Test project (scratchpad only, never in-repo): `/private/tmp/claude-501/-Users-donbeave-Projects-tailrocks-termrock/d206de3d-a988-4b17-be91-66ba8c58af80/scratchpad/archtest/` — vendored `JetBrainsMono-Regular.ttf` (v2.304, sha256 `a0bf60ef…`, from the official release zip URL in §2), renders three 9×18-cell rows (ASCII unhinted, ASCII **hinted**, box/block glyphs U+250C…U+2593) at size 14 px via `swash` `Source::Outline`/`Format::Alpha`, plus a tiny-skia band exercising its own AA pipeline (`fill_rect`, winding-fill triangle, circle), composites coverage with integer-only source-over, then prints SHA-256 of the raw premultiplied-RGBA buffer and of `Pixmap::encode_png()` bytes.

Method (exact commands, host: Darwin 25.5.0, rustc 1.97.1, `arch -x86_64 /usr/bin/true` → Rosetta OK):

```
cargo build --release                                  # aarch64-apple-darwin native
./target/release/archtest out-arm64.png                # run 1
./target/release/archtest                              # run 2
cargo build --release --target x86_64-apple-darwin     # target was already installed
./target/x86_64-apple-darwin/release/archtest out-x86_64.png   # Mach-O x86_64, executed via Rosetta 2
./target/x86_64-apple-darwin/release/archtest          # run 2
cmp out-arm64.png out-x86_64.png
```

Results — **all four runs bit-identical** (confidence: HIGH, measured):

| check | aarch64 native (run 1/2) | x86_64 under Rosetta (run 1/2) |
|---|---|---|
| raw RGBA sha256 | `61f9e95e6a274d108e9d9f2a02e5b3ac8d4a58b0ff229e7833843162a2461f55` (both) | same hash (both) |
| PNG sha256 | `8e4e6b19fe82bc8378e35f7d699d30fa1e6e3f06f102e0631464e24cbc609a5f` (both) | same hash (both) |
| PNG size | 7436 bytes, 207×78 | 7436 bytes |
| on-disk PNGs | `cmp` → byte-identical | — |

So: (a) raw RGBA identical across arches, (b) PNG bytes identical across arches, (c) double-run identical on each arch. The float-bearing stages exercised and identical: skrifa outline scaling, zeno coverage rasterization (hinted and unhinted), tiny-skia AA rect/path fills with default `simd` feature on. Visual output inspected (correct JBM glyphs, box glyphs, AA shapes on phosphor palette).

Exact resolved versions (archtest `Cargo.lock`, 38 packages): swash 0.2.10, skrifa 0.44.0, read-fonts 0.41.0, font-types 0.12.3, zeno 0.3.3, yazi 0.2.1, tiny-skia 0.12.0, tiny-skia-path 0.12.0, png 0.18.1, fdeflate 0.3.7, flate2 1.1.9, miniz_oxide 0.8.9, simd-adler32 0.3.10, crc32fast 1.5.0, sha2 0.10.9, bytemuck 1.25.2, arrayref 0.3.9, arrayvec 0.7.8, strict-num 0.1.1 (+ build/derive deps). (confidence: HIGH)

Scope caveats of the measurement (facts, not doubts): Rosetta 2 executes real x86_64 SSE semantics but historically does not expose every AVX2 runtime-dispatch path a physical x86_64 CI runner would take (relevant to `simd-adler32`/`fdeflate` PNG encoding — checksum output is exact integer math regardless of path, and the settled constraint pixel-compares rather than byte-compares PNGs anyway); both binaries ran on one OS (macOS libm/allocator), so macOS↔Linux identity — the actual dev-bless/CI-verify pair per chapter 04 — is not covered by this experiment. (confidence: HIGH that these are the limits of what was measured)

### 5. Rust-side PNG decode/compare for the CI gate

- `png` 0.18.1 (MIT OR Apache-2.0 — inside the current allowlist) decodes with `Decoder::new(reader)` → `read_info()` → `Reader::output_buffer_size()` → `next_frame(&mut buf)`; `Reader::info()` exposes width/height/color type/bit depth; the same crate encodes (`Encoder::new(w, h)`, `set_color`, `set_depth`, `write_header()` → `write_image_data`) — <https://docs.rs/png/0.18.1/png/>. It is already in the archtest graph via tiny-skia's default `png-format` feature, which is exactly `["std", "dep:png"]` (tiny-skia manifest). (confidence: HIGH)
- `image` 0.25.10 (MIT OR Apache-2.0) is the multi-format alternative with pixel-access APIs — <https://docs.rs/image/0.25.10>; strictly more surface than a PNG-only gate needs (fact of scope, not a recommendation). (confidence: HIGH)
- Raw-buffer comparison needs no decode at all when both sides are in-process: `Pixmap::data()` returns the premultiplied RGBA bytes directly (used for the raw hash in §4). (confidence: HIGH, empirical)
- Workspace policy: dependencies are centralized under `[workspace.dependencies]` (`Cargo.toml:19-36`); dev-dependencies are established practice (`crates/termrock/Cargo.toml:35-37` uses `serde_json`/`stats_alloc`; `crates/termrock-cli/Cargo.toml:25`); nothing restricts adding dev-deps, but `deny.toml:10` denies wildcard versions and — per §2 — `cargo deny check licenses` covers dev-dependency graphs, so BSD-3-Clause raster crates need an allowlist edit even as dev-deps. `unicode-width 0.2.2` and `sha2 0.11.0`/`hex` already exist as workspace deps (`Cargo.toml:33,35-36`). (confidence: HIGH)

## Dead ends and contradictions

- The task premise "agg's default = JetBrains Mono (bundled)" is half-right: JBM is first in agg's default font-family *list*, but agg bundles only Symbols Nerd Font and Noto Emoji — JBM comes from the host system (<https://docs.asciinema.org/manual/agg/usage/>). Ghostty, by contrast, genuinely embeds JBM.
- Ghostty's config reference page does not state the font-family default; the embedding fact had to come from the 1.2.0 release notes instead.
- crates.io's per-version API path `…/crates/<name>/<ver>` returned no `version` key via plain curl; feature flags were instead read from the vendored registry manifests (stronger source anyway).
- cargo-deny's licenses page doesn't document dev-dep scope; the `exclude-dev` semantics live on the config page, which states the effect of `true` but not the default explicitly.
- Cross-surface inconsistency found while answering Q3 (recorded, not judged): DIM is darkened twice (0.6 in `frame.rs:184-189`, ×0.7 again in `preview-metrics.ts:167-172`), and the SVG export uses a different font stack and its own ~0.55 dim (`svg.rs:195,309-311`) than the canvas path — three surfaces, three dim values.
- No embedded instructions were encountered in any fetched web content or repo file.

## Open unknowns

- macOS(aarch64) ↔ Linux(x86_64 CI runner) bit-identity of the same stack — this experiment varied only ISA on one OS. Same-toolchain cross-OS is the remaining untested axis (chapter 04 carries the determinism argument; no cross-OS number exists yet).
- Whether physical x86_64 hardware taking AVX2 runtime-dispatch paths in `fdeflate`/`simd-adler32` produces the same PNG *bytes* as the Rosetta run (raw pixel identity is unaffected — those crates only run at encode time; and the settled gate pixel-compares).
- Whether cargo-deny 0.20.2's `exclude-dev` default is definitively `false` (implied by docs phrasing; not stated on the fetched page).
- Whether `reuse lint` is enforced anywhere outside the pinned tool install (no invocation found; a human-run habit can't be excluded).
- swash 0.2.10's hinting engine determinism across font *sizes* other than 14 px was not exercised (one size, hinted + unhinted, both identical).
