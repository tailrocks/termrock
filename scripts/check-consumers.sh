#!/usr/bin/env bash
# Builds and runs the four independent consumer fixtures of docs/api-contract.md.
#
# Each fixture is its own workspace with its own committed Cargo.lock. Each runs
# in a separate Cargo invocation with a separate target directory under
# $TERMROCK_CONSUMER_TARGET (default: target/consumers), because a workspace
# `--all-features` build is not evidence of what one consumer resolves.
# Every command is bounded by $TERMROCK_CONSUMER_TIMEOUT seconds (default 240);
# a timeout is a failure, never a skip.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="${TERMROCK_CONSUMER_TARGET:-$root/target/consumers}"
limit="${TERMROCK_CONSUMER_TIMEOUT:-240}"
fixtures=(minimal keyed-modal terminal jackin-host)
mkdir -p "$target"

run() {
  if command -v timeout > /dev/null 2>&1; then
    timeout "${limit}s" "$@"
  elif command -v gtimeout > /dev/null 2>&1; then
    gtimeout "${limit}s" "$@"
  else
    echo "check-consumers: no timeout command; refusing to run unbounded" >&2
    exit 2
  fi
}

for fixture in "${fixtures[@]}"; do
  manifest="$root/crates/termrock/tests/consumers/$fixture/Cargo.toml"
  echo "== $fixture: tests"
  run cargo nextest run --locked --no-tests fail \
    --manifest-path "$manifest" --target-dir "$target/$fixture"
  echo "== $fixture: normal dependency tree"
  run cargo tree --locked -e normal --manifest-path "$manifest" > "$target/$fixture.tree"
  cat "$target/$fixture.tree"
done

forbid() { # fixture package
  if grep -E "(^|[^a-z_-])$2 v" "$target/$1.tree" > /dev/null; then
    echo "check-consumers: $2 leaked into the $1 consumer" >&2
    exit 1
  fi
}
require() { # fixture package
  grep -E "(^|[^a-z_-])$2 v" "$target/$1.tree" > /dev/null \
    || { echo "check-consumers: $2 missing from the $1 consumer" >&2; exit 1; }
}

forbid minimal clap
forbid minimal termpane
forbid keyed-modal clap
forbid keyed-modal termpane
require terminal termpane
forbid terminal clap
require jackin-host termpane
forbid jackin-host clap
echo "check-consumers: ok"
