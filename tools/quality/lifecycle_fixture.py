#!/usr/bin/env python3
"""Pinned source-only native fixture for fake-socket lifecycle coverage."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tomllib

COMMIT = '76d0e0c188d0ac79f497ad50b7ff7464d47cbf2d'
ARCHIVE_SHA256 = 'fc651690f3e8c3ce3968eb9dc3d9828ba89e562ed4537b1922e4a371d92872bf'
PREFIX = 'musutrade-GateProbes-76d0e0c/plugins/docker-lifecycle/native/'
FEATURE = 'harness-gate/lifecycle-joint-tests'
PODMAN_FIXTURES = {
    '0.4.0': {
        'commit': '56f99ac0930018e73c693d38ddc9901cb5158053',
        'archive_sha256': '00fda426c6446e54969d6a9b6f70de25b8e884fd708b7bb203c59fd046d1db4d',
        'prefix': 'musutrade-GateProbes-56f99ac/plugins/podman-lifecycle/native/',
        'source_protocol': '0.4.0',
        'protocol': '0.4.0',
    },
    '0.4.1': {
        'commit': '7afb658c9239c4e47d55012a8392c152f76bbf35',
        'archive_sha256': '99a573fe8cc5e9402415a01c7ef5231119bdd54941e388914e90fd43ac5b3d6c',
        'prefix': 'musutrade-GateProbes-7afb658/plugins/podman-lifecycle/native/',
        'source_protocol': '0.4.1',
        'protocol': '0.4.1',
    },
    # The reviewed 0.4.1 adapter source is also the deterministic 0.5 fixture;
    # only its exact protocol dependency is rewritten before the path patch.
    '0.5.0': {
        'commit': '7afb658c9239c4e47d55012a8392c152f76bbf35',
        'archive_sha256': '99a573fe8cc5e9402415a01c7ef5231119bdd54941e388914e90fd43ac5b3d6c',
        'prefix': 'musutrade-GateProbes-7afb658/plugins/podman-lifecycle/native/',
        'source_protocol': '0.4.1',
        'protocol': '0.5.0',
    },
    # Local pre-publication source for protocol 0.6.0. The commit is a local
    # product head, not a GitHub-downloadable commit. CI must not infer a remote
    # archive from it; supply --podman-archive from the reviewed local source or
    # replace this entry after the product branch is pushed and published.
    '0.6.0': {
        'commit': 'd69c59762bce6320f3d6e98ae1843af58d09e3b5',
        'archive_sha256': '3b39a39545706df5632d2951517baee214b1669afabca593ffb872ca3b8c0c50',
        'prefix': 'musutrade-GateProbes-d69c59762bce/plugins/podman-lifecycle/native/',
        'source_protocol': '0.6.0',
        'protocol': '0.6.0',
        'source': 'local-archive',
        'lock_sha256': 'e6e5ce69fc63e7a7767a02d204a6a959c8969167e94873153aa05261eb37356c',
    },
}
PODMAN_FEATURE = 'harness-gate/podman-capture-joint-tests'
LIMIT = 64 * 1024 * 1024


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def supported(crate):
    manifest = Path(crate) / 'Cargo.toml'
    return manifest.is_file() and 'lifecycle-joint-tests' in tomllib.loads(
        manifest.read_text()).get('features', {})


def podman_supported(crate):
    return 'podman-capture-joint-tests' in tomllib.loads(
        (Path(crate) / 'Cargo.toml').read_text()).get('features', {})


def podman_fixture(crate):
    manifest = Path(crate) / 'runtime-protocol/Cargo.toml'
    try:
        version = tomllib.loads(manifest.read_text())['package']['version']
    except (OSError, tomllib.TOMLDecodeError, KeyError, TypeError) as exc:
        raise ValueError(f'cannot read Podman fixture protocol version: {exc}') from exc
    if version not in PODMAN_FIXTURES:
        raise ValueError(f'unsupported Podman fixture protocol version: {version}')
    return PODMAN_FIXTURES[version]


def pin_podman_protocol(native, source_protocol, protocol):
    if source_protocol == protocol:
        return
    manifest = Path(native) / 'Cargo.toml'
    lock = Path(native) / 'Cargo.lock'
    expected = f'gatehouse-runtime-protocol = "={source_protocol}"'
    text = manifest.read_text()
    if text.count(expected) != 1:
        raise ValueError('unexpected Podman fixture protocol requirement')
    manifest.write_text(text.replace(expected, f'gatehouse-runtime-protocol = "={protocol}"'))
    old = f'name = "gatehouse-runtime-protocol"\nversion = "{source_protocol}"\n'
    lock_text = lock.read_text()
    if lock_text.count(old) != 1:
        raise ValueError('unexpected Podman fixture protocol lock')
    lock.write_text(lock_text.replace(old, f'name = "gatehouse-runtime-protocol"\nversion = "{protocol}"\n'))


def pin_podman_local_protocol(native, crate, env):
    """Prepare one reviewed path override, then let the build run --locked."""
    native, crate = Path(native), Path(crate)
    manifest = native / 'Cargo.toml'
    lock = native / 'Cargo.lock'
    protocol = crate / 'runtime-protocol'
    expected = 'gatehouse-runtime-protocol = "=0.6.0"'
    text = manifest.read_text()
    if text.count(expected) != 1:
        raise ValueError('unexpected local Podman fixture protocol requirement')
    replacement = ('gatehouse-runtime-protocol = { path = '
                   + json.dumps(str(protocol)) + ', version = "=0.6.0" }')
    manifest.write_text(text.replace(expected, replacement))
    before = tomllib.loads(lock.read_text())
    subprocess.run(['cargo', 'update', '--offline', '--manifest-path', str(manifest),
                    '-p', 'gatehouse-runtime-protocol'], env=env, cwd=crate,
                   check=True, capture_output=True)
    after = tomllib.loads(lock.read_text())
    intended = {'gatehouse-runtime-protocol', 'gateprobes-podman-lifecycle'}
    before_rows = [row for row in before.get('package', [])
                   if row.get('name') not in intended]
    after_rows = [row for row in after.get('package', [])
                  if row.get('name') not in intended]
    if before_rows != after_rows:
        raise ValueError('local Podman fixture lock updated unrelated dependencies')
    entries = [row for row in after.get('package', [])
               if row.get('name') == 'gatehouse-runtime-protocol']
    if len(entries) != 1 or entries[0].get('version') != '0.6.0':
        raise ValueError('local Podman fixture protocol lock mismatch')
    if 'source' in entries[0] or 'checksum' in entries[0]:
        raise ValueError('local Podman fixture protocol must not claim a registry pin')
    return digest(lock)


def features(crate):
    return FEATURE + (',' + PODMAN_FEATURE if podman_supported(crate) else '')


def source_digest(crate):
    """Bind both binaries to compilation inputs, excluding generated targets."""
    crate = Path(crate)
    paths = []
    for folder in ('src', 'runtime-protocol', 'quality-core', 'quality-replay', 'tests', '.cargo'):
        paths.extend(p for p in (crate / folder).rglob('*') if p.is_file()
                     and (p.suffix == '.rs' or p.name in ('Cargo.toml', 'Cargo.lock', 'config.toml')))
    paths.extend(crate / name for name in ('Cargo.toml', 'Cargo.lock', 'build.rs')
                 if (crate / name).is_file())
    rows = [(str(p.relative_to(crate)), digest(p)) for p in sorted(set(paths))]
    return hashlib.sha256(json.dumps(rows, separators=(',', ':')).encode()).hexdigest()


def extract(archive, destination, *, expected_sha=None, prefix=None):
    expected_sha = ARCHIVE_SHA256 if expected_sha is None else expected_sha
    prefix = PREFIX if prefix is None else prefix
    archive, destination = Path(archive), Path(destination)
    if archive.stat().st_size > LIMIT or digest(archive) != expected_sha:
        raise ValueError('lifecycle fixture archive digest/size mismatch')
    # Never execute or import anything from unverified archive contents.
    with tarfile.open(archive, 'r:gz') as stream:
        members = stream.getmembers()
        if len(members) > 10000 or sum(m.size for m in members) > LIMIT:
            raise ValueError('lifecycle fixture archive bounds')
        selected = []
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or '..' in path.parts or member.name in seen:
                raise ValueError('invalid lifecycle fixture member path')
            seen.add(member.name)
            if not (member.isdir() or member.isfile()):
                raise ValueError('unsupported lifecycle fixture member type')
            if member.name.startswith(prefix) and member.isfile():
                relative = PurePosixPath(member.name[len(prefix):])
                if not relative.parts or member.mode & 0o7000:
                    raise ValueError('invalid native fixture member')
                selected.append((member, relative))
        if not {'Cargo.toml', 'Cargo.lock', 'src/main.rs'} <= {str(p) for _, p in selected}:
            raise ValueError('incomplete native fixture')
        destination.mkdir(mode=0o700, parents=True, exist_ok=False)
        for member, relative in selected:
            path = destination / str(relative)
            path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            with path.open('xb') as output:
                output.write(stream.extractfile(member).read())
            path.chmod(0o600)


def pin_registry_protocol(native):
    """Keep the Docker v1 fixture on published 0.3, independent of Core's version."""
    manifest = tomllib.loads((native / 'Cargo.toml').read_text())
    if manifest['dependencies']['gatehouse-runtime-protocol'] != '=0.3.0':
        raise ValueError('unexpected fixture protocol requirement')
    path = native / 'Cargo.lock'
    lock = path.read_text()
    old = 'name = "gatehouse-runtime-protocol"\nversion = "0.3.0"\ndependencies = ['
    if lock.count(old) != 1:
        raise ValueError('unexpected fixture protocol lock')
    new = ('name = "gatehouse-runtime-protocol"\nversion = "0.3.0"\n'
           'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
           'checksum = "357cf18eb55a7a83d9b8840280e77c6f7efe263a74bdeafc7e132cad7a607e54"\ndependencies = [')
    path.write_text(lock.replace(old, new))


