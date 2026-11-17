#!/usr/bin/env python3
"""Run the functional suite against freshly built candidate binaries.

The Showcase, TablePro and Holla journeys (`tests/functional`) launch real
application binaries, but those binaries belong to other packages, so Cargo
gives that test target no `CARGO_BIN_EXE_<name>`. This launcher closes the gap:

1. `cargo build --workspace --all-features --bins --message-format=json`,
   bounded by a timeout;
2. read the JSON line by line and keep the executable of every
   `compiler-artifact` whose target is one of the four exact binary names;
3. refuse a failed or timed-out build, a missing or duplicated binary and a
   path that does not exist, so a stale artifact from an earlier build can
   never run;
4. only then start `cargo nextest run` with `TERMROCK_TEST_SHOWCASE`,
   `TERMROCK_TEST_TABLEPRO` and `TERMROCK_TEST_HOLLA` (and
   `TERMROCK_TEST_JACKIN_PREVIEW`) set to the canonical paths.

A partition or a test-name filter keeps each run inside the command budget:

    python3 scripts/run_functional.py --partition 1/4
    python3 scripts/run_functional.py --test-name s05_

Neither value reaches a command line as typed. A partition is rebuilt from two
integers, a test name from a fixed alphabet, and the Cargo profile is looked up
in a table, so no argument can inject an option into Cargo or nextest.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

BINARIES = {
    "showcase": "TERMROCK_TEST_SHOWCASE",
    "tablepro": "TERMROCK_TEST_TABLEPRO",
    "holla": "TERMROCK_TEST_HOLLA",
    "jackin-preview": "TERMROCK_TEST_JACKIN_PREVIEW",
}
BUILD_TIMEOUT = 240
# Cargo profile names this launcher accepts, as constants.
CARGO_PROFILES = {"dev": "dev", "release": "release"}
# Characters a test-name filter may use; each is rebuilt from this constant.
TEST_NAME_ALPHABET = "abcdefghijklmnopqrstuvwxyz0123456789_:"


class LaunchRefused(Exception):
    """The candidates cannot be trusted; no test session may start."""


def build_command(cargo_profile: str | None) -> list[str]:
    command = [
        "cargo",
        "build",
        "--workspace",
        "--all-features",
        "--bins",
        "--message-format=json",
    ]
    if cargo_profile:
        command += ["--profile", cargo_profile]
    return command


def parse_artifacts(stdout: str) -> dict[str, set[str]]:
    """Executables per wanted binary name, from Cargo's JSON stdout.

    Every stdout line must be JSON. Diagnostics belong on stderr, so a stray
    line here means the build output cannot be trusted.
    """
    found: dict[str, set[str]] = {name: set() for name in BINARIES}
    finished: bool | None = None
    for number, line in enumerate(stdout.splitlines(), 1):
        if not line.strip():
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            raise LaunchRefused(
                f"cargo stdout line {number} is not JSON: {error}"
            ) from error
        reason = message.get("reason")
        if reason == "build-finished":
            finished = bool(message.get("success"))
        if reason != "compiler-artifact":
            continue
        target = message.get("target") or {}
        executable = message.get("executable")
        name = target.get("name")
        if executable is None or name not in BINARIES:
            continue
        if "bin" not in (target.get("kind") or []):
            continue
        found[name].add(executable)
    if finished is not True:
        raise LaunchRefused("cargo did not report a successful build-finished")
    return found


def resolve_candidates(found: dict[str, set[str]]) -> dict[str, str]:
    """One existing, canonical file per binary name, or a refusal."""
    resolved: dict[str, str] = {}
    for name in BINARIES:
        paths = {str(Path(path).resolve()) for path in found[name]}
        if not paths:
            raise LaunchRefused(f"the build produced no executable for `{name}`")
        if len(paths) > 1:
            raise LaunchRefused(
                f"the build produced {len(paths)} different executables for `{name}`: "
                + ", ".join(sorted(paths))
            )
        (path,) = paths
        if not Path(path).is_file():
            raise LaunchRefused(f"the executable for `{name}` does not exist: {path}")
        resolved[name] = path
    return resolved


def build_candidates(cargo_profile: str | None, timeout: int) -> dict[str, str]:
    command = build_command(cargo_profile)
    try:
        completed = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise LaunchRefused(f"the build exceeded {timeout} seconds") from error
    except OSError as error:
        raise LaunchRefused(f"cannot run cargo: {error}") from error
    if completed.returncode != 0:
        raise LaunchRefused(
            f"the build failed with exit status {completed.returncode}; "
            "no earlier artifact is used"
        )
    return resolve_candidates(parse_artifacts(completed.stdout))


def canonical_test_name(text: str) -> str:
    """The same text, rebuilt character by character from the constant alphabet."""
    if not text or any(character not in TEST_NAME_ALPHABET for character in text):
        raise LaunchRefused(
            f"a test name uses only lowercase letters, digits, `_` and `:`: {text!r}"
        )
    return "".join(TEST_NAME_ALPHABET[TEST_NAME_ALPHABET.index(c)] for c in text)


def parse_partition(text: str) -> str:
    match = re.fullmatch(r"([1-9][0-9]{0,2})/([1-9][0-9]{0,2})", text)
    if match is None or int(match.group(1)) > int(match.group(2)):
        raise LaunchRefused(f"a partition is K/N with 1 <= K <= N, got {text!r}")
    return f"hash:{int(match.group(1))}/{int(match.group(2))}"


def nextest_command(arguments: argparse.Namespace) -> list[str]:
    command = [
        "cargo",
        "nextest",
        "run",
        "--workspace",
        "--all-features",
        "--profile",
        "functional",
    ]
    if arguments.cargo_profile:
        command += ["--cargo-profile", CARGO_PROFILES[arguments.cargo_profile]]
    if arguments.partition:
        command += ["--partition", parse_partition(arguments.partition)]
    if arguments.test_name:
        command += ["-E", f"test(~{canonical_test_name(arguments.test_name)})"]
    if arguments.no_tests_fail:
        command += ["--no-tests", "fail"]
    return command


def run(arguments: argparse.Namespace) -> int:
    try:
        command = nextest_command(arguments)
        profile = CARGO_PROFILES[arguments.cargo_profile] if arguments.cargo_profile else None
        candidates = build_candidates(profile, arguments.build_timeout)
    except LaunchRefused as refusal:
        print(f"run_functional: refusing to launch: {refusal}", file=sys.stderr)
        return 1
    environment = dict(os.environ)
    for name, path in candidates.items():
        environment[BINARIES[name]] = path
        print(f"run_functional: {BINARIES[name]}={path}", file=sys.stderr)
    return subprocess.run(command, env=environment, check=False).returncode


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Build the candidate binaries, then run the functional suite.",
    )
    parser.add_argument(
        "--cargo-profile",
        choices=sorted(CARGO_PROFILES),
        help="Cargo profile for the build and for nextest (default: dev)",
    )
    parser.add_argument(
        "--build-timeout",
        type=int,
        default=BUILD_TIMEOUT,
        help=f"seconds allowed for the build (default {BUILD_TIMEOUT})",
    )
    parser.add_argument(
        "--partition",
        metavar="K/N",
        help="run the K-th of N hash partitions of the tests",
    )
    parser.add_argument(
        "--test-name",
        metavar="TEXT",
        help="run the tests whose name contains TEXT (lowercase, digits, _ and :)",
    )
    parser.add_argument(
        "--no-tests-fail",
        action="store_true",
        help="fail when the selection matches no test",
    )
    return run(parser.parse_args(argv))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
