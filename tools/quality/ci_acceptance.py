#!/usr/bin/env python3
"""Validate and summarize Rust-generated CI contract receipts; never measure or evaluate policy."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import sys

ROOT = Path(__file__).resolve().parents[2]
MATRIX = ROOT / 'tools/harness-gate/fixtures/workflow/ci-matrix.json'
IDENTITY = ('GITHUB_SHA', 'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'RUNNER_OS', 'RUNNER_ARCH')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def validate(receipt, matrix_bytes, identity):
    """Check receipt completeness/integrity, leaving all quality decisions to Rust."""
    matrix = json.loads(matrix_bytes)
    require(receipt['schema'] == 'quality-ci-acceptance/v1', 'receipt schema mismatch')
    require(receipt['matrix_sha256'] == digest(matrix_bytes), 'matrix digest mismatch')
    require(receipt['identity'] == identity, 'receipt run/platform identity mismatch')
    expected = {(shape['id'], mode) for shape in matrix['shapes'] for mode in matrix['modes']}
    seen = set()
    summary = []
    for case in receipt['cases']:
        key = case['shape'], case['mode']
        require(key in expected and key not in seen, f'unexpected/duplicate case: {key}')
        seen.add(key)
        require(case['profile'] == matrix['profile'], f'profile mismatch: {key}')
        require(case['status'] == 'pass' and case['direct_parity'] is True, f'failed acceptance: {key}')
        require(case['producer_launches'] == 1 and case['reuse_launches'] == 0,
                f'duplicate or missing producer: {key}')
        seconds = case['seconds']
        require(isinstance(seconds, (int, float)) and math.isfinite(seconds) and seconds > 0,
                f'invalid elapsed time: {key}')
        decoded = {}
        for name, item in case['files'].items():
            path = PurePosixPath(name)
            require(not path.is_absolute() and '..' not in path.parts and '\\' not in name,
                    f'unsafe evidence path: {name}')
            data = base64.b64decode(item['base64'], validate=True)
            require(digest(data) == item['sha256'], f'evidence digest mismatch: {name}')
            decoded[name] = data
        direct = json.loads(decoded['direct-report.json'])
        quality = case['report']['quality']
        require(direct == quality['project_report'], f'direct/verify report mismatch: {key}')
        require(quality['producers'] and set(quality['producers'].values()) == {'retained'},
                f'non-retained producer: {key}')
        state = json.loads(decoded['.harness-gate/workflow-state.json'])
        for pin in state['retained'].values():
            require(digest(decoded[pin['path']]) == pin['sha256'], f'retained digest mismatch: {key}')
        for path, pin in state['artifacts'].items():
            require(digest(decoded[f"{state['artifact_root']}/{path}"]) == pin,
                    f'artifact inventory mismatch: {key}/{path}')
        required_negatives = set(matrix['negative_cases']) if case['mode'] == 'pass' else set()
        require(set(case['negatives']) == required_negatives, f'missing negative acceptance: {key}')
        for name, report in case['negatives'].items():
            require(report['status'] == 'FAIL' and report['quality']['status'] == 'blocked'
                    and report['quality']['error'], f'negative did not fail closed: {key}/{name}')
        summary.append({field: case[field] for field in
                        ('shape', 'mode', 'profile', 'seconds', 'producer_launches', 'reuse_launches')})
    require(seen == expected, 'incomplete acceptance matrix')
    require(math.isfinite(receipt['seconds']) and receipt['seconds'] > 0, 'invalid receipt time')
    return {'schema': 'quality-ci-acceptance-summary/v1', 'status': 'pass',
            'identity': identity, 'matrix_sha256': receipt['matrix_sha256'],
            'acceptance_test_elapsed_seconds': receipt['seconds'], 'cases': summary,
            'timing_scope': 'One test within the existing Test job; not total job duration or billed runner work.'}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    args = parser.parse_args(argv)
    output = args.directory / 'summary.json'
    output.unlink(missing_ok=True)
    try:
        receipt_bytes = (args.directory / 'receipt.json').read_bytes()
        summary = validate(json.loads(receipt_bytes), MATRIX.read_bytes(),
                           {key: os.environ.get(key, '') for key in IDENTITY})
        summary['receipt_sha256'] = digest(receipt_bytes)
        output.write_text(json.dumps(summary, indent=2) + '\n')
        print(f"CI workflow acceptance: {len(summary['cases'])} cases; "
              f"{summary['acceptance_test_elapsed_seconds']:.3f} seconds; receipts: {args.directory}")
        return 0
    except (ValueError, KeyError, TypeError, OSError) as error:
        print(f'CI workflow acceptance failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
