#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
umask 022
repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64)
    platform=macos-arm64
    hash_tool=/usr/bin/shasum
    ;;
  Linux:x86_64|Linux:amd64)
    platform=linux-x64
    hash_tool=/usr/bin/sha256sum
    ;;
  *)
    printf 'generated-workflows: unsupported host for the installer recovery test\n' >&2
    exit 2
    ;;
esac
sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | /usr/bin/awk '{print $1}'
  else
    "$hash_tool" "$1" | /usr/bin/awk '{print $1}'
  fi
}
file_identity() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%d:%i:%u:%Lp:%l' "$1"
  else
    /usr/bin/stat -c '%d:%i:%u:%a:%h' "$1"
  fi
}
file_size() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%z' "$1"
  else
    /usr/bin/stat -c '%s' "$1"
  fi
}
file_mode() {
  if [[ "$platform" == macos-arm64 ]]; then
    /usr/bin/stat -f '%Lp' "$1"
  else
    /usr/bin/stat -c '%a' "$1"
  fi
}
fingerprint_tree() {
  local root=$1
  local path
  while IFS= read -r path; do
    if [[ -L "$path" ]]; then
      printf 'l %s -> %s\n' "${path#"$root"/}" "$(/usr/bin/readlink "$path")"
    elif [[ -d "$path" ]]; then
      printf 'd %s\n' "${path#"$root"/}"
    else
      printf 'f %s %s %s %s\n' "${path#"$root"/}" "$(file_identity "$path")" "$(file_size "$path")" "$(sha256_file "$path")"
    fi
  done < <(/usr/bin/find "$root" -mindepth 1 | LC_ALL=C /usr/bin/sort)
}
probe_lock_free() {
  /usr/bin/perl - "$1" <<'PERL'
use strict;
use warnings;
use Fcntl qw(:flock);
my $path = $ARGV[0];
exit 2 unless defined $path && -f $path;
open(my $handle, '<', $path) or exit 2;
my $locked = flock($handle, LOCK_EX | LOCK_NB);
if ($locked) {
    close($handle);
    exit 0;
}
exit 1 if $!{EAGAIN} || $!{EWOULDBLOCK};
exit 2;
PERL
}
test_root=$(mktemp -d "${TMPDIR:-/tmp}/installer-recovery-test.XXXXXX")
cleanup_test_root() {
  find "$test_root" -type d -exec chmod u+rwx {} + 2>/dev/null || true
  find "$test_root" -type f -exec chmod u+w {} + 2>/dev/null || true
  find "$test_root" -depth -delete 2>/dev/null || true
}
trap cleanup_test_root EXIT
sandbox="$test_root/sandbox"
home_dir="$test_root/home"
installer="$sandbox/scripts/install-generated-workflow-guard.sh"
mutant_installer="$sandbox/scripts/install-generated-workflow-guard-preflight-first.sh"
mkdir -p "$sandbox/scripts" "$home_dir"
for input in generated-workflow-guard-config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects trusted-seed-cargo.lock trusted-seed-cargo-policy.json python-mise.toml python-mise.macos-arm64.lock.toml python-mise.linux-x64.lock.toml; do
  cp -p "$repo_root/scripts/$input" "$sandbox/scripts/$input"
done
cp -p "$repo_root/scripts/install-generated-workflow-guard.sh" "$installer"
cat > "$sandbox/scripts/check-generated-workflows.sh" <<'CHECKER'
#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
exit 0
CHECKER
chmod 500 "$sandbox/scripts/check-generated-workflows.sh"
cat > "$test_root/fake-mise" <<'MISE'
#!/bin/bash
set -euo pipefail
stage=
command=
while (($#)); do
  case "$1" in
    --cd) stage=${2:?}; shift 2 ;;
    --locked) shift ;;
    *) command=${1:?}; shift; break ;;
  esac
