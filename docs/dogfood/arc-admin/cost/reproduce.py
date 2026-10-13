#!/usr/bin/env python3
"""Derive the cost/ownership ledger from retained evidence, without running Arc-Admin."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tomllib

ARC = Path(__file__).resolve().parent.parent


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def derive(arc=ARC):
    module(arc / "reproduce.py", "arc_before").check()
    module(arc / "shadow/reproduce.py", "arc_shadow").matrix(arc / "shadow")
    inventory = json.loads((arc / "inventory.json").read_text())
    costs = json.loads((arc / "cost-summary.json").read_text())
    native = json.loads((arc / "shadow/evidence/arc/test_result.json").read_text())
    observation = json.loads((arc / "shadow/observation.json").read_text())
    quality = tomllib.loads((arc / "quality/quality.toml").read_text())
    index = json.loads((arc / "cost/ci-run-index.json").read_text())
    assert index["total_count"] == len(index["runs"]), "incomplete run index"
    assert all(r["head_sha"] == inventory["commit"] for r in index["runs"])
    assert [r["id"] for r in index["runs"] if r["path"] == ".github/workflows/ci.yml"] == [32701649122]
    before = next(c for c in costs if c["run_id"] == 32701649122)
    assert all(not r["dispatched_steps"] and not r["emitted_reports"] for r in observation["harness_runs"])
    durations = {s["label"]: s["duration_ms"] for s in native["steps"]}
    steps = inventory["flow"]["steps"]
    commands = [{
        "id": s["id"], "command": [s["program"], *s["args"]], "cwd": s["cwd"],
        "profiles": s["profiles"], "blocks_when_selected": True,
        "explicit_policy_required": s["id"] in inventory["flow"]["policy"]["required_steps"],
        "before_owner": "cargo-flow/" + s["component"],
        "observed_local_arc_executions": 1, "observed_local_harness_executions": 0,
        "local_arc_duration_ms": durations[s["label"]],
        "naive_full_shadow_extra_executions": 1,
        "target_owner": "harness-gate/execution", "target_executions": 1,
        "project_owned": True,
    } for s in steps]
    measurements = [{
        "collector": name, **produce,
        "before_authoritative_producers": 0, "observed_shadow_authoritative_producers": 0,
        "target_owner": "harness-gate/collector/" + name,
        "target_producers": 0 if (name == "frontend" and produce["capability"] == "risk.crap") else 1,
        "status": "unsupported" if (name == "frontend" and produce["capability"] == "risk.crap") else "requires host provisioning",
    } for name in quality["profiles"]["full"]["collectors"]
      for produce in quality["collectors"][name]["produces"]]
    inputs = ["source-manifest.json", "ci-capture.json", "inventory.json", "cost-summary.json",
              "shadow/observation.json", "shadow/evidence/arc/test_result.json",
              "quality/quality.toml", "import/flow.toml", "cost/ci-run-index.json"]
    return {
        "schema_version": 1, "source_commit": inventory["commit"],
        "inputs_sha256": {p: hashlib.sha256((arc / p).read_bytes()).hexdigest() for p in inputs},
        "before_self_hosted_ci": before,
        "shadow_self_hosted_ci": {"status": "NOT_MEASURED", "added_wall_seconds": None,
            "added_runner_seconds": None, "reason": "No paired self-hosted shadow run; GH-204 stopped before dispatch and has no command elapsed timestamps."},
        "local_observation": {"arc_sum_step_seconds": sum(durations.values()) / 1000,
            "arc_command_step_seconds": sum(durations[s["label"]] for s in steps) / 1000,
            "arc_prelude_seconds": sum(durations[s] for s in inventory["always_blocking_prelude"]) / 1000,
            "harness_elapsed_seconds": None, "ci_runner_seconds": None,
            "qualification": "Sum of native step timers; neither workflow wall time nor runner occupancy."},
        "target": {"status": "DESIGN_ONLY", "wall_seconds": None, "runner_seconds": None,
            "monetary_cost": None, "authority_transfer_permitted": False},
        "command_inventory": commands, "measurement_inventory": measurements,
    }


def check_report(actual, expected):
    assert actual == expected, "cost/ownership report differs from retained evidence; regenerate and review"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    result = derive()
    path = ARC / "cost/report.json"
    if args.write:
        path.write_text(json.dumps(result, indent=2) + "\n")
    else:
        check_report(json.loads(path.read_text()), result)
    print("Historical pretrial ledger validated: 25 command owners; original shadow cost NOT_MEASURED. See ci_reproduce.py for the separate bounded self-hosted trial series; transfer remains blocked.")


if __name__ == "__main__":
    main()
