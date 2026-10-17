"""Verify reviewed, protected runtime inputs without widening controller limits."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path, PurePosixPath
import re
import stat

from .common import canonical, checked, digest, protected, read_json, read_regular, require


MAX_FILE = 512 * 1024 * 1024
MAX_TOTAL = 6 * 1024 * 1024 * 1024
MAX_MEMBERS = 150000
MAX_INVENTORY = 64 * 1024 * 1024
SCOPE = ('official manifest payload assembly; no package code execution, Root ownership, '
         'installation or quality acceptance')


def mount_path(value):
    require(isinstance(value, str) and re.fullmatch('/[A-Za-z0-9_./+-]+', value)
            and value == str(Path(value)) and '..' not in Path(value).parts,
            'runtime mount operands must be canonical absolute paths without systemd escapes')
    return Path(value)


def relative_path(value):
    require(isinstance(value, str) and re.fullmatch('[A-Za-z0-9_./+@=-]+', value)
            and not value.startswith('/') and value == str(PurePosixPath(value))
            and all(part not in ('.', '..', '') for part in value.split('/'))
            and len(value.split('/')) <= 64,
            'unsafe runtime inventory path')
    return PurePosixPath(value)


@checked
def settings(config):
    """Optional fields are explicit, pinned and disjoint from writable domains."""
    paths = []
    if 'runtime' in config:
        value = config['runtime']
        require(isinstance(value, dict) and set(value) == {'root', 'inventory', 'tool_lock_sha256'},
                'declare the complete reviewed runtime binding')
        root, inventory = mount_path(value['root']), mount_path(value['inventory'])
        require(re.fullmatch('[0-9a-f]{64}', value['tool_lock_sha256']) is not None,
                'invalid reviewed tool lock digest')
        protected(root, directory=True)
        protected(inventory)
        require(str(inventory) in config['protected_files'], 'runtime inventory is not pinned')
        require(root != inventory and root not in inventory.parents,
                'runtime inventory must remain outside the payload')
        paths.extend((root, inventory))
    if 'cargo_config' in config:
        path = mount_path(config['cargo_config'])
        protected(path)
        require(str(path) in config['protected_files'], 'Cargo configuration is not pinned')
        require(not config.get('cargo_registry') and not config.get('cargo_git'),
                'readonly Cargo configuration cannot mix with raw registry or Git inputs')
        paths.append(path)
    # A protected path must not alias scratch/cache/source/inbox through ancestry.
    domains = [Path(config[name]) for name in ('source', 'scratch', 'inbox') if name in config]
    if 'cache' in config:
        domains.append(Path(config['cache']['root']))
    require(all(a != b and a not in b.parents and b not in a.parents
                for a in paths for b in domains), 'runtime inputs overlap a mutable execution domain')
    require(all(a != b and a not in b.parents and b not in a.parents
                for index, a in enumerate(paths) for b in paths[index + 1:]),
            'runtime and Cargo bindings must be disjoint')


def unchanged(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


@checked
def file_digest(path, row):
    """Only large runtime files use this separate streaming bound."""
    path = protected(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as source:
        before = os.fstat(source.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                and before.st_size == row[0] and stat.S_IMODE(before.st_mode) == row[2],
                'runtime file size, mode or link count differs')
        sha, size = hashlib.sha256(), 0
        while block := source.read(1024 * 1024):
            size += len(block)
            require(size <= row[0] <= MAX_FILE, 'runtime file exceeds its reviewed bound')
            sha.update(block)
        require(size == row[0] and sha.hexdigest() == row[1]
                and unchanged(before) == unchanged(os.fstat(source.fileno()))
                and unchanged(before) == unchanged(path.lstat()),
                'runtime file bytes or identity changed')


@checked
def verify(config):
    settings(config)
    # Keep these small configuration pins subject to the existing generic limit.
    if 'cargo_config' in config:
        path = protected(config['cargo_config'])
        require(digest(read_regular(path, 1024 * 1024)) == config['protected_files'][str(path)],
                'readonly Cargo configuration differs from its reviewed pin')
    if 'runtime' not in config:
        return None
    value = config['runtime']
    metadata = protected(value['inventory'])
    raw = read_regular(metadata, MAX_INVENTORY)
    require(digest(raw) == config['protected_files'][str(metadata)], 'runtime inventory pin differs')
    inventory = read_json(metadata, MAX_INVENTORY)
    require(set(inventory) == {'schema', 'tool_lock_sha256', 'files', 'directories', 'bytes',
                               'archive_bytes', 'scope'}
            and inventory['schema'] == 'gatehouse-official-tools/v1'
            and inventory['tool_lock_sha256'] == value['tool_lock_sha256']
            and inventory['scope'] == SCOPE and raw == canonical(inventory) + b'\n',
            'runtime inventory contract differs')
    files, directories = inventory['files'], inventory['directories']
    require(isinstance(files, dict) and 0 < len(files) <= MAX_MEMBERS
            and isinstance(directories, list) and len(directories) <= MAX_MEMBERS
            and all(isinstance(name, str) for name in directories)
            and directories == sorted(set(directories)), 'invalid runtime inventory counts')
    derived_directories, total = set(), 0
    for name, row in files.items():
        path = relative_path(name)
        require(isinstance(row, list) and len(row) == 3 and type(row[0]) is int
                and 0 <= row[0] <= MAX_FILE and isinstance(row[1], str)
                and re.fullmatch('[0-9a-f]{64}', row[1]) is not None
                and type(row[2]) is int and row[2] in (0o444, 0o555), 'invalid runtime file row')
        derived_directories.update(str(parent) for parent in path.parents if str(parent) != '.')
        total += row[0]
        require(total <= MAX_TOTAL, 'runtime exceeds total byte budget')
    require(type(inventory['bytes']) is int and inventory['bytes'] == total
            and type(inventory['archive_bytes']) is int and 0 < inventory['archive_bytes'] <= 2 * 1024**3
            and directories == sorted(derived_directories) and not files.keys() & derived_directories,
            'runtime totals or directory inventory differ')
    for name in ('rustc', 'cargo', 'cargo-nextest', 'node', 'rustfmt', 'cargo-clippy'):
        require(files.get('bin/' + name, [0, '', 0])[2] == 0o555, 'approved runtime executable missing')
    root = protected(value['root'], directory=True)
    pending, observed_files, observed_directories = [root], set(), set()
    while pending:
        directory = pending.pop()
        protected(directory, directory=True)
        require(stat.S_IMODE(directory.lstat().st_mode) == 0o555, 'runtime directory must be readonly')
        with os.scandir(directory) as children:
            for entry in children:
                name = str(Path(entry.path).relative_to(root))
                if entry.is_dir(follow_symlinks=False):
                    require(name in derived_directories, 'unexpected runtime directory')
                    observed_directories.add(name)
                    pending.append(Path(entry.path))
                else:
                    require(name in files, 'unexpected runtime file')
                    file_digest(Path(entry.path), files[name])
                    observed_files.add(name)
    require(observed_files == files.keys() and observed_directories == derived_directories,
            'runtime files or directories are missing')
    # Re-read the reviewed pin after streaming, rather than trusting mutable metadata.
    require(read_regular(protected(metadata), MAX_INVENTORY) == raw, 'runtime inventory changed during verification')
    return {'files': len(files), 'bytes': total, 'inventory_sha256': digest(raw)}
