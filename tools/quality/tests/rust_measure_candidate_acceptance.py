#!/usr/bin/env python3
"""Explicit Core consumer acceptance for the pinned 0.4.0 measurer.

Not discovered by the default unittest suite. Run with the released 0.4.0
binary and the historical 0.3.1 binary to prove the production consumer
accepts only the new identity:
  python3 tools/quality/tests/rust_measure_candidate_acceptance.py \
    --binary PATH --legacy-binary PATH
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import stat
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/quality"))
import source_measure

EXPECTED_SYMBOLS = ['included', 'nested', 'plain']
EXPECTED_EXCLUDED = [[3, 1, 4, 17]]


def write_fake(path: Path, payload: dict) -> Path:
    path.write_text("#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' " + json.dumps(json.dumps(payload)))
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


def check_consumer(result: dict, configuration: dict) -> None:
    names = [row['name'] for row in result['symbols']]
    if names != EXPECTED_SYMBOLS:
        raise AssertionError(f'unexpected consumer symbols: {names}')
    if result['excluded'] != EXPECTED_EXCLUDED:
        raise AssertionError(f'unexpected excluded spans: {result["excluded"]}')
    if result['configuration'] != configuration:
        raise AssertionError('consumer target echo mismatch')


def expect_rejection(binary: Path, source: Path, configuration: dict, label: str) -> None:
    try:
        source_measure.ast(source, binary, configuration)
    except ValueError as error:
        if ('incompatible AST analyzer' not in str(error) and
                'AST target configuration mismatch' not in str(error)):
            raise
        return
    raise SystemExit(f'production consumer accepted {label}')


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--legacy-binary', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    legacy = args.legacy_binary.resolve()
    for label, path in (('released', binary), ('legacy', legacy)):
        if not path.is_file():
            raise SystemExit(f'{label} measurer missing: {path}')
    configuration = {'target': 'x86_64-unknown-linux-gnu',
                     'cfg': ['target_os="linux"', 'unix']}
    source = '''#[cfg(any(test, unix))]
fn included() { if true {} }
#[cfg(all(test, target_os = "linux"))]
fn excluded() {}
#[cfg(all(unix, not(test)))]
fn nested() { if true {} }
#[cfg_attr(test, derive(Debug))]
struct Config { value: u8 }
fn plain() {}
'''
    plain_source = 'fn plain() -> usize { 1 }\n'
    with tempfile.TemporaryDirectory(prefix='rust-measure-040-') as tmp:
        path = Path(tmp) / 'source.rs'
        path.write_text(source)
        plain_path = Path(tmp) / 'plain.rs'
        plain_path.write_text(plain_source)
        result = source_measure.ast(path, binary, configuration)
        check_consumer(result, configuration)
        if source_measure.SERIES['analyzer'] != 'harness-gate-rust-measure/0.4.0':
            raise SystemExit('production series identity mismatch')
        if source_measure.MEASURER_IDENTITY['version'] != '0.4.0':
            raise SystemExit('production measurer identity mismatch')
        expect_rejection(legacy, plain_path, configuration, 'the real 0.3.1 binary')

        fake = write_fake(Path(tmp) / 'fake', {
            'analyzer': 'harness-gate-rust-measure', 'version': '0.4.0',
            'rule': 'mccabe-rust-3', 'configuration': configuration,
            'symbols': result['symbols'], 'excluded': [[9, 9, 9, 9]]})
        try:
            check_consumer(source_measure.ast(path, fake, configuration), configuration)
        except AssertionError:
            mutated_span_rejected = True
        else:
            raise SystemExit('consumer accepted a mutated excluded span')

        for payload in (
            {'analyzer': 'harness-gate-rust-measure', 'version': '0.3.1',
             'rule': 'mccabe-rust-3', 'configuration': configuration, 'symbols': [], 'excluded': []},
            {'analyzer': 'harness-gate-rust-measure', 'version': '9.9.9',
             'rule': 'mccabe-rust-3', 'configuration': configuration, 'symbols': [], 'excluded': []},
            {'analyzer': 'harness-gate-rust-measure', 'version': '0.4.0',
             'rule': 'mccabe-rust-2', 'configuration': configuration, 'symbols': [], 'excluded': []},
            {'analyzer': 'harness-gate-rust-measure', 'version': '0.4.0',
             'rule': 'mccabe-rust-3', 'configuration': {'target': 'wrong', 'cfg': []}, 'symbols': [], 'excluded': []},
        ):
            fake = write_fake(Path(tmp) / ('fake-' + payload['version'] + '-' + payload['rule']), payload)
            expect_rejection(fake, path, configuration, f'invalid identity {payload}')
        print(json.dumps({'consumer': source_measure.MEASURER_IDENTITY,
                          'series': source_measure.SERIES,
                          'symbols': EXPECTED_SYMBOLS, 'excluded': EXPECTED_EXCLUDED,
                          'configuration': configuration,
                          'legacy_identity': {'analyzer': 'harness-gate-rust-measure',
                                              'version': '0.3.1', 'rule': 'mccabe-rust-3'},
                          'legacy_rejected': True,
                          'mutated_span_rejected': mutated_span_rejected,
                          'invalid_identities_rejected': 4}, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
