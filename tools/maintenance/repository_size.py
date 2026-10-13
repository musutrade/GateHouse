#!/usr/bin/env python3
"""Check only changed Git blobs, including versions removed before the final commit."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
FILE_LIMIT = 512 * 1024
CHANGE_LIMIT = 5 * 1024 * 1024
POLICY = '.github/repository-size-exceptions.json'
REVIEW = '.github/repository-size-review.json'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def git(root, *args, input=None):
    return subprocess.run(['git', *args], cwd=root, input=input, check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def resolve_commit(root, ref):
    require(bool(ref) and not re.fullmatch('0+', ref),
            'missing incremental baseline or head (zero push base is not supported); '
            'provide a known base commit, never silently substitute HEAD^')
    return git(root, 'rev-parse', '--verify', '--end-of-options', ref + '^{commit}').decode().strip()


def tree(root, commit):
    result = {}
    for record in git(root, 'ls-tree', '-rz', commit).split(b'\0'):
        if not record:
            continue
        metadata, path = record.split(b'\t', 1)
        mode, kind, oid = metadata.split()
        if kind == b'blob':
            result[path.decode('utf-8', 'surrogateescape')] = (mode.decode(), oid.decode())
    return result


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f'duplicate JSON field: {key}')
        result[key] = value
    return result


def committed_json(root, snapshot, path, *, required=False):
    entry = snapshot.get(path)
    if entry is None:
        require(not required, f'missing committed policy: {path}')
        return None
    mode, oid = entry
    require(mode in ('100644', '100755'), f'policy must be a regular file: {path}')
    size = int(git(root, 'cat-file', '-s', oid))
    require(size <= FILE_LIMIT, f'policy metadata exceeds {FILE_LIMIT} bytes: {path}')
    return json.loads(git(root, 'cat-file', 'blob', oid).decode('utf-8'),
                      object_pairs_hook=unique_fields)


def exact_path(value):
    require(isinstance(value, str) and value and '\0' not in value
            and all(part not in ('', '.', '..') for part in value.split('/')),
            f'expected exact repository-relative path: {value!r}')
    return value


def text_field(value, label):
    require(isinstance(value, str) and bool(value.strip()), f'missing {label}')


def exceptions(policy):
    require(isinstance(policy, dict) and set(policy) == {'schema', 'exceptions'}
            and policy['schema'] == 'repository-size-exceptions/v1'
            and isinstance(policy['exceptions'], list), 'invalid repository size exceptions')
    result = {}
    for entry in policy['exceptions']:
        require(isinstance(entry, dict)
                and set(entry) == {'path', 'owner', 'reason', 'max_bytes'},
                'exception needs exactly path, owner, reason and max_bytes')
        path = exact_path(entry['path'])
        require(path not in result, f'duplicate exception: {path!r}')
        require(path not in (POLICY, REVIEW), 'policy metadata cannot exempt itself')
        text_field(entry['owner'], f'exception owner: {path!r}')
        text_field(entry['reason'], f'exception reason: {path!r}')
        require(type(entry['max_bytes']) is int and entry['max_bytes'] > FILE_LIMIT,
                f'exception max_bytes must be an integer above {FILE_LIMIT}: {path!r}')
        result[path] = entry
    return result


def changed_blobs(root, base, head):
    """Collect destination path/OID pairs; never use line-delimited Git paths."""
    baseline = tree(root, base)
    revisions = git(root, 'rev-list', '--reverse', '--parents', head, '^' + base).splitlines()
    # The endpoint diff also covers non-ancestor pushes. Per-commit diffs catch
    # an oversized blob added and then removed within one push/PR.
    comparisons = [(base, head)]
    for revision in revisions:
        commit, *parents = revision.decode('ascii').split()
        comparisons.append((parents[0] if parents else None, commit))
    pairs = set()
    for old, new in comparisons:
        args = ['diff-tree', '--no-commit-id', '--raw', '-r', '-z', '--no-renames',
                '--no-abbrev', '--no-ext-diff']
        args += [old, new] if old else ['--root', new]
        fields = git(root, *args, '--').split(b'\0')
        for index in range(0, len(fields) - 1, 2):
            metadata, encoded_path = fields[index:index + 2]
            old_mode, new_mode, old_oid, new_oid, status = metadata.split()
            if new_mode not in (b'100644', b'100755', b'120000') or old_oid == new_oid:
                continue
            path = encoded_path.decode('utf-8', 'surrogateescape')
            oid = new_oid.decode('ascii')
            if path in baseline and baseline[path][1] == oid:
                continue
            pairs.add((path, oid))
    if not pairs:
        return []
    oids = sorted({oid for _, oid in pairs})
    measured = git(root, 'cat-file', '--batch-check=%(objectname) %(objecttype) %(objectsize)',
                   input=('\n'.join(oids) + '\n').encode('ascii'))
    sizes = {}
    for line in measured.splitlines():
        oid, kind, size = line.decode('ascii').split()
        require(kind == 'blob', f'expected changed blob: {oid}')
        sizes[oid] = int(size)
    return [{'path': path, 'oid': oid, 'bytes': sizes[oid]} for path, oid in sorted(pairs)]


def review_inventory(blobs):
    # The receipt cannot list its own OID. Its versions still count toward the
    # total and each must remain <=512 KiB without an exception.
    return [entry for entry in blobs if entry['path'] != REVIEW]


def check_review(review, base, blobs):
    require(isinstance(review, dict)
            and set(review) == {'schema', 'base', 'owner', 'reason', 'blobs'}
            and review['schema'] == 'repository-size-review/v1',
            f'changes above {CHANGE_LIMIT} bytes need a committed {REVIEW}')
    require(review['base'] == base, 'bulk review base does not match this change')
    text_field(review['owner'], 'bulk review owner')
    text_field(review['reason'], 'bulk review reason')
    entries = review['blobs']
    require(isinstance(entries, list), 'bulk review blobs must be an exact inventory')
    for entry in entries:
        require(isinstance(entry, dict) and set(entry) == {'path', 'oid', 'bytes'},
                'bulk review entries need exactly path, oid and bytes')
        exact_path(entry['path'])
        require(isinstance(entry['oid'], str)
                and re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', entry['oid']),
                'bulk review entry needs a full Git blob OID')
        require(type(entry['bytes']) is int and entry['bytes'] >= 0,
                'bulk review entry needs exact byte size')
    identities = {(entry['path'], entry['oid']) for entry in entries}
    require(len(identities) == len(entries), 'duplicate bulk review entry')
    recorded = {(entry['path'], entry['oid'], entry['bytes']) for entry in entries}
    required = {(entry['path'], entry['oid'], entry['bytes'])
                for entry in review_inventory(blobs)}
    # A squash merge drops intermediate versions present in the reviewed PR.
    # Retaining those exact entries is safe; omitting a changed blob is not.
    require(required <= recorded,
            'bulk review inventory is incomplete; copy review_template from the report')


def evaluate(root, base, head):
    base, head = resolve_commit(root, base), resolve_commit(root, head)
    snapshot = tree(root, head)
    allowed = exceptions(committed_json(root, snapshot, POLICY, required=True))
    blobs = changed_blobs(root, base, head)
    errors = []
    for entry in blobs:
        if entry['bytes'] <= FILE_LIMIT:
            continue
        exemption = allowed.get(entry['path'])
        if exemption is None or entry['bytes'] > exemption['max_bytes']:
            errors.append(f"{entry['path']!r} ({entry['oid']}): {entry['bytes']} bytes; "
                          'needs an exact path exception with a sufficient max_bytes')
    total = sum(entry['bytes'] for entry in blobs)
    if total > CHANGE_LIMIT:
        try:
            review = committed_json(root, snapshot, REVIEW)
            check_review(review, base, blobs)
        except (ValueError, UnicodeError) as error:
            errors.append(str(error))
    result = {'schema': 'repository-size-report/v1', 'base': base, 'head': head,
              'file_limit_bytes': FILE_LIMIT, 'change_limit_bytes': CHANGE_LIMIT,
              'changed_blob_bytes': total, 'blobs': blobs, 'errors': errors}
    if total > CHANGE_LIMIT:
        result['review_template'] = {
            'schema': 'repository-size-review/v1', 'base': base,
            'owner': '', 'reason': '', 'blobs': review_inventory(blobs)}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-sha', required=True)
    parser.add_argument('--head-sha', default='HEAD')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    try:
        result = evaluate(ROOT, args.base_sha, args.head_sha)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'repository size check failed: {error}', file=sys.stderr)
        return 1
    encoded = json.dumps(result, ensure_ascii=True, indent=2) + '\n'
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding='utf-8')
    print(encoded, end='')
    return 1 if result['errors'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
