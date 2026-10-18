"""Only exact source input releases may cross the bootstrap inspection boundary."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[3]


def load(name,file):
    spec=importlib.util.spec_from_file_location(name,ROOT/'tools/trusted_gate/deploy'/file)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


BOOT=load('release_entry','import-protected-inputs.py')
R=load('release_contract','bootstrap-release.py')
P=load('release_packager','package-bootstrap-release.py')


class BootstrapReleaseTests(unittest.TestCase):
    def setUp(self):
        before=os.umask(0o077);self.addCleanup(os.umask,before)
        self.temporary=tempfile.TemporaryDirectory();self.addCleanup(self.temporary.cleanup)
        self.root=Path(self.temporary.name);self.head='a'*40
        self.helpers=self.root/'helpers';self.helpers.mkdir()
        self.payload={}
        for name in BOOT.FILES:
            raw=(ROOT/name).read_bytes()
            path=self.helpers/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
            self.payload['helpers/'+name]=raw
        self.code={'schema':'gatehouse-bootstrap-code/v1','revision':self.head,
                   'files':{name:hashlib.sha256(self.payload['helpers/'+name]).hexdigest() for name in BOOT.FILES}}
        self.manifest_raw=BOOT.canonical(self.code);self.manifest=self.root/'manifest.json'
        self.manifest.write_bytes(self.manifest_raw);self.manifest_pin=hashlib.sha256(self.manifest_raw).hexdigest()
        self.payload.update({'metadata.json':BOOT.canonical(R.metadata(self.head)),
                             'helper-manifest.json':self.manifest_raw,'source.bundle':b'bounded bundle fixture'})
        # Schema/parser tests use a small explicit synthetic old inventory. Real
        # fixed659 bytes are checked by the actual committed-package observation.
        for name in R.old_sources():self.payload['before/'+name]=('old:'+name).encode()
        for name in R.source_map():
            old=self.payload.get('before/'+name)
            self.payload['files/'+name]=old if R.source_map()[name][2]=='retain' else (
                self.payload['helpers/'+name] if name in BOOT.FILES else ('new:'+name).encode())
        self.payload['files/'+R.LINUX.FLOW]=(ROOT/R.LINUX.FLOW).read_bytes()
        self.payload['files/'+R.LINUX.INVENTORY]=(ROOT/R.LINUX.INVENTORY).read_bytes()
        old={name:self.payload['before/'+name] for name in R.old_sources()}
        pin=hashlib.sha256(BOOT.canonical(R.rows(old))).hexdigest()
        guard=patch.object(R,'OLD_INPUTS',pin);guard.start();self.addCleanup(guard.stop)
        self.seal()
        self.archive=self.root/'bootstrap.tar'
        self.B=SimpleNamespace(**{name:getattr(BOOT,name) for name in ('need','canonical','unique','path_operand')})
        self.B.owned=self.owned
        self.B.read=lambda path,limit:self.read(path,limit)
        self.B.authenticate=lambda *args:self.code
        self.B.frozen_lock=lambda *args:b'fixture lock'
        def git(source,*args):
            self.assertEqual(args[0],'show');head,name=args[1].split(':',1)
            self.assertEqual(head,self.head)
            return self.payload.get('files/'+name,self.payload.get('helpers/'+name))
        self.B.git=git

    def owned(self,path,directory=False,private=False):
        path=Path(path);info=path.lstat()
        for parent in (path,*path.parents):BOOT.need(not parent.is_symlink(),'linked fixture')
        BOOT.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode),'fixture type')
        BOOT.need(directory or info.st_nlink==1,'fixture hardlink')
        BOOT.need(not info.st_mode&0o022 and (not private or not info.st_mode&0o077),'fixture mode')
        return path

    def read(self,path,limit):
        self.owned(path);raw=Path(path).read_bytes();BOOT.need(len(raw)<=limit,'fixture read bound');return raw

    def seal(self):
        self.payload['payload-manifest.json']=BOOT.canonical({'schema':'gatehouse-bootstrap-payload/v1',
            'files':R.rows({name:raw for name,raw in self.payload.items() if name!='payload-manifest.json'}),
            'installed_plan':R.plan(self.payload),'source_binding_required':True})

    def encoded(self,*,extra=None,omit=None,kind=None,format=tarfile.USTAR_FORMAT):
        output=io.BytesIO()
        with tarfile.open(fileobj=output,mode='w:',format=format) as target:
            items=list(sorted(self.payload.items()))
            if extra:items.append(extra)
            for name,raw in items:
                if name==omit:continue
                member=tarfile.TarInfo(name);member.mode=0o400;member.size=len(raw)
                if kind and name=='source.bundle':member.type=kind;member.linkname='/etc/passwd'
                target.addfile(member,io.BytesIO(raw))
        return output.getvalue()

    def decode(self,raw=None,expected=None):
        raw=self.encoded() if raw is None else raw
        return R.decode(self.B,io.BytesIO(raw),len(raw),expected or hashlib.sha256(raw).hexdigest(),self.code,self.manifest_raw)

    def test_exact_source_release_is_inspected_without_extraction_or_core(self):
        payload,result=self.decode()
        self.assertEqual(result['members'],4+len(R.old_sources())+len(R.source_map())+len(BOOT.FILES))
        self.assertFalse(result['administrator_ready']);self.assertTrue(result['source_binding_required'])
        self.assertFalse(any(name.endswith('/harness-gate') for name in payload))
        self.assertFalse((self.root/'files').exists())

    def test_wrong_digest_or_truncated_archive_rejected_before_decision(self):
        with self.assertRaises(ValueError):self.decode(expected='0'*64)
        with self.assertRaises(ValueError):self.decode(self.encoded()[:-1])

    def test_duplicate_missing_unknown_and_binary_members_are_rejected(self):
        for name in ['source.bundle','../escape','bin/harness-gate','helpers/extra.py']:
            with self.subTest(name=name):
                with self.assertRaises(ValueError):self.decode(self.encoded(extra=(name,b'unsafe')))
        with self.assertRaises(ValueError):self.decode(self.encoded(omit='source.bundle'))

    def test_links_and_extension_headers_are_rejected(self):
        for kind in (tarfile.SYMTYPE,tarfile.LNKTYPE,tarfile.DIRTYPE,tarfile.XHDTYPE,tarfile.GNUTYPE_SPARSE):
            with self.subTest(kind=kind):
                with self.assertRaises((ValueError,tarfile.TarError)):self.decode(self.encoded(kind=kind))

    def test_noncanonical_tar_metadata_padding_or_trailer_is_rejected(self):
        raw=self.encoded()
        changed=bytearray(raw);changed[-1]=1
        with self.assertRaises(ValueError):self.decode(bytes(changed))
        first=tarfile.TarInfo.frombuf(raw[:512],'utf-8','strict')
        offset=512+first.size
        if offset%512:
            changed=bytearray(raw);changed[offset]=1
            with self.assertRaises(ValueError):self.decode(bytes(changed))
        changed=bytearray(raw);changed[257]=ord('X')
        with self.assertRaises(ValueError):self.decode(bytes(changed))

    def test_scope_flags_order_and_core_provenance_cannot_be_promoted(self):
        old=self.payload['metadata.json']
        changes=[('administrator_ready',True),('administrator_ready',0),('core_binary_included',True),
                 ('coverage_acceptance','complete'),('core_provenance','operator-record'),
                 ('required_migration_order',list(reversed(R.ORDER)))]
        for key,value in changes:
            with self.subTest(key=key,value=value):
                data=json.loads(old);data[key]=value;self.payload['metadata.json']=BOOT.canonical(data);self.seal()
                with self.assertRaises(ValueError):self.decode()
        self.payload['metadata.json']=old

    def test_old_inventory_cannot_be_rehashed_into_a_new_accepted_baseline(self):
        name=next(iter(R.old_sources()));self.payload['before/'+name]+=b'changed';self.seal()
        with self.assertRaises(ValueError):self.decode()

    def test_retained_mount_and_delegation_cannot_change(self):
        name=R.DEPLOY+'sudoers.example';self.payload['files/'+name]+=b'changed';self.seal()
        with self.assertRaises(ValueError):self.decode()

    def test_helper_blob_or_payload_digest_mismatch_blocks(self):
        name=next(iter(self.code['files']));self.payload['helpers/'+name]+=b'changed'
        with self.assertRaises(ValueError):self.decode()

    def test_installed_plan_destination_mode_or_source_binding_cannot_change(self):
        old=self.payload['payload-manifest.json']
        for key,value in [('destination','/etc/shadow'),('mode',0o777)]:
            manifest=json.loads(old);name=next(iter(manifest['installed_plan']));manifest['installed_plan'][name][key]=value
            self.payload['payload-manifest.json']=BOOT.canonical(manifest)
            with self.assertRaises(ValueError):self.decode()
        manifest=json.loads(old);manifest['source_binding_required']=False
        self.payload['payload-manifest.json']=BOOT.canonical(manifest)
        with self.assertRaises(ValueError):self.decode()

    def test_source_binding_requires_external_manifest_and_exact_git_bytes(self):
        self.decode()
        value=R.bind_source(self.B,self.payload,self.root/'source',self.head,self.manifest_pin)
        self.assertTrue(value['source_bound']);self.assertFalse(value['administrator_ready'])
        with self.assertRaises(ValueError):R.bind_source(self.B,self.payload,self.root/'source',self.head,'0'*64)
        self.B.git=lambda *args:b'different protected Git bytes'
        with self.assertRaises(ValueError):R.bind_source(self.B,self.payload,self.root/'source',self.head,self.manifest_pin)

    def test_inspection_reauthenticates_and_reads_private_regular_archive_only(self):
        raw=self.encoded();self.archive.write_bytes(raw);pin=hashlib.sha256(raw).hexdigest()
        calls=[];self.B.authenticate=lambda *args:(calls.append('auth') or self.code)
        with patch.object(R.os,'geteuid',return_value=0):
            value=R.inspect(self.B,self.helpers,self.manifest,self.manifest_pin,self.archive,pin)
        self.assertFalse(value['administrator_ready']);self.assertEqual(calls,['auth','auth'])
        alias=self.root/'alias';os.link(self.archive,alias)
        with patch.object(R.os,'geteuid',return_value=0),self.assertRaises(ValueError):
            R.inspect(self.B,self.helpers,self.manifest,self.manifest_pin,self.archive,pin)

    def test_archive_and_member_limits_are_enforced(self):
        with patch.object(R,'MAX_ARCHIVE',512),self.assertRaises(ValueError):self.decode()
        with patch.object(R,'MAX_MEMBER',1),self.assertRaises(ValueError):self.decode()

    def test_packager_rejects_root_and_dirty_source_before_output(self):
        output=self.root/'package'
        with patch.object(P.os,'geteuid',return_value=0),self.assertRaises(ValueError):P.package(ROOT,output)
        with patch.object(P.subprocess,'check_output',return_value=b'M dirty\n'),self.assertRaises(ValueError):
            P.package(ROOT,output)
        self.assertFalse(output.exists())

    def test_real_nonroot_inspection_cli_rejects_before_reading_package(self):
        if os.geteuid()==0:self.skipTest('requires ordinary user')
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),
            'inspect-release','--helper-root',str(self.helpers),'--manifest',str(self.manifest),
            '--manifest-sha256',self.manifest_pin,'--archive',str(self.archive),'--archive-sha256','0'*64],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'bootstrap release blocked',result.stderr)


if __name__=='__main__':unittest.main()
