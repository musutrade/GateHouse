#!/usr/bin/env python3
"""Required CI policy and fresh, review-only quality candidates (schema 1)."""
from __future__ import annotations

import argparse
from collections import defaultdict
import copy
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import uuid
import xml.etree.ElementTree as ET

from critical_paths import require_committed_sources
from function_risk import crap_line, own_lines
from production_coverage import CoverageTokenizer, counts, require
from quality_common import ROOT, git_sha, metadata, sha256, write_json
from source_measure import SOURCE_FILES, compiler_configuration

# Retained for single-runner local experiments. Required CI now starts the two
# risk sides as independent jobs and assembles them with assemble_risk().
PARALLEL_RISK = os.environ.get('CI_PARALLEL_RISK', '0') == '1'

# The required check name verified by baseline discovery and PR-reuse probes.
# The merge job keeps this display name even when collection is split into
# shards, so certified-base lineage and reuse receipts keep matching.
COVERAGE_JOB = 'Quality Coverage and Critical Paths'
# Split-collection shards: job name -> stages that shard owns.
SHARD_STAGES = {'fast': ('legacy', 'production'), 'risk': ('risk',), 'matrix': ('matrix',)}
SHARD_TOOL_IDENTITY = 'tool-identity.json'
# Risk collection may run as two independent workers, one per measured commit.
# Each worker records only its side; the merge job assembles both into the
# ordinary risk shard consumed by the existing three-shard merge.
RISK_SIDES = ('base', 'head')
RISK_SIDE_SUFFIXES = ('source.tar', 'manifest.json', 'coverage.json', 'coverage.lcov',
                      'coverage.cobertura.xml', 'risk.json')
RISK_SIDE_ARTIFACTS = {
    side: frozenset(f'{side}-{suffix}' for suffix in RISK_SIDE_SUFFIXES)
    for side in RISK_SIDES
}
RISK_REQUIRED_ARTIFACTS = frozenset().union(*RISK_SIDE_ARTIFACTS.values())

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
REVIEWED_REMOVED_SOURCES = (
    'tools/harness-gate/src/service/postgres.rs',
    # Retired transport cleanup: webhook delivery settings no longer gate
    # verification, so the unused net_policy module was deleted. Reviewed
    # removal; the risk series is otherwise unchanged.
    'tools/harness-gate/src/net_policy.rs',
    # Platform navigation retirement: Core no longer validates GitHub
    # repositories or reads Git objects to build source links, so the
    # GitHub-only source_links module was deleted. Reviewed removal; the
    # remaining risk inventory and thresholds are unchanged.
    'tools/harness-gate/src/source_links.rs',
    # Operator lifecycle extraction: hook installation/status/uninstall now
    # belong to the verified external operator, so the three lifecycle modules
    # were deleted after the release acceptance. Reviewed removal; remaining
    # measured sources and thresholds are unchanged.
    'tools/harness-gate/src/hooks/filesystem.rs',
    'tools/harness-gate/src/hooks/mod.rs',
    'tools/harness-gate/src/hooks/paths.rs',
)


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
        measured = [job for job in jobs if job['name'] == COVERAGE_JOB]
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


def tool_identity() -> dict:
    """Record collection-side tool versions and build configuration.

    Shards record this identity next to their candidate; merge requires all
    shards to agree. The sealed manifest separately records the merge runner's
    tools as the sealing identity, which is a different claim.
    """
    from ci_cargo import configuration_hash
    from quality_common import command_output
    tools = {tool: command_output(command) for tool, command in {
        'rustc': ['rustc', '-vV'], 'cargo': ['cargo', '--version'],
        'nextest': ['cargo', 'nextest', '--version'],
        'llvm-cov': ['cargo', 'llvm-cov', '--version'],
    }.items()}
    require(all(tools.values()), 'missing tool identity')
    return {'schema': 1, 'tools': tools, 'configuration_sha256': configuration_hash()}


_RESERVED_ARTIFACTS = ('candidate.json', SHARD_TOOL_IDENTITY)


def _canonical_relative(relative: str) -> str:
    """Require a canonical shard-relative artifact path.

    Rejects absolute paths, '.', '..', backslashes, empty duplicates and the
    names merge itself owns, so aliases cannot dodge the duplicate-path check
    or redirect a copy outside the output directory.
    """
    require(isinstance(relative, str) and relative and '\\' not in relative,
            f'invalid artifact path: {relative!r}')
    path = PurePosixPath(relative)
    require(not path.is_absolute(), f'absolute artifact path: {relative}')
    require(all(part not in ('', '.', '..') for part in path.parts),
            f'non-canonical artifact path: {relative}')
    require(str(path) == relative, f'non-canonical artifact path: {relative}')
    require(path.name not in _RESERVED_ARTIFACTS, f'reserved artifact path: {relative}')
    return str(path)


def _validated_tool_identity(name: str, path: Path) -> str:
    """Parse and validate a shard's collected tool identity."""
    try:
        identity = json.loads(path.read_text())
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ValueError(f'{name}: invalid {SHARD_TOOL_IDENTITY}: {error}') from error
    require(isinstance(identity, dict) and identity.get('schema') == 1,
            f'{name}: invalid {SHARD_TOOL_IDENTITY} schema')
    tools = identity.get('tools')
    require(isinstance(tools, dict) and set(tools) == {'rustc', 'cargo', 'nextest', 'llvm-cov'},
            f'{name}: {SHARD_TOOL_IDENTITY} must record exactly the four tools')
    require(all(isinstance(value, str) and value.strip() for value in tools.values()),
            f'{name}: empty tool identity value')
    require(re.fullmatch('[0-9a-f]{64}', str(identity.get('configuration_sha256') or '')),
            f'{name}: invalid configuration digest')
    return json.dumps(identity, sort_keys=True)