def build(crate, output, work, archive=None, podman_archive=None):
    crate, output, work = Path(crate).resolve(), Path(output).resolve(), Path(work).resolve()
    if not supported(crate):
        raise ValueError('Core snapshot has no lifecycle joint feature')
    if output.exists() or work.exists():
        raise ValueError('fixture build requires fresh output and work directories')
    before = source_digest(crate)
    work.mkdir(mode=0o700, parents=True)
    if archive is None:
        archive = work / 'source.tar.gz'
        subprocess.run(['curl', '--fail', '--location', '--proto', '=https',
                        '--proto-redir', '=https', '--max-time', '120', '--max-filesize', str(LIMIT), '--output', str(archive),
                        f'https://api.github.com/repos/musutrade/GateProbes/tarball/{COMMIT}'], check=True)
    native = work / 'native'
    extract(archive, native)
    pin_registry_protocol(native)
    env = os.environ.copy()
    for key in list(env):
        if key.startswith('CARGO_LLVM_COV') or key in (
                'RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'LLVM_PROFILE_FILE',
                'CARGO_BUILD_TARGET'):
            env.pop(key, None)
    binaries = {}
    builds = [
        ('adapter', native / 'Cargo.toml', [], 'gateprobes-docker-lifecycle'),
        ('core', crate / 'Cargo.toml', ['--package', 'harness-gate', '--bin', 'harness-gate'], 'harness-gate'),
    ]
    podman_pin = None
    if podman_supported(crate):
        podman_pin = podman_fixture(crate)
        if podman_archive is None and podman_pin.get('source') == 'local-archive':
            raise ValueError(
                'protocol 0.6.0 fixture uses a local product source; supply --podman-archive '
                'from the reviewed head d69c59762bce6320f3d6e98ae1843af58d09e3b5, or push the '
                'product branch and replace the mapping with the real GitHub archive identity')
        if podman_archive is None:
            podman_archive = work / 'podman-source.tar.gz'
            subprocess.run(['curl', '--fail', '--location', '--proto', '=https',
                            '--proto-redir', '=https', '--max-time', '120', '--max-filesize', str(LIMIT),
                            '--output', str(podman_archive),
                            f"https://api.github.com/repos/musutrade/GateProbes/tarball/{podman_pin['commit']}"], check=True)
        podman = work / 'podman-native'
        extract(podman_archive, podman, expected_sha=podman_pin['archive_sha256'],
                prefix=podman_pin['prefix'])
        if podman_pin.get('source') == 'local-archive':
            podman_lock_sha256 = pin_podman_local_protocol(podman, crate, env)
            podman_extra = []
        else:
            pin_podman_protocol(podman, podman_pin['source_protocol'], podman_pin['protocol'])
            podman_lock_sha256 = None
            podman_extra = ['--config',
                            'patch.crates-io.gatehouse-runtime-protocol.path=' + json.dumps(str(crate / 'runtime-protocol'))]
        builds.append(('podman', podman / 'Cargo.toml', podman_extra,
                       'gateprobes-podman-lifecycle'))
        fault = work / 'podman-fault'
        with (work / 'fault-build.log').open('w') as log:
            subprocess.run(['rustc', '--edition=2021',
                            str(crate / 'src/process/podman_capture_host/fault_fixture.rs'),
                            '-o', str(fault)], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        binaries['fault'] = fault
    for name, manifest, extra, filename in builds:
        env['CARGO_TARGET_DIR'] = str(work / (name + '-target'))
        with (work / (name + '-build.log')).open('w') as log:
            subprocess.run(['cargo', 'build', '--locked', '--manifest-path', str(manifest), *extra],
                           env=env, stdout=log, stderr=subprocess.STDOUT, check=True, cwd=crate)
        binaries[name] = Path(env['CARGO_TARGET_DIR']) / 'debug' / filename
    if before != source_digest(crate):
        raise ValueError('Core sources changed during fixture build')
    output.mkdir(mode=0o700, parents=True)
    receipt = {'schema': 1, 'source_only': True, 'gp_commit': COMMIT,
               'archive_sha256': ARCHIVE_SHA256, 'core_source_sha256': before, 'binaries': {}}
    if podman_pin is not None:
        receipt['podman'] = {'commit': podman_pin['commit'],
                             'archive_sha256': podman_pin['archive_sha256']}
        if podman_pin.get('source') == 'local-archive':
            if podman_lock_sha256 is None:
                raise ValueError('local Podman fixture lock was not prepared')
            receipt['podman']['source'] = 'local-archive'
            receipt['podman']['lock_sha256'] = podman_lock_sha256
    for name, binary in binaries.items():
        shutil.copyfile(binary, output / name)
        (output / name).chmod(0o500)
        receipt['binaries'][name] = digest(output / name)
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    (output / 'receipt.json').chmod(0o600)
    return receipt


def environment(crate, bundle, journal):
    bundle, journal = Path(bundle).resolve(), Path(journal).resolve()
    receipt = json.loads((bundle / 'receipt.json').read_text())
    if (receipt.get('schema'), receipt.get('source_only'), receipt.get('gp_commit'),
        receipt.get('archive_sha256'), receipt.get('core_source_sha256')) != (
            1, True, COMMIT, ARCHIVE_SHA256, source_digest(crate)):
        raise ValueError('lifecycle fixture receipt identity mismatch')
    expected_binaries = {'adapter', 'core'}
    if podman_supported(crate):
        podman_pin = podman_fixture(crate)
        expected_podman = {'commit': podman_pin['commit'],
                           'archive_sha256': podman_pin['archive_sha256']}
        if podman_pin.get('source') == 'local-archive':
            expected_podman['source'] = 'local-archive'
            expected_podman['lock_sha256'] = podman_pin.get('lock_sha256', '')
        if receipt.get('podman') != expected_podman:
            raise ValueError('Podman fixture receipt identity mismatch')
        expected_binaries |= {'podman', 'fault'}
    elif 'podman' in receipt:
        raise ValueError('unexpected Podman fixture receipt')
    if set(receipt.get('binaries', {})) != expected_binaries:
        raise ValueError('lifecycle fixture binary inventory mismatch')
    values = {}
    for name, expected in receipt['binaries'].items():
        path = bundle / name
        if path.is_symlink() or not path.is_file() or digest(path) != expected:
            raise ValueError('lifecycle fixture binary digest mismatch')
        path.chmod(0o500)  # Artifact transfer does not preserve executable modes.
        key = {'podman': 'HG_PODMAN_NATIVE', 'fault': 'HG_PODMAN_FAULT'}.get(name, 'GATEHOUSE_LIFECYCLE_' + name.upper())
        if name in ('podman', 'fault'):
            values[key + '_BINARY'], values[key + '_SHA256'] = str(path), expected
            continue
        values[key], values[key + '_SHA256'] = str(path), expected
    if podman_supported(crate):
        values['HG_PODMAN_CORE_BINARY'] = values['GATEHOUSE_LIFECYCLE_CORE']
        values['HG_PODMAN_CORE_SHA256'] = values['GATEHOUSE_LIFECYCLE_CORE_SHA256']
    journal.mkdir(mode=0o700, parents=True, exist_ok=False)
    for path in journal.parents:
        stat = path.stat()
        if stat.st_mode & 0o022 or stat.st_uid not in (0, os.geteuid()) or path.is_symlink():
            raise ValueError('unprotected lifecycle journal ancestor')
    values['GATEHOUSE_JOURNAL_TEST_ROOT'] = str(journal)
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('build', 'activate'))
    parser.add_argument('--crate', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--work', type=Path)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--podman-archive', type=Path)
    parser.add_argument('--journal', type=Path)
    parser.add_argument('--env-file', type=Path)
    args = parser.parse_args()
    if args.operation == 'build':
        if args.work is None:
            parser.error('build requires --work')
        build(args.crate, args.bundle, args.work, args.archive, args.podman_archive)
    else:
        if args.journal is None or args.env_file is None:
            parser.error('activate requires --journal and --env-file')
        values = environment(args.crate, args.bundle, args.journal)
        if any('\n' in value or '\r' in value for value in values.values()):
            raise ValueError('invalid fixture environment value')
        with args.env_file.open('a') as output:
            output.write(''.join(f'{key}={value}\n' for key, value in values.items()))


if __name__ == '__main__':
    main()
