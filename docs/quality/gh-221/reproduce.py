#!/usr/bin/env python3
"""Reconcile pinned sources and inspect GH-220 evidence without historical binaries."""
import argparse
from collections import Counter
import gzip
import json
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/quality'))
import rust_native_driver as native
import rust_native_classify as classify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    native.require(output.is_relative_to(ROOT / 'target'), 'output must be inside this workspace target/')
    output.mkdir(parents=True, exist_ok=True)
    previous = ROOT / 'docs/quality/gh-220'
    index = classify.read_json(previous / 'production-artifacts.json')
    committed = {Path(p['path']).name: p for p in index['committed_files']}
    source_index = classify.read_json(previous / 'artifact-index.json')
    committed.update({Path(p['path']).name: p for p in source_index['committed_files']})
    inputs = []
    for name in ('production-raw.tar.xz', 'production-report.json.gz', 'arc-admin-e5a1ee5.tar.gz'):
        entry = committed[name]
        path = ROOT / entry['path']
        native.require(native.file_hash(path) == entry['sha256'] and path.stat().st_size == entry['bytes'],
                       'committed artifact mismatch: ' + name)
        inputs.append(entry)
    raw = output / 'backend'
    if not raw.exists():
        with tarfile.open(previous / 'production-raw.tar.xz') as archive:
            archive.extractall(output, filter='data')
    evidence = classify.load_evidence(raw, index['anchor'], previous / 'production-report.json.gz',
                                     committed['production-report.json.gz']['sha256'])
    result = classify.classify(evidence)
    (output / 'backend-classification.json.gz').write_bytes(gzip.compress(
        (json.dumps(result, indent=2, sort_keys=True) + '\n').encode(), mtime=0))

    snapshots = {}
    for name, path in [('gh219', ROOT / 'docs/quality/rust-native-inventory-replay.json'),
                       ('gh220_baseline', previous / 'source-replay-baseline.json'),
                       ('gh220_final', previous / 'source-replay-final.json')]:
        data = classify.read_json(path)
        snapshots[name] = {p['path'].removeprefix('backend/'): p['sha256'] for p in data['files']}
    sources = {p['relative']: p['sha256'] for p in result['files']}
    native.require(len(sources) == 47 and all(value == sources for value in snapshots.values()),
                   'complete 47-source replay identities differ')

    archive_path = previous / 'arc-admin-e5a1ee5.tar.gz'
    command = ['gzip', '-t', str(archive_path.relative_to(ROOT))]
    checked = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    extracted, extraction_error = {}, None
    try:
        with tarfile.open(archive_path, mode='r|gz') as archive:
            for member in archive:
                if '/backend/' in member.name:
                    relative = member.name.split('/backend/', 1)[1]
                    if member.isfile() and relative in sources:
                        native.require(relative not in extracted, 'duplicate archive source')
                        extracted[relative] = archive.extractfile(member).read()
    except (EOFError, tarfile.TarError) as exc:
        extraction_error = str(exc)
    native.require({p: native.digest(b) for p, b in extracted.items()} == sources,
                   'archive backend source hash inventory differs')
    reconciled = []
    for path, sha in sorted(sources.items()):
        retained = native.file_hash(raw / 'source' / path)
        native.require(retained == sha, 'retained source differs: ' + path)
        compiler = [{'unit': u['id'], 'source_id': s['source_id'], 'file': s['file'], 'sha256': s['sha256']}
                    for u in evidence['units'] if u['production'] for s in u['inventory']['sources']
                    if s['source'] is not None and evidence['capture']['production_sources'].get(
                        native.source_key(s['file'], u['cwd']), {}).get('relative') == path]
        native.require(compiler and all(s['sha256'] == sha for s in compiler), 'compiler source differs: ' + path)
        reconciled.append({'relative': path, 'sha256': sha, 'archive_sha256': native.digest(extracted[path]),
                           'retained_sha256': retained, 'replays': {k: v[path] for k, v in snapshots.items()},
                           'compiler_sources': compiler})
    native.write_json(output / 'source-reconciliation.json', {
        'source_commit': 'e5a1ee5f7ec6dbae461355106d469384581d4607', 'inputs': inputs,
        'complete_backend_sources_verified': True, 'source_count': len(reconciled),
        'archive_integrity': {'command': command, 'exit_code': checked.returncode,
                              'stderr': checked.stderr, 'stream_extraction_error': extraction_error,
                              'scope': 'All 47 backend source members reconcile; whole archive is truncated.'},
        'files': reconciled})

    # These names identify the issue's historical question; they are never classifier rules.
    historical = ['src/handlers/mod.rs', 'src/permissions.rs', 'src/permissions/departments.rs',
                  'src/repositories/mod.rs', 'src/services/mod.rs']
    rows = {p['relative']: p for p in result['files']}
    five = []
    for path in historical:
        row = rows[path]
        children = [d['name'] for d in row['definitions'] if d['kind'] == 'Mod']
        child_owners = [{'owner': f['owner'], 'unit': f['unit'], 'name': f['name'],
                         'source_lines': f['source_lines'], 'instances': f['instances']}
                        for f in evidence['report']['functions']
                        if any(f['name'].startswith(c + '::') for c in children)]
        five.append(row | {'source': extracted[path].decode(), 'child_module_runtime_owners': child_owners})
    auth = rows['src/auth.rs']
    native.write_json(output / 'five-file-provenance.json', {
        'identity': result['identity'], 'verification': result['verification'], 'files': five,
        'permission_runtime_consumers': [o for o in auth['owners'] if 'RequirePermission' in o['name']],
        'permission_runtime_source': {'relative': 'src/auth.rs', 'sha256': sources['src/auth.rs'],
                                      'source': extracted['src/auth.rs'].decode()},
        'module_exports': {'relative': 'src/lib.rs', 'sha256': sources['src/lib.rs'],
                           'source': extracted['src/lib.rs'].decode()},
        'historical_llvm': {'series': 'llvm-file-summary-unfiltered/1', 'status': 'measurement_error',
                            'issue_observation': {'source_files': 47, 'llvm_file_records': 42},
                            'files': [{'relative': p, 'status': 'measurement_error', 'metrics': None}
                                      for p in historical],
                            'reason': 'No authenticated original LLVM file-summary capture/configuration; '
                                      'MIR replay cannot certify or replace that history.'}})
    native.write_json(output / 'file-summary.json', {
        'identity_sha256': result['identity_sha256'], 'counts': result['counts'],
        'classification_complete': result['classification_complete'], 'measurement_passed': result['measurement_passed'],
        'files': [{k: row[k] for k in ('relative', 'sha256', 'status', 'reason', 'metrics')} |
                  {'definition_roles': dict(Counter(d['role'] for d in row['definitions'])),
                   'runtime_owners': len(row['owners']), 'execution': row.get('execution')}
                  for row in result['files']]})
    print(json.dumps({'sources_verified': len(reconciled), 'classification': result['counts'],
                      'archive_integrity_exit': checked.returncode, 'measurement_passed': result['measurement_passed']}))


if __name__ == '__main__':
    main()
