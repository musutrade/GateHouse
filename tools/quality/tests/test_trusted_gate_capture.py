"""Selected worker evidence must survive through Core logs without scratch export."""
import base64
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('capture_trusted_gate',
    ROOT / 'tools/trusted_gate/__init__.py', submodule_search_locations=[str(ROOT / 'tools/trusted_gate')])
PACKAGE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PACKAGE
SPEC.loader.exec_module(PACKAGE)
from capture_trusted_gate import isolation as CAPTURE
from capture_trusted_gate import execution as EXECUTION
from capture_trusted_gate.common import Blocked, canonical, digest


class WorkerCaptureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.scratch = self.root / 'scratch'
        self.scratch.mkdir()
        (self.scratch / 'job').mkdir()
        self.data = b'actual receipt\x00\xff\n'
        (self.scratch / 'job/report.json').write_bytes(self.data)
        (self.scratch / 'operator.pem').write_bytes(b'disposable key MUST remain private')
        self.run = {'id': 'run-id', 'head': 'a' * 40, 'local': 'running'}

    def capture(self, names=None, uid=None):
        return CAPTURE.capture_artifacts(self.scratch, names or ['job/report.json'],
            os.getuid() if uid is None else uid, self.run, 'linux-test', 7)

    def test_only_named_complete_bytes_are_bound_and_failed_exit_is_preserved(self):
        value = self.capture()
        self.assertEqual(set(value['worker_artifacts']), {'job/report.json'})
        receipt = value['worker_artifacts']['job/report.json']
        self.assertEqual(base64.b64decode(receipt['base64']), self.data)
        self.assertEqual(receipt['sha256'], digest(self.data))
        self.assertEqual(receipt['size_bytes'], len(self.data))
        self.assertEqual(value['head'], self.run['head'])
        self.assertEqual(value['exit_code'], 7)
        self.assertNotIn(b'disposable key', canonical(value))

    def test_invalid_inventory_never_resolves_paths_or_globs(self):
        for names in (['../operator.pem'], ['/etc/shadow'], ['job//report.json'], ['job/./report.json'],
                      ['job/*.json'], ['job/report.json'] * 2, ['job\\report.json'], 'job/report.json',
                      [None], ['a'] * 129):
            with self.subTest(names=names), self.assertRaises(Blocked):
                CAPTURE.capture_names({'capture_files': names})

    def test_missing_named_file_is_not_optional(self):
        with self.assertRaises(Blocked):
            self.capture(['job/missing.json'])

    def test_leaf_directory_links_hardlinks_and_special_files_are_rejected(self):
        (self.scratch / 'linked').symlink_to(self.scratch / 'job', target_is_directory=True)
        (self.scratch / 'leaf').symlink_to(self.scratch / 'job/report.json')
        os.link(self.scratch / 'job/report.json', self.scratch / 'hardlink')
        os.mkfifo(self.scratch / 'fifo')
        for name in ('linked/report.json', 'leaf', 'hardlink', 'fifo', 'job'):
            with self.subTest(name=name), self.assertRaises(Blocked):
                self.capture([name])

    def test_wrong_owner_is_rejected(self):
        with self.assertRaisesRegex(Blocked, 'owner'):
            self.capture(uid=os.getuid() + 1)

    def test_per_file_and_total_budgets_are_enforced(self):
        large = b'x' * (4 * 1024 * 1024)
        (self.scratch / 'oversize').write_bytes(large + b'x')
        with self.assertRaisesRegex(Blocked, 'oversized'):
            self.capture(['oversize'])
        for number in range(3):
            (self.scratch / str(number)).write_bytes(large)
        with self.assertRaisesRegex(Blocked, 'total evidence budget'):
            self.capture([str(number) for number in range(3)])

    def test_changed_file_during_read_is_rejected(self):
        real_fdopen = CAPTURE.os.fdopen
        class ChangingFile:
            def __init__(inner, fd, mode):
                inner.stream = real_fdopen(fd, mode)
            def __enter__(inner):
                return inner
            def __exit__(inner, *args):
                inner.stream.close()
            def fileno(inner):
                return inner.stream.fileno()
            def read(inner, count):
                data = inner.stream.read(count)
                (self.scratch / 'job/report.json').write_bytes(b'changed')
                return data
        with patch.object(CAPTURE.os, 'fdopen', side_effect=ChangingFile):
            with self.assertRaisesRegex(Blocked, 'changed during capture'):
                self.capture()

    def test_launch_proves_worker_stopped_before_capture_and_log_emission(self):
        account = SimpleNamespace(pw_uid=os.getuid(), pw_gid=os.getgid(), pw_shell='/usr/sbin/nologin')
        for name in ('cache', 'records/run-id'):
            (self.root / name).mkdir(parents=True)
        step = {'capture_files': ['job/report.json'], 'toolchain_files': []}
        config = {'steps': {'linux-test': step}, 'executor_user': 'worker', 'developer_uid': os.getuid()+1,
                  'scratch': str(self.scratch), 'records': str(self.root / 'records'),
                  'source': '/source', 'repository': 'owner/repository', 'protected_files': {},
                  'cache': {'root': str(self.root / 'cache'), 'max_bytes': 10, 'max_entries': 1, 'ttl_seconds': 1}}
        self.run['input'] = {'policy': 'policy'}
        journal = SimpleNamespace(get=lambda run_id: self.run)
        cache = SimpleNamespace(directory=lambda selected: self.root / 'cache', maintain=lambda **kw: {})
        order = []
        def stopped(*args):
            order.append('stopped')
        def captured(*args):
            order.append('captured')
            return {'worker_artifacts': {'job/report.json': {'sha256': 'f'*64, 'size_bytes': 1}}, 'exit_code': 7}
        with patch.object(CAPTURE.os, 'geteuid', return_value=0), \
             patch.object(CAPTURE.pwd, 'getpwnam', return_value=account), \
             patch.object(CAPTURE, 'unprivileged_identity', return_value=account), \
             patch.object(CAPTURE, 'protected', side_effect=lambda p, **kw: Path(p)), \
             patch.object(CAPTURE.os, 'chown'), patch.object(CAPTURE, 'Cache', return_value=cache), \
             patch.object(CAPTURE, 'dimensions', return_value={}), \
             patch.object(CAPTURE, 'sandbox_arguments', return_value=['worker']), \
             patch.object(CAPTURE.subprocess, 'run', return_value=SimpleNamespace(returncode=7)), \
             patch.object(CAPTURE, 'confirm_units_stopped', side_effect=stopped) as stopping, \
             patch.object(CAPTURE, 'capture_artifacts', side_effect=captured) as capturing, \
             patch('sys.stdout', new_callable=io.StringIO) as output:
            self.assertEqual(CAPTURE.launch(config, journal, 'run-id', 'linux-test'), 7)
            self.assertEqual(order, ['stopped', 'captured'])
            self.assertEqual(json.loads(output.getvalue())['exit_code'], 7)
            capturing.reset_mock()
            stopping.side_effect = Blocked('worker still populated')
            config['steps']['blocked-test'] = step
            output.seek(0)
            output.truncate()
            with self.assertRaisesRegex(Blocked, 'still populated'):
                CAPTURE.launch(config, journal, 'run-id', 'blocked-test')
            capturing.assert_not_called()
            self.assertEqual(output.getvalue(), '')
        # The blocked launch also writes its initial per-step record; select the
        # linux-test receipt instead of depending on filesystem glob order.
        record = json.loads((self.root / 'records/run-id/cache' /
                             (digest('linux-test'.encode()) + '.json')).read_text())
        self.assertEqual(record['captured_files']['job/report.json']['size_bytes'], 1)


