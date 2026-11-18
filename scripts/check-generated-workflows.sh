#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set +x +a
set -euo pipefail
LC_ALL=C
export LC_ALL
unset trusted_setup_token
trusted_setup_token=
if [[ "${1-}" == prepare-cache ]]; then
  trusted_setup_token=${GITHUB_COM_TOKEN-}
fi
unset GITHUB_COM_TOKEN
if [[ -n "$trusted_setup_token" && ( ! "$trusted_setup_token" =~ ^[A-Za-z0-9_]+$ || ${#trusted_setup_token} -gt 512 ) ]]; then
  printf 'generated-workflows: invalid setup credential format\n' >&2
  exit 1
fi

entry_index_present=0
entry_index=
if [[ ${GIT_INDEX_FILE+x} ]]; then
  entry_index_present=1
  entry_index=$GIT_INDEX_FILE
fi
caller_directory=$(pwd -P) || {
  printf 'generated-workflows: cannot resolve the caller directory\n' >&2
  exit 1
}
if [[ "$entry_index_present" -eq 1 && "$entry_index" != /* ]]; then
  entry_index="$caller_directory/$entry_index"
fi
git_candidate=$(command -v git) || {
  printf 'generated-workflows: Git is unavailable in the caller PATH\n' >&2
  exit 127
}
if [[ "$git_candidate" == /* ]]; then
  git_command=$git_candidate
elif [[ "$git_candidate" == */* ]]; then
  git_command="$caller_directory/$git_candidate"
else
  git_command=
  path_remainder="$PATH:"
  while [[ "$path_remainder" == *:* ]]; do
    path_entry=${path_remainder%%:*}
    path_remainder=${path_remainder#*:}
    if [[ -z "$path_entry" ]]; then
      path_entry=.
    fi
    if [[ "$path_entry" != /* ]]; then
      path_entry="$caller_directory/$path_entry"
    fi
    if [[ -f "$path_entry/git" && -x "$path_entry/git" ]]; then
      git_command="$path_entry/git"
      break
    fi
  done
fi
if [[ -z "$git_command" || ! -x "$git_command" || -d "$git_command" ]]; then
  printf 'generated-workflows: caller PATH Git is not an absolute executable file\n' >&2
  exit 127
fi

checker_directory=$(cd "$(dirname "$0")" && pwd -P)
install_manifest="$checker_directory/manifest"
installed_checker=0
if [[ -f "$install_manifest" || -L "$install_manifest" ]]; then
  installed_checker=1
fi
if [[ "$installed_checker" -eq 1 ]]; then
  TMPDIR=/tmp
  export TMPDIR
fi

export GIT_NO_REPLACE_OBJECTS=1

case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64)
    hash_tool=/usr/bin/shasum
    platform=macos-arm64
    mise_sha256=21c269cd3c88e31c56aad7f7ccd9916b6fa78683937d93e5d17129b783d6f962
    python_artifact_sha256=1b1b03fca8aa351698403f3d100ee83db7a5d99748fc7707681ac31a6a846ad9
    python_lock_sha256=6a849972c8d6ce7e8a5b3970b73dab00918aeb60155ed0c3c91e711991a9f8a1
    python_relative_path=bin/python3.15
    ;;
  Linux:x86_64|Linux:amd64)
    hash_tool=/usr/bin/sha256sum
    platform=linux-x64
    mise_sha256=0296d9cba9324106d4e0962bb94a2072902f3658813aeb3b09504947bb8ddc2b
    python_artifact_sha256=7b03163a980c607ba8194709d4a4086735a7a9c488cc475b098157306927b36b
    python_lock_sha256=5c13cb611f7f461656eeb052684c8d82f17eb5979d65f89b48622a0f60c4a168
    python_relative_path=bin/python3.15
    ;;
  *) printf 'generated-workflows: unsupported host platform\n' >&2; exit 2 ;;
esac
expected_config_sha256=bbc7db0a6b0b68c3a6090dab4284f730ea965d00be01e482720ef1afff1b7458
expected_python_mise_sha256=4bf669510cff7c1f5d5c7c8edc5f6d496bedf872b635295a607f81680074744d
expected_inventory_helper_sha256=0541173812097cd6ac4deb065a7e45b6960ddcf26369eb3fd32805a0b66f33eb
trusted_seed_commit=f4d9999ace7d6deb4294cd89c00afcc80e748d49
trusted_seed_tree=95406daef6ba234fde62fc47b933fbb8bdfce320
trusted_lock_sha256=d1abe8b74eac36822b25a735e8cc28e2c8434103a8c87f662a9ebe2da5f593a7
trusted_seed_policy_sha256=6d6c35161e163ff07f39d413e3d25c6d60016ebfcb6627e278e062a86df30eae
trusted_manifests_sha256=4e7f83b2eaf98c6b3413d10cc4c0dacc4c123d84ecdfd401e5da90daf7ad7f19
trusted_metadata_pack_sha256=845b98b650d85074bdad58d771e785f01d0824e4647efe0113fcdb7821704874
trusted_metadata_object_list_sha256=58233f6e2765561ef177650f49b76fb45e0f0980a97d98df5f39759a36525268

sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | awk '{print $1}'
  else
    "$hash_tool" "$1" | awk '{print $1}'
  fi
}

trusted_seed_lock_artifact="$checker_directory/trusted-seed-cargo.lock"
trusted_seed_policy_artifact="$checker_directory/trusted-seed-cargo-policy.json"
if [[ "$platform" == macos-arm64 ]]; then
  trusted_seed_policy_size=$(/usr/bin/stat -f '%z' "$trusted_seed_policy_artifact" 2>/dev/null || true)
else
  trusted_seed_policy_size=$(/usr/bin/stat -c '%s' "$trusted_seed_policy_artifact" 2>/dev/null || true)
fi
if [[ -L "$trusted_seed_lock_artifact" || ! -f "$trusted_seed_lock_artifact" || "$(sha256_file "$trusted_seed_lock_artifact")" != "$trusted_lock_sha256" || -L "$trusted_seed_policy_artifact" || ! -f "$trusted_seed_policy_artifact" || ! "$trusted_seed_policy_size" =~ ^[0-9]+$ || "$trusted_seed_policy_size" -gt 262144 || "$(sha256_file "$trusted_seed_policy_artifact")" != "$trusted_seed_policy_sha256" ]]; then
  printf 'generated-workflows: reviewed trusted seed Cargo policy artifacts are missing or changed\n' >&2
  exit 1
fi

if [[ "$installed_checker" -eq 1 ]]; then
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    python_generation=$(printf '%s\n' "$platform" '3.15.2' "$python_artifact_sha256" "$python_lock_sha256" "$expected_python_mise_sha256" "$expected_inventory_helper_sha256" '2026.11.4' "$mise_sha256" | "$hash_tool" -a 256 | /usr/bin/awk '{print $1}')
  else
    python_generation=$(printf '%s\n' "$platform" '3.15.2' "$python_artifact_sha256" "$python_lock_sha256" "$expected_python_mise_sha256" "$expected_inventory_helper_sha256" '2026.11.4' "$mise_sha256" | "$hash_tool" | /usr/bin/awk '{print $1}')
  fi
  checker_digest=${checker_directory##*/}
  install_root="$checker_directory/../.."
  artifact_root="$install_root/artifacts"
  if [[ ! "$checker_digest" =~ ^[0-9a-f]{64}$ || -L "$checker_directory" || -L "$install_manifest" || ! -f "$checker_directory/check-generated-workflows.sh" || -L "$checker_directory/check-generated-workflows.sh" || ! -f "$checker_directory/config.toml" || -L "$checker_directory/config.toml" || ! -f "$checker_directory/trusted-seed.commit" || -L "$checker_directory/trusted-seed.commit" || ! -f "$checker_directory/trusted-seed-metadata.pack" || -L "$checker_directory/trusted-seed-metadata.pack" || ! -f "$checker_directory/trusted-seed-metadata.objects" || -L "$checker_directory/trusted-seed-metadata.objects" || ! -f "$trusted_seed_lock_artifact" || -L "$trusted_seed_lock_artifact" || ! -f "$trusted_seed_policy_artifact" || -L "$trusted_seed_policy_artifact" ]]; then
    printf 'generated-workflows: installed checker artifacts are unsafe\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$checker_directory/check-generated-workflows.sh")" != "$checker_digest" || "$(sha256_file "$checker_directory/config.toml")" != "$expected_config_sha256" || "$(sha256_file "$checker_directory/python-inventory.sh")" != "$expected_inventory_helper_sha256" || "$(sha256_file "$checker_directory/trusted-seed-metadata.pack")" != "$trusted_metadata_pack_sha256" || "$(sha256_file "$checker_directory/trusted-seed-metadata.objects")" != "$trusted_metadata_object_list_sha256" || "$(sha256_file "$trusted_seed_lock_artifact")" != "$trusted_lock_sha256" || "$(sha256_file "$trusted_seed_policy_artifact")" != "$trusted_seed_policy_sha256" || "$("$git_command" hash-object -t commit --stdin < "$checker_directory/trusted-seed.commit")" != "$trusted_seed_commit" ]]; then
    printf 'generated-workflows: installed checker artifacts failed integrity validation\n' >&2
    exit 1
  fi
  if ! /usr/bin/grep -F -x -q 'version=2' "$install_manifest" || ! /usr/bin/grep -F -x -q 'checker_version=2' "$install_manifest" || ! /usr/bin/grep -F -x -q "trusted_seed_cargo_lock_sha256=$trusted_lock_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "trusted_seed_cargo_policy_sha256=$trusted_seed_policy_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "checker_sha256=$checker_digest" "$install_manifest" || ! /usr/bin/grep -F -x -q "config_sha256=$expected_config_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "inventory_helper_sha256=$expected_inventory_helper_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "seed_commit=$trusted_seed_commit" "$install_manifest" || ! /usr/bin/grep -F -x -q "seed_tree=$trusted_seed_tree" "$install_manifest" || ! /usr/bin/grep -F -x -q "metadata_pack_sha256=$trusted_metadata_pack_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "metadata_object_list_sha256=$trusted_metadata_object_list_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "mise_version=2026.11.4" "$install_manifest" || ! /usr/bin/grep -F -x -q "mise_sha256=$mise_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "python_version=3.15.2" "$install_manifest" || ! /usr/bin/grep -F -x -q "python_artifact_sha256=$python_artifact_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "python_lock_sha256=$python_lock_sha256" "$install_manifest" || ! /usr/bin/grep -F -x -q "python_mise_sha256=$expected_python_mise_sha256" "$install_manifest"; then
    printf 'generated-workflows: installed checker manifest does not match its artifacts\n' >&2
    exit 1
  fi
  mise_command="$artifact_root/mise/2026.11.4/$platform/mise"
  python_command="$artifact_root/python/3.15.2/$platform-$python_generation/runtime/$python_relative_path"
  python_inventory="$artifact_root/python/3.15.2/$platform-$python_generation/inventory"
  python_artifact_manifest="$artifact_root/python/3.15.2/$platform-$python_generation/manifest"
  python_mise_config="$artifact_root/python/3.15.2/$platform-$python_generation/mise.toml"
  python_mise_lock="$artifact_root/python/3.15.2/$platform-$python_generation/mise.lock"
  if [[ ! -f "$mise_command" || -L "$mise_command" || "$(sha256_file "$mise_command")" != "$mise_sha256" ]]; then
    printf 'generated-workflows: approved mise binary is missing or changed; run bash scripts/install-generated-workflow-guard.sh from reviewed code\n' >&2
    exit 127
  fi
  if [[ ! -x "$python_command" || -L "$python_command" || ! -f "$python_inventory" || -L "$python_inventory" || ! -f "$python_artifact_manifest" || -L "$python_artifact_manifest" || ! -f "$python_mise_config" || -L "$python_mise_config" || ! -f "$python_mise_lock" || -L "$python_mise_lock" || "$(sha256_file "$python_inventory")" != "$(/usr/bin/grep '^inventory_sha256=' "$install_manifest" | /usr/bin/cut -d= -f2-)" ]]; then
    printf 'generated-workflows: pinned Python 3.15.2 or its inventory is missing or changed; run bash scripts/install-generated-workflow-guard.sh from reviewed code\n' >&2
    exit 127
  fi
  if [[ "$(sha256_file "$python_mise_config")" != "$expected_python_mise_sha256" || "$(sha256_file "$python_mise_lock")" != "$python_lock_sha256" ]]; then
    printf 'generated-workflows: private Python Mise configuration or lock changed\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$python_inventory")" != "$(/usr/bin/grep '^inventory_sha256=' "$install_manifest" | /usr/bin/cut -d= -f2-)" ]]; then
    printf 'generated-workflows: pinned Python inventory manifest changed\n' >&2
    exit 1
  fi
  if ! /usr/bin/grep -F -x -q "python_version=3.15.2" "$python_artifact_manifest" || ! /usr/bin/grep -F -x -q "backend=core:python" "$python_artifact_manifest" || ! /usr/bin/grep -F -x -q "archive_sha256=$python_artifact_sha256" "$python_artifact_manifest" || ! /usr/bin/grep -F -x -q "lock_sha256=$python_lock_sha256" "$python_artifact_manifest" || ! /usr/bin/grep -F -x -q "mise_config_sha256=$expected_python_mise_sha256" "$python_artifact_manifest" || ! /usr/bin/grep -F -x -q "inventory_helper_sha256=$expected_inventory_helper_sha256" "$python_artifact_manifest" || ! /usr/bin/grep -F -x -q "inventory_sha256=$(sha256_file "$python_inventory")" "$python_artifact_manifest"; then
    printf 'generated-workflows: pinned Python artifact manifest does not match its archive or inventory\n' >&2
    exit 1
  fi
  "$checker_directory/python-inventory.sh" check "$artifact_root/python/3.15.2/$platform-$python_generation/runtime" "$python_inventory" || exit $?
else
  python_command=
  for candidate in /opt/homebrew/bin/python3 /usr/local/bin/python3 /usr/bin/python3 /opt/local/bin/python3; do
    if [[ -x "$candidate" ]] && "$candidate" -I -S -c 'import sys, tomllib; sys.exit(sys.version_info < (3, 11))' >/dev/null 2>&1; then
      python_command=$candidate
      break
    fi
  done
  if [[ -z "$python_command" ]]; then
    python_command=$(command -v python3 || true)
  fi
  if [[ -z "$python_command" ]]; then
    printf 'generated-workflows: isolated test Python is unavailable\n' >&2
    exit 127
  fi
fi

caller_path=$PATH
if [[ "$installed_checker" -eq 1 ]]; then
  caller_path=/usr/bin:/bin:/usr/sbin:/sbin
  curl_candidate=/usr/bin/curl
else
  mise_command=$(command -v mise) || {
    printf 'generated-workflows: mise is unavailable\n' >&2
    exit 127
  }
  curl_candidate=$(command -v curl) || {
    printf 'generated-workflows: curl is unavailable in the test PATH\n' >&2
    exit 127
  }
fi
clean_git_environment=("PATH=$caller_path" "HOME=${TMPDIR:-/tmp}" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1)
if [[ "$entry_index_present" -eq 1 ]]; then
  clean_git_environment+=("GIT_INDEX_FILE=$entry_index")
fi
repo_root=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$(pwd -P)" rev-parse --show-toplevel) || {
  printf 'generated-workflows: cannot resolve the checker repository\n' >&2
  exit 1
}
repo_root=$(cd "$repo_root" && pwd -P)
source_git_directory=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-dir) || {
  printf 'generated-workflows: cannot resolve the source Git directory\n' >&2
  exit 1
}
source_git_directory=$(cd "$source_git_directory" && pwd -P) || {
  printf 'generated-workflows: cannot resolve the source Git directory\n' >&2
  exit 1
}
objects=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-path objects) || {
  printf 'generated-workflows: cannot resolve the source object directory\n' >&2
  exit 1
}
objects=$(cd "$objects" && pwd -P) || {
  printf 'generated-workflows: cannot resolve the source object directory\n' >&2
  exit 1
}
object_format=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --show-object-format) || {
  printf 'generated-workflows: cannot resolve the source object format\n' >&2
  exit 1
}
effective_index=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-path index)
curl_command=$("$python_command" -I -S - "$curl_candidate" <<'PY'
import os
import sys
from pathlib import Path

try:
    path = Path(sys.argv[1]).resolve(strict=True)
except OSError:
    raise SystemExit(1)
if not path.is_absolute() or not path.is_file() or not os.access(path, os.X_OK):
    raise SystemExit(1)
print(path)
PY
) || {
  printf 'generated-workflows: caller PATH curl is not an executable file\n' >&2
  exit 127
}

case "$object_format" in
  sha1) object_length=40 ;;
  sha256) object_length=64 ;;
  *) printf 'generated-workflows: unsupported Git object format %s\n' "$object_format" >&2; exit 2 ;;
esac

temporary_base=${TMPDIR:-/tmp}
temporary_root=$(mktemp -d "$temporary_base/jackin-generated-workflows.XXXXXX") || {
  printf 'generated-workflows: cannot create a temporary directory under %s\n' "$temporary_base" >&2
  exit 1
}
cleanup_temporary_root() {
  unset trusted_setup_token
  if [[ -n "${trusted_seed_stage_dir:-}" && -d "$trusted_seed_stage_dir" && ! -L "$trusted_seed_stage_dir" ]]; then
    chmod -R u+w "$trusted_seed_stage_dir" 2>/dev/null || true
    find "$trusted_seed_stage_dir" -depth -delete 2>/dev/null || true
  fi
  if [[ -n "${tool_cache_stage_dir:-}" && -d "$tool_cache_stage_dir" && ! -L "$tool_cache_stage_dir" ]]; then
    chmod -R u+w "$tool_cache_stage_dir" 2>/dev/null || true
    find "$tool_cache_stage_dir" -depth -delete 2>/dev/null || true
  fi
  chmod u+w "$temporary_root" 2>/dev/null || true
  find "$temporary_root" -type d -exec chmod u+w {} + 2>/dev/null || true
  find "$temporary_root" -type f -exec chmod u+w {} + 2>/dev/null || true
  find "$temporary_root" -depth -delete
}
trap cleanup_temporary_root EXIT
temporary_root=$(cd "$temporary_root" && pwd -P) || {
  printf 'generated-workflows: cannot resolve the temporary directory\n' >&2
  exit 1
}
source_alternate_dirs="$temporary_root/source-alternate-directories"
env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" count-objects -v | sed -n 's/^alternate: //p' > "$source_alternate_dirs" || {
  printf 'generated-workflows: cannot resolve source alternate object directories\n' >&2
  exit 1
}
trusted_metadata_cache=
if [[ "$installed_checker" -eq 1 ]]; then
  trusted_metadata_cache="$install_root/cache/seed/$trusted_seed_commit"
  trusted_private_objects="$trusted_metadata_cache/objects"
else
  trusted_metadata_cache="$temporary_root/trusted-seed-cache"
  trusted_private_objects="$trusted_metadata_cache/objects"
fi
trusted_cache_validated=0
trusted_cache_provisioning=0
index_tree_objects="$temporary_root/index-tree-objects"
verified_blob_objects="$temporary_root/verified-blob-objects"
snapshot_reader="$temporary_root/snapshot-reader.git"
maximum_snapshot_entries=8192
maximum_trusted_seed_metadata_entries=$((maximum_snapshot_entries * 2))
maximum_snapshot_blob_bytes=$((16 * 1024 * 1024))
maximum_snapshot_total_bytes=$((512 * 1024 * 1024))
maximum_snapshot_listing_bytes=$((8 * 1024 * 1024))
maximum_hydration_missing_blobs=1024
maximum_hydration_total_bytes=$((256 * 1024 * 1024))
hydration_budget_file="$temporary_root/blob-hydration-budget"
hydration_deadline_file="$temporary_root/blob-hydration-deadline"

fail_materialization() {
  printf 'generated-workflows: cannot materialize snapshot: %s\n' "$1" >&2
  return 1
}

git_in_snapshot_context() {
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "$git_command" "$@"
}

git_in_trusted_reader() {
  local reader=$1
  shift
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_OBJECT_DIRECTORY=$trusted_private_objects" \
    "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "$git_command" --git-dir="$reader" -c core.hooksPath=/dev/null "$@"
}

git_in_private_snapshot_reader() {
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_DIR=$snapshot_reader" \
    "GIT_OBJECT_DIRECTORY=$index_tree_objects" \
    "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$trusted_private_objects:$objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "$git_command" -c core.hooksPath=/dev/null "$@"
}

git_with_private_tree_objects() {
  local private_index=${1-}
  local -a index_environment=(
    "HOME=$temporary_root/private-git-home"
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin"
    "GIT_DIR=$snapshot_reader"
    "GIT_OBJECT_DIRECTORY=$index_tree_objects"
    "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$trusted_private_objects:$objects"
    GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0
  )
  shift || true
  if [[ -n "$private_index" ]]; then
    index_environment+=("GIT_INDEX_FILE=$private_index")
  fi
  env -i "${index_environment[@]}" "$git_command" -c core.hooksPath=/dev/null "$@"
}

git_in_source_context() {
  env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" "$@"
}

copy_unchanged_regular_file() {
  local source=$1
  local destination=$2
  local label=$3
  local before
  local after
  local copied
  if [[ -L "$source" || ! -f "$source" ]]; then
    printf 'generated-workflows: %s is not a regular file\n' "$label" >&2
    return 1
  fi
  before=$(sha256_file "$source") || return $?
  cp -p "$source" "$destination" || return $?
  after=$(sha256_file "$source") || return $?
  copied=$(sha256_file "$destination") || return $?
  if [[ "$before" != "$after" || "$before" != "$copied" ]]; then
    printf 'generated-workflows: %s changed while the private copy was made\n' "$label" >&2
    return 1
  fi
}

