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
workflow: it consumes the full repository archive, so it cannot run from a
checkout that deliberately lacks plugin sources.
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

# Tests that exercise plugin implementations. They are owned by GateProbes
# under the split and are excluded from the core selection with that reason;
# they are not core tests with a missing input.
PLUGIN_OWNED_TESTS = (
    'test_native_artifact_paths',
    'test_preset_packs',
    'test_rust_collector_contract',
    'test_rust_collector_delivery',
    'test_rust_collector_project',
    'test_rust_collector_runtime',
    'test_rust_native',
    'test_rust_native_classify',
    'test_rust_native_driver',
    'test_typescript_acceptance',
    'test_typescript_advisory',
    'test_typescript_contracts',
    'test_typescript_policy',
    'test_typescript_reference',
    'test_typescript_semantics',
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


def exclude_plugin_implementation(copy: Path) -> list[str]:
    removed = []
    for relative in PLUGIN_IMPLEMENTATION:
        path = copy / relative
        require(path.exists(), f'plugin implementation path is missing: {relative}')
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
    tests = copy / 'tools' / 'quality' / 'tests'
    available = sorted(path.stem for path in tests.glob('test_*.py'))
    selected = [name for name in available if name not in PLUGIN_OWNED_TESTS]
    missing = [name for name in selected if not (tests / f'{name}.py').is_file()]
    require(not missing, 'selected core tests are missing: ' + ', '.join(missing))
    environment = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}
    result = run([sys.executable, '-m', 'unittest', *selected],
                 tests, log=workdir / 'logs' / 'python-selection.log', env=environment)
    output = result.stdout + result.stderr
    summary = UNITTEST_SUMMARY.search(output)
    outcome = UNITTEST_RESULT.search(output)
    counts = dict(item.split('=') for item in (outcome.group(2) or '').split(', ') if '=' in item) if outcome else {}
    report = {
        'selected_modules': selected,
        'excluded_plugin_owned': [name for name in available if name in PLUGIN_OWNED_TESTS],
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
    report['excluded_plugin_implementation'] = exclude_plugin_implementation(copy)
    report['local_git_head'] = init_local_git(copy)
    check_required_inputs(copy)

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
