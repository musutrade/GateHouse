"""Actual retained trees/WAL and source-loaded old Check lookup, with mocked API.

Root ownership, accepted-stage/configuration authentication and worker admission
are replaced inside private fixtures. No actual remote or installation acceptance.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import ssl
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX = load('effects_preservation_fixture', 'tools/quality/tests/test_trusted_gate_preservation.py')
J = load('effects_journal', 'tools/trusted_gate/deploy/admit-upgrade-journal.py')
E = load('effects_library', 'tools/trusted_gate/deploy/admit-upgrade-effects.py')
P = load('effects_preflight', 'tools/trusted_gate/deploy/preflight-history-pilot.py')
FENCE = load('effects_lease', 'tools/trusted_gate/deploy/install-transaction.py')
KEEP = FIX.KEEP;BOOT = FIX.BOOT


class UpgradeEffectsTests(unittest.TestCase):
    def setUp(self):self.fixture()

    def fixture(self, registered=True):
        self.f = FIX.PreservationTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        f = self.f;config = json.loads(f.config_path.read_bytes())
        config.update(repository=P.REPOSITORY, repository_id=P.REPOSITORY_ID,
            app_id=4867361, installation_id=159921491, app_key=str(f.root/'checks.pem'),
            check_name='GateHouse Trusted Verification')
        f.dump(f.config_path, config);self.old_raw=f.config_path.read_bytes();self.config=config
        inputs=json.loads(f.writer.execute('SELECT input FROM runs').fetchone()[0])
        inputs.update(repository=P.REPOSITORY,repository_id=P.REPOSITORY_ID,head=f.head,
                      policy=E.digest(BOOT.canonical(config)[:-1]))
        f.writer.execute('UPDATE runs SET input=?',(BOOT.canonical(inputs)[:-1].decode(),))
        self.invocation=f.source/'.gatehouse-host/reports/invocations/registered-invocation'
        self.evidence=None
        if registered:
            self.invocation.mkdir(mode=0o700)
            for name,value in (('test_result.json',{'status':'FAIL','invocation_id':self.invocation.name}),
                               ('manifest.json',{'artifacts':[]}),('artifact-registry.json',{'artifacts':[]}),
                               ('invocation.json',{'commit':f.head})):
                BOOT.write(self.invocation/name,BOOT.canonical(value),0o600)
            (self.invocation/'isolation').mkdir(mode=0o700)
            (self.invocation/'empty').mkdir(mode=0o700)
            BOOT.write(self.invocation/'isolation/raw.stderr',b'private retained diagnostic',0o600)
            BOOT.write(f.control/KEEP.RUN/'configuration.json',self.old_raw,0o600)
            self.evidence={'directory':str(self.invocation),'run':KEEP.RUN,
                'configuration_sha256':E.digest(self.old_raw),**{key:E.digest((self.invocation/name).read_bytes())
                for name,key in (('test_result.json','machine_sha256'),('manifest.json','manifest_sha256'),
                                 ('artifact-registry.json','registry_sha256'))}}
            f.writer.execute('UPDATE runs SET evidence=?,local=?',(BOOT.canonical(self.evidence)[:-1].decode(),'passed'))
        f.preserve();self.pin=E.digest((f.destination/'preservation.json').read_bytes())
        self.destination=f.control/KEEP.RUN/'publication'
        if registered:shutil.copytree(self.invocation,self.destination)
        self.B=SimpleNamespace(**vars(f.B));self.B.authenticate=Mock(return_value={'revision':'a'*40})
        self.builder=SimpleNamespace(worker_identity=Mock(),quiet_units=Mock(),idle=Mock())
        self.compose=SimpleNamespace(retained_configuration=Mock(side_effect=lambda *args:self.old_raw))
        payload={f'trusted_gate/{name}.py':subprocess.check_output(['/usr/bin/git','show',
            '659803a2f144926997e56d072c7fe8760d6ed176:tools/trusted_gate/'+name+'.py'],cwd=ROOT)
            for name in ('common','github')}
        cls=P.client_class(payload);self.client=cls.__new__(cls)
        self.client.app_id=config['app_id'];self.client.token='private-test-token';self.client.expires=123
        self.client.opener=E.urllib.request.build_opener(cls.__init__.__globals__['NoRedirect'](),
                                                      E.urllib.request.ProxyHandler({}))
        self.checks=[];self.remote_calls=0
        self.client.repo=Mock(side_effect=self.api)
        self.client_factory=Mock(return_value=self.client)
        self.preflight=SimpleNamespace(REPOSITORY=P.REPOSITORY,REPOSITORY_ID=P.REPOSITORY_ID,
            NEGATIVE_CHECK=P.NEGATIVE_CHECK,accepted_release=Mock(return_value=payload),
            client_class=Mock(return_value=self.client_factory))
        self.B.load=lambda helpers,name:{'preserve-pilot-state.py':KEEP,'admit-upgrade-journal.py':J,
            'install-transaction.py':FENCE,'build-protected-core.py':self.builder,
            'bootstrap-release.py':SimpleNamespace(BASE=f.head),'preflight-history-pilot.py':self.preflight,
            'compose-linux-host.py':self.compose}[name]
        self.update(local='invalid',remote='revoking',reason=J.REASON);self.update(remote='revoked')

    def event(self, operation, detail):
        sequence=self.f.writer.execute('SELECT max(sequence) FROM events').fetchone()[0]+1
        self.f.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)',
            (sequence,KEEP.RUN,float(sequence),operation,BOOT.canonical(detail)[:-1].decode()))

    def update(self, **fields):
        self.f.writer.execute('UPDATE runs SET '+','.join(key+'=?' for key in fields)+' WHERE id=?',
                              (*fields.values(),KEEP.RUN));self.event('update',fields)

    def check(self, **changes):
        return dict(id=81234,name=self.config['check_name'],app={'id':4867361},head_sha=self.f.head,
                    external_id=KEEP.RUN,status='completed',conclusion='failure',**changes)

    def api(self, method, path):
        self.assertEqual(method,'GET');self.remote_calls+=1
        if path=='':return {'id':P.REPOSITORY_ID,'full_name':P.REPOSITORY,'default_branch':'main'}
        if path=='/git/ref/heads/main':return {'ref':'refs/heads/main','object':{'type':'commit','sha':self.f.head}}
        if path=='/git/commits/'+self.f.head:
            inputs=json.loads(self.f.writer.execute('SELECT input FROM runs').fetchone()[0])
            return {'sha':self.f.head,'tree':{'sha':inputs['tree']}}
        if path.startswith('/commits/'+self.f.head+'/check-runs?'):
            return {'check_runs':copy.deepcopy(self.checks)}
        raise AssertionError('unexpected API read '+path)

    def observe(self, **kwargs):
        f=self.f
        with f.domain(),patch.multiple(J,CONTROL=f.control,PILOT=f.volume,BASE=f.head), \
             patch.object(E,'PILOT',f.volume),patch.object(BOOT,'owned',side_effect=f.owned), \
             patch.object(E.os,'geteuid',return_value=0),patch.object(FENCE,'LEASE',f.control/'controller.lock'):
            with (f.control/'controller.lock').open('r+b') as lease:
                return E.observe(self.B,f.root/'helpers',f.root/'manifest','a'*64,f.destination,
                                 kwargs.pop('pin',self.pin),kwargs.pop('lock_fd',lease.fileno()),**kwargs)

    def physical(self):
        paths=[self.f.journal_path,self.f.journal_path.with_name('journal.sqlite3-wal'),
               self.f.destination/'journal.sqlite3',self.f.destination/'preservation.json',self.f.raw_report]
        return {str(path):path.read_bytes() for path in paths if path.exists()}

    def publication_only(self, *, evidence=None, saved=None, policy=None):
        saved=copy.deepcopy(saved if saved is not None else json.loads((self.f.destination/'preservation.json').read_bytes()))
        row=self.f.writer.execute('SELECT * FROM runs').fetchone();run=dict(zip(KEEP.TABLES['runs'],row))
        run['evidence']=BOOT.canonical(self.evidence if evidence is None else evidence)[:-1].decode()
        if policy is not None:
            inputs=json.loads(run['input']);inputs['policy']=policy;run['input']=BOOT.canonical(inputs)[:-1].decode()
        with self.f.domain(),patch.object(E,'PILOT',self.f.volume),patch.object(BOOT,'owned',side_effect=self.f.owned):
            return E.publication(self.B,KEEP,saved,self.f.destination,self.config,run)

    def test_complete_copy_with_absent_unpersisted_check_is_readonly_and_never_quality_acceptance(self):
        before=self.physical()
        with patch.object(self.B,'write',side_effect=AssertionError('write')), \
             patch.object(self.B,'record',side_effect=AssertionError('record')):
            result=self.observe()
        self.assertEqual(before,self.physical());self.assertTrue(result['remote_revocation_confirmed'])
        self.assertTrue(result['retained_publication_verified']);self.assertIsNone(result['remote']['check'])
        self.assertEqual(result['publication']['files'],5)
        for key in ('installation_admitted','administrator_ready','production_gate_enabled'):self.assertIs(result[key],False)
        self.assertIn(b'FAIL',(self.destination/'test_result.json').read_bytes())
        self.assertIsNone(self.client.token);self.assertEqual(self.client.expires,0)
        self.assertEqual(self.client_factory.call_args.kwargs,{'permissions':{'contents':'read','checks':'read'}})
        self.assertTrue(all(call.args[0]=='GET' for call in self.client.repo.call_args_list))

    def test_exact_failure_check_is_read_back_without_mutation(self):
        self.checks=[self.check()];result=self.observe()
        self.assertEqual(result['remote']['check']['id'],81234)
        self.assertEqual(self.client.repo.call_count,8)

    def test_explicit_proxy_only_routes_https_and_preserves_tls_redirect_handlers(self):
        before=list(self.client.opener.handlers);proxy='http://proxy.example.invalid:7890'
        tls=next(handler for handler in before if isinstance(handler,E.urllib.request.HTTPSHandler))
        context=tls._context
        result=self.observe(api_proxy=proxy)
        self.assertTrue(all(handler in self.client.opener.handlers for handler in before))
        added=[handler for handler in self.client.opener.handlers if handler not in before]
        self.assertEqual(len(added),1);self.assertEqual(added[0].proxies,{'https':proxy})
        self.assertEqual(result['api_transport_sha256'],E.digest(BOOT.canonical({'https_proxy':proxy})))
        self.assertIs(tls._context,context)
        effective=context if context is not None else ssl._create_default_https_context()
        self.assertEqual(effective.verify_mode,ssl.CERT_REQUIRED);self.assertTrue(effective.check_hostname)
        redirect=next(handler for handler in before if type(handler).__name__=='NoRedirect')
        with self.assertRaises(ValueError):redirect.redirect_request(None,None,302,'',{},'http://elsewhere.invalid')

    def test_missing_explicit_proxy_does_not_inherit_operator_or_agent_environment(self):
        with patch.dict(os.environ,{'HTTPS_PROXY':'http://untrusted.invalid:8','https_proxy':'http://untrusted.invalid:9'}):
            result=self.observe()
        self.assertFalse(any(isinstance(handler,E.urllib.request.ProxyHandler) and handler.proxies
                             for handler in self.client.opener.handlers))
        self.assertEqual(result['api_transport_sha256'],E.digest(BOOT.canonical({'https_proxy':None})))

    def test_proxy_credentials_paths_redirects_and_noncanonical_or_invalid_ports_block_before_api(self):
        for proxy in ('http://user:secret@proxy.invalid:80','file:///tmp/proxy','http://proxy.invalid:80/',
                      'http://proxy.invalid:80?token=x','http://proxy.invalid:80#x','http://proxy.invalid',
                      'http://proxy.invalid:080','http://proxy.invalid:65536','http://proxy.invalid:0',
                      'http://proxy.invalid:80\n','http://PROXY.invalid:80',True):
            with self.subTest(proxy=proxy),self.assertRaises(ValueError):self.observe(api_proxy=proxy)
        self.client.repo.assert_not_called();self.assertIsNone(self.client.token)

    def test_real_saved_wal_and_git_retirement_survive_remote_and_copy_verification(self):
        f=self.f;self.event('retire-intent',{'head':f.head,'retained':str(self.destination)})
        subprocess.run(['/usr/bin/git','-C',str(f.repository/'mirror.git'),'worktree','remove','--force',str(f.source)],
                       env=BOOT.ENV,check=True,capture_output=True)
        (f.repository/'candidate.bundle').unlink();(f.repository/'frozen.json').unlink()
        self.event('retired',{'head':f.head});result=self.observe()
        self.assertEqual(result['phase'],'retired');self.assertTrue(result['retained_publication_verified'])
        self.assertFalse(f.source.exists());self.assertTrue((f.destination/'tree/source/.git').exists())

    def test_unregistered_failed_run_preserves_raw_reports_without_manufacturing_copy(self):
        self.fixture(registered=False);result=self.observe()
        self.assertFalse(result['publication']['registered']);self.assertFalse(self.destination.exists())
        self.assertTrue((self.f.destination/'tree/source/.gatehouse-host/reports/invocations/failed-invocation/test_result.json').exists())

    def test_unregistered_publication_even_empty_is_rejected_and_retained(self):
        self.fixture(registered=False);self.destination.mkdir(mode=0o700)
        with self.assertRaisesRegex(ValueError,'unregistered publication'):self.observe()
        self.assertTrue(self.destination.exists())

    def test_registered_copy_must_exist_even_before_retirement(self):
        shutil.rmtree(self.destination)
        with self.assertRaises(OSError):self.observe()
        self.assertTrue(self.f.source.exists());self.assertIsNone(self.client.token)

    def test_missing_changed_or_extra_publication_files_block(self):
        path=self.destination/'manifest.json';raw=path.read_bytes();path.unlink()
        with self.assertRaisesRegex(ValueError,'partial retained'):self.observe()
        BOOT.write(path,b'changed',0o600)
        with self.assertRaisesRegex(ValueError,'publication file differs'):self.observe()
        path.write_bytes(raw);BOOT.write(self.destination/'extra',b'unknown',0o600)
        with self.assertRaisesRegex(ValueError,'unregistered publication file'):self.observe()

    def test_unknown_directory_and_symlink_or_hardlink_block_without_cleanup(self):
        extra=self.destination/'unknown';extra.mkdir(mode=0o700)
        with self.assertRaisesRegex(ValueError,'unknown or changed'):self.observe()
        extra.rmdir();extra.symlink_to('missing')
        with self.assertRaises(ValueError):self.observe()
        extra.unlink();os.link(self.destination/'manifest.json',extra)
        with self.assertRaises(ValueError):self.observe()
        self.assertTrue(extra.exists())

    def test_publication_mode_and_private_root_are_exact(self):
        self.destination.chmod(0o755)
        with self.assertRaises(ValueError):self.observe()
        self.destination.chmod(0o700);(self.destination/'manifest.json').chmod(0o644)
        with self.assertRaisesRegex(ValueError,'publication file differs'):self.observe()

    def test_registered_identity_path_and_saved_registration_must_agree(self):
        evidence=copy.deepcopy(self.evidence);evidence['run']='another-run'
        with self.assertRaisesRegex(ValueError,'another run'):self.publication_only(evidence=evidence)
        evidence=copy.deepcopy(self.evidence);evidence['directory']=str(self.f.root/'operator-publication')
        with self.assertRaisesRegex(ValueError,'outside the old'):self.publication_only(evidence=evidence)
        saved=json.loads((self.f.destination/'preservation.json').read_bytes());saved['evidence_registered']=False
        with self.assertRaisesRegex(ValueError,'registration differs'):self.publication_only(saved=saved)

    def test_registered_evidence_and_configuration_pins_cannot_be_substituted(self):
        for key in ('machine_sha256','manifest_sha256','registry_sha256','configuration_sha256'):
            evidence=copy.deepcopy(self.evidence);evidence[key]='f'*64
            with self.subTest(key=key),self.assertRaisesRegex(ValueError,'digest differs|configuration was not preserved'):
                self.publication_only(evidence=evidence)
        with self.assertRaisesRegex(ValueError,'old policy'):self.publication_only(policy='f'*64)

    def test_missing_saved_invocation_member_or_publication_directory_is_rejected(self):
        saved=json.loads((self.f.destination/'preservation.json').read_bytes())
        key=next(key for key in saved['inventory']['files'] if key.endswith('registered-invocation/invocation.json'))
        saved['inventory']['files'].pop(key)
        with self.assertRaisesRegex(ValueError,'not completely preserved'):self.publication_only(saved=saved)
        (self.destination/'empty').rmdir()
        with self.assertRaisesRegex(ValueError,'partial retained'):self.publication_only()

    def test_publication_file_and_entry_budgets_block_without_deleting_inputs(self):
        with patch.object(KEEP,'MAX_FILE',1),self.assertRaisesRegex(ValueError,'byte budget'):
            self.publication_only()
        with patch.object(KEEP,'MAX_ENTRIES',1),self.assertRaisesRegex(ValueError,'entry budget'):
            self.publication_only()
        self.assertTrue((self.destination/'manifest.json').exists())

    def test_success_running_cancelled_or_foreign_external_check_cannot_confirm_revocation(self):
        for field,value in (('conclusion','success'),('conclusion','cancelled'),('status','in_progress'),
                            ('external_id','another-run'),('head_sha','f'*40),('id',True),('id',P.NEGATIVE_CHECK)):
            check=self.check();check[field]=value;self.checks=[check]
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):self.observe()

    def test_duplicate_authoritative_checks_reject_actual_old_client_lookup(self):
        self.checks=[self.check(),self.check()]
        with self.assertRaisesRegex(ValueError,'multiple authoritative'):self.observe()

    def test_other_app_check_is_not_mistaken_for_this_run(self):
        check=self.check();check['app']['id']=999;self.checks=[check]
        result=self.observe();self.assertIsNone(result['remote']['check'])

    def test_persisted_check_cannot_disappear_or_be_retargeted(self):
        # Replace the final completion with the same actual journal update order.
        self.f.writer.execute('DELETE FROM events WHERE sequence=3')
        self.f.writer.execute("UPDATE runs SET remote='revoking'")
        self.update(check_id=81234);self.update(remote='revoked')
        with self.assertRaisesRegex(ValueError,'missing remotely'):self.observe()
        self.checks=[self.check()];self.assertEqual(self.observe()['remote']['check']['id'],81234)
        self.checks[0]['id']=99
        with self.assertRaisesRegex(ValueError,'identity changed'):self.observe()

    def test_repository_branch_commit_and_tree_changes_block(self):
        real=self.api
        for path,key in (('', 'id'),('/git/ref/heads/main','ref'),('/git/commits/'+self.f.head,'sha')):
            def api(method,current,path=path,key=key):
                value=real(method,current)
                if current==path:value[key]='wrong'
                return value
            self.client.repo.side_effect=api
            with self.assertRaisesRegex(ValueError,'repository/base/commit'):self.observe()
        def branch(method,path):
            value=real(method,path)
            if path=='/git/ref/heads/main':value['object']['sha']='f'*40
            return value
        self.client.repo.side_effect=branch
        with self.assertRaisesRegex(ValueError,'repository/base/commit'):self.observe()

    def test_transport_failure_clears_token_and_retains_all_local_state(self):
        before=self.physical();self.client.repo.side_effect=RuntimeError('API outcome unknown')
        with self.assertRaisesRegex(RuntimeError,'outcome unknown'):self.observe()
        self.assertIsNone(self.client.token);self.assertEqual(before,self.physical());self.assertTrue(self.f.source.exists())

    def test_remote_change_between_readbacks_is_rejected(self):
        real=self.api;count=0
        def api(method,path):
            nonlocal count
            if '/check-runs?' in path:
                count+=1
                if count==2:self.checks=[self.check()]
            return real(method,path)
        self.client.repo.side_effect=api
        with self.assertRaisesRegex(ValueError,'remote state changed'):self.observe()

    def test_journal_advance_during_api_reads_is_rejected(self):
        real=self.api;changed=False
        def api(method,path):
            nonlocal changed
            if not changed:
                changed=True;self.event('retire-intent',{'head':self.f.head,'retained':str(self.destination)})
            return real(method,path)
        self.client.repo.side_effect=api
        with self.assertRaisesRegex(ValueError,'journal changed'):self.observe()

    def test_copy_changed_after_first_scan_is_rejected(self):
        real=E.publication;count=0
        def scan(*args):
            nonlocal count
            value=real(*args);count+=1
            if count==1:(self.destination/'isolation/raw.stderr').write_bytes(b'changed')
            return value
        with patch.object(E,'publication',side_effect=scan),self.assertRaisesRegex(ValueError,'publication file differs'):
            self.observe()

    def test_changed_pinned_preservation_or_accepted_stage_blocks_before_api(self):
        with self.assertRaisesRegex(ValueError,'manifest pin'):self.observe(pin='f'*64)
        self.client.repo.assert_not_called();self.preflight.accepted_release.side_effect=ValueError('old stage changed')
        with self.assertRaisesRegex(ValueError,'old stage changed'):self.observe()
        self.client.repo.assert_not_called()

    def test_pending_revocation_is_not_a_remote_effect_proof(self):
        self.f.writer.execute('DELETE FROM events WHERE sequence=3');self.f.writer.execute("UPDATE runs SET remote='revoking'")
        with self.assertRaisesRegex(ValueError,'not completed local'):self.observe()
        self.client.repo.assert_not_called()

    def test_wrong_or_missing_lease_and_active_worker_block_before_api(self):
        for value in (None,True,-1):
            with self.assertRaisesRegex(ValueError,'inherited'):self.observe(lock_fd=value)
        with self.f.config_path.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError,'lease'):self.observe(lock_fd=wrong.fileno())
        self.builder.quiet_units.side_effect=ValueError('worker active')
        with self.assertRaisesRegex(ValueError,'worker active'):self.observe()
        self.client.repo.assert_not_called()

    def test_time_budget_blocks_before_reads_without_state_changes(self):
        with patch.object(E,'MAX_SECONDS',-1),self.assertRaisesRegex(ValueError,'time budget'):self.observe()
        self.client.repo.assert_not_called()

    def test_time_budget_expiring_during_remote_reads_clears_credentials_and_blocks(self):
        real=self.api;before=self.physical()
        def api(method,path):
            value=real(method,path);E.MAX_SECONDS=-1;return value
        self.client.repo.side_effect=api
        with patch.object(E,'MAX_SECONDS',120),self.assertRaisesRegex(ValueError,'time budget'):self.observe()
        self.assertEqual(before,self.physical());self.assertIsNone(self.client.token)

    def test_nonroot_and_direct_execution_expose_no_administrator_entry(self):
        with patch.object(E.os,'geteuid',return_value=1000),patch.object(self.B,'load') as imported:
            with self.assertRaisesRegex(ValueError,'administrator'):E.observe(self.B,*([None]*6))
        imported.assert_not_called()
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/admit-upgrade-effects.py')],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'no administrator entry',result.stderr)


if __name__=='__main__':unittest.main()
