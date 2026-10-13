"""Retain this workspace's actual GH-229 logs and synthetic rehearsal bytes."""
import hashlib
import json
from pathlib import Path
import tarfile

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'target/gh-229'
DEST = Path(__file__).resolve().parent


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def write(name, value):
    (DEST / name).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


local = {'CARGO_TARGET_DIR': str(SOURCE / 'cargo'), 'TMPDIR': str(SOURCE / 'tmp')}
isolated = local | {'GIT_CEILING_DIRECTORIES': str(SOURCE / 'tmp')}
proxy_names = ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'http_proxy', 'https_proxy', 'all_proxy']
nextest = ['cargo', 'nextest', 'run', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--locked']
records = []


def record(log, argv, result, environment=None, removed=None, note=None):
    path = SOURCE / 'logs' / log
    records.append({'log': 'logs/' + log, 'log_sha256': sha(path), 'argv': argv,
                    'result': result, 'environment_overrides': environment or {},
                    'environment_removed': removed or [], 'note': note})


# Result values below are receipts of observed tool completions, not inferred
# from empty logs. Rerunning this retention script does not rerun any check.
record('nextest.log', nextest, {'exit_code': 100, 'passed': 39, 'failed': 1, 'not_run': 352}, local)
record('nextest-isolated.log', nextest, {'exit_code': 100, 'passed': 354, 'failed': 1, 'not_run': 37}, isolated)
record('nextest-final.log', nextest, {'exit_code': 0, 'passed': 392, 'skipped': 0}, isolated, proxy_names)
record('fmt.log', ['cargo', 'fmt', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--', '--check'],
       {'exit_code': 0}, {'CARGO_TARGET_DIR': local['CARGO_TARGET_DIR']})
record('clippy.log', ['cargo', 'clippy', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--all-targets', '--', '-D', 'warnings'],
       {'exit_code': 0}, local)
record('quality-tests.log', ['python3', '-m', 'unittest', 'discover', '-s', 'tools/quality/tests', '-v'],
       {'exit_code': 0, 'tests': 402, 'skipped_classes': 3}, isolated)
record('release-tests.log', ['python3', '-m', 'unittest', 'discover', '-s', 'tools/release/tests', '-v'],
       {'exit_code': 0, 'tests': 41})
record('release-tests-final.log', ['python3', '-m', 'unittest', 'discover', '-s', 'tools/release/tests', '-v'],
       {'exit_code': 0, 'tests': 42}, {'TMPDIR': local['TMPDIR']})
record('delivery-initial.log', None, {'unittest_status': 'failed', 'failed_subtests': 2, 'shell_wrapper_exit_code': 0},
       note='Intermediate unittest invocation; exact filter/wrapper not retained. Log is failed evidence, not a passed command.')
record('delivery-signature-fix.log', None, {'exit_code': 0, 'tests': 12},
       note='Intermediate focused delivery rerun; exact filter not retained. Complete final release invocation above is authoritative.')
record('core-install.log', ['bash', 'tools/release/tests/test_install.sh'], {'exit_code': 0}, {'TMPDIR': local['TMPDIR']})
for suffix, directory in [('', 'dry-run'), ('-final', 'dry-run-final')]:
    record('dry-run' + suffix + '.log', ['python3', 'tools/release/collector_dry_run.py', '--output', 'target/gh-229/' + directory],
           {'exit_code': 0, 'synthetic': True, 'native_measurement': 'not_performed', 'publication': 'not_attempted'},
           {'TMPDIR': local['TMPDIR']} if suffix else {})
docs = ['python3', 'tools/quality/docs_consistency.py', '--output', 'target/quality/docs-consistency.json']
record('docs.log', docs, {'exit_code': 1})
record('docs-final.log', docs, {'exit_code': 0}, isolated)
spec = ['openspec', 'validate', 'package-official-rust-collector-for-independent-delivery', '--strict', '--no-interactive']
record('openspec.log', spec, {'status': 'tool_rejected', 'exit_code': None},
       note='Tool rejected telemetry access to https://edge.openspec.dev:443 after validation message; not passed.')
record('openspec-final.log', spec, {'exit_code': 0}, {'OPENSPEC_TELEMETRY': '0'})
record('diff.log', ['git', 'diff', '--check'], {'exit_code': 0})
write('validation.json', {'issue': 229, 'baseline': '9bdc203c21a75cc769fdb2adbafb897e4b96a30e',
                        'cwd': str(ROOT), 'commands': records,
                        'project_config_check': 'not_applicable: no .harness-gate/flow.toml',
                        'project_ci_verify': 'not_applicable: no declared ci profile',
                        'hosted_required_ci': 'controller_pending', 'native_positive': 'not_performed'})
(DEST / 'docs-consistency.json').write_bytes((ROOT / 'target/quality/docs-consistency.json').read_bytes())

archives = []
for output, directories in [('validation-logs.tar.gz', ['logs']),
                            ('dry-run-evidence.tar.gz', ['dry-run', 'dry-run-final'])]:
    members = []
    with tarfile.open(DEST / output, 'w:gz') as archive:
        for directory in directories:
            for path in sorted((SOURCE / directory).rglob('*')):
                if path.is_file():
                    assert not path.is_symlink()
                    name = str(path.relative_to(SOURCE))
                    archive.add(path, arcname=name, recursive=False)
                    members.append({'path': name, 'bytes': path.stat().st_size, 'sha256': sha(path)})
    archives.append({'path': output, 'bytes': (DEST / output).stat().st_size,
                     'sha256': sha(DEST / output), 'members': members})
write('retained-evidence.json', {'schema': 'gh-229-retained-evidence/v1', 'archives': archives})
print(json.dumps({'archives': [{k: v for k, v in a.items() if k != 'members'} for a in archives]}))
