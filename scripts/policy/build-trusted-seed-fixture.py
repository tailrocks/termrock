#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import os
import re
import shutil
import stat
import subprocess
import sys
from pathlib import Path

MAX_CHECKER_BYTES = 4 * 1024 * 1024
MAX_COMMIT_BYTES = 64 * 1024
MAX_OBJECT_LIST_BYTES = 2 * 1024 * 1024
MAX_PACK_BYTES = 64 * 1024 * 1024
MAX_BASELINE_BLOB_BYTES = 16 * 1024 * 1024
MAX_TOTAL_BLOB_BYTES = 512 * 1024 * 1024
MAX_TREE_LISTING_BYTES = 8 * 1024 * 1024
MAX_TREE_ENTRIES = 8192
MAX_OBJECTS = 16386


def fail(message):
    raise SystemExit(f"generated-workflows: trusted seed fixture incompatibility: {message}")


def directory_flags():
    return os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)


def file_flags():
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)


def open_directory(path):
    canonical = Path(path).resolve(strict=True)
    return os.open(canonical, directory_flags())


def open_absolute_parent(path):
    absolute = Path(os.path.abspath(os.fspath(path)))
    parent = absolute.parent.resolve(strict=True)
    parent_fd = os.open(parent, directory_flags())
    return parent_fd, absolute.name


def open_relative_parent(root_fd, relative_path, label):
    parts = Path(relative_path).parts
    if not parts or Path(relative_path).is_absolute() or any(part in {"", ".", ".."} for part in parts):
        fail(f"{label} path is unsafe")
    current_fd = os.dup(root_fd)
    for part in parts[:-1]:
        try:
            metadata = os.stat(part, dir_fd=current_fd, follow_symlinks=False)
        except FileNotFoundError:
            os.close(current_fd)
            fail(f"{label} is missing")
        except OSError:
            os.close(current_fd)
            fail(f"{label} path cannot be inspected safely")
        if stat.S_ISLNK(metadata.st_mode):
            os.close(current_fd)
            fail(f"{label} path traverses a symlink")
        if not stat.S_ISDIR(metadata.st_mode):
            os.close(current_fd)
            fail(f"{label} path traverses a non-directory")
        try:
            next_fd = os.open(part, directory_flags(), dir_fd=current_fd)
        except OSError:
            os.close(current_fd)
            fail(f"{label} path changed while it was opened")
        os.close(current_fd)
        current_fd = next_fd
    return current_fd, parts[-1]


def stable_stat(left, right):
    return (
        left.st_dev,
        left.st_ino,
        left.st_mode,
        left.st_size,
        left.st_mtime_ns,
        left.st_ctime_ns,
    ) == (
        right.st_dev,
        right.st_ino,
        right.st_mode,
        right.st_size,
        right.st_mtime_ns,
        right.st_ctime_ns,
    )


def read_open_file(file_fd, parent_fd, leaf, label, byte_limit, missing_message=None, nonregular_message=None, growth_message=None):
    try:
        before = os.fstat(file_fd)
    except OSError:
        fail(f"{label} could not be inspected safely")
    if not stat.S_ISREG(before.st_mode):
        fail(nonregular_message or f"required {label} is not a regular non-symlink file")
    if before.st_size < 0 or before.st_size > byte_limit:
        fail(f"{label} exceeds its reviewed byte bound")
    chunks = []
    remaining = byte_limit + 1
    while remaining:
        try:
            chunk = os.read(file_fd, min(1024 * 1024, remaining))
        except OSError:
            fail(f"{label} could not be read safely")
        if not chunk:
            break
        chunks.append(chunk)
        remaining -= len(chunk)
    payload = b"".join(chunks)
    if len(payload) > byte_limit:
        fail(growth_message or f"{label} exceeds its reviewed byte bound")
    try:
        after = os.fstat(file_fd)
        named = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
    except OSError:
        fail(f"{label} changed while it was read")
    if not stat.S_ISREG(named.st_mode) or not stable_stat(before, after) or not stable_stat(after, named) or len(payload) != after.st_size:
        fail(growth_message or f"{label} changed while it was read")
    return payload, before


