#!/usr/bin/env python3
"""Deliver an existing baseline candidate; never measure or accept a baseline.

Only the baseline-refresh workflow's main-branch candidates are supported.
GitHub API run/attempt metadata, the immutable artifact archive digest and Git
objects bind the original evidence. There is no new historical seal or manifest.
"""
from __future__ import annotations

import argparse
import base64
from datetime import datetime
import hashlib
import io
import json
import math
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen
import zipfile

WORKFLOW = '.github/workflows/quality-baseline-refresh.yml'
JOB = 'Capture reviewable baseline candidate'
CAPTURE = 'Capture candidate measurements'
UPLOAD = 'Upload raw benchmark evidence'
FILES = ('docs/benchmarks/phase-1/current.json', 'docs/benchmarks/phase-1/current.md')
MAX_ARCHIVE = 32 * 1024 * 1024
MAX_EXPANDED = 128 * 1024 * 1024
MAX_ENTRIES = 4096
MAX_ATTEMPTS = 20
LINK_PREFIX = '<!-- baseline-delivery-v1 '
LINK_KEYS = ('repository', 'source_run_id', 'source_run_attempt', 'measured_sha', 'artifact_id', 'artifact_digest')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def timestamp(value):
    require(isinstance(value, str), 'missing original timestamp')
    result = datetime.fromisoformat(value.replace('Z', '+00:00'))
    require(result.tzinfo is not None, 'missing timestamp timezone')
    return result


class GitHub:
    origin = 'https://api.github.com'

    def __init__(self, repository, token):
        require(re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository), 'invalid repository')
        self.repository = repository
        self.token = token

    def request(self, path, body=None, binary=False):
        # Redirected signed archive URLs receive no bearer token. urllib's
        # default redirect handler otherwise forwards Authorization headers.
        url = self.origin + '/repos/' + self.repository + path
        payload = None if body is None else json.dumps(body).encode()
        request = Request(url, data=payload, headers={
            'Authorization': 'Bearer ' + self.token,
            'Accept': 'application/vnd.github+json',
            'X-GitHub-Api-Version': '2022-11-28',
        })
        if binary:
            import urllib.request

            class NoAuthRedirect(urllib.request.HTTPRedirectHandler):
                def redirect_request(self, req, fp, code, msg, headers, newurl):
                    redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
                    if redirected is not None:
                        require(newurl.startswith('https://'), 'insecure archive redirect')
                        redirected.remove_header('Authorization')
                    return redirected

            opener = urllib.request.build_opener(NoAuthRedirect()).open
        else:
            opener = urlopen
        with opener(request, timeout=30) as response:
            limit = MAX_ARCHIVE if binary else 8 * 1024 * 1024
            data = response.read(limit + 1)
        require(len(data) <= limit, 'API response exceeds size limit')
        return data if binary else json.loads(data)

    def pages(self, path, key):
        result = []
        for page in range(1, 11):
            value = self.request(path + ('&' if '?' in path else '?') +
                                 urlencode({'per_page': 100, 'page': page}))
            rows = value if key is None else value[key]
            require(isinstance(rows, list), 'invalid API pagination')
            result.extend(rows)
            if len(rows) < 100:
                return result
        raise ValueError('API pagination exceeds bounded workflow scope')


def precheck(api):
    try:
        value = api.request('/actions/permissions/workflow')
        enabled = value['can_approve_pull_request_reviews']
        require(type(enabled) is bool, 'missing boolean PR permission setting')
        return {'mode': 'pr', 'permission': 'policy-enabled' if enabled else 'denied',
                'reason': ('The policy permits Actions PR creation; delivery is still checked.' if enabled else
                           'Actions PR creation is disabled. An enterprise restriction requires its administrator; no setting is changed.')}
    except (HTTPError, URLError, OSError, ValueError, KeyError, TypeError) as error:
        reason = f'HTTP {error.code}' if isinstance(error, HTTPError) else type(error).__name__
        return {'mode': 'pr', 'permission': 'unknown',
                'reason': f'Cannot reliably read Actions PR capability ({reason}); PR intent remains, not a precheck PASS.'}


