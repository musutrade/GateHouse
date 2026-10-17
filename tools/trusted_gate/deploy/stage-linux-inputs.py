#!/usr/bin/python3 -I
"""Stage reviewed offline Linux inputs; never install or authorize a quality result."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys

SPEC = importlib.util.spec_from_file_location('linux_input_contract', Path(__file__).with_name('run-linux-test.py'))
JOB = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(JOB)
MAX_FILE = 512 * 1024 * 1024
MAX_TOTAL = 6 * 1024 * 1024 * 1024
MAX_FILES = 150000
MAX_INVENTORY = 64 * 1024 * 1024
ROLES = {'toolchain', 'registry', 'onboarding', 'archives'}


def need(value, message):
    if not value:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def safe_path(name):
    need(isinstance(name, str) and len(name) <= 2048 and name
         and all(part not in ('', '.', '..') and '\\' not in part and '\0' not in part
                 for part in name.split('/')), 'invalid runtime inventory path')
    return Path(name)


def plain_path(path, directory=False):
    path = Path(path)
    need(path.is_absolute() and '..' not in path.parts, 'absolute runtime path required')
    for parent in (path, *path.parents):
        need(not parent.is_symlink(), 'linked runtime path')
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode), 'special runtime input')
    return path


def fingerprint(path, copy_to=None):
    """Stream large compiler libraries; do not change controller/config budgets."""
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as source:
        before = os.fstat(source.fileno())
        need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and before.st_size <= MAX_FILE,
             'linked, special or oversized runtime input')
        need(not before.st_mode & 0o7000, 'privileged runtime input mode')
        sha, count = hashlib.sha256(), 0
        target = None
        try:
            if copy_to is not None:
                target = open(copy_to, 'xb')
            while data := source.read(1024 * 1024):
                count += len(data)
                need(count <= MAX_FILE, 'growing runtime input exceeds budget')
                sha.update(data)
                if target:
                    target.write(data)
            after = os.fstat(source.fileno())
            need((before.st_size, before.st_mtime_ns, before.st_ctime_ns, before.st_mode) ==
                 (after.st_size, after.st_mtime_ns, after.st_ctime_ns, after.st_mode)
                 and count == before.st_size, 'runtime input changed while reading')
            mode = 0o555 if before.st_mode & 0o111 else 0o444
            if target:
                os.fchmod(target.fileno(), mode)
        finally:
            if target:
                target.close()
    return [count, sha.hexdigest(), mode]


def inventory(directory):
    directory = plain_path(directory, directory=True)
    files, directories, total = {}, [], 0
    for root, dirs, names in os.walk(directory, followlinks=False):
        for name in dirs:
            path = plain_path(Path(root) / name, directory=True)
            relative = str(path.relative_to(directory))
            safe_path(relative)
            directories.append(relative)
            need(len(directories) <= MAX_FILES, 'runtime directory count exceeds budget')
        for name in names:
            path = plain_path(Path(root) / name)
            relative = str(path.relative_to(directory))
            safe_path(relative)
            files[relative] = fingerprint(path)
            total += files[relative][0]
            need(len(files) <= MAX_FILES and total <= MAX_TOTAL, 'runtime inventory exceeds budget')
    return files, sorted(directories), total


def save(path, value, mode=0o600):
    temporary = path.with_name('.' + path.name + '.tmp')
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(canonical(value) + b'\n')
        stream.flush()
        os.fchmod(stream.fileno(), mode)
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def verify(directory, expected_sha256, trusted=False):
    directory = plain_path(directory, directory=True)
    if trusted:
        need(os.geteuid() == 0, 'trusted runtime verification requires the administrator')
        for parent in (directory, *directory.parents):
            info = parent.lstat()
            need(info.st_uid == 0 and not info.st_mode & 0o022, 'unprotected runtime parent')
    manifest = plain_path(directory / 'inventory.json')
    need(manifest.stat().st_size <= MAX_INVENTORY, 'runtime inventory metadata exceeds budget')
    raw = manifest.read_bytes()
    need(re.fullmatch('[0-9a-f]{64}', expected_sha256) and hashlib.sha256(raw).hexdigest() == expected_sha256,
         'runtime inventory differs from reviewed digest')
    value = json.loads(raw, object_pairs_hook=JOB.INPUTS.unique)
    need(set(value) == {'schema', 'files', 'directories', 'total_bytes', 'inputs'}
         and value['schema'] == 'gatehouse-linux-inputs/v1', 'unknown runtime inventory')
    expected = value['files']
    need(isinstance(expected, dict) and 0 < len(expected) <= MAX_FILES and
         all(safe_path(name).parts[0] in ROLES for name in expected), 'unknown runtime role')
    need(all(isinstance(row, list) and len(row) == 3 and type(row[0]) is int
             and 0 <= row[0] <= MAX_FILE and isinstance(row[1], str)
             and re.fullmatch('[0-9a-f]{64}', row[1]) and row[2] in (0o444, 0o555)
             for row in expected.values()), 'invalid runtime file metadata')
    need({'toolchain/bin/' + name for name in ('rustc', 'cargo', 'rustdoc', 'cargo-nextest', 'node')}
         <= set(expected), 'missing required runtime tool')
    need(isinstance(value['directories'], list) and len(value['directories']) <= MAX_FILES
         and all(safe_path(name).parts[0] in ROLES for name in value['directories'])
         and ROLES <= set(value['directories']), 'invalid runtime directories')
    inputs = value['inputs']
    need(isinstance(inputs, dict) and set(inputs) == {'test_data_manifest_sha256', 'onboarding_commit',
         'onboarding_sha256'} and inputs['onboarding_commit'] == JOB.ONBOARDING_COMMIT
         and inputs['onboarding_sha256'] == JOB.ONBOARDING_SHA256
         and isinstance(inputs['test_data_manifest_sha256'], str)
         and re.fullmatch('[0-9a-f]{64}', inputs['test_data_manifest_sha256']), 'runtime input pins differ')
    actual, directories, total = inventory(directory)
    actual.pop('inventory.json')
    # inventory.json is coordinator metadata and is verified separately above.
    total -= len(raw)
    need(actual == expected and directories == value['directories'] and total == value['total_bytes'],
         'runtime files, directories, bytes or executable modes differ')
    for root, dirs, names in os.walk(directory):
        for name in (None, *dirs, *names):
            path = Path(root) if name is None else Path(root) / name
            info = path.lstat()
            expected_mode = 0o555 if path.is_dir() else 0o555 if info.st_mode & 0o111 else 0o444
            need(stat.S_IMODE(info.st_mode) == expected_mode, 'runtime input is writable or has unexpected mode')
            if trusted:
                need(info.st_uid == 0, 'runtime input is not administrator-owned')
    return {'inventory_sha256': expected_sha256, 'files': len(expected), 'bytes': total,
            'trusted_owner_verified': trusted}


def stage(toolchain, registry, onboarding, archives, nextest, node, manifest, manifest_sha256, destination):
    need(os.geteuid() != 0, 'runtime staging requires an unprivileged operator; it never installs')
    destination = Path(destination)
    need(destination.is_absolute() and '..' not in destination.parts, 'absolute fresh runtime destination required')
    plain_path(destination.parent, directory=True)
    record = destination.with_name(destination.name + '-preparation.json')
    need(not destination.exists() and not destination.is_symlink() and not record.exists()
         and not record.is_symlink(), 'old or partial runtime staging must be retained, never reused')
    sources = {role: plain_path(path, directory=True) for role, path in
               (('toolchain', toolchain), ('registry', registry), ('onboarding', onboarding), ('archives', archives))}
    for path in (*sources.values(), plain_path(nextest), plain_path(node), plain_path(manifest)):
        need(path != destination and path not in destination.parents and destination not in path.parents,
             'runtime output overlaps its input')
    need(set(p.name for p in sources['registry'].iterdir()) == {'CACHEDIR.TAG', 'cache', 'src', 'index'},
         'copy only the registry, never Cargo home credentials/configuration')
    need(JOB.directory_digest(onboarding) == JOB.ONBOARDING_SHA256, 'onboarding source differs from fixed CI pin')
    manifest_raw = JOB.INPUTS.regular(manifest, 16 * 1024 * 1024)
    need(hashlib.sha256(manifest_raw).hexdigest() == manifest_sha256, 'test-data manifest differs from reviewed pin')
    data = json.loads(manifest_raw, object_pairs_hook=JOB.INPUTS.unique)
    need(data['schema'] == 'external-test-data/v1' and data['archives'], 'invalid test-data manifest')
    expected_archives = {}
    for row in data['archives']:
        sha = row['archive_sha256']
        need(re.fullmatch('[0-9a-f]{64}', sha) and type(row['archive_size']) is int
             and 0 < row['archive_size'] <= MAX_FILE, 'invalid pinned test-data archive')
        name = sha + '.tar.gz'
        need(name not in expected_archives, 'duplicate pinned archive')
        expected_archives[name] = [row['archive_size'], sha, 0o444]
    state = {'state': 'incomplete', 'scope': 'operator input staging only; not installed or quality acceptance'}
    save(record, state)
    destination.mkdir(mode=0o700)
    try:
        files, directories, total = {}, set(ROLES), 0
        for role, source in sources.items():
            if role == 'archives':
                selected, subdirs = expected_archives, []
            else:
                selected, subdirs, _ = inventory(source)
            if role == 'toolchain':
                need({'bin/rustc', 'bin/cargo', 'bin/rustdoc'} <= set(selected), 'incomplete Rust toolchain')
                need(not {'bin/cargo-nextest', 'bin/node'} & set(selected), 'extra tool would overwrite Rust input')
            (destination / role).mkdir(mode=0o700)
            for name in sorted(subdirs, key=lambda name: (name.count('/'), name)):
                (destination / role / name).mkdir(mode=0o700)
                directories.add(role + '/' + name)
            for name, expected in sorted(selected.items()):
                target = destination / role / safe_path(name)
                actual = fingerprint(source / name, target)
                need(actual == expected, 'runtime input changed during copy or pinned archive differs')
                files[role + '/' + name] = actual
                total += actual[0]
                need(len(files) <= MAX_FILES and total <= MAX_TOTAL, 'combined runtime exceeds budget')
            if role != 'archives':
                after, after_dirs, _ = inventory(source)
                need(after == selected and after_dirs == subdirs, 'runtime source changed during staging')
        for name, source in (('cargo-nextest', nextest), ('node', node)):
            files['toolchain/bin/' + name] = fingerprint(source, destination / 'toolchain/bin' / name)
            need(files['toolchain/bin/' + name][2] == 0o555, 'runtime tool is not executable')
            total += files['toolchain/bin/' + name][0]
        need(len(files) <= MAX_FILES and total <= MAX_TOTAL, 'combined runtime exceeds budget')
        value = {'schema': 'gatehouse-linux-inputs/v1', 'files': files, 'directories': sorted(directories),
                 'total_bytes': total, 'inputs': {'test_data_manifest_sha256': manifest_sha256,
                     'onboarding_commit': JOB.ONBOARDING_COMMIT, 'onboarding_sha256': JOB.ONBOARDING_SHA256}}
        need(len(canonical(value)) + 1 <= MAX_INVENTORY, 'runtime inventory metadata exceeds budget')
        save(destination / 'inventory.json', value, mode=0o444)
        for root, dirs, names in os.walk(destination, topdown=False):
            os.chmod(root, 0o555)
        sha = hashlib.sha256((destination / 'inventory.json').read_bytes()).hexdigest()
        result = verify(destination, sha)
        state.update(state='staged', **result)
        save(record, state)
        return state
    except (OSError, ValueError, KeyError, TypeError) as error:
        state.update(state='blocked', reason=str(error))
        save(record, state)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('toolchain', 'registry', 'onboarding', 'archives', 'nextest', 'node', 'manifest', 'destination'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--manifest-sha256', required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(stage(**vars(args)), sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(json.dumps({'state': 'blocked', 'reason': str(error)}))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
