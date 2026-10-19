#!/usr/bin/env python3
"""Fetch reviewed external compatibility artifacts; never used by Core runtime."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
import urllib.request

LIMIT = 16 * 1024 * 1024


def unpack(data, expected, destination):
    if hashlib.sha256(data).hexdigest() != expected['sha256']:
        raise ValueError('external artifact digest mismatch')
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        members = archive.getmembers()
        seen = set()
        total = 0
        for member in members:
            path = PurePosixPath(member.name)
            raw_parts = member.name.rstrip('/').split('/')
            if (any(part in ('', '.', '..') for part in raw_parts)
                    or path.is_absolute() or '..' in path.parts or not path.parts
                    or path.parts[0] != expected['prefix']
                    or member.name in seen or not (member.isfile() or member.isdir())):
                raise ValueError('unsafe or duplicate external archive member')
            seen.add(member.name)
            total += member.size
            if total > LIMIT or len(seen) > 100:
                raise ValueError('external archive exceeds compatibility limits')
        archive.extractall(destination, filter='data')
    return destination / expected['prefix']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    config = json.loads(Path(__file__).with_name('artifacts.json').read_text())
    if config['schema'] != 'gatehouse-external-compatibility/v1' or config['repository'] != 'musutrade/GateProbes':
        raise ValueError('unexpected external compatibility registry')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    receipt = {}
    for name, row in config['artifacts'].items():
        for key in ('tag', 'archive', 'prefix'):
            if not re.fullmatch(r'[a-zA-Z0-9._-]+', row[key]):
                raise ValueError('unsafe external artifact identity')
        if not re.fullmatch(r'[0-9a-f]{64}', row['sha256']):
            raise ValueError('invalid artifact digest')
        url = f"https://github.com/{config['repository']}/releases/download/{row['tag']}/{row['archive']}"
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read(LIMIT + 1)
        if len(data) > LIMIT:
            raise ValueError('external artifact exceeds download limit')
        installed = unpack(data, row, output)
        receipt[name] = {**row, 'path': str(installed)}
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')


if __name__ == '__main__':
    main()
