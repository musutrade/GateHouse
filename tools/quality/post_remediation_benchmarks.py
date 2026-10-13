#!/usr/bin/env python3
"""Capture R-16 scheduler/wait scenario baselines and R-17 validation evidence.

This is a local review tool, not a CI gate. It compares the pre-hardening
commit (fixed 100 ms polling and repeated scheduler scans) with the post-change
binary (bounded backoff and indexed readiness) across representative fixtures,
then records config-validation allocation growth for the borrowed-context
change. Raw per-sample JSON is written beside the reviewable Markdown.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from quality_common import ROOT, fail, metadata, stats, write_json


SCENARIO_WORKER = r"""#!/usr/bin/env python3
from __future__ import annotations

import argparse
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--sleep", type=float, default=0.0)
parser.add_argument("--exit", type=int, default=0)
args = parser.parse_args()

if args.sleep > 0:
    time.sleep(args.sleep)
raise SystemExit(args.exit)
"""

SCENARIOS = ("fast", "slow", "narrow", "wide", "deep", "failed", "cancel")
REPORT_RELATIVE = Path(".harness-gate") / "reports" / "test_result.json"
ALLOCATION_SIZES = (100, 400, 1200)
BENCHMARK_FIXTURE = ROOT / "tools" / "quality" / "fixtures" / "benchmark" / ".harness-gate"


def q(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def init_git(root: Path) -> None:
    subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.name", "Quality Benchmark"], cwd=root, check=True
    )
    subprocess.run(
        ["git", "config", "user.email", "quality@example.invalid"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "add", "."], cwd=root, check=True)
    subprocess.run(
        ["git", "commit", "--quiet", "-m", "benchmark fixture"], cwd=root, check=True
    )


def init_project(root: Path, name: str) -> None:
    root.mkdir(parents=True, exist_ok=True)
    (root / ".harness-gate").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(BENCHMARK_FIXTURE / "audit.toml", root / ".harness-gate" / "audit.toml")
    shutil.copyfile(BENCHMARK_FIXTURE / "secrets.toml", root / ".harness-gate" / "secrets.toml")
    write(root / "post_remediation_worker.py", SCENARIO_WORKER)


def flow_header(name: str, parallel: bool, max_parallel: int) -> str:
    execution = f"""
[execution]
parallel = {str(parallel).lower()}
max_parallel = {max_parallel}
"""
    return f"""version = 2

[project]
name = {q(name)}
default_profile = "full"
hook_profile = "full"

