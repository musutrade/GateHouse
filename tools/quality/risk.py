#!/usr/bin/env python3
"""Build risk.json/risk.md from two retained, commit-bound measurement bundles."""
from __future__ import annotations

import argparse
from datetime import date
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

from function_risk import CRAP_VERSION, MAPPING_VERSION, crap_line, map_functions
from production_coverage import load_inventory, require, summarize
from quality_common import ROOT, read_json, sha256, write_json

SCHEMA_VERSION = 1
POLICY_VERSION = "incremental-risk-1"
TOOLS = {"rustc", "llvm_cov", "llvm_profdata", "cargo_llvm_cov", "nextest", "python"}
HOTSPOTS = {
    ("src/doctor/checks.rs", "run_check"), ("src/app/commands.rs", "run"),
    ("src/verify/mod.rs", "run_selected"), ("src/verify/steps.rs", "configured_task"),
    ("src/process/adapter.rs", "run_with_cancel"), ("src/verify/parser.rs", "count_json_results"),
}


def nonempty(value, label):
    require(isinstance(value, str) and bool(value.strip()), f"missing/invalid {label}")
    return value


def artifact(root: Path, entry: dict) -> Path:
    path = (root / nonempty(entry["path"], "artifact path")).resolve()
    require(path.is_relative_to(root.resolve()), "artifact escapes evidence bundle")
    require(path.is_file() and path.stat().st_size > 0, f"missing/empty artifact: {path}")
    require(re.fullmatch(r"[0-9a-f]{64}", entry["sha256"]) is not None and sha256(path) == entry["sha256"],
            f"stale/corrupt artifact: {path}")
    return path


def git_source(commit: str, relative: str, repo: Path) -> bytes:
    return subprocess.check_output(["git", "show", f"{commit}:tools/harness-gate/{relative}"],
                                   cwd=repo, stderr=subprocess.PIPE)


