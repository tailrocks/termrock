#!/usr/bin/env python3
"""Merge one captured scene into reference/manifest.json (internal helper).

Records the exact capture command, the reference commit it came from, tmux
version and content digests — everything needed to tell a stale artifact from a
nondeterministic one.

Usage: _manifest.py MANIFEST OUT_DIR NAME BIN COLS ROWS ARGS_JSON KEYS_JSON MOUSE_JSON
"""
import datetime
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


def git(args, cwd):
    try:
        return subprocess.run(
            ["git", *args], cwd=cwd, capture_output=True, text=True, check=True
        ).stdout.strip()
    except Exception:
        return "unknown"


def tmux_version():
    try:
        return subprocess.run(["tmux", "-V"], capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        return "unknown"


def main():
    manifest_path = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    name, bin_name, cols, rows = sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6]
    args = json.loads(sys.argv[7]) if len(sys.argv) > 7 else []
    keys = json.loads(sys.argv[8]) if len(sys.argv) > 8 else []
    mouse = json.loads(sys.argv[9]) if len(sys.argv) > 9 else []

    junie = os.environ.get("JUNIE_REPO", "/Users/donbeave/Projects/terminal-components-claude")
    entry = {
        "bin": bin_name,
        "args": args,
        "keys": keys,
        "mouse": mouse,
        "cols": int(cols),
        "rows": int(rows),
        "junie_commit": git(["rev-parse", "--short", "HEAD"], junie),
        "junie_dirty": git(["status", "--porcelain"], junie) != "",
        "tmux": tmux_version(),
        "captured_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "sha256": {},
    }
    for ext in ("txt", "ansi"):
        p = out_dir / f"{name}.{ext}"
        entry["sha256"][ext] = hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None

    data = {}
    if manifest_path.exists():
        try:
            data = json.loads(manifest_path.read_text())
        except Exception:
            data = {}
    scenes = data.get("scenes", {})
    scenes[name] = entry
    data["scenes"] = scenes
    data["junie_commit"] = entry["junie_commit"]
    data["tmux"] = entry["tmux"]
    data["updated_at"] = entry["captured_at"]
    manifest_path.write_text(json.dumps(data, indent=2, sort_keys=False) + "\n")


if __name__ == "__main__":
    main()
