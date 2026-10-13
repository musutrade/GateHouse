#!/usr/bin/env python3
"""Capture deterministic verification, test, and release-small baselines."""

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

from quality_common import CRATE, QUALITY_ROOT, fail, metadata, sha256, stats, write_json


HARNESS_VERSION = 1
FIXTURE_VERSION = 1
REGRESSION_THRESHOLD_PERCENT = 15.0
FIXTURE = Path(__file__).with_name("fixtures") / "benchmark"
STATE_RELATIVE = Path(".harness-gate") / "parallel-state" / "state.json"
NEXTEST_EXCLUDED_BINARY = "benchmark_fixture_test"


def timed(
    command: list[str], cwd: Path, env: dict[str, str] | None = None
) -> tuple[float, subprocess.CompletedProcess[str]]:
    started = time.perf_counter()
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    return time.perf_counter() - started, result


def init_fixture(destination: Path) -> None:
    shutil.copytree(FIXTURE, destination, dirs_exist_ok=True)
    subprocess.run(["git", "init", "--quiet"], cwd=destination, check=True)
    subprocess.run(["git", "config", "user.name", "Quality Benchmark"], cwd=destination, check=True)
    subprocess.run(["git", "config", "user.email", "quality@example.invalid"], cwd=destination, check=True)
    subprocess.run(["git", "add", "."], cwd=destination, check=True)
    subprocess.run(["git", "commit", "--quiet", "-m", "fixture"], cwd=destination, check=True)


def configure_interpreter(root: Path) -> None:
    """Use the interpreter that launched this benchmark on every platform."""
    flow = root / ".harness-gate" / "flow.toml"
    interpreter = Path(sys.executable).name
    flow.write_text(flow.read_text().replace('program = "python3"', f'program = "{interpreter}"'))


def configure_execution(root: Path, parallel: bool) -> int:
    """Apply the benchmark-only execution policy to an isolated fixture."""
    flow = root / ".harness-gate" / "flow.toml"
    if not parallel:
        # Serial mode still passes the same static preflight. Make the
        # intentionally shared service ordering explicit in the fixture.
        source = flow.read_text()
        source = source.replace(
            'id = "benchmark.status"\n',
            'id = "benchmark.status"\ndepends_on = ["benchmark.diff"]\n',
        )
        flow.write_text(source)
        return 1
    # The product's preflight intentionally rejects unordered shared services.
    # Keep service reuse in the serial sample and remove that fixture-only
    # dependency from the independent parallel sample.
    source = flow.read_text()
    source = source.replace(
        '[services.benchmark]\n'
        'kind = "environment"\n'
        'source_env = "HARNESS_GATE_BENCHMARK_SERVICE_URL"\n'
        'inject_env = "BENCHMARK_SERVICE_URL"\n\n',
        "",
    )
    source = source.replace('\nservices = ["benchmark"]', "")
    flow.write_text(source + "\n[execution]\nparallel = true\nmax_parallel = 2\n")
    return 2


def configure_scope_fixture(root: Path, files: int = 600, rules: int = 48) -> None:
    """Create a large, unmatched-safe scope workload for matcher evidence."""
    flow = root / ".harness-gate" / "flow.toml"
    source = flow.read_text()
    source = source.replace(
        '[services.benchmark]\n'
        'kind = "environment"\n'
        'source_env = "HARNESS_GATE_BENCHMARK_SERVICE_URL"\n'
        'inject_env = "BENCHMARK_SERVICE_URL"\n\n',
        "",
    )
    source = source.replace('\nservices = ["benchmark"]', "")
    start = source.index("rules = [{")
    end = source.index("\n\n[[steps]]", start)
    entries = []
    for index in range(rules):
        entries.append(
            '  { patterns = ["src/module%02d/**", "src/module%02d/**/*.rs", "src/module%02d/**/*.toml"], components = ["project"] },'
            % (index, index, index)
        )
    replacement = "rules = [\n" + "\n".join(entries) + "\n]"
    flow.write_text(source[:start] + replacement + source[end:])
    for index in range(files):
        module = index % rules
        path = root / "src" / (f"module{module:02d}") / f"generated-{index:04d}.rs"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("// generated benchmark input\n")


