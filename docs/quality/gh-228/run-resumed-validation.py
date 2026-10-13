import json
import os
from pathlib import Path
import subprocess
import time
import sys

root = Path.cwd()
directory = root / 'target/gh-228' / os.environ.get('GH228_VALIDATION_RUN', 'resumed-validation')
directory.mkdir(exist_ok=True)
(directory / 'logs').mkdir(exist_ok=True)
environment = os.environ | {
    'CARGO_TARGET_DIR': str(root / 'target/gh-228/cargo'),
    'TMPDIR': str(root / 'target/gh-228/tmp'),
    'OPENSPEC_TELEMETRY': '0',
    'GIT_CEILING_DIRECTORIES': str(root / 'target/gh-228/tmp'),
    'NATIVE_DRIVER': str(root / 'target/gh-228/operator-native-runtime/build/debug/harness-gate-rust-native-driver'),
    'NATIVE_DRIVER_SYSROOT': subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip(),
    'HARNESS_GATE_NATIVE_POLICY_BINARY': str(root / 'target/gh-228/cargo/debug/harness-gate'),
    'RUST_COLLECTOR_TEST_VENDOR': str(root / 'target/gh-228/project-inputs/vendor'),
    'RUST_COLLECTOR_RUNTIME': str(root / 'target/gh-228/bundle-v6-one'),
}
# Local webhook tests need direct localhost connections, as recorded by GH-227.
removed = ['HTTP_PROXY', 'http_proxy', 'HTTPS_PROXY', 'https_proxy', 'ALL_PROXY', 'all_proxy']
for key in removed:
    environment.pop(key, None)
commands = [
    ('nextest', ['cargo', 'nextest', 'run', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--locked']),
    ('fmt', ['cargo', 'fmt', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--', '--check']),
    ('clippy', ['cargo', 'clippy', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--all-targets', '--', '-D', 'warnings']),
    ('python', ['python3', '-m', 'unittest', 'discover', '-s', 'tools/quality/tests', '-v']),
    ('docs', ['python3', 'tools/quality/docs_consistency.py', '--output', 'target/quality/docs-consistency.json']),
    ('openspec', ['openspec', 'validate', 'package-official-rust-collector-for-independent-delivery', '--strict', '--no-interactive']),
    ('diff', ['git', 'diff', '--check']),
]
selected = sys.argv[1:] or [name for name, _ in commands]
record = directory / ('-'.join(selected) + '.json')
if record.exists():
    raise SystemExit('Refusing to overwrite prior results: ' + str(record))
results = []
for name, command in commands:
    if name not in selected:
        continue
    started = time.monotonic()
    print('Starting ' + name, flush=True)
    with (directory / 'logs' / (name + '.log')).open('wb') as output:
        result = subprocess.run(command, env=environment, stdout=output, stderr=subprocess.STDOUT)
    row = {'name': name, 'command': command, 'exit_code': result.returncode,
           'seconds': round(time.monotonic() - started, 3)}
    results.append(row)
    record.write_text(json.dumps({
        'cwd': str(root), 'environment': {key: environment[key] for key in ('CARGO_TARGET_DIR', 'TMPDIR', 'GIT_CEILING_DIRECTORIES', 'OPENSPEC_TELEMETRY', 'NATIVE_DRIVER', 'NATIVE_DRIVER_SYSROOT', 'HARNESS_GATE_NATIVE_POLICY_BINARY', 'RUST_COLLECTOR_RUNTIME', 'RUST_COLLECTOR_TEST_VENDOR')},
        'removed_environment': removed, 'results': results,
        'not_applicable': ['harness-gate config check', 'harness-gate verify --profile ci --all'],
        'reason': 'No .harness-gate/flow.toml declaring ci exists.'}, indent=2) + '\n')
    print(json.dumps(row), flush=True)
