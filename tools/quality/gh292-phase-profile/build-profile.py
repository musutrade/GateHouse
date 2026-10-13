#!/usr/bin/env python3
"""Build the external Linux phase driver and bind its actual compiler artifact."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--source-root', type=Path, required=True)
p.add_argument('--source-identity', required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--toolchain', default='1.99.0')
a = p.parse_args()
root, out, package = a.source_root.resolve(), a.output.resolve(), Path(__file__).resolve().parent
out.mkdir(parents=True, exist_ok=False)
(out / 'tmp').mkdir()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
sources = {}
for directory in ('tools/harness-gate/quality-core', 'tools/quality/schemas'):
    for path in sorted((root / directory).rglob('*')):
        if path.is_file():
            sources[str(path.relative_to(root))] = sha(path)
for relative in ('tools/harness-gate/src/utils/redaction.rs', 'tools/harness-gate/src/verify/report.rs'):
    sources[relative] = sha(root / relative)
package_hashes = {str(path.relative_to(package)): sha(path) for path in sorted(package.rglob('*')) if path.is_file()}
tools = {}
for name in ('rustc', 'cargo'):
    path = Path(subprocess.check_output(['rustup', 'which', '--toolchain', a.toolchain, name], text=True).strip())
    tools[name] = {'path': str(path), 'sha256': sha(path),
                   'version': subprocess.check_output([str(path), '-vV' if name == 'rustc' else '--version'], text=True)}
env = os.environ.copy()
for key in ('RUSTC', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER',
            'RUSTC_WORKSPACE_WRAPPER', 'LLVM_PROFILE_FILE', 'CARGO_LLVM_COV_TARGET_DIR'):
    env.pop(key, None)
env.update(GH292_SOURCE_ROOT=str(root), CARGO_TARGET_DIR=str(out / 'target'), TMPDIR=str(out / 'tmp'),
           CARGO_BUILD_JOBS='4', CARGO_TERM_COLOR='never')
argv = ['cargo', '+' + a.toolchain, 'build', '--locked', '--release', '--manifest-path',
        str(package / 'Cargo.toml'), '--message-format=json']
record = {'status': 'RUNNING', 'source_identity': a.source_identity, 'source_root': str(root),
          'source_hashes': sources, 'package_hashes': package_hashes, 'toolchain': tools,
          'argv': argv, 'cwd': str(package), 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'effective_env': {key: env.get(key) for key in ('GH292_SOURCE_ROOT', 'CARGO_TARGET_DIR', 'TMPDIR',
                                                        'CARGO_BUILD_JOBS', 'RUSTFLAGS', 'RUSTC', 'CARGO_BUILD_TARGET')},
          'cleared_compiler_environment': True}
(out / 'build-record.json').write_text(json.dumps(record, indent=2) + '\n')
with (out / 'stdout.log').open('xb') as stdout, (out / 'stderr.log').open('xb') as stderr:
    result = subprocess.run(argv, cwd=package, env=env, stdout=stdout, stderr=stderr)
record['returncode'] = result.returncode
record['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
record['logs_sha256'] = {name: sha(out / name) for name in ('stdout.log', 'stderr.log')}
if result.returncode:
    record['status'] = 'FAIL'
    (out / 'build-record.json').write_text(json.dumps(record, indent=2) + '\n')
    raise SystemExit('Build failed; raw logs retained.')
artifacts = []
for line in (out / 'stdout.log').read_text().splitlines():
    item = json.loads(line)
    if item.get('reason') == 'compiler-artifact' and item.get('executable') and item['target']['name'] == 'gh292-phase-profile':
        artifacts.append(item)
assert len(artifacts) == 1
binary = Path(artifacts[0]['executable']).resolve()
assert all(sha(root / name) == value for name, value in sources.items())
assert all(sha(package / name) == value for name, value in package_hashes.items())
assert all(sha(Path(tool['path'])) == tool['sha256'] for tool in tools.values())
generated = {}
for filename in ('source-bindings.rs', 'report-redaction-functions.rs'):
    paths = list((out / 'target/release/build').glob('*/out/' + filename))
    assert len(paths) == 1
    path = paths[0]
    (out / filename).write_bytes(path.read_bytes())
    generated[filename] = {'original_path': str(path), 'sha256': sha(path)}
record.update(status='PASS', binary=str(binary), binary_sha256=sha(binary),
              compiler_artifact=artifacts[0], generated_sources=generated)
(out / 'build-record.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'record': str(out / 'build-record.json'), 'binary': str(binary), 'sha256': sha(binary)}))
