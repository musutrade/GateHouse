"""Execute preserved delivery protections in a disposable historical layout.

The snapshot has no product entry in Core. Shared protocols and tools are linked
from their single editable Core source, never copied into the frozen inventory.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = 'retired/legacy-collector/manifest.json'


def safe_path(relative: str) -> Path:
    path = Path(relative)
    if path.is_absolute() or '..' in path.parts or not path.parts:
        raise ValueError(f'unsafe frozen path: {relative}')
    return path


def verify(root: Path = ROOT) -> dict:
    manifest = json.loads((root / MANIFEST).read_text())
    if manifest.get('schema') != 'frozen-legacy-delivery/v1':
        raise ValueError('invalid frozen delivery manifest')
    snapshot = root / safe_path(manifest['snapshot'])
    inventory = json.loads((root / 'docs/quality/issue-13/removal-inventory.json').read_text())
    actual = {p.relative_to(snapshot).as_posix() for p in snapshot.rglob('*') if p.is_file()}
    if actual != set(manifest['files']):
        raise ValueError('frozen delivery file inventory changed')
    modules = sorted(Path(p).stem for p in actual
                     if p.startswith('tools/release/tests/test_') and p.endswith('.py'))
    if modules != manifest['test_modules'] or not manifest['expected_methods']:
        raise ValueError('frozen delivery test inventory changed')
    for relative, row in manifest['files'].items():
        path = snapshot / safe_path(relative)
        removal = inventory['files'][relative]
        if (path.is_symlink() or not path.is_file()
                or hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']
                or row['sha256'] != removal['sha256']
                or removal['frozen_path'] != path.relative_to(root).as_posix()
                or (root / relative).exists()):
            raise ValueError(f'frozen delivery source changed or active copy restored: {relative}')
    for relative in manifest['shared_sources'] + manifest['shared_inputs']:
        path = root / safe_path(relative)
        if not path.exists() or path.is_symlink():
            raise ValueError(f'missing shared Core input: {relative}')
    return manifest


def run(root: Path = ROOT) -> subprocess.CompletedProcess[str]:
    manifest = verify(root)
    with tempfile.TemporaryDirectory(prefix='frozen-legacy-delivery-') as temporary:
        stage = Path(temporary) / 'src'
        shutil.copytree(root / manifest['snapshot'], stage)
        for relative in manifest['shared_sources'] + manifest['shared_inputs']:
            destination = stage / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.symlink_to(root / relative, target_is_directory=(root / relative).is_dir())
        # Import paths are process-local; frozen helpers never enter Core's tests.
        script = '''
import json, sys, unittest
from pathlib import Path
root = Path.cwd()
sys.path[:0] = [str(root / 'tools/release'), str(root / 'tools/quality')]
suite = unittest.defaultTestLoader.discover('tools/release/tests')
def ids(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from ids(item)
        else:
            yield item.id()
methods = list(ids(suite))
result = unittest.TextTestRunner(verbosity=2).run(suite)
print(json.dumps({'methods': methods, 'tests_run': result.testsRun,
                  'failures': len(result.failures), 'errors': len(result.errors),
                  'skipped': len(result.skipped)}))
sys.exit(0 if result.wasSuccessful() and not result.skipped and methods else 1)
'''
        result = subprocess.run([sys.executable, '-B', '-c', script], cwd=stage,
                                capture_output=True, text=True)
        report = json.loads(result.stdout.splitlines()[-1])
        if (sorted(report['methods']) != manifest['expected_methods']
                or report['tests_run'] != len(manifest['expected_methods'])):
            result.returncode = 1
            result.stderr += '\nFrozen delivery method inventory differs from the original executed suite.\n'
        return result


if __name__ == '__main__':
    result = run()
    print(result.stdout, end='')
    print(result.stderr, end='', file=sys.stderr)
    raise SystemExit(result.returncode)
