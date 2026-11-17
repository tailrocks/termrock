#!/bin/sh
# Nextest setup script `app-bins` (.config/nextest.toml): build the app
# binaries the functional-cross cases launch, and tell the tests where they are.
#
# The binaries go to their own target directory so this default-feature build
# never replaces target/debug/showcase, tablepro or holla. A run with
# `--all-features` builds those with `work-metrics`, and the tests of the root
# package launch them by that path (CARGO_BIN_EXE_<name>). Building into the
# shared directory silently swapped in a build without the feature for any
# partition that also held a functional-cross test.
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
done
