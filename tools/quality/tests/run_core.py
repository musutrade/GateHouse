#!/usr/bin/env python3
"""Run core and retained compatibility tests after verifying plugin handoffs.

Only the exact modules accepted in the pinned GateProbes run are delegated.
New test modules run locally by default. Transferred sources may be absent. Any remaining copy must match the accepted
input; Core consumers and schemas are mandatory. Execution evidence binds the
exact hosted run and complete method inventory, including explicit product holds.
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


def checked_path(relative, prefixes):
    path = Path(relative)
    require(not path.is_absolute() and '..' not in path.parts
            and relative.startswith(prefixes), f'invalid guarded source: {relative}')
    return path


def select(root=ROOT):
    receipt = read_receipt(root)
    require(isinstance(receipt, dict) and receipt.get('schema') == 'core-plugin-test-handoff/v2',
            'invalid plugin test handoff schema')
    require(receipt.get('repository') == 'musutrade/GateProbes', 'unexpected plugin test owner')
    require(re.fullmatch(r'[0-9a-f]{40}', str(receipt.get('revision', ''))),
            'plugin handoff requires an exact commit')
    require(re.fullmatch(r'https://github.com/musutrade/GateProbes/actions/runs/[0-9]+',
                         str(receipt.get('ci_run', ''))), 'plugin handoff requires a recorded CI run')
    delegated = receipt.get('delegated_tests')
    sources = receipt.get('guarded_sources')
    optional = receipt.get('transferred_sources')
    require(isinstance(delegated, dict) and delegated, 'plugin handoff has no delegated tests')
    require(isinstance(sources, dict) and sources, 'plugin handoff has no guarded sources')
    require(isinstance(optional, dict) and optional, 'plugin handoff has no transferred sources')
    require(not sources.keys() & optional.keys(), 'ambiguous source ownership')
    sys.path.insert(0, str(ROOT / 'tools/quality'))
    from core_isolation_check import PLUGIN_IMPLEMENTATION
    for relative, expected in {**sources, **optional}.items():
        path = checked_path(relative, ('tools/quality/', 'tools/harness-gate/presets/',
            'tools/harness-gate/fixtures/preset-compat/', 'tools/harness-gate/fixtures/', 'schema/', 'tools/maintenance/', 'test-data/'))
        require(re.fullmatch(r'[0-9a-f]{64}', str(expected)), f'invalid source digest: {relative}')
        source = root / path
        if relative in optional:
            require(relative in (f'{TESTS}/{name}.py' for name in delegated)
                    or any(relative == prefix or relative.startswith(prefix + '/') for prefix in PLUGIN_IMPLEMENTATION),
                    f'Core input cannot be optional: {relative}')
            if not source.exists() and not source.is_symlink():
                continue
        require(source.is_file() and not source.is_symlink(), f'missing guarded source: {relative}')
        require(hashlib.sha256(source.read_bytes()).hexdigest() == expected,
                f'plugin handoff is stale: {relative}; renew parity or restore local tests')
    trees = receipt.get('transferred_trees')
    require(isinstance(trees, dict) and trees, 'plugin handoff requires exact transferred trees')
    for directory, expected in trees.items():
        require(directory in PLUGIN_IMPLEMENTATION and isinstance(expected, list) and expected
                and len(set(expected)) == len(expected)
                and all(path in optional and path.startswith(directory + '/') for path in expected),
                'plugin handoff requires an exact guarded file list')
        require(set(tracked_files(root, directory)) <= set(expected),
                f'plugin tracked files changed: {directory}; renew parity or restore local tests')
    acceptance = receipt.get('acceptance')
    require(isinstance(acceptance, dict), 'missing compatibility acceptance')
    proof_path = checked_path(acceptance.get('file', ''), ('docs/quality/',))
    proof_bytes = (root / proof_path).read_bytes()
    require(hashlib.sha256(proof_bytes).hexdigest() == acceptance.get('sha256'),
            'compatibility acceptance digest mismatch')
    proof = json.loads(proof_bytes)
    require(isinstance(proof, dict), 'invalid compatibility acceptance')
    require(proof.get('schema') == 'legacy-core-compatibility-result/v1'
            and proof.get('status') == 'pass' and proof.get('producer_parity') == 'pass'
            and proof.get('revision') == receipt['revision'] and proof.get('ci_run') == receipt['ci_run']
            and all(proof.get(name) == 0 for name in ('failures', 'errors', 'skipped')),
            'missing, failed or mismatched compatibility execution evidence')
    inventory_path = checked_path(acceptance.get('inventory_file', ''), ('docs/quality/',))
    inventory_bytes = (root / inventory_path).read_bytes()
    require(hashlib.sha256(inventory_bytes).hexdigest() == proof.get('inventory_sha256'),
            'compatibility source inventory does not match executed evidence')
    inventory = json.loads(inventory_bytes)
    require(isinstance(inventory, dict) and inventory.get('schema') == 'legacy-core-compatibility/v1'
            and isinstance(inventory.get('files'), dict), 'invalid executed compatibility inventory')
    executed = inventory['files']
    # Updating a local digest alone cannot renew parity with the executed code.
    for relative, expected in {**sources, **optional}.items():
        if relative in executed and relative not in (
                'tools/quality/tests/test_typescript_policy.py', 'tools/quality/tests/test_preset_packs.py'):
            require(expected == executed[relative].get('sha256'),
                    f'source differs from executed compatibility input: {relative}; renew parity')
    fixture_path = 'tools/quality/fixtures/core-typescript-policy.json'
    if fixture_path in sources:
        require(proof.get('core_fixture_sha256') == sources[fixture_path],
                'Core policy fixture is not the proven producer output')
    methods = proof.get('methods')
    require(isinstance(methods, list) and methods and all(isinstance(row, dict) for row in methods),
            'compatibility acceptance has no method inventory')
    ids = [row.get('method') for row in methods]
    require(all(isinstance(name, str) for name in ids) and len(ids) == len(set(ids)),
            'duplicate or invalid compatibility methods')
    required = [row for row in methods if row.get('execution') == 'required']
    held = [row for row in methods if row.get('execution') == 'held']
    require(len(required) + len(held) == len(methods) and proof.get('tests_run') == len(required)
            and proof.get('held') == len(held), 'compatibility method count mismatch')
    require(all(row['method'].startswith('test_rust_collector_runtime.StandaloneNativeTests.')
                and row.get('reason') for row in held), 'unapproved held compatibility methods')
    for name, row in delegated.items():
        require(re.fullmatch(r'test_[a-z0-9_]+', name) and isinstance(row, dict),
                f'invalid delegated test: {name}')
        relative = f'{TESTS}/{name}.py'
        require(relative in optional and row.get('sha256') == optional[relative],
                f'delegated test is not guarded: {name}')
        require(isinstance(row.get('path'), str) and row['path'].startswith(('plugins/', 'compatibility/'))
                and row['path'].endswith(f'/{name}.py') and row.get('command'),
                f'delegated test has no plugin execution evidence: {name}')
        if row.get('acceptance') == 'legacy':
            require(any(item['method'].startswith(name + '.') and item.get('owner') == 'GateProbes' for item in methods),
                    f'delegated module missing from executed inventory: {name}')
        else:
            native = receipt.get('native_acceptance')
            require(row.get('acceptance') == 'native' and name in ('test_rust_collector_project',
                    'test_rust_native_driver', 'test_native_artifact_paths', 'test_rust_native_classify')
                    and isinstance(native, dict), f'delegated test has no accepted native execution: {name}')
            native_path = checked_path(native.get('file', ''), ('docs/quality/',))
            native_bytes = (root / native_path).read_bytes()
            require(hashlib.sha256(native_bytes).hexdigest() == native.get('sha256'), 'native acceptance digest mismatch')
            original = json.loads(native_bytes)
            require(original.get('schema') == 'core-plugin-test-handoff/v1'
                    and original.get('repository') == receipt['repository']
                    and original.get('revision') == native.get('revision')
                    and original.get('ci_run') == native.get('ci_run')
                    and original.get('delegated_tests', {}).get(name, {}).get('sha256') == row['sha256'],
                    f'native delegation differs from accepted parity: {name}')
            for relative, expected in original['guarded_sources'].items():
                require({**sources, **optional}.get(relative) == expected,
                        f'native/source-risk input differs from accepted parity: {relative}')
    modules = []
    tests = root / TESTS
    for path in sorted(tests.rglob('test_*.py')):
        relative = path.relative_to(tests)
        for directory in relative.parents:
            if directory != Path('.'):
                require((tests / directory / '__init__.py').is_file(),
                        f'test subdirectory needs __init__.py: {directory}')
        modules.append('.'.join(relative.with_suffix('').parts))
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
