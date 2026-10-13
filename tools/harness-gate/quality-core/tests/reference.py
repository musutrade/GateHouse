"""Test-only transport/oracle for the task 2 slice; never evaluates release policy.

Read the immutable GH-146 corpus and existing contract fixtures. Materialize raw
bytes in the caller's temporary directory and record Python validation outcomes.
The Rust test consumes the identical inputs and independently validates them.
"""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import sys
import tarfile

QUALITY = Path(sys.argv[1]).resolve()
WORK = Path(sys.argv[2]).resolve()
sys.path.insert(0, str(QUALITY))
sys.path.insert(0, str(QUALITY / 'fixtures/generic-core'))
import replay
import harness_evidence as evidence
import project_model as project


def read(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())


def outcome(fn):
    try:
        value = fn()
        return dict(accepted=True, value=value)
    except (project.ModelError, evidence.MeasurementError) as error:
        return dict(accepted=False, reason_class=type(error).__name__, reason=str(error))


cases = []


def add(name, kind, **data):
    if kind == 'project':
        expected = outcome(lambda: project.validate_project(data['project']))
    elif kind == 'series':
        expected = outcome(lambda: evidence.require_compatible_series(data['base'], data['head']))
    else:
        context = {key: data[key] for key in ('project', 'source_root', 'artifact_root', 'expected')}
        if kind == 'requirements':
            expected = outcome(lambda: evidence.evaluate_requirements(data['records'], data['requirements'], **context))
        else:
            expected = outcome(lambda: hashlib.sha256(evidence.canonical_serialize(data['records'], **context)).hexdigest())
    cases.append(dict(name=name, kind=kind, oracle=expected, **data))
    return expected


root = QUALITY / 'fixtures/generic-core'
manifest = read(root / 'manifest.json')
for name, sha in manifest['files'].items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == sha, name
for name, sha in manifest['schemas'].items():
    assert hashlib.sha256((QUALITY.parents[1] / name).read_bytes()).hexdigest() == sha, name
with tarfile.open(root / 'artifacts.tar.gz') as archive:
    blobs = {}
    for member in archive.getmembers():
        assert member.isfile() and member.name not in blobs
        data = archive.extractfile(member).read()
        assert hashlib.sha256(data).hexdigest() == member.name
        blobs[member.name] = data
for item in manifest['cases']:
    case = read(root / item['input'])
    for side in ('head', 'base'):
        if side not in case:
            continue
        payload = case[side]
        context = dict(project=payload['project'], records=payload['records'], expected=payload['expected'])
        for kind in ('source', 'artifact'):
            destination = WORK / item['id'] / side / kind
            destination.mkdir(parents=True)
            for path, sha in payload['files'][kind].items():
                project.canonical_path(path)
                target = destination / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(blobs[sha])
            context[kind + '_root'] = str(destination)
        actual = add(item['id'] + '/' + side, 'evidence', **context)
        if side == 'head':
            frozen = read(root / item['expected'])['validation']
            assert actual['accepted'] == frozen['accepted']
            if not actual['accepted']:
                assert actual['reason_class'] == frozen['reason_class']
                # Only known missing-file wording may vary; semantic fields stay exact.
                assert replay.oracle_matches(frozen, replay.portable_errors(actual, WORK / item['id'])), (item['id'], actual, frozen)

base_project = read(QUALITY / 'fixtures/project-model/base.json')
base_records = read(QUALITY / 'fixtures/harness-evidence/polyglot.json')
context = dict(project=base_project, expected=read(QUALITY / 'fixtures/harness-evidence/expected.json'),
               source_root=str(QUALITY / 'fixtures/project-model/sources'),
               artifact_root=str(QUALITY / 'fixtures/harness-evidence'))
add('polyglot', 'evidence', records=base_records, **context)
for side in ('base', 'head'):
    add('project-' + side, 'project', project=read(QUALITY / f'fixtures/project-model/{side}.json'))


