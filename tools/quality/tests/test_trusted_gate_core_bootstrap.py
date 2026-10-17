"""Core bootstrap never captures from an active or unbound build worker."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools/trusted_gate/deploy' / filename)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


BOOT = load('core_bootstrap_transport', 'import-protected-inputs.py')
BUILD = load('core_bootstrap_primitives', 'build-protected-core.py')


class CoreBootstrapTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.cleanup)
        self.control = self.root / 'control'
        self.control.mkdir(mode=0o700)
        self.B = SimpleNamespace(**{name: getattr(BOOT, name) for name in (
            'need', 'path_operand', 'write', 'record', 'canonical', 'unique', 'readonly', 'ENV')})
        self.B.owned = self.owned
        self.B.read = lambda path, limit: self.read(path, limit)
        self.B.authenticate = lambda *args: {'revision': 'a' * 40}
        self.B.git = lambda *args: BUILD.CORE_TREE.encode()
        self.lock = b'exact frozen lock'
        self.binding = {'lock_sha256': hashlib.sha256(self.lock).hexdigest()}
        self.host = {'cc': '/usr/bin/reviewed-cc', 'ar': '/usr/bin/reviewed-ar'}
        self.destination = self.root / 'build'
        self.order = []

    def cleanup(self):
        for parent, _, names in os.walk(self.root):
            os.chmod(parent, 0o700)
            for name in names:
                path = Path(parent) / name
                if path.is_file() and not path.is_symlink(): path.chmod(0o600)
        self.temporary.cleanup()

    def owned(self, path, **kwargs):
        # Ordinary-user fixture replaces ownership, not archive/build authority.
        return Path(path)

    def read(self, path, limit):
        raw = Path(path).read_bytes()
        BOOT.need(len(raw) <= limit, 'fixture exceeds limit')
        return raw

    def snapshot(self, B, source, head, destination):
        destination.mkdir()
        BOOT.write(destination / 'Cargo.lock', self.lock)
        return {'tree': BUILD.CORE_TREE, 'files': 1, 'bytes': len(self.lock), 'inventory_sha256': 'f' * 64}

    def execute(self, argv, **kwargs):
        if argv[0] == '/usr/bin/systemctl':
            self.order.append('stop-request')
            return SimpleNamespace(returncode=0)
        self.order.append('launch')
        path = self.destination / 'worker/target/release'
        path.mkdir(mode=0o700)
        (path / 'harness-gate').write_bytes(b'generated Core fixture')
        (path / 'harness-gate').chmod(0o700)
        return SimpleNamespace(returncode=0)

    def inputs(self, *args):
        self.order.append('input-check')
        return self.binding

    def verify_snapshot(self, *args):
        self.order.append('snapshot-check')

    def stopped(self, *args):
        self.order.append('stopped')

    def build(self, lock_fd=None, process=None, stopping=None, checking=None):
        capture = BUILD.capture
        def captured(*args):
            self.order.append('capture')
            return capture(*args)
        with patch.object(BUILD, 'CONTROL', self.control), patch.object(BUILD, 'UID', os.getuid()), \
             patch.object(BUILD, 'GID', os.getgid()), patch.object(BUILD.os, 'geteuid', return_value=0), \
             patch.object(BUILD, 'worker_identity'), patch.object(BUILD, 'idle'), \
             patch.object(BUILD, 'host_inputs', return_value=self.host), \
             patch.object(BUILD, 'imported_inputs', side_effect=checking or self.inputs), \
             patch.object(BUILD, 'source_snapshot', side_effect=self.snapshot), \
             patch.object(BUILD, 'verify_snapshot', side_effect=self.verify_snapshot), \
             patch.object(BUILD.subprocess, 'run', side_effect=process or self.execute), \
             patch.object(BUILD, 'stopped', side_effect=stopping or self.stopped), \
             patch.object(BUILD, 'capture', side_effect=captured):
            return BUILD.build(self.B, self.root/'helpers', self.root/'manifest', 'a'*64,
                self.root/'source', 'a'*40, 'b'*40, self.root/'inputs', 'b'*64,
                self.root/'host', 'c'*64, self.destination, lock_fd=lock_fd)

    def test_capture_follows_stop_and_fresh_source_host_and_input_checks(self):
        value = self.build()
        self.assertEqual(self.order, ['input-check', 'snapshot-check', 'launch', 'stopped',
                                      'input-check', 'snapshot-check', 'capture'])
        self.assertEqual(value['state'], 'captured')
        self.assertTrue(value['worker_stop_proven'])
        self.assertEqual(value['binary']['sha256'], hashlib.sha256(b'generated Core fixture').hexdigest())
        self.assertEqual((self.destination/'harness-gate').stat().st_mode & 0o777, 0o555)
        self.assertEqual(json.loads((self.destination/'build.json').read_text()), value)
        with self.assertRaisesRegex(ValueError, 'never reuse'):
            self.build()

    def test_unknown_stop_never_reads_or_exports_the_generated_binary(self):
        def active(*args):
            raise ValueError('worker active')
        with self.assertRaisesRegex(ValueError, 'worker active'):
            self.build(stopping=active)
        self.assertNotIn('capture', self.order)
        value = json.loads((self.destination/'build.json').read_text())
        self.assertEqual(value['state'], 'blocked')
        self.assertFalse(value['worker_stop_proven'])
        self.assertFalse((self.destination/'harness-gate').exists())

    def test_failed_exit_and_timeout_request_stop_and_never_capture(self):
        execute = self.execute
        for timeout in (False, True):
            def failed(argv, **kwargs):
                if argv[0] == '/usr/bin/systemctl': return execute(argv, **kwargs)
                if timeout: raise subprocess.TimeoutExpired(argv, 1)
                execute(argv, **kwargs)
                return SimpleNamespace(returncode=1)
            self.destination = self.root / ('timeout' if timeout else 'failed')
            self.order.clear()
            with self.subTest(timeout=timeout), self.assertRaises((ValueError, subprocess.TimeoutExpired)):
                self.build(process=failed)
            self.assertIn('stop-request', self.order)
            self.assertNotIn('capture', self.order)
            value = json.loads((self.destination/'build.json').read_text())
            self.assertEqual(value['state'], 'blocked')
            self.assertTrue(value['worker_stop_proven'])

    def test_changed_input_after_worker_stop_blocks_capture(self):
        checks = 0
        def changed(*args):
            nonlocal checks
            checks += 1
            if checks > 1: raise ValueError('input changed')
            return self.binding
        with self.assertRaisesRegex(ValueError, 'input changed'):
            self.build(checking=changed)
        self.assertNotIn('capture', self.order)
        self.assertFalse((self.destination/'harness-gate').exists())

    def test_inherited_controller_lease_remains_open_and_wrong_descriptor_is_rejected(self):
        with (self.control/'controller.lock').open('w+b') as lease:
            self.assertEqual(self.build(lock_fd=lease.fileno())['state'], 'captured')
            os.fstat(lease.fileno())
        self.destination = self.root/'wrong-lease'
        with (self.root/'wrong.lock').open('w+b') as wrong:
            with self.assertRaisesRegex(ValueError, 'lease descriptor differs'):
                self.build(lock_fd=wrong.fileno())
            os.fstat(wrong.fileno())

    def test_build_requires_root_before_any_input_is_read(self):
        with patch.object(BUILD.os, 'geteuid', return_value=1000), self.assertRaisesRegex(ValueError, 'administrator'):
            BUILD.build(self.B, None, None, None, None, None, None, None, None, None, None, None)

    def test_worker_command_has_fixed_privileges_mounts_profile_and_incremental_flags(self):
        command, env = BUILD.command(self.B, Path('/source'), Path('/inputs'), Path('/worker'),
                                     Path('/cargo-config'), 'unit.service', self.host)
        for arg in ('--property=User=995', '--property=Group=981', '--property=NoNewPrivileges=yes',
                    '--property=PrivateNetwork=yes', '--property=KillMode=control-group',
                    '--property=CapabilityBoundingSet=', '--property=ReadWritePaths=/work/output',
                    '--property=WorkingDirectory=/work/source', '--property=TemporaryFileSystem=/work'):
            self.assertIn(arg, command)
        self.assertEqual(command[command.index('--')+1:command.index('--')+4], ['/usr/bin/env', '-i',
                         'PATH=/opt/gatehouse/toolchain/bin:/usr/bin:/bin'])
        self.assertEqual(command[-9:], ['/opt/gatehouse/toolchain/bin/cargo', 'build',
            '--release', '--locked', '--offline', '--manifest-path', '/work/source/Cargo.toml', '--bin', 'harness-gate'])
        self.assertEqual(env['CARGO_INCREMENTAL'], '0')
        self.assertEqual(env['CARGO_PROFILE_RELEASE_INCREMENTAL'], 'false')
        self.assertEqual(env['CARGO_HOME'], '/work/output/cargo-home')
        self.assertFalse({'GH_TOKEN', 'GITHUB_TOKEN', 'LD_PRELOAD', 'RUSTC_WRAPPER'} & env.keys())

    def test_generated_binary_links_and_size_guards_do_not_relax_immutable_input_rules(self):
        worker = self.root/'worker'
        release = worker/'target/release'
        release.mkdir(parents=True, mode=0o700)
        binary = release/'harness-gate'
        binary.write_bytes(b'generated fixture'); binary.chmod(0o700)
        os.link(binary, worker/'generated-hardlink')
        with patch.object(BUILD, 'UID', os.getuid()):
            self.assertEqual(BUILD.capture(self.B, worker, self.root/'captured')['bytes'], len(b'generated fixture'))
            with patch.object(BUILD, 'MAX_BINARY', 1), self.assertRaisesRegex(ValueError, 'unsafe generated'):
                BUILD.capture(self.B, worker, self.root/'oversized')
            binary.unlink(); binary.symlink_to(worker/'generated-hardlink')
            with self.assertRaises((OSError, ValueError)):
                BUILD.capture(self.B, worker, self.root/'linked')

    def test_stopped_worker_requires_empty_cgroup_and_no_executor_processes(self):
        for state in ('active running', 'unknown unknown'):
            with patch.object(BUILD.subprocess, 'run', return_value=SimpleNamespace(stdout=(
                    'unit.service loaded '+state+'\n').encode())), self.assertRaisesRegex(ValueError, 'still active'):
                BUILD.stopped(self.B, 'unit.service')
        for returncode in (0, 2):
            replies = [SimpleNamespace(stdout=b''), SimpleNamespace(returncode=returncode, stdout=b'123\n', stderr=b'')]
            with patch.object(BUILD.subprocess, 'run', side_effect=replies), self.assertRaisesRegex(ValueError, 'processes'):
                BUILD.stopped(self.B, 'unit.service')
        replies = [SimpleNamespace(stdout=b''), SimpleNamespace(returncode=1, stdout=b'', stderr=b'')]
        with patch.object(BUILD.subprocess, 'run', side_effect=replies):
            BUILD.stopped(self.B, 'unit.service')
        events=self.root/'cgroup.events'
        for populated,allowed in ((0,True),(1,False)):
            events.write_text('populated '+str(populated)+'\nfrozen 0\n')
            replies=[SimpleNamespace(stdout=b'unit.service loaded inactive dead\n'),
                     SimpleNamespace(stdout=b'/system.slice/unit.service\n'),
                     SimpleNamespace(returncode=1,stdout=b'',stderr=b'')]
            with patch.object(BUILD.subprocess,'run',side_effect=replies), \
                 patch.object(self.B,'owned',return_value=events):
                if allowed: BUILD.stopped(self.B,'unit.service')
                else:
                    with self.assertRaisesRegex(ValueError,'not proven empty'):
                        BUILD.stopped(self.B,'unit.service')

    def test_snapshot_is_from_reviewed_git_blobs_without_parent_cargo_configuration(self):
        source = self.root/'repo'
        crate = source/'tools/harness-gate'
        (crate/'src').mkdir(parents=True)
        (source/'.cargo').mkdir()
        (source/'.cargo/config.toml').write_text('unreviewed parent configuration')
        (crate/'Cargo.toml').write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
        (crate/'Cargo.lock').write_bytes(self.lock)
        (crate/'src/main.rs').write_bytes(b'fn main() {}\n')
        def git(*args):
            return subprocess.check_output(['/usr/bin/git','-C',str(source),*args], env=BOOT.ENV,
                                            stderr=subprocess.PIPE)
        git('init','-q');git('add','.')
        git('-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','snapshot')
        head = git('rev-parse','HEAD').decode().strip()
        tree = git('rev-parse',head+':tools/harness-gate').decode().strip()
        self.B.git = lambda root, *args: git(*args)
        self.B.load = lambda root, name: load('snapshot_contract', name)
        self.B.readonly = lambda root: self.make_readonly(root)
        output = self.root/'snapshot'
        with patch.object(BUILD,'CORE_TREE',tree):
            record = BUILD.source_snapshot(self.B,source,head,output)
        self.assertEqual(record['files'],3)
        self.assertFalse((output/'.cargo').exists())
        self.assertEqual((output/'src/main.rs').read_bytes(),b'fn main() {}\n')
        BUILD.verify_snapshot(self.B,self.root,output,record)
        path = output/'src/main.rs';path.chmod(0o600);path.write_bytes(b'changed');path.chmod(0o444)
        with self.assertRaisesRegex(ValueError,'snapshot changed'):
            BUILD.verify_snapshot(self.B,self.root,output,record)
        with self.assertRaisesRegex(ValueError,'independent review'):
            BUILD.source_snapshot(self.B,source,head,self.root/'unreviewed')
        git_read = self.B.git
        def corrupt(root, *args):
            raw = git_read(root, *args)
            return raw + b'x' if args[:2] == ('cat-file', 'tree') else raw
        self.B.git = corrupt
        with patch.object(BUILD,'CORE_TREE',tree), self.assertRaisesRegex(ValueError,'tree object changed'):
            BUILD.source_snapshot(self.B,source,head,self.root/'corrupt-tree')

    def make_readonly(self, root):
        for parent, _, files in os.walk(root):
            for name in files:
                path=Path(parent)/name;path.chmod(0o555 if path.stat().st_mode&0o111 else 0o444)
            Path(parent).chmod(0o555)

    def test_real_imported_fixture_is_rechecked_without_trusting_mutable_inventories(self):
        path = ROOT/'tools/quality/tests/test_trusted_gate_protected_import.py'
        spec=importlib.util.spec_from_file_location('build_input_fixture',path)
        fixtures=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixtures)
        case=fixtures.ProtectedImportTests()
        case.setUp()
        self.addCleanup(case.doCleanups)
        case.import_inputs()
        binding=case.destination/'binding.json'
        pin=hashlib.sha256(binding.read_bytes()).hexdigest()
        def verify():
            with case.fixture_domain():
                return BUILD.imported_inputs(fixtures.BOOT,case.destination,pin,case.source,
                                             case.head,case.head,case.helpers)
        self.assertEqual(verify()['state'],'imported')
        node=case.destination/'tools/toolchain/bin/node'
        node.chmod(0o700);node.write_bytes(b'changed tool');node.chmod(0o555)
        with self.assertRaisesRegex(ValueError,'tool payload changed'):
            verify()
        inventory=case.destination/'tools/inventory.json'
        inventory.chmod(0o600);inventory.write_bytes(b'{}\n');inventory.chmod(0o444)
        with self.assertRaisesRegex(ValueError,'metadata pin differs'):
            verify()

    def test_executor_memberships_and_host_recipe_pins_are_explicit(self):
        account=SimpleNamespace(pw_uid=995,pw_name='gatehouse-executor',pw_gid=981,pw_shell='/usr/sbin/nologin')
        group=SimpleNamespace(gr_name='gatehouse-executor',gr_mem=['gatehouse-executor'])
        with patch.object(BUILD.pwd,'getpwuid',return_value=account), \
             patch.object(BUILD.grp,'getgrgid',return_value=group), \
             patch.object(BUILD.grp,'getgrall',return_value=[group]):
            BUILD.worker_identity(self.B)
            privileged=SimpleNamespace(gr_name='docker',gr_mem=['gatehouse-executor'])
            with patch.object(BUILD.grp,'getgrall',return_value=[group,privileged]), self.assertRaises(ValueError):
                BUILD.worker_identity(self.B)
        files={'/usr/bin/'+name:hashlib.sha256(b'reviewed').hexdigest() for name in
               ('git','env','systemd-run','systemctl','pgrep','reviewed-cc','reviewed-ar')}
        value={'schema':'gatehouse-core-build-host/v1','files':files,**self.host}
        raw=BOOT.canonical(value)
        self.B.read=lambda path,limit:raw if str(path)=='/host.json' else b'reviewed'
        self.assertEqual(BUILD.host_inputs(self.B,'/host.json',hashlib.sha256(raw).hexdigest()),value)
        self.B.read=lambda path,limit:raw if str(path)=='/host.json' else b'changed'
        with self.assertRaisesRegex(ValueError,'host build input differs'):
            BUILD.host_inputs(self.B,'/host.json',hashlib.sha256(raw).hexdigest())

    def test_core_cli_rejects_actual_nonroot_before_creating_worker_state(self):
        if os.geteuid()==0:
            return
        result=subprocess.run([sys.executable,'-I',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),
            'build-core','--helper-root','/missing','--manifest','/missing','--manifest-sha256','a'*64,
            '--source','/missing','--head','a'*40,'--base','b'*40,'--inputs','/missing',
            '--inputs-sha256','b'*64,'--host-recipe','/missing','--host-recipe-sha256','c'*64,
            '--destination',str(self.destination)],capture_output=True,text=True)
        self.assertEqual(result.returncode,1)
        self.assertIn('Core bootstrap blocked (ValueError)',result.stderr)
        self.assertFalse(self.destination.exists())


if __name__ == '__main__':
    unittest.main()