validate_source_object_store() {
  local path
  local alternates="$objects/info/alternates"
  local http_alternates="$objects/info/http-alternates"

  if [[ -L "$objects" || ! -d "$objects" ]]; then
    printf 'generated-workflows: source object directory is not a regular directory\n' >&2
    return 1
  fi
  for path in "$alternates" "$http_alternates"; do
    if [[ -L "$path" || -s "$path" ]]; then
      printf 'generated-workflows: source object store uses an unsupported alternate object source\n' >&2
      return 1
    fi
  done
  if [[ "$objects" == *:* || "$objects" == *$'\n'* || "$objects" == *$'\r'* || "$objects" == *\\* || "$objects" == *\"* ]]; then
    printf 'generated-workflows: source object directory cannot be represented as a trusted reader alternate\n' >&2
    return 1
  fi
  if [[ "$verified_blob_objects" == "$objects" || -L "$verified_blob_objects" ]]; then
    printf 'generated-workflows: private verified blob object directory is unsafe\n' >&2
    return 1
  fi
  if [[ "$verified_blob_objects" == *:* || "$verified_blob_objects" == *$'\n'* || "$verified_blob_objects" == *$'\r'* || "$verified_blob_objects" == *\\* || "$verified_blob_objects" == *\"* ]]; then
    printf 'generated-workflows: private verified blob directory cannot be represented as a reader alternate\n' >&2
    return 1
  fi
  (umask 077 && mkdir -p "$index_tree_objects/info" "$index_tree_objects/pack" "$verified_blob_objects/info" "$verified_blob_objects/pack") || return $?
  if [[ "$installed_checker" -ne 1 ]]; then
    (umask 077 && mkdir -p "$trusted_private_objects/info" "$trusted_private_objects/pack") || return $?
  fi
}

initialize_snapshot_reader() {
  validate_source_object_store || return $?
  if [[ -d "$snapshot_reader" && ! -L "$snapshot_reader" && -f "$snapshot_reader/HEAD" ]]; then
    return 0
  fi
  mkdir -p "$temporary_root/private-git-home" || return $?
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    "$git_command" init --bare --quiet --object-format="$object_format" "$snapshot_reader" || {
      printf 'generated-workflows: cannot create the isolated snapshot Git reader\n' >&2
      return 1
    }
}

initialize_trusted_reader() {
  local reader=$1
  local seed_commit_object
  local seed_tree_object
  validate_source_object_store || return $?
  if [[ "$trusted_private_objects" == "$objects" || -L "$trusted_private_objects" ]]; then
    printf 'generated-workflows: private trusted seed object directory is unsafe\n' >&2
    return 1
  fi
  mkdir -p "$temporary_root/trusted-reader-home" || return $?
  if [[ "$installed_checker" -eq 1 && "$trusted_cache_provisioning" -ne 1 ]]; then
    if [[ ! -d "$trusted_metadata_cache" || -L "$trusted_metadata_cache" || ! -d "$trusted_private_objects" || -L "$trusted_private_objects" || ! -f "$trusted_metadata_cache/objects.txt" || -L "$trusted_metadata_cache/objects.txt" || ! -f "$trusted_metadata_cache/manifest" || -L "$trusted_metadata_cache/manifest" ]]; then
      printf 'generated-workflows: approved seed metadata cache is unavailable; run trusted setup from reviewed code\n' >&2
      return 1
    fi
    if [[ "$trusted_cache_validated" -eq 0 ]]; then
      validate_trusted_metadata_cache || return $?
      trusted_cache_validated=1
    fi
  elif [[ "$installed_checker" -ne 1 ]]; then
    (umask 077 && mkdir -p "$trusted_private_objects/info" "$trusted_private_objects/pack") || return $?
  fi
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    "$git_command" init --bare --quiet --object-format="$object_format" "$reader" || {
      printf 'generated-workflows: cannot create the isolated trusted source reader\n' >&2
      return 1
    }
}

validate_trusted_metadata_inventory() {
  "$python_command" -I -S - "$@" <<'PY'
import re
import sys
from pathlib import Path

expected_path, actual_path, listing_path, commit, root_tree, maximum_entries, maximum_listing, maximum_blob, maximum_total = sys.argv[1:]
maximum_entries = int(maximum_entries)
maximum_listing = int(maximum_listing)
maximum_blob = int(maximum_blob)
maximum_total = int(maximum_total)
def reject(message):
    raise SystemExit(f"generated-workflows: approved seed metadata {message}")

expected = Path(expected_path).read_text().splitlines()
if expected != sorted(set(expected)) or len(expected) > maximum_entries + 2 or any(re.fullmatch(r"[0-9a-f]{40}", oid) is None for oid in expected):
    reject("object list is malformed or exceeds its reviewed count bound")
if re.fullmatch(r"[0-9a-f]{40}", commit) is None or re.fullmatch(r"[0-9a-f]{40}", root_tree) is None or commit == root_tree or commit not in expected or root_tree not in expected:
    reject("object list omits its pinned commit or root tree")
actual = {}
actual_path = Path(actual_path)
if actual_path.stat().st_size > (maximum_entries + 2) * 128:
    reject("Git object inventory exceeds its reviewed count bound")
for line in actual_path.read_text().splitlines():
    fields = line.split()
    if len(fields) != 3 or re.fullmatch(r"[0-9a-f]{40}", fields[0]) is None or fields[1] not in {"commit", "tree", "blob"} or not fields[2].isdigit() or fields[0] in actual or len(actual) >= maximum_entries + 2:
        reject("pack contains an invalid Git object inventory")
    actual[fields[0]] = (fields[1], int(fields[2]))
listing_path = Path(listing_path)
if listing_path.stat().st_size > maximum_listing:
    reject("tree listing exceeds its reviewed byte bound")
raw = listing_path.read_bytes()
if not raw.endswith(b"\0"):
    reject("tree listing is truncated")
entries = raw[:-1].split(b"\0") if raw != b"\0" else []
if len(entries) > maximum_entries:
    reject("tree exceeds its reviewed entry bound")
expected_types = {commit: "commit", root_tree: "tree"}
for entry in entries:
    try:
        header, path_bytes = entry.split(b"\t", 1)
        mode, kind, oid = header.decode("ascii").split(" ")
        path = path_bytes.decode("utf-8", "strict")
    except (ValueError, UnicodeError):
        reject("tree entry is malformed")
    parts = path.split("/")
    if not path or path.startswith("/") or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts) or any(ord(character) < 32 or ord(character) == 127 for character in path):
        reject("tree contains an unsafe path")
    if re.fullmatch(r"[0-9a-f]{40}", oid) is None:
        reject("tree contains an invalid object identity")
    if mode == "040000" and kind == "tree":
        object_type = "tree"
    elif (mode, kind) in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
        excluded = any(path.startswith(prefix) and "/" not in path[len(prefix):] and path[len(prefix):].endswith(".tui.pb") for prefix in ("tests/functional/baselines/functional/", "crates/termrock-functional/tests/functional/baselines/functional/", "apps/jackin/tests/functional/baselines/functional/"))
        if excluded:
            continue
        object_type = "blob"
    else:
        reject("tree contains an unsupported entry")
    prior_type = expected_types.get(oid)
    if prior_type is not None and prior_type != object_type:
        reject("tree assigns one object identity incompatible types")
    expected_types[oid] = object_type
if set(expected) != set(expected_types) or set(actual) != set(expected_types):
    reject("pack object inventory does not match the pinned tree object set")
if any(actual[oid][0] != object_type for oid, object_type in expected_types.items()):
    reject("pack object types do not match the pinned tree")
total = 0
for oid, (kind, size) in actual.items():
    if kind != "blob":
        continue
    if size > maximum_blob:
        reject("contains an oversized blob")
    total += size
if total > maximum_total:
    reject("exceeds its reviewed object byte bound")
PY
}

validate_trusted_metadata_cache() {
  local reader="$temporary_root/trusted-metadata-validator.git"
  local listing="$temporary_root/trusted-metadata-cache-objects"
  local actual="$temporary_root/trusted-metadata-cache-actual"
  local expected_tree="$temporary_root/trusted-metadata-cache-expected-tree"
  local listed_hash
  local listed_count
  local seed_type
  local tree
  local tree_type
  if [[ -L "$trusted_metadata_cache/objects.txt" || -L "$trusted_metadata_cache/manifest" ]]; then
    printf 'generated-workflows: approved seed metadata cache contains a symlink\n' >&2
    return 1
  fi
  if [[ -e "$trusted_private_objects/info/alternates" || -L "$trusted_private_objects/info/alternates" ]]; then
    printf 'generated-workflows: approved seed metadata cache depends on a source object alternate\n' >&2
    return 1
  fi
  listed_hash=$(sha256_file "$trusted_metadata_cache/objects.txt") || return $?
  if ! /usr/bin/grep -F -x -q "objects_sha256=$listed_hash" "$trusted_metadata_cache/manifest" || ! /usr/bin/grep -F -x -q "seed_commit=$trusted_seed_commit" "$trusted_metadata_cache/manifest" || ! /usr/bin/grep -F -x -q "root_tree=$trusted_seed_tree" "$trusted_metadata_cache/manifest" || ! /usr/bin/grep -F -x -q "metadata_pack_sha256=$trusted_metadata_pack_sha256" "$trusted_metadata_cache/manifest" || ! /usr/bin/grep -F -x -q "metadata_object_list_sha256=$trusted_metadata_object_list_sha256" "$trusted_metadata_cache/manifest"; then
    printf 'generated-workflows: approved seed metadata cache manifest does not match its object list\n' >&2
    return 1
  fi
  listed_count=$(wc -l < "$trusted_metadata_cache/objects.txt" | tr -d ' ') || return $?
  if [[ ! "$listed_count" =~ ^[1-9][0-9]*$ || "$listed_count" -gt "$((maximum_trusted_seed_metadata_entries + 2))" ]]; then
    printf 'generated-workflows: approved seed metadata cache object count is invalid\n' >&2
    return 1
  fi
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_OBJECT_DIRECTORY=$trusted_private_objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    "$git_command" init --bare --quiet --object-format=sha1 "$reader" || {
      printf 'generated-workflows: cannot validate the approved seed metadata cache\n' >&2
      return 1
    }
  seed_type=$(env -i "HOME=$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" "GIT_OBJECT_DIRECTORY=$trusted_private_objects" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" --git-dir="$reader" cat-file -t "$trusted_seed_commit" 2>/dev/null) || {
    printf 'generated-workflows: approved seed commit is missing from its private metadata cache\n' >&2
    return 1
  }
  if [[ "$seed_type" != commit ]]; then
    printf 'generated-workflows: approved seed object is not a commit\n' >&2
    return 1
  fi
  tree=$(env -i "HOME=$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" "GIT_OBJECT_DIRECTORY=$trusted_private_objects" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" --git-dir="$reader" rev-parse --verify "$trusted_seed_commit^{tree}") || {
    printf 'generated-workflows: approved seed tree is missing from its private metadata cache\n' >&2
    return 1
  }
  tree_type=$(env -i "HOME=$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" "GIT_OBJECT_DIRECTORY=$trusted_private_objects" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" --git-dir="$reader" cat-file -t "$tree") || return $?
  if [[ "$tree_type" != tree ]]; then
    printf 'generated-workflows: approved seed root object is not a tree\n' >&2
    return 1
  fi
  if ! /usr/bin/grep -F -x -q "root_tree=$tree" "$trusted_metadata_cache/manifest" || ! /usr/bin/grep -F -x -q "object_count=$listed_count" "$trusted_metadata_cache/manifest"; then
    printf 'generated-workflows: approved seed metadata cache manifest does not match its tree inventory\n' >&2
    return 1
  fi
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_OBJECT_DIRECTORY=$trusted_private_objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    "$git_command" --git-dir="$reader" cat-file --batch-all-objects --batch-check > "$actual" || return $?
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_OBJECT_DIRECTORY=$trusted_private_objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    "$git_command" --git-dir="$reader" ls-tree -r -t -z --full-tree "$tree" > "$listing" || return $?
  "$python_command" -I -S - "$trusted_metadata_cache" <<'PY'
import os
import stat
import sys
from pathlib import Path

cache = Path(sys.argv[1])
owner = os.getuid()
for directory in (cache, cache.parent, cache.parent.parent, cache.parent.parent.parent):
    info = directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode) or info.st_uid != owner or info.st_mode & 0o077:
        raise SystemExit("generated-workflows: approved seed metadata cache directory chain is unsafe")
for directory, names, filenames in os.walk(cache, followlinks=False):
    for name in names + filenames:
        path = Path(directory) / name
        info = path.lstat()
        if info.st_uid != owner or (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)) and info.st_mode & 0o222:
            raise SystemExit("generated-workflows: approved seed metadata cache has unsafe owner or writable contents")
        if stat.S_ISLNK(info.st_mode) or not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)):
            raise SystemExit("generated-workflows: approved seed metadata cache contains an unsafe filesystem entry")
PY
  validate_trusted_metadata_inventory "$trusted_metadata_cache/objects.txt" "$actual" "$listing" "$trusted_seed_commit" "$tree" "$maximum_trusted_seed_metadata_entries" "$maximum_snapshot_listing_bytes" "$maximum_snapshot_blob_bytes" "$maximum_snapshot_total_bytes" || return $?
}

provision_trusted_metadata_cache() {
  local stage="$trusted_metadata_cache.stage.$$"
  local reader="$temporary_root/trusted-metadata-provisioner.git"
  local final_cache=$trusted_metadata_cache
  local tree
  local listing="$temporary_root/trusted-seed-metadata-listing"
  local object_list="$temporary_root/trusted-seed-metadata-oids"
  local pack="$temporary_root/trusted-seed-metadata.pack"
  local object_count
  local list_hash
  local seed_directory="$temporary_root/trusted-seed-provision"
  local cargo_home="$temporary_root/trusted-seed-provision-cargo"
  local metadata_pack="$checker_directory/trusted-seed-metadata.pack"
  local metadata_oids="$checker_directory/trusted-seed-metadata.objects"
  local metadata_actual="$temporary_root/trusted-seed-metadata-actual"
  local metadata_object_count
  local pack_bytes
  local embedded_commit
  local object_environment=("HOME=$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" "GIT_OBJECT_DIRECTORY=$stage/objects" "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$objects" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0)
  validate_source_object_store || return $?
  if [[ -e "$trusted_metadata_cache" || -L "$trusted_metadata_cache" ]]; then
    validate_trusted_metadata_cache || return $?
    return 0
  fi
  mkdir -p "${trusted_metadata_cache%/*}" || return $?
  if [[ -L "${trusted_metadata_cache%/*}" || -L "$trusted_metadata_cache" ]]; then
    printf 'generated-workflows: approved seed metadata cache parent is unsafe\n' >&2
    return 1
  fi
  mkdir "$stage" || return $?
  trusted_seed_stage_dir=$stage
  mkdir "$stage/objects" "$stage/objects/info" "$stage/objects/pack" || return $?
  trusted_private_objects="$stage/objects"
  trusted_cache_provisioning=1
  mkdir -p "$temporary_root/trusted-reader-home" || return $?
  embedded_commit=$(env -i "HOME=$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" hash-object -t commit --stdin < "$checker_directory/trusted-seed.commit") || return $?
  if [[ "$embedded_commit" != "$trusted_seed_commit" || "$(sha256_file "$metadata_pack")" != "$trusted_metadata_pack_sha256" || "$(sha256_file "$metadata_oids")" != "$trusted_metadata_object_list_sha256" ]]; then
    printf 'generated-workflows: embedded approved seed commit failed Git object verification\n' >&2
    return 1
  fi
  pack_bytes=$(wc -c < "$metadata_pack" | tr -d ' ') || return $?
  metadata_object_count=$(wc -l < "$metadata_oids" | tr -d ' ') || return $?
  if [[ ! "$pack_bytes" =~ ^[0-9]+$ || "$pack_bytes" -gt "$((maximum_snapshot_total_bytes))" || ! "$metadata_object_count" =~ ^[1-9][0-9]*$ || "$metadata_object_count" -gt "$((maximum_trusted_seed_metadata_entries + 2))" ]]; then
    printf 'generated-workflows: reviewed seed metadata pack or object list exceeds its bound\n' >&2
    return 1
  fi
  "$python_command" -I -S - "$metadata_oids" "$trusted_seed_commit" "$trusted_seed_tree" "$metadata_object_count" <<'PY' || return $?
import re
import sys
from pathlib import Path

path, commit, root_tree, expected_count = sys.argv[1:]
objects = Path(path).read_text().splitlines()
if objects != sorted(set(objects)) or len(objects) != int(expected_count) or commit not in objects or root_tree not in objects or any(re.fullmatch(r"[0-9a-f]{40}", item) is None for item in objects):
    raise SystemExit("generated-workflows: reviewed seed metadata object list is malformed")
PY
  env -i HOME="$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" init --bare --quiet --object-format=sha1 "$reader" || return $?
  local metadata_index_environment=("HOME=$temporary_root/trusted-reader-home" "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" "GIT_DIR=$reader" "GIT_OBJECT_DIRECTORY=$stage/objects" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0)
  env -i "${metadata_index_environment[@]}" "$git_command" index-pack --stdin < "$metadata_pack" > "$temporary_root/trusted-seed-metadata-pack-index" || {
    printf 'generated-workflows: reviewed seed metadata pack failed Git object verification\n' >&2
    return 1
  }
  env -i "${metadata_index_environment[@]}" "$git_command" cat-file --batch-all-objects --batch-check > "$metadata_actual" || return $?
  if [[ "$(env -i "${metadata_index_environment[@]}" "$git_command" cat-file -t "$trusted_seed_commit")" != commit ]]; then
    printf 'generated-workflows: reviewed seed metadata pack commit type is invalid\n' >&2
    return 1
  fi
  tree=$(env -i "${metadata_index_environment[@]}" "$git_command" rev-parse --verify "$trusted_seed_commit^{tree}") || {
    printf 'generated-workflows: reviewed seed metadata pack has no approved root tree\n' >&2
    return 1
  }
  if [[ "$tree" != "$trusted_seed_tree" ]]; then
    printf 'generated-workflows: reviewed seed metadata root tree does not match its fixed identity\n' >&2
    return 1
  fi
  env -i "${object_environment[@]}" "$git_command" --git-dir="$reader" ls-tree -r -t -z --full-tree "$tree" > "$listing" || {
    printf 'generated-workflows: approved seed tree metadata is incomplete; setup fails closed\n' >&2
    return 1
  }
  validate_trusted_metadata_inventory "$metadata_oids" "$metadata_actual" "$listing" "$trusted_seed_commit" "$tree" "$maximum_trusted_seed_metadata_entries" "$maximum_snapshot_listing_bytes" "$maximum_snapshot_blob_bytes" "$maximum_snapshot_total_bytes" || return $?
  cp "$metadata_oids" "$object_list" || return $?
  mkdir -p "$cargo_home" || return $?
  prepare_trusted_seed "$seed_directory" "$reader" "$temporary_root/trusted-seed-provision-files" '' "$cargo_home" || return $?
  object_count=$(wc -l < "$object_list" | tr -d ' ') || return $?
  if [[ ! "$object_count" =~ ^[1-9][0-9]*$ || "$object_count" -gt "$((maximum_trusted_seed_metadata_entries + 2))" ]]; then
    printf 'generated-workflows: approved seed metadata object count exceeds its bound\n' >&2
    return 1
  fi
  env -i "${object_environment[@]}" "$git_command" pack-objects --stdout < "$object_list" > "$pack" || {
    printf 'generated-workflows: cannot package the approved seed metadata objects\n' >&2
    return 1
  }
  pack_bytes=$(wc -c < "$pack" | tr -d ' ') || return $?
  if [[ ! "$pack_bytes" =~ ^[0-9]+$ ]] || (( pack_bytes > maximum_snapshot_total_bytes )); then
    printf 'generated-workflows: approved seed metadata pack exceeds its reviewed byte bound\n' >&2
    return 1
  fi
  env -i "${object_environment[@]}" "$git_command" index-pack --stdin < "$pack" >/dev/null || return $?
  if [[ -e "$stage/objects/info/alternates" || -L "$stage/objects/info/alternates" ]]; then
    printf 'generated-workflows: approved seed cache attempted to retain a source object alternate\n' >&2
    return 1
  fi
  list_hash=$(sha256_file "$object_list") || return $?
  cp "$object_list" "$stage/objects.txt" || return $?
  printf 'seed_commit=%s\nroot_tree=%s\nobject_count=%s\nobjects_sha256=%s\nmetadata_pack_sha256=%s\nmetadata_object_list_sha256=%s\n' "$trusted_seed_commit" "$tree" "$object_count" "$list_hash" "$trusted_metadata_pack_sha256" "$trusted_metadata_object_list_sha256" > "$stage/manifest"
  chmod 400 "$stage/objects.txt" "$stage/manifest"
  find "$stage/objects" -type f -exec chmod a-w {} + || return $?
  find "$stage/objects" -type d -exec chmod a-w {} + || return $?
  chmod 500 "$stage/objects" "$stage"
  trusted_metadata_cache="$stage"
  validate_trusted_metadata_cache || return $?
  trusted_metadata_cache=$final_cache
  trusted_cache_provisioning=0
  mv "$stage" "$final_cache" || return $?
  trusted_seed_stage_dir=
  trusted_metadata_cache=$final_cache
  trusted_private_objects="$trusted_metadata_cache/objects"
  trusted_cache_validated=1
  validate_trusted_metadata_cache || return $?
}

validate_trusted_tree_entries() {
  "$python_command" -I -S - "$1" "$object_length" <<'PY'
import re
import sys
from pathlib import PurePosixPath

entries_path, object_length = sys.argv[1:]
raw = open(entries_path, "rb").read()
if not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: trusted seed tree listing is incomplete")
seen = set()
for entry in raw[:-1].split(b"\0"):
    try:
        header, path_bytes = entry.split(b"\t", 1)
        mode, kind, object_id = header.decode("ascii").split(" ")
        path = path_bytes.decode("utf-8", "strict")
    except (ValueError, UnicodeError):
        raise SystemExit("generated-workflows: trusted seed tree entry is malformed")
    parts = PurePosixPath(path).parts
    if not path or path.startswith("/") or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts):
        raise SystemExit("generated-workflows: trusted seed contains an unsafe path")
    if any(part == ".gitmodules" for part in parts):
        raise SystemExit("generated-workflows: trusted seed contains submodule metadata")
    if any(ord(character) < 32 or ord(character) == 127 for character in path):
        raise SystemExit("generated-workflows: trusted seed path contains control characters")
    if path in seen:
        raise SystemExit("generated-workflows: trusted seed contains a duplicate path")
    seen.add(path)
    if re.fullmatch(r"[0-9a-f]{" + object_length + r"}", object_id) is None:
        raise SystemExit("generated-workflows: trusted seed has an invalid Git object identity")
    if mode == "160000" or kind == "commit":
        raise SystemExit("generated-workflows: trusted seed contains a Git submodule")
    if (mode, kind) not in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
        raise SystemExit("generated-workflows: trusted seed contains an unsupported Git tree entry")
PY
}

bounded_tree_listing() {
  local kind=$1
  local reader=${2-}
  local tree=$3
  local destination=$4
  local -a args=(ls-tree -r -z --full-tree "$tree")
  local program='import sys
from pathlib import Path

destination, maximum_entries, maximum_bytes = sys.argv[1:]
maximum_entries = int(maximum_entries)
maximum_bytes = int(maximum_bytes)
path = Path(destination)
entries = 0
size = 0
partial = b""
with path.open("wb") as output:
    while True:
        chunk = sys.stdin.buffer.read(65536)
        if not chunk:
            break
        size += len(chunk)
        if size > maximum_bytes:
            raise SystemExit("generated-workflows: snapshot tree listing exceeds the size limit")
        combined = partial + chunk
        entries += combined.count(b"\0")
        if entries > maximum_entries:
            raise SystemExit("generated-workflows: snapshot tree exceeds the entry count limit")
        partial = combined.rsplit(b"\0", 1)[-1]
        output.write(chunk)
if partial:
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")'
  if [[ "$kind" == trusted ]]; then
    git_in_trusted_reader "$reader" "${args[@]}" | "$python_command" -I -S -c "$program" "$destination" "$maximum_snapshot_entries" "$maximum_snapshot_listing_bytes"
  else
    git_in_private_snapshot_reader "${args[@]}" | "$python_command" -I -S -c "$program" "$destination" "$maximum_snapshot_entries" "$maximum_snapshot_listing_bytes"
  fi
}

validate_snapshot_tree_entries() {
  "$python_command" -I -S - "$1" "$object_length" <<'PY'
import re
import sys


entries_path, object_length = sys.argv[1:]
raw = open(entries_path, "rb").read()
if raw and not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")
seen = set()
for entry in raw.split(b"\0")[:-1] if raw else ():
    try:
        header, path_bytes = entry.split(b"\t", 1)
        mode, kind, object_id = header.decode("ascii").split(" ")
        path = path_bytes.decode("utf-8", "strict")
    except (ValueError, UnicodeError):
        raise SystemExit("generated-workflows: snapshot tree entry is malformed")
    parts = path.split("/")
    if not path or path.startswith("/") or path.endswith("/") or "//" in path or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts):
        raise SystemExit("generated-workflows: snapshot tree contains an unsafe path")
    if any(ord(character) < 32 or ord(character) == 127 for character in path):
        raise SystemExit("generated-workflows: snapshot tree path contains control characters")
    if path in seen:
        raise SystemExit("generated-workflows: snapshot tree contains a duplicate path")
    seen.add(path)
    if re.fullmatch(r"[0-9a-f]{" + object_length + r"}", object_id) is None:
        raise SystemExit("generated-workflows: snapshot tree has an invalid Git object identity")
    if mode == "160000" or kind == "commit":
        raise SystemExit("generated-workflows: cannot materialize snapshot: snapshot contains unsupported gitlink entries")
    if (mode, kind) not in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
        raise SystemExit("generated-workflows: snapshot tree contains an unsupported entry type")
PY
}

