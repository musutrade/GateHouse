"""The core risk gate consumes a pinned, verifiable measurer release artifact."""
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ci_quality
import measurer_release as release
from source_measure import provenance

ROOT = Path(__file__).resolve().parents[3]


class MeasurerPinTests(unittest.TestCase):
    @staticmethod
    def cache() -> Path:
        return Path(os.environ.get('HARNESS_GATE_MEASURER_CACHE', ROOT / 'target/rust-measure-release'))

    @staticmethod
    def crafted_archive(pin: dict, members: dict[str, bytes]) -> Path:
        directory = Path(tempfile.mkdtemp(prefix='measurer-pin-'))
        archive = directory / pin['asset']
        with tarfile.open(archive, 'w:gz') as tar:
            for name, payload in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(payload)
                info.mode = 0o755
                tar.addfile(info, io.BytesIO(payload))
        return archive

    def test_released_artifact_identity_matches_the_pin_and_the_actual_binary(self):
        pin = ci_quality.load_measurer_pin()
        binary = ci_quality.ensure_measurer(self.cache())
        released = json.loads((binary.parent / 'plugin.json').read_text())
        for field in ('analyzer', 'version', 'rule', 'executable'):
            self.assertEqual(pin[field], released[field], field)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'identity.rs'
            source.write_text('fn main() {}\n')
            result = subprocess.run([str(binary), str(source), '--target-cfg'],
                                    input=json.dumps(ci_quality.compiler_configuration()),
                                    capture_output=True, text=True, check=True)
        payload = json.loads(result.stdout)
        self.assertEqual((payload['analyzer'], payload['version'], payload['rule']),
                         (pin['analyzer'], pin['version'], pin['rule']))
        self.assertEqual(payload['configuration'], ci_quality.compiler_configuration())

    def test_verify_measurer_asset_accepts_only_the_pinned_digest(self):
        pin = ci_quality.load_measurer_pin()
        with tempfile.TemporaryDirectory() as directory:
            asset = Path(directory) / pin['asset']
            asset.write_bytes(b'pinned measurer bytes')
            with self.assertRaisesRegex(ValueError, 'digest mismatch'):
                ci_quality.verify_measurer_asset(asset, pin)
            pin['sha256'] = ci_quality.sha256(asset)
            ci_quality.verify_measurer_asset(asset, pin)

    def test_extraction_rejects_members_outside_the_release_root(self):
        pin = ci_quality.load_measurer_pin()
        root = pin['asset'].removesuffix('.tar.gz')
        archive = self.crafted_archive(pin, {f'{root}/{pin["executable"]}': b'elf',
                                             '../escape': b'escape'})
        crafted = dict(pin)
        crafted['sha256'] = ci_quality.sha256(archive)
        with self.assertRaisesRegex(ValueError, 'unsafe measurer archive member'):
            release.extract_measurer(archive, archive.parent, crafted)

    def test_extraction_requires_the_executable_inside_the_release_root(self):
        pin = ci_quality.load_measurer_pin()
        root = pin['asset'].removesuffix('.tar.gz')
        archive = self.crafted_archive(pin, {f'{root}/README.md': b'no executable'})
        crafted = dict(pin)
        crafted['sha256'] = ci_quality.sha256(archive)
        with self.assertRaisesRegex(ValueError, 'executable missing'):
            release.extract_measurer(archive, archive.parent, crafted)

    def test_core_risk_stage_no_longer_builds_the_measurer_from_source(self):
        source = (ROOT / 'tools/quality/ci_quality.py').read_text()
        self.assertNotIn('rust-measure/Cargo.toml', source)
        self.assertNotIn("'analyzer-build'", source)
        self.assertIn('ensure_measurer', source)
        self.assertIn("'measurer-download'", Path(release.__file__).read_text())

    def test_pin_rejects_inconsistent_and_unknown_identities(self):
        pin = release.load_measurer_pin()
        cases = {'schema': 'rust-measure-release-pin/v1', 'repository': 'evil/repo',
                 'release': 'latest', 'asset': '../asset.tar.gz', 'source_commit': 'main',
                 'certificate_identity': 'https://github.com/evil/repo', 'sha256': '0',
                 'version': '0.3.2', 'analyzer': 'unknown', 'rule': 'mccabe-rust-4',
                 'executable': '../binary', 'workflow': 'other.yml', 'oidc_issuer': 'evil'}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'pin.json'
            for key, value in cases.items():
                with self.subTest(key=key):
                    path.write_text(json.dumps({**pin, key: value}))
                    with self.assertRaises(ValueError):
                        release.load_measurer_pin(path)

    def test_download_and_provenance_share_repository_and_trust(self):
        pin = release.load_measurer_pin()
        self.assertEqual(release.release_url(pin, pin['asset']),
                         f'https://github.com/{pin["repository"]}/releases/download/{pin["release"]}/{pin["asset"]}')
        commands = []
        release.verify_attestation(Path('archive'), pin, lambda name, argv: commands.append(argv))
        argv = commands[0]
        for flag, value in (('--repo', pin['repository']), ('--cert-identity', pin['certificate_identity']),
                            ('--source-ref', 'refs/tags/' + pin['release']),
                            ('--source-digest', pin['source_commit']), ('--signer-digest', pin['source_commit'])):
            self.assertEqual(argv[argv.index(flag) + 1], value)
        self.assertNotIn('Harness-Gate', Path(provenance.__code__.co_filename).read_text().split('def provenance')[1].split('def char_offset')[0])

    def test_cached_digest_mismatch_fails_without_network_or_fallback(self):
        pin = release.load_measurer_pin()
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            scoped = cache / pin['sha256']
            scoped.mkdir()
            (scoped / pin['asset']).write_bytes(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'digest mismatch'):
                release.ensure_measurer(cache, command=lambda *args: self.fail('unexpected network'))

    def test_signature_failure_stops_before_extraction(self):
        pin = release.load_measurer_pin()
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            scoped = cache / pin['sha256']
            scoped.mkdir()
            (scoped / pin['asset']).write_bytes(b'archive')
            def rejected(name, argv):
                raise subprocess.CalledProcessError(1, argv)
            with patch.object(release, 'verify_measurer_asset'), patch.object(release, 'extract_measurer') as extract:
                with self.assertRaises(subprocess.CalledProcessError):
                    release.ensure_measurer(cache, command=rejected)
                extract.assert_not_called()

    def test_inventory_rejects_wrong_source_tag_missing_or_duplicate_asset(self):
        pin = release.load_measurer_pin()
        data = {'schema_version': 1, 'tag': pin['release'],
                'source': {'repository': 'https://github.com/' + pin['repository'], 'commit': pin['source_commit']},
                'assets': [{'name': pin['asset'], 'kind': 'package', 'sha256': pin['sha256'], 'size_bytes': 7}]}
        for key, value in (('source', {'repository': 'evil', 'commit': pin['source_commit']}),
                           ('tag', 'wrong'), ('assets', []), ('assets', data['assets'] * 2)):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                cache = Path(directory)
                scoped = cache / pin['sha256']
                scoped.mkdir()
                (scoped / pin['asset']).write_bytes(b'archive')
                (scoped / 'release-inventory.json').write_text(json.dumps({**data, key: value}))
                with patch.object(release, 'verify_measurer_asset'), patch.object(release, 'extract_measurer') as extract:
                    with self.assertRaisesRegex(ValueError, 'provenance mismatch|inventory mismatch'):
                        release.ensure_measurer(cache, command=lambda *args: None)
                    extract.assert_not_called()

    def test_archive_rejects_links_duplicates_and_size_budget(self):
        pin = release.load_measurer_pin()
        root = pin['asset'].removesuffix('.tar.gz')
        for case in ('symlink', 'hardlink', 'duplicate', 'oversized'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                archive = Path(directory) / pin['asset']
                with tarfile.open(archive, 'w:gz') as tar:
                    info = tarfile.TarInfo(root + '/binary')
                    if case in ('symlink', 'hardlink'):
                        info.type = tarfile.SYMTYPE if case == 'symlink' else tarfile.LNKTYPE
                        info.linkname = '/tmp/outside'
                        tar.addfile(info)
                    elif case == 'duplicate':
                        tar.addfile(info)
                        tar.addfile(info)
                    else:
                        info.size = 33 * 1024 * 1024
                        tar.addfile(info, io.BytesIO(b'x' * info.size))
                crafted = {**pin, 'sha256': ci_quality.sha256(archive)}
                with self.assertRaisesRegex(ValueError, 'unsupported|duplicate|budget'):
                    release.extract_measurer(archive, archive.parent, crafted)

    def test_manifest_and_binary_identity_reject_unknown_capability(self):
        pin = release.load_measurer_pin()
        root = pin['asset'].removesuffix('.tar.gz')
        manifest = {**release.IDENTITY, 'name': pin['analyzer'], 'consumer': 'Core CI',
                    'schema': 'rust-measure-plugin/v1', 'authority': 'measurement-only',
                    'series_rule': pin['rule'] + '/1', 'target': 'x86_64-unknown-linux-gnu'}
        import hashlib
        for field in ('analyzer', 'version', 'rule', 'authority', 'series_rule', 'target', 'binary_sha256', 'capabilities'):
            with self.subTest(field=field):
                archive = self.crafted_archive(pin, {root + '/' + pin['executable']: b'binary',
                    root + '/plugin.json': json.dumps({**manifest, **({field: 'unknown'} if field != 'binary_sha256' else {})}).encode()})
                crafted = {**pin, 'sha256': ci_quality.sha256(archive),
                           'binary_sha256': '0' * 64 if field == 'binary_sha256' else hashlib.sha256(b'binary').hexdigest()}
                with self.assertRaisesRegex(ValueError, 'mismatch|unknown measurer capability'):
                    release.extract_measurer(archive, archive.parent, crafted)


if __name__ == '__main__':
    unittest.main()
