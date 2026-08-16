# TUI PNG baselines — vetted summary

Topic: storing TUI widget designs as PNG baselines in git, rendering them "as
real as possible" (libghostty was the working hypothesis), and gating PRs on
image regeneration. Informs: [`roadmap/jackin-termrock-parity`](../../roadmap/jackin-termrock-parity/README.md).

Chapters vetted 2026-08-16. All conclusions below link their evidence.

## Headline conclusions

1. **libghostty cannot render PNGs today — the premise fails.** The only
   public artifact is libghostty-vt: terminal *state* (ANSI bytes → cell grid
   with resolved RGB), explicitly unstable, no tagged release, no font API,
   no pixel output of any kind; Ghostty's renderers are GPU-only (opengl,
   metal, webgl) and app-internal; a collaborator states "libghostty
   currently does not do any sort of rendering". Any libghostty-based PNG
   pipeline means building the entire rasterizer (font + glyphs + compositor)
   yourself — and feeding it ANSI bytes to re-derive a grid Ratatui already
   owns in-process. → [01](01-libghostty-embedding.md)
2. **Nobody in the Rust TUI world does pixel regression.** ratatui, zellij,
   helix, gitui, yazi all use text-buffer snapshots (insta) or none. The most
   advanced shipping visual gate is Textual's normalized-SVG string compare
   with an HTML diff report and `--snapshot-update` bless. The proven
   pure-Rust raster pipeline is asciinema agg's: avt grid → fontdb-pinned
   fonts → swash shaping → tiny-skia raster. No reusable grid→PNG crate
   exists; agg proves it composes from published crates. → [02](02-ecosystem-approaches.md)
3. **Byte-determinism requires escaping the OS text stack.** FreeType version
   drift and CoreText-vs-FreeType make any OS-stack pipeline machine-variable
   (matplotlib pins a FreeType build; Playwright keys baselines per-OS and
   disclaims cross-machine identity). A pinned font file + pinned pure-Rust
   crates (swash/tiny-skia class) has zero OS inputs — plausibly bit-identical
   everywhere, but undocumented; needs one empirical two-platform test.
   → [04](04-determinism-ci-storage.md)
4. **Compare decoded pixels, not PNG bytes.** The `png` crate's changelog
   proves encoder bytes churn across versions even when pixels don't; every
   surveyed snapshot tool (Playwright/pixelmatch, odiff, jest-image-snapshot,
   pytest-mpl) compares decoded pixels. Zero-tolerance pixel equality keeps
   the gate exact without freezing the encoder. → [04](04-determinism-ci-storage.md)
5. **Plain git, not LFS.** GitHub shows only the pointer file for LFS objects
   in PR diffs — incompatible with the settled reviewer-sees-image-diff
   requirement. Plain-git PNGs get 2-up/swipe/onion-skin rich diffs; the full
   baseline set is ~0.5–15 MB against a 118 MiB pack — trivial. This resolves
   the storage research question outright. → [04](04-determinism-ci-storage.md)
6. **The termrock seams are ready; the old rev is usable but limited.**
   `frame.rs` exports truecolor cell JSON for any of 1066 stories (gaps:
   drops italic/strikethrough, pre-resolves reversed/dim into RGB, no cursor
   or wide-char field, phosphor-only CLI). The old rev `5ff94ee` builds clean
   today, has 45 stories — all still existing at HEAD, including all 25
   jackin-subset ones — and a deterministic SVG exporter, but no frame JSON
   and color-only SVGs (all modifiers absent). Full-fidelity old-rev capture
   needs a small side harness against its public widget constructors, or a
   patch. No Buffer→ANSI writer exists anywhere; the unresolved `Buffer`
   before encode is the seam for one. → [03](03-termrock-seams-and-old-rev.md)
7. **CI shape:** PR lane is Velnor self-hosted via generated, DO-NOT-EDIT
   workflows delegating to SHA-pinned `tailrocks/velnor-actions` — a PNG job
   lands there (or as a sibling standalone workflow like docs.yml), not in
   ci.yml. A pure-Rust rasterizer needs no GPU/Xvfb on any lane; an
   emulator-capture approach would need GL/Xvfb on Linux and can never
   produce macOS-native (Metal/CoreText) pixels in CI. The repo already runs
   a render-twice-diff determinism gate for SVGs. → [04](04-determinism-ci-storage.md), [03](03-termrock-seams-and-old-rev.md)

## Plan-pass additions (2026-08-16, chapters 05–06)

8. **The PNG gate can be a workspace test — no new workflow.** The committed
   text goldens already run on every PR because `mise run ci`/`test` execute
   workspace nextest, and the pinned velnor-actions `ci-code.yml` runs
   exactly those mise tasks (its only extension point is the repo's own mise
   task bodies; no containers; PRs always on `ubuntu-26.04`). A
   goldens-style PNG test with a bless env var inherits PR gating for free.
   docs.yml/hygiene.yml are hand-written precedents if a standalone workflow
   is ever needed. All PR runners are Linux; no macOS lane exists.
   → [05](05-ci-placement-and-commands.md)
