#!/usr/bin/env python3
"""Drive bin/ref_capture.sh once per capture-plan scenario (internal helper).

Kept out of bash deliberately: the plan carries argv that contains spaces and
quotes, and Python's subprocess handles that without a quoting layer.

Usage: _capture_all.py --script ref_capture.sh --out DIR --scenarios scenarios.json5
"""
import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from run import load_scenarios  # noqa: E402


def read_records(stream):
    """Read count-prefixed NUL-separated records emitted by run.py --print-capture-plan."""
    fields = stream.buffer.read().decode().split("\0")
    fields = fields[:-1] if fields and fields[-1] == "" else fields
    i = 0
    while i < len(fields):
        scene, bin_name, cols, rows = fields[i : i + 4]
        i += 4
        groups = []
        for _ in range(3):
            n = int(fields[i])
            i += 1
            groups.append(fields[i : i + n])
            i += n
        yield {
            "scene": scene,
            "bin": bin_name,
            "cols": int(cols),
            "rows": int(rows),
            "args": groups[0],
            "keys": groups[1],
            "mouse": groups[2],
        }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--script", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--scenarios", required=True)
    a = ap.parse_args()

    plans = list(read_records(sys.stdin))
    failures = 0
    for p in plans:
        cmd = [
            a.script, "--bin", p["bin"], "--cols", str(p["cols"]), "--rows", str(p["rows"]),
            "--args", repr_json(p["args"]), "--out", a.out, "--scenarios", a.scenarios,
            "--no-build",
        ]
        for k in p["keys"]:
            cmd += ["--key", k]
        for m in p["mouse"]:
            cmd += ["--mouse", m]
        cmd.append(p["scene"])
        res = subprocess.run(cmd)
        if res.returncode != 0:
            failures += 1
            print(f"_capture_all: {p['scene']} failed with {res.returncode}", file=sys.stderr)
    if failures:
        sys.exit(f"_capture_all: {failures}/{len(plans)} captures failed")


def repr_json(values):
    import json

    return json.dumps(list(values))


if __name__ == "__main__":
    main()
