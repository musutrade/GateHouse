"""One editable source per schema role; generated copies stay byte-identical."""
import json
import unittest
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "docs/quality/schema-ownership.json"


def load_manifest():
    return json.loads(MANIFEST.read_text())


class SchemaOwnershipTests(unittest.TestCase):
    def setUp(self):
        self.manifest = load_manifest()
        self.roles = self.manifest["roles"]
        self.generated = {str(k): str(v) for k, v in self.manifest["generated"].items()}
        self.roots = [Path(role["root"]) for role in self.roles.values()]

    def occurrences(self):
        found = defaultdict(list)
        for root in self.roots:
            for path in sorted((ROOT / root).glob("*.schema.json")):
                found[path.name].append(path.relative_to(ROOT).as_posix())
        return found

    def test_roles_have_distinct_roots_and_owners(self):
        roots = [role["root"] for role in self.roles.values()]
        self.assertEqual(len(roots), len(set(roots)))
        for role in self.roles.values():
            self.assertIn(role["owner"], {"core", "plugin"})
            self.assertTrue((ROOT / role["root"]).is_dir(), role["root"])

    def test_duplicated_basename_has_exactly_one_editable_source(self):
        for name, paths in self.occurrences().items():
            if len(paths) < 2:
                continue
            sources = [path for path in paths if path not in self.generated]
            self.assertEqual(len(sources), 1, (name, paths))

    def test_generated_copies_match_their_source(self):
        for copy, source in self.generated.items():
            copy_path = ROOT / copy
            source_path = ROOT / source
            self.assertTrue(copy_path.is_file(), copy)
            self.assertTrue(source_path.is_file(), source)
            self.assertNotIn(source, self.generated, source)
            self.assertEqual(source_path.name, copy_path.name, (source, copy))
            self.assertEqual(copy_path.read_bytes(), source_path.read_bytes(), copy)

    def test_every_duplicate_is_marked_generated(self):
        for name, paths in self.occurrences().items():
            if len(paths) < 2:
                continue
            for path in paths:
                if path in self.generated:
                    continue
                for other in paths:
                    if other == path:
                        continue
                    self.assertEqual(self.generated.get(other), path, (name, path, other))


if __name__ == "__main__":
    unittest.main()