def edit(value, path, replacement=None, delete=False):
    for key in path[:-1]:
        value = value[key]
    if delete:
        del value[path[-1]]
    else:
        value[path[-1]] = replacement


for fixture in read(QUALITY / 'fixtures/harness-evidence/negative.json'):
    records = copy.deepcopy(base_records)
    edit(records[0], fixture['path'], fixture.get('value'), fixture.get('delete', False))
    add('negative/' + fixture['name'], 'evidence', records=records, **context)
for field in base_records[0]:
    records = copy.deepcopy(base_records)
    del records[0][field]
    add('missing/' + field, 'evidence', records=records, **context)
for field in ('metrics', 'capabilities', 'artifacts'):
    records = copy.deepcopy(base_records)
    records[0][field].append(copy.deepcopy(records[0][field][0]))
    add('duplicate/' + field, 'evidence', records=records, **context)
for value in (None, 1.0, True, [], {}, {'type': 'unknown'}, {'type': 'ratio', 'covered': True, 'total': 5},
              {'type': 'ratio', 'covered': 6, 'total': 5}, {'type': 'ratio', 'covered': 0, 'total': 0},
              {'type': 'ratio', 'covered': -1, 'total': 5},
              {'type': 'ratio', 'covered': 10**100, 'total': 10**100 + 1}):
    records = copy.deepcopy(base_records)
    records[0]['metrics'][0]['value'] = value
    add('typed/' + str(value), 'evidence', records=records, **context)
for field in ('commit', 'base_commit', 'target', 'run'):
    changed = copy.deepcopy(context)
    changed['expected'][field] = 'f' * 40 if 'commit' in field else 'other'
    add('stale/' + field, 'evidence', records=base_records, **changed)
for field, replacement in [('collector', {'name': 'other', 'version': '1'}), ('source', {'path': 'other', 'sha256': '0'*64}),
                            ('component', 'other'), ('status', 'measured')]:
    records = copy.deepcopy(base_records)
    records[0][field] = replacement
    add('envelope/' + field, 'evidence', records=records, **context)
for field in ('metrics', 'capabilities'):
    for refs in (['unknown'], [base_records[0][field][0]['artifacts'][0]] * 2):
        records = copy.deepcopy(base_records)
        records[0][field][0]['artifacts'] = refs
        add('artifact-links/' + field + str(refs), 'evidence', records=records, **context)
records = copy.deepcopy(base_records)
records[0]['artifacts'][0]['bytes'] += 1
add('artifact-size', 'evidence', records=records, **context)
records = copy.deepcopy(base_records)
records[1]['id'] = records[0]['id']
add('duplicate-evidence-id', 'evidence', records=records, **context)
records = copy.deepcopy(base_records)
extra = copy.deepcopy(records[0]); extra['id'] = 'another-id'; records.append(extra)
add('duplicate-subject-series', 'evidence', records=records, **context)

# Every capability state and requirement mode, including absent component evidence.
for state in ('supported', 'unsupported', 'not_configured', 'not_collected', 'measurement_error', 'not_applicable'):
    records = copy.deepcopy(base_records)
    if state != 'supported':
        for record in records:
            record['metrics'] = []
            for capability in record['capabilities']:
                capability['state'] = state
            record['status'] = 'measurement_error' if state == 'measurement_error' else 'unavailable'
    for mode in ('required', 'informational'):
        for unavailable in ('blocked', 'measurement_error'):
            requirements = dict(schema='capability-requirements/v1', requirements=[dict(
                component=records[0]['component'], metric='coverage.line', mode=mode, on_unavailable=unavailable)])
            add('/'.join((state, mode, unavailable)), 'requirements', records=records, requirements=requirements, **context)
requirements = read(QUALITY / 'fixtures/harness-evidence/requirements.json')
add('polyglot-requirements', 'requirements', records=base_records, requirements=requirements, **context)
add('missing-component-evidence', 'requirements', records=base_records[:1], requirements=requirements, **context)
for change in ('duplicate', 'unknown-component', 'unknown-metric'):
    req = copy.deepcopy(requirements)
    if change == 'duplicate': req['requirements'].append(copy.deepcopy(req['requirements'][0]))
    else: req['requirements'][0]['component' if change == 'unknown-component' else 'metric'] = 'unknown.metric'
    add(change, 'requirements', records=base_records, requirements=req, **context)

