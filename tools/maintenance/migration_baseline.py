#!/usr/bin/env python3
"""Bind the one GateHouse seed transition to the accepted legacy source revision."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
SEED = '691447fbe5af64d01995d9e15c35781df4945e0d'
SOURCE_REPOSITORY = 'https://github.com/musutrade/Harness-Gate.git'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def resolve(base, head, repository, fetch=False, root=ROOT):
    for label, value in [('base', base), ('head', head)]:
        require(re.fullmatch('[0-9a-f]{40}', value or '') and set(value) != {'0'}, f'invalid {label} SHA')
    if base != SEED:
        return base
    require(repository == 'musutrade/GateHouse', 'seed bridge is restricted to GateHouse')
    raw = subprocess.check_output(['git', 'show', f'{head}:.github/repository-migration.json'], cwd=root)
    manifest = json.loads(raw)
    require(manifest.get('schema') == 'repository-migration/v1' and manifest.get('seed_commit') == SEED
            and manifest.get('source_repository') == 'musutrade/Harness-Gate'
            and manifest.get('destination_repository') == repository, 'invalid migration binding')
    source = manifest.get('source_commit', '')
    require(re.fullmatch('[0-9a-f]{40}', source) and set(source) != {'0'}, 'invalid migration source')
    if fetch:
        subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', SOURCE_REPOSITORY, source],
                       cwd=root, check=True, stdout=sys.stderr, stderr=sys.stderr)
        require(subprocess.check_output(['git', 'rev-parse', 'FETCH_HEAD'], cwd=root, text=True).strip() == source,
                'fetched source does not match migration manifest')
        # The bootstrap bridge may change storage and repository plumbing only.
        # It cannot admit changed production code against an invented baseline.
        subprocess.run(['git', 'diff', '--exit-code', source, head, '--',
                        'tools/harness-gate/src', 'tools/harness-gate/quality-core',
                        'tools/harness-gate/quality-replay', 'schema'], cwd=root, check=True,
                       stdout=sys.stderr, stderr=sys.stderr)
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--repository', default=os.environ.get('GITHUB_REPOSITORY', ''))
    parser.add_argument('--fetch', action='store_true')
    args = parser.parse_args()
    try:
        print(resolve(args.base, args.head, args.repository, args.fetch))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'migration baseline failed: {error}\n')


if __name__ == '__main__':
    main()
