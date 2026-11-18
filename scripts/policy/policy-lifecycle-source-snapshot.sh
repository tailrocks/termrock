#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/policy-lifecycle-source-snapshot.XXXXXX")
trap 'rm -rf "$test_root"' EXIT
git_command=$(command -v git)
if [[ "$git_command" != /* ]]; then
  git_command="$(pwd -P)/$git_command"
fi
source "$repo_root/tests/policy-git-object-lifecycle.sh"

create_repository() {
  local name=$1
  local branch=$2
  local repository="$test_root/$name"
  "$git_command" init --quiet --initial-branch=main "$repository"
  "$git_command" -C "$repository" config user.name 'Snapshot qualification'
  "$git_command" -C "$repository" config user.email 'snapshot@example.invalid'
  "$git_command" -C "$repository" config snapshot.marker 'fixed source value'
  printf 'tracked source\n' > "$repository/tracked.txt"
  printf 'staged base\n' > "$repository/staged.txt"
  printf 'unstaged base\n' > "$repository/unstaged.txt"
  "$git_command" -C "$repository" add -- tracked.txt staged.txt unstaged.txt
  "$git_command" -C "$repository" commit --quiet -m snapshot-source
  "$git_command" -C "$repository" cat-file -e 'HEAD^{commit}'
  [[ ! -f "$repository/.git/shallow" ]]
  if [[ "$branch" != main ]]; then
    "$git_command" -C "$repository" switch --quiet -c "$branch"
  fi
  printf 'staged after\n' > "$repository/staged.txt"
  "$git_command" -C "$repository" add -- staged.txt
  printf 'unstaged after\n' > "$repository/unstaged.txt"
  printf 'untracked source\n' > "$repository/untracked.txt"
  mkdir -p "$repository/scripts"
  printf 'seed sentinel contents\n' > "$repository/scripts/trusted-seed-cargo.lock"
  printf 'hook contents\n' > "$repository/.git/hooks/snapshot-hook"
  chmod 755 "$repository/.git/hooks/snapshot-hook"
  printf '%s\n' "$repository"
}

normalize_private_index() {
  local repository=$1
  "$git_command" -C "$repository" status --porcelain=v1 -z > /dev/null
  "$git_command" -C "$repository" write-tree > /dev/null
  "$git_command" -C "$repository" status --porcelain=v1 -z > /dev/null
  "$git_command" -C "$repository" write-tree > /dev/null
}

capture_reference_state() {
  local repository=$1
  local output=$2
  python3 -I -S - "$git_command" "$repository" > "$output" <<'PY'
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

command, root_value = sys.argv[1:]
root = Path(root_value).resolve()
def git(*args):
    return subprocess.check_output([command, "-C", str(root), *args])
def digest(value):
    return hashlib.sha256(value).hexdigest()
def git_path(value):
    output = git("rev-parse", "--path-format=absolute", "--git-path", value)
    return Path(os.fsdecode(output.rstrip(b"\n")))
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
def working_inventory():
    entries = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if relative.parts[0] == ".git":
            continue
        metadata = path.lstat()
        item = {"path": os.fsdecode(relative), "mode": stat.S_IMODE(metadata.st_mode)}
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

branch_result = subprocess.run([command, "-C", str(root), "symbolic-ref", "-q", "--short", "HEAD"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
if branch_result.returncode == 0:
    if branch_result.stderr or branch_result.stdout.count(b"\n") != 1 or not branch_result.stdout.endswith(b"\n"):
        raise SystemExit("reference capture observed malformed attached branch output")
    branch = branch_result.stdout[:-1].decode("utf-8")
elif branch_result.returncode == 1 and not branch_result.stdout and not branch_result.stderr:
    branch = ""
else:
    raise SystemExit("reference capture observed an unapproved symbolic-ref result")
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
refs = git("show-ref", "--head")
status = git("status", "--porcelain=v1", "-z")
tree = git("write-tree").decode().strip()
unstaged = git("diff", "--binary")
staged = git("diff", "--cached", "--binary")
snapshot = {
    "head": git("rev-parse", "HEAD").decode().strip(),
    "branch": branch,
    "tree": tree,
    "refs": refs.decode().splitlines(),
    "status_sha256": digest(status),
    "refs_sha256": digest(refs),
    "index_sha256": digest(index_path.read_bytes()),
    "unstaged_diff_sha256": digest(unstaged),
    "staged_diff_sha256": digest(staged),
    "config_sha256": digest(config_path.read_bytes()),
    "hooks": inventory(hooks_path),
    "untracked": untracked_entries,
    "seed_sentinel": sentinel_info,
}
physical = {
    "head": snapshot["head"],
    "head_tree": git("rev-parse", "HEAD^{tree}").decode().strip(),
    "branch": branch,
    "tree": tree,
    "refs_sha256": snapshot["refs_sha256"],
    "index_sha256": snapshot["index_sha256"],
    "status_sha256": snapshot["status_sha256"],
    "unstaged_diff_sha256": snapshot["unstaged_diff_sha256"],
    "staged_diff_sha256": snapshot["staged_diff_sha256"],
    "working_files": working_inventory(),
}
print(json.dumps({"snapshot": snapshot, "physical": physical}, sort_keys=True, separators=(",", ":")))
PY
}

assert_snapshot() {
  local repository=$1
  local expected_branch=$2
  local label=$3
  local before="$test_root/$label.reference.before.json"
  local snapshot="$test_root/$label.snapshot.json"
  local after="$test_root/$label.reference.after.json"
  normalize_private_index "$repository"
  capture_reference_state "$repository" "$before"
  policy_lifecycle_source_snapshot "$repository" > "$snapshot"
  capture_reference_state "$repository" "$after"
  python3 -I -S - "$before" "$snapshot" "$after" "$expected_branch" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

before = json.loads(Path(sys.argv[1]).read_text())
snapshot = json.loads(Path(sys.argv[2]).read_text())
after = json.loads(Path(sys.argv[3]).read_text())
expected_branch = sys.argv[4]
expected_fields = {
    "head", "branch", "tree", "refs", "status_sha256", "refs_sha256",
    "index_sha256", "unstaged_diff_sha256", "staged_diff_sha256",
    "config_sha256", "hooks", "untracked", "seed_sentinel",
}
if set(snapshot) != expected_fields or snapshot != before["snapshot"]:
    raise SystemExit("public source snapshot differs from independently captured Git and filesystem state")
if before != after:
    raise SystemExit("source state changed across public snapshot capture")
if snapshot["branch"] != expected_branch or snapshot["head"] != before["physical"]["head"]:
    raise SystemExit("snapshot branch or HEAD differs from the real repository")
empty_digest = hashlib.sha256(b"").hexdigest()
if snapshot["status_sha256"] == empty_digest or snapshot["staged_diff_sha256"] == empty_digest or snapshot["unstaged_diff_sha256"] == empty_digest:
    raise SystemExit("source fixture did not exercise status, staged diff, and unstaged diff state")
if snapshot["tree"] == before["physical"]["head_tree"]:
    raise SystemExit("source fixture index tree did not include its staged change")
if not any(entry["path"] == "hooks/snapshot-hook" for entry in snapshot["hooks"]):
    raise SystemExit("source snapshot omitted the independently created hook")
untracked_paths = {entry["path"] for entry in snapshot["untracked"]}
if untracked_paths != {"untracked.txt", "scripts/trusted-seed-cargo.lock"}:
    raise SystemExit("source snapshot omitted or added an untracked fixture")
if snapshot["seed_sentinel"] is None or snapshot["config_sha256"] != before["snapshot"]["config_sha256"]:
    raise SystemExit("source snapshot omitted the seed sentinel or actual config digest")
PY
}

attached_ascii_repository=$(create_repository attached-ascii main)
assert_snapshot "$attached_ascii_repository" main attached-ascii

attached_utf8_repository=$(create_repository attached-utf8 'feature/café')
assert_snapshot "$attached_utf8_repository" 'feature/café' attached-utf8

detached_repository=$(create_repository detached main)
detached_head=$("$git_command" -C "$detached_repository" rev-parse HEAD)
"$git_command" -C "$detached_repository" switch --quiet --detach "$detached_head"
python3 -I -S - "$git_command" "$detached_repository" <<'PY'
import subprocess
import sys

result = subprocess.run([sys.argv[1], "-C", sys.argv[2], "symbolic-ref", "-q", "--short", "HEAD"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
if result.returncode != 1 or result.stdout or result.stderr:
    raise SystemExit("real detached HEAD did not produce the approved symbolic-ref result")
PY
assert_snapshot "$detached_repository" '' detached
[[ "$("$git_command" -C "$detached_repository" rev-parse HEAD)" == "$detached_head" ]]

broken_repository="$test_root/broken"
"$git_command" init --quiet "$broken_repository"
: > "$broken_repository/.git/HEAD"
if policy_lifecycle_source_snapshot "$broken_repository" > "$test_root/broken.snapshot.json" 2> "$test_root/broken.snapshot.err"; then
  printf 'source snapshot accepted an invalid real Git repository\n' >&2
  exit 1
fi
[[ ! -s "$test_root/broken.snapshot.json" ]]