[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[scope]
unmatched = "all"
rules = [{{ patterns = ["**"], components = ["project"] }}]
{execution}"""


def step_toml(
    index: int,
    sleep_seconds: float,
    *,
    exit_code: int = 0,
    timeout_secs: int = 60,
    depends_on: list[str] | None = None,
) -> str:
    step_id = f"benchmark.{index:04d}"
    args = [
        "post_remediation_worker.py",
        "--sleep",
        f"{sleep_seconds:.3f}",
    ]
    if exit_code:
        args.extend(["--exit", str(exit_code)])
    rendered_args = ", ".join(q(arg) for arg in args)
    dependency = ""
    if depends_on:
        dependency = "depends_on = [" + ", ".join(q(item) for item in depends_on) + "]\n"
    return f"""
[[steps]]
id = {q(step_id)}
label = {q(f"benchmark step {index}")}
component = "project"
profiles = ["full"]
program = {q(Path(sys.executable).name)}
args = [{rendered_args}]
cwd = "{{root}}"
log = {q(f"benchmark_{index:04d}.log")}
timeout_secs = {timeout_secs}
{dependency}"""


def scenario_flow(name: str) -> str:
    if name == "fast":
        body = "".join(step_toml(index, 0.02) for index in range(24))
        return flow_header(name, True, 8) + body
    if name == "slow":
        body = "".join(step_toml(index, 1.0) for index in range(4))
        return flow_header(name, True, 4) + body
    if name == "narrow":
        body = "".join(step_toml(index, 0.01) for index in range(12))
        return flow_header(name, False, 1) + body
    if name == "wide":
        root_step = step_toml(0, 0.01)
        leaves = "".join(
            step_toml(index + 1, 0.01, depends_on=["benchmark.0000"])
            for index in range(24)
        )
        return flow_header(name, True, 8) + root_step + leaves
    if name == "deep":
        body = []
        for index in range(32):
            dependency = None if index == 0 else [f"benchmark.{index - 1:04d}"]
            body.append(step_toml(index, 0.01, depends_on=dependency))
        return flow_header(name, False, 1) + "".join(body)
    if name == "failed":
        root_step = step_toml(0, 0.01, exit_code=7)
        leaves = "".join(
            step_toml(index + 1, 0.01, depends_on=["benchmark.0000"])
            for index in range(40)
        )
        return flow_header(name, True, 8) + root_step + leaves
    if name == "cancel":
        root_step = step_toml(0, 30.0, timeout_secs=1)
        leaves = "".join(
            step_toml(index + 1, 0.01, depends_on=["benchmark.0000"])
            for index in range(10)
        )
        return flow_header(name, True, 4) + root_step + leaves
    raise ValueError(f"unknown scenario {name}")


def scenario_root(name: str, sample: int) -> Path:
    root = Path(tempfile.mkdtemp(prefix=f"harness-gate-r16-{name}-{sample}-"))
    init_project(root, f"r16-{name}")
    write(root / ".harness-gate" / "flow.toml", scenario_flow(name))
    init_git(root)
    return root


def summarize_report(report: dict[str, Any]) -> dict[str, Any]:
    steps = report.get("steps", [])
    skipped = report.get("skipped_steps", [])
    step_seconds = sum(int(step.get("duration_ms", 0)) for step in steps) / 1000.0
    return {
        "passed": sum(bool(step.get("passed")) for step in steps),
        "failed": sum(not bool(step.get("passed")) for step in steps),
        "timed_out": sum(bool(step.get("timed_out")) for step in steps),
        "cancelled": sum(bool(step.get("cancelled")) for step in steps),
        "skipped": len(skipped),
        "step_seconds": step_seconds,
    }


def run_verify(binary: Path, root: Path) -> tuple[float, dict[str, Any], bool]:
    command = [
        str(binary),
        "--color",
        "never",
        "--project-root",
        str(root),
        "verify",
        "--all",
    ]
    started = time.perf_counter()
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    elapsed = time.perf_counter() - started
    report_path = root / REPORT_RELATIVE
    if not report_path.is_file():
        detail = (result.stderr or result.stdout)[-2000:]
        fail(f"verify produced no report for {root.name}: {detail}")
    report = json.loads(report_path.read_text())
    summary = summarize_report(report)
    summary["wall_seconds"] = elapsed
    summary["scheduler_overhead_seconds"] = max(0.0, elapsed - summary["step_seconds"])
    summary["returncode"] = result.returncode
    return elapsed, summary, result.returncode == 0


def scenario_sample(
    binary: Path, name: str, sample: int
) -> dict[str, Any]:
    root = scenario_root(name, sample)
    try:
        _, summary, succeeded = run_verify(binary, root)
        summary["sample"] = sample
        summary["expected_passed"] = True
        summary["actual_success"] = succeeded
        return summary
    finally:
        shutil.rmtree(root, ignore_errors=True)


def allocation_flow(name: str, checks: int) -> str:
    doctor = "\n".join(
        f"""
[[doctor.checks]]
id = "alloc.{index:04d}"
label = "allocation check {index}"
kind = "path"
path = "."
timeout_secs = 5"""
        for index in range(checks)
    )
    return flow_header(name, False, 1) + doctor + step_toml(0, 0.0)


def allocation_sample(
    binary: Path, checks: int, sample: int
) -> dict[str, Any]:
    root = Path(tempfile.mkdtemp(prefix=f"harness-gate-r17-alloc-{sample}-"))
    init_project(root, "r17-allocation")
    write(root / ".harness-gate" / "flow.toml", allocation_flow("r17-allocation", checks))
    init_git(root)
    try:
        command = [
            str(binary),
            "--color",
            "never",
            "--project-root",
            str(root),
            "config",
            "check",
        ]
        measured = False
        time_command = Path("/usr/bin/time")
        if time_command.is_file():
            command = [str(time_command), "-v", *command]
            measured = True
        started = time.perf_counter()
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        elapsed = time.perf_counter() - started
        if result.returncode != 0:
            fail(
                f"config check failed for {checks} checks: "
                f"{result.stderr or result.stdout}"
            )
        child_maxrss = 0
        if measured:
            for line in result.stderr.splitlines():
                if "Maximum resident set size (kbytes):" in line:
                    child_maxrss = int(line.rsplit(":", 1)[1].strip())
                    break
        elif hasattr(__import__("resource"), "getrusage"):
            resource = __import__("resource")
            child_maxrss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        return {
            "sample": sample,
            "checks": checks,
            "wall_seconds": elapsed,
            "child_maxrss_kb": child_maxrss,
            "measured_with_time": measured,
        }
    finally:
        shutil.rmtree(root, ignore_errors=True)


def aggregate(samples: list[dict[str, Any]]) -> dict[str, Any]:
    wall = [sample["wall_seconds"] for sample in samples]
    result = {**stats(wall), "samples": wall}
    if "scheduler_overhead_seconds" in samples[0]:
        overhead = [sample["scheduler_overhead_seconds"] for sample in samples]
        result["scheduler_overhead_seconds"] = stats(overhead)
    if "child_maxrss_kb" in samples[0]:
        memory = [sample["child_maxrss_kb"] for sample in samples]
        result["child_maxrss_kb"] = stats(memory)
    result["runs"] = samples
    return result


def percentage_delta(after: float, before: float) -> float | None:
    if not before:
        return None
    return ((after / before) - 1.0) * 100.0


def run(binary: Path, samples: int) -> dict[str, Any]:
    return run_allocation(binary, samples)


def run_allocation(binary: Path, samples: int) -> dict[str, Any]:
    allocation: dict[int, dict[str, Any]] = {}
    for checks in ALLOCATION_SIZES:
        samples_for_size = [
            allocation_sample(binary, checks, index + 1) for index in range(samples)
        ]
        allocation[str(checks)] = aggregate(samples_for_size)
    return allocation


def run_paired(
    before_binary: Path, after_binary: Path, samples: int
) -> dict[str, dict[str, Any]]:
    scenarios: dict[str, dict[str, Any]] = {}
    for name in SCENARIOS:
        before_samples: list[dict[str, Any]] = []
        after_samples: list[dict[str, Any]] = []
        for index in range(samples):
            # Alternate binary order so machine-load drift does not favor one
            # side for every sample.
            if index % 2 == 0:
                before_samples.append(scenario_sample(before_binary, name, index + 1))
                after_samples.append(scenario_sample(after_binary, name, index + 1))
            else:
                after_samples.append(scenario_sample(after_binary, name, index + 1))
                before_samples.append(scenario_sample(before_binary, name, index + 1))
        scenarios[name] = {
            "before": aggregate(before_samples),
            "after": aggregate(after_samples),
        }
    return scenarios


def compare(
    before: dict[str, Any], after: dict[str, Any]
) -> dict[str, Any]:
    comparison: dict[str, Any] = {}
    for name in SCENARIOS:
        before_median = before["scenarios"][name]["median"]
        after_median = after["scenarios"][name]["median"]
        comparison[name] = {
            "before_median_seconds": before_median,
            "after_median_seconds": after_median,
            "delta_percent": percentage_delta(after_median, before_median),
        }
    comparison["allocation"] = {}
    for checks in ALLOCATION_SIZES:
        before_median = before["allocation"][str(checks)]["median"]
        after_median = after["allocation"][str(checks)]["median"]
        before_memory = before["allocation"][str(checks)]["child_maxrss_kb"]["median"]
        after_memory = after["allocation"][str(checks)]["child_maxrss_kb"]["median"]
        comparison["allocation"][str(checks)] = {
            "before_wall_median_seconds": before_median,
            "after_wall_median_seconds": after_median,
            "wall_delta_percent": percentage_delta(after_median, before_median),
            "before_maxrss_median_kb": before_memory,
            "after_maxrss_median_kb": after_memory,
            "maxrss_delta_percent": percentage_delta(
                float(after_memory), float(before_memory)
            ),
        }
    return comparison


def write_scenario_markdown(
    path: Path,
    before: dict[str, Any],
    after: dict[str, Any],
    comparison: dict[str, Any],
    samples: int,
) -> None:
    lines = [
        "# R-16 Scheduler and Wait Scenario Baseline (Before/After)",
        "",
        "**Status:** Accepted local evidence for the R-16 wait/backoff and indexed-readiness change.",
        "",
        f"**Samples per scenario:** {samples} warm runs per binary.",
        "",
        "Scenarios: `fast` (24 quick independent steps), `slow` (four 1s steps), "
        "`narrow` (12-step serial chain), `wide` (1 root plus 24 leaves), "
        "`deep` (32-step serial chain), `failed` (failing root with 40 skipped "
        "dependents), and `cancel` (1s timeout cancels a 30s step and skips 10 dependents).",
        "",
        "| Scenario | Before median (s) | After median (s) | Delta |",
        "| --- | ---: | ---: | ---: |",
    ]
    for name in SCENARIOS:
        row = comparison[name]
        delta = (
            f"{row['delta_percent']:.2f}%"
            if row["delta_percent"] is not None
            else "n/a"
        )
        lines.append(
            f"| {name} | {row['before_median_seconds']:.4f} | "
            f"{row['after_median_seconds']:.4f} | {delta} |"
        )
    lines += [
        "",
        "Raw per-sample JSON is in `r16-scenarios.json`.",
        "",
    ]
    path.write_text("\n".join(lines) + "\n")


def write_allocation_markdown(
    path: Path,
    before: dict[str, Any],
    after: dict[str, Any],
    comparison: dict[str, Any],
    samples: int,
) -> None:
    lines = [
        "# R-17 Configuration Validation Allocation Evidence",
        "",
        "**Status:** Accepted local evidence for the borrowed validation-context change.",
        "",
        "Valid configs with 100/400/1200 doctor checks exercise the former "
        "whole-config `clone()` per check path and the current borrowed-context "
        "validation. Wall time and peak child RSS are median values over "
        f"{samples} samples.",
        "",
        "| Checks | Before wall (s) | After wall (s) | Before maxrss (KB) | After maxrss (KB) |",
        "| ---: | ---: | ---: | ---: | ---: |",
    ]
    for checks in ALLOCATION_SIZES:
        row = comparison["allocation"][str(checks)]
        lines.append(
            f"| {checks} | {row['before_wall_median_seconds']:.4f} | "
            f"{row['after_wall_median_seconds']:.4f} | "
            f"{row['before_maxrss_median_kb']:.0f} | "
            f"{row['after_maxrss_median_kb']:.0f} |"
        )
    lines += [
        "",
        "Raw per-sample JSON is in `r17-config-allocation.json`.",
        "",
    ]
    path.write_text("\n".join(lines) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--before-binary", type=Path, required=True)
    parser.add_argument("--after-binary", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=ROOT / "docs" / "benchmarks" / "r16-r17-evidence-2026-09-03",
    )
    args = parser.parse_args()
    if args.samples < 1:
        fail("at least one sample is required")
    scenario_pairs = run_paired(args.before_binary, args.after_binary, args.samples)
    before = {
        "scenarios": {
            name: scenario_pairs[name]["before"] for name in SCENARIOS
        },
        "allocation": run_allocation(args.before_binary, args.samples),
    }
    after = {
        "scenarios": {
            name: scenario_pairs[name]["after"] for name in SCENARIOS
        },
        "allocation": run_allocation(args.after_binary, args.samples),
    }
    comparison = compare(before, after)
    before_version = subprocess.check_output(
        [str(args.before_binary), "--version"], text=True
    ).strip()
    after_version = subprocess.check_output(
        [str(args.after_binary), "--version"], text=True
    ).strip()
    evidence = {
        "metadata": metadata(tool="post-remediation-benchmarks"),
        "before_binary": {
            "name": before_version.replace(" ", "-"),
            "version": before_version,
            "size_bytes": args.before_binary.stat().st_size,
        },
        "after_binary": {
            "name": after_version.replace(" ", "-"),
            "version": after_version,
            "size_bytes": args.after_binary.stat().st_size,
        },
        "samples": args.samples,
        "comparison": comparison,
        "before": before,
        "after": after,
    }
    json_path = args.output_dir / "r16-scenarios.json"
    allocation_json = args.output_dir / "r17-config-allocation.json"
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(
        json.dumps(
            {
                **evidence,
                "before": {"scenarios": before["scenarios"]},
                "after": {"scenarios": after["scenarios"]},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    allocation_json.write_text(
        json.dumps(
            {
                "metadata": evidence["metadata"],
                "comparison": comparison["allocation"],
                "before": {"allocation": before["allocation"]},
                "after": {"allocation": after["allocation"]},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    write_scenario_markdown(
        args.output_dir / "r16-scenarios.md",
        before,
        after,
        comparison,
        args.samples,
    )
    write_allocation_markdown(
        args.output_dir / "r17-config-allocation.md",
        before,
        after,
        comparison,
        args.samples,
    )
    print(json.dumps(comparison, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
