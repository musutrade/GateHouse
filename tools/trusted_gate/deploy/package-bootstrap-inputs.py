#!/usr/bin/python3 -I
"""Export the exact committed helper closure for administrator review, never install."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess


def package(output):
    if os.geteuid() == 0:
        raise ValueError('bootstrap packaging requires an unprivileged operator')
    root = Path(__file__).resolve().parents[3]
    def git(*args):
        return subprocess.check_output(['/usr/bin/git', '-C', str(root), *args])
    if git('status', '--porcelain'):
        raise ValueError('commit the reviewed helper source before packaging')
    head = git('rev-parse', 'HEAD').decode().strip()
    spec = importlib.util.spec_from_file_location('bootstrap_inventory', Path(__file__).with_name('import-protected-inputs.py'))
    importer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(importer)
    output = Path(output)
    if not output.is_absolute() or '..' in output.parts or output.exists() or output.is_symlink():
        raise ValueError('fresh absolute bootstrap package output required')
    for path in (output.parent, *output.parent.parents):
        if path.is_symlink():
            raise ValueError('linked bootstrap package parent')
    output.mkdir(mode=0o700)
    helpers = output / 'helpers'
    helpers.mkdir(mode=0o700)
    pins, total = {}, 0
    for name in sorted(importer.FILES):
        raw = git('show', head + ':' + name)
        if len(raw) > 1024 * 1024:
            raise ValueError('oversized bootstrap helper')
        total += len(raw)
        pins[name] = hashlib.sha256(raw).hexdigest()
        path = helpers / name
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        importer.write(path, raw)
    raw = importer.canonical({'schema': 'gatehouse-bootstrap-code/v1', 'revision': head, 'files': pins})
    importer.write(output / 'manifest.json', raw)
    if git('rev-parse', 'HEAD').decode().strip() != head or git('status', '--porcelain'):
        raise ValueError('helper source changed during packaging; retain partial output')
    for parent, _, _ in os.walk(helpers):
        Path(parent).chmod(0o555)
    value = {'head': head, 'helper_files': len(pins), 'helper_bytes': total,
             'manifest_sha256': hashlib.sha256(raw).hexdigest(), 'administrator_ready': False,
             'scope': 'reviewable helper bytes only; no ownership approval, installation or protected import'}
    (output / 'package.json').write_text(json.dumps(value, sort_keys=True, indent=2) + '\n')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(package(args.output), sort_keys=True, indent=2))


if __name__ == '__main__':
    main()
