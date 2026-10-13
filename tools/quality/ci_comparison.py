#!/usr/bin/env python3
"""Summarize retained hosted timings without treating overlapping runs as causal proof."""

import argparse
import hashlib
import json
import re
import statistics
from pathlib import Path


def distribution(values):
    return {'median': statistics.median(values), 'min': min(values), 'max': max(values)}


def summarize(runs):
    active = {job['name'] for run in runs for job in run['jobs']
              if job['conclusion'] != 'skipped'}
    return {
        'run_ids': [run['run_id'] for run in runs],
        'critical_path_seconds': distribution([r['critical_path_seconds'] for r in runs]),
        'category_seconds': {key: distribution([r['category_seconds'][key] for r in runs])
                             for key in runs[0]['category_seconds']},
        'runner_wall_minutes_by_os': {
            key: distribution([r['runner_wall_minutes_by_os'][key] for r in runs])
            for key in runs[0]['runner_wall_minutes_by_os']},
        'total_runner_wall_minutes': distribution([
            sum(j['wall_seconds'] for j in r['jobs']) / 60 for r in runs]),
        'job_wall_seconds': {name: distribution([
            next(j['wall_seconds'] for j in r['jobs'] if j['name'] == name) for r in runs])
            for name in sorted(active)},
    }


def cache_audit(run):
    result = []
    for job in run['jobs']:
        lines = job['excerpt']
        target = [line for line in lines if 'cargo-target-v1-' in line]
        nested = lambda suffix: sum(int(match[1]) for line in lines
                                    if (match := re.search(
                                        re.escape(suffix) + r';.*duration_ms=(\d+)\]', line))) / 1000
        result.append({
            'job_name': job['job_name'],
            'target_cache_hits': sum('Cache restored from key:' in line for line in target),
            'target_cache_misses': sum('Cache not found for input keys:' in line for line in target),
            'source_cache_hits': sum('Cache restored from key: cargo-sources-v1-' in line for line in lines),
            'target_cache_restore_and_save_seconds': nested('.__actions_cache_2'),
            'nested_boundary_upload_seconds': nested('.__actions_upload-artifact'),
            'uploaded_zip_bytes': sum(int(m[1]) for line in lines
                                      if (m := re.search(r'Final size is (\d+) bytes', line))),
        })
    return {'run_id': run['run_id'], 'jobs': result}


def compare(directory):
    names = ('hosted-baseline.json', 'hosted-after.json', 'hosted-stages.json',
             'hosted-after-log-audit.json')
    raw = {name: (directory / name).read_bytes() for name in names}
    data = {name: json.loads(value) for name, value in raw.items()}
    return {
        'schema_version': 1,
        'input_sha256': {name: hashlib.sha256(value).hexdigest() for name, value in raw.items()},
        'before': summarize(data[names[0]]['runs']),
        'after': summarize(data[names[1]]['runs']),
        'stage_diagnostics_excluded_from_comparison': summarize(data[names[2]]['runs']),
        'supplemental_log_audit': [cache_audit(run) for run in data[names[3]]['runs']],
        'limitations': [
            'Two after runs; changing commits, hosted contention and compiler versions are not controlled.',
            'Nested cache/upload timings overlap normalized setup/cleanup; never add them to runner time.',
            'Compiled-cache rollback and restored docs Cargo calls are not timed by this cohort.',
            'No causal critical-path or total-runner reduction is established; see after-state.md.',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, default=Path('docs/quality/ci-topology'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(compare(args.directory), indent=2) + '\n')


if __name__ == '__main__':
    main()
