"""Acceptance cannot count duplicate runs or accept incomplete/tampered evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality
import rust_equivalence as acceptance


class AcceptanceTests(unittest.TestCase):
    def test_invalid_windows_fail_closed_with_retained_report(self):
        original = json.loads(acceptance.WINDOW.read_text())
        for case in ('missing_run', 'duplicate_head', 'duplicate_run', 'changed_archive'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                window = copy.deepcopy(original)
                if case == 'missing_run':
                    window['runs'].pop()
                elif case in ('duplicate_head', 'duplicate_run'):
                    key = 'commit' if case == 'duplicate_head' else 'run'
                    window['runs'][1]['identity'][key] = window['runs'][0]['identity'][key]
                else:
                    window['runs'][0]['sha256'] = '0' * 64
                manifest = Path(directory) / 'window.json'
                manifest.write_text(json.dumps(window))
                output = Path(directory) / 'acceptance'
                with patch.object(acceptance, 'WINDOW', manifest):
                    report = acceptance.replay(output)
                self.assertFalse(report['accepted'])
                self.assertTrue(report['failures'])
                self.assertEqual(json.loads((output / 'acceptance.json').read_text()), report)


class ShadowWorkflowTests(unittest.TestCase):
    def test_shadow_reuses_exact_artifact_and_event_identity_without_collection(self):
        workflow = (acceptance.ROOT / '.github/workflows/ci.yml').read_text()
        shadow = workflow.split('  quality-generic-shadow:\n')[1].split('  quality-contracts:\n')[0]
        collection = workflow.split('  quality-coverage:\n')[1].split('  quality-generic-shadow:\n')[0]
        self.assertIn('needs: [classify, quality-coverage]', shadow)
        self.assertIn('actions/download-artifact@018cc2cf5baa6db3ef3c5f8a56943fffe632ef53 # v6', shadow)
        for identity in (
                'BASE_SHA: ${{ github.event.pull_request.base.sha || github.event.before }}',
                'HEAD_SHA: ${{ github.sha }}',
                'RUN_ID: ${{ github.run_id }}-${{ github.run_attempt }}',
                'name: quality-coverage-${{ github.run_id }}-${{ github.run_attempt }}'):
            self.assertIn(identity, shadow)
            self.assertIn(identity, collection)
        self.assertIn('--candidate target/quality/candidate/candidate.json', shadow)
        self.assertIn('--output target/quality/generic-shadow', shadow)
        self.assertIn('target/quality/generic-shadow/*.json', shadow)
        self.assertIn('target/quality/artifact-validation.json', shadow)
        self.assertIn('needs.quality-coverage.outputs.manifest-sha256', shadow)
        self.assertLess(shadow.index('ci_artifact.py verify'), shadow.index('rust_reference.py'))
        self.assertEqual(shadow.count('if: ${{ always() }}'), 1)
        self.assertNotIn('continue-on-error', shadow)
        for expensive in ('cargo ', 'ci_quality.py collect', 'source_measure.py', 'critical_paths.py'):
            self.assertNotIn(expensive, shadow)

    def test_required_identity_dependencies_and_fail_closed_contract_remain_frozen(self):
        workflow = (acceptance.ROOT / '.github/workflows/ci.yml').read_text()
        required = workflow.split('  quality-required:\n')[1]
        self.assertIn('name: Required Quality Aggregate\n', required)
        self.assertIn('if: ${{ always() }}', required)
        expected = ['classify', 'test', 'test-cross-platform', 'security-audit', 'fmt', 'clippy', 'build',
                    'build-cross-platform', 'coverage', 'quality-coverage', 'quality-contracts',
                    'quality-contracts-cross-platform', 'quality-baseline', 'docs-consistency',
                    'release-contracts', 'quality-scripts']
        dependencies = required.split('needs:')[1].split('runs-on:')[0].strip(' []\n').replace('\n', '')
        self.assertEqual([v.strip() for v in dependencies.split(',')], expected)
        self.assertEqual(set(ci_quality.COMMON + ci_quality.PUSH_ONLY), set(expected))
        self.assertIn('NEEDS_JSON: ${{ toJSON(needs) }}', required)
        self.assertIn('run: python3 tools/quality/ci_quality.py aggregate', required)
        for event in ('push', 'pull_request'):
            needs = {name: {'result': 'success'} for name in expected}
            needs['quality-generic-shadow'] = {'result': 'failure'}
            self.assertEqual(ci_quality.aggregate(event, needs), [])
            for job in ci_quality.COMMON + (ci_quality.PUSH_ONLY if event == 'push' else ()):
                for state in ('failure', 'cancelled', 'skipped', 'measurement_error', None):
                    with self.subTest(event=event, job=job, state=state):
                        self.assertEqual(ci_quality.aggregate(event, {
                            **needs, job: {'result': state}}), [job])


if __name__ == '__main__':
    unittest.main()
