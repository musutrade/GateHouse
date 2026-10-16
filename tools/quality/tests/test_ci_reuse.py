"""Real merge Git objects, fake read-only GitHub API, bounded receipt ZIPs."""
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_reuse as reuse

NOW = datetime(2026, 10, 15, 11, tzinfo=timezone.utc)
REPO = {'id': 71, 'full_name': 'fixture/repository', 'default_branch': 'main'}


class API:
    def __init__(self, fixture):
        self.f = fixture
        self.calls = []
        self.error = None
        self.run = {'id': 101, 'run_attempt': 1, 'repository': REPO, 'head_repository': REPO,
                    'head_sha': fixture.head, 'head_commit': {'tree_id': fixture.git('rev-parse', fixture.head+'^{tree}')},
                    'event': 'pull_request', 'path': reuse.WORKFLOW, 'workflow_id': 88,
                    'status': 'completed', 'conclusion': 'success', 'pull_requests': [],
                    'created_at': '2026-10-15T10:00:00Z'}
        self.attempt = copy.deepcopy(self.run)
        self.runs = [copy.deepcopy(self.run)]
        self.pr = {'number': 7, 'merged': True, 'state': 'closed', 'merge_commit_sha': fixture.merge,
                   'merged_at': '2026-10-15T10:30:00Z', 'base': {'ref': 'main', 'repo': REPO},
                   'head': {'sha': fixture.head, 'repo': REPO}}
        self.prs = [{'number': 7}]
        steps = [{'name': reuse.RECORD_STEP, 'number': 3, 'status': 'completed', 'conclusion': 'success',
                  'started_at': '2026-10-15T10:00:10Z', 'completed_at': '2026-10-15T10:00:11Z'},
                 {'name': reuse.UPLOAD_STEP, 'number': 4, 'status': 'completed', 'conclusion': 'success',
                  'started_at': '2026-10-15T10:00:12Z', 'completed_at': '2026-10-15T10:00:20Z'},
                 {'name': reuse.CLASSIFY_STEP, 'number': 5, 'status': 'completed', 'conclusion': 'success'}]
        self.jobs = [{'id': 1000+i, 'name': name, 'run_id': 101, 'head_sha': fixture.head,
                      'status': 'completed', 'conclusion': 'success', 'steps': steps if i == 0 else []}
                     for i, name in enumerate(reuse.REQUIRED_JOBS)]
        self.receipt = copy.deepcopy(fixture.receipt)
        self.artifact = {'id': 202, 'name': 'ci-equivalence-101-1', 'expired': False, 'size_in_bytes': 1000,
                         'created_at': '2026-10-15T10:00:15Z', 'updated_at': '2026-10-15T10:00:16Z',
                         'expires_at': '2026-11-15T10:00:15Z', 'workflow_run': {'id': 101,
                         'head_sha': fixture.head, 'repository_id': 71, 'head_repository_id': 71}}
        self.artifacts = [self.artifact]
        self.pack()

    def pack(self, entries=None):
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, 'w', zipfile.ZIP_DEFLATED) as archive:
            for name, data in entries or [('receipt.json', json.dumps(self.receipt).encode())]:
                archive.writestr(name, data)
        self.archive = stream.getvalue()
        self.artifact['digest'] = 'sha256:' + hashlib.sha256(self.archive).hexdigest()

    def get(self, path, binary=False):
        self.calls.append(('GET', path, binary))
        if self.error:
            raise self.error
        if path == '/pulls/7': return self.pr
        if path == '/actions/workflows/ci.yml':
            return {'id': 88, 'path': reuse.WORKFLOW, 'state': 'active'}
        if path == '/actions/runs/101': return self.run
        if path == '/actions/runs/101/attempts/1': return self.attempt
        if path == '/actions/artifacts/202/zip': return self.archive
        raise AssertionError('unexpected request: '+path)

    def pages(self, path, key=None):
        self.calls.append(('GET', path, False))
        if self.error:
            raise self.error
        if path == f'/commits/{self.f.merge}/pulls': return self.prs
        if path.startswith('/actions/workflows/88/runs?'): return self.runs
        if path == '/actions/runs/101/attempts/1/jobs': return self.jobs
        if path == '/actions/runs/101/artifacts': return self.artifacts
        raise AssertionError('unexpected request: '+path)


