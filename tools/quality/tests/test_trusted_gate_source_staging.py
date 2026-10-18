"""Offline source staging does not release or modify the accepted pilot slot."""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, path):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


FIX = load('source_staging_fixture',ROOT/'tools/quality/tests/test_trusted_gate_preservation.py')
STAGE = load('source_staging_library',ROOT/'tools/trusted_gate/deploy/stage-protected-source.py')
BOOT = FIX.BOOT


class SourceStagingTests(unittest.TestCase):
    def setUp(self):
        self.fixture=FIX.PreservationTests(); self.fixture.setUp(); self.addCleanup(self.fixture.doCleanups)
        self.root,self.pilot,self.B=self.fixture.root,self.fixture.volume,self.fixture.B
        self.B.ENV=BOOT.ENV
        self.old_head=self.fixture.head
        self.volume=self.root/'linux'; self.volume.mkdir()
        self.author=self.root/'author'
        subprocess.run(['/usr/bin/git','clone','--no-hardlinks',str(self.root/'seed'),str(self.author)],
                       env=BOOT.ENV,check=True,capture_output=True)
        (self.author/'candidate.txt').write_text('reviewed candidate\n')
        self.commit()
        self.helpers=self.root/'helpers'
        name='tools/harness-gate/Cargo.lock'
        helper=self.helpers/name; helper.parent.mkdir(parents=True); helper.write_bytes((self.author/name).read_bytes())
        self.code={'revision':self.head,'files':{name:hashlib.sha256(helper.read_bytes()).hexdigest()}}
        self.B.authenticate=lambda *args:self.code
        self.builder=SimpleNamespace(pilot_slot=lambda B:None,quiet_units=lambda B:None,idle=lambda B:None)
        self.volume_api=SimpleNamespace(admit=lambda *args:{'status':'ready'})
        self.B.load=lambda root,name:self.builder if name=='build-protected-core.py' else self.volume_api
        self.bundle=self.root/'candidate.bundle'
        self.pack()
        freeze=self.pilot/'repository/frozen.json'
        value=json.loads(freeze.read_bytes()); value.update(repository='musutrade/GateHouse',repository_id=1414069625)
        freeze.write_bytes(BOOT.canonical(value))

    def git(self,*args):
        return subprocess.check_output(['/usr/bin/git','-C',str(self.author),*args],env=BOOT.ENV,stderr=subprocess.PIPE)

    def commit(self):
        self.git('add','-A')
        self.git('-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','candidate')
        self.head=self.git('rev-parse','HEAD').decode().strip()

    def pack(self):
        if self.bundle.exists(): self.bundle.unlink()
        self.git('bundle','create',str(self.bundle),'HEAD','^'+self.old_head)
        self.pin=hashlib.sha256(self.bundle.read_bytes()).hexdigest()
        self.code['revision']=self.head

    def stage(self,**kwargs):
        with patch.multiple(STAGE,PILOT=self.pilot,VOLUME=self.volume,OLD_HEAD=self.old_head,BASE=self.old_head), \
             patch.object(BOOT,'owned',side_effect=self.fixture.owned),patch.object(STAGE.os,'geteuid',return_value=0):
            return STAGE.stage(self.B,self.helpers,self.root/'manifest','a'*64,self.bundle,self.pin,self.head,
                               self.root/'recipe','b'*64,**kwargs)

    def blocked(self):
        self.assertEqual(json.loads((self.volume/'bootstrap-source/stage.json').read_bytes())['state'],'blocked')

    def test_real_prerequisite_bundle_stages_clean_source_and_preserves_old_slot(self):
        freeze=(self.pilot/'repository/frozen.json').read_bytes()
        value=self.stage()
        self.assertEqual(value['state'],'staged'); self.assertFalse(value['production_gate_enabled'])
        self.assertEqual((self.volume/'bootstrap-source/source/candidate.txt').read_text(),'reviewed candidate\n')
        self.assertEqual((self.pilot/'repository/frozen.json').read_bytes(),freeze)
        self.assertEqual(self.fixture.writer.execute('SELECT local,remote FROM runs').fetchone(),('failed','pending'))
        self.assertEqual(self.fixture.git('rev-parse','HEAD').decode().strip(),self.old_head)

    def test_new_git_store_has_no_hardlinks_into_old_store(self):
        self.stage()
        mirror=self.volume/'bootstrap-source/mirror.git'
        for path in mirror.rglob('*'):
            if path.is_file(): self.assertEqual(path.stat().st_nlink,1)

    def test_wrong_bundle_pin_blocks_before_output(self):
        self.pin='0'*64
        with self.assertRaises(ValueError):self.stage()
        self.assertFalse((self.volume/'bootstrap-source').exists())

    def test_changed_bundle_advertisement_is_blocked(self):
        self.bundle.unlink(); self.git('bundle','create',str(self.bundle),'--all')
        self.pin=hashlib.sha256(self.bundle.read_bytes()).hexdigest()
        with self.assertRaises(ValueError):self.stage()
        self.blocked()

    def test_symlink_source_is_rejected_before_worktree(self):
        (self.author/'link').symlink_to('/etc/passwd'); self.commit(); self.pack()
        with self.assertRaises(ValueError):self.stage()
        self.blocked(); self.assertFalse((self.volume/'bootstrap-source/source').exists())

    def test_submodule_source_is_rejected_before_worktree(self):
        self.git('update-index','--add','--cacheinfo','160000,'+self.old_head+',submodule')
        self.git('-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','submodule')
        self.head=self.git('rev-parse','HEAD').decode().strip(); self.pack()
        with self.assertRaises(ValueError):self.stage()
        self.blocked()

    def test_reserved_overlay_source_is_rejected(self):
        path=self.author/'.gatehouse-host/forged'; path.parent.mkdir(); path.write_text('forged')
        self.commit(); self.pack()
        with self.assertRaises(ValueError):self.stage()
        self.blocked()

    def test_authenticated_helper_blob_mismatch_blocks_checkout(self):
        self.code['files']['tools/harness-gate/Cargo.lock']='0'*64
        with self.assertRaises(ValueError):self.stage()
        self.blocked(); self.assertFalse((self.volume/'bootstrap-source/source').exists())

    def test_changed_retained_repository_or_base_is_not_adopted(self):
        freeze=self.pilot/'repository/frozen.json'; value=json.loads(freeze.read_bytes()); value['repository_id']=1
        freeze.write_bytes(BOOT.canonical(value))
        with self.assertRaises(ValueError):self.stage()
        self.assertFalse((self.volume/'bootstrap-source').exists())

    def test_partial_stage_is_retained_without_retry_reuse(self):
        path=self.volume/'bootstrap-source'; path.mkdir(); (path/'partial').touch()
        with self.assertRaises(ValueError):self.stage()
        self.assertTrue((path/'partial').exists())

    def test_unknown_storage_or_active_old_slot_blocks_creation(self):
        def blocked(*args):raise ValueError('unknown')
        self.volume_api.admit=blocked
        with self.assertRaises(ValueError):self.stage()
        self.assertFalse((self.volume/'bootstrap-source').exists())
        self.volume_api.admit=lambda *args:{}; self.builder.pilot_slot=blocked
        with self.assertRaises(ValueError):self.stage()
        self.assertFalse((self.volume/'bootstrap-source').exists())

    def test_inherited_lock_is_not_released_and_competitor_is_blocked(self):
        with (self.pilot/'control/controller.lock').open('r+b') as stream:
            fcntl.flock(stream.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):self.stage()
            self.stage(lock_fd=stream.fileno())
            with (self.pilot/'control/controller.lock').open('r+b') as rival:
                with self.assertRaises(BlockingIOError):fcntl.flock(rival.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)

    def test_source_limits_block_oversized_candidate(self):
        with patch.object(STAGE,'MAX_SOURCE',1):
            with self.assertRaises(ValueError):self.stage()
        self.blocked()

    def test_actual_git_output_over_budget_is_blocked(self):
        with patch.object(STAGE,'MAX_REPLY',1):
            with self.assertRaises(ValueError):self.stage()
        self.blocked()

    def test_staging_executes_only_local_git_with_clean_environment(self):
        original=subprocess.run; observed=[]
        def checked(argv,**kwargs):
            self.assertEqual(argv[0],'/usr/bin/git'); self.assertEqual(kwargs['env'],BOOT.ENV)
            self.assertFalse(any(str(arg).startswith(('http:','https:','ext:')) for arg in argv))
            observed.append(argv); return original(argv,**kwargs)
        with patch.object(subprocess,'run',side_effect=checked): self.stage()
        self.assertTrue(observed)

    def test_real_nonroot_entry_rejects_without_staging(self):
        if os.geteuid()==0:self.skipTest('requires ordinary user')
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),
            'stage-source','--helper-root',str(self.helpers),'--manifest',str(self.root/'missing'),
            '--manifest-sha256','a'*64,'--bundle',str(self.bundle),'--bundle-sha256',self.pin,'--head',self.head,
            '--tool-recipe',str(self.root/'recipe'),'--tool-recipe-sha256','b'*64],capture_output=True)
        self.assertNotEqual(result.returncode,0); self.assertIn(b'source staging blocked',result.stderr)
        self.assertFalse((self.volume/'bootstrap-source').exists())


if __name__=='__main__':unittest.main()
