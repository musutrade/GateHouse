#!/usr/bin/env python3
"""Verify the retained local parity and controlled-negative rerun receipts."""
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ARC = ROOT.parent
SPEC = importlib.util.spec_from_file_location('arc_negative', ARC / 'negative/reproduce.py')
negative = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(negative)


def verify(root=ROOT, source_root=ARC):
    manifest = json.loads((root / 'manifest.json').read_text())
    required = {'evidence/receipt.json'} | {
        f'evidence/{name}/{file}' for name in ['arc', 'execution', 'quality']
        for file in ['test_result.json', 'test_result.md']
    } | {f'evidence/{name}-{mode}.log' for name in ['arc', 'execution', 'quality']
         for mode in ['scope', 'full']}
    assert required <= set(manifest['artifacts'])
    for name, sha in manifest['artifacts'].items():
        assert hashlib.sha256((root / name).read_bytes()).hexdigest() == sha, name
    receipt = json.loads((root / 'evidence/receipt.json').read_text())
    inventory = json.loads((source_root / 'inventory.json').read_text())
    assert receipt['source_sha'] == inventory['commit']
    assert receipt['tracked_source_unchanged']
    assert not receipt['authority_transfer_permitted']
    assert receipt['reports_present'] == dict.fromkeys(['arc', 'execution', 'quality'], True)
    sources = {
        '.arc-flow/flow.toml': 'sources/.arc-flow/flow.toml.txt',
        '.shadow-execution/flow.toml': 'import/flow.toml',
        '.harness-gate/flow.toml': 'import/flow.toml',
        '.harness-gate/quality.toml': 'quality/quality.toml',
    }
    for path in (source_root / 'quality/packs').rglob('*.json'):
        relative = path.relative_to(source_root / 'quality')
        sources[str(Path('.harness-gate') / relative)] = str(Path('quality') / relative)
    assert set(sources) | {'.arc-flow/.gitignore'} == set(receipt['configuration_hashes'])
    assert receipt['configuration_hashes']['.arc-flow/.gitignore'] == hashlib.sha256(b'reports/\n').hexdigest()
    for path, frozen in sources.items():
        assert receipt['configuration_hashes'][path] == hashlib.sha256(
            (source_root / frozen).read_bytes()).hexdigest(), path
    expected = [(f'{name}-{mode}', 1 if name != 'arc' and mode == 'full' else 0)
                for name in ['arc', 'execution', 'quality'] for mode in ['scope', 'full']]
    assert [(c['name'], c['exit_code']) for c in receipt['commands']] == expected
    for command in receipt['commands']:
        assert command['elapsed_seconds'] > 0
        suffix = ['scope', '--all'] if command['name'].endswith('-scope') else [
            'verify', '--profile', 'full', '--all']
        assert command['argv'][-len(suffix):] == suffix
    labels = inventory['always_blocking_prelude'] + [
        s['label'] for s in inventory['flow']['steps'] if 'full' in s['profiles']]
    assert len(labels) == 27
    for name in ['arc', 'execution', 'quality']:
        report = json.loads((root / f'evidence/{name}/test_result.json').read_text())
        assert report['profile'] == 'full'
        assert report['scope'] == {'mode': 'all', 'changed_files': [],
                                   'components': ['backend', 'frontend', 'workflow'], 'unmatched_files': []}
        assert [s['label'] for s in report['steps']] == labels, name
        assert all(s['passed'] and not s['timed_out'] and not s['cancelled']
                   for s in report['steps']), name
        assert report['passed'] is (name == 'arc'), name
        if name == 'arc':
            continue
        assert report['evidence_complete'] and not report['skipped_steps']
        assert [s['step_id'] for s in report['steps']] == [
            'builtin.secret-scan', 'builtin.architecture-audit', *inventory['profile_steps']['full']]
        if name in ['execution', 'quality']:
            quality = report['quality']
            assert quality['participation']['profile'] == 'full'
            assert quality['status'] == quality['full_quality_status'] == 'blocked'
            assert quality['phase'] == 'configuration' and quality['project_report'] is None
            assert 'quality workflow input is missing:' in quality['error']
            assert quality['error'].endswith('/.harness-gate/runtime/full-state.json')
            assert 'Quality profile "full": blocked' in (root / f'evidence/{name}-full.log').read_text()
    controls = negative.verify(root / 'negative', source_root=source_root)
    return {'result': 'PASS', 'application_results_per_engine': 27,
            'application_command_parity': 'PASS', 'native_quality': 'blocked; tracked in GH-215',
            'controlled_commands': controls['commands'], 'controlled_quality': controls['quality'],
            'authority_transfer_permitted': False}


if __name__ == '__main__':
    print(json.dumps(verify(), indent=2))
