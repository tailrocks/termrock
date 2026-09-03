# verify/junie — Junie reference ↔ canonical TermRock catalog

Cross-repository cell-grid verification. The read-only reference is
`/Users/donbeave/Projects/terminal-components-claude` (source binaries
`showcase` and `tablepro`). The target is the single `termrock-catalog`
application. There is no separate target preview entry point.

The checked-in source evidence uses all five source artifacts
(`.ansi`, `.cursor`, `.txt`, `.html`, `.png`) for every inventoried source shot.
`_manifest.py` records a SHA-256 for each artifact and refuses to write a scene
entry when any artifact is missing or empty. The scenario runner remains a
focused crop diagnostic; strict five-artifact parity uses
`compare_artifacts.py` and never reads a tolerance or baseline.

## Layout

```
scenarios.json5        hand-maintained source-state → catalog-scenario mapping
reference/
  scenes/<scene>.txt   source plain-text cell grid
  scenes/<scene>.ansi  source SGR terminal capture
  scenes/<scene>.cursor source cursor x y visibility
  scenes/<scene>.html  source deterministic HTML export
  scenes/<scene>.png   source raster capture
  manifest.json        source capture provenance and digests
baselines/<scene>.grid.json  legacy crop diagnostics only; never used by the strict comparator
bin/
  ref_capture.sh       source-side tmux capture driver
  _capture_all.py      driver for ref_capture.sh --all
  _manifest.py         capture provenance updater
  ansi2grid.py         source ANSI → canonical cell grid
  frame2grid.py        catalog frame JSON → canonical cell grid
  diff_grid.py         text/color cell comparison
  diff_png.py          raster comparison diagnostic
  run.py               scenario runner and report writer
  target_capture       canonical five-artifact replay output (CLI below)
out/                   generated reports and frames; gitignored
```

## Canonical cell grid

Both sides are normalized to one cell representation:

```json
{"cols": 120, "rows": 40,
 "cells": [{"ch": "▎", "fg": [72,224,84], "bg": [0,0,0],
            "bold": false, "dim": false, "italic": false,
            "underline": false, "reverse": false, "strike": false}]}
```

`ansi2grid.py` parses source `tmux capture-pane -e -p` output using the source
ANSI state machine. `frame2grid.py` parses the independent JSON emitted by the
canonical catalog:

```sh
cargo run -q -p termrock-catalog -- frame --scenario <catalog-scenario-id> \
  --cols 120 --rows 40
```

The catalog frame is the same deterministic rendered state used by the native
application. The comparison layer maps the frame's `reversed` style to
`reverse`; unsupported italic/strike bits remain false rather than being
guessed.

## Gates

| layer | compares | current role |
|---|---|---|
| text | glyph in each compared cell | hard gate |
| color | RGB and supported modifiers | hard gate when enabled |
| pixel | decoded PNG pixels | diagnostic until raster engines are pinned identically |

`run.py` skips a `pending-termrock-scene` entry because no catalog scenario is
mapped yet. Active entries render the canonical catalog scenario, crop both
grids, and report the focused text/color diagnostic. Its crop budgets are not a
source-shot parity claim.

The strict comparator does not turn a pending entry into a pass and does not
use a baseline to hide a mismatch. It requires every `.ansi`, `.cursor`, `.txt`,
`.html`, and `.png` on both sides. A pass is byte-exact for text/cursor and
raw-ANSI framing, cell-exact for ANSI/HTML content and styles, and
pixel-exact for decoded PNGs.

`diff_png.py` re-rasterizes source ANSI at the catalog raster metrics when
requested. Source and target currently use different raster engines, fonts, and
cell metrics, so this helper reports visual distance only and never determines
the harness exit status.

## Capture and comparison

