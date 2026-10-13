"""Test-only tasks 3–4 oracle. Retained bytes only; collectors are never rerun."""
import copy
from dataclasses import asdict, is_dataclass
from datetime import datetime
import gzip
import hashlib
import io
import json
from pathlib import Path
import shutil
import sys
import tarfile
import unittest

QUALITY, WORK = map(lambda p: Path(p).resolve(), sys.argv[1:])
sys.path[:0] = [str(QUALITY), str(QUALITY / 'tests')]
sys.path.insert(0, str(QUALITY / 'fixtures/generic-core'))
import replay
import harness_evidence as evidence
import project_model as project
import policy_engine as engine
import policy_ratchet as ratchet
import test_policy_engine
import test_policy_ratchet
import test_project_report
import project_report


def read(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())


def serial(value):
    if is_dataclass(value):
        return asdict(value)
    if isinstance(value, (Path, datetime)):
        return value.isoformat() if isinstance(value, datetime) else str(value)
    raise TypeError(type(value))


def plain(value):
    return json.loads(json.dumps(value, default=serial))


def outcome(fn):
    try:
        return dict(accepted=True, value=plain(fn()))
    except (project.ModelError, evidence.MeasurementError) as error:
        return dict(accepted=False, reason_class=type(error).__name__, reason=str(error))


cases = []
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


def evaluate_case(name, args, kwargs, frozen=None):
    oracle = outcome(lambda: original_evaluate(*args, **kwargs))
    if frozen is not None:
        expected = dict(accepted=True, value=frozen['policy_result']) if 'policy_result' in frozen else dict(
            accepted=False, **frozen['evaluation_error'])
        # Temporary roots and known native missing-file wording may vary.
        canonical = replay.portable_errors(oracle, WORK / name)
        assert replay.oracle_matches(expected, canonical), name
    cases.append(dict(name=name, kind='evaluate', args=plain(args), kwargs=plain(kwargs),
                      oracle=oracle))
    if oracle["accepted"]:
        report = project_report.report(oracle["value"], kwargs["project"], args[0])
        cases[-1]["project_report"] = report
        if frozen is not None and "project_report" in frozen:
            canonical = replay.portable_errors(report, WORK / name)
            assert replay.oracle_matches(frozen["project_report"], canonical), name
        cases.append(dict(name=name + "/report", kind="report",
                          args=[oracle["value"], plain(kwargs["project"]), args[0]], kwargs={},
                          oracle=dict(accepted=True, value=report)))
    return oracle


original_evaluate = engine.evaluate
for item in manifest['cases']:
    case = read(root / item['input'])
    contexts = {}
    for side in ('head', 'base'):
        if side not in case:
            continue
        payload = case[side]
        context = {key: payload[key] for key in ('project', 'expected')}
        for kind in ('source', 'artifact'):
            destination = WORK / item['id'] / side / kind
            destination.mkdir(parents=True)
            for path, sha in payload['files'][kind].items():
                project.canonical_path(path)
                target = destination / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(blobs[sha])
            context[kind + '_root'] = destination
        contexts[side] = context
    kwargs = dict(contexts['head'], now=datetime.fromisoformat(case['now'].replace('Z', '+00:00')))
    for key in ('selection', 'mappings', 'exceptions'):
        if key in case:
            kwargs[key] = case[key]
    if 'base' in case:
        kwargs.update(base_records=case['base']['records'], base_context=contexts['base'])
    evaluate_case(item['id'], [case['policy'], case['head']['records']], kwargs,
                  read(root / item['expected']))

# Capture actual reference acceptance tests, preserving bytes before their cleanup
# or mutation. Nested calls stay inside Python; Rust independently re-evaluates.
depth = 0
active_test = ''
originals = {}


def capture(module, method):
    original = getattr(module, method)
    originals[method] = original
    def wrapped(*args, **kwargs):
        global depth
        if depth:
            return original(*args, **kwargs)
        name = active_test + '/' + method + '/' + str(len(cases))
        saved_args, saved_kwargs = copy.deepcopy(args), copy.deepcopy(kwargs)
        if method == 'evaluate':
            for side, context in [('head', saved_kwargs), ('base', saved_kwargs.get('base_context'))]:
                if context:
                    for key in ('source_root', 'artifact_root'):
                        destination = WORK / name / side / key
                        shutil.copytree(context[key], destination)
                        context[key] = destination
        depth += 1
        try:
            if method == 'evaluate':
                oracle = evaluate_case(name, saved_args, saved_kwargs)
            else:
                oracle = outcome(lambda: original(*saved_args, **saved_kwargs))
                cases.append(dict(name=name, kind=method, args=plain(saved_args),
                                  kwargs=plain(saved_kwargs), oracle=oracle))
        finally:
            depth -= 1
        # Keep the test's native types, exception and exact original behavior.
        depth += 1
        try:
            return original(*args, **kwargs)
        finally:
            depth -= 1
    setattr(module, method, wrapped)


for method in ('evaluate', 'compare', 'aggregate', 'validate_policy', '_select'):
    capture(engine, method)
capture(ratchet, 'review_exceptions')


class Result(unittest.TextTestResult):
    def startTest(self, test):
        global active_test
        active_test = test.id()
        super().startTest(test)


suite = unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromModule(module)
                           for module in (test_policy_engine, test_policy_ratchet, test_project_report))
