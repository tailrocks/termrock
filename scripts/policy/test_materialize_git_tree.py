from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MATERIALIZER = ROOT / "scripts/materialize-git-tree.py"


class BatchMaterializerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.snapshot = self.root / "snapshot"
        self.snapshot.mkdir()
        self.entries = self.root / "entries"
        self.statuses = self.root / "statuses"
        self.allowlist = self.root / "allowlist"
        self.allowlist.write_text("", encoding="utf-8")
        self.fake_git = self.root / "fake-git"
        self.calls = self.root / "calls"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_materializer(self, replies: list[bytes], *, maximum_blob: int = 1024, maximum_total: int = 2048) -> subprocess.CompletedProcess[str]:
        self.fake_git.write_text(
            f"#!{sys.executable}\n"
            "import pathlib, sys\n"
            f"with pathlib.Path({str(self.calls)!r}).open('a', encoding='ascii') as f: f.write('call\\n')\n"
            f"replies = {replies!r}\n"
            "sys.stdin.buffer.read()\n"
            "sys.stdout.buffer.write(b''.join(replies))\n"
            "sys.stdout.buffer.flush()\n",
            encoding="utf-8",
        )
        self.fake_git.chmod(0o755)
        env = {
            "PATH": os.environ["PATH"],
            "TUISCO_READER_HOME": str(self.root),
            "TUISCO_GIT_PATH": os.environ["PATH"],
            "TUISCO_MAX_BLOB": str(maximum_blob),
            "TUISCO_MAX_TOTAL": str(maximum_total),
        }
        return subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                str(MATERIALIZER),
                str(self.fake_git),
                "snapshot",
                str(self.root / "fake.git"),
                str(self.root / "objects"),
                str(self.root / "verified"),
                str(self.root / "source"),
                str(self.entries),
                str(self.statuses),
                str(self.allowlist),
                str(self.snapshot),
            ],
            input="",
            text=True,
            capture_output=True,
            env=env,
            check=False,
            timeout=2,
        )

    def prepare(self, records: list[tuple[str, str, bytes]]) -> list[bytes]:
        entries = []
        statuses = []
        replies: list[bytes] = []
        for mode, path, data in records:
            oid = ("a" if not entries else chr(ord("a") + len(entries))) * 40
            entries.append(f"{mode} blob {oid}\t{path}".encode())
            statuses.append(f"{oid} blob {len(data)}")
            replies.append(f"{oid} blob {len(data)}\n".encode("ascii") + data + b"\n")
        self.entries.write_bytes(b"\0".join(entries) + b"\0")
        self.statuses.write_text("\n".join(statuses) + "\n", encoding="ascii")
        return replies

    def test_one_batch_preserves_bytes_executable_mode_and_symlink(self) -> None:
        replies = self.prepare([
            ("100644", "nested/text", b"raw\x00bytes\xff"),
            ("100755", "nested/tool", b"#!/bin/sh\n"),
            ("120000", "nested/link", b"text"),
        ])
        result = self.run_materializer(replies)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls.read_text(encoding="ascii").splitlines(), ["call"])
        self.assertEqual((self.snapshot / "nested/text").read_bytes(), b"raw\x00bytes\xff")
        self.assertEqual((self.snapshot / "nested/tool").stat().st_mode & 0o777, 0o755)
        self.assertTrue((self.snapshot / "nested/link").is_symlink())
        self.assertEqual(os.readlink(self.snapshot / "nested/link"), "text")

    def test_rejects_header_identity_size_and_separator_mismatch(self) -> None:
        replies = self.prepare([("100644", "file", b"body")])
        variants = [
            replies[0].replace(b"a" * 40, b"b" * 40, 1),
            replies[0].replace(b"blob 4", b"blob 5", 1),
            replies[0][:-1] + b"x",
            replies[0][:-2],
        ]
        for variant in variants:
            with self.subTest(reply=variant[:50]):
                self.calls.unlink(missing_ok=True)
                result = self.run_materializer([variant])
                self.assertNotEqual(result.returncode, 0)

    def test_truncated_batch_response_does_not_wait_for_another_request(self) -> None:
        self.prepare([("100644", "one", b"body"), ("100644", "two", b"next")])
        result = self.run_materializer([b"a" * 40 + b" blob 4\nbo"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Git batch object data is truncated", result.stderr)
        self.assertEqual(self.calls.read_text(encoding="ascii").splitlines(), ["call"])

    def test_limits_reject_before_starting_batch_reader(self) -> None:
        replies = self.prepare([("100644", "file", b"012345")])
        result = self.run_materializer(replies, maximum_blob=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("per-object size limit", result.stderr)
        self.assertFalse(self.calls.exists())

        replies = self.prepare([("100644", "one", b"1234"), ("100644", "two", b"5678")])
        result = self.run_materializer(replies, maximum_total=7)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("total materialized blob size limit", result.stderr)
        self.assertFalse(self.calls.exists())

    def test_approved_blob_size_must_equal_reviewed_size(self) -> None:
        path = "tests/functional/reference-bins/test"
        replies = self.prepare([("100755", path, b"123456")])
        self.allowlist.write_text(f"{path}\t{'a' * 40}\t7\n", encoding="ascii")
        result = self.run_materializer(replies, maximum_blob=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.calls.exists())

    def test_unsafe_path_is_rejected_before_starting_batch_reader(self) -> None:
        replies = self.prepare([("100644", "../escape", b"x")])
        result = self.run_materializer(replies)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsafe path", result.stderr)
        self.assertFalse(self.calls.exists())

    def test_symlink_target_with_nul_is_rejected(self) -> None:
        replies = self.prepare([("120000", "link", b"bad\0target")])
        result = self.run_materializer(replies)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlink target contains a NUL byte", result.stderr)
        self.assertFalse((self.snapshot / "link").exists())


if __name__ == "__main__":
    unittest.main()