9. **Cross-arch bit-identity: measured, holds.** swash 0.2.10 + tiny-skia
   0.12.0 + vendored JetBrains Mono 2.304 produced identical raw-RGBA and
   PNG sha256 on aarch64 native vs x86_64-under-Rosetta, double-runs
   identical (hashes + method in chapter; reproduced by the orchestrator).
   Minimal crate set is swash + tiny-skia (38-crate graph, no fontdb, no
   shaper — `charmap.map` per glyph suffices). Remaining untested axis:
   macOS↔Linux same-arch (carried as assumption A3 in the plan ledger).
   → [06](06-rasterizer-facts-and-archtest.md)
10. **Repo-policy facts the plans must handle:** deny.toml's license
    allowlist blocks tiny-skia (BSD-3-Clause) and arrayref (BSD-2-Clause)
    even as dev-deps; REUSE.toml's `**`=Apache-2.0 annotation would
    misdeclare a vendored OFL-1.1 font; JBM v2.304 is OFL-1.1 (sha256
    recorded). Existing seam constants to match: 9×18 px cells, 14 px font,
    baseline 14. Known cross-surface defect on record: dim is darkened
    twice on the web path (0.6 Rust × 0.7 canvas) and differently in SVG.
    → [06](06-rasterizer-facts-and-archtest.md)

## Candidate directions (no verdict — user chooses)

- **A. Pure-Rust in-process rasterizer** (agg architecture: ratatui `Buffer`
  or frame JSON → vendored font + swash shaping + tiny-skia → PNG).
  Determinism: strongest available (no OS stack, no browser, no GPU). CI:
  cargo-only, both lanes, macOS bless matches Linux CI by construction
  (pending the empirical cross-arch test). Realism: real shaping and
  rasterization of the true grid, but not a real terminal emulator's
  pipeline. Cost: build a small renderer (~the part agg implements in-app);
  fix frame fidelity gaps (italic/strikethrough, wide chars) at the seam.
- **B. libghostty-vt as emulation backend + self-built rasterizer.** Buys
  ghostty's VT semantics — which termrock does not need (it owns the cell
  grid already); still requires the entire rendering stack of A, plus a
  Buffer→ANSI writer, a Zig 0.16 toolchain in CI, and a pre-stable API.
  Strictly more cost than A for no pixel-realism gain today. Revisit if
  ghostty ships its planned CPU rendering pipeline as embeddable API.
- **C. Browser pipeline** (existing `TerminalPreview` canvas/wasm host +
  Playwright `toHaveScreenshot`). Battle-tested bless workflow; but the
  vendor explicitly disclaims cross-machine pixel identity — demands one
  pinned render environment (container), which collides with macOS bless +
  Velnor self-hosted reality; xterm.js itself does not pixel-baseline its
  own renderer.
- **D. SVG-first with optional PNG rasterization** (extend the current
  deterministic SVG exporter; Textual-style normalized compare; resvg for
  PNG when an image is wanted). Cheapest, already half-built, byte-identical
  today; weakest realism (glyph rasterization deferred to the viewer), and
  resvg cross-platform byte-identity is unverified (the linchpin unknown).

## Ruled out

- **libghostty as the PNG renderer** — no rendering exists in the library
  (ch. 01); the roadmap's "render using libghostty" hypothesis cannot be
  implemented today in any form short of writing the renderer yourself.
- **git-LFS for baselines** — pointer-only PR display defeats the settled
  review requirement (ch. 04).
- **PNG byte-compare as the CI predicate** — encoder-version churn produces
  false baselines rewrites; pixel-compare at zero tolerance is the exact
  equivalent without that failure mode (ch. 04).
- **Playwright-style per-platform baseline sets** — incompatible with a
  single-source-of-truth bless-required gate (ch. 04).

## Open unknowns (disposition)

- Cross-arch bit-identity of the pure-Rust stack (swash/tiny-skia class) —
  **empirical test at plan time**: render on x86_64 + aarch64, compare.
- resvg SVG→PNG byte-identity across platforms — **only matters if direction
  D is chosen**; same empirical test shape.
- `png` crate byte-determinism at a fixed version — **mitigated by
  pixel-compare**; assumption recorded, not blocking.
- `tailrocks/velnor-actions` ci-code.yml extensibility (container support,
  fleet OS/arch) — **plan-time read of that repo**.
- GitHub's size cutoff for rich image diffs — **irrelevant at few-KB files**;
  scoped out.
- Old-rev capture fidelity (SVG color-only vs side harness) — **decision
  routed to the item's Open questions**.
