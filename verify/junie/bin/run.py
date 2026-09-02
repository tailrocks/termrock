#!/usr/bin/env python3
"""Orchestrator for the junie-tui -> termrock fidelity harness.

    python3 bin/run.py                         # compare every scenario, print table
    python3 bin/run.py --only showcase_tables_120x40
    python3 bin/run.py --layer text            # text layer only (what CI runs)
    python3 bin/run.py --update-baseline       # bless current deltas as budgets
    python3 bin/run.py --print-capture-plan    # feed bin/ref_capture.sh --all

Gating model (see research/junie-campaign/verification-infra.md):

  * A scenario with no termrock story yet is `pending-termrock-scene` and is
    reported as SKIP. The harness gates on what exists; ports fill the rest.
  * An active scenario is compared on the text and color cell layers. It passes
    when the measured deltas fit the budgets, which come from the scenario's own
    `tolerance` block when it declares one (aspirational, usually 0) and from
    baselines/<scene>.grid.json otherwise (a ratchet blessed with
    --update-baseline: budgets may only shrink).
  * The pixel layer is advisory. It is skipped unless Pillow and a raster pair
    are present, and never affects the exit status.

Writes verify/junie/out/report.json, verify/junie/out/report.md and
verify/junie/last-report.json. Exits nonzero only on hard failures of
non-pending scenarios.
"""
import argparse
import datetime
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent                       # verify/junie
REPO = ROOT.parents[1]                   # termrock repo root
JUNIE = os.environ.get("JUNIE_REPO", "/Users/donbeave/Projects/terminal-components-claude")

sys.path.insert(0, str(HERE))
import diff_grid  # noqa: E402

LAYERS = ("text", "color", "pixel")


# --------------------------------------------------------------------------- #
# json5 (the subset this file uses: // and /* */ comments, trailing commas)
# --------------------------------------------------------------------------- #
def _strip_json5(text):
    """-> strict JSON. Handles // and /* */ comments, bare keys, trailing commas.

    String literals are protected: each is kept verbatim as its own segment and
    every rewrite below is applied to the code segments only.
    """
    segments = []          # (is_string, text)
    code = []              # accumulate contiguous non-string source
    i, n = 0, len(text)

    def flush():
        if code:
            segments.append((False, "".join(code)))
            del code[:]

    while i < n:
        c = text[i]
        if c == '"':
            flush()
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    break
                j += 1
            segments.append((True, text[i : j + 1]))
            i = j + 1
            continue
        if c == "/" and text[i + 1 : i + 2] == "/":
            k = text.find("\n", i)
            i = n if k < 0 else k
            continue
        if c == "/" and text[i + 1 : i + 2] == "*":
            i = text.find("*/", i) + 2
            continue
        code.append(c)
        i += 1
    flush()

    def fix_code(code):
        code = BARE_KEY.sub(r'"\1"', code)
        code = re.sub(r",(\s*[}\]])", r"\1", code)
        return code

    return "".join(s if is_str else fix_code(s) for is_str, s in segments)


BARE_KEY = re.compile(r"(?<![A-Za-z0-9_\"'\]])([A-Za-z_][A-Za-z0-9_]*)(?=\s*:)")


def load_scenarios(path=None):
    path = Path(path or (ROOT / "scenarios.json5"))
    data = json.loads(_strip_json5(path.read_text(encoding="utf-8")))
    if isinstance(data, dict):
        data = data.get("scenarios", [])
    return data


def git(*args):
    try:
        return subprocess.run(
            ["git", *args], cwd=REPO, capture_output=True, text=True, check=True
        ).stdout.strip()
    except Exception:
        return "unknown"