preflight_tree_blob_sizes() {
  local kind=$1
  local reader=${2-}
  local entries=$3
  local allow_missing=${4:-false}
  local oids="$temporary_root/tree-blob-oids"
  local statuses="$temporary_root/tree-blob-status"
  "$python_command" -I -S - "$entries" "$oids" <<'PY'
import sys
from pathlib import Path

entries_path, output_path = sys.argv[1:]
raw = Path(entries_path).read_bytes()
if raw and not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")
objects = []
for entry in raw.split(b"\0")[:-1] if raw else ():
    header, _ = entry.split(b"\t", 1)
    mode, kind, oid = header.decode("ascii").split(" ")
    if kind == "blob" and mode in {"100644", "100755", "120000"}:
        objects.append(oid)
Path(output_path).write_text("".join(oid + "\n" for oid in objects))
PY
  if [[ "$kind" == trusted ]]; then
    git_in_trusted_reader "$reader" cat-file --batch-check < "$oids" > "$statuses" || return $?
  else
    git_in_private_snapshot_reader cat-file --batch-check < "$oids" > "$statuses" || return $?
  fi
  "$python_command" -I -S - "$entries" "$statuses" "$allow_missing" "$maximum_snapshot_blob_bytes" "$maximum_snapshot_total_bytes" "$kind" <<'PY'
import sys
from pathlib import Path

entries_path, statuses_path, allow_missing, maximum_blob, maximum_total, kind = sys.argv[1:]
raw = Path(entries_path).read_bytes()
entries = [(part.split(b"\t", 1)[1].decode("utf-8"), part.split(b"\t", 1)[0].decode("ascii").split()[2]) for part in raw.split(b"\0") if part]
allowed = {
    ("tests/functional/reference-bins/aarch64-apple-darwin/holla", "e6c7debc7a4f36855d8c9d0b150b4ff0735dcf03"): 16835672,
    ("crates/termrock-functional/tests/functional/reference-bins/aarch64-apple-darwin/holla", "e6c7debc7a4f36855d8c9d0b150b4ff0735dcf03"): 16835672,
    ("tests/functional/reference-bins/aarch64-apple-darwin/showcase", "15d1adf929c8ce69d0e2eddca63a3fab1f40d9b4"): 20608024,
    ("crates/termrock-functional/tests/functional/reference-bins/aarch64-apple-darwin/showcase", "15d1adf929c8ce69d0e2eddca63a3fab1f40d9b4"): 20608024,
    ("tests/functional/reference-bins/aarch64-apple-darwin/pr2-3cbe13c/jackin", "7af05a2846a2843e357eb64454e82e51ecd7a36a"): 20601600,
    ("crates/termrock-functional/tests/functional/reference-bins/aarch64-apple-darwin/pr2-3cbe13c/jackin", "7af05a2846a2843e357eb64454e82e51ecd7a36a"): 20601600,
}
oids = [oid for _, oid in entries]
statuses = Path(statuses_path).read_text().splitlines()
if len(oids) != len(statuses):
    raise SystemExit("generated-workflows: snapshot blob inspection was incomplete")
missing_allowed = allow_missing == "true"
total = 0
for (path, oid), status in zip(entries, statuses, strict=True):
    fields = status.split()
    if len(fields) == 2 and fields == [oid, "missing"]:
        if missing_allowed:
            continue
        if kind == "trusted":
            raise SystemExit("generated-workflows: trusted seed included blob object is unavailable")
        raise SystemExit("generated-workflows: cannot materialize snapshot: snapshot contains an unavailable blob object")
    if len(fields) != 3 or fields[0] != oid or fields[1] != "blob" or not fields[2].isdigit():
        raise SystemExit("generated-workflows: snapshot tree contains an unexpected blob object type")
    size = int(fields[2])
    if size > int(maximum_blob) and allowed.get((path, oid)) != size:
        raise SystemExit("generated-workflows: cannot materialize snapshot: snapshot blob exceeds the per-object size limit")
    total += size
    if total > int(maximum_total):
        raise SystemExit("generated-workflows: cannot materialize snapshot: snapshot exceeds the total materialized blob size limit")
PY
}

copy_bounded_blob() {
  local reader=$1
  local object_id=$2
  local destination=$3
  local remaining=$4
  local blob_limit=${5:-$maximum_snapshot_blob_bytes}
  local program='import sys
from pathlib import Path

destination, maximum_blob, maximum_total = sys.argv[1:]
maximum_blob = int(maximum_blob)
maximum_total = int(maximum_total)
total = 0
with Path(destination).open("wb") as output:
    while True:
        chunk = sys.stdin.buffer.read(65536)
        if not chunk:
            break
        total += len(chunk)
        if total > maximum_blob:
            raise SystemExit("generated-workflows: snapshot blob exceeds the per-object size limit")
        if total > maximum_total:
            raise SystemExit("generated-workflows: snapshot exceeds the total materialized blob size limit")
        output.write(chunk)
print(total)'
  local copied
  if [[ -n "$reader" ]]; then
    copied=$(git_in_trusted_reader "$reader" cat-file blob "$object_id" | "$python_command" -I -S -c "$program" "$destination" "$blob_limit" "$remaining") || return $?
  else
    copied=$(git_in_private_snapshot_reader cat-file blob "$object_id" | "$python_command" -I -S -c "$program" "$destination" "$blob_limit" "$remaining") || return $?
  fi
  printf '%s\n' "$copied"
}

project_trusted_tree_entries() {
  "$python_command" -I -S - "$1" "$2" <<'PY'
import sys
from pathlib import Path

source_path, destination_path = sys.argv[1:]
prefixes = (
    "tests/functional/baselines/functional/", "crates/termrock-functional/tests/functional/baselines/functional/",
    "apps/jackin/tests/functional/baselines/functional/",
)
raw = Path(source_path).read_bytes()
if not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")
projected = []
for entry in raw[:-1].split(b"\0"):
    header, path_bytes = entry.split(b"\t", 1)
    mode, kind, object_id = header.decode("ascii").split(" ")
    path = path_bytes.decode("utf-8", "strict")
    excluded = False
    for prefix in prefixes:
        if path.startswith(prefix):
            name = path[len(prefix):]
            if "/" not in name and name.endswith(".tui.pb"):
                if (mode, kind) != ("100644", "blob"):
                    raise SystemExit("generated-workflows: trusted seed baseline projection contains an unsupported entry type")
                excluded = True
                break
    if not excluded:
        projected.append(entry)
Path(destination_path).write_bytes(b"\0".join(projected) + (b"\0" if projected else b""))
PY
}

hydrate_blobs() {
  local kind=$1
  local reader=${2-}
  local entries=$3
  local scope=$4
  local description=$5
  local all_oids="$temporary_root/$scope-blob-oids"
  local object_status="$temporary_root/$scope-blob-object-status"
  local missing_oids="$temporary_root/$scope-missing-blob-oids"
  local response_dir
  local object_id
  local written_oid
  local verified_oid
  local missing_count
  local remaining_count
  local remaining_bytes
  local downloaded_bytes
  local writer
  local writer_home
  local -a writer_environment hydration_environment
  local hydration_token=
  export -n hydration_token
  local used_count
  local used_bytes
  local extra_budget_field

  "$python_command" -I -S - "$entries" "$all_oids" <<'PY'
import sys
from pathlib import Path

source_path, destination_path = sys.argv[1:]
raw = Path(source_path).read_bytes()
if not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: trusted seed tree listing is incomplete")
objects = set()
for entry in raw[:-1].split(b"\0"):
    header, _ = entry.split(b"\t", 1)
    mode, kind, object_id = header.decode("ascii").split(" ")
    if kind == "blob":
        objects.add(object_id)
Path(destination_path).write_text("".join(f"{object_id}\n" for object_id in sorted(objects)))
PY
  if [[ "$kind" == trusted ]]; then
    git_in_trusted_reader "$reader" cat-file --batch-check < "$all_oids" > "$object_status" || return $?
  else
    git_in_private_snapshot_reader cat-file --batch-check < "$all_oids" > "$object_status" || return $?
  fi
  "$python_command" -I -S - "$all_oids" "$object_status" "$missing_oids" "$kind" <<'PY' || return $?
import sys
from pathlib import Path

oids_path, status_path, missing_path, kind = sys.argv[1:]
oids = Path(oids_path).read_text().splitlines()
statuses = Path(status_path).read_text().splitlines()
if len(oids) != len(statuses):
    raise SystemExit(f"generated-workflows: {kind} snapshot object inspection was incomplete")
missing = []
for expected, line in zip(oids, statuses, strict=True):
    fields = line.split()
    if len(fields) == 2 and fields == [expected, "missing"]:
        missing.append(expected)
    elif len(fields) == 3 and fields[0] == expected and fields[1] == "blob" and fields[2].isdigit():
        continue
    else:
        raise SystemExit(f"generated-workflows: {kind} snapshot blob object has an unexpected type")
Path(missing_path).write_text("".join(f"{object_id}\n" for object_id in missing))
PY
  if [[ ! -s "$missing_oids" ]]; then
    return 0
  fi
  if [[ "$object_format" != sha1 ]]; then
    fail_materialization "missing snapshot blobs can only be hydrated for SHA-1 Git repositories"
    return 1
  fi
  if [[ "$kind" == trusted ]]; then
    writer=$reader
    writer_home="$temporary_root/trusted-reader-home"
  else
    writer=$snapshot_reader
    writer_home="$temporary_root/private-git-home"
  fi
  if [[ ! -f "$hydration_budget_file" ]]; then
    (umask 077 && printf '0 0\n' > "$hydration_budget_file") || return $?
  fi
  if ! IFS=' ' read -r used_count used_bytes extra_budget_field < "$hydration_budget_file" ||
    [[ ! "$used_count" =~ ^[0-9]+$ || ! "$used_bytes" =~ ^[0-9]+$ || -n "$extra_budget_field" ]]; then
    fail_materialization "blob hydration invocation budget is invalid"
    return 1
  fi
  missing_count=$(wc -l < "$missing_oids" | tr -d ' ') || return $?
  remaining_count=$((maximum_hydration_missing_blobs - used_count))
  remaining_bytes=$((maximum_hydration_total_bytes - used_bytes))
  if (( missing_count > remaining_count )); then
    fail_materialization "missing snapshot blobs exceed the invocation object count limit"
    return 1
  fi
  if (( remaining_bytes < 0 )); then
    fail_materialization "missing snapshot blobs exceed the invocation byte limit"
    return 1
  fi
  if [[ ! -f "$hydration_deadline_file" ]]; then
    "$python_command" -I -S - "$hydration_deadline_file" <<'PY' || return $?
import sys
import time
from pathlib import Path

with Path(sys.argv[1]).open("x") as deadline:
    deadline.write(str(time.monotonic() + 15 * 60))
PY
    chmod 600 "$hydration_deadline_file" || return $?
  fi
  response_dir=$(mktemp -d "$temporary_root/$scope-blob-responses.XXXXXX") || return $?
  hydration_environment=(/usr/bin/env -i)
  if [[ "$installed_checker" -eq 1 && "${prepare_cache_mode:-0}" -eq 1 && ( "$kind" == trusted || "${trusted_snapshot_auth:-0}" -eq 1 ) && -n "$trusted_setup_token" ]]; then
    hydration_token=$trusted_setup_token
    hydration_environment+=(-S 'GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}')
  fi
  downloaded_bytes=$(
    GITHUB_COM_TOKEN="$hydration_token" "${hydration_environment[@]}" \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "TMPDIR=$temporary_root" \
    LANG=C \
    LC_ALL=C \
      "$python_command" -I -S - "$missing_oids" "$response_dir" "$curl_command" "$temporary_root" "$description" "$remaining_count" "$remaining_bytes" "$hydration_deadline_file" <<'PY'
import base64
import hashlib
import json
import os
import re
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path

oids_path, response_dir, curl, temporary_root, description, maximum_missing_blobs, maximum_total_bytes, deadline_path = sys.argv[1:]
token = os.environ.pop("GITHUB_COM_TOKEN", None) or None
maximum_missing_blobs = int(maximum_missing_blobs)
maximum_total_bytes = int(maximum_total_bytes)
maximum_blob_bytes = 4 * 1024 * 1024
maximum_json_bytes = 6 * 1024 * 1024
maximum_header_bytes = 32 * 1024
phase_deadline = float(Path(deadline_path).read_text())
endpoint = "https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/"
redirect_statuses = {300, 301, 302, 303, 304, 305, 307, 308}

def reject(message):
    raise SystemExit(f"generated-workflows: {description} blob hydration {message}")

oids = Path(oids_path).read_text().splitlines()
if len(oids) > maximum_missing_blobs:
    reject("exceeds the missing object count limit")
if len(set(oids)) != len(oids) or any(re.fullmatch(r"[0-9a-f]{40}", oid) is None for oid in oids):
    reject("received an invalid verified object list")

def terminate(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1)

def response_headers(raw):
    if not raw:
        reject("response headers have an invalid size")
    if len(raw) > maximum_header_bytes:
        reject("response headers exceed the size limit")
    try:
        lines = raw.decode("iso-8859-1").replace("\r\n", "\n").split("\n")
    except UnicodeError:
        reject("response headers are malformed")
    status_lines = [line for line in lines if line.startswith("HTTP/")]
    if len(status_lines) != 1:
        reject("response has ambiguous status headers")
    match = re.fullmatch(r"HTTP/[0-9]+(?:\.[0-9]+)?[ \t]+([1-5][0-9]{2})(?:[ \t].*)?", status_lines[0])
    if match is None:
        reject("response has malformed status headers")
    seen = set()
    locations = []
    for line in lines[lines.index(status_lines[0]) + 1:]:
        if not line:
            continue
        if line[0] in " \t" or ":" not in line:
            reject("response headers are malformed")
        name, value = line.split(":", 1)
        lowered = name.lower()
        token_punctuation = "!#$%&" + chr(39) + "*+-.^_" + chr(96) + "|~"
        if re.fullmatch("[0-9A-Za-z" + re.escape(token_punctuation) + "]+", name) is None or lowered in seen:
            reject("response headers are ambiguous")
        seen.add(lowered)
        if lowered == "location":
            location = value.strip(" \t")
            if not location or any(ord(character) < 32 or ord(character) == 127 for character in location):
                reject("redirect location is malformed")
            locations.append(location)
    return int(match.group(1)), locations

def fetch(oid):
    if time.monotonic() >= phase_deadline:
        reject("exceeded the hydration time limit")
    current_url = endpoint + oid
    if re.fullmatch(re.escape(endpoint) + r"[0-9a-f]{40}", current_url) is None:
        reject("constructed an invalid fixed endpoint")
    with tempfile.TemporaryDirectory(prefix="trusted-blob-http-", dir=temporary_root) as temporary:
        header_path = Path(temporary) / "headers"
        command = [
            curl,
            "--disable",
            "--proto",
            "=https",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-filesize",
            str(maximum_json_bytes),
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "X-GitHub-Api-Version: 2022-11-28",
            current_url,
        ]
        if token:
            command[2:2] = ["--include", "--variable", "%GITHUB_COM_TOKEN", "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"]
        else:
            command[2:2] = ["--dump-header", str(header_path)]
        environment = {
            "HOME": os.environ["HOME"],
            "PATH": os.environ["PATH"],
            "TMPDIR": os.environ["TMPDIR"],
            "LANG": "C",
            "LC_ALL": "C",
        }
        if token:
            environment["GITHUB_COM_TOKEN"] = token
        try:
            process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        except OSError:
            reject("cannot start the trusted caller curl executable")
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        payload = bytearray()
        request_deadline = min(time.monotonic() + 32, phase_deadline)
        try:
            while True:
                if not token and header_path.exists() and header_path.stat().st_size > maximum_header_bytes:
                    terminate(process)
                    reject("response headers exceed the size limit")
                remaining = request_deadline - time.monotonic()
                if remaining <= 0:
                    terminate(process)
                    reject("request exceeded the time limit")
                ready = selector.select(min(remaining, 0.1))
                if ready:
                    limit = maximum_json_bytes + (maximum_header_bytes if token else 0)
                    chunk = os.read(process.stdout.fileno(), min(65536, limit + 1 - len(payload)))
                    if not chunk:
                        break
                    payload.extend(chunk)
                    if token:
                        boundary = payload.find(b"\r\n\r\n")
                        if boundary < 0:
                            boundary = payload.find(b"\n\n")
                        if boundary < 0 and len(payload) > maximum_header_bytes:
                            terminate(process)
                            reject("response headers exceed the size limit")
                    if len(payload) > limit:
                        terminate(process)
                        reject("response exceeds the encoded size limit")
            try:
                process.wait(timeout=max(0.1, request_deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                terminate(process)
                reject("request exceeded the time limit")
        finally:
            selector.close()
            if process.poll() is None:
                terminate(process)
        if process.returncode != 0:
            if process.returncode == 63 or len(payload) > maximum_json_bytes:
                reject("response exceeds the encoded size limit")
            reject("cannot retrieve the fixed official blob endpoint")
        if token:
            if token.encode("ascii") in payload:
                reject("response contains the setup credential")
            boundary = payload.find(b"\r\n\r\n")
            separator_size = 4
            if boundary < 0:
                boundary = payload.find(b"\n\n")
                separator_size = 2
            if boundary < 0 or boundary + separator_size > maximum_header_bytes:
                reject("response headers have an invalid size")
            header_bytes = bytes(payload[:boundary + separator_size])
            payload = payload[boundary + separator_size:]
            if len(payload) > maximum_json_bytes:
                reject("response exceeds the encoded size limit")
        else:
            if not header_path.is_file():
                reject("response headers have an invalid size")
            if header_path.stat().st_size > maximum_header_bytes:
                reject("response headers exceed the size limit")
            header_bytes = header_path.read_bytes()
        status, locations = response_headers(header_bytes)
        if status in redirect_statuses or locations:
            reject("redirects are not permitted")
        if status != 200:
            reject(f"endpoint returned HTTP {status}")
        if not payload:
            reject("response is empty")
        return bytes(payload)

def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result

total_decoded = 0
for oid in oids:
    response = fetch(oid)
    try:
        document = json.loads(
            response.decode("utf-8", "strict"),
            object_pairs_hook=unique_pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError("invalid JSON constant")),
        )
    except (UnicodeError, ValueError, RecursionError, json.JSONDecodeError):
        reject("response JSON is malformed or ambiguous")
    if not isinstance(document, dict) or document.get("sha") != oid:
        reject("response object identity does not match the requested blob")
    if document.get("encoding") != "base64":
        reject("response encoding is unsupported")
    declared_size = document.get("size")
    if type(declared_size) is not int or declared_size < 0 or declared_size > maximum_blob_bytes:
        reject("response declares an invalid blob size")
    content = document.get("content")
    if not isinstance(content, str):
        reject("response content is not base64 text")
    content_without_crlf = content.replace("\r\n", "")
    if "\r" in content_without_crlf:
        reject("response base64 contains an unsupported line break")
    encoded = content_without_crlf.replace("\n", "")
    try:
        encoded_bytes = encoded.encode("ascii", "strict")
        decoded = base64.b64decode(encoded_bytes, validate=True)
    except (UnicodeError, ValueError):
        reject("response base64 is malformed")
    if token and token.encode("ascii") in decoded:
        reject("response contains the setup credential")
    if len(decoded) != declared_size:
        reject("decoded blob size does not match the declaration")
    if len(decoded) > maximum_blob_bytes:
        reject("decoded blob exceeds the size limit")
    actual_oid = hashlib.sha1(b"blob " + str(len(decoded)).encode() + b"\0" + decoded).hexdigest()
    if actual_oid != oid:
        reject("decoded blob does not match its Git object identity")
    total_decoded += len(decoded)
    if total_decoded > maximum_total_bytes:
        reject("decoded data exceeds the total size limit")
    output = Path(response_dir) / oid
    with output.open("xb") as destination:
        destination.write(decoded)
    output.chmod(0o600)
print(total_decoded)
PY
  ) || return $?
  if [[ ! "$downloaded_bytes" =~ ^[0-9]+$ ]]; then
    fail_materialization "blob hydration returned an invalid invocation budget update"
    return 1
  fi
  printf '%s %s\n' "$((used_count + missing_count))" "$((used_bytes + downloaded_bytes))" > "$hydration_budget_file" || return $?
  writer_environment=(
    "HOME=$writer_home"
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin"
    "GIT_DIR=$writer"
    "GIT_OBJECT_DIRECTORY=$verified_blob_objects"
    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_GLOBAL=/dev/null
    GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1
    GIT_NO_LAZY_FETCH=1
    GIT_OPTIONAL_LOCKS=0
  )
  while IFS= read -r object_id || [[ -n "$object_id" ]]; do
    verified_oid=$(env -i "${writer_environment[@]}" "$git_command" hash-object --stdin < "$response_dir/$object_id") || {
      printf 'generated-workflows: %s blob hydration could not recheck the verified object privately\n' "$description" >&2
      return 1
    }
    if [[ "$verified_oid" != "$object_id" ]]; then
      printf 'generated-workflows: %s blob hydration private object identity changed before publication\n' "$description" >&2
      return 1
    fi
    written_oid=$(env -i "${writer_environment[@]}" "$git_command" hash-object -w --stdin < "$response_dir/$object_id") || {
      printf 'generated-workflows: %s blob hydration could not write the verified object privately\n' "$description" >&2
      return 1
    }
    if [[ "$written_oid" != "$object_id" ]]; then
      printf 'generated-workflows: %s blob hydration private object identity changed\n' "$description" >&2
      return 1
    fi
  done < "$missing_oids"
}

read_link_target() {
  local target
  target=$(readlink "$1" || exit $?; printf '\001') || return $?
  target=${target%$'\001'}
  target=${target%$'\n'}
  printf '%s' "$target"
}

validate_symlink_containment() {
  local snapshot=$1
  local link_path=$2
  local target
  local resolved
  local queue
  local component
  local candidate
  local link_depth=0

  target=$(read_link_target "$snapshot/$link_path") || return $?
  if [[ -z "$target" || "$target" == /* ]]; then
    fail_materialization "symlink target leaves snapshot"
    return 1
  fi

  resolved=${link_path%/*}
  [[ "$resolved" == "$link_path" ]] && resolved=
  queue=$target
  while [[ -n "$queue" ]]; do
    if [[ "$queue" == */* ]]; then
      component=${queue%%/*}
      queue=${queue#*/}
    else
      component=$queue
      queue=
    fi
    case "$component" in
      ''|.) ;;
      ..)
        if [[ -z "$resolved" ]]; then
          fail_materialization "symlink target leaves snapshot"
          return 1
        fi
        if [[ "$resolved" == */* ]]; then
          resolved=${resolved%/*}
        else
          resolved=
        fi
        ;;
      *)
        if [[ -n "$resolved" ]]; then
          candidate="$resolved/$component"
        else
          candidate=$component
        fi
        if [[ -L "$snapshot/$candidate" ]]; then
          link_depth=$((link_depth + 1))
          if [[ "$link_depth" -gt 40 ]]; then
            fail_materialization "symlink chain cannot be resolved within snapshot"
            return 1
          fi
          target=$(read_link_target "$snapshot/$candidate") || return $?
          if [[ -z "$target" || "$target" == /* ]]; then
            fail_materialization "symlink target leaves snapshot"
            return 1
          fi
          resolved=${candidate%/*}
          [[ "$resolved" == "$candidate" ]] && resolved=
          if [[ -n "$queue" ]]; then
            queue="$target/$queue"
          else
            queue=$target
          fi
        else
          resolved=$candidate
        fi
        ;;
    esac
  done
}

validate_snapshot_symlinks() {
  local snapshot=$1
  local path
  local links_file
  local status
  links_file=$(mktemp "$temporary_root/symlinks.XXXXXX") || return $?
  if ! find "$snapshot" -type l -print0 > "$links_file"; then
    rm -f "$links_file"
    return 1
  fi
  while IFS= read -r -d '' path || [[ -n "$path" ]]; do
    validate_symlink_containment "$snapshot" "${path#"$snapshot"/}" || {
      status=$?
      return "$status"
    }
  done < "$links_file"
  rm -f "$links_file" || return $?
}

create_parent_directories() {
  local root=$1
  local path=$2
  local parent_path
  local current
  local component

  case "$path" in
    /*|.|..|./*|../*|*/./*|*/.|*/../*|*/..|.git|.git/*|*//*|'')
      fail_materialization "unsafe path"
      return 1
      ;;
  esac

  parent_path=${path%/*}
  if [[ "$parent_path" == "$path" ]]; then
    return 0
  fi

  current=$root
  while [[ -n "$parent_path" ]]; do
    case "$parent_path" in
      */*)
        component=${parent_path%%/*}
        parent_path=${parent_path#*/}
        ;;
      *)
        component=$parent_path
        parent_path=
        ;;
    esac
    current="$current/$component"
    if [[ -L "$current" ]]; then
      fail_materialization "path traverses a symlink"
      return 1
    fi
    if [[ -e "$current" ]]; then
      if [[ ! -d "$current" ]]; then
        fail_materialization "path traverses a non-directory"
        return 1
      fi
    else
      mkdir "$current" || return $?
    fi
  done
}

