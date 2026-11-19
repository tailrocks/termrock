#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
"""Re-freeze (or verify) the generated-workflow trusted seed.

    scripts/refreeze-trusted-seed.py --check
    scripts/refreeze-trusted-seed.py --write [--source REV]
    scripts/refreeze-trusted-seed.py --repin-checker

--check verifies, without writing anything, that every pin of the trusted seed agrees with the
artifacts committed in this checkout: the embedded seed commit, the metadata pack and object list,
the Cargo policy, the baseline fixture pack, and the digests and identities repeated in the
checker, the fixed configuration, the installer, the generator, the fixture builder and the
policy tests. It needs no network and no source history; the seed objects come from the pack.

--write builds the whole seed from one source commit (default HEAD) and rewrites every artifact
and pin, then runs --check on the result:

  1. the seed tree is the source tree plus the untracked sentinel scripts/trusted-seed-cargo.lock
     (the bytes of the source Cargo.lock);
  2. the seed is a parentless commit with a fixed identity and the source commit's own committer
     date, so the same source commit always yields the same seed commit;
  3. the object list, the metadata pack, the baseline fixture pack and the Cargo policy follow from
     that tree and from the exclusion rule that scripts/check-generated-workflows.sh declares;
  4. the pins are rewritten in dependency order: seed identities, then the configuration digest,
     then the checker digest, then everything that repeats the checker digest.

--repin-checker is for an ordinary edit of the checker or the fixed configuration that leaves the
seed alone: it recomputes the configuration and checker digests and rewrites only the pins that
repeat them (checker, installer, auth policy, mutant harness).

--write refuses to run on a dirty tree (any tracked change, or any untracked managed file, including
the sentinel of step 1), and refuses a source commit that is not reachable from the
upstream ref (default origin/main). It is Python standard library only and never touches the network.
The pack bytes are reproducible for one Git and zlib build; the checker verifies the reviewed
bytes, not a re-derivation, so --check never recompresses.
"""
import argparse
import ast
import base64
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

SEED_LOCK_PATH = "scripts/trusted-seed-cargo.lock"
COMMIT_FILE = "scripts/trusted-seed.commit"
OBJECTS_FILE = "scripts/trusted-seed-metadata.objects"
PACK_FILE = "scripts/trusted-seed-metadata.pack"
POLICY_FILE = "scripts/trusted-seed-cargo-policy.json"
FIXTURE_PACK_FILE = "scripts/policy/fixtures/trusted-seed-baselines.pack"
CHECKER = "scripts/check-generated-workflows.sh"
CONFIG = "scripts/generated-workflow-guard-config.toml"
INSTALLER = "scripts/install-generated-workflow-guard.sh"
GENERATOR = "scripts/generate-trusted-seed-cargo-policy.py"
BUILDER = "scripts/policy/build-trusted-seed-fixture.py"
FIXTURE_TEST = "scripts/policy/test_trusted_seed_fixture.py"
AUTH = "scripts/policy/check-generated-workflow-auth.py"
MUTANTS = "scripts/policy/check-generated-workflow-auth-mutants.py"
INSTALLED_TEST = "scripts/policy/check-installed-generated-workflow-guard.sh"
HARNESS = "scripts/policy/check-generated-workflows.sh"

# Everything --write may change. --write rewrites these paths and refuses to start while any of them
# has a tracked change or exists untracked, so the pins are computed from committed bytes only.
MANAGED_PATHS = frozenset({
    COMMIT_FILE, OBJECTS_FILE, PACK_FILE, POLICY_FILE, FIXTURE_PACK_FILE, SEED_LOCK_PATH,
    CHECKER, CONFIG, INSTALLER, GENERATOR, BUILDER, FIXTURE_TEST, AUTH, MUTANTS, INSTALLED_TEST, HARNESS,
})

SEED_SIGNER = "Alexey Zhokhov <alexey@zhokhov.com>"
OID = r"[0-9a-f]{40}"
SHA = r"[0-9a-f]{64}"
MAX_POLICY_BYTES = 262144
MAX_FIXTURE_PACK_BYTES = 1024 * 1024
MAX_OBJECT_LIST_BYTES = 1024 * 1024
MAX_LISTING_BYTES = 8 * 1024 * 1024
PACK_OPTIONS = (
    "-c", "pack.threads=1", "-c", "pack.compression=9", "-c", "pack.window=10", "-c", "pack.depth=50",
    "pack-objects", "--stdout", "--threads=1", "--no-reuse-delta", "--no-reuse-object",
)

