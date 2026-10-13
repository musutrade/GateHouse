"""Pinned downloads fail closed and never overwrite local evidence."""
import copy
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'maintenance'))
import test_data as data


class ExternalDataTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.root = self.base / 'checkout'
        self.root.mkdir()
        self.cache = self.base / 'cache'
        self.cache.mkdir()
        self.name = 'docs/quality/gh-1/evidence.json'
        self.payload = b'{"failure":true}\n'
        self.member = {'size': len(self.payload), 'sha256': hashlib.sha256(self.payload).hexdigest()}
        self.row = {'group': 'quality-history', 'url': 'https://github.com/musutrade/Harness-Gate/releases/download/source-data-20261013/quality-history-20261013.tar.gz',
                    'files': {self.name: self.member}, 'file_count': 1}
        self.make_archive([(self.name, self.payload)])
        self.manifest = {'schema': 'external-test-data/v1', 'source_commit': 'a' * 40, 'archives': [self.row]}

    def make_archive(self, members):
        path = self.base / 'data.tar.gz'
        with tarfile.open(path, 'w:gz') as archive:
            for name, payload in members:
                item = tarfile.TarInfo(name)
                item.size = len(payload)
                archive.addfile(item, io.BytesIO(payload))
        self.row.update(archive_size=path.stat().st_size, archive_sha256=data.digest(path))
        self.cached = self.cache / (self.row['archive_sha256'] + '.tar.gz')
        self.cached.write_bytes(path.read_bytes())

    def read(self, manifest=None):
        path = self.base / 'manifest.json'
        path.write_text(json.dumps(manifest or self.manifest))
        return data.read_manifest(path)

    def hydrate(self):
        return data.hydrate(self.root, self.read(), self.cache, {'quality-history'}, True)

    def test_exact_restore_and_offline_idempotence(self):
        self.assertEqual(self.hydrate()['restored_files'], 1)
        self.assertEqual((self.root / self.name).read_bytes(), self.payload)
        self.cached.unlink()
        self.assertEqual(self.hydrate()['restored_files'], 0)

    def test_corrupt_cache_fails_before_writing(self):
        self.cached.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'corrupt cached'):
            self.hydrate()
        self.assertFalse((self.root / self.name).exists())

    def test_offline_miss_does_not_attempt_network(self):
        self.cached.unlink()
        with patch.object(data.urllib.request, 'urlopen', side_effect=AssertionError('network')):
            with self.assertRaisesRegex(ValueError, 'offline cache miss'):
                self.hydrate()

    def test_changed_existing_file_is_never_overwritten(self):
        path = self.root / self.name
        path.parent.mkdir(parents=True)
        path.write_bytes(b'local edit')
        with self.assertRaisesRegex(ValueError, 'refusing overwrite'):
            self.hydrate()
        self.assertEqual(path.read_bytes(), b'local edit')

    def test_symlink_parent_is_rejected(self):
        try:
            (self.root / 'docs').symlink_to(self.base, target_is_directory=True)
        except OSError:
            self.skipTest('symlink privilege unavailable')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.hydrate()

    def test_path_traversal_and_source_paths_are_rejected(self):
        for name in ('../outside.json', 'docs/quality/../../outside.json', 'tools/harness-gate/src/main.rs', 'docs/quality/script.py'):
            value = copy.deepcopy(self.manifest)
            value['archives'][0]['files'] = {name: self.member}
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.read(value)

    def test_unpinned_or_insecure_url_is_rejected(self):
        for url in ('http://github.com/x', 'https://evil.example/a', 'https://github.com/musutrade/Harness-Gate/releases/latest/a.tar.gz'):
            value = copy.deepcopy(self.manifest)
            value['archives'][0]['url'] = url
            with self.subTest(url=url), self.assertRaises(ValueError):
                self.read(value)

    def test_duplicate_json_keys_fail(self):
        path = self.base / 'manifest.json'
        path.write_text('{"schema":1,"schema":2}')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            data.read_manifest(path)

    def test_unindexed_archive_member_is_rejected(self):
        self.make_archive([(self.name, self.payload), ('docs/quality/extra.json', b'{}')])
        with self.assertRaisesRegex(ValueError, 'unindexed'):
            self.hydrate()
        self.assertFalse((self.root / self.name).exists())

    def test_wrong_member_hash_and_missing_member_are_rejected(self):
        for members in ([(self.name, b'x' * len(self.payload))], []):
            self.make_archive(members)
            with self.subTest(members=members), self.assertRaises(ValueError):
                self.hydrate()
            self.assertFalse((self.root / self.name).exists())

    def test_group_selection_does_not_silently_ignore_typos(self):
        with self.assertRaisesRegex(ValueError, 'unknown groups'):
            data.hydrate(self.root, self.read(), self.cache, {'typo'}, True)


if __name__ == '__main__':
    unittest.main()

class MeasuredSnapshotDataTests(unittest.TestCase):
    def test_base_and_head_restore_their_own_pinned_inputs(self):
        import os
        import shutil
        import subprocess
        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        import ci_quality
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            cache = root / 'cache/gatehouse-test-data'
            cache.mkdir(parents=True)
            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.PIPE, text=True).strip()
            git('init'); git('config', 'user.name', 'Snapshot Data Test'); git('config', 'user.email', 'test@example.invalid')
            for name in ('tools/harness-gate/src/main.rs', 'tools/quality/fixture.txt', 'schema/fixture.json', 'docs/dogfood/fixture.txt'):
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text('fixture')
            scripts = root / 'tools/maintenance'; scripts.mkdir()
            for name in ('test_data.py', 'restore_evidence.py'):
                shutil.copyfile(Path(data.__file__).parent / name, scripts / name)
            (root / 'test-data').mkdir()
            commits = []
            for label in ('base', 'head'):
                rows = []
                for group, name in [('regression-corpus', 'tools/quality/fixtures/pinned.json'), ('arc-admin-replay', 'docs/dogfood/pinned.json')]:
                    payload = label.encode()
                    archive = root / 'payload.tar.gz'
                    with tarfile.open(archive, 'w:gz') as bundle:
                        member = tarfile.TarInfo(name); member.size = len(payload)
                        bundle.addfile(member, io.BytesIO(payload))
                    sha = data.digest(archive)
                    shutil.copyfile(archive, cache / (sha + '.tar.gz'))
                    rows.append({'group': group, 'url': 'https://github.com/musutrade/Harness-Gate/releases/download/source-data-20261013/pinned.tar.gz', 'archive_size': archive.stat().st_size, 'archive_sha256': sha,
                                 'file_count': 1, 'files': {name: {'size': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}}})
                (root / 'test-data/manifest.json').write_text(json.dumps({'schema': 'external-test-data/v1', 'source_commit': 'a'*40, 'archives': rows}))
                git('add', 'tools', 'docs', 'schema', 'test-data'); git('commit', '-m', label)
                commits.append(git('rev-parse', 'HEAD'))
            with patch.object(ci_quality, 'ROOT', root), patch.dict(os.environ, XDG_CACHE_HOME=str(root / 'cache')):
                collector = ci_quality.Collector(root / 'candidate', *commits, 'external-data-test')
                for label, commit in zip(('base','head'), commits):
                    snapshot = collector.snapshot(label, commit)
                    self.assertEqual((snapshot / 'tools/quality/fixtures/pinned.json').read_text(), label)
                    self.assertEqual((snapshot / 'docs/dogfood/pinned.json').read_text(), label)
