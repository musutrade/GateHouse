from __future__ import annotations

import copy
import json
from decimal import Decimal
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from production_coverage import INVENTORY, REQUIRED, load_inventory, meets_threshold, summarize, test_ranges
from quality_common import CRATE


class ProductionCoverageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.raw = self.root / "raw.json"
        self.lcov = self.root / "coverage.lcov"
        self.inventory_path = self.root / "inventory.json"
        self.inventory = json.loads(INVENTORY.read_text())
        self.inventory["boundaries"] = {
            name: {"blocking": name in REQUIRED, "files": [f"quality-core/{name}.rs" if name == "quality-core" else f"src/{name}.rs"]}
            for name in sorted(REQUIRED | {"service-adapters", "other"})}
        self.inventory["exclusions"] = []
        self.inventory["unmapped_sources"] = []
        self.report = {"type": "llvm.coverage.json.export", "data": [{"files": [], "functions": []}]}
        self.line_records = {}
        for boundary in self.inventory["boundaries"].values():
            self.add_source(boundary["files"][0])

    def add_source(self, relative, production_hits=(1, 1, 1, 0, 1)):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('pub fn production() {\n let x = 1;\n if x == 2 {\n panic!("dead");\n}}\n'
                        '#[cfg(test)]\nmod tests {\n fn test_only() {}\n}\n')
        lines = {i: hit for i, hit in enumerate(production_hits, 1)} | {8: 1}
        self.line_records[str(path)] = lines
        self.report["data"][0]["files"].append({"filename": str(path), "summary": {
            "lines": {"count": len(lines), "covered": sum(n > 0 for n in lines.values())},
            "functions": {"count": 2, "covered": 2}, "regions": {"count": 3, "covered": 2}}})
        self.sync_segments(str(path))
        for start, end, regions in [(1, 5, [[4, 1, 4, 16, 0, 0, 0, 0]]), (8, 8, [])]:
            self.report["data"][0]["functions"].append({"filenames": [str(path)],
                "name": f"{relative}:{start}", "count": 1,
                "regions": [[start, 1, end, 2, 1, 0, 0, 0], *regions]})

    def sync_segments(self, path):
        item = next(f for f in self.report["data"][0]["files"] if f["filename"] == path)
        item["segments"] = [segment for line, count in sorted(self.line_records[path].items())
                            for segment in ([line, 1, count, True, True, False],
                                            [line, 99, 0, False, False, False])]

    def write(self):
        self.inventory_path.write_text(json.dumps(self.inventory))
        self.raw.write_text(json.dumps(self.report))
        self.lcov.write_text("".join("SF:" + path + "\n" + "".join(
            f"DA:{line},{count}\n" for line, count in lines.items()) + "end_of_record\n"
            for path, lines in self.line_records.items()))

    def evaluate(self):
        self.write()
        return summarize(self.raw, self.lcov, self.inventory_path, self.root)

    def test_checked_in_inventory_covers_every_source_once(self):
        inventory, owners, excluded = load_inventory(INVENTORY, CRATE)
        self.assertEqual(owners["src/service/lease.rs"], "service-core")
        self.assertEqual(owners["src/service/docker.rs"], "service-core")
        self.assertEqual(owners["src/service/commands.rs"], "service-core")
        self.assertEqual(owners["src/service/inspection.rs"], "service-core")
        self.assertEqual(owners["src/service/runtime.rs"], "service-adapters")
        self.assertIn("src/scope/benchmark.rs", excluded)
        self.assertTrue(inventory["unmapped_sources"])
        self.assertEqual(owners["quality-core/policy.rs"], "quality-core")
        self.assertEqual(owners["quality-core/replay.rs"], "quality-core")
        self.assertEqual(owners["src/process/replay.rs"], "process")

    def test_exact_threshold_passes_and_keeps_separate_raw_metrics(self):
        result = self.evaluate()
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["aggregate"]["lines"], {"covered": 4 * len(REQUIRED), "count": 5 * len(REQUIRED), "percent": 80.0})
        self.assertEqual(result["aggregate"]["functions"]["count"], len(REQUIRED))
        self.assertEqual(result["aggregate"]["regions"]["count"], 2 * len(REQUIRED))
        self.assertEqual(result["boundaries"]["service-adapters"]["status"], "informational")

    def test_outside_src_production_is_blocking_and_uninventoried_sources_fail(self):
        path = str(self.root / "quality-core/quality-core.rs")
        self.line_records[path][1] = 0
        self.sync_segments(path)
        item = next(f for f in self.report["data"][0]["files"] if f["filename"] == path)
        item["summary"]["lines"]["covered"] -= 1
        self.assertIn("quality-core", self.evaluate()["failures"])
        self.add_source("quality-core/unlisted.rs")
        with self.assertRaisesRegex(ValueError, "source inventory mismatch"):
            self.evaluate()

    def test_threshold_never_rounds_up_or_accepts_invalid_values(self):
        self.assertFalse(meets_threshold(799999, 1000000))
        self.assertTrue(meets_threshold(800000, 1000000))
        self.assertTrue(meets_threshold(800001, 1000000))
        self.assertFalse(meets_threshold(0, 0))
        for threshold in ["NaN", "Infinity", "79.9", "101"]:
            with self.subTest(threshold=threshold), self.assertRaises(ValueError):
                meets_threshold(100, 100, Decimal(threshold))

    def test_boundary_failure_cannot_be_diluted_by_other_boundaries(self):
        path = str(self.root / "src/app.rs")
        self.line_records[path][1] = 0
        self.sync_segments(path)
        next(f for f in self.report["data"][0]["files"] if f["filename"] == path)["summary"]["lines"]["covered"] -= 1
        result = self.evaluate()
        self.assertIn("app", result["failures"])
        self.assertEqual(result["status"], "fail")
        # Even perfect informational files cannot contribute to the aggregate.
        self.assertEqual(result["aggregate"]["lines"]["covered"], 4 * len(REQUIRED) - 1)

    def test_test_only_additions_do_not_change_production_counts(self):
        before = self.evaluate()["aggregate"]
        path = self.root / "src/app.rs"
        with path.open("a") as stream:
            stream.write('#[test]\nfn another_test() {}\n')
        self.line_records[str(path)][11] = 1
        self.sync_segments(str(path))
        item = next(f for f in self.report["data"][0]["files"] if f["filename"] == str(path))
        for metric in ("lines", "functions", "regions"):
            item["summary"][metric]["count"] += 1
            item["summary"][metric]["covered"] += 1
        self.report["data"][0]["functions"].append({"name": "extra-test", "filenames": [str(path)],
            "count": 1, "regions": [[11, 1, 11, 22, 1, 0, 0, 0]]})
        self.assertEqual(self.evaluate()["aggregate"], before)

    def test_unhit_production_functions_remain_in_denominator(self):
        function = self.report["data"][0]["functions"][0]
        function["count"] = 0
        for region in function["regions"]:
            region[4] = 0
        result = self.evaluate()
        self.assertEqual(result["aggregate"]["functions"]["count"], len(REQUIRED))
        self.assertEqual(result["aggregate"]["functions"]["covered"], len(REQUIRED) - 1)

    def test_instances_are_merged_by_source_location(self):
        function = copy.deepcopy(self.report["data"][0]["functions"][0])
        function["name"] += "-instantiation"
        self.report["data"][0]["functions"].append(function)
        self.assertEqual(self.evaluate()["aggregate"]["functions"]["count"], len(REQUIRED))

    def test_exclusions_have_reasons_and_never_overlap(self):
        for kind in ["test", "generated", "benchmark-only"]:
            with self.subTest(kind=kind):
                self.inventory["exclusions"] = [{"path": "src/app.rs", "kind": kind, "reason": "fixture"}]
                with self.assertRaisesRegex(ValueError, "duplicate exclusion"):
                    self.evaluate()

    def test_excluded_files_cannot_inflate_production_metrics(self):
        before = self.evaluate()["aggregate"]
        for kind in ("test", "generated", "benchmark-only"):
            source = f"src/{kind}.rs"
            self.add_source(source, production_hits=(1, 1, 1, 1, 1))
            self.inventory["exclusions"].append({"path": source, "kind": kind, "reason": "Non-production fixture"})
        self.assertEqual(self.evaluate()["aggregate"], before)
        self.inventory["exclusions"][0]["reason"] = ""
        with self.assertRaisesRegex(ValueError, "invalid exclusion reason"):
            self.evaluate()

    def test_boundary_with_only_test_mappings_fails(self):
        path = str(self.root / "src/app.rs")
        (self.root / "src/app.rs").write_text("\n" * 5 + '#[cfg(test)]\nmod tests {\n fn test_only() {}\n}\n')
        self.line_records[path] = {8: 1}
        self.sync_segments(path)
        item = next(f for f in self.report["data"][0]["files"] if f["filename"] == path)
        item["summary"] = {name: {"count": 1, "covered": 1} for name in ("lines", "functions", "regions")}
        self.report["data"][0]["functions"] = [f for f in self.report["data"][0]["functions"]
                                                if f["filenames"] != [path] or f["regions"][0][0] == 8]
        result = self.evaluate()
        self.assertEqual(result["boundaries"]["app"]["lines"]["count"], 0)
        self.assertIn("app", result["failures"])

    def test_negative_inventory_and_raw_reports(self):
        cases = {
            "empty boundary": lambda: self.inventory["boundaries"]["app"].update(files=[]),
            "duplicate assignment": lambda: self.inventory["boundaries"]["other"]["files"].append("src/app.rs"),
            "missing or changed blocking": lambda: self.inventory["boundaries"]["app"].update(blocking=False),
            "inventory mismatch": lambda: (self.root / "src/new.rs").write_text("fn new() {}"),
            "missing raw source": lambda: self.report["data"][0]["files"].pop(0),
            "duplicate raw file": lambda: self.report["data"][0]["files"].append(self.report["data"][0]["files"][0]),
            "unknown coverage path": lambda: self.report["data"][0]["files"].append({"filename": str(self.root / "unknown.rs")}),
            "empty raw coverage": lambda: self.report["data"][0].update(functions=[]),
            "missing/ambiguous functions": lambda: self.report["data"][0]["functions"].pop(0),
            "missing LCOV source": lambda: self.line_records.pop(next(iter(self.line_records))),
            "line counts disagree": lambda: self.line_records[next(iter(self.line_records))].update({1: 9, 4: 1}),
            "missing/ambiguous regions": lambda: self.report["data"][0]["functions"][0]["regions"].pop(),
        }
        for message, change in cases.items():
            with self.subTest(message=message):
                original = (copy.deepcopy(self.inventory), copy.deepcopy(self.report), copy.deepcopy(self.line_records))
                change()
                with self.assertRaisesRegex(ValueError, message):
                    self.evaluate()
                self.inventory, self.report, self.line_records = original
                (self.root / "src/new.rs").unlink(missing_ok=True)

    def test_missing_raw_reports_fail_without_collecting(self):
        self.write()
        self.raw.unlink()
        with self.assertRaises(FileNotFoundError):
            summarize(self.raw, self.lcov, self.inventory_path, self.root)
        result = subprocess.run([sys.executable, str(INVENTORY.with_name("coverage.py")), "--production",
                                 "--raw", str(self.raw), "--lcov", str(self.lcov),
                                 "--output", str(self.root / "out.json")], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(json.loads((self.root / "out.json").read_text())["status"], "fail")

    def test_test_ranges_handle_literals_helpers_and_production_after_tests(self):
        source = '#[cfg(test)]\nmod tests {\n const S: &str = r#" } "#;\n fn helper() {}\n}\nfn live() {}\n'
        self.assertEqual(test_ranges(source), [(1, 5)])
        self.assertEqual(test_ranges('#[cfg(not(test))]\nfn live() {}'), [])
        self.assertEqual(test_ranges('#[cfg(all(test, target_os = "linux"))]\nmod tests {}'), [(1, 2)])
        self.assertEqual(test_ranges('fn live() { let s = r##" [" ] } "##; let c = b\'}\'; }'), [])
        with self.assertRaisesRegex(ValueError, "compound test cfg"):
            test_ranges('#[cfg(any(test, unix))]\nfn mixed() {}')


if __name__ == "__main__":
    unittest.main()
