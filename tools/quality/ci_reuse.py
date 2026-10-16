#!/usr/bin/env python3
"""Optional push check inheritance; receipts are bindings, never self-attested PASS.

Run with python -I. The PR record/upload must precede repository script execution.
Only the three declared jobs may inherit conclusions; original artifacts keep their
original commit/run identity. Every unavailable proof means ordinary full checking.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time
from urllib.parse import urlencode
from urllib.request import Request, build_opener, HTTPRedirectHandler
import zipfile

SCHEMA = 'ci-pr-equivalence/v1'
PROOF_SCHEMA = 'ci-push-inheritance/v1'
WORKFLOW = '.github/workflows/ci.yml'
PUBLISHER = 'tools/quality/ci_reuse.py'
PUBLISHERS = (WORKFLOW, PUBLISHER)
REUSABLE = ('test', 'test-cross-platform', 'quality-coverage')
REQUIRED_JOBS = ('Classify change', 'Test', 'Test (macos-latest)', 'Test (windows-latest)',
                 'Security Audit', 'Format', 'Clippy', 'Build',
                 'Quality Coverage and Critical Paths', 'Quality CLI Contracts',
                 'Documentation Consistency', 'Release Governance Contracts',
                 'Quality Script Tests', 'Required Quality Aggregate')
RECORD_STEP = 'Record PR equivalence receipt'
UPLOAD_STEP = 'Upload PR equivalence receipt'
CLASSIFY_STEP = 'Classify changed paths'
INPUT_PATHS = ('.github', '.config', 'tools/quality', 'tools/harness-gate/Cargo.toml',
               'tools/harness-gate/Cargo.lock', 'test-data/manifest.json', 'schema')
MAX_ARCHIVE = 1024 * 1024
MAX_RECEIPT = 32 * 1024
MAX_API = 4 * 1024 * 1024
MAX_AGE = timedelta(hours=24)
RECEIPT_KEYS = {'schema', 'eligible', 'repository', 'repository_id', 'pull_request',
                'base_sha', 'head_sha', 'tested_sha', 'tested_tree', 'workflow',
                'workflow_blob', 'publisher_blobs', 'inputs_sha256', 'run_id', 'run_attempt'}


class Unusable(ValueError):
    pass


def require(value, reason):
    if not value:
        raise Unusable(reason)


def integer(value):
    require(type(value) is int and 0 < value < 2**63, 'invalid numeric identity')
    return value


def commit(value):
    require(isinstance(value, str) and re.fullmatch('[0-9a-f]{40}', value)
            and value != '0' * 40, 'invalid Git identity')
    return value


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def decode(data):
    return json.loads(data, object_pairs_hook=unique)


def timestamp(value):
    require(isinstance(value, str), 'missing timestamp')
    result = datetime.fromisoformat(value.replace('Z', '+00:00'))
    require(result.tzinfo is not None, 'timestamp lacks timezone')
    return result


def git(root, *args):
    return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.DEVNULL)


def object_id(root, expression):
    return commit(git(root, 'rev-parse', '--verify', expression).decode().strip())


def parents(root, sha):
    return git(root, 'show', '-s', '--format=%P', commit(sha)).decode().split()


def publishers(root, sha):
    return {path: object_id(root, f'{sha}:{path}') for path in PUBLISHERS}


def stable_publishers(root, base, sha):
    current = publishers(root, sha)
    require(current == publishers(root, base), 'publisher changed since tested base')
    return current


def inputs(root, sha):
    return digest(git(root, 'ls-tree', '-r', '-z', '--full-tree', sha, '--', *INPUT_PATHS))


def repository(event, expected):
    require(isinstance(expected, str) and re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', expected),
            'invalid repository name')
    repo = event['repository']
    require(repo['full_name'] == expected and repo['default_branch'] == 'main', 'repository mismatch')
    return integer(repo['id'])


def record(root, event, expected, sha, run_id, attempt):
    """Read platform/Git bindings before any untrusted repository script runs."""
    repo_id = repository(event, expected)
    pr = event['pull_request']
    require(pr['head']['repo']['id'] == repo_id == pr['base']['repo']['id'], 'fork PR unsupported')
    require(pr['base']['ref'] == 'main', 'unsupported PR target')
    base, head, sha = commit(pr['base']['sha']), commit(pr['head']['sha']), commit(sha)
    require(object_id(root, 'HEAD') == sha and parents(root, sha) == [base, head],
            'checkout is not the recorded effective PR merge')
    trusted = stable_publishers(root, base, sha)
    for path in PUBLISHERS:
        require((root / path).read_bytes() == git(root, 'show', f'{sha}:{path}'), 'publisher worktree modified')
    return {'schema': SCHEMA, 'eligible': True, 'repository': expected, 'repository_id': repo_id,
            'pull_request': integer(pr['number']), 'base_sha': base, 'head_sha': head,
            'tested_sha': sha, 'tested_tree': object_id(root, f'{sha}^{{tree}}'),
            'workflow': WORKFLOW, 'workflow_blob': trusted[WORKFLOW],
            'publisher_blobs': trusted, 'inputs_sha256': inputs(root, sha),
            'run_id': integer(run_id), 'run_attempt': integer(attempt)}


class NoAuthRedirect(HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, newurl):
        require(newurl.startswith('https://'), 'insecure API redirect')
        redirected = super().redirect_request(request, response, code, message, headers, newurl)
        if redirected is not None:
            redirected.remove_header('Authorization')
        return redirected


class GitHub:
    """Bounded read-only API client; never send bearer credentials to archive hosts."""
    def __init__(self, repository_name, token):
        require(re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository_name), 'invalid repository')
        require(bool(token), 'missing read token')
        self.repository, self.token = repository_name, token
        self.deadline = time.monotonic() + 45

    def get(self, path, binary=False):
        remaining = self.deadline - time.monotonic()
        require(remaining > 0, 'optional API proof exceeded time budget')
        request = Request('https://api.github.com/repos/' + self.repository + path,
                          headers={'Authorization': 'Bearer ' + self.token,
                                   'Accept': 'application/vnd.github+json',
                                   'X-GitHub-Api-Version': '2022-11-28'}, method='GET')
        limit = MAX_ARCHIVE if binary else MAX_API
        with build_opener(NoAuthRedirect()).open(request, timeout=min(10, remaining)) as response:
            data = response.read(limit + 1)
        require(len(data) <= limit, 'API response too large')
        return data if binary else decode(data)

    def pages(self, path, key=None):
        rows = []
        for page in range(1, 4):
            value = self.get(path + ('&' if '?' in path else '?') + urlencode({'per_page': 100, 'page': page}))
            batch = value if key is None else value[key]
            require(isinstance(batch, list), 'invalid pagination')
            rows.extend(batch)
            if len(batch) < 100:
                return rows
        raise Unusable('ambiguous pagination exceeds bound')


def verify_run(run, repo_id, head, workflow_id):
    integer(run['id']); integer(run['run_attempt'])
    require(run['repository']['id'] == repo_id == run['head_repository']['id'], 'run repository mismatch')
    require(run['head_sha'] == head and run['event'] == 'pull_request'
            and run['path'] == WORKFLOW and run['workflow_id'] == workflow_id, 'run input mismatch')
    require(run['status'] == 'completed' and run['conclusion'] == 'success', 'PR run did not complete successfully')


def verified_jobs(api, run, head):
    rows = api.pages(f'/actions/runs/{run["id"]}/attempts/{run["run_attempt"]}/jobs', 'jobs')
    selected = {}
    for name in REQUIRED_JOBS:
        matches = [row for row in rows if row['name'] == name]
        require(len(matches) == 1, 'required job missing or ambiguous: ' + name)
        job = matches[0]
        integer(job['id'])
        require(job['run_id'] == run['id'] and job['head_sha'] == head and
                job['status'] == 'completed' and job['conclusion'] == 'success',
                'required job not successful: ' + name)
        selected[name] = job
    steps = selected['Classify change']['steps']
    producer = []
    for name in (RECORD_STEP, UPLOAD_STEP, CLASSIFY_STEP):
        matches = [step for step in steps if step['name'] == name]
        require(len(matches) == 1 and matches[0]['conclusion'] == 'success'
                and matches[0]['status'] == 'completed', 'receipt publisher step missing or failed')
        producer.append(matches[0])
    require(producer[1]['number'] == producer[0]['number'] + 1
            and producer[1]['number'] < producer[2]['number'], 'receipt was not sealed before repository scripts')
    return selected, producer


def receipt_archive(data, artifact):
    require('sha256:' + digest(data) == artifact['digest'], 'artifact digest mismatch')
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        files = archive.infolist()
        require(len(files) == 1, 'receipt archive must contain one file')
        item = files[0]
        kind = stat.S_IFMT(item.external_attr >> 16)
        require(item.filename == 'receipt.json' and kind in (0, stat.S_IFREG)
                and not item.is_dir() and 0 < item.file_size <= MAX_RECEIPT, 'invalid receipt ZIP member')
        return decode(archive.read(item))


def verify_receipt(receipt, expected):
    require(set(receipt) == RECEIPT_KEYS and receipt['schema'] == SCHEMA
            and receipt['eligible'] is True, 'receipt schema or eligibility mismatch')
    commit(receipt['tested_sha'])
    for key, value in expected.items():
        require(receipt[key] == value and type(receipt[key]) is type(value), 'receipt binding mismatch: ' + key)


def probe(root, event, expected, sha, api, now):
    repo_id = repository(event, expected)
    require(event['ref'] == 'refs/heads/main' and event['forced'] is False
            and event['deleted'] is False, 'not an ordinary main push')
    before, sha = commit(event['before']), commit(sha)
    require(event['after'] == sha and object_id(root, 'HEAD') == sha, 'push checkout mismatch')
    lineage = parents(root, sha)
    require(len(lineage) == 2 and lineage[0] == before, 'push is not one two-parent merge')
    head = commit(lineage[1])
    trusted = stable_publishers(root, before, sha)
    prs = api.pages(f'/commits/{sha}/pulls')
    require(len(prs) == 1, 'associated PR missing or ambiguous')
    pr = api.get('/pulls/' + str(integer(prs[0]['number'])))
    require(pr['merged'] is True and pr['state'] == 'closed' and pr['merge_commit_sha'] == sha
            and pr['head']['sha'] == head and pr['base']['ref'] == 'main'
            and pr['head']['repo']['id'] == repo_id == pr['base']['repo']['id'], 'merged PR binding mismatch')
    require(timestamp(pr['merged_at']) <= now, 'PR merge timestamp is in future')
    workflow = api.get('/actions/workflows/ci.yml')
    require(workflow['path'] == WORKFLOW and workflow['state'] == 'active', 'unexpected workflow')
    workflow_id = integer(workflow['id'])
    runs = api.pages(f'/actions/workflows/{workflow_id}/runs?' + urlencode({'head_sha': head, 'event': 'pull_request'}), 'workflow_runs')
    require(len(runs) == 1, 'PR run missing or ambiguous')
    run_id = integer(runs[0]['id'])
    run = api.get(f'/actions/runs/{run_id}')
    verify_run(run, repo_id, head, workflow_id)
    require(run['id'] == run_id and run['run_attempt'] == runs[0]['run_attempt'], 'latest run identity changed')
    created = timestamp(run['created_at'])
    require(created <= now <= created + MAX_AGE, 'PR run proof is too old or future')
    attempt = integer(run['run_attempt'])
    historical = api.get(f'/actions/runs/{run_id}/attempts/{attempt}')
    verify_run(historical, repo_id, head, workflow_id)
    require(historical['id'] == run_id and historical['run_attempt'] == attempt, 'run attempt mismatch')
    jobs, steps = verified_jobs(api, run, head)
    artifacts = api.pages(f'/actions/runs/{run_id}/artifacts', 'artifacts')
    matches = [row for row in artifacts if row['name'] == f'ci-equivalence-{run_id}-{attempt}']
    require(len(matches) == 1, 'receipt artifact missing or ambiguous')
    artifact = matches[0]
    artifact_id = integer(artifact['id'])
    require(artifact['expired'] is False and timestamp(artifact['expires_at']) > now
            and type(artifact['size_in_bytes']) is int and 0 < artifact['size_in_bytes'] <= MAX_ARCHIVE,
            'receipt artifact expired or oversized')
    binding = artifact['workflow_run']
    require(binding['id'] == run_id and binding['head_sha'] == head and
            binding['repository_id'] == repo_id == binding['head_repository_id'], 'artifact belongs to another run')
    created, updated = timestamp(artifact['created_at']), timestamp(artifact['updated_at'])
    require(timestamp(steps[0]['completed_at']) <= timestamp(steps[1]['started_at']) <= created
            <= updated <= timestamp(steps[1]['completed_at']), 'artifact outside original attempt upload window')
    receipt = receipt_archive(api.get(f'/actions/artifacts/{artifact_id}/zip', binary=True), artifact)
    expected_receipt = {'repository': expected, 'repository_id': repo_id, 'pull_request': integer(pr['number']),
                        'base_sha': before, 'head_sha': head, 'tested_tree': object_id(root, f'{sha}^{{tree}}'),
                        'workflow': WORKFLOW, 'workflow_blob': trusted[WORKFLOW], 'publisher_blobs': trusted,
                        'inputs_sha256': inputs(root, sha), 'run_id': run_id, 'run_attempt': attempt}
    verify_receipt(receipt, expected_receipt)
    # Recheck live state after archive reads: reruns or deletion are not stale success.
    latest = api.get(f'/actions/runs/{run_id}')
    verify_run(latest, repo_id, head, workflow_id)
    require(latest['run_attempt'] == attempt and latest['id'] == run_id, 'run reran during verification')
    return {'schema': PROOF_SCHEMA, 'reuse_verified': True, 'reused_jobs': list(REUSABLE),
            'reason': 'verified equivalent PR conclusions', 'push_sha': sha, 'base_sha': before,
            'tree': receipt['tested_tree'], 'provenance': {'repository': expected, 'pull_request': pr['number'],
            'run_id': run_id, 'run_attempt': attempt, 'tested_sha': receipt['tested_sha'],
            'artifact_id': artifact_id, 'artifact_digest': artifact['digest'],
            'run_url': f'https://github.com/{expected}/actions/runs/{run_id}',
            'required_jobs': {name: job['id'] for name, job in jobs.items()}},
            'evidence_identity': 'Original PR artifacts retain their original commit/run identity; no fresh push evidence is asserted.'}


def unavailable(error):
    reason = str(error) if isinstance(error, Unusable) else type(error).__name__
    return {'schema': PROOF_SCHEMA, 'reuse_verified': False, 'reused_jobs': [], 'reason': reason}


def safe_probe(*args, **kwargs):
    try:
        return probe(*args, **kwargs)
    except Exception as error:
        return unavailable(error)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_text(json.dumps(value, sort_keys=True, indent=2) + '\n')
    temporary.replace(path)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('record', 'probe'))
    parser.add_argument('--repository', required=True)
    parser.add_argument('--event', type=Path, required=True)
    parser.add_argument('--event-name', default=os.environ.get('GITHUB_EVENT_NAME', ''))
    parser.add_argument('--sha', required=True)
    parser.add_argument('--run-id', type=int, default=int(os.environ.get('GITHUB_RUN_ID', '0')))
    parser.add_argument('--attempt', type=int, default=int(os.environ.get('GITHUB_RUN_ATTEMPT', '0')))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--github-output', type=Path)
    args = parser.parse_args(argv)
    try:
        require(sys.flags.isolated, 'invoke with python -I')
        root = Path.cwd()
        require(Path(__file__).resolve() == (root / PUBLISHER).resolve(), 'publisher location mismatch')
        require(args.event.stat().st_size <= MAX_API, 'event payload too large')
        event = decode(args.event.read_bytes())
        if args.operation == 'record':
            require(args.event_name == 'pull_request', 'receipt only records PR events')
            result = record(root, event, args.repository, args.sha, args.run_id, args.attempt)
        else:
            require(args.event_name == 'push', 'PRs and other events cannot skip')
            api = GitHub(args.repository, os.environ.get('GH_TOKEN') or os.environ.get('GITHUB_TOKEN'))
            result = safe_probe(root, event, args.repository, args.sha, api, datetime.now(timezone.utc))
    except Exception as error:
        result = unavailable(error)
    write_json(args.output, result)
    if args.github_output:
        with args.github_output.open('a') as output:
            output.write('reuse_verified=' + ('true' if result.get('reuse_verified') is True else 'false') + '\n')
    print(json.dumps({key: result[key] for key in ('schema', 'reason', 'provenance', 'reused_jobs', 'evidence_identity') if key in result} | {'reuse_verified': result.get('reuse_verified', False)}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
