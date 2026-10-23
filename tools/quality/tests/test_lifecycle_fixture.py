import hashlib
import io
import json
import tomllib
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch, Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import lifecycle_fixture as fixture
import ci_quality


class LifecycleFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='.lifecycle-fixture-', dir=Path.home())
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.crate = self.root / 'crate'
        self.crate.mkdir()
        (self.crate / 'Cargo.toml').write_text('[features]\nlifecycle-joint-tests = []\n')
        (self.crate / 'src').mkdir()
        (self.crate / 'src/main.rs').write_text('fn main() {}\n')

    def archive(self, extra=()):
        path = self.root / 'source.tar.gz'
        with tarfile.open(path, 'w:gz') as stream:
            for name, kind in [(fixture.PREFIX + x, tarfile.REGTYPE)
                               for x in ('Cargo.toml', 'Cargo.lock', 'src/main.rs')] + list(extra):
                member = tarfile.TarInfo(name)
                member.type = kind
                member.mode = 0o644
                member.size = 1 if kind == tarfile.REGTYPE else 0
                stream.addfile(member, io.BytesIO(b'x') if member.size else None)
        return path

    def test_registry_protocol_pin_preserves_exact_version_and_rejects_drift(self):
        native = self.root / 'native'
        native.mkdir()
        manifest = native / 'Cargo.toml'
        manifest.write_text('[dependencies]\ngatehouse-runtime-protocol = "=0.3.0"\n')
        lock = native / 'Cargo.lock'
        original = ('[[package]]\nname = "gatehouse-runtime-protocol"\n'
                    'version = "0.3.0"\ndependencies = ["serde"]\n')
        lock.write_text(original)
        fixture.pin_registry_protocol(native)
        self.assertIn('version = "0.3.0"', lock.read_text())
        self.assertIn('checksum = "357cf18eb55a7a83d9b8840280e77c6f7efe263a74bdeafc7e132cad7a607e54"', lock.read_text())
        self.assertIn('dependencies = ["serde"]', lock.read_text())
        with self.assertRaisesRegex(ValueError, 'protocol lock'):
            fixture.pin_registry_protocol(native)
        lock.write_text(original)
        manifest.write_text('[dependencies]\ngatehouse-runtime-protocol = "=0.4.0"\n')
        with self.assertRaisesRegex(ValueError, 'protocol requirement'):
            fixture.pin_registry_protocol(native)
        self.assertEqual(lock.read_text(), original)

    def test_archive_digest_checked_before_any_extraction(self):
        archive = self.archive()
        with self.assertRaisesRegex(ValueError, 'digest/size'):
            fixture.extract(archive, self.root / 'native')
        self.assertFalse((self.root / 'native').exists())

    def test_native_subset_only_and_private_members(self):
        archive = self.archive([('other/readme', tarfile.REGTYPE)])
        with patch.object(fixture, 'ARCHIVE_SHA256', fixture.digest(archive)):
            fixture.extract(archive, self.root / 'native')
        self.assertEqual(sorted(str(p.relative_to(self.root / 'native'))
                                for p in (self.root / 'native').rglob('*') if p.is_file()),
                         ['Cargo.lock', 'Cargo.toml', 'src/main.rs'])
        self.assertEqual((self.root / 'native/Cargo.toml').stat().st_mode & 0o777, 0o600)

    def test_bad_members_rejected_before_extraction(self):
        for name, kind in [('../escape', tarfile.REGTYPE),
                           (fixture.PREFIX + 'link', tarfile.SYMTYPE),
                           (fixture.PREFIX + 'Cargo.toml', tarfile.REGTYPE)]:
            with self.subTest(name=name):
                archive = self.archive([(name, kind)])
                with patch.object(fixture, 'ARCHIVE_SHA256', fixture.digest(archive)):
                    with self.assertRaises(ValueError):
                        fixture.extract(archive, self.root / 'native')
                self.assertFalse((self.root / 'native').exists())

    def bundle(self):
        bundle = self.root / 'bundle'
        bundle.mkdir()
        for name in ('adapter', 'core'):
            (bundle / name).write_bytes(b'\x7fELF' + name.encode())
        receipt = {'schema': 1, 'source_only': True, 'gp_commit': fixture.COMMIT,
                   'archive_sha256': fixture.ARCHIVE_SHA256,
                   'core_source_sha256': fixture.source_digest(self.crate),
                   'binaries': {name: fixture.digest(bundle / name) for name in ('adapter', 'core')}}
        (bundle / 'receipt.json').write_text(json.dumps(receipt))
        return bundle

    def test_exact_pair_environment_and_private_journal(self):
        values = fixture.environment(self.crate, self.bundle(), self.root / 'journal')
        self.assertEqual(len(values), 5)
        self.assertEqual(values['GATEHOUSE_LIFECYCLE_CORE_SHA256'],
                         fixture.digest(self.root / 'bundle/core'))
        self.assertEqual((self.root / 'journal').stat().st_mode & 0o777, 0o700)
        self.assertEqual((self.root / 'bundle/core').stat().st_mode & 0o777, 0o500)

    def test_podman_fixture_tracks_crate_protocol_version(self):
        protocol = self.crate / 'runtime-protocol'
        protocol.mkdir()
        manifest = protocol / 'Cargo.toml'
        for version in ('0.4.0', '0.4.1', '0.5.0', '0.6.0'):
            with self.subTest(version=version):
                manifest.write_text(f'[package]\nname = "protocol"\nversion = "{version}"\n')
                self.assertIs(fixture.podman_fixture(self.crate), fixture.PODMAN_FIXTURES[version])
        self.assertEqual(fixture.PODMAN_FIXTURES['0.6.0']['source'], 'remote-archive')
        self.assertEqual(fixture.PODMAN_FIXTURES['0.6.0']['archive_size'], 1045332)
        self.assertTrue(fixture.PODMAN_FIXTURES['0.6.0']['prefix'].startswith('musutrade-GateProbes-d69c597/'))
        manifest.write_text('[package]\nname = "protocol"\nversion = "0.4.2"\n')
        with self.assertRaisesRegex(ValueError, 'unsupported Podman fixture'):
            fixture.podman_fixture(self.crate)

    def test_podman_protocol_rewrite_is_exact_and_preserves_old_fixture(self):
        native = self.root / 'native'
        native.mkdir()
        (native / 'Cargo.toml').write_text(
            '[dependencies]\ngatehouse-runtime-protocol = "=0.4.1"\n')
        (native / 'Cargo.lock').write_text(
            '[[package]]\nname = "gatehouse-runtime-protocol"\nversion = "0.4.1"\n')
        fixture.pin_podman_protocol(native, '0.4.1', '0.5.0')
        self.assertIn('=0.5.0', (native / 'Cargo.toml').read_text())
        self.assertIn('version = "0.5.0"', (native / 'Cargo.lock').read_text())
        self.assertNotIn('=0.4.1', (native / 'Cargo.toml').read_text())

    def test_podman_feature_receipt_and_all_three_binary_bindings(self):
        (self.crate / 'Cargo.toml').write_text('[features]\nlifecycle-joint-tests = []\npodman-capture-joint-tests = []\n')
        protocol = self.crate / 'runtime-protocol'
        protocol.mkdir()
        (protocol / 'Cargo.toml').write_text('[package]\nname = "protocol"\nversion = "0.4.1"\n')
        self.assertEqual(fixture.features(self.crate), fixture.FEATURE + ',' + fixture.PODMAN_FEATURE)
        bundle = self.bundle()
        receipt = json.loads((bundle / 'receipt.json').read_text())
        podman = fixture.PODMAN_FIXTURES['0.4.1']
        receipt['podman'] = {'commit': podman['commit'], 'archive_sha256': podman['archive_sha256']}
        for name in ('podman', 'fault'):
            (bundle / name).write_bytes(b'\x7fELF' + name.encode())
            receipt['binaries'][name] = fixture.digest(bundle / name)
        (bundle / 'receipt.json').write_text(json.dumps(receipt))
        values = fixture.environment(self.crate, bundle, self.root / 'journal')
        self.assertEqual(values['HG_PODMAN_CORE_BINARY'], values['GATEHOUSE_LIFECYCLE_CORE'])
        self.assertEqual(values['HG_PODMAN_NATIVE_SHA256'], fixture.digest(bundle / 'podman'))
        self.assertEqual(values['HG_PODMAN_FAULT_SHA256'], fixture.digest(bundle / 'fault'))
        receipt['podman']['commit'] = '0' * 40
        (bundle / 'receipt.json').write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, 'Podman fixture receipt'):
            fixture.environment(self.crate, bundle, self.root / 'journal2')
        receipt['podman']['commit'] = podman['commit']
        (bundle / 'receipt.json').write_text(json.dumps(receipt))
        (bundle / 'podman').chmod(0o700)
        (bundle / 'podman').write_bytes(b'wrong')
        with self.assertRaisesRegex(ValueError, 'binary digest'):
            fixture.environment(self.crate, bundle, self.root / 'journal3')

    def test_modified_binary_and_symlink_refused(self):
        bundle = self.bundle()
        (bundle / 'core').write_bytes(b'wrong')
        with self.assertRaisesRegex(ValueError, 'binary digest'):
            fixture.environment(self.crate, bundle, self.root / 'journal')
        (bundle / 'core').unlink()
        (bundle / 'core').symlink_to(bundle / 'adapter')
        with self.assertRaisesRegex(ValueError, 'binary digest'):
            fixture.environment(self.crate, bundle, self.root / 'journal')

    def test_changed_source_and_receipt_identity_refused(self):
        bundle = self.bundle()
        (self.crate / 'src/main.rs').write_text('fn main() { panic!() }')
        with self.assertRaisesRegex(ValueError, 'receipt identity'):
            fixture.environment(self.crate, bundle, self.root / 'journal')
        receipt = json.loads((bundle / 'receipt.json').read_text())
        receipt.update(core_source_sha256=fixture.source_digest(self.crate), gp_commit='0' * 40)
        (bundle / 'receipt.json').write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, 'receipt identity'):
            fixture.environment(self.crate, bundle, self.root / 'journal')

    def test_unsafe_journal_parent_refused(self):
        bundle = self.bundle()
        parent = self.root / 'shared'
        parent.mkdir(mode=0o777)
        parent.chmod(0o777)
        with self.assertRaisesRegex(ValueError, 'unprotected'):
            fixture.environment(self.crate, bundle, parent / 'journal')

    def test_old_base_has_no_joint_feature(self):
        self.assertTrue(fixture.supported(self.crate))
        (self.crate / 'Cargo.toml').write_text('[features]\n')
        self.assertFalse(fixture.supported(self.crate))
        with self.assertRaisesRegex(ValueError, 'no lifecycle joint feature'):
            fixture.build(self.crate, self.root / 'bundle', self.root / 'work')

    def test_risk_head_prepares_pair_and_preserves_environment(self):
        collector = ci_quality.Collector(self.root / 'candidate', 'base', 'head', 'fixture')
        collector.python = Mock()
        with patch.object(fixture, 'environment', return_value={'GATEHOUSE_LIFECYCLE_CORE': 'core'}), \
                patch.dict('os.environ', {'GATEHOUSE_JOURNAL_TEST_ROOT': str(self.root)}):
            args, env = collector._lifecycle_coverage_inputs(
                'head', self.crate, 'x86_64-unknown-linux-gnu', {'RETAIN_ME': 'value'})
        self.assertEqual(args, ['--features', fixture.FEATURE])
        self.assertEqual(env, {'RETAIN_ME': 'value', 'GATEHOUSE_LIFECYCLE_CORE': 'core'})
        self.assertEqual(collector.python.call_args.args[:3],
                         ('head-lifecycle-build', 'lifecycle_fixture.py', 'build'))
        (self.crate / 'Cargo.toml').write_text('[features]\n')
        collector.python.reset_mock()
        self.assertEqual(collector._lifecycle_coverage_inputs(
            'base', self.crate, 'x86_64-unknown-linux-gnu', {'BASE': 'yes'}), ([], {'BASE': 'yes'}))
        collector.python.assert_not_called()

    def test_fast_workflow_carries_pair_feature_and_verified_environment(self):
        workflow = (Path(__file__).resolve().parents[3] / '.github/workflows/ci.yml').read_text()
        build = workflow.split('  quality-fast-build:', 1)[1].split('  quality-fast-shard:', 1)[0]
        shard = workflow.split('  quality-fast-shard:', 1)[1].split('  quality-fast:', 1)[0]
        self.assertIn('lifecycle_fixture.py build', build)
        self.assertIn('--features harness-gate/lifecycle-joint-tests,harness-gate/podman-capture-joint-tests', build)
        self.assertIn('target/quality/fast-archive/lifecycle/', build)
        self.assertLess(shard.index('lifecycle_fixture.py activate'), shard.index('cargo llvm-cov nextest'))
        self.assertIn('--env-file "$GITHUB_ENV"', shard)