def validate_run(root: Path, manifest: dict) -> dict:
    run = dict(manifest["run"])
    run_id = nonempty(run["id"], "run id")
    require(re.fullmatch(r"[A-Za-z0-9_-]+", run_id) is not None, "invalid run id")
    start, end = run["started_ns"], run["finished_ns"]
    require(type(start) is int and type(end) is int and 0 < start <= end, "invalid run timestamps")
    require(run["commit"] == manifest["commit"] and run["target"] == manifest["target"],
            "test run commit/target mismatch")
    require(run["status"] == "passed", "missing/failed/cancelled test run")
    require(run["instrumentation"] == "-C instrument-coverage", "CLI coverage instrumentation absent")
    nonempty(run["command"], "test command")
    try:
        junit = ET.parse(artifact(root, run["results"])).getroot()
    except ET.ParseError as error:
        raise ValueError(f"invalid JUnit results: {error}") from error
    require(junit.tag in ("testsuites", "testsuite"), "expected JUnit test results")
    test_results = {}
    for case in junit.iter("testcase"):
        identifier = nonempty(case.get("classname"), "test classname") + "::" + nonempty(case.get("name"), "test name")
        require(identifier not in test_results, "duplicate JUnit test identity")
        require(case.find("failure") is None and case.find("error") is None, "failed test result")
        test_results[identifier] = "skipped" if case.find("skipped") is not None else "passed"
    require(bool(test_results) and "passed" in test_results.values(), "empty/non-executed test results")
    for suite in junit.iter("testsuite"):
        require(int(suite.get("failures", "0")) == 0 and int(suite.get("errors", "0")) == 0,
                "failed test suite")
    run["test_results"] = test_results
    require(bool(run["binaries"]) and bool(run["profiles"]), "missing instrumented binaries/profiles")
    binaries = {}
    for binary in run["binaries"]:
        identifier = nonempty(binary["id"], "binary id")
        require(identifier not in binaries, "duplicate binary id")
        path = artifact(root, binary["artifact"])
        binary_data = path.read_bytes()
        require(binary_data[:4] in (b"\x7fELF", b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf") or
                binary_data[:2] == b"MZ", "unsupported/non-executable binary artifact")
        # Rust/LLVM embeds profile section names in instrumented executables.
        require(b"__llvm_prf" in binary_data, "binary has no LLVM profile sections")
        require(binary["role"] in ("cli", "test"), "unknown binary role")
        binaries[identifier] = binary["role"]
    require("cli" in binaries.values() and "test" in binaries.values(), "missing CLI/test binary")
    profiles = set()
    seen = set()
    for profile in run["profiles"]:
        path = artifact(root, profile["artifact"])
        require(path not in seen, "duplicate raw profile")
        seen.add(path)
        require(profile["run_id"] == run_id and profile["binary_id"] in binaries,
                "stale/unknown profile provenance")
        require(start <= path.stat().st_mtime_ns <= end, f"stale profile timestamp: {path}")
        require(path.suffix == ".profraw" and run_id in path.parts, "profile not isolated by run id")
        with path.open("rb") as stream:
            header = stream.read(16)
        require(len(header) == 16 and any(int.from_bytes(header[:8], order) in
                (0xFF6C70726F667281, 0xFF6C70726F665281) for order in ("little", "big")),
                "invalid LLVM raw profile header")
        profiles.add(profile["binary_id"])
    require(profiles == set(binaries), "missing subprocess/test binary profiles")
    return run


def snapshot(path: Path, expected_commit: str, repo: Path = ROOT) -> dict:
    manifest = read_json(path)
    root = path.parent
    require(manifest["schema_version"] == SCHEMA_VERSION, "unsupported bundle schema")
    require(re.fullmatch(r"[0-9a-f]{40}", expected_commit) is not None and
            manifest["commit"] == expected_commit, "missing/mismatched base/head commit")
    nonempty(manifest["target"], "target triple")
    require(set(manifest["tools"]) == TOOLS, "missing/unknown tool versions")
    for name, version in manifest["tools"].items():
        nonempty(version, name)
    require(manifest["profile"] == "dev" and manifest["instrumentation"] == "-C instrument-coverage",
            "unsupported measurement profile/instrumentation")
    require(manifest["branch"]["status"] == "unsupported", "branch-supported series is not implemented")
    require(set(manifest["branch"]) == {"status", "reason", "tool_version"}, "unsupported branch must not have counters")
    nonempty(manifest["branch"]["reason"], "branch unsupported reason")
    require(manifest["branch"]["tool_version"] == manifest["tools"]["llvm_cov"], "branch tool mismatch")
    run = validate_run(root, manifest)
    crate = (root / manifest["source_root"]).resolve()
    require(crate.is_relative_to(root.resolve()) and crate.is_dir(), "source snapshot escapes/missing")
    coverage_root = Path(nonempty(manifest["coverage_root"], "recorded coverage root"))
    require(coverage_root.is_absolute(), "recorded coverage root must be absolute")
    inventory_path = artifact(root, manifest["inventory"])
    inventory, owners, _ = load_inventory(inventory_path, crate)
    # Compare to immutable Git objects in this workspace, never another checkout.
    tracked = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", expected_commit,
                                       "tools/harness-gate/src"], cwd=repo, text=True).splitlines()
    expected_sources = {p.removeprefix("tools/harness-gate/") for p in tracked if p.endswith(".rs")}
    require(expected_sources == {p.relative_to(crate).as_posix() for p in (crate / "src").rglob("*.rs")},
            "source snapshot differs from commit inventory")
    for source in sorted(expected_sources):
        require((crate / source).read_bytes() == git_source(expected_commit, source, repo),
                f"source snapshot does not match commit: {source}")
    raw = artifact(root, manifest["llvm"])
    lcov = artifact(root, manifest["lcov"])
    try:
        require(ET.parse(artifact(root, manifest["cobertura"])).getroot().tag == "coverage", "invalid Cobertura root")
    except ET.ParseError as error:
        raise ValueError(f"invalid Cobertura: {error}") from error
    for key in ("llvm", "lcov", "cobertura"):
        require(manifest[key]["run_id"] == run["id"], "coverage export from stale run")
        require(manifest[key]["profiles_sha256"] == digest(run["profiles"]), "coverage profile set mismatch")
    coverage = summarize(raw, lcov, inventory_path, crate, coverage_root=coverage_root)
    records = [read_json(artifact(root, entry)) for entry in manifest["complexity"]]
    require(bool(records), "missing complexity records")
    for record in records:
        require(record["series"]["toolchain"]["python"] == manifest["tools"]["python"],
                "complexity Python tool mismatch")
    analyzer_series = [{k: record["series"][k] for k in ("analyzer", "rule", "toolchain")} for record in records]
    require(all(s == analyzer_series[0] for s in analyzer_series), "mixed complexity series")
    functions = map_functions(records, read_json(raw), crate, owners, expected_commit,
                              {r["path"] for r in inventory["unmapped_sources"]}, coverage_root=coverage_root)
    for function in functions:
        function["evidence"] = {"bundle": str(path), "bundle_sha256": sha256(path),
                                "llvm": str(raw), "llvm_sha256": sha256(raw),
                                "complexity": next(entry for entry, record in zip(manifest["complexity"], records)
                                                   if record["series"]["source"]["path"] == function["source"]["path"])}
    series = {"mapping": MAPPING_VERSION, "crap": CRAP_VERSION, "policy": POLICY_VERSION, "complexity": analyzer_series[0],
              "target": manifest["target"], "tools": manifest["tools"], "profile": manifest["profile"],
              "instrumentation": manifest["instrumentation"], "branch": manifest["branch"],
              "inventory_version": inventory["version"], "coverage_metrics": inventory["metrics_version"]}
    return {"commit": expected_commit, "series": series, "series_id": digest(series),
            "functions": functions, "coverage": coverage, "run": run, "bundle": str(path)}


