#!/usr/bin/env python3
"""Compare public archives and actual AST/complexity outputs without compiling."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/quality'))
from source_measure import SOURCE_FILES, SERIES, compiler_configuration, complexity
from measurer_release import load_measurer_pin
from production_coverage import require


def digest(data):
    return hashlib.sha256(data).hexdigest()


def payload(archive, pin):
    require(digest(archive.read_bytes()) == pin['sha256'], 'archive digest mismatch')
    root = pin['asset'].removesuffix('.tar.gz')
    with tarfile.open(archive) as tar:
        return {name: tar.extractfile(root + '/' + name).read()
                for name in (pin['executable'], 'plugin.json')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--legacy-archive', type=Path, required=True)
    parser.add_argument('--new-archive', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    legacy = json.loads((Path(__file__).parent / 'legacy-pin-v1.json').read_text())
    current = load_measurer_pin()
    old, new = payload(args.legacy_archive, legacy), payload(args.new_archive, current)
    require(old == new, 'binary or capability bytes differ: requires series migration review')
    inputs = {}
    for source in SOURCE_FILES:
        path = ROOT / 'tools/harness-gate/src' / source
        if path.is_file():
            inputs[source] = path.read_text()
    # Retain the fixed AST boundary cases already used by Core regression tests.
    tree = ast.parse((ROOT / 'tools/quality/tests/test_source_measure.py').read_text())
    for node in tree.body:
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
            names = [n.id for n in node.targets if isinstance(n, ast.Name)]
            if names and ('fn ' in node.value.value or 'pub fn ' in node.value.value):
                inputs['fixture:' + names[0]] = node.value.value
    records = []
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        binaries = []
        for label, data in (('old', old), ('new', new)):
            binary = work / label
            binary.write_bytes(data[current['executable']])
            binary.chmod(0o755)
            binaries.append(binary)
        source = work / 'input.rs'
        for target in ('x86_64-unknown-linux-gnu', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc'):
            configuration = compiler_configuration(target)
            for name, text in inputs.items():
                source.write_text(text)
                results = [subprocess.run([str(binary), str(source), '--target-cfg'],
                    input=json.dumps(configuration), text=True, capture_output=True) for binary in binaries]
                require([(r.returncode, r.stdout, r.stderr) for r in results[:1]] == [(r.returncode, r.stdout, r.stderr) for r in results[1:]], name)
                result = results[0]
                row = {'input': name, 'input_sha256': digest(text.encode()), 'target': target,
                       'exit_code': result.returncode, 'output_sha256': digest((result.stdout + result.stderr).encode())}
                if result.returncode == 0:
                    inventory = json.loads(result.stdout)
                    require((inventory['analyzer'], inventory['version'], inventory['rule']) == (current['analyzer'], current['version'], current['rule']), 'actual analyzer identity mismatch')
                    row['symbols'] = len(inventory['symbols'])
                    row['complexities_sha256'] = digest(json.dumps([complexity(s['raw']) for s in inventory['symbols']], separators=(',', ':')).encode())
                records.append(row)
    report = {'schema': 'measurer-continuity/v1', 'status': 'pass', 'series': SERIES,
              'legacy_archive_sha256': legacy['sha256'], 'new_archive_sha256': current['sha256'],
              'binary_sha256': digest(new[current['executable']]), 'plugin_manifest_sha256': digest(new['plugin.json']),
              'binary_and_manifest_byte_identical': True, 'inputs': len(inputs), 'comparisons': len(records),
              'records': records,
              'lineage': 'No metric or series migration. Identical binary and manifest; unchanged measurement/instrumentation/join/ratchet code. Provenance changes explicitly; base and head are measured with the same new tool declaration. Retained historical reports remain immutable.'}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(f'PASS: {len(records)} AST/complexity comparisons; executable and capability bytes identical')


if __name__ == '__main__':
    main()