# --------------------------------------------------------------------------- #
# rendering
# --------------------------------------------------------------------------- #
def sha256(path):
    p = Path(path)
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def reference_grid(scene, cols, rows, cache={}):
    """Parse the committed reference .ansi into a cell grid (memoized per run)."""
    key = (scene, cols, rows)
    if key not in cache:
        src = ROOT / "reference" / "scenes" / f"{scene}.ansi"
        if not src.exists():
            raise FileNotFoundError(f"missing reference artifact {src}")
        from ansi2grid import parse_ansi

        cells, notes = parse_ansi(src.read_text(encoding="utf-8", errors="replace"), cols, rows)
        cache[key] = {"cols": cols, "rows": rows, "cells": cells}
        for note in notes[:3]:
            print(f"  note: {scene}: {note}", file=sys.stderr)
    return cache[key]


def termrock_frame(story, cols, rows, keys, out_dir):
    """Render one lookbook story to a TerminalFrame JSON (cached on disk)."""
    out_dir.mkdir(parents=True, exist_ok=True)
    slug = re.sub(r"[^A-Za-z0-9_.-]", "_", f"{story}_{cols}x{rows}_{'-'.join(keys)}")
    dst = out_dir / f"{slug}.frame.json"
    if not dst.exists():
        cmd = [
            "cargo", "run", "-q", "-p", "termrock-lookbook", "--", "frame",
            "--story", story, "--cols", str(cols), "--rows", str(rows),
        ]
        if keys:
            cmd += ["--keys", ",".join(keys)]
        res = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True)
        if res.returncode != 0:
            raise RuntimeError(f"lookbook frame {story}: {res.stderr.strip()[:400]}")
        dst.write_text(res.stdout)
    import frame2grid

    return frame2grid.convert(json.loads(dst.read_text()))


# --------------------------------------------------------------------------- #
# baselines
# --------------------------------------------------------------------------- #
def baseline_path(scene):
    return ROOT / "baselines" / f"{scene}.grid.json"


def load_baseline(scene):
    p = baseline_path(scene)
    return json.loads(p.read_text()) if p.exists() else None


