import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import harness_evidence as evidence

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures/harness-evidence"


class HarnessEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.records = evidence.load_json(FIXTURES / "polyglot.json")
        self.context = {
            "project": evidence.load_json(ROOT / "fixtures/project-model/base.json"),
            "source_root": ROOT / "fixtures/project-model/sources",
            "artifact_root": FIXTURES,
            "expected": evidence.load_json(FIXTURES / "expected.json"),
        }

    def validate(self, records=None):
        return evidence.validate_evidence(self.records if records is None else records,
                                          **self.context)

    def test_polyglot_values_and_raw_artifacts_preserve_tool_semantics(self):
        self.validate()
        self.assertEqual(len({r["series"]["id"] for r in self.records}), 4)
        for record in self.records:
            raw = evidence.load_json(FIXTURES / record["artifacts"][0]["path"])
            self.assertEqual(raw["tool"], record["series"]["tool"]["name"])
            self.assertEqual(raw["source"], record["source"])
            values = {m["name"]: m["value"] for m in record["metrics"]}
            self.assertEqual(values, raw["tool_native"]["values"])
            self.assertEqual({v["type"] for v in values.values()}, set(evidence.VALUE_TYPES) - {"rational"})
            self.assertEqual(values["coverage.line"], {"type": "ratio", "covered": 4, "total": 5})

    def test_negative_fixtures_fail_with_measurement_error(self):
        for fixture in evidence.load_json(FIXTURES / "negative.json"):
            with self.subTest(case=fixture["name"]):
                records = copy.deepcopy(self.records)
                parent = records[0]
                for key in fixture["path"][:-1]:
                    parent = parent[key]
                key = fixture["path"][-1]
                if fixture.get("delete"):
                    del parent[key]
                else:
                    parent[key] = fixture["value"]
                with self.assertRaisesRegex(evidence.MeasurementError, fixture["error"]):
                    self.validate(records)

    def test_every_required_envelope_field_is_required(self):
        for field in self.records[0]:
            with self.subTest(field=field):
                records = copy.deepcopy(self.records)
                del records[0][field]
                with self.assertRaises(evidence.MeasurementError):
                    self.validate(records)

    def test_duplicate_ids_metrics_capabilities_and_subject_series(self):
        for collection in (None, "metrics", "capabilities", "artifacts"):
            with self.subTest(collection=collection):
                records = copy.deepcopy(self.records)
                if collection is None:
                    records[1]["id"] = records[0]["id"]
                else:
                    records[0][collection].append(copy.deepcopy(records[0][collection][0]))
                with self.assertRaisesRegex(evidence.MeasurementError, "duplicate"):
                    self.validate(records)
        duplicate = copy.deepcopy(self.records[0])
        duplicate["id"] = "another-id"
        with self.assertRaisesRegex(evidence.MeasurementError, "duplicate subject/series"):
            self.validate(self.records + [duplicate])

    def test_canonical_serialization_roundtrip_and_key_order(self):
        canonical = evidence.canonical_serialize(self.records, **self.context)
        reordered = [{k: r[k] for k in reversed(r)} for r in self.records]
        self.assertEqual(canonical, evidence.canonical_serialize(reordered, **self.context))
        self.assertEqual(canonical, evidence.canonical_serialize(json.loads(canonical), **self.context))
        self.assertNotIn(b"\n", canonical)

    def test_json_duplicates_nonfinite_and_untyped_floats_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "invalid.json"
            for payload in ('{"x":1,"x":2}', '{"x":NaN}', '{"x":Infinity}'):
                path.write_text(payload)
                with self.assertRaises(evidence.MeasurementError):
                    evidence.load_json(path)
        for value in (0.0, 1.0, float("nan"), float("inf")):
            self.records[0]["metrics"][0]["value"] = value
            with self.assertRaises(evidence.MeasurementError):
                self.validate()

    def test_illegal_typed_values_and_unknown_metric_names(self):
        bad = [
            ("coverage.line", {"type": [], "covered": 4, "total": 5}),
            ("coverage.line", {"type": {}, "covered": 4, "total": 5}),
            ("coverage.line", {"type": "ratio", "covered": True, "total": 5}),
            ("coverage.line", {"type": "ratio", "covered": 0, "total": 0}),
            ("coverage.line", {"type": "ratio", "covered": -1, "total": 5}),
            ("coverage.line", {"type": "ratio", "covered": 4, "total": 5, "ratio": 1}),
            ("complexity.cyclomatic", {"type": "count", "value": True}),
            ("complexity.cyclomatic", {"type": "count", "value": -1}),
            ("contract.schema_valid", {"type": "boolean", "value": 1}),
            ("performance.duration", {"type": "duration", "value": 1, "unit": "seconds"}),
            ("bundle.size", {"type": "size", "value": 1, "unit": "kilobytes"}),
            ("risk.crap", {"type": "decimal", "value": "3.0"}),
            ("risk.crap", {"type": "decimal", "value": "NaN"}),
            ("risk.crap", {"type": "count", "value": 3}),
        ]
        for name, value in bad:
            with self.subTest(name=name, value=value):
                records = copy.deepcopy(self.records)
                next(m for m in records[0]["metrics"] if m["name"] == name)["value"] = value
                with self.assertRaises(evidence.MeasurementError):
                    self.validate(records)
        self.records[0]["series"]["metrics"][0]["name"] = "unknown.metric"
        with self.assertRaises(evidence.MeasurementError):
            self.validate()

    def test_non_supported_states_never_have_values_even_zero_or_one(self):
        for state in ("unsupported", "not_configured", "not_collected",
                      "measurement_error", "not_applicable"):
            for count in (0, 1):
                with self.subTest(state=state, count=count):
                    records = copy.deepcopy(self.records)
                    records[0]["capabilities"][0]["state"] = state
                    records[0]["metrics"][0]["value"] = {"type": "ratio", "covered": count, "total": 1}
                    with self.assertRaisesRegex(evidence.MeasurementError, "capability/evidence mismatch"):
                        self.validate(records)

    def test_supported_ratio_endpoints_keep_raw_counts(self):
        for count in (0, 1):
            self.records[0]["metrics"][0]["value"] = {"type": "ratio", "covered": count, "total": 1}
            canonical = evidence.canonical_serialize(self.records, **self.context)
            self.assertEqual(json.loads(canonical)[0]["metrics"][0]["value"]["covered"], count)

    def test_status_is_derived_from_capabilities(self):
        record = self.records[0]
        for capability in record["capabilities"]:
            if capability["state"] == "measurement_error":
                capability["state"] = "not_collected"
        record["status"] = "measured"
        self.validate()
        record["metrics"] = []
        for capability in record["capabilities"]:
            capability["state"] = "not_collected"
        record["status"] = "unavailable"
        self.validate()
        record["status"] = "measured"
        with self.assertRaisesRegex(evidence.MeasurementError, "status/capability mismatch"):
            self.validate()

    def test_series_changes_and_missing_base_prevent_comparison(self):
        base = self.records[0]["series"]
        evidence.require_compatible_series(base, copy.deepcopy(base))
        for field in ("collector", "tool", "rule", "runtime", "source_identity", "normalization"):
            with self.subTest(field=field):
                head = copy.deepcopy(base)
                head[field]["version"] = "2"
                head["id"] = evidence.series_id(head)
                self.assertNotEqual(base["id"], head["id"])
                with self.assertRaisesRegex(evidence.MeasurementError, "incompatible"):
                    evidence.require_compatible_series(base, head)
        for field, value in (("target", "other"), ("name", "new-series")):
            head = copy.deepcopy(base)
            head[field] = value
            head["id"] = evidence.series_id(head)
            with self.assertRaisesRegex(evidence.MeasurementError, "incompatible"):
                evidence.require_compatible_series(base, head)
        for other in self.records[1:]:
            with self.assertRaisesRegex(evidence.MeasurementError, "incompatible"):
                evidence.require_compatible_series(base, other["series"])
        with self.assertRaisesRegex(evidence.MeasurementError, "missing base"):
            evidence.require_compatible_series(None, base)

    def test_context_source_and_collector_mismatch(self):
        for field in self.context["expected"]:
            with self.subTest(field=field):
                context = copy.deepcopy(self.context)
                context["expected"][field] = "f" * 40 if "commit" in field else "other"
                with self.assertRaisesRegex(evidence.MeasurementError, "stale"):
                    evidence.validate_evidence(self.records, **context)
        for field in ("source", "collector"):
            records = copy.deepcopy(self.records)
            records[0][field]["sha256" if field == "source" else "version"] = "0" * 64
            with self.assertRaisesRegex(evidence.MeasurementError, "mismatch"):
                self.validate(records)

    def test_artifact_escape_symlink_tamper_and_size(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "root"
            root.mkdir()
            outside = Path(directory) / "outside"
            outside.write_bytes(b"raw")
            (root / "link").symlink_to(outside)
            for path in ("../outside", str(outside), "link", "./raw", "a/../raw", "C:/raw", "a\\raw"):
                with self.subTest(path=path), self.assertRaises(evidence.MeasurementError):
                    evidence._file(root, path, hashlib.sha256(b"raw").hexdigest())
            (root / "raw").write_bytes(b"tampered")
            with self.assertRaisesRegex(evidence.MeasurementError, "digest mismatch"):
                evidence._file(root, "raw", hashlib.sha256(b"raw").hexdigest())
        self.records[0]["artifacts"][0]["bytes"] += 1
        with self.assertRaisesRegex(evidence.MeasurementError, "byte count"):
            self.validate()

    def test_stale_source_bytes_and_artifact_source_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            context = dict(self.context, source_root=Path(directory))
            source = Path(directory) / self.records[0]["source"]["path"]
            source.parent.mkdir(parents=True)
            source.write_text("changed source")
            with self.assertRaisesRegex(evidence.MeasurementError, "digest mismatch"):
                evidence.validate_evidence(self.records, **context)
        self.records[0]["artifacts"][0]["source"]["sha256"] = "f" * 64
        with self.assertRaisesRegex(evidence.MeasurementError, "stale artifact"):
            self.validate()

    def test_artifact_links_and_series_capability_set(self):
        for path in (("metrics", "artifacts"), ("capabilities", "artifacts")):
            records = copy.deepcopy(self.records)
            records[0][path[0]][0][path[1]] = ["unknown"]
            with self.assertRaises(evidence.MeasurementError):
                self.validate(records)
        self.records[0]["capabilities"].pop()
        with self.assertRaisesRegex(evidence.MeasurementError, "series/capability mismatch"):
            self.validate()

    def test_same_requirement_policy_reads_each_ecosystem(self):
        requirements = evidence.load_json(FIXTURES / "requirements.json")
        results = evidence.evaluate_requirements(self.records, requirements, **self.context)
        self.assertEqual(len(results), 12)
        for result in results:
            self.assertEqual(result["status"], {
                "coverage.line": "available", "coverage.branch": "blocked",
                "coverage.region": "not_collected"}[result["metric"]])
        for requirement in requirements["requirements"]:
            requirement["on_unavailable"] = "measurement_error"
        results = evidence.evaluate_requirements(self.records, requirements, **self.context)
        self.assertTrue(all(r["status"] == "measurement_error" for r in results
                            if r["metric"] == "coverage.branch"))

    def test_required_and_informational_states_remain_distinct(self):
        record = self.records[0]
        for capability in record["capabilities"]:
            for mode in ("required", "informational"):
                requirements = {"schema": "capability-requirements/v1", "requirements": [{
                    "component": record["component"], "metric": capability["metric"],
                    "mode": mode, "on_unavailable": "blocked"}]}
                result = evidence.evaluate_requirements(self.records, requirements, **self.context)[0]
                self.assertEqual(result["state"], capability["state"])
                if mode == "informational":
                    self.assertEqual(result["status"], "available" if capability["state"] ==
                                     "supported" else capability["state"])
        requirements["requirements"][0].update(component="worker", metric="coverage.line", mode="required")
        result = evidence.evaluate_requirements([record], requirements, **self.context)[0]
        self.assertEqual(result["status"], "measurement_error")
        record["metrics"].pop(0)
        with self.assertRaisesRegex(evidence.MeasurementError, "capability/evidence mismatch"):
            evidence.evaluate_requirements(self.records, requirements, **self.context)

    def test_cli_retains_success_and_failure_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "validation.json"
            command = [sys.executable, str(ROOT / "harness_evidence.py"), str(FIXTURES / "polyglot.json"),
                       "--project", str(ROOT / "fixtures/project-model/base.json"),
                       "--source-root", str(self.context["source_root"]), "--artifact-root", str(FIXTURES),
                       "--expected", str(FIXTURES / "expected.json"), "--output", str(output)]
            self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
            self.assertEqual(json.loads(output.read_text())["status"], "valid")
            command[2] = str(Path(directory) / "missing.json")
            self.assertEqual(subprocess.run(command, capture_output=True).returncode, 1)
            self.assertEqual(json.loads(output.read_text())["status"], "measurement_error")


if __name__ == "__main__":
    unittest.main()
