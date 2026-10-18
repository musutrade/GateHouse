"""Original observer, real private files/flock; Root provenance and GitHub replaced explicitly."""
import copy
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch
import urllib.parse

ROOT = Path(__file__).resolve().parents[3]


def load(name,path):
    spec = importlib.util.spec_from_file_location(name,ROOT/path)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


R = load('installation_readmission','tools/trusted_gate/deploy/recover-interrupted-installation.py')
E = load('readmission_original_effects','tools/trusted_gate/deploy/admit-upgrade-effects.py')
BOOT = load('readmission_file_primitives','tools/trusted_gate/deploy/import-protected-inputs.py')
P = load('readmission_accepted_factory','tools/trusted_gate/deploy/preflight-history-pilot.py')
SOURCE = Path(R.__file__).read_bytes()


class ReadmissionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name);self.control = self.root/'control';self.control.mkdir(mode=0o700)
        self.output = self.root/'output';self.output.mkdir(mode=0o700)
        self.stage = self.root/'stage';self.stage.mkdir(mode=0o700)
        self.program = self.root/'recovery.py';BOOT.write(self.program,SOURCE,0o400)
        for name,value in {'CONTROL':self.control,'ADMISSION':self.output/'readmission.json',
                           'STAGE':self.stage,'__file__':str(self.program)}.items():
            p=patch.object(R,name,value);p.start();self.addCleanup(p.stop)
        self.lease = self.control/'controller.lock';BOOT.write(self.lease,b'',0o600)
        self.fd = os.open(self.lease,os.O_RDWR);self.addCleanup(os.close,self.fd)
        lease = [os.fstat(self.fd).st_dev,os.fstat(self.fd).st_ino]
        self.B = SimpleNamespace(**{name:getattr(BOOT,name) for name in
            ('need','canonical','unique','write','path_operand','record')})
        self.B.owned = self.owned;self.B.read = self.read
        self.fenced = {'head':R.HEAD,'phase':'fenced','transaction':'a'*36}
        fence_pin = R.sha(BOOT.canonical(self.fenced))
        self.admin = {'schema':'gatehouse-administrator-dispatch/v1','head':R.HEAD,'phase':'dispatching',
            'inflight':None,'completed':[{'step':s,'sha256':'b'*64} for s in R.STEPS],
            'helper_manifest_sha256':R.MANIFEST,'recipe_sha256':R.RECIPE,'lease':lease,'transaction':'c'*36}
        self.lifecycle = {'head':R.HEAD,'helper_manifest_sha256':R.MANIFEST,'recipe_sha256':'b'*64,
            'phase':'installing','transaction':'a'*36,'material_sha256':'d'*64,'entry_fence_sha256':fence_pin,
            'old_head':R.OLD_HEAD,'old_run':R.OLD_RUN,'lease':lease}
        self.files = {'head':R.HEAD,'phase':'installing','transaction':'a'*36,'material_sha256':'d'*64,
            'fence_sha256':fence_pin,'directories_done':[{},{}],'files_done':[],'inflight':None}
        self.dump('administrator-upgrade.json',self.admin);self.dump('linux-upgrade.json',self.lifecycle)
        self.dump('file-install.json',self.files)
        self.fence = SimpleNamespace(LEASE=self.lease,lease=self.lock,read_receipt=lambda B:copy.deepcopy(self.fenced))
        self.effects = SimpleNamespace(remote=E.remote)
        self.other = SimpleNamespace(untouched=True)
        self.B.load = Mock(side_effect=lambda root,name:self.fence if name=='install-transaction.py'
                           else self.effects if name=='admit-upgrade-effects.py' else self.other)
        self.B.authenticate = Mock(return_value={'revision':R.HEAD})
        for name in R.CONTEXT_NAMES:
            if not hasattr(self.B,name):setattr(self.B,name,Mock())
        self.parents = list(R.APPROVED_PARENTS)
        self.branch = R.APPROVED_MAIN;self.host_tree = '7'*40;self.tree_changed = False
        self.client = SimpleNamespace(repo=Mock(side_effect=self.reply),find_check=Mock(return_value=None))
        self.config = {'coverage_acceptance':'transition','repository':'musutrade/GateHouse','check_name':'Host',
                       'app_id':4867361}
        self.run = {'id':R.OLD_RUN,'head':R.OLD_HEAD,'local':'invalid','remote':'revoked',
                    'check_id':None,'input':{'tree':'8'*40}}
        self.preflight = SimpleNamespace(REPOSITORY_ID=1414069625,REPOSITORY='musutrade/GateHouse',
                                         NEGATIVE_CHECK=117423517761)
        self.contract = SimpleNamespace(BASE=R.SOURCE_BASE)
        self.recovery = R.Readmission(self.B,self.fd)

    def owned(self,path,directory=False,private=False):
        path=Path(path);self.assertTrue(path==self.root or self.root in path.parents)
        info=path.lstat();BOOT.need(not stat.S_ISLNK(info.st_mode) and not info.st_mode&0o022,'fixture path')
        BOOT.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink==1,'fixture type')
        BOOT.need(not private or not info.st_mode&0o077,'fixture privacy');return path

    def read(self,path,limit):
        raw=self.owned(path).read_bytes();BOOT.need(len(raw)<=limit,'fixture budget');return raw

    def lock(self,B,fd):
        self.assertEqual(os.fstat(fd).st_ino,self.lease.stat().st_ino)
        fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)

    def dump(self,name,value):BOOT.record(self.control/name,value)

    def reply(self,method,path):
        self.assertEqual(method,'GET')
        if path=='':return {'id':1414069625,'full_name':'musutrade/GateHouse','default_branch':'main'}
        if path=='/git/ref/heads/main':return {'ref':'refs/heads/main','object':{'type':'commit','sha':self.branch}}
        if path=='/git/commits/'+R.OLD_HEAD:return {'sha':R.OLD_HEAD,'tree':{'sha':'8'*40}}
        for commit,root,tools in ((R.SOURCE_BASE,'1'*40,'2'*40),(R.APPROVED_MAIN,'3'*40,'4'*40)):
            if path=='/git/commits/'+commit:return {'sha':commit,'tree':{'sha':root},
                'parents':[{'sha':sha} for sha in self.parents]}
            if path=='/git/trees/'+root:return {'sha':root,'truncated':False,
                'tree':[{'path':'tools','type':'tree','sha':tools}]}
            if path=='/git/trees/'+tools:return {'sha':tools,'truncated':False,
                'tree':[{'path':'harness-gate','type':'tree',
                         'sha':'9'*40 if self.tree_changed and commit==R.APPROVED_MAIN else self.host_tree}]}
        raise AssertionError(path)

    def observe(self):
        return self.recovery.remote(E.remote,self.B,self.client,self.config,self.run,self.contract,self.preflight)

    def test_truthful_current_base_original_observer_checks_and_immutable_proof(self):
        result=self.observe();self.assertEqual(result['base'],R.APPROVED_MAIN)
        self.assertEqual(self.contract.BASE,R.SOURCE_BASE);self.assertEqual(result['head'],R.OLD_HEAD)
        self.assertEqual(R.ADMISSION.stat().st_mode&0o777,0o400)
        saved=R.ADMISSION.read_bytes();second=self.observe();self.assertEqual(second,result)
        self.assertEqual(R.ADMISSION.read_bytes(),saved)
        calls=[c.args[1] for c in self.client.repo.call_args_list]
        self.assertFalse(any('/compare/' in path or '..' in path for path in calls))
        self.assertEqual(calls.count('/git/commits/'+R.APPROVED_MAIN),2)
        self.assertEqual(calls.count('/git/ref/heads/main'),2)

    def test_foreign_next_or_old_main_is_rejected_without_writing_readmission(self):
        for main in (R.SOURCE_BASE,'f'*40):
            self.branch=main
            with self.assertRaisesRegex(ValueError,'remote repository/base/commit'):self.observe()
            self.assertFalse(R.ADMISSION.exists())

    def test_changed_host_tree_blocks_readmission(self):
        self.tree_changed=True
        with self.assertRaisesRegex(ValueError,'unchanged host subtrees'):self.observe()
        self.assertFalse(R.ADMISSION.exists())

    def test_missing_reordered_foreign_or_extra_parents_block(self):
        for parents in ([],[R.SOURCE_BASE],list(reversed(R.APPROVED_PARENTS)),
                        ['e'*40,R.APPROVED_PARENTS[1]],R.APPROVED_PARENTS+['f'*40]):
            good=list(self.parents);self.parents=parents
            with self.assertRaises(ValueError):self.observe()
            self.parents=good;self.assertFalse(R.ADMISSION.exists())

    def accepted_client(self):
        payload={}
        for name,pin in (('common','8596d8b9595d9a27870ca61dc37bb36b5cc23971244d57810a240bade7726d7c'),
                         ('github','ebc151fe3e562ff31f0b967efc06317371dcc8720b0532f20073d641732263b9')):
            raw=(ROOT/'tools/quality/tests/fixtures/accepted-659'/(name+'.py')).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(),pin);payload['trusted_gate/'+name+'.py']=raw
        cls=P.client_class(payload);client=cls.__new__(cls)
        client.repository='musutrade/GateHouse';client.token='fixture-only-token';client.expires=time.time()+3600
        paths=[]
        def opened(request,timeout):
            url=urllib.parse.urlsplit(request.full_url)
            self.assertEqual(url.scheme,'https');self.assertEqual(url.netloc,'api.github.com')
            prefix='/repos/musutrade/GateHouse';self.assertTrue(url.path.startswith(prefix))
            path=url.path[len(prefix):]+('?' + url.query if url.query else '')
            paths.append(path);self.assertEqual(request.method,'GET');self.assertEqual(timeout,30)
            value={'check_runs':[]} if path.startswith('/commits/'+R.OLD_HEAD+'/check-runs?') else self.reply('GET',path)
            import io
            return contextlib.closing(io.BytesIO(json.dumps(value).encode()))
        client.opener=SimpleNamespace(open=Mock(side_effect=opened))
        return client,paths

    def test_real_accepted_request_repo_check_lookup_and_path_guard_allow_the_parent_proof(self):
        self.client,paths=self.accepted_client()
        result=self.observe();self.assertEqual(result['base'],R.APPROVED_MAIN)
        self.assertTrue(paths);self.assertTrue(any('/check-runs?' in path for path in paths))
        self.assertFalse(any('/compare/' in path or '..' in path for path in paths))
        self.assertEqual(json.loads(R.ADMISSION.read_bytes())['ancestry']['parents'],R.APPROVED_PARENTS)

    def test_old_compare_route_is_rejected_by_unmodified_client_before_http(self):
        client,paths=self.accepted_client()
        with self.assertRaisesRegex(Exception,'invalid GitHub API path'):
            client.repo('GET','/compare/'+R.SOURCE_BASE+'...'+R.APPROVED_MAIN)
        self.assertEqual(paths,[]);self.assertFalse(R.ADMISSION.exists())

    def test_real_client_foreign_parent_or_changed_tree_never_writes_admission(self):
        self.client,paths=self.accepted_client();self.parents=['e'*40,R.APPROVED_PARENTS[1]]
        with self.assertRaises(ValueError):self.observe()
        self.assertFalse(R.ADMISSION.exists());self.parents=list(R.APPROVED_PARENTS);self.tree_changed=True
        with self.assertRaises(ValueError):self.observe()
        self.assertFalse(R.ADMISSION.exists())

    def test_original_repository_commit_and_check_guards_remain_active(self):
        for mutate in (lambda:self.run['input'].update(tree='e'*40),
                       lambda:self.run.update(check_id=12)):
            old=copy.deepcopy(self.run);mutate()
            with self.assertRaises(ValueError):self.observe()
            self.run=old;self.assertFalse(R.ADMISSION.exists())

    def test_cannot_readmit_quality_success_live_run_or_other_head(self):
        for key,value in (('local','passed'),('remote','pending'),('head','e'*40)):
            good=copy.deepcopy(self.run);self.run[key]=value
            with self.assertRaisesRegex(ValueError,'retired transition pilot'):self.observe()
            self.run=good
        self.config['coverage_acceptance']='full'
        with self.assertRaises(ValueError):self.observe()

    def test_first_readmission_requires_zero_files_and_no_unknown_intent(self):
        for mutate in (lambda:self.files.update(files_done=[{}]),
                       lambda:self.files.update(inflight={'kind':'file','path':'unknown'}),
                       lambda:self.files.update(directories_done=[])):
            good=copy.deepcopy(self.files);mutate();self.dump('file-install.json',self.files)
            with self.assertRaises(ValueError):R.Readmission(self.B,self.fd)
            self.files=good;self.dump('file-install.json',good)

    def test_partial_metadata_or_early_activation_is_not_adopted(self):
        path=self.control/'.file-install.json.tmp';BOOT.write(path,b'partial',0o600)
        with self.assertRaisesRegex(ValueError,'unfinished recovery metadata'):R.Readmission(self.B,self.fd)
        path.unlink();self.dump('controller-activation.json',{'phase':'prepared'})
        with self.assertRaisesRegex(ValueError,'advanced activation'):R.Readmission(self.B,self.fd)

    def test_material_or_original_lease_change_blocks_replay(self):
        self.observe();self.files['material_sha256']='e'*64;self.dump('file-install.json',self.files)
        with self.assertRaises(ValueError):self.observe()

    def test_readmission_record_mode_or_bytes_change_blocks(self):
        self.observe();R.ADMISSION.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'immutable'):self.observe()
        R.ADMISSION.chmod(0o400);raw=R.ADMISSION.read_bytes();R.ADMISSION.chmod(0o600)
        value=json.loads(raw);value['approved_main']='f'*40;R.ADMISSION.write_bytes(BOOT.canonical(value));R.ADMISSION.chmod(0o400)
        with self.assertRaises(ValueError):self.observe()

    def test_later_known_file_progress_keeps_same_generation_readmission(self):
        result=self.observe();self.files['files_done']=[{'path':'known-validated-by-original-engine'}]
        self.dump('file-install.json',self.files)
        self.assertEqual(self.observe(),result)

    def test_only_historical_remote_observer_is_adapted(self):
        self.assertIs(self.recovery.load(self.stage/'helpers','unrelated.py'),self.other)
        effect=self.recovery.load(self.stage/'helpers','admit-upgrade-effects.py')
        self.assertEqual(effect.remote(self.B,self.client,self.config,self.run,self.contract,self.preflight)['base'],R.APPROVED_MAIN)
        self.assertIs(E.remote.__globals__['remote'],E.remote)

    def test_checkpoint_observation_preserves_original_verifier_result_and_exception(self):
        original=Mock(return_value={'unchanged':'verified'})
        checker=SimpleNamespace(verify_completed=original)
        self.B.load=Mock(return_value=checker)
        with patch.object(R,'CHECKPOINT') as checkpoint:
            result=self.recovery.load(self.stage/'helpers','check-core-policy.py').verify_completed('argument')
            self.assertEqual(result,{'unchanged':'verified'});original.assert_called_once_with('argument')
            checkpoint.assert_called_once_with('retained-core-verification')
            original.side_effect=ValueError('still blocked')
            with self.assertRaisesRegex(ValueError,'still blocked'):checker.verify_completed('argument')

    def test_dispatch_uses_original_context_and_resume_once_with_one_continuous_lease(self):
        fcntl.flock(self.fd,fcntl.LOCK_UN);seen=[]
        def context(B,*args):
            seen.append(args[-1]);other=os.open(self.lease,os.O_RDWR)
            try:
                with self.assertRaises(BlockingIOError):fcntl.flock(other,fcntl.LOCK_EX|fcntl.LOCK_NB)
            finally:os.close(other)
            return SimpleNamespace(B=B)
        A=SimpleNamespace(Context=Mock(side_effect=context),receipt=Mock(return_value=self.admin),
                          resume=Mock(return_value={'phase':'complete'}),prepare=Mock(side_effect=AssertionError('rebuild')))
        with patch.object(R.os,'geteuid',return_value=0):self.assertEqual(R.dispatch(self.B,A),{'phase':'complete'})
        A.resume.assert_called_once();A.prepare.assert_not_called()
        with self.assertRaises(OSError):os.fstat(seen[0])

    def test_incomplete_preparation_never_restarts_build(self):
        self.admin['phase']='preparing';self.dump('administrator-upgrade.json',self.admin)
        with self.assertRaises(ValueError):R.Readmission(self.B,self.fd)


if __name__ == '__main__':unittest.main()
