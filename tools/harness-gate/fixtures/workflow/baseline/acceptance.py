#!/usr/bin/env python3
"""Real Git/artifact baseline transport and direct Rust evaluator acceptance."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'compiler'))
from acceptance import prepare, pin, write, load, run, evaluate, subject_id, series_id


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(root, *args):
    result = subprocess.run(['git', '-C', str(root), *args], capture_output=True, check=True)
    return result.stdout


def context(state, direct, commit, base):
    state['expected'].update(commit=commit, base_commit=base)
    direct['expected'] = copy.deepcopy(state['expected'])
    for record in direct['records']:
        record['context'] = copy.deepcopy(state['expected'])
        for artifact in record['artifacts']:
            artifact['context'] = copy.deepcopy(state['expected'])


def setup(root, kind, lineage=None, unknown=True, required=True, custom=False):
    state, direct = prepare(root, unknown)
    if custom:
        record = direct['records'][0]
        capability = record['capabilities'][1]
        capability.update(state='supported', artifacts=[record['artifacts'][0]['id']])
        capability['reason'] = 'Measured custom pulse count'
        record['metrics'].append(dict(name='quasar.pulses', value=dict(type='count', value=7), artifacts=capability['artifacts']))
    q = root / '.harness-gate/quality.toml'
    if unknown and not custom:
        series = state['series']['coverage']
        old = series['id']
        series['metrics'] = [series['metrics'][0]]
        series['id'] = series_id(series)
        direct['records'][0]['capabilities'] = [direct['records'][0]['capabilities'][0]]
        q.write_text(q.read_text().rsplit('\n[[collectors.coverage.produces]]', 1)[0].replace(old, series['id']))
    provider = ('{ kind = "retained_artifact", manifest = "retained/manifest.json" }'
                if kind == 'retained' else '{ kind = "git", reference = "trusted-base", merge_base = ' + ('true' if kind == 'merge' else 'false') + ' }')
    q.write_text(q.read_text().replace('provider = { kind = "none" }', 'provider = ' + provider)
                 .replace('required = false', 'required = ' + str(required).lower()))
    if required:
        direct['policy']['rules'][0]['ratchet'] = dict(deny_regression=True, allow_legacy_debt=True)
        write(root / '.harness-gate/policy.json', direct['policy'])
    pin(root, state)
    git(root, 'init', '-q')
    # Baseline identities bind file bytes, including native platform line endings.
    git(root, 'config', 'core.autocrlf', 'false')
    git(root, 'config', 'user.email', 'fixture@example.invalid')
    git(root, 'config', 'user.name', 'Baseline Fixture')
    git(root, 'add', '.harness-gate', 'src')
    git(root, 'commit', '-qm', 'base')
    base = git(root, 'rev-parse', 'HEAD').decode().strip()
    git(root, 'tag', 'trusted-base')
    context(state, direct, base, '0' * 40)
    base_state, base_direct = copy.deepcopy(state), copy.deepcopy(direct)
    retained = root / 'retained'
    files = dict(state['config_files'])
    files.update({s['path']: s['source_sha256'] for subjects in state['subjects'].values() for s in subjects})
    files.update({state['artifact_root'] + '/' + p: sha for p, sha in state['artifacts'].items()})
    for name in files:
        target = retained / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / name, target)
    if lineage:
        subject = state['subjects']['module'][0]
        old = subject['id']
        if lineage == 'move':
            (root / 'src/lib.rs').rename(root / 'src/moved.rs')
            subject['path'] = 'src/moved.rs'
            q.write_text(q.read_text().replace('src/lib.rs', 'src/moved.rs'))
        else:
            subject['discriminator'] = 'renamed-module'
        subject['id'] = subject_id('example', subject)
        direct['project']['subjects'] = [copy.deepcopy(subject)]
        direct['records'][0]['subject'] = copy.deepcopy(subject)
        direct['records'][0]['source']['path'] = subject['path']
        for artifact in direct['records'][0]['artifacts']:
            artifact['source']['path'] = subject['path']
        direct['selection'] = dict(changed_subject=[subject['id']], critical_subject=[])
        state['mappings'] = direct['mappings'] = dict(schema='subject-mappings/v1', project='example', mappings=[dict(kind=lineage, **{'from': old}, to=[subject['id']], reason='Trusted fixture lineage')])
    git(root, 'add', '.harness-gate', 'src')
    git(root, 'commit', '--allow-empty', '-qm', 'head')
    head = git(root, 'rev-parse', 'HEAD').decode().strip()
    if kind == 'merge':
        tree = git(root, 'rev-parse', base + '^{tree}').decode().strip()
        side = git(root, 'commit-tree', tree, '-p', base, '-m', 'divergent target').decode().strip()
        git(root, 'tag', '-f', 'trusted-base', side)
    context(state, direct, head, base)
    pin(root, state)
    manifest = dict(schema='quality-baseline-manifest/v1', state=base_state, evidence=base_direct['records'], files=files)
    write(retained / 'manifest.json', manifest)
    request = dict(schema='quality-baseline-request/v1', state=copy.deepcopy(base_state), manifest='retained/manifest.json', manifest_sha256=digest(retained / 'manifest.json'))
    return state, direct, manifest, request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--harness-gate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary, output = args.harness_gate.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    cases = ('git', 'merge', 'retained', 'custom-capability', 'custom-git', 'custom-stale', 'custom-incompatible', 'rust-debt', 'rename', 'move', 'regression',
             'optional-missing', 'required-missing', 'optional-no-request', 'required-no-request',
             'optional-none', 'missing-ref', 'optional-missing-ref', 'wrong-ref', 'head-substitution',
             'stale-commit', 'stale-run', 'changed-config', 'changed-tool', 'incompatible-series',
             'manifest-hash', 'artifact-hash', 'source-hash', 'inventory', 'traversal', 'symlink',
             'stale-evidence', 'capability-binding', 'invalid-mapping', 'git-source-mismatch',
             'optional-corrupt', 'output-in-tree', 'existing-output', 'dirty-tree')
    results = []
    for name in cases:
        root = output / ('repo-' + name)
        kind = name if name in ('git', 'merge') else 'retained'
        if name in ('custom-git', 'missing-ref', 'optional-missing-ref', 'wrong-ref', 'head-substitution', 'git-source-mismatch', 'dirty-tree'):
            kind = 'git'
        required = not name.startswith('optional')
        state, direct, manifest, request = setup(root, kind, name if name in ('rename', 'move') else None, name != 'rust-debt', required, name.startswith('custom-'))
        q = root / '.harness-gate/quality.toml'
        manifest_path = root / request['manifest']
        if name in ('optional-missing', 'required-missing'):
            manifest_path.unlink()
        if name == 'optional-none':
            q.write_text(q.read_text().replace('{ kind = "retained_artifact", manifest = "retained/manifest.json" }', '{ kind = "none" }'))
        if name in ('missing-ref', 'optional-missing-ref'):
            q.write_text(q.read_text().replace('trusted-base', 'missing-ref'))
        if name == 'wrong-ref':
            state['expected']['base_commit'] = '2' * 40
        if name == 'head-substitution':
            q.write_text(q.read_text().replace('trusted-base', 'HEAD'))
            state['expected']['base_commit'] = state['expected']['commit']
        if name in ('stale-commit', 'stale-run', 'changed-config'):
            if name == 'stale-commit': request['state']['expected']['commit'] = '2' * 40
            if name == 'stale-run': request['state']['expected']['run'] = 'another-ci-run'
            if name == 'changed-config': request['state']['config_files']['.harness-gate/policy.json'] = '0' * 64
        if name in ('changed-tool', 'incompatible-series', 'custom-incompatible'):
            series = state['series']['coverage']
            old = series['id']
            if name == 'changed-tool': series['tool']['version'] = 'changed'
            else: series['name'] = 'incompatible-custom-series'
            series['id'] = series_id(series)
            q.write_text(q.read_text().replace(old, series['id']))
        if name in ('manifest-hash', 'optional-corrupt'):
            manifest_path.write_text(manifest_path.read_text() + ' ')
        if name in ('artifact-hash', 'source-hash'):
            (manifest_path.parent / ('target/evidence/raw.json' if name == 'artifact-hash' else 'src/lib.rs')).write_text('tampered')
        if name == 'inventory': manifest['files']['unexpected'] = '0' * 64
        if name == 'traversal':
            manifest['state']['artifacts'] = {'../../../escape': '0' * 64}
            request['state'] = copy.deepcopy(manifest['state'])
        if name == 'symlink':
            path = manifest_path.parent / 'target/evidence/raw.json'
            path.unlink()
            path.symlink_to(root / 'target/evidence/raw.json')
        if name in ('stale-evidence', 'custom-stale'): manifest['evidence'][0]['context']['commit'] = state['expected']['commit']
        if name == 'capability-binding': manifest['evidence'][0]['capabilities'][0]['metric'] = 'alien.unbound'
        if name == 'invalid-mapping': state['mappings'] = {'invalid': True}
        if name == 'git-source-mismatch':
            path = manifest_path.parent / 'src/lib.rs'
            path.write_text('different immutable source')
            subject = manifest['state']['subjects']['module'][0]
            subject['source_sha256'] = digest(path)
            subject['id'] = subject_id('example', subject)
            manifest['files']['src/lib.rs'] = digest(path)
            request['state'] = copy.deepcopy(manifest['state'])
        if name == 'dirty-tree':
            (root / 'untracked.txt').write_text('preserve me')
            (root / 'src/lib.rs').write_text('local dirty source')
            # Head compilation correctly requires current pinned source bytes.
            subject = state['subjects']['module'][0]
            subject['source_sha256'] = digest(root / 'src/lib.rs')
            subject['id'] = subject_id('example', subject)
        if name in ('inventory', 'traversal', 'stale-evidence', 'custom-stale', 'capability-binding', 'git-source-mismatch'):
            write(manifest_path, manifest)
            request['manifest_sha256'] = digest(manifest_path)
        if name in ('rename', 'move', 'rust-debt'):
            # Both sides retain existing debt; a lost mapping would turn this into failure.
            direct['records'][0]['metrics'][0]['value']['covered'] = 3
            manifest['evidence'][0]['metrics'][0]['value']['covered'] = 3
            write(manifest_path, manifest)
            request['manifest_sha256'] = digest(manifest_path)
        if name == 'regression': direct['records'][0]['metrics'][0]['value']['covered'] = 3
        pin(root, state)
        request_path = Path(write(root / 'request.json', request))
        destination = output / ('resolved-' + name)
        if name == 'output-in-tree': destination = root / 'forbidden'
        if name == 'existing-output':
            destination.mkdir()
            (destination / 'sentinel').write_text('preserve')
        before = git(root, 'status', '--porcelain=v1', '-z')
        index = (root / '.git/index').read_bytes()
        argv = ['baseline', '--repository-root', root, '--state', root / 'state.json', '--output', destination]
        if name not in ('optional-no-request', 'required-no-request'): argv += ['--request', request_path]
        result = run(binary, root, argv)
        assert git(root, 'status', '--porcelain=v1', '-z') == before, name
        assert (root / '.git/index').read_bytes() == index, name
        success = name in ('git', 'merge', 'retained', 'custom-capability', 'custom-git', 'rust-debt', 'rename', 'move', 'regression', 'optional-missing', 'optional-no-request', 'optional-none', 'optional-missing-ref', 'dirty-tree')
        assert (result.returncode == 0) == success, (name, result.stderr)
        if not success:
            assert not destination.exists() or name == 'existing-output', name
            if name == 'existing-output': assert (destination / 'sentinel').read_text() == 'preserve'
        else:
            resolved = load(destination / 'resolution.json')
            if name.startswith('optional'):
                assert resolved['status'] == 'unavailable', name
                assert not (destination / 'evidence.json').exists(), name
            else:
                assert resolved['status'] == 'available', name
                assert resolved['commit'] == request['state']['expected']['commit'], name
                assert load(destination / 'expected.json') == request['state']['expected'], name
                assert load(destination / 'evidence.json') == manifest['evidence'], name
                if name in ('git', 'merge'):
                    repeat = output / ('repeat-' + name)
                    repeated_args = list(argv)
                    repeated_args[repeated_args.index('--output') + 1] = repeat
                    repeated = run(binary, root, repeated_args)
                    assert repeated.returncode == 0, repeated.stderr
                    assert load(repeat / 'resolution.json')['identity'] == resolved['identity'], name
                if name != 'dirty-tree':
                    extra = []
                    for k in ('project', 'expected', 'evidence'): extra += ['--base-' + k, destination / (k + '.json')]
                    extra += ['--base-source-root', resolved['source_root'], '--base-artifact-root', resolved['artifact_root']]
                    low = evaluate(binary, root, direct, False, extra)
                    compiled = evaluate(binary, root, direct, True, extra)
                    assert low.returncode == compiled.returncode, (name, low.stderr, compiled.stderr)
                    assert (root / 'direct-report.json').read_bytes() == (root / 'compiled-report.json').read_bytes(), name
                    assert (compiled.returncode == 0) == (name not in ('regression', 'custom-capability', 'custom-git')), (name, compiled.stderr, (root / 'compiled-report.json').read_text())
                    if name in ('rename', 'move'):
                        report = (root / 'compiled-report.json').read_text()
                        assert '"inherits_history": true' in report and '"legacy_debt_allowed": true' in report, report
        results.append(dict(case=name, provider=kind, success=success, stderr=result.stderr))
    write(output / 'acceptance.json', dict(schema='baseline-acceptance/v1', cases=results, passed=True))
    print(f'{len(results)} baseline cases passed')


if __name__ == '__main__':
    main()
