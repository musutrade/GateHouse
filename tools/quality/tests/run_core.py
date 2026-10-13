#!/usr/bin/env python3
"""Run core and retained compatibility tests after verifying plugin handoffs.

Only the exact modules accepted in the pinned GateProbes run are delegated.
New test modules run locally by default. Retained plugin source changes require
a renewed handoff or restoration of the corresponding local tests.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
RECEIPT = 'docs/quality/ci-plugin-test-handoff.json'
TESTS = 'tools/quality/tests'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_receipt(root):
    def unique_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f'duplicate handoff field: {key}')
            result[key] = value
        return result
    return json.loads((root / RECEIPT).read_text(), object_pairs_hook=unique_pairs)


def tracked_files(root, directory):
    result = subprocess.run(['git', 'ls-files', '-z', '--', directory], cwd=root,
                            check=True, capture_output=True)
    return sorted(path.decode('utf-8') for path in result.stdout.split(b'\0') if path)


def select(root=ROOT):
    receipt = read_receipt(root)
    require(isinstance(receipt, dict)
            and receipt.get('schema') == 'core-plugin-test-handoff/v1',
            'invalid plugin test handoff schema')
    require(receipt.get('repository') == 'musutrade/GateProbes',
            'unexpected plugin test owner')
    require(re.fullmatch(r'[0-9a-f]{40}', str(receipt.get('revision', ''))),
            'plugin handoff requires an exact commit')
    require(re.fullmatch(r'https://github.com/musutrade/GateProbes/actions/runs/[0-9]+',
                         str(receipt.get('ci_run', ''))),
            'plugin handoff requires a recorded CI run')
    delegated = receipt.get('delegated_tests')
    sources = receipt.get('guarded_sources')
    require(isinstance(delegated, dict) and delegated,
            'plugin handoff has no delegated tests')
    require(isinstance(sources, dict) and sources,
            'plugin handoff has no guarded sources')
    for relative, expected in sources.items():
        path = Path(relative)
        require(not path.is_absolute() and '..' not in path.parts
                and relative.startswith(('tools/quality/', 'tools/harness-gate/presets/',
                                            'tools/harness-gate/fixtures/preset-compat/')),
                f'invalid guarded source: {relative}')
        require(re.fullmatch(r'[0-9a-f]{64}', str(expected)),
                f'invalid source digest: {relative}')
        source = root / path
        require(source.is_file() and not source.is_symlink(),
                f'missing guarded source: {relative}')
        require(hashlib.sha256(source.read_bytes()).hexdigest() == expected,
                f'plugin handoff is stale: {relative}; renew parity or restore local tests')
    step = receipt.get('source_risk_step')
    require(isinstance(step, dict)
            and step.get('guarded_root') == 'tools/quality/rust-source-risk',
            'source-risk handoff requires its guarded tree')
    expected_files = step.get('tracked_files')
    require(isinstance(expected_files, list) and expected_files
            and all(isinstance(path, str) and path in sources for path in expected_files)
            and len(set(expected_files)) == len(expected_files),
            'source-risk handoff requires an exact guarded file list')
    require(tracked_files(root, step['guarded_root']) == sorted(expected_files),
            'source-risk tracked files changed; renew parity or restore local tests')
    for name, row in delegated.items():
        require(re.fullmatch(r'test_[a-z0-9_]+', name) and isinstance(row, dict),
                f'invalid delegated test: {name}')
        relative = f'{TESTS}/{name}.py'
        require(relative in sources and row.get('sha256') == sources[relative],
                f'delegated test is not guarded: {name}')
        require(isinstance(row.get('path'), str) and row['path'].startswith('plugins/')
                and row['path'].endswith(f'/{name}.py') and row.get('command'),
                f'delegated test has no plugin execution evidence: {name}')
    modules = []
    tests = root / TESTS
    for path in sorted(tests.rglob('test_*.py')):
        relative = path.relative_to(tests)
        for directory in relative.parents:
            if directory != Path('.'):
                require((tests / directory / '__init__.py').is_file(),
                        f'test subdirectory needs __init__.py: {directory}')
        modules.append('.'.join(relative.with_suffix('').parts))
    require(set(delegated) <= set(modules), 'delegated test module is missing')
    selected = [name for name in modules if name not in delegated]
    require(selected, 'core test selection is empty')
    return selected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--list', action='store_true', help='validate handoff and print selected modules')
    args = parser.parse_args()
    try:
        selected = select()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'core test selection failed: {error}', file=sys.stderr)
        return 1
    if args.list:
        print('\n'.join(selected))
        return 0
    print(f'Running {len(selected)} core/retained modules; plugin handoff verified', flush=True)
    sys.path.insert(0, str(ROOT / TESTS))
    suite = unittest.defaultTestLoader.loadTestsFromNames(selected)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    raise SystemExit(main())
