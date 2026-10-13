#!/usr/bin/env python3
"""Capture the reproducibility contract for quality measurements.

This command deliberately records provenance; it does not make a quality
decision.  The generated evidence is a candidate until a reviewer accepts it
for a baseline series.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any

from quality_common import CRATE, QUALITY_ROOT, ROOT, metadata, sha256, write_json


REVIEW_BASELINE = "c9101c3191be7a5fd639c64c3781ccb154e0ce34"
REVIEW_BASELINE_VERSION = "v0.3.7"
PROFILE = "dev"
INSTRUMENTATION = "-C instrument-coverage"


def command(command: list[str], *, cwd: Path = ROOT) -> str:
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True, check=True)
    return result.stdout.strip()


def cargo_target_directory() -> Path:
    value = json.loads(
        command(
            [
                "cargo",
                "metadata",
                "--manifest-path",
                str(CRATE / "Cargo.toml"),
                "--locked",
                "--format-version",
                "1",
                "--no-deps",
            ]
        )
    )["target_directory"]
    return Path(value)


def nextest_provenance() -> dict[str, Any]:
    raw = command(
        [
            "cargo",
            "nextest",
            "list",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "--locked",
            "--message-format",
            "json",
        ]
    )
    listing = json.loads(raw)
    target_directory = Path(listing["rust-build-meta"]["target-directory"])
    binaries = []
    for suite in listing.get("rust-suites", {}).values():
        path = Path(suite["binary-path"])
        item: dict[str, Any] = {
            "binary_id": suite["binary-id"],
            "kind": suite["kind"],
            "path": str(path),
            "exists": path.is_file(),
            "test_count": len(suite.get("testcases", {})),
        }
        if path.is_file():
            item["sha256"] = sha256(path)
            item["bytes"] = path.stat().st_size
        binaries.append(item)
    return {
        "command": "cargo nextest list --manifest-path tools/harness-gate/Cargo.toml --locked --message-format json",
        "profile": PROFILE,
        "target_directory": str(target_directory),
        "test_count": listing.get("test-count"),
        "binaries": binaries,
    }


def llvm_cov_provenance() -> dict[str, Any]:
    output = command(
        [
            "cargo",
            "llvm-cov",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "show-env",
            "--sh",
        ]
    )
    variables: dict[str, str] = {}
    for line in output.splitlines():
        match = re.match(r"export ([A-Z0-9_]+)='?(.*?)'?$", line)
        if match:
            variables[match.group(1)] = match.group(2)
    profile_file = variables.get("LLVM_PROFILE_FILE")
    rustflags = variables.get("__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS", "")
    return {
        "command": "cargo llvm-cov --manifest-path tools/harness-gate/Cargo.toml --locked --json",
        "profile": PROFILE,
        "target_directory": variables.get("CARGO_LLVM_COV_TARGET_DIR"),
        "instrumented_by": command(["cargo", "llvm-cov", "--version"]),
        "rustflags": rustflags.replace("\x1f", " "),
        "instrumentation": INSTRUMENTATION in rustflags.replace("\x1f", " "),
        "profile_file_pattern": profile_file,
        "child_process_policy": {
            "environment": "LLVM_PROFILE_FILE is inherited by child processes",
            "binary": "a child executable must itself be built with -C instrument-coverage",
            "evidence": "retain profraw files and match their target/profile to this record",
        },
        "branch_status": {
            "status": "unsupported",
            "reason": "the baseline command does not pass cargo-llvm-cov --branch",
        },
    }


def run(output: Path) -> int:
    target_directory = cargo_target_directory()
    base_metadata = metadata()
    openspec_version = command(["openspec", "--version"])
    result = {
        **base_metadata,
        "tool": "measurement-contract",
        "contract_version": 1,
        "review_baseline": REVIEW_BASELINE,
        "review_baseline_version": REVIEW_BASELINE_VERSION,
        "target_triple": base_metadata["target"],
        "openspec": openspec_version,
        "status": "candidate",
        "baseline": {
            "review_commit": REVIEW_BASELINE,
            "review_version": REVIEW_BASELINE_VERSION,
            "review_commit_available_locally": (
                subprocess.run(
                    ["git", "cat-file", "-e", f"{REVIEW_BASELINE}^{{commit}}"],
                    cwd=ROOT,
                    capture_output=True,
                ).returncode
                == 0
            ),
            "measured_commit": base_metadata["commit"],
        },
        "measurements": {
            "tests": nextest_provenance(),
            "coverage": llvm_cov_provenance(),
            "target_directory": str(target_directory),
            "profiles": {
                "tests_and_coverage": PROFILE,
                "distribution_benchmark": "release-small",
            },
        },
        "coverage_interpretation": {
            "file_level": "one source file's executable lines divided by that file's executable lines",
            "module_level": "sum covered/executable lines for the declared source prefix",
            "aggregate": "sum of the six blocking module counters; service is informational",
            "not_interchangeable": [
                "file-level observations cannot be substituted for module-level aggregates",
                "raw LLVM totals include files outside the blocking source boundaries",
            ],
        },
        "commands": {
            "tests": "cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked",
            "coverage": "python3 tools/quality/coverage.py --output target/quality/coverage.json",
            "format": "cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check",
            "lint": "cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings",
        },
    }
    write_json(output, result)
    output.with_suffix(".md").write_text(
        "# Measurement Contract Candidate\n\n"
        f"- Measured commit: `{result['baseline']['measured_commit']}`\n"
        f"- Review baseline: `{REVIEW_BASELINE}` ({REVIEW_BASELINE_VERSION})\n"
        f"- Target: `{result['target_triple']}`\n"
        f"- Test/coverage profile: `{PROFILE}`\n"
        f"- Target directory: `{target_directory}`\n"
        f"- Coverage instrumentation: `{INSTRUMENTATION}` via "
        f"`{result['measurements']['coverage']['instrumented_by']}`\n"
        f"- Child-process profile pattern: `{result['measurements']['coverage']['profile_file_pattern']}`\n\n"
        "This is candidate evidence. It becomes an accepted baseline only after "
        "review of the JSON, raw profiles, and matching commit/target/tool series.\n"
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "measurement-contract.json")
    args = parser.parse_args()
    raise SystemExit(run(args.output))
