"""Recoverable exact-input Check delivery. No caller may supply a PASS verdict."""
from __future__ import annotations

import time

from .common import Blocked, canonical, digest, require


class Control:
    def __init__(self, journal, github, executor, config, clock=time.time):
        self.journal, self.github, self.executor = journal, github, executor
        self.config, self.clock = config, clock

    def current(self, run):
        latest = self.journal.latest(run['repository'], run['head'])
        require(latest['id'] == run['id'], 'superseded gate run cannot publish')
        require(run['input']['policy'] == self.executor.policy_identity(), 'trusted policy/tool inputs changed')

    def validate(self, inputs):
        """Called only by the protected local ingress, before publishing a candidate."""
        require(inputs['repository'] == self.config['repository'], 'repository not configured')
        require(inputs['policy'] == self.executor.policy_identity(), 'policy identity mismatch')
        self.executor.validate_inputs(inputs)
        run = self.journal.create(inputs)
        self.current(run)
        if run['local'] == 'passed':
            self.evidence(run)
            return run
        require(run['local'] == 'pending', 'a failed/interrupted run requires explicit revocation and a new epoch')
        # The state is persisted before execution. A crash never implies successful completion.
        self.journal.update(run['id'], local='running')
        try:
            evidence = self.executor.execute(run)
            self.executor.confirm_stopped(run)
            self.executor.verify_evidence(run, evidence)
            self.current(run)
            require(self.clock() <= evidence['valid_until'], 'verification already expired')
            return self.journal.update(run['id'], local='passed', evidence=evidence)
        except BaseException:
            self.journal.update(run['id'], local='failed', reason='local verification failed or interrupted')
            raise

    def evidence(self, run):
        self.current(run)
        require(run['local'] == 'passed' and run['evidence'], 'no complete trusted local result')
        require(self.clock() <= run['evidence']['valid_until'], 'local evidence expired')
        self.executor.validate_inputs(run['input'])
        self.executor.verify_evidence(run, run['evidence'])

    def applicable(self, run):
        try:
            self.evidence(run)
            self.github.remote(run, self.config)
        except Blocked:
            self.revoke(run['id'], 'input, evidence, policy or remote binding is no longer applicable')
            raise

    def attach(self, run_id, pr):
        """Attach an already published exact candidate; remote existence is independently checked."""
        require(type(pr) is int and pr > 0, 'invalid PR number')
        run = self.journal.get(run_id)
        self.evidence(run)
        require(run['remote'] not in ('merged', 'revoked'), 'terminal batch cannot be published')
        require(run['pr'] in (None, pr), 'cannot retarget a gate run to another PR')
        candidate = {**run, 'pr': pr}
        self.github.remote(candidate, self.config)
        return self.journal.update(run_id, pr=pr, remote='attached')

    def publish(self, run_id):
        run = self.journal.get(run_id)
        require(run['pr'] is not None and run['remote'] not in ('merged', 'revoked'), 'no active PR binding')
        self.applicable(run)
        # Persist intent before external effects. A lost POST/PATCH response is recovered by listing
        # the same App/name/head and updating that check; never rerun Core or create another PR.
        if run['remote'] != 'published':
            self.journal.update(run_id, remote='reporting')
            check_id = self.github.update_check(run, self.config['check_name'], 'in_progress',
                                               summary='Local verification recorded; checking applicability.')
            run = self.journal.update(run_id, check_id=check_id)
        self.applicable(run)
        inputs = run['input']
        summary = (f"Repository: {run['repository']}\nCandidate: {run['head']}\n"
                   f"Tree: {inputs['tree']}\nBaseline: {inputs['base']}\n"
                   f"Policy: {inputs['policy']}\nGate run: {run['id']}\n"
                   f"Evidence: {digest(canonical(run['evidence']))}\n"
                   'Rust Core completed the configured full verification; no Actions rerun requested.')
        try:
            check_id = self.github.update_check(run, self.config['check_name'], 'completed', 'success', summary)
        except Blocked:
            self.revoke(run_id, 'GitHub returned an inconsistent authoritative Check result')
            raise
        self.journal.update(run_id, check_id=check_id, remote='published')
        # Recheck after success too. A strict source-bound branch rule is still essential for
        # the unavoidable race between API reads and GitHub's actual merge operation.
        self.applicable(run)
        return self.journal.get(run_id)

    def revoke(self, run_id, reason):
        run = self.journal.get(run_id)
        require(self.journal.latest(run['repository'], run['head'])['id'] == run_id,
                'superseded operation cannot overwrite the current Check')
        require(run['remote'] != 'merged', 'merged delivery cannot be undone by revoking a Check')
        self.executor.confirm_stopped(run)
        if run['remote'] == 'revoked':
            return run
        # Local revocation is recorded first; the slot remains held until remote failure is confirmed.
        self.journal.update(run_id, local='invalid', remote='revoking', reason=reason)
        old = self.github.find_check(run, self.config['check_name'])
        if old:
            check_id = self.github.update_check(run, self.config['check_name'], 'completed', 'failure', reason)
            self.journal.update(run_id, check_id=check_id)
        return self.journal.update(run_id, remote='revoked')

    def reconcile(self):
        results = []
        for run in self.journal.outstanding():
            if run['remote'] == 'revoking':
                results.append(self.revoke(run['id'], run['reason']))
                continue
            if run['local'] == 'running':
                # Do not launch another executor or guess whether an old process has stopped.
                results.append({'id': run['id'], 'blocked': 'confirm the executor process tree stopped before recovery'})
                continue
            if run['remote'] == 'reporting':
                results.append(self.publish(run['id']))
                continue
            if run['remote'] == 'published':
                try:
                    self.evidence(run)
                except Blocked:
                    results.append(self.revoke(run['id'], 'local evidence or policy is no longer applicable'))
                    continue
                pr = self.github.repo('GET', '/pulls/' + str(run['pr']))
                if pr.get('merged'):
                    merge = self.github.repo('GET', '/git/commits/' + pr['merge_commit_sha'])
                    require(pr['head']['sha'] == run['head'] and
                            pr['base']['repo']['full_name'] == run['repository'] and
                            merge['tree']['sha'] == run['input']['tree'] and
                            merge['parents'][0]['sha'] == run['input']['base'],
                            'landed content/baseline differs; merge/squash are supported, rebase requires separate equivalence')
                    self.journal.event(run['id'], 'merged', {'commit': merge['sha'], 'tree': merge['tree']['sha']})
                    results.append(self.journal.update(run['id'], remote='merged'))
                    continue
                try:
                    self.evidence(run)
                    self.github.remote(run, self.config)
                    check = self.github.find_check(run, self.config['check_name'])
                    require(check and check['external_id'] == run['id']
                            and check['status'] == 'completed' and check['conclusion'] == 'success',
                            'remote Check no longer matches the current gate run')
                except Blocked:
                    results.append(self.revoke(run['id'], 'reconciliation detected invalid or withdrawn result'))
                    continue
            results.append(run)
        return results
