"""One dispatch, durable bounded progress and detached system-manager arguments.

Native file/thread behavior runs under the test user; Root provenance/manager
effects are explicitly mocked. This is not a Root installation acceptance test.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

ROOT=Path(__file__).resolve().parents[3]
DEPLOY=ROOT/'tools/trusted_gate/deploy'


def load(name,file):
    spec=importlib.util.spec_from_file_location(name,DEPLOY/file)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


W=load('recovery_service_worker','recovery-worker.py')
S=load('recovery_service_launcher','start-interrupted-recovery.py')
RESUME=load('recovery_safe_summary','resume-interrupted-063.py')


class RecoveryServiceTests(unittest.TestCase):
    def setUp(self):
        previous=os.umask(0o077);self.addCleanup(os.umask,previous)
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.op=self.root/'operation';self.out=self.root/'runs'/'output'
        self.control=self.root/'control';self.source=self.root/'source';self.public=self.root/'progress.json'
        for path in (self.op,self.out,self.control,self.source):path.mkdir(mode=0o700,parents=True)
        for module in (W,S):
            for name,value in {'OPERATION':self.op,'OUTPUT':self.out}.items():self.patching(module,name,value)
        self.patching(W,'CONTROL',self.control);self.patching(W,'PUBLIC',self.public);self.patching(S,'SOURCE',self.source)

    def patching(self,module,name,value):
        guard=patch.object(module,name,value);guard.start();self.addCleanup(guard.stop)

    def protected(self,path,directory=False):
        info=path.lstat();W.need(not stat.S_ISLNK(info.st_mode) and not info.st_mode&0o022,'fixture protection')
        W.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink==1,'fixture type')
        return info

    def test_committed_source_matches_all_external_program_pins(self):
        files={'resume.py':'resume-interrupted-063.py','worker.py':'recovery-worker.py',
               'recovery.py':'recover-interrupted-installation.py'}
        for name,(_,pin) in S.PINS.items():
            self.assertEqual(hashlib.sha256((DEPLOY/files[name]).read_bytes()).hexdigest(),pin)
        self.assertEqual(W.RESUME_PIN,S.PINS['resume.py'][1]);self.assertEqual(W.RECOVERY_PIN,S.PINS['recovery.py'][1])

    def test_manager_service_is_detached_exec_with_no_restarts_or_terminal(self):
        args=S.command();self.assertEqual(args[0],'/usr/bin/systemd-run')
        for item in ('--wait','--pipe','--pty','--scope','--collect'):self.assertNotIn(item,args)
        for item in ('--service-type=exec','--remain-after-exit','--property=Restart=no',
                     '--property=StandardInput=null','--working-directory=/'):self.assertIn(item,args)
        self.assertEqual(args[-3:],['/usr/bin/python3.14','-I',str(self.op/'worker.py')])

    def test_existing_unit_never_repeats_dispatch_or_reads_source(self):
        with patch.object(S.os,'geteuid',return_value=0),patch.object(S,'manager',return_value={'LoadState':'loaded'}),\
             patch.object(S,'read') as read,patch.object(S.subprocess,'run') as run,patch('builtins.print'):
            S.main();read.assert_not_called();run.assert_not_called()

    def test_native_staging_preserves_exact_pins_private_modes_and_single_start(self):
        self.out.rmdir()
        for target,(name,pin) in S.PINS.items():
            file={'resume.py':'resume-interrupted-063.py','worker.py':'recovery-worker.py',
                  'recovery.py':'recover-interrupted-installation.py'}[target]
            (self.source/name).write_bytes((DEPLOY/file).read_bytes())
        with patch.object(S.os,'geteuid',return_value=0),patch.object(S,'inspect',
                side_effect=lambda p,root_only=True,directory=False:self.protected(p,directory)),\
             patch.object(S,'manager',side_effect=[{'LoadState':'not-found'},{'LoadState':'loaded'}]),\
             patch.object(S,'previous_failure'),\
             patch.object(S.subprocess,'run',return_value=SimpleNamespace(returncode=0)) as run,patch('builtins.print'):
            S.main();run.assert_called_once()
        for target,(_,pin) in S.PINS.items():
            path=self.op/target
            self.assertEqual(path.stat().st_mode&0o777,0o400);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),pin)
        self.assertEqual((self.out/'stdout.log').stat().st_mode&0o777,0o600)

    def test_unknown_staging_content_is_retained_and_blocks_start(self):
        (self.op/'unknown').write_bytes(b'retain')
        with patch.object(S.os,'geteuid',return_value=0),patch.object(S,'inspect',
                side_effect=lambda p,root_only=True,directory=False:self.protected(p,directory)),\
             patch.object(S,'manager',return_value={'LoadState':'not-found'}),\
             patch.object(S,'previous_failure'),\
             patch.object(S,'read',return_value=b''),patch.object(S,'PINS',{}),\
             patch.object(S.subprocess,'run') as run:
            with self.assertRaisesRegex(ValueError,'unknown private'):S.main()
            run.assert_not_called()
        self.assertEqual((self.op/'unknown').read_bytes(),b'retain')

    def test_progress_whitelists_fields_and_retains_only_private_result(self):
        value={'head':W.HEAD,'phase':'installing','files_done':[{}]*2,'inflight':{'kind':'file','path':'PRIVATE'},
               'private':'KEY-OR-TOKEN'}
        (self.control/'file-install.json').write_text(json.dumps(value))
        with patch.object(W,'protected',side_effect=self.protected):
            monitor=W.Monitor();monitor.update()
        progress=self.public.read_text();self.assertNotIn('PRIVATE',progress);self.assertNotIn('KEY-OR-TOKEN',progress)
        self.assertEqual(json.loads(progress)['progress']['files']['completed_files'],2)
        self.assertEqual(self.public.stat().st_mode&0o777,0o644)
        self.assertEqual((self.out/'progress.json').stat().st_mode&0o777,0o600)
        value['phase']='unexpected-secret';(self.control/'file-install.json').write_text(json.dumps(value))
        with patch.object(W,'protected',side_effect=self.protected):
            self.assertEqual(W.snapshot()['files'],{'read_error_type':'ValueError'})

    def test_checkpoint_progress_has_only_fixed_labels_and_no_authority_effect(self):
        with patch.object(W,'protected',side_effect=self.protected):
            monitor=W.Monitor();monitor.checkpoint('retained-core-verification');monitor.update()
            self.assertEqual(json.loads(self.public.read_bytes())['last_checkpoint'],'retained-core-verification')
            self.assertFalse(json.loads(self.public.read_bytes())['administrator_ready'])
            with self.assertRaisesRegex(ValueError,'unknown operator'):monitor.checkpoint('private-key-content')
            monitor.update();self.assertNotIn('private-key-content',self.public.read_text())

    def test_worker_runs_exactly_once_joins_observer_and_persists_blocked_result(self):
        wrapper=b'''import json
def literals(raw):pass
def main():
 dispatch(None,None)
 print(json.dumps({'status':'blocked','head':'063','private':'retained privately'}))
 return 1
'''
        recovery=b'''def dispatch(B,A):
 with open(__file__+'.called','a') as stream:stream.write('once')
'''
        (self.op/'resume.py').write_bytes(wrapper);(self.op/'recovery.py').write_bytes(recovery)
        with patch.object(W.os,'geteuid',return_value=0),patch.object(W,'protected',side_effect=self.protected),\
             patch.object(W,'RESUME_PIN',hashlib.sha256(wrapper).hexdigest()),\
             patch.object(W,'RECOVERY_PIN',hashlib.sha256(recovery).hexdigest()):
            self.assertEqual(W.main(),1)
        self.assertEqual((self.op/'recovery.py.called').read_text(),'once')
        self.assertEqual(json.loads((self.out/'result.json').read_bytes())['status'],'blocked')
        self.assertEqual(json.loads(self.public.read_bytes())['status'],'blocked')
        self.assertNotIn('retained privately',self.public.read_text())

    def test_invalid_resume_pin_blocks_before_dispatch(self):
        (self.op/'resume.py').write_bytes(b'changed')
        with patch.object(W.os,'geteuid',return_value=0),patch.object(W,'protected',side_effect=self.protected):
            with self.assertRaisesRegex(ValueError,'resume program changed'):W.main()
        self.assertFalse((self.out/'result.json').exists())

    def test_nonroot_launcher_never_contacts_manager(self):
        with patch.object(S.os,'geteuid',return_value=1000),patch.object(S,'manager') as manager:
            with self.assertRaises(SystemExit):S.main()
            manager.assert_not_called()

    def previous_result(self):
        self.previous=self.root/'previous';self.previous.mkdir(mode=0o700)
        self.patching(S,'PREVIOUS_OUTPUT',self.previous)
        self.old_manager={'LoadState':'loaded','ActiveState':'failed','SubState':'failed',
                          'Result':'exit-code','ExecMainStatus':'1'}
        self.old_result={'head':S.HEAD,'status':'blocked','dispatch_attempted':True,'automatic_retries':0,
            'failure':{'type':'Blocked','guard':'unlisted exception text omitted','helper_frames':[
                {'file':'<accepted659-github>','function':'request','line':42}]}}
        path=self.previous/'result.json';path.write_text(json.dumps(self.old_result));path.chmod(0o600)
        return path

    def test_explicit_new_attempt_requires_exact_stopped_previous_failure_and_retains_bytes(self):
        path=self.previous_result();raw=path.read_bytes()
        with patch.object(S,'manager',return_value=self.old_manager) as manager,\
             patch.object(S,'inspect',side_effect=lambda p,root_only=True,directory=False:self.protected(p,directory)):
            S.previous_failure();manager.assert_called_once_with(S.PREVIOUS_UNIT)
        self.assertEqual(path.read_bytes(),raw)
        self.assertNotEqual(S.UNIT,S.PREVIOUS_UNIT);self.assertNotEqual(S.OUTPUT,S.PREVIOUS_OUTPUT)

    def test_running_ambiguous_or_foreign_previous_failure_cannot_start_new_attempt(self):
        path=self.previous_result()
        with patch.object(S,'inspect',side_effect=lambda p,root_only=True,directory=False:self.protected(p,directory)):
            for key,value in (('LoadState','not-found'),('ActiveState','active'),('SubState','running'),
                              ('Result','success'),('ExecMainStatus','0')):
                manager={**self.old_manager,key:value}
                with patch.object(S,'manager',return_value=manager),self.assertRaises(ValueError):S.previous_failure()
            for key,value in (('head','f'*40),('status','component-complete'),('automatic_retries',1),
                              ('failure',{'type':'APIError','helper_frames':[]})):
                path.write_text(json.dumps({**self.old_result,key:value}))
                with patch.object(S,'manager',return_value=self.old_manager),self.assertRaises(ValueError):S.previous_failure()

    def test_prior_admission_or_changed_private_mode_is_not_rebound(self):
        path=self.previous_result()
        with patch.object(S,'manager',return_value=self.old_manager),\
             patch.object(S,'inspect',side_effect=lambda p,root_only=True,directory=False:self.protected(p,directory)):
            path.chmod(0o644)
            with self.assertRaises(ValueError):S.previous_failure()
            path.chmod(0o600);(self.previous/'readmission.json').write_text('retain')
            with self.assertRaisesRegex(ValueError,'unexpected earlier'):S.previous_failure()
        self.assertEqual((self.previous/'readmission.json').read_text(),'retain')

    def test_new_summary_only_admits_exact_static_path_guard(self):
        self.assertEqual(RESUME.failure(RuntimeError('invalid GitHub API path'))['guard'],'invalid GitHub API path')
        self.assertEqual(RESUME.failure(RuntimeError('private-token=never-display'))['guard'],
                         'unlisted exception text omitted')


if __name__=='__main__':unittest.main()
