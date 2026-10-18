"""Real private receipts/lease; preparation workers, Root admission and API mocked."""
from contextlib import redirect_stderr, redirect_stdout
import copy
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT/path)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


A = load('administrator_protocol', 'tools/trusted_gate/deploy/administrator-upgrade.py')
B = load('administrator_entry', 'tools/trusted_gate/deploy/import-protected-inputs.py')
F = load('administrator_lease', 'tools/trusted_gate/deploy/install-transaction.py')
I = load('administrator_metadata', 'tools/trusted_gate/deploy/install-reviewed-files.py')


class AdministratorDispatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name);self.control = self.root/'control';self.control.mkdir(mode=0o700)
        self.volume = self.root/'volume';self.volume.mkdir(mode=0o700)
        self.outputs = self.root/'outputs';self.outputs.mkdir(mode=0o700)
        self.lease = self.control/'controller.lock';B.write(self.lease,b'',0o600)
        self.fd = os.open(self.lease,os.O_RDWR);self.addCleanup(os.close,self.fd)
        self.B = SimpleNamespace(**{name:getattr(B,name) for name in ('need','canonical','unique','path_operand','write','record')})
        self.B.owned = self.owned;self.B.read = self.read
        self.B.authenticate = Mock(return_value={'revision':'a'*40})
        self.B.load = Mock(return_value=F)
        self.patch(A,'RECEIPT',self.control/'administrator.json');self.patch(A,'CONTROL',self.control)
        self.patch(A,'VOLUME',self.volume);self.patch(A,'BUNDLE',self.volume/'administrator-source.bundle')
        self.patch(A,'INPUT_RECIPE',self.volume/'installation-inputs.json');self.patch(F,'LEASE',self.lease)
        self.patch(I,'RECEIPT',self.control/'file-install.json');self.patch(F,'RECEIPT',self.control/'fence.json')
        self.patch(F,'BARRIER',self.control/'barrier.json')
        self.patch(A.os,'geteuid',lambda:0)
        self.calls = [];self.dispatches = [];self.ctx = SimpleNamespace(B=self.B,fd=self.fd,files=I,fence=F,
            code={'revision':'a'*40},manifest_pin='b'*64,recipe_pin='c'*64,helpers=self.root/'helpers',
            manifest=self.root/'manifest',value={'api_proxy':'http://proxy.example.invalid:7890'},
            lifecycle=SimpleNamespace(RECEIPT=self.control/'linux-upgrade.json'),
            activation=SimpleNamespace(RECEIPT=self.control/'activation.json'),
            preflight=SimpleNamespace(observe=Mock()),inputs=SimpleNamespace(recipe=Mock()))
        self.ctx.guard=self.guard;self.ctx.fresh=self.fresh;self.ctx.target=self.target
        self.ctx.perform=self.perform;self.ctx.verify=self.verify;self.ctx.dispatch=self.dispatch

    def patch(self, obj, name, value):
        context=patch.object(obj,name,value);context.start();self.addCleanup(context.stop)

    def owned(self,path,directory=False,private=False):
        path=Path(path);B.need(path==self.root or self.root in path.parents,'fixture containment')
        info=path.lstat();B.need(not stat.S_ISLNK(info.st_mode) and not info.st_mode&0o022,'fixture link/mode')
        B.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink==1,'fixture type')
        B.need(not private or not info.st_mode&0o077,'fixture private');return path

    def read(self,path,limit):
        raw=self.owned(path).read_bytes();B.need(len(raw)<=limit,'fixture bound');return raw

    def guard(self,**kwargs):F.lease(self.B,self.ctx.fd)
    def target(self,step):return A.INPUT_RECIPE if step=='recipe' else self.outputs/(step+'.json')
    def fresh(self,step):B.need(not A.exists(self.target(step)),'unknown preparation output')

    def perform(self,step,pins):
        state=self.state();self.assertEqual(state['inflight'],step)
        self.calls.append((step,self.ctx.fd))
        B.record(self.target(step),{'step':step,'head':'a'*40,'state':'complete'})

    def verify(self,step,pins,pin):
        raw=self.read(self.target(step),1024**2);B.need(A.digest(raw)==pin,'completed stage hash changed')
        B.need(json.loads(raw)=={'step':step,'head':'a'*40,'state':'complete'},'stage is partial or foreign')

    def dispatch(self,pin):
        self.dispatches.append((self.ctx.fd,pin));return {'phase':'entries-enabled'}

    def state(self):return json.loads(A.RECEIPT.read_bytes())

    def execute(self):
        if not A.exists(A.RECEIPT):A.prepare(self.ctx)
        return A.resume(self.ctx)

    def test_all_stage_intents_ordered_pins_and_one_original_fd_until_dispatch(self):
        result=self.execute();self.assertEqual(result['phase'],'complete')
        self.assertEqual([name for name,fd in self.calls],list(A.STEPS))
        self.assertEqual({fd for name,fd in self.calls},{self.fd});self.assertEqual(self.dispatches[0][0],self.fd)
        self.assertEqual([r['step'] for r in result['completed']],list(A.STEPS));self.assertIsNone(result['inflight'])
        self.assertFalse(result['administrator_ready']);self.assertFalse(result['production_gate_enabled'])
        self.ctx.preflight.observe.assert_called_once_with(self.B,self.ctx.helpers,self.ctx.manifest,'b'*64,
            lock_fd=self.fd,api_proxy='http://proxy.example.invalid:7890')

    def test_complete_retry_does_not_prepare_or_mutate_metadata(self):
        expected=self.execute();raw=A.RECEIPT.read_bytes()
        with patch.object(self.ctx,'perform',side_effect=AssertionError('repeat preparation')), \
             patch.object(self.B,'record',side_effect=AssertionError('metadata mutation')):
            self.assertEqual(self.execute(),expected)
        self.assertEqual(A.RECEIPT.read_bytes(),raw);self.assertEqual(len(self.dispatches),2)

    def test_complete_core_after_lost_response_is_adopted_without_second_build(self):
        real=self.ctx.perform
        def perform(step,pins):
            real(step,pins)
            if step=='core':raise OSError('build response lost')
        with patch.object(self.ctx,'perform',side_effect=perform),self.assertRaisesRegex(OSError,'response lost'):
            self.execute()
        self.assertEqual(self.state()['inflight'],'core')
        self.assertEqual(self.execute()['phase'],'complete')
        self.assertEqual(sum(name=='core' for name,fd in self.calls),1)

    def test_partial_failed_core_is_retained_and_never_rebuilt(self):
        real=self.ctx.perform
        def perform(step,pins):
            if step=='core':
                B.record(self.target(step),{'step':step,'state':'blocked'});raise OSError('Core failed')
            real(step,pins)
        with patch.object(self.ctx,'perform',side_effect=perform),self.assertRaisesRegex(OSError,'Core failed'):self.execute()
        raw=self.target('core').read_bytes()
        with patch.object(self.ctx,'perform',side_effect=AssertionError('rebuild')),self.assertRaisesRegex(ValueError,'partial or foreign'):
            self.execute()
        self.assertEqual(self.target('core').read_bytes(),raw);self.assertEqual(self.dispatches,[])

    def test_prior_completed_artifact_change_blocks_before_next_effect(self):
        real=self.ctx.perform
        def perform(step,pins):
            if step=='input':raise OSError('before import')
            real(step,pins)
        with patch.object(self.ctx,'perform',side_effect=perform),self.assertRaises(OSError):self.execute()
        self.target('source').write_bytes(b'changed')
        with patch.object(self.ctx,'perform',side_effect=AssertionError('effect')),self.assertRaisesRegex(ValueError,'hash changed'):
            self.execute()

    def test_record_failure_after_complete_output_resumes_only_matching_intent(self):
        real=self.B.record
        def record(path,value):
            if path==A.RECEIPT and len(value['completed'])==4:raise OSError('checkpoint unavailable')
            real(path,value)
        with patch.object(self.B,'record',side_effect=record),self.assertRaises(OSError):self.execute()
        self.assertEqual(self.state()['inflight'],'input');self.execute()
        self.assertEqual(sum(name=='input' for name,fd in self.calls),1)

    def test_unknown_preexisting_outputs_block_before_transaction(self):
        B.record(self.target('core'),{'foreign':True})
        with self.assertRaisesRegex(ValueError,'unknown preparation output'):self.execute()
        self.assertFalse(A.RECEIPT.exists());self.assertEqual(self.calls,[])

    def test_output_appearing_after_initial_check_still_needs_matching_stage_intent(self):
        A.prepare(self.ctx);B.record(self.target('core'),{'step':'core','head':'a'*40,'state':'complete'})
        with self.assertRaisesRegex(ValueError,'unknown preparation output'):A.resume(self.ctx)
        self.assertNotIn('core',[name for name,fd in self.calls]);self.assertIsNone(self.state()['inflight'])

    def test_partial_artifact_metadata_blocks_completed_dispatch_replay(self):
        self.execute();target=self.target('core')
        B.write(target.with_name('.'+target.name+'.tmp'),b'partial next state',0o600)
        with self.assertRaisesRegex(ValueError,'unfinished preparation artifact'):self.execute()
        self.assertEqual(len(self.dispatches),1)

    def test_existing_file_fence_or_activation_blocks_fresh_administrator_prepare(self):
        with patch.object(I,'RECEIPT',self.control/'file-install.json'),patch.object(F,'RECEIPT',self.control/'fence.json'), \
             patch.object(F,'BARRIER',self.control/'barrier.json'):
            B.record(F.BARRIER,{'foreign':True})
            with self.assertRaisesRegex(ValueError,'existing installation'):self.execute()
        self.assertFalse(A.RECEIPT.exists())

    def test_forged_prefix_skip_phase_or_foreign_generation_cannot_dispatch(self):
        A.prepare(self.ctx);good=self.state()
        for mutate in (lambda v:v.update(phase='complete'),lambda v:v.update(recipe_sha256='f'*64),
                       lambda v:v.update(completed=[{'step':'core','sha256':'a'*64}]),
                       lambda v:v.update(inflight='core'),lambda v:v.update(transaction='not-uuid')):
            value=copy.deepcopy(good);mutate(value);B.record(A.RECEIPT,value)
            with self.assertRaises((ValueError,KeyError)):A.resume(self.ctx)
        self.assertEqual(self.dispatches,[])

    def test_partial_administrator_metadata_is_retained(self):
        B.write(A.RECEIPT.with_name('.'+A.RECEIPT.name+'.tmp'),b'partial',0o600)
        with self.assertRaisesRegex(ValueError,'existing administrator'):self.execute()
        self.assertFalse(A.RECEIPT.exists())

    def test_dispatch_response_loss_retries_dispatch_without_preparation(self):
        with patch.object(self.ctx,'dispatch',side_effect=OSError('activation response lost')),self.assertRaises(OSError):self.execute()
        self.assertEqual(self.state()['phase'],'dispatching')
        with patch.object(self.ctx,'perform',side_effect=AssertionError('preparation repeat')):self.execute()

    def test_run_authenticates_before_load_and_holds_original_lease_until_return(self):
        order=[];self.B.authenticate.side_effect=lambda *args:order.append('authenticate')
        self.B.load.side_effect=lambda *args:(order.append('load') or F)
        seen=[];real=self.ctx.perform
        def factory(*args):self.ctx.fd=args[-1];seen.append(args[-1]);return self.ctx
        def perform(step,pins):
            other=os.open(self.lease,os.O_RDWR)
            try:
                with self.assertRaises(BlockingIOError):fcntl.flock(other,fcntl.LOCK_EX|fcntl.LOCK_NB)
            finally:os.close(other)
            real(step,pins)
        with patch.object(A,'Context',side_effect=factory),patch.object(self.ctx,'perform',side_effect=perform):
            result=A.run(self.B,self.ctx.helpers,self.ctx.manifest,'b'*64,self.root/'recipe','c'*64)
        self.assertEqual(order[:2],['authenticate','load']);self.assertEqual(result['phase'],'complete')
        self.assertEqual({fd for name,fd in self.calls},set(seen))
        with self.assertRaises(OSError):os.fstat(seen[0])

    def test_busy_lease_or_authentication_failure_precedes_context(self):
        F.lease(self.B,self.fd)
        with patch.object(A,'Context',side_effect=AssertionError('context')),self.assertRaises(BlockingIOError):
            A.run(self.B,self.ctx.helpers,self.ctx.manifest,'b'*64,self.root/'recipe','c'*64)
        self.B.authenticate.side_effect=ValueError('auth unavailable')
        with patch.object(A,'Context',side_effect=AssertionError('context')),self.assertRaisesRegex(ValueError,'auth unavailable'):
            A.run(self.B,self.ctx.helpers,self.ctx.manifest,'b'*64,self.root/'recipe','c'*64)


