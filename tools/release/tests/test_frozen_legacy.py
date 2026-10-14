"""Keep the original paused-product protections executing after source cleanup."""
from pathlib import Path
import json
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import frozen_legacy


class FrozenLegacyTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for directory in ('retired/legacy-collector', 'docs/quality/issue-13'):
            shutil.copytree(frozen_legacy.ROOT / directory, root / directory)
        manifest = json.loads((root / frozen_legacy.MANIFEST).read_text())
        for relative in manifest['shared_sources'] + manifest['shared_inputs']:
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            if (frozen_legacy.ROOT / relative).is_dir():
                destination.mkdir()
            else:
                destination.write_text('shared input fixture')
        return root, manifest

    def test_changed_frozen_bytes_fail_before_execution(self):
        root, manifest = self.fixture()
        source = root / manifest['snapshot'] / next(iter(manifest['files']))
        source.write_text('changed')
        with self.assertRaisesRegex(ValueError, 'source changed'):
            frozen_legacy.verify(root)

    def test_unknown_frozen_file_cannot_escape_inventory(self):
        root, manifest = self.fixture()
        (root / manifest['snapshot'] / 'extra.py').write_text('unreviewed')
        with self.assertRaisesRegex(ValueError, 'file inventory changed'):
            frozen_legacy.verify(root)

    def test_restored_active_plugin_copy_fails(self):
        root, manifest = self.fixture()
        source = root / next(iter(manifest['files']))
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text('restored')
        with self.assertRaisesRegex(ValueError, 'active copy restored'):
            frozen_legacy.verify(root)

    def test_missing_shared_core_source_fails(self):
        root, manifest = self.fixture()
        (root / manifest['shared_sources'][0]).unlink()
        with self.assertRaisesRegex(ValueError, 'missing shared Core input'):
            frozen_legacy.verify(root)

    def test_hash_refresh_cannot_change_original_frozen_bytes(self):
        root, manifest = self.fixture()
        relative = next(iter(manifest['files']))
        manifest['files'][relative]['sha256'] = '0' * 64
        (root / frozen_legacy.MANIFEST).write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, 'source changed'):
            frozen_legacy.verify(root)

    def test_original_delivery_protections(self):
        result = frozen_legacy.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        print(result.stdout, end='')