materialize_tree() {
  local snapshot=$1
  local entries=$2
  local reader=${3-}
  local entry
  local header
  local path
  local mode
  local type
  local object_id
  local target
  local link_file
  local link_target
  local entry_number=0
  local materialized_bytes=0
  local copied_bytes
  local blob_limit

  while IFS= read -r -d '' entry || [[ -n "$entry" ]]; do
    header=${entry%%$'\t'*}
    path=${entry#*$'\t'}
    mode=${header%% *}
    header=${header#* }
    type=${header%% *}
    object_id=${header#* }
    entry_number=$((entry_number + 1))

    case "$path" in
      /*|.|..|./*|../*|*/./*|*/.|*/../*|*/..|.git|.git/*|*//*|'')
        fail_materialization "unsafe path"
        return 1
        ;;
    esac

    target="$snapshot/$path"
    create_parent_directories "$snapshot" "$path" || return $?
    if [[ -e "$target" || -L "$target" ]]; then
      fail_materialization "duplicate path"
      return 1
    fi

    blob_limit=$maximum_snapshot_blob_bytes
    case "$path:$object_id" in
      tests/functional/reference-bins/aarch64-apple-darwin/holla:e6c7debc7a4f36855d8c9d0b150b4ff0735dcf03) blob_limit=16835672 ;;
      crates/termrock-functional/tests/functional/reference-bins/aarch64-apple-darwin/holla:e6c7debc7a4f36855d8c9d0b150b4ff0735dcf03) blob_limit=16835672 ;;
      tests/functional/reference-bins/aarch64-apple-darwin/showcase:15d1adf929c8ce69d0e2eddca63a3fab1f40d9b4) blob_limit=20608024 ;;
      crates/termrock-functional/tests/functional/reference-bins/aarch64-apple-darwin/showcase:15d1adf929c8ce69d0e2eddca63a3fab1f40d9b4) blob_limit=20608024 ;;
      tests/functional/reference-bins/aarch64-apple-darwin/pr2-3cbe13c/jackin:7af05a2846a2843e357eb64454e82e51ecd7a36a) blob_limit=20601600 ;;
      crates/termrock-functional/tests/functional/reference-bins/aarch64-apple-darwin/pr2-3cbe13c/jackin:7af05a2846a2843e357eb64454e82e51ecd7a36a) blob_limit=20601600 ;;
    esac
    case "$mode:$type" in
      100644:blob|100755:blob)
        copied_bytes=$(copy_bounded_blob "$reader" "$object_id" "$target" "$((maximum_snapshot_total_bytes - materialized_bytes))" "$blob_limit") || {
          fail_materialization "snapshot contains an unavailable or oversized blob object"
          return 1
        }
        if (( blob_limit != maximum_snapshot_blob_bytes && copied_bytes != blob_limit )); then
          fail_materialization "approved reference blob size differs from its reviewed identity"
          return 1
        fi
        materialized_bytes=$((materialized_bytes + copied_bytes))
        if [[ "$mode" == 100755 ]]; then
          chmod 755 "$target" || return $?
        else
          chmod 644 "$target" || return $?
        fi
        ;;
      120000:blob)
        link_file="$temporary_root/link-target-$entry_number"
        copied_bytes=$(copy_bounded_blob "$reader" "$object_id" "$link_file" "$((maximum_snapshot_total_bytes - materialized_bytes))") || {
          fail_materialization "snapshot contains an unavailable or oversized blob object"
          return 1
        }
        materialized_bytes=$((materialized_bytes + copied_bytes))
        if IFS= read -r -d '' link_target < "$link_file"; then
          fail_materialization "symlink target contains a NUL byte"
          return 1
        fi
        if [[ -z "$link_target" ]]; then
          fail_materialization "symlink target is empty"
          return 1
        fi
        "$python_command" -I -S - "$link_target" "$target" <<'PY' || return $?
import os
import sys

os.symlink(sys.argv[1], sys.argv[2])
PY
        ;;
      160000:commit)
        fail_materialization "snapshot contains unsupported gitlink entries"
        return 1
        ;;
      *)
        fail_materialization "unsupported entry mode $mode"
        return 1
        ;;
    esac
  done < "$entries"
}

read_snapshot_tool_selection() {
  "$python_command" -I -S - "$1" <<'PY'
import re
import sys
import tomllib
from pathlib import Path
import os

snapshot = Path(sys.argv[1])
supported = {
    "0.1.78": {
        "rust": "1.99.1",
        "jactionlint": "2.4.0",
        "shellcheck": "0.11.0",
    },
}
selected = {"github:tailrocks/velnor-new": "velnor-actions", "rust": None, "jactionlint": None, "shellcheck": None}
supported_targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]

def literal_version(value, name):
    if type(value) is not str or re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", value) is None:
        raise SystemExit(f"generated-workflows: snapshot {name} version is not a stable literal release")
    return value

def config_paths():
    found = []
    for directory, names, filenames in os.walk(snapshot, followlinks=False):
        names[:] = [name for name in names if name != ".git" and not (Path(directory) / name).is_symlink()]
        for filename in filenames:
            if filename in {"mise.toml", ".mise.toml", "mise.local.toml", ".mise.local.toml"} or (filename.startswith(("mise.", ".mise.")) and filename.endswith(".toml")):
                found.append(Path(directory) / filename)
    return found

def load_config(path, label):
    if path.is_symlink() or not path.is_file():
        raise SystemExit(f"generated-workflows: snapshot {label} is not a regular file")
    try:
        with path.open("rb") as source:
            return tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"generated-workflows: snapshot {label} is invalid TOML") from error

def check_aliases(config):
    aliases = config.get("tool_alias", {})
    if type(aliases) is not dict:
        raise SystemExit("generated-workflows: snapshot tool_alias is not a table")
    protected = set(selected) | set(selected.values())
    for alias, target in aliases.items():
        if type(alias) is not str or type(target) is not str:
            raise SystemExit("generated-workflows: snapshot tool_alias contains a nonliteral entry")
        if alias in protected or target in protected:
            raise SystemExit("generated-workflows: snapshot aliases a selected tool")

configs = config_paths()
if snapshot / "mise.toml" not in configs:
    raise SystemExit("generated-workflows: snapshot mise.toml is missing")
versions = {}
for path in configs:
    config = load_config(path, "mise config")
    check_aliases(config)
    tools = config.get("tools", {})
    if type(tools) is not dict:
        raise SystemExit("generated-workflows: snapshot tools is not a table")
    for name, value in tools.items():
        if name not in selected:
            continue
        if name == "github:tailrocks/velnor-new":
            if type(value) is not dict or set(value) != {"version", "bin"} or value.get("bin") != selected[name]:
                raise SystemExit("generated-workflows: snapshot Velnor entry has unsupported options or executable")
            version = literal_version(value.get("version"), "Velnor")
            key = "velnor"
        elif name == "rust":
            if type(value) is not dict or set(value) - {"version", "profile", "components", "targets", "mr_boxington"}:
                raise SystemExit("generated-workflows: snapshot Rust entry has unsupported options")
            version = literal_version(value.get("version"), "Rust")
            if value.get("profile", "minimal") != "minimal" or value.get("components", ["clippy", "rustfmt"]) != ["clippy", "rustfmt"]:
                raise SystemExit("generated-workflows: snapshot Rust profile or components are unsupported")
            if type(value.get("mr_boxington", False)) is not bool:
                raise SystemExit("generated-workflows: snapshot Rust wrapper option is not a literal boolean")
            targets = value.get("targets", [])
            if type(targets) is not list or targets != supported_targets:
                raise SystemExit("generated-workflows: snapshot Rust targets are outside the reviewed support policy")
            key = name
        else:
            if type(value) is dict:
                if set(value) - {"version"}:
                    raise SystemExit(f"generated-workflows: snapshot {name} entry has unsupported options")
                value = value.get("version")
            version = literal_version(value, name)
            key = name
        if key in versions and versions[key] != version:
            raise SystemExit(f"generated-workflows: snapshot contains conflicting {key} pins")
        versions[key] = version

if set(versions) != {"velnor", "rust", "jactionlint", "shellcheck"}:
    raise SystemExit("generated-workflows: snapshot is missing a selected tool pin")
policy = supported.get(versions["velnor"])
if policy is None or any(versions[name] != policy[name] for name in policy):
    raise SystemExit("generated-workflows: snapshot tool tuple is outside the reviewed support policy")

toolchains = []
for directory, names, filenames in os.walk(snapshot, followlinks=False):
    names[:] = [name for name in names if name != ".git" and not (Path(directory) / name).is_symlink()]
    if "rust-toolchain" in filenames:
        raise SystemExit("generated-workflows: legacy rust-toolchain file is unsupported")
    if "rust-toolchain.toml" in filenames:
        toolchains.append(Path(directory) / "rust-toolchain.toml")
if not toolchains:
    raise SystemExit("generated-workflows: snapshot rust-toolchain.toml is missing")
for path in toolchains:
    config = load_config(path, "rust-toolchain.toml")
    toolchain = config.get("toolchain")
    if type(toolchain) is not dict or set(toolchain) - {"channel", "profile", "components", "targets"}:
        raise SystemExit("generated-workflows: snapshot Rust toolchain declaration is unsupported")
    if toolchain.get("channel") != versions["rust"] or toolchain.get("profile", "minimal") != "minimal":
        raise SystemExit("generated-workflows: snapshot Rust toolchain channel or profile is unsupported")
    if toolchain.get("components", ["clippy", "rustfmt"]) != ["clippy", "rustfmt"]:
        raise SystemExit("generated-workflows: snapshot Rust toolchain components are unsupported")
    targets = toolchain.get("targets", [])
    if type(targets) is not list or targets != supported_targets:
        raise SystemExit("generated-workflows: snapshot Rust toolchain targets are outside the reviewed support policy")

sys.stdout.write("\n".join((versions["velnor"], versions["rust"], versions["jactionlint"], versions["shellcheck"])) + "\n")
PY
}

validate_cargo_inputs() {
  local approved_inputs
  local approved_lock_sha256
  local approved_manifest_fingerprint
  local validation_status
  local validation_mode=$1
  local snapshot=$2
  local cargo_home=$3
  local expected_lock_sha256=${4:-}
  local expected_manifest_fingerprint=${5:-}
  if approved_inputs=$("$python_command" -I -S - "$validation_mode" "$snapshot" "$cargo_home" "$trusted_seed_lock_artifact" "$trusted_seed_policy_artifact" "$trusted_lock_sha256" "$trusted_manifests_sha256" "$trusted_seed_policy_sha256" "$expected_lock_sha256" "$expected_manifest_fingerprint" <<'PY'
import base64
import copy
import hashlib
import json
import os
import re
import stat
import sys
import tomllib
from pathlib import Path, PurePosixPath

mode, snapshot_arg, cargo_home_arg, trusted_lock_arg, policy_arg, trusted_lock_sha256, trusted_manifests_sha256, trusted_policy_sha256, expected_lock_sha256, expected_manifest_fingerprint = sys.argv[1:]
snapshot = Path(snapshot_arg).resolve(strict=True)
cargo_home = Path(cargo_home_arg).resolve(strict=True)
trusted_lock_path = Path(trusted_lock_arg)
policy_path = Path(policy_arg)
manifest_paths = ("Cargo.toml", "apps/jackin/Cargo.toml")
registry_source = "registry+https://github.com/rust-lang/crates.io-index"
max_manifest_bytes = 1048576
max_policy_bytes = 262144
max_policy_total_bytes = 2097152
missing = object()

def reject(message):
    raise SystemExit(f"generated-workflows: {message}")

def toml_equal(left, right):
    if type(left) is not type(right):
        return False
    if type(left) is dict:
        return left.keys() == right.keys() and all(toml_equal(left[key], right[key]) for key in left)
    if type(left) is list:
        return len(left) == len(right) and all(
            toml_equal(left_value, right_value)
            for left_value, right_value in zip(left, right, strict=True)
        )
    return left == right

if mode not in {"trusted-seed", "selected-snapshot"}:
    reject("Cargo input validation mode is unsupported")

def sha256(data):
    return hashlib.sha256(data).hexdigest()

def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            reject("trusted seed Cargo policy artifact contains a duplicate field")
        result[key] = value
    return result

def load_policy():
    descriptor = None
    try:
        descriptor = os.open(policy_path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        info = os.fstat(descriptor)
        if not stat.S_ISREG(info.st_mode) or info.st_size > max_policy_bytes:
            reject("trusted seed Cargo policy artifact is unsafe or oversized")
        with os.fdopen(descriptor, "rb") as source:
            descriptor = None
            raw = source.read(max_policy_bytes + 1)
            if len(raw) > max_policy_bytes or source.read(1):
                reject("trusted seed Cargo policy artifact is unsafe or oversized")
    except OSError as error:
        raise SystemExit("generated-workflows: trusted seed Cargo policy artifact is unavailable") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
    if sha256(raw) != trusted_policy_sha256:
        reject("trusted seed Cargo policy artifact failed its reviewed digest")
    try:
        policy = json.loads(raw.decode("ascii"), object_pairs_hook=unique_object)
    except (UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise SystemExit("generated-workflows: trusted seed Cargo policy artifact is malformed") from error
    if type(policy) is not dict or set(policy) != {"schema", "seed_commit", "seed_tree", "manifest_aggregate_sha256", "manifests"}:
        reject("trusted seed Cargo policy artifact has unsupported fields")
    if policy.get("schema") != 1 or policy.get("seed_commit") != "f4d9999ace7d6deb4294cd89c00afcc80e748d49" or policy.get("seed_tree") != "95406daef6ba234fde62fc47b933fbb8bdfce320" or policy.get("manifest_aggregate_sha256") != trusted_manifests_sha256:
        reject("trusted seed Cargo policy artifact does not match the reviewed source identity")
    records = policy.get("manifests")
    if type(records) is not list or [record.get("path") if type(record) is dict else None for record in records] != list(manifest_paths):
        reject("trusted seed Cargo policy artifact has an unexpected manifest path set")
    result = {}
    total = 0
    aggregate = hashlib.sha256()
    for record in records:
        if type(record) is not dict or set(record) != {"path", "sha256", "bytes_base64"}:
            reject("trusted seed Cargo policy artifact contains an unsupported manifest record")
        path = record["path"]
        digest = record["sha256"]
        encoded = record["bytes_base64"]
        if type(digest) is not str or re.fullmatch(r"[0-9a-f]{64}", digest) is None or type(encoded) is not str:
            reject("trusted seed Cargo policy artifact contains an invalid manifest identity")
        try:
            data = base64.b64decode(encoded, validate=True)
        except (ValueError, base64.binascii.Error) as error:
            raise SystemExit("generated-workflows: trusted seed Cargo policy artifact contains invalid manifest bytes") from error
        total += len(data)
        if len(data) > max_manifest_bytes or total > max_policy_total_bytes or sha256(data) != digest:
            reject("trusted seed Cargo policy artifact manifest bytes do not match their fixed digest")
        aggregate.update(path.encode("utf-8"))
        aggregate.update(b"\0")
        aggregate.update(digest.encode("ascii"))
        aggregate.update(b"\n")
        result[path] = data
    if aggregate.hexdigest() != trusted_manifests_sha256 or len(result) != len(manifest_paths):
        reject("trusted seed Cargo policy artifact aggregate differs from the reviewed manifest identity")
    return result

def load_toml(path, label):
    if path.is_symlink() or not path.is_file():
        reject(f"{label} is not a regular file")
    try:
        with path.open("rb") as source:
            return tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"generated-workflows: {label} is invalid TOML") from error

def load_lock(path, label):
    try:
        data = path.read_bytes()
        lock = tomllib.loads(data.decode("utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"generated-workflows: {label} is invalid TOML") from error
    if type(lock) is not dict or set(lock) != {"version", "package"} or type(lock.get("version")) is not int or lock["version"] != 4:
        reject(f"{label} has unsupported lock metadata")
    packages = lock.get("package")
    if type(packages) is not list:
        reject(f"{label} has no package inventory")
    identities = {}
    local_identities = set()
    for package in packages:
        if type(package) is not dict or set(package) - {"name", "version", "source", "checksum", "dependencies"}:
            reject(f"{label} contains unsupported package metadata")
        name = package.get("name")
        version = package.get("version")
        source = package.get("source")
        if type(name) is not str or re.fullmatch(r"[A-Za-z0-9_-]+", name) is None or type(version) is not str or not version:
            reject(f"{label} contains an invalid package identity")
        if source is not None and (type(source) is not str or not source):
            reject(f"{label} contains an invalid package source")
        identity = (name, version, source)
        if identity in identities:
            reject(f"{label} contains an ambiguous package identity")
        identities[identity] = package
        if source is None:
            local_identities.add(identity)
        dependencies = package.get("dependencies", [])
        if type(dependencies) is not list or any(type(item) is not str for item in dependencies):
            reject(f"{label} contains an invalid dependency list")
    return lock, identities, local_identities, sha256(data), data

def dependency_target(raw, identities, label):
    match = re.fullmatch(r"([A-Za-z0-9_-]+)(?: ([0-9]+(?:\.[0-9]+){2}(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?))?(?: \(([^()]+)\))?", raw)
    if match is None:
        reject(f"{label} contains a malformed dependency identity")
    name, version, source = match.groups()
    if source is not None and version is None:
        reject(f"{label} contains a malformed source-qualified dependency")
    matches = [identity for identity in identities if identity[0] == name and (version is None or identity[1] == version) and (source is None or identity[2] == source)]
    if len(matches) != 1:
        reject(f"{label} contains an unresolved or ambiguous dependency identity")
    return matches[0]

def validate_lock_edges(identities, label):
    result = {}
    for identity, package in identities.items():
        seen = set()
        resolved = {}
        for dependency in package.get("dependencies", []):
            target = dependency_target(dependency, identities, label)
            if target in seen:
                reject(f"{label} contains a duplicate dependency edge")
            seen.add(target)
            resolved[target] = dependency
        result[identity] = resolved
    return result

def validate_snapshot_lock(path, trusted_path, permitted_deltas):
    trusted, trusted_identities, trusted_locals, trusted_digest, trusted_bytes = load_lock(trusted_path, "trusted seed Cargo.lock")
    if trusted_digest != trusted_lock_sha256:
        reject("trusted seed Cargo.lock does not match its reviewed identity")
    candidate, candidate_identities, candidate_locals, candidate_digest, candidate_bytes = load_lock(path, "snapshot Cargo.lock")
    if candidate.keys() != trusted.keys() or not toml_equal(candidate["version"], trusted["version"]):
        reject("Cargo.lock changes reviewed lock-level metadata")
    if len(candidate_identities) != 299 or len(candidate_locals) != 2:
        reject("Cargo.lock changes the reviewed package inventory")
    if set(candidate_identities) != set(trusted_identities):
        reject("Cargo.lock changes the reviewed package inventory")
    if candidate_locals != trusted_locals:
        reject("Cargo.lock changes the reviewed local package identities")
    trusted_edges = validate_lock_edges(trusted_identities, "trusted seed Cargo.lock")
    candidate_edges = validate_lock_edges(candidate_identities, "snapshot Cargo.lock")
    removed_edges = set()
    added_edges = set()
    for identity, trusted_package in trusted_identities.items():
        candidate_package = candidate_identities[identity]
        if identity in trusted_locals:
            trusted_fields = {key: value for key, value in trusted_package.items() if key != "dependencies"}
            candidate_fields = {key: value for key, value in candidate_package.items() if key != "dependencies"}
            if not toml_equal(candidate_fields, trusted_fields):
                reject("Cargo.lock changes a reviewed local package record")
            before = trusted_edges[identity]
            after = candidate_edges[identity]
            for target in before.keys() & after.keys():
                if not toml_equal(before[target], after[target]):
                    reject("Cargo.lock changes a reviewed local dependency edge")
            for target in before.keys() - after.keys():
                removed_edges.add((identity, target))
                if target not in trusted_locals:
                    reject("Cargo.lock removes an external dependency edge")
            for target in after.keys() - before.keys():
                added_edges.add((identity, target))
                if target[2] != registry_source and (identity, target) not in permitted_deltas["added"]:
                    reject("Cargo.lock adds an unreviewed dependency identity")
        elif not toml_equal(candidate_package, trusted_package):
            reject("Cargo.lock changes a reviewed external package record")
    optional_added = permitted_deltas.get("optional_added", False) and not added_edges
    if removed_edges != permitted_deltas["removed"] or (added_edges != permitted_deltas["added"] and not optional_added):
        reject("Cargo.lock local edge changes do not match the selected manifest dependency changes")
    return candidate_digest

def validate_cargo_config(path):
    config = load_toml(path, "Cargo config")
    if set(config) - {"term"}:
        reject("Cargo config contains unsupported settings")
    term = config.get("term", {})
    if type(term) is not dict or set(term) - {"color", "verbose"}:
        reject("Cargo config contains unsupported term settings")
    if "color" in term and (type(term["color"]) is not str or term["color"] not in {"auto", "always", "never"}):
        reject("Cargo config color is not a supported literal")
    if "verbose" in term and type(term["verbose"]) is not bool:
        reject("Cargo config verbose is not a literal boolean")

def effective_config_paths(directory):
    current = directory
    while True:
        for name in ("config", "config.toml"):
            path = current / ".cargo" / name
            if path.exists() or path.is_symlink():
                yield path
        if current.parent == current:
            break
        current = current.parent

def path_is_inside(base, raw_path, label):
    if type(raw_path) is not str or not raw_path or "\\" in raw_path or re.match(r"^[A-Za-z]:", raw_path):
        reject(f"{label} is not a supported relative path")
    relative = PurePosixPath(raw_path)
    if relative.is_absolute():
        reject(f"{label} escapes the snapshot")
    resolved = Path(os.path.realpath(base.joinpath(*relative.parts)))
    try:
        resolved.relative_to(snapshot)
    except ValueError:
        reject(f"{label} escapes the snapshot")

def dependency_sections(data, label):
    result = {}
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        if section in data:
            value = data[section]
            if type(value) is not dict:
                reject(f"{label}.{section} is not a dependency table")
            result[("root", section)] = value
    if "target" in data:
        targets = data["target"]
        if type(targets) is not dict:
            reject(f"{label}.target is not a target table")
        for target_name, target_data in targets.items():
            if type(target_name) is not str or type(target_data) is not dict:
                reject(f"{label}.target contains an unsupported target declaration")
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                if section in target_data:
                    value = target_data[section]
                    if type(value) is not dict:
                        reject(f"{label}.target.{target_name}.{section} is not a dependency table")
                    result[("target", target_name, section)] = value
    return result

def without_dependency_sections(data):
    result = copy.deepcopy(data)
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        result.pop(section, None)
    result.pop("features", None)
    targets = result.get("target")
    if type(targets) is dict:
        for target_name in list(targets):
            target_data = targets[target_name]
            if type(target_data) is dict:
                for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                    target_data.pop(section, None)
                if not target_data:
                    del targets[target_name]
        if not targets:
            result.pop("target", None)
    return result

def package_identity(manifest, label):
    package = manifest.get("package")
    if type(package) is not dict:
        reject(f"{label} has no package identity")
    name = package.get("name")
    version = package.get("version")
    if type(name) is not str or re.fullmatch(r"[A-Za-z0-9_-]+", name) is None or type(version) is not str or not version:
        reject(f"{label} has an invalid package identity")
    return (name, version, None)

def checked_dependency_name(value, label):
    if type(value) is not str or re.fullmatch(r"[A-Za-z0-9_-]+", value) is None:
        reject(f"{label} has an invalid Cargo package name")
    return value

def dependency_deltas(baseline_data, selected_data, trusted_identities, trusted_locals):
    candidate = copy.deepcopy(selected_data)
    root = candidate["Cargo.toml"]
    app = candidate["apps/jackin/Cargo.toml"]
    workspace = root.get("workspace", {}).get("dependencies", {})
    root_dev = root.get("dev-dependencies", {})
    app_dev = app.get("dev-dependencies", {})
    baseline_workspace = baseline_data["Cargo.toml"].get("workspace", {}).get("dependencies", {})
    baseline_bundle_absent = "sha2" not in baseline_workspace and "sha2" not in baseline_data["Cargo.toml"].get("dev-dependencies", {}) and "jackin-console" not in baseline_data["apps/jackin/Cargo.toml"].get("dev-dependencies", {})
    if (baseline_bundle_absent
        and workspace.get("sha2") == "0.10.9"
        and root_dev.get("sha2") == {"workspace": True}
        and app_dev.get("jackin-console") == {"path": ".", "default-features": False, "features": ["demo"]}):
        del workspace["sha2"]
        del root_dev["sha2"]
        del app_dev["jackin-console"]
        if not toml_equal(workspace, baseline_data["Cargo.toml"].get("workspace", {}).get("dependencies", {})):
            reject("Cargo manifests change workspace dependency source declarations")
        if not toml_equal(candidate, baseline_data):
            reject("Cargo manifests change values outside the reviewed dependency bundle")
        root_identity = package_identity(baseline_data["Cargo.toml"], "Cargo.toml")
        app_identity = package_identity(baseline_data["apps/jackin/Cargo.toml"], "apps/jackin/Cargo.toml")
        sha_identity = ("sha2", "0.10.9", registry_source)
        if root_identity not in trusted_locals or app_identity not in trusted_locals or sha_identity not in trusted_identities:
            reject("reviewed dependency bundle does not match trusted package identities")
        return {"removed": set(), "added": {(root_identity, sha_identity), (app_identity, app_identity)}, "removed_keys": set(), "optional_added": True}
    removed = set()
    added = set()
    removed_declarations = set()
    added_declarations = set()
    removed_keys = set()
    for path in manifest_paths:
        label = path
        baseline_package = baseline_data[path]
        selected_package = selected_data[path]
        baseline_workspace = baseline_package.get("workspace", {})
        selected_workspace = selected_package.get("workspace", {})
        if type(baseline_workspace) is not dict or type(selected_workspace) is not dict:
            reject("Cargo manifests change protected workspace values")
        if not toml_equal(baseline_workspace.get("dependencies", {}), selected_workspace.get("dependencies", {})):
            reject("Cargo manifests change workspace dependency source declarations")
        if not toml_equal(without_dependency_sections(baseline_package), without_dependency_sections(selected_package)):
            reject("Cargo manifests change protected nondependency values")
        owner = package_identity(baseline_package, label)
        if package_identity(selected_package, label) != owner or owner not in trusted_locals:
            reject("Cargo manifests change reviewed local package identities")
        before_tables = dependency_sections(baseline_package, label)
        after_tables = dependency_sections(selected_package, label)
        for table_path in before_tables.keys() | after_tables.keys():
            before = before_tables.get(table_path, {})
            after = after_tables.get(table_path, {})
            for dependency_name in before.keys() | after.keys():
                old_value = before.get(dependency_name, missing)
                new_value = after.get(dependency_name, missing)
                if toml_equal(old_value, new_value):
                    continue
                if old_value is not missing and new_value is missing:
                    before_locations = [key for key, table in before_tables.items() if dependency_name in table]
                    after_locations = [key for key, table in after_tables.items() if dependency_name in table]
                    if len(before_locations) != 1 or after_locations:
                        reject("Cargo manifest removes an ambiguous local dependency key")
                    target = validate_local_removal(path, dependency_name, old_value, trusted_identities)
                    removed.add((owner, target))
                    removed_declarations.add((path, table_path, dependency_name))
                    removed_keys.add((path, dependency_name))
                    continue
                if old_value is missing and new_value is not missing:
                    target = validate_external_addition(dependency_name, new_value, trusted_identities)
                    added.add((owner, target))
                    added_declarations.add((path, table_path, dependency_name))
                    continue
                reject("Cargo manifests change an existing dependency declaration")
    if len(removed_declarations) > 1 or len(added_declarations) > 1 or (removed_declarations and added_declarations):
        reject("Cargo manifests exceed the reviewed dependency edge change limit")
    return {"removed": removed, "added": added, "removed_keys": removed_keys}

def validate_feature_changes(baseline_data, selected_data, removed_keys):
    keys_by_manifest = {}
    for path, dependency_name in removed_keys:
        keys_by_manifest.setdefault(path, set()).add(dependency_name)
    for path in manifest_paths:
        baseline_features = baseline_data[path].get("features", {})
        selected_features = selected_data[path].get("features", {})
        if type(baseline_features) is not dict or type(selected_features) is not dict:
            reject("Cargo manifests change protected feature declarations")
        if baseline_features.keys() != selected_features.keys():
            reject("Cargo manifests change protected feature keys")
        removed_dependency_keys = keys_by_manifest.get(path, set())
        for feature_name, baseline_values in baseline_features.items():
            selected_values = selected_features[feature_name]
            if type(baseline_values) is not list or any(type(value) is not str for value in baseline_values) or type(selected_values) is not list or any(type(value) is not str for value in selected_values):
                reject("Cargo manifests contain unsupported feature declarations")
            expected_values = [
                value
                for value in baseline_values
                if not any(re.fullmatch(re.escape(dependency_name) + r"/[A-Za-z0-9_-]+", value) for dependency_name in removed_dependency_keys)
            ]
            if not toml_equal(selected_values, expected_values):
                reject("Cargo manifests change feature values outside a removed local dependency reference")

def validate_local_removal(manifest_path, dependency_name, spec, trusted_identities):
    label = f"{manifest_path}.{dependency_name}"
    if type(spec) is not dict or "path" not in spec or set(spec) - {"path", "package", "default-features", "features", "optional"}:
        reject("Cargo manifest removes an unsupported local dependency edge")
    raw_path = spec["path"]
    if type(raw_path) is not str:
        reject("Cargo manifest removes a local dependency with a nonliteral path")
    base = (snapshot / manifest_path).parent
    path_is_inside(base, raw_path, f"{label}.path")
    resolved = Path(os.path.realpath(base.joinpath(*PurePosixPath(raw_path).parts)))
    if not resolved.is_dir() or resolved.is_symlink():
        reject("Cargo manifest removes a local dependency with an unresolved path")
    try:
        relative_manifest = (resolved / "Cargo.toml").relative_to(snapshot).as_posix()
    except ValueError:
        reject("Cargo manifest removes a local dependency that leaves the snapshot")
    if relative_manifest not in manifest_paths:
        reject("Cargo manifest removes a local dependency outside the reviewed workspace")
    target_manifest = tomllib.loads(baseline_manifest_bytes[relative_manifest].decode("utf-8"))
    target_identity = package_identity(target_manifest, f"trusted seed {relative_manifest}")
    target_name = target_identity[0]
    declared_package = spec.get("package", dependency_name)
    if checked_dependency_name(declared_package, label) != target_name:
        reject("Cargo manifest removes a local dependency with an ambiguous package alias")
    if "default-features" in spec and type(spec["default-features"]) is not bool:
        reject("Cargo manifest removes a local dependency with an invalid default-features value")
    if "optional" in spec and type(spec["optional"]) is not bool:
        reject("Cargo manifest removes a local dependency with an invalid optional value")
    if "features" in spec and (type(spec["features"]) is not list or any(type(feature) is not str for feature in spec["features"]) or len(spec["features"]) != len(set(spec["features"]))):
        reject("Cargo manifest removes a local dependency with an invalid feature list")
    matches = [identity for identity in trusted_identities if identity == target_identity]
    if len(matches) != 1:
        reject("Cargo manifest local dependency does not resolve to a unique reviewed package")
    return matches[0]

def validate_external_addition(dependency_name, spec, trusted_identities):
    label = dependency_name
    if type(spec) is not dict or set(spec) not in ({"version", "features"}, {"version", "features", "package"}):
        reject("Cargo manifest adds an unsupported dependency source declaration")
    version = spec.get("version")
    features = spec.get("features")
    if type(version) is not str or re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", version) is None:
        reject("Cargo manifest adds a dependency without an exact stable version")
    if type(features) is not list or any(type(feature) is not str or not feature for feature in features) or len(features) != len(set(features)):
        reject("Cargo manifest adds a dependency with an invalid literal feature list")
    package_name = checked_dependency_name(spec.get("package", dependency_name), label)
    matches = [identity for identity in trusted_identities if identity[0] == package_name and identity[1] == version and identity[2] == registry_source]
    if len(matches) != 1:
        reject("Cargo manifest added registry dependency does not match a unique reviewed lock record")
    return matches[0]

def inspect_paths(value, base, location):
    if isinstance(value, dict):
        for key, item in value.items():
            label = f"{location}.{key}"
            if key in {"path", "readme", "license-file"} and isinstance(item, str):
                path_is_inside(base, item, label)
            elif key == "build" and isinstance(item, str):
                path_is_inside(base, item, label)
            elif key in {"include", "exclude"} and isinstance(item, list):
                for entry in item:
                    path_is_inside(base, entry, label)
            inspect_paths(item, base, label)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            inspect_paths(item, base, f"{location}[{index}]")

config_files = set()
manifests = []
cargo_locks = []
for directory, names, filenames in os.walk(snapshot, followlinks=False):
    current = Path(directory)
    for name in tuple(names):
        child = current / name
        if name == ".git":
            names.remove(name)
        elif child.is_symlink() and child.is_dir():
            reject("Cargo inputs are nested beneath a symlink directory")
    for filename in filenames:
        path = current / filename
        if path.parent.name == ".cargo" and filename in {"config", "config.toml"}:
            config_files.add(path)
        if filename == "Cargo.toml":
            manifests.append(path)
        if filename == "Cargo.lock":
            cargo_locks.append(path)
for manifest in manifests:
    for path in effective_config_paths(manifest.parent):
        config_files.add(path)
for path in effective_config_paths(snapshot):
    config_files.add(path)
for path in sorted(config_files):
    validate_cargo_config(path)
for name in ("config", "config.toml"):
    path = cargo_home / name
    if path.exists() or path.is_symlink():
        validate_cargo_config(path)

if not manifests or any(path.is_symlink() or not path.is_file() for path in manifests):
    reject("Cargo manifest is symlinked or is not a regular file")
baseline_manifest_bytes = load_policy()
candidate_manifest_bytes = {}
candidate_data = {}
baseline_data = {}
manifest_digest = hashlib.sha256()
for path in sorted(manifests, key=lambda item: item.relative_to(snapshot).as_posix()):
    relative = path.relative_to(snapshot).as_posix()
    if path.stat().st_size > max_manifest_bytes:
        reject("Cargo manifest exceeds the reviewed size limit")
    try:
        data = path.read_bytes()
        parsed = tomllib.loads(data.decode("utf-8"))
        baseline = tomllib.loads(baseline_manifest_bytes[relative].decode("utf-8"))
    except KeyError:
        reject("Cargo manifest path set is outside the reviewed workspace")
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"generated-workflows: Cargo manifest is invalid TOML: {relative}") from error
    candidate_manifest_bytes[relative] = data
    candidate_data[relative] = parsed
    baseline_data[relative] = baseline
    digest = sha256(data)
    manifest_digest.update(relative.encode("utf-8"))
    manifest_digest.update(b"\0")
    manifest_digest.update(digest.encode("ascii"))
    manifest_digest.update(b"\n")
candidate_manifest_fingerprint = manifest_digest.hexdigest()
if set(candidate_manifest_bytes) != set(manifest_paths):
    reject("Cargo manifest path set is outside the reviewed workspace")
trusted_lock_data, trusted_identities, trusted_locals, trusted_lock_digest, trusted_lock_bytes = load_lock(trusted_lock_path, "trusted seed Cargo.lock")
if trusted_lock_digest != trusted_lock_sha256 or len(trusted_identities) != 299 or len(trusted_locals) != 2:
    reject("trusted seed Cargo.lock does not match its reviewed identity")
validate_lock_edges(trusted_identities, "trusted seed Cargo.lock")
if mode == "trusted-seed":
    if candidate_manifest_fingerprint != trusted_manifests_sha256 or any(candidate_manifest_bytes[path] != baseline_manifest_bytes[path] for path in manifest_paths):
        reject("trusted seed Cargo manifests do not match their reviewed byte identity")
    permitted_deltas = {"removed": set(), "added": set(), "removed_keys": set()}
else:
    permitted_deltas = dependency_deltas(baseline_data, candidate_data, trusted_identities, trusted_locals)
lock_path = snapshot / "Cargo.lock"
if lock_path.is_symlink() or not lock_path.is_file():
    reject("snapshot Cargo.lock is not a regular file")
if lock_path not in cargo_locks:
    reject("snapshot Cargo.lock was not discovered as a regular input")
root_lock_sha256 = ""
for path in cargo_locks:
    if path.is_symlink() or not path.is_file():
        reject("nested Cargo.lock is not a regular file")
    if path == lock_path and trusted_lock_path is not None:
        if trusted_lock_path.is_symlink() or not trusted_lock_path.is_file():
            reject("trusted seed Cargo.lock is not a regular file")
        if mode == "trusted-seed":
            try:
                candidate_lock_bytes = path.read_bytes()
            except OSError as error:
                raise SystemExit("generated-workflows: trusted seed Cargo.lock is unreadable") from error
            if sha256(candidate_lock_bytes) != trusted_lock_sha256:
                reject("trusted seed Cargo.lock does not match its reviewed byte identity")
        root_lock_sha256 = validate_snapshot_lock(path, trusted_lock_path, permitted_deltas)
        if mode == "selected-snapshot":
            validate_feature_changes(baseline_data, candidate_data, permitted_deltas["removed_keys"])
    else:
        try:
            lock_bytes = path.read_bytes()
        except OSError as error:
            raise SystemExit("generated-workflows: Cargo.lock is unreadable") from error
        lock_digest = hashlib.sha256(lock_bytes).hexdigest()
        try:
            candidate_lock = tomllib.loads(lock_bytes.decode("utf-8"))
            trusted_lock_bytes = trusted_lock_path.read_bytes()
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            raise SystemExit("generated-workflows: nested Cargo.lock is invalid TOML") from error
        if not toml_equal(candidate_lock, tomllib.loads(trusted_lock_bytes.decode("utf-8"))) or lock_digest != trusted_lock_sha256:
            reject("Cargo.lock is outside the reviewed dependency support policy")
        if path == lock_path:
            root_lock_sha256 = lock_digest
for manifest in manifests:
    if manifest.is_symlink():
        reject("Cargo manifest is symlinked")
    data = load_toml(manifest, "Cargo manifest")
    base = manifest.parent
    inspect_paths(data, base, str(manifest.relative_to(snapshot)))
    workspace = data.get("workspace", {})
    if type(workspace) is dict:
        for key in ("members", "default-members", "exclude"):
            values = workspace.get(key, [])
            if type(values) is not list:
                reject("Cargo workspace paths are not an array")
            for value in values:
                path_is_inside(base, value, f"{manifest.relative_to(snapshot)} workspace {key}")
if not re.fullmatch(r"[0-9a-f]{64}", root_lock_sha256):
    reject("snapshot Cargo.lock digest is unavailable after policy validation")
if expected_lock_sha256 and (not re.fullmatch(r"[0-9a-f]{64}", expected_lock_sha256) or root_lock_sha256 != expected_lock_sha256):
    reject("snapshot Cargo.lock changed after policy validation")
if expected_manifest_fingerprint and (not re.fullmatch(r"[0-9a-f]{64}", expected_manifest_fingerprint) or candidate_manifest_fingerprint != expected_manifest_fingerprint):
    reject("snapshot Cargo manifests changed after policy validation")
if mode == "trusted-seed" and root_lock_sha256 != trusted_lock_sha256:
    reject("trusted seed Cargo.lock does not match its reviewed byte identity")
sys.stdout.write(root_lock_sha256 + "\t" + candidate_manifest_fingerprint + "\n")
PY
  ); then
    IFS=$'\t' read -r approved_lock_sha256 approved_manifest_fingerprint <<< "$approved_inputs"
    if [[ ! "$approved_lock_sha256" =~ ^[0-9a-f]{64}$ ]]; then
      printf 'generated-workflows: Cargo lock policy returned an invalid candidate digest\n' >&2
      return 1
    fi
    if [[ ! "$approved_manifest_fingerprint" =~ ^[0-9a-f]{64}$ ]]; then
      printf 'generated-workflows: Cargo manifest policy returned an invalid candidate fingerprint\n' >&2
      return 1
    fi
    validated_cargo_lock_sha256=$approved_lock_sha256
    validated_cargo_manifest_fingerprint=$approved_manifest_fingerprint
  else
    validation_status=$?
    return "$validation_status"
  fi
}

validate_trusted_lock_sources() {
  "$python_command" -I -S - "$1" "$trusted_lock_sha256" <<'PY'
import hashlib
import re
import sys
import tomllib
from pathlib import Path

path = Path(sys.argv[1])
expected_hash = sys.argv[2]
expected_local = {"junie-tui", "jackin-console"}
expected_git_repositories = {"termpane", "tuiscotti"}
registry_source = "registry+https://github.com/rust-lang/crates.io-index"

def reject(message):
    raise SystemExit(f"generated-workflows: trusted seed lock {message}")

try:
    data = path.read_bytes()
    lock = tomllib.loads(data.decode("utf-8"))
except (OSError, UnicodeError, tomllib.TOMLDecodeError):
    reject("is unreadable or invalid TOML")
if hashlib.sha256(data).hexdigest() != expected_hash:
    reject("does not match the reviewed lock identity")
if type(lock.get("version")) is not int or lock["version"] != 4 or set(lock) - {"version", "package", "metadata"}:
    reject("has an unsupported format")
packages = lock.get("package")
if type(packages) is not list:
    reject("has no package inventory")
registry_count = 0
git_repositories = {}
local_names = set()
identities = set()
for package in packages:
    if type(package) is not dict or set(package) - {"name", "version", "source", "checksum", "dependencies"}:
        reject("contains an unsupported package entry")
    name = package.get("name")
    version = package.get("version")
    if type(name) is not str or not name or type(version) is not str or not version:
        reject("contains a package without a literal identity")
    source = package.get("source")
    identity = (name, version, source)
    if identity in identities:
        reject("contains a duplicate package identity")
    identities.add(identity)
    if source == registry_source:
        checksum = package.get("checksum")
        if type(checksum) is not str or re.fullmatch(r"[0-9a-f]{64}", checksum) is None:
            reject("contains a registry package without a valid SHA-256 checksum")
        registry_count += 1
    elif isinstance(source, str) and source.startswith("git+"):
        match = re.fullmatch(r"git\+https://github\.com/tailrocks/(termpane|tuiscotti)\?rev=([0-9a-f]{40})#([0-9a-f]{40})", source)
        if match is None or match.group(2) != match.group(3) or name != match.group(1) or "checksum" in package:
            reject("contains an unexpected Git package identity")
        repository = match.group(1)
        if repository in git_repositories and git_repositories[repository] != source:
            reject("contains multiple revisions for one Git source")
        git_repositories[repository] = source
    elif source is None:
        if "checksum" in package or name not in expected_local:
            reject("contains an unsupported local package")
        local_names.add(name)
    else:
        reject("contains an unsupported registry or Git source")
if registry_count == 0 or set(git_repositories) != expected_git_repositories or local_names != expected_local:
    reject("does not match the reviewed crates.io, Git, and workspace source inventory")
PY
}

prepare_trusted_seed() {
  local seed=$1
  local reader=$2
  local entries=$3
  local projected_entries="$temporary_root/trusted-seed-projected-tree-entries"
  local commit_type
  local tree
  local tree_type
  local seed_selection
  local expected_selection=$4
  local cargo_home=$5

  if [[ "$object_format" != sha1 ]]; then
    printf 'generated-workflows: trusted seed requires the reviewed SHA-1 Git object format\n' >&2
    return 1
  fi
  initialize_trusted_reader "$reader" || return $?
  commit_type=$(git_in_trusted_reader "$reader" cat-file -t "$trusted_seed_commit" 2>/dev/null) || {
    printf 'generated-workflows: reviewed trusted seed commit object is unavailable\n' >&2
    return 1
  }
  if [[ "$commit_type" != commit ]]; then
    printf 'generated-workflows: reviewed trusted seed object is not a commit\n' >&2
    return 1
  fi
  tree=$(git_in_trusted_reader "$reader" rev-parse --verify "$trusted_seed_commit^{tree}") || {
    printf 'generated-workflows: reviewed trusted seed commit tree is unavailable\n' >&2
    return 1
  }
  tree_type=$(git_in_trusted_reader "$reader" cat-file -t "$tree" 2>/dev/null) || {
    printf 'generated-workflows: reviewed trusted seed tree object is unavailable\n' >&2
    return 1
  }
  if [[ "$tree_type" != tree ]]; then
    printf 'generated-workflows: reviewed trusted seed object is not a tree\n' >&2
    return 1
  fi
  bounded_tree_listing trusted "$reader" "$tree" "$entries" || {
    printf 'generated-workflows: cannot read the complete reviewed trusted seed tree\n' >&2
    return 1
  }
  validate_trusted_tree_entries "$entries" || return $?
  project_trusted_tree_entries "$entries" "$projected_entries" || return $?
  preflight_tree_blob_sizes trusted "$reader" "$projected_entries" true || return $?
  hydrate_blobs trusted "$reader" "$projected_entries" trusted-seed "trusted seed" || return $?
  preflight_tree_blob_sizes trusted "$reader" "$projected_entries" false || return $?
  mkdir -p "$seed" || return $?
  materialize_tree "$seed" "$projected_entries" "$reader" || {
    printf 'generated-workflows: cannot materialize the reviewed trusted seed\n' >&2
    return 1
  }
  validate_snapshot_symlinks "$seed" || return $?
  validate_cargo_inputs trusted-seed "$seed" "$cargo_home" "$trusted_lock_sha256" "$trusted_manifests_sha256" || return $?
  validate_trusted_lock_sources "$seed/Cargo.lock" || return $?
  seed_selection=$(read_snapshot_tool_selection "$seed") || {
    printf 'generated-workflows: trusted seed toolchain inputs are unsupported\n' >&2
    return 1
  }
  if [[ -n "$expected_selection" && "$seed_selection" != "$expected_selection" ]]; then
    printf 'generated-workflows: trusted seed toolchain does not match the reviewed snapshot pins\n' >&2
    return 1
  fi
  find "$seed" -type f -exec chmod a-w {} + || return $?
  find "$seed" -type d -exec chmod a-w {} + || return $?
}

trusted_seed_inventory() {
  "$python_command" -I -S - "$1" <<'PY'
import hashlib
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
digest = hashlib.sha256()
entries = []

def fail(error):
    raise error

for directory, names, filenames in os.walk(root, followlinks=False, onerror=fail):
    current = Path(directory)
    for name in names + filenames:
        path = current / name
        entries.append(path)
for path in sorted(entries, key=lambda item: os.fsencode(item.relative_to(root))):
    relative = os.fsencode(path.relative_to(root))
    info = path.lstat()
    mode = stat.S_IMODE(info.st_mode)
    if stat.S_ISLNK(info.st_mode):
        kind = b"link"
        content = os.fsencode(os.readlink(path))
    elif stat.S_ISREG(info.st_mode):
        kind = b"file"
        content = path.read_bytes()
    elif stat.S_ISDIR(info.st_mode):
        kind = b"directory"
        content = b""
    else:
        raise SystemExit("generated-workflows: trusted seed contains a special file")
    for field in (relative, kind, str(mode).encode("ascii"), content):
        digest.update(len(field).to_bytes(8, "big"))
        digest.update(field)
print(digest.hexdigest())
PY
}

normalize_tool_cache_symlinks() {
  "$python_command" -I -S - "$1" <<'PY'
import os
import stat
import sys
from pathlib import Path

root_input = Path(sys.argv[1])
root_info = root_input.lstat()
if not stat.S_ISDIR(root_info.st_mode) or stat.S_ISLNK(root_info.st_mode):
    raise SystemExit("generated-workflows: qualified tool cache staging tree is unsafe")
root = root_input.resolve(strict=True)

def fail(error):
    raise error

for directory, names, filenames in os.walk(root, topdown=True, followlinks=False, onerror=fail):
    current = Path(directory)
    for name in names + filenames:
        path = current / name
        info = path.lstat()
        if not stat.S_ISLNK(info.st_mode):
            continue
        link_target = os.readlink(path)
        target = path.resolve(strict=True)
        if not target.is_relative_to(root):
            raise SystemExit("generated-workflows: qualified tool cache has an escaping symlink")
        if os.path.isabs(link_target):
            relative_target = os.path.relpath(target, start=path.parent)
            path.unlink()
            path.symlink_to(relative_target)
            if path.resolve(strict=True) != target:
                raise SystemExit("generated-workflows: qualified tool cache symlink normalization failed")
PY
}

validate_tool_cache() {
  local generation=$1
  local expected_key=$2
  local expected_platform=$3
  local expected_velnor=$4
  local expected_rust=$5
  local expected_jactionlint=$6
  local expected_shellcheck=$7
  local expected_inventory
  local actual_inventory
  if [[ -L "$generation" || ! -d "$generation" || -L "$generation/manifest" || ! -f "$generation/manifest" || -L "$generation/tool-state" || ! -d "$generation/tool-state" ]]; then
    printf 'generated-workflows: qualified tool cache generation is missing or unsafe; run trusted setup from reviewed code\n' >&2
    return 1
  fi
  for field in "key=$expected_key" "platform=$expected_platform" "velnor=$expected_velnor" "rust=$expected_rust" "jactionlint=$expected_jactionlint" "shellcheck=$expected_shellcheck" "seed_commit=$trusted_seed_commit" "trusted_lock_sha256=$trusted_lock_sha256" "trusted_seed_cargo_policy_sha256=$trusted_seed_policy_sha256" "checker_version=2" "checker_sha256=${checker_digest:-test}" "config_sha256=$expected_config_sha256" "mise_version=2026.11.4" "mise_sha256=$mise_sha256" "python_version=3.15.2" "python_artifact_sha256=$python_artifact_sha256" "python_lock_sha256=$python_lock_sha256" "python_mise_sha256=$expected_python_mise_sha256" "inventory_helper_sha256=$expected_inventory_helper_sha256"; do
    if ! /usr/bin/grep -F -x -q "$field" "$generation/manifest"; then
      printf 'generated-workflows: qualified tool cache manifest does not match its reviewed identity\n' >&2
      return 1
    fi
  done
  expected_inventory=$(/usr/bin/sed -n 's/^payload_sha256=//p' "$generation/manifest")
  if [[ ! "$expected_inventory" =~ ^[0-9a-f]{64}$ ]]; then
    printf 'generated-workflows: qualified tool cache manifest has no payload inventory digest\n' >&2
    return 1
  fi
  actual_inventory=$(trusted_seed_inventory "$generation/tool-state") || return $?
  if [[ "$actual_inventory" != "$expected_inventory" ]]; then
    printf 'generated-workflows: qualified tool cache payload changed; refusing to execute it\n' >&2
    return 1
  fi
  "$python_command" -I -S - "$generation" <<'PY'
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
tool_state = (root / "tool-state").resolve(strict=True)
owner = os.getuid()
for directory in (root, root.parent, root.parent.parent, root.parent.parent.parent):
    info = directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode) or info.st_uid != owner or info.st_mode & 0o077:
        raise SystemExit("generated-workflows: qualified tool cache directory chain is unsafe")
def fail(error):
    raise error

for directory, names, filenames in os.walk(root, followlinks=False, onerror=fail):
    current = Path(directory)
    for name in names + filenames:
        path = current / name
        info = path.lstat()
        if info.st_uid != owner or stat.S_ISREG(info.st_mode) and info.st_mode & 0o222 or stat.S_ISDIR(info.st_mode) and info.st_mode & 0o222:
            raise SystemExit("generated-workflows: qualified tool cache has unsafe owner or writable contents")
        if stat.S_ISLNK(info.st_mode):
            target = path.resolve(strict=True)
            if not target.is_relative_to(tool_state):
                raise SystemExit("generated-workflows: qualified tool cache has an escaping symlink")
        elif not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)):
            raise SystemExit("generated-workflows: qualified tool cache contains a special file")
PY
}

publish_tool_cache() {
  local generation=$1
  local staging=$2
  local key=$3
  local host_platform=$4
  local velnor=$5
  local rust=$6
  local jactionlint=$7
  local shellcheck=$8
  local payload_sha256
  local cache_parent=${generation%/*}
  mkdir -p "$staging/tool-state/mise-config" || return $?
  cp "$private_config" "$staging/tool-state/mise-config/mise.toml" || return $?
  cp "$private_lock" "$staging/tool-state/mise-config/mise.lock" || return $?
  normalize_tool_cache_symlinks "$staging/tool-state" || return $?
  find "$staging/tool-state" -type f -exec chmod a-w {} + || return $?
  find "$staging/tool-state" -type d -exec chmod a-w {} + || return $?
  chmod 500 "$staging/tool-state" || return $?
  payload_sha256=$(trusted_seed_inventory "$staging/tool-state") || return $?
  printf 'key=%s\nplatform=%s\nvelnor=%s\nrust=%s\njactionlint=%s\nshellcheck=%s\nseed_commit=%s\ntrusted_lock_sha256=%s\ntrusted_seed_cargo_policy_sha256=%s\nchecker_version=2\nchecker_sha256=%s\nconfig_sha256=%s\nmise_version=2026.11.4\nmise_sha256=%s\npython_version=3.15.2\npython_artifact_sha256=%s\npython_lock_sha256=%s\npython_mise_sha256=%s\ninventory_helper_sha256=%s\npayload_sha256=%s\n' "$key" "$host_platform" "$velnor" "$rust" "$jactionlint" "$shellcheck" "$trusted_seed_commit" "$trusted_lock_sha256" "$trusted_seed_policy_sha256" "${checker_digest:-test}" "$expected_config_sha256" "$mise_sha256" "$python_artifact_sha256" "$python_lock_sha256" "$expected_python_mise_sha256" "$expected_inventory_helper_sha256" "$payload_sha256" > "$staging/manifest" || return $?
  chmod 400 "$staging/manifest" || return $?
  chmod 500 "$staging" || return $?
  mkdir -p "$cache_parent" || return $?
  if [[ -e "$generation" || -L "$generation" ]]; then
    printf 'generated-workflows: qualified tool cache generation appeared during setup\n' >&2
    return 1
  fi
  mv "$staging" "$generation" || return $?
  if ! validate_tool_cache "$generation" "$key" "$host_platform" "$velnor" "$rust" "$jactionlint" "$shellcheck"; then
    /bin/rm -f "$generation/manifest" 2>/dev/null || true
    find "$generation" -type d -exec chmod u+w {} + 2>/dev/null || true
    find "$generation" -type f -exec chmod u+w {} + 2>/dev/null || true
    find "$generation" -depth -delete 2>/dev/null || true
    return 1
  fi
  tool_cache_stage_dir=
}

validate_private_lock() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" "$5" "$6" "$7" <<'PY'
import re
import sys
import tomllib

path, velnor_version, rust_version, jactionlint_version, shellcheck_version, platform, rust_host = sys.argv[1:]
platforms = {
    "macos-arm64": {
        "velnor_suffix": "aarch64-apple-darwin",
        "velnor_asset": "719690385",
        "velnor_checksum": "sha256:c0435f5b4978ae3ecdb030b4870d2a172ea91fecd7ed23e583ecdd1bcd047ef8",
        "jactionlint_suffix": "darwin_arm64",
        "shellcheck_suffix": "darwin.aarch64",
        "shellcheck_asset": "279056932",
    },
    "linux-x64": {
        "velnor_suffix": "x86_64-unknown-linux-gnu",
        "velnor_asset": "719690387",
        "velnor_checksum": "sha256:3caad6b07553a6840496b9a729bfaac86efa8071be2d32268b06a63d2c1aa9e7",
        "jactionlint_suffix": "linux_amd64",
        "shellcheck_suffix": "linux.x86_64",
        "shellcheck_asset": "279056942",
    },
}
identity = platforms.get(platform)
if identity is None:
    raise SystemExit("generated-workflows: current platform has no reviewed tool artifacts")

try:
    with open(path, "rb") as lock_file:
        lock = tomllib.load(lock_file)
except (OSError, tomllib.TOMLDecodeError) as error:
    raise SystemExit("generated-workflows: private tool lock is invalid TOML") from error

tools = lock.get("tools")
expected_keys = {"github:tailrocks/velnor-new", "rust", "jactionlint", "shellcheck"}
if type(tools) is not dict or set(tools) != expected_keys:
    raise SystemExit("generated-workflows: private lock contains missing or unexpected tools")

def entry_for(name, version, backend):
    entries = tools.get(name)
    if type(entries) is not list or len(entries) != 1:
        raise SystemExit(f"generated-workflows: private lock does not contain one {name} release")
    entry = entries[0]
    if entry.get("version") != version or entry.get("backend") != backend:
        raise SystemExit(f"generated-workflows: private lock selected an unsupported {name} backend or version")
    return entry

def platform_entry(entry):
    result = entry.get(f"platforms.{platform}")
    if type(result) is not dict:
        raise SystemExit("generated-workflows: private tool lock has no current platform artifact")
    if re.fullmatch(r"sha256:[0-9a-f]{64}", str(result.get("checksum", ""))) is None:
        raise SystemExit("generated-workflows: private tool lock has no SHA-256 artifact identity")
    return result

velnor = entry_for("github:tailrocks/velnor-new", velnor_version, "github:tailrocks/velnor-new")
velnor_platform = platform_entry(velnor)
velnor_url = f"https://github.com/tailrocks/velnor-new/releases/download/v{velnor_version}/velnor-actions-{velnor_version}-{identity['velnor_suffix']}"
velnor_api = f"https://api.github.com/repos/tailrocks/velnor-new/releases/assets/{identity['velnor_asset']}"
if velnor_platform.get("url") != velnor_url or velnor_platform.get("url_api") != velnor_api or velnor_platform.get("checksum") != identity["velnor_checksum"] or velnor_platform.get("provenance") != "github-attestations":
    raise SystemExit("generated-workflows: private Velnor lock does not match the authenticated official release")

jactionlint = entry_for("jactionlint", jactionlint_version, "packslip:github.com/jdx/jactionlint")
jactionlint_platform = platform_entry(jactionlint)
jactionlint_url = f"https://github.com/jdx/jactionlint/releases/download/v{jactionlint_version}/jactionlint_{jactionlint_version}_{identity['jactionlint_suffix']}.tar.gz"
if jactionlint_platform.get("url") != jactionlint_url or jactionlint_platform.get("signer") != "sigstore-oidc:https://github.com/jdx/jactionlint/.github/workflows/release.yaml" or jactionlint_platform.get("repository_ids") != {"repository": "1404951382"}:
    raise SystemExit("generated-workflows: private jactionlint lock lacks the official release identity")

shellcheck = entry_for("shellcheck", shellcheck_version, "aqua:koalaman/shellcheck")
shellcheck_platform = platform_entry(shellcheck)
shellcheck_url = f"https://github.com/koalaman/shellcheck/releases/download/v{shellcheck_version}/shellcheck-v{shellcheck_version}.{identity['shellcheck_suffix']}.tar.xz"
shellcheck_api = f"https://api.github.com/repos/koalaman/shellcheck/releases/assets/{identity['shellcheck_asset']}"
if shellcheck_platform.get("url") != shellcheck_url or shellcheck_platform.get("url_api") != shellcheck_api:
    raise SystemExit("generated-workflows: private shellcheck lock does not match the official release asset")

rust = entry_for("rust", rust_version, "core:rust")
options = rust.get("options")
if type(options) is not dict or options != {"components": "clippy,rustfmt", "profile": "minimal", "targets": rust_host}:
    raise SystemExit("generated-workflows: private Rust lock has unsupported components or host targets")

print("authenticated private tool lock validated")
PY
}

validate_private_tool_path() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" <<'PY'
import os
import sys
from pathlib import Path

kind, state_value, data_value, cargo_value, rustup_value, root_value, executable_value, name, version = sys.argv[1:]

def inside(path, parent):
    try:
        return os.path.commonpath((str(path), str(parent))) == str(parent)
    except ValueError:
        return False

def shown(value):
    return ascii(str(value)[:240])

def fail(message, **values):
    details = " ".join(f"{key}={shown(value)}" for key, value in values.items())
    raise SystemExit("generated-workflows: " + message + (" " + details if details else ""))

def resolve(label, value):
    try:
        return Path(value).resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve {label}", path=value, errno=error.errno)

state = resolve("private resolver state", state_value)
data = resolve("private Mise data directory", data_value)
cargo = resolve("private Cargo home", cargo_value)
rustup = resolve("private Rustup home", rustup_value)

if not all(inside(path, state) for path in (data, cargo, rustup)):
    fail("private tool roots leave the resolver state", state=state, data=data, cargo=cargo, rustup=rustup)

root = Path(root_value)
if kind != "rust-sysroot" and not root.is_absolute():
    fail("Mise returned a relative tool root", reported_root=root)
root_lexical = Path(os.path.abspath(root))
if kind != "rust-sysroot":
    root_real = resolve("Mise-reported tool root", root)
    installs = resolve("private Mise install directory", data / "installs")
    if not inside(root_lexical, installs):
        fail("Mise-reported tool root is outside private installs", reported_root=root_lexical, private_installs=installs)

    def validate_install_root(canonical_install_root=True):
        if root_lexical.name != version or (canonical_install_root and (root_real.name != version or not inside(root_real, installs))):
            fail(
                f"{name} Mise root does not match its authenticated private pin",
                reported_root=root_lexical,
                canonical_root=root_real,
                private_installs=installs,
                expected_version=version,
            )

if kind == "rust-root":
    validate_install_root(False)
    cargo_bin = resolve("private Cargo proxy directory", cargo / "bin")
    if root_real != cargo_bin or not inside(cargo_bin, cargo):
        fail("Rust Mise root does not resolve to the private Cargo proxy directory", reported_root=root_lexical, canonical_root=root_real, cargo_bin=cargo_bin)
elif kind == "tool-root":
    validate_install_root()
elif kind == "rust-executable":
    validate_install_root(False)
    executable = Path(executable_value)
    cargo_bin = resolve("private Cargo proxy directory", cargo / "bin")
    if not executable.is_absolute() or executable.name != name:
        fail(f"Mise selected an unexpected Rust {name} executable", executable=executable, reported_root=root_lexical)
    resolved = resolve(f"Mise-selected Rust {name} proxy", executable)
    if not resolved.is_file() or not os.access(resolved, os.X_OK) or not inside(resolved, cargo_bin):
        fail(f"Rust {name} executable leaves the private Cargo proxy directory", executable=executable, canonical_executable=resolved, cargo_bin=cargo_bin)
elif kind == "tool-executable":
    validate_install_root()
    executable = Path(executable_value)
    if not executable.is_absolute() or executable.name != name:
        fail(f"Mise selected an unexpected {name} executable path", executable=executable, reported_root=root_lexical)
    resolved = resolve(f"Mise-selected {name} executable", executable)
    if not resolved.is_file() or not os.access(resolved, os.X_OK) or not inside(resolved, root_real):
        fail(f"{name} executable leaves its authenticated private install", executable=executable, canonical_executable=resolved, authenticated_root=root_real)
elif kind == "rust-sysroot":
    actual = Path(executable_value)
    toolchains = resolve("private Rustup toolchains directory", rustup / "toolchains")
    expected = toolchains / f"{version}-{name}"
    expected_real = resolve("pinned private Rustup toolchain", expected)
    actual_real = resolve("rustc-reported sysroot", actual)
    expected_leaf = f"{version}-{name}"
    if actual_real.name != expected_leaf or not inside(actual_real, toolchains) or actual_real != expected_real or not inside(expected_real, state):
        fail("rustc sysroot is outside the pinned private Rustup toolchain", reported_sysroot=actual, canonical_sysroot=actual_real, private_toolchains=toolchains, expected_toolchain=expected_leaf)
else:
    fail("invalid private tool path validation mode", mode=kind)
PY
}

validate_trusted_git_source_trees() {
  local lock_path=$1
  local home_dir=$2
  local runtime_tmp=$3
  local safe_path=$4
  local metadata_environment=(/usr/bin/env -i)
  if [[ "$installed_checker" -eq 1 && "${prepare_cache_mode:-0}" -eq 1 && -n "$trusted_setup_token" ]]; then
    metadata_environment+=(-S 'GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}')
  fi
  metadata_environment+=(\
    "HOME=$home_dir" \
    "PATH=$safe_path" \
    "TMPDIR=$runtime_tmp" \
    LANG=C \
    LC_ALL=C)
  GITHUB_COM_TOKEN="$trusted_setup_token" "${metadata_environment[@]}" "$python_command" -I -S - "$lock_path" "$curl_command" "$trusted_lock_sha256" <<'PY'
import hashlib
import json
import os
import re
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import urljoin, urlsplit

lock_path, curl, expected_hash = sys.argv[1:]
token = os.environ.pop("GITHUB_COM_TOKEN", None) or None

def fail(message):
    raise SystemExit(f"generated-workflows: {message}")

redirect_statuses = {301, 302, 303, 307, 308}
maximum_redirects = 3
maximum_header_bytes = 65536
maximum_body_bytes = 8 * 1024 * 1024

try:
    import tomllib
    lock_bytes = Path(lock_path).read_bytes()
    lock = tomllib.loads(lock_bytes.decode("utf-8"))
except (OSError, UnicodeError, tomllib.TOMLDecodeError):
    fail("reviewed Cargo lock is unavailable for Git source qualification")
if hashlib.sha256(lock_bytes).hexdigest() != expected_hash:
    fail("reviewed Cargo lock does not match its authenticated identity")
packages = lock.get("package")
if type(packages) is not list:
    fail("reviewed Cargo lock has no package inventory")
expected = {}
repositories = {}
for package in packages:
    if not isinstance(package, dict):
        fail("reviewed Cargo lock contains an invalid package record")
    source = package.get("source")
    if not isinstance(source, str) or not source.startswith("git+"):
        continue
    match = re.fullmatch(r"git\+https://github\.com/tailrocks/(termpane|tuiscotti)\?rev=([0-9a-f]{40})#([0-9a-f]{40})", source)
    if match is None or match.group(2) != match.group(3) or package.get("name") != match.group(1):
        fail("reviewed Cargo lock contains an unsupported Git source identity")
    repository = match.group(1)
    if repository in repositories and repositories[repository] != source:
        fail("reviewed Cargo lock contains multiple revisions for one Git source")
    repositories[repository] = source
    expected[source] = ("tailrocks", repository, match.group(2))
if set(repositories) != {"termpane", "tuiscotti"}:
    fail("reviewed Cargo lock Git source set does not match its allowlist")
locked_sources = set(expected)

def validate_url(url, expected_url):
    try:
        parsed = urlsplit(url)
        port = parsed.port
    except ValueError:
        fail("trusted Git source metadata URL is malformed")
    expected_parts = urlsplit(expected_url)
    if parsed.scheme != "https" or parsed.netloc != "api.github.com" or parsed.hostname != "api.github.com" or port not in (None, 443) or parsed.username is not None or parsed.password is not None or parsed.path != expected_parts.path or parsed.query != expected_parts.query or parsed.fragment or "\\" in url:
        fail("trusted Git source metadata URL is outside the official API path")
    if any(ord(character) < 32 or ord(character) == 127 for character in url):
        fail("trusted Git source metadata URL contains control characters")

def parse_headers(raw):
    if not raw or len(raw) > maximum_header_bytes:
        fail("trusted Git source response headers have an invalid size")
    if b"\r\n" in raw:
        normalized = raw.replace(b"\r\n", b"\n")
        if b"\r" in normalized:
            fail("trusted Git source response has malformed headers")
    else:
        normalized = raw
    lines = normalized.decode("iso-8859-1").split("\n")
    status_lines = [line for line in lines if line.startswith("HTTP/")]
    if len(status_lines) != 1 or not lines or status_lines[0] != lines[0]:
        fail("trusted Git source response has ambiguous status headers")
    match = re.fullmatch(r"HTTP/[0-9]+(?:\.[0-9]+)?[ \t]+([1-5][0-9]{2})(?:[ \t][\t\x20-\x7e]*)?", status_lines[0])
    if match is None:
        fail("trusted Git source response has malformed status headers")
    locations = []
    for line in lines[lines.index(status_lines[0]) + 1:]:
        if not line:
            continue
        if line[0] in " \t" or ":" not in line:
            fail("trusted Git source response has malformed headers")
        name, value = line.split(":", 1)
        if re.fullmatch(r"[!#$%&'*+\-.^_`|~0-9A-Za-z]+", name) is None or any((ord(character) < 32 and character != "\t") or ord(character) == 127 for character in value):
            fail("trusted Git source response has malformed headers")
        if name.lower() == "location":
            location = value.strip(" \t")
            if not location or any(ord(character) < 32 or ord(character) == 127 for character in location):
                fail("trusted Git source redirect location is malformed")
            locations.append(location)
    if len(locations) > 1:
        fail("trusted Git source response has ambiguous redirect locations")
    return int(match.group(1)), locations

def read_headers(path):
    return parse_headers(path.read_bytes())

def parse_included_response(raw):
    delimiter = raw.find(b"\r\n\r\n")
    delimiter_size = 4
    if delimiter < 0:
        delimiter = raw.find(b"\n\n")
        delimiter_size = 2
    if delimiter < 0 or delimiter > maximum_header_bytes:
        fail("trusted Git source response headers have an invalid size")
    status, locations = parse_headers(raw[:delimiter])
    body = raw[delimiter + delimiter_size:]
    if len(body) > maximum_body_bytes:
        fail("trusted Git source response exceeds the size limit")
    return status, locations, body

def terminate(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1)

def fetch(url):
    current_url = url
    seen = set()
    for redirect_count in range(maximum_redirects + 1):
        validate_url(current_url, url)
        if current_url in seen:
            fail("trusted Git source metadata redirect loop detected")
        seen.add(current_url)
        with tempfile.TemporaryDirectory(prefix="git-source-metadata-", dir=os.environ["TMPDIR"]) as temporary:
            header_path = Path(temporary) / "headers"
            command = [
                curl,
                "--disable",
                "--proto",
                "=https",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "--max-filesize",
                str(maximum_body_bytes),
                "--header",
                "Accept: application/vnd.github+json",
                "--header",
                "X-GitHub-Api-Version: 2022-11-28",
            ]
            if token:
                command[2:2] = ["--include", "--variable", "%GITHUB_COM_TOKEN", "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"]
            else:
                command[2:2] = ["--dump-header", str(header_path)]
            command.append(current_url)
            environment = {name: os.environ[name] for name in ("HOME", "PATH", "TMPDIR", "LANG", "LC_ALL")}
            if token:
                environment["GITHUB_COM_TOKEN"] = token
            try:
                process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            except OSError:
                fail("cannot start the trusted caller curl executable for Git source metadata")
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            payload = bytearray()
            deadline = time.monotonic() + 32
            maximum_response_bytes = maximum_body_bytes + (maximum_header_bytes + 4 if token else 0)
            try:
                while True:
                    if not token and header_path.exists() and header_path.stat().st_size > maximum_header_bytes:
                        terminate(process)
                        fail("trusted Git source response headers exceed the size limit")
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        terminate(process)
                        fail("trusted Git source metadata request exceeded the time limit")
                    ready = selector.select(min(remaining, 0.1))
                    if ready:
                        chunk = os.read(process.stdout.fileno(), min(65536, maximum_response_bytes + 1 - len(payload)))
                        if not chunk:
                            break
                        payload.extend(chunk)
                        if len(payload) > maximum_response_bytes:
                            terminate(process)
                            fail("trusted Git source response exceeds the size limit")
                try:
                    process.wait(timeout=max(0.1, deadline - time.monotonic()))
                except subprocess.TimeoutExpired:
                    terminate(process)
                    fail("trusted Git source metadata request exceeded the time limit")
            finally:
                selector.close()
                if process.poll() is None:
                    terminate(process)
            if process.returncode != 0:
                fail("trusted Git source metadata request failed")
            if token:
                status, locations, body = parse_included_response(bytes(payload))
                if token.encode("ascii") in payload:
                    fail("trusted Git source metadata response contains the setup credential")
            else:
                if not header_path.is_file() or header_path.stat().st_size > maximum_header_bytes:
                    fail("trusted Git source response headers have an invalid size")
                status, locations = read_headers(header_path)
                body = bytes(payload)
            if status in redirect_statuses:
                if not locations:
                    fail("trusted Git source redirect has no location")
                if redirect_count == maximum_redirects:
                    fail("trusted Git source exceeded the redirect limit")
                try:
                    target = urljoin(current_url, locations[0])
                except ValueError:
                    fail("trusted Git source redirect URL is malformed")
                validate_url(target, url)
                current_url = target
                continue
            if status != 200:
                fail(f"trusted Git source metadata returned HTTP status {status}")
            if not body:
                fail("trusted Git source metadata response is empty")
            return body
    fail("trusted Git source exceeded the redirect limit")

def qualify_repo(source, identity):
    owner, repository, commit = identity
    commit_url = f"https://api.github.com/repos/{owner}/{repository}/git/commits/{commit}"
    try:
        commit_data = json.loads(fetch(commit_url))
    except (UnicodeError, json.JSONDecodeError):
        fail("trusted Git source commit metadata is malformed")
    if type(commit_data) is not dict or commit_data.get("sha") != commit:
        fail("trusted Git source commit identity does not match the lock")
    tree_data = commit_data.get("tree")
    tree_sha = tree_data.get("sha") if type(tree_data) is dict else None
    if type(tree_sha) is not str or re.fullmatch(r"[0-9a-f]{40}", tree_sha) is None:
        fail("trusted Git source commit has no valid tree identity")
    tree_url = f"https://api.github.com/repos/{owner}/{repository}/git/trees/{tree_sha}?recursive=1"
    try:
        tree = json.loads(fetch(tree_url))
    except (UnicodeError, json.JSONDecodeError):
        fail("trusted Git source tree metadata is malformed")
    if type(tree) is not dict or tree.get("sha") != tree_sha or tree.get("truncated") is not False or type(tree.get("tree")) is not list:
        fail("trusted Git source tree metadata is incomplete or ambiguous")
    seen = set()
    for entry in tree["tree"]:
        if type(entry) is not dict or type(entry.get("path")) is not str or type(entry.get("mode")) is not str or type(entry.get("type")) is not str or type(entry.get("sha")) is not str:
            fail("trusted Git source tree entry is malformed")
        path = entry["path"]
        components = path.split("/")
        if not path or path.startswith("/") or "\\" in path or any(component in {"", ".", "..", ".git"} for component in components) or any(ord(character) < 32 or ord(character) == 127 for character in path):
            fail("trusted Git source tree contains an unsafe path")
        if path in seen:
            fail("trusted Git source tree contains a duplicate path")
        seen.add(path)
        if path == ".gitmodules" or path.endswith("/.gitmodules"):
            fail("trusted Git source tree contains submodule metadata")
        if entry["mode"] == "160000" or entry["type"] == "commit":
            fail("trusted Git source tree contains a submodule")
        if (entry["mode"], entry["type"]) not in {("040000", "tree"), ("100644", "blob"), ("100755", "blob"), ("120000", "blob")} or re.fullmatch(r"[0-9a-f]{40}", entry["sha"]) is None:
            fail("trusted Git source tree contains an unsupported object entry")

for source in sorted(locked_sources):
    qualify_repo(source, expected[source])
PY
}

validate_cargo_metadata_output() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" "$5" "$6" <<'PY'
import hashlib
import json
import os
import re
import sys
import tomllib
from pathlib import Path

metadata_path, root_value, cargo_home_value, require_git_sources, manifest_value, expected_lock_sha256 = sys.argv[1:]
root = Path(root_value).resolve(strict=True)
cargo_home = Path(cargo_home_value).resolve(strict=True)
manifest_input = Path(manifest_value).resolve(strict=True)
registry_source = "registry+https://github.com/rust-lang/crates.io-index"

def reject(message):
    raise SystemExit(f"generated-workflows: {message}")

def inside(path, parent):
    try:
        return os.path.commonpath((str(path), str(parent))) == str(parent)
    except ValueError:
        return False

try:
    with open(metadata_path, "rb") as source:
        metadata = json.load(source)
except (OSError, UnicodeError, json.JSONDecodeError):
    reject("Cargo metadata output is missing or malformed")
if type(metadata) is not dict or type(metadata.get("packages")) is not list or type(metadata.get("workspace_members")) is not list or type(metadata.get("workspace_root")) is not str:
    reject("Cargo metadata output is incomplete")
try:
    workspace_root = Path(metadata["workspace_root"]).resolve(strict=True)
except OSError:
    reject("Cargo metadata workspace root is unavailable")
if not inside(workspace_root, root):
    reject("Cargo metadata discovered a workspace outside the qualified tree")
lock_path = workspace_root / "Cargo.lock"
if lock_path.is_symlink() or not lock_path.is_file() or hashlib.sha256(lock_path.read_bytes()).hexdigest() != expected_lock_sha256:
    reject("Cargo metadata workspace does not use the reviewed Cargo.lock")
try:
    lock = tomllib.loads(lock_path.read_text(encoding="utf-8"))
except (OSError, UnicodeError, tomllib.TOMLDecodeError):
    reject("Cargo metadata workspace lock is malformed")
packages_from_lock = lock.get("package")
if type(packages_from_lock) is not list:
    reject("Cargo metadata workspace lock has no package inventory")
expected_git = {package.get("source") for package in packages_from_lock if isinstance(package, dict) and isinstance(package.get("source"), str) and package["source"].startswith("git+")}
repositories = set()
for source in expected_git:
    match = re.fullmatch(r"git\+https://github\.com/tailrocks/(termpane|tuiscotti)\?rev=([0-9a-f]{40})#([0-9a-f]{40})", source)
    if match is None or match.group(2) != match.group(3) or match.group(1) in repositories:
        reject("Cargo metadata workspace lock contains an unsupported Git source")
    repositories.add(match.group(1))
if repositories != {"termpane", "tuiscotti"}:
    reject("Cargo metadata workspace lock has an unreviewed Git source set")
packages = metadata["packages"]
if not packages:
    reject("Cargo metadata discovered no packages")
package_ids = set()
workspace_package_ids = set(metadata["workspace_members"])
package_manifests = set()
seen_git_sources = set()
seen_registry = False
for package in packages:
    if type(package) is not dict or type(package.get("id")) is not str or type(package.get("manifest_path")) is not str:
        reject("Cargo metadata contains an invalid package record")
    package_ids.add(package["id"])
    source = package.get("source")
    try:
        manifest = Path(package["manifest_path"]).resolve(strict=True)
    except OSError:
        reject("Cargo metadata package manifest is unavailable")
    package_manifests.add(manifest)
    if source is None:
        if not inside(manifest, root):
            reject("Cargo metadata local package leaves the qualified tree")
    elif source == registry_source:
        if not inside(manifest, cargo_home):
            reject("Cargo metadata registry package leaves the private Cargo home")
        seen_registry = True
    elif source in expected_git:
        if not inside(manifest, cargo_home):
            reject("Cargo metadata Git package leaves the private Cargo home")
        seen_git_sources.add(source)
    else:
        reject("Cargo metadata discovered an unsupported registry or Git source")
if not workspace_package_ids or not workspace_package_ids.issubset(package_ids):
    reject("Cargo metadata workspace membership is incomplete")
if manifest_input != workspace_root / "Cargo.toml" and manifest_input not in package_manifests:
    reject("Cargo metadata omitted a discovered manifest")
for package in packages:
    if package["id"] in workspace_package_ids:
        if package.get("source") is not None:
            reject("Cargo metadata workspace member is not local to the qualified tree")
        if not inside(Path(package["manifest_path"]).resolve(strict=True), workspace_root):
            reject("Cargo metadata workspace member leaves its workspace root")
if require_git_sources == "true" and (seen_git_sources != expected_git or not seen_registry):
    reject("trusted seed metadata omitted a reviewed Git or crates.io source")
PY
}

qualify_cargo_workspaces() {
  local root=$1
  local cargo_executable=$2
  local cargo_home=$3
  local home_dir=$4
  local rustup_home=$5
  local safe_path=$6
  local safe_git_config=$7
  local runtime_tmp=$8
  local rust_pin=$9
  local label=${10}
  local require_git_sources=${11}
  local expected_lock_sha256=${12}
  local validation_mode=${13}
  local expected_manifest_fingerprint=${14}
  local manifest_list="$runtime_tmp/$label-cargo-manifests"
  local manifest
  local metadata_path
  local error_path
  local workspace_count=0
  local manifest_count=0
  local seen_manifest_list="$runtime_tmp/$label-qualified-manifests"
  local -a metadata_environment=(
    "HOME=$home_dir"
    "PATH=$safe_path"
    "TMPDIR=$runtime_tmp"
    "CARGO_HOME=$cargo_home"
    "RUSTUP_HOME=$rustup_home"
    "RUSTUP_TOOLCHAIN=$rust_pin"
    "CARGO_NET_OFFLINE=true"
    "CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse"
    "GIT_CONFIG_GLOBAL=$safe_git_config"
    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1
    GIT_NO_LAZY_FETCH=1
    GIT_OPTIONAL_LOCKS=0
    GIT_TERMINAL_PROMPT=0
    LANG=C
    LC_ALL=C
  )

  validate_cargo_inputs "$validation_mode" "$root" "$cargo_home" "$expected_lock_sha256" "$expected_manifest_fingerprint" || return $?
  if ! find "$root" -type d -name .git -prune -o -type f -name Cargo.toml -print0 > "$manifest_list"; then
    printf 'generated-workflows: cannot enumerate %s Cargo manifests\n' "$label" >&2
    return 1
  fi
  : > "$seen_manifest_list" || return $?
  while IFS= read -r -d '' manifest || [[ -n "$manifest" ]]; do
    manifest_count=$((manifest_count + 1))
    metadata_path="$runtime_tmp/$label-metadata-$manifest_count.json"
    error_path="$runtime_tmp/$label-metadata-$manifest_count.log"
    validate_cargo_inputs "$validation_mode" "$root" "$cargo_home" "$expected_lock_sha256" "$expected_manifest_fingerprint" || return $?
    if ! (cd "$root" && env -i "${metadata_environment[@]}" "$cargo_executable" metadata --locked --offline --all-features --format-version 1 --manifest-path "$manifest") > "$metadata_path" 2> "$error_path"; then
      printf 'generated-workflows: offline Cargo metadata failed for %s manifest %s\n' "$label" "${manifest#"$root"/}" >&2
      "$python_command" -I -S - "$error_path" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
      return 1
    fi
    validate_cargo_inputs "$validation_mode" "$root" "$cargo_home" "$expected_lock_sha256" "$expected_manifest_fingerprint" || return $?
    validate_cargo_metadata_output "$metadata_path" "$root" "$cargo_home" "$require_git_sources" "$manifest" "$expected_lock_sha256" || return $?
    printf '%s\0' "$manifest" >> "$seen_manifest_list" || return $?
    workspace_count=$((workspace_count + 1))
  done < "$manifest_list"
  if [[ "$manifest_count" -eq 0 || "$workspace_count" -ne "$manifest_count" ]]; then
    printf 'generated-workflows: %s Cargo workspace discovery was incomplete\n' "$label" >&2
    return 1
  fi
}

fetch_trusted_seed_dependencies() {
  local seed=$1
  local snapshot=$2
  local cargo_executable=$3
  local cargo_home=$4
  local home_dir=$5
  local rustup_home=$6
  local safe_path=$7
  local safe_git_config=$8
  local runtime_tmp=$9
  local rust_pin=${10}
  local seed_inventory=${11}
  local warm_tool_cache=${12:-false}
  local snapshot_lock_sha256=${13}
  local snapshot_manifest_fingerprint=${14}
  local seed_lock_sha256
  local cargo_log="$runtime_tmp/trusted-cargo-fetch.log"
  local -a cargo_environment=(
    "HOME=$home_dir"
    "PATH=$safe_path"
    "TMPDIR=$runtime_tmp"
    "CARGO_HOME=$cargo_home"
    "RUSTUP_HOME=$rustup_home"
    "RUSTUP_TOOLCHAIN=$rust_pin"
    CARGO_NET_OFFLINE=false
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1
    GIT_NO_LAZY_FETCH=1
    GIT_OPTIONAL_LOCKS=0
    GIT_TERMINAL_PROMPT=0
    LANG=C
    LC_ALL=C
  )

  validate_cargo_inputs trusted-seed "$seed" "$cargo_home" "$trusted_lock_sha256" "$trusted_manifests_sha256" || return $?
  seed_lock_sha256=$validated_cargo_lock_sha256
  validate_trusted_lock_sources "$seed/Cargo.lock" || return $?
  validate_cargo_inputs selected-snapshot "$snapshot" "$cargo_home" "$snapshot_lock_sha256" "$snapshot_manifest_fingerprint" || return $?
  if [[ "$warm_tool_cache" != true ]]; then
    if ! (cd "$seed" && env -i "${cargo_environment[@]}" "$cargo_executable" fetch --locked --manifest-path "$seed/Cargo.toml") > "$cargo_log" 2>&1; then
      printf 'generated-workflows: authenticated Cargo could not fetch the reviewed trusted seed\n' >&2
      "$python_command" -I -S - "$cargo_log" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
      return 1
    fi
  fi
  validate_cargo_inputs selected-snapshot "$snapshot" "$cargo_home" "$snapshot_lock_sha256" "$snapshot_manifest_fingerprint" || return $?
  qualify_cargo_workspaces "$seed" "$cargo_executable" "$cargo_home" "$home_dir" "$rustup_home" "$safe_path" "$safe_git_config" "$runtime_tmp" "$rust_pin" trusted-seed true "$seed_lock_sha256" trusted-seed "$trusted_manifests_sha256" || return $?
  qualify_cargo_workspaces "$snapshot" "$cargo_executable" "$cargo_home" "$home_dir" "$rustup_home" "$safe_path" "$safe_git_config" "$runtime_tmp" "$rust_pin" actual-snapshot false "$snapshot_lock_sha256" selected-snapshot "$snapshot_manifest_fingerprint" || return $?
  local after_inventory
  after_inventory=$(trusted_seed_inventory "$seed") || return $?
  if [[ "$after_inventory" != "$seed_inventory" ]]; then
    printf 'generated-workflows: trusted seed changed during Cargo fetch or metadata qualification\n' >&2
    return 1
  fi
}

install_rustup_bootstrap() {
  local rust_host=$1
  local bootstrap="$downloads_dir/rustup-init-$rust_host"
  local -a bootstrap_environment=(
    "HOME=$home_dir"
    "PATH=$safe_path"
    "TMPDIR=$runtime_tmp"
    LANG=C
    LC_ALL=C
  )
  env -i "${bootstrap_environment[@]}" "$python_command" -I -S - "$rust_host" "$bootstrap" "$curl_command" "$safe_path" "$runtime_tmp" <<'PY' || return 1
import hashlib
import os
import re
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import urljoin, urlsplit

host, destination, curl, safe_path, runtime_tmp = sys.argv[1:]
if host not in {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}:
    raise SystemExit("generated-workflows: rustup bootstrap target is unsupported")
base_path = f"/rustup/dist/{host}"
bootstrap_path = base_path + "/rustup-init"
checksum_path = bootstrap_path + ".sha256"
redirect_statuses = {301, 302, 303, 307, 308}
maximum_redirects = 3
maximum_header_bytes = 65536

def validate_url(url, expected_path):
    try:
        parsed = urlsplit(url)
        port = parsed.port
    except ValueError:
        raise SystemExit("generated-workflows: rustup bootstrap redirect URL is malformed")
    if parsed.scheme != "https" or parsed.hostname != "static.rust-lang.org" or port not in (None, 443) or parsed.username is not None or parsed.password is not None or parsed.path != expected_path or "?" in url or "#" in url or "\\" in url:
        raise SystemExit("generated-workflows: rustup bootstrap URL is outside the approved official distribution path")
    if any(ord(character) < 32 or ord(character) == 127 for character in url):
        raise SystemExit("generated-workflows: rustup bootstrap redirect URL contains control characters")

def response_headers(path):
    raw = path.read_bytes()
    if not raw or len(raw) > maximum_header_bytes:
        raise SystemExit("generated-workflows: rustup bootstrap response headers have an invalid size")
    try:
        lines = raw.decode("iso-8859-1").replace("\r\n", "\n").split("\n")
    except UnicodeError:
        raise SystemExit("generated-workflows: rustup bootstrap response headers are malformed")
    status_lines = [line for line in lines if line.startswith("HTTP/")]
    if len(status_lines) != 1:
        raise SystemExit("generated-workflows: rustup bootstrap response has ambiguous status headers")
    match = re.fullmatch(r"HTTP/[0-9]+(?:\.[0-9]+)?[ \t]+([1-5][0-9]{2})(?:[ \t].*)?", status_lines[0])
    if match is None:
        raise SystemExit("generated-workflows: rustup bootstrap response has malformed status headers")
    locations = []
    for line in lines[lines.index(status_lines[0]) + 1:]:
        if not line:
            continue
        if line[0] in " \t" or ":" not in line:
            raise SystemExit("generated-workflows: rustup bootstrap response has malformed headers")
        name, value = line.split(":", 1)
        if name.lower() == "location":
            location = value.strip(" \t")
            if not location or any(ord(character) < 32 or ord(character) == 127 for character in location):
                raise SystemExit("generated-workflows: rustup bootstrap redirect location is malformed")
            locations.append(location)
    if len(locations) > 1:
        raise SystemExit("generated-workflows: rustup bootstrap response has ambiguous redirect locations")
    return int(match.group(1)), locations

def terminate(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1)

def fetch(initial_url, expected_path, maximum_bytes):
    current_url = initial_url
    seen = set()
    for redirect_count in range(maximum_redirects + 1):
        validate_url(current_url, expected_path)
        if current_url in seen:
            raise SystemExit("generated-workflows: rustup bootstrap redirect loop detected")
        seen.add(current_url)
        with tempfile.TemporaryDirectory(prefix="rustup-fetch-", dir=runtime_tmp) as temporary:
            header_path = Path(temporary) / "headers"
            command = [
                curl,
                "--disable",
                "--proto",
                "=https",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "--max-filesize",
                str(maximum_bytes),
                "--dump-header",
                str(header_path),
                current_url,
            ]
            environment = {
                "HOME": os.environ["HOME"],
                "PATH": safe_path,
                "TMPDIR": runtime_tmp,
                "LANG": "C",
                "LC_ALL": "C",
            }
            try:
                process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            except OSError:
                raise SystemExit("generated-workflows: cannot start the trusted caller curl executable")
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            payload = bytearray()
            deadline = time.monotonic() + 32
            try:
                while True:
                    if header_path.exists() and header_path.stat().st_size > maximum_header_bytes:
                        terminate(process)
                        raise SystemExit("generated-workflows: rustup bootstrap response headers exceed the size limit")
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        terminate(process)
                        raise SystemExit("generated-workflows: rustup bootstrap request exceeded the time limit")
                    ready = selector.select(min(remaining, 0.1))
                    if ready:
                        chunk = os.read(process.stdout.fileno(), min(65536, maximum_bytes + 1 - len(payload)))
                        if not chunk:
                            break
                        payload.extend(chunk)
                        if len(payload) > maximum_bytes:
                            terminate(process)
                            raise SystemExit("generated-workflows: rustup bootstrap response exceeds the size limit")
                try:
                    process.wait(timeout=max(0.1, deadline - time.monotonic()))
                except subprocess.TimeoutExpired:
                    terminate(process)
                    raise SystemExit("generated-workflows: rustup bootstrap request exceeded the time limit")
            finally:
                selector.close()
                if process.poll() is None:
                    terminate(process)
            if process.returncode != 0:
                raise SystemExit("generated-workflows: cannot retrieve official Rustup bootstrap")
            if not header_path.is_file() or header_path.stat().st_size > maximum_header_bytes:
                raise SystemExit("generated-workflows: rustup bootstrap response headers have an invalid size")
            status, locations = response_headers(header_path)
            if status in redirect_statuses:
                if not locations:
                    raise SystemExit("generated-workflows: rustup bootstrap redirect has no location")
                if redirect_count == maximum_redirects:
                    raise SystemExit("generated-workflows: rustup bootstrap exceeded the redirect limit")
                try:
                    target = urljoin(current_url, locations[0])
                except ValueError:
                    raise SystemExit("generated-workflows: rustup bootstrap redirect URL is malformed")
                validate_url(target, expected_path)
                current_url = target
                continue
            if status != 200:
                raise SystemExit("generated-workflows: rustup bootstrap response has an unexpected status")
            if not payload:
                raise SystemExit("generated-workflows: rustup bootstrap response is empty")
            return bytes(payload)
    raise SystemExit("generated-workflows: rustup bootstrap exceeded the redirect limit")

checksum_url = "https://static.rust-lang.org" + checksum_path
binary_url = "https://static.rust-lang.org" + bootstrap_path
checksum_bytes = fetch(checksum_url, checksum_path, 4096)
try:
    checksum_text = checksum_bytes.decode("ascii").strip()
except UnicodeError:
    raise SystemExit("generated-workflows: official Rustup checksum manifest is malformed")
match = re.fullmatch(r"([0-9a-f]{64})[ \t]+\*?(?:\./)?rustup-init", checksum_text)
if match is None:
    raise SystemExit("generated-workflows: official Rustup checksum manifest is malformed")
binary = fetch(binary_url, bootstrap_path, 64 * 1024 * 1024)
if hashlib.sha256(binary).hexdigest() != match.group(1):
    raise SystemExit("generated-workflows: rustup bootstrap checksum does not match the official manifest")
destination_path = Path(destination)
destination_path.write_bytes(binary)
destination_path.chmod(0o700)
print(f"rustup-init sha256:{match.group(1)}")
PY
  env -i \
    "HOME=$home_dir" \
    "PATH=$safe_path" \
    "TMPDIR=$runtime_tmp" \
    "CARGO_HOME=$cargo_home" \
    "RUSTUP_HOME=$rustup_home" \
    "RUSTUP_DIST_SERVER=https://static.rust-lang.org" \
    "RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup" \
    LANG=C \
    LC_ALL=C \
    "$bootstrap" -y --no-modify-path --default-host "$rust_host" --default-toolchain none > "$downloads_dir/rustup-bootstrap.log" 2>&1 || {
      printf 'generated-workflows: authenticated Rustup bootstrap failed\n' >&2
      if [[ -f "$downloads_dir/rustup-bootstrap.log" ]]; then
        "$python_command" -I -S - "$downloads_dir/rustup-bootstrap.log" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
      fi
      return 1
    }
  if [[ ! -x "$cargo_home/bin/rustup" || ! -f "$rustup_home/settings.toml" ]]; then
    printf 'generated-workflows: authenticated Rustup bootstrap did not create private state\n' >&2
    return 1
  fi
}

run_generator() {
  local snapshot=$1
  local preview=$2
  local label=$3
  local selection
  local tool_pin
  local rust_pin
  local jactionlint_pin
  local shellcheck_pin
  local platform
  local rust_host
  local tool_cache_identity
  local tool_cache_generation
  local tool_state_dir
  local cache_stage_dir=
  local warm_tool_cache=false
  local cached_mise_config
  local state_dir="$temporary_root/$label-mise-state"
  local config_dir="$temporary_root/$label-mise-config"
  local data_dir
  local cache_dir="$temporary_root/$label-mise-cache"
  local downloads_dir="$temporary_root/$label-mise-downloads"
  local home_dir="$temporary_root/$label-home"
  local runtime_tmp="$temporary_root/$label-runtime-tmp"
  local xdg_config="$temporary_root/$label-xdg-config"
  local xdg_data="$temporary_root/$label-xdg-data"
  local xdg_cache="$temporary_root/$label-xdg-cache"
  local cargo_home
  local rustup_home
  local trusted_seed="$temporary_root/$label-trusted-seed"
  local trusted_reader="$temporary_root/$label-trusted-source-reader.git"
  local trusted_entries="$temporary_root/$label-trusted-seed-tree-entries"
  local private_config="$config_dir/mise.toml"
  local private_lock="$config_dir/mise.lock"
  local safe_git_config="$config_dir/gitconfig"
  local safe_path
  local generator_path
  local tool_bin
  local tool_dir
  local rust_dir
  local jactionlint_dir
  local shellcheck_dir
  local tool_executable
  local rust_cargo_executable
  local rustc_executable
  local jactionlint_executable
  local shellcheck_executable
  local rust_sysroot
  local rustc_version
  local cargo_version
  local jactionlint_version_output
  local jactionlint_reported_version
  local shellcheck_version_output
  local shellcheck_reported_version
  local trusted_seed_inventory_value
  local validated_snapshot_lock_sha256
  local validated_snapshot_manifest_fingerprint
  local -a mise_environment
  local -a generator_environment

  selection=$(read_snapshot_tool_selection "$snapshot") || {
    printf 'generated-workflows: cannot read reviewed snapshot tool data\n' >&2
    return 1
  }
  tool_pin=$(sed -n '1p' <<< "$selection") || return $?
  rust_pin=$(sed -n '2p' <<< "$selection") || return $?
  jactionlint_pin=$(sed -n '3p' <<< "$selection") || return $?
  shellcheck_pin=$(sed -n '4p' <<< "$selection") || return $?
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) platform=linux-x64; rust_host=x86_64-unknown-linux-gnu ;;
    Darwin:arm64|Darwin:aarch64) platform=macos-arm64; rust_host=aarch64-apple-darwin ;;
    *) printf 'generated-workflows: unsupported host platform for authenticated tools\n' >&2; return 1 ;;
  esac
  tool_cache_identity=$("$python_command" -I -S - "${checker_digest:-test}" "2" "$platform" "$tool_pin" "$rust_pin" "$jactionlint_pin" "$shellcheck_pin" "$trusted_seed_commit" "$trusted_lock_sha256" "2026.11.4" "$mise_sha256" "3.15.2" "$python_artifact_sha256" "$python_lock_sha256" "$expected_python_mise_sha256" "$expected_config_sha256" "$expected_inventory_helper_sha256" "$trusted_seed_policy_sha256" <<'PY'
import hashlib
import sys
print(hashlib.sha256("\n".join(sys.argv[1:]).encode("ascii")).hexdigest())
PY
) || return $?
  if [[ "$installed_checker" -eq 1 ]]; then
    local tool_cache_parent="$install_root/cache/tools"
    tool_cache_generation="$tool_cache_parent/$tool_cache_identity"
    if [[ -e "$tool_cache_generation" || -L "$tool_cache_generation" ]]; then
      validate_tool_cache "$tool_cache_generation" "$tool_cache_identity" "$platform" "$tool_pin" "$rust_pin" "$jactionlint_pin" "$shellcheck_pin" || return $?
      warm_tool_cache=true
      tool_state_dir="$tool_cache_generation/tool-state"
    elif [[ "${prepare_cache_mode:-0}" -eq 1 ]]; then
      mkdir -p "$tool_cache_parent" || return $?
      if [[ -L "$tool_cache_parent" || ! -d "$tool_cache_parent" ]]; then
        printf 'generated-workflows: private tool cache parent is unsafe\n' >&2
        return 1
      fi
      cache_stage_dir="$tool_cache_parent/.stage.$$.${RANDOM}"
      if ! mkdir "$cache_stage_dir"; then
        printf 'generated-workflows: cannot create private tool cache staging directory\n' >&2
        return 1
      fi
      tool_cache_stage_dir=$cache_stage_dir
      tool_state_dir="$cache_stage_dir/tool-state"
    else
      printf 'generated-workflows: qualified tool cache is unavailable; run bash scripts/install-generated-workflow-guard.sh from reviewed code\n' >&2
      return 1
    fi
  else
    tool_state_dir="$temporary_root/$label-tool-state"
  fi
  cached_mise_config="$tool_state_dir/mise-config"
  data_dir="$tool_state_dir/mise-data"
  cargo_home="$tool_state_dir/cargo-home"
  rustup_home="$tool_state_dir/rustup-home"
  mkdir -p "$state_dir" "$config_dir" "$data_dir" "$cache_dir" "$downloads_dir" "$home_dir" "$runtime_tmp" "$xdg_config" "$xdg_data" "$xdg_cache" "$cargo_home" "$rustup_home" || return $?
  : > "$config_dir/global.toml" || return $?
  : > "$config_dir/system.toml" || return $?
  : > "$safe_git_config" || return $?

  (
    if [[ ! -f "$snapshot/mise.toml" || -L "$snapshot/mise.toml" ]]; then
      printf 'generated-workflows: missing regular snapshot config mise.toml\n' >&2
      exit 1
    fi
    selection=$(read_snapshot_tool_selection "$snapshot") || {
      printf 'generated-workflows: cannot read reviewed snapshot tool data\n' >&2
      exit 1
    }
    tool_pin=$(sed -n '1p' <<< "$selection") || exit $?
    rust_pin=$(sed -n '2p' <<< "$selection") || exit $?
    jactionlint_pin=$(sed -n '3p' <<< "$selection") || exit $?
    shellcheck_pin=$(sed -n '4p' <<< "$selection") || exit $?
    tool_bin=velnor-actions
    case "$(uname -s):$(uname -m)" in
      Linux:x86_64|Linux:amd64) platform=linux-x64 ;;
      Darwin:arm64|Darwin:aarch64) platform=macos-arm64 ;;
      *)
        printf 'generated-workflows: unsupported host platform for authenticated tools\n' >&2
        exit 1
        ;;
    esac
    case "$platform" in
      macos-arm64) rust_host=aarch64-apple-darwin ;;
      linux-x64) rust_host=x86_64-unknown-linux-gnu ;;
    esac
    validate_cargo_inputs selected-snapshot "$snapshot" "$cargo_home" || exit $?
    validated_snapshot_lock_sha256=$validated_cargo_lock_sha256
    validated_snapshot_manifest_fingerprint=$validated_cargo_manifest_fingerprint
    prepare_trusted_seed "$trusted_seed" "$trusted_reader" "$trusted_entries" "$selection" "$cargo_home" || exit $?
    trusted_seed_inventory_value=$(trusted_seed_inventory "$trusted_seed") || exit $?
    validate_cargo_inputs selected-snapshot "$snapshot" "$cargo_home" "$validated_snapshot_lock_sha256" "$validated_snapshot_manifest_fingerprint" || exit $?
    if [[ "$validated_cargo_lock_sha256" != "$validated_snapshot_lock_sha256" || "$validated_cargo_manifest_fingerprint" != "$validated_snapshot_manifest_fingerprint" ]]; then
      printf 'generated-workflows: snapshot Cargo inputs changed after offline policy validation\n' >&2
      exit 1
    fi

    safe_path="${mise_command%/*}:${git_command%/*}:$cargo_home/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    if [[ "$warm_tool_cache" != true ]]; then
      validate_trusted_git_source_trees "$trusted_seed/Cargo.lock" "$home_dir" "$runtime_tmp" "$safe_path" || exit $?
    fi

    cat > "$private_config" <<EOF || exit $?
[tools]
"github:tailrocks/velnor-new" = { version = "$tool_pin", bin = "$tool_bin", github_attestations = true }
jactionlint = "$jactionlint_pin"
shellcheck = "$shellcheck_pin"
rust = { version = "$rust_pin", profile = "minimal", components = ["clippy", "rustfmt"], targets = ["$rust_host"] }
EOF
    chmod 600 "$private_config" || exit $?

    mise_environment=(
      "HOME=$home_dir"
      "PATH=$safe_path"
      "TMPDIR=$runtime_tmp"
      "XDG_CONFIG_HOME=$xdg_config"
      "XDG_DATA_HOME=$xdg_data"
      "XDG_CACHE_HOME=$xdg_cache"
      "CARGO_HOME=$cargo_home"
      "CARGO_TARGET_DIR=$temporary_root/$label-cargo-target"
      "RUSTUP_HOME=$rustup_home"
      "RUSTUP_DIST_SERVER=https://static.rust-lang.org"
      "RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup"
      "MISE_CARGO_HOME=$cargo_home"
      "MISE_RUSTUP_HOME=$rustup_home"
      "MISE_RUST_DEFAULT_HOST=$rust_host"
      "LANG=C"
      "LC_ALL=C"
      "GIT_CONFIG_GLOBAL=$safe_git_config"
      GIT_CONFIG_NOSYSTEM=1
      GIT_NO_REPLACE_OBJECTS=1
      GIT_OPTIONAL_LOCKS=0
      "MISE_CONFIG_DIR=$config_dir"
      "MISE_CONFIG_FILE=$private_config"
      "MISE_GLOBAL_CONFIG_FILE=$config_dir/global.toml"
      "MISE_SYSTEM_CONFIG_FILE=$config_dir/system.toml"
      "MISE_STATE_DIR=$state_dir"
      "MISE_DATA_DIR=$data_dir"
      "MISE_SHIMS_DIR=$temporary_root/$label-shims"
      "MISE_CACHE_DIR=$cache_dir"
      "MISE_DOWNLOADS_DIR=$downloads_dir"
      "MISE_ENABLE_TOOLS=github:tailrocks/velnor-new,rust,jactionlint,shellcheck"
      MISE_ENV=
      MISE_IDIOMATIC_VERSION_FILE_ENABLE_TOOLS=
      MISE_LOCKFILE=1
      MISE_LOCKED_VERIFY_PROVENANCE=1
      MISE_GITHUB_ATTESTATIONS=1
      MISE_GITHUB_GITHUB_ATTESTATIONS=1
      MISE_PARANOID=1
      MISE_PROVENANCE_API_FAILURES_FATAL=1
      MISE_NO_ENV=1
      MISE_NO_HOOKS=1
      MISE_AUTO_INSTALL=0
      MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml
      "MISE_CEILING_PATHS=$temporary_root"
      "MISE_TRUSTED_CONFIG_PATHS=$private_config"
    )

    if [[ "$warm_tool_cache" == true ]]; then
      if [[ ! -f "$cached_mise_config/mise.toml" || -L "$cached_mise_config/mise.toml" || ! -f "$cached_mise_config/mise.lock" || -L "$cached_mise_config/mise.lock" ]]; then
        printf 'generated-workflows: qualified private mise configuration is unavailable\n' >&2
        exit 1
      fi
      cp "$cached_mise_config/mise.toml" "$private_config" || exit $?
      cp "$cached_mise_config/mise.lock" "$private_lock" || exit $?
    else
      install_rustup_bootstrap "$rust_host" || exit $?
      env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" lock --platform "$platform" </dev/null || {
        printf 'generated-workflows: cannot resolve authenticated private tools for Velnor %s\n' "$tool_pin" >&2
        exit 1
      }
    fi
    if [[ ! -f "$private_lock" || -L "$private_lock" ]]; then
      printf 'generated-workflows: authenticated private tool lock was not created\n' >&2
      exit 1
    fi
    validate_private_lock "$private_lock" "$tool_pin" "$rust_pin" "$jactionlint_pin" "$shellcheck_pin" "$platform" "$rust_host" || exit $?

    if [[ "$warm_tool_cache" != true ]]; then
      env -i "${mise_environment[@]}" "$mise_command" --locked --cd "$config_dir" install </dev/null || {
        printf 'generated-workflows: authenticated private tool installation failed\n' >&2
        exit 1
      }
    fi

    tool_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where github:tailrocks/velnor-new) || {
      printf 'generated-workflows: authenticated Velnor is not installed\n' >&2
      exit 1
    }
    if [[ "${tool_dir##*/}" != "$tool_pin" ]]; then
      printf 'generated-workflows: installed Velnor does not match snapshot pin %s\n' "$tool_pin" >&2
      exit 1
    fi
    validate_private_tool_path tool-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$tool_dir" '' github:tailrocks/velnor-new "$tool_pin" || exit $?
    rust_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where rust) || {
      printf 'generated-workflows: authenticated Rust toolchain is not installed\n' >&2
      exit 1
    }
    validate_private_tool_path rust-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rust_dir" '' rust "$rust_pin" || exit $?
    jactionlint_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where jactionlint) || {
      printf 'generated-workflows: authenticated jactionlint is not installed\n' >&2
      exit 1
    }
    if [[ "${jactionlint_dir##*/}" != "$jactionlint_pin" ]]; then
      printf 'generated-workflows: installed jactionlint does not match reviewed pin %s\n' "$jactionlint_pin" >&2
      exit 1
    fi
    validate_private_tool_path tool-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$jactionlint_dir" '' jactionlint "$jactionlint_pin" || exit $?
    shellcheck_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where shellcheck) || {
      printf 'generated-workflows: authenticated shellcheck is not installed\n' >&2
      exit 1
    }
    if [[ "${shellcheck_dir##*/}" != "$shellcheck_pin" ]]; then
      printf 'generated-workflows: installed shellcheck does not match reviewed pin %s\n' "$shellcheck_pin" >&2
      exit 1
    fi
    validate_private_tool_path tool-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$shellcheck_dir" '' shellcheck "$shellcheck_pin" || exit $?
    tool_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which "$tool_bin") || {
      printf 'generated-workflows: authenticated Velnor executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path tool-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$tool_dir" "$tool_executable" "$tool_bin" "$tool_pin" || exit $?
    rust_cargo_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which cargo) || {
      printf 'generated-workflows: pinned Rust cargo executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path rust-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rust_dir" "$rust_cargo_executable" cargo "$rust_pin" || exit $?
    rustc_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which rustc) || {
      printf 'generated-workflows: pinned Rust rustc executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path rust-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rust_dir" "$rustc_executable" rustc "$rust_pin" || exit $?
    jactionlint_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which jactionlint) || {
      printf 'generated-workflows: pinned jactionlint executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path tool-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$jactionlint_dir" "$jactionlint_executable" jactionlint "$jactionlint_pin" || exit $?
    shellcheck_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which shellcheck) || {
      printf 'generated-workflows: pinned shellcheck executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path tool-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$shellcheck_dir" "$shellcheck_executable" shellcheck "$shellcheck_pin" || exit $?
    cd "$snapshot" || exit $?
    rustc_version=$(env -i "${mise_environment[@]}" "$rustc_executable" --version) || {
      printf 'generated-workflows: pinned Rust rustc failed at %s\n' "$rustc_executable" >&2
      exit 1
    }
    case "$rustc_version" in
      "rustc $rust_pin"|"rustc $rust_pin "*) ;;
      *)
        printf 'generated-workflows: rustc at %s reported an unexpected version\n' "$rustc_executable" >&2
        exit 1
        ;;
    esac
    cargo_version=$(env -i "${mise_environment[@]}" "$rust_cargo_executable" --version) || {
      printf 'generated-workflows: pinned Rust cargo failed at %s\n' "$rust_cargo_executable" >&2
      exit 1
    }
    case "$cargo_version" in
      "cargo $rust_pin"|"cargo $rust_pin "*) ;;
      *)
        printf 'generated-workflows: cargo at %s reported an unexpected version\n' "$rust_cargo_executable" >&2
        exit 1
        ;;
    esac
    rust_sysroot=$(env -i "${mise_environment[@]}" "$rustc_executable" --print sysroot) || {
      printf 'generated-workflows: pinned Rust rustc could not report its sysroot at %s\n' "$rustc_executable" >&2
      exit 1
    }
    validate_private_tool_path rust-sysroot "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rustup_home/toolchains/$rust_pin-$rust_host" "$rust_sysroot" "$rust_host" "$rust_pin" || exit $?
    jactionlint_version_output=$(env -i "${mise_environment[@]}" "$jactionlint_executable" --version) || {
      printf 'generated-workflows: pinned jactionlint failed at %s\n' "$jactionlint_executable" >&2
      exit 1
    }
    jactionlint_reported_version=${jactionlint_version_output%%$'\n'*}
    if [[ "$jactionlint_reported_version" != "$jactionlint_pin" ]]; then
      printf 'generated-workflows: jactionlint at %s reported an unexpected version\n' "$jactionlint_executable" >&2
      exit 1
    fi
    shellcheck_version_output=$(env -i "${mise_environment[@]}" "$shellcheck_executable" --version) || {
      printf 'generated-workflows: pinned shellcheck failed at %s\n' "$shellcheck_executable" >&2
      exit 1
    }
    shellcheck_reported_version=$(sed -n 's/^version: //p' <<< "$shellcheck_version_output")
    if [[ "$shellcheck_reported_version" != "$shellcheck_pin" ]]; then
      printf 'generated-workflows: shellcheck at %s reported an unexpected version\n' "$shellcheck_executable" >&2
      exit 1
    fi

    fetch_trusted_seed_dependencies "$trusted_seed" "$snapshot" "$rust_cargo_executable" "$cargo_home" "$home_dir" "$rustup_home" "$safe_path" "$safe_git_config" "$runtime_tmp" "$rust_pin" "$trusted_seed_inventory_value" "$warm_tool_cache" "$validated_snapshot_lock_sha256" "$validated_snapshot_manifest_fingerprint" || exit $?

    validate_cargo_inputs selected-snapshot "$snapshot" "$cargo_home" "$validated_snapshot_lock_sha256" "$validated_snapshot_manifest_fingerprint" || exit $?

    generator_path="${tool_executable%/*}:${rust_cargo_executable%/*}:${jactionlint_executable%/*}:${shellcheck_executable%/*}:$safe_path"
    generator_environment=(
      "${mise_environment[@]}"
      "PATH=$generator_path"
      CARGO_NET_OFFLINE=true
      CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
      "RUSTUP_TOOLCHAIN=$rust_pin"
    )
    env -i "${generator_environment[@]}" "$tool_executable" generate --output-dir "$preview" </dev/null || exit $?
    validate_cargo_inputs selected-snapshot "$snapshot" "$cargo_home" "$validated_snapshot_lock_sha256" "$validated_snapshot_manifest_fingerprint" || exit $?
    validate_snapshot_symlinks "$snapshot" || exit $?
    validate_snapshot_symlinks "$preview" || exit $?

    local comparison_status
    if git_in_snapshot_context -C "$snapshot" diff --no-index --quiet --no-ext-diff --no-textconv -- "$snapshot/.github" "$preview/.github"; then
      comparison_status=0
    else
      comparison_status=$?
    fi

    if [[ "$comparison_status" -eq 1 ]]; then
      printf 'generated-workflows: drift in %s snapshot\n' "$label" >&2
      exit 1
    fi
    if [[ "$comparison_status" -ne 0 ]]; then
      exit "$comparison_status"
    fi

    if [[ "$warm_tool_cache" != true && "$installed_checker" -eq 1 && "${prepare_cache_mode:-0}" -eq 1 ]]; then
      publish_tool_cache "$tool_cache_generation" "$cache_stage_dir" "$tool_cache_identity" "$platform" "$tool_pin" "$rust_pin" "$jactionlint_pin" "$shellcheck_pin" || exit $?
    fi
    exit "$comparison_status"
  )
}

