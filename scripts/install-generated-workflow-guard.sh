#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set +x +a
set -euo pipefail
LC_ALL=C
export LC_ALL
unset trusted_setup_token
trusted_setup_token=${GITHUB_COM_TOKEN-}
unset GITHUB_COM_TOKEN
if [[ -n "$trusted_setup_token" && ( ! "$trusted_setup_token" =~ ^[A-Za-z0-9_]+$ || ${#trusted_setup_token} -gt 512 ) ]]; then
  printf 'generated-workflows: invalid setup credential format\n' >&2
  exit 1
fi
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
git_command=/usr/bin/git
if [[ ! -x "$git_command" ]]; then
  printf 'generated-workflows: /usr/bin/git is unavailable\n' >&2
  exit 127
fi
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64) hash_tool=/usr/bin/shasum ;;
  Linux:x86_64|Linux:amd64) hash_tool=/usr/bin/sha256sum ;;
  *) printf 'generated-workflows: unsupported host for hook installation\n' >&2; exit 2 ;;
esac
reviewed_checker_sha256=17ec8f27937e9bb3cc75aefad75e37bc859eaf012b39ac25da997f2b81d821b3
reviewed_config_sha256=bbc7db0a6b0b68c3a6090dab4284f730ea965d00be01e482720ef1afff1b7458
reviewed_python_mise_sha256=4bf669510cff7c1f5d5c7c8edc5f6d496bedf872b635295a607f81680074744d
reviewed_inventory_helper_sha256=0541173812097cd6ac4deb065a7e45b6960ddcf26369eb3fd32805a0b66f33eb
reviewed_seed_commit=f4d9999ace7d6deb4294cd89c00afcc80e748d49
reviewed_seed_tree=95406daef6ba234fde62fc47b933fbb8bdfce320
reviewed_seed_cargo_lock_sha256=d1abe8b74eac36822b25a735e8cc28e2c8434103a8c87f662a9ebe2da5f593a7
reviewed_seed_cargo_policy_sha256=6d6c35161e163ff07f39d413e3d25c6d60016ebfcb6627e278e062a86df30eae
reviewed_metadata_pack_sha256=845b98b650d85074bdad58d771e785f01d0824e4647efe0113fcdb7821704874
reviewed_metadata_object_list_sha256=58233f6e2765561ef177650f49b76fb45e0f0980a97d98df5f39759a36525268
install_version=2
mise_version=2026.11.4
python_version=3.15.2
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64)
    platform=macos-arm64
    mise_sha256=21c269cd3c88e31c56aad7f7ccd9916b6fa78683937d93e5d17129b783d6f962
    python_artifact_sha256=1b1b03fca8aa351698403f3d100ee83db7a5d99748fc7707681ac31a6a846ad9
    python_lock_sha256=6a849972c8d6ce7e8a5b3970b73dab00918aeb60155ed0c3c91e711991a9f8a1
    python_lock_file=python-mise.macos-arm64.lock.toml
    ;;
  Linux:x86_64|Linux:amd64)
    platform=linux-x64
    mise_sha256=0296d9cba9324106d4e0962bb94a2072902f3658813aeb3b09504947bb8ddc2b
    python_artifact_sha256=7b03163a980c607ba8194709d4a4086735a7a9c488cc475b098157306927b36b
    python_lock_sha256=5c13cb611f7f461656eeb052684c8d82f17eb5979d65f89b48622a0f60c4a168
    python_lock_file=python-mise.linux-x64.lock.toml
    ;;
esac

sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | awk '{print $1}'
  else
    "$hash_tool" "$1" | awk '{print $1}'
  fi
}

