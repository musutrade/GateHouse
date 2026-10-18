"""Data capture cannot retire the pilot, bless mutable data or reuse partial state."""
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT=Path(__file__).resolve().parents[3]


def load(name,path):
    s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m


FIX=load('linux_seal_fixture',ROOT/'tools/quality/tests/test_trusted_gate_linux_inputs.py')
BOOT=load('linux_seal_entry',ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py')
SEAL=load('linux_seal_library',ROOT/'tools/trusted_gate/deploy/seal-linux-inputs.py')
STAGE=load('linux_seal_stage',ROOT/'tools/trusted_gate/deploy/stage-linux-inputs.py')
DIRECTORY=load('linux_seal_directory',ROOT/'tools/trusted_gate/deploy/prepare-locked-directory.py')


class LinuxInputSealingTests(unittest.TestCase):
    def setUp(self):
        self.fixture=FIX.LinuxInputBindingTests();self.fixture.setUp();self.addCleanup(self.fixture.doCleanups)
        self.root=self.fixture.root;self.source=self.fixture.source
        self.control=self.root/'control';self.control.mkdir();(self.control/'controller.lock').touch(mode=0o600)
        self.volume=self.root/'linux';self.volume.mkdir(mode=0o700)
        self.destination=self.volume/'bootstrap-linux-inputs'
        self.inputs=self.root/'imported'
        (self.inputs/'dependencies').mkdir(parents=True)
        shutil.copytree(self.fixture.roles['directory_sources'],self.inputs/'dependencies/sources')
        self.archives=self.fixture.roles['test_data_archives'];self.onboarding=self.fixture.roles['onboarding']
        self.git('init','-q');self.commit()
        self.B=SimpleNamespace(**{name:getattr(BOOT,name) for name in (
            'need','path_operand','read','write','record','canonical','unique','git','frozen_lock','readonly','ENV')})
        self.B.owned=self.owned;self.B.authenticate=Mock(return_value={'revision':self.head})
        self.builder=SimpleNamespace(pilot_slot=Mock(),quiet_units=Mock(),idle=Mock(),imported_inputs=Mock(
            side_effect=lambda *args:{'lock_sha256':SEAL.digest(self.fixture.lock.read_bytes())}))
        self.volume_api=SimpleNamespace(admit=Mock(return_value={'status':'ready'}))
        self.B.load=lambda root,file:{'build-protected-core.py':self.builder,'prepare-linux-volume.py':self.volume_api,
            'stage-linux-inputs.py':STAGE,'prepare-locked-directory.py':DIRECTORY}[file]

    def git(self,*args):
        return subprocess.check_output(['/usr/bin/git','-C',str(self.source),*args],env=BOOT.ENV,stderr=subprocess.PIPE)

    def commit(self):
        self.git('add','-A');self.git('-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','source facts')
        self.head=self.git('rev-parse','HEAD').decode().strip()

    def owned(self,path,directory=False,private=False):
        p=Path(path)
        if p!=self.root and self.root not in p.parents:return BOOT.owned(p,directory=directory,private=private)
        for parent in (p,*p.parents):BOOT.need(not parent.is_symlink(),'linked fixture path')
        i=p.lstat();BOOT.need(stat.S_ISDIR(i.st_mode) if directory else stat.S_ISREG(i.st_mode),'fixture type')
        BOOT.need(directory or i.st_nlink==1,'fixture hardlink')
        BOOT.need(not i.st_mode&0o022 and (not private or not i.st_mode&0o077),'fixture mode')
        return p

    def seal(self,**kwargs):
        original=BOOT.owned
        # Avoid recursion when checking the real OS Git binary outside fixture.
        def owned(path,**options):
            p=Path(path)
            if p!=self.root and self.root not in p.parents:return original(p,**options)
            return self.owned(p,**options)
        with patch.multiple(SEAL,CONTROL=self.control,VOLUME=self.volume,BASE=self.head), \
             patch.object(BOOT,'owned',side_effect=owned),patch.object(SEAL.os,'geteuid',return_value=0), \
             patch.object(STAGE.JOB,'ONBOARDING_SHA256',self.fixture.onboarding_pin):
            return SEAL.seal(self.B,self.root/'helpers',self.root/'manifest','a'*64,self.source,self.head,
                self.inputs,'b'*64,self.archives,self.onboarding,self.root/'recipe','c'*64,**kwargs)

    def blocked(self):
        self.assertEqual(json.loads((self.destination/'seal.json').read_bytes())['state'],'blocked')

    def test_real_git_hash_copy_and_seal_roundtrip_through_controller_input_verifier(self):
        before={p:p.read_bytes() for p in [self.fixture.lock,self.fixture.manifest]}
        result=self.seal()
        self.assertEqual(result['state'],'sealed');self.assertFalse(result['administrator_ready'])
        self.assertEqual(result['files'],4);self.assertFalse(result['production_gate_enabled'])
        config={**self.fixture.config,'linux_inputs':result['linux_inputs'],'cargo_config':result['cargo_config'],
                'protected_files':{**self.fixture.config['protected_files'],**result['protected_files']}}
        with patch.object(FIX.RUNTIME,'protected',side_effect=self.fixture.protected), \
             patch.object(FIX.RUNTIME,'ONBOARDING_SHA256',self.fixture.onboarding_pin):
            self.assertEqual(FIX.RUNTIME.verify(config)['linux_inputs']['packages'],1)
        for p,raw in before.items():self.assertEqual(p.read_bytes(),raw)
        self.assertEqual(self.git('status','--porcelain'),b'')
        record=json.loads((self.destination/'seal.json').read_bytes())
        self.assertEqual(record['binding_sha256'],SEAL.digest((self.destination/'role-binding.json').read_bytes()))

    def test_authentication_and_revision_mismatch_block_before_creation(self):
        self.B.authenticate.return_value={'revision':'f'*40}
        with self.assertRaises(ValueError):self.seal()
        self.assertFalse(self.destination.exists())

    def test_unknown_volume_or_running_worker_or_old_slot_blocks_before_creation(self):
        for api,name in [(self.volume_api,'admit'),(self.builder,'pilot_slot'),(self.builder,'quiet_units'),(self.builder,'idle')]:
            function=getattr(api,name);function.side_effect=ValueError('blocked boundary')
            with self.assertRaises(ValueError):self.seal()
            self.assertFalse(self.destination.exists());function.side_effect=None

    def test_wrong_import_binding_proof_blocks_before_output(self):
        self.builder.imported_inputs.side_effect=ValueError('wrong imported pin')
        with self.assertRaises(ValueError):self.seal()
        self.assertFalse(self.destination.exists())

    def test_manifest_not_in_git_or_changed_lock_cannot_select_captured_bytes(self):
        for path in [self.fixture.lock,self.fixture.manifest]:
            old=path.read_bytes();path.write_bytes(old+b'changed')
            with self.assertRaises(ValueError):self.seal()
            self.assertFalse(self.destination.exists());path.write_bytes(old)

    def test_partial_or_completed_destination_is_never_reused(self):
        self.seal()
        with self.assertRaises(ValueError):self.seal()
        self.assertEqual(json.loads((self.destination/'seal.json').read_bytes())['state'],'sealed')

    def test_archive_digest_or_size_drift_retains_blocked_private_output(self):
        path=next(self.archives.iterdir());path.chmod(0o600);path.write_bytes(b'changed');path.chmod(0o444)
        with self.assertRaises(ValueError):self.seal()
        self.blocked();self.assertTrue(self.source.exists())

    def test_onboarding_rehash_or_extra_files_cannot_override_reviewed_source_pin(self):
        self.onboarding.chmod(0o700);(self.onboarding/'extra').write_bytes(b'changed');self.onboarding.chmod(0o555)
        with self.assertRaises(ValueError):self.seal()
        self.blocked();self.assertTrue((self.destination/'onboarding/extra').exists())

    def test_onboarding_symlinks_hardlinks_and_special_files_cannot_be_copied(self):
        self.onboarding.chmod(0o700);path=self.onboarding/'unsafe'
        for kind in ['symlink','hardlink','fifo']:
            if kind=='symlink':path.symlink_to(self.fixture.lock)
            elif kind=='hardlink':os.link(self.onboarding/'tools/onboarding/acceptance.py',path)
            else:os.mkfifo(path)
            with self.assertRaises(ValueError):self.seal()
            self.blocked();path.unlink()
            self.destination.rename(self.volume/('failed-'+kind))

    def test_empty_extra_directory_cannot_be_sealed_as_unreviewed_tree_data(self):
        self.onboarding.chmod(0o700);(self.onboarding/'empty-extra').mkdir();self.onboarding.chmod(0o555)
        with self.assertRaisesRegex(ValueError,'empty/extra'):self.seal()
        self.blocked()

    def test_combined_byte_entry_or_metadata_bounds_do_not_seal_success(self):
        for name,value in [('MAX_TOTAL',1),('MAX_MEMBERS',1),('MAX_METADATA',1)]:
            with patch.object(SEAL,name,value),self.assertRaises(ValueError):self.seal()
            if self.destination.exists():
                self.blocked();self.destination.rename(self.volume/('failed-'+name))

    def test_snapshot_file_change_during_capture_is_retained_blocked(self):
        real_identity=SEAL.identity;calls=[]
        def changed(info):
            row=real_identity(info);calls.append(row)
            return row if len(calls)==1 else ('changed',)
        with patch.object(SEAL,'identity',side_effect=changed),self.assertRaises(ValueError):self.seal()
        self.blocked()

    def test_source_changes_during_capture_cannot_seal(self):
        original=SEAL.copy_tree
        def changed(*args):
            result=original(*args);self.fixture.manifest.write_bytes(b'changed');return result
        with patch.object(SEAL,'copy_tree',side_effect=changed),self.assertRaises(ValueError):self.seal()
        self.blocked()

    def test_imported_input_drift_at_postcheck_cannot_seal(self):
        self.builder.imported_inputs.side_effect=[{'lock_sha256':SEAL.digest(self.fixture.lock.read_bytes())},ValueError('drift')]
        with self.assertRaises(ValueError):self.seal()
        self.blocked()

    def test_captured_file_or_metadata_mutation_cannot_seal(self):
        original=self.B.read
        def changed(path,limit):
            raw=original(path,limit)
            return b'changed' if Path(path).name=='linux-inputs.json' else raw
        self.B.read=changed
        with self.assertRaises(ValueError):self.seal()
        self.blocked()

    def test_pre_and_post_guards_and_authentication_are_required(self):
        self.seal()
        self.assertEqual(self.B.authenticate.call_count,2)
        self.assertEqual(self.builder.imported_inputs.call_count,2)
        self.assertEqual(self.builder.pilot_slot.call_count,2)
        self.assertEqual(self.volume_api.admit.call_count,2)

    def test_inherited_exact_lease_remains_held_and_wrong_descriptor_is_rejected(self):
        lease=self.control/'controller.lock'
        with lease.open('r+b') as stream:
            fcntl.flock(stream.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):self.seal()
            self.seal(lock_fd=stream.fileno());os.fstat(stream.fileno())
            with lease.open('r+b') as rival:
                with self.assertRaises(BlockingIOError):fcntl.flock(rival.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
        with self.fixture.lock.open('rb') as stream:
            with self.assertRaisesRegex(ValueError,'lease'):self.seal(lock_fd=stream.fileno())

    def test_real_nonroot_cli_rejects_before_credentials_or_output(self):
        if os.geteuid()==0:self.skipTest('requires actual ordinary user')
        args=[sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),'seal-linux-inputs']
        for name in ['helper-root','manifest','source','inputs','archives','onboarding','tool-recipe']:args+=['--'+name,str(self.root/'missing')]
        for name in ['manifest-sha256','input-binding-sha256','tool-recipe-sha256']:args+=['--'+name,'a'*64]
        args+=['--head',self.head]
        result=subprocess.run(args,capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'Linux input sealing blocked',result.stderr)
        self.assertFalse(self.destination.exists())


if __name__=='__main__':unittest.main()
