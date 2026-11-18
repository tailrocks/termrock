#!/usr/bin/env python3
"""Bound Git snapshot blobs with a narrow, content-addressed binary exception."""

from __future__ import annotations

import sys
from pathlib import Path


def validate(
    entries_raw: bytes,
    statuses: list[str],
    allow_missing: bool,
    maximum_blob: int,
    maximum_total: int,
    kind: str,
    allowlist_text: str,
) -> None:
    if entries_raw and not entries_raw.endswith(b"\0"):
        raise ValueError("generated-workflows: snapshot tree listing is incomplete")

    allowed: dict[tuple[str, str], int] = {}
    for number, line in enumerate(allowlist_text.splitlines(), start=1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 3:
            raise ValueError(f"generated-workflows: malformed large-blob allowlist row {number}")
        path, object_id, size_text = fields
        if (
            not path.startswith(("tests/functional/reference-bins/", "crates/termrock-functional/tests/functional/reference-bins/"))
            or ".." in Path(path).parts
            or not size_text.isascii()
            or not size_text.isdigit()
        ):
            raise ValueError(f"generated-workflows: invalid large-blob allowlist row {number}")
        key = (path, object_id)
        if key in allowed:
            raise ValueError(f"generated-workflows: duplicate large-blob allowlist row {number}")
        allowed[key] = int(size_text)

    entries = []
    for entry in entries_raw.split(b"\0")[:-1] if entries_raw else ():
        header, path_bytes = entry.split(b"\t", 1)
        mode, object_kind, object_id = header.decode("ascii").split(" ")
        path = path_bytes.decode("utf-8", "strict")
        if object_kind == "blob" and mode in {"100644", "100755", "120000"}:
            entries.append((path, object_id))
    if len(entries) != len(statuses):
        raise ValueError("generated-workflows: snapshot blob inspection was incomplete")

    total = 0
    for (path, expected_oid), status in zip(entries, statuses, strict=True):
        fields = status.split()
        if len(fields) == 2 and fields == [expected_oid, "missing"]:
            if allow_missing:
                continue
            if kind == "trusted":
                raise ValueError("generated-workflows: trusted seed included blob object is unavailable")
            raise ValueError("generated-workflows: cannot materialize snapshot: snapshot contains an unavailable blob object")
        if len(fields) != 3 or fields[0] != expected_oid or fields[1] != "blob" or not fields[2].isdigit():
            raise ValueError("generated-workflows: snapshot tree contains an unexpected blob object type")
        size = int(fields[2])
        if size > maximum_blob and allowed.get((path, expected_oid)) != size:
            raise ValueError("generated-workflows: cannot materialize snapshot: snapshot blob exceeds the per-object size limit")
        total += size
        if total > maximum_total:
            raise ValueError("generated-workflows: cannot materialize snapshot: snapshot exceeds the total materialized blob size limit")


def main(arguments: list[str]) -> int:
    if len(arguments) != 7:
        raise SystemExit("generated-workflows: snapshot size validator received invalid arguments")
    entries_path, statuses_path, allow_missing, maximum_blob, maximum_total, kind, allowlist_path = arguments
    try:
        validate(
            Path(entries_path).read_bytes(),
            Path(statuses_path).read_text().splitlines(),
            allow_missing == "true",
            int(maximum_blob),
            int(maximum_total),
            kind,
            Path(allowlist_path).read_text(),
        )
    except (OSError, UnicodeError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
