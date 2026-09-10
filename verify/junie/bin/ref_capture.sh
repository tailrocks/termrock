#!/usr/bin/env bash
# Capture one Junie source-reference scene into verify/junie/reference/scenes/.
# The source executable is `showcase` or `tablepro`; the TermRock target is the
# canonical `termrock-catalog` application and is captured separately.
#
# Thin wrapper around the source repo's tools/capture.sh. It fixes the two
# defects that make an unqualified "run capture.sh" reproduce nothing:
#
#   1. capture.sh defaults BIN to target/debug/junie-tui, a source binary that
#      does not exist (the source crate builds `showcase` and `tablepro`). We
#      always pass BIN.
#   2. capture.sh's PNG step needs $PY from tools/env.sh — a scratchpad venv path
#      that does not survive. PNG is copied when the source capture produced it;
#      a missing source PNG is a capture failure, not a parity success.
#
# It also pins tmux to a private socket (bin/shim/tmux) so the source helper's global
# `default-terminal` mutation inside capture.sh cannot leak into the user's
# sessions. The source checkout is never used as the helper's working directory:
# this script archives the recorded commit into a temporary copy, builds there,
# and removes only that temporary copy on exit.
#
# Usage:
#   bin/ref_capture.sh --bin showcase --page Buttons --cols 120 --rows 40 showcase_buttons_120x40
#   bin/ref_capture.sh --bin tablepro --args '["--connect","Local PostgreSQL"]' tablepro_local_120x40
#   bin/ref_capture.sh --key Tab --key Enter --mouse 'move|60,7' --mouse 'click|60,7' NAME
#   bin/ref_capture.sh --all [--out DIR] [--no-build]     # every scenario in scenarios.json5
#
# Env: JUNIE_REPO (optional canonical source checkout; a sibling checkout is
# discovered when unset). JUNIE_CAPTURE_SOURCE_DIR is an internal inherited
# variable used by --all so each child reuses the same isolated copy.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REFERENCE_MANIFEST="$ROOT/reference/manifest.json"
JUNIE="${JUNIE_REPO:-}"
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
SOURCE_SHA=""
SOURCE_REF=""
ISOLATED="${JUNIE_CAPTURE_SOURCE_DIR:-}"
TEMP_ROOT=""
OWN_TEMP=0

die() { echo "ref_capture: $*" >&2; exit 2; }
command -v python3 >/dev/null 2>&1 || die "python3 is required"

if [ -z "$JUNIE" ]; then
  for candidate in \
    "$ROOT/../../../../terminal-components-claude" \
    "$ROOT/../../../terminal-components-claude"; do
    if [ -d "$candidate/.git" ] && [ -d "$candidate/tools" ]; then
      JUNIE="$(cd "$candidate" && pwd)"
      break
    fi
  done
fi
[ -n "$JUNIE" ] || die "reference repo not found; set JUNIE_REPO to a git checkout"
[ -d "$JUNIE/tools" ] || die "reference repo has no tools directory: $JUNIE"
[ -f "$REFERENCE_MANIFEST" ] || die "recorded source manifest missing: $REFERENCE_MANIFEST"

read_manifest_metadata() {
  python3 - "$REFERENCE_MANIFEST" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    manifest = json.load(handle)
source_sha = manifest.get("source_sha")
source_ref = manifest.get("source_ref", "main")
if not isinstance(source_sha, str) or len(source_sha) != 40:
    raise SystemExit("manifest source_sha must be a full 40-character commit SHA")
print(source_sha)
print(source_ref)
PY
}

SOURCE_METADATA="$(read_manifest_metadata)" || die "cannot read recorded source metadata"
SOURCE_SHA="${SOURCE_METADATA%%$'\n'*}"
SOURCE_REF="${SOURCE_METADATA#*$'\n'}"
[ "$SOURCE_REF" = "$SOURCE_METADATA" ] && SOURCE_REF=main
[ -n "$SOURCE_SHA" ] || die "recorded source SHA is empty"
[ -n "${JUNIE_SOURCE_SHA:-}" ] && [ "$JUNIE_SOURCE_SHA" != "$SOURCE_SHA" ] && \
  die "JUNIE_SOURCE_SHA does not match recorded source SHA $SOURCE_SHA"

prepare_isolated_source() {
  if [ -n "$ISOLATED" ]; then
    [ -f "$ISOLATED/.termrock-source-sha" ] ||
      die "JUNIE_CAPTURE_SOURCE_DIR is not a managed isolated source copy: $ISOLATED"
    [ "$(tr -d '[:space:]' < "$ISOLATED/.termrock-source-sha")" = "$SOURCE_SHA" ] ||
      die "isolated source copy is not pinned to $SOURCE_SHA: $ISOLATED"
  else
    TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/termrock-junie.XXXXXX")"
    ISOLATED="$TEMP_ROOT/source"
    mkdir -p "$ISOLATED"
    OWN_TEMP=1
    git -C "$JUNIE" cat-file -e "$SOURCE_SHA^{commit}" 2>/dev/null ||
      die "recorded source SHA $SOURCE_SHA is not available in $JUNIE"
    git -C "$JUNIE" archive --format=tar "$SOURCE_SHA" | tar -xf - -C "$ISOLATED" ||
      die "could not archive source $SOURCE_SHA from $JUNIE"
    printf '%s\n' "$SOURCE_SHA" > "$ISOLATED/.termrock-source-sha"
  fi
  [ -d "$ISOLATED/tools" ] || die "isolated source copy has no tools directory: $ISOLATED"
  export JUNIE_CAPTURE_SOURCE_DIR="$ISOLATED"
  export JUNIE_REPO="$ISOLATED"
  export JUNIE_SOURCE_SHA="$SOURCE_SHA"
  export JUNIE_SOURCE_REF="$SOURCE_REF"
  export JUNIE_SOURCE_DIRTY=0
  export PY=python3
}

