"""The nextest setup script builds app binaries with the run's feature set.

Runs the real scripts/build-app-bins.sh against a fake `cargo` that records the
build it is asked for, and a temporary NEXTEST_ENV.
"""

import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "build-app-bins.sh"
NAMES = ["showcase", "tablepro", "holla"]


class BuildAppBinsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="build-app-bins-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.built = self.root / "cargo-args"
        self.env_file = self.root / "nextest-env"
        self.env_file.write_text("")
        cargo = self.root / "cargo"
        cargo.write_text(f"#!/bin/sh\necho \"$@\" >> '{self.built}'\n")
        cargo.chmod(cargo.stat().st_mode | stat.S_IEXEC)

    def run_script(self, nextest_args=None, parent_args=None, **extra):
        environment = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith(("TCB_", "TERMROCK_TEST_"))
        }
        environment.update(
            PATH=f"{self.root}{os.pathsep}{environment['PATH']}",
            CARGO_TARGET_DIR=str(self.root / "target"),
            NEXTEST_ENV=str(self.env_file),
        )
        if nextest_args is not None:
            environment["TCB_NEXTEST_ARGS"] = nextest_args
        environment.update(extra)
        command = ["sh", str(SCRIPT)]
        if parent_args is not None:
            command = ["sh", "-c", f'sh "{SCRIPT}"; :', "cargo-nextest", *parent_args]
        result = subprocess.run(
            command,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def cargo_call(self):
        return self.built.read_text().strip()

    def exported(self):
        return dict(
            line.split("=", 1) for line in self.env_file.read_text().splitlines()
        )

    def assert_build(self, expected_features, expected_dir):
        call = self.cargo_call()
        self.assertIn("--package termrock-showcase", call)
        self.assertIn(f"--target-dir {self.root / 'target' / expected_dir}", call)
        before_target = call.split("--target-dir")[0]
        self.assertTrue(
            before_target.rstrip().endswith(expected_features or "termrock-holla"),
            call,
        )
        exported = self.exported()
        for name in NAMES:
            expected = self.root / "target" / expected_dir / "debug" / name
            self.assertEqual(exported[f"TCB_BIN_{name.upper()}"], str(expected))
            self.assertEqual(exported[f"TERMROCK_TEST_{name.upper()}"], str(expected))

    def test_all_features_run_builds_all_feature_apps(self):
        self.run_script("cargo nextest run --workspace --all-targets --all-features --locked")
        self.assert_build("--all-features", "app-bins-all-features")

    def test_feature_list_with_work_metrics_builds_work_metrics_apps(self):
        for args in [
            "nextest run --features work-metrics",
            "nextest run --features=cli,work-metrics",
            "nextest run -F work-metrics",
            "nextest run -Fwork-metrics",
            "nextest run --features junie-tui/work-metrics",
        ]:
            with self.subTest(args=args):
                self.built.unlink(missing_ok=True)
                self.env_file.write_text("")
                self.run_script(args)
                self.assert_build("--features work-metrics", "app-bins-work-metrics")

    def test_default_run_builds_default_apps(self):
        for args in [
            "nextest run --workspace --profile functional --no-tests fail",
            "nextest run --features cli,terminal",
            "nextest run --archive-file a.tar.zst --partition hash:1/4",
            "nextest run -E test(~work-metrics) -- --all-features",
            "",
        ]:
            with self.subTest(args=args):
                self.built.unlink(missing_ok=True)
                self.env_file.write_text("")
                self.run_script(args)
                self.assert_build("", "app-bins")

    def test_an_explicit_choice_wins_over_the_command_line(self):
        self.run_script("nextest run --all-features", TCB_APP_FEATURES="")
        self.assert_build("", "app-bins")
        self.built.unlink()
        self.env_file.write_text("")
        self.run_script("nextest run", TCB_APP_FEATURES="all-features")
        self.assert_build("--all-features", "app-bins-all-features")
        self.built.unlink()
        self.env_file.write_text("")
        self.run_script("nextest run", TCB_APP_FEATURES="work-metrics")
        self.assert_build("--features work-metrics", "app-bins-work-metrics")

    def test_the_features_come_from_the_parent_process_command_line(self):
        # No TCB_NEXTEST_ARGS: the script reads `ps` for its parent, as under
        # nextest. The wrapper shell stays alive because of the trailing `:`.
        for parent_args, features, directory in [
            (["nextest", "run", "--all-features"], "--all-features", "app-bins-all-features"),
            (["nextest", "run", "-F", "work-metrics"], "--features work-metrics", "app-bins-work-metrics"),
            (["nextest", "run", "--workspace"], "", "app-bins"),
        ]:
            with self.subTest(parent_args=parent_args):
                self.built.unlink(missing_ok=True)
                self.env_file.write_text("")
                self.run_script(parent_args=parent_args)
                self.assert_build(features, directory)

    def test_binaries_the_caller_exported_are_used_and_not_rebuilt(self):
        paths = {f"TERMROCK_TEST_{name.upper()}": f"/prebuilt/{name}" for name in NAMES}
        self.run_script("nextest run --all-features", **paths)
        self.assertFalse(self.built.exists(), "nothing may be built")
        exported = self.exported()
        for name in NAMES:
            self.assertEqual(exported[f"TCB_BIN_{name.upper()}"], f"/prebuilt/{name}")
            self.assertNotIn(f"TERMROCK_TEST_{name.upper()}", exported)


if __name__ == "__main__":
    unittest.main()
