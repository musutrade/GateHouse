"""Real candidate Git objects/archive bytes; simulated GitHub delivery only.

Set BASELINE_DELIVERY_TEST_EVIDENCE to a fresh directory to keep all fixtures.
These tests never establish actual Actions PR permission or accept a baseline.
"""
import base64
import contextlib
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qs, urlsplit
import warnings
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import baseline_delivery as delivery


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


class CaptureAPI(delivery.GitHub):
    """The only write accepted here is a PR POST; everything else is a GET."""
    def __init__(self, root):
        super().__init__('fixture/repository', 'synthetic-token')
        self.root, self.calls = root, []
        self.permission = {'default_workflow_permissions': 'read', 'can_approve_pull_request_reviews': True}
        self.files, verification = {}, {}
        for mode, peak in [('serial', 1), ('parallel', 2)]:
            runs = []
            for number in range(1, 6):
                prefix = f'benchmark-runs/{mode}/sample-{number}/'
                steps = [{'label': name, 'duration_ms': 100, 'passed': True, 'log': prefix+'logs/'+name+'.log'}
                         for name in ['scan', 'audit', 'diff', 'status']]
                for step in steps:
                    self.files[step['log']] = b'original worker evidence\n'
                raw_steps = [{**step, 'log': '/original/.harness-gate/reports/' +
                              step['log'].removeprefix(prefix)} for step in steps]
                self.files[prefix+'test_result.json'] = encoded({'passed': True, 'steps': raw_steps})
                self.files[prefix+'parallel-state.json'] = encoded({'current': 0, 'workers': 2, 'observed_peak': peak})
                self.files[prefix+'command-result.json'] = encoded({'exit_code': 0, 'seconds': 1.25})
                runs.append({'sample': number, 'mode': mode, 'seconds': 1.25, 'configured_limit': peak,
                             'observed_peak': peak, 'report': prefix+'test_result.json', 'steps': steps})
            verification[mode] = {'count': 5, 'samples': [1.25]*5, 'runs': runs}
        scope = []
        for number in range(1, 6):
            row = {'sample': number, 'mode': 'working-tree', 'paths': 601, 'iterations': 100,
                   'equivalent': True, 'cached_total_us': 100, 'uncached_total_us': 200,
                   'cached_per_iteration_us': 1.0, 'uncached_per_iteration_us': 2.0,
                   'speedup': 2.0, 'wall_seconds': 1.0}
            name = f'benchmark-runs/scope/sample-{number}.json'
            self.files[name] = encoded(row)
            scope.append({**row, 'raw': name})
        self.candidate = {'tool': 'quality-benchmarks', 'harness_version': 1, 'fixture_version': 1,
                          'rustc': 'rustc original', 'cargo': 'cargo original', 'target': 'original-target',
                          'series_key': 'original-series', 'verification': verification,
                          'scope_matcher': {'runs': scope},
                          'tests_seconds': {'count': 5, 'warm_samples': [1.0]*5, 'cold': 2.0},
                          'regression_policy': {'threshold_percent': 15.0},
                          'comparison': {'regression': True}}
        self.git(['init', '--quiet'])
        self.git(['config', 'user.name', 'Capture fixture'])
        self.git(['config', 'user.email', 'fixture@example.invalid'])
        self.git(['config', 'commit.gpgsign', 'false'])
        self.git(['config', 'core.hooksPath', str(root/'unused-hooks')])
        for name in delivery.FILES:
            path = root/name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('old baseline\n')
        # Candidate code must remain data; neither verification nor delivery
        # may execute it or rebuild this source.
        (root/'do-not-execute.py').write_text('raise RuntimeError("candidate executed")\n')
        self.git(['add', '.']); self.git(['commit', '--quiet', '-m', 'measured source'])
        self.measured = self.git(['rev-parse', 'HEAD']).strip()
        self.candidate['commit'] = self.measured
        self.blobs = {delivery.FILES[0]: encoded(self.candidate), delivery.FILES[1]: b'# Original candidate\n'}
        for name, value in self.blobs.items():
            (root/name).write_bytes(value)
        self.git(['add', '--', *delivery.FILES]); self.git(['commit', '--quiet', '-m', 'existing action candidate'])
        self.candidate_sha = self.git(['rev-parse', 'HEAD']).strip()
        self.branch = self.candidate_sha
        self.repo = {'id': 71, 'full_name': self.repository, 'default_branch': 'main'}
        self.run = {'id': 101, 'run_attempt': 1, 'path': delivery.WORKFLOW, 'event': 'schedule',
                    'head_sha': self.measured, 'head_branch': 'main', 'repository': self.repo,
                    'head_repository': self.repo, 'conclusion': 'failure'}
        self.steps = [{'name': delivery.CAPTURE, 'conclusion': 'success',
                       'started_at': '2026-10-01T00:00:00Z', 'completed_at': '2026-10-01T00:01:00Z'},
                      {'name': delivery.UPLOAD, 'conclusion': 'success',
                       'started_at': '2026-10-01T00:02:00Z', 'completed_at': '2026-10-01T00:03:00Z'}]
        self.attempt_jobs = {1: [{'name': delivery.JOB, 'run_id': 101, 'steps': self.steps}]}
        self.artifact = {'id': 202, 'name': 'quality-baseline-refresh-101-1', 'expired': False,
                         'created_at': '2026-10-01T00:02:10Z', 'updated_at': '2026-10-01T00:02:20Z',
                         'workflow_run': {'id': 101, 'head_sha': self.measured,
                                          'repository_id': 71, 'head_repository_id': 71}}
        self.repack()
        self.parents = [self.measured]
        self.changed = [{'filename': name, 'status': 'modified'} for name in delivery.FILES]
        self.prs, self.post_error, self.after_post_sha = [], None, None
        self.attempt_identity_override = None

    def git(self, arguments):
        return subprocess.check_output(['git', *arguments], cwd=self.root, text=True)

    def repack(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            for name, value in sorted(self.files.items()):
                archive.writestr(name, value)
        self.archive = data.getvalue()
        self.artifact['digest'] = 'sha256:' + delivery.sha(self.archive)
        (self.root/'original-artifact.zip').write_bytes(self.archive)

    def pr(self):
        return {'head': {'sha': self.candidate_sha, 'ref': 'automation/quality-baseline-101', 'repo': self.repo},
                'base': {'ref': 'main', 'repo': self.repo}, 'html_url': 'https://github.com/fixture/repository/pull/7',
                'body': delivery.association_line({'repository': self.repository, 'run_id': 101, 'run_attempt': 1,
                        'measured_sha': self.measured, 'artifact_id': 202, 'artifact_digest': self.artifact['digest']})}

    def request(self, path, body=None, binary=False):
        self.calls.append({'path': path, 'body': body, 'binary': binary})
        (self.root/'api-calls.json').write_bytes(encoded(self.calls))
        if body is not None:
            if path != '/pulls':
                raise AssertionError('unexpected GitHub write: '+path)
            if self.post_error:
                raise self.post_error
            pr = self.pr(); pr['body'] = body['body']; self.prs = [pr]
            if self.after_post_sha:
                self.branch = self.after_post_sha
            return pr
        target = urlsplit(path)
        if path == '': return self.repo
        if path == '/actions/permissions/workflow':
            if isinstance(self.permission, Exception): raise self.permission
            return self.permission
        if target.path == '/actions/runs/101': return self.run
        if target.path.startswith('/actions/runs/101/attempts/'):
            number = int(target.path.split('/')[5])
            if target.path.endswith('/jobs'): return {'jobs': self.attempt_jobs[number]}
            return {**self.run, 'run_attempt': self.attempt_identity_override or number}
        if target.path == '/actions/runs/101/artifacts': return {'artifacts': [self.artifact]}
        if path == '/actions/artifacts/202/zip': return self.archive
        if path == '/git/ref/heads/automation/quality-baseline-101': return {'object': {'sha': self.branch}}
        if target.path.startswith('/git/commits/'):
            return {'sha': self.candidate_sha, 'parents': [{'sha': value} for value in self.parents]}
        if target.path.startswith('/compare/'):
            return {'total_commits': 1, 'status': 'ahead', 'base_commit': {'sha': self.measured}, 'files': self.changed}
        if target.path.startswith('/contents/'):
            self.assert_candidate_ref(parse_qs(target.query)['ref'][0])
            name = target.path.removeprefix('/contents/'); value = self.blobs[name]
            return {'type': 'file', 'path': name, 'encoding': 'base64', 'size': len(value),
                    'sha': hashlib.sha1(b'blob '+str(len(value)).encode()+b'\0'+value).hexdigest(),
                    'content': base64.b64encode(value).decode()}
        if target.path == '/pulls': return self.prs
        if target.path == '/pulls/7': return self.pr()
        raise AssertionError('unexpected GitHub request: '+path)

    def assert_candidate_ref(self, value):
        if value != self.candidate_sha: raise AssertionError('unpinned candidate read')


class BaselineDeliveryTests(unittest.TestCase):
    def setUp(self):
        retained = os.environ.get('BASELINE_DELIVERY_TEST_EVIDENCE')
        if retained:
            Path(retained).mkdir(parents=True, exist_ok=True)
            self.root = Path(tempfile.mkdtemp(prefix=self._testMethodName+'-', dir=retained))
        else:
            work = tempfile.TemporaryDirectory()
            self.addCleanup(work.cleanup)
            self.root = Path(work.name)
        self.api = CaptureAPI(self.root)
        self.invocations = 0

    def verify(self):
        return delivery.verify_existing(self.api, 101, 1, self.api.candidate_sha, 202, self.api.artifact['digest'])

    def invoke(self, operation, pins=True):
        arguments = ['baseline_delivery.py', operation]
        if pins:
            arguments += ['--run-id', '101', '--attempt', '1', '--candidate-sha', self.api.candidate_sha,
                          '--artifact-id', '202', '--artifact-digest', self.api.artifact['digest']]
        self.invocations += 1
        output = self.root/f'result-{self.invocations}.json'
        arguments += ['--output', str(output)]
        console = io.StringIO()
        with patch.object(delivery, 'GitHub', return_value=self.api), \
                patch.dict(os.environ, {'GITHUB_REPOSITORY': self.api.repository, 'GH_TOKEN': 'synthetic-token',
                                        'GITHUB_STEP_SUMMARY': str(self.root/'summary.md')}), \
                patch.object(sys, 'argv', arguments), contextlib.redirect_stdout(console):
            status = delivery.main()
        (self.root/f'invocation-{self.invocations}.json').write_bytes(encoded({
            'arguments': arguments, 'status': status, 'stdout': console.getvalue(), 'api_calls': self.api.calls}))
        return status, json.loads(output.read_text())

    def test_precheck_false_stops_and_true_is_policy_enabled_even_with_read_default(self):
        for enabled, state, expected in [(False, 'denied', 1), (True, 'policy-enabled', 0)]:
            self.api.permission['can_approve_pull_request_reviews'] = enabled
            self.api.calls.clear()
            status, report = self.invoke('precheck', pins=False)
            self.assertEqual((status, report['permission']), (expected, state))
            self.assertEqual([row['path'] for row in self.api.calls], ['/actions/permissions/workflow'])
            self.assertFalse(report['baseline_accepted'])

    def test_precheck_unreadable_or_malformed_is_unknown_never_available(self):
        cases = [{}, {'can_approve_pull_request_reviews': 'true'}, None,
                 URLError('unreachable'), TimeoutError('timeout')]
        cases += [HTTPError('https://api.github.com', code, 'denied', {}, None) for code in (403, 404, 500)]
        for value in cases:
            with self.subTest(value=value):
                self.api.permission = value
                status, report = self.invoke('precheck', pins=False)
                self.assertEqual((status, report['permission']), (0, 'unknown'))
                self.assertIn('not a precheck PASS', report['reason'])

    def test_failed_pr_can_retry_original_sha_archive_and_tools_without_measurement(self):
        before = (copy.deepcopy(self.api.candidate), self.api.archive, self.api.git(['rev-parse', 'HEAD']))
        self.api.post_error = HTTPError('https://api.github.com', 403, 'Actions cannot create PR', {}, None)
        status, failure = self.invoke('deliver')
        self.assertEqual((status, failure['delivery']), (1, 'blocked'))
        self.assertEqual(failure['candidate_sha'], self.api.candidate_sha)
        self.assertEqual(failure['artifact_digest'], self.api.artifact['digest'])
        self.assertEqual(failure['original_tool_identity']['series_key'], 'original-series')
        self.assertEqual(failure['raw_sha256'], {name: delivery.sha(data) for name, data in self.api.files.items()})
        self.api.post_error = None
        # No process may be launched by delivery. Fixture Git objects already exist.
        with patch.object(subprocess, 'Popen', side_effect=AssertionError('measurement or candidate execution')):
            status, report = self.invoke('deliver')
        self.assertEqual((status, report['delivery']), (0, 'created'))
        self.assertFalse(report['baseline_accepted'])
        self.assertEqual(report['original_tool_identity']['series_key'], 'original-series')
        self.assertEqual(before, (self.api.candidate, self.api.archive, self.api.git(['rev-parse', 'HEAD'])))
        post = next(row for row in self.api.calls if row['body'])
        for value in (self.api.candidate_sha, self.api.measured, '101/1', '/artifacts/202', self.api.artifact['digest']):
            self.assertIn(value, post['body']['body'])
        self.assertTrue(all(row['path'] == '/pulls' for row in self.api.calls if row['body']))

    def test_unknown_delivery_keeps_pr_intent_and_denied_never_reads_candidate(self):
        self.api.permission = HTTPError('', 403, 'no admin read', {}, None)
        status, report = self.invoke('deliver')
        self.assertEqual((status, report['permission']), (0, 'unknown'))
        self.api.permission = {'can_approve_pull_request_reviews': False}; self.api.calls.clear()
        status, report = self.invoke('deliver')
        self.assertEqual((status, report['delivery']), (1, 'blocked'))
        self.assertEqual(len(self.api.calls), 1)

    def test_existing_pr_is_reused_and_branch_or_pr_movement_is_blocked(self):
        self.api.prs = [self.api.pr()]
        report = delivery.deliver(self.api, self.verify())
        self.assertEqual(report['delivery'], 'existing')
        self.assertFalse(any(row['body'] for row in self.api.calls))
        self.api.prs[0]['head']['sha'] = 'f'*40
        with self.assertRaisesRegex(ValueError, 'candidate/PR identity changed'):
            delivery.deliver(self.api, self.verify())
        self.api.prs = []; self.api.after_post_sha = 'e'*40
        with self.assertRaisesRegex(ValueError, 'candidate/PR identity changed'):
            delivery.deliver(self.api, self.verify())

    def test_existing_pr_requires_unique_exact_original_evidence_association(self):
        verified = self.verify()
        valid = delivery.association_line(verified)
        cases = ['', 'Original run/attempt: 101/1', valid+'\n'+valid]
        for key, value in [('source_run_id', 102), ('source_run_attempt', 2), ('artifact_id', 203),
                           ('artifact_digest', 'sha256:'+'0'*64), ('repository', 'other/repository'),
                           ('measured_sha', 'a'*40), ('source_run_id', '101')]:
            changed = delivery.association(verified); changed[key] = value
            cases.append(delivery.LINK_PREFIX+json.dumps(changed)+' -->')
        cases.append(valid.replace('"artifact_id":202', '"artifact_id":202,"artifact_id":202'))
        for body in cases:
            with self.subTest(body=body):
                self.api.prs = [self.api.pr()]; self.api.prs[0]['body'] = body
                with self.assertRaisesRegex(ValueError, 'PR evidence association'):
                    delivery.deliver(self.api, verified)
        self.assertFalse(any(row['body'] for row in self.api.calls))

    def test_source_attempt_candidate_parent_and_exact_file_boundary_fail_closed(self):
        self.api.attempt_identity_override = 2
        with self.assertRaisesRegex(ValueError, 'original repository/workflow/run/attempt'): self.verify()
        self.api.attempt_identity_override = None
        mutations = [('event', 'pull_request'), ('path', '.github/workflows/other.yml'),
                     ('head_branch', 'untrusted'), ('head_repository', {'id': 99})]
        for key, value in mutations:
            original = copy.deepcopy(self.api.run)
            self.api.run[key] = value
            with self.assertRaisesRegex(ValueError, 'original repository/workflow/run/attempt'):
                self.verify()
            self.api.run = original
        self.api.parents.append('a'*40)
        with self.assertRaisesRegex(ValueError, 'sole parent'): self.verify()
        self.api.parents.pop()
        self.api.changed.append({'filename': 'do-not-execute.py', 'status': 'modified'})
        with self.assertRaisesRegex(ValueError, 'exactly the two baseline files'): self.verify()
        self.api.changed.pop(); self.api.branch = 'b'*40
        with self.assertRaisesRegex(ValueError, 'branch moved'): self.verify()

    def test_attempt_specific_steps_and_artifact_identity_digest_are_required(self):
        changes = [('expired', True, 'expired'), ('name', 'other-run', 'artifact name'),
                   ('created_at', '2026-10-02T00:00:00Z', 'requested attempt upload'),
                   ('workflow_run', {'id': 303}, 'mixed artifact source identity')]
        for key, value, message in changes:
            original = copy.deepcopy(self.api.artifact)
            self.api.artifact[key] = value
            with self.assertRaisesRegex(ValueError, message): self.verify()
            self.api.artifact = original
        self.api.steps[0]['conclusion'] = 'failure'
        with self.assertRaisesRegex(ValueError, 'capture/upload step did not succeed'): self.verify()
        self.api.steps[0]['conclusion'] = 'success'
        self.api.archive += b'changed bytes'
        with self.assertRaisesRegex(ValueError, 'archive digest mismatch'): self.verify()

    def test_candidate_json_measured_identity_and_git_blob_digest_are_exact(self):
        blob = self.api.blobs[delivery.FILES[0]]
        changed = copy.deepcopy(self.api.candidate); changed['commit'] = 'a'*40
        self.api.blobs[delivery.FILES[0]] = encoded(changed)
        with self.assertRaisesRegex(ValueError, 'candidate measured commit differs'): self.verify()
        self.api.blobs[delivery.FILES[0]] = blob
        original = self.api.request
        def bad_blob(path, *args, **kwargs):
            result = original(path, *args, **kwargs)
            if path.startswith('/contents/'): result['sha'] = '0'*40
            return result
        with patch.object(self.api, 'request', side_effect=bad_blob):
            with self.assertRaisesRegex(ValueError, 'Git blob identity mismatch'): self.verify()
        with self.assertRaisesRegex(ValueError, 'digest pin differs from API'):
            delivery.verify_existing(self.api, 101, 1, self.api.candidate_sha, 202, 'sha256:'+'0'*64)

    def test_real_candidate_fixture_has_one_parent_two_files_and_unexecuted_canary(self):
        self.assertEqual(self.api.git(['rev-list', '--parents', '-n', '1', 'HEAD']).split(),
                         [self.api.candidate_sha, self.api.measured])
        self.assertEqual(set(self.api.git(['diff', '--name-only', self.api.measured, 'HEAD']).splitlines()),
                         set(delivery.FILES))
        self.assertTrue((self.root/'do-not-execute.py').read_text().startswith('raise RuntimeError'))
        with patch.object(subprocess, 'Popen', side_effect=AssertionError('candidate execution')):
            self.verify()

    def test_api_reads_are_bounded_and_pagination_cannot_silently_truncate(self):
        api = delivery.GitHub('fixture/repository', 'synthetic-token')
        class Response:
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def read(self, limit):
                self.limit = limit
                return b'x'*limit
        response = Response()
        with patch.object(delivery, 'urlopen', return_value=response) as opened:
            with self.assertRaisesRegex(ValueError, 'API response exceeds size limit'):
                api.request('/actions/runs/101')
        request = opened.call_args.args[0]
        self.assertEqual(request.get_method(), 'GET')
        self.assertEqual(request.full_url, 'https://api.github.com/repos/fixture/repository/actions/runs/101')
        self.assertEqual(opened.call_args.kwargs['timeout'], 30)
        self.assertEqual(response.limit, 8*1024*1024+1)
        with patch.object(api, 'request', return_value={'jobs': [{}]*100}) as requests:
            with self.assertRaisesRegex(ValueError, 'pagination exceeds bounded workflow scope'):
                api.pages('/actions/runs/101/attempts/1/jobs', 'jobs')
        self.assertEqual(requests.call_count, 10)
        self.assertIn('page=10', requests.call_args.args[0])

    def test_legacy_artifact_keeps_absent_fields_and_requires_unique_attempt_window(self):
        self.api.artifact['name'] = 'quality-baseline-refresh-101'
        self.api.files = {name: data for name, data in self.api.files.items() if not name.endswith('/command-result.json')}
        self.api.repack()
        report = self.verify()
        self.assertEqual(len(report['original_absent_command_records']), 10)
        self.assertIn('not authenticated', report['warm_command_records'])
        self.assertTrue(any('/attempts/1/jobs?' in row['path'] for row in self.api.calls))
        self.api.run['run_attempt'] = 2
        self.api.attempt_jobs[2] = copy.deepcopy(self.api.attempt_jobs[1])
        with self.assertRaisesRegex(ValueError, 'ambiguous legacy artifact attempt'): self.verify()
        self.api.attempt_jobs[2][0]['steps'][1]['completed_at'] = None
        with self.assertRaisesRegex(ValueError, 'incomplete legacy upload window'): self.verify()

    def test_retained_raw_samples_references_and_summary_are_checked_without_new_formulas(self):
        candidate = self.api.candidate
        original = copy.deepcopy(candidate)
        for rows in [candidate['tests_seconds']['warm_samples'], candidate['verification']['serial']['runs'],
                     candidate['verification']['parallel']['runs'], candidate['scope_matcher']['runs']]:
            saved = list(rows)
            for count in (0, 4, 6):
                rows[:] = (saved + saved[:1])[:count]
                with self.assertRaisesRegex(ValueError, 'five samples'):
                    delivery.raw_evidence(candidate, self.api.files)
            rows[:] = saved
        log = candidate['verification']['serial']['runs'][0]['steps'][0]['log']
        saved = self.api.files.pop(log)
        with self.assertRaisesRegex(ValueError, 'missing original step log'):
            delivery.raw_evidence(candidate, self.api.files)
        self.api.files[log] = saved
        step = candidate['verification']['serial']['runs'][0]['steps'][0]
        step['log'] = candidate['verification']['serial']['runs'][0]['steps'][1]['log']
        with self.assertRaisesRegex(ValueError, 'original log reference/summary mismatch'):
            delivery.raw_evidence(candidate, self.api.files)
        step['log'] = log
        raw = 'benchmark-runs/scope/sample-1.json'; saved = self.api.files[raw]
        self.api.files[raw] = encoded({'sample': 1})
        with self.assertRaisesRegex(ValueError, 'raw scope/summary mismatch'):
            delivery.raw_evidence(candidate, self.api.files)
        self.api.files[raw] = saved
        evidence = delivery.raw_evidence(candidate, self.api.files)
        self.assertEqual(candidate, original)
        self.assertEqual(evidence['raw_sha256'][log], delivery.sha(self.api.files[log]))

    def test_zip_limits_and_unsafe_duplicate_symlink_paths_reject_before_read(self):
        def archive(names):
            stream = io.BytesIO()
            with warnings.catch_warnings(), zipfile.ZipFile(stream, 'w') as output:
                warnings.simplefilter('ignore', UserWarning)
                for name in names: output.writestr(name, b'bytes')
            return stream.getvalue()
        for names in [['/absolute'], ['../escape'], ['x/../escape'], ['x\\escape'], ['C:/escape'],
                      ['x//escape'], ['x/./escape'], ['same', 'same'], ['file', 'file/child']]:
            data = archive(names)
            with self.assertRaisesRegex(ValueError, 'archive path|file/directory collision'):
                delivery.archive_files(data, 'sha256:'+delivery.sha(data))
        link = zipfile.ZipInfo('link'); link.create_system = 3; link.external_attr = (stat.S_IFLNK | 0o777) << 16
        data = archive([link])
        with self.assertRaisesRegex(ValueError, 'symlink/special/encrypted'):
            delivery.archive_files(data, 'sha256:'+delivery.sha(data))
        data = archive(['a', 'b'])
        for key, limit in [('MAX_ENTRIES', 1), ('MAX_EXPANDED', 1), ('MAX_ARCHIVE', 1)]:
            with patch.object(delivery, key, limit), self.assertRaisesRegex(ValueError, 'size limit'):
                delivery.archive_files(data, 'sha256:'+delivery.sha(data))

    def test_missing_pins_and_summary_missing_branch_artifact_are_explicit_blockers(self):
        status, report = self.invoke('deliver', pins=False)
        self.assertEqual(status, 1); self.assertIn('identity pins', report['reason'])
        self.assertFalse(any(row['body'] for row in self.api.calls))
        original = self.api.request
        def no_branch(path, *args, **kwargs):
            if path.startswith('/git/ref/'): raise HTTPError('', 404, 'not found', {}, None)
            return original(path, *args, **kwargs)
        with patch.object(self.api, 'request', side_effect=no_branch):
            report = delivery.summary(self.api, 101, 1)
        self.assertEqual(len(report['blockers']), 2)
        self.assertNotIn('candidate_sha', report)
        self.assertEqual(report['delivery'], 'unverified')

    def test_workflow_preserves_capture_action_always_raw_upload_and_minimal_retry_permissions(self):
        root = Path(__file__).resolve().parents[3]
        workflow = (root/'.github/workflows/quality-baseline-refresh.yml').read_text()
        capture, retry = workflow.split('  delivery-only:\n')
        self.assertLess(capture.index('baseline_delivery.py precheck'), capture.index('name: Install Rust'))
        self.assertIn('--samples 5', capture)
        self.assertIn('uses: peter-evans/create-pull-request@22a9089034f40e5a961c8808d113e2c98fb63676 # v7', capture)
        self.assertIn('name: Upload raw benchmark evidence\n        id: evidence\n        if: ${{ always() }}', capture)
        self.assertIn('path: target/quality', capture)
        self.assertIn('if-no-files-found: warn', capture)
        guard = capture[capture.index('name: Require retained evidence'):capture.index('name: Create baseline')]
        for expected in ['steps.capture.outcome', 'steps.evidence.outcome', '^\u005b0-9\u005d+$', '^\u005b0-9a-f\u005d{64}$', 'exit 1']:
            self.assertIn(expected, guard)
        self.assertIn(delivery.LINK_PREFIX, capture)
        self.assertIn('github.run_attempt', capture)
        self.assertEqual(workflow.count('actions: read'), 2)
        self.assertIn('contents: read', retry); self.assertIn('pull-requests: write', retry)
        self.assertIn('ref: ${{ github.event.repository.default_branch }}', retry)
        self.assertIn('cancel-in-progress: false', workflow)
        for forbidden in ['benchmarks.py', 'Install Rust', 'cargo ', 'nextest', 'create-pull-request', 'git commit',
                          'git push', 'rebase', '--force', '--accept', 'stage', 'seal']:
            self.assertNotIn(forbidden, retry)
        self.assertNotIn('artifact-only', workflow)
        source = (root/'tools/quality/baseline_delivery.py').read_text()
        for forbidden in ['subprocess', 'os.system', "method='PUT'", "method='PATCH'"]:
            self.assertNotIn(forbidden, source)


if __name__ == '__main__':
    unittest.main()
