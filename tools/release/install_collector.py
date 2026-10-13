#!/usr/bin/env python3
"""Host-trusted, offline Linux collector lifecycle. No payload code is executed."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import shutil
import stat
import tarfile
import uuid

import collector_assets as assets
import collector_receipt as receipt
import collector_store as store

require = assets.require
DELIVERY = '.delivery'


def sync(directory):
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic(path, value):
    temporary = path.parent / ('.write-' + path.name)
    if temporary.exists() or temporary.is_symlink():
        assets.regular(temporary)
        temporary.unlink()
    with temporary.open('x') as stream:
        json.dump(value, stream, sort_keys=True)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    sync(path.parent)


def directories(names):
    return {str(p) for name in names for p in Path(name).parents if str(p) != '.'}


def tree_check(root, names, *, complete):
    """Never traverse links. Undeclared content blocks removal as well as install."""
    expected, dirs = set(names), directories(names)
    found = set()
    for current, children, files in os.walk(root, followlinks=False):
        for name in children + files:
            path = Path(current) / name
            relative = str(path.relative_to(root))
            mode = path.lstat()
            require(not stat.S_ISLNK(mode.st_mode), 'unsafe installed link')
            if stat.S_ISDIR(mode.st_mode):
                require(relative in dirs, 'extra installed directory')
            else:
                receipt.regular(path)
                require(relative in expected, 'extra installed payload')
                found.add(relative)
    require(not complete or found == expected, 'missing installed payload')


def remove_owned(root, names):
    if not root.exists():
        return
    require(not root.is_symlink() and root.is_dir(), 'unsafe transaction directory')
    tree_check(root, names, complete=False)
    for name in names:
        (root / name).unlink(missing_ok=True)
    for name in sorted(directories(names), key=lambda n: len(Path(n).parts), reverse=True):
        path = root / name
        if path.exists():
            path.rmdir()
    root.rmdir()
    sync(root.parent)


def recover(root):
    journal = root / 'transaction.json'
    if not journal.exists():
        return
    assets.regular(journal)
    value = assets.read(journal)
    relative = assets.safe_name(value['directory'])
    parts = Path(relative).parts
    require(len(parts) == 2 and parts[0] in ('staging', 'versions'), 'invalid recovery location')
    names = [assets.safe_name(n) for n in value['files']]
    remove_owned(root / relative, names)
    journal.unlink()
    sync(root)


@contextmanager
def locked(root):
    require(root.is_absolute(), 'installation root must be absolute')
    for path in [*reversed(root.parents), root]:
        require(not path.is_symlink(), 'unsafe installation root link')
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    mode = root.stat()
    require(mode.st_uid == os.getuid() and mode.st_mode & 0o077 == 0, 'installation root must be private (0700)')
    descriptor = os.open(root / 'lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        mode = os.fstat(descriptor)
        require(stat.S_ISREG(mode.st_mode) and mode.st_nlink == 1, 'unsafe lock')
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        for name in ('staging', 'versions'):
            path = root / name
            require(not path.is_symlink(), 'unsafe lifecycle directory')
            path.mkdir(mode=0o700, exist_ok=True)
        recover(root)
        yield
    finally:
        os.close(descriptor)


def payload_names(manifest):
    names = [assets.safe_name(p['path']) for p in manifest['payloads']]
    require(all(n != DELIVERY and not n.startswith(DELIVERY + '/') for n in names), 'reserved payload path')
    require(not set(names) & directories(names), 'payload file/directory collision')
    require('bin/harness-gate-rust-collector' in names, 'missing collector launcher')
    return names


def extract(archive_path, destination, manifest):
    names = payload_names(manifest)
    expected = {p['path']: p['sha256'] for p in manifest['payloads']}
    dirs, seen = directories(names), set()
    with tarfile.open(archive_path, 'r:') as archive:
        for member in archive:
            name = assets.safe_name(member.name)
            require(name not in seen, 'duplicate archive member')
            seen.add(name)
            require(not member.pax_headers and not member.issparse(), 'extended archive metadata rejected')
            if member.isdir():
                require(name in dirs and member.mode & ~0o777 == 0, 'extra/unsafe archive directory')
                continue
            require(member.isfile() and name in expected and member.mode in (0o644, 0o755),
                    'unsafe link/type/mode or extra archive payload')
            require(name != 'bin/harness-gate-rust-collector' or member.mode == 0o755,
                    'collector launcher must be executable')
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            with archive.extractfile(member) as source, path.open('xb') as output:
                shutil.copyfileobj(source, output)
                os.fchmod(output.fileno(), member.mode)
                output.flush()
                os.fsync(output.fileno())
            require(assets.sha(path) == expected[name], 'payload checksum mismatch')
        archive.fileobj.seek(archive.offset)
        while block := archive.fileobj.read(1024 * 1024):
            require(not any(block), 'extra data after archive terminator')
    require(set(names) <= seen, 'missing archive payload')
    for name in sorted(dirs, key=lambda n: len(Path(n).parts), reverse=True):
        sync(destination / name)
    sync(destination)


def installed(root, selected, trust):
    path = root / 'versions' / assets.version(selected)
    require(path.is_dir() and not path.is_symlink(), 'version not installed')
    require(not (path / DELIVERY).is_symlink(), 'unsafe delivery metadata link')
    manifest, controls = verify_directory(path, trust, 'rust-collector-v' + selected)
    names = payload_names(manifest) + [DELIVERY + '/' + n for n in controls]
    tree_check(path, names, complete=True)
    return path, names


def verify_directory(path, trust, tag):
    metadata = path / DELIVERY
    replay = metadata / receipt.NAME
    if replay.exists() or replay.is_symlink():
        assets.regular(replay)
        manifest = assets.contract.load_manifest((metadata / 'manifest.json').read_bytes())
        digest = receipt.verify(assets.read(replay), path, manifest)
        manifest = assets.verify(metadata, trust, tag, archive_sha256=digest)
        controls = list(assets.ASSETS[1:] + assets.CONTROL) + [receipt.NAME]
        if (metadata / 'collector.tar').exists():
            controls.append('collector.tar')
    else:
        manifest = assets.verify(metadata, trust, tag)
        controls = list(assets.ASSETS + assets.CONTROL)
        for row in manifest['payloads']:
            receipt.regular(path / row['path'])
            require(assets.sha(path / row['path']) == row['sha256'], 'installed payload checksum mismatch')
        with tarfile.open(metadata / 'collector.tar', 'r:') as archive:
            for member in archive:
                if member.isfile():
                    require(stat.S_IMODE((path / assets.safe_name(member.name)).stat().st_mode) == member.mode,
                            'installed payload mode mismatch')
    return manifest, controls


def compact(root, path, trust, tag):
    """Verify first; receipt + archive is a recoverable intermediate format."""
    manifest, _ = verify_directory(path, trust, tag)
    archive = path / DELIVERY / 'collector.tar'
    replay = path / DELIVERY / receipt.NAME
    if not replay.exists():
        value = receipt.create(archive)
        require(receipt.verify(value, path, manifest) == assets.sha(archive), 'receipt archive checksum mismatch')
        # Keep incomplete receipt writes outside the verified version tree.
        temporary = root / 'migration-receipt.json'
        atomic(temporary, value)
        os.replace(temporary, replay)
        sync(replay.parent)
    verify_directory(path, trust, tag)
    archive.unlink(missing_ok=True)
    sync(path / DELIVERY)
    for row in manifest['payloads']:
        store.share(root, path / row['path'], row['sha256'])
        sync((path / row['path']).parent)
    sync(store.directory(root))
    verify_directory(path, trust, tag)


def install(root, release, trust, tag, checkpoint=lambda _: None, *, self_check=None):
    with locked(root):
        # Snapshot only fixed release names. Verification operates on private bytes.
        require(set(p.name for p in release.iterdir()) == set(assets.ASSETS + assets.CONTROL),
                'missing/extra release assets')
        stage_name = 'staging/' + uuid.uuid4().hex
        stage = root / stage_name
        controls = [DELIVERY + '/' + n for n in assets.ASSETS + assets.CONTROL]
        atomic(root / 'transaction.json', {'directory': stage_name, 'files': controls})
        (stage / DELIVERY).mkdir(parents=True, mode=0o700)
        for name in assets.ASSETS + assets.CONTROL:
            assets.regular(release / name)
            shutil.copyfile(release / name, stage / DELIVERY / name)
            with (stage / DELIVERY / name).open('rb') as stream:
                os.fsync(stream.fileno())
        sync(stage / DELIVERY)
        manifest = assets.verify(stage / DELIVERY, trust, tag)
        selected = assets.version(manifest['collector']['version'])
        destination = root / 'versions' / selected
        require(not destination.exists() and not destination.is_symlink(), 'version already installed; use select')
        names = payload_names(manifest) + controls + [DELIVERY + '/' + receipt.NAME]
        atomic(root / 'transaction.json', {'directory': stage_name, 'files': names})
        checkpoint('verified')
        extract(stage / DELIVERY / 'collector.tar', stage, manifest)
        tree_check(stage, payload_names(manifest) + controls, complete=True)
        compact(root, stage, trust, tag)
        if self_check is not None:
            self_check(stage)
            verify_directory(stage, trust, tag)
        checkpoint('extracted')
        os.rename(stage, destination)
        sync(root / 'versions')
        sync(root / 'staging')
        checkpoint('committed')
        atomic(root / 'current.json', {'version': selected})
        checkpoint('activated')
        (root / 'transaction.json').unlink()
        sync(root)
        return destination / 'bin/harness-gate-rust-collector'


def select(root, selected, trust):
    """Explicit selection/rollback rechecks signature, ABI and every installed byte."""
    with locked(root):
        path, _ = installed(root, selected, trust)
        atomic(root / 'current.json', {'version': selected})
        return path / 'bin/harness-gate-rust-collector'


def uninstall(root, selected, trust):
    with locked(root):
        path, names = installed(root, selected, trust)
        current = root / 'current.json'
        if current.exists():
            assets.regular(current)
            if assets.read(current)['version'] == selected:
                current.unlink()
                sync(root)
        atomic(root / 'transaction.json', {'directory': str(path.relative_to(root)), 'files': names})
        recover(root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--trust', type=Path, required=True)
    sub = parser.add_subparsers(dest='action', required=True)
    command = sub.add_parser('install')
    command.add_argument('--release', type=Path, required=True)
    command.add_argument('--tag', required=True)
    for action in ('select', 'rollback', 'uninstall'):
        sub.add_parser(action).add_argument('--version', required=True)
    sub.add_parser('recover')
    sub.add_parser('usage')
    sub.add_parser('migrate')
    command = sub.add_parser('cleanup')
    command.add_argument('--keep', type=int, default=2, help='previous versions to retain, in addition to current')
    command.add_argument('--dry-run', action='store_true')
    command = sub.add_parser('export')
    command.add_argument('--version', required=True)
    command.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    trust = assets.read(args.trust)
    if args.action == 'install':
        print(install(args.root, args.release, trust, args.tag))
    elif args.action in ('select', 'rollback'):
        print(select(args.root, args.version, trust))
    elif args.action == 'uninstall':
        uninstall(args.root, args.version, trust)
    elif args.action in ('migrate', 'export'):
        import collector_maintenance as maintenance
        result = (maintenance.migrate(args.root, trust) if args.action == 'migrate' else
                  maintenance.export(args.root, args.version, trust, args.output))
        print(json.dumps(result, sort_keys=True))
    else:
        with locked(args.root):
            if args.action == 'usage':
                print(json.dumps(store.usage(args.root), sort_keys=True))
            elif args.action == 'cleanup':
                import collector_maintenance as maintenance
                print(json.dumps(maintenance.cleanup_locked(args.root, trust, args.keep, dry_run=args.dry_run), sort_keys=True))


if __name__ == '__main__':
    main()
