"""Source/configuration generation regressions; ordinary files, no live Root admission."""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import py_compile
import runpy
import stat
import sys
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
import uuid
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('controller_bootstrap_fixture', ROOT / 'tools/trusted_gate/host.py')
H = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(H)


class ControllerSourceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.root = self.directory / 'trusted_gate'
        self.root.mkdir(mode=0o755)
        self.root.chmod(0o755)
        for name in H.MODULES:
            raw = (ROOT / 'tools/trusted_gate' / (name + '.py')).read_bytes()
            (self.root / (name + '.py')).write_bytes(raw)
            (self.root / (name + '.py')).chmod(0o644)
        self.write('cli', b'from .common import controller_configuration\ndef main(argv):\n'
                   b'    return controller_configuration(argv[0])["value"]\n')
        self.interpreter = self.directory / 'python3.14'
        self.interpreter.write_bytes(b'ordinary interpreter pin fixture, never executed')
        self.interpreter.chmod(0o755)
        self.config_path = self.directory / 'host.json'
        self.barrier_path = self.directory / 'installation.json'
        self.activation_path = self.directory / 'controller-activation.json'
        self.pin()
        # Replace Root ownership/ancestor admission only. Real ordinary file, link,
        # mode, inode, timestamp, byte limits and loader operations remain exercised.
        self.guard = patch.object(H, 'protected', side_effect=self.ordinary_protected)
        self.guard.start()
        self.addCleanup(self.guard.stop)

    def ordinary_protected(self, path, *, directory=False, mode=None):
        H.need(path.is_absolute() and '..' not in path.parts and (path == self.directory or self.directory in path.parents))
        for parent in (path, *path.parents):
            if parent != self.directory and self.directory not in parent.parents:
                continue
            info = parent.lstat()
            H.need(not stat.S_ISLNK(info.st_mode) and not info.st_mode & 0o022)
        info = path.lstat()
        H.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink == 1)
        H.need(mode is None or stat.S_IMODE(info.st_mode) == mode)
        return H.identity(info)

    def write(self, module, raw):
        path = self.root / (module + '.py')
        path.write_bytes(raw)
        path.chmod(0o644)

    def pin(self, value=7):
        pins = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in self.root.glob('*.py')}
        pins[str(self.interpreter)] = hashlib.sha256(self.interpreter.read_bytes()).hexdigest()
        self.config = {'value': value, 'protected_files': pins}
        self.config_path.write_text(json.dumps(self.config))
        self.config_path.chmod(0o600)

    def snapshot(self):
        return H.Snapshot(self.root, self.config_path, self.interpreter, self.barrier_path, self.activation_path)

    def execute(self):
        return H.run(self.snapshot(), [str(self.config_path)])

    def barrier(self, phase='active'):
        self.config['environment'] = {'source_revision': 'a' * 40, 'bootstrap_helper_manifest_sha256': 'a' * 64}
        self.config_path.write_text(json.dumps(self.config))
        value = {'schema': 'gatehouse-installation-barrier/v1', 'transaction': str(uuid.uuid4()),
                 'phase': phase, 'head': 'a' * 40, 'configuration_sha256': hashlib.sha256(self.config_path.read_bytes()).hexdigest(),
                 'plan_sha256': 'b' * 64}
        self.write_barrier(value)
        activation = {'schema':'gatehouse-controller-activation/v1', 'head':value['head'],
            'transaction':value['transaction'], 'lease':[1,2], 'phase':'entries-enabled',
            'administrator_ready':False, 'production_gate_enabled':False,
            **{name:'a'*64 for name in ('helper_manifest_sha256','recipe_sha256','material_sha256',
                 'upgrade_receipt_sha256','file_receipt_sha256','entry_fence_sha256')}}
        self.activation_path.write_bytes(json.dumps(activation,sort_keys=True,separators=(',',':')).encode()+b'\n')
        self.activation_path.chmod(0o600)
        return value

    def write_barrier(self, value):
        self.barrier_path.write_bytes(json.dumps(value, sort_keys=True, separators=(',', ':')).encode() + b'\n')
        self.barrier_path.chmod(0o600)

    def test_active_installation_barrier_matches_pinned_configuration_and_source_head(self):
        self.barrier()
        self.assertEqual(self.execute(), 7)

    def test_incomplete_activation_blocks_effectful_run_before_any_cli_import(self):
        self.barrier();value=json.loads(self.activation_path.read_bytes())
        for phase in ('prepared','marker-intent','marker-active','internal-intent','internal-enabled','public-intent'):
            value['phase']=phase
            self.activation_path.write_bytes(json.dumps(value,sort_keys=True,separators=(',',':')).encode()+b'\n')
            snapshot=self.snapshot();snapshot.verify()
            with patch.object(H,'SourceFinder',side_effect=AssertionError('CLI import')),self.assertRaises(H.HostBlocked):
                H.run(snapshot,[])

    def test_active_marker_requires_activation_and_binds_head_transaction_manifest_and_flags(self):
        for change in ({'head':'b'*40},{'transaction':str(uuid.uuid4())},{'helper_manifest_sha256':'b'*64},
                       {'administrator_ready':True},{'production_gate_enabled':True},{'lease':[1,True]},
                       {'extra':'unknown'}):
            self.barrier();value=json.loads(self.activation_path.read_bytes());value.update(change)
            self.activation_path.write_bytes(json.dumps(value,sort_keys=True,separators=(',',':')).encode()+b'\n')
            with self.assertRaises(H.HostBlocked):self.execute()
        self.barrier();self.activation_path.unlink()
        with self.assertRaises(OSError):self.execute()

    def test_armed_snapshot_cannot_readopt_changed_activation_or_drop_completion(self):
        self.barrier();snapshot=self.snapshot();snapshot.arm();raw=self.activation_path.read_bytes()
        self.activation_path.unlink()
        with self.assertRaises(OSError):H.run(snapshot,[])
        self.activation_path.write_bytes(raw);self.activation_path.chmod(0o600)
        with self.assertRaises(H.HostBlocked):H.run(snapshot,[])

    def test_legacy_without_barrier_rejects_orphan_activation_and_partial_metadata(self):
        self.activation_path.write_bytes(b'orphan');self.activation_path.chmod(0o600)
        with self.assertRaises(H.HostBlocked):self.execute()
        self.activation_path.unlink()
        self.activation_path.with_name('.'+self.activation_path.name+'.tmp').write_bytes(b'partial')
        with self.assertRaises(H.HostBlocked):self.execute()

    def test_activation_metadata_mode_noncanonical_json_and_link_rejects(self):
        self.barrier();self.activation_path.chmod(0o644)
        with self.assertRaises(H.HostBlocked):self.execute()
        self.activation_path.chmod(0o600);value=json.loads(self.activation_path.read_bytes())
        self.activation_path.write_text(json.dumps(value))
        with self.assertRaises(H.HostBlocked):self.execute()
        self.activation_path.unlink();self.activation_path.symlink_to(self.config_path)
        with self.assertRaises(H.HostBlocked):self.execute()

    def test_every_incomplete_barrier_blocks_before_configuration_read(self):
        original = H.read
        for phase in ('fencing', 'fenced', 'retired', 'installing', 'installed-fenced', 'unknown'):
            self.barrier(phase)
            with self.subTest(phase=phase), patch.object(H, 'read', wraps=original) as reader, self.assertRaises(H.HostBlocked):
                self.snapshot()
            self.assertNotIn(self.config_path, [call.args[0] for call in reader.call_args_list])

    def test_active_marker_wrong_generation_or_unknown_fields_rejected(self):
        for mutate in (lambda v:v.update(head='c'*40), lambda v:v.update(configuration_sha256='c'*64),
                       lambda v:v.update(plan_sha256=True), lambda v:v.update(transaction=str(uuid.uuid1())),
                       lambda v:v.update(extra='unknown')):
            value=self.barrier();mutate(value);self.write_barrier(value)
            with self.assertRaises(H.HostBlocked):self.snapshot()

    def test_new_barrier_after_snapshot_blocks_loaded_process(self):
        snapshot=self.snapshot();self.barrier('fencing')
        with self.assertRaises(H.HostBlocked):H.run(snapshot, [])
        self.assertNotIn('trusted_gate', sys.modules)

    def test_active_barrier_disappearance_or_replacement_rejects_existing_process(self):
        self.barrier();snapshot=self.snapshot();raw=self.barrier_path.read_bytes()
        self.barrier_path.unlink()
        with self.assertRaises(H.HostBlocked):H.run(snapshot, [])
        self.barrier_path.write_bytes(raw);self.barrier_path.chmod(0o600)
        with self.assertRaises(H.HostBlocked):H.run(snapshot, [])

    def test_barrier_bounds_mode_symlink_and_duplicate_json_fail_closed(self):
        self.barrier();self.barrier_path.chmod(0o644)
        with self.assertRaises(H.HostBlocked):self.snapshot()
        self.barrier_path.chmod(0o600)
        for raw in (b'x'*4097,b'{"phase":"active","phase":"fenced"}\n'):
            self.barrier_path.write_bytes(raw)
            with self.assertRaises((H.HostBlocked, ValueError)):self.snapshot()
        self.barrier_path.unlink();self.barrier_path.symlink_to(self.config_path)
        with self.assertRaises(H.HostBlocked):self.snapshot()

    def test_active_barrier_noncanonical_bytes_or_bad_environment_rejected(self):
        value=self.barrier();self.barrier_path.write_text(json.dumps(value))
        with self.assertRaises(H.HostBlocked):self.snapshot()
        value=self.barrier();self.config['environment']=[];self.config_path.write_text(json.dumps(self.config))
        value['configuration_sha256']=hashlib.sha256(self.config_path.read_bytes()).hexdigest();self.write_barrier(value)
        with self.assertRaises(H.HostBlocked):self.snapshot()

    def test_source_bound_configuration_returns_value_without_namespace_or_finder_leak(self):
        before = list(sys.meta_path)
        self.assertEqual(self.execute(), 7)
        self.assertEqual(sys.meta_path, before)
        self.assertFalse(any(name == 'trusted_gate' or name.startswith('trusted_gate.') for name in sys.modules))
        self.assertEqual(self.execute(), 7)

    def test_timestamp_alias_cache_reads_old_code_normally_but_snapshot_compiles_pinned_new_bytes(self):
        path = self.root / 'cli.py'
        first = b'def main(argv):\n    return 1\n'
        second = b'def main(argv):\n    return 2\n'
        self.write('cli', first)
        py_compile.compile(str(path), doraise=True, invalidation_mode=py_compile.PycInvalidationMode.TIMESTAMP)
        (self.root / '__pycache__').chmod(0o755)
        before = path.stat()
        self.write('cli', second)
        os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
        self.pin()
        spec = importlib.util.spec_from_file_location('ordinary_stale_fixture', path)
        module = importlib.util.module_from_spec(spec)
        with patch.object(sys, 'dont_write_bytecode', True):
            spec.loader.exec_module(module)
            self.assertEqual(module.main([]), 1)
            self.assertEqual(self.execute(), 2)
        self.assertEqual(path.read_bytes(), second)

    def test_unpinned_missing_or_changed_module_is_rejected_before_initializer(self):
        for mutation in ('changed', 'missing', 'unpinned'):
            with self.subTest(mutation=mutation):
                path = self.root / 'cache.py'
                raw = path.read_bytes()
                if mutation == 'changed':path.write_bytes(raw + b'\n')
                elif mutation == 'missing':path.unlink()
                else:
                    del self.config['protected_files'][str(path)]
                    self.config_path.write_text(json.dumps(self.config))
                with self.assertRaises((H.HostBlocked, OSError)):
                    self.snapshot()
                path.write_bytes(raw);path.chmod(0o644);self.pin()

    def test_changed_config_source_interpreter_or_mode_after_snapshot_is_rejected(self):
        for path in (self.config_path, self.root / 'cli.py', self.interpreter):
            for mutation in ('bytes', 'replace', 'mode'):
                with self.subTest(path=path.name, mutation=mutation):
                    raw, mode = path.read_bytes(), stat.S_IMODE(path.stat().st_mode)
                    snapshot = self.snapshot()
                    if mutation == 'bytes':path.write_bytes(raw + b' ')
                    elif mutation == 'replace':
                        temp = path.with_name(path.name + '.new');temp.write_bytes(raw);temp.chmod(mode);temp.replace(path)
                    else:path.chmod(0o666)
                    with self.assertRaises(H.HostBlocked):H.run(snapshot, [str(self.config_path)])
                    path.write_bytes(raw);path.chmod(mode)

    def test_same_size_mtime_source_mutation_after_snapshot_still_rejected(self):
        path = self.root / 'cli.py'
        self.write('cli', b'def main(argv):\n    return 1\n');self.pin()
        snapshot = self.snapshot();before = path.stat()
        self.write('cli', b'def main(argv):\n    return 2\n')
        os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
        with self.assertRaises(H.HostBlocked):H.run(snapshot, [])

    def test_config_changed_by_loaded_code_is_rejected_before_journal_activity(self):
        self.write('cli', b'from pathlib import Path\nfrom .common import controller_configuration\n'
                   b'def main(argv):\n    Path(argv[0]).write_bytes(b"{}")\n'
                   b'    controller_configuration(argv[0])\n    raise AssertionError("must not reach journal")\n')
        self.pin()
        with self.assertRaisesRegex(ValueError, 'trusted input is malformed'):self.execute()
        self.assertNotIn('trusted_gate', sys.modules)

    def test_unknown_import_never_falls_through_to_path_or_another_finder(self):
        self.write('cli', b'import trusted_gate.unknown\ndef main(argv):return 7\n');self.pin()
        calls = []
        class Fallback:
            def find_spec(self, fullname, path=None, target=None):
                calls.append(fullname)
                return None
        finder = Fallback();sys.meta_path.append(finder)
        try:
            with self.assertRaisesRegex(ImportError, 'outside the pinned'):
                self.execute()
        finally:sys.meta_path.remove(finder)
        self.assertNotIn('trusted_gate.unknown', calls)

    def test_unknown_files_and_cache_symlinks_are_rejected(self):
        for name in ('unknown.py', 'cli.pyc', 'extension.so', 'other'):
            path = self.root / name;path.write_bytes(b'x')
            with self.subTest(name=name), self.assertRaises(H.HostBlocked):self.snapshot()
            path.unlink()
        (self.root / '__pycache__').symlink_to(self.directory, target_is_directory=True)
        with self.assertRaises(H.HostBlocked):self.snapshot()

    def test_source_hardlinks_symlinks_wrong_modes_and_private_config_mode_fail(self):
        path = self.root / 'cache.py';raw = path.read_bytes()
        alias = self.directory / 'alias'
        os.link(path, alias)
        with self.assertRaises(H.HostBlocked):self.snapshot()
        alias.unlink();path.unlink();alias.write_bytes(raw);path.symlink_to(alias)
        with self.assertRaises(H.HostBlocked):self.snapshot()
        path.unlink();path.write_bytes(raw);path.chmod(0o600)
        with self.assertRaises(H.HostBlocked):self.snapshot()
        path.chmod(0o644);self.config_path.chmod(0o644)
        with self.assertRaises(H.HostBlocked):self.snapshot()

    def test_empty_oversize_or_syntactically_invalid_source_rejected_before_execution(self):
        for data in (b'', b'#' * (H.MAX_SOURCE + 1), b'broken syntax !'):
            self.write('cli', data);self.pin()
            with self.subTest(size=len(data)), self.assertRaises((H.HostBlocked, SyntaxError)):
                self.snapshot()

    def test_duplicate_nonfinite_and_nonobject_config_rejected(self):
        for raw in (b'{"value":1,"value":2}', b'{"value":NaN}', b'[]', b'null', b'not-json'):
            self.config_path.write_bytes(raw)
            with self.subTest(raw=raw), self.assertRaises((H.HostBlocked, ValueError)):
                self.snapshot()

    def test_interpreter_pin_mismatch_is_rejected(self):
        self.interpreter.write_bytes(b'new interpreter')
        with self.assertRaises(H.HostBlocked):self.snapshot()

    def test_change_during_source_capture_is_rejected(self):
        original = H.read
        def read(path, limit, *, mode):
            result = original(path, limit, mode=mode)
            if path.name == 'source.py':
                (self.root / '__init__.py').write_bytes(b'# replaced during snapshot\n')
            return result
        with patch.object(H, 'read', side_effect=read), self.assertRaises(H.HostBlocked):self.snapshot()

    def test_preexisting_namespace_is_not_reused_or_deleted(self):
        old = ModuleType('trusted_gate')
        with patch.dict(sys.modules, {'trusted_gate': old}):
            with self.assertRaises(H.HostBlocked):self.execute()
            self.assertIs(sys.modules['trusted_gate'], old)

    def test_initializer_failure_cleans_all_namespace_and_finder_state(self):
        self.write('__init__', b'raise RuntimeError("fixture failure")\n');self.pin()
        before = list(sys.meta_path)
        with self.assertRaises(RuntimeError):self.execute()
        self.assertEqual(sys.meta_path, before)
        self.assertNotIn('trusted_gate', sys.modules)

    def test_wrong_configuration_path_and_direct_import_cannot_authorize_configuration(self):
        snapshot = self.snapshot()
        with self.assertRaises(H.HostBlocked):snapshot.configuration(self.config_path.parent / 'other.json')
        name = 'ordinary_unbound_configuration'
        spec = importlib.util.spec_from_file_location(name, self.root / 'common.py')
        module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        with self.assertRaisesRegex(module.Blocked, 'source-bound host bootstrap'):
            module.controller_configuration(self.config_path)

    def test_production_path_guard_rejects_rootless_writable_link_multilink_and_wrong_type(self):
        self.guard.stop()
        def info(**changes):
            fields = dict(st_uid=0, st_mode=stat.S_IFREG | 0o644, st_nlink=1, st_gid=0,
                          st_dev=1, st_ino=1, st_size=1, st_mtime_ns=1, st_ctime_ns=1)
            fields.update(changes);return SimpleNamespace(**fields)
        for bad in (info(st_uid=1000), info(st_mode=stat.S_IFREG | 0o666),
                    info(st_mode=stat.S_IFLNK | 0o777), info(st_nlink=2), info(st_mode=stat.S_IFIFO | 0o644)):
            def lstat(path):
                return bad if path == Path('/controller.py') else info(st_mode=stat.S_IFDIR | 0o755)
            with self.subTest(bad=bad), patch.object(Path, 'lstat', lstat), self.assertRaises(H.HostBlocked):
                H.protected(Path('/controller.py'), mode=0o644)
        with self.assertRaises(H.HostBlocked):H.protected(Path('relative.py'))
        self.guard.start()

    def test_actual_cli_rechecks_generation_after_lease_before_journal_or_github(self):
        self.write('cli', (ROOT / 'tools/trusted_gate/cli.py').read_bytes());self.pin()
        finder = H.SourceFinder(self.snapshot());sys.meta_path.insert(0, finder)
        old_umask = os.umask(0o077)
        try:
            import trusted_gate.cli as cli
            def stale_lease(*args):
                self.config_path.write_bytes(b'{}')
            with patch.object(cli, 'CONFIG', self.config_path), \
                 patch.object(cli, 'configuration', return_value={'records': str(self.directory)}), \
                 patch.object(cli.fcntl, 'flock', side_effect=stale_lease), \
                 patch.object(cli, 'Journal') as journal, patch.object(cli, 'GitHub') as github, \
                 patch('sys.stderr', new_callable=io.StringIO) as output:
                self.assertEqual(cli.main(['status']), 1)
                journal.assert_not_called();github.assert_not_called()
                self.assertEqual(json.loads(output.getvalue())['status'], 'blocked')
        finally:
            os.umask(old_umask)
            sys.meta_path.remove(finder)
            for name in list(sys.modules):
                if name == 'trusted_gate' or name.startswith('trusted_gate.'):del sys.modules[name]

    def test_actual_cli_lock_failure_does_not_open_journal_or_app_client(self):
        self.write('cli', (ROOT / 'tools/trusted_gate/cli.py').read_bytes());self.pin()
        finder = H.SourceFinder(self.snapshot());sys.meta_path.insert(0, finder)
        old_umask = os.umask(0o077)
        try:
            import trusted_gate.cli as cli
            with patch.object(cli, 'configuration', return_value={'records': str(self.directory)}), \
                 patch.object(cli.fcntl, 'flock', side_effect=BlockingIOError('lease busy')), \
                 patch.object(cli, 'Journal') as journal, patch.object(cli, 'GitHub') as github, \
                 patch('sys.stderr', new_callable=io.StringIO) as output:
                self.assertEqual(cli.main(['status']), 1)
                journal.assert_not_called();github.assert_not_called()
                self.assertEqual(json.loads(output.getvalue())['status'], 'blocked')
        finally:
            os.umask(old_umask)
            sys.meta_path.remove(finder)
            for name in list(sys.modules):
                if name == 'trusted_gate' or name.startswith('trusted_gate.'):del sys.modules[name]

    def test_entry_rejects_replaced_running_interpreter_before_snapshot(self):
        with patch.object(os, 'geteuid', return_value=0), patch.object(sys, 'executable', str(H.INTERPRETER)), \
             patch.object(os.path, 'samefile', return_value=False), patch.object(H, 'Snapshot') as snapshot, \
             patch('sys.stderr', new_callable=io.StringIO):
            self.assertEqual(H.main(), 1);snapshot.assert_not_called()

    def test_file_replaced_after_open_is_rejected_even_if_read_descriptor_is_stable(self):
        path = self.root / 'cache.py';original = os.open
        def open_file(name, flags):
            fd = original(name, flags)
            replacement = self.directory / 'replacement';replacement.write_bytes(path.read_bytes())
            replacement.chmod(0o644);replacement.replace(path)
            return fd
        with patch.object(os, 'open', side_effect=open_file), self.assertRaises(H.HostBlocked):
            H.read(path, H.MAX_SOURCE, mode=0o644)

    def test_all_actual_controller_modules_import_with_source_bound_bootstrap(self):
        self.write('cli', (ROOT / 'tools/trusted_gate/cli.py').read_bytes());self.pin()
        with patch('sys.stdout', new_callable=io.StringIO), self.assertRaises(SystemExit) as result:
            H.run(self.snapshot(), ['--help'])
        self.assertEqual(result.exception.code, 0)
        self.assertNotIn('trusted_gate', sys.modules)

    def test_entry_rejects_wrong_identity_or_interpreter_without_snapshot_or_state(self):
        for uid, executable in ((1000, '/usr/bin/python3.14'), (0, '/agent/python3.14')):
            with patch.object(os, 'geteuid', return_value=uid), patch.object(sys, 'executable', executable), \
                 patch.object(H, 'Snapshot') as snapshot, patch('sys.stderr', new_callable=io.StringIO) as output:
                self.assertEqual(H.main(), 1);snapshot.assert_not_called()
                self.assertEqual(json.loads(output.getvalue())['status'], 'blocked')

    def test_internal_step_uses_pinned_interpreter_and_only_fixed_run_environment(self):
        launcher = ROOT / 'tools/trusted_gate/deploy/gatehouse-step'
        with patch('os.execve', side_effect=SystemExit(77)) as execute, \
             patch.dict(os.environ, {'GATEHOUSE_RUN_ID': 'fixed-run', 'GITHUB_TOKEN': 'must-not-forward'}), \
             patch.object(sys, 'argv', [str(launcher), 'linux-test']), self.assertRaises(SystemExit):
            runpy.run_path(str(launcher), run_name='__main__')
        self.assertEqual(execute.call_args.args, ('/usr/bin/python3.14',
            ['/usr/bin/python3.14', '-I', '/opt/gatehouse/trusted_gate/host.py', 'step', 'linux-test'],
            {'PATH': '/usr/bin:/bin', 'GATEHOUSE_RUN_ID': 'fixed-run'}))


if __name__ == '__main__':
    unittest.main()
