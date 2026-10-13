from __future__ import annotations

import copy
from datetime import date
from fractions import Fraction
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from complexity_analyzer import analyze_source
from function_risk import crap_line, map_functions, own_lines, span_key
from risk import evaluate, main, markdown

COMMIT = "a" * 40
SOURCE = '''mod one {
fn same<T>(x: T) {
    let f = || {
        if true { 1; }
    };
    f();
}
}
mod two {
fn same() {
    if false { 1; }
}
}
'''


class FunctionRiskTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = "src/lib.rs"
        (self.root / "src").mkdir()
        (self.root / self.path).write_text(SOURCE)
        self.record = analyze_source(SOURCE, self.path)
        self.record["commit"] = COMMIT
        self.llvm = {"type": "llvm.coverage.json.export", "data": [{"functions": []}]}
        for i, symbol in enumerate(self.record["symbols"]):
            span = span_key(symbol["span"])
            self.llvm["data"][0]["functions"].append({
                "name": f"mangled_{i}", "count": int(i < 2),
                "filenames": [str(self.root / self.path)],
                "regions": [[*span, int(i < 2), 0, 0, 0]]})

    def mapped(self):
        return map_functions([self.record], self.llvm, self.root, {self.path: "core"}, COMMIT)

    def snapshots(self):
        functions = self.mapped()
        for f in functions:
            f["evidence"] = {"llvm": "raw.json", "bundle": "manifest.json"}
        series = {"target": "fixture", "branch": {"status": "unsupported", "reason": "fixture", "tool_version": "fixture"}}
        snapshot = {"commit": COMMIT, "series": series, "series_id": "fixture", "functions": functions,
                    "coverage": {"status": "pass"}, "run": {"id": "run", "results": {"path": "tests.xml"},
                                                        "test_results": {"failure_test": "passed"}}}
        return copy.deepcopy(snapshot), snapshot

    def policy(self):
        return {"schema_version": 1, "identity_mappings": [], "mandatory_symbols": [],
                "hotspot_symbols": [], "boundary_tests": [], "exceptions": []}

    def test_same_names_closures_and_unhit_functions_are_separate(self):
        rows = self.mapped()
        self.assertEqual(len(rows), 3)
        self.assertEqual({r["qualified_name"] for r in rows}, {"one::same", "one::same::closure_1", "two::same"})
        self.assertEqual(next(r for r in rows if r["qualified_name"] == "two::same")["functions"]["covered"], 0)
        self.assertTrue(all(r["lines"]["count"] > 0 for r in rows))

    def test_generic_instantiations_deduplicate_source_locations(self):
        original = self.mapped()
        instance = copy.deepcopy(self.llvm["data"][0]["functions"][0])
        instance["name"] = "instantiation_two"
        instance["count"] = 0
        instance["regions"][0][4] = 0
        self.llvm["data"][0]["functions"].append(instance)
        rows = self.mapped()
        for before, after in zip(original, rows):
            for key in ("lines", "functions", "regions", "crap_line_exact"):
                self.assertEqual(before[key], after[key])
        self.assertEqual(len(rows[0]["instances"]), 2)

    def test_missing_llvm_function_fails(self):
        self.llvm["data"][0]["functions"].pop()
        with self.assertRaisesRegex(ValueError, "missing LLVM function"):
            self.mapped()

    def test_llvm_without_complexity_fails(self):
        function = copy.deepcopy(self.llvm["data"][0]["functions"][0])
        function["regions"] = [[99, 1, 100, 1, 0, 0, 0, 0]]
        self.llvm["data"][0]["functions"].append(function)
        with self.assertRaisesRegex(ValueError, "unmapped"):
            self.mapped()

    def test_omitted_complexity_cannot_hide_unmapped_production(self):
        self.record["symbols"].pop()
        with self.assertRaisesRegex(ValueError, "does not reproduce"):
            self.mapped()

    def test_source_digest_and_commit_must_match(self):
        (self.root / self.path).write_text(SOURCE + "\n")
        with self.assertRaisesRegex(ValueError, "digest"):
            self.mapped()
        (self.root / self.path).write_text(SOURCE)
        self.record["commit"] = "b" * 40
        with self.assertRaisesRegex(ValueError, "commit"):
            self.mapped()

    def test_closure_execution_cannot_cover_parent(self):
        regions = {(1, 1, 5, 2, 0): 0, (2, 1, 4, 2, 0): 9}
        lines = own_lines(regions, [(2, 1, 4, 2)])
        self.assertEqual(set(lines.values()), {0})
        self.assertNotIn(3, lines)

    def test_inner_regions_and_gaps_override_parent_counter(self):
        lines = own_lines({(1, 1, 8, 1, 0): 1, (2, 1, 4, 1, 0): 0,
                           (5, 1, 7, 1, 3): 0}, [])
        self.assertEqual(lines, {1: 1, 2: 0, 3: 0, 4: 1, 7: 1})

    def test_invalid_regions_fail(self):
        for field, value in ((4, -1), (5, 90), (7, 99)):
            with self.subTest(field=field):
                old = self.llvm["data"][0]["functions"][0]["regions"][0][field]
                self.llvm["data"][0]["functions"][0]["regions"][0][field] = value
                with self.assertRaises(ValueError):
                    self.mapped()
                self.llvm["data"][0]["functions"][0]["regions"][0][field] = old

    def test_unknown_nested_function_cannot_inherit_parent_mapping(self):
        self.llvm["data"][0]["functions"].append({"name": "nested", "count": 1,
            "filenames": [str(self.root / self.path)], "regions": [[4, 12, 4, 20, 1, 0, 0, 0]]})
        with self.assertRaisesRegex(ValueError, "unmapped"):
            self.mapped()

    def test_utf8_byte_columns_map_without_changing_source_identity(self):
        source = 'fn utf8() { let x = "雪"; }\n'
        (self.root / self.path).write_text(source)
        self.record = analyze_source(source, self.path)
        self.record["commit"] = COMMIT
        span = span_key(self.record["symbols"][0]["span"], source)
        self.llvm["data"][0]["functions"] = [{"name": "utf8", "count": 1,
            "filenames": [str(self.root / self.path)], "regions": [[*span, 1, 0, 0, 0]]}]
        self.assertEqual(self.mapped()[0]["qualified_name"], "utf8")

    def test_crap_math_uses_unrounded_fraction(self):
        self.assertEqual(crap_line(10, 80, 100), Fraction(54, 5))
        self.assertEqual(float(crap_line(10, 80, 100)), 10.8)
        # Both display as 30.000000, but only exact coverage can pass CC=30.
        self.assertEqual(crap_line(30, 1000000, 1000000), 30)
        self.assertGreater(crap_line(30, 999999, 1000000), 30)
        for counts in ((0, 0), (-1, 5), (6, 5), (True, 5)):
            with self.assertRaises(ValueError):
                crap_line(10, *counts)

    def test_changed_crap_blocks_unchanged_debt_is_separate(self):
        base, head = self.snapshots()
        head["functions"][0].update(cc=31)
        base["functions"][0].update(cc=31)
        report = evaluate(base, head, self.policy())
        self.assertEqual(report["status"], "pass-with-debt")
        self.assertEqual(report["functions"][0]["status"], "debt")
        head["functions"][0]["body_sha256"] = "changed"
        report = evaluate(base, head, self.policy())
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["functions"][0]["change"], "modified")

    def test_move_rechecked_even_with_explicit_mapping(self):
        base, head = self.snapshots()
        f = head["functions"][0]
        old_id = f["id"]
        f.update(id="moved", cc=31)
        f["source"]["path"] = "src/moved.rs"
        policy = self.policy()
        policy["identity_mappings"] = [{"base": old_id, "head": "moved", "reason": "move"}]
        report = evaluate(base, head, policy)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["functions"][0]["change"], "moved")
        policy["identity_mappings"] = []
        self.assertEqual(evaluate(base, head, policy)["functions"][0]["change"], "new")

    def test_line_shift_does_not_reclassify_unchanged_body(self):
        base, head = self.snapshots()
        head["functions"][0]["id"] = "shifted"
        head["functions"][0]["span"]["start_line"] += 10
        self.assertEqual(evaluate(base, head, self.policy())["functions"][0]["change"], "unchanged")

    def test_selected_hotspot_move_requires_mapping_and_keeps_boundary_rules(self):
        base, head = self.snapshots()
        previous = base["functions"][0]
        previous["source"]["path"] = "src/verify/parser.rs"
        previous["qualified_name"] = "count_json_results"
        policy = self.policy()
        with self.assertRaisesRegex(ValueError, "missing selected hotspot"):
            evaluate(base, head, policy)
        policy["identity_mappings"] = [{"base": previous["id"], "head": head["functions"][0]["id"],
                                        "reason": "relocate selected parser"}]
        report = evaluate(base, head, policy)
        self.assertEqual(report["status"], "fail")
        self.assertTrue(report["functions"][0]["high_risk"])
        self.assertIn("boundary test", " ".join(report["functions"][0]["violations"]))

    def test_splits_are_new_and_cannot_reuse_identity(self):
        base, head = self.snapshots()
        original = head["functions"][0]["id"]
        for i in (0, 1):
            head["functions"][i].update(id=f"split{i}", qualified_name=f"split{i}", cc=31)
        report = evaluate(base, head, self.policy())
        self.assertEqual([f["change"] for f in report["functions"][:2]], ["new", "new"])
        self.assertEqual(report["status"], "fail")
        policy = self.policy()
        policy["identity_mappings"] = [{"base": original, "head": f"split{i}", "reason": "split"} for i in (0, 1)]
        with self.assertRaisesRegex(ValueError, "one-to-one"):
            evaluate(base, head, policy)

    def test_high_risk_requires_each_metric_and_boundary_test(self):
        base, head = self.snapshots()
        f = head["functions"][0]
        f.update(cc=11, body_sha256="changed", lines={"covered": 80, "count": 100}, regions={"covered": 79, "count": 100})
        policy = self.policy()
        report = evaluate(base, head, policy)
        self.assertEqual(report["status"], "fail")
        self.assertIn("regions < 80%", " ".join(report["functions"][0]["violations"]))
        f["regions"]["covered"] = 80
        policy["boundary_tests"] = [{"symbol": f["id"], "test_id": "failure_test", "observable": "typed failure",
                                      "status": "passed", "commit": COMMIT, "target": "fixture", "run_id": "run",
                                      "results": head["run"]["results"]}]
        self.assertEqual(evaluate(base, head, policy)["status"], "pass")
        f["lines"]["covered"] = 79
        self.assertEqual(evaluate(base, head, policy)["status"], "fail")
        policy["boundary_tests"][0]["status"] = "skipped"
        with self.assertRaisesRegex(ValueError, "boundary test"):
            evaluate(base, head, policy)

    def test_selected_low_cc_hotspot_still_requires_boundary_test(self):
        base, head = self.snapshots()
        policy = self.policy()
        policy["hotspot_symbols"] = [head["functions"][0]["id"]]
        self.assertEqual(evaluate(base, head, policy)["status"], "fail")
        policy["hotspot_symbols"] = ["missing"]
        with self.assertRaisesRegex(ValueError, "missing mandatory/hotspot"):
            evaluate(base, head, policy)

    def test_valid_exception_never_waives_and_expired_exception_fails(self):
        base, head = self.snapshots()
        policy = self.policy()
        policy["exceptions"] = [{"symbol": head["functions"][0]["id"], "issue": "#92", "owner": "owner",
                                  "approver": "reviewer", "reason": "debt", "expires": "2026-09-08",
                                  "compensating_controls": "manual review"}]
        head["functions"][0].update(cc=31, body_sha256="changed")
        self.assertEqual(evaluate(base, head, policy, date(2026, 9, 7))["status"], "fail")
        head["functions"][0].update(cc=1)
        self.assertEqual(evaluate(base, head, policy, date(2026, 9, 8))["status"], "fail")
        del policy["exceptions"][0]["approver"]
        with self.assertRaises(KeyError):
            evaluate(base, head, policy)

    def test_incompatible_series_fail(self):
        base, head = self.snapshots()
        head["series"]["target"] = "other"
        with self.assertRaisesRegex(ValueError, "incompatible"):
            evaluate(base, head, self.policy())

    def test_production_boundary_failure_remains_blocking(self):
        base, head = self.snapshots()
        head["coverage"]["status"] = "fail"
        report = evaluate(base, head, self.policy())
        self.assertEqual(report["status"], "fail")
        self.assertIn("production coverage boundaries failed", markdown(report))

    def test_boundary_test_must_exist_and_pass_in_retained_results(self):
        base, head = self.snapshots()
        policy = self.policy()
        policy["boundary_tests"] = [{"symbol": head["functions"][0]["id"], "test_id": "absent",
            "observable": "failure", "status": "passed", "commit": COMMIT, "target": "fixture", "run_id": "run",
            "results": head["run"]["results"]}]
        with self.assertRaisesRegex(ValueError, "absent/skipped"):
            evaluate(base, head, policy)
        head["run"]["test_results"]["absent"] = "skipped"
        with self.assertRaisesRegex(ValueError, "absent/skipped"):
            evaluate(base, head, policy)

    def test_branch_unsupported_has_no_fake_percent(self):
        base, head = self.snapshots()
        report = evaluate(base, head, self.policy())
        self.assertNotIn("percent", report["branch"])
        self.assertIn("Branch: **unsupported**", markdown(report))

    def test_cli_missing_base_emits_error_artifacts_and_exits_nonzero(self):
        output = self.root / "risk.json"
        result = main(["--base", str(self.root / "missing"), "--head", "unused", "--base-sha", COMMIT,
                       "--head-sha", COMMIT, "--policy", "unused", "--output", str(output)])
        self.assertEqual(result, 1)
        self.assertEqual(json.loads(output.read_text())["status"], "measurement-error")
        self.assertTrue(output.with_suffix(".md").exists())


if __name__ == "__main__":
    unittest.main()