done
marker_dir=$(/usr/bin/dirname "$0")
case "$command" in
  config)
    printf '{\n'
    printf '  "files": [\n'
    printf '    "path": "%s",\n' "$stage/mise.toml"
    printf '    "path": "%s",\n' "$stage/global/config.toml"
    printf '    "path": "%s",\n' "$stage/system/config.toml"
    printf '  ]\n'
    printf '}\n'
    ;;
  install)
    if [[ -f "$marker_dir/hold-lock" ]]; then
      printf '%s\n' "$$" > "$marker_dir/mise-running"
      sleep 30 &
      sleep_pid=$!
      wait "$sleep_pid" || true
    fi
    runtime="$stage/data/installs/python/3.15.2"
    mkdir -p "$runtime/bin"
    true_binary=/usr/bin/true
    if [[ ! -x "$true_binary" ]]; then
      true_binary=/bin/true
    fi
    cp "$true_binary" "$runtime/bin/python3.15"
    chmod 755 "$runtime/bin/python3.15"
    ;;
  *)
    printf 'fake mise: unsupported command %s\n' "$command" >&2
    exit 64
    ;;
esac
MISE
chmod 500 "$test_root/fake-mise"
cat > "$test_root/stub-curl" <<STUB
#!/bin/bash
set -euo pipefail
output=
while ((\$#)); do
  case "\$1" in
    --output) output=\$2; shift 2 ;;
    *) shift ;;
  esac
done
if [[ -z "\$output" ]]; then
  printf 'stub curl: no download target\n' >&2
  exit 64
fi
if [[ -e "$test_root/stub-curl-wrong-bytes" ]]; then
  printf 'stub curl: payload that must fail the reviewed checksum\n' > "\$output"
  exit 0
fi
/bin/cat "$test_root/fake-mise" > "\$output"
STUB
chmod 500 "$test_root/stub-curl"
/usr/bin/git init -q "$sandbox"
python3 - "$installer" "$(sha256_file "$sandbox/scripts/check-generated-workflows.sh")" "$(sha256_file "$test_root/fake-mise")" "$platform" "$test_root/stub-curl" <<'PY'
import re
import shlex
import sys

path, checker_sha, mise_sha, platform, curl_stub = sys.argv[1:]
source = open(path).read()
source = re.sub(r"^reviewed_checker_sha256=.*$", f"reviewed_checker_sha256={checker_sha}", source, count=1, flags=re.M)
pattern = re.compile(rf"(platform={re.escape(platform)}\n(\s*)mise_sha256=)[0-9a-f]{{64}}")
source, count = pattern.subn(lambda match: f"{match.group(1)}{mise_sha}", source, count=1)
if count != 1:
    raise SystemExit("generated-workflows: the reviewed Mise pin was not rewritten")
boundary = "/usr/bin/curl "
if source.count(boundary) != 1:
    raise SystemExit("generated-workflows: the pinned Mise download boundary was not found")
source = source.replace(boundary, f"{shlex.quote(curl_stub)} ", 1)
open(path, "w").write(source)
PY
python3 - "$installer" "$mutant_installer" <<'PY'
import sys

preflight = (
    "validate_hook_pair || exit 1\n"
    "capture_hook_baseline || exit 1\n"
    "validate_hook_pair || exit 1\n"
    "assert_expected_hook_states || {\n"
    "  printf 'generated-workflows: existing hook changed during initial validation\\n' >&2\n"
    "  exit 1\n"
    "}\n"
)
marker = "umask 077\n"
source = open(sys.argv[1]).read()
if source.count(preflight) != 1:
    raise SystemExit("generated-workflows: the guarded hook preflight block was not found for the ordering mutant")
if source.count(marker) != 1:
    raise SystemExit("generated-workflows: the ordering mutant marker was not found")
