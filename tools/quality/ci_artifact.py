#!/usr/bin/env python3
"""Seal retained CI quality evidence and verify it before cross-job consumption.

The manifest digest travels through the producer job output, independently of
the downloaded artifact. Hashes are integrity checks, not signatures or a grant
of authority to an untrusted workflow. Existing quality policy still runs.
"""
import argparse
import json
import os
from pathlib import Path
import re
import sys

from ci_cargo import configuration_hash
from quality_common import command_output, git_sha, sha256, write_json

MANIFEST = 'ci-artifact.json'
EXCLUDED = {'build', 'snapshots'}


def identity():
    values = {key: os.environ[key] for key in (
        'GITHUB_REPOSITORY', 'GITHUB_WORKFLOW', 'GITHUB_SHA',
        'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'RUNNER_OS', 'RUNNER_ARCH')}
    if any(not value for value in values.values()) or values['GITHUB_SHA'] != git_sha():
        raise ValueError('artifact identity must match the current checkout and run')
    return {**values, 'producer': 'quality-coverage', 'kind': 'instrumented-quality-evidence',
            'configuration_sha256': configuration_hash()}


def inventory(root, *, producer=False):
    result = {}
    for path in sorted(root.rglob('*')):
        relative = path.relative_to(root)
        if relative.parts[0] in EXCLUDED and producer:
            continue
        if path.is_symlink():
            raise ValueError(f'symlink in artifact: {relative}')
        if relative.parts[0] in EXCLUDED:
            raise ValueError(f'mutable build state in artifact: {relative}')
        if path.is_file() and relative.as_posix() != MANIFEST:
            result[relative.as_posix()] = sha256(path)
    if 'candidate.json' not in result:
        raise ValueError('missing candidate.json')
    return result


def seal(root, expected):
    if os.environ['GITHUB_JOB'] != expected['producer']:
        raise ValueError('unexpected artifact producer')
    manifest = root / MANIFEST
    if manifest.exists():
        raise ValueError('artifact is already sealed')
    tools = {tool: command_output(command) for tool, command in {
        'rustc': ['rustc', '-vV'], 'cargo': ['cargo', '--version'],
        'nextest': ['cargo', 'nextest', '--version'],
        'llvm-cov': ['cargo', 'llvm-cov', '--version'],
    }.items()}
    if not all(tools.values()):
        raise ValueError('missing artifact tool identity')
    write_json(manifest, {'schema': 1, 'identity': expected, 'tools': tools,
                          'files': inventory(root, producer=True)})
    return sha256(manifest)


def verify(root, expected, digest):
    manifest = root / MANIFEST
    if not re.fullmatch('[0-9a-f]{64}', digest or ''):
        raise ValueError('missing or invalid producer manifest digest')
    if manifest.is_symlink() or sha256(manifest) != digest:
        raise ValueError('artifact manifest hash mismatch')
    report = json.loads(manifest.read_text())
    if report['schema'] != 1 or report['identity'] != expected:
        raise ValueError('artifact source/run/attempt/configuration identity mismatch')
    if (set(report['tools']) != {'rustc', 'cargo', 'nextest', 'llvm-cov'} or
            not all(isinstance(value, str) and value for value in report['tools'].values())):
        raise ValueError('missing artifact tool identity')
    if inventory(root) != report['files']:
        raise ValueError('artifact file inventory/hash mismatch')
    return {'verified': True, 'manifest_sha256': digest,
            'identity': expected, 'tools': report['tools'], 'files': len(report['files'])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('seal', 'verify'))
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--manifest-sha256')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    try:
        if args.operation == 'seal':
            digest = seal(args.root, identity())
            with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
                output.write(f'manifest-sha256={digest}\n')
            print(f'Sealed quality artifact: {digest}')
        else:
            result = verify(args.root, identity(), args.manifest_sha256)
            if args.output:
                write_json(args.output, result)
            print(json.dumps(result, indent=2))
    except (ValueError, KeyError, TypeError, OSError) as error:
        print(f'CI artifact rejected: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