```sh
JUNIE_REPO=/path/to/terminal-components-claude
TERMROCK=/path/to/termrock-presentation
export JUNIE_REPO
cd "$TERMROCK/verify/junie"

# Rebuild and capture all source scenarios from a temporary archive pinned to
# reference/manifest.json:source_sha. The canonical source checkout is read-only.
bin/ref_capture.sh --all --out reference/scenes

# Capture one source state by hand.
bin/ref_capture.sh --bin showcase --page Tables --cols 120 --rows 40 \
  showcase_tables_120x40
bin/ref_capture.sh --bin tablepro --args '["--connect","Local PostgreSQL"]' \
  tablepro_local_120x40
bin/ref_capture.sh --bin tablepro --key '?' tablepro_help_120x40

# Compare active scenarios.
python3 bin/run.py
python3 bin/run.py --layer text
python3 bin/run.py --only showcase_tables_120x40
python3 bin/run.py --list-scenes

# Capture target artifacts from the same app/state used by native and web hosts.
cargo run -q -p termrock-catalog -- capture --out target_capture --scenario t_100
cargo run -q -p termrock-catalog -- capture --out target_capture --all

# Strictly compare a complete five-artifact pair. No baseline or tolerance is read.
python3 bin/compare_artifacts.py \
  reference/scenes/f_overview target_capture/f_overview \
  --cols 120 --rows 40 --diff-dir out/diffs
```

`run.py` writes `out/report.json`, `out/report.md`,
`out/<scene>.text.diff`, and `last-report.json`. It exits nonzero for a hard
failure in an active scenario.

The canonical target checks are:

```sh
cargo test -p termrock-catalog --all-features
cargo nextest run -p termrock-catalog --all-features --locked
cd docs && bun run check:preview-posters && bun run check:preview-metrics
```

Use `termrock-catalog` for all new headless renders and native launch checks;
the target has one preview/catalog gate.

## Reference capture details

`ref_capture.sh` wraps the source repository's `tools/capture.sh` because the
source helper defaults to a binary name that is not built by the source crate.
The wrapper archives the recorded source commit into a temporary isolated copy,
builds and runs the absolute release binary inside that copy, and removes only
the temporary copy after capture. It never writes or deletes in the canonical
source checkout. It uses a private tmux socket, forces `PY=python3` (a PATH
command, not a machine-specific interpreter path), copies all five artifacts,
and fails if any requested artifact is absent.

The environment follows the source capture contract:

```text
TERM=xterm-256color
COLORTERM=truecolor
NO_COLOR unset
```

Pane dimensions come from tmux. Newly regenerated entries record source commit,
tmux version, capture time, dimensions, arguments, input events, and artifact
digests. Existing entries marked `checked-in source shot` intentionally retain
only the immutable source SHA, dimensions, events, and artifact digests because
their original machine capture metadata is unavailable.

## Source provenance

`reference/manifest.json` records `main` at `e43cf67` and the inspected
`jackin` ref. The checked-in `main/shots` artifacts last changed at `4b857a0`;
the executable advanced afterward without shot regeneration. The parity gate
therefore compares independently rendered TermRock output against those
immutable shots and fails on any mismatch. It does not bless a baseline or
silently select a hybrid source state.

## Porting a pending source state

```sh
cd /Users/donbeave/Projects/tailrocks/termrock-presentation/verify/junie

python3 bin/run.py --list-scenes | grep pending
sed -n '1,40p' reference/scenes/showcase_dialogs_120x40.txt

# Inspect a canonical catalog frame.
cargo run -q -p termrock-catalog -- frame --scenario <catalog-scenario-id> \
  --cols 120 --rows 40

# After the reusable catalog page exists, set the scenario's target mapping
# in scenarios.json5 and compare it.
python3 bin/run.py --only showcase_dialogs_120x40
```

Use the actual public TermRock components and the shared catalog state/event
model. Do not add page-local widgets, palettes, shells, or static screenshot
renderers. Resolve missing reusable capabilities in the coordination ledger;
do not mask them in this harness.

## Source-state discipline

The source repository is evidence, not a writable target. Capture the selected
source ref consistently and record its SHA in the capture manifest. Do not mix
files from different source refs. Scenario filenames may retain the source
binary name `showcase`; that name identifies the reference executable, not a
second TermRock preview application.

When a comparison fails, inspect the source and target independently. Report
the scenario, source and target revisions, dimensions, ordered input events,
first differing cell/value, and the generated diff path. Do not loosen a
tolerance or bless a baseline merely to make the run pass.

## Known limits

* The five TablePro scenes are mapped to the shared application adapter, but
  remain subject to source-state and layout comparison.
* `run.py` is intentionally a crop diagnostic. `compare_artifacts.py` is the
  zero-tolerance HTML/PNG gate for complete source/target capture pairs.
* The pixel helper requires Pillow and a local monospace font. Without Pillow,
  it exits with a diagnostic skip; that does not convert a cell mismatch into a
  pass.
* Wide glyphs occupy two cells in `ansi2grid.py`; the trailing cell is emitted
  with an empty glyph so column math remains explicit.
