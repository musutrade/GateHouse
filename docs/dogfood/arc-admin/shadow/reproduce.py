#!/usr/bin/env python3
"""Verify retained observation evidence and regenerate the complete gate matrix."""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
CATEGORIES = {
    "Arc-Admin issue", "Harness-Gate capability gap", "Harness-Gate UX gap",
    "expected stricter generic-quality difference",
}


def read(path):
    return json.loads(path.read_text())


def matrix(root=HERE):
    inventory = read(root.parent / "inventory.json")
    observation = read(root / "observation.json")
    for name, digest in observation["artifacts"].items():
        path = root / name
        assert path.resolve().is_relative_to(root.resolve()), name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, name
    assert observation["source"]["commit"] == inventory["commit"]
    assert observation["source"]["tracked_diff_before"] == ""
    assert observation["source"]["tracked_diff_after"] == ""
    configs = observation["source"]["configuration_sha256"]
    for name, path in (
        (".arc-flow/flow.toml", "sources/.arc-flow/flow.toml.txt"),
        (".shadow-execution/flow.toml", "import/flow.toml"),
        (".harness-gate/flow.toml", "import/flow.toml"),
        (".harness-gate/quality.toml", "quality/quality.toml"),
    ):
        assert configs[name] == hashlib.sha256((root.parent / path).read_bytes()).hexdigest(), name
    assert observation["mode"] == "shadow"
    assert observation["merge_authority"] == "unchanged: Arc-Admin cargo flow and existing CI"
    discrepancies = observation["discrepancies"]
    ids = [d["id"] for d in discrepancies]
    assert len(ids) == len(set(ids)), "duplicate discrepancy"
    assert all(d["category"] in CATEGORIES and d["explanation"] for d in discrepancies)
    known = set(ids)
    report = read(root / "evidence/arc/test_result.json")
    assert report["profile"] == "full" and report["scope"]["mode"] == "all"
    assert report["scope"]["components"] == inventory["components"]
    actual = {s["label"]: s for s in report["steps"]}
    expected_labels = inventory["always_blocking_prelude"] + [s["label"] for s in inventory["flow"]["steps"]]
    assert len(actual) == len(report["steps"]) == len(expected_labels)
    assert set(actual) == set(expected_labels), "missing or unexpected Arc-Admin gate"
    assert report["passed"] == all(s["passed"] for s in actual.values())
    assert observation["arc_exit_code"] == (0 if report["passed"] else 1)
    for run in observation["harness_runs"]:
        assert run["exit_code"] == 1 and run["validation_result"] == "ERROR"
        assert run["dispatched_steps"] == [] and run["emitted_reports"] == []
        assert "HGCFG-SHARED-SERVICE" in (root / run["log"]).read_text()
    assert len(observation["harness_runs"]) == 2
    assert {r["variant"] for r in observation["harness_runs"]} == {"execution", "quality"}
    used = set()
    rows = []
    required = set(inventory["flow"]["policy"]["required_steps"])
    for step in inventory["flow"]["steps"]:
        result = actual[step["label"]]
        log = "evidence/arc/logs/" + step["log"]
        assert Path(result["log"]).name == step["log"] and log in observation["artifacts"]
        refs = observation["gate_discrepancies"][step["id"]]
        assert refs and len(refs) == len(set(refs)) and set(refs) <= known
        used.update(refs)
        if not result["passed"]:
            assert any(d["category"] == "Arc-Admin issue" for d in discrepancies if d["id"] in refs)
        rows.append({"step": step["id"], "policy_required": step["id"] in required,
                     "blocks_when_selected": True,
                     "arc_admin": "PASS" if result["passed"] else "FAIL",
                     "arc_detail": result["detail"],
                     "arc_log": log,
                     "harness_gate": "NOT_RUN: configuration rejected",
                     "discrepancies": refs})
    assert set(observation["gate_discrepancies"]) == {r["step"] for r in rows}
    dimensions = [d["dimension"] for d in observation["dimensions"]]
    assert len(dimensions) == 6 and set(dimensions) == {
        "component selection", "PostgreSQL service", "environment isolation",
        "diagnostics and validation readiness", "reports and artifacts", "generic quality",
    }, "missing comparison dimension"
    for dimension in observation["dimensions"]:
        refs = dimension["discrepancies"]
        assert refs and len(refs) == len(set(refs)) and set(refs) <= known
        used.update(refs)
    assert used == known, "unreferenced discrepancy"
    return {"source_commit": inventory["commit"], "mode": "shadow",
            "parity": "blocked; runtime equivalence not established",
            "validation_is_workflow_state": False,
            "components": {"arc_admin": report["scope"]["components"],
                           "harness_gate": "NOT_COMPUTED: configuration rejected"},
            "prelude": [{"gate": label, "arc_admin": "PASS" if actual[label]["passed"] else "FAIL",
                         "harness_gate": "NOT_RUN: configuration rejected", "discrepancies": ["HG-CAP-001"]}
                        for label in inventory["always_blocking_prelude"]],
            "gates": rows, "dimensions": observation["dimensions"],
            "unexplained_discrepancies": 0}


def render(value):
    lines = ["# Arc-Admin shadow parity matrix", "", "Generated from retained evidence by `python3 docs/dogfood/arc-admin/shadow/reproduce.py --write`.", "",
             "Full profile, all source. Every row remains blocking when selected. Harness-Gate rejected configuration before dispatch; NOT_RUN is neither PASS nor a command failure. Runtime parity is blocked. Validation results do not represent workflow/lifecycle state.", "",
             "| Gate | Explicitly required | Arc-Admin | Harness-Gate | Discrepancies |",
             "| --- | --- | --- | --- | --- |"]
    for row in value["prelude"]:
        lines.append(f"| {row['gate']} | always | {row['arc_admin']} | {row['harness_gate']} | {', '.join(row['discrepancies'])} |")
    for row in value["gates"]:
        lines.append(f"| {row['step']} | {'yes' if row['policy_required'] else 'no (still blocks)'} | {row['arc_admin']} | {row['harness_gate']} | {', '.join(row['discrepancies'])} |")
    lines += ["", "| Dimension | Arc-Admin | Harness-Gate | Discrepancies |",
              "| --- | --- | --- | --- |"]
    for row in value["dimensions"]:
        lines.append(f"| {row['dimension']} | {row['arc_admin']} | {row['harness_gate']} | {', '.join(row['discrepancies'])} |")
    lines += ["", "See [observation and classifications](observation.json) for source provenance, commands, service/environment limits, diagnostics and artifact hashes.", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    value = matrix()
    for name, content in (("matrix.json", json.dumps(value, indent=2) + "\n"), ("matrix.md", render(value))):
        path = HERE / name
        if args.write:
            path.write_text(content)
        else:
            assert path.read_text() == content, f"stale {name}"
    print(f"Validated {len(value['gates'])} blocking gates, two prelude gates; zero unexplained discrepancies; runtime parity blocked.")