# --------------------------------------------------------------------------- #
# one scenario
# --------------------------------------------------------------------------- #
def compare_scenario(sc, opts, frames_dir):
    scene = sc["scene"]
    ref_spec = sc.get("reference") or {}
    tm_spec = sc.get("termrock") or {}
    cols, rows = ref_spec.get("cols", 120), ref_spec.get("rows", 40)

    status = sc.get("status") or ("pending-termrock-scene" if not tm_spec.get("story") else "active")
    result = {
        "scene": scene,
        "status": status,
        "geometry": f"{cols}x{rows}",
        "note": sc.get("note", ""),
        "reference_bin": ref_spec.get("bin"),
    }
    if status == "pending-termrock-scene":
        result["skipped"] = f"pending-termrock-scene: no termrock story mapped yet ({tm_spec.get('wanted', 'unspecified')})"
        return result

    try:
        ref = diff_grid.crop(reference_grid(scene, cols, rows, {}), ref_spec.get("crop"))
    except FileNotFoundError as e:
        result.update(status="FAIL", failed="missing-reference-artifacts", detail=str(e))
        return result

    crop_box = ref_spec.get("crop")
    if crop_box:
        w, h = crop_box[2], crop_box[3]
        story_cols, story_rows = w - 2, h - 2          # STORY_PAD = 1 per side
    else:
        story_cols = tm_spec.get("cols", cols)
        story_rows = tm_spec.get("rows", rows)
    try:
        got = diff_grid.crop(
            termrock_frame(
                tm_spec["story"], tm_spec.get("cols", story_cols), tm_spec.get("rows", story_rows),
                tm_spec.get("keys", []), frames_dir,
            ),
            tm_spec.get("crop"),
        )
    except (RuntimeError, KeyError) as e:
        result.update(status="FAIL", failed="termrock-render-error", detail=str(e))
        return result

    if not opts.quiet:
        print(f"  rendered {tm_spec['story']} -> {got['cols']}x{got['rows']} vs reference {ref['cols']}x{ref['rows']}")

    ref, got = diff_grid.fit(ref, got)
    text = diff_grid.diff_text(ref, got)
    color = diff_grid.diff_color(ref, got) if opts.layer_color else None

    result["text"] = {k: text[k] for k in ("cells_differing", "cells_total", "lines_differing", "lines_total")}
    result["color"] = (
        {k: color[k] for k in ("cells_differing", "cells_considered", "background_only_differing")}
        if color else None
    )
    result["diff"] = f"out/{scene}.text.diff"
    _write_diff(scene, text, color)   # always: an unblessed scenario is exactly the one you diff

    # ---- which budgets apply -------------------------------------------------
    # A scenario that declares `tolerance` is aspirational (usually 0 cells): it
    # gates on that number no matter what was blessed. Otherwise the blessed
    # ratchet baseline applies, and an unblessed scenario fails loudly instead of
    # silently passing on an unmeasured delta.
    declared = sc.get("tolerance") or {}
    base = load_baseline(scene)
    budgets = {
        "text_cells": declared.get("text_cells", (base or {}).get("text_cells_budget")),
        "color_cells": declared.get("color_cells", (base or {}).get("color_cells_budget")),
    }
    if base and base.get("reference_ansi_sha256") != sha256(ROOT / "reference" / "scenes" / f"{scene}.ansi"):
        result.update(
            status="FAIL", failed="stale-baseline",
            detail="reference .ansi changed since this baseline was blessed; re-run --update-baseline",
        )
        result["baseline"] = "stale"
        return result

    unresolved = [k for k, v in budgets.items() if v is None]
    if unresolved:
        result.update(
            status="FAIL", failed="unblessed",
            detail=f"no budget for {', '.join(unresolved)}: declare `tolerance` in scenarios.json5 "
                   f"or bless the current delta with --update-baseline",
        )
        result["measured"] = {"text_cells": text["cells_differing"], "color_cells": color["cells_differing"] if color else None}
        return result

    result["budget"] = budgets
    result["diff"] = f"out/{scene}.text.diff"
    _write_diff(scene, text, color)
    hard = text["cells_differing"] <= budgets["text_cells"] and (
        color is None or color["cells_differing"] <= budgets["color_cells"]
    )
    result["exact"] = text["cells_differing"] == 0 and (color is None or color["cells_differing"] == 0)
    result["status"] = "PASS" if hard else "FAIL"
    if not hard:
        result["failed"] = "over-budget"
    return result


def _write_diff(scene, text, color):
    out = ROOT / "out"
    out.mkdir(exist_ok=True)
    with open(out / f"{scene}.text.diff", "w", encoding="utf-8") as fh:
        fh.write(f"# text layer: {text['cells_differing']}/{text['cells_total']} cells, "
                 f"{text['lines_differing']}/{text['lines_total']} lines differ\n")
        fh.writelines(l + "\n" for l in text["detail"][:400])


def maybe_pixel(result, sc, opts):
    """Advisory only. Never changes `status`, never affects the exit code."""
    layers = sc.get("layers") or {}
    # absent == the spec's default: advisory, attempted whenever it can be
    if layers.get("pixel") is False:
        result["pixel"] = {"skipped": "disabled by scenario"}
        return
    if sc.get("status") == "pending-termrock-scene":
        result["pixel"] = {"skipped": "pending-termrock-scene"}
        return
    import diff_png

    if diff_png.Image is None:
        result["pixel"] = {"skipped": "pillow-missing (advisory layer is local-only)"}
        return
    ref_png = ROOT / "reference" / "scenes" / f"{sc['scene']}.png"
    got_png = ROOT / "out" / f"{sc['scene']}.termrock.png"
    if not (ref_png.exists() and got_png.exists()):
        result["pixel"] = {"skipped": "no raster pair (bin/diff_png.py --ref-ansi renders one locally)"}
        return
    if not (opts.cols and opts.rows):
        result["pixel"] = {"skipped": "need --cols/--rows to locate cell rects"}
        return
    result["pixel"] = diff_png.compare(str(ref_png), str(got_png), opts.cols, opts.rows, 6.0)


