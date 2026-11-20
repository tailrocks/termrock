#!/bin/sh
# Nextest setup script `settle` (.config/nextest.toml): in CI, wait for the
# `cargo-nextest nextest list` that velnor-ci runs beside the test run to finish
# before the first test starts.
#
# Why. CI runs `cargo-nextest nextest run --archive-file ... --extract-to .
# --extract-overwrite`. When nextest prints "Starting N tests", velnor-ci starts
# a second `cargo-nextest nextest list` (its "early list",
# crates/velnor-ci/src/io/early_list.rs) whose argv still carries the same
# extract flags, so it extracts the whole archive again over the test binaries.
# tar removes and recreates every file (mode 0644, then the exec bit afterwards),
# so a test launched in that window fails with EACCES ("Permission denied",
# nextest: `[double-spawn] failed to exec`), ETXTBSY or ENOENT. That is run
# 44791274299: 44 failures in capsule_input, capsule_panes, capsule_pointer,
# capsule_spawn and clipboard_policy, the first binaries in the archive, in
# archive order, within one second of "Starting". Tracked in
# tailrocks/velnor-new#2402 and #2361.
#
# What. Nextest runs a setup script right after "Starting" and before any test,
# so the list is already starting (the functional jobs never hit this because
# `app-bins` takes over a minute). This script gives it a second to appear, then
# waits until no `nextest list` process is left, for at most two minutes. A
# command line without such a process, as in a local run, costs nothing: the
# script returns at once outside GitHub Actions. No test is skipped, retried or
# changed.
#
# Remove this when velnor-ci stops re-extracting during a run.
set -u

[ -n "${GITHUB_ACTIONS:-}" ] || exit 0

sleep 1
waited=0
while pgrep -f 'nextest list' >/dev/null 2>&1; do
    if [ "$waited" -ge 600 ]; then
        printf '%s\n' 'nextest-settle: a nextest list was still running after 120 s; starting the tests anyway' >&2
        exit 0
    fi
    waited=$((waited + 1))
    sleep 0.2
done
