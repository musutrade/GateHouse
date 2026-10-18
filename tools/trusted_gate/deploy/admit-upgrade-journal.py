"""Read-only journal admission for forward upgrade phases; never revoke or install."""
from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
import time

CONTROL = Path('/srv/gatehouse/pilot/control')
PILOT = Path('/srv/gatehouse/pilot')
BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'
REASON = 'administrator-reviewed Linux transition upgrade'
MAX_SECONDS = 10


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


def snapshot(B, keep, path, *, live):
    """Observe committed WAL pages; never construct Journal or create a sidecar."""
    path = B.owned(path, private=True)
    for item in (path, path.with_name(path.name + '-wal'), path.with_name(path.name + '-shm')):
        if item.exists() or item.is_symlink():
            B.need(B.owned(item, private=True).stat().st_size <= keep.MAX_DB, 'upgrade journal physical budget exceeded')
    B.need(not live or not path.with_name(path.name + '-wal').exists()
           or path.with_name(path.name + '-shm').exists(), 'live WAL observation requires existing shared-memory state')
    database = keep.connect(path);deadline = time.monotonic() + MAX_SECONDS
    database.set_progress_handler(lambda: int(time.monotonic() > deadline), 1000)
    try:
        database.execute('BEGIN')
        result = keep.database_identity(B, database)
        B.need(time.monotonic() <= deadline, 'upgrade journal observation timed out')
        return result
    finally:
        database.close()


def replay(B, keep, saved_identity, saved_rows, live_identity, live_rows):
    """Reconstruct the sole permitted run mutation from the appended event suffix."""
    B.need(saved_identity['schema_sha256'] == live_identity['schema_sha256'], 'upgrade journal schema changed')
    original = keep.old_run(B, saved_rows)
    expected = dict(original);saved_events = saved_rows['events'];events = live_rows['events']
    B.need(len(events) >= len(saved_events), 'upgrade journal lost historical events')
    same(B, events[:len(saved_events)], saved_events, 'upgrade journal historical event prefix changed')
    sequence = saved_events[-1][0] if saved_events else 0
    retirement_started = retired = False
    for row in events[len(saved_events):]:
        event = dict(zip(keep.TABLES['events'], row))
        B.need(type(event['sequence']) is int and event['sequence'] == sequence + 1
               and event['run'] == keep.RUN and type(event['time']) in (int, float)
               and math.isfinite(event['time']) and event['time'] > 0,
               'upgrade journal event identity/order differs')
        sequence = event['sequence']
        detail = json.loads(event['detail'], object_pairs_hook=B.unique)
        B.need(event['detail'].encode() == B.canonical(detail)[:-1], 'upgrade journal event is not canonical')
        if event['operation'] == 'update':
            B.need(not retirement_started, 'upgrade run changed after retirement started')
            if set(detail) == {'local', 'remote', 'reason'}:
                same(B, detail, {'local': 'invalid', 'remote': 'revoking', 'reason': REASON},
                     'upgrade revocation intent differs')
                B.need(expected['remote'] in ('pending', 'revoking'), 'upgrade revocation moved backwards')
            elif set(detail) == {'check_id'}:
                check = detail['check_id']
                B.need(expected['remote'] == 'revoking' and type(check) is int and check > 0
                       and expected['check_id'] in (None, check), 'upgrade Check binding differs')
            elif set(detail) == {'remote'}:
                B.need(expected['remote'] == 'revoking' and detail['remote'] == 'revoked',
                       'upgrade revocation completion differs')
            else:
                B.need(False, 'unknown upgrade run mutation')
            expected.update(detail)
        elif event['operation'] == 'retire-intent':
            B.need(expected['remote'] == 'revoked', 'source retirement precedes confirmed journal revocation')
            same(B, detail, {'head': keep.HEAD, 'retained': str(CONTROL / keep.RUN / 'publication')},
                 'upgrade retirement intent differs')
            retirement_started = True;retired = False
        elif event['operation'] == 'retired':
            B.need(retirement_started and not retired and expected['remote'] == 'revoked',
                   'source retirement completion has no matching intent')
            same(B, detail, {'head': keep.HEAD}, 'upgrade retirement completion differs')
            retired = True
        else:
            B.need(False, 'unknown upgrade journal event')
    expected_rows = [tuple(expected[key] for key in keep.TABLES['runs']) if row[0] == keep.RUN else row
                     for row in saved_rows['runs']]
    same(B, live_rows['runs'], expected_rows, 'upgrade journal run rows do not match forward event replay')
    phase = 'retired' if retired else ('retiring' if retirement_started else
        ('revoked' if expected['remote'] == 'revoked' else ('revoking' if expected['remote'] == 'revoking' else 'prepared')))
    return {'phase': phase, 'remote': expected['remote'], 'local': expected['local'],
            'check_id': expected['check_id'], 'appended_events': len(events) - len(saved_events)}


