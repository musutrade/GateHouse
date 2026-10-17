"""Disposable worker preparation cannot consume stale/configured or changed inputs."""
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('worker_workspace',
    ROOT / 'tools/trusted_gate/deploy/prepare-linux-workspace.py')
WORKSPACE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WORKSPACE)


@unittest.skipIf(os.geteuid() == 0, 'requires a real unprivileged worker identity')
class WorkspaceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        self.history = self.root / 'history'
        self.cache = self.root / 'archives'
        self.cache.mkdir()
        self.destination = self.root / 'workspace'
        (self.source / 'tools/maintenance').mkdir(parents=True)
        for name in ('test_data.py', 'restore_evidence.py'):
            shutil.copyfile(ROOT / 'tools/maintenance' / name, self.source / 'tools/maintenance' / name)
        self.name, self.data = 'docs/quality/input.json', b'{"fixture":true}\n'
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode='w:gz') as archive:
            member = tarfile.TarInfo(self.name)
            member.size = len(self.data)
            archive.addfile(member, io.BytesIO(self.data))
        archive_bytes = stream.getvalue()
        digest = WORKSPACE.digest(archive_bytes)
        self.archive = self.cache / (digest + '.tar.gz')
        self.archive.write_bytes(archive_bytes)
        manifest = {'schema': 'external-test-data/v1', 'source_commit': 'a' * 40,
            'archives': [{'group': 'quality-history',
                'url': 'https://github.com/musutrade/Harness-Gate/releases/download/source-data-20261013/quality-history-20261013.tar.gz',
                'archive_size': len(archive_bytes), 'archive_sha256': digest, 'file_count': 1,
                'files': {self.name: {'size': len(self.data), 'sha256': WORKSPACE.digest(self.data)}}}]}
        (self.source / 'test-data').mkdir()
        (self.source / 'test-data/manifest.json').write_text(json.dumps(manifest))
        (self.source / '.gitignore').write_text('/docs/quality/input.json\n')
        self.git(self.source, 'init', '--template=')
        self.git(self.source, 'add', '.')
        self.commit('base')
        self.base = self.git(self.source, 'rev-parse', 'HEAD')
        (self.source / 'candidate.txt').write_text('candidate\n')
        self.git(self.source, 'add', '.')
        self.commit('candidate')
        self.head = self.git(self.source, 'rev-parse', 'HEAD')
        self.git(self.root, 'init', '--bare', '--template=', str(self.history))
        self.git(self.history, 'fetch', '--no-tags', str(self.source), self.head, self.base)
        self.git(self.history, 'update-ref', 'refs/heads/candidate', self.head)
        self.git(self.history, 'update-ref', 'refs/gatehouse/base', self.base)
        self.git(self.history, 'symbolic-ref', 'HEAD', 'refs/heads/candidate')
        self.git(self.history, '--work-tree=' + str(self.source), 'reset', '--mixed', self.head)
        self.git(self.history, 'config', '--local', 'core.hooksPath', '/dev/null')
        self.git(self.history, 'config', '--local', 'core.bare', 'false')
        (self.history / 'FETCH_HEAD').unlink()
        (self.source / '.git').rename(self.root / 'controller')
        (self.source / '.git').write_text('gitdir: ' + str(self.history) + '\n')
        self.pin = WORKSPACE.digest((self.source / 'test-data/manifest.json').read_bytes())

    def git(self, root, *args):
        return subprocess.check_output(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
            '-C', str(root), *args], env=WORKSPACE.environment(), stderr=subprocess.PIPE).decode().strip()

    def commit(self, message):
        self.git(self.source, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-m', message)

    def prepare(self, **changes):
        values = dict(source=self.source, history=self.history, destination=self.destination,
                      cache=self.cache, head=self.head, base=self.base, manifest_sha256=self.pin)
        values.update(changes)
        return WORKSPACE.prepare(**values)

    def test_real_offline_restore_and_independent_fixture_identity(self):
        record = self.prepare()
        self.assertEqual(record['state'], 'prepared')
        self.assertEqual(record['restored_files'], 1)
        self.assertEqual((self.destination / self.name).read_bytes(), self.data)
        self.assertFalse((self.source / self.name).exists())
        self.assertEqual(self.git(self.destination, 'rev-parse', 'HEAD'), self.head)
        self.assertEqual(self.git(self.destination, 'rev-parse', 'refs/gatehouse/base'), self.base)
        self.assertEqual(self.git(self.destination, 'remote'), '')
        self.assertFalse((self.destination / '.git/objects/info/alternates').exists())
        saved = json.loads((self.root / 'workspace-input-preparation.json').read_text())
        self.assertEqual(saved, record)

    def test_missing_archive_is_not_downloaded_or_reused(self):
        self.archive.unlink()
        with self.assertRaisesRegex(ValueError, 'offline input restoration failed'):
            self.prepare()
        record = json.loads((self.root / 'workspace-input-preparation.json').read_text())
        self.assertEqual(record['state'], 'incomplete')
        self.assertIn('offline cache miss', record['stderr'])
        self.assertNotEqual(record['restore_exit_code'], 0)
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.prepare()

    def test_corrupt_archive_preserves_failure_and_input(self):
        self.archive.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'offline input restoration failed'):
            self.prepare()
        record = json.loads((self.root / 'workspace-input-preparation.json').read_text())
        self.assertIn('corrupt cached archive', record['stderr'])
        self.assertEqual(self.archive.read_bytes(), b'corrupt')
        self.assertFalse((self.destination / self.name).exists())

    def test_restoration_budget_is_checked_before_any_workspace_write(self):
        path = self.source / 'test-data/manifest.json'
        manifest = json.loads(path.read_text())
        manifest['archives'][0]['files'][self.name]['size'] = 129 * 1024 * 1024
        path.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, 'restored input size'):
            self.prepare(manifest_sha256=WORKSPACE.digest(path.read_bytes()))
        self.assertFalse(self.destination.exists())

    def test_linked_cache_path_and_external_history_objects_are_rejected(self):
        alias = self.root / 'cache-alias'
        alias.symlink_to(self.cache, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'linked workspace path'):
            self.prepare(cache=alias)
        (self.history / 'objects/info/alternates').symlink_to(self.root / 'missing')
        with self.assertRaisesRegex(ValueError, 'external history objects'):
            self.prepare()
        self.assertFalse(self.destination.exists())

    def test_helper_timeout_retains_incomplete_preparation_record(self):
        actual = subprocess.run
        def timeout_helper(argv, **kwargs):
            if argv[0] == '/usr/bin/python3':
                raise subprocess.TimeoutExpired(argv, 120)
            return actual(argv, **kwargs)
        with patch.object(WORKSPACE.subprocess, 'run', side_effect=timeout_helper):
            with self.assertRaises(subprocess.TimeoutExpired):
                self.prepare()
        self.assertEqual(json.loads((self.root / 'workspace-input-preparation.json').read_text())['state'],
                         'incomplete')

    def test_wrong_head_base_or_manifest_blocks_before_materialization(self):
        for changes in ({'head': 'a' * 40}, {'base': 'b' * 40}, {'manifest_sha256': 'c' * 64}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.prepare(**changes)
            self.assertFalse(self.destination.exists())

    def test_source_controller_pointer_or_unsafe_history_configuration_is_rejected(self):
        self.git(self.history, 'config', '--local', 'remote.origin.url', 'https://example.invalid/repository')
        with self.assertRaisesRegex(ValueError, 'not sanitized'):
            self.prepare()
        self.git(self.history, 'config', '--local', '--remove-section', 'remote.origin')
        (self.source / '.git').write_text('gitdir: ' + str(self.root / 'controller') + '\n')
        with self.assertRaisesRegex(ValueError, 'sanitized history view'):
            self.prepare()

    def test_dirty_source_or_overlapping_workspace_is_rejected(self):
        (self.source / 'candidate.txt').write_text('changed\n')
        with self.assertRaisesRegex(ValueError, 'tracked bytes changed'):
            self.prepare()
        with self.assertRaisesRegex(ValueError, 'separate'):
            self.prepare(destination=self.source / 'generated')

    def test_hiding_changed_bytes_with_skip_worktree_is_rejected(self):
        self.git(self.source, 'update-index', '--skip-worktree', 'candidate.txt')
        (self.source / 'candidate.txt').write_text('changed\n')
        self.assertEqual(self.git(self.source, 'status', '--porcelain', '--untracked-files=no'), '')
        with self.assertRaisesRegex(ValueError, 'workspace bytes or mode differ'):
            self.prepare()
        self.assertFalse(self.destination.exists())

    def test_worker_helper_cannot_hide_mutation_in_its_writable_index(self):
        actual = subprocess.run
        def mutate_after_restore(argv, **kwargs):
            result = actual(argv, **kwargs)
            if argv[0] == '/usr/bin/python3':
                self.git(self.destination, 'update-index', '--skip-worktree', 'candidate.txt')
                (self.destination / 'candidate.txt').write_text('changed\n')
            return result
        with patch.object(WORKSPACE.subprocess, 'run', side_effect=mutate_after_restore):
            with self.assertRaisesRegex(ValueError, 'workspace bytes or mode differ'):
                self.prepare()
        self.assertEqual(json.loads((self.root / 'workspace-input-preparation.json').read_text())['state'],
                         'incomplete')

    def test_root_execution_is_forbidden_before_creating_a_workspace(self):
        with patch.object(WORKSPACE.os, 'geteuid', return_value=0):
            with self.assertRaisesRegex(ValueError, 'unprivileged worker'):
                self.prepare()
        self.assertFalse(self.destination.exists())


if __name__ == '__main__':
    unittest.main()
