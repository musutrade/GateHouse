"""Separately registered fixed-capacity Linux staging volume; never alter pilot storage."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import uuid

IMAGE = Path('/var/lib/gatehouse/linux.ext4')
MARKER = Path('/var/lib/gatehouse/linux-storage.json')
VOLUME = Path('/srv/gatehouse/linux')
CONTROL = Path('/srv/gatehouse/pilot/control')
SIZE = 16 * 1024**3
HEADROOM = 2 * 1024**3
COMMANDS = {'fallocate', 'mkfs', 'fsck', 'blkid', 'mount', 'findmnt', 'losetup'}
PHASES = {'intent', 'created', 'allocated', 'formatted', 'mounted'}


def tools(B, builder, recipe, pin):
    raw = B.read(recipe, 64 * 1024)
    B.need(hashlib.sha256(raw).hexdigest() == pin, 'volume tool recipe pin differs')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value) and set(value) == {'schema', 'commands', 'files'}
           and value['schema'] == 'gatehouse-volume-tools/v1' and value['commands'].keys() == COMMANDS
           and value['files'].keys() == set(value['commands'].values()), 'volume tool recipe differs')
    for name, expected in value['files'].items():
        B.path_operand(name)
        B.need(hashlib.sha256(builder.host_file(name)).hexdigest() == expected, 'volume management tool changed')
    return value['commands']


def run(B, commands, name, *args, allowed=(0,), timeout=60):
    result = subprocess.run([commands[name], *map(str, args)], env=B.ENV, capture_output=True, timeout=timeout)
    B.need(result.returncode in allowed and len(result.stdout) <= 1024**2 and len(result.stderr) <= 1024**2,
           'volume management operation failed or exceeded budget')
    return result


def probe(B, commands):
    result = run(B, commands, 'blkid', '--probe', '--output', 'export', IMAGE, allowed=(0, 2))
    if result.returncode == 2:
        B.need(not result.stdout and not result.stderr, 'unknown image signature')
        return None
    fields = {}
    for line in result.stdout.decode().splitlines():
        key, value = line.split('=', 1)
        B.need(key not in fields, 'ambiguous filesystem signature')
        fields[key] = value
    B.need(fields.get('TYPE') == 'ext4' and re.fullmatch('[0-9a-f-]{36}', fields.get('UUID', '')),
           'unexpected filesystem signature')
    return fields['UUID']


def mounts(B, commands):
    result = run(B, commands, 'findmnt', '--json', '--submounts', '--mountpoint', VOLUME,
                 '--output', 'TARGET,SOURCE,FSTYPE,UUID,OPTIONS,FSROOT', allowed=(0, 1))
    if result.returncode == 1:
        B.need(not result.stdout and not result.stderr, 'mount status unknown')
        return []
    value = json.loads(result.stdout, object_pairs_hook=B.unique)
    B.need(isinstance(value.get('filesystems'), list) and len(value['filesystems']) == 1,
           'ambiguous mounted target')
    return value['filesystems']


def loops(B, commands):
    result = run(B, commands, 'losetup', '--json', '--list', '--associated', IMAGE,
                 '--output', 'NAME,BACK-FILE,BACK-INO,BACK-MAJ:MIN,OFFSET,SIZELIMIT,RO')
    value = json.loads(result.stdout, object_pairs_hook=B.unique)
    B.need(isinstance(value.get('loopdevices'), list), 'loop backing status unknown')
    return value['loopdevices']


def image(B, record, *, allocated=False):
    path = B.owned(IMAGE, private=True)
    info = path.stat()
    B.need(stat.S_IMODE(info.st_mode) == 0o600 and [info.st_dev, info.st_ino] == record['image_identity']
           and info.st_size in (0, SIZE), 'registered image identity/size differs')
    if allocated:
        B.need(info.st_size == SIZE and info.st_blocks * 512 >= SIZE, 'image reservation is incomplete')
    return info


def capacity(B, info=None):
    values = os.statvfs(IMAGE.parent)
    remaining = SIZE if info is None else max(0, SIZE - info.st_blocks * 512)
    B.need(values.f_bavail * values.f_frsize >= remaining + HEADROOM,
           'reserve image capacity and host filesystem headroom before allocation')


def block_device(path):
    # /dev/loopN is a kernel block device, not a regular protected input file.
    info = Path(path).lstat()
    if not stat.S_ISBLK(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o002:
        raise ValueError('unexpected loop device inode')
    return info.st_rdev


def mounted(B, commands, record, rows):
    B.need(len(rows) == 1, 'registered volume is not mounted')
    row = rows[0]
    options = set(row['options'].split(','))
    B.need(row['target'] == str(VOLUME) and row['fstype'] == 'ext4' and row['uuid'] == record['uuid']
           and row['fsroot'] == '/' and not row.get('children')
           and {'rw', 'nodev', 'nosuid', 'noatime'} <= options
           and not options & {'ro', 'dev', 'suid', 'noexec'}
           and re.fullmatch('/dev/loop[0-9]+', row['source']), 'mount identity/options differ')
    backing = loops(B, commands)
    info = image(B, record, allocated=True)
    capacity(B, info)
    B.need(len(backing) == 1, 'image has ambiguous loop associations')
    loop = backing[0]
    B.need(loop['name'] == row['source'] and loop['back-file'] == str(IMAGE)
           and type(loop['back-ino']) is int and loop['back-ino'] == info.st_ino
           and loop['back-maj:min'] == str(os.major(info.st_dev)) + ':' + str(os.minor(info.st_dev))
           and type(loop['offset']) is int and loop['offset'] == 0
           and type(loop['sizelimit']) is int and loop['sizelimit'] in (0, SIZE) and loop['ro'] is False,
           'loop is not the registered complete image')
    B.need(block_device(row['source']) == VOLUME.stat().st_dev, 'mounted device number differs')
    aliases = json.loads(run(B, commands, 'findmnt', '--json', '--source', row['source'],
                             '--output', 'TARGET,FSROOT').stdout, object_pairs_hook=B.unique)
    B.need(aliases.get('filesystems') == [{'target': str(VOLUME), 'fsroot': '/'}],
           'registered image has another mount alias')
    values = os.statvfs(VOLUME)
    B.need(SIZE * 9 // 10 <= values.f_blocks * values.f_frsize <= SIZE, 'mounted filesystem size differs')


def empty_target(B):
    if VOLUME.exists() or VOLUME.is_symlink():
        B.owned(VOLUME, directory=True, private=True)
        B.need(not any(VOLUME.iterdir()), 'unmounted staging target is occupied')


def admit(B, helpers, manifest, manifest_pin, recipe, recipe_pin):
    """Readonly admission for a caller already holding the controller lease."""
    code = B.authenticate(helpers, manifest, manifest_pin)
    builder = B.load(helpers, 'build-protected-core.py')
    commands = tools(B, builder, recipe, recipe_pin)
    raw = B.read(B.owned(MARKER, private=True), 64 * 1024)
    record = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(record) and record['schema'] == 'gatehouse-linux-storage/v1'
           and record['phase'] == 'mounted' and record['status'] == 'ready' and record['size'] == SIZE
           and record['image'] == str(IMAGE) and record['mountpoint'] == str(VOLUME)
           and record['helper_revision'] == code['revision'] and record['helper_manifest_sha256'] == manifest_pin
           and record['tools_sha256'] == recipe_pin and record['production_gate_enabled'] is False,
           'staging storage is not the reviewed ready registration')
    B.owned(VOLUME, directory=True, private=True)
    mounted(B, commands, record, mounts(B, commands))
    return record


def prepare(B, helpers, manifest, manifest_pin, recipe, recipe_pin, lock_fd=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for volume preparation')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported staging volume platform')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin)
    builder = B.load(helpers, 'build-protected-core.py')
    commands = tools(B, builder, recipe, recipe_pin)
    for parent in (IMAGE.parent, VOLUME.parent, CONTROL):
        B.owned(parent, directory=True)
    B.need(not IMAGE.is_symlink() and not MARKER.is_symlink() and not VOLUME.is_symlink(), 'linked storage target')
    lease = B.owned(CONTROL / 'controller.lock', private=True)
    own_lease = lock_fd is None
    if own_lease:
        lock_fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW)
    try:
        actual, expected = os.fstat(lock_fd), lease.stat()
        B.need((actual.st_dev, actual.st_ino) == (expected.st_dev, expected.st_ino), 'volume lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        builder.pilot_slot(B)
        builder.quiet_units(B)
        builder.idle(B)
        rows = mounts(B, commands)
        if MARKER.exists():
            raw = B.read(B.owned(MARKER, private=True), 64 * 1024)
            record = json.loads(raw, object_pairs_hook=B.unique)
            B.need(raw == B.canonical(record) and record['schema'] == 'gatehouse-linux-storage/v1'
                   and record['phase'] in PHASES and record['size'] == SIZE
                   and record['image'] == str(IMAGE) and record['mountpoint'] == str(VOLUME)
                   and record['helper_revision'] == code['revision']
                   and record['helper_manifest_sha256'] == manifest_pin and record['tools_sha256'] == recipe_pin
                   and record['production_gate_enabled'] is False
                   and record['status'] in ('preparing', 'blocked', 'ready')
                   and str(uuid.UUID(record['uuid'])) == record['uuid'], 'unknown storage registration')
        else:
            B.need(not rows and not IMAGE.exists(), 'unregistered image/mount; retain and reconcile')
            empty_target(B)
            capacity(B)
            record = {'schema': 'gatehouse-linux-storage/v1', 'phase': 'intent', 'status': 'preparing',
                      'image': str(IMAGE), 'mountpoint': str(VOLUME), 'size': SIZE, 'uuid': str(uuid.uuid4()),
                      'helper_revision': code['revision'], 'helper_manifest_sha256': manifest_pin,
                      'tools_sha256': recipe_pin, 'production_gate_enabled': False}
            B.record(MARKER, record)
        try:
            if record['phase'] == 'intent':
                B.need(not rows and not IMAGE.exists(), 'image created without sealed inode; explicit reconciliation required')
                empty_target(B)
                capacity(B)
                fd = os.open(IMAGE, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                try:
                    os.fsync(fd)
                    info = os.fstat(fd)
                    record.update(phase='created', image_identity=[info.st_dev, info.st_ino])
                    B.record(MARKER, record)
                finally:
                    os.close(fd)
            info = image(B, record)
            if record['phase'] in ('created', 'allocated'):
                B.need(not rows and not loops(B, commands), 'unformatted image is mounted or associated')
                empty_target(B)
                signature = probe(B, commands)
                if record['phase'] == 'created':
                    B.need(signature is None, 'unexpected signature before allocation')
                    capacity(B, info)
                    run(B, commands, 'fallocate', '--length', SIZE, IMAGE, timeout=300)
                    image(B, record, allocated=True)
                    record['phase'] = 'allocated'
                    B.record(MARKER, record)
                image(B, record, allocated=True)
                if signature is None:
                    # nodiscard keeps the physical reservation made by fallocate.
                    run(B, commands, 'mkfs', '-t', 'ext4', '-q', '-F', '-U', record['uuid'], '-m', '0', '-E',
                        'nodiscard,lazy_itable_init=0,lazy_journal_init=0', IMAGE, timeout=300)
                B.need(probe(B, commands) == record['uuid'], 'formatted UUID differs')
                image(B, record, allocated=True)
                run(B, commands, 'fsck', '-f', '-n', IMAGE, timeout=300)
                record['phase'] = 'formatted'
                B.record(MARKER, record)
            if not rows:
                B.need(record['phase'] in ('formatted', 'mounted') and not loops(B, commands),
                       'unknown unmounted image state')
                empty_target(B)
                image(B, record, allocated=True)
                B.need(probe(B, commands) == record['uuid'], 'registered filesystem identity differs')
                run(B, commands, 'fsck', '-f', '-n', IMAGE, timeout=300)
                if not VOLUME.exists():
                    VOLUME.mkdir(mode=0o700)
                run(B, commands, 'mount', '--internal-only', '--types', 'ext4', '--options',
                    'loop,nodev,nosuid,noatime', IMAGE, VOLUME)
                rows = mounts(B, commands)
            B.need(record['phase'] in ('formatted', 'mounted'), 'unexpected existing mount')
            mounted(B, commands, record, rows)
            VOLUME.chmod(0o700)
            B.owned(VOLUME, directory=True, private=True)
            B.authenticate(helpers, manifest, manifest_pin)
            tools(B, builder, recipe, recipe_pin)
            builder.pilot_slot(B)
            builder.quiet_units(B)
            builder.idle(B)
            record.pop('failure', None)
            record.update(phase='mounted', status='ready')
            B.record(MARKER, record)
            return record
        except BaseException as error:
            record.update(status='blocked', failure=type(error).__name__)
            B.record(MARKER, record)
            # Never reformat, detach or unmount in exception cleanup.
            raise
    finally:
        if own_lease:
            os.close(lock_fd)