log = io.StringIO()
run = unittest.TextTestRunner(stream=log, resultclass=Result).run(suite)
assert run.wasSuccessful(), log.getvalue()
# Deterministic arithmetic and policy boundary matrix independent of Rust code.
def add(kind, *args, **kwargs):
    fn = dict(compare=originals['compare'], validate_policy=originals['validate_policy'],
              _select=originals['_select'], decision=lambda r, h, b: ratchet.decision(r, h, b, originals['compare']),
              review_exceptions=originals['review_exceptions'])[kind]
    cases.append(dict(name='matrix/' + kind + '/' + str(len(cases)), kind=kind,
                      args=plain(args), kwargs=plain(kwargs), oracle=outcome(lambda: fn(*args, **kwargs))))

values = [({'type': 'ratio', 'covered': 10**120, 'total': 10**120+1},
           {'type': 'ratio', 'covered': 10**120-1, 'total': 10**120}),
          ({'type': 'rational', 'numerator': 10**120+1, 'denominator': 7},
           {'type': 'rational', 'numerator': 10**120, 'denominator': 7}),
          ({'type': 'decimal', 'value': '0.'+'0'*120+'1'}, {'type': 'decimal', 'value': '0'}),
          ({'type': 'decimal', 'value': '1.2'}, {'type': 'decimal', 'value': '1.19'}),
          ({'type': 'boolean', 'value': True}, {'type': 'boolean', 'value': False})]
for kind in ('count', 'duration', 'size'):
    values.append((dict(type=kind, value=10**120+1), dict(type=kind, value=10**120)))
    if kind != 'count':
        for value in values[-1]:
            value['unit'] = 'nanoseconds' if kind == 'duration' else 'bytes'
for a, b in values:
    for x, y in ((a, b), (b, a), (a, a)):
        for op in ('lt', 'le', 'eq', 'ne', 'ge', 'gt', 'unknown'):
            add('compare', x, op, y)
for value in ({}, {'type': 'unknown'}, {'type':'ratio','covered':2,'total':1},
              {'type':'ratio','covered':0,'total':0}, {'type':'count','value':True},
              {'type':'rational','numerator':1,'denominator':0},
              {'type':'decimal','value':'1.0'}, {'type':'decimal','value':'-1'},
              {'type':'count','value':-1}, {'type':'size','value':1.5}):
    add('compare', value, 'eq', value)
add('compare', {'type':'count','value':1}, 'eq', {'type':'size','value':1})
f = test_policy_engine.PolicyTests()
f.setUp()
for field in f.policy['rules'][0]:
    policy = copy.deepcopy(f.policy)
    del policy['rules'][0][field]
    add('validate_policy', policy, f.context['project'])
for changes in ({'metric':'unknown'}, {'scope':{'kind':'subject','subject':'unknown'}},
                {'scope':{'kind':'component','component':'unknown'}},
                {'scope':{'kind':'boundary','component':f.records[0]['component'],'boundary':'unknown'}},
                {'limit':{'type':'ratio','covered':6,'total':5}},
                {'limit':{'type':'count','value':1}}, {'required':'yes'},
                {'ratchet':{'deny_regression':True}}, {'operator':'unknown'}):
    policy = copy.deepcopy(f.policy)
    policy['rules'][0].update(changes)
    add('validate_policy', policy, f.context['project'])
add('validate_policy', f.policy, f.context['project'])
policy = copy.deepcopy(f.policy)
policy['rules'] *= 2
add('validate_policy', policy, f.context['project'])
for kind in ('changed_subject', 'critical_subject'):
    for selection in ({}, {kind:[]}, {kind:False}, {kind:['unknown']},
                      {kind:[f.records[0]['subject']['id']]*2},
                      {kind:[f.records[-1]['subject']['id'], f.records[0]['subject']['id']]}):
        add('_select', {'kind':kind}, f.context['project'], selection, f.context['expected']['target'])
for op in ('lt','le','eq','ne','ge','gt'):
    for before, after in ((64,64),(64,55),(64,65),(18,27),(31,30),(30,31)):
        rule = test_policy_engine.rule('risk.crap', {'type':'decimal','value':'30'}, operator=op,
                                      ratchet={'deny_regression':True,'allow_legacy_debt':True})
        add('decision', rule, {'type':'decimal','value':str(after)}, {'type':'decimal','value':str(before)})
# Report-only shapes exercise empty indexes, absent subject IDs, optional failures,
# and components with no gates using the reference aggregate semantics.
f = test_project_report.ProjectReportTests()
f.setUp()
project_value, policy_value = plain(f.project), plain(f.policy)
base_result = original_evaluate(f.policy, f.records, **f.context)
for name, result in [('empty', {**base_result, 'results': []}),
                     ('missing-subject', {**base_result, 'results': [
                         {**base_result['results'][0], 'subject': None}]})]:
    cases.append(dict(name='report-boundary/' + name, kind='report',
                      args=[result, project_value, policy_value], kwargs={},
                      oracle=outcome(lambda: project_report.report(result, project_value, policy_value))))
(WORK / 'cases.json').write_text(json.dumps(cases, ensure_ascii=False))
print(json.dumps(dict(frozen_cases=len(manifest['cases']), reference_tests=run.testsRun,
                     comparisons=len(cases), kinds={k: sum(c['kind'] == k for c in cases)
                     for k in sorted({c['kind'] for c in cases})})))
