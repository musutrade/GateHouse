"""Separate storage admission never reformats unknown images or changes old pilot."""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT/'tools/trusted_gate/deploy'/file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BOOT = load('volume_entry', 'import-protected-inputs.py')
VOL = load('volume_candidate', 'prepare-linux-volume.py')


class VolumeTests(unittest.TestCase):
    def setUp(self):
        before = os.umask(0o077)
        self.addCleanup(os.umask, before)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.image, self.marker = self.root/'storage/linux.ext4', self.root/'storage/marker.json'
        self.target, self.control = self.root/'mount/linux', self.root/'old/control'
        for path in (self.image.parent, self.target.parent, self.control): path.mkdir(parents=True)
        (self.control/'controller.lock').touch(mode=0o600)
        (self.control/'keep-old-pilot').write_bytes(b'accepted659')
        self.size = 64 * 1024
        self.commands = {key: '/reviewed/'+key for key in VOL.COMMANDS}
        self.recipe = self.root/'tools.json'
        self.recipe.write_bytes(BOOT.canonical({'schema':'gatehouse-volume-tools/v1', 'commands':self.commands,
            'files':{path:hashlib.sha256(path.encode()).hexdigest() for path in self.commands.values()}}))
        self.recipe_pin = hashlib.sha256(self.recipe.read_bytes()).hexdigest()
        self.real_owned, self.real_statvfs = BOOT.owned, os.statvfs
        self.B = SimpleNamespace(**{name:getattr(BOOT,name) for name in (
            'need','path_operand','read','write','record','canonical','unique','ENV')})
        self.B.owned = self.owned
        self.B.authenticate = lambda *args: {'revision':'a'*40}
        self.builder = SimpleNamespace(host_file=lambda path: str(path).encode(), pilot_slot=lambda B:None,
                                       quiet_units=lambda B:None, idle=lambda B:None)
        self.B.load = lambda *args: self.builder
        self.signature, self.rows, self.calls = None, [], []
        self.loop_override = None
        self.alias_rows = None
        self.fail_name, self.low_capacity = None, False

    def owned(self, path, directory=False, private=False):
        path = Path(path)
        if path != self.root and self.root not in path.parents:
            return self.real_owned(path,directory=directory,private=private)
        for parent in (path,*path.parents): BOOT.need(not parent.is_symlink(),'linked fixture')
        info=path.lstat()
        BOOT.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode),'fixture type')
        BOOT.need(directory or info.st_nlink==1,'fixture hardlink')
        BOOT.need(not info.st_mode & 0o022 and (not private or not info.st_mode & 0o077),'fixture mode')
        return path

    def disk(self,path):
        if Path(path)==self.target:
            return SimpleNamespace(f_blocks=self.size//4096,f_frsize=4096)
        if self.low_capacity: return SimpleNamespace(f_bavail=0,f_frsize=4096)
        return self.real_statvfs(path)

    def execute(self,argv,**kwargs):
        self.assertEqual(kwargs['env'],BOOT.ENV)
        name=Path(argv[0]).name
        self.calls.append((name,argv[1:]))
        data=b''; code=0
        if name=='findmnt':
            if '--source' in argv:
                rows=self.alias_rows if self.alias_rows is not None else [{'target':str(self.target),'fsroot':'/'}]
                data=json.dumps({'filesystems':rows}).encode()
            elif self.rows: data=json.dumps({'filesystems':self.rows}).encode()
            else: code=1
        elif name=='blkid':
            if self.signature: data=('TYPE=ext4\nUUID='+self.signature+'\n').encode()
            else: code=2
        elif name=='losetup':
            rows=[]
            if self.rows:
                info=self.image.stat()
                rows=[{'name':'/dev/loop123','back-file':str(self.image),'back-ino':info.st_ino,
                       'back-maj:min':str(os.major(info.st_dev))+':'+str(os.minor(info.st_dev)),
                       'offset':0,'sizelimit':0,'ro':False}]
                if self.loop_override: rows[0].update(self.loop_override)
            data=json.dumps({'loopdevices':rows}).encode()
        elif name=='fallocate':
            with self.image.open('r+b') as stream: os.posix_fallocate(stream.fileno(),0,self.size)
        elif name=='mkfs': self.signature=argv[argv.index('-U')+1]
        elif name=='mount':
            self.rows=[{'target':str(self.target),'source':'/dev/loop123','fstype':'ext4','uuid':self.signature,
                        'options':'rw,nodev,nosuid,noatime','fsroot':'/'}]
        elif name!='fsck': self.fail('unexpected tool')
        if self.fail_name==name: raise subprocess.TimeoutExpired(argv,1)
        return SimpleNamespace(returncode=code,stdout=data,stderr=b'')

    def prepare(self,admission=False,**kwargs):
        # Only fixture Root uid/ownership, kernel mount/loop probes and management
        # programs are replaced. Small file allocation/flock/JSON checks are real.
        with patch.multiple(VOL,IMAGE=self.image,MARKER=self.marker,VOLUME=self.target,CONTROL=self.control,SIZE=self.size), \
             patch.object(BOOT,'owned',side_effect=self.owned), patch.object(VOL.os,'geteuid',return_value=0), \
             patch.object(VOL.os,'statvfs',side_effect=self.disk), \
             patch.object(VOL,'block_device',side_effect=lambda path:self.target.stat().st_dev), \
             patch.object(VOL.subprocess,'run',side_effect=self.execute):
            if admission:
                return VOL.admit(self.B,self.root/'helpers',self.root/'manifest','a'*64,self.recipe,self.recipe_pin)
            return VOL.prepare(self.B,self.root/'helpers',self.root/'manifest','a'*64,self.recipe,self.recipe_pin,**kwargs)

    def test_readonly_admission_requires_ready_matching_registration(self):
        self.prepare(); self.calls=[]
        self.assertEqual(self.prepare(admission=True)['status'],'ready')
        self.assertFalse({'fallocate','mkfs','fsck','mount'} & {name for name,args in self.calls})
        value=json.loads(self.marker.read_bytes()); value['status']='blocked'
        self.marker.write_bytes(BOOT.canonical(value))
        with self.assertRaises(ValueError):self.prepare(admission=True)

    def test_fresh_registered_reservation_format_and_mount_leave_old_pilot_untouched(self):
        value=self.prepare()
        self.assertEqual((value['phase'],value['status']),('mounted','ready'))
        self.assertFalse(value['production_gate_enabled'])
        self.assertEqual((self.control/'keep-old-pilot').read_bytes(),b'accepted659')
        args=next(args for name,args in self.calls if name=='mkfs')
        self.assertIn('nodiscard,lazy_itable_init=0,lazy_journal_init=0',args)
        self.assertEqual(args[args.index('-t')+1],'ext4')
        self.assertIn('--internal-only',next(args for name,args in self.calls if name=='mount'))
        self.assertIn('--submounts',next(args for name,args in self.calls if name=='findmnt'))
        self.assertGreaterEqual(self.image.stat().st_blocks*512,self.size)

    def test_valid_mounted_retry_checks_identity_without_format_allocate_or_mount(self):
        first=self.prepare(); self.calls=[]
        self.assertEqual(self.prepare()['uuid'],first['uuid'])
        self.assertFalse({'fallocate','mkfs','fsck','mount'} & {name for name,args in self.calls})

    def test_registered_unmounted_image_is_checked_then_remounted_without_format(self):
        self.prepare(); self.rows=[]; self.calls=[]
        self.prepare()
        names=[name for name,args in self.calls]
        self.assertIn('fsck',names); self.assertIn('mount',names)
        self.assertNotIn('mkfs',names); self.assertNotIn('fallocate',names)

    def test_unknown_image_blocks_before_marker_or_format(self):
        self.image.write_bytes(b'unknown')
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse(self.marker.exists())
        self.assertEqual(self.image.read_bytes(),b'unknown')

    def test_unknown_mount_blocks_without_creating_image(self):
        self.rows=[{'unknown':'mount'}]
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse(self.marker.exists()); self.assertFalse(self.image.exists())

    def test_unsealed_image_after_intent_is_not_reused(self):
        self.fail_name='fallocate'
        with self.assertRaises(subprocess.TimeoutExpired): self.prepare()
        value=json.loads(self.marker.read_bytes()); value['phase']='intent'; value.pop('image_identity')
        self.marker.write_bytes(BOOT.canonical(value)); self.calls=[]; self.fail_name=None
        with self.assertRaises(ValueError): self.prepare()
        self.assertNotIn('mkfs',[name for name,args in self.calls])

    def test_interrupted_known_allocation_can_resume_without_losing_inode(self):
        self.fail_name='fallocate'
        with self.assertRaises(subprocess.TimeoutExpired): self.prepare()
        old=self.image.stat().st_ino
        self.assertEqual(json.loads(self.marker.read_bytes())['phase'],'created')
        self.fail_name=None
        self.assertEqual(self.prepare()['status'],'ready')
        self.assertEqual(self.image.stat().st_ino,old)

    def test_interrupted_format_checks_recognized_filesystem_without_reformatting(self):
        self.fail_name='mkfs'
        with self.assertRaises(subprocess.TimeoutExpired): self.prepare()
        self.assertEqual(json.loads(self.marker.read_bytes())['phase'],'allocated')
        self.fail_name='fsck'; self.calls=[]
        with self.assertRaises(subprocess.TimeoutExpired): self.prepare()
        self.assertNotIn('mkfs',[name for name,args in self.calls])
        self.fail_name=None; self.calls=[]
        self.assertEqual(self.prepare()['status'],'ready')
        self.assertNotIn('mkfs',[name for name,args in self.calls])

    def test_mounted_uuid_or_options_drift_never_reformats(self):
        self.prepare()
        for field,value in [('uuid','b'*36),('options','rw,suid,nodev,noatime'),('fsroot','/subdir')]:
            with self.subTest(field=field):
                before=self.rows[0][field]; self.rows[0][field]=value; self.calls=[]
                with self.assertRaises(ValueError): self.prepare()
                self.assertNotIn('mkfs',[name for name,args in self.calls])
                self.rows[0][field]=before

    def test_loop_inode_device_offset_or_unknown_backing_blocks_admission(self):
        self.prepare()
        for key,value in [('back-ino',None),('back-ino',123),('back-maj:min','0:0'),('offset',4096),
                          ('sizelimit',1),('ro',True),('back-file','/wrong/image')]:
            with self.subTest(key=key,value=value):
                self.loop_override={key:value}
                with self.assertRaises(ValueError): self.prepare()

    def test_submount_or_other_alias_or_lost_headroom_blocks_mounted_retry(self):
        self.prepare()
        self.rows[0]['children']=[{'target':str(self.target/'unexpected')}]
        with self.assertRaises(ValueError): self.prepare()
        self.rows[0].pop('children')
        self.alias_rows=[{'target':str(self.target),'fsroot':'/'},{'target':'/another','fsroot':'/'}]
        with self.assertRaises(ValueError): self.prepare()
        self.alias_rows=None; self.low_capacity=True
        with self.assertRaises(ValueError): self.prepare()

    def test_sparse_or_replaced_registered_image_blocks_admission(self):
        self.prepare()
        with self.image.open('r+b') as stream: stream.truncate(0); stream.truncate(self.size)
        with self.assertRaises(ValueError): self.prepare()
        old=self.image.with_name('old.ext4'); self.image.rename(old)
        self.image.touch()
        with self.image.open('r+b') as stream: os.posix_fallocate(stream.fileno(),0,self.size)
        with self.assertRaises(ValueError): self.prepare()

    def test_recipe_or_tool_drift_rejected_before_registration(self):
        self.recipe_pin='0'*64
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse(self.marker.exists())
        self.recipe_pin=hashlib.sha256(self.recipe.read_bytes()).hexdigest()
        self.builder.host_file=lambda path:b'changed'
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse(self.marker.exists())

    def test_insufficient_host_headroom_leaves_no_image_or_marker(self):
        self.low_capacity=True
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse(self.image.exists()); self.assertFalse(self.marker.exists())

    def test_changed_helper_registration_is_not_adopted(self):
        self.prepare()
        value=json.loads(self.marker.read_bytes()); value['helper_manifest_sha256']='b'*64
        self.marker.write_bytes(BOOT.canonical(value)); self.calls=[]
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse({'fallocate','mkfs','mount'} & {name for name,args in self.calls})

    def test_inherited_lease_remains_held_and_competing_descriptor_is_refused(self):
        with (self.control/'controller.lock').open('r+b') as stream:
            fcntl.flock(stream.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError): self.prepare()
            self.assertFalse(self.marker.exists())
            self.prepare(lock_fd=stream.fileno())
            with (self.control/'controller.lock').open('r+b') as rival:
                with self.assertRaises(BlockingIOError): fcntl.flock(rival.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)

    def test_active_or_unknown_old_slot_blocks_before_image_creation(self):
        def blocked(B): raise ValueError('unknown old pilot')
        self.builder.pilot_slot=blocked
        with self.assertRaises(ValueError): self.prepare()
        self.assertFalse(self.marker.exists()); self.assertFalse(self.image.exists())

    def test_nonroot_cli_rejected_before_touching_storage(self):
        if os.geteuid()==0: self.skipTest('requires ordinary user')
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),
            'prepare-volume','--helper-root',str(self.root/'missing'),'--manifest',str(self.root/'missing.json'),
            '--manifest-sha256','a'*64,'--tool-recipe',str(self.recipe),'--tool-recipe-sha256',self.recipe_pin],
            capture_output=True)
        self.assertNotEqual(result.returncode,0)
        self.assertIn(b'volume preparation blocked',result.stderr)
        self.assertFalse(self.marker.exists())

    def test_actual_ordinary_small_image_retains_allocation_through_nondiscard_format(self):
        commands={name:shutil.which(name) for name in ('fallocate','mke2fs','e2fsck','blkid')}
        if not all(commands.values()): self.skipTest('requires ext4 utilities')
        image=self.root/'ordinary16MiB.ext4'; image.touch(mode=0o600)
        size=16*1024**2; identity='ad35565f-96fc-4a49-8d72-180839cdf441'
        argv=[commands['fallocate'],'--length',str(size),str(image)]
        subprocess.run(argv,env=BOOT.ENV,capture_output=True,check=True,timeout=30)
        before=image.stat().st_blocks*512
        subprocess.run([commands['mke2fs'],'-q','-F','-U',identity,'-m','0','-E',
            'nodiscard,lazy_itable_init=0,lazy_journal_init=0','-t','ext4',str(image)],
            env=BOOT.ENV,capture_output=True,check=True,timeout=30)
        self.assertGreaterEqual(before,size); self.assertGreaterEqual(image.stat().st_blocks*512,size)
        subprocess.run([commands['e2fsck'],'-f','-n',str(image)],env=BOOT.ENV,capture_output=True,check=True,timeout=30)
        result=subprocess.run([commands['blkid'],'--probe','--output','export',str(image)],
                              env=BOOT.ENV,capture_output=True,check=True,timeout=30)
        self.assertIn(('UUID='+identity).encode(),result.stdout)
        self.assertIn(b'TYPE=ext4',result.stdout)


if __name__=='__main__': unittest.main()
