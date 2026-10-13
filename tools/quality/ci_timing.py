#!/usr/bin/env python3
"""Normalize retained GitHub run/attempt job timestamps; no workflow execution."""
import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import statistics


def seconds(start, end):
    duration = (datetime.fromisoformat(end.replace('Z', '+00:00')) -
                datetime.fromisoformat(start.replace('Z', '+00:00'))).total_seconds()
    if duration < 0:
        raise ValueError('negative hosted duration')
    return duration


def category(name):
    if name.startswith('Install cargo-') or name == 'Install tarpaulin':
        return 'tool_install'
    if (name.startswith(('Upload ', 'Download ')) or name in (
            'Seal immutable quality evidence',
            'Verify artifact identity and hashes before consumption')):
        return 'artifact'
    if name == 'Collect and require fresh coverage, risk and matrix evidence':
        return 'quality_collection'
    if name.startswith(('Post ', 'Complete job')):
        return 'cleanup'
    if (name == 'Configure Cargo state' or
            name.startswith(('Set up job', 'Run actions/checkout@', 'Install ', 'Cache '))):
        return 'setup'
    return 'execution'


def normalize_job(job):
    result = {key: job[key] for key in ('id', 'name', 'conclusion', 'started_at', 'completed_at')}
    if job['conclusion'] == 'skipped':
        return {**result, 'os': None, 'wall_seconds': 0, 'steps': [], 'category_seconds': {}}
    os_names = {'ubuntu': 'Linux', 'macos': 'macOS', 'windows': 'Windows'}
    platforms = {os_names[label.split('-')[0]] for label in job['labels']
                 if label.split('-')[0] in os_names}
    if len(platforms) != 1:
        raise ValueError(f"ambiguous hosted OS for {job['name']}")
    steps = []
    totals = {}
    for step in job['steps']:
        elapsed = (0 if step['conclusion'] == 'skipped' else
                   seconds(step['started_at'], step['completed_at']))
        kind = category(step['name'])
        steps.append({**step, 'wall_seconds': elapsed, 'category': kind})
        totals[kind] = totals.get(kind, 0) + elapsed
    wall = seconds(job['started_at'], job['completed_at'])
    return {**result, 'os': platforms.pop(), 'wall_seconds': wall, 'steps': steps,
            'category_seconds': totals, 'unattributed_seconds': wall - sum(totals.values())}


def normalize_run(run, contract):
    if (run['event'], run['status'], run['conclusion']) != ('pull_request', 'completed', 'success'):
        raise ValueError('baseline requires successful completed PR runs')
    if run['collection_identity']['RUN_ID'] != f"{run['id']}-{run['run_attempt']}":
        raise ValueError('mixed collection attempt identity')
    if any(j['run_attempt'] != run['run_attempt'] for j in run['jobs']):
        raise ValueError('mixed job attempts')
    jobs = [normalize_job(job) for job in run['jobs']]
    required_names = [name for job in contract['jobs'].values()
                      if 'pull_request' in job['required_events'] for name in job['check_names']]
    by_name = {j['name']: j for j in jobs}
    if len(by_name) != len(jobs):
        raise ValueError('duplicate hosted job names')
    required = [by_name[name] for name in required_names]
    aggregate = by_name['Required Quality Aggregate']
    if any(j['conclusion'] != 'success' for j in [*required, aggregate]):
        raise ValueError('required hosted job did not succeed')
    last = max(required, key=lambda j: j['completed_at'])
    active = [j for j in jobs if j['conclusion'] != 'skipped']
    runner_minutes = {os: round(sum(j['wall_seconds'] for j in active if j['os'] == os) / 60, 4)
                      for os in ('Linux', 'macOS', 'Windows')}
    categories = {kind: sum(j['category_seconds'].get(kind, 0) for j in active)
                  for kind in ('setup', 'tool_install', 'quality_collection', 'execution',
                               'artifact', 'cleanup')}
    return {'run_id': run['id'], 'attempt': run['run_attempt'], 'url': run['html_url'],
            'event': run['event'], 'workflow_blob_sha': run['workflow_blob_sha'],
            'pr_head_sha': run['head_sha'], 'collection_identity': run['collection_identity'],
            'workflow_wall_seconds': seconds(run['run_started_at'],
                                             max(j['completed_at'] for j in active)),
            'api_lifecycle_seconds': seconds(run['created_at'], run['updated_at']),
            'critical_path_seconds': seconds(run['created_at'], aggregate['completed_at']),
            'last_required_child': last['name'],
            'aggregate_wait_seconds': seconds(run['created_at'], aggregate['started_at']),
            'aggregate_dispatch_gap_seconds': seconds(last['completed_at'], aggregate['started_at']),
            'aggregate_wall_seconds': aggregate['wall_seconds'],
            'runner_wall_minutes_by_os': runner_minutes, 'category_seconds': categories, 'jobs': jobs}


def normalize(source, contract):
    runs = [normalize_run(run, contract) for run in source['runs']]
    paths = [run['critical_path_seconds'] for run in runs]
    return {'schema_version': 1, 'normalization': 'ci_timing.py v1; see baseline.md',
            'critical_path_seconds': {'median': statistics.median(paths),
                                      'min': min(paths), 'max': max(paths)}, 'runs': runs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--contract', type=Path,
                        default=Path(__file__).parent / 'fixtures/ci-topology.json')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    raw = args.input.read_bytes()
    result = normalize(json.loads(raw), json.loads(args.contract.read_text()))
    result['input_sha256'] = hashlib.sha256(raw).hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
