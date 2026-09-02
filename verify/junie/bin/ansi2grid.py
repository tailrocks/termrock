#!/usr/bin/env python3
"""Parse `tmux capture-pane -e -p` output (SGR truecolor) into a normalized cell grid.

The SGR table is NOT re-implemented here: State/apply are imported from the
reference repo's own tools/ansi2html.py, so the two repos can never disagree
about what a color code means.

Output shape (the canonical grid used by every layer of this harness):

    {"cols": int, "rows": int,
     "cells": [{"ch": str, "fg": [r,g,b], "bg": [r,g,b],
                "bold": bool, "dim": bool, "italic": bool,
                "underline": bool, "reverse": bool, "strike": bool}, ...]}

cells is row-major, exactly cols*rows entries. Every cell carries a resolved
RGB triple — no deferred "default" colors — so the grid is self-contained.

Usage: ansi2grid.py <in.ansi> <out.grid.json> [--cols N] [--rows N]
"""
import argparse
import json
import os
import re
import sys
import unicodedata
from pathlib import Path

# Resolved colors for "terminal default" cells. tmux -e emits explicit SGR for
# every styled cell; unstyled padding gets these. Same defaults ansi2html.py uses.
DEFAULT_FG = [0xD0, 0xD0, 0xD0]
DEFAULT_BG = [0x00, 0x00, 0x00]

SGR = re.compile(r"\x1b\[([0-9;:]*)m")
OTHER = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")

FIELDS = ("ch", "fg", "bg", "bold", "dim", "italic", "underline", "reverse", "strike")


def ref_tools_dir():
    """Locate the reference repo's tools/ (home of ansi2html.py)."""
    candidates = []
    if os.environ.get("JUNIE_REPO"):
        candidates.append(Path(os.environ["JUNIE_REPO"]) / "tools")
    candidates += [
        Path("/Users/donbeave/Projects/terminal-components-claude/tools"),
        Path(__file__).resolve().parents[3] / "terminal-components-claude" / "tools",
    ]
    for c in candidates:
        if (c / "ansi2html.py").is_file():
            return c
    sys.exit(
        "ansi2grid: cannot locate the junie reference repo (need tools/ansi2html.py).\n"
        "Set JUNIE_REPO=/path/to/terminal-components-claude"
    )


sys.path.insert(0, str(ref_tools_dir()))
sys.dont_write_bytecode = True  # do not drop __pycache__ into the reference repo
import ansi2html  # noqa: E402  (reference SGR state machine)


def _rgb(color, which):
    if color is None:
        return list(DEFAULT_FG if which == "fg" else DEFAULT_BG)
    h = color.lstrip("#")
    return [int(h[i : i + 2], 16) for i in (0, 2, 4)]


def _width(ch):
    if unicodedata.combining(ch):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def _cell(ch, state):
    return {
        "ch": ch,
        "fg": _rgb(state.fg, "fg"),
        "bg": _rgb(state.bg, "bg"),
        "bold": bool(state.bold),
        "dim": bool(state.dim),
        "italic": bool(state.italic),
        "underline": bool(state.underline),
        "reverse": bool(state.reverse),
        "strike": bool(state.strike),
    }


def _blank():
    c = {k: v for k, v in zip(FIELDS, (" ", DEFAULT_FG, DEFAULT_BG) + (False,) * 6)}
    return json.loads(json.dumps(c))


def parse_ansi(text, cols, rows):
    """-> (cells row-major cols*rows, notes[])"""
    notes = []
    grid = []
    for lineno, line in enumerate(text.split("\n")[:rows]):
        # strip non-SGR escape sequences exactly like the reference converter does
        line = OTHER.sub(lambda m: m.group(0) if m.group(0).endswith("m") else "", line)
        state = ansi2html.State()
        row = []
        pos = 0
        for m in SGR.finditer(line):
            _emit(row, line[pos : m.start()], state, notes, lineno)
            ansi2html.apply(state, m.group(1))
            pos = m.end()
        _emit(row, line[pos:], state, notes, lineno)
        grid.append(row)

    while len(grid) < rows:
        notes.append(f"row {len(grid)}: absent in stream, filled blank")
        grid.append([_blank() for _ in range(cols)])

    flat = []
    for row in grid:
        flat.extend(row[:cols])
        if len(row) < cols:
            notes.append(f"row {len(flat) // cols - 1}: short by {cols - len(row)}, padded blank")
            flat.extend([_blank() for _ in range(cols - len(row))])
    return flat, notes


def _emit(row, text, state, notes, lineno):
    for ch in text:
        w = _width(ch)
        if w == 0:
            if row:
                row[-1]["ch"] += ch  # combining mark joins its base cell
            continue
        row.append(_cell(ch, state))
        if w == 2:
            # the wide glyph owns the following cell too, so column math stays honest
            row.append(_cell("", state))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--cols", type=int, default=120)
    ap.add_argument("--rows", type=int, default=40)
    a = ap.parse_args()
    text = Path(a.src).read_text(encoding="utf-8", errors="replace")
    cells, notes = parse_ansi(text, a.cols, a.rows)
    grid = {"cols": a.cols, "rows": a.rows, "cells": cells}
    Path(a.dst).write_text(json.dumps(grid, ensure_ascii=False))
    for n in notes[:20]:
        print(f"note: {n}", file=sys.stderr)
    print(f"{a.dst}: {a.cols}x{a.rows} ({len(cells)} cells)")


if __name__ == "__main__":
    main()
