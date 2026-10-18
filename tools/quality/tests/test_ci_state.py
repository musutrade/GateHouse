"""Negative contracts for mutable cache and immutable artifact trust boundaries."""
import copy
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_artifact
import ci_cargo
import contracts
from quality_common import ROOT, sha256, write_json


class CargoStateTests(unittest.TestCase):
    def configure(self, root, *, job='test', profiles='test;default',
                  toolchain='toolchain', configuration='config', **environment):
        env = {'RUNNER_OS': 'Linux', 'RUNNER_ARCH': 'X64',
               'GITHUB_ENV': str(root / 'env'), 'GITHUB_OUTPUT': str(root / 'output'),
               **environment}
        target = root / 'target/ci' / env['RUNNER_OS'] / env['RUNNER_ARCH'] / job
        with patch.dict(os.environ, env), patch.object(ci_cargo, 'ROOT', root), \
                patch.object(ci_cargo, 'target_directory', return_value=target), \
                patch.object(ci_cargo, 'command_output', return_value=toolchain), \
                patch.object(ci_cargo, 'configuration_hash', return_value=configuration), \
                patch('builtins.print'):
            ci_cargo.configure(job, profiles)
        return json.loads((root / f'target/quality/cache/{job}.json').read_text())

    def test_cache_identity_separates_os_arch_class_profile_and_flags(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            first = self.configure(root)
            self.assertEqual(first['cache_identity'], self.configure(root)['cache_identity'])
            for change in ({'RUNNER_OS': 'Windows'}, {'RUNNER_OS': 'macOS'},
                           {'RUNNER_ARCH': 'ARM64'}, {'job': 'release'},
                           {'toolchain': 'new rustc'}, {'configuration': 'changed lock/config'},
                           {'profiles': 'release;all-features'},
                           {'RUSTFLAGS': '-Cinstrument-coverage'}):
                with self.subTest(change=change):
                    self.assertNotEqual(first['cache_identity'],
                                        self.configure(root, **change)['cache_identity'])
            self.assertIn('CARGO_TARGET_DIR=', (root / 'env').read_text())

    def test_metadata_mismatch_fails_before_cache_outputs(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with patch.dict(os.environ, RUNNER_OS='Linux', RUNNER_ARCH='X64'), \
                    patch.object(ci_cargo, 'ROOT', root), \
                    patch.object(ci_cargo, 'target_directory', return_value=root / 'wrong'):
                with self.assertRaisesRegex(ValueError, 'Cargo target mismatch'):
                    ci_cargo.configure('test', 'test')
            self.assertFalse((root / 'target').exists())

    def test_contracts_rebuild_and_resolve_redirected_target(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / 'debug' / ('harness-gate.exe' if os.name == 'nt' else 'harness-gate')
            binary.parent.mkdir()
            binary.write_text('stale cached executable')
            with patch.object(ci_cargo, 'target_directory', return_value=root), \
                    patch.object(contracts.subprocess, 'run') as build, \
                    patch.object(contracts, 'collect', return_value=[]) as collect, \
                    patch.object(contracts, 'metadata', return_value={}):
                contracts.run(root / 'report.json', structured=True)
                self.assertIn('build', build.call_args.args[0])
                collect.assert_called_once_with(binary, staged_secrets=False)
                collect.reset_mock()
                build.side_effect = subprocess.CalledProcessError(1, 'cargo build')
                with self.assertRaises(subprocess.CalledProcessError):
                    contracts.run(root / 'failed.json', structured=True)
                collect.assert_not_called()


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.expected = {'producer': 'quality-coverage', 'kind': 'instrumented-quality-evidence',
                         'GITHUB_SHA': 'a' * 40, 'GITHUB_REPOSITORY': 'owner/repo',
                         'GITHUB_WORKFLOW': 'CI', 'GITHUB_RUN_ID': '123',
                         'GITHUB_RUN_ATTEMPT': '1', 'configuration_sha256': 'b' * 64,
                         'RUNNER_OS': 'Linux', 'RUNNER_ARCH': 'X64'}
        write_json(self.root / 'candidate.json', {'commit': 'a' * 40})
        (self.root / 'coverage.json').write_text('original evidence')
        with patch.dict(os.environ, GITHUB_JOB='quality-coverage'), \
                patch.object(ci_artifact, 'command_output', return_value='pinned tool identity'):
            self.digest = ci_artifact.seal(self.root, self.expected)

    def test_round_trip_and_missing_producer_anchor(self):
        self.assertTrue(ci_artifact.verify(self.root, self.expected, self.digest)['verified'])
        for digest in ('', None, 'f' * 64):
            with self.subTest(digest=digest), self.assertRaises(ValueError):
                ci_artifact.verify(self.root, self.expected, digest)

    def test_every_identity_dimension_fails_closed(self):
        for key in self.expected:
            expected = {**self.expected, key: 'different'}
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'identity mismatch'):
                ci_artifact.verify(self.root, expected, self.digest)

    def test_changed_missing_or_extra_evidence_fails_closed(self):
        evidence = self.root / 'coverage.json'
        for operation in ('change', 'delete', 'extra'):
            with self.subTest(operation=operation):
                evidence.write_text('original evidence')
                if operation == 'change':
                    evidence.write_text('tampered evidence')
                elif operation == 'delete':
                    evidence.unlink()
                else:
                    (self.root / 'unlisted.json').write_text('extra')
                with self.assertRaisesRegex(ValueError, 'inventory/hash mismatch'):
                    ci_artifact.verify(self.root, self.expected, self.digest)

    def test_manifest_tool_tamper_and_self_rehashed_payload_fail(self):
        manifest = self.root / ci_artifact.MANIFEST
        original = json.loads(manifest.read_text())
        for field in ('tools', 'files'):
            report = copy.deepcopy(original)
            report[field] = {}
            write_json(manifest, report)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'manifest hash mismatch'):
                ci_artifact.verify(self.root, self.expected, self.digest)
        report = copy.deepcopy(original)
        report['files'] = {'../escape': '0' * 64}
        write_json(manifest, report)
        with self.assertRaisesRegex(ValueError, 'inventory/hash mismatch'):
            ci_artifact.verify(self.root, self.expected, sha256(manifest))

    def test_build_state_and_symlinks_are_not_reusable_evidence(self):
        mutable = self.root / 'build'
        mutable.mkdir()
        with self.assertRaisesRegex(ValueError, 'mutable build state'):
            ci_artifact.verify(self.root, self.expected, self.digest)
        mutable.rmdir()
        (self.root / 'link').symlink_to(self.root / 'coverage.json')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            ci_artifact.verify(self.root, self.expected, self.digest)

    def test_missing_manifest_and_wrong_producer_fail(self):
        (self.root / ci_artifact.MANIFEST).unlink()
        with self.assertRaises(OSError):
            ci_artifact.verify(self.root, self.expected, self.digest)
        with patch.dict(os.environ, GITHUB_JOB='other'):
            with self.assertRaisesRegex(ValueError, 'unexpected artifact producer'):
                ci_artifact.seal(self.root, self.expected)


class WorkflowBoundaryTests(unittest.TestCase):
    def test_unproductive_compiled_caches_are_disabled_for_all_callers(self):
        action = (ROOT / '.github/actions/cargo-state/action.yml').read_text()
        self.assertIn("default: 'false'", action.split('  cache-target:')[1].split('runs:')[0])
        for workflow in (ROOT / '.github/workflows').glob('*.yml'):
            self.assertNotRegex(workflow.read_text(), r'cache-target:\s*[\"\x27]?true')

    def test_quality_collection_shards_are_only_owners_and_shadow_only_consumes_evidence(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        jobs = dict(re.findall(r'^  ([a-z][a-z-]+):\n(.*?)(?=^  [a-z][a-z-]+:|\Z)',
                               workflow, re.M | re.S))
        owners = [name for name, body in jobs.items() if 'ci_quality.py collect' in body]
        self.assertEqual(owners, ['quality-fast', 'quality-risk', 'quality-matrix'])
        for name in owners:
            self.assertEqual(jobs[name].count('ci_quality.py collect'), 1)
        risk = jobs['quality-risk']
        self.assertIn('side: [base, head]', risk)
        self.assertIn('shard: [1, 2, 3, 4]', risk)
        self.assertIn('ci_quality.py collect-risk-shard', risk)
        self.assertIn('RISK_SIDE: ${{ matrix.side }}', risk)
        self.assertIn('RISK_SHARD: ${{ matrix.shard }}', risk)
        self.assertIn('--risk-shard "$RISK_SHARD" --partitions 4', risk)
        self.assertIn('--exclude-tests "$excluded" --extra-tests "$extra"', risk)
        self.assertIn("excluded='test(frozen_policy_and_reference_acceptance_match_python)'", risk)
        self.assertIn('html_export_combined_thousand_subjects_preserves_fail_and_rejects_tampering', risk)
        self.assertIn('extra="$excluded"', risk)
        self.assertIn('quality-artifacts-risk-${{ matrix.side }}-${{ matrix.shard }}-', risk)
        self.assertNotIn('CI_PARALLEL_RISK', risk)
        merge = jobs['quality-coverage']
        self.assertEqual(merge.count('ci_quality.py collect'), 0)
        self.assertIn('ci_quality.py assemble-risk-shards', merge)
        self.assertIn('pattern: quality-artifacts-risk-*', merge)
        self.assertLess(merge.index('assemble-risk-shards'), merge.index('ci_quality.py merge'))
        self.assertIn('ci_quality.py verify', merge)
        self.assertIn('needs: [classify, quality-fast, quality-risk, quality-matrix]', merge)
        consumer = jobs['quality-generic-shadow']
        self.assertIn('needs: [classify, quality-coverage]', consumer)
        self.assertIn('quality-coverage-${{ github.run_id }}-${{ github.run_attempt }}', consumer)
        self.assertIn('--candidate target/quality/candidate/candidate.json', consumer)
        self.assertNotRegex(consumer, r'cargo|rust-toolchain|install-ci-tool|--collect|coverage\.py|source_measure\.py')
        self.assertNotIn('ci_artifact.py seal', consumer)

    def test_authoritative_consumer_requires_independent_manifest_digest(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        consumer = workflow.split('  quality-generic-shadow:\n')[1].split('  quality-contracts:\n')[0]
        self.assertIn('needs.quality-coverage.outputs.manifest-sha256', consumer)
        self.assertLess(consumer.index('ci_artifact.py verify'), consumer.index('rust_reference.py'))
        projection = consumer.split('- name: Project and compare retained evidence')[1].split('- name: Upload')[0]
        self.assertNotIn('always()', projection)
        self.assertNotIn('continue-on-error', consumer)
        self.assertNotIn('tools/harness-gate/target/', workflow)
        self.assertNotIn('uses: actions/cache', workflow)
        action = (ROOT / '.github/actions/cargo-state/action.yml').read_text()
        self.assertIn('path: ${{ steps.state.outputs.target }}', action)
        self.assertNotIn('/bin/', action)
        self.assertNotIn('enableCrossOsArchive', action)

    def test_linux_tests_are_partitioned_with_one_acceptance_owner(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        job = workflow.split('  test:\n', 1)[1].split('  test-cross-platform:\n', 1)[0]
        self.assertIn('shard: [1, 2, 3, 4]', job)
        self.assertIn('--partition count:${{ matrix.shard }}/4', job)
        self.assertIn('not test(configured_ci_acceptance_retains_parity_reuse_and_negative_evidence)', job)
        self.assertIn('not test(html_export_combined_thousand_subjects_preserves_fail_and_rejects_tampering)', job)
        acceptance = job.split('- name: Run CI acceptance and HTML example tests', 1)[1]
        acceptance = acceptance.split('- name: Test findings feedback', 1)[0]
        self.assertIn('if: matrix.shard == 1', acceptance)
        self.assertIn('HARNESS_GATE_CI_ACCEPTANCE:', acceptance)
        self.assertIn('HG_HTML_ACCEPTANCE_DIR:', acceptance)
        upload = job.split('- name: Upload configured workflow acceptance', 1)[1]
        upload = upload.split('- name: Verify generated configuration schema', 1)[0]
        self.assertIn('if: always() && matrix.shard == 1', upload)
        onboarding = job.split('- name: Retain real plugin onboarding evidence', 1)[1]
        self.assertIn('if: always() && matrix.shard == 1', onboarding)

    def test_cross_platform_tests_are_partitioned_without_changing_the_check_name(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        native = workflow.split('  test-cross-platform:\n')[1].split('  security-audit:\n')[0]
        self.assertIn("name: ${{ matrix.shard == 1 && format('Test ({0})', matrix.os) || "
                      "format('Test ({0}, shard {1})', matrix.os, matrix.shard) }}", native)
        self.assertIn('os: [macos-latest, windows-latest]', native)
        self.assertIn('shard: [1, 2, 3, 4]', native)
        self.assertIn('CI_OS: ${{ matrix.os }}', native)
        self.assertIn('CI_SHARD: ${{ matrix.shard }}', native)
        self.assertIn("CI_PARTITIONS: 4", native)
        self.assertIn('--partition "count:${CI_SHARD}/${CI_PARTITIONS}"', native)
        self.assertIn('-E "not ($excluded)"', native)
        self.assertIn('-E "$extra"', native)
        self.assertIn("excluded='test(frozen_policy_and_reference_acceptance_match_python)'", native)
        # Heavy tests are now distributed across shards 1 and 3 (was shard 2 and 3 with 3-way split)
        self.assertIn("extra='test(large_core_quality_json_is_redacted_and_finalized_without_truncation)'", native)
        self.assertIn("extra='test(frozen_evidence_and_integrity_matrix_matches_python)'", native)
        self.assertIn('fail-fast: false', native)
        regex = re.compile(r'^\s+if:', re.M)
        self.assertNotRegex(native.split('    steps:\n', 1)[1], regex)

    def test_measurements_do_not_restore_compiled_state(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        for name in ('quality-fast', 'quality-risk', 'quality-matrix'):
            block = re.split(r'\n  [a-z]', workflow.split(f'  {name}:\n')[1])[0]
            self.assertIn("cache-target: 'false'", block)
        trends = (ROOT / '.github/workflows/quality-trends.yml').read_text()
        for name in ('coverage', 'quality-baseline'):
            block = re.split(r'\n  [a-z]', trends.split(f'  {name}:\n')[1])[0]
            self.assertIn("cache-target: 'false'", block)
        self.assertIn('matrix:\n        os: [macos-latest, windows-latest]', workflow)
