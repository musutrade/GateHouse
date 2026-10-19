#!/usr/bin/env python3
"""Retained direct/compiled Rust equivalence and compiler trust-boundary corpus."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[4]
sys.path.insert(0, str(REPO / "tools/quality"))
from project_model import subject_id
from harness_evidence import series_id


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + '\n')
    return str(path)


def load(path):
    return json.loads(path.read_text())


def prepare(root, unknown=False):
    (root / '.harness-gate').mkdir(parents=True)
    (root / 'src').mkdir()
    (root / 'target/evidence').mkdir(parents=True)
    shutil.copyfile(ROOT / 'source.txt', root / 'src/lib.rs')
    shutil.copyfile(ROOT / 'artifact.txt', root / 'target/evidence/raw.json')
    shutil.copyfile(REPO / 'tools/harness-gate/fixtures/preset-compat/rust-api.flow.toml', root / '.harness-gate/flow.toml')
    direct = load(ROOT / 'direct.json')
    state = load(ROOT / ('unknown-ecosystem.json' if unknown else 'state.json'))
    quality = (ROOT / 'quality.toml').read_text()
    if unknown:
        quality = quality.replace(direct['records'][0]['series']['id'], state['series']['coverage']['id'])
        quality += '\n[[collectors.coverage.produces]]\ntarget = { kind = "component", id = "app" }\ncapability = "quasar.pulses"\nseries = "' + state['series']['coverage']['id'] + '"\n'
        direct['project']['components'] = list(state['components'].values())
        direct['records'][0]['series'] = state['series']['coverage']
        direct['records'][0]['collector'] = state['series']['coverage']['collector']
        direct['records'][0]['capabilities'].append(dict(metric='quasar.pulses', state='unsupported', reason='uncertified custom metric', artifacts=['raw']))
    (root / '.harness-gate/quality.toml').write_text(quality)
    write(root / '.harness-gate/policy.json', direct['policy'])
    write(root / '.harness-gate/coverage-request.json', {'pack': 'trusted-fixture'})
    pin(root, state)
    return state, direct


def pin(root, state):
    # Contract keys match configured paths, independent of native OS separators.
    state['config_files'] = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                             for p in sorted((root / '.harness-gate').iterdir())}
    write(root / 'state.json', state)


def run(binary, root, args):
    return subprocess.run([str(binary), 'quality', *map(str, args)], cwd=root,
                          capture_output=True, text=True)


def compile_inputs(binary, root):
    return run(binary, root, ['compile', '--repository-root', root, '--state', root / 'state.json', '--output', root / 'compiled.json'])


def evaluate(binary, root, direct, compiled, extra=()):
    args = ['evaluate', '--evidence', write(root / 'evidence.json', direct['records']),
            '--now', '2026-09-09T00:00:00Z', '--output', root / ('compiled-report.json' if compiled else 'direct-report.json')]
    if compiled:
        args += ['--repository-root', root, '--state', root / 'state.json']
    else:
        for key in ('project', 'policy', 'expected'):
            args += ['--' + key, write(root / (key + '.json'), direct[key])]
        args += ['--source-root', root, '--artifact-root', root / 'target/evidence',
                 '--selection', write(root / 'selection.json', direct.get('selection', {'changed_subject': [direct['project']['subjects'][0]['id']], 'critical_subject': []})),
                 '--exceptions', write(root / 'exceptions.json', direct.get('exceptions', []))]
        if direct.get('mappings') is not None:
            args += ['--mappings', write(root / 'mappings.json', direct['mappings'])]
    return run(binary, root, args + list(extra))


def add_relationship(root, state, direct):
    component = copy.deepcopy(state['components']['app'])
    component['id'] = 'consumer'
    component['path'] = 'consumer'
    component['source_boundaries'][0]['path'] = 'consumer/src'
    state['components']['consumer'] = component
    subject = copy.deepcopy(direct['records'][0]['subject'])
    subject['component'] = 'consumer'
    subject['path'] = 'consumer/src/lib.rs'
    subject['id'] = subject_id('example', subject)
    state['subjects']['consumer-module'] = [subject]
    state['relationship_kinds'] = {'consumes': 'consumes/v1'}
    (root / 'consumer/src').mkdir(parents=True)
    shutil.copyfile(ROOT / 'source.txt', root / subject['path'])
    q = root / '.harness-gate/quality.toml'
    q.write_text(q.read_text() + '''
[components.consumer]
flow_components = ["app"]
source_roots = ["consumer/src"]
artifact_root = "target/evidence"
[subjects.consumer-module]
component = "consumer"
kind = "module"
selection = { kind = "explicit", paths = ["consumer/src/lib.rs"] }
[relationships.link]
kind = "consumes"
from = { kind = "subject", id = "module" }
to = { kind = "component", id = "consumer" }
''')
    direct['project']['components'].append(component)
    direct['project']['subjects'].append(subject)
    direct['selection'] = dict(changed_subject=[direct['records'][0]['subject']['id']], critical_subject=[])
    direct['project']['subjects'].sort(key=lambda s: s['id'])
    direct['project']['relationships'] = [dict(id='link', kind='consumes/v1', producer='app', consumer='consumer', subjects=sorted(s['id'] for s in direct['project']['subjects']), metadata={})]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--harness-gate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary, output = args.harness_gate.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    results = []
    for name in ('pass', 'threshold-fail', 'missing-evidence', 'stale-context', 'malformed-value', 'duplicate-evidence', 'required-unsupported', 'invalid-exception', 'ratchet-pass', 'ratchet-regression', 'missing-base', 'subject-policy', 'unknown-ecosystem', 'unknown-supported', 'discovery', 'multiple-subjects', 'relationship', 'ratchet-mapping', 'ratchet-invalid-mapping', 'no-optionals'):
        root = output / name
        state, direct = prepare(root, name in ('unknown-ecosystem', 'unknown-supported'))
        record = direct['records'][0]
        if name == 'unknown-supported':
            series = state['series']['coverage']
            old = series['id']
            series['metrics'] = [series['metrics'][0]]
            series['id'] = series_id(series)
            record['capabilities'] = [record['capabilities'][0]]
            q = root / '.harness-gate/quality.toml'
            q.write_text(q.read_text().rsplit('\n[[collectors.coverage.produces]]', 1)[0].replace(old, series['id']))
        if name == 'no-optionals':
            state['selection'] = state['exceptions'] = None
        if name == 'discovery':
            q = root / '.harness-gate/quality.toml'
            q.write_text(q.read_text().replace('kind = "explicit", paths = ["src/lib.rs"]', 'kind = "discovery", collector = "coverage", query = "pack-owned-module-query"'))
        if name == 'multiple-subjects':
            subject = copy.deepcopy(record['subject'])
            subject['discriminator'] = 'second-module'
            subject['id'] = subject_id('example', subject)
            state['subjects']['module'].append(subject)
            direct['project']['subjects'].append(subject)
            direct['project']['subjects'].sort(key=lambda s: s['id'])
            second = copy.deepcopy(record)
            second['id'] = 'second-coverage'
            second['subject'] = subject
            direct['records'].append(second)
            direct['selection'] = dict(changed_subject=[s['id'] for s in direct['project']['subjects']], critical_subject=[])
        if name == 'relationship':
            add_relationship(root, state, direct)
        if name == 'threshold-fail': record['metrics'][0]['value']['covered'] = 1
        if name == 'missing-evidence': direct['records'] = []
        if name == 'stale-context': record['context'] = dict(record['context'], run='stale')
        if name == 'malformed-value': record['metrics'][0]['value']['covered'] = 7
        if name == 'duplicate-evidence': direct['records'].append(copy.deepcopy(record))
        if name == 'required-unsupported':
            record['capabilities'][0]['state'] = 'unsupported'
            record['metrics'] = []
            record['status'] = 'unavailable'
        if name == 'invalid-exception': state['exceptions'] = direct['exceptions'] = [{'invalid': True}]
        if name == 'ratchet-mapping': state['mappings'] = direct['mappings'] = dict(schema='subject-mappings/v1', project='example', mappings=[])
        if name == 'ratchet-invalid-mapping': state['mappings'] = direct['mappings'] = dict(invalid=True)
        if name.startswith('ratchet') or name == 'missing-base':
            direct['policy']['rules'][0]['ratchet'] = dict(deny_regression=True, allow_legacy_debt=True)
            q = root / '.harness-gate/quality.toml'
            q.write_text(q.read_text().replace('required = false', 'required = true').replace('provider = { kind = "none" }', 'provider = { kind = "git", reference = "origin/main", merge_base = true }'))
        if name == 'subject-policy':
            rule = direct['policy']['rules'][0]
            rule['scope'] = dict(kind='subject', subject='module')
            q = root / '.harness-gate/quality.toml'
            q.write_text(q.read_text().replace('target = { kind = "component", id = "app" }', 'target = { kind = "subject", id = "module" }'))
        write(root / '.harness-gate/policy.json', direct['policy'])
        pin(root, state)
        compiled = compile_inputs(binary, root)
        assert compiled.returncode == 0, (name, compiled.stderr)
        bundle = load(root / 'compiled.json')
        if name == 'subject-policy':
            rule['id'] += '@' + record['subject']['id']
            rule['scope']['subject'] = record['subject']['id']
        assert bundle['project'] == direct['project'], name
        assert bundle['policy'] == direct['policy'], name
        assert bundle['expected'] == direct['expected'], name
        before = (root / 'compiled.json').read_bytes()
        state['subjects']['module'].reverse()
        pin(root, state)
        assert compile_inputs(binary, root).returncode == 0
        assert (root / 'compiled.json').read_bytes() == before, name
        extra = []
        if name.startswith('ratchet'):
            base = copy.deepcopy(direct)
            base['expected']['commit'] = direct['expected']['base_commit']
            for base_record in base['records']:
                base_record['context'] = base['expected']
                for artifact in base_record['artifacts']:
                    artifact['context'] = base['expected']
            if name == 'ratchet-regression': record['metrics'][0]['value']['covered'] = 3
            for key in ('project', 'expected'):
                extra += ['--base-' + key, write(root / ('base-' + key + '.json'), base[key])]
            extra += ['--base-evidence', write(root / 'base-evidence.json', base['records']), '--base-source-root', str(root), '--base-artifact-root', str(root / 'target/evidence')]
        low = evaluate(binary, root, direct, False, extra)
        high = evaluate(binary, root, direct, True, extra)
        assert high.returncode == low.returncode, (name, low.stderr, high.stderr)
        assert (root / 'compiled-report.json').read_bytes() == (root / 'direct-report.json').read_bytes(), name
        expected_pass = name in ('pass', 'ratchet-pass', 'subject-policy', 'unknown-supported', 'discovery', 'multiple-subjects', 'relationship', 'ratchet-mapping', 'no-optionals')
        assert (high.returncode == 0) == expected_pass, (name, high.stderr)
        results.append(dict(case=name, equivalent=True, passed=expected_pass))
    # Compiler-only trust failures are stricter than caller-trusted direct inputs.
    for name in ('config', 'source', 'artifact', 'tool', 'profile', 'inventory', 'series', 'subject', 'selection', 'root', 'bindings', 'artifact-binding', 'subject-kind', 'subject-path', 'subject-target', 'source-boundary', 'config-inventory', 'collector-inventory', 'kind-inventory', 'unknown-selection', 'malformed-state', 'malformed-evidence', 'missing-state', 'missing-config', 'schema'):
        root = output / ('reject-' + name)
        state, direct = prepare(root)
        if name in ('config', 'source', 'artifact'):
            file = {'config': '.harness-gate/policy.json', 'source': 'src/lib.rs', 'artifact': 'target/evidence/raw.json'}[name]
            with (root / file).open('a') as f: f.write(' ')
        if name == 'tool': state['series']['coverage']['tool']['version'] = 'stale'
        if name == 'profile': state['profile'] = 'unknown'
        if name == 'inventory': state['components']['extra'] = state['components']['app']
        if name == 'series': state['series']['coverage']['id'] = 'measurement-series/v1:' + '0' * 64
        if name == 'subject': state['subjects']['module'][0]['id'] = 'subject-identity/v1:' + '0' * 64
        if name == 'selection': state['selection']['changed_subject'] = ['missing']
        if name == 'root': state['artifact_root'] = '../outside'
        if name == 'bindings': direct['records'][0]['series']['id'] = 'measurement-series/v1:' + '0' * 64
        if name == 'artifact-binding': direct['records'][0]['artifacts'][0]['sha256'] = '0' * 64
        if name == 'subject-kind': state['subjects']['module'][0]['kind'] = 'function/v1'
        if name == 'subject-path': state['subjects']['module'][0]['path'] = 'src/elsewhere.rs'
        if name == 'subject-target': state['subjects']['module'][0]['target'] = 'wrong'
        if name == 'source-boundary': state['components']['app']['source_boundaries'][0]['path'] = '.'
        if name == 'config-inventory': state['config_files'].pop('.harness-gate/policy.json')
        if name == 'collector-inventory': state['series'] = {}
        if name == 'kind-inventory': state['subject_kinds'] = {}
        if name == 'unknown-selection': state['selection'] = {'unknown': []}
        if name == 'schema': state['schema'] = 'quality-trusted-state/v99'
        if name == 'malformed-evidence': direct['records'] = {}
        write(root / 'state.json', state)
        if name == 'malformed-state': (root / 'state.json').write_text('{')
        if name == 'missing-state': (root / 'state.json').unlink()
        if name == 'missing-config': (root / '.harness-gate/quality.toml').unlink()
        report = root / 'compiled-report.json'
        report.write_text('stale pass')
        result = evaluate(binary, root, direct, True)
        assert result.returncode != 0 and not report.exists(), (name, result.stderr)
        if name not in ('bindings', 'artifact-binding', 'malformed-evidence'):
            (root / 'compiled.json').write_text('stale bundle')
            assert compile_inputs(binary, root).returncode != 0
            assert not (root / 'compiled.json').exists()
        results.append(dict(case='reject-' + name, rejected=True))
    for filename in ('state.json', '.harness-gate/quality.toml', 'src/lib.rs', 'target/evidence/raw.json'):
        root = output / ('alias-' + Path(filename).name)
        state, direct = prepare(root)
        destination = root / filename
        before = destination.read_bytes()
        result = run(binary, root, ['compile', '--repository-root', root, '--state', root / 'state.json', '--output', destination])
        assert result.returncode != 0 and destination.read_bytes() == before
        result = run(binary, root, ['evaluate', '--repository-root', root, '--state', root / 'state.json', '--evidence', write(root / 'evidence.json', direct['records']), '--output', destination])
        assert result.returncode != 0 and destination.read_bytes() == before
        results.append(dict(case='output-alias-' + filename, rejected=True))
    root = output / 'digest-context'
    state, _ = prepare(root)
    assert compile_inputs(binary, root).returncode == 0
    before = load(root / 'compiled.json')
    state['expected']['run'] = 'fresh-run'
    write(root / 'state.json', state)
    assert compile_inputs(binary, root).returncode == 0
    after = load(root / 'compiled.json')
    assert after['identity'] != before['identity']
    # A new run identity is a compilation input, not a policy revision.
    assert after['quality_policy_identity'] == before['quality_policy_identity']
    results.append(dict(case='digest-context', identity_changed=True, policy_stable=True))
    # Presentation and delivery settings are neither policy nor runtime inputs,
    # but the full compilation identity still binds the edited flow bytes.
    root = output / 'policy-projection'
    state, _ = prepare(root)
    assert compile_inputs(binary, root).returncode == 0
    baseline = load(root / 'compiled.json')
    flow_path = root / '.harness-gate/flow.toml'
    source = flow_path.read_text()
    assert 'label = "' in source
    flow_path.write_text(source.replace('label = "', 'label = "edited ', 1))
    pin(root, state)
    assert compile_inputs(binary, root).returncode == 0
    edited = load(root / 'compiled.json')
    assert edited['identity'] != baseline['identity']
    assert edited['quality_policy_identity'] == baseline['quality_policy_identity']
    results.append(dict(case='presentation-not-policy', identity_changed=True, policy_stable=True))
    # A threshold edit is a policy revision.
    policy_path = root / '.harness-gate/policy.json'
    policy = load(policy_path)
    policy['rules'][0]['limit']['covered'] = 3
    write(policy_path, policy)
    pin(root, state)
    assert compile_inputs(binary, root).returncode == 0
    tightened = load(root / 'compiled.json')
    assert tightened['quality_policy_identity'] != baseline['quality_policy_identity']
    results.append(dict(case='threshold-is-policy', policy_changed=True))
    write(output / 'acceptance.json', dict(schema='quality-compilation-acceptance/v1', cases=results, unexplained_mismatches=0))
    print(f'{len(results)} compilation/equivalence cases; zero unexplained mismatches')


if __name__ == '__main__':
    main()
