"""Real local signatures and filesystem lifecycle; synthetic payloads, no native claim."""
from __future__ import annotations

import copy
import io
import json
import multiprocessing
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

RELEASE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RELEASE))
sys.path.insert(0, str(RELEASE.parents[0] / 'quality/tests'))
import collector_assets as assets
import collector_release_policy as policy
import install_collector as installer
from test_rust_collector_contract import fixture


def receipt(manifest):
    return {'schema': 'rust-collector-eligibility/v1', 'status': 'pass',
            'repository': assets.REPOSITORY, 'tag': 'rust-collector-v' + manifest['collector']['version'],
            'commit': manifest['source_commit'], 'protected_main': 'refs/remotes/origin/main',
            'ci': {'workflow': '.github/workflows/ci.yml', 'run_id': 1,
                   'aggregate_job': 'Required Quality Aggregate', 'aggregate_job_id': 1},
            'environment': {'name': policy.ENVIRONMENT, 'required_reviewers': True,
                            'prevent_self_review': True, 'can_admins_bypass': False}}


class DeliveryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.keys = tempfile.TemporaryDirectory()
        cls.keydir = Path(cls.keys.name)
        cls.openssl = str(Path(shutil.which('openssl')).resolve())
        subprocess.run([cls.openssl, 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048',
                        '-out', str(cls.keydir / 'private.pem')], check=True, capture_output=True)
        subprocess.run([cls.openssl, 'pkey', '-in', str(cls.keydir / 'private.pem'), '-pubout',
                        '-out', str(cls.keydir / 'public.pem')], check=True, capture_output=True)
        cls.trust = {'schema': 'rust-collector-host-trust/v1', 'openssl': cls.openssl,
                     'rsa_signature_bytes': 256,
                     'openssl_sha256': assets.sha(Path(cls.openssl)),
                     'public_key': str(cls.keydir / 'public.pem'),
                     'public_key_sha256': assets.sha(cls.keydir / 'public.pem'),
                     'host_libraries': {'test-only-openssl': cls.openssl}}

    @classmethod
    def tearDownClass(cls):
        cls.keys.cleanup()

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / 'installed'
        self.release = self.base / 'release'
        self.release.mkdir()
        self.manifest = fixture()[0]
        self.manifest['collector']['version'] = '0.1.0-rc.1'
        self.manifest['host_abi'] = assets.probe_host(self.trust)
        self.payload = {t['path']: b'synthetic tool\n' for t in self.manifest['tools']}
        self.payload['bin/harness-gate-rust-collector'] = b'#!/bin/sh\nexit 99\n'
        for tool in self.manifest['tools']:
            tool['sha256'] = assets.hashlib.sha256(self.payload[tool['path']]).hexdigest()
        self.manifest['payloads'] = [{'path': name, 'sha256': assets.hashlib.sha256(data).hexdigest(),
                                      'role': 'tool'} for name, data in self.payload.items()]
        self.tag = 'rust-collector-v0.1.0-rc.1'
        self.make_archive()
        self.sign()

    def make_archive(self, additions=(), omit=()):
        with tarfile.open(self.release / 'collector.tar', 'w') as archive:
            for name, data in self.payload.items():
                if name in omit:
                    continue
                member = tarfile.TarInfo(name)
                member.size, member.mode = len(data), 0o755
                archive.addfile(member, io.BytesIO(data))
            for member, data in additions:
                archive.addfile(member, io.BytesIO(data))

    def sign(self):
        assets.write(self.release / 'manifest.json', self.manifest)
        assets.prepare(self.release, receipt(self.manifest))
        self.resign_inventory()

    def resign_inventory(self):
        subprocess.run([self.openssl, 'dgst', '-sha256', '-sign', str(self.keydir / 'private.pem'),
                        '-out', str(self.release / assets.CONTROL[1]), str(self.release / assets.CONTROL[0])],
                       check=True, capture_output=True)

    def install(self, checkpoint=lambda _: None):
        return installer.install(self.root, self.release, self.trust, self.tag, checkpoint)

    def test_signed_install_rollback_uninstall_preserves_evidence(self):
        evidence = self.base / 'project-capture.profraw'
        evidence.write_bytes(b'original trusted project bytes')
        first = self.install()
        self.assertEqual(first.read_bytes(), self.payload['bin/harness-gate-rust-collector'])
        self.manifest['collector']['version'] = '0.1.0-rc.2'
        self.tag = 'rust-collector-v0.1.0-rc.2'
        self.sign()
        second = self.install()
        self.assertNotEqual(first, second)
        self.assertEqual(installer.select(self.root, '0.1.0-rc.1', self.trust), first)
        installer.uninstall(self.root, '0.1.0-rc.2', self.trust)
        self.assertTrue(first.exists())
        self.assertFalse(second.exists())
        installer.uninstall(self.root, '0.1.0-rc.1', self.trust)
        self.assertFalse((self.root / 'current.json').exists())
        self.assertEqual(evidence.read_bytes(), b'original trusted project bytes')

    def test_missing_extra_unsigned_tampered_assets(self):
        for name in assets.ASSETS + assets.CONTROL:
            path = self.release / name
            raw = path.read_bytes()
            with self.subTest(missing=name), self.assertRaises((ValueError, OSError)):
                path.unlink()
                self.install()
            path.write_bytes(raw)
            with self.subTest(tampered=name), self.assertRaises((ValueError, OSError)):
                path.write_bytes(raw + b'bad')
                self.install()
            path.write_bytes(raw)
        (self.release / 'extra').write_text('not declared')
        with self.assertRaisesRegex(ValueError, 'extra'):
            self.install()
        self.assertFalse((self.root / 'current.json').exists())

    def test_exact_tag_abi_and_host_trust_rejected(self):
        for tag in ('v0.1.0-rc.1', 'rust-collector-v0.1.0', '../escape'):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                installer.install(self.root, self.release, self.trust, tag)
        for field in ('kernel', 'glibc', 'runtime_dependencies_sha256'):
            original = self.manifest['host_abi'][field]
            self.manifest['host_abi'][field] = 'a' * 64
            self.sign()
            with self.subTest(abi=field), self.assertRaisesRegex(ValueError, 'ABI'):
                self.install()
            self.manifest['host_abi'][field] = original
        self.sign()
        bad = dict(self.trust, public_key_sha256='0' * 64)
        with self.assertRaisesRegex(ValueError, 'trust pin'):
            installer.install(self.root, self.release, bad, self.tag)

    def test_traversal_links_duplicates_extra_and_missing_archive_payload(self):
        cases = []
        for name in ('../escape', '/absolute', 'bin/../../escape', 'extra', 'bin/driver'):
            member = tarfile.TarInfo(name)
            member.mode = 0o644
            cases.append(member)
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE):
            member = tarfile.TarInfo('unsafe')
            member.type, member.linkname = kind, '../../escape'
            cases.append(member)
        for member in cases:
            self.make_archive([(member, b'')])
            self.sign()
            with self.subTest(name=member.name, kind=member.type), self.assertRaises(ValueError):
                self.install()
        self.make_archive(omit=['bin/python'])
        self.sign()
        with self.assertRaisesRegex(ValueError, 'missing archive'):
            self.install()
        self.assertFalse((self.base / 'escape').exists())

    def test_interruption_recovery_at_each_activation_boundary(self):
        self.install()
        for index, point in enumerate(('verified', 'extracted', 'committed', 'activated'), 2):
            selected = '0.1.0-rc.' + str(index)
            self.manifest['collector']['version'] = selected
            self.tag = 'rust-collector-v' + selected
            self.sign()
            prior = (self.root / 'current.json').read_bytes()

            def crash(phase):
                if phase == point:
                    raise InterruptedError(phase)

            with self.subTest(point=point), self.assertRaises(InterruptedError):
                self.install(crash)
            if point != 'activated':
                self.assertEqual((self.root / 'current.json').read_bytes(), prior)
            with installer.locked(self.root):
                pass
            self.assertEqual(list((self.root / 'staging').iterdir()), [])
            if point in ('committed', 'activated'):
                installer.select(self.root, selected, self.trust)
            else:
                self.install()

    def test_existing_version_and_corrupt_rollback_fail_closed(self):
        path = self.install()
        with self.assertRaisesRegex(ValueError, 'already installed'):
            self.install()
        path.write_text('tampered')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            installer.select(self.root, '0.1.0-rc.1', self.trust)

    def test_uninstall_refuses_unowned_data_and_symlinks(self):
        executable = self.install()
        root = executable.parents[1]
        extra = root / 'project.profraw'
        extra.write_text('preserve')
        with self.assertRaisesRegex(ValueError, 'extra'):
            installer.uninstall(self.root, '0.1.0-rc.1', self.trust)
        self.assertEqual(extra.read_text(), 'preserve')
        extra.unlink()
        executable.unlink()
        executable.symlink_to(self.base / 'unrelated')
        with self.assertRaisesRegex(ValueError, 'link'):
            installer.uninstall(self.root, '0.1.0-rc.1', self.trust)

    def test_signed_but_inconsistent_provenance_sbom_and_inventory(self):
        for name in ('sbom.spdx.json', 'provenance.json'):
            self.sign()
            document = assets.read(self.release / name)
            if name == 'sbom.spdx.json':
                document['files'].pop()
            else:
                document['predicate']['workflow'] = '.github/workflows/evil.yml'
            assets.write(self.release / name, document)
            inv = assets.read(self.release / assets.CONTROL[0])
            inv['assets'] = assets.subjects(self.release, assets.ASSETS)
            assets.write(self.release / assets.CONTROL[0], inv)
            self.resign_inventory()
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.install()

    def test_uninstall_recovers_partial_deletion(self):
        executable = self.install()
        original = Path.unlink
        removed = []

        def interrupted(path, *args, **kwargs):
            result = original(path, *args, **kwargs)
            if path.name == 'driver':
                removed.append(path)
                raise InterruptedError('interrupted uninstall')
            return result

        with patch.object(Path, 'unlink', interrupted), self.assertRaises(InterruptedError):
            installer.uninstall(self.root, '0.1.0-rc.1', self.trust)
        self.assertTrue(removed)
        with installer.locked(self.root):
            pass
        self.assertFalse(executable.parents[1].exists())

    def test_sigkill_preserves_active_and_recovery_is_idempotent(self):
        self.install()
        self.manifest['collector']['version'] = '0.1.0-rc.2'
        self.tag = 'rust-collector-v0.1.0-rc.2'
        self.sign()
        before = (self.root / 'current.json').read_bytes()

        def child():
            self.install(lambda phase: os.kill(os.getpid(), signal.SIGKILL) if phase == 'extracted' else None)

        process = multiprocessing.get_context('fork').Process(target=child)
        process.start()
        process.join(15)
        self.assertEqual(process.exitcode, -signal.SIGKILL)
        self.assertEqual((self.root / 'current.json').read_bytes(), before)
        for _ in range(2):
            with installer.locked(self.root):
                pass
        self.install()

    def test_unsigned_payload_digest_and_unsafe_permissions(self):
        # A valid distribution signature cannot excuse an inconsistent manifest.
        self.payload['bin/python'] = b'tampered runtime'
        self.make_archive()
        self.sign()
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.install()
        self.payload['bin/python'] = b'synthetic tool\n'
        self.make_archive()
        self.sign()
        executable = self.install()
        executable.chmod(0o4755)
        with self.assertRaisesRegex(ValueError, 'mode'):
            installer.select(self.root, '0.1.0-rc.1', self.trust)

    def test_installer_root_and_asset_links_are_rejected(self):
        other = self.base / 'other'
        other.mkdir()
        self.root.symlink_to(other, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'root link'):
            self.install()
        self.root.unlink()
        archive = self.release / 'collector.tar'
        moved = other / 'collector.tar'
        archive.rename(moved)
        archive.symlink_to(moved)
        with self.assertRaisesRegex(ValueError, 'unsafe file'):
            self.install()

    def test_signed_trailing_archive_payload_is_rejected(self):
        with (self.release / 'collector.tar').open('ab') as stream:
            stream.write(b'undeclared payload after tar terminator')
        self.sign()
        with self.assertRaisesRegex(ValueError, 'after archive terminator'):
            self.install()

    def test_recovery_preserves_unowned_staging_evidence(self):
        def stop(phase):
            if phase == 'extracted':
                raise InterruptedError()
        with self.assertRaises(InterruptedError):
            self.install(stop)
        stage = next((self.root / 'staging').iterdir())
        extra = stage / 'capture.profraw'
        extra.write_text('preserve')
        with self.assertRaisesRegex(ValueError, 'extra'):
            with installer.locked(self.root):
                pass
        self.assertEqual(extra.read_text(), 'preserve')


