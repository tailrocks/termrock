#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import difflib
import json
import os
from pathlib import Path
import re
import signal
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib

root = Path(__file__).resolve().parent.parent
artifacts = os.environ.get("BASELINE_GUARD_TEST_ARTIFACT_ROOT")
if not artifacts:
    raise SystemExit("set BASELINE_GUARD_TEST_ARTIFACT_ROOT to previously qualified artifacts")
qualified_cache_source = os.environ.get("BASELINE_GUARD_TEST_QUALIFIED_CACHE")
qualified_cache_root = Path("/Users/donbeave/Projects/jackin-project/wt/baseline178-qualification-v4/.git/jackin-generated-workflow-guard")
qualified_cache_generation = qualified_cache_root / "cache/tools/9a0ed85e2b52adbaf29bddccde72f00f702e0cc7f33ce352e3dd806cd231a3ee"
qualified_source_manifest_sha256 = "e10ba791a6eee5777d094a11f591d8a1fb4b9d0b8754d9b571aac7d21ad1015e"
qualified_payload_sha256 = "6a0ba692733d2590cb07d65de6c45a9a7bceffb1e4004062961418130b24cbbb"
qualified_checker_sha256 = "b7fb4264b6b863073f62a56e941590dd11d6f051f586d5e6f2cb5139bec54f4e"
qualified_manifest_key = "9a0ed85e2b52adbaf29bddccde72f00f702e0cc7f33ce352e3dd806cd231a3ee"
qualified_fixture_head = "1d7f63dbb13ecb2bf3e3295e7a296dc2080a62b1"
qualified_seed_commit = "f4d9999ace7d6deb4294cd89c00afcc80e748d49"
qualified_seed_tree = "95406daef6ba234fde62fc47b933fbb8bdfce320"
qualified_cache_receipts = {
    "baseline178-v7-authenticated-real-setup-postconditions-v1.json": "e1029452c0b1e79f25502e0425277e23e94f9a14c3ec98174f37a71b692a2ef0",
    "baseline178-v7-authenticated-real-setup-receipt-v1/command.json": "a13019f5235c8ba4898b4ee35503f72111e1ade05ba4da04993e8379f5d2df2f",
    "baseline178-v7-authenticated-real-setup-receipt-v1/before.json": "f7c39624ffde1c4896e81feb2b48dc498b25a83908d01e59e39da0a74cbfbc14",
    "baseline178-v7-authenticated-real-setup-receipt-v1/result.json": "08b4fea0c4e43fe7c46a24e46db238697aff8d6242397a047dd8af5364d0cf8d",
    "baseline178-v7-authenticated-real-setup-receipt-v1/run.log": "6bca70ddf78e4048d20a13bccd22f222c7104d15a656f6bd7fa3b114ccae5e83",
    "baseline178-v7-real-installed-precommit-receipt-v1/result.json": "0fd390b96424b64394e2a98c5df2ec02ff6c269f1aafd049c1ce4ea0073be52c",
    "baseline178-v7-real-installed-precommit-receipt-v1/run.log": "84108b57df62f312b2ab37bf07c6c5a4bff651e7c315b2fe312e11bcce21f9e3",
    "baseline178-v7-real-installed-precommit-receipt-v1/command.json": "a4144dbcbe8467ef6981bea16fe7a582b8624778c7507373de90570a6594d75f",
    "baseline178-v7-real-installed-precommit-receipt-v1/before.json": "f7c39624ffde1c4896e81feb2b48dc498b25a83908d01e59e39da0a74cbfbc14",
}
warm_cases = {"hook-pre-commit", "hook-pre-push"}
cases = {"all", "setup", "metadata-auth", "metadata-confinement", "metadata-no-token", "metadata-invalid-token", "metadata-uninstalled", "metadata-response", "metadata-redirect", "metadata-status", "hook-pre-commit", "hook-pre-push", "wrong-tree", "response-header", "response-body", "response-redirect", "response-identity"}
if not sys.argv[1:]:
    selected_case = "all"
elif sys.argv[1:] == ["--setup-only"]:
    selected_case = "setup"
elif len(sys.argv) == 3 and sys.argv[1] == "--case" and sys.argv[2] in cases:
    selected_case = sys.argv[2]
else:
    raise SystemExit("usage: check-generated-workflow-auth.py [--setup-only | --case CASE]")
if selected_case in warm_cases | {"all"}:
    if not qualified_cache_source:
        raise SystemExit("set BASELINE_GUARD_TEST_QUALIFIED_CACHE to the pinned real setup generation for hook cases")
    if Path(qualified_cache_source).resolve(strict=True) != qualified_cache_generation.resolve(strict=True):
        raise SystemExit("hook fixture source does not match the pinned qualified cache generation")
    if Path(artifacts).resolve(strict=True) != (qualified_cache_root / "artifacts").resolve(strict=True):
        raise SystemExit("hook fixture artifacts do not match the pinned real setup generation")
def selected(case):
    return selected_case in ("all", case)
case_completion_prefix = "BASELINE_AUTH_CASE_COMPLETE="
case_assertions = set()

def case_completion_witnesses(output):
    return [line[len(case_completion_prefix):] for line in output.splitlines() if line.startswith(case_completion_prefix)]

def mark_case_assertions(case):
    if case in case_assertions:
        raise SystemExit("selected case repeated its public completion oracle: " + case)
    case_assertions.add(case)

def finish_case(case):
    if case == "all":
        expected = {"all", "hook-pre-commit", "hook-pre-push", "wrong-tree", "response-header", "response-body", "response-redirect", "response-identity"}
    else:
        expected = {case}
    if case_assertions != expected:
        raise SystemExit("selected case did not reach its exact public completion oracle set: " + case)
    print(case_completion_prefix + case, flush=True)
    raise SystemExit(0)

all_cold_cases = (
    "metadata-confinement",
    "metadata-no-token",
    "metadata-invalid-token",
    "metadata-uninstalled",
    "metadata-response",
    "metadata-redirect",
    "metadata-status",
    "setup",
)
all_cold_witnesses = []
if selected_case == "all":
    fixture_environment = {name: os.environ[name] for name in ("PATH", "HOME", "LANG", "LC_ALL") if name in os.environ}
    fixture_environment["BASELINE_GUARD_TEST_ARTIFACT_ROOT"] = artifacts
    for name in ("BASELINE_GUARD_TEST_CHECKER", "BASELINE_GUARD_AUTH_EVIDENCE_DIR", "BASELINE_GUARD_TEST_QUALIFIED_CACHE"):
        if name in os.environ:
            fixture_environment[name] = os.environ[name]
    for case in all_cold_cases:
        child = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--case", case], check=False, env=fixture_environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        witnesses = case_completion_witnesses(child.stdout)
        if child.returncode != 0 or witnesses != [case]:
            print(child.stdout, end="")
            print(child.stderr, end="", file=sys.stderr)
            raise SystemExit("all-cases child did not complete its exact public witness: " + case)
        all_cold_witnesses.append(witnesses[0])
        print(child.stdout, end="")
        print(child.stderr, end="", file=sys.stderr)
    if len(all_cold_witnesses) != len(all_cold_cases) or set(all_cold_witnesses) != set(all_cold_cases):
        raise SystemExit("all-cases cold child witness set is incomplete or duplicated")
dummy = "GuardDummy" + "Credential_47291"
clean = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": "/tmp", "LANG": "C", "LC_ALL": "C", "GIT_NO_LAZY_FETCH": "1", "GIT_NO_REPLACE_OBJECTS": "1"}
evidence_base = os.environ.get("BASELINE_GUARD_AUTH_EVIDENCE_DIR")
evidence = None
if evidence_base:
    Path(evidence_base).mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="auth-fixture.", dir=evidence_base))
    print("evidence:", evidence, flush=True)

def run(args, **kwargs):
    return subprocess.run(args, check=True, env=clean, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)

