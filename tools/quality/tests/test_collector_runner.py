import copy
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collector_runner as runner
import harness_evidence as evidence

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures/collectors"
spec = importlib.util.spec_from_file_location("synthetic_collector", FIXTURES / "synthetic.py")
synthetic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(synthetic)


class CollectorRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name).resolve()
        self.project = evidence.load_json(ROOT / "fixtures/project-model/base.json")
        self.records = evidence.load_json(ROOT / "fixtures/harness-evidence/polyglot.json")
        record = self.records[0]
        self.request = {
            "schema": "harness-collector-request/v1", "project": self.project["id"],
            "component": record["component"], "collector": record["collector"],
            "context": record["context"],
            "requested_capabilities": ["coverage.line", "coverage.branch"],
            "workspace_root": str((ROOT / "fixtures/project-model/sources").resolve()),
            "output_root": str(self.output), "parameters": {},
        }

    def run_adapter(self, adapter=None):
        return runner.run_collector(adapter or runner.InternalAdapter(synthetic.collect),
                                    self.request, project=self.project)

    def assert_failure(self, code, adapter=None):
        with self.assertRaises(runner.CollectionError) as caught:
            self.run_adapter(adapter)
        self.assertEqual(caught.exception.as_dict()["code"], code)
        self.assertTrue(caught.exception.as_dict()["message"])

    def test_synthetic_cases_through_both_transports(self):
        for case in evidence.load_json(FIXTURES / "cases.json"):
            for external in (False, True):
                if case.get("external_only") and not external:
                    continue
                with self.subTest(scenario=case["scenario"], external=external):
                    with tempfile.TemporaryDirectory(dir=self.temp.name) as output:
                        self.request["output_root"] = output
                        self.request["parameters"] = {"scenario": case["scenario"]}
                        adapter = (runner.SubprocessAdapter(
                            (sys.executable, str(FIXTURES / "synthetic.py")),
                            0.1 if case["scenario"] == "timeout" else 5)
                            if external else runner.InternalAdapter(synthetic.collect))
                        if "error" in case:
                            self.assert_failure(case["error"], adapter)
                        else:
                            records = self.run_adapter(adapter)
                            self.assertEqual(records, [self.records[0]])
                            if "state" in case:
                                branch = next(c for c in records[0]["capabilities"]
                                              if c["metric"] == "coverage.branch")
                                self.assertEqual(branch["state"], case["state"])
                                self.assertNotIn("coverage.branch", [m["name"]
                                                for m in records[0]["metrics"]])

    def test_availability_consumer_is_transport_independent(self):
        results = []
        for adapter in (runner.InternalAdapter(synthetic.collect), runner.SubprocessAdapter(
                (sys.executable, str(FIXTURES / "synthetic.py")))):
            with tempfile.TemporaryDirectory(dir=self.temp.name) as output:
                self.request["output_root"] = output
                records = self.run_adapter(adapter)
                requirements = {"schema": "capability-requirements/v1", "requirements": [
                    {"component": self.request["component"], "metric": metric,
                     "mode": "required", "on_unavailable": "blocked"}
                    for metric in self.request["requested_capabilities"]]}
                results.append(evidence.evaluate_requirements(
                    records, requirements, project=self.project,
                    source_root=self.request["workspace_root"], artifact_root=output,
                    expected=self.request["context"]))
        self.assertEqual(results[0], results[1])
        self.assertEqual([r["status"] for r in results[0]], ["available", "blocked"])

    def test_every_request_field_is_required_before_invocation(self):
        original = copy.deepcopy(self.request)
        def forbidden(request):
            self.fail("invalid request must not invoke collector")
        for field in original:
            with self.subTest(field=field):
                self.request = {k: v for k, v in original.items() if k != field}
                self.assert_failure("invalid_request", runner.InternalAdapter(forbidden))

    def test_internal_cannot_rewrite_caller_provenance(self):
        original = copy.deepcopy(self.request)
        def mutate(request):
            request["context"]["commit"] = "f" * 40
            request["parameters"]["scenario"] = "stale_commit"
            return synthetic.collect(request)
        self.assert_failure("stale_context", runner.InternalAdapter(mutate))
        self.assertEqual(self.request, original)

    def test_output_root_must_be_fresh(self):
        (self.output / "old.json").write_text("old evidence")
        self.assert_failure("invalid_request")

    def test_missing_executable_and_internal_exception_are_typed(self):
        self.assert_failure("subprocess_error", runner.SubprocessAdapter(("/missing/collector",)))
        def broken(request):
            raise RuntimeError("analyzer unavailable")
        self.assert_failure("adapter_error", runner.InternalAdapter(broken))

    def test_mixed_error_and_evidence_is_rejected(self):
        def mixed(request):
            response = synthetic.collect(request)
            response["error"] = {"code": "measurement_error", "message": "parse failed"}
            return response
        self.assert_failure("invalid_response", runner.InternalAdapter(mixed))

    def test_nonfinite_json_and_trailing_documents_are_rejected(self):
        for text in ('{"error": NaN}', '{} {}', '[]'):
            with self.subTest(text=text):
                adapter = runner.SubprocessAdapter((sys.executable, "-c", f"print({text!r})"))
                self.assert_failure("invalid_response" if text == '[]' else "malformed_json", adapter)

    def test_all_synthetic_components_preserve_retained_measurements(self):
        for record in self.records:
            with self.subTest(component=record["component"]):
                with tempfile.TemporaryDirectory(dir=self.temp.name) as output:
                    self.request.update(component=record["component"], collector=record["collector"],
                                        output_root=output)
                    self.assertEqual(self.run_adapter(), [record])

    def test_artifact_inventory_and_integrity_fail_closed(self):
        def missing_file(response):
            (Path(self.request["output_root"]) / response["artifacts"][0]["path"]).unlink()
        cases = [
            (missing_file, "missing_artifact"),
            (lambda r: r["artifacts"].append(copy.deepcopy(r["artifacts"][0])), "invalid_artifact"),
            (lambda r: r["artifacts"][0].update(bytes=0), "undeclared_artifact"),
            (lambda r: r["evidence"][0]["source"].update(sha256="f" * 64), "invalid_evidence"),
            (lambda r: r["evidence"][0]["series"].update(id="measurement-series/v1:" + "f" * 64),
             "invalid_evidence"),
        ]
        for mutate, code in cases:
            with self.subTest(code=code):
                with tempfile.TemporaryDirectory(dir=self.temp.name) as output:
                    self.request["output_root"] = output
                    def collect(request):
                        response = synthetic.collect(request)
                        mutate(response)
                        return response
                    self.assert_failure(code, runner.InternalAdapter(collect))

    def test_capability_measurement_error_remains_evidence_for_consumer(self):
        def collect(request):
            response = synthetic.collect(request)
            record = response["evidence"][0]
            record["metrics"] = [m for m in record["metrics"] if m["name"] != "coverage.line"]
            capability = next(c for c in record["capabilities"] if c["metric"] == "coverage.line")
            capability.update(state="measurement_error", reason="synthetic instrumentation failed")
            record["status"] = "measurement_error"
            return response
        records = self.run_adapter(runner.InternalAdapter(collect))
        self.assertEqual(records[0]["status"], "measurement_error")

    @unittest.skipUnless(os.name == "posix", "process-group cleanup is POSIX-specific")
    def test_timeout_kills_descendant_holding_stdout(self):
        adapter = runner.SubprocessAdapter((sys.executable, "-c",
            "import subprocess,sys,time; "
            "subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); "
            "time.sleep(30)"), 0.2)
        self.assert_failure("timeout", adapter)


if __name__ == "__main__":
    unittest.main()
