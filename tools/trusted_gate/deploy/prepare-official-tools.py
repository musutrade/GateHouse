#!/usr/bin/python3 -I
"""Assemble reviewed official tool payloads without executing package code."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import importlib.util
import json
import lzma
import os
from pathlib import Path
import posixpath
import stat
import tarfile


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


TOOLS = load('official_archive_lock', 'acquire-linux-tools.py')
CRATES = TOOLS.INPUTS
STAGE = load('official_file_contract', 'stage-linux-inputs.py')
need = CRATES.need
MAX_FILE = 512 * 1024 * 1024
MAX_DECODED = 2 * 1024 * 1024 * 1024
MAX_TOTAL = 6 * 1024 * 1024 * 1024
MAX_MEMBERS = 150000
MAX_METADATA = 4 * 1024 * 1024
MAX_INVENTORY = 64 * 1024 * 1024


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode() + b'\n'


def component_identity(role, row):
    filename = row['url'].rsplit('/', 1)[1]
    if role in TOOLS.RUST:
        root = filename.removesuffix('.tar.xz')
        component = 'rust-std-' + TOOLS.TARGET if role == 'rust-std' else role
        return root, component
    return ('node-v24.18.0-linux-x64', None) if role == 'node' else (None, None)


def archive_inventory(path, role, row):
    """Read tar with bounded decoding. Unselected Node links are never followed."""
    root, component = component_identity(role, row)
    prefix = root + '/' + component + '/' if component else None
    wanted = root + '/bin/node' if role == 'node' else 'cargo-nextest'
    payload, seen, metadata, count = {}, set(), {}, 0
    with CRATES.regular(path, row['max_bytes']) as (source, before):
        need(CRATES.content(source, before.st_size) == row['sha256'], 'official archive checksum differs')
        source.seek(0)
        decoded = 0
        decoder = lzma.LZMAFile(source) if row['url'].endswith('.xz') else gzip.GzipFile(fileobj=source)
        with decoder as stream:
            while block := stream.read(1024 * 1024):
                decoded += len(block)
                need(decoded <= MAX_DECODED, 'decoded official archive exceeds budget')
        source.seek(0)
        with tarfile.open(fileobj=source, mode='r|*') as bundle:
            for member in bundle:
                CRATES.safe_path(member.name)
                need(member.name not in seen, 'duplicate official archive member')
                seen.add(member.name)
                count += 1
                need(count <= MAX_MEMBERS and not member.mode & 0o7000,
                     'official archive count or privileged mode')
                if root:
                    need(member.name.split('/')[0] == root, 'unexpected official archive root')
                selected = bool(prefix and member.name.startswith(prefix)
                                and member.name != prefix + 'manifest.in') or member.name == wanted
                if member.issym() and role == 'node' and not selected:
                    link = member.linkname
                    need(link and not link.startswith('/') and '\\' not in link and '\0' not in link,
                         'escaping unselected Node link')
                    resolved = posixpath.normpath(posixpath.join(posixpath.dirname(member.name), link))
                    CRATES.safe_path(resolved)
                    need(resolved.split('/')[0] == root, 'escaping unselected Node link')
                    continue
                need(member.type in (tarfile.REGTYPE, tarfile.AREGTYPE, tarfile.DIRTYPE),
                     'linked or special official payload')
                if member.isdir():
                    need(member.size == 0, 'official directory has content')
                    continue
                need(0 <= member.size <= MAX_FILE, 'official payload file exceeds budget')
                meta = component and member.name in (root + '/components', prefix + 'manifest.in')
                if meta:
                    need(member.size <= MAX_METADATA, 'component manifest exceeds budget')
                    with bundle.extractfile(member) as content:
                        raw = content.read(MAX_METADATA + 1)
                    need(len(raw) == member.size, 'truncated component manifest')
                    metadata[member.name] = raw.decode('utf-8').splitlines()
                elif selected:
                    relative = member.name[len(prefix):] if prefix else 'bin/' + role
                    CRATES.safe_path(relative)
                    with bundle.extractfile(member) as content:
                        digest = CRATES.content(content, member.size)
                    payload[relative] = {'member': member.name, 'file': [member.size, digest,
                        0o555 if member.mode & 0o111 else 0o444]}
    if component:
        need(metadata.get(root + '/components') == [component], 'official component selection differs')
        manifest = metadata.get(prefix + 'manifest.in', [])
        need(manifest and len(manifest) <= MAX_MEMBERS
             and all(line.startswith('file:') for line in manifest), 'unsupported component manifest')
        names = [line[5:] for line in manifest]
        for name in names:
            CRATES.safe_path(name)
        need(len(set(names)) == len(names) and set(names) == payload.keys(),
             'component manifest and complete payload differ')
    else:
        need(payload.keys() == {'bin/' + role}, 'missing selected official executable')
    need(payload, 'empty official payload')
    return payload


def materialize(archive, payload, destination):
    wanted = {row['member']: (name, row['file']) for name, row in payload.items()}
    found = set()
    with archive.open('rb') as source, tarfile.open(fileobj=source, mode='r|*') as bundle:
        for member in bundle:
            if member.name not in wanted:
                continue
            relative, expected = wanted[member.name]
            need(member.isfile() and not member.islnk() and member.size == expected[0]
                 and (0o555 if member.mode & 0o111 else 0o444) == expected[2]
                 and member.name not in found, 'official payload changed before materialization')
            found.add(member.name)
            target = destination.joinpath(*CRATES.safe_path(relative).parts)
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'wb') as output, bundle.extractfile(member) as content:
                digest, size = hashlib.sha256(), 0
                while block := content.read(1024 * 1024):
                    size += len(block)
                    need(size <= expected[0], 'growing official payload')
                    output.write(block)
                    digest.update(block)
                need(size == expected[0] and digest.hexdigest() == expected[1], 'official payload bytes differ')
                output.flush()
                os.fchmod(output.fileno(), expected[2])
                os.fsync(output.fileno())
    need(found == wanted.keys(), 'official payload missing during materialization')


def copy_archive(source, destination, row):
    with CRATES.regular(source, row['max_bytes']) as (stream, before):
        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as output:
            sha, size = hashlib.sha256(), 0
            while block := stream.read(1024 * 1024):
                size += len(block)
                need(size <= row['max_bytes'], 'growing official archive')
                output.write(block)
                sha.update(block)
            need(size == before.st_size and sha.hexdigest() == row['sha256'], 'official archive checksum differs')
            output.flush()
            os.fchmod(output.fileno(), 0o444)
            os.fsync(output.fileno())


def derive(directory, expected_sha256):
    raw, rows = TOOLS.lock(directory / 'inputs.lock.json', expected_sha256)
    archives = CRATES.plain(directory / 'archives', directory=True)
    need({p.name for p in archives.iterdir()} == {row['url'].rsplit('/', 1)[1] for row in rows.values()},
         'official copied archive inventory differs')
    files, directories, total, compressed = {}, set(), 0, 0
    for role, row in sorted(rows.items()):
        archive = archives / row['url'].rsplit('/', 1)[1]
        payload = archive_inventory(archive, role, row)
        compressed += archive.stat().st_size
        need(compressed <= TOOLS.MAX_TOTAL, 'combined official archives exceed budget')
        for name, item in payload.items():
            need(name not in files, 'colliding official tool payloads')
            path = CRATES.safe_path(name)
            files[name] = item['file']
            directories.update(str(parent) for parent in path.parents if str(parent) != '.')
            total += item['file'][0]
            need(total <= MAX_TOTAL and len(files) <= MAX_MEMBERS, 'official tool payload exceeds budget')
    need(not files.keys() & directories, 'official file/directory conflict')
    for name in ('rustc', 'cargo', 'cargo-nextest', 'node', 'rustfmt', 'cargo-clippy'):
        need('bin/' + name in files and files['bin/' + name][2] == 0o555,
             'missing or non-executable approved tool')
    return {'schema': 'gatehouse-official-tools/v1', 'tool_lock_sha256': expected_sha256,
            'files': files, 'directories': sorted(directories), 'bytes': total, 'archive_bytes': compressed,
            'scope': 'official manifest payload assembly; no package code execution, Root ownership, installation or quality acceptance'}


def verify(directory, expected_sha256):
    directory = CRATES.plain(directory, directory=True)
    need({p.name for p in directory.iterdir()} == {'inputs.lock.json', 'archives', 'toolchain', 'inventory.json'},
         'official preparation top-level inventory differs')
    value = derive(directory, expected_sha256)
    need(CRATES.read(directory / 'inventory.json', MAX_INVENTORY) == canonical(value),
         'official inventory differs from independently derived archive facts')
    files, directories, total = STAGE.inventory(directory / 'toolchain')
    need(files == value['files'] and directories == value['directories'] and total == value['bytes'],
         'assembled official tool files, modes or directories differ')
    for root, directories, names in os.walk(directory):
        for name in (None, *directories, *names):
            path = Path(root) if name is None else Path(root) / name
            mode = path.lstat().st_mode
            need(not path.is_symlink() and stat.S_IMODE(mode) ==
                 (0o555 if stat.S_ISDIR(mode) or mode & 0o111 else 0o444), 'writable or linked prepared tool input')
    return value


def prepare(lock, expected_sha256, archives, destination):
    need(os.geteuid() != 0, 'official tool preparation requires the unprivileged operator; never installs')
    raw, rows = TOOLS.lock(lock, expected_sha256)
    archives = CRATES.plain(archives, directory=True)
    destination = Path(destination)
    need(destination.is_absolute() and '..' not in destination.parts, 'absolute fresh official tool output required')
    CRATES.plain(destination.parent, directory=True)
    for path in (archives, Path(lock)):
        need(destination != path and destination not in path.parents and path not in destination.parents,
             'official output overlaps inputs')
    state = destination.with_name(destination.name + '-preparation.json')
    need(not destination.exists() and not destination.is_symlink()
         and not state.exists() and not state.is_symlink(), 'retain old or partial official tool preparation; never reuse')
    destination.mkdir(mode=0o700)
    value = {'schema': 'gatehouse-official-tool-preparation/v1', 'state': 'incomplete',
             'tool_lock_sha256': expected_sha256, 'prepared_roles': []}
    TOOLS.record(state, value)
    try:
        (destination / 'inputs.lock.json').write_bytes(raw)
        copied = destination / 'archives'
        toolchain = destination / 'toolchain'
        copied.mkdir(mode=0o700)
        toolchain.mkdir(mode=0o700)
        compressed, total, names = 0, 0, set()
        for role, row in sorted(rows.items()):
            archive = copied / row['url'].rsplit('/', 1)[1]
            copy_archive(archives / archive.name, archive, row)
            compressed += archive.stat().st_size
            need(compressed <= TOOLS.MAX_TOTAL, 'combined official archives exceed budget')
            payload = archive_inventory(archive, role, row)
            need(not names & payload.keys(), 'colliding official tool payloads')
            names.update(payload)
            total += sum(item['file'][0] for item in payload.values())
            need(total <= MAX_TOTAL and len(names) <= MAX_MEMBERS, 'official tool payload exceeds budget')
            materialize(archive, payload, toolchain)
            value['prepared_roles'].append(role)
            TOOLS.record(state, value)
        need(CRATES.read(lock) == raw, 'tool lock changed during preparation')
        inventory = derive(destination, expected_sha256)
        encoded = canonical(inventory)
        need(len(encoded) <= MAX_INVENTORY, 'official inventory exceeds budget')
        (destination / 'inventory.json').write_bytes(encoded)
        for root, directories, files in os.walk(destination):
            for name in files:
                path = Path(root) / name
                path.chmod(0o555 if path.stat().st_mode & 0o111 else 0o444)
            Path(root).chmod(0o555)
        need(verify(destination, expected_sha256) == inventory, 'final official tool verification differs')
        value['state'] = 'prepared'
        TOOLS.record(state, value)
        return inventory
    except BaseException:
        value['state'] = 'blocked'
        TOOLS.record(state, value)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock', type=Path, default=Path(__file__).with_name('linux-tools.lock.json'))
    parser.add_argument('--lock-sha256', required=True)
    parser.add_argument('--archives', type=Path, required=True)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = prepare(args.lock, args.lock_sha256, args.archives, args.destination)
    except Exception as error:
        parser.exit(1, 'official tool preparation blocked (' + type(error).__name__ + '); retain private partial state\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