class Fixture:
    def __init__(self, root):
        self.root = root
        self.git('init', '--quiet')
        for name, value in [('user.name', 'Fixture'), ('user.email', 'fixture@example.invalid'),
                            ('commit.gpgsign', 'false'), ('core.hooksPath', str(root/'unused-hooks'))]:
            self.git('config', name, value)
        for path, data in [(reuse.PUBLISHER, Path(reuse.__file__).read_bytes()),
                           (reuse.WORKFLOW, b'name: CI\n'), ('product.txt', b'base\n'),
                           ('tools/harness-gate/Cargo.lock', b'locked\n')]:
            destination = root/path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        self.commit('ancestor')
        ancestor = self.git('rev-parse', 'HEAD')
        (root/'product.txt').write_text('PR product\n')
        self.commit('PR head')
        self.head = self.git('rev-parse', 'HEAD')
        self.git('checkout', '--quiet', '--detach', ancestor)
        (root/'base-only.txt').write_text('new base input\n')
        self.commit('base advanced')
        self.base = self.git('rev-parse', 'HEAD')
        self.git('merge', '--quiet', '--no-ff', self.head, '-m', 'effective PR merge')
        self.tested = self.git('rev-parse', 'HEAD')
        tree = self.git('rev-parse', 'HEAD^{tree}')
        self.merge = self.git('commit-tree', tree, '-p', self.base, '-p', self.head, '-m', 'main merge')
        self.pr_event = {'repository': REPO, 'pull_request': {'number': 7,
                         'base': {'sha': self.base, 'ref': 'main', 'repo': REPO},
                         'head': {'sha': self.head, 'repo': REPO}}}
        self.receipt = reuse.record(root, self.pr_event, REPO['full_name'], self.tested, 101, 1)
        self.git('checkout', '--quiet', '--detach', self.merge)
        self.event = {'repository': REPO, 'ref': 'refs/heads/main', 'before': self.base,
                      'after': self.merge, 'forced': False, 'deleted': False}

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.DEVNULL).decode().strip()

    def commit(self, message):
        self.git('add', '.')
        self.git('commit', '--quiet', '-m', message)

    def probe(self, api=None, now=NOW):
        return reuse.safe_probe(self.root, self.event, REPO['full_name'], self.merge, api or API(self), now)


class ReuseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.f = Fixture(Path(self.temp.name))

    def test_equivalent_merge_inherits_only_three_conclusions_with_original_provenance(self):
        api = API(self.f)
        self.assertNotEqual(self.f.head, self.f.tested)
        self.assertNotEqual(api.run['head_commit']['tree_id'], self.f.receipt['tested_tree'])
        result = self.f.probe(api)
        self.assertIs(result['reuse_verified'], True, result)
        self.assertEqual(result['reused_jobs'], list(reuse.REUSABLE))
        self.assertEqual(result['provenance']['tested_sha'], self.f.tested)
        self.assertEqual(result['provenance']['run_id'], 101)
        self.assertTrue(all(method == 'GET' for method, _, _ in api.calls))
        self.assertNotIn('fresh', result['reason'])

    def test_garbage_collected_synthetic_merge_and_empty_run_pr_array_are_supported(self):
        obj = self.f.root/'.git/objects'/self.f.tested[:2]/self.f.tested[2:]
        obj.unlink()
        self.assertTrue(self.f.probe()['reuse_verified'])

    def test_every_required_job_must_be_unique_and_successful_in_exact_attempt(self):
        for name in reuse.REQUIRED_JOBS:
            for conclusion in ['failure', 'cancelled', 'skipped', None]:
                with self.subTest(name=name, conclusion=conclusion):
                    api = API(self.f)
                    next(j for j in api.jobs if j['name'] == name)['conclusion'] = conclusion
                    self.assertFalse(self.f.probe(api)['reuse_verified'])
            for mutation in ['missing', 'duplicate', 'wrong-head', 'wrong-run']:
                with self.subTest(name=name, mutation=mutation):
                    api = API(self.f); job = next(j for j in api.jobs if j['name'] == name)
                    if mutation == 'missing': api.jobs.remove(job)
                    elif mutation == 'duplicate': api.jobs.append(copy.deepcopy(job))
                    elif mutation == 'wrong-head': job['head_sha'] = 'f'*40
                    else: job['run_id'] = 102
                    self.assertFalse(self.f.probe(api)['reuse_verified'])

    def test_receipt_self_claims_do_not_replace_api_verification(self):
        api = API(self.f); api.receipt['reuse_verified'] = True; api.pack()
        self.assertFalse(self.f.probe(api)['reuse_verified'])
        api = API(self.f); api.run['conclusion'] = 'failure'
        self.assertFalse(self.f.probe(api)['reuse_verified'])

    def test_receipt_identity_mutations_fall_back(self):
        changes = {'base_sha': 'e'*40, 'head_sha': 'e'*40, 'tested_tree': 'e'*40,
                   'workflow_blob': 'e'*40, 'inputs_sha256': 'e'*64, 'run_id': 102,
                   'run_attempt': 2, 'repository': 'other/repository', 'repository_id': 72,
                   'pull_request': 8, 'workflow': '.github/workflows/other.yml', 'eligible': False,
                   'tested_sha': 'not-a-commit', 'publisher_blobs': {}, 'schema': 'other/v1'}
        for key, value in changes.items():
            with self.subTest(key=key):
                api = API(self.f); api.receipt[key] = value; api.pack()
                self.assertFalse(self.f.probe(api)['reuse_verified'])

    def test_api_metadata_and_ambiguities_fall_back(self):
        mutations = [lambda a: a.pr.update(merged=False), lambda a: a.pr.update(merge_commit_sha='e'*40),
                     lambda a: a.pr['head'].update(sha='e'*40), lambda a: a.pr['head'].update(repo={'id':72}),
                     lambda a: a.prs.clear(), lambda a: a.prs.append({'number':8}),
                     lambda a: a.runs.clear(), lambda a: a.runs.append(copy.deepcopy(a.run)),
                     lambda a: a.run.update(run_attempt=2), lambda a: a.run.update(event='push'),
                     lambda a: a.run.update(status='in_progress'), lambda a: a.run.update(head_sha='e'*40),
                     lambda a: a.run.update(workflow_id=89), lambda a: a.run.update(path='other'),
                     lambda a: a.run.update(head_repository={'id':72}),
                     lambda a: a.attempt.update(run_attempt=2), lambda a: a.attempt.update(id=102),
                     lambda a: a.attempt.update(conclusion='failure'),
                     lambda a: a.artifacts.clear(), lambda a: a.artifacts.append(copy.deepcopy(a.artifact)),
                     lambda a: a.artifact.update(expired=True), lambda a: a.artifact.update(size_in_bytes=2**30),
                     lambda a: a.artifact.update(name='ci-equivalence-101-2'),
                     lambda a: a.artifact.update(created_at='2026-10-15T10:01:00Z'),
                     lambda a: a.artifact.update(expires_at='2026-10-15T10:30:00Z'),
                     lambda a: a.artifact.update(digest='sha256:'+'0'*64),
                     lambda a: a.artifact['workflow_run'].update(id=102),
                     lambda a: a.jobs[0]['steps'][0].update(conclusion='failure'),
                     lambda a: a.jobs[0]['steps'][1].update(number=6)]
        for i, mutate in enumerate(mutations):
            with self.subTest(i=i):
                api = API(self.f); mutate(api)
                self.assertFalse(self.f.probe(api)['reuse_verified'])
        for error in [PermissionError('denied'), OSError('API unavailable'), ValueError('invalid response')]:
            api = API(self.f); api.error = error
            self.assertFalse(self.f.probe(api)['reuse_verified'])

    def test_nonmatching_push_and_age_fall_back(self):
        for key, value in [('before', self.f.head), ('after','e'*40), ('forced',True),
                           ('deleted',True), ('ref','refs/heads/other')]:
            with self.subTest(key=key):
                old = self.f.event[key]; self.f.event[key] = value
                self.assertFalse(self.f.probe()['reuse_verified']); self.f.event[key] = old
        self.assertFalse(self.f.probe(now=NOW+timedelta(days=2))['reuse_verified'])
        self.assertFalse(self.f.probe(now=NOW-timedelta(days=2))['reuse_verified'])

    def test_bad_zip_duplicate_json_and_extra_members_fall_back(self):
        for entries in [[('../receipt.json', b'{}')], [('receipt.json', b'{"schema":1,"schema":2}')],
                        [('receipt.json', b' '* (reuse.MAX_RECEIPT+1))],
                        [('receipt.json', json.dumps(self.f.receipt).encode()), ('extra', b'bad')]]:
            api = API(self.f); api.pack(entries)
            self.assertFalse(self.f.probe(api)['reuse_verified'])
        api = API(self.f); api.archive = b'not ZIP'; api.artifact['digest'] = 'sha256:'+reuse.digest(api.archive)
        self.assertFalse(self.f.probe(api)['reuse_verified'])

    def test_producer_change_and_nonmerge_push_do_not_reuse(self):
        (self.f.root/reuse.PUBLISHER).write_text('changed producer\n')
        self.f.commit('producer changed')
        new_head = self.f.git('rev-parse','HEAD')
        changed_merge = self.f.git('commit-tree',new_head+'^{tree}','-p',self.f.base,'-p',self.f.head,'-m','changed publisher merge')
        self.f.git('checkout','--quiet','--detach',changed_merge)
        changed_event = copy.deepcopy(self.f.event); changed_event['after'] = changed_merge
        decision = reuse.safe_probe(self.f.root, changed_event, REPO['full_name'], changed_merge, API(self.f), NOW)
        self.assertFalse(decision['reuse_verified'])
        self.assertEqual(decision['reason'],'publisher changed since tested base')
        self.assertFalse(reuse.safe_probe(self.f.root, self.f.event, REPO['full_name'], new_head,
                                         API(self.f), NOW)['reuse_verified'])
        self.f.git('checkout','--quiet','--detach',self.f.tested)
        with self.assertRaises(reuse.Unusable):
            event = copy.deepcopy(self.f.pr_event); event['pull_request']['head']['repo'] = {'id':72}
            reuse.record(self.f.root,event,REPO['full_name'],self.f.tested,101,1)

    def test_rerun_starting_during_download_revokes_probe(self):
        api = API(self.f); original = api.get; reads = 0
        def changed(path, binary=False):
            nonlocal reads
            if path == '/actions/runs/101':
                reads += 1
                if reads == 2:
                    value = copy.deepcopy(api.run); value['run_attempt'] = 2
                    return value
            return original(path,binary)
        api.get = changed
        self.assertFalse(self.f.probe(api)['reuse_verified'])

    def test_bootstrap_publisher_missing_from_base_is_not_a_receipt(self):
        self.f.git('checkout','--quiet','--detach',self.f.base)
        (self.f.root/reuse.PUBLISHER).unlink(); self.f.commit('old base without publisher')
        old_base = self.f.git('rev-parse','HEAD')
        tested = self.f.git('commit-tree',self.f.merge+'^{tree}','-p',old_base,'-p',self.f.head,'-m','bootstrap merge')
        self.f.git('checkout','--quiet','--detach',tested)
        event = copy.deepcopy(self.f.pr_event); event['pull_request']['base']['sha'] = old_base
        event_path = self.f.root/'bootstrap.json'; event_path.write_text(json.dumps(event))
        output = self.f.root/'receipt.json'
        args = [sys.executable,'-I',str(self.f.root/reuse.PUBLISHER),'record','--repository',REPO['full_name'],
                '--event',str(event_path),'--event-name','pull_request','--sha',tested,
                '--run-id','101','--attempt','1','--output',str(output)]
        completed = subprocess.run(args,cwd=self.f.root,capture_output=True,text=True)
        self.assertEqual(completed.returncode,0,completed.stderr)
        self.assertFalse(json.loads(output.read_text()).get('eligible',False))

    def test_cli_pr_probe_cannot_emit_true_and_record_requires_isolation(self):
        event = self.f.root/'event.json'; event.write_text(json.dumps(self.f.pr_event))
        output = self.f.root/'decision.json'; gh_output = self.f.root/'outputs'
        args = [sys.executable, '-I', str(self.f.root/reuse.PUBLISHER), 'probe', '--repository',REPO['full_name'],
                '--event',str(event),'--event-name','pull_request','--sha',self.f.merge,
                '--output',str(output),'--github-output',str(gh_output)]
        p = subprocess.run(args,cwd=self.f.root,capture_output=True,text=True)
        self.assertEqual(p.returncode,0,p.stderr); self.assertFalse(json.loads(output.read_text())['reuse_verified'])
        self.assertEqual(gh_output.read_text(),'reuse_verified=false\n')
        self.f.git('checkout','--quiet','--detach',self.f.tested)
        args = [sys.executable, str(self.f.root/reuse.PUBLISHER), 'record', '--repository',REPO['full_name'],
                '--event',str(event),'--event-name','pull_request','--sha',self.f.tested,
                '--run-id','101','--attempt','1','--output',str(output)]
        subprocess.run(args,cwd=self.f.root,check=True,capture_output=True)
        self.assertFalse(json.loads(output.read_text()).get('eligible',False))
        args.insert(1,'-I'); subprocess.run(args,cwd=self.f.root,check=True,capture_output=True)
        self.assertTrue(json.loads(output.read_text())['eligible'])

    def test_optional_api_deadline_falls_back_without_another_network_request(self):
        with patch.object(reuse.time,'monotonic',side_effect=[0,46]):
            api = reuse.GitHub('fixture/repository','synthetic-token')
            with self.assertRaises(reuse.Unusable):
                api.get('/actions/runs/101')

    def test_archive_redirect_removes_credentials_and_rejects_http(self):
        from urllib.request import Request
        handler = reuse.NoAuthRedirect()
        request = Request('https://api.github.com/artifact',headers={'Authorization':'Bearer synthetic'})
        redirected = handler.redirect_request(request,None,302,'',{},'https://storage.example.invalid/file')
        self.assertIsNone(redirected.get_header('Authorization'))
        with self.assertRaises(reuse.Unusable):
            handler.redirect_request(request,None,302,'',{},'http://storage.example.invalid/file')


if __name__ == '__main__':
    unittest.main()
