"""Frozen historical inputs plus explicit adversarial/ratchet derivations."""
import copy
from fractions import Fraction
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import harness_evidence as evidence
import policy_engine as engine
import rust_reference as adapter
from quality_common import sha256, write_json

ROOT = Path(__file__).resolve().parents[3]
HISTORY = json.loads((ROOT / 'tools/quality/fixtures/rust-reference/history.json').read_text())


class RustReferenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.directory = Path(cls.temp.name)
        cls.original = cls.directory / 'original'
        archive = ROOT / HISTORY['retained_candidate']['path']
        assert sha256(archive) == HISTORY['retained_candidate']['sha256']
        with tarfile.open(archive) as bundle:
            bundle.extractall(cls.original, filter='data')
        cls.candidate = json.loads((cls.original / 'candidate.json').read_text())
        cls.risk = json.loads((cls.original / 'head-risk.json').read_text())
        cls.context = {'commit': cls.candidate['commit'], 'base_commit': cls.candidate['base_sha'],
                       'target': cls.candidate['target'], 'run': cls.candidate['run_id']}
        cls.sources = cls.directory / 'sources'
        adapter.sources_from_archive(cls.original / 'head-source.tar', cls.sources)

    def setUp(self):
        self.temp_case = tempfile.TemporaryDirectory(dir=self.directory)
        self.addCleanup(self.temp_case.cleanup)
        self.case = Path(self.temp_case.name)

    def shadow(self, root=None, **identity):
        return adapter.shadow((root or self.original) / 'candidate.json', self.case / 'shadow',
                              **{'head': self.candidate['commit'], 'base': self.candidate['base_sha'],
                                 'run_id': self.candidate['run_id'], **identity})

    def mutable(self):
        root = self.case / 'candidate'
        shutil.copytree(self.original, root)
        return root

    def rehash(self, root, filename, report):
        write_json(root / filename, report)
        candidate = json.loads((root / 'candidate.json').read_text())
        candidate['artifacts'][filename] = sha256(root / filename)
        write_json(root / 'candidate.json', candidate)

    def test_complete_profile_fixture_preserves_certified_required_policy_and_series(self):
        self.shadow()
        reference = json.loads((ROOT / 'tools/harness-gate/fixtures/workflow/collectors/certified-rust-profiles.json').read_text())
        rules = json.loads((self.case / 'shadow/head-risk-policy.json').read_text())['rules']
        records = json.loads((self.case / 'shadow/head-risk-evidence.json').read_text())
        self.assertEqual(reference['rules'], [r for r in rules if r['required'] and r['scope']['boundary'] == 'high'])
        self.assertEqual(reference['series'], next(r['series'] for r in records if r['subject']['boundary'] == 'high'))
        self.assertEqual({r['metric'] for r in reference['rules']}, {'coverage.line', 'coverage.region', 'risk.crap'})
        for profile in ('full', 'ci'):
            self.assertEqual(set(reference['profiles'][profile]['policies']), {r['id'] for r in reference['rules']})

    def test_historical_replay_preserves_all_values_identities_and_artifacts_without_collection(self):
        with patch.object(subprocess, 'run', side_effect=AssertionError('collection forbidden')), \
             patch.object(subprocess, 'check_output', side_effect=AssertionError('collection forbidden')):
            report = self.shadow()
        self.assertTrue(report['compatible'], report['compatibility_failures'])
        self.assertFalse(report['migration_blocked'])
        self.assertEqual(report['current_state'], report['generic_state'])
        self.assertEqual(report['identity'], self.context)
        self.assertEqual(report['retained_artifacts'], self.candidate['artifacts'])
        self.assertEqual(report['current_stages'], self.candidate['stages'])
        native = json.loads((self.original / 'risk.json').read_text())
        self.assertEqual(report['identities'], native['identities'])
        self.assertEqual(len(report['identities']), HISTORY['retained_candidate']['functions'])
        self.assertEqual(sum(x['coverage_debt'] for x in report['identities']),
                         HISTORY['retained_candidate']['legacy_debt'])
        for label in ('base', 'head'):
            records = json.loads((self.case / f'shadow/{label}-risk-evidence.json').read_text())
            native_rows = json.loads((self.original / f'{label}-risk.json').read_text())['functions']
            self.assertEqual(len(records), len(native_rows))
            for record, row in zip(records, native_rows):
                self.assertEqual(json.loads(record['subject']['metadata']['rust_native']), row)
                self.assertEqual(record['source']['sha256'], row['source_sha256'])
                values = {m['name']: m['value'] for m in record['metrics']}
                self.assertEqual(engine._value(values['risk.crap']), Fraction(*row['crap_exact']))
                self.assertEqual(values['complexity.cyclomatic']['value'], row['cc'])
                for key in ('lines', 'regions'):
                    self.assertEqual(values[adapter.METRICS[key]],
                                     {'type': 'ratio', 'covered': row[key]['covered'], 'total': row[key]['count']})
                self.assertNotIn('coverage.branch', values)
                self.assertEqual(next(c['state'] for c in record['capabilities']
                                      if c['metric'] == 'coverage.branch'), 'unsupported')
                self.assertEqual(record['artifacts'][0]['path'], f'{label}-risk.json')
        production = json.loads((self.original / 'production.json').read_text())
        records = json.loads((self.case / 'shadow/production-evidence.json').read_text())
        for record in records:
            row = json.loads(record['subject']['metadata']['rust_native'])
            for metric in record['metrics']:
                key = next(k for k, v in adapter.METRICS.items() if v == metric['name'])
                self.assertEqual(metric['value'], {'type': 'ratio', 'covered': row[key]['covered'],
                                                   'total': row[key]['count']})
            if record['subject']['kind'] == 'file/v1':
                path = record['source']['path'].removeprefix('tools/harness-gate/')
                self.assertEqual(row, production['files'][path])
                self.assertEqual(record['source']['sha256'], production['source_sha256'][path])
        for metric, pair in HISTORY['retained_candidate']['aggregate'].items():
            self.assertEqual([production['aggregate'][metric][k] for k in ('covered', 'count')], pair)

    def test_accepted_phase_one_baseline_retains_its_original_incompatible_series(self):
        fixture = HISTORY['accepted_baseline']
        path = ROOT / fixture['path']
        before = path.read_bytes()
        self.assertEqual(sha256(path), fixture['sha256'])
        native = json.loads(before)
        self.assertEqual(native['commit'], fixture['commit'])
        self.assertEqual(native['series_key'], fixture['series'])
        report = adapter.shadow(path, self.case / 'shadow', head=native['commit'],
                                base=native['commit'], run_id='33223804928')
        self.assertEqual(report['state'], fixture['projection'])
        self.assertTrue(report['migration_blocked'])
        self.assertEqual(path.read_bytes(), before)

    def test_failed_production_gate_preserves_counts_and_blocks_cli(self):
        root = self.mutable()
        production = json.loads((root / 'production.json').read_text())
        production['threshold'] = '100'
        production['failures'] = []
        for key, row in {**production['boundaries'], 'aggregate': production['aggregate']}.items():
            if row.get('blocking', True):
                row['status'] = ('pass' if row['lines']['count'] and
                                 row['lines']['covered'] == row['lines']['count'] else 'fail')
                if row['status'] == 'fail':
                    production['failures'].append(key)
        production['status'] = 'fail'
        self.rehash(root, 'production.json', production)
        candidate = json.loads((root / 'candidate.json').read_text())
        candidate['stages']['production']['status'] = 'failure'
        write_json(root / 'candidate.json', candidate)
        command = [sys.executable, str(ROOT / 'tools/quality/rust_reference.py'),
                   '--candidate', str(root / 'candidate.json'), '--output', str(self.case / 'shadow'),
                   '--head-sha', candidate['commit'], '--base-sha', candidate['base_sha'],
                   '--run-id', candidate['run_id']]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads((self.case / 'shadow/shadow.json').read_text())
        self.assertTrue(report['compatible'], report['compatibility_failures'])
        self.assertTrue(report['migration_blocked'])
        self.assertEqual(report['current_state'], 'fail')
        self.assertEqual(report['production']['aggregate']['state'], 'fail')

    def test_tampered_or_missing_raw_artifact_is_measurement_error(self):
        for action in ('tampered', 'missing'):
            with self.subTest(action=action):
                root = self.mutable()
                path = root / 'head-risk.json'
                path.unlink() if action == 'missing' else path.write_text('{}')
                report = self.shadow(root)
                self.assertFalse(report['compatible'])
                self.assertTrue(report['migration_blocked'])
                self.assertEqual(report['state'], 'measurement_error')
                shutil.rmtree(root)
                shutil.rmtree(self.case / 'shadow')

    def test_stale_run_and_unknown_series_fail_closed(self):
        report = self.shadow(run_id='different-run')
        self.assertFalse(report['compatible'])
        self.assertIn('identity', report['compatibility_failures'][0])
        with self.assertRaisesRegex(ValueError, 'unknown Rust risk series'):
            adapter.risk_contract({**self.risk['series'], 'rule': 'unknown'})

    def test_other_required_stage_failures_cannot_be_hidden_by_projection(self):
        root = self.mutable()
        for status in ('failure', 'cancelled', 'skipped', 'measurement_error'):
            with self.subTest(status=status):
                candidate = copy.deepcopy(self.candidate)
                candidate['stages']['matrix'] = {'status': status, 'error': 'retained failure'}
                write_json(root / 'candidate.json', candidate)
                report = self.shadow(root)
                self.assertTrue(report['compatible'], report['compatibility_failures'])
                self.assertEqual(report['current_state'], 'fail')
                self.assertEqual(report['generic_state'], 'fail')
                self.assertTrue(report['migration_blocked'])
                self.assertEqual(report['current_stages'], candidate['stages'])
                shutil.rmtree(self.case / 'shadow')

    def test_stale_production_raw_reference_is_rejected(self):
        root = self.mutable()
        report = json.loads((root / 'production.json').read_text())
        key = next(k for k in report['raw_artifacts'] if k.endswith('/coverage.raw.json'))
        report['raw_artifacts'][key] = '0' * 64
        self.rehash(root, 'production.json', report)
        result = self.shadow(root)
        self.assertFalse(result['compatible'])
        self.assertIn('stale production raw input', result['compatibility_failures'][0])

    def test_policy_mismatch_blocks_even_when_current_rust_passes(self):
        evaluate = engine.evaluate
        def wrong(*args, **kwargs):
            result = evaluate(*args, **kwargs)
            result['aggregate']['state'] = 'fail'
            return result
        with patch.object(engine, 'evaluate', side_effect=wrong):
            report = self.shadow()
        self.assertFalse(report['compatible'])
        self.assertTrue(report['migration_blocked'])
        self.assertEqual(report['current_state'], 'pass')
        self.assertIn('production aggregate', report['compatibility_failures'])

    def test_native_debt_and_changed_function_ratchet(self):
        hotspots = adapter.risk_contract(self.risk['series'])
        low = next(r for r in self.risk['functions'] if not r['passed'] and r['cc'] <= 10 and
                   Fraction(*r['crap_exact']) <= 30 and r['name'] not in hotspots[r['source']])
        high = next(r for r in self.risk['functions'] if not r['passed'] and r['cc'] > 10 and
                    r['name'] not in hotspots[r['source']])
        for changed in (False, True):
            report = {**self.risk, 'functions': [low, high]}
            previous = {**self.risk, 'functions': [] if changed else [low, high]}
            _, result, identities = adapter.project_risk(self.original, self.sources, self.context,
                                                         report, previous, label='head')
            self.assertTrue(all(r['coverage_debt'] for r in identities))
            self.assertEqual([r['changed'] for r in identities], [changed, changed])
            self.assertTrue(identities[0]['accepted'])
            self.assertEqual(identities[1]['accepted'], not changed)
            self.assertEqual(result['aggregate']['state'], 'fail' if changed else 'pass')

    def test_optional_capability_and_measurement_error_drift_blocks_equivalence(self):
        evaluate = engine.evaluate
        for state in ('pass', 'measurement_error', 'omitted'):
            with self.subTest(state=state):
                def drift(*args, **kwargs):
                    result = evaluate(*args, **kwargs)
                    gate = next(g for g in result['results'] if g['state'] == 'unsupported')
                    if state == 'omitted':
                        result['results'].remove(gate)
                    else:
                        gate['state'] = state
                    return result
                with patch.object(engine, 'evaluate', side_effect=drift):
                    report = self.shadow()
                self.assertEqual(report['current_state'], 'pass')
                self.assertEqual(report['state'], 'measurement_error')
                self.assertFalse(report['compatible'])
                self.assertTrue(report['migration_blocked'])
                shutil.rmtree(self.case / 'shadow')

    def test_missing_candidate_retains_measurement_error_report(self):
        report = self.shadow(self.case / 'missing')
        self.assertEqual(report['state'], 'measurement_error')
        self.assertTrue(report['migration_blocked'])
        self.assertEqual(json.loads((self.case / 'shadow/shadow.json').read_text()), report)

    def test_raw_counter_and_display_mismatch_is_rejected(self):
        for field, value in [('crap_line', 0.0), ('cc', 999), ('crap_exact', [1, 0]), ('passed', False)]:
            with self.subTest(field=field), self.assertRaises((ValueError, ArithmeticError)):
                report = copy.deepcopy(self.risk)
                report['functions'] = [report['functions'][0]]
                report['functions'][0][field] = value
                adapter.project_risk(self.original, self.sources, self.context, report, self.risk, label='head')

    def test_empty_counts_are_unavailable_and_never_fabricated_values(self):
        values, states = adapter.coverage_values({'functions': {'count': 0, 'covered': 0}})
        self.assertEqual(values, {})
        self.assertEqual(states, {'coverage.function': 'not_applicable'})
        with self.assertRaises(ValueError):
            adapter.coverage_values({'lines': {'count': 0, 'covered': 1}})

    def test_native_json_rejects_duplicate_keys_and_nonfinite_display_values(self):
        path = self.case / 'native.json'
        for raw in ('{"cc": 1, "cc": 2}', '{"crap_line": NaN}', '{"crap_line": Infinity}'):
            path.write_text(raw)
            with self.assertRaises(ValueError):
                adapter.load_native(path)
        path.write_text('{"crap_line": 3.25}')
        self.assertEqual(adapter.load_native(path), {'crap_line': 3.25})


class ExactRiskTests(unittest.TestCase):
    def test_repeating_and_large_rationals_are_exact(self):
        limit = {'type': 'rational', 'numerator': 30, 'denominator': 1}
        for numerator, denominator, expected in [(433, 108, True), (30, 1, True),
                 (300000000000000001, 10000000000000000, False)]:
            value = {'type': 'rational', 'numerator': numerator, 'denominator': denominator}
            evidence._shape(value, definition='Rational')
            self.assertEqual(engine.compare(value, 'le', limit), expected)
        with self.assertRaises(ValueError):
            engine.compare(limit, 'le', {'type': 'decimal', 'value': '30'})
        for bad in ({'type': 'rational', 'numerator': 1, 'denominator': 0},
                    {'type': 'rational', 'numerator': -1, 'denominator': 2},
                    {'type': 'rational', 'numerator': 1.0, 'denominator': 2}):
            with self.assertRaises(ValueError):
                evidence._shape(bad, definition='Rational')
        self.assertFalse(evidence.metric_type_matches('coverage.line', 'rational'))


if __name__ == '__main__':
    unittest.main()
