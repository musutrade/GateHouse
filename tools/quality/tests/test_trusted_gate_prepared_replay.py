"""Prepared replay survives retirement without accepting changed facts or live state.

Ordinary private fixtures replace Root ownership, authenticated release/source,
mount and worker boundaries. Real SQLite snapshots, retention trees, accepted
configuration semantics, runtime payload hashing and receipt comparison run.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX = load('prepared_configuration_fixture', 'tools/quality/tests/test_trusted_gate_host_config.py')
C = FIX.C;BOOT = FIX.BOOT
P = load('prepared_old_preflight', 'tools/trusted_gate/deploy/preflight-history-pilot.py')
KEEP = load('prepared_retention', 'tools/trusted_gate/deploy/preserve-pilot-state.py')
FENCE = load('prepared_lease', 'tools/trusted_gate/deploy/install-transaction.py')


class PreparedConfigurationReplayTests(unittest.TestCase):
    def setUp(self):
        self.f = FIX.HostConfigurationTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        f = self.f;self.B = SimpleNamespace(**vars(f.B))
        f.contract.old_source_map = FIX.R.old_source_map
        self.old_payload = {P.member_name(row[0]): f.payload['before/' + name]
                            for name, row in FIX.R.old_source_map().items()}
        self.old_payload['bin/harness-gate'] = b'old accepted Core fixture'
        old = f.old
        old.update(schema='gatehouse-trusted-host/v1', repository=P.REPOSITORY, repository_id=P.REPOSITORY_ID,
            base_branch='main', profile='full', developer_uid=999, executor_user='gatehouse-executor',
            core='/opt/gatehouse/bin/harness-gate', policy_directory='/etc/gatehouse/policy',
            environment={'source_revision': P.HEAD, 'release_sha256': P.RELEASE},
            installation_id=159921491, app_key='/etc/gatehouse/checks.private-key.pem')
        old.update({key: str(f.control.parent / relative) for key, relative in (
            ('source', 'source'), ('mirror', 'repository/mirror.git'), ('scratch', 'scratch'))})
        old['protected_files'] = P.accepted_pins(self.B, FIX.R, self.old_payload, f.builder)
        f.config_path.write_bytes(BOOT.canonical(old))
        self.configuration = f.compose();self.config_pin = C.digest((f.output / 'binding.json').read_bytes())
        self.preservation = f.root / 'retention';self.preservation.mkdir(mode=0o700)
        self.accepted = {'head': P.HEAD, 'run': P.RUN, 'release_sha256': P.RELEASE,
            'coverage_acceptance': 'transition', 'production_gate_enabled': False,
            'authoritative_publish': 'blocked', 'negative_conclusion': 'failure', 'negative_check': P.NEGATIVE_CHECK,
            'worker_git_history': 'passed within the protected Core boundary step'}
        metadata = {'host.json': BOOT.canonical(old), 'marker.json': BOOT.canonical(
            {'head': P.HEAD, 'release_sha256': P.RELEASE}), 'acceptance.json': BOOT.canonical(self.accepted)}
        for name, raw in metadata.items():BOOT.write(self.preservation / name, raw, 0o400)
        self.journal(self.preservation / 'journal.sqlite3', 'pending', old)
        self.journal(f.control / 'journal.sqlite3', 'revoked', old)
        tree = self.preservation / 'tree';(tree / 'control/failed').mkdir(parents=True, mode=0o700)
        BOOT.write(tree / 'control/failed/core.log', b'FAIL', 0o400)
        saved = KEEP.connect(self.preservation / 'journal.sqlite3')
        try:identity, _ = KEEP.database_identity(self.B, saved)
        finally:saved.close()
        self.saved = {'schema': 'gatehouse-pilot-preservation/v1', 'state': 'preserved', 'head': KEEP.HEAD,
            'run': KEEP.RUN, 'helper_revision': f.head, 'helper_manifest_sha256': 'a'*64,
            'production_gate_enabled': False, 'metadata_sha256': {name: C.digest(raw) for name, raw in metadata.items()},
            'journal': identity, 'journal_sha256': C.digest((self.preservation / 'journal.sqlite3').read_bytes()),
            'inventory': {'files': {'control/failed/core.log': [4, C.digest(b'FAIL'), 0o600]},
                'directories': {'control': 0o700, 'control/failed': 0o700}, 'bytes': 4}}
        self.preservation_pin = self.save_manifest()
        self.preflight = SimpleNamespace(**{name: getattr(P, name) for name in (
            'accepted_pins', 'configuration', 'REPOSITORY', 'REPOSITORY_ID')})
        self.preflight.accepted_release = Mock(return_value=self.old_payload)
        self.preflight.observe = Mock(side_effect=AssertionError('must not replay live fixed659 preflight'))
        f.builder.pilot_slot.side_effect = AssertionError('must not require a pending live old slot')
        f.storage.admit.return_value = {'schema': 'fixture-storage', 'phase': 'mounted'}
        self.B.load = lambda helpers, name: {'build-protected-core.py': f.builder,
            'prepare-linux-volume.py': f.storage, 'bootstrap-release.py': f.contract,
            'preflight-history-pilot.py': self.preflight, 'linux-transition-policy.py': FIX.R.LINUX,
            'preserve-pilot-state.py': KEEP, 'install-transaction.py': FENCE}[name]
        for function in (f.builder.pilot_slot, f.storage.admit, f.contract.bind_source,
                         f.builder.worker_identity, self.B.authenticate):function.reset_mock()

    def journal(self, path, remote, config):
        database = sqlite3.connect(path)
        try:
            database.executescript('''CREATE TABLE runs (id TEXT PRIMARY KEY, repository TEXT NOT NULL,
                head TEXT NOT NULL, epoch INTEGER NOT NULL, input TEXT NOT NULL, local TEXT NOT NULL,
                evidence TEXT, remote TEXT NOT NULL, check_id INTEGER, pr INTEGER, reason TEXT, created REAL NOT NULL,
                UNIQUE(repository,head,epoch));
                CREATE TABLE events (sequence INTEGER PRIMARY KEY,run TEXT NOT NULL,time REAL NOT NULL,
                operation TEXT NOT NULL,detail TEXT NOT NULL);''')
            inputs = {'repository': P.REPOSITORY, 'repository_id': P.REPOSITORY_ID, 'base': C.BASE,
                'policy': C.digest(json.dumps(config, sort_keys=True, separators=(',', ':')).encode())}
            database.execute('INSERT INTO runs VALUES (?,?,?,?,?,?,?,?,?,?,?,?)',
                (P.RUN, P.REPOSITORY, P.HEAD, 1, json.dumps(inputs), 'failed', None, remote, None, None, 'retained', 1.0))
            database.execute('INSERT INTO events VALUES (?,?,?,?,?)', (1, P.RUN, 1.0, remote, '{}'))
            database.commit()
        finally:database.close()
        path.chmod(0o400 if remote == 'pending' else 0o600)

    def save_manifest(self):
        raw = BOOT.canonical(self.saved);path = self.preservation / 'preservation.json'
        if path.exists():path.write_bytes(raw)
        else:BOOT.write(path, raw, 0o600)
        return C.digest(raw)

    def replay(self, **options):
        f = self.f
        with patch.multiple(C, VOLUME=f.volume, CONTROL=f.control, INBOX=f.control.parent/'inbox', CONFIG=f.config_path), \
             patch.object(P, 'VOLUME', f.control.parent), \
             patch.object(FENCE, 'LEASE', f.control/'controller.lock'), patch.object(C.os, 'geteuid', return_value=0):
            with (f.control / 'controller.lock').open('r+b') as lease:
                return C.verify_prepared(self.B, f.root/'helpers', f.manifest, 'a'*64, f.archive,
                    C.digest(f.archive.read_bytes()), 'b'*64, f.seal_pin, f.core_pin, self.config_pin,
                    f.root/'host-recipe', 'c'*64, f.root/'volume-recipe', 'd'*64,
                    options.pop('preservation', self.preservation), self.preservation_pin,
                    options.pop('lock_fd', lease.fileno()), **options)

    def snapshot(self):
        return {str(path): (path.stat().st_mode, path.read_bytes())
                for path in self.f.root.rglob('*') if path.is_file()}

    def test_retired_live_slot_and_missing_old_source_do_not_require_rollback_or_any_writes(self):
        f = self.f;self.assertFalse(Path(f.old['source']).exists())
        f.config_path.write_bytes(b'partially installed configuration must not be read here')
        before = self.snapshot();old_read = self.B.read
        def read(path, limit):
            self.assertNotIn(Path(path), [f.config_path, f.control/'journal.sqlite3'])
            return old_read(path, limit)
        self.B.read = read
        with patch.object(self.B, 'write', side_effect=AssertionError('unexpected write')), \
             patch.object(self.B, 'record', side_effect=AssertionError('unexpected state mutation')):
            result = self.replay()
        self.assertEqual(before, self.snapshot());self.assertEqual(result['state'], 'verified')
        for flag in ('administrator_ready', 'production_gate_enabled', 'installation_admitted', 'captured_core_policy_checked'):
            self.assertIs(result[flag], False)
        self.assertEqual(result['configuration_sha256'], self.configuration['configuration_sha256'])
        self.assertEqual(result['plan_sha256'], self.configuration['plan_sha256'])
        self.preflight.observe.assert_not_called();f.builder.pilot_slot.assert_not_called()

    def test_missing_or_wrong_inherited_lease_cannot_replay(self):
        for descriptor in (None, True, -1):
            with self.assertRaisesRegex(ValueError, 'inherited'):self.replay(lock_fd=descriptor)
        with self.f.config_path.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError, 'lease'):self.replay(lock_fd=wrong.fileno())

    def test_completed_binding_needs_external_pin_exact_state_and_no_extra_outputs(self):
        pin = self.config_pin;self.config_pin = 'f'*64
        with self.assertRaises(ValueError):self.replay()
        self.config_pin = pin;path = self.f.output/'compose.json';raw = path.read_bytes()
        value = json.loads(raw);value['state'] = 'blocked';path.write_bytes(BOOT.canonical(value))
        with self.assertRaisesRegex(ValueError, 'complete control'):self.replay()
        path.write_bytes(raw);BOOT.write(self.f.output/'extra.json', b'unknown', 0o600)
        with self.assertRaisesRegex(ValueError, 'inventory'):self.replay()

    def test_rehashed_binding_cannot_grant_authority(self):
        path = self.f.output/'binding.json';value = json.loads(path.read_bytes());value['administrator_ready'] = True
        path.chmod(0o600);path.write_bytes(BOOT.canonical(value));path.chmod(0o400);self.config_pin = C.digest(path.read_bytes())
        with self.assertRaisesRegex(ValueError, 'binding differs'):self.replay()

    def test_rehashed_preservation_cannot_change_release_or_helper_identity(self):
        original = copy.deepcopy(self.saved)
        for key, value in (('state', 'incomplete'), ('head', 'f'*40), ('helper_revision', 'f'*40),
                           ('helper_manifest_sha256', 'f'*64), ('production_gate_enabled', True)):
            self.saved = {**original, key: value};self.preservation_pin = self.save_manifest()
            with self.assertRaises(ValueError):self.replay()

    def test_rehashed_preserved_acceptance_cannot_promote_transition(self):
        path = self.preservation/'acceptance.json';self.accepted['production_gate_enabled'] = True
        path.chmod(0o600);path.write_bytes(BOOT.canonical(self.accepted));path.chmod(0o400)
        self.saved['metadata_sha256']['acceptance.json'] = C.digest(path.read_bytes())
        self.preservation_pin = self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'marker/report'):self.replay()

    def test_rehashed_preserved_config_must_match_accepted_source_pins(self):
        path = self.preservation/'host.json';value = json.loads(path.read_bytes())
        value['protected_files']['/opt/gatehouse/trusted_gate/host.py'] = 'f'*64
        path.chmod(0o600);path.write_bytes(BOOT.canonical(value));path.chmod(0o400)
        self.saved['metadata_sha256']['host.json'] = C.digest(path.read_bytes());self.preservation_pin = self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'accepted configuration'):self.replay()

    def test_rehashed_snapshot_must_still_contain_the_accepted_pending_run(self):
        path = self.preservation/'journal.sqlite3';path.chmod(0o600)
        database = sqlite3.connect(path)
        try:
            database.execute("UPDATE runs SET remote='revoked'");database.commit()
            identity, _ = KEEP.database_identity(self.B, database)
        finally:database.close()
        path.chmod(0o400);self.saved['journal'] = identity
        self.saved['journal_sha256'] = C.digest(path.read_bytes());self.preservation_pin = self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'sole slot'):self.replay()

    def test_preserved_config_cannot_be_rewritten_away_from_saved_run_policy(self):
        path = self.preservation/'host.json';value = json.loads(path.read_bytes());value['unknown_extension'] = 'changed'
        path.chmod(0o600);path.write_bytes(BOOT.canonical(value));path.chmod(0o400)
        self.saved['metadata_sha256']['host.json'] = C.digest(path.read_bytes());self.preservation_pin = self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'configuration identity'):self.replay()

    def test_changed_retained_raw_failure_or_extra_tree_file_blocks(self):
        path = self.preservation/'tree/control/failed/core.log';path.chmod(0o600);path.write_bytes(b'PASS');path.chmod(0o400)
        with self.assertRaisesRegex(ValueError, 'retained file differs'):self.replay()
        path.chmod(0o600);path.write_bytes(b'FAIL');path.chmod(0o400)
        BOOT.write(path.with_name('unknown'), b'extra', 0o400)
        with self.assertRaisesRegex(ValueError, 'unexpected retained'):self.replay()

    def test_retention_cannot_share_staging_volume(self):
        with self.assertRaisesRegex(ValueError, 'separate'):self.replay(preservation=self.f.volume/'retention')

    def test_wrong_retention_pin_or_unsealed_file_cannot_be_replayed(self):
        pin = self.preservation_pin;self.preservation_pin = 'f'*64
        with self.assertRaisesRegex(ValueError, 'manifest pin'):self.replay()
        self.preservation_pin = pin;path = self.preservation/'tree/control/failed/core.log';path.chmod(0o600)
        with self.assertRaisesRegex(ValueError, 'not sealed'):self.replay()

    def test_runtime_tool_and_core_payloads_are_rehashed(self):
        for path in (self.f.inputs/'tools/toolchain/bin/rustc', self.f.core_root/'harness-gate'):
            raw = path.read_bytes();mode = path.stat().st_mode & 0o777
            path.chmod(0o700);path.write_bytes(b'changed');path.chmod(mode)
            with self.assertRaises(ValueError):self.replay()
            path.chmod(0o700);path.write_bytes(raw);path.chmod(mode)

    def test_staging_import_provenance_and_frozen_bundle_remain_required(self):
        self.f.imported['helper_manifest_sha256'] = 'f'*64
        with self.assertRaisesRegex(ValueError, 'another helper'):self.replay()
        self.f.imported['helper_manifest_sha256'] = 'a'*64
        path = self.f.source_root/'candidate.bundle';path.chmod(0o600);path.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'staged source'):self.replay()

    def test_os_drift_and_accepted_release_failure_block(self):
        old = self.f.os_files['/usr/bin/git'];self.f.os_files['/usr/bin/git'] = b'changed OS'
        with self.assertRaises(ValueError):self.replay()
        self.f.os_files['/usr/bin/git'] = old
        self.preflight.accepted_release.side_effect = ValueError('old release changed')
        with self.assertRaisesRegex(ValueError, 'old release changed'):self.replay()

    def test_input_mutation_on_second_observation_fails_readonly(self):
        self.f.builder.imported_inputs.side_effect = [copy.deepcopy(self.f.imported), ValueError('input changed')]
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'input changed'):self.replay()
        self.assertEqual(before, self.snapshot())

    def test_receipt_mutation_during_second_input_check_cannot_return_verified(self):
        calls = 0
        def imported(*args):
            nonlocal calls
            calls += 1
            if calls == 2:(self.f.output/'host.json').write_bytes(b'changed during replay')
            return copy.deepcopy(self.f.imported)
        self.f.builder.imported_inputs.side_effect = imported
        with self.assertRaisesRegex(ValueError, 'outputs changed'):self.replay()

    def test_unsafe_worker_or_storage_status_blocks_before_output_or_retention_mutation(self):
        for api, name in ((self.f.builder, 'worker_identity'), (self.f.builder, 'quiet_units'),
                          (self.f.builder, 'idle'), (self.f.storage, 'admit')):
            function = getattr(api, name);function.side_effect = ValueError('unsafe boundary')
            before = self.snapshot()
            with self.assertRaisesRegex(ValueError, 'unsafe boundary'):self.replay()
            self.assertEqual(before, self.snapshot());function.side_effect = None

    def test_storage_identity_or_final_lease_failure_rejects_result(self):
        self.f.storage.admit.side_effect = [{'identity': 1}, {'identity': 2}]
        with self.assertRaisesRegex(ValueError, 'storage changed'):self.replay()
        self.f.storage.admit.side_effect = None
        with patch.object(FENCE, 'lease', side_effect=[None, ValueError('lease replaced')]):
            with self.assertRaisesRegex(ValueError, 'lease replaced'):self.replay()

    def test_source_binding_workers_and_authentication_are_rechecked(self):
        self.replay()
        self.assertEqual(self.preflight.accepted_release.call_count, 2)
        self.assertEqual(self.f.storage.admit.call_count, 2)
        self.assertEqual(self.f.contract.bind_source.call_count, 2)
        self.assertEqual(self.f.builder.worker_identity.call_count, 2)
        self.assertEqual(self.B.authenticate.call_count, 2)

    def test_nonroot_rejects_before_loading_any_helper(self):
        with patch.object(C.os, 'geteuid', return_value=1000), patch.object(self.B, 'load') as imported:
            with self.assertRaisesRegex(ValueError, 'administrator'):
                C.verify_prepared(self.B, *([None]*16))
        imported.assert_not_called()


if __name__ == '__main__':unittest.main()
