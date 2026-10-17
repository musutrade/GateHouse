"""Build complete directory sources from locked archives, never cache metadata."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('locked_directory',
    ROOT / 'tools/trusted_gate/deploy/prepare-locked-directory.py')
DIRECTORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIRECTORY)


@unittest.skipIf(os.geteuid() == 0, 'directory preparation requires an unprivileged operator')
class LockedDirectoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.cleanup)
        self.root = Path(self.temp.name)
        self.archives = self.root / 'archives'
        self.archives.mkdir()
        self.archive = self.archives / 'fixture-1.2.3.crate'
        self.lock = self.root / 'Cargo.lock'
        self.destination = self.root / 'prepared'
        self.members = [('src/lib.rs', b'pub fn exact() {}\n', 0o644, tarfile.REGTYPE),
                        ('.gitignore', b'/target\n', 0o644, tarfile.REGTYPE),
                        ('script', b'#!/bin/sh\n', 0o755, tarfile.REGTYPE)]
        self.make_archive()

    def cleanup(self):
        for root, directories, names in os.walk(self.root):
            os.chmod(root, 0o700)
        self.temp.cleanup()

    def make_archive(self, members=None):
        with tarfile.open(self.archive, 'w:gz') as bundle:
            for name, data, mode, kind in self.members if members is None else members:
                member = tarfile.TarInfo('fixture-1.2.3/' + name)
                member.size, member.mode, member.type = len(data), mode, kind
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                    member.linkname = 'src/lib.rs'
                bundle.addfile(member, io.BytesIO(data))
        self.checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.lock.write_text('version = 4\n[[package]]\nname = "fixture"\nversion = "1.2.3"\n'
            'source = "' + DIRECTORY.CRATES.SOURCE + '"\nchecksum = "' + self.checksum + '"\n')
        self.pin = hashlib.sha256(self.lock.read_bytes()).hexdigest()

    def prepare(self):
        return DIRECTORY.prepare(self.lock, self.pin, self.archives, self.destination)

    def verify(self):
        return DIRECTORY.verify(self.destination, self.pin)

    @property
    def package(self):
        return self.destination / 'sources/fixture-1.2.3'

    def test_exact_full_source_metadata_and_private_archive_are_readonly(self):
        original = self.archive.read_bytes()
        result = self.prepare()
        self.assertEqual(result['files'], 3)
        self.assertEqual(result['archive_bytes'], len(original))
        self.assertEqual(result, self.verify())
        self.assertEqual((self.package / '.gitignore').read_bytes(), b'/target\n')
        self.assertEqual((self.package / 'script').stat().st_mode & 0o777, 0o555)
        self.assertEqual((self.destination / 'archives' / self.archive.name).read_bytes(), original)
        self.assertEqual(self.archive.read_bytes(), original)
        self.assertFalse((self.package / '.cargo-ok').exists())
        checksum = json.loads((self.package / '.cargo-checksum.json').read_text())
        self.assertEqual(checksum['package'], self.checksum)
        self.assertEqual(set(checksum['files']), {row[0] for row in self.members})
        self.assertEqual(self.destination.stat().st_mode & 0o777, 0o555)
        self.assertEqual(json.loads(self.destination.with_name('prepared-preparation.json').read_text())['state'], 'prepared')

    def test_wrong_lock_digest_and_output_overlap_block_before_output(self):
        with self.assertRaisesRegex(ValueError, 'trusted digest'):
            DIRECTORY.prepare(self.lock, '0' * 64, self.archives, self.destination)
        with self.assertRaisesRegex(ValueError, 'overlaps'):
            DIRECTORY.prepare(self.lock, self.pin, self.archives, self.archives / 'nested')
        self.assertFalse(self.destination.exists())

    def test_corrupt_archive_retains_failure_and_refuses_reuse(self):
        self.archive.write_bytes(b'not the locked archive')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.prepare()
        record = self.destination.with_name('prepared-preparation.json')
        self.assertEqual(json.loads(record.read_text())['state'], 'blocked')
        self.assertTrue((self.destination / 'archives' / self.archive.name).exists())
        with self.assertRaisesRegex(ValueError, 'retain old or partial'):
            self.prepare()

    def test_copied_archive_is_used_after_original_input_changes(self):
        original = DIRECTORY.copy_archive
        def changed(source, destination, expected, limit=None):
            original(source, destination, expected, limit)
            source.write_bytes(b'changed after private snapshot')
        with patch.object(DIRECTORY, 'copy_archive', side_effect=changed):
            result = self.prepare()
        self.assertEqual(result, self.verify())

    def test_archive_traversal_links_duplicates_and_specials_block_before_extraction(self):
        for members in ([('../escape', b'x', 0o644, tarfile.REGTYPE)],
                        [('linked', b'', 0o644, tarfile.SYMTYPE)],
                        [self.members[0], self.members[0]],
                        [('fifo', b'', 0o644, tarfile.FIFOTYPE)]):
            self.make_archive(members)
            with self.subTest(members=members), self.assertRaises(ValueError):
                self.prepare()
            self.assertFalse((self.destination / 'sources/fixture-1.2.3').exists())
            # Use a distinct fresh output for each retained failed attempt.
            self.destination = self.destination.with_name(self.destination.name + '-next')

    def test_forged_checksum_and_inventory_cannot_hide_changed_source(self):
        self.prepare()
        changed = self.package / 'src/lib.rs'
        changed.chmod(0o644); changed.write_bytes(b'changed dependency'); changed.chmod(0o444)
        metadata = self.package / '.cargo-checksum.json'
        value = json.loads(metadata.read_text())
        value['files']['src/lib.rs'] = hashlib.sha256(changed.read_bytes()).hexdigest()
        metadata.chmod(0o644); metadata.write_bytes(DIRECTORY.canonical(value)); metadata.chmod(0o444)
        inventory = self.destination / 'inventory.json'
        data = json.loads(inventory.read_text())
        data['packages']['fixture-1.2.3']['cargo_checksum_sha256'] = hashlib.sha256(metadata.read_bytes()).hexdigest()
        inventory.chmod(0o644); inventory.write_bytes(DIRECTORY.canonical(data)); inventory.chmod(0o444)
        with self.assertRaisesRegex(ValueError, 'checksum metadata differs'):
            self.verify()

    def test_hidden_missing_extra_files_directories_and_marker_block(self):
        self.prepare()
        self.package.chmod(0o700)
        for name, directory in (('.hidden', False), ('extra-dir', True), ('.cargo-ok', False)):
            path = self.package / name
            path.mkdir() if directory else path.write_bytes(b'{"v":1}')
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.verify()
            path.rmdir() if directory else path.unlink()
        (self.package / '.gitignore').unlink()
        with self.assertRaisesRegex(ValueError, 'source differs'):
            self.verify()

    def test_changed_archive_extra_package_and_writable_permissions_block(self):
        self.prepare()
        self.package.chmod(0o700)
        with self.assertRaisesRegex(ValueError, 'writable'):
            self.verify()
        self.package.chmod(0o555)
        parent = self.destination / 'sources'; parent.chmod(0o700)
        (parent / 'extra-package').mkdir()
        with self.assertRaisesRegex(ValueError, 'package inventory differs'):
            self.verify()
        (parent / 'extra-package').rmdir(); parent.chmod(0o555)
        archive = self.destination / 'archives' / self.archive.name
        archive.chmod(0o644); archive.write_bytes(b'changed archive'); archive.chmod(0o444)
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.verify()

    def test_combined_size_count_and_metadata_budgets_are_enforced(self):
        for name, value in (('MAX_TOTAL', 1), ('MAX_FILES', 1), ('MAX_METADATA', 1)):
            with self.subTest(name=name), patch.object(DIRECTORY.CRATES, name, value), self.assertRaises(ValueError):
                self.prepare()
            self.destination = self.destination.with_name(self.destination.name + '-next')
        with patch.object(DIRECTORY, 'MAX_INVENTORY', 1), self.assertRaisesRegex(ValueError, 'inventory exceeds'):
            self.prepare()

    def test_lock_mutation_root_execution_and_linked_parents_block(self):
        with patch.object(DIRECTORY.os, 'geteuid', return_value=0), self.assertRaisesRegex(ValueError, 'unprivileged'):
            self.prepare()
        linked = self.root / 'linked'
        linked.symlink_to(self.archives, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'linked'):
            DIRECTORY.prepare(self.lock, self.pin, linked, self.destination)
        original = DIRECTORY.copy_archive
        def changed(source, destination, expected, limit=None):
            original(source, destination, expected, limit)
            self.lock.write_bytes(self.lock.read_bytes() + b'\n')
        with patch.object(DIRECTORY, 'copy_archive', side_effect=changed), self.assertRaisesRegex(ValueError, 'lock changed'):
            self.prepare()


if __name__ == '__main__':
    unittest.main()