def scope_matcher_sample(
    binary: Path, raw_root: Path, number: int, repeats: int = 100
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="harness-gate-quality-benchmark-scope-") as directory:
        root = Path(directory)
        init_fixture(root)
        configure_scope_fixture(root)
        seconds, result = timed(
            [
                str(binary),
                "--color",
                "never",
                "--project-root",
                str(root),
                "scope",
                "--benchmark-repeat",
                str(repeats),
            ],
            CRATE.parent.parent,
        )
        if result.returncode != 0:
            fail(f"scope matcher benchmark sample {number} failed: {result.stderr or result.stdout}")
        try:
            evidence = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            fail(f"scope matcher benchmark sample {number} returned invalid JSON: {error}")
        evidence["wall_seconds"] = seconds
        evidence["sample"] = number
        archive = raw_root / "scope" / f"sample-{number}.json"
        write_json(archive, evidence)
        evidence["raw"] = str(Path("benchmark-runs") / "scope" / f"sample-{number}.json")
        return evidence


def verification_sample(
    binary: Path, root: Path, raw_root: Path, number: int, mode: str, max_parallel: int
) -> dict[str, Any]:
    reports = root / ".harness-gate" / "reports"
    shutil.rmtree(reports, ignore_errors=True)
    state_path = root / STATE_RELATIVE
    state_path.parent.mkdir(parents=True, exist_ok=True)
    state_path.write_text(json.dumps({"current": 0, "observed_peak": 0, "workers": 0}) + "\n")
    environment = os.environ.copy()
    environment["HARNESS_GATE_BENCHMARK_SERVICE_URL"] = "http://127.0.0.1:43123"
    seconds, result = timed(
        [str(binary), "--color", "never", "--project-root", str(root), "verify", "--all"],
        CRATE.parent.parent,
        environment,
    )
    # Preserve diagnostics before any validation can fail and the temporary
    # fixture is removed. Archiving stays outside the measured command time.
    archive = raw_root / mode / f"sample-{number}"
    archive.parent.mkdir(parents=True, exist_ok=True)
    if reports.is_dir():
        shutil.copytree(reports, archive)
    else:
        archive.mkdir()
    if state_path.is_file():
        shutil.copy2(state_path, archive / "parallel-state.json")
    output_limit = 64 * 1024
    write_json(archive / "command-result.json", {
        "exit_code": result.returncode,
        "seconds": seconds,
        "stdout_tail": result.stdout[-output_limit:],
        "stderr_tail": result.stderr[-output_limit:],
        "stdout_truncated": len(result.stdout) > output_limit,
        "stderr_truncated": len(result.stderr) > output_limit,
    })
    if result.returncode != 0:
        fail(f"benchmark verification sample {number} failed: {result.stderr or result.stdout}\n"
             f"retained evidence: {archive}")
    report_path = reports / "test_result.json"
    if not report_path.is_file():
        fail(f"benchmark verification sample {number} did not write {report_path}")
    report = json.loads(report_path.read_text())
    if not state_path.is_file():
        fail(f"benchmark verification sample {number} did not write {state_path}")
    observed = json.loads(state_path.read_text())
    if observed.get("current") != 0:
        fail(f"benchmark verification sample {number} leaked an active worker")
    observed_peak = observed.get("observed_peak")
    worker_count = observed.get("workers")
    if not isinstance(observed_peak, int) or not isinstance(worker_count, int):
        fail(f"benchmark verification sample {number} has invalid concurrency evidence")
    if worker_count != 2:
        fail(
            f"benchmark verification sample {number} expected two workers, got {worker_count}"
        )
    if observed_peak < 1 or observed_peak > max_parallel:
        fail(
            f"benchmark verification sample {number} observed peak {observed_peak} "
            f"outside configured limit {max_parallel}"
        )
    steps = []
    for step in report["steps"]:
        source_log = Path(step["log"])
        try:
            relative_log = source_log.relative_to(reports)
        except ValueError:
            relative_log = Path("logs") / source_log.name
        steps.append(
            {
                "label": step["label"],
                "duration_ms": step["duration_ms"],
                "passed": step["passed"],
                "log": str(Path("benchmark-runs") / mode / f"sample-{number}" / relative_log),
            }
        )
    if len(steps) < 4 or not report["passed"]:
        fail(f"benchmark verification sample {number} did not complete two gates and two configured steps")
    step_seconds = sum(step["duration_ms"] for step in steps) / 1000.0
    return {
        "sample": number,
        "mode": mode,
        "seconds": seconds,
        "scheduler_overhead_seconds": max(0.0, seconds - step_seconds),
        "configured_limit": max_parallel,
        "service_startup_reuse": {
            "service": "benchmark" if mode == "serial" else None,
            "node_uses": 2 if mode == "serial" else 0,
            "starts": 1 if mode == "serial" else 0,
            "reuses": 1 if mode == "serial" else 0,
        },
        "observed_peak": observed_peak,
        "steps": steps,
        "report": str(Path("benchmark-runs") / mode / f"sample-{number}" / "test_result.json"),
    }