def jobs(api, run_id, attempt):
    return api.pages(f'/actions/runs/{run_id}/attempts/{attempt}/jobs', 'jobs')


def producer_steps(rows, run_id):
    matches = [row for row in rows if row['name'] == JOB and row['run_id'] == run_id]
    require(len(matches) == 1, 'missing or ambiguous capture job')
    steps = matches[0]['steps']
    result = []
    for name in (CAPTURE, UPLOAD):
        found = [step for step in steps if step['name'] == name]
        require(len(found) == 1 and found[0]['conclusion'] == 'success',
                'original capture/upload step did not succeed: ' + name)
        result.append(found[0])
    return result


def in_upload_window(artifact, steps):
    capture, upload = steps
    created, updated = timestamp(artifact['created_at']), timestamp(artifact['updated_at'])
    return (timestamp(capture['completed_at']) <= created and
            timestamp(upload['started_at']) <= created <= updated <= timestamp(upload['completed_at']))


def artifact_for_attempt(api, run, attempt, artifact_id):
    rows = api.pages(f'/actions/runs/{run["id"]}/artifacts', 'artifacts')
    found = [row for row in rows if row['id'] == artifact_id]
    require(len(found) == 1, 'artifact does not belong uniquely to original run')
    artifact = found[0]
    prefix = f'quality-baseline-refresh-{run["id"]}'
    require(artifact['name'] in (prefix, f'{prefix}-{attempt}'), 'unexpected original artifact name')
    require(artifact['expired'] is False, 'original artifact expired')
    binding = artifact['workflow_run']
    require(binding['id'] == run['id'] and binding['head_sha'] == run['head_sha'] and
            binding['repository_id'] == run['repository']['id'] and
            binding['head_repository_id'] == run['repository']['id'], 'mixed artifact source identity')
    steps = producer_steps(jobs(api, run['id'], attempt), run['id'])
    require(in_upload_window(artifact, steps), 'artifact is not bound to requested attempt upload')
    if artifact['name'] == prefix:
        # Legacy names did not include the attempt. Authenticate their original
        # timestamps against every attempt rather than inventing an attempt tag.
        latest = api.request(f'/actions/runs/{run["id"]}')['run_attempt']
        require(type(latest) is int and attempt <= latest <= MAX_ATTEMPTS, 'cannot uniquely authenticate legacy attempt')
        windows = []
        for number in range(1, latest + 1):
            if number == attempt:
                windows.append(number)
                continue
            matches = [row for row in jobs(api, run['id'], number)
                       if row['name'] == JOB and row['run_id'] == run['id']]
            require(len(matches) == 1, 'cannot uniquely authenticate legacy attempt job')
            uploads = [step for step in matches[0]['steps'] if step['name'] == UPLOAD]
            require(len(uploads) == 1, 'cannot uniquely authenticate legacy upload step')
            upload = uploads[0]
            if upload['conclusion'] == 'skipped' and upload['started_at'] is None:
                continue
            require(upload['started_at'] and upload['completed_at'], 'cannot authenticate incomplete legacy upload window')
            if (timestamp(upload['started_at']) <= timestamp(artifact['created_at']) and
                    timestamp(artifact['updated_at']) <= timestamp(upload['completed_at'])):
                windows.append(number)
        require(windows == [attempt], 'ambiguous legacy artifact attempt')
    return artifact


