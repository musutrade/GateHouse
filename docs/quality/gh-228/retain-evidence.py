"""Retain original GH-228 evidence bytes; never recreate a capture or anchor."""
import hashlib
import json
from pathlib import Path
import tarfile

root = Path.cwd()
work = root / 'target/gh-228'
output = root / 'docs/quality/gh-228'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def retain(name, paths):
    destination = output / name
    if destination.exists():
        raise SystemExit('Refusing to overwrite retained evidence: ' + str(destination))
    paths = sorted(set(paths))
    files = {}
    with tarfile.open(destination, 'w:gz') as archive:
        for path in paths:
            relative = str(path.relative_to(root))
            archive.add(path, arcname=relative, recursive=False)
            files[relative] = ({'symlink': str(path.readlink())} if path.is_symlink()
                               else {'sha256': sha(path), 'size_bytes': path.stat().st_size})
    # Verify the archive actually contains every original byte, rather than
    # relying solely on the uncompressed source inventory.
    with tarfile.open(destination) as archive:
        for member in archive:
            expected = files[member.name]
            if member.issym():
                assert member.linkname == expected['symlink']
            else:
                with archive.extractfile(member) as stream:
                    assert hashlib.file_digest(stream, 'sha256').hexdigest() == expected['sha256']
    return {'path': str(destination.relative_to(root)), 'sha256': sha(destination),
            'size_bytes': destination.stat().st_size, 'files': files}


native_roots = [work / 'runtime-tests', work / 'standalone-probes']
captures = []
for directory in native_roots:
    for manifest in sorted(directory.rglob('manifest.json')):
        record = json.loads(manifest.read_text())
        if record.get('schema') != 'native-driver-artifacts/1':
            continue
        for name, digest in record['artifacts'].items():
            assert sha(manifest.parent / name) == digest, str(manifest) + ': ' + name
        captures.append({'manifest': str(manifest.relative_to(root)),
                         'anchor': sha(manifest), 'verified_artifacts': len(record['artifacts'])})
native_files = [p for directory in native_roots for p in directory.rglob('*') if p.is_file()]
native = retain('native-evidence.tar.gz', native_files)
log_roots = [work / 'logs', *sorted(work.glob('resumed-validation*'))]
logs = [p for directory in log_roots for p in directory.rglob('*') if p.is_file()]
logs += list(work.glob('*.json'))
logs += [work / 'operator-native-runtime/bootstrap.json', work / 'operator-native-runtime/native-tests.log']
validation = retain('resumed-validation-logs.tar.gz', logs)
index = {'schema': 'gh-228-retained-evidence/1', 'cwd': str(root),
         'command': 'python3 docs/quality/gh-228/retain-evidence.py',
         'captures': captures, 'archives': [native, validation],
         'limits': 'Private runtime tar files and rustc-dev archive remain workspace-local; see runtime.md.'}
(output / 'retained-evidence.json').write_text(json.dumps(index, indent=2, sort_keys=True) + '\n')
print(json.dumps({'captures': len(captures), 'archives': [
    {k: v for k, v in row.items() if k != 'files'} for row in index['archives']]}))
