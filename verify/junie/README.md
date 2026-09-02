# verify/junie — junie-tui ⇄ termrock fidelity harness

Cross-repo cell-grid comparison. The reference is [`junie-tui`]
(`/Users/donbeave/Projects/terminal-components-claude`, bins `showcase` +
`tablepro`); the target is this repo's `termrock-lookbook` stories.

Design and rationale: `research/junie-campaign/verification-infra.md`
(§5 harness design, §6 minimal new code). This directory is the whole
implementation: Python 3 stdlib plus a POSIX shell wrapper, no new Rust, no new
crate, no new workflow.

## Layout

```
scenarios.json5        the only hand-maintained mapping (reference page -> termrock story)
reference/
  scenes/<scene>.txt   plain text grid      (byte-stable, this is what CI compares)
  scenes/<scene>.ansi  SGR truecolor grid   (byte-stable run-to-run on a given tmux)
  scenes/<scene>.cursor cursor x y flag
  manifest.json        per-scene capture provenance: junie commit, tmux version, digests
baselines/<scene>.grid.json  blessed per-cell delta budgets (ratchet, see below)
bin/
  ref_capture.sh   reference-side tmux driver (wraps the reference's tools/capture.sh)
  _capture_all.py  per-scene driver used by `ref_capture.sh --all`
  _manifest.py     merges one capture into reference/manifest.json
  ansi2grid.py     .ansi -> canonical cell grid (imports the reference's own SGR machine)
  frame2grid.py    termrock `frame` JSON -> the same canonical cell grid
  diff_grid.py     text + color cell diff, budgets, machine-readable report
  diff_png.py      advisory pixel layer (CIEDE2000), needs Pillow, never a gate
  run.py           orchestrator: report.json / report.md / last-report.json, --update-baseline
out/               generated, gitignored
```

## The canonical cell grid

Both sides are normalized into one shape before any comparison:

```json
{"cols": 120, "rows": 40,
 "cells": [{"ch": "▎", "fg": [72,224,84], "bg": [0,0,0],
            "bold": false, "dim": false, "italic": false,
            "underline": false, "reverse": false, "strike": false}]}
```

`ansi2grid.py` produces it from `tmux capture-pane -e -p` output by importing
`State`/`apply` from the reference repo's `tools/ansi2html.py` — the SGR table is
never re-implemented here, so the two repos cannot disagree about what a color
code means. `frame2grid.py` produces it from
`cargo run -p termrock-lookbook -- frame --story <id>` (mapping termrock's
`reversed` field to `reverse`; termrock's `FrameCell` has no italic/strike bits,
and the grid records them as `false` rather than guessing).

## The three layers

| layer | compares | gate |
|---|---|---|
| text  | `ch` per cell, inside the compared region | **hard**, font/OS/terminal independent — this is the CI gate |
| color | `fg`/`bg` RGB + bold/dim/italic/underline/reverse per cell | **hard** (local), catches accent-vs-error, focus bar, hover lift, backdrop dimming |
| pixel | CIEDE2000 per cell on re-rasterized images | **advisory, local only**, auto-skipped without Pillow |

The text layer compares every cell of the compared region; the color layer stops
at each row's *extent* — the last non-blank cell on either side. Beyond that both
sides are background, so comparing further would only measure the two apps'
different canvas colors, which is not widget fidelity. A row where the reference
has content and termrock has none is still measured up to the reference's last
cell, not silently skipped.

The pixel layer is not a gate by construction: the reference rasterizes with
Pillow + FreeType using a system font (9×20 cells, 12 px padding) and termrock
with swash using vendored JetBrains Mono (9×18 cells, no padding). `diff_png.py
--ref-ansi` re-rasterizes the reference at termrock's metrics so the two images
at least line up cell-for-cell, then reports mean/max ΔE and how many cells
exceed the threshold. It still only ever tells you "these glyphs look similar",
never "these are equal".

## Gating model

* `status: "pending-termrock-scene"` in `scenarios.json5` — no termrock story
  maps to this page yet. `run.py` reports **SKIP** and does not gate on it. Ports
  fill these in.
* Otherwise the scenario is **active**: `run.py` renders the story, crops both
  sides and measures the text and color deltas. It **PASS**es when the deltas fit
  the budgets, which come from the scenario's own `tolerance` block when it
  declares one (aspirational, usually `0` — exact parity), and from
  `baselines/<scene>.grid.json` otherwise.
* `baselines/` is a **ratchet**: `--update-baseline` blesses the currently
  measured deltas, and a later run may only stay equal or shrink. This is what
  makes an honest gate possible today — no termrock story reproduces a full junie
  app page byte-for-byte yet, so the harness records how far the port is and
  refuses to let it drift further. When a scenario declares `tolerance`, that
  number wins and the ratchet is ignored.
* An active scenario with no baseline at all is **FAIL** (`unblessed`), not an
  automatic pass. A baseline whose recorded reference digest no longer matches
  the committed `.ansi` is **FAIL** (`stale-baseline`): the reference moved, so
  the delta has to be re-measured. That is the structural fix for the failure
  mode already seen twice in this repo (stale `shots/`, stale `preview-posters/`).