open(sys.argv[2], "w").write(source.replace(marker, preflight + marker))
PY
chmod 500 "$mutant_installer"
mise_version=$(/usr/bin/sed -n 's/^mise_version=//p' "$installer")
python_version=$(/usr/bin/sed -n 's/^python_version=//p' "$installer")
reviewed_checker_sha256=$(/usr/bin/sed -n 's/^reviewed_checker_sha256=//p' "$installer")
install_root="$sandbox/.git/jackin-generated-workflow-guard"
version_dir="$install_root/versions/$reviewed_checker_sha256"
lock_file="$install_root/install.advisory.lock"
mise_dir="$install_root/artifacts/mise/$mise_version/$platform"
python_dir="$install_root/artifacts/python/$python_version"
mkdir -p "$mise_dir" "$python_dir"
chmod 700 "$install_root" "$install_root/artifacts" "$install_root/artifacts/mise" "$install_root/artifacts/mise/$mise_version" "$mise_dir" "$install_root/artifacts/python" "$python_dir"
cp "$test_root/fake-mise" "$mise_dir/mise"
chmod 500 "$mise_dir/mise"
printf 'preserve this unrelated install payload\n' > "$install_root/unrelated-preserved-sentinel"
chmod 600 "$install_root/unrelated-preserved-sentinel"
run_installer() {
  local script=$1
  local log=$2
  local status=0
  (cd "$sandbox" && HOME="$home_dir" /bin/bash "$script") > "$log" 2>&1 || status=$?
  printf '%s' "$status"
}
installer_status=$(run_installer "$installer" "$test_root/install-first.log")
if [[ "$installer_status" -ne 0 ]]; then
  cat "$test_root/install-first.log" >&2
  printf 'generated-workflows: the reviewed installer did not complete its first sandbox installation\n' >&2
  exit 1
fi
hooks_dir="$sandbox/.git/hooks"
for hook in pre-commit pre-push; do
  if [[ ! -f "$hooks_dir/$hook" || -L "$hooks_dir/$hook" ]]; then
    printf 'generated-workflows: the reviewed installer did not publish the %s hook\n' "$hook" >&2
    exit 1
  fi
done
inventory=$(find "$python_dir" -mindepth 2 -maxdepth 2 -type f -name inventory -print -quit)
if [[ -z "$inventory" || ! -f "$inventory" ]]; then
  printf 'generated-workflows: the reviewed installer did not publish the qualified Python inventory\n' >&2
  exit 1
fi
printf 'baseline sandbox installation: PASS\n'
chmod u+w "$inventory"
printf 'corrupt inventory fixture for the recovery case\n' >> "$inventory"
chmod 400 "$inventory"
damaged_inventory_sha=$(sha256_file "$inventory")
installer_status=$(run_installer "$installer" "$test_root/install-repair-python.log")
if [[ "$installer_status" -ne 0 ]]; then
  cat "$test_root/install-repair-python.log" >&2
  printf 'generated-workflows: a corrupt Python generation is not repaired on rerun\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$inventory")" == "$damaged_inventory_sha" ]]; then
  printf 'generated-workflows: the corrupt Python inventory survived the repair\n' >&2
  exit 1
fi
quarantined_generation=0
for quarantined_path in "$python_dir"/.quarantine.*/generation; do
  if [[ -d "$quarantined_path" && -f "$quarantined_path/inventory" && "$(sha256_file "$quarantined_path/inventory")" == "$damaged_inventory_sha" ]]; then
    quarantined_generation=1
  fi
done
if [[ "$quarantined_generation" -ne 1 ]]; then
  cat "$test_root/install-repair-python.log" >&2
  printf 'generated-workflows: the corrupt Python generation was not quarantined as one generation\n' >&2
  exit 1
fi
if [[ "$(/bin/cat "$install_root/unrelated-preserved-sentinel")" != 'preserve this unrelated install payload' ]]; then
  printf 'generated-workflows: the Python repair changed an unrelated install payload\n' >&2
  exit 1
