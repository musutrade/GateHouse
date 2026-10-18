"""Actual ordinary fchmod/receipts/lease recovery; Root admission/DAC replaced only."""
import errno
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import uuid

ROOT = Path(__file__).resolve().parents[3]

def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools/trusted_gate/deploy' / filename)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module

T = load('installation_fence_fixture', 'install-transaction.py')
BOOT = load('installation_record_fixture', 'import-protected-inputs.py')


class InstallationFenceTests(unittest.TestCase):
    def setUp(self):
        mask = os.umask(0o077);self.addCleanup(os.umask, mask)
        self.temporary = tempfile.TemporaryDirectory();self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.entries = tuple(self.root / name for name in ('gatehouse-control', 'gatehouse-step'))
        self.entry_fds = {}
        for path in self.entries:
            path.write_bytes(b'#!/usr/bin/python3.14\nraise SystemExit(23)\n');path.chmod(0o755)
            self.entry_fds[path] = os.open(path, os.O_RDONLY)
        self.addCleanup(lambda: [os.close(fd) for fd in self.entry_fds.values()])
        self.receipt = self.root / 'entry-fence.json';self.barrier = self.root / 'installation.json'
        self.config = self.root / 'host.json';self.config.write_bytes(b'{"old_configuration":true}\n')
        self.lease_path = self.root / 'controller.lock';self.lease_path.touch(mode=0o600)
        self.lease_fd = os.open(self.lease_path, os.O_RDWR);self.addCleanup(os.close, self.lease_fd)
        for key,value in {'ENTRIES':self.entries,'RECEIPT':self.receipt,'BARRIER':self.barrier,
                          'CONFIG':self.config,'LEASE':self.lease_path}.items():
            p=patch.object(T,key,value);p.start();self.addCleanup(p.stop)
        self.real_identity=T.identity
        def identity(info):
            return {**self.real_identity(info),'uid':0}
        p=patch.object(T,'identity',side_effect=identity);p.start();self.addCleanup(p.stop)
        p=patch.object(os,'geteuid',return_value=0);p.start();self.addCleanup(p.stop)
        # Root can read a mode000 entry. Keep ordinary descriptors to emulate only
        # that DAC ability; the actual fchmod, fsync, inode and flock operations run.
        real_open=os.open
        def root_open(path,flags,*args,**kwargs):
            p=Path(path) if isinstance(path,(str,Path)) else None
            if p in self.entry_fds and flags == os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK:
                old=os.fstat(self.entry_fds[p]);current=p.lstat()
                if (old.st_dev,old.st_ino)==(current.st_dev,current.st_ino):
                    fd=os.dup(self.entry_fds[p]);os.lseek(fd,0,os.SEEK_SET);return fd
            return real_open(path,flags,*args,**kwargs)
        p=patch.object(os,'open',side_effect=root_open);p.start();self.addCleanup(p.stop)
        self.B=SimpleNamespace(need=BOOT.need,canonical=BOOT.canonical,unique=BOOT.unique,
                               owned=self.owned,read=self.read,record=BOOT.record)
        self.pins={str(path):hashlib.sha256(self.read(path,T.MAX_FILE)).hexdigest() for path in self.entries}
        self.transaction=str(uuid.uuid4());self.head='a'*40;self.configuration_pin='b'*64

    def owned(self,path,directory=False,private=False):
        path=Path(path)
        self.B.need(path==self.root or self.root in path.parents,'outside ordinary fixture')
        for parent in (path,*path.parents):
            if parent!=self.root and self.root not in parent.parents:continue
            info=parent.lstat();self.B.need(not stat.S_ISLNK(info.st_mode) and not info.st_mode&0o022,'unsafe fixture path')
        info=path.stat()
        self.B.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink==1,'unsafe fixture type')
        self.B.need(not private or not info.st_mode&0o077,'exposed fixture state')
        return path

    def read(self,path,limit):
        path=self.owned(path)
        with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK),'rb') as stream:
            raw=stream.read(limit+1)
        self.B.need(len(raw)<=limit,'fixture file exceeds budget')
        return raw

    def prepare(self):
        return T.prepare(self.B,self.transaction,self.head,self.configuration_pin,self.pins,self.lease_fd)

    def fence(self):
        return T.fence(self.B,self.lease_fd)

    def test_fence_changes_original_inodes_and_never_authorizes_installation(self):
        old={p:p.stat().st_ino for p in self.entries}
        self.prepare();result=self.fence()
        self.assertEqual(result['phase'],'fenced')
        self.assertIs(result['controller_drain_proven'],False)
        self.assertIs(result['administrator_ready'],False)
        self.assertEqual({p:p.stat().st_ino for p in self.entries},old)
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0 for p in self.entries))
        self.assertEqual(json.loads(self.barrier.read_bytes())['phase'],'fenced')
        for fd in self.entry_fds.values():self.assertEqual(stat.S_IMODE(os.fstat(fd).st_mode),0)
        def records():
            return [(T.identity(path.stat()),path.stat().st_mode,path.stat().st_ctime_ns,
                     hashlib.sha256(self.read(path,64*1024)).hexdigest()) for path in (self.barrier,self.receipt)]
        before=records()
        self.assertEqual(self.fence(),result)
        self.assertEqual(records(),before)

    def test_interruption_after_receipt_before_barrier_resumes_without_overwrite(self):
        real=self.B.record
        def crash(path,value):
            real(path,value)
            if path==self.receipt:raise OSError('fixture crash after durable intent')
        self.B.record=crash
        with self.assertRaises(OSError):self.prepare()
        self.assertFalse(self.barrier.exists())
        self.B.record=real
        self.assertEqual(self.fence()['phase'],'fenced')

    def test_interruption_after_first_inode_permission_change_resumes_original_receipt(self):
        self.prepare();real=os.fchmod;calls=[]
        def crash(fd,mode):
            real(fd,mode)
            if mode==0:
                calls.append(fd)
                if len(calls)==1:raise OSError('fixture crash after first chmod')
        with patch.object(os,'fchmod',side_effect=crash),self.assertRaises(OSError):self.fence()
        self.assertEqual([stat.S_IMODE(p.stat().st_mode) for p in self.entries],[0,0o755])
        self.assertEqual(self.fence()['phase'],'fenced')

    def test_interruption_after_durable_barrier_before_final_receipt_resumes(self):
        self.prepare();real=self.B.record
        def crash(path,value):
            real(path,value)
            if path==self.barrier and value['phase']=='fenced':raise OSError('fixture crash after barrier')
        self.B.record=crash
        with self.assertRaises(OSError):self.fence()
        self.B.record=real
        self.assertEqual(self.fence()['phase'],'fenced')

    def test_all_entries_verified_before_any_mutation(self):
        self.prepare();self.entries[1].write_bytes(b'unknown entry')
        with self.assertRaises(ValueError):self.fence()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_replaced_inode_same_bytes_is_rejected(self):
        self.prepare();path=self.entries[1];raw=path.read_bytes()
        other=self.root/'replacement';other.write_bytes(raw);other.chmod(0o755);other.replace(path)
        with self.assertRaises(ValueError):self.fence()
        self.assertEqual(stat.S_IMODE(self.entries[0].stat().st_mode),0o755)

    def test_completed_fence_cannot_be_reopened_or_rolled_back(self):
        self.prepare();value=self.fence();self.entries[0].chmod(0o755)
        with self.assertRaises(ValueError):self.fence()
        self.entries[0].chmod(0)
        self.B.record(self.barrier,T.marker(self.B,value,'fencing'))
        with self.assertRaises(ValueError):self.fence()

    def test_unknown_advanced_or_missing_barrier_retains_fenced_state(self):
        self.prepare();value=self.fence()
        for phase in ('active','retired','unknown'):
            self.B.record(self.barrier,T.marker(self.B,value,phase))
            with self.subTest(phase=phase),self.assertRaises(ValueError):self.fence()
        self.barrier.unlink()
        with self.assertRaises(ValueError):self.fence()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0 for p in self.entries))

    def test_unknown_receipt_fields_and_boolean_identity_alias_rejected(self):
        original=self.prepare()
        for mutate in (lambda v:v.update(extra=True),lambda v:v.update(controller_drain_proven=True),
                       lambda v:v['entries'][str(self.entries[0])]['identity'].update(ino=True)):
            value=json.loads(json.dumps(original));mutate(value);self.B.record(self.receipt,value)
            with self.assertRaises(ValueError):self.fence()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_changed_configuration_blocks_before_permission_mutation(self):
        self.prepare();self.config.write_bytes(b'changed config')
        with self.assertRaises(ValueError):self.fence()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_repeated_prepare_never_overwrites_receipt_or_marker(self):
        self.prepare();before=self.receipt.read_bytes(),self.barrier.read_bytes()
        with self.assertRaises(ValueError):self.prepare()
        self.assertEqual((self.receipt.read_bytes(),self.barrier.read_bytes()),before)

    def test_wrong_lease_descriptor_and_nonroot_rejected_without_receipt(self):
        fd=os.open(self.config,os.O_RDONLY)
        try:
            with self.assertRaises(ValueError):T.prepare(self.B,self.transaction,self.head,self.configuration_pin,self.pins,fd)
        finally:os.close(fd)
        with patch.object(os,'geteuid',return_value=1000),self.assertRaises(ValueError):self.prepare()
        self.assertFalse(self.receipt.exists())

    def test_wrong_expected_pins_and_extra_entry_not_admitted(self):
        for pins in ({**self.pins,'/unexpected':'c'*64},{**self.pins,str(self.entries[0]):'c'*64}):
            with self.assertRaises(ValueError):T.prepare(self.B,self.transaction,self.head,self.configuration_pin,pins,self.lease_fd)
        self.assertFalse(self.receipt.exists())

    def test_record_modes_and_marker_bind_exact_transaction_plan(self):
        value=self.prepare()
        self.assertEqual(stat.S_IMODE(self.receipt.stat().st_mode),0o600)
        self.assertEqual(stat.S_IMODE(self.barrier.stat().st_mode),0o600)
        marker=json.loads(self.barrier.read_bytes())
        self.assertEqual(marker,T.marker(self.B,value,'fencing'))
        value['entries'][str(self.entries[0])]['sha256']='f'*64
        self.assertNotEqual(marker['plan_sha256'],T.marker(self.B,value,'fencing')['plan_sha256'])

    def test_busy_real_lease_prevents_any_record_or_permission_mutation(self):
        other=os.open(self.lease_path,os.O_RDWR)
        try:
            fcntl.flock(other,fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):self.prepare()
        finally:os.close(other)
        self.assertFalse(self.receipt.exists());self.assertFalse(self.barrier.exists())
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_replacement_between_observation_and_descriptor_open_is_rejected(self):
        self.prepare();real=os.open
        def replace(path,flags,*args,**kwargs):
            fd=real(path,flags,*args,**kwargs)
            if Path(path)==self.entries[0] and flags==os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK:
                replacement=self.root/'raced-entry';replacement.write_bytes(b'changed');replacement.chmod(0o755)
                replacement.replace(path)
            return fd
        with patch.object(os,'open',side_effect=replace),self.assertRaises(ValueError):self.fence()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_symlink_hardlink_and_writable_entry_stop_prepare(self):
        path=self.entries[1];raw=path.read_bytes();alias=self.root/'alias'
        os.link(path,alias)
        with self.assertRaises(ValueError):self.prepare()
        alias.unlink();path.chmod(0o777)
        with self.assertRaises(ValueError):self.prepare()
        path.chmod(0o755);path.unlink();alias.write_bytes(raw);path.symlink_to(alias)
        with self.assertRaises(ValueError):self.prepare()
        self.assertFalse(self.receipt.exists())

    def test_direct_invocation_has_no_administrator_entry_or_mutation(self):
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/install-transaction.py')],
                              capture_output=True,timeout=10)
        self.assertNotEqual(result.returncode,0)
        self.assertIn(b'no administrator entry',result.stderr)
        self.assertFalse(self.receipt.exists());self.assertFalse(self.barrier.exists())

    def test_unfinished_metadata_is_retained_and_blocks_recovery(self):
        self.prepare()
        for target in (self.receipt,self.barrier):
            temporary=target.with_name('.'+target.name+'.tmp');temporary.write_bytes(b'partial private record')
            with self.assertRaises(ValueError):self.fence()
            self.assertEqual(temporary.read_bytes(),b'partial private record')
            temporary.unlink()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_impossible_phase_pairs_and_reopened_completed_barrier_are_rejected(self):
        value=self.prepare();self.B.record(self.barrier,T.marker(self.B,value,'fenced'))
        with self.assertRaises(ValueError):self.fence()
        value['phase']='fencing';self.B.record(self.receipt,value)
        with self.assertRaises(ValueError):self.fence()
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))

    def test_readonly_fenced_prerequisite_does_not_complete_partial_state(self):
        self.prepare();before=self.receipt.read_bytes(),self.barrier.read_bytes()
        with self.assertRaises(ValueError):T.fenced(self.B,self.lease_fd)
        self.assertEqual((self.receipt.read_bytes(),self.barrier.read_bytes()),before)
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode)==0o755 for p in self.entries))
        completed=self.fence()
        with patch.object(self.B,'record',side_effect=AssertionError('readonly observation wrote state')):
            self.assertEqual(T.fenced(self.B,self.lease_fd),completed)

    def test_readonly_fenced_prerequisite_rejects_reopened_entry_and_config_drift(self):
        self.prepare();self.fence();self.entries[0].chmod(0o755)
        with self.assertRaises(ValueError):T.fenced(self.B,self.lease_fd)
        self.entries[0].chmod(0);self.config.write_bytes(b'changed')
        with self.assertRaises(ValueError):T.fenced(self.B,self.lease_fd)


if __name__=='__main__':unittest.main()
