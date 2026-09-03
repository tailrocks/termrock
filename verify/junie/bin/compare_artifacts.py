#!/usr/bin/env python3
"""Strictly compare one source/target five-artifact capture pair.

The cell artifacts are compared semantically after parsing. Text and cursor
files are byte exact. PNGs are decoded and compared pixel-for-pixel; a diff PNG
is emitted on the first mismatch. No tolerance or baseline can turn a mismatch
into a pass.

Usage:
  compare_artifacts.py SOURCE_STEM TARGET_STEM --cols N --rows N
"""
import argparse
import binascii
import json
import re
import struct
import sys
import unicodedata
import zlib
from html.parser import HTMLParser
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from ansi2grid import parse_ansi  # noqa: E402

FIELDS = ("ch", "fg", "bg", "bold", "dim", "italic", "underline", "reverse", "strike")
ARTIFACTS = ("ansi", "cursor", "txt", "html", "png")
DEFAULT_FG = [0xD0, 0xD0, 0xD0]
DEFAULT_BG = [0, 0, 0]


def _blank():
    return {
        "ch": " ",
        "fg": DEFAULT_FG[:],
        "bg": DEFAULT_BG[:],
        "bold": False,
        "dim": False,
        "italic": False,
        "underline": False,
        "reverse": False,
        "strike": False,
    }


def _width(ch):
    if unicodedata.combining(ch):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def _color(style, key, default):
    match = re.search(rf"{key}:#([0-9a-fA-F]{{6}})", style)
    return [int(match.group(1)[i : i + 2], 16) for i in (0, 2, 4)] if match else default[:]


def _style(style):
    normalized = re.sub(r"\s+", "", style.lower())
    decoration = " ".join(re.findall(r"text-decoration:([^;]+)", normalized))
    return {
        "fg": _color(normalized, "color", DEFAULT_FG),
        "bg": _color(normalized, "background", DEFAULT_BG),
        "bold": "font-weight:700" in normalized,
        "dim": "opacity:.6" in normalized or "opacity:0.6" in normalized,
        "italic": "font-style:italic" in normalized,
        "underline": "underline" in decoration,
        "reverse": False,
        "strike": "line-through" in decoration,
    }


class _HtmlGrid(HTMLParser):
    def __init__(self, cols, rows):
        super().__init__(convert_charrefs=True)
        self.cols = cols
        self.rows = rows
        self.in_pre = False
        self.in_style = False
        self.pre_seen = False
        self.current = _style("")
        self.grid = [[]]

    def handle_starttag(self, tag, attrs):
        if tag == "pre":
            self.in_pre = True
            self.pre_seen = True
        elif tag == "style":
            self.in_style = True
        elif tag == "span" and self.in_pre:
            self.current = _style(dict(attrs).get("style", ""))

    def handle_endtag(self, tag):
        if tag == "pre":
            self.in_pre = False
        elif tag == "style":
            self.in_style = False
        elif tag == "span":
            self.current = _style("")

    def handle_data(self, data):
        if not self.in_pre or self.in_style:
            return
        for ch in data:
            if ch == "\n":
                self.grid.append([])
                continue
            width = _width(ch)
            if width == 0:
                if self.grid[-1]:
                    self.grid[-1][-1]["ch"] += ch
                continue
            cell = {**self.current, "ch": ch}
            self.grid[-1].append(cell)
            if width == 2:
                self.grid[-1].append({**self.current, "ch": ""})

    def result(self):
        if not self.pre_seen:
            raise ValueError("HTML capture has no <pre> element")
        if len(self.grid) != self.rows:
            raise ValueError(
                f"HTML <pre> has {len(self.grid)} rows, expected {self.rows}"
            )
        oversized = [index for index, row in enumerate(self.grid) if len(row) > self.cols]
        if oversized:
            raise ValueError(
                f"HTML row {oversized[0]} has {len(self.grid[oversized[0]])} cells, "
                f"expected at most {self.cols}"
            )
        cells = []
        for row in self.grid[: self.rows]:
            cells.extend(row[: self.cols])
            cells.extend(_blank() for _ in range(max(0, self.cols - len(row))))
        cells.extend(_blank() for _ in range(max(0, self.cols * self.rows - len(cells))))
        return {"cols": self.cols, "rows": self.rows, "cells": cells[: self.cols * self.rows]}