fi
printf 'corrupt Python generation quarantined and rebuilt from the verified pins: PASS\n'
printf 'hold the installer lock\n' > "$mise_dir/hold-lock"
chmod 600 "$mise_dir/hold-lock"
chmod u+w "$inventory"
printf 'corrupt inventory fixture for the abandoned lock case\n' >> "$inventory"
chmod 400 "$inventory"
/bin/rm -f "$mise_dir/mise-running"
(cd "$sandbox" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/install-abandoned.log" 2>&1 &
abandoned_pid=$!
deadline=$((SECONDS + 120))
while [[ ! -f "$mise_dir/mise-running" ]]; do
  if ! kill -0 "$abandoned_pid" 2>/dev/null; then
    cat "$test_root/install-abandoned.log" >&2
    printf 'generated-workflows: the reviewed installer exited before it held the install lock\n' >&2
    exit 1
  fi
  if ((SECONDS >= deadline)); then
    kill -9 "$abandoned_pid" 2>/dev/null || true
    printf 'generated-workflows: the reviewed installer did not reach its locked tool qualification\n' >&2
    exit 1
  fi
  sleep 0.2
done
contended_config="$version_dir/config.toml"
/bin/cat "$contended_config" > "$test_root/contended-config.pristine"
contended_config_sha=$(sha256_file "$contended_config")
chmod u+w "$contended_config"
printf 'contended preflight corruption\n' >> "$contended_config"
install_state_before=$(fingerprint_tree "$install_root")
hooks_state_before=$(fingerprint_tree "$hooks_dir")
contender_status=$(run_installer "$installer" "$test_root/install-contender.log")
if [[ "$contender_status" -ne 75 ]] || ! /usr/bin/grep -F -q 'another trusted checker installation is in progress' "$test_root/install-contender.log"; then
  cat "$test_root/install-contender.log" >&2
  kill -9 "$abandoned_pid" 2>/dev/null || true
  printf 'generated-workflows: a live install lock did not refuse a concurrent installation\n' >&2
  exit 1
fi
install_state_after=$(fingerprint_tree "$install_root")
hooks_state_after=$(fingerprint_tree "$hooks_dir")
if [[ "$install_state_before" != "$install_state_after" || "$hooks_state_before" != "$hooks_state_after" ]]; then
  kill -9 "$abandoned_pid" 2>/dev/null || true
  printf 'generated-workflows: the refused contender mutated the installation or the hooks\n' >&2
  exit 1
fi
mutant_status=$(run_installer "$mutant_installer" "$test_root/install-contender-preflight-first.log")
if [[ "$mutant_status" -eq 75 ]] || ! /usr/bin/grep -F -q 'refusing to replace an existing hook' "$test_root/install-contender-preflight-first.log"; then
  cat "$test_root/install-contender-preflight-first.log" >&2
  kill -9 "$abandoned_pid" 2>/dev/null || true
  printf 'generated-workflows: the contention case does not distinguish lock-first from preflight-first ordering\n' >&2
  exit 1
fi
install_state_after_mutant=$(fingerprint_tree "$install_root")
hooks_state_after_mutant=$(fingerprint_tree "$hooks_dir")
if [[ "$install_state_before" != "$install_state_after_mutant" || "$hooks_state_before" != "$hooks_state_after_mutant" ]]; then
  kill -9 "$abandoned_pid" 2>/dev/null || true
  printf 'generated-workflows: the preflight-first refusal mutated the installation or the hooks\n' >&2
  exit 1
fi
printf 'the install lock precedes the hook preflight and the refusals change nothing: PASS\n'
chmod u+w "$contended_config"
/bin/cat "$test_root/contended-config.pristine" > "$contended_config"
chmod 400 "$contended_config"
if [[ "$(sha256_file "$contended_config")" != "$contended_config_sha" ]]; then
  kill -9 "$abandoned_pid" 2>/dev/null || true
  printf 'generated-workflows: the contended generation could not be restored for recovery\n' >&2
  exit 1
fi
pre_kill_pre_commit_identity=$(file_identity "$hooks_dir/pre-commit")
pre_kill_pre_commit_hash=$(sha256_file "$hooks_dir/pre-commit")
pre_kill_pre_push_identity=$(file_identity "$hooks_dir/pre-push")
pre_kill_pre_push_hash=$(sha256_file "$hooks_dir/pre-push")
kill -9 "$abandoned_pid" 2>/dev/null || true
wait "$abandoned_pid" 2>/dev/null || true
lock_release_deadline=$((SECONDS + 10))
lock_released=0
while (( SECONDS < lock_release_deadline )); do
  if probe_lock_free "$lock_file"; then
    lock_released=1
    break
  fi
  sleep 0.1
done
if [[ "$lock_released" -ne 1 ]]; then
  printf 'generated-workflows: killing only the installer did not release the abandoned install lock\n' >&2
  exit 1
fi
/bin/rm -f "$mise_dir/hold-lock" "$mise_dir/mise-running"
recovery_status=$(run_installer "$installer" "$test_root/install-recovery.log")
if [[ "$recovery_status" -ne 0 ]]; then
  cat "$test_root/install-recovery.log" >&2
  printf 'generated-workflows: an abandoned installation did not recover on the next run\n' >&2
  exit 1
fi
for hook in pre-commit pre-push; do
  if [[ ! -f "$hooks_dir/$hook" || -L "$hooks_dir/$hook" ]]; then
    printf 'generated-workflows: the recovered installation did not publish the %s hook\n' "$hook" >&2
    exit 1
  fi
  if [[ "$(file_mode "$hooks_dir/$hook")" != 700 ]]; then
    printf 'generated-workflows: the recovered %s hook has an unexpected mode\n' "$hook" >&2
    exit 1
  fi
  case "$hook" in
    pre-commit)
      if [[ "$(sha256_file "$hooks_dir/$hook")" != "$pre_kill_pre_commit_hash" ]]; then
        printf 'generated-workflows: the recovered pre-commit hook is not the correctly rendered hook\n' >&2
        exit 1
      fi
      if [[ "$(file_identity "$hooks_dir/$hook")" == "$pre_kill_pre_commit_identity" ]]; then
        printf 'generated-workflows: the recovered pre-commit hook was not republished with a new identity\n' >&2
        exit 1
      fi
      ;;
    pre-push)
      if [[ "$(sha256_file "$hooks_dir/$hook")" != "$pre_kill_pre_push_hash" ]]; then
        printf 'generated-workflows: the recovered pre-push hook is not the correctly rendered hook\n' >&2
        exit 1
      fi
      if [[ "$(file_identity "$hooks_dir/$hook")" == "$pre_kill_pre_push_identity" ]]; then
        printf 'generated-workflows: the recovered pre-push hook was not republished with a new identity\n' >&2
        exit 1
      fi
      ;;
  esac
