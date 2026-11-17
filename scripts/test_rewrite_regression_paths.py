from __future__ import annotations

import contextlib
import importlib.util
import io
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str, path: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rewrite = load_script("rewrite_regression_paths", "scripts/rewrite-regression-paths.py")

COVERAGE = (
    "pub const EXCEPTIONS: &[CapabilityException] = &[\n"
    '    CapabilityException { api: "widgets::brand::Brand", reason: "r", '
    'regression_test: "src/widgets/brand.rs::brand_draws_the_mark", tracking_issue: None },\n'
    '    CapabilityException { api: "widgets::brand::Other", reason: "r", '
    'regression_test: "src/widgets/brand_tests.rs::other_case", tracking_issue: None },\n'
    '    CapabilityException { api: "core::id::Id", reason: "r", '
    'regression_test: "tests/regressions/ids.rs::ids_are_stable", tracking_issue: None },\n'
    "];\n"
)
LANE = (
    "pub const EXCEPTIONS: &[CapabilityException] = &[\n"
    '    CapabilityException { api: "widgets::brand::Brand::new", reason: "r",\n'
    '        regression_test: "src/widgets/brand.rs::new_brand_is_empty", tracking_issue: None },\n'
    "];\n"
)
MOVE = "src/widgets/brand.rs"
MOVED = "crates/termrock-controls/src/brand.rs"
NAME_STATUS = f"R100\t{MOVE}\t{MOVED}\nM\tsrc/lib.rs\nD\ttests/old.rs\nA\tsrc/new.rs\n"


class ParseRenamesTests(unittest.TestCase):
    def test_reads_renames_and_deletions_and_ignores_the_rest(self):
        self.assertEqual(
            rewrite.parse_renames(NAME_STATUS), {MOVE: MOVED, "tests/old.rs": None}
        )

    def test_reads_arrow_lines_comments_and_blank_lines(self):
        self.assertEqual(
            rewrite.parse_renames("# scratch\n\nsrc/a.rs -> crates/x/src/a.rs\n"),
            {"src/a.rs": "crates/x/src/a.rs"},
        )


class RewriteTextTests(unittest.TestCase):
    def test_rewrites_the_path_and_keeps_the_test_name(self):
        text, changes = rewrite.rewrite_text(COVERAGE, {MOVE: MOVED})
        self.assertIn(f'regression_test: "{MOVED}::brand_draws_the_mark"', text)
        self.assertEqual(changes, [(2, MOVE, MOVED)])

    def test_leaves_a_path_that_only_shares_a_prefix_and_every_other_byte(self):
        text, _ = rewrite.rewrite_text(COVERAGE, {MOVE: MOVED})
        self.assertIn('"src/widgets/brand_tests.rs::other_case"', text)
        self.assertEqual(
            text.replace(MOVED, MOVE), COVERAGE, "nothing but the path changed"
        )

    def test_a_deleted_file_is_reported_and_its_line_is_kept(self):
        text, changes = rewrite.rewrite_text(COVERAGE, {MOVE: None})
        self.assertEqual(text, COVERAGE)
        self.assertEqual(changes, [(2, MOVE, None)])

    def test_a_path_with_no_file_and_no_rename_is_stale(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "tests/regressions").mkdir(parents=True)
            (root / "tests/regressions/ids.rs").write_text("")
            stale = rewrite.stale_paths(COVERAGE, root, {MOVE: MOVED})
        self.assertEqual(stale, [(3, "src/widgets/brand_tests.rs")])