with tempfile.TemporaryDirectory(prefix="guard-auth-test.") as temporary:
    base = Path(temporary)
    repo = base / "repository with spaces"
    fake = base / "curl-fixture"
    events = base / "events"
    scenario = base / "scenario"
    source = str(root)
    reviewed_lock = tomllib.loads((root / "scripts/trusted-seed-cargo.lock").read_text(encoding="utf-8"))
    metadata_commits = {}
    for package in reviewed_lock["package"]:
        source_value = package.get("source")
        if isinstance(source_value, str) and source_value.startswith("git+"):
            match = re.fullmatch(r"git\+https://github\.com/tailrocks/(termpane|tuiscotti)\?rev=([0-9a-f]{40})#([0-9a-f]{40})", source_value)
            if match is None:
                raise SystemExit("reviewed lock contains an unsupported fixture Git source")
            metadata_commits[match.group(1)] = match.group(2)
    if set(metadata_commits) != {"termpane", "tuiscotti"}:
        raise SystemExit("reviewed lock fixture Git source set is incomplete")
    metadata_trees = {name: hashlib.sha1((name + ":" + commit).encode(), usedforsecurity=False).hexdigest() for name, commit in metadata_commits.items()}

    def expected_metadata_events(authentication):
        expected = []
        for name in sorted(metadata_commits):
            commit = metadata_commits[name]
            expected.append(f"{authentication} metadata commit {name} {commit}")
            expected.append(f"{authentication} metadata tree {name} {metadata_trees[name]}")
        return expected

    def is_metadata_request(line):
        return any(
            line.startswith(f"{authentication} metadata {kind} ")
            for authentication in ("authenticated", "anonymous")
            for kind in ("commit", "tree")
        )

    def observed_metadata_events(log):
        return [line for line in log.splitlines() if is_metadata_request(line)]

    def matches_metadata_events(log, authentication, count=None):
        expected = expected_metadata_events(authentication)
        if count is not None:
            expected = expected[:count]
        return observed_metadata_events(log) == expected

    def request_events(log):
        request_kinds = {"authenticated blob", "anonymous blob", "authenticated other", "anonymous other"}
        return [line for line in log.splitlines() if line in request_kinds or is_metadata_request(line)]

    def matches_request_boundary(log, authentication, metadata_count, include_bootstrap=False):
        expected = [authentication + " blob", *expected_metadata_events(authentication)[:metadata_count]]
        if include_bootstrap:
            expected.append("anonymous other")
        oid_prefix = authentication + "-oid "
        observed_oids = [line for line in log.splitlines() if line.startswith(("authenticated-oid ", "anonymous-oid "))]
        return request_events(log) == expected and observed_oids == [oid_prefix + omitted]

    def snapshot_public_state(repository, common_directory):
        working_tree = []
        def collect(directory):
            for path in sorted(directory.iterdir()):
                if path == repository / ".git":
                    continue
                relative = path.relative_to(repository)
                info = path.lstat()
                mode = info.st_mode & 0o7777
                if path.is_symlink():
                    value = ("link", mode, os.readlink(path))
                elif path.is_dir():
                    value = ("directory", mode)
                    collect(path)
                elif path.is_file():
                    value = ("file", mode, hashlib.sha256(path.read_bytes()).hexdigest())
                else:
                    value = ("other", mode)
                working_tree.append((relative.as_posix(), value))
        collect(repository)
        working_tree.sort()
        index_path = common_directory / "index"
        index_state = hashlib.sha256(index_path.read_bytes()).hexdigest()
        head_file_state = hashlib.sha256((common_directory / "HEAD").read_bytes()).hexdigest()
        head_state = run(["/usr/bin/git", "-C", str(repository), "rev-parse", "HEAD"]).stdout.decode().strip()
        refs_state = run(["/usr/bin/git", "-C", str(repository), "for-each-ref", "--format=%(refname) %(objectname)"]).stdout
        return working_tree, index_state, head_file_state, head_state, refs_state

    def snapshot_hook_state(repository):
        raw = run(["/usr/bin/git", "-C", str(repository), "rev-parse", "--path-format=absolute", "--git-path", "hooks"]).stdout.decode().strip()
        directory = Path(raw)
        if not directory.is_absolute():
            directory = repository / directory
        if directory.is_symlink():
            return ((".", "link", os.readlink(directory), directory.lstat().st_mode & 0o7777),)
        if not directory.exists():
            return ()
        state = []
        def collect(current):
            for path in sorted(current.iterdir()):
                relative = path.relative_to(directory).as_posix()
                info = path.lstat()
                mode = info.st_mode & 0o7777
                if path.is_symlink():
                    value = ("link", mode, os.readlink(path))
                elif path.is_dir():
                    value = ("directory", mode)
                    collect(path)
                elif path.is_file():
                    value = ("file", mode, hashlib.sha256(path.read_bytes()).hexdigest())
                else:
                    value = ("other", mode)
                state.append((relative, value))
        collect(directory)
        return tuple(sorted(state))

    def parse_cache_manifest(path):
        raw = path.read_bytes()
        fields = {}
        lines = raw.splitlines(keepends=True)
        if not lines or any(not line.endswith(b"\n") for line in lines):
            raise SystemExit("qualified tool cache manifest has invalid line endings")
        for line in lines:
            body = line[:-1]
            if body.count(b"=") != 1:
                raise SystemExit("qualified tool cache manifest contains an ambiguous field")
            key_bytes, value_bytes = body.split(b"=", 1)
            try:
                key = key_bytes.decode("ascii")
                value = value_bytes.decode("ascii")
            except UnicodeDecodeError as error:
                raise SystemExit("qualified tool cache manifest is not ASCII") from error
            if not key or key in fields:
                raise SystemExit("qualified tool cache manifest contains a duplicate field")
            fields[key] = value
        expected_fields = {
            "key", "platform", "velnor", "rust", "jactionlint", "shellcheck", "seed_commit",
            "trusted_lock_sha256", "trusted_seed_cargo_policy_sha256", "checker_version", "checker_sha256",
            "config_sha256", "mise_version", "mise_sha256", "python_version", "python_artifact_sha256",
            "python_lock_sha256", "python_mise_sha256", "inventory_helper_sha256", "payload_sha256",
        }
        if set(fields) != expected_fields:
            raise SystemExit("qualified tool cache manifest has an unexpected field set")
        return raw, fields

    def filesystem_snapshot(path):
        root_info = path.lstat()
        entries = []
        def visit(current):
            info = current.lstat()
            relative = "." if current == path else current.relative_to(path).as_posix()
            mode = stat.S_IMODE(info.st_mode)
            if stat.S_ISLNK(info.st_mode):
                value = ("link", mode, os.readlink(current))
            elif stat.S_ISDIR(info.st_mode):
                value = ("directory", mode)
                for child in sorted(current.iterdir(), key=lambda item: os.fsencode(item.name)):
                    visit(child)
            elif stat.S_ISREG(info.st_mode):
                value = ("file", mode, hashlib.sha256(current.read_bytes()).hexdigest())
            else:
                value = ("special", mode)
            entries.append((relative, info.st_uid, info.st_nlink, value))
        visit(path)
        return tuple(sorted(entries))

    def payload_inventory(path):
        digest = hashlib.sha256()
        entries = []
        def fail(error):
            raise error
        for directory, names, filenames in os.walk(path, followlinks=False, onerror=fail):
            current = Path(directory)
            entries.extend(current / name for name in names + filenames)
        for entry in sorted(entries, key=lambda item: os.fsencode(item.relative_to(path))):
            relative = os.fsencode(entry.relative_to(path))
            info = entry.lstat()
            mode = stat.S_IMODE(info.st_mode)
            if stat.S_ISLNK(info.st_mode):
                kind = b"link"
                content = os.fsencode(os.readlink(entry))
            elif stat.S_ISREG(info.st_mode):
                kind = b"file"
                content = entry.read_bytes()
            elif stat.S_ISDIR(info.st_mode):
                kind = b"directory"
                content = b""
            else:
                raise SystemExit("qualified tool cache contains a special file")
            for field in (relative, kind, str(mode).encode("ascii"), content):
                digest.update(len(field).to_bytes(8, "big"))
                digest.update(field)
        return digest.hexdigest()

    def validate_cache_tree(generation, expected_payload):
        owner = os.getuid()
        for directory in (generation, generation.parent, generation.parent.parent, generation.parent.parent.parent):
            info = directory.lstat()
            if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode) or info.st_uid != owner or stat.S_IMODE(info.st_mode) & 0o077:
                raise SystemExit("qualified tool cache directory chain is unsafe")
        manifest = generation / "manifest"
        tool_state = generation / "tool-state"
        manifest_info = manifest.lstat()
        state_info = tool_state.lstat()
        if not stat.S_ISREG(manifest_info.st_mode) or manifest_info.st_uid != owner or manifest_info.st_mode & 0o222 or stat.S_IMODE(manifest_info.st_mode) & 0o077 or not stat.S_ISDIR(state_info.st_mode) or stat.S_ISLNK(state_info.st_mode) or state_info.st_uid != owner or state_info.st_mode & 0o222:
            raise SystemExit("qualified tool cache manifest or payload is unsafe")
        root = tool_state.resolve(strict=True)
        for directory, names, filenames in os.walk(tool_state, followlinks=False):
            current = Path(directory)
            for name in names + filenames:
                path = current / name
                info = path.lstat()
                if info.st_uid != owner:
                    raise SystemExit("qualified tool cache payload has an unexpected owner")
                if stat.S_ISLNK(info.st_mode):
                    link_text = os.readlink(path)
                    if os.path.isabs(link_text):
                        raise SystemExit("qualified tool cache contains an unnormalized absolute symlink")
                    try:
                        target = path.resolve(strict=True)
                    except OSError as error:
                        raise SystemExit("qualified tool cache contains a dangling symlink") from error
                    if not target.is_relative_to(root):
                        raise SystemExit("qualified tool cache contains an escaping symlink")
                elif stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode):
                    if info.st_mode & 0o222:
                        raise SystemExit("qualified tool cache payload is writable")
                else:
                    raise SystemExit("qualified tool cache contains a special file")
        actual_payload = payload_inventory(tool_state)
        if actual_payload != expected_payload:
            raise SystemExit("qualified tool cache payload inventory does not match its reviewed identity")
        return actual_payload

    def verify_qualified_cache_receipts():
        receipt_root = Path("/Users/donbeave/Projects/jackin-project/handoff/state-2026-11-05/current")
        for relative, expected in qualified_cache_receipts.items():
            path = receipt_root / relative
            if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                raise SystemExit("qualified cache receipt changed or is unavailable")
        postconditions = json.loads((receipt_root / "baseline178-v7-authenticated-real-setup-postconditions-v1.json").read_text())
        setup_result = json.loads((receipt_root / "baseline178-v7-authenticated-real-setup-receipt-v1/result.json").read_text())
        hook_result = json.loads((receipt_root / "baseline178-v7-real-installed-precommit-receipt-v1/result.json").read_text())
        expected_version = str(qualified_cache_root / "versions" / qualified_checker_sha256)
        if postconditions.get("checker_sha256") != qualified_checker_sha256 or postconditions.get("installed_versions") != [expected_version] or setup_result.get("exit") != 0 or setup_result.get("source_unchanged") is not True or hook_result.get("exit") != 0 or hook_result.get("source_unchanged") is not True:
            raise SystemExit("qualified cache receipts do not prove successful unchanged-source setup and installed pre-commit")
        cache_records = postconditions.get("qualified_cache_manifests")
        if not isinstance(cache_records, list) or len(cache_records) != 1:
            raise SystemExit("qualified cache receipt has an ambiguous cache-generation set")
        record = cache_records[0]
        fields = record.get("fields")
        if record.get("manifest_sha256") != qualified_source_manifest_sha256 or record.get("path") != str(qualified_cache_generation) or not isinstance(fields, dict):
            raise SystemExit("qualified cache receipt does not bind the selected source generation")
        expected = {
            "key": qualified_manifest_key,
            "checker_sha256": qualified_checker_sha256,
            "platform": "macos-arm64",
            "velnor": "0.1.78",
            "rust": "1.99.1",
            "jactionlint": "2.4.0",
            "shellcheck": "0.11.0",
            "seed_commit": "f4d9999ace7d6deb4294cd89c00afcc80e748d49",
        }
        if any(fields.get(name) != value for name, value in expected.items()):
            raise SystemExit("qualified cache receipt has unexpected production tool or seed identities")

    def cache_key(fields, checker_sha256):
        names = (
            "platform", "velnor", "rust", "jactionlint", "shellcheck", "seed_commit", "trusted_lock_sha256",
            "mise_version", "mise_sha256", "python_version", "python_artifact_sha256", "python_lock_sha256",
            "python_mise_sha256", "config_sha256", "inventory_helper_sha256", "trusted_seed_cargo_policy_sha256",
        )
        try:
            values = [checker_sha256, "2", *(fields[name] for name in names)]
        except KeyError as error:
            raise SystemExit("qualified tool cache identity is incomplete") from error
        return hashlib.sha256("\n".join(values).encode("ascii")).hexdigest()

    def ensure_fixture_private_directory(path):
        if path.is_symlink():
            raise SystemExit("derived tool cache parent contains a symlink")
        if not path.exists():
            path.mkdir(mode=0o700)
        info = path.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
            raise SystemExit("derived tool cache parent is not private and owned")

    def rebind_qualified_cache(source_generation, checker_sha256):
        source_manifest = source_generation / "manifest"
        source_raw, source_fields = parse_cache_manifest(source_manifest)
        if hashlib.sha256(source_raw).hexdigest() != qualified_source_manifest_sha256:
            raise SystemExit("qualified cache source manifest does not match the real setup receipt")
        if source_fields["key"] != qualified_manifest_key or source_fields["checker_sha256"] != qualified_checker_sha256 or source_fields["payload_sha256"] != qualified_payload_sha256:
            raise SystemExit("qualified cache manifest does not match its pinned production identity")
        if cache_key(source_fields, qualified_checker_sha256) != qualified_manifest_key:
            raise SystemExit("qualified cache key does not match the schema-2 production tuple")
        source_snapshot = filesystem_snapshot(source_generation)
        validate_cache_tree(source_generation, qualified_payload_sha256)
        target_key = cache_key(source_fields, checker_sha256)
        target_generation = install_root / "cache/tools" / target_key
        if target_generation.exists() or target_generation.is_symlink():
            raise SystemExit("derived test cache generation already exists")
        ensure_fixture_private_directory(install_root)
        ensure_fixture_private_directory(install_root / "cache")
        ensure_fixture_private_directory(install_root / "cache/tools")
        shutil.copytree(source_generation, target_generation, symlinks=True, copy_function=shutil.copy2)
        if payload_inventory(target_generation / "tool-state") != qualified_payload_sha256:
            raise SystemExit("copied qualified cache payload changed during fixture preparation")
        for source_path in source_generation.rglob("*"):
            relative = source_path.relative_to(source_generation)
            target_path = target_generation / relative
            if source_path.is_file() and not source_path.is_symlink():
                source_info = source_path.stat()
                target_info = target_path.stat()
                if (source_info.st_dev, source_info.st_ino) == (target_info.st_dev, target_info.st_ino) or target_info.st_nlink != 1:
                    raise SystemExit("derived cache shares a hard-linked payload file with its qualified source")
        original_mode = stat.S_IMODE((target_generation / "manifest").lstat().st_mode)
        lines = source_raw.splitlines(keepends=True)
        rebound_lines = []
        changed = set()
        for line in lines:
            key, separator, _ = line[:-1].partition(b"=")
            if not separator:
                raise SystemExit("qualified tool cache manifest contains a malformed field")
            if key == b"key":
                rebound_lines.append(b"key=" + target_key.encode("ascii") + b"\n")
                changed.add("key")
            elif key == b"checker_sha256":
                rebound_lines.append(b"checker_sha256=" + checker_sha256.encode("ascii") + b"\n")
                changed.add("checker_sha256")
            else:
                rebound_lines.append(line)
        if changed != {"key", "checker_sha256"}:
            raise SystemExit("qualified tool cache manifest does not have unique rebindable identity fields")
        manifest_path = target_generation / "manifest"
        os.chmod(manifest_path, original_mode | 0o200)
        manifest_path.write_bytes(b"".join(rebound_lines))
        os.chmod(manifest_path, original_mode)
        _, rebound_fields = parse_cache_manifest(manifest_path)
        expected_fields = dict(source_fields, key=target_key, checker_sha256=checker_sha256)
        if rebound_fields != expected_fields or cache_key(rebound_fields, checker_sha256) != target_key:
            raise SystemExit("derived tool cache manifest changed fields outside the approved boundary binding")
        validate_cache_tree(target_generation, qualified_payload_sha256)
        if filesystem_snapshot(source_generation) != source_snapshot:
            raise SystemExit("qualified source tool cache changed while making its private fixture copy")
        return target_generation, target_key, source_snapshot

    if selected_case in warm_cases | {"all"}:
        verify_qualified_cache_receipts()

    run(["/usr/bin/git", "clone", "--quiet", "--depth=1", "file://" + source, str(repo)])
    run(["/usr/bin/git", "-C", str(repo), "remote", "set-url", "origin", "https://attacker.invalid/repository"])
    (repo / "scripts").mkdir(exist_ok=True)
    for name in ("check-generated-workflows.sh", "install-generated-workflow-guard.sh", "generated-workflow-guard-config.toml", "python-inventory.sh", "python-mise.toml", "python-mise.macos-arm64.lock.toml", "python-mise.linux-x64.lock.toml", "trusted-seed.commit", "trusted-seed-cargo.lock", "trusted-seed-cargo-policy.json", "trusted-seed-metadata.pack", "trusted-seed-metadata.objects"):
        shutil.copy2(root / "scripts" / name, repo / "scripts" / name)
    seed_commit_bytes = (repo / "scripts/trusted-seed.commit").read_bytes()
    seed_commit_lines = seed_commit_bytes.splitlines()
    if not seed_commit_lines or re.fullmatch(rb"tree ([0-9a-f]{40})", seed_commit_lines[0]) is None:
        raise SystemExit("fixture trusted seed commit has no valid tree identity")
    seed_tree = seed_commit_lines[0][5:].decode("ascii")
    installer_source = (repo / "scripts/install-generated-workflow-guard.sh").read_text(encoding="utf-8")
    reviewed_commit_match = re.search(r"^reviewed_seed_commit=([0-9a-f]{40})$", installer_source, re.M)
    reviewed_tree_match = re.search(r"^reviewed_seed_tree=([0-9a-f]{40})$", installer_source, re.M)
    actual_seed_commit = run(["/usr/bin/git", "hash-object", "-t", "commit", "--stdin"], input=seed_commit_bytes).stdout.decode().strip()
    if reviewed_commit_match is None or reviewed_tree_match is None or actual_seed_commit != reviewed_commit_match.group(1) or seed_tree != reviewed_tree_match.group(1):
        raise SystemExit("fixture seed commit/tree does not match the installed review identity")
    if actual_seed_commit != qualified_seed_commit or seed_tree != qualified_seed_tree:
        raise SystemExit("fixture seed commit/tree does not match the pinned authenticated seed identity")
    seed_commit = actual_seed_commit
    fake.write_text('''#!/usr/bin/env python3
import base64, json, os, subprocess, sys
from pathlib import Path
from urllib.parse import urlsplit
token = os.environ.pop("GITHUB_COM_TOKEN", "")
args = sys.argv[1:]
source = ''' + repr(source) + '''
events = Path(''' + repr(str(events)) + ''')
scenario = Path(''' + repr(str(scenario)) + ''')
metadata_commits = ''' + repr(metadata_commits) + '''
metadata_trees = ''' + repr(metadata_trees) + '''
url = args[-1]
fixed = "https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/"
metadata_response = None
metadata_kind = None
metadata_name = None
metadata_sha = None
for repository, commit in metadata_commits.items():
    commit_url = "https://api.github.com/repos/tailrocks/" + repository + "/git/commits/" + commit
    tree_url = "https://api.github.com/repos/tailrocks/" + repository + "/git/trees/" + metadata_trees[repository] + "?recursive=1"
    if url == commit_url:
        metadata_kind, metadata_name, metadata_sha = "commit", repository, commit
        metadata_response = {"sha": commit, "tree": {"sha": metadata_trees[repository]}}
    elif url == tree_url:
        metadata_kind, metadata_name, metadata_sha = "tree", repository, metadata_trees[repository]
        metadata_response = {"sha": metadata_trees[repository], "truncated": False, "tree": []}
metadata_allowed = metadata_response is not None
with events.open("a") as f:
    if token and any(token in arg for arg in args):
        f.write("credential-in-argv\\n")
    if token and not (url.startswith(fixed) or metadata_allowed):
        f.write("credential-scope-error\\n")
    if token and url.startswith(fixed) and ("--variable" not in args or "%GITHUB_COM_TOKEN" not in args or "--expand-header" not in args or "Authorization: Bearer {{GITHUB_COM_TOKEN}}" not in args or "--include" not in args):
        f.write("credential-scope-error\\n")
    if token and metadata_allowed and ("--variable" not in args or "%GITHUB_COM_TOKEN" not in args or "--expand-header" not in args or "Authorization: Bearer {{GITHUB_COM_TOKEN}}" not in args or "--include" not in args or "--dump-header" in args or "--disable" not in args or "--proto" not in args or "=https" not in args or "--max-time" not in args or "30" not in args or "--max-filesize" not in args or str(8 * 1024 * 1024) not in args or "--location" in args or "-L" in args):
        f.write("metadata-capture-error\\n")
    if not token and metadata_allowed and ("--dump-header" not in args or "--include" in args):
        f.write("metadata-capture-error\\n")
    if metadata_allowed:
        f.write(("authenticated" if token else "anonymous") + " metadata " + metadata_kind + " " + metadata_name + " " + metadata_sha + "\\n")
    else:
        f.write(("authenticated" if token else "anonymous") + " " + ("blob" if url.startswith(fixed) else "other") + "\\n")
    if url.startswith(fixed):
        f.write(("authenticated-oid " if token else "anonymous-oid ") + url[len(fixed):] + "\\n")
if metadata_allowed:
    mode = scenario.read_text() if scenario.exists() else ""
    status = b"HTTP/2 200\\r\\ncontent-type: application/json\\r\\n\\r\\n"
    body = json.dumps(metadata_response).encode()
    if mode == "metadata-header":
        status = b"HTTP/2 200\\r\\nX-Reflected: " + token.encode() + b"\\r\\n\\r\\n"
    elif mode == "metadata-body":
        metadata_response["reflected"] = token
        body = json.dumps(metadata_response).encode()
    elif mode == "metadata-redirect":
        status = b"HTTP/2 302\\r\\nLocation: https://attacker.invalid/\\r\\n\\r\\n"
        body = b""
    elif mode.startswith("metadata-redirect-"):
        parsed = urlsplit(url)
        if mode == "metadata-redirect-host":
            location = "https://attacker.invalid/" + parsed.path
        elif mode == "metadata-redirect-scheme":
            location = "http://api.github.com" + parsed.path + ("?" + parsed.query if parsed.query else "")
        elif mode == "metadata-redirect-userinfo":
            location = "https://user@api.github.com" + parsed.path + ("?" + parsed.query if parsed.query else "")
        elif mode == "metadata-redirect-port":
            location = "https://api.github.com:444" + parsed.path + ("?" + parsed.query if parsed.query else "")
        elif mode == "metadata-redirect-path":
            location = "https://api.github.com" + parsed.path + "/unexpected" + ("?" + parsed.query if parsed.query else "")
        elif mode == "metadata-redirect-query":
            location = "https://api.github.com" + parsed.path + "?recursive=0"
        elif mode == "metadata-redirect-identity":
            prefix = parsed.path.rsplit("/", 1)[0]
            location = "https://api.github.com" + prefix + "/" + "0" * 40 + ("?" + parsed.query if parsed.query else "")
        elif mode == "metadata-redirect-malformed":
            location = "https://api.github.com" + parsed.path + "\\x01"
        else:
            location = url
        status = b"HTTP/2 302\\r\\nLocation: " + location.encode() + b"\\r\\n\\r\\n"
        body = b""
    elif mode == "metadata-wrong-sha" and metadata_kind == "commit":
        metadata_response["sha"] = "0" * 40
        body = json.dumps(metadata_response).encode()
    elif mode == "metadata-wrong-tree-sha" and metadata_kind == "tree":
        metadata_response["sha"] = "0" * 40
        body = json.dumps(metadata_response).encode()
    elif mode == "metadata-malformed":
        body = b"{malformed"
    elif mode == "metadata-truncated":
        body = b"{\\"sha\\":"
    elif mode == "metadata-multiple-status":
        status = b"HTTP/2 200\\r\\n\\r\\nHTTP/1.1 200 Fixture\\r\\n\\r\\n"
    elif mode == "metadata-duplicate-location":
        status = b"HTTP/2 200\\r\\nLocation: https://api.github.com/first\\r\\nLocation: https://api.github.com/second\\r\\n\\r\\n"
    elif mode == "metadata-malformed-header":
        status = b"HTTP/2 200\\r\\nBad Header: invalid\\r\\n\\r\\n"
    elif mode == "metadata-oversize-header":
        status = b"HTTP/2 200\\r\\nX-Large: " + b"x" * 65537 + b"\\r\\n\\r\\n"
    elif mode == "metadata-403":
        status = b"HTTP/2 403 Forbidden\\r\\nX-RateLimit-Remaining: 0\\r\\n\\r\\n"
        body = b"private response detail"
    elif mode == "metadata-oversize":
        body = b"x" * (8 * 1024 * 1024 + 1)
    if "--dump-header" in args:
        Path(args[args.index("--dump-header") + 1]).write_bytes(status)
        if token and metadata_allowed:
            with events.open("a") as f:
                f.write("authenticated metadata filesystem-write headers=" + ("credential-reflected" if token.encode() in status else "no-credential") + "\\n")
        sys.stdout.buffer.write(body)
    else:
        sys.stdout.buffer.write(status + body)
    sys.exit(0)
if not url.startswith(fixed):
    if "--dump-header" in args:
        Path(args[args.index("--dump-header") + 1]).write_bytes(b"HTTP/1.1 502 Fixture\\r\\n\\r\\n")
    sys.exit(0)
oid = url[len(fixed):]
if len(oid) != 40 or any(c not in "0123456789abcdef" for c in oid):
    sys.exit(1)
data = subprocess.run(["/usr/bin/git", "-C", source, "cat-file", "blob", oid], check=True, stdout=subprocess.PIPE, env={"PATH": "/usr/bin:/bin", "HOME": "/tmp", "GIT_NO_LAZY_FETCH": "1", "GIT_NO_REPLACE_OBJECTS": "1", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}).stdout
body = json.dumps({"sha": oid, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode()}).encode()
headers = b"HTTP/2 200\\r\\ndate: Fri, 06 Nov 2026 00:00:00 GMT\\r\\ncontent-type: application/json\\r\\n\\r\\n"
if token:
    mode = scenario.read_text() if scenario.exists() else ""
    if mode == "header":
        headers = b"HTTP/2 200\\r\\nX-Reflected: " + token.encode() + b"\\r\\n\\r\\n"
    elif mode == "body":
        body = json.dumps({"sha": oid, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode(), "reflected": token}).encode()
    elif mode == "redirect":
        headers = b"HTTP/2 302\\r\\nLocation: https://attacker.invalid/\\r\\n\\r\\n"
    elif mode == "identity":
        body = json.dumps({"sha": "0" * 40, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode()}).encode()
    sys.stdout.buffer.write(headers + body)
else:
    Path(args[args.index("--dump-header") + 1]).write_bytes(headers)
    sys.stdout.buffer.write(body)
''')
    fake.chmod(0o700)
    checker = repo / "scripts/check-generated-workflows.sh"
    alternate_checker = os.environ.get("BASELINE_GUARD_TEST_CHECKER")
    deferred_hook_mutant = bool(alternate_checker) and selected_case in warm_cases
    alternate_checker_bytes = Path(alternate_checker).read_bytes() if deferred_hook_mutant else None
    hook_mutant_name = os.environ.get("BASELINE_GUARD_TEST_MUTANT_NAME") if deferred_hook_mutant else None
    if deferred_hook_mutant and hook_mutant_name not in {"metadata-token-only", "hook-pre-commit", "hook-pre-push"}:
        raise SystemExit("warm hook mutant fixture is missing its approved mutant identity")
    if deferred_hook_mutant and hook_mutant_name == "metadata-token-only":
        reviewed_mutant_source = (root / "scripts/check-generated-workflows.sh").read_text(encoding="utf-8")
        if hashlib.sha256(reviewed_mutant_source.encode("utf-8")).hexdigest() != qualified_checker_sha256:
            raise SystemExit("token-only mutant base checker does not match the pinned production review")
        def replace_mutant_once(text, old, new):
            if text.count(old) != 1:
                raise SystemExit("token-only mutant target is missing or ambiguous")
            return text.replace(old, new, 1)
        reviewed_mutant_source = replace_mutant_once(reviewed_mutant_source, 'if [[ "${1-}" == prepare-cache ]]; then', 'if [[ "${1-}" == prepare-cache || "${1-}" == pre-commit || "${1-}" == pre-push ]]; then')
        reviewed_mutant_source = replace_mutant_once(reviewed_mutant_source, 'if [[ "$installed_checker" -eq 1 && "${prepare_cache_mode:-0}" -eq 1 && -n "$trusted_setup_token" ]]; then', 'if [[ -n "$trusted_setup_token" ]]; then')
        old_mutant_block = "    safe_path=\"${mise_command%/*}:${git_command%/*}:$cargo_home/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n    if [[ \"$warm_tool_cache\" != true ]]; then\n      validate_trusted_git_source_trees \"$trusted_seed/Cargo.lock\" \"$home_dir\" \"$runtime_tmp\" \"$safe_path\" || exit $?\n    fi"
        new_mutant_block = old_mutant_block.replace('"$warm_tool_cache" != true', '"$warm_tool_cache" != true || -n "$trusted_setup_token"')
        reviewed_mutant_source = replace_mutant_once(reviewed_mutant_source, old_mutant_block, new_mutant_block)
        if alternate_checker_bytes != reviewed_mutant_source.encode("utf-8"):
            raise SystemExit("token-only mutant is not the exact approved three-edit checker mutation")
    if alternate_checker and not deferred_hook_mutant:
        shutil.copy2(alternate_checker, checker)
    original = checker.read_text()
    if original.count("curl_candidate=/usr/bin/curl") != 1:
        raise SystemExit("fixture checker does not contain one approved curl boundary")
    if not alternate_checker and hashlib.sha256(original.encode("utf-8")).hexdigest() != qualified_checker_sha256:
        raise SystemExit("fixture checker does not match the pinned production review")
    boundary_checker = original.replace("curl_candidate=/usr/bin/curl", "curl_candidate=" + repr(str(fake)), 1)
    checker.write_text(boundary_checker)
    if checker.read_text() != boundary_checker:
        raise SystemExit("fixture checker boundary substitution changed unexpected bytes")
    digest = hashlib.sha256(checker.read_bytes()).hexdigest()
    installer = repo / "scripts/install-generated-workflow-guard.sh"
    text = installer.read_text()
    lines = text.splitlines(keepends=True)
    installer.write_text("".join("reviewed_checker_sha256=" + digest + "\n" if line.startswith("reviewed_checker_sha256=") else line for line in lines))
    if evidence:
        (evidence / "checker-boundary.patch").write_text("".join(difflib.unified_diff(original.splitlines(keepends=True), checker.read_text().splitlines(keepends=True), fromfile="reviewed-checker", tofile="fixture-checker")))
        (evidence / "fixture-hashes.txt").write_text("checker_sha256=" + digest + "\ninstaller_sha256=" + hashlib.sha256(installer.read_bytes()).hexdigest() + "\ncurl_fixture_sha256=" + hashlib.sha256(fake.read_bytes()).hexdigest() + "\n")
    common = Path(run(["/usr/bin/git", "-C", str(repo), "rev-parse", "--path-format=absolute", "--git-common-dir"]).stdout.decode().strip())
    install_root = common / "jackin-generated-workflow-guard"
    install_root.mkdir(mode=0o700)
    shutil.copytree(artifacts, install_root / "artifacts", symlinks=True)

    def invoke_without_publication(name, args, stdin=None, environment=None):
        hooks_before = snapshot_hook_state(repo)
        result = invoke(name, args, stdin, environment)
        if snapshot_hook_state(repo) != hooks_before:
            raise SystemExit("setup published or changed Git hooks before qualification completed")
        cache_tools = install_root / "cache" / "tools"
        if cache_tools.is_symlink() or (cache_tools.exists() and not cache_tools.is_dir()):
            raise SystemExit("setup left an unsafe tool cache path after qualification failure")
        if cache_tools.is_dir() and any(re.fullmatch(r"[0-9a-f]{64}", entry.name) for entry in cache_tools.iterdir()):
            raise SystemExit("setup published a completed tool cache before qualification completed")
        if any(path.is_file() and not path.is_symlink() and dummy.encode() in path.read_bytes() for path in install_root.rglob("*")):
            raise SystemExit("credential appeared in the installation")
        return result

    def isolate_missing_blob():
        seed_listing = run(["/usr/bin/git", "-C", source, "ls-tree", "-r", seed_tree]).stdout.decode().splitlines()
        prefixes = ("tests/functional/baselines/functional/", "apps/jackin/tests/functional/baselines/functional/")
        excluded = set()
        included = set()
        for line in seed_listing:
            header, path = line.split("\t", 1)
            mode, kind, oid = header.split()
            excluded_baseline = False
            for prefix in prefixes:
                if path.startswith(prefix):
                    name = path[len(prefix):]
                    if "/" not in name and name.endswith(".tui.pb"):
                        if (mode, kind) != ("100644", "blob"):
                            raise SystemExit("trusted seed baseline exclusion has an unsupported object type")
                        excluded_baseline = True
                        break
            if excluded_baseline:
                excluded.add(oid)
            else:
                included.add(oid)
        available = set(run(["/usr/bin/git", "-C", str(repo), "cat-file", "--batch-all-objects", "--batch-check=%(objectname)"]).stdout.decode().splitlines())
        eligible = sorted((excluded - included) & available)
        if not eligible:
            raise SystemExit("fixture lacks an available current-seed-only baseline blob")
        omitted = eligible[0]
        if run(["/usr/bin/git", "-C", source, "cat-file", "-t", omitted]).stdout.decode().strip() != "blob" or run(["/usr/bin/git", "-C", str(repo), "cat-file", "-t", omitted]).stdout.decode().strip() != "blob":
            raise SystemExit("fixture omitted object is not an available current-seed blob")
        pack_path = base / "fixture-objects.pack"
        with pack_path.open("wb") as pack:
            subprocess.run(["/usr/bin/git", "-C", str(repo), "pack-objects", "--stdout"], input=("\n".join(sorted(available - {omitted})) + "\n").encode(), stdout=pack, stderr=subprocess.PIPE, env=clean, check=True)
        objects = common / "objects"
        replacement_objects = base / "replacement-objects"
        (replacement_objects / "pack").mkdir(parents=True)
        (replacement_objects / "info").mkdir()
        with pack_path.open("rb") as pack:
            subprocess.run(["/usr/bin/git", "-C", str(repo), "index-pack", "--stdin"], stdin=pack, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=dict(clean, GIT_OBJECT_DIRECTORY=str(replacement_objects)), check=True)
        old_objects = base / "old-objects"
        objects.rename(old_objects)
        replacement_objects.rename(objects)
        shutil.rmtree(old_objects)
        if subprocess.run(["/usr/bin/git", "-C", str(repo), "cat-file", "-e", omitted], env=clean, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
            raise SystemExit("fixture failed to omit its current-seed baseline blob")
        if evidence:
            (evidence / "omitted-object.txt").write_text(omitted + "\n")
        return omitted

    omitted = None if selected_case in warm_cases | {"all"} else isolate_missing_blob()
    env = dict(clean, GITHUB_COM_TOKEN=dummy)

    def bounded_redacted_output(data, limit=16384):
        redacted = data.replace(dummy.encode(), b"[dummy credential redacted]")
        if len(redacted) <= limit:
            return redacted
        suffix = ("\n[output truncated; captured_bytes=" + str(len(redacted)) + "]\n").encode()
        return redacted[:limit] + suffix

    def process_group_exists(process_group):
        try:
            os.killpg(process_group, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            return True
        except OSError:
            return True
        return True

    def signal_process_group(process_group, requested_signal):
        try:
            os.killpg(process_group, requested_signal)
        except ProcessLookupError:
            return "already-absent"
        except OSError as error:
            return "error-" + str(error.errno)
        return "sent"

    def wait_for_process_group_exit(process_group, duration):
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            if not process_group_exists(process_group):
                return True
            time.sleep(0.05)
        return not process_group_exists(process_group)

    def terminate_timed_out_process(process):
        process_group = process.pid
        term_result = signal_process_group(process_group, signal.SIGTERM)
        term_exit_confirmed = wait_for_process_group_exit(process_group, 2)
        kill_result = "not-needed"
        if not term_exit_confirmed:
            kill_result = signal_process_group(process_group, signal.SIGKILL)
        try:
            stdout, stderr = process.communicate(timeout=5)
        except subprocess.TimeoutExpired as first_error:
            try:
                process.kill()
            except OSError:
                pass
            try:
                stdout, stderr = process.communicate(timeout=5)
            except subprocess.TimeoutExpired as final_error:
                stdout = final_error.output or b""
                stderr = final_error.stderr or b""
                first_stdout = first_error.output or b""
                first_stderr = first_error.stderr or b""
                if not stdout.startswith(first_stdout):
                    stdout = first_stdout + stdout
                if not stderr.startswith(first_stderr):
                    stderr = first_stderr + stderr
                if process.stdout:
                    process.stdout.close()
                if process.stderr:
                    process.stderr.close()
        if process.returncode is None:
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
        group_exit_confirmed = wait_for_process_group_exit(process_group, 5)
        if not group_exit_confirmed:
            kill_result = signal_process_group(process_group, signal.SIGKILL)
            group_exit_confirmed = wait_for_process_group_exit(process_group, 5)
        return stdout or b"", stderr or b"", term_result, kill_result, group_exit_confirmed

    def invoke(name, args, stdin=None, environment=None):
        selected_environment = env if environment is None else environment
        print("BASELINE_AUTH_CHILD_STAGE_BEGIN=" + name, flush=True)
        if evidence:
            (evidence / "active-stage.txt").write_text("stage=" + name + "\noutcome=running\n")
        process = subprocess.Popen(
            args,
            cwd=repo,
            env=selected_environment,
            stdin=subprocess.PIPE if stdin is not None else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            stdout, stderr = process.communicate(input=stdin, timeout=180)
        except subprocess.TimeoutExpired:
            stdout, stderr, term_result, kill_result, group_exit_confirmed = terminate_timed_out_process(process)
            event_bytes = events.read_bytes() if events.exists() else b""
            safe_stdout = bounded_redacted_output(stdout)
            safe_stderr = bounded_redacted_output(stderr)
            safe_events = bounded_redacted_output(event_bytes)
            child_reaped = process.returncode is not None
            cleanup = "confirmed" if group_exit_confirmed and child_reaped else "unconfirmed"
            if evidence:
                (evidence / (name + ".result")).write_text(
                    "outcome=timeout\n"
                    + "stage=" + name + "\n"
                    + "timeout_seconds=180\n"
                    + "child_pid=" + str(process.pid) + "\n"
                    + "process_group_id=" + str(process.pid) + "\n"
                    + "term_signal=" + term_result + "\n"
                    + "kill_signal=" + kill_result + "\n"
                    + "child_returncode=" + str(process.returncode) + "\n"
                    + "child_reaped=" + ("true" if child_reaped else "false") + "\n"
                    + "process_group_exit=" + ("confirmed" if group_exit_confirmed else "unconfirmed") + "\n"
                    + "cleanup_scope=direct-child-and-owned-process-group\n"
                    + "owned_process_cleanup=" + cleanup + "\n"
                )
                (evidence / (name + ".stdout")).write_bytes(safe_stdout)
                (evidence / (name + ".stderr")).write_bytes(safe_stderr)
                (evidence / (name + ".events")).write_bytes(safe_events)
                (evidence / "active-stage.txt").write_text(
                    "stage=" + name + "\noutcome=timeout\nowned_process_cleanup=" + cleanup + "\n"
                )

            def diagnostic_output(label, data):
                decoded = data.decode("utf-8", "replace").rstrip()
                return label + ":\n" + (decoded if decoded else "[no output captured]")
            report = (
                "fixture child timed out: stage=" + name
                + " timeout_seconds=180 child_pid=" + str(process.pid)
                + " child_reaped=" + ("true" if child_reaped else "false")
                + " process_group_exit=" + ("confirmed" if group_exit_confirmed else "unconfirmed")
                + " owned_process_cleanup=" + cleanup
                + "\n" + diagnostic_output("child stdout (bounded, credential-redacted)", safe_stdout)
                + "\n" + diagnostic_output("child stderr (bounded, credential-redacted)", safe_stderr)
                + "\n" + diagnostic_output("fixture boundary events (bounded, credential-redacted)", safe_events)
            )
            print("BASELINE_AUTH_CHILD_STAGE_TIMEOUT=" + name + " owned_process_cleanup=" + cleanup, flush=True)
            raise SystemExit(report)
        result = subprocess.CompletedProcess(args, process.returncode, stdout, stderr)
        outputs = result.stdout + result.stderr
        event_bytes = events.read_bytes() if events.exists() else b""
        if dummy.encode() in outputs or dummy.encode() in event_bytes:
            if evidence:
                (evidence / (name + ".log")).write_text("credential detected; output withheld\n")
            raise SystemExit("credential appeared in fixture output")
        if evidence:
            (evidence / (name + ".stdout")).write_bytes(result.stdout)
            (evidence / (name + ".stderr")).write_bytes(result.stderr)
            (evidence / (name + ".events")).write_bytes(event_bytes)
            (evidence / (name + ".result")).write_text("exit=" + str(result.returncode) + "\n")
            (evidence / "active-stage.txt").write_text("stage=" + name + "\noutcome=exit\n")
        print("BASELINE_AUTH_CHILD_STAGE_COMPLETE=" + name + " exit=" + str(result.returncode), flush=True)
        return result

    expected_generated_paths = (
        ".github/AGENTS.md",
        ".github/CLAUDE.md",
        ".github/actions/declared-task-0/action.yml",
        ".github/actions/u26/action.yml",
        ".github/actions/velnor-tool-seed/action.yml",
        ".github/actions/velnor-tools-cache-restore/action.yml",
        ".github/actions/velnor-tools-cache-scrub/action.yml",
        ".github/actions/velnor-tools-prelude-u26/action.yml",
        ".github/actions/velnor-verify-jactionlint/action.yml",
        ".github/actions/velnor-verify-restored-rust/action.yml",
        ".github/actions/velnor-verify-rust-manifest/action.yml",
        ".github/jactionlint.yaml",
        ".github/scripts/velnor-tools-cache-identity.sh",
        ".github/velnor/task-execution-manifest-v1.json",
        ".github/workflows/cache-prune.yml",
        ".github/workflows/ci.yml",
        ".github/workflows/state-seal.yml",
        ".github/workflows/suites.yml",
    )
    readonly_input_messages = (
        "mise.lock is read-only input; Velnor never modifies it",
        "mise.toml is read-only input; Velnor never modifies it",
        "rust-toolchain.toml is read-only input; Velnor never modifies it",
    )

    def assert_generator_output(result, label):
        try:
            stdout_lines = result.stdout.decode("utf-8", "strict").splitlines()
            report_lines = result.stderr.decode("utf-8", "strict").splitlines()
        except UnicodeDecodeError as error:
            raise SystemExit("pinned generator output is not UTF-8: " + label) from error
        report_shape_prefixes = (
            "Preview:",
            "Repository:",
            ".github/",
            "Recommendation:",
            "Recommendations:",
            "Warning:",
            "Warnings:",
        )
        if any(line.startswith(report_shape_prefixes) or line in readonly_input_messages for line in stdout_lines):
            raise SystemExit("pinned generator emitted report-shaped output on stdout: " + label)
        if len(report_lines) != 2 + len(expected_generated_paths) + len(readonly_input_messages):
            raise SystemExit("pinned generator did not report the reviewed generated path set: " + label)
        if (
            not report_lines[0].startswith("Preview: ")
            or not report_lines[0][len("Preview: "):].strip()
            or not report_lines[1].startswith("Repository: ")
            or not report_lines[1][len("Repository: "):].strip()
        ):
            raise SystemExit("pinned generator did not report nonempty preview and repository locations: " + label)
        expected_report_tail = expected_generated_paths + readonly_input_messages
        if tuple(report_lines[2:]) != expected_report_tail:
            raise SystemExit("pinned generator did not report the reviewed generated path set: " + label)

    def assert_seed_tool_pins(repository):
        listing = run(["/usr/bin/git", "-C", str(repository), "ls-tree", "-r", "-z", seed_tree]).stdout
        if not listing.endswith(b"\0"):
            raise SystemExit("selected seed tree listing is incomplete")
        selected = {}
        rust_channels = []
        config_names = {"mise.toml", ".mise.toml", "mise.local.toml", ".mise.local.toml"}
        for raw_entry in listing[:-1].split(b"\0"):
            _, path_bytes = raw_entry.split(b"\t", 1)
            try:
                path = path_bytes.decode("utf-8", "strict")
            except UnicodeDecodeError as error:
                raise SystemExit("selected seed tree contains a non-UTF-8 path") from error
            filename = path.rsplit("/", 1)[-1]
            is_mise = filename in config_names or filename.startswith(("mise.", ".mise.")) and filename.endswith(".toml")
            if is_mise:
                config = tomllib.loads(run(["/usr/bin/git", "-C", str(repository), "show", seed_commit + ":" + path]).stdout.decode("utf-8", "strict"))
                tools = config.get("tools", {})
                if not isinstance(tools, dict):
                    raise SystemExit("selected seed Mise tools are not a table")
                for name, value in tools.items():
                    if name == "github:tailrocks/velnor-new":
                        if not isinstance(value, dict) or value.get("bin") != "velnor-actions":
                            raise SystemExit("selected seed Velnor identity is unsupported")
                        version = value.get("version")
                        key = "velnor"
                    elif name in {"rust", "jactionlint", "shellcheck"}:
                        version = value.get("version") if isinstance(value, dict) else value
                        key = name
                    else:
                        continue
                    if key in selected and selected[key] != version:
                        raise SystemExit("selected seed has conflicting tool pins")
                    selected[key] = version
            if filename == "rust-toolchain.toml":
                config = tomllib.loads(run(["/usr/bin/git", "-C", str(repository), "show", seed_commit + ":" + path]).stdout.decode("utf-8", "strict"))
                rust_channels.append(config.get("toolchain", {}).get("channel") if isinstance(config.get("toolchain"), dict) else None)
        expected = {"velnor": "0.1.78", "rust": "1.99.1", "jactionlint": "2.4.0", "shellcheck": "0.11.0"}
        if selected != expected or not rust_channels or any(channel != expected["rust"] for channel in rust_channels):
            raise SystemExit("selected authenticated seed does not pin the qualified Velnor 0.1.78 tool tuple")

    def prepare_warm_checker_fixture():
        source_generation = Path(qualified_cache_source).resolve(strict=True)
        if source_generation != qualified_cache_generation.resolve(strict=True):
            raise SystemExit("qualified cache source changed after its startup check")
        if run(["/usr/bin/git", "-C", str(repo), "rev-parse", "HEAD"]).stdout.decode().strip() != qualified_fixture_head:
            raise SystemExit("fixture source HEAD does not match the pinned old baseline reference")
        fixed_cache, fixed_key, source_snapshot = rebind_qualified_cache(source_generation, digest)
        metadata_pack = repo / "scripts/trusted-seed-metadata.pack"
        metadata_objects = repo / "scripts/trusted-seed-metadata.objects"
        if hashlib.sha256(metadata_pack.read_bytes()).hexdigest() != "845b98b650d85074bdad58d771e785f01d0824e4647efe0113fcdb7821704874" or hashlib.sha256(metadata_objects.read_bytes()).hexdigest() != "58233f6e2765561ef177650f49b76fb45e0f0980a97d98df5f39759a36525268":
            raise SystemExit("selected seed metadata pack does not match its embedded reviewed identities")
        with metadata_pack.open("rb") as pack:
            subprocess.run(["/usr/bin/git", "-C", str(repo), "index-pack", "--stdin"], stdin=pack, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=clean, check=True)
        selected_tree = run(["/usr/bin/git", "-C", str(repo), "rev-parse", "--verify", seed_commit + "^{tree}"]).stdout.decode().strip()
        if selected_tree != seed_tree or run(["/usr/bin/git", "-C", str(repo), "cat-file", "-t", seed_commit]).stdout.decode().strip() != "commit":
            raise SystemExit("authenticated seed commit does not resolve to the reviewed root tree")
        run(["/usr/bin/git", "-C", str(repo), "read-tree", seed_commit])
        if run(["/usr/bin/git", "-C", str(repo), "write-tree"]).stdout.decode().strip() != seed_tree:
            raise SystemExit("fixture index does not equal the authenticated seed tree")
        assert_seed_tool_pins(repo)
        if filesystem_snapshot(source_generation) != source_snapshot:
            raise SystemExit("qualified source cache changed during selected seed preparation")
        events.write_text("")
        installed = invoke("warm-install", ["/bin/bash", str(installer)])
        setup_log = events.read_text() if events.exists() else ""
        if installed.returncode != 0 or not matches_metadata_events(setup_log, "anonymous", 0) or request_events(setup_log) or "credential-in-argv" in setup_log or "credential-scope-error" in setup_log:
            raise SystemExit("warm fixture installer did not complete from the qualified cached toolchain")
        assert_generator_output(installed, "warm fixture installation")
        if dummy.encode() in installed.stdout + installed.stderr or filesystem_snapshot(source_generation) != source_snapshot:
            raise SystemExit("warm fixture installer exposed credentials or changed the source cache")
        fixed_version = install_root / "versions" / digest
        fixed_checker = fixed_version / "check-generated-workflows.sh"
        if not fixed_checker.is_file() or hashlib.sha256(fixed_checker.read_bytes()).hexdigest() != digest:
            raise SystemExit("warm fixture installer did not publish the reviewed checker generation")
        if payload_inventory(fixed_cache / "tool-state") != qualified_payload_sha256:
            raise SystemExit("warm fixture installer changed its copied tool payload")
        fixed_hooks = snapshot_hook_state(repo)
        mutant_checker_path = fixed_checker
        mutant_digest = None
        mutant_cache = None
        mutant_cache_key = None
        if deferred_hook_mutant:
            mutant_source = alternate_checker_bytes.decode("utf-8", "strict")
            if mutant_source.count("curl_candidate=/usr/bin/curl") != 1:
                raise SystemExit("hook mutant checker does not have the unique approved curl boundary")
            mutant_source = mutant_source.replace("curl_candidate=/usr/bin/curl", "curl_candidate=" + repr(str(fake)))
            mutant_digest = hashlib.sha256(mutant_source.encode("utf-8")).hexdigest()
            mutant_version = install_root / "versions" / mutant_digest
            if mutant_version.exists() or mutant_version.is_symlink() or mutant_digest == digest:
                raise SystemExit("hook mutant generation is ambiguous")
            shutil.copytree(fixed_version, mutant_version, symlinks=True, copy_function=shutil.copy2)
            mutant_checker_path = mutant_version / "check-generated-workflows.sh"
            checker_mode = stat.S_IMODE(mutant_checker_path.lstat().st_mode)
            os.chmod(mutant_checker_path, checker_mode | 0o200)
            mutant_checker_path.write_text(mutant_source)
            os.chmod(mutant_checker_path, checker_mode)
            manifest_path = mutant_version / "manifest"
            manifest_raw = manifest_path.read_bytes()
            version_fields = {}
            version_lines = []
            changed_fields = set()
            for line in manifest_raw.splitlines(keepends=True):
                if not line.endswith(b"\n") or line[:-1].count(b"=") != 1:
                    raise SystemExit("installed checker manifest is malformed before mutant rebinding")
                key, value = line[:-1].split(b"=", 1)
                key_text = key.decode("ascii")
                if key_text in version_fields:
                    raise SystemExit("installed checker manifest has a duplicate field")
                version_fields[key_text] = value.decode("ascii")
                if key == b"checker_sha256":
                    version_lines.append(b"checker_sha256=" + mutant_digest.encode("ascii") + b"\n")
                    changed_fields.add("checker_sha256")
                else:
                    version_lines.append(line)
            if changed_fields != {"checker_sha256"} or version_fields.get("checker_sha256") != digest:
                raise SystemExit("installed checker manifest cannot be uniquely rebound to the approved mutant")
            manifest_mode = stat.S_IMODE(manifest_path.lstat().st_mode)
            os.chmod(manifest_path, manifest_mode | 0o200)
            manifest_path.write_bytes(b"".join(version_lines))
            os.chmod(manifest_path, manifest_mode)
            mutant_cache, mutant_cache_key, mutant_source_snapshot = rebind_qualified_cache(source_generation, mutant_digest)
            if mutant_source_snapshot != source_snapshot or payload_inventory(mutant_cache / "tool-state") != payload_inventory(fixed_cache / "tool-state"):
                raise SystemExit("hook mutant cache does not preserve the fixed qualified payload")
            _, mutant_fields = parse_cache_manifest(mutant_cache / "manifest")
            if mutant_cache != install_root / "cache/tools" / mutant_cache_key or mutant_fields["key"] != mutant_cache_key or mutant_fields["checker_sha256"] != mutant_digest or cache_key(mutant_fields, mutant_digest) != mutant_cache_key:
                raise SystemExit("hook mutant cache path does not match its rebound schema-2 identity")
        if filesystem_snapshot(source_generation) != source_snapshot or snapshot_hook_state(repo) != fixed_hooks:
            raise SystemExit("warm fixture setup changed its qualified source cache or published hooks unexpectedly")
        return fixed_checker, mutant_checker_path, source_generation, source_snapshot, fixed_cache, fixed_hooks, fixed_key, mutant_digest, mutant_cache, mutant_cache_key

    def object_stores(repository, trusted_install_root):
        object_path = Path(run(["/usr/bin/git", "-C", str(repository), "rev-parse", "--path-format=absolute", "--git-path", "objects"]).stdout.decode().strip()).resolve(strict=True)
        result = [object_path]
        pending = [object_path]
        while pending:
            directory = pending.pop()
            alternates = directory / "info/alternates"
            if not alternates.exists():
                continue
            if alternates.is_symlink() or not alternates.is_file():
                raise SystemExit("fixture Git object store has an unsafe alternate list")
            for line in alternates.read_text().splitlines():
                alternate = Path(line)
                if not alternate.is_absolute():
                    alternate = directory / alternate
                alternate = alternate.resolve(strict=True)
                if alternate not in result:
                    result.append(alternate)
                    pending.append(alternate)
        seed_cache = trusted_install_root / "cache/seed"
        if seed_cache.is_dir() and not seed_cache.is_symlink():
            for candidate in seed_cache.rglob("objects"):
                if candidate.is_dir() and not candidate.is_symlink():
                    resolved = candidate.resolve(strict=True)
                    if resolved not in result:
                        result.append(resolved)
        return tuple(result)

    def assert_blob_absent_from_sources(repository, trusted_install_root, oid):
        for object_directory in object_stores(repository, trusted_install_root):
            result = subprocess.run(["/usr/bin/git", "-C", str(repository), "cat-file", "-e", oid], env=dict(clean, GIT_OBJECT_DIRECTORY=str(object_directory)), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if result.returncode == 0:
                raise SystemExit("fixture omitted blob remains available in an accessible Git object store")
            if result.returncode != 1:
                raise SystemExit("fixture could not verify omitted blob absence in an accessible Git object store")

    def write_manifest_field(path, field, value):
        original_mode = stat.S_IMODE(path.lstat().st_mode)
        lines = path.read_bytes().splitlines(keepends=True)
        changed = 0
        output = []
        for line in lines:
            if line.startswith(field.encode("ascii") + b"="):
                output.append(field.encode("ascii") + b"=" + value.encode("ascii") + b"\n")
                changed += 1
            else:
                output.append(line)
        if changed != 1:
            raise SystemExit("private cache fixture could not uniquely change its selected manifest field")
        os.chmod(path, original_mode | 0o200)
        path.write_bytes(b"".join(output))
        os.chmod(path, original_mode)

    def no_generator_success(result, label):
        try:
            lines = (result.stdout + result.stderr).decode("utf-8", "strict").splitlines()
        except UnicodeDecodeError as error:
            raise SystemExit("negative cache fixture output is not UTF-8: " + label) from error
        if any(line.startswith(".github/") or line.startswith("Preview: ") or line.startswith("Repository: ") or line in readonly_input_messages for line in lines):
            raise SystemExit("negative cache fixture reached generated-workflow success output: " + label)

    def selected_velnor_config():
        candidates = []
        listing = run(["/usr/bin/git", "-C", str(repo), "ls-tree", "-r", "-z", seed_tree]).stdout
        if not listing.endswith(b"\0"):
            raise SystemExit("selected seed tree listing is incomplete while staging a pin negative")
        for raw_entry in listing[:-1].split(b"\0"):
            _, path_bytes = raw_entry.split(b"\t", 1)
            path = path_bytes.decode("utf-8", "strict")
            filename = path.rsplit("/", 1)[-1]
            if not (filename in {"mise.toml", ".mise.toml", "mise.local.toml", ".mise.local.toml"} or filename.startswith(("mise.", ".mise.")) and filename.endswith(".toml")):
                continue
            content = run(["/usr/bin/git", "-C", str(repo), "show", seed_commit + ":" + path]).stdout
            config = tomllib.loads(content.decode("utf-8", "strict"))
            tools = config.get("tools", {})
            velnor = tools.get("github:tailrocks/velnor-new") if isinstance(tools, dict) else None
            if isinstance(velnor, dict) and velnor.get("version") == "0.1.78" and velnor.get("bin") == "velnor-actions":
                candidates.append((path, content))
        if len(candidates) != 1:
            raise SystemExit("selected seed does not have one exact Velnor configuration for the pin negative")
        return candidates[0]

    def stage_incompatible_selected_pin(mode, records):
        index_path = common / "index"
        original_index = index_path.read_bytes()
        original_index_mode = stat.S_IMODE(index_path.lstat().st_mode)
        path, content = selected_velnor_config()
        old_pin = b'version = "0.1.78"'
        new_pin = b'version = "0.1.79"'
        if content.count(old_pin) != 1:
            raise SystemExit("selected Velnor pin cannot be uniquely changed in the private negative fixture")
        changed_content = content.replace(old_pin, new_pin, 1)
        changed_config = tomllib.loads(changed_content.decode("utf-8", "strict"))
        if changed_config["tools"]["github:tailrocks/velnor-new"].get("version") != "0.1.79":
            raise SystemExit("incompatible selected Velnor pin did not parse as intended")
        blob = run(["/usr/bin/git", "-C", str(repo), "hash-object", "-w", "--stdin"], input=changed_content).stdout.decode().strip()
        run(["/usr/bin/git", "-C", str(repo), "update-index", "--cacheinfo", "100644," + blob + "," + path])
        selected_records = records
        if mode == "pre-push":
            changed_tree = run(["/usr/bin/git", "-C", str(repo), "write-tree", "--missing-ok"]).stdout.decode().strip()
            changed_commit = subprocess.run(["/usr/bin/git", "-C", str(repo), "commit-tree", changed_tree, "-m", "incompatible pin fixture"], env=commit_env, check=True, stdout=subprocess.PIPE).stdout.decode().strip()
            selected_records = ("refs/heads/main " + changed_commit + " refs/heads/main " + "0" * 40 + "\n").encode()

        def restore():
            index_path.write_bytes(original_index)
            os.chmod(index_path, original_index_mode)

        return restore, selected_records

    def run_public_cache_negative(mode, hook_path, records, name, diagnostic, mutate):
        original_state = snapshot_public_state(repo, common)
        original_hooks = snapshot_hook_state(repo)
        source_before = filesystem_snapshot(qualified_source)
        original_cache = filesystem_snapshot(fixed_cache)
        original_mutant_cache = filesystem_snapshot(mutant_cache) if mutant_cache else None
        restore, selected_records = mutate()
        try:
            staged_state = snapshot_public_state(repo, common)
            events.write_text("")
            assert_blob_absent_from_sources(repo, install_root, omitted)
            rejected = invoke("warm-negative-" + mode + "-" + name, ["/bin/bash", str(hook_path)], selected_records)
            log = events.read_text() if events.exists() else ""
            if name == "selected-pin-incompatible":
                if rejected.returncode != 1 or rejected.stdout != b"" or rejected.stderr != diagnostic:
                    raise SystemExit("public warm negative did not reach its selected cache or pin diagnostic: " + name)
            elif rejected.returncode == 0 or diagnostic not in rejected.stderr:
                raise SystemExit("public warm negative did not reach its selected cache or pin diagnostic: " + name)
            no_generator_success(rejected, name)
            if dummy.encode() in rejected.stdout + rejected.stderr or dummy in log or observed_metadata_events(log) or "authenticated " in log or "credential-" in log:
                raise SystemExit("public warm negative crossed or exposed the authenticated metadata boundary: " + name)
            requests = request_events(log)
            oids = [line for line in log.splitlines() if line.startswith(("authenticated-oid ", "anonymous-oid "))]
            allowed_observations = (([], []), (["anonymous blob"], ["anonymous-oid " + omitted]))
            if (requests, oids) not in allowed_observations:
                raise SystemExit("public warm negative escaped the intended anonymous hydration boundary: " + name)
            if snapshot_public_state(repo, common) != staged_state or snapshot_hook_state(repo) != original_hooks:
                raise SystemExit("public warm negative changed the staged repository or installed hooks: " + name)
            assert_blob_absent_from_sources(repo, install_root, omitted)
        finally:
            restore()
        if snapshot_public_state(repo, common) != original_state or snapshot_hook_state(repo) != original_hooks:
            raise SystemExit("public warm negative failed to restore repository or hook state: " + name)
        if filesystem_snapshot(qualified_source) != source_before or filesystem_snapshot(fixed_cache) != original_cache or (mutant_cache is not None and filesystem_snapshot(mutant_cache) != original_mutant_cache):
            raise SystemExit("public warm negative changed the qualified source or private fixture cache: " + name)
        print("BASELINE_AUTH_NEGATIVE_CASE_COMPLETE=" + mode + ":" + name, flush=True)

    def run_public_cache_negatives(mode, hook_path, records):
        completed = []
        cache_parent = fixed_cache.parent

        def missing_generation():
            backup = cache_parent / ".negative-cache-backup"
            if backup.exists() or backup.is_symlink():
                raise SystemExit("private cache negative backup path already exists")
            fixed_cache.rename(backup)
            def restore():
                backup.rename(fixed_cache)
            return restore, records

        def tampered_manifest():
            manifest = fixed_cache / "manifest"
            original = manifest.read_bytes()
            original_mode = stat.S_IMODE(manifest.lstat().st_mode)
            write_manifest_field(manifest, "velnor", "0.1.79")
            def restore():
                os.chmod(manifest, original_mode | 0o200)
                manifest.write_bytes(original)
                os.chmod(manifest, original_mode)
            return restore, records

        payload_file = next(path for path in sorted((fixed_cache / "tool-state").rglob("*")) if path.is_file() and not path.is_symlink())

        def tampered_payload():
            original = payload_file.read_bytes()
            original_mode = stat.S_IMODE(payload_file.lstat().st_mode)
            os.chmod(payload_file, original_mode | 0o200)
            payload_file.write_bytes(original + b"\0")
            def restore():
                os.chmod(payload_file, original_mode | 0o200)
                payload_file.write_bytes(original)
                os.chmod(payload_file, original_mode)
            return restore, records

        def unsafe_mode():
            manifest = fixed_cache / "manifest"
            original_manifest = manifest.read_bytes()
            manifest_mode = stat.S_IMODE(manifest.lstat().st_mode)
            original_mode = stat.S_IMODE(payload_file.lstat().st_mode)
            os.chmod(payload_file, original_mode | 0o200)
            write_manifest_field(manifest, "payload_sha256", payload_inventory(fixed_cache / "tool-state"))
            def restore():
                os.chmod(payload_file, original_mode)
                os.chmod(manifest, manifest_mode | 0o200)
                manifest.write_bytes(original_manifest)
                os.chmod(manifest, manifest_mode)
            return restore, records

        def unsafe_link():
            manifest = fixed_cache / "manifest"
            original_manifest = manifest.read_bytes()
            manifest_mode = stat.S_IMODE(manifest.lstat().st_mode)
            link = next(path for path in sorted((fixed_cache / "tool-state").rglob("*")) if path.is_symlink())
            original_link = os.readlink(link)
            link_mode = stat.S_IMODE(link.lstat().st_mode)
            link_parent = link.parent
            link_parent_mode = stat.S_IMODE(link_parent.lstat().st_mode)

            def restore_link_mode():
                if stat.S_IMODE(link.lstat().st_mode) == link_mode:
                    return
                if hasattr(os, "lchmod"):
                    os.lchmod(link, link_mode)
                elif os.chmod in os.supports_follow_symlinks:
                    os.chmod(link, link_mode, follow_symlinks=False)
                else:
                    raise SystemExit("private cache fixture cannot restore symlink mode")

            def replace_link(target):
                os.chmod(link_parent, link_parent_mode | stat.S_IWUSR)
                try:
                    if not (link.is_symlink() and os.readlink(link) == target):
                        if link.is_symlink() or link.exists():
                            link.unlink()
                        link.symlink_to(target)
                    restore_link_mode()
                finally:
                    os.chmod(link_parent, link_parent_mode)

            def restore():
                try:
                    replace_link(original_link)
                finally:
                    os.chmod(manifest, manifest_mode | 0o200)
                    try:
                        manifest.write_bytes(original_manifest)
                    finally:
                        os.chmod(manifest, manifest_mode)

            try:
                replace_link("/tmp")
                write_manifest_field(manifest, "payload_sha256", payload_inventory(fixed_cache / "tool-state"))
            except BaseException:
                restore()
                raise

            return restore, records

        def incompatible_pin():
            return stage_incompatible_selected_pin(mode, records)

        negative_cases = (
            ("cache-absent", b"qualified tool cache is unavailable", missing_generation),
            ("manifest-tampered", b"qualified tool cache manifest does not match its reviewed identity", tampered_manifest),
            ("payload-tampered", b"qualified tool cache payload changed; refusing to execute it", tampered_payload),
            ("unsafe-mode", b"qualified tool cache has unsafe owner or writable contents", unsafe_mode),
            ("unsafe-link", b"qualified tool cache has an escaping symlink", unsafe_link),
            ("selected-pin-incompatible", b"generated-workflows: snapshot tool tuple is outside the reviewed support policy\ngenerated-workflows: cannot read reviewed snapshot tool data\n", incompatible_pin),
        )
        for name, diagnostic, mutate in negative_cases:
            run_public_cache_negative(mode, hook_path, records, name, diagnostic, mutate)
            completed.append(name)
        expected = {name for name, _, _ in negative_cases}
        if len(completed) != len(negative_cases) or set(completed) != expected:
            raise SystemExit("public warm negative witness set is incomplete or duplicated: " + mode)
    if selected_case == "metadata-no-token":
        events.write_text("")
        result = invoke_without_publication("metadata-no-token", ["/bin/bash", str(installer)], environment=clean)
        log = events.read_text() if events.exists() else ""
        if result.returncode == 0 or b"rustup bootstrap response has an unexpected status" not in result.stderr or not matches_metadata_events(log, "anonymous") or not matches_request_boundary(log, "anonymous", 4, True) or "metadata-capture-error" in log:
            raise SystemExit("absent setup credential did not use the bounded anonymous metadata path")
        print("absent setup credential uses anonymous metadata requests: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case == "metadata-invalid-token":
        events.write_text("")
        invalid = dict(clean, GITHUB_COM_TOKEN="invalid credential")
        result = invoke_without_publication("metadata-invalid-token", ["/bin/bash", str(installer)], environment=invalid)
        log = events.read_text() if events.exists() else ""
        if result.returncode == 0 or b"invalid setup credential format" not in result.stderr or log:
            raise SystemExit("malformed setup credential reached a child process")
        print("malformed setup credential rejected before child processes: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case == "metadata-uninstalled":
        events.write_text("")
        result = invoke_without_publication("metadata-uninstalled", ["/bin/bash", str(checker), "prepare-cache"], environment=env)
        log = events.read_text() if events.exists() else ""
        if result.returncode == 0 or b"prepare-cache is available only through an installed trusted checker" not in result.stderr or log:
            raise SystemExit("uninstalled checker accepted setup authorization or started a child")
        print("uninstalled checker rejects setup authorization before child processes: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case == "metadata-confinement":
        scenario.write_text("metadata-header")
        events.write_text("")
        rejected = invoke_without_publication("metadata-confinement", ["/bin/bash", str(installer)])
        log = events.read_text() if events.exists() else ""
        if "authenticated metadata filesystem-write headers=credential-reflected" in log:
            raise SystemExit("credential-bearing metadata header was written to a file during curl")
        if rejected.returncode == 0 or b"response contains the setup credential" not in rejected.stderr or not matches_metadata_events(log, "authenticated", 1) or not matches_request_boundary(log, "authenticated", 1) or dummy in log or "credential-in-argv" in log or "credential-scope-error" in log or "metadata-capture-error" in log:
            raise SystemExit("authenticated metadata response was not confined to bounded memory")
        print("reflected authenticated metadata headers remain in bounded memory: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case == "metadata-response":
        expected_diagnostics = {
            "metadata-header": b"response contains the setup credential",
            "metadata-body": b"response contains the setup credential",
            "metadata-wrong-sha": b"commit identity does not match the lock",
            "metadata-wrong-tree-sha": b"tree metadata is incomplete or ambiguous",
            "metadata-malformed": b"commit metadata is malformed",
            "metadata-truncated": b"commit metadata is malformed",
            "metadata-multiple-status": b"response has ambiguous status headers",
            "metadata-duplicate-location": b"response has ambiguous redirect locations",
            "metadata-malformed-header": b"response has malformed headers",
            "metadata-oversize-header": b"response headers have an invalid size",
            "metadata-oversize": b"response exceeds the size limit",
        }
        for mode, diagnostic in expected_diagnostics.items():
            scenario.write_text(mode)
            events.write_text("")
            rejected = invoke_without_publication("metadata-" + mode, ["/bin/bash", str(installer)])
            log = events.read_text() if events.exists() else ""
            if rejected.returncode == 0 or diagnostic not in rejected.stderr or dummy.encode() in rejected.stdout + rejected.stderr or "metadata-capture-error" in log or "credential-in-argv" in log or "credential-scope-error" in log:
                raise SystemExit("authenticated metadata response rejection failed: " + mode)
            expected_requests = 2 if mode == "metadata-wrong-tree-sha" else 1
            if dummy in log or not matches_metadata_events(log, "authenticated", expected_requests) or not matches_request_boundary(log, "authenticated", expected_requests):
                raise SystemExit("metadata response escaped its bounded authenticated endpoint: " + mode)
            for path in install_root.rglob("*"):
                if path.is_file() and not path.is_symlink() and dummy.encode() in path.read_bytes():
                    raise SystemExit("credential appeared in the installation: " + mode)
        scenario.unlink(missing_ok=True)
        print("authenticated metadata identity, bounds, and reflection cases: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case == "metadata-redirect":
        redirect_cases = {
            "metadata-redirect-host": b"outside the official API path",
            "metadata-redirect-scheme": b"outside the official API path",
            "metadata-redirect-userinfo": b"outside the official API path",
            "metadata-redirect-port": b"outside the official API path",
            "metadata-redirect-path": b"outside the official API path",
            "metadata-redirect-query": b"outside the official API path",
            "metadata-redirect-identity": b"outside the official API path",
            "metadata-redirect-malformed": b"response has malformed headers",
            "metadata-redirect-loop": b"redirect loop detected",
        }
        for mode, diagnostic in redirect_cases.items():
            scenario.write_text(mode)
            events.write_text("")
            rejected = invoke_without_publication(mode, ["/bin/bash", str(installer)])
            log = events.read_text() if events.exists() else ""
            if rejected.returncode == 0 or diagnostic not in rejected.stderr or not matches_metadata_events(log, "authenticated", 1) or not matches_request_boundary(log, "authenticated", 1) or "credential-scope-error" in log or "metadata-capture-error" in log or "credential-in-argv" in log:
                raise SystemExit("authenticated metadata redirect escaped its exact endpoint: " + mode)
        print("changed-origin metadata redirect rejected before a second request: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case == "metadata-status":
        scenario.write_text("metadata-403")
        events.write_text("")
        rejected = invoke_without_publication("metadata-status", ["/bin/bash", str(installer)])
        output = rejected.stdout + rejected.stderr
        log = events.read_text() if events.exists() else ""
        metadata_events = observed_metadata_events(log)
        expected_commits = expected_metadata_events("authenticated")[:1]
        if rejected.returncode == 0 or b"HTTP status 403" not in output or b"private response detail" in output or dummy.encode() in output or "metadata-capture-error" in log or "credential-in-argv" in log or "credential-scope-error" in log or metadata_events != expected_commits or not matches_request_boundary(log, "authenticated", 1):
            raise SystemExit("metadata HTTP failure was not diagnosed with sanitized numeric status")
        print("metadata HTTP failure reports only its numeric status: PASS")
        mark_case_assertions(selected_case)
        finish_case(selected_case)
    if selected_case not in warm_cases | {"all"}:
        result = invoke_without_publication("setup", ["/bin/bash", str(installer)])
        if dummy.encode() in result.stdout + result.stderr:
            raise SystemExit("credential appeared in setup output")
        log = events.read_text() if events.exists() else ""
        if not matches_metadata_events(log, "authenticated") or not matches_request_boundary(log, "authenticated", 4, True) or "metadata-capture-error" in log or "credential-in-argv" in log or "credential-scope-error" in log or result.returncode == 0 or b"rustup bootstrap response has an unexpected status" not in result.stderr:
            raise SystemExit("authenticated fixed-host boundary failed")
        if "anonymous other\n" not in log:
            raise SystemExit("setup did not reach the next anonymous boundary")
        authenticated_oids = {line.split()[1] for line in log.splitlines() if line.startswith("authenticated-oid ")}
        if omitted not in authenticated_oids:
            raise SystemExit("setup did not authenticate the selected missing current-seed blob")
        for path in install_root.rglob("*"):
            if path.is_file() and not path.is_symlink() and dummy.encode() in path.read_bytes():
                raise SystemExit("credential appeared in the installation")
        if result.returncode == 0:
            raise SystemExit("fixture unexpectedly completed real tool qualification")
        if selected_case in ("setup", "metadata-auth"):
            print("authenticated full-seed setup reaches anonymous next boundary: PASS")
            mark_case_assertions(selected_case)
            finish_case(selected_case)
    if selected_case in warm_cases | {"all"}:
        original_omitted = omitted
        fixed_checker, mutant_checker, qualified_source, qualified_snapshot, fixed_cache, fixed_hooks, fixed_cache_key, mutant_digest, mutant_cache, mutant_cache_key = prepare_warm_checker_fixture()
        omitted = isolate_missing_blob()
        if original_omitted is not None and omitted != original_omitted:
            raise SystemExit("warm fixture selected a different omitted blob than its cold witness")
        assert_blob_absent_from_sources(repo, install_root, omitted)
    missing = omitted
    if selected_case in warm_cases | {"all"} and run(["/usr/bin/git", "-C", str(repo), "write-tree", "--missing-ok"]).stdout.decode().strip() != seed_tree:
        raise SystemExit("warm fixture selected index no longer equals the authenticated seed tree")
    run(["/usr/bin/git", "-C", str(repo), "update-index", "--add", "--info-only", "--cacheinfo", "100644," + missing + ",missing-auth-fixture.txt"])
    tree = run(["/usr/bin/git", "-C", str(repo), "write-tree", "--missing-ok"]).stdout.decode().strip()
    if selected_case in warm_cases | {"all"}:
        base_entries = run(["/usr/bin/git", "-C", str(repo), "ls-tree", "-r", "-z", seed_tree]).stdout.split(b"\0")
        final_entries = run(["/usr/bin/git", "-C", str(repo), "ls-tree", "-r", "-z", tree]).stdout.split(b"\0")
        base_entries = [entry for entry in base_entries if entry]
        final_entries = [entry for entry in final_entries if entry]
        expected_entry = b"100644 blob " + missing.encode("ascii") + b"\tmissing-auth-fixture.txt"
        if sorted(final_entries) != sorted(base_entries + [expected_entry]):
            raise SystemExit("warm fixture selected tree differs from the authenticated seed only by the intended missing blob")
    commit_env = dict(clean, GIT_AUTHOR_NAME="Guard Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid", GIT_COMMITTER_NAME="Guard Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid")
    commit = subprocess.run(["/usr/bin/git", "-C", str(repo), "commit-tree", tree, "-m", "fixture"], env=commit_env, check=True, stdout=subprocess.PIPE).stdout.decode().strip()
    fixture_head = run(["/usr/bin/git", "-C", str(repo), "rev-parse", "HEAD"]).stdout.decode().strip()
    if selected_case in warm_cases | {"all"} and fixture_head != qualified_fixture_head:
        raise SystemExit("warm fixture changed the old baseline HEAD")
    if commit == fixture_head:
        raise SystemExit("pre-push fixture commit is not distinct from HEAD")
    if evidence and selected_case in warm_cases | {"all"}:
        (evidence / "warm-fixture-binding.txt").write_text(
            "qualified_cache_path=" + str(qualified_cache_generation) + "\n"
            + "qualified_manifest_sha256=" + qualified_source_manifest_sha256 + "\n"
            + "qualified_payload_sha256=" + qualified_payload_sha256 + "\n"
            + "qualified_source_snapshot_sha256=" + hashlib.sha256(repr(qualified_snapshot).encode()).hexdigest() + "\n"
            + "trusted_seed_commit=" + seed_commit + "\n"
            + "trusted_seed_tree=" + seed_tree + "\n"
            + "candidate_tree=" + tree + "\n"
            + "pre_push_commit=" + commit + "\n"
            + "original_head=" + fixture_head + "\n"
            + "omitted_blob=" + omitted + "\n"
            + "fixed_checker_sha256=" + digest + "\n"
            + "fixed_cache_key=" + fixed_cache_key + "\n"
            + "mutant_checker_sha256=" + (mutant_digest or "none") + "\n"
            + "mutant_cache_key=" + (mutant_cache_key or "none") + "\n"
        )
    successful_hook_modes = set()
    for mode in ("pre-commit", "pre-push"):
        if not selected("hook-" + mode):
            continue
        records = None if mode == "pre-commit" else ("refs/heads/main " + commit + " refs/heads/main " + "0" * 40 + "\n").encode()
        if selected_case in warm_cases and run(["/usr/bin/git", "-C", str(repo), "rev-parse", commit + "^{tree}"]).stdout.decode().strip() != tree:
            raise SystemExit("pre-push fixture commit does not name its exact selected candidate tree")
        hooks_directory = Path(run(["/usr/bin/git", "-C", str(repo), "rev-parse", "--path-format=absolute", "--git-path", "hooks"]).stdout.decode().strip())
        public_hook = hooks_directory / mode
        if not public_hook.is_file() or public_hook.is_symlink() or not os.access(public_hook, os.X_OK):
            raise SystemExit("installed public hook is missing or unsafe: " + mode)
        run_public_cache_negatives(mode, public_hook, records)
        cache_snapshot = filesystem_snapshot(fixed_cache)
        mutant_cache_snapshot = filesystem_snapshot(mutant_cache) if mutant_cache else None

        def run_fixed_hook(witness):
            assert_blob_absent_from_sources(repo, install_root, omitted)
            events.write_text("")
            state_before = snapshot_public_state(repo, common)
            hooks_before = snapshot_hook_state(repo)
            hook_path = public_hook
            if not hook_path.is_file() or hook_path.is_symlink() or not os.access(hook_path, os.X_OK):
                raise SystemExit("installed public hook is missing or unsafe: " + mode)
            assert_blob_absent_from_sources(repo, install_root, omitted)
            hook_result = invoke(witness, ["/bin/bash", str(hook_path)], records)
            state_after = snapshot_public_state(repo, common)
            hook_log = events.read_text() if events.exists() else ""
            if hook_result.returncode != 0:
                raise SystemExit("ordinary checker returned an unexpected status for " + mode + ": " + str(hook_result.returncode))
            if state_after != state_before or snapshot_hook_state(repo) != hooks_before or hooks_before != fixed_hooks:
                raise SystemExit("ordinary checker changed repository or hook state for " + mode)
            if dummy.encode() in hook_result.stdout + hook_result.stderr or observed_metadata_events(hook_log) or "authenticated " in hook_log or "credential-" in hook_log:
                raise SystemExit("ordinary checker received or exposed the credential: " + mode)
            if request_events(hook_log) != ["anonymous blob"] or [line for line in hook_log.splitlines() if line.startswith(("authenticated-oid ", "anonymous-oid "))] != ["anonymous-oid " + omitted]:
                raise SystemExit("ordinary checker did not anonymously hydrate only the intended missing seed blob: " + mode)
            assert_generator_output(hook_result, witness)
            if filesystem_snapshot(qualified_source) != qualified_snapshot or filesystem_snapshot(fixed_cache) != cache_snapshot:
                raise SystemExit("ordinary checker changed its qualified source or copied tool cache: " + mode)
            if mutant_cache and filesystem_snapshot(mutant_cache) != mutant_cache_snapshot:
                raise SystemExit("ordinary checker changed the rebound mutant cache: " + mode)
            assert_blob_absent_from_sources(repo, install_root, omitted)
            return hook_result, hook_log

        if deferred_hook_mutant:
            fixed_result, fixed_log = run_fixed_hook("fixed-" + mode)
            successful_hook_modes.add(mode)
            print("BASELINE_AUTH_FIXED_HOOK_COMPLETE=" + mode, flush=True)
            events.write_text("")
            assert_blob_absent_from_sources(repo, install_root, omitted)
            state_before = snapshot_public_state(repo, common)
            hooks_before = snapshot_hook_state(repo)
            assert_blob_absent_from_sources(repo, install_root, omitted)
            mutant_result = invoke("mutant-" + mode, ["/bin/bash", str(mutant_checker), mode], records)
            state_after = snapshot_public_state(repo, common)
            mutant_log = events.read_text() if events.exists() else ""
            if state_after != state_before or snapshot_hook_state(repo) != hooks_before or hooks_before != fixed_hooks:
                raise SystemExit("mutant checker changed repository or hook state for " + mode)
            if filesystem_snapshot(qualified_source) != qualified_snapshot or filesystem_snapshot(fixed_cache) != cache_snapshot or not mutant_cache or filesystem_snapshot(mutant_cache) != mutant_cache_snapshot:
                raise SystemExit("mutant checker changed its qualified source or copied tool cache: " + mode)
            if hook_mutant_name == "metadata-token-only":
                if not matches_metadata_events(mutant_log, "authenticated") or observed_metadata_events(mutant_log) != expected_metadata_events("authenticated"):
                    raise SystemExit("token-only hook mutant did not reach the exact authenticated locked metadata requests")
                expected_mutant_requests = expected_metadata_events("authenticated")
                if request_events(mutant_log) != expected_mutant_requests + ["anonymous blob"] or [line for line in mutant_log.splitlines() if line.startswith(("authenticated-oid ", "anonymous-oid "))] != ["anonymous-oid " + omitted]:
                    raise SystemExit("token-only hook mutant escaped the approved metadata and anonymous hydration boundaries")
                if mutant_result.returncode != 0:
                    raise SystemExit("token-only hook mutant did not complete the ordinary hook path: " + mode)
                assert_generator_output(mutant_result, "metadata-token-only mutant " + mode)
                if evidence:
                    (evidence / ("metadata-token-only-child-" + mode + ".txt")).write_text(
                        "child_exit=" + str(mutant_result.returncode) + "\n"
                        + "metadata_events=" + str(observed_metadata_events(mutant_log)) + "\n"
                        + "request_events=" + str(request_events(mutant_log)) + "\n"
                        + "generator_output=complete\n"
                        + "child_diagnostic=none\n"
                    )
            else:
                if request_events(mutant_log) != ["authenticated blob"] or [line for line in mutant_log.splitlines() if line.startswith(("authenticated-oid ", "anonymous-oid "))] != ["authenticated-oid " + omitted]:
                    raise SystemExit("hook mutant did not authenticate only the intended missing blob: " + mode)
                if mutant_result.returncode != 0:
                    raise SystemExit("hook mutant did not complete the ordinary hook path: " + mode)
                assert_generator_output(mutant_result, hook_mutant_name + " mutant " + mode)
            if dummy.encode() in mutant_result.stdout + mutant_result.stderr or "credential-in-argv" in mutant_log or "credential-scope-error" in mutant_log or "metadata-capture-error" in mutant_log:
                raise SystemExit("hook mutant exposed its credential outside the authenticated metadata boundary")
            if filesystem_snapshot(qualified_source) != qualified_snapshot or filesystem_snapshot(fixed_cache) != cache_snapshot or filesystem_snapshot(mutant_cache) != mutant_cache_snapshot:
                raise SystemExit("hook mutant modified a cache payload")
            assertion = "BASELINE_AUTH_MUTANT_ASSERTION=" + hook_mutant_name + ":ordinary-hook-credential-isolation"
            print(assertion, flush=True)
            raise SystemExit("ordinary checker received or exposed the credential: " + mode)
        run_fixed_hook(mode)
        successful_hook_modes.add(mode)
        mark_case_assertions("hook-" + mode)
        print("BASELINE_AUTH_FIXED_HOOK_COMPLETE=" + mode, flush=True)
    if selected("wrong-tree"):
        original_checker = checker.read_text()
        original_installer = installer.read_text()
        comparison = '    if [[ "$resolved_seed_tree" != "$trusted_seed_tree" ]]; then'
        if original_checker.count(comparison) != 1:
            raise SystemExit("wrong-tree fixture could not isolate the reviewed tree gate")
        checker.write_text(original_checker.replace(comparison, '    resolved_seed_tree=' + '0' * 40 + '\n' + comparison))
        wrong_tree_digest = hashlib.sha256(checker.read_bytes()).hexdigest()
        installer.write_text(original_installer.replace("reviewed_checker_sha256=" + digest, "reviewed_checker_sha256=" + wrong_tree_digest))
        events.write_text("")
        wrong_tree = invoke("wrong-tree", ["/bin/bash", str(installer)])
        wrong_tree_log = events.read_text() if events.exists() else ""
        if wrong_tree.returncode == 0 or b"approved seed tree does not match the reviewed identity" not in wrong_tree.stderr or "authenticated " in wrong_tree_log:
            raise SystemExit("wrong seed tree was not rejected before authenticated HTTP")
        checker.write_text(original_checker)
        installer.write_text(original_installer)
        if evidence:
            (evidence / "wrong-tree-mutation.txt").write_text("injected resolved_seed_tree=" + "0" * 40 + " immediately before reviewed tree comparison\nchecker_sha256=" + wrong_tree_digest + "\n")
        print("wrong seed tree rejected before authenticated HTTP: PASS")
        mark_case_assertions("wrong-tree")
    seed_cache = install_root / "cache" / "seed"
    for mode, diagnostic in (("header", b"response contains the setup credential"), ("body", b"response contains the setup credential"), ("redirect", b"redirects are not permitted"), ("identity", b"response object identity does not match the requested blob")):
        if not selected("response-" + mode):
            continue
        if seed_cache.exists():
            for path in seed_cache.rglob("*"):
                if not path.is_symlink():
                    path.chmod(path.stat().st_mode | 0o700)
            shutil.rmtree(seed_cache)
        scenario.write_text(mode)
        events.write_text("")
        rejected = invoke("response-" + mode, ["/bin/bash", str(installer)])
        log = events.read_text() if events.exists() else ""
        if rejected.returncode == 0 or diagnostic not in rejected.stderr or dummy.encode() in rejected.stdout + rejected.stderr or "credential-" in log:
            raise SystemExit("authenticated response rejection failed: " + mode)
        if log.count("authenticated blob\n") != 1:
            raise SystemExit("authenticated response escaped the fixed endpoint: " + mode)
        if seed_cache.exists() and any(seed_cache.iterdir()):
            raise SystemExit("rejected response published a seed cache: " + mode)
        for path in install_root.rglob("*"):
            if path.is_file() and not path.is_symlink() and dummy.encode() in path.read_bytes():
                raise SystemExit("credential appeared in the installation: " + mode)
        mark_case_assertions("response-" + mode)
    print("selected credential boundary cases: PASS")
    print("private installed checker fixture sha256:", digest)
    print("authenticated fixed-host hydration and anonymous next boundary: PASS")
    expected_hook_modes = {mode for mode in ("pre-commit", "pre-push") if selected("hook-" + mode)}
    if successful_hook_modes != expected_hook_modes:
        raise SystemExit("selected case did not complete its exact public hook set")
    if selected_case == "all" and (len(all_cold_witnesses) != len(all_cold_cases) or set(all_cold_witnesses) != set(all_cold_cases)):
        raise SystemExit("all-cases did not complete its exact cold witness set")
    if selected_case == "all":
        mark_case_assertions("all")
    finish_case(selected_case)
