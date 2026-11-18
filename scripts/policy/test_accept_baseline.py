import hashlib
import importlib.util
import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "accept-baseline.py"
SPEC = importlib.util.spec_from_file_location("accept_baseline", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
accept_baseline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(accept_baseline)


class AcceptancePreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="accept-baseline-preflight-")
        self.root = Path(self.temp.name)
        self.baselines = self.root / "baselines"
        self.baselines.mkdir()
        self.test_id = "pre_refactor/j06_tabs/j06_tabs_layout_120x40"
        self.baseline = self.baselines / f"{self.test_id}.tui.pb"
        self.baseline.parent.mkdir(parents=True)
        self.baseline.write_bytes(b"previous-approved-sample")
        self.baseline_sha = hashlib.sha256(self.baseline.read_bytes()).hexdigest()
        evidence = self.root / "evidence"
        capture = evidence / "reference-candidates"
        capture.mkdir(parents=True)
        self.candidate = capture / "candidate.tui.pb"
        self.candidate.write_bytes(b"new-reference-sample")
        self.candidate_sha = hashlib.sha256(self.candidate.read_bytes()).hexdigest()
        self.binary = self.root / "jackin"
        self.binary.write_bytes(b"reference-binary")
        self.binary_sha = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        self.session = evidence / "session.json"
        session = {
            "mode": "reference",
            "reference_sha": "source-commit",
            "lockfile_sha256": "lock-digest",
            "binary_path": str(self.binary),
            "binary_sha256": self.binary_sha,
            "args": ["--scenario", "capsule-multi"],
            "comparator_rev": "comparator-revision",
            "cols": 120,
            "rows": 40,
            "color": "truecolor",
            "platform": "macos",
            "motion": "reduced",
            "TERM": "xterm-256color",
            "clock": False,
            "history": False,
            "env": {
                "COLORTERM": "truecolor",
                "LANG": "C.UTF-8",
            },
        }
        self.session.write_text(json.dumps(session))
        self.index = capture / "index.jsonl"
        entry = {
            "test_id": self.test_id,
            "candidate_path": str(self.candidate),
            "candidate_sha256": self.candidate_sha,
            "session_path": str(self.session),
            "session_sha256": hashlib.sha256(self.session.read_bytes()).hexdigest(),
            "binary_path": str(self.binary),
            "binary_sha256": self.binary_sha,
        }
        self.index.write_text(json.dumps(entry) + "\n")
        self.manifest = self.root / "reference-manifest.json"
        manifest = {
            "schema": "reference-manifest.v1",
            "reference_sha": "source-commit",
            "lockfile_sha256": "lock-digest",
            "host": "aarch64-apple-darwin",
            "toolchain": {"rustc": "pinned"},
            "comparator": {"name": "frozen state", "rev": "comparator-revision"},
            "acceptance": {
                "candidate_gate_env": "FUNCTIONAL_CANDIDATE_GATE",
                "script": "scripts/accept-baseline.py",
            },
            "binaries": {
                "jackin-pr2": {
                    "identity_name": "jackin",
                    "platforms": {
                        "aarch64-apple-darwin": {
                            "path": str(self.binary),
                            "sha256": self.binary_sha,
                        }
                    },
                }
            },
        }
        self.manifest.write_text(json.dumps(manifest))

    def tearDown(self):
        self.temp.cleanup()

    def test_session_digest_is_for_the_exact_bytes_that_were_parsed(self):
        parsed, digest = accept_baseline.read_reference_session(self.session)
        self.assertEqual(parsed["mode"], "reference")
        self.assertEqual(digest, hashlib.sha256(self.session.read_bytes()).hexdigest())
        self.session.write_bytes(self.session.read_bytes() + b" ")
        _, changed_digest = accept_baseline.read_reference_session(self.session)
        self.assertNotEqual(digest, changed_digest)

    def run_preflight(
        self,
        *,
        expected_test_id=None,
        fixture_args=None,
        environment=(),
        with_session=True,
        with_profile=True,
        with_provenance=True,
    ):
        expected_test_id = expected_test_id or self.test_id
        fixture_args = fixture_args or ["--scenario", "capsule-multi"]
        argv = [
            str(SCRIPT),
            "--candidate",
            str(self.candidate),
            "--candidate-sha256",
            self.candidate_sha,
            "--expect-baseline",
            self.baseline_sha,
            "--baselines",
            str(self.baselines),
            "--receipt",
            str(self.root / "receipts" / "accepted.json"),
            "--precondition",
            "J06 fresh reference capture",
            "--binary",
            str(self.binary),
            "--binary-sha256",
            self.binary_sha,
            "--manifest",
            str(self.manifest),
            "--tuiscotti",
            "must-not-run",
        ]
        if with_provenance:
            argv.extend(
                [
                    "--provenance",
                    str(self.root / "receipts" / "accepted.provenance.json"),
                ]
            )
        if with_session:
            argv.extend(
                [
                    "--session",
                    str(self.session),
                    "--capture-index",
                    str(self.index),
                    "--expected-test-id",
                    expected_test_id,
                ]
            )
        if with_profile:
            argv.extend(
                [
                    "--expected-cols",
                    "120",
                    "--expected-rows",
                    "40",
                    "--expected-color",
                    "truecolor",
                    "--expected-platform",
                    "macos",
                    "--expected-motion",
                    "reduced",
                    "--expected-term",
                    "xterm-256color",
                    "--expected-clock",
                    "false",
                    "--expected-history",
                    "false",
                ]
            )
        for item in fixture_args:
            argv.append(f"--fixture-arg={item}")
        for item in environment:
            argv.append(f"--env={item}")
        before = self.baseline.read_bytes()
        with patch("sys.argv", argv), patch.object(accept_baseline.subprocess, "run") as run:
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                with self.assertRaises(SystemExit):
                    accept_baseline.main()
        run.assert_not_called()
        self.last_stderr = stderr.getvalue()
        self.assertEqual(self.baseline.read_bytes(), before)
        self.assertFalse((self.root / "receipts" / "accepted.json").exists())

    def test_capture_id_mismatch_fails_before_cli_or_baseline_write(self):
        self.run_preflight(expected_test_id="pre_refactor/j06_tabs/wrong_id")

    def test_fixture_args_must_match_exact_order_before_accept(self):
        self.run_preflight(fixture_args=["capsule-multi", "--scenario"])

    def test_claimed_environment_must_match_captured_session(self):
        self.run_preflight(environment=["COLORTERM=monochrome"])

    def test_session_mutation_invalidates_capture_index(self):
        session = json.loads(self.session.read_text())
        session["env"]["LANG"] = "C"
        self.session.write_text(json.dumps(session))
        self.run_preflight()

    def test_alternate_manifest_requires_session_before_accept(self):
        self.run_preflight(with_session=False)

    def test_alternate_manifest_requires_complete_profile_before_accept(self):
        self.run_preflight(with_profile=False)

    def test_candidate_mode_acceptance_needs_provenance_before_the_cli_runs(self):
        argv = [
            str(SCRIPT),
            "--candidate",
            str(self.candidate),
            "--candidate-sha256",
            self.candidate_sha,
            "--expect-baseline",
            self.baseline_sha,
            "--baselines",
            str(self.baselines),
            "--receipt",
            str(self.root / "receipts" / "accepted.json"),
            "--precondition",
            "J06 fresh reference capture",
            "--binary",
            str(self.binary),
            "--binary-sha256",
            self.binary_sha,
            "--tuiscotti",
            "must-not-run",
        ]
        with patch("sys.argv", argv), patch.object(accept_baseline.subprocess, "run") as run:
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                with self.assertRaises(SystemExit):
                    accept_baseline.main()
        run.assert_not_called()
        self.assertIn("provenance", stderr.getvalue())
        self.assertFalse((self.root / "receipts" / "accepted.json").exists())

    def test_the_receipt_and_the_provenance_must_be_different_files(self):
        argv = [
            str(SCRIPT),
            "--candidate",
            str(self.candidate),
            "--candidate-sha256",
            self.candidate_sha,
            "--expect-baseline",
            self.baseline_sha,
            "--baselines",
            str(self.baselines),
            "--receipt",
            str(self.root / "receipts" / "accepted.json"),
            "--provenance",
            str(self.root / "receipts" / "accepted.json"),
            "--precondition",
            "J06 fresh reference capture",
            "--binary",
            str(self.binary),
            "--binary-sha256",
            self.binary_sha,
            "--tuiscotti",
            "must-not-run",
        ]
        with patch("sys.argv", argv), patch.object(accept_baseline.subprocess, "run") as run:
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                with self.assertRaises(SystemExit):
                    accept_baseline.main()
        run.assert_not_called()
        self.assertIn("different files", stderr.getvalue())
        self.assertFalse((self.root / "receipts" / "accepted.json").exists())

    def test_acceptance_requires_provenance_before_the_cli_runs(self):
        self.run_preflight(with_provenance=False)
        self.assertIn(
            "provenance",
            self.last_stderr,
            "the refusal names the missing provenance file",
        )


if __name__ == "__main__":
    unittest.main()