cleanup() {
  if [ -n "$ISOLATED" ] && [ -x "$ISOLATED/tools/capture.sh" ]; then
    "$ISOLATED/tools/capture.sh" stop >/dev/null 2>&1 || true
    rm -f "$ISOLATED/shots/$CAP".ansi "$ISOLATED/shots/$CAP".txt \
      "$ISOLATED/shots/$CAP".cursor "$ISOLATED/shots/$CAP".html \
      "$ISOLATED/shots/$CAP".png
  fi
  if [ "$OWN_TEMP" -eq 1 ] && [ -n "$TEMP_ROOT" ]; then
    rm -rf "$TEMP_ROOT"
  fi
  return 0
}
trap 'cleanup' EXIT INT TERM

while [ $# -gt 0 ]; do
  case "$1" in
    --bin) BIN_NAME="$2"; shift 2 ;;
    --page) PAGE="$2"; shift 2 ;;
    --args) JSON_ARGS="$2"; shift 2 ;;
    --cols) COLS="$2"; shift 2 ;;
    --rows) ROWS="$2"; shift 2 ;;
    --key) KEYS+=("$2"); shift 2 ;;
    --mouse) MOUSE+=("$2"); shift 2 ;;
    --out) OUT="$2"; MANIFEST="$2/manifest.json"; shift 2 ;;
    --scenarios) SCENARIOS="$2"; shift 2 ;;
    --all) ALL=1; shift ;;
    --no-build) BUILD=0; shift ;;
    -h|--help) sed -n '2,29p' "$0"; exit 0 ;;
    -*) die "unknown flag $1" ;;
    *) NAME="$1"; shift ;;
  esac
done

prepare_isolated_source
PATH="$ROOT/bin/shim:$PATH"
export PATH
export PYTHONDONTWRITEBYTECODE=1   # keep tools/__pycache__ out of the reference repo

case "$BIN_NAME" in
  showcase|tablepro) ;;
  "") ;;
  *) die "unsupported source binary: $BIN_NAME (expected showcase or tablepro)" ;;
esac
case "$COLS:$ROWS" in
  *[!0-9:]*|:*) die "columns and rows must be positive integers" ;;
esac
[ "$COLS" -gt 0 ] && [ "$ROWS" -gt 0 ] || die "columns and rows must be positive integers"

if [ "$OUT" = "$ROOT/reference/scenes" ]; then
  die "--out is required; immutable reference/scenes cannot be overwritten by capture"
fi

if [ "$ALL" -eq 1 ]; then
  if [ "$BUILD" -eq 1 ]; then
    echo "building reference (release) ..."
    (cd "$ISOLATED" && cargo build --release --quiet) ||
      die "cargo build --release failed in isolated source copy $ISOLATED"
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
[ -x "$ISOLATED/target/release/$BIN_NAME" ] ||
  die "$ISOLATED/target/release/$BIN_NAME missing (run without --no-build or use --all)"

ARGS="$(python3 -c 'import json,shlex,sys; a=json.loads(sys.argv[1]); print(" ".join(shlex.quote(x) for x in a))' "$JSON_ARGS")"

BIN="$ISOLATED/target/release/$BIN_NAME" ARGS="$ARGS" \
  "$ISOLATED/tools/capture.sh" start "$COLS" "$ROWS" >/dev/null

for k in "${KEYS[@]+"${KEYS[@]}"}"; do "$ISOLATED/tools/capture.sh" keys "$k" >/dev/null; done
for m in "${MOUSE[@]+"${MOUSE[@]}"}"; do
  KIND="${m%%|*}"; XY="${m#*|}"
  "$ISOLATED/tools/capture.sh" mouse "${XY%%,*}" "${XY##*,}" "$KIND" >/dev/null
done

"$ISOLATED/tools/capture.sh" shot "$CAP" >/dev/null
for ext in ansi txt cursor html png; do
  if [ ! -s "$ISOLATED/shots/$CAP.$ext" ]; then
    echo "ref_capture: $NAME: capture produced no $CAP.$ext in isolated source copy" >&2
    exit 3
  fi
  cp "$ISOLATED/shots/$CAP.$ext" "$OUT/$NAME.$ext"
done

KEYS_JSON="$(python3 -c 'import json,sys; sys.stdout.write(json.dumps(sys.argv[1:]))' "${KEYS[@]+"${KEYS[@]}"}")"
MOUSE_JSON="$(python3 -c 'import json,sys; sys.stdout.write(json.dumps(sys.argv[1:]))' "${MOUSE[@]+"${MOUSE[@]}"}")"
python3 "$ROOT/bin/_manifest.py" "$MANIFEST" "$OUT" "$NAME" \
  "$BIN_NAME" "$COLS" "$ROWS" "$JSON_ARGS" "$KEYS_JSON" "$MOUSE_JSON"
echo "captured $NAME (${COLS}x${ROWS})"
