#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
selector=${1:-all}
case "$selector" in
  all|option-like-symlink|relative-git-path) ;;
  *) printf 'usage: %s [all|option-like-symlink|relative-git-path]\n' "$0" >&2; exit 2 ;;
esac

run_option_like_symlink_mutant() {
  local mutant_root
  local mutant_checker
  local output
  mutant_root=$(mktemp -d "${TMPDIR:-/tmp}/workflow-mutant.XXXXXX")
  trap 'find "$mutant_root" -depth -delete' RETURN
  mkdir -p "$mutant_root/scripts"
  cp -p "$repo_root/scripts/check-generated-workflows.sh" "$mutant_root/scripts/check-generated-workflows.sh"
  (cd "$mutant_root" && /usr/bin/patch --quiet -p1 < "$repo_root/tests/mutants/option-like-symlink.patch")
  mutant_checker="$mutant_root/scripts/check-generated-workflows.sh"
  output="$mutant_root/mutant.log"
  if ! CHECKER_UNDER_TEST="$mutant_checker" bash "$repo_root/tests/check-generated-workflows.sh" toolchain > "$output" 2>&1; then
    if ! /usr/bin/grep -F -q 'FAIL option-like symlink cannot replace the same-named caller file' "$output"; then
      cat "$output" >&2
      printf 'generated-workflows: option-like symlink mutant failed outside its target public assertion\n' >&2
      return 1
    fi
  else
    cat "$output" >&2
    printf 'generated-workflows: option-like symlink mutant survived the public checker case\n' >&2
    return 1
  fi
  bash "$repo_root/tests/check-generated-workflows.sh" toolchain
  printf 'option-like symlink public mutant: detected; fixed checker case: PASS\n'
}

run_relative_git_path_mutant() {
  local fixed_output
  local mutant_output
  fixed_output=$(mktemp "${TMPDIR:-/tmp}/workflow-fixed-e2e.XXXXXX")
  mutant_output=$(mktemp "${TMPDIR:-/tmp}/workflow-mutant-e2e.XXXXXX")
  trap 'rm -f "$fixed_output" "$mutant_output"' RETURN
  if ! BASELINE_GUARD_E2E_FOCUS=relative-git-path bash "$repo_root/tests/check-installed-generated-workflow-guard.sh" > "$fixed_output" 2>&1; then
    cat "$fixed_output" >&2
    printf 'generated-workflows: fixed relative Git path public case failed\n' >&2
    return 1
  fi
  if ! BASELINE_GUARD_PUBLIC_MUTANT=relative-git-path BASELINE_GUARD_E2E_FOCUS=relative-git-path bash "$repo_root/tests/check-installed-generated-workflow-guard.sh" > "$mutant_output" 2>&1; then
    cat "$mutant_output" >&2
    printf 'generated-workflows: relative Git path public mutant harness failed\n' >&2
    return 1
  fi
  if ! /usr/bin/grep -F -q 'relative PATH staged executable Git sentinel reaches matching real generator comparison: PASS, not executed' "$fixed_output" || ! /usr/bin/grep -F -q 'relative Git path mutant: DETECTED at staged final-comparison sentinel' "$mutant_output"; then
    cat "$fixed_output" >&2
    cat "$mutant_output" >&2
    printf 'generated-workflows: relative Git path public result did not match the approved green/red behavior\n' >&2
    return 1
  fi
  printf 'relative Git path public mutant: detected at final comparison; fixed public case: PASS\n'
}

case "$selector" in
  all)
    run_option_like_symlink_mutant
    run_relative_git_path_mutant
    ;;
  option-like-symlink) run_option_like_symlink_mutant ;;
  relative-git-path) run_relative_git_path_mutant ;;
esac
