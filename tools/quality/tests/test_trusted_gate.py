"""External delivery failures cannot manufacture a Core PASS or repeat verification."""
from __future__ import annotations

import copy
import json
import importlib.util
import io
import os
from pathlib import Path
import runpy
import stat
import tarfile
import sys
import tempfile
import tomllib
import unittest
import subprocess
import uuid
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('gatehouse_trusted_gate',
    ROOT / 'tools/trusted_gate/__init__.py', submodule_search_locations=[str(ROOT / 'tools/trusted_gate')])
PACKAGE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PACKAGE
SPEC.loader.exec_module(PACKAGE)
from gatehouse_trusted_gate.common import Blocked, canonical, digest, read_json, read_regular, read_submission, submission_inbox
from gatehouse_trusted_gate.control import Control
from gatehouse_trusted_gate.github import APIError, GitHub
from gatehouse_trusted_gate.journal import Journal
from gatehouse_trusted_gate.cache import Cache, DIMENSIONS, identity, dimensions as resolved_dimensions
from gatehouse_trusted_gate.isolation import sandbox_arguments, unprivileged_identity
from gatehouse_trusted_gate.execution import Executor as HostExecutor, clean_environment
from gatehouse_trusted_gate.source import prepare, retire
from gatehouse_trusted_gate.history import prepare as prepare_history
from gatehouse_trusted_gate.publication import deliver

PILOT_SPEC = importlib.util.spec_from_file_location('pilot_installer', ROOT / 'tools/trusted_gate/deploy/install-pilot.py')
PILOT = importlib.util.module_from_spec(PILOT_SPEC)
PILOT_SPEC.loader.exec_module(PILOT)
PROBE_SPEC = importlib.util.spec_from_file_location('pilot_probe', ROOT / 'tools/trusted_gate/deploy/pilot-probe.py')
PROBE = importlib.util.module_from_spec(PROBE_SPEC)
PROBE_SPEC.loader.exec_module(PROBE)
UPGRADE_SPEC = importlib.util.spec_from_file_location('pilot_upgrade', ROOT / 'tools/trusted_gate/deploy/upgrade-history-pilot.py')
UPGRADE = importlib.util.module_from_spec(UPGRADE_SPEC)
UPGRADE_SPEC.loader.exec_module(UPGRADE)


