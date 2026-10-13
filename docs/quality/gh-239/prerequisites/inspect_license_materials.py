"""Inspect retained bytes without inferring license applicability or approval."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import tarfile
import tomllib


def sha(data):
    return hashlib.sha256(data).hexdigest()


def members(archive):
    result = {}
    for member in archive.getmembers():
        path = PurePosixPath(member.name)
        if path.is_absolute() or '..' in path.parts or member.name in result:
            raise ValueError('unsafe or duplicate member: ' + member.name)
        if not (member.isfile() or member.isdir()):
            raise ValueError('unsupported member: ' + member.name)
        result[member.name] = member
    return result


def inspect(path, expected, metadata=()):
    rows, crates = [], []
    with tarfile.open(path) as archive:
        entries = members(archive)
        files = {name for name, member in entries.items() if member.isfile()}
        if files != set(expected) | set(metadata):
            raise ValueError('inventory mismatch: ' + str(path))
        for name in sorted(files):
            member = entries[name]
            with archive.extractfile(member) as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            if name in expected and digest != expected[name]['sha256']:
                raise ValueError('digest mismatch: ' + name)
            rows.append(dict(path=name, sha256=digest, size=member.size,
                             redistribution='pending'))
            if not name.startswith('licenses/crates/') or not name.endswith('.crate'):
                continue
            data = archive.extractfile(member).read()
            with tarfile.open(fileobj=io.BytesIO(data)) as crate:
                inner = members(crate)
                root = PurePosixPath(name).name.removesuffix('.crate')
                manifest = crate.extractfile(inner[root + '/Cargo.toml']).read()
                package = tomllib.loads(manifest.decode())['package']
                texts = []
                for nested, item in sorted(inner.items()):
                    basename = PurePosixPath(nested).name.lower()
                    if item.isfile() and basename.startswith(('license', 'copying', 'copyright', 'notice')):
                        text = crate.extractfile(item).read()
                        texts.append(dict(path=nested, sha256=sha(text), size=len(text)))
                crates.append(dict(path=name, sha256=digest, package=package['name'],
                                  version=package['version'],
                                  declared_license=package.get('license'),
                                  declared_license_file=package.get('license-file'),
                                  cargo_toml_sha256=sha(manifest), notice_candidates=texts,
                                  registry_checksum_verification='pending',
                                  applicability_review='pending', redistribution='pending'))
    with path.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return dict(archive_sha256=digest, archive_size=path.stat().st_size,
                payload_count=len(rows), payloads=rows, crates=crates)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('retained_root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    candidate = args.retained_root / 'candidate'
    raw = (candidate / 'manifest.json').read_bytes()
    manifest = json.loads(raw)
    expected = {row['path']: row for row in manifest['payloads']}
    if len(expected) != len(manifest['payloads']):
        raise ValueError('duplicate manifest payload')
    bootstrap = args.retained_root / 'installer-bootstrap.tar'
    with tarfile.open(bootstrap) as archive:
        entries = members(archive)
        raw_inputs = archive.extractfile(entries['bootstrap-inputs.json']).read()
        inputs = json.loads(raw_inputs)
    result = dict(schema='rust-collector-license-materials-inspection/v1',
                  scope='historical GH-231 bytes; not final RC approval',
                  status='pending', approvals=0, reviewer=None,
                  source_commit=manifest['source_commit'], manifest_sha256=sha(raw),
                  bootstrap_inputs_sha256=sha(raw_inputs),
                  collector=inspect(candidate / 'collector.tar', expected),
                  bootstrap=inspect(bootstrap, inputs, ('bootstrap-inputs.json',)))
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: dict(payloads=result[k]['payload_count'], crates=len(result[k]['crates']))
                      for k in ('collector', 'bootstrap')}))


if __name__ == '__main__':
    main()