def digest(value) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def evaluate(base: dict, head: dict, policy: dict, today: date | None = None) -> dict:
    require(base["series"] == head["series"], "incompatible base/head series/tools; baseline review required")
    require(policy["schema_version"] == 1, "unsupported risk policy")
    today = today or date.today()
    old = {f["id"]: f for f in base["functions"]}
    new = {f["id"]: f for f in head["functions"]}
    require(len(old) == len(base["functions"]) and len(new) == len(head["functions"]), "duplicate function identity")
    mappings = policy["identity_mappings"]
    require(len({m["base"] for m in mappings}) == len(mappings) and
            len({m["head"] for m in mappings}) == len(mappings), "identity mapping must be one-to-one; splits are new")
    explicit = {}
    for mapping in mappings:
        require(mapping["base"] in old and mapping["head"] in new, "unknown identity mapping")
        nonempty(mapping["reason"], "identity mapping reason")
        explicit[mapping["head"]] = old[mapping["base"]]
    high_risk = set(policy["mandatory_symbols"]) | set(policy["hotspot_symbols"])
    require(high_risk <= new.keys(), "missing mandatory/hotspot source symbol")
    high_risk |= {f["id"] for f in new.values()
                  if (f["source"]["path"], f["qualified_name"]) in HOTSPOTS}
    for previous in old.values():
        if (previous["source"]["path"], previous["qualified_name"]) not in HOTSPOTS:
            continue
        successors = {f["id"] for f in new.values()
                      if (f["source"]["path"], f["qualified_name"], f["kind"]) ==
                      (previous["source"]["path"], previous["qualified_name"], previous["kind"])}
        successors |= {identifier for identifier, source in explicit.items() if source["id"] == previous["id"]}
        require(bool(successors), "missing selected hotspot; source moves require explicit identity mapping")
        high_risk |= successors
    tests = policy["boundary_tests"]
    for test in tests:
        require(test["symbol"] in new and test["commit"] == head["commit"] and
                test["target"] == head["series"]["target"] and test["run_id"] == head["run"]["id"],
                "boundary test source/run mismatch")
        require(test["status"] == "passed", "boundary test skipped/failed/cancelled")
        nonempty(test["test_id"], "boundary test id")
        nonempty(test["observable"], "boundary test failure observable")
        # Artifacts must already be retained and hashed in the head bundle.
        require(test["results"] == head["run"]["results"], "boundary test results not from head run")
        require(head["run"]["test_results"].get(test["test_id"]) == "passed",
                "boundary test absent/skipped in retained results")
    exceptions = policy["exceptions"]
    policy_failures = []
    for exception in exceptions:
        for field in ("symbol", "issue", "owner", "approver", "reason", "expires", "compensating_controls"):
            nonempty(exception[field], f"exception {field}")
        require(exception["symbol"] in new, "unknown exception symbol")
        if date.fromisoformat(exception["expires"]) <= today:
            policy_failures.append(f"expired exception: {exception['symbol']}")
    require(head["coverage"]["status"] in ("pass", "fail"), "missing production coverage status")
    if head["coverage"]["status"] == "fail":
        policy_failures.append("production coverage boundaries failed")
    functions, debt, failures, identities = [], [], list(policy_failures), []
    consumed = set()
    for function in head["functions"]:
        f = dict(function)
        candidates = [o for o in old.values() if o["source"]["path"] == f["source"]["path"] and
                      o["qualified_name"] == f["qualified_name"] and o["kind"] == f["kind"]]
        previous = explicit.get(f["id"])
        if previous is None and len(candidates) == 1:
            previous = candidates[0]
        if previous is not None:
            require(previous["id"] not in consumed, "base identity reused; split functions must be new")
            consumed.add(previous["id"])
        same_body = previous is not None and previous["body_sha256"] == f["body_sha256"]
        moved = previous is not None and (previous["source"]["path"], previous["qualified_name"]) != (f["source"]["path"], f["qualified_name"])
        f["change"] = "new" if previous is None else "moved" if moved else "unchanged" if same_body else "modified"
        # A move is rechecked, even with identical source. Splits cannot inherit debt.
        changed = f["change"] != "unchanged"
        f["high_risk"] = f["cc"] > 10 or f["id"] in high_risk
        score = crap_line(f["cc"], f["lines"]["covered"], f["lines"]["count"])
        f["crap_line"] = float(score)
        f["crap_line_exact"] = {"numerator": score.numerator, "denominator": score.denominator}
        violations = []
        if score > 30:
            violations.append("crap_line > 30; add failure-path coverage or decompose")
        if f["high_risk"]:
            for metric in ("lines", "regions"):
                c = f[metric]
                require(type(c["count"]) is int and type(c["covered"]) is int and
                        0 <= c["covered"] <= c["count"] and c["count"] > 0, f"invalid {metric} counts")
                if c["covered"] * 100 < 80 * c["count"]:
                    violations.append(f"{metric} < 80%; cover actual function boundaries")
            if not any(t["symbol"] == f["id"] for t in tests):
                violations.append("missing passing boundary test with failure observable")
        # Selected/mandatory hotspots remain blocking even if unchanged. Other
        # historical violations are reported debt, never labelled function pass.
        blocking = changed or f["id"] in high_risk
        f["violations"] = violations
        f["status"] = "fail" if violations and blocking else "debt" if violations else "pass"
        f["exceptions"] = [e for e in exceptions if e["symbol"] == f["id"]]
        if f["status"] == "fail":
            failures.append(f["id"])
        elif f["status"] == "debt":
            debt.append(f["id"])
        identities.append({"base": previous["id"] if previous else None, "head": f["id"], "change": f["change"]})
        functions.append(f)
    return {"schema_version": SCHEMA_VERSION, "crap_version": CRAP_VERSION,
            "base_commit": base["commit"], "head_commit": head["commit"],
            "series": head["series"], "series_id": head["series_id"],
            "branch": head["series"]["branch"], "status": "fail" if failures else "pass-with-debt" if debt else "pass",
            "gate_scope": "incremental functions plus selected/mandatory hotspots; not whole-project acceptance",
            "failures": failures, "historical_debt": debt, "functions": functions,
            "identity_mappings": identities, "removed": sorted(set(old) - consumed),
            "coverage": head["coverage"], "policy": policy, "evaluated_on": today.isoformat()}