# --------------------------------------------------------------------------- #
# reporting
# --------------------------------------------------------------------------- #
def write_report(results, opts):
    out = ROOT / "out"
    out.mkdir(exist_ok=True)
    counts = {"PASS": 0, "FAIL": 0, "SKIP": 0}
    for r in results:
        counts["SKIP" if r["status"] == "pending-termrock-scene" else r["status"]] += 1

    report = {
        "junie_commit": json.loads((ROOT / "reference" / "manifest.json").read_text()).get("junie_commit")
        if (ROOT / "reference" / "manifest.json").exists() else "unknown",
        "termrock_commit": git("rev-parse", "--short", "HEAD"),
        "generated_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "layers": {"text": True, "color": opts.layer_color, "pixel": "advisory"},
        "total": len(results),
        "passed": counts["PASS"],
        "failed": counts["FAIL"],
        "skipped": counts["SKIP"],
        "scenarios": results,
    }
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    (ROOT / "last-report.json").write_text(json.dumps(report, indent=2) + "\n")
    (out / "report.md").write_text(render_markdown(report))
    return report


def render_markdown(report):
    lines = [
        "# junie ⇄ termrock fidelity report",
        "",
        f"reference `{report['junie_commit']}` · termrock `{report['termrock_commit']}` · "
        f"{report['generated_at']}",
        "",
        f"{report['total']} scenarios: **{report['passed']} PASS**, {report['failed']} FAIL, "
        f"{report['skipped']} SKIP (pending-termrock-scene)",
        "",
        "| scenario | geom | status | text cells | color cells | budget | note |",
        "|---|---|---|---|---|---|---|",
    ]
    order = {"FAIL": 0, "PASS": 1, "pending-termrock-scene": 2}
    for r in sorted(report["scenarios"], key=lambda r: order.get(r["status"], 3)):
        t = r.get("text") or {}
        c = r.get("color") or {}
        b = r.get("budget") or {}
        budget = f"{b.get('text_cells', '—')}/{b.get('color_cells', '—')}" if b else "—"
        lines.append(
            f"| {r['scene']} | {r['geometry']} | {r['status']} | "
            f"{t.get('cells_differing', '—')} | {c.get('cells_differing', '—') if c else '—'} | "
            f"{budget} | {r.get('failed') or r.get('skipped') or r.get('note', '')} |"
        )
    lines += ["", "text/color columns are *cells differing* inside the compared region; "
              "`budget` is text/color.", ""]
    return "\n".join(lines)


