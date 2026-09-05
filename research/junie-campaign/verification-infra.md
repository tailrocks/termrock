# Verification Infrastructure — junie-tui ⇄ TermRock fidelity harness

Date: 2026-09-02. **Implemented** — see "Implemented" at the bottom of this file.
Reference: `/Users/donbeave/Projects/terminal-components-claude` (crate `junie-tui`, bins `showcase` + `tablepro`).
Target: `/Users/donbeave/Projects/tailrocks/termrock`.

Every claim below marked **[verified]** was executed against the real repos in this session.

---

## 0. Executive summary

| Question | Answer |
|---|---|
| How is the reference rendered? | Live binary in a 120×40 tmux pane; `tmux capture-pane` produces `.ansi` (SGR) and `.txt` (plain); Python rasters `.ansi` → `.png`. |
| Is the reference byte-stable? | **Yes, run-to-run identical** for `.txt` and `.ansi` **[verified]**. PNG is deterministic given the same `.ansi` + font **[verified]**. |
| Is the committed `shots/` usable as truth? | **No — stale.** `shots/` was last touched at the initial commit `4b857a0`; HEAD is `e43cf67`, which added Code editor / Data grid / Chips & selects / Pickers pages. Re-captured `f_overview.txt` differs from the committed one in 7 lines (sidebar rows) **[verified]**. |
| What IS fresh and stable in the reference? | `tests/showcase_baseline.txt` — 40 FNV-1a digests over painted cells at 120×40 and 80×24, passing at HEAD **[verified]**. But it is an opaque hash that *excludes the sidebar area*, so it cannot be used for cross-repo diffing. |
| Termrock text render today? | `cargo run -p termrock-lookbook -- frame --story <id> [--cols N] [--rows N] [--keys k1,k2]` → JSON cell grid on stdout. **[verified runs]** |
| Termrock PNG render today? | `cargo run -p termrock-lookbook -- render-png --out <dir>` (123 subset PNGs). **[verified runs]** |
| Comparable today with zero new code? | **Text geometry only**, via a shell one-liner (§4.1). No color compare, no pixel compare. |
| Minimal new code? | ~200 lines of Python in `verify/junie/` (§6): ANSI→cell parser (reuses the reference's own `ansi2html.py` state machine), frame-JSON→grid, per-cell text diff, perceptual PNG diff, runner + baseline mode. |

**Blocking correction to `campaign-plan.md`:** the plan ranks `shots/` as source of truth #1
and #2. Those artifacts predate 4 of the 20 reference pages. The harness must
**re-generate** reference artifacts at HEAD and commit the regenerated grids, never read
the stale `shots/` in place.

---

## 1. Reference capture pipeline (junie-tui)

### 1.1 Files

| File | Role |
|---|---|
| `tools/capture.sh` | tmux driver: `start <cols> <rows>`, `keys`, `mouse`, `shot <name>`, `resize`, `stop`. |
| `tools/env.sh` | Exports `PY` = path to a **scratchpad venv** interpreter with Pillow. |
| `tools/ansi2html.py` | SGR state machine (`State`, `apply`, `SGRE`, `OTHER`) + `.ansi` → standalone HTML. |
| `tools/ansi2png.py` | `.ansi` → PNG via Pillow. |
| `src/bin/showcase/main.rs` | `showcase [--color truecolor\|256\|16\|none] [--page NAME]`. |
| `src/bin/tablepro/main.rs` | `tablepro [--color …] [--connect NAME]`. |
| `src/bin/showcase/app_tests.rs` | `Harness` (TestBackend) + `showcase_visual_baseline` FNV test. |

### 1.2 What `capture.sh` actually does

```bash
tools/capture.sh start 120 40
  tmux -f /dev/null new-session -d -s junie_cap -x 120 -y 40 \
    "env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor ${BIN} ${ARGS:-} 2>shots/stderr.log; sleep 30"
  tmux set-option -t junie_cap status off
  tmux set-option -g default-terminal "tmux-256color"
  tmux set-option -ga terminal-overrides ",*:Tc"
```

- Terminal size is the **tmux pane geometry**, so `-x 120 -y 40` is exactly 120×40 cells.
- Env is pinned: `NO_COLOR` unset, `TERM=xterm-256color`, `COLORTERM=truecolor`.
  `junie_tui::theme::ColorLevel::detect()` therefore resolves to `TrueColor`.
- `Theme::for_level(TrueColor)` returns exactly `Theme::junie()` **[verified, `src/theme.rs:166`]**,
  so the tmux-rendered frame and the in-process `Harness` (which constructs `Theme::junie()`
  directly) are the same theme. No color-level skew between the two paths.

```bash
tools/capture.sh shot NAME
  cols=$(tmux display -p '#{pane_width}'); rows=$(…pane_height)
  tmux capture-pane -t junie_cap -e -p -N > shots/NAME.ansi      # SGR truecolor
  tmux display -p '#{cursor_x} #{cursor_y} #{cursor_flag}' > shots/NAME.cursor
  tmux capture-pane -t junie_cap -p       > shots/NAME.txt       # plain
  python3 tools/ansi2html.py shots/NAME.ansi shots/NAME.html 120 40
  $PY tools/ansi2png.py shots/NAME.ansi shots/NAME.png 120 40 shots/NAME.cursor
```

**BUG — the default binary is wrong.** `BIN=${BIN:-target/debug/junie-tui}`, but the crate
builds `showcase` and `tablepro`. With the default, the pane stays blank and
`shots/stderr.log` says `env: target/debug/junie-tui: No such file or directory` **[verified]**.
The harness must always pass `BIN=` explicitly (or the reference repo needs a one-line fix).
This is why an unqualified "run capture.sh" reproduces nothing.

### 1.3 Exact artifact contents **[verified byte-level]**

`shots/f_overview.txt` (120×40), produced by `capture-pane -p`:

- 41 lines = 40 rows + trailing newline.
- **No ANSI escapes at all.**
- **Trailing whitespace is stripped per line** by tmux (0 of 40 lines end in a space).
  So the file is a *ragged-right* text grid: line 1 is 118 columns wide, not 120.
- Blank rows are genuinely empty lines.

`shots/f_overview.ansi`, produced by `capture-pane -e -p`:

- Same 40 rows, but each row **padded to exactly 120 visible cells** with SGR-styled spaces.
- SGR deltas carry across line boundaries; the stream starts
  `\x1b[38;2;255;255;255m\x1b[48;2;0;0;0m \x1b[38;2;72;224;84m▪…` — truecolor `38;2;r;g;b`.
- 11 523 bytes vs 3 063 for `.txt`.

`shots/f_overview.cursor` → `120 39 0` (x, y, visible-flag).

`shots/f_overview.png` → 1104×824 RGB. Formula: `(cols*CW + 2*PAD) × (rows*CH + 2*PAD)`
with `CW=9, CH=20, SIZE=15, PAD=12` → `(1080+24) × (800+24)`.

`ansi2png.py` rasters with **JetBrains Mono Nerd Font Mono** at `~/Library/Fonts/`,
falling back to `/System/Library/Fonts/Menlo.ttc`. Per-cell it fills the cell rect with bg,
draws the glyph at `(x, y+1)`, applies `dim` as a 60/40 fg↔bg blend, draws underline at
`CH-3`, and paints the cursor as a solid white rect when `cursor_flag == 1`.
Wide chars get `2*CW`. `.html` is ignored by this campaign (its CSS is font-dependent).

### 1.4 Scene selection

Scenes are **pages**, not flags on the capture script. `ARGS` is forwarded to the binary:

```bash
ARGS="--page Overview" BIN=target/debug/showcase tools/capture.sh start 120 40
```

The 20 pages (`NAV_ENTRIES`, `src/bin/showcase/app.rs:58`) — `PageId::from_name` matches on
alphanumeric-lowercased label, so `"chips & selects"`, `Chips`, `chipsselects` all resolve:

| # | `--page` | Label | Section |
|---|---|---|---|
| 0 | `Overview` | Overview | Foundations |
| 1 | `Buttons` | Buttons | Components |
| 2 | `Inputs` | Inputs | Components |
| 3 | `TextAreas` | Text areas | Components |
| 4 | `Forms` | Forms | Components |
| 5 | `Lists` | Lists | Components |
| 6 | `Trees` | Trees | Components |
| 7 | `Tables` | Tables | Components |
| 8 | `Editable` | Editable tables | Components |
| 9 | `Panels` | Panels | Components |
| 10 | `Sidebars` | Sidebars | Components |
| 11 | `Dialogs` | Dialogs | Components |
| 12 | `Progress` | Progress | Components |
| 13 | `Scrolling` | Scrolling | Components |
| 14 | `Editor` | Code editor | Components |
| 15 | `Grid` | Data grid | Components |
| 16 | `Chips` | Chips & selects | Components |
| 17 | `Pickers` | Pickers | Components |
| 18 | `Settings` | Settings | Screens |
| 19 | `TaskRunner` | Task runner | Screens |

Non-default sizes use `tools/capture.sh resize <cols> <rows>` (that is where the
`f_80x24_*` and `s_*_80` artifacts came from). Interactive states (`*_hover`, `*_edit`,
`*_running`, `t_*` tablepro states) are driven with `tools/capture.sh keys …` and
`tools/capture.sh mouse <x> <y> [move|click|wheelup|wheeldown]` (SGR mouse sequences via
`send-keys -l`). 67 distinct scene names exist in `shots/`.

### 1.5 Determinism, measured **[verified]**

| Test | Result |
|---|---|
| Same source, two captures, minutes apart → `.txt` | **byte-identical** |
| Same source, two captures → `.ansi` | **byte-identical** |
| `.ansi` → `.png` twice | **byte-identical** |
| Fresh `.png` vs committed `shots/f_overview.png` | **pixel-identical** (`ImageChops.difference().getbbox() is None`) |
| Committed `shots/f_overview.txt` vs HEAD render | **7 lines differ** — shots are stale, not nondeterministic |

Conclusion: the capture pipeline is deterministic. The committed artifacts are simply old.
Timing sleeps (`0.08`/`0.15` s in `keys`/`mouse`) are generous relative to the 80 ms spinner
tick; animated pages (task runner, spinner) must be shot at a pinned tick count, which the
`keys` path already effectively does by sampling after a fixed delay.

### 1.6 The in-process baseline (why it can't be reused)

`src/bin/showcase/app_tests.rs:648` `showcase_visual_baseline`:

- Builds `Harness::new(w, h, PageId)` → `Terminal<TestBackend>` → `term.draw(|f| app.render(f))`.
- FNV-1a over `format!("{}|{:?}|{:?}|{:?};", symbol, fg, bg, modifier)` for every cell.
- **Skips every cell inside `app.sidebar_area()`** (the sidebar grows with page count).
- Writes `tests/showcase_baseline.txt` = `"<w>x<h> <label> <16-hex>"` × 40 lines.
- `UPDATE_BASELINE=1 cargo test baseline` blesses; plain `cargo test` compares. **Passes at HEAD.**

Two disqualifiers for cross-repo use: it is an opaque digest (no per-cell delta), and it
excludes the sidebar. It also lives in a `#[cfg(test)]` module of a *binary* target, so
`junie_tui` does not export it. Keep it as the reference repo's own regression gate; do not
build the campaign on it. `tablepro/app_tests.rs` has the same `Harness` shape but **no**
baseline file.

---

## 2. Termrock rendering paths available today

### 2.1 `crates/termrock-raster` — Buffer → PNG

Pure-Rust swash + tiny-skia, fonts vendored (`include_bytes!` JetBrains Mono Regular/Bold/Italic
in `src/fonts.rs`, SHA-256 pinned by tests). No resvg, no SVG.

```rust
// crates/termrock-raster/src/lib.rs — entire public surface
pub const CELL_WIDTH_PX: u32 = 9;
pub const CELL_HEIGHT_PX: u32 = 18;
pub const FONT_SIZE_PX: f32 = 14.0;
pub const BASELINE_PX: u32 = 14;

pub use compare::{PixelDiff, compare_png_pixels};
pub use render::{RenderError, render_pixmap, render_png};

render_pixmap(buffer: &ratatui::buffer::Buffer, palette: &termrock::style::RolePalette)
    -> Result<tiny_skia::Pixmap, RenderError>;
render_png(buffer: &Buffer, palette: &RolePalette) -> Result<Vec<u8>, RenderError>;

compare_png_pixels(a: &[u8], b: &[u8]) -> Result<(), PixelDiff>;
// PixelDiff = Decode{input,message} | DimensionMismatch{a,b} | FirstDifference{x,y,a:[u8;4],b:[u8;4]}
```

Input is an already-painted `ratatui` `Buffer` plus a palette (used only for the canvas
background). Handles REVERSED, DIM (×6/10), UNDERLINED (rows 15–16), CROSSED_OUT (rows 8–9),
wide graphemes (span 2), per-cell clipping. Render-twice is SHA-256 identical
(`crates/termrock-raster/tests/raster.rs`).

**No text-grid export in this crate.** Text lives in lookbook (§2.2) and in the
`render_golden` test helper.

### 2.2 `crates/termrock-lookbook` — the headless render seam

`src/frame.rs` is backend-neutral and fully offline (ratatui `TestBackend` → `Terminal::draw`),
no browser, no crossterm, no TTY:

```rust
pub const STORY_PAD: u16 = 1;
pub const CELL_WIDTH_PX: u16 = 9;
pub const CELL_HEIGHT_PX: u16 = 18;
pub const RESPONSIVE_STORY_SIZES: &[(u16,u16)] = &[(28,6),(40,8),(56,12),(72,16),(80,24)];

pub struct FrameCell { ch: String, fg: [u8;3], bg: [u8;3],
                       bold: bool, dim: bool, underline: bool, reversed: bool }   // serde
pub struct TerminalFrame { story_id, title, component: String, cols, rows, story_cols,
                           story_rows: u16, cells: Vec<FrameCell>, interactive: bool,
                           theme: String }                                        // serde
pub struct PreviewKey { key: String, ctrl, alt, shift, meta: bool }

pub fn story_by_id(id: &str) -> Option<Story>;
pub fn paint_story_buffer(story: Story, system: &DesignSystem,
                          cols: Option<u16>, rows: Option<u16>) -> Buffer;
pub fn paint_story_frame(story, system, cols, rows) -> TerminalFrame;
pub fn paint_story_after_keys(story, system, cols, rows, keys: &[PreviewKey]) -> TerminalFrame;
pub fn encode_buffer(&Buffer) -> (u16, u16, Vec<FrameCell>);   // lossy: RGB approx, drops italic/strike
pub fn decode_preview_key(&PreviewKey) -> Option<KeyEvent>;
```

`paint_story_buffer` paints the `PREVIEW_CARD` background, `Clear`, `Role::Canvas`, then the
story into the inset area (`STORY_PAD = 1` per side). **Use `paint_story_buffer` (not
`encode_buffer`) when fidelity matters** — `encode_buffer` is the lossy one.

`src/png.rs` (`#[cfg(feature = "native")]`):

```rust
pub const JACKIN_SUBSET_COMPONENTS: [&str; 17];
pub fn subset_stories() -> Vec<Story>;                  // 123 stories today, test asserts >= 87
pub fn story_png_filename(story: Story) -> String;      // id.replace('/', "-") + ".png"
pub fn render_story_png(story: Story) -> Vec<u8>;       // paint_story_buffer → render_png
pub fn write_story_pngs(out_dir) -> io::Result<Vec<PathBuf>>;
```

`Story` format (`src/stories.rs`, 27 k lines): `Story { id, title, identity, description,
width, height, spec }` where `StorySpec = Fixture(fn(&mut Frame, Rect, &DesignSystem)) |
Mounted(fn() -> Box<dyn StoryInteraction>)`. `stories()` hand-builds ~954 passive entries plus
~200 mounted interactive ones; `cargo run -p termrock-lookbook -- list --format json` reports
**1155** stories **[verified]**.

### 2.3 CLI — `crates/termrock-lookbook` bin

```
usage: termrock-lookbook <terminal|list|inventory|render|render-png|check|frame|export-posters>

frame --story <id> [--cols N] [--rows N] [--keys k1,k2]   # ONE TerminalFrame JSON on stdout
render-png --out <dir>                                    # 123 subset PNGs
export-posters --out <dir> --story <id>…                  # <slug>.json posters
render [--theme phosphor|slate] --out <dir>               # SVG (offline determinism check only)
list [--format json] | inventory --format json | check --dir <dir>
terminal [--story <id>]                                   # the only TTY-requiring subcommand
```

`--keys` drives the real interactor (`paint_story_after_keys`), so interactive reference
states have a headless counterpart. **[verified]**

```
$ cargo run -q -p termrock-lookbook -- frame --story list/selection --cols 40 --rows 8
{"story_id":"list/selection","title":"List","component":"List","cols":42,"rows":10,
 "story_cols":40,"story_rows":8,"cells":[{"ch":" ","fg":[255,255,255],"bg":[28,28,28],
 "bold":false,...}, ...]}
```

```
$ cargo run -q -p termrock-lookbook -- render-png --out /tmp/tr_png   # 116-123 files
$ python3 -c "from PIL import Image; print(Image.open('/tmp/tr_png/action-bar-basic.png').size)"
(450, 72)   # 50 cols × 9 + 0 pad, 4 rows × 18 + 0 pad  → NO padding
```

### 2.4 Existing baseline machinery in termrock

| Layer | Path | Comparator | Bless |
|---|---|---|---|
| Golden text (15 flagship stories) | `crates/termrock-lookbook/goldens/<id with /→__>.txt` | exact string, reports first differing row | `TERMROCK_BLESS_PREVIEWS=1` (`mise run bless-previews`) |
| PNG baselines (123 files) | `crates/termrock-lookbook/baselines/png/<slug>.png` | `compare_png_pixels`, **zero tolerance**, plus a render-twice determinism gate that must never be blessed | `TERMROCK_BLESS_PNGS=1` (`mise run bless-pngs`) |
| Poster JSON (227 files) | `docs/public/preview-posters/<slug>.json` | `docs/scripts/export-preview-posters.ts --check`: set-exactness + identity + dims + `JSON.stringify` equality | re-run `bun run build:preview-posters` |
| SVG inventory | `crates/termrock-lookbook/src/svg.rs:125` | names only, bytes explicitly not compared | — |

The golden `.txt` format is **`"<id> <cols>x<rows> (story WxH)"` header, then `rows` lines of
`cols` symbols, space-filled, trailing whitespace NOT trimmed** (§4.1 needs a trim).

Posters are **not** PNGs — they are the checked-in `TerminalFrame` JSON, painted client-side
onto a canvas. Commit `f51d0ba8` made `example_help_doctor_report()` env-independent precisely
so this JSON compare stays portable; commit `57f2fe43` re-blessed 35 stale posters. That is the
exact same failure mode as the stale reference `shots/`.

### 2.5 Test-time rendering idiom

No insta, no snapshot crate, no `assert_buffer` helper — everything is hand-rolled over
ratatui `Buffer`:

```rust
// crates/termrock/tests/design_gate.rs:287
fn painted(area: Rect, paint: impl FnOnce(&mut Buffer)) -> Buffer {
    let mut buffer = Buffer::empty(area);
    paint(&mut buffer);
    buffer
}
```

plus `TestBackend`/`Terminal` in `crates/termrock-lookbook/src/frame.rs`, and per-cell reads
via `buffer[(x,y)].symbol() / .fg / .bg` and whole-buffer `assert_eq!(a.content(), b.content())`.
`crates/termrock/tests/design_gate.rs` (~40 tests) gates **style law**, not fidelity: one
overflow-note, one chord notation, no underline grammar, selection chrome not overridden in
widget paint, `bold_budget_per_row`, `accent_budget`, modal geometry fuzz, scroll/truncation
rules, `patterns_only_compose` / `widgets_never_import_patterns`, `state_matrix_distinct`.
It will not catch a junie-fidelity regression — that is what this harness is for.

### 2.6 Other CLI

- `crates/termrock-cli` bin `termrock`: `doctor [--profile …]`, `contract list|check`,
  `plan|add|diff|check <entry-dir>`. Registry diff, **no rendering to file**.
- `crates/termrock-showcase` bin `termrock-showcase`: no arg parsing, unconditional
  crossterm loop, **requires a TTY** — useless for capture.
- `crates/termrock-lookbook-web`: no binaries; wasm-bindgen `mount_demo / dispatch_demo /
  demo_frame / reset_demo / catalog_json` over `demo::DemoSession`. `check-preview-metrics.ts`
  drives the real WASM headlessly under Bun.

---

## 3. Cell-metric reconciliation (the geometry mismatch)

| | reference `ansi2png.py` | termrock `termrock-raster` |
|---|---|---|
| cell width | 9 px | 9 px — **match** |
| cell height | 20 px | 18 px |
| font size | 15 px | 14 px |
| baseline | n/a (draws at `y+1`) | 14 px |
| outer padding | 12 px all sides | 0 |
| rasterizer | Pillow + FreeType (system) | swash (vendored, pure Rust) |
| font | JetBrains Mono Nerd Font Mono from `~/Library/Fonts` | vendored JetBrains Mono |
| dims | `(cols*9+24) × (rows*20+24)` | `cols*9 × rows*18` |

Verified PNG sizes: reference 1104×824 for 120×40; termrock 450×72 for 50×4.

Consequence: a cross-repo PNG pixel diff is meaningless until the grids are aligned. The
harness must rasterize the reference `.ansi` with **termrock's** metrics — `CW=9, CH=18,
SIZE=14, PAD=0` — so both images are `cols*9 × rows*18` and cell `(x,y)` maps to the same
pixel rect on both sides. After that, pixels still differ per-glyph because the rasterizers
differ, which is why the PNG layer is threshold-based and advisory (§5.3), never a hard gate.

Text and color layers need no such reconciliation: both sides are pure cell grids.

---

## 4. What is comparable TODAY with zero new code

### 4.1 Text geometry — yes, one line of shell **[works]**

Reference `.txt` is already trailing-trimmed; termrock golden `.txt` is padded and has a
header. Normalize and diff:

```sh
REF=/Users/donbeave/Projects/terminal-components-claude
diff <(sed -e 's/[[:space:]]*$//' "$REF/shots/f_overview.txt") \
     <(sed -e '1d' -e 's/[[:space:]]*$//' \
        crates/termrock-lookbook/goldens/list__selection.txt) || true
```

This gives a real line-level delta with zero implementation. It is enough to start
workstream 8 immediately.

### 4.2 Cell colors — no

Requires parsing the reference `.ansi` SGR stream into per-cell fg/bg. `tools/ansi2html.py`
already contains the exact state machine needed (`State` + `apply`), but it only exposes
HTML output, not a cell grid. Needs the ~40-line parser in §6.

### 4.3 Pixels — no

`compare_png_pixels` is zero-tolerance and the two PNGs have different dimensions and
different rasterizers. A byte `cmp` reports "differ" and nothing more. Needs §5.3.

### 4.4 What cannot be compared at all

- `shots/*.html` — font-dependent CSS, no termrock counterpart. Skip.
- `tests/showcase_baseline.txt` FNV digests — sidebar-excluded, opaque. Reference-internal only.
- Any termrock `render` SVG output — byte-identity is explicitly not supported upstream.

---

## 5. Proposed harness design

### 5.1 File layout

```
verify/junie/
  README.md                     # how to run, how to bless, what each layer means
  scenarios.json5               # the ONLY hand-maintained mapping (see 5.2)
  bin/
    ref_capture.sh              # reference-side driver (see 5.4)
    ansi2grid.py                # .ansi -> cell grid  (imports reference ansi2html.State)
    frame2grid.py               # termrock frame JSON -> cell grid
    diff_grid.py                # text + color cell diff, per-line/per-cell
    diff_png.py                 # perceptual pixel diff
    run.py                      # orchestrator: report + baseline mode
  reference/                    # COMMITTED, regenerated reference artifacts
    scenes/<scene>.txt          # plain text grid     (byte-stable, CI-safe)
    scenes/<scene>.ansi         # SGR truecolor grid  (byte-stable, CI-safe)
    scenes/<scene>.cursor       # cursor x y flag
    scenes/<scene>.png          # raster at TERMROCK metrics (LOCAL only, advisory)
    manifest.json               # {scene: {cols, rows, junie_commit, captured_at}}
  baselines/
    <scene>.grid.json           # blessed per-cell delta budget / exact expected grid
    <scene>.pixels.json         # blessed pixel metrics (local only)
  out/                          # gitignored: fresh renders, diffs, report.md, report.json
```

`verify/junie/out/`, `verify/junie/reference/scenes/*.png` go in `.gitignore`.

### 5.2 Scenario table

One entry per comparison. Deliberately explicit rather than inferred, because the
junie→termrock mapping is a design decision that workstreams 5/6 own:

```json5
{
  "scene": "f_overview",              // reference artifact stem
  "reference": {
    "bin": "showcase",                // showcase | tablepro
    "args": ["--page", "Overview"],
    "cols": 120, "rows": 40,
    "keys": [],                       // capture.sh keys, in order
    "mouse": []                       // ["move|x,y", "click|x,y"]
  },
  "termrock": {
    "story": "overview/…",            // termrock-lookbook story id
    "cols": 120, "rows": 40,
    "keys": []                        // PreviewKeys through paint_story_after_keys
  },
  "layers": { "text": true, "color": true, "pixel": "advisory" },
  "tolerance": {
    "text_cells": 0,                  // exact
    "color_cells_max": 0,             // exact RGB triples
    "pixel_max_deltaE": 6.0,          // CIEDE2000
    "pixel_max_fraction_off": 0.01    // 1% of cells may exceed deltaE
  }
}
```

Seed it with the 20 showcase pages × {120×40, 80×24} = 40 scenarios — exactly the same matrix
the reference's own FNV baseline covers, so the two are mutually checkable.

### 5.3 The three comparison layers

**(a) Text grid — hard gate.** Both sides normalized to: `rows` lines, each `cols` cells,
trailing whitespace stripped. Diff is line-major then cell-major:

```
scene f_overview  120x40
  L19  C0   ref '▎'  got ' '            (gutter missing)
  L19  C3   ref 'C'  got 'R'
  L19  C0-34 ref '▎  Code editor'  got '▎  Task runner'
summary: 7/40 lines differ, 41/4800 cells differ (0.85%)  FAIL
```

Exit non-zero when `text_cells` budget is exceeded. This layer is font-independent,
OS-independent, and byte-stable — **it is the gate that runs in CI.**

**(b) Color grid — hard gate.** Convert reference `.ansi` to per-cell
`(symbol, fg_rgb, bg_rgb, bold, dim, italic, underline, reverse)` using the reference's own
`ansi2html.State` (import it — do not re-implement the SGR table). Compare against the
termrock `FrameCell` fields. Catches everything text cannot: accent vs error, focus bar
color, hover lift, backdrop dimming. Same report shape, `color_cells` budget.

**(c) Pixel — advisory, local only.** Re-rasterize the reference `.ansi` at termrock metrics
(§3) so both PNGs are `cols*9 × rows*18`, then per-cell compare in CIELAB with CIEDE2000 and
report: cells over threshold, worst cell, mean deltaE, and a side-by-side PNG with differing
cells outlined. Because Pillow/FreeType and swash rasterize the same JetBrains Mono
differently, this layer can only ever be advisory. **Never a CI gate.** Reuse
`termrock_raster::compare_png_pixels` only for the intra-repo zero-tolerance gate it already
serves.

### 5.4 Exact commands

Reference regeneration (one-time per junie commit; must be re-run whenever the reference
moves — the stale-`shots/` lesson):

```sh
JUNIE=/Users/donbeave/Projects/terminal-components-claude
TERMROCK=/Users/donbeave/Projects/tailrocks/termrock
cd "$JUNIE" && cargo build                          # 9 s
cd "$TERMROCK/verify/junie"

# one scene
bin/ref_capture.sh --bin showcase --args "--page Overview" --cols 120 --rows 40 \
                   --out reference/scenes f_overview
# all 40 seeded scenarios
bin/ref_capture.sh --all --out reference/scenes
```

`bin/ref_capture.sh` is a thin wrapper that fixes the two capture.sh defects — it always
passes `BIN=$JUNIE/target/debug/<bin>` (never the broken `junie-tui` default) and always
`source "$JUNIE/tools/env.sh"` for `$PY` (that venv path is a scratchpad absolute path and
will vanish; the script should fall back to `python3 -m pip install pillow` into a local
`verify/junie/.venv`). It then calls `capture.sh start/shot/stop` and normalizes output into
`verify/junie/reference/scenes/`.

Termrock rendering per scenario:

```sh
cargo run -q -p termrock-lookbook -- frame --story <story> \
      --cols <cols> --rows <rows> [--keys k1,k2] > out/<scene>.frame.json
```

Compare + report:

```sh
python3 bin/run.py                       # compare all scenarios, write out/report.{md,json}
python3 bin/run.py --only f_overview     # one scenario
python3 bin/run.py --layer text          # text only (what CI runs)
python3 bin/run.py --update-baseline     # bless: copy out/ -> baselines/
```

Existing termrock-side gates to run alongside (already wired, no new code):

```sh
cargo nextest run -p termrock-lookbook --all-features --test goldens --locked
cargo nextest run -p termrock-lookbook --all-features --test png_baselines --locked
cd docs && bun run check:preview-posters && bun run check:preview-metrics
```

### 5.5 Report format

`out/report.json`:

```json
{ "junie_commit": "e43cf67", "termrock_commit": "<sha>",
  "generated_at": "…", "total": 40, "passed": 31, "failed": 9,
  "scenarios": [ { "scene": "f_overview",
                   "text":  { "lines_differing": 7, "cells_differing": 41,
                              "cells_total": 4800, "pass": false,
                              "diff": "out/f_overview.text.diff" },
                   "color": { "cells_differing": 12, "pass": false,
                              "diff": "out/f_overview.color.diff" },
                   "pixel": { "mean_deltaE": 3.1, "max_deltaE": 9.4,
                              "fraction_off": 0.004, "advisory_pass": true } } ] }
```

`out/report.md` is the human read: one table row per scenario, worst-offender ordering,
embedded links to the per-cell diffs and the outlined PNG pair.

### 5.6 CI vs local

| | CI (termrock repo) | Local (dev machine) |
|---|---|---|
| Reference re-render | **never** — the junie repo is not a termrock dependency | on demand, when `reference/manifest.json` is behind |
| Text layer | **hard gate.** Compare fresh termrock render against **committed** `verify/junie/reference/scenes/*.txt` | same |
| Color layer | **hard gate** against committed `*.ansi` | same |
| Pixel layer | **skipped** — font/OS/rasterizer dependent | advisory only |
| Baseline update | never | `--update-baseline`, reviewed in the PR |

Rationale: `.txt` and `.ansi` are byte-stable run-to-run **[verified]** and contain no
font, OS, or terminal-emulator dependence once captured — they are already cell grids.
PNG inherits Pillow + FreeType + `~/Library/Fonts` + macOS, exactly the class of
environment-dependence that made the doctor poster check non-portable before `f51d0ba8`.

CI placement: extend the existing `mise run gate` / `docs.yml` rather than adding a workflow.
Concretely, one step in `docs.yml` job `docs` next to the render-a/render-b `diff -r` block:

```yaml
- run: python3 verify/junie/bin/run.py --layer text --layer color --precomputed-reference
```

plus a weekly `hygiene.yml` step that re-renders the reference and opens a PR when
`reference/manifest.json` is stale — so the committed grids cannot rot the way `shots/` did.
That staleness PR is the structural fix for the failure mode already observed twice
(stale `shots/`, stale `preview-posters/`).

### 5.7 Keeping reference renders byte-stable

1. Pin env exactly as `capture.sh` does: `env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor`.
   `ColorLevel::detect()` then returns `TrueColor`, which equals `Theme::junie()`.
2. Pin geometry via tmux pane size (`-x/-y`), never via the binary.
3. Pin the junie commit in `reference/manifest.json` and require it to match the working tree
   before regenerating.
4. Record `tmux -V` in the manifest. `.txt` is immune to SGR-emission changes across tmux
   versions; `.ansi` is not. If `.ansi` ever drifts across a tmux upgrade, re-normalize through
   `ansi2grid.py` — the *cell grid* is the stable form, so commit nothing downstream of the
   raw stream except the normalized grid.
5. Never commit PNGs as CI baselines. Commit them only as local advisory artifacts.
6. Drive interactive states with `keys`/`mouse`, not wall-clock sleeps, wherever possible; where
   an animation must settle, sample at a fixed multiple of the 80 ms tick and record the tick
   count in the scenario entry.

---

## 6. Minimal new code

| File | Lines | Purpose |
|---|---|---|
| `verify/junie/bin/ref_capture.sh` | ~50 | wraps reference `capture.sh`, fixes `BIN` default and `$PY` fallback, normalizes output paths |
| `verify/junie/bin/ansi2grid.py` | ~40 | `.ansi` → per-cell `(sym, fg, bg, mods)`; imports `State`/`apply` from reference `tools/ansi2html.py`, no SGR table re-implemented |
| `verify/junie/bin/frame2grid.py` | ~25 | termrock `frame` JSON → same cell-grid shape |
| `verify/junie/bin/diff_grid.py` | ~60 | text + color per-line/per-cell diff, budget check, machine-readable output |
| `verify/junie/bin/diff_png.py` | ~60 | metric-normalized re-raster + CIEDE2000 per-cell diff + outlined pair |
| `verify/junie/bin/run.py` | ~90 | scenario table, orchestration, `report.{md,json}`, `--update-baseline` |
| `verify/junie/scenarios.json5` | data | the 40 seeded scenarios |

Total ≈ 325 lines, no new Rust, no new crate, no new workflow file.

**Two optional reference-repo changes** (better long-term, both small, and both out of scope
for this campaign since the tmux path avoids them):

1. `tools/capture.sh:12` — `BIN=${BIN:-target/debug/showcase}` (the current default points at a
   binary that does not exist; every invocation is broken until overridden).
2. A `--dump-grid` mode (or a test) that writes the full-frame `TestBackend` buffer as text and
   styled cells, replacing tmux entirely. This removes the tmux dependency and the sidebar
   exclusion that cripples the FNV baseline, and makes the reference render reproducible in
   plain CI. Worth doing in the reference repo on its own merits.

**Zero-new-code start available now (§4.1):** the `diff <(sed …)` one-liner against the
existing `crates/termrock-lookbook/goldens/*.txt`.

---

## 7. Open questions for the campaign owner

1. **Story mapping ownership.** `scenarios.json5` needs one termrock story id per reference
   page. That mapping is the deliverable of workstreams 2/5/6 — the harness should ship with
   the 40 reference-side entries and empty termrock ids, filled in as ports land.
2. **TablePro.** `tablepro` has no baseline file and 60+ captured states (`t_*`). Termrock has
   no equivalent app surface today. Recommend deferring tablepro scenarios until workstream 7,
   but capture its reference grids now while the reference is at a known commit — they are
   cheap and the reference repo may move.
3. **Interactive states.** `*_hover`, `*_edit`, `*_running` require `--keys`/`--mouse` on the
   reference side and `PreviewKey`s on the termrock side. Mouse coordinates are cell-based on
   both sides, so they translate directly.
4. **macOS↔Linux rasterizer equivalence** (research `tui-png-baselines/04` assumption A3) is
   still open for termrock's own PNG gate. It does not affect this harness, because the pixel
   layer is local-only advisory — but resolving it would let the pixel layer move into CI.

---

## 8. Implemented

Implemented on 2026-09-02, same day as the design. Everything below is
`verify/junie/` plus the section you are reading; nothing under `crates/`,
`docs/` or `tests/` was touched. Reference repo left byte-clean (verified with
`git status` after every capture).

### 8.1 What exists

```
verify/junie/
  README.md                     run/bless/port instructions (the operator doc)
  scenarios.json5               45 scenarios: 40 showcase + 5 tablepro
  .gitignore                    out/, last-report.json, reference/scenes/*.png, __pycache__/
  bin/ref_capture.sh            reference capture (one scene)
  bin/_capture_all.py           per-scene driver behind --all
  bin/_manifest.py              reference/manifest.json writer
  bin/ansi2grid.py              .ansi -> cell grid (imports reference tools/ansi2html.py State/apply)
  bin/frame2grid.py             lookbook frame JSON -> cell grid
  bin/diff_grid.py              text + color layers, budgets, exit codes
  bin/diff_png.py               advisory pixel layer (CIEDE2000), Pillow-optional
  bin/shim/tmux                 private-socket tmux shim (see 8.4)
  reference/scenes/*.{txt,ansi,cursor}   45 scenes × 3 artifacts, committed
  reference/manifest.json       per-scene provenance (junie commit, tmux, SHA-256)
  baselines/<scene>.grid.json   3 blessed ratchets (the active scenarios)
  out/, last-report.json        generated, gitignored
```

### 8.2 Exact commands

```sh
cd /Users/donbeave/Projects/tailrocks/termrock/verify/junie

bin/ref_capture.sh --all --out reference/scenes     # regenerate the reference side (~90 s for 45)
bin/ref_capture.sh --bin showcase --page Tables --cols 120 --rows 40 showcase_tables_120x40
bin/ref_capture.sh --bin tablepro --args '["--connect","Local PostgreSQL"]' tablepro_local_120x40
python3 bin/run.py                                  # 3 PASS / 0 FAIL / 42 SKIP, exit 0
python3 bin/run.py --layer text                     # text only (the CI shape)
python3 bin/run.py --only showcase_tables_120x40
python3 bin/run.py --update-baseline                # bless measured deltas into baselines/
```

### 8.3 What was captured

All 20 showcase pages from `NAV_ENTRIES` at both 120×40 and 80×24 = 40 scenes,
plus 5 tablepro screens (`default` at both sizes, `--connect "Local PostgreSQL"`,
`--connect Production`, and the `?` help overlay). junie at `e43cf67`
("feat: refine showcase and tablepro UI"), built `--release`, captured through
tmux 3.7c on a private socket, env pinned exactly as §1.2 describes
(`-u NO_COLOR TERM=xterm-256color COLORTERM=truecolor`), so the theme is
`Theme::junie()`. Every artifact is in `reference/scenes/` with its SHA-256 in
`reference/manifest.json`.

`tablepro` has no `--page` flag (§7.2 was right); its screens are reached with
`--connect` and `--key`. Multi-word connection names survive because
`ref_capture.sh` re-quotes the arg vector with `shlex` before `capture.sh`'s
unquoted `${ARGS}` reaches `/bin/sh -c`. The screens that are *not* reachable
this way (per-tab states, query runner, history) are not in the table — they
need the `keys` choreography to be worked out, which belongs to workstream 7.

### 8.4 Deviations from §5 (each one deliberate)

1. **Private tmux socket.** `capture.sh` sets `default-terminal` *globally* on
   whatever server it finds. `ref_capture.sh` puts `bin/shim/tmux` first on
   `PATH`, so every call goes to a `-L jrverify` socket and the user's sessions
   are untouched. The shim resolves the real tmux by absolute path — `exec tmux`
   would re-resolve `PATH` and recurse into itself.
2. **The reference repo stays byte-clean.** `shots/stderr.log` is tracked
   upstream and `capture.sh` overwrites it; it is snapshotted and restored, and
   the scratch `shots/jr_cap.*` is deleted. `PYTHONDONTWRITEBYTECODE=1` keeps
   `tools/__pycache__` out of the reference tree.
3. **No committed derived grids.** §5.7.4 suggested committing the normalized
   cell grid as well as the raw `.ansi`. Committed grids would be a *second*
   artifact class that can rot, which is the exact failure mode this harness
   exists to kill, so `run.py` derives them from the committed `.ansi` through
   the committed parser instead. The parser is in this repo, so the derivation
   is reproducible in CI.
4. **Gating is a ratchet, not only exact-zero.** No termrock story reproduces a
   full junie page byte-for-byte today (§7.1), so "compare and fail" would gate
   on nothing. `baselines/<scene>.grid.json` blesses the measured delta;
   `--update-baseline` only ever tightens it (`min` with the previous budget), a
   scenario that declares `tolerance` overrides the ratchet, and an active
   scenario with no budget at all FAILs as `unblessed` rather than passing
   silently. A baseline whose reference digest no longer matches the committed
   `.ansi` FAILs as `stale-baseline` — that is the anti-rot tripwire.
5. **`reference.crop` / `termrock.crop`.** A story covers a *region* of a page,
   not the page, so scenarios can crop the reference to `[x,y,w,h]`; the termrock
   story is then rendered at `(w-2) × (h-2)` (one `STORY_PAD` cell per side) and
   cropped back to exactly the reference's box. Both sides are padded to the
   union when they still disagree.
6. **Text layer compares every cell; color layer stops at the row extent.**
   Trailing padding compares equal for free, so the text layer counts real
   content, including content missing on one side. The color layer stops at the
   last non-blank cell of a row on *either* side: past that, both sides are
   canvas and the delta would measure the two apps' backgrounds, not the widget.

### 8.5 Determinism (measured, this session)

Two independent `--all` runs into different directories, minutes apart:

| artifact | result |
|---|---|
| 45 × `.txt` | byte-identical |
| 45 × `.ansi` | byte-identical |

`diff -r` between the two trees is empty. No exceptions.

### 8.6 Current counts

`python3 bin/run.py` → **3 PASS, 0 FAIL, 42 SKIP** (exit 0). The 42 SKIPs are
`pending-termrock-scene`: a reference page with no termrock story yet (including
all 5 tablepro screens). The 3 active scenarios are the ones where termrock
already renders something comparable, cropped to the matching region:

| scenario | reference crop | termrock story | text cells | color cells | budget |
|---|---|---|---|---|---|
| `showcase_buttons_120x40` | `[27,6,46,6]` | `button-group/dialog` | 75/276 | 91/91 | 75 / 91 |
| `showcase_lists_120x40` | `[27,6,23,15]` | `list/selection` | 112/345 | 163/163 | 112 / 163 |
| `showcase_tables_120x40` | `[27,4,70,10]` | `table/sorted` | 466/700 | 627/627 | 466 / 627 |

These numbers are the honest distance, not a pass: `table/sorted` reproduces
junie's table *grammar* (sort glyphs, accent-barred selected row, right-aligned
numeric column) but none of its fixture data. Every further port tightens the
ratchet.

The pixel layer reports `skipped: pillow-missing` — Pillow is not installed for
this environment's `python3`, exactly the case §5.3(c) anticipated. Install it
to get advisory ΔE numbers; `bin/diff_png.py --ref-ansi <scene>.ansi --cols N
--rows N <a.png> --ref-out <b.png>` re-rasterizes the reference at termrock's
metrics first, per §3.

### 8.7 Verified gate behaviour

* `diff_grid.py` with budget 0 on the tables pair: exit 1, per-line/per-cell
  diff written. Identical grids, budget 0: exit 0, 0 cells.
* Tampering a baseline's `reference_ansi_sha256` → `FAIL stale-baseline`;
  restoring it → `PASS`.
* Rendering a story that is not the blessed one changes the counts and trips the
  budget — the gate is sensitive to real termrock changes, not just to the
  reference side.

### 8.8 How a porting agent uses it

```sh
cd /Users/donbeave/Projects/tailrocks/termrock/verify/junie
python3 bin/run.py --list-scenes | grep pending      # 1. pick a scene
sed -n '1,40p' reference/scenes/showcase_dialogs_120x40.txt   # 2. read the target
#    3. implement, then make the scenario active in scenarios.json5
#       (termrock.story + reference.crop; story renders at crop size - 2)
python3 bin/run.py --only showcase_dialogs_120x40    # 4. measure
sed -n '1,80p' out/showcase_dialogs_120x40.text.diff #    per-line/per-cell deltas
#    5. iterate until the delta stops shrinking, then
python3 bin/run.py --only showcase_dialogs_120x40 --update-baseline
```

Shrink the delta before beautifying; never loosen a budget to make a scenario
pass (declare a tighter `tolerance` instead); if a scene is a design decision
rather than a port, say so in the scenario's `note`.

### 8.9 Left open

* CI wiring (§5.6's `docs.yml` step and the weekly staleness PR) is *not* added
  here — this task was scoped to `verify/junie/`. `python3
  verify/junie/bin/run.py --layer text --layer color --precomputed-reference` is
  not the exact invocation either: the flag that exists is `--layer`, repeated,
  and the reference is always the committed one, so a CI step is simply
  `python3 verify/junie/bin/run.py --layer text --layer color`.
* tablepro `keys` choreography for per-tab states.
* Pillow (or a Pillow-bearing interpreter) on CI/dev machines if the advisory
  pixel layer is ever wanted locally.

## Addendum — 2026-09-05: scenes are replay exports, not tmux captures

The sections above describe the regime this investigation designed and the
tmux-based capture flow originally planned for `reference/scenes/`. That flow
was never the durable truth: the checked-in scene artifacts were regenerated
on 2026-09-05 as deterministic exports of the canonical catalog replay
(`termrock-catalog capture`), and `reference/manifest.json` records
replay-export provenance accordingly. `bin/ref_capture.sh`,
`bin/_capture_all.py`, `bin/_manifest.py`, and `bin/shim/tmux` are removed.
The manifest shape at line ~421 (`junie_commit`, `captured_at` per scene) and
the per-scene provenance note at line ~677 are historical; scenes now carry
`cols`, `rows`, `events`, `evidence`, and digests only, with the event
authority in `crates/termrock-catalog/src/scenarios.rs`. Live source anchoring
is the `verify/junie/source-headless/` goldens read by
`crates/termrock-catalog/tests/parity.rs`; the scenes gate is a frozen-snapshot
drift tripwire.