done
if [[ "$(/bin/cat "$install_root/unrelated-preserved-sentinel")" != 'preserve this unrelated install payload' ]]; then
  printf 'generated-workflows: the recovered installation changed an unrelated install payload\n' >&2
  exit 1
fi
printf 'an abandoned installation is recovered by the next run after killing only the installer: PASS\n'
damaged_mise="$mise_dir/mise"
chmod u+w "$damaged_mise"
printf 'X' | /bin/dd of="$damaged_mise" bs=1 seek=0 conv=notrunc 2> "$test_root/damaged-mise.log"
chmod 500 "$damaged_mise"
damaged_mise_sha=$(sha256_file "$damaged_mise")
if [[ "$damaged_mise_sha" == "$(sha256_file "$test_root/fake-mise")" ]]; then
  printf 'generated-workflows: the Mise corruption fixture did not change the pinned artifact\n' >&2
  exit 1
fi
installer_status=$(run_installer "$installer" "$test_root/install-repair-mise.log")
if [[ "$installer_status" -ne 0 ]]; then
  cat "$test_root/install-repair-mise.log" >&2
  printf 'generated-workflows: a changed Mise artifact is not rebuilt from the verified pins\n' >&2
  exit 1
fi
if /usr/bin/grep -F -q 'refusing to replace an existing hook' "$test_root/install-repair-mise.log"; then
  cat "$test_root/install-repair-mise.log" >&2
  printf 'generated-workflows: a changed Mise artifact still refuses the rerun instead of being repaired\n' >&2
  exit 1
fi
quarantined_mise=0
for quarantined_artifact in "$mise_dir"/.quarantine.*/artifact; do
  if [[ -f "$quarantined_artifact" && "$(sha256_file "$quarantined_artifact")" == "$damaged_mise_sha" ]]; then
    quarantined_mise=1
  fi
