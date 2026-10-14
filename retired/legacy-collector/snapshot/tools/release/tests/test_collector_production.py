"""Real RSA and bootstrap authentication; Sigstore subprocess is a test double.

These tests prove fail-closed orchestration, not production Sigstore certification.
"""
import copy
import io
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collector_assets as assets
import collector_sigstore as sigstore
import prepare_collector_candidate as candidate
import production_collector_release as production
import sign_collector_candidate as private_signing
import collector_release_policy as policy
import test_collector_delivery as fixtures


class ProductionSignatureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixtures.DeliveryTests.setUpClass()

    @classmethod
    def tearDownClass(cls):
        fixtures.DeliveryTests.tearDownClass()

    def setUp(self):
        self.fixture = fixtures.DeliveryTests('runTest')
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.release = self.fixture.release
        self.trust = copy.deepcopy(self.fixture.trust)
        self.trust['schema'] = 'rust-collector-host-trust/v2'
        for name in ('cosign', 'trusted_root'):
            path = self.fixture.base / name
            path.write_bytes(b'test double, never production trust')
            self.trust[name] = str(path)
            self.trust[name + '_sha256'] = assets.sha(path)
        self.envelope = sigstore.envelope((self.release / 'release-inventory.sig').read_bytes(),
                                         {'test_double': True})
        assets.write(self.release / 'release-inventory.sig', self.envelope)

    def run_verifier(self, result=0):
        real_run = subprocess.run
        calls = []
        def run(args, **kwargs):
            if args[0] != self.trust['cosign']:
                return real_run(args, **kwargs)
            calls.append((args, kwargs))
            return subprocess.CompletedProcess(args, result)
        with patch.object(sigstore.subprocess, 'run', side_effect=run):
            assets.verify(self.release, self.trust, self.fixture.tag)
        return calls

    def test_exact_identity_offline_trust_and_no_ambient_overrides(self):
        calls = self.run_verifier()
        self.assertEqual(len(calls), 1)
        args, kwargs = calls[0]
        self.assertEqual(args[args.index('--certificate-identity') + 1], sigstore.IDENTITY)
        self.assertEqual(args[args.index('--certificate-oidc-issuer') + 1], sigstore.ISSUER)
        self.assertEqual(args[args.index('--trusted-root') + 1], self.trust['trusted_root'])
        self.assertIn('--offline', args)
        self.assertEqual(set(kwargs['env']), {'PATH', 'HOME', 'LANG'})

    def test_rejects_wrong_identity_unattested_or_bad_inclusion_verifier_result(self):
        # Real cosign interprets certificate and inclusion proofs. Any rejection
        # must propagate even though this inventory has a valid real RSA signature.
        with self.assertRaisesRegex(ValueError, 'Sigstore identity'):
            self.run_verifier(1)

    def test_unsigned_missing_bundle_tamper_and_changed_verifier(self):
        for field, value in (('sigstore_bundle', {}), ('schema', 'rust-collector-signatures/v1'),
                             ('rsa_signature', 'AAAA')):
            with self.subTest(field=field):
                assets.write(self.release / 'release-inventory.sig', self.envelope | {field: value})
                with self.assertRaises(ValueError):
                    self.run_verifier()
        assets.write(self.release / 'release-inventory.sig', self.envelope)
        Path(self.trust['cosign']).write_text('changed verifier')
        with self.assertRaisesRegex(ValueError, 'host trust pin changed'):
            self.run_verifier()

    def test_missing_extra_and_tampered_assets(self):
        for name in assets.ASSETS + assets.CONTROL:
            path = self.release / name
            original = path.read_bytes()
            path.unlink()
            with self.subTest(missing=name), self.assertRaisesRegex(ValueError, 'missing/extra'):
                self.run_verifier()
            path.write_bytes(original)
        extra = self.release / 'extra'
        extra.write_bytes(b'extra')
        with self.assertRaisesRegex(ValueError, 'missing/extra'):
            self.run_verifier()
        extra.unlink()
        with (self.release / 'collector.tar').open('ab') as stream:
            stream.write(b'tampered')
        with self.assertRaisesRegex(ValueError, 'inventory mismatch'):
            self.run_verifier()

    def test_unsigned_preparation_does_not_create_provenance_or_eligibility(self):
        for name in ('sbom.spdx.json', 'provenance.json') + assets.CONTROL:
            (self.release / name).unlink()
        receipt = candidate.prepare(self.release)
        self.assertEqual(receipt['status'], 'unsigned-ineligible')
        self.assertEqual(set(p.name for p in self.release.iterdir()), set(assets.ASSETS[:3]))
        self.assertNotIn('eligibility', receipt)
        self.assertEqual(len(receipt['assets']), 3)

    def test_other_valid_signed_candidate_cannot_replace_approved_bytes(self):
        # Signature acceptance alone does not bind an independent approval.
        packet = {'unsigned_assets': {name: {'sha256': assets.sha(self.release / name),
            'size': (self.release / name).stat().st_size} for name in assets.ASSETS[:3]}}
        production.approved_unsigned(packet, self.release)
        packet['unsigned_assets']['manifest.json']['sha256'] = '0' * 64
        self.run_verifier()
        with self.assertRaisesRegex(ValueError, 'approved input digest changed'):
            production.approved_unsigned(packet, self.release)

    def test_download_requires_exact_original_bytes_including_signature(self):
        import shutil
        downloaded = self.fixture.base / 'independent-download'
        shutil.copytree(self.release, downloaded)
        production.exact_download(downloaded, self.release)
        # Harmless JSON whitespace still changes the immutable published bytes.
        with (downloaded / 'release-inventory.sig').open('ab') as stream:
            stream.write(b'\n')
        with self.assertRaisesRegex(ValueError, 'approved input digest changed'):
            production.exact_download(downloaded, self.release)

    def test_unsigned_preparation_rejects_payload_substitution(self):
        self.fixture.payload['bin/harness-gate-rust-collector'] = b'substituted'
        self.fixture.make_archive()
        for name in ('sbom.spdx.json', 'provenance.json') + assets.CONTROL:
            (self.release / name).unlink()
        with self.assertRaisesRegex(ValueError, 'payload checksum mismatch'):
            candidate.prepare(self.release)
        self.assertFalse((self.release / 'sbom.spdx.json').exists())


class ApprovalTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'test-only-receipt'
        self.path.write_text('synthetic test receipt; no release eligibility')
        self.manifest = fixtures.fixture()[0]
        self.digest = assets.contract.fingerprint(self.manifest)
        self.licenses = {'status': 'approved', 'reviewer': 'test-only',
            'manifest_sha256': self.digest, 'payloads': [p | {'license': 'MIT',
            'redistribution': 'approved'} for p in self.manifest['payloads']]}
        self.validation = {'source_commit': self.manifest['source_commit'],
            'manifest_sha256': self.digest, 'checks': {name: {'status': 'pass',
                'receipt_url': 'https://example.invalid/test-only', 'path': str(self.path),
                'sha256': assets.sha(self.path)} for name in ('fresh_host_install',
                'native_capture_reexport', 'generic_core', 'lifecycle_negatives',
                'bootstrap_negatives', 'real_sigstore_positive_and_negatives')}}

    def check(self, licenses=None, validation=None):
        production.reviewed_checks(self.manifest, self.digest,
            self.licenses if licenses is None else licenses,
            self.validation if validation is None else validation)

    def test_every_license_payload_and_redistribution_decision_required(self):
        self.check()
        for mutate in (lambda x: x['payloads'].pop(),
                       lambda x: x['payloads'].append(x['payloads'][0]),
                       lambda x: x['payloads'][0].update(license='NOASSERTION'),
                       lambda x: x['payloads'][0].update(redistribution='pending'),
                       lambda x: x.update(manifest_sha256='0' * 64)):
            review = copy.deepcopy(self.licenses)
            mutate(review)
            with self.assertRaises(ValueError):
                self.check(licenses=review)

    def test_every_validation_must_be_successful_current_and_hash_pinned(self):
        for name in self.validation['checks']:
            value = copy.deepcopy(self.validation)
            value['checks'][name]['status'] = 'pending'
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.check(validation=value)
        with self.assertRaisesRegex(ValueError, 'stale validation'):
            self.check(validation=self.validation | {'source_commit': '0' * 40})
        self.path.write_text('tampered receipt')
        with self.assertRaisesRegex(ValueError, 'digest changed'):
            self.check()
        self.path.unlink()
        with self.assertRaises((ValueError, FileNotFoundError)):
            self.check()


    def test_publication_reuses_only_the_exact_private_acceptance_tag(self):
        import json
        source = self.manifest['source_commit']
        tag = 'rust-collector-v0.1.0-rc.1'
        row = {'ref': 'refs/tags/' + tag, 'object': {'type': 'commit', 'sha': source}}
        with patch.object(production.subprocess, 'check_output', return_value=json.dumps([row])), \
             patch.object(production.subprocess, 'run') as mutate:
            production.create_or_verify_tag(tag, source)
            mutate.assert_not_called()
            with self.assertRaisesRegex(ValueError, 'tag differs'):
                production.create_or_verify_tag(tag, '0' * 40)
            mutate.assert_not_called()
        with patch.object(production.subprocess, 'check_output', return_value='[]'), \
             patch.object(production.subprocess, 'run') as mutate:
            production.create_or_verify_tag(tag, source)
            mutate.assert_called_once()
            self.assertNotIn('PATCH', mutate.call_args.args[0])

    def test_wrong_source_and_unsuccessful_main_ci_stop_preflight(self):
        packet = self.path.parent / 'packet.json'
        source = self.manifest['source_commit']
        assets.write(packet, {'schema': 'rust-collector-publication-approval/v1',
                             'source_commit': source, 'version': '0.1.0-rc.1'})
        client = Mock(repository=assets.REPOSITORY)
        with patch.object(production.policy.core, '_resolve_commit', return_value='0' * 40):
            with self.assertRaisesRegex(ValueError, 'exact checked-out protected main'):
                production.preflight(packet, assets.sha(packet), source, client)
        client.get_json.assert_not_called()
        with patch.object(production.policy.core, '_resolve_commit', return_value=source), \
             patch.object(production.policy.core, 'verify_ci_run', side_effect=ValueError('CI pending')):
            with self.assertRaisesRegex(ValueError, 'CI pending'):
                production.preflight(packet, assets.sha(packet), source, client)
        client.get_json.assert_not_called()


