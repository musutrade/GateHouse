"""Destructive-path and process-liveness checks run only in temporary repositories."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / 'maintenance/local_storage.py'
SPEC = importlib.util.spec_from_file_location('local_storage', SCRIPT)
storage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(storage)


def entry(path, **kwargs):
    return dict(path=path, kind='cache', reviewed=True, reproducible=True,
                contains_evidence=False, contains_backup=False, reason='rebuildable test cache', **kwargs)


class CleanupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'repo'
        self.root.mkdir()
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.git('config', 'user.email', 'test@example.invalid')
        self.git('config', 'user.name', 'Test')
        (self.root / 'source').write_text('tracked source')
        self.git('add', 'source')
        self.git('commit', '-qm', 'initial')
        self.cache('cache/a')
        self.manifest = {'version': 1, 'entries': [entry('cache/a')]}
        self.activity = patch.object(storage, 'active_references', return_value={'test': 'isolated'})
        self.activity.start()
        self.addCleanup(self.activity.stop)

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.root), *args], check=True, capture_output=True)

    def cache(self, name):
        path = self.root / name
        path.mkdir(parents=True, exist_ok=True)
        (path / 'object').write_bytes(b'regenerable artifact')
        return path

    def plan(self):
        return storage.make_plan(self.root, self.manifest)

    def test_default_apply_is_read_only_and_execute_removes_only_candidate(self):
        self.cache('unregistered')
        plan = self.plan()
        self.assertFalse(storage.apply(plan)['executed'])
        self.assertTrue((self.root / 'cache/a/object').exists())
        self.assertEqual(storage.apply(plan, True)['deleted'], ['cache/a'])
        self.assertFalse((self.root / 'cache/a').exists())
        self.assertTrue((self.root / 'unregistered/object').exists())
        self.assertEqual((self.root / 'source').read_text(), 'tracked source')

    def test_modified_content_refuses_all_deletion(self):
        plan = self.plan()
        (self.root / 'cache/a/object').write_bytes(b'changed')
        with self.assertRaisesRegex(storage.SafetyError, 'stale'):
            storage.apply(plan, True)
        self.assertTrue((self.root / 'cache/a/object').exists())

    def test_expired_or_future_plan_is_rejected(self):
        for created, expiry in [(0, 1), (time.time()+100, time.time()+200)]:
            plan = self.plan()
            plan.update(created_at=created, expires_at=expiry)
            with self.assertRaisesRegex(storage.SafetyError, 'expired|invalid'):
                storage.apply(plan, True)

    def test_index_change_invalidates_plan(self):
        plan = self.plan()
        (self.root / 'new-source').write_text('new tracked input')
        self.git('add', 'new-source')
        with self.assertRaisesRegex(storage.SafetyError, 'stale'):
            storage.apply(plan, True)

    def test_tracked_content_and_staged_deletion_are_protected(self):
        self.git('add', 'cache/a/object')
        self.git('commit', '-qm', 'tracked fixture')
        self.git('rm', '--cached', 'cache/a/object')
        with self.assertRaisesRegex(storage.SafetyError, 'tracked'):
            self.plan()

    def test_unknown_evidence_backup_and_unreviewed_are_kept(self):
        for kind in ['backup', 'evidence', 'unknown']:
            self.manifest['entries'][0]['kind'] = kind
            self.assertEqual(self.plan()['entries'][0]['action'], 'keep')
        self.manifest['entries'][0]['kind'] = 'cache'
        self.manifest['entries'][0]['reviewed'] = False
        self.assertEqual(self.plan()['entries'][0]['action'], 'keep')

    def test_nested_duplicate_and_path_traversal_are_rejected(self):
        for path in ['cache/a', 'cache', 'cache/a/deeper', '../outside', '/tmp', '.',
                     'cache//b', 'cache/../b', '.git/objects']:
            with self.subTest(path=path):
                self.manifest['entries'] = [entry('cache/a'), entry(path)]
                with self.assertRaises(storage.SafetyError):
                    self.plan()

    def test_symlink_and_nested_repository_are_rejected_without_touching_target(self):
        outside = Path(self.temp.name) / 'outside'
        outside.mkdir()
        (outside / 'keep').write_text('keep')
        link = self.root / 'cache/a/link'
        link.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(storage.SafetyError, 'symlink'):
            self.plan()
        self.assertTrue((outside / 'keep').exists())
        link.unlink()
        (self.root / 'cache/a/.git').write_text('gitdir: /elsewhere')
        with self.assertRaisesRegex(storage.SafetyError, 'nested repository'):
            self.plan()

    def test_ancestor_symlink_is_rejected(self):
        (self.root / 'alias').symlink_to(self.root / 'cache', target_is_directory=True)
        self.manifest['entries'] = [entry('alias/a')]
        with self.assertRaisesRegex(storage.SafetyError, 'real directories'):
            self.plan()

    def test_retain_two_only_counts_completed_closed_archived_runs(self):
        rows = []
        for i in range(5):
            row = entry(f'run/{i}')
            row.update(kind='run', state='completed', failure_closed=True, archived=True,
                       review_pending=False, group='measurement', completed_at=time.time()-100+i)
            self.cache(row['path'])
            rows.append(row)
        rows[4]['review_pending'] = True
        rows[3]['failure_closed'] = False
        rows.append(dict(rows[0], path='run/not-archived', archived=False))
        self.manifest['entries'] = rows
        actions = {r['path']: r['action'] for r in self.plan()['entries']}
        self.assertEqual([p for p, a in actions.items() if a == 'delete'], ['run/0'])

    def test_forged_plan_disposition_is_rejected(self):
        plan = self.plan()
        plan['entries'][0]['action'] = 'keep'
        with self.assertRaisesRegex(storage.SafetyError, 'stale'):
            storage.apply(plan, True)

    def test_activity_failure_during_apply_prevents_removal(self):
        plan = self.plan()
        with patch.object(storage, 'active_references', side_effect=storage.SafetyError('active')):
            with self.assertRaisesRegex(storage.SafetyError, 'active'):
                storage.apply(plan, True)
        self.assertTrue((self.root / 'cache/a/object').exists())


@unittest.skipUnless(sys.platform == 'linux', 'Linux /proc process inspection')
class ProcessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.candidate = self.root / 'candidate'
        self.candidate.mkdir()
        self.proc = self.root / 'proc'
        self.proc.mkdir()
        (self.proc / 'locks').write_text('')

    def child(self, code, cwd=None):
        process = subprocess.Popen([sys.executable, '-c', code], cwd=cwd,
                                   stdout=subprocess.PIPE, text=True)
        self.addCleanup(lambda: (process.terminate(), process.wait(), process.stdout.close()))
        self.assertEqual(process.stdout.readline().strip(), 'ready')
        (self.proc / str(process.pid)).symlink_to(Path('/proc') / str(process.pid), target_is_directory=True)
        return process

    def inspect(self, inodes=(), exemptions=()):
        return storage.active_references([str(self.candidate)], set(inodes), os.getuid(), exemptions, self.proc)

    def test_live_cwd_blocks_cleanup(self):
        self.child("import time; print('ready', flush=True); time.sleep(60)", self.candidate)
        with self.assertRaisesRegex(storage.SafetyError, 'active path'):
            self.inspect()

    def test_live_fd_blocks_even_through_hardlink_outside_candidate(self):
        artifact = self.candidate / 'artifact'
        artifact.write_text('content')
        link = self.root / 'hardlink'
        os.link(artifact, link)
        self.child(f"import time; f=open({str(link)!r}); print('ready', flush=True); time.sleep(60)")
        info = artifact.stat()
        with self.assertRaisesRegex(storage.SafetyError, 'active file descriptor'):
            self.inspect([(info.st_dev, info.st_ino)])

    def test_memory_mapping_blocks_after_original_fd_closed(self):
        artifact = self.candidate / 'artifact'
        artifact.write_text('content')
        self.child(f"import os,ctypes,time; fd=os.open({str(artifact)!r},os.O_RDONLY); "
                   "lib=ctypes.CDLL(None); lib.mmap.restype=ctypes.c_void_p; "
                   "m=lib.mmap(None,7,1,2,fd,0); assert m != ctypes.c_void_p(-1).value; os.close(fd); "
                   "print('ready',flush=True); time.sleep(60)")
        with self.assertRaisesRegex(storage.SafetyError, 'active memory mapping|active path'):
            info = artifact.stat()
            self.inspect([(info.st_dev, info.st_ino)])

    def test_exact_exception_only_waives_unreadable_matching_instance(self):
        process = self.child("import time; print('ready',flush=True); time.sleep(60)")
        identity = storage.process_identity(self.proc / str(process.pid))
        exemption = dict(identity, reason='controlled test service has unavailable maps')
        original = Path.read_text
        def read(path, *args, **kwargs):
            if path.name == 'maps':
                raise PermissionError('test deny')
            return original(path, *args, **kwargs)
        with patch.object(Path, 'read_text', read):
            with self.assertRaisesRegex(storage.SafetyError, 'cannot inspect owner PID'):
                self.inspect()
            self.assertEqual(self.inspect(exemptions=[exemption])['used_process_exemptions'], [exemption])
            exemption['starttime'] += 1
            with self.assertRaisesRegex(storage.SafetyError, 'cannot inspect owner PID'):
                self.inspect(exemptions=[exemption])

    def test_kernel_lock_blocks_cleanup(self):
        artifact = self.candidate / 'artifact'
        artifact.touch()
        info = artifact.stat()
        device = f'{os.major(info.st_dev):02x}:{os.minor(info.st_dev):02x}:{info.st_ino}'
        (self.proc / 'locks').write_text(f'1: FLOCK ADVISORY WRITE 123 {device} 0 EOF\n')
        with self.assertRaisesRegex(storage.SafetyError, 'kernel file lock'):
            self.inspect([(info.st_dev, info.st_ino)])

    def test_unsupported_host_is_refused(self):
        with patch.object(storage.sys, 'platform', 'darwin'):
            with self.assertRaisesRegex(storage.SafetyError, 'requires Linux'):
                self.inspect()


if __name__ == '__main__':
    unittest.main()
