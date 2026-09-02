#!/usr/bin/env python3
"""Cell-level diff of two canonical grids: the text layer and the color layer.

  * text layer  compares `ch` over every cell of the (cropped, fitted) region.
    Hard gate, font/OS/terminal independent. Because both grids are padded to the
    same width, trailing spaces compare equal for free, so this counts real
    content differences — including content present on one side and missing on
    the other.
  * color layer compares fg/bg RGB plus bold/dim/italic/underline/reverse, but
    only up to each row's extent: the last non-blank cell on EITHER side. Beyond
    that, both sides are background and the comparison would only measure the two
    apps' different canvas colors, which is not widget fidelity. A row where the
    reference has content and termrock has none is therefore still measured —
    up to the reference's last cell — not silently skipped.

Exit status: 0 within budget, 1 over budget, 2 usage/IO error.

Usage: diff_grid.py <ref.grid.json> <got.grid.json> [--text-budget N] [--color-budget N]
                    [--out REPORT.json] [--diff-out DIFF.txt]
"""
import argparse
import json
import sys
from pathlib import Path

SAMPLES = 12


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def crop(grid, box):
    """-> sub-grid for [x,y,w,h]; box None -> the grid unchanged."""
    if not box:
        return grid
    x, y, w, h = box
    cols, rows = grid["cols"], grid["rows"]
    cells = []
    for yy in range(y, y + h):
        start = yy * cols + x
        line = grid["cells"][start : start + w] if 0 <= yy < rows and 0 <= x < cols else []
        cells.extend(line)
        cells.extend(_blank() for _ in range(w - len(line)))
    return {"cols": w, "rows": h, "cells": cells}


def _blank():
    return {"ch": " ", "fg": [0, 0, 0], "bg": [0, 0, 0]}


def fit(ref, got):
    """Pad both grids to the same cols/rows (union) so indexes always line up."""
    cols = max(ref["cols"], got["cols"])
    rows = max(ref["rows"], got["rows"])

    def pad(g):
        have = len(g["cells"])
        if g["cols"] == cols and g["rows"] == rows and have == cols * rows:
            return g
        cells = []
        for y in range(rows):
            if y >= g["rows"]:
                cells.extend(_blank() for _ in range(cols))
                continue
            base = y * g["cols"]
            line = g["cells"][base : min(base + g["cols"], have)]
            cells.extend(line)
            cells.extend(_blank() for _ in range(cols - len(line)))
        return {"cols": cols, "rows": rows, "cells": cells}

    return pad(ref), pad(got)


def row_extents(ref, got):
    """Per row, the last cell index that is non-blank on either side (inclusive)."""
    limits = []
    for y in range(ref["rows"]):
        lr = max((i for i, c in enumerate(_row(ref, y)) if c["ch"].strip()), default=-1)
        lg = max((i for i, c in enumerate(_row(got, y)) if c["ch"].strip()), default=-1)
        limits.append(max(lr, lg))
    return limits


def _row(grid, y):
    cols = grid["cols"]
    return grid["cells"][y * cols : (y + 1) * cols]


def diff_text(ref, got):
    differing = 0
    lines = 0
    samples = []
    total = ref["cols"] * ref["rows"]
    detail = []
    for y in range(ref["rows"]):
        r, g = _row(ref, y), _row(got, y)
        row_diffs = []
        for x in range(ref["cols"]):
            a, b = r[x]["ch"], g[x]["ch"]
            if a != b:
                differing += 1
                row_diffs.append(x)
                if len(samples) < SAMPLES:
                    samples.append({"line": y, "col": x, "ref": a, "got": b})
        if row_diffs:
            lines += 1
            lo, hi = row_diffs[0], row_diffs[-1]
            detail.append(
                f"L{y:<3} C{lo}-{hi}  ref {_span(r, lo, hi)}  got {_span(g, lo, hi)}"
            )
    return {
        "cells_differing": differing,
        "cells_total": total,
        "lines_differing": lines,
        "lines_total": ref["rows"],
        "samples": samples,
        "detail": detail,
    }


def _span(row, lo, hi):
    text = "".join(c["ch"] for c in row[lo : hi + 1])
    return repr(text if len(text) <= 34 else text[:31] + "…")


