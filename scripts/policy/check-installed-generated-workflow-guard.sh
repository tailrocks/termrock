#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
# A fresh clone has no untracked scripts/trusted-seed-cargo.lock; restore the
# reviewed sentinel bytes from the tracked metadata pack before anything reads
# them (this script is idempotent and never rewrites an existing sentinel).
bash "$repo_root/scripts/materialize-trusted-seed-cargo-lock.sh" || exit $?
e2e_focus=${BASELINE_GUARD_E2E_FOCUS:-all}
cold_install=${BASELINE_GUARD_COLD_INSTALL:-0}
public_mutant=${BASELINE_GUARD_PUBLIC_MUTANT:-}
installer_mutant=${BASELINE_GUARD_INSTALLER_MUTANT:-}
policy_case=${BASELINE_GUARD_POLICY_CASE:-all}
policy_preflight_only=${BASELINE_GUARD_POLICY_PREFLIGHT_ONLY:-0}
policy_control_export=${BASELINE_GUARD_POLICY_CONTROL_EXPORT:-}
negative_policy_cases='external-checksum external-source external-version external-dependency-graph unknown-local-package forged-source-less-entry duplicate-package-identity ambiguous-unqualified-edge duplicate-edge unknown-lock-metadata unknown-package-metadata missing-external-record malformed-lock unresolved-local-edge root-lock-symlink nested-lock-symlink nested-lock-changed path-escape'
case "$e2e_focus" in
  mutant-cache-validation-only|cache-validation-only|all|installer-reinstall|installer-forged-hook|installer-managed-hook-upgrade|installer-repair-mise|installer-repair-mise-http-failure|installer-repair-mise-wrong-hash|installer-repair-python-executable|installer-repair-python-inventory|installer-repair-python-missing-inventory|installer-unsafe-python|installer-lock-lifecycle|git-path-spaces|relative-git-path|cargo-local-lock-policy|installer-seed-lock-artifact) ;;
  *) printf 'generated-workflows: unsupported real-toolchain focus %s\n' "$e2e_focus" >&2; exit 2 ;;
esac
case "$cold_install" in
  0|1) ;;
  *) printf 'generated-workflows: BASELINE_GUARD_COLD_INSTALL must be 0 or 1\n' >&2; exit 2 ;;
esac
case "$public_mutant" in
  ''|relative-git-path|whole-lock-digest|external-record|unknown-field|edge-resolution|path-policy|feature-coupling|typed-toml-equality) ;;
  *) printf 'generated-workflows: unsupported public mutant %s\n' "$public_mutant" >&2; exit 2 ;;
esac
case "$policy_case" in
  all|external-checksum|external-source|external-version|external-dependency-graph|unknown-local-package|forged-source-less-entry|duplicate-package-identity|ambiguous-unqualified-edge|duplicate-edge|unknown-lock-metadata|unknown-package-metadata|missing-external-record|malformed-lock|unresolved-local-edge|root-lock-symlink|nested-lock-symlink|nested-lock-changed|path-escape|feature-only-removal|feature-unrelated-token|feature-new-key|feature-similar-key|feature-edge-retained|feature-lock-only-removal|typed-workspace-boolean-int|typed-dependency-boolean-int|typed-protected-boolean-int|lock-version-float|positive-external-edge|positive-local-local-removal|positive-lock-formatting|positive-pair) ;;
  *) printf 'generated-workflows: unsupported Cargo lock policy case %s\n' "$policy_case" >&2; exit 2 ;;
esac
if [[ "$policy_case" == positive-pair && "$e2e_focus" != cargo-local-lock-policy ]]; then
  printf 'generated-workflows: positive-pair requires the Cargo local lock policy focus\n' >&2
  exit 2
fi
case "$policy_preflight_only" in
  0|1) ;;
  *) printf 'generated-workflows: BASELINE_GUARD_POLICY_PREFLIGHT_ONLY must be 0 or 1\n' >&2; exit 2 ;;
esac
if [[ "$policy_preflight_only" == 1 ]]; then
  if [[ "$e2e_focus" != cargo-local-lock-policy || ( "$policy_case" != positive-external-edge && "$policy_case" != positive-local-local-removal ) || -n "$public_mutant" || -n "$installer_mutant" || -z "$policy_control_export" || "$policy_control_export" != /* || -e "$policy_control_export" || -L "$policy_control_export" ]]; then
    printf 'generated-workflows: policy preflight requires the selected positive fixture and a new absolute export path\n' >&2
    exit 2
  fi
  policy_export_parent=$(cd "$(dirname "$policy_control_export")" && pwd -P) || {
    printf 'generated-workflows: policy control export parent must already exist\n' >&2
    exit 2
  }
  policy_export_resolved="$policy_export_parent/$(basename "$policy_control_export")"
  case "$policy_export_resolved" in
    "$repo_root"|"$repo_root"/*)
      printf 'generated-workflows: policy control export cannot be inside the original source checkout\n' >&2
      exit 2
      ;;
  esac
elif [[ -n "$policy_control_export" ]]; then
  printf 'generated-workflows: BASELINE_GUARD_POLICY_CONTROL_EXPORT requires policy preflight mode\n' >&2
  exit 2
fi
case "$installer_mutant" in
  ''|raw-path-recognition|marker-path-recognition|new-generation-rendering|mise-validation-only|mise-wrong-hash-publish-stop|python-validation-only|python-integrity-bypass|old-runtime-inventory-bypass|pre-push-publication-validation-only|no-child-lock-retention|pre-push-publication-failure|pre-push-publication-failure-accepted-mode|pre-push-publication-failure-with-late-update|late-precommit-replacement|late-prepush-replacement) ;;
  *) printf 'generated-workflows: unsupported installer source fault %s\n' "$installer_mutant" >&2; exit 2 ;;
esac
if [[ -n "$public_mutant" && ( "$e2e_focus" != relative-git-path && "$e2e_focus" != cargo-local-lock-policy || "$cold_install" != 0 ) ]]; then
  printf 'generated-workflows: the public source mutant requires its matching warm behavior run\n' >&2
  exit 2
fi
case "$public_mutant:$e2e_focus" in
  :*|relative-git-path:relative-git-path|whole-lock-digest:cargo-local-lock-policy|external-record:cargo-local-lock-policy|unknown-field:cargo-local-lock-policy|edge-resolution:cargo-local-lock-policy|path-policy:cargo-local-lock-policy|feature-coupling:cargo-local-lock-policy|typed-toml-equality:cargo-local-lock-policy) ;;
  *) printf 'generated-workflows: public source mutant does not match its behavior case\n' >&2; exit 2 ;;
esac
case "$public_mutant" in
  whole-lock-digest) expected_policy_case=positive-external-edge ;;
  external-record) expected_policy_case=external-checksum ;;
  unknown-field) expected_policy_case=unknown-lock-metadata ;;
  edge-resolution) expected_policy_case=positive-external-edge ;;
  path-policy) expected_policy_case=path-escape ;;
  feature-coupling) expected_policy_case=feature-only-removal ;;
  typed-toml-equality) expected_policy_case=typed-workspace-boolean-int ;;
  *) expected_policy_case= ;;
esac
if [[ -n "$expected_policy_case" ]]; then
  if [[ "$policy_case" != all && "$policy_case" != "$expected_policy_case" ]]; then
    if [[ "$public_mutant" != whole-lock-digest || "$policy_case" != positive-lock-formatting ]]; then
      printf 'generated-workflows: public source mutant requires its matching policy case\n' >&2
      exit 2
    fi
  fi
  if [[ "$public_mutant" != whole-lock-digest || "$policy_case" != positive-lock-formatting ]]; then
    policy_case=$expected_policy_case
  fi
fi
if [[ -n "$installer_mutant" && "$cold_install" != 0 ]]; then
  printf 'generated-workflows: installer source faults require a warm artifact fixture\n' >&2
  exit 2
fi
case "$installer_mutant:$e2e_focus" in
  :*|raw-path-recognition:installer-managed-hook-upgrade|marker-path-recognition:installer-forged-hook|marker-path-recognition:installer-managed-hook-upgrade|new-generation-rendering:installer-managed-hook-upgrade|mise-validation-only:installer-repair-mise|mise-wrong-hash-publish-stop:installer-repair-mise-wrong-hash|python-validation-only:installer-repair-python-executable|python-validation-only:installer-repair-python-inventory|python-validation-only:installer-repair-python-missing-inventory|python-integrity-bypass:installer-repair-python-inventory|old-runtime-inventory-bypass:installer-managed-hook-upgrade|pre-push-publication-validation-only:installer-lock-lifecycle|no-child-lock-retention:installer-lock-lifecycle|pre-push-publication-failure:installer-managed-hook-upgrade|pre-push-publication-failure-accepted-mode:installer-managed-hook-upgrade|pre-push-publication-failure-with-late-update:installer-managed-hook-upgrade|late-precommit-replacement:installer-managed-hook-upgrade|late-prepush-replacement:installer-managed-hook-upgrade) ;;
  *) printf 'generated-workflows: installer source fault does not match its public behavior case\n' >&2; exit 2 ;;
esac
git_command=$(command -v git)
if [[ "$git_command" != /* ]]; then
  git_command="$(pwd -P)/$git_command"
fi
source "$repo_root/tests/policy-git-object-lifecycle.sh"
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64) hash_tool=/usr/bin/shasum; platform=macos-arm64 ;;
  Linux:x86_64|Linux:amd64) hash_tool=/usr/bin/sha256sum; platform=linux-x64 ;;
  *) printf 'generated-workflows: unsupported host for the real-toolchain qualification\n' >&2; exit 2 ;;
esac
sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | /usr/bin/awk '{print $1}'
  else
    "$hash_tool" "$1" | /usr/bin/awk '{print $1}'
  fi
}
sha256_stdin() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 | /usr/bin/awk '{print $1}'
  else
    "$hash_tool" | /usr/bin/awk '{print $1}'
  fi
}
policy_verified_velnor_executable() {
  python3 -I -S - "$installation/cache/tools" "$checker_digest" "$platform" "$installed_manifest" <<'PY'
import hashlib
import os
import re
import stat
import sys
from pathlib import Path

cache_root = Path(sys.argv[1])
checker_sha, platform, installed_manifest = sys.argv[2:]
def fail(error):
    raise error
def read_fields(path):
    result = {}
    for line in path.read_text().splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in result:
            raise SystemExit("generated-workflows: authenticated tool cache manifest is malformed")
        result[key] = value
    return result

installed = read_fields(Path(installed_manifest))
policy_sha = installed.get("trusted_seed_cargo_policy_sha256", "")
if not re.fullmatch(r"[0-9a-f]{64}", policy_sha):
    raise SystemExit("generated-workflows: installed checker policy identity is missing or invalid")
generations = []
if cache_root.is_dir() and not cache_root.is_symlink():
    for candidate in cache_root.iterdir():
        manifest = candidate / "manifest"
        if candidate.is_dir() and not candidate.is_symlink() and manifest.is_file() and not manifest.is_symlink():
            candidate_fields = read_fields(manifest)
            if candidate_fields.get("checker_sha256") == checker_sha and candidate_fields.get("platform") == platform and candidate_fields.get("velnor") == "0.1.78" and candidate_fields.get("seed_commit") == installed.get("seed_commit") and candidate_fields.get("trusted_lock_sha256") == installed.get("trusted_seed_cargo_lock_sha256") and candidate_fields.get("trusted_seed_cargo_policy_sha256") == policy_sha and candidate_fields.get("config_sha256") == installed.get("config_sha256") and candidate_fields.get("inventory_helper_sha256") == installed.get("inventory_helper_sha256"):
                generations.append((candidate, candidate_fields))
if len(generations) != 1:
    raise SystemExit("generated-workflows: installed checker has no unique authenticated Velnor 0.1.78 tool cache generation")
generation, fields = generations[0]
names = (
    "platform", "velnor", "rust", "jactionlint", "shellcheck", "seed_commit",
    "trusted_lock_sha256", "mise_version", "mise_sha256", "python_version",
    "python_artifact_sha256", "python_lock_sha256", "python_mise_sha256",
    "config_sha256", "inventory_helper_sha256", "trusted_seed_cargo_policy_sha256",
)
if fields.get("checker_version") != "2" or fields.get("platform") != platform or fields.get("velnor") != "0.1.78":
    raise SystemExit("generated-workflows: authenticated tool cache does not bind the reviewed Velnor 0.1.78 pin")
try:
    identity = [checker_sha, "2"] + [fields[name] for name in names]
except KeyError as error:
    raise SystemExit("generated-workflows: authenticated tool cache identity is incomplete") from error
expected_key = hashlib.sha256("\n".join(identity).encode("ascii")).hexdigest()
if fields.get("key") != expected_key or generation.name != expected_key:
    raise SystemExit("generated-workflows: authenticated tool cache key does not bind its reviewed manifest")
tool_state = generation / "tool-state"
if tool_state.is_symlink() or not tool_state.is_dir():
    raise SystemExit("generated-workflows: authenticated tool cache payload is missing or unsafe")
for directory in (cache_root, generation, tool_state):
    info = directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise SystemExit("generated-workflows: authenticated tool cache directory chain is unsafe")
manifest_path = generation / "manifest"
manifest_info = manifest_path.lstat()
if not stat.S_ISREG(manifest_info.st_mode) or manifest_info.st_uid != os.getuid() or manifest_info.st_mode & 0o222:
    raise SystemExit("generated-workflows: authenticated tool cache manifest is unsafe")
for cache_name, install_name in (
    ("seed_commit", "seed_commit"),
    ("trusted_lock_sha256", "trusted_seed_cargo_lock_sha256"),
    ("trusted_seed_cargo_policy_sha256", "trusted_seed_cargo_policy_sha256"),
    ("config_sha256", "config_sha256"),
    ("inventory_helper_sha256", "inventory_helper_sha256"),
    ("mise_version", "mise_version"),
    ("mise_sha256", "mise_sha256"),
    ("python_version", "python_version"),
    ("python_artifact_sha256", "python_artifact_sha256"),
    ("python_lock_sha256", "python_lock_sha256"),
    ("python_mise_sha256", "python_mise_sha256"),
):
    if fields.get(cache_name) != installed.get(install_name):
        raise SystemExit("generated-workflows: authenticated tool cache differs from the installed checker pins")
inventory = hashlib.sha256()
entries = []
for directory, names, filenames in os.walk(tool_state, followlinks=False, onerror=fail):
    current = Path(directory)
    for name in names + filenames:
        entries.append(current / name)
for path in sorted(entries, key=lambda item: os.fsencode(item.relative_to(tool_state))):
    relative = os.fsencode(path.relative_to(tool_state))
    info = path.lstat()
    if info.st_uid != os.getuid() or stat.S_ISREG(info.st_mode) and info.st_mode & 0o222 or stat.S_ISDIR(info.st_mode) and info.st_mode & 0o222:
        raise SystemExit("generated-workflows: authenticated tool cache payload has unsafe ownership or write permissions")
    mode = stat.S_IMODE(info.st_mode)
    if stat.S_ISLNK(info.st_mode):
        kind = b"link"
        content = os.fsencode(os.readlink(path))
        target = path.resolve(strict=True)
        if not target.is_relative_to(tool_state.resolve(strict=True)):
            raise SystemExit("generated-workflows: authenticated tool cache has an escaping symlink")
    elif stat.S_ISREG(info.st_mode):
        kind = b"file"
        content = path.read_bytes()
    elif stat.S_ISDIR(info.st_mode):
        kind = b"directory"
        content = b""
    else:
        raise SystemExit("generated-workflows: authenticated tool cache contains a special file")
    for field in (relative, kind, str(mode).encode("ascii"), content):
        inventory.update(len(field).to_bytes(8, "big"))
        inventory.update(field)
if fields.get("payload_sha256") != inventory.hexdigest():
    raise SystemExit("generated-workflows: authenticated tool cache payload changed after hook qualification")
executable = tool_state / "mise-data/installs/github-tailrocks-velnor-new" / fields["velnor"] / "velnor-actions"
try:
    resolved_executable = executable.resolve(strict=True)
except OSError as error:
    raise SystemExit("generated-workflows: verified Velnor executable is unavailable") from error
if not resolved_executable.is_relative_to(tool_state.resolve(strict=True)):
    raise SystemExit("generated-workflows: verified Velnor executable escaped the authenticated cache")
info = resolved_executable.lstat()
if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o222 or info.st_mode & 0o111 == 0:
    raise SystemExit("generated-workflows: verified Velnor executable is missing or unsafe")
print(executable)
PY
}
if [[ "$e2e_focus" == cache-validation-only ]]; then
  installation=${BASELINE_GUARD_CACHE_VALIDATION_ROOT:-}
  if [[ "$installation" != /* || ! -d "$installation" || -L "$installation" || "$policy_case" != all || "$cold_install" != 0 || -n "$public_mutant" || -n "$installer_mutant" || "$policy_preflight_only" != 0 ]]; then
    printf 'generated-workflows: cache validation requires a private absolute installation root\n' >&2
    exit 2
  fi
  checker_digest=$(sha256_file "$repo_root/scripts/check-generated-workflows.sh") || exit 1
  installed_manifest="$installation/versions/$checker_digest/manifest"
  if [[ ! -f "$installed_manifest" || -L "$installed_manifest" ]]; then
    printf 'generated-workflows: cache validation installed manifest is missing or unsafe\n' >&2
    exit 1
  fi
  policy_verified_velnor_executable
  exit $?
elif [[ -n "${BASELINE_GUARD_CACHE_VALIDATION_ROOT:-}" ]]; then
  printf 'generated-workflows: cache validation root requires cache-validation-only focus\n' >&2
  exit 2
fi
copy_reviewed_old_input() {
  local input=$1
  local destination=$2
  local mode
  mode=$("$git_command" -C "$upgrade_source_repository" ls-tree "$upgrade_old_head" -- "scripts/$input" | /usr/bin/awk '{print $1}')
  case "$mode" in
    100644) mode=644 ;;
    100755) mode=755 ;;
    *) printf 'generated-workflows: reviewed old input has an unsupported Git mode: %s\n' "$input" >&2; return 1 ;;
  esac
  "$git_command" -C "$upgrade_source_repository" show "$upgrade_old_head:scripts/$input" > "$destination"
  chmod "$mode" "$destination"
}
directory_digest() {
  python3 -I -S - "$1" <<'PY'
import hashlib
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
entries = []
if root.exists() or root.is_symlink():
    def visit(path):
        info = path.lstat()
        relative = "." if path == root else path.relative_to(root).as_posix()
        mode = stat.S_IMODE(info.st_mode)
        if stat.S_ISLNK(info.st_mode):
            entries.append(f"{relative}\0link\0{mode:o}\0{os.readlink(path)}")
        elif stat.S_ISDIR(info.st_mode):
            entries.append(f"{relative}\0directory\0{mode:o}")
            for child in sorted(path.iterdir(), key=lambda item: item.name):
                visit(child)
        elif stat.S_ISREG(info.st_mode):
            entries.append(f"{relative}\0file\0{mode:o}\0{hashlib.sha256(path.read_bytes()).hexdigest()}")
        else:
            entries.append(f"{relative}\0special\0{mode:o}")
    visit(root)
print(hashlib.sha256("\n".join(entries).encode()).hexdigest())
PY
}
rekey_mutant_tool_cache() {
  local cache_root=$1
  local original_checker=$2
  local mutant_checker=$3
  local info_file="$test_root/mutant-cache-info"
  local source_generation
  local target_generation
  local target_key
  python3 -I -S - "$cache_root" "$original_checker" "$mutant_checker" > "$info_file" <<'PY' || return 1
import hashlib
import re
import sys
from pathlib import Path

cache_root = Path(sys.argv[1])
original_checker, mutant_checker = sys.argv[2:]
for generation in sorted(cache_root.iterdir()):
    manifest = generation / "manifest"
    if not generation.is_dir() or generation.is_symlink() or not manifest.is_file() or manifest.is_symlink():
        continue
    fields = {}
    for line in manifest.read_text().splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in fields:
            raise SystemExit("generated-workflows: warm cache manifest is malformed")
        fields[key] = value
    if fields.get("checker_sha256") != original_checker:
        continue
    if fields.get("checker_version") != "2" or not re.fullmatch(r"[0-9a-f]{64}", fields.get("trusted_seed_cargo_policy_sha256", "")):
        raise SystemExit("generated-workflows: warm cache version or policy digest is invalid")
    names = (
        "platform", "velnor", "rust", "jactionlint", "shellcheck", "seed_commit",
        "trusted_lock_sha256", "mise_version", "mise_sha256", "python_version",
        "python_artifact_sha256", "python_lock_sha256", "python_mise_sha256",
        "config_sha256", "inventory_helper_sha256", "trusted_seed_cargo_policy_sha256",
    )
    try:
        values = [mutant_checker, "2"] + [fields[name] for name in names]
    except KeyError as error:
        raise SystemExit("generated-workflows: warm cache manifest is incomplete") from error
    key = hashlib.sha256("\n".join(values).encode("ascii")).hexdigest()
    print(generation)
    print(key)
    raise SystemExit(0)
raise SystemExit("generated-workflows: no warm cache matches the reviewed checker")
PY
  source_generation=$(/usr/bin/sed -n '1p' "$info_file")
  target_key=$(/usr/bin/sed -n '2p' "$info_file")
  if [[ -z "$source_generation" || ! "$target_key" =~ ^[0-9a-f]{64}$ ]]; then
    printf 'generated-workflows: cannot select the warm cache for the public mutant\n' >&2
    return 1
  fi
  target_generation="$cache_root/$target_key"
  case "$(uname -s)" in
    Darwin) /bin/cp -cRp "$source_generation" "$target_generation" || return 1 ;;
    Linux) /bin/cp -a --reflink=auto "$source_generation" "$target_generation" || return 1 ;;
    *) printf 'generated-workflows: unsupported host for warm mutant cache setup\n' >&2; return 2 ;;
  esac
  chmod u+w "$target_generation/manifest" || return 1
  python3 -I -S - "$target_generation/manifest" "$target_key" "$mutant_checker" <<'PY' || return 1
import sys
from pathlib import Path

path = Path(sys.argv[1])
target_key, mutant_checker = sys.argv[2:]
fields = {}
for line in path.read_text().splitlines():
    key, separator, value = line.partition("=")
    if not separator or key in fields:
        raise SystemExit("generated-workflows: copied warm cache manifest is malformed")
    fields[key] = value
fields["key"] = target_key
fields["checker_sha256"] = mutant_checker
path.write_text("".join(f"{key}={value}\n" for key, value in fields.items()))
PY
  chmod 400 "$target_generation/manifest" || return 1
  tool_cache_generation=$target_generation
}
if [[ "$e2e_focus" == mutant-cache-validation-only ]]; then
  installation=${BASELINE_GUARD_MUTANT_CACHE_ROOT:-}
  mutant_checker=${BASELINE_GUARD_MUTANT_CHECKER:-}
  if [[ "$installation" != /* || ! -d "$installation" || -L "$installation" || "$mutant_checker" != /* || ! -f "$mutant_checker" || -L "$mutant_checker" || "$policy_case" != all || "$cold_install" != 0 || -n "$public_mutant" || -n "$installer_mutant" || "$policy_preflight_only" != 0 ]]; then
    printf 'generated-workflows: mutant cache validation requires private absolute inputs\n' >&2
    exit 2
  fi
  original_checker=$(sha256_file "$repo_root/scripts/check-generated-workflows.sh") || exit 1
  checker_digest=$(sha256_file "$mutant_checker") || exit 1
  installed_manifest="$installation/versions/$checker_digest/manifest"
  if [[ ! -f "$installed_manifest" || -L "$installed_manifest" ]]; then
    printf 'generated-workflows: mutant cache installed manifest is missing or unsafe\n' >&2
    exit 1
  fi
  test_root=$(mktemp -d "${TMPDIR:-/tmp}/mutant-cache-validation.XXXXXX") || exit 1
  trap 'rm -rf "$test_root"' EXIT
  if ! rekey_mutant_tool_cache "$installation/cache/tools" "$original_checker" "$checker_digest"; then
    exit 1
  fi
  policy_verified_velnor_executable
  exit $?
elif [[ -n "${BASELINE_GUARD_MUTANT_CACHE_ROOT:-}" || -n "${BASELINE_GUARD_MUTANT_CHECKER:-}" ]]; then
  printf 'generated-workflows: mutant cache inputs require mutant-cache-validation-only focus\n' >&2
  exit 2
fi
apply_installer_mutant() {
  python3 -I -S - "$repository/scripts/install-generated-workflow-guard.sh" "$installer_mutant" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
mutant = sys.argv[2]
source = path.read_text()
changes = {
    "raw-path-recognition": (
        '    if render_hook "$expected_mode" "$versions_dir/$generation" | /usr/bin/cmp -s - "$target"; then\n',
        '    if /usr/bin/grep -F -q "$install_root/versions/" "$target"; then\n',
    ),
    "marker-path-recognition": (
        '    if render_hook "$expected_mode" "$versions_dir/$generation" | /usr/bin/cmp -s - "$target"; then\n',
        "    if /usr/bin/grep -F -q 'guard_owner=jackin-generated-workflow-guard' \"$target\" && /usr/bin/grep -F -q \"$install_root/versions/\" \"$target\"; then\n",
    ),
    "new-generation-rendering": (
        '  render_hook "$mode" > "$temporary"\n',
        '  render_hook "$mode" "$versions_dir/$validated_pre_commit_generation" > "$temporary"\n',
    ),
    "mise-validation-only": (
        '    quarantine_file "$mise_artifact"\n',
        '    printf \'generated-workflows: pinned mise artifact failed integrity validation\\n\' >&2\n    exit 1\n',
    ),
    "mise-wrong-hash-publish-stop": (
        '  if [[ "$(sha256_file "$mise_download")" != "$mise_sha256" ]]; then\n    printf \'generated-workflows: official mise artifact checksum does not match the reviewed release identity\\n\' >&2\n    exit 1\n  fi\n',
        '  if [[ "$(sha256_file "$mise_download")" != "$mise_sha256" ]]; then\n    chmod 500 "$mise_download"\n    mv "$mise_download" "$mise_artifact"\n    mise_download=\n    printf \'generated-workflows: wrong-hash source fault published its payload before execution\\n\' >&2\n    exit 99\n  fi\n',
    ),
    "python-validation-only": (
        '    quarantine_directory "$python_artifact_dir"\n',
        '    printf \'generated-workflows: pinned Python generation failed integrity validation\\n\' >&2\n    exit 1\n',
    ),
    "pre-push-publication-validation-only": (
        '  validate_existing_hook "$hooks_path/pre-push" pre-push || return 1\n',
        '  :\n',
    ),
    "old-runtime-inventory-bypass": (
        '  env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LC_ALL=C LANG=C /bin/bash "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory" || return 1\n',
        '  :\n',
    ),
    "pre-push-publication-failure": (
        'if ! mv -f "$pre_push_temp" "$hooks_dir/pre-push"; then\n',
        'if ! false; then\n',
    ),
    "pre-push-publication-failure-accepted-mode": (
        'if ! mv -f "$pre_push_temp" "$hooks_dir/pre-push"; then\n',
        'if ! false; then\n',
    ),
    "pre-push-publication-failure-with-late-update": (
        'if ! mv -f "$pre_push_temp" "$hooks_dir/pre-push"; then\n',
        'printf "#!/bin/bash\\nforeign late pre-push\\n" > "$hooks_path/pre-push"\nchmod 700 "$hooks_path/pre-push"\nif ! false; then\n',
    ),
    "late-precommit-replacement": (
        "assert_target_state \"$hooks_dir/pre-commit\" \"$pre_commit_published_identity\" \"$pre_commit_published_hash\" || abort_publication 'pre-commit changed after publication'\nif ! validate_hook_pair || ! assert_expected_hook_states; then\n  abort_publication 'the hook pair changed after pre-commit publication'\n",
        "assert_target_state \"$hooks_dir/pre-commit\" \"$pre_commit_published_identity\" \"$pre_commit_published_hash\" || abort_publication 'pre-commit changed after publication'\nprintf '#!/bin/bash\\nforeign late pre-commit\\n' > \"$hooks_path/pre-commit\"\nchmod 700 \"$hooks_path/pre-commit\"\nif ! validate_hook_pair || ! assert_expected_hook_states; then\n  abort_publication 'the hook pair changed after pre-commit publication'\n",
    ),
    "late-prepush-replacement": (
        "assert_target_state \"$hooks_dir/pre-commit\" \"$pre_commit_published_identity\" \"$pre_commit_published_hash\" || abort_publication 'pre-commit changed after publication'\nif ! validate_hook_pair || ! assert_expected_hook_states; then\n  abort_publication 'the hook pair changed after pre-commit publication'\n",
        "assert_target_state \"$hooks_dir/pre-commit\" \"$pre_commit_published_identity\" \"$pre_commit_published_hash\" || abort_publication 'pre-commit changed after publication'\nprintf '#!/bin/bash\\nforeign late pre-push\\n' > \"$hooks_path/pre-push\"\nchmod 700 \"$hooks_path/pre-push\"\nif ! validate_hook_pair || ! assert_expected_hook_states; then\n  abort_publication 'the hook pair changed after pre-commit publication'\n",
    ),
    "no-child-lock-retention": (
        '  if ! env -i "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" install python > "$python_stage/install.log" 2>&1; then\n',
        '  if ! env -i 9>&- "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" install python > "$python_stage/install.log" 2>&1; then\n',
    ),
}
before, after = changes[mutant]
if source.count(before) != 1:
    raise SystemExit("generated-workflows: installer source fault did not match exactly one approved branch")
path.write_text(source.replace(before, after, 1))
PY
}
apply_python_integrity_bypass() {
  python3 -I -S - "$repository/scripts/check-generated-workflows.sh" "$repository/scripts/install-generated-workflow-guard.sh" <<'PY'
import sys
from pathlib import Path

checker_path, installer_path = map(Path, sys.argv[1:])
checker = checker_path.read_text()
changes = (
    (r''' || ! -f "$python_inventory" || -L "$python_inventory"''', ''),
    (r'''"$(sha256_file "$python_inventory")" != "$(/usr/bin/grep '^inventory_sha256=' "$install_manifest" | /usr/bin/cut -d= -f2-)"''', '"0" != "0"'),
    (r''' || ! /usr/bin/grep -F -x -q "inventory_sha256=$(sha256_file "$python_inventory")" "$python_artifact_manifest"''', ''),
    ('  "$checker_directory/python-inventory.sh" check "$artifact_root/python/3.15.2/$platform-$python_generation/runtime" "$python_inventory" || exit $?\n', '  :\n'),
)
for before, after in changes:
    expected = 1 if before != changes[1][0] else 2
    if checker.count(before) != expected:
        raise SystemExit("generated-workflows: Python integrity bypass did not match its exact reviewed branch")
    checker = checker.replace(before, after)
checker_path.write_text(checker)
digest = __import__("hashlib").sha256(checker.encode()).hexdigest()
installer = installer_path.read_text()
needle = next(line for line in installer.splitlines(keepends=True) if line.startswith("reviewed_checker_sha256="))
installer_path.write_text(installer.replace(needle, f"reviewed_checker_sha256={digest}\n", 1))
PY
}
apply_lock_policy_mutant() {
  python3 -I -S - "$repository/scripts/check-generated-workflows.sh" "$repository/scripts/install-generated-workflow-guard.sh" "$public_mutant" <<'PY'
import hashlib
import sys
from pathlib import Path

checker_path, installer_path = map(Path, sys.argv[1:3])
mutant = sys.argv[3]
checker = checker_path.read_text()
if mutant == "whole-lock-digest":
    before = '        root_lock_sha256 = validate_snapshot_lock(path, trusted_lock_path, permitted_deltas)\n'
    after = '        root_lock_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()\n        if root_lock_sha256 != trusted_lock_sha256:\n            reject("Cargo.lock is outside the reviewed dependency support policy")\n'
elif mutant == "external-record":
    before = '        elif candidate_package != trusted_package:\n            reject("Cargo.lock changes a reviewed external package record")\n'
    after = '        elif candidate_package != trusted_package:\n            pass\n'
elif mutant == "unknown-field":
    before = 'if type(lock) is not dict or set(lock) != {"version", "package"} or lock.get("version") != 4:'
    replacement = 'if type(lock) is not dict or not {"version", "package"}.issubset(lock) or lock.get("version") != 4:'
    if checker.count(before) != 1:
        raise SystemExit("generated-workflows: unknown-field mutant could not identify lock field policy")
    checker = checker.replace(before, replacement, 1)
    before = 'if candidate.keys() != trusted.keys() or candidate["version"] != trusted["version"]:'
    after = 'if candidate["version"] != trusted["version"]:'
elif mutant == "edge-resolution":
    before = '        for dependency in package.get("dependencies", []):\n            target = dependency_target(dependency, identities, label)\n'
    after = '        for dependency in package.get("dependencies", []):\n            raw_dependency = dependency.split(" ", 1)[0] if identity[2] is None else dependency\n            target = dependency_target(raw_dependency, identities, label)\n'
elif mutant == "path-policy":
    before = '    try:\n        resolved.relative_to(snapshot)\n    except ValueError:\n        reject(f"{label} escapes the snapshot")\n'
    after = '    try:\n        resolved.relative_to(snapshot)\n    except ValueError:\n        pass\n'
elif mutant == "feature-coupling":
    before = '            if selected_values != expected_values:\n                reject("Cargo manifests change feature values outside a removed local dependency reference")\n'
    after = '            if False:\n                reject("Cargo manifests change feature values outside a removed local dependency reference")\n'
elif mutant == "typed-toml-equality":
    before = "def toml_equal(left, right):\n    if type(left) is not type(right):\n        return False\n    if type(left) is dict:\n        return left.keys() == right.keys() and all(toml_equal(left[key], right[key]) for key in left)\n    if type(left) is list:\n        return len(left) == len(right) and all(toml_equal(left_value, right_value) for left_value, right_value in zip(left, right, strict=True))\n    return left == right\n"
    after = "def toml_equal(left, right):\n    return left == right\n"
else:
    raise SystemExit("generated-workflows: unsupported lock policy mutant")
if checker.count(before) != 1:
    raise SystemExit("generated-workflows: lock policy source fault did not match exactly one policy branch")
checker = checker.replace(before, after, 1)
checker_path.write_text(checker)
digest = hashlib.sha256(checker.encode()).hexdigest()
installer = installer_path.read_text()
needle = next(line for line in installer.splitlines(keepends=True) if line.startswith("reviewed_checker_sha256="))
installer_path.write_text(installer.replace(needle, f"reviewed_checker_sha256={digest}\n", 1))
PY
}
apply_policy_acquisition_sentinels() {
  python3 -I -S - "$repository/scripts/check-generated-workflows.sh" "$repository/scripts/install-generated-workflow-guard.sh" "$test_root/policy-acquisition-sentinels" <<'PY'
import hashlib
import shlex
import sys
from pathlib import Path

checker_path = Path(sys.argv[1])
installer_path = Path(sys.argv[2])
sentinel_root = Path(sys.argv[3])
source = checker_path.read_text()
armed = shlex.quote(str(sentinel_root.parent / "policy-acquisition-armed"))
probes = (
    ("http", '  downloaded_bytes=$(', "  "),
    ("rustup-http", '  env -i "${bootstrap_environment[@]}" "$python_command" -I -S - "$rust_host"', "  "),
    ("mise-lock", '      env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" lock', "      "),
    ("mise-install", '      env -i "${mise_environment[@]}" "$mise_command" --locked --cd "$config_dir" install', "      "),
    ("mise", '    tool_dir=$(env -i "${mise_environment[@]}" "$mise_command"', "    "),
    ("cargo-fetch", '    if ! (cd "$seed" && env -i "${cargo_environment[@]}" "$cargo_executable" fetch', "    "),
    ("cargo-metadata", '    if ! (cd "$root" && env -i "${metadata_environment[@]}" "$cargo_executable" metadata', "    "),
)
for name, anchor, indentation in probes:
    if source.count(anchor) != 1:
        raise SystemExit(f"generated-workflows: {name} acquisition sentinel anchor is ambiguous")
    marker = shlex.quote(str(sentinel_root / f"{name}.reached"))
    diagnostic = shlex.quote(f"generated-workflows: test acquisition sentinel reached: {name}")
    sentinel = (
        f"{indentation}if [[ -e {armed} ]]; then\n"
        f"{indentation}  printf '%s\\n' {shlex.quote(name)} > {marker}\n"
        f"{indentation}  printf '%s\\n' {diagnostic} >&2\n"
        f"{indentation}  exit 97\n"
        f"{indentation}fi\n"
    )
    source = source.replace(anchor, sentinel + anchor, 1)
checker_path.write_text(source)
digest = hashlib.sha256(source.encode()).hexdigest()
installer = installer_path.read_text()
needle = next(line for line in installer.splitlines(keepends=True) if line.startswith("reviewed_checker_sha256="))
installer_path.write_text(installer.replace(needle, f"reviewed_checker_sha256={digest}\n", 1))
PY
  mkdir -p "$test_root/policy-acquisition-sentinels"
}
source_common=$("$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
source_common=$(cd "$source_common" && pwd -P)
guard_install_root=${BASELINE_GUARD_INSTALL_ROOT:-$source_common/jackin-generated-workflow-guard}
upgrade_source_repository=${BASELINE_GUARD_UPGRADE_SOURCE_REPOSITORY:-}
upgrade_old_checker_sha256=9d2fb0526f89cd58257579876786efeaacc3f88119f2f8cb782b8b0873d5c1eb
upgrade_old_installer_sha256=669ef9abe5b24a2f16f6e21bd5e5074ef7f9dc3a70b395b015da0b0cf3816c6e
upgrade_old_head=d29d80ee1bf3da923a0ed98e3514a2e9a5289698
if [[ "$e2e_focus" == installer-managed-hook-upgrade ]]; then
  if [[ -z "$upgrade_source_repository" || ! -d "$upgrade_source_repository/.git" || -L "$upgrade_source_repository/.git" ]]; then
    printf 'generated-workflows: installer-managed-hook-upgrade requires BASELINE_GUARD_UPGRADE_SOURCE_REPOSITORY\n' >&2
    exit 2
  fi
  if [[ "$("$git_command" -C "$upgrade_source_repository" rev-parse HEAD)" != "$upgrade_old_head" || "$("$git_command" -C "$upgrade_source_repository" show "$upgrade_old_head:scripts/install-generated-workflow-guard.sh" | sha256_stdin)" != "$upgrade_old_installer_sha256" || "$("$git_command" -C "$upgrade_source_repository" show "$upgrade_old_head:scripts/check-generated-workflows.sh" | sha256_stdin)" != "$upgrade_old_checker_sha256" ]]; then
    printf 'generated-workflows: managed-hook upgrade source differs from the reviewed old installer fixture\n' >&2
    exit 2
  fi
  upgrade_source_common=$("$git_command" -C "$upgrade_source_repository" rev-parse --path-format=absolute --git-common-dir)
  upgrade_source_common=$(cd "$upgrade_source_common" && pwd -P)
  upgrade_old_install_root="$upgrade_source_common/jackin-generated-workflow-guard"
  if [[ -L "$upgrade_old_install_root" || ! -d "$upgrade_old_install_root" || -L "$upgrade_old_install_root/versions/$upgrade_old_checker_sha256" || ! -d "$upgrade_old_install_root/versions/$upgrade_old_checker_sha256" ]]; then
    printf 'generated-workflows: reviewed old generation fixture is unavailable or unsafe\n' >&2
    exit 2
  fi
fi
if [[ "$policy_preflight_only" != 1 && "$cold_install" -eq 0 && ( -L "$guard_install_root" || ! -d "$guard_install_root" ) ]]; then
  printf 'generated-workflows: run bash scripts/install-generated-workflow-guard.sh before this real-toolchain qualification\n' >&2
  exit 2
fi

test_root=$(mktemp -d "${TMPDIR:-/tmp}/generated-workflow-e2e.XXXXXX")
evidence_base=${BASELINE_GUARD_EVIDENCE_DIR:-}
network_enforcement=not-available
network_fixture_pid=
capture_optional_installer_diagnostics() {
  local evidence_run=$1
  (
    set +e
    local provenance_dir="$evidence_run/provenance"
    local reviewed_installer="$repo_root/scripts/install-generated-workflow-guard.sh"
    local copied_installer=
    local reviewed_checker="$repo_root/scripts/check-generated-workflows.sh"
    local copied_checker=
    local reviewed_hash=unavailable
    local copied_hash=unavailable
    local diff_status=unavailable
    local checker_diff_status=unavailable
    local current_uid
    local source
    local destination
    local owner
    local mode
    local links
    local size
    local quarantine_root=
    local quarantine_parent=
    local quarantine_leaf=
    local q_name=
    local metadata_name
    mkdir -p "$provenance_dir/python" "$provenance_dir/quarantine" || exit 1
    if [[ -n "${repository:-}" ]]; then
      copied_installer="$repository/scripts/install-generated-workflow-guard.sh"
      copied_checker="$repository/scripts/check-generated-workflows.sh"
    fi
    if [[ -f "$reviewed_installer" && ! -L "$reviewed_installer" ]]; then
      cp -p "$reviewed_installer" "$provenance_dir/reviewed-installer.sh" || true
    fi
    if [[ -n "$copied_installer" && -f "$copied_installer" && ! -L "$copied_installer" ]]; then
      cp -p "$copied_installer" "$provenance_dir/copied-installer.sh" || true
    fi
    if [[ -f "$provenance_dir/reviewed-installer.sh" && ! -L "$provenance_dir/reviewed-installer.sh" ]]; then
      reviewed_hash=$(sha256_file "$provenance_dir/reviewed-installer.sh" 2>/dev/null || printf unavailable)
    fi
    if [[ -f "$provenance_dir/copied-installer.sh" && ! -L "$provenance_dir/copied-installer.sh" ]]; then
      copied_hash=$(sha256_file "$provenance_dir/copied-installer.sh" 2>/dev/null || printf unavailable)
    fi
    if [[ -f "$provenance_dir/reviewed-installer.sh" && ! -L "$provenance_dir/reviewed-installer.sh" && -f "$provenance_dir/copied-installer.sh" && ! -L "$provenance_dir/copied-installer.sh" ]]; then
      if /usr/bin/diff -u "$provenance_dir/reviewed-installer.sh" "$provenance_dir/copied-installer.sh" > "$provenance_dir/copied-installer.diff"; then
        diff_status=0
      else
        diff_status=$?
        if [[ "$diff_status" -gt 1 ]]; then
          diff_status=error-$diff_status
        fi
      fi
    fi
    {
      printf 'focus=%s\n' "${e2e_focus:-unavailable}"
      printf 'installer_mutant=%s\n' "${installer_mutant:-none}"
      printf 'public_mutant=%s\n' "${public_mutant:-none}"
      printf 'reviewed_installer_path=%s\n' "$reviewed_installer"
      printf 'copied_installer_path=%s\n' "${copied_installer:-unavailable}"
      printf 'reviewed_installer_sha256=%s\n' "$reviewed_hash"
      printf 'copied_installer_sha256=%s\n' "$copied_hash"
      printf 'copied_source_diff_status=%s\n' "$diff_status"
    } > "$provenance_dir/installer-context.txt"
    current_uid=$(/usr/bin/id -u 2>/dev/null || printf unavailable)
    copy_private_small_file() {
      source=$1
      destination=$2
      metadata_name=$3
      if [[ -L "$source" || ! -f "$source" ]]; then
        printf '%s=unavailable-or-unsafe\n' "$metadata_name" >> "$provenance_dir/python-metadata-context.txt"
        return 0
      fi
      if [[ "$platform" == macos-arm64 ]]; then
        owner=$(/usr/bin/stat -f '%u' "$source" 2>/dev/null || printf unavailable)
        mode=$(/usr/bin/stat -f '%Lp' "$source" 2>/dev/null || printf unavailable)
        links=$(/usr/bin/stat -f '%l' "$source" 2>/dev/null || printf unavailable)
        size=$(/usr/bin/stat -f '%z' "$source" 2>/dev/null || printf unavailable)
      else
        owner=$(/usr/bin/stat -c '%u' "$source" 2>/dev/null || printf unavailable)
        mode=$(/usr/bin/stat -c '%a' "$source" 2>/dev/null || printf unavailable)
        links=$(/usr/bin/stat -c '%h' "$source" 2>/dev/null || printf unavailable)
        size=$(/usr/bin/stat -c '%s' "$source" 2>/dev/null || printf unavailable)
      fi
      if [[ "$owner" != "$current_uid" || "$links" != 1 || ! "$mode" =~ ^[0-7]+$ || ! "$size" =~ ^[0-9]+$ ]] || (( (8#$mode & 077) != 0 || size > 16777216 )); then
        printf '%s=skipped-unsafe-or-over-16MiB\n' "$metadata_name" >> "$provenance_dir/python-metadata-context.txt"
        return 0
      fi
      if cp -p "$source" "$destination"; then
        printf '%s=sha256:%s,mode:%s,size:%s\n' "$metadata_name" "$(sha256_file "$destination" 2>/dev/null || printf unavailable)" "$mode" "$size" >> "$provenance_dir/python-metadata-context.txt"
      else
        printf '%s=copy-failed\n' "$metadata_name" >> "$provenance_dir/python-metadata-context.txt"
      fi
    }
    copy_bounded_source() {
      source=$1
      destination=$2
      metadata_name=$3
      if [[ -L "$source" || ! -f "$source" ]]; then
        printf '%s=unavailable-or-unsafe\n' "$metadata_name" >> "$provenance_dir/checker-source-context.txt"
        return 0
      fi
      if [[ "$platform" == macos-arm64 ]]; then
        owner=$(/usr/bin/stat -f '%u' "$source" 2>/dev/null || printf unavailable)
        mode=$(/usr/bin/stat -f '%Lp' "$source" 2>/dev/null || printf unavailable)
        links=$(/usr/bin/stat -f '%l' "$source" 2>/dev/null || printf unavailable)
        size=$(/usr/bin/stat -f '%z' "$source" 2>/dev/null || printf unavailable)
      else
        owner=$(/usr/bin/stat -c '%u' "$source" 2>/dev/null || printf unavailable)
        mode=$(/usr/bin/stat -c '%a' "$source" 2>/dev/null || printf unavailable)
        links=$(/usr/bin/stat -c '%h' "$source" 2>/dev/null || printf unavailable)
        size=$(/usr/bin/stat -c '%s' "$source" 2>/dev/null || printf unavailable)
      fi
      if [[ "$owner" != "$current_uid" || "$links" != 1 || ! "$mode" =~ ^[0-7]+$ || ! "$size" =~ ^[0-9]+$ ]] || (( (8#$mode & 022) != 0 || size > 4194304 )); then
        printf '%s=skipped-unsafe-or-over-4MiB\n' "$metadata_name" >> "$provenance_dir/checker-source-context.txt"
        return 0
      fi
      if cp -p "$source" "$destination"; then
        printf '%s=sha256:%s,mode:%s,size:%s\n' "$metadata_name" "$(sha256_file "$destination" 2>/dev/null || printf unavailable)" "$mode" "$size" >> "$provenance_dir/checker-source-context.txt"
      else
        printf '%s=copy-failed\n' "$metadata_name" >> "$provenance_dir/checker-source-context.txt"
      fi
    }
    copy_bounded_source "$reviewed_checker" "$provenance_dir/reviewed-checker.sh" reviewed_checker
    if [[ -n "$copied_checker" ]]; then
      copy_bounded_source "$copied_checker" "$provenance_dir/copied-checker.sh" copied_checker
    else
      printf 'copied_checker=unavailable\n' >> "$provenance_dir/checker-source-context.txt"
    fi
    if [[ -f "$provenance_dir/reviewed-checker.sh" && ! -L "$provenance_dir/reviewed-checker.sh" && -f "$provenance_dir/copied-checker.sh" && ! -L "$provenance_dir/copied-checker.sh" ]]; then
      if /usr/bin/diff -u "$provenance_dir/reviewed-checker.sh" "$provenance_dir/copied-checker.sh" > "$provenance_dir/copied-checker.diff"; then
        checker_diff_status=0
      else
        checker_diff_status=$?
        if [[ "$checker_diff_status" -gt 1 ]]; then
          checker_diff_status=error-$checker_diff_status
        fi
      fi
    fi
    {
      printf 'reviewed_checker_path=%s\n' "$reviewed_checker"
      printf 'copied_checker_path=%s\n' "${copied_checker:-unavailable}"
      printf 'copied_checker_diff_status=%s\n' "$checker_diff_status"
      if [[ -f "$provenance_dir/reviewed-checker.sh" && ! -L "$provenance_dir/reviewed-checker.sh" ]]; then
        printf 'reviewed_checker_sha256=%s\n' "$(sha256_file "$provenance_dir/reviewed-checker.sh" 2>/dev/null || printf unavailable)"
      else
        printf 'reviewed_checker_sha256=unavailable\n'
      fi
      if [[ -f "$provenance_dir/copied-checker.sh" && ! -L "$provenance_dir/copied-checker.sh" ]]; then
        printf 'copied_checker_sha256=%s\n' "$(sha256_file "$provenance_dir/copied-checker.sh" 2>/dev/null || printf unavailable)"
      else
        printf 'copied_checker_sha256=unavailable\n'
      fi
    } >> "$provenance_dir/installer-context.txt"
    if [[ -n "${test_root:-}" ]]; then
      copy_private_small_file "$test_root/python-inventory.original" "$provenance_dir/python/original-inventory" original_inventory
    fi
    if [[ -n "${python_artifact_dir:-}" && -d "$python_artifact_dir" && ! -L "$python_artifact_dir" ]]; then
      copy_private_small_file "$python_artifact_dir/inventory" "$provenance_dir/python/rebuilt-inventory" rebuilt_inventory
    else
      printf 'rebuilt_inventory=unavailable-or-unsafe\n' >> "$provenance_dir/python-metadata-context.txt"
    fi
    if [[ -n "${python_artifact_dir:-}" ]]; then
      if [[ -n "${python_generation_parent:-}" ]]; then
        quarantine_parent=$python_generation_parent
      else
        quarantine_parent=$(dirname "$python_artifact_dir")
      fi
    fi
    if [[ -n "${quarantined_generation:-}" && -n "$quarantine_parent" && "$(basename "$quarantined_generation")" == generation ]]; then
      quarantine_root=$(dirname "$quarantined_generation")
      q_name=$(basename "$quarantine_root")
      if [[ "$(dirname "$quarantine_root")" == "$quarantine_parent" && "$q_name" == .quarantine.* && -d "$quarantine_root" && ! -L "$quarantine_root" && -d "$quarantined_generation" && ! -L "$quarantined_generation" ]]; then
        for metadata_name in manifest inventory mise.toml mise.lock; do
          copy_private_small_file "$quarantined_generation/$metadata_name" "$provenance_dir/quarantine/original-$metadata_name" "quarantine_$metadata_name"
        done
        if [[ "$platform" == macos-arm64 ]]; then
          mode=$(/usr/bin/stat -f '%Lp' "$quarantined_generation" 2>/dev/null || printf unavailable)
        else
          mode=$(/usr/bin/stat -c '%a' "$quarantined_generation" 2>/dev/null || printf unavailable)
        fi
        printf 'quarantined_generation=%s\nquarantined_generation_mode=%s\n' "$quarantined_generation" "$mode" >> "$provenance_dir/python-metadata-context.txt"
      else
        printf 'quarantined_generation=unavailable-or-unsafe\n' >> "$provenance_dir/python-metadata-context.txt"
      fi
    else
      printf 'quarantined_generation=unavailable\n' >> "$provenance_dir/python-metadata-context.txt"
    fi
  )
}
write_sorted_find_results() {
  local root=$1
  local raw_path=$2
  local sorted_path=$3
  shift 3
  if ! find "$root" "$@" -print > "$raw_path"; then
    rm -f "$raw_path"
    return 1
  fi
  if ! LC_ALL=C sort "$raw_path" > "$sorted_path"; then
    rm -f "$raw_path"
    return 1
  fi
  rm "$raw_path"
}
list_contains_exact_line() {
  local expected_line=$1
  local list_path=$2
  local grep_status
  if /usr/bin/grep -F -x -q "$expected_line" "$list_path"; then
    return 0
  else
    grep_status=$?
  fi
  if [[ "$grep_status" -eq 1 ]]; then
    return 1
  fi
  return "$grep_status"
}
record_completion_witness() {
  local witness=$1
  if [[ -z "$witness" || "$witness" == *$'\n'* ]]; then
    printf 'generated-workflows: invalid completion witness\n' >&2
    return 1
  fi
  printf '%s\n' "$witness" >> "$completion_witnesses_file"
}
verify_completion_witnesses() {
  local expected_case
  local expected_witness
  local witness_count
  local matching_count
  if [[ -z "${completion_witnesses_file:-}" || -L "$completion_witnesses_file" || ! -f "$completion_witnesses_file" ]]; then
    printf 'generated-workflows: completion witness file is unavailable\n' >&2
    return 1
  fi
  witness_count=$(/usr/bin/awk 'END { print NR + 0 }' "$completion_witnesses_file") || return 1
  if [[ "$e2e_focus" == cargo-local-lock-policy ]]; then
    if [[ "$policy_preflight_only" == 1 ]]; then
      expected_witness=policy-preflight
      if [[ "$witness_count" -ne 1 ]]; then
        printf 'generated-workflows: policy preflight has %s completion witnesses, expected exactly one\n' "$witness_count" >&2
        return 1
      fi
      matching_count=$(/usr/bin/awk -v expected="$expected_witness" '$0 == expected { count++ } END { print count + 0 }' "$completion_witnesses_file") || return 1
      if [[ "$matching_count" -ne 1 ]]; then
        printf 'generated-workflows: policy preflight has no unique completion witness\n' >&2
        return 1
      fi
      return 0
    fi
    if [[ "$policy_case" == all ]]; then
      if [[ "$witness_count" -ne 20 ]]; then
        printf 'generated-workflows: Cargo policy matrix completion witness count is %s, expected 20\n' "$witness_count" >&2
        return 1
      fi
      for expected_case in $negative_policy_cases positive-external-edge positive-local-local-removal; do
        expected_witness="policy:$expected_case"
        matching_count=$(/usr/bin/awk -v expected="$expected_witness" '$0 == expected { count++ } END { print count + 0 }' "$completion_witnesses_file") || return 1
        if [[ "$matching_count" -ne 1 ]]; then
          printf 'generated-workflows: Cargo policy case %s has %s completion witnesses, expected exactly one\n' "$expected_case" "$matching_count" >&2
          return 1
        fi
      done
      return 0
    fi
    if [[ "$policy_case" == positive-pair ]]; then
      if [[ "$witness_count" -ne 2 ]]; then
        printf 'generated-workflows: Cargo policy positive pair has %s completion witnesses, expected 2\n' "$witness_count" >&2
        return 1
      fi
      for expected_case in positive-local-local-removal positive-external-edge; do
        expected_witness="policy:$expected_case"
        matching_count=$(/usr/bin/awk -v expected="$expected_witness" '$0 == expected { count++ } END { print count + 0 }' "$completion_witnesses_file") || return 1
        if [[ "$matching_count" -ne 1 ]]; then
          printf 'generated-workflows: Cargo policy case %s has %s completion witnesses, expected exactly one\n' "$expected_case" "$matching_count" >&2
          return 1
        fi
      done
      return 0
    fi
    expected_witness="policy:$policy_case"
  else
    expected_witness="focus:$e2e_focus"
  fi
  if [[ "$witness_count" -ne 1 ]]; then
    printf 'generated-workflows: selected case has %s completion witnesses, expected exactly one\n' "$witness_count" >&2
    return 1
  fi
  matching_count=$(/usr/bin/awk -v expected="$expected_witness" '$0 == expected { count++ } END { print count + 0 }' "$completion_witnesses_file") || return 1
  if [[ "$matching_count" -ne 1 ]]; then
    printf 'generated-workflows: selected case %s has no unique completion witness\n' "$expected_witness" >&2
    return 1
  fi
}
mark_completion_complete() {
  if ! verify_completion_witnesses; then
    return 1
  fi
  completion_state=complete
}
capture_required_run_evidence() {
  local evidence_run=
  local name
  if ! mkdir -p "$evidence_base"; then
    printf 'generated-workflows: cannot create the evidence directory\n' >&2
    return 1
  fi
  if [[ -L "$evidence_base" || ! -d "$evidence_base" ]]; then
    printf 'generated-workflows: evidence directory is unsafe\n' >&2
    return 1
  fi
  evidence_run=$(mktemp -d "$evidence_base/qualification.XXXXXX") || return 1
  if ! capture_optional_installer_diagnostics "$evidence_run"; then
    printf 'generated-workflows: optional installer diagnostics are incomplete\n' >&2
  fi
  for name in "$test_root"/*.log "$test_root"/*.stdout; do
    if [[ -f "$name" && ! -L "$name" ]]; then
      cp -p "$name" "$evidence_run/" || return 1
    fi
  done
  if [[ -f "$test_root/network-deny.sb" ]]; then
    cp -p "$test_root/network-deny.sb" "$evidence_run/" || return 1
  fi
  if [[ -f "$test_root/network-control.result" ]]; then
    cp -p "$test_root/network-control.result" "$evidence_run/" || return 1
  fi
  if [[ -f "$completion_witnesses_file" && ! -L "$completion_witnesses_file" ]]; then
    cp -p "$completion_witnesses_file" "$evidence_run/completion-witnesses.log" || return 1
  fi
  for name in policy-fixture-context.txt policy-source.patch policy-source-untracked.nul policy-source-untracked.original.json policy-source-status.original policy-source-refs.original policy-source-snapshot.original.json policy-source-snapshot.current.json policy-candidate-control-paths policy-external-changed-paths.nul policy-external-generated-paths.nul policy-external-generated-files.json policy-external-candidate-paths.expected.nul policy-local-changed-paths.nul policy-local-generated-paths.nul policy-local-generated-files.json policy-local-candidate-paths.expected.nul policy-local-generator-state.log policy-formatting-changed-paths.nul policy-formatting-generated-paths.nul policy-formatting-generated-files.json policy-formatting-candidate-paths.expected.nul policy-formatting-lock.candidate policy-formatting-semantic-proof.json policy-formatting-generator-state.log policy-formatting-generator-version.stderr policy-formatting-generator-first.log policy-formatting-generator-second.log positive-lock-formatting-candidate-paths policy-positive-lock-formatting-state.log positive-lock-formatting-commit.log positive-lock-formatting-push.log positive-lock-formatting-mutant-fixture-commit.log positive-lock-formatting-mutant-push.log positive-local-local-removal-candidate-paths policy-local-positive-paths positive-external-edge-candidate-paths policy-positive-external-edge.patch policy-external-app-manifest.patch policy-mutant.patch policy-mutant-checker.original policy-mutant-checker.mutant policy-mutant-whole-lock-digest.patch policy-mutant-edge-resolution.patch policy-mutant-checker-whole-lock-digest policy-mutant-checker-edge-resolution; do
    if [[ -f "$test_root/$name" && ! -L "$test_root/$name" ]]; then
      cp -p "$test_root/$name" "$evidence_run/" || return 1
    fi
  done
  if [[ -d "$test_root/policy-candidate-files" && ! -L "$test_root/policy-candidate-files" ]]; then
    mkdir "$evidence_run/policy-candidate-files" || return 1
    for name in root-Cargo.toml.candidate app-Cargo.toml.candidate Cargo.lock.candidate; do
      cp -p "$test_root/policy-candidate-files/$name" "$evidence_run/policy-candidate-files/" || return 1
    done
  fi
  mkdir "$evidence_run/cache" || return 1
  if [[ -n "${installation:-}" && -n "${checker_digest:-}" && -f "$installation/versions/$checker_digest/manifest" ]]; then
    cp -p "$installation/versions/$checker_digest/manifest" "$evidence_run/cache/installed-checker.manifest" || return 1
  fi
  if [[ -n "${installation:-}" && -f "$installation/cache/seed/f4d9999ace7d6deb4294cd89c00afcc80e748d49/manifest" ]]; then
    cp -p "$installation/cache/seed/f4d9999ace7d6deb4294cd89c00afcc80e748d49/manifest" "$evidence_run/cache/seed.manifest" || return 1
    cp -p "$installation/cache/seed/f4d9999ace7d6deb4294cd89c00afcc80e748d49/objects.txt" "$evidence_run/cache/seed.objects" || return 1
  fi
  if [[ -z "${tool_cache_generation:-}" && -n "${installation:-}" && -d "$installation/cache/tools" ]]; then
    tool_cache_generation=$(find "$installation/cache/tools" -mindepth 1 -maxdepth 1 -type d ! -name '.stage*' -print -quit) || return 1
  fi
  if [[ -n "${tool_cache_generation:-}" && -f "$tool_cache_generation/manifest" ]]; then
    cp -p "$tool_cache_generation/manifest" "$evidence_run/cache/tools.manifest" || return 1
    cp -p "$tool_cache_generation/tool-state/mise-config/mise.toml" "$evidence_run/cache/tools-mise.toml" || return 1
    cp -p "$tool_cache_generation/tool-state/mise-config/mise.lock" "$evidence_run/cache/tools-mise.lock" || return 1
  fi
  if [[ -n "${python_artifact_dir:-}" && -f "$python_artifact_dir/manifest" ]]; then
    cp -p "$python_artifact_dir/manifest" "$evidence_run/cache/python.manifest" || return 1
    cp -p "$python_artifact_dir/mise.toml" "$evidence_run/cache/python-mise.toml" || return 1
    cp -p "$python_artifact_dir/mise.lock" "$evidence_run/cache/python-mise.lock" || return 1
    if [[ -f "$python_artifact_dir/inventory" && ! -L "$python_artifact_dir/inventory" ]]; then
      cp -p "$python_artifact_dir/inventory" "$evidence_run/cache/python.inventory" || return 1
      printf 'inventory=present\n' > "$evidence_run/cache/python-inventory-state.txt" || return 1
    elif [[ ! -e "$python_artifact_dir/inventory" && ! -L "$python_artifact_dir/inventory" ]]; then
      printf 'inventory=missing\n' > "$evidence_run/cache/python-inventory-state.txt" || return 1
    else
      printf 'inventory=unsafe-or-unsupported\n' > "$evidence_run/cache/python-inventory-state.txt" || return 1
    fi
  fi
  local evidence_source_head=unavailable
  local evidence_seed_state=unavailable
  if [[ -n "${repository:-}" && -d "$repository" ]]; then
    evidence_source_head=$("$git_command" -C "$repository" rev-parse HEAD 2>/dev/null || true)
    if "$git_command" -C "$repository" cat-file -e 'f4d9999ace7d6deb4294cd89c00afcc80e748d49^{commit}' 2>/dev/null; then
      evidence_seed_state=present
    else
      evidence_seed_state=absent
    fi
  fi
  {
    printf 'source_head=%s\n' "$evidence_source_head"
    printf 'approved_seed_in_shallow_source=%s\n' "$evidence_seed_state"
    printf 'checker_sha256=%s\n' "${checker_digest:-unavailable}"
    printf 'platform=%s\n' "${platform:-unavailable}"
    printf 'network_enforcement=%s\n' "$network_enforcement"
  } > "$evidence_run/qualification.txt" || return 1
  printf 'generated-workflows: preserved real-toolchain evidence at %s\n' "$evidence_run"
}
cleanup_e2e() {
  local original_status=$?
  local final_status=$original_status
  local cleanup_status=0
  local network_pid
  local wait_status
  set +e
  if [[ "$original_status" -eq 0 ]]; then
    if [[ "${completion_state:-incomplete}" != complete ]]; then
      printf 'generated-workflows: harness exited successfully without completing its selected case\n' >&2
      final_status=1
    elif ! verify_completion_witnesses; then
      final_status=1
    fi
  fi
  if [[ -n "$network_fixture_pid" ]]; then
    network_pid=$network_fixture_pid
    if kill -0 "$network_pid" 2>/dev/null; then
      if ! kill "$network_pid" 2>/dev/null && kill -0 "$network_pid" 2>/dev/null; then
        printf 'generated-workflows: cannot stop the owned network fixture during cleanup\n' >&2
        cleanup_status=1
      fi
    fi
    if wait "$network_pid" 2>/dev/null; then
      wait_status=0
    else
      wait_status=$?
    fi
    network_fixture_pid=
    if [[ "$wait_status" -ne 0 && "$wait_status" -ne 143 ]]; then
      printf 'generated-workflows: owned network fixture exited unexpectedly during cleanup\n' >&2
      cleanup_status=1
    fi
    if kill -0 "$network_pid" 2>/dev/null; then
      printf 'generated-workflows: owned network fixture remains after cleanup\n' >&2
      cleanup_status=1
    fi
  fi
  if [[ -n "$evidence_base" ]]; then
    if ! capture_required_run_evidence; then
      printf 'generated-workflows: required qualification evidence could not be preserved\n' >&2
      cleanup_status=1
    fi
  fi
  if ! find "$test_root" -type d -exec chmod u+rwx {} +; then
    printf 'generated-workflows: cannot restore owned temporary directory permissions for cleanup\n' >&2
    cleanup_status=1
  fi
  if ! find "$test_root" -depth -delete; then
    printf 'generated-workflows: cannot remove the owned temporary test tree\n' >&2
    cleanup_status=1
  fi
  if [[ -e "$test_root" || -L "$test_root" ]]; then
    printf 'generated-workflows: temporary test root remains after cleanup\n' >&2
    cleanup_status=1
  fi
  if [[ "$original_status" -eq 0 && "$cleanup_status" -ne 0 ]]; then
    final_status=1
  fi
  exit "$final_status"
}
completion_state=incomplete
completion_witnesses_file="$test_root/completion-witnesses.log"
trap cleanup_e2e EXIT
: > "$completion_witnesses_file"
policy_source_repository=
policy_candidate_head=
policy_candidate_parent=
policy_candidate_tree=
policy_control_head=
policy_control_tree=
policy_external_fixture_head=
policy_external_fixture_parent=
policy_external_fixture_tree=
policy_external_output_sha=
policy_external_generated_paths="$test_root/policy-external-generated-paths.nul"
policy_external_generated_manifest="$test_root/policy-external-generated-files.json"
policy_external_fixture_patch="$test_root/policy-positive-external-edge.patch"
policy_local_generated_paths="$test_root/policy-local-generated-paths.nul"
policy_local_generated_manifest="$test_root/policy-local-generated-files.json"
policy_local_output_sha=
policy_formatting_generated_paths="$test_root/policy-formatting-generated-paths.nul"
policy_formatting_generated_manifest="$test_root/policy-formatting-generated-files.json"
policy_formatting_output_sha=
policy_formatting_candidate_paths="$test_root/policy-formatting-candidate-paths.expected.nul"
policy_local_fixture_head=
policy_local_fixture_tree=
policy_candidate_files="$test_root/policy-candidate-files"
policy_source_status_sha=
policy_source_index_tree=
policy_source_refs_sha=
policy_source_untracked_manifest_sha=
policy_source_snapshot_sha=
policy_source_branch=
policy_source_head=
policy_historical_head=
policy_seed_sha=
policy_source_snapshot_kind=
write_policy_source_untracked_manifest() {
  local root=$1
  local inventory=$2
  local manifest=$3
  if ! "$git_command" -C "$root" ls-files --others --exclude-standard -z > "$inventory"; then
    printf 'generated-workflows: cannot enumerate all untracked source inputs\n' >&2
    return 1
  fi
  python3 -I -S - "$root" "$inventory" "$manifest" <<'PY'
import hashlib
import json
import os
import stat
import sys
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
inventory = Path(sys.argv[2]).read_bytes().split(b"\0")
entries = []
seen = set()
for encoded in inventory:
    if not encoded:
        continue
    try:
        value = encoded.decode("utf-8")
    except UnicodeError as error:
        raise SystemExit("generated-workflows: untracked source path is not UTF-8") from error
    relative = PurePosixPath(value)
    if relative.is_absolute() or not relative.parts or ".." in relative.parts or ".git" in relative.parts or value in seen:
        raise SystemExit("generated-workflows: untracked source path is unsafe or duplicated")
    seen.add(value)
    source_path = root.joinpath(*relative.parts)
    parent = root
    for part in relative.parts[:-1]:
        parent = parent / part
        info = parent.lstat()
        if not stat.S_ISDIR(info.st_mode):
            raise SystemExit("generated-workflows: untracked source path traverses a non-directory or symlink")
    info = source_path.lstat()
    mode = stat.S_IMODE(info.st_mode)
    if stat.S_ISREG(info.st_mode):
        data = source_path.read_bytes()
        entries.append([value, "file", mode, len(data), hashlib.sha256(data).hexdigest()])
    elif stat.S_ISLNK(info.st_mode):
        entries.append([value, "symlink", mode, os.readlink(source_path)])
    else:
        raise SystemExit("generated-workflows: untracked source input is a special file")
Path(sys.argv[3]).write_text(json.dumps(entries, ensure_ascii=False, separators=(",", ":")) + "\n")
PY
}
prepare_local_local_removal_fixture() {
  python3 -I -S - "$1" <<'PY'
import copy
import re
import tomllib
from pathlib import Path

root = Path(__import__("sys").argv[1])
manifest = root / "apps/jackin/Cargo.toml"
lock_path = root / "Cargo.lock"
manifest_text = manifest.read_text()
lock_text = lock_path.read_text()
dependency_line = 'junie-tui = { path = "../..", default-features = false, features = ["terminal"] }\n'
feature_line = 'demo = ["dep:clap", "junie-tui/cli"]\n'
if manifest_text.count(dependency_line) != 1 or manifest_text.count(feature_line) != 1:
    raise SystemExit("generated-workflows: local-local removal dependency or feature anchor is ambiguous")
try:
    manifest_before = tomllib.loads(manifest_text)
    lock_before = tomllib.loads(lock_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit("generated-workflows: local-local removal candidate is invalid TOML") from error
dependencies_before = manifest_before.get("dependencies", {})
features_before = manifest_before.get("features", {})
if dependencies_before.get("junie-tui") != {"path": "../..", "default-features": False, "features": ["terminal"]} or features_before.get("demo") != ["dep:clap", "junie-tui/cli"]:
    raise SystemExit("generated-workflows: local-local removal dependency or feature semantics drifted")
packages_before = lock_before.get("package")
if type(packages_before) is not list:
    raise SystemExit("generated-workflows: local-local removal candidate lock has no package inventory")
owners = [entry for entry in packages_before if entry.get("name") == "jackin-console" and entry.get("version") == "0.1.0" and entry.get("source") is None]
if len(owners) != 1 or type(owners[0].get("dependencies")) is not list or owners[0]["dependencies"].count("junie-tui") != 1:
    raise SystemExit("generated-workflows: local-local removal lock edge is absent or ambiguous")
blocks = re.split(r"(?=^\[\[package\]\]$)", lock_text, flags=re.M)
owner_blocks = [index for index, block in enumerate(blocks) if re.search(r'^name = "jackin-console"$', block, re.M) and re.search(r'^version = "0.1.0"$', block, re.M) and re.search(r'^source = ', block, re.M) is None]
if len(owner_blocks) != 1:
    raise SystemExit("generated-workflows: local-local removal lock block is ambiguous")
index = owner_blocks[0]
match = re.search(r"(?ms)^(dependencies = \[\n)(.*?)(^\])", blocks[index])
if match is None:
    raise SystemExit("generated-workflows: local-local removal lock dependency list is missing")
dependency_lines = match.group(2).splitlines()
if dependency_lines.count(' "junie-tui",') != 1:
    raise SystemExit("generated-workflows: local-local removal lock edge line is ambiguous")
dependency_lines.remove(' "junie-tui",')
replacement = match.group(1) + "\n".join(dependency_lines) + "\n]"
blocks[index] = blocks[index][:match.start()] + replacement + blocks[index][match.end():]
lock_after_text = "".join(blocks)
manifest_after_text = manifest_text.replace(feature_line, 'demo = ["dep:clap"]\n', 1).replace(dependency_line, "", 1)
try:
    manifest_after = tomllib.loads(manifest_after_text)
    lock_after = tomllib.loads(lock_after_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit("generated-workflows: local-local removal fixture is invalid TOML") from error
expected_manifest = copy.deepcopy(manifest_before)
del expected_manifest["dependencies"]["junie-tui"]
expected_manifest["features"]["demo"] = ["dep:clap"]
if manifest_after != expected_manifest:
    raise SystemExit("generated-workflows: local-local removal changed unrelated manifest data")
expected_lock = copy.deepcopy(lock_before)
expected_owners = [entry for entry in expected_lock["package"] if entry.get("name") == "jackin-console" and entry.get("version") == "0.1.0" and entry.get("source") is None]
expected_owners[0]["dependencies"].remove("junie-tui")
if lock_after != expected_lock:
    raise SystemExit("generated-workflows: local-local removal changed package records beyond its one local edge")
manifest.write_text(manifest_after_text)
lock_path.write_text(lock_after_text)
PY
}
assert_policy_source_unchanged() {
  local current_status_sha
  local current_refs_sha
  local current_untracked_sha
  local current_source_snapshot_sha
  local current_source_branch
  current_source_branch=$(policy_lifecycle_branch "$repo_root") || return 1
  if [[ "$("$git_command" -C "$repo_root" rev-parse HEAD)" != "$policy_source_head" || "$current_source_branch" != "$policy_source_branch" || "$("$git_command" -C "$repo_root" write-tree)" != "$policy_source_index_tree" ]]; then
    printf 'generated-workflows: policy fixture changed the original source ref, HEAD, or index\n' >&2
    return 1
  fi
  if ! "$git_command" -C "$repo_root" status --porcelain=v1 -z > "$test_root/policy-source-status.current"; then
    printf 'generated-workflows: cannot inspect original policy source state\n' >&2
    return 1
  fi
  current_status_sha=$(sha256_file "$test_root/policy-source-status.current") || return 1
  if [[ "$current_status_sha" != "$policy_source_status_sha" ]]; then
    printf 'generated-workflows: policy fixture changed the original source checkout state\n' >&2
    return 1
  fi
  if ! "$git_command" -C "$repo_root" show-ref --head > "$test_root/policy-source-refs.current"; then
    printf 'generated-workflows: cannot inspect original policy source refs\n' >&2
    return 1
  fi
  current_refs_sha=$(sha256_file "$test_root/policy-source-refs.current") || return 1
  if [[ "$current_refs_sha" != "$policy_source_refs_sha" ]]; then
    printf 'generated-workflows: policy fixture changed an original source ref\n' >&2
    return 1
  fi
  if ! write_policy_source_untracked_manifest "$repo_root" "$test_root/policy-source-untracked.current.nul" "$test_root/policy-source-untracked.current.json"; then
    return 1
  fi
  current_untracked_sha=$(sha256_file "$test_root/policy-source-untracked.current.json") || return 1
  if [[ "$current_untracked_sha" != "$policy_source_untracked_manifest_sha" ]]; then
    printf 'generated-workflows: policy fixture changed an original untracked source input\n' >&2
    return 1
  fi
  if ! policy_lifecycle_source_snapshot "$repo_root" > "$test_root/policy-source-snapshot.current.json"; then
    printf 'generated-workflows: cannot capture the original source snapshot after a policy boundary\n' >&2
    return 1
  fi
  current_source_snapshot_sha=$(sha256_file "$test_root/policy-source-snapshot.current.json") || return 1
  if [[ "$current_source_snapshot_sha" != "$policy_source_snapshot_sha" ]]; then
    printf 'generated-workflows: policy boundary changed the complete original source snapshot\n' >&2
    return 1
  fi
}
prepare_policy_candidate_control() {
  local policy_patch="$test_root/policy-source.patch"
  local untracked_inventory="$test_root/policy-source-untracked.nul"
  local candidate_diff="$test_root/policy-candidate-control-paths"
  local expected_diff="$test_root/policy-candidate-control-paths.expected"
  local seed_is_tracked
  local seed_is_untracked
  local input_path
  policy_source_head=$("$git_command" -C "$repo_root" rev-parse HEAD)
  policy_source_branch=$(policy_lifecycle_branch "$repo_root") || return 1
  policy_source_index_tree=$("$git_command" -C "$repo_root" write-tree)
  if ! "$git_command" -C "$repo_root" status --porcelain=v1 -z > "$test_root/policy-source-status.original"; then
    printf 'generated-workflows: cannot inspect original policy source state\n' >&2
    return 1
  fi
  policy_source_status_sha=$(sha256_file "$test_root/policy-source-status.original") || return 1
  if ! "$git_command" -C "$repo_root" show-ref --head > "$test_root/policy-source-refs.original"; then
    printf 'generated-workflows: cannot inspect original policy source refs\n' >&2
    return 1
  fi
  policy_source_refs_sha=$(sha256_file "$test_root/policy-source-refs.original") || return 1
  if ! policy_lifecycle_source_snapshot "$repo_root" > "$test_root/policy-source-snapshot.original.json"; then
    printf 'generated-workflows: cannot capture the complete original policy source snapshot\n' >&2
    return 1
  fi
  policy_source_snapshot_sha=$(sha256_file "$test_root/policy-source-snapshot.original.json") || return 1
  if [[ ! -f "$repo_root/scripts/trusted-seed-cargo.lock" || -L "$repo_root/scripts/trusted-seed-cargo.lock" ]]; then
    printf 'generated-workflows: reviewed trusted seed is missing or unsafe in the source checkout\n' >&2
    return 1
  fi
  if [[ ! -f "$repo_root/scripts/trusted-seed-cargo-policy.json" || -L "$repo_root/scripts/trusted-seed-cargo-policy.json" ]]; then
    printf 'generated-workflows: reviewed trusted seed Cargo policy is missing or unsafe in the source checkout\n' >&2
    return 1
  fi
  if ! python3 -I -S - "$repo_root/scripts/trusted-seed.commit" <<'PYSEED' 2>/dev/null
import hashlib
import os
import stat
import sys

path = sys.argv[1]
try:
    before_path = os.lstat(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or before.st_size > 4096:
            raise ValueError
        chunks = []
        remaining = 4097
        while remaining:
            chunk = os.read(fd, remaining)
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(fd)
    finally:
        os.close(fd)
    after_path = os.lstat(path)
    def identity(info):
        return (info.st_dev, info.st_ino, info.st_mode, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
    if len(data) > 4096 or len(data) != before.st_size or identity(before_path) != identity(before) or identity(before) != identity(after) or identity(after) != identity(after_path):
        raise ValueError
    if hashlib.sha1(b"commit " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest() != "f4d9999ace7d6deb4294cd89c00afcc80e748d49":
        raise ValueError
except (OSError, ValueError):
    sys.exit(1)
PYSEED
  then
    printf 'generated-workflows: reviewed trusted seed commit differs from the approved immutable seed\n' >&2
    return 1
  fi
  policy_seed_sha=$(sha256_file "$repo_root/scripts/trusted-seed-cargo.lock") || return 1
  if [[ "$policy_seed_sha" != d1abe8b74eac36822b25a735e8cc28e2c8434103a8c87f662a9ebe2da5f593a7 ]]; then
    printf 'generated-workflows: policy fixture trusted seed differs from the reviewed source input\n' >&2
    return 1
  fi
  policy_manifest_sha=$(sha256_file "$repo_root/scripts/trusted-seed-cargo-policy.json") || return 1
  if [[ "$policy_manifest_sha" != 6d6c35161e163ff07f39d413e3d25c6d60016ebfcb6627e278e062a86df30eae ]]; then
    printf 'generated-workflows: policy fixture trusted seed Cargo policy differs from the reviewed source input\n' >&2
    return 1
  fi
  if ! write_policy_source_untracked_manifest "$repo_root" "$untracked_inventory" "$test_root/policy-source-untracked.original.json"; then
    return 1
  fi
  policy_source_untracked_manifest_sha=$(sha256_file "$test_root/policy-source-untracked.original.json") || return 1
  if "$git_command" -C "$repo_root" ls-files --error-unmatch -- scripts/trusted-seed-cargo.lock >/dev/null 2>&1; then
    seed_is_tracked=1
  else
    seed_is_tracked=0
  fi
  if ! python3 -I -S - "$untracked_inventory" "$seed_is_tracked" <<'PY'
import sys
from pathlib import Path

inventory = Path(sys.argv[1]).read_bytes().split(b"\0")
paths = [item.decode("utf-8") for item in inventory if item]
if sys.argv[2] == "0" and paths.count("scripts/trusted-seed-cargo.lock") != 1:
    raise SystemExit("generated-workflows: untracked inventory does not contain the reviewed trusted seed")
PY
  then
    return 1
  fi
  seed_is_untracked=0
  if [[ "$seed_is_tracked" == 0 ]]; then
    seed_is_untracked=1
  fi
  policy_source_repository="$test_root/policy-source"
  if ! "$git_command" clone --quiet --no-hardlinks "$repo_root" "$policy_source_repository"; then
    printf 'generated-workflows: cannot create the private policy source snapshot\n' >&2
    return 1
  fi
  "$git_command" -C "$policy_source_repository" remote remove origin
  "$git_command" -C "$policy_source_repository" config user.name 'Generated workflow qualification'
  "$git_command" -C "$policy_source_repository" config user.email 'generated-workflow@example.invalid'
  if ! "$git_command" -C "$repo_root" diff --binary "$policy_source_head" -- > "$policy_patch"; then
    printf 'generated-workflows: cannot capture the complete tracked policy candidate input\n' >&2
    return 1
  fi
  if [[ -s "$policy_patch" || "$seed_is_untracked" == 1 ]]; then
    policy_source_snapshot_kind=private-dirty-snapshot
    policy_historical_head=$policy_source_head
    "$git_command" -C "$policy_source_repository" switch --quiet -C main "$policy_historical_head"
    if [[ -s "$policy_patch" ]] && ! "$git_command" -C "$policy_source_repository" apply --binary "$policy_patch"; then
      printf 'generated-workflows: tracked candidate patch cannot be applied to its historical HEAD\n' >&2
      return 1
    fi
  else
    policy_source_snapshot_kind=committed-candidate
    policy_historical_head=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_source_head^1") || {
      printf 'generated-workflows: clean committed candidate has no historical parent\n' >&2
      return 1
    }
    "$git_command" -C "$policy_source_repository" switch --quiet -C main "$policy_source_head"
  fi
  if [[ "$seed_is_untracked" == 1 ]]; then
    mkdir -p "$policy_source_repository/scripts"
    cp -p "$repo_root/scripts/trusted-seed-cargo.lock" "$policy_source_repository/scripts/trusted-seed-cargo.lock"
  fi
  if [[ ! -f "$policy_source_repository/scripts/trusted-seed-cargo-policy.json" ]]; then
    mkdir -p "$policy_source_repository/scripts"
    cp -p "$repo_root/scripts/trusted-seed-cargo-policy.json" "$policy_source_repository/scripts/trusted-seed-cargo-policy.json"
  fi
  if [[ ! -f "$policy_source_repository/scripts/generate-trusted-seed-cargo-policy.py" ]]; then
    mkdir -p "$policy_source_repository/scripts"
    cp -p "$repo_root/scripts/generate-trusted-seed-cargo-policy.py" "$policy_source_repository/scripts/generate-trusted-seed-cargo-policy.py"
  fi
  if [[ "$(sha256_file "$policy_source_repository/scripts/trusted-seed-cargo.lock")" != "$policy_seed_sha" ]]; then
    printf 'generated-workflows: private candidate snapshot did not preserve the trusted seed bytes\n' >&2
    return 1
  fi
  if [[ "$(sha256_file "$policy_source_repository/scripts/trusted-seed-cargo-policy.json")" != "$policy_manifest_sha" ]]; then
    printf 'generated-workflows: private candidate snapshot did not preserve the trusted seed Cargo policy bytes\n' >&2
    return 1
  fi
  if [[ "$policy_source_snapshot_kind" == private-dirty-snapshot ]]; then
    "$git_command" -C "$policy_source_repository" add -A -- .
    if "$git_command" -C "$policy_source_repository" diff --cached --quiet; then
      printf 'generated-workflows: dirty source candidate contains no tracked or trusted-seed changes\n' >&2
      return 1
    fi
    if ! "$git_command" -c core.hooksPath=/dev/null -C "$policy_source_repository" commit --quiet -m 'test: snapshot reviewed Cargo policy candidate'; then
      printf 'generated-workflows: cannot create the private policy candidate commit\n' >&2
      return 1
    fi
    policy_candidate_head=$("$git_command" -C "$policy_source_repository" rev-parse HEAD)
    policy_candidate_parent=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_candidate_head^1")
    if [[ "$policy_candidate_parent" != "$policy_historical_head" ]]; then
      printf 'generated-workflows: private candidate commit does not retain its historical source parent\n' >&2
      return 1
    fi
  else
    policy_candidate_head=$policy_source_head
    policy_candidate_parent=$policy_historical_head
  fi
  policy_candidate_tree=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_candidate_head^{tree}")
  "$git_command" -C "$policy_source_repository" update-ref refs/qualification/policy-candidate "$policy_candidate_head"
  if ! "$git_command" -C "$policy_source_repository" ls-files --error-unmatch -- scripts/trusted-seed-cargo.lock >/dev/null 2>&1 || ! "$git_command" -C "$policy_source_repository" ls-files --error-unmatch -- scripts/trusted-seed-cargo-policy.json >/dev/null 2>&1 || [[ "$("$git_command" -C "$policy_source_repository" rev-parse refs/heads/main)" != "$policy_candidate_head" || -n "$("$git_command" -C "$policy_source_repository" status --porcelain)" ]]; then
    printf 'generated-workflows: private candidate checkout is not clean at the actual candidate commit\n' >&2
    return 1
  fi
  mkdir -p "$policy_candidate_files"
  for input_path in Cargo.toml apps/jackin/Cargo.toml Cargo.lock; do
    local candidate_file
    case "$input_path" in
      Cargo.toml) candidate_file=root-Cargo.toml.candidate ;;
      apps/jackin/Cargo.toml) candidate_file=app-Cargo.toml.candidate ;;
      Cargo.lock) candidate_file=Cargo.lock.candidate ;;
    esac
    if [[ ! -f "$repo_root/$input_path" || -L "$repo_root/$input_path" ]]; then
      printf 'generated-workflows: reviewed candidate input is missing or unsafe: %s\n' "$input_path" >&2
      return 1
    fi
    "$git_command" -C "$policy_source_repository" show "$policy_candidate_head:$input_path" > "$policy_candidate_files/$candidate_file"
    if ! cmp -s "$repo_root/$input_path" "$policy_candidate_files/$candidate_file"; then
      printf 'generated-workflows: private candidate source differs from the actual input file: %s\n' "$input_path" >&2
      return 1
    fi
    if [[ "$(sha256_file "$policy_source_repository/$input_path")" != "$(sha256_file "$repo_root/$input_path")" ]]; then
      printf 'generated-workflows: candidate file hash changed while snapshotting: %s\n' "$input_path" >&2
      return 1
    fi
  done
  if ! python3 -I -S - "$policy_source_repository" "$policy_seed_sha" <<'PY'
import hashlib
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
expected_seed_sha = sys.argv[2]
seed_bytes = (root / "scripts/trusted-seed-cargo.lock").read_bytes()
candidate_bytes = (root / "Cargo.lock").read_bytes()
if hashlib.sha256(seed_bytes).hexdigest() != expected_seed_sha:
    raise SystemExit("generated-workflows: private candidate seed hash changed before policy qualification")
if candidate_bytes != seed_bytes:
    raise SystemExit("generated-workflows: actual candidate Cargo.lock is not byte-equal to the pinned trusted seed")
try:
    seed = tomllib.loads(seed_bytes.decode("utf-8"))
    candidate = tomllib.loads(candidate_bytes.decode("utf-8"))
    tomllib.loads((root / "Cargo.toml").read_text())
    app = tomllib.loads((root / "apps/jackin/Cargo.toml").read_text())
except (tomllib.TOMLDecodeError, UnicodeError) as error:
    raise SystemExit("generated-workflows: policy candidate metadata is invalid TOML") from error
if candidate != seed or candidate.get("version") != 4 or type(candidate.get("package")) is not list:
    raise SystemExit("generated-workflows: actual candidate lock graph differs from the pinned trusted seed")
packages = candidate["package"]
seed_packages = seed.get("package")
if type(seed_packages) is not list or len(packages) != 299 or packages != seed_packages:
    raise SystemExit("generated-workflows: complete candidate package inventory differs from the trusted seed")
identities = [(item.get("name"), item.get("version"), item.get("source")) for item in packages]
if len(identities) != len(set(identities)):
    raise SystemExit("generated-workflows: trusted seed package identities are not unique")
local = {item.get("name"): item for item in packages if item.get("source") is None}
if set(local) != {"jackin-console", "junie-tui"}:
    raise SystemExit("generated-workflows: trusted seed local-owner identities drifted")
if local["junie-tui"].get("version") != "0.1.0" or local["junie-tui"].get("dependencies", []).count("syn 3.0.9") != 1:
    raise SystemExit("generated-workflows: trusted seed junie-tui syn edge identity drifted")
if local["jackin-console"].get("version") != "0.1.0" or local["jackin-console"].get("dependencies", []).count("unicode-segmentation") != 1:
    raise SystemExit("generated-workflows: trusted seed jackin-console unicode-segmentation edge identity drifted")
syn = [item for item in packages if item.get("name") == "syn"]
if len(syn) != 3 or {item.get("version") for item in syn} != {"1.0.109", "2.0.119", "3.0.9"}:
    raise SystemExit("generated-workflows: trusted seed syn version inventory is not the reviewed three-record set")
syn_qualified = [item for item in packages if item.get("name") == "syn" and item.get("version") == "3.0.9" and item.get("source") == "registry+https://github.com/rust-lang/crates.io-index"]
if len(syn_qualified) != 1:
    raise SystemExit("generated-workflows: trusted seed qualified syn edge does not resolve uniquely")
external = [item for item in packages if item.get("source") is not None]
seed_external = [item for item in seed_packages if item.get("source") is not None]
if external != seed_external:
    raise SystemExit("generated-workflows: complete external package inventory differs from the trusted seed")
if app.get("dependencies", {}).get("junie-tui") != {"path": "../..", "default-features": False, "features": ["terminal"]}:
    raise SystemExit("generated-workflows: candidate app local dependency identity drifted")
PY
  then
    return 1
  fi
  if [[ "$(sha256_file "$policy_source_repository/Cargo.lock")" != "$policy_seed_sha" ]]; then
    printf 'generated-workflows: actual candidate Cargo.lock does not match the trusted seed hash\n' >&2
    return 1
  fi
  if ! "$git_command" -c core.hooksPath=/dev/null -C "$policy_source_repository" commit --allow-empty --quiet -m 'test: preserve trusted Cargo policy control'; then
    printf 'generated-workflows: cannot create the empty immutable policy control child\n' >&2
    return 1
  fi
  policy_control_head=$("$git_command" -C "$policy_source_repository" rev-parse HEAD)
  policy_control_tree=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_control_head^{tree}")
  if [[ "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_control_head^1")" != "$policy_candidate_head" || "$policy_control_tree" != "$policy_candidate_tree" ]]; then
    printf 'generated-workflows: empty policy control does not preserve the candidate parent and exact tree\n' >&2
    return 1
  fi
  "$git_command" -C "$policy_source_repository" update-ref refs/qualification/policy-control "$policy_control_head"
  if ! "$git_command" -C "$policy_source_repository" diff --name-only "$policy_candidate_head" "$policy_control_head" > "$candidate_diff"; then
    printf 'generated-workflows: cannot compare the immutable candidate and empty control trees\n' >&2
    return 1
  fi
  : > "$expected_diff"
  if ! cmp -s "$candidate_diff" "$expected_diff"; then
    printf 'generated-workflows: empty policy control changed candidate tree paths\n' >&2
    return 1
  fi
  if [[ "$("$git_command" -C "$policy_source_repository" rev-parse refs/heads/main)" != "$policy_control_head" || -n "$("$git_command" -C "$policy_source_repository" status --porcelain)" ]]; then
    printf 'generated-workflows: private policy source refs or checkout are not at the clean control commit\n' >&2
    return 1
  fi
  assert_policy_source_unchanged || return 1
  {
    printf 'historical_head=%s\n' "$policy_historical_head"
    printf 'source_head=%s\n' "$policy_source_head"
    printf 'source_branch=%s\n' "$policy_source_branch"
    printf 'source_snapshot_kind=%s\n' "$policy_source_snapshot_kind"
    printf 'source_index_tree=%s\n' "$policy_source_index_tree"
    printf 'source_status_sha256=%s\n' "$policy_source_status_sha"
    printf 'source_refs_sha256=%s\n' "$policy_source_refs_sha"
    printf 'source_snapshot_sha256=%s\n' "$policy_source_snapshot_sha"
    printf 'source_untracked_manifest_sha256=%s\n' "$policy_source_untracked_manifest_sha"
    printf 'tracked_candidate_patch_sha256=%s\n' "$(sha256_file "$policy_patch")"
    printf 'untracked_inventory_sha256=%s\n' "$(sha256_file "$untracked_inventory")"
    printf 'candidate_commit=%s\n' "$policy_candidate_head"
    printf 'candidate_parent=%s\n' "$policy_candidate_parent"
    printf 'candidate_tree=%s\n' "$policy_candidate_tree"
    printf 'control_commit=%s\n' "$policy_control_head"
    printf 'control_tree=%s\n' "$policy_control_tree"
    printf 'control_parent=%s\n' "$policy_candidate_head"
    printf 'control_empty_commit=true\n'
    printf 'trusted_seed_sha256=%s\n' "$policy_seed_sha"
    printf 'candidate_root_manifest_sha256=%s\n' "$(sha256_file "$policy_candidate_files/root-Cargo.toml.candidate")"
    printf 'candidate_app_manifest_sha256=%s\n' "$(sha256_file "$policy_candidate_files/app-Cargo.toml.candidate")"
    printf 'candidate_lock_sha256=%s\n' "$(sha256_file "$policy_candidate_files/Cargo.lock.candidate")"
    printf 'control_root_manifest_sha256=%s\n' "$(sha256_file "$policy_source_repository/Cargo.toml")"
    printf 'control_app_manifest_sha256=%s\n' "$(sha256_file "$policy_source_repository/apps/jackin/Cargo.toml")"
    printf 'control_lock_sha256=%s\n' "$(sha256_file "$policy_source_repository/Cargo.lock")"
    printf 'candidate_checker_sha256=%s\n' "$(sha256_file "$policy_source_repository/scripts/check-generated-workflows.sh")"
  } > "$test_root/policy-fixture-context.txt"
  if [[ "$e2e_focus" == cargo-local-lock-policy ]]; then
    local saved_repository=
    local saved_public_mutant=$public_mutant
    local preview_directory
    local preview_mutant
    for preview_mutant in whole-lock-digest edge-resolution; do
      preview_directory="$test_root/policy-mutant-preview-$preview_mutant"
      mkdir -p "$preview_directory/scripts"
      cp -p "$policy_source_repository/scripts/check-generated-workflows.sh" "$preview_directory/scripts/check-generated-workflows.sh"
      cp -p "$policy_source_repository/scripts/install-generated-workflow-guard.sh" "$preview_directory/scripts/install-generated-workflow-guard.sh"
      repository=$preview_directory
      public_mutant=$preview_mutant
      apply_lock_policy_mutant
      cp -p "$preview_directory/scripts/check-generated-workflows.sh" "$test_root/policy-mutant-checker-$preview_mutant"
      if /usr/bin/diff -u "$policy_source_repository/scripts/check-generated-workflows.sh" "$test_root/policy-mutant-checker-$preview_mutant" > "$test_root/policy-mutant-$preview_mutant.patch"; then
        printf 'generated-workflows: policy checker mutant preview produced no source diff: %s\n' "$preview_mutant" >&2
        return 1
      else
        local preview_diff_status=$?
        if [[ "$preview_diff_status" -ne 1 ]]; then
          printf 'generated-workflows: cannot preserve policy checker mutant preview: %s\n' "$preview_mutant" >&2
          return 1
        fi
      fi
      printf '%s_mutant_patch_sha256=%s\n' "$preview_mutant" "$(sha256_file "$test_root/policy-mutant-$preview_mutant.patch")" >> "$test_root/policy-fixture-context.txt"
      printf '%s_mutant_checker_sha256=%s\n' "$preview_mutant" "$(sha256_file "$test_root/policy-mutant-checker-$preview_mutant")" >> "$test_root/policy-fixture-context.txt"
    done
    repository=$saved_repository
    public_mutant=$saved_public_mutant
  fi
  if [[ "$policy_preflight_only" == 1 ]]; then
    local policy_export_head=$policy_control_head
    local policy_export_tree=$policy_control_tree
    if [[ "$policy_case" == positive-local-local-removal ]]; then
      local local_fixture_paths="$test_root/policy-local-positive-paths"
      local local_fixture_paths_after="$test_root/policy-local-positive-paths.after"
      local local_fixture_paths_after_sorted="$test_root/policy-local-positive-paths.after.sorted"
      local expected_local_fixture_paths="$test_root/policy-local-positive-paths.expected"
      if ! "$git_command" -C "$policy_source_repository" switch --quiet -C main "$policy_candidate_head"; then
        printf 'generated-workflows: cannot restore the candidate before local-positive preflight\n' >&2
        return 1
      fi
      if ! prepare_local_local_removal_fixture "$policy_source_repository"; then
        return 1
      fi
      if ! "$git_command" -C "$policy_source_repository" add -- apps/jackin/Cargo.toml Cargo.lock; then
        printf 'generated-workflows: cannot stage the local-positive metadata fixture\n' >&2
        return 1
      fi
      if ! "$git_command" -C "$policy_source_repository" diff --cached --name-only "$policy_candidate_head" -- > "$local_fixture_paths"; then
        printf 'generated-workflows: cannot inspect local-positive fixture paths\n' >&2
        return 1
      fi
      printf '%s\n' Cargo.lock apps/jackin/Cargo.toml | LC_ALL=C sort > "$expected_local_fixture_paths"
      if ! cmp -s "$local_fixture_paths" "$expected_local_fixture_paths"; then
        printf 'generated-workflows: local-positive fixture changed paths outside its manifest and lock edge\n' >&2
        return 1
      fi
      if ! "$git_command" -c core.hooksPath=/dev/null -C "$policy_source_repository" commit --quiet -m 'test: prepare valid local dependency removal fixture'; then
        printf 'generated-workflows: cannot commit the private local-positive metadata fixture\n' >&2
        return 1
      fi
      policy_local_fixture_head=$("$git_command" -C "$policy_source_repository" rev-parse HEAD)
      policy_local_fixture_tree=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_local_fixture_head^{tree}")
      if ! "$git_command" -C "$policy_source_repository" diff --name-only "$policy_candidate_head" "$policy_local_fixture_head" -- > "$local_fixture_paths_after" || ! LC_ALL=C sort "$local_fixture_paths_after" > "$local_fixture_paths_after_sorted"; then
        printf 'generated-workflows: cannot verify local-positive fixture commit paths\n' >&2
        return 1
      fi
      if [[ "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_local_fixture_head^1")" != "$policy_candidate_head" || "$("$git_command" -C "$policy_source_repository" rev-parse refs/heads/main)" != "$policy_local_fixture_head" || -n "$("$git_command" -C "$policy_source_repository" status --porcelain)" ]] || ! cmp -s "$local_fixture_paths_after_sorted" "$expected_local_fixture_paths"; then
        printf 'generated-workflows: local-positive metadata fixture lost its candidate parent, exact paths, or clean state\n' >&2
        return 1
      fi
      policy_export_head=$policy_local_fixture_head
      policy_export_tree=$policy_local_fixture_tree
      {
        printf 'local_positive_fixture_commit=%s\n' "$policy_local_fixture_head"
        printf 'local_positive_fixture_parent=%s\n' "$policy_candidate_head"
        printf 'local_positive_fixture_tree=%s\n' "$policy_local_fixture_tree"
        printf 'local_positive_manifest_sha256=%s\n' "$(sha256_file "$policy_source_repository/apps/jackin/Cargo.toml")"
        printf 'local_positive_lock_sha256=%s\n' "$(sha256_file "$policy_source_repository/Cargo.lock")"
      } >> "$test_root/policy-fixture-context.txt"
    fi
    /bin/cp -Rp "$policy_source_repository" "$policy_control_export"
    if [[ "$("$git_command" -C "$policy_control_export" rev-parse HEAD)" != "$policy_export_head" || "$("$git_command" -C "$policy_control_export" rev-parse refs/qualification/policy-candidate)" != "$policy_candidate_head" || "$("$git_command" -C "$policy_control_export" rev-parse 'HEAD^{tree}')" != "$policy_export_tree" ]]; then
      printf 'generated-workflows: exported policy control lost its candidate or tree binding\n' >&2
      return 1
    fi
    cat "$test_root/policy-fixture-context.txt"
    if [[ "$policy_case" == positive-local-local-removal ]]; then
      printf 'local_positive_export=%s\n' "$policy_control_export"
      printf 'local_positive_commit=%s\n' "$policy_export_head"
      printf 'local_positive_tree=%s\n' "$policy_export_tree"
    else
      printf 'control_export=%s\n' "$policy_control_export"
    fi
    record_completion_witness policy-preflight
    mark_completion_complete
    exit 0
  fi
}
if [[ "$e2e_focus" == cargo-local-lock-policy ]]; then
  prepare_policy_candidate_control
fi
repository="$test_root/repository with 'quotes' and spaces"
remote="$test_root/remote.git"
home_dir="$test_root/home"
hooks_path="$test_root/custom hooks"
attacker_marker="$test_root/attacker executed"
mkdir -p "$repository" "$home_dir"
: > "$test_root/case-results.log"
"$git_command" init --quiet --initial-branch=main "$repository"
if [[ "$e2e_focus" == cargo-local-lock-policy ]]; then
  source_repository=$policy_source_repository
  if [[ "$policy_case" == positive-external-edge ]]; then
    source_head=$policy_control_head
  else
    source_head=$policy_candidate_head
  fi
else
  source_repository=$repo_root
  source_head=$("$git_command" -C "$repo_root" rev-parse HEAD)
fi
"$git_command" -C "$repository" fetch --quiet --depth=1 --no-tags "file://$source_repository" "$source_head"
"$git_command" -C "$repository" checkout --quiet -B main FETCH_HEAD
if [[ "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" ]]; then
  printf 'generated-workflows: disposable source checkout did not bind the selected source commit\n' >&2
  exit 1
fi
if "$git_command" -C "$repository" cat-file -e 'f4d9999ace7d6deb4294cd89c00afcc80e748d49^{commit}' 2>/dev/null; then
  printf 'generated-workflows: shallow fixture unexpectedly contains the approved seed commit\n' >&2
  exit 1
fi
"$git_command" -C "$repository" config user.name 'Generated workflow qualification'
"$git_command" -C "$repository" config user.email 'generated-workflow@example.invalid'
"$git_command" -C "$repository" config core.hooksPath "$hooks_path"

if [[ "$e2e_focus" == cargo-local-lock-policy ]]; then
  :
elif [[ "$e2e_focus" == installer-managed-hook-upgrade ]]; then
  for input in check-generated-workflows.sh generated-workflow-guard-config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects python-mise.toml python-mise.macos-arm64.lock.toml python-mise.linux-x64.lock.toml install-generated-workflow-guard.sh; do
    copy_reviewed_old_input "$input" "$repository/scripts/$input"
  done
else
  for input in check-generated-workflows.sh generated-workflow-guard-config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects trusted-seed-cargo.lock trusted-seed-cargo-policy.json python-mise.toml python-mise.macos-arm64.lock.toml python-mise.linux-x64.lock.toml install-generated-workflow-guard.sh; do
    cp -p "$repo_root/scripts/$input" "$repository/scripts/$input"
  done
fi
cp -p "$repository/scripts/check-generated-workflows.sh" "$test_root/checkout-checker.original"
original_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
if [[ "$policy_case" == typed-workspace-boolean-int || "$policy_case" == typed-dependency-boolean-int || "$policy_case" == typed-protected-boolean-int || "$policy_case" == lock-version-float ]]; then
  apply_policy_acquisition_sentinels
  mutant_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
fi
if [[ "$public_mutant" == relative-git-path ]]; then
  (cd "$repository" && /usr/bin/patch --quiet -p1 < "$repo_root/tests/mutants/relative-git-path.patch")
  mutant_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
  /usr/bin/sed "s/^reviewed_checker_sha256=.*/reviewed_checker_sha256=$mutant_checker_sha256/" "$repository/scripts/install-generated-workflow-guard.sh" > "$test_root/mutant-installer"
  mv "$test_root/mutant-installer" "$repository/scripts/install-generated-workflow-guard.sh"
fi
if [[ "$public_mutant" == whole-lock-digest || "$public_mutant" == external-record || "$public_mutant" == unknown-field || "$public_mutant" == edge-resolution || "$public_mutant" == path-policy || "$public_mutant" == feature-coupling || "$public_mutant" == typed-toml-equality ]]; then
  apply_lock_policy_mutant
  mutant_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
fi
if [[ "$e2e_focus" == cargo-local-lock-policy && ( "$public_mutant" == whole-lock-digest || "$public_mutant" == external-record || "$public_mutant" == unknown-field || "$public_mutant" == edge-resolution || "$public_mutant" == path-policy || "$public_mutant" == feature-coupling || "$public_mutant" == typed-toml-equality ) ]]; then
  cp -p "$test_root/checkout-checker.original" "$test_root/policy-mutant-checker.original"
  cp -p "$repository/scripts/check-generated-workflows.sh" "$test_root/policy-mutant-checker.mutant"
  if /usr/bin/diff -u "$test_root/policy-mutant-checker.original" "$test_root/policy-mutant-checker.mutant" > "$test_root/policy-mutant.patch"; then
    printf 'generated-workflows: policy checker mutant produced no source diff\n' >&2
    exit 1
  else
    mutant_diff_status=$?
    if [[ "$mutant_diff_status" -ne 1 ]]; then
      printf 'generated-workflows: cannot preserve policy checker mutant patch\n' >&2
      exit 1
    fi
  fi
  {
    printf 'mutant=%s\n' "$public_mutant"
    printf 'checker_original_sha256=%s\n' "$original_checker_sha256"
    printf 'checker_mutant_sha256=%s\n' "$mutant_checker_sha256"
  } >> "$test_root/policy-fixture-context.txt"
fi
if [[ "$installer_mutant" == python-integrity-bypass ]]; then
  apply_python_integrity_bypass
fi
common_dir=$("$git_command" -C "$repository" rev-parse --path-format=absolute --git-common-dir)
common_dir=$(cd "$common_dir" && pwd -P)
installation="$common_dir/jackin-generated-workflow-guard"
case "$(uname -s)" in
  Darwin)
    if [[ "$cold_install" -eq 0 ]]; then /bin/cp -cRp "$guard_install_root" "$installation"; fi
    ;;
  Linux)
    if [[ "$cold_install" -eq 0 ]]; then /bin/cp -a --reflink=auto "$guard_install_root" "$installation"; fi
    ;;
  *) printf 'generated-workflows: unsupported host for the real-toolchain qualification\n' >&2; exit 2 ;;
esac
if [[ "$public_mutant" == relative-git-path || "$public_mutant" == whole-lock-digest || "$public_mutant" == external-record || "$public_mutant" == unknown-field || "$public_mutant" == edge-resolution || "$public_mutant" == path-policy || "$public_mutant" == feature-coupling || "$public_mutant" == typed-toml-equality || "$policy_case" == typed-workspace-boolean-int || "$policy_case" == typed-dependency-boolean-int || "$policy_case" == typed-protected-boolean-int || "$policy_case" == lock-version-float ]]; then
  rekey_mutant_tool_cache "$installation/cache/tools" "$original_checker_sha256" "$mutant_checker_sha256"
fi
"$git_command" init --quiet --bare "$remote"
"$git_command" -C "$remote" fetch --quiet "file://$source_repository" "$source_head"
"$git_command" -C "$remote" update-ref refs/heads/main FETCH_HEAD
"$git_command" -C "$repository" remote add origin "$remote"
if [[ "$e2e_focus" == installer-seed-lock-artifact ]]; then
  source_seed_lock="$repository/scripts/trusted-seed-cargo.lock"
  source_seed_policy="$repository/scripts/trusted-seed-cargo-policy.json"
  cp -p "$source_seed_lock" "$test_root/trusted-seed-cargo.lock.original"
  cp -p "$source_seed_policy" "$test_root/trusted-seed-cargo-policy.json.original"
  printf 'preserve this seed artifact fixture sentinel\n' > "$test_root/seed-lock-sentinel"
  source_index_tree=$("$git_command" -C "$repository" write-tree)
  source_config_sha=$(sha256_file "$repository/.git/config")
  source_output_sha=$(directory_digest "$repository/.github")
  source_receiver_refs=$("$git_command" -C "$remote" show-ref || true)
  source_sentinel_sha=$(sha256_file "$test_root/seed-lock-sentinel")
  for artifact_name in seed-lock seed-policy; do
    case "$artifact_name" in
      seed-lock)
        source_artifact=$source_seed_lock
        artifact_original="$test_root/trusted-seed-cargo.lock.original"
        changed_diagnostic='trusted seed Cargo.lock differs from the reviewed installer pins'
        ;;
      seed-policy)
        source_artifact=$source_seed_policy
        artifact_original="$test_root/trusted-seed-cargo-policy.json.original"
        changed_diagnostic='trusted seed Cargo policy differs from the reviewed installer pins'
        ;;
    esac
    for artifact_case in missing changed symlink nonregular; do
      rm -rf "$source_artifact"
      cp -p "$artifact_original" "$source_artifact"
      case "$artifact_case" in
        missing)
          rm "$source_artifact"
          expected='reviewed installation input is not a regular file'
          ;;
        changed)
          chmod u+w "$source_artifact"
          printf 'changed\n' >> "$source_artifact"
          chmod 400 "$source_artifact"
          expected=$changed_diagnostic
          ;;
        symlink)
          rm "$source_artifact"
          ln -s "$artifact_original" "$source_artifact"
          expected='reviewed installation input is not a regular file'
          ;;
        nonregular)
          rm "$source_artifact"
          mkdir "$source_artifact"
          expected='reviewed installation input is not a regular file'
          ;;
      esac
      installer_status=0
      if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/$artifact_name-source-$artifact_case.log" 2>&1; then
        installer_status=0
      else
        installer_status=$?
      fi
      if [[ "$installer_status" -eq 0 ]] || ! /usr/bin/grep -F -q "$expected" "$test_root/$artifact_name-source-$artifact_case.log" || [[ -e "$hooks_path/pre-commit" || -L "$hooks_path/pre-commit" || -e "$hooks_path/pre-push" || -L "$hooks_path/pre-push" || -e "$installation/versions/$original_checker_sha256" || "$("$git_command" -C "$repository" write-tree)" != "$source_index_tree" || "$(sha256_file "$repository/.git/config")" != "$source_config_sha" || "$(directory_digest "$repository/.github")" != "$source_output_sha" || "$("$git_command" -C "$remote" show-ref || true)" != "$source_receiver_refs" || "$(sha256_file "$test_root/seed-lock-sentinel")" != "$source_sentinel_sha" ]]; then
        cat "$test_root/$artifact_name-source-$artifact_case.log" >&2
        printf 'generated-workflows: invalid trusted seed Cargo artifact installer input changed publication or fixture state\n' >&2
        exit 1
      fi
    done
    rm -rf "$source_artifact"
    cp -p "$artifact_original" "$source_artifact"
  done
fi
if ! (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/setup.log" 2>&1; then
  cat "$test_root/setup.log" >&2
  printf 'generated-workflows: real installed checker setup failed\n' >&2
  exit 1
fi
if [[ -n "$installer_mutant" && "$installer_mutant" != python-integrity-bypass && "$e2e_focus" != installer-managed-hook-upgrade ]]; then
  apply_installer_mutant
fi
printf 'private installed checker setup: PASS\n' >> "$test_root/case-results.log"

checker_digest=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
installed_manifest="$installation/versions/$checker_digest/manifest"
manifest_value() { /usr/bin/sed -n "s/^$1=//p" "$installed_manifest"; }
prepare_external_edge_fixture() {
  local policy_velnor_executable
  local policy_velnor_version
  local first_output_sha
  local second_output_sha
  local changed_paths="$test_root/policy-external-changed-paths.nul"
  local candidate_paths="$test_root/policy-external-candidate-paths.nul"
  local expected_paths="$test_root/policy-external-candidate-paths.expected.nul"
  local diff_status
  if ! "$git_command" -C "$policy_source_repository" switch --quiet -C main "$policy_control_head" || [[ -n "$("$git_command" -C "$policy_source_repository" status --porcelain)" || "$("$git_command" -C "$policy_source_repository" rev-parse HEAD)" != "$policy_control_head" || "$("$git_command" -C "$policy_source_repository" write-tree)" != "$policy_control_tree" ]]; then
    printf 'generated-workflows: external-edge fixture did not start from the immutable empty control child\n' >&2
    return 1
  fi
  if ! python3 -I -S - "$policy_source_repository" "$policy_seed_sha" <<'PY'
import copy
import hashlib
import re
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
seed_sha = sys.argv[2]
app_path = root / "apps/jackin/Cargo.toml"
lock_path = root / "Cargo.lock"
seed_bytes = (root / "scripts/trusted-seed-cargo.lock").read_bytes()
app_text = app_path.read_text()
lock_bytes = lock_path.read_bytes()
if hashlib.sha256(seed_bytes).hexdigest() != seed_sha or lock_bytes != seed_bytes:
    raise SystemExit("generated-workflows: external-edge fixture control is not byte-equal to the pinned seed lock")
try:
    app_before = tomllib.loads(app_text)
    lock_before = tomllib.loads(lock_bytes.decode("utf-8"))
except (tomllib.TOMLDecodeError, UnicodeError) as error:
    raise SystemExit("generated-workflows: external-edge fixture inputs are invalid TOML") from error
for table in (app_before.get("dependencies", {}), app_before.get("dev-dependencies", {}), app_before.get("build-dependencies", {})):
    if "syn" in table:
        raise SystemExit("generated-workflows: app already declares syn before the external-edge fixture")
for table_name, table in app_before.items():
    if table_name.startswith("target.") and type(table) is dict:
        for dependency_kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            if "syn" in table.get(dependency_kind, {}):
                raise SystemExit("generated-workflows: app target metadata already declares syn")
dependency = {"version": "3.0.9", "features": ["full", "visit"]}
section = "[dev-dependencies]\n"
line = 'syn = { version = "3.0.9", features = ["full", "visit"] }\n'
if app_text.count(section) != 1 or line in app_text:
    raise SystemExit("generated-workflows: app dev-dependency insertion point is ambiguous")
app_after_text = app_text.replace(section, section + line, 1)
try:
    app_after = tomllib.loads(app_after_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit("generated-workflows: external-edge app manifest is invalid TOML") from error
expected_app = copy.deepcopy(app_before)
expected_app.setdefault("dev-dependencies", {})["syn"] = dependency
if app_after != expected_app:
    raise SystemExit("generated-workflows: external-edge app manifest changed semantics beyond the new syn dev-dependency")
packages = lock_before.get("package")
if type(packages) is not list:
    raise SystemExit("generated-workflows: external-edge control lock has no package inventory")
local_owners = [item for item in packages if item.get("source") is None and item.get("name") == "jackin-console" and item.get("version") == "0.1.0"]
if len(local_owners) != 1 or local_owners[0].get("dependencies", []).count("syn 3.0.9") != 0:
    raise SystemExit("generated-workflows: external-edge owner or preexisting edge is ambiguous")
syn = [item for item in packages if item.get("name") == "syn"]
qualified = [item for item in syn if item.get("version") == "3.0.9" and item.get("source") == "registry+https://github.com/rust-lang/crates.io-index"]
if len(syn) != 3 or len(qualified) != 1:
    raise SystemExit("generated-workflows: external syn 3.0.9 edge does not resolve to the unique reviewed registry record")
lock_text = lock_bytes.decode("utf-8")
blocks = re.split(r"(?=^\[\[package\]\]$)", lock_text, flags=re.M)
owners = [index for index, block in enumerate(blocks) if re.search(r'^name = "jackin-console"$', block, re.M) and re.search(r'^version = "0.1.0"$', block, re.M) and re.search(r'^source = ', block, re.M) is None]
if len(owners) != 1:
    raise SystemExit("generated-workflows: external-edge Cargo.lock owner block is ambiguous")
index = owners[0]
match = re.search(r"(?ms)^(dependencies = \[\n)(.*?)(^\])", blocks[index])
if match is None:
    raise SystemExit("generated-workflows: external-edge owner dependency list is missing")
entries = match.group(2).splitlines()
edge = ' "syn 3.0.9",'
if edge in entries:
    raise SystemExit("generated-workflows: external-edge qualified edge already exists")
entries = sorted(entries + [edge])
replacement = match.group(1) + "\n".join(entries) + "\n]"
blocks[index] = blocks[index][:match.start()] + replacement + blocks[index][match.end():]
lock_after_text = "".join(blocks)
try:
    lock_after = tomllib.loads(lock_after_text)
except tomllib.TOMLDecodeError as error:
    raise SystemExit("generated-workflows: external-edge Cargo.lock is invalid TOML") from error
expected_lock = copy.deepcopy(lock_before)
expected_owner = next(item for item in expected_lock["package"] if item.get("source") is None and item.get("name") == "jackin-console" and item.get("version") == "0.1.0")
expected_owner["dependencies"] = sorted(expected_owner.get("dependencies", []) + ["syn 3.0.9"])
if lock_after != expected_lock:
    raise SystemExit("generated-workflows: external-edge Cargo.lock changed package identities or records beyond one owner edge")
external_before = [item for item in lock_before["package"] if item.get("source") is not None]
external_after = [item for item in lock_after["package"] if item.get("source") is not None]
if external_after != external_before:
    raise SystemExit("generated-workflows: external-edge fixture changed the reviewed external package inventory")
if lock_after["package"].count(expected_owner) != 1:
    raise SystemExit("generated-workflows: external-edge lock owner edge is not unique")
if lock_after["package"] == lock_before["package"] or lock_after_text.encode("utf-8") == seed_bytes:
    raise SystemExit("generated-workflows: external-edge fixture did not change the lock graph and bytes")
app_path.write_text(app_after_text)
lock_path.write_text(lock_after_text)
PY
  then
    return 1
  fi
  policy_velnor_executable=$(policy_verified_velnor_executable) || return 1
  tool_cache_generation=${policy_velnor_executable%%/tool-state/*}
  if ! policy_velnor_version=$(env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C "$policy_velnor_executable" --version 2> "$test_root/policy-velnor-version.stderr"); then
    cat "$test_root/policy-velnor-version.stderr" >&2
    printf 'generated-workflows: verified Velnor executable could not report its version\n' >&2
    return 1
  fi
  if [[ "$policy_velnor_version" != *0.1.78* ]]; then
    printf 'generated-workflows: verified Velnor executable did not report reviewed version 0.1.78\n' >&2
    return 1
  fi
  if ! (cd "$policy_source_repository" && env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 "$policy_velnor_executable" generate --output-dir "$policy_source_repository") > "$test_root/policy-external-generator-first.log" 2>&1; then
    cat "$test_root/policy-external-generator-first.log" >&2
    printf 'generated-workflows: authenticated Velnor 0.1.78 failed to generate the external-edge fixture outputs\n' >&2
    return 1
  fi
  first_output_sha=$(directory_digest "$policy_source_repository/.github") || return 1
  if ! (cd "$policy_source_repository" && env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 "$policy_velnor_executable" generate --output-dir "$policy_source_repository") > "$test_root/policy-external-generator-second.log" 2>&1; then
    cat "$test_root/policy-external-generator-second.log" >&2
    printf 'generated-workflows: authenticated Velnor 0.1.78 failed its repeat generation\n' >&2
    return 1
  fi
  second_output_sha=$(directory_digest "$policy_source_repository/.github") || return 1
  if [[ "$first_output_sha" != "$second_output_sha" ]]; then
    printf 'generated-workflows: external-edge Velnor output changed on the second generation\n' >&2
    return 1
  fi
  policy_external_output_sha=$second_output_sha
  if ! python3 -I -S - "$policy_source_repository" "$policy_control_head" "$changed_paths" "$policy_external_generated_paths" "$policy_external_generated_manifest" <<'PY'
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
control, changed_output, generated_output, manifest_output = sys.argv[2:]
def git(*arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments])
def inventory(directory):
    result = {}
    if not directory.exists() and not directory.is_symlink():
        return result
    for path in [directory, *sorted(directory.rglob("*"))] if directory.is_dir() and not directory.is_symlink() else [directory]:
        info = path.lstat()
        relative = ".github" if path == directory else path.relative_to(root).as_posix()
        value = {"mode": stat.S_IMODE(info.st_mode)}
        if stat.S_ISREG(info.st_mode):
            content = path.read_bytes()
            value.update(kind="file", sha256=hashlib.sha256(content).hexdigest())
        elif stat.S_ISLNK(info.st_mode):
            target = os.readlink(path)
            value.update(kind="symlink", target=target, sha256=hashlib.sha256(os.fsencode(target)).hexdigest())
        elif stat.S_ISDIR(info.st_mode):
            continue
        else:
            raise SystemExit("generated-workflows: Velnor emitted a special file")
        result[relative] = value
    return result

before = {}
listing = git("ls-tree", "-r", "-z", control, "--", ".github").split(b"\0")
for entry in listing:
    if not entry:
        continue
    header, encoded_path = entry.split(b"\t", 1)
    mode, kind, oid = header.split()
    path = os.fsdecode(encoded_path)
    if kind != b"blob":
        raise SystemExit("generated-workflows: control .github tree contains a non-blob entry")
    content = git("cat-file", "blob", os.fsdecode(oid))
    if mode == b"120000":
        before[path] = {"mode": 0o777, "kind": "symlink", "target": os.fsdecode(content), "sha256": hashlib.sha256(content).hexdigest()}
    else:
        before[path] = {"mode": int(mode, 8) & 0o777, "kind": "file", "sha256": hashlib.sha256(content).hexdigest()}
after = inventory(root / ".github")
changed_github = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
tracked = {os.fsdecode(path) for path in git("diff", "--name-only", "-z", control).split(b"\0") if path}
untracked = {os.fsdecode(path) for path in git("ls-files", "--others", "--exclude-standard", "-z").split(b"\0") if path}
ignored = {os.fsdecode(path) for path in git("ls-files", "--others", "--ignored", "--exclude-standard", "-z").split(b"\0") if path}
changed = tracked | untracked | set(changed_github)
expected = {"Cargo.lock", "apps/jackin/Cargo.toml"} | set(changed_github)
if changed != expected:
    raise SystemExit("generated-workflows: external-edge fixture changed paths beyond its two Cargo inputs and exact generated outputs")
if any(not path.startswith(".github/") for path in ignored):
    raise SystemExit("generated-workflows: external-edge fixture created an ignored path outside generated .github outputs")
if any(not path.startswith(".github/") for path in changed_github):
    raise SystemExit("generated-workflows: generated output path escaped .github")
Path(changed_output).write_bytes(b"".join(os.fsencode(path) + b"\0" for path in sorted(changed)))
Path(generated_output).write_bytes(b"".join(os.fsencode(path) + b"\0" for path in changed_github))
records = [{"path": path, "before": before.get(path), "after": after.get(path)} for path in changed_github]
Path(manifest_output).write_text(json.dumps({"paths": records}, sort_keys=True, separators=(",", ":")) + "\n")
PY
  then
    return 1
  fi
  if ! "$git_command" -C "$policy_source_repository" add -f -A -- Cargo.lock apps/jackin/Cargo.toml .github || ! "$git_command" -C "$policy_source_repository" diff --cached --name-only -z "$policy_control_head" -- > "$candidate_paths"; then
    printf 'generated-workflows: cannot stage or inspect the immutable external-edge fixture paths\n' >&2
    return 1
  fi
  if ! python3 -I -S - "$candidate_paths" "$expected_paths" "$changed_paths" <<'PY'
import os
import sys
from pathlib import Path

actual = sorted(item for item in Path(sys.argv[1]).read_bytes().split(b"\0") if item)
changed = sorted(item for item in Path(sys.argv[3]).read_bytes().split(b"\0") if item)
if actual != changed:
    raise SystemExit("generated-workflows: staged external-edge fixture paths differ from its exact semantic diff")
Path(sys.argv[2]).write_bytes(b"".join(item + b"\0" for item in changed))
PY
  then
    return 1
  fi
  if ! "$git_command" -c core.hooksPath=/dev/null -C "$policy_source_repository" commit --quiet -m 'test: qualify reviewed external Cargo edge'; then
    printf 'generated-workflows: cannot seal the immutable external-edge fixture commit\n' >&2
    return 1
  fi
  policy_external_fixture_head=$("$git_command" -C "$policy_source_repository" rev-parse HEAD)
  policy_external_fixture_parent=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_external_fixture_head^1")
  policy_external_fixture_tree=$("$git_command" -C "$policy_source_repository" rev-parse "$policy_external_fixture_head^{tree}")
  if [[ "$policy_external_fixture_parent" != "$policy_control_head" || "$policy_external_fixture_tree" == "$policy_control_tree" || "$("$git_command" -C "$policy_source_repository" rev-parse refs/heads/main)" != "$policy_external_fixture_head" || -n "$("$git_command" -C "$policy_source_repository" status --porcelain)" ]]; then
    printf 'generated-workflows: external-edge fixture commit lost its control parent, changed tree, or clean immutable state\n' >&2
    return 1
  fi
  "$git_command" -C "$policy_source_repository" update-ref refs/qualification/policy-external-edge "$policy_external_fixture_head"
  if ! "$git_command" -C "$policy_source_repository" diff --binary "$policy_control_head" "$policy_external_fixture_head" -- > "$policy_external_fixture_patch"; then
    printf 'generated-workflows: cannot preserve the exact external-edge fixture patch\n' >&2
    return 1
  fi
  diff_status=0
  if /usr/bin/diff -u "$policy_candidate_files/app-Cargo.toml.candidate" "$policy_source_repository/apps/jackin/Cargo.toml" > "$test_root/policy-external-app-manifest.patch"; then
    diff_status=0
  else
    diff_status=$?
  fi
  if [[ "$diff_status" -ne 1 ]]; then
    printf 'generated-workflows: external-edge app manifest semantic patch is absent or unreadable\n' >&2
    return 1
  fi
  {
    printf 'external_fixture_commit=%s\n' "$policy_external_fixture_head"
    printf 'external_fixture_parent=%s\n' "$policy_external_fixture_parent"
    printf 'external_fixture_tree=%s\n' "$policy_external_fixture_tree"
    printf 'external_manifest_addition=apps/jackin:dev-dependencies.syn@3.0.9(full,visit)\n'
    printf 'external_lock_edge_addition=jackin-console 0.1.0->syn 3.0.9\n'
    printf 'external_fixture_patch_sha256=%s\n' "$(sha256_file "$policy_external_fixture_patch")"
    printf 'external_app_manifest_patch_sha256=%s\n' "$(sha256_file "$test_root/policy-external-app-manifest.patch")"
    printf 'external_app_manifest_sha256=%s\n' "$(sha256_file "$policy_source_repository/apps/jackin/Cargo.toml")"
    printf 'external_lock_sha256=%s\n' "$(sha256_file "$policy_source_repository/Cargo.lock")"
    printf 'external_root_manifest_sha256=%s\n' "$(sha256_file "$policy_source_repository/Cargo.toml")"
    printf 'external_trusted_seed_sha256=%s\n' "$(sha256_file "$policy_source_repository/scripts/trusted-seed-cargo.lock")"
    printf 'external_velnor_config_sha256=%s\n' "$(sha256_file "$policy_source_repository/.velnor/config.toml")"
    printf 'external_generated_output_sha256=%s\n' "$policy_external_output_sha"
    printf 'external_generator_repeat_stable=true\n'
    printf 'external_fixture_path_oracle_sha256=%s\n' "$(sha256_file "$changed_paths")"
    printf 'external_generated_paths_sha256=%s\n' "$(sha256_file "$policy_external_generated_paths")"
    printf 'external_generated_manifest_sha256=%s\n' "$(sha256_file "$policy_external_generated_manifest")"
    printf 'external_generator_sha256=%s\n' "$(sha256_file "$policy_velnor_executable")"
    printf 'external_generator_version=%s\n' "$policy_velnor_version"
    if [[ ! -s "$policy_external_generated_paths" ]]; then
      printf 'external_generated_paths=none\n'
    else
      printf 'external_generated_paths='; tr '\0' '\n' < "$policy_external_generated_paths" | paste -sd, -; printf '\n'
    fi
  } >> "$test_root/policy-fixture-context.txt"
}
if [[ "$hash_tool" == /usr/bin/shasum ]]; then
  python_generation=$(printf '%s\n' "$platform" "$(manifest_value python_version)" "$(manifest_value python_artifact_sha256)" "$(manifest_value python_lock_sha256)" "$(manifest_value python_mise_sha256)" "$(manifest_value inventory_helper_sha256)" "$(manifest_value mise_version)" "$(manifest_value mise_sha256)" | "$hash_tool" -a 256 | /usr/bin/awk '{print $1}')
else
  python_generation=$(printf '%s\n' "$platform" "$(manifest_value python_version)" "$(manifest_value python_artifact_sha256)" "$(manifest_value python_lock_sha256)" "$(manifest_value python_mise_sha256)" "$(manifest_value inventory_helper_sha256)" "$(manifest_value mise_version)" "$(manifest_value mise_sha256)" | "$hash_tool" | /usr/bin/awk '{print $1}')
fi
python_artifact_dir="$installation/artifacts/python/3.15.2/$platform-$python_generation"
python_runtime="$python_artifact_dir/runtime"
python_inventory="$python_artifact_dir/inventory"
python_artifact_manifest="$python_artifact_dir/manifest"
python_mise_config="$python_artifact_dir/mise.toml"
python_mise_lock="$python_artifact_dir/mise.lock"
trusted_checker="$installation/versions/$checker_digest/check-generated-workflows.sh"
if [[ ! -f "$trusted_checker" || -L "$trusted_checker" ]]; then
  printf 'generated-workflows: installed trusted checker is unavailable after setup\n' >&2
  exit 1
fi
stat_mode() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%Lp' "$1"
  else
    /usr/bin/stat -c '%a' "$1"
  fi
}
stat_links() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%l' "$1"
  else
    /usr/bin/stat -c '%h' "$1"
  fi
}
run_installed_hooks() {
  if ! (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/installed-pre-commit.log" 2>&1; then
    cat "$test_root/installed-pre-commit.log" >&2
    printf 'generated-workflows: installed pre-commit hook failed after installer case\n' >&2
    return 1
  fi
  if ! (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-push" origin "$remote" < /dev/null) > "$test_root/installed-pre-push.log" 2>&1; then
    cat "$test_root/installed-pre-push.log" >&2
    printf 'generated-workflows: installed pre-push hook failed after installer case\n' >&2
    return 1
  fi
}
save_hook_baseline() {
  cp -p "$hooks_path/pre-commit" "$test_root/pre-commit.before"
  cp -p "$hooks_path/pre-push" "$test_root/pre-push.before"
  saved_pre_commit_mode=$(stat_mode "$hooks_path/pre-commit")
  saved_pre_push_mode=$(stat_mode "$hooks_path/pre-push")
}
restore_hook_baseline() {
  cp -p "$test_root/pre-commit.before" "$hooks_path/pre-commit"
  cp -p "$test_root/pre-push.before" "$hooks_path/pre-push"
}
assert_hooks_match_baseline() {
  if ! cmp -s "$test_root/pre-commit.before" "$hooks_path/pre-commit" || ! cmp -s "$test_root/pre-push.before" "$hooks_path/pre-push" || [[ "$(stat_mode "$hooks_path/pre-commit")" != "$saved_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$saved_pre_push_mode" ]]; then
    printf 'generated-workflows: installer changed a canonical hook unexpectedly\n' >&2
    return 1
  fi
}
if [[ "$e2e_focus" == installer-seed-lock-artifact ]]; then
  run_installed_hooks
  save_hook_baseline
  installed_seed_lock="$installation/versions/$checker_digest/trusted-seed-cargo.lock"
  installed_seed_policy="$installation/versions/$checker_digest/trusted-seed-cargo-policy.json"
  generation_directory="$installation/versions/$checker_digest"
  cp -p "$installed_seed_lock" "$test_root/installed-trusted-seed-cargo.lock.original"
  cp -p "$installed_seed_policy" "$test_root/installed-trusted-seed-cargo-policy.json.original"
  printf 'preserve installed checker artifact control\n' > "$test_root/installed-seed-lock-sentinel"
  installed_index_tree=$("$git_command" -C "$repository" write-tree)
  installed_config_sha=$(sha256_file "$repository/.git/config")
  installed_output_sha=$(directory_digest "$repository/.github")
  installed_receiver_refs=$("$git_command" -C "$remote" show-ref || true)
  installed_sentinel_sha=$(sha256_file "$test_root/installed-seed-lock-sentinel")
  generation_mode_before=$(stat_mode "$generation_directory")
  for artifact_name in seed-lock seed-policy; do
    case "$artifact_name" in
      seed-lock)
        installed_artifact=$installed_seed_lock
        artifact_original="$test_root/installed-trusted-seed-cargo.lock.original"
        ;;
      seed-policy)
        installed_artifact=$installed_seed_policy
        artifact_original="$test_root/installed-trusted-seed-cargo-policy.json.original"
        ;;
    esac
    artifact_cases='missing changed symlink nonregular'
    if [[ "$artifact_name" == seed-policy ]]; then
      artifact_cases='missing changed path-list candidate-bytes symlink nonregular'
    fi
    for artifact_case in $artifact_cases; do
      chmod u+w "$generation_directory"
      rm -rf "$installed_artifact"
      cp -p "$artifact_original" "$installed_artifact"
      expected='installed checker artifacts are unsafe'
      case "$artifact_case" in
        missing)
          rm "$installed_artifact"
          ;;
        changed)
          chmod u+w "$installed_artifact"
          printf 'changed\n' >> "$installed_artifact"
          chmod 400 "$installed_artifact"
          expected='installed checker artifacts failed integrity validation'
          ;;
        path-list)
          chmod u+w "$installed_artifact"
          python3 -I -S - "$installed_artifact" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
policy = json.loads(path.read_text())
policy["manifests"][0]["path"] = "unreviewed/Cargo.toml"
path.write_text(json.dumps(policy, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n")
PY
          chmod 400 "$installed_artifact"
          expected='installed checker artifacts failed integrity validation'
          ;;
        candidate-bytes)
          chmod u+w "$installed_artifact"
          python3 -I -S - "$installed_artifact" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
policy = json.loads(path.read_text())
encoded = policy["manifests"][0]["bytes_base64"]
policy["manifests"][0]["bytes_base64"] = ("A" if encoded[0] != "A" else "B") + encoded[1:]
path.write_text(json.dumps(policy, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n")
PY
          chmod 400 "$installed_artifact"
          expected='installed checker artifacts failed integrity validation'
          ;;
        symlink)
          rm "$installed_artifact"
          ln -s "$artifact_original" "$installed_artifact"
          ;;
        nonregular)
          rm "$installed_artifact"
          mkdir "$installed_artifact"
          ;;
      esac
      chmod "$generation_mode_before" "$generation_directory"
      installer_status=0
      if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/$artifact_name-generation-$artifact_case.log" 2>&1; then
        installer_status=0
      else
        installer_status=$?
      fi
      if [[ "$installer_status" -eq 0 ]] || ! /usr/bin/grep -F -q "$expected" "$test_root/$artifact_name-generation-$artifact_case.log" || [[ "$("$git_command" -C "$repository" write-tree)" != "$installed_index_tree" || "$(sha256_file "$repository/.git/config")" != "$installed_config_sha" || "$(directory_digest "$repository/.github")" != "$installed_output_sha" || "$("$git_command" -C "$remote" show-ref || true)" != "$installed_receiver_refs" || "$(sha256_file "$test_root/installed-seed-lock-sentinel")" != "$installed_sentinel_sha" ]]; then
        cat "$test_root/$artifact_name-generation-$artifact_case.log" >&2
        printf 'generated-workflows: invalid installed trusted seed Cargo artifact changed qualification state\n' >&2
        exit 1
      fi
      chmod u+w "$generation_directory"
      rm -rf "$installed_artifact"
      cp -p "$artifact_original" "$installed_artifact"
      chmod 400 "$installed_artifact"
      chmod "$generation_mode_before" "$generation_directory"
      assert_hooks_match_baseline
    done
  done
  run_installed_hooks
  assert_hooks_match_baseline
  printf 'source and installed trusted seed Cargo artifact rejection matrix: PASS\n'
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == cargo-local-lock-policy ]]; then
  run_installed_hooks
  policy_baseline_tree=$("$git_command" -C "$repository" rev-parse "$source_head^{tree}")
  if [[ "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_baseline_tree" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$source_head" || -n "$("$git_command" -C "$repository" status --porcelain)" ]]; then
    printf 'generated-workflows: baseline hooks did not preserve the selected policy checkout and refs\n' >&2
    exit 1
  fi
  {
    if [[ "$source_head" == "$policy_control_head" ]]; then
      printf 'baseline_name=control-initial\n'
    else
      printf 'baseline_name=candidate-initial\n'
    fi
    printf 'baseline_commit=%s\n' "$source_head"
    printf 'baseline_tree=%s\n' "$policy_baseline_tree"
    printf 'pre_commit_log_sha256=%s\n' "$(sha256_file "$test_root/installed-pre-commit.log")"
    printf 'pre_push_log_sha256=%s\n' "$(sha256_file "$test_root/installed-pre-push.log")"
    printf 'pre_commit_hook_sha256=%s\n' "$(sha256_file "$hooks_path/pre-commit")"
    printf 'pre_push_hook_sha256=%s\n' "$(sha256_file "$hooks_path/pre-push")"
  } > "$test_root/policy-hook-initial.log"
  cp -p "$repository/Cargo.lock" "$test_root/cargo-lock.control"
  cp -p "$repository/Cargo.toml" "$test_root/root-cargo-manifest.control"
  cp -p "$repository/apps/jackin/Cargo.toml" "$test_root/app-cargo-manifest.control"
  printf 'preserve policy fixture state\n' > "$test_root/policy-sentinel"
  assert_policy_source_unchanged
  policy_config_sha=$(sha256_file "$repository/.git/config")
  policy_output_sha=$(directory_digest "$repository/.github")
  policy_hook_commit_sha=$(sha256_file "$hooks_path/pre-commit")
  policy_hook_push_sha=$(sha256_file "$hooks_path/pre-push")
  policy_sentinel_sha=$(sha256_file "$test_root/policy-sentinel")
  policy_initial_receiver_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort || true)
  if [[ "$policy_case" == all || "$policy_case" == positive-external-edge || "$policy_case" == positive-pair ]]; then
    prepare_external_edge_fixture || exit 1
    if [[ "$policy_external_fixture_parent" != "$policy_control_head" || "$policy_external_fixture_tree" != "$("$git_command" -C "$policy_source_repository" rev-parse "$policy_external_fixture_head^{tree}")" ]]; then
      printf 'generated-workflows: external-edge fixture lineage changed after immutable preparation\n' >&2
      exit 1
    fi
    assert_policy_source_unchanged
  fi
  mutate_policy_fixture() {
    case "$1" in
      feature-unrelated-removal|feature-unrelated-token|feature-new-key|feature-similar-key|feature-edge-retained|feature-lock-only-removal)
        prepare_local_local_removal_fixture "$repository" || return 1
        case "$1" in
          feature-edge-retained) cp -p "$test_root/cargo-lock.control" "$repository/Cargo.lock" ;;
          feature-lock-only-removal) cp -p "$test_root/app-cargo-manifest.control" "$repository/apps/jackin/Cargo.toml" ;;
        esac
        ;;
    esac
    python3 -I -S - "$repository" "$test_root" "$1" <<'PY'
import re
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
evidence = Path(sys.argv[2])
case = sys.argv[3]
lock_path = root / "Cargo.lock"
manifest_path = root / "apps/jackin/Cargo.toml"
text = lock_path.read_text()
manifest_text = manifest_path.read_text()

def package_blocks(value):
    return re.split(r"(?=^\[\[package\]\]$)", value, flags=re.M)

def selected_package(blocks, name, predicate=None):
    matches = []
    for index, block in enumerate(blocks):
        if re.search(rf'^name = "{re.escape(name)}"$', block, re.M) and (predicate is None or predicate(block)):
            matches.append(index)
    if len(matches) != 1:
        raise SystemExit(f"generated-workflows: {name} lock fixture is ambiguous")
    return matches[0]

def external_package(blocks, predicate=None):
    matches = []
    for index, block in enumerate(blocks):
        if re.search(r'^source = "registry\+[^\"]+"$', block, re.M) and (predicate is None or predicate(block)):
            matches.append(index)
    if not matches:
        raise SystemExit("generated-workflows: external lock fixture is missing")
    return matches[0]

def replace_package_dependencies(value, name, transform):
    blocks = package_blocks(value)
    index = selected_package(blocks, name)
    block = blocks[index]
    match = re.search(r"(?ms)^(dependencies = \[\n)(.*?)(^\])", block)
    if match is None:
        raise SystemExit(f"generated-workflows: {name} dependency list is missing")
    lines = match.group(2).splitlines()
    updated = transform(lines)
    replacement = match.group(1) + "\n".join(updated) + "\n]"
    blocks[index] = block[:match.start()] + replacement + block[match.end():]
    return "".join(blocks)

def add_dependency(lines, dependency):
    entry = f' "{dependency}",'
    if entry in lines:
        raise SystemExit("generated-workflows: policy dependency fixture already exists")
    return sorted(lines + [entry])

if case == "external-checksum":
    blocks = package_blocks(text)
    index = external_package(blocks, lambda block: re.search(r'^checksum = "[0-9a-f]{64}"$', block, re.M) is not None)
    block = blocks[index]
    match = re.search(r'^checksum = "([0-9a-f])([0-9a-f]{63})"$', block, re.M)
    if match is None:
        raise SystemExit("generated-workflows: checksum fixture is missing")
    block = block[:match.start(1)] + ("0" if match.group(1) != "0" else "1") + block[match.end(1):]
    blocks[index] = block
    text = "".join(blocks)
elif case == "external-source":
    blocks = package_blocks(text)
    index = external_package(blocks)
    block = blocks[index]
    match = re.search(r'^source = "registry\+[^\"]+"$', block, re.M)
    if match is None:
        raise SystemExit("generated-workflows: source fixture is missing")
    blocks[index] = block[:match.start()] + 'source = "registry+https://example.invalid/index"' + block[match.end():]
    text = "".join(blocks)
elif case == "external-version":
    blocks = package_blocks(text)
    index = external_package(blocks)
    block = blocks[index]
    match = re.search(r'^version = "[^"]+"$', block, re.M)
    if match is None:
        raise SystemExit("generated-workflows: version fixture is missing")
    blocks[index] = block[:match.start()] + 'version = "99.0.0"' + block[match.end():]
    text = "".join(blocks)
elif case == "external-dependency-graph":
    blocks = package_blocks(text)
    index = external_package(blocks, lambda block: re.search(r"(?ms)^dependencies = \[\n.+?^\]", block) is not None)
    block = blocks[index]
    match = re.search(r"(?ms)^(dependencies = \[\n)(.*?)(^\])", block)
    lines = match.group(2).splitlines()
    if not lines:
        raise SystemExit("generated-workflows: external dependency graph fixture is empty")
    replacement = match.group(1) + "\n".join(lines[1:]) + "\n]"
    blocks[index] = block[:match.start()] + replacement + block[match.end():]
    text = "".join(blocks)
elif case == "unknown-local-package":
    text += '\n[[package]]\nname = "policy-local-fixture"\nversion = "0.1.0"\ndependencies = []\n'
elif case == "forged-source-less-entry":
    blocks = package_blocks(text)
    index = external_package(blocks)
    block = blocks[index]
    block, source_count = re.subn(r'^source = "registry\+[^\"]+"\n', "", block, count=1, flags=re.M)
    block, checksum_count = re.subn(r'^checksum = "[0-9a-f]{64}"\n', "", block, count=1, flags=re.M)
    if source_count != 1 or checksum_count != 1:
        raise SystemExit("generated-workflows: forged local fixture fields are missing")
    blocks[index] = block
    text = "".join(blocks)
elif case == "duplicate-package-identity":
    blocks = package_blocks(text)
    index = selected_package(blocks, "junie-tui")
    text += "\n" + blocks[index]
elif case == "ambiguous-unqualified-edge":
    text = replace_package_dependencies(text, "junie-tui", lambda lines: add_dependency(lines, "syn"))
elif case == "duplicate-edge":
    text = replace_package_dependencies(text, "junie-tui", lambda lines: lines + [' "unicode-segmentation",'])
elif case == "unknown-lock-metadata":
    anchor = "version = 4\n"
    if text.count(anchor) != 1:
        raise SystemExit("generated-workflows: lock metadata fixture anchor is ambiguous")
    text = text.replace(anchor, anchor + "unreviewed = true\n", 1)
elif case == "unknown-package-metadata":
    blocks = package_blocks(text)
    index = selected_package(blocks, "junie-tui")
    block = blocks[index]
    anchor = 'version = "0.1.0"\n'
    if block.count(anchor) != 1:
        raise SystemExit("generated-workflows: package metadata fixture anchor is ambiguous")
    blocks[index] = block.replace(anchor, anchor + "unreviewed = true\n", 1)
    text = "".join(blocks)
elif case == "lock-version-float":
    anchor = "version = 4\n"
    if text.count(anchor) != 1:
        raise SystemExit("generated-workflows: lock version type fixture anchor is ambiguous")
    text = text.replace(anchor, "version = 4.0\n", 1)
elif case == "missing-external-record":
    blocks = package_blocks(text)
    index = external_package(blocks)
    del blocks[index]
    text = "".join(blocks)
elif case == "unresolved-local-edge":
    text = replace_package_dependencies(text, "jackin-console", lambda lines: add_dependency(lines, "unresolved-policy-fixture"))
elif case == "malformed-lock":
    text += "\n[broken\n"
elif case in {"nested-lock-symlink", "nested-lock-changed"}:
    nested = root / "policy-nested"
    nested.mkdir()
    if case == "nested-lock-symlink":
        (nested / "Cargo.lock").symlink_to("../Cargo.lock")
    else:
        data = (root / "scripts" / "trusted-seed-cargo.lock").read_bytes()
        (nested / "Cargo.lock").write_bytes(data + b"\n")
elif case == "root-lock-symlink":
    target = root / "policy-lock-target"
    target.write_bytes(lock_path.read_bytes())
    lock_path.unlink()
    lock_path.symlink_to(target.name)
elif case == "path-escape":
    value = manifest_text
    anchor = 'license = "MIT"\n'
    if value.count(anchor) != 1:
        raise SystemExit("generated-workflows: path escape fixture anchor is ambiguous")
    manifest_text = value.replace(anchor, anchor + 'include = ["../../../outside/**"]\n', 1)
elif case == "typed-workspace-boolean-int":
    anchor = 'clap = { workspace = true, optional = true }'
    if manifest_text.count(anchor) != 1:
        raise SystemExit("generated-workflows: workspace boolean type fixture anchor is ambiguous")
    manifest_text = manifest_text.replace(anchor, 'clap = { workspace = 1, optional = true }', 1)
elif case == "typed-dependency-boolean-int":
    anchor = 'clap = { workspace = true, optional = true }'
    if manifest_text.count(anchor) != 1:
        raise SystemExit("generated-workflows: nested dependency type fixture anchor is ambiguous")
    manifest_text = manifest_text.replace(anchor, 'clap = { workspace = true, optional = 0 }', 1)
elif case == "typed-protected-boolean-int":
    anchor = "doctest = false\n"
    if manifest_text.count(anchor) != 1:
        raise SystemExit("generated-workflows: protected boolean type fixture anchor is ambiguous")
    manifest_text = manifest_text.replace(anchor, "doctest = 0\n", 1)
elif case == "feature-only-removal":
    parsed = tomllib.loads(manifest_text)
    packages = tomllib.loads(text)["package"]
    owners = [item for item in packages if item.get("name") == "jackin-console" and item.get("source") is None]
    anchor = 'demo = ["dep:clap", "junie-tui/cli"]\n'
    if parsed.get("dependencies", {}).get("junie-tui") != {"path": "../..", "default-features": False, "features": ["terminal"]} or parsed.get("features", {}).get("demo") != ["dep:clap", "junie-tui/cli"] or len(owners) != 1 or owners[0].get("dependencies", []).count("junie-tui") != 1 or manifest_text.count(anchor) != 1:
        raise SystemExit("generated-workflows: feature-only-removal precondition does not retain the exact dependency and lock edge")
    manifest_text = manifest_text.replace(anchor, 'demo = ["dep:clap"]\n', 1)
elif case in {"feature-unrelated-removal", "feature-unrelated-token", "feature-new-key", "feature-similar-key"}:
    parsed = tomllib.loads(manifest_text)
    packages = tomllib.loads(text)["package"]
    owners = [item for item in packages if item.get("name") == "jackin-console" and item.get("source") is None]
    anchor = 'demo = ["dep:clap"]\n'
    if "junie-tui" in parsed.get("dependencies", {}) or parsed.get("features", {}).get("demo") != ["dep:clap"] or len(owners) != 1 or owners[0].get("dependencies", []).count("junie-tui") != 0 or manifest_text.count(anchor) != 1:
        raise SystemExit("generated-workflows: coupled feature mutation precondition lacks the exact local dependency and lock-edge removal")
    if case == "feature-unrelated-removal":
        manifest_text = manifest_text.replace(anchor, 'demo = []\n', 1)
    elif case == "feature-unrelated-token":
        manifest_text = manifest_text.replace(anchor, 'demo = ["dep:clap", "default"]\n', 1)
    elif case == "feature-new-key":
        feature_section = "[features]\n"
        if manifest_text.count(feature_section) != 1:
            raise SystemExit("generated-workflows: feature key fixture section is ambiguous")
        manifest_text = manifest_text.replace(feature_section, feature_section + 'unreviewed = []\n', 1)
    else:
        manifest_text = manifest_text.replace(anchor, 'demo = ["dep:clap", "junie-tui-extra/cli"]\n', 1)
elif case == "feature-edge-retained":
    parsed = tomllib.loads(manifest_text)
    packages = tomllib.loads(text)["package"]
    owners = [item for item in packages if item.get("name") == "jackin-console" and item.get("source") is None]
    if "junie-tui" in parsed.get("dependencies", {}) or parsed.get("features", {}).get("demo") != ["dep:clap"] or len(owners) != 1 or owners[0].get("dependencies", []).count("junie-tui") != 1:
        raise SystemExit("generated-workflows: retained-edge fixture precondition does not remove only the manifest dependency")
elif case == "feature-lock-only-removal":
    parsed = tomllib.loads(manifest_text)
    packages = tomllib.loads(text)["package"]
    owners = [item for item in packages if item.get("name") == "jackin-console" and item.get("source") is None]
    if parsed.get("dependencies", {}).get("junie-tui") != {"path": "../..", "default-features": False, "features": ["terminal"]} or parsed.get("features", {}).get("demo") != ["dep:clap", "junie-tui/cli"] or len(owners) != 1 or owners[0].get("dependencies", []).count("junie-tui") != 0:
        raise SystemExit("generated-workflows: lock-only fixture precondition does not remove only the local lock edge")
else:
    raise SystemExit("generated-workflows: unsupported policy fixture")
if case not in {"nested-lock-symlink", "nested-lock-changed", "root-lock-symlink", "path-escape"}:
    lock_path.write_text(text)
if manifest_text != manifest_path.read_text():
    manifest_path.write_text(manifest_text)
PY
  }
  policy_expected_diagnostic() {
    case "$1" in
      external-checksum|external-dependency-graph) printf '%s' 'Cargo.lock changes a reviewed external package record' ;;
      external-source|external-version|unknown-local-package|forged-source-less-entry|missing-external-record) printf '%s' 'Cargo.lock changes the reviewed package inventory' ;;
      duplicate-package-identity) printf '%s' 'snapshot Cargo.lock contains an ambiguous package identity' ;;
      ambiguous-unqualified-edge|unresolved-local-edge) printf '%s' 'snapshot Cargo.lock contains an unresolved or ambiguous dependency identity' ;;
      duplicate-edge) printf '%s' 'snapshot Cargo.lock contains a duplicate dependency edge' ;;
      unknown-lock-metadata) printf '%s' 'snapshot Cargo.lock has unsupported lock metadata' ;;
      unknown-package-metadata) printf '%s' 'snapshot Cargo.lock contains unsupported package metadata' ;;
      lock-version-float) printf '%s' 'snapshot Cargo.lock has unsupported lock metadata' ;;
      malformed-lock) printf '%s' 'snapshot Cargo.lock is invalid TOML' ;;
      root-lock-symlink) printf '%s' 'snapshot Cargo.lock is not a regular file' ;;
      nested-lock-symlink) printf '%s' 'nested Cargo.lock is not a regular file' ;;
      nested-lock-changed) printf '%s' 'Cargo.lock is outside the reviewed dependency support policy' ;;
      path-escape) printf '%s' 'apps/jackin/Cargo.toml.package.include escapes the snapshot' ;;
      typed-workspace-boolean-int|typed-dependency-boolean-int) printf '%s' 'Cargo manifests change an existing dependency declaration' ;;
      typed-protected-boolean-int) printf '%s' 'Cargo manifests change protected nondependency values' ;;
      feature-only-removal|feature-unrelated-token|feature-similar-key) printf '%s' 'Cargo manifests change feature values outside a removed local dependency reference' ;;
      feature-new-key) printf '%s' 'Cargo manifests change protected feature keys' ;;
      feature-edge-retained|feature-lock-only-removal) printf '%s' 'Cargo.lock local edge changes do not match the selected manifest dependency changes' ;;
      *) printf 'generated-workflows: no diagnostic is defined for policy case %s\n' "$1" >&2; return 1 ;;
    esac
  }
  switch_policy_baseline_to_control() {
    if [[ "$source_head" == "$policy_control_head" ]]; then
      return 0
    fi
    policy_fixture_advance_to_control || return 1
    run_installed_hooks
    if [[ "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_baseline_tree" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" || -n "$("$git_command" -C "$repository" status --porcelain)" ]]; then
      printf 'generated-workflows: control hooks did not preserve the isolated external-edge baseline\n' >&2
      return 1
    fi
    record_policy_hook_baseline control-after-switch
    assert_policy_state
    assert_policy_source_unchanged
  }
  add_policy_fixture_to_index() {
    "$git_command" -C "$repository" add -A -- Cargo.lock Cargo.toml apps/jackin/Cargo.toml
    if [[ -e "$repository/policy-nested" || -L "$repository/policy-nested" ]]; then
      "$git_command" -C "$repository" add -A -- policy-nested
    fi
    if [[ -e "$repository/policy-lock-target" || -L "$repository/policy-lock-target" ]]; then
      "$git_command" -C "$repository" add -A -- policy-lock-target
    fi
  }
  assert_policy_state() {
    local expected_output_sha=${1:-$policy_output_sha}
    if [[ "$(sha256_file "$repository/.git/config")" != "$policy_config_sha" || "$(directory_digest "$repository/.github")" != "$expected_output_sha" || "$(sha256_file "$hooks_path/pre-commit")" != "$policy_hook_commit_sha" || "$(sha256_file "$hooks_path/pre-push")" != "$policy_hook_push_sha" || "$(sha256_file "$test_root/policy-sentinel")" != "$policy_sentinel_sha" ]]; then
      printf 'generated-workflows: policy fixture changed configuration, generated output, hooks, or sentinel state\n' >&2
      exit 1
    fi
  }
  record_policy_hook_baseline() {
    local baseline_name=$1
    {
      printf 'baseline_name=%s\n' "$baseline_name"
      printf 'baseline_commit=%s\n' "$source_head"
      printf 'baseline_tree=%s\n' "$policy_baseline_tree"
      printf 'pre_commit_log_sha256=%s\n' "$(sha256_file "$test_root/installed-pre-commit.log")"
      printf 'pre_push_log_sha256=%s\n' "$(sha256_file "$test_root/installed-pre-push.log")"
      printf 'pre_commit_hook_sha256=%s\n' "$(sha256_file "$hooks_path/pre-commit")"
      printf 'pre_push_hook_sha256=%s\n' "$(sha256_file "$hooks_path/pre-push")"
    } > "$test_root/policy-hook-$baseline_name.log"
  }
  record_policy_case_baseline() {
    local case_name=$1
    local baseline_refs
    if [[ "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_baseline_tree" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$source_head" || -n "$("$git_command" -C "$repository" status --porcelain)" ]]; then
      printf 'generated-workflows: %s did not start from its declared policy baseline\n' "$case_name" >&2
      return 1
    fi
    baseline_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
    {
      printf 'case=%s\n' "$case_name"
      printf 'baseline_commit=%s\n' "$source_head"
      printf 'baseline_tree=%s\n' "$policy_baseline_tree"
      printf 'candidate_commit=%s\n' "$policy_candidate_head"
      printf 'candidate_tree=%s\n' "$policy_candidate_tree"
      printf 'root_manifest_sha256=%s\n' "$(sha256_file "$repository/Cargo.toml")"
      printf 'app_manifest_sha256=%s\n' "$(sha256_file "$repository/apps/jackin/Cargo.toml")"
      printf 'lock_sha256=%s\n' "$(sha256_file "$repository/Cargo.lock")"
      printf 'receiver_refs_sha256=%s\n' "$(printf '%s\n' "$baseline_refs" | sha256_stdin)"
    } > "$test_root/policy-baseline-$case_name.log"
  }
  is_typed_policy_case() {
    case "$1" in
      typed-workspace-boolean-int|typed-dependency-boolean-int|typed-protected-boolean-int|lock-version-float) return 0 ;;
      *) return 1 ;;
    esac
  }
  clear_policy_acquisition_sentinels() {
    rm -f "$test_root/policy-acquisition-sentinels"/*.reached
  }
  assert_no_policy_acquisition_sentinels() {
    local service
    for service in http rustup-http mise-lock mise-install mise cargo-fetch cargo-metadata; do
      if [[ -e "$test_root/policy-acquisition-sentinels/$service.reached" ]]; then
        printf 'generated-workflows: typed Cargo policy rejection reached the %s acquisition sentinel\n' "$service" >&2
        return 1
      fi
    done
  }
  assert_policy_acquisition_sentinel() {
    local log_path=$1
    local count=0
    local service
    for service in http rustup-http mise-lock mise-install mise cargo-fetch cargo-metadata; do
      if [[ -f "$test_root/policy-acquisition-sentinels/$service.reached" ]]; then
        count=$((count + 1))
        if ! /usr/bin/grep -F -q "generated-workflows: test acquisition sentinel reached: $service" "$log_path"; then
          printf 'generated-workflows: acquisition sentinel output does not identify %s\n' "$service" >&2
          return 1
        fi
      fi
    done
    if [[ "$count" -ne 1 ]]; then
      printf 'generated-workflows: old-comparison mutant reached %s acquisition sentinels, expected exactly one\n' "$count" >&2
      return 1
    fi
  }
  run_policy_rejection() {
    local policy_name=$1
    local boundary=$2
    local expected
    local staged_tree
    local status
    local policy_branch
    local candidate_oid=
    local candidate_tree=
    local boundary_index_tree
    local boundary_head
    local boundary_branch
    local boundary_receiver_refs=
    local boundary_outcome=rejected
    local expect_admission=0
    local expect_acquisition_sentinel=0
    local baseline_lock_sha256
    local expected_receiver_refs=
    local actual_receiver_refs=
    local candidate_receiver_present=0
    local expected_mutated_path=
    expected=$(policy_expected_diagnostic "$policy_name")
    case "$public_mutant:$policy_name" in
      external-record:external-checksum|unknown-field:unknown-lock-metadata|path-policy:path-escape|feature-coupling:feature-only-removal) expect_admission=1 ;;
      typed-toml-equality:typed-workspace-boolean-int) expect_acquisition_sentinel=1 ;;
    esac
    reset_policy_fixture
    record_policy_case_baseline "$policy_name-$boundary"
    baseline_lock_sha256=$(sha256_file "$repository/Cargo.lock")
    mutate_policy_fixture "$policy_name"
    if [[ ( "$policy_name" == typed-workspace-boolean-int || "$policy_name" == typed-dependency-boolean-int || "$policy_name" == typed-protected-boolean-int ) && "$(sha256_file "$repository/Cargo.lock")" != "$baseline_lock_sha256" ]]; then
      printf 'generated-workflows: %s typed manifest fixture changed Cargo.lock bytes\n' "$policy_name" >&2
      exit 1
    fi
    add_policy_fixture_to_index
    case "$policy_name" in
      typed-workspace-boolean-int|typed-dependency-boolean-int|typed-protected-boolean-int) expected_mutated_path=apps/jackin/Cargo.toml ;;
      lock-version-float) expected_mutated_path=Cargo.lock ;;
    esac
    if [[ -n "$expected_mutated_path" && "$("$git_command" -C "$repository" diff --cached --name-only "$source_head")" != "$expected_mutated_path" ]]; then
      printf 'generated-workflows: %s typed fixture changed paths beyond its intended input\n' "$policy_name" >&2
      exit 1
    fi
    staged_tree=$("$git_command" -C "$repository" write-tree)
    if [[ "$policy_name" == typed-workspace-boolean-int || "$policy_name" == typed-dependency-boolean-int || "$policy_name" == typed-protected-boolean-int || "$policy_name" == lock-version-float ]]; then
      clear_policy_acquisition_sentinels
    fi
    if [[ "$boundary" == pre-commit ]]; then
      if is_typed_policy_case "$policy_name"; then
        : > "$test_root/policy-acquisition-armed"
      fi
      status=0
      if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/policy-$policy_name-pre-commit.log" 2>&1; then
        status=0
      else
        status=$?
      fi
      rm -f "$test_root/policy-acquisition-armed"
      if [[ "$expect_acquisition_sentinel" == 1 ]]; then
        assert_policy_acquisition_sentinel "$test_root/policy-$policy_name-pre-commit.log" || {
          cat "$test_root/policy-$policy_name-pre-commit.log" >&2
          exit 1
        }
      elif is_typed_policy_case "$policy_name"; then
        assert_no_policy_acquisition_sentinels || {
          cat "$test_root/policy-$policy_name-pre-commit.log" >&2
          exit 1
        }
      fi
      if [[ "$expect_acquisition_sentinel" == 1 ]]; then
        if [[ "$status" -eq 0 || "$("$git_command" -C "$repository" write-tree)" != "$staged_tree" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" ]]; then
          cat "$test_root/policy-$policy_name-pre-commit.log" >&2
          printf 'generated-workflows: old-comparison mutant did not reach an acquisition sentinel at pre-commit\n' >&2
          exit 1
        fi
        boundary_outcome=typed-equality-mutant-reached-acquisition-sentinel
      elif [[ "$expect_admission" == 1 ]]; then
        if [[ "$status" -ne 0 || "$("$git_command" -C "$repository" write-tree)" != "$staged_tree" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" ]]; then
          cat "$test_root/policy-$policy_name-pre-commit.log" >&2
          printf 'generated-workflows: matched %s guard mutant did not admit %s at pre-commit with the staged index preserved\n' "$public_mutant" "$policy_name" >&2
          exit 1
        fi
        boundary_outcome=matched-guard-mutant-admitted
      else
        if [[ "$status" -eq 0 ]] || ! /usr/bin/grep -F -q "$expected" "$test_root/policy-$policy_name-pre-commit.log" || [[ "$("$git_command" -C "$repository" write-tree)" != "$staged_tree" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" ]]; then
          cat "$test_root/policy-$policy_name-pre-commit.log" >&2
          printf 'generated-workflows: pre-commit did not reject %s at its Cargo policy boundary\n' "$policy_name" >&2
          exit 1
        fi
      fi
      boundary_index_tree=$staged_tree
    else
      policy_branch="policy-push-$policy_name"
      "$git_command" -C "$repository" switch -c "$policy_branch" "$source_head" >/dev/null
      if ! (cd "$repository" && HOME="$home_dir" "$git_command" -c core.hooksPath=/dev/null commit -m "test: prepare $policy_name pre-push snapshot") > "$test_root/policy-$policy_name-commit-fixture.log" 2>&1; then
        cat "$test_root/policy-$policy_name-commit-fixture.log" >&2
        printf 'generated-workflows: cannot prepare the non-HEAD %s fixture\n' "$policy_name" >&2
        exit 1
      fi
      candidate_oid=$("$git_command" -C "$repository" rev-parse HEAD)
      candidate_tree=$("$git_command" -C "$repository" rev-parse "$candidate_oid^{tree}")
      if [[ "$candidate_tree" != "$staged_tree" || "$("$git_command" -C "$repository" rev-parse "$candidate_oid^1")" != "$source_head" ]]; then
        printf 'generated-workflows: %s pre-push commit does not preserve its intentionally staged mutation tree\n' "$policy_name" >&2
        exit 1
      fi
      "$git_command" -C "$repository" switch main >/dev/null
      if [[ "$candidate_oid" == "$("$git_command" -C "$repository" rev-parse HEAD)" ]]; then
        printf 'generated-workflows: %s pre-push fixture is unexpectedly HEAD\n' "$policy_name" >&2
        exit 1
      fi
      if is_typed_policy_case "$policy_name"; then
        : > "$test_root/policy-acquisition-armed"
      fi
      status=0
      if (cd "$repository" && HOME="$home_dir" "$git_command" push origin "$policy_branch:refs/heads/$policy_branch") > "$test_root/policy-$policy_name-pre-push.log" 2>&1; then
        status=0
      else
        status=$?
      fi
      rm -f "$test_root/policy-acquisition-armed"
      if [[ "$expect_acquisition_sentinel" == 1 ]]; then
        assert_policy_acquisition_sentinel "$test_root/policy-$policy_name-pre-push.log" || {
          cat "$test_root/policy-$policy_name-pre-push.log" >&2
          exit 1
        }
      elif is_typed_policy_case "$policy_name"; then
        assert_no_policy_acquisition_sentinels || {
          cat "$test_root/policy-$policy_name-pre-push.log" >&2
          exit 1
        }
      fi
      if [[ "$expect_acquisition_sentinel" == 1 ]]; then
        if "$git_command" -C "$remote" show-ref --verify --quiet "refs/heads/$policy_branch"; then
          candidate_receiver_present=1
        fi
        if [[ "$status" -eq 0 || "$candidate_receiver_present" -ne 0 || "$("$git_command" -C "$remote" show-ref || true)" != "$policy_initial_receiver_refs" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" rev-parse "$policy_branch")" != "$candidate_oid" ]]; then
          cat "$test_root/policy-$policy_name-pre-push.log" >&2
          printf 'generated-workflows: old-comparison mutant did not reach an acquisition sentinel at non-HEAD pre-push\n' >&2
          exit 1
        fi
        boundary_outcome=typed-equality-mutant-reached-acquisition-sentinel
      elif [[ "$expect_admission" == 1 ]]; then
        expected_receiver_refs=$(printf '%s\n%s refs/heads/%s\n' "$policy_initial_receiver_refs" "$candidate_oid" "$policy_branch" | LC_ALL=C sort)
        actual_receiver_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
        if [[ "$status" -ne 0 || "$actual_receiver_refs" != "$expected_receiver_refs" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$source_head" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" rev-parse "refs/heads/$policy_branch")" != "$candidate_oid" ]]; then
          cat "$test_root/policy-$policy_name-pre-push.log" >&2
          printf 'generated-workflows: matched %s guard mutant did not admit %s at non-HEAD pre-push with one exact receiver ref update\n' "$public_mutant" "$policy_name" >&2
          exit 1
        fi
        boundary_outcome=matched-guard-mutant-admitted
        boundary_receiver_refs=$actual_receiver_refs
        if ! "$git_command" -C "$remote" update-ref -d "refs/heads/$policy_branch" "$candidate_oid"; then
          printf 'generated-workflows: cannot clean the private receiver after recording matched mutant admission\n' >&2
          exit 1
        fi
        if [[ "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" ]]; then
          printf 'generated-workflows: matched mutant admission cleanup changed unrelated receiver refs\n' >&2
          exit 1
        fi
      else
        if [[ "$status" -eq 0 ]] || ! /usr/bin/grep -F -q "$expected" "$test_root/policy-$policy_name-pre-push.log" || "$git_command" -C "$remote" show-ref --verify --quiet "refs/heads/$policy_branch" || [[ "$("$git_command" -C "$remote" show-ref || true)" != "$policy_initial_receiver_refs" ]]; then
          cat "$test_root/policy-$policy_name-pre-push.log" >&2
          printf 'generated-workflows: non-HEAD pre-push did not reject %s at its Cargo policy boundary\n' "$policy_name" >&2
          exit 1
        fi
      fi
      boundary_index_tree=$("$git_command" -C "$repository" write-tree)
      [[ -n "$boundary_receiver_refs" ]] || boundary_receiver_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
      "$git_command" -C "$repository" branch -D "$policy_branch" >/dev/null
    fi
    boundary_head=$("$git_command" -C "$repository" rev-parse HEAD)
    boundary_branch=$("$git_command" -C "$repository" symbolic-ref --short HEAD)
    [[ -n "$boundary_receiver_refs" ]] || boundary_receiver_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
    assert_policy_state
    assert_policy_source_unchanged
    reset_policy_fixture
    if [[ "$("$git_command" -C "$repository" write-tree)" != "$policy_baseline_tree" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)" != "$policy_initial_receiver_refs" ]]; then
      printf 'generated-workflows: %s fixture cleanup changed the source index tree\n' "$policy_name" >&2
      exit 1
    fi
    assert_policy_source_unchanged
    {
      printf 'case=%s\n' "$policy_name"
      printf 'boundary=%s\n' "$boundary"
      printf 'boundary_outcome=%s\n' "$boundary_outcome"
      printf 'expected_diagnostic=%s\n' "$expected"
      printf 'boundary_status=%s\n' "$status"
      printf 'intentionally_staged_mutation_tree=%s\n' "$staged_tree"
      printf 'boundary_index_tree=%s\n' "$boundary_index_tree"
      printf 'non_head_candidate_commit=%s\n' "${candidate_oid:-none}"
      printf 'non_head_candidate_tree=%s\n' "${candidate_tree:-none}"
      printf 'boundary_head=%s\n' "$boundary_head"
      printf 'boundary_branch=%s\n' "$boundary_branch"
      printf 'boundary_receiver_refs_sha256=%s\n' "$(printf '%s\n' "$boundary_receiver_refs" | sha256_stdin)"
      printf 'reset_index_tree=%s\n' "$policy_baseline_tree"
      printf 'reset_head=%s\n' "$source_head"
      printf 'reset_branch=main\n'
      printf 'reset_receiver_refs_sha256=%s\n' "$(printf '%s\n' "$policy_initial_receiver_refs" | sha256_stdin)"
      printf 'git_config_sha256=%s\n' "$policy_config_sha"
      printf 'generated_output_sha256=%s\n' "$policy_output_sha"
      printf 'pre_commit_hook_sha256=%s\n' "$policy_hook_commit_sha"
      printf 'pre_push_hook_sha256=%s\n' "$policy_hook_push_sha"
      printf 'sentinel_sha256=%s\n' "$policy_sentinel_sha"
      printf 'source_index_tree=%s\n' "$policy_source_index_tree"
      printf 'source_status_sha256=%s\n' "$policy_source_status_sha"
      printf 'source_refs_sha256=%s\n' "$policy_source_refs_sha"
      printf 'source_snapshot_sha256=%s\n' "$policy_source_snapshot_sha"
      printf 'hook_log_sha256=%s\n' "$(sha256_file "$test_root/policy-$policy_name-$boundary.log")"
    } > "$test_root/policy-$policy_name-$boundary-state.log"
  }
  assert_policy_generated_tree() {
    python3 -I -S - "$repository" "$1" "$2" <<'PY'
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
tree = sys.argv[2]
records = json.loads(Path(sys.argv[3]).read_text())["paths"]
def git(*arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments])

for record in records:
    path = record["path"]
    after = record["after"]
    listing = git("ls-tree", "-r", "-z", tree, "--", path).split(b"\0")
    entries = [item for item in listing if item]
    if after is None:
        if entries:
            raise SystemExit("generated-workflows: committed generated output retained a fixture-deleted path")
        continue
    if len(entries) != 1:
        raise SystemExit("generated-workflows: committed generated output path is missing or ambiguous")
    header, encoded_path = entries[0].split(b"\t", 1)
    mode, kind, oid = header.split()
    if os.fsdecode(encoded_path) != path or kind != b"blob":
        raise SystemExit("generated-workflows: committed generated output identity changed")
    content = git("cat-file", "blob", os.fsdecode(oid))
    expected_mode = b"120000" if after["kind"] == "symlink" else (b"100755" if after["mode"] & 0o111 else b"100644")
    if mode != expected_mode or hashlib.sha256(content).hexdigest() != after["sha256"]:
        raise SystemExit("generated-workflows: hook changed the staged bytes or mode of a generated output")
    if after["kind"] == "symlink" and content.decode("utf-8") != after["target"]:
        raise SystemExit("generated-workflows: hook changed a generated output symlink target")
PY
  }
  assert_policy_external_generated_tree() {
    assert_policy_generated_tree "$1" "$policy_external_generated_manifest"
  }
  assert_policy_local_generated_tree() {
    assert_policy_generated_tree "$1" "$policy_local_generated_manifest"
  }
  assert_policy_formatting_generated_tree() {
    assert_policy_generated_tree "$1" "$policy_formatting_generated_manifest"
  }
  assert_policy_formatting_tree() {
    local tree=$1
    local actual_file
    local manifest
    actual_file="$test_root/policy-formatting-committed-file.actual"
    "$git_command" -C "$repository" show "$tree:Cargo.lock" > "$actual_file"
    if ! cmp -s "$test_root/policy-formatting-lock.candidate" "$actual_file"; then
      printf 'generated-workflows: committed Cargo.lock bytes differ from the formatting candidate\n' >&2
      return 1
    fi
    for manifest in Cargo.toml apps/jackin/Cargo.toml; do
      "$git_command" -C "$repository" show "$tree:$manifest" > "$actual_file"
      if ! cmp -s <("$git_command" -C "$repository" show "$source_head:$manifest") "$actual_file"; then
        printf 'generated-workflows: committed formatting fixture changed manifest bytes: %s\n' "$manifest" >&2
        return 1
      fi
    done
    assert_policy_formatting_generated_tree "$tree"
  }
  assert_policy_positive_generated_tree() {
    case "$1" in
      positive-external-edge) assert_policy_external_generated_tree "$2" ;;
      positive-lock-formatting) assert_policy_formatting_tree "$2" ;;
      *) assert_policy_local_generated_tree "$2" ;;
    esac
  }
  run_local_positive_generator() {
    local policy_velnor_executable
    local policy_velnor_version
    local first_output_sha
    local second_output_sha
    local changed_paths="$test_root/policy-local-changed-paths.nul"
    policy_velnor_executable=$(policy_verified_velnor_executable) || return 1
    tool_cache_generation=${policy_velnor_executable%%/tool-state/*}
    if ! policy_velnor_version=$(env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C "$policy_velnor_executable" --version 2> "$test_root/policy-local-generator-version.stderr"); then
      cat "$test_root/policy-local-generator-version.stderr" >&2
      printf 'generated-workflows: authenticated Velnor executable could not report its version for the local-edge fixture\n' >&2
      return 1
    fi
    if [[ "$policy_velnor_version" != *0.1.78* ]]; then
      printf 'generated-workflows: local-edge fixture generator did not report reviewed Velnor version 0.1.78\n' >&2
      return 1
    fi
    if ! (cd "$repository" && env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 "$policy_velnor_executable" generate --output-dir "$repository") > "$test_root/policy-local-generator-first.log" 2>&1; then
      cat "$test_root/policy-local-generator-first.log" >&2
      printf 'generated-workflows: authenticated Velnor 0.1.78 failed to generate the local-edge fixture outputs\n' >&2
      return 1
    fi
    first_output_sha=$(directory_digest "$repository/.github") || return 1
    if ! (cd "$repository" && env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 "$policy_velnor_executable" generate --output-dir "$repository") > "$test_root/policy-local-generator-second.log" 2>&1; then
      cat "$test_root/policy-local-generator-second.log" >&2
      printf 'generated-workflows: authenticated Velnor 0.1.78 failed its local-edge repeat generation\n' >&2
      return 1
    fi
    second_output_sha=$(directory_digest "$repository/.github") || return 1
    if [[ "$first_output_sha" != "$second_output_sha" ]]; then
      printf 'generated-workflows: local-edge Velnor output changed on the second generation\n' >&2
      return 1
    fi
    policy_local_output_sha=$second_output_sha
    if ! python3 -I -S - "$repository" "$source_head" "$changed_paths" "$policy_local_generated_paths" "$policy_local_generated_manifest" <<'PY'
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
baseline, changed_output, generated_output, manifest_output = sys.argv[2:]
def git(*arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments])
def inventory(directory):
    result = {}
    if not directory.exists() and not directory.is_symlink():
        return result
    paths = [directory]
    if directory.is_dir() and not directory.is_symlink():
        paths.extend(sorted(directory.rglob("*")))
    for path in paths:
        info = path.lstat()
        relative = ".github" if path == directory else path.relative_to(root).as_posix()
        if stat.S_ISREG(info.st_mode):
            value = {"mode": stat.S_IMODE(info.st_mode), "kind": "file", "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
        elif stat.S_ISLNK(info.st_mode):
            target = os.readlink(path)
            value = {"mode": stat.S_IMODE(info.st_mode), "kind": "symlink", "target": target, "sha256": hashlib.sha256(os.fsencode(target)).hexdigest()}
        elif stat.S_ISDIR(info.st_mode):
            continue
        else:
            raise SystemExit("generated-workflows: Velnor emitted a special file")
        result[relative] = value
    return result

before = {}
for entry in git("ls-tree", "-r", "-z", baseline, "--", ".github").split(b"\0"):
    if not entry:
        continue
    header, encoded_path = entry.split(b"\t", 1)
    mode, kind, oid = header.split()
    path = os.fsdecode(encoded_path)
    if kind != b"blob":
        raise SystemExit("generated-workflows: baseline .github tree contains a non-blob entry")
    content = git("cat-file", "blob", os.fsdecode(oid))
    if mode == b"120000":
        before[path] = {"mode": 0o777, "kind": "symlink", "target": os.fsdecode(content), "sha256": hashlib.sha256(content).hexdigest()}
    else:
        before[path] = {"mode": int(mode, 8) & 0o777, "kind": "file", "sha256": hashlib.sha256(content).hexdigest()}
after = inventory(root / ".github")
changed_github = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
tracked = {os.fsdecode(path) for path in git("diff", "--name-only", "-z", baseline).split(b"\0") if path}
untracked = {os.fsdecode(path) for path in git("ls-files", "--others", "--exclude-standard", "-z").split(b"\0") if path}
ignored = {os.fsdecode(path) for path in git("ls-files", "--others", "--ignored", "--exclude-standard", "-z").split(b"\0") if path}
changed = tracked | untracked | set(changed_github)
expected = {"Cargo.lock", "apps/jackin/Cargo.toml"} | set(changed_github)
if changed != expected:
    raise SystemExit("generated-workflows: local-edge fixture changed paths beyond its two Cargo inputs and exact generated outputs")
if any(not path.startswith(".github/") for path in ignored):
    raise SystemExit("generated-workflows: local-edge fixture created an ignored path outside generated .github outputs")
if any(not path.startswith(".github/") for path in changed_github):
    raise SystemExit("generated-workflows: local generated output path escaped .github")
Path(changed_output).write_bytes(b"".join(os.fsencode(path) + b"\0" for path in sorted(changed)))
Path(generated_output).write_bytes(b"".join(os.fsencode(path) + b"\0" for path in changed_github))
records = [{"path": path, "before": before.get(path), "after": after.get(path)} for path in changed_github]
Path(manifest_output).write_text(json.dumps({"paths": records}, sort_keys=True, separators=(",", ":")) + "\n")
PY
    then
      return 1
    fi
    if [[ ! -s "$policy_local_generated_paths" ]]; then
      printf 'generated-workflows: local-positive Velnor generation produced no .github changes\n' >> "$test_root/policy-local-generator-state.log"
      printf 'local_generated_paths=none\n' >> "$test_root/policy-fixture-context.txt"
    else
      printf 'local_generated_paths=' >> "$test_root/policy-local-generator-state.log"
      tr '\0' '\n' < "$policy_local_generated_paths" | paste -sd, - >> "$test_root/policy-local-generator-state.log"
      printf 'local_generated_paths=' >> "$test_root/policy-fixture-context.txt"
      tr '\0' '\n' < "$policy_local_generated_paths" | paste -sd, - >> "$test_root/policy-fixture-context.txt"
    fi
    {
      printf 'generator_version=%s\n' "$policy_velnor_version"
      printf 'generator_sha256=%s\n' "$(sha256_file "$policy_velnor_executable")"
      printf 'generator_repeat_stable=true\n'
      printf 'generated_output_sha256=%s\n' "$policy_local_output_sha"
      printf 'generated_path_oracle_sha256=%s\n' "$(sha256_file "$changed_paths")"
      printf 'generated_paths_sha256=%s\n' "$(sha256_file "$policy_local_generated_paths")"
      printf 'generated_manifest_sha256=%s\n' "$(sha256_file "$policy_local_generated_manifest")"
    } >> "$test_root/policy-local-generator-state.log"
    {
      printf 'local_generator_version=%s\n' "$policy_velnor_version"
      printf 'local_generator_sha256=%s\n' "$(sha256_file "$policy_velnor_executable")"
      printf 'local_generator_repeat_stable=true\n'
      printf 'local_generated_output_sha256=%s\n' "$policy_local_output_sha"
      printf 'local_generated_path_oracle_sha256=%s\n' "$(sha256_file "$changed_paths")"
      printf 'local_generated_manifest_sha256=%s\n' "$(sha256_file "$policy_local_generated_manifest")"
    } >> "$test_root/policy-fixture-context.txt"
  }
  prepare_lock_formatting_fixture() {
    if ! python3 -I -S - "$repository" "$source_head" "$test_root/policy-formatting-lock.candidate" "$test_root/policy-formatting-semantic-proof.json" <<'PY'
import hashlib
import json
import subprocess
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
baseline, candidate_output, proof_output = sys.argv[2:]
candidate_output = Path(candidate_output)
proof_output = Path(proof_output)
def git(*arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments])
baseline_lock = git("show", f"{baseline}:Cargo.lock")
if not baseline_lock.endswith(b"\n") or baseline_lock.endswith(b"\n\n"):
    raise SystemExit("generated-workflows: baseline lock lacks a final line break or already has a trailing blank line")
candidate_lock = baseline_lock + b"\n"
if candidate_lock[:-1] != baseline_lock or len(candidate_lock) != len(baseline_lock) + 1:
    raise SystemExit("generated-workflows: formatting fixture did not append exactly one byte")
parsed_before = tomllib.loads(baseline_lock.decode("utf-8"))
parsed_after = tomllib.loads(candidate_lock.decode("utf-8"))
def typed_equal(left, right):
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(typed_equal(left[key], right[key]) for key in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(typed_equal(a, b) for a, b in zip(left, right))
    return left == right
if not typed_equal(parsed_before, parsed_after):
    raise SystemExit("generated-workflows: one trailing blank line changed parsed Cargo.lock values")
if type(parsed_before.get("version")) is not int or parsed_before["version"] != 4:
    raise SystemExit("generated-workflows: formatting fixture baseline does not contain integer lock version 4")
packages_before = parsed_before.get("package")
packages_after = parsed_after.get("package")
if not isinstance(packages_before, list) or len(packages_before) != 299 or len(packages_after) != len(packages_before):
    raise SystemExit("generated-workflows: formatting fixture package inventory differs from the selected 299-package lock")
def edge_count(packages):
    return sum(len(package.get("dependencies", [])) for package in packages)
edges_before = edge_count(packages_before)
edges_after = edge_count(packages_after)
if edges_before != edges_after:
    raise SystemExit("generated-workflows: formatting fixture changed the Cargo.lock dependency edge count")
manifest_hashes = {}
for manifest in ("Cargo.toml", "apps/jackin/Cargo.toml"):
    baseline_bytes = git("show", f"{baseline}:{manifest}")
    current_bytes = (root / manifest).read_bytes()
    if current_bytes != baseline_bytes:
        raise SystemExit(f"generated-workflows: formatting fixture changed manifest bytes: {manifest}")
    manifest_hashes[manifest] = hashlib.sha256(current_bytes).hexdigest()
candidate_output.write_bytes(candidate_lock)
(root / "Cargo.lock").write_bytes(candidate_lock)
proof = {
    "baseline_lock_sha256": hashlib.sha256(baseline_lock).hexdigest(),
    "candidate_lock_sha256": hashlib.sha256(candidate_lock).hexdigest(),
    "baseline_lock_bytes": len(baseline_lock),
    "candidate_lock_bytes": len(candidate_lock),
    "appended_bytes": 1,
    "appended_byte_hex": candidate_lock[-1:].hex(),
    "parsed_type_sensitive_equal": True,
    "lock_version_type": type(parsed_after["version"]).__name__,
    "lock_version": parsed_after["version"],
    "package_count_before": len(packages_before),
    "package_count_after": len(packages_after),
    "dependency_edge_count_before": edges_before,
    "dependency_edge_count_after": edges_after,
    "manifest_sha256": manifest_hashes,
    "manifest_bytes_unchanged": True,
}
proof_output.write_text(json.dumps(proof, sort_keys=True, separators=(",", ":")) + "\n")
PY
    then
      return 1
    fi
    policy_formatting_candidate_lock_sha=$(sha256_file "$test_root/policy-formatting-lock.candidate") || return 1
  }
  run_lock_formatting_positive_generator() {
    local policy_velnor_executable
    local policy_velnor_version
    local first_output_sha
    local second_output_sha
    local changed_paths="$test_root/policy-formatting-changed-paths.nul"
    policy_velnor_executable=$(policy_verified_velnor_executable) || return 1
    tool_cache_generation=${policy_velnor_executable%%/tool-state/*}
    if ! policy_velnor_version=$(env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C "$policy_velnor_executable" --version 2> "$test_root/policy-formatting-generator-version.stderr"); then
      cat "$test_root/policy-formatting-generator-version.stderr" >&2
      printf 'generated-workflows: authenticated Velnor could not report its version for the lock-formatting fixture\n' >&2
      return 1
    fi
    if [[ "$policy_velnor_version" != *0.1.78* ]]; then
      printf 'generated-workflows: lock-formatting fixture generator did not report reviewed Velnor version 0.1.78\n' >&2
      return 1
    fi
    if ! (cd "$repository" && env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 "$policy_velnor_executable" generate --output-dir "$repository") > "$test_root/policy-formatting-generator-first.log" 2>&1; then
      cat "$test_root/policy-formatting-generator-first.log" >&2
      printf 'generated-workflows: authenticated Velnor 0.1.78 failed to generate lock-formatting outputs\n' >&2
      return 1
    fi
    first_output_sha=$(directory_digest "$repository/.github") || return 1
    if ! (cd "$repository" && env -i HOME="$home_dir" PATH=/usr/bin:/bin LC_ALL=C LANG=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 "$policy_velnor_executable" generate --output-dir "$repository") > "$test_root/policy-formatting-generator-second.log" 2>&1; then
      cat "$test_root/policy-formatting-generator-second.log" >&2
      printf 'generated-workflows: authenticated Velnor 0.1.78 failed its lock-formatting repeat generation\n' >&2
      return 1
    fi
    second_output_sha=$(directory_digest "$repository/.github") || return 1
    if [[ "$first_output_sha" != "$second_output_sha" ]]; then
      printf 'generated-workflows: lock-formatting Velnor output changed on its second generation\n' >&2
      return 1
    fi
    policy_formatting_output_sha=$second_output_sha
    if ! cmp -s "$test_root/policy-formatting-lock.candidate" "$repository/Cargo.lock"; then
      printf 'generated-workflows: Velnor changed candidate Cargo.lock formatting bytes\n' >&2
      return 1
    fi
    if ! python3 -I -S - "$repository" "$source_head" "$changed_paths" "$policy_formatting_generated_paths" "$policy_formatting_generated_manifest" "$policy_formatting_candidate_paths" <<'PY'
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
baseline, changed_output, generated_output, manifest_output, candidate_output = sys.argv[2:]
def git(*arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments])
def inventory(directory):
    result = {}
    if not directory.exists() and not directory.is_symlink():
        return result
    paths = [directory]
    if directory.is_dir() and not directory.is_symlink():
        paths.extend(sorted(directory.rglob("*")))
    for path in paths:
        info = path.lstat()
        relative = ".github" if path == directory else path.relative_to(root).as_posix()
        if stat.S_ISREG(info.st_mode):
            value = {"mode": stat.S_IMODE(info.st_mode), "kind": "file", "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
        elif stat.S_ISLNK(info.st_mode):
            target = os.readlink(path)
            value = {"mode": stat.S_IMODE(info.st_mode), "kind": "symlink", "target": target, "sha256": hashlib.sha256(os.fsencode(target)).hexdigest()}
        elif stat.S_ISDIR(info.st_mode):
            continue
        else:
            raise SystemExit("generated-workflows: Velnor emitted a special file")
        result[relative] = value
    return result

before = {}
for entry in git("ls-tree", "-r", "-z", baseline, "--", ".github").split(b"\0"):
    if not entry:
        continue
    header, encoded_path = entry.split(b"\t", 1)
    mode, kind, oid = header.split()
    path = os.fsdecode(encoded_path)
    if kind != b"blob":
        raise SystemExit("generated-workflows: baseline .github tree contains a non-blob entry")
    content = git("cat-file", "blob", os.fsdecode(oid))
    if mode == b"120000":
        before[path] = {"mode": 0o777, "kind": "symlink", "target": os.fsdecode(content), "sha256": hashlib.sha256(content).hexdigest()}
    else:
        before[path] = {"mode": int(mode, 8) & 0o777, "kind": "file", "sha256": hashlib.sha256(content).hexdigest()}
after = inventory(root / ".github")
changed_github = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
tracked = {os.fsdecode(path) for path in git("diff", "--name-only", "-z", baseline).split(b"\0") if path}
untracked = {os.fsdecode(path) for path in git("ls-files", "--others", "--exclude-standard", "-z").split(b"\0") if path}
ignored = {os.fsdecode(path) for path in git("ls-files", "--others", "--ignored", "--exclude-standard", "-z").split(b"\0") if path}
changed = tracked | untracked | set(changed_github)
expected = {"Cargo.lock"} | set(changed_github)
if changed != expected:
    raise SystemExit("generated-workflows: lock-formatting fixture changed paths beyond Cargo.lock and exact generated outputs")
if any(not path.startswith(".github/") for path in ignored):
    raise SystemExit("generated-workflows: lock-formatting fixture created an ignored path outside generated .github outputs")
if any(not path.startswith(".github/") for path in changed_github):
    raise SystemExit("generated-workflows: lock-formatting output path escaped .github")
for manifest in ("Cargo.toml", "apps/jackin/Cargo.toml"):
    if (root / manifest).read_bytes() != git("show", f"{baseline}:{manifest}"):
        raise SystemExit(f"generated-workflows: generator changed manifest bytes: {manifest}")
Path(changed_output).write_bytes(b"".join(os.fsencode(path) + b"\0" for path in sorted(changed)))
Path(generated_output).write_bytes(b"".join(os.fsencode(path) + b"\0" for path in changed_github))
records = [{"path": path, "before": before.get(path), "after": after.get(path)} for path in sorted(set(before) | set(after))]
Path(manifest_output).write_text(json.dumps({"paths": records}, sort_keys=True, separators=(",", ":")) + "\n")
candidate_paths = [b"Cargo.lock", *(os.fsencode(path) for path in changed_github)]
Path(candidate_output).write_bytes(b"".join(path + b"\0" for path in sorted(candidate_paths)))
PY
    then
      return 1
    fi
    {
      printf 'generator_version=%s\n' "$policy_velnor_version"
      printf 'generator_sha256=%s\n' "$(sha256_file "$policy_velnor_executable")"
      printf 'generator_repeat_stable=true\n'
      printf 'generated_output_sha256=%s\n' "$policy_formatting_output_sha"
      printf 'generated_paths_sha256=%s\n' "$(sha256_file "$policy_formatting_generated_paths")"
      printf 'generated_manifest_sha256=%s\n' "$(sha256_file "$policy_formatting_generated_manifest")"
      printf 'semantic_proof_sha256=%s\n' "$(sha256_file "$test_root/policy-formatting-semantic-proof.json")"
      printf 'candidate_lock_sha256=%s\n' "$policy_formatting_candidate_lock_sha"
    } >> "$test_root/policy-formatting-generator-state.log"
  }
  run_policy_positive() {
    local positive_name=$1
    local policy_branch="policy-positive-$positive_name"
    local staged_tree
    local candidate_oid
    local candidate_tree
    local candidate_file
    local expected
    local status
    local candidate_paths="$test_root/$positive_name-candidate-paths"
    local expected_paths="$test_root/$positive_name-candidate-paths.expected"
    local expected_output_sha="$policy_output_sha"
    local receiver_refs_before
    local repository_refs_before=
    reset_policy_fixture
    record_policy_case_baseline "$positive_name"
    if [[ "$positive_name" == positive-lock-formatting ]]; then
      repository_refs_before=$("$git_command" -C "$repository" show-ref --head | LC_ALL=C sort)
    fi
    if [[ "$positive_name" == positive-external-edge ]]; then
      if [[ "$source_head" != "$policy_control_head" || "$policy_external_fixture_parent" != "$policy_control_head" ]]; then
        printf 'generated-workflows: positive external-edge fixture did not use the immutable control child as its parent\n' >&2
        exit 1
      fi
      expected_output_sha=$policy_external_output_sha
      if ! "$git_command" -C "$repository" apply --binary "$policy_external_fixture_patch"; then
        printf 'generated-workflows: cannot apply the immutable external-edge fixture to the installed boundary checkout\n' >&2
        exit 1
      fi
      if ! cmp -s "$policy_source_repository/apps/jackin/Cargo.toml" "$repository/apps/jackin/Cargo.toml" || ! cmp -s "$policy_source_repository/Cargo.lock" "$repository/Cargo.lock" || ! cmp -s "$policy_source_repository/Cargo.toml" "$repository/Cargo.toml" || [[ "$(directory_digest "$repository/.github")" != "$policy_external_output_sha" ]]; then
        printf 'generated-workflows: replayed external-edge fixture differs from its immutable metadata or generated outputs\n' >&2
        exit 1
      fi
    elif [[ "$positive_name" == positive-lock-formatting ]]; then
      prepare_lock_formatting_fixture || exit 1
      if ! run_lock_formatting_positive_generator; then
        exit 1
      fi
      expected_output_sha=$policy_formatting_output_sha
    else
      prepare_local_local_removal_fixture "$repository"
      if ! run_local_positive_generator; then
        exit 1
      fi
      expected_output_sha=$policy_local_output_sha
    fi
    if [[ "$positive_name" == positive-external-edge ]]; then
      "$git_command" -C "$repository" add -f -A -- Cargo.lock apps/jackin/Cargo.toml .github
    else
      "$git_command" -C "$repository" add -f -A -- apps/jackin/Cargo.toml Cargo.lock .github
    fi
    staged_tree=$("$git_command" -C "$repository" write-tree)
    if [[ "$positive_name" == positive-external-edge ]]; then
      if [[ "$staged_tree" != "$policy_external_fixture_tree" ]]; then
        printf 'generated-workflows: staged external-edge index does not match the immutable fixture tree\n' >&2
        exit 1
      fi
      if ! "$git_command" -C "$repository" diff --cached --name-only -z "$source_head" -- > "$candidate_paths"; then
        printf 'generated-workflows: cannot inspect staged external-edge fixture paths\n' >&2
        exit 1
      fi
      if ! python3 -I -S - "$candidate_paths" "$test_root/policy-external-candidate-paths.expected.nul" <<'PY'
import sys
from pathlib import Path

actual = sorted(item for item in Path(sys.argv[1]).read_bytes().split(b"\0") if item)
expected = sorted(item for item in Path(sys.argv[2]).read_bytes().split(b"\0") if item)
if actual != expected:
    raise SystemExit("generated-workflows: staged external-edge fixture paths differ from the exact immutable fixture paths")
PY
      then
        exit 1
      fi
      assert_policy_external_generated_tree "$staged_tree"
    elif [[ "$positive_name" == positive-lock-formatting ]]; then
      if [[ "$staged_tree" == "$policy_candidate_tree" ]]; then
        printf 'generated-workflows: lock-formatting fixture did not change the candidate tree\n' >&2
        exit 1
      fi
      if ! "$git_command" -C "$repository" diff --cached --name-only -z "$source_head" -- > "$candidate_paths"; then
        printf 'generated-workflows: cannot inspect staged lock-formatting fixture paths\n' >&2
        exit 1
      fi
      if ! python3 -I -S - "$candidate_paths" "$policy_formatting_candidate_paths" <<'PY'
import sys
from pathlib import Path

actual = sorted(item for item in Path(sys.argv[1]).read_bytes().split(b"\0") if item)
expected = sorted(item for item in Path(sys.argv[2]).read_bytes().split(b"\0") if item)
if actual != expected:
    raise SystemExit("generated-workflows: staged lock-formatting paths differ from Cargo.lock and fresh Velnor outputs")
PY
      then
        printf 'generated-workflows: lock-formatting changed paths outside Cargo.lock and generated outputs\n' >&2
        exit 1
      fi
      assert_policy_formatting_tree "$staged_tree"
    else
      if [[ "$staged_tree" == "$policy_candidate_tree" ]]; then
        printf 'generated-workflows: local-local removal fixture did not change the candidate tree\n' >&2
        exit 1
      fi
      if ! "$git_command" -C "$repository" diff --cached --name-only -z "$source_head" -- > "$candidate_paths"; then
        printf 'generated-workflows: cannot inspect staged local-local removal paths\n' >&2
        exit 1
      fi
      if ! python3 -I -S - "$candidate_paths" "$policy_local_generated_paths" "$expected_paths" <<'PY'
import sys
from pathlib import Path

actual = sorted(item for item in Path(sys.argv[1]).read_bytes().split(b"\0") if item)
generated = sorted(item for item in Path(sys.argv[2]).read_bytes().split(b"\0") if item)
expected = sorted({b"Cargo.lock", b"apps/jackin/Cargo.toml", *generated})
if actual != expected:
    raise SystemExit("generated-workflows: staged local-edge paths differ from its exact Cargo and generated output paths")
Path(sys.argv[3]).write_bytes(b"".join(item + b"\0" for item in expected))
PY
      then
        printf 'generated-workflows: local-local removal changed paths outside its manifest and lock edge\n' >&2
        exit 1
      fi
      assert_policy_local_generated_tree "$staged_tree"
    fi
    "$git_command" -C "$repository" switch -c "$policy_branch" "$source_head" >/dev/null
    if [[ ( "$positive_name" == positive-external-edge && ( "$public_mutant" == whole-lock-digest || "$public_mutant" == edge-resolution ) ) || ( "$positive_name" == positive-lock-formatting && "$public_mutant" == whole-lock-digest ) ]]; then
      case "$public_mutant" in
        whole-lock-digest) expected='Cargo.lock is outside the reviewed dependency support policy' ;;
        edge-resolution) expected='snapshot Cargo.lock contains an unresolved or ambiguous dependency identity' ;;
      esac
      if (cd "$repository" && HOME="$home_dir" "$git_command" commit -m "test: qualify $positive_name Cargo lock policy") > "$test_root/$positive_name-commit.log" 2>&1; then
        status=0
      else
        status=$?
      fi
      if [[ "$status" -eq 0 || "$("$git_command" -C "$repository" write-tree)" != "$staged_tree" || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" rev-parse "refs/heads/$policy_branch")" != "$source_head" || "$("$git_command" -C "$remote" show-ref || true)" != "$policy_initial_receiver_refs" ]] || ! /usr/bin/grep -F -q "$expected" "$test_root/$positive_name-commit.log" || /usr/bin/grep -E -q 'unbound variable|selected_policy_cases' "$test_root/$positive_name-commit.log"; then
        cat "$test_root/$positive_name-commit.log" >&2
        printf 'generated-workflows: %s mutant did not reject the %s fixture at its intended pre-commit policy boundary\n' "$public_mutant" "$positive_name" >&2
        exit 1
      fi
      assert_policy_state "$expected_output_sha"
      assert_policy_positive_generated_tree "$positive_name" "$staged_tree"
      assert_policy_source_unchanged
      if ! (cd "$repository" && HOME="$home_dir" "$git_command" -c core.hooksPath=/dev/null commit -m "test: prepare $positive_name mutant pre-push snapshot") > "$test_root/$positive_name-mutant-fixture-commit.log" 2>&1; then
        cat "$test_root/$positive_name-mutant-fixture-commit.log" >&2
        printf 'generated-workflows: cannot prepare the private mutant pre-push candidate\n' >&2
        exit 1
      fi
      candidate_oid=$("$git_command" -C "$repository" rev-parse HEAD)
      candidate_tree=$("$git_command" -C "$repository" rev-parse "$candidate_oid^{tree}")
      if [[ ( "$positive_name" == positive-external-edge && "$candidate_tree" != "$policy_external_fixture_tree" ) || ( "$positive_name" == positive-lock-formatting && "$candidate_tree" != "$staged_tree" ) || "$("$git_command" -C "$repository" rev-parse "$candidate_oid^1")" != "$source_head" ]]; then
        printf 'generated-workflows: mutant pre-push fixture does not match its approved candidate tree\n' >&2
        exit 1
      fi
      assert_policy_positive_generated_tree "$positive_name" "$candidate_tree"
      "$git_command" -C "$repository" switch main >/dev/null
      if [[ "$candidate_oid" == "$("$git_command" -C "$repository" rev-parse HEAD)" ]]; then
        printf 'generated-workflows: mutant pre-push fixture is unexpectedly HEAD\n' >&2
        exit 1
      fi
      if (cd "$repository" && HOME="$home_dir" "$git_command" push origin "$policy_branch:refs/heads/$policy_branch") > "$test_root/$positive_name-mutant-push.log" 2>&1; then
        status=0
      else
        status=$?
      fi
      if [[ "$status" -eq 0 || "$("$git_command" -C "$remote" show-ref || true)" != "$policy_initial_receiver_refs" || "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" rev-parse "refs/heads/$policy_branch")" != "$candidate_oid" ]] || "$git_command" -C "$remote" show-ref --verify --quiet "refs/heads/$policy_branch" || ! /usr/bin/grep -F -q "$expected" "$test_root/$positive_name-mutant-push.log" || /usr/bin/grep -E -q 'unbound variable|selected_policy_cases' "$test_root/$positive_name-mutant-push.log"; then
        cat "$test_root/$positive_name-mutant-push.log" >&2
        printf 'generated-workflows: %s mutant did not reject the non-HEAD candidate at its intended pre-push policy boundary\n' "$public_mutant" >&2
        exit 1
      fi
      assert_policy_state "$expected_output_sha"
      assert_policy_positive_generated_tree "$positive_name" "$candidate_tree"
      assert_policy_source_unchanged
      reset_policy_fixture
      "$git_command" -C "$repository" branch -D "$policy_branch" >/dev/null
      if [[ "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" write-tree)" != "$policy_baseline_tree" || "$("$git_command" -C "$remote" show-ref || true)" != "$policy_initial_receiver_refs" ]]; then
        printf 'generated-workflows: %s mutant rejection did not preserve control refs and index state\n' "$public_mutant" >&2
        exit 1
      fi
      if [[ "$positive_name" == positive-lock-formatting && "$("$git_command" -C "$repository" show-ref --head | LC_ALL=C sort)" != "$repository_refs_before" ]]; then
        printf 'generated-workflows: formatting mutant rejection changed an unrelated local ref\n' >&2
        exit 1
      fi
      assert_policy_source_unchanged
      printf 'generated-workflows: %s mutant caused the %s fixture to fail both real policy boundaries as expected\n' "$public_mutant" "$positive_name" >&2
      exit 1
    fi
    if ! (cd "$repository" && HOME="$home_dir" "$git_command" commit -m "test: qualify $positive_name Cargo lock policy") > "$test_root/$positive_name-commit.log" 2>&1; then
      cat "$test_root/$positive_name-commit.log" >&2
      printf 'generated-workflows: actual pre-commit rejected valid %s manifests and lock edges\n' "$positive_name" >&2
      exit 1
    fi
    candidate_oid=$("$git_command" -C "$repository" rev-parse HEAD)
    candidate_tree=$("$git_command" -C "$repository" rev-parse "$candidate_oid^{tree}")
    if [[ "$("$git_command" -C "$repository" rev-parse "$candidate_oid^1")" != "$source_head" ]]; then
      printf 'generated-workflows: positive %s commit is not a child of its declared baseline\n' "$positive_name" >&2
      exit 1
    fi
    if [[ "$positive_name" == positive-external-edge ]]; then
      if [[ "$candidate_tree" != "$policy_external_fixture_tree" ]]; then
        printf 'generated-workflows: actual positive commit does not reproduce the immutable external-edge fixture tree\n' >&2
        exit 1
      fi
      assert_policy_external_generated_tree "$candidate_tree"
      for input_path in Cargo.toml apps/jackin/Cargo.toml Cargo.lock; do
        "$git_command" -C "$repository" show "$candidate_oid:$input_path" > "$test_root/$positive_name-$(basename "$input_path").actual"
        if ! cmp -s <("$git_command" -C "$policy_source_repository" show "$policy_external_fixture_head:$input_path") "$test_root/$positive_name-$(basename "$input_path").actual"; then
          printf 'generated-workflows: actual positive commit does not preserve immutable fixture file bytes: %s\n' "$input_path" >&2
          exit 1
        fi
      done
      assert_policy_state "$expected_output_sha"
    elif [[ "$positive_name" == positive-lock-formatting ]]; then
      if [[ "$candidate_tree" != "$staged_tree" ]]; then
        printf 'generated-workflows: lock-formatting commit does not preserve its exact staged candidate tree\n' >&2
        exit 1
      fi
      assert_policy_formatting_tree "$candidate_tree"
      assert_policy_state "$expected_output_sha"
    else
      if [[ "$candidate_tree" != "$staged_tree" ]]; then
        printf 'generated-workflows: local-local removal commit does not preserve its exact staged fixture tree\n' >&2
        exit 1
      fi
      assert_policy_local_generated_tree "$candidate_tree"
      assert_policy_state "$expected_output_sha"
    fi
    "$git_command" -C "$repository" switch main >/dev/null
    if [[ "$candidate_oid" == "$("$git_command" -C "$repository" rev-parse HEAD)" ]]; then
      printf 'generated-workflows: positive pre-push fixture is unexpectedly HEAD\n' >&2
      exit 1
    fi
    if ! (cd "$repository" && HOME="$home_dir" "$git_command" push origin "$policy_branch:refs/heads/$policy_branch") > "$test_root/$positive_name-push.log" 2>&1; then
      cat "$test_root/$positive_name-push.log" >&2
      printf 'generated-workflows: actual pre-push rejected valid %s manifests and lock edges\n' "$positive_name" >&2
      exit 1
    fi
    receiver_refs_before=$policy_initial_receiver_refs
    expected_receiver_refs=$(printf '%s\n%s refs/heads/%s\n' "$policy_initial_receiver_refs" "$candidate_oid" "$policy_branch" | LC_ALL=C sort)
    actual_receiver_refs=$("$git_command" -C "$remote" show-ref | LC_ALL=C sort)
    if [[ "$actual_receiver_refs" != "$expected_receiver_refs" || "$("$git_command" -C "$remote" rev-parse refs/heads/main)" != "$source_head" ]]; then
      printf 'generated-workflows: valid %s branch did not update only its receiver ref\n' "$positive_name" >&2
      exit 1
    fi
    policy_initial_receiver_refs=$actual_receiver_refs
    assert_policy_state "$expected_output_sha"
    assert_policy_positive_generated_tree "$positive_name" "$candidate_tree"
    assert_policy_source_unchanged
    if [[ "$("$git_command" -C "$repository" write-tree)" != "$policy_baseline_tree" ]]; then
      printf 'generated-workflows: positive %s fixture changed the source index tree\n' "$positive_name" >&2
      exit 1
    fi
    "$git_command" -C "$repository" branch -D "$policy_branch" >/dev/null
    if "$git_command" -C "$repository" show-ref --verify --quiet "refs/heads/$policy_branch" || [[ "$("$git_command" -C "$repository" symbolic-ref --short HEAD)" != main || "$("$git_command" -C "$repository" rev-parse HEAD)" != "$source_head" || "$("$git_command" -C "$repository" status --porcelain)" != '' ]]; then
      printf 'generated-workflows: positive %s cleanup did not return the source checkout to the control commit\n' "$positive_name" >&2
      exit 1
    fi
    if [[ "$positive_name" == positive-lock-formatting && "$("$git_command" -C "$repository" show-ref --head | LC_ALL=C sort)" != "$repository_refs_before" ]]; then
      printf 'generated-workflows: lock-formatting positive changed an unrelated local ref\n' >&2
      exit 1
    fi
    assert_policy_source_unchanged
    {
      printf 'case=%s\n' "$positive_name"
      printf 'baseline_commit=%s\n' "$source_head"
      printf 'candidate_commit=%s\n' "$candidate_oid"
      printf 'candidate_parent=%s\n' "$source_head"
      printf 'candidate_tree=%s\n' "$candidate_tree"
      printf 'staged_tree=%s\n' "$staged_tree"
      printf 'immutable_external_fixture_commit=%s\n' "${policy_external_fixture_head:-none}"
      printf 'immutable_external_fixture_tree=%s\n' "${policy_external_fixture_tree:-none}"
      printf 'pre_commit_log_sha256=%s\n' "$(sha256_file "$test_root/$positive_name-commit.log")"
      printf 'pre_push_log_sha256=%s\n' "$(sha256_file "$test_root/$positive_name-push.log")"
      printf 'staged_path_list_sha256=%s\n' "$(sha256_file "$candidate_paths")"
      printf 'generated_output_sha256=%s\n' "$expected_output_sha"
      printf 'generated_paths_sha256=%s\n' "$(case "$positive_name" in positive-external-edge) sha256_file "$policy_external_generated_paths" ;; positive-lock-formatting) sha256_file "$policy_formatting_generated_paths" ;; *) sha256_file "$policy_local_generated_paths" ;; esac)"
      printf 'generated_manifest_sha256=%s\n' "$(case "$positive_name" in positive-external-edge) sha256_file "$policy_external_generated_manifest" ;; positive-lock-formatting) sha256_file "$policy_formatting_generated_manifest" ;; *) sha256_file "$policy_local_generated_manifest" ;; esac)"
      if [[ "$positive_name" == positive-lock-formatting ]]; then
        printf 'formatting_semantic_proof_sha256=%s\n' "$(sha256_file "$test_root/policy-formatting-semantic-proof.json")"
        printf 'candidate_lock_sha256=%s\n' "$policy_formatting_candidate_lock_sha"
      fi
      if [[ "$positive_name" == positive-local-local-removal && ! -s "$policy_local_generated_paths" ]]; then
        printf 'generated_paths=none\n'
      fi
      printf 'receiver_refs_before_sha256=%s\n' "$(printf '%s\n' "$receiver_refs_before" | sha256_stdin)"
      printf 'receiver_refs_after_sha256=%s\n' "$(printf '%s\n' "$actual_receiver_refs" | sha256_stdin)"
      if [[ "$positive_name" == positive-lock-formatting ]]; then
        printf 'repository_refs_before_sha256=%s\n' "$(printf '%s\n' "$repository_refs_before" | sha256_stdin)"
        printf 'repository_refs_after_sha256=%s\n' "$("$git_command" -C "$repository" show-ref --head | LC_ALL=C sort | sha256_stdin)"
      fi
      printf 'git_config_sha256=%s\n' "$policy_config_sha"
      printf 'pre_commit_hook_sha256=%s\n' "$policy_hook_commit_sha"
      printf 'pre_push_hook_sha256=%s\n' "$policy_hook_push_sha"
      printf 'sentinel_sha256=%s\n' "$policy_sentinel_sha"
      printf 'source_index_tree=%s\n' "$policy_source_index_tree"
      printf 'source_status_sha256=%s\n' "$policy_source_status_sha"
      printf 'source_refs_sha256=%s\n' "$policy_source_refs_sha"
      printf 'source_snapshot_sha256=%s\n' "$policy_source_snapshot_sha"
      printf 'reset_index_tree=%s\n' "$policy_baseline_tree"
      printf 'reset_head=%s\n' "$source_head"
      printf 'reset_branch=main\n'
    } > "$test_root/policy-$positive_name-state.log"
    record_completion_witness "policy:$positive_name"
  }
  if [[ "$policy_case" == all ]]; then
    for selected_case in $negative_policy_cases; do
      run_policy_rejection "$selected_case" pre-commit
      run_policy_rejection "$selected_case" pre-push
      record_completion_witness "policy:$selected_case"
    done
  elif [[ "$policy_case" != positive-external-edge && "$policy_case" != positive-local-local-removal && "$policy_case" != positive-lock-formatting && "$policy_case" != positive-pair ]]; then
    run_policy_rejection "$policy_case" pre-commit
    run_policy_rejection "$policy_case" pre-push
    case "$public_mutant:$policy_case" in
      external-record:external-checksum|unknown-field:unknown-lock-metadata|path-policy:path-escape)
        printf 'matched %s mutant admitted %s at pre-commit and non-HEAD pre-push with exact state proofs\n' "$public_mutant" "$policy_case"
        ;;
    esac
    record_completion_witness "policy:$policy_case"
  fi
  if [[ "$policy_case" == all || "$policy_case" == positive-local-local-removal || "$policy_case" == positive-pair ]]; then
    run_policy_positive positive-local-local-removal
  fi
  if [[ "$policy_case" == all || "$policy_case" == positive-pair ]]; then
    switch_policy_baseline_to_control
  fi
  if [[ "$policy_case" == all || "$policy_case" == positive-external-edge || "$policy_case" == positive-pair ]]; then
    run_policy_positive positive-external-edge
  fi
  if [[ "$policy_case" == positive-lock-formatting ]]; then
    run_policy_positive positive-lock-formatting
  fi
  if [[ "$policy_case" == positive-pair ]]; then
    printf 'installed pre-commit and non-HEAD pre-push Cargo lock positive pair: PASS\n'
  elif [[ "$policy_case" == positive-lock-formatting ]]; then
    printf 'installed pre-commit and non-HEAD pre-push Cargo lock formatting positive: PASS\n'
  else
    printf 'installed pre-commit and non-HEAD pre-push Cargo lock policy matrix: PASS\n'
  fi
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == installer-reinstall ]]; then
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  save_hook_baseline
  printf -v escaped_install_root '%q' "$installation"
  if /usr/bin/grep -F -q "$installation/versions/" "$hooks_path/pre-commit" || ! /usr/bin/grep -F -x -q "trusted_install_root=$escaped_install_root" "$hooks_path/pre-commit" || ! /usr/bin/grep -F -x -q "trusted_install_root=$escaped_install_root" "$hooks_path/pre-push"; then
    printf 'generated-workflows: spaced installer root was not rendered with Bash shell escaping\n' >&2
    exit 1
  fi
  if [[ "$saved_pre_commit_mode" != 700 || "$saved_pre_push_mode" != 700 ]]; then
    printf 'generated-workflows: newly installed hook modes are not canonical\n' >&2
    exit 1
  fi
  if ! (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/reinstall.log" 2>&1; then
    cat "$test_root/reinstall.log" >&2
    printf 'generated-workflows: installer did not recognize its canonical hooks on rerun\n' >&2
    exit 1
  fi
  if ! cmp -s "$test_root/pre-commit.before" "$hooks_path/pre-commit" || ! cmp -s "$test_root/pre-push.before" "$hooks_path/pre-push" || [[ "$(stat_mode "$hooks_path/pre-commit")" != "$saved_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$saved_pre_push_mode" || "$(/bin/cat "$unrelated_sentinel")" != 'preserve this unrelated install payload' ]]; then
    printf 'generated-workflows: canonical reinstall changed hook bytes, modes, or unrelated state\n' >&2
    exit 1
  fi
  run_installed_hooks
  printf 'canonical installer rerun with escaped space and quote paths preserves both hooks and invokes both real launchers: PASS\n' >> "$test_root/case-results.log"
  printf 'canonical installer rerun with escaped space and quote paths: PASS\n'
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == installer-forged-hook ]]; then
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  save_hook_baseline
  printf -v escaped_install_root '%q' "$installation"
  for target in pre-commit pre-push; do
    restore_hook_baseline
    hook_path="$hooks_path/$target"
    forged_hook="$test_root/forged-$target"
    printf '#!/bin/bash\nguard_owner=jackin-generated-workflow-guard\nraw_path=%s/versions/foreign\nescaped_path=%s\n' "$installation" "$escaped_install_root" > "$forged_hook"
    chmod 700 "$forged_hook"
    cp "$forged_hook" "$hook_path"
    chmod 700 "$hook_path"
    forged_hash=$(sha256_file "$hook_path")
    forged_mode=$(stat_mode "$hook_path")
    if [[ ! -f "$hook_path" || -L "$hook_path" || "$forged_mode" != 700 ]]; then
      printf 'generated-workflows: forged hook fixture is not a regular safe-mode file\n' >&2
      exit 1
    fi
    other_target=pre-push
    [[ "$target" != pre-push ]] || other_target=pre-commit
    other_hash=$(sha256_file "$hooks_path/$other_target")
    other_mode=$(stat_mode "$hooks_path/$other_target")
    installer_status=0
    if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/forged-$target.log" 2>&1; then
      installer_status=0
    else
      installer_status=$?
    fi
    if [[ "$installer_status" -eq 0 ]] || ! /usr/bin/grep -F -q 'refusing to replace an existing hook' "$test_root/forged-$target.log" || [[ "$(sha256_file "$hook_path")" != "$forged_hash" || "$(stat_mode "$hook_path")" != "$forged_mode" || "$(sha256_file "$hooks_path/$other_target")" != "$other_hash" || "$(stat_mode "$hooks_path/$other_target")" != "$other_mode" || "$(/bin/cat "$unrelated_sentinel")" != 'preserve this unrelated install payload' ]]; then
      cat "$test_root/forged-$target.log" >&2
      printf 'generated-workflows: installer did not refuse and preserve forged %s hook state\n' "$target" >&2
      exit 1
    fi
    restore_hook_baseline
  done
  for target in pre-commit pre-push; do
    restore_hook_baseline
    hook_path="$hooks_path/$target"
    printf '\nbody-appended altered launcher\n' >> "$hook_path"
    altered_hash=$(sha256_file "$hook_path")
    other_target=pre-push
    [[ "$target" != pre-push ]] || other_target=pre-commit
    other_hash=$(sha256_file "$hooks_path/$other_target")
    other_mode=$(stat_mode "$hooks_path/$other_target")
    installer_status=0
    if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/altered-$target.log" 2>&1; then
      installer_status=0
    else
      installer_status=$?
    fi
    if [[ "$installer_status" -eq 0 ]] || ! /usr/bin/grep -F -q 'refusing to replace an existing hook' "$test_root/altered-$target.log" || [[ "$(sha256_file "$hook_path")" != "$altered_hash" || "$(sha256_file "$hooks_path/$other_target")" != "$other_hash" || "$(stat_mode "$hooks_path/$other_target")" != "$other_mode" || "$(/bin/cat "$unrelated_sentinel")" != 'preserve this unrelated install payload' ]]; then
      cat "$test_root/altered-$target.log" >&2
      printf 'generated-workflows: installer did not refuse and preserve altered %s hook state\n' "$target" >&2
      exit 1
    fi
    restore_hook_baseline
  done
  printf 'forged owner-marker hooks and altered canonical hooks are refused and preserved at preflight: PASS\n' >> "$test_root/case-results.log"
  printf 'forged and altered hook refusal: PASS\n'
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == installer-managed-hook-upgrade ]]; then
  copy_fixture_tree() {
    case "$(uname -s)" in
      Darwin) /bin/cp -cRp "$1" "$2" ;;
      Linux) /bin/cp -a --reflink=auto "$1" "$2" ;;
      *) return 2 ;;
    esac
  }
  run_candidate_installer() {
    local log=$1
    if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$log" 2>&1; then
      return 0
    else
      return $?
    fi
  }
  snapshot_path() {
    python3 -I -S - "$1" <<'PY'
import hashlib
import json
import os
import stat
import sys

root = os.path.abspath(sys.argv[1])
records = []

def add(path, relative):
    metadata = os.lstat(path)
    entry = {
        "path": relative,
        "mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
        "uid": metadata.st_uid,
        "links": metadata.st_nlink,
    }
    if stat.S_ISLNK(metadata.st_mode):
        entry["type"] = "link"
        entry["target"] = os.readlink(path)
    elif stat.S_ISDIR(metadata.st_mode):
        entry["type"] = "directory"
    elif stat.S_ISREG(metadata.st_mode):
        digest = hashlib.sha256()
        with open(path, "rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        entry["type"] = "file"
        entry["sha256"] = digest.hexdigest()
    else:
        entry["type"] = "special"
        entry["device"] = metadata.st_rdev
    records.append(entry)
    if entry["type"] == "directory":
        with os.scandir(path) as children:
            names = sorted((child.name for child in children), key=os.fsencode)
        for name in names:
            child = os.path.join(path, name)
            child_relative = name if relative == "." else f"{relative}/{name}"
            add(child, child_relative)

if not os.path.lexists(root):
    records.append({"path": ".", "type": "missing"})
else:
    add(root, ".")
for record in records:
    print(json.dumps(record, ensure_ascii=True, separators=(",", ":")))
PY
  }
  capture_upgrade_baseline() {
    local label=$1
    local directory="$test_root/baseline-$label"
    mkdir "$directory"
    snapshot_path "$installation/versions" > "$directory/versions"
    snapshot_path "$old_version_dir" > "$directory/old-generation"
    snapshot_path "$test_root/escaped-new-generation" > "$directory/escaped-generation"
    snapshot_path "$old_python_artifact_dir" > "$directory/old-python-generation"
    snapshot_path "$new_version_dir" > "$directory/new-generation"
    snapshot_path "$hooks_path/pre-commit" > "$directory/pre-commit"
    snapshot_path "$hooks_path/pre-push" > "$directory/pre-push"
    snapshot_path "$unrelated_sentinel" > "$directory/sentinel"
    snapshot_path "$common_dir/config" > "$directory/git-config"
    snapshot_path "$common_dir/index" > "$directory/git-index"
  }
  assert_upgrade_entries() {
    local label=$1
    shift
    local directory="$test_root/baseline-$label"
    local entry
    local path
    for entry in "$@"; do
      case "$entry" in
        versions) path="$installation/versions" ;;
        old-generation) path=$old_version_dir ;;
        escaped-generation) path="$test_root/escaped-new-generation" ;;
        old-python-generation) path=$old_python_artifact_dir ;;
        new-generation) path=$new_version_dir ;;
        pre-commit) path="$hooks_path/pre-commit" ;;
        pre-push) path="$hooks_path/pre-push" ;;
        sentinel) path=$unrelated_sentinel ;;
        git-config) path=$common_dir/config ;;
        git-index) path=$common_dir/index ;;
      esac
      snapshot_path "$path" > "$directory/$entry.after"
      if ! cmp -s "$directory/$entry" "$directory/$entry.after"; then
        printf 'generated-workflows: managed-upgrade state snapshot changed: %s\n' "$entry" >&2
        return 1
      fi
    done
  }
  assert_upgrade_baseline() {
    assert_upgrade_entries "$1" versions old-generation escaped-generation old-python-generation new-generation pre-commit pre-push sentinel git-config git-index
  }
  render_expected_current_hook() {
    local mode=$1
    local generation_dir=$2
    local expected_install_root=$3
    local expected_hash_tool=$4
    LC_ALL=C
    export LC_ALL
    printf '#!/bin/bash\nset +x +a\nunset GITHUB_COM_TOKEN\nset -euo pipefail\nLC_ALL=C\nexport LC_ALL\nguard_owner=jackin-generated-workflow-guard\nPATH=/usr/bin:/bin:/usr/sbin:/sbin\nexport PATH\nentry_index_present=0\nentry_index=\nif [[ ${GIT_INDEX_FILE+x} ]]; then\n  entry_index_present=1\n  entry_index=$GIT_INDEX_FILE\nfi\n'
    printf 'checker=%q\nmode=%q\ntrusted_install_root=%q\ntrusted_version_dir=%q\nhash_tool=%q\n' "$generation_dir/check-generated-workflows.sh" "$mode" "$expected_install_root" "$generation_dir" "$expected_hash_tool"
    cat <<'HOOK'
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
  restore_upgrade_hooks() {
    cp -p "$test_root/old-pre-commit" "$hooks_path/pre-commit"
    cp -p "$test_root/old-pre-push" "$hooks_path/pre-push"
    chmod 700 "$hooks_path/pre-commit" "$hooks_path/pre-push"
  }
  assert_regular_hook_refusal() {
    local label=$1
    local pre_commit_mode
    local pre_push_mode
    local installer_status
    cp -p "$hooks_path/pre-commit" "$test_root/refusal-pre-commit"
    cp -p "$hooks_path/pre-push" "$test_root/refusal-pre-push"
    pre_commit_mode=$(stat_mode "$hooks_path/pre-commit")
    pre_push_mode=$(stat_mode "$hooks_path/pre-push")
    capture_upgrade_baseline "$label"
    if run_candidate_installer "$test_root/refusal-$label.log"; then
      installer_status=0
    else
      installer_status=$?
    fi
    if [[ "$installer_status" -eq 0 || ! -f "$hooks_path/pre-commit" || -L "$hooks_path/pre-commit" || ! -f "$hooks_path/pre-push" || -L "$hooks_path/pre-push" || "$(stat_mode "$hooks_path/pre-commit")" != "$pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$pre_push_mode" ]] || ! cmp -s "$test_root/refusal-pre-commit" "$hooks_path/pre-commit" || ! cmp -s "$test_root/refusal-pre-push" "$hooks_path/pre-push" || ! assert_upgrade_baseline "$label"; then
      cat "$test_root/refusal-$label.log" >&2
      printf 'generated-workflows: refusal case %s changed hook or unrelated fixture state\n' "$label" >&2
      return 1
    fi
    restore_upgrade_hooks
    printf 'managed-hook upgrade refusal %s: PASS\n' "$label" >> "$test_root/case-results.log"
  }
  assert_old_upgrade_metadata() {
    if [[ "$(sha256_file "$old_manifest")" != "$old_manifest_sha256" || "$(sha256_file "$old_checker")" != "$upgrade_old_checker_sha256" || "$(sha256_file "$old_python_manifest")" != "$old_python_manifest_sha256" || "$(sha256_file "$old_python_inventory")" != "$old_python_inventory_sha256" ]]; then
      printf 'generated-workflows: old-generation artifacts changed during the upgrade fixture\n' >&2
      return 1
    fi
  }
  old_version_dir="$installation/versions/$upgrade_old_checker_sha256"
  old_manifest="$old_version_dir/manifest"
  old_checker="$old_version_dir/check-generated-workflows.sh"
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  chmod 600 "$unrelated_sentinel"
  if [[ -e "$old_version_dir" || -L "$old_version_dir" ]]; then
    printf 'generated-workflows: old generation already exists in the warm candidate fixture\n' >&2
    exit 1
  fi
  old_source_version="$upgrade_old_install_root/versions/$upgrade_old_checker_sha256"
  old_python_version=$(/usr/bin/sed -n 's/^python_version=//p' "$old_source_version/manifest")
  old_python_lock_sha256=$(/usr/bin/sed -n 's/^python_lock_sha256=//p' "$old_source_version/manifest")
  old_python_artifact_sha256=$(/usr/bin/sed -n 's/^python_artifact_sha256=//p' "$old_source_version/manifest")
  old_python_helper_sha256=$(/usr/bin/sed -n 's/^inventory_helper_sha256=//p' "$old_source_version/manifest")
  old_python_source_dir=
  for candidate in "$upgrade_old_install_root/artifacts/python/$old_python_version/$platform-"*; do
    [[ -d "$candidate" && ! -L "$candidate" && -f "$candidate/manifest" && ! -L "$candidate/manifest" ]] || continue
    if /usr/bin/grep -F -x -q "archive_sha256=$old_python_artifact_sha256" "$candidate/manifest" && /usr/bin/grep -F -x -q "lock_sha256=$old_python_lock_sha256" "$candidate/manifest" && /usr/bin/grep -F -x -q "inventory_helper_sha256=$old_python_helper_sha256" "$candidate/manifest"; then
      if [[ -n "$old_python_source_dir" ]]; then
        printf 'generated-workflows: reviewed old source has multiple matching Python generations\n' >&2
        exit 1
      fi
      old_python_source_dir=$candidate
    fi
  done
  if [[ -z "$old_python_source_dir" ]]; then
    printf 'generated-workflows: reviewed old Python artifact generation is unavailable\n' >&2
    exit 2
  fi
  copy_fixture_tree "$old_source_version" "$installation/versions/"
  copy_fixture_tree "$old_python_source_dir" "$installation/artifacts/python/$old_python_version/"
  old_version_dir="$installation/versions/$upgrade_old_checker_sha256"
  old_manifest="$old_version_dir/manifest"
  old_checker="$old_version_dir/check-generated-workflows.sh"
  old_manifest_sha256=$(sha256_file "$old_manifest")
  old_python_artifact_dir="$installation/artifacts/python/$old_python_version/${old_python_source_dir##*/}"
  old_python_manifest="$old_python_artifact_dir/manifest"
  old_python_inventory="$old_python_artifact_dir/inventory"
  old_python_manifest_sha256=$(sha256_file "$old_python_manifest")
  old_python_inventory_sha256=$(sha256_file "$old_python_inventory")
  if [[ "$(sha256_file "$old_checker")" != "$upgrade_old_checker_sha256" ]]; then
    printf 'generated-workflows: copied old generation does not match its digest directory\n' >&2
    exit 1
  fi
  old_cache_found=0
  for cache_manifest in "$installation/cache/tools"/*/manifest; do
    [[ -f "$cache_manifest" && ! -L "$cache_manifest" ]] || continue
    if /usr/bin/grep -F -x -q "checker_sha256=$upgrade_old_checker_sha256" "$cache_manifest"; then
      old_cache_found=1
    fi
  done
  if [[ "$old_cache_found" -ne 1 ]]; then
    printf 'generated-workflows: warm fixture lacks the reviewed old checker cache\n' >&2
    exit 2
  fi
  for input in check-generated-workflows.sh generated-workflow-guard-config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects python-mise.toml python-mise.macos-arm64.lock.toml python-mise.linux-x64.lock.toml install-generated-workflow-guard.sh; do
    copy_reviewed_old_input "$input" "$repository/scripts/$input"
  done
  rm -f "$hooks_path/pre-commit" "$hooks_path/pre-push"
  if ! (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/old-installer.log" 2>&1; then
    cat "$test_root/old-installer.log" >&2
    printf 'generated-workflows: reviewed old installer could not construct the disposable prior-generation hook pair\n' >&2
    exit 1
  fi
  cp -p "$hooks_path/pre-commit" "$test_root/old-pre-commit"
  cp -p "$hooks_path/pre-push" "$test_root/old-pre-push"
  if [[ "$(stat_mode "$hooks_path/pre-commit")" != 700 || "$(stat_mode "$hooks_path/pre-push")" != 700 ]]; then
    printf 'generated-workflows: reviewed old installer did not create canonical 0700 hook files\n' >&2
    exit 1
  fi
  render_expected_current_hook pre-commit "$old_version_dir" "$installation" "$hash_tool" > "$test_root/expected-old-pre-commit"
  render_expected_current_hook pre-push "$old_version_dir" "$installation" "$hash_tool" > "$test_root/expected-old-pre-push"
  if ! cmp -s "$test_root/expected-old-pre-commit" "$hooks_path/pre-commit" || ! cmp -s "$test_root/expected-old-pre-push" "$hooks_path/pre-push"; then
    printf 'generated-workflows: reviewed d29 installer output differs from the independent old launcher render\n' >&2
    exit 1
  fi
  if ! (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/old-installer-rerun.log" 2>&1 || ! cmp -s "$test_root/old-pre-commit" "$hooks_path/pre-commit" || ! cmp -s "$test_root/old-pre-push" "$hooks_path/pre-push"; then
    cat "$test_root/old-installer-rerun.log" >&2
    printf 'generated-workflows: prior installer did not recognize its own full canonical launcher pair\n' >&2
    exit 1
  fi
  for input in check-generated-workflows.sh generated-workflow-guard-config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects python-mise.toml python-mise.macos-arm64.lock.toml python-mise.linux-x64.lock.toml install-generated-workflow-guard.sh; do
    cp -p "$repo_root/scripts/$input" "$repository/scripts/$input"
  done
  for input in trusted-seed-cargo.lock trusted-seed-cargo-policy.json; do
    if [[ -f "$repo_root/scripts/$input" && ! -L "$repo_root/scripts/$input" ]]; then
      cp -p "$repo_root/scripts/$input" "$repository/scripts/$input"
    fi
  done
  if [[ -n "$installer_mutant" ]]; then
    apply_installer_mutant
  fi
  new_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
  new_installed_manifest="$installation/versions/$new_checker_sha256/manifest"
  new_checker="$installation/versions/$new_checker_sha256/check-generated-workflows.sh"
  new_version_dir="$installation/versions/$new_checker_sha256"
  if [[ "$new_checker_sha256" == "$upgrade_old_checker_sha256" || ! -f "$new_installed_manifest" || -L "$new_installed_manifest" ]]; then
    printf 'generated-workflows: upgrade fixture does not contain a distinct installed new checker generation\n' >&2
    exit 1
  fi
  old_manifest_fields=$(/usr/bin/awk 'END { print NR }' "$old_manifest")
  new_manifest_fields=$(/usr/bin/awk 'END { print NR }' "$new_installed_manifest")
  new_seed_lock_sha256=$(/usr/bin/sed -n 's/^trusted_seed_cargo_lock_sha256=//p' "$new_installed_manifest")
  new_seed_policy_sha256=$(/usr/bin/sed -n 's/^trusted_seed_cargo_policy_sha256=//p' "$new_installed_manifest")
  if [[ "$old_manifest_fields" -ne 16 || "$new_manifest_fields" -ne 18 || -e "$old_version_dir/trusted-seed-cargo.lock" || -L "$old_version_dir/trusted-seed-cargo.lock" || -e "$old_version_dir/trusted-seed-cargo-policy.json" || -L "$old_version_dir/trusted-seed-cargo-policy.json" || ! -f "$new_version_dir/trusted-seed-cargo.lock" || -L "$new_version_dir/trusted-seed-cargo.lock" || "$(sha256_file "$new_version_dir/trusted-seed-cargo.lock")" != "$new_seed_lock_sha256" || ! -f "$new_version_dir/trusted-seed-cargo-policy.json" || -L "$new_version_dir/trusted-seed-cargo-policy.json" || "$(sha256_file "$new_version_dir/trusted-seed-cargo-policy.json")" != "$new_seed_policy_sha256" ]]; then
    printf 'generated-workflows: reviewed 16-field old and 18-field new trusted Cargo artifact manifests are not coherent\n' >&2
    exit 1
  fi
  git_config_before=$(sha256_file "$common_dir/config")
  git_index_before=$(sha256_file "$common_dir/index")
  new_generation_before=$(sha256_file "$new_installed_manifest")
  if [[ "$installer_mutant" == pre-push-publication-failure || "$installer_mutant" == pre-push-publication-failure-accepted-mode || "$installer_mutant" == pre-push-publication-failure-with-late-update || "$installer_mutant" == late-precommit-replacement || "$installer_mutant" == late-prepush-replacement ]]; then
    if [[ "$installer_mutant" == pre-push-publication-failure-accepted-mode ]]; then
      chmod 755 "$hooks_path/pre-commit"
      chmod 750 "$hooks_path/pre-push"
    fi
    changed_hook=pre-push
    unchanged_hook=pre-commit
    if [[ "$installer_mutant" == late-precommit-replacement ]]; then
      changed_hook=pre-commit
      unchanged_hook=pre-push
    fi
    if [[ "$installer_mutant" != pre-push-publication-failure && "$installer_mutant" != pre-push-publication-failure-accepted-mode ]]; then
      printf '#!/bin/bash\nforeign late %s\n' "$changed_hook" > "$test_root/expected-foreign-$changed_hook"
      chmod 700 "$test_root/expected-foreign-$changed_hook"
    fi
    pre_commit_initial_mode=$(stat_mode "$hooks_path/pre-commit")
    pre_push_initial_mode=$(stat_mode "$hooks_path/pre-push")
    capture_upgrade_baseline "$installer_mutant"
    if run_candidate_installer "$test_root/$installer_mutant.log"; then
      printf 'generated-workflows: controlled publication fault unexpectedly succeeded\n' >&2
      exit 1
    fi
    if [[ "$installer_mutant" == pre-push-publication-failure || "$installer_mutant" == pre-push-publication-failure-accepted-mode ]]; then
      if ! cmp -s "$test_root/old-pre-commit" "$hooks_path/pre-commit" || ! cmp -s "$test_root/old-pre-push" "$hooks_path/pre-push" || "$(stat_mode "$hooks_path/pre-commit")" != "$pre_commit_initial_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$pre_push_initial_mode"; then
        cat "$test_root/$installer_mutant.log" >&2
        printf 'generated-workflows: failed second publication did not restore both verified old hook bodies and modes\n' >&2
        exit 1
      fi
      assert_upgrade_baseline "$installer_mutant"
      printf 'managed-hook second-publication failure restores the old pair and accepted modes: PASS\n'
    elif [[ "$installer_mutant" == pre-push-publication-failure-with-late-update ]]; then
      if ! cmp -s "$test_root/old-pre-commit" "$hooks_path/pre-commit" || "$(stat_mode "$hooks_path/pre-commit")" != 700 || ! cmp -s "$test_root/expected-foreign-pre-push" "$hooks_path/pre-push" || "$(stat_mode "$hooks_path/pre-push")" != 700; then
        cat "$test_root/$installer_mutant.log" >&2
        printf 'generated-workflows: failed second publication did not restore its own peer and preserve the late foreign hook\n' >&2
        exit 1
      fi
      assert_upgrade_entries "$installer_mutant" versions old-generation escaped-generation old-python-generation new-generation pre-commit sentinel git-config git-index
      printf 'managed-hook second-publication failure restores the verified peer and preserves the late update: PASS\n'
    else
      if ! cmp -s "$test_root/expected-foreign-$changed_hook" "$hooks_path/$changed_hook" || "$(stat_mode "$hooks_path/$changed_hook")" != 700 || ! cmp -s "$test_root/old-$unchanged_hook" "$hooks_path/$unchanged_hook"; then
        cat "$test_root/$installer_mutant.log" >&2
        printf 'generated-workflows: late foreign %s hook was overwritten or changed its peer\n' "$changed_hook" >&2
        exit 1
      fi
      assert_upgrade_entries "$installer_mutant" versions old-generation escaped-generation old-python-generation new-generation "$unchanged_hook" sentinel git-config git-index
      printf 'managed-hook late %s replacement is refused and preserved: PASS\n' "$changed_hook"
    fi
    record_completion_witness "focus:$e2e_focus"
    mark_completion_complete
    exit 0
  fi
  chmod 700 "$hooks_path/pre-commit" "$hooks_path/pre-push"
  for target in pre-commit pre-push; do
    restore_upgrade_hooks
    hook_path="$hooks_path/$target"
    printf '#!/bin/bash\nguard_owner=jackin-generated-workflow-guard\nraw_path=%s/versions/foreign\n' "$installation" > "$hook_path"
    chmod 700 "$hook_path"
    assert_regular_hook_refusal "forged-marker-$target"
    restore_upgrade_hooks
    printf '\nappended foreign command\n' >> "$hook_path"
    assert_regular_hook_refusal "altered-$target"
    restore_upgrade_hooks
    chmod 777 "$hook_path"
    assert_regular_hook_refusal "unsafe-mode-$target"
  done
  restore_upgrade_hooks
  cp -p "$test_root/old-pre-push" "$hooks_path/pre-commit"
  chmod 700 "$hooks_path/pre-commit"
  assert_regular_hook_refusal swapped-hook-mode
  restore_upgrade_hooks
  old_python_executable="$old_python_artifact_dir/runtime/bin/python3.15"
  old_python_executable_mode=$(stat_mode "$old_python_executable")
  if [[ "$old_python_executable_mode" != 555 ]]; then
    printf 'generated-workflows: reviewed old Python executable is not immutable mode 0555\n' >&2
    exit 1
  fi
  chmod 777 "$old_python_executable"
  assert_regular_hook_refusal unsafe-python-executable-mode
  chmod "$old_python_executable_mode" "$old_python_executable"
  old_python_stdlib_relative=$(python3 -I -S - "$old_python_inventory" <<'PY'
import sys

for line in open(sys.argv[1], encoding="utf-8"):
    fields = line.rstrip("\n").split("\t")
    if fields[0] == "file" and fields[3].startswith("lib/python3.15/") and fields[3].endswith(".py") and int(fields[1], 8) & 0o111 == 0:
        print(fields[3])
        break
PY
)
  if [[ -z "$old_python_stdlib_relative" ]]; then
    printf 'generated-workflows: old Python inventory has no non-executable standard-library source file\n' >&2
    exit 2
  fi
  old_python_stdlib_file="$old_python_artifact_dir/runtime/$old_python_stdlib_relative"
  old_python_stdlib_mode=$(stat_mode "$old_python_stdlib_file")
  old_python_stdlib_sha256=$(/usr/bin/awk -F '\t' -v path="$old_python_stdlib_relative" '$1 == "file" && $4 == path { count++; hash = $3 } END { if (count != 1) exit 1; print hash }' "$old_python_inventory")
  if [[ "$(sha256_file "$old_python_stdlib_file")" != "$old_python_stdlib_sha256" || $((8#$old_python_stdlib_mode & 0111)) -ne 0 ]]; then
    printf 'generated-workflows: selected standard-library member does not match its non-executable inventory row\n' >&2
    exit 2
  fi
  cp -p "$old_python_stdlib_file" "$test_root/old-python-stdlib.before-corruption"
  chmod u+w "$old_python_stdlib_file"
  printf '\nmanaged upgrade inventory corruption\n' >> "$old_python_stdlib_file"
  chmod "$old_python_stdlib_mode" "$old_python_stdlib_file"
  if [[ "$(sha256_file "$old_python_stdlib_file")" == "$old_python_stdlib_sha256" ]]; then
    printf 'generated-workflows: standard-library corruption did not change bytes\n' >&2
    exit 2
  fi
  assert_regular_hook_refusal changed-python-standard-library-member
  chmod u+w "$old_python_stdlib_file"
  cp "$test_root/old-python-stdlib.before-corruption" "$old_python_stdlib_file"
  chmod "$old_python_stdlib_mode" "$old_python_stdlib_file"
  if [[ "$(sha256_file "$old_python_stdlib_file")" != "$old_python_stdlib_sha256" ]]; then
    printf 'generated-workflows: standard-library fixture restoration changed its recorded bytes\n' >&2
    exit 1
  fi
  old_python_runtime="$old_python_artifact_dir/runtime"
  old_python_runtime_mode=$(stat_mode "$old_python_runtime")
  old_python_bin_mode=$(stat_mode "$old_python_runtime/bin")
  old_python_executable_sha256=$(sha256_file "$old_python_executable")
  old_python_inventory_executable_sha256=$(/usr/bin/awk -F '\t' '$1 == "file" && $4 == "bin/python3.15" { count++; hash = $3 } END { if (count != 1) exit 1; print hash }' "$old_python_inventory")
  if [[ "$old_python_executable_sha256" != "$old_python_inventory_executable_sha256" ]]; then
    printf 'generated-workflows: old Python executable does not match its recorded inventory row\n' >&2
    exit 2
  fi
  copy_fixture_tree "$old_python_runtime/bin" "$test_root/outside-python-bin"
  if [[ "$(sha256_file "$test_root/outside-python-bin/python3.15")" != "$old_python_inventory_executable_sha256" || "$(stat_mode "$test_root/outside-python-bin/python3.15")" != 555 ]]; then
    printf 'generated-workflows: outside bin fixture did not preserve the executable bytes and mode\n' >&2
    exit 2
  fi
  chmod u+w "$old_python_runtime/bin"
  chmod u+w "$old_python_runtime"
  rm -rf "$old_python_runtime/bin"
  ln -s "$test_root/outside-python-bin" "$old_python_runtime/bin"
  chmod "$old_python_runtime_mode" "$old_python_runtime"
  if [[ ! -L "$old_python_runtime/bin" || "$(sha256_file "$old_python_runtime/bin/python3.15")" != "$old_python_inventory_executable_sha256" || "$(stat_mode "$old_python_runtime/bin/python3.15")" != 555 ]]; then
    printf 'generated-workflows: outside bin symlink did not preserve the inventoried executable\n' >&2
    exit 2
  fi
  assert_regular_hook_refusal outside-bin-ancestor-symlink
  chmod u+w "$old_python_runtime"
  rm "$old_python_runtime/bin"
  case "$(uname -s)" in
    Darwin) /bin/cp -cRp "$test_root/outside-python-bin" "$old_python_runtime/bin" ;;
    Linux) /bin/cp -a --reflink=auto "$test_root/outside-python-bin" "$old_python_runtime/bin" ;;
    *) exit 2 ;;
  esac
  chmod "$old_python_bin_mode" "$old_python_runtime/bin"
  chmod "$old_python_runtime_mode" "$old_python_runtime"
  if [[ "$(stat_mode "$old_python_runtime/bin")" != "$old_python_bin_mode" ]]; then
    printf 'generated-workflows: restored Python bin directory mode differs from the source fixture\n' >&2
    exit 1
  fi
  cp -p "$old_manifest" "$test_root/old-generation-manifest"
  chmod u+w "$old_version_dir"
  chmod u+w "$old_manifest"
  printf 'checker_sha256=%s\n' "$upgrade_old_checker_sha256" >> "$old_manifest"
  chmod 400 "$old_manifest"
  chmod 500 "$old_version_dir"
  assert_regular_hook_refusal duplicate-generation-manifest-key
  chmod u+w "$old_version_dir"
  cp -p "$test_root/old-generation-manifest" "$old_manifest"
  chmod 400 "$old_manifest"
  chmod 500 "$old_version_dir"
  /usr/bin/sed '/^python_lock_sha256=/d' "$old_manifest" > "$test_root/old-generation-manifest.missing"
  chmod u+w "$old_version_dir"
  cp "$test_root/old-generation-manifest.missing" "$old_manifest"
  chmod 400 "$old_manifest"
  chmod 500 "$old_version_dir"
  assert_regular_hook_refusal missing-generation-manifest-field
  chmod u+w "$old_version_dir"
  cp -p "$test_root/old-generation-manifest" "$old_manifest"
  chmod 400 "$old_manifest"
  chmod 500 "$old_version_dir"
  cp -p "$old_checker" "$test_root/old-generation-checker"
  chmod u+w "$old_checker"
  printf '\nchanged checker bytes\n' >> "$old_checker"
  chmod 400 "$old_checker"
  assert_regular_hook_refusal checker-digest-mismatch
  chmod u+w "$old_version_dir"
  cp -p "$test_root/old-generation-checker" "$old_checker"
  chmod 400 "$old_checker"
  chmod 500 "$old_version_dir"
  cp -p "$old_python_manifest" "$test_root/old-python-artifact-manifest"
  chmod u+w "$old_python_artifact_dir"
  chmod u+w "$old_python_manifest"
  /usr/bin/sed 's/^backend=core:python$/backend=core:unknown/' "$old_python_manifest" > "$test_root/old-python-artifact-manifest.changed"
  chmod 400 "$test_root/old-python-artifact-manifest.changed"
  mv "$test_root/old-python-artifact-manifest.changed" "$old_python_manifest"
  chmod 500 "$old_python_artifact_dir"
  assert_regular_hook_refusal python-artifact-manifest-mismatch
  chmod u+w "$old_python_artifact_dir"
  cp -p "$test_root/old-python-artifact-manifest" "$old_python_manifest"
  chmod 500 "$old_python_artifact_dir"
  chmod 700 "$old_version_dir"
  assert_regular_hook_refusal nonprivate-generation-directory
  chmod 500 "$old_version_dir"
  versions_directory="$installation/versions"
  versions_directory_mode=$(stat_mode "$versions_directory")
  chmod u+w "$versions_directory"
  mv "$new_version_dir" "$test_root/escaped-new-generation"
  ln -s "$test_root/escaped-new-generation" "$new_version_dir"
  chmod "$versions_directory_mode" "$versions_directory"
  capture_upgrade_baseline escaped-current-generation
  if run_candidate_installer "$test_root/escaped-current-generation.log" || ! assert_upgrade_baseline escaped-current-generation; then
    cat "$test_root/escaped-current-generation.log" >&2
    printf 'generated-workflows: symlinked current generation was not refused with all generation and hook state unchanged\n' >&2
    exit 1
  fi
  chmod u+w "$versions_directory"
  rm "$new_version_dir"
  mv "$test_root/escaped-new-generation" "$new_version_dir"
  chmod "$versions_directory_mode" "$versions_directory"
  printf 'symlinked generation path escape: REJECTED with complete install and hook state unchanged\n' >> "$test_root/case-results.log"
  for entry in $(seq 1 65); do mkdir "$versions_directory/.stage.upgrade-$entry"; done
  assert_regular_hook_refusal excessive-generation-enumeration
  for entry in $(seq 1 65); do rmdir "$versions_directory/.stage.upgrade-$entry"; done
  restore_upgrade_hooks
  cp -p "$hooks_path/pre-commit" "$test_root/symlinked-hook-target"
  rm "$hooks_path/pre-commit"
  ln -s "$test_root/symlinked-hook-target" "$hooks_path/pre-commit"
  capture_upgrade_baseline symlinked-hook
  pre_push_hash=$(sha256_file "$hooks_path/pre-push")
  if run_candidate_installer "$test_root/symlinked-hook.log" || [[ ! -L "$hooks_path/pre-commit" || "$(readlink "$hooks_path/pre-commit")" != "$test_root/symlinked-hook-target" || "$(sha256_file "$hooks_path/pre-push")" != "$pre_push_hash" ]] || ! assert_upgrade_baseline symlinked-hook; then
    cat "$test_root/symlinked-hook.log" >&2
    printf 'generated-workflows: symlinked managed hook was not refused and preserved\n' >&2
    exit 1
  fi
  rm "$hooks_path/pre-commit"
  cp -p "$test_root/old-pre-commit" "$hooks_path/pre-commit"
  chmod 700 "$hooks_path/pre-commit"
  cp -p "$hooks_path/pre-push" "$test_root/directory-hook-target"
  rm "$hooks_path/pre-push"
  mkdir "$hooks_path/pre-push"
  capture_upgrade_baseline directory-hook
  pre_commit_hash=$(sha256_file "$hooks_path/pre-commit")
  if run_candidate_installer "$test_root/directory-hook.log" || [[ ! -d "$hooks_path/pre-push" || -L "$hooks_path/pre-push" || "$(sha256_file "$hooks_path/pre-commit")" != "$pre_commit_hash" ]] || ! assert_upgrade_baseline directory-hook; then
    cat "$test_root/directory-hook.log" >&2
    printf 'generated-workflows: directory hook was not refused and preserved\n' >&2
    exit 1
  fi
  rmdir "$hooks_path/pre-push"
  cp -p "$test_root/old-pre-push" "$hooks_path/pre-push"
  chmod 700 "$hooks_path/pre-push"
  capture_upgrade_baseline positive-upgrade
  if ! run_candidate_installer "$test_root/upgrade.log"; then
    cat "$test_root/upgrade.log" >&2
    printf 'generated-workflows: candidate installer refused the real prior-generation hook pair\n' >&2
    exit 1
  fi
  printf -v escaped_new_checker '%q' "$installation/versions/$new_checker_sha256/check-generated-workflows.sh"
  printf -v escaped_new_version_dir '%q' "$installation/versions/$new_checker_sha256"
  for target in pre-commit pre-push; do
    mode=
    case "$target" in
      pre-commit) mode=pre-commit ;;
      pre-push) mode=pre-push ;;
      *) printf 'generated-workflows: unsupported hook mode in lifecycle fixture\n' >&2; exit 2 ;;
    esac
    /usr/bin/grep -F -x -q "checker=$escaped_new_checker" "$hooks_path/$target"
    /usr/bin/grep -F -x -q "trusted_version_dir=$escaped_new_version_dir" "$hooks_path/$target"
    /usr/bin/grep -F -x -q "mode=$mode" "$hooks_path/$target"
    [[ "$(stat_mode "$hooks_path/$target")" == 700 ]]
  done
  render_expected_current_hook pre-commit "$new_version_dir" "$installation" "$hash_tool" > "$test_root/expected-new-pre-commit"
  render_expected_current_hook pre-push "$new_version_dir" "$installation" "$hash_tool" > "$test_root/expected-new-pre-push"
  if ! cmp -s "$test_root/expected-new-pre-commit" "$hooks_path/pre-commit" || ! cmp -s "$test_root/expected-new-pre-push" "$hooks_path/pre-push" || [[ "$(sha256_file "$new_checker")" != "$new_checker_sha256" || "$new_checker_sha256" == "$upgrade_old_checker_sha256" || "$(sha256_file "$hooks_path/pre-commit")" == "$(sha256_file "$hooks_path/pre-push")" ]]; then
    printf 'generated-workflows: upgraded hook pair is not a distinct current-format pair\n' >&2
    exit 1
  fi
  assert_upgrade_entries positive-upgrade versions old-generation escaped-generation old-python-generation new-generation sentinel git-config git-index
  upgraded_pre_commit_hash=$(sha256_file "$hooks_path/pre-commit")
  upgraded_pre_push_hash=$(sha256_file "$hooks_path/pre-push")
  upgraded_pre_commit_mode=$(stat_mode "$hooks_path/pre-commit")
  upgraded_pre_push_mode=$(stat_mode "$hooks_path/pre-push")
  capture_upgrade_baseline upgrade-rerun
  if ! run_candidate_installer "$test_root/upgrade-rerun.log" || [[ "$(sha256_file "$hooks_path/pre-commit")" != "$upgraded_pre_commit_hash" || "$(sha256_file "$hooks_path/pre-push")" != "$upgraded_pre_push_hash" || "$(stat_mode "$hooks_path/pre-commit")" != "$upgraded_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$upgraded_pre_push_mode" ]] || ! assert_upgrade_baseline upgrade-rerun; then
    cat "$test_root/upgrade-rerun.log" >&2
    printf 'generated-workflows: current-generation rerun changed canonical hook bytes or modes\n' >&2
    exit 1
  fi
  cp -p "$test_root/old-pre-push" "$hooks_path/pre-push"
  capture_upgrade_baseline interrupted-pair
  if ! run_candidate_installer "$test_root/interrupted-pair.log" || [[ "$(sha256_file "$hooks_path/pre-commit")" != "$upgraded_pre_commit_hash" || "$(sha256_file "$hooks_path/pre-push")" != "$upgraded_pre_push_hash" ]] || ! assert_upgrade_entries interrupted-pair versions old-generation escaped-generation old-python-generation new-generation sentinel git-config git-index; then
    cat "$test_root/interrupted-pair.log" >&2
    printf 'generated-workflows: installer did not recover a coherent current/old interrupted pair\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$old_manifest")" != "$old_manifest_sha256" || "$(sha256_file "$old_checker")" != "$upgrade_old_checker_sha256" || "$(sha256_file "$old_python_manifest")" != "$old_python_manifest_sha256" || "$(sha256_file "$old_python_inventory")" != "$old_python_inventory_sha256" || "$(sha256_file "$new_installed_manifest")" != "$new_generation_before" || "$(sha256_file "$common_dir/config")" != "$git_config_before" || "$(sha256_file "$common_dir/index")" != "$git_index_before" || "$(/bin/cat "$unrelated_sentinel")" != 'preserve this unrelated install payload' ]]; then
    printf 'generated-workflows: upgrade changed installed metadata, Git settings, index, or unrelated sentinel\n' >&2
    exit 1
  fi
  assert_old_upgrade_metadata
  printf '\nmanaged hook upgrade qualification\n' >> "$repository/README.md"
  (cd "$repository" && "$git_command" add README.md)
  if ! GIT_TRACE="$test_root/commit.trace" HOME="$home_dir" "$git_command" -C "$repository" commit --quiet -m 'test: qualify managed hook upgrade'; then
    cat "$test_root/commit.trace" >&2
    printf 'generated-workflows: actual Git pre-commit dispatch failed after managed-hook upgrade\n' >&2
    exit 1
  fi
  if ! /usr/bin/grep -F -q "$hooks_path/pre-commit" "$test_root/commit.trace"; then
    printf 'generated-workflows: Git commit trace does not show the registered pre-commit hook\n' >&2
    exit 1
  fi
  commit_identity=$($git_command -C "$repository" log -1 --format='%an <%ae>%n%cn <%ce>')
  current_branch=$($git_command -C "$repository" symbolic-ref --short HEAD)
  commit_hash=$($git_command -C "$repository" rev-parse HEAD)
  if [[ "$commit_identity" != $'Generated workflow qualification <generated-workflow@example.invalid>\nGenerated workflow qualification <generated-workflow@example.invalid>' || "$current_branch" == managed-hook-upgrade ]]; then
    printf 'generated-workflows: real qualification commit identity or non-HEAD push target is inconsistent\n' >&2
    exit 1
  fi
  if ! GIT_TRACE="$test_root/push.trace" HOME="$home_dir" "$git_command" -C "$repository" push --quiet origin HEAD:refs/heads/managed-hook-upgrade; then
    cat "$test_root/push.trace" >&2
    printf 'generated-workflows: actual Git non-HEAD push dispatch failed after managed-hook upgrade\n' >&2
    exit 1
  fi
  if ! /usr/bin/grep -F -q "$hooks_path/pre-push" "$test_root/push.trace"; then
    printf 'generated-workflows: Git push trace does not show the registered pre-push hook\n' >&2
    exit 1
  fi
  if [[ "$(git --git-dir "$remote" rev-parse refs/heads/managed-hook-upgrade)" != "$commit_hash" ]]; then
    printf 'generated-workflows: non-HEAD push did not update the local remote ref to the qualifying commit\n' >&2
    exit 1
  fi
  assert_upgrade_entries positive-upgrade versions old-generation escaped-generation old-python-generation new-generation sentinel git-config
  printf 'prior reviewed installer upgrade, idempotent rerun, mixed-pair recovery, actual Git commit and non-HEAD push: PASS\n' >> "$test_root/case-results.log"
  printf 'managed hook upgrade lifecycle: PASS\n'
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == installer-repair-mise-http-failure || "$e2e_focus" == installer-repair-mise-wrong-hash ]]; then
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  chmod 600 "$unrelated_sentinel"
  save_hook_baseline
  run_installed_hooks
  openssl_command=$(command -v openssl || true)
  if [[ -z "$openssl_command" || "$openssl_command" != /* ]]; then
    printf 'generated-workflows: OpenSSL is unavailable for the loopback TLS network fixture\n' >&2
    exit 2
  fi
  fixture_ca="$test_root/network-fixture-ca.pem"
  fixture_key="$test_root/network-fixture-key.pem"
  fixture_config="$test_root/network-fixture-openssl.cnf"
  fixture_body="$test_root/network-fixture-response.bin"
  fixture_port_file="$test_root/network-fixture-port"
  fixture_request_log="$test_root/network-fixture-request.log"
  fixture_expected_path=/fixture/mise
  fixture_status=200
  if [[ "$e2e_focus" == installer-repair-mise-http-failure ]]; then
    fixture_status=503
    printf 'synthetic loopback HTTP failure fixture\n' > "$fixture_body"
  else
    printf 'synthetic loopback wrong-hash payload fixture %s\n' "$test_root" > "$fixture_body"
  fi
  cat > "$fixture_config" <<'OPENSSL'
[req]
prompt = no
distinguished_name = distinguished_name
x509_extensions = v3_req
[distinguished_name]
CN = 127.0.0.1
[v3_req]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,digitalSignature,keyEncipherment
subjectAltName = IP:127.0.0.1
OPENSSL
  if ! "$openssl_command" req -x509 -newkey rsa:2048 -nodes -sha256 -days 1 -keyout "$fixture_key" -out "$fixture_ca" -config "$fixture_config" -extensions v3_req > /dev/null 2>&1; then
    printf 'generated-workflows: could not create the synthetic loopback TLS certificate\n' >&2
    exit 2
  fi
  chmod 600 "$fixture_key"
  chmod 400 "$fixture_ca"
  network_body_sha=$(sha256_file "$fixture_body")
  python3_command=$(command -v python3)
  if [[ "$python3_command" != /* ]]; then
    python3_command="$(pwd -P)/$python3_command"
  fi
  /usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin LC_ALL=C LANG=C "$python3_command" -I -S - "$fixture_port_file" "$fixture_request_log" "$fixture_ca" "$fixture_key" "$fixture_body" "$fixture_status" "$fixture_expected_path" <<'PY' > "$test_root/network-fixture-server.stdout" 2> "$test_root/network-fixture-server.stderr" &
import ssl
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

port_file, request_log, certificate, private_key, body_path, response_status, expected_path = sys.argv[1:]
request_log = Path(request_log)
body = Path(body_path).read_bytes()
expected_status = int(response_status)
expected_peer = "127.0.0.1"

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        peer = self.client_address[0]
        valid_request = self.command == "GET" and self.path == expected_path and peer == expected_peer
        status = expected_status if valid_request else 404
        request_log.write_text(
            f"request_count=1 peer={peer} method={self.command} path={self.path} status={status}\n",
            encoding="ascii",
        )
        self.send_response(status)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        return

server = HTTPServer((expected_peer, 0), Handler)
server.request_timeout = 20
server.timeout = server.request_timeout
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(certificate, private_key)
server.socket = context.wrap_socket(server.socket, server_side=True)
Path(port_file).write_text(f"{server.server_address[1]}\n", encoding="ascii")
server.handle_request()
server.server_close()
if not request_log.exists():
    request_log.write_text("request_count=0 timed_out=true\n", encoding="ascii")
PY
  network_fixture_pid=$!
  network_fixture_child_pid=$network_fixture_pid
  network_fixture_started_seconds=$SECONDS
  for ((attempt = 0; attempt < 200; attempt++)); do
    [[ -s "$fixture_port_file" ]] && break
    if ! kill -0 "$network_fixture_pid" 2>/dev/null; then
      cat "$test_root/network-fixture-server.stderr" >&2
      printf 'generated-workflows: loopback TLS server did not become ready\n' >&2
      exit 1
    fi
    sleep 0.05
  done
  if [[ ! -s "$fixture_port_file" ]]; then
    printf 'generated-workflows: loopback TLS server readiness exceeded its bounded wait\n' >&2
    exit 1
  fi
  fixture_port=$(/bin/cat "$fixture_port_file")
  if [[ ! "$fixture_port" =~ ^[0-9]+$ || "$fixture_port" -lt 1 || "$fixture_port" -gt 65535 ]]; then
    printf 'generated-workflows: loopback TLS server selected an invalid ephemeral port\n' >&2
    exit 1
  fi
  cp "$fixture_ca" "$installation/.network-fixture-ca.pem"
  chmod 400 "$installation/.network-fixture-ca.pem"
  python3 -I -S - "$repository/scripts/install-generated-workflow-guard.sh" "$fixture_port" "$fixture_expected_path" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
port, expected_path = sys.argv[2:]
source = path.read_text()
old_url = '"https://github.com/jdx/mise/releases/download/v$mise_version/mise-v$mise_version-$platform"'
new_url = f'"https://127.0.0.1:{port}{expected_path}"'
curl_marker = '/usr/bin/curl --disable --proto'
curl_replacement = '/usr/bin/curl --disable --cacert "$install_root/.network-fixture-ca.pem" --proto'
execution_marker = '  if ! env -i "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" config ls --json > "$python_stage/config-ls.json" 2> "$python_stage/config-ls.log"; then\n'
execution_guard = (
    '  if [[ "$(sha256_file "$mise_artifact")" != "$mise_sha256" ]]; then\n'
    '    printf \'generated-workflows: test harness refused to execute a mismatched Mise payload\\n\' >&2\n'
    '    exit 98\n'
    '  fi\n'
)
if source.count(old_url) != 1 or source.count(curl_marker) != 1 or source.count(execution_marker) != 1:
    raise SystemExit("generated-workflows: loopback network fixture did not match the copied installer exactly")
source = source.replace(old_url, new_url, 1)
source = source.replace(curl_marker, curl_replacement, 1)
source = source.replace(execution_marker, execution_guard + execution_marker, 1)
path.write_text(source)
PY
  mise_version_value=$(manifest_value mise_version)
  mise_sha_value=$(manifest_value mise_sha256)
  mise_artifact="$installation/artifacts/mise/$mise_version_value/$platform/mise"
  if [[ ! -f "$mise_artifact" || -L "$mise_artifact" || "$(sha256_file "$mise_artifact")" != "$mise_sha_value" ]]; then
    printf 'generated-workflows: qualified Mise artifact is unavailable before download failure case\n' >&2
    exit 1
  fi
  original_mise_mode=$(stat_mode "$mise_artifact")
  original_sentinel_sha=$(sha256_file "$unrelated_sentinel")
  original_sentinel_mode=$(stat_mode "$unrelated_sentinel")
  chmod u+w "$mise_artifact"
  printf 'corrupt-mise-network-fixture:%s\n' "$test_root" > "$mise_artifact"
  chmod "$original_mise_mode" "$mise_artifact"
  damaged_mise_sha=$(sha256_file "$mise_artifact")
  if [[ "$damaged_mise_sha" == "$mise_sha_value" || ( "$e2e_focus" == installer-repair-mise-wrong-hash && "$network_body_sha" == "$mise_sha_value" ) ]]; then
    printf 'generated-workflows: Mise network corruption or response fixture unexpectedly matched the pinned hash\n' >&2
    exit 1
  fi
  mise_artifact_parent=$(dirname "$mise_artifact")
  if ! write_sorted_find_results "$mise_artifact_parent" "$test_root/mise-quarantine-before.raw" "$test_root/mise-quarantine-before.list" -mindepth 1 -maxdepth 1 -type d -name '.quarantine.*'; then
    printf 'generated-workflows: cannot enumerate Mise quarantines before repair\n' >&2
    exit 1
  fi
  installer_status=0
  if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/mise-network-repair.log" 2>&1; then
    installer_status=0
  else
    installer_status=$?
  fi
  server_status=0
  if wait "$network_fixture_pid"; then
    server_status=0
  else
    server_status=$?
  fi
  network_fixture_elapsed_seconds=$((SECONDS - network_fixture_started_seconds))
  network_fixture_pid=
  /bin/rm -f "$installation/.network-fixture-ca.pem"
  request_record=$(/bin/cat "$fixture_request_log" 2>/dev/null || true)
  if [[ "$server_status" -ne 0 || "$request_record" != "request_count=1 peer=127.0.0.1 method=GET path=$fixture_expected_path status=$fixture_status" ]]; then
    cat "$test_root/network-fixture-server.stderr" >&2
    printf 'generated-workflows: actual curl did not make exactly the intended loopback-only TLS request\n' >&2
    exit 1
  fi
  if ! cmp -s "$test_root/pre-commit.before" "$hooks_path/pre-commit" || ! cmp -s "$test_root/pre-push.before" "$hooks_path/pre-push" || [[ "$(stat_mode "$hooks_path/pre-commit")" != "$saved_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$saved_pre_push_mode" || "$(sha256_file "$unrelated_sentinel")" != "$original_sentinel_sha" || "$(stat_mode "$unrelated_sentinel")" != "$original_sentinel_mode" ]]; then
    printf 'generated-workflows: Mise download failure changed canonical hooks or unrelated sentinel state\n' >&2
    exit 1
  fi
  quarantined_mise=
  quarantine_count=0
  if ! write_sorted_find_results "$mise_artifact_parent" "$test_root/mise-quarantine-after.raw" "$test_root/mise-quarantine-after.list" -mindepth 1 -maxdepth 1 -type d -name '.quarantine.*'; then
    printf 'generated-workflows: cannot enumerate Mise quarantines after repair\n' >&2
    exit 1
  fi
  while IFS= read -r quarantine_parent; do
    if list_contains_exact_line "$quarantine_parent" "$test_root/mise-quarantine-before.list"; then
      continue
    else
      quarantine_list_status=$?
      if [[ "$quarantine_list_status" -ne 1 ]]; then
        printf 'generated-workflows: cannot compare Mise quarantine listings\n' >&2
        exit 1
      fi
    fi
    quarantined_artifact="$quarantine_parent/artifact"
    if [[ -f "$quarantined_artifact" && ! -L "$quarantined_artifact" && "$(sha256_file "$quarantined_artifact")" == "$damaged_mise_sha" ]]; then
      quarantine_count=$((quarantine_count + 1))
      quarantined_mise=$quarantined_artifact
    fi
  done < "$test_root/mise-quarantine-after.list"
  if [[ "$quarantine_count" -ne 1 || "$(stat_mode "$quarantined_mise")" != "$original_mise_mode" ]]; then
    printf 'generated-workflows: failed Mise download did not preserve exactly one corrupt quarantined artifact and mode\n' >&2
    exit 1
  fi
  leftover_download=0
  for download in "$installation"/.mise-download.*; do
    if [[ -e "$download" || -L "$download" ]]; then
      leftover_download=1
    fi
  done
  if [[ "$e2e_focus" == installer-repair-mise-http-failure ]]; then
    expected_diagnostic='cannot download the pinned official mise release'
  else
    expected_diagnostic='official mise artifact checksum does not match the reviewed release identity'
  fi
  {
    printf 'focus=%s\n' "$e2e_focus"
    printf 'installer_status=%s\n' "$installer_status"
    printf 'network_fixture_pid=%s\n' "$network_fixture_child_pid"
    printf 'network_fixture_status=%s\n' "$server_status"
    printf 'network_fixture_lifetime_seconds=%s\n' "$network_fixture_elapsed_seconds"
    printf 'network_fixture_bind=127.0.0.1\n'
    printf 'network_fixture_port=%s\n' "$fixture_port"
    printf 'network_fixture_headers_logged=false\n'
    printf 'fixture_request=%s\n' "$request_record"
    printf 'pre_commit_before_sha256=%s\n' "$(sha256_file "$test_root/pre-commit.before")"
    printf 'pre_commit_before_mode=%s\n' "$saved_pre_commit_mode"
    printf 'pre_push_before_sha256=%s\n' "$(sha256_file "$test_root/pre-push.before")"
    printf 'pre_push_before_mode=%s\n' "$saved_pre_push_mode"
    printf 'pre_commit_after_sha256=%s\n' "$(sha256_file "$hooks_path/pre-commit")"
    printf 'pre_commit_after_mode=%s\n' "$(stat_mode "$hooks_path/pre-commit")"
    printf 'pre_push_after_sha256=%s\n' "$(sha256_file "$hooks_path/pre-push")"
    printf 'pre_push_after_mode=%s\n' "$(stat_mode "$hooks_path/pre-push")"
    printf 'sentinel_sha256=%s\n' "$original_sentinel_sha"
    printf 'sentinel_mode=%s\n' "$original_sentinel_mode"
    printf 'sentinel_after_sha256=%s\n' "$(sha256_file "$unrelated_sentinel")"
    printf 'sentinel_after_mode=%s\n' "$(stat_mode "$unrelated_sentinel")"
    printf 'pinned_mise_sha256=%s\n' "$mise_sha_value"
    printf 'corrupted_quarantine_sha256=%s\n' "$damaged_mise_sha"
    printf 'quarantined_mise_mode=%s\n' "$(stat_mode "$quarantined_mise")"
    printf 'quarantine_match_count=%s\n' "$quarantine_count"
    printf 'expected_diagnostic=%s\n' "$expected_diagnostic"
    printf 'replacement_target_present=%s\n' "$(if [[ -e "$mise_artifact" || -L "$mise_artifact" ]]; then printf true; else printf false; fi)"
    printf 'download_temporary_remains=%s\n' "$leftover_download"
  } > "$test_root/mise-network-repair-evidence.log"
  if [[ "$installer_mutant" == mise-wrong-hash-publish-stop ]]; then
    if [[ "$installer_status" -ne 99 ]] || ! /usr/bin/grep -F -q 'wrong-hash source fault published its payload before execution' "$test_root/mise-network-repair.log" || [[ ! -f "$mise_artifact" || -L "$mise_artifact" || "$(sha256_file "$mise_artifact")" != "$network_body_sha" || "$quarantine_count" -ne 1 || "$leftover_download" -ne 0 ]]; then
      cat "$test_root/mise-network-repair.log" >&2
      printf 'generated-workflows: wrong-hash causal fault precondition did not publish the staged mismatched response before any execution\n' >&2
      exit 1
    fi
  fi
  if [[ -e "$mise_artifact" || -L "$mise_artifact" ]]; then
    cat "$test_root/mise-network-repair.log" >&2
    printf 'generated-workflows: failed or wrong-hash Mise download created an unexpected replacement target\n' >&2
    exit 1
  fi
  if [[ "$leftover_download" -ne 0 ]]; then
    printf 'generated-workflows: failed or wrong-hash Mise download left a temporary payload\n' >&2
    exit 1
  fi
  if [[ "$installer_status" -ne 1 ]]; then
    cat "$test_root/mise-network-repair.log" >&2
    printf 'generated-workflows: failed or wrong-hash Mise download did not return the expected failure status\n' >&2
    exit 1
  fi
  if ! /usr/bin/grep -F -q "$expected_diagnostic" "$test_root/mise-network-repair.log"; then
    cat "$test_root/mise-network-repair.log" >&2
    printf 'generated-workflows: Mise download failure did not report the expected trust-boundary failure\n' >&2
    exit 1
  fi
  printf '%s refused the network response, preserved the corrupt quarantine and left hooks and sentinel unchanged: PASS\n' "$e2e_focus" >> "$test_root/case-results.log"
  printf '%s: PASS\n' "$e2e_focus"
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == installer-repair-mise ]]; then
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  save_hook_baseline
  mise_version_value=$(manifest_value mise_version)
  mise_sha_value=$(manifest_value mise_sha256)
  mise_artifact="$installation/artifacts/mise/$mise_version_value/$platform/mise"
  if [[ ! -f "$mise_artifact" || -L "$mise_artifact" || "$(sha256_file "$mise_artifact")" != "$mise_sha_value" ]]; then
    printf 'generated-workflows: qualified Mise artifact is unavailable before repair case\n' >&2
    exit 1
  fi
  chmod u+w "$mise_artifact"
  printf 'X' | /bin/dd of="$mise_artifact" bs=1 seek=0 conv=notrunc 2> "$test_root/corrupt-mise-dd.log"
  damaged_mise_sha=$(sha256_file "$mise_artifact")
  if [[ "$damaged_mise_sha" == "$mise_sha_value" ]]; then
    printf 'generated-workflows: Mise corruption fixture did not change the pinned artifact\n' >&2
    exit 1
  fi
  installer_status=0
  if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/repair-mise.log" 2>&1; then
    installer_status=0
  else
    installer_status=$?
  fi
  if [[ "$installer_mutant" == mise-validation-only ]] && ! /usr/bin/grep -F -q 'pinned mise artifact failed integrity validation' "$test_root/repair-mise.log"; then
    cat "$test_root/repair-mise.log" >&2
    printf 'generated-workflows: Mise validation-only fault did not reach the corrupt-owned-artifact branch\n' >&2
    exit 1
  fi
  quarantined_mise=0
  for quarantined_artifact in "$(dirname "$mise_artifact")"/.quarantine.*/artifact; do
    if [[ -f "$quarantined_artifact" && "$(sha256_file "$quarantined_artifact")" == "$damaged_mise_sha" ]]; then
      quarantined_mise=1
    fi
  done
  if [[ "$(sha256_file "$mise_artifact")" != "$mise_sha_value" || "$quarantined_mise" -ne 1 || "$(/bin/cat "$unrelated_sentinel")" != 'preserve this unrelated install payload' ]]; then
    printf 'generated-workflows: Mise repair did not restore the pinned artifact and preserve quarantine state\n' >&2
    exit 1
  fi
  if [[ "$installer_status" -ne 0 ]]; then
    cat "$test_root/repair-mise.log" >&2
    printf 'generated-workflows: installer did not complete after restoring the corrupt Mise artifact\n' >&2
    exit 1
  fi
  assert_hooks_match_baseline
  run_installed_hooks
  printf 'corrupt pinned Mise artifact repaired and quarantined; both canonical hooks pass: PASS\n' >> "$test_root/case-results.log"
  printf 'corrupt pinned Mise artifact repair: PASS\n'
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$e2e_focus" == installer-repair-python-executable || "$e2e_focus" == installer-repair-python-inventory || "$e2e_focus" == installer-repair-python-missing-inventory ]]; then
  python_inventory_evidence=
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  chmod 600 "$unrelated_sentinel"
  save_hook_baseline
  original_python_sha=$(sha256_file "$python_runtime/bin/python3.15")
  original_inventory_sha=$(sha256_file "$python_inventory")
  cp -p "$python_inventory" "$test_root/python-inventory.original"
  original_python_manifest_sha=$(sha256_file "$python_artifact_manifest")
  original_python_config_sha=$(sha256_file "$python_mise_config")
  original_python_lock_sha=$(sha256_file "$python_mise_lock")
  original_python_generation_mode=$(stat_mode "$python_artifact_dir")
  original_python_bin_mode=$(stat_mode "$python_runtime/bin")
  original_python_executable_mode=$(stat_mode "$python_runtime/bin/python3.15")
  original_python_inventory_mode=$(stat_mode "$python_inventory")
  original_python_manifest_mode=$(stat_mode "$python_artifact_manifest")
  original_sentinel_sha=$(sha256_file "$unrelated_sentinel")
  original_sentinel_mode=$(stat_mode "$unrelated_sentinel")
  if [[ "$original_inventory_sha" != "$(manifest_value inventory_sha256)" ]] || ! "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
    printf 'generated-workflows: qualified Python generation did not validate before repair intervention\n' >&2
    exit 1
  fi
  run_installed_hooks
  case "$e2e_focus" in
    installer-repair-python-executable)
      printf -v marker_path '%q' "$test_root/invalid-python-ran"
      chmod u+w "$python_runtime/bin" "$python_runtime/bin/python3.15"
      printf '#!/bin/sh\n/bin/touch %s\nexit 0\n' "$marker_path" > "$python_runtime/bin/python3.15"
      chmod "$original_python_executable_mode" "$python_runtime/bin/python3.15"
      chmod "$original_python_bin_mode" "$python_runtime/bin"
      damaged_python_sha=$(sha256_file "$python_runtime/bin/python3.15")
      if [[ "$damaged_python_sha" == "$original_python_sha" || "$(sha256_file "$python_inventory")" != "$original_inventory_sha" || "$(sha256_file "$python_artifact_manifest")" != "$original_python_manifest_sha" || "$(stat_mode "$python_artifact_dir")" != "$original_python_generation_mode" || "$(stat_mode "$python_runtime/bin")" != "$original_python_bin_mode" || "$(stat_mode "$python_runtime/bin/python3.15")" != "$original_python_executable_mode" ]]; then
        printf 'generated-workflows: Python executable-only fixture changed inventory, manifests, or original modes\n' >&2
        exit 1
      fi
      ;;
    installer-repair-python-inventory)
      chmod u+w "$python_inventory"
      printf '%s\n' 'damaged inventory only fixture' >> "$python_inventory"
      chmod "$original_python_inventory_mode" "$python_inventory"
      damaged_inventory_sha=$(sha256_file "$python_inventory")
      if [[ "$damaged_inventory_sha" == "$original_inventory_sha" || "$(sha256_file "$python_runtime/bin/python3.15")" != "$original_python_sha" || "$(sha256_file "$python_artifact_manifest")" != "$original_python_manifest_sha" || "$(stat_mode "$python_inventory")" != "$original_python_inventory_mode" ]]; then
        printf 'generated-workflows: Python inventory-only fixture changed unrelated generation state\n' >&2
        exit 1
      fi
      ;;
    installer-repair-python-missing-inventory)
      python_inventory_evidence="$test_root/missing-python-inventory.evidence"
      chmod u+w "$python_artifact_dir"
      mv "$python_inventory" "$python_inventory_evidence"
      chmod "$original_python_generation_mode" "$python_artifact_dir"
      if [[ -e "$python_inventory" || -L "$python_inventory" || "$(sha256_file "$python_inventory_evidence")" != "$original_inventory_sha" || "$(sha256_file "$python_runtime/bin/python3.15")" != "$original_python_sha" || "$(sha256_file "$python_artifact_manifest")" != "$original_python_manifest_sha" || "$(stat_mode "$python_artifact_dir")" != "$original_python_generation_mode" ]]; then
        printf 'generated-workflows: incomplete Python fixture changed unrelated generation state or retained inventory\n' >&2
        exit 1
      fi
      ;;
  esac
  python_generation_parent=$(dirname "$python_artifact_dir")
  if ! write_sorted_find_results "$python_generation_parent" "$test_root/python-quarantine-before.raw" "$test_root/python-quarantine-before.list" -mindepth 1 -maxdepth 1 -type d -name '.quarantine.*'; then
    printf 'generated-workflows: cannot enumerate Python quarantines before repair\n' >&2
    exit 1
  fi
  case "$e2e_focus" in
    installer-repair-python-executable)
      prerepair_diagnostic='generated-workflows: pinned Python artifact inventory does not match its qualified archive'
      prerepair_inventory_state=$original_inventory_sha
      prerepair_runtime_state=$damaged_python_sha
      prerepair_missing_inventory_evidence_state=not-applicable
      prerepair_missing_inventory_evidence_mode=not-applicable
      ;;
    installer-repair-python-inventory)
      prerepair_diagnostic='generated-workflows: pinned Python 3.15.2 or its inventory is missing or changed; run bash scripts/install-generated-workflow-guard.sh from reviewed code'
      prerepair_inventory_state=$damaged_inventory_sha
      prerepair_runtime_state=$original_python_sha
      prerepair_missing_inventory_evidence_state=not-applicable
      prerepair_missing_inventory_evidence_mode=not-applicable
      ;;
    installer-repair-python-missing-inventory)
      prerepair_diagnostic='generated-workflows: pinned Python 3.15.2 or its inventory is missing or changed; run bash scripts/install-generated-workflow-guard.sh from reviewed code'
      prerepair_inventory_state=missing
      prerepair_runtime_state=$original_python_sha
      prerepair_missing_inventory_evidence_state=$(sha256_file "$python_inventory_evidence")
      prerepair_missing_inventory_evidence_mode=$(stat_mode "$python_inventory_evidence")
      ;;
  esac
  prerepair_manifest_sha=$(sha256_file "$python_artifact_manifest")
  prerepair_generation_mode=$(stat_mode "$python_artifact_dir")
  prerepair_runtime_mode=$(stat_mode "$python_runtime/bin/python3.15")
  prerepair_inventory_mode=$(if [[ -f "$python_inventory" && ! -L "$python_inventory" ]]; then stat_mode "$python_inventory"; else printf missing; fi)
  prerepair_manifest_mode=$(stat_mode "$python_artifact_manifest")
  prerepair_installed_hook_status=0
  if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/python-pre-repair-hook.log" 2>&1; then
    prerepair_installed_hook_status=0
  else
    prerepair_installed_hook_status=$?
  fi
  printf 'pre_repair_hook_status=%s\n' "$prerepair_installed_hook_status" >> "$test_root/python-repair-case-evidence.log"
  if [[ "$prerepair_installed_hook_status" -eq 0 ]] || ! /usr/bin/grep -F -x -q "$prerepair_diagnostic" "$test_root/python-pre-repair-hook.log"; then
    cat "$test_root/python-pre-repair-hook.log" >&2
    printf 'generated-workflows: damaged Python hook did not refuse with the exact integrity diagnostic before repair\n' >&2
    exit 1
  fi
  if ! write_sorted_find_results "$python_generation_parent" "$test_root/python-quarantine-after-prerepair.raw" "$test_root/python-quarantine-after-prerepair.list" -mindepth 1 -maxdepth 1 -type d -name '.quarantine.*'; then
    printf 'generated-workflows: cannot enumerate Python quarantines after pre-repair hook\n' >&2
    exit 1
  fi
  prerepair_current_inventory=$(if [[ -f "$python_inventory" && ! -L "$python_inventory" ]]; then sha256_file "$python_inventory"; else printf missing; fi)
  prerepair_inventory_mode_after=$(if [[ -f "$python_inventory" && ! -L "$python_inventory" ]]; then stat_mode "$python_inventory"; else printf missing; fi)
  prerepair_quarantine_unchanged=0
  if cmp -s "$test_root/python-quarantine-before.list" "$test_root/python-quarantine-after-prerepair.list"; then prerepair_quarantine_unchanged=1; fi
  prerepair_hooks_unchanged=0
  if cmp -s "$test_root/pre-commit.before" "$hooks_path/pre-commit" && cmp -s "$test_root/pre-push.before" "$hooks_path/pre-push"; then prerepair_hooks_unchanged=1; fi
  prerepair_missing_inventory_evidence_current=$(if [[ -z "$python_inventory_evidence" ]]; then printf not-applicable; elif [[ -f "$python_inventory_evidence" && ! -L "$python_inventory_evidence" ]]; then sha256_file "$python_inventory_evidence"; else printf missing; fi)
  prerepair_missing_inventory_evidence_mode_current=$(if [[ -z "$python_inventory_evidence" ]]; then printf not-applicable; elif [[ -f "$python_inventory_evidence" && ! -L "$python_inventory_evidence" ]]; then stat_mode "$python_inventory_evidence"; else printf missing; fi)
  if [[ -e "$test_root/invalid-python-ran" || "$(sha256_file "$python_runtime/bin/python3.15")" != "$prerepair_runtime_state" || "$prerepair_current_inventory" != "$prerepair_inventory_state" || "$prerepair_missing_inventory_evidence_current" != "$prerepair_missing_inventory_evidence_state" || "$prerepair_missing_inventory_evidence_mode_current" != "$prerepair_missing_inventory_evidence_mode" || "$(sha256_file "$python_artifact_manifest")" != "$prerepair_manifest_sha" || "$(stat_mode "$python_artifact_dir")" != "$prerepair_generation_mode" || "$(stat_mode "$python_runtime/bin/python3.15")" != "$prerepair_runtime_mode" || "$prerepair_inventory_mode" != "$prerepair_inventory_mode_after" || "$(stat_mode "$python_artifact_manifest")" != "$prerepair_manifest_mode" || "$prerepair_quarantine_unchanged" -ne 1 || "$prerepair_hooks_unchanged" -ne 1 || "$(stat_mode "$hooks_path/pre-commit")" != "$saved_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$saved_pre_push_mode" || "$(sha256_file "$unrelated_sentinel")" != "$original_sentinel_sha" || "$(stat_mode "$unrelated_sentinel")" != "$original_sentinel_mode" ]]; then
    printf 'generated-workflows: pre-repair hook changed damaged Python, manifest, modes, quarantine state, hooks, or sentinel\n' >&2
    exit 1
  fi
  {
    printf 'pre_repair_hook_diagnostic=%s\n' "$prerepair_diagnostic"
    printf 'pre_repair_runtime_sha256=%s\n' "$(sha256_file "$python_runtime/bin/python3.15")"
    printf 'pre_repair_inventory_sha256=%s\n' "$prerepair_current_inventory"
    printf 'pre_repair_missing_inventory_evidence_sha256=%s\n' "$prerepair_missing_inventory_evidence_current"
    printf 'pre_repair_manifest_sha256=%s\n' "$(sha256_file "$python_artifact_manifest")"
    printf 'pre_repair_generation_mode=%s\n' "$(stat_mode "$python_artifact_dir")"
    printf 'pre_repair_runtime_mode=%s\n' "$(stat_mode "$python_runtime/bin/python3.15")"
    printf 'pre_repair_inventory_mode=%s\n' "$prerepair_inventory_mode"
    printf 'pre_repair_missing_inventory_evidence_mode=%s\n' "$prerepair_missing_inventory_evidence_mode_current"
    printf 'pre_repair_manifest_mode=%s\n' "$(stat_mode "$python_artifact_manifest")"
    printf 'pre_repair_quarantine_set_unchanged=true\n'
    printf 'pre_repair_installed_hook_preservation=true\n'
    printf 'pre_repair_sentinel_preservation=true\n'
  } >> "$test_root/python-repair-case-evidence.log"
  {
    printf 'focus=%s\n' "$e2e_focus"
    printf 'original_python_sha256=%s\n' "$original_python_sha"
    printf 'original_inventory_sha256=%s\n' "$original_inventory_sha"
    printf 'original_python_manifest_sha256=%s\n' "$original_python_manifest_sha"
    printf 'original_python_config_sha256=%s\n' "$original_python_config_sha"
    printf 'original_python_lock_sha256=%s\n' "$original_python_lock_sha"
    printf 'original_python_generation_mode=%s\n' "$original_python_generation_mode"
    printf 'original_python_bin_mode=%s\n' "$original_python_bin_mode"
    printf 'original_python_executable_mode=%s\n' "$original_python_executable_mode"
    printf 'original_python_inventory_mode=%s\n' "$original_python_inventory_mode"
    printf 'original_python_manifest_mode=%s\n' "$original_python_manifest_mode"
    printf 'checker_binding_inventory_sha256=%s\n' "$(manifest_value inventory_sha256)"
    printf 'damaged_python_sha256=%s\n' "${damaged_python_sha:-unchanged}"
    printf 'damaged_inventory_sha256=%s\n' "${damaged_inventory_sha:-unchanged}"
  } >> "$test_root/python-repair-case-evidence.log"
  installer_status=0
  if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/repair-python.log" 2>&1; then
    installer_status=0
  else
    installer_status=$?
  fi
  if [[ "$installer_mutant" == python-validation-only ]] && ! /usr/bin/grep -F -q 'pinned Python generation failed integrity validation' "$test_root/repair-python.log"; then
    cat "$test_root/repair-python.log" >&2
    printf 'generated-workflows: Python validation-only fault did not reach the corrupt-owned-generation branch\n' >&2
    exit 1
  fi
  new_quarantine_count=0
  new_quarantine_path=
  if ! write_sorted_find_results "$python_generation_parent" "$test_root/python-generations-after.raw" "$test_root/python-generations-after.list" -mindepth 2 -maxdepth 2 -type d -name generation; then
    printf 'generated-workflows: cannot enumerate Python generations after repair\n' >&2
    exit 1
  fi
  while IFS= read -r quarantine_generation; do
    quarantine_parent=$(dirname "$quarantine_generation")
    if list_contains_exact_line "$quarantine_parent" "$test_root/python-quarantine-before.list"; then
      continue
    else
      quarantine_list_status=$?
      if [[ "$quarantine_list_status" -ne 1 ]]; then
        printf 'generated-workflows: cannot compare Python quarantine listings\n' >&2
        exit 1
      fi
    fi
    new_quarantine_count=$((new_quarantine_count + 1))
    new_quarantine_path=$quarantine_generation
  done < "$test_root/python-generations-after.list"
  {
    printf 'installer_status=%s\n' "$installer_status"
    printf 'new_quarantine_count=%s\n' "$new_quarantine_count"
    printf 'invalid_runtime_marker_present=%s\n' "$(if [[ -e "$test_root/invalid-python-ran" ]]; then printf true; else printf false; fi)"
    printf 'pre_commit_after_sha256=%s\n' "$(sha256_file "$hooks_path/pre-commit")"
    printf 'pre_commit_after_mode=%s\n' "$(stat_mode "$hooks_path/pre-commit")"
    printf 'pre_push_after_sha256=%s\n' "$(sha256_file "$hooks_path/pre-push")"
    printf 'pre_push_after_mode=%s\n' "$(stat_mode "$hooks_path/pre-push")"
    printf 'sentinel_sha256=%s\n' "$(sha256_file "$unrelated_sentinel")"
    printf 'sentinel_mode=%s\n' "$(stat_mode "$unrelated_sentinel")"
  } >> "$test_root/python-repair-case-evidence.log"
  if [[ -e "$test_root/invalid-python-ran" ]]; then
    printf 'generated-workflows: corrupted Python runtime marker was executed\n' >&2
    exit 1
  fi
  if ! cmp -s "$test_root/pre-commit.before" "$hooks_path/pre-commit" || ! cmp -s "$test_root/pre-push.before" "$hooks_path/pre-push" || [[ "$(stat_mode "$hooks_path/pre-commit")" != "$saved_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$saved_pre_push_mode" || "$(sha256_file "$unrelated_sentinel")" != "$original_sentinel_sha" || "$(stat_mode "$unrelated_sentinel")" != "$original_sentinel_mode" ]]; then
    printf 'generated-workflows: Python repair intervention changed canonical hooks or unrelated sentinel state\n' >&2
    exit 1
  fi
  if [[ "$new_quarantine_count" -ne 1 ]]; then
    printf 'generated-workflows: Python repair did not create exactly one whole-generation quarantine\n' >&2
    exit 1
  fi
  if [[ "$installer_status" -ne 0 ]] && ! /usr/bin/grep -F -q 'installed checker manifest does not bind the qualified runtime artifacts' "$test_root/repair-python.log"; then
    cat "$test_root/repair-python.log" >&2
    printf 'generated-workflows: installer failed outside the approved incompatible-inventory refusal\n' >&2
    exit 1
  fi
  quarantined_generation=$new_quarantine_path
  {
    printf 'quarantined_generation=%s\n' "$quarantined_generation"
    printf 'quarantined_generation_mode=%s\n' "$(stat_mode "$quarantined_generation")"
    printf 'quarantined_manifest_sha256=%s\n' "$(sha256_file "$quarantined_generation/manifest")"
    printf 'quarantined_python_sha256=%s\n' "$(sha256_file "$quarantined_generation/runtime/bin/python3.15")"
    printf 'quarantined_inventory_present=%s\n' "$(if [[ -e "$quarantined_generation/inventory" || -L "$quarantined_generation/inventory" ]]; then printf true; else printf false; fi)"
    printf 'quarantined_inventory_sha256=%s\n' "$(if [[ -f "$quarantined_generation/inventory" && ! -L "$quarantined_generation/inventory" ]]; then sha256_file "$quarantined_generation/inventory"; else printf missing; fi)"
  } >> "$test_root/python-repair-case-evidence.log"
  if [[ "$(stat_mode "$quarantined_generation")" != "$original_python_generation_mode" || "$(sha256_file "$quarantined_generation/manifest")" != "$original_python_manifest_sha" || "$(sha256_file "$quarantined_generation/mise.toml")" != "$original_python_config_sha" || "$(sha256_file "$quarantined_generation/mise.lock")" != "$original_python_lock_sha" ]]; then
    printf 'generated-workflows: quarantined Python generation lost its original mode or immutable metadata\n' >&2
    exit 1
  fi
  case "$e2e_focus" in
    installer-repair-python-executable)
      if [[ "$(sha256_file "$quarantined_generation/runtime/bin/python3.15")" != "$damaged_python_sha" || "$(sha256_file "$quarantined_generation/inventory")" != "$original_inventory_sha" || "$(stat_mode "$quarantined_generation/runtime/bin/python3.15")" != "$original_python_executable_mode" ]]; then
        printf 'generated-workflows: executable-only quarantine did not preserve the altered runtime and original inventory exactly\n' >&2
        exit 1
      fi
      python3 -I -S - "$test_root/python-inventory.original" "$test_root/python-inventory-quarantined.expected" "$damaged_python_sha" "$original_python_executable_mode" <<'PY'
import sys
from pathlib import Path

source, destination, damaged_sha, executable_mode = sys.argv[1:]
lines = Path(source).read_text().splitlines(keepends=True)
matches = 0
for index, line in enumerate(lines):
    fields = line.rstrip("\n").split("\t")
    if len(fields) == 4 and fields[0] == "file" and fields[3] == "bin/python3.15":
        if fields[1] != executable_mode:
            raise SystemExit("generated-workflows: original inventory executable mode did not match the fixture")
        fields[2] = damaged_sha
        lines[index] = "\t".join(fields) + "\n"
        matches += 1
if matches != 1:
    raise SystemExit("generated-workflows: original inventory did not identify exactly one Python executable")
Path(destination).write_text("".join(lines))
PY
      if ! "$repo_root/scripts/python-inventory.sh" check "$quarantined_generation/runtime" "$test_root/python-inventory-quarantined.expected"; then
        printf 'generated-workflows: executable-only quarantine changed another inventoried runtime entry\n' >&2
        exit 1
      fi
      ;;
    installer-repair-python-inventory)
      if [[ "$(sha256_file "$quarantined_generation/runtime/bin/python3.15")" != "$original_python_sha" || "$(sha256_file "$quarantined_generation/inventory")" != "$damaged_inventory_sha" || "$(stat_mode "$quarantined_generation/inventory")" != "$original_python_inventory_mode" ]]; then
        printf 'generated-workflows: inventory-only quarantine did not preserve the damaged inventory and original runtime exactly\n' >&2
        exit 1
      fi
      if ! "$repo_root/scripts/python-inventory.sh" check "$quarantined_generation/runtime" "$test_root/python-inventory.original"; then
        printf 'generated-workflows: inventory-only quarantine changed an inventoried runtime entry\n' >&2
        exit 1
      fi
      ;;
    installer-repair-python-missing-inventory)
      if [[ -e "$quarantined_generation/inventory" || -L "$quarantined_generation/inventory" || "$(sha256_file "$python_inventory_evidence")" != "$original_inventory_sha" || "$(sha256_file "$quarantined_generation/runtime/bin/python3.15")" != "$original_python_sha" ]]; then
        printf 'generated-workflows: incomplete-generation quarantine changed the missing inventory or remaining runtime\n' >&2
        exit 1
      fi
      if ! "$repo_root/scripts/python-inventory.sh" check "$quarantined_generation/runtime" "$test_root/python-inventory.original"; then
        printf 'generated-workflows: incomplete-generation quarantine changed an inventoried runtime entry\n' >&2
        exit 1
      fi
      ;;
  esac
  repaired_inventory_sha=$(sha256_file "$python_inventory")
  printf 'rebuilt_inventory_sha256=%s\n' "$repaired_inventory_sha" >> "$test_root/python-repair-case-evidence.log"
  if [[ "$repaired_inventory_sha" == "$(manifest_value inventory_sha256)" ]] && "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
    if [[ "$installer_status" -ne 0 ]]; then
      cat "$test_root/repair-python.log" >&2
      printf 'generated-workflows: installer failed despite rebuilding a checker-bound Python inventory\n' >&2
      exit 1
    fi
    assert_hooks_match_baseline
    run_installed_hooks
    printf 'recovery_outcome=checker-bound repair and hooks pass\n' >> "$test_root/python-repair-case-evidence.log"
    printf '%s repaired through whole-generation quarantine without executing invalid Python bytes; checker binding and both hooks pass: PASS\n' "$e2e_focus" >> "$test_root/case-results.log"
    printf '%s: PASS\n' "$e2e_focus"
    record_completion_witness "focus:$e2e_focus"
    mark_completion_complete
    exit 0
  fi
  if [[ "$installer_status" -ne 0 && "$repaired_inventory_sha" != "$(manifest_value inventory_sha256)" ]] && /usr/bin/grep -F -q 'installed checker manifest does not bind the qualified runtime artifacts' "$test_root/repair-python.log"; then
    if ! "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
      printf 'generated-workflows: incompatible Python rebuild did not validate against its own inventory\n' >&2
      exit 1
    fi
    assert_hooks_match_baseline
    printf 'recovery_outcome=safe refusal of incompatible rebuilt inventory\n' >> "$test_root/python-repair-case-evidence.log"
    printf '%s safely refused an inventory incompatible with the immutable checker binding; quarantined invalid generation and hooks/sentinel are preserved: PASS\n' "$e2e_focus" >> "$test_root/case-results.log"
    printf '%s: safe refusal for incompatible rebuilt inventory: PASS\n' "$e2e_focus"
    record_completion_witness "focus:$e2e_focus"
    mark_completion_complete
    exit 0
  fi
  cat "$test_root/repair-python.log" >&2
  printf 'generated-workflows: Python repair neither restored the checker-bound inventory nor safely refused incompatibility\n' >&2
  exit 1
fi
if [[ "$e2e_focus" == installer-unsafe-python ]]; then
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  chmod 600 "$unrelated_sentinel"
  save_hook_baseline
  original_python_sha=$(sha256_file "$python_runtime/bin/python3.15")
  original_inventory_sha=$(sha256_file "$python_inventory")
  original_python_manifest_sha=$(sha256_file "$python_artifact_manifest")
  original_python_generation_mode=$(stat_mode "$python_artifact_dir")
  original_python_inventory_mode=$(stat_mode "$python_inventory")
  original_sentinel_sha=$(sha256_file "$unrelated_sentinel")
  original_sentinel_mode=$(stat_mode "$unrelated_sentinel")
  if [[ "$original_inventory_sha" != "$(manifest_value inventory_sha256)" ]] || ! "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
    printf 'generated-workflows: qualified Python generation did not validate before unsafe-root cases\n' >&2
    exit 1
  fi
  run_installed_hooks
  assert_unchanged_state() {
    if ! cmp -s "$test_root/pre-commit.before" "$hooks_path/pre-commit" || ! cmp -s "$test_root/pre-push.before" "$hooks_path/pre-push" || [[ "$(stat_mode "$hooks_path/pre-commit")" != "$saved_pre_commit_mode" || "$(stat_mode "$hooks_path/pre-push")" != "$saved_pre_push_mode" || "$(sha256_file "$unrelated_sentinel")" != "$original_sentinel_sha" || "$(stat_mode "$unrelated_sentinel")" != "$original_sentinel_mode" ]]; then
      printf 'generated-workflows: unsafe Python root intervention changed canonical hooks or unrelated sentinel state\n' >&2
      return 1
    fi
  }
  chmod 750 "$python_artifact_dir"
  installer_status=0
  if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/unsafe-python-mode.log" 2>&1; then
    installer_status=0
  else
    installer_status=$?
  fi
  if [[ "$installer_status" -eq 0 ]] || ! /usr/bin/grep -F -q 'pinned Python artifact generation root is foreign or unsafe' "$test_root/unsafe-python-mode.log" || [[ "$(stat_mode "$python_artifact_dir")" != 750 || "$(sha256_file "$python_runtime/bin/python3.15")" != "$original_python_sha" || "$(sha256_file "$python_inventory")" != "$original_inventory_sha" || "$(sha256_file "$python_artifact_manifest")" != "$original_python_manifest_sha" ]]; then
    cat "$test_root/unsafe-python-mode.log" >&2
    chmod "$original_python_generation_mode" "$python_artifact_dir"
    printf 'generated-workflows: unsafe Python root mode was not refused and preserved\n' >&2
    exit 1
  fi
  chmod "$original_python_generation_mode" "$python_artifact_dir"
  assert_unchanged_state
  python_root_evidence="$test_root/python-root-preserved"
  if [[ -e "$python_root_evidence" || -L "$python_root_evidence" ]]; then
    printf 'generated-workflows: unsafe Python symlink evidence path unexpectedly exists\n' >&2
    exit 1
  fi
  mv "$python_artifact_dir" "$python_root_evidence"
  ln -s "$python_root_evidence" "$python_artifact_dir"
  installer_status=0
  if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/unsafe-python-symlink.log" 2>&1; then
    installer_status=0
  else
    installer_status=$?
  fi
  if [[ "$installer_status" -eq 0 || ! -L "$python_artifact_dir" || "$(readlink "$python_artifact_dir")" != "$python_root_evidence" ]] || ! /usr/bin/grep -F -q 'pinned Python artifact generation root is foreign or unsafe' "$test_root/unsafe-python-symlink.log" || [[ "$(sha256_file "$python_root_evidence/runtime/bin/python3.15")" != "$original_python_sha" || "$(sha256_file "$python_root_evidence/inventory")" != "$original_inventory_sha" || "$(sha256_file "$python_root_evidence/manifest")" != "$original_python_manifest_sha" ]]; then
    cat "$test_root/unsafe-python-symlink.log" >&2
    if [[ -L "$python_artifact_dir" && "$(readlink "$python_artifact_dir")" == "$python_root_evidence" ]]; then
      rm "$python_artifact_dir"
      mv "$python_root_evidence" "$python_artifact_dir"
    fi
    printf 'generated-workflows: symlinked Python root was not refused without following or changing its target\n' >&2
    exit 1
  fi
  assert_unchanged_state
  rm "$python_artifact_dir"
  mv "$python_root_evidence" "$python_artifact_dir"
  if [[ "$(stat_mode "$python_artifact_dir")" != "$original_python_generation_mode" ]]; then
    printf 'generated-workflows: Python root mode changed while restoring the symlink fixture\n' >&2
    exit 1
  fi
  python_generation_parent=$(dirname "$python_artifact_dir")
  python_hardlink_evidence="$python_generation_parent/.inventory-hardlink-evidence"
  if [[ -e "$python_hardlink_evidence" || -L "$python_hardlink_evidence" ]]; then
    printf 'generated-workflows: Python hard-link evidence path unexpectedly exists\n' >&2
    exit 1
  fi
  ln "$python_inventory" "$python_hardlink_evidence"
  if [[ "$(stat_links "$python_inventory")" != 2 ]]; then
    printf 'generated-workflows: Python hard-link fixture did not create a two-link inventory\n' >&2
    exit 1
  fi
  if ! write_sorted_find_results "$python_generation_parent" "$test_root/python-hardlink-quarantine-before.raw" "$test_root/python-hardlink-quarantine-before.list" -mindepth 1 -maxdepth 1 -type d -name '.quarantine.*'; then
    printf 'generated-workflows: cannot enumerate Python quarantines before hard-link repair\n' >&2
    exit 1
  fi
  installer_status=0
  if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/unsafe-python-hardlink.log" 2>&1; then
    installer_status=0
  else
    installer_status=$?
  fi
  if [[ "$installer_status" -ne 0 ]]; then
    cat "$test_root/unsafe-python-hardlink.log" >&2
    printf 'generated-workflows: installer did not recover from the multiply-linked Python inventory\n' >&2
    exit 1
  fi
  hardlink_quarantine_count=0
  hardlink_quarantine_path=
  if ! write_sorted_find_results "$python_generation_parent" "$test_root/python-hardlink-generations-after.raw" "$test_root/python-hardlink-generations-after.list" -mindepth 2 -maxdepth 2 -type d -name generation; then
    printf 'generated-workflows: cannot enumerate Python generations after hard-link repair\n' >&2
    exit 1
  fi
  while IFS= read -r quarantine_generation; do
    quarantine_parent=$(dirname "$quarantine_generation")
    if list_contains_exact_line "$quarantine_parent" "$test_root/python-hardlink-quarantine-before.list"; then
      continue
    else
      quarantine_list_status=$?
      if [[ "$quarantine_list_status" -ne 1 ]]; then
        printf 'generated-workflows: cannot compare hard-link quarantine listings\n' >&2
        exit 1
      fi
    fi
    hardlink_quarantine_count=$((hardlink_quarantine_count + 1))
    hardlink_quarantine_path=$quarantine_generation
  done < "$test_root/python-hardlink-generations-after.list"
  if [[ "$hardlink_quarantine_count" -ne 1 ]]; then
    printf 'generated-workflows: multiply-linked Python inventory was not quarantined as one complete generation\n' >&2
    exit 1
  fi
  quarantined_generation=$hardlink_quarantine_path
  if [[ "$(stat_mode "$quarantined_generation")" != "$original_python_generation_mode" || "$(sha256_file "$quarantined_generation/inventory")" != "$original_inventory_sha" || "$(sha256_file "$python_hardlink_evidence")" != "$original_inventory_sha" || "$(stat_links "$python_hardlink_evidence")" != 2 ]]; then
    printf 'generated-workflows: hard-link quarantine did not preserve the original inventory and external link\n' >&2
    exit 1
  fi
  repaired_inventory_sha=$(sha256_file "$python_inventory")
  if [[ "$repaired_inventory_sha" == "$(manifest_value inventory_sha256)" ]] && "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
    if [[ "$installer_status" -ne 0 ]]; then
      cat "$test_root/unsafe-python-hardlink.log" >&2
      printf 'generated-workflows: hard-link recovery failed despite rebuilding a checker-bound Python inventory\n' >&2
      exit 1
    fi
    assert_unchanged_state
    assert_hooks_match_baseline
    run_installed_hooks
    rm "$python_hardlink_evidence"
    printf 'unsafe Python root mode and symlink refused without modification; hard-linked inventory quarantined whole and external link preserved with checker-bound repair: PASS\n' >> "$test_root/case-results.log"
    printf 'unsafe Python root and hard-link cases: PASS\n'
    record_completion_witness "focus:$e2e_focus"
    mark_completion_complete
    exit 0
  fi
  if [[ "$installer_status" -ne 0 && "$repaired_inventory_sha" != "$(manifest_value inventory_sha256)" ]] && /usr/bin/grep -F -q 'installed checker manifest does not bind the qualified runtime artifacts' "$test_root/unsafe-python-hardlink.log" && "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"; then
    assert_unchanged_state
    assert_hooks_match_baseline
    rm "$python_hardlink_evidence"
    printf 'unsafe Python root mode and symlink refused without modification; hard-linked inventory quarantined whole and incompatible repair was safely refused: PASS\n' >> "$test_root/case-results.log"
    printf 'unsafe Python root and hard-link cases: safe refusal for incompatible rebuilt inventory\n'
    record_completion_witness "focus:$e2e_focus"
    mark_completion_complete
    exit 0
  fi
  cat "$test_root/unsafe-python-hardlink.log" >&2
  printf 'generated-workflows: hard-link recovery neither restored the checker binding nor safely refused incompatibility\n' >&2
  exit 1
fi
if [[ "$e2e_focus" == installer-lock-lifecycle ]]; then
  unrelated_sentinel="$installation/unrelated-preserved-sentinel"
  printf 'preserve this unrelated install payload\n' > "$unrelated_sentinel"
  save_hook_baseline
  mise_version_value=$(manifest_value mise_version)
  mise_sha_value=$(manifest_value mise_sha256)
  mise_artifact="$installation/artifacts/mise/$mise_version_value/$platform/mise"
  if [[ ! -f "$mise_artifact" || -L "$mise_artifact" || "$(sha256_file "$mise_artifact")" != "$mise_sha_value" ]]; then
    printf 'generated-workflows: qualified Mise artifact is unavailable before lock lifecycle case\n' >&2
    exit 1
  fi
  chmod u+w "$python_inventory"
  printf '%s\n' 'corrupt inventory fixture for real Mise installation' >> "$python_inventory"
  python3 -I -S - "$repository" "$installation" "$mise_artifact" "$hooks_path" "$home_dir" "$test_root" "$installer_mutant" "$python_inventory" <<'PY'
import hashlib
import os
import select
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path

repository, installation, mise_artifact, hooks_path, home_dir, test_root, mutant, python_inventory = sys.argv[1:]
repository = Path(repository)
installation = Path(installation)
mise_artifact = str(mise_artifact).encode()
hooks_path = Path(hooks_path)
test_root = Path(test_root)
python_inventory = Path(python_inventory)
lock_path = installation / "install.advisory.lock"
sentinel = installation / "unrelated-preserved-sentinel"
pre_commit = hooks_path / "pre-commit"
pre_push = hooks_path / "pre-push"
lock_log = test_root / "lock-lifecycle-evidence.log"
lock_log.write_text("")
installer_log = test_root / "lock-lifecycle-installer.log"
publication_log = test_root / "lock-lifecycle-publication.log"
recovery_log = test_root / "lock-lifecycle-recovery.log"
push_recovery_log = test_root / "lock-lifecycle-pre-push-recovery.log"
contender_live_log = test_root / "lock-lifecycle-contender-live.log"
contender_dead_log = test_root / "lock-lifecycle-contender-dead.log"
CREDENTIAL_ENVIRONMENT_NAMES = ("GITHUB_COM_TOKEN", "GITHUB_TOKEN", "GH_TOKEN")
setup_credentials = []
for credential_name in CREDENTIAL_ENVIRONMENT_NAMES:
    credential_value = os.environ.pop(credential_name, "")
    if credential_value:
        setup_credentials.append(credential_value.encode())

def record(key, value):
    with lock_log.open("a") as output:
        output.write(f"{key}={value}\n")

def fail(message):
    raise RuntimeError(message)

def snapshot():
    result = subprocess.run(
        ["/bin/ps", "-ww", "-axo", "pid=,ppid=,pgid=,stat=,command="],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    processes = {}
    for line in result.stdout.splitlines():
        fields = line.strip().split(None, 4)
        if len(fields) != 5:
            continue
        pid, parent, group, state, command = fields
        processes[int(pid)] = {
            "parent": int(parent),
            "group": int(group),
            "state": state,
            "command": command,
        }
    return processes

def descendants(root, processes):
    depths = {}
    frontier = [(root, 0)]
    while frontier:
        parent, depth = frontier.pop(0)
        for pid, info in processes.items():
            if info["parent"] == parent and pid not in depths:
                depths[pid] = depth + 1
                frontier.append((pid, depth + 1))
    return sorted(depths, key=lambda pid: depths[pid], reverse=True)

def start_identity(pid):
    if sys.platform == "darwin":
        try:
            result = subprocess.run(
                ["/bin/ps", "-o", "lstart=", "-p", str(pid)],
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
            )
        except OSError:
            return None
        return result.stdout.strip() if result.returncode == 0 else None
    try:
        value = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    fields = value.rsplit(") ", 1)[-1].split()
    return fields[19] if len(fields) > 19 else None

def require_identity(pid, expected_identity=None):
    identity = start_identity(pid)
    if identity is None:
        fail("process start identity could not be observed")
    if expected_identity is not None and identity != expected_identity:
        fail("process identity changed during qualification")
    return identity

def command_bytes(pid, processes=None, expected_identity=None):
    identity = require_identity(pid, expected_identity)
    if sys.platform != "darwin":
        try:
            value = Path(f"/proc/{pid}/cmdline").read_bytes()
        except OSError:
            fail("process argument observation could not be read")
    else:
        current_processes = snapshot() if processes is None else processes
        info = current_processes.get(pid)
        if info is None:
            fail("process argument observation is unavailable")
        value = info["command"].encode()
    if not value or not value.strip(b"\0"):
        fail("process argument observation is empty")
    require_identity(pid, identity)
    return value

def fd9_identity(pid):
    if sys.platform != "darwin":
        try:
            opened = os.stat(f"/proc/{pid}/fd/9")
        except OSError:
            return None
        return opened.st_dev, opened.st_ino
    result = subprocess.run(
        ["/usr/sbin/lsof", "-F", "pDfin", "-a", "-p", str(pid), "-d", "9"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    fields = {}
    descriptor_found = False
    for line in result.stdout.splitlines():
        if line == "f9":
            descriptor_found = True
        elif line.startswith(("D", "i", "n")):
            fields[line[0]] = line[1:]
    if result.returncode != 0 or not descriptor_found or "D" not in fields or "i" not in fields:
        return None
    try:
        return int(fields["D"], 0), int(fields["i"])
    except ValueError:
        return None

def all_fd9_holders(lock_identity):
    if sys.platform != "darwin":
        holders = []
        for descriptor in Path("/proc").glob("[0-9]*/fd/9"):
            try:
                opened = os.stat(descriptor)
            except OSError:
                continue
            if (opened.st_dev, opened.st_ino) == lock_identity:
                holders.append(int(descriptor.parts[2]))
        return sorted(holders)
    result = subprocess.run(
        ["/usr/sbin/lsof", "-F", "pDfi", "-d", "9"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    holders = []
    current_pid = None
    descriptor_found = False
    fields = {}
    for line in result.stdout.splitlines() + ["p"]:
        if line.startswith("p"):
            if current_pid is not None and descriptor_found and "D" in fields and "i" in fields:
                try:
                    identity = int(fields["D"], 0), int(fields["i"])
                    if identity == lock_identity:
                        holders.append(current_pid)
                except ValueError:
                    pass
            current_pid = int(line[1:]) if line[1:].isdigit() else None
            descriptor_found = False
            fields = {}
        elif line == "f9":
            descriptor_found = True
        elif line.startswith(("D", "i")):
            fields[line[0]] = line[1:]
    return sorted(set(holders))

def process_environment(pid, expected_identity=None):
    identity = require_identity(pid, expected_identity)
    if sys.platform != "darwin":
        try:
            value = Path(f"/proc/{pid}/environ").read_bytes()
        except OSError:
            fail("process environment observation could not be read")
    else:
        try:
            result = subprocess.run(
                ["/bin/ps", "eww", "-p", str(pid), "-o", "command="],
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
            )
        except OSError:
            fail("process environment observation could not be read")
        if result.returncode != 0:
            fail("process environment observation command failed")
        value = result.stdout
    if not value or not value.strip(b"\0"):
        fail("process environment observation is empty")
    if sys.platform == "darwin" and b"HOME=" not in value:
        fail("process environment observation omitted the expected environment marker")
    require_identity(pid, identity)
    return value

def assert_no_token(pid, processes=None, expected_identity=None, extra_tokens=()):
    identity = require_identity(pid, expected_identity)
    environment = process_environment(pid, identity)
    arguments = command_bytes(pid, processes, identity)
    credential_names = tuple(f"{name}=".encode() for name in CREDENTIAL_ENVIRONMENT_NAMES)
    credential_values = tuple(setup_credentials) + tuple(extra_tokens)
    name_found = any(name in environment or name in arguments for name in credential_names)
    value_found = any(value and (value in environment or value in arguments) for value in credential_values)
    require_identity(pid, identity)
    if name_found or value_found:
        fail("installer child environment contains the setup credential")

def installer_command():
    return ["/bin/bash", "scripts/install-generated-workflow-guard.sh"]

def installer_environment():
    environment = os.environ.copy()
    for credential_name in CREDENTIAL_ENVIRONMENT_NAMES:
        environment.pop(credential_name, None)
    environment["HOME"] = str(home_dir)
    return environment

def start_installer(path):
    output = path.open("wb")
    process = subprocess.Popen(
        installer_command(),
        cwd=repository,
        env=installer_environment(),
        stdout=output,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        identity = require_identity(process.pid)
        assert_no_token(process.pid, expected_identity=identity)
    except BaseException:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=10)
        output.close()
        raise
    record("post_setup_installer_parent_credential_absent", "true")
    return process, output, identity

def verify_observer_negative_controls():
    unavailable_pid = 2147483647
    for observation in (
        lambda: process_environment(unavailable_pid),
        lambda: command_bytes(unavailable_pid),
    ):
        try:
            observation()
        except RuntimeError as error:
            if str(error) != "process start identity could not be observed":
                raise
        else:
            fail("unavailable process observation was accepted")
    record("missing_process_identity_rejected", "true")
    if sys.platform == "linux" and os.geteuid() != 0:
        program = "import ctypes\nimport time\nlibc = ctypes.CDLL(None, use_errno=True)\nif libc.prctl(4, 0, 0, 0, 0) != 0:\n    raise SystemExit(91)\nprint('ready', flush=True)\ntime.sleep(30)\n"
        process = subprocess.Popen(
            [sys.executable, "-I", "-S", "-c", program],
            env=installer_environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            if process.stdout is None or not select.select([process.stdout], [], [], 5)[0] or process.stdout.readline() != b"ready\n":
                fail("nondumpable process did not establish the proc observation control")
            identity = require_identity(process.pid)
            try:
                process_environment(process.pid, identity)
            except RuntimeError as error:
                if str(error) != "process environment observation could not be read":
                    raise
            else:
                fail("nondumpable process environment was readable")
            record("unreadable_proc_environment_rejected", "true")
            command_bytes(process.pid, expected_identity=identity)
            record("readable_proc_arguments_control", "true")
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            if process.stdout is not None:
                process.stdout.close()
    elif sys.platform == "linux":
        record("unreadable_proc_observation_control", "unavailable-root")
    else:
        record("unreadable_proc_observation_control", "not-applicable-platform")
    canary = f"jackin-harness-canary-{os.urandom(12).hex()}".encode()
    environment = installer_environment()
    environment["HARNESS_CANARY"] = canary.decode()
    process = subprocess.Popen(
        ["/bin/sleep", "30"],
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    try:
        identity = require_identity(process.pid)
        observed_environment = process_environment(process.pid, identity)
        observed_arguments = command_bytes(process.pid, expected_identity=identity)
        if canary not in observed_environment or not observed_arguments:
            fail("credential canary control did not establish readable identity-bound observations")
        require_identity(process.pid, identity)
        try:
            assert_no_token(process.pid, expected_identity=identity, extra_tokens=(canary,))
        except RuntimeError as error:
            if str(error) != "installer child environment contains the setup credential":
                raise
            record("environment_canary_rejected", "true")
        else:
            fail("observer accepted a harmless credential canary in a real child environment")
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=10)

def stop_owned(pid, group, known_identity=None):
    processes = snapshot()
    info = processes.get(pid)
    identity = start_identity(pid)
    if info is None or info["group"] != group or (known_identity is not None and identity != known_identity):
        fail("refusing to signal a process outside the verified installer process group")
    if not info["state"].startswith("T"):
        os.kill(pid, signal.SIGSTOP)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        current = snapshot().get(pid)
        if current is None:
            fail("verified installer descendant exited before it could be stopped")
        if current["group"] != group or start_identity(pid) != identity:
            fail("verified installer descendant identity changed while stopping it")
        if current["state"].startswith("T"):
            return identity
        time.sleep(0.02)
    fail("verified installer descendant did not enter the stopped state")

def stop_tool_tree(pid, group):
    stopped = {}
    for _ in range(5):
        processes = snapshot()
        if pid not in processes or processes[pid]["group"] != group:
            fail("real Mise process left its installer process group")
        descendants_now = descendants(pid, processes)
        for descendant in descendants_now + [pid]:
            info = snapshot().get(descendant)
            if info is None or info["group"] != group:
                continue
            if descendant not in stopped:
                stopped[descendant] = stop_owned(descendant, group)
            elif not info["state"].startswith("T"):
                stop_owned(descendant, group, stopped[descendant])
        fresh = snapshot()
        remaining = [
            item
            for item in descendants(pid, fresh) + [pid]
            if item in fresh and fresh[item]["group"] == group and not fresh[item]["state"].startswith("T")
        ]
        if not remaining:
            return stopped
    fail("real installer operation descendants could not be stopped as one bounded group")

def resume_tool_tree(stopped, group):
    for pid in reversed(list(stopped)):
        info = snapshot().get(pid)
        if info is None:
            continue
        if info["group"] != group or start_identity(pid) != stopped[pid]:
            fail("refusing to resume a process whose identity changed")
        os.kill(pid, signal.SIGCONT)

def kill_owned_group(group, pid, identity):
    if start_identity(pid) != identity:
        return False
    try:
        if os.getpgid(pid) != group:
            return False
    except ProcessLookupError:
        return False
    try:
        os.killpg(group, signal.SIGKILL)
    except ProcessLookupError:
        return False
    return True

def find_mise_child(process, lock_identity, require_descriptor=True, timeout=180):
    deadline = time.monotonic() + timeout
    parent_identity = start_identity(process.pid)
    while time.monotonic() < deadline:
        if process.poll() is not None:
            fail("actual installer exited before a live Mise installation child was observed")
        processes = snapshot()
        parent = processes.get(process.pid)
        if parent is None or parent["group"] != process.pid or start_identity(process.pid) != parent_identity:
            fail("actual installer parent identity changed during child qualification")
        if fd9_identity(process.pid) != lock_identity:
            time.sleep(0.025)
            continue
        assert_no_token(process.pid, processes, parent_identity)
        for pid in descendants(process.pid, processes):
            info = processes[pid]
            snapshot_command = info["command"].encode()
            if info["group"] != process.pid or mise_artifact not in snapshot_command or b"install" not in snapshot_command or b"python" not in snapshot_command:
                continue
            child_identity = require_identity(pid)
            command = command_bytes(pid, processes, child_identity)
            if mise_artifact not in command or b"install" not in command or b"python" not in command:
                continue
            if os.getpgid(pid) != process.pid:
                fail("selected Mise process left its installer process group")
            child_descriptor = fd9_identity(pid)
            if require_descriptor and child_descriptor != lock_identity:
                continue
            assert_no_token(pid, processes, child_identity)
            require_identity(process.pid, parent_identity)
            record("setup_credential_absent_from_tool_env_argv", "true")
            record("installer_pid", process.pid)
            record("installer_fd9_device", lock_identity[0])
            record("installer_fd9_inode", lock_identity[1])
            record("tool_pid", pid)
            record("tool_command_contains_pinned_mise_install", "true")
            record("tool_fd9_matches_persistent_inode", child_descriptor == lock_identity)
            record("tool_process_start_identity", child_identity)
            return pid, child_identity, parent_identity
        time.sleep(0.025)
    fail("no bounded real Mise install descendant matched the persistent-lock descriptor requirement")

def check_log_for_token(path):
    try:
        before = path.lstat()
        if not stat.S_ISREG(before.st_mode):
            fail("installer evidence log is not a regular file")
        contents = path.read_bytes()
        after = path.lstat()
    except OSError:
        fail("installer evidence log could not be read")
    if (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino) or len(contents) != after.st_size:
        fail("installer evidence log changed during observation")
    if not contents:
        fail("installer evidence log observation is empty")
    credential_names = tuple(name.encode() for name in CREDENTIAL_ENVIRONMENT_NAMES)
    if any(name in contents for name in credential_names) or any(value in contents for value in setup_credentials):
        fail("installer evidence log contains the setup credential")
    return contents

def run_contender(path, timeout=8):
    process, output, identity = start_installer(path)
    try:
        status = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            if process.poll() is None and os.getpgid(process.pid) == process.pid:
                os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=10)
        output.close()
        log = check_log_for_token(path).decode(errors="replace")
        return None, log
    output.close()
    log = check_log_for_token(path).decode(errors="replace")
    return status, log

def require_busy(status, log, phase):
    if status != 75 or "generated-workflows: another trusted checker installation is in progress" not in log:
        fail(f"actual installer did not refuse lock contention {phase}")
    record(f"contender_{phase}_exit", status)
    record(f"contender_{phase}_diagnostic", "native installer lock is held")

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

lock_stat = os.stat(lock_path)
lock_identity = lock_stat.st_dev, lock_stat.st_ino
if not stat.S_ISREG(lock_stat.st_mode) or os.path.islink(lock_path) or stat.S_IMODE(lock_stat.st_mode) & 0o077 or lock_stat.st_nlink != 1 or lock_stat.st_uid != os.getuid():
    fail("persistent installer lock leaf failed the real inode precondition")
sentinel_before = digest(sentinel)
pre_commit_before_mode = stat.S_IMODE(pre_commit.stat().st_mode)
pre_push_before_mode = stat.S_IMODE(pre_push.stat().st_mode)
sentinel_before_mode = stat.S_IMODE(sentinel.stat().st_mode)
record("lock_device", lock_identity[0])
record("lock_inode", lock_identity[1])
record("pre_commit_before", digest(pre_commit))
record("pre_push_before", digest(pre_push))
record("unrelated_sentinel_before", digest(sentinel))
record("pre_commit_before_mode", oct(pre_commit_before_mode))
record("pre_push_before_mode", oct(pre_push_before_mode))
record("unrelated_sentinel_before_mode", oct(sentinel_before_mode))
verify_observer_negative_controls()

first_process = None
first_output = None
publication_process = None
publication_output = None
try:
    first_process, first_output, parent_identity = start_installer(installer_log)
    child_pid, child_identity, parent_identity = find_mise_child(
        first_process,
        lock_identity,
        require_descriptor=mutant != "no-child-lock-retention",
    )
    stopped = stop_tool_tree(child_pid, first_process.pid)
    if first_process.poll() is not None:
        fail("actual installer parent exited while its verified operation child was stopped")
    record("stopped_descendant_count", len(stopped))
    record("stopped_descendant_pids", ",".join(str(pid) for pid in stopped))
    if fd9_identity(first_process.pid) != lock_identity:
        fail("actual installer parent lost descriptor 9 before the live-parent contention check")
    child_lock_identity = fd9_identity(child_pid)
    if mutant == "no-child-lock-retention":
        record("retention_mutant_child_fd9_matches", child_lock_identity == lock_identity)
        if child_lock_identity == lock_identity:
            fail("retention source fault precondition did not close descriptor 9 for the real child")
    elif child_lock_identity != lock_identity:
        fail("verified actual installer child lost descriptor 9 before lock intervention")
    status, log = run_contender(contender_live_log)
    require_busy(status, log, "while_parent_alive")
    if digest(pre_commit) != digest(test_root / "pre-commit.before") or digest(pre_push) != digest(test_root / "pre-push.before") or digest(sentinel) != sentinel_before or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("live-parent lock contention changed hooks or the unrelated sentinel")
    if first_process.poll() is not None or start_identity(first_process.pid) != parent_identity or os.getpgid(first_process.pid) != first_process.pid:
        fail("actual installer parent identity changed before the SIGKILL proof")
    os.kill(first_process.pid, signal.SIGKILL)
    first_process.wait(timeout=10)
    first_output.close()
    record("parent_sigkill_status", first_process.returncode)
    child_still_holds_lock = start_identity(child_pid) == child_identity and fd9_identity(child_pid) == lock_identity
    holders_after_parent = all_fd9_holders(lock_identity)
    record("fd9_holders_after_parent_sigkill", ",".join(str(pid) for pid in holders_after_parent) or "none")
    if not child_still_holds_lock:
        fail("stopped real installer descendant did not retain the lock inode after parent SIGKILL")
    record("stopped_tool_after_parent_sigkill_fd9_matches", "true")
    status, log = run_contender(contender_dead_log, timeout=8)
    require_busy(status, log, "after_parent_sigkill_while_child_alive")
    if digest(pre_commit) != digest(test_root / "pre-commit.before") or digest(pre_push) != digest(test_root / "pre-push.before") or digest(sentinel) != sentinel_before or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("post-parent lock contention changed hooks or the unrelated sentinel")
    kill_owned_group(first_process.pid, child_pid, child_identity)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        holders = all_fd9_holders(lock_identity)
        if not holders:
            break
        time.sleep(0.05)
    else:
        fail("a process still holds descriptor 9 on the persistent lock inode after bounded cleanup")
    record("fixture_fd9_holders_after_cleanup", "none")
    check_log_for_token(installer_log)
    first_process = None
    first_output = None

    publication_process, publication_output, _ = start_installer(publication_log)
    publication_child, publication_child_identity, publication_parent_identity = find_mise_child(
        publication_process,
        lock_identity,
        require_descriptor=True,
    )
    publication_stopped = stop_tool_tree(publication_child, publication_process.pid)
    if publication_process.poll() is not None or fd9_identity(publication_process.pid) != lock_identity or fd9_identity(publication_child) != lock_identity:
        fail("publication-race installer parent and child were not both holding the lock inode")
    foreign = b"#!/bin/bash\nguard_owner=jackin-generated-workflow-guard\nforeign publication-race hook\n"
    pre_commit.write_bytes(foreign)
    os.chmod(pre_commit, 0o700)
    record("publication_intervention", "pre-commit replaced after real child fd9 verification")
    resume_tool_tree(publication_stopped, publication_process.pid)
    status = publication_process.wait(timeout=600)
    publication_output.close()
    publication_text = check_log_for_token(publication_log).decode(errors="replace")
    if status == 0 or "generated-workflows: refusing to replace an existing hook" not in publication_text:
        fail("controlled actual installer run did not refuse the hook changed after preflight")
    if pre_commit.read_bytes() != foreign or stat.S_IMODE(pre_commit.stat().st_mode) != 0o700 or digest(pre_push) != digest(test_root / "pre-push.before") or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or digest(sentinel) != sentinel_before or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("publication-time hook refusal did not preserve both hooks and the unrelated sentinel")
    record("publication_intervention_result", "late foreign pre-commit hook refused and preserved")
    record("pre_commit_after_publication_sha256", digest(pre_commit))
    record("pre_commit_after_publication_mode", oct(stat.S_IMODE(pre_commit.stat().st_mode)))
    record("pre_push_after_pre_commit_race_sha256", digest(pre_push))
    record("pre_push_after_pre_commit_race_mode", oct(stat.S_IMODE(pre_push.stat().st_mode)))
    record("unrelated_sentinel_after_pre_commit_race_sha256", digest(sentinel))
    record("unrelated_sentinel_after_pre_commit_race_mode", oct(stat.S_IMODE(sentinel.stat().st_mode)))
    os.chmod(pre_commit, 0o700)
    pre_commit.write_bytes((test_root / "pre-commit.before").read_bytes())
    publication_process = None
    publication_output = None
    recovery_process, recovery_output, recovery_identity = start_installer(recovery_log)
    try:
        status = recovery_process.wait(timeout=600)
    except subprocess.TimeoutExpired:
        kill_owned_group(recovery_process.pid, recovery_process.pid, recovery_identity)
        recovery_process.wait(timeout=10)
        fail("bounded actual installer recovery timed out")
    recovery_output.close()
    check_log_for_token(recovery_log)
    if status != 0 or digest(pre_commit) != digest(test_root / "pre-commit.before") or digest(pre_push) != digest(test_root / "pre-push.before") or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or digest(sentinel) != sentinel_before or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("actual installer did not recover after publication-time refusal")
    record("recovery_installer_exit", status)

    try:
        installation_root = installation.resolve(strict=True)
        inventory_path = python_inventory.resolve(strict=True)
        inventory_path.relative_to(installation_root)
    except (OSError, ValueError):
        fail("qualified Python inventory is not inside the private installation")
    inventory_stat = python_inventory.lstat()
    inventory_mode = stat.S_IMODE(inventory_stat.st_mode)
    if not stat.S_ISREG(inventory_stat.st_mode) or python_inventory.is_symlink() or inventory_stat.st_uid != os.getuid() or inventory_stat.st_nlink != 1 or inventory_path != python_inventory.absolute():
        fail("qualified Python inventory failed the private regular-file precondition")
    inventory_before = digest(python_inventory)
    if digest(pre_commit) != digest(test_root / "pre-commit.before") or digest(pre_push) != digest(test_root / "pre-push.before") or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or digest(sentinel) != sentinel_before or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("pre-push inventory intervention began with changed hooks or unrelated sentinel")
    record("pre_push_inventory_before_sha256", inventory_before)
    record("pre_push_inventory_before_mode", oct(inventory_mode))
    record("pre_push_inventory_before_device", inventory_stat.st_dev)
    record("pre_push_inventory_before_inode", inventory_stat.st_ino)
    record("pre_push_inventory_before_nlink", inventory_stat.st_nlink)
    os.chmod(python_inventory, inventory_mode | stat.S_IWUSR)
    try:
        with python_inventory.open("ab") as output:
            output.write(b"\nlock lifecycle pre-push publication inventory corruption\n")
    finally:
        os.chmod(python_inventory, inventory_mode)
    inventory_stat_after = python_inventory.lstat()
    inventory_after = digest(python_inventory)
    if inventory_after == inventory_before or stat.S_IMODE(inventory_stat_after.st_mode) != inventory_mode or inventory_stat_after.st_uid != os.getuid() or inventory_stat_after.st_nlink != 1 or (inventory_stat_after.st_dev, inventory_stat_after.st_ino) != (inventory_stat.st_dev, inventory_stat.st_ino):
        fail("pre-push inventory intervention did not preserve the qualified private file mode and identity")
    record("pre_push_inventory_intervention", "private qualified inventory corrupted through filesystem boundary")
    record("pre_push_inventory_after_sha256", inventory_after)
    record("pre_push_inventory_after_mode", oct(stat.S_IMODE(inventory_stat_after.st_mode)))
    record("pre_push_inventory_after_device", inventory_stat_after.st_dev)
    record("pre_push_inventory_after_inode", inventory_stat_after.st_ino)
    record("pre_push_inventory_after_nlink", inventory_stat_after.st_nlink)
    if digest(pre_commit) != digest(test_root / "pre-commit.before") or digest(pre_push) != digest(test_root / "pre-push.before") or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or digest(sentinel) != sentinel_before or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("pre-push inventory intervention changed hooks or the unrelated sentinel")

    publication_process, publication_output, _ = start_installer(publication_log)
    publication_child, publication_child_identity, publication_parent_identity = find_mise_child(
        publication_process,
        lock_identity,
        require_descriptor=True,
    )
    publication_stopped = stop_tool_tree(publication_child, publication_process.pid)
    if publication_process.poll() is not None or fd9_identity(publication_process.pid) != lock_identity or fd9_identity(publication_child) != lock_identity:
        fail("pre-push publication-race installer parent and child were not both holding the lock inode")
    foreign_pre_push = b"#!/bin/bash\nguard_owner=jackin-generated-workflow-guard\nforeign pre-push publication-race hook\n"
    pre_push.write_bytes(foreign_pre_push)
    os.chmod(pre_push, 0o700)
    record("pre_push_publication_intervention", "pre-push replaced after real child fd9 verification")
    resume_tool_tree(publication_stopped, publication_process.pid)
    status = publication_process.wait(timeout=600)
    publication_output.close()
    publication_text = check_log_for_token(publication_log).decode(errors="replace")
    record("pre_push_publication_status", status)
    record("pre_push_after_publication_sha256", digest(pre_push))
    record("pre_push_after_publication_mode", oct(stat.S_IMODE(pre_push.stat().st_mode)))
    if status == 0:
        if pre_push.read_bytes() == foreign_pre_push:
            fail("pre-push publication validation returned success while leaving the foreign hook in place")
        fail("pre-push publication validation admitted and overwrote the late foreign hook")
    if status == 0 or "generated-workflows: refusing to replace an existing hook" not in publication_text:
        fail("controlled actual installer run did not refuse the pre-push hook changed after preflight")
    if pre_push.read_bytes() != foreign_pre_push or stat.S_IMODE(pre_push.stat().st_mode) != 0o700 or digest(pre_commit) != digest(test_root / "pre-commit.before") or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or digest(sentinel) != sentinel_before or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("pre-push publication-time refusal did not preserve both hooks and the unrelated sentinel")
    record("pre_push_publication_intervention_result", "late foreign pre-push hook refused and preserved")
    record("pre_commit_after_pre_push_race_sha256", digest(pre_commit))
    record("pre_commit_after_pre_push_race_mode", oct(stat.S_IMODE(pre_commit.stat().st_mode)))
    record("pre_push_after_publication_sha256", digest(pre_push))
    record("pre_push_after_publication_mode", oct(stat.S_IMODE(pre_push.stat().st_mode)))
    record("unrelated_sentinel_after_pre_push_race_sha256", digest(sentinel))
    record("unrelated_sentinel_after_pre_push_race_mode", oct(stat.S_IMODE(sentinel.stat().st_mode)))
    pre_push.write_bytes((test_root / "pre-push.before").read_bytes())
    os.chmod(pre_push, pre_push_before_mode)
    publication_process = None
    publication_output = None
    recovery_process, recovery_output, recovery_identity = start_installer(push_recovery_log)
    try:
        status = recovery_process.wait(timeout=600)
    except subprocess.TimeoutExpired:
        kill_owned_group(recovery_process.pid, recovery_process.pid, recovery_identity)
        recovery_process.wait(timeout=10)
        fail("bounded actual installer recovery after the pre-push race timed out")
    recovery_output.close()
    check_log_for_token(push_recovery_log)
    if status != 0 or digest(pre_commit) != digest(test_root / "pre-commit.before") or digest(pre_push) != digest(test_root / "pre-push.before") or stat.S_IMODE(pre_commit.stat().st_mode) != pre_commit_before_mode or stat.S_IMODE(pre_push.stat().st_mode) != pre_push_before_mode or digest(sentinel) != sentinel_before or stat.S_IMODE(sentinel.stat().st_mode) != sentinel_before_mode:
        fail("actual installer did not recover after pre-push publication-time refusal")
    record("pre_push_recovery_installer_exit", status)
finally:
    for process, output, candidate, candidate_identity in (
        (first_process, first_output, locals().get("child_pid"), locals().get("child_identity")),
        (publication_process, publication_output, locals().get("publication_child"), locals().get("publication_child_identity")),
    ):
        if process is None:
            continue
        if process.poll() is None:
            kill_owned_group(process.pid, process.pid, start_identity(process.pid))
        elif candidate is not None and candidate_identity is not None:
            kill_owned_group(process.pid, candidate, candidate_identity)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            pass
        if output is not None and not output.closed:
            output.close()
PY
  assert_hooks_match_baseline
  run_installed_hooks
  printf 'real installer descendant retains descriptor 9 through parent SIGKILL; pre-commit and pre-push publication races refuse and recovery succeeds: PASS\n' >> "$test_root/case-results.log"
  printf 'real installer child lock lifecycle and both hook publication races: PASS\n'
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi

relative_git_directory='relative git tools'
mkdir "$repository/$relative_git_directory"
ln -s "$git_command" "$repository/$relative_git_directory/git"
cat > "$test_root/staged git sentinel" <<EOF
#!/bin/bash
/bin/touch '$attacker_marker'
exit 93
EOF
chmod 755 "$test_root/staged git sentinel"
sentinel_oid=$("$git_command" -C "$repository" hash-object -w --stdin < "$test_root/staged git sentinel")
"$git_command" -C "$repository" update-index --add --cacheinfo "100755,$sentinel_oid,$relative_git_directory/git"
relative_index=$("$git_command" -C "$repository" rev-parse --path-format=absolute --git-path index)
relative_index_before=$(sha256_file "$relative_index")
relative_link_before=$(readlink "$repository/$relative_git_directory/git")
relative_objects_before=$("$git_command" -C "$repository" cat-file --batch-all-objects --batch-check='%(objectname)' | /usr/bin/sort)
relative_checker_status=0
if (cd "$repository" && HOME="$home_dir" PATH="$relative_git_directory:/usr/bin:/bin:/usr/sbin:/sbin" /bin/bash "$trusted_checker" pre-commit) > "$test_root/relative-git-match.log" 2>&1; then
  relative_checker_status=0
else
  relative_checker_status=$?
fi
if [[ "$public_mutant" == relative-git-path ]]; then
  if [[ "$relative_checker_status" -eq 0 || ! -e "$attacker_marker" ]]; then
    cat "$test_root/relative-git-match.log" >&2
    printf 'generated-workflows: relative Git path mutant precondition did not execute the staged sentinel at final comparison\n' >&2
    exit 1
  fi
fi
if [[ -e "$attacker_marker" ]]; then
  printf 'generated-workflows: staged git sentinel executed during a matching comparison\n' >&2
  exit 1
fi
if [[ "$relative_checker_status" -ne 0 ]]; then
  cat "$test_root/relative-git-match.log" >&2
  printf 'generated-workflows: installed checker failed a matching snapshot with a relative caller PATH\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$relative_index")" != "$relative_index_before" || "$(readlink "$repository/$relative_git_directory/git")" != "$relative_link_before" || "$("$git_command" -C "$repository" cat-file --batch-all-objects --batch-check='%(objectname)' | /usr/bin/sort)" != "$relative_objects_before" ]]; then
  printf 'generated-workflows: relative Git path comparison changed source index, worktree link, or Git objects\n' >&2
  exit 1
fi
printf 'relative PATH staged executable Git sentinel reaches matching real generator comparison: PASS, not executed\n' >> "$test_root/case-results.log"
printf 'relative PATH staged executable Git sentinel reaches matching real generator comparison: PASS, not executed\n'
if [[ "$e2e_focus" == relative-git-path ]]; then
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if ! (cd "$repository/docs" && HOME="$home_dir" GIT_INDEX_FILE=../.git/index /bin/bash "$trusted_checker" pre-commit) > "$test_root/relative-index-match.log" 2>&1; then
  cat "$test_root/relative-index-match.log" >&2
  printf 'generated-workflows: relative effective index was not resolved from its caller directory\n' >&2
  exit 1
fi
printf 'relative effective index from subdirectory: PASS\n' >> "$test_root/case-results.log"
space_git_dir="$test_root/git tools"
mkdir "$space_git_dir"
ln -s "$git_command" "$space_git_dir/git"
if ! (cd "$repository" && HOME="$home_dir" PATH="$space_git_dir:/usr/bin:/bin:/usr/sbin:/sbin" /bin/bash "$trusted_checker" pre-commit) > "$test_root/git-path-spaces.log" 2>&1; then
  cat "$test_root/git-path-spaces.log" >&2
  printf 'generated-workflows: installed checker rejected a caller Git executable path containing spaces\n' >&2
  exit 1
fi
printf 'installed checker with Git executable path containing spaces: PASS\n' >> "$test_root/case-results.log"
if [[ "$e2e_focus" == git-path-spaces ]]; then
  record_completion_witness "focus:$e2e_focus"
  mark_completion_complete
  exit 0
fi
if [[ "$(uname -s)" == Darwin && -x /usr/bin/sandbox-exec ]]; then
  network_profile='(version 1)(allow default)(deny network*)'
  printf '%s\n' "$network_profile" > "$test_root/network-deny.sb"
  if /usr/bin/sandbox-exec -p "$network_profile" /usr/bin/env -i PATH=/usr/bin:/bin HOME=/tmp /usr/bin/curl --disable --noproxy '*' --connect-timeout 2 --max-time 4 --silent --show-error https://1.1.1.1/ > "$test_root/network-control.stdout" 2> "$test_root/network-control.log"; then
    printf 'generated-workflows: network-denied curl positive control unexpectedly connected\n' >&2
    exit 1
  else
    network_control_status=$?
  fi
  printf 'curl_exit=%s\n' "$network_control_status" > "$test_root/network-control.result"
  network_enforcement=macos-sandbox-exec-deny-network
  for warm_run in 1 2; do
    if ! (cd "$repository" && HOME="$home_dir" /usr/bin/sandbox-exec -p "$network_profile" /bin/bash "$trusted_checker" pre-commit) > "$test_root/offline-warm-$warm_run.log" 2>&1; then
      cat "$test_root/offline-warm-$warm_run.log" >&2
      printf 'generated-workflows: warmed installed checker failed with network access denied\n' >&2
      exit 1
    fi
  done
  printf 'two warm installed pre-commit runs under network-denied sandbox: PASS\n' >> "$test_root/case-results.log"
fi
printf 'name: intentional drift\n' > "$repository/.github/workflows/intentional-drift.yml"
"$git_command" -C "$repository" add -- .github/workflows/intentional-drift.yml
if (cd "$repository" && HOME="$home_dir" PATH="$relative_git_directory:/usr/bin:/bin:/usr/sbin:/sbin" /bin/bash "$trusted_checker" pre-commit) > "$test_root/relative-git-drift.log" 2>&1; then
  printf 'generated-workflows: installed checker accepted drift with a relative caller PATH\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: drift in index snapshot' "$test_root/relative-git-drift.log" || [[ -e "$attacker_marker" ]]; then
  cat "$test_root/relative-git-drift.log" >&2
  printf 'generated-workflows: relative PATH drift case failed before the real generator comparison\n' >&2
  exit 1
fi
printf 'relative PATH drift reached real generator comparison: REJECTED\n' >> "$test_root/case-results.log"
"$git_command" -C "$repository" reset --quiet HEAD -- .github/workflows/intentional-drift.yml
rm "$repository/.github/workflows/intentional-drift.yml"

if ! HOME="$home_dir" "$git_command" -C "$repository" commit --allow-empty --quiet -m generated-workflow-match; then
  printf 'generated-workflows: real Velnor rejected matching generated workflows\n' >&2
  exit 1
fi
printf 'matching real commit hook: PASS\n' >> "$test_root/case-results.log"
push_main_and_verify() {
  local description=$1
  local push_log=$2
  if ! HOME="$home_dir" "$git_command" -C "$repository" push --quiet origin main >"$push_log" 2>&1; then
    cat "$push_log" >&2
    if /usr/bin/grep -E -q 'shallow update not allowed|deny updating a hidden ref|unpacker error' "$push_log"; then
      printf 'generated-workflows: receiver rejected %s (pre-push hook result is recorded separately)\n' "$description" >&2
    else
      printf 'generated-workflows: pre-push hook or transport rejected %s\n' "$description" >&2
    fi
    return 1
  fi
  local client_head
  local receiver_head
  client_head=$("$git_command" -C "$repository" rev-parse HEAD)
  receiver_head=$("$git_command" -C "$remote" rev-parse refs/heads/main)
  if [[ "$receiver_head" != "$client_head" ]]; then
    printf 'generated-workflows: receiver ref %s does not match client HEAD\n' "$receiver_head" >&2
    return 1
  fi
}
if ! push_main_and_verify 'matching generated workflows' "$test_root/matching-push.log"; then
  printf 'generated-workflows: matching pre-push hook and receiver update did not both succeed\n' >&2
  exit 1
fi
printf 'matching real pre-push hook: PASS\n' >> "$test_root/case-results.log"

cat > "$repository/scripts/branch-hk-sentinel.sh" <<EOF
#!/bin/bash
/bin/touch '$attacker_marker'
EOF
chmod 755 "$repository/scripts/branch-hk-sentinel.sh"
cat > "$repository/hk.pkl" <<'EOF'
amends "package://github.com/jdx/hk/releases/download/v2.11.1/hk@2.11.1#/Config.pkl"
hooks {
  ["pre-commit"] {
    steps {
      ["hostile"] {
        check = "bash scripts/branch-hk-sentinel.sh"
      }
    }
  }
}
EOF
cp "$repository/hk.pkl" "$repository/hk.local.pkl"
mkdir -p "$home_dir/.config/hk"
cp "$repository/hk.pkl" "$home_dir/.config/hk/hk.pkl"
cat > "$repository/scripts/check-generated-workflows.sh" <<EOF
#!/bin/bash
/bin/touch '$attacker_marker'
exit 94
EOF
chmod 755 "$repository/scripts/check-generated-workflows.sh"
"$git_command" -C "$repository" add -- scripts/branch-hk-sentinel.sh hk.pkl hk.local.pkl scripts/check-generated-workflows.sh
if ! HOME="$home_dir" "$git_command" -C "$repository" commit --allow-empty --quiet -m hostile-checkout-configuration; then
  printf 'generated-workflows: installed checker rejected matching hostile checkout and user hk configuration\n' >&2
  exit 1
fi
if [[ -e "$attacker_marker" ]]; then
  printf 'generated-workflows: checkout or user hk configuration executed a sentinel\n' >&2
  exit 1
fi
if ! push_main_and_verify 'hostile checkout and user hk configuration' "$test_root/hostile-push.log"; then
  printf 'generated-workflows: hostile checkout pre-push hook and receiver update did not both succeed\n' >&2
  exit 1
fi
if [[ -e "$attacker_marker" ]]; then
  printf 'generated-workflows: checkout or user hk configuration executed during push\n' >&2
  exit 1
fi

printf 'selected change\n' > "$repository/docs/selected with spaces.md"
"$git_command" -C "$repository" add -- 'docs/selected with spaces.md'
printf 'name: staged invalid workflow\n' > "$repository/.github/workflows/staged-invalid.yml"
"$git_command" -C "$repository" add -- .github/workflows/staged-invalid.yml
if ! HOME="$home_dir" "$git_command" -C "$repository" commit --only --quiet -m commit-only-selected -- 'docs/selected with spaces.md'; then
  printf 'generated-workflows: commit --only validated the ordinary index instead of its effective selected index\n' >&2
  exit 1
fi
if [[ "$("$git_command" -C "$repository" diff --cached --name-only)" != .github/workflows/staged-invalid.yml ]]; then
  printf 'generated-workflows: commit --only changed an unrelated staged workflow\n' >&2
  exit 1
fi
printf 'commit --only selected valid path with unrelated staged drift: PASS\n' >> "$test_root/case-results.log"
"$git_command" -C "$repository" reset --quiet HEAD -- .github/workflows/staged-invalid.yml
rm "$repository/.github/workflows/staged-invalid.yml"

"$git_command" -C "$repository" add -- .github/workflows/ci.yml
printf 'name: selected invalid workflow\n' > "$repository/.github/workflows/ci.yml"
if HOME="$home_dir" "$git_command" -C "$repository" commit --only --quiet -m commit-only-invalid -- .github/workflows/ci.yml > "$test_root/commit-only-invalid.log" 2>&1; then
  printf 'generated-workflows: commit --only accepted an invalid selected snapshot\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: drift in index snapshot' "$test_root/commit-only-invalid.log"; then
  cat "$test_root/commit-only-invalid.log" >&2
  printf 'generated-workflows: invalid commit --only case failed before generator comparison\n' >&2
  exit 1
fi
printf 'commit --only selected invalid workflow: REJECTED by real generator comparison\n' >> "$test_root/case-results.log"
"$git_command" -C "$repository" reset --quiet HEAD -- .github/workflows/ci.yml
"$git_command" -C "$repository" checkout --quiet -- .github/workflows/ci.yml

missing_index="$test_root/missing explicit index"
if (cd "$repository" && HOME="$home_dir" GIT_INDEX_FILE="$missing_index" /bin/bash "$trusted_checker" pre-commit) > "$test_root/missing-index.log" 2>&1; then
  printf 'generated-workflows: missing explicit effective index was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'effective index is missing or not a regular file' "$test_root/missing-index.log"; then
  cat "$test_root/missing-index.log" >&2
  printf 'generated-workflows: missing explicit index failed at an unexpected boundary\n' >&2
  exit 1
fi
mkdir "$test_root/invalid explicit index"
if (cd "$repository" && HOME="$home_dir" GIT_INDEX_FILE="$test_root/invalid explicit index" /bin/bash "$trusted_checker" pre-commit) > "$test_root/invalid-index.log" 2>&1; then
  printf 'generated-workflows: non-file explicit effective index was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'effective index is missing or not a regular file' "$test_root/invalid-index.log"; then
  cat "$test_root/invalid-index.log" >&2
  printf 'generated-workflows: invalid explicit index failed at an unexpected boundary\n' >&2
  exit 1
fi

"$git_command" -C "$repository" config core.splitIndex true
"$git_command" -C "$repository" update-index --split-index
linked_repository="$test_root/linked worktree"
"$git_command" -C "$repository" worktree add --quiet -b linked-qualification "$linked_repository" main
linked_git_directory=$("$git_command" -C "$linked_repository" rev-parse --path-format=absolute --git-dir)
linked_index=$("$git_command" -C "$linked_repository" rev-parse --path-format=absolute --git-path index)
linked_shared=$(GIT_INDEX_FILE="$linked_index" "$git_command" -C "$linked_repository" rev-parse --shared-index-path)
if [[ -z "$linked_shared" ]]; then
  printf 'generated-workflows: linked worktree fixture did not create a split index\n' >&2
  exit 1
fi
if [[ "$linked_shared" != /* ]]; then
  linked_shared="$linked_repository/$linked_shared"
fi
linked_index_before=$(sha256_file "$linked_index")
linked_shared_before=$(sha256_file "$linked_shared")
if ! (cd "$linked_repository" && HOME="$home_dir" "$hooks_path/pre-commit"); then
  printf 'generated-workflows: linked worktree split-index hook failed\n' >&2
  exit 1
fi
if [[ "$linked_index_before" != "$(sha256_file "$linked_index")" || "$linked_shared_before" != "$(sha256_file "$linked_shared")" ]]; then
  printf 'generated-workflows: linked worktree split-index hook changed its source index or base\n' >&2
  exit 1
fi
printf 'linked worktree split index unchanged after hook: PASS\n' >> "$test_root/case-results.log"
custom_index="$linked_git_directory/index with spaces"
cp -p "$linked_index" "$custom_index"
custom_shared=$(GIT_INDEX_FILE="$custom_index" "$git_command" -C "$linked_repository" rev-parse --shared-index-path)
if [[ "$custom_shared" != /* ]]; then
  custom_shared="$linked_repository/$custom_shared"
fi
custom_index_before=$(sha256_file "$custom_index")
custom_shared_before=$(sha256_file "$custom_shared")
if ! (cd "$linked_repository" && HOME="$home_dir" GIT_INDEX_FILE="$custom_index" "$hooks_path/pre-commit"); then
  printf 'generated-workflows: custom split-index hook failed\n' >&2
  exit 1
fi
if [[ "$custom_index_before" != "$(sha256_file "$custom_index")" || "$custom_shared_before" != "$(sha256_file "$custom_shared")" ]]; then
  printf 'generated-workflows: custom split-index hook changed its source index or base\n' >&2
  exit 1
fi
printf 'custom split index with spaces unchanged after hook: PASS\n' >> "$test_root/case-results.log"
linked_config=$("$git_command" -C "$linked_repository" rev-parse --path-format=absolute --git-common-dir)/config
linked_worktree_config="$linked_git_directory/config.worktree"
linked_head_before=$("$git_command" -C "$linked_repository" rev-parse HEAD)
linked_refs_before=$("$git_command" -C "$linked_repository" show-ref --head 2>/dev/null || true)
linked_index_before_missing=$(sha256_file "$linked_index")
linked_config_before=$(sha256_file "$linked_config")
if [[ -f "$linked_worktree_config" ]]; then
  linked_worktree_config_before=$(sha256_file "$linked_worktree_config")
else
  linked_worktree_config_before=absent
fi
linked_objects_before=$("$git_command" -C "$linked_repository" cat-file --batch-all-objects --batch-check='%(objectname) %(objecttype) %(objectsize)' | /usr/bin/sort)
linked_shared_before_missing=$(sha256_file "$linked_shared")
mv "$linked_shared" "$test_root/linked shared index backup"
if [[ "$network_enforcement" == macos-sandbox-exec-deny-network ]]; then
  if (cd "$linked_repository" && HOME="$home_dir" /usr/bin/sandbox-exec -p "$network_profile" /bin/bash "$hooks_path/pre-commit") > "$test_root/missing-shared-index.log" 2>&1; then
    printf 'generated-workflows: missing split-index base was accepted\n' >&2
    exit 1
  fi
elif (cd "$linked_repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/missing-shared-index.log" 2>&1; then
  printf 'generated-workflows: missing split-index base was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: cannot resolve the effective split-index base' "$test_root/missing-shared-index.log" && ! /usr/bin/grep -F -q 'generated-workflows: effective split-index base is unavailable' "$test_root/missing-shared-index.log"; then
  cat "$test_root/missing-shared-index.log" >&2
  printf 'generated-workflows: missing split-index base was not rejected at source resolution\n' >&2
  exit 1
fi
if [[ -e "$linked_shared" || -L "$linked_shared" || "$(sha256_file "$linked_index")" != "$linked_index_before_missing" || "$(sha256_file "$linked_config")" != "$linked_config_before" || "$("$git_command" -C "$linked_repository" rev-parse HEAD)" != "$linked_head_before" || "$("$git_command" -C "$linked_repository" show-ref --head 2>/dev/null || true)" != "$linked_refs_before" || "$("$git_command" -C "$linked_repository" cat-file --batch-all-objects --batch-check='%(objectname) %(objecttype) %(objectsize)' | /usr/bin/sort)" != "$linked_objects_before" ]]; then
  printf 'generated-workflows: missing split-index refusal changed source index, configuration, refs, or Git objects\n' >&2
  exit 1
fi
if [[ "$linked_worktree_config_before" == absent ]]; then
  if [[ -e "$linked_worktree_config" || -L "$linked_worktree_config" ]]; then
    printf 'generated-workflows: missing split-index refusal created worktree configuration\n' >&2
    exit 1
  fi
elif [[ ! -f "$linked_worktree_config" || "$(sha256_file "$linked_worktree_config")" != "$linked_worktree_config_before" ]]; then
  printf 'generated-workflows: missing split-index refusal changed worktree configuration\n' >&2
  exit 1
fi
printf 'missing split-index base: REJECTED with source state unchanged\n' >> "$test_root/case-results.log"
mv "$test_root/linked shared index backup" "$linked_shared"
if [[ "$(sha256_file "$linked_shared")" != "$linked_shared_before_missing" ]]; then
  printf 'generated-workflows: restored split-index base differs from its original bytes\n' >&2
  exit 1
fi
if ! (cd "$linked_repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/restored-shared-index.log" 2>&1; then
  cat "$test_root/restored-shared-index.log" >&2
  printf 'generated-workflows: restored split-index base did not pass the public hook\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$linked_index")" != "$linked_index_before_missing" || "$(sha256_file "$linked_shared")" != "$linked_shared_before_missing" ]]; then
  printf 'generated-workflows: restored split-index hook changed its source index or base\n' >&2
  exit 1
fi
printf 'restored split-index base: public hook PASS with index and base unchanged\n' >> "$test_root/case-results.log"

printf 'name: intentional drift\n' > "$repository/.github/workflows/intentional-drift.yml"
"$git_command" -C "$repository" add -- .github/workflows/intentional-drift.yml
if HOME="$home_dir" "$git_command" -C "$repository" commit --quiet -m generated-workflow-drift > "$test_root/drift.log" 2>&1; then
  printf 'generated-workflows: real Velnor accepted a generated-workflow drift\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: drift in index snapshot' "$test_root/drift.log"; then
  cat "$test_root/drift.log" >&2
  printf 'generated-workflows: drift case failed before the real generator comparison\n' >&2
  exit 1
fi
printf 'staged workflow drift reached real generator comparison: REJECTED\n' >> "$test_root/case-results.log"

tool_cache_generation=$(find "$installation/cache/tools" -mindepth 1 -maxdepth 1 -type d ! -name '.stage*' -print -quit)
if [[ -z "$tool_cache_generation" || ! -f "$tool_cache_generation/tool-state/mise-config/mise.lock" ]]; then
  printf 'generated-workflows: qualified tool cache is unavailable for the integrity case\n' >&2
  exit 1
fi
tool_lock="$tool_cache_generation/tool-state/mise-config/mise.lock"
cp -p "$tool_lock" "$test_root/tool-lock.original"
chmod u+w "$tool_lock"
printf '%s\n' 'invalid = true' >> "$tool_lock"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/corrupt-tool-cache.log" 2>&1; then
  printf 'generated-workflows: corrupted persistent tool cache was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'qualified tool cache payload changed; refusing to execute it' "$test_root/corrupt-tool-cache.log"; then
  cat "$test_root/corrupt-tool-cache.log" >&2
  printf 'generated-workflows: corrupt tool cache failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'corrupt persistent tool cache: REJECTED before execution\n' >> "$test_root/case-results.log"
cp -p "$test_root/tool-lock.original" "$tool_lock"

python_inventory="$python_artifact_dir/inventory"
cp -p "$python_inventory" "$test_root/python-inventory.original"
chmod u+w "$python_inventory"
printf '%s\n' 'invalid inventory' >> "$python_inventory"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/corrupt-python-inventory.log" 2>&1; then
  printf 'generated-workflows: corrupted private Python inventory was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'pinned Python 3.15.2 or its inventory is missing or changed' "$test_root/corrupt-python-inventory.log"; then
  cat "$test_root/corrupt-python-inventory.log" >&2
  printf 'generated-workflows: corrupt Python inventory failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'corrupt private Python inventory: REJECTED before interpreter execution\n' >> "$test_root/case-results.log"
cp -p "$test_root/python-inventory.original" "$python_inventory"

trusted_config="$installation/versions/$checker_digest/config.toml"
cp -p "$trusted_config" "$test_root/trusted-config.original"
chmod u+w "$trusted_config"
printf '%s' 'tampered=true' >> "$trusted_config"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/tampered-config.log" 2>&1; then
  printf 'generated-workflows: altered installed checker configuration was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'installed checker artifacts failed integrity validation' "$test_root/tampered-config.log"; then
  cat "$test_root/tampered-config.log" >&2
  printf 'generated-workflows: altered installed configuration failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'tampered installed configuration: REJECTED before checker execution\n' >> "$test_root/case-results.log"
cp -p "$test_root/trusted-config.original" "$trusted_config"

cp -p "$trusted_checker" "$test_root/trusted-checker.untampered"
chmod u+w "$trusted_checker"
printf '%s' 'tampered checker' >> "$trusted_checker"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/tampered-checker.log" 2>&1; then
  printf 'generated-workflows: altered installed checker was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'trusted checker failed its launcher integrity check' "$test_root/tampered-checker.log"; then
  cat "$test_root/tampered-checker.log" >&2
  printf 'generated-workflows: altered installed checker failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'tampered installed checker: REJECTED by launcher digest\n' >> "$test_root/case-results.log"
cp -p "$test_root/trusted-checker.untampered" "$trusted_checker"

checker_version_dir=${trusted_checker%/*}
case "$(uname -s)" in
  Darwin)
    checker_version_mode=$(/usr/bin/stat -f '%Lp' "$checker_version_dir")
    checker_version_mode_now() { /usr/bin/stat -f '%Lp' "$checker_version_dir"; }
    ;;
  Linux)
    checker_version_mode=$(/usr/bin/stat -c '%a' "$checker_version_dir")
    checker_version_mode_now() { /usr/bin/stat -c '%a' "$checker_version_dir"; }
    ;;
esac
chmod u+w "$checker_version_dir"
mv "$trusted_checker" "$test_root/trusted-checker.original"
ln -s "$test_root/trusted-checker.original" "$trusted_checker"
chmod "$checker_version_mode" "$checker_version_dir"
if [[ "$(checker_version_mode_now)" != "$checker_version_mode" || ! -L "$trusted_checker" ]]; then
  printf 'generated-workflows: symlink fixture did not preserve the installed version directory mode\n' >&2
  exit 1
fi
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/symlink-checker.log" 2>&1; then
  printf 'generated-workflows: symlinked installed checker was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'trusted checker installation is unsafe' "$test_root/symlink-checker.log"; then
  cat "$test_root/symlink-checker.log" >&2
  printf 'generated-workflows: symlinked installed checker failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'symlinked installed checker: REJECTED by launcher boundary\n' >> "$test_root/case-results.log"
chmod u+w "$checker_version_dir"
rm "$trusted_checker"
mv "$test_root/trusted-checker.original" "$trusted_checker"
chmod "$checker_version_mode" "$checker_version_dir"
if [[ "$(checker_version_mode_now)" != "$checker_version_mode" || -L "$trusted_checker" ]]; then
  printf 'generated-workflows: restored checker fixture did not preserve the installed version directory mode\n' >&2
  exit 1
fi

cp -p "$test_root/checkout-checker.original" "$repository/scripts/check-generated-workflows.sh"
foreign_hooks="$test_root/foreign hooks"
mkdir "$foreign_hooks"
printf '#!/bin/sh\nexit 0\n' > "$foreign_hooks/pre-commit"
chmod 755 "$foreign_hooks/pre-commit"
foreign_hook_before=$(sha256_file "$foreign_hooks/pre-commit")
"$git_command" -C "$repository" config core.hooksPath "$foreign_hooks"
if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/foreign-hook-install.log" 2>&1; then
  printf 'generated-workflows: installer replaced a foreign pre-commit hook\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'refusing to replace an existing hook' "$test_root/foreign-hook-install.log" || [[ "$foreign_hook_before" != "$(sha256_file "$foreign_hooks/pre-commit")" ]]; then
  cat "$test_root/foreign-hook-install.log" >&2
  printf 'generated-workflows: foreign pre-commit hook was not preserved after explicit refusal\n' >&2
  exit 1
fi
printf 'foreign pre-commit hook: installer refused and preserved bytes\n' >> "$test_root/case-results.log"
printf 'PASS real installed checker accepts matching generated workflows and rejects drift\n'
record_completion_witness "focus:$e2e_focus"
mark_completion_complete