# key, file, regex with exactly one group around the pinned value, minimum match count.
PIN_SITES = (
    ("seed_commit", CHECKER, rf"(?m)^trusted_seed_commit=({OID})$", 1),
    ("seed_tree", CHECKER, rf"(?m)^trusted_seed_tree=({OID})$", 1),
    ("lock_sha256", CHECKER, rf"(?m)^trusted_lock_sha256=({SHA})$", 1),
    ("policy_sha256", CHECKER, rf"(?m)^trusted_seed_policy_sha256=({SHA})$", 1),
    ("manifests_sha256", CHECKER, rf"(?m)^trusted_manifests_sha256=({SHA})$", 1),
    ("pack_sha256", CHECKER, rf"(?m)^trusted_metadata_pack_sha256=({SHA})$", 1),
    ("objects_sha256", CHECKER, rf"(?m)^trusted_metadata_object_list_sha256=({SHA})$", 1),
    ("config_sha256", CHECKER, rf"(?m)^expected_config_sha256=({SHA})$", 1),
    ("seed_commit", CONFIG, rf'(?m)^trusted_seed_commit = "({OID})"$', 1),
    ("seed_tree", CONFIG, rf'(?m)^trusted_seed_tree = "({OID})"$', 1),
    ("lock_sha256", CONFIG, rf'(?m)^trusted_lock_sha256 = "({SHA})"$', 1),
    ("pack_sha256", CONFIG, rf'(?m)^trusted_metadata_pack_sha256 = "({SHA})"$', 1),
    ("objects_sha256", CONFIG, rf'(?m)^trusted_metadata_object_list_sha256 = "({SHA})"$', 1),
    ("policy_sha256", CONFIG, rf'(?m)^trusted_seed_cargo_policy_sha256 = "({SHA})"$', 1),
    ("seed_commit", GENERATOR, rf'(?m)^SEED_COMMIT = "({OID})"$', 1),
    ("seed_tree", GENERATOR, rf'(?m)^SEED_TREE = "({OID})"$', 1),
    ("manifests_sha256", GENERATOR, rf'(?m)^EXPECTED_AGGREGATE = "({SHA})"$', 1),
    ("checker_sha256", INSTALLER, rf"(?m)^reviewed_checker_sha256=({SHA})$", 1),
    ("config_sha256", INSTALLER, rf"(?m)^reviewed_config_sha256=({SHA})$", 1),
    ("seed_commit", INSTALLER, rf"(?m)^reviewed_seed_commit=({OID})$", 1),
    ("seed_tree", INSTALLER, rf"(?m)^reviewed_seed_tree=({OID})$", 1),
    ("lock_sha256", INSTALLER, rf"(?m)^reviewed_seed_cargo_lock_sha256=({SHA})$", 1),
    ("policy_sha256", INSTALLER, rf"(?m)^reviewed_seed_cargo_policy_sha256=({SHA})$", 1),
    ("pack_sha256", INSTALLER, rf"(?m)^reviewed_metadata_pack_sha256=({SHA})$", 1),
    ("objects_sha256", INSTALLER, rf"(?m)^reviewed_metadata_object_list_sha256=({SHA})$", 1),
    ("checker_sha256", AUTH, rf'(?m)^qualified_checker_sha256 = "({SHA})"$', 1),
    ("seed_commit", AUTH, rf'(?m)^qualified_seed_commit = "({OID})"$', 1),
    ("seed_tree", AUTH, rf'(?m)^qualified_seed_tree = "({OID})"$', 1),
    ("seed_commit", AUTH, rf'"seed_commit": "({OID})"', 1),
    ("pack_sha256", AUTH, rf'metadata_pack\.read_bytes\(\)\)\.hexdigest\(\) != "({SHA})"', 1),
    ("objects_sha256", AUTH, rf'metadata_objects\.read_bytes\(\)\)\.hexdigest\(\) != "({SHA})"', 1),
    ("checker_sha256", MUTANTS, rf'(?m)^expected_checker = "({SHA})"$', 1),
    ("seed_commit", INSTALLED_TEST, rf"cache/seed/({OID})/", 3),
    ("seed_commit", INSTALLED_TEST, rf"'({OID})\^\{{commit\}}'", 2),
    ("seed_commit", INSTALLED_TEST, rf'hexdigest\(\) != "({OID})"', 1),
    ("lock_sha256", INSTALLED_TEST, rf'"\$policy_seed_sha" != ({SHA}) \]', 1),
    ("policy_sha256", INSTALLED_TEST, rf'"\$policy_manifest_sha" != ({SHA}) \]', 1),
    ("lock_sha256", HARNESS, rf'hexdigest\(\) != "({SHA})":\n    raise SystemExit\("trusted lock identity mismatch"\)', 1),
    ("baseline_pack_sha256", BUILDER, rf'(?m)^BASELINE_PACK_SHA256 = "({SHA})"$', 1),
    ("baseline_count", BUILDER, r"(?m)^EXPECTED_BASELINE_EXCLUSIONS = (\d+)$", 1),
    ("absent_count", BUILDER, r"(?m)^EXPECTED_ABSENT_EXCLUSIONS = (\d+)$", 1),
    ("baseline_pack_sha256", FIXTURE_TEST, rf"(?m)^DIGEST = '({SHA})'$", 1),
)
# A key's stage is the order its value can be known: seed identities first, then the digest of the
# configuration that repeats them, then the digest of the checker that repeats both.
STAGE_OF_KEY = {
    "seed_commit": 1, "seed_tree": 1, "lock_sha256": 1, "policy_sha256": 1, "manifests_sha256": 1,
    "pack_sha256": 1, "objects_sha256": 1, "baseline_pack_sha256": 1, "baseline_count": 1, "absent_count": 1,
    "config_sha256": 2, "checker_sha256": 3,
}

# Pins that only a real installer qualification run can produce. --check lists them and does not
# judge them: they bind a warm tool cache, a recorded fixture head and recorded receipts.
EXTERNAL_PINS = (
    (AUTH, "qualified_cache_root, qualified_cache_generation"),
    (AUTH, "qualified_source_manifest_sha256, qualified_payload_sha256, qualified_manifest_key"),
    (AUTH, "qualified_fixture_head, qualified_cache_receipts"),
    (AUTH, 'velnor "0.1.78" and jactionlint "2.4.0" in the qualified receipt, seed tool tuple and fixture mise.toml checks'),
)