def archive_files(data, digest):
    require(re.fullmatch(r'sha256:[0-9a-f]{64}', digest or ''), 'missing original artifact digest')
    require(sha(data) == digest.removeprefix('sha256:'), 'original artifact archive digest mismatch')
    require(len(data) <= MAX_ARCHIVE, 'archive exceeds compressed size limit')
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        entries = archive.infolist()
        require(len(entries) <= MAX_ENTRIES and sum(e.file_size for e in entries) <= MAX_EXPANDED,
                'archive exceeds entry/expanded size limits')
        names, result = set(), {}
        for entry in entries:
            name = entry.filename.rstrip('/') if entry.is_dir() else entry.filename
            path = PurePosixPath(name)
            require(entry.orig_filename == entry.filename and name and not path.is_absolute() and
                    not any(ord(c) < 32 for c in name) and '\\' not in name and ':' not in name and
                    not any(part in ('', '.', '..') for part in name.split('/')) and
                    path.as_posix() == name and name not in names, 'unsafe or duplicate archive path')
            names.add(name)
            mode = stat.S_IFMT(entry.external_attr >> 16)
            require(mode in (0, stat.S_IFDIR if entry.is_dir() else stat.S_IFREG) and
                    not entry.flag_bits & 1, 'symlink/special/encrypted archive entry')
            if not entry.is_dir():
                result[name] = archive.read(entry)
        require(all(not any(parent.as_posix() in result for parent in PurePosixPath(name).parents
                            if parent.as_posix() != '.') for name in result), 'archive file/directory collision')
        return result


def raw_evidence(candidate, files):
    def read(name):
        require(isinstance(name, str) and name.startswith('benchmark-runs/') and name in files,
                'missing or unsafe original raw reference: ' + str(name))
        return json.loads(files[name])

    def five(rows):
        require(isinstance(rows, list) and len(rows) == 5, 'original candidate must retain five samples')

    require(candidate['tool'] == 'quality-benchmarks' and candidate['harness_version'] == 1 and
            candidate['fixture_version'] == 1, 'unsupported original benchmark format')
    for key in ('rustc', 'cargo', 'target', 'series_key'):
        require(isinstance(candidate[key], str) and candidate[key], 'missing original tool/series identity')
    warm = candidate['tests_seconds']['warm_samples']
    five(warm)
    require(candidate['tests_seconds']['count'] == 5 and
            all(type(value) in (int, float) and math.isfinite(value) and value >= 0 for value in warm),
            'invalid retained warm sample value')
    missing_commands = []
    for mode in ('serial', 'parallel'):
        summary = candidate['verification'][mode]
        runs = summary['runs']
        five(runs)
        require(summary['count'] == 5 and summary['samples'] == [row['seconds'] for row in runs],
                'original sample summary mismatch')
        for number, run in enumerate(runs, 1):
            prefix = f'benchmark-runs/{mode}/sample-{number}/'
            require(run['sample'] == number and run['mode'] == mode and
                    run['report'] == prefix + 'test_result.json', 'original sample identity mismatch')
            report = read(run['report'])
            require(report['passed'] is True and len(report['steps']) == len(run['steps']) >= 4,
                    'original raw report incomplete or failed')
            for observed, expected in zip(report['steps'], run['steps']):
                require(all(observed[key] == expected[key] for key in ('label', 'duration_ms', 'passed')) and
                        expected['passed'] is True and expected['log'].startswith(prefix), 'raw step/summary mismatch')
                read_name = expected['log']
                original_log = PurePosixPath(observed['log'])
                parts = original_log.parts
                roots = [index for index in range(len(parts) - 1)
                         if parts[index:index + 2] == ('.harness-gate', 'reports')]
                relative = PurePosixPath(*parts[roots[-1] + 2:]) if roots else PurePosixPath('logs', original_log.name)
                require(read_name == prefix + relative.as_posix(), 'original log reference/summary mismatch')
                require(read_name in files, 'missing original step log')
            state = read(prefix + 'parallel-state.json')
            require(state['current'] == 0 and state['workers'] == 2 and
                    state['observed_peak'] == run['observed_peak'] and
                    1 <= state['observed_peak'] <= run['configured_limit'], 'raw concurrency/summary mismatch')
            command_name = prefix + 'command-result.json'
            if command_name in files:
                command = read(command_name)
                require(command['exit_code'] == 0 and command['seconds'] == run['seconds'], 'raw command/summary mismatch')
            else:
                missing_commands.append(command_name)
    runs = candidate['scope_matcher']['runs']
    five(runs)
    for number, run in enumerate(runs, 1):
        require(run['sample'] == number and run['equivalent'] is True and
                run['raw'] == f'benchmark-runs/scope/sample-{number}.json',
                'original scope sample identity mismatch')
        require(read(run['raw']) == {key: value for key, value in run.items() if key != 'raw'},
                'raw scope/summary mismatch')
    return {'raw_sha256': {name: sha(value) for name, value in sorted(files.items())},
            'original_absent_command_records': missing_commands,
            'warm_command_records': 'not present in this benchmark format; not authenticated',
            'manifest': 'no delivery manifest required or retroactively supplied'}