class AdministratorRecipeTests(unittest.TestCase):
    def setUp(self):
        self.f=AdministratorDispatchTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        self.root,self.B=self.f.root,self.f.B;inputs=self.root/'inputs';inputs.mkdir(mode=0o700)
        paths={}
        for name in A.PATHS:
            path=inputs/name
            if name in A.PINS:B.write(path,b'protected recipe input',0o400)
            else:path.mkdir(mode=0o700)
            paths[name]=str(path)
        self.value={'schema':'gatehouse-administrator-upgrade/v1','head':'a'*40,
            'helper_manifest_sha256':'b'*64,'paths':paths,'pins':{name:'c'*64 for name in A.PINS},
            'api_proxy':'http://proxy.example.invalid:7890'}
        self.path=inputs/'administrator.json';self.save()

    def save(self):
        if self.path.exists():self.path.chmod(0o600);self.path.unlink()
        raw=B.canonical(self.value);B.write(self.path,raw,0o400);self.pin=A.digest(raw)

    def read(self):return A.recipe(self.B,self.path,self.pin,'a'*40,'b'*64)

    def test_exact_closed_sealed_recipe_has_no_arbitrary_output_or_pass_fields(self):
        self.assertEqual(self.read(),self.value)
        for field in ('destination','core_binary','quality_passed'):
            self.value[field]=True;self.save()
            with self.assertRaisesRegex(ValueError,'identity or inventory'):self.read()
            self.value.pop(field)

    def test_wrong_head_manifest_pin_and_mode_reject(self):
        self.path.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'sealed'):self.read()
        self.path.chmod(0o400)
        with self.assertRaisesRegex(ValueError,'sealed'):A.recipe(self.B,self.path,'f'*64,'a'*40,'b'*64)
        for field in ('head','helper_manifest_sha256'):
            old=self.value[field];self.value[field]='f'*len(old);self.save()
            with self.assertRaisesRegex(ValueError,'identity or inventory'):self.read()
            self.value[field]=old

    def test_duplicate_fields_missing_path_or_non_digest_pin_reject(self):
        self.path.chmod(0o600);self.path.write_bytes(b'{"head":"a","head":"b"}\n');self.path.chmod(0o400)
        self.pin=A.digest(self.path.read_bytes())
        with self.assertRaisesRegex(ValueError,'duplicate'):self.read()
        self.value['paths'].pop('onboarding');self.save()
        with self.assertRaisesRegex(ValueError,'inventory'):self.read()

    def test_input_link_and_deployment_overlap_reject(self):
        path=Path(self.value['paths']['release']);raw=path.read_bytes();path.unlink()
        other=path.with_name('other');B.write(other,raw,0o400);path.symlink_to(other)
        with self.assertRaisesRegex(ValueError,'fixture link'):self.read()
        path.unlink();B.write(path,raw,0o400);self.value['paths']['onboarding']=str(self.f.volume);self.save()
        with self.assertRaisesRegex(ValueError,'overlaps mutable'):self.read()