class RunTests(unittest.TestCase):
    def workspace(self, tmp):
        root = Path(tmp)
        (root / "apps/showcase/src/coverage").mkdir(parents=True)
        (root / "apps/showcase/src/coverage.rs").write_text(COVERAGE)
        (root / "apps/showcase/src/coverage/lane_shell.rs").write_text(LANE)
        (root / "src/widgets").mkdir(parents=True)
        (root / "src/widgets/brand_tests.rs").write_text("")
        (root / "tests/regressions").mkdir(parents=True)
        (root / "tests/regressions/ids.rs").write_text("")
        return root

    def test_dry_run_lists_every_table_and_writes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.workspace(tmp)
            out = io.StringIO()
            unresolved = rewrite.run(root, {MOVE: MOVED}, True, out)
            self.assertEqual(unresolved, 0)
            self.assertEqual(
                (root / "apps/showcase/src/coverage.rs").read_text(), COVERAGE
            )
            self.assertEqual(
                (root / "apps/showcase/src/coverage/lane_shell.rs").read_text(), LANE
            )
        report = out.getvalue()
        self.assertIn(f"coverage.rs:2: would rewrite {MOVE} -> {MOVED}", report)
        self.assertIn(f"lane_shell.rs:3: would rewrite {MOVE} -> {MOVED}", report)
        self.assertIn("2 path(s) to rewrite, 0 unresolved", report)

    def test_the_real_run_fixes_the_paths_in_the_coverage_file_and_the_lane_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.workspace(tmp)
            (root / MOVED).parent.mkdir(parents=True)
            (root / MOVED).write_text("")
            rewrite.run(root, {MOVE: MOVED}, False, io.StringIO())
            coverage = (root / "apps/showcase/src/coverage.rs").read_text()
            lane = (root / "apps/showcase/src/coverage/lane_shell.rs").read_text()
            self.assertEqual(coverage, COVERAGE.replace(MOVE, MOVED))
            self.assertEqual(lane, LANE.replace(MOVE, MOVED))
            again = io.StringIO()
            self.assertEqual(rewrite.run(root, {MOVE: MOVED}, False, again), 0)
            self.assertIn("0 path(s) rewritten", again.getvalue())

    def test_a_deleted_file_makes_the_run_unresolved(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.workspace(tmp)
            out = io.StringIO()
            self.assertEqual(rewrite.run(root, {MOVE: None}, False, out), 2)
            self.assertIn("was deleted and has no new path", out.getvalue())


class GitMoveTests(unittest.TestCase):
    def git(self, root, *args):
        subprocess.run(
            ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
            cwd=root,
            check=True,
            capture_output=True,
        )

    def test_a_scratch_git_move_is_found_and_fixed_end_to_end(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "apps/showcase/src").mkdir(parents=True)
            (root / "apps/showcase/src/coverage.rs").write_text(COVERAGE)
            (root / "src/widgets").mkdir(parents=True)
            (root / MOVE).write_text("#[test]\nfn brand_draws_the_mark() {}\n" * 4)
            self.git(root, "init", "-q")
            self.git(root, "add", "-A")
            self.git(root, "commit", "-q", "-m", "base")
            (root / MOVED).parent.mkdir(parents=True)
            self.git(root, "mv", MOVE, MOVED)
            with contextlib.redirect_stdout(io.StringIO()):
                dry = rewrite.main(["--root", str(root), "--range", "HEAD", "--dry"])
            self.assertEqual(dry, 1)
            self.assertIn(
                "tests/regressions/ids.rs", (root / "apps/showcase/src/coverage.rs").read_text()
            )
            self.assertIn(
                f'"{MOVE}::', (root / "apps/showcase/src/coverage.rs").read_text()
            )
            (root / "src/widgets/brand_tests.rs").write_text("")
            (root / "tests/regressions").mkdir(parents=True)
            (root / "tests/regressions/ids.rs").write_text("")
            with contextlib.redirect_stdout(io.StringIO()):
                real = rewrite.main(["--root", str(root), "--range", "HEAD"])
            self.assertEqual(real, 0)
            self.assertEqual(
                (root / "apps/showcase/src/coverage.rs").read_text(),
                COVERAGE.replace(MOVE, MOVED),
            )


if __name__ == "__main__":
    unittest.main()