if __name__ == '__main__':
    unittest.main()


class LocalPodmanFixtureLockTests(unittest.TestCase):
    def test_local_path_override_is_prepared_once_and_builds_locked(self):
        import os
        temporary = tempfile.TemporaryDirectory(prefix='.fixture-local-', dir=Path.home())
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        root.chmod(0o700)
        protocol = root / 'runtime-protocol'
        (protocol / 'src').mkdir(parents=True)
        (protocol / 'Cargo.toml').write_text(
            '[package]\nname = "gatehouse-runtime-protocol"\nversion = "0.6.0"\nedition = "2021"\n')
        (protocol / 'src/lib.rs').write_text('pub fn fixture() {}\n')
        native = root / 'native'
        (native / 'src').mkdir(parents=True)
        (native / 'Cargo.toml').write_text(
            '[package]\nname = "gateprobes-podman-lifecycle"\nversion = "0.3.0"\nedition = "2021"\n'
            '[dependencies]\ngatehouse-runtime-protocol = "=0.6.0"\n')
        (native / 'src/main.rs').write_text('fn main() {}\n')
        (native / 'Cargo.lock').write_text(
            'version = 4\n\n[[package]]\nname = "gateprobes-podman-lifecycle"\nversion = "0.3.0"\n'
            'dependencies = ["gatehouse-runtime-protocol"]\n\n'
            '[[package]]\nname = "gatehouse-runtime-protocol"\nversion = "0.5.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
            'checksum = "f1619c05a4069ca03b477f456ee08f831f873022080c8ed96c850dbe20f6f90b"\n')
        env = os.environ.copy()
        for key in ('RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET'):
            env.pop(key, None)
        checksum = fixture.pin_podman_local_protocol(native, root, env)
        manifest = (native / 'Cargo.toml').read_text()
        self.assertIn('path = ' + json.dumps(str(protocol)), manifest)
        lock = (native / 'Cargo.lock').read_text()
        self.assertIn('version = "0.6.0"', lock)
        record = tomllib.loads(lock)
        entries = [row for row in record["package"] if row["name"] == "gatehouse-runtime-protocol"]
        self.assertEqual(len(entries), 1)
        self.assertNotIn("source", entries[0])
        self.assertNotIn("checksum", entries[0])
        self.assertEqual(checksum, fixture.digest(native / 'Cargo.lock'))
