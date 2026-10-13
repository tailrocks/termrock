# 01 — libghostty embedding and headless rendering

Questions: What is libghostty today (API surface, language, build, stability, platforms, docs)? Can it render offscreen/headless to pixels or PNG? How does font loading/shaping work and can a font be pinned? Do existing projects use it for screenshots/image rendering? What input does it need (PTY bytes vs direct cell injection)? What is proven vs speculative for a CI PNG pipeline?
Informs: jackin-termrock-parity
Method: both — web (ghostty.org, mitchellh.com, GitHub, docs.rs) + reference clone of https://github.com/ghostty-org/ghostty @ 26df373ec83fb1cebb4fee0a8394144ae984a9b8 (shallow, committed 2026-08-16). All clone citations are file:line at that commit.
Vetted: 2026-08-16

## Findings

### 1. What is libghostty today (2026)

- Two distinct C surfaces exist in the repo, and they are frequently conflated in secondary sources. `include/ghostty.h:1-11` opens: "Ghostty's internal embedder API, a.k.a. 'libghostty-internal'. The only consumer of this API is the macOS app … External embedders should instead use `libghostty-vt`." — https://github.com/ghostty-org/ghostty @ 26df373 (confidence: HIGH)
- The public product is **libghostty-vt**: a zero-dependency C/Zig library for "parsing and handling terminal escape sequences as well as maintaining terminal state such as styles, cursor position, screen, scrollback" — `include/ghostty/vt.h:1-12`, which also carries: "WARNING: This is an incomplete, work-in-progress API. It is not yet stable and is definitely going to change." (confidence: HIGH)
- Language/build: written in Zig; `build.zig.zon:6` requires `minimum_zig_version = "0.16.0"`. Build emits shared lib, static lib (`libghostty-vt.a`), and an Apple universal xcframework — `build.zig:123-181`. Library version string is `0.1.0-dev` (`build.zig:10`); repo app version `1.3.2-dev` (`build.zig.zon:3`). (confidence: HIGH)
- No tagged libghostty-vt release exists as of 2026-08-16: `git ls-remote --tags origin` on the clone shows only app tags ending at `v1.3.1`, none matching vt/lib. Mitchell Hashimoto's announcement (2025-09-22) targeted "a tagged version within the next 6 months" — that target has passed without a tag. — https://mitchellh.com/writing/libghostty-is-coming (confidence: HIGH)
- Platforms: macOS, Linux, Windows (per-OS lib naming at `build.zig:152-155`, `202-209`), plus WebAssembly (`include/ghostty/vt/wasm.h`, `src/main_wasm.zig`). (confidence: HIGH)
- Zig consumers get first-class modules `ghostty-vt` / `ghostty-vt-c` (`src/build/GhosttyZig.zig:28`); C consumers get `include/ghostty/vt/*.h` (29 headers: terminal, render, snapshot, formatter, selection, kitty_graphics, key/mouse encoding, osc, sgr, …). (confidence: HIGH)
- Docs: Doxygen site at https://libghostty.tip.ghostty.org/ ; 26 buildable C examples under `example/c-vt-*` in-repo; ghostty.org's `/docs/vt/reference` is a VT-sequence reference, not API docs. (confidence: HIGH)
- Rust bindings exist: crate `libghostty-vt` 0.2.1 (released 2026-07-18, maintainers Uzaaft/pluiedev), repo https://github.com/Uzaaft/libghostty-rs — safe wrappers `Terminal`, `RenderState`, row/cell iterators, key/mouse encoders. README states it "requires Zig 0.16.x on PATH" and fetches pinned ghostty source in `build.rs`; "vendored builds target Zig's portable baseline CPU". Crate self-describes as unstable. — https://docs.rs/libghostty-vt (confidence: MED — README/docs read, not built)
- Node-API bindings: https://github.com/coder/libghostty-vt-node ("ABI-stable Node-API bindings"). (confidence: MED — repo description only)

### 2. Offscreen/headless rendering to pixels or PNG

