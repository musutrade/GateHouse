"""Push only a locally accepted candidate and recover its one deterministic PR."""
from __future__ import annotations

import subprocess
import base64
import urllib.parse

from .common import require
from .execution import clean_environment


def deliver(control, publisher, run_id, title, body):
    require(isinstance(title, str) and 0 < len(title.encode()) <= 256
            and isinstance(body, str) and len(body.encode()) <= 16384, 'publication text exceeds budget')
    require(publisher.app_id != control.github.app_id, 'delivery identity must not own authoritative Check credentials')
    run = control.journal.get(run_id)
    control.evidence(run)
    require(run['remote'] not in ('merged', 'revoked', 'revoking'), 'batch is not publishable')
    branch = 'gatehouse/' + run_id
    inputs = run['input']
    base = control.github.repo('GET', '/git/ref/heads/' + control.config['base_branch'])
    require(base['object']['sha'] == inputs['base'], 'target baseline changed before push')
    # List refs instead of treating arbitrary HTTP 404/403/transport errors as absence.
    refs = publisher.repo('GET', '/git/matching-refs/heads/' + urllib.parse.quote(branch, safe=''))
    require(len(refs) <= 1, 'ambiguous deterministic publication branch')
    if refs:
        require(refs[0]['ref'] == 'refs/heads/' + branch and refs[0]['object']['sha'] == run['head'],
                'publication branch was changed externally')
    else:
        control.journal.event(run_id, 'push-intent', {'branch': branch, 'head': run['head']})
        env = clean_environment()
        env.update(GIT_CONFIG_COUNT='1', GIT_CONFIG_KEY_0='http.https://github.com/.extraheader',
                   GIT_CONFIG_VALUE_0='AUTHORIZATION: basic ' + base64.b64encode(
                       ('x-access-token:' + publisher.installation_token()).encode()).decode())
        result = subprocess.run(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
            '-c', 'protocol.ext.allow=never', 'push', '--porcelain',
            'https://github.com/' + inputs['repository'] + '.git',
            run['head'] + ':refs/heads/' + branch], cwd=control.executor.source,
            env=env, capture_output=True, timeout=120)
        require(result.returncode == 0, 'push failed or outcome unknown; recover the same candidate')
    control.journal.event(run_id, 'push-confirmed', {'branch': branch, 'head': run['head']})
    control.evidence(run)
    owner = inputs['repository'].split('/')[0]
    prs = publisher.repo('GET', '/pulls?state=all&head=' + urllib.parse.quote(owner + ':' + branch, safe='')
                         + '&base=' + control.config['base_branch'] + '&per_page=100')
    require(len(prs) <= 1, 'multiple PRs for one frozen operation')
    if prs:
        pr = prs[0]
        require(pr['head']['sha'] == run['head'] and pr['state'] == 'open'
                and not pr.get('merged_at'), 'recovered PR no longer matches the active candidate')
    else:
        control.journal.event(run_id, 'pr-intent', {'branch': branch})
        pr = publisher.repo('POST', '/pulls', {'head': branch, 'base': control.config['base_branch'],
                                             'title': title, 'body': body, 'draft': False})
    return control.attach(run_id, pr['number'])
