#!/usr/bin/env python3
"""Advisory pixel layer: CIEDE2000 per cell between two rasterized grids.

This layer is ADVISORY and LOCAL-ONLY. It is never a gate. The two repos
rasterize with different engines (Pillow+FreeType with a system font here,
swash with vendored JetBrains Mono in termrock-raster) and different cell
metrics, so byte or per-pixel equality is unachievable by construction:

    reference ansi2png.py : CW=9 CH=20 SIZE=15 PAD=12  -> (cols*9+24) x (rows*20+24)
    termrock raster       : CW=9 CH=18 SIZE=14 PAD=0   ->  cols*9     x  rows*18

To make the images comparable at all, the reference side is re-rasterized at
TERMROCK's metrics (--ref-ansi mode) so both PNGs are cols*9 x rows*18 and cell
(x,y) maps to the same pixel rectangle on both sides.

Requires Pillow. Without it the script exits 3 with a clear message; run.py
treats that as SKIP, never FAIL.

Usage:
  diff_png.py <a.png> <b.png> [--cols N] [--rows N] [--deltae T] [--out report.json]
  diff_png.py --ref-ansi <scene.ansi> --cols N --rows N --ref-out re-raster.png <a.png>
              [--pair-out pair.png]
"""
import argparse
import json
import math
import sys
from pathlib import Path

# termrock metrics — the only ones that matter, since both images end up at them
CW, CH = 9, 18

try:
    from PIL import Image, ImageDraw, ImageFont  # noqa: F401
except Exception:  # pragma: no cover - depends on the machine
    Image = None


def require_pillow():
    if Image is None:
        sys.stderr.write(
            "diff_png: Pillow is not installed. The pixel layer is advisory and is "
            "skipped (run `python3 -m pip install pillow` to enable it).\n"
        )
        sys.exit(3)


def ciede2000(lab1, lab2):
    """CIEDE2000 color difference (Sharma et al. 2005 formulation)."""
    L1, a1, b1 = lab1
    L2, a2, b2 = lab2
    c1 = math.hypot(a1, b1)
    c2 = math.hypot(a2, b2)
    cbar = (c1 + c2) / 2
    g = 0.5 * (1 - math.sqrt(cbar**7 / (cbar**7 + 25.0**7))) if cbar else 0.0
    ap1, ap2 = (1 + g) * a1, (1 + g) * a2
    cp1, cp2 = math.hypot(ap1, b1), math.hypot(ap2, b2)
    hp1 = math.degrees(math.atan2(b1, ap1)) % 360 if cp1 else 0.0
    hp2 = math.degrees(math.atan2(b2, ap2)) % 360 if cp2 else 0.0
    dL, dC = L2 - L1, cp2 - cp1
    if cp1 * cp2 == 0:
        dh = 0.0
    else:
        dh = hp2 - hp1
        if dh > 180:
            dh -= 360
        elif dh < -180:
            dh += 360
    dH = 2 * math.sqrt(cp1 * cp2) * math.sin(math.radians(dh / 2))
    Lbar, Cbar = (L1 + L2) / 2, (cp1 + cp2) / 2
    if cp1 * cp2 == 0:
        hbar = hp1 + hp2
    else:
        hbar = (hp1 + hp2) / 2
        if abs(hp1 - hp2) > 180:
            hbar += 180 if hp1 + hp2 < 360 else -180
    t = (
        1
        - 0.17 * math.cos(math.radians(hbar - 30))
        + 0.24 * math.cos(math.radians(2 * hbar))
        + 0.32 * math.cos(math.radians(3 * hbar + 6))
        - 0.20 * math.cos(math.radians(4 * hbar - 63))
    )
    sl = 1 + 0.015 * (Lbar - 50) ** 2 / math.sqrt(20 + (Lbar - 50) ** 2)
    sc = 1 + 0.045 * Cbar
    sh = 1 + 0.015 * Cbar * t
    rt = -2 * math.sqrt(Cbar**7 / (Cbar**7 + 25.0**7)) * math.sin(math.radians(60 * math.exp(-((hbar - 275) / 25) ** 2)))
    return math.sqrt((dL / sl) ** 2 + (dC / sc) ** 2 + (dH / sh) ** 2 + rt * (dC / sc) * (dH / sh))


def srgb_to_lab(rgb):
    def f(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (f(c) for c in rgb[:3])
    x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047
    y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883
    e = 216 / 24389
    k = 24389 / 27

    def h(t):
        return t ** (1 / 3) if t > e else (k * t + 16) / 116

    fx, fy, fz = h(x), h(y), h(z)
    return (116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz))


def cell_means(img, cols, rows):
    px = img.convert("RGB")
    means = []
    for y in range(rows):
        for x in range(cols):
            box = px.crop((x * CW, y * CH, (x + 1) * CW, (y + 1) * CH))
            means.append(box.resize((1, 1), Image.BOX).getpixel((0, 0)))
    return means


def compare(a_path, b_path, cols, rows, threshold):
    a, b = Image.open(a_path), Image.open(b_path)
    for name, im in (("a", a), ("b", b)):
        want = (cols * CW, rows * CH)
        if im.size != want:
            sys.stderr.write(f"diff_png: {name} is {im.size[0]}x{im.size[1]}, expected {want[0]}x{want[1]}\n")
            sys.exit(2)
    ma, mb = cell_means(a, cols, rows), cell_means(b, cols, rows)
    deltas = [ciede2000(srgb_to_lab(p), srgb_to_lab(q)) for p, q in zip(ma, mb)]
    off = sum(1 for d in deltas if d > threshold)
    return {
        "cells": len(deltas),
        "mean_deltaE": round(sum(deltas) / len(deltas), 3),
        "max_deltaE": round(max(deltas), 3),
        "cells_over_threshold": off,
        "fraction_off": round(off / len(deltas), 5),
        "threshold": threshold,
        "advisory_pass": (off / len(deltas)) <= 0.01,
    }