class Failure(Exception):
    pass


def fail(message):
    raise Failure(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git_object_id(kind, data):
    return hashlib.sha1(kind.encode() + b" " + str(len(data)).encode() + b"\0" + data).hexdigest()


class Git:
    """Git with a private HOME, no user or system config, and replace refs disabled."""

    def __init__(self, home):
        executable = shutil.which("git")
        if executable is None:
            fail("Git is required")
        self.executable = executable
        self.home = str(home)

    def environment(self, extra=None):
        environment = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": self.home,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_SYSTEM": os.devnull,
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0",
            "LC_ALL": "C",
            "LANG": "C",
        }
        environment.update(extra or {})
        return environment

    def run(self, location, *arguments, data=None, extra=None, check=True):
        flag = "--git-dir" if str(location).endswith(".git") else "-C"
        process = subprocess.run(
            [self.executable, flag, str(location), *arguments],
            input=data,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=self.environment(extra),
        )
        if check and process.returncode != 0:
            detail = process.stderr.decode("utf-8", "backslashreplace").strip()
            fail(f"git {' '.join(arguments[:3])} failed: {detail}")
        return process.stdout if check else process


class Rules:
    """The trusted-seed exclusion rule, read from the one place the checker declares it."""

    def __init__(self, checker_text):
        values = {}
        for name in ("baseline_prefixes", "artifact_prefixes", "artifact_paths"):
            found = re.findall(rf"(?m)^trusted_seed_excluded_{name}='([^'\n]*)'$", checker_text)
            if len(found) != 1:
                fail(f"checker does not declare one trusted_seed_excluded_{name}")
            values[name] = tuple(found[0].split())
        if not values["baseline_prefixes"] or not values["artifact_prefixes"]:
            fail("checker exclusion rule has an empty prefix list")
        if not all(item.endswith("/") for item in values["baseline_prefixes"] + values["artifact_prefixes"]):
            fail("checker exclusion prefixes must end with a slash")
        self.baseline_prefixes = values["baseline_prefixes"]
        self.artifact_prefixes = values["artifact_prefixes"]
        self.artifact_paths = frozenset(values["artifact_paths"])

    def classify(self, path, mode, kind):
        """None (kept in the pack), "baseline" (kept only in the fixture) or "absent" (in neither).

        Mirrors the projection in scripts/check-generated-workflows.sh: a baseline entry must be a
        regular 100644 blob and an absent entry a regular 100644 or 100755 blob. Any other mode under
        those paths is refused, not classified, so a symlink there cannot pass as either class.
        """
        for prefix in self.baseline_prefixes:
            if path.startswith(prefix) and "/" not in path[len(prefix):] and path[len(prefix):].endswith(".tui.pb"):
                if (mode, kind) != ("100644", "blob"):
                    fail(f"seed tree baseline entry is not a regular file: {mode} {kind} {path}")
                return "baseline"
        if path in self.artifact_paths or any(path.startswith(prefix) for prefix in self.artifact_prefixes):
            if (mode, kind) not in {("100644", "blob"), ("100755", "blob")}:
                fail(f"seed tree excluded artifact is not a regular file: {mode} {kind} {path}")
            return "absent"
        return None


class Tree:
    """A listed seed tree: every entry with its exclusion class, and the objects each class needs."""

    def __init__(self, git, location, root_tree, rules):
        listing = git.run(location, "ls-tree", "-r", "-t", "-z", "--full-tree", root_tree)
        if len(listing) > MAX_LISTING_BYTES or not listing.endswith(b"\0"):
            fail("seed tree listing is truncated or oversized")
        self.root_tree = root_tree
        self.entries = []
        self.object_types = {root_tree: "tree"}
        for raw in listing[:-1].split(b"\0"):
            try:
                header, path_bytes = raw.split(b"\t", 1)
                mode, kind, oid = header.decode("ascii").split(" ")
                path = path_bytes.decode("utf-8", "strict")
            except (ValueError, UnicodeError):
                fail("seed tree has a malformed entry")
            parts = path.split("/")
            if not path or path.startswith("/") or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts) or any(ord(ch) < 32 or ord(ch) == 127 for ch in path):
                fail("seed tree has an unsafe path")
            if re.fullmatch(OID, oid) is None:
                fail("seed tree has a malformed object identity")
            if (mode, kind) == ("040000", "tree"):
                object_type, exclusion = "tree", None
            elif (mode, kind) in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
                object_type, exclusion = "blob", rules.classify(path, mode, kind)
            else:
                fail(f"seed tree has an unsupported entry: {mode} {kind} {path}")
            if self.object_types.setdefault(oid, object_type) != object_type:
                fail("seed tree gives one object identity two types")
            self.entries.append((mode, kind, oid, path, exclusion))
        kept = {oid for _, _, oid, _, exclusion in self.entries if exclusion is None}
        baseline = {oid for _, _, oid, _, exclusion in self.entries if exclusion == "baseline"}
        absent = {oid for _, _, oid, _, exclusion in self.entries if exclusion == "absent"}
        self.baseline_only = baseline - kept
        self.absent_only = absent - kept - baseline
        self.baseline_count = sum(1 for entry in self.entries if entry[4] == "baseline")
        self.absent_count = sum(1 for entry in self.entries if entry[4] == "absent")
        self.blobs = {path: (mode, oid) for mode, kind, oid, path, _ in self.entries if kind == "blob"}

    def pack_objects(self, commit):
        """The reviewed metadata object set: the commit, the root tree, every tree and kept blob."""
        result = {commit, self.root_tree}
        result.update(oid for oid, kind in self.object_types.items() if kind == "tree")
        result.update(oid for _, kind, oid, _, exclusion in self.entries if kind == "blob" and exclusion is None)
        return result

    def expected_types(self, commit, objects):
        return {oid: ("commit" if oid == commit else self.object_types[oid]) for oid in objects}


