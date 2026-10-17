"""Capture reviewed Linux data under the retained pilot lease; never install."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat

CONTROL = Path('/srv/gatehouse/pilot/control')
VOLUME = Path('/srv/gatehouse/linux')
BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'
SCOPE = 'reviewed readonly Linux inputs only; no installation or quality acceptance'
MAX_FILE = 512 * 1024**2
MAX_TOTAL = 6 * 1024**3
MAX_MEMBERS = 150000
MAX_METADATA = 64 * 1024**2


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def inventory(B, stage, root):
    files, directories, total = stage.inventory(root)
    derived = {str(parent) for name in files for parent in Path(name).parents if str(parent) != '.'}
    B.need(files and total <= MAX_TOTAL and len(files) <= MAX_MEMBERS and directories == sorted(derived),
           'Linux input inventory has empty/extra directories or exceeds budget')
    for parent, names, leaves in os.walk(root):
        path = B.owned(parent, directory=True)
        B.need(stat.S_IMODE(path.stat().st_mode) == 0o555, 'sealed directory is not readonly')
        for name in leaves:
            path = B.owned(Path(parent) / name)
            B.need(stat.S_IMODE(path.stat().st_mode) == files[str(path.relative_to(root))][2],
                   'sealed file mode differs')
    return {'files': files, 'directories': directories, 'bytes': total}


def copy_tree(B, stage, source, destination):
    """Copy bounded untrusted data first; the fixed reviewed digest comes next."""
    stage.plain_path(source, directory=True)
    destination.mkdir(mode=0o700)
    entries, total = 0, 0
    for parent, directories, files in os.walk(source, followlinks=False):
        parent = stage.plain_path(parent, directory=True)
        directories.sort();files.sort()
        for name in directories:
            child = stage.plain_path(parent / name, directory=True)
            relative = child.relative_to(source)
            stage.safe_path(str(relative))
            (destination / relative).mkdir(mode=0o700)
            entries += 1
            B.need(entries <= MAX_MEMBERS, 'captured input entry budget exceeded')
        for name in files:
            path = parent / name
            relative = path.relative_to(source)
            stage.safe_path(str(relative))
            with _source_file(stage, path) as stream:
                before = os.fstat(stream.fileno())
                total += before.st_size;entries += 1
                B.need(before.st_size <= MAX_FILE and total <= MAX_TOTAL and entries <= MAX_MEMBERS,
                       'captured input byte/entry budget exceeded')
                fd = os.open(destination / relative, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, 'wb') as output:
                    size = 0
                    while chunk := stream.read(1024**2):
                        size += len(chunk)
                        B.need(size <= before.st_size, 'captured input grew while copying')
                        output.write(chunk)
                    B.need(size == before.st_size and identity(before) == identity(os.fstat(stream.fileno()))
                           == identity(path.lstat()), 'captured input changed while copying')
                    output.flush();os.fsync(output.fileno())
                    os.fchmod(output.fileno(), 0o555 if before.st_mode & 0o111 else 0o444)
    return total


def identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _source_file(stage, path):
    stage.plain_path(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    stream = os.fdopen(fd, 'rb')
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_mode & 0o7000 or info.st_size > MAX_FILE:
        stream.close()
        raise ValueError('special, linked, privileged or oversized captured input')
    return stream


def source_manifest(B, source, head):
    lock = B.frozen_lock(source, head, BASE)
    path = B.owned(source / 'test-data/manifest.json')
    raw = B.read(path, 4 * 1024**2)
    B.need(B.git(source, 'show', head + ':test-data/manifest.json') == raw,
           'test-data manifest differs from protected Git source')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(value['schema'] == 'external-test-data/v1' and isinstance(value['archives'], list)
           and 0 < len(value['archives']) <= 1024, 'invalid source test-data manifest')
    total, expected = 0, {}
    for row in value['archives']:
        checksum, size = row['archive_sha256'], row['archive_size']
        B.need(isinstance(checksum, str) and re.fullmatch('[0-9a-f]{64}', checksum)
               and type(size) is int and 0 < size <= MAX_FILE and checksum not in expected,
               'invalid source test-data archive pin')
        total += size
        B.need(total <= MAX_TOTAL, 'source test-data archives exceed budget')
        expected[checksum] = size
    return lock, raw, expected


def seal(B, helpers, manifest, manifest_pin, source, head, inputs, input_pin,
         archives, onboarding, recipe, recipe_pin, lock_fd=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for Linux input sealing')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin)
    B.need(isinstance(head, str) and re.fullmatch('[0-9a-f]{40}', head) and code['revision'] == head,
           'Linux input source differs from authenticated helper revision')
    helpers, source, inputs, archives, onboarding, recipe = map(B.path_operand,
        (helpers, source, inputs, archives, onboarding, recipe))
    builder = B.load(helpers, 'build-protected-core.py')
    volume = B.load(helpers, 'prepare-linux-volume.py')
    stage = B.load(helpers, 'stage-linux-inputs.py')
    directory = B.load(helpers, 'prepare-locked-directory.py')
    destination = VOLUME / 'bootstrap-linux-inputs'
    B.need(all(destination != path and destination not in path.parents and path not in destination.parents
               for path in (helpers, Path(manifest), source, inputs, archives, onboarding, recipe)),
           'Linux input output overlaps an input')
    lease = B.owned(CONTROL / 'controller.lock', private=True)
    own_lease = lock_fd is None
    if own_lease:
        lock_fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW)
    try:
        actual, expected = os.fstat(lock_fd), lease.stat()
        B.need(stat.S_ISREG(actual.st_mode) and (actual.st_dev, actual.st_ino) ==
               (expected.st_dev, expected.st_ino), 'Linux input lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        def guards():
            builder.pilot_slot(B);builder.quiet_units(B);builder.idle(B)
            volume.admit(B, helpers, manifest, manifest_pin, recipe, recipe_pin)
            B.owned(VOLUME, directory=True, private=True)
        guards()
        B.need(not destination.exists() and not destination.is_symlink(), 'retain previous Linux capture; never reuse')
        lock, manifest_raw, expected_archives = source_manifest(B, source, head)
        imported = builder.imported_inputs(B, inputs, input_pin, source, head, BASE, helpers)
        B.need(imported['lock_sha256'] == digest(lock), 'imported dependency lock differs')
        destination.mkdir(mode=0o700)
        status = destination / 'seal.json'
        state = {'schema': 'gatehouse-linux-input-seal/v1', 'state': 'incomplete', 'head': head, 'base': BASE,
                 'helper_manifest_sha256': manifest_pin, 'input_binding_sha256': input_pin,
                 'administrator_ready': False, 'production_gate_enabled': False}
        B.record(status, state)
        try:
            captured_archives = destination / 'test-data-archives';captured_archives.mkdir(mode=0o700)
            for checksum, size in sorted(expected_archives.items()):
                name = checksum + '.tar.gz'
                directory.copy_archive(archives / name, captured_archives / name, checksum, size)
                B.need((captured_archives / name).stat().st_size == size, 'captured archive size differs')
            captured_onboarding = destination / 'onboarding'
            copy_tree(B, stage, onboarding, captured_onboarding)
            B.need(stage.JOB.directory_digest(captured_onboarding) == stage.JOB.ONBOARDING_SHA256,
                   'captured onboarding differs from independently reviewed source')
            for path in (captured_archives, captured_onboarding):B.readonly(path)
            roles = {'directory_sources': inputs / 'dependencies/sources',
                     'test_data_archives': captured_archives, 'onboarding': captured_onboarding}
            rows = {role: inventory(B, stage, path) for role, path in roles.items()}
            B.need(sum(row['bytes'] for row in rows.values()) <= MAX_TOTAL
                   and sum(len(row['files']) for row in rows.values()) <= MAX_MEMBERS,
                   'combined Linux inputs exceed budget')
            metadata = {'schema': 'gatehouse-linux-inputs/v1', 'scope': SCOPE, 'roles': rows,
                        'lock_sha256': digest(lock), 'test_data_manifest_sha256': digest(manifest_raw),
                        'onboarding_source_sha256': stage.JOB.ONBOARDING_SHA256}
            raw = B.canonical(metadata)
            B.need(len(raw) <= MAX_METADATA, 'Linux input metadata exceeds budget')
            metadata_path = destination / 'linux-inputs.json';B.write(metadata_path, raw, 0o444)
            cargo = destination / 'cargo-config.toml'
            cargo_raw = b'[source.crates-io]\nreplace-with = "gatehouse-locked"\n' \
                b'[source.gatehouse-locked]\ndirectory = "/work/dependencies/sources"\n[net]\noffline = true\n'
            B.write(cargo, cargo_raw, 0o444)
            B.need(source_manifest(B, source, head) == (lock, manifest_raw, expected_archives),
                   'frozen source changed during Linux capture')
            B.need(builder.imported_inputs(B, inputs, input_pin, source, head, BASE, helpers) == imported,
                   'imported inputs changed during Linux capture')
            B.need({role:inventory(B, stage, path) for role,path in roles.items()} == rows
                   and stage.JOB.directory_digest(captured_onboarding) == stage.JOB.ONBOARDING_SHA256
                   and B.read(metadata_path, MAX_METADATA) == raw and B.read(cargo, 1024**2) == cargo_raw,
                   'captured Linux inputs changed before sealing')
            B.authenticate(helpers, manifest, manifest_pin);guards()
            result = {**state, 'state': 'sealed', 'helper_revision': code['revision'],
                      'linux_inputs': {'inventory': str(metadata_path), **{role:str(path) for role,path in roles.items()}},
                      'cargo_config': str(cargo), 'protected_files': {str(metadata_path):digest(raw), str(cargo):digest(cargo_raw)},
                      'files': sum(len(row['files']) for row in rows.values()),
                      'bytes': sum(row['bytes'] for row in rows.values()),
                      'scope': 'sealed input facts only; not complete host configuration, installation or quality acceptance'}
            B.write(destination / 'role-binding.json', B.canonical(result), 0o400)
            state = {**state, 'state': 'sealed', 'binding_sha256': digest(B.canonical(result))}
            B.record(status, state)
            return result
        except BaseException:
            state['state'] = 'blocked';B.record(status, state)
            raise
    finally:
        if own_lease:
            os.close(lock_fd)
