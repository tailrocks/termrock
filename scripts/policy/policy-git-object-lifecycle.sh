#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
reset_policy_fixture() {
  "$git_command" -C "$repository" reset --hard "$source_head" >/dev/null
  "$git_command" -C "$repository" switch --quiet main >/dev/null 2>&1
  "$git_command" -C "$repository" reset --hard "$source_head" >/dev/null
  rm -rf "$repository/policy-nested" "$repository/policy-lock-target"
}

policy_fixture_advance_to_control() {
  local expected_control_refs
  local actual_control_refs
  local receiver_update_status
  if [[ "$source_head" == "$policy_control_head" ]]; then
    return 0
  fi
  if [[ "$source_head" != "$policy_candidate_head" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$policy_candidate_head" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" ]]; then
    printf 'generated-workflows: receiver or source baseline drifted before the isolated empty-control case\n' >&2
    return 1
  fi
  if ! "$git_command" -C "$policy_source_repository" cat-file -e "$policy_candidate_head^{commit}" || ! "$git_command" -C "$policy_source_repository" cat-file -e "$policy_control_head^{commit}" || [[ "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_control_head^{tree}")" != "$policy_control_tree" || "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_control_head^1")" != "$policy_candidate_head" || "$policy_control_tree" != "$policy_candidate_tree" ]]; then
    printf 'generated-workflows: immutable candidate or empty control object has invalid parent or tree identity\n' >&2
    return 1
  fi
  reset_policy_fixture
  if ! "$git_command" -C "$repository" fetch --quiet --depth=1 --no-tags "file://$policy_source_repository" "$policy_control_head"; then
    printf 'generated-workflows: cannot fetch the empty control for the external-edge case\n' >&2
    return 1
  fi
  if ! "$git_command" -C "$repository" cat-file -e "$policy_control_head^{commit}"; then
    printf 'generated-workflows: working repository did not import the empty control object\n' >&2
    return 1
  fi
  "$git_command" -C "$repository" checkout --quiet -B main FETCH_HEAD
  if ! "$git_command" -C "$remote" fetch --quiet --no-tags "file://$policy_source_repository" "$policy_control_head"; then
    printf 'generated-workflows: cannot import the empty control into the private receiver\n' >&2
    return 1
  fi
  if "$git_command" -C "$remote" update-ref refs/heads/main "$policy_control_head" "$policy_candidate_head"; then
    :
  else
    receiver_update_status=$?
    printf 'generated-workflows: cannot switch the private receiver main to the empty external-edge control\n' >&2
    return "$receiver_update_status"
  fi
  expected_control_refs=$(printf '%s\n' "$policy_initial_receiver_refs" | awk -v old="$policy_candidate_head" -v replacement="$policy_control_head" '$2 == "refs/heads/main" && $1 == old { printf "%s %s\n", replacement, $2; found = 1; next } { print } END { if (!found) exit 1 }' | LC_ALL=C sort) || {
    printf 'generated-workflows: candidate receiver refs lack the expected main baseline\n' >&2
    return 1
  }
  actual_control_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
  if [[ "$actual_control_refs" != "$expected_control_refs" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$policy_control_head" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$policy_control_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_control_tree" || -n "$("$git_command" -C "$repository" status --porcelain)" ]]; then
    printf 'generated-workflows: external-edge control isolation changed unrelated refs or checkout state\n' >&2
    return 1
  fi
  source_head=$policy_control_head
  policy_baseline_tree=$policy_control_tree
  policy_initial_receiver_refs=$actual_control_refs
}

policy_lifecycle_hash_file() {
  if [[ "$1" == - ]]; then
    python3 -I -S -c 'import hashlib, sys; print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest())'
  else
    python3 -I -S -c 'import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' "$1"
  fi
}