def parse_inventory(raw):
    inventory = {}
    for line in raw.decode("ascii").splitlines():
        fields = line.split()
        if len(fields) != 3 or fields[1] not in {"commit", "tree", "blob"} or not fields[2].isdigit() or fields[0] in inventory:
            fail("object inventory is malformed")
        inventory[fields[0]] = (fields[1], int(fields[2]))
    return inventory


def bare_repository(git, parent, name, alternate=None):
    location = Path(parent) / f"{name}.git"
    git.run(parent, "init", "--bare", "--quiet", "--object-format=sha1", str(location))
    if alternate is not None:
        (location / "objects" / "info").mkdir(parents=True, exist_ok=True)
        (location / "objects" / "info" / "alternates").write_text(f"{alternate}\n")
    return location


def index_pack(git, location, pack_bytes):
    git.run(location, "index-pack", "--stdin", data=pack_bytes)
    return parse_inventory(git.run(location, "cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype) %(objectsize)"))


def read_text(root, relative):
    path = root / relative
    if path.is_symlink() or not path.is_file():
        fail(f"{relative} is missing or not a regular file")
    return path.read_text(encoding="utf-8")


def read_bytes(root, relative, limit=None):
    path = root / relative
    if path.is_symlink() or not path.is_file():
        fail(f"{relative} is missing or not a regular file")
    data = path.read_bytes()
    if limit is not None and len(data) > limit:
        fail(f"{relative} exceeds its {limit} byte bound")
    return data


def parse_bounds(checker_text, installer_text):
    def find(pattern, text, label):
        found = re.findall(pattern, text)
        if len(found) != 1:
            fail(f"cannot read the {label} bound")
        return int(found[0])

    return {
        "entries": find(r"(?m)^maximum_trusted_seed_metadata_entries=(\d+)$", checker_text, "seed metadata entry"),
        "blob": find(r"(?m)^maximum_snapshot_blob_bytes=\$\(\((\d+) \* 1024 \* 1024\)\)$", checker_text, "per-blob") * 1024 * 1024,
        "total": find(r"(?m)^maximum_snapshot_total_bytes=\$\(\((\d+) \* 1024 \* 1024\)\)$", checker_text, "total blob") * 1024 * 1024,
        "pack": find(r"trusted-seed-metadata\.pack\) limit=(\d+) ;;", installer_text, "installer pack"),
    }


def reviewed_tool_table(checker_text):
    found = re.findall(r"(?ms)^supported = (\{.*?^\})$", checker_text)
    if len(found) != 1:
        fail("checker has no single reviewed tool tuple table")
    table = ast.literal_eval(found[0])
    if not table or any(set(row) != {"rust", "jactionlint", "shellcheck"} for row in table.values()):
        fail("checker tool tuple table is malformed")
    return table


def seed_tool_tuple(mise_text):
    tools = tomllib.loads(mise_text).get("tools", {})

    def version(name):
        value = tools.get(name)
        return value.get("version") if isinstance(value, dict) else value

    return version("github:tailrocks/velnor-new"), {name: version(name) for name in ("rust", "jactionlint", "shellcheck")}