MODS = ("bold", "dim", "italic", "underline", "reverse", "strike")


def diff_color(ref, got):
    limits = row_extents(ref, got)
    considered = 0
    differing = 0
    background_only = 0
    samples = []
    detail = []
    for y in range(ref["rows"]):
        r, g = _row(ref, y), _row(got, y)
        for x in range(ref["cols"]):
            if x > limits[y]:
                break
            considered += 1
            why = []
            if r[x]["fg"] != g[x]["fg"]:
                why.append(f"fg {r[x]['fg']}!={g[x]['fg']}")
            if r[x]["bg"] != g[x]["bg"]:
                why.append(f"bg {r[x]['bg']}!={g[x]['bg']}")
            why += [m for m in MODS if r[x].get(m) != g[x].get(m)]
            if not why:
                continue
            differing += 1
            if r[x]["ch"] == g[x]["ch"] and not r[x]["ch"].strip():
                background_only += 1
            if len(samples) < SAMPLES:
                samples.append(
                    {"line": y, "col": x, "ref": r[x]["ch"], "got": g[x]["ch"], "why": why}
                )
            if len(detail) < 400:
                detail.append(f"L{y:<3} C{x:<4} ref {r[x]['ch']!r} got {g[x]['ch']!r}  {', '.join(why)}")
    return {
        "cells_differing": differing,
        "cells_considered": considered,
        "background_only_differing": background_only,
        "samples": samples,
        "detail": detail,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("ref")
    ap.add_argument("got")
    ap.add_argument("--text-budget", type=int, default=0)
    ap.add_argument("--color-budget", type=int, default=0)
    ap.add_argument("--crop-ref", default=None, help="x,y,w,h")
    ap.add_argument("--crop-got", default=None, help="x,y,w,h")
    ap.add_argument("--out", default=None, help="write JSON report here")
    ap.add_argument("--diff-out", default=None, help="write human-readable diff here")
    a = ap.parse_args()

    ref, got = load(a.ref), load(a.got)
    if a.crop_ref:
        ref = crop(ref, [int(v) for v in a.crop_ref.split(",")])
    if a.crop_got:
        got = crop(got, [int(v) for v in a.crop_got.split(",")])
    ref, got = fit(ref, got)

    t = diff_text(ref, got)
    c = diff_color(ref, got)
    report = {
        "ref": a.ref,
        "got": a.got,
        "geometry": f"{ref['cols']}x{ref['rows']}",
        "text": t,
        "color": c,
        "text_pass": t["cells_differing"] <= a.text_budget,
        "color_pass": c["cells_differing"] <= a.color_budget,
        "pass": t["cells_differing"] <= a.text_budget and c["cells_differing"] <= a.color_budget,
        "budget": {"text_cells": a.text_budget, "color_cells": a.color_budget},
    }
    if a.out:
        Path(a.out).write_text(json.dumps(report, indent=2) + "\n")
    if a.diff_out:
        with open(a.diff_out, "w", encoding="utf-8") as fh:
            fh.write(f"# text layer — {t['cells_differing']}/{t['cells_total']} cells, "
                     f"{t['lines_differing']}/{t['lines_total']} lines\n")
            fh.writelines(l + "\n" for l in t["detail"][:200])
            fh.write(f"\n# color layer — {c['cells_differing']}/{c['cells_considered']} cells "
                     f"({c['background_only_differing']} background-only)\n")
            fh.writelines(l + "\n" for l in c["detail"][:200])

    print(
        f"text: {t['cells_differing']}/{t['cells_total']} cells, "
        f"{t['lines_differing']}/{t['lines_total']} lines (budget {a.text_budget}) "
        f"{'PASS' if report['text_pass'] else 'FAIL'}"
    )
    print(
        f"color: {c['cells_differing']}/{c['cells_considered']} cells "
        f"({c['background_only_differing']} bg-only) (budget {a.color_budget}) "
        f"{'PASS' if report['color_pass'] else 'FAIL'}"
    )
    for l in t["detail"][:8]:
        print("  " + l)
    sys.exit(0 if report["pass"] else 1)


if __name__ == "__main__":
    main()