owned_private_directory() {
  local path=$1
  local owner
  local mode
  if [[ -L "$path" || ! -d "$path" ]]; then
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$path")
    mode=$(/usr/bin/stat -f '%Lp' "$path")
  else
    owner=$(/usr/bin/stat -c '%u' "$path")
    mode=$(/usr/bin/stat -c '%a' "$path")
  fi
  [[ "$owner" == "$(/usr/bin/id -u)" ]] && (( (8#$mode & 077) == 0 ))
}

owned_safe_directory() {
  local path=$1
  local owner
  local mode
  if [[ -L "$path" || ! -d "$path" ]]; then
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$path")
    mode=$(/usr/bin/stat -f '%Lp' "$path")
  else
    owner=$(/usr/bin/stat -c '%u' "$path")
    mode=$(/usr/bin/stat -c '%a' "$path")
  fi
  [[ "$owner" == "$(/usr/bin/id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] || return 1
  (( (8#$mode & 0222) == 0 && (8#$mode & 07000) == 0 ))
}

owned_private_file() {
  local path=$1
  local owner
  local mode
  local links
  if [[ -L "$path" || ! -f "$path" ]]; then
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$path")
    mode=$(/usr/bin/stat -f '%Lp' "$path")
    links=$(/usr/bin/stat -f '%l' "$path")
  else
    owner=$(/usr/bin/stat -c '%u' "$path")
    mode=$(/usr/bin/stat -c '%a' "$path")
    links=$(/usr/bin/stat -c '%h' "$path")
  fi
  [[ "$owner" == "$(/usr/bin/id -u)" && "$links" == 1 ]] && (( (8#$mode & 077) == 0 ))
}

file_size() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%z' "$1"
  else
    /usr/bin/stat -c '%s' "$1"
  fi
}

file_identity() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%d:%i:%u:%Lp:%l' "$1"
  else
    /usr/bin/stat -c '%d:%i:%u:%a:%h' "$1"
  fi
}

file_mode() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%Lp' "$1"
  else
    /usr/bin/stat -c '%a' "$1"
  fi
}

limited_private_file() {
  local path=$1
  local limit=$2
  local size
  owned_private_file "$path" || return 1
  size=$(file_size "$path") || return 1
  [[ "$size" =~ ^[0-9]+$ ]] && (( size <= limit ))
}

limited_owned_executable() {
  local path=$1
  local limit=$2
  local owner
  local mode
  local links
  local size
  if [[ -L "$path" || ! -f "$path" ]]; then
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$path")
    mode=$(/usr/bin/stat -f '%Lp' "$path")
    links=$(/usr/bin/stat -f '%l' "$path")
  else
    owner=$(/usr/bin/stat -c '%u' "$path")
    mode=$(/usr/bin/stat -c '%a' "$path")
    links=$(/usr/bin/stat -c '%h' "$path")
  fi
  size=$(file_size "$path") || return 1
  [[ "$owner" == "$(/usr/bin/id -u)" && "$links" == 1 && "$mode" =~ ^[0-7]{3,4}$ && "$size" =~ ^[0-9]+$ ]] || return 1
  (( (8#$mode & 0222) == 0 && (8#$mode & 0500) == 0500 && (8#$mode & 07000) == 0 && size <= limit ))
}

bounded_runtime_bytes=0
bound_runtime_tree() {
  local root=$1
  local result
  local number_pattern='^(0|[1-9][0-9]*)$'
  local row_pattern='^([0-9]+):([0-9]+):([0-9a-fA-F]+)$'
  result=$(
    local path='' target size current_uid count=0 path_bytes=0 target_bytes=0 file_bytes=0
    local -a runtime_paths=()
    current_uid=$(/usr/bin/id -u) || exit 1
    check_runtime_batch() {
      local output row owner mode kind index=0 finished=0
      (( ${#runtime_paths[@]} == 0 )) && return 0
      if [[ "$platform" == macos-arm64 ]]; then
        output=$(/usr/bin/stat -f '%u:%z:%p' "${runtime_paths[@]}" && printf 'END\n') || return 1
      else
        output=$(/usr/bin/stat -c '%u:%s:%f' "${runtime_paths[@]}" && printf 'END\n') || return 1
      fi
      while IFS= read -r row; do
        if [[ "$row" == END ]]; then
          (( finished == 0 && index == ${#runtime_paths[@]} )) || return 1
          finished=1
          continue
        fi
        (( finished == 0 && index < ${#runtime_paths[@]} )) || return 1
        [[ "$row" =~ $row_pattern ]] || return 1
        owner=${BASH_REMATCH[1]}
        size=${BASH_REMATCH[2]}
        mode=${BASH_REMATCH[3]}
        [[ "$owner" == "$current_uid" ]] || return 1
        if [[ "$platform" == macos-arm64 ]]; then
          [[ "$mode" =~ ^[0-7]{1,7}$ ]] || return 1
          kind=$((8#$mode & 0170000))
        else
          [[ "$mode" =~ ^[0-9a-fA-F]{1,8}$ ]] || return 1
          kind=$((16#$mode & 0170000))
        fi
        path=${runtime_paths[$index]}
        case "$kind" in
          (40960)
            [[ -L "$path" ]] || return 1
            target=$(/usr/bin/readlink "$path" && printf '.') || return 1
            target=${target%.}
            target=${target%$'\n'}
            [[ -n "$target" ]] || return 1
            target_bytes=$((target_bytes + ${#target}))
            (( target_bytes <= 16777216 )) || return 1
            ;;
          (16384) [[ ! -L "$path" && -d "$path" ]] || return 1 ;;
          (32768)
            [[ ! -L "$path" && -f "$path" && "$size" =~ $number_pattern && ${#size} -le 9 ]] || return 1
            (( 10#$size <= 268435456 - file_bytes )) || return 1
            file_bytes=$((file_bytes + 10#$size))
            ;;
          (*) return 1 ;;
        esac
        index=$((index + 1))
      done <<< "$output"
      (( finished == 1 )) || return 1
      runtime_paths=()
    }
    /usr/bin/find "$root" -mindepth 1 -print0 | {
      while IFS= read -r -d '' path; do
        count=$((count + 1))
        (( count <= 65536 )) || exit 1
        path_bytes=$((path_bytes + ${#path}))
        (( path_bytes <= 16777216 )) || exit 1
        runtime_paths+=("$path")
        if (( ${#runtime_paths[@]} == 64 )); then
          check_runtime_batch || exit 1
        fi
      done
      [[ -z "$path" ]] || exit 1
      check_runtime_batch || exit 1
      printf '%s\n' "$file_bytes"
    }
  ) || return 1
  [[ "$result" =~ $number_pattern && ${#result} -le 9 ]] || return 1
  bounded_runtime_bytes=$result
}

generation_inspected_total=0
validate_installed_generation() {
  local generation=$1
  local scope=${2:-all}
  local directory="$versions_dir/$generation"
  local manifest="$directory/manifest"
  local line
  local key
  local value
  local line_count=0
  local seen_version=0
  local seen_checker_version=0
  local seen_checker_sha256=0
  local seen_config_sha256=0
  local seen_inventory_helper_sha256=0
  local seen_seed_commit=0
  local seen_seed_tree=0
  local seen_metadata_pack_sha256=0
  local seen_metadata_object_list_sha256=0
  local seen_mise_version=0
  local seen_mise_sha256=0
  local seen_python_version=0
  local seen_python_artifact_sha256=0
  local seen_python_lock_sha256=0
  local seen_python_mise_sha256=0
  local seen_inventory_sha256=0
  local seen_trusted_seed_cargo_lock_sha256=0
  local seen_trusted_seed_cargo_policy_sha256=0
  local manifest_version=
  local manifest_checker_version=
  local manifest_checker_sha256=
  local manifest_config_sha256=
  local manifest_inventory_helper_sha256=
  local manifest_seed_commit=
  local manifest_seed_tree=
  local manifest_metadata_pack_sha256=
  local manifest_metadata_object_list_sha256=
  local manifest_mise_version=
  local manifest_mise_sha256=
  local manifest_python_version=
  local manifest_python_artifact_sha256=
  local manifest_python_lock_sha256=
  local manifest_python_mise_sha256=
  local manifest_inventory_sha256=
  local manifest_trusted_seed_cargo_lock_sha256=
  local manifest_trusted_seed_cargo_policy_sha256=
  local total_bytes=0
  local actual
  local commit_tree
  local python_generation
  local python_directory
  local python_runtime
  local python_inventory
  local python_artifact_manifest
  local python_mise_config
  local python_mise_lock
  local mise_binary
  local expected_python_manifest
  local inventory_python_hash

  [[ "$generation" =~ ^[0-9a-f]{64}$ ]] || return 1
  owned_private_directory "$install_root" || return 1
  owned_private_directory "$versions_dir" || return 1
  owned_private_directory "$directory" || return 1
  limited_private_file "$manifest" 8192 || return 1
  [[ -z "$(/usr/bin/tail -c 1 "$manifest")" ]] || return 1
  total_bytes=$(file_size "$manifest") || return 1

  while IFS= read -r line || [[ -n "$line" ]]; do
    line_count=$((line_count + 1))
    [[ "$line" == *=* && "$line" != *=*=* ]] || return 1
    key=${line%%=*}
    value=${line#*=}
    [[ -n "$value" && "$line_count" -le 18 ]] || return 1
    case "$key" in
      version)
        [[ "$seen_version" -eq 0 ]] || return 1
        seen_version=1
        manifest_version=$value
        ;;
      checker_version)
        [[ "$seen_checker_version" -eq 0 ]] || return 1
        seen_checker_version=1
        manifest_checker_version=$value
        ;;
      checker_sha256)
        [[ "$seen_checker_sha256" -eq 0 ]] || return 1
        seen_checker_sha256=1
        manifest_checker_sha256=$value
        ;;
      config_sha256)
        [[ "$seen_config_sha256" -eq 0 ]] || return 1
        seen_config_sha256=1
        manifest_config_sha256=$value
        ;;
      inventory_helper_sha256)
        [[ "$seen_inventory_helper_sha256" -eq 0 ]] || return 1
        seen_inventory_helper_sha256=1
        manifest_inventory_helper_sha256=$value
        ;;
      seed_commit)
        [[ "$seen_seed_commit" -eq 0 ]] || return 1
        seen_seed_commit=1
        manifest_seed_commit=$value
        ;;
      seed_tree)
        [[ "$seen_seed_tree" -eq 0 ]] || return 1
        seen_seed_tree=1
        manifest_seed_tree=$value
        ;;
      metadata_pack_sha256)
        [[ "$seen_metadata_pack_sha256" -eq 0 ]] || return 1
        seen_metadata_pack_sha256=1
        manifest_metadata_pack_sha256=$value
        ;;
      metadata_object_list_sha256)
        [[ "$seen_metadata_object_list_sha256" -eq 0 ]] || return 1
        seen_metadata_object_list_sha256=1
        manifest_metadata_object_list_sha256=$value
        ;;
      mise_version)
        [[ "$seen_mise_version" -eq 0 ]] || return 1
        seen_mise_version=1
        manifest_mise_version=$value
        ;;
      mise_sha256)
        [[ "$seen_mise_sha256" -eq 0 ]] || return 1
        seen_mise_sha256=1
        manifest_mise_sha256=$value
        ;;
      python_version)
        [[ "$seen_python_version" -eq 0 ]] || return 1
        seen_python_version=1
        manifest_python_version=$value
        ;;
      python_artifact_sha256)
        [[ "$seen_python_artifact_sha256" -eq 0 ]] || return 1
        seen_python_artifact_sha256=1
        manifest_python_artifact_sha256=$value
        ;;
      python_lock_sha256)
        [[ "$seen_python_lock_sha256" -eq 0 ]] || return 1
        seen_python_lock_sha256=1
        manifest_python_lock_sha256=$value
        ;;
      python_mise_sha256)
        [[ "$seen_python_mise_sha256" -eq 0 ]] || return 1
        seen_python_mise_sha256=1
        manifest_python_mise_sha256=$value
        ;;
      inventory_sha256)
        [[ "$seen_inventory_sha256" -eq 0 ]] || return 1
        seen_inventory_sha256=1
        manifest_inventory_sha256=$value
        ;;
      trusted_seed_cargo_lock_sha256)
        [[ "$seen_trusted_seed_cargo_lock_sha256" -eq 0 ]] || return 1
        seen_trusted_seed_cargo_lock_sha256=1
        manifest_trusted_seed_cargo_lock_sha256=$value
        ;;
      trusted_seed_cargo_policy_sha256)
        [[ "$seen_trusted_seed_cargo_policy_sha256" -eq 0 ]] || return 1
        seen_trusted_seed_cargo_policy_sha256=1
        manifest_trusted_seed_cargo_policy_sha256=$value
        ;;
      *)
        return 1
        ;;
    esac
  done < "$manifest"

  [[ ( "$line_count" -eq 16 && "$seen_trusted_seed_cargo_lock_sha256" -eq 0 && "$seen_trusted_seed_cargo_policy_sha256" -eq 0 || "$line_count" -eq 17 && "$seen_trusted_seed_cargo_lock_sha256" -eq 1 && "$seen_trusted_seed_cargo_policy_sha256" -eq 0 || "$line_count" -eq 18 && "$seen_trusted_seed_cargo_lock_sha256" -eq 1 && "$seen_trusted_seed_cargo_policy_sha256" -eq 1 ) && "$seen_version" -eq 1 && "$seen_checker_version" -eq 1 && "$seen_checker_sha256" -eq 1 && "$seen_config_sha256" -eq 1 && "$seen_inventory_helper_sha256" -eq 1 && "$seen_seed_commit" -eq 1 && "$seen_seed_tree" -eq 1 && "$seen_metadata_pack_sha256" -eq 1 && "$seen_metadata_object_list_sha256" -eq 1 && "$seen_mise_version" -eq 1 && "$seen_mise_sha256" -eq 1 && "$seen_python_version" -eq 1 && "$seen_python_artifact_sha256" -eq 1 && "$seen_python_lock_sha256" -eq 1 && "$seen_python_mise_sha256" -eq 1 && "$seen_inventory_sha256" -eq 1 ]] || return 1
  [[ ( "$manifest_version" == 1 && "$manifest_checker_version" == 1 && "$line_count" -le 17 || "$manifest_version" == 2 && "$manifest_checker_version" == 2 && "$line_count" -eq 18 ) && "$manifest_checker_sha256" == "$generation" ]] || return 1
  [[ "$manifest_checker_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_config_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_inventory_helper_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_metadata_pack_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_metadata_object_list_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_mise_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_python_artifact_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_python_lock_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_python_mise_sha256" =~ ^[0-9a-f]{64}$ && "$manifest_inventory_sha256" =~ ^[0-9a-f]{64}$ ]] || return 1
  if [[ "$seen_trusted_seed_cargo_lock_sha256" -eq 1 && ! "$manifest_trusted_seed_cargo_lock_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    return 1
  fi
  if [[ "$seen_trusted_seed_cargo_policy_sha256" -eq 1 && ! "$manifest_trusted_seed_cargo_policy_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    return 1
  fi
  [[ "$manifest_seed_commit" =~ ^[0-9a-f]{40}$ && "$manifest_seed_tree" =~ ^[0-9a-f]{40}$ && "$manifest_mise_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$manifest_python_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || return 1

  for line in check-generated-workflows.sh config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects; do
    local path="$directory/$line"
    local limit=1048576
    case "$line" in
      check-generated-workflows.sh) limit=4194304 ;;
      trusted-seed-metadata.pack) limit=33554432 ;;
      trusted-seed-metadata.objects) limit=1048576 ;;
    esac
    limited_private_file "$path" "$limit" || return 1
    total_bytes=$((total_bytes + $(file_size "$path")))
    (( total_bytes <= 268435456 )) || return 1
  done

  if [[ "$seen_trusted_seed_cargo_lock_sha256" -eq 1 ]]; then
    limited_private_file "$directory/trusted-seed-cargo.lock" 16777216 || return 1
    total_bytes=$((total_bytes + $(file_size "$directory/trusted-seed-cargo.lock")))
    (( total_bytes <= 268435456 )) || return 1
    actual=$(sha256_file "$directory/trusted-seed-cargo.lock") || return 1
    [[ "$actual" == "$manifest_trusted_seed_cargo_lock_sha256" ]] || return 1
  elif [[ -e "$directory/trusted-seed-cargo.lock" || -L "$directory/trusted-seed-cargo.lock" ]]; then
    return 1
  fi

  if [[ "$seen_trusted_seed_cargo_policy_sha256" -eq 1 ]]; then
    limited_private_file "$directory/trusted-seed-cargo-policy.json" 262144 || return 1
    total_bytes=$((total_bytes + $(file_size "$directory/trusted-seed-cargo-policy.json")))
    (( total_bytes <= 268435456 )) || return 1
    actual=$(sha256_file "$directory/trusted-seed-cargo-policy.json") || return 1
    [[ "$actual" == "$manifest_trusted_seed_cargo_policy_sha256" ]] || return 1
  elif [[ -e "$directory/trusted-seed-cargo-policy.json" || -L "$directory/trusted-seed-cargo-policy.json" ]]; then
    return 1
  fi

  actual=$(sha256_file "$directory/check-generated-workflows.sh") || return 1
  [[ "$actual" == "$manifest_checker_sha256" ]] || return 1
  actual=$(sha256_file "$directory/config.toml") || return 1
  [[ "$actual" == "$manifest_config_sha256" ]] || return 1
  actual=$(sha256_file "$directory/python-inventory.sh") || return 1
  [[ "$actual" == "$manifest_inventory_helper_sha256" ]] || return 1
  actual=$(sha256_file "$directory/trusted-seed-metadata.pack") || return 1
  [[ "$actual" == "$manifest_metadata_pack_sha256" ]] || return 1
  actual=$(sha256_file "$directory/trusted-seed-metadata.objects") || return 1
  [[ "$actual" == "$manifest_metadata_object_list_sha256" ]] || return 1
  actual=$(git_clean hash-object -t commit --stdin < "$directory/trusted-seed.commit") || return 1
  [[ "$actual" == "$manifest_seed_commit" ]] || return 1
  IFS=' ' read -r key commit_tree < "$directory/trusted-seed.commit"
  [[ "$key" == tree && "$commit_tree" =~ ^[0-9a-f]{40}$ && "$commit_tree" == "$manifest_seed_tree" ]] || return 1

  if [[ "$scope" == generation ]]; then
    generation_inspected_total=$((generation_inspected_total + total_bytes))
    (( generation_inspected_total <= 536870912 ))
    return $?
  fi

  mise_binary="$artifact_root/mise/$manifest_mise_version/$platform/mise"
  owned_private_directory "$artifact_root" || return 1
  owned_private_directory "$artifact_root/mise" || return 1
  owned_private_directory "$artifact_root/mise/$manifest_mise_version" || return 1
  owned_private_directory "$artifact_root/mise/$manifest_mise_version/$platform" || return 1
  limited_private_file "$mise_binary" 268435456 || return 1
  total_bytes=$((total_bytes + $(file_size "$mise_binary")))
  (( total_bytes <= 268435456 )) || return 1
  actual=$(sha256_file "$mise_binary") || return 1
  [[ "$actual" == "$manifest_mise_sha256" ]] || return 1

  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    python_generation=$(printf '%s\n' "$platform" "$manifest_python_version" "$manifest_python_artifact_sha256" "$manifest_python_lock_sha256" "$manifest_python_mise_sha256" "$manifest_inventory_helper_sha256" "$manifest_mise_version" "$manifest_mise_sha256" | "$hash_tool" -a 256 | /usr/bin/awk '{print $1}') || return 1
  else
    python_generation=$(printf '%s\n' "$platform" "$manifest_python_version" "$manifest_python_artifact_sha256" "$manifest_python_lock_sha256" "$manifest_python_mise_sha256" "$manifest_inventory_helper_sha256" "$manifest_mise_version" "$manifest_mise_sha256" | "$hash_tool" | /usr/bin/awk '{print $1}') || return 1
  fi
  [[ "$python_generation" =~ ^[0-9a-f]{64}$ ]] || return 1
  python_directory="$artifact_root/python/$manifest_python_version/$platform-$python_generation"
  python_runtime="$python_directory/runtime"
  python_inventory="$python_directory/inventory"
  python_artifact_manifest="$python_directory/manifest"
  python_mise_config="$python_directory/mise.toml"
  python_mise_lock="$python_directory/mise.lock"
  owned_private_directory "$artifact_root/python" || return 1
  owned_private_directory "$artifact_root/python/$manifest_python_version" || return 1
  owned_private_directory "$python_directory" || return 1
  owned_private_directory "$python_runtime" || return 1
  limited_private_file "$python_inventory" 16777216 || return 1
  limited_private_file "$python_artifact_manifest" 8192 || return 1
  limited_private_file "$python_mise_config" 16777216 || return 1
  limited_private_file "$python_mise_lock" 16777216 || return 1
  total_bytes=$((total_bytes + $(file_size "$python_inventory") + $(file_size "$python_artifact_manifest") + $(file_size "$python_mise_config") + $(file_size "$python_mise_lock")))
  (( total_bytes <= 268435456 )) || return 1
  actual=$(sha256_file "$python_inventory") || return 1
  [[ "$actual" == "$manifest_inventory_sha256" ]] || return 1
  actual=$(sha256_file "$python_mise_config") || return 1
  [[ "$actual" == "$manifest_python_mise_sha256" ]] || return 1
  actual=$(sha256_file "$python_mise_lock") || return 1
  [[ "$actual" == "$manifest_python_lock_sha256" ]] || return 1
  expected_python_manifest=$(printf 'python_version=%s\nbackend=core:python\narchive_sha256=%s\nlock_sha256=%s\nmise_config_sha256=%s\ninventory_helper_sha256=%s\ninventory_sha256=%s' "$manifest_python_version" "$manifest_python_artifact_sha256" "$manifest_python_lock_sha256" "$manifest_python_mise_sha256" "$manifest_inventory_helper_sha256" "$manifest_inventory_sha256")
  if ! printf '%s\n' "$expected_python_manifest" | /usr/bin/cmp -s - "$python_artifact_manifest"; then
    return 1
  fi
  owned_safe_directory "$python_runtime/bin" || return 1
  limited_owned_executable "$python_runtime/bin/python3.15" 268435456 || return 1
  [[ -x "$python_runtime/bin/python3.15" ]] || return 1
  actual=$(sha256_file "$python_runtime/bin/python3.15") || return 1
  [[ "$actual" =~ ^[0-9a-f]{64}$ ]] || return 1
  inventory_python_hash=$(/usr/bin/awk -F '\t' '$1 == "file" && $4 == "bin/python3.15" { count++; hash = $3 } END { if (count != 1) exit 1; print hash }' "$python_inventory") || return 1
  [[ "$inventory_python_hash" == "$actual" ]] || return 1
  bound_runtime_tree "$python_runtime" || return 1
  (( total_bytes <= 268435456 - bounded_runtime_bytes )) || return 1
  total_bytes=$((total_bytes + bounded_runtime_bytes))
  env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LC_ALL=C LANG=C /bin/bash "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory" || return 1
  generation_inspected_total=$((generation_inspected_total + total_bytes))
  (( generation_inspected_total <= 536870912 ))
}

git_clean() {
  env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="${HOME:-/tmp}" GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" -C "$repo_root" "$@"
}

worktree_root=$(git_clean rev-parse --show-toplevel) || {
  printf 'generated-workflows: run setup from a Git worktree\n' >&2
  exit 1
}
worktree_root=$(cd "$worktree_root" && pwd -P)
common_dir=$(git_clean rev-parse --path-format=absolute --git-common-dir) || exit 1
common_dir=$(cd "$common_dir" && pwd -P)
install_root="$common_dir/jackin-generated-workflow-guard"
versions_dir="$install_root/versions"
version_dir="$versions_dir/$reviewed_checker_sha256"
cache_root="$install_root/cache"
artifact_root="$install_root/artifacts"
lock_dir="$install_root/install.lock"
hooks_path=$(git_clean rev-parse --path-format=absolute --git-path hooks) || exit 1
if [[ "$hooks_path" != /* ]]; then
  hooks_path="$worktree_root/$hooks_path"
fi
hooks_path_parent=$(dirname "$hooks_path")
hooks_path_leaf=$(basename "$hooks_path")
if [[ -d "$hooks_path" ]]; then
  hooks_path_logical=$(cd "$hooks_path" && pwd -L) || exit 1
  hooks_path_physical=$(cd "$hooks_path" && pwd -P) || exit 1
elif [[ -d "$hooks_path_parent" ]]; then
  hooks_path_logical=$(cd "$hooks_path_parent" && pwd -L) || exit 1
  hooks_path_physical=$(cd "$hooks_path_parent" && pwd -P) || exit 1
  hooks_path_logical="$hooks_path_logical/$hooks_path_leaf"
  hooks_path_physical="$hooks_path_physical/$hooks_path_leaf"
else
  printf 'generated-workflows: effective hook directory parent is unavailable\n' >&2
  exit 1
fi
if [[ "$hooks_path_logical" != "$hooks_path_physical" ]]; then
  printf 'generated-workflows: effective hook directory traverses a symlink\n' >&2
  exit 1
fi
hooks_path=$hooks_path_physical
case "$hooks_path/" in
  "$worktree_root/"*)
    case "$hooks_path/" in
      "$common_dir/"*) ;;
      *)
        printf 'generated-workflows: configured core.hooksPath is inside the checkout; use a Git metadata hook directory\n' >&2
        exit 1
        ;;
    esac
    ;;
esac
render_hook() {
  local mode=$1
  local generation_dir=${2:-$version_dir}
  {
    printf '#!/bin/bash\nset +x +a\nunset GITHUB_COM_TOKEN\nset -euo pipefail\nLC_ALL=C\nexport LC_ALL\nguard_owner=jackin-generated-workflow-guard\nPATH=/usr/bin:/bin:/usr/sbin:/sbin\nexport PATH\nentry_index_present=0\nentry_index=\nif [[ ${GIT_INDEX_FILE+x} ]]; then\n  entry_index_present=1\n  entry_index=$GIT_INDEX_FILE\nfi\n' || return $?
    printf 'checker=%q\nmode=%q\ntrusted_install_root=%q\ntrusted_version_dir=%q\nhash_tool=%q\n' "$generation_dir/check-generated-workflows.sh" "$mode" "$install_root" "$generation_dir" "$hash_tool" || return $?
    cat <<'HOOK' || return $?
sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | /usr/bin/awk '{print $1}'
  else
    "$hash_tool" "$1" | /usr/bin/awk '{print $1}'
  fi
}
if [[ -L "$trusted_install_root" || ! -d "$trusted_install_root" || -L "$trusted_install_root/versions" || ! -d "$trusted_install_root/versions" || -L "$trusted_version_dir" || ! -d "$trusted_version_dir" || "$checker" != "$trusted_version_dir/check-generated-workflows.sh" || -L "$checker" || ! -f "$checker" ]]; then
  printf 'generated-workflows: trusted checker installation is unsafe\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$checker")" != "${trusted_version_dir##*/}" ]]; then
  printf 'generated-workflows: trusted checker failed its launcher integrity check\n' >&2
  exit 1
fi
current_user=$(/usr/bin/id -u)
for trusted_directory in "$trusted_install_root" "$trusted_install_root/versions" "$trusted_version_dir"; do
  if [[ "$(uname -s)" == Darwin ]]; then
    trusted_owner=$(/usr/bin/stat -f '%u' "$trusted_directory")
    trusted_mode=$(/usr/bin/stat -f '%Lp' "$trusted_directory")
  else
    trusted_owner=$(/usr/bin/stat -c '%u' "$trusted_directory")
    trusted_mode=$(/usr/bin/stat -c '%a' "$trusted_directory")
  fi
  if [[ "$trusted_owner" != "$current_user" ]] || (( (8#$trusted_mode & 077) != 0 )); then
    printf 'generated-workflows: trusted checker directory owner or permissions are unsafe\n' >&2
    exit 1
  fi
done
if [[ "$(uname -s)" == Darwin ]]; then
  checker_owner=$(/usr/bin/stat -f '%u' "$checker")
  checker_mode=$(/usr/bin/stat -f '%Lp' "$checker")
else
  checker_owner=$(/usr/bin/stat -c '%u' "$checker")
  checker_mode=$(/usr/bin/stat -c '%a' "$checker")
fi
if [[ "$checker_owner" != "$current_user" ]] || (( (8#$checker_mode & 022) != 0 )); then
  printf 'generated-workflows: trusted checker owner or permissions are unsafe\n' >&2
  exit 1
fi
if [[ "$entry_index_present" -eq 1 ]]; then
  exec /usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LANG=C LC_ALL=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "GIT_INDEX_FILE=$entry_index" /bin/bash "$checker" "$mode" "$@"
else
  exec /usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LANG=C LC_ALL=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 /bin/bash "$checker" "$mode" "$@"
fi
HOOK
  }
}

installed_generations=()
enumerate_installed_generations() {
  local path
  local leaf
  local count=0
  installed_generations=()
  if [[ ! -e "$versions_dir" && ! -L "$versions_dir" ]]; then
    return 0
  fi
  owned_private_directory "$install_root" && owned_private_directory "$versions_dir" || return 1
  for path in "$versions_dir"/* "$versions_dir"/.[!.]* "$versions_dir"/..?*; do
    [[ -e "$path" || -L "$path" ]] || continue
    count=$((count + 1))
    (( count <= 64 )) || return 1
    leaf=${path##*/}
    if [[ "$leaf" == .stage.* ]]; then
      owned_private_directory "$path" || return 1
      continue
    fi
    [[ "$leaf" =~ ^[0-9a-f]{64}$ && ! -L "$path" && -d "$path" ]] || return 1
    installed_generations+=("$leaf")
  done
}

validate_existing_hook() {
  local target=$1
  local expected_mode=$2
  local mode
  local owner
  local matched_generation=
  local generation
  local hook_size
  validated_generation=
  if [[ ! -e "$target" && ! -L "$target" ]]; then
    return 0
  fi
  if [[ -L "$target" || ! -f "$target" ]]; then
    printf 'generated-workflows: refusing to replace an existing hook at %s\n' "$target" >&2
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$target")
    mode=$(/usr/bin/stat -f '%Lp' "$target")
  else
    owner=$(/usr/bin/stat -c '%u' "$target")
    mode=$(/usr/bin/stat -c '%a' "$target")
  fi
  if [[ "$owner" != "$(/usr/bin/id -u)" ]] || (( (8#$mode & 022) != 0 )); then
    printf 'generated-workflows: refusing to replace an unsafe existing hook at %s\n' "$target" >&2
    return 1
  fi
  hook_size=$(file_size "$target") || return 1
  if [[ ! "$hook_size" =~ ^[0-9]+$ ]] || (( hook_size > 1048576 )); then
    printf 'generated-workflows: refusing to replace an oversized existing hook at %s\n' "$target" >&2
    return 1
  fi
  if ! enumerate_installed_generations; then
    printf 'generated-workflows: refusing to replace an existing hook at %s\n' "$target" >&2
    return 1
  fi
  for generation in "${installed_generations[@]}"; do
    if render_hook "$expected_mode" "$versions_dir/$generation" | /usr/bin/cmp -s - "$target"; then
      if [[ -n "$matched_generation" ]] || ! validate_installed_generation "$generation" generation; then
        printf 'generated-workflows: refusing to replace an existing hook at %s\n' "$target" >&2
        return 1
      fi
      matched_generation=$generation
    fi
  done
  if [[ -z "$matched_generation" ]]; then
    printf 'generated-workflows: refusing to replace an existing hook at %s\n' "$target" >&2
    return 1
  fi
  validated_generation=$matched_generation
}

validate_hook_pair() {
  local pre_commit_generation
  local pre_push_generation
  generation_inspected_total=0
  validate_existing_hook "$hooks_path/pre-commit" pre-commit || return 1
  pre_commit_generation=$validated_generation
  validate_existing_hook "$hooks_path/pre-push" pre-push || return 1
  pre_push_generation=$validated_generation
  if [[ -n "$pre_commit_generation" && -n "$pre_push_generation" && "$pre_commit_generation" != "$reviewed_checker_sha256" && "$pre_push_generation" != "$reviewed_checker_sha256" && "$pre_commit_generation" != "$pre_push_generation" ]]; then
    printf 'generated-workflows: refusing to replace hooks from divergent prior generations\n' >&2
    return 1
  fi
  validated_pre_commit_generation=$pre_commit_generation
  validated_pre_push_generation=$pre_push_generation
}

hook_state_captured=0
pre_commit_published=0
pre_push_published=0
capture_hook_baseline() {
  local path
  path="$hooks_path/pre-commit"
  if [[ -e "$path" || -L "$path" ]]; then
    pre_commit_initial_identity=$(file_identity "$path") || return 1
    pre_commit_initial_hash=$(sha256_file "$path") || return 1
    pre_commit_initial_mode=$(file_mode "$path") || return 1
  else
    pre_commit_initial_identity=missing
    pre_commit_initial_hash=missing
    pre_commit_initial_mode=missing
  fi
  path="$hooks_path/pre-push"
  if [[ -e "$path" || -L "$path" ]]; then
    pre_push_initial_identity=$(file_identity "$path") || return 1
    pre_push_initial_hash=$(sha256_file "$path") || return 1
    pre_push_initial_mode=$(file_mode "$path") || return 1
  else
    pre_push_initial_identity=missing
    pre_push_initial_hash=missing
    pre_push_initial_mode=missing
  fi
  hook_state_captured=1
}

assert_target_state() {
  local target=$1
  local expected_identity=$2
  local expected_hash=$3
  local actual_identity
  local actual_hash
  if [[ "$expected_identity" == missing ]]; then
    [[ ! -e "$target" && ! -L "$target" ]]
    return $?
  fi
  [[ -f "$target" && ! -L "$target" ]] || return 1
  actual_identity=$(file_identity "$target") || return 1
  actual_hash=$(sha256_file "$target") || return 1
  [[ "$actual_identity" == "$expected_identity" && "$actual_hash" == "$expected_hash" ]]
}

assert_expected_hook_states() {
  [[ "$hook_state_captured" -eq 1 ]] || return 1
  if [[ "$pre_commit_published" -eq 1 ]]; then
    assert_target_state "$hooks_path/pre-commit" "$pre_commit_published_identity" "$pre_commit_published_hash" || return 1
  elif [[ "${pre_commit_restored:-0}" -eq 1 ]]; then
    assert_target_state "$hooks_path/pre-commit" "$pre_commit_restored_identity" "$pre_commit_restored_hash" || return 1
  else
    assert_target_state "$hooks_path/pre-commit" "$pre_commit_initial_identity" "$pre_commit_initial_hash" || return 1
  fi
  if [[ "$pre_push_published" -eq 1 ]]; then
    assert_target_state "$hooks_path/pre-push" "$pre_push_published_identity" "$pre_push_published_hash" || return 1
  elif [[ "${pre_push_restored:-0}" -eq 1 ]]; then
    assert_target_state "$hooks_path/pre-push" "$pre_push_restored_identity" "$pre_push_restored_hash" || return 1
  else
    assert_target_state "$hooks_path/pre-push" "$pre_push_initial_identity" "$pre_push_initial_hash" || return 1
  fi
}
if [[ -L "$repo_root/scripts/materialize-trusted-seed-cargo-lock.sh" || ! -f "$repo_root/scripts/materialize-trusted-seed-cargo-lock.sh" ]]; then
  printf 'generated-workflows: trusted seed sentinel bootstrap helper is missing or unsafe in the source checkout\n' >&2
  exit 1
fi
# A fresh clone has no untracked scripts/trusted-seed-cargo.lock; restore the
# reviewed sentinel from the tracked metadata pack (idempotent; never rewrites an
# existing sentinel) before this installer requires and re-verifies its digest.
if [[ ! -e "$repo_root/scripts/trusted-seed-cargo.lock" && ! -L "$repo_root/scripts/trusted-seed-cargo.lock" ]]; then
  /bin/bash "$repo_root/scripts/materialize-trusted-seed-cargo-lock.sh"
fi
for path in "$repo_root/scripts/check-generated-workflows.sh" "$repo_root/scripts/generated-workflow-guard-config.toml" "$repo_root/scripts/python-inventory.sh" "$repo_root/scripts/trusted-seed.commit" "$repo_root/scripts/trusted-seed-metadata.pack" "$repo_root/scripts/trusted-seed-metadata.objects" "$repo_root/scripts/trusted-seed-cargo.lock" "$repo_root/scripts/trusted-seed-cargo-policy.json" "$repo_root/scripts/python-mise.toml" "$repo_root/scripts/$python_lock_file"; do
  if [[ -L "$path" || ! -f "$path" ]]; then
    printf 'generated-workflows: reviewed installation input is not a regular file\n' >&2
    exit 1
  fi
done
actual_seed_cargo_policy_size=$(file_size "$repo_root/scripts/trusted-seed-cargo-policy.json")
if [[ ! "$actual_seed_cargo_policy_size" =~ ^[0-9]+$ || "$actual_seed_cargo_policy_size" -gt 262144 ]]; then
  printf 'generated-workflows: trusted seed Cargo policy exceeds its reviewed size bound\n' >&2
  exit 1
fi
actual_checker_sha256=$(sha256_file "$repo_root/scripts/check-generated-workflows.sh")
actual_config_sha256=$(sha256_file "$repo_root/scripts/generated-workflow-guard-config.toml")
actual_inventory_helper_sha256=$(sha256_file "$repo_root/scripts/python-inventory.sh")
actual_python_mise_sha256=$(sha256_file "$repo_root/scripts/python-mise.toml")
actual_python_lock_sha256=$(sha256_file "$repo_root/scripts/$python_lock_file")
actual_metadata_pack_sha256=$(sha256_file "$repo_root/scripts/trusted-seed-metadata.pack")
actual_metadata_object_list_sha256=$(sha256_file "$repo_root/scripts/trusted-seed-metadata.objects")
actual_seed_cargo_lock_sha256=$(sha256_file "$repo_root/scripts/trusted-seed-cargo.lock")
actual_seed_cargo_policy_sha256=$(sha256_file "$repo_root/scripts/trusted-seed-cargo-policy.json")
if [[ "$actual_seed_cargo_lock_sha256" != "$reviewed_seed_cargo_lock_sha256" ]]; then
  printf 'generated-workflows: checkout checker, configuration, inventory helper, or trusted seed Cargo.lock differs from the reviewed installer pins\n' >&2
  exit 1
fi
if [[ "$actual_seed_cargo_policy_sha256" != "$reviewed_seed_cargo_policy_sha256" ]]; then
  printf 'generated-workflows: trusted seed Cargo policy differs from the reviewed installer pins\n' >&2
  exit 1
fi
if [[ "$actual_checker_sha256" != "$reviewed_checker_sha256" || "$actual_config_sha256" != "$reviewed_config_sha256" || "$actual_inventory_helper_sha256" != "$reviewed_inventory_helper_sha256" || "$actual_python_mise_sha256" != "$reviewed_python_mise_sha256" || "$actual_python_lock_sha256" != "$python_lock_sha256" || "$actual_metadata_pack_sha256" != "$reviewed_metadata_pack_sha256" || "$actual_metadata_object_list_sha256" != "$reviewed_metadata_object_list_sha256" ]]; then
  printf 'generated-workflows: checkout checker, configuration, inventory helper, or trusted seed metadata differs from the reviewed installer pins\n' >&2
  exit 1
fi
seed_object=$(git_clean hash-object -t commit --stdin < "$repo_root/scripts/trusted-seed.commit") || exit 1
if [[ "$seed_object" != "$reviewed_seed_commit" ]]; then
  printf 'generated-workflows: embedded reviewed seed commit does not match its Git object ID\n' >&2
  exit 1
fi

umask 077
if [[ -e "$install_root" || -L "$install_root" ]]; then
  if [[ -L "$install_root" || ! -d "$install_root" ]]; then
    printf 'generated-workflows: private install root is unsafe\n' >&2
    exit 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    install_owner=$(/usr/bin/stat -f '%u' "$install_root")
    install_mode=$(/usr/bin/stat -f '%Lp' "$install_root")
  else
    install_owner=$(/usr/bin/stat -c '%u' "$install_root")
    install_mode=$(/usr/bin/stat -c '%a' "$install_root")
  fi
  if [[ "$install_owner" != "$(/usr/bin/id -u)" ]]; then
    printf 'generated-workflows: private install root has the wrong owner\n' >&2
    exit 1
  fi
  if [[ "$install_mode" != 700 ]]; then
    printf 'generated-workflows: private install root must have mode 700\n' >&2
    exit 1
  fi
else
  mkdir "$install_root"
  chmod 700 "$install_root"
fi
if [[ -e "$lock_dir" || -L "$lock_dir" ]]; then
  if [[ -L "$lock_dir" || ! -d "$lock_dir" ]]; then
    printf 'generated-workflows: legacy installer lock path is unsafe\n' >&2
    exit 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    legacy_lock_owner=$(/usr/bin/stat -f '%u' "$lock_dir")
    legacy_lock_mode=$(/usr/bin/stat -f '%Lp' "$lock_dir")
  else
    legacy_lock_owner=$(/usr/bin/stat -c '%u' "$lock_dir")
    legacy_lock_mode=$(/usr/bin/stat -c '%a' "$lock_dir")
  fi
  if [[ "$legacy_lock_owner" != "$(/usr/bin/id -u)" ]] || (( (8#$legacy_lock_mode & 077) != 0 )); then
    printf 'generated-workflows: legacy installer lock directory is unsafe\n' >&2
    exit 1
  fi
fi
lock_file="$install_root/install.advisory.lock"
if [[ -L "$lock_file" || ( -e "$lock_file" && ! -f "$lock_file" ) ]]; then
  printf 'generated-workflows: advisory installer lock file is unsafe\n' >&2
  exit 1
fi
if [[ ! -x /usr/bin/perl ]]; then
  printf 'generated-workflows: /usr/bin/perl is required for the installer lock\n' >&2
  exit 127
fi
IFS= read -r -d '' installer_lock_perl <<'PERL' || true
use strict;
use warnings;
use Cwd qw(realpath);
use Errno qw(EAGAIN EWOULDBLOCK);
use Fcntl qw(:DEFAULT :flock :mode);
use File::Basename qw(dirname basename);

my ($action, $path) = @ARGV;
die "generated-workflows: installer lock arguments are invalid\n"
    unless defined $action && defined $path && ($action eq "create" || $action eq "hold");
my $parent = realpath(dirname($path));
die "generated-workflows: installer lock parent is not canonical\n"
    unless defined $parent && "$parent/" . basename($path) eq $path;
sub validate_lock {
    my ($handle, $leaf) = @_;
    my @opened = stat($handle);
    my @named = lstat($leaf);
    die "generated-workflows: installer lock metadata is unavailable\n"
        unless @opened && @named;
    die "generated-workflows: installer lock must be the canonical owned regular inode\n"
        unless S_ISREG($opened[2]) && S_ISREG($named[2])
            && $opened[0] == $named[0] && $opened[1] == $named[1]
            && $opened[4] == $> && $named[4] == $>
            && (($opened[2] & 0077) == 0) && (($named[2] & 0077) == 0)
            && $opened[3] == 1 && $named[3] == 1;
}
if ($action eq "create") {
    my $nofollow = eval { Fcntl::O_NOFOLLOW() };
    die "generated-workflows: Perl lacks O_NOFOLLOW support\n" if $@ || !defined $nofollow;
    my @before = lstat($path);
    die "generated-workflows: advisory installer lock is not a regular file\n"
        if @before && !S_ISREG($before[2]);
    my $handle;
    sysopen($handle, $path, O_RDONLY | O_CREAT | $nofollow, 0600)
        or die "generated-workflows: cannot safely open advisory installer lock: $!\n";
    validate_lock($handle, $path);
    close($handle) or die "generated-workflows: cannot close advisory installer lock\n";
    exit 0;
}
my $handle;
open($handle, "<&=9")
    or die "generated-workflows: cannot attach inherited advisory lock descriptor: $!\n";
validate_lock($handle, $path);
my $locked = flock($handle, LOCK_EX | LOCK_NB);
if (!$locked) {
    exit 75 if $! == EAGAIN || $! == EWOULDBLOCK;
    die "generated-workflows: native advisory installer lock failed: $!\n";
}
my $origin = getppid();
$| = 1;
print "locked\n" or die "generated-workflows: cannot report installer lock acquisition: $!\n";
for (;;) {
    last if getppid() != $origin;
    select(undef, undef, undef, 0.05);
}
exit 0;
PERL
installer_lock_helper() {
  /usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin /usr/bin/perl -e "$installer_lock_perl" "$@"
}
installer_lock_helper create "$lock_file"
exec 9< "$lock_file"
lock_state_file=$(/usr/bin/mktemp "$install_root/.lock-holder.XXXXXX")
/usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin /usr/bin/perl -e "$installer_lock_perl" hold "$lock_file" < /dev/null > "$lock_state_file" 2>&1 &
lock_holder_pid=$!
lock_holder_deadline=$((SECONDS + 30))
lock_holder_state=
while (( SECONDS < lock_holder_deadline )); do
  lock_holder_state=
  IFS= read -r lock_holder_state < "$lock_state_file" || true
  if [[ "$lock_holder_state" == locked ]]; then
    break
  fi
  kill -0 "$lock_holder_pid" 2>/dev/null || break
  sleep 0.05
done
if [[ "$lock_holder_state" != locked ]]; then
  lock_status=0
  kill "$lock_holder_pid" 2>/dev/null || true
  wait "$lock_holder_pid" 2>/dev/null || lock_status=$?
  /bin/rm -f "$lock_state_file"
  if [[ "$lock_status" -eq 75 ]]; then
    printf 'generated-workflows: another trusted checker installation is in progress\n' >&2
    exit 75
  fi
  printf 'generated-workflows: cannot acquire the native advisory installer lock\n' >&2
  exit "$lock_status"
fi
if ! kill -0 "$lock_holder_pid" 2>/dev/null; then
  /bin/rm -f "$lock_state_file"
  printf 'generated-workflows: the native advisory installer lock holder exited unexpectedly\n' >&2
  exit 1
fi
exec 9<&-
cleanup() {
  unset trusted_setup_token
  if [[ -n "${lock_holder_pid:-}" ]] && kill -0 "$lock_holder_pid" 2>/dev/null; then
    kill "$lock_holder_pid" 2>/dev/null || true
    wait "$lock_holder_pid" 2>/dev/null || true
  fi
  if [[ -n "${lock_state_file:-}" && -f "$lock_state_file" ]]; then
    /bin/rm -f "$lock_state_file"
  fi
  for hook_temp in "${pre_commit_temp:-}" "${pre_push_temp:-}"; do
    if [[ -n "$hook_temp" && -f "$hook_temp" && ! -L "$hook_temp" ]]; then
      /bin/rm -f "$hook_temp"
    fi
  done
  if [[ "${preserve_hook_backups:-0}" -ne 1 ]]; then
    for hook_backup in "${pre_commit_backup:-}" "${pre_push_backup:-}"; do
      if [[ -n "$hook_backup" && -f "$hook_backup" && ! -L "$hook_backup" ]]; then
        /bin/rm -f "$hook_backup"
      fi
    done
  fi
  if [[ -n "${stage_dir:-}" && -d "$stage_dir" ]]; then
    find "$stage_dir" -type d -exec chmod u+rwx {} + 2>/dev/null || true
    find "$stage_dir" -type f -exec chmod u+w {} + 2>/dev/null || true
    find "$stage_dir" -depth -delete
  fi
  if [[ -n "${python_stage:-}" && -d "$python_stage" ]]; then
    find "$python_stage" -type d -exec chmod u+w {} + 2>/dev/null || true
    find "$python_stage" -type f -exec chmod u+w {} + 2>/dev/null || true
    find "$python_stage" -depth -delete 2>/dev/null || true
  fi
  if [[ -n "${mise_download:-}" && -f "$mise_download" && ! -L "$mise_download" ]]; then
    /bin/rm -f "$mise_download"
  fi
}
trap cleanup EXIT
validate_hook_pair || exit 1
capture_hook_baseline || exit 1
validate_hook_pair || exit 1
assert_expected_hook_states || {
  printf 'generated-workflows: existing hook changed during initial validation\n' >&2
  exit 1
}
validate_hook_pair || exit 1
assert_expected_hook_states || {
  printf 'generated-workflows: existing hook changed before installation preparation\n' >&2
  exit 1
}
ensure_private_directory() {
  local path=$1
  if [[ -L "$path" || ( -e "$path" && ! -d "$path" ) ]]; then
    printf 'generated-workflows: private installation directory is unsafe\n' >&2
    return 1
  fi
  if [[ ! -e "$path" ]]; then
    mkdir "$path" || return $?
    chmod 700 "$path" || return $?
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    path_owner=$(/usr/bin/stat -f '%u' "$path")
    path_mode=$(/usr/bin/stat -f '%Lp' "$path")
  else
    path_owner=$(/usr/bin/stat -c '%u' "$path")
    path_mode=$(/usr/bin/stat -c '%a' "$path")
  fi
  if [[ "$path_owner" != "$(/usr/bin/id -u)" ]] || (( (8#$path_mode & 077) != 0 )); then
    printf 'generated-workflows: private installation directory owner or permissions are unsafe\n' >&2
    return 1
  fi
}
quarantine_directory() {
  local path=$1
  local parent
  local owner
  local mode
  local quarantine
  local current_user
  local restored_mode
  if [[ -L "$path" || ! -d "$path" ]] || ! owned_private_directory "$path"; then
    printf 'generated-workflows: refusing to quarantine an unsafe directory\n' >&2
    return 1
  fi
  parent=$(dirname "$path")
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$path")
    mode=$(/usr/bin/stat -f '%Lp' "$path")
  else
    owner=$(/usr/bin/stat -c '%u' "$path")
    mode=$(/usr/bin/stat -c '%a' "$path")
  fi
  current_user=$(/usr/bin/id -u)
  if [[ "$owner" != "$current_user" || ! "$mode" =~ ^[0-7]+$ ]] || (( (8#$mode & 077) != 0 )); then
    printf 'generated-workflows: refusing to quarantine a foreign or unsafe directory\n' >&2
    return 1
  fi
  quarantine=$(/usr/bin/mktemp -d "$parent/.quarantine.XXXXXX") || return $?
  if ! chmod 700 "$quarantine"; then
    rmdir "$quarantine" 2>/dev/null || true
    printf 'generated-workflows: cannot prepare the invalid owned generation quarantine\n' >&2
    return 1
  fi
  if [[ -L "$path" || ! -d "$path" ]] || ! owned_private_directory "$path"; then
    rmdir "$quarantine" 2>/dev/null || true
    printf 'generated-workflows: refusing to quarantine an unsafe directory\n' >&2
    return 1
  fi
  if ! chmod u+w "$path"; then
    rmdir "$quarantine" 2>/dev/null || true
    printf 'generated-workflows: cannot make the invalid owned generation renameable\n' >&2
    return 1
  fi
  if ! mv "$path" "$quarantine/generation"; then
    if owned_private_directory "$path"; then
      chmod "$mode" "$path" || {
        printf 'generated-workflows: cannot restore the invalid owned generation mode after quarantine failure\n' >&2
        return 1
      }
      if [[ "$platform" == macos-arm64 ]]; then
        restored_mode=$(/usr/bin/stat -f '%Lp' "$path")
      else
        restored_mode=$(/usr/bin/stat -c '%a' "$path")
      fi
      if [[ "$restored_mode" != "$mode" ]]; then
        printf 'generated-workflows: invalid owned generation mode changed after quarantine failure\n' >&2
        return 1
      fi
    fi
    rmdir "$quarantine" 2>/dev/null || true
    printf 'generated-workflows: cannot quarantine the invalid owned generation\n' >&2
    return 1
  fi
  if [[ -L "$quarantine/generation" || ! -d "$quarantine/generation" ]] || ! owned_private_directory "$quarantine/generation"; then
    printf 'generated-workflows: cannot verify the quarantined owned generation before mode restoration\n' >&2
    return 1
  fi
  if ! chmod "$mode" "$quarantine/generation"; then
    if [[ ! -e "$path" && ! -L "$path" ]] && owned_private_directory "$quarantine/generation" && mv "$quarantine/generation" "$path"; then
      if owned_private_directory "$path"; then
        if ! chmod "$mode" "$path"; then
          printf 'generated-workflows: cannot restore the invalid owned generation mode after quarantine rollback\n' >&2
          return 1
        fi
        if [[ "$platform" == macos-arm64 ]]; then
          restored_mode=$(/usr/bin/stat -f '%Lp' "$path")
        else
          restored_mode=$(/usr/bin/stat -c '%a' "$path")
        fi
        if [[ "$restored_mode" != "$mode" ]]; then
          printf 'generated-workflows: invalid owned generation mode changed after quarantine rollback\n' >&2
          return 1
        fi
      fi
      rmdir "$quarantine" 2>/dev/null || true
    fi
    printf 'generated-workflows: cannot restore the invalid owned generation mode after quarantine\n' >&2
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    restored_mode=$(/usr/bin/stat -f '%Lp' "$quarantine/generation")
  else
    restored_mode=$(/usr/bin/stat -c '%a' "$quarantine/generation")
  fi
  if [[ "$restored_mode" != "$mode" ]]; then
    printf 'generated-workflows: quarantined owned generation mode did not return to its original value\n' >&2
    return 1
  fi
}
quarantine_file() {
  local path=$1
  local parent
  local owner
  local mode
  local links
  local quarantine
  if [[ -L "$path" || ! -f "$path" ]]; then
    printf 'generated-workflows: refusing to quarantine an unsafe artifact\n' >&2
    return 1
  fi
  parent=$(dirname "$path")
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$path")
    mode=$(/usr/bin/stat -f '%Lp' "$path")
    links=$(/usr/bin/stat -f '%l' "$path")
  else
    owner=$(/usr/bin/stat -c '%u' "$path")
    mode=$(/usr/bin/stat -c '%a' "$path")
    links=$(/usr/bin/stat -c '%h' "$path")
  fi
  if [[ "$owner" != "$(/usr/bin/id -u)" || "$links" != 1 ]] || (( (8#$mode & 077) != 0 )); then
    printf 'generated-workflows: refusing to quarantine a foreign or unsafe artifact\n' >&2
    return 1
  fi
  quarantine=$(/usr/bin/mktemp -d "$parent/.quarantine.XXXXXX") || return $?
  chmod 700 "$quarantine"
  if ! mv "$path" "$quarantine/artifact"; then
    rmdir "$quarantine" 2>/dev/null || true
    printf 'generated-workflows: cannot quarantine the invalid owned artifact\n' >&2
    return 1
  fi
}
ensure_private_directory "$versions_dir"
ensure_private_directory "$cache_root"
ensure_private_directory "$artifact_root"
mise_artifact_dir="$artifact_root/mise/$mise_version/$platform"
if [[ "$hash_tool" == /usr/bin/shasum ]]; then
  python_generation=$(printf '%s\n' "$platform" "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$mise_version" "$mise_sha256" | "$hash_tool" -a 256 | /usr/bin/awk '{print $1}')
else
  python_generation=$(printf '%s\n' "$platform" "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$mise_version" "$mise_sha256" | "$hash_tool" | /usr/bin/awk '{print $1}')
fi
python_artifact_dir="$artifact_root/python/$python_version/$platform-$python_generation"
ensure_private_directory "$artifact_root/mise"
ensure_private_directory "$artifact_root/mise/$mise_version"
ensure_private_directory "$mise_artifact_dir"
ensure_private_directory "$artifact_root/python"
ensure_private_directory "$artifact_root/python/$python_version"
mise_artifact="$mise_artifact_dir/mise"
if [[ -e "$mise_artifact" || -L "$mise_artifact" ]]; then
  if [[ -L "$mise_artifact" || ! -f "$mise_artifact" ]]; then
    printf 'generated-workflows: approved mise artifact has an unsafe type\n' >&2
    exit 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    mise_owner=$(/usr/bin/stat -f '%u' "$mise_artifact")
    mise_mode=$(/usr/bin/stat -f '%Lp' "$mise_artifact")
    mise_links=$(/usr/bin/stat -f '%l' "$mise_artifact")
  else
    mise_owner=$(/usr/bin/stat -c '%u' "$mise_artifact")
    mise_mode=$(/usr/bin/stat -c '%a' "$mise_artifact")
    mise_links=$(/usr/bin/stat -c '%h' "$mise_artifact")
  fi
  if [[ "$mise_owner" != "$(/usr/bin/id -u)" || "$mise_links" != 1 ]] || (( (8#$mise_mode & 077) != 0 )); then
    printf 'generated-workflows: approved mise artifact is foreign or unsafe\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$mise_artifact")" != "$mise_sha256" ]]; then
    quarantine_file "$mise_artifact"
  fi
fi
if [[ ! -e "$mise_artifact" && ! -L "$mise_artifact" ]]; then
  mise_download=$(/usr/bin/mktemp "$install_root/.mise-download.XXXXXX")
  if ! env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="${TMPDIR:-/tmp}" TMPDIR="${TMPDIR:-/tmp}" LANG=C LC_ALL=C /usr/bin/curl --disable --proto '=https' --location --fail --silent --show-error --connect-timeout 10 --max-time 180 --max-filesize 268435456 --output "$mise_download" "https://github.com/jdx/mise/releases/download/v$mise_version/mise-v$mise_version-$platform"; then
    printf 'generated-workflows: cannot download the pinned official mise release\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$mise_download")" != "$mise_sha256" ]]; then
    printf 'generated-workflows: official mise artifact checksum does not match the reviewed release identity\n' >&2
    exit 1
  fi
  chmod 500 "$mise_download"
  if [[ -e "$mise_artifact" || -L "$mise_artifact" ]]; then
    printf 'generated-workflows: mise publication target appeared unexpectedly\n' >&2
    exit 1
  fi
  mv "$mise_download" "$mise_artifact"
  mise_download=
fi
python_runtime="$python_artifact_dir/runtime"
python_inventory="$python_artifact_dir/inventory"
python_artifact_manifest="$python_artifact_dir/manifest"
python_mise_config="$python_artifact_dir/mise.toml"
python_mise_lock="$python_artifact_dir/mise.lock"
python_generation_valid=0
if [[ -e "$python_artifact_dir" || -L "$python_artifact_dir" ]]; then
  if [[ -L "$python_artifact_dir" || ! -d "$python_artifact_dir" ]] || ! owned_private_directory "$python_artifact_dir"; then
    printf 'generated-workflows: pinned Python artifact generation root is foreign or unsafe\n' >&2
    exit 1
  fi
  if owned_private_directory "$python_runtime" && owned_private_file "$python_inventory" && owned_private_file "$python_artifact_manifest" && owned_private_file "$python_mise_config" && owned_private_file "$python_mise_lock"; then
    if [[ "$(sha256_file "$python_mise_config")" == "$reviewed_python_mise_sha256" && "$(sha256_file "$python_mise_lock")" == "$python_lock_sha256" ]]; then
      python_inventory_sha256=$(sha256_file "$python_inventory")
      expected_python_manifest=$(printf 'python_version=%s\nbackend=core:python\narchive_sha256=%s\nlock_sha256=%s\nmise_config_sha256=%s\ninventory_helper_sha256=%s\ninventory_sha256=%s' "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$python_inventory_sha256")
      if [[ "$(/bin/cat "$python_artifact_manifest")" == "$expected_python_manifest" ]] && "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
        python_generation_valid=1
      fi
    fi
  fi
  if [[ "$python_generation_valid" -ne 1 ]]; then
    quarantine_directory "$python_artifact_dir"
  fi
fi
if [[ "$python_generation_valid" -eq 0 ]]; then
  python_stage=$(/usr/bin/mktemp -d "$artifact_root/python/$python_version/.stage.XXXXXX")
  python_stage=$(cd "$python_stage" && pwd -P)
  python_stage_parent=$(cd "$python_stage/.." && pwd -P)
  chmod 700 "$python_stage"
  cp "$repo_root/scripts/python-mise.toml" "$python_stage/mise.toml"
  cp "$repo_root/scripts/$python_lock_file" "$python_stage/mise.lock"
  chmod 400 "$python_stage/mise.toml" "$python_stage/mise.lock"
  mkdir -p "$python_stage/global" "$python_stage/system" "$python_stage/home" "$python_stage/data" "$python_stage/cache" "$python_stage/tmp"
  : > "$python_stage/global/config.toml"
  : > "$python_stage/system/config.toml"
  python_mise_environment=(
    PATH=/usr/bin:/bin:/usr/sbin:/sbin
    "HOME=$python_stage/home"
    "TMPDIR=$python_stage/tmp"
    "MISE_DATA_DIR=$python_stage/data"
    "MISE_CACHE_DIR=$python_stage/cache"
    "MISE_CONFIG_DIR=$python_stage/global"
    "MISE_GLOBAL_CONFIG_FILE=$python_stage/global/config.toml"
    "MISE_SYSTEM_CONFIG_DIR=$python_stage/system"
    "MISE_SYSTEM_CONFIG_FILE=$python_stage/system/config.toml"
    "MISE_CONFIG_FILE=$python_stage/mise.toml"
    "MISE_TRUSTED_CONFIG_PATHS=$python_stage/mise.toml"
    "MISE_CEILING_PATHS=$python_stage_parent"
    "MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml"
    MISE_NO_ENV=1
    MISE_NO_HOOKS=1
    MISE_AUTO_INSTALL=0
    MISE_LOCKFILE=1
    MISE_PROVENANCE_API_FAILURES_FATAL=1
    LANG=C
    LC_ALL=C
  )
  if [[ "$(sha256_file "$python_stage/mise.toml")" != "$reviewed_python_mise_sha256" || "$(sha256_file "$python_stage/mise.lock")" != "$python_lock_sha256" ]]; then
    printf 'generated-workflows: staged private Python Mise configuration or lock failed integrity validation\n' >&2
    exit 1
  fi
  if ! env -i "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" config ls --json > "$python_stage/config-ls.json" 2> "$python_stage/config-ls.log"; then
    cat "$python_stage/config-ls.log" >&2
    printf 'generated-workflows: Mise could not discover the staged private Python configuration\n' >&2
    exit 1
  fi
  python_config_count=$(/usr/bin/grep -c '"path":' "$python_stage/config-ls.json" || true)
  if [[ "$python_config_count" -ne 3 ]] || ! /usr/bin/grep -F -x -q "    \"path\": \"$python_stage/mise.toml\"," "$python_stage/config-ls.json" || ! /usr/bin/grep -F -x -q "    \"path\": \"$python_stage/global/config.toml\"," "$python_stage/config-ls.json" || ! /usr/bin/grep -F -x -q "    \"path\": \"$python_stage/system/config.toml\"," "$python_stage/config-ls.json"; then
    cat "$python_stage/config-ls.json" >&2
    printf 'generated-workflows: Mise loaded an unexpected Python configuration set\n' >&2
    exit 1
  fi
  /bin/rm -f "$python_stage/config-ls.json" "$python_stage/config-ls.log"
  if ! env -i "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" install python > "$python_stage/install.log" 2>&1; then
    cat "$python_stage/install.log" >&2
    printf 'generated-workflows: Mise could not qualify the locked Python %s artifact\n' "$python_version" >&2
    exit 1
  fi
  if [[ "$(sha256_file "$python_stage/mise.toml")" != "$reviewed_python_mise_sha256" || "$(sha256_file "$python_stage/mise.lock")" != "$python_lock_sha256" ]]; then
    printf 'generated-workflows: private Python Mise configuration or lock changed during qualification\n' >&2
    exit 1
  fi
  mise_python_runtime="$python_stage/data/installs/python/$python_version"
  if [[ -L "$mise_python_runtime" || ! -d "$mise_python_runtime" ]]; then
    printf 'generated-workflows: private Mise Python installation is missing\n' >&2
    exit 1
  fi
  cp -Rp "$mise_python_runtime" "$python_stage/runtime"
  rm -rf "$python_stage/data" "$python_stage/cache" "$python_stage/home" "$python_stage/global" "$python_stage/system" "$python_stage/tmp"
  /bin/rm -f "$python_stage/install.log"
  runtime_python="$python_stage/runtime/bin/python3.15"
  if [[ -L "$runtime_python" || ! -f "$runtime_python" || ! -x "$runtime_python" ]]; then
    printf 'generated-workflows: locked relocatable Python executable is missing or unsafe\n' >&2
    exit 1
  fi
  find "$python_stage/runtime" -type f -exec chmod a-w {} +
  find "$python_stage/runtime" -type d -exec chmod a-w {} +
  "$repo_root/scripts/python-inventory.sh" write "$python_stage/runtime" "$python_stage/inventory"
  python_inventory_sha256=$(sha256_file "$python_stage/inventory")
  printf 'python_version=%s\nbackend=core:python\narchive_sha256=%s\nlock_sha256=%s\nmise_config_sha256=%s\ninventory_helper_sha256=%s\ninventory_sha256=%s\n' "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$python_inventory_sha256" > "$python_stage/manifest"
  chmod 400 "$python_stage/mise.toml" "$python_stage/mise.lock" "$python_stage/inventory" "$python_stage/manifest"
  chmod 500 "$python_stage/runtime" "$python_stage"
  mv "$python_stage" "$python_artifact_dir"
  python_stage=
  "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"
fi
python_inventory_sha256=$(sha256_file "$python_inventory")
python_relative_path=bin/python3.15
if [[ ! -x "$python_runtime/$python_relative_path" || -L "$python_runtime/$python_relative_path" ]]; then
  printf 'generated-workflows: pinned Python executable is missing or symlinked\n' >&2
  exit 1
fi
if ! "$python_runtime/$python_relative_path" -I -S -c 'import sys, tomllib; sys.exit(sys.version_info[:3] != (3, 15, 2))'; then
  printf 'generated-workflows: locked private Python runtime failed after publication\n' >&2
  exit 1
fi
if [[ -e "$version_dir" || -L "$version_dir" ]]; then
  if [[ -L "$version_dir" || ! -d "$version_dir" || ! -f "$version_dir/check-generated-workflows.sh" || -L "$version_dir/check-generated-workflows.sh" || ! -f "$version_dir/config.toml" || -L "$version_dir/config.toml" || ! -f "$version_dir/trusted-seed.commit" || -L "$version_dir/trusted-seed.commit" || ! -f "$version_dir/trusted-seed-metadata.pack" || -L "$version_dir/trusted-seed-metadata.pack" || ! -f "$version_dir/trusted-seed-metadata.objects" || -L "$version_dir/trusted-seed-metadata.objects" || ! -f "$version_dir/trusted-seed-cargo.lock" || -L "$version_dir/trusted-seed-cargo.lock" || ! -f "$version_dir/trusted-seed-cargo-policy.json" || -L "$version_dir/trusted-seed-cargo-policy.json" || ! -f "$version_dir/manifest" || -L "$version_dir/manifest" ]]; then
    printf 'generated-workflows: installed checker generation is unsafe\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$version_dir/check-generated-workflows.sh")" != "$reviewed_checker_sha256" || "$(sha256_file "$version_dir/config.toml")" != "$reviewed_config_sha256" || "$(sha256_file "$version_dir/python-inventory.sh")" != "$reviewed_inventory_helper_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.pack")" != "$reviewed_metadata_pack_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.objects")" != "$reviewed_metadata_object_list_sha256" || "$(sha256_file "$version_dir/trusted-seed-cargo.lock")" != "$reviewed_seed_cargo_lock_sha256" || "$(sha256_file "$version_dir/trusted-seed-cargo-policy.json")" != "$reviewed_seed_cargo_policy_sha256" || "$(git_clean hash-object -t commit --stdin < "$version_dir/trusted-seed.commit")" != "$reviewed_seed_commit" ]]; then
    printf 'generated-workflows: installed checker generation failed integrity validation\n' >&2
    exit 1
  fi
else
  stage_dir="$versions_dir/.stage.$$"
  mkdir "$stage_dir"
  chmod 700 "$stage_dir"
  cp "$repo_root/scripts/check-generated-workflows.sh" "$stage_dir/check-generated-workflows.sh"
  cp "$repo_root/scripts/generated-workflow-guard-config.toml" "$stage_dir/config.toml"
  cp "$repo_root/scripts/python-inventory.sh" "$stage_dir/python-inventory.sh"
  cp "$repo_root/scripts/trusted-seed.commit" "$stage_dir/trusted-seed.commit"
  cp "$repo_root/scripts/trusted-seed-metadata.pack" "$stage_dir/trusted-seed-metadata.pack"
  cp "$repo_root/scripts/trusted-seed-metadata.objects" "$stage_dir/trusted-seed-metadata.objects"
  cp "$repo_root/scripts/trusted-seed-cargo.lock" "$stage_dir/trusted-seed-cargo.lock"
  cp "$repo_root/scripts/trusted-seed-cargo-policy.json" "$stage_dir/trusted-seed-cargo-policy.json"
  chmod 400 "$stage_dir/check-generated-workflows.sh" "$stage_dir/config.toml" "$stage_dir/trusted-seed.commit" "$stage_dir/trusted-seed-metadata.pack" "$stage_dir/trusted-seed-metadata.objects" "$stage_dir/trusted-seed-cargo.lock" "$stage_dir/trusted-seed-cargo-policy.json"
  chmod 500 "$stage_dir/python-inventory.sh"
  printf 'version=%s\nchecker_version=%s\nchecker_sha256=%s\nconfig_sha256=%s\ninventory_helper_sha256=%s\nseed_commit=%s\nseed_tree=%s\ntrusted_seed_cargo_lock_sha256=%s\ntrusted_seed_cargo_policy_sha256=%s\nmetadata_pack_sha256=%s\nmetadata_object_list_sha256=%s\nmise_version=%s\nmise_sha256=%s\npython_version=%s\npython_artifact_sha256=%s\npython_lock_sha256=%s\npython_mise_sha256=%s\ninventory_sha256=%s\n' "$install_version" "$install_version" "$reviewed_checker_sha256" "$reviewed_config_sha256" "$reviewed_inventory_helper_sha256" "$reviewed_seed_commit" "$reviewed_seed_tree" "$reviewed_seed_cargo_lock_sha256" "$reviewed_seed_cargo_policy_sha256" "$reviewed_metadata_pack_sha256" "$reviewed_metadata_object_list_sha256" "$mise_version" "$mise_sha256" "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$python_inventory_sha256" > "$stage_dir/manifest"
  chmod 400 "$stage_dir/manifest"
  chmod 500 "$stage_dir"
  if [[ -L "$repo_root/scripts/trusted-seed-cargo.lock" || ! -f "$repo_root/scripts/trusted-seed-cargo.lock" || "$(sha256_file "$repo_root/scripts/trusted-seed-cargo.lock")" != "$reviewed_seed_cargo_lock_sha256" || -L "$repo_root/scripts/trusted-seed-cargo-policy.json" || ! -f "$repo_root/scripts/trusted-seed-cargo-policy.json" || "$(sha256_file "$repo_root/scripts/trusted-seed-cargo-policy.json")" != "$reviewed_seed_cargo_policy_sha256" || "$(sha256_file "$stage_dir/check-generated-workflows.sh")" != "$reviewed_checker_sha256" || "$(sha256_file "$stage_dir/config.toml")" != "$reviewed_config_sha256" || "$(sha256_file "$stage_dir/trusted-seed-metadata.pack")" != "$reviewed_metadata_pack_sha256" || "$(sha256_file "$stage_dir/trusted-seed-metadata.objects")" != "$reviewed_metadata_object_list_sha256" || "$(sha256_file "$stage_dir/trusted-seed-cargo.lock")" != "$reviewed_seed_cargo_lock_sha256" || "$(sha256_file "$stage_dir/trusted-seed-cargo-policy.json")" != "$reviewed_seed_cargo_policy_sha256" || "$(git_clean hash-object -t commit --stdin < "$stage_dir/trusted-seed.commit")" != "$reviewed_seed_commit" ]]; then
    printf 'generated-workflows: staged checker generation failed integrity validation\n' >&2
    exit 1
  fi
  mv "$stage_dir" "$version_dir"
  stage_dir=
fi

if [[ "$(sha256_file "$version_dir/check-generated-workflows.sh")" != "$reviewed_checker_sha256" || "$(sha256_file "$version_dir/config.toml")" != "$reviewed_config_sha256" || "$(sha256_file "$version_dir/python-inventory.sh")" != "$reviewed_inventory_helper_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.pack")" != "$reviewed_metadata_pack_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.objects")" != "$reviewed_metadata_object_list_sha256" || "$(sha256_file "$version_dir/trusted-seed-cargo.lock")" != "$reviewed_seed_cargo_lock_sha256" || "$(sha256_file "$version_dir/trusted-seed-cargo-policy.json")" != "$reviewed_seed_cargo_policy_sha256" || "$(sha256_file "$python_inventory")" != "$python_inventory_sha256" ]]; then
  printf 'generated-workflows: installed checker or runtime artifact failed final integrity validation\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -x -q "trusted_seed_cargo_lock_sha256=$reviewed_seed_cargo_lock_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "trusted_seed_cargo_policy_sha256=$reviewed_seed_cargo_policy_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "metadata_pack_sha256=$reviewed_metadata_pack_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "metadata_object_list_sha256=$reviewed_metadata_object_list_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "seed_tree=$reviewed_seed_tree" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "inventory_sha256=$python_inventory_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "python_artifact_sha256=$python_artifact_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "python_lock_sha256=$python_lock_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "python_mise_sha256=$reviewed_python_mise_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "mise_sha256=$mise_sha256" "$version_dir/manifest"; then
  printf 'generated-workflows: installed checker manifest does not bind the qualified runtime artifacts\n' >&2
  exit 1
fi
if ! validate_installed_generation "$reviewed_checker_sha256"; then
  printf 'generated-workflows: installed checker generation failed artifact validation\n' >&2
  exit 1
fi
prepare_environment=(/usr/bin/env -i)
if [[ -n "$trusted_setup_token" ]]; then
  prepare_environment+=(-S 'GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}')
fi
GITHUB_COM_TOKEN="$trusted_setup_token" "${prepare_environment[@]}" PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LANG=C LC_ALL=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 /bin/bash "$version_dir/check-generated-workflows.sh" prepare-cache || {
  printf 'generated-workflows: trusted tool and dependency cache qualification failed; no new hook was installed\n' >&2
  exit 1
}

validate_hook_pair || exit 1
assert_expected_hook_states || {
  printf 'generated-workflows: existing hook changed during cache preparation\n' >&2
  exit 1
}

unset trusted_setup_token

write_hook() {
  local mode=$1
  local temporary=$2
  render_hook "$mode" > "$temporary"
  chmod 700 "$temporary"
}

make_hook_backup() {
  local target=$1
  local hook_name=$2
  local initial_identity
  local initial_hash
  local initial_mode
  local backup
  case "$hook_name" in
    pre-commit)
      initial_identity=$pre_commit_initial_identity
      initial_hash=$pre_commit_initial_hash
      initial_mode=$pre_commit_initial_mode
      ;;
    pre-push)
      initial_identity=$pre_push_initial_identity
      initial_hash=$pre_push_initial_hash
      initial_mode=$pre_push_initial_mode
      ;;
    *) return 2 ;;
  esac
  if [[ "$initial_hash" == missing ]]; then
    return 0
  fi
  assert_target_state "$target" "$initial_identity" "$initial_hash" || return 1
  backup=$(/usr/bin/mktemp "$hooks_dir/.jackin-generated-workflow-guard.$$.${hook_name}.backup.XXXXXX") || return 1
  if ! /bin/cat "$target" > "$backup" || ! chmod 600 "$backup" || ! limited_private_file "$backup" 1048576; then
    /bin/rm -f "$backup"
    return 1
  fi
  if [[ "$(sha256_file "$backup")" != "$initial_hash" || "$(file_mode "$backup")" != 600 ]]; then
    /bin/rm -f "$backup"
    return 1
  fi
  case "$hook_name" in
    pre-commit) pre_commit_backup=$backup ;;
    pre-push) pre_push_backup=$backup ;;
  esac
}

restore_hook() {
  local target=$1
  local hook_name=$2
  local initial_identity
  local initial_hash
  local initial_mode
  local published_identity
  local published_hash
  local was_published
  local backup
  local restore_temp
  case "$hook_name" in
    pre-commit)
      initial_identity=$pre_commit_initial_identity
      initial_hash=$pre_commit_initial_hash
      initial_mode=$pre_commit_initial_mode
      published_identity=${pre_commit_published_identity:-}
      published_hash=${pre_commit_published_hash:-}
      was_published=$pre_commit_published
      backup=${pre_commit_backup:-}
      ;;
    pre-push)
      initial_identity=$pre_push_initial_identity
      initial_hash=$pre_push_initial_hash
      initial_mode=$pre_push_initial_mode
      published_identity=${pre_push_published_identity:-}
      published_hash=${pre_push_published_hash:-}
      was_published=$pre_push_published
      backup=${pre_push_backup:-}
      ;;
    *) return 2 ;;
  esac
  [[ "$was_published" -eq 1 ]] || return 0
  if ! assert_target_state "$target" "$published_identity" "$published_hash"; then
    printf 'generated-workflows: preserving a changed hook during rollback at %s\n' "$target" >&2
    return 1
  fi
  if [[ "$initial_hash" == missing ]]; then
    /bin/rm -f "$target" || return 1
    if [[ "$hook_name" == pre-commit ]]; then
      pre_commit_published=0
      pre_commit_restored=1
      pre_commit_restored_identity=missing
      pre_commit_restored_hash=missing
    else
      pre_push_published=0
      pre_push_restored=1
      pre_push_restored_identity=missing
      pre_push_restored_hash=missing
    fi
    return 0
  fi
  if [[ -z "$backup" ]] || ! limited_private_file "$backup" 1048576 || [[ "$(sha256_file "$backup")" != "$initial_hash" || "$(file_mode "$backup")" != 600 ]]; then
    printf 'generated-workflows: refusing to restore an unverified hook backup for %s\n' "$hook_name" >&2
    return 1
  fi
  restore_temp=$(/usr/bin/mktemp "$hooks_dir/.jackin-generated-workflow-guard.$$.${hook_name}.restore.XXXXXX") || return 1
  if ! /bin/cat "$backup" > "$restore_temp" || ! chmod 600 "$restore_temp" || ! limited_private_file "$restore_temp" 1048576 || [[ "$(sha256_file "$restore_temp")" != "$initial_hash" || "$(file_mode "$restore_temp")" != 600 ]]; then
    /bin/rm -f "$restore_temp"
    printf 'generated-workflows: cannot stage a private verified %s hook restoration\n' "$hook_name" >&2
    return 1
  fi
  if ! assert_target_state "$target" "$published_identity" "$published_hash" || ! mv -f "$restore_temp" "$target"; then
    /bin/rm -f "$restore_temp"
    printf 'generated-workflows: cannot restore the verified previous %s hook\n' "$hook_name" >&2
    return 1
  fi
  if ! chmod "$initial_mode" "$target"; then
    printf 'generated-workflows: restored %s hook could not recover its validated mode\n' "$hook_name" >&2
    return 1
  fi
  if [[ "$(sha256_file "$target")" != "$initial_hash" || "$(file_mode "$target")" != "$initial_mode" ]]; then
    printf 'generated-workflows: restored %s hook failed byte or mode verification\n' "$hook_name" >&2
    return 1
  fi
  if [[ "$hook_name" == pre-commit ]]; then
    pre_commit_backup=
    pre_commit_published=0
    pre_commit_restored=1
    pre_commit_restored_identity=$(file_identity "$target") || return 1
    pre_commit_restored_hash=$(sha256_file "$target") || return 1
  else
    pre_push_backup=
    pre_push_published=0
    pre_push_restored=1
    pre_push_restored_identity=$(file_identity "$target") || return 1
    pre_push_restored_hash=$(sha256_file "$target") || return 1
  fi
}

rollback_hooks() {
  local rollback_failed=0
  if [[ "$pre_commit_published" -ne 1 && "${pre_commit_restored:-0}" -ne 1 ]] && ! assert_target_state "$hooks_dir/pre-commit" "$pre_commit_initial_identity" "$pre_commit_initial_hash"; then
    rollback_failed=1
  fi
  if [[ "$pre_push_published" -ne 1 && "${pre_push_restored:-0}" -ne 1 ]] && ! assert_target_state "$hooks_dir/pre-push" "$pre_push_initial_identity" "$pre_push_initial_hash"; then
    rollback_failed=1
  fi
  restore_hook "$hooks_dir/pre-push" pre-push || rollback_failed=1
  restore_hook "$hooks_dir/pre-commit" pre-commit || rollback_failed=1
  if [[ "$rollback_failed" -ne 0 ]]; then
    preserve_hook_backups=1
    printf 'generated-workflows: rollback was incomplete; verified backups were preserved for recovery\n' >&2
    return 1
  fi
  /bin/rm -f "$pre_commit_backup" "$pre_push_backup"
  pre_commit_backup=
  pre_push_backup=
}

abort_publication() {
  local message=$1
  if ! rollback_hooks; then
    printf 'generated-workflows: %s; one or more changed hook paths were preserved\n' "$message" >&2
  else
    printf 'generated-workflows: %s; the previous registration was restored\n' "$message" >&2
  fi
  exit 1
}

if [[ -L "$hooks_path" ]]; then
  printf 'generated-workflows: effective hook directory is a symlink\n' >&2
  exit 1
fi
mkdir -p "$hooks_path"
hooks_dir=$(cd "$hooks_path" && pwd -P)
if [[ -L "$hooks_dir" || ! -d "$hooks_dir" ]]; then
  printf 'generated-workflows: effective hook directory is unsafe\n' >&2
  exit 1
fi
if [[ "$platform" == macos-arm64 ]]; then
  hooks_dir_owner=$(/usr/bin/stat -f '%u' "$hooks_dir")
  hooks_dir_mode=$(/usr/bin/stat -f '%Lp' "$hooks_dir")
else
  hooks_dir_owner=$(/usr/bin/stat -c '%u' "$hooks_dir")
  hooks_dir_mode=$(/usr/bin/stat -c '%a' "$hooks_dir")
fi
if [[ "$hooks_dir_owner" != "$(/usr/bin/id -u)" ]]; then
  printf 'generated-workflows: effective hook directory has the wrong owner\n' >&2
  exit 1
fi
if [[ ! "$hooks_dir_mode" =~ ^[0-7]{3,4}$ ]] || (( (8#$hooks_dir_mode & 18) != 0 )); then
  printf 'generated-workflows: effective hook directory is writable by group or others\n' >&2
  exit 1
fi
pre_commit_temp=$(/usr/bin/mktemp "$hooks_dir/.jackin-generated-workflow-guard.$$.pre-commit.new.XXXXXX") || exit 1
pre_push_temp=$(/usr/bin/mktemp "$hooks_dir/.jackin-generated-workflow-guard.$$.pre-push.new.XXXXXX") || exit 1
write_hook pre-commit "$pre_commit_temp" || exit 1
write_hook pre-push "$pre_push_temp" || exit 1
pre_commit_backup=
pre_push_backup=
pre_commit_restored=0
pre_push_restored=0
make_hook_backup "$hooks_dir/pre-commit" pre-commit || exit 1
make_hook_backup "$hooks_dir/pre-push" pre-push || exit 1
if ! validate_hook_pair || ! assert_expected_hook_states; then
  preserve_hook_backups=1
  printf 'generated-workflows: existing hook changed before publication\n' >&2
  exit 1
fi
if ! render_hook pre-commit | /usr/bin/cmp -s - "$pre_commit_temp" || ! render_hook pre-push | /usr/bin/cmp -s - "$pre_push_temp" || [[ "$(file_mode "$pre_commit_temp")" != 700 || "$(file_mode "$pre_push_temp")" != 700 ]]; then
  printf 'generated-workflows: staged hook render failed integrity validation\n' >&2
  exit 1
fi
if ! validate_hook_pair || ! assert_expected_hook_states; then
  preserve_hook_backups=1
  printf 'generated-workflows: existing hook changed before pre-commit publication\n' >&2
  exit 1
fi
pre_commit_expected_hash=$(sha256_file "$pre_commit_temp") || exit 1
pre_push_expected_hash=$(sha256_file "$pre_push_temp") || exit 1
pre_commit_staged_identity=$(file_identity "$pre_commit_temp") || exit 1
pre_push_staged_identity=$(file_identity "$pre_push_temp") || exit 1
if ! validate_hook_pair || ! assert_expected_hook_states; then
  preserve_hook_backups=1
  printf 'generated-workflows: existing hook changed immediately before pre-commit publication\n' >&2
  exit 1
fi
if ! mv -f "$pre_commit_temp" "$hooks_dir/pre-commit"; then
  abort_publication 'cannot install the pre-commit hook'
fi
pre_commit_published_identity=$pre_commit_staged_identity
pre_commit_published_hash=$pre_commit_expected_hash
pre_commit_published=1
if [[ "$(sha256_file "$hooks_dir/pre-commit")" != "$pre_commit_expected_hash" ]]; then
  preserve_hook_backups=1
  abort_publication 'pre-commit changed during publication'
fi
assert_target_state "$hooks_dir/pre-commit" "$pre_commit_published_identity" "$pre_commit_published_hash" || abort_publication 'pre-commit changed after publication'
if ! validate_hook_pair || ! assert_expected_hook_states; then
  abort_publication 'the hook pair changed after pre-commit publication'
fi
if ! validate_hook_pair || ! assert_expected_hook_states; then
  abort_publication 'existing hook changed before pre-push publication'
fi
if ! mv -f "$pre_push_temp" "$hooks_dir/pre-push"; then
  abort_publication 'cannot install the pre-push hook'
fi
pre_push_published_identity=$pre_push_staged_identity
pre_push_published_hash=$pre_push_expected_hash
pre_push_published=1
if [[ "$(sha256_file "$hooks_dir/pre-push")" != "$pre_push_expected_hash" ]]; then
  preserve_hook_backups=1
  abort_publication 'pre-push changed during publication'
fi
assert_target_state "$hooks_dir/pre-push" "$pre_push_published_identity" "$pre_push_published_hash" || abort_publication 'pre-push changed after publication'
if ! validate_hook_pair || ! assert_expected_hook_states; then
  abort_publication 'the hook pair changed after publication'
fi
/bin/rm -f "$pre_commit_backup" "$pre_push_backup"
pre_commit_backup=
pre_push_backup=
printf 'generated-workflows: installed trusted checker %s in Git common metadata\n' "$reviewed_checker_sha256"
