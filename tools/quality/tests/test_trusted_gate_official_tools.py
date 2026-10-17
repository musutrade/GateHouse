"""Official archive manifests, not an operator sysroot, define tool payloads."""
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
SPEC = importlib.util.spec_from_file_location('official_tools',
    ROOT / 'tools/trusted_gate/deploy/prepare-official-tools.py')
OFFICIAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OFFICIAL)


@unittest.skipIf(os.geteuid() == 0, 'tool preparation requires an unprivileged operator')
class OfficialToolTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.cleanup)
        self.root = Path(self.temp.name)
        self.archives = self.root / 'archives'; self.archives.mkdir()
        self.destination = self.root / 'prepared'
        self.manifest = self.root / 'inputs.lock.json'
        self.value = json.loads((ROOT / 'tools/trusted_gate/deploy/linux-tools.lock.json').read_text())
        self.payloads = {'rustc': {'bin/rustc': (b'compiler', 0o755), 'lib/library': (b'library', 0o644)},
            'cargo': {'bin/cargo': (b'cargo', 0o755)},
            'rust-std': {'lib/rustlib/target/lib/std.rlib': (b'std', 0o644)},
            'rustfmt-preview': {'bin/rustfmt': (b'fmt', 0o755)},
            'clippy-preview': {'bin/cargo-clippy': (b'clippy', 0o755)},
            'llvm-tools-preview': {'lib/rustlib/target/bin/llvm-cov': (b'llvm', 0o755)}}
        for role in self.value['artifacts']:
            self.make_archive(role)

    def cleanup(self):
        for root, dirs, names in os.walk(self.root):
            os.chmod(root, 0o700)
        self.temp.cleanup()

    def save(self):
        self.manifest.write_text(json.dumps(self.value))
        self.pin = hashlib.sha256(self.manifest.read_bytes()).hexdigest()

    def make_archive(self, role, manifest=None, components=None, extra=()):
        row = self.value['artifacts'][role]
        path = self.archives / row['url'].rsplit('/', 1)[1]
        root, component = OFFICIAL.component_identity(role, row)
        entries = []
        if component:
            prefix = root + '/' + component + '/'
            entries.append((root + '/components', components or (component + '\n').encode(), 0o644, tarfile.REGTYPE, ''))
            files = self.payloads[role]
            data = manifest if manifest is not None else ''.join('file:' + p + '\n' for p in files).encode()
            entries.append((prefix + 'manifest.in', data, 0o644, tarfile.REGTYPE, ''))
            entries += [(prefix + p, data, mode, tarfile.REGTYPE, '') for p, (data, mode) in files.items()]
            entries.append((root + '/install.sh', b'never execute this fixture', 0o755, tarfile.REGTYPE, ''))
        elif role == 'node':
            entries += [(root + '/bin/node', b'node', 0o755, tarfile.REGTYPE, ''),
                        (root + '/bin/npm', b'', 0o777, tarfile.SYMTYPE, '../lib/npm.js'),
                        (root + '/lib/npm.js', b'unselected npm', 0o644, tarfile.REGTYPE, '')]
        else:
            entries.append(('cargo-nextest', b'nextest', 0o755, tarfile.REGTYPE, ''))
        entries.extend(extra)
        with tarfile.open(path, 'w:xz' if path.name.endswith('.xz') else 'w:gz') as bundle:
            for name, data, mode, kind, link in entries:
                member = tarfile.TarInfo(name)
                member.size, member.mode, member.type, member.linkname = len(data), mode, kind, link
                bundle.addfile(member, io.BytesIO(data))
        row['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        self.save()

    def prepare(self):
        return OFFICIAL.prepare(self.manifest, self.pin, self.archives, self.destination)

    def verify(self):
        return OFFICIAL.verify(self.destination, self.pin)

    def test_all_manifest_files_and_exact_binaries_without_install_scripts(self):
        with patch('subprocess.run', side_effect=AssertionError('package code must not execute')):
            value = self.prepare()
        self.assertEqual(value, self.verify())
        self.assertEqual(len(value['files']), 9)
        self.assertEqual((self.destination / 'toolchain/lib/library').read_bytes(), b'library')
        self.assertFalse((self.destination / 'toolchain/bin/npm').exists())
        self.assertFalse(list((self.destination / 'toolchain').rglob('install.sh')))
        self.assertEqual(self.destination.stat().st_mode & 0o777, 0o555)
        self.assertEqual(len(list((self.destination / 'archives').iterdir())), 8)

    def test_wrong_pin_old_output_overlap_and_root_execution_block(self):
        with self.assertRaisesRegex(ValueError, 'reviewed digest'):
            OFFICIAL.prepare(self.manifest, '0' * 64, self.archives, self.destination)
        with self.assertRaisesRegex(ValueError, 'overlaps'):
            OFFICIAL.prepare(self.manifest, self.pin, self.archives, self.archives / 'nested')
        with patch.object(OFFICIAL.os, 'geteuid', return_value=0), self.assertRaisesRegex(ValueError, 'unprivileged'):
            self.prepare()
        self.destination.mkdir()
        with self.assertRaisesRegex(ValueError, 'retain old or partial'):
            self.prepare()

    def test_corruption_is_retained_blocked_and_never_reused(self):
        row = self.value['artifacts']['cargo']
        (self.archives / row['url'].rsplit('/', 1)[1]).write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.prepare()
        state = self.destination.with_name('prepared-preparation.json')
        self.assertEqual(json.loads(state.read_text())['state'], 'blocked')
        with self.assertRaisesRegex(ValueError, 'retain old or partial'):
            self.prepare()

    def test_component_manifest_missing_extra_duplicate_directory_and_traversal_block(self):
        for manifest in (b'file:missing\n', b'file:bin/rustc\n',
                         b'file:bin/rustc\nfile:bin/rustc\n', b'dir:bin\n', b'file:../escape\n'):
            self.make_archive('rustc', manifest=manifest)
            row = self.value['artifacts']['rustc']
            with self.subTest(manifest=manifest), self.assertRaises(ValueError):
                OFFICIAL.archive_inventory(self.archives / row['url'].rsplit('/', 1)[1], 'rustc', row)
        self.make_archive('rustc', components=b'rustc\nother\n')
        row = self.value['artifacts']['rustc']
        with self.assertRaisesRegex(ValueError, 'component selection'):
            OFFICIAL.archive_inventory(self.archives / row['url'].rsplit('/', 1)[1], 'rustc', row)

    def test_archive_paths_specials_duplicates_and_selected_links_block(self):
        role = 'cargo-nextest'; row = self.value['artifacts'][role]
        for name, kind, link in (('../escape', tarfile.REGTYPE, ''),
                                 ('fifo', tarfile.FIFOTYPE, ''),
                                 ('cargo-nextest', tarfile.REGTYPE, ''),
                                 ('linked', tarfile.SYMTYPE, 'cargo-nextest')):
            self.make_archive(role, extra=[(name, b'', 0o644, kind, link)])
            with self.subTest(name=name), self.assertRaises(ValueError):
                OFFICIAL.archive_inventory(self.archives / row['url'].rsplit('/', 1)[1], role, row)
        role='node'; row=self.value['artifacts'][role]; root,_=OFFICIAL.component_identity(role,row)
        self.make_archive(role, extra=[(root + '/bad', b'', 0o777, tarfile.SYMTYPE, '../../escape')])
        with self.assertRaisesRegex(ValueError, 'escaping|unsafe'):
            OFFICIAL.archive_inventory(self.archives / row['url'].rsplit('/', 1)[1], role, row)

    def test_cross_component_collision_blocks_before_overwrite(self):
        self.payloads['clippy-preview']['bin/cargo'] = (b'wrong cargo', 0o755)
        self.make_archive('clippy-preview')
        with self.assertRaisesRegex(ValueError, 'colliding'):
            self.prepare()
        self.assertEqual((self.destination / 'toolchain/bin/cargo').read_bytes(), b'cargo')

    def test_forged_inventory_cannot_hide_changed_bytes_or_executable_modes(self):
        self.prepare()
        path = self.destination / 'toolchain/bin/node'
        path.chmod(0o755); path.write_bytes(b'forged node'); path.chmod(0o555)
        metadata = self.destination / 'inventory.json'; value = json.loads(metadata.read_text())
        value['files']['bin/node'] = [len(b'forged node'), hashlib.sha256(b'forged node').hexdigest(), 0o555]
        metadata.chmod(0o644); metadata.write_bytes(OFFICIAL.canonical(value)); metadata.chmod(0o444)
        with self.assertRaisesRegex(ValueError, 'independently derived'):
            self.verify()

    def test_extra_hidden_file_symlink_and_writable_directory_block(self):
        self.prepare()
        prefix = self.destination / 'toolchain'; prefix.chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'writable'):
            self.verify()
        for kind in ('hidden', 'link'):
            path = prefix / '.unexpected'
            path.write_bytes(b'extra') if kind == 'hidden' else path.symlink_to('bin/node')
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.verify()
            path.unlink()

    def test_all_decoding_file_count_payload_and_inventory_budgets_block(self):
        for name in ('MAX_DECODED', 'MAX_FILE', 'MAX_MEMBERS', 'MAX_TOTAL', 'MAX_METADATA', 'MAX_INVENTORY'):
            with self.subTest(name=name), patch.object(OFFICIAL, name, 1), self.assertRaises(ValueError):
                self.prepare()
            self.destination = self.destination.with_name(self.destination.name + '-next')
        with patch.object(OFFICIAL.TOOLS, 'MAX_TOTAL', 1), self.assertRaises(ValueError):
            self.prepare()

    def test_original_archive_change_after_copy_does_not_replace_snapshot(self):
        original = OFFICIAL.copy_archive
        def changed(source, destination, row):
            original(source, destination, row)
            source.write_bytes(b'changed after snapshot')
        with patch.object(OFFICIAL, 'copy_archive', side_effect=changed):
            value = self.prepare()
        self.assertEqual(value, self.verify())

    def test_source_lock_mutation_and_hardlinked_inputs_block(self):
        original = OFFICIAL.copy_archive
        def changed(source, destination, row):
            original(source, destination, row)
            self.manifest.write_bytes(self.manifest.read_bytes() + b'\n')
        with patch.object(OFFICIAL, 'copy_archive', side_effect=changed), self.assertRaisesRegex(ValueError, 'lock changed'):
            self.prepare()
        self.destination = self.root / 'second'
        self.save()
        path = self.archives / self.value['artifacts']['cargo']['url'].rsplit('/',1)[1]
        os.link(path, self.root / 'hardlink')
        with self.assertRaisesRegex(ValueError, 'linked'):
            self.prepare()


if __name__ == '__main__':
    unittest.main()
