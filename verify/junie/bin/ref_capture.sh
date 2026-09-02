#!/usr/bin/env bash
# Capture one reference (junie-tui) scene into verify/junie/reference/scenes/.
#
# Thin wrapper around the reference repo's tools/capture.sh. It exists to fix the
# two defects that make an unqualified "run capture.sh" reproduce nothing:
#
#   1. capture.sh defaults BIN to target/debug/junie-tui, a binary that does not
#      exist (the crate builds `showcase` and `tablepro`). We always pass BIN.
#   2. capture.sh's PNG step needs $PY from tools/env.sh — a scratchpad venv path
#      that does not survive. PNGs are advisory-only here and come from
#      bin/diff_png.py when Pillow is available, so $PY is not needed at all.
#
# It also pins tmux to a private socket (bin/shim/tmux) so the global
# `default-terminal` mutation inside capture.sh cannot leak into the user's
# sessions, and restores shots/stderr.log so the reference repo stays byte-clean.
#
# Usage:
#   bin/ref_capture.sh --bin showcase --page Buttons --cols 120 --rows 40 showcase_buttons_120x40
#   bin/ref_capture.sh --bin tablepro --args '["--connect","Local PostgreSQL"]' tablepro_local_120x40
#   bin/ref_capture.sh --key Tab --key Enter --mouse 'move|60,7' --mouse 'click|60,7' NAME
#   bin/ref_capture.sh --all [--out DIR] [--no-build]     # every scenario in scenarios.json5
#
# Env: JUNIE_REPO (default /Users/donbeave/Projects/terminal-components-claude)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JUNIE="${JUNIE_REPO:-/Users/donbeave/Projects/terminal-components-claude}"
OUT="$ROOT/reference/scenes"
MANIFEST="$ROOT/reference/manifest.json"
SCENARIOS="$ROOT/scenarios.json5"
CAP="jr_cap"
BIN_NAME=""
PAGE=""
JSON_ARGS="[]"
COLS=120
ROWS=40
KEYS=()
MOUSE=()
NAME=""
ALL=0
BUILD=1

die() { echo "ref_capture: $*" >&2; exit 2; }
command -v python3 >/dev/null 2>&1 || die "python3 is required"

while [ $# -gt 0 ]; do
  case "$1" in
    --bin) BIN_NAME="$2"; shift 2 ;;
    --page) PAGE="$2"; shift 2 ;;
    --args) JSON_ARGS="$2"; shift 2 ;;
    --cols) COLS="$2"; shift 2 ;;
    --rows) ROWS="$2"; shift 2 ;;
    --key) KEYS+=("$2"); shift 2 ;;
    --mouse) MOUSE+=("$2"); shift 2 ;;
    --out) OUT="$2"; MANIFEST="$(cd "$(dirname "$2")" && pwd)/manifest.json"; shift 2 ;;
    --scenarios) SCENARIOS="$2"; shift 2 ;;
    --all) ALL=1; shift ;;
    --no-build) BUILD=0; shift ;;
    -h|--help) sed -n '2,29p' "$0"; exit 0 ;;
    -*) die "unknown flag $1" ;;
    *) NAME="$1"; shift ;;
  esac
done

[ -d "$JUNIE/tools" ] || die "reference repo not found at $JUNIE (set JUNIE_REPO)"
export JUNIE_REPO="$JUNIE"
export PYTHONDONTWRITEBYTECODE=1   # keep tools/__pycache__ out of the reference repo
PATH="$ROOT/bin/shim:$PATH"

if [ "$ALL" -eq 1 ]; then
  if [ "$BUILD" -eq 1 ]; then
    echo "building reference (release) ..."
    (cd "$JUNIE" && cargo build --release --quiet) || die "cargo build --release failed in $JUNIE"
  fi
  # (no `exec` here: inside a pipeline it only replaces the subshell, and the
  #  script would fall through into single-scene mode)
  python3 "$ROOT/bin/run.py" --print-capture-plan --scenarios "$SCENARIOS" |
    python3 "$ROOT/bin/_capture_all.py" --script "$0" --out "$OUT" --scenarios "$SCENARIOS"
  exit $?
fi

mkdir -p "$OUT"
[ -n "$NAME" ] || die "a scene name is required (or use --all)"
[ -n "$BIN_NAME" ] || die "--bin showcase|tablepro is required"
[ -x "$JUNIE/target/release/$BIN_NAME" ] || die "$JUNIE/target/release/$BIN_NAME missing (build the reference first)"

ARGS="$(python3 -c 'import json,shlex,sys; a=json.loads(sys.argv[1]); print(" ".join(shlex.quote(x) for x in a))' "$JSON_ARGS")"

# capture.sh writes shots/stderr.log; snapshot and restore it so the reference
# working tree stays byte-clean (shots/ is tracked in the reference repo).
ERRLOG="$JUNIE/shots/stderr.log"
ERRBAK=""
[ -f "$ERRLOG" ] && { ERRBAK="$(mktemp)"; cp "$ERRLOG" "$ERRBAK"; }
cleanup() {
  "$JUNIE/tools/capture.sh" stop >/dev/null 2>&1 || true
  rm -f "$JUNIE/shots/$CAP".ansi "$JUNIE/shots/$CAP".txt "$JUNIE/shots/$CAP".cursor \
        "$JUNIE/shots/$CAP".html "$JUNIE/shots/$CAP".png
  if [ -n "$ERRBAK" ]; then cp "$ERRBAK" "$ERRLOG"; rm -f "$ERRBAK"; fi
  return 0
}
trap 'cleanup' EXIT INT TERM

BIN="$JUNIE/target/release/$BIN_NAME" ARGS="$ARGS" \
  "$JUNIE/tools/capture.sh" start "$COLS" "$ROWS" >/dev/null

for k in "${KEYS[@]+"${KEYS[@]}"}"; do "$JUNIE/tools/capture.sh" keys "$k" >/dev/null; done
for m in "${MOUSE[@]+"${MOUSE[@]}"}"; do
  KIND="${m%%|*}"; XY="${m#*|}"
  "$JUNIE/tools/capture.sh" mouse "${XY%%,*}" "${XY##*,}" "$KIND" >/dev/null
done

"$JUNIE/tools/capture.sh" shot "$CAP" >/dev/null
for ext in ansi txt cursor; do
  if [ ! -s "$JUNIE/shots/$CAP.$ext" ]; then
    echo "ref_capture: $NAME: capture produced no $CAP.$ext" >&2
    exit 3
  fi
  cp "$JUNIE/shots/$CAP.$ext" "$OUT/$NAME.$ext"
done

trap '' EXIT INT TERM
cleanup

KEYS_JSON="$(python3 -c 'import json,sys; sys.stdout.write(json.dumps(sys.argv[1:]))' "${KEYS[@]+"${KEYS[@]}"}")"
MOUSE_JSON="$(python3 -c 'import json,sys; sys.stdout.write(json.dumps(sys.argv[1:]))' "${MOUSE[@]+"${MOUSE[@]}"}")"
python3 "$ROOT/bin/_manifest.py" "$MANIFEST" "$OUT" "$NAME" \
  "$BIN_NAME" "$COLS" "$ROWS" "$JSON_ARGS" "$KEYS_JSON" "$MOUSE_JSON"
echo "captured $NAME (${COLS}x${ROWS})"