def risk_markdown(base: str, head: str, comparison: dict) -> str:
    return (
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


def _risk_side_candidate(name: str, directory: Path) -> tuple[dict, dict[str, str], str, dict]:
    """Validate one worker's side candidate and its own evidence bytes."""
    require(name in RISK_SIDES, f'unknown risk side: {name}')
    directory = directory.resolve()
    require(directory.is_dir(), f'{name}: risk side directory missing')
    candidate_path = directory / 'candidate.json'
    require(candidate_path.is_file(), f'{name}: missing candidate.json')
    report = json.loads(candidate_path.read_text())
    require(report.get('schema_version') == 1 and report.get('candidate') is True,
            f'{name}: invalid candidate schema')
    stages = report.get('stages')
    require(isinstance(stages, dict) and set(stages) == {'risk'},
            f'{name}: risk worker must record exactly the risk stage')
    stage = stages['risk']
    require(isinstance(stage, dict) and stage.get('status') == 'success',
            f'{name}: risk worker did not succeed')
    require(stage.get('side') == name, f'{name}: risk side identity mismatch')
    identity = (report.get('base_sha'), report.get('commit'), report.get('run_id'))
    require(all(re.fullmatch('[0-9a-f]{40}', str(value or ''))
                for value in identity[:2]) and isinstance(identity[2], str) and identity[2],
            f'{name}: invalid base/head/run identity')

    artifacts = report.get('artifacts')
    require(isinstance(artifacts, dict), f'{name}: invalid artifact inventory')
    local: dict[str, str] = {}
    for relative, digest in artifacts.items():
        canonical = _canonical_relative(relative)
        require(isinstance(digest, str) and re.fullmatch('[0-9a-f]{64}', digest),
                f'{name}: invalid artifact digest: {relative}')
        artifact = (directory / canonical).resolve()
        require(artifact.is_relative_to(directory), f'{name}: artifact escapes shard: {relative}')
        require(artifact.is_file() and sha256(artifact) == digest,
                f'{name}: stale or modified artifact: {relative}')
        local[canonical] = digest
    require(RISK_SIDE_ARTIFACTS[name] <= local.keys(),
            f'{name}: missing risk side evidence')
    require(not ({'risk.json', 'risk.md', 'risk-compare.log'} & local.keys()),
            f'{name}: side evidence pre-claims assembly output')
    other = 'head' if name == 'base' else 'base'
    require(not (RISK_SIDE_ARTIFACTS[other] & local.keys()),
            f'{name}: contains {other} side required evidence')

    tool_file = directory / SHARD_TOOL_IDENTITY
    require(tool_file.is_file(), f'{name}: missing {SHARD_TOOL_IDENTITY}')
    tool_document = _validated_tool_identity(name, tool_file)
    measurer = report.get('measurer')
    require(isinstance(measurer, dict) and measurer, f'{name}: missing measurer pin')
    commands = report.get('commands')
    require(isinstance(commands, list), f'{name}: invalid command record')
    return report, local, tool_document, measurer



def _llvm_tool(name: str) -> Path:
    """Resolve the LLVM tool shipped with the active Rust toolchain."""
    libdir = subprocess.check_output(['rustc', '--print', 'target-libdir'],
                                     cwd=ROOT, text=True).strip()
    tool = Path(libdir).parent / 'bin' / name
    require(tool.is_file(), f'LLVM tool missing from active toolchain: {name}')
    return tool


def _fast_archive_objects(target: Path, debug_root: Path | None = None) -> list[Path]:
    """Map archived nextest metadata to extracted test objects."""
    metadata_path = target / 'nextest' / 'binaries-metadata.json'
    require(metadata_path.is_file(), 'fast archive is missing nextest metadata')
    data = json.loads(metadata_path.read_text())
    binaries = data.get('rust-binaries')
    require(isinstance(binaries, dict) and binaries, 'fast archive has no rust binaries')
    debug_root = debug_root or (target / 'debug')
    objects = []
    seen = set()
    for row in binaries.values():
        require(isinstance(row, dict) and isinstance(row.get('binary-path'), str),
                'fast archive has invalid binary metadata')
        candidate = debug_root / 'deps' / Path(row['binary-path']).name
        if candidate.is_file() and candidate not in seen:
            seen.add(candidate)
            objects.append(candidate)
    main = debug_root / 'harness-gate'
    if main.is_file() and main not in seen:
        objects.append(main)
    require(objects, 'fast archive has no runnable objects')
    return objects


def _fast_ignore_regex(target: Path) -> str:
    """Match cargo-llvm-cov's source filtering for archive-backed reports."""
    workspace = (ROOT / 'tools' / 'harness-gate').resolve()
    target = target.resolve()
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')).resolve()
    rustup_home = Path(os.environ.get('RUSTUP_HOME', Path.home() / '.rustup')).resolve()
    return '|'.join((
        r'/rustc/([0-9a-f]+|[0-9]+\.[0-9]+\.[0-9]+)/',
        f'^{re.escape(str(workspace))}(/.*)?/(tests|examples|benches)/',
        f'^{re.escape(str(workspace))}(/.*)?/(tests\\.rs|[0-9a-zA-Z_-]+[_-]tests\\.rs)$',
        f'^{re.escape(str(target))}($|/)',
        f'^{re.escape(str(cargo_home))}/(registry|git)/',
        f'^{re.escape(str(rustup_home))}($|/)',
        f'^{re.escape(str(workspace))}/quality-replay($|/)',
    ))


def _lcov_to_cobertura(lcov: Path, workspace: Path) -> str:
    """Encode the merged LCOV line map as a Cobertura XML artifact."""
    records = {}
    current = None
    for line in lcov.read_text().splitlines():
        if line.startswith('SF:'):
            require(current is None, 'unterminated LCOV before SF')
            current = line[3:]
            require(current not in records, f'duplicate LCOV source: {current}')
            records[current] = {}
        elif line.startswith('DA:'):
            require(current is not None, 'LCOV DA outside source')
            number, count = map(int, line[3:].split(',')[:2])
            require(number > 0 and count >= 0 and number not in records[current],
                    'invalid duplicate LCOV DA')
            records[current][number] = count
        elif line == 'end_of_record':
            require(current is not None, 'LCOV end without source')
            current = None
    require(current is None and records, 'empty or incomplete LCOV report')
    prefix = str(workspace.resolve()) + '/'
    rows = {}
    for filename, lines in records.items():
        relative = Path(filename.replace('\\', '/')).as_posix()
        if relative.startswith(prefix):
            relative = relative[len(prefix):]
        rows[relative] = lines
    source = ET.Element('sources')
    ET.SubElement(source, 'source').text = str(workspace.resolve())
    packages = ET.Element('packages')
    grouped = {}
    total_covered = total_valid = 0
    for relative, lines in sorted(rows.items()):
        parent = Path(relative).parent.as_posix()
        grouped.setdefault(parent, []).append((relative, lines))
        total_covered += sum(hit > 0 for hit in lines.values())
        total_valid += len(lines)
    for parent, classes in sorted(grouped.items()):
        package = ET.SubElement(packages, 'package', {
            'name': parent.replace('/', '.'),
            'line-rate': '0',
            'branch-rate': '0',
            'complexity': '0',
        })
        elements = []
        package_covered = package_valid = 0
        for relative, lines in classes:
            covered = sum(hit > 0 for hit in lines.values())
            package_covered += covered
            package_valid += len(lines)
            klass = ET.Element('class', {
                'name': relative.replace('/', '.'),
                'filename': relative,
                'line-rate': str(covered / len(lines)) if lines else '0',
                'branch-rate': '0',
                'complexity': '0',
            })
            ET.SubElement(klass, 'methods')
            line_nodes = ET.SubElement(klass, 'lines')
            for number, hits in sorted(lines.items()):
                ET.SubElement(line_nodes, 'line', {
                    'number': str(number), 'hits': str(hits), 'branch': 'false'})
            elements.append(klass)
        for element in elements:
            package.append(element)
        package.set('line-rate', str(package_covered / package_valid) if package_valid else '0')
    root = ET.Element('coverage', {
        'timestamp': str(int(time.time())),
        'version': 'llvm-cov',
        'lines-valid': str(total_valid),
        'lines-covered': str(total_covered),
        'line-rate': str(total_covered / total_valid) if total_valid else '0',
        'branches-valid': '0',
        'branches-covered': '0',
        'branch-rate': '0',
        'complexity': '0',
    })
    root.append(source)
    root.append(packages)
    ET.indent(root, space='  ')
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(root, encoding='unicode') + '\n'


def assemble_risk(base_shard: Path, head_shard: Path, output: Path) -> dict:
    """Assemble two independently collected risk workers into one shard.

    Side-specific required evidence must be present exactly once. Shared
    telemetry (for example the measurer download log) is retained once when
    the bytes agree; differing copies are namespaced by side instead of being
    silently overwritten. The comparison itself runs only after both side
    candidates and every listed digest have been validated.
    """
    output = output.resolve()
    require(not output.exists(), 'risk shard directory already exists; choose a fresh path')
    shards = {'base': Path(base_shard), 'head': Path(head_shard)}
    reports: dict[str, dict] = {}
    evidence: dict[str, dict[str, str]] = {}
    tool_identities: set[str] = set()
    identities: set[tuple[str, str, str]] = set()
    measurers: set[str] = set()
    for name in RISK_SIDES:
        report, local, tool_document, measurer = _risk_side_candidate(name, shards[name])
        reports[name] = report
        evidence[name] = local
        tool_identities.add(tool_document)
        identities.add((report['base_sha'], report['commit'], report['run_id']))
        measurers.add(json.dumps(measurer, sort_keys=True))
    require(len(identities) == 1, f'risk side identity mismatch: {identities}')
    require(len(tool_identities) == 1, 'risk side tool identity mismatch')
    require(len(measurers) == 1, 'conflicting measurer pins across risk sides')

    grouped: dict[str, list[tuple[str, str]]] = {}
    for name in RISK_SIDES:
        for relative, digest in evidence[name].items():
            grouped.setdefault(relative, []).append((name, digest))
    copies: dict[str, tuple[str, str]] = {}
    for relative, entries in grouped.items():
        if len({digest for _, digest in entries}) == 1:
            copies[relative] = (entries[0][0], relative)
            continue
        for name, _ in entries:
            destination = f'shared/{name}/{relative}'
            require(destination not in copies, f'colliding namespaced evidence: {destination}')
            copies[destination] = (name, relative)

    output.mkdir(parents=True)
    artifacts: dict[str, str] = {}
    for destination, (source_side, source_relative) in copies.items():
        canonical = _canonical_relative(destination)
        target = (output / canonical).resolve()
        require(target.is_relative_to(output), f'target escapes risk shard: {destination}')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((shards[source_side].resolve() / source_relative).read_bytes())
        artifacts[canonical] = sha256(target)
    require(RISK_REQUIRED_ARTIFACTS <= artifacts.keys(), 'assembled risk shard is incomplete')
    require('risk.json' not in artifacts and 'risk.md' not in artifacts,
            'risk side evidence must not pre-claim the assembled comparison')
    require('risk-compare.log' not in artifacts, 'risk side evidence collides with comparison log')

    base_sha, commit, run_id = next(iter(identities))
    compare_command = [sys.executable, str(ROOT / 'tools/quality/source_measure.py'), 'compare',
                       '--base', str(output / 'base-risk.json'),
                       '--head', str(output / 'head-risk.json'),
                       '--output', str(output / 'risk.json')]
    compare_started = time.monotonic()
    with (output / 'risk-compare.log').open('w') as log:
        code = subprocess.run(compare_command, cwd=ROOT, stdout=log,
                              stderr=subprocess.STDOUT).returncode
    artifacts['risk-compare.log'] = sha256(output / 'risk-compare.log')
    require(code == 0, f'risk comparison exited {code}; see risk-compare.log')
    comparison = json.loads((output / 'risk.json').read_text())
    (output / 'risk.md').write_text(risk_markdown(base_sha, commit, comparison))
    artifacts['risk.json'] = sha256(output / 'risk.json')
    artifacts['risk.md'] = sha256(output / 'risk.md')
    (output / SHARD_TOOL_IDENTITY).write_text(next(iter(tool_identities)))

    side_seconds = {}
    for name in RISK_SIDES:
        seconds = reports[name]['stages']['risk'].get('seconds')
        require(isinstance(seconds, (int, float)) and seconds >= 0,
                f'{name}: invalid risk duration')
        side_seconds[name] = seconds
    commands = []
    for name in RISK_SIDES:
        commands.extend(reports[name]['commands'])
    commands.append({'name': 'risk-compare', 'argv': compare_command, 'exit_code': code,
                     'seconds': time.monotonic() - compare_started})
    assembled = {**metadata(), 'schema_version': 1, 'candidate': True,
                 'base_sha': base_sha, 'commit': commit, 'run_id': run_id,
                 'stages': {'risk': {
                     'status': 'success',
                     'seconds': max(side_seconds.values()),
                     'workers': len(RISK_SIDES),
                     'side_seconds': side_seconds,
                 }},
                 'commands': commands, 'artifacts': artifacts,
                 'measurer': json.loads(next(iter(measurers)))}
    write_json(output / 'candidate.json', assembled)
    return assembled


def _merge_lcov_files(paths: list[Path]) -> str:
    """Merge disjoint LCOV partitions by summing line/function/branch hits."""
    records: dict[str, dict] = {}
    for path in paths:
        current = None
        for raw in path.read_text().splitlines():
            if raw.startswith('SF:'):
                current = records.setdefault(raw[3:], {
                    'functions': {}, 'function_hits': defaultdict(int),
                    'lines': defaultdict(int), 'branches': defaultdict(int)})
            elif raw.startswith('FN:'):
                require(current is not None, 'LCOV function before SF record')
                line_text, name = raw[3:].split(',', 1)
                line = int(line_text)
                prior = current['functions'].setdefault(name, line)
                require(prior == line, f'conflicting LCOV function line: {name}')
            elif raw.startswith('FNDA:'):
                require(current is not None, 'LCOV function hit before SF record')
                count_text, name = raw[5:].split(',', 1)
                current['function_hits'][name] += int(count_text)
            elif raw.startswith('DA:'):
                require(current is not None, 'LCOV line before SF record')
                line_text, count_text = raw[3:].split(',', 1)
                current['lines'][int(line_text)] += int(count_text)
            elif raw.startswith('BRDA:'):
                require(current is not None, 'LCOV branch before SF record')
                line_text, block, branch, count_text = raw[5:].split(',', 3)
                count = 0 if count_text == '-' else int(count_text)
                current['branches'][(int(line_text), int(block), branch)] += count
    output = []
    for source in sorted(records):
        record = records[source]
        output.append(f'SF:{source}')
        for name, line in sorted(record['functions'].items(), key=lambda item: (item[1], item[0])):
            output.append(f'FN:{line},{name}')
        for name in sorted(record['function_hits']):
            output.append(f'FNDA:{record["function_hits"][name]},{name}')
        output.append(f'FNF:{len(record["functions"])}')
        output.append(f'FNH:{sum(record["function_hits"].get(name, 0) > 0 for name in record["functions"])}')
        for line in sorted(record['lines']):
            output.append(f'DA:{line},{record["lines"][line]}')
        for key in sorted(record['branches']):
            output.append(f'BRDA:{key[0]},{key[1]},{key[2]},{record["branches"][key]}')
        output.append(f'BRF:{len(record["branches"])}')
        output.append(f'BRH:{sum(value > 0 for value in record["branches"].values())}')
        output.append(f'LF:{len(record["lines"])}')
        output.append(f'LH:{sum(value > 0 for value in record["lines"].values())}')
        output.append('end_of_record')
    require(bool(output), 'no LCOV records to merge')
    return '\n'.join(output) + '\n'


def _line_rate(covered: int, total: int) -> str:
    return str(0 if total == 0 else covered / total)


def _merge_cobertura_files(paths: list[Path]) -> str:
    """Merge Cobertura method/class line hits while retaining class/method shape."""
    sources: list[str] = []
    source_seen: set[str] = set()
    packages: dict[str, dict[str, dict]] = {}
    versions = set()
    for path in paths:
        root = ET.parse(path).getroot()
        versions.add(root.get('version', '2.0.3'))
        for source in root.findall('./sources/source'):
            value = source.text or ''
            if value not in source_seen:
                source_seen.add(value)
                sources.append(value)
        for package in root.findall('./packages/package'):
            package_name = package.get('name', '')
            classes = packages.setdefault(package_name, {})
            for item in package.findall('./classes/class'):
                filename = item.get('filename', '')
                slot = classes.setdefault(filename, {
                    'name': item.get('name', filename), 'lines': defaultdict(int),
                    'methods': {}})
                for line in item.findall('./lines/line'):
                    slot['lines'][int(line.get('number'))] += int(line.get('hits', '0'))
                for method in item.findall('./methods/method'):
                    key = (method.get('name', ''), method.get('signature', ''))
                    method_slot = slot['methods'].setdefault(key, {
                        'complexity': method.get('complexity', '0'),
                        'lines': defaultdict(int)})
                    for line in method.findall('./lines/line'):
                        method_slot['lines'][int(line.get('number'))] += int(line.get('hits', '0'))
    require(bool(packages), 'no Cobertura packages to merge')
    root = ET.Element('coverage', {
        'branch-rate': '0', 'branches-covered': '0', 'branches-valid': '0',
        'complexity': '0', 'timestamp': '0',
        'version': sorted(versions)[0] if len(versions) == 1 else '2.0.3'})
    source_root = ET.SubElement(root, 'sources')
    for source in sources:
        ET.SubElement(source_root, 'source').text = source
    package_root = ET.SubElement(root, 'packages')
    total_covered = total_valid = 0
    for package_name, classes in packages.items():
        package = ET.SubElement(package_root, 'package', {
            'branch-rate': '0', 'complexity': '0', 'name': package_name})
        class_root = ET.SubElement(package, 'classes')
        package_covered = package_valid = 0
        for filename, slot in classes.items():
            lines = slot['lines']
            covered = sum(value > 0 for value in lines.values())
            valid = len(lines)
            package_covered += covered
            package_valid += valid
            item = ET.SubElement(class_root, 'class', {
                'branch-rate': '0', 'complexity': '0', 'filename': filename,
                'line-rate': _line_rate(covered, valid), 'name': slot['name']})
            method_root = ET.SubElement(item, 'methods')
            for (name, signature), method in slot['methods'].items():
                method_lines = method['lines']
                method_covered = sum(value > 0 for value in method_lines.values())
                method_valid = len(method_lines)
                method_element = ET.SubElement(method_root, 'method', {
                    'name': name, 'signature': signature,
                    'complexity': method['complexity'],
                    'line-rate': _line_rate(method_covered, method_valid),
                    'branch-rate': '0'})
                method_lines_element = ET.SubElement(method_element, 'lines')
                for number in sorted(method_lines):
                    ET.SubElement(method_lines_element, 'line', {
                        'hits': str(method_lines[number]), 'number': str(number),
                        'branch': 'false'})
            lines_element = ET.SubElement(item, 'lines')
            for number in sorted(lines):
                ET.SubElement(lines_element, 'line', {
                    'hits': str(lines[number]), 'number': str(number),
                    'branch': 'false'})
        package.set('line-rate', _line_rate(package_covered, package_valid))
        total_covered += package_covered
        total_valid += package_valid
    root.set('line-rate', _line_rate(total_covered, total_valid))
    root.set('lines-covered', str(total_covered))
    root.set('lines-valid', str(total_valid))
    ET.indent(root, space='    ')
    prefix = ('<?xml version="1.0" ?>\n'
              '<!DOCTYPE coverage SYSTEM "https://cobertura.sourceforge.net/xml/coverage-04.dtd">\n')
    return prefix + ET.tostring(root, encoding='unicode') + '\n'


def _function_structure(function: dict):
    require(not function.get('branches') and not function.get('mcdc_records'),
            'branch/MCDC coverage is unsupported for risk sharding')
    regions = function.get('regions')
    require(isinstance(regions, list) and regions, 'LLVM function without regions')
    return (function['name'], tuple(function['filenames']),
            tuple((r[0], r[1], r[2], r[3], r[5], r[6], r[7]) for r in regions))


def _merge_llvm_coverage_files(paths: list[Path]) -> dict:
    """Combine disjoint LLVM exports; each partition contains the full function list."""
    documents = [json.loads(path.read_text()) for path in paths]
    require(documents, 'no LLVM coverage documents to merge')
    for document in documents:
        require(document.get('type') == 'llvm.coverage.json.export' and
                isinstance(document.get('data'), list) and len(document['data']) == 1,
                'invalid LLVM coverage document')
    merged = {}
    for document in documents:
        for function in document['data'][0]['functions']:
            key = _function_structure(function)
            if key not in merged:
                merged[key] = copy.deepcopy(function)
                continue
            target = merged[key]
            require(len(target['regions']) == len(function['regions']),
                    'LLVM region layout changed across partitions')
            target['count'] += function['count']
            for target_region, source_region in zip(target['regions'], function['regions']):
                target_region[4] += source_region[4]
    output = {key: copy.deepcopy(documents[0][key])
              for key in ('type', 'version', 'cargo_llvm_cov') if key in documents[0]}
    output['data'] = [{'files': [], 'functions': list(merged.values()), 'totals': {}}]
    return output


def _partial_function_identity(row: dict) -> tuple:
    return (row['source'], row['kind'], row['name'], tuple(row['span']),
            row['syntax_sha256'], row['source_sha256'], row['cc'], row['selected'])


def _merge_risk_side_reports(paths: list[Path], *, manifest_sha256: str,
                             llvm_sha256: str) -> dict:
    """Merge disjoint partial risk reports and recompute coverage/CRAP fields."""
    reports = [json.loads(path.read_text()) for path in paths]
    require(reports, 'no partial risk reports to merge')
    first = reports[0]
    for report in reports:
        for key in ('series', 'configuration', 'tools', 'source_inventory'):
            require(report.get(key) == first.get(key), f'partial risk {key} mismatch')
        require(report.get('manifest_sha256') == first.get('manifest_sha256'),
                'partial risk manifest mismatch')
    identities = [[_partial_function_identity(row) for row in report['functions']]
                  for report in reports]
    require(all(rows == identities[0] for rows in identities),
            'partial risk function inventory/order mismatch')
    functions = []
    for index, base in enumerate(first['functions']):
        regions: dict[tuple, int] = defaultdict(int)
        lines: dict[int, int] = defaultdict(int)
        instances: dict[tuple, int] = defaultdict(int)
        for report in reports:
            row = report['functions'][index]
            require('line_hits' in row, 'partial risk report lacks per-line hits')
            for item in row['raw_regions']:
                regions[(tuple(item['span']), item['kind'])] += item['count']
            for line, count in row['line_hits']:
                lines[int(line)] += count
            for instance in row['instances']:
                instances[(instance['index'], instance['name'])] += instance['count']
        region_map = {(*span, kind): count for (span, kind), count in regions.items()}
        line_counts = counts(lines)
        code = counts({region: count for region, count in region_map.items() if region[4] == 0})
        score = crap_line(base['cc'], line_counts['covered'], line_counts['count'])
        passed = (line_counts['covered'] * 5 >= line_counts['count'] * 4 and
                  code['covered'] * 5 >= code['count'] * 4 and score <= 30)
        row = copy.deepcopy(base)
        row.update({
            'instances': [{'index': index_value, 'name': name, 'count': count}
                          for (index_value, name), count in sorted(instances.items())],
            'lines': line_counts, 'regions': code,
            'crap_line': float(score),
            'crap_exact': [score.numerator, score.denominator],
            'passed': passed,
            'raw_regions': [{'span': list(span), 'kind': kind, 'count': count}
                            for (span, kind), count in sorted(regions.items())],
            'line_hits': [[line, count] for line, count in sorted(lines.items())],
        })
        functions.append(row)
    report = {key: copy.deepcopy(first[key])
              for key in ('series', 'configuration', 'tools', 'source_inventory')}
    report.update({
        'functions': functions,
        'manifest_sha256': manifest_sha256,
        'llvm_sha256': llvm_sha256,
        'selected_failures': [f"{r['source']}::{r['name']}" for r in functions
                              if r['selected'] and not r['passed']],
        'nonselected_debt': [f"{r['source']}::{r['name']}" for r in functions
                             if not r['selected'] and not r['passed']],
    })
    return report


def _risk_shard_required(side: str, shard: int) -> frozenset[str]:
    return frozenset({
        f'{side}-manifest.json', f'{side}-source.tar',
        f'{side}-coverage-{shard}.json', f'{side}-coverage-{shard}.lcov',
        f'{side}-coverage-{shard}.cobertura.xml', f'{side}-risk-{shard}.json',
    })


def _risk_shard_candidate(side: str, shard: int, directory: Path):
    require(side in RISK_SIDES, f'unknown risk side: {side}')
    directory = directory.resolve()
    require(directory.is_dir(), f'{side}-{shard}: risk shard directory missing')
    report = json.loads((directory / 'candidate.json').read_text())
    require(report.get('schema_version') == 1 and report.get('candidate') is True,
            f'{side}-{shard}: invalid candidate schema')
    require(set(report.get('stages') or {}) == {'risk'},
            f'{side}-{shard}: must record exactly the risk stage')
    stage = report['stages']['risk']
    require(stage.get('status') == 'success' and stage.get('side') == side and
            stage.get('shard') == shard and isinstance(stage.get('partitions'), int),
            f'{side}-{shard}: invalid risk shard stage')
    require(stage['partitions'] >= 2 and 1 <= shard <= stage['partitions'],
            f'{side}-{shard}: invalid partition identity')
    identity = (report.get('base_sha'), report.get('commit'), report.get('run_id'))
    require(all(re.fullmatch('[0-9a-f]{40}', str(value or '')) for value in identity[:2]) and
            isinstance(identity[2], str) and identity[2], f'{side}-{shard}: invalid identity')
    artifacts = report.get('artifacts')
    require(isinstance(artifacts, dict), f'{side}-{shard}: invalid artifact inventory')
    local = {}
    for relative, digest in artifacts.items():
        canonical = _canonical_relative(relative)
        require(isinstance(digest, str) and re.fullmatch('[0-9a-f]{64}', digest),
                f'{side}-{shard}: invalid artifact digest: {relative}')
        artifact = (directory / canonical).resolve()
        require(artifact.is_relative_to(directory), f'{side}-{shard}: artifact escapes shard')
        require(artifact.is_file() and sha256(artifact) == digest,
                f'{side}-{shard}: stale or modified artifact: {relative}')
        local[canonical] = digest
    require(_risk_shard_required(side, shard) <= local.keys(),
            f'{side}-{shard}: missing shard evidence')
    other = 'head' if side == 'base' else 'base'
    require(not any(path.startswith(other + '-') for path in local),
            f'{side}-{shard}: contains other-side evidence')
    tool_file = directory / SHARD_TOOL_IDENTITY
    require(tool_file.is_file(), f'{side}-{shard}: missing {SHARD_TOOL_IDENTITY}')
    tool_document = _validated_tool_identity(f'{side}-{shard}', tool_file)
    measurer = report.get('measurer')
    require(isinstance(measurer, dict) and measurer, f'{side}-{shard}: missing measurer pin')
    return report, local, tool_document, measurer


def assemble_risk_shards(shards_root: Path, output: Path) -> dict:
    """Merge per-side test shards, then assemble the ordinary risk shard."""
    shards_root = Path(shards_root).resolve()
    require(shards_root.is_dir(), 'risk shard root missing')
    candidates = {}
    pattern = re.compile(r'quality-artifacts-risk-(base|head)-(\d+)-')
    for child in sorted(shards_root.iterdir()):
        if not child.is_dir():
            continue
        match = pattern.match(child.name)
        if not match:
            continue
        key = (match[1], int(match[2]))
        require(key not in candidates, f'duplicate risk shard artifact: {key}')
        candidates[key] = child
    require(candidates, 'no risk shard artifacts found')
    sides: dict[str, dict[int, Path]] = {side: {} for side in RISK_SIDES}
    partitions = set()
    identities = set()
    tools = set()
    measurers = set()
    for (side, shard), directory in candidates.items():
        report, _local, tool_document, measurer = _risk_shard_candidate(side, shard, directory)
        require(side in sides, f'invalid risk side: {side}')
        sides[side][shard] = directory
        partitions.add(report['stages']['risk']['partitions'])
        identities.add((report['base_sha'], report['commit'], report['run_id']))
        tools.add(tool_document)
        measurers.add(json.dumps(measurer, sort_keys=True))
    require(len(partitions) == 1, f'risk shard partition count mismatch: {partitions}')
    partition_count = next(iter(partitions))
    require(all(set(shards) == set(range(1, partition_count + 1))
                for shards in sides.values()), 'incomplete risk shard set')
    require(len(identities) == 1, f'risk shard identity mismatch: {identities}')
    require(len(tools) == 1, 'risk shard tool identity mismatch')
    require(len(measurers) == 1, 'conflicting measurer pins across risk shards')
    base_sha, commit, run_id = next(iter(identities))
    tool_document = next(iter(tools))
    measurer = json.loads(next(iter(measurers)))

    with tempfile.TemporaryDirectory(prefix='gatehouse-risk-side-') as temporary:
        side_directories = {}
        for side in RISK_SIDES:
            ordered = [sides[side][shard] for shard in range(1, partition_count + 1)]
            reports = []
            local_evidence = []
            for directory in ordered:
                report, local, _tool, _measurer = _risk_shard_candidate(
                    side, int(re.search(r'quality-artifacts-risk-\w+-(\d+)-', directory.name)[1]),
                    directory)
                reports.append(report)
                local_evidence.append(local)
            shared_names = {f'{side}-manifest.json', f'{side}-source.tar'}
            for relative in shared_names:
                require(len({evidence[relative] for evidence in local_evidence}) == 1,
                        f'{side}: shared shard evidence differs: {relative}')
            destination = Path(temporary) / side
            destination.mkdir()
            report_dir = ordered[0]
            for relative in shared_names:
                (destination / relative).write_bytes((report_dir / relative).read_bytes())
            coverage_paths = [directory / f'{side}-coverage-{shard}.json'
                              for shard, directory in enumerate(ordered, 1)]
            lcov_paths = [directory / f'{side}-coverage-{shard}.lcov'
                          for shard, directory in enumerate(ordered, 1)]
            cobertura_paths = [directory / f'{side}-coverage-{shard}.cobertura.xml'
                               for shard, directory in enumerate(ordered, 1)]
            risk_paths = [directory / f'{side}-risk-{shard}.json'
                          for shard, directory in enumerate(ordered, 1)]
            write_json(destination / f'{side}-coverage.json',
                       _merge_llvm_coverage_files(coverage_paths))
            (destination / f'{side}-coverage.lcov').write_text(
                _merge_lcov_files(lcov_paths))
            (destination / f'{side}-coverage.cobertura.xml').write_text(
                _merge_cobertura_files(cobertura_paths))
            manifest_sha = sha256(destination / f'{side}-manifest.json')
            llvm_sha = sha256(destination / f'{side}-coverage.json')
            write_json(destination / f'{side}-risk.json',
                       _merge_risk_side_reports(
                           risk_paths, manifest_sha256=manifest_sha, llvm_sha256=llvm_sha))
            (destination / SHARD_TOOL_IDENTITY).write_text(tool_document)
            artifacts = {}
            for artifact in destination.rglob('*'):
                relative = artifact.relative_to(destination)
                if artifact.is_file() and relative.parts[0] not in (
                        'candidate.json', SHARD_TOOL_IDENTITY):
                    artifacts[str(relative)] = sha256(artifact)
            side_seconds = max(float(report['stages']['risk'].get('seconds', 0))
                               for report in reports)
            commands = [command for report in reports for command in report.get('commands', [])]
            side_candidate = {
                **metadata(), 'schema_version': 1, 'candidate': True,
                'base_sha': base_sha, 'commit': commit, 'run_id': run_id,
                'stages': {'risk': {'status': 'success', 'side': side,
                                     'seconds': side_seconds}},
                'commands': commands, 'artifacts': artifacts, 'measurer': measurer}
            write_json(destination / 'candidate.json', side_candidate)
            side_directories[side] = destination
        return assemble_risk(side_directories['base'], side_directories['head'], output)


def merge(output: Path, shards: dict[str, Path]) -> dict:
    """Merge shard candidates into one fresh output without overwriting evidence.

    Fail-closed checks before any file is copied:
    - exactly the expected shards, each with a clean candidate.json;
    - identical base/head/run identity across shards;
    - every shard owns exactly its expected stages, all successful;
    - artifact paths are canonical and reserved names are rejected;
    - every listed artifact digest matches the shard's own bytes;
    - no canonical artifact path is claimed by more than one shard;
    - every shard recorded a valid, identical collection tool identity;
    - at most one measurer pin, agreed by every shard that records one.
    """
    output = output.resolve()
    require(not output.exists(), 'candidate directory already exists; choose a fresh path')
    require(set(shards) == set(SHARD_STAGES), f'shards must be exactly {sorted(SHARD_STAGES)}')

    reports = {}
    identity = set()
    tool_identities = set()
    claimed: dict[str, str] = {}
    copies: dict[str, list[str]] = {name: [] for name in SHARD_STAGES}
    measurer = {}
    for name, directory in shards.items():
        directory = directory.resolve()
        report = json.loads((directory / 'candidate.json').read_text())
        require(report['schema_version'] == 1 and report['candidate'] is True,
                f'{name}: invalid candidate schema')
        require(set(report['stages']) == set(SHARD_STAGES[name]),
                f'{name}: stages must be exactly {sorted(SHARD_STAGES[name])}')
        require(all(s['status'] == 'success' for s in report['stages'].values()),
                f'{name}: failed or incomplete stages')
        identity.add((report['base_sha'], report['commit'], report['run_id']))
        tool_file = directory / SHARD_TOOL_IDENTITY
        require(tool_file.is_file(), f'{name}: missing {SHARD_TOOL_IDENTITY}')
        tool_identities.add(_validated_tool_identity(name, tool_file))
        if 'measurer' in report:
            measurer[name] = report['measurer']
        for relative, digest in report['artifacts'].items():
            canonical = _canonical_relative(relative)
            artifact = (directory / canonical).resolve()
            require(artifact.is_relative_to(directory), f'{name}: artifact escapes shard: {relative}')
            require(artifact.is_file() and sha256(artifact) == digest,
                    f'{name}: stale or modified artifact: {relative}')
            require(canonical not in claimed,
                    f'duplicate artifact path in {name} and {claimed.get(canonical)}: {relative}')
            claimed[canonical] = name
            copies[name].append(canonical)
        reports[name] = report

    require(len(identity) == 1, f'shard identity mismatch: {identity}')
    require(len(tool_identities) == 1, 'shard tool identity mismatch')
    require(len(set(json.dumps(v, sort_keys=True) for v in measurer.values())) <= 1,
            'conflicting measurer pins across shards')

    output.mkdir(parents=True)
    for name in SHARD_STAGES:
        for relative in copies[name]:
            target = (output / relative).resolve()
            require(target.is_relative_to(output), f'target escapes output: {relative}')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((shards[name].resolve() / relative).read_bytes())
    (output / SHARD_TOOL_IDENTITY).write_text(next(iter(tool_identities)))

    base_sha, commit, run_id = next(iter(identity))
    merged = {**metadata(), 'schema_version': 1, 'candidate': True,
              'base_sha': base_sha, 'commit': commit, 'run_id': run_id,
              'stages': {}, 'commands': [], 'artifacts': {}}
    if measurer:
        merged['measurer'] = next(iter(measurer.values()))
    for name in SHARD_STAGES:
        report = reports[name]
        merged['stages'].update(report['stages'])
        merged['commands'].extend(report['commands'])
        merged['artifacts'].update(report['artifacts'])
    write_json(output / 'candidate.json', merged)
    return merged


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

    def output_command(self, name: str, command: list[str], output: Path,
                       allowed=(0,), env=None):
        """Run a command whose stdout is itself the retained artifact."""
        started = time.monotonic()
        print(f'{name}: started (log: {name}.log)', flush=True)
        effective_env = env if env is not None else self.environment
        output.parent.mkdir(parents=True, exist_ok=True)
        with (self.directory / f'{name}.log').open('w') as log, output.open('wb') as stream:
            code = subprocess.run(command, cwd=ROOT, env=effective_env,
                                  stdout=stream, stderr=log).returncode
        elapsed = time.monotonic() - started
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
                    'tools/harness-gate/src/config/toml_fuzz.rs',
                    'tools/harness-gate/src/findings/tests.rs',
                    'tools/harness-gate/src/preset/tests.rs',
                    'tools/harness-gate/src/scope/tests.rs',
                    'tools/harness-gate/src/process/tests.rs',
                    'tools/harness-gate/src/process/runtime_lifecycle_host/tests.rs',
                    'tools/harness-gate/src/process/podman_capture_host/tests.rs',
                    'tools/harness-gate/src/process/podman_capture_host/fault_fixture.rs',
                    'tools/harness-gate/src/process/runtime_lifecycle_host/tests/joint.rs',
                    'tools/harness-gate/src/doctor/tests.rs',
                    'tools/harness-gate/src/service/tests.rs',
                    'tools/harness-gate/src/service/tests/availability.rs',
                    'tools/harness-gate/src/test_support.rs',
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

    def _lifecycle_coverage_inputs(self, label, crate, target, env):
        # The base snapshot predates lifecycle; do not change its features or protocol.
        from lifecycle_fixture import supported, environment, FEATURE
        if not target.endswith('linux-gnu') or not supported(crate):
            return [], env
        bundle = self.directory / f'{label}-lifecycle-bundle'
        self.python(f'{label}-lifecycle-build', 'lifecycle_fixture.py', 'build',
                    '--crate', crate, '--bundle', bundle,
                    '--work', self.directory / f'{label}-lifecycle-build', env=env)
        # Keep Unix socket paths short and all journal ancestors protected.
        import tempfile
        parent = Path(os.environ.get('GATEHOUSE_JOURNAL_TEST_ROOT', Path.home()))
        journal_parent = Path(tempfile.mkdtemp(prefix='.gh-lifecycle-', dir=parent))
        values = environment(crate, bundle, journal_parent / 'journal')
        return ['--features', FEATURE], {**(os.environ if env is None else env), **values}

    def _collect_commit_coverage(self, label: str, commit: str, snapshot: Path, binary: Path, target: str, *, env=None):
        """Shared per-commit coverage collection for both modes."""
        crate = snapshot / 'tools/harness-gate'
        manifest = self.directory / f'{label}-manifest.json'
        features, env = self._lifecycle_coverage_inputs(label, crate, target, env)

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
            ['cargo', 'llvm-cov', 'nextest', '--workspace', '--locked', *features,
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

    def _collect_commit_coverage_shard(self, side: str, commit: str, snapshot: Path,
                                       binary: Path, target: str, shard: int,
                                       partitions: int, *, excluded_tests: str = '',
                                       extra_tests: str = '', env=None):
        """Run one test partition and retain partial coverage/risk evidence."""
        crate = snapshot / 'tools/harness-gate'
        manifest = self.directory / f'{side}-manifest.json'
        coverage = self.directory / f'{side}-coverage-{shard}.json'
        features, env = self._lifecycle_coverage_inputs(side, crate, target, env)

        self.python(
            f'{side}-prepare',
            'source_measure.py', 'prepare',
            '--crate', crate,
            '--binary', binary,
            '--manifest', manifest,
            '--target', target, env=env
        )

        cargo_manifest = str(crate / 'Cargo.toml')
        nextest = ['cargo', 'llvm-cov', 'nextest', '--workspace', '--locked', *features,
                   '--manifest-path', cargo_manifest, '--target', target]
        partition = [*nextest, '--partition', f'hash:{shard}/{partitions}', '--no-report']
        if excluded_tests:
            partition.extend(['-E', f'not ({excluded_tests})'])
        self.command(f'{side}-coverage-{shard}', partition, env=env)

        if extra_tests:
            self.command(
                f'{side}-coverage-extra-{shard}',
                [*nextest, '--no-report', '-E', extra_tests], env=env
            )

        # Generate one report after both invocations so all retained profiles
        # participate in every evidence format.
        self.command(
            f'{side}-coverage-json-{shard}',
            ['cargo', 'llvm-cov', 'report', '--workspace',
             '--manifest-path', cargo_manifest, '--target', target,
             '--json', '--output-path', str(coverage)], env=env
        )

        for flag, suffix in (('--lcov', 'lcov'), ('--cobertura', 'cobertura.xml')):
            self.command(
                f'{side}-{suffix}-{shard}',
                ['cargo', 'llvm-cov', 'report', '--workspace',
                 '--manifest-path', cargo_manifest, '--target', target,
                 flag, '--output-path',
                 str(self.directory / f'{side}-coverage-{shard}.{suffix}')], env=env
            )

        self.python(
            f'{side}-measure-{shard}',
            'source_measure.py', 'measure-partial',
            '--crate', crate,
            '--binary', binary,
            '--manifest', manifest,
            '--llvm', coverage,
            '--output', self.directory / f'{side}-risk-{shard}.json',
            env=env
        )

        report = json.loads((self.directory / f'{side}-risk-{shard}.json').read_text())
        require(report['manifest_sha256'] == sha256(manifest), f'{side}-{shard}: stale risk manifest')
        require(report['llvm_sha256'] == sha256(coverage), f'{side}-{shard}: stale risk profiles')
        original = json.loads(manifest.read_text())
        for path, item in original['files'].items():
            relative = (ROOT / 'tools/harness-gate/src' / path).resolve().relative_to(ROOT)
            committed = subprocess.check_output(['git', 'show', f'{commit}:{relative}'], cwd=ROOT)
            require(item['original'].encode() == committed,
                    f'{side}-{shard}: risk source/commit mismatch for {path}')
        for path in original['absent_sources']:
            relative = (ROOT / 'tools/harness-gate/src' / path).resolve().relative_to(ROOT)
            exists = subprocess.run(['git', 'cat-file', '-e', f'{commit}:{relative}'],
                                    cwd=ROOT, capture_output=True).returncode == 0
            require(not exists, f'{side}-{shard}: absent risk source exists in commit: {relative}')
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
        (self.directory / 'risk.md').write_text(risk_markdown(base, head, comparison))

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

    def _risk_side(self, side: str):
        """Collect exactly one side of the risk pair for an independent worker."""
        require(side in RISK_SIDES, f'unknown risk side: {side}')
        base, head = self.report['base_sha'], self.report['commit']
        self._validate_risk_scope(base, head)
        binary = self.measurer()
        target = compiler_configuration()['target']
        commit = base if side == 'base' else head
        snapshot = self.snapshot(side, commit)
        self._collect_commit_coverage(side, commit, snapshot, binary, target)

    def _record_artifacts(self):
        self.report['artifacts'] = {}
        for artifact in self.directory.rglob('*'):
            relative = artifact.relative_to(self.directory)
            if relative.parts[0] not in ('build', 'snapshots', 'candidate.json') and artifact.is_file():
                self.report['artifacts'][str(relative)] = sha256(artifact)
        self.save()

    def assemble_fast_coverage(self, archive: Path, profdata_root: Path, partitions: int):
        """Assemble archive-backed legacy/production evidence from test shards."""
        require(partitions >= 2, f'invalid fast coverage partition count: {partitions}')
        archive = Path(archive).resolve()
        profdata_root = Path(profdata_root).resolve()
        require(archive.is_file(), f'fast coverage archive missing: {archive}')
        require(profdata_root.is_dir(), f'fast coverage profile root missing: {profdata_root}')
        profiles = sorted(profdata_root.glob('fast-shard-*.profdata'))
        require(len(profiles) == partitions,
                f'expected {partitions} fast shard profiles, found {len(profiles)}')

        started = time.monotonic()
        self.report['stages'] = {'legacy': {'status': 'running'}}
        self.save()
        current = 'legacy'
        try:
            with tempfile.TemporaryDirectory(prefix='gatehouse-fast-coverage-') as temporary:
                extract = Path(temporary)
                self.command('fast-archive-extract', [
                    'tar', '--zstd', '-xf', str(archive), '-C', str(extract)])
                target = extract / 'target'
                debug = extract / 'debug'
                debug.symlink_to(target / 'debug', target_is_directory=True)
                objects = _fast_archive_objects(target, debug)
                require(objects, 'fast archive has no coverage objects')
                merged = extract / 'merged.profdata'
                self.command('fast-profdata-merge', [
                    str(_llvm_tool('llvm-profdata')), 'merge', '-sparse',
                    *map(str, profiles), '-o', str(merged)])
                raw = self.directory / 'coverage.raw.json'
                lcov = self.directory / 'coverage.lcov'
                cobertura = self.directory / 'coverage.cobertura.xml'
                object_args = [item for obj in objects for item in ('-object', str(obj))]
                ignore = _fast_ignore_regex(target)
                llvm_cov = str(_llvm_tool('llvm-cov'))
                profile = f'-instr-profile={merged}'
                self.output_command('fast-coverage-json', [
                    llvm_cov, 'export', profile, *object_args,
                    '-ignore-filename-regex', ignore], raw)
                self.output_command('fast-coverage-lcov', [
                    llvm_cov, 'export', '-format=lcov', profile, *object_args,
                    '-ignore-filename-regex', ignore], lcov)
                cobertura.write_text(_lcov_to_cobertura(lcov, ROOT / 'tools/harness-gate'))
                self.python('fast-legacy', 'coverage.py', '--raw', raw,
                            '--output', self.directory / 'coverage.json')
                legacy_seconds = time.monotonic() - started
                self.report['stages']['legacy'] = {
                    'status': 'success', 'seconds': legacy_seconds}
                self.save()
                current = 'production'
                production_started = time.monotonic()
                self.python('fast-production', 'coverage.py', '--production',
                            '--raw', raw, '--lcov', lcov,
                            '--output', self.directory / 'production.json')
                self.report['stages']['production'] = {
                    'status': 'success', 'seconds': time.monotonic() - production_started}
        except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
            self.report['stages'][current] = {
                'status': 'failure', 'error': str(error)}
            self.save()
            require(False, f'failed to assemble fast coverage: {error}')
        self.report['collection_seconds'] = time.monotonic() - started
        self.save()
        self._record_artifacts()
        print('fast coverage assembled: legacy + production', flush=True)

    def collect_risk_side(self, side: str):
        """Run one risk worker and retain a partial candidate for assembly."""
        require(side in RISK_SIDES, f'unknown risk side: {side}')
        started = time.monotonic()
        tick = time.monotonic()
        self.report['stages']['risk'] = {'status': 'running', 'side': side}
        self.save()
        try:
            self._risk_side(side)
            result = {'status': 'success', 'side': side}
        except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
            result = {'status': 'failure', 'side': side, 'error': str(error)}
        result['seconds'] = time.monotonic() - tick
        self.report['stages']['risk'] = result
        self.save()
        print(f'risk-{side}: {result}', flush=True)
        require(result['status'] == 'success',
                f'failed, cancelled, skipped or incomplete risk {side} collection')
        require(git_sha() == self.report['commit'], 'commit changed during collection')
        require_committed_sources(self.report['commit'])
        self.report['collection_seconds'] = time.monotonic() - started
        self._record_artifacts()

    def collect_risk_shard(self, side: str, shard: int, partitions: int, *,
                           excluded_tests: str = '', extra_tests: str = ''):
        """Run one risk test partition; the merge job combines side shards."""
        require(side in RISK_SIDES, f'unknown risk side: {side}')
        require(partitions >= 2 and 1 <= shard <= partitions,
                f'invalid risk shard {shard}/{partitions}')
        started = time.monotonic()
        tick = time.monotonic()
        self.report['stages']['risk'] = {
            'status': 'running', 'side': side, 'shard': shard, 'partitions': partitions}
        self.save()
        try:
            base, head = self.report['base_sha'], self.report['commit']
            self._validate_risk_scope(base, head)
            binary = self.measurer()
            target = compiler_configuration()['target']
            commit = base if side == 'base' else head
            snapshot = self.snapshot(side, commit)
            self._collect_commit_coverage_shard(
                side, commit, snapshot, binary, target, shard, partitions,
                excluded_tests=excluded_tests, extra_tests=extra_tests)
            result = {'status': 'success', 'side': side, 'shard': shard,
                      'partitions': partitions}
        except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
            result = {'status': 'failure', 'side': side, 'shard': shard,
                      'partitions': partitions, 'error': str(error)}
        result['seconds'] = time.monotonic() - tick
        self.report['stages']['risk'] = result
        self.save()
        print(f'risk-{side}-{shard}/{partitions}: {result}', flush=True)
        require(result['status'] == 'success',
                f'failed, cancelled, skipped or incomplete risk {side} shard {shard}')
        require(git_sha() == self.report['commit'], 'commit changed during collection')
        require_committed_sources(self.report['commit'])
        self.report['collection_seconds'] = time.monotonic() - started
        self._record_artifacts()

    def matrix(self):
        self.python('matrix', 'critical_paths.py', '--collect', '--evidence',
                    self.directory / 'critical-path-runs/bundle.json', '--output',
                    self.directory / 'critical-paths.json')

    def collect(self, stages=None):
        """Collect evidence for specified stages.

        Args:
            stages: Tuple of stage names to run. If None, runs all STAGES.
                   Can be used to run a subset of stages for parallel job execution.
        """
        if stages is None:
            stages = STAGES
        else:
            require(len(stages) == len(set(stages)), f'duplicate stages: {stages}')
            for stage in stages:
                require(stage in STAGES, f'unknown stage: {stage}')
            if 'production' in stages:
                require('legacy' in stages, 'production coverage requires the legacy stage')

        started = time.monotonic()
        for stage in stages:
            tick = time.monotonic()
            self.report['stages'][stage] = {'status': 'running'}
            self.save()
            try:
                getattr(self, stage)()
                result = {'status': 'success'}
            except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
                result = {'status': 'failure', 'error': str(error)}
            result['seconds'] = time.monotonic() - tick
            self.report['stages'][stage] = result
            self.save()
            print(f'{stage}: {result}', flush=True)
        require(git_sha() == self.report['commit'], 'commit changed during collection')
        require_committed_sources(self.report['commit'])
        self.report['collection_seconds'] = time.monotonic() - started
        self._record_artifacts()
        # A partial collection is one shard of a split job; a failed shard must
        # exit non-zero here so the merge job is skipped rather than fed failure
        # evidence. Full collections keep the historical verify() failure path.
        if set(stages) == set(STAGES):
            verify(self.directory / 'candidate.json', self.report['commit'], self.report['base_sha'], self.report['run_id'])
        else:
            require(all(self.report['stages'][stage]['status'] == 'success' for stage in stages),
                    'failed, cancelled, skipped or incomplete collection')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('aggregate', 'classify', 'collect',
                                              'collect-risk-side', 'collect-risk-shard',
                                              'assemble-risk', 'assemble-risk-shards',
                                              'assemble-fast-coverage', 'verify',
                                              'certified-base', 'merge', 'tool-identity'))
    parser.add_argument('--event', default=os.environ.get('EVENT_NAME'))
    parser.add_argument('--needs', default=os.environ.get('NEEDS_JSON'))
    parser.add_argument('--base-sha')
    parser.add_argument('--head-sha')
    parser.add_argument('--run-id', default=str(uuid.uuid4()))
    parser.add_argument('--output', type=Path, default=ROOT / 'target/quality/candidate')
    parser.add_argument('--github-output', type=Path)
    parser.add_argument('--stages', type=str, help='Comma-separated list of stages to run (default: all)')
    parser.add_argument('--side', choices=RISK_SIDES, help='risk worker side')
    parser.add_argument('--risk-shard', type=int, help='risk test shard number')
    parser.add_argument('--partitions', type=int, help='number of test shards')
    parser.add_argument('--exclude-tests', default='',
                        help='nextest filterset excluded from every risk partition')
    parser.add_argument('--extra-tests', default='',
                        help='nextest filterset run once in the selected risk shard')
    parser.add_argument('--base-shard', type=Path, help='base risk worker candidate')
    parser.add_argument('--head-shard', type=Path, help='head risk worker candidate')
    parser.add_argument('--shards-root', type=Path, help='downloaded risk shard artifact root')
    parser.add_argument('--archive', type=Path, help='instrumented nextest archive')
    parser.add_argument('--profdata-root', type=Path, help='downloaded fast shard profdata root')
    parser.add_argument('--shard', action='append', default=[],
                        help='name=path shard for merge; repeat once per expected shard')
    args = parser.parse_args()
    try:
        if args.operation == 'tool-identity':
            write_json(args.output, tool_identity())
        elif args.operation == 'merge':
            shards = {}
            for item in args.shard:
                name, _, path = item.partition('=')
                require(name in SHARD_STAGES and path and name not in shards,
                        f'invalid --shard argument: {item}')
                shards[name] = Path(path)
            merge(args.output, shards)
        elif args.operation == 'assemble-risk':
            require(args.base_shard is not None and args.head_shard is not None,
                    'assemble-risk needs --base-shard and --head-shard')
            assemble_risk(args.base_shard, args.head_shard, args.output)
        elif args.operation == 'assemble-risk-shards':
            require(args.shards_root is not None,
                    'assemble-risk-shards needs --shards-root')
            assemble_risk_shards(args.shards_root, args.output)
        elif args.operation == 'certified-base':
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
            if args.operation in ('collect', 'collect-risk-side', 'collect-risk-shard',
                                  'assemble-fast-coverage'):
                require(git_sha() == args.head_sha, 'head must equal checked-out commit')
                require_committed_sources(args.head_sha)
                collector = Collector(args.output, args.base_sha, args.head_sha, args.run_id)
                if args.operation == 'collect':
                    stages = None
                    if args.stages:
                        stages = tuple(s.strip() for s in args.stages.split(','))
                    collector.collect(stages=stages)
                elif args.operation == 'collect-risk-side':
                    require(args.side in RISK_SIDES, 'collect-risk-side needs --side base|head')
                    collector.collect_risk_side(args.side)
                elif args.operation == 'collect-risk-shard':
                    require(args.side in RISK_SIDES, 'collect-risk-shard needs --side base|head')
                    require(args.risk_shard is not None and args.partitions is not None,
                            'collect-risk-shard needs --risk-shard and --partitions')
                    collector.collect_risk_shard(
                        args.side, args.risk_shard, args.partitions,
                        excluded_tests=args.exclude_tests, extra_tests=args.extra_tests)
                else:
                    require(args.archive is not None and args.profdata_root is not None
                            and args.partitions is not None,
                            'assemble-fast-coverage needs --archive, --profdata-root and --partitions')
                    collector.assemble_fast_coverage(
                        args.archive, args.profdata_root, args.partitions)
            else:
                verify(args.output / 'candidate.json', args.head_sha, args.base_sha, args.run_id)
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as error:
        print(f'quality CI failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
