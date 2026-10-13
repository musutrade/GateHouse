#!/usr/bin/env python3
"""Isolated acceptance: core build and tests without plugin sources.

Takes one full commit, extracts a read-only snapshot into a fresh temporary
directory, removes the plugin implementation paths assigned to GateProbes by
the core/plugin split, and runs the core build, the full cargo test set and the
core Python test selection in isolation. The original repository is never
modified; a local git HEAD is created inside the copy because quality tools
record commit metadata. A missing required input or exclusion path fails
explicitly instead of being skipped.

This entry point is manual acceptance tooling. It is not wired into any
workflow: the ordinary selector verifies the exact accepted handoff; no separate test
exclusion list is used. Already transferred implementation paths may be absent.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CARGO_MANIFEST = 'tools/harness-gate/Cargo.toml'
FULL_SHA = re.compile(r'^[0-9a-f]{40}$')
TEST_RESULT = re.compile(
    r'^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; \d+ ignored; '
    r'\d+ measured; (\d+) filtered out', re.M)
UNITTEST_SUMMARY = re.compile(r'^Ran (\d+) tests? in ', re.M)
UNITTEST_RESULT = re.compile(r'^(OK|FAILED)(?: \(([^)]*)\))?$', re.M)

# Plugin implementation sources owned by GateProbes (`CORE_PLUGIN_SPLIT.md`,
# design boundary table). The list must stay exact: a missing path fails the
# check so the isolation never silently stops excluding moved code.
PLUGIN_IMPLEMENTATION = (
    'tools/quality/rust-collector-runtime',
    'tools/quality/rust-measure',
    'tools/quality/rust-native-driver',
    'tools/quality/rust-native-plugin',
    'tools/quality/rust-source-risk',
    'tools/quality/typescript-risk',
    'tools/quality/http-json-contract',
    'tools/quality/build_rust_collector.py',
    'tools/quality/collect_typescript_contracts.py',
    'tools/quality/rust_collector_contract.py',
    'tools/quality/rust_collector_delivery.py',
    'tools/quality/rust_collector_entry.py',
    'tools/quality/rust_collector_project.py',
    'tools/quality/rust_native.py',
    'tools/quality/rust_native_classify.py',
    'tools/quality/rust_native_config.py',
    'tools/quality/rust_native_driver.py',
    'tools/quality/rust_native_policy.py',
    'tools/quality/rust_native_replay.py',
    'tools/quality/typescript_acceptance.py',
    'tools/quality/typescript_advisory.py',
    'tools/quality/typescript_contracts.py',
    'tools/quality/typescript_reference.py',
    'tools/quality/typescript_semantics.py',
)

REQUIRED_CORE_INPUTS = (
    CARGO_MANIFEST,
    'tools/harness-gate/quality-core/mod.rs',
    'tools/quality/ci_quality.py',
    'tools/quality/rust-measure-release.json',
    'schema/flow.schema.json',
    'install.sh',
    'docs/quality/gh-146/python-boundary.json',
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f'error: {message}')


def run(argv: list[str], cwd: Path, log: Path | None = None,
        env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True)
    if log is not None:
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text(result.stdout + result.stderr)
    return result


def snapshot(commit: str, source: Path) -> None:
    archive = source / 'core-source.tar'
    created = run(['git', 'archive', '--format=tar', f'--output={archive}', commit], ROOT)
    require(created.returncode == 0, f'cannot archive {commit}: {created.stderr.strip()}')
    with tarfile.open(archive) as tar:
        tar.extractall(source / 'src', filter='data')


def exclude_plugin_implementation(copy: Path, require_all: bool = True) -> list[str]:
    removed = []
    for relative in PLUGIN_IMPLEMENTATION:
        path = copy / relative
        require(not path.is_symlink(), f'symlink plugin implementation path: {relative}')
        if not path.exists():
            require(not require_all, f'plugin implementation path is missing: {relative}')
            continue
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
        removed.append(relative)
    for relative in PLUGIN_IMPLEMENTATION:
        require(not (copy / relative).exists(), f'plugin implementation still present: {relative}')
    return removed


def init_local_git(copy: Path) -> str:
    identity = {
        'GIT_AUTHOR_NAME': 'Core Isolation', 'GIT_AUTHOR_EMAIL': 'isolation@example.invalid',
        'GIT_COMMITTER_NAME': 'Core Isolation', 'GIT_COMMITTER_EMAIL': 'isolation@example.invalid',
        'GIT_AUTHOR_DATE': '2026-01-01T00:00:00Z', 'GIT_COMMITTER_DATE': '2026-01-01T00:00:00Z',
    }
    environment = {**os.environ, **identity}
    for argv in (['git', 'init', '-q'], ['git', 'add', '-A'],
                 ['git', 'commit', '-q', '-m', 'isolated core source snapshot']):
        result = run(argv, copy, env=environment)
        require(result.returncode == 0, f'{" ".join(argv)} failed: {result.stderr.strip()}')
    return run(['git', 'rev-parse', 'HEAD'], copy).stdout.strip()


def check_required_inputs(copy: Path) -> None:
    missing = [relative for relative in REQUIRED_CORE_INPUTS if not (copy / relative).exists()]
    require(not missing, 'required core inputs are missing: ' + ', '.join(missing))


def cargo(copy: Path, workdir: Path, command: str) -> dict[str, object]:
    environment = {**os.environ, 'CARGO_TARGET_DIR': str(workdir / 'target')}
    result = run(['cargo', command, '--locked', '--manifest-path', CARGO_MANIFEST],
                 copy, log=workdir / 'logs' / f'cargo-{command}.log', env=environment)
    # Cargo reports one filtered listing run per binary besides the real runs.
    counts = [row for row in TEST_RESULT.findall(result.stdout) if row[3] == '0']
    return {
        'command': f'cargo {command} --locked --manifest-path {CARGO_MANIFEST}',
        'exit_code': result.returncode,
        'passed': sum(int(row[1]) for row in counts),
        'failed': sum(int(row[2]) for row in counts),
    }


def python_selection(copy: Path, workdir: Path) -> dict[str, object]:
    environment = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}
    command = [sys.executable, 'tools/quality/tests/run_core.py']
    selection = run(command + ['--list'], copy, env=environment)
    require(selection.returncode == 0, 'ordinary Core selector rejected isolation: ' + selection.stderr)
    selected = selection.stdout.splitlines()
    result = run(command, copy, log=workdir / 'logs' / 'python-selection.log', env=environment)
    output = result.stdout + result.stderr
    summary = UNITTEST_SUMMARY.search(output)
    outcome = UNITTEST_RESULT.search(output)
    counts = dict(item.split('=') for item in (outcome.group(2) or '').split(', ') if '=' in item) if outcome else {}
    report = {
        'selected_modules': selected,
        'command': 'python3 tools/quality/tests/run_core.py',
        'tests_run': int(summary.group(1)) if summary else 0,
        'failed': int(counts.get('failures', 0)) + int(counts.get('errors', 0)),
        'skipped': int(counts.get('skipped', 0)),
        'exit_code': result.returncode,
    }
    report['passed'] = report['tests_run'] - report['failed'] - report['skipped']
    if result.returncode != 0:
        report['failures'] = [
            line for line in output.splitlines()
            if re.match(r'^(ERROR|FAIL): ', line)
        ]
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--commit', required=True,
                        help='full 40-hex commit to verify; maintained main or a reviewed PR head')
    parser.add_argument('--workdir', type=Path,
                        help='working directory (default: a fresh temporary directory)')
    parser.add_argument('--report', type=Path, help='JSON report path (default: <workdir>/result.json)')
    parser.add_argument('--skip-cargo', action='store_true')
    parser.add_argument('--skip-python', action='store_true')
    args = parser.parse_args()

    require(FULL_SHA.fullmatch(args.commit) is not None,
            '--commit must be a full 40-hex commit, not a branch, tag or short SHA')
    resolved = run(['git', 'rev-parse', f'{args.commit}^{{commit}}'], ROOT)
    require(resolved.returncode == 0, f'unknown commit: {args.commit}')

    workdir = args.workdir or Path(tempfile.mkdtemp(prefix='core-isolation-'))
    require(not (workdir / 'src').exists(), f'working directory already contains src: {workdir}')
    workdir.mkdir(parents=True, exist_ok=True)
    report: dict[str, object] = {'commit': args.commit, 'workdir': str(workdir), 'status': 'fail'}

    snapshot(args.commit, workdir)
    copy = workdir / 'src'
    report['excluded_plugin_implementation'] = exclude_plugin_implementation(copy, require_all=False)
    report['local_git_head'] = init_local_git(copy)
    check_required_inputs(copy)
    hydration = run([sys.executable, 'tools/maintenance/test_data.py'], copy,
                    log=workdir / 'logs' / 'test-data.log')
    require(hydration.returncode == 0, 'verified regression data restore failed: ' + hydration.stderr)
    report['test_data'] = json.loads(hydration.stdout)

    if not args.skip_cargo:
        build = cargo(copy, workdir, 'build')
        report['cargo_build'] = build
        if build['exit_code'] == 0:
            report['cargo_test'] = cargo(copy, workdir, 'test')
    if not args.skip_python:
        report['python_selection'] = python_selection(copy, workdir)

    ok = True
    if not args.skip_cargo:
        build = report.get('cargo_build')
        test = report.get('cargo_test')
        ok = ok and isinstance(build, dict) and build.get('exit_code') == 0
        ok = ok and isinstance(test, dict) and test.get('exit_code') == 0
    if not args.skip_python:
        selection = report.get('python_selection')
        ok = ok and isinstance(selection, dict) and selection.get('exit_code') == 0
    report['status'] = 'pass' if ok else 'fail'
    destination = args.report or workdir / 'result.json'
    destination.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if ok else 1


if __name__ == '__main__':
    raise SystemExit(main())
