"""Arc-Admin bindings preserve certified measurements and project-owned gates."""
import copy
import json
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]
ARC = ROOT / "docs/dogfood/arc-admin"
QUALITY = ARC / "quality"
PRESETS = ROOT / "tools/harness-gate/fixtures/preset-compat"


def read(path):
    return json.loads(path.read_text())


class ArcAdminQualityTests(unittest.TestCase):
    def setUp(self):
        self.quality = tomllib.loads((QUALITY / "quality.toml").read_text())
        self.flow = tomllib.loads((ARC / "import/flow.toml").read_text())

    def test_composed_packs_preserve_every_policy_and_capability(self):
        for selection in read(PRESETS / "angular-rust-postgres.packs.json"):
            pack = (PRESETS / "packs" / (selection["pack"] + ".json")).read_text()
            for key, value in selection["bindings"].items():
                pack = pack.replace("${" + key + "}", value)
            expanded = json.loads(pack)
            for name, expected in expanded["files"].items():
                self.assertEqual(read(QUALITY / name.removeprefix(".harness-gate/")), expected)
            for section in ("components", "subjects", "relationships", "collectors", "policies"):
                for key, value in expanded["quality"].get(section, {}).items():
                    self.assertEqual(self.quality[section][key], value)

    def test_rust_binds_certified_series_and_unchanged_thresholds(self):
        reference = read(ROOT / "tools/harness-gate/fixtures/workflow/collectors/certified-rust-profiles.json")
        capabilities = read(QUALITY / "packs/backend/capabilities.json")
        self.assertEqual(capabilities["series"], reference["series"])
        rules = copy.deepcopy(reference["rules"])
        for rule in rules:
            rule.update(id="backend." + rule["metric"], scope={"kind": "component", "component": "backend"})
        self.assertEqual(read(QUALITY / "packs/backend/policy.json")["rules"], rules)
        self.assertEqual(self.quality["baseline"], {
            "required": True, "provider": {"kind": "git", "reference": "origin/main", "merge_base": True}})
        self.assertEqual(self.quality["profiles"]["full"]["workflow"]["baseline_request"],
                         ".harness-gate/runtime/full-baseline-request.json")

    def test_angular_crap_is_unsupported_and_never_numeric(self):
        capabilities = read(QUALITY / "packs/frontend/capabilities.json")
        self.assertEqual(capabilities["states"]["risk.crap"], "unsupported")
        self.assertNotIn("values", capabilities)
        rules = read(QUALITY / "packs/frontend/policy.json")["rules"]
        self.assertFalse(next(r for r in rules if r["metric"] == "risk.crap")["required"])
        self.assertIn("frontend.risk.crap", self.quality["profiles"]["full"]["policies"])

    def test_relationship_has_required_measurements_without_replacing_hooks(self):
        self.assertEqual(self.quality["relationships"], {"frontend-api": {
            "kind": "api-contract", "from": {"kind": "component", "id": "frontend"},
            "to": {"kind": "component", "id": "backend"}}})
        rules = read(QUALITY / "packs/frontend-api/policy.json")["rules"]
        self.assertEqual({r["metric"] for r in rules}, {
            "contract.breaking_changes", "contract.client_drift", "contract.compatible"})
        self.assertTrue(all(r["required"] for r in rules))
        self.assertEqual(set(self.quality["collectors"]), {"frontend", "backend", "frontend-api"})
        original = read(ARC / "inventory.json")["flow"]["steps"]
        for step in self.flow["steps"]:
            self.assertEqual(step.pop("input"), "repository")
        self.assertEqual(self.flow["steps"], original)
        self.assertTrue({"frontend.e2e", "frontend.fullstack-smoke", "workflow.api-generation",
                         "workflow.production-deployment-config"}.issubset(s["id"] for s in original))
        self.assertTrue(all("command" not in collector for collector in self.quality["collectors"].values()))

    def test_profile_scope_matches_import_without_inventing_ci_or_database_quality(self):
        self.assertEqual(self.quality["project"], {"id": "arc-admin", "name": self.flow["project"]["name"]})
        declared = {p for step in self.flow["steps"] for p in step["profiles"]}
        self.assertEqual(set(self.quality["profiles"]), declared)
        self.assertEqual(set(self.quality["components"]), {"frontend", "backend"})
        hook = self.quality["profiles"]["hook"]
        self.assertEqual((hook["assurance"], hook["policies"], hook["collectors"]), ("partial", [], []))
        full = self.quality["profiles"]["full"]
        self.assertEqual(full["assurance"], "complete")
        self.assertEqual(set(full["policies"]), set(self.quality["policies"]))
        self.assertEqual(set(full["collectors"]), set(self.quality["collectors"]))


if __name__ == "__main__":
    unittest.main()