class PilotReleaseTests(unittest.TestCase):
    def archive(self, mutation=None, extra=None):
        files = {name: b'data' for name in PILOT.FILES}
        meta = {'repository': 'musutrade/GateHouse', 'head': 'a' * 40, 'coverage_acceptance': 'transition',
                'core_profile': 'release', 'release_incremental': False,
                'core_build': {'sha256': digest(files['bin/harness-gate'])}}
        files['metadata.json'] = canonical(meta)
        if mutation:
            mutation(files, meta)
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode='w:') as archive:
            for name, data in files.items():
                item = tarfile.TarInfo(name)
                item.size = len(data)
                archive.addfile(item, io.BytesIO(data))
            if extra:
                archive.addfile(extra)
        return stream.getvalue()

    def test_fixed_release_inventory_and_binary_provenance(self):
        data = self.archive()
        files, meta = PILOT.release(data, digest(data))
        self.assertEqual(set(files), PILOT.FILES)
        self.assertEqual(meta['coverage_acceptance'], 'transition')
        with self.assertRaisesRegex(ValueError, 'digest'):
            PILOT.release(data, '0' * 64)

    def test_traversal_duplicate_link_special_and_oversize_members_are_rejected(self):
        for name in ('../escape', '/etc/shadow', 'source.bundle'):
            item = tarfile.TarInfo(name)
            data = self.archive(extra=item)
            with self.subTest(name=name), self.assertRaises(ValueError):
                PILOT.release(data, digest(data))
        for kind, size in ((tarfile.SYMTYPE, 0), (tarfile.LNKTYPE, 0), (tarfile.CHRTYPE, 0),
                           (tarfile.REGTYPE, 128 * 1024 * 1024 + 1)):
            item = tarfile.TarInfo('bin/harness-gate')
            item.type, item.size, item.linkname = kind, size, '/etc/shadow'
            # A real declared-size header tests rejection before any oversized allocation.
            data = item.tobuf() + b'\0' * 1024
            with self.subTest(kind=kind, size=size), self.assertRaises(ValueError):
                PILOT.release(data, digest(data))

    def test_incomplete_complete_mode_and_changed_binary_cannot_be_installed(self):
        def changed_mode(files, meta):
            meta['coverage_acceptance'] = 'complete'
            files['metadata.json'] = canonical(meta)
        for mutation in (lambda files, meta: files.pop('source.bundle'), changed_mode,
                         lambda files, meta: files.update({'bin/harness-gate': b'different'})):
            data = self.archive(mutation=mutation)
            with self.assertRaises(ValueError):
                PILOT.release(data, digest(data))

    @unittest.skipIf(os.geteuid() == 0, 'requires an actual non-root caller')
    def test_installer_cannot_elevate_a_nonroot_caller(self):
        result = subprocess.run([sys.executable, '-I', str(ROOT / 'tools/trusted_gate/deploy/install-pilot.py'),
            '--release', '/nonexistent/gatehouse-pilot.tar', '--sha256', '0' * 64],
            capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertIn('administrator authentication required', json.loads(result.stderr)['reason'])


class PilotUpgradeTests(unittest.TestCase):
    def payloads(self):
        before = {name: b'unchanged' for name in PILOT.FILES if name != 'trusted_gate/history.py'}
        after = {**before, 'trusted_gate/history.py': b'new history'}
        old = {'head': UPGRADE.OLD_HEAD, 'coverage_acceptance': 'transition'}
        new = {'head': 'a' * 40, 'base_at_packaging': 'b' * 40,
               'coverage_acceptance': 'transition', 'worker_git_history_probe': True}
        return before, after, old, new

    def test_exact_transition_upgrade_only_allows_reviewed_files(self):
        before, after, old, new = self.payloads()
        after['trusted_gate/isolation.py'] = b'reviewed isolation'
        files = UPGRADE.plan(before, after, old, new)
        self.assertEqual(files['/opt/gatehouse/trusted_gate/history.py']['before'], None)
        self.assertEqual(files['/opt/gatehouse/bin/pilot-probe.py']['mode'], 0o755)
        for name in ('bin/harness-gate', 'sudoers', 'policy/flow.toml', 'srv-gatehouse-pilot.mount'):
            modified = {**after, name: b'changed unrelated protection'}
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'unreviewed'):
                UPGRADE.plan(before, modified, old, new)

    def test_unknown_release_inventory_or_missing_live_probe_is_rejected(self):
        before, after, old, new = self.payloads()
        for before_meta, after_meta, files in (({**old, 'head': 'c' * 40}, new, after),
                (old, {**new, 'coverage_acceptance': 'complete'}, after),
                (old, {**new, 'worker_git_history_probe': False}, after),
                (old, new, {**after, 'keys/private-key.pem': b'forbidden'})):
            with self.assertRaises(ValueError):
                UPGRADE.plan(before, files, before_meta, after_meta)

    def test_expected_configuration_preserves_unrelated_permissions_and_inputs(self):
        before, after, old, new = self.payloads()
        files = UPGRADE.plan(before, after, old, new)
        config = {'app_id': 123, 'delivery': {'app_id': 456}, 'coverage_acceptance': 'transition',
                  'environment': {'source_revision': old['head'], 'release_sha256': 'old'},
                  'protected_files': {'/opt/gatehouse/trusted_gate/isolation.py': 'old'},
                  'steps': {'pilot-boundary': {'argv': ['old'], 'timeout_seconds': 120}}}
        changed = UPGRADE.expected_config(config, files, new, 'new-digest')
        self.assertEqual(changed['app_id'], config['app_id'])
        self.assertEqual(changed['delivery'], config['delivery'])
        self.assertEqual(changed['coverage_acceptance'], 'transition')
        self.assertEqual(changed['steps']['pilot-boundary']['argv'][-2:], [new['head'], new['base_at_packaging']])
        self.assertTrue(changed['steps']['pilot-boundary']['git_history'])
        self.assertNotIn('git_history', config['steps']['pilot-boundary'])

    def test_atomic_file_replacement_failure_retains_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / 'host.json'
            file.write_bytes(b'original')
            with patch.object(UPGRADE, 'owned', side_effect=lambda p, private=False: Path(p)), \
                 patch.object(UPGRADE.os, 'replace', side_effect=PermissionError('fixture failure')):
                with self.assertRaises(PermissionError):
                    UPGRADE.replace(file, b'new', 0o600)
            self.assertEqual(file.read_bytes(), b'original')
            self.assertEqual([p.name for p in Path(directory).iterdir()], ['host.json'])

    def test_partial_retained_publication_cannot_authorize_source_removal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            backup = root / 'backup'
            backup.mkdir(mode=0o700)
            run_id = str(uuid.uuid4())
            publication = root / run_id / 'publication'
            publication.mkdir(parents=True, mode=0o700)
            evidence = {}
            for name, key in (('test_result.json', 'machine_sha256'), ('manifest.json', 'manifest_sha256'),
                              ('artifact-registry.json', 'registry_sha256')):
                (publication / name).write_bytes(b'canonical')
                evidence[key] = digest(b'canonical')
            artifact = publication / 'required.log'
            artifact.write_bytes(b'retained measured evidence')
            journal = Journal(root / 'journal.db')
            self.addCleanup(journal.close)
            journal.event(run_id, 'retired', {})
            run = {'id': run_id, 'remote': 'revoked', 'evidence': evidence}
            def fixture_owned(path, private=False):
                path = Path(path)
                if private and path.stat().st_mode & 0o077:
                    raise ValueError('private fixture path exposed')
                return path
            with patch.object(UPGRADE, 'owned', side_effect=fixture_owned):
                expected = UPGRADE.publication_inventory(publication)
                UPGRADE.write_json(backup / 'publication-inventory.json', expected)
                UPGRADE.retained(journal, run, root, backup)
                artifact.unlink()
                with self.assertRaisesRegex(ValueError, 'incomplete or changed'):
                    UPGRADE.retained(journal, run, root, backup)

    @unittest.skipIf(os.geteuid() == 0, 'requires an actual non-root caller')
    def test_upgrade_cannot_elevate_nonroot_or_touch_missing_inputs(self):
        result = subprocess.run([sys.executable, '-I', str(ROOT / 'tools/trusted_gate/deploy/upgrade-history-pilot.py'),
            '--release', '/nonexistent/archive', '--sha256', '0' * 64, '--installer-sha256', '0' * 64],
            capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertIn('administrator authentication required', json.loads(result.stderr)['reason'])
        self.assertEqual(result.stdout, '')


class Executor:
    def __init__(self):
        self.policy = 'p' * 64
        self.executions = 0
        self.valid = True
        self.fail = False

    def policy_identity(self):
        return self.policy

    def validate_inputs(self, inputs):
        if not self.valid:
            raise Blocked('source changed')

    def execute(self, run):
        self.executions += 1
        if self.fail:
            raise Blocked('Core failed')
        return {'valid_until': 200, 'manifest': 'm' * 64, 'run': run['id']}

    def verify_evidence(self, run, evidence):
        if not self.valid or evidence['run'] != run['id']:
            raise Blocked('evidence changed')

    def confirm_stopped(self, run):
        return None


class Remote:
    app_id = 42

    def __init__(self):
        self.check = None
        self.valid = True
        self.failure = None
        self.successes = 0
        self.creations = 0
        self.merged = False

    def remote(self, run, config):
        if not self.valid:
            raise Blocked('base/head changed')
        return {}

    def find_check(self, run, name):
        return self.check

    def update_check(self, run, name, status, conclusion=None, summary=''):
        if not self.check:
            self.creations += 1
        self.check = {'id': 123, 'external_id': run['id'], 'status': status, 'conclusion': conclusion}
        if conclusion == 'success':
            self.successes += 1
        if self.failure == status:
            self.failure = None
            raise APIError('response lost after server accepted write')
        return 123

    def repo(self, method, path, body=None):
        if '/git/ref/' in path:
            return {'object': {'sha': 'c' * 40}}
        if '/pulls/' in path:
            return {'merged': self.merged, 'merge_commit_sha': 'd' * 40,
                    'head': {'sha': 'a' * 40}, 'base': {'repo': {'full_name': 'owner/project'}}}
        return {'sha': 'd' * 40, 'tree': {'sha': 'b' * 40}, 'parents': [{'sha': 'c' * 40}]}


class ControlTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'journal.db'
        self.journal = Journal(self.path)
        self.addCleanup(lambda: self.journal.close())
        self.executor = Executor()
        self.remote = Remote()
        self.now = 100
        self.control = Control(self.journal, self.remote, self.executor,
            {'repository': 'owner/project', 'check_name': 'Trusted Gate', 'base_branch': 'main'},
            clock=lambda: self.now)
        self.inputs = {'repository': 'owner/project', 'head': 'a' * 40,
                       'tree': 'b' * 40, 'base': 'c' * 40, 'policy': self.executor.policy}

    def ready(self):
        run = self.control.validate(self.inputs)
        return self.control.attach(run['id'], 7)

    def test_duplicate_ingress_and_reporting_do_not_rerun_core(self):
        run = self.ready()
        again = self.control.validate(self.inputs)
        self.assertEqual(run['id'], again['id'])
        self.control.publish(run['id'])
        self.control.publish(run['id'])
        self.assertEqual(self.executor.executions, 1)
        self.assertEqual(self.remote.creations, 1)

    def test_unknown_create_response_recovers_the_same_check_after_restart(self):
        run = self.ready()
        self.remote.failure = 'in_progress'
        with self.assertRaises(APIError):
            self.control.publish(run['id'])
        self.assertEqual(self.journal.get(run['id'])['remote'], 'reporting')
        self.journal.close()
        self.journal = Journal(self.path)
        self.control.journal = self.journal
        self.control.publish(run['id'])
        self.assertEqual(self.remote.creations, 1)
        self.assertEqual(self.executor.executions, 1)
        self.assertEqual(self.journal.get(run['id'])['remote'], 'published')

    def test_unknown_success_response_is_retryable_without_revalidation_execution(self):
        run = self.ready()
        self.remote.failure = 'completed'
        with self.assertRaises(APIError):
            self.control.publish(run['id'])
        self.control.publish(run['id'])
        self.assertEqual(self.remote.creations, 1)
        self.assertEqual(self.executor.executions, 1)

    def test_reporting_intent_is_recovered_by_reconciliation(self):
        run = self.ready()
        self.remote.failure = 'in_progress'
        with self.assertRaises(APIError):
            self.control.publish(run['id'])
        self.assertEqual(self.control.reconcile()[0]['remote'], 'published')
        self.assertEqual(self.executor.executions, 1)
        self.assertEqual(self.remote.creations, 1)

    def test_publication_retry_recovers_lost_pr_response_without_second_push_or_pr(self):
        run = self.control.validate(self.inputs)
        self.executor.source = Path(self.tmp.name)
        class Publisher:
            app_id = 9
            def __init__(self):
                self.refs, self.prs, self.posts = [], [], 0
            def installation_token(self):
                return 'secret-fixture-token'
            def repo(self, method, path, body=None):
                if 'matching-refs' in path:
                    return self.refs
                if method == 'GET':
                    return self.prs
                self.posts += 1
                self.prs = [{'number': 7, 'state': 'open', 'head': {'sha': run['head']}}]
                raise APIError('PR accepted; response lost')
        publisher = Publisher()
        pushes = []
        def push(argv, **kwargs):
            pushes.append(argv)
            self.assertNotIn('secret-fixture-token', ' '.join(argv))
            self.assertNotIn('--force', argv)
            publisher.refs = [{'ref': 'refs/heads/gatehouse/' + run['id'], 'object': {'sha': run['head']}}]
            return subprocess.CompletedProcess(argv, 0)
        with patch('gatehouse_trusted_gate.publication.subprocess.run', side_effect=push):
            with self.assertRaises(APIError):
                deliver(self.control, publisher, run['id'], 'test', 'literal $(text)')
            recovered = deliver(self.control, publisher, run['id'], 'test', 'literal $(text)')
        self.assertEqual(recovered['pr'], 7)
        self.assertEqual(publisher.posts, 1)
        self.assertEqual(len(pushes), 1)
        self.assertEqual(self.executor.executions, 1)

    def test_base_change_between_push_and_report_never_gets_success(self):
        run = self.ready()
        self.remote.valid = False
        with self.assertRaises(Blocked):
            self.control.publish(run['id'])
        self.assertEqual(self.remote.successes, 0)
        self.assertEqual(self.journal.get(run['id'])['remote'], 'revoked')

    def test_changed_policy_evidence_and_expiry_revoke_published_green(self):
        for change in ('policy', 'evidence', 'expiry'):
            with self.subTest(change=change):
                run = self.ready()
                self.control.publish(run['id'])
                if change == 'policy':
                    self.executor.policy = 'q' * 64
                elif change == 'evidence':
                    self.executor.valid = False
                else:
                    self.now = 201
                result = self.control.reconcile()
                self.assertEqual(result[0]['remote'], 'revoked')
                self.assertEqual(self.remote.check['conclusion'], 'failure')
                self.executor.policy = self.inputs['policy']
                self.executor.valid = True
                self.now = 100

    def test_revocation_transport_failure_keeps_slot_occupied_until_confirmed(self):
        run = self.ready()
        self.control.publish(run['id'])
        self.remote.failure = 'completed'
        with self.assertRaises(APIError):
            self.control.revoke(run['id'], 'withdrawn')
        self.assertEqual(self.journal.get(run['id'])['remote'], 'revoking')
        with self.assertRaises(Blocked):
            self.control.validate({**self.inputs, 'head': 'f' * 40})
        self.control.reconcile()
        self.assertEqual(self.journal.get(run['id'])['remote'], 'revoked')

    def test_failed_core_cannot_be_attached_or_replayed_as_success(self):
        self.executor.fail = True
        with self.assertRaises(Blocked):
            self.control.validate(self.inputs)
        run = self.journal.latest(self.inputs['repository'], self.inputs['head'])
        with self.assertRaises(Blocked):
            self.control.attach(run['id'], 7)
        self.executor.fail = False
        with self.assertRaises(Blocked):
            self.control.validate(self.inputs)
        self.assertEqual(self.executor.executions, 1)

    def test_running_after_crash_is_never_relaunched_automatically(self):
        run = self.journal.create(self.inputs)
        self.journal.update(run['id'], local='running')
        with self.assertRaises(Blocked):
            self.control.validate(self.inputs)
        self.assertIn('confirm', self.control.reconcile()[0]['blocked'])
        self.assertEqual(self.executor.executions, 0)

    def test_slot_is_held_until_merge_and_landed_tree_confirmed(self):
        run = self.ready()
        self.control.publish(run['id'])
        with self.assertRaises(Blocked):
            self.control.validate({**self.inputs, 'head': 'f' * 40})
        self.remote.merged = True
        self.assertEqual(self.control.reconcile()[0]['remote'], 'merged')
        self.assertNotEqual(run['id'], self.control.validate({**self.inputs, 'head': 'f' * 40})['id'])

    def test_new_epoch_fences_old_revocation(self):
        old = self.ready()
        self.control.revoke(old['id'], 'retry after failure')
        new = self.control.validate(self.inputs)
        self.assertGreater(new['epoch'], old['epoch'])
        with self.assertRaises(Blocked):
            self.control.revoke(old['id'], 'late event')


class APITests(unittest.TestCase):
    def client(self):
        client = object.__new__(GitHub)
        client.app_id = 42
        return client

    def test_wrong_app_same_name_does_not_satisfy_authoritative_check(self):
        client = self.client()
        client.checks = lambda head: [{'name': 'Trusted Gate', 'head_sha': head, 'app': {'id': 9}}]
        self.assertIsNone(client.find_check({'head': 'a' * 40, 'check_id': None}, 'Trusted Gate'))

    def test_duplicate_own_checks_block_recovery(self):
        client = self.client()
        client.checks = lambda head: [{'name': 'Trusted Gate', 'head_sha': head, 'app': {'id': 42}}] * 2
        with self.assertRaises(Blocked):
            client.find_check({'head': 'a' * 40, 'check_id': None}, 'Trusted Gate')

    def test_required_source_and_strict_base_are_mandatory(self):
        inputs = {'repository': 'owner/project', 'repository_id': 10, 'head': 'a' * 40, 'tree': 'b' * 40, 'base': 'c' * 40}
        run = {'input': inputs, 'pr': 7}
        pr = {'state': 'open', 'draft': False, 'merged': False,
              'head': {'repo': {'full_name': 'owner/project'}, 'sha': inputs['head']},
              'base': {'repo': {'full_name': 'owner/project'}, 'sha': inputs['base'], 'ref': 'main'}}
        for app, strict in ((None, True), (9, True), (42, False), (42, True)):
            with self.subTest(app=app, strict=strict):
                client = self.client()
                responses = [{'id': 10}, {'sha': inputs['head'], 'tree': {'sha': inputs['tree']}}, pr,
                             {'object': {'sha': inputs['base']}},
                             [{'type': 'required_status_checks', 'parameters': {
                                 'strict_required_status_checks_policy': strict,
                                 'required_status_checks': [{'context': 'Trusted Gate', 'integration_id': app}]}}]]
                client.repo = lambda *args: responses.pop(0)
                if app == 42 and strict:
                    self.assertEqual(client.remote(run, {'repository_id': 10, 'base_branch': 'main', 'check_name': 'Trusted Gate'}), pr)
                else:
                    with self.assertRaises(Blocked):
                        client.remote(run, {'repository_id': 10, 'base_branch': 'main', 'check_name': 'Trusted Gate'})

    def test_neutral_skipped_or_direct_green_creation_are_rejected(self):
        client = self.client()
        client.find_check = lambda *args: None
        for conclusion in ('neutral', 'skipped', 'success'):
            with self.subTest(conclusion=conclusion), self.assertRaises(Blocked):
                client.update_check({'id': 'run', 'head': 'a' * 40}, 'Trusted Gate', 'completed', conclusion)


class FileTests(unittest.TestCase):
    def test_delegated_file_reads_cannot_publish_root_files_or_special_entries(self):
        with tempfile.TemporaryDirectory() as directory:
            inbox = Path(directory) / 'inbox'
            inbox.mkdir()
            body = inbox / 'body.txt'
            body.write_bytes(b'reviewed body')
            outside = Path(directory) / 'private-key'
            outside.write_bytes(b'never publish')
            link = inbox / 'link'
            link.symlink_to(outside)
            fifo = inbox / 'fifo'
            os.mkfifo(fifo)
            # Ownership of the deployed parent/inbox is tested separately. These are real
            # file-descriptor reads; no mocked read can hide a symlink, hardlink or FIFO.
            with patch('gatehouse_trusted_gate.common.submission_inbox', return_value=inbox):
                self.assertEqual(read_submission(body, inbox, os.getuid(), 100), b'reviewed body')
                for path, uid, limit in ((outside, os.getuid(), 100), (link, os.getuid(), 100),
                        (fifo, os.getuid(), 100), (body, os.getuid() + 1, 100), (body, os.getuid(), 1),
                        (inbox / '..' / 'private-key', os.getuid(), 100)):
                    with self.subTest(path=path, uid=uid, limit=limit), self.assertRaises(Blocked):
                        read_submission(path, inbox, uid, limit)
                os.link(body, inbox / 'hardlink')
                with self.assertRaises(Blocked):
                    read_submission(body, inbox, os.getuid(), 100)

    def test_inbox_requires_protected_parent_root_sticky_and_no_world_access(self):
        inbox = Path('/var/spool/gatehouse/inbox')
        with patch('gatehouse_trusted_gate.common.protected') as protected_parent:
            for owner, mode in ((0, stat.S_IFDIR | 0o1730), (1000, stat.S_IFDIR | 0o1730),
                    (0, stat.S_IFDIR | 0o0770), (0, stat.S_IFDIR | 0o1777),
                    (0, stat.S_IFLNK | 0o1730)):
                with self.subTest(owner=owner, mode=mode), patch.object(Path, 'lstat',
                        return_value=SimpleNamespace(st_uid=owner, st_mode=mode)):
                    if owner == 0 and mode == stat.S_IFDIR | 0o1730:
                        self.assertEqual(submission_inbox(inbox), inbox)
                    else:
                        with self.assertRaises(Blocked):
                            submission_inbox(inbox)
            protected_parent.assert_called_with(inbox.parent, directory=True)

    def test_public_launcher_rejects_internal_actions_and_discards_caller_environment(self):
        launcher = ROOT / 'tools/trusted_gate/deploy/gatehouse-control'
        with patch('os.geteuid', return_value=0), patch('os.execve', side_effect=SystemExit(77)) as execute:
            for action in ('step', 'retire', 'revoke', 'maintain-cache', '/bin/sh'):
                with self.subTest(action=action), patch.object(sys, 'argv', [str(launcher), action]), self.assertRaises(SystemExit):
                    runpy.run_path(str(launcher), run_name='__main__')
            execute.assert_not_called()
            with patch.dict(os.environ, {'PYTHONPATH': '/agent', 'GITHUB_TOKEN': 'must-not-forward'}), \
                 patch.object(sys, 'argv', [str(launcher), 'status']), self.assertRaises(SystemExit) as exit_status:
                runpy.run_path(str(launcher), run_name='__main__')
            self.assertEqual(exit_status.exception.code, 77)
            program, argv, environment = execute.call_args.args
            self.assertEqual(program, '/usr/bin/python3')
            self.assertEqual(argv, ['/usr/bin/python3', '-I', '/opt/gatehouse/trusted_gate/host.py', 'status'])
            self.assertEqual(environment, {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8'})

    def test_duplicate_json_symlink_and_oversize_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'input'
            path.write_text('{"pass":true,"pass":false}')
            with self.assertRaises(Blocked):
                read_json(path)
            with self.assertRaises(Blocked):
                read_regular(path, 1)
            link = Path(directory) / 'link'
            link.symlink_to(path)
            with self.assertRaises(Blocked):
                read_regular(link, 100)


class CacheTests(unittest.TestCase):
    def dimensions(self):
        return {name: 'false' if name == 'release_incremental' else name for name in DIMENSIONS}

    def test_toolchain_platform_dependency_profile_and_flags_invalidate_namespace(self):
        dimensions = self.dimensions()
        original = identity(dimensions)
        for key in dimensions:
            with self.subTest(key=key):
                value = 'true' if key == 'release_incremental' else 'different'
                self.assertNotEqual(identity({**dimensions, key: value}), original)
        with self.assertRaises(Blocked):
            identity({key: value for key, value in dimensions.items() if key != 'toolchain'})

    def test_same_compatible_namespace_preserves_build_bytes_but_no_gate_result(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Cache(directory, 100, 3, 60)
            first = cache.directory(self.dimensions())
            (first / 'object').write_bytes(b'compiled')
            second = cache.directory(self.dimensions())
            self.assertEqual(first, second)
            self.assertEqual((second / 'object').read_bytes(), b'compiled')

    def test_capacity_cleanup_preserves_active_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Cache(directory, 10, 1, 60)
            old = cache.directory(self.dimensions())
            (old / 'object').write_bytes(b'123456789')
            active = cache.directory({**self.dimensions(), 'profile': 'test'})
            (active / 'object').write_bytes(b'12345')
            summary = cache.maintain(active=(active.name,))
            self.assertEqual(summary['removed'], [old.name])
            self.assertTrue(active.is_dir())

    def test_unknown_or_symlinked_cache_never_deleted(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Cache(directory, 1, 1, 1)
            (Path(directory) / 'unknown').mkdir()
            with self.assertRaises(Blocked):
                cache.maintain(now=9999999999)
            self.assertTrue((Path(directory) / 'unknown').exists())

    def test_actual_lock_configuration_and_toolchain_bytes_control_compatibility(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / 'Cargo.lock').write_text('lock v1')
            (source / 'Cargo.toml').write_text('configuration v1')
            tool = source / 'compiler'
            tool.write_bytes(b'compiler v1')
            step = {'cache_identity': self.dimensions(), 'argv': [str(tool), 'build', '--release'],
                    'env': {'CARGO_PROFILE_RELEASE_INCREMENTAL': 'false'},
                    'toolchain_files': [str(tool)], 'cache_inputs': {
                        'dependencies': ['Cargo.lock'], 'configuration': ['Cargo.toml', '.cargo/config.toml']}}
            with patch('gatehouse_trusted_gate.cache.protected', side_effect=lambda p, **kw: Path(p)):
                def key():
                    return identity(resolved_dimensions(source, step, 'owner/project', 'worker:policy'))
                initial = key()
                (source / 'lib.rs').write_text('ordinary source change')
                self.assertEqual(initial, key())
                (source / 'Cargo.lock').write_text('lock v2')
                lock = key()
                self.assertNotEqual(initial, lock)
                (source / '.cargo').mkdir()
                (source / '.cargo/config.toml').write_text('new configuration')
                configuration = key()
                self.assertNotEqual(lock, configuration)
                tool.write_bytes(b'compiler v2')
                self.assertNotEqual(configuration, key())

    def test_release_incremental_setting_must_match_the_executed_environment(self):
        with tempfile.TemporaryDirectory() as directory:
            tool = Path(directory) / 'compiler'
            tool.write_bytes(b'compiler')
            step = {'cache_identity': self.dimensions(), 'argv': [str(tool)], 'env': {},
                    'toolchain_files': [str(tool)], 'cache_inputs': {'dependencies': [], 'configuration': []}}
            with patch('gatehouse_trusted_gate.cache.protected', side_effect=lambda p, **kw: Path(p)):
                with self.assertRaises(Blocked):
                    resolved_dimensions(directory, step, 'owner/project', 'worker')
                step['env'] = {'CARGO_PROFILE_RELEASE_INCREMENTAL': 'false', 'CARGO_INCREMENTAL': '1'}
                with self.assertRaises(Blocked):
                    resolved_dimensions(directory, step, 'owner/project', 'worker')


class IsolationTests(unittest.TestCase):
    def config(self):
        return {'steps': {'tests': {'argv': ['/usr/bin/python3', '-m', 'unittest'], 'timeout_seconds': 60}},
                'executor_user': 'gatehouse-executor', 'source': '/srv/gatehouse/source',
                'records': '/var/lib/gatehouse/control', 'executor_memory_bytes': 1024**3,
                'executor_file_bytes': 1024**3, 'protected_files': {'/usr/bin/python3': 'x'}}

    def test_worker_hides_control_state_and_credentials_and_is_bound_to_core_cgroup(self):
        args = sandbox_arguments(self.config(), {'id': 'run-id'}, 'tests', '/scratch', '/cache')
        for value in ('--property=NoNewPrivileges=yes', '--property=PrivateNetwork=yes',
                      '--property=User=gatehouse-executor', '--property=ProtectHome=yes',
                      '--property=CapabilityBoundingSet=', '--property=KillMode=control-group',
                      '--property=BindsTo=gatehouse-core-run-id.service'):
            self.assertIn(value, args)
        self.assertTrue(any('/etc/gatehouse' in value and '/var/lib/gatehouse/control' in value for value in args))
        self.assertEqual(args[-4:], ['--', '/usr/bin/python3', '-m', 'unittest'])

    def test_unpinned_executable_or_loader_override_is_rejected(self):
        config = self.config()
        config['steps']['tests']['env'] = {'LD_PRELOAD': '/work/source/inject.so'}
        with self.assertRaises(Blocked):
            sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache')
        config = self.config()
        config['steps']['tests']['argv'][0] = '/work/source/fake-core'
        with self.assertRaises(Blocked):
            sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache')

    def test_agent_environment_is_not_forwarded(self):
        with patch.dict('os.environ', {'GH_TOKEN': 'secret', 'PYTHONPATH': '/agent', 'RUSTC_WRAPPER': '/agent/compiler'}):
            environment = clean_environment()
        self.assertFalse({'GH_TOKEN', 'PYTHONPATH', 'RUSTC_WRAPPER'} & environment.keys())

    def test_reviewed_history_is_readonly_and_cannot_be_retargeted_by_step_environment(self):
        config = self.config()
        config['steps']['tests']['git_history'] = True
        with self.assertRaisesRegex(Blocked, 'history view'):
            sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache')
        args = sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache', '/private/view')
        self.assertTrue(any('/private/view:/work/history' in arg and 'BindReadOnlyPaths=' in arg for arg in args))
        self.assertTrue(any('/private/git-pointer:/work/source/.git' in arg for arg in args))
        self.assertFalse(any(arg.startswith(('--setenv=GIT_DIR=', '--setenv=GIT_WORK_TREE=')) for arg in args))
        self.assertIn('--setenv=GIT_OPTIONAL_LOCKS=0', args)
        self.assertTrue(any('/work/source/.gatehouse-host' in arg and 'InaccessiblePaths=' in arg for arg in args))
        for name in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_CONFIG_COUNT', 'GIT_CONFIG_SYSTEM'):
            config['steps']['tests']['env'] = {name: '/untrusted'}
            with self.subTest(name=name), self.assertRaisesRegex(Blocked, 'Git input/configuration'):
                sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache', '/private/view')

    def test_privileged_developer_and_executor_groups_are_rejected(self):
        account = SimpleNamespace(pw_uid=1001, pw_gid=1001, pw_name='developer')
        for name in ('docker', 'lxd', 'sudo', 'disk', 'users'):
            group = SimpleNamespace(gr_name=name, gr_mem=['developer'])
            with self.subTest(group=name), \
                 patch('gatehouse_trusted_gate.isolation.pwd.getpwuid', return_value=account), \
                 patch('gatehouse_trusted_gate.isolation.grp.getgrall', return_value=[group]), \
                 patch('gatehouse_trusted_gate.isolation.grp.getgrgid', return_value=SimpleNamespace(gr_name='users')):
                if name == 'users':
                    self.assertEqual(unprivileged_identity(1001), account)
                else:
                    with self.assertRaises(Blocked):
                        unprivileged_identity(1001)


class WorkerHistoryTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        self.addCleanup(temporary.cleanup)
        def writable():
            for directory, _, files in os.walk(root):
                os.chmod(directory, 0o755)
                for name in files:
                    os.chmod(Path(directory) / name, 0o644)
        self.addCleanup(writable)
        source = root / 'source'
        source.mkdir()
        def git(*args):
            return subprocess.check_output(['git', '-c', 'core.hooksPath=/dev/null', *args],
                cwd=source, env=clean_environment(), stderr=subprocess.DEVNULL, text=True).strip()
        git('init', '-q', '-b', 'main')
        (source / 'code.txt').write_text('baseline\n')
        git('add', 'code.txt')
        git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'base')
        base = git('rev-parse', 'HEAD')
        (source / 'code.txt').write_text('candidate\n')
        git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qam', 'candidate')
        head, tree = git('rev-parse', 'HEAD'), git('rev-parse', 'HEAD^{tree}')
        git('config', 'remote.origin.url', 'https://private-fixture.invalid/repository')
        git('config', 'http.proxy', 'http://private-fixture.invalid:1234')
        git('config', 'http.extraheader', 'AUTHORIZATION: fixture-header')
        git('config', 'credential.helper', 'fixture-helper')
        records = root / 'records'
        run_id = str(uuid.uuid4())
        (records / run_id).mkdir(parents=True)
        return ({'source': str(source), 'records': str(records)},
                {'id': run_id, 'head': head, 'input': {'head': head, 'base': base, 'tree': tree}})

    def test_real_history_supports_clean_status_base_diff_and_archive_without_controller_metadata(self):
        config, run = self.fixture()
        with patch('gatehouse_trusted_gate.history.protected', side_effect=lambda p, **kw: Path(p)):
            view = prepare_history(config, run)
            self.assertEqual(view, prepare_history(config, run))
        settings = (view / 'config').read_text()
        self.assertFalse(any(name in settings for name in ('private-fixture', 'fixture-header', 'credential', 'remote')))
        self.assertFalse((view / 'FETCH_HEAD').exists())
        self.assertFalse((view / 'objects/info/alternates').exists())
        source = Path(config['source'])
        original = source / '.git'
        hidden = source / '.hidden-controller-git'
        original.rename(hidden)
        original.write_text('gitdir: ' + str(view) + '\n')
        env = {**clean_environment(), 'GIT_OPTIONAL_LOCKS': '0'}
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=source, env=env).decode()
        self.assertEqual(git('rev-parse', 'HEAD').strip(), run['head'])
        self.assertEqual(git('show', run['input']['base'] + ':code.txt'), 'baseline\n')
        self.assertIn('+candidate', git('diff', run['input']['base'], run['head']))
        self.assertEqual(git('status', '--porcelain', '--untracked-files=no'), '')
        nested = source / 'test-fixture'
        nested.mkdir()
        subprocess.run(['git', 'init', '-q', '-b', 'fixture'], cwd=nested, env=env, check=True)
        fixture_dir = subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=nested, env=env, text=True).strip()
        self.assertEqual(fixture_dir, str(nested / '.git'))
        archive = subprocess.check_output(['git', 'archive', run['head']], cwd=source, env=env)
        with tarfile.open(fileobj=io.BytesIO(archive)) as stream:
            self.assertEqual(stream.extractfile('code.txt').read(), b'candidate\n')
        for directory, _, files in os.walk(view):
            self.assertEqual(stat.S_IMODE(Path(directory).stat().st_mode), 0o555)
            for name in files:
                self.assertEqual(stat.S_IMODE((Path(directory) / name).stat().st_mode), 0o444)
        with self.assertRaises(PermissionError):
            (view / 'config').open('ab')

    def test_retained_view_tamper_and_changed_baseline_are_blocked(self):
        config, run = self.fixture()
        with patch('gatehouse_trusted_gate.history.protected', side_effect=lambda p, **kw: Path(p)):
            view = prepare_history(config, run)
            changed = copy.deepcopy(run)
            changed['input']['base'] = run['head']
            with self.assertRaisesRegex(Blocked, 'retained worker Git view changed'):
                prepare_history(config, changed)
            (view / 'config').chmod(0o644)
            with (view / 'config').open('a') as stream:
                stream.write('\n[credential]\n helper = unsafe\n')
            with self.assertRaisesRegex(Blocked, 'retained worker Git view changed'):
                prepare_history(config, run)

    def test_live_probe_reads_exact_history_and_preserves_fixture_discovery(self):
        config, run = self.fixture()
        with patch('gatehouse_trusted_gate.history.protected', side_effect=lambda p, **kw: Path(p)):
            view = prepare_history(config, run)
        source = Path(config['source'])
        (source / '.git').rename(source / '.hidden-controller-git')
        (source / '.git').write_text('gitdir: ' + str(view) + '\n')
        with patch.dict(os.environ, clean_environment(), clear=True):
            report = PROBE.history_probe(run['head'], run['input']['base'], source, view)
            self.assertTrue(report['clean'])
            self.assertTrue(report['nested_fixture_identity'])
            self.assertGreater(report['archive_bytes'], 0)
            self.assertEqual(report['head'], run['head'])
            with self.assertRaisesRegex(ValueError, 'refs differ'):
                PROBE.history_probe('a' * 40, run['input']['base'], source, view)
            with patch.dict(os.environ, {'GIT_DIR': str(view)}):
                with self.assertRaisesRegex(ValueError, 'global Git retargeting'):
                    PROBE.history_probe(run['head'], run['input']['base'], source, view)

    def test_dirty_source_and_interrupted_export_are_retained_and_blocked(self):
        config, run = self.fixture()
        source = Path(config['source'])
        view = Path(config['records']) / run['id'] / 'git-history'
        with patch('gatehouse_trusted_gate.history.protected', side_effect=lambda p, **kw: Path(p)):
            (source / 'code.txt').write_text('changed after freeze\n')
            with self.assertRaisesRegex(Blocked, 'source bytes changed'):
                prepare_history(config, run)
            self.assertFalse(view.exists())
            (source / 'code.txt').write_text('candidate\n')
            view.mkdir()
            (view / 'interrupted').write_text('retain original partial export')
            with self.assertRaises(Blocked):
                prepare_history(config, run)
            self.assertEqual((view / 'interrupted').read_text(), 'retain original partial export')


class SourceTests(unittest.TestCase):
    def test_baseline_fetch_timeout_blocks_without_exposing_transport_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mirror = root / 'mirror.git'
            mirror.mkdir()
            config = {'source': str(root / 'source'), 'mirror': str(mirror),
                      'repository': 'owner/project', 'repository_id': 10,
                      'base_branch': 'main', 'max_bundle_bytes': 1024}
            reader = SimpleNamespace(
                repo=lambda method, path: {'object': {'sha': 'b' * 40}} if path else {'id': 10},
                installation_token=lambda: 'private-fixture-token')
            journal = SimpleNamespace(outstanding=lambda: [])
            timeout = subprocess.TimeoutExpired(['git', 'fetch'], 120,
                output=b'private-fixture-token', stderr=b'private transport response')
            with patch('gatehouse_trusted_gate.source.protected', side_effect=lambda p, **kw: Path(p)), \
                 patch('gatehouse_trusted_gate.source.subprocess.run', side_effect=timeout):
                with self.assertRaisesRegex(Blocked, '^independent baseline fetch timed out; reconcile the administrator-managed Git network route$') as rejected:
                    prepare(config, reader, journal, b'bundle', 'a' * 40)
            self.assertNotIn('private-fixture-token', str(rejected.exception))
            self.assertIsNone(rejected.exception.__cause__)
            self.assertTrue((root / 'frozen.json').is_file())
            self.assertFalse((root / 'source').exists())

    def test_old_terminal_epoch_cannot_retire_source_owned_by_active_batch(self):
        with tempfile.TemporaryDirectory() as directory:
            journal = Journal(Path(directory) / 'journal.db')
            self.addCleanup(journal.close)
            executor = Executor()
            control = Control(journal, Remote(), executor,
                {'repository': 'owner/project', 'check_name': 'Trusted Gate'}, clock=lambda: 100)
            inputs = {'repository': 'owner/project', 'head': 'a' * 40,
                      'tree': 'b' * 40, 'base': 'c' * 40, 'policy': executor.policy}
            old = control.validate(inputs)
            control.revoke(old['id'], 'retired attempt')
            current = control.validate(inputs)
            with self.assertRaisesRegex(Blocked, 'later active epoch'):
                retire({}, executor, journal, old['id'])
            self.assertEqual(journal.outstanding()[0]['id'], current['id'])
            control.revoke(current['id'], 'newer terminal attempt')
            with self.assertRaisesRegex(Blocked, 'later epoch exists'):
                retire({}, executor, journal, old['id'])

    def test_real_bundle_fixed_input_and_interrupted_preparation_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / 'candidate'
            candidate.mkdir()
            def git(*args):
                return subprocess.run(['git', '-c', 'core.hooksPath=/dev/null', *args],
                    cwd=candidate, check=True, capture_output=True, text=True).stdout.strip()
            git('init', '-q', '-b', 'main')
            (candidate / 'code.txt').write_text('baseline\n')
            git('add', 'code.txt')
            git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'base')
            base = git('rev-parse', 'HEAD')
            (candidate / 'code.txt').write_text('candidate\n')
            git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qam', 'candidate')
            head = git('rev-parse', 'HEAD')
            bundle = root / 'input.bundle'
            git('bundle', 'create', str(bundle), 'HEAD')
            mirror = root / 'mirror.git'
            git('clone', '--bare', str(candidate), str(mirror))
            config = {'source': str(root / 'source'), 'mirror': str(mirror), 'repository': 'owner/project',
                      'repository_id': 10, 'base_branch': 'main', 'max_bundle_bytes': 1024 * 1024,
                      'max_source_files': 100, 'max_source_bytes': 1024 * 1024}
            class Reader:
                def repo(self, method, path):
                    return {'object': {'sha': base}} if path else {'id': 10}
                def installation_token(self):
                    return 'fixture-token'
            class EmptyJournal:
                def outstanding(self):
                    return []
            actual_run = subprocess.run
            fetches = []
            def run(argv, **kwargs):
                if 'https://github.com/owner/project.git' in argv:
                    fetches.append(argv)
                    self.assertNotIn('fixture-token', ' '.join(argv))
                    return subprocess.CompletedProcess(argv, 0, stdout=b'', stderr=b'')
                return actual_run(argv, **kwargs)
            with patch('gatehouse_trusted_gate.source.protected', side_effect=lambda p, **kw: Path(p)), \
                 patch('gatehouse_trusted_gate.source.subprocess.run', side_effect=run):
                inputs = prepare(config, Reader(), EmptyJournal(), bundle.read_bytes(), head)
                repeated = prepare(config, Reader(), EmptyJournal(), bundle.read_bytes(), head)
            self.assertEqual(inputs, repeated)
            self.assertEqual(inputs['head'], head)
            self.assertEqual(inputs['base'], base)
            self.assertEqual((root / 'source/code.txt').read_text(), 'candidate\n')
            self.assertEqual(len(fetches), 1)


class PublicationEvidenceTests(unittest.TestCase):
    def fixture(self, root, mutation=None):
        source = root / 'source'
        directory = source / '.gatehouse-host/reports/invocations/inv-fixture'
        directory.mkdir(parents=True)
        metadata = {'invocation_id': 'inv-fixture', 'commit': 'a' * 40, 'profile': 'full',
                    'input_mode': 'all', 'source_identity': 'working-tree:' + 'a' * 40,
                    'execution_root': str(source), 'configuration_digest': 'sha256:' + 'd' * 64}
        machine = {**metadata, 'schema_version': '1', 'evidence_complete': True,
                   'status': 'PASS', 'passed': True, 'failures': [], 'skipped_steps': [],
                   'steps': [{'step_id': 'unit', 'status': 'PASS', 'passed': True}]}
        if mutation:
            mutation(machine, metadata)
        data = canonical(metadata)
        (directory / 'invocation.json').write_bytes(data)
        artifact = {'path': 'invocation.json', 'kind': 'invocation-metadata',
                    'size_bytes': len(data), 'sha256': digest(data)}
        machine['artifacts'] = [{**artifact, 'invocation_id': 'inv-fixture'}]
        registry = {'schema_version': '1', 'invocation_id': 'inv-fixture', 'artifacts': machine['artifacts']}
        manifest = {'schema_version': '1', 'invocation_id': 'inv-fixture', 'artifacts': [artifact]}
        for name, value in (('test_result.json', machine), ('manifest.json', manifest), ('artifact-registry.json', registry)):
            (directory / name).write_bytes(canonical(value))
        evidence = {'directory': str(directory), 'run': 'run',
                    'machine_sha256': digest((directory / 'test_result.json').read_bytes()),
                    'manifest_sha256': digest((directory / 'manifest.json').read_bytes()),
                    'registry_sha256': digest((directory / 'artifact-registry.json').read_bytes())}
        executor = object.__new__(HostExecutor)
        executor.source, executor.config = source, {'coverage': ['unit']}
        return executor, directory, evidence

    def verify(self, executor, evidence):
        # This fixture tests evidence semantics, not deployed root ownership or isolation.
        with patch('gatehouse_trusted_gate.execution.protected', side_effect=lambda p, **kw: Path(p)):
            executor.verify_evidence({'id': 'run', 'head': 'a' * 40}, evidence)

    def test_complete_core_publication_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            executor, _, evidence = self.fixture(Path(directory))
            self.verify(executor, evidence)

    def test_missing_partial_skipped_failed_waived_or_wrong_input_never_authorize(self):
        mutations = [
            lambda m, i: m.update(evidence_complete=False),
            lambda m, i: m.update(status='FAIL'),
            lambda m, i: m.update(steps=[]),
            lambda m, i: m.update(profile='hook'),
            lambda m, i: m.update(skipped_steps=[{'id': 'unit'}]),
            lambda m, i: m['steps'][0].update(waived=True),
            lambda m, i: m['steps'][0].update(status='SKIPPED'),
            lambda m, i: i.update(commit='b' * 40),
            lambda m, i: m.update(configuration_digest='different'),
            lambda m, i: m.update(input_mode='staged'),
        ]
        for number, mutation in enumerate(mutations):
            with self.subTest(number=number), tempfile.TemporaryDirectory() as directory:
                executor, _, evidence = self.fixture(Path(directory), mutation)
                with self.assertRaises(Blocked):
                    self.verify(executor, evidence)

    def test_tampered_controls_artifact_and_extra_file_are_rejected(self):
        for name in ('test_result.json', 'manifest.json', 'artifact-registry.json', 'invocation.json', 'extra.log'):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                executor, directory, evidence = self.fixture(Path(temporary))
                (directory / name).write_bytes(b'changed')
                with self.assertRaises(Blocked):
                    self.verify(executor, evidence)


class PilotPolicyProfileTests(unittest.TestCase):
    def test_default_and_hook_profiles_are_declared_by_the_step_inventory(self):
        flow = tomllib.loads((ROOT / 'tools/trusted_gate/deploy/pilot.flow.toml').read_text(encoding='utf-8'))
        declared = {profile for step in flow['steps'] for profile in step['profiles']}
        self.assertIn(flow['project']['default_profile'], declared)
        self.assertIn(flow['project']['hook_profile'], declared)
        self.assertIn('full', declared)
        self.assertIn('hook', declared)


if __name__ == '__main__':
    unittest.main()
