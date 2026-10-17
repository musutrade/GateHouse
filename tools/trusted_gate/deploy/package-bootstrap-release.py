#!/usr/bin/python3 -I
"""Export exact bootstrap source inputs; never execute or promote an operator Core."""
import argparse
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def package(root, output):
    if os.geteuid() == 0:
        raise ValueError('bootstrap release packaging requires an unprivileged operator')
    root, output = Path(root), Path(output)
    B = load('bootstrap_release_entry', root / 'tools/trusted_gate/deploy/import-protected-inputs.py')
    R = load('bootstrap_release_contract', root / 'tools/trusted_gate/deploy/bootstrap-release.py')
    def git(*args):
        return subprocess.check_output(['/usr/bin/git', '--no-replace-objects', '-c', 'core.hooksPath=/dev/null',
            '-c', 'core.fsmonitor=false', '-C', str(root), *args], env=B.ENV, stderr=subprocess.PIPE)
    if git('status', '--porcelain'):
        raise ValueError('commit reviewed source before packaging')
    head = git('rev-parse', 'HEAD').decode().strip()
    if git('rev-parse', head + ':tools/harness-gate').decode().strip() != R.CORE_TREE:
        raise ValueError('Core crate tree lacks independent review')
    git('merge-base', '--is-ancestor', R.BASE, head)
    output = B.path_operand(output)
    if output.exists() or output.is_symlink():
        raise ValueError('retain existing package; fresh output required')
    for parent in output.parents:
        if parent.is_symlink():
            raise ValueError('linked package parent')
    output.mkdir(mode=0o700)
    payload = {}
    def committed(revision, name):
        raw = git('show', revision + ':' + name)
        if len(raw) > R.MAX_MEMBER:
            raise ValueError('oversized source input')
        return raw
    for name in sorted(B.FILES):
        payload['helpers/' + name] = committed(head, name)
    code = {'schema': 'gatehouse-bootstrap-code/v1', 'revision': head,
            'files': {name: hashlib.sha256(payload['helpers/' + name]).hexdigest() for name in sorted(B.FILES)}}
    manifest_raw = B.canonical(code)
    payload['helper-manifest.json'] = manifest_raw
    for name in sorted(R.source_map()):
        payload['files/' + name] = committed(head, name)
    for name in sorted(R.old_sources()):
        payload['before/' + name] = committed(R.OLD_HEAD, name)
    bundle = output / 'source.bundle'
    git('bundle', 'create', str(bundle), 'HEAD', '^' + R.BASE)
    if bundle.stat().st_size > R.MAX_BUNDLE or git('bundle', 'list-heads', str(bundle)) != (head + ' HEAD\n').encode():
        raise ValueError('source bundle differs from exact reviewed head/budget')
    payload['source.bundle'] = bundle.read_bytes()
    payload['metadata.json'] = B.canonical(R.metadata(head))
    payload['payload-manifest.json'] = B.canonical({'schema': 'gatehouse-bootstrap-payload/v1',
        'files': R.rows(payload), 'installed_plan': R.plan(payload), 'source_binding_required': True})
    R.validate(B, payload, code, manifest_raw)
    archive = output / 'bootstrap-source.tar'
    with tarfile.open(archive, 'x:', format=tarfile.USTAR_FORMAT) as target:
        for name, raw in sorted(payload.items()):
            member = tarfile.TarInfo(name)
            member.size = len(raw)
            member.mode = 0o400
            target.addfile(member, io.BytesIO(raw))
    archive.chmod(0o600)
    with archive.open('rb') as source:
        archive_pin = hashlib.file_digest(source, 'sha256').hexdigest()
    with archive.open('rb') as source:
        R.decode(B, source, archive.stat().st_size, archive_pin, code, manifest_raw)
    if git('rev-parse', 'HEAD').decode().strip() != head or git('status', '--porcelain'):
        raise ValueError('source changed while packaging; retain partial output')
    B.write(output / 'helper-manifest.json', manifest_raw)
    result = {'head': head, 'archive': str(archive), 'bytes': archive.stat().st_size,
              'sha256': archive_pin, 'members': len(payload), 'helper_files': len(B.FILES),
              'helper_manifest_sha256': hashlib.sha256(manifest_raw).hexdigest(),
              'old_source_inputs': len(R.old_sources()), 'proposed_file_inputs': len(R.source_map()),
              'core_binary_included': False, 'source_binding_required': True, 'administrator_ready': False,
              'scope': 'source review inputs only; external archives/recipes and executable upgrade acceptance still required'}
    B.record(output / 'package.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(package(Path(__file__).resolve().parents[3], args.output), sort_keys=True, indent=2))


if __name__ == '__main__':
    main()
