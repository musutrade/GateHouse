"""Historical evidence restoration must validate bytes and contain extraction."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('restore_evidence', ROOT / 'tools/maintenance/restore_evidence.py')
restore = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(restore)


class EvidenceArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / 'evidence.tar'
        self.destination = self.root / 'restored'
        self.files = {'docs/old/failure.log': b'failed\n', 'docs/old/pass.log': b'passed\n'}

    def index(self, entries=None, supplemental=False):
        with tarfile.open(self.archive, 'w') as bundle:
            for name, value in self.files.items() if entries is None else entries:
                info = tarfile.TarInfo(name)
                if isinstance(value, bytes):
                    info.size = len(value)
                    bundle.addfile(info, io.BytesIO(value))
                else:
                    info.type = value
                    info.linkname = '../../escaped'
                    bundle.addfile(info)
        record = {'archive_sha256': hashlib.sha256(self.archive.read_bytes()).hexdigest(),
                  'file_count': len(self.files), 'files': {
                      name: {'size': len(value), 'sha256': hashlib.sha256(value).hexdigest()}
                      for name, value in self.files.items()}}
        if supplemental:
            return {'schema': 'quality-evidence-archive-index/v1',
                    'archive_sha256': '0' * 64, 'file_count': 1,
                    'files': {'docs/legacy.log': {'size': 0, 'sha256': hashlib.sha256(b'').hexdigest()}},
                    'supplemental_archives': [record]}
        return {'schema': 'quality-evidence-archive-index/v1', **record}

    def run_restore(self, index):
        return restore.restore(index, self.archive, self.destination, ['docs/old/failure.log'])

    def test_restores_selected_bytes_from_original_and_supplemental_records(self):
        for supplemental in (False, True):
            with self.subTest(supplemental=supplemental):
                self.destination = self.root / str(supplemental)
                result = self.run_restore(self.index(supplemental=supplemental))
                self.assertEqual(result['verified_members'], 2)
                self.assertEqual((self.destination / 'docs/old/failure.log').read_bytes(), b'failed\n')
                self.assertFalse((self.destination / 'docs/old/pass.log').exists())

    def test_tampered_archive_or_member_leaves_no_output(self):
        index = self.index()
        self.archive.write_bytes(self.archive.read_bytes() + b'tampered')
        with self.assertRaisesRegex(ValueError, 'archive SHA-256'):
            self.run_restore(index)
        index = self.index()
        index['files']['docs/old/pass.log']['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'member digest'):
            self.run_restore(index)
        self.assertFalse(self.destination.exists())

    def test_rejects_traversal_links_duplicates_missing_and_unindexed_members(self):
        cases = [
            [('../escaped', b'bad')],
            [('docs/old/failure.log', tarfile.SYMTYPE)],
            [('docs/old/failure.log', tarfile.LNKTYPE)],
            list(self.files.items()) + [('docs/old/pass.log', b'passed\n')],
            [('docs/old/failure.log', b'failed\n')],
            list(self.files.items()) + [('docs/unindexed.log', b'unknown')],
        ]
        for entries in cases:
            with self.subTest(entries=entries):
                with self.assertRaises(ValueError):
                    self.run_restore(self.index(entries))
                self.assertFalse(self.destination.exists())
                self.assertFalse((self.root / 'escaped').exists())

    def test_rejects_existing_destination_and_bad_index(self):
        index = self.index()
        self.destination.mkdir()
        with self.assertRaisesRegex(ValueError, 'destination must not exist'):
            self.run_restore(index)
        self.destination.rmdir()
        for name in ('/absolute', '../escape', 'docs/../escape', 'docs\\escape', 'C:/escape', './docs/file'):
            bad = copy.deepcopy(index)
            bad['files'][name] = bad['files'].pop('docs/old/pass.log')
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.run_restore(bad)
        bad = copy.deepcopy(index)
        bad['supplemental_archives'] = [copy.deepcopy(index)]
        with self.assertRaisesRegex(ValueError, 'duplicate indexed'):
            self.run_restore(bad)

    def test_repository_index_has_unique_paths_and_complete_records(self):
        index = json.loads((ROOT / 'docs/quality/archived-evidence-index.json').read_text())
        restore.archive_records(index)


if __name__ == '__main__':
    unittest.main()
