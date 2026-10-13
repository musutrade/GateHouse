#!/usr/bin/env python3
"""Generate and gate source-boundary coverage evidence."""

from __future__ import annotations

import argparse
import subprocess
from collections import defaultdict
from decimal import Decimal
from pathlib import Path

from quality_common import CRATE, QUALITY_ROOT, fail, metadata, read_json, write_json


CORE_MODULES = {
    "config": "src/config/",
    "verify": "src/verify/",
    "process": "src/process/",
    "audit": "src/audit/",
    "scope": "src/scope/",
    "secrets": "src/secrets/",
}

# External adapters remain visible in the report but are not silently counted
# in the deterministic core threshold. Docker coverage requires a daemon and
# is validated by the service contract tests when that capability is present.
ADAPTER_MODULES = {
    "service": "src/service/",
}

# Benchmark-only harness code is exercised by the performance workflow, not by
# the functional coverage suite. Keep it out of the scope engine threshold so
# adding measurement scenarios does not dilute production-path coverage.
EXCLUDED_FILES = {"scope/benchmark.rs"}


def run(output: Path, threshold: float, production: bool = False) -> int:
    raw = output.with_name("coverage.raw.json")
    raw.parent.mkdir(parents=True, exist_ok=True)
    # Keep the existing CLI measurement boundary; the candidate workspace crate
    # has its own differential tests and has not transferred semantic authority.
    subprocess.run(
        [
            "cargo",
            "llvm-cov",
            "--package", "harness-gate",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "--locked",
            "--json",
            "--output-path",
            str(raw),
        ],
        check=True,
        cwd=CRATE.parent.parent,
    )
    for format_flag, artifact in (("--lcov", output.with_name("coverage.lcov")), ("--cobertura", output.with_name("coverage.cobertura.xml"))):
        subprocess.run(
            [
                "cargo",
                "llvm-cov",
                "--package", "harness-gate",
                "--manifest-path",
                str(CRATE / "Cargo.toml"),
                "--locked",
                "--no-run",
                format_flag,
                "--output-path",
                str(artifact),
            ],
            check=True,
            cwd=CRATE.parent.parent,
        )
    if production:
        from production_coverage import run as production_run
        return production_run(raw, output.with_name("coverage.lcov"), output, Decimal(str(threshold)))
    report = read_json(raw)
    files = report["data"][0]["files"]
    totals: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for item in files:
        filename = item["filename"].replace("\\", "/")
        marker = "/src/"
        relative = filename.split(marker, 1)[-1] if marker in filename else filename
        if relative in EXCLUDED_FILES:
            continue
        module = relative.split("/", 1)[0]
        if module not in CORE_MODULES and module not in ADAPTER_MODULES:
            continue
        covered = int(item["summary"]["lines"]["covered"])
        count = int(item["summary"]["lines"]["count"])
        totals[module][0] += covered
        totals[module][1] += count

    modules = {}
    failures = []
    aggregate_covered = aggregate_count = 0
    for module, prefix in {**CORE_MODULES, **ADAPTER_MODULES}.items():
        covered, count = totals[module]
        aggregate_covered += covered
        aggregate_count += count
        percent = (covered / count * 100) if count else None
        is_core = module in CORE_MODULES
        modules[module] = {
            "source": prefix,
            "covered": covered,
            "executable": count,
            "percent": percent,
            "status": (
                "n/a"
                if count == 0
                else ("pass" if percent >= threshold else "fail")
                if is_core
                else "informational"
            ),
        }
        if module in CORE_MODULES and (count == 0 or percent < threshold):
            failures.append(module)
    core_covered = sum(totals[module][0] for module in CORE_MODULES)
    core_count = sum(totals[module][1] for module in CORE_MODULES)
    aggregate = core_covered / core_count * 100 if core_count else 0
    result = {
        **metadata(
            tool="cargo-llvm-cov",
            command="cargo llvm-cov --manifest-path tools/harness-gate/Cargo.toml --locked --json",
            threshold=threshold,
            source_boundaries={**CORE_MODULES, **ADAPTER_MODULES},
            blocking_boundaries=list(CORE_MODULES),
            informational_boundaries=list(ADAPTER_MODULES),
            exclusion_list_version="quality-1",
            raw_artifacts=[str(raw), str(output.with_name("coverage.lcov")), str(output.with_name("coverage.cobertura.xml"))],
        ),
        "modules": modules,
        "aggregate": {
            "covered": core_covered,
            "executable": core_count,
            "percent": aggregate,
            "status": "pass" if aggregate >= threshold else "fail",
        },
    }
    write_json(output, result)
    lines = [
        "# Coverage Baseline",
        "",
        f"Commit: `{result['commit']}`",
        f"Threshold: {threshold:.1f}%",
        "",
        "| Module | Covered | Executable | Coverage | Status |",
        "| --- | ---: | ---: | ---: | --- |",
    ]
    for module, item in modules.items():
        percent = "N/A" if item["percent"] is None else f"{item['percent']:.2f}%"
        lines.append(f"| `{module}` | {item['covered']} | {item['executable']} | {percent} | {item['status']} |")
    lines.extend(["", f"Blocking core aggregate: **{aggregate:.2f}%**", "", "Service adapter coverage is informational and requires service-contract evidence.", ""])
    output.with_suffix(".md").write_text("\n".join(lines))
    if failures or aggregate < threshold:
        fail(f"coverage below {threshold:.1f}% for: {', '.join(failures) or 'aggregate'}")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "coverage.json")
    parser.add_argument("--threshold", type=float, default=80.0)
    parser.add_argument("--production", action="store_true", help="Enforce the expanded production-source inventory (candidate until CI adoption)")
    parser.add_argument("--raw", type=Path, help="Evaluate an existing LLVM JSON export; requires --production and --lcov")
    parser.add_argument("--lcov", type=Path, help="Matching LCOV export for --raw")
    args = parser.parse_args()
    if args.raw is not None or args.lcov is not None:
        if not args.production or args.raw is None or args.lcov is None:
            parser.error("--raw and --lcov require each other and --production")
        from production_coverage import run as production_run
        raise SystemExit(production_run(args.raw, args.lcov, args.output, Decimal(str(args.threshold))))
    raise SystemExit(run(args.output, args.threshold, args.production))
