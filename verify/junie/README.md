# verify/junie — Junie reference ↔ canonical TermRock catalog

Cross-repository cell-grid verification. The read-only reference is
`/Users/donbeave/Projects/terminal-components-claude` (source binaries
`showcase` and `tablepro`). The target is the single `termrock-catalog`
application. There is no separate target preview entry point.

`reference/scenes/` holds the frozen snapshot of the canonical catalog replay:
five artifacts (`.ansi`, `.cursor`, `.txt`, `.html`, `.png`) for each of the 63
inventored scenarios, exported deterministically by
`termrock-catalog capture`. `reference/manifest.json` records a SHA-256 for
every artifact plus the replay-export provenance; its event arrays are checked
against the Rust replay inventory (`crates/termrock-catalog/src/scenarios.rs`)
by `bin/validate_event_authority.py`. Live source anchoring lives outside this
snapshot: `source-headless/` holds goldens rendered by the source executables
themselves, and `crates/termrock-catalog/tests/parity.rs` compares the catalog
against them page by page.

## Layout

```
scenarios.json5        hand-maintained source-state → catalog-scenario mapping
reference/
  scenes/<scene>.txt   replay plain-text cell grid (frozen snapshot)
  scenes/<scene>.ansi  replay ANSI export (frozen snapshot)
  scenes/<scene>.cursor replay cursor x y visibility (frozen snapshot)
  scenes/<scene>.html  replay deterministic HTML export (frozen snapshot)
  scenes/<scene>.png   replay raster export (frozen snapshot)
  manifest.json        replay-export provenance and artifact digests
source-headless/       live-executable source goldens (<scene>_120x40.txt);
                       rendered by the pinned source binaries, read by
                       tests/parity.rs
baselines/<scene>.grid.json  legacy crop diagnostics only; never used by the strict comparator
bin/
  ansi2grid.py         ANSI export → canonical cell grid
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

`ansi2grid.py` parses ANSI exports using the source ANSI state machine.
`frame2grid.py` parses the independent JSON emitted by the canonical catalog:

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
use a baseline to hide a mismatch. It requires every `.ansi`, `.cursor`,
`.txt`, `.html`, and `.png` on both sides. A pass is byte-exact for
text/cursor and raw-ANSI framing, cell-exact for ANSI/HTML content and styles,
and pixel-exact for decoded PNGs. Because both sides of this comparison come
from the same deterministic replay, the strict gate freezes the snapshot: it
catches unintended render, scenario, or manifest drift, not source divergence.
Source divergence is caught by `tests/parity.rs` against
`source-headless/`.

`diff_png.py` re-rasterizes ANSI at the catalog raster metrics when requested.
It reports visual distance only and never determines the harness exit status.

## Snapshot regeneration and comparison

```sh
# Export the frozen replay snapshot (all 63 scenarios, five artifacts each).
cargo run -q -p termrock-catalog -- capture --out <staging-dir> --all
# Copy the artifacts into reference/scenes/ and update reference/manifest.json
# (recompute SHA-256 per artifact, keep provenance fields honest).

# Compare target artifacts against the frozen snapshot. No baseline or
# tolerance is read.
cargo run -q -p termrock-catalog -- capture --out target_capture --scenario t_100
cargo run -q -p termrock-catalog -- capture --out target_capture --all
python3 bin/compare_artifacts.py --manifest reference/manifest.json \
  --source-dir reference/scenes --target-dir target_capture \
  --diff-dir out/diffs

# Crop diagnostics.
python3 bin/run.py
python3 bin/run.py --layer text
python3 bin/run.py --only showcase_tables_120x40
python3 bin/run.py --list-scenes
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

## Live source anchor

`source-headless/` goldens are rendered by the source repository's own
executables (`showcase`, `tablepro`) built from the pinned commit
`e43cf670`, captured in a 120×40 pane. They are immutable evidence: the parity
gate (`cargo test -p termrock-catalog --test parity`) renders each catalog
page cold and fails on the first differing cell. It does not bless a baseline
or silently select a hybrid source state. Regenerating those goldens is a
deliberate act: rebuild the source at the new pin, re-render every page, and
update the catalog to match before committing both.

The capture environment follows the source capture contract:

```text
TERM=xterm-256color
COLORTERM=truecolor
NO_COLOR unset
```

## Source provenance

`reference/manifest.json` pins the mirrored source generation at `e43cf670`
for the 20-page showcase prefix; the tablepro scenarios mirror later source
states. The checked-in `reference/scenes/` artifacts are deterministic catalog
replay exports (see `scene_provenance` in the manifest), not machine captures;
their event arrays are reconstructions validated against the Rust replay
inventory. The parity gate against `source-headless/` provides the independent
source anchor.

## Porting a pending source state

```sh
cd /Users/donbeave/Projects/tailrocks/termrock/verify/junie

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
source ref consistently and record its SHA. Do not mix files from different
source refs. Scenario filenames may retain the source binary name `showcase`;
that name identifies the reference executable, not a second TermRock preview
application.

When a comparison fails, inspect the source and target independently. Report
the scenario, source and target revisions, dimensions, ordered input events,
first differing cell/value, and the generated diff path. Do not loosen a
tolerance or bless a baseline merely to make the run pass.

## Known limits

* The five TablePro scenes are mapped to the shared application adapter, but
  remain subject to snapshot and layout comparison.
* `run.py` is intentionally a crop diagnostic. `compare_artifacts.py` is the
  zero-tolerance HTML/PNG gate for complete replay/target artifact pairs.
* The pixel helper requires Pillow and a local monospace font. Without Pillow,
  it exits with a diagnostic skip; that does not convert a cell mismatch into a
  pass.
* Wide glyphs occupy two cells in `ansi2grid.py`; the trailing cell is emitted
  with an empty glyph so column math remains explicit.
* The interactive key/mouse steps of the 63 scenarios have no independent
  source anchor; their mirrored page logic is exercised by the replay
  snapshot, while the 20 idle source pages are anchored by the parity gate.
