"""Every fixture has one owner; core tests never reach a plugin fixture."""
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "docs/quality/fixture-ownership.json"
FIXTURES = ROOT / "tools/quality/fixtures"
CORE_CRATE = ROOT / "tools/harness-gate"


def load_manifest():
    return json.loads(MANIFEST.read_text())


class FixtureOwnershipTests(unittest.TestCase):
    def setUp(self):
        self.manifest = load_manifest()
        self.owners = {name: list(paths) for name, paths in self.manifest["owners"].items()}

    def test_every_fixture_entry_has_exactly_one_owner(self):
        seen = {}
        for owner, paths in self.owners.items():
            for path in paths:
                self.assertNotIn(path, seen, path)
                seen[path] = owner
        for path, owner in seen.items():
            self.assertIn(owner, {"core", "plugin"})
            self.assertTrue((ROOT / path).exists(), path)

    def test_every_top_level_fixture_is_classified(self):
        classified = {path for paths in self.owners.values() for path in paths}
        actual = {
            child.relative_to(ROOT).as_posix()
            for child in FIXTURES.iterdir()
            if not child.name.startswith(".")
        }
        self.assertEqual(classified, actual)

    def test_core_source_does_not_reference_plugin_fixtures(self):
        plugin_names = {
            Path(path).name for path in self.owners.get("plugin", [])
        }
        offenders = []
        for source in CORE_CRATE.rglob("*.rs"):
            text = source.read_text(errors="replace")
            for name in sorted(plugin_names):
                if f"fixtures/{name}" in text:
                    offenders.append((source.relative_to(ROOT).as_posix(), name))
        self.assertEqual(offenders, [])

    def test_frozen_python_equivalence_inputs_stay_with_core(self):
        # D11 keeps these historical replay consumers in Core. Inspect their
        # literal inputs without importing/running the replay or collectors.
        for source in (
            "tools/quality/rust_equivalence.py",
            "tools/quality/tests/test_rust_reference.py",
        ):
            references = re.findall(
                r"['\"](tools/quality/fixtures/[^'\"]+)['\"]",
                (ROOT / source).read_text(),
            )
            self.assertTrue(references, source)
            for reference in references:
                with self.subTest(source=source, fixture=reference):
                    self.assertTrue((ROOT / reference).is_file(), reference)
                    owners = [
                        owner for owner, paths in self.owners.items()
                        if any(Path(reference).is_relative_to(path) for path in paths)
                    ]
                    self.assertEqual(owners, ["core"], reference)


if __name__ == "__main__":
    unittest.main()