class AdministratorEntryTests(unittest.TestCase):
    def test_authenticated_cli_emits_only_public_summary(self):
        value={'schema':'gatehouse-administrator-dispatch/v1','head':'a'*40,'phase':'complete',
               'administrator_ready':False,'production_gate_enabled':False,'private':'never printed'}
        dispatcher=SimpleNamespace(run=Mock(return_value=value));out=io.StringIO()
        argv=['entry','administrator-upgrade','--helper-root','/root/helpers','--manifest','/root/manifest',
              '--manifest-sha256','a'*64,'--recipe','/root/recipe','--recipe-sha256','b'*64]
        with patch.object(B.sys,'argv',argv),patch.object(B,'authenticate') as auth, \
             patch.object(B,'load',return_value=dispatcher),redirect_stdout(out):B.main()
        auth.assert_called_once();self.assertNotIn('private',out.getvalue());self.assertNotIn('never printed',out.getvalue())
        context=dispatcher.run.call_args.args[0];self.assertIs(context.import_inputs,B.import_inputs)
        self.assertEqual(json.loads(out.getvalue())['phase'],'complete')

    def test_cli_authentication_error_does_not_import_or_print_sensitive_exception(self):
        argv=['entry','administrator-upgrade','--helper-root','/root/helpers','--manifest','/root/manifest',
              '--manifest-sha256','a'*64,'--recipe','/root/recipe','--recipe-sha256','b'*64]
        err=io.StringIO()
        with patch.object(B.sys,'argv',argv),patch.object(B,'authenticate',side_effect=ValueError('secret diagnostic')), \
             patch.object(B,'load',side_effect=AssertionError('helper import')),redirect_stderr(err),self.assertRaises(SystemExit):B.main()
        self.assertNotIn('secret diagnostic',err.getvalue());self.assertIn('retain private state',err.getvalue())


