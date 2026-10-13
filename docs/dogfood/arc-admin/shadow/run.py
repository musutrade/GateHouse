#!/usr/bin/env python3
"""Repeat full shadow commands in a fresh disposable snapshot inside this workspace.

Requires authenticated gh, git, cargo, npm, Docker and a built harness-gate.
Retains raw outputs/exit codes; never changes authority or rewrites flow steps.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import time

ROOT = Path(__file__).resolve().parents[4]
ARC = Path(__file__).resolve().parent.parent
COMMIT = json.loads((ARC / "source-manifest.json").read_text())["commit"]
REMOVED_ENV = ("DATABASE_URL", "TEST_DATABASE_URL", "ARC_FLOW_CONFIG", "ARC_FLOW_REPORTS",
               "ARC_FLOW_AUDIT_CONFIG", "ARC_FLOW_SECRETS_CONFIG", "AUDITOR_CONFIG",
               "REPORT_DIR", "PROJECT_ROOT")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new directory under workspace target/")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / "target").resolve()):
        parser.error("output must be inside this workspace's target/")
    output.mkdir(parents=True, exist_ok=False)
    source = output / "source"
    env = {k: v for k, v in os.environ.items() if k not in REMOVED_ENV}
    env.update(CARGO_TARGET_DIR=str(output / "cargo-target"),
               npm_config_cache=str(output / "npm-cache"), TMPDIR=str(output / "tmp"),
               GIT_CEILING_DIRECTORIES=str(output / "tmp"))
    (output / "tmp").mkdir()
    receipts = []
    # Whitelist CI identity only; never retain the runner's full environment.
    (output / "timing-context.json").write_text(json.dumps({
        "source_commit": COMMIT,
        "clock": "time.monotonic_ns",
        "unit": "seconds",
        "ci_identity": {key: os.environ.get(key) for key in (
            "GITHUB_REPOSITORY", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT",
            "GITHUB_JOB", "GITHUB_SHA", "RUNNER_OS", "RUNNER_ARCH")},
        "limitation": "Command elapsed time only. Obtain job occupancy and queue times from Actions job timestamps; local receipts do not establish self-hosted CI cost.",
    }, indent=2) + "\n")

    def run(name, command, cwd=ROOT, required=False):
        started_at = datetime.now(timezone.utc).isoformat()
        started_ns = time.monotonic_ns()
        with (output / (name + ".log")).open("wb") as log:
            result = subprocess.run(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT, check=False)
        elapsed_seconds = (time.monotonic_ns() - started_ns) / 1_000_000_000
        receipts.append({"name": name, "command": command, "cwd": str(cwd), "exit_code": result.returncode,
                         "started_at": started_at, "completed_at": datetime.now(timezone.utc).isoformat(),
                         "elapsed_seconds": elapsed_seconds})
        (output / "commands.json").write_text(json.dumps(receipts, indent=2) + "\n")
        if required and result.returncode:
            raise SystemExit(f"prerequisite failed: {name}; see {output / (name + '.log')}")

    run("current-main", ["gh", "api", "repos/musutrade/arc-admin/commits/main", "--jq", ".sha"], required=True)
    with (output / "source.tar.gz").open("wb") as archive:
        subprocess.run(["gh", "api", f"repos/musutrade/arc-admin/tarball/{COMMIT}"], stdout=archive, check=True)
    source.mkdir()
    with tarfile.open(output / "source.tar.gz") as archive:
        members = archive.getmembers()
        for member in members:
            member.name = member.name.partition("/")[2]
        archive.extractall(source, members=[m for m in members if m.name], filter="data")
    for name, command in (
        ("git-init", ["git", "init", "-q"]),
        ("git-fetch", ["git", "fetch", "--depth=1", "https://github.com/musutrade/arc-admin.git", COMMIT]),
        ("git-index", ["git", "read-tree", "FETCH_HEAD"]),
        ("git-head", ["git", "update-ref", "HEAD", COMMIT]),
    ):
        run(name, command, source, required=True)
    run("source-before", ["git", "diff", "--exit-code", "HEAD"], source, required=True)
    for folder in (".shadow-execution", ".harness-gate"):
        (source / folder).mkdir()
        shutil.copy2(ARC / "import/flow.toml", source / folder / "flow.toml")
    shutil.copy2(ARC / "quality/quality.toml", source / ".harness-gate/quality.toml")
    shutil.copytree(ARC / "quality/packs", source / ".harness-gate/packs")
    run("npm-ci", ["npm", "ci", "--prefix", "frontend"], source, required=True)
    run("arc-scope", ["cargo", "flow", "scope", "--all"], source)
    run("arc-full", ["cargo", "flow", "verify", "--profile", "full", "--all"], source)
    reports = source / "codex-audit-pipeline/.codex/reports"
    shutil.copytree(reports, output / "arc-reports")

    def hashes():
        return {str(p.relative_to(reports)): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in reports.rglob("*") if p.is_file()}

    before = hashes()
    binary = ROOT / "target/debug/harness-gate"
    for name, config in (("execution", ".shadow-execution/flow.toml"), ("quality", ".harness-gate/flow.toml")):
        command = [str(binary), "--project-root", str(source), "--config", config]
        run(f"harness-{name}-scope", command + ["scope", "--all"])
        run(f"harness-{name}-full", command + ["verify", "--profile", "full", "--all"])
        after = hashes()
        (output / f"harness-{name}-report-changes.json").write_text(json.dumps(
            {p: h for p, h in after.items() if before.get(p) != h}, indent=2) + "\n")
        before = after
    run("source-after", ["git", "diff", "--exit-code", "HEAD"], source, required=True)
    print(f"Shadow evidence retained in {output}; inspect command results, not workflow state.")


if __name__ == "__main__":
    main()
