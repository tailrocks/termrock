#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
umask 022
repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
helper=${PYTHON_INVENTORY_UNDER_TEST:-$repo_root/scripts/python-inventory.sh}
test_root=$(mktemp -d "${TMPDIR:-/tmp}/python-inventory-test.XXXXXX")
trap 'find "$test_root" -depth -delete' EXIT
root="$test_root/runtime"
inventory="$test_root/inventory"
mkdir -p "$root/directory" "$root/zz-batch"
printf 'original\n' > "$root/directory/file with spaces"
printf 'backslash\n' > "$root/back\\slash"
ln -s 'directory/file with spaces' "$root/link"
for ((index = 1; index <= 65; index++)); do
  printf 'batch file %03d\n' "$index" > "$root/zz-batch/file-$(printf '%03d' "$index")"
done
/bin/bash "$helper" write "$root" "$inventory"
/bin/bash "$helper" check "$root" "$inventory"
for ((index = 1; index <= 65; index++)); do
  relative="zz-batch/file-$(printf '%03d' "$index")"
  if ! awk -F '\t' -v path="$relative" '$1 == "file" && $4 == path { found = 1 } END { exit !found }' "$inventory"; then
    printf 'inventory omitted %s\n' "$relative" >&2
    exit 1
  fi
done
printf 'positive inventory: PASS\n'
reject() {
  if /bin/bash "$helper" check "$root" "$inventory" > "$test_root/output" 2>&1; then
    printf 'inventory accepted %s\n' "$1" >&2
    exit 1
  fi
  printf '%s: REJECTED\n' "$1"
}
reject_write() {
  if /bin/bash "$helper" write "$root" "$test_root/rejected-inventory" > "$test_root/output" 2>&1; then
    printf 'inventory write accepted %s\n' "$1" >&2
    exit 1
  fi
  printf '%s write: REJECTED\n' "$1"
}
printf 'changed batch-tail bytes\n' > "$root/zz-batch/file-065"
reject 'changed 65th batch-tail file bytes'
printf 'batch file 065\n' > "$root/zz-batch/file-065"
printf 'changed\n' > "$root/directory/file with spaces"
reject 'changed bytes'
printf 'original\n' > "$root/directory/file with spaces"
chmod 600 "$root/directory/file with spaces"
reject 'changed file mode'
chmod 644 "$root/directory/file with spaces"
if [[ "$(id -u)" -ne 0 ]]; then
  chmod 000 "$root/directory/file with spaces"
  if /bin/bash "$helper" write "$root" "$test_root/unreadable-inventory" > "$test_root/output" 2>&1; then
    printf 'inventory accepted an unreadable file\n' >&2
    exit 1
  fi
  printf 'unreadable file: REJECTED\n'
  chmod 644 "$root/directory/file with spaces"
fi
chmod 700 "$root/directory"
reject 'changed directory mode'
chmod 755 "$root/directory"
mkfifo "$root/pipe"
reject_write 'special file'
reject 'special file'
rm "$root/pipe"
rm "$root/link"
ln -s 'back\slash' "$root/link"
reject 'changed link target'
rm "$root/link"
ln -s 'directory/file with spaces' "$root/link"
printf extra > "$root/extra"
reject 'added file'
rm "$root/extra"
mv "$root/back\\slash" "$test_root/saved"
reject 'removed file'
mv "$test_root/saved" "$root/back\\slash"
printf x > "$root/$(printf 'bad\tname')"
reject_write 'control character path'
reject 'control character path'
rm "$root/$(printf 'bad\tname')"
ln -s "$(printf 'bad\ttarget')" "$root/bad-link"
reject_write 'control character link target'
reject 'control character link target'
rm "$root/bad-link"
/bin/bash "$helper" check "$root" "$inventory"
printf 'restored inventory: PASS\n'
