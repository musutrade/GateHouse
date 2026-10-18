"""Real WAL/Git/copy/file lifecycle, replacing Root/preparation/native/API borders."""
from contextlib import contextmanager, ExitStack
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT=Path(__file__).resolve().parents[3]


def load(name,path):
    spec=importlib.util.spec_from_file_location(name,ROOT/path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FILES=load('lifecycle_file_fixture','tools/quality/tests/test_trusted_gate_file_install.py')
JOURNAL=load('lifecycle_journal_fixture','tools/quality/tests/test_trusted_gate_upgrade_journal.py')
INPUTS=load('lifecycle_inputs','tools/trusted_gate/deploy/observe-installation-inputs.py')
EFFECTS=load('lifecycle_effects','tools/trusted_gate/deploy/admit-upgrade-effects.py')
L=load('lifecycle_coordinator','tools/trusted_gate/deploy/coordinate-linux-upgrade.py')
I=FILES.I;FENCE=FILES.FENCE;BOOT=FILES.BOOT;J=JOURNAL.J;KEEP=JOURNAL.KEEP


def accepted():
    return {'trusted_gate/'+name+'.py':(ROOT/'tools/quality/tests/fixtures/accepted-659'/(name+'.py')).read_bytes()
            for name in L.CODE_PINS}


class ClosedOldControllerTests(unittest.TestCase):
    def setUp(self):self.B=SimpleNamespace(need=BOOT.need);self.payload=accepted()

    def test_exact_accepted_graph_loads_without_installed_package_and_is_removed(self):
        before=set(sys.modules);finders=list(sys.meta_path)
        with L.old_controller(self.B,self.payload) as old:
            self.assertEqual(old.control.Control.__module__,old.prefix+'.control')
            self.assertNotIn('trusted_gate',sys.modules.keys()-before)
            self.assertEqual(old.execution.clean_environment()['GIT_CONFIG_GLOBAL'],'/dev/null')
            self.assertTrue(all(name==old.prefix or name.startswith(old.prefix+'.') for name in sys.modules.keys()-before
                                if name.startswith('_gatehouse_upgrade_')))
        self.assertEqual(sys.meta_path,finders)
        self.assertFalse(any(name.startswith(old.prefix) for name in sys.modules))

    def test_changed_missing_or_nonbyte_module_rejects_before_installing_importer(self):
        for value in (b'operator replacement',False):
            payload=dict(self.payload);payload['trusted_gate/common.py']=value;before=list(sys.meta_path)
            with self.assertRaisesRegex(ValueError,'source differs'):
                with L.old_controller(self.B,payload):pass
            self.assertEqual(sys.meta_path,before)
        self.payload.pop('trusted_gate/source.py')
        with self.assertRaises(KeyError):
            with L.old_controller(self.B,self.payload):pass

    def test_cli_host_publication_and_unknown_modules_cannot_import(self):
        with L.old_controller(self.B,self.payload) as old:
            for name in ('cli','host','publication','runtime','untrusted'):
                with self.subTest(name=name),self.assertRaises(ImportError):
                    __import__(old.prefix+'.'+name,fromlist=[name])

    def test_nested_contexts_are_independent_and_exception_cleans_both(self):
        with self.assertRaisesRegex(RuntimeError,'interrupt'):
            with L.old_controller(self.B,self.payload) as first:
                with L.old_controller(self.B,self.payload) as second:
                    self.assertNotEqual(first.prefix,second.prefix);raise RuntimeError('interrupt')
        self.assertFalse(any(name.startswith(first.prefix) or name.startswith(second.prefix) for name in sys.modules))

    def test_revoke_adapter_cannot_publish_or_touch_other_run_or_negative_check(self):
        raw=SimpleNamespace(find_check=Mock(return_value=None),update_check=Mock(return_value=123))
        keep=SimpleNamespace(RUN='fixed-run',HEAD='a'*40);p=SimpleNamespace(REPOSITORY='owner/repo',NEGATIVE_CHECK=99)
        adapter=L.RevokeOnly(self.B,raw,{'check_name':'exact'},keep,p)
        run={'id':keep.RUN,'head':keep.HEAD,'repository':p.REPOSITORY}
        for status,conclusion in (('completed','success'),('completed','neutral'),('in_progress',None)):
            with self.assertRaisesRegex(ValueError,'only withdraw'):adapter.update_check(run,'exact',status,conclusion)
        raw.update_check.assert_not_called()
        self.assertEqual(adapter.update_check(run,'exact','completed','failure'),123)
        with self.assertRaisesRegex(ValueError,'another old run'):adapter.find_check({**run,'id':'other'},'exact')
        raw.find_check.return_value={'id':99,'external_id':keep.RUN}
        with self.assertRaisesRegex(ValueError,'identity differs'):adapter.find_check(run,'exact')
        raw.find_check.return_value=None;raw.update_check.return_value=True
        with self.assertRaisesRegex(ValueError,'response Check identity'):adapter.update_check(run,'exact','completed','failure')


class UpgradeLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.m=FILES.ReviewedFileTransactionTests();self.m.setUp();self.addCleanup(self.m.doCleanups)
        self.f=self.m.f;self.j=JOURNAL.UpgradeJournalTests();self.j.setUp();self.addCleanup(self.j.doCleanups)
        self.jf=self.j.f;f=self.f;jf=self.jf
        # Return the private original entries to their pre-coordinator modes.
        for path in f.entries:path.chmod(0o755)
        f.receipt.unlink();f.barrier.unlink()
        before=jf.config_path.read_bytes();f.config.write_bytes(before)
        self.m.materials['files'][str(f.config)]['before']=before
        original_owned=self.B_owned=f.B.owned
        def owned(path,**kwargs):
            path=Path(path)
            return original_owned(path,**kwargs) if path==f.root or f.root in path.parents else jf.owned(path,**kwargs)
        self.B=f.B;self.B.owned=owned;self.B.ENV=BOOT.ENV;self.B.git=BOOT.git;self.B.frozen_lock=BOOT.frozen_lock
        original_read=self.B.read
        self.B.read=lambda path,limit:original_read(path,limit) if Path(path)==f.root or f.root in Path(path).parents else BOOT.read(path,limit)
        self.B.authenticate=Mock(return_value={'revision':f.head})
        self.builder=SimpleNamespace(worker_identity=Mock(),quiet_units=Mock(),idle=Mock())
        self.B.load=lambda helpers,name:{'preserve-pilot-state.py':KEEP,'install-transaction.py':FENCE,
                                        'build-protected-core.py':self.builder}[name]
        self.ctx=SimpleNamespace(B=self.B,fd=f.lease_fd,code={'revision':f.head},manifest_pin='a'*64,
            recipe_pin='b'*64,material=self.m.materials,material_pin=L.digest(BOOT.canonical(I.descriptor(self.B,self.m.contract,self.m.materials))),
            keep=KEEP,files=I,fence=FENCE,contract=self.m.contract,journal=J,effects=EFFECTS,payload=accepted(),
            value={'paths':{'preservation':str(jf.destination)},'pins':{'preservation':self.j.pin},'api_proxy':None})
        self.ctx.contract.BASE=jf.head
        self.ctx.preflight=SimpleNamespace(REPOSITORY='musutrade/GateHouse',REPOSITORY_ID=1414069625,NEGATIVE_CHECK=117423517761)
        self.ctx.config=json.loads(before)
        self.ctx.config.update(policy_directory=str(f.root/'installed/etc/gatehouse/policy'),core=str(self.m.core),
            app_id=4867361,installation_id=159921491,app_key=str(jf.root/'checks.pem'),repository='musutrade/GateHouse',
            check_name='GateHouse Trusted Verification')
        BOOT.write(Path(self.ctx.config['app_key']),b'private fixture key',0o600)
        self.state=jf.control/'linux-upgrade.json';self.check=None;self.clients=[];self.failure=None
        self.native=Mock();self.file_observations=[]
        self.ctx.guard=self.guard;self.ctx.live=self.live;self.ctx.effect=self.effect
        self.ctx.observe_files=self.observe_files;self.ctx.mutators=self.mutators

    @contextmanager
    def domain(self):
        with self.jf.domain(),patch.multiple(J,CONTROL=self.jf.control,PILOT=self.jf.volume,BASE=self.jf.head), \
             patch.multiple(L,CONTROL=self.jf.control,RECEIPT=self.state),patch.object(EFFECTS,'PILOT',self.jf.volume), \
             patch.object(BOOT,'owned',side_effect=self.B.owned):
            yield

    def guard(self,*,fenced=False):
        FENCE.lease(self.B,self.ctx.fd);self.native()
        if fenced:FENCE.fenced(self.B,self.ctx.fd)

    def live(self):
        return J.observe(self.B,self.f.root/'helpers',self.f.root/'manifest','a'*64,self.jf.destination,self.j.pin,self.ctx.fd)

    def api(self,method,path,body=None):
        if self.failure=='read':raise RuntimeError('API read lost')
        p=self.ctx.preflight
        if method=='GET' and path=='':return {'id':p.REPOSITORY_ID,'full_name':p.REPOSITORY,'default_branch':'main'}
        if method=='GET' and path=='/git/ref/heads/main':return {'ref':'refs/heads/main','object':{'type':'commit','sha':self.jf.head}}
        if method=='GET' and path=='/git/commits/'+self.jf.head:
            inputs=json.loads(self.jf.writer.execute('SELECT input FROM runs WHERE id=?',(KEEP.RUN,)).fetchone()[0])
            return {'sha':self.jf.head,'tree':{'sha':inputs['tree']}}
        if method=='GET' and path.startswith('/commits/'+self.jf.head+'/check-runs?'):
            return {'check_runs':[] if self.check is None else [copy.deepcopy(self.check)]}
        if method=='PATCH' and self.check is not None and path=='/check-runs/'+str(self.check['id']):
            self.assertEqual(body['conclusion'],'failure');self.check.update(status=body['status'],conclusion=body['conclusion'])
            if self.failure=='patch':raise RuntimeError('PATCH response lost')
            return copy.deepcopy(self.check)
        raise AssertionError('unexpected API operation '+method+' '+path)

    @contextmanager
    def kit(self):
        with L.old_controller(self.B,self.ctx.payload) as old,ExitStack() as stack:
            for name in tuple(sys.modules):
                if name==old.prefix or name.startswith(old.prefix+'.'):
                    module=sys.modules[name]
                    if hasattr(module,'protected'):stack.enter_context(patch.object(module,'protected',side_effect=self.B.owned))
            stack.enter_context(patch.object(old.execution.Executor,'confirm_stopped',side_effect=lambda run:self.native()))
            stack.enter_context(patch.object(old.github.GitHub,'repo',side_effect=self.api))
            yield old

    @contextmanager
    def mutators(self,*,revoke):
        with self.kit() as old:
            journal=old.journal.Journal(self.jf.journal_path);client=None
            try:
                executor=old.execution.Executor(self.ctx.config)
                if revoke:
                    client=old.github.GitHub(4867361,159921491,self.ctx.config['app_key'],'musutrade/GateHouse',
                                             permissions={'checks':'write','contents':'read'})
                    self.clients.append(client)
                yield old,journal,executor,client
            finally:
                if client is not None:client.token,client.expires=None,0
                journal.close()

    def effect(self):
        state=self.live();p,saved,run=L.saved_run(self.ctx)
        EFFECTS.publication(self.B,KEEP,saved,p,self.ctx.config,run)
        with self.kit() as old:
            journal=old.journal.Journal(self.jf.journal_path)
            try:
                live=journal.get(KEEP.RUN)
                client=old.github.GitHub(4867361,159921491,self.ctx.config['app_key'],'musutrade/GateHouse',
                                         permissions={'checks':'read','contents':'read'})
                EFFECTS.remote(self.B,client,self.ctx.config,live,self.ctx.contract,self.ctx.preflight)
            finally:journal.close()
        return state

    def observe_files(self,phase):
        self.assertEqual(self.live()['phase'],'retired');self.file_observations.append(phase)
        INPUTS.generation(self.B,FENCE,I,self.m.contract,self.m.materials,self.ctx.fd,phase)

    def prepare(self):
        # Preparation uses only repository/base reads, without invoking a write client.
        with self.kit() as old,patch.object(L,'old_controller') as loader:
            @contextmanager
            def reuse(*args):yield old
            loader.side_effect=reuse
            return L.prepare(self.ctx)

    def execute_pipeline(self):
        with self.domain():
            if not self.state.exists():self.prepare()
            return L.resume(self.ctx)

    def read_state(self):return json.loads(self.state.read_bytes())

    def register(self):
        jf=self.jf;invocation=jf.source/'.gatehouse-host/reports/invocations/registered'
        invocation.mkdir(mode=0o700)
        for name,raw in (('test_result.json',b'{"status":"FAIL"}'),('manifest.json',b'{"artifacts":[]}'),
                         ('artifact-registry.json',b'{"artifacts":[]}'),('invocation.json',BOOT.canonical({'commit':jf.head}))):
            BOOT.write(invocation/name,raw,0o600)
        (invocation/'empty').mkdir(mode=0o700);(invocation/'isolation').mkdir(mode=0o700)
        BOOT.write(invocation/'isolation/raw.stderr',b'preserved private diagnostic',0o600)
        evidence={'run':KEEP.RUN,'directory':str(invocation),**{key:L.digest((invocation/name).read_bytes())
            for name,key in (('test_result.json','machine_sha256'),('manifest.json','manifest_sha256'),
                             ('artifact-registry.json','registry_sha256'))}}
        jf.writer.execute('UPDATE runs SET local=?,evidence=?',('passed',BOOT.canonical(evidence)[:-1].decode()))
        jf.destination=jf.root/'registered-backup';jf.preserve();self.j.pin=L.digest((jf.destination/'preservation.json').read_bytes())
        self.ctx.value['paths']['preservation']=str(jf.destination);self.ctx.value['pins']['preservation']=self.j.pin
        return jf.control/KEEP.RUN/'publication'

    def check_run(self):
        self.check={'id':81234,'name':self.ctx.config['check_name'],'app':{'id':4867361},'head_sha':self.jf.head,
                    'external_id':KEEP.RUN,'status':'completed','conclusion':'success'}

    def test_real_absent_check_wal_git_and_all27_files_complete_disabled_with_saved_pending_unchanged(self):
        before=(self.jf.destination/'journal.sqlite3').read_bytes();result=self.execute_pipeline()
        self.assertEqual(result['phase'],'installed-disabled');self.assertEqual(self.m.read_state()['phase'],'installed-disabled')
        self.assertFalse(self.jf.source.exists());self.assertEqual((self.jf.destination/'journal.sqlite3').read_bytes(),before)
        for path in self.f.entries:self.assertEqual(path.stat().st_mode&0o777,0)
        self.assertIn('before-files',self.file_observations);self.assertIn('file-checkpoint',self.file_observations)
        for key in ('entries_enabled','administrator_ready','production_gate_enabled'):self.assertIs(result[key],False)
        self.assertTrue(all(client.token is None for client in self.clients))

    def test_actual_old_control_withdraws_only_exact_check_and_persists_id(self):
        self.check_run();self.execute_pipeline();self.assertEqual(self.check['conclusion'],'failure')
        self.assertEqual(self.jf.writer.execute('SELECT check_id FROM runs WHERE id=?',(KEEP.RUN,)).fetchone()[0],81234)

    def test_registered_raw_copy_is_complete_before_real_source_retirement(self):
        destination=self.register();self.execute_pipeline()
        self.assertEqual((destination/'isolation/raw.stderr').read_bytes(),b'preserved private diagnostic')
        self.assertTrue((destination/'empty').is_dir());self.assertFalse(self.jf.source.exists())
        self.assertIn(b'FAIL',(destination/'test_result.json').read_bytes())

    def test_partial_publication_temporary_is_retained_and_blocks_retry_without_retiring(self):
        destination=self.register()
        with self.domain():self.prepare()
        temp=destination.with_name('.publication-'+self.read_state()['transaction']+'.new');real=self.B.write
        def write(path,raw,mode=0o444):
            real(path,raw,mode)
            if temp in Path(path).parents:raise OSError('copy write interrupted')
        with patch.object(self.B,'write',side_effect=write),self.assertRaisesRegex(OSError,'copy write'):self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'copying');self.assertTrue(self.jf.source.exists())
        with self.assertRaisesRegex(ValueError,'incomplete publication temporary'):self.execute_pipeline()
        self.assertTrue(temp.exists());self.assertFalse(destination.exists());self.assertTrue(self.jf.source.exists())

    def test_complete_copy_before_rename_is_reverified_and_resumed(self):
        destination=self.register();real=os.replace
        def replace(source,target):
            if Path(target)==destination:raise OSError('before publication rename')
            return real(source,target)
        with patch.object(os,'replace',side_effect=replace),self.assertRaisesRegex(OSError,'before publication'):self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'copying');self.assertTrue(self.jf.source.exists())
        self.assertEqual(self.execute_pipeline()['phase'],'installed-disabled')

    def test_copy_rename_before_parent_sync_is_adopted_only_after_complete_validation(self):
        destination=self.register();real=os.replace
        def replace(source,target):
            real(source,target)
            if Path(target)==destination:raise OSError('publication rename response lost')
        with patch.object(os,'replace',side_effect=replace),self.assertRaisesRegex(OSError,'response lost'):self.execute_pipeline()
        self.assertTrue(destination.exists());self.assertEqual(self.read_state()['phase'],'copying')
        self.assertEqual(self.execute_pipeline()['phase'],'installed-disabled')

    def test_preexisting_publication_blocks_before_any_coordinator_change(self):
        destination=self.jf.control/KEEP.RUN/'publication';destination.mkdir(mode=0o700)
        with self.assertRaisesRegex(ValueError,'without an upgrade transaction'):self.execute_pipeline()
        self.assertFalse(self.state.exists());self.assertFalse(self.f.receipt.exists());self.assertTrue(self.jf.source.exists())

    def test_lost_patch_response_keeps_revoking_source_and_recovers_same_operation(self):
        self.check_run();self.failure='patch'
        with self.assertRaisesRegex(RuntimeError,'response lost'):self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'revoking');self.assertTrue(self.jf.source.exists())
        self.assertEqual(self.check['conclusion'],'failure');self.assertFalse(self.m.receipt.exists())
        for path in self.f.entries:self.assertEqual(path.stat().st_mode&0o777,0)
        self.failure=None;transaction=self.read_state()['transaction'];self.execute_pipeline()
        self.assertEqual(self.read_state()['transaction'],transaction);self.assertEqual(self.check['id'],81234)

    def test_remote_success_after_local_revocation_blocks_before_source_retirement(self):
        self.check_run();real=self.ctx.effect
        def changed():
            self.check['conclusion']='success';return real()
        with patch.object(self.ctx,'effect',side_effect=changed),self.assertRaisesRegex(ValueError,'not confirmed remotely'):
            self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'copying');self.assertTrue(self.jf.source.exists())
        self.assertEqual(self.jf.writer.execute('SELECT remote FROM runs WHERE id=?',(KEEP.RUN,)).fetchone()[0],'revoked')
        self.assertFalse(self.m.receipt.exists());self.check['conclusion']='failure'
        self.assertEqual(self.execute_pipeline()['phase'],'installed-disabled')

    def test_interrupted_real_retirement_resumes_without_reopening_or_restoring_pending_journal(self):
        freeze=self.jf.repository/'frozen.json';real=Path.unlink
        def unlink(path,*args,**kwargs):
            if path==freeze:raise OSError('freeze removal interrupted')
            return real(path,*args,**kwargs)
        with patch.object(Path,'unlink',new=unlink),self.assertRaisesRegex(OSError,'freeze removal'):
            self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'retiring');self.assertFalse(self.jf.source.exists())
        self.assertFalse((self.jf.repository/'candidate.bundle').exists());self.assertTrue(freeze.exists())
        self.assertFalse(self.m.receipt.exists())
        self.assertEqual(self.execute_pipeline()['phase'],'installed-disabled')
        self.assertFalse(freeze.exists())

    def test_read_failure_before_preparation_creates_no_transaction_or_fence(self):
        self.failure='read'
        with self.assertRaisesRegex(RuntimeError,'read lost'):self.execute_pipeline()
        self.assertFalse(self.state.exists());self.assertFalse(self.f.receipt.exists())

    def test_completed_retry_does_not_revoke_retire_or_write_again(self):
        expected=self.execute_pipeline();raw=self.state.read_bytes()
        with patch.object(self.B,'record',side_effect=AssertionError('record')), \
             patch.object(self.ctx,'mutators',side_effect=AssertionError('mutating old controller')):
            self.assertEqual(self.execute_pipeline(),expected)
        self.assertEqual(self.state.read_bytes(),raw)

    def test_unknown_receipt_foreign_recipe_or_partial_metadata_is_retained(self):
        with self.domain():self.prepare()
        state=self.read_state();state['phase']='active';BOOT.record(self.state,state)
        with self.assertRaisesRegex(ValueError,'unknown upgrade'):self.execute_pipeline()
        state['phase']='prepared';state['recipe_sha256']='f'*64;BOOT.record(self.state,state)
        with self.assertRaisesRegex(ValueError,'another generation'):self.execute_pipeline()
        self.state.unlink();BOOT.write(self.state.with_name('.'+self.state.name+'.tmp'),b'partial',0o600)
        with self.assertRaisesRegex(ValueError,'existing upgrade'):self.execute_pipeline()

    def test_native_guard_failure_after_fencing_stops_before_journal_mutation(self):
        calls=0
        def guard():
            nonlocal calls
            calls+=1
            if calls>=3:raise ValueError('unsafe native process')
        self.native.side_effect=guard
        with self.assertRaisesRegex(ValueError,'unsafe native'):self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'fenced');self.assertTrue(self.jf.source.exists())
        self.assertEqual(self.jf.writer.execute('SELECT remote FROM runs WHERE id=?',(KEEP.RUN,)).fetchone()[0],'pending')

    def test_file_failure_keeps_installing_and_recovers_known_rename(self):
        target=self.m.first_replace();real=os.replace
        def replace(source,destination):
            real(source,destination)
            if Path(destination)==Path(target):raise OSError('rename response interrupted')
        with patch.object(os,'replace',side_effect=replace),self.assertRaisesRegex(OSError,'rename response'):self.execute_pipeline()
        self.assertEqual(self.read_state()['phase'],'installing');self.assertFalse(self.jf.source.exists())
        self.assertEqual(self.execute_pipeline()['phase'],'installed-disabled')

    def test_forged_forward_jump_or_fence_identity_cannot_reopen_entries(self):
        with self.domain():self.prepare()
        state=self.read_state();state.update(phase='installed-disabled',entry_fence_sha256='f'*64);BOOT.record(self.state,state)
        with self.assertRaises(OSError):self.execute_pipeline()
        self.assertFalse(self.m.receipt.exists())

    def test_direct_cli_and_nonroot_context_are_blocked(self):
        with patch.object(L.os,'geteuid',return_value=1000):
            with self.assertRaisesRegex(ValueError,'administrator'):L.Context(self.B,*([None]*6))
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/coordinate-linux-upgrade.py')],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'no administrator entry',result.stderr)


if __name__=='__main__':unittest.main()