def markdown(report: dict) -> str:
    text = "# Function risk evidence\n\n" + f"Status: **{report['status']}**\n\n"
    if report["status"] == "measurement-error":
        return text + report["error"] + "\n"
    text += (f"Base: `{report['base_commit']}`; head: `{report['head_commit']}`.\n\n"
             f"Series: `{report['series_id']}`. Branch: **unsupported** — {report['branch']['reason']} "
             f"(`{report['branch']['tool_version']}`).\n\n"
             "CRAP decisions use exact rational values; decimals below are display only. "
             "Line, function and region counters are independent. Historical debt is not whole-project acceptance.\n\n"
             "| Source symbol | Change / status | CC | Line | Function | Region | crap_line | Evidence |\n"
             "| --- | --- | ---: | --- | --- | --- | ---: | --- |\n")
    for f in report["functions"]:
        metrics = [f"{f[m]['covered']}/{f[m]['count']}" for m in ("lines", "functions", "regions")]
        text += (f"| `{f['id']}` | {f['change']} / {f['status']} | {f['cc']} | " + " | ".join(metrics) +
                 f" | {f['crap_line']:.6f} | [LLVM]({f['evidence']['llvm']}) / [bundle]({f['evidence']['bundle']}) |\n")
    text += "\n## Historical debt\n\n" + "\n".join(f"- `{v}`" for v in report["historical_debt"])
    text += "\n\n## Failures\n\n" + "\n".join(f"- `{f['id']}`: {'; '.join(f['violations'])}"
                                                       for f in report["functions"] if f["violations"])
    text += "\n" + "\n".join(f"- {v}" for v in report["failures"] if v not in {f["id"] for f in report["functions"]})
    return text + "\n"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--head", type=Path, required=True)
    parser.add_argument("--base-sha", required=True)
    parser.add_argument("--head-sha", required=True)
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/quality/risk.json")
    args = parser.parse_args(argv)
    try:
        base = snapshot(args.base, args.base_sha)
        head = snapshot(args.head, args.head_sha)
        report = evaluate(base, head, read_json(args.policy))
        report["command"] = [sys.executable, "tools/quality/risk.py", *(argv if argv is not None else sys.argv[1:])]
        report["raw_artifacts"] = {str(p): sha256(p) for p in (args.base, args.head, args.policy)}
    except (OSError, ValueError, KeyError, TypeError, IndexError, subprocess.CalledProcessError) as error:
        report = {"schema_version": SCHEMA_VERSION, "status": "measurement-error", "error": str(error)}
    write_json(args.output, report)
    args.output.with_suffix(".md").write_text(markdown(report))
    print(f"function risk: {report['status']} ({args.output})")
    return 1 if report["status"] in ("fail", "measurement-error") else 0


if __name__ == "__main__":
    raise SystemExit(main())
