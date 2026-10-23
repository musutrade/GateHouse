import hashlib
import io
import json
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
        self.assertIn('--features harness-gate/lifecycle-joint-tests', build)
        self.assertIn('target/quality/fast-archive/lifecycle/', build)
        self.assertLess(shard.index('lifecycle_fixture.py activate'), shard.index('cargo llvm-cov nextest'))
        self.assertIn('--env-file "$GITHUB_ENV"', shard)


if __name__ == '__main__':
    unittest.main()
