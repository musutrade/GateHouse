"""Private fixed-659 preservation; no revocation, retirement or installation."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import time

VOLUME = Path('/srv/gatehouse/pilot')
CONFIG = Path('/etc/gatehouse/host.json')
MARKER = Path('/var/lib/gatehouse/pilot-install.json')
HEAD = '659803a2f144926997e56d072c7fe8760d6ed176'
RUN = 'ce86a2a4-ff18-435f-bb5d-aafc313ceca6'
RELEASE = '04190f57c76f9625be1e805ab212c48802181f458fecdc34f5478f796efd77ef'
MAX_DB = 64 * 1024**2
MAX_FILE = 256 * 1024**2
MAX_TOTAL = 1024**3
MAX_ENTRIES = 30000
MAX_ROWS = 50000
MAX_MANIFEST = 16 * 1024**2
TABLES = {
    'runs': ('id', 'repository', 'head', 'epoch', 'input', 'local', 'evidence', 'remote',
             'check_id', 'pr', 'reason', 'created'),
    'events': ('sequence', 'run', 'time', 'operation', 'detail'),
}


def connect(path):
    database = sqlite3.connect(Path(path).as_uri() + '?mode=ro', uri=True, timeout=1)
    database.execute('PRAGMA trusted_schema=OFF')
    database.execute('PRAGMA query_only=ON')
    return database


def database_identity(B, database):
    B.need(database.execute('PRAGMA integrity_check').fetchall() == [('ok',)], 'journal integrity differs')
    schema = database.execute('SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name').fetchall()
    B.need({row[1] for row in schema if row[0] == 'table'} == set(TABLES)
           and all(row[0] in ('table', 'index') and row[2] in TABLES for row in schema),
           'unknown journal schema')
    identity, rows, size = {}, {}, 0
    for table, columns in TABLES.items():
        B.need(tuple(row[1] for row in database.execute('PRAGMA table_info(' + table + ')')) == columns,
               'journal columns differ')
        values = []
        for row in database.execute('SELECT * FROM ' + table + ' ORDER BY ' + columns[0]):
            size += len(B.canonical(row))
            B.need(size <= MAX_DB and len(values) < MAX_ROWS, 'journal logical snapshot exceeds budget')
            values.append(row)
        rows[table] = values
        identity[table] = {'rows': len(values), 'sha256': hashlib.sha256(B.canonical(values)).hexdigest()}
    identity['schema_sha256'] = hashlib.sha256(B.canonical(schema)).hexdigest()
    return identity, rows


def old_run(B, rows):
    candidates = [dict(zip(TABLES['runs'], row)) for row in rows['runs']
                  if row[TABLES['runs'].index('remote')] not in ('merged', 'revoked')]
    B.need(len(candidates) == 1, 'old pilot no longer owns the sole slot')
    run = candidates[0]
    B.need(run['id'] == RUN and run['head'] == HEAD and run['repository'] == 'musutrade/GateHouse'
           and run['local'] in ('passed', 'failed') and run['remote'] == 'pending'
           and run['pr'] is None and run['check_id'] is None, 'accepted pilot identity/state differs')
    return run


def journal_snapshot(B, journal, destination):
    """SQLite backup includes committed WAL pages; never copy the main file alone."""
    for path in (journal, journal.with_name(journal.name + '-wal'), journal.with_name(journal.name + '-shm')):
        if path.exists() or path.is_symlink():
            B.owned(path, private=True)
            B.need(path.stat().st_size <= MAX_DB, 'journal physical file exceeds budget')
    B.owned(journal, private=True)
    B.write(destination, b'', 0o600)
    source, target = connect(journal), sqlite3.connect(destination)
    deadline = time.monotonic() + 30
    page_size = source.execute('PRAGMA page_size').fetchone()[0]
    source.set_progress_handler(lambda: int(time.monotonic() > deadline), 10000)
    target.set_progress_handler(lambda: int(time.monotonic() > deadline), 10000)
    def progress(status, remaining, total):
        B.need(time.monotonic() <= deadline and total * page_size <= MAX_DB,
               'journal backup exceeds time/page budget')
    try:
        source.execute('BEGIN')
        identity, rows = database_identity(B, source)
        run = old_run(B, rows)
        source.backup(target, pages=128, progress=progress, sleep=0.01)
        B.need(target.execute('PRAGMA journal_mode=DELETE').fetchone() == ('delete',),
               'retained database must be self-contained')
        B.need(database_identity(B, target)[0] == identity, 'journal backup differs from its transaction')
    finally:
        target.close()
        source.close()
    destination.chmod(0o400)
    B.need(destination.stat().st_size <= MAX_DB, 'journal backup exceeds budget')
    with destination.open('rb') as stream:
        os.fsync(stream.fileno())
    return identity, run


def file_identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def inventory(B, roots, output=None):
    """Walk without links; private copies retain all raw reports, including failures."""
    files, directories, total, entries = {}, {}, 0, 0
    for label, root in roots.items():
        B.owned(root, directory=True)
        for parent, names, leaves in os.walk(root):
            names.sort(); leaves.sort()
            parent = Path(parent)
            B.owned(parent, directory=True)
            relative = Path(label) / parent.relative_to(root)
            directories[str(relative)] = stat.S_IMODE(parent.stat().st_mode)
            entries += 1
            B.need(entries <= MAX_ENTRIES, 'preservation entry budget exceeded')
            if output is not None:
                (output / relative).mkdir(mode=0o700, parents=True, exist_ok=True)
            for name in names:
                B.owned(parent / name, directory=True)
            for name in leaves:
                if label == 'control' and parent == root and name in (
                        'controller.lock', 'journal.sqlite3', 'journal.sqlite3-wal', 'journal.sqlite3-shm'):
                    continue
                path = B.owned(parent / name)
                before = path.lstat()
                total += before.st_size
                entries += 1
                B.need(before.st_size <= MAX_FILE and total <= MAX_TOTAL and entries <= MAX_ENTRIES,
                       'preservation byte/entry budget exceeded')
                raw = B.read(path, MAX_FILE)
                B.need(file_identity(before) == file_identity(path.lstat()), 'preservation source changed')
                key = str(relative / name)
                files[key] = [len(raw), hashlib.sha256(raw).hexdigest(), stat.S_IMODE(before.st_mode)]
                if output is not None:
                    B.write(output / key, raw, 0o400)
    return {'files': files, 'directories': directories, 'bytes': total}


def verify_copy(B, root, expected):
    seen_files, seen_directories = set(), set()
    for parent, directories, files in os.walk(root):
        parent = Path(parent)
        B.owned(parent, directory=True, private=True)
        if parent != root:
            seen_directories.add(str(parent.relative_to(root)))
        for name in directories:
            B.owned(parent / name, directory=True, private=True)
        for name in files:
            path = B.owned(parent / name, private=True)
            B.need(stat.S_IMODE(path.stat().st_mode) == 0o400, 'retained file is not sealed')
            key = str(path.relative_to(root))
            B.need(key in expected['files'], 'unexpected retained file')
            raw = B.read(path, MAX_FILE)
            B.need([len(raw), hashlib.sha256(raw).hexdigest()] == expected['files'][key][:2],
                   'retained file differs')
            seen_files.add(key)
    B.need(seen_files == expected['files'].keys() and seen_directories == set(expected['directories']),
           'partial retained tree')


def verify(B, destination, manifest_pin):
    """A future upgrader must pin and recheck retention before destructive work."""
    destination = B.owned(B.path_operand(destination), directory=True, private=True)
    raw = B.read(B.owned(destination / 'preservation.json', private=True), MAX_MANIFEST)
    B.need(hashlib.sha256(raw).hexdigest() == manifest_pin, 'retention manifest pin differs')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value) and value['schema'] == 'gatehouse-pilot-preservation/v1'
           and value['state'] == 'preserved' and value['head'] == HEAD and value['run'] == RUN
           and value['production_gate_enabled'] is False, 'retention is incomplete or belongs to another pilot')
    metadata = {'host.json', 'marker.json', 'acceptance.json'}
    B.need(value['metadata_sha256'].keys() == metadata and {p.name for p in destination.iterdir()} ==
           metadata | {'preservation.json', 'journal.sqlite3', 'tree'}, 'retention inventory differs')
    for name in metadata:
        path = B.owned(destination / name, private=True)
        B.need(stat.S_IMODE(path.stat().st_mode) == 0o400 and
               hashlib.sha256(B.read(path, 1024**2)).hexdigest() == value['metadata_sha256'][name],
               'retained metadata changed')
    saved = B.owned(destination / 'journal.sqlite3', private=True)
    B.need(stat.S_IMODE(saved.stat().st_mode) == 0o400 and
           hashlib.sha256(B.read(saved, MAX_DB)).hexdigest() == value['journal_sha256'], 'retained database changed')
    database = connect(saved)
    try:
        identity, rows = database_identity(B, database)
        B.need(identity == value['journal'], 'retained journal identity differs')
        old_run(B, rows)
    finally:
        database.close()
    verify_copy(B, destination / 'tree', value['inventory'])
    return value


def preserve(B, helpers, manifest, manifest_pin, destination, lock_fd=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for preservation')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin)
    destination = B.path_operand(destination)
    B.owned(destination.parent, directory=True, private=True)
    B.need(destination != VOLUME and VOLUME not in destination.parents and destination not in VOLUME.parents
           and not destination.exists() and not destination.is_symlink(), 'fresh separate preservation output required')
    B.need(all(destination != path and destination not in path.parents and path not in destination.parents
               for path in (Path(helpers), Path(manifest), CONFIG, MARKER)), 'preservation overlaps a protected input')
    control = VOLUME / 'control'
    B.owned(control, directory=True, private=True)
    lease = B.owned(control / 'controller.lock', private=True)
    own_lease = lock_fd is None
    if own_lease:
        lock_fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW)
    try:
        info, expected = os.fstat(lock_fd), lease.stat()
        B.need((info.st_dev, info.st_ino) == (expected.st_dev, expected.st_ino), 'preservation lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        builder = B.load(helpers, 'build-protected-core.py')
        builder.quiet_units(B)
        builder.idle(B)
        destination.mkdir(mode=0o700)
        state = {'schema': 'gatehouse-pilot-preservation/v1', 'state': 'incomplete', 'head': HEAD, 'run': RUN,
                 'helper_revision': code['revision'], 'helper_manifest_sha256': manifest_pin,
                 'production_gate_enabled': False, 'scope': 'private state preservation only; no quality or retirement authorization'}
        status = destination / 'preservation.json'
        B.record(status, state)
        try:
            identity, run = journal_snapshot(B, control / 'journal.sqlite3', destination / 'journal.sqlite3')
            def metadata():
                return {name: B.read(B.owned(path, private=True), 1024**2) for name, path in {
                    'host.json': CONFIG, 'marker.json': MARKER, 'acceptance.json': control / 'pilot-acceptance.json'}.items()}
            raw = metadata()
            config, marker, accepted = (json.loads(raw[name], object_pairs_hook=B.unique)
                                        for name in ('host.json', 'marker.json', 'acceptance.json'))
            B.need(config['source'] == str(VOLUME / 'source') and config['mirror'] == str(VOLUME / 'repository/mirror.git')
                   and config['records'] == str(control) and config['environment']['source_revision'] == HEAD
                   and marker['head'] == accepted['head'] == HEAD and accepted['run'] == RUN
                   and marker['release_sha256'] == accepted['release_sha256'] == RELEASE
                   and accepted['production_gate_enabled'] is False, 'installed acceptance/configuration differs')
            inputs = json.loads(run['input'], object_pairs_hook=B.unique)
            policy = json.dumps(config, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
            B.need(hashlib.sha256(policy).hexdigest() == inputs['policy'], 'accepted host policy differs')
            source = VOLUME / 'source'
            original_lock = B.frozen_lock(source, HEAD, inputs['base'])
            B.need(B.git(source, 'rev-parse', 'HEAD^{tree}').decode().strip() == inputs['tree'], 'frozen source tree differs')
            B.need(not B.git(source, 'status', '--porcelain', '--untracked-files=no'), 'tracked source changed')
            freeze_raw = B.read(VOLUME / 'repository/frozen.json', 1024**2)
            freeze = json.loads(freeze_raw, object_pairs_hook=B.unique)
            bundle = B.read(VOLUME / 'repository/candidate.bundle', MAX_FILE)
            B.need(freeze['head'] == HEAD and freeze['base'] == inputs['base']
                   and freeze['bundle_sha256'] == hashlib.sha256(bundle).hexdigest(), 'frozen bundle differs')
            for name, value in raw.items():
                B.write(destination / name, value, 0o400)
            roots = {name: VOLUME / name for name in ('source', 'repository', 'control')}
            tree = destination / 'tree'
            tree.mkdir(mode=0o700)
            expected_tree = inventory(B, roots, tree)
            B.need(inventory(B, roots) == expected_tree and metadata() == raw, 'pilot changed during preservation')
            verify_copy(B, tree, expected_tree)
            database = connect(control / 'journal.sqlite3')
            try:
                database.execute('BEGIN')
                B.need(database_identity(B, database)[0] == identity, 'journal changed during preservation')
            finally:
                database.close()
            retained = connect(destination / 'journal.sqlite3')
            try:
                B.need(database_identity(B, retained)[0] == identity, 'retained journal changed')
            finally:
                retained.close()
            B.need(B.frozen_lock(source, HEAD, inputs['base']) == original_lock
                   and not B.git(source, 'status', '--porcelain', '--untracked-files=no')
                   and B.read(VOLUME / 'repository/frozen.json', 1024**2) == freeze_raw
                   and B.read(VOLUME / 'repository/candidate.bundle', MAX_FILE) == bundle,
                   'frozen source/bundle changed during preservation')
            B.authenticate(helpers, manifest, manifest_pin)
            builder.quiet_units(B)
            builder.idle(B)
            completed = {**state, 'state': 'preserved', 'journal': identity, 'inventory': expected_tree,
                         'journal_sha256': hashlib.sha256(B.read(destination / 'journal.sqlite3', MAX_DB)).hexdigest(),
                         'metadata_sha256': {name: hashlib.sha256(value).hexdigest() for name, value in raw.items()},
                         'local': run['local'], 'evidence_registered': bool(run['evidence'])}
            B.need(len(B.canonical(completed)) <= MAX_MANIFEST, 'preservation manifest exceeds budget')
            state = completed
            B.record(status, state)
            return state
        except BaseException:
            state['state'] = 'blocked'
            B.record(status, state)
            raise
    finally:
        if own_lease:
            os.close(lock_fd)