class EligibilityTests(unittest.TestCase):
    def test_personal_approval_pins_owner_and_versions_the_receipt(self):
        environment = {'name': policy.ENVIRONMENT, 'can_admins_bypass': False,
                       'protection_rules': [{'type': 'required_reviewers',
                           'prevent_self_review': False,
                           'reviewers': [{'type': 'User', 'reviewer': dict(policy.PERSONAL_REVIEWER)}]}]}
        manifest = fixture()[0]
        manifest['collector']['version'] = '0.1.0-rc.1'
        from unittest.mock import Mock
        client = Mock(repository=assets.REPOSITORY)
        client.get_json.return_value = environment
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'manifest.json'
            assets.write(path, manifest)
            original = receipt(manifest)
            with patch.object(policy.core, 'verify_git_state', return_value=manifest['source_commit']), \
                    patch.object(policy.core, 'verify_ci_run', return_value=original['ci']) as ci_check:
                actual = policy.verify(Path(temporary), path, original['tag'], original['commit'], client)
            self.assertEqual(actual['schema'], 'rust-collector-eligibility/v2')
            self.assertEqual(actual['environment']['reviewer']['login'], 'higoalespn')
            ci_check.assert_called_once_with(client, original['commit'], 'main')
            policy.validate_receipt(actual, original['tag'], original['commit'])
            for schema in ('rust-collector-eligibility/v1', 'rust-collector-eligibility/v3'):
                bad = copy.deepcopy(actual)
                bad['schema'] = schema
                with self.assertRaises(ValueError):
                    policy.validate_receipt(bad, original['tag'], original['commit'])
            bad = copy.deepcopy(actual)
            bad['environment']['reviewer']['id'] = 1
            with self.assertRaises(ValueError):
                policy.validate_receipt(bad, original['tag'], original['commit'])

        for mutate in (lambda v: v.update(name='release'),
                       lambda v: v.update(can_admins_bypass=True),
                       lambda v: v['protection_rules'][0].update(reviewers=[]),
                       lambda v: v['protection_rules'][0]['reviewers'][0].update(type='Team'),
                       lambda v: v['protection_rules'][0]['reviewers'][0]['reviewer'].update(id=1),
                       lambda v: v['protection_rules'][0]['reviewers'][0]['reviewer'].update(login='other'),
                       lambda v: v['protection_rules'].append(copy.deepcopy(v['protection_rules'][0])),
                       lambda v: v['protection_rules'][0]['reviewers'].append({'type': 'User', 'reviewer': {'id': 1}})):
            bad = copy.deepcopy(environment)
            mutate(bad)
            with self.assertRaises(ValueError):
                policy.protected_environment(bad)

    def test_independent_eligibility_reuses_exact_main_ci_without_core_version(self):
        manifest = fixture()[0]
        manifest['collector']['version'] = '0.1.0-rc.1'
        tag, commit = 'rust-collector-v0.1.0-rc.1', manifest['source_commit']
        environment = {'can_admins_bypass': False, 'protection_rules': [
            {'type': 'required_reviewers', 'prevent_self_review': True, 'reviewers': [{'id': 1}]}]}
        from unittest.mock import Mock
        client = Mock(repository=assets.REPOSITORY)
        client.get_json.return_value = environment
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'manifest.json'
            assets.write(path, manifest)
            with patch.object(policy.core, 'verify_git_state', return_value=commit) as git_check, \
                    patch.object(policy.core, 'verify_ci_run', return_value=receipt(manifest)['ci']) as ci_check:
                result = policy.verify(Path(temporary), path, tag, commit, client)
                self.assertEqual(result['status'], 'pass')
                git_check.assert_called_once_with(Path(temporary), tag, commit, 'refs/remotes/origin/main')
                ci_check.assert_called_once_with(client, commit, 'main')
                for bad_tag, bad_commit in (('v0.1.0-rc.1', commit), (tag, '0' * 40)):
                    with self.assertRaises(ValueError):
                        policy.verify(Path(temporary), path, bad_tag, bad_commit, client)
            with patch.object(policy.core, 'verify_git_state', side_effect=policy.core.PolicyError('not main')):
                with self.assertRaisesRegex(policy.core.PolicyError, 'not main'):
                    policy.verify(Path(temporary), path, tag, commit, client)
            with patch.object(policy.core, 'verify_git_state', return_value=commit), \
                    patch.object(policy.core, 'verify_ci_run', side_effect=policy.core.PolicyError('missing CI')):
                with self.assertRaisesRegex(policy.core.PolicyError, 'missing CI'):
                    policy.verify(Path(temporary), path, tag, commit, client)
            manifest['collector']['version'] = '0.1.0'
            assets.write(path, manifest)
            with self.assertRaisesRegex(ValueError, 'stable promotion blocked'):
                policy.verify(Path(temporary), path, 'rust-collector-v0.1.0', commit, client)

    def test_production_template_uses_restricted_runner_group(self):
        source = (RELEASE / 'rust-collector-release.production.yml').read_text()
        self.assertIn('    runs-on:\n      group: harness-gate-rust-collector-release\n'
                      '      labels: rust-collector-release', source)
        self.assertIn('environment: rust-collector-release', source)
        self.assertIn('needs: protection', source)
        self.assertNotIn('runs-on: [self-hosted', source)

    def test_default_rehearsal_and_explicit_publication_remain_protected(self):
        source = (RELEASE.parents[1] / assets.WORKFLOW).read_text()
        self.assertIn('needs: protection', source)
        self.assertIn('policy.protected_environment(client.get_json(', source)
        self.assertIn('environment: rust-collector-release', source)
        self.assertIn('contents: read', source)
        self.assertIn('workflow_dispatch:', source)
        rehearsal = source.split('  dry-run:', 1)[1].split('\n  sign-private-candidate:', 1)[0]
        self.assertNotIn('contents: write', rehearsal)
        self.assertNotIn('id-token: write', rehearsal)
        self.assertIn("inputs.signing_packet_sha256 == ''", rehearsal)
        signing = source.split('  sign-private-candidate:', 1)[1].split('\n  publish:', 1)[0]
        publication = source.split('\n  publish:', 1)[1]
        self.assertNotIn('gh release create', rehearsal)
        self.assertNotIn('gh release create', signing)
        self.assertIn("inputs.publication_packet_sha256 != ''", publication)
        self.assertIn("inputs.signing_packet_sha256 == ''", publication)
        self.assertIn("github.ref == 'refs/heads/main'", publication)
        self.assertIn('needs: protection', publication)
        self.assertIn('environment: rust-collector-release', publication)
        self.assertIn('group: harness-gate-rust-collector-release', publication)
        self.assertIn('production_collector_release.py assemble', publication)
        self.assertIn('--packet-sha256', publication)
        self.assertNotIn('git tag ', source)
        self.assertNotIn('\n  push:', source)

    def test_only_independent_exact_stable_or_rc_tags(self):
        for value in ('0.1.0', '12.34.56-rc.7'):
            self.assertEqual(assets.version(value), value)
        for value in ('v0.1.0', '0.1', '01.1.0', '0.1.0-rc.0', '0.1.0-beta.1', '0.1.0+build', '../1'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                assets.version(value)

    def test_protected_approval_is_mandatory(self):
        good = {'can_admins_bypass': False, 'protection_rules': [
            {'type': 'required_reviewers', 'prevent_self_review': True, 'reviewers': [{'id': 1}]}]}
        self.assertTrue(policy.protected_environment(good)['required_reviewers'])
        for mutate in (lambda v: v.update(can_admins_bypass=True),
                       lambda v: v['protection_rules'][0].update(prevent_self_review=False),
                       lambda v: v['protection_rules'][0].update(reviewers=[])):
            value = copy.deepcopy(good)
            mutate(value)
            with self.assertRaises(ValueError):
                policy.protected_environment(value)

    def test_receipt_cannot_drop_ci_or_approval(self):
        manifest = fixture()[0]
        value = receipt(manifest)
        policy.validate_receipt(value, value['tag'], value['commit'])
        for field in ('tag', 'commit', 'protected_main', 'environment', 'ci'):
            bad = copy.deepcopy(value)
            bad[field] = {} if field in ('ci', 'environment') else 'wrong'
            with self.subTest(field=field), self.assertRaises((ValueError, KeyError)):
                policy.validate_receipt(bad, value['tag'], value['commit'])


if __name__ == '__main__':
    unittest.main()
