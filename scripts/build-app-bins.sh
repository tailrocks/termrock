#!/bin/sh
# Nextest setup script `app-bins` (.config/nextest.toml): build the app
# binaries the functional-cross cases and the root functional journeys launch,
# and tell the tests where they are.
#
# The binaries go to their own target directory so this default-feature build
# never replaces target/debug/showcase, tablepro or holla, which a developer's
# own `--all-features` build may have produced.
#
# `TCB_BIN_<NAME>` is for functional-cross. `TERMROCK_TEST_<NAME>` is for the
# root `functional` target (tests/functional/bins.rs); a value the caller
# already exported, as scripts/run_functional.py does, is left alone.
set -eu

target=${CARGO_TARGET_DIR:-target}
case $target in
/*) ;;
*) target=$PWD/$target ;;
esac
dir=$target/app-bins

cargo build --locked --package termrock-showcase --package termrock-tablepro \
    --package termrock-holla --target-dir "$dir"

: "${NEXTEST_ENV:?run this script as a nextest setup script}"
for name in showcase tablepro holla; do
    upper=$(printf '%s' "$name" | tr '[:lower:]' '[:upper:]')
    printf 'TCB_BIN_%s=%s\n' "$upper" "$dir/debug/$name" >>"$NEXTEST_ENV"
    if [ -z "$(printenv "TERMROCK_TEST_$upper" || true)" ]; then
        printf 'TERMROCK_TEST_%s=%s\n' "$upper" "$dir/debug/$name" >>"$NEXTEST_ENV"
    fi
done