class AdministratorWiringTests(unittest.TestCase):
    def setUp(self):
        self.f=AdministratorDispatchTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        self.ctx=SimpleNamespace(**vars(self.f.ctx));self.ctx.pins={name:'d'*64 for name in A.PINS}
        self.ctx.p={name:str(self.f.root/name) for name in A.PATHS};self.ctx.payload={'source.bundle':b'captured source bundle'}
        self.ctx.preservation=self.f.root/'retained';self.ctx.contract=SimpleNamespace(BASE='e'*40)
        for name in ('volume','keep','staging','sealer','builder','compose','checker'):
            setattr(self.ctx,name,SimpleNamespace(**{method:Mock() for method in
                ('prepare','preserve','stage','seal','build','compose','inspect','verify_completed')}))
        self.ctx.B.import_inputs=Mock();self.pins={name:'e'*64 for name in A.STEPS}
        self.ctx.installation_recipe=lambda pins:A.Context.installation_recipe(self.ctx,pins)

    def test_preparation_calls_use_typed_fixed_outputs_and_same_fd(self):
        for step in A.STEPS:A.Context.perform(self.ctx,step,self.pins)
        for actor,method in (('volume','prepare'),('keep','preserve'),('staging','stage'),('sealer','seal'),
                             ('builder','build'),('compose','compose'),('checker','inspect')):
            call=getattr(getattr(self.ctx,actor),method).call_args
            self.assertEqual(call.args[-1],self.f.fd)
        self.assertEqual(self.ctx.compose.compose.call_args.kwargs,{'api_proxy':self.ctx.value['api_proxy']})
        self.assertEqual(self.ctx.checker.inspect.call_args.kwargs,{'api_proxy':self.ctx.value['api_proxy']})
        self.assertEqual(self.ctx.B.import_inputs.call_args.args[-1],A.VOLUME/'bootstrap-inputs')
        self.assertEqual(A.BUNDLE.read_bytes(),self.ctx.payload['source.bundle'])
        value=json.loads(A.INPUT_RECIPE.read_bytes());self.assertEqual(set(value['pins']),
            {'archive','host','volume_recipe','input','seal','core','configuration','inspection','preservation'})
        self.assertEqual(stat.S_IMODE(A.INPUT_RECIPE.stat().st_mode),0o400)

    def test_activation_receipt_dispatch_skips_historical_lifecycle_resume(self):
        result={'phase':'entries-enabled','head':'a'*40,'administrator_ready':False,'production_gate_enabled':False}
        self.ctx.lifecycle.coordinate=Mock();self.ctx.activation.activate=Mock(return_value=result)
        A.Context.dispatch(self.ctx,'f'*64)
        shared=self.ctx.activation.activate.call_args.args;self.assertEqual(shared[-1],self.f.fd)
        self.ctx.lifecycle.coordinate.assert_called_once_with(*shared)
        B.record(self.ctx.activation.RECEIPT,{'phase':'marker-intent'})
        with patch.object(self.ctx.lifecycle,'coordinate',side_effect=AssertionError('historical fenced replay')):
            self.assertEqual(A.Context.dispatch(self.ctx,'f'*64),result)

    def test_dispatch_rejects_wrong_head_partial_result_and_ready_claim(self):
        self.ctx.lifecycle.coordinate=Mock()
        for change in ({'head':'f'*40},{'phase':'internal-intent'},{'administrator_ready':True},{'production_gate_enabled':True}):
            result={'phase':'entries-enabled','head':'a'*40,'administrator_ready':False,'production_gate_enabled':False,**change}
            self.ctx.activation.activate=Mock(return_value=result)
            with self.assertRaisesRegex(ValueError,'did not complete'):A.Context.dispatch(self.ctx,'f'*64)

    def test_direct_context_nonroot_rejects_before_authentication_or_load(self):
        with patch.object(A.os,'geteuid',return_value=1000),patch.object(self.ctx.B,'authenticate',side_effect=AssertionError('auth')):
            with self.assertRaisesRegex(ValueError,'native administrator'):A.Context(self.ctx.B,*([None]*6))


