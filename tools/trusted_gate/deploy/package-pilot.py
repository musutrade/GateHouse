#!/usr/bin/python3 -I
"""Prepare a local reviewable release; never install, sign or change GitHub state."""
import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--core-build', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    def git(*argv):
        return subprocess.check_output(['/usr/bin/git', *argv], cwd=root).decode().strip()
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('commit the reviewed tracked source before packaging')
    revision = git('rev-parse', 'HEAD')
    build = json.loads(args.core_build.read_text())
    core = Path(build['binary'])
    if (build['profile'] != 'release' or build['release_incremental'] is not False
            or git('rev-parse', revision + ':tools/harness-gate') != build['source_crate_tree']
            or hashlib.sha256(core.read_bytes()).hexdigest() != build['sha256']):
        raise ValueError('Core build does not match the reviewed source/release settings')
    args.output.mkdir(mode=0o700, parents=False, exist_ok=False)
    spec = importlib.util.spec_from_file_location('pilot_installer', Path(__file__).with_name('install-pilot.py'))
    installer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(installer)
    payload = {}
    def committed(path):
        return subprocess.check_output(['/usr/bin/git', 'show', revision + ':' + path], cwd=root)
    for name in installer.MODULES:
        payload['trusted_gate/' + name + '.py'] = committed('tools/trusted_gate/' + name + '.py')
    for destination, source in {
        'bin/gatehouse-control': 'tools/trusted_gate/deploy/gatehouse-control',
        'bin/pilot-probe.py': 'tools/trusted_gate/deploy/pilot-probe.py',
        'policy/flow.toml': 'tools/trusted_gate/deploy/pilot.flow.toml',
        'policy/gatehouse-step': 'tools/trusted_gate/deploy/gatehouse-step',
        'policy/audit.toml': 'tools/harness-gate/presets/empty.audit.toml',
        'policy/secrets.toml': 'tools/harness-gate/presets/default.secrets.toml',
        'srv-gatehouse-pilot.mount': 'tools/trusted_gate/deploy/srv-gatehouse-pilot.mount',
        'sudoers': 'tools/trusted_gate/deploy/sudoers.example',
    }.items():
        payload[destination] = committed(source)
    payload['bin/harness-gate'] = core.read_bytes()
    bundle = args.output / 'source.bundle'
    base = git('rev-parse', 'origin/main')
    subprocess.run(['/usr/bin/git', 'merge-base', '--is-ancestor', base, revision], cwd=root, check=True)
    subprocess.run(['/usr/bin/git', 'bundle', 'create', str(bundle), 'HEAD', '^' + base], cwd=root, check=True)
    if git('bundle', 'list-heads', str(bundle)).split()[0] != revision:
        raise ValueError('source revision changed during packaging')
    payload['source.bundle'] = bundle.read_bytes()
    payload['metadata.json'] = json.dumps({'repository': 'musutrade/GateHouse', 'head': revision,
        'base_at_packaging': base, 'coverage_acceptance': 'transition', 'core_profile': 'release',
        'release_incremental': False, 'core_build': build}, sort_keys=True, indent=2).encode() + b'\n'
    archive_path = args.output / 'release.tar'
    with tarfile.open(archive_path, 'x:') as archive:
        for name, data in sorted(payload.items()):
            member = tarfile.TarInfo(name)
            member.size = len(data)
            member.mode = 0o644
            archive.addfile(member, io.BytesIO(data))
    archive_path.chmod(0o600)
    digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    installer.release(archive_path.read_bytes(), digest)
    install_path = args.output / 'install-pilot.py'
    install_path.write_bytes(committed('tools/trusted_gate/deploy/install-pilot.py'))
    install_path.chmod(0o600)
    bundle.unlink()  # The bounded immutable archive already contains this data.
    result = {'head': revision, 'archive': str(archive_path), 'sha256': digest,
              'installer': str(install_path), 'installer_sha256': hashlib.sha256(install_path.read_bytes()).hexdigest(),
              'scope': 'reviewable isolation pilot only; administrator execution required'}
    (args.output / 'release.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