def nextest_command() -> list[str]:
    # benchmark_fixture_test launches the product's parallel benchmark probe.
    # Excluding it keeps the timed suite from running the benchmark inside the
    # benchmark under the same constrained runner. The probe stays required in
    # the normal Test and Quality Script Tests jobs.
    return [
        "cargo",
        "nextest",
        "run",
        "--manifest-path",
        str(CRATE / "Cargo.toml"),
        "--locked",
        "--filter-expr",
        "not binary(" + NEXTEST_EXCLUDED_BINARY + ")",
    ]


def bounded_head_tail(text: str, limit: int = 4000) -> str:
    """Keep the failure head (probe message) as well as the diagnostic tail."""
    if len(text) <= limit:
        return text
    return text[:limit] + "\n[... truncated ...]\n" + text[-limit:]


def retain_nextest_sample(
    output: Path, label: str, result: subprocess.CompletedProcess[str]
) -> Path:
    archive = output.parent / "nextest-samples"
    archive.mkdir(parents=True, exist_ok=True)
    (archive / (label + ".stdout.txt")).write_text(result.stdout or "")
    (archive / (label + ".stderr.txt")).write_text(result.stderr or "")
    return archive


def nextest_timings(output: Path, samples: int) -> tuple[float, list[float]]:
    command = nextest_command()
    cold_seconds, cold_result = timed(command, CRATE.parent.parent)
    if cold_result.returncode != 0:
        archive = retain_nextest_sample(output, "cold", cold_result)
        fail(
            "cold nextest sample failed\n"
            f"retained: {archive}\n"
            f"stdout:\n{bounded_head_tail(cold_result.stdout)}\n"
            f"stderr:\n{bounded_head_tail(cold_result.stderr)}"
        )
    warm_seconds = []
    for number in range(1, samples + 1):
        seconds, result = timed(command, CRATE.parent.parent)
        if result.returncode != 0:
            archive = retain_nextest_sample(output, f"warm-{number}", result)
            fail(
                f"warm nextest sample {number} failed\n"
                f"retained: {archive}\n"
                f"stdout:\n{bounded_head_tail(result.stdout)}\n"
                f"stderr:\n{bounded_head_tail(result.stderr)}"
            )
        warm_seconds.append(seconds)
    return cold_seconds, warm_seconds


