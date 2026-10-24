#!/usr/bin/env python3
"""Explicit Core consumer acceptance for the candidate 0.4.0 measurer.

Not discovered by the default unittest suite: production remains pinned to
0.3.1. Run with the real candidate binary:
  python3 tools/quality/tests/rust_measure_candidate_acceptance.py --binary PATH
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/quality"))
import source_measure
from source_measure_candidate import CANDIDATE_IDENTITY, CANDIDATE_SERIES, ast_040


def write_fake(path: Path, payload: dict) -> Path:
    path.write_text("#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' " + json.dumps(json.dumps(payload)))
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        raise SystemExit(f'candidate measurer missing: {binary}')
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
    with tempfile.TemporaryDirectory(prefix='rust-measure-040-') as tmp:
        path = Path(tmp) / 'source.rs'
        path.write_text(source)
        result = ast_040(path, binary, configuration)
        names = [row['name'] for row in result['symbols']]
        if names != ['included', 'nested', 'plain']:
            raise SystemExit(f'unexpected candidate symbols: {names}')
        if not result['excluded']:
            raise SystemExit('candidate did not report excluded spans')
        if result['configuration'] != configuration:
            raise SystemExit('candidate target echo mismatch')
        if CANDIDATE_SERIES['analyzer'] != 'harness-gate-rust-measure/0.4.0':
            raise SystemExit('candidate series identity mismatch')
        if source_measure.SERIES['analyzer'] != 'harness-gate-rust-measure/0.3.1':
            raise SystemExit('production series identity changed')
        try:
            source_measure.ast(path, binary, configuration)
        except ValueError as error:
            if 'incompatible AST analyzer' not in str(error):
                raise
        else:
            raise SystemExit('old 0.3.1 consumer accepted the 0.4.0 candidate')
        fake = write_fake(Path(tmp) / 'fake', {
            'analyzer': 'harness-gate-rust-measure', 'version': '0.3.1',
            'rule': 'mccabe-rust-3', 'configuration': configuration, 'symbols': [], 'excluded': []})
        for payload in (
            {'analyzer': 'harness-gate-rust-measure', 'version': '0.3.1',
             'rule': 'mccabe-rust-3', 'configuration': configuration, 'symbols': [], 'excluded': []},
            {'analyzer': 'harness-gate-rust-measure', 'version': '0.4.0',
             'rule': 'mccabe-rust-2', 'configuration': configuration, 'symbols': [], 'excluded': []},
            {'analyzer': 'harness-gate-rust-measure', 'version': '0.4.0',
             'rule': 'mccabe-rust-3', 'configuration': {'target': 'wrong', 'cfg': []}, 'symbols': [], 'excluded': []},
        ):
            fake = write_fake(Path(tmp) / ('fake-' + payload['version'] + '-' + payload['rule']), payload)
            try:
                ast_040(path, fake, configuration)
            except ValueError:
                continue
            raise SystemExit(f'candidate accepted invalid identity: {payload}')
        print(json.dumps({'candidate': CANDIDATE_IDENTITY, 'series': CANDIDATE_SERIES,
                          'symbols': names, 'excluded': result['excluded'],
                          'configuration': result['configuration'],
                          'old_consumer_rejected': True, 'invalid_identities_rejected': 3}, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