class PrivateCandidateTests(unittest.TestCase):
    """Candidate signing cannot substitute publication approval or accept changed bytes."""
    @classmethod
    def setUpClass(cls):
        fixtures.DeliveryTests.setUpClass()

    @classmethod
    def tearDownClass(cls):
        fixtures.DeliveryTests.tearDownClass()

    def setUp(self):
        helper = ProductionSignatureTests()
        helper.setUp()
        self.addCleanup(helper.doCleanups)
        self.fixture, self.release, self.trust = helper.fixture, helper.release, helper.trust
        for name in ('provenance.json',) + assets.CONTROL:
            (self.release / name).unlink()
        self.trust_path = self.fixture.base / 'host-trust.json'
        assets.write(self.trust_path, self.trust)
        self.packet_path = self.fixture.base / 'signing-packet.json'
        self.packet = {'schema': 'rust-collector-private-signing/v1',
            'source_commit': self.fixture.manifest['source_commit'],
            'version': self.fixture.manifest['collector']['version'],
            'directory': str(self.release), 'output': str(self.fixture.base / 'signed'),
            'trust': {'path': str(self.trust_path), 'sha256': assets.sha(self.trust_path)},
            'unsigned_assets': {name: {'sha256': assets.sha(self.release / name),
                'size': (self.release / name).stat().st_size} for name in assets.ASSETS[:3]}}
        self.client = Mock(repository=assets.REPOSITORY)

    def check_candidate(self):
        assets.write(self.packet_path, self.packet)
        with patch.object(policy.core, '_resolve_commit', return_value=self.packet['source_commit']), \
             patch.object(policy.core, 'verify_ci_run') as ci, \
             patch.object(policy, 'protected_environment') as environment:
            result = private_signing.preflight(self.packet_path, assets.sha(self.packet_path),
                                               self.packet['source_commit'], self.client)
            ci.assert_called_once()
            environment.assert_called_once()
            return result

    def test_private_candidate_needs_no_future_install_receipt_and_creates_no_release(self):
        self.check_candidate()
        self.assertFalse(Path(self.packet['output']).exists())
        self.assertEqual(set(p.name for p in self.release.iterdir()), set(assets.ASSETS[:3]))
        source = (Path(private_signing.__file__)).read_text()
        self.assertNotIn('gh release', source)
        self.assertNotIn("'published': True", source)

    def test_changed_packet_assets_trust_and_existing_output_reject(self):
        self.check_candidate()
        for field in ('manifest.json', 'collector.tar', 'sbom.spdx.json'):
            path = self.release / field
            original = path.read_bytes()
            path.write_bytes(original + b'\n')
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.check_candidate()
            path.write_bytes(original)
        original = self.trust_path.read_bytes()
        self.trust_path.write_bytes(original + b'\n')
        with self.assertRaises(ValueError):
            self.check_candidate()
        self.trust_path.write_bytes(original)
        Path(self.packet['output']).mkdir()
        with self.assertRaisesRegex(ValueError, 'must be fresh'):
            self.check_candidate()

    def test_wrong_packet_hash_source_stable_version_and_bad_ci_reject(self):
        self.check_candidate()
        with self.assertRaisesRegex(ValueError, 'digest changed'):
            private_signing.preflight(self.packet_path, '0' * 64, self.packet['source_commit'], self.client)
        with self.assertRaisesRegex(ValueError, 'wrong signing source'):
            private_signing.preflight(self.packet_path, assets.sha(self.packet_path), '0' * 40, self.client)
        self.packet['version'] = '0.1.0'
        with self.assertRaisesRegex(ValueError, 'RC version'):
            self.check_candidate()
        self.packet['version'] = self.fixture.manifest['collector']['version']
        assets.write(self.packet_path, self.packet)
        with patch.object(policy.core, '_resolve_commit', return_value=self.packet['source_commit']), \
             patch.object(policy.core, 'verify_ci_run', side_effect=ValueError('CI pending')):
            with self.assertRaisesRegex(ValueError, 'CI pending'):
                private_signing.preflight(self.packet_path, assets.sha(self.packet_path),
                                          self.packet['source_commit'], self.client)


class BootstrapAuthenticationTests(unittest.TestCase):
    def test_authenticate_before_any_archive_execution(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            marker = root / 'executed'
            archive = root / 'bootstrap.tar'
            # Shell sentinel models executable bootstrap bytes; this is not a
            # fresh-host/private-Python acceptance scenario.
            raw = ('#!/bin/sh\nprintf executed > "' + str(marker) + '"\n').encode()
            with tarfile.open(archive, 'w') as output:
                member = tarfile.TarInfo('python/bin/python3')
                member.size, member.mode = len(raw), 0o755
                output.addfile(member, io.BytesIO(raw))
            shell = Path(__file__).resolve().parents[1] / 'bootstrap_collector.sh'
            command = ['sh', str(shell)]
            for digest, path in (('0' * 64, archive), (assets.sha(archive), root / 'missing'),
                                 ('', archive)):
                result = subprocess.run(command + [digest, str(path), '--help'], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(marker.exists())
            result = subprocess.run(command + [assets.sha(archive), str(archive), '--help'], capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(marker.read_text(), 'executed')


if __name__ == '__main__':
    unittest.main()
