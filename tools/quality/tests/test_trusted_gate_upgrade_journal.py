"""Upgrade observation must preserve history and follow actual journal semantics.

Root/authentication/worker boundaries are replaced explicitly. Real WAL journals,
SQLite backup, retained failed reports and Git worktree retirement are exercised.
GitHub calls are mocks and no remote revocation or installation is accepted.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX = load('upgrade_journal_preservation_fixture', 'tools/quality/tests/test_trusted_gate_preservation.py')
GATE = load('upgrade_journal_control_fixture', 'tools/quality/tests/test_trusted_gate.py')
J = load('upgrade_journal_library', 'tools/trusted_gate/deploy/admit-upgrade-journal.py')
FENCE = load('upgrade_journal_lease', 'tools/trusted_gate/deploy/install-transaction.py')
KEEP = FIX.KEEP;BOOT = FIX.BOOT


class UpgradeJournalTests(unittest.TestCase):
    def setUp(self):
        self.f = FIX.PreservationTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        f = self.f;f.preserve();self.B = SimpleNamespace(**vars(f.B))
        self.pin = J.hashlib.sha256((f.destination/'preservation.json').read_bytes()).hexdigest()
        self.B.authenticate = Mock(return_value={'revision': 'a'*40})
        self.builder = SimpleNamespace(worker_identity=Mock(), quiet_units=Mock(), idle=Mock())
        self.B.load = lambda helpers, name: {'preserve-pilot-state.py': KEEP, 'install-transaction.py': FENCE,
                                            'build-protected-core.py': self.builder}[name]

    def observe(self, **options):
        f = self.f
        with f.domain(), patch.multiple(J, CONTROL=f.control, PILOT=f.volume, BASE=f.head), \
             patch.object(BOOT, 'owned', side_effect=f.owned), patch.object(J.os, 'geteuid', return_value=0), \
             patch.object(FENCE, 'LEASE', f.control/'controller.lock'):
            with (f.control/'controller.lock').open('r+b') as lease:
                return J.observe(self.B, f.root/'helpers', f.root/'manifest', 'a'*64,
                    options.pop('preservation', f.destination), self.pin, options.pop('lock_fd', lease.fileno()), **options)

    def event(self, operation, detail, *, run=KEEP.RUN):
        sequence = self.f.writer.execute('SELECT max(sequence) FROM events').fetchone()[0] + 1
        self.f.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)',
            (sequence, run, float(sequence), operation, BOOT.canonical(detail)[:-1].decode()))

    def update(self, **fields):
        self.f.writer.execute('UPDATE runs SET ' + ','.join(key+'=?' for key in fields) + ' WHERE id=?',
                              (*fields.values(), KEEP.RUN))
        self.event('update', fields)

    def revoke(self):
        self.update(local='invalid', remote='revoking', reason=J.REASON);self.update(remote='revoked')

    def intent(self):
        self.event('retire-intent', {'head': self.f.head, 'retained': str(self.f.control/KEEP.RUN/'publication')})

    def physical(self):
        f = self.f
        return {str(path): path.read_bytes() for path in (
            f.journal_path, f.journal_path.with_name('journal.sqlite3-wal'), f.config_path,
            f.destination/'journal.sqlite3', f.destination/'preservation.json', f.raw_report) if path.exists()}

    def test_pending_observation_is_readonly_and_never_claims_remote_or_installation_acceptance(self):
        before = self.physical()
        with patch.object(self.B, 'write', side_effect=AssertionError('unexpected write')), \
             patch.object(self.B, 'record', side_effect=AssertionError('unexpected state change')):
            result = self.observe()
        self.assertEqual(self.physical(), before);self.assertEqual(result['phase'], 'prepared')
        self.assertEqual(result['old_source_present'], [True, True, True]);self.assertEqual(result['appended_events'], 0)
        for flag in ('remote_revocation_confirmed', 'retained_publication_verified',
                     'installation_admitted', 'administrator_ready', 'production_gate_enabled'):
            self.assertIs(result[flag], False)
        self.assertEqual(self.builder.quiet_units.call_count, 2);self.assertEqual(self.B.authenticate.call_count, 2)

    def test_actual_control_revocation_then_git_retirement_replays_without_restoring_saved_pending_db(self):
        f = self.f;journal = GATE.Journal(f.journal_path);self.addCleanup(journal.close)
        config = json.loads(f.config_path.read_bytes());config['check_name'] = 'fixture-check'
        github = SimpleNamespace(find_check=Mock(return_value=None));executor = SimpleNamespace(confirm_stopped=Mock())
        control = GATE.Control(journal, github, executor, config)
        control.revoke(KEEP.RUN, J.REASON)
        self.assertEqual(self.observe()['phase'], 'revoked')
        source_module = sys.modules[GATE.retire.__module__]
        with patch.object(source_module, 'protected', side_effect=f.owned):
            GATE.retire(config, executor, journal, KEEP.RUN)
        before = self.physical();result = self.observe()
        self.assertEqual(result['phase'], 'retired');self.assertEqual(result['old_source_present'], [False]*3)
        self.assertFalse(result['remote_revocation_confirmed']);self.assertEqual(before, self.physical())
        saved = KEEP.connect(f.destination/'journal.sqlite3')
        try:self.assertEqual(saved.execute('SELECT remote FROM runs WHERE id=?', (KEEP.RUN,)).fetchone()[0], 'pending')
        finally:saved.close()
        self.assertEqual(journal.get(KEEP.RUN)['remote'], 'revoked')
        report = f.destination/'tree/source/.gatehouse-host/reports/invocations/failed-invocation/test_result.json'
        self.assertEqual(report.read_text(), '{"status":"FAIL"}')

    def test_actual_lost_api_response_leaves_revoking_and_source_retained(self):
        f = self.f;journal = GATE.Journal(f.journal_path);self.addCleanup(journal.close)
        github = SimpleNamespace(find_check=Mock(side_effect=RuntimeError('lost API response')))
        executor = SimpleNamespace(confirm_stopped=Mock())
        with self.assertRaisesRegex(RuntimeError, 'lost API'):
            GATE.Control(journal, github, executor, {'check_name':'fixture'}).revoke(KEEP.RUN, J.REASON)
        result = self.observe();self.assertEqual(result['phase'], 'revoking')
        self.assertTrue(f.source.exists());self.assertFalse(result['remote_revocation_confirmed'])

    def test_check_id_binding_and_repeated_revocation_intent_are_forward_only(self):
        self.update(local='invalid', remote='revoking', reason=J.REASON);self.update(check_id=123)
        self.update(local='invalid', remote='revoking', reason=J.REASON);self.update(check_id=123)
        self.update(remote='revoked');result = self.observe()
        self.assertEqual(result['phase'], 'revoked');self.assertEqual(result['check_id'], 123)
        self.update(local='invalid', remote='revoking', reason=J.REASON)
        with self.assertRaisesRegex(ValueError, 'moved backwards'):self.observe()

    def test_row_rewrite_without_corresponding_event_is_rejected(self):
        self.f.writer.execute("UPDATE runs SET remote='revoked',local='invalid' WHERE id=?", (KEEP.RUN,))
        with self.assertRaisesRegex(ValueError, 'run rows'):self.observe()

    def test_earlier_events_cannot_be_rewritten_or_removed(self):
        self.f.writer.execute("UPDATE events SET detail='changed' WHERE sequence=1")
        with self.assertRaisesRegex(ValueError, 'prefix changed'):self.observe()
        self.f.writer.execute('DELETE FROM events')
        with self.assertRaisesRegex(ValueError, 'lost historical'):self.observe()

    def test_new_or_other_run_cannot_appear_without_rejecting_the_upgrade_slot(self):
        row = list(self.f.writer.execute('SELECT * FROM runs').fetchone());row[0] = 'foreign-run';row[2] = 'f'*40
        self.f.writer.execute('INSERT INTO runs VALUES (?,?,?,?,?,?,?,?,?,?,?,?)', row)
        with self.assertRaisesRegex(ValueError, 'run rows'):self.observe()

    def test_foreign_actor_event_and_sequence_gap_are_rejected(self):
        self.event('update', {'remote':'revoked'}, run='another-run')
        with self.assertRaisesRegex(ValueError, 'identity/order'):self.observe()
        self.f.writer.execute('UPDATE events SET run=?,sequence=9 WHERE sequence=2', (KEEP.RUN,))
        with self.assertRaisesRegex(ValueError, 'identity/order'):self.observe()

    def test_unknown_operation_or_noncanonical_detail_is_rejected(self):
        self.event('approved', {'production_gate_enabled':True})
        with self.assertRaisesRegex(ValueError, 'unknown upgrade journal event'):self.observe()
        self.f.writer.execute("UPDATE events SET operation='update',detail=' {\"remote\":\"revoked\"}' WHERE sequence=2")
        with self.assertRaisesRegex(ValueError, 'not canonical'):self.observe()

    def test_unknown_fields_wrong_reason_or_premature_completion_cannot_advance(self):
        for detail in ({'remote':'revoked'}, {'local':'passed'}, {'evidence':None},
                       {'local':'invalid','remote':'revoking','reason':'unreviewed reason'}):
            self.event('update', detail)
            with self.assertRaises(ValueError):self.observe()
            self.f.writer.execute('DELETE FROM events WHERE sequence=2')

    def test_check_id_requires_revocation_and_cannot_retarget_existing_check(self):
        self.update(check_id=1)
        with self.assertRaisesRegex(ValueError, 'Check binding'):self.observe()
        self.f.writer.execute('DELETE FROM events WHERE sequence=2');self.f.writer.execute('UPDATE runs SET check_id=NULL')
        self.update(local='invalid',remote='revoking',reason=J.REASON);self.update(check_id=1);self.update(check_id=2)
        with self.assertRaisesRegex(ValueError, 'Check binding'):self.observe()

    def test_nonfinite_or_nonpositive_event_time_is_rejected(self):
        self.update(local='invalid',remote='revoking',reason=J.REASON)
        self.f.writer.execute('UPDATE events SET time=0 WHERE sequence=2')
        with self.assertRaises(ValueError):self.observe()
        self.f.writer.execute('UPDATE events SET time=? WHERE sequence=2', (float('inf'),))
        with self.assertRaises(ValueError):self.observe()

    def test_retirement_before_revocation_or_with_wrong_destination_is_rejected(self):
        self.intent()
        with self.assertRaisesRegex(ValueError, 'precedes'):self.observe()
        self.f.writer.execute('DELETE FROM events WHERE sequence=2');self.revoke()
        self.event('retire-intent', {'head':self.f.head,'retained':'/tmp/unreviewed'})
        with self.assertRaisesRegex(ValueError, 'retirement intent differs'):self.observe()

    def test_completion_without_intent_and_run_mutation_after_intent_are_rejected(self):
        self.revoke();self.event('retired', {'head':self.f.head})
        with self.assertRaisesRegex(ValueError, 'no matching intent'):self.observe()
        self.f.writer.execute('DELETE FROM events WHERE sequence=4');self.intent();self.update(check_id=3)
        with self.assertRaisesRegex(ValueError, 'changed after retirement'):self.observe()

    def test_retirement_recovery_admits_only_the_known_source_bundle_freeze_removal_order(self):
        f = self.f;self.revoke();self.intent();self.assertEqual(self.observe()['phase'], 'retiring')
        subprocess.run(['/usr/bin/git','-C',str(f.repository/'mirror.git'),'worktree','remove','--force',str(f.source)],
                       env=BOOT.ENV,check=True,capture_output=True)
        self.assertEqual(self.observe()['old_source_present'], [False,True,True])
        (f.repository/'candidate.bundle').unlink();self.assertEqual(self.observe()['old_source_present'], [False,False,True])
        (f.repository/'frozen.json').unlink();self.assertEqual(self.observe()['phase'], 'retiring')
        self.event('retired', {'head':f.head});self.assertEqual(self.observe()['phase'], 'retired')
        self.intent();self.event('retired', {'head':f.head});self.assertEqual(self.observe()['phase'], 'retired')

    def test_missing_source_without_intent_and_false_retired_event_are_rejected(self):
        self.revoke();self.intent();self.event('retired', {'head':self.f.head})
        with self.assertRaisesRegex(ValueError, 'source presence'):self.observe()
        self.f.writer.execute('DELETE FROM events WHERE sequence IN (4,5)')
        subprocess.run(['/usr/bin/git','-C',str(self.f.repository/'mirror.git'),'worktree','remove','--force',str(self.f.source)],
                       env=BOOT.ENV,check=True,capture_output=True)
        with self.assertRaisesRegex(ValueError, 'source presence'):self.observe()

    def test_illegal_partial_removal_and_changed_old_bundle_or_tracked_source_are_rejected(self):
        f = self.f;path = f.repository/'candidate.bundle';original = path.read_bytes();path.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'frozen source input'):self.observe()
        path.write_bytes(original);tracked = f.source/'tools/harness-gate/Cargo.lock';tracked.write_bytes(b'changed')
        with self.assertRaises(ValueError):self.observe()
        self.revoke();self.intent();path.unlink()
        with self.assertRaisesRegex(ValueError, 'source presence'):self.observe()

    def test_saved_snapshot_pin_and_all_failed_evidence_remain_required(self):
        pin = self.pin;self.pin = 'f'*64
        with self.assertRaisesRegex(ValueError, 'manifest pin'):self.observe()
        self.pin = pin;path = self.f.destination/'tree/control/pilot-validation-attempt.stderr'
        path.chmod(0o600);path.write_bytes(b'erased failure');path.chmod(0o400)
        with self.assertRaisesRegex(ValueError, 'retained file differs'):self.observe()

    def test_rehashed_saved_policy_cannot_differ_from_the_recorded_old_run(self):
        f = self.f;path = f.destination/'host.json';config = json.loads(path.read_bytes());config['extra'] = 'changed'
        path.chmod(0o600);path.write_bytes(BOOT.canonical(config));path.chmod(0o400)
        status = f.destination/'preservation.json';value = json.loads(status.read_bytes())
        value['metadata_sha256']['host.json'] = J.hashlib.sha256(path.read_bytes()).hexdigest()
        status.write_bytes(BOOT.canonical(value));self.pin = J.hashlib.sha256(status.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, 'policy or storage'):self.observe()

    def test_missing_live_shm_is_rejected_before_connecting_or_creating_a_file(self):
        f = self.f;shm = f.journal_path.with_name('journal.sqlite3-shm');hidden = shm.with_suffix('.private-copy')
        shm.rename(hidden)
        try:
            with self.assertRaisesRegex(ValueError, 'existing shared-memory'):self.observe()
            self.assertFalse(shm.exists())
        finally:hidden.rename(shm)

    def test_missing_or_wrong_lease_and_unsafe_worker_observation_block(self):
        for descriptor in (None, True, -1):
            with self.assertRaisesRegex(ValueError, 'inherited'):self.observe(lock_fd=descriptor)
        with self.f.config_path.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError, 'lease'):self.observe(lock_fd=wrong.fileno())
        self.builder.quiet_units.side_effect = ValueError('worker active')
        with self.assertRaisesRegex(ValueError, 'worker active'):self.observe()

    def test_helper_identity_or_late_lease_drift_rejects_observation(self):
        self.B.authenticate.return_value = {'revision':'f'*40}
        with self.assertRaisesRegex(ValueError, 'another helper release'):self.observe()
        self.B.authenticate.return_value = {'revision':'a'*40}
        with patch.object(FENCE, 'lease', side_effect=[None,None,ValueError('lease replaced')]):
            with self.assertRaisesRegex(ValueError, 'lease replaced'):self.observe()

    def test_schema_change_is_rejected_even_if_the_old_rows_are_unchanged(self):
        self.f.writer.execute('CREATE INDEX unexpected ON runs(local)')
        with self.assertRaisesRegex(ValueError, 'schema changed'):self.observe()

    def test_live_transition_between_two_observations_cannot_be_returned(self):
        original = J.snapshot;calls = 0
        def observe(*args, **kwargs):
            nonlocal calls
            if kwargs['live']:
                calls += 1
                if calls == 2:self.update(local='invalid',remote='revoking',reason=J.REASON)
            return original(*args, **kwargs)
        with patch.object(J, 'snapshot', side_effect=observe):
            with self.assertRaisesRegex(ValueError, 'changed during observation'):self.observe()

    def test_nonroot_and_direct_cli_fail_without_administrator_entry(self):
        with patch.object(J.os, 'geteuid', return_value=1000), patch.object(self.B, 'load') as imported:
            with self.assertRaisesRegex(ValueError, 'administrator'):
                J.observe(self.B, *([None]*6))
        imported.assert_not_called()
        result = subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/admit-upgrade-journal.py')],
                                capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'no administrator entry',result.stderr)


if __name__ == '__main__':unittest.main()