class AdministratorActivationIntegrationTests(unittest.TestCase):
    def test_real27_file_snapshot_activation_and_completed_dispatch_replay(self):
        fixture=load('administrator_activation_fixture','tools/quality/tests/test_trusted_gate_activation.py')
        f=fixture.ForwardActivationTests();f.setUp();self.addCleanup(f.doCleanups)
        def activate(*args):
            self.assertEqual(args[-1],f.ctx.fd)
            if not fixture.A.RECEIPT.exists():fixture.A.prepare(f.ctx)
            return fixture.A.resume(f.ctx)
        ctx=SimpleNamespace(B=f.B,helpers=f.ctx.helpers,manifest=f.ctx.manifest,manifest_pin=f.ctx.manifest_pin,
            fd=f.ctx.fd,code=f.ctx.code,lifecycle=SimpleNamespace(coordinate=Mock()),
            activation=SimpleNamespace(RECEIPT=f.receipt,activate=activate))
        with patch.object(A,'INPUT_RECIPE',f.f.root/'sealed-inputs.json'):
            self.assertEqual(A.Context.dispatch(ctx,'a'*64)['phase'],'entries-enabled')
            with patch.object(ctx.lifecycle,'coordinate',side_effect=AssertionError('old phase replay')):
                self.assertEqual(A.Context.dispatch(ctx,'a'*64)['phase'],'entries-enabled')
        for path in f.f.entries:
            self.assertEqual(stat.S_IMODE(path.stat().st_mode),0o755)
            self.assertEqual(stat.S_IMODE(os.fstat(f.f.entry_fds[path]).st_mode),0)


if __name__=='__main__':unittest.main()