def candidate_files(api, candidate_sha, measured_sha):
    commit = api.request('/git/commits/' + candidate_sha)
    require(commit['sha'] == candidate_sha and [p['sha'] for p in commit['parents']] == [measured_sha],
            'candidate must have the original measured commit as its sole parent')
    comparison = api.request(f'/compare/{measured_sha}...{candidate_sha}')
    require(comparison['total_commits'] == 1 and comparison['status'] == 'ahead' and
            comparison['base_commit']['sha'] == measured_sha and
            {row['filename'] for row in comparison['files']} == set(FILES) and
            len(comparison['files']) == 2 and all(row['status'] == 'modified' for row in comparison['files']),
            'candidate changes are not exactly the two baseline files')
    result = {}
    for name in FILES:
        record = api.request('/contents/' + name + '?' + urlencode({'ref': candidate_sha}))
        require(record['type'] == 'file' and record['path'] == name and record['encoding'] == 'base64',
                'candidate baseline input is not a regular Git blob')
        content = base64.b64decode(record['content'].replace('\n', ''), validate=True)
        require(record['size'] == len(content) and
                record['sha'] == hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest(),
                'candidate Git blob identity mismatch')
        result[name] = content
    return result


def branch_sha(api, run_id):
    return api.request(f'/git/ref/heads/automation/quality-baseline-{run_id}')['object']['sha']


def verify_existing(api, run_id, attempt, candidate_sha, artifact_id, digest):
    require(type(run_id) is int and run_id > 0 and type(attempt) is int and 1 <= attempt <= MAX_ATTEMPTS and
            type(artifact_id) is int and artifact_id > 0 and re.fullmatch('[0-9a-f]{40}', candidate_sha or ''),
            'missing or invalid delivery-only identity pins')
    run = api.request(f'/actions/runs/{run_id}/attempts/{attempt}')
    repo = api.request('')
    require(run['id'] == run_id and run['run_attempt'] == attempt and run['path'] == WORKFLOW and
            run['event'] in ('schedule', 'workflow_dispatch') and run['head_branch'] == repo['default_branch'] and
            run['repository']['full_name'] == api.repository and run['repository']['id'] == repo['id'] and
            run['head_repository']['id'] == repo['id'] and re.fullmatch('[0-9a-f]{40}', run['head_sha']),
            'unexpected original repository/workflow/run/attempt source')
    require(branch_sha(api, run_id) == candidate_sha, 'candidate branch moved or missing')
    blobs = candidate_files(api, candidate_sha, run['head_sha'])
    candidate = json.loads(blobs[FILES[0]])
    require(candidate['commit'] == run['head_sha'], 'candidate measured commit differs from original run')
    artifact = artifact_for_attempt(api, run, attempt, artifact_id)
    require(artifact['digest'] == digest, 'original artifact digest pin differs from API')
    files = archive_files(api.request(f'/actions/artifacts/{artifact_id}/zip', binary=True), digest)
    evidence = raw_evidence(candidate, files)
    return {'mode': 'pr', 'baseline_accepted': False, 'repository': api.repository,
            'run_id': run_id, 'run_attempt': attempt, 'measured_sha': run['head_sha'],
            'candidate_sha': candidate_sha, 'candidate_parent': run['head_sha'],
            'branch': f'automation/quality-baseline-{run_id}', 'base': repo['default_branch'],
            'artifact_id': artifact_id, 'artifact_digest': digest,
            'candidate_files_sha256': {name: sha(value) for name, value in blobs.items()},
            'original_tool_identity': {key: candidate[key] for key in ('rustc', 'cargo', 'target', 'series_key')},
            **evidence}


