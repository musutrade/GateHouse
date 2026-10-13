import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_timing
import ci_comparison
from quality_common import ROOT


class HostedTimingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        directory = ROOT / 'docs/quality/ci-topology'
        cls.raw = (directory / 'hosted-input.json').read_bytes()
        cls.source = json.loads(cls.raw)
        cls.contract = json.loads((ROOT / 'tools/quality/fixtures/ci-topology.json').read_text())
        cls.baseline = json.loads((directory / 'hosted-baseline.json').read_text())

    def test_retained_baseline_reproduces_from_hosted_timestamps(self):
        result = ci_timing.normalize(self.source, self.contract)
        result['input_sha256'] = hashlib.sha256(self.raw).hexdigest()
        self.assertEqual(result, self.baseline)
        first = result['runs'][0]
        # Manually checked 02:29:26 -> 02:48:27 aggregate completion.
        self.assertEqual(first['critical_path_seconds'], 1141)
        self.assertEqual(first['last_required_child'], 'Quality Coverage and Critical Paths')
        # Native job wall minutes do not include any OS billing multiplier.
        self.assertEqual(first['runner_wall_minutes_by_os']['Windows'], 339 / 60)
        skipped = [j for j in first['jobs'] if j['conclusion'] == 'skipped']
        self.assertTrue(skipped)
        self.assertTrue(all(j['wall_seconds'] == 0 and j['os'] is None for j in skipped))

    def test_incomplete_or_mixed_hosted_evidence_is_rejected(self):
        for mutation in ('missing-native', 'failed-native', 'mixed-attempt', 'unknown-os'):
            with self.subTest(mutation=mutation):
                run = copy.deepcopy(self.source['runs'][0])
                native = next(j for j in run['jobs'] if j['name'] == 'Test (windows-latest)')
                if mutation == 'missing-native':
                    run['jobs'].remove(native)
                elif mutation == 'failed-native':
                    native['conclusion'] = 'failure'
                elif mutation == 'mixed-attempt':
                    native['run_attempt'] += 1
                else:
                    native['labels'] = ['self-hosted']
                with self.assertRaises((ValueError, KeyError)):
                    ci_timing.normalize_run(run, self.contract)

    def test_step_categories_separate_install_collection_and_cleanup(self):
        self.assertEqual(ci_timing.category('Configure Cargo state'), 'setup')
        self.assertEqual(ci_timing.category('Seal immutable quality evidence'), 'artifact')
        self.assertEqual(ci_timing.category('Verify artifact identity and hashes before consumption'), 'artifact')
        self.assertEqual(ci_timing.category('Install cargo-nextest and cargo-llvm-cov'), 'tool_install')
        self.assertEqual(ci_timing.category('Install Rust'), 'setup')
        self.assertEqual(ci_timing.category('Post Install Python'), 'cleanup')
        self.assertEqual(ci_timing.category('Upload quality evidence'), 'artifact')
        with self.assertRaises(ValueError):
            ci_timing.seconds('2026-09-09T01:00:01Z', '2026-09-09T01:00:00Z')

    def test_after_and_stage_evidence_reproduce_with_the_baseline_normalization(self):
        directory = ROOT / 'docs/quality/ci-topology'
        for source, output in [('hosted-after-input.json', 'hosted-after.json'),
                               ('hosted-stage-input.json', 'hosted-stages.json')]:
            raw = (directory / source).read_bytes()
            result = ci_timing.normalize(json.loads(raw), self.contract)
            result['input_sha256'] = hashlib.sha256(raw).hexdigest()
            self.assertEqual(result, json.loads((directory / output).read_text()))

    def test_comparison_keeps_runner_cost_and_nested_overhead_separate(self):
        directory = ROOT / 'docs/quality/ci-topology'
        result = ci_comparison.compare(directory)
        self.assertEqual(result, json.loads((directory / 'hosted-comparison.json').read_text()))
        self.assertEqual(result['before']['category_seconds']['tool_install']['median'], 423)
        self.assertEqual(result['after']['critical_path_seconds']['median'], 878.5)
        self.assertEqual(result['after']['total_runner_wall_minutes']['max'], 2420 / 60)
        latest = result['supplemental_log_audit'][0]['jobs']
        self.assertEqual(sum(j['target_cache_hits'] for j in latest), 0)
        self.assertEqual(sum(j['target_cache_misses'] for j in latest), 7)
        self.assertAlmostEqual(sum(j['target_cache_restore_and_save_seconds'] for j in latest), 41.964)


if __name__ == '__main__':
    unittest.main()
