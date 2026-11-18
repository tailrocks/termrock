import importlib.util
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "run_functional.py"
SPEC = importlib.util.spec_from_file_location("run_functional", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
run_functional = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(run_functional)

NAMES = list(run_functional.BINARIES)


def artifact(name, executable, kind="bin"):
    return json.dumps(
        {
            "reason": "compiler-artifact",
            "target": {"name": name, "kind": [kind]},
            "executable": executable,
        }
    )


FINISHED = json.dumps({"reason": "build-finished", "success": True})


class ArgumentTests(unittest.TestCase):
    def test_a_partition_is_rebuilt_from_two_integers(self):
        self.assertEqual(run_functional.parse_partition("3/12"), "hash:3/12")
        for bad in ["0/4", "5/4", "1", "1/4 --x", "a/b", "1/4;ls", ""]:
            with self.assertRaises(run_functional.LaunchRefused, msg=bad):
                run_functional.parse_partition(bad)

    def test_a_test_name_is_limited_to_the_fixed_alphabet(self):
        self.assertEqual(run_functional.canonical_test_name("s05_a::b"), "s05_a::b")
        for bad in ["", "-E", "a b", "a)|test(", "A", "a/b"]:
            with self.assertRaises(run_functional.LaunchRefused, msg=bad):
                run_functional.canonical_test_name(bad)

    def test_nextest_command_carries_only_rebuilt_values(self):
        arguments = run_functional.argparse.Namespace(
            cargo_profile="release",
            partition="2/4",
            test_name="s05_",
            no_tests_fail=True,
        )
        self.assertEqual(
            run_functional.nextest_command(arguments),
            [
                "cargo", "nextest", "run", "--workspace", "--all-features",
                "--profile", "functional", "--cargo-profile", "release",
                "--partition", "hash:2/4", "-E", "test(~s05_)",
                "--no-tests", "fail",
            ],
        )


class ParseTests(unittest.TestCase):
    def test_collects_only_binary_targets_with_an_executable(self):
        stdout = "\n".join(
            [
                artifact("showcase", "/t/showcase"),
                artifact("showcase", None),
                artifact("holla", "/t/libholla", kind="lib"),
                artifact("other", "/t/other"),
                FINISHED,
            ]
        )
        found = run_functional.parse_artifacts(stdout)
        self.assertEqual(found["showcase"], {"/t/showcase"})
        self.assertEqual(found["holla"], set())

    def test_rejects_non_json_stdout_and_unfinished_builds(self):
        with self.assertRaises(run_functional.LaunchRefused):
            run_functional.parse_artifacts("warning: not json\n" + FINISHED)
        with self.assertRaises(run_functional.LaunchRefused):
            run_functional.parse_artifacts(artifact("showcase", "/t/showcase"))
        with self.assertRaises(run_functional.LaunchRefused):
            run_functional.parse_artifacts(
                json.dumps({"reason": "build-finished", "success": False})
            )


class ResolveTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="run-functional-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def binary(self, name):
        path = self.root / name
        path.write_bytes(b"")
        return str(path)

    def found(self):
        return {name: {self.binary(name)} for name in NAMES}

    def test_accepts_one_existing_file_per_name(self):
        resolved = run_functional.resolve_candidates(self.found())
        self.assertEqual(sorted(resolved), sorted(NAMES))
        for path in resolved.values():
            self.assertTrue(Path(path).is_absolute())

    def test_rejects_a_missing_name_a_missing_file_and_a_duplicate(self):
        found = self.found()
        found["holla"] = set()
        with self.assertRaisesRegex(run_functional.LaunchRefused, "no executable"):
            run_functional.resolve_candidates(found)
        found = self.found()
        found["holla"] = {str(self.root / "gone")}
        with self.assertRaisesRegex(run_functional.LaunchRefused, "does not exist"):
            run_functional.resolve_candidates(found)
        found = self.found()
        found["holla"].add(self.binary("holla-2"))
        with self.assertRaisesRegex(run_functional.LaunchRefused, "different"):
            run_functional.resolve_candidates(found)


class LauncherTests(unittest.TestCase):
    """Runs the real script against a fake `cargo` that records what it is asked."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="run-functional-launch-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.launched = self.root / "launched.env"
        self.binaries = {}
        for name in NAMES:
            path = self.root / name
            path.write_bytes(b"")
            self.binaries[name] = path

    def fake_cargo(self, build_exit, build_sleep=0):
        lines = "\n".join(
            artifact(name, str(path)) for name, path in self.binaries.items()
        )
        script = self.root / "cargo"
        script.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = build ]; then\n'
            f"  sleep {build_sleep}\n"
            "  cat <<'JSON'\n"
            f"{lines}\n{FINISHED}\n"
            "JSON\n"
            f"  exit {build_exit}\n"
            "fi\n"
            f"env | grep '^TERMROCK_TEST_' > '{self.launched}'\n"
            f"echo \"$@\" >> '{self.launched}'\n"
            "exit 0\n"
        )
        script.chmod(script.stat().st_mode | stat.S_IEXEC)

    def launch(self, *extra):
        path = f"{self.root}{os.pathsep}{os.environ['PATH']}"
        environment = dict(os.environ, PATH=path)
        return subprocess.run(
            [sys.executable, str(SCRIPT), *extra],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_successful_build_exports_every_candidate_and_forwards_arguments(self):
        self.fake_cargo(build_exit=0)
        result = self.launch("--partition", "1/4", "--test-name", "s05_")
        self.assertEqual(result.returncode, 0, result.stderr)
        recorded = self.launched.read_text()
        for name, variable in run_functional.BINARIES.items():
            self.assertIn(f"{variable}={self.binaries[name].resolve()}", recorded)
        self.assertIn("--partition hash:1/4", recorded)
        self.assertIn("-E test(~s05_)", recorded)

    def test_functional_launcher_refuses_failed_build_with_old_artifact(self):
        # Every executable exists from an earlier build, and the failed build
        # still prints their paths: no session may start.
        self.fake_cargo(build_exit=101)
        result = self.launch()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("refusing to launch", result.stderr)
        self.assertFalse(self.launched.exists(), "nextest must not run")

    def test_functional_launcher_refuses_a_timed_out_build(self):
        self.fake_cargo(build_exit=0, build_sleep=5)
        result = self.launch("--build-timeout", "1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exceeded 1 seconds", result.stderr)
        self.assertFalse(self.launched.exists(), "nextest must not run")


if __name__ == "__main__":
    unittest.main()
