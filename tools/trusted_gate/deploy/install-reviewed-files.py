"""Internal disabled-entry file transaction; no administrator CLI or activation.

An authenticated coordinator must supply source-bound materials and repeat live
remote/publication/journal/storage/process/generation admission at every checkpoint.
These primitives grant no such admission and never enable delegated entries.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat

RECEIPT = Path('/srv/gatehouse/pilot/control/file-install.json')
CORE = Path('/opt/gatehouse/bin/harness-gate')
CONFIG = Path('/etc/gatehouse/host.json')
ENTRIES = (Path('/opt/gatehouse/bin/gatehouse-control'), Path('/etc/gatehouse/policy/gatehouse-step'))
DIRECTORIES = {Path('/opt/gatehouse/linux-test'): 0o755,
               Path('/srv/gatehouse/linux/scratch'): 0o700, Path('/srv/gatehouse/linux/cache'): 0o700}
MAX_FILE = 128 * 1024**2
MAX_TOTAL = 384 * 1024**2
MAX_RECEIPT = 256 * 1024


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


def materialize(B, contract, payload, head, old_core, core, old_raw, new_raw):
    """Pure byte mapping; caller must bind release/Git/capture/preparation provenance."""
    manifest = payload['helper-manifest.json'];code = json.loads(manifest, object_pairs_hook=B.unique)
    contract.validate(B, payload, code, manifest)
    B.need(head == code['revision'], 'file materials belong to another source revision')
    old = json.loads(old_raw, object_pairs_hook=B.unique);new = json.loads(new_raw, object_pairs_hook=B.unique)
    B.need(new_raw == B.canonical(new) and new['environment']['source_revision'] == head
           and old['protected_files'][str(CORE)] == digest(old_core)
           and new['protected_files'][str(CORE)] == digest(core), 'Core/configuration material pins differ')
    previous = {target: (name, mode) for name, (target, mode, action) in contract.old_source_map().items()}
    files = {}
    for name, (target, mode, action) in contract.source_map().items():
        before = payload['before/' + previous[target][0]] if target in previous else None
        before_mode = previous[target][1] if target in previous else None
        after = payload['files/' + name]
        if Path(target) in ENTRIES:mode = before_mode = 0
        B.need(action == 'retain' or new['protected_files'][target] == digest(after), 'new source file pin differs')
        files[target] = {'before': before, 'before_mode': before_mode, 'after': after, 'mode': mode, 'action': action}
    files[str(CORE)] = {'before': old_core, 'before_mode': 0o755, 'after': core, 'mode': 0o555, 'action': 'replace'}
    files[str(CONFIG)] = {'before': old_raw, 'before_mode': 0o600, 'after': new_raw, 'mode': 0o600, 'action': 'replace'}
    result = {'head': head, 'files': files};descriptor(B, contract, result)
    return result


def descriptor(B, contract, materials):
    B.need(set(materials) == {'head', 'files'} and isinstance(materials['head'], str)
           and re.fullmatch('[0-9a-f]{40}', materials['head']), 'file material identity differs')
    expected = {target: (0 if Path(target) in ENTRIES else mode, action)
                for target, mode, action in contract.source_map().values()}
    expected.update({str(CORE): (0o555, 'replace'), str(CONFIG): (0o600, 'replace')})
    B.need(len(expected) == 27 and materials['files'].keys() == expected.keys(), 'exact 27-file material inventory required')
    previous = {target: 0 if Path(target) in ENTRIES else mode
                for target, mode, action in contract.old_source_map().values()}
    previous.update({str(CORE): 0o755, str(CONFIG): 0o600})
    rows, total = {}, 0
    for target, row in materials['files'].items():
        B.path_operand(target)
        B.need(set(row) == {'before', 'before_mode', 'after', 'mode', 'action'}
               and type(row['mode']) is int and (row['mode'], row['action']) == expected[target]
               and type(row['after']) is bytes, 'file material fields differ')
        before = row['before'];limit = MAX_FILE if Path(target) == CORE else 1024**2
        B.need(0 < len(row['after']) <= limit and
               ((type(before) is bytes and 0 < len(before) <= limit and type(row['before_mode']) is int
                 and row['before_mode'] == previous[target]) if target in previous
                else before is None and row['before_mode'] is None), 'file material before state differs')
        B.need(row['action'] != 'retain' or before == row['after'] and row['before_mode'] == row['mode'],
               'retained installation input changed')
        total += len(row['after']) + (len(before) if before is not None else 0)
        B.need(total <= MAX_TOTAL, 'file material budget exceeded')
        rows[target] = {'before_sha256': digest(before) if before is not None else None,
            'before_mode': row['before_mode'], 'after_sha256': digest(row['after']), 'mode': row['mode'],
            'action': row['action'], 'bytes': len(row['after'])}
    return {'head': materials['head'], 'files': rows, 'directories': {str(p): mode for p, mode in DIRECTORIES.items()}}


def identity(info):
    return {key: getattr(info, 'st_' + key) for key in (
        'dev', 'ino', 'uid', 'gid', 'nlink', 'size', 'mode', 'mtime_ns', 'ctime_ns')}


def directory_identity(info):
    return {key: getattr(info, 'st_' + key) for key in ('dev', 'ino', 'uid', 'gid', 'mode')}


def probe(B, path, raw, mode):
    if raw is None:
        B.need(not path.exists() and not path.is_symlink(), 'unrecorded installed file appeared')
        return None
    path = B.owned(path);before = identity(path.stat())
    B.need(stat.S_IMODE(before['mode']) == mode and B.read(path, len(raw)) == raw
           and identity(path.stat()) == before, 'installed file bytes/mode changed')
    return before


def temporary(path, transaction):
    return path.with_name('.' + path.name + '.gatehouse-' + transaction + '.new')


def file_order(materials):
    last = [str(CORE), str(CONFIG), *map(str, reversed(ENTRIES))]
    return sorted(materials['files'].keys() - set(last)) + last


def sync_parent(path):
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:os.fsync(fd)
    finally:os.close(fd)


def metadata(B, path):
    pending = path.with_name('.' + path.name + '.tmp')
    B.need(not pending.exists() and not pending.is_symlink(), 'retain unfinished file transaction metadata')
    raw = B.read(B.owned(path, private=True), MAX_RECEIPT)
    B.need(stat.S_IMODE(path.stat().st_mode) == 0o600, 'file transaction metadata mode differs')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value), 'file transaction metadata is not canonical')
    return value


def barrier(B, fence, original, lease_fd):
    fence.lease(B, lease_fd)
    same(B, fence.read_receipt(B), original, 'original entry fence receipt changed')
    B.need(original['phase'] == 'fenced', 'original entry fence was not completed')
    path = fence.BARRIER
    B.need(stat.S_IMODE(B.owned(path, private=True).stat().st_mode) == 0o600
           and B.read(path, 4096) == B.canonical(fence.marker(B, original, 'fenced')),
           'disabled installation barrier changed or advanced')


def inspect(B, fence, contract, materials, state, original, lease_fd, fresh_temporary=None):
    """Validate the whole known generation before every mutation, including retry."""
    plan = descriptor(B, contract, materials);barrier(B, fence, original, lease_fd)
    B.need(set(state) == {'schema', 'head', 'transaction', 'material_sha256', 'fence_sha256', 'phase',
                          'before', 'directories_done', 'files_done', 'inflight', 'entries_enabled',
                          'administrator_ready', 'production_gate_enabled'}
           and state['schema'] == 'gatehouse-file-install/v1' and state['head'] == materials['head'] == original['head']
           and state['transaction'] == original['transaction'] and state['phase'] in ('intent', 'installing', 'installed-disabled')
           and state['material_sha256'] == digest(B.canonical(plan))
           and state['fence_sha256'] == digest(B.canonical(original)) and state['entries_enabled'] is False
           and state['administrator_ready'] is False and state['production_gate_enabled'] is False,
           'file transaction identity or phase differs')
    B.need(original['configuration_sha256'] == plan['files'][str(CONFIG)]['after_sha256']
           and original['before_configuration_sha256'] == plan['files'][str(CONFIG)]['before_sha256'],
           'file configuration differs from fenced generation')
    directory_order = sorted(map(str, DIRECTORIES));order = file_order(materials)
    for key, expected in (('directories_done', directory_order), ('files_done', order)):
        rows = state[key]
        B.need(isinstance(rows, list) and len(rows) <= len(expected)
               and all(isinstance(row, dict) and set(row) == {'path', 'identity'} for row in rows)
               and [row['path'] for row in rows] == expected[:len(rows)], 'file transaction progress is not an ordered prefix')
    B.need(state['before'].keys() == materials['files'].keys(), 'file transaction baseline inventory differs')
    dirs_done = {row['path']: row['identity'] for row in state['directories_done']}
    done = {row['path']: row['identity'] for row in state['files_done']}
    B.need(not done or len(dirs_done) == len(directory_order), 'file installation preceded directory preparation')
    remaining_kind = 'directory' if len(dirs_done) < len(directory_order) else 'file'
    remaining = directory_order[len(dirs_done):] if remaining_kind == 'directory' else order[len(done):]
    pending = state['inflight']
    B.need(pending is None or bool(remaining) and pending == {'kind': remaining_kind, 'path': remaining[0]},
           'file transaction inflight intent differs')
    B.need(state['phase'] != 'intent' or not dirs_done and not done and pending is None, 'initial file transaction already advanced')
    B.need(state['phase'] != 'installed-disabled' or not remaining and pending is None, 'installed file transaction is incomplete')
    for path, mode in DIRECTORIES.items():
        exists = path.exists() or path.is_symlink()
        if str(path) in dirs_done:
            info = B.owned(path, directory=True).stat()
            B.need(stat.S_IMODE(info.st_mode) == mode, 'installed directory mode changed')
            same(B, directory_identity(info), dirs_done[str(path)], 'installed directory identity changed')
        elif pending == {'kind': 'directory', 'path': str(path)} and exists:
            info = B.owned(path, directory=True).stat()
            B.need(stat.S_IMODE(info.st_mode) in (0o700, mode) and not any(path.iterdir()),
                   'interrupted directory creation is unknown')
        else:
            B.need(not exists, 'unrecorded installation directory appeared')
        if exists:
            allowed = {Path(target).name for target in materials['files'] if Path(target).parent == path}
            if fresh_temporary is not None and Path(fresh_temporary['path']).parent == path:
                allowed.add(Path(fresh_temporary['path']).name)
            B.need({p.name for p in path.iterdir()} <= allowed, 'unknown file in installation directory')
    fresh_seen = False
    for target, row in materials['files'].items():
        path = Path(target);temp = temporary(path, state['transaction'])
        if fresh_temporary is not None and fresh_temporary['path'] == str(temp):
            B.need(pending == {'kind': 'file', 'path': target}, 'fresh temporary has no matching intent')
            same(B, probe(B, temp, row['after'], row['mode']), fresh_temporary['identity'], 'fresh temporary changed')
            fresh_seen = True
        else:
            B.need(not temp.exists() and not temp.is_symlink(), 'retain partial installed-file temporary for reconciliation')
        baseline = state['before'][target]
        if target in done:
            actual = probe(B, path, row['after'], row['mode'])
            same(B, actual, done[target], 'completed installed file identity changed')
            if row['action'] == 'retain':same(B, actual, baseline, 'retained input inode changed')
        elif pending == {'kind': 'file', 'path': target} and row['action'] != 'retain':
            # A durable intent permits only its original inode or exact intended
            # new bytes/mode after rename, never another pending file's state.
            try:actual = probe(B, path, row['before'], row['before_mode']);same(B, actual, baseline, 'before inode changed')
            except (OSError, ValueError):probe(B, path, row['after'], row['mode'])
        else:
            actual = probe(B, path, row['before'], row['before_mode'])
            same(B, actual, baseline, 'pending installed file identity changed')
    for path in ENTRIES:
        B.need(stat.S_IMODE(B.owned(path).stat().st_mode) == 0, 'delegated entry was enabled during file installation')
        target = str(path);baseline = state['before'][target]
        same(B, {key: baseline[key] for key in original['entries'][target]['identity']},
             original['entries'][target]['identity'], 'file baseline differs from original fenced inode')
        B.need(plan['files'][target]['before_sha256'] == original['entries'][target]['sha256'],
               'file baseline differs from original fenced bytes')
    B.need(fresh_temporary is None or fresh_seen, 'unexpected fresh temporary proof')
    return plan


def checkpoint(B, fence, contract, materials, state, original, lease_fd, guard, fresh_temporary=None):
    B.need(callable(guard) and guard() is True, 'coordinator checkpoint did not admit the next file transition')
    same(B, metadata(B, RECEIPT), state, 'durable file transaction changed')
    return inspect(B, fence, contract, materials, state, original, lease_fd, fresh_temporary)


def prepare(B, fence, contract, materials, lease_fd, guard):
    """Caller must authenticate all preparation and live admission under this lease."""
    fence.lease(B, lease_fd);descriptor(B, contract, materials)
    B.need(callable(guard) and guard() is True, 'coordinator checkpoint did not admit file preparation')
    original = fence.fenced(B, lease_fd)
    B.owned(RECEIPT.parent, directory=True, private=True)
    for path in (RECEIPT, RECEIPT.with_name('.' + RECEIPT.name + '.tmp')):
        B.need(not path.exists() and not path.is_symlink(), 'retain existing file transaction')
    before = {target: probe(B, Path(target), row['before'], row['before_mode'])
              for target, row in materials['files'].items()}
    for path in DIRECTORIES:
        B.owned(path.parent, directory=True)
        B.need(not path.exists() and not path.is_symlink(), 'fresh installation directory required')
    state = {'schema': 'gatehouse-file-install/v1', 'head': materials['head'], 'transaction': original['transaction'],
        'material_sha256': digest(B.canonical(descriptor(B, contract, materials))),
        'fence_sha256': digest(B.canonical(original)), 'phase': 'intent', 'before': before,
        'directories_done': [], 'files_done': [], 'inflight': None, 'entries_enabled': False,
        'administrator_ready': False, 'production_gate_enabled': False}
    B.need(guard() is True, 'coordinator checkpoint did not admit file preparation')
    inspect(B, fence, contract, materials, state, original, lease_fd)
    B.need(not RECEIPT.exists() and not RECEIPT.is_symlink(), 'file transaction appeared during preparation')
    B.record(RECEIPT, state)
    checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
    return state


def resume(B, fence, contract, materials, lease_fd, guard):
    """Forward-only replacement; complete state remains disabled and unaccepted."""
    fence.lease(B, lease_fd);original = fence.read_receipt(B);state = metadata(B, RECEIPT)
    checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
    if state['phase'] == 'installed-disabled':return state
    if state['phase'] == 'intent':
        state['phase'] = 'installing';B.record(RECEIPT, state)
    for target in sorted(map(str, DIRECTORIES)):
        if target in {row['path'] for row in state['directories_done']}:continue
        checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
        state['inflight'] = {'kind': 'directory', 'path': target};B.record(RECEIPT, state)
        checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
        path = Path(target);mode = DIRECTORIES[path]
        if not path.exists():path.mkdir(mode=0o700)
        fd = os.open(B.owned(path, directory=True), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:os.fchmod(fd, mode);os.fsync(fd)
        finally:os.close(fd)
        sync_parent(path)
        state['directories_done'].append({'path': target, 'identity': directory_identity(path.stat())})
        state['inflight'] = None;B.record(RECEIPT, state)
    for target in file_order(materials):
        if target in {row['path'] for row in state['files_done']}:continue
        checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
        state['inflight'] = {'kind': 'file', 'path': target};B.record(RECEIPT, state)
        checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
        path = Path(target);row = materials['files'][target]
        try:actual = probe(B, path, row['after'], row['mode'])
        except (OSError, ValueError):
            B.owned(path.parent, directory=True)
            temp = temporary(path, state['transaction'])
            B.write(temp, row['after'], row['mode'])
            fresh = {'path': str(temp), 'identity': probe(B, temp, row['after'], row['mode'])}
            checkpoint(B, fence, contract, materials, state, original, lease_fd, guard, fresh)
            # Only this newly written, fully rechecked temporary may be renamed
            # in this call. A retained temporary on a later entry always blocks.
            os.replace(temp, path)
            actual = probe(B, path, row['after'], row['mode'])
        # A prior process may have renamed this file and died before syncing
        # its directory. Persist that rename before recording it as completed.
        sync_parent(path)
        state['files_done'].append({'path': target, 'identity': actual})
        state['inflight'] = None;B.record(RECEIPT, state)
    checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
    state['phase'] = 'installed-disabled';B.record(RECEIPT, state)
    checkpoint(B, fence, contract, materials, state, original, lease_fd, guard)
    return state


if __name__ == '__main__':
    raise SystemExit('reviewed file replacement is an internal library; no administrator entry is available')
