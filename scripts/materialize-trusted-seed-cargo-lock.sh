#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
#
# Materializes the untracked trusted-seed sentinel `scripts/trusted-seed-cargo.lock`
# from the tracked, digest-pinned metadata pack (scripts/trusted-seed-metadata.pack).
# The sentinel is deliberately untracked in Git (scripts/policy/policy-git-object-lifecycle.sh
# asserts it is untracked yet present), so a fresh clone has to restore the exact
# reviewed bytes before any harness or the installer can consume them.
#
# The reviewed identities are read from scripts/check-generated-workflows.sh, the
# same single source the harnesses already parse (`sed -n 's/^trusted_seed_commit=//p'`),
# so this bootstrap can never drift from the pins the checker enforces.
#
# Fails closed: if the sentinel already exists it is left untouched (a deliberately
# corrupted sentinel must still fail the downstream integrity checks), and any
# mismatch between the restored bytes and trusted_lock_sha256 aborts.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
checker="$repo_root/scripts/check-generated-workflows.sh"
metadata_pack="$repo_root/scripts/trusted-seed-metadata.pack"
metadata_oids="$repo_root/scripts/trusted-seed-metadata.objects"
seed_commit_file="$repo_root/scripts/trusted-seed.commit"
target="$repo_root/scripts/trusted-seed-cargo.lock"

if command -v /usr/bin/shasum >/dev/null 2>&1; then
  hash_tool=/usr/bin/shasum
  hash_arguments=(-a 256)
elif command -v /usr/bin/sha256sum >/dev/null 2>&1; then
  hash_tool=/usr/bin/sha256sum
  hash_arguments=()
else
  printf 'generated-workflows: no SHA-256 tool is available to restore the trusted seed sentinel\n' >&2
  exit 1
fi

sha256_file() {
  "$hash_tool" "${hash_arguments[@]}" "$1" | awk '{print $1}'
}

git_command=$(command -v git)
if [[ "$git_command" != /* ]]; then
  git_command="$(pwd -P)/$git_command"
fi

trusted_seed_commit=$(sed -n 's/^trusted_seed_commit=//p' "$checker" | LC_ALL=C sort -u)
trusted_lock_sha256=$(sed -n 's/^trusted_lock_sha256=//p' "$checker" | LC_ALL=C sort -u)
trusted_metadata_pack_sha256=$(sed -n 's/^trusted_metadata_pack_sha256=//p' "$checker" | LC_ALL=C sort -u)
trusted_metadata_object_list_sha256=$(sed -n 's/^trusted_metadata_object_list_sha256=//p' "$checker" | LC_ALL=C sort -u)
for value in "$trusted_seed_commit" "$trusted_lock_sha256" "$trusted_metadata_pack_sha256" "$trusted_metadata_object_list_sha256"; do
  if [[ ! "$value" =~ ^[0-9a-f]{40}([0-9a-f]{24})?$ ]]; then
    printf 'generated-workflows: checker has no single reviewed trusted seed identity\n' >&2
    exit 1
  fi
done

if [[ -e "$target" || -L "$target" ]]; then
  # Present (regular, corrupted, or a symlink): never silently rewrite the sentinel.
  exit 0
fi

for path in "$checker" "$metadata_pack" "$metadata_oids" "$seed_commit_file"; do
  if [[ -L "$path" || ! -f "$path" ]]; then
    printf 'generated-workflows: reviewed trusted seed metadata is missing or unsafe in the source checkout\n' >&2
    exit 1
  fi
done
if [[ "$(sha256_file "$metadata_pack")" != "$trusted_metadata_pack_sha256" || "$(sha256_file "$metadata_oids")" != "$trusted_metadata_object_list_sha256" ]]; then
  printf 'generated-workflows: reviewed trusted seed metadata differs from the checker pins\n' >&2
  exit 1
fi

stage=$(mktemp -d "${TMPDIR:-/tmp}/trusted-seed-sentinel.XXXXXX")
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/objects" "$stage/home"
reader_environment=(HOME="$stage/home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" GIT_DIR="$stage/reader.git" GIT_OBJECT_DIRECTORY="$stage/objects" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0)
if [[ "$(env -i HOME="$stage/home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" hash-object -t commit --stdin < "$seed_commit_file")" != "$trusted_seed_commit" ]]; then
  printf 'generated-workflows: reviewed seed commit identity failed Git object verification\n' >&2
  exit 1
fi
env -i HOME="$stage/home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" init --bare --quiet --object-format=sha1 "$stage/reader.git" >/dev/null
env -i "${reader_environment[@]}" "$git_command" index-pack --stdin < "$metadata_pack" >/dev/null
seed_lock_blob=$(env -i "${reader_environment[@]}" "$git_command" rev-parse --verify "$trusted_seed_commit:scripts/trusted-seed-cargo.lock") || {
  printf 'generated-workflows: reviewed seed tree has no trusted seed Cargo lock\n' >&2
  exit 1
}
env -i "${reader_environment[@]}" "$git_command" cat-file blob "$seed_lock_blob" > "$stage/trusted-seed-cargo.lock"
if [[ "$(sha256_file "$stage/trusted-seed-cargo.lock")" != "$trusted_lock_sha256" ]]; then
  printf 'generated-workflows: restored trusted seed sentinel does not match its reviewed digest\n' >&2
  exit 1
fi
mv "$stage/trusted-seed-cargo.lock" "$target"
printf 'generated-workflows: restored the untracked trusted seed sentinel from the reviewed metadata pack\n'