def html_grid(path, cols, rows):
    parser = _HtmlGrid(cols, rows)
    parser.feed(path.read_text(encoding="utf-8", errors="replace"))
    return parser.result()


def _cell_diff(a, b, cols, rows):
    if a["cols"] != b["cols"] or a["rows"] != b["rows"]:
        return (0, 0, f"geometry {a['cols']}x{a['rows']} != {b['cols']}x{b['rows']}")
    for i, (left, right) in enumerate(zip(a["cells"], b["cells"])):
        for field in FIELDS:
            if left.get(field) != right.get(field):
                return (
                    i % cols,
                    i // cols,
                    f"{field}: {left.get(field)!r} != {right.get(field)!r}",
                )
    return None


def _byte_diff(left, right):
    limit = min(len(left), len(right))
    for offset in range(limit):
        if left[offset] != right[offset]:
            return (
                f"byte {offset}: 0x{left[offset]:02x} != 0x{right[offset]:02x} "
                f"(lengths {len(left)} and {len(right)})"
            )
    if len(left) != len(right):
        return f"length {len(left)} != {len(right)} (first extra byte at {limit})"
    return None


def _normalized_ansi(raw):
    """Normalize only CRLF framing; never discard terminal controls or cells."""
    return raw.replace(b"\r\n", b"\n")


def _chunk(kind, payload):
    data = kind + payload
    return struct.pack(">I", len(payload)) + data + struct.pack(">I", binascii.crc32(data) & 0xFFFFFFFF)


