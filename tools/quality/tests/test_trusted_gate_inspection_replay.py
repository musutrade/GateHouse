"""Retained passive inspection must still bind exact commands, replies and view.

Root/helper/host/prepared-state/unit boundaries are replaced explicitly. Metadata,
typed reply parsers, closed inventories and command comparison run in real files.
Separate small real-Git cases exercise readonly copied source and policy guards.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
import uuid

ROOT = Path(__file__).resolve().parents[3]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX = load('inspection_replay_fixture', 'tools/quality/tests/test_trusted_gate_core_inspection.py')
C = FIX.C;B0 = FIX.B0
FENCE = load('inspection_replay_lease', 'tools/trusted_gate/deploy/install-transaction.py')


class CompletedInspectionReplayTests(unittest.TestCase):
    def setUp(self):
        self.fixture = FIX.CoreInspectionTests();self.fixture.setUp();self.addCleanup(self.fixture.doCleanups)
        f = self.fixture;self.f = f.f;self.B = SimpleNamespace(**vars(f.B));self.destination = f.destination
        def copy_view(*args):
            (self.destination/'source').mkdir();(self.destination/'mirror.git').mkdir()
            self.B.write(self.destination/'candidate.bundle', self.f.payload['source.bundle'], 0o400)
            return f.view
        f.copy.side_effect = copy_view
        def run(B, builder, destination, label, argv, env):
            unit = 'gatehouse-core-inspection-' + str(uuid.uuid4()) + '.service'
            outer = C.unit_command(B, destination, label, argv, env, unit)
            B.write(destination/(label+'.stdout'), f.responses[label], 0o600)
            B.write(destination/(label+'.stderr'), b'', 0o600)
            B.record(destination/(label+'.unit.json'), {'unit': unit, 'argv': outer,
                'state': 'stopped', 'worker_stop_proven': True, 'exit_code': 0})
            return f.responses[label]
        f.run.side_effect = run;self.checked = f.inspect()
        self.pin = C.hashlib.sha256((self.destination/'binding.json').read_bytes()).hexdigest()
        self.prepared = {'schema': 'fixture-prepared', 'installation_admitted': False,
            'configuration_sha256': f.configuration['configuration_sha256'], 'plan_sha256': f.configuration['plan_sha256']}
        self.composer = SimpleNamespace(verify_prepared=Mock(side_effect=lambda *args: copy.deepcopy(self.prepared)))
        self.B.load = lambda helpers, name: {'compose-linux-host.py': self.composer,
            'build-protected-core.py': self.f.builder, 'stage-linux-inputs.py': FIX.STAGE,
            'bootstrap-release.py': self.f.contract, 'install-transaction.py': FENCE}[name]
        self.view = Mock(side_effect=lambda *args: copy.deepcopy(f.view))
        self.f.builder.stopped.reset_mock();self.B.authenticate.reset_mock()

    def replay(self, **options):
        f = self.f
        with patch.multiple(C, VOLUME=f.volume, CONTROL=f.control), patch.object(C.os, 'geteuid', return_value=0), \
             patch.object(FENCE, 'LEASE', f.control/'controller.lock'), patch.object(C, 'verified_view', self.view):
            with (f.control/'controller.lock').open('r+b') as lease:
                return C.verify_completed(self.B, f.root/'helpers', f.manifest, 'a'*64, f.archive,
                    C.hashlib.sha256(f.archive.read_bytes()).hexdigest(), 'b'*64, f.seal_pin, f.core_pin,
                    self.fixture.pin, self.pin, f.root/'host-recipe', 'c'*64, f.root/'volume-recipe', 'd'*64,
                    f.root/'retention', 'e'*64, options.pop('lock_fd', lease.fileno()), **options)

    def snapshot(self):
        return {str(path): (path.stat().st_mode, path.read_bytes())
                for path in self.f.root.rglob('*') if path.is_file()}

    def rebind(self, change=None):
        path = self.destination/'binding.json';saved = json.loads(path.read_bytes())
        if change is not None:change(saved)
        path.chmod(0o600);path.write_bytes(B0.canonical(saved));path.chmod(0o400)
        self.pin = C.hashlib.sha256(path.read_bytes()).hexdigest()
        status_path = self.destination/'inspection.json';status = json.loads(status_path.read_bytes())
        status['binding_sha256'] = self.pin;status_path.write_bytes(B0.canonical(status))

    def change_invocation(self, label, change):
        path = self.destination/(label+'.unit.json');value = json.loads(path.read_bytes());change(value)
        path.write_bytes(B0.canonical(value))
        self.rebind(lambda saved: saved['replies'][label].update(
            invocation_sha256=C.hashlib.sha256(path.read_bytes()).hexdigest()))

    def test_completed_replay_rechecks_three_stopped_units_without_execution_or_mutation(self):
        before = self.snapshot()
        with patch.object(self.B, 'write', side_effect=AssertionError('unexpected write')), \
             patch.object(self.B, 'record', side_effect=AssertionError('unexpected record')), \
             patch.object(C, 'run_command', side_effect=AssertionError('no new parser units')), \
             patch.object(C.subprocess, 'run', side_effect=AssertionError('no process or signalling')):
            result = self.replay()
        self.assertEqual(before, self.snapshot());self.assertEqual(result['state'], 'verified')
        self.assertIs(result['captured_core_policy_checked'], True)
        for flag in ('installation_admitted', 'administrator_ready', 'production_gate_enabled'):
            self.assertIs(result[flag], False)
        self.assertEqual(self.composer.verify_prepared.call_count, 2);self.assertEqual(self.view.call_count, 2)
        self.assertEqual(self.f.builder.stopped.call_count, 6);self.assertEqual(self.B.authenticate.call_count, 2)
        for call in self.composer.verify_prepared.call_args_list:self.assertEqual(call.args[9], self.fixture.pin)

    def test_wrong_or_missing_inherited_controller_descriptor_blocks(self):
        for descriptor in (None, True, -1):
            with self.assertRaisesRegex(ValueError, 'inherited'):self.replay(lock_fd=descriptor)
        with self.f.config_path.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError, 'lease'):self.replay(lock_fd=wrong.fileno())

    def test_wrong_external_inspection_pin_is_not_repaired_or_overwritten(self):
        before = self.snapshot();self.pin = 'f'*64
        with self.assertRaises(ValueError):self.replay()
        self.assertEqual(before, self.snapshot())

    def test_incomplete_blocked_or_mismatched_control_record_rejects(self):
        path = self.destination/'inspection.json';original = path.read_bytes()
        for state in ('incomplete', 'blocked'):
            value = json.loads(original);value['state'] = state;path.write_bytes(B0.canonical(value))
            with self.assertRaisesRegex(ValueError, 'completed control'):self.replay()
        value = json.loads(original);value['binding_sha256'] = 'f'*64;path.write_bytes(B0.canonical(value))
        with self.assertRaisesRegex(ValueError, 'completed control'):self.replay()

    def test_unknown_inventory_or_metadata_modes_reject(self):
        extra = self.destination/'unreviewed.json';self.B.write(extra, b'unknown', 0o600)
        with self.assertRaisesRegex(ValueError, 'inventory'):self.replay()
        extra.unlink();(self.destination/'binding.json').chmod(0o600)
        with self.assertRaisesRegex(ValueError, 'mode'):self.replay()

    def test_rehashed_binding_cannot_promote_quality_or_change_prepared_identity(self):
        original = (self.destination/'binding.json').read_bytes()
        for key, value in (('administrator_ready', True), ('production_gate_enabled', True),
                           ('coverage_acceptance', 'complete'), ('authoritative_publish', 'allowed'),
                           ('head', 'f'*40), ('configuration_sha256', 'f'*64), ('plan_sha256', 'f'*64)):
            path = self.destination/'binding.json';path.chmod(0o600);path.write_bytes(original);path.chmod(0o400)
            self.rebind(lambda saved: saved.update({key: value}))
            with self.assertRaisesRegex(ValueError, 'binding differs'):self.replay()

    def test_seconds_is_observation_only_and_boolean_or_negative_cannot_pass(self):
        for value in (True, -1, '1.0'):
            self.rebind(lambda saved: saved.update(seconds=value))
            with self.assertRaisesRegex(ValueError, 'observation time'):self.replay()

    def test_rehashed_changed_exit_or_stop_claim_is_not_accepted(self):
        for key, value in (('exit_code', False), ('exit_code', 1), ('worker_stop_proven', False), ('state', 'running')):
            original = (self.destination/'configuration.unit.json').read_bytes()
            self.change_invocation('configuration', lambda unit: unit.update({key: value}))
            with self.assertRaisesRegex(ValueError, 'invocation contract'):self.replay()
            (self.destination/'configuration.unit.json').write_bytes(original)

    def test_rehashed_widened_sandbox_properties_or_environment_are_rejected(self):
        path = self.destination/'configuration.unit.json';original = path.read_bytes()
        for before, after in (('--property=PrivateNetwork=yes', '--property=PrivateNetwork=no'),
                              ('--property=User=root', '--property=User=gem'),
                              ('--property=NoNewPrivileges=yes', '--property=NoNewPrivileges=no'),
                              ('GIT_CONFIG_GLOBAL=/dev/null', 'GIT_CONFIG_GLOBAL=/home/gem/.gitconfig')):
            path.write_bytes(original)
            def change(unit):unit['argv'] = [after if arg == before else arg for arg in unit['argv']]
            self.change_invocation('configuration', change)
            with self.assertRaisesRegex(ValueError, 'invocation contract'):self.replay()

    def test_rehashed_command_cannot_execute_quality_or_another_profile(self):
        self.change_invocation('full', lambda unit: unit['argv'].remove('--plan'))
        with self.assertRaisesRegex(ValueError, 'invocation contract'):self.replay()

    def test_reused_or_non_uuid4_unit_identity_rejects(self):
        full = json.loads((self.destination/'full.unit.json').read_bytes())['unit']
        def reuse(unit):
            old = unit['unit'];unit['unit'] = full
            unit['argv'] = ['--unit='+full if arg == '--unit='+old else arg for arg in unit['argv']]
        self.change_invocation('hook', reuse)
        with self.assertRaisesRegex(ValueError, 'reused a unit'):self.replay()
        self.change_invocation('configuration', lambda unit: unit.update(
            unit='gatehouse-core-inspection-00000000-0000-0000-0000-000000000000.service'))
        with self.assertRaises(ValueError):self.replay()

    def test_stale_stop_receipt_cannot_replace_current_stopped_observation(self):
        self.f.builder.stopped.side_effect = ValueError('unit is active or unknown')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'active or unknown'):self.replay()
        self.assertEqual(before, self.snapshot())

    def test_rehashed_failed_config_report_and_waived_hook_plan_reject(self):
        path = self.destination/'configuration.stdout';original = path.read_bytes()
        response = {'schema_version': 1, 'valid': False, 'truncated': False, 'diagnostics': []}
        path.write_bytes(B0.canonical(response))
        self.rebind(lambda saved: saved['replies']['configuration'].update(
            sha256=C.hashlib.sha256(path.read_bytes()).hexdigest()))
        with self.assertRaisesRegex(ValueError, 'configuration is invalid'):self.replay()
        path.write_bytes(original);hook = self.destination/'hook.stdout'
        response = FIX.PROTO.envelope('hook');response['plan']['nodes'].pop();hook.write_bytes(B0.canonical(response))
        with self.assertRaisesRegex(ValueError, 'node inventory'):self.replay()

    def test_nonempty_stderr_and_oversized_or_non_json_response_reject(self):
        stderr = self.destination/'configuration.stderr';stderr.write_bytes(b'private failure retained')
        with self.assertRaisesRegex(ValueError, 'stderr'):self.replay()
        stderr.write_bytes(b'');path = self.destination/'configuration.stdout';path.write_bytes(b'not-json')
        with self.assertRaises(ValueError):self.replay()
        path.write_bytes(b'x'*(C.MAX_REPLY+1))
        with self.assertRaises(ValueError):self.replay()

    def test_missing_view_or_payload_provenance_blocks_before_return(self):
        self.view.side_effect = ValueError('view no longer source-bound')
        with self.assertRaisesRegex(ValueError, 'source-bound'):self.replay()

    def test_mismatched_or_rehashed_view_metadata_cannot_replace_saved_tree(self):
        path = self.destination/'view.json';path.chmod(0o600);path.write_bytes(B0.canonical({'different': 'view'}));path.chmod(0o400)
        self.rebind(lambda saved: saved.update(view_sha256=C.hashlib.sha256(path.read_bytes()).hexdigest()))
        with self.assertRaisesRegex(ValueError, 'view changed'):self.replay()

    def test_changed_prepared_state_or_late_response_mutation_cannot_return_verified(self):
        self.composer.verify_prepared.side_effect = [self.prepared, {**self.prepared, 'plan_sha256': 'f'*64}]
        with self.assertRaisesRegex(ValueError, 'configuration changed'):self.replay()
        def review(*args):
            if self.view.call_count == 1:(self.destination/'configuration.stdout').write_bytes(b'changed after first review')
            return copy.deepcopy(self.prepared)
        self.composer.verify_prepared.side_effect = review
        with self.assertRaises(ValueError):self.replay()

    def test_final_authentication_or_controller_inode_drift_rejects_result(self):
        self.B.authenticate.side_effect = [{'revision': self.f.head}, ValueError('helper changed')]
        with self.assertRaisesRegex(ValueError, 'helper changed'):self.replay()
        self.B.authenticate.side_effect = None
        with patch.object(FENCE, 'lease', side_effect=[None, ValueError('lease changed')]):
            with self.assertRaisesRegex(ValueError, 'lease changed'):self.replay()

    def test_nonroot_fails_before_helper_import(self):
        with patch.object(C.os, 'geteuid', return_value=1000), patch.object(self.B, 'load') as imported:
            with self.assertRaisesRegex(ValueError, 'administrator'):
                C.verify_completed(self.B, *([None]*17))
        imported.assert_not_called()


class InspectionViewReplayTests(unittest.TestCase):
    def setUp(self):
        self.fixture = FIX.CoreInspectionTests();self.fixture.setUp();self.addCleanup(self.fixture.doCleanups)
        f = self.fixture;self.f = f.f;self.destination = f.destination;self.destination.mkdir()
        def git(where, *args):
            return subprocess.check_output(['/usr/bin/git', '-C', str(where), *args], env=B0.ENV, stderr=subprocess.PIPE)
        self.git = git;mirror = self.f.source_root/'mirror.git'
        git(self.f.root, 'clone', '--mirror', '--no-hardlinks', str(self.f.source), str(mirror))
        bundle = self.f.root/'inspection.bundle';git(self.f.source, 'bundle', 'create', str(bundle), 'HEAD')
        self.payload = {**self.f.payload, 'source.bundle': bundle.read_bytes()}
        self.context = SimpleNamespace(**vars(f.B));self.real_owned = B0.owned
        self.context.owned = self.owned;self.context.frozen_lock = B0.frozen_lock
        self.context.readonly = B0.readonly;self.context.git = B0.git
        with patch.object(B0, 'owned', side_effect=self.owned), patch.multiple(C, VOLUME=self.f.volume, BASE=self.f.head):
            self.expected = C.copy_view(self.context, self.f.root/'helpers', self.payload, self.f.head, self.destination)

    def owned(self, path, **kwargs):
        path = Path(path)
        return self.f.B.owned(path, **kwargs) if path == self.f.root or self.f.root in path.parents else self.real_owned(path, **kwargs)

    def verify(self):
        with patch.object(B0, 'owned', side_effect=self.owned), patch.multiple(C, VOLUME=self.f.volume, BASE=self.f.head):
            return C.verified_view(self.context, FIX.STAGE, self.f.contract, self.payload,
                                   self.destination, self.f.head, 'a'*64)

    def test_actual_independent_git_and_policy_view_replays_without_source_mutation(self):
        before = self.git(self.f.source, 'status', '--porcelain')
        self.assertEqual(self.verify(), self.expected)
        self.assertEqual(self.git(self.f.source, 'status', '--porcelain'), before)
        self.assertFalse((self.f.source/'.gatehouse-host').exists())

    def test_changed_readonly_mode_is_rejected_even_when_inventory_hash_is_identical(self):
        path = self.destination/'source/test-data/manifest.json';path.chmod(0o644)
        with patch.object(B0, 'owned', side_effect=self.owned), patch.multiple(C, VOLUME=self.f.volume, BASE=self.f.head):
            self.assertEqual(C.view_inventory(self.context, FIX.STAGE, self.destination, self.f.head), self.expected)
        with self.assertRaisesRegex(ValueError, 'no longer readonly'):self.verify()

    def test_resealed_policy_bytes_cannot_be_rebound_by_rehashing_the_view(self):
        path = self.destination/'source/.gatehouse-host/flow.toml';path.chmod(0o600);path.write_bytes(b'waived flow');path.chmod(0o444)
        with self.assertRaisesRegex(ValueError, 'policy differs'):self.verify()

    def test_other_modified_tracked_file_or_unexpected_overlay_blocks(self):
        path = self.destination/'source/test-data/manifest.json';raw = path.read_bytes()
        path.chmod(0o600);path.write_bytes(b'changed tracked input');path.chmod(0o444)
        with self.assertRaisesRegex(ValueError, 'working tree changed'):self.verify()
        path.chmod(0o600);path.write_bytes(raw);path.chmod(0o444)
        overlay = self.destination/'source/.gatehouse-host';overlay.chmod(0o700)
        B0.write(overlay/'extra.toml', b'unknown policy', 0o444);overlay.chmod(0o555)
        with self.assertRaisesRegex(ValueError, 'policy inventory'):self.verify()

    def test_different_bundle_cannot_be_hidden_in_a_rehashed_view(self):
        path = self.destination/'candidate.bundle';path.chmod(0o600);path.write_bytes(b'another bundle');path.chmod(0o400)
        with self.assertRaisesRegex(ValueError, 'bundle differs'):self.verify()


if __name__ == '__main__':unittest.main()