check_tree() {
  local tree=$1
  local label=$2
  local snapshot="$temporary_root/$label"
  local preview="$temporary_root/$label-preview"
  local entries="$temporary_root/$label-tree-entries"

  initialize_snapshot_reader || return $?
  mkdir -p "$snapshot" "$preview" || return $?
  git_in_snapshot_context init --object-format="$object_format" --quiet "$snapshot" || return $?
  mkdir -p "$snapshot/.git/objects/info" || return $?
  printf '%s\n%s\n' "$index_tree_objects" "$verified_blob_objects" > "$snapshot/.git/objects/info/alternates" || return $?
  if [[ "$installed_checker" -eq 1 ]]; then
    printf '%s\n' "$trusted_private_objects" >> "$snapshot/.git/objects/info/alternates" || return $?
  fi
  printf '%s\n' "$objects" >> "$snapshot/.git/objects/info/alternates" || return $?
  while IFS= read -r alternate || [[ -n "$alternate" ]]; do
    if [[ "$alternate" != /* ]]; then
      printf 'generated-workflows: source alternate object directory is not absolute\n' >&2
      return 1
    fi
    printf '%s\n' "$alternate" >> "$snapshot/.git/objects/info/alternates" || return $?
  done < "$source_alternate_dirs"
  bounded_tree_listing snapshot '' "$tree" "$entries" || return $?
  validate_snapshot_tree_entries "$entries" || return $?
  preflight_tree_blob_sizes snapshot '' "$entries" true || return $?
  hydrate_blobs snapshot '' "$entries" "$label" "selected snapshot" || return $?
  preflight_tree_blob_sizes snapshot '' "$entries" false || return $?
  git_in_snapshot_context -C "$snapshot" read-tree "$tree" || return $?
  materialize_tree "$snapshot" "$entries" || return $?
  validate_snapshot_symlinks "$snapshot" || return $?
  find "$snapshot" -type f -exec chmod a-w {} + || return $?
  find "$snapshot" -type d -exec chmod a-w {} + || return $?
  run_generator "$snapshot" "$preview" "$label"
}

resolve_push_tree() {
  local local_oid=$1
  local remote_ref=$2
  local peeled_oid
  local peeled_type
  local commit

  initialize_snapshot_reader || return $?
  case "$remote_ref" in
    refs/heads/*)
      commit=$(git_in_private_snapshot_reader rev-parse --verify "${local_oid}^{commit}") || {
        printf 'generated-workflows: cannot resolve pushed commit %s for %s\n' "$local_oid" "$remote_ref" >&2
        return 1
      }
      ;;
    refs/tags/*)
      peeled_oid=$(git_in_private_snapshot_reader rev-parse --verify "${local_oid}^{}") || {
        printf 'generated-workflows: cannot peel pushed tag object %s for %s\n' "$local_oid" "$remote_ref" >&2
        return 1
      }
      peeled_type=$(git_in_private_snapshot_reader cat-file -t "$peeled_oid") || {
        printf 'generated-workflows: cannot resolve peeled tag object %s for %s\n' "$peeled_oid" "$remote_ref" >&2
        return 1
      }
      if [[ "$peeled_type" != commit ]]; then
        return 2
      fi
      commit=$peeled_oid
      ;;
    *)
      return 2
      ;;
  esac

  local tree
  tree=$(git_in_private_snapshot_reader rev-parse --verify "${commit}^{tree}") || {
    printf 'generated-workflows: cannot resolve pushed tree for %s\n' "$remote_ref" >&2
    return 1
  }
  printf '%s\n' "$tree"
}

check_push_records() {
  local record=
  local local_ref
  local local_oid
  local remote_ref
  local remote_oid
  local extra
  local tree
  local checked=0
  local resolve_status

  while IFS= read -r record || [[ -n "$record" ]]; do
    IFS=$' \t' read -r local_ref local_oid remote_ref remote_oid extra <<< "$record"

    if [[ -z "${local_ref:-}" || -z "${local_oid:-}" || -z "${remote_ref:-}" || -z "${remote_oid:-}" || -n "${extra:-}" ]]; then
      printf 'generated-workflows: invalid pre-push record\n' >&2
      return 2
    fi

    if [[ ${#local_oid} -ne $object_length || ! "$local_oid" =~ ^[0-9a-f]+$ || ${#remote_oid} -ne $object_length || ! "$remote_oid" =~ ^[0-9a-f]+$ ]]; then
      printf 'generated-workflows: invalid object ID in pre-push record\n' >&2
      return 2
    fi

    if [[ "$local_oid" =~ ^0+$ ]]; then
      continue
    fi

    case "$remote_ref" in
      refs/heads/*|refs/tags/*) ;;
      *) continue ;;
    esac

    if tree=$(resolve_push_tree "$local_oid" "$remote_ref"); then
      :
    else
      resolve_status=$?
      if [[ "$resolve_status" -eq 2 ]]; then
        continue
      fi
      return "$resolve_status"
    fi

    checked=$((checked + 1))
    check_tree "$tree" "push-$checked" || return $?
  done
}

prepare_cache_mode=0
case "${1:-}" in
  pre-commit)
    if [[ $# -ne 1 ]]; then
      printf 'generated-workflows: expected pre-commit mode\n' >&2
      exit 2
    fi
    initialize_snapshot_reader || exit $?
    private_index="$temporary_root/effective-index"
    if [[ -L "$effective_index" || ! -f "$effective_index" ]]; then
      printf 'generated-workflows: effective index is missing or not a regular file\n' >&2
      exit 1
    fi
    copy_unchanged_regular_file "$effective_index" "$private_index" 'effective index' || exit $?
    shared_index=$(git_in_source_context rev-parse --shared-index-path) || {
      printf 'generated-workflows: cannot resolve the effective split-index base\n' >&2
      exit 1
    }
    if [[ -n "$shared_index" ]]; then
      shared_basename=${shared_index##*/}
      if [[ ! "$shared_basename" =~ ^sharedindex\.[0-9a-f]{$object_length}$ ]]; then
        printf 'generated-workflows: effective split-index base name is invalid\n' >&2
        exit 1
      fi
      if [[ "$shared_index" == /* ]]; then
        source_shared_index=$shared_index
      else
        source_shared_index="$repo_root/$shared_index"
      fi
      source_shared_parent=$(cd "$(dirname "$source_shared_index")" && pwd -P) || {
        printf 'generated-workflows: cannot resolve the source split-index directory\n' >&2
        exit 1
      }
      if [[ "$source_shared_parent" != "$source_git_directory" ]]; then
        printf 'generated-workflows: effective split-index base is outside the source Git directory\n' >&2
        exit 1
      fi
      if [[ -L "$source_shared_index" || ! -f "$source_shared_index" ]]; then
        printf 'generated-workflows: effective split-index base is unavailable\n' >&2
        exit 1
      fi
      copy_unchanged_regular_file "$source_shared_index" "$temporary_root/$shared_basename" 'effective split-index base' || exit $?
    fi
    tree=$(git_with_private_tree_objects "$private_index" write-tree --missing-ok) || {
      printf 'generated-workflows: cannot materialize the staged index\n' >&2
      exit 1
    }
    check_tree "$tree" index
    ;;
  pre-push)
    if [[ $# -ne 1 && $# -ne 3 ]]; then
      printf 'generated-workflows: expected pre-push mode and optional Git hook arguments\n' >&2
      exit 2
    fi
    check_push_records
    ;;
  prepare-cache)
    if [[ "$installed_checker" -ne 1 || $# -ne 1 ]]; then
      printf 'generated-workflows: prepare-cache is available only through an installed trusted checker\n' >&2
      exit 2
    fi
    prepare_cache_mode=1
    provision_trusted_metadata_cache || exit $?
    trusted_seed_reader="$temporary_root/prewarm-trusted-seed-reader.git"
    initialize_trusted_reader "$trusted_seed_reader" || exit $?
    resolved_seed_tree=$(git_in_trusted_reader "$trusted_seed_reader" rev-parse --verify "$trusted_seed_commit^{tree}") || {
      printf 'generated-workflows: approved seed tree is unavailable for trusted setup\n' >&2
      exit 1
    }
    if [[ "$resolved_seed_tree" != "$trusted_seed_tree" ]]; then
      printf 'generated-workflows: approved seed tree does not match the reviewed identity\n' >&2
      exit 1
    fi
    trusted_snapshot_auth=1
    check_tree "$resolved_seed_tree" trusted-seed
    trusted_snapshot_auth=0
    ;;
  *)
    printf 'generated-workflows: expected pre-commit, pre-push, or prepare-cache mode\n' >&2
    exit 2
    ;;
esac