Exit status: nonzero only when a non-pending scenario FAILs. Advisory pixel
results never affect it.

## Exact commands

```sh
JUNIE=/Users/donbeave/Projects/terminal-components-claude
TERMROCK=/Users/donbeave/Projects/tailrocks/termrock
cd "$TERMROCK/verify/junie"

# regenerate the reference side (one-time per junie commit; ~2 s per scene)
bin/ref_capture.sh --all --out reference/scenes

# one scene, by hand
bin/ref_capture.sh --bin showcase --page Tables --cols 120 --rows 40 showcase_tables_120x40
bin/ref_capture.sh --bin tablepro --args '["--connect","Local PostgreSQL"]' tablepro_local_120x40
bin/ref_capture.sh --key '?' tablepro_help_120x40

# compare everything
python3 bin/run.py                                   # text + color + advisory pixel
python3 bin/run.py --layer text                      # text only (what CI runs)
python3 bin/run.py --only showcase_tables_120x40
python3 bin/run.py --list-scenes                     # scene -> status

# bless the current deltas (after reviewing out/*.text.diff)
python3 bin/run.py --update-baseline
```

`run.py` prints a PASS/FAIL/SKIP table and writes `out/report.json`,
`out/report.md` and `last-report.json`. Per-scenario diffs land in
`out/<scene>.text.diff`.

Existing termrock-side gates to run alongside (already wired, no new code):

```sh
cargo nextest run -p termrock-lookbook --all-features --test goldens --locked
cargo nextest run -p termrock-lookbook --all-features --test png_baselines --locked
cd docs && bun run check:preview-posters && bun run check:preview-metrics
```

## Keeping the reference byte-stable

`ref_capture.sh` exists because the reference's own `tools/capture.sh` cannot be
driven as-is:

1. `capture.sh` defaults `BIN` to `target/debug/junie-tui`, which does not exist
   (the crate builds `showcase` and `tablepro`). We always pass the absolute path
   of `target/release/<bin>`.
2. `capture.sh`'s PNG step needs `$PY` from `tools/env.sh`, a scratchpad venv path
   that does not survive. PNGs are advisory and come from `diff_png.py` instead.
3. `capture.sh` sets `default-terminal` **globally** on whatever tmux server it
   finds. We put `bin/shim/tmux` first on `PATH` so every tmux call goes to a
   private `-L jrverify` socket; the user's sessions are untouched.
4. `capture.sh` overwrites the tracked `shots/stderr.log`; we snapshot and restore
   it, and delete the scratch `shots/jr_cap.*`, so the reference working tree is
   byte-clean after a capture. `PYTHONDONTWRITEBYTECODE=1` keeps
   `tools/__pycache__` out of it too.

Env is pinned exactly as upstream does (`env -u NO_COLOR TERM=xterm-256color
COLORTERM=truecolor`), so `ColorLevel::detect()` resolves to `TrueColor` and the
rendered theme is `Theme::junie()`. Geometry is pinned by tmux pane size
(`-x/-y`), never by the binary. `reference/manifest.json` records the junie
commit, tmux version and SHA-256 of every artifact, which is how a stale capture
is told apart from a nondeterministic one.

## How a porting agent uses it

```sh
cd /Users/donbeave/Projects/tailrocks/termrock/verify/junie

# 1. pick a pending scene (a reference page with no termrock story yet)
python3 bin/run.py --list-scenes | grep pending

# 2. look at what has to be reproduced
sed -n '1,40p' reference/scenes/showcase_dialogs_120x40.txt

# 3. implement the termrock side, then declare the scenario active in
#    scenarios.json5: give termrock.story, and a reference.crop when the story
#    covers a region of the page rather than the whole page (the story is then
#    rendered at crop size minus the 1-cell story pad on each side)

# 4. measure
python3 bin/run.py --only showcase_dialogs_120x40
$PAGER out/showcase_dialogs_120x40.text.diff     # per-line/per-cell deltas

# 5. iterate on the widget until the delta stops shrinking, then
python3 bin/run.py --only showcase_dialogs_120x40 --update-baseline
```

Rules of thumb while porting: shrink the delta before beautifying; do not loosen
a budget to make a scenario pass (tighten it instead, with `tolerance`); and if a
scene is genuinely a different design decision rather than a port, say so in the
scenario's `note` instead of leaving it at a large ratchet.

## Known limits

* `tablepro` has no `--page` flag — only `--color` and `--connect` — so its
  screens are reached with `--connect` and `--key`. Multi-word connection names
  survive because `ref_capture.sh` re-quotes the arg vector through `shlex`
  before handing it to `capture.sh`'s unquoted `${ARGS}`.
* The pixel layer needs Pillow, which is not installed in this environment's
  `python3`; `run.py` reports `pixel: skipped: pillow-missing`. Install Pillow
  (or point `$PATH` at an interpreter that has it) to get advisory ΔE numbers.
* Wide glyphs occupy two cells in `ansi2grid.py` (the trailing cell is emitted as
  an empty `ch`), so column math stays honest; the reference UI is otherwise
  narrow-character, and no scene currently triggers a width mismatch.