def read_relative(root_fd, relative_path, label, byte_limit, missing_message=None, nonregular_message=None, growth_message=None):
    parent_fd, leaf = open_relative_parent(root_fd, relative_path, label)
    try:
        try:
            named = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            fail(missing_message or f"required {label} is missing")
        except OSError:
            fail(f"{label} path cannot be inspected safely")
        if stat.S_ISLNK(named.st_mode) or not stat.S_ISREG(named.st_mode):
            fail(nonregular_message or f"required {label} is not a regular non-symlink file")
        if named.st_size < 0 or named.st_size > byte_limit:
            fail(f"{label} exceeds its reviewed byte bound")
        try:
            file_fd = os.open(leaf, file_flags(), dir_fd=parent_fd)
        except OSError:
            fail(f"{label} changed while it was opened")
        try:
            return read_open_file(file_fd, parent_fd, leaf, label, byte_limit, missing_message, nonregular_message, growth_message)
        finally:
            os.close(file_fd)
    finally:
        os.close(parent_fd)


def read_absolute(path, label, byte_limit):
    try:
        parent_fd, leaf = open_absolute_parent(path)
    except OSError:
        fail(f"required {label} is missing or its parent is unsafe")
    try:
        try:
            named = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            fail(f"required {label} is missing")
        except OSError:
            fail(f"required {label} path cannot be inspected safely")
        if stat.S_ISLNK(named.st_mode) or not stat.S_ISREG(named.st_mode):
            fail(f"required {label} is not a regular non-symlink file")
        if named.st_size < 0 or named.st_size > byte_limit:
            fail(f"{label} exceeds its reviewed byte bound")
        try:
            file_fd = os.open(leaf, file_flags(), dir_fd=parent_fd)
        except OSError:
            fail(f"{label} changed while it was opened")
        try:
            return read_open_file(file_fd, parent_fd, leaf, label, byte_limit)
        finally:
            os.close(file_fd)
    finally:
        os.close(parent_fd)


def isolated_environment(home):
    return {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"),
        "HOME": str(home),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_SYSTEM": os.devnull,
        "GIT_CONFIG_COUNT": "4",
        "GIT_CONFIG_KEY_0": "core.hooksPath",
        "GIT_CONFIG_VALUE_0": os.devnull,
        "GIT_CONFIG_KEY_1": "core.fsmonitor",
        "GIT_CONFIG_VALUE_1": "false",
        "GIT_CONFIG_KEY_2": "credential.helper",
        "GIT_CONFIG_VALUE_2": "",
        "GIT_CONFIG_KEY_3": "core.sshCommand",
        "GIT_CONFIG_VALUE_3": "/usr/bin/false",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_ALLOW_PROTOCOL": "file",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_ATTR_NOSYSTEM": "1",
    }


def bounded_process(command, environment, output_limit, input_bytes=None):
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE if input_bytes is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=environment,
    )
    try:
        if input_bytes is not None:
            try:
                process.stdin.write(input_bytes)
                process.stdin.close()
            except OSError:
                process.kill()
                process.wait()
                raise subprocess.CalledProcessError(process.returncode or 1, command)
        output = process.stdout.read(output_limit + 1)
        if len(output) > output_limit:
            process.kill()
            process.wait()
            fail("Git output exceeds its reviewed byte bound")
        extra = process.stdout.read(1)
        if extra:
            process.kill()
            process.wait()
            fail("Git output exceeds its reviewed byte bound")
        status = process.wait()
        if status != 0:
            raise subprocess.CalledProcessError(status, command)
        return output
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def run_git(git, repository, *arguments, home, output_limit=1024 * 1024, input_bytes=None):
    environment = isolated_environment(home)
    command = [git, "-c", f"core.hooksPath={os.devnull}", "-c", "core.fsmonitor=false", "-c", "credential.helper=", "-c", "core.sshCommand=/usr/bin/false", "-C", str(repository), *arguments]
    return bounded_process(command, environment, output_limit, input_bytes)


def parse_constants(checker):
    source, _ = read_absolute(checker, "checker", MAX_CHECKER_BYTES)
    values = {}
    for name in (
        "trusted_seed_commit",
        "trusted_seed_tree",
        "trusted_metadata_pack_sha256",
        "trusted_metadata_object_list_sha256",
    ):
        found = re.findall(rb"(?m)^" + name.encode() + rb"=([0-9a-f]+)$", source)
        if len(found) != 1:
            fail(f"checker does not contain one pinned {name}")
        values[name] = found[0].decode("ascii")
    if not re.fullmatch(r"[0-9a-f]{40}", values["trusted_seed_commit"]):
        fail("checker seed commit identity is malformed")
    if not re.fullmatch(r"[0-9a-f]{40}", values["trusted_seed_tree"]):
        fail("checker seed tree identity is malformed")
    return values


