"""Keep the assurance oracle reproducible and connected to required CI."""

import shutil
import importlib.util
import tempfile
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location(
    "arc_admin_baseline", ROOT / "docs/dogfood/arc-admin/reproduce.py"
)
baseline = importlib.util.module_from_spec(spec)
spec.loader.exec_module(baseline)


class ArcAdminBaselineTests(unittest.TestCase):
    def test_frozen_inventory_and_cost_are_reproducible(self):
        baseline.check()

    def test_source_tampering_and_inventory_omission_fail(self):
        for name in ("sources/.arc-flow/flow.toml.txt", "inventory.json", "cost-summary.json"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory) / "baseline"
                shutil.copytree(baseline.BASELINE, root)
                path = root / name
                path.write_text(path.read_text() + "\n")
                with self.assertRaises(AssertionError):
                    baseline.check(root)

    def test_skipped_jobs_do_not_create_negative_runner_cost(self):
        import json
        costs = json.loads(baseline.reproduce()["cost-summary.json"])
        skipped = [job for cost in costs for job in cost["jobs"] if job["conclusion"] == "skipped"]
        self.assertTrue(skipped)
        self.assertTrue(all(job["runner_seconds"] == 0 for job in skipped))


if __name__ == "__main__":
    unittest.main()
