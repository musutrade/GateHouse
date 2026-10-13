from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from complexity_analyzer import analyze_source
from function_risk import map_functions, span_key
from production_coverage import INVENTORY, REQUIRED
from quality_common import sha256
from risk import TOOLS, digest, main, snapshot, validate_run

COMMIT = "a" * 40


class RiskBundleTests(unittest.TestCase):
    """Synthetic provenance fixtures exercise validation, not baseline acceptance."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.crate = self.root / "crate"
        self.sources = {}
        self.manifest_path = self.root / "bundle.json"
        inventory = json.loads(INVENTORY.read_text())
        inventory.update(boundaries={}, exclusions=[], unmapped_sources=[])
        raw = {"type": "llvm.coverage.json.export", "data": [{"files": [], "functions": []}]}
        complexity, lcov = [], []
        for boundary in sorted(REQUIRED | {"service-adapters"}):
            relative = f"src/{boundary}.rs"
            source = "pub fn production() { }\n"
            self.sources[relative] = source.encode()
            file = self.crate / relative
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(source)
            inventory["boundaries"][boundary] = {"blocking": boundary in REQUIRED, "files": [relative]}
            record = analyze_source(source, relative)
            record["commit"] = COMMIT
            complexity.append(self.json_artifact(f"complexity/{boundary}.json", record))
            span = span_key(record["symbols"][0]["span"])
            raw["data"][0]["functions"].append({"name": boundary, "count": 1,
                "filenames": [str(file)], "regions": [[*span, 1, 0, 0, 0]]})
            raw["data"][0]["files"].append({"filename": str(file),
                "summary": {metric: {"count": 1, "covered": 1} for metric in ("lines", "functions", "regions")},
                "segments": [[1, 1, 1, True, True, False], [1, 24, 0, False, False, False]]})
            lcov.append(f"SF:{file}\nDA:1,1\nend_of_record\n")
        results = self.artifact("results.xml", b'<testsuites><testsuite tests="1" failures="0" errors="0">'
                                b'<testcase classname="integration" name="failure_path"/></testsuite></testsuites>')
        binaries = [{"id": role, "role": role, "artifact": self.artifact(f"bin/{role}", b"\x7fELF fixture __llvm_prf")}
                    for role in ("cli", "test")]
        profiles = [{"binary_id": role, "run_id": "run1",
                     "artifact": self.artifact(f"profiles/run1/{role}.profraw", bytes.fromhex("8172666f72706cff0a00000000000000"))}
                    for role in ("cli", "test")]
        for profile in profiles:
            os.utime(self.root / profile["artifact"]["path"], ns=(10**18, 10**18))
        run = {"id": "run1", "started_ns": 10**18, "finished_ns": 10**18 + 10,
               "commit": COMMIT, "target": "fixture", "instrumentation": "-C instrument-coverage",
               "status": "passed", "command": "fixture nextest", "results": results,
               "binaries": binaries, "profiles": profiles}
        llvm = self.json_artifact("llvm.json", raw)
        lcov_entry = self.artifact("coverage.lcov", "".join(lcov).encode())
        cobertura = self.artifact("cobertura.xml", b'<coverage><packages/></coverage>')
        for entry in (llvm, lcov_entry, cobertura):
            entry.update(run_id="run1", profiles_sha256=digest(profiles))
        self.manifest = {"schema_version": 1, "commit": COMMIT, "target": "fixture",
                         "tools": {name: platform.python_version() if name == "python" else "fixture-v1" for name in TOOLS},
                         "profile": "dev", "instrumentation": "-C instrument-coverage",
                         "branch": {"status": "unsupported", "reason": "fixture", "tool_version": "fixture-v1"},
                         "source_root": "crate", "coverage_root": str(self.crate),
                         "inventory": self.json_artifact("inventory.json", inventory),
                         "llvm": llvm, "lcov": lcov_entry, "cobertura": cobertura, "complexity": complexity, "run": run}
        self.policy = {"schema_version": 1, "mandatory_symbols": [], "hotspot_symbols": [],
                       "identity_mappings": [], "boundary_tests": [], "exceptions": []}
        # Keep fixtures in the current workspace's test temp area. Git reads are
        # injected immutable objects; no secondary repository/checkout is made.
        self.addCleanup(patch.stopall)
        patch("risk.subprocess.check_output", return_value="\n".join(
            "tools/harness-gate/" + path for path in self.sources)).start()
        patch("risk.git_source", side_effect=lambda commit, path, repo: self.sources[path]).start()

    def artifact(self, relative, data):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return {"path": relative, "sha256": sha256(path)}

    def json_artifact(self, relative, value):
        return self.artifact(relative, json.dumps(value).encode())

    def snapshot(self):
        self.manifest_path.write_text(json.dumps(self.manifest))
        return snapshot(self.manifest_path, COMMIT)

    def test_complete_bundle_reproduces_and_cli_writes_both_reports(self):
        first = self.snapshot()
        self.assertEqual(first, self.snapshot())
        self.assertEqual(len(first["functions"]), len(REQUIRED) + 1)
        self.assertEqual(first["coverage"]["status"], "pass")
        policy = self.root / "policy.json"
        policy.write_text(json.dumps(self.policy))
        output = self.root / "risk.json"
        self.assertEqual(main(["--base", str(self.manifest_path), "--head", str(self.manifest_path),
                               "--base-sha", COMMIT, "--head-sha", COMMIT, "--policy", str(policy),
                               "--output", str(output)]), 0)
        report = json.loads(output.read_text())
        self.assertEqual(report["status"], "pass")
        self.assertIn("command", report)
        self.assertIn("[LLVM]", output.with_suffix(".md").read_text())
        self.assertTrue(all(f["evidence"]["llvm_sha256"] for f in report["functions"]))

    def test_stale_missing_and_incompatible_bundle_fields_fail_closed(self):
        cases = {
            "stale profile timestamp": lambda m: m["run"].update(started_ns=10**18 + 1),
            "unknown profile provenance": lambda m: m["run"]["profiles"][0].update(run_id="old"),
            "missing subprocess/test binary profiles": lambda m: m["run"]["profiles"].pop(),
            "missing CLI/test binary": lambda m: m["run"]["binaries"].pop(0),
            "coverage export from stale run": lambda m: m["llvm"].update(run_id="old"),
            "coverage profile set mismatch": lambda m: m["llvm"].update(profiles_sha256="f" * 64),
            "missing/unknown tool versions": lambda m: m["tools"].pop("rustc"),
            "complexity Python tool mismatch": lambda m: m["tools"].update(python="old"),
            "missing/mismatched base/head commit": lambda m: m.update(commit="b" * 40),
            "test run commit/target mismatch": lambda m: m["run"].update(target="other"),
            "missing/failed/cancelled test run": lambda m: m["run"].update(status="cancelled"),
            "stale/corrupt artifact": lambda m: m["llvm"].update(sha256="0" * 64),
            "artifact escapes": lambda m: m["llvm"].update(path="../outside"),
            "unsupported branch must not have counters": lambda m: m["branch"].update(percent=100),
        }
        original = copy.deepcopy(self.manifest)
        for message, mutate in cases.items():
            with self.subTest(message=message):
                self.manifest = copy.deepcopy(original)
                mutate(self.manifest)
                with self.assertRaisesRegex(ValueError, message):
                    self.snapshot()

    def test_source_snapshot_is_bound_to_git_bytes(self):
        next((self.crate / "src").glob("*.rs")).write_text("pub fn changed() {}\n")
        with self.assertRaisesRegex(ValueError, "source snapshot does not match commit"):
            self.snapshot()

    def test_relocated_source_snapshot_preserves_raw_artifacts(self):
        before = self.snapshot()
        shutil.copytree(self.crate, self.root / "relocated")
        self.manifest["source_root"] = "relocated"
        after = self.snapshot()
        self.assertEqual(before["series"], after["series"])
        self.assertEqual([f["lines"] for f in before["functions"]], [f["lines"] for f in after["functions"]])
        self.assertEqual(before["functions"][0]["evidence"]["llvm_sha256"], after["functions"][0]["evidence"]["llvm_sha256"])

    def test_required_fields_do_not_have_success_defaults(self):
        for field in ("run", "tools", "complexity", "inventory", "branch", "lcov", "cobertura", "coverage_root"):
            with self.subTest(field=field):
                original = self.manifest.pop(field)
                with self.assertRaises(KeyError):
                    self.snapshot()
                self.manifest[field] = original

    def test_failed_empty_or_fabricated_junit_cannot_claim_passing_run(self):
        for data in (b"invalid", b"<testsuites/>", b'<testsuite><testcase classname="x" name="y"><failure/></testcase></testsuite>',
                     b'<testsuite><testcase classname="x" name="y"><skipped/></testcase></testsuite>'):
            with self.subTest(data=data):
                self.manifest["run"]["results"] = self.artifact("results.xml", data)
                with self.assertRaises(ValueError):
                    validate_run(self.root, self.manifest)

    def test_uninstrumented_cli_is_measurement_error(self):
        self.manifest["run"]["binaries"][0]["artifact"] = self.artifact("bin/cli", b"\x7fELF plain binary")
        with self.assertRaisesRegex(ValueError, "no LLVM profile sections"):
            self.snapshot()

    def test_real_llvm_export_disjoint_regions_generic_closure_and_unhit(self):
        fixture = Path(__file__).resolve().parents[1] / "fixtures/risk"
        source = (fixture / "input.rs").read_text()
        path = self.crate / "src/real.rs"
        path.write_text(source)
        raw = json.loads((fixture / "llvm.json").read_text().replace("@SOURCE@", str(path)))
        record = analyze_source(source, "src/real.rs")
        record["commit"] = COMMIT
        functions = map_functions([record], raw, self.crate, {"src/real.rs": "other"}, COMMIT)
        by_name = {f["qualified_name"]: f for f in functions}
        self.assertEqual(len(by_name), 4)
        self.assertEqual(len(by_name["generic"]["instances"]), 2)
        self.assertEqual(len(by_name["generic::closure_1"]["instances"]), 2)
        self.assertEqual(by_name["generic"]["lines"], {"covered": 4, "count": 4, "percent": 100.0})
        self.assertEqual(by_name["unhit"]["lines"], {"covered": 0, "count": 1, "percent": 0.0})


if __name__ == "__main__":
    unittest.main()