series = base_records[0]['series']
add('compatible-series', 'series', base=series, head=series)
add('missing-series', 'series', base=None, head=series)
for field in ('collector', 'tool', 'rule', 'runtime', 'source_identity', 'normalization', 'target', 'name'):
    head = copy.deepcopy(series)
    if isinstance(head[field], dict): head[field]['version'] = '2'
    else: head[field] = 'other'
    head['id'] = evidence.series_id(head)
    add('incompatible/' + field, 'series', base=series, head=head)
for field in ('unsorted', 'duplicate', 'unknown', 'wrong-type', 'noncanonical'):
    head = copy.deepcopy(series)
    if field == 'unsorted': head['metrics'].reverse()
    if field == 'duplicate': head['metrics'].append(head['metrics'][0])
    if field == 'unknown': head['metrics'][-1]['name'] = 'zzz.unknown'
    if field == 'wrong-type': head['metrics'][0]['type'] = 'boolean'
    head['id'] = evidence.series_id(head) if field != 'noncanonical' else 'measurement-series/v1:' + '0' * 64
    add('series/' + field, 'series', base=series, head=head)

# Port the existing project reference matrix, keeping each failure independent.
mutations = []
for field in ('components', 'subjects', 'relationships'):
    mutations.append((f'duplicate-{field}', lambda p, f=field: p[f].append(copy.deepcopy(p[f][0]))))
for field in ('component', 'target', 'boundary'):
    mutations.append((f'unknown-subject-{field}', lambda p, f=field: p['subjects'][0].update({f: 'missing'})))
for field in ('targets', 'source_boundaries'):
    mutations.append((f'duplicate-{field}', lambda p, f=field: p['components'][0][f].append(copy.deepcopy(p['components'][0][f][0]))))
mutations += [
    ('duplicate-component-path', lambda p: p['components'][1].update(path=p['components'][0]['path'])),
    ('boundary-escape', lambda p: p['components'][0]['source_boundaries'][0].update(path='other')),
    ('unknown-target-boundary', lambda p: p['components'][0]['targets'][0]['boundaries'].append('missing')),
    ('duplicate-target-boundary', lambda p: p['components'][0]['targets'][0]['boundaries'].append(p['components'][0]['targets'][0]['boundaries'][0])),
    ('unknown-consumer', lambda p: p['relationships'][0].update(consumer='missing')),
    ('self-relationship', lambda p: p['relationships'][0].update(consumer=p['relationships'][0]['producer'])),
    ('duplicate-edge', lambda p: p['relationships'].append(dict(p['relationships'][0], id='another'))),
    ('unknown-relationship-subject', lambda p: p['relationships'][0]['subjects'].append('subject-identity/v1:' + '0'*64)),
    ('duplicate-relationship-subject', lambda p: p['relationships'][0]['subjects'].append(p['relationships'][0]['subjects'][0])),
    ('unrelated-subject', lambda p: p['relationships'][0]['subjects'].append(p['subjects'][3]['id'])),
    ('future-ecosystem', lambda p: p['components'][0].update(metadata={'language': 'future-language'})),
    ('metadata-not-identity', lambda p: p['subjects'][0].update(metadata={'framework': 'other', 'metadata': 'descriptive'})),
]
for field, value in [('source_sha256', None), ('source_sha256', 'z'*64), ('source_sha256', 'a'*64+'\n'),
                     ('kind', 'unknown/v1'), ('identity_version', 'subject-identity/v2'),
                     ('id', 'subject-identity/v1:'+'0'*64), ('metadata', {'x':1}),
                     ('discriminator', ' run'), ('discriminator', 'cafe\u0301')]:
    mutations.append((field + str(value), lambda p, f=field, v=value: edit(p['subjects'][0], [f], v, v is None)))
