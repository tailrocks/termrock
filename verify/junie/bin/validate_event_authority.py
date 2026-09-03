#!/usr/bin/env python3
"""Validate the source-shot event authority against the Rust replay inventory.

The source capture helper stores rendered artifacts, not an input-event log.
The manifest therefore owns the checked-in replay reconstruction. This gate
ensures that reconstruction is complete, atomic, one-to-one, and identical to
the Rust inventory used by capture replay.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
from pathlib import Path


SOURCE_COUNT = 63
ATOMIC_EVENT_RE = re.compile(r"^(?:.* x[0-9]+|.*(?:\.\.\.|…))$")
STEP_RE = re.compile(r"\bStep::([A-Za-z]+)")
CALL_RE = re.compile(r"\b(?:cat|tp_sql|tp)\s*\(")


class ValidationError(ValueError):
    """Raised when manifest and replay authority disagree."""


def mask_comments(source: str) -> str:
    """Blank Rust comments without changing offsets or string literals."""

    out = list(source)
    i = 0
    quote = None
    while i < len(source):
        if quote:
            if source[i] == "\\":
                i += 2
                continue
            if source[i] == quote:
                quote = None
            i += 1
            continue
        if source[i] in {'"', "'"}:
            quote = source[i]
            i += 1
            continue
        if source.startswith("//", i):
            end = source.find("\n", i)
            end = len(source) if end < 0 else end
            out[i:end] = " " * (end - i)
            i = end
            continue
        if source.startswith("/*", i):
            end = source.find("*/", i + 2)
            end = len(source) if end < 0 else end + 2
            out[i:end] = " " * (end - i)
            i = end
            continue
        i += 1
    return "".join(out)


def matching_paren(source: str, opening: int) -> int:
    depth = 0
    quote = None
    i = opening
    while i < len(source):
        c = source[i]
        if quote:
            if c == "\\":
                i += 2
                continue
            if c == quote:
                quote = None
            i += 1
            continue
        if c in {'"', "'"}:
            quote = c
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValidationError("unclosed Rust call")


def rust_literal(value: str):
    try:
        return ast.literal_eval(value.strip().rstrip(","))
    except (SyntaxError, ValueError) as error:
        raise ValidationError(f"invalid Rust literal {value!r}: {error}") from error


def step_events(call: str) -> list[str]:
    """Convert the Step expressions in one Rust Scenario constructor call."""

    masked = mask_comments(call)
    events = []
    for match in STEP_RE.finditer(masked):
        variant = match.group(1)
        pos = match.end()
        while pos < len(call) and call[pos].isspace():
            pos += 1
        args = None
        if pos < len(call) and call[pos] == "(":
            end = matching_paren(call, pos)
            args = call[pos + 1 : end].strip()
        if variant in {
            "Tab",
            "BackTab",
            "Enter",
            "Esc",
            "Space",
            "Up",
            "Down",
            "Left",
            "Right",
            "Home",
            "End",
            "Backspace",
        }:
            if args is not None:
                raise ValidationError(f"{variant} unexpectedly has arguments")
            events.append(variant)
        elif variant == "Char":
            events.append(str(rust_literal(args)))
        elif variant in {"Ctrl", "Alt"}:
            value = rust_literal(args)
            if variant == "Ctrl" and value == " ":
                events.append("Ctrl-Space")
            else:
                events.append(f"{variant}-{value}")
        elif variant == "Type":
            events.append(f"type {rust_literal(args)}")
        elif variant in {"Move", "Click", "WheelDown", "Resize"}:
            events.append(f"{variant.lower()} {','.join(part.strip() for part in args.split(','))}")
        elif variant == "Ticks":
            events.append(f"ticks:{args.strip()}")
        else:
            raise ValidationError(f"unsupported Rust Step variant: {variant}")
    return events


def rust_inventory(path: Path) -> dict[str, list[str]]:
    source = path.read_text(encoding="utf-8")
    start = source.index("pub static ALL")
    end = source.index("pub static TABLEPRO", start)
    body = source[start:end]
    masked = mask_comments(body)
    inventory = {}
    for match in CALL_RE.finditer(masked):
        opening = masked.find("(", match.start(), match.end())
        closing = matching_paren(masked, opening)
        call = body[opening + 1 : closing]
        id_match = re.search(r'"((?:\\.|[^"\\])*)"', call)
        if not id_match:
            raise ValidationError(f"scenario call at offset {match.start()} has no ID")
        scenario_id = rust_literal(f'"{id_match.group(1)}"')
        if scenario_id in inventory:
            raise ValidationError(f"Rust inventory duplicates {scenario_id!r}")
        inventory[scenario_id] = step_events(call)
    return inventory


def validate(manifest_path: Path, rust_path: Path) -> tuple[int, int]:
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValidationError(f"cannot read manifest {manifest_path}: {error}") from error

    if manifest.get("event_authority") != "reference/manifest.json":
        raise ValidationError("manifest event_authority is not reference/manifest.json")
    scenes = manifest.get("scenes")
    if not isinstance(scenes, dict):
        raise ValidationError("manifest scenes must be an object")
    if len(scenes) != SOURCE_COUNT:
        raise ValidationError(f"manifest has {len(scenes)} scenes; expected {SOURCE_COUNT}")

    rust = rust_inventory(rust_path)
    if len(rust) != SOURCE_COUNT:
        raise ValidationError(f"Rust inventory has {len(rust)} scenes; expected {SOURCE_COUNT}")
    if set(scenes) != set(rust):
        missing = sorted(set(rust) - set(scenes))
        extra = sorted(set(scenes) - set(rust))
        raise ValidationError(f"manifest/Rust IDs differ: missing={missing}, extra={extra}")

    for scenario_id, expected in rust.items():
        events = scenes[scenario_id].get("events")
        if not isinstance(events, list) or not all(isinstance(event, str) for event in events):
            raise ValidationError(f"{scenario_id}: events must be a string array")
        for event in events:
            if ATOMIC_EVENT_RE.match(event):
                raise ValidationError(f"{scenario_id}: lossy event summary {event!r}")
        if events != expected:
            raise ValidationError(
                f"{scenario_id}: manifest events differ from Rust replay: "
                f"manifest={events!r} rust={expected!r}"
            )

    return len(scenes), sum(len(events) for events in rust.values())


def main() -> int:
    root = Path(__file__).resolve().parents[3]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=root / "verify/junie/reference/manifest.json",
    )
    parser.add_argument(
        "--rust",
        type=Path,
        default=root / "crates/termrock-catalog/src/scenarios.rs",
    )
    args = parser.parse_args()
    try:
        scenes, events = validate(args.manifest, args.rust)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(f"event authority valid: {scenes} scenes, {events} ordered events")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
