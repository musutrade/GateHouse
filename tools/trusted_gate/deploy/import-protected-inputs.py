#!/usr/bin/python3 -I
"""Administrator-only input import and isolated Core bootstrap; never install."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tarfile
from types import SimpleNamespace

ENTRY = 'tools/trusted_gate/deploy/import-protected-inputs.py'
FILES = {ENTRY, 'tools/maintenance/restore_evidence.py', *(
    'tools/trusted_gate/deploy/' + name for name in (
        'prepare-official-tools.py', 'acquire-linux-tools.py', 'verify-locked-crates.py',
        'stage-linux-inputs.py', 'run-linux-test.py', 'prepare-linux-workspace.py',
        'prepare-locked-directory.py', 'linux-tools.lock.json', 'build-protected-core.py',
        'preserve-pilot-state.py', 'prepare-linux-volume.py', 'stage-protected-source.py'))}
ENV = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'GIT_CONFIG_NOSYSTEM': '1',
       'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_OPTIONAL_LOCKS': '0'}


def need(value, message):
    if not value:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode() + b'\n'


def unique(pairs):
    value = {}
    for key, item in pairs:
        need(key not in value, 'duplicate bootstrap metadata field')
        value[key] = item
    return value


def path_operand(value):
    value = str(value)
    need(re.fullmatch('/[A-Za-z0-9_./+-]+', value) and not value.startswith('//') and value == str(Path(value))
         and '..' not in Path(value).parts, 'canonical absolute bootstrap path required')
    return Path(value)


def owned(path, directory=False, private=False):
    # Internal archive filenames are data, never systemd operands or shell text.
    # The archive validator already enforces relative containment; allow their
    # full valid names while keeping entry-point operands separately restricted.
    path = Path(path)
    need(path.is_absolute() and '..' not in path.parts, 'absolute protected input path required')
    for parent in (path, *path.parents):
        info = parent.lstat()
        need(info.st_uid == 0 and not stat.S_ISLNK(info.st_mode) and not info.st_mode & 0o022,
             'bootstrap inputs must be Root-protected')
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode), 'unexpected bootstrap input type')
    need(directory or info.st_nlink == 1, 'hard-linked bootstrap input')
    need(not private or not info.st_mode & 0o077, 'bootstrap control must be private')
    return path


def identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def read(path, limit):
    path = owned(path)
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
        before = os.fstat(stream.fileno())
        need(stat.S_ISREG(before.st_mode) and before.st_size <= limit, 'oversized bootstrap metadata')
        raw = stream.read(limit + 1)
        need(len(raw) == before.st_size and len(raw) <= limit and identity(before) ==
             identity(os.fstat(stream.fileno())) == identity(path.lstat()), 'bootstrap input changed')
    return raw


def write(path, raw, mode=0o444):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as output:
        output.write(raw)
        output.flush()
        os.fchmod(output.fileno(), mode)
        os.fsync(output.fileno())


def record(path, value):
    temporary = path.with_name('.' + path.name + '.tmp')
    write(temporary, canonical(value), 0o600)
    os.replace(temporary, path)
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def authenticate(root, manifest, expected_sha256):
    """No project helper is imported until the entire exact tree is checked."""
    need(os.geteuid() == 0, 'administrator authentication required for protected import')
    root = owned(root, directory=True)
    need(Path(__file__) == root / ENTRY, 'run the importer from its protected helper tree')
    raw = read(manifest, 64 * 1024)
    need(isinstance(expected_sha256, str) and re.fullmatch('[0-9a-f]{64}', expected_sha256)
         and hashlib.sha256(raw).hexdigest() == expected_sha256, 'bootstrap manifest pin differs')
    value = json.loads(raw, object_pairs_hook=unique)
    need(set(value) == {'schema', 'revision', 'files'} and value['schema'] == 'gatehouse-bootstrap-code/v1'
         and re.fullmatch('[0-9a-f]{40}', value['revision']) and isinstance(value['files'], dict)
         and value['files'].keys() == FILES and raw == canonical(value), 'bootstrap code inventory differs')
    expected_directories = {str(parent) for name in FILES for parent in Path(name).parents if str(parent) != '.'}
    seen = set()
    for parent, directories, names in os.walk(root):
        for name in directories:
            relative = str((Path(parent) / name).relative_to(root))
            need(relative in expected_directories, 'unexpected bootstrap helper directory')
            owned(Path(parent) / name, directory=True)
        for name in names:
            relative = str((Path(parent) / name).relative_to(root))
            need(relative in FILES, 'unexpected bootstrap helper file')
            seen.add(relative)
    need(seen == FILES, 'bootstrap helper file missing')
    for name, pin in value['files'].items():
        need(isinstance(pin, str) and re.fullmatch('[0-9a-f]{64}', pin)
             and hashlib.sha256(read(root / name, 1024 * 1024)).hexdigest() == pin,
             'bootstrap helper differs from reviewed pin')
    # Fresh exact trees forbid injected bytecode; do not create new bytecode either.
    sys.dont_write_bytecode = True
    return value


def load(root, name):
    path = root / 'tools/trusted_gate/deploy' / name
    spec = importlib.util.spec_from_file_location('protected_' + name.replace('-', '_'), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def git(source, *args):
    owned('/usr/bin/git')
    result = subprocess.run(['/usr/bin/git', '--no-replace-objects', '-c', 'core.hooksPath=/dev/null',
        '-c', 'core.fsmonitor=false', '-c', 'protocol.ext.allow=never', '-C', str(source), *args],
        env=ENV, capture_output=True, check=True, timeout=120)
    need(len(result.stdout) <= 4 * 1024 * 1024, 'frozen source response exceeds budget')
    return result.stdout


def git_metadata(source):
    """Check the worktree/common Git stores before Git can read local settings."""
    pointer = source / '.git'
    owned(pointer, directory=pointer.is_dir())
    if pointer.is_dir():
        store = pointer
    else:
        raw = read(pointer, 4096).decode('utf-8')
        need(raw.startswith('gitdir: ') and raw.endswith('\n') and raw.count('\n') == 1,
             'unsupported frozen Git pointer')
        store = path_operand(raw[8:-1])
    owned(store, directory=True)
    common = store / 'commondir'
    if common.exists() or common.is_symlink():
        raw = read(common, 4096).decode('utf-8')
        need(raw.endswith('\n') and raw.count('\n') == 1, 'unsupported common Git pointer')
        common = store.parent.parent if raw == '../..\n' else path_operand(raw[:-1])
    else:
        common = store
    count, total = 0, 0
    for location in {store, common}:
        owned(location, directory=True)
        for name in ('objects/info/alternates', 'objects/info/http-alternates', 'info/grafts'):
            path = location / name
            need(not path.exists() and not path.is_symlink(), 'external Git object/history inputs are forbidden')
        for parent, directories, files in os.walk(location):
            for name in (*directories, *files):
                path = Path(parent) / name
                is_directory = path.is_dir()
                owned(path, directory=is_directory)
                count += 1
                if not is_directory:
                    size = path.stat().st_size
                    need(size <= 128 * 1024 * 1024, 'frozen Git file exceeds budget')
                    total += size
                need(count <= 20000 and total <= 256 * 1024 * 1024, 'frozen Git stores exceed budget')


def frozen_lock(source, head, base):
    source = owned(source, directory=True)
    need(re.fullmatch('[0-9a-f]{40}', head) and re.fullmatch('[0-9a-f]{40}', base), 'exact frozen head/base required')
    git_metadata(source)
    need(git(source, 'rev-parse', 'HEAD').decode().strip() == head, 'frozen source head differs')
    git(source, 'merge-base', '--is-ancestor', base, head)
    object_name = head + ':tools/harness-gate/Cargo.lock'
    size = int(git(source, 'cat-file', '-s', object_name))
    need(0 < size <= 4 * 1024 * 1024, 'frozen Cargo.lock exceeds budget')
    raw = git(source, 'cat-file', 'blob', object_name)
    need(len(raw) == size and read(source / 'tools/harness-gate/Cargo.lock', 4 * 1024 * 1024) == raw,
         'frozen Cargo.lock differs from candidate Git object')
    return raw


def readonly(root):
    for parent, directories, files in os.walk(root):
        for name in files:
            path = owned(Path(parent) / name)
            path.chmod(0o555 if path.stat().st_mode & 0o111 else 0o444)
        owned(parent, directory=True).chmod(0o555)


def import_tools(helper, lock, pin, archives, destination):
    raw, rows = helper.TOOLS.lock(lock, pin)
    destination.mkdir(mode=0o700)
    write(destination / 'inputs.lock.json', raw)
    copies, toolchain = destination / 'archives', destination / 'toolchain'
    copies.mkdir(mode=0o700)
    toolchain.mkdir(mode=0o700)
    total, compressed, names = 0, 0, set()
    for role, row in sorted(rows.items()):
        copied = copies / row['url'].rsplit('/', 1)[1]
        helper.copy_archive(archives / copied.name, copied, row)
        compressed += copied.stat().st_size
        need(compressed <= helper.TOOLS.MAX_TOTAL, 'imported official archives exceed budget')
        payload = helper.archive_inventory(copied, role, row)
        need(not names & payload.keys(), 'colliding imported tool payloads')
        names.update(payload)
        total += sum(item['file'][0] for item in payload.values())
        need(total <= helper.MAX_TOTAL and len(names) <= helper.MAX_MEMBERS, 'imported tool payload exceeds budget')
        helper.materialize(copied, payload, toolchain)
    value = helper.derive(destination, pin)
    raw_inventory = helper.canonical(value)
    need(len(raw_inventory) <= helper.MAX_INVENTORY, 'imported tool inventory exceeds budget')
    write(destination / 'inventory.json', raw_inventory)
    readonly(destination)
    need(helper.verify(destination, pin) == value, 'imported tools differ from locked archives')
    return value


def import_dependencies(helper, raw, archives, destination):
    pin = hashlib.sha256(raw).hexdigest()
    packages, local = helper.CRATES.locked_packages(raw, pin)
    destination.mkdir(mode=0o700)
    write(destination / 'Cargo.lock', raw)
    copies, sources = destination / 'archives', destination / 'sources'
    copies.mkdir(mode=0o700)
    sources.mkdir(mode=0o700)
    total, compressed, count = 0, 0, 0
    for package, checksum in sorted(packages.items()):
        copied = copies / (package + '.crate')
        helper.copy_archive(archives / copied.name, copied, checksum, helper.CRATES.MAX_TOTAL - total - compressed)
        compressed += copied.stat().st_size
        expected = helper.CRATES.archive_inventory(copied, package, checksum)
        total += expected[2]
        count += len(expected[0])
        need(total + compressed <= helper.CRATES.MAX_TOTAL and count <= helper.CRATES.MAX_FILES,
             'imported dependency sources exceed budget')
        with tarfile.open(copied, mode='r:gz') as bundle:
            bundle.extractall(sources, filter='data')
        directory = sources / package
        need(helper.CRATES.source_inventory(directory) == expected, 'imported dependency source differs')
        for parent, _, _ in os.walk(directory):
            Path(parent).chmod(0o700)
        write(directory / '.cargo-checksum.json', helper.checksum(expected[0], checksum))
        helper.package_inventory(directory, expected, checksum)
    value = helper.derive(destination, pin)
    raw_inventory = helper.canonical(value)
    need(len(raw_inventory) <= helper.MAX_INVENTORY, 'imported dependency inventory exceeds budget')
    write(destination / 'inventory.json', raw_inventory)
    readonly(destination)
    need(helper.verify(destination, pin) == value, 'imported dependencies differ from locked archives')
    return value


def import_inputs(root, manifest, manifest_pin, source, head, base, tool_archives, crate_archives, destination):
    os.umask(0o077)
    code = authenticate(root, manifest, manifest_pin)
    root, destination = Path(root), path_operand(destination)
    owned(destination.parent, directory=True, private=True)
    source, tool_archives, crate_archives = map(path_operand, (source, tool_archives, crate_archives))
    inputs = (root, Path(manifest), source, tool_archives, crate_archives)
    need(all(destination != path and destination not in path.parents and path not in destination.parents
             for path in inputs), 'protected output overlaps bootstrap inputs')
    state = destination.with_name(destination.name + '-import.json')
    need(not destination.exists() and not destination.is_symlink() and not state.exists() and not state.is_symlink(),
         'retain old or partial protected import; never reuse')
    raw = frozen_lock(source, head, base)
    official, directory = load(root, 'prepare-official-tools.py'), load(root, 'prepare-locked-directory.py')
    lock = root / 'tools/trusted_gate/deploy/linux-tools.lock.json'
    tool_pin = code['files']['tools/trusted_gate/deploy/linux-tools.lock.json']
    destination.mkdir(mode=0o700)
    status = {'schema': 'gatehouse-protected-import/v1', 'state': 'incomplete', 'head': head, 'base': base,
              'helper_revision': code['revision'], 'helper_manifest_sha256': manifest_pin,
              'lock_sha256': hashlib.sha256(raw).hexdigest(), 'tool_lock_sha256': tool_pin,
              'scope': 'archive-derived input import only; no installation, Core build, protected worker or quality acceptance'}
    record(state, status)
    try:
        tools = import_tools(official, lock, tool_pin, tool_archives, destination / 'tools')
        dependencies = import_dependencies(directory, raw, crate_archives, destination / 'dependencies')
        need(frozen_lock(source, head, base) == raw, 'frozen source changed during import')
        authenticate(root, manifest, manifest_pin)
        status.update(state='imported', tool_files=len(tools['files']), tool_bytes=tools['bytes'],
                      dependency_packages=len(dependencies['packages']), dependency_files=dependencies['files'],
                      dependency_bytes=dependencies['bytes'],
                      tool_inventory_sha256=hashlib.sha256(read(destination / 'tools/inventory.json',
                                                               official.MAX_INVENTORY)).hexdigest(),
                      dependency_inventory_sha256=hashlib.sha256(read(destination / 'dependencies/inventory.json',
                                                                     directory.MAX_INVENTORY)).hexdigest())
        write(destination / 'binding.json', canonical(status))
        readonly(destination)
        record(state, status)
        return status
    except BaseException:
        status['state'] = 'blocked'
        record(state, status)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    if sys.argv[1:2] == ['stage-source']:
        for name in ('helper-root', 'manifest', 'bundle', 'tool-recipe'):
            parser.add_argument('--' + name, type=Path, required=True)
        for name in ('manifest-sha256', 'bundle-sha256', 'head', 'tool-recipe-sha256'):
            parser.add_argument('--' + name, required=True)
        args = parser.parse_args(sys.argv[2:])
        try:
            authenticate(args.helper_root, args.manifest, args.manifest_sha256)
            staging = load(args.helper_root, 'stage-protected-source.py')
            context = SimpleNamespace(**{name: globals()[name] for name in (
                'need', 'owned', 'path_operand', 'read', 'write', 'record', 'canonical', 'unique',
                'git', 'frozen_lock', 'load', 'authenticate', 'readonly', 'ENV')})
            result = staging.stage(context, args.helper_root, args.manifest, args.manifest_sha256,
                args.bundle, args.bundle_sha256, args.head, args.tool_recipe, args.tool_recipe_sha256)
        except Exception as error:
            parser.exit(1, 'source staging blocked (' + type(error).__name__ + '); retain private state\n')
        print(json.dumps({key: result[key] for key in ('schema', 'state', 'head', 'base', 'files', 'bytes', 'production_gate_enabled')}, sort_keys=True))
        return
    if sys.argv[1:2] == ['prepare-volume']:
        for name in ('helper-root', 'manifest', 'tool-recipe'):
            parser.add_argument('--' + name, type=Path, required=True)
        for name in ('manifest-sha256', 'tool-recipe-sha256'):
            parser.add_argument('--' + name, required=True)
        args = parser.parse_args(sys.argv[2:])
        try:
            authenticate(args.helper_root, args.manifest, args.manifest_sha256)
            volume = load(args.helper_root, 'prepare-linux-volume.py')
            context = SimpleNamespace(**{name: globals()[name] for name in (
                'need', 'owned', 'path_operand', 'read', 'write', 'record', 'canonical', 'unique',
                'git', 'frozen_lock', 'load', 'authenticate', 'readonly', 'ENV')})
            result = volume.prepare(context, args.helper_root, args.manifest, args.manifest_sha256,
                                    args.tool_recipe, args.tool_recipe_sha256)
        except Exception as error:
            parser.exit(1, 'volume preparation blocked (' + type(error).__name__ + '); retain private state\n')
        print(json.dumps({key: result[key] for key in ('schema', 'phase', 'status', 'size', 'production_gate_enabled')}, sort_keys=True))
        return
    if sys.argv[1:2] == ['preserve-pilot']:
        for name in ('helper-root', 'manifest', 'destination'):
            parser.add_argument('--' + name, type=Path, required=True)
        parser.add_argument('--manifest-sha256', required=True)
        args = parser.parse_args(sys.argv[2:])
        try:
            authenticate(args.helper_root, args.manifest, args.manifest_sha256)
            retention = load(args.helper_root, 'preserve-pilot-state.py')
            context = SimpleNamespace(**{name: globals()[name] for name in (
                'need', 'owned', 'path_operand', 'read', 'write', 'record', 'canonical', 'unique',
                'git', 'frozen_lock', 'load', 'authenticate', 'readonly', 'ENV')})
            result = retention.preserve(context, args.helper_root, args.manifest, args.manifest_sha256,
                                        args.destination)
        except Exception as error:
            parser.exit(1, 'pilot preservation blocked (' + type(error).__name__ + '); retain private state\n')
        summary = {key: result[key] for key in ('schema', 'state', 'head', 'run', 'production_gate_enabled')}
        summary['manifest_sha256'] = hashlib.sha256(canonical(result)).hexdigest()
        print(json.dumps(summary, sort_keys=True))
        return
    if sys.argv[1:2] == ['build-core']:
        for name in ('helper-root', 'manifest', 'source', 'inputs', 'host-recipe', 'destination'):
            parser.add_argument('--' + name, type=Path, required=True)
        for name in ('manifest-sha256', 'head', 'base', 'inputs-sha256', 'host-recipe-sha256'):
            parser.add_argument('--' + name, required=True)
        args = parser.parse_args(sys.argv[2:])
        try:
            authenticate(args.helper_root, args.manifest, args.manifest_sha256)
            builder = load(args.helper_root, 'build-protected-core.py')
            context = SimpleNamespace(**{name: globals()[name] for name in (
                'need', 'owned', 'path_operand', 'read', 'write', 'record', 'canonical', 'unique',
                'git', 'frozen_lock', 'load', 'authenticate', 'readonly', 'ENV')})
            result = builder.build(context, args.helper_root, args.manifest, args.manifest_sha256,
                args.source, args.head, args.base, args.inputs, args.inputs_sha256,
                args.host_recipe, args.host_recipe_sha256, args.destination)
        except Exception as error:
            parser.exit(1, 'Core bootstrap blocked (' + type(error).__name__ + '); retain private state\n')
        print(json.dumps(result, sort_keys=True))
        return
    for name in ('helper-root', 'manifest', 'source', 'tool-archives', 'crate-archives', 'destination'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('manifest-sha256', 'head', 'base'):
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    try:
        result = import_inputs(args.helper_root, args.manifest, args.manifest_sha256, args.source,
            args.head, args.base, args.tool_archives, args.crate_archives, args.destination)
    except Exception as error:
        parser.exit(1, 'protected input import blocked (' + type(error).__name__ + '); retain private state\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
