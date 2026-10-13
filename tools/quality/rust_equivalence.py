#!/usr/bin/env python3
"""Replay the pinned Rust acceptance window and its adversarial fixtures."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import tarfile
import unittest

import rust_reference as adapter
from quality_common import ROOT, sha256, write_json

WINDOW = ROOT / 'tools/quality/fixtures/rust-reference/equivalence-window.json'


def replay(output):
    adapter.evidence.require(not output.exists(), 'acceptance output must be fresh')
    output.mkdir(parents=True)
    report = {'schema': 'rust-equivalence-acceptance/v1', 'accepted': False,
              'authoritative': 'rust', 'window_sha256': sha256(WINDOW), 'runs': [],
              'failures': [], 'evaluator_sha256': {
                  str(p.relative_to(ROOT)): sha256(p) for p in
                  sorted((ROOT / 'tools/quality').glob('*.py'))}}
    try:
        window = adapter.load_native(WINDOW)
        adapter.evidence.require(window['schema'] == 'rust-equivalence-window/v1', 'unknown window')
        runs = window['runs']
        adapter.evidence.require(len(runs) == window['required_runs'] == 2 and
                                 len({r['identity']['commit'] for r in runs}) ==
                                 window['required_distinct_heads'] == 2 and
                                 len({r['identity']['run'] for r in runs}) == 2,
                                 'incomplete or duplicate acceptance window')
        for index, run in enumerate(runs):
            archive = ROOT / run['archive']
            adapter.evidence.require(archive.resolve().is_relative_to(ROOT), 'archive escapes repository')
            adapter.evidence.require(sha256(archive) == run['sha256'], 'changed window archive')
            candidate_root = output / str(index) / 'candidate'
            with tarfile.open(archive) as bundle:
                bundle.extractall(candidate_root, filter='data')
            candidate = candidate_root / 'candidate.json'
            adapter.evidence.require(sha256(candidate) == run['candidate_sha256'], 'changed window candidate')
            identity = run['identity']
            shadow = adapter.shadow(candidate, output / str(index) / 'shadow',
                                    head=identity['commit'], base=identity['base_commit'],
                                    run_id=identity['run'])
            report['runs'].append({**run, 'shadow': f'{index}/shadow/shadow.json',
                                   'state': shadow['state'], 'compatible': shadow['compatible'],
                                   'current_state': shadow.get('current_state'),
                                   'generic_state': shadow.get('generic_state'),
                                   'compatibility_failures': shadow['compatibility_failures']})
            if (not shadow['compatible'] or shadow['migration_blocked'] or
                    shadow['state'] != 'compatible' or shadow['compatibility_failures'] or
                    shadow.get('current_state') != 'pass' or shadow.get('generic_state') != 'pass'):
                report['failures'].append('window replay mismatch: ' + identity['run'])
        # This suite contains raw corruption, stale identity, failed measurement
        # stages, debt/outcome disagreement and optional capability drift cases.
        with (output / 'fixtures.log').open('w') as log:
            suite = unittest.defaultTestLoader.discover(
                str(ROOT / 'tools/quality/tests'), pattern='test_rust_reference.py')
            result = unittest.TextTestRunner(stream=log, verbosity=2).run(suite)
        report['fixtures'] = {'run': result.testsRun, 'failures': len(result.failures),
                              'errors': len(result.errors), 'skipped': len(result.skipped),
                              'log_sha256': sha256(output / 'fixtures.log')}
        if not result.wasSuccessful() or result.skipped or result.testsRun < 15:
            report['failures'].append('adversarial fixture suite incomplete or failed')
        report['accepted'] = not report['failures']
    except (ValueError, KeyError, TypeError, OSError, tarfile.TarError) as error:
        report['failures'].append(str(error))
    write_json(output / 'acceptance.json', report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        report = replay(args.output)
        print(json.dumps({k: report[k] for k in ('accepted', 'failures')}))
        return int(not report['accepted'])
    except (ValueError, OSError) as error:
        print(f'Rust equivalence acceptance failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
