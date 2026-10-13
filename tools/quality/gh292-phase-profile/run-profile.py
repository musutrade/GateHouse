#!/usr/bin/env python3
"""Run a selected frozen binary on the same existing inputs three times each."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--build-record', type=Path, required=True)
p.add_argument('--inputs', type=Path, required=True)
p.add_argument('--source-root', type=Path, required=True)
p.add_argument('--source-identity', required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--toolchain', default='1.99.0')
p.add_argument('--mode', choices=['before', 'after'], required=True)
p.add_argument('--baseline-summary', type=Path)
a = p.parse_args()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
binary, inputs, root, out = [x.resolve() for x in (a.binary, a.inputs, a.source_root, a.output)]
out.mkdir(parents=True, exist_ok=False)
manifest = {str(path.relative_to(inputs)): sha(path) for path in sorted(inputs.rglob('*')) if path.is_file()}
prepared_path = inputs / 'fixture-input-manifest.json'
prepared = json.loads(prepared_path.read_text())
assert prepared == {key: value for key, value in manifest.items() if key != 'fixture-input-manifest.json'}
assert (a.mode == 'after') == (a.baseline_summary is not None)
baseline = json.loads(a.baseline_summary.read_text()) if a.baseline_summary else None
if baseline:
    assert baseline['identity']['input_manifest'] == manifest
    assert baseline['identity']['input_root'] == str(inputs)
sources = {}
for directory in ('tools/harness-gate/quality-core', 'tools/quality/schemas'):
    for path in sorted((root / directory).rglob('*')):
        if path.is_file():
            sources[str(path.relative_to(root))] = sha(path)
for relative in ('tools/harness-gate/src/utils/redaction.rs', 'tools/harness-gate/src/verify/report.rs'):
    sources[relative] = sha(root / relative)
tools = {}
for name in ('rustc', 'cargo'):
    path = Path(subprocess.check_output(['rustup', 'which', '--toolchain', a.toolchain, name], text=True).strip())
    version = subprocess.check_output([str(path), '-vV' if name == 'rustc' else '--version'], text=True)
    tools[name] = {'path': str(path), 'sha256': sha(path), 'version': version}
build_record = json.loads(a.build_record.read_text())
assert build_record['status'] == 'PASS'
assert binary == Path(build_record['binary']).resolve()
assert sha(binary) == build_record['binary_sha256']
assert root == Path(build_record['source_root']).resolve()
assert sources == build_record['source_hashes']
assert tools == build_record['toolchain']
assert a.source_identity == build_record['source_identity']
identity = {'mode': a.mode, 'prepared_manifest_sha256': sha(prepared_path), 'baseline_summary': str(a.baseline_summary) if a.baseline_summary else None, 'build_record': str(a.build_record.resolve()), 'build_record_sha256': sha(a.build_record), 'source_identity': a.source_identity, 'source_root': str(root), 'source_hashes': sources,
            'toolchain': tools, 'binary': str(binary), 'binary_sha256': sha(binary),
            'input_root': str(inputs), 'input_manifest': manifest,
            'cache_condition': 'Fresh processes; OS page cache uncontrolled and previously touched; no cold-cache claim.'}
(out / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')
rows = []
env = os.environ.copy()
env['TMPDIR'] = str(out)
for fixture in ('small', 'large'):
    for sample in range(1, 4):
        name = f'{fixture}-{sample}'
        argv = [str(binary), str(inputs / 'inputs' / fixture), str(out / name)]
        invocation = {'argv': argv, 'cwd': str(out), 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                      'effective_environment': {'TMPDIR': env['TMPDIR']}, 'identity': str(out / 'identity.json')}
        result = subprocess.run(argv, cwd=out, env=env, capture_output=True)
        (out / f'{name}.stdout.log').write_bytes(result.stdout)
        (out / f'{name}.stderr.log').write_bytes(result.stderr)
        invocation['returncode'] = result.returncode
        invocation['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (out / f'{name}.invocation.json').write_text(json.dumps(invocation, indent=2) + '\n')
        if result.returncode:
            raise SystemExit(f'{name} failed; later samples stopped, raw logs retained')
        measurement = json.loads((out / name / 'measurement.json').read_text())
        assert sha(out / name / 'quality-project-report.json') == measurement['report_sha256']
        rows.append({'fixture': fixture, 'sample': sample, 'measurement': measurement})
assert manifest == {str(path.relative_to(inputs)): sha(path) for path in sorted(inputs.rglob('*')) if path.is_file()}
assert sha(binary) == identity['binary_sha256']
medians = {}
for fixture in ('small', 'large'):
    records = [row['measurement'] for row in rows if row['fixture'] == fixture]
    assert len({row['report_sha256'] for row in records}) == 1
    medians[fixture] = {key: statistics.median(row[key] for row in records)
                        for key in ('wall_seconds', 'cpu_seconds', 'peak_rss_kib', 'report_bytes')}
    medians[fixture]['report_sha256'] = records[0]['report_sha256']
    medians[fixture]['phases'] = [{
        'phase': phase['phase'],
        'wall_seconds': statistics.median(row['phases'][index]['wall_seconds'] for row in records),
        'cpu_seconds': statistics.median(row['phases'][index]['cpu_seconds'] for row in records),
        'kernel_io_delta': {key: statistics.median(row['phases'][index]['kernel_io_delta'][key] for row in records)
                            for key in phase['kernel_io_delta']},
    } for index, phase in enumerate(records[0]['phases'])]
if baseline:
    assert baseline['identity']['toolchain'] == tools
    for fixture in ('small', 'large'):
        assert medians[fixture]['report_sha256'] == baseline['medians'][fixture]['report_sha256']
        assert medians[fixture]['report_bytes'] == baseline['medians'][fixture]['report_bytes']
(out / 'summary.json').write_text(json.dumps({'identity': identity, 'samples': rows, 'medians': medians}, indent=2) + '\n')
(out / 'sha256.json').write_text(json.dumps({str(path.relative_to(out)): sha(path) for path in sorted(out.rglob('*'))
                                            if path.is_file()}, indent=2) + '\n')
print(json.dumps(medians, indent=2))
