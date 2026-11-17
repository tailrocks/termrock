#!/bin/sh
# Per-family and per-app work report for #56 (docs/performance-contracts.md,
# "The per-family report"). Runs every deterministic cost contract with the
# work counters compiled in, then prints one table of the counts those
# contracts recorded, grouped by family or app, and the timing reports of the
# last scripts/measure-performance.sh run beside them when they exist.
#
# Usage: scripts/performance-report.sh [--no-run]
#
#   --no-run   summarize the reports already in <target dir>/performance
#
# The counts are units of work from the contracts' own fixtures, never times.
# Timings come only from measure-performance.sh, which builds without the
# counters; the two kinds are printed in separate sections and never combined.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
target=${CARGO_TARGET_DIR:-$root/target}
out=$target/performance

if [ "${1:-}" != "--no-run" ]; then
  command -v cargo-nextest >/dev/null 2>&1 || {
    echo "cargo-nextest is required (https://nexte.st)" >&2
    exit 2
  }
  # Counts of an earlier run must not pass for this one's.
  mkdir -p "$out"
  find "$out" -name '*.json' ! -name '*.timing.json' -exec rm -f {} +
  # Four hash partitions, as the CI suite runs them.
  for shard in 1 2 3 4; do
    cargo nextest run --profile performance --workspace --all-features \
      --no-tests fail --partition "hash:$shard/4"
  done
fi

[ -d "$out" ] || { echo "no reports in $out" >&2; exit 1; }

python3 -I - "$out" <<'PY'
import json, pathlib, sys

out = pathlib.Path(sys.argv[1])
FAMILY = {
    "viewport": "TextViewport", "viewport_adapter": "TextViewport",
    "scroll_view": "ScrollView", "scroll_panel": "ScrollPanel",
    "list_box": "ListBox", "tree_view": "TreeView",
    "code_editor": "CodeEditor", "text_area": "TextArea",
}
APPS = ("showcase", "tablepro", "holla", "jackin")

def group(scenario):
    for app in APPS:
        if scenario.startswith(app):
            return "app: " + app
    return "family: " + FAMILY.get(scenario, scenario)

counts, timings = [], []
for path in sorted(out.glob("*.json")):
    report = json.loads(path.read_text())
    (timings if path.name.endswith(".timing.json") else counts).append(report)

print("== Work counts (calling thread, instrumentation on) ==")
rows = []
for r in counts:
    work = r.get("work_delta") or {}
    nonzero = ", ".join(f"{k}={v}" for k, v in work.items() if v)
    rows.append((group(r["scenario"]), r["scenario"], r["operation"],
                 r["record_count"], nonzero or "none counted"))
last = None
for g, s, o, n, w in sorted(rows):
    if g != last:
        print(f"\n[{g}]")
        last = g
    print(f"  {s} / {o} / {n}: {w}")

print("\n== Timings (counters off, release; median and p95 in ns) ==")
if not timings:
    print("  none: run scripts/measure-performance.sh first")
for r in sorted(timings, key=lambda r: (group(r["scenario"]), r["scenario"], r["operation"], r["record_count"])):
    print(f"  {group(r['scenario'])}: {r['scenario']} / {r['operation']} / {r['record_count']}: "
          f"median {r['median_ns']}, p95 {r['p95_ns']}, samples {r['samples']}, "
          f"instrumentation_enabled {str(r['instrumentation_enabled']).lower()}")
PY
