import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import project_model as model

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures/project-model"


class ProjectModelTests(unittest.TestCase):
    def setUp(self):
        self.base = model.load_project(FIXTURES / "base.json")
        self.head = model.load_project(FIXTURES / "head.json")
        self.mappings = json.loads((FIXTURES / "mappings.json").read_text())

    def reidentify(self, subject, project=None):
        subject["id"] = model.subject_id((project or self.base)["id"], subject)

    def test_polyglot_graph_and_source_digests_without_toolchains(self):
        self.assertEqual({c["metadata"]["language"] for c in self.base["components"]},
                         {"typescript", "rust", "python", "java"})
        self.assertEqual(len(self.base["relationships"]), 3)
        contract = self.base["subjects"][4]
        relation = self.base["relationships"][0]
        self.assertEqual((relation["producer"], relation["consumer"]), ("api", "frontend"))
        self.assertEqual(contract["kind"], "contract/v1")
        self.assertIn(contract["id"], relation["subjects"])
        for project, directory in ((self.base, "sources"), (self.head, "head-sources")):
            for subject in project["subjects"]:
                source = (FIXTURES / directory / subject["path"]).read_bytes()
                self.assertEqual(hashlib.sha256(source).hexdigest(), subject["source_sha256"])
        # Unrecognized ecosystem metadata needs no new core branch.
        self.base["components"][0]["metadata"] = {"language": "future-language"}
        model.validate_project(self.base)

    def test_same_short_names_remain_distinct_and_metadata_is_not_identity(self):
        subjects = self.base["subjects"][:4]
        self.assertEqual({s["metadata"]["short_name"] for s in subjects}, {"run"})
        self.assertEqual(len({s["id"] for s in subjects}), 4)
        subject = copy.deepcopy(subjects[0])
        subject["discriminator"] = "Other.run"
        self.reidentify(subject)
        self.assertNotEqual(subject["id"], subjects[0]["id"])
        self.base["subjects"].append(subject)
        model.validate_project(self.base)
        subject["metadata"] = {"framework": "another", "metadata": "descriptive"}
        self.assertEqual(model.subject_id(self.base["id"], subject), subject["id"])
        self.assertNotEqual(model.subject_id("other-project", subject), subject["id"])

    def test_missing_digest_unknown_kinds_and_noncanonical_ids_fail(self):
        for field, value in (("source_sha256", None), ("source_sha256", "z" * 64),
                             ("source_sha256", "a" * 64 + "\n"),
                             ("kind", "unknown/v1"), ("kind", "method/v2"),
                             ("identity_version", "subject-identity/v2"),
                             ("id", "subject-identity/v1:" + "0" * 64),
                             ("metadata", {"x": 1}), ("discriminator", " run")):
            with self.subTest(field=field, value=value):
                project = copy.deepcopy(self.base)
                if value is None:
                    del project["subjects"][0][field]
                else:
                    project["subjects"][0][field] = value
                with self.assertRaises(model.ModelError):
                    model.validate_project(project)

    def test_duplicate_entities_and_unknown_graph_references_fail(self):
        for field in ("components", "subjects", "relationships"):
            project = copy.deepcopy(self.base)
            project[field].append(copy.deepcopy(project[field][0]))
            with self.subTest(field=field), self.assertRaisesRegex(model.ModelError, "duplicate"):
                model.validate_project(project)
        mutations = [
            lambda p: p["subjects"][0].update(component="missing"),
            lambda p: p["subjects"][0].update(target="missing"),
            lambda p: p["subjects"][0].update(boundary="missing"),
            lambda p: p["components"][0]["targets"][0]["boundaries"].append("missing"),
            lambda p: p["relationships"][0].update(consumer="missing"),
            lambda p: p["relationships"][0].update(consumer="api"),
            lambda p: p["relationships"][0]["subjects"].append("subject-identity/v1:" + "0" * 64),
            lambda p: p["relationships"][0]["subjects"].append(p["subjects"][3]["id"]),
            lambda p: p["components"][0]["targets"].append(p["components"][0]["targets"][0]),
            lambda p: p["components"][0]["source_boundaries"].append(p["components"][0]["source_boundaries"][0]),
        ]
        for mutate in mutations:
            project = copy.deepcopy(self.base)
            mutate(project)
            with self.assertRaises(model.ModelError):
                model.validate_project(project)

    def test_paths_spans_and_conflicting_source_identity_fail(self):
        for path in ("/frontend/src/run.ts", "frontend/src/../run.ts", "frontend//src/run.ts",
                     "frontend/./src/run.ts", "frontend\\src\\run.ts", "C:/run.ts",
                     "api/src/run.rs", "frontend/src/run.ts/", "frontend/src/cafe\u0301.ts"):
            project = copy.deepcopy(self.base)
            project["subjects"][0]["path"] = path
            with self.subTest(path=path), self.assertRaises(model.ModelError):
                self.reidentify(project["subjects"][0])
                model.validate_project(project)
        for span in ({"start_line": 2, "start_column": 1, "end_line": 1, "end_column": 1},
                     {"start_line": True, "start_column": 1, "end_line": 1, "end_column": 1}):
            subject = copy.deepcopy(self.base["subjects"][0])
            subject["span"] = span
            with self.assertRaises(model.ModelError):
                self.reidentify(subject)
        subject = copy.deepcopy(self.base["subjects"][0])
        subject["source_sha256"] = "a" * 64
        self.reidentify(subject)
        self.base["subjects"].append(subject)
        with self.assertRaisesRegex(model.ModelError, "conflicting source"):
            model.validate_project(self.base)

    def test_explicit_rename_move_split_and_unchanged_lineage(self):
        resolved = model.validate_mappings(self.base, self.head, self.mappings)
        self.assertEqual(len(resolved), 4)
        for mapping in self.mappings["mappings"]:
            for identity in mapping["to"]:
                self.assertEqual(model.baseline_identity(self.base, self.head, self.mappings, identity),
                                 mapping["from"])
        unchanged = self.base["subjects"][3]["id"]
        self.assertEqual(model.baseline_identity(self.base, self.head, self.mappings, unchanged), unchanged)

    def test_no_implicit_favorable_baseline_inheritance(self):
        empty = {**self.mappings, "mappings": []}
        for mapping in self.mappings["mappings"]:
            for identity in mapping["to"]:
                with self.assertRaisesRegex(model.ModelError, "explicit identity mapping required"):
                    model.baseline_identity(self.base, self.head, empty, identity)
        # Even same symbol/path with different source bytes cannot inherit.
        head = copy.deepcopy(self.base)
        head["relationships"] = []
        subject = head["subjects"][0]
        subject["source_sha256"] = "a" * 64
        self.reidentify(subject)
        with self.assertRaisesRegex(model.ModelError, "explicit identity mapping required"):
            model.baseline_identity(self.base, head, empty, subject["id"])
        with self.assertRaisesRegex(model.ModelError, "unknown head subject"):
            model.baseline_identity(self.base, self.head, empty, "missing")

    def test_unknown_ambiguous_and_invalid_mappings_fail(self):
        mutations = [
            lambda m: m.update(project="other"),
            lambda m: m["mappings"].append(copy.deepcopy(m["mappings"][0])),
            lambda m: m["mappings"][0].update(kind="merge"),
            lambda m: m["mappings"][0].update(kind="split"),
            lambda m: m["mappings"][0].update(kind="move"),
            lambda m: m["mappings"][1].update(kind="rename"),
            lambda m: m["mappings"][0].update(to=[self.base["subjects"][3]["id"]]),
            lambda m: m["mappings"][0].update(to=["subject-identity/v1:" + "0" * 64]),
            lambda m: m["mappings"][0].update({"from": "subject-identity/v1:" + "0" * 64}),
            lambda m: m["mappings"][0].update({"from": self.base["subjects"][3]["id"]}),
            lambda m: m["mappings"][2]["to"].append(m["mappings"][2]["to"][0]),
            lambda m: m["mappings"][1].update(to=m["mappings"][2]["to"][:1]),
        ]
        for mutate in mutations:
            mappings = copy.deepcopy(self.mappings)
            mutate(mappings)
            with self.assertRaises(model.ModelError):
                model.validate_mappings(self.base, self.head, mappings)

        # Two valid split shapes still cannot claim the same destination.
        mappings = copy.deepcopy(self.mappings)
        mappings["mappings"][1].update(kind="split", to=mappings["mappings"][2]["to"])
        with self.assertRaisesRegex(model.ModelError, "ambiguous mapping destination"):
            model.validate_mappings(self.base, self.head, mappings)

    def test_loader_rejects_duplicate_json_keys_and_cli_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "project.json"
            path.write_text('{"id":"one","id":"two"}')
            with self.assertRaisesRegex(model.ModelError, "duplicate JSON key"):
                model.load_project(path)
            result = subprocess.run([sys.executable, str(Path(model.__file__)), str(path)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn("duplicate JSON key", result.stderr)


if __name__ == "__main__":
    unittest.main()