def _png_decode(path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("bad PNG signature")
    pos = 8
    width = height = None
    bit_depth = color_type = None
    compressed = bytearray()
    while pos < len(data):
        if pos + 12 > len(data):
            raise ValueError("truncated PNG chunk header")
        size = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        if size > len(data) - pos - 12:
            raise ValueError(f"truncated PNG chunk {kind!r}")
        payload = data[pos + 8 : pos + 8 + size]
        expected_crc = struct.unpack(">I", data[pos + 8 + size : pos + 12 + size])[0]
        actual_crc = binascii.crc32(kind + payload) & 0xFFFFFFFF
        if expected_crc != actual_crc:
            raise ValueError(f"PNG chunk {kind.decode('ascii', 'replace')} has invalid CRC")
        pos += size + 12
        if kind == b"IHDR":
            width, height, bit_depth, color_type, compression, filt, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
            if (bit_depth, color_type, compression, filt, interlace) not in ((8, 2, 0, 0, 0), (8, 6, 0, 0, 0)):
                raise ValueError("only non-interlaced 8-bit RGB/RGBA PNGs are supported")
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            break
    if width is None or height is None:
        raise ValueError("missing IHDR")
    if not compressed:
        raise ValueError("missing IDAT")
    bpp = 3 if color_type == 2 else 4
    raw = zlib.decompress(bytes(compressed))
    stride = width * bpp
    expected = height * (stride + 1)
    if len(raw) != expected:
        raise ValueError(f"decoded payload is {len(raw)} bytes, expected {expected}")
    rows = []
    previous = bytearray(stride)
    pos = 0
    for _ in range(height):
        filter_type = raw[pos]
        encoded = raw[pos + 1 : pos + 1 + stride]
        pos += stride + 1
        decoded = bytearray(stride)
        for i, value in enumerate(encoded):
            left = decoded[i - bpp] if i >= bpp else 0
            up = previous[i]
            upper_left = previous[i - bpp] if i >= bpp else 0
            if filter_type == 0:
                decoded[i] = value
            elif filter_type == 1:
                decoded[i] = (value + left) & 0xFF
            elif filter_type == 2:
                decoded[i] = (value + up) & 0xFF
            elif filter_type == 3:
                decoded[i] = (value + ((left + up) // 2)) & 0xFF
            elif filter_type == 4:
                p = left + up - upper_left
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - upper_left)
                predictor = left if pa <= pb and pa <= pc else up if pb <= pc else upper_left
                decoded[i] = (value + predictor) & 0xFF
            else:
                raise ValueError(f"unsupported PNG filter {filter_type}")
        rows.append(bytes(decoded))
        previous = decoded
    pixels = b"".join(rows)
    if bpp == 3:
        pixels = b"".join(pixel + b"\xff" for pixel in (pixels[i : i + 3] for i in range(0, len(pixels), 3)))
    return width, height, pixels


def _png_encode(width, height, rgba):
    raw = b"".join(b"\x00" + rgba[y * width * 4 : (y + 1) * width * 4] for y in range(height))
    payload = b"\x89PNG\r\n\x1a\n"
    payload += _chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    payload += _chunk(b"IDAT", zlib.compress(raw, 9))
    return payload + _chunk(b"IEND", b"")


def compare_png(source, target, diff_path):
    sw, sh, sp = _png_decode(source)
    tw, th, tp = _png_decode(target)
    if (sw, sh) != (tw, th):
        return f"dimensions {sw}x{sh} != {tw}x{th}"
    if sp == tp:
        return None
    first = next(i for i, (a, b) in enumerate(zip(sp, tp)) if a != b)
    pixel = first // 4
    x, y = pixel % sw, pixel // sw
    diff = bytearray(sw * sh * 4)
    for i in range(sw * sh):
        same = sp[i * 4 : i * 4 + 4] == tp[i * 4 : i * 4 + 4]
        diff[i * 4 : i * 4 + 4] = b"\x00\x00\x00\xff" if same else b"\xff\x40\x40\xff"
    diff_path.parent.mkdir(parents=True, exist_ok=True)
    diff_path.write_bytes(_png_encode(sw, sh, bytes(diff)))
    return f"first pixel ({x}, {y}) {list(sp[first - first % 4 : first - first % 4 + 4])} != {list(tp[first - first % 4 : first - first % 4 + 4])}; diff={diff_path}"


def compare(source_stem, target_stem, cols, rows, diff_dir):
    failures = []
    for ext in ("txt", "cursor"):
        source = source_stem.with_suffix(f".{ext}")
        target = target_stem.with_suffix(f".{ext}")
        if not source.is_file() or not target.is_file():
            missing = []
            if not source.is_file():
                missing.append(str(source))
            if not target.is_file():
                missing.append(str(target))
            failures.append(f"{ext}: missing required artifact(s): {', '.join(missing)}")
        elif (diff := _byte_diff(source.read_bytes(), target.read_bytes())) is not None:
            failures.append(f"{ext}: {diff}")

    source = source_stem.with_suffix(".ansi")
    target = target_stem.with_suffix(".ansi")
    if not source.is_file() or not target.is_file():
        missing = []
        if not source.is_file():
            missing.append(str(source))
        if not target.is_file():
            missing.append(str(target))
        failures.append(f"ansi: missing required artifact(s): {', '.join(missing)}")
    else:
        if (diff := _byte_diff(_normalized_ansi(source.read_bytes()), _normalized_ansi(target.read_bytes()))) is not None:
            failures.append(f"ansi: raw stream {diff}")
        try:
            left, _ = parse_ansi(source.read_text(encoding="utf-8", errors="replace"), cols, rows)
            right, _ = parse_ansi(target.read_text(encoding="utf-8", errors="replace"), cols, rows)
            grid_left = {"cols": cols, "rows": rows, "cells": left}
            grid_right = {"cols": cols, "rows": rows, "cells": right}
            if (diff := _cell_diff(grid_left, grid_right, cols, rows)) is not None:
                failures.append(f"ansi: cell ({diff[0]},{diff[1]}) {diff[2]}")
        except (OSError, UnicodeError, ValueError, IndexError) as error:
            failures.append(f"ansi: parse failure: {error}")

    source = source_stem.with_suffix(".html")
    target = target_stem.with_suffix(".html")
    if not source.is_file() or not target.is_file():
        missing = []
        if not source.is_file():
            missing.append(str(source))
        if not target.is_file():
            missing.append(str(target))
        failures.append(f"html: missing required artifact(s): {', '.join(missing)}")
    else:
        try:
            if (diff := _cell_diff(html_grid(source, cols, rows), html_grid(target, cols, rows), cols, rows)) is not None:
                failures.append(f"html: cell ({diff[0]},{diff[1]}) {diff[2]}")
        except (OSError, UnicodeError, ValueError, IndexError) as error:
            failures.append(f"html: parse failure: {error}")

    source = source_stem.with_suffix(".png")
    target = target_stem.with_suffix(".png")
    if not source.is_file() or not target.is_file():
        missing = []
        if not source.is_file():
            missing.append(str(source))
        if not target.is_file():
            missing.append(str(target))
        failures.append(f"png: missing required artifact(s): {', '.join(missing)}")
    else:
        try:
            if png_failure := compare_png(source, target, diff_dir / f"{target_stem.name}.diff.png"):
                failures.append(f"png: {png_failure}")
        except (ValueError, zlib.error) as error:
            failures.append(f"png: decode failure: {error}")
    return failures


def _manifest_scenes(path):
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read manifest {path}: {error}") from error
    scenes = manifest.get("scenes") if isinstance(manifest, dict) else None
    if not isinstance(scenes, dict):
        raise ValueError(f"manifest {path} must contain a scenes object")

    result = []
    for scene_id, scene in scenes.items():
        if not isinstance(scene_id, str) or not scene_id:
            raise ValueError(f"manifest {path} contains an invalid scene ID")
        if not isinstance(scene, dict):
            raise ValueError(f"manifest scene {scene_id!r} must be an object")
        cols = scene.get("cols")
        rows = scene.get("rows")
        if (
            isinstance(cols, bool)
            or not isinstance(cols, int)
            or isinstance(rows, bool)
            or not isinstance(rows, int)
            or cols <= 0
            or rows <= 0
        ):
            raise ValueError(
                f"manifest scene {scene_id!r} must have positive integer cols and rows"
            )
        result.append((scene_id, cols, rows))
    return result


def compare_manifest(manifest_path, source_dir, target_dir, diff_dir):
    scenes = _manifest_scenes(manifest_path)
    passed = 0
    failed = 0
    artifact_failures = 0

    for scene_id, cols, rows in scenes:
        source_stem = source_dir / scene_id
        target_stem = target_dir / scene_id
        failures = compare(source_stem, target_stem, cols, rows, diff_dir)
        if failures:
            failed += 1
            artifact_failures += len(failures)
            print(f"FAIL {scene_id} ({cols}x{rows})")
            for failure in failures:
                print(f"- {failure}")
        else:
            passed += 1
            print(f"PASS {scene_id} ({cols}x{rows})")

    print(
        "SUMMARY "
        f"scenes={len(scenes)} "
        f"passed={passed} "
        f"failed={failed} "
        f"artifact_failures={artifact_failures}"
    )
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_stem", type=Path, nargs="?")
    parser.add_argument("target_stem", type=Path, nargs="?")
    parser.add_argument("--cols", type=int)
    parser.add_argument("--rows", type=int)
    parser.add_argument("--diff-dir", type=Path, default=Path("verify/junie/diffs"))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--source-dir", type=Path)
    parser.add_argument("--target-dir", type=Path)
    args = parser.parse_args()

    manifest_mode = any(
        value is not None for value in (args.manifest, args.source_dir, args.target_dir)
    )
    if manifest_mode:
        if args.source_stem is not None or args.target_stem is not None:
            parser.error("manifest mode does not accept positional source/target stems")
        if args.manifest is None or args.source_dir is None or args.target_dir is None:
            parser.error("manifest mode requires --manifest, --source-dir, and --target-dir")
        if args.cols is not None or args.rows is not None:
            parser.error("manifest mode reads cols and rows from the manifest")
        try:
            return compare_manifest(args.manifest, args.source_dir, args.target_dir, args.diff_dir)
        except ValueError as error:
            parser.error(str(error))

    if args.source_stem is None or args.target_stem is None:
        parser.error("one-scene mode requires SOURCE_STEM TARGET_STEM")
    if args.cols is None or args.rows is None:
        parser.error("one-scene mode requires --cols and --rows")
    if args.cols <= 0 or args.rows <= 0:
        parser.error("--cols and --rows must be positive")
    failures = compare(args.source_stem, args.target_stem, args.cols, args.rows, args.diff_dir)
    if failures:
        print("FAIL")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("PASS: txt cursor ansi html png")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