def reraster_ansi(ansi_path, cols, rows, out_path, cursor_path=None):
    """Rasterize a reference .ansi at termrock's metrics so a pixel diff is meaningful."""
    require_pillow()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from ansi2grid import parse_ansi

    cells, _ = parse_ansi(Path(ansi_path).read_text(encoding="utf-8", errors="replace"), cols, rows)
    font = _load_font()
    cursor = [int(v) for v in Path(cursor_path).read_text().split()] if cursor_path else [0, 0, 0]
    img = Image.new("RGB", (cols * CW, rows * CH), tuple(cells[0]["bg"]))
    draw = ImageDraw.Draw(img)
    for i, c in enumerate(cells):
        x, y = (i % cols) * CW, (i // cols) * CH
        bg, fg = tuple(c["bg"]), tuple(c["fg"])
        if c.get("reverse"):
            bg, fg = fg, bg
        draw.rectangle([x, y, x + CW - 1, y + CH - 1], fill=bg)
        if c["ch"].strip():
            if c.get("dim"):
                fg = tuple(round(f * 0.6 + b * 0.4) for f, b in zip(fg, bg))
            draw.text((x + 1, y + 1), c["ch"], font=font, fill=fg)
        if c.get("underline"):
            draw.line([x, y + CH - 3, x + CW - 1, y + CH - 3], fill=fg)
    if cursor[2] == 1:
        cx, cy = cursor[0] * CW, cursor[1] * CH
        draw.rectangle([cx, cy, cx + CW - 1, cy + CH - 1], fill=(255, 255, 255))
    img.save(out_path)
    return out_path


def _load_font():
    for cand in (
        Path.home() / "Library/Fonts/JetBrainsMonoNerdFontMono-Regular.ttf",
        Path.home() / "Library/Fonts/JetBrains Mono Nerd Font Mono.ttf",
        Path("/System/Library/Fonts/Menlo.ttc"),
        Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
    ):
        if cand.exists():
            return ImageFont.truetype(str(cand), 14)
    sys.stderr.write("diff_png: no monospace font found for re-rasterization\n")
    sys.exit(3)


def pair_image(a_path, b_path, cols, rows, deltas, threshold, out_path):
    a, b = Image.open(a_path).convert("RGB"), Image.open(b_path).convert("RGB")
    canvas = Image.new("RGB", (a.width * 2 + 8, a.height + 20), (20, 20, 20))
    canvas.paste(a, (0, 20))
    canvas.paste(b, (a.width + 8, 20))
    d = ImageDraw.Draw(canvas)
    for i, delta in enumerate(deltas):
        if delta > threshold:
            x, y = (i % cols) * CW, (i // cols) * CH + 20
            d.rectangle([x, y, x + CW - 1, y + CH - 1], outline=(255, 96, 96))
    d.text((4, 4), f"reference | termrock   (deltaE > {threshold} outlined)", fill=(220, 220, 220))
    canvas.save(out_path)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("a", nargs="?")
    ap.add_argument("b", nargs="?")
    ap.add_argument("--ref-ansi", default=None)
    ap.add_argument("--ref-out", default=None)
    ap.add_argument("--ref-cursor", default=None)
    ap.add_argument("--cols", type=int, default=None)
    ap.add_argument("--rows", type=int, default=None)
    ap.add_argument("--deltae", type=float, default=6.0)
    ap.add_argument("--pair-out", default=None)
    ap.add_argument("--out", default=None)
    a = ap.parse_args()

    if a.ref_ansi:
        require_pillow()
        if not (a.cols and a.rows and a.b):
            sys.exit("diff_png: --ref-ansi needs --cols, --rows and the termrock PNG operand")
        re_raster = a.ref_out or str(Path(a.b).with_name(Path(a.a).stem + ".termrock-metrics.png"))
        reraster_ansi(a.ref_ansi, a.cols, a.rows, re_raster, a.ref_cursor)
        print(f"re-rasterized {a.ref_ansi} at termrock metrics -> {re_raster}")
        a.a = re_raster

    require_pillow()
    if not (a.a and a.b and a.cols and a.rows):
        sys.exit("diff_png: need two PNGs plus --cols/--rows")

    report = compare(a.a, a.b, a.cols, a.rows, a.deltae)
    report.update({"a": a.a, "b": a.b, "advisory_only": True})
    if a.pair_out:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        ma, mb = cell_means(Image.open(a.a), a.cols, a.rows), cell_means(Image.open(a.b), a.cols, a.rows)
        deltas = [ciede2000(srgb_to_lab(p), srgb_to_lab(q)) for p, q in zip(ma, mb)]
        pair_image(a.a, a.b, a.cols, a.rows, deltas, a.deltae, a.pair_out)
        report["pair"] = a.pair_out
    if a.out:
        Path(a.out).write_text(json.dumps(report, indent=2) + "\n")
    print(
        f"pixel (advisory): mean dE {report['mean_deltaE']}, max {report['max_deltaE']}, "
        f"{report['cells_over_threshold']}/{report['cells']} cells over {a.deltae} "
        f"({report['fraction_off']:.2%})"
    )
    sys.exit(0)


if __name__ == "__main__":
    main()
