from pathlib import Path
import json, subprocess, shutil, copy, hashlib

import argparse
parser = argparse.ArgumentParser()
parser.add_argument('--source-root', type=Path, required=True)
parser.add_argument('--source-identity', required=True)
parser.add_argument('--output', type=Path, required=True)
a = parser.parse_args()
r = a.output.resolve()
r.mkdir(exist_ok=False, parents=True)
fixtures = a.source_root.resolve()/'tools/quality/fixtures/contracts'
G = a.source_identity
artifacts = r/'fixture-artifacts'
artifacts.mkdir()
records = json.loads((fixtures/'evidence.json').read_text())
for record in records:
    for a in record['artifacts']:
        path = artifacts/a['path']
        if path.exists(): continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((fixtures.parent/a['path']).read_bytes())
        assert hashlib.sha256(path.read_bytes()).hexdigest() == a['sha256']
        assert path.stat().st_size == a['bytes']

def write_fixture(name, project, policy, evidence, sources, expected_state, note):
    root = r/'inputs'/name
    root.mkdir(parents=True)
    config = dict(schema='gh292-profile-fixture/v1', name=name, source_commit=G,
                  source_root=str(sources), artifact_root=str(artifacts),
                  expected_state=expected_state, subjects=len(project['subjects']),
                  records=len(evidence), rules=len(policy['rules']),
                  boundary=note)
    for filename,value in [('project.json',project),('policy.json',policy),('records.json',evidence),
                           ('expected.json',json.loads((fixtures/'expected.json').read_text())),('fixture.json',config)]:
        (root/filename).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')
    return root

project = json.loads((fixtures/'project.json').read_text())
policy = json.loads((fixtures/'policy.json').read_text())
small_sources = r/'small-sources'
shutil.copytree(fixtures/'sources',small_sources)
small = write_fixture('small',project,policy,records,small_sources,'fail',
                      'Unmodified fixed synthetic contracts fixture, including cross-component failing gates; no real collector execution.')

large_project = copy.deepcopy(project)
large_project['subjects'] = []
large_project['relationships'] = []
large_project['metadata']['purpose'] = 'gh292 synthetic large fixture; no measurement/toolchain claims'
large_policy = copy.deepcopy(policy)
large_policy['rules'] = copy.deepcopy(policy['rules'][:4])
for rule,component in zip(large_policy['rules'],project['components']):
    rule['scope'] = dict(kind='component',component=component['id'])
sources = r/'large-sources'
large_records = []
for n in range(250):
    for original in records[:4]:
        record = copy.deepcopy(original)
        subject = record['subject']
        original_path = subject['path']
        subject['path'] = str(Path(original_path).parent/f'gh292-{n:04}'/Path(original_path).name)
        payload = dict(identity_version='subject-identity/v1',project=project['id'])
        for key in ['component','target','boundary','kind','path','discriminator','span','source_sha256']:
            if key in subject: payload[key] = subject[key]
        subject['id'] = 'subject-identity/v1:'+hashlib.sha256(json.dumps(payload,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
        record['id'] += f'-gh292-{n:04}'
        record['source']['path'] = subject['path']
        for artifact in record['artifacts']: artifact['source']['path'] = subject['path']
        target = sources/subject['path']
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes((fixtures/'sources'/original_path).read_bytes())
        large_project['subjects'].append(copy.deepcopy(subject))
        large_records.append(record)
large = write_fixture('large',large_project,large_policy,large_records,sources,'pass',
                      '1000 independently identified synthetic subjects, four component-scope rules; manual fixture source bytes copied exactly; no collector execution. Small fixture covers cross-component behavior.')
manifest = {}
for root in [r/'inputs',artifacts,sources,small_sources]:
    for p in sorted(root.rglob('*')):
        if p.is_file(): manifest[str(p.relative_to(r))] = hashlib.sha256(p.read_bytes()).hexdigest()
(r/'fixture-input-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'small':str(small),'large':str(large),'files':len(manifest)}))
