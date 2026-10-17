"""Offline runtime snapshots remain input artifacts, never operator authority."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('linux_runtime_staging',
    ROOT / 'tools/trusted_gate/deploy/stage-linux-inputs.py')
STAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STAGE)


@unittest.skipIf(os.geteuid() == 0, 'runtime staging requires an unprivileged operator')
class RuntimeStagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.cleanup)
        self.root = Path(self.temp.name)
        self.paths = {role: self.root / role for role in STAGE.ROLES}
        for path in self.paths.values():
            path.mkdir()
        for name in ('rustc', 'cargo', 'rustdoc'):
            path = self.paths['toolchain'] / 'bin' / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(b'fixture executable')
            path.chmod(0o755)
        for name in ('CACHEDIR.TAG', 'index', 'cache', 'src'):
            path = self.paths['registry'] / name
            if name == 'CACHEDIR.TAG':
                path.write_bytes(b'cache tag')
            else:
                path.mkdir()
        (self.paths['registry'] / 'cache/crate').write_bytes(b'offline crate')
        plugin = self.paths['onboarding'] / 'tools/onboarding/acceptance.py'
        plugin.parent.mkdir(parents=True)
        plugin.write_bytes(b'# pinned fixture\n')
        self.pin = STAGE.JOB.directory_digest(self.paths['onboarding'])
        pinned = patch.object(STAGE.JOB, 'ONBOARDING_SHA256', self.pin)
        pinned.start()
        self.addCleanup(pinned.stop)
        archive = b'exact pinned archive'
        sha = hashlib.sha256(archive).hexdigest()
        (self.paths['archives'] / (sha + '.tar.gz')).write_bytes(archive)
        (self.paths['archives'] / 'unselected-private-file').write_bytes(b'never copy')
        self.manifest = self.root / 'manifest.json'
        self.manifest.write_text(json.dumps({'schema': 'external-test-data/v1', 'archives': [
            {'archive_sha256': sha, 'archive_size': len(archive)}]}))
        self.manifest_sha = hashlib.sha256(self.manifest.read_bytes()).hexdigest()
        for name in ('nextest', 'node'):
            path = self.root / name
            path.write_bytes(b'fixture extra tool')
            path.chmod(0o755)
        self.destination = self.root / 'snapshot'

    def cleanup(self):
        for root, directories, names in os.walk(self.root):
            os.chmod(root, 0o700)
        self.temp.cleanup()

    def stage(self, **changes):
        args = {**self.paths, 'nextest': self.root / 'nextest', 'node': self.root / 'node',
                'manifest': self.manifest, 'manifest_sha256': self.manifest_sha,
                'destination': self.destination}
        args.update(changes)
        with patch.object(STAGE.JOB, 'ONBOARDING_SHA256', self.pin):
            return STAGE.stage(**args)

    def test_exact_offline_snapshot_is_readonly_and_excludes_unselected_inputs(self):
        report = self.stage()
        self.assertEqual(report['state'], 'staged')
        self.assertFalse(report['trusted_owner_verified'])
        value = json.loads((self.destination / 'inventory.json').read_text())
        self.assertNotIn('archives/unselected-private-file', value['files'])
        self.assertEqual(value['inputs']['test_data_manifest_sha256'], self.manifest_sha)
        self.assertEqual(STAGE.verify(self.destination, report['inventory_sha256'])['files'], 9)
        for root, directories, names in os.walk(self.destination):
            self.assertEqual(stat.S_IMODE(Path(root).stat().st_mode), 0o555)
            for name in names:
                self.assertFalse((Path(root) / name).stat().st_mode & 0o222)
        with self.assertRaisesRegex(ValueError, 'administrator'):
            STAGE.verify(self.destination, report['inventory_sha256'], trusted=True)

    def test_cargo_home_credentials_or_config_are_never_copied(self):
        for name in ('credentials.toml', 'config.toml', 'token'):
            path = self.paths['registry'] / name
            path.write_bytes(b'private fixture')
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'registry'):
                self.stage()
            path.unlink()
        self.assertFalse(self.destination.exists())

    def test_onboarding_and_manifest_pins_are_required_before_copy(self):
        with self.assertRaisesRegex(ValueError, 'manifest differs'):
            self.stage(manifest_sha256='0' * 64)
        (self.paths['onboarding'] / 'changed').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'onboarding source differs'):
            self.stage()
        self.assertFalse(self.destination.exists())

    def test_old_partial_destination_and_record_are_preserved(self):
        record = self.destination.with_name('snapshot-preparation.json')
        record.write_bytes(b'old failure')
        with self.assertRaisesRegex(ValueError, 'old or partial'):
            self.stage()
        self.assertEqual(record.read_bytes(), b'old failure')
        record.unlink()
        self.destination.mkdir()
        (self.destination / 'old').write_bytes(b'old bytes')
        with self.assertRaisesRegex(ValueError, 'old or partial'):
            self.stage()
        self.assertEqual((self.destination / 'old').read_bytes(), b'old bytes')

    def test_missing_or_bad_archive_leaves_blocked_state_and_no_reuse(self):
        archive = next(self.paths['archives'].glob('*.tar.gz'))
        archive.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'pinned archive differs'):
            self.stage()
        record = json.loads(self.destination.with_name('snapshot-preparation.json').read_text())
        self.assertEqual(record['state'], 'blocked')
        self.assertTrue(self.destination.exists())
        with self.assertRaisesRegex(ValueError, 'old or partial'):
            self.stage()

    def test_links_special_files_and_oversized_libraries_are_rejected(self):
        extra = self.paths['toolchain'] / 'extra'
        for kind in ('symlink', 'hardlink', 'fifo', 'oversize'):
            if kind == 'symlink':
                extra.symlink_to(self.root / 'node')
            elif kind == 'hardlink':
                os.link(self.root / 'node', extra)
            elif kind == 'fifo':
                os.mkfifo(extra)
            else:
                with extra.open('wb') as stream:
                    stream.truncate(STAGE.MAX_FILE + 1)
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                STAGE.inventory(self.paths['toolchain'])
            extra.unlink()

    def test_total_file_and_directory_count_budgets_are_bounded(self):
        with patch.object(STAGE, 'MAX_TOTAL', 1), self.assertRaisesRegex(ValueError, 'budget'):
            STAGE.inventory(self.paths['toolchain'])
        with patch.object(STAGE, 'MAX_FILES', 1), self.assertRaisesRegex(ValueError, 'budget'):
            STAGE.inventory(self.paths['toolchain'])

    def test_changed_source_during_copy_is_blocking_and_retained(self):
        original = STAGE.fingerprint
        def changed(path, copy_to=None):
            value = original(path, copy_to)
            if copy_to is not None and str(path).endswith('/bin/cargo'):
                Path(path).write_bytes(b'changed after copy')
            return value
        with patch.object(STAGE, 'fingerprint', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'source changed'):
                self.stage()
        self.assertEqual(json.loads(self.destination.with_name('snapshot-preparation.json').read_text())['state'], 'blocked')

    def test_changed_bytes_extra_file_executable_mode_and_permissions_invalidate_inventory(self):
        report = self.stage()
        target = self.destination / 'toolchain/bin/cargo'
        original = target.read_bytes()
        target.chmod(0o755)
        target.write_bytes(b'changed bytes')
        target.chmod(0o555)
        with self.assertRaisesRegex(ValueError, 'differ'):
            STAGE.verify(self.destination, report['inventory_sha256'])
        target.chmod(0o755)
        target.write_bytes(original)
        target.chmod(0o444)
        with self.assertRaisesRegex(ValueError, 'differ'):
            STAGE.verify(self.destination, report['inventory_sha256'])
        target.chmod(0o555)
        parent = self.destination / 'archives'
        parent.chmod(0o755)
        (parent / 'extra').write_bytes(b'unknown')
        with self.assertRaisesRegex(ValueError, 'differ'):
            STAGE.verify(self.destination, report['inventory_sha256'])
        (parent / 'extra').unlink()
        with self.assertRaisesRegex(ValueError, 'writable'):
            STAGE.verify(self.destination, report['inventory_sha256'])

    def test_wrong_manifest_digest_and_inventory_traversal_are_rejected(self):
        report = self.stage()
        with self.assertRaisesRegex(ValueError, 'reviewed digest'):
            STAGE.verify(self.destination, '0' * 64)
        for name in ('/root/file', '../file', 'toolchain/../file', 'toolchain//file', 'toolchain\\file'):
            with self.subTest(name=name), self.assertRaises(ValueError):
                STAGE.safe_path(name)

    def test_output_cannot_overlap_inputs(self):
        with self.assertRaisesRegex(ValueError, 'overlaps'):
            self.stage(destination=self.paths['registry'] / 'destination')
        self.assertFalse((self.paths['registry'] / 'destination').exists())

    def test_non_executable_extra_tool_is_not_silently_accepted(self):
        (self.root / 'node').chmod(0o600)
        with self.assertRaisesRegex(ValueError, 'not executable'):
            self.stage()
        self.assertEqual(json.loads(self.destination.with_name('snapshot-preparation.json').read_text())['state'], 'blocked')


if __name__ == '__main__':
    unittest.main()