def association(verified):
    return dict(zip(LINK_KEYS, (verified['repository'], verified['run_id'], verified['run_attempt'],
                               verified['measured_sha'], verified['artifact_id'], verified['artifact_digest'])))


def association_line(verified):
    return LINK_PREFIX + json.dumps(association(verified), separators=(',', ':'), sort_keys=True) + ' -->'


def check_association(body, verified):
    require(isinstance(body, str), 'missing PR evidence association')
    lines = [line for line in body.splitlines() if line.startswith(LINK_PREFIX)]
    require(len(lines) == 1 and lines[0].endswith(' -->'), 'missing or ambiguous PR evidence association')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, 'duplicate PR evidence association field')
            result[key] = value
        return result
    value = json.loads(lines[0][len(LINK_PREFIX):-4], object_pairs_hook=unique)
    expected = association(verified)
    require(isinstance(value, dict) and value.keys() == expected.keys() and
            all(type(value[key]) is type(expected[key]) and value[key] == expected[key] for key in expected),
            'PR evidence association conflicts with original run/attempt/artifact')


def deliver(api, verified):
    require(branch_sha(api, verified['run_id']) == verified['candidate_sha'], 'candidate branch changed before PR delivery')
    head = api.repository.split('/')[0] + ':' + verified['branch']
    rows = api.pages('/pulls?' + urlencode({'state': 'open', 'head': head, 'base': verified['base']}), None)
    require(len(rows) <= 1, 'ambiguous existing candidate PR')
    if rows:
        pr = rows[0]
    else:
        body = (f"Delivery-only retry of original baseline candidate `{verified['candidate_sha']}`.\n\n"
                f"Original run/attempt: {verified['run_id']}/{verified['run_attempt']}; measured commit `{verified['measured_sha']}`.\n"
                f"Original evidence: https://github.com/{api.repository}/actions/runs/{verified['run_id']}/artifacts/{verified['artifact_id']}\n"
                f"Archive digest: `{verified['artifact_digest']}`.\n\n"
                "No measurements were rerun and no baseline was adopted. Review original JSON, Markdown and raw evidence before merging.")
        body += '\n\n' + association_line(verified) + '\n'
        pr = api.request('/pulls', {'head': verified['branch'], 'base': verified['base'],
                                   'title': 'chore(quality): refresh phase one baseline', 'body': body})
    check_association(pr.get('body'), verified)
    require(pr['head']['sha'] == verified['candidate_sha'] and pr['head']['repo']['full_name'] == api.repository and
            pr['head']['ref'] == verified['branch'] and
            pr['base']['ref'] == verified['base'] and pr['base']['repo']['full_name'] == api.repository and
            branch_sha(api, verified['run_id']) == verified['candidate_sha'], 'candidate/PR identity changed during delivery')
    return {**verified, 'delivery': 'created' if not rows else 'existing', 'pull_request': pr['html_url']}


