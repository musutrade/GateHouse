"""Verify reviewed, protected runtime inputs without widening controller limits."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tomllib

from .common import canonical, checked, digest, protected, read_json, read_regular, require


MAX_FILE = 512 * 1024 * 1024
MAX_TOTAL = 6 * 1024 * 1024 * 1024
MAX_MEMBERS = 150000
MAX_INVENTORY = 64 * 1024 * 1024
SCOPE = ('official manifest payload assembly; no package code execution, Root ownership, '
         'installation or quality acceptance')
LINUX_MOUNTS = {'directory_sources': '/work/dependencies/sources',
                'test_data_archives': '/work/inputs/test-data-archives', 'onboarding': '/work/inputs/onboarding'}
LINUX_SCOPE = 'reviewed readonly Linux inputs only; no installation or quality acceptance'
ONBOARDING_SHA256 = '152196af414d22f6409ba99e14eebb950823280863e05af6ecd95edfece6936d'


def mount_path(value):
    require(isinstance(value, str) and re.fullmatch('/[A-Za-z0-9_./+-]+', value) and not value.startswith('//')
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


def input_relative_path(value):
    # Crate test fixtures contain spaces and punctuation. These names are file
    # data, never systemd/shell operands; mount_path stays separately restricted.
    require(isinstance(value, str) and 0 < len(value) <= 2048 and '\\' not in value
            and not any(ord(char) < 32 or ord(char) == 127 for char in value)
            and not value.startswith('/') and value == str(PurePosixPath(value))
            and all(part not in ('', '.', '..') for part in value.split('/'))
            and len(value.split('/')) <= 64, 'unsafe Linux input relative path')
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
    if 'linux_inputs' in config:
        value = config['linux_inputs']
        require('runtime' in config and 'cargo_config' in config and 'source' in config
                and isinstance(value, dict) and set(value) == {'inventory', *LINUX_MOUNTS},
                'Linux inputs require the complete runtime, Cargo and role bindings')
        metadata = mount_path(value['inventory'])
        protected(metadata)
        require(str(metadata) in config['protected_files'], 'Linux input inventory is not pinned')
        paths.append(metadata)
        for role in LINUX_MOUNTS:
            path = mount_path(value[role])
            protected(path, directory=True)
            paths.append(path)
    # A protected path must not alias scratch/cache/source/inbox through ancestry.
    domains = [Path(config[name]) for name in ('source', 'scratch', 'inbox') if name in config]
    if 'cache' in config:
        domains.append(Path(config['cache']['root']))
    if 'linux_inputs' in config:
        domains.extend(Path(config[name]) for name in ('records', 'mirror', 'policy_directory') if name in config)
        if 'app_key' in config:
            domains.append(Path(config['app_key']))
        if 'delivery' in config:
            domains.append(Path(config['delivery']['app_key']))
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


def input_tree(root, value):
    """Verify all role bytes against a separately pinned Root inventory."""
    require(isinstance(value, dict) and set(value) == {'files', 'directories', 'bytes'}, 'invalid Linux role inventory')
    files, directories = value['files'], value['directories']
    require(isinstance(files, dict) and 0 < len(files) <= MAX_MEMBERS and isinstance(directories, list)
            and len(directories) <= MAX_MEMBERS and all(isinstance(name, str) for name in directories)
            and directories == sorted(set(directories)), 'invalid Linux input counts')
    derived, total = set(), 0
    for name, row in files.items():
        path = input_relative_path(name)
        require(isinstance(row, list) and len(row) == 3 and type(row[0]) is int and 0 <= row[0] <= MAX_FILE
                and isinstance(row[1], str) and re.fullmatch('[0-9a-f]{64}', row[1])
                and type(row[2]) is int and row[2] in (0o444, 0o555), 'invalid Linux input file row')
        total += row[0]
        require(total <= MAX_TOTAL, 'Linux role exceeds byte budget')
        derived.update(str(parent) for parent in path.parents if str(parent) != '.')
    require(type(value['bytes']) is int and value['bytes'] == total and directories == sorted(derived)
            and not files.keys() & derived, 'Linux role totals or directories differ')
    seen_files, seen_dirs, pending = set(), set(), [protected(root, directory=True)]
    while pending:
        directory = protected(pending.pop(), directory=True)
        require(stat.S_IMODE(directory.stat().st_mode) == 0o555, 'Linux input directory must be readonly')
        with os.scandir(directory) as children:
            for entry in children:
                name = str(Path(entry.path).relative_to(root))
                if entry.is_dir(follow_symlinks=False):
                    require(name in derived, 'unexpected Linux input directory')
                    seen_dirs.add(name);pending.append(Path(entry.path))
                else:
                    require(name in files, 'unexpected Linux input file')
                    file_digest(Path(entry.path), files[name]);seen_files.add(name)
    require(seen_files == files.keys() and seen_dirs == derived, 'Linux input tree is incomplete')
    return total, len(files)


def dependency_binding(source, root, files, expected):
    raw = read_regular(protected(source / 'tools/harness-gate/Cargo.lock'), 4 * 1024**2)
    require(digest(raw) == expected, 'Linux dependencies belong to another source lock')
    lock = tomllib.loads(raw.decode('utf-8'))
    require(type(lock['version']) is int and lock['version'] == 4 and isinstance(lock['package'], list)
            and 0 < len(lock['package']) <= 4096, 'invalid Linux dependency lock')
    packages, seen = {}, set()
    for item in lock['package']:
        name, version = item['name'], item['version']
        require(isinstance(name, str) and re.fullmatch('[A-Za-z0-9_][A-Za-z0-9_-]{0,127}', name)
                and isinstance(version, str) and len(version) <= 128
                and re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?', version),
                'invalid Linux package identity')
        package = name + '-' + version
        require(package not in seen, 'duplicate Linux package identity')
        seen.add(package)
        if 'source' not in item:
            require('checksum' not in item, 'local package has a registry checksum')
            continue
        require(item['source'] == 'registry+https://github.com/rust-lang/crates.io-index'
                and isinstance(item.get('checksum'), str) and re.fullmatch('[0-9a-f]{64}', item['checksum']),
                'unsupported Linux dependency source or checksum')
        packages[package] = item['checksum']
    require(packages and {Path(name).parts[0] for name in files} == packages.keys(),
            'Linux directory sources differ from locked packages')
    for package, checksum in packages.items():
        name = package + '/.cargo-checksum.json'
        require(name in files, 'missing Cargo directory checksum')
        raw_checksum = read_regular(protected(root / name), 4 * 1024**2)
        value = read_json(root / name, 4 * 1024**2)
        expected_files = {key[len(package)+1:]:row[1] for key,row in files.items()
                          if key.startswith(package + '/') and key != name}
        require(value == {'package': checksum, 'files': expected_files} and digest(raw_checksum) == files[name][1],
                'Cargo directory checksum differs from pinned locked sources')
    return len(packages)


@checked
def verify_linux_inputs(config):
    if 'linux_inputs' not in config:
        return None
    value = config['linux_inputs']
    metadata = protected(value['inventory'])
    raw = read_regular(metadata, MAX_INVENTORY)
    require(digest(raw) == config['protected_files'][str(metadata)], 'Linux input inventory pin differs')
    inventory = read_json(metadata, MAX_INVENTORY)
    require(set(inventory) == {'schema', 'scope', 'roles', 'lock_sha256', 'test_data_manifest_sha256',
                               'onboarding_source_sha256'}
            and inventory['schema'] == 'gatehouse-linux-inputs/v1' and inventory['scope'] == LINUX_SCOPE
            and isinstance(inventory['roles'], dict) and inventory['roles'].keys() == LINUX_MOUNTS.keys()
            and raw == canonical(inventory) + b'\n', 'Linux input inventory contract differs')
    total, count = 0, 0
    for role in LINUX_MOUNTS:
        size, entries = input_tree(Path(value[role]), inventory['roles'][role])
        total += size;count += entries
        require(total <= MAX_TOTAL and count <= MAX_MEMBERS, 'combined Linux inputs exceed budget')
    source = Path(config['source'])
    packages = dependency_binding(source, Path(value['directory_sources']),
        inventory['roles']['directory_sources']['files'], inventory['lock_sha256'])
    manifest = protected(source / 'test-data/manifest.json')
    manifest_raw = read_regular(manifest, 4 * 1024**2)
    require(digest(manifest_raw) == inventory['test_data_manifest_sha256'], 'Linux test-data manifest differs')
    data = read_json(manifest, 4 * 1024**2)
    require(data['schema'] == 'external-test-data/v1' and isinstance(data['archives'], list)
            and 0 < len(data['archives']) <= 1024, 'invalid Linux test-data archives')
    archives = {}
    for item in data['archives']:
        checksum = item['archive_sha256']
        require(isinstance(checksum, str) and re.fullmatch('[0-9a-f]{64}', checksum)
                and type(item['archive_size']) is int and 0 < item['archive_size'] <= MAX_FILE
                and checksum + '.tar.gz' not in archives, 'invalid or duplicate Linux test-data archive')
        archives[checksum + '.tar.gz'] = [item['archive_size'], checksum, 0o444]
    require(inventory['roles']['test_data_archives']['files'] == archives, 'Linux archive bytes differ from source manifest')
    onboarding = inventory['roles']['onboarding']['files']
    derived = {name:[row[0], row[1], row[2] == 0o555] for name,row in onboarding.items()}
    require('tools/onboarding/acceptance.py' in onboarding
            and inventory['onboarding_source_sha256'] == ONBOARDING_SHA256 == digest(canonical(derived)),
            'Linux onboarding differs from independently reviewed source')
    cargo_raw = read_regular(protected(config['cargo_config']), 1024**2)
    require(digest(cargo_raw) == config['protected_files'][config['cargo_config']], 'Linux Cargo configuration pin differs')
    cargo = tomllib.loads(cargo_raw.decode())
    require(canonical(cargo) == canonical({'source': {'crates-io': {'replace-with': 'gatehouse-locked'},
                'gatehouse-locked': {'directory': LINUX_MOUNTS['directory_sources']}}, 'net': {'offline': True}}),
            'Linux Cargo configuration must use only the reviewed offline directory sources')
    require(read_regular(protected(metadata), MAX_INVENTORY) == raw
            and read_regular(protected(manifest), 4 * 1024**2) == manifest_raw
            and digest(read_regular(protected(source / 'tools/harness-gate/Cargo.lock'), 4 * 1024**2)) == inventory['lock_sha256'],
            'Linux metadata changed during verification')
    return {'files': count, 'bytes': total, 'packages': packages, 'inventory_sha256': digest(raw)}


@checked
def verify(config):
    settings(config)
    # Keep these small configuration pins subject to the existing generic limit.
    if 'cargo_config' in config:
        path = protected(config['cargo_config'])
        require(digest(read_regular(path, 1024 * 1024)) == config['protected_files'][str(path)],
                'readonly Cargo configuration differs from its reviewed pin')
    linux = verify_linux_inputs(config)
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
    result = {'files': len(files), 'bytes': total, 'inventory_sha256': digest(raw)}
    if linux is not None:
        result['linux_inputs'] = linux
    return result