# --------------------------------------------------------------------------- #
# main
# --------------------------------------------------------------------------- #
def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--scenarios", default=str(ROOT / "scenarios.json5"))
    ap.add_argument("--only", action="append", help="scene name (repeatable)")
    ap.add_argument("--layer", action="append", choices=LAYERS, help="restrict layers (repeatable)")
    ap.add_argument("--update-baseline", action="store_true", help="bless measured deltas as budgets")
    ap.add_argument("--print-capture-plan", action="store_true")
    ap.add_argument("--list-scenes", action="store_true")
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument("--cols", type=int)
    ap.add_argument("--rows", type=int)
    opts = ap.parse_args()

    scenarios = load_scenarios(opts.scenarios)
    if opts.only:
        scenarios = [s for s in scenarios if s["scene"] in set(opts.only)]
        missing = set(opts.only) - {s["scene"] for s in scenarios}
        if missing and not scenarios:
            sys.exit(f"run.py: no such scene(s): {', '.join(sorted(missing))}")

    if opts.list_scenes:
        for s in scenarios:
            st = s.get("status") or ("pending-termrock-scene" if not (s.get("termrock") or {}).get("story") else "active")
            print(f"{s['scene']:42} {st}")
        return

    if opts.print_capture_plan:
        # name, bin, cols, rows, args, keys, mouse — NUL-separated, count-prefixed,
        # consumed by bin/_capture_all.py
        for s in scenarios:
            r = s["reference"]
            _emit(s["scene"], r["bin"], r["cols"], r["rows"], r.get("args", []),
                  r.get("keys", []), r.get("mouse", []))
        return

    layers = set(opts.layer) if opts.layer else {"text", "color", "pixel"}
    opts.layer_color = "color" in layers

    frames = ROOT / "out" / "frames"
    results = []
    for sc in scenarios:
        if not opts.quiet:
            print(f"· {sc['scene']}")
        res = compare_scenario(sc, opts, frames)
        if "pixel" in layers:
            maybe_pixel(res, sc, opts)
        if opts.quiet:
            print(f"{res['status']:>4}  {sc['scene']}")
        else:
            print(f"  -> {res['status']}")
        results.append(res)

    report = write_report(results, opts)

    # ---- console table ------------------------------------------------------
    print()
    print(f"{'STATUS':<8}{'SCENARIO':<40}{'TEXT':>10}{'COLOR':>10}  REASON")
    print("-" * 100)
    for r in sorted(results, key=lambda r: {"FAIL": 0, "PASS": 1}.get(r["status"], 2)):
        t = r.get("text") or {}
        c = r.get("color") or {}
        reason = r.get("failed") or r.get("skipped") or ""
        status = "SKIP" if r["status"] == "pending-termrock-scene" else r["status"]
        print(
            f"{status:<8}{r['scene']:<40}"
            f"{(str(t.get('cells_differing', '-')) + '/' + str(t.get('cells_total', '-'))) if t else '-':>10}"
            f"{(str(c.get('cells_differing', '-')) + '/' + str(c.get('cells_considered', '-'))) if c else '-':>10}"
            f"  {reason}"
        )
    print("-" * 100)
    print(
        f"{report['total']} scenarios: {report['passed']} PASS, {report['failed']} FAIL, "
        f"{report['skipped']} SKIP   (report: verify/junie/out/report.json, "
        f"verify/junie/last-report.json)"
    )

    if opts.update_baseline:
        blessed = 0
        for r in results:
            if r.get("status") not in ("PASS", "FAIL"):
                continue
            entry = {
                "scene": r["scene"],
                "text_cells_budget": r["text"]["cells_differing"],
                "color_cells_budget": (r.get("color") or {}).get("cells_differing", 0),
                "measured": {
                    "lines_differing": r["text"]["lines_differing"],
                    "background_only_differing": (r.get("color") or {}).get("background_only_differing", 0),
                },
                "reference_ansi_sha256": sha256(ROOT / "reference" / "scenes" / f"{r['scene']}.ansi"),
                "termrock_commit": git("rev-parse", "--short", "HEAD"),
                "blessed_at": report["generated_at"],
                "note": "ratchet: re-bless only when the delta shrinks or the reference moves",
            }
            p = baseline_path(r["scene"])
            if p.exists():
                old = json.loads(p.read_text())
                entry["text_cells_budget"] = min(entry["text_cells_budget"], old.get("text_cells_budget", 10**9))
                entry["color_cells_budget"] = min(entry["color_cells_budget"], old.get("color_cells_budget", 10**9))
                entry["note"] += "; tightened from %s/%s" % (
                    old.get("text_cells_budget"), old.get("color_cells_budget"))
            p.write_text(json.dumps(entry, indent=2) + "\n")
            blessed += 1
        print(f"blessed {blessed} baseline(s) into verify/junie/baselines/")

    sys.exit(1 if report["failed"] else 0)


def _emit(scene, bin_name, cols, rows, args, keys, mouse):
    """One capture-plan record: name, bin, cols, rows, then n/items per group.

    Fields are NUL-separated and counts are ASCII, so spaces and quotes survive
    without any shell quoting. Consumed by bin/_capture_all.py.
    """
    fields = [scene, bin_name, str(cols), str(rows)]
    for group in (args, keys, mouse):
        fields.append(str(len(group)))
        fields.extend(str(x) for x in group)
    sys.stdout.write("\0".join(fields) + "\0")


if __name__ == "__main__":
    main()
