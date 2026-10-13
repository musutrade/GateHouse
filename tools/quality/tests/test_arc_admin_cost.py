"""Retained cost claims must not promote missing CI trials or erase gate owners."""
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
COST = ROOT / "docs/dogfood/arc-admin/cost"
SPEC = importlib.util.spec_from_file_location("arc_cost", COST / "reproduce.py")
cost = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cost)


class ArcAdminCostTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.expected = cost.derive()

    def test_retained_ledger_reproduces_and_preserves_all_obligations(self):
        cost.check_report(json.loads((COST / "report.json").read_text()), self.expected)
        commands = self.expected["command_inventory"]
        self.assertEqual(len({c["id"] for c in commands}), 25)
        self.assertEqual(sum(c["explicit_policy_required"] for c in commands), 23)
        self.assertTrue(all(c["blocks_when_selected"] and c["project_owned"] and
                            c["target_executions"] == 1 for c in commands))
        measurements = self.expected["measurement_inventory"]
        self.assertEqual(sum(m["target_producers"] for m in measurements), 8)
        unsupported = [m for m in measurements if m["status"] == "unsupported"]
        self.assertEqual([(m["collector"], m["capability"], m["target_producers"])
                          for m in unsupported], [("frontend", "risk.crap", 0)])

        self.assertIsNone(self.expected["shadow_self_hosted_ci"]["added_wall_seconds"])
        self.assertIsNone(self.expected["shadow_self_hosted_ci"]["added_runner_seconds"])
        self.assertFalse(self.expected["target"]["authority_transfer_permitted"])


if __name__ == "__main__":
    unittest.main()
