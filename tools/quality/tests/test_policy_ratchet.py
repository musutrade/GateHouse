"""Acceptance fixtures for GH-115; synthetic measurements, no adapter certification."""
import copy
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import test_policy_engine as fixtures
from test_policy_engine import rule, ROOT
import harness_evidence as evidence
import policy_engine as engine
import project_model as model


class RatchetTests(unittest.TestCase):
    def setUp(self):
        fixture = fixtures.PolicyTests()
        fixture.setUp()
        self.records, self.context = fixture.records, fixture.context
        self.base = copy.deepcopy(self.records)
        self.base_context = copy.deepcopy(self.context)
        self.base_context['expected']['commit'] = self.context['expected']['base_commit']
        for record in self.base:
            record['context'] = copy.deepcopy(self.base_context['expected'])
            for artifact in record['artifacts']:
                artifact['context'] = copy.deepcopy(self.base_context['expected'])
        self.policy = {'schema': 'harness-policy/v1', 'rules': [rule(
            'risk.crap', {'type': 'decimal', 'value': '30'}, operator='le',
            ratchet={'deny_regression': True, 'allow_legacy_debt': True})]}
        self.mappings = {'schema': 'subject-mappings/v1', 'project': self.context['project']['id'],
                         'mappings': []}
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        sources = Path(self.tmp.name) / 'sources'
        shutil.copytree(self.context['source_root'], sources)
        self.context['source_root'] = sources

    def evaluate(self, **kwargs):
        options = {'base_records': self.base, 'base_context': self.base_context,
                   'mappings': self.mappings}
        options.update(kwargs)
        return engine.evaluate(self.policy, self.records, **self.context, **options)

    def values(self, base, head, index=0, metric='risk.crap', kind='decimal'):
        for records, value in ((self.base, base), (self.records, head)):
            next(m for m in records[index]['metrics'] if m['name'] == metric)['value'] = (
                {'type': kind, 'value': str(value) if kind == 'decimal' else value})

    def change(self, kind='modify', index=0):
        subject = self.records[index]['subject']
        before = copy.deepcopy(subject)
        if kind == 'modify':
            path = self.context['source_root'] / subject['path']
            path.write_bytes(path.read_bytes() + b'\n// synthetic edit\n')
            subject['source_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        elif kind == 'rename':
            subject['discriminator'] += '_renamed'
        elif kind == 'move':
            path = Path(subject['path'])
            subject['path'] = path.with_name('moved' + path.suffix).as_posix()
            shutil.copyfile(self.context['source_root'] / before['path'],
                            self.context['source_root'] / subject['path'])
        subject['id'] = model.subject_id(self.context['project']['id'], subject)
        self.records[index]['source'] = {'path': subject['path'], 'sha256': subject['source_sha256']}
        for artifact in self.records[index]['artifacts']:
            artifact['source'] = copy.deepcopy(self.records[index]['source'])
        self.context['project']['subjects'] = [copy.deepcopy(subject) if s['id'] == before['id']
                                               else s for s in self.context['project']['subjects']]
        for relationship in self.context['project']['relationships']:
            relationship['subjects'] = [subject['id'] if identity == before['id'] else identity
                                        for identity in relationship['subjects']]
        self.mappings['mappings'].append({'kind': kind, 'from': before['id'],
                                         'to': [subject['id']], 'reason': 'reviewed fixture lineage'})
        return before

    def add_subject(self, index=0):
        record = copy.deepcopy(self.records[index])
        record['id'] += '-new'
        record['subject']['discriminator'] += '_new'
        record['subject']['id'] = model.subject_id(self.context['project']['id'], record['subject'])
        self.context['project']['subjects'].append(copy.deepcopy(record['subject']))
        self.records.append(record)
        return record

    def test_improved_debt_and_new_failure_coexist_across_ecosystems(self):
        for index in range(4):
            self.values(64, 55, index)
            self.change(index=index)
        new = self.add_subject()
        next(m for m in new['metrics'] if m['name'] == 'risk.crap')['value']['value'] = '31'
        report = self.evaluate()
        self.assertEqual(report['aggregate']['state'], 'fail')
        self.assertEqual(len(report['debt_ledger']), 5)
        for child in report['results'][:4]:
            self.assertEqual(child['state'], 'informational')
            self.assertEqual(child['record']['baseline']['classification'], 'modified')
            self.assertEqual(child['record']['ratchet']['debt'], 'improved')
            self.assertFalse(child['record']['ratchet']['absolute_compliant'])
            self.assertEqual(child['record']['base']['value'], '64')
        self.assertEqual(report['results'][-1]['state'], 'fail')
        self.assertEqual(report['results'][-1]['record']['baseline']['classification'], 'new')
        # Canonical identity/series classification must not depend on input order.
        self.records.reverse()
        self.base.reverse()
        self.context['project']['subjects'].reverse()
        reordered = self.evaluate()
        self.assertEqual({r['subject']: r for r in report['results']},
                         {r['subject']: r for r in reordered['results']})
        Path(self.tmp.name, 'acceptance.json').write_text(json.dumps(report))

    def test_threshold_and_regression_are_independent(self):
        for base, head, state, debt, trend in [
                (64, 64, 'informational', 'unchanged', 'unchanged'),
                (64, 55, 'informational', 'improved', 'improved'),
                (64, 65, 'fail', 'regressed', 'regressed'),
                (18, 27, 'fail', 'none', 'regressed'),
                (31, 30, 'pass', 'resolved', 'improved'),
                (30, 31, 'fail', 'new', 'regressed')]:
            with self.subTest(base=base, head=head):
                self.values(base, head)
                child = self.evaluate()['results'][0]
                self.assertEqual(child['record']['baseline']['classification'], 'unchanged')
                self.assertEqual((child['state'], child['record']['ratchet']['debt'],
                                  child['record']['ratchet']['trend']), (state, debt, trend))
        self.policy['rules'][0]['ratchet']['deny_regression'] = False
        self.values(18, 27)
        self.assertEqual(self.evaluate()['results'][0]['state'], 'pass')
        self.values(64, 55)
        self.policy['rules'][0]['ratchet']['allow_legacy_debt'] = False
        self.assertEqual(self.evaluate()['results'][0]['state'], 'fail')

    def test_disabled_or_malformed_ratchet_cannot_silently_enable_incremental_pass(self):
        self.policy['rules'][0]['ratchet'] = {'deny_regression': True}
        with self.assertRaises(evidence.MeasurementError):
            self.evaluate()
        self.policy['rules'][0]['ratchet'] = {'deny_regression': True, 'allow_legacy_debt': 'yes'}
        with self.assertRaises(evidence.MeasurementError):
            self.evaluate()

    def test_missing_base_series_capabilities_and_provenance_fail_closed(self):
        for options in ({'base_records': None, 'base_context': None, 'mappings': None},
                        {'base_records': []}, {'base_context': None},
                        {'base_records': self.base[1:]}):
            with self.subTest(options=list(options)):
                self.assertEqual(self.evaluate(**options)['aggregate']['state'], 'measurement_error')
        original = copy.deepcopy(self.base)
        self.base[0]['series']['tool']['version'] += '-drift'
        self.base[0]['series']['id'] = evidence.series_id(self.base[0]['series'])
        self.assertEqual(self.evaluate()['aggregate']['state'], 'measurement_error')
        self.base = original
        for state in ('unsupported', 'not_configured', 'not_collected', 'measurement_error'):
            base = copy.deepcopy(original)
            next(c for c in base[0]['capabilities'] if c['metric'] == 'risk.crap')['state'] = state
            base[0]['metrics'] = [m for m in base[0]['metrics'] if m['name'] != 'risk.crap']
            if state == 'measurement_error':
                base[0]['status'] = state
            self.assertEqual(self.evaluate(base_records=base)['aggregate']['state'], 'measurement_error')
        for key in ('commit', 'target'):
            context = copy.deepcopy(self.base_context)
            context['expected'][key] = 'wrong'
            self.assertEqual(self.evaluate(base_context=context)['aggregate']['state'], 'measurement_error')
        context = copy.deepcopy(self.base_context)
        context['project']['id'] = 'another-project'
        self.assertEqual(self.evaluate(base_context=context)['aggregate']['state'], 'measurement_error')

    def test_explicit_lineage_and_splits_cannot_duplicate_debt_allowance(self):
        self.values(64, 55)
        self.change('rename')
        child = self.evaluate()['results'][0]
        self.assertEqual(child['state'], 'informational')
        self.assertEqual(child['record']['baseline']['classification'], 'modified')
        no_mapping = {**self.mappings, 'mappings': []}
        child = self.evaluate(mappings=no_mapping)['results'][0]
        self.assertEqual(child['state'], 'fail')
        self.assertIsNone(child['record']['base'])
        duplicate = copy.deepcopy(self.mappings)
        duplicate['mappings'] *= 2
        self.assertEqual(self.evaluate(mappings=duplicate)['aggregate']['state'], 'measurement_error')
        new = self.add_subject()
        self.mappings['mappings'][0].update(kind='split', to=[self.records[0]['subject']['id'],
                                                            new['subject']['id']])
        report = self.evaluate()
        for child in (report['results'][0], report['results'][-1]):
            self.assertEqual(child['state'], 'fail')
            self.assertIsNone(child['record']['base'])
            self.assertEqual(child['record']['baseline']['lineage_kind'], 'split')
            self.assertFalse(child['record']['baseline']['inherits_history'])

    def test_move_inheritance_and_invalid_modification_locator(self):
        self.values(64, 55)
        self.change('move')
        self.assertEqual(self.evaluate()['results'][0]['state'], 'informational')
        self.mappings['mappings'][0]['kind'] = 'modify'
        self.assertEqual(self.evaluate()['aggregate']['state'], 'measurement_error')

    def test_new_subject_requires_compatible_base_series(self):
        new = self.add_subject()
        new['series']['tool']['version'] += '-drift'
        new['series']['id'] = evidence.series_id(new['series'])
        child = self.evaluate()['results'][-1]
        self.assertEqual(child['state'], 'measurement_error')
        self.assertIn('compatible base series', child['reason'])

    def test_ratio_and_boolean_ratchets_use_exact_generic_comparisons(self):
        self.policy['rules'][0].update(metric='coverage.line', operator='ge',
                                      limit={'type': 'ratio', 'covered': 4, 'total': 5})
        next(m for m in self.records[0]['metrics'] if m['name'] == 'coverage.line')['value'] = {
            'type': 'ratio', 'covered': 3, 'total': 4}
        self.assertEqual(self.evaluate()['results'][0]['record']['ratchet']['trend'], 'regressed')
        self.policy['rules'][0].update(metric='contract.schema_valid', operator='eq',
                                      limit={'type': 'boolean', 'value': True})
        self.values(True, False, metric='contract.schema_valid', kind='boolean')
        self.assertEqual(self.evaluate()['results'][0]['state'], 'fail')

    def test_exceptions_preserve_failure_and_require_complete_unexpired_metadata(self):
        self.values(18, 31)
        exception = {'policy': 'risk.crap', 'subject': self.records[0]['subject']['id'],
                     'owner': 'team', 'issue': 'GH-115', 'reason': 'temporary review',
                     'expiry': '2026-10-01T00:00:00Z', 'compensating_control': 'manual check',
                     'approver': 'reviewer'}
        now = datetime(2026, 9, 8, tzinfo=timezone.utc)
        report = self.evaluate(exceptions=[exception], now=now)
        self.assertEqual(report['aggregate']['state'], 'fail')
        self.assertEqual(report['results'][0]['record']['exception_review']['state'], 'documented')
        invalid = []
        for key in ('owner', 'issue', 'reason', 'expiry', 'compensating_control'):
            item = dict(exception)
            del item[key]
            invalid.append([item])
        for expiry in ('2026-09-08T00:00:00Z', '2020-01-01T00:00:00Z', 'bad', '2026-10-01'):
            invalid.append([{**exception, 'expiry': expiry}])
        invalid.extend([[exception, exception], [{**exception, 'owner': ' '}],
                        [{**exception, 'policy': 'unknown'}], False])
        for items in invalid:
            with self.subTest(exceptions=items):
                report = self.evaluate(exceptions=items, now=now)
                self.assertEqual(report['aggregate']['state'], 'measurement_error')
                self.assertEqual(report['quality_aggregate']['state'], 'fail')
                self.assertEqual(report['results'][0]['state'], 'fail')

    def test_cli_accepts_separate_base_roots_and_serializes_ledger(self):
        self.values(64, 55)
        root = Path(self.tmp.name)
        documents = {'policy': self.policy, 'evidence': self.records,
                     'project': self.context['project'], 'expected': self.context['expected'],
                     'base-evidence': self.base, 'base-project': self.base_context['project'],
                     'base-expected': self.base_context['expected'], 'mappings': self.mappings}
        command = [sys.executable, str(ROOT / 'policy_engine.py'), '--reference-only']
        for name, document in documents.items():
            path = root / (name + '.json')
            path.write_text(json.dumps(document))
            command.extend(['--' + name, str(path)])
        for prefix, context in (('', self.context), ('base-', self.base_context)):
            for name in ('source_root', 'artifact_root'):
                command.extend(['--' + prefix + name.replace('_', '-'), str(context[name])])
        output = root / 'report.json'
        command.extend(['--output', str(output)])
        run = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(json.loads(output.read_text())['debt_ledger'][0]['record']['ratchet']['debt'],
                         'improved')


if __name__ == '__main__':
    unittest.main()
