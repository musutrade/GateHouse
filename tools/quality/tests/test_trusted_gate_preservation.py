"""Preserve the actual WAL transaction and raw failed reports before retirement."""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools/trusted_gate/deploy' / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BOOT = load('preservation_entry', 'import-protected-inputs.py')
KEEP = load('preservation_library', 'preserve-pilot-state.py')


class PreservationTests(unittest.TestCase):
    def setUp(self):
        mask = os.umask(0o077)
        self.addCleanup(os.umask, mask)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.volume = self.root / 'pilot'
        self.source = self.volume / 'source'
        self.control = self.volume / 'control'
        self.repository = self.volume / 'repository'
        for path in (self.source, self.control, self.repository / 'mirror.git'):
            path.mkdir(mode=0o700, parents=True)
        (self.control / 'controller.lock').touch(mode=0o600)
        lock = self.source / 'tools/harness-gate/Cargo.lock'
        lock.parent.mkdir(parents=True)
        lock.write_bytes(b'fixture source lock\n')
        self.git('init', '-q')
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'frozen')
        self.head = self.git('rev-parse', 'HEAD').decode().strip()
        tree = self.git('rev-parse', 'HEAD^{tree}').decode().strip()
        seed = self.root / 'seed'
        self.source.rename(seed)
        subprocess.run(['/usr/bin/git', 'clone', '--no-hardlinks', '--bare', str(seed), str(self.repository/'mirror.git')],
                       env=BOOT.ENV, check=True, capture_output=True)
        subprocess.run(['/usr/bin/git', '-C', str(self.repository/'mirror.git'), 'worktree', 'add',
                        '--detach', str(self.source), self.head], env=BOOT.ENV, check=True, capture_output=True)
        self.git('bundle', 'create', str(self.repository / 'candidate.bundle'), '--all')
        bundle = (self.repository / 'candidate.bundle').read_bytes()
        self.dump(self.repository / 'frozen.json', {'head': self.head, 'base': self.head,
                                                  'bundle_sha256': hashlib.sha256(bundle).hexdigest()})
        reports = self.source / '.gatehouse-host/reports/invocations/failed-invocation'
        reports.mkdir(parents=True)
        (reports / 'test_result.json').write_text('{"status":"FAIL"}')
        self.raw_report = reports / 'test_result.json'
        run_records = self.control / KEEP.RUN
        run_records.mkdir()
        (run_records / 'core.log').write_text('retained private failure\n')
        (self.control / 'pilot-validation-attempt.stderr').write_text('retained private traceback\n')
        self.config_path, self.marker_path = self.root / 'host.json', self.root / 'marker.json'
        config = {'source': str(self.source), 'mirror': str(self.repository / 'mirror.git'),
                  'records': str(self.control), 'environment': {'source_revision': self.head}}
        self.dump(self.config_path, config)
        self.dump(self.marker_path, {'head': self.head, 'release_sha256': KEEP.RELEASE})
        self.dump(self.control / 'pilot-acceptance.json', {'head': self.head, 'run': KEEP.RUN,
                  'release_sha256': KEEP.RELEASE, 'production_gate_enabled': False})
        policy = json.dumps(config, sort_keys=True, separators=(',', ':')).encode()
        inputs = json.dumps({'base': self.head, 'tree': tree, 'policy': hashlib.sha256(policy).hexdigest()})
        self.journal_path = self.control / 'journal.sqlite3'
        self.writer = sqlite3.connect(self.journal_path, isolation_level=None)
        self.addCleanup(self.writer.close)
        self.writer.execute('PRAGMA journal_mode=WAL')
        self.writer.execute('PRAGMA wal_autocheckpoint=0')
        self.writer.executescript('''CREATE TABLE runs (
            id TEXT PRIMARY KEY, repository TEXT NOT NULL, head TEXT NOT NULL,
            epoch INTEGER NOT NULL, input TEXT NOT NULL, local TEXT NOT NULL,
            evidence TEXT, remote TEXT NOT NULL, check_id INTEGER, pr INTEGER,
            reason TEXT, created REAL NOT NULL, UNIQUE(repository,head,epoch));
            CREATE TABLE events (sequence INTEGER PRIMARY KEY, run TEXT NOT NULL,
            time REAL NOT NULL, operation TEXT NOT NULL, detail TEXT NOT NULL);''')
        self.writer.execute('INSERT INTO runs VALUES (?,?,?,?,?,?,?,?,?,?,?,?)',
                            (KEEP.RUN, 'musutrade/GateHouse', self.head, 1, inputs, 'failed',
                             None, 'pending', None, None, 'failed pilot', 1.0))
        self.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)', (1, KEEP.RUN, 1.0, 'created', '{}'))
        self.destination = self.root / 'backup'
        self.real_owned = BOOT.owned
        self.B = SimpleNamespace(**{name: getattr(BOOT, name) for name in (
            'need', 'path_operand', 'read', 'write', 'record', 'canonical', 'unique', 'git', 'frozen_lock')})
        self.B.owned = self.owned
        self.B.authenticate = lambda *args: {'revision': 'a' * 40}
        self.builder = SimpleNamespace(quiet_units=lambda B: None, idle=lambda B: None)
        self.B.load = lambda *args: self.builder

    def dump(self, path, value):
        path.write_bytes(BOOT.canonical(value))

    def git(self, *args):
        return subprocess.check_output(['/usr/bin/git', '-C', str(self.source), *args],
                                       env=BOOT.ENV, stderr=subprocess.PIPE)

    def owned(self, path, directory=False, private=False):
        # Replace Root uid only inside ordinary fixture; retain mode/type/link guards.
        path = Path(path)
        if path != self.root and self.root not in path.parents:
            return self.real_owned(path, directory=directory, private=private)
        for parent in (path, *path.parents):
            BOOT.need(not parent.is_symlink(), 'linked fixture input')
        info = path.lstat()
        BOOT.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode), 'fixture type')
        BOOT.need(directory or info.st_nlink == 1, 'hard-linked fixture input')
        BOOT.need(not info.st_mode & 0o022 and (not private or not info.st_mode & 0o077), 'fixture mode')
        return path

    def domain(self):
        return patch.multiple(KEEP, VOLUME=self.volume, CONFIG=self.config_path, MARKER=self.marker_path, HEAD=self.head)

    def preserve(self, **kwargs):
        with self.domain(), patch.object(BOOT, 'owned', side_effect=self.owned), \
             patch.object(KEEP.os, 'geteuid', return_value=0):
            return KEEP.preserve(self.B, self.root/'helpers', self.root/'manifest', 'a'*64,
                                 self.destination, **kwargs)

    def blocked(self):
        value = json.loads((self.destination / 'preservation.json').read_text())
        self.assertEqual(value['state'], 'blocked')

    def test_wal_only_committed_rows_are_in_backup_without_checkpointing_original(self):
        before = self.journal_path.read_bytes()
        wal = self.journal_path.with_name('journal.sqlite3-wal')
        self.assertTrue(wal.exists())
        self.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)', (2, KEEP.RUN, 2.0, 'wal-only', '{}'))
        with self.domain(), patch.object(BOOT, 'owned', side_effect=self.owned):
            identity, run = KEEP.journal_snapshot(self.B, self.journal_path, self.root/'snapshot.sqlite3')
        self.assertEqual(identity['events']['rows'], 2)
        self.assertEqual(self.journal_path.read_bytes(), before)
        self.assertTrue(wal.exists())
        saved = sqlite3.connect(self.root/'snapshot.sqlite3')
        try:
            self.assertEqual(saved.execute('SELECT operation FROM events ORDER BY sequence').fetchall(),
                             [('created',), ('wal-only',)])
        finally:
            saved.close()
        self.assertFalse((self.root/'snapshot.sqlite3-wal').exists())
        self.assertFalse((self.root/'snapshot.sqlite3-shm').exists())

    def test_failed_pilot_without_registered_evidence_retains_all_raw_reports_and_git(self):
        value = self.preserve()
        self.assertEqual(value['state'], 'preserved')
        self.assertFalse(value['evidence_registered'])
        raw = self.destination/'tree/source/.gatehouse-host/reports/invocations/failed-invocation/test_result.json'
        self.assertEqual(raw.read_bytes(), self.raw_report.read_bytes())
        self.assertIn('control/pilot-validation-attempt.stderr', value['inventory']['files'])
        self.assertIn('source/.git', value['inventory']['files'])
        self.assertIn('repository/candidate.bundle', value['inventory']['files'])
        self.assertNotIn('control/journal.sqlite3-wal', value['inventory']['files'])
        self.assertTrue(self.source.exists())
        self.assertEqual(self.writer.execute('SELECT local,remote FROM runs').fetchone(), ('failed', 'pending'))

    def test_retained_git_objects_restore_after_original_linked_worktree_and_mirror_disappear(self):
        self.preserve()
        self.source.rename(self.root/'unavailable-source')
        self.repository.rename(self.root/'unavailable-repository')
        restored = self.root/'restored'
        subprocess.run(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null', 'clone', '--no-hardlinks',
            '--no-checkout', str(self.destination/'tree/repository/mirror.git'), str(restored)],
            env=BOOT.ENV, check=True, capture_output=True)
        head = subprocess.check_output(['/usr/bin/git', '-C', str(restored), 'rev-parse', 'HEAD'], env=BOOT.ENV)
        self.assertEqual(head.decode().strip(), self.head)

    def test_registered_evidence_is_preserved_without_granting_quality_or_remote_transition(self):
        self.writer.execute('UPDATE runs SET local=?,evidence=?', ('passed', '{"directory":"raw"}'))
        value = self.preserve()
        self.assertTrue(value['evidence_registered'])
        self.assertFalse(value['production_gate_enabled'])
        self.assertEqual(self.writer.execute('SELECT remote FROM runs').fetchone()[0], 'pending')

    def test_retention_verification_rejects_wrong_pin_and_changed_metadata(self):
        self.preserve()
        pin = hashlib.sha256((self.destination/'preservation.json').read_bytes()).hexdigest()
        with self.domain(), patch.object(BOOT, 'owned', side_effect=self.owned):
            self.assertEqual(KEEP.verify(self.B, self.destination, pin)['state'], 'preserved')
            with self.assertRaises(ValueError): KEEP.verify(self.B, self.destination, '0'*64)
            saved = self.destination/'host.json'
            saved.chmod(0o600); saved.write_bytes(b'{}'); saved.chmod(0o400)
            with self.assertRaises(ValueError): KEEP.verify(self.B, self.destination, pin)

    def test_retention_verification_rejects_missing_or_extra_raw_files(self):
        self.preserve()
        pin = hashlib.sha256((self.destination/'preservation.json').read_bytes()).hexdigest()
        root = self.destination/'tree'
        with self.domain(), patch.object(BOOT, 'owned', side_effect=self.owned):
            path = root/'extra'
            path.write_bytes(b'extra'); path.chmod(0o400)
            with self.assertRaises(ValueError): KEEP.verify(self.B, self.destination, pin)
            path.unlink()
            (root/'control/pilot-validation-attempt.stderr').unlink()
            with self.assertRaises(ValueError): KEEP.verify(self.B, self.destination, pin)

    def test_partial_existing_output_is_never_reused(self):
        self.destination.mkdir()
        (self.destination/'partial').touch()
        with self.assertRaises(ValueError): self.preserve()
        self.assertTrue((self.destination/'partial').exists())

    def test_copy_corruption_cannot_seal_success(self):
        write = self.B.write
        def altered(path, raw, mode=0o444):
            if Path(path).name == 'core.log': raw += b'corrupt'
            write(path, raw, mode)
        with patch.object(self.B, 'write', side_effect=altered):
            with self.assertRaises(ValueError): self.preserve()
        self.blocked()

    def test_source_mutation_during_copy_is_retained_blocked(self):
        original = KEEP.inventory
        def changed(B, roots, output=None):
            result = original(B, roots, output)
            if output is not None: self.raw_report.write_text('changed')
            return result
        with patch.object(KEEP, 'inventory', side_effect=changed):
            with self.assertRaises(ValueError): self.preserve()
        self.blocked()

    def test_journal_change_during_copy_blocks_completion(self):
        original = KEEP.inventory
        def changed(B, roots, output=None):
            result = original(B, roots, output)
            if output is not None:
                self.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)', (2, KEEP.RUN, 2.0, 'unexpected', '{}'))
            return result
        with patch.object(KEEP, 'inventory', side_effect=changed):
            with self.assertRaises(ValueError): self.preserve()
        self.blocked()

    def test_corrupt_retained_database_blocks_completion(self):
        original = KEEP.inventory
        def changed(B, roots, output=None):
            result = original(B, roots, output)
            if output is not None:
                saved = self.destination/'journal.sqlite3'
                saved.chmod(0o600)
                saved.write_bytes(b'corrupt database')
                saved.chmod(0o400)
            return result
        with patch.object(KEEP, 'inventory', side_effect=changed):
            with self.assertRaises(sqlite3.DatabaseError): self.preserve()
        self.blocked()

    def test_symlinks_hardlinks_and_nonregular_files_are_rejected(self):
        path = self.source/'unsafe'
        for kind in ('symlink', 'hardlink', 'fifo'):
            with self.subTest(kind=kind):
                if kind == 'symlink': path.symlink_to(self.raw_report)
                elif kind == 'hardlink': os.link(self.raw_report, path)
                else: os.mkfifo(path)
                with self.assertRaises(ValueError): self.preserve()
                self.blocked()
                path.unlink()
                self.destination = self.root / ('backup-after-' + kind)

    def test_wrong_or_running_or_revoked_slot_blocks_snapshot(self):
        for column, value in [('head', 'b'*40), ('local', 'running'), ('remote', 'revoked'), ('pr', 127)]:
            with self.subTest(column=column):
                old = self.writer.execute('SELECT '+column+' FROM runs').fetchone()[0]
                self.writer.execute('UPDATE runs SET '+column+'=?', (value,))
                with self.assertRaises(ValueError): self.preserve()
                self.blocked()
                self.writer.execute('UPDATE runs SET '+column+'=?', (old,))
                self.destination = self.root / ('backup-after-' + column)

    def test_unknown_schema_and_bounded_copy_fail_closed(self):
        self.writer.execute('CREATE VIEW unexpected AS SELECT * FROM runs')
        with self.assertRaises(ValueError): self.preserve()
        self.blocked()
        self.writer.execute('DROP VIEW unexpected')
        self.destination = self.root/'bounded'
        with patch.object(KEEP, 'MAX_TOTAL', 1):
            with self.assertRaises(ValueError): self.preserve()
        self.blocked()

    def test_inherited_lease_is_retained_and_wrong_descriptor_refused(self):
        with (self.control/'controller.lock').open('r+b') as stream:
            fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError): self.preserve()
            self.assertFalse(self.destination.exists())
            self.preserve(lock_fd=stream.fileno())
            os.fstat(stream.fileno())
            with (self.control/'controller.lock').open('r+b') as rival:
                with self.assertRaises(BlockingIOError):
                    fcntl.flock(rival.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        self.destination = self.root/'another'
        with self.config_path.open('rb') as stream:
            with self.assertRaises(ValueError): self.preserve(lock_fd=stream.fileno())
        self.assertFalse(self.destination.exists())

    def test_drifted_bundle_or_accepted_configuration_blocks_backup(self):
        for name in ('bundle', 'policy'):
            with self.subTest(name=name):
                path = self.repository/'candidate.bundle' if name == 'bundle' else self.config_path
                old = path.read_bytes()
                if name == 'bundle': path.write_bytes(b'changed')
                else:
                    value = json.loads(old); value['unknown'] = True; self.dump(path, value)
                with self.assertRaises(ValueError): self.preserve()
                self.blocked()
                path.write_bytes(old)
                self.destination = self.root / ('backup-after-' + name)

    def test_real_nonroot_cli_rejects_before_creating_output(self):
        if os.geteuid() == 0: self.skipTest('requires actual ordinary user')
        result = subprocess.run([sys.executable, '-B', str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),
            'preserve-pilot', '--helper-root', str(self.root/'missing'), '--manifest', str(self.root/'missing.json'),
            '--manifest-sha256', 'a'*64, '--destination', str(self.destination)], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'preservation blocked', result.stderr)
        self.assertFalse(self.destination.exists())


if __name__ == '__main__':
    unittest.main()
