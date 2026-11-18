#!/bin/sh
# Nextest setup script `app-bins` (.config/nextest.toml): build the app
# binaries the functional-cross cases and the root functional journeys launch,
# and tell the tests where they are.
#
# The binaries must carry the feature set of the run that launches them. The
# test targets compile `termrock::metrics::ENABLED` from `work-metrics`, and
# Showcase's Work counters card shows "compiled out" when the binary lacks it,
# so a default-feature build under `--all-features` fails
# `s06_meters::option_work_counters_card_follows_the_build`.
#
# A setup script is not told the run's features, so they come from, in order:
#   1. `TCB_APP_FEATURES`: `all-features`, a comma list such as `work-metrics`,
#      or empty for default features. Set it when the run's command line is not
#      visible here, as with `cargo nextest run --archive-file`: an archive
#      carries the features it was built with, and nothing here can read them.
#   2. The command line of the nextest process that started this script
#      (`--all-features`, `--features`/`-F`), or `TCB_NEXTEST_ARGS` when set.
#   3. Default features, which is how CI builds the `functional` archive.
# `s06_meters::option_work_counters_card_follows_the_build` fails a mismatch
# with a message that names this script, so a wrong guess is not silent.
#
# Each feature set builds into its own directory, `<target>/app-bins` for the
# default set and `<target>/app-bins-<set>` otherwise, so this build never
# replaces target/debug/showcase, tablepro or holla, which a developer's own
# build may have produced, and a switch between sets never rebuilds the other.
#
# `TCB_BIN_<NAME>` is for functional-cross. `TERMROCK_TEST_<NAME>` is for the
# root `functional` target (crates/termrock-functional/tests/functional/bins.rs). When the caller already
# exported all three `TERMROCK_TEST_<NAME>`, as scripts/run_functional.py does
# after building with the run's features, nothing is built and those paths are
# used for both.
set -eu

# Prints the app features the nextest command line `$1` asks for, as
# `--all-features`, `--features <list>` or nothing. Only `work-metrics` is an
# app feature, so any other `--features` item is not passed on.
requested_features() {
    all=
    list=
    next=
    set -f
    # shellcheck disable=SC2086
    set -- $1
    for word in "$@"; do
        if [ -n "$next" ]; then
            list="$list,$word"
            next=
            continue
        fi
        case $word in
        --) break ;;
        --all-features) all=1 ;;
        --features | -F) next=1 ;;
        --features=*) list="$list,${word#--features=}" ;;
        -F?*) list="$list,${word#-F}" ;;
        esac
    done
    set +f
    if [ -n "$all" ]; then
        printf '%s\n' '--all-features'
        return
    fi
    case ,$(printf '%s' "$list" | tr ' ' ',' | sed 's|[A-Za-z0-9_-]*/||g'), in
    *,work-metrics,*) printf '%s\n' '--features work-metrics' ;;
    esac
}

if [ -n "${TCB_APP_FEATURES+x}" ]; then
    case $TCB_APP_FEATURES in
    '') features= ;;
    all-features) features=--all-features ;;
    *) features=$(requested_features "--features $TCB_APP_FEATURES") ;;
    esac
else
    features=$(requested_features "${TCB_NEXTEST_ARGS-$(ps -o args= -p "$PPID" 2>/dev/null || true)}")
fi

: "${NEXTEST_ENV:?run this script as a nextest setup script}"

target=${CARGO_TARGET_DIR:-target}
case $target in
/*) ;;
*) target=$PWD/$target ;;
esac
case $features in
'') dir=$target/app-bins ;;
--all-features) dir=$target/app-bins-all-features ;;
*) dir=$target/app-bins-work-metrics ;;
esac

prebuilt=1
for name in showcase tablepro holla; do
    upper=$(printf '%s' "$name" | tr '[:lower:]' '[:upper:]')
    [ -n "$(printenv "TERMROCK_TEST_$upper" || true)" ] || prebuilt=
done

if [ -z "$prebuilt" ]; then
    # shellcheck disable=SC2086
    cargo build --locked --package termrock-showcase --package termrock-tablepro \
        --package termrock-holla $features --target-dir "$dir"
fi

for name in showcase tablepro holla; do
    upper=$(printf '%s' "$name" | tr '[:lower:]' '[:upper:]')
    if [ -n "$prebuilt" ]; then
        path=$(printenv "TERMROCK_TEST_$upper")
    else
        path=$dir/debug/$name
        if [ -z "$(printenv "TERMROCK_TEST_$upper" || true)" ]; then
            printf 'TERMROCK_TEST_%s=%s\n' "$upper" "$path" >>"$NEXTEST_ENV"
        fi
    fi
    printf 'TCB_BIN_%s=%s\n' "$upper" "$path" >>"$NEXTEST_ENV"
done
