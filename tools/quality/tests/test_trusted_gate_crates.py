"""A cache marker/checksum cannot conceal changed locked dependency sources."""
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
SPEC = importlib.util.spec_from_file_location('locked_crates',
    ROOT / 'tools/trusted_gate/deploy/verify-locked-crates.py')
CRATES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CRATES)


class LockedCrateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.archives = self.root / 'cache'
        self.sources = self.root / 'src'
        self.archives.mkdir()
        self.sources.mkdir()
        self.package = 'fixture-1.2.3'
        self.source = self.sources / self.package
        self.source.mkdir()
        (self.source / 'src').mkdir()
        (self.source / 'src/lib.rs').write_bytes(b'pub fn original() {}\n')
        (self.source / '.gitignore').write_bytes(b'/target\n')
        (self.source / '.cargo-ok').write_bytes(b'{"v":1}')
        self.members = [('src/lib.rs', b'pub fn original() {}\n', 0o644, tarfile.REGTYPE),
                        ('.gitignore', b'/target\n', 0o644, tarfile.REGTYPE)]
        self.archive = self.archives / (self.package + '.crate')
        self.lock = self.root / 'Cargo.lock'
        self.make_archive()

    def make_archive(self, members=None):
        with tarfile.open(self.archive, 'w:gz') as bundle:
            for name, data, mode, kind in self.members if members is None else members:
                member = tarfile.TarInfo(self.package + '/' + name)
                member.mode, member.type, member.size = mode, kind, len(data)
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                    member.linkname = 'src/lib.rs'
                bundle.addfile(member, io.BytesIO(data))
        self.checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.write_lock()

    def write_lock(self, source=CRATES.SOURCE, checksum=None):
        self.lock.write_text('version = 4\n[[package]]\nname = "fixture"\nversion = "1.2.3"\n'
            'source = ' + json.dumps(source) + '\nchecksum = ' + json.dumps(checksum or self.checksum) +
            '\n[[package]]\nname = "local"\nversion = "0.1.0"\n')
        self.pin = hashlib.sha256(self.lock.read_bytes()).hexdigest()

    def verify(self):
        return CRATES.verify(self.lock, self.pin, self.archives, self.sources)

    def test_selected_crate_exact_bytes_modes_and_hidden_files_match(self):
        (self.sources / 'unselected-package').mkdir()
        (self.archives / 'unselected.crate').write_bytes(b'not selected')
        result = self.verify()
        self.assertEqual(result['files'], 2)
        self.assertEqual(result['bytes'], sum(len(row[1]) for row in self.members))
        self.assertEqual(result['local_packages_not_checked'], ['local-0.1.0'])
        self.assertEqual(result['packages'][self.package]['archive_sha256'], self.checksum)
        self.assertFalse((self.source / '.cargo-checksum.json').exists())

    def test_changed_source_and_forged_cache_metadata_cannot_override_archive(self):
        (self.source / 'src/lib.rs').write_bytes(b'pub fn replaced() {}\n')
        with self.assertRaisesRegex(ValueError, 'source differs'):
            self.verify()
        (self.source / '.cargo-checksum.json').write_text(json.dumps({'package': self.checksum,
            'files': {'src/lib.rs': hashlib.sha256(b'pub fn replaced() {}\n').hexdigest()}}))
        with self.assertRaisesRegex(ValueError, 'source differs'):
            self.verify()

    def test_changed_archive_is_not_trusted_via_its_filename(self):
        self.archive.write_bytes(b'changed archive')
        with self.assertRaisesRegex(ValueError, 'locked checksum'):
            self.verify()

    def test_missing_extra_hidden_file_directory_and_mode_are_rejected(self):
        path = self.source / '.gitignore'
        original = path.read_bytes()
        path.unlink()
        with self.assertRaisesRegex(ValueError, 'source differs'):
            self.verify()
        path.write_bytes(original)
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'source differs'):
            self.verify()
        path.chmod(0o644)
        for name, directory in (('.hidden', False), ('extra-dir', True)):
            extra = self.source / name
            extra.mkdir() if directory else extra.write_bytes(b'extra')
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'source differs'):
                self.verify()
            extra.rmdir() if directory else extra.unlink()

    def test_links_special_files_and_privileged_modes_are_rejected(self):
        extra = self.source / 'extra'
        for kind in ('symlink', 'hardlink', 'fifo', 'privileged'):
            if kind == 'symlink':
                extra.symlink_to(self.source / 'src/lib.rs')
            elif kind == 'hardlink':
                os.link(self.source / 'src/lib.rs', extra)
            elif kind == 'fifo':
                os.mkfifo(extra)
            else:
                extra.write_bytes(b'privileged'); extra.chmod(0o4644)
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.verify()
            extra.unlink()

    def test_parent_links_and_relative_paths_are_rejected(self):
        linked = self.root / 'linked'
        linked.symlink_to(self.sources, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'linked'):
            CRATES.verify(self.lock, self.pin, self.archives, linked)
        with self.assertRaisesRegex(ValueError, 'absolute'):
            CRATES.verify(Path('Cargo.lock'), self.pin, self.archives, self.sources)

    def test_malformed_or_duplicate_extraction_marker_is_rejected(self):
        for raw in (b'{"v":1,"v":1}', b'{"v":true}', b'{"v":2}', b'{"v":1,"extra":1}'):
            (self.source / '.cargo-ok').write_bytes(raw)
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                self.verify()

    def test_wrong_lock_pin_unknown_sources_and_invalid_checksum_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'trusted digest'):
            CRATES.verify(self.lock, '0' * 64, self.archives, self.sources)
        for source in ('git+https://example.invalid/repo', 'registry+https://example.invalid'):
            self.write_lock(source=source)
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, 'unsupported'):
                self.verify()
        self.write_lock(checksum='invalid')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.verify()

    def test_duplicate_and_unsafe_locked_identities_are_rejected(self):
        raw = self.lock.read_bytes()
        for changed in (raw + b'[[package]]\nname = "fixture"\nversion = "1.2.3"\n',
                        raw.replace(b'name = "fixture"', b'name = "../fixture"')):
            with self.subTest(raw=changed), self.assertRaises(ValueError):
                CRATES.locked_packages(changed, hashlib.sha256(changed).hexdigest())

    def test_archive_traversal_prefix_reserved_duplicate_and_special_members_block(self):
        for name in ('../escape', 'src/../../escape', '/escape', 'src\\file', '.cargo-ok'):
            self.make_archive([(name, b'x', 0o644, tarfile.REGTYPE)])
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.verify()
        self.make_archive(self.members + [self.members[0]])
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            self.verify()
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE):
            self.make_archive([('special', b'', 0o644, kind)])
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, 'special'):
                self.verify()

    def test_file_directory_conflicts_are_rejected_before_source_comparison(self):
        self.make_archive([('src', b'file', 0o644, tarfile.REGTYPE), *self.members])
        with self.assertRaisesRegex(ValueError, 'conflicting'):
            self.verify()

    def test_archive_source_metadata_and_combined_budgets_block(self):
        for field, value in (('MAX_PACKAGE', 1), ('MAX_FILE', 1), ('MAX_FILES', 1),
                             ('MAX_TOTAL', 1), ('MAX_METADATA', 1), ('MAX_PACKAGES', 1)):
            with self.subTest(field=field), patch.object(CRATES, field, value), self.assertRaises(ValueError):
                self.verify()

    def test_file_mutation_during_read_and_lock_mutation_before_result_block(self):
        original = CRATES.content
        # fdopen gives an integer .name; identify the file via /proc instead.
        def mutating(stream, size):
            result = original(stream, size)
            if isinstance(getattr(stream, 'name', None), int) and os.readlink(
                    '/proc/self/fd/' + str(stream.fileno())).endswith('/src/lib.rs'):
                (self.source / 'src/lib.rs').write_bytes(b'changed while reading')
            return result
        with patch.object(CRATES, 'content', side_effect=mutating), self.assertRaisesRegex(ValueError, 'changed'):
            self.verify()
        (self.source / 'src/lib.rs').write_bytes(self.members[0][1])
        original_inventory = CRATES.source_inventory
        def change_lock(directory):
            value = original_inventory(directory)
            self.lock.write_bytes(self.lock.read_bytes() + b'\n# changed\n')
            return value
        with patch.object(CRATES, 'source_inventory', side_effect=change_lock), self.assertRaisesRegex(ValueError, 'lock changed'):
            self.verify()


if __name__ == '__main__':
    unittest.main()
