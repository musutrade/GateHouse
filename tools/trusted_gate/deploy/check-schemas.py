#!/usr/bin/python3 -I
"""Measure both generated schemas without writing into the frozen candidate."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess

LIMIT = 16 * 1024 * 1024


def need(value, message):
    if not value:
        raise ValueError(message)


def regular(path, limit=LIMIT):
    path = Path(path)
    for item in (path, *path.parents):
        need(not item.is_symlink(), 'symbolic link in schema input/output')
    info = path.stat()
    need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_size <= limit,
         'schema input/output is linked, special or too large: ' + str(path))
    return path.read_bytes()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def environment():
    # No project, Agent or credential environment is forwarded to these reads.
    return {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'GIT_CONFIG_NOSYSTEM': '1',
            'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_OPTIONAL_LOCKS': '0',
            'GIT_TERMINAL_PROMPT': '0'}


def git(source, *args):
    return subprocess.check_output(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
        '-c', 'safe.directory=' + str(source), '-C', str(source), *args],
        env=environment(), stderr=subprocess.PIPE, timeout=30).decode().strip()


def binding(source, head, crate_tree):
    need(all(re.fullmatch('[0-9a-f]{40}', value) for value in (head, crate_tree)),
         'exact head and reviewed Core crate tree are required')
    need(git(source, 'rev-parse', 'HEAD') == head, 'frozen candidate head differs')
    need(git(source, 'rev-parse', 'HEAD:tools/harness-gate') == crate_tree,
         'Core build does not match the candidate crate tree')
    need(not git(source, 'status', '--porcelain', '--untracked-files=no'),
         'frozen candidate tracked bytes changed')
    return git(source, 'rev-parse', 'HEAD^{tree}')


def measure(source, output, core, core_sha256, head, crate_tree):
    source, output, core = map(Path, (source, output, core))
    need(all(path.is_absolute() and '..' not in path.parts for path in (source, output, core)),
         'schema adapter paths must be absolute and normalized')
    need(source != output and source not in output.parents and output not in source.parents,
         'schema output must be separate from the frozen source')
    for item in (source, *source.parents, output.parent, *output.parent.parents):
        need(not item.is_symlink(), 'symbolic link in schema input/output')
    need(re.fullmatch('[0-9a-f]{64}', core_sha256)
         and digest(regular(core, 128 * 1024 * 1024)) == core_sha256, 'Core differs from its reviewed pin')
    tree = binding(source, head, crate_tree)
    expected = {name: regular(source / 'schema' / (name + '.schema.json'))
                for name in ('flow', 'quality')}
    # Never consume stale generated files or reuse an interrupted measurement.
    output.mkdir(mode=0o700, exist_ok=False)
    report = {'schema': 'gatehouse-schema-measurement/v1', 'head': head, 'tree': tree,
              'core_crate_tree': crate_tree, 'core_sha256': core_sha256,
              'status': 'incomplete', 'schemas': [],
              'scope': 'Schema equality only; Core retains final quality authority'}
    def record():
        temporary = output / '.result.json.tmp'
        with temporary.open('x') as stream:
            json.dump(report, stream, sort_keys=True, indent=2)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, output / 'result.json')
    record()
    try:
        for name in ('flow', 'quality'):
            argv = [str(core), '--project-root', str(output), 'schema', 'export']
            if name == 'quality':
                argv.append('--quality')
            result = subprocess.run(argv, cwd=source, env=environment(),
                                    capture_output=True, timeout=30, check=False)
            need(len(result.stdout) + len(result.stderr) <= 1024 * 1024,
                 'schema command output exceeds budget')
            (output / (name + '.stdout')).write_bytes(result.stdout)
            (output / (name + '.stderr')).write_bytes(result.stderr)
            need(result.returncode == 0, 'Core schema export failed: ' + name)
            generated = regular(output / 'schema' / (name + '.schema.json'))
            matches = generated == expected[name]
            report['schemas'].append({'name': name, 'argv': argv, 'exit_code': result.returncode,
                'expected_sha256': digest(expected[name]), 'generated_sha256': digest(generated),
                'bytes': len(generated), 'matches': matches})
        need(binding(source, head, crate_tree) == tree
             and digest(regular(core, 128 * 1024 * 1024)) == core_sha256
             and all(regular(source / 'schema' / (name + '.schema.json')) == value
                     for name, value in expected.items()), 'schema inputs changed during measurement')
        report['status'] = 'matched' if all(row['matches'] for row in report['schemas']) else 'mismatch'
    except (OSError, ValueError, subprocess.SubprocessError):
        report['status'] = 'measurement_error'
        record()
        raise
    record()
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--core', type=Path, required=True)
    parser.add_argument('--core-sha256', required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--core-crate-tree', required=True)
    args = parser.parse_args()
    try:
        report = measure(args.source, args.output, args.core, args.core_sha256,
                         args.head, args.core_crate_tree)
        print(json.dumps(report, sort_keys=True))
        return 0 if report['status'] == 'matched' else 1
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({'status': 'measurement_error', 'reason': str(error)}))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
