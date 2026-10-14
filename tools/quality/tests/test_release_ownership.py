"""Every release script has one owning repository; shared tools have one source."""
import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "docs/quality/release-ownership.json"
RELEASE = ROOT / "tools/release"


class ReleaseOwnershipTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads(MANIFEST.read_text())
        self.owners = {name: list(paths) for name, paths in self.manifest["owners"].items()}
        self.shared = {str(k): str(v) for k, v in self.manifest["shared"].items()}

    def test_every_entry_has_exactly_one_owner(self):
        seen = {}
        for owner, paths in self.owners.items():
            for path in paths:
                self.assertNotIn(path, seen, path)
                seen[path] = owner
        for path, owner in seen.items():
            self.assertIn(owner, {"gatehouse", "gateprobes"})
            self.assertTrue((ROOT / path).exists(), path)
        for path, owner in self.shared.items():
            self.assertNotIn(path, seen, path)
            self.assertEqual(owner, "gatehouse", path)
            self.assertTrue((ROOT / path).exists(), path)

    def test_every_release_file_is_classified(self):
        classified = {path for paths in self.owners.values() for path in paths}
        classified |= set(self.shared)
        classified = {path for path in classified if path.startswith("tools/release/")}
        actual = {
            child.relative_to(ROOT).as_posix()
            for child in RELEASE.iterdir()
            if child.is_file() and not child.name.startswith(".")
        }
        self.assertEqual(classified, actual)

    def test_core_release_workflow_and_install_entry_stay_with_gatehouse(self):
        self.assertIn(".github/workflows/release.yml", self.owners["gatehouse"])
        self.assertIn("install.sh", self.owners["gatehouse"])

    def test_plugin_identity_and_shared_copy_rules_are_declared(self):
        self.assertTrue(self.manifest["plugin_release_identity"].strip())
        self.assertTrue(self.manifest["shared_copy_rule"].strip())

    def test_every_workflow_has_one_owner_and_disposition(self):
        workflows = self.manifest["workflows"]
        actual = {
            path.relative_to(ROOT).as_posix()
            for path in (ROOT / ".github/workflows").iterdir()
            if path.is_file() and path.suffix in {".yml", ".yaml"}
        }
        self.assertEqual(set(workflows), actual)
        allowed = {
            "core-retained",
            "legacy-entry-retained",
            "paused-not-migrated",
            "compatibility-validation-retained",
        }
        for path, row in workflows.items():
            self.assertIn(row["owner"], {"gatehouse", "gateprobes"}, path)
            self.assertIn(row["disposition"], allowed, path)
            self.assertTrue(row["note"].strip(), path)
            if "revival_owner" in row:
                self.assertIn(row["revival_owner"], {"gatehouse", "gateprobes"}, path)

    def test_retired_workflows_are_archived_and_restorable(self):
        retired = self.manifest["retired_workflows"]
        self.assertTrue(retired)
        for path, row in retired.items():
            self.assertTrue((ROOT / path).is_file(), path)
            self.assertEqual(row["owner"], "gatehouse", path)
            self.assertEqual(row["disposition"], "retired-restorable", path)
            self.assertRegex(row["restore_commit"], r"^[0-9a-f]{40}$")
            self.assertTrue(row["was"].startswith(".github/workflows/"), path)
            self.assertFalse((ROOT / row["was"]).exists(), row["was"])

    def test_paused_production_chain_stays_separate_and_unprovisioned(self):
        paused = self.manifest["retired_workflows"]["retired/workflows/rust-collector-release.yml"]
        self.assertEqual(paused["disposition"], "retired-restorable")
        self.assertEqual(paused["publication_state"], "paused-not-migrated")
        self.assertEqual(paused["owner"], "gatehouse")
        self.assertEqual(paused["revival_owner"], "gateprobes")
        archive = ROOT / "retired/workflows/rust-collector-release.yml"
        self.assertFalse((ROOT / paused["was"]).exists())
        self.assertTrue((ROOT / paused["retirement_record"]).is_file())
        self.assertEqual(hashlib.sha256(archive.read_bytes()).hexdigest(), paused["sha256"])
        source = archive.read_text()
        self.assertIn("Legacy collector signing/publication is suspended", source)
        self.assertIn("github.repository == 'musutrade/Harness-Gate'", source)

    def test_generated_copies_pin_one_source_and_exact_bytes(self):
        copies = self.manifest["generated_copies"]
        self.assertEqual(set(copies), {
            "tools/release/release_policy.py",
            "tools/release/release_inventory.py",
        })
        for source, row in copies.items():
            path = ROOT / source
            self.assertTrue(path.is_file(), source)
            self.assertEqual(row["plugin_repository"], "musutrade/GateProbes")
            self.assertTrue(row["plugin_path"].endswith(path.name), source)
            self.assertRegex(row["revision"], r"^[0-9a-f]{40}$")
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertEqual(actual, row["sha256"], source)


if __name__ == "__main__":
    unittest.main()
