"""Real ordinary file transactions keep both delegated entries disabled.

Root uid/DAC, fixed deployment paths and coordinator admission are replaced only
inside this private fixture. Actual mode changes, identities, exclusive writes,
fsync, atomic replacement, durable receipts and failure recovery are exercised.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX = load('file_install_fence_fixture', 'tools/quality/tests/test_trusted_gate_install_transaction.py')
RELEASE = load('file_install_release_fixture', 'tools/quality/tests/test_trusted_gate_bootstrap_release.py')
I = load('file_install_library', 'tools/trusted_gate/deploy/install-reviewed-files.py')
R = RELEASE.R;BOOT = FIX.BOOT;FENCE = FIX.T


class ReviewedFileTransactionTests(unittest.TestCase):
    def setUp(self):
        self.f = FIX.InstallationFenceTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        f = self.f;self.extra_fds = {};self.addCleanup(lambda: [os.close(fd) for fd in self.extra_fds.values()])
        def location(target):
            if target == '/opt/gatehouse/bin/gatehouse-control':return str(f.entries[0])
            if target == '/etc/gatehouse/policy/gatehouse-step':return str(f.entries[1])
            if target.startswith('/opt/gatehouse/linux-test/'):return str(f.root/'linux-test'/Path(target).name)
            return str(f.root/'installed'/target.lstrip('/'))
        source = {name: (location(target), mode, action) for name, (target, mode, action) in R.source_map().items()}
        old = {name: (location(target), mode, action) for name, (target, mode, action) in R.old_source_map().items()}
        self.contract = SimpleNamespace(source_map=lambda: source, old_source_map=lambda: old)
        self.core = f.root/'harness-gate';self.receipt = f.root/'file-install.json'
        directories = {f.root/'linux-test':0o755, f.root/'scratch':0o700, f.root/'cache':0o700}
        for key, value in {'CORE':self.core,'CONFIG':f.config,'ENTRIES':f.entries,
                           'DIRECTORIES':directories,'RECEIPT':self.receipt}.items():
            context = patch.object(I, key, value);context.start();self.addCleanup(context.stop)
        for name in ('identity','directory_identity'):
            original = getattr(I, name)
            context = patch.object(I, name, side_effect=lambda info, original=original: {**original(info),'uid':0})
            context.start();self.addCleanup(context.stop)
        real_chmod = os.fchmod
        def chmod(fd, mode):
            if mode == 0 and stat.S_ISREG(os.fstat(fd).st_mode):
                # Reopen our own still-readable temporary before mode000, then
                # retain the descriptor to emulate Root reads without reopening.
                info = os.fstat(fd);key = (info.st_dev,info.st_ino)
                if key not in self.extra_fds:
                    self.extra_fds[key] = os.open('/proc/self/fd/'+str(fd), os.O_RDONLY)
            return real_chmod(fd, mode)
        context = patch.object(os, 'fchmod', side_effect=chmod);context.start();self.addCleanup(context.stop)
        original_read = f.B.read
        def read(path, limit):
            path = f.owned(path);info = path.stat();key = (info.st_dev,info.st_ino)
            if key in self.extra_fds:
                fd = self.extra_fds[key];os.lseek(fd,0,os.SEEK_SET);raw = os.read(fd,limit+1)
                f.B.need(len(raw)<=limit,'fixture read bound');return raw
            return original_read(path,limit)
        f.B.read = read;f.B.path_operand = BOOT.path_operand;f.B.write = BOOT.write
        previous = {target: (name, mode) for name, (target, mode, action) in old.items()}
        files = {}
        for name, (target, mode, action) in source.items():
            before = ('old:'+previous[target][0]).encode() if target in previous else None
            before_mode = previous[target][1] if target in previous else None
            if Path(target) in f.entries:before = f.B.read(Path(target),1024**2);mode=before_mode=0
            after = before if action == 'retain' else ('new:'+name).encode()
            files[target] = {'before':before,'before_mode':before_mode,'after':after,'mode':mode,'action':action}
            if before is not None and Path(target) not in f.entries:
                path = Path(target);path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
                BOOT.write(path,before,before_mode)
        files[str(self.core)] = {'before':b'old protected Core','before_mode':0o755,
            'after':b'new captured Core','mode':0o555,'action':'replace'}
        BOOT.write(self.core,files[str(self.core)]['before'],0o755)
        files[str(f.config)] = {'before':f.config.read_bytes(),'before_mode':0o600,
            'after':BOOT.canonical({'candidate':f.head,'transition':True}),'mode':0o600,'action':'replace'}
        self.materials = {'head':f.head,'files':files};self.guard = Mock(return_value=True)
        f.configuration_pin = I.digest(files[str(f.config)]['after']);f.prepare();self.original = f.fence()

    def prepare(self):return I.prepare(self.f.B,FENCE,self.contract,self.materials,self.f.lease_fd,self.guard)
    def resume(self):return I.resume(self.f.B,FENCE,self.contract,self.materials,self.f.lease_fd,self.guard)
    def read_state(self):return json.loads(self.receipt.read_bytes())
    def save_state(self, value):BOOT.record(self.receipt,value)
    def first_replace(self):
        return next(target for target in I.file_order(self.materials)
                    if self.materials['files'][target]['before']!=self.materials['files'][target]['after'])

    def crash_after_intent(self, target):
        original = self.f.B.record
        def record(path,value):
            original(path,value)
            if path == self.receipt and value['inflight'] == {'kind':'file','path':target}:
                raise OSError('crash after durable file intent')
        with patch.object(self.f.B,'record',side_effect=record),self.assertRaisesRegex(OSError,'durable file intent'):
            self.resume()

    def crash_after_rename(self, target):
        real = os.replace
        def replace(source,destination):
            real(source,destination)
            if Path(destination)==Path(target):raise OSError('crash after file rename')
        with patch.object(os,'replace',side_effect=replace),self.assertRaisesRegex(OSError,'file rename'):
            self.resume()

    def test_complete27_file_transaction_retains_original_controls_disabled_and_never_activates(self):
        retained = {path:Path(path).stat().st_ino for path,row in self.materials['files'].items() if row['action']=='retain'}
        barrier = self.f.barrier.read_bytes();self.prepare();result = self.resume()
        self.assertEqual(result['phase'],'installed-disabled');self.assertEqual(len(result['files_done']),27)
        for path,row in self.materials['files'].items():
            self.assertEqual(self.f.B.read(Path(path),I.MAX_FILE),row['after'])
            self.assertEqual(stat.S_IMODE(Path(path).stat().st_mode),row['mode'])
        self.assertEqual({path:Path(path).stat().st_ino for path in retained},retained)
        self.assertEqual(self.f.barrier.read_bytes(),barrier)
        for path in self.f.entries:
            self.assertEqual(stat.S_IMODE(path.stat().st_mode),0)
            self.assertEqual(stat.S_IMODE(os.fstat(self.f.entry_fds[path]).st_mode),0)
        for flag in ('entries_enabled','administrator_ready','production_gate_enabled'):self.assertIs(result[flag],False)
        self.assertGreater(self.guard.call_count,50)

    def test_completed_retry_is_readonly(self):
        self.prepare();expected = self.resume();before = self.receipt.read_bytes()
        with patch.object(self.f.B,'write',side_effect=AssertionError('unexpected write')), \
             patch.object(self.f.B,'record',side_effect=AssertionError('unexpected record')):
            self.assertEqual(self.resume(),expected)
        self.assertEqual(self.receipt.read_bytes(),before)

    def test_wrong_or_false_coordinator_checkpoint_blocks_without_mutation(self):
        for value in (None,False,1):
            self.guard.return_value=value
            with self.assertRaisesRegex(ValueError,'coordinator checkpoint'):self.prepare()
            self.assertFalse(self.receipt.exists())

    def test_missing_extra_materials_or_wrong_modes_are_rejected_before_intent(self):
        original=copy.deepcopy(self.materials);key=self.first_replace()
        self.materials['files'].pop(key)
        with self.assertRaisesRegex(ValueError,'27-file'):self.prepare()
        self.materials=copy.deepcopy(original);self.materials['files'][key]['mode']=True
        with self.assertRaisesRegex(ValueError,'fields differ'):self.prepare()
        self.materials=copy.deepcopy(original);self.materials['files']['/tmp/unreviewed']=self.materials['files'][key]
        with self.assertRaisesRegex(ValueError,'27-file'):self.prepare()
        self.assertFalse(self.receipt.exists())

    def test_wrong_fenced_head_configuration_or_entry_bytes_rejects_intent(self):
        self.materials['head']='f'*40
        with self.assertRaisesRegex(ValueError,'identity or phase'):self.prepare()
        self.materials['head']=self.f.head;row=self.materials['files'][str(self.f.config)];row['after']=b'other config'
        with self.assertRaisesRegex(ValueError,'configuration differs'):self.prepare()
        self.assertFalse(self.receipt.exists())

    def test_prepare_retains_existing_receipt_or_unfinished_metadata(self):
        self.prepare();before=self.receipt.read_bytes()
        with self.assertRaisesRegex(ValueError,'existing file transaction'):self.prepare()
        self.assertEqual(self.receipt.read_bytes(),before)
        self.receipt.unlink();BOOT.write(self.receipt.with_name('.'+self.receipt.name+'.tmp'),b'private intent',0o600)
        with self.assertRaisesRegex(ValueError,'existing file transaction'):self.prepare()

    def test_crash_after_initial_intent_resumes_forward(self):
        real=self.f.B.record
        def record(path,value):
            real(path,value)
            if path==self.receipt:raise OSError('crash after initial intent')
        with patch.object(self.f.B,'record',side_effect=record),self.assertRaises(OSError):self.prepare()
        self.assertEqual(self.read_state()['phase'],'intent');self.assertEqual(self.resume()['phase'],'installed-disabled')

    def test_crash_after_directory_creation_adopts_only_its_empty_known_intent(self):
        self.prepare();real=I.sync_parent;target=sorted(I.DIRECTORIES)[0]
        def sync(path):
            real(path)
            if path==target:raise OSError('crash after mkdir fsync')
        with patch.object(I,'sync_parent',side_effect=sync),self.assertRaisesRegex(OSError,'mkdir fsync'):self.resume()
        self.assertTrue(target.is_dir());self.assertEqual(self.read_state()['inflight'],{'kind':'directory','path':str(target)})
        self.assertEqual(self.resume()['phase'],'installed-disabled')

    def test_unrecorded_directory_or_unknown_contents_cannot_be_reused(self):
        self.prepare();path=sorted(I.DIRECTORIES)[0];path.mkdir(mode=I.DIRECTORIES[path])
        with self.assertRaisesRegex(ValueError,'unrecorded installation directory'):self.resume()
        self.assertTrue(path.exists())

    def test_crash_after_file_intent_before_write_resumes(self):
        self.prepare();target=self.first_replace();self.crash_after_intent(target)
        self.assertEqual(self.f.B.read(Path(target),I.MAX_FILE),self.materials['files'][target]['before'])
        self.assertEqual(self.resume()['phase'],'installed-disabled')

    def test_crash_after_complete_temp_keeps_it_and_blocks_automatic_reconciliation(self):
        self.prepare();target=self.first_replace();real=self.f.B.write
        temp=I.temporary(Path(target),self.f.transaction)
        def write(path,raw,mode):
            real(path,raw,mode)
            if path==temp:raise OSError('crash after temporary fsync')
        with patch.object(self.f.B,'write',side_effect=write),self.assertRaisesRegex(OSError,'temporary fsync'):self.resume()
        self.assertEqual(temp.read_bytes(),self.materials['files'][target]['after'])
        with self.assertRaisesRegex(ValueError,'retain partial'):self.resume()
        self.assertTrue(temp.exists());self.assertEqual(self.f.B.read(Path(target),I.MAX_FILE),self.materials['files'][target]['before'])

    def test_crash_after_rename_adopts_exact_new_file_under_its_durable_intent(self):
        self.prepare();target=self.first_replace();self.crash_after_rename(target)
        self.assertEqual(self.read_state()['inflight'],{'kind':'file','path':target})
        self.assertEqual(self.f.B.read(Path(target),I.MAX_FILE),self.materials['files'][target]['after'])
        self.assertEqual(self.resume()['phase'],'installed-disabled')

    def test_changed_renamed_file_is_retained_and_never_overwritten(self):
        self.prepare();target=self.first_replace();self.crash_after_rename(target)
        path=Path(target);path.chmod(0o600);path.write_bytes(b'unknown replacement');path.chmod(self.materials['files'][target]['mode'])
        before=self.receipt.read_bytes()
        with self.assertRaises(ValueError):self.resume()
        self.assertEqual(path.read_bytes(),b'unknown replacement');self.assertEqual(self.receipt.read_bytes(),before)

    def test_recovered_rename_must_sync_parent_before_recording_completion(self):
        self.prepare();target=self.first_replace();self.crash_after_rename(target)
        before=self.receipt.read_bytes();real=I.sync_parent
        def sync(path):
            if path==Path(target):raise OSError('directory sync unavailable')
            return real(path)
        with patch.object(I,'sync_parent',side_effect=sync),self.assertRaisesRegex(OSError,'sync unavailable'):
            self.resume()
        self.assertEqual(self.receipt.read_bytes(),before)
        self.assertEqual(self.resume()['phase'],'installed-disabled')

    def test_fresh_temporary_changed_at_checkpoint_blocks_rename_and_retains_bytes(self):
        self.prepare();target=self.first_replace();temp=I.temporary(Path(target),self.f.transaction)
        def guard():
            if temp.exists():
                temp.chmod(0o600);temp.write_bytes(b'changed at checkpoint')
                temp.chmod(self.materials['files'][target]['mode'])
            return True
        self.guard.side_effect=guard
        with self.assertRaisesRegex(ValueError,'bytes/mode changed'):self.resume()
        self.assertEqual(temp.read_bytes(),b'changed at checkpoint')
        self.assertEqual(self.f.B.read(Path(target),I.MAX_FILE),self.materials['files'][target]['before'])

    def test_core_larger_than_source_file_limit_is_installed_without_truncation(self):
        raw=b'captured Core\n'*(1024**2//13+1)
        self.assertGreater(len(raw),1024**2)
        self.materials['files'][str(self.core)]['after']=raw
        self.prepare();self.resume()
        self.assertEqual(self.core.read_bytes(),raw)

    def test_entry_rename_recovery_keeps_old_and_new_descriptors_disabled(self):
        self.prepare();target=str(self.f.entries[1]);self.crash_after_rename(target)
        self.assertEqual(stat.S_IMODE(Path(target).stat().st_mode),0)
        self.assertEqual(stat.S_IMODE(os.fstat(self.f.entry_fds[Path(target)]).st_mode),0)
        self.assertEqual(self.resume()['phase'],'installed-disabled')

    def test_interrupted_directory_with_unknown_contents_is_retained_and_rejected(self):
        self.prepare();real=I.sync_parent;target=sorted(I.DIRECTORIES)[0]
        def sync(path):
            real(path)
            if path==target:raise OSError('crash after directory creation')
        with patch.object(I,'sync_parent',side_effect=sync),self.assertRaises(OSError):self.resume()
        unknown=target/'unknown';BOOT.write(unknown,b'private unknown',0o600)
        before=self.receipt.read_bytes()
        with self.assertRaisesRegex(ValueError,'directory creation is unknown'):self.resume()
        self.assertEqual(unknown.read_bytes(),b'private unknown');self.assertEqual(self.receipt.read_bytes(),before)

    def test_new_bytes_on_future_file_without_intent_are_not_adopted(self):
        self.prepare();target=self.first_replace();row=self.materials['files'][target];path=Path(target)
        path.chmod(0o600);path.write_bytes(row['after']);path.chmod(row['mode'])
        with self.assertRaisesRegex(ValueError,'bytes/mode changed'):self.resume()

    def test_completed_identity_or_progress_cannot_be_rewritten(self):
        self.prepare();self.resume();state=self.read_state();path=Path(state['files_done'][0]['path'])
        raw=path.read_bytes();mode=path.stat().st_mode&0o777;temp=path.with_name('unrelated-new-inode')
        BOOT.write(temp,raw,mode);os.replace(temp,path)
        with self.assertRaisesRegex(ValueError,'identity changed'):self.resume()

    def test_out_of_order_progress_and_forged_completion_are_rejected(self):
        self.prepare();state=self.read_state();state['phase']='installed-disabled';self.save_state(state)
        with self.assertRaisesRegex(ValueError,'incomplete'):self.resume()
        state['phase']='installing';state['files_done']=[{'path':I.file_order(self.materials)[1],'identity':{}}];self.save_state(state)
        with self.assertRaisesRegex(ValueError,'ordered prefix'):self.resume()

    def test_mutated_material_descriptor_or_retained_bytes_blocks(self):
        self.prepare();target=self.first_replace();self.materials['files'][target]['after']=b'changed material'
        with self.assertRaisesRegex(ValueError,'identity or phase'):self.resume()
        self.materials['files'][target]['after']=('new:'+next(name for name,row in self.contract.source_map().items() if row[0]==target)).encode()
        retained=next(Path(path) for path,row in self.materials['files'].items() if row['action']=='retain')
        mode=retained.stat().st_mode&0o777;retained.chmod(0o600);retained.write_bytes(b'changed retained input');retained.chmod(mode)
        with self.assertRaisesRegex(ValueError,'bytes/mode changed'):self.resume()

    def test_barrier_active_or_entry_reenabled_cannot_resume(self):
        self.prepare();raw=self.f.barrier.read_bytes();value=json.loads(raw);value['phase']='active'
        self.f.barrier.write_bytes(BOOT.canonical(value))
        with self.assertRaisesRegex(ValueError,'changed or advanced'):self.resume()
        self.f.barrier.write_bytes(raw);self.f.entries[0].chmod(0o755)
        with self.assertRaises(ValueError):self.resume()
        self.assertEqual(self.f.entries[0].stat().st_mode&0o777,0o755)

    def test_unknown_temporary_symlink_or_exposed_metadata_is_retained(self):
        self.prepare();target=self.first_replace();temp=I.temporary(Path(target),self.f.transaction);temp.symlink_to('missing')
        with self.assertRaisesRegex(ValueError,'retain partial'):self.resume()
        self.assertTrue(temp.is_symlink());temp.unlink();self.receipt.chmod(0o644)
        with self.assertRaises(ValueError):self.resume()

    def test_wrong_controller_lease_and_mid_operation_checkpoint_failure_block(self):
        self.prepare()
        with self.f.config.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError,'lease'):
                I.resume(self.f.B,FENCE,self.contract,self.materials,wrong.fileno(),self.guard)
        self.guard.side_effect=ValueError('remote applicability changed')
        before=self.receipt.read_bytes()
        with self.assertRaisesRegex(ValueError,'applicability changed'):self.resume()
        self.assertEqual(self.receipt.read_bytes(),before)

    def test_direct_execution_exposes_no_administrator_cli(self):
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/install-reviewed-files.py')],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'no administrator entry',result.stderr)


class ReviewedFileMaterialTests(unittest.TestCase):
    def setUp(self):
        self.f=RELEASE.BootstrapReleaseTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        self.old_core=b'accepted old Core';self.core=b'captured new Core'
        self.old=BOOT.canonical({'protected_files':{str(I.CORE):I.digest(self.old_core)}})
        self.config={'environment':{'source_revision':self.f.head},'protected_files':{
            target:I.digest(self.f.payload['files/'+name]) for name,(target,mode,action) in R.source_map().items() if action!='retain'}}
        self.config['protected_files'][str(I.CORE)]=I.digest(self.core)

    def materialize(self,**kwargs):
        return I.materialize(self.f.B,R,self.f.payload,self.f.head,kwargs.get('old_core',self.old_core),
            kwargs.get('core',self.core),self.old,kwargs.get('new_raw',BOOT.canonical(self.config)))

    def test_source_release_maps_exact27_files_and_keeps_both_entries_zero_mode(self):
        materials=self.materialize();self.assertEqual(len(materials['files']),27)
        for path in I.ENTRIES:self.assertEqual(materials['files'][str(path)]['mode'],0)
        self.assertEqual(materials['files'][str(I.CORE)]['mode'],0o555)
        self.assertEqual(materials['files'][str(I.CONFIG)]['mode'],0o600)
        for row in materials['files'].values():
            if row['action']=='retain':self.assertEqual(row['before'],row['after'])

    def test_wrong_core_old_core_or_configuration_pins_are_rejected(self):
        for args in ({'core':b'operator Core'},{'old_core':b'unknown old Core'},{'new_raw':b'{"not":"canonical"}'}):
            with self.assertRaises((ValueError,KeyError)):self.materialize(**args)
        self.config['environment']['source_revision']='f'*40
        with self.assertRaisesRegex(ValueError,'material pins'):self.materialize()

    def test_rehashed_source_release_cannot_change_fixed_old_reference_inventory(self):
        name=next(iter(R.old_sources()));self.f.payload['before/'+name]=b'changed old reference';self.f.seal()
        with self.assertRaisesRegex(ValueError,'source inventory differs'):self.materialize()


if __name__=='__main__':unittest.main()
