#!/usr/bin/env python3
"""Rewrite `regression_test` paths in the Showcase coverage tables after files move.

The #14 gate checks that every `regression_test: "path/to/file.rs::test_name"`
names a live `#[test]` in that file. About a hundred of the strings point into
`src/` or `tests/`, so every PR of the crate split that moves a file makes some
of them stale. This script reads the renames of that PR and rewrites the paths
in `apps/showcase/src/coverage.rs` and `apps/showcase/src/coverage/lane_*.rs`.
Only the path part before `::` changes; the test name is kept.

The renames come from `git diff -M --name-status`, by default against
`origin/main...HEAD`, or from `--renames FILE` (the same lines, or `old -> new`):

    python3 scripts/rewrite-regression-paths.py --dry
    python3 scripts/rewrite-regression-paths.py
    git diff -M --name-status HEAD | python3 scripts/rewrite-regression-paths.py --renames -

`--dry` lists the candidates and writes nothing. A path whose file was deleted
or no longer exists on disk and has no rename is listed as unresolved, and the
exit status is 1, so a PR cannot leave a stale path behind unnoticed.
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TABLES = ["apps/showcase/src/coverage.rs", "apps/showcase/src/coverage/lane_*.rs"]
REGRESSION = re.compile(r'(regression_test:\s*")([^"]*?\.rs)(::[^"]*")')


def parse_renames(text):
    """`{old: new}` from `git diff -M --name-status` lines or `old -> new` lines.

    A deleted file maps to None, so the caller can say it has no new home.
    """
    renames = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "->" in line:
            old, new = (part.strip() for part in line.split("->", 1))
            renames[old] = new
            continue
        fields = line.split("\t")
        status = fields[0]
        if status.startswith("R") and len(fields) == 3:
            renames[fields[1]] = fields[2]
        elif status == "D" and len(fields) == 2:
            renames[fields[1]] = None
    return renames


def table_files(root):
    files = []
    for pattern in TABLES:
        files.extend(sorted(root.glob(pattern)))
    return files


def rewrite_text(text, renames):
    """The text with every renamed path rewritten, and `(line, old, new)` per change.

    `new` is None for a path whose file was deleted; the text keeps it.
    """
    changes = []

    def replace(match):
        old = match.group(2)
        if old not in renames:
            return match.group(0)
        new = renames[old]
        line = text.count("\n", 0, match.start()) + 1
        changes.append((line, old, new))
        if new is None:
            return match.group(0)
        return match.group(1) + new + match.group(3)

    return REGRESSION.sub(replace, text), changes


def stale_paths(text, root, renames):
    """`(line, path)` of each path that names no file under `root` and has no rename."""
    stale = []
    for match in REGRESSION.finditer(text):
        path = match.group(2)
        if path in renames or (root / path).is_file():
            continue
        stale.append((text.count("\n", 0, match.start()) + 1, path))
    return stale


def git_renames(root, git_range):
    args = ["git", "diff", "-M", "--name-status"]
    if git_range:
        args.append(git_range)
    result = subprocess.run(
        args, cwd=root, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        sys.exit(f"{' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def run(root, renames, dry, out):
    """Rewrite or list; return the number of unresolved paths."""
    unresolved = 0
    rewritten = 0
    for path in table_files(root):
        text = path.read_text()
        new_text, changes = rewrite_text(text, renames)
        shown = path.relative_to(root)
        for line, old, new in changes:
            if new is None:
                unresolved += 1
                print(f"{shown}:{line}: {old} was deleted and has no new path", file=out)
            else:
                rewritten += 1
                verb = "would rewrite" if dry else "rewrote"
                print(f"{shown}:{line}: {verb} {old} -> {new}", file=out)
        for line, old in stale_paths(text, root, renames):
            unresolved += 1
            print(f"{shown}:{line}: {old} names no file and has no rename", file=out)
        if not dry and new_text != text:
            path.write_text(new_text)
    print(f"{rewritten} path(s) {'to rewrite' if dry else 'rewritten'}, {unresolved} unresolved", file=out)
    return unresolved


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--dry", action="store_true", help="list the changes and write nothing")
    parser.add_argument(
        "--renames",
        metavar="FILE",
        help="rename list (`-` for stdin); default is `git diff -M --name-status`",
    )
    parser.add_argument(
        "--range",
        default="origin/main...HEAD",
        help="git range for the default rename list (default %(default)s)",
    )
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root")
    args = parser.parse_args(argv)
    if args.renames == "-":
        text = sys.stdin.read()
    elif args.renames:
        text = Path(args.renames).read_text()
    else:
        text = git_renames(args.root, args.range)
    return 1 if run(args.root, parse_renames(text), args.dry, sys.stdout) else 0


if __name__ == "__main__":
    sys.exit(main())