for path in ('/frontend/src/run.ts', 'frontend/src/../run.ts', 'frontend//src/run.ts', 'frontend/./src/run.ts',
             'frontend\\src\\run.ts', 'C:/run.ts', 'api/src/run.rs', 'frontend/src/run.ts/', 'frontend/src/cafe\u0301.ts'):
    mutations.append(('path/' + path, lambda p, v=path: p['subjects'][0].update(path=v)))
for span in ({'start_line':2,'start_column':1,'end_line':1,'end_column':1},
             {'start_line':True,'start_column':1,'end_line':1,'end_column':1}):
    mutations.append(('span/' + str(span), lambda p, v=span: p['subjects'][0].update(span=v)))
def conflict(p):
    subject = copy.deepcopy(p['subjects'][0])
    subject['source_sha256'] = 'a'*64
    subject['id'] = project.subject_id(p['id'], subject)
    p['subjects'].append(subject)
mutations.append(('conflicting-source', conflict))
for name, mutate in mutations:
    model = copy.deepcopy(base_project)
    mutate(model)
    add('project/' + name, 'project', project=model)

# Exercise every typed value independently, including precision beyond u64.
typed_values = {
    'coverage.line': [{'type':'ratio','covered':0,'total':1}, {'type':'ratio','covered':10**100,'total':10**100+1}],
    'complexity.cyclomatic': [{'type':'count','value':0}, {'type':'count','value':10**100}],
    'contract.schema_valid': [{'type':'boolean','value':False}],
    'performance.duration': [{'type':'duration','value':10**100,'unit':'nanoseconds'}],
    'bundle.size': [{'type':'size','value':10**100,'unit':'bytes'}],
    'risk.crap': [{'type':'decimal','value':'0.000'}, {'type':'rational','numerator':10**100,'denominator':10**101+1}],
}
for metric, values in typed_values.items():
    for value in values:
        record = copy.deepcopy(base_records[0])
        record['status'] = 'measured'
        record['series']['metrics'] = [dict(name=metric, type=value['type'])]
        record['series']['id'] = evidence.series_id(record['series'])
        record['metrics'] = [dict(name=metric, value=value, artifacts=['raw'])]
        record['capabilities'] = [dict(metric=metric, state='supported', reason='synthetic', artifacts=['raw'])]
        add('value/' + str(value), 'evidence', records=[record], **context)
        for key in value:
            bad = copy.deepcopy(record)
            del bad['metrics'][0]['value'][key]
            add('value-missing/' + str(value) + '/' + key, 'evidence', records=[bad], **context)
            for invalid in [None, [], {}, True, -1, 1.5, 'wrong']:
                bad = copy.deepcopy(record)
                bad['metrics'][0]['value'][key] = invalid
                add('value-invalid/' + str(value) + '/' + key + '/' + str(invalid), 'evidence', records=[bad], **context)
        bad = copy.deepcopy(record)
        bad['metrics'][0]['value']['extra'] = 'unknown'
        add('value-extra/' + str(value), 'evidence', records=[bad], **context)

# Changes that remain valid must recompute identity, independent of metadata.
for field, value in [('discriminator','café😀'), ('path','frontend/src/café.ts'),
                     ('span',dict(start_line=10**100,start_column=1,end_line=10**100,end_column=2))]:
    model = copy.deepcopy(base_project)
    subject = model['subjects'][0]
    old = subject['id']
    subject[field] = value
    subject['id'] = project.subject_id(model['id'], subject)
    assert subject['id'] != old
    for relationship in model['relationships']:
        relationship['subjects'] = [subject['id'] if s == old else s for s in relationship['subjects']]
    add('identity/' + field, 'project', project=model)

(WORK / 'cases.json').write_text(json.dumps(cases, ensure_ascii=False), encoding='utf-8')
print(json.dumps({'cases':len(cases), 'frozen_cases':len(manifest['cases']), 'kinds':{kind:sum(c['kind']==kind for c in cases) for kind in ('evidence','project','series','requirements')}}))
