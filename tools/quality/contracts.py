#!/usr/bin/env python3
"""Exercise the public CLI contract and compare normalized golden snapshots."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from quality_common import CRATE, QUALITY_ROOT, fail, metadata, write_json


SNAPSHOT = Path(__file__).with_name("snapshots") / "contracts.json"
JSON_PARSE_FAILURE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "json-results" / "error_object.py"
)


def normalize(value: str, root: Path) -> str:
    value = value.replace(str(root), "<PROJECT_ROOT>")
    value = re.sub(r"inv-\d+-\d{9}-\d+-\d+", "<INVOCATION_ID>", value)
    # These identities and evidence digests are intentionally derived from
    # each temporary fixture, so they must not make the textual golden
    # snapshot depend on the fixture name or commit timestamp.
    value = re.sub(r"\b(?:working-tree|git-tree):[0-9a-f]{40}\b", "<SOURCE_IDENTITY>", value)
    value = re.sub(
        r'("configuration_digest"\s*:\s*)"sha256:[0-9a-f]{64}"',
        r'\1"<CONFIGURATION_DIGEST>"',
        value,
    )
    value = re.sub(
        r'("commit"\s*:\s*)"[0-9a-f]{40}"',
        r'\1"<COMMIT>"',
        value,
    )
    value = re.sub(
        r'("sha256"\s*:\s*)"[0-9a-f]{64}"',
        r'\1"<SHA256>"',
        value,
    )
    value = re.sub(
        r'("size_bytes"\s*:\s*)\d+',
        r'\1"<SIZE_BYTES>"',
        value,
    )
    value = re.sub(r"\b20\d{2}-\d{2}-\d{2}T[^\s]+", "<TIMESTAMP>", value)
    value = re.sub(r"\b\d+ ms\b", "<DURATION>", value)
    value = re.sub(r"\b\d+\.\d+s\b", "<DURATION>", value)
    value = re.sub(r'("duration_ms"\s*:\s*)\d+', r'\1"<DURATION_MS>"', value)
    return value


def command(binary: Path, root: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), "--color", "never", "--project-root", str(root), *args],
        cwd=CRATE.parent.parent,
        text=True,
        capture_output=True,
        env={**os.environ, "LC_ALL": "C", "TZ": "UTC"},
    )


def init(root: Path, binary: Path, preset: str = "generic", git: bool = False) -> None:
    result = command(binary, root, ["init", "--preset", preset])
    if result.returncode != 0:
        raise RuntimeError(result.stderr or result.stdout)
    if git:
        subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.name", "Quality Test"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.email", "quality@example.invalid"], cwd=root, check=True)
        subprocess.run(["git", "add", "."], cwd=root, check=True)
        subprocess.run(["git", "commit", "--quiet", "-m", "fixture"], cwd=root, check=True)


def report_snapshot(root: Path, names: list[str]) -> dict[str, Any]:
    report_root = root / ".harness-gate" / "reports"
    snapshot: dict[str, Any] = {}
    for name in names:
        path = report_root / name
        snapshot[name] = {
            "exists": path.is_file(),
            "content": normalize(path.read_text(errors="replace"), root) if path.is_file() else None,
        }
    logs = report_root / "logs"
    if logs.is_dir():
        snapshot["logs"] = {
            str(path.relative_to(report_root)): normalize(path.read_text(errors="replace"), root)
            for path in sorted(logs.glob("*"))
            if path.is_file()
        }
    return snapshot


def scenario(
    binary: Path,
    name: str,
    args: list[str],
    expected: int,
    setup: str = "none",
    report_names: list[str] | None = None,
    expected_order: list[str] | None = None,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"harness-gate-contract-{name}-") as directory:
        root = Path(directory)
        if setup == "init":
            init(root, binary)
        elif setup == "git":
            init(root, binary, git=True)
        elif setup == "invalid":
            init(root, binary)
            (root / ".harness-gate" / "flow.toml").write_text("version = 2\ninvalid = true\n")
        elif setup == "verify-failure":
            init(root, binary, git=True)
            flow = root / ".harness-gate" / "flow.toml"
            flow.write_text(
                re.sub(
                    r'(\[\[steps\]\]\nid = "project\.diff-check".*?\nprogram = )"git"',
                    r'\1"false"',
                    flow.read_text(),
                    count=1,
                    flags=re.DOTALL,
                )
            )
        elif setup == "verify-parallel":
            init(root, binary, git=True)
            flow = root / ".harness-gate" / "flow.toml"
            source = flow.read_text()
            source = source.replace('profiles = ["hook"]', 'profiles = ["full", "hook"]')
            source = source.replace(
                "[execution]\nparallel = false\n",
                "[execution]\nparallel = true\nmax_parallel = 2\n",
            )
            flow.write_text(source)
        elif setup == "verify-json-parse-failure":
            init(root, binary, git=True)
            (root / "fixtures").mkdir()
            shutil.copy2(JSON_PARSE_FAILURE_FIXTURE, root / "fixtures" / "error_object.py")
            flow = root / ".harness-gate" / "flow.toml"
            interpreter = Path(sys.executable).name
            flow.write_text(
                flow.read_text()
                + f"""

