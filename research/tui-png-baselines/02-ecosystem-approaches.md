# 02 — Ecosystem approaches to TUI visual snapshots

Questions: How do well-regarded projects produce terminal/TUI visual snapshots for git-stored baselines and CI regression? Covered: (1) vhs, (2) freeze, (3) asciinema agg, (4) Rust terminal-emulation/rasterization crates, (5) web-emulator + Playwright path, (6) how major TUI projects test visuals in CI, (7) realism vs determinism vs CI cost per approach.
Informs: jackin-termrock-parity
Method: web (WebFetch of primary repos/manifests/docs; WebSearch for discovery; GitHub code-search via `gh api search/code` for usage counts). No reference clones needed — all claims verified against fetched primary files.
Vetted: 2026-08-16

## Findings

### 1. Charmbracelet vhs — rendering pipeline, outputs, determinism, CI

- vhs requires `ttyd` and `ffmpeg` on PATH; ttyd hosts the terminal session, ffmpeg encodes video — https://github.com/charmbracelet/vhs README (confidence: HIGH)
- vhs drives a headless Chrome browser via `github.com/go-rod/rod v0.116.2` (Chrome DevTools Protocol automation) — https://raw.githubusercontent.com/charmbracelet/vhs/main/go.mod (confidence: HIGH)
- ttyd's browser frontend is xterm.js (WebGL2 renderer), so vhs's visual output is xterm.js-rendered-in-Chromium, screenshotted frame by frame — https://github.com/tsl0922/ttyd README (confidence: HIGH)
- Output formats: GIF, MP4, WebM, a directory of PNG frames (`Output frames/`), and `.txt`/`.ascii` "golden files" the README explicitly positions for integration testing; a dedicated `Screenshot` command "captures the current frame (png format)" — https://github.com/charmbracelet/vhs README (confidence: HIGH)
- Font control is script-level: `FontSize`, `FontFamily`, `LetterSpacing`, `LineHeight` tape settings — https://github.com/charmbracelet/vhs README (confidence: HIGH)
- Official CI story: `charmbracelet/vhs-action` installs vhs (pinned version), installs fonts (JetBrains Mono by default, ~10 more optional), runs tapes, and supports two workflows: auto-commit regenerated assets, or upload + comment the generated image on the PR — https://github.com/charmbracelet/vhs-action (confidence: HIGH). The PR-comment/auto-commit pattern is an existing precedent for the "regenerate in CI, reviewer sees the image" half of a bless-required gate.
- No primary-source determinism guarantee found. "vhs is deterministic" appears only in third-party blogs. Counter-evidence in the tracker: rendering of a Bubble Tea app differs/breaks under vhs vs local terminal (https://github.com/charmbracelet/vhs/issues/362); inconsistent playback timing / dropped frames (https://github.com/charmbracelet/vhs/issues/367) (confidence: MED — issues read via search summaries, not full threads)

### 2. Charmbracelet freeze — ANSI→SVG/PNG, fonts, fidelity

- freeze generates images of code and of terminal output (`--execute "cmd"` captures ANSI output); output formats `.svg`, `.png`, `.webp`; window chrome, shadows, padding are template features — https://github.com/charmbracelet/freeze README (confidence: HIGH)
- PNG is produced by rasterizing the generated SVG through `github.com/kanrichan/resvg-go v0.0.2-0.20231001163256` (a Go/wasm binding of resvg), not a native Go raster pipeline — https://raw.githubusercontent.com/charmbracelet/freeze/main/go.mod (confidence: HIGH)
- Font handling: default "JetBrains Mono" 14px / 1.2em line height; custom fonts via `--font.file` (TTF/WOFF/WOFF2) embedded directly into the SVG; ligatures behind `--font.ligatures` — https://github.com/charmbracelet/freeze README (confidence: HIGH)
- Fidelity boundary: freeze is a styled-text imager, not a terminal emulator — the README documents ANSI capture, but no screen model (cursor addressing, alt-screen, redraw). Internals of `--execute` (pty vs pipe) are undocumented — README + repo listing only (confidence: MED; emulation depth is an open unknown below)

### 3. asciinema agg / ecosystem — cast→image, font rasterization, determinism

- agg converts asciicast recordings to GIF only; no PNG still output documented anywhere in README or manual — https://github.com/asciinema/agg, https://docs.asciinema.org/manual/agg/ (confidence: HIGH)
- Current stack (Cargo.toml): `avt 0.18` (terminal emulation), `fontdb` (font discovery), two rendering backends — `swash 0.2.7` (default; glyph shaping/scaling) and `resvg 0.45` — plus `tiny-skia` raster and `gifski` GIF encoding. `fontdue` is NOT present in the current dependency set — https://raw.githubusercontent.com/asciinema/agg/main/Cargo.toml (confidence: HIGH)
- avt is asciinema's Rust virtual terminal: ANSI parser "based on excellent state diagram by Paul Williams", primary/alternate screen buffers as a character grid with attributes, query API for screen + cursor; explicitly excludes input handling and rendering; used by asciinema CLI, player, server, and agg — https://github.com/asciinema/avt (confidence: HIGH)
- Fonts come from the system via fontdb, with "sensible cross-platform defaults and implicit fallbacks", `--font-dir` for out-of-tree fonts, Nerd Font and color-emoji support — https://docs.asciinema.org/manual/agg/ (confidence: HIGH)
- Determinism inference: because agg rasterizes in-process (no browser, no GPU) its variance surface is font resolution; identical output across machines requires pinning fonts via `--font-dir` + explicit family, since default discovery is host-dependent. Method: derived from fontdb system-discovery + the manual's fallback wording; no primary determinism statement exists (confidence: MED)

### 4. Rust crates: terminal emulation and ANSI/grid→image

Emulation (ANSI bytes → queryable cell grid), all render-free:

- `vt100` (doy): "parses a terminal byte stream and provides an in-memory representation of the rendered contents"; per-cell content + fg/bg color query; v0.16.2 released 2026-06-04 (actively maintained; method: docs.rs/crates.io version page) — https://docs.rs/vt100/latest/vt100/ (confidence: HIGH)
- `alacritty_terminal`: Alacritty's backend as a library — `Term` state machine, optimized `Grid`, vte-based parsing, pty event loop; rendering explicitly left to the consumer — https://docs.rs/alacritty_terminal/latest/alacritty_terminal/ (confidence: HIGH)
- `termwiz` (wezterm project): escape-sequence parser with semantic model + `Surface` cell-grid abstraction (`surface`, `escape`, `cell` modules) — https://docs.rs/termwiz/latest/termwiz/ (confidence: HIGH)
- `avt` (asciinema): see §3 — https://github.com/asciinema/avt (confidence: HIGH)

ANSI/grid → image tools built on those crates:

- `termframe` (pamburus): "non-interactive terminal emulator that executes a single command, renders its output in an internal virtual session, and exports a screenshot as an SVG file"; SVG only. Cargo.toml shows the assembly: `termwiz 0.23` (emulation) + `portable-pty` (session) + `allsorts` (font parsing) + askama-templated `svg` output + `rust-embed`; optional font embedding — https://github.com/pamburus/termframe, https://raw.githubusercontent.com/pamburus/termframe/main/Cargo.toml (confidence: HIGH)
- `termsnap` (tomcur): terminal output → SVG "using an in-memory instance of Alacritty under the hood" (alacritty_terminal) for control-sequence compatibility — https://github.com/tomcur/termsnap (confidence: MED — README summary via search results, repo not fetched directly)
- `term-transcript` (slowli): captures command output and renders SVG via Handlebars templates; includes a `test` module (`TestConfig`) that parses transcripts back from SVG and asserts against re-execution — purpose-built CLI snapshot testing. Explicit limitations: default capture is OS pipes ("the terminal is not emulated... programs dependent on isatty checks or getting term size can produce different output"), optional `portable-pty` feature, and "most escape sequences are dropped" — https://github.com/slowli/term-transcript, https://docs.rs/term-transcript/latest/term_transcript/ (confidence: HIGH)
- `termshot` (homeport, Go not Rust but same niche): ANSI command output → PNG directly, carbon.now.sh-style window chrome — https://github.com/homeport/termshot (confidence: HIGH)
- No published Rust crate was found that takes a cell grid to PNG as a reusable library. The closest reference implementation is agg's internal pipeline (avt grid → swash glyphs → tiny-skia raster → frames), which is application code, not an exported API. Method: crates.io/lib.rs searches + the surveys above (confidence: MED — absence claim)

### 5. Web-emulator path: xterm.js / browser + Playwright screenshots

- vhs itself is the flagship of this path (ttyd/xterm.js page + go-rod Chrome screenshots; §1) (confidence: HIGH)
- xterm.js's own test suite runs Playwright integration tests in real browsers against DOM/canvas renderers, but a code search for `toHaveScreenshot` in `xtermjs/xterm.js` returns 0 hits — their assertions are buffer/JS-state, not pixel baselines. Method: `gh api search/code q="repo:xtermjs/xterm.js toHaveScreenshot"` → total_count 0; wiki https://github.com/xtermjs/xterm.js/wiki/Contributing (confidence: MED — GitHub code search can under-index)
- `@microsoft/tui-test`: end-to-end TUI test framework using `@xterm/headless` + node-pty; `toMatchSnapshot()` for terminal snapshots plus `toHaveFgColor()`/`toHaveBgColor()` — buffer/text-level, no pixel screenshots — https://github.com/microsoft/tui-test (confidence: MED — README via search summary)
- `termless` (beorn): "Like Playwright, but for terminal apps" — one `TerminalBackend` interface across xterm.js, Ghostty, Alacritty, WezTerm, vt100 backends; SVG & PNG screenshots (PNG via optional `@resvg/resvg-js`), optional Playwright/Chromium rendering "for browser-shaped text"; explicitly test/dev infrastructure. Young: 32 stars, 610 commits (method: README/GitHub sidebar at fetch time) — https://github.com/beorn/termless (confidence: HIGH for claims, MED for maturity as adoption signal)
- Playwright `toHaveScreenshot` baseline workflow: first run generates the baseline by screenshotting "until two consecutive screenshots matched"; baselines live in `<testfile>-snapshots/`; bless with `--update-snapshots`; tolerance via `maxDiffPixels`/threshold; volatile elements maskable via custom CSS (`stylePath`) — https://playwright.dev/docs/test-snapshots (confidence: HIGH)
- Playwright's own determinism position: snapshot filenames embed browser+OS (`...-chromium-darwin.png`) because "browser rendering can vary based on the host OS, version, settings, hardware, power source (battery vs. power adapter), headless mode, and other factors"; official guidance is "run tests in the same environment where the baseline screenshots were generated" — https://playwright.dev/docs/test-snapshots (confidence: HIGH). Cross-machine pixel identity of browser canvas output is explicitly NOT promised by the vendor.

### 6. How major TUI projects test visuals in CI

- ratatui (docs + own repo): recommended pattern is `TestBackend` + insta text snapshots — each snapshot line is a terminal row; the docs state "Asserting with color is not supported as of now". No image-based regression — https://ratatui.rs/recipes/testing/snapshots/ (confidence: HIGH); `insta` appears in 156 files of ratatui/ratatui incl. CONTRIBUTING.md and widget sources (method: `gh api search/code q="repo:ratatui/ratatui insta"`) (confidence: MED — token search)
- zellij: e2e tests run the binary in a Docker container, connect via ssh, send commands, and compare output "against predefined snapshots" — text snapshots of terminal state, not images — https://raw.githubusercontent.com/zellij-org/zellij/main/CONTRIBUTING.md (confidence: HIGH)
- helix: integration tests drive `Application` with a mock event loop (`helix-term/tests/`); rendering during integration tests was only enabled by PR #5819 to exercise UI code paths; `assert_snapshot` count in the repo is 0 — no snapshot or image regression — https://github.com/helix-editor/helix/blob/master/docs/CONTRIBUTING.md, https://github.com/helix-editor/helix/pull/5819; method: `gh api search/code` → 0 (confidence: MED)
- gitui: `insta = { version = "1.41.0", features = ["filters"] }` as dev-dependency (raw master Cargo.toml line 71) — text snapshot tests — https://raw.githubusercontent.com/gitui-org/gitui/master/Cargo.toml (confidence: HIGH)
- yazi: no insta in the workspace root manifest; `assert_snapshot` code search count 0 — no snapshot-based visual testing found (method: raw Cargo.toml grep + `gh api search/code`) (confidence: MED — per-crate manifests not exhaustively checked)
- Textual (Python; the largest TUI framework doing real visual regression): `pytest-textual-snapshot` saves an SVG screenshot of the running app and compares on the next run; implementation is syrupy `SVGImageExtension(SingleFileSnapshotExtension)` doing normalized SVG string equality (`normalize_svg()` strips the unique IDs rich's `export_svg()` generates); bless via `--snapshot-update`; failures produce a Jinja2-templated HTML report with visual diffs — https://github.com/Textualize/pytest-textual-snapshot, https://raw.githubusercontent.com/Textualize/pytest-textual-snapshot/main/pytest_textual_snapshot.py (confidence: HIGH)
- Net survey result: no major Rust TUI project was found doing pixel/PNG visual regression in CI; the ecosystem norm is text-buffer snapshots (insta) and the most advanced shipping visual gate is Textual's normalized-SVG-string compare with an HTML review report. Method: the six project checks above (confidence: MED — absence claim over a finite survey)
- libghostty status (bounds the roadmap-preferred path, recorded as fact not verdict): ghostty.org describes libghostty as the internal "cross-platform, C-ABI compatible library" covering "terminal emulation, font handling, and rendering", but "As of the initial public release, libghostty is not yet a stable API and has not been released as a standalone, stable library" — https://ghostty.org/docs/about (confidence: HIGH). The first extracted piece, libghostty-vt, is parsing + terminal state only ("cursor position, current styles, text wrapping"), with GPU rendering, input handling, and widgets named as future components; C API "isn't ready yet" at announcement — https://mitchellh.com/writing/libghostty-is-coming, https://github.com/ghostty-org/ghostty/pull/8840 (confidence: HIGH for announcement-time facts; current C-API maturity is an open unknown below)

### 7. Per-approach: realism vs determinism vs CI cost

Each row is an assessment derived strictly from the cited findings above; method: synthesis, no new sources.

- Browser pipeline, recorder-style (vhs: ttyd/xterm.js + headless Chrome + ffmpeg) — Realism: real browser glyph rasterization of xterm.js's WebGL/canvas renderer; NOT a native terminal's pipeline, and rendering divergence from local terminals is documented (issue #362). Determinism: weakest — Chrome version, fonts, and host factors all enter; no vendor guarantee. CI cost: highest dependency stack (ttyd + ffmpeg + Chrome + font install via vhs-action). Sources: §1 (confidence: HIGH on inputs, MED on the comparative ranking)
- Browser pipeline, test-native (xterm.js page + Playwright `toHaveScreenshot`) — Realism: same rendering class as vhs. Determinism: vendor-managed rather than solved — platform-suffixed baselines, same-environment rule, `maxDiffPixels` tolerance; the bless workflow (`--update-snapshots`, baselines in git) is exactly the "bless-required" shape and is battle-tested. CI cost: browser download + page harness; per-screenshot fast. Sources: §5 (confidence: HIGH)
- In-process Rust rasterization (agg's architecture: avt/termwiz/alacritty_terminal grid + fontdb-pinned fonts + swash shaping + tiny-skia → PNG) — Realism: real font shaping and rasterization of the true cell grid, but no terminal-emulator-specific renderer quirks (that is both its fidelity ceiling and its variance shield). Determinism: strongest available — no browser, no GPU, fonts pinnable via vendored `--font-dir`-style loading. CI cost: lowest — pure cargo, no external binaries. Gap: no off-the-shelf grid→PNG crate; agg is the proof it composes from published crates (avt+swash+tiny-skia all on crates.io). Sources: §3, §4 (confidence: MED — determinism strength inferred, not benchmarked)
- SVG-string snapshot with optional PNG pass (Textual, termframe, term-transcript, termsnap; freeze/termless show the SVG→PNG rasterizer add-on via resvg bindings) — Realism: lowest for testing purposes — glyph rasterization is deferred to the viewer, and term-transcript documents dropped escape sequences; termframe/termsnap fix the emulation half (termwiz/alacritty_terminal) but stay vector. Determinism: highest (normalized text equality, Textual-proven at scale, with an HTML review report). CI cost: minimal. PNG baselines require the extra resvg rasterization step whose cross-platform bit-identity is unverified. Sources: §2, §4, §6 (confidence: HIGH on mechanics, MED on resvg identity)
- Native terminal library (libghostty path) — Realism ceiling: eventually the highest (an actual production terminal's emulation + planned GPU renderer). Today: standalone surface is VT state only, no rendering, API pre-stable — using it now means writing the rasterizer yourself on top of its grid, i.e. it currently competes with vt100/termwiz/alacritty_terminal/avt as an emulation backend, not as a snapshot renderer. Sources: §6 libghostty finding (confidence: HIGH for status)

## Dead ends and contradictions

- agg + fontdue: the question's premise is stale — current agg (Cargo.toml at main) uses swash/resvg + fontdb; fontdue absent.
- `emux-render` (lib.rs hit): a damage-tracking TUI screen renderer for the emux multiplexer (crossterm-based), not an image rasterizer. Ruled out.
- `G0rocks/termframe`: name collision — draws text frames in the terminal; the screenshot tool is `pamburus/termframe`.
- xterm.js repo pixel testing: despite running Playwright in real browsers, zero `toHaveScreenshot` usage found — even the terminal-emulator-in-browser project does not pixel-baseline its own renderer output (code-search caveat noted).
- ghostty.org/docs/about says libghostty covers "rendering capabilities" while the announced standalone artifact (libghostty-vt) explicitly excludes rendering — resolved: rendering exists inside the shipped Ghostty apps' internal lib; the extractable/standalone piece is VT-only.
- "vhs is deterministic" — found only in third-party blogs (e.g. brightcoding.dev), never in charmbracelet primary sources; tracker issues #362/#367 cut against it. Treated as unsubstantiated.
- asciinema ecosystem cast→PNG still: no official tool; agg is GIF-only. (Third-party `svg-term-cli`/`termtosvg` are cast→SVG-animation, JS/Python, unmaintained-looking — leads only, not evaluated.)
- Playwright docs do not actually recommend Docker by name (a common community claim); the primary text says "same environment", nothing more specific.

## Open unknowns

- freeze `--execute` capture mechanics (pty vs pipe) and its real emulation depth — internals undocumented; needs a source read of the freeze repo.
- Whether resvg-based SVG→PNG rasterization with fully embedded fonts is byte-identical across OS/arch — no primary statement anywhere; only an empirical two-platform test settles it. (This is the linchpin for any SVG-intermediate route to git-stable PNG baselines.)
- Cross-run byte-stability of agg/tiny-skia/swash raster output on one machine (and gifski's) — asserted nowhere; needs measurement.
- xterm.js ligature/shaping fidelity inside the vhs pipeline (ligatures addon status and whether ttyd's build enables it) — unverified.
- Current (2026-08) libghostty-vt C API maturity beyond "not yet a stable standalone library" on ghostty.org — release notes after the Sep 2025 announcement not surveyed.
- termless backend mechanics for Ghostty/Alacritty/WezTerm (linking their libraries vs driving binaries) and whether its PNG output is baseline-stable — repo internals not read.
- GitHub code-search zero-counts (yazi, helix, xterm.js) are indexing-dependent; per-crate manifest sweeps would firm these to HIGH.
- Whether any Rust project uses Playwright screenshot baselines for a TUI specifically — none surfaced in searches, but the negative is weakly held.
