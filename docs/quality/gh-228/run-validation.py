import json
import os
from pathlib import Path
import subprocess
import time

root = Path.cwd()
directory = root / 'target/gh-228'
environment = os.environ | {
    'CARGO_TARGET_DIR': str(directory / 'cargo'),
    'TMPDIR': str(directory / 'tmp'),
    'OPENSPEC_TELEMETRY': '0',
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
results = []
for name, command in commands:
    started = time.monotonic()
    print('Starting ' + name, flush=True)
    with (directory / 'logs' / (name + '.log')).open('wb') as output:
        result = subprocess.run(command, env=environment, stdout=output, stderr=subprocess.STDOUT)
    row = {'name': name, 'command': command, 'exit_code': result.returncode,
           'seconds': round(time.monotonic() - started, 3)}
    results.append(row)
    (directory / 'validation.json').write_text(json.dumps({
        'cwd': str(root), 'environment': {key: environment[key] for key in ('CARGO_TARGET_DIR', 'TMPDIR', 'OPENSPEC_TELEMETRY')},
        'removed_environment': removed, 'results': results,
        'not_applicable': ['harness-gate config check', 'harness-gate verify --profile ci --all'],
        'reason': 'No .harness-gate/flow.toml declaring ci exists.'}, indent=2) + '\n')
    print(json.dumps(row), flush=True)