def canonical_policy(policy):
    return (json.dumps(policy, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n").encode("ascii")


def manifest_aggregate(records):
    digest = hashlib.sha256()
    for path, file_digest in records:
        digest.update(path.encode("utf-8") + b"\0" + file_digest.encode("ascii") + b"\n")
    return digest.hexdigest()


def derive_facts(root, git, scratch, log):
    """Verify the committed seed artifacts against each other and return the pinned values."""
    checker_text = read_text(root, CHECKER)
    installer_text = read_text(root, INSTALLER)
    rules = Rules(checker_text)
    bounds = parse_bounds(checker_text, installer_text)

    commit_bytes = read_bytes(root, COMMIT_FILE, 64 * 1024)
    commit = git_object_id("commit", commit_bytes)
    header, _, message = commit_bytes.partition(b"\n\n")
    header_lines = header.decode("utf-8").splitlines()
    if not header_lines or re.fullmatch(rf"tree ({OID})", header_lines[0]) is None or any(line.startswith("parent ") for line in header_lines):
        fail("embedded seed commit is not a parentless commit with a leading tree line")
    tree = header_lines[0].split()[1]
    if not re.search(rb"(?m)^Signed-off-by: [^<\n]+ <[^>\n]+>$", message):
        fail("embedded seed commit has no sign-off")
    log("seed commit", commit)

    pack = read_bytes(root, PACK_FILE, bounds["pack"])
    objects_bytes = read_bytes(root, OBJECTS_FILE, MAX_OBJECT_LIST_BYTES)
    object_list = objects_bytes.decode("ascii").splitlines()
    if object_list != sorted(set(object_list)) or len(object_list) > bounds["entries"] + 2 or any(re.fullmatch(OID, item) is None for item in object_list):
        fail("object list is unsorted, duplicated, malformed or over its entry bound")

    repository = bare_repository(git, scratch, "metadata")
    inventory = index_pack(git, repository, pack)
    if inventory.get(commit, (None,))[0] != "commit":
        fail("metadata pack omits the seed commit")
    if git.run(repository, "rev-parse", "--verify", f"{commit}^{{tree}}").decode().strip() != tree:
        fail("metadata pack commit tree differs from the embedded commit")
    if git.run(repository, "cat-file", "commit", commit) != commit_bytes:
        fail("metadata pack commit bytes differ from the embedded commit")

    listed = Tree(git, repository, tree, rules)
    if len(listed.entries) > bounds["entries"]:
        fail(f"seed tree has {len(listed.entries)} entries, over the {bounds['entries']} bound")
    expected = listed.pack_objects(commit)
    if set(object_list) != expected:
        fail("object list differs from the commit, root tree, trees and kept blobs of the seed tree")
    if set(inventory) != expected:
        fail("metadata pack objects differ from the object list")
    expected_types = listed.expected_types(commit, expected)
    if any(inventory[oid][0] != kind for oid, kind in expected_types.items()):
        fail("metadata pack object types differ from the seed tree")
    blob_total = 0
    for oid, (kind, size) in inventory.items():
        if kind == "blob":
            if size > bounds["blob"]:
                fail(f"metadata pack blob {oid} is {size} bytes, over the {bounds['blob']} bound")
            blob_total += size
    if blob_total > bounds["total"]:
        fail("metadata pack blobs exceed the total byte bound")
    log("metadata pack", f"{len(pack)} bytes, {len(inventory)} objects, {blob_total} blob bytes")

    table = reviewed_tool_table(checker_text)
    if "mise.toml" not in listed.blobs:
        fail("seed tree has no mise.toml")
    velnor, tuple_ = seed_tool_tuple(git.run(repository, "cat-file", "blob", listed.blobs["mise.toml"][1]).decode("utf-8"))
    if table.get(velnor) != tuple_:
        fail(f"seed tree tool tuple (velnor {velnor}, {tuple_}) is not a reviewed tuple of the checker")
    log("tool tuple", f"seed velnor {velnor} {tuple_}; checker admits Velnor {', '.join(sorted(table))}")

    if SEED_LOCK_PATH not in listed.blobs or "Cargo.lock" not in listed.blobs or listed.blobs[SEED_LOCK_PATH][1] != listed.blobs["Cargo.lock"][1]:
        fail("seed tree lacks the lock sentinel or it differs from the root Cargo.lock")
    lock_bytes = git.run(repository, "cat-file", "blob", listed.blobs[SEED_LOCK_PATH][1])
    lock = tomllib.loads(lock_bytes.decode("utf-8"))
    packages = lock["package"]
    local_packages = sorted((package["name"], package["version"]) for package in packages if "source" not in package)
    sentinel = root / SEED_LOCK_PATH
    sentinel_state = "absent (scripts/materialize-trusted-seed-cargo-lock.sh restores it)"
    if sentinel.exists() or sentinel.is_symlink():
        if sentinel.is_symlink() or not sentinel.is_file() or sentinel.read_bytes() != lock_bytes:
            fail("the untracked sentinel scripts/trusted-seed-cargo.lock differs from the seed tree lock")
        sentinel_state = "present, matches the seed tree"

    policy_bytes = read_bytes(root, POLICY_FILE, MAX_POLICY_BYTES)
    policy = json.loads(policy_bytes.decode("ascii"))
    if canonical_policy(policy) != policy_bytes:
        fail("Cargo policy is not in its canonical encoding")
    if set(policy) != {"schema", "seed_commit", "seed_tree", "manifest_aggregate_sha256", "manifests", "lock_package_count", "local_package_count", "local_packages", "nested_locks"} or policy["schema"] != 2:
        fail("Cargo policy has unsupported fields")
    if policy["seed_commit"] != commit or policy["seed_tree"] != tree:
        fail("Cargo policy names another seed")
    manifest_paths = sorted(path for path in listed.blobs if path.rsplit("/", 1)[-1] == "Cargo.toml")
    if [record["path"] for record in policy["manifests"]] != manifest_paths or "Cargo.toml" not in manifest_paths:
        fail("Cargo policy manifest set differs from the Cargo.toml files of the seed tree")
    records = []
    for record in policy["manifests"]:
        mode, oid = listed.blobs[record["path"]]
        data = git.run(repository, "cat-file", "blob", oid)
        if mode != "100644" or base64.b64decode(record["bytes_base64"], validate=True) != data or record["sha256"] != sha256(data):
            fail(f"Cargo policy manifest bytes differ from the seed tree: {record['path']}")
        records.append((record["path"], record["sha256"]))
    aggregate = manifest_aggregate(records)
    if policy["manifest_aggregate_sha256"] != aggregate:
        fail("Cargo policy manifest aggregate is wrong")
    nested = sorted(path for path in listed.blobs if path.rsplit("/", 1)[-1] == "Cargo.lock" and path != "Cargo.lock")
    if [item["path"] for item in policy["nested_locks"]] != nested or any(
        item["sha256"] != sha256(git.run(repository, "cat-file", "blob", listed.blobs[item["path"]][1])) for item in policy["nested_locks"]
    ):
        fail("Cargo policy nested lock set or digests differ from the seed tree")
    if policy["lock_package_count"] != len(packages) or policy["local_package_count"] != len(local_packages) or policy["local_packages"] != [
        {"name": name, "version": version} for name, version in local_packages
    ]:
        fail("Cargo policy lock inventory differs from the seed tree lock")
    log("cargo policy", f"{len(policy_bytes)} bytes, {len(manifest_paths)} manifests, {len(nested)} nested locks, "
        f"{len(packages)} lock packages, {len(local_packages)} local")

    fixture = read_bytes(root, FIXTURE_PACK_FILE, MAX_FIXTURE_PACK_BYTES)
    fixture_inventory = index_pack(git, bare_repository(git, scratch, "baselines"), fixture)
    if set(fixture_inventory) != listed.baseline_only or any(kind != "blob" for kind, _ in fixture_inventory.values()):
        fail("baseline fixture pack objects differ from the baseline-only blobs of the seed tree")
    log("baseline fixture", f"{len(fixture)} bytes, {len(fixture_inventory)} objects, {listed.baseline_count} baseline entries, "
        f"{listed.absent_count} entries absent from every pack")

    return {
        "seed_commit": commit,
        "seed_tree": tree,
        "lock_sha256": sha256(lock_bytes),
        "policy_sha256": sha256(policy_bytes),
        "manifests_sha256": aggregate,
        "pack_sha256": sha256(pack),
        "objects_sha256": sha256(objects_bytes),
        "baseline_pack_sha256": sha256(fixture),
        "baseline_count": str(listed.baseline_count),
        "absent_count": str(listed.absent_count),
        "config_sha256": sha256(read_bytes(root, CONFIG)),
        "checker_sha256": sha256(read_bytes(root, CHECKER)),
        "_sentinel": sentinel_state,
        "_pack_bytes": len(pack),
        "_pack_bound": bounds["pack"],
        "_blob_bound": bounds["blob"],
    }


def check_pins(root, facts):
    """Compare every pin site and the fixed configuration with the derived facts."""
    problems = []
    config = tomllib.loads(read_text(root, CONFIG))
    for key, name in (
        ("seed_commit", "trusted_seed_commit"), ("seed_tree", "trusted_seed_tree"), ("lock_sha256", "trusted_lock_sha256"),
        ("pack_sha256", "trusted_metadata_pack_sha256"), ("objects_sha256", "trusted_metadata_object_list_sha256"),
        ("policy_sha256", "trusted_seed_cargo_policy_sha256"),
    ):
        if config.get(name) != facts[key]:
            problems.append(f"{CONFIG}: {name} is {config.get(name)}, expected {facts[key]}")
    texts = {}
    results = []
    for key, relative, pattern, minimum in PIN_SITES:
        text = texts.setdefault(relative, read_text(root, relative))
        found = re.findall(pattern, text)
        status = "ok"
        if len(found) < minimum:
            status = f"found {len(found)} site(s), expected at least {minimum}"
            problems.append(f"{relative}: pin {key}: {status}")
        elif any(value != facts[key] for value in found):
            wrong = sorted({value for value in found if value != facts[key]})
            status = f"has {wrong[0]}, expected {facts[key]}"
            problems.append(f"{relative}: pin {key}: {status}")
        results.append((key, relative, len(found), status))
    return problems, results


def check(root, git, scratch, log):
    facts = derive_facts(root, git, scratch, log)
    if facts["_pack_bytes"] > facts["_pack_bound"]:
        fail(f"metadata pack is {facts['_pack_bytes']} bytes, over the installer bound {facts['_pack_bound']}")
    # The pack is itself a tracked blob, so every later snapshot must be able to carry it.
    if facts["_pack_bytes"] > facts["_blob_bound"]:
        fail(f"metadata pack is {facts['_pack_bytes']} bytes, over the {facts['_blob_bound']} byte per-blob snapshot bound")
    problems, results = check_pins(root, facts)
    for key, relative, count, status in results:
        log(f"pin {key}", f"{relative} x{count}: {status}")
    log("sentinel", facts["_sentinel"])
    for relative, description in EXTERNAL_PINS:
        log("external pin (not judged)", f"{relative}: {description}")
    if problems:
        for problem in problems:
            print(f"FAIL {problem}", file=sys.stderr)
        fail(f"{len(problems)} pin(s) disagree with the committed seed artifacts")
    return facts


def apply_sites(root, facts, stage):
    changed = set()
    for key, relative, pattern, _ in PIN_SITES:
        if STAGE_OF_KEY[key] != stage:
            continue
        path = root / relative
        text = path.read_text(encoding="utf-8")
        new_text, count = re.subn(pattern, lambda match: match.group(0).replace(match.group(1), facts[key], 1), text)
        if count == 0:
            fail(f"{relative}: no site for pin {key}")
        if new_text != text:
            path.write_text(new_text, encoding="utf-8")
            changed.add(relative)
    return changed


def write_file(root, relative, data, mode=0o644):
    path = root / relative
    if path.is_symlink() or (path.exists() and not path.is_file()):
        fail(f"{relative} is not a regular file")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".refreeze-tmp")
    temporary.write_bytes(data)
    os.chmod(temporary, mode)
    os.replace(temporary, path)


def build_seed(root, git, scratch, source, signer, message, log):
    """Create the seed tree and parentless commit in a scratch repository that borrows the source objects."""
    objects_dir = git.run(root, "rev-parse", "--path-format=absolute", "--git-path", "objects").decode().strip()
    repository = bare_repository(git, scratch, "seed", alternate=objects_dir)
    index = Path(scratch) / "seed.index"
    index_env = {"GIT_INDEX_FILE": str(index)}
    git.run(repository, "read-tree", source, extra=index_env)
    tracked = git.run(repository, "ls-tree", "--name-only", source, "--", SEED_LOCK_PATH).decode().strip()
    if tracked:
        fail(f"{SEED_LOCK_PATH} is tracked in the source commit; it is deliberately untracked")
    lock_blob = git.run(repository, "rev-parse", "--verify", f"{source}:Cargo.lock").decode().strip()
    git.run(repository, "update-index", "--add", "--cacheinfo", f"100644,{lock_blob},{SEED_LOCK_PATH}", extra=index_env)
    tree = git.run(repository, "write-tree", extra=index_env).decode().strip()
    date = git.run(repository, "show", "-s", "--format=%cd", "--date=raw", source).decode().strip()
    name, email = re.fullmatch(r"(.+) <([^<>]+)>", signer).groups()
    message_file = Path(scratch) / "seed-message.txt"
    message_file.write_text(message, encoding="utf-8")
    identity = {
        "GIT_AUTHOR_NAME": name, "GIT_AUTHOR_EMAIL": email, "GIT_AUTHOR_DATE": date,
        "GIT_COMMITTER_NAME": name, "GIT_COMMITTER_EMAIL": email, "GIT_COMMITTER_DATE": date,
    }
    commit = git.run(repository, "-c", "commit.gpgsign=false", "commit-tree", tree, "-F", str(message_file), extra=identity).decode().strip()
    body = git.run(repository, "cat-file", "commit", commit)
    if git_object_id("commit", body) != commit or re.search(rb"(?m)^parent ", body.split(b"\n\n", 1)[0]):
        fail("seed commit is not a parentless commit with a matching identity")
    log("seed tree", f"{tree} (source tree {git.run(repository, 'rev-parse', source + '^{tree}').decode().strip()} + sentinel {lock_blob})")
    log("seed commit", f"{commit}, dated {date}")
    return repository, commit, tree, body, lock_blob


def default_message(source, signer):
    return (
        "chore(ci): freeze post-split generator guard seed\n\n"
        f"Source: {source}\n"
        "Task: re-freeze the generated-workflow trusted seed (#509)\n"
        f"Signed-off-by: {signer}\n"
    )


def dirty_problems(root, git):
    """Every tracked change, and every managed path that exists without being tracked."""
    problems = git.run(root, "status", "--porcelain", "--untracked-files=no").decode().splitlines()
    for relative in sorted(MANAGED_PATHS):
        path = root / relative
        if path.exists() or path.is_symlink():
            if git.run(root, "ls-files", "--error-unmatch", "--", relative, check=False).returncode != 0:
                problems.append(f"?? {relative} (untracked managed file)")
    return problems


def write(root, git, scratch, arguments, log):
    problems = dirty_problems(root, git)
    if problems:
        fail("--write refuses a dirty tree; commit or remove these first:\n  " + "\n  ".join(problems))
    source = git.run(root, "rev-parse", "--verify", f"{arguments.source}^{{commit}}").decode().strip()
    if not arguments.no_upstream_check:
        result = git.run(root, "merge-base", "--is-ancestor", source, arguments.upstream, check=False)
        if result.returncode != 0:
            fail(f"source {source} is not reachable from {arguments.upstream}; pass --no-upstream-check only for a dry run")
    log("source", source)
    signer = SEED_SIGNER
    message = Path(arguments.message).read_text(encoding="utf-8") if arguments.message else default_message(source, signer)
    if f"Signed-off-by: {signer}" not in message.splitlines():
        fail("the seed commit message must carry the signer's Signed-off-by line")

    checker_text = read_text(root, CHECKER)
    rules = Rules(checker_text)
    bounds = parse_bounds(checker_text, read_text(root, INSTALLER))
    repository, commit, tree, commit_body, _ = build_seed(root, git, scratch, source, signer, message, log)
    listed = Tree(git, repository, tree, rules)
    if len(listed.entries) > bounds["entries"]:
        fail(f"seed tree has {len(listed.entries)} entries, over the {bounds['entries']} bound")
    objects = sorted(listed.pack_objects(commit))
    if len(objects) > bounds["entries"] + 2:
        fail("object list exceeds the entry bound")
    object_bytes = "".join(f"{oid}\n" for oid in objects).encode("ascii")
    pack = git.run(repository, *PACK_OPTIONS, data=object_bytes)
    if len(pack) > bounds["pack"]:
        fail(f"metadata pack is {len(pack)} bytes, over the installer bound {bounds['pack']}; widen the exclusion rule")
    fixture = git.run(repository, *PACK_OPTIONS, data="".join(f"{oid}\n" for oid in sorted(listed.baseline_only)).encode("ascii"))
    if len(fixture) > MAX_FIXTURE_PACK_BYTES:
        fail(f"baseline fixture pack is {len(fixture)} bytes, over its {MAX_FIXTURE_PACK_BYTES} bound")
    log("metadata pack", f"{len(pack)} bytes for {len(objects)} objects (installer bound {bounds['pack']})")
    log("baseline fixture", f"{len(fixture)} bytes for {len(listed.baseline_only)} objects")

    write_file(root, COMMIT_FILE, commit_body)
    write_file(root, OBJECTS_FILE, object_bytes)
    write_file(root, PACK_FILE, pack)
    write_file(root, FIXTURE_PACK_FILE, fixture)
    write_file(root, SEED_LOCK_PATH, git.run(repository, "cat-file", "blob", listed.blobs[SEED_LOCK_PATH][1]))

    manifests = []
    for path in sorted(path for path in listed.blobs if path.rsplit("/", 1)[-1] == "Cargo.toml"):
        manifests.append((path, sha256(git.run(repository, "cat-file", "blob", listed.blobs[path][1]))))
    aggregate = manifest_aggregate(manifests)

    seed_facts = {
        "seed_commit": commit, "seed_tree": tree, "manifests_sha256": aggregate,
        "baseline_pack_sha256": sha256(fixture), "baseline_count": str(listed.baseline_count), "absent_count": str(listed.absent_count),
        "pack_sha256": sha256(pack), "objects_sha256": sha256(object_bytes),
        "lock_sha256": sha256(git.run(repository, "cat-file", "blob", listed.blobs[SEED_LOCK_PATH][1])),
    }
    # The generator re-derives the policy from the seed repository and refuses unless its own
    # three pins equal what was just computed, so the aggregate is confirmed independently.
    patch_generator(root, seed_facts)
    output = Path(scratch) / "policy-output"
    output.mkdir()
    process = subprocess.run(
        [sys.executable, "-I", str(root / GENERATOR), str(repository), "trusted-seed-cargo-policy.json"],
        cwd=output, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LC_ALL": "C"},
    )
    if process.returncode != 0:
        fail("policy generator failed: " + process.stderr.decode("utf-8", "backslashreplace").strip())
    policy_bytes = (output / "trusted-seed-cargo-policy.json").read_bytes()
    write_file(root, POLICY_FILE, policy_bytes)
    seed_facts["policy_sha256"] = sha256(policy_bytes)
    log("cargo policy", f"{len(policy_bytes)} bytes, digest {seed_facts['policy_sha256']}")

    # Stage 1 values are known; the configuration digest depends on them, the checker digest on both.
    facts = dict(seed_facts)
    apply_sites(root, facts, 1)
    facts["config_sha256"] = sha256(read_bytes(root, CONFIG))
    apply_sites(root, facts, 2)
    facts["checker_sha256"] = sha256(read_bytes(root, CHECKER))
    apply_sites(root, facts, 3)
    log("config digest", facts["config_sha256"])
    log("checker digest", facts["checker_sha256"])
    log("git", subprocess.run([git.executable, "--version"], stdout=subprocess.PIPE, check=True).stdout.decode().strip())


def patch_generator(root, seed_facts):
    path = root / GENERATOR
    text = path.read_text(encoding="utf-8")
    for key, name in (("seed_commit", "SEED_COMMIT"), ("seed_tree", "SEED_TREE"), ("manifests_sha256", "EXPECTED_AGGREGATE")):
        text, count = re.subn(rf'(?m)^{name} = "[0-9a-f]+"$', f'{name} = "{seed_facts[key]}"', text)
        if count != 1:
            fail(f"generator has no single {name} pin")
    path.write_text(text, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0], formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="verify the pins against the committed artifacts; write nothing")
    mode.add_argument("--write", action="store_true", help="rebuild every seed artifact and pin from --source")
    mode.add_argument("--repin-checker", action="store_true", help="rewrite only the configuration and checker digest pins")
    parser.add_argument("--repo", default=str(Path(__file__).resolve().parents[1]), help="checkout to verify or rewrite")
    parser.add_argument("--source", default="HEAD", help="--write: commit whose tree becomes the seed tree (default HEAD)")
    parser.add_argument("--upstream", default="origin/main", help="--write: ref that must contain --source")
    parser.add_argument("--no-upstream-check", action="store_true", help="--write: skip the upstream containment check (dry runs only)")
    parser.add_argument("--message", help="--write: file with the seed commit message (must carry the signer's sign-off)")
    arguments = parser.parse_args()
    root = Path(arguments.repo).resolve(strict=True)

    def log(label, value):
        print(f"{label}: {value}")

    with tempfile.TemporaryDirectory(prefix="refreeze-trusted-seed.") as scratch:
        home = Path(scratch) / "home"
        home.mkdir()
        git = Git(home)
        try:
            if arguments.repin_checker:
                facts = {"config_sha256": sha256(read_bytes(root, CONFIG))}
                apply_sites(root, facts, 2)
                facts["checker_sha256"] = sha256(read_bytes(root, CHECKER))
                apply_sites(root, facts, 3)
                log("config digest", facts["config_sha256"])
                log("checker digest", facts["checker_sha256"])
                return 0
            if arguments.write:
                write(root, git, scratch, arguments, log)
                print("-- verifying the rewritten checkout")
            check(root, git, scratch, log)
        except Failure as error:
            print(f"refreeze-trusted-seed: {error}", file=sys.stderr)
            return 1
    print("PASS: every trusted-seed pin agrees with the committed artifacts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