- **libghostty-vt produces no pixels.** Its "render" API is render *state*: "Represents the state required to render a visible screen (a viewport) … Read from the render state to get the data needed to draw your frame" — `include/ghostty/vt/render.h:22-40`. Per cell it exposes the full grapheme as UTF-8, resolved fg/bg RGB (palette-flattened), style, selection membership — `render.h:715-790` enum `GhosttyRenderStateRowCellsData` (`…_BG_COLOR`, `…_FG_COLOR`, `…_GRAPHEMES_UTF8`). Exported C symbols confirm state-only surface: `src/lib_vt.zig:281-295`. Drawing is explicitly the caller's job. (confidence: HIGH)
- Ghostty's actual pixel renderers are GPU-only and live outside the library: `src/renderer/backend.zig:5-8` — `pub const Backend = enum { opengl, metal, webgl }`. No software/CPU backend exists; `grep -ri screenshot src/` returns nothing. (confidence: HIGH)
- Collaborator statement in the screenshot-tool discussion (2026-02): "libghostty currently does not do any sort of rendering"; libghostty's goal framed as low-level terminal emulation, with a CPU rendering pipeline mentioned only as a plan; the proposal to add a screenshot tool remained unresolved. — https://github.com/ghostty-org/ghostty/discussions/10597 (confidence: HIGH for "no rendering" quote; MED for the CPU-pipeline plan, which appears only in discussion, no tracking issue found)
- The announced rendering roadmap is GPU-surface-shaped, not headless: future libraries include "GPU rendering (provide us with an OpenGL or Metal surface and we'll take care of the rest)". — https://mitchellh.com/writing/libghostty-is-coming (confidence: HIGH)
- The internal `ghostty.h` embedder API does drive a real renderer (Metal), but it is macOS-app glue requiring a view/surface — `include/ghostty.h:1-11`, `build.zig:190-209` ("This is NOT libghostty … glue between Ghostty GUI on macOS and the full Ghostty GUI core", installed as `ghostty-internal.*`). Not a headless path. (confidence: HIGH)
- What the library *does* export from state, headlessly: the formatter — "Format terminal content as plain text, VT sequences, or HTML" — `include/ghostty/vt/formatter.h:22-30`, `ghostty_formatter_format_buf/alloc` at `formatter.h:161,184`. The app exposes the same as `write_screen_file:…,html` and `copy_to_clipboard:html` (`src/input/Binding.zig:1126,4605`). So HTML-out is first-class; PNG-out does not exist anywhere. (confidence: HIGH)
- Also headless: full binary state snapshots ("GHOSTSNP" CRC-protected record stream) via `ghostty_snapshot_encode/decode` — `include/ghostty/vt/snapshot.h:22-60,253-489` — state serialization, not an image. (confidence: HIGH)
- Notable internal-only fact: ghostty contains a pure-CPU OpenType `glyf` outline rasterizer built on the z2d Zig 2D library (`src/font/glyf_rasterize.zig:1-16`) with a PNG-comparison test (`src/font/glyf_rasterize_png_test.zig`). It feeds the app's glyph atlas; it is not exported by libghostty-vt (`src/lib_vt.zig` exports no font symbols). CPU glyph rasterization exists in the codebase but is not embeddable API. (confidence: HIGH)

### 3. Font loading/shaping and pinning

- The font system is app-side, not library-side. Backends: `freetype`, `freetype_windows`, `fontconfig_freetype` (Linux default), and CoreText variants (macOS default) — `src/font/backend.zig:3-60` (`default()` returns `.coretext` on Darwin, `.fontconfig_freetype` otherwise). (confidence: HIGH)
- Ghostty bundles fonts as embedded resources — JetBrains Mono variable/static, Nerd Font symbols, Noto emoji — `src/font/embedded.zig:9-23`. This is the app's default face, reproducible for the *app*, but none of it is reachable through libghostty-vt. (confidence: HIGH)
- Consequence for reproducibility: libghostty-vt has **no font API at all** (no font/shaper exports in `src/lib_vt.zig`). Any consumer building images must bring its own font stack, so font pinning is trivially possible and entirely the consumer's responsibility — exactly as the official demo does: ghostling implements "font rendering and glyph rasterization" itself on Raylib; libghostty supplies parsing/state/render-state only. Ghostling build requires "CMake 3.19+, Ninja, a C compiler, Zig 0.16.x on PATH". — https://github.com/ghostty-org/ghostling (confidence: HIGH for lib_vt exports; MED for ghostling README details)

### 4. Existing projects rendering images / headless use

