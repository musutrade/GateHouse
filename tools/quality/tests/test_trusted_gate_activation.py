"""Real file modes and installed Snapshot; Root/preparation/native/API borders replaced."""
from contextlib import contextmanager
import copy
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT/path)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX = load('activation_file_fixture', 'tools/quality/tests/test_trusted_gate_file_install.py')
A = load('activation_library', 'tools/trusted_gate/deploy/activate-linux-controller.py')
HOST = load('activation_installed_host', 'tools/trusted_gate/host.py')
I = FIX.I;F = FIX.FENCE;BASIC = FIX.BOOT


class ForwardActivationTests(unittest.TestCase):
    def setUp(self):
        self.m = FIX.ReviewedFileTransactionTests();self.m.setUp();self.addCleanup(self.m.doCleanups)
        f = self.f = self.m.f;self.B = f.B
        self.patch('persist_storage', Mock());self.patch('observe_storage', Mock())
        self.source = f.root/'installed/opt/gatehouse/trusted_gate'
        self.interpreter = f.root/'python3.14'
        BASIC.write(self.interpreter, b'fixture interpreter, not executed', 0o755)
        pins = {str(self.interpreter):A.digest(self.interpreter.read_bytes())}
        for name in HOST.MODULES:
            raw = (ROOT/'tools/trusted_gate'/(name+'.py')).read_bytes();target = str(self.source/(name+'.py'))
            self.m.materials['files'][target]['after'] = raw;pins[target] = A.digest(raw)
        paths = {name:f.root/name for name in ('records', 'mirror', 'source', 'inbox')}
        for path in paths.values():path.mkdir(mode=0o700)
        policy = f.root/'installed/etc/gatehouse/policy'
        self.config = {'schema':'gatehouse-trusted-host/v1', 'repository':'musutrade/GateHouse',
            'base_branch':'main', 'profile':'full', 'coverage_acceptance':'transition', 'app_id':4867361,
            'developer_uid':999, 'freshness_seconds':3600, 'timeout_seconds':1800,
            'environment':{'source_revision':f.head, 'bootstrap_helper_manifest_sha256':'a'*64}, 'protected_files':pins,
            **{name:str(path) for name,path in paths.items()}, 'scratch':str(f.root/'scratch'),
            'policy_directory':str(policy), 'cache':{'root':str(f.root/'cache')}}
        raw = BASIC.canonical(self.config);self.m.materials['files'][str(f.config)]['after'] = raw
        original = F.read_receipt(self.B);original['configuration_sha256'] = A.digest(raw)
        BASIC.record(f.receipt, original);BASIC.record(f.barrier, F.marker(self.B, original, 'fenced'))
        self.m.prepare();self.m.resume()
        self.receipt = f.root/'controller-activation.json';self.upgrade_receipt = f.root/'linux-upgrade.json'
        old = {'phase':'installed-disabled', 'head':f.head, 'transaction':f.transaction,
               'lease':[os.fstat(f.lease_fd).st_dev, os.fstat(f.lease_fd).st_ino],
               'entry_fence_sha256':A.digest(BASIC.canonical(original))}
        BASIC.record(self.upgrade_receipt, old)
        self.ctx = SimpleNamespace(B=self.B, fd=f.lease_fd, material=self.m.materials, files=I,
            fence=F, contract=self.m.contract, lifecycle=SimpleNamespace(receipt=Mock(return_value=old),
                RECEIPT=self.upgrade_receipt), manifest_pin='a'*64, recipe_pin='b'*64,
            material_pin=A.digest(BASIC.canonical(I.descriptor(self.B, self.m.contract, self.m.materials))),
            guard=Mock(), observe_files=Mock(side_effect=self.disabled), new_config=self.config, new_payload={},
            code={'revision':f.head}, helpers=f.root/'helpers', manifest=f.root/'manifest',
            value={'pins':{'preservation':'c'*64}, 'api_proxy':None})
        self.effect = {'phase':'retired', 'head':f.head, 'helper_manifest_sha256':'a'*64,
            'preservation_sha256':'c'*64, 'remote_revocation_confirmed':True, 'retained_publication_verified':True,
            'api_transport_sha256':A.digest(BASIC.canonical({'https_proxy':None})),
            'installation_admitted':False, 'administrator_ready':False, 'production_gate_enabled':False}
        self.ctx.effect = Mock(side_effect=lambda:copy.deepcopy(self.effect))
        self.ctx.inputs = SimpleNamespace(inspection=Mock(return_value={'prepared':'verified boundary'}),
            materials=Mock(side_effect=lambda *args:copy.deepcopy(self.m.materials)))
        self.ctx.checker = SimpleNamespace()
        self.runtime = Mock(return_value={'files':1, 'bytes':1})
        self.ctx.compose = SimpleNamespace(runtime_verification=self.runtime)
        self.scans = Mock(return_value=[]);self.context = Mock(return_value={'mnt':'mnt:[1]'})
        self.ctx.drain = SimpleNamespace(proc_context=self.context, ProcView=lambda:SimpleNamespace(scan=self.scans))
        # The real Snapshot compiles installed source/configuration. Substitute
        # only Root/DAC/account admission, not the barrier or source-byte checks.
        self.host_factory = A.host_module;self.snapshot_calls = []
        self.patch('SOURCE', self.source);self.patch('INTERPRETER', self.interpreter);self.patch('RECEIPT', self.receipt)
        context = patch.object(A, 'host_module', side_effect=self.host);context.start();self.addCleanup(context.stop)
        real_open = os.open
        def root_open(path, flags, *args, **kwargs):
            p = Path(path) if isinstance(path, (str, Path)) else None
            if p in f.entries and flags == os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK:
                info = p.stat();key = (info.st_dev, info.st_ino)
                if key in self.m.extra_fds:
                    fd = os.dup(self.m.extra_fds[key]);os.lseek(fd, 0, os.SEEK_SET);return fd
            return real_open(path, flags, *args, **kwargs)
        context = patch.object(os, 'open', side_effect=root_open);context.start();self.addCleanup(context.stop)

    def patch(self, name, value):
        context = patch.object(A, name, value);context.start();self.addCleanup(context.stop)

    def disabled(self, phase):
        self.assertEqual(phase, 'installed-disabled')
        I.inspect(self.B, F, self.m.contract, self.m.materials, self.m.read_state(), F.read_receipt(self.B), self.ctx.fd)

    def host(self, ctx):
        host = self.host_factory(ctx)
        def protected(path, *, directory=False, mode=None):
            self.B.owned(path, directory=directory)
            info = Path(path).stat();self.B.need(mode is None or stat.S_IMODE(info.st_mode) == mode, 'fixture host mode')
            return host.identity(info)
        host.protected = protected;real = host.SourceFinder.exec_module;fixture = self
        class Finder(host.SourceFinder):
            def exec_module(self, module):
                real(self, module)
                if module.__name__ == 'trusted_gate.common':
                    module.protected = fixture.B.owned
                    module.submission_inbox = lambda path:fixture.B.owned(path, directory=True)
                elif module.__name__ == 'trusted_gate.cli':
                    module.CONFIG = fixture.f.config;module.unprivileged_identity = Mock()
                elif module.__name__ == 'trusted_gate.journal':
                    module.Journal = Mock(side_effect=AssertionError('activation must not construct Journal'))
                elif module.__name__ == 'trusted_gate.github':
                    module.GitHub = Mock(side_effect=AssertionError('activation must not create network client'))
        host.SourceFinder = Finder
        snapshot = host.Snapshot
        def actual(*args):
            self.snapshot_calls.append(json.loads(self.f.barrier.read_bytes())['phase'])
            return snapshot(*args)
        host.Snapshot = actual;return host

    def execute(self):
        if not self.receipt.exists():A.prepare(self.ctx)
        return A.resume(self.ctx)

    def state(self):return json.loads(self.receipt.read_bytes())

    def test_storage_persistence_failure_precedes_active_marker_and_entry_enablement(self):
        A.prepare(self.ctx)
        with patch.object(A,'persist_storage',side_effect=ValueError('storage not persistent')):
            with self.assertRaisesRegex(ValueError,'not persistent'):A.resume(self.ctx)
        self.assertEqual(self.state()['phase'],'prepared')
        self.assertEqual(json.loads(self.f.barrier.read_bytes())['phase'],'fenced')
        self.assertTrue(all(stat.S_IMODE(path.stat().st_mode)==0 for path in self.f.entries))

    def test_storage_persistence_rechecked_after_active_marker_before_entry_modes(self):
        def storage(ctx,value):
            if json.loads(self.f.barrier.read_bytes())['phase']=='active':raise ValueError('storage link disappeared')
        with patch.object(A,'observe_storage',side_effect=storage),self.assertRaisesRegex(ValueError,'link disappeared'):
            self.execute()
        self.assertEqual(self.state()['phase'],'marker-intent')
        self.assertTrue(all(stat.S_IMODE(path.stat().st_mode)==0 for path in self.f.entries))
        self.assertEqual(self.execute()['phase'],'entries-enabled')

    def test_actual_snapshot_configuration_and_internal_first_public_last_modes(self):
        changes = [];real = os.fchmod
        def chmod(fd, mode):
            if mode == 0o755:
                info = os.fstat(fd)
                changes.append(next(path for path in self.f.entries if path.stat().st_ino == info.st_ino))
            return real(fd, mode)
        with patch.object(os, 'fchmod', side_effect=chmod):result = self.execute()
        self.assertEqual(changes, [self.f.entries[1], self.f.entries[0]])
        self.assertEqual(result['phase'], 'entries-enabled');self.assertTrue(self.snapshot_calls)
        self.assertEqual(set(self.snapshot_calls), {'active'})
        for path in self.f.entries:
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o755)
            self.assertEqual(stat.S_IMODE(os.fstat(self.f.entry_fds[path]).st_mode), 0)
        self.assertFalse(result['administrator_ready']);self.assertFalse(result['production_gate_enabled'])
        self.assertFalse(any(name == 'trusted_gate' or name.startswith('trusted_gate.') for name in sys.modules))

    def test_completion_retry_revalidates_without_mode_or_metadata_mutation(self):
        expected = self.execute();raw = self.receipt.read_bytes()
        with patch.object(os, 'fchmod', side_effect=AssertionError('mode write')), \
             patch.object(self.B, 'record', side_effect=AssertionError('metadata write')):
            self.assertEqual(self.execute(), expected)
        self.assertEqual(raw, self.receipt.read_bytes())

    def test_unknown_marker_or_reopened_entry_before_intent_blocks(self):
        A.prepare(self.ctx);value = json.loads(self.f.barrier.read_bytes());value['phase'] = 'active'
        BASIC.record(self.f.barrier, value)
        with self.assertRaisesRegex(ValueError, 'unknown or moved backwards'):self.execute()
        value['phase'] = 'fenced';BASIC.record(self.f.barrier, value);self.f.entries[1].chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'outside its durable intent'):self.execute()

    def test_lost_active_marker_replace_response_recovers_before_enabling_entries(self):
        real = os.replace
        def replace(source, target):
            real(source, target)
            if Path(target) == self.f.barrier:raise OSError('active marker response lost')
        with patch.object(os, 'replace', side_effect=replace), self.assertRaisesRegex(OSError, 'response lost'):
            self.execute()
        self.assertEqual(self.state()['phase'], 'marker-intent')
        self.assertTrue(all(stat.S_IMODE(path.stat().st_mode) == 0 for path in self.f.entries))
        self.assertEqual(self.execute()['phase'], 'entries-enabled')

    def test_partial_marker_metadata_is_retained_and_blocks_recovery(self):
        real = self.B.record
        def record(path, value):
            if path == self.f.barrier:
                BASIC.write(path.with_name('.'+path.name+'.tmp'), b'partial active marker', 0o600)
                raise OSError('marker interrupted')
            return real(path, value)
        with patch.object(self.B, 'record', side_effect=record), self.assertRaisesRegex(OSError, 'marker interrupted'):
            self.execute()
        with self.assertRaisesRegex(ValueError, 'unfinished'):self.execute()
        self.assertTrue(self.f.barrier.with_name('.'+self.f.barrier.name+'.tmp').exists())
        self.assertTrue(all(stat.S_IMODE(path.stat().st_mode) == 0 for path in self.f.entries))

    def mode_crash(self, target):
        real = os.fchmod
        def chmod(fd, mode):
            real(fd, mode)
            if mode == 0o755 and os.fstat(fd).st_ino == target.stat().st_ino:raise OSError('chmod response lost')
        with patch.object(os, 'fchmod', side_effect=chmod), self.assertRaisesRegex(OSError, 'chmod response lost'):
            self.execute()

    def test_internal_chmod_crash_keeps_public_disabled_and_resumes_known_intent(self):
        self.mode_crash(self.f.entries[1]);self.assertEqual(self.state()['phase'], 'internal-intent')
        self.assertEqual(stat.S_IMODE(self.f.entries[0].stat().st_mode), 0)
        self.assertEqual(self.execute()['phase'], 'entries-enabled')

    def test_public_chmod_crash_resumes_only_known_public_intent(self):
        self.mode_crash(self.f.entries[0]);self.assertEqual(self.state()['phase'], 'public-intent')
        self.assertEqual(self.execute()['phase'], 'entries-enabled')

    def test_executable_public_intent_cannot_run_controller_before_completion(self):
        self.mode_crash(self.f.entries[0]);host = self.host(self.ctx)
        snapshot = host.Snapshot(self.source, self.f.config, self.interpreter, self.f.barrier, self.receipt)
        # Source/configuration can be inspected, but CLI import/Journal/network
        # must be unreachable while the durable activation is incomplete.
        with patch.object(host, 'SourceFinder', side_effect=AssertionError('CLI import before completion')), \
             self.assertRaises(host.HostBlocked):host.run(snapshot, ['status'])
        self.assertFalse(any(name == 'trusted_gate' or name.startswith('trusted_gate.') for name in sys.modules))
        self.execute();snapshot.arm();snapshot.verify()

    def test_snapshot_failure_after_active_marker_keeps_both_entries_disabled(self):
        with patch.object(A, 'snapshot_configuration', side_effect=ValueError('actual Snapshot unavailable')), \
             self.assertRaisesRegex(ValueError, 'Snapshot unavailable'):self.execute()
        self.assertEqual(self.state()['phase'], 'marker-intent')
        self.assertTrue(all(stat.S_IMODE(path.stat().st_mode) == 0 for path in self.f.entries))
        self.assertEqual(self.execute()['phase'], 'entries-enabled')

    def test_runtime_failure_precedes_active_marker_and_modes(self):
        self.runtime.side_effect = ValueError('runtime binding changed')
        with self.assertRaisesRegex(ValueError, 'runtime binding'):self.execute()
        self.assertEqual(self.state()['phase'], 'prepared')
        self.assertEqual(json.loads(self.f.barrier.read_bytes())['phase'], 'fenced')

    def test_runtime_configuration_cannot_diverge_from_installed_recipe_materials(self):
        self.config['repository'] = 'other/repository'
        with self.assertRaisesRegex(ValueError, 'runtime configuration differs'):self.execute()
        self.assertEqual(self.state()['phase'], 'prepared')
        self.assertEqual(json.loads(self.f.barrier.read_bytes())['phase'], 'fenced')
        self.assertTrue(all(stat.S_IMODE(path.stat().st_mode) == 0 for path in self.f.entries))

    def test_nonentry_mutation_replacement_or_unknown_source_blocks(self):
        A.prepare(self.ctx);path = self.source/'runtime.py';path.write_bytes(b'changed')
        with self.assertRaises((ValueError, OSError)):self.execute()
        self.assertEqual(self.state()['phase'], 'prepared')

    def test_replaced_entry_with_equal_bytes_and_mode_is_rejected(self):
        A.prepare(self.ctx);path = self.f.entries[0];raw = self.B.read(path, 1024**2)
        temp = path.with_name('replacement');BASIC.write(temp, raw, 0)
        os.replace(temp, path)
        with self.assertRaisesRegex(ValueError, 'inode changed'):self.execute()

    def test_remote_change_native_scan_change_or_prepared_material_change_blocks(self):
        self.effect['remote_revocation_confirmed'] = False
        with self.assertRaisesRegex(ValueError, 'remote/publication'):self.execute()
        self.effect['remote_revocation_confirmed'] = True;self.scans.side_effect = [[], [{'pid':77}]]
        with self.assertRaisesRegex(ValueError, 'native processes changed'):self.execute()
        self.scans.side_effect = None
        self.ctx.inputs.materials.side_effect = lambda *args:{**self.m.materials, 'head':'f'*40}
        with self.assertRaises(ValueError):self.execute()

    def test_receipt_generation_mode_and_partial_metadata_remain_bound(self):
        A.prepare(self.ctx);state = self.state();state['recipe_sha256'] = 'f'*64;BASIC.record(self.receipt, state)
        with self.assertRaisesRegex(ValueError, 'another installed generation'):self.execute()
        self.receipt.unlink();BASIC.write(self.receipt.with_name('.'+self.receipt.name+'.tmp'), b'partial', 0o600)
        with self.assertRaisesRegex(ValueError, 'existing controller activation'):self.execute()

    def test_actual_snapshot_rejects_inactive_marker_and_missing_pinned_module(self):
        host = self.host(self.ctx)
        with self.assertRaises(host.HostBlocked):A.snapshot_configuration(self.ctx, host)
        self.assertFalse(any(name == 'trusted_gate' or name.startswith('trusted_gate.') for name in sys.modules))
        (self.source/'runtime.py').unlink()
        with self.assertRaises((ValueError, OSError)):self.execute()


if __name__ == '__main__':unittest.main()
