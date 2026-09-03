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
import tempfile
from pathlib import Path


ARTIFACTS = ("ansi", "cursor", "txt", "html", "png")


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
    if len(sys.argv) < 7:
        print(
            "usage: _manifest.py MANIFEST OUT_DIR NAME BIN COLS ROWS "
            "[ARGS_JSON] [KEYS_JSON] [MOUSE_JSON]",
            file=sys.stderr,
        )
        return 2

    manifest_path = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    name, bin_name, cols, rows = sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6]
    try:
        args = json.loads(sys.argv[7]) if len(sys.argv) > 7 else []
        keys = json.loads(sys.argv[8]) if len(sys.argv) > 8 else []
        mouse = json.loads(sys.argv[9]) if len(sys.argv) > 9 else []
    except json.JSONDecodeError as error:
        print(f"manifest: invalid event JSON: {error}", file=sys.stderr)
        return 2

    if Path(name).name != name or name in ("", ".", ".."):
        print(f"manifest: invalid scene name: {name!r}", file=sys.stderr)
        return 2

    missing = []
    empty = []
    for ext in ARTIFACTS:
        artifact = out_dir / f"{name}.{ext}"
        if not artifact.is_file():
            missing.append(str(artifact))
        elif artifact.stat().st_size == 0:
            empty.append(str(artifact))
    if missing or empty:
        if missing:
            print(
                "manifest: missing required artifact(s): " + ", ".join(missing),
                file=sys.stderr,
            )
        if empty:
            print(
                "manifest: empty required artifact(s): " + ", ".join(empty),
                file=sys.stderr,
            )
        return 2

    data = {}
    if manifest_path.exists():
        try:
            data = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            print(f"manifest: cannot read {manifest_path}: {error}", file=sys.stderr)
            return 2

    junie = os.environ.get("JUNIE_REPO", "")
    source_sha = os.environ.get("JUNIE_SOURCE_SHA") or data.get("source_sha")
    if not source_sha:
        source_sha = git(["rev-parse", "HEAD"], junie) if junie else "unknown"
    source_ref = os.environ.get("JUNIE_SOURCE_REF") or data.get("source_ref", "main")
    dirty_override = os.environ.get("JUNIE_SOURCE_DIRTY")
    if dirty_override is None:
        dirty = bool(git(["status", "--porcelain"], junie)) if junie else False
    else:
        dirty = dirty_override not in ("", "0", "false", "False", "no", "No")

    if source_sha == "unknown":
        print(
            "manifest: source SHA unavailable; set JUNIE_SOURCE_SHA or JUNIE_REPO",
            file=sys.stderr,
        )
        return 2

    sha256 = {
        ext: hashlib.sha256((out_dir / f"{name}.{ext}").read_bytes()).hexdigest()
        for ext in ARTIFACTS
    }
    captured_at = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    scenes = data.get("scenes", {})
    previous = scenes.get(name, {})
    entry = {
        "bin": bin_name,
        "args": args,
        "cols": int(cols),
        "rows": int(rows),
        "source_ref": source_ref,
        "source_sha": source_sha,
        "junie_commit": source_sha[:7],
        "junie_dirty": dirty,
        "tmux": tmux_version(),
        "captured_at": captured_at,
        "events": previous.get("events", []),
        "evidence": previous.get("evidence", "capture from pinned source commit"),
        "sha256": sha256,
    }
    scenes[name] = entry
    data["scenes"] = scenes
    data["source_ref"] = source_ref
    data["source_sha"] = source_sha
    data["event_authority"] = "reference/manifest.json"
    data["junie_commit"] = entry["junie_commit"]
    data["tmux"] = entry["tmux"]
    data["updated_at"] = captured_at
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            dir=manifest_path.parent,
            prefix=f".{manifest_path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            temporary = Path(handle.name)
            json.dump(data, handle, indent=2, sort_keys=False)
            handle.write("\n")
        os.replace(temporary, manifest_path)
    except OSError as error:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        print(f"manifest: cannot write {manifest_path}: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