[parsers.json-results]
kind = "json"
minimum = 1

[[steps]]
id = "fixture.json-parse-failure"
label = "JSON parse failure fixture"
component = "project"
profiles = ["full"]
program = {json.dumps(interpreter)}
args = ["fixtures/error_object.py"]
cwd = "{{root}}"
log = "json_parse_failure.log"
timeout_secs = 60
parser = "json-results"
"""
            )
        result = command(binary, root, args)
        stdout = normalize(result.stdout, root)
        stderr = normalize(result.stderr, root)
        combined = stdout + stderr
        error_codes = sorted(set(re.findall(r"ERROR \[(E\d{4})\]", combined)))
        selected_reports = report_names or []
        publication_order: list[str] | None = None
        report_path = root / ".harness-gate" / "reports" / "test_result.json"
        if report_path.is_file() and expected_order is not None:
            report = json.loads(report_path.read_text())
            publication_order = [step["label"] for step in report.get("steps", [])]
            if publication_order != expected_order:
                result = subprocess.CompletedProcess(
                    result.args,
                    1,
                    result.stdout,
                    result.stderr,
                )
        contract_error = None
        if name == "verify-json-parse-failure":
            if result.returncode == 0:
                contract_error = "fixture unexpectedly passed"
            elif not report_path.is_file():
                contract_error = "verification report was not written"
            else:
                report = json.loads(report_path.read_text())
                raw_log = root / ".harness-gate" / "reports" / "logs" / "json_parse_failure.log"
                if not raw_log.is_file() or raw_log.read_text().strip() != (
                    '{"duration_ms":5000,"status":"error"}'
                ):
                    contract_error = "fixture did not emit the expected JSON error object"
                fixture_steps = [
                    step
                    for step in report.get("steps", [])
                    if step.get("label") == "JSON parse failure fixture"
                ]
                if contract_error is not None:
                    pass
                elif len(fixture_steps) != 1:
                    contract_error = "fixture step missing or duplicated in report"
                else:
                    fixture_step = fixture_steps[0]
                    parser = fixture_step.get("parser")
                    if report.get("status") != "FAIL" or report.get("passed") is not False:
                        contract_error = "verification report did not record failure"
                    elif not any(
                        failure.get("code") == "RESULT_PARSE_FAILURE"
                        for failure in report.get("failures", [])
                    ):
                        contract_error = "verification report failures omitted RESULT_PARSE_FAILURE"
                    elif fixture_step.get("failure_code") != "RESULT_PARSE_FAILURE":
                        contract_error = "fixture did not report RESULT_PARSE_FAILURE"
                    elif not isinstance(parser, dict) or parser.get("complete") is not False:
                        contract_error = "fixture parser evidence was not incomplete"
                    elif fixture_step.get("passed") is not False:
                        contract_error = "fixture step unexpectedly passed"
        record = {
            "name": name,
            "args": args,
            "expected_exit_code": expected,
            "exit_code": result.returncode,
            "stdout": stdout,
            "stderr": stderr,
            "error_codes": error_codes,
            "ansi": "\x1b[" in combined,
            "reports": report_snapshot(root, selected_reports),
            "status": "pass"
            if result.returncode == expected and contract_error is None
            else "fail",
        }
        if contract_error is not None:
            record["contract_error"] = contract_error
        if expected_order is not None:
            record["publication_order"] = publication_order
        return record


def collect(binary: Path, staged_secrets: bool = True) -> list[dict[str, Any]]:
    return [
        scenario(binary, "help", ["--help"], 0),
        scenario(binary, "version", ["--version"], 0),
        scenario(binary, "presets", ["presets"], 0),
        scenario(binary, "init", ["init", "--preset", "generic"], 0),
        scenario(binary, "schema", ["schema", "export", "--output", "schema/flow.schema.json"], 0),
        scenario(binary, "config-valid", ["config", "check"], 0, "init"),
        scenario(binary, "config-json-valid", ["config", "check", "--format", "json"], 0, "init"),
        scenario(binary, "config-invalid", ["config", "check"], 1, "invalid"),
        scenario(binary, "config-json-invalid", ["config", "check", "--format", "json"], 1, "invalid"),
        scenario(binary, "scope", ["scope", "--all"], 0, "git", ["scope.json"]),
        scenario(binary, "secrets", ["secrets", *( ["--staged"] if staged_secrets else [] )], 0, "git", ["secret_scan.json"]),
        scenario(binary, "audit", ["audit"], 0, "init", ["review_context.json", "review_context.md"]),
        scenario(
            binary,
            "verify-success",
            ["verify", "--all"],
            0,
            "git",
            ["test_result.json", "test_result.md"],
            ["secret scan", "architecture audit", "Git whitespace check"],
        ),
        scenario(
            binary,
            "verify-parallel-success",
            ["verify", "--all"],
            0,
            "verify-parallel",
            ["test_result.json", "test_result.md"],
            ["secret scan", "architecture audit", "Git whitespace check", "staged Git whitespace check"],
        ),
        scenario(binary, "verify-failure", ["verify", "--all"], 1, "verify-failure", ["test_result.json", "test_result.md"]),
        scenario(
            binary,
            "verify-json-parse-failure",
            ["verify", "--all"],
            1,
            "verify-json-parse-failure",
            ["test_result.json", "test_result.md"],
        ),
        scenario(binary, "verify-one-step", ["step", "project.diff-check"], 0, "git", ["test_result.json", "test_result.md"]),
        scenario(binary, "unknown-profile", ["verify", "--all", "--profile", "missing"], 1, "init"),
        scenario(binary, "unknown-step", ["step", "missing.step"], 1, "init"),
        scenario(binary, "unknown-command", ["missing-command"], 2),
    ]


def run(output: Path, accept: bool = False, structured: bool = False) -> int:
    from ci_cargo import target_directory

    # A restored executable is disposable cache state. Cargo must establish
    # freshness before the binary can supply required contract evidence.
    subprocess.run(["cargo", "build", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked"], check=True, cwd=CRATE.parent.parent)
    binary = target_directory() / "debug" / ("harness-gate.exe" if os.name == "nt" else "harness-gate")
    scenarios = collect(binary, staged_secrets=not structured)
    report = {**metadata(tool="cli-contracts", snapshot_version=1), "scenarios": scenarios}
    write_json(output, report)
    if accept:
        SNAPSHOT.parent.mkdir(parents=True, exist_ok=True)
        write_json(SNAPSHOT, {"snapshot_version": 1, "scenarios": scenarios})
    elif not structured and SNAPSHOT.exists():
        expected = json.loads(SNAPSHOT.read_text())
        actual = {"snapshot_version": 1, "scenarios": scenarios}
        if actual != expected:
            fail(f"CLI contract snapshot differs; review {SNAPSHOT} and rerun with --accept")
    failures = [item["name"] for item in scenarios if item["status"] != "pass" or item["ansi"]]
    if failures:
        fail(f"contract scenarios failed: {', '.join(failures)}")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "contracts.json")
    parser.add_argument("--accept", action="store_true", help="write a reviewed snapshot; never use in CI")
    parser.add_argument("--structured", action="store_true", help="assert structured contracts without a Linux textual snapshot")
    args = parser.parse_args()
    raise SystemExit(run(args.output, args.accept, args.structured))