def summary(api, run_id, attempt, artifact_id=None, digest=None, pr_number=None):
    result = {'mode': 'pr', 'baseline_accepted': False, 'run_id': run_id, 'run_attempt': attempt,
              'artifact_id': artifact_id, 'artifact_digest': digest, 'delivery': 'unverified'}
    errors = []
    try:
        run = api.request(f'/actions/runs/{run_id}/attempts/{attempt}')
        result['measured_sha'] = run['head_sha']
    except (HTTPError, URLError, OSError, ValueError, KeyError, TypeError):
        errors.append('original run unavailable')
    try:
        candidate = branch_sha(api, run_id)
        result['candidate_sha'] = candidate
        result['candidate_parents'] = [row['sha'] for row in api.request('/git/commits/' + candidate)['parents']]
    except (HTTPError, URLError, OSError, ValueError, KeyError, TypeError):
        errors.append('candidate branch unavailable; no commit is recreated')
    if not artifact_id or not digest:
        errors.append('original artifact id/digest unavailable; raw diagnostics may still have been uploaded')
    if pr_number:
        try:
            pr = api.request(f'/pulls/{pr_number}')
            require(pr['head']['sha'] == result.get('candidate_sha') and
                    pr['head']['repo']['full_name'] == api.repository and
                    pr['head']['ref'] == f'automation/quality-baseline-{run_id}', 'observed PR candidate identity mismatch')
            check_association(pr.get('body'), {'repository': api.repository, 'run_id': run_id,
                              'run_attempt': attempt, 'measured_sha': result.get('measured_sha'),
                              'artifact_id': artifact_id, 'artifact_digest': digest})
            result['pull_request'] = pr['html_url']
        except (HTTPError, URLError, OSError, ValueError, KeyError, TypeError):
            errors.append('PR unavailable or does not match observed candidate')
    result['blockers'] = errors
    result['retry_operation'] = 'delivery-only'
    result['retry_inputs'] = {'operation': 'delivery-only', 'source_run_id': str(run_id),
                              'source_run_attempt': str(attempt), 'candidate_sha': result.get('candidate_sha'),
                              'artifact_id': str(artifact_id) if artifact_id else None, 'artifact_digest': digest}
    return result


def record(result, output=None):
    text = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if output:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text)
    print(text, end='')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as stream:
            stream.write('### Baseline PR delivery (candidate only)\n\n```json\n' + text + '```\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('precheck', 'verify-existing', 'deliver', 'summary'))
    parser.add_argument('--run-id', type=int)
    parser.add_argument('--attempt', type=int)
    parser.add_argument('--candidate-sha')
    parser.add_argument('--artifact-id', type=int)
    parser.add_argument('--artifact-digest')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = {'mode': 'pr', 'baseline_accepted': False}
    try:
        api = GitHub(os.environ['GITHUB_REPOSITORY'], os.environ['GH_TOKEN'])
        if args.operation == 'precheck':
            result.update(precheck(api))
            if os.environ.get('GITHUB_OUTPUT'):
                with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
                    stream.write('permission=' + result['permission'] + '\n')
            record(result, args.output)
            return 1 if result['permission'] == 'denied' else 0
        if args.operation == 'summary':
            number = os.environ.get('CAPTURE_PR_NUMBER', '')
            require(not number or number.isdecimal(), 'invalid observed PR number')
            result.update(summary(api, args.run_id, args.attempt, args.artifact_id, args.artifact_digest,
                                  int(number) if number else None))
            result['permission'] = os.environ.get('PR_PERMISSION_STATE') or 'unknown'
            record(result, args.output)
            return 1 if result['blockers'] else 0
        result.update({'run_id': args.run_id, 'run_attempt': args.attempt, 'candidate_sha': args.candidate_sha,
                       'artifact_id': args.artifact_id, 'artifact_digest': args.artifact_digest})
        capability = precheck(api)
        result.update(capability)
        require(capability['permission'] != 'denied', capability['reason'])
        verified = verify_existing(api, args.run_id, args.attempt, args.candidate_sha, args.artifact_id, args.artifact_digest)
        result.update(verified)
        if args.operation == 'deliver':
            result.update(deliver(api, verified))
        record(result, args.output)
        return 0
    except (HTTPError, URLError, OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile) as error:
        result['delivery'] = 'blocked'
        result['reason'] = f'HTTP {error.code}' if isinstance(error, HTTPError) else str(error)
        record(result, args.output)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
