#!/bin/sh
# Release elapsed-time measurements for #56 (docs/performance-contracts.md,
# "Release measurements"). Builds the ignored `work_measurements` module of the `regressions` target in the
# release profile with the work counters off, runs them one at a time, and
# prints a summary of the reports they wrote to <target dir>/performance.
#
# Usage: scripts/measure-performance.sh
#
# Run it on an otherwise idle machine. It prints the load average before and
# after so a reader can tell how idle that was; it does not decide for you.
# Nothing here compares a time to a threshold: no wall-clock gate exists.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
target=${CARGO_TARGET_DIR:-$root/target}
out=$target/performance

command -v cargo-nextest >/dev/null 2>&1 || {
  echo "cargo-nextest is required (https://nexte.st)" >&2
  exit 2
}

# Reports of an earlier run must not pass for this one's.
mkdir -p "$out"
rm -f "$out"/*.timing.json "$out"/measurement-summary.txt

# Build first, so the load printed below is the load the measurement ran under,
# not the load of the compiler.
cargo nextest list --release -p termrock-conformance --test regressions \
  --profile performance-measure --run-ignored only >/dev/null

dirty=$(git status --porcelain --untracked-files=no 2>/dev/null | wc -l | tr -d ' ')
{
  echo "commit:        $(git rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "tracked files changed vs HEAD: $dirty"
  echo "machine:       $(uname -srm)"
  echo "cpu:           $( (sysctl -n machdep.cpu.brand_string 2>/dev/null \
    || sed -n 's/^model name[[:space:]]*:[[:space:]]*//p' /proc/cpuinfo 2>/dev/null | head -n 1) \
    | head -n 1)"
  echo "rustc:         $(rustc --version)"
  echo "load before:   $(uptime)"
} | tee "$out/measurement-environment.txt"

# --release: timing a debug build means nothing. No --features: the counters
# must be off. test-threads = 1 comes from the profile.
cargo nextest run --release -p termrock-conformance --test regressions \
  --profile performance-measure --run-ignored only --no-tests fail

echo "load after:    $(uptime)" | tee -a "$out/measurement-environment.txt"

{
  printf '%-18s %-28s %9s %12s %12s %12s\n' scenario operation records median_ns p95_ns setup_ns
  for f in "$out"/*.timing.json; do
    awk -F'[:,]' '
      /"scenario"/    { gsub(/[ "]/, "", $2); s = $2 }
      /"operation"/   { gsub(/[ "]/, "", $2); o = $2 }
      /"record_count"/{ gsub(/ /, "", $2); r = $2 }
      /"median_ns"/   { gsub(/ /, "", $2); m = $2 }
      /"p95_ns"/      { gsub(/ /, "", $2); p = $2 }
      /"setup_ns_or_null"/ { gsub(/ /, "", $2); u = $2 }
      END { printf "%-18s %-28s %9s %12s %12s %12s\n", s, o, r, m, p, u }
    ' "$f"
  done | sort
} | tee "$out/measurement-summary.txt"
echo "reports: $out/*.timing.json"