def source_state(B, keep, preservation, phase):
    paths = [PILOT / 'source', PILOT / 'repository/candidate.bundle', PILOT / 'repository/frozen.json']
    present = tuple(path.exists() or path.is_symlink() for path in paths)
    allowed = ((False, False, False),) if phase == 'retired' else (
        ((True, True, True), (False, True, True), (False, False, True), (False, False, False))
        if phase == 'retiring' else ((True, True, True),))
    B.need(present in allowed, 'old source presence differs from the forward retirement phase')
    for index in (1, 2):
        if present[index]:
            path = B.owned(paths[index], private=True)
            retained = preservation / 'tree/repository' / path.name
            B.need(B.read(path, keep.MAX_FILE) == B.read(retained, keep.MAX_FILE), 'old frozen source input changed')
    if present[0]:
        B.owned(paths[0], directory=True)
        B.frozen_lock(paths[0], keep.HEAD, BASE)
        B.need(not B.git(paths[0], 'status', '--porcelain', '--untracked-files=no'), 'old tracked source changed')
    return list(present)


def observe(B, helpers, manifest, manifest_pin, preservation, preservation_pin, lock_fd):
    """Internal coordinator prerequisite, not remote/publication/install admission.

    The caller must separately authenticate prepared inputs, passive inspection,
    current base, remote Check state, entry generation, controller drain and
    retained publication before mutation. A journal event proves no remote effect.
    """
    B.need(os.geteuid() == 0, 'administrator authentication required for upgrade journal observation')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported upgrade journal platform')
    B.need(type(lock_fd) is int and lock_fd >= 0, 'upgrade journal requires inherited controller lease')
    code = B.authenticate(helpers, manifest, manifest_pin)
    helpers, manifest, preservation = map(B.path_operand, (helpers, manifest, preservation))
    keep = B.load(helpers, 'preserve-pilot-state.py');fence = B.load(helpers, 'install-transaction.py')
    builder = B.load(helpers, 'build-protected-core.py')
    B.need(preservation != PILOT and PILOT not in preservation.parents and preservation not in PILOT.parents,
           'preservation must remain separate from pilot state')
    def guards():
        fence.lease(B, lock_fd);builder.worker_identity(B);builder.quiet_units(B);builder.idle(B)
    guards()
    saved = keep.verify(B, preservation, preservation_pin)
    B.need(saved['helper_revision'] == code['revision'] and saved['helper_manifest_sha256'] == manifest_pin,
           'upgrade retention belongs to another helper release')
    saved_identity, saved_rows = snapshot(B, keep, preservation / 'journal.sqlite3', live=False)
    same(B, saved_identity, saved['journal'], 'upgrade retained journal identity changed')
    original = keep.old_run(B, saved_rows)
    inputs = json.loads(original['input'], object_pairs_hook=B.unique)
    config = json.loads(B.read(preservation / 'host.json', 1024**2), object_pairs_hook=B.unique)
    policy = json.dumps(config, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    B.need(inputs['policy'] == hashlib.sha256(policy).hexdigest() and inputs['base'] == BASE
           and config['source'] == str(PILOT / 'source') and config['mirror'] == str(PILOT / 'repository/mirror.git')
           and config['records'] == str(CONTROL) and config['environment']['source_revision'] == keep.HEAD,
           'upgrade retained policy or storage identity differs')
    live_identity, live_rows = snapshot(B, keep, CONTROL / 'journal.sqlite3', live=True)
    state = replay(B, keep, saved_identity, saved_rows, live_identity, live_rows)
    presence = source_state(B, keep, preservation, state['phase'])
    guards()
    same(B, snapshot(B, keep, CONTROL / 'journal.sqlite3', live=True)[0], live_identity,
         'upgrade journal changed during observation')
    same(B, source_state(B, keep, preservation, state['phase']), presence, 'old source changed during observation')
    same(B, keep.verify(B, preservation, preservation_pin), saved, 'upgrade retention changed during observation')
    B.authenticate(helpers, manifest, manifest_pin);fence.lease(B, lock_fd)
    return {'schema': 'gatehouse-upgrade-journal/v1', 'state': 'observed', 'head': code['revision'],
        'helper_manifest_sha256': manifest_pin, 'preservation_sha256': preservation_pin,
        'old_head': keep.HEAD, 'old_run': keep.RUN, **state, 'old_source_present': presence,
        'journal_sha256': hashlib.sha256(B.canonical(live_identity)).hexdigest(),
        'remote_revocation_confirmed': False, 'retained_publication_verified': False,
        'installation_admitted': False, 'administrator_ready': False, 'production_gate_enabled': False,
        'scope': 'read-only journal/source phase observation only; remote, publication, entry and drain admission required'}


if __name__ == '__main__':
    raise SystemExit('upgrade journal admission is an internal library; no administrator entry is available')