done
if [[ "$quarantined_mise" -ne 1 ]]; then
  cat "$test_root/install-repair-mise.log" >&2
  printf 'generated-workflows: a changed Mise artifact was not quarantined for repair under the install lock\n' >&2
  exit 1
fi
if [[ -L "$damaged_mise" || ! -f "$damaged_mise" || ! -x "$damaged_mise" ]]; then
  cat "$test_root/install-repair-mise.log" >&2
  printf 'generated-workflows: the Mise rebuild did not publish an executable replacement\n' >&2
  exit 1
fi
if [[ "$(file_mode "$damaged_mise")" != 500 || "$(sha256_file "$damaged_mise")" != "$(sha256_file "$test_root/fake-mise")" ]]; then
  cat "$test_root/install-repair-mise.log" >&2
  printf 'generated-workflows: the rebuilt Mise artifact does not match the reviewed pinned identity\n' >&2
  exit 1
fi
validation_status=$(run_installer "$installer" "$test_root/install-post-repair-validation.log")
if [[ "$validation_status" -ne 0 ]]; then
  cat "$test_root/install-post-repair-validation.log" >&2
  printf 'generated-workflows: the rebuilt Mise artifact fails post-repair validation\n' >&2
  exit 1
fi
if [[ "$(/bin/cat "$install_root/unrelated-preserved-sentinel")" != 'preserve this unrelated install payload' ]]; then
  printf 'generated-workflows: the Mise repair changed an unrelated install payload\n' >&2
  exit 1
fi
printf 'changed Mise artifact detected, quarantined and rebuilt from the verified pins: PASS\n'
chmod u+w "$damaged_mise"
printf 'Y' | /bin/dd of="$damaged_mise" bs=1 seek=1 conv=notrunc 2> "$test_root/damaged-mise-rebuild.log"
chmod 500 "$damaged_mise"
damaged_rebuild_sha=$(sha256_file "$damaged_mise")
if [[ "$damaged_rebuild_sha" == "$(sha256_file "$test_root/fake-mise")" ]]; then
  printf 'generated-workflows: the Mise rebuild fixture did not change the pinned artifact\n' >&2
  exit 1
fi
printf 'fixture\n' > "$test_root/stub-curl-wrong-bytes"
installer_status=$(run_installer "$installer" "$test_root/install-rebuild-failed.log")
if [[ "$installer_status" -eq 0 ]]; then
  cat "$test_root/install-rebuild-failed.log" >&2
  printf 'generated-workflows: a Mise rebuild with mismatched bytes was reported as successful\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'checksum does not match' "$test_root/install-rebuild-failed.log"; then
  cat "$test_root/install-rebuild-failed.log" >&2
  printf 'generated-workflows: the failed Mise rebuild did not report the reviewed checksum mismatch\n' >&2
  exit 1
fi
if [[ -e "$damaged_mise" || -L "$damaged_mise" ]]; then
  cat "$test_root/install-rebuild-failed.log" >&2
  printf 'generated-workflows: the failed Mise rebuild published an artifact anyway\n' >&2
  exit 1
fi
quarantined_rebuild=0
for quarantined_artifact in "$mise_dir"/.quarantine.*/artifact; do
  if [[ -f "$quarantined_artifact" && "$(sha256_file "$quarantined_artifact")" == "$damaged_rebuild_sha" ]]; then
    quarantined_rebuild=1
  fi
done
if [[ "$quarantined_rebuild" -ne 1 ]]; then
  cat "$test_root/install-rebuild-failed.log" >&2
  printf 'generated-workflows: the Mise artifact damaged before the failed rebuild was not quarantined\n' >&2
  exit 1
fi
if [[ "$(/bin/cat "$install_root/unrelated-preserved-sentinel")" != 'preserve this unrelated install payload' ]]; then
  printf 'generated-workflows: the failed Mise rebuild changed an unrelated install payload\n' >&2
  exit 1
fi
printf 'a Mise rebuild whose bytes break the reviewed checksum publishes nothing: PASS\n'
printf 'installer lock recovery and artifact repair: PASS\n'
