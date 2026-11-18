#!/usr/bin/env python3
"""Regression tests for the generated-workflow snapshot size boundary."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "validate_snapshot_blob_sizes", ROOT / "scripts/validate-snapshot-blob-sizes.py"
)
assert SPEC is not None and SPEC.loader is not None
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class SnapshotBlobSizeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.path = "tests/functional/reference-bins/aarch64-apple-darwin/tool"
        self.oid = "a" * 40
        self.entry = f"100755 blob {self.oid}\t{self.path}\0".encode()
        self.size = 20_000_000
        self.status = [f"{self.oid} blob {self.size}"]
        self.allowlist = f"{self.path}\t{self.oid}\t{self.size}\n"
        self.maximum_blob = 16 * 1024 * 1024

    def validate(self, *, entry: bytes | None = None, status: list[str] | None = None, allowlist: str | None = None) -> None:
        VALIDATOR.validate(
            self.entry if entry is None else entry,
            self.status if status is None else status,
            False,
            self.maximum_blob,
            5 * 512 * 1024 * 1024,
            "snapshot",
            self.allowlist if allowlist is None else allowlist,
        )

    def test_exact_path_oid_and_size_are_allowed(self) -> None:
        self.validate()

    def test_different_path_is_rejected(self) -> None:
        wrong = f"100755 blob {self.oid}\ttests/functional/reference-bins/aarch64-apple-darwin/other\0".encode()
        with self.assertRaisesRegex(ValueError, "per-object size limit"):
            self.validate(entry=wrong)

    def test_different_oid_is_rejected(self) -> None:
        wrong_oid = "b" * 40
        wrong = f"100755 blob {wrong_oid}\t{self.path}\0".encode()
        with self.assertRaisesRegex(ValueError, "per-object size limit"):
            self.validate(entry=wrong, status=[f"{wrong_oid} blob {self.size}"])

    def test_different_size_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "per-object size limit"):
            self.validate(status=[f"{self.oid} blob {self.size + 1}"])

    def test_oversized_regular_source_is_rejected(self) -> None:
        path = "crates/tuiscotti/src/lib.rs"
        entry = f"100644 blob {self.oid}\t{path}\0".encode()
        with self.assertRaisesRegex(ValueError, "per-object size limit"):
            self.validate(entry=entry)

    def test_aggregate_limit_accepts_exact_boundary(self) -> None:
        count = 160
        entries = b"".join(
            f"100644 blob {self.oid}\tfixtures/{number}\0".encode()
            for number in range(count)
        )
        statuses = [f"{self.oid} blob {16 * 1024 * 1024}"] * count
        VALIDATOR.validate(
            entries,
            statuses,
            False,
            self.maximum_blob,
            5 * 512 * 1024 * 1024,
            "snapshot",
            "",
        )

    def test_aggregate_limit_rejects_one_byte_over_boundary(self) -> None:
        entries = b"".join(
            f"100644 blob {self.oid}\tfixtures/{number}\0".encode()
            for number in range(161)
        )
        sizes = [16 * 1024 * 1024] * 160 + [1]
        statuses = [f"{self.oid} blob {size}" for size in sizes]
        with self.assertRaisesRegex(ValueError, "total materialized blob size limit"):
            VALIDATOR.validate(
                entries,
                statuses,
                False,
                self.maximum_blob,
                5 * 512 * 1024 * 1024,
                "snapshot",
                "",
            )


if __name__ == "__main__":
    unittest.main()
