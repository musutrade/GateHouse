"""Real private state, lease and symlinks; native mount/system-manager facts mocked."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

ROOT=Path(__file__).resolve().parents[3]


def load(name,path):
    spec=importlib.util.spec_from_file_location(name,ROOT/path);module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module);return module


P=load('mount_persistence','tools/trusted_gate/deploy/persist-linux-mount.py')
B=load('mount_bootstrap','tools/trusted_gate/deploy/import-protected-inputs.py')
F=load('mount_original_lease','tools/trusted_gate/deploy/install-transaction.py')
I=load('mount_private_records','tools/trusted_gate/deploy/install-reviewed-files.py')


class MountPersistenceTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
        self.units=self.root/'units';self.units.mkdir(mode=0o700);self.vendor=self.root/'vendor';self.vendor.mkdir(mode=0o700)
        self.file=self.units/P.UNIT;self.link=self.units/'multi-user.target.wants'/P.UNIT
        self.receipt=self.root/'persistence.json';self.marker=self.root/'storage.json';self.lease=self.root/'controller.lock'
        self.unit_raw=(ROOT/'tools/trusted_gate/deploy/srv-gatehouse-linux.mount').read_bytes()
        B.write(self.file,self.unit_raw,0o644);B.write(self.lease,b'',0o600)
        self.fd=os.open(self.lease,os.O_RDWR);self.addCleanup(os.close,self.fd)
        self.volume={'schema':'fixture-ready-volume','status':'ready','uuid':'known'};B.write(self.marker,B.canonical(self.volume),0o600)
        self.patch(P,'FILE',self.file);self.patch(P,'LINK',self.link);self.patch(P,'RECEIPT',self.receipt)
        self.patch(F,'LEASE',self.lease);self.patch(P.os,'geteuid',lambda:0)
        self.patch(P,'root_owned',lambda info:info.st_uid==os.getuid())
        self.B=SimpleNamespace(**{name:getattr(B,name) for name in ('need','canonical','unique','record','write','ENV')})
        self.B.owned=self.owned;self.B.read=self.read
        self.storage=SimpleNamespace(MARKER=self.marker,admit=Mock(side_effect=lambda *args:copy.deepcopy(self.volume)))
        self.B.load=Mock(return_value=self.storage)
        self.ctx=SimpleNamespace(B=self.B,files=I,fd=self.fd,fence=F,helpers=self.root/'helpers',manifest=self.root/'manifest',
            manifest_pin='a'*64,inputs=SimpleNamespace(UNIT_ROOTS=(self.units,self.vendor)),
            value={'paths':{'volume_recipe':str(self.root/'tools.json')},'pins':{'volume_recipe':'b'*64}},
            material={'files':{str(self.file):{'before':None,'mode':0o644,'action':'replace','after':self.unit_raw}}})
        self.value={name:'c'*64 for name in ('helper_manifest_sha256','recipe_sha256','material_sha256',
             'upgrade_receipt_sha256','file_receipt_sha256','entry_fence_sha256')}
        self.value.update(head='a'*40,transaction='00000000-0000-4000-8000-000000000001',lease=[os.fstat(self.fd).st_dev,os.fstat(self.fd).st_ino],phase='prepared')
        self.guard=Mock(side_effect=lambda:F.lease(self.B,self.fd));self.loaded=False;self.calls=[]
        self.patch(P.subprocess,'run',self.command)

    def patch(self,obj,name,value):
        p=patch.object(obj,name,value);p.start();self.addCleanup(p.stop)

    def owned(self,path,directory=False,private=False):
        path=Path(path);B.need(path==self.root or self.root in path.parents,'fixture containment')
        info=path.lstat();B.need(not stat.S_ISLNK(info.st_mode) and not info.st_mode&0o022,'fixture link/mode')
        B.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink==1,'fixture type')
        B.need(not private or not info.st_mode&0o077,'fixture private');return path

    def read(self,path,limit):
        raw=self.owned(path).read_bytes();B.need(len(raw)<=limit,'fixture read limit');return raw

    def state(self):return json.loads(self.receipt.read_bytes())

    def create_link(self,target=None):
        self.link.parent.mkdir(mode=0o700,exist_ok=True);self.link.symlink_to(target or str(self.file))

    def properties(self):
        return {'Id':P.UNIT,'LoadState':'loaded' if self.loaded else 'not-found','ActiveState':'active','SubState':'mounted',
            'UnitFileState':'enabled' if self.link.is_symlink() else 'disabled','FragmentPath':str(self.file),
            'DropInPaths':'','NeedDaemonReload':'no'}

    def command(self,argv,**kw):
        self.assertEqual(argv[0],'/usr/bin/systemctl');self.assertEqual(kw['env'],B.ENV);self.assertTrue(kw['check'])
        self.assertEqual(kw['timeout'],20);self.calls.append(argv[1:]);args=argv[1:]
        if args==['daemon-reload']:
            self.assertEqual(self.state()['phase'],'reload-intent');self.assertFalse(self.link.exists());self.loaded=True
        elif args==['--no-reload','enable',P.UNIT]:
            self.assertEqual(self.state()['phase'],'enable-intent');self.create_link()
        else:
            self.assertEqual(args[:2],['show',P.UNIT]);row=self.properties()
            return SimpleNamespace(stdout=('\n'.join(key+'='+value for key,value in row.items())+'\n').encode(),stderr=b'')
        return SimpleNamespace(stdout=b'',stderr=b'')

    def execute(self):return P.persist(self.ctx,self.value,self.guard)

    def test_durable_intents_original_lease_and_no_start_or_mount_command(self):
        result=self.execute();self.assertEqual(result['phase'],'enabled');self.assertFalse(result['administrator_ready'])
        self.assertEqual(os.readlink(self.link),str(self.file));self.assertGreater(self.guard.call_count,3)
        self.assertEqual([x for x in self.calls if x[0]!='show'],[['daemon-reload'],['--no-reload','enable',P.UNIT]])
        self.assertEqual(result['storage_sha256'],P.digest(self.marker.read_bytes()))

    def test_completed_retry_is_readonly_and_keeps_same_metadata_and_link(self):
        result=self.execute();raw=self.receipt.read_bytes();stamp=self.link.lstat();self.calls=[]
        with patch.object(self.B,'record',side_effect=AssertionError('metadata mutation')):
            self.assertEqual(self.execute(),result)
        self.assertTrue(all(call[0]=='show' for call in self.calls));self.assertEqual(self.receipt.read_bytes(),raw)
        self.assertEqual(self.link.lstat(),stamp)

    def test_existing_enablement_without_intent_is_not_adopted(self):
        self.create_link()
        with self.assertRaisesRegex(ValueError,'without its durable intent'):self.execute()
        self.assertFalse(self.receipt.exists());self.assertFalse(self.calls)

    def test_lost_enable_response_adopts_only_known_exact_link_without_reenable(self):
        real=P.command
        def lost(ctx,*args):
            result=real(ctx,*args)
            if args==('--no-reload','enable',P.UNIT):raise OSError('enable response lost')
            return result
        with patch.object(P,'command',side_effect=lost),self.assertRaisesRegex(OSError,'response lost'):self.execute()
        self.assertEqual(self.state()['phase'],'enable-intent');self.calls=[]
        self.assertEqual(self.execute()['phase'],'enabled');self.assertTrue(all(call[0]=='show' for call in self.calls))

    def test_lost_reload_response_repeats_only_idempotent_reload(self):
        real=P.command;count=0
        def lost(ctx,*args):
            nonlocal count
            result=real(ctx,*args)
            if args==('daemon-reload',) and count==0:count+=1;raise OSError('reload response lost')
            return result
        with patch.object(P,'command',side_effect=lost):
            with self.assertRaisesRegex(OSError,'response lost'):self.execute()
            self.assertEqual(self.execute()['phase'],'enabled')

    def test_failed_guard_prevents_persistence_metadata_or_systemctl_effect(self):
        with patch.object(self,'guard',side_effect=ValueError('native admission failed')):
            with self.assertRaisesRegex(ValueError,'native admission'):self.execute()
        self.assertFalse(self.receipt.exists());self.assertFalse(self.calls)

    def test_changed_unit_mode_bytes_or_hardlink_blocks_before_reload(self):
        self.file.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'changed'):self.execute()
        self.file.chmod(0o644);self.file.write_bytes(b'foreign')
        with self.assertRaisesRegex(ValueError,'changed'):self.execute()
        self.file.write_bytes(self.unit_raw);os.link(self.file,self.root/'alias')
        with self.assertRaisesRegex(ValueError,'type'):self.execute()
        self.assertFalse(self.calls)

    def test_all_dropin_scopes_and_unit_overrides_are_rejected(self):
        for name in ('mount.d','srv-.mount.d','srv-gatehouse-.mount.d',P.UNIT+'.d'):
            path=self.vendor/name;path.mkdir()
            with self.subTest(name=name),self.assertRaisesRegex(ValueError,'drop-in'):self.execute()
            path.rmdir()
        (self.vendor/P.UNIT).symlink_to(self.file)
        with self.assertRaisesRegex(ValueError,'override'):self.execute()

    def test_unknown_wants_slot_and_wrong_link_target_are_rejected(self):
        other=self.vendor/'local-fs.target.wants';other.mkdir(mode=0o700);slot=other/P.UNIT;slot.symlink_to(self.file)
        with self.assertRaisesRegex(ValueError,'unknown mount enablement'):self.execute()
        slot.unlink();self.create_link('/foreign')
        with self.assertRaisesRegex(ValueError,'link differs'):self.execute()

    def test_manager_foreign_fragment_dropin_or_state_blocks_before_enable(self):
        real=self.properties
        for key,value in (('FragmentPath','/foreign'),('DropInPaths','/foreign'),('ActiveState','inactive'),('NeedDaemonReload','yes')):
            with self.subTest(key=key),patch.object(self,'properties',side_effect=lambda key=key,value=value:{**real(),key:value}):
                with self.assertRaisesRegex(ValueError,'did not adopt'):self.execute()
            self.assertFalse(self.link.exists());self.receipt.unlink();self.calls=[]

    def test_unknown_generation_or_partial_record_blocks_and_retains(self):
        self.execute();value=self.state();value['head']='b'*40;B.record(self.receipt,value)
        with self.assertRaisesRegex(ValueError,'another generation'):self.execute()
        value['head']='a'*40;B.record(self.receipt,value);partial=self.receipt.with_name('.'+self.receipt.name+'.tmp');partial.write_bytes(b'partial')
        with self.assertRaisesRegex(ValueError,'partial'):self.execute()
        self.assertEqual(partial.read_bytes(),b'partial')

    def test_changed_storage_registration_and_link_ownership_block_completed_replay(self):
        self.execute();self.volume['uuid']='different';self.marker.chmod(0o600);self.marker.write_bytes(B.canonical(self.volume))
        with self.assertRaisesRegex(ValueError,'another generation'):self.execute()
        self.volume['uuid']='known';self.marker.write_bytes(B.canonical(self.volume))
        with patch.object(P,'root_owned',lambda info:False),self.assertRaisesRegex(ValueError,'link differs'):self.execute()

    def test_no_new_enablement_after_activation_advanced_but_completed_replay_allowed(self):
        self.value['phase']='marker-active'
        with self.assertRaisesRegex(ValueError,'after activation'):self.execute()
        self.value['phase']='prepared';self.execute();self.value['phase']='entries-enabled'
        self.assertEqual(self.execute()['phase'],'enabled')

    def test_nonroot_or_changed_lease_blocks_before_manager_effects(self):
        with patch.object(P.os,'geteuid',lambda:999),self.assertRaisesRegex(ValueError,'administrator'):self.execute()
        self.lease.unlink();B.write(self.lease,b'',0o600)
        with self.assertRaisesRegex(ValueError,'lease differs'):self.execute()
        self.assertFalse(self.calls)

    def test_systemctl_failure_never_marks_enablement_complete_or_cleans_state(self):
        real=P.command
        def failed(ctx,*args):
            if args==('--no-reload','enable',P.UNIT):raise subprocess.CalledProcessError(1,['systemctl'])
            return real(ctx,*args)
        with patch.object(P,'command',side_effect=failed),self.assertRaises(subprocess.CalledProcessError):self.execute()
        self.assertEqual(self.state()['phase'],'enable-intent');self.assertFalse(self.link.exists())


class NativeMountUnitTests(unittest.TestCase):
    def test_native_offline_enable_creates_only_the_expected_absolute_wants_link(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);units=root/'etc/systemd/system';units.mkdir(parents=True)
            (units/P.UNIT).write_bytes((ROOT/'tools/trusted_gate/deploy/srv-gatehouse-linux.mount').read_bytes())
            result=subprocess.run(['/usr/bin/systemctl','--root',str(root),'--no-reload','enable',P.UNIT],
                                  env=B.ENV,capture_output=True,timeout=20)
            self.assertEqual(result.returncode,0,result.stderr.decode())
            links=[p for p in root.rglob('*') if p.is_symlink()]
            self.assertEqual(links,[units/'multi-user.target.wants'/P.UNIT])
            self.assertEqual(os.readlink(links[0]),'/etc/systemd/system/'+P.UNIT)


if __name__=='__main__':unittest.main()