class WorkerStoppedTests(unittest.TestCase):
    def test_active_unknown_and_unpopulated_states_do_not_authorize_capture(self):
        for state in ('active running', 'activating start', 'deactivating stop', 'unknown unknown'):
            response = SimpleNamespace(stdout=('worker.service loaded ' + state + '\n').encode())
            with patch.object(EXECUTION.subprocess, 'run', return_value=response):
                with self.subTest(state=state), self.assertRaises(Blocked):
                    EXECUTION.confirm_units_stopped('worker.service')

    def test_collected_unit_or_inactive_empty_cgroup_is_accepted(self):
        with patch.object(EXECUTION.subprocess, 'run', return_value=SimpleNamespace(stdout=b'')):
            EXECUTION.confirm_units_stopped('worker.service')
        for events, allowed in (('populated 0\nfrozen 0\n', True), ('populated 1\n', False),
                                ('populated 01\n', False)):
            responses = [SimpleNamespace(stdout=b'worker.service loaded inactive dead\n'),
                         SimpleNamespace(stdout=b'/system.slice/worker.service\n')]
            with patch.object(EXECUTION.subprocess, 'run', side_effect=responses), \
                 patch.object(Path, 'is_file', return_value=True), \
                 patch.object(Path, 'read_text', return_value=events):
                if allowed:
                    EXECUTION.confirm_units_stopped('worker.service')
                else:
                    with self.assertRaises(Blocked):
                        EXECUTION.confirm_units_stopped('worker.service')


if __name__ == '__main__':
    unittest.main()
