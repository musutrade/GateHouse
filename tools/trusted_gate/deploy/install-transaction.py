"""Installation barrier primitives; no administrator entry, activation or retirement."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import uuid

BARRIER = Path('/var/lib/gatehouse/installation.json')
RECEIPT = Path('/srv/gatehouse/pilot/control/entry-fence.json')
LEASE = Path('/srv/gatehouse/pilot/control/controller.lock')
CONFIG = Path('/etc/gatehouse/host.json')
ENTRIES = (Path('/opt/gatehouse/bin/gatehouse-control'), Path('/etc/gatehouse/policy/gatehouse-step'))
MAX_FILE = 1024**2


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def identifier(B, transaction, head, configuration_pin):
    value = uuid.UUID(transaction)
    B.need(value.version == 4 and str(value) == transaction and re.fullmatch('[0-9a-f]{40}', head)
           and re.fullmatch('[0-9a-f]{64}', configuration_pin), 'installation generation identity differs')


def lease(B, fd):
    B.need(os.geteuid() == 0, 'administrator identity required for installation fencing')
    path = B.owned(LEASE, private=True)
    actual, expected = os.fstat(fd), path.stat()
    B.need(stat.S_ISREG(actual.st_mode) and actual.st_nlink == 1
           and (actual.st_dev, actual.st_ino) == (expected.st_dev, expected.st_ino), 'installation lease differs')
    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)


def identity(info):
    # fchmod changes ctime. Bytes, mtime, owner, link count and inode still have
    # to match the durable pre-fence intent; only the reviewed mode can change.
    return {key: getattr(info, 'st_' + key) for key in ('dev', 'ino', 'uid', 'gid', 'nlink', 'size', 'mtime_ns')}


def file(B, path, expected_pin, modes):
    path = B.owned(path)
    before = path.stat()
    B.need(stat.S_IMODE(before.st_mode) in modes and before.st_nlink == 1,
           'entry mode or link count differs')
    raw = B.read(path, MAX_FILE)
    B.need(raw and digest(raw) == expected_pin and identity(path.stat()) == identity(before),
           'entry bytes changed')
    return identity(before)


def plan(B, value):
    return {key: value[key] for key in ('transaction', 'head', 'configuration_sha256',
                                      'before_configuration_sha256', 'entries')}


def marker(B, value, phase):
    return {'schema': 'gatehouse-installation-barrier/v1', 'transaction': value['transaction'], 'phase': phase,
            'head': value['head'], 'configuration_sha256': value['configuration_sha256'],
            'plan_sha256': digest(B.canonical(plan(B, value)))}


def validate(B, value):
    B.need(set(value) == {'schema', 'transaction', 'head', 'configuration_sha256', 'before_configuration_sha256',
                          'entries', 'phase', 'controller_drain_proven', 'administrator_ready', 'production_gate_enabled'}
           and value['schema'] == 'gatehouse-entry-fence/v1' and value['phase'] in ('intent', 'fencing', 'fenced')
           and value['controller_drain_proven'] is False and value['administrator_ready'] is False
           and value['production_gate_enabled'] is False, 'entry fence receipt is not a known incomplete installation')
    identifier(B, value['transaction'], value['head'], value['configuration_sha256'])
    B.need(re.fullmatch('[0-9a-f]{64}', value['before_configuration_sha256'])
           and isinstance(value['entries'], dict) and set(value['entries']) == {str(path) for path in ENTRIES},
           'entry fence inventory differs')
    for row in value['entries'].values():
        B.need(set(row) == {'sha256', 'identity'} and re.fullmatch('[0-9a-f]{64}', row['sha256'])
               and set(row['identity']) == {'dev', 'ino', 'uid', 'gid', 'nlink', 'size', 'mtime_ns'}
               and all(type(number) is int and number >= 0 for number in row['identity'].values())
               and row['identity']['uid'] == 0 and row['identity']['nlink'] == 1
               and 0 < row['identity']['size'] <= MAX_FILE, 'entry fence identity differs')
    return value


def read_receipt(B):
    for path in (RECEIPT, BARRIER):
        temporary = path.with_name('.' + path.name + '.tmp')
        B.need(not temporary.exists() and not temporary.is_symlink(),
               'unfinished installation metadata requires reconciliation')
    raw = B.read(B.owned(RECEIPT, private=True), 64 * 1024)
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value) and stat.S_IMODE(RECEIPT.stat().st_mode) == 0o600,
           'entry fence receipt differs')
    return validate(B, value)


def prepare(B, transaction, head, configuration_pin, expected_entries, lease_fd):
    """Internal primitive for a future authenticated coordinator, after full preparation.

    The caller must independently authenticate the complete release/preparation,
    retention, current base and applicability under this same continuous lease.
    There is deliberately no CLI or claim that this primitive admits installation.
    """
    lease(B, lease_fd)
    identifier(B, transaction, head, configuration_pin)
    B.need(set(expected_entries) == {str(path) for path in ENTRIES}, 'exact delegated entry pins required')
    for path in (RECEIPT, BARRIER):
        B.owned(path.parent, directory=True)
        B.need(not path.exists() and not path.is_symlink(), 'retain existing installation transaction')
        temporary = path.with_name('.' + path.name + '.tmp')
        B.need(not temporary.exists() and not temporary.is_symlink(), 'retain unfinished installation metadata')
    entries = {}
    for path in ENTRIES:
        pin = expected_entries[str(path)]
        B.need(isinstance(pin, str) and re.fullmatch('[0-9a-f]{64}', pin), 'entry pin differs')
        entries[str(path)] = {'sha256': pin, 'identity': file(B, path, pin, (0o755,))}
    before = digest(B.read(B.owned(CONFIG, private=True), MAX_FILE))
    value = {'schema': 'gatehouse-entry-fence/v1', 'transaction': transaction, 'head': head,
             'configuration_sha256': configuration_pin, 'before_configuration_sha256': before,
             'entries': entries, 'phase': 'intent', 'controller_drain_proven': False,
             'administrator_ready': False, 'production_gate_enabled': False}
    validate(B, value)
    # Root record() provides file fsync, atomic replace and parent fsync. A crash
    # after the receipt but before the barrier is recoverable without undoing it.
    B.record(RECEIPT, value)
    B.record(BARRIER, marker(B, value, 'fencing'))
    return value


def fence(B, lease_fd):
    """Resume only original pinned inodes; never replace paths or enable entry."""
    lease(B, lease_fd)
    value = read_receipt(B)
    B.need(digest(B.read(B.owned(CONFIG, private=True), MAX_FILE)) == value['before_configuration_sha256'],
           'installed configuration changed before entry fencing')
    barrier_exists = BARRIER.exists() or BARRIER.is_symlink()
    barrier_complete = False
    if barrier_exists:
        raw = B.read(B.owned(BARRIER, private=True), 4096)
        B.need(stat.S_IMODE(BARRIER.stat().st_mode) == 0o600
               and raw in (B.canonical(marker(B, value, 'fencing')), B.canonical(marker(B, value, 'fenced'))),
               'installation barrier is unknown or already advanced')
        B.need(value['phase'] != 'fenced' or raw == B.canonical(marker(B, value, 'fenced')),
               'completed barrier moved backwards')
        barrier_complete = raw == B.canonical(marker(B, value, 'fenced'))
        B.need(value['phase'] != 'intent' or not barrier_complete, 'barrier advanced before fencing intent')
    else:
        B.need(value['phase'] == 'intent', 'installation barrier disappeared')
    # Validate ALL entries before any permission mutation, including recovery.
    modes = (0o755,) if value['phase'] == 'intent' else (0o755, 0o000)
    for path in ENTRIES:
        row = value['entries'][str(path)]
        B.need(file(B, path, row['sha256'], modes) == row['identity'], 'original entry inode differs')
        B.need(not barrier_complete or stat.S_IMODE(path.stat().st_mode) == 0,
               'completed entry fence was reopened')
    if value['phase'] == 'fenced':
        return value
    if barrier_complete:
        # The only forward recovery here is the final receipt that follows the
        # already durable completed barrier; do not rewrite earlier phases.
        lease(B, lease_fd)
        B.need(digest(B.read(B.owned(CONFIG, private=True), MAX_FILE)) == value['before_configuration_sha256'],
               'installed configuration changed during fence recovery')
        value['phase'] = 'fenced'
        B.record(RECEIPT, value)
        return value
    if not barrier_exists:
        B.record(BARRIER, marker(B, value, 'fencing'))
    value['phase'] = 'fencing'
    B.record(RECEIPT, value)
    for path in ENTRIES:
        row = value['entries'][str(path)]
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
            info = os.fstat(stream.fileno())
            raw = stream.read(MAX_FILE + 1)
            B.need(identity(info) == row['identity'] and stat.S_ISREG(info.st_mode)
                   and stat.S_IMODE(info.st_mode) in (0, 0o755)
                   and 0 < len(raw) <= MAX_FILE and digest(raw) == row['sha256']
                   and identity(path.lstat()) == row['identity'],
                   'entry changed before original-inode permission fence')
            if stat.S_IMODE(info.st_mode):
                os.fchmod(stream.fileno(), 0)
                os.fsync(stream.fileno())
            after = os.fstat(stream.fileno())
            B.need(identity(after) == row['identity'] and stat.S_IMODE(after.st_mode) == 0
                   and file(B, path, row['sha256'], (0,)) == row['identity'], 'entry fence not durable or path replaced')
    lease(B, lease_fd)
    B.need(digest(B.read(B.owned(CONFIG, private=True), MAX_FILE)) == value['before_configuration_sha256'],
           'installed configuration changed during entry fencing')
    B.record(BARRIER, marker(B, value, 'fenced'))
    value['phase'] = 'fenced'
    B.record(RECEIPT, value)
    return value


if __name__ == '__main__':
    raise SystemExit('installation fencing is an internal library; no administrator entry is available')
