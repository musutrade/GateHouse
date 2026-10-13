#!/usr/bin/env python3
"""Check retained negative evidence, including semantics after artifact rehashing."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ERRORS = {
    'crash': 'ADAPTER_PROTOCOL_FAILURE: adapter exited with 17',
    'missing-evidence': 'missing or unexpected subject/capability/series',
    'tamper-artifact': 'artifact/source digest mismatch',
    'stale-context': 'stale commit/base/target/run evidence',
    'stale-baseline': 'baseline source/target identity mismatch',
    'incompatible-baseline': 'incompatible baseline profile/tool/measurement series',
}


def verify(root=ROOT, source_root=None):
    source_root = root.parent if source_root is None else source_root
    manifest = json.loads((root / 'manifest.json').read_text())
    assert set(manifest['artifacts']) == {'evidence/commands.json', 'evidence/quality.json'}
    for name, sha in manifest['artifacts'].items():
        assert hashlib.sha256((root / name).read_bytes()).hexdigest() == sha, name
    commands = json.loads((root / 'evidence/commands.json').read_text())
    assert commands['frozen_sources_unchanged'] and commands['disposable_fixtures_removed']
    for name, sha in commands['frozen_source_hashes'].items():
        assert hashlib.sha256((source_root / name).read_bytes()).hexdigest() == sha, name
    ids = {'frontend.e2e', 'backend.tests', 'frontend.fullstack-smoke'}
    assert len(commands['cases']) == 6
    assert {c['id'] for c in commands['cases']} == {f'{i}-{s}' for i in ids for s in [0, 7]}
    for case in commands['cases']:
        passed = case['injected_exit_code'] == 0
        assert case['expected_exit_code'] == case['receipt']['exit_code'] == (0 if passed else 1)
        assert case['config_check']['exit_code'] == 0 and case['source_unchanged']
        assert case['report']['passed'] is passed and case['report']['evidence_complete']
        steps = [s for s in case['report']['steps'] if s['step_id'] == case['original_step']['id']]
        assert len(steps) == 1 and steps[0]['step_id'] == case['original_step']['id']
        assert steps[0]['passed'] is passed
        assert 'controlled-project-command' in case['log']
        assert case['absent_ci']['exit_code'] == 1
        assert 'E1401' in case['absent_ci']['stderr']
    quality = json.loads((root / 'evidence/quality.json').read_text())
    assert quality['measurement_origin'] == 'synthetic signed transport fixtures'
    expected = {name: ('blocked', False) for name in ERRORS}
    expected.update({'control': ('pass', True), 'crap-ratchet': ('fail', False),
                     'rust-pass': ('pass', True), 'rust-policy-failure': ('fail', False),
                     'angular-reference-pass': ('pass', True)})
    for profile in ['hook', 'full', 'ci']:
        expected[f'{profile}-omitted'] = ('not_collected', True)
        expected[f'{profile}-required-not-collected'] = ('fail', False)
    assert len(quality['cases']) == len(expected)
    assert {c['id'] for c in quality['cases']} == set(expected)
    for case in quality['cases']:
        name = case['id']
        status, passed = expected[name]
        q = case['report']['quality']
        assert (case['expected_quality_status'], case['expected_workflow_passed']) == (status, passed)
        assert q['status'] == status and case['report']['passed'] is passed, name
        assert case['source_unchanged'] and case['baseline_request_unchanged']
        if name in ERRORS:
            assert ERRORS[name] in q['error'], name
            assert q.get('project_report') is None
        if name.endswith('-omitted'):
            assert q['full_quality_status'] == 'not_collected'
            assert q['evidence'] == [] and q['producers'] == {}
        if name == 'crap-ratchet':
            gate = next(iter(q['project_report']['gates'].values()))
            r = gate['record']
            assert gate['state'] == 'fail' and r['metric'] == 'risk.crap'
            assert r['head']['value'] == '40' and r['base']['value'] == '35'
            assert r['policy']['limit']['value'] == '30'
            assert r['ratchet']['debt'] == 'regressed' and not r['ratchet']['legacy_debt_allowed']
            assert r['measurement_series']['base'] == r['measurement_series']['head']
        if name == 'rust-policy-failure':
            gates = list(q['project_report']['gates'].values())
            assert any(g['state'] == 'fail' and g['record']['metric'] == 'risk.crap' for g in gates)
            assert all(g['state'] == 'pass' for g in gates if g['record']['metric'] != 'risk.crap')
        if name == 'angular-reference-pass':
            assert q['evidence']
            for record in q['evidence']:
                assert any(c['metric'] == 'risk.crap' and c['state'] == 'unsupported'
                           for c in record['capabilities'])
                assert all(m['name'] != 'risk.crap' for m in record['metrics'])
    return {'commands': len(commands['cases']), 'quality': len(quality['cases']),
            'application_runtime_parity': 'not established', 'result': 'PASS'}


if __name__ == '__main__':
    print(json.dumps(verify(), indent=2))
