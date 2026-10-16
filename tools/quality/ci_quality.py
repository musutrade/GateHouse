#!/usr/bin/env python3
"""Required CI policy and fresh, review-only quality candidates (schema 1)."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import threading
import time
import uuid

from critical_paths import require_committed_sources
from production_coverage import CoverageTokenizer, require
from quality_common import ROOT, git_sha, metadata, sha256, write_json
from source_measure import SOURCE_FILES, compiler_configuration

# Feature flag for parallel risk collection (default: disabled, experimental)
PARALLEL_RISK = os.environ.get('CI_PARALLEL_RISK', '0') == '1'

# Feature flag for CI performance monitoring (default: disabled, experimental)
CI_MONITORING = os.environ.get('CI_MONITORING', '0') == '1'
MONITOR_OUTPUT = Path(os.environ.get('CI_MONITOR_DIR', ROOT / 'target/ci-monitor'))

COMMON = ('classify', 'test', 'test-cross-platform', 'security-audit', 'fmt', 'clippy', 'build',
          'quality-coverage', 'quality-contracts', 'docs-consistency', 'release-contracts',
          'quality-scripts')
PUSH_ONLY = ('build-cross-platform', 'quality-contracts-cross-platform')
# Only these jobs may inherit a verified PR result on an equivalent main push.
REUSABLE = ('test', 'test-cross-platform', 'quality-coverage')
STAGES = ('legacy', 'production', 'risk', 'matrix')
# Rust integration tests and their Python oracles read these repository trees.
# Historical reports under docs/quality are not test inputs: archiving that
# directory duplicated hundreds of MB of compressed evidence for each side.
SNAPSHOT_PATHS = ('tools/harness-gate', 'tools/quality', 'schema', 'docs/dogfood')
REPLAY_RELOCATION = ('tools/harness-gate/quality-core/examples/differential_replay.rs',
                     'tools/harness-gate/quality-replay/examples/differential_replay.rs')
# Reviewed 2026-10-12: the Postgres recipe left service/postgres.rs for the
# measured recipes series (source_measure.SOURCE_FILES). The exemption covers
# only the confirmed deletion: it applies while the old path exists at base and
# is absent at head. Recreating or modifying the old path fails the measurement
# review. This records the reviewed removal only; it does not establish
# function-level continuity between old and new sources, which are measured as
# separate functions.
REVIEWED_REMOVED_SOURCES = ('tools/harness-gate/src/service/postgres.rs',)


def _monitor_mark(name: str, pid: int | None = None, context: dict | None = None):
    """Record a timing mark if monitoring is enabled."""
    if not CI_MONITORING:
        return
    try:
        MONITOR_OUTPUT.mkdir(parents=True, exist_ok=True)
        mark_file = MONITOR_OUTPUT / f"mark_{os.getpid()}.jsonl"
        
        record = {
            'name': name,
            'timestamp': time.time(),
            'timestamp_utc': time.strftime('%Y-%m-%dT%H:%M:%S', time.gmtime()),
            'monotonic': time.monotonic(),
            'pid': pid or os.getpid(),
        }
        if context:
            record['context'] = context
        
        with open(mark_file, 'a') as f:
            f.write(json.dumps(record) + '\n')
            f.flush()
    except Exception as e:
        print(f'Warning: monitoring mark failed: {e}', file=sys.stderr)


def reviewed_removals(base: str, head: str) -> set[str]:
    """Exempt a reviewed removal only while the old path is actually gone."""
    removals = set()
    for path in REVIEWED_REMOVED_SOURCES:
        present = [
            subprocess.run(['git', 'cat-file', '-e', f'{commit}:{path}'],
                           cwd=ROOT, capture_output=True).returncode == 0
            for commit in (base, head)
        ]
        if present == [True, False]:
            removals.add(path)
    return removals
REQUIRED_ARTIFACTS = {'coverage.json', 'coverage.raw.json', 'coverage.lcov',
                      'coverage.cobertura.xml', 'production.json', 'risk.json',
                      'critical-paths.json', 'critical-path-runs/bundle.json'} | {
    f'{label}-{suffix}' for label in ('base', 'head') for suffix in (
        'source.tar', 'manifest.json', 'coverage.json', 'coverage.lcov',
        'coverage.cobertura.xml', 'risk.json')}
# Keep the public helpers for existing Core consumers; one implementation owns
# pin validation, download, trust verification and safe extraction.
from measurer_release import (load_measurer_pin, verify_measurer_asset, ensure_measurer)


def relocated_migration_sources(base: str, head: str) -> set[str]:
    """Recognize only the unchanged move of the unpublished replay executable."""
    old, new = REPLAY_RELOCATION
    change = subprocess.check_output(
        ['git', 'diff', '--name-status', '--find-renames=100%', base, head, '--', old, new],
        cwd=ROOT, text=True)
    return {old} if change == f'R100\t{old}\t{new}\n' else set()


def terminal_test_module_boundary(source: str) -> tuple[str, str] | None:
    """Retain every byte except one exact top-level terminal cfg(test) body."""
    # The fixture lexer does not implement Rust's special shebang/BOM handling.
    # Reject that boundary rather than letting ignored text spoof a test module.
    if source.startswith(('#!', '\ufeff')):
        return None
    tokens = CoverageTokenizer(source).tokenize()
    header = ['#', '[', 'cfg', '(', 'test', ')', ']', 'mod', 'tests', '{']
    stack, pairs, modules = [], {}, []
    for index, token in enumerate(tokens):
        if not stack and [t.text for t in tokens[index:index + len(header)]] == header:
            modules.append(index + len(header) - 1)
        if token.text in ('(', '[', '{'):
            stack.append(index)
        elif token.text in (')', ']', '}'):
            require(bool(stack), 'unbalanced Rust source')
            opening = stack.pop()
            require(tokens[opening].text == {')': '(', ']': '[', '}': '{'}[token.text],
                    'unbalanced Rust source')
            pairs[opening] = index
    require(not stack, 'unbalanced Rust source')
    if len(modules) != 1 or pairs[modules[0]] != len(tokens) - 1:
        return None
    opening, closing = tokens[modules[0]], tokens[pairs[modules[0]]]
    return source[:opening.offset + 1], source[closing.offset:]


def only_terminal_test_module_changed(base: str, head: str, path: str) -> bool:
    """Prove unchanged production source bytes, not macro/runtime equivalence."""
    try:
        snapshots = []
        for commit in (base, head):
            mode = subprocess.check_output(
                ['git', '--literal-pathspecs', 'ls-tree', '-z',
                 '--format=%(objectmode) %(objecttype)', commit, '--', path],
                cwd=ROOT, stderr=subprocess.PIPE)
            require(mode in (b'100644 blob\0', b'100755 blob\0'), 'not one regular source file')
            source = subprocess.check_output(['git', 'show', f'{commit}:{path}'],
                                             cwd=ROOT, stderr=subprocess.PIPE).decode('utf-8')
            snapshots.append((mode, terminal_test_module_boundary(source)))
        return snapshots[0][1] is not None and snapshots[0] == snapshots[1]
    except (OSError, UnicodeError, ValueError, subprocess.CalledProcessError):
        # Missing files, unsupported lexical forms and malformed input retain
        # the existing measurement-review failure, never a guessed exemption.
        return False


PURE_DOC_EXCLUDED_PREFIXES = ('docs/dogfood/', 'docs/benchmarks/', 'docs/quality/')


def is_pure_document(path: str) -> bool:
    """Only prose markdown. Fixtures, records, specs and ownership are inputs."""
    if not path.endswith('.md'):
        return False
    if '/' not in path:
        return True
    return path.startswith('docs/') and not path.startswith(PURE_DOC_EXCLUDED_PREFIXES)


def changed_paths(base: str, head: str) -> list[str]:
    output = subprocess.check_output(
        ['git', 'diff', '--name-status', '--find-renames', base, head], cwd=ROOT, text=True,
        stderr=subprocess.PIPE)
    paths = []
    for line in output.splitlines():
        parts = line.split('\t')
        if len(parts) >= 2 and parts[1]:
            paths.append(parts[1])
        if len(parts) >= 3 and parts[2]:
            paths.append(parts[2])
    return paths


def classify(base: str, head: str) -> dict:
    """Pure docs select light checks only; unknown or empty scope stays full."""
    if not base or not head or set(base) == {'0'}:
        return {'docs_only': False, 'run_full': True, 'files': 0}
    paths = changed_paths(base, head)
    docs_only = bool(paths) and all(is_pure_document(path) for path in paths)
    return {'docs_only': docs_only, 'run_full': not docs_only, 'files': len(paths)}


# Known product/test changes defer full metrics to the daily run. Everything
# outside this allowlist, including measurement bindings/configuration, stays full.
ORDINARY_PREFIXES = ('tools/harness-gate/src/', 'tools/harness-gate/tests/')
NATIVE_PREFIXES = tuple('tools/harness-gate/src/' + name for name in
                        ('process/', 'service/', 'utils/', 'scope/', 'project/',
                         'secrets/', 'app/', 'verify/', 'ui.rs', 'main.rs', 'lib.rs'))


# Last full main validation before impact routing; later routed pushes must not
# advance the periodic lineage merely because their own short delta was checked.
PERIODIC_BOOTSTRAP = '444b378e15497d852689d12721b40dab0e7fe0cd'


def certified_base(head: str) -> str:
    """Use an actually successful full main collection, never yesterday's HEAD."""
    repo = os.environ.get('GITHUB_REPOSITORY', '')
    require(bool(re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo)), 'missing repository')
    def api(path):
        return json.loads(subprocess.check_output(['gh', 'api', f'repos/{repo}{path}'],
                                                 text=True, timeout=30))
    runs = api('/actions/workflows/ci.yml/runs?branch=main&status=success&per_page=100')['workflow_runs']
    for run in runs:
        sha = run['head_sha']
        if run['event'] not in ('schedule', 'workflow_dispatch') and not (
                run['event'] == 'push' and sha == PERIODIC_BOOTSTRAP):
            continue
        if not re.fullmatch('[0-9a-f]{40}', sha):
            continue
        if subprocess.run(['git', 'merge-base', '--is-ancestor', sha, head], cwd=ROOT,
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            continue
        jobs = api(f"/actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100")['jobs']
        measured = [job for job in jobs if job['name'] == 'Quality Coverage and Critical Paths']
        if len(measured) == 1 and measured[0]['conclusion'] == 'success':
            return sha
    raise ValueError('no certified main collection in recent runs; restore lineage before collecting')


def impact(base: str, head: str, event: str) -> dict:
    if event not in ('push', 'pull_request'):
        return {'run_evidence': True, 'run_native': True}
    try:
        paths = changed_paths(base, head) if base and head and set(base) != {'0'} else []
    except (OSError, subprocess.SubprocessError):
        paths = []
    if not paths:
        return {'run_evidence': True, 'run_native': True}
    ordinary = all(is_pure_document(p) or
                   (p.startswith(ORDINARY_PREFIXES) and p.endswith('.rs')) for p in paths)
    native = not ordinary or any(p.startswith(NATIVE_PREFIXES) or
                                p.startswith('tools/harness-gate/tests/') for p in paths)
    if ordinary and not native:
        try:
            diff = subprocess.check_output(['git', 'diff', '--no-ext-diff', '--unified=0',
                                            base, head, '--', *ORDINARY_PREFIXES],
                                           cwd=ROOT, text=True, stderr=subprocess.PIPE)
            native = bool(re.search(r'windows|unix|target_os|target_family|std::os|std::process|\bunsafe\b|\bextern\b', diff))
        except (OSError, subprocess.SubprocessError):
            native = True
    return {'run_evidence': not ordinary, 'run_native': native}


def aggregate(event: str, needs: dict, full: bool = True, reuse: bool = False,
              evidence: bool = True, native: bool = True) -> list[str]:
    """Evaluate child outcomes only; never collect evidence or launch tools here."""
    require(event in ('push', 'pull_request', 'schedule', 'workflow_dispatch'), 'unsupported CI event')
    require(event in ('push', 'pull_request') or (full and evidence and native and not reuse),
            'periodic/manual runs require all gates')
    require(isinstance(needs, dict), 'CI needs must be an object')
    require(not reuse or (event == 'push' and full), 'reuse requires a full push')
    required = COMMON + (PUSH_ONLY if event != 'pull_request' else ())
    deferred = set()
    if not evidence:
        deferred.add('quality-coverage')
    if not native:
        deferred.update(('test-cross-platform', *PUSH_ONLY))
    if full:
        return [name for name in required
                if not isinstance(needs.get(name), dict) or needs[name].get('result') not in
                (('success', 'skipped') if (reuse and name in REUSABLE) or name in deferred else ('success',))]
    failures = []
    for name in required:
        entry = needs.get(name)
        result = entry.get('result') if isinstance(entry, dict) else None
        if name in ('classify', 'docs-consistency'):
            if result != 'success':
                failures.append(name)
        elif result not in ('skipped', 'success'):
            failures.append(name)
    return failures


def verify(path: Path, head: str, base: str, run_id: str) -> dict:
    report = json.loads(path.read_text())
    require(report['schema_version'] == 1 and report['candidate'] is True, 'invalid candidate schema')
    require((report['commit'], report['base_sha'], report['run_id']) == (head, base, run_id),
            'stale or mixed candidate identity')
    require(set(report['stages']) == set(STAGES), 'missing required evidence stage')
    require(all(s['status'] == 'success' for s in report['stages'].values()),
            'failed, cancelled, skipped or incomplete collection')
    require(REQUIRED_ARTIFACTS <= report['artifacts'].keys(), 'missing required raw evidence')
    for relative, digest in report['artifacts'].items():
        artifact = (path.parent / relative).resolve()
        require(artifact.is_relative_to(path.parent.resolve()), 'artifact escapes candidate')
        require(sha256(artifact) == digest, f'stale or modified artifact: {relative}')
    return report


class Collector:
    def __init__(self, directory: Path, base: str, head: str, run_id: str):
        self.directory = directory.resolve()
        require(not self.directory.exists(), 'candidate directory already exists; choose a fresh path')
        self.directory.mkdir(parents=True)
        self.environment = dict(os.environ)
        for key in ('LLVM_PROFILE_FILE', 'CARGO_BUILD_TARGET', 'CARGO_LLVM_COV_TARGET_DIR',
                    'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
            self.environment.pop(key, None)
        self.environment['CARGO_TARGET_DIR'] = str(self.directory / 'build')
        self.report = {**metadata(), 'schema_version': 1, 'candidate': True,
                       'base_sha': base, 'commit': head, 'run_id': run_id,
                       'stages': {}, 'commands': [], 'artifacts': {}}
        self._save_lock = threading.Lock()
        self.save()

    def save(self):
        with self._save_lock:
            write_json(self.directory / 'candidate.json', self.report)

    def command(self, name: str, command: list[str], allowed=(0,), env=None):
        started = time.monotonic()
        print(f'{name}: started (log: {name}.log)', flush=True)
        # Use provided env or fall back to self.environment
        effective_env = env if env is not None else self.environment
        with (self.directory / f'{name}.log').open('w') as log:
            code = subprocess.run(command, cwd=ROOT, env=effective_env,
                                  stdout=log, stderr=subprocess.STDOUT).returncode
        elapsed = time.monotonic() - started
        # Protect report mutation and save as one atomic operation
        with self._save_lock:
            self.report['commands'].append({'name': name, 'argv': command, 'exit_code': code,
                                            'seconds': elapsed})
            write_json(self.directory / 'candidate.json', self.report)
        print(f'{name}: exited {code} in {elapsed:.2f}s', flush=True)
        require(code in allowed, f'{name} exited {code}; see {name}.log')

    def python(self, name: str, script: str, *args: str, allowed=(0,), env=None):
        self.command(name, [sys.executable, str(ROOT / 'tools/quality' / script), *map(str, args)], allowed, env=env)

    def measurer(self) -> Path:
        """Consume the same verified release as the Core measurement tests."""
        pin = load_measurer_pin()
        def command(name, argv):
            self.command(name, argv)
        binary = ensure_measurer(self.directory / 'measurer', command=command, pin=pin)
        self.report['measurer'] = pin
        self.save()
        
        # For CI monitoring: write measurer info to a known location
        import hashlib
        try:
            measurer_sha256 = hashlib.file_digest(open(binary, 'rb'), 'sha256').hexdigest()
            measurer_info_path = Path('target/ci-monitor/measurer_info.txt')
            measurer_info_path.parent.mkdir(parents=True, exist_ok=True)
            measurer_info_path.write_text(f"{binary}\n{measurer_sha256}\n")
        except Exception:
            pass  # Non-critical
        
        return binary

    def legacy(self):
        self.python('legacy', 'coverage.py', '--test-runner', 'nextest',
                    '--output', self.directory / 'coverage.json')

    def production(self):
        self.python('production', 'coverage.py', '--production', '--raw', self.directory / 'coverage.raw.json',
                    '--lcov', self.directory / 'coverage.lcov', '--output', self.directory / 'production.json')

    def snapshot(self, label: str, commit: str) -> Path:
        snapshot = self.directory / 'snapshots' / label
        snapshot.mkdir(parents=True)
        archive = self.directory / f'{label}-source.tar'
        # Integration tests embed documentation fixtures. Archive them from the
        # measured commit too, preserving their repository-relative paths.
        paths = list(SNAPSHOT_PATHS)
        external = subprocess.run(['git', 'cat-file', '-e', f'{commit}:test-data/manifest.json'],
                                  cwd=ROOT, capture_output=True).returncode == 0
        if external:
            paths.extend(('tools/maintenance', 'test-data'))
        self.command(f'{label}-archive', ['git', 'archive', '--format=tar', f'--output={archive}',
                                          commit, *paths])
        with tarfile.open(archive) as source:
            source.extractall(snapshot, filter='data')
        if external:
            # Each measured commit supplies its own pinned bytes; never reuse
            # head data for the base or substitute a missing/corrupt payload.
            self.command(f'{label}-test-data', [sys.executable,
                         str(snapshot / 'tools/maintenance/test_data.py'),
                         '--root', str(snapshot), '--group', 'regression-corpus',
                         '--group', 'arc-admin-replay'])
        return snapshot

    def _validate_risk_scope(self, base: str, head: str):
        """Shared scope validation for both sequential and parallel risk collection."""
        changed = subprocess.check_output(
            ['git', 'diff', '--name-only', base, head, '--',
             'tools/harness-gate/src', 'tools/harness-gate/quality-core'],
            cwd=ROOT, text=True
        ).splitlines()

        relocated = relocated_migration_sources(base, head)
        removed = reviewed_removals(base, head)

        unsupported = [
            p for p in changed
            if p.endswith('.rs')
            and p not in relocated
            and p not in removed
            and p not in {
                str((ROOT / 'tools/harness-gate/src' / source).resolve().relative_to(ROOT))
                for source in SOURCE_FILES
            }
            and not (
                p.startswith('tools/harness-gate/quality-core/tests/')
                or p in (
                    'tools/harness-gate/quality-core/tests.rs',
                    'tools/harness-gate/quality-core/policy_tests.rs',
                    'tools/harness-gate/src/config/quality/tests.rs',
                    'tools/harness-gate/src/config/tests.rs',
                    'tools/harness-gate/src/verify/tests.rs',
                    'tools/harness-gate/src/verify/cache/tests.rs',
                    'tools/harness-gate/src/verify/parser_fuzz.rs',
                    'tools/harness-gate/src/findings/tests.rs',
                    'tools/harness-gate/src/preset/tests.rs',
                    'tools/harness-gate/src/scope/tests.rs',
                    'tools/harness-gate/src/process/tests.rs',
                    'tools/harness-gate/src/doctor/tests.rs',
                    'tools/harness-gate/src/service/tests.rs',
                    'tools/harness-gate/src/recipes/tests.rs',
                    'tools/harness-gate/src/config/quality/collectors/tests.rs',
                    'tools/harness-gate/src/config/quality/collectors/tests/dogfood_acceptance.rs',
                    'tools/harness-gate/src/config/quality/collectors/ci_acceptance.rs'
                )
            )
        ]

        unsupported = [p for p in unsupported if not only_terminal_test_module_changed(base, head, p)]

        require(
            not unsupported,
            'production changes outside supported risk series; measurement review required: '
            + ', '.join(unsupported)
        )

    def _collect_commit_coverage(self, label: str, commit: str, snapshot: Path, binary: Path, target: str, *, env=None):
        """Shared per-commit coverage collection for both modes."""
        crate = snapshot / 'tools/harness-gate'
        manifest = self.directory / f'{label}-manifest.json'

        self.python(
            f'{label}-prepare',
            'source_measure.py', 'prepare',
            '--crate', crate,
            '--binary', binary,
            '--manifest', manifest,
            '--target', target, env=env
        )

        cargo_manifest = str(crate / 'Cargo.toml')
        self.command(
            f'{label}-coverage',
            ['cargo', 'llvm-cov', 'nextest', '--workspace', '--locked',
             '--manifest-path', cargo_manifest, '--target', target,
             '--json', '--output-path', str(self.directory / f'{label}-coverage.json')], env=env
        )

        for flag, suffix in (('--lcov', 'lcov'), ('--cobertura', 'cobertura.xml')):
            self.command(
                f'{label}-{suffix}',
                ['cargo', 'llvm-cov', 'report', '--workspace',
                 '--manifest-path', cargo_manifest, '--target', target,
                 flag, '--output-path', str(self.directory / f'{label}-coverage.{suffix}')], env=env
            )

        self.python(
            f'{label}-measure',
            'source_measure.py', 'measure',
            '--crate', crate,
            '--binary', binary,
            '--manifest', manifest,
            '--llvm', self.directory / f'{label}-coverage.json',
            '--output', self.directory / f'{label}-risk.json',
            allowed=(0, 1), env=env
        )

        report = json.loads((self.directory / f'{label}-risk.json').read_text())
        require(report['manifest_sha256'] == sha256(manifest), f'{label}: stale risk manifest')
        require(
            report['llvm_sha256'] == sha256(self.directory / f'{label}-coverage.json'),
            f'{label}: stale risk profiles'
        )

        original = json.loads(manifest.read_text())
        for path, item in original['files'].items():
            relative = (ROOT / 'tools/harness-gate/src' / path).resolve().relative_to(ROOT)
            committed = subprocess.check_output(['git', 'show', f'{commit}:{relative}'], cwd=ROOT)
            require(item['original'].encode() == committed, f'{label}: risk source/commit mismatch for {path}')

        for path in original['absent_sources']:
            relative = (ROOT / 'tools/harness-gate/src' / path).resolve().relative_to(ROOT)
            exists = subprocess.run(
                ['git', 'cat-file', '-e', f'{commit}:{relative}'],
                cwd=ROOT,
                capture_output=True
            ).returncode == 0
            require(not exists, f'{label}: absent risk source exists in commit: {relative}')

        return report

    def _compare_risk(self, base: str, head: str):
        """Shared risk comparison for both modes."""
        self.python(
            'risk-compare',
            'source_measure.py', 'compare',
            '--base', self.directory / 'base-risk.json',
            '--head', self.directory / 'head-risk.json',
            '--output', self.directory / 'risk.json'
        )

        comparison = json.loads((self.directory / 'risk.json').read_text())
        (self.directory / 'risk.md').write_text(
            '# Candidate function risk\n\n'
            f'Base: `{base}`\n\nHead: `{head}`\n\n'
            f"Identities: {len(comparison['identities'])}; "
            f"failures: {len(comparison['failures'])}.\n\n"
            'Scope: GH-94 files, staged snapshot input, adapter replay, '
            'project discovery, CLI dispatch, quality configuration, '
            'preset composition and all linked generic-core production sources. '
            'Raw counters, exact rational CRAP and historical debt are retained '
            'in head-risk.json. Branch coverage is unsupported. '
            'This candidate is not an accepted baseline.\n'
        )

    def risk(self):
        if PARALLEL_RISK:
            from ci_quality_parallel import risk_parallel
            risk_parallel(self)
        else:
            self._risk_sequential()

    def _risk_sequential(self):
        """Sequential risk collection (preserves original cache behavior)."""
        base, head = self.report['base_sha'], self.report['commit']
        self._validate_risk_scope(base, head)

        binary = self.measurer()
        target = compiler_configuration()['target']

        for label, commit in (('base', base), ('head', head)):
            snapshot = self.snapshot(label, commit)
            self._collect_commit_coverage(label, commit, snapshot, binary, target)

        self._compare_risk(base, head)

    def matrix(self):
        self.python('matrix', 'critical_paths.py', '--collect', '--evidence',
                    self.directory / 'critical-path-runs/bundle.json', '--output',
                    self.directory / 'critical-paths.json')

    def collect(self):
        _monitor_mark('collect_start', context={'commit': self.report['commit'], 'base_sha': self.report['base_sha']})
        started = time.monotonic()
        try:
            for stage in STAGES:
                tick = time.monotonic()
                _monitor_mark(f'stage_{stage}_start')
                self.report['stages'][stage] = {'status': 'running'}
                self.save()
                try:
                    getattr(self, stage)()
                    result = {'status': 'success'}
                except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
                    result = {'status': 'failure', 'error': str(error)}
                    _monitor_mark(f'stage_{stage}_error', context={'error': str(error)})
                result['seconds'] = time.monotonic() - tick
                self.report['stages'][stage] = result
                self.save()
                _monitor_mark(f'stage_{stage}_end', context={'status': result['status'], 'seconds': result['seconds']})
                print(f'{stage}: {result}', flush=True)
            require(git_sha() == self.report['commit'], 'commit changed during collection')
            require_committed_sources(self.report['commit'])
            self.report['collection_seconds'] = time.monotonic() - started
            for artifact in self.directory.rglob('*'):
                relative = artifact.relative_to(self.directory)
                if relative.parts[0] not in ('build', 'snapshots', 'candidate.json') and artifact.is_file():
                    self.report['artifacts'][str(relative)] = sha256(artifact)
            self.save()
            verify(self.directory / 'candidate.json', self.report['commit'], self.report['base_sha'], self.report['run_id'])
            _monitor_mark('collect_success', context={'collection_seconds': self.report['collection_seconds']})
        except Exception as e:
            _monitor_mark('collect_error', context={'error': str(e)})
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('aggregate', 'classify', 'collect', 'verify', 'certified-base'))
    parser.add_argument('--event', default=os.environ.get('EVENT_NAME'))
    parser.add_argument('--needs', default=os.environ.get('NEEDS_JSON'))
    parser.add_argument('--base-sha')
    parser.add_argument('--head-sha')
    parser.add_argument('--run-id', default=str(uuid.uuid4()))
    parser.add_argument('--output', type=Path, default=ROOT / 'target/quality/candidate')
    parser.add_argument('--github-output', type=Path)
    args = parser.parse_args()
    try:
        if args.operation == 'certified-base':
            require(bool(re.fullmatch('[0-9a-f]{40}', args.head_sha or '')), 'full head SHA required')
            print(certified_base(args.head_sha))
        elif args.operation == 'aggregate':
            full = os.environ.get('RUN_FULL', 'true').strip().lower() != 'false'
            reuse_value = os.environ.get('REUSE_VERIFIED', 'false')
            require(reuse_value in ('true', 'false'), 'invalid reuse decision')
            decisions = {key: os.environ.get(key, 'true') for key in ('RUN_EVIDENCE', 'RUN_NATIVE')}
            require(all(value in ('true', 'false') for value in decisions.values()), 'invalid impact decision')
            failures = aggregate(args.event, json.loads(args.needs or '{}'), full=full,
                                 reuse=reuse_value == 'true', evidence=decisions['RUN_EVIDENCE'] == 'true',
                                 native=decisions['RUN_NATIVE'] == 'true')
            require(not failures, f'required jobs did not succeed: {failures}')
        elif args.operation == 'classify':
            result = classify(args.base_sha or '', args.head_sha or '')
            if args.event in ('schedule', 'workflow_dispatch'):
                result.update(docs_only=False, run_full=True)
            result.update(impact(args.base_sha or '', args.head_sha or '', args.event))
            print('classify: ' + json.dumps(result, sort_keys=True))
            if args.github_output:
                args.github_output.write_text(
                    f"base_sha={args.base_sha}\n"
                    f"run_full={'true' if result['run_full'] else 'false'}\n"
                    f"docs_only={'true' if result['docs_only'] else 'false'}\n"
                    f"files={result['files']}\n"
                    f"run_evidence={str(result['run_evidence']).lower()}\n"
                    f"run_native={str(result['run_native']).lower()}\n")
        else:
            for commit in (args.base_sha, args.head_sha):
                require(bool(re.fullmatch('[0-9a-f]{40}', commit or '')), 'full base/head SHAs required')
                subprocess.run(['git', 'cat-file', '-e', f'{commit}^{{commit}}'], cwd=ROOT, check=True)
            if args.operation == 'collect':
                require(git_sha() == args.head_sha, 'head must equal checked-out commit')
                require_committed_sources(args.head_sha)
                Collector(args.output, args.base_sha, args.head_sha, args.run_id).collect()
            else:
                verify(args.output / 'candidate.json', args.head_sha, args.base_sha, args.run_id)
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as error:
        print(f'quality CI failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
