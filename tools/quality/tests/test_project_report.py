import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import harness_evidence as evidence
import policy_engine as engine
import project_report

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parents[1]
FIXTURE = ROOT / 'fixtures/contracts'


class ProjectReportTests(unittest.TestCase):
    def setUp(self):
        self.project, self.policy = project_report.load_config(FIXTURE / 'project-config.json', REPO)
        self.records = evidence.load_json(FIXTURE / 'evidence.json')
        self.context = {'project': self.project, 'source_root': FIXTURE / 'sources',
                        'artifact_root': ROOT / 'fixtures',
                        'expected': evidence.load_json(FIXTURE / 'expected.json')}

    def evaluate(self):
        result = engine.evaluate(self.policy, self.records, **self.context)
        return project_report.report(result, self.project, self.policy)

    def test_local_green_contract_break_blocks_both_participants_and_project(self):
        report = self.evaluate()
        self.assertEqual(report['aggregate']['state'], 'fail')
        for component in ('api', 'frontend'):
            self.assertEqual(report['components'][component]['local']['state'], 'pass')
            self.assertEqual(report['components'][component]['cross_component']['state'], 'fail')
            self.assertEqual(report['components'][component]['aggregate']['state'], 'fail')
        for component in ('worker', 'billing'):
            self.assertEqual(report['components'][component]['aggregate']['state'], 'pass')
            self.assertEqual(report['components'][component]['cross_component']['state'], 'not_applicable')
        breaking = [g for g in report['gates'].values() if g['policy'] == 'contract.breaking_changes'][0]
        self.assertEqual(breaking['record']['head']['value'], 1)
        self.assertEqual(breaking['record']['relationship']['producer'], 'api')
        self.assertEqual(breaking['record']['relationship']['consumer'], 'frontend')
        self.assertEqual(breaking['record']['subject']['kind'], 'contract/v1')
        self.assertEqual(breaking['record']['contract'], self.records[-1]['contract'])

    def test_report_indexes_are_lossless_and_do_not_double_count_project_gates(self):
        report = self.evaluate()
        self.assertEqual(len(report['gates']), 8)
        self.assertEqual(len(report['aggregate']['blockers']), 3)
        for dimension in ('subject', 'policy', 'status'):
            refs = [i for group in report['indexes'][dimension].values() for i in group]
            self.assertCountEqual(refs, report['gates'])
        for gate in report['gates'].values():
            original = next(g for g in report['policy_result']['results'] if
                            (g['policy'], g['subject']) == (gate['policy'], gate['subject']))
            self.assertEqual(gate, original)
            self.assertTrue(gate['record']['evidence_links']['head']['artifacts'])

    def test_generic_contract_values_use_existing_typed_policy_comparison(self):
        # Policies over the same retained facts may ask for diagnostic conditions.
        # The collector's measured status never becomes a gate verdict.
        for rule in self.policy['rules'][4:]:
            rule['limit'] = next(m['value'] for m in self.records[-1]['metrics']
                                 if m['name'] == rule['metric'])
        self.assertEqual(self.evaluate()['aggregate']['state'], 'pass')
        self.policy['rules'][-1]['required'] = False
        self.policy['rules'][-1]['limit'] = {'type': 'boolean', 'value': True}
        self.assertEqual(self.evaluate()['aggregate']['state'], 'pass')

    def test_compatible_retained_contract_and_regenerated_client_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for record in self.records:
                for artifact in record['artifacts']:
                    path = root / artifact['path']
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes((self.context['artifact_root'] / artifact['path']).read_bytes())
            self.context['artifact_root'] = root
            contract = self.records[-1]
            values = {r['metric']: r['limit'] for r in self.policy['rules'][4:]}
            for metric in contract['metrics']:
                metric['value'] = values[metric['name']]
            head = json.loads((FIXTURE / 'sources/api/src/openapi.json').read_text())
            replacements = {'baseline': head, 'consumer': {'required_response_fields': ['primaryEmail']},
                            'client': {'generated_fields': ['primaryEmail'],
                                       'generator': {'name': 'synthetic-generator', 'version': '1'}},
                            'facts': {'synthetic': True, 'values': values, 'change': 'unchanged contract'}}
            for artifact in contract['artifacts']:
                if artifact['id'] in replacements:
                    data = (json.dumps(replacements[artifact['id']], indent=2) + '\n').encode()
                    (root / artifact['path']).write_bytes(data)
                    artifact.update(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
            contract['contract']['generated_client']['contract_sha256'] = contract['source']['sha256']
            self.assertEqual(self.evaluate()['aggregate']['state'], 'pass')

    def test_one_contract_can_report_separate_consumer_relationships(self):
        link = copy.deepcopy(self.project['relationships'][0])
        link.update(id='api-worker-contract', consumer='worker', subjects=[self.records[-1]['subject']['id']])
        self.project['relationships'].append(link)
        record = copy.deepcopy(self.records[-1])
        record['id'] = 'synthetic-worker-contract'
        record['series']['name'] = 'synthetic-api-worker'
        record['series']['id'] = evidence.series_id(record['series'])
        record['contract'].update(relationship=link['id'], consumer='worker')
        record['contract']['baseline']['series_id'] = record['series']['id']
        self.records.append(record)
        for rule in copy.deepcopy(self.policy['rules'][4:]):
            rule['id'] = 'worker.' + rule['id']
            rule['scope']['relationship'] = link['id']
            self.policy['rules'].append(rule)
        report = self.evaluate()
        self.assertEqual(len(report['gates']), 12)
        self.assertEqual(len(report['indexes']['relationship'][link['id']]), 4)
        self.assertEqual(report['components']['worker']['local']['state'], 'pass')
        self.assertEqual(report['components']['worker']['cross_component']['state'], 'fail')

    def test_missing_and_mismatched_contract_provenance_blocks(self):
        original = copy.deepcopy(self.records[-1])
        mutations = [
            ('contract.breaking_changes', lambda r: r.pop('contract'), 'missing contract provenance'),
            ('contract.breaking_changes', lambda r: r['contract'].pop('baseline'), 'missing contract baseline'),
            ('contract.breaking_changes', lambda r: r['contract']['baseline'].update(commit='f'*40), 'stale contract baseline'),
            ('contract.breaking_changes', lambda r: r['contract']['baseline'].update(series_id='measurement-series/v1:'+'f'*64), 'incompatible contract baseline'),
            ('contract.breaking_changes', lambda r: r['contract'].update(consumer='worker'), 'participants mismatch'),
            ('contract.schema_valid', lambda r: r['contract'].update(contract_artifact='facts'), 'digest mismatch'),
            ('contract.compatible', lambda r: r['contract'].pop('consumer_artifact'), 'missing consumer expectation'),
            ('contract.compatible', lambda r: r['contract'].update(consumer_artifact='unknown'), 'missing required contract artifact'),
            ('contract.client_drift', lambda r: r['contract'].pop('generated_client'), 'missing generated-client evidence'),
            ('contract.client_drift', lambda r: r['contract']['generated_client'].update(contract_sha256=r['source']['sha256']), 'contradicts contract digest'),
        ]
        for policy_id, mutate, reason in mutations:
            with self.subTest(reason=reason):
                self.records[-1] = copy.deepcopy(original)
                mutate(self.records[-1])
                report = self.evaluate()
                gate = next(g for g in report['gates'].values() if g['policy'] == policy_id)
                self.assertEqual(gate['state'], 'measurement_error')
                self.assertIn(reason, gate['reason'])
                self.assertEqual(report['aggregate']['state'], 'measurement_error')
                self.assertEqual(report['components']['api']['local']['state'], 'pass')

    def test_missing_unavailable_and_tampered_contract_evidence_never_passes(self):
        original = copy.deepcopy(self.records)
        self.records.pop()
        report = self.evaluate()
        self.assertEqual(report['aggregate']['state'], 'measurement_error')
        self.assertEqual(len(report['indexes']['relationship']['api-client']), 4)
        for state in ('unsupported', 'not_collected', 'not_configured', 'measurement_error', 'not_applicable'):
            self.records = copy.deepcopy(original)
            record = self.records[-1]
            record['capabilities'][0]['state'] = state
            record['metrics'] = record['metrics'][1:]
            if state == 'measurement_error':
                record['status'] = state
            self.assertNotEqual(self.evaluate()['aggregate']['state'], 'pass')
        self.records = original
        self.records[-1]['artifacts'][0]['sha256'] = 'f' * 64
        report = self.evaluate()
        self.assertEqual(report['aggregate']['state'], 'measurement_error')
        self.assertEqual(report['components']['api']['local']['state'], 'measurement_error')

    def test_policy_requires_explicit_valid_contract_and_subject(self):
        for scope in ({'kind': 'relationship', 'relationship': 'missing'},
                      {'kind': 'relationship', 'relationship': 'work-queue'},
                      {'kind': 'subject', 'subject': 'subject-identity/v1:' + 'f'*64}):
            with self.subTest(scope=scope):
                self.policy['rules'][4]['scope'] = scope
                with self.assertRaises(evidence.MeasurementError):
                    self.evaluate()

    def test_target_mismatch_keeps_contract_error_in_participant_indexes(self):
        self.context['expected']['target'] = 'other'
        report = self.evaluate()
        self.assertEqual(report['aggregate']['state'], 'measurement_error')
        self.assertEqual(len(report['indexes']['relationship']['api-client']), 4)

    def test_cli_single_rust_and_polyglot_replays(self):
        with tempfile.TemporaryDirectory() as directory:
            for prefix, code, state in [('single-rust-', 0, 'pass'), ('', 1, 'fail')]:
                output = Path(directory) / 'report.json'
                config = FIXTURE / (prefix + 'config.json' if prefix else 'project-config.json')
                command = [sys.executable, str(ROOT / 'project_report.py'), '--reference-only', 'evaluate',
                           '--config', str(config), '--root', str(REPO),
                           '--evidence', str(FIXTURE / (prefix + 'evidence.json')),
                           '--expected', str(FIXTURE / 'expected.json'),
                           '--source-root', str(FIXTURE / 'sources'),
                           '--artifact-root', str(ROOT / 'fixtures'), '--output', str(output)]
                result = subprocess.run(command, cwd=directory, capture_output=True, text=True)
                self.assertEqual(result.returncode, code, result.stderr)
                report = json.loads(output.read_text())
                self.assertEqual(report['aggregate']['state'], state)
                self.assertEqual(len(report['components']), 1 if prefix else 4)

    def test_config_is_opt_in_and_legacy_flow_is_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '.harness-gate').mkdir()
            flow = root / '.harness-gate/flow.toml'
            legacy = b'version = "1"\n[project]\nname = "rust-project"\n'
            flow.write_bytes(legacy)
            output = root / 'report.json'
            command = [sys.executable, str(ROOT / 'project_report.py'), '--reference-only', 'check', '--output', str(output)]
            result = subprocess.run(command, cwd=root, capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn('project.json', json.loads(output.read_text())['error'])
            self.assertEqual(flow.read_bytes(), legacy)
            result = subprocess.run(command + ['--config', str(FIXTURE / 'single-rust-config.json'),
                                               '--root', str(REPO)], cwd=root, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(flow.read_bytes(), legacy)

    def test_config_rejects_unknown_versions_fields_and_escaping_references(self):
        config = evidence.load_json(FIXTURE / 'project-config.json')
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'config.json'
            for change in ({'schema': 'harness-project-config/v2'}, {'default_path': 'src'},
                           {'project_model': '../outside.json'}, {'policy': '/tmp/policy.json'}):
                path.write_text(json.dumps({**config, **change}))
                with self.assertRaises((evidence.MeasurementError, ValueError)):
                    project_report.load_config(path, REPO)
            link = Path(directory) / 'escape.json'
            link.symlink_to(FIXTURE / 'project.json')
            path.write_text(json.dumps({**config, 'project_model': 'escape.json'}))
            with self.assertRaisesRegex(evidence.MeasurementError, 'escapes project root'):
                project_report.load_config(path, directory)


if __name__ == '__main__':
    unittest.main()
