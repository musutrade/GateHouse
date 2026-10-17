"""The administrator input importer checks code and archives before using them."""
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


BOOT = module('protected_import', ROOT / 'tools/trusted_gate/deploy/import-protected-inputs.py')
OFFICIAL_FIXTURES = module('protected_official_fixtures', ROOT / 'tools/quality/tests/test_trusted_gate_official_tools.py')
CRATE_FIXTURES = module('protected_crate_fixtures', ROOT / 'tools/quality/tests/test_trusted_gate_directory.py')


class ProtectedImportTests(unittest.TestCase):
    def setUp(self):
        previous_umask = os.umask(0o077)
        self.addCleanup(os.umask, previous_umask)
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.cleanup)
        self.tool_fixture = OFFICIAL_FIXTURES.OfficialToolTests()
        self.tool_fixture.setUp()
        self.addCleanup(self.tool_fixture.doCleanups)
        self.crate_fixture = CRATE_FIXTURES.LockedDirectoryTests()
        self.crate_fixture.setUp()
        self.addCleanup(self.crate_fixture.doCleanups)
        self.helpers = self.root / 'helpers'
        for name in BOOT.FILES:
            path = self.helpers / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((ROOT / name).read_bytes())
        (self.helpers / 'tools/trusted_gate/deploy/linux-tools.lock.json').write_bytes(self.tool_fixture.manifest.read_bytes())
        self.manifest = self.root / 'manifest.json'
        self.repin()
        self.source = self.root / 'source'
        (self.source / 'tools/harness-gate').mkdir(parents=True)
        (self.source / 'tools/harness-gate/Cargo.lock').write_bytes(self.crate_fixture.lock.read_bytes())
        self.git('init', '-q', '-b', 'candidate')
        self.git('add', 'tools/harness-gate/Cargo.lock')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'frozen')
        self.head = self.git('rev-parse', 'HEAD').decode().strip()
        self.parent = self.root / 'outputs'
        self.parent.mkdir(mode=0o700)
        self.destination = self.parent / 'imported'
        self.real_owned = BOOT.owned

    def cleanup(self):
        for parent, _, names in os.walk(self.root):
            os.chmod(parent, 0o700)
            for name in names:
                path = Path(parent) / name
                if not path.is_symlink() and path.is_file():
                    path.chmod(0o600)
        self.temporary.cleanup()

    def git(self, *args):
        return subprocess.check_output(['/usr/bin/git', '-C', str(self.source), *args], env=BOOT.ENV,
                                       stderr=subprocess.PIPE)

    def repin(self):
        self.value = {'schema': 'gatehouse-bootstrap-code/v1', 'revision': 'a' * 40,
            'files': {name: hashlib.sha256((self.helpers / name).read_bytes()).hexdigest() for name in BOOT.FILES}}
        self.manifest.write_bytes(BOOT.canonical(self.value))
        self.pin = hashlib.sha256(self.manifest.read_bytes()).hexdigest()

    def fixture_owned(self, path, directory=False, private=False):
        """Replace Root ownership only; real paths, types, modes and reads remain."""
        path = BOOT.path_operand(path)
        if path != self.root and self.root not in path.parents:
            return self.real_owned(path, directory=directory, private=private)
        for parent in (path, *path.parents):
            BOOT.need(not parent.is_symlink(), 'linked fixture bootstrap input')
        info = path.lstat()
        BOOT.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode), 'fixture bootstrap type')
        BOOT.need(directory or info.st_nlink == 1, 'hard-linked bootstrap input')
        BOOT.need(not info.st_mode & 0o022 and (not private or not info.st_mode & 0o077), 'mutable fixture input')
        return path

    @contextlib.contextmanager
    def fixture_domain(self):
        before = os.umask(0o077)
        try:
            with patch.object(BOOT, '__file__', str(self.helpers / BOOT.ENTRY)), \
                 patch.object(BOOT.os, 'geteuid', return_value=0), \
                 patch.object(BOOT, 'owned', side_effect=self.fixture_owned):
                yield
        finally:
            os.umask(before)

    def import_inputs(self):
        with self.fixture_domain():
            return BOOT.import_inputs(self.helpers, self.manifest, self.pin, self.source, self.head, self.head,
                self.tool_fixture.archives, self.crate_fixture.archives, self.destination)

    def test_full_fixture_import_is_archive_derived_readonly_and_binds_frozen_git_lock(self):
        result = self.import_inputs()
        self.assertEqual(result['state'], 'imported')
        self.assertEqual((result['tool_files'], result['dependency_packages'], result['dependency_files']), (9, 1, 3))
        self.assertEqual(result['lock_sha256'], self.crate_fixture.pin)
        self.assertEqual(result['head'], self.head)
        self.assertEqual(json.loads((self.destination / 'binding.json').read_text()), result)
        self.assertEqual(json.loads(self.destination.with_name('imported-import.json').read_text()), result)
        self.assertEqual((self.destination / 'dependencies/sources/fixture-1.2.3/.gitignore').read_bytes(), b'/target\n')
        self.assertFalse(list(self.destination.rglob('install.sh')))
        for parent, directories, files in os.walk(self.destination):
            self.assertEqual(Path(parent).stat().st_mode & 0o777, 0o555)
            for name in files:
                self.assertFalse((Path(parent) / name).stat().st_mode & 0o222)
        with self.fixture_domain(), patch.object(BOOT, 'load', wraps=BOOT.load) as loading:
            with self.assertRaisesRegex(ValueError, 'retain old or partial'):
                self.import_inputs()
            loading.assert_not_called()

    def test_nonroot_entry_blocks_before_loading_any_helper(self):
        with patch.object(BOOT.os, 'geteuid', return_value=1000), \
             patch.object(BOOT, 'load') as loading, self.assertRaisesRegex(ValueError, 'administrator authentication'):
            BOOT.import_inputs(self.helpers, self.manifest, self.pin, self.source, self.head, self.head,
                self.tool_fixture.archives, self.crate_fixture.archives, self.destination)
        loading.assert_not_called()
        self.assertFalse(self.destination.exists())

    def test_actual_ownership_guard_rejects_developer_owned_inputs(self):
        original = Path.lstat
        def developer_stat(path):
            values = list(original(path))
            values[4] = 1000
            return os.stat_result(values)
        with patch.object(Path, 'lstat', developer_stat), self.assertRaisesRegex(ValueError, 'Root-protected'):
            BOOT.owned(self.manifest)

    def test_manifest_pin_and_every_helper_are_checked_before_import(self):
        executable = self.helpers / 'tools/trusted_gate/deploy/prepare-official-tools.py'
        executable.write_text("raise RuntimeError('unreviewed helper must never execute')\n")
        with self.fixture_domain(), patch.object(BOOT, 'load') as loading, self.assertRaisesRegex(ValueError, 'reviewed pin'):
            self.import_inputs()
        loading.assert_not_called()
        self.assertFalse(self.destination.exists())
        self.pin = '0' * 64
        with self.fixture_domain(), self.assertRaisesRegex(ValueError, 'manifest pin'):
            BOOT.authenticate(self.helpers, self.manifest, self.pin)

    def test_malformed_manifest_and_different_entry_location_are_rejected(self):
        original = self.manifest.read_bytes()
        for mutation in (lambda value: value.update(revision='main'),
                         lambda value: value.update(extra=True),
                         lambda value: value['files'].pop(BOOT.ENTRY),
                         lambda value: value['files'].update({'unreviewed.py': 'a' * 64})):
            value = json.loads(original)
            mutation(value)
            self.manifest.write_bytes(BOOT.canonical(value))
            pin = hashlib.sha256(self.manifest.read_bytes()).hexdigest()
            with self.fixture_domain(), self.assertRaisesRegex(ValueError, 'code inventory'):
                BOOT.authenticate(self.helpers, self.manifest, pin)
        self.manifest.write_bytes(b'{"schema":"one","schema":"two"}')
        pin = hashlib.sha256(self.manifest.read_bytes()).hexdigest()
        with self.fixture_domain(), self.assertRaisesRegex(ValueError, 'duplicate'):
            BOOT.authenticate(self.helpers, self.manifest, pin)
        self.manifest.write_bytes(original)
        with self.fixture_domain(), patch.object(BOOT, '__file__', '/unprotected/importer.py'), \
             self.assertRaisesRegex(ValueError, 'protected helper tree'):
            BOOT.authenticate(self.helpers, self.manifest, self.pin)

    def test_extra_missing_linked_hardlinked_and_writable_code_trees_are_rejected(self):
        path = self.helpers / 'tools/maintenance/restore_evidence.py'
        raw = path.read_bytes()
        for attack in ('extra', 'bytecode', 'missing', 'link', 'hardlink', 'writable'):
            if attack == 'extra':
                changed = self.helpers / 'unreviewed.py'; changed.write_bytes(b'no')
            elif attack == 'bytecode':
                changed = self.helpers / 'tools/trusted_gate/deploy/__pycache__'; changed.mkdir()
            elif attack == 'missing':
                path.unlink()
            elif attack == 'link':
                path.unlink(); path.symlink_to(self.manifest)
            elif attack == 'hardlink':
                changed = self.root / 'hardlink'; os.link(path, changed)
            else:
                path.chmod(0o666)
            with self.fixture_domain(), self.subTest(attack=attack), self.assertRaises(ValueError):
                BOOT.authenticate(self.helpers, self.manifest, self.pin)
            if attack in ('extra', 'hardlink'):
                changed.unlink()
            elif attack == 'bytecode':
                changed.rmdir()
            elif attack in ('missing', 'link'):
                if path.is_symlink(): path.unlink()
                path.write_bytes(raw)
            path.chmod(0o644)

    def test_source_head_base_and_working_lock_must_match_git_objects(self):
        with self.fixture_domain():
            with self.assertRaisesRegex(ValueError, 'head differs'):
                BOOT.frozen_lock(self.source, 'b' * 40, self.head)
            with self.assertRaisesRegex(ValueError, 'exact frozen'):
                BOOT.frozen_lock(self.source, self.head, 'main')
            lock = self.source / 'tools/harness-gate/Cargo.lock'
            lock.write_bytes(b'changed working lock')
            with self.assertRaisesRegex(ValueError, 'differs from candidate Git object'):
                BOOT.frozen_lock(self.source, self.head, self.head)
        self.assertFalse(self.destination.exists())

    def test_failed_archive_import_is_retained_blocked_and_cannot_be_reused(self):
        row = self.tool_fixture.value['artifacts']['cargo']
        (self.tool_fixture.archives / row['url'].rsplit('/', 1)[1]).write_bytes(b'corrupted official archive')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.import_inputs()
        state = json.loads(self.destination.with_name('imported-import.json').read_text())
        self.assertEqual(state['state'], 'blocked')
        self.assertTrue((self.destination / 'tools/archives').is_dir())
        self.assertFalse((self.destination / 'binding.json').exists())
        with self.assertRaisesRegex(ValueError, 'retain old or partial'):
            self.import_inputs()

    def test_only_private_snapshots_are_parsed_after_original_archive_changes(self):
        load = BOOT.load
        def changed_loader(root, name):
            helper = load(root, name)
            copy = helper.copy_archive
            if name == 'prepare-official-tools.py':
                def changed(source, destination, row):
                    copy(source, destination, row)
                    source.write_bytes(b'changed after snapshot')
            else:
                def changed(source, destination, pin, limit=None):
                    copy(source, destination, pin, limit)
                    source.write_bytes(b'changed after snapshot')
            helper.copy_archive = changed
            return helper
        with patch.object(BOOT, 'load', side_effect=changed_loader):
            self.assertEqual(self.import_inputs()['state'], 'imported')

    def test_crate_failure_after_tool_import_does_not_complete_the_binding(self):
        self.crate_fixture.archive.write_bytes(b'corrupt dependency archive')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.import_inputs()
        self.assertTrue((self.destination / 'tools/inventory.json').is_file())
        self.assertFalse((self.destination / 'binding.json').exists())
        self.assertEqual(json.loads(self.destination.with_name('imported-import.json').read_text())['state'], 'blocked')

    def test_code_change_during_import_invalidates_the_whole_result(self):
        importing = BOOT.import_dependencies
        def changed(*args):
            value = importing(*args)
            path = self.helpers / 'tools/trusted_gate/deploy/prepare-linux-workspace.py'
            path.write_bytes(path.read_bytes() + b'\n# changed during protected import\n')
            return value
        with patch.object(BOOT, 'import_dependencies', side_effect=changed), self.assertRaisesRegex(ValueError, 'reviewed pin'):
            self.import_inputs()
        self.assertEqual(json.loads(self.destination.with_name('imported-import.json').read_text())['state'], 'blocked')
        self.assertFalse((self.destination / 'binding.json').exists())

    def test_source_change_during_import_never_records_success(self):
        frozen = BOOT.frozen_lock
        count = 0
        def changing(*args):
            nonlocal count
            count += 1
            return frozen(*args) if count == 1 else b'changed source lock'
        with patch.object(BOOT, 'frozen_lock', side_effect=changing), self.assertRaisesRegex(ValueError, 'changed during import'):
            self.import_inputs()
        self.assertEqual(json.loads(self.destination.with_name('imported-import.json').read_text())['state'], 'blocked')
        self.assertFalse((self.destination / 'binding.json').exists())

    def test_output_must_be_fresh_private_disjoint_and_unambiguous(self):
        for path in ('relative', '/a/../b', '/a%h', '/a b', '/a:b'):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, 'canonical absolute'):
                BOOT.path_operand(path)
        self.parent.chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'mutable fixture'):
            self.import_inputs()
        self.parent.chmod(0o700)
        self.destination = self.helpers / 'nested'
        with self.assertRaisesRegex(ValueError, 'mutable fixture|overlaps'):
            self.import_inputs()


if __name__ == '__main__':
    unittest.main()
