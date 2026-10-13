#!/usr/bin/env python3
"""Explicit-policy scope acceptance, separate from the frozen compiler oracle.

Retain every request, source/artifact byte, exit status, stdout and stderr under
--output. No compiler state is passed to the evaluate entry point.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil

from acceptance import evaluate, prepare, subject_id, write


def two_components(root):
    _, direct = prepare(root)
    component = copy.deepcopy(direct['project']['components'][0])
    component['id'] = 'failing'
    component['path'] = 'failing'
    component['source_boundaries'][0]['path'] = 'failing/src'
    direct['project']['components'].append(component)
    record = copy.deepcopy(direct['records'][0])
    record['id'] = 'failing-coverage'
    record['component'] = 'failing'
    subject = record['subject']
    subject['component'] = 'failing'
    subject['path'] = 'failing/src/lib.rs'
    subject['id'] = subject_id(direct['project']['id'], subject)
    direct['project']['subjects'].append(copy.deepcopy(subject))
    direct['project']['subjects'].sort(key=lambda item: item['id'])
    (root / 'failing/src').mkdir(parents=True)
    shutil.copyfile(root / 'src/lib.rs', root / subject['path'])
    record['source']['path'] = subject['path']
    record['artifacts'][0]['source']['path'] = subject['path']
    record['metrics'][0]['value']['covered'] = 1
    raw = b'{"covered": 1, "total": 5}\n'
    (root / 'target/evidence/failing-raw.json').write_bytes(raw)
    artifact = record['artifacts'][0]
    artifact['path'] = 'failing-raw.json'
    artifact['bytes'] = len(raw)
    artifact['sha256'] = hashlib.sha256(raw).hexdigest()
    direct['records'].append(record)
    return direct


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--harness-gate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary, output = args.harness_gate.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    results = []
    for name, scope, expected_status, expected_error in [
        ('component-pass-control', dict(kind='component', component='app'), 0, None),
        ('project-includes-failing-component', dict(kind='project'), 1, None),
        ('project-extra-component', dict(kind='project', component='app'), 1,
         '$: unknown field(s): component'),
        ('project-extra-boundary', dict(kind='project', boundary='source'), 1,
         '$: unknown field(s): boundary'),
        ('unknown-kind', dict(kind='future-scope'), 1, 'unknown policy scope kind'),
    ]:
        root = output / name
        direct = two_components(root)
        direct['policy']['rules'][0]['scope'] = scope
        write(root / 'case.json', direct)
        # Seed a valid pass first, then reuse its output path. Invalid input must
        # not leave that earlier pass available as this invocation's report.
        if expected_error is not None:
            control = copy.deepcopy(direct)
            control['policy']['rules'][0]['scope'] = dict(kind='component', component='app')
            seeded = evaluate(binary, root, control, False)
            (root / 'seed.stdout').write_text(seeded.stdout)
            (root / 'seed.stderr').write_text(seeded.stderr)
            assert seeded.returncode == 0, seeded.stderr
            assert json.loads((root / 'direct-report.json').read_text())['aggregate']['state'] == 'pass'
        result = evaluate(binary, root, direct, False)
        (root / 'stdout.log').write_text(result.stdout)
        (root / 'stderr.log').write_text(result.stderr)
        write(root / 'exit.json', dict(status=result.returncode, expected=expected_status,
                                      expected_error=expected_error))
        assert result.returncode == expected_status, (name, result.returncode, result.stderr)
        report_path = root / 'direct-report.json'
        if expected_error is not None:
            assert result.stderr.strip() == 'ERROR [E1000]: command failed: ' + expected_error, result.stderr
            assert not report_path.exists(), (name, 'stale pass report survived invalid input')
        else:
            report = json.loads(report_path.read_text())
            assert report['aggregate']['state'] == ('fail' if expected_status else 'pass'), report
            gates = report['policy_result']['results']
            assert len(gates) == (2 if expected_status else 1), gates
            if expected_status:
                assert {gate['subject'] for gate in gates} == {subject['id'] for subject in direct['project']['subjects']}
                assert {gate['state'] for gate in gates} == {'pass', 'fail'}
                failing = next(gate for gate in gates if gate['state'] == 'fail')
                assert failing['record']['component'] == 'failing'
        results.append(dict(case=name, exit=result.returncode))
    write(output / 'acceptance.json', dict(status='pass', cases=results))


if __name__ == '__main__':
    main()
