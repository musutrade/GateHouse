#!/usr/bin/python3 -I
"""Read-only comparison of locked crate archives and Cargo's extracted sources.

This does not import inputs, trust registry metadata/tools, or authorize quality.
The caller must provide immutable inputs and a separately trusted lock digest.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import tarfile
import tomllib

SPEC = importlib.util.spec_from_file_location('archive_paths',
    Path(__file__).resolve().parents[2] / 'maintenance/restore_evidence.py')
ARCHIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ARCHIVE)
need = ARCHIVE.require
MAX_METADATA = 4 * 1024 * 1024
MAX_FILE = 128 * 1024 * 1024
MAX_PACKAGE = 256 * 1024 * 1024
MAX_TOTAL = 2 * 1024 * 1024 * 1024
MAX_FILES = 100000
MAX_PACKAGES = 1024
SOURCE = 'registry+https://github.com/rust-lang/crates.io-index'


def safe_path(name):
    path = ARCHIVE.safe_path(name)
    need(len(name) <= 2048 and not any(ord(c) < 32 for c in name), 'invalid crate path')
    return path


def plain(path, directory=False):
    path = Path(path)
    need(path.is_absolute() and '..' not in path.parts, 'absolute crate input path required')
    for parent in (path, *path.parents):
        need(not parent.is_symlink(), 'linked crate input path')
    mode = path.lstat().st_mode
    need(stat.S_ISDIR(mode) if directory else stat.S_ISREG(mode), 'special crate input')
    return path


def identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mode,
            info.st_mtime_ns, info.st_ctime_ns, info.st_nlink)


@contextmanager
def regular(path, limit):
    path = plain(path)
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
        before = os.fstat(stream.fileno())
        need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
             and before.st_size <= limit and not before.st_mode & 0o7000,
             'linked, special, privileged or oversized crate file')
        yield stream, before
        need(identity(before) == identity(os.fstat(stream.fileno())) == identity(path.lstat()),
             'crate file changed while reading')


def read(path, limit=None):
    limit = MAX_METADATA if limit is None else limit
    with regular(path, limit) as (stream, before):
        data = stream.read(limit + 1)
        need(len(data) == before.st_size and len(data) <= limit, 'crate metadata grew')
        return data


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate crate metadata key')
        result[key] = value
    return result


def locked_packages(raw, expected_sha256):
    need(isinstance(expected_sha256, str) and re.fullmatch('[0-9a-f]{64}', expected_sha256)
         and hashlib.sha256(raw).hexdigest() == expected_sha256, 'Cargo.lock differs from trusted digest')
    value = tomllib.loads(raw.decode('utf-8'))
    need(value.get('version') == 4 and isinstance(value.get('package'), list)
         and 0 < len(value['package']) <= MAX_PACKAGES, 'unsupported or oversized Cargo.lock')
    packages, local = {}, []
    for row in value['package']:
        need(isinstance(row, dict), 'invalid locked package record')
        name, version = row.get('name'), row.get('version')
        need(isinstance(name, str) and re.fullmatch('[A-Za-z0-9_][A-Za-z0-9_-]{0,127}', name)
             and isinstance(version, str) and len(version) <= 128
             and re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?', version),
             'invalid locked package identity')
        package = name + '-' + version
        need(package not in packages and package not in local, 'duplicate locked package')
        if 'source' not in row:
            need('checksum' not in row, 'local package has registry checksum')
            local.append(package)
        else:
            need(row['source'] == SOURCE, 'unsupported locked package source')
            checksum = row.get('checksum')
            need(isinstance(checksum, str) and re.fullmatch('[0-9a-f]{64}', checksum),
                 'missing or invalid locked checksum')
            packages[package] = checksum
    need(packages, 'no locked registry packages')
    return packages, sorted(local)


def content(stream, size):
    digest, count = hashlib.sha256(), 0
    while block := stream.read(min(1024 * 1024, size - count + 1)):
        count += len(block)
        need(count <= size, 'crate content grew')
        digest.update(block)
    need(count == size, 'truncated crate content')
    return digest.hexdigest()


def archive_inventory(path, package, checksum):
    files, directories, seen, total = {}, set(), set(), 0
    with regular(path, MAX_FILE) as (stream, before):
        need(content(stream, before.st_size) == checksum, 'crate archive differs from locked checksum: ' + package)
        stream.seek(0)
        # Bound even extended tar metadata before tarfile parses it, and validate
        # the complete gzip stream/trailer. Never extract or retain decoded bytes.
        unpacked = 0
        with gzip.GzipFile(fileobj=stream) as decoded:
            while block := decoded.read(1024 * 1024):
                unpacked += len(block)
                need(unpacked <= MAX_PACKAGE + MAX_METADATA, 'decoded crate archive exceeds budget')
        stream.seek(0)
        with tarfile.open(fileobj=stream, mode='r|gz') as bundle:
            for member in bundle:
                name = member.name
                # tarfile strips the conventional trailing slash on directory headers.
                parts = safe_path(name).parts
                need(parts[0] == package and name not in seen, 'wrong or duplicate crate member')
                seen.add(name)
                need(len(seen) <= MAX_FILES and not member.mode & 0o7000,
                     'crate archive member budget or privileged mode')
                need(member.type in (tarfile.REGTYPE, tarfile.AREGTYPE, tarfile.DIRTYPE),
                     'linked or special crate member')
                if len(parts) == 1:
                    need(member.isdir(), 'crate archive root is not a directory')
                    continue
                relative = '/'.join(parts[1:])
                need(relative not in ('.cargo-ok', '.cargo-checksum.json'), 'reserved Cargo metadata in archive')
                directories.update('/'.join(parts[1:i]) for i in range(2, len(parts)))
                if member.isdir():
                    need(member.size == 0, 'crate directory has content')
                    directories.add(relative)
                    continue
                need(0 <= member.size <= MAX_FILE, 'crate member exceeds budget')
                total += member.size
                need(total <= MAX_PACKAGE, 'crate package exceeds budget')
                with bundle.extractfile(member) as source:
                    files[relative] = [member.size, content(source, member.size), bool(member.mode & 0o111)]
    need(files and not files.keys() & directories, 'empty or conflicting crate archive')
    return files, directories, total


def source_inventory(directory):
    directory = plain(directory, directory=True)
    files, directories, identities, total = {}, set(), {}, 0
    def walk_error(error):
        raise error
    for root, dirs, names in os.walk(directory, followlinks=False, onerror=walk_error):
        parent = plain(root, directory=True)
        info = parent.lstat()
        need(not info.st_mode & 0o7000, 'privileged crate directory')
        identities[parent] = identity(info)
        for name in dirs:
            path = plain(parent / name, directory=True)
            relative = path.relative_to(directory).as_posix()
            safe_path(relative)
            directories.add(relative)
            need(len(directories) <= MAX_FILES, 'crate directory count exceeds budget')
        for name in names:
            path = parent / name
            relative = path.relative_to(directory).as_posix()
            safe_path(relative)
            if relative == '.cargo-ok':
                marker = json.loads(read(path), object_pairs_hook=unique)
                need(type(marker) is dict and set(marker) == {'v'} and type(marker['v']) is int
                     and marker['v'] == 1, 'unsupported Cargo extraction marker')
                continue
            with regular(path, MAX_FILE) as (stream, info):
                files[relative] = [info.st_size, content(stream, info.st_size), bool(info.st_mode & 0o111)]
            total += info.st_size
            need(len(files) <= MAX_FILES and total <= MAX_PACKAGE, 'crate source exceeds budget')
    for path, before in identities.items():
        need(identity(path.lstat()) == before, 'crate directory changed while reading')
    return files, directories, total


def verify(lock, expected_sha256, archives, sources):
    raw = read(lock)
    packages, local = locked_packages(raw, expected_sha256)
    archives, sources = plain(archives, directory=True), plain(sources, directory=True)
    rows, total, count = {}, 0, 0
    for package, checksum in sorted(packages.items()):
        expected = archive_inventory(archives / (package + '.crate'), package, checksum)
        actual = source_inventory(sources / package)
        need(actual == expected, 'extracted crate source differs from locked archive: ' + package)
        files, directories, size = expected
        total += size
        count += len(files)
        need(total <= MAX_TOTAL and count <= MAX_FILES, 'locked crate inventory exceeds budget')
        inventory_sha256 = hashlib.sha256(json.dumps(files, sort_keys=True,
            separators=(',', ':')).encode()).hexdigest()
        rows[package] = {'archive_sha256': checksum, 'files': len(files), 'bytes': size,
                         'source_inventory_sha256': inventory_sha256}
    need(read(lock) == raw, 'Cargo.lock changed during verification')
    return {'schema': 'gatehouse-locked-crates/v1', 'lock_sha256': expected_sha256,
            'packages': rows, 'files': count, 'bytes': total, 'local_packages_not_checked': local,
            'scope': 'read-only selected registry archive/source equivalence; immutable inputs required; '
                     'not registry metadata, compiler provenance, import or quality acceptance'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock', type=Path, required=True)
    parser.add_argument('--lock-sha256', required=True)
    parser.add_argument('--archives', type=Path, required=True)
    parser.add_argument('--sources', type=Path, required=True)
    args = parser.parse_args()
    try:
        value = verify(args.lock, args.lock_sha256, args.archives, args.sources)
    except (OSError, EOFError, ValueError, KeyError, TypeError, UnicodeError, tarfile.TarError) as error:
        parser.exit(1, f'locked crate verification blocked: {error}\n')
    print(json.dumps(value, sort_keys=True))


if __name__ == '__main__':
    main()
