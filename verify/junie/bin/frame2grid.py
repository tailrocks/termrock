#!/usr/bin/env python3
"""Convert a termrock-catalog `frame` JSON document into the canonical cell grid.

Input is whatever `cargo run -p termrock-catalog -- frame --scenario <id>` prints on
stdout (a single TerminalFrame). Field names are mapped onto the grid vocabulary
used by ansi2grid.py / diff_grid.py — notably `reversed` -> `reverse`.

Usage: frame2grid.py <frame.json|-> <out.grid.json>
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ansi2grid import DEFAULT_BG, DEFAULT_FG, FIELDS  # noqa: E402


def convert(frame):
    cols, rows = int(frame["cols"]), int(frame["rows"])
    cells = frame["cells"]
    if len(cells) != cols * rows:
        raise SystemExit(
            f"frame2grid: {frame.get('story_id')}: {len(cells)} cells for {cols}x{rows}"
        )
    out = []
    for c in cells:
        out.append(
            {
                "ch": c["ch"],
                "fg": list(c["fg"]),
                "bg": list(c["bg"]),
                "bold": bool(c.get("bold", False)),
                "dim": bool(c.get("dim", False)),
                "italic": bool(c.get("italic", False)),
                "underline": bool(c.get("underline", False)),
                "reverse": bool(c.get("reversed", c.get("reverse", False))),
                "strike": bool(c.get("strike", False)),
            }
        )
    return {"cols": cols, "rows": rows, "cells": out}


def blank(cols, rows):
    return {
        "cols": cols,
        "rows": rows,
        "cells": [
            {
                "ch": " ",
                "fg": list(DEFAULT_FG),
                "bg": list(DEFAULT_BG),
                "bold": False,
                "dim": False,
                "italic": False,
                "underline": False,
                "reverse": False,
                "strike": False,
            }
            for _ in range(cols * rows)
        ],
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("src", help="frame JSON path, or '-' for stdin")
    ap.add_argument("dst")
    a = ap.parse_args()
    text = sys.stdin.read() if a.src == "-" else Path(a.src).read_text(encoding="utf-8")
    grid = convert(json.loads(text))
    Path(a.dst).write_text(json.dumps(grid, ensure_ascii=False))
    print(f"{a.dst}: {grid['cols']}x{grid['rows']}")


if __name__ == "__main__":
    main()