def tree_inventory(git, seed_repository, commit, root_tree, home):
    listing = run_git(
        git,
        seed_repository,
        "ls-tree",
        "-r",
        "-t",
        "-z",
        "--full-tree",
        root_tree,
        home=home,
        output_limit=MAX_TREE_LISTING_BYTES,
    )
    if not listing.endswith(b"\0"):
        fail("pinned tree listing is truncated or exceeds its bound")
    raw_entries = listing[:-1].split(b"\0") if listing != b"\0" else []
    if len(raw_entries) > MAX_TREE_ENTRIES:
        fail("pinned tree exceeds its entry bound")
    entries = []
    expected_types = {commit: "commit", root_tree: "tree"}
    for raw in raw_entries:
        try:
            header, raw_path = raw.split(b"\t", 1)
            mode, kind, oid = header.decode("ascii").split(" ")
            path = raw_path.decode("utf-8", "strict")
        except (ValueError, UnicodeError):
            fail("pinned tree contains a malformed entry")
        parts = path.split("/")
        if not path or path.startswith("/") or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts) or any(ord(char) < 32 or ord(char) == 127 for char in path):
            fail("pinned tree contains an unsafe path")
        if not re.fullmatch(r"[0-9a-f]{40}", oid):
            fail("pinned tree contains a malformed object identity")
        if mode == "040000" and kind == "tree":
            object_type = "tree"
        elif (mode, kind) in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
            object_type = "blob"
        else:
            fail("pinned tree contains an unsupported entry")
        excluded = object_type == "blob" and any(
            path.startswith(prefix) and "/" not in path[len(prefix):] and path[len(prefix):].endswith(".tui.pb")
            for prefix in (
                "tests/functional/baselines/functional/",
                "apps/jackin/tests/functional/baselines/functional/",
            )
        )
        previous = expected_types.get(oid)
        if previous is not None and previous != object_type:
            fail("pinned tree assigns one object identity incompatible types")
        expected_types[oid] = object_type
        entries.append((mode, kind, oid, path, excluded))
    return entries, expected_types


def parse_actual_object_inventory(actual_raw):
    actual = {}
    for line in actual_raw.splitlines():
        fields = line.split()
        if len(fields) != 3 or fields[1] not in {b"commit", b"tree", b"blob"} or not fields[2].isdigit():
            fail("metadata pack contains an invalid Git object inventory")
        try:
            oid = fields[0].decode("ascii", "strict")
        except UnicodeError:
            fail("metadata pack contains an invalid Git object inventory")
        if not re.fullmatch(r"[0-9a-f]{40}", oid) or oid in actual:
            fail("metadata pack contains an invalid Git object inventory")
        actual[oid] = (fields[1].decode("ascii"), int(fields[2]))
        if len(actual) > MAX_OBJECTS:
            fail("metadata pack object inventory exceeds its reviewed count bound")
    return actual


def validate_metadata_object_inventory(expected_types, actual, object_list):
    expected_objects = set(expected_types)
    if set(object_list) != expected_objects or set(actual) != expected_objects:
        fail("metadata pack object inventory does not match the pinned tree inventory")
    if any(actual[oid][0] != object_type for oid, object_type in expected_types.items()):
        fail("metadata pack object types do not match the pinned tree")


def object_directory_path(git, repo_root, home):
    output = run_git(
        git,
        repo_root,
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "objects",
        home=home,
        output_limit=4096,
    )
    try:
        return Path(output.decode("utf-8", "strict").strip())
    except UnicodeError:
        fail("source Git object database path is malformed")


def reject_source_alternates(objects_path):
    try:
        objects_path = Path(os.path.abspath(os.fspath(objects_path)))
        parent_fd, leaf = open_absolute_parent(objects_path)
    except OSError:
        fail("source Git object database path is unsafe")
    try:
        try:
            objects_metadata = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        except OSError:
            fail("source Git object database path is unsafe")
        if stat.S_ISLNK(objects_metadata.st_mode) or not stat.S_ISDIR(objects_metadata.st_mode):
            fail("source Git object database path is unsafe")
        try:
            objects_fd = os.open(leaf, directory_flags(), dir_fd=parent_fd)
        except OSError:
            fail("source Git object database path changed while it was opened")
    finally:
        os.close(parent_fd)
    try:
        try:
            info_fd = os.open("info", directory_flags(), dir_fd=objects_fd)
        except FileNotFoundError:
            return
        except OSError:
            fail("source Git object database info path is unsafe")
        try:
            for name in ("alternates", "http-alternates"):
                try:
                    os.stat(name, dir_fd=info_fd, follow_symlinks=False)
                except FileNotFoundError:
                    continue
                except OSError:
                    fail("source Git object alternate path cannot be inspected safely")
                fail("source Git object database has an alternate object source")
        finally:
            os.close(info_fd)
    finally:
        os.close(objects_fd)


