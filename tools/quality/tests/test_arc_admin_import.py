"""Independent semantic parity against the hash-checked GH-201 oracle."""
import hashlib
import json
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]
BASE = ROOT / "docs/dogfood/arc-admin"


class ArcAdminImportTests(unittest.TestCase):
    def test_every_baseline_declaration_and_blocker_survives(self):
        baseline = json.loads((BASE / "inventory.json").read_text())
        imported = tomllib.loads((BASE / "import/flow.toml").read_text())
        for step in imported["steps"]:
            self.assertEqual(step.pop("input"), "repository")
        self.assertEqual(imported, baseline["flow"])
        report = json.loads((BASE / "import/flow.import.json").read_text())
        self.assertEqual([step["id"] for step in report["steps"]], baseline["blocking_when_selected"])
        self.assertTrue(all(step["blocks_when_selected"] for step in report["steps"]))
        self.assertEqual(len(report["steps"]), 25)
        self.assertEqual(sum("hook" in step["profiles"] for step in report["steps"]), 17)
        self.assertEqual([step["id"] for step in report["steps"] if not step["listed_in_required_steps"]], baseline["blocking_beyond_policy_list"])
        self.assertEqual(report["authority_transfer"], "blocked")
        self.assertTrue(report["runtime_blockers"])

    def test_metrics_and_artifact_identity(self):
        source = (BASE / "sources/.arc-flow/flow.toml.txt").read_bytes()
        output = (BASE / "import/flow.toml").read_bytes()
        report = json.loads((BASE / "import/flow.import.json").read_text())
        self.assertEqual(report["source_sha256"], hashlib.sha256(source).hexdigest())
        self.assertEqual(report["output_sha256"], hashlib.sha256(output).hexdigest())
        metrics = report["ux_metrics"]
        self.assertEqual(metrics["source_config_bytes"], len(source))
        self.assertEqual(metrics["generated_config_bytes"], len(output))
        self.assertEqual(metrics["duplicated_step_definitions"], 25)
        self.assertEqual(metrics["manually_reentered_steps"], 0)
        self.assertEqual(metrics["manual_execution_config_edits"], 0)
        self.assertIsNone(metrics["operator_elapsed_seconds"])


if __name__ == "__main__":
    unittest.main()