- Curated ecosystem list: https://github.com/Uzaaft/awesome-libghostty. Headless-emulation projects exist and are healthy: `termscope` ("headless terminal emulator CLI powered by libghostty-vt", self-described "Playwright for the terminal"), `headless-terminal` (Go, "Puppeteer for TUIs … backed by libghostty-vt", https://github.com/montanaflynn/headless-terminal), `vterm-mcp` (drives/tests TUIs via libghostty-vt PTY), `term2html` (ANSI→HTML), `evp` (terminal recorder, Rust), browser/WASM efforts (`browstty`, `ghostty-web`, `vscode-bootty`), BEAM NIFs (`ghostty_ex`). (confidence: MED — list descriptions and search snippets; individual repos not read)
- **No project was found that emits PNG/pixels from libghostty.** All located consumers either stay in text/cell space (termscope, headless-terminal, term2html) or bolt on their own graphics stack (ghostling→Raylib, libghostty-rs's ghostling port→macroquad, browser efforts→WebGL/canvas). (confidence: MED — absence-of-evidence across the awesome list, GitHub topic search, and web search)
- First shipped commercial-grade consumer: cmux, a macOS terminal "built on Ghostty's libghostty rendering engine" — i.e. the internal `ghostty.h`/Metal path, GUI-only, macOS-only, not headless. — https://github.com/manaflow-ai/cmux, https://cmux.com/ (confidence: MED — repo description plus news coverage)
- Ghostty's own image-comparison testing exists only for the internal font rasterizer (`src/font/glyf_rasterize_png_test.zig`), not for terminal frames. (confidence: HIGH)
- The in-repo screenshot discussion (#10597) surfaced HTML export as the maintained alternative and questioned image-based testing reliability across environments; no screenshot facility was committed. (confidence: HIGH)

### 5. Input model: PTY bytes vs direct cell injection

- Input is a raw VT/ANSI byte stream into the terminal object: `ghostty_terminal_vt_write` (`include/ghostty/vt/terminal.h:1749`), `ghostty_terminal_vt_write_until_ground` (`terminal.h:1779`), plus continuation APIs for split sequences (`terminal.h:1810-1870`). Flow: create terminal → write bytes → update render state → iterate rows/cells (demonstrated in `example/c-vt-render/`). (confidence: HIGH)
- **No PTY in the library**: `src/lib_vt.zig` exports no pty symbols; `src/pty.zig` is app-side. Consumers own process/PTY plumbing (termscope/headless-terminal each wrap their own PTY around it) or feed captured bytes directly — no PTY is required for deterministic input. (confidence: HIGH)
- **No direct cell-grid injection API.** The only structured-state input is snapshot restore, whose "GHOSTSNP" binary format is documented (`snapshot.h:52-60`) but designed for encode→decode round-trips of an existing terminal, not authoring. A Ratatui-owned buffer would have to be serialized to ANSI and re-parsed to become ghostty cells — the library re-derives the cell grid a Ratatui `Buffer` already contains. (confidence: HIGH)

### 6. Proven vs speculative for a CI PNG pipeline

Proven (primary sources, working code in the wild):
- Headless, CPU-only, GPU-free, cross-platform terminal **emulation**: ANSI bytes in → cell grid with resolved RGB fg/bg, graphemes, styles out (`render.h`; termscope/headless-terminal as existence proofs). (confidence: HIGH)
- Headless **HTML/text/VT export** of a frame from terminal state (`formatter.h:22-30`). (confidence: HIGH)
- Rust consumption via the `libghostty-vt` crate — with a Zig 0.16.x toolchain requirement in CI at build time. (confidence: MED)

Absent or speculative:
- **Any pixel/PNG output from libghostty — absent.** No software renderer backend exists (`src/renderer/backend.zig:5-8`); the CPU rendering pipeline is a discussion-level plan with no located tracking issue; the announced GPU rendering library is future work and requires a caller-provided OpenGL/Metal surface. (confidence: HIGH for absence at commit 26df373; MED for plans)
- A libghostty-based PNG pipeline therefore requires consumer-built rasterization (pinned font + glyph rasterizer + cell compositor) or an HTML→browser-render step — both entirely outside libghostty. (confidence: HIGH, follows from the above)
- API stability: every layer carries explicit instability warnings (`vt.h:10-12`, `lib_vt.zig:1-9`, Rust crate 0.x "breaking changes are expected"); no tagged library release exists. (confidence: HIGH)

## Dead ends and contradictions

- `https://ghostty.org/docs/libghostty` → HTTP 404. `ghostty.org/docs/vt/reference` is a VT escape-sequence reference, not API documentation; the API docs are the Doxygen site https://libghostty.tip.ghostty.org/.
- The Mintlify page "libghostty C API Overview" (ghostty-org-ghostty.mintlify.app/api/overview) documents `ghostty_app_t`/`ghostty_surface_t` with Metal/OpenGL rendering as "libghostty" — contradicted by the repo, where that header is now labeled internal, macOS-app-only (`include/ghostty.h:1-11`; `build.zig:193-195` "This is NOT libghostty"). Treated as stale.
- News claims that cmux gets "GPU-accelerated rendering from libghostty" describe the internal embedder path, not the public libghostty-vt — resolved via the header/build comments above; not evidence of an embeddable renderer.
- Kitty graphics PNG support (`include/ghostty/vt/kitty_graphics.h`, `example/c-vt-kitty-graphics`) is inbound image *display* state, not outbound frame rendering — ruled out as a rendering path.
- `grep -ri screenshot src/` on the clone: zero hits — no hidden screenshot facility.
- The internal CPU glyph rasterizer (`src/font/glyf_rasterize.zig`) looked like a possible pixel path but is not exported by any library target — ruled out as embeddable API today.

## Open unknowns

- Whether the "CPU rendering pipeline" plan has a tracking issue, owner, or timeline — only the #10597 discussion mention was found.
- Whether the Rust `libghostty-vt` crate can build in CI without a Zig toolchain (its "vendored" prebuilt story) — README read, not exercised by building.
- `evp`'s actual output format (frames? GIF? video?) — repo not located/read; only the awesome-list description.
- Whether ghostty's internal glyf rasterizer plus shaper (`src/font/shape.zig`, `src/font/shaper/`) will ever be packaged as a libghostty font/rendering library — announced only as vague future "input handling, GPU rendering, GTK/Swift widgets".
- Coverage/fidelity of the HTML formatter output (does it capture every style ghostty renders — underline styles, curly underlines, minimum-contrast adjustments?) — header read, implementation not traced.