def checked_artifact(repo_fd, relative_path, label, limit, missing_message, size_message):
    try:
        payload, metadata = read_relative(
            repo_fd,
            relative_path,
            label,
            limit,
            missing_message=missing_message,
            growth_message=size_message,
        )
    except SystemExit:
        raise
    if metadata.st_size > limit:
        fail(size_message)
    return payload


def validate_and_import(repo_root, checker, seed_repository):
    git = shutil.which("git", path=os.environ.get("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"))
    if git is None or not Path(git).is_absolute():
        fail("Git executable path is not absolute")
    repo_root = Path(repo_root).resolve(strict=True)
    seed_repository = Path(os.path.abspath(os.fspath(seed_repository)))
    constants = parse_constants(checker)
    try:
        repo_fd = os.open(repo_root, directory_flags())
    except OSError:
        fail("source repository root is unsafe")
    try:
        home = seed_repository.parent / "git-home"
        home.mkdir(mode=0o700, parents=True, exist_ok=True)
        source_objects = object_directory_path(git, repo_root, home)
        reject_source_alternates(source_objects)
        artifacts = {
            "commit": checked_artifact(
                repo_fd,
                "scripts/trusted-seed.commit",
                "embedded commit identity",
                MAX_COMMIT_BYTES,
                "embedded commit identity missing",
                "embedded commit identity exceeds its reviewed byte bound",
            ),
            "pack": checked_artifact(
                repo_fd,
                "scripts/trusted-seed-metadata.pack",
                "metadata pack",
                MAX_PACK_BYTES,
                "metadata pack digest missing",
                "metadata pack exceeds its reviewed byte bound",
            ),
            "objects": checked_artifact(
                repo_fd,
                "scripts/trusted-seed-metadata.objects",
                "metadata object list",
                MAX_OBJECT_LIST_BYTES,
                "metadata object list digest missing",
                "metadata object list exceeds its reviewed byte bound",
            ),
        }
        if hashlib.sha256(artifacts["pack"]).hexdigest() != constants["trusted_metadata_pack_sha256"]:
            fail("metadata pack digest mismatch")
        if hashlib.sha256(artifacts["objects"]).hexdigest() != constants["trusted_metadata_object_list_sha256"]:
            fail("metadata object list digest mismatch")
        commit_oid = hashlib.sha1(b"commit " + str(len(artifacts["commit"])).encode() + b"\0" + artifacts["commit"]).hexdigest()
        if commit_oid != constants["trusted_seed_commit"]:
            fail("embedded commit identity mismatch")
        object_lines = artifacts["objects"].decode("ascii", "strict").splitlines()
        if object_lines != sorted(set(object_lines)) or not object_lines or len(object_lines) > MAX_OBJECTS or any(not re.fullmatch(r"[0-9a-f]{40}", oid) for oid in object_lines):
            fail("metadata object list is malformed or exceeds its reviewed count bound")
        if constants["trusted_seed_commit"] not in object_lines or constants["trusted_seed_tree"] not in object_lines or constants["trusted_seed_commit"] == constants["trusted_seed_tree"]:
            fail("metadata object list omits the pinned commit or root tree")
        object_format = run_git(git, repo_root, "rev-parse", "--show-object-format", home=home, output_limit=32).decode("ascii", "strict").strip()
        if object_format != "sha1":
            fail("source repository object format is not the pinned SHA-1 format")
        try:
            seed_repository.lstat()
        except FileNotFoundError:
            pass
        else:
            fail("private seed repository output path already exists")
        try:
            parent_fd = os.open(seed_repository.parent.resolve(strict=True), directory_flags())
        except OSError:
            fail("private seed repository parent is unsafe")
        else:
            os.close(parent_fd)
        run_git(git, seed_repository.parent, "init", "--bare", "--quiet", "--object-format=sha1", str(seed_repository), home=home, output_limit=4096)
        run_git(git, seed_repository, "index-pack", "--stdin", home=home, output_limit=4096, input_bytes=artifacts["pack"])
        actual_raw = run_git(
            git,
            seed_repository,
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname) %(objecttype) %(objectsize)",
            home=home,
            output_limit=4 * 1024 * 1024,
        )
        actual = parse_actual_object_inventory(actual_raw)
        commit = constants["trusted_seed_commit"]
        root_tree = constants["trusted_seed_tree"]
        if actual.get(commit, (None,))[0] != "commit":
            fail("metadata pack omits a commit object")
        tree = run_git(git, seed_repository, "rev-parse", "--verify", commit + "^{tree}", home=home, output_limit=128).decode("ascii", "strict").strip()
        if tree != root_tree or actual.get(root_tree, (None,))[0] != "tree":
            fail("metadata pack root tree does not match the pinned identity")
        commit_body = run_git(git, seed_repository, "cat-file", "commit", commit, home=home, output_limit=MAX_COMMIT_BYTES)
        header = commit_body.split(b"\n\n", 1)[0]
        if re.search(rb"(?m)^parent [0-9a-f]{40}$", header):
            fail("pinned seed root commit unexpectedly has a parent")
        entries, expected_types = tree_inventory(git, seed_repository, commit, root_tree, home)
        excluded_count = sum(1 for entry in entries if entry[4])
        if excluded_count != 160:
            fail("pinned functional baseline exclusion inventory differs from the reviewed count")
        expected_metadata = {oid for mode, kind, oid, path, excluded in entries if not excluded}
        expected_metadata.update((commit, root_tree))
        expected_metadata_types = {oid: kind for oid, kind in expected_types.items() if oid in expected_metadata}
        validate_metadata_object_inventory(expected_metadata_types, actual, object_lines)
        total_blob_bytes = 0
        for kind, size in actual.values():
            if kind == "blob":
                if size > MAX_BASELINE_BLOB_BYTES:
                    fail("metadata pack contains an oversized blob")
                total_blob_bytes += size
        if total_blob_bytes > MAX_TOTAL_BLOB_BYTES:
            fail("metadata pack exceeds its reviewed object byte bound")
        baseline_pack, _ = read_relative(
            repo_fd,
            "tests/fixtures/trusted-seed-baselines.pack",
            "trusted seed baseline pack",
            1024 * 1024,
        )
        if hashlib.sha256(baseline_pack).hexdigest() != "44f010a183cf42d933e61622c00b49c944842c096641063d7057345b951140f7":
            fail("trusted seed baseline pack digest differs from its reviewed identity")
        run_git(git, seed_repository, "index-pack", "--stdin", home=home, output_limit=4096, input_bytes=baseline_pack)
        complete_raw = run_git(
            git,
            seed_repository,
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname) %(objecttype) %(objectsize)",
            home=home,
            output_limit=4 * 1024 * 1024,
        )
        complete = parse_actual_object_inventory(complete_raw)
        if set(complete) != set(expected_types) or any(complete[oid][0] != kind for oid, kind in expected_types.items()):
            fail("complete seed object inventory differs from the pinned tree")
        total_blob_bytes = 0
        for kind, size in complete.values():
            if kind == "blob":
                if size > MAX_BASELINE_BLOB_BYTES:
                    fail("complete seed contains an oversized blob")
                total_blob_bytes += size
        if total_blob_bytes > MAX_TOTAL_BLOB_BYTES:
            fail("complete seed exceeds its reviewed object byte bound")
        run_git(git, seed_repository, "update-ref", "refs/heads/trusted-seed", commit, home=home, output_limit=4096)
        seed_objects_path = seed_repository / "objects"
        reject_private_alternates(seed_objects_path)
    finally:
        os.close(repo_fd)