policy_lifecycle_branch() {
  python3 -I -S - "$git_command" "$1" <<'PY'
import subprocess
import sys

command, root = sys.argv[1:]
result = subprocess.run([command, "-C", root, "symbolic-ref", "-q", "--short", "HEAD"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
if result.returncode == 1 and not result.stdout and not result.stderr:
    raise SystemExit(0)
if result.returncode != 0:
    sys.stderr.buffer.write(result.stderr or b"git symbolic-ref failed without an error message\n")
    raise SystemExit(result.returncode if 0 < result.returncode < 256 else 1)
if result.stderr or result.stdout.count(b"\n") != 1 or not result.stdout.endswith(b"\n"):
    sys.stderr.buffer.write(result.stderr or b"git symbolic-ref returned malformed branch output\n")
    raise SystemExit(1)
try:
    branch_bytes = result.stdout[:-1]
    branch = branch_bytes.decode("utf-8")
except UnicodeDecodeError:
    raise SystemExit("git symbolic-ref returned a branch name that is not valid UTF-8")
if not branch:
    raise SystemExit("git symbolic-ref returned an empty branch name")
validated = subprocess.run([command.encode(), b"check-ref-format", b"--branch", branch_bytes], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
if validated.returncode != 0 or validated.stdout != branch_bytes + b"\n" or validated.stderr:
    sys.stderr.buffer.write(validated.stderr or b"git symbolic-ref returned an invalid branch name\n")
    raise SystemExit(1)
sys.stdout.buffer.write(branch_bytes)
PY
}

policy_lifecycle_source_snapshot() {
  local branch
  branch=$(policy_lifecycle_branch "$1") || return $?
  python3 -I -S - "$1" "$branch" <<'PY'
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
def git(*args):
    return subprocess.check_output(["git", "-C", str(root), *args])
def digest(value):
    return hashlib.sha256(value).hexdigest()
def git_path(value):
    path = Path(os.fsdecode(git("rev-parse", "--path-format=absolute", "--git-path", value).rstrip(b"\n")))
    return path
def inventory(directory):
    if not directory.exists() and not directory.is_symlink():
        return []
    entries = []
    paths = [directory]
    if directory.is_dir() and not directory.is_symlink():
        paths.extend(sorted(directory.rglob("*")))
    for path in paths:
        metadata = path.lstat()
        item = {"path": os.fsdecode(path.relative_to(directory.parent)), "mode": stat.S_IMODE(metadata.st_mode)}
        if stat.S_ISREG(metadata.st_mode):
            item["sha256"] = digest(path.read_bytes())
        elif stat.S_ISLNK(metadata.st_mode):
            item["target"] = os.readlink(path)
        elif stat.S_ISDIR(metadata.st_mode):
            item["kind"] = "directory"
        else:
            item["kind"] = "other"
        entries.append(item)
    return entries
index_path = git_path("index")
config_path = Path(os.fsdecode(git("rev-parse", "--path-format=absolute", "--git-common-dir").rstrip(b"\n"))) / "config"
hooks_path = git_path("hooks")
untracked = git("ls-files", "--others", "--exclude-standard", "-z").split(b"\0")
untracked_entries = []
for raw in untracked:
    if not raw:
        continue
    path = root / os.fsdecode(raw)
    metadata = path.lstat()
    item = {"path": os.fsdecode(raw), "mode": stat.S_IMODE(metadata.st_mode)}
    if stat.S_ISREG(metadata.st_mode):
        item["sha256"] = digest(path.read_bytes())
    elif stat.S_ISLNK(metadata.st_mode):
        item["target"] = os.readlink(path)
    else:
        item["kind"] = "other"
    untracked_entries.append(item)
sentinel = root / "scripts/trusted-seed-cargo.lock"
sentinel_info = None
if sentinel.exists() and not sentinel.is_symlink():
    sentinel_info = {"mode": stat.S_IMODE(sentinel.stat().st_mode), "sha256": digest(sentinel.read_bytes())}
snapshot = {
    "head": git("rev-parse", "HEAD").decode().strip(),
    "branch": sys.argv[2],
    "tree": git("write-tree").decode().strip(),
    "refs": git("show-ref", "--head").decode().splitlines(),
    "status_sha256": digest(git("status", "--porcelain=v1", "-z")),
    "refs_sha256": digest(git("show-ref", "--head")),
    "index_sha256": digest(index_path.read_bytes()),
    "unstaged_diff_sha256": digest(git("diff", "--binary")),
    "staged_diff_sha256": digest(git("diff", "--cached", "--binary")),
    "config_sha256": digest(config_path.read_bytes()),
    "hooks": inventory(hooks_path),
    "untracked": untracked_entries,
    "seed_sentinel": sentinel_info,
}
print(json.dumps(snapshot, sort_keys=True, separators=(",", ":")))
PY
}

policy_lifecycle_prepare_git_fixture() {
  local fixture_name=$1
  repository="$test_root/$fixture_name-working"
  remote="$test_root/$fixture_name-receiver.git"
  mkdir -p "$repository"
  "$git_command" init --quiet --bare "$remote"
  "$git_command" -C "$remote" fetch --quiet "file://$policy_source_repository" "$policy_candidate_head"
  "$git_command" -C "$remote" update-ref refs/heads/main "$policy_candidate_head"
  "$git_command" init --quiet --initial-branch=main "$repository"
  "$git_command" -C "$repository" config user.name 'Generated workflow qualification'
  "$git_command" -C "$repository" config user.email 'generated-workflow@example.invalid'
  "$git_command" -C "$repository" remote add origin "$remote"
  "$git_command" -C "$repository" fetch --quiet --depth=1 --no-tags "file://$policy_source_repository" "$policy_candidate_head"
  "$git_command" -C "$repository" checkout --quiet -B main FETCH_HEAD
  "$git_command" -C "$repository" fetch --quiet --depth=2 --no-tags "file://$policy_source_repository" "$policy_local_fixture_head"
  "$git_command" -C "$repository" switch --quiet -c policy-positive-positive-local-local-removal FETCH_HEAD
  "$git_command" -C "$repository" push --quiet origin policy-positive-positive-local-local-removal:refs/heads/policy-positive-positive-local-local-removal
  "$git_command" -C "$repository" switch --quiet main
  source_head=$policy_candidate_head
  policy_baseline_tree=$policy_candidate_tree
  policy_initial_receiver_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
  if [[ "$("$git_command" -C "$remote" show-ref --verify refs/heads/main)" != "$policy_candidate_head refs/heads/main" || "$("$git_command" -C "$remote" show-ref --verify refs/heads/policy-positive-positive-local-local-removal)" != "$policy_local_fixture_head refs/heads/policy-positive-positive-local-local-removal" || -n "$("$git_command" -C "$repository" status --porcelain)" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$policy_candidate_head" ]]; then
    printf 'policy-git-object-lifecycle: fixture did not preserve the candidate and first positive branch baseline\n' >&2
    return 1
  fi
  if "$git_command" -C "$remote" cat-file -e "$policy_control_head^{commit}" 2>/dev/null; then
    printf 'policy-git-object-lifecycle: control object unexpectedly exists in the candidate-only receiver\n' >&2
    return 1
  fi
}

policy_lifecycle_self_test() {
  set -euo pipefail
  repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
  test_root=$(mktemp -d "${TMPDIR:-/tmp}/policy-git-object-lifecycle.XXXXXX")
  trap 'rm -rf "$test_root"' EXIT
  git_command=$(command -v git)
  if [[ "$git_command" != /* ]]; then
    git_command="$(pwd -P)/$git_command"
  fi
  policy_source_repository="$test_root/control-source"
  mkdir -p "$test_root"
  cp -p "$repo_root/tests/policy-git-object-lifecycle.sh" "$test_root/policy-git-object-lifecycle.fixed.sh"
  fixed_helper_sha=$(policy_lifecycle_hash_file "$test_root/policy-git-object-lifecycle.fixed.sh")
  # Restore the untracked trusted-seed sentinel before the first source snapshot
  # so the preflight cannot be blamed for changing the source checkout, and so
  # the untracked-yet-present expectation below is satisfied in a fresh clone.
  bash "$repo_root/scripts/materialize-trusted-seed-cargo-lock.sh" || return $?
  policy_lifecycle_source_snapshot "$repo_root" > "$test_root/source.before.json"
  if ! BASELINE_GUARD_E2E_FOCUS=cargo-local-lock-policy BASELINE_GUARD_POLICY_CASE=positive-local-local-removal BASELINE_GUARD_POLICY_PREFLIGHT_ONLY=1 BASELINE_GUARD_POLICY_CONTROL_EXPORT="$policy_source_repository" /bin/bash "$repo_root/tests/check-installed-generated-workflow-guard.sh" > "$test_root/preflight.log" 2>&1; then
    cat "$test_root/preflight.log" >&2
    printf 'policy-git-object-lifecycle: actual harness candidate/control preflight failed\n' >&2
    return 1
  fi
  policy_lifecycle_source_snapshot "$repo_root" > "$test_root/source.after-preflight.json"
  if ! cmp -s "$test_root/source.before.json" "$test_root/source.after-preflight.json"; then
    printf 'policy-git-object-lifecycle: actual harness preflight changed the original source snapshot\n' >&2
    return 1
  fi
  if "$git_command" -C "$repo_root" ls-files --error-unmatch -- scripts/trusted-seed-cargo.lock >/dev/null 2>&1 || [[ ! -f "$repo_root/scripts/trusted-seed-cargo.lock" || -L "$repo_root/scripts/trusted-seed-cargo.lock" ]]; then
    printf 'policy-git-object-lifecycle: expected untracked trusted-seed sentinel is unavailable\n' >&2
    return 1
  fi
  policy_candidate_head=$("$git_command" -C "$policy_source_repository" rev-parse refs/qualification/policy-candidate)
  policy_candidate_tree=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_candidate_head^{tree}")
  policy_control_head=$("$git_command" -C "$policy_source_repository" rev-parse refs/qualification/policy-control)
  policy_control_tree=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_control_head^{tree}")
  policy_local_fixture_head=$("$git_command" -C "$policy_source_repository" rev-parse HEAD)
  if [[ "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_control_head^1")" != "$policy_candidate_head" || "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_local_fixture_head^1")" != "$policy_candidate_head" ]]; then
    printf 'policy-git-object-lifecycle: preflight candidate, control, and first positive commits do not have the reviewed parent graph\n' >&2
    return 1
  fi
  policy_lifecycle_prepare_git_fixture broken
  local refs_before
  local negative_status
  local fixed_status
  local restored_status
  local mutation_diff_status
  local broken_remote
  local fixed_repository
  local fixed_remote
  local restored_repository
  local restored_remote
  refs_before=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
  broken_remote=$remote
  python3 -I -S - "$test_root/policy-git-object-lifecycle.fixed.sh" "$test_root/policy-git-object-lifecycle.broken.sh" <<'PY'
import sys
from pathlib import Path

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
text = source.read_text()
import_block = '''  if ! "$git_command" -C "$remote" fetch --quiet --no-tags "file://$policy_source_repository" "$policy_control_head"; then
    printf 'generated-workflows: cannot import the empty control into the private receiver\\n' >&2
    return 1
  fi
'''
if text.count(import_block) != 1:
    raise SystemExit("policy-git-object-lifecycle: private helper mutation did not match one receiver import")
destination.write_text(text.replace(import_block, "", 1))
PY
  broken_helper_sha=$(policy_lifecycle_hash_file "$test_root/policy-git-object-lifecycle.broken.sh")
  if /usr/bin/diff -u "$test_root/policy-git-object-lifecycle.fixed.sh" "$test_root/policy-git-object-lifecycle.broken.sh" > "$test_root/policy-git-object-lifecycle.broken.patch"; then
    printf 'policy-git-object-lifecycle: private helper mutation produced no diff\n' >&2
    return 1
  else
    mutation_diff_status=$?
    if [[ "$mutation_diff_status" -ne 1 ]]; then
      printf 'policy-git-object-lifecycle: cannot preserve the private helper mutation diff\n' >&2
      return 1
    fi
  fi
  broken_helper_patch_sha=$(policy_lifecycle_hash_file "$test_root/policy-git-object-lifecycle.broken.patch")
  source "$test_root/policy-git-object-lifecycle.broken.sh"
  if policy_fixture_advance_to_control > "$test_root/broken-transition.log" 2>&1; then
    printf 'policy-git-object-lifecycle: receiver-import mutant unexpectedly completed the shared transition\n' >&2
    return 1
  else
    negative_status=$?
  fi
  if [[ "$negative_status" -ne 128 ]] || ! /usr/bin/grep -E -q 'missing object|nonexistent object|does not exist' "$test_root/broken-transition.log" || [[ "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$refs_before" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$policy_control_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_control_tree" || -n "$("$git_command" -C "$repository" status --porcelain)" ]] || "$git_command" -C "$remote" cat-file -e "$policy_control_head^{commit}" 2>/dev/null; then
    cat "$test_root/broken-transition.log" >&2
    printf 'policy-git-object-lifecycle: receiver-import mutant did not reach the missing-object update-ref failure or changed refs\n' >&2
    return 1
  fi
  source "$repo_root/tests/policy-git-object-lifecycle.sh"
  if [[ "$(policy_lifecycle_hash_file "$repo_root/tests/policy-git-object-lifecycle.sh")" != "$fixed_helper_sha" ]] || ! cmp -s "$repo_root/tests/policy-git-object-lifecycle.sh" "$test_root/policy-git-object-lifecycle.fixed.sh"; then
    printf 'policy-git-object-lifecycle: fixed shared helper was not restored byte-for-byte after the private mutation\n' >&2
    return 1
  fi
  if policy_fixture_advance_to_control; then
    fixed_status=0
  else
    fixed_status=$?
    return "$fixed_status"
  fi
  fixed_repository=$repository
  fixed_remote=$remote
  if [[ "$fixed_remote" != "$broken_remote" ]]; then
    printf 'policy-git-object-lifecycle: fixed retry did not reuse the broken transition receiver\n' >&2
    return 1
  fi
  expected_refs=$(printf '%s\n' "$refs_before" | awk -v old="$policy_candidate_head" -v replacement="$policy_control_head" '$2 == "refs/heads/main" && $1 == old { printf "%s %s\n", replacement, $2; found = 1; next } { print } END { if (!found) exit 1 }' | LC_ALL=C sort)
  if [[ "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$expected_refs" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$policy_control_head" || "$("$git_command" -C "$remote" rev-parse refs/heads/policy-positive-positive-local-local-removal)" != "$policy_local_fixture_head" || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$policy_control_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_control_tree" || -n "$("$git_command" -C "$repository" status --porcelain)" ]]; then
    printf 'policy-git-object-lifecycle: fixed transition did not preserve exact receiver refs and a clean control checkout\n' >&2
    return 1
  fi
  policy_lifecycle_source_snapshot "$repo_root" > "$test_root/source.after-fixed.json"
  if ! cmp -s "$test_root/source.before.json" "$test_root/source.after-fixed.json"; then
    printf 'policy-git-object-lifecycle: fixed transition changed the original source snapshot\n' >&2
    return 1
  fi
  policy_lifecycle_prepare_git_fixture restored
  if policy_fixture_advance_to_control; then
    restored_status=0
  else
    restored_status=$?
    return "$restored_status"
  fi
  restored_repository=$repository
  restored_remote=$remote
  if [[ "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$policy_control_head" || "$("$git_command" -C "$remote" rev-parse refs/heads/policy-positive-positive-local-local-removal)" != "$policy_local_fixture_head" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$expected_refs" || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$policy_control_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_control_tree" || -n "$("$git_command" -C "$repository" status --porcelain)" ]]; then
    printf 'policy-git-object-lifecycle: restored fixed transition did not preserve exact receiver refs and a clean control checkout\n' >&2
    return 1
  fi
  policy_lifecycle_source_snapshot "$repo_root" > "$test_root/source.after-restored.json"
  if ! cmp -s "$test_root/source.before.json" "$test_root/source.after-restored.json"; then
    printf 'policy-git-object-lifecycle: restored fixed transition changed the original source snapshot\n' >&2
    return 1
  fi
  printf 'candidate_commit=%s\n' "$policy_candidate_head"
  printf 'candidate_tree=%s\n' "$policy_candidate_tree"
  printf 'control_commit=%s\n' "$policy_control_head"
  printf 'control_tree=%s\n' "$policy_control_tree"
  printf 'first_positive_commit=%s\n' "$policy_local_fixture_head"
  printf 'broken_shared_helper=private mutated copy of tests/policy-git-object-lifecycle.sh\n'
  printf 'broken_helper_mutation=omit the bare receiver control fetch only\n'
  printf 'broken_helper_original_sha256=%s\n' "$fixed_helper_sha"
  printf 'broken_helper_mutated_sha256=%s\n' "$broken_helper_sha"
  printf 'broken_helper_patch_sha256=%s\n' "$broken_helper_patch_sha"
  printf 'broken_and_fixed_receiver=%s\n' "$broken_remote"
  printf 'broken_transition_command=policy_fixture_advance_to_control from the private mutated helper\n'
  printf 'broken_transition_status=%s\n' "$negative_status"
  printf 'broken_transition_error=%s\n' "$(tr '\n' ' ' < "$test_root/broken-transition.log")"
  printf 'broken_transition_output_sha256=%s\n' "$(policy_lifecycle_hash_file "$test_root/broken-transition.log")"
  printf 'fixed_working_import=git -C %s fetch --quiet --depth=1 --no-tags file://%s %s\n' "$fixed_repository" "$policy_source_repository" "$policy_control_head"
  printf 'fixed_receiver_import=git -C %s fetch --quiet --no-tags file://%s %s\n' "$fixed_remote" "$policy_source_repository" "$policy_control_head"
  printf 'fixed_receiver_ref_update=git -C %s update-ref refs/heads/main %s %s\n' "$fixed_remote" "$policy_control_head" "$policy_candidate_head"
  printf 'fixed_transition_status=%s\n' "$fixed_status"
  printf 'restored_receiver_import=git -C %s fetch --quiet --no-tags file://%s %s\n' "$restored_remote" "$policy_source_repository" "$policy_control_head"
  printf 'restored_receiver_ref_update=git -C %s update-ref refs/heads/main %s %s\n' "$restored_remote" "$policy_control_head" "$policy_candidate_head"
  printf 'restored_transition_status=%s\n' "$restored_status"
  printf 'receiver_refs_before:\n%s\n' "$refs_before"
  printf 'receiver_refs_after_fixed:\n%s\n' "$expected_refs"
  printf 'receiver_refs_before_sha256=%s\n' "$(printf '%s\n' "$refs_before" | policy_lifecycle_hash_file -)"
  printf 'receiver_refs_after_fixed_sha256=%s\n' "$(printf '%s\n' "$expected_refs" | policy_lifecycle_hash_file -)"
  printf 'source_snapshot=%s\n' "$(cat "$test_root/source.before.json")"
  printf 'source_snapshot_sha256=%s\n' "$(policy_lifecycle_hash_file "$test_root/source.before.json")"
  printf 'broken_causal_control=PASS\nfixed_transition=PASS\nrestored_transition=PASS\n'
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  if [[ "${1:-}" != --self-test || "$#" -ne 1 ]]; then
    printf 'usage: %s --self-test\n' "$0" >&2
    exit 2
  fi
  policy_lifecycle_self_test
fi
