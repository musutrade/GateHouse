#!/usr/bin/env python3
"""Collect bounded self-hosted trials; successful collection is not gate acceptance."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
ARC = ROOT / 'docs/dogfood/arc-admin'
SOURCE_SHA = json.loads((ARC / 'source-manifest.json').read_text())['commit']
IDENTITY_KEYS = ('GITHUB_REPOSITORY', 'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT',
                 'GITHUB_JOB', 'GITHUB_SHA', 'RUNNER_NAME', 'RUNNER_OS', 'RUNNER_ARCH')
REMOVED = ('DATABASE_URL', 'TEST_DATABASE_URL', 'ARC_FLOW_CONFIG', 'ARC_FLOW_REPORTS',
           'ARC_FLOW_AUDIT_CONFIG', 'ARC_FLOW_SECRETS_CONFIG', 'AUDITOR_CONFIG',
           'REPORT_DIR', 'PROJECT_ROOT', 'LLVM_PROFILE_FILE', 'RUSTFLAGS')


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def run(directory, name, argv, cwd, env):
    started = datetime.now(timezone.utc).isoformat()
    tick = time.monotonic_ns()
    with (directory / f'{name}.log').open('wb') as log:
        result = subprocess.run(argv, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
    receipt = {'name': name, 'argv': argv, 'started_at': started,
               'completed_at': datetime.now(timezone.utc).isoformat(),
               'elapsed_seconds': (time.monotonic_ns() - tick) / 1e9,
               'exit_code': result.returncode}
    write(directory / f'{name}.json', receipt)
    print(json.dumps(receipt), flush=True)
    return receipt


def preserve_reports(source, destination):
    reports = source / 'codex-audit-pipeline/.codex/reports'
    if reports.exists():
        shutil.copytree(reports, destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('prepare', 'before', 'shadow', 'summarize'))
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--sample', type=int, choices=(1, 2, 3))
    args = parser.parse_args()
    root = args.output.resolve()
    if not root.is_relative_to((ROOT / 'target').resolve()):
        parser.error('output must be in workspace target/')
    env = {k: v for k, v in os.environ.items() if k not in REMOVED}
    env.update(CARGO_TARGET_DIR=str(root / 'cargo-target'), TMPDIR=str(root / 'tmp'),
               npm_config_cache=str(root / 'npm-cache'),
               GIT_CEILING_DIRECTORIES=str(root / 'tmp'))
    source = root / 'source'
    if args.mode == 'prepare':
        root.mkdir(parents=True, exist_ok=False)
        (root / 'tmp').mkdir()
        evidence = root / 'evidence'
        evidence.mkdir()
        context = {'source_sha': SOURCE_SHA, 'harness_sha': subprocess.check_output(
            ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'ci_identity': {k: os.environ.get(k) for k in IDENTITY_KEYS},
            'host': {'machine': os.uname().machine, 'kernel': os.uname().release,
                     'logical_cpus': os.cpu_count()},
            'cache_protocol': 'Fresh workspace-local Cargo and npm caches at setup. All six sequential samples reuse those caches; sample 1 is initial-use, samples 2/3 are warm. No cold-cache or steady-state savings claim.',
            'quality_limit': 'Missing trusted native producers/state/baseline must remain failed. No synthetic inputs or baseline substitution.',
            'setup_excluded_from_samples': True,
            'workflow_host': 'Harness-Gate measurement workflow; workload is the pinned Arc-Admin source, not its original multijob CI topology.'}
        write(evidence / 'context.json', context)
        source.mkdir()
        commands = [('git-init', ['git', 'init', '-q']),
                    ('git-fetch', ['git', 'fetch', '--depth=1', 'https://github.com/musutrade/arc-admin.git', SOURCE_SHA]),
                    ('git-checkout', ['git', 'checkout', '--detach', 'FETCH_HEAD'])]
        for name, command in commands:
            if run(evidence, name, command, source, env)['exit_code']:
                raise SystemExit(f'failed setup: {name}')
        expected_node = (source / '.node-version').read_text().strip().removeprefix('v')
        actual_node = subprocess.check_output(['node', '--version'], text=True).strip().removeprefix('v')
        if actual_node != expected_node:
            raise SystemExit(f'Node version mismatch: required {expected_node}, got {actual_node}')
        for folder in ('.shadow-execution', '.harness-gate'):
            (source / folder).mkdir()
            shutil.copy2(ARC / 'import/flow.toml', source / folder / 'flow.toml')
        shutil.copy2(ARC / 'quality/quality.toml', source / '.harness-gate/quality.toml')
        shutil.copytree(ARC / 'quality/packs', source / '.harness-gate/packs')
        for name, command in [('npm-ci', ['npm', 'ci', '--prefix', 'frontend']),
                              ('scope', ['cargo', 'flow', 'scope', '--all'])]:
            if run(evidence, name, command, source, env)['exit_code']:
                raise SystemExit(f'failed setup: {name}')
        for name, command in [('rustc', ['rustc', '-vV']), ('node', ['node', '--version']),
                              ('npm', ['npm', '--version']), ('docker', ['docker', 'version', '--format', '{{.Server.Version}}'])]:
            run(evidence, name, command, source, env)
        write(evidence / 'configuration-hashes.json', {
            str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest()
            for folder in ('.arc-flow', '.harness-gate', '.shadow-execution')
            for p in (source / folder).rglob('*') if p.is_file()})
        return
    evidence = root / 'evidence'
    if args.mode == 'summarize':
        samples = [json.loads(p.read_text()) for p in sorted(evidence.glob('*/sample.json'))]
        assert len(samples) == 6, 'all before/shadow receipts required'
        pairs = []
        for number in (1, 2, 3):
            before = next(s for s in samples if s['mode'] == 'before' and s['sample'] == number)
            shadow = next(s for s in samples if s['mode'] == 'shadow' and s['sample'] == number)
            pairs.append({'sample': number, 'before_seconds': before['elapsed_seconds'],
                          'shadow_seconds': shadow['elapsed_seconds'],
                          'added_seconds': shadow['elapsed_seconds'] - before['elapsed_seconds'],
                          'before_passed': all(c['exit_code'] == 0 for c in before['commands']),
                          'shadow_passed': all(c['exit_code'] == 0 for c in shadow['commands'])})
        write(evidence / 'summary.json', {'pairs': pairs, 'samples': samples,
            'authority_transfer_permitted': False,
            'scope': 'Sequential full-command workloads, excludes setup and job overhead. Same runner, source, scope and configurations; shared warm caches introduce order effects. Failed trials are not complete-quality cost or runtime assurance parity.',
            'runner_work': 'One job without overlapping samples: each sample elapsed time is occupied runner time for that segment. Obtain full job/step timing separately from Actions API; do not substitute these segments for the original multijob topology.'})
        return
    if args.sample is None:
        parser.error('--sample required for a trial')
    directory = evidence / f'{args.sample}-{args.mode}'
    directory.mkdir(exist_ok=False)
    subprocess.run(['git', 'diff', '--exit-code', SOURCE_SHA], cwd=source, check=True)
    tick = time.monotonic_ns()
    commands = [run(directory, 'arc-full', ['cargo', 'flow', 'verify', '--profile', 'full', '--all'], source, env)]
    preserve_reports(source, directory / 'arc-reports')
    if args.mode == 'shadow':
        binary = ROOT / 'target/harness-build/debug/harness-gate'
        commands.append(run(directory, 'harness-full', [str(binary), '--project-root', str(source),
            '--config', '.harness-gate/flow.toml', 'verify', '--profile', 'full', '--all'], source, env))
        preserve_reports(source, directory / 'harness-reports')
    elapsed = (time.monotonic_ns() - tick) / 1e9
    subprocess.run(['git', 'diff', '--exit-code', SOURCE_SHA], cwd=source, check=True)
    write(directory / 'sample.json', {'mode': args.mode, 'sample': args.sample,
        'elapsed_seconds': elapsed, 'commands': commands,
        'passed': all(c['exit_code'] == 0 for c in commands)})


if __name__ == '__main__':
    main()