def run(output: Path, samples: int, raw_dir: Path | None = None) -> int:
    if samples < 1:
        fail("at least one warm sample is required")
    subprocess.run(["cargo", "build", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked", "--profile", "release-small"], check=True, cwd=CRATE.parent.parent)
    target_metadata = subprocess.check_output(
        [
            "cargo",
            "metadata",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "--locked",
            "--format-version",
            "1",
            "--no-deps",
        ],
        cwd=CRATE.parent.parent,
        text=True,
    )
    # Cargo target directories may be redirected by the host or CI cache.
    # Resolve the actual directory instead of assuming crate/target.
    target_directory = Path(json.loads(target_metadata)["target_directory"])
    binary = target_directory / "release-small" / (
        "harness-gate.exe" if os.name == "nt" else "harness-gate"
    )
    if not binary.is_file():
        fail(f"release-small binary missing: {binary}")

    cold_seconds, warm_seconds = nextest_timings(output, samples)

    raw_root = raw_dir or output.parent / "benchmark-runs"
    shutil.rmtree(raw_root, ignore_errors=True)
    raw_root.mkdir(parents=True, exist_ok=True)
    verification: dict[str, list[dict[str, Any]]] = {}
    for mode, parallel in (("serial", False), ("parallel", True)):
        with tempfile.TemporaryDirectory(prefix=f"harness-gate-quality-benchmark-{mode}-") as directory:
            root = Path(directory)
            init_fixture(root)
            configure_interpreter(root)
            max_parallel = configure_execution(root, parallel)
            verification[mode] = [
                verification_sample(binary, root, raw_root, index + 1, mode, max_parallel)
                for index in range(samples)
            ]
    verification_summary = {}
    for mode, samples_for_mode in verification.items():
        seconds = [sample["seconds"] for sample in samples_for_mode]
        verification_summary[mode] = {
            **stats(seconds),
            "scheduler_overhead_seconds": stats(
                sample["scheduler_overhead_seconds"] for sample in samples_for_mode
            ),
            "samples": seconds,
            "configured_limit": samples_for_mode[0]["configured_limit"],
            "observed_peak": max(sample["observed_peak"] for sample in samples_for_mode),
            "service_startup_reuse": {
                "service": "benchmark" if mode == "serial" else None,
                "node_uses": sum(
                    sample["service_startup_reuse"]["node_uses"] for sample in samples_for_mode
                ),
                "starts": sum(
                    sample["service_startup_reuse"]["starts"] for sample in samples_for_mode
                ),
                "reuses": sum(
                    sample["service_startup_reuse"]["reuses"] for sample in samples_for_mode
                ),
            },
            "runs": samples_for_mode,
        }
    serial_median = verification_summary["serial"]["median"]
    parallel_median = verification_summary["parallel"]["median"]
    scope_samples = [scope_matcher_sample(binary, raw_root, index + 1) for index in range(samples)]
    scope_summary = {
        "cached_per_iteration_us": stats(
            sample["cached_per_iteration_us"] for sample in scope_samples
        ),
        "uncached_per_iteration_us": stats(
            sample["uncached_per_iteration_us"] for sample in scope_samples
        ),
        "speedup": stats(sample["speedup"] or 0.0 for sample in scope_samples),
        "paths": scope_samples[0]["paths"],
        "iterations": scope_samples[0]["iterations"],
        "equivalent": all(sample["equivalent"] for sample in scope_samples),
        "runs": scope_samples,
    }
    run_metadata = metadata(
        tool="quality-benchmarks",
        harness_version=HARNESS_VERSION,
        fixture_version=FIXTURE_VERSION,
        cache_state="cold-and-warm",
        raw_evidence=str(raw_root),
    )
    # ADR-0025 comparisons are valid only inside a stable measurement series.
    # Keep the key explicit so reviewers and automation cannot compare results
    # across targets, toolchains, fixture revisions, or cache policies.
    series_key = ":".join(
        str(run_metadata[field])
        for field in ("target", "rustc", "harness_version", "fixture_version", "cache_state")
    )
    comparison_delta = ((parallel_median / serial_median) - 1.0) * 100.0 if serial_median else None
    result = {
        **run_metadata,
        "series_key": series_key,
        "regression_policy": {
            "threshold_percent": REGRESSION_THRESHOLD_PERCENT,
            "comparison": "parallel median wall time versus serial median",
            "incompatible_series": "record as a new baseline series",
        },
        "verification": {
            **verification_summary,
            "comparison": {
                "serial_median_seconds": serial_median,
                "parallel_median_seconds": parallel_median,
                "speedup": (serial_median / parallel_median) if parallel_median else None,
                "delta_percent": comparison_delta,
                "regression": bool(
                    comparison_delta is not None
                    and comparison_delta > REGRESSION_THRESHOLD_PERCENT
                ),
            },
        },
        "tests_seconds": {
            "cold": cold_seconds,
            "warm_samples": warm_seconds,
            "excluded_binaries": [NEXTEST_EXCLUDED_BINARY],
            **stats(warm_seconds),
        },
        "scope_matcher": scope_summary,
        "binary": {"path": str(binary), "bytes": binary.stat().st_size, "sha256": sha256(binary)},
    }
    write_json(output, result)
    output.with_suffix(".md").write_text(
        "# Quality Baseline\n\n"
        f"Serial verification median: **{result['verification']['serial']['median']:.3f}s**\n\n"
        f"Parallel verification median: **{result['verification']['parallel']['median']:.3f}s**\n\n"
        f"Comparison speedup: **{result['verification']['comparison']['speedup']:.3f}x**\n\n"
        f"Comparison delta: **{result['verification']['comparison']['delta_percent']:.2f}%**\n\n"
        f"Scope matcher cached median: **{result['scope_matcher']['cached_per_iteration_us']['median']:.1f}us**; "
        f"uncached: **{result['scope_matcher']['uncached_per_iteration_us']['median']:.1f}us** "
        f"({result['scope_matcher']['speedup']['median']:.2f}x speedup)\n\n"
        f"Series key: `{result['series_key']}`\n\n"
        f"Test warm median: **{result['tests_seconds']['median']:.3f}s** (cold: {cold_seconds:.3f}s)\n\n"
        f"Release-small binary: **{result['binary']['bytes']} bytes**\n"
        f"SHA-256: `{result['binary']['sha256']}`\n"
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "benchmarks.json")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--raw-dir", type=Path, help="dedicated directory for retained per-sample reports")
    args = parser.parse_args()
    raise SystemExit(run(args.output, args.samples, args.raw_dir))
