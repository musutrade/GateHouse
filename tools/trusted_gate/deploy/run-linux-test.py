#!/usr/bin/python3 -I
"""Execute the existing Linux Test workload; Core retains final quality authority."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import time

SPEC = importlib.util.spec_from_file_location('worker_inputs',
    Path(__file__).with_name('prepare-linux-workspace.py'))
INPUTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INPUTS)
ONBOARDING_COMMIT = '53c01505bed2ff61dfad1d106abbec2dcd961d05'
ONBOARDING_SHA256 = '152196af414d22f6409ba99e14eebb950823280863e05af6ecd95edfece6936d'
WORKFLOW_SHA256 = '69a811afc4ca0db934813945110fc0e28cc2411ffaab4f64762fa3ae068c24a8'


def built_core_digest(path, snapshot=None):
    # Cargo legitimately hard-links its newly built executable into debug/deps.
    # Accept that generated output only; immutable input guards remain unchanged.
    path = Path(path)
    for item in (path, *path.parents):
        INPUTS.need(not item.is_symlink(), 'linked compiled Core path')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as source:
        info = os.fstat(source.fileno())
        INPUTS.need(stat.S_ISREG(info.st_mode) and info.st_size <= 256 * 1024 * 1024,
                    'compiled Core is special or oversized')
        value = hashlib.file_digest(source, 'sha256').hexdigest()
        if snapshot is not None:
            source.seek(0)
            with Path(snapshot).open('xb') as target:
                shutil.copyfileobj(source, target)
                os.fchmod(target.fileno(), 0o700)
            INPUTS.need(INPUTS.digest(INPUTS.regular(snapshot, 256 * 1024 * 1024)) == value,
                        'compiled Core changed during snapshot')
        INPUTS.need(os.fstat(source.fileno()).st_size == info.st_size, 'compiled Core changed while reading')
    return value


def directory_digest(directory):
    """Pin a separately reviewed snapshot of the exact CI onboarding commit."""
    directory = Path(directory)
    files, total = {}, 0
    for parent in (directory, *directory.parents):
        INPUTS.need(not parent.is_symlink(), 'linked onboarding source')
    INPUTS.need(directory.is_dir(), 'missing onboarding source')
    for root, directories, names in os.walk(directory):
        for name in directories:
            INPUTS.need(name != '.git' and not (Path(root) / name).is_symlink(), 'unsafe onboarding directory')
        for name in names:
            path = Path(root) / name
            data = INPUTS.regular(path, 128 * 1024 * 1024)
            total += len(data)
            files[str(path.relative_to(directory))] = [len(data), INPUTS.digest(data),
                                                       bool(path.stat().st_mode & 0o111)]
            INPUTS.need(len(files) <= 20000 and total <= INPUTS.MAX_BYTES, 'onboarding source exceeds budget')
    INPUTS.need(files and 'tools/onboarding/acceptance.py' in files, 'incomplete onboarding source')
    return INPUTS.digest(json.dumps(files, sort_keys=True, separators=(',', ':')).encode())


def clean_environment(output, head, identity, supplied):
    forbidden = {'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS', 'RUSTC', 'RUSTDOC',
                 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_INCREMENTAL',
                 'CARGO_BUILD_TARGET', 'RUSTC_BOOTSTRAP'}
    INPUTS.need(not any(key in supplied for key in forbidden)
                and not any(key.startswith('CARGO_PROFILE_') or
                            (key.startswith('CARGO_TARGET_') and key != 'CARGO_TARGET_DIR')
                            for key in supplied),
                'compiler/profile overrides differ from the existing Linux Test job')
    result = {key: supplied[key] for key in ('PATH', 'CARGO_HOME', 'CARGO_TARGET_DIR',
              'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_JOBS', 'NEXTEST_TEST_THREADS')
              if key in supplied}
    for key in ('CARGO_HOME', 'CARGO_TARGET_DIR'):
        INPUTS.need(key in result and Path(result[key]).is_absolute() and '..' not in Path(result[key]).parts,
                    'explicit Cargo home and persistent target paths required')
    INPUTS.need(result.get('PATH') and identity.isdecimal() and int(identity) > 0,
                'reviewed tool PATH and positive local receipt identity required')
    result.update(HOME=str(output / 'home'), TMPDIR=str(output / 'tmp'),
        LANG='C.UTF-8', CARGO_NET_OFFLINE='true', CARGO_TERM_COLOR='always', RUST_BACKTRACE='1',
        PYTHONDONTWRITEBYTECODE='1', GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null',
        GIT_TERMINAL_PROMPT='0', GITHUB_SHA=head, GITHUB_RUN_ID=identity,
        GITHUB_RUN_ATTEMPT='1', RUNNER_OS='Linux', RUNNER_ARCH='X64')
    return result


def run(source, history, cache, output, head, base, manifest_sha256,
        onboarding_source, identity):
    INPUTS.need(os.geteuid() != 0, 'Linux Test adapter requires the unprivileged worker')
    INPUTS.need(sys.platform == 'linux', 'Linux Test requires Linux, never substitutes for a native cell')
    source, history, cache, output, onboarding_source = map(Path,
        (source, history, cache, output, onboarding_source))
    INPUTS.need(output.is_absolute() and '..' not in output.parts, 'absolute fresh output required')
    for path in (source, history, cache, onboarding_source):
        INPUTS.need(path != output and path not in output.parents and output not in path.parents,
                    'Linux Test output overlaps protected inputs')
    for path in (output, *output.parents):
        INPUTS.need(not path.is_symlink(), 'linked Linux Test output')
    INPUTS.need(directory_digest(onboarding_source) == ONBOARDING_SHA256,
                'onboarding source differs from its reviewed pin')
    env = clean_environment(output, head, identity, os.environ)
    output.mkdir(mode=0o700, exist_ok=False)
    for name in ('home', 'tmp'):
        (output / name).mkdir(mode=0o700)
    project, html = output / 'project', output / 'html-export'
    env.update(HARNESS_GATE_CI_ACCEPTANCE=str(project / 'target/quality/workflow-acceptance'),
               HG_HTML_ACCEPTANCE_DIR=str(html), HG_HTML_ACCEPTANCE_COMMIT=head)
    report = {'head': head, 'base': base, 'onboarding_commit': ONBOARDING_COMMIT,
        'onboarding_source_sha256': ONBOARDING_SHA256, 'local_receipt_identity': identity,
        'scope': 'Linux Test commands only; local receipt identity is not an Actions run; Core owns quality',
        'state': 'incomplete', 'commands': []}
    def save():
        temporary = output / '.result.json.tmp'
        with temporary.open('w') as stream:
            json.dump(report, stream, sort_keys=True, indent=2)
            stream.write('\n')
        os.replace(temporary, output / 'result.json')
    save()
    deadline = time.monotonic() + 1200  # Existing Test job's total 20-minute budget.
    def command(name, argv, extra=None):
        remaining = deadline - time.monotonic()
        INPUTS.need(remaining > 0, 'Linux Test total time budget exhausted')
        row = {'name': name, 'argv': [str(arg) for arg in argv], 'state': 'running'}
        report['commands'].append(row)
        save()
        started = time.monotonic()
        with (output / (name + '.stdout')).open('xb') as stdout, (output / (name + '.stderr')).open('xb') as stderr:
            result = subprocess.run(argv, cwd=project, env={**env, **(extra or {})},
                stdout=stdout, stderr=stderr, timeout=remaining, check=False)
        row.update(state='completed', exit_code=result.returncode, wall_seconds=time.monotonic() - started)
        save()
        INPUTS.need(result.returncode == 0, 'Linux Test command failed: ' + name)
    try:
        report['preparation'] = INPUTS.prepare(source, history, project, cache, head, base, manifest_sha256)
        INPUTS.need(INPUTS.digest(INPUTS.regular(project / '.github/workflows/ci.yml')) == WORKFLOW_SHA256,
                    'CI workflow changed; review complete Linux Test command parity before running')
        command('nextest-version', ['bash', '.github/actions/install-ci-tool/tool.sh', 'verify', 'cargo-nextest'])
        for name, argv in (
            ('nextest', ['cargo', 'nextest', 'run', '--manifest-path', 'tools/harness-gate/Cargo.toml', '--locked']),
            ('findings-feedback', ['node', '--test', '.github/actions/findings-feedback/feedback.test.cjs']),
            ('report-artifact', ['node', '--test', '.github/actions/report-artifact/prepare.test.cjs'])):
            command(name, argv)
        provenance = INPUTS.regular(html / 'provenance.json', 65536)
        command('html-transport', ['node', '.github/actions/report-artifact/prepare.cjs'], {
            'REPORT_DIRECTORY': str(html), 'REPORT_PROVENANCE_SHA256': INPUTS.digest(provenance),
            'REPORT_COMMIT': head, 'REPORT_UPLOAD': 'true', 'REPORT_RETENTION_DAYS': '7',
            'RUNNER_TEMP': str(output / 'tmp'), 'GITHUB_OUTPUT': str(output / 'html-staging.txt')})
        # prepare.cjs stages only the validated two display files. No upload action/API is called.
        staging = dict(line.split('=', 1) for line in INPUTS.regular(output / 'html-staging.txt', 8192).decode().splitlines())
        staged = Path(staging['directory'])
        INPUTS.need(output / 'tmp' in staged.parents and set(p.name for p in staged.iterdir()) ==
                    {'provenance.json', 'test_result.html'}, 'HTML staging is incomplete or escaping')
        for name in ('provenance.json', 'test_result.html'):
            INPUTS.need(INPUTS.regular(staged / name, 2 * 1024 * 1024) ==
                        INPUTS.regular(html / name, 2 * 1024 * 1024), 'HTML staging bytes differ')
        report['html_staging'] = str(staged)
        command('workflow-acceptance', [sys.executable, 'tools/quality/ci_acceptance.py', '--directory',
                                      env['HARNESS_GATE_CI_ACCEPTANCE']])
        command('schema-flow', ['cargo', 'run', '--manifest-path', 'tools/harness-gate/Cargo.toml',
                                '--', 'schema', 'export'])
        command('schema-quality', ['cargo', 'run', '--manifest-path', 'tools/harness-gate/Cargo.toml',
                                   '--locked', '--', 'schema', 'export', '--quality'])
        command('schema-diff', ['git', 'diff', '--exit-code', '--', 'schema/flow.schema.json', 'schema/quality.schema.json'])
        built = Path(env['CARGO_TARGET_DIR']) / 'debug/harness-gate'
        core = output / 'tested-core'
        core_sha = built_core_digest(built, core)
        command('onboarding', [sys.executable, '-B', str(onboarding_source / 'tools/onboarding/acceptance.py'),
                               '--core', str(core), '--work-dir', str(output / 'onboarding')])
        onboarding = json.loads(INPUTS.regular(output / 'onboarding/report.json', 1024 * 1024))
        INPUTS.need(onboarding['schema'] == 'gateprobes-onboarding-acceptance/v1'
                    and onboarding['passed'] is True and onboarding['ecosystems'] == ['rust', 'python']
                    and onboarding['core_sha256'] == core_sha and len(onboarding['steps']) == 29,
                    'pinned onboarding receipt is missing, incomplete or bound to another Core')
        INPUTS.need(INPUTS.digest(INPUTS.regular(core, 256 * 1024 * 1024)) == core_sha
                    and built_core_digest(built) == core_sha,
                    'tested Core changed during onboarding')
        report['onboarding_steps'] = len(onboarding['steps'])
        report['tested_core_sha256'] = core_sha
        report['post_validation'] = INPUTS.verify_prepared(source, history, project, head, base, manifest_sha256)
        INPUTS.need(directory_digest(onboarding_source) == ONBOARDING_SHA256, 'onboarding source changed during execution')
        report['state'] = 'commands_completed'
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        report.update(state='blocked', reason=str(error))
        save()
        raise
    save()
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('source', 'history', 'cache', 'output', 'onboarding-source'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('head', 'base', 'manifest-sha256', 'identity'):
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    try:
        report = run(args.source, args.history, args.cache, args.output, args.head, args.base,
                     args.manifest_sha256, args.onboarding_source, args.identity)
        print(json.dumps({'head': report['head'], 'state': report['state'],
                          'scope': report['scope'], 'commands': len(report['commands'])}))
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(json.dumps({'state': 'blocked', 'reason': str(error)}))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
