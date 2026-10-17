"""Dedicated App authentication and bounded Checks API transport, without gh/PAT fallback."""
from __future__ import annotations

import base64
import json
import http.client
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

from .common import Blocked, canonical, checked, protected, require


def b64(value):
    return base64.urlsafe_b64encode(value).rstrip(b'=')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise Blocked('GitHub API redirect rejected')


class APIError(RuntimeError):
    """An unknown external outcome; keep the same operation for recovery."""


class GitHub:
    def __init__(self, app_id, installation_id, key, repository, permissions=None):
        require(type(app_id) is int and app_id > 0 and type(installation_id) is int
                and installation_id > 0, 'App and installation IDs are required')
        self.app_id, self.installation_id = app_id, installation_id
        self.key = protected(key, private=True)
        self.repository = repository
        self.permissions = permissions or {'checks': 'write', 'contents': 'read', 'pull_requests': 'read'}
        self.token = None
        self.expires = 0
        self.opener = urllib.request.build_opener(NoRedirect(), urllib.request.ProxyHandler({}))

    def request(self, method, path, body=None, *, token=None):
        require(path.startswith('/') and not path.startswith('//') and '..' not in path,
                'invalid GitHub API path')
        if token is None:
            token = self.installation_token()
        request = urllib.request.Request('https://api.github.com' + path,
            data=canonical(body) if body is not None else None, method=method,
            headers={'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
                     'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'GateHouse-trusted-host',
                     'Content-Type': 'application/json'})
        try:
            with self.opener.open(request, timeout=30) as response:
                data = response.read(4 * 1024 * 1024 + 1)
                require(len(data) <= 4 * 1024 * 1024, 'GitHub response exceeds limit')
                return json.loads(data)
        except urllib.error.HTTPError as error:
            # Do not print body, headers, credentials or user-controlled exception context.
            raise APIError(f'GitHub API {method} failed with HTTP {error.code}') from None
        except (urllib.error.URLError, OSError, http.client.HTTPException):
            raise APIError('GitHub API transport failure; recover the same operation') from None

    def installation_token(self):
        if self.token and time.time() < self.expires:
            return self.token
        now = int(time.time())
        header = b64(canonical({'alg': 'RS256', 'typ': 'JWT'}))
        claims = b64(canonical({'iat': now - 60, 'exp': now + 540, 'iss': str(self.app_id)}))
        message = header + b'.' + claims
        signature = subprocess.run(['/usr/bin/openssl', 'dgst', '-sha256', '-sign', str(self.key)],
            input=message, capture_output=True, check=True, timeout=10).stdout
        jwt = (message + b'.' + b64(signature)).decode()
        installation = self.request('GET', f'/app/installations/{self.installation_id}', token=jwt)
        require(installation['app_id'] == self.app_id, 'installation belongs to another App')
        response = self.request('POST', f'/app/installations/{self.installation_id}/access_tokens',
            {'repositories': [self.repository.split('/')[1]],
             'permissions': self.permissions}, token=jwt)
        actual = {key: value for key, value in response.get('permissions', {}).items() if key != 'metadata'}
        require(actual == self.permissions, 'unexpected token permissions')
        self.token = response['token']
        self.expires = now + 3000
        return self.token

    def repo(self, method, path, body=None):
        return self.request(method, '/repos/' + self.repository + path, body)

    def checks(self, head):
        checks = []
        for page in range(1, 12):
            batch = self.repo('GET', f'/commits/{head}/check-runs?filter=all&per_page=100&page={page}')
            checks.extend(batch['check_runs'])
            if len(batch['check_runs']) < 100:
                return checks
        raise Blocked('too many Check Runs; refuse ambiguous recovery')

    @checked
    def remote(self, run, config):
        inputs = run['input']
        repository = self.repo('GET', '')
        require(repository['id'] == inputs['repository_id'] == config['repository_id'], 'repository identity changed')
        head = self.repo('GET', '/git/commits/' + inputs['head'])
        require(head['sha'] == inputs['head'] and head['tree']['sha'] == inputs['tree'],
                'remote commit/tree differs from verified input')
        pr = self.repo('GET', '/pulls/' + str(run['pr']))
        require(pr['state'] == 'open' and not pr.get('draft') and not pr.get('merged'), 'PR is not open and ready')
        require(pr['head']['repo']['full_name'] == inputs['repository']
                and pr['base']['repo']['full_name'] == inputs['repository']
                and pr['head']['sha'] == inputs['head']
                and pr['base']['ref'] == config['base_branch']
                and pr['base']['sha'] == inputs['base'], 'PR input/baseline no longer matches')
        branch = self.repo('GET', '/git/ref/heads/' + urllib.parse.quote(config['base_branch'], safe=''))
        require(branch['object']['sha'] == inputs['base'], 'target branch advanced')
        rules = self.repo('GET', '/rules/branches/' + urllib.parse.quote(config['base_branch'], safe=''))
        required = [item['parameters'] for item in rules if item['type'] == 'required_status_checks']
        require(any(item.get('strict_required_status_checks_policy') is True and
                    {'context': config['check_name'], 'integration_id': self.app_id} in
                    item.get('required_status_checks', []) for item in required),
                'required Check must use strict baseline protection and the dedicated App source')
        return pr

    @checked
    def find_check(self, run, name):
        own = [c for c in self.checks(run['head']) if c['name'] == name
               and c['app']['id'] == self.app_id]
        require(len(own) <= 1, 'multiple authoritative checks on one SHA; operator reconciliation required')
        if own:
            check = own[0]
            require(check['head_sha'] == run['head'], 'Check Run head mismatch')
            require(check.get('external_id') == run['id'] or
                    (run.get('previous_id') is not None and check.get('external_id') == run['previous_id']
                     and check['id'] == run['check_id']), 'Check belongs to an unrecognized gate run')
            if run['check_id'] is not None:
                require(check['id'] == run['check_id'], 'Check Run identity changed')
            return check
        require(run['check_id'] is None, 'persisted Check Run is missing remotely')
        return None

    @checked
    def update_check(self, run, name, status, conclusion=None, summary=''):
        require(status in ('in_progress', 'completed'), 'invalid Check status')
        require(conclusion in (None, 'success', 'failure'), 'neutral/skipped/stale are never delivery results')
        require((status == 'completed') == (conclusion is not None), 'incomplete Check result')
        old = self.find_check(run, name)
        body = {'name': name, 'external_id': run['id'], 'status': status,
                'output': {'title': 'GateHouse trusted local verification', 'summary': summary}}
        if (run.get('evidence') or {}).get('url'):
            body['details_url'] = run['evidence']['url']
        if conclusion:
            body.update(conclusion=conclusion, completed_at=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()))
        if old:
            result = self.repo('PATCH', '/check-runs/' + str(old['id']), body)
        else:
            # Never create green directly: recovery first has a durable blocking check.
            require(status == 'in_progress', 'must establish a blocking Check before success')
            result = self.repo('POST', '/check-runs', {**body, 'head_sha': run['head']})
        require(result['app']['id'] == self.app_id and result['head_sha'] == run['head']
                and result['external_id'] == run['id'] and result['status'] == status
                and result.get('conclusion') == conclusion, 'GitHub did not accept the expected Check state')
        return result['id']
