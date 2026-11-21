import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "accept-changed-baselines.py"
SPEC = importlib.util.spec_from_file_location("accept_changed_baselines", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
accept_changed_baselines = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(accept_changed_baselines)

CRATE = accept_changed_baselines.CRATE
REFRESH_TSV = accept_changed_baselines.REFRESH_TSV
PREFIX = "tests/functional/baselines/"


class RefreshRowTests(unittest.TestCase):
    def test_active_path_is_crate_relative_and_resolves_to_the_baseline(self):
        baseline = accept_changed_baselines.BASELINES / "functional/showcase_page_pickers.tui.pb"
        row = accept_changed_baselines.refresh_row(
            "functional/showcase_page_pickers", baseline, "a" * 64, "b" * 64,
            "archive.tui.pb", "receipt.json", "provenance.json",
        )
        fields = row.split("\t")
        self.assertEqual(len(fields), 7)
        self.assertEqual(fields[1], PREFIX + "functional/showcase_page_pickers.tui.pb")
        self.assertNotIn("crates/", fields[1])
        self.assertEqual(CRATE / fields[1], baseline)

    def test_pre_refactor_baseline_keeps_its_directory(self):
        baseline = accept_changed_baselines.BASELINES / "pre_refactor/h01_finder/h01_finder_x.tui.pb"
        row = accept_changed_baselines.refresh_row(
            "pre_refactor/h01_finder/h01_finder_x", baseline, "a" * 64, "b" * 64,
            "archive.tui.pb", "receipt.json", "provenance.json",
        )
        self.assertEqual(
            row.split("\t")[1], PREFIX + "pre_refactor/h01_finder/h01_finder_x.tui.pb"
        )


class RefreshTsvTests(unittest.TestCase):
    def test_existing_rows_use_the_crate_relative_active_path(self):
        rows = REFRESH_TSV.read_text().splitlines()[1:]
        self.assertTrue(rows, "refresh TSV has no rows")
        for row in rows:
            active = row.split("\t")[1]
            with self.subTest(test_id=row.split("\t")[0]):
                self.assertTrue(active.startswith(PREFIX), active)


if __name__ == "__main__":
    unittest.main()
