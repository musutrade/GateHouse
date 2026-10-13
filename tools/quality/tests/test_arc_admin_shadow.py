"""Observation evidence must never promote missing gates or stale reports to PASS."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SHADOW = ROOT / "docs/dogfood/arc-admin/shadow"
SPEC = importlib.util.spec_from_file_location("arc_shadow", SHADOW / "reproduce.py")
shadow = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(shadow)


class ArcAdminShadowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "shadow"
        shutil.copytree(SHADOW, self.root)
        for name in ("inventory.json", "sources/.arc-flow/flow.toml.txt", "import/flow.toml", "quality/quality.toml"):
            path = self.root.parent / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(SHADOW.parent / name, path)

    def change(self, name, update, rehash=False):
        path = self.root / name
        value = json.loads(path.read_text())
        update(value)
        path.write_text(json.dumps(value))
        if rehash:
            self.change("observation.json", lambda o: o["artifacts"].update(
                {name: hashlib.sha256(path.read_bytes()).hexdigest()}))

    def test_retained_matrix_covers_every_blocker_without_claiming_parity(self):
        result = shadow.matrix(self.root)
        self.assertEqual(result, json.loads((SHADOW / "matrix.json").read_text()))
        self.assertEqual(shadow.render(result), (SHADOW / "matrix.md").read_text())
        self.assertEqual(len(result["gates"]), 25)
        self.assertEqual(sum(r["policy_required"] for r in result["gates"]), 23)
        self.assertTrue(all(r["harness_gate"].startswith("NOT_RUN") for r in result["gates"]))
        self.assertFalse(result["validation_is_workflow_state"])

    def test_missing_required_gate_cannot_be_reported_as_complete(self):
        self.change("evidence/arc/test_result.json", lambda r: r["steps"].pop(), rehash=True)
        with self.assertRaises(AssertionError):
            shadow.matrix(self.root)

    def test_unclassified_or_ambiguous_discrepancy_is_rejected(self):
        self.change("observation.json", lambda o: o["discrepancies"][0].update(
            category="Harness-Gate capability gap; Arc-Admin issue"))
        with self.assertRaises(AssertionError):
            shadow.matrix(self.root)

    def test_gate_with_unknown_discrepancy_is_rejected(self):
        self.change("observation.json", lambda o: o["gate_discrepancies"].update(
            {"backend.tests": ["unexplained"]}))
        with self.assertRaises(AssertionError):
            shadow.matrix(self.root)

    def test_corrupt_artifact_is_rejected(self):
        (self.root / "evidence/arc/test_result.json").write_text("{}")
        with self.assertRaisesRegex(AssertionError, "test_result.json"):
            shadow.matrix(self.root)

    def test_execution_overlay_changes_invalidate_observation(self):
        with (self.root.parent / "import/flow.toml").open("a") as config:
            config.write("\n# changed after observation\n")
        with self.assertRaisesRegex(AssertionError, "flow.toml"):
            shadow.matrix(self.root)

    def test_missing_service_comparison_is_rejected(self):
        self.change("observation.json", lambda o: o["dimensions"].pop(1))
        with self.assertRaisesRegex(AssertionError, "missing comparison dimension"):
            shadow.matrix(self.root)

    def test_predispatch_error_cannot_claim_pass_or_emit_arc_reports(self):
        self.change("observation.json", lambda o: o["harness_runs"][0].update(
            validation_result="PASS", emitted_reports=["evidence/arc/test_result.json"]))
        with self.assertRaises(AssertionError):
            shadow.matrix(self.root)


if __name__ == "__main__":
    unittest.main()
