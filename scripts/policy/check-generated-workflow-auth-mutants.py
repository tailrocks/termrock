#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parent.parent
checker = root / "scripts/check-generated-workflows.sh"
fixture = root / "tests/check-generated-workflow-auth.py"
source = checker.read_text()
expected_checker = "b7fb4264b6b863073f62a56e941590dd11d6f051f586d5e6f2cb5139bec54f4e"
if hashlib.sha256(checker.read_bytes()).hexdigest() != expected_checker:
    raise SystemExit("mutant driver requires the pinned independently reviewed checker")

def replace_once(text, old, new):
    if text.count(old) != 1:
        raise SystemExit("mutant target is missing or ambiguous")
    return text.replace(old, new, 1)

def replace_metadata_once(old, new):
    marker = "validate_trusted_git_source_trees() {"
    if source.count(marker) != 1:
        raise SystemExit("metadata mutant scope is missing or ambiguous")
    prefix, metadata = source.split(marker, 1)
    return prefix + marker + replace_once(metadata, old, new)

def make_mutant(name):
    if name == "setup-auth":
        return replace_once(source, "    hydration_environment+=(-S 'GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}')", "    :")
    if name == "metadata-auth":
        return replace_metadata_once('command[2:2] = ["--include", "--variable", "%GITHUB_COM_TOKEN", "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"]', 'command[2:2] = ["--include"]')
    if name == "metadata-argv":
        return replace_metadata_once('"--variable", "%GITHUB_COM_TOKEN", "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"', '"--variable", token, "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"')
    if name == "metadata-dump-header":
        return replace_metadata_once('command[2:2] = ["--include", "--variable", "%GITHUB_COM_TOKEN", "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"]', 'command[2:2] = ["--dump-header", str(header_path), "--variable", "%GITHUB_COM_TOKEN", "--expand-header", "Authorization: Bearer {{GITHUB_COM_TOKEN}}"]')
    if name == "metadata-token-only":
        result = replace_once(source, 'if [[ "${1-}" == prepare-cache ]]; then', 'if [[ "${1-}" == prepare-cache || "${1-}" == pre-commit || "${1-}" == pre-push ]]; then')
        result = replace_once(result, 'if [[ "$installed_checker" -eq 1 && "${prepare_cache_mode:-0}" -eq 1 && -n "$trusted_setup_token" ]]; then', 'if [[ -n "$trusted_setup_token" ]]; then')
        old = "    safe_path=\"${mise_command%/*}:${git_command%/*}:$cargo_home/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n    if [[ \"$warm_tool_cache\" != true ]]; then\n      validate_trusted_git_source_trees \"$trusted_seed/Cargo.lock\" \"$home_dir\" \"$runtime_tmp\" \"$safe_path\" || exit $?\n    fi"
        new = old.replace('"$warm_tool_cache" != true', '"$warm_tool_cache" != true || -n "$trusted_setup_token"')
        return replace_once(result, old, new)
    if name in ("hook-pre-commit", "hook-pre-push"):
        result = replace_once(source, 'if [[ "${1-}" == prepare-cache ]]; then', 'if [[ "${1-}" == prepare-cache || "${1-}" == pre-commit || "${1-}" == pre-push ]]; then')
        old = '"${prepare_cache_mode:-0}" -eq 1 && ( "$kind" == trusted || "${trusted_snapshot_auth:-0}" -eq 1 )'
        return replace_once(result, old, '( "$kind" == trusted || "$kind" == snapshot )')
    if name == "wrong-tree":
        old = '''      printf 'generated-workflows: approved seed tree does not match the reviewed identity\\n' >&2
      exit 1'''
        return replace_once(source, old, '      resolved_seed_tree=$trusted_seed_tree')
    if name in ("response-header", "response-body"):
        return replace_once(source, 'if token.encode("ascii") in payload:', 'if False:')
    if name == "response-redirect":
        result = replace_once(source, 'if status in redirect_statuses or locations:', 'if False:')
        return result.replace('if status != 200:', 'if status not in (200, 302):', 1)
    if name == "response-identity":
        return replace_once(source, 'if not isinstance(document, dict) or document.get("sha") != oid:', 'if not isinstance(document, dict):')
    if name == "argv":
        return replace_once(source, '"--variable", "%GITHUB_COM_TOKEN"', '"--variable", token')
    raise SystemExit("unknown mutant")

cases = {
    "setup-auth": ("setup", "authenticated fixed-host boundary failed"),
    "metadata-auth": ("metadata-auth", "authenticated fixed-host boundary failed"),
    "metadata-argv": ("metadata-auth", "authenticated fixed-host boundary failed"),
    "metadata-dump-header": ("metadata-confinement", "credential-bearing metadata header was written to a file during curl"),
    "metadata-token-only": ("hook-pre-commit", "ordinary checker received or exposed the credential: pre-commit"),
    "hook-pre-commit": ("hook-pre-commit", "ordinary checker received or exposed the credential: pre-commit"),
    "hook-pre-push": ("hook-pre-push", "ordinary checker received or exposed the credential: pre-push"),
    "wrong-tree": ("wrong-tree", "wrong seed tree was not rejected before authenticated HTTP"),
    "response-header": ("response-header", "authenticated response rejection failed: header"),
    "response-body": ("response-body", "authenticated response rejection failed: body"),
    "response-redirect": ("response-redirect", "authenticated response rejection failed: redirect"),
    "response-identity": ("response-identity", "authenticated response rejection failed: identity"),
    "argv": ("setup", "authenticated fixed-host boundary failed"),
}
if sys.argv[1:] == ["--list"]:
    for name, (case, _) in cases.items():
        print(name, case)
    raise SystemExit(0)
if len(sys.argv) != 2 or sys.argv[1] not in cases:
    raise SystemExit("usage: check-generated-workflow-auth-mutants.py MUTANT | --list")
name = sys.argv[1]
case, expected_failure = cases[name]
with tempfile.TemporaryDirectory(prefix="guard-auth-mutant.") as directory:
    mutant = Path(directory) / "checker.sh"
    mutant.write_text(make_mutant(name))
    mutant.chmod(0o700)
    digest = hashlib.sha256(mutant.read_bytes()).hexdigest()
    environment = dict(os.environ, BASELINE_GUARD_TEST_CHECKER=str(mutant), BASELINE_GUARD_TEST_MUTANT_NAME=name)
    result = subprocess.run([sys.executable, str(fixture), "--case", case], env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode == 0 or expected_failure not in result.stderr:
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
        raise SystemExit("mutant did not fail at the required public assertion: " + name)
    if name in {"metadata-token-only", "hook-pre-commit", "hook-pre-push"}:
        mode = "pre-push" if name == "hook-pre-push" else "pre-commit"
        fixed_witness = "BASELINE_AUTH_FIXED_HOOK_COMPLETE=" + mode
        assertion_witness = "BASELINE_AUTH_MUTANT_ASSERTION=" + name + ":ordinary-hook-credential-isolation"
        if fixed_witness not in result.stdout or assertion_witness not in result.stdout:
            print(result.stdout, end="")
            print(result.stderr, end="", file=sys.stderr)
            raise SystemExit("hook mutant child exited without the fixed and named causal witnesses: " + name)
    print(result.stdout.replace("GuardDummy" + "Credential_47291", "[dummy credential redacted]"), end="")
    print(result.stderr.replace("GuardDummy" + "Credential_47291", "[dummy credential redacted]"), end="", file=sys.stderr)
    print("mutant", name, "checker_sha256", digest, "detected by", case)
