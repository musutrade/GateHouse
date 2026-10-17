#!/usr/bin/python3 -I
"""Prepare exact locked Cargo directory sources; never install or grant quality."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import tarfile

SPEC = importlib.util.spec_from_file_location('locked_directory_contract',
    Path(__file__).with_name('verify-locked-crates.py'))
CRATES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CRATES)
need = CRATES.need
MAX_INVENTORY = 64 * 1024 * 1024


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode() + b'\n'


def write(path, data, mode=0o444):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as output:
        output.write(data)
        output.flush()
        os.fchmod(output.fileno(), mode)
        os.fsync(output.fileno())


def record(path, value):
    temporary = path.with_name('.' + path.name + '.tmp')
    write(temporary, canonical(value), 0o600)
    os.replace(temporary, path)


def copy_archive(source, destination, expected, limit=None):
    """Snapshot before validation/extraction; never reopen an Agent-writable input."""
    limit = CRATES.MAX_FILE if limit is None else min(limit, CRATES.MAX_FILE)
    need(limit > 0, 'combined crate archive budget exhausted')
    with CRATES.regular(source, limit) as (stream, before):
        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as output:
            sha, size = hashlib.sha256(), 0
            while block := stream.read(1024 * 1024):
                size += len(block)
                need(size <= limit, 'growing crate archive exceeds budget')
                output.write(block)
                sha.update(block)
            need(size == before.st_size and sha.hexdigest() == expected,
                 'crate archive differs from locked checksum during copy')
            output.flush()
            os.fchmod(output.fileno(), 0o444)
            os.fsync(output.fileno())


def checksum(files, archive_sha256):
    raw = canonical({'package': archive_sha256, 'files': {name: row[1] for name, row in files.items()}})
    need(len(raw) <= CRATES.MAX_METADATA, 'Cargo checksum metadata exceeds budget')
    return raw


def package_inventory(directory, expected, archive_sha256):
    files, directories, total = expected
    # Cargo's cache marker is not an archive member and is unnecessary here.
    marker = directory / '.cargo-ok'
    need(not marker.exists() and not marker.is_symlink(), 'unexpected Cargo cache marker')
    actual, actual_directories, actual_total = CRATES.source_inventory(directory)
    metadata = checksum(files, archive_sha256)
    row = actual.pop('.cargo-checksum.json', None)
    need(row == [len(metadata), hashlib.sha256(metadata).hexdigest(), False],
         'generated Cargo checksum metadata differs from locked archive')
    need(actual == files and actual_directories == directories
         and actual_total - len(metadata) == total,
         'prepared directory source differs from locked archive')
    return {'archive_sha256': archive_sha256, 'files': len(files), 'bytes': total,
            'source_inventory_sha256': hashlib.sha256(canonical(files)).hexdigest(),
            'cargo_checksum_sha256': hashlib.sha256(metadata).hexdigest()}


def derive(directory, expected_sha256):
    directory = CRATES.plain(directory, directory=True)
    raw = CRATES.read(directory / 'Cargo.lock')
    packages, local = CRATES.locked_packages(raw, expected_sha256)
    sources = CRATES.plain(directory / 'sources', directory=True)
    archives = CRATES.plain(directory / 'archives', directory=True)
    need({p.name for p in sources.iterdir()} == packages.keys(), 'directory source package inventory differs')
    need({p.name for p in archives.iterdir()} == {p + '.crate' for p in packages},
         'prepared archive inventory differs')
    rows, total, count, compressed = {}, 0, 0, 0
    for package, digest in sorted(packages.items()):
        expected = CRATES.archive_inventory(archives / (package + '.crate'), package, digest)
        total += expected[2]
        count += len(expected[0])
        compressed += (archives / (package + '.crate')).stat().st_size
        need(total + compressed <= CRATES.MAX_TOTAL and count <= CRATES.MAX_FILES, 'locked directory exceeds budget')
        rows[package] = package_inventory(sources / package, expected, digest)
    return {'schema': 'gatehouse-locked-directory/v1', 'lock_sha256': expected_sha256,
            'packages': rows, 'files': count, 'bytes': total, 'archive_bytes': compressed,
            'local_packages_not_checked': local,
            'scope': 'archive-derived directory sources only; no registry index, installation, protected ownership or quality acceptance'}


def verify(directory, expected_sha256):
    directory = CRATES.plain(directory, directory=True)
    need({p.name for p in directory.iterdir()} == {'Cargo.lock', 'sources', 'archives', 'inventory.json'},
         'prepared directory top-level inventory differs')
    value = derive(directory, expected_sha256)
    metadata = CRATES.read(directory / 'inventory.json', MAX_INVENTORY)
    need(metadata == canonical(value), 'prepared inventory differs from independently derived archive facts')
    for root, directories, names in os.walk(directory):
        for name in (None, *directories, *names):
            path = Path(root) if name is None else Path(root) / name
            info = path.lstat()
            mode = 0o555 if stat.S_ISDIR(info.st_mode) or info.st_mode & 0o111 else 0o444
            need(not path.is_symlink() and stat.S_IMODE(info.st_mode) == mode,
                 'prepared input is linked, writable or has unexpected mode')
    return value


def prepare(lock, expected_sha256, archives, destination):
    need(os.geteuid() != 0, 'directory preparation candidate requires the unprivileged operator; never installs')
    raw = CRATES.read(lock)
    packages, local = CRATES.locked_packages(raw, expected_sha256)
    archives = CRATES.plain(archives, directory=True)
    destination = Path(destination)
    need(destination.is_absolute() and '..' not in destination.parts, 'absolute fresh directory output required')
    CRATES.plain(destination.parent, directory=True)
    for source in (archives, Path(lock)):
        need(destination != source and destination not in source.parents and source not in destination.parents,
             'directory output overlaps inputs')
    state = destination.with_name(destination.name + '-preparation.json')
    need(not destination.exists() and not destination.is_symlink()
         and not state.exists() and not state.is_symlink(), 'retain old or partial directory preparation; never reuse')
    destination.mkdir(mode=0o700)
    status = {'schema': 'gatehouse-locked-directory-preparation/v1', 'state': 'incomplete',
              'lock_sha256': expected_sha256, 'prepared_packages': 0}
    record(state, status)
    try:
        write(destination / 'Cargo.lock', raw)
        copies = destination / 'archives'
        sources = destination / 'sources'
        copies.mkdir(mode=0o700)
        sources.mkdir(mode=0o700)
        total, count, compressed = 0, 0, 0
        for package, digest in sorted(packages.items()):
            archive = copies / (package + '.crate')
            copy_archive(archives / archive.name, archive, digest, CRATES.MAX_TOTAL - total - compressed)
            compressed += archive.stat().st_size
            need(compressed <= CRATES.MAX_TOTAL, 'combined compressed crate archives exceed budget')
            expected = CRATES.archive_inventory(archive, package, digest)
            total += expected[2]
            count += len(expected[0])
            need(total + compressed <= CRATES.MAX_TOTAL and count <= CRATES.MAX_FILES, 'locked directory exceeds budget')
            # Standard-library extraction follows validation of the private copy.
            # No custom extractor and no Cargo, build scripts or network access.
            with tarfile.open(archive, mode='r:gz') as bundle:
                bundle.extractall(sources, filter='data')
            directory = sources / package
            need(CRATES.source_inventory(directory) == expected, 'extracted archive source differs')
            for root, directories, names in os.walk(directory):
                Path(root).chmod(0o700)
            write(directory / '.cargo-checksum.json', checksum(expected[0], digest))
            package_inventory(directory, expected, digest)
            status['prepared_packages'] += 1
            record(state, status)
        need(CRATES.read(lock) == raw, 'Cargo.lock changed during directory preparation')
        value = derive(destination, expected_sha256)
        encoded = canonical(value)
        need(len(encoded) <= MAX_INVENTORY, 'prepared directory inventory exceeds budget')
        write(destination / 'inventory.json', encoded)
        for root, directories, names in os.walk(destination):
            for name in names:
                path = Path(root) / name
                path.chmod(0o555 if path.stat().st_mode & 0o111 else 0o444)
            Path(root).chmod(0o555)
        need(verify(destination, expected_sha256) == value, 'final prepared directory verification differs')
        status['state'] = 'prepared'
        record(state, status)
        return value
    except BaseException:
        status['state'] = 'blocked'
        record(state, status)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock', type=Path, required=True)
    parser.add_argument('--lock-sha256', required=True)
    parser.add_argument('--archives', type=Path, required=True)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = prepare(args.lock, args.lock_sha256, args.archives, args.destination)
    except Exception as error:
        parser.exit(1, 'locked directory preparation blocked (' + type(error).__name__ + '); retain private partial state\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
