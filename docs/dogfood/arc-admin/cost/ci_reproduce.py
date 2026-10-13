#!/usr/bin/env python3
"""Validate retained Actions identity and cost receipts without rerunning work."""
import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def read(root, path):
    return json.loads((root / path).read_text())


def seconds(start, end):
    result = (datetime.fromisoformat(end.replace('Z', '+00:00')) -
              datetime.fromisoformat(start.replace('Z', '+00:00'))).total_seconds()
    assert result >= 0, 'negative duration'
    return result


def derive(root):
    manifest = read(root, 'sha256.json')
    assert set(manifest) == {str(p.relative_to(root)) for p in root.rglob('*') if p.is_file() and p != root / 'sha256.json'}, 'incomplete artifact manifest'
    for relative, digest in manifest.items():
        path = (root / relative).resolve()
        assert path.is_relative_to(root.resolve()), 'artifact escape'
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, f'modified artifact: {relative}'
    context = read(root, 'context.json')
    run = read(root, 'actions-run.json')
    jobs = read(root, 'actions-jobs.json')['jobs']
    job = next(j for j in jobs if j['name'] == 'measure')
    identity = context['ci_identity']
    assert str(run['id']) == identity['GITHUB_RUN_ID']
    assert str(run['run_attempt']) == identity['GITHUB_RUN_ATTEMPT']
    assert run['head_sha'] == context['harness_sha'] == identity['GITHUB_SHA']
    assert run['repository']['full_name'] == identity['GITHUB_REPOSITORY'] == 'musutrade/Harness-Gate'
    assert job['run_id'] == run['id'] and job['runner_name'] == identity['RUNNER_NAME']
    assert job['labels'] == ['gh206-measure'] and job['runner_id'] > 0
    assert context['source_sha'] == json.loads((HERE.parent / 'source-manifest.json').read_text())['commit']
    summary = read(root, 'summary.json')
    assert summary['authority_transfer_permitted'] is False
    rows = []
    for pair in summary['pairs']:
        sample = pair['sample']
        timings = {}
        for mode in ('before', 'shadow'):
            path = f'{sample}-{mode}/sample.json'
            assert path in manifest
            receipt = read(root, path)
            assert receipt['sample'] == sample and receipt['mode'] == mode
            commands = receipt['commands']
            assert [c['name'] for c in commands] == (['arc-full'] if mode == 'before' else ['arc-full', 'harness-full'])
            for command in commands:
                retained = read(root, f'{sample}-{mode}/{command["name"]}.json')
                assert retained == command and command['elapsed_seconds'] >= 0
                assert isinstance(command['exit_code'], int)
            assert receipt['passed'] == all(c['exit_code'] == 0 for c in commands)
            assert receipt['elapsed_seconds'] >= sum(c['elapsed_seconds'] for c in commands)
            step = next(s for s in job['steps'] if s['name'] == f'{mode.title()} sample {sample}')
            assert step['status'] == 'completed'
            timings[mode] = {'segment_seconds': receipt['elapsed_seconds'],
                             'actions_step_seconds': seconds(step['started_at'], step['completed_at']),
                             'commands_passed': receipt['passed']}
            assert pair[f'{mode}_seconds'] == receipt['elapsed_seconds']
        assert pair['added_seconds'] == timings['shadow']['segment_seconds'] - timings['before']['segment_seconds']
        native = read(root, f'{sample}-shadow/arc-reports/test_result.json')
        harness = read(root, f'{sample}-shadow/harness-reports/test_result.json')
        assert harness['source_identity'] == 'working-tree:' + context['source_sha']
        assert native['profile'] == harness['profile'] == 'full'
        assert native['scope']['mode'] == harness['scope']['mode'] == 'all'
        inventory = json.loads((HERE.parent / 'inventory.json').read_text())
        expected = {step['label'] for step in inventory['flow']['steps']} | set(inventory['always_blocking_prelude'])
        native_steps = {step['label']: step['passed'] for step in native['steps']}
        harness_steps = {step['label']: step['passed'] for step in harness['steps']}
        assert set(native_steps) == set(harness_steps) == expected
        assert len(native['steps']) == len(harness['steps']) == len(expected)
        rows.append({'sample': sample, **timings, 'added_segment_seconds': pair['added_seconds'],
                     'traditional_checks': len(expected), 'traditional_results_match': native_steps == harness_steps,
                     'traditional_checks_all_passed': all(native_steps.values()) and all(harness_steps.values()),
                     'quality_status': harness['quality']['status'], 'quality_error': harness['quality']['error'],
                     'harness_failures': harness['failures']})
    assert sorted(row['sample'] for row in rows) == [1, 2, 3]
    return {'source_sha': context['source_sha'], 'harness_sha': context['harness_sha'],
            'run_url': run['html_url'], 'job_url': job['html_url'], 'samples': rows,
            'job_runner_seconds': seconds(job['started_at'], job['completed_at']),
            'initial_wait_seconds': seconds(run['created_at'], job['started_at']),
            'authority_transfer_permitted': False, 'quality_acceptance': 'not established',
            'scope': context['workflow_host'], 'cache_protocol': context['cache_protocol']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    result = derive(HERE / 'selfhosted')
    report = HERE / 'selfhosted-report.json'
    if args.write:
        report.write_text(json.dumps(result, indent=2) + '\n')
    else:
        assert json.loads(report.read_text()) == result, 'stale cost report'
    print('Self-hosted trial receipts verified; authority transfer remains blocked.')


if __name__ == '__main__':
    main()