def reject_private_alternates(objects_path):
    objects_path = Path(objects_path)
    try:
        objects_fd = os.open(objects_path.resolve(strict=True), directory_flags())
    except OSError:
        fail("private seed repository object database is unsafe")
    try:
        try:
            info_fd = os.open("info", directory_flags(), dir_fd=objects_fd)
        except FileNotFoundError:
            return
        except OSError:
            fail("private seed repository object info path is unsafe")
        try:
            for name in ("alternates", "http-alternates"):
                try:
                    os.stat(name, dir_fd=info_fd, follow_symlinks=False)
                except FileNotFoundError:
                    continue
                except OSError:
                    fail("private seed repository alternate path cannot be inspected safely")
                fail("private seed repository unexpectedly has an object alternate")
        finally:
            os.close(info_fd)
    finally:
        os.close(objects_fd)


def main():
    if len(sys.argv) != 4:
        fail("fixture builder arguments are invalid")
    repo_root, checker, seed_repository = sys.argv[1:]
    try:
        validate_and_import(repo_root, checker, seed_repository)
    except subprocess.CalledProcessError:
        fail("Git rejected a pinned metadata or object operation")
    except (OSError, UnicodeError, ValueError):
        fail("source fixture input could not be processed safely")


if __name__ == "__main__":
    main()
