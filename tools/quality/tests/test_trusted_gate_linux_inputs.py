"""Readonly Linux bindings must verify bytes and frozen sources before launch."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[3]
SPEC=importlib.util.spec_from_file_location('linux_bound_gate',ROOT/'tools/trusted_gate/__init__.py',
                                         submodule_search_locations=[str(ROOT/'tools/trusted_gate')])
PACKAGE=importlib.util.module_from_spec(SPEC);sys.modules[SPEC.name]=PACKAGE;SPEC.loader.exec_module(PACKAGE)
from linux_bound_gate import runtime as RUNTIME, isolation as ISOLATION
from linux_bound_gate.common import Blocked, canonical, digest


class LinuxInputBindingTests(unittest.TestCase):
    def setUp(self):
        mask=os.umask(0o077);self.addCleanup(os.umask,mask)
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        self.root=Path(temp.name)
        def writable():
            for parent,dirs,names in os.walk(self.root):
                if not Path(parent).is_symlink():Path(parent).chmod(0o700)
                for name in names:
                    p=Path(parent)/name
                    if p.is_file() and not p.is_symlink():p.chmod(0o600)
        self.addCleanup(writable)
        self.source=self.root/'source';(self.source/'tools/harness-gate').mkdir(parents=True)
        (self.source/'test-data').mkdir()
        self.lock=self.source/'tools/harness-gate/Cargo.lock'
        self.lock.write_text('version = 4\n[[package]]\nname = "example"\nversion = "1.0.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "'+('a'*64)+'"\n')
        self.roles={role:self.root/role for role in RUNTIME.LINUX_MOUNTS}
        for path in self.roles.values():path.mkdir()
        package=self.roles['directory_sources']/'example-1.0.0';(package/'src').mkdir(parents=True)
        (package/'src/lib.rs').write_bytes(b'pub fn fixture() {}\n')
        (package/'.cargo-checksum.json').write_bytes(canonical({'package':'a'*64,
            'files':{'src/lib.rs':digest((package/'src/lib.rs').read_bytes())}})+b'\n')
        archive=b'compressed archive fixture';checksum=digest(archive)
        (self.roles['test_data_archives']/(checksum+'.tar.gz')).write_bytes(archive)
        self.manifest=self.source/'test-data/manifest.json'
        self.manifest.write_bytes(canonical({'schema':'external-test-data/v1','archives':[
            {'archive_size':len(archive),'archive_sha256':checksum}]}))
        onboarding=self.roles['onboarding'];(onboarding/'tools/onboarding').mkdir(parents=True)
        (onboarding/'tools/onboarding/acceptance.py').write_bytes(b'# do not execute\n')
        self.trees={role:self.inventory(path) for role,path in self.roles.items()}
        onb={n:[r[0],r[1],r[2]==0o555] for n,r in self.trees['onboarding']['files'].items()}
        self.onboarding_pin=digest(canonical(onb))
        self.metadata=self.root/'linux-inventory.json'
        self.value={'schema':'gatehouse-linux-inputs/v1','scope':RUNTIME.LINUX_SCOPE,'roles':self.trees,
            'lock_sha256':digest(self.lock.read_bytes()),'test_data_manifest_sha256':digest(self.manifest.read_bytes()),
            'onboarding_source_sha256':self.onboarding_pin}
        tools=self.root/'tools';(tools/'bin').mkdir(parents=True)
        for name in ['rustc','cargo','cargo-nextest','node','rustfmt','cargo-clippy']:
            (tools/'bin'/name).write_bytes(b'fixture');(tools/'bin'/name).chmod(0o555)
        tool_tree=self.inventory(tools)
        tool_inventory=self.root/'tools.json'
        tool_inventory.write_bytes(canonical({'schema':'gatehouse-official-tools/v1','tool_lock_sha256':'b'*64,
            **tool_tree,'archive_bytes':1,'scope':RUNTIME.SCOPE})+b'\n')
        cargo=self.root/'cargo.toml';cargo.write_text('[source.crates-io]\nreplace-with = "gatehouse-locked"\n'
            '[source.gatehouse-locked]\ndirectory = "/work/dependencies/sources"\n[net]\noffline = true\n')
        self.config={'runtime':{'root':str(tools),'inventory':str(tool_inventory),'tool_lock_sha256':'b'*64},
            'cargo_config':str(cargo),'source':str(self.source),
            'linux_inputs':{'inventory':str(self.metadata),**{role:str(path) for role,path in self.roles.items()}},
            'protected_files':{str(tool_inventory):digest(tool_inventory.read_bytes()),str(cargo):digest(cargo.read_bytes())}}
        self.repin()

    def inventory(self,path):
        files={};dirs=[]
        for parent,directories,names in os.walk(path):
            for name in directories:dirs.append(str((Path(parent)/name).relative_to(path)))
            for name in names:
                p=Path(parent)/name;raw=p.read_bytes();mode=0o555 if p.stat().st_mode&0o111 else 0o444
                p.chmod(mode);files[str(p.relative_to(path))]=[len(raw),digest(raw),mode]
            Path(parent).chmod(0o555)
        return {'files':files,'directories':sorted(dirs),'bytes':sum(r[0] for r in files.values())}

    def repin(self):
        self.metadata.write_bytes(canonical(self.value)+b'\n')
        self.config['protected_files'][str(self.metadata)]=digest(self.metadata.read_bytes())

    def protected(self,path,directory=False,private=False):
        # Only Root ownership is replaced inside this fixture; links, modes,
        # types, streaming reads, JSON and all source/content checks execute.
        path=Path(path)
        for p in (path,*path.parents):
            if p==self.root or self.root in p.parents:
                if p.is_symlink() or p.stat().st_mode&0o022:raise Blocked('unsafe fixture path')
        if not (path.is_dir() if directory else path.is_file()):raise Blocked('fixture type')
        return path

    def verify(self):
        with patch.object(RUNTIME,'protected',side_effect=self.protected), \
             patch.object(RUNTIME,'ONBOARDING_SHA256',self.onboarding_pin):
            return RUNTIME.verify(self.config)

    def test_all_role_bytes_and_source_bindings_verified_without_running_package_code(self):
        with patch('subprocess.run',side_effect=AssertionError('must not execute packages')):
            report=self.verify()
        self.assertEqual(report['linux_inputs']['packages'],1)
        self.assertEqual(report['linux_inputs']['files'],4)
        self.assertIsNone(RUNTIME.verify({}))

    def test_missing_role_runtime_cargo_or_external_pin_blocks(self):
        old=copy.deepcopy(self.config)
        for key in ['runtime','cargo_config']:
            self.config=copy.deepcopy(old);del self.config[key]
            with self.assertRaises(Blocked):self.verify()
        self.config=copy.deepcopy(old);del self.config['linux_inputs']['onboarding']
        with self.assertRaises(Blocked):self.verify()
        self.config=copy.deepcopy(old);del self.config['protected_files'][str(self.metadata)]
        with self.assertRaises(Blocked):self.verify()

    def test_unknown_role_or_metadata_scope_and_totals_fail_closed(self):
        for key,new in [('schema','unknown'),('scope','quality-pass'),('onboarding_source_sha256','0'*64)]:
            old=self.value[key];self.value[key]=new;self.repin()
            with self.assertRaises(Blocked):self.verify()
            self.value[key]=old
        self.value['roles']['extra']=self.trees['onboarding'];self.repin()
        with self.assertRaises(Blocked):self.verify()

    def test_overlap_with_scratch_control_source_other_roles_or_metadata_blocks(self):
        old=copy.deepcopy(self.config)
        for key in ['source','scratch','records','mirror','policy_directory']:
            self.config=copy.deepcopy(old);self.config[key]=str(self.roles['onboarding'])
            with self.assertRaises(Blocked):self.verify()
        for target in [str(self.roles['onboarding']),str(self.metadata),str(self.roles['directory_sources']/'child')]:
            self.config=copy.deepcopy(old);self.config['linux_inputs']['test_data_archives']=target
            with self.assertRaises(Blocked):self.verify()

    def test_mount_operand_injection_and_noncanonical_paths_block(self):
        for name in ['/tmp/a b','/tmp/a:/etc','/tmp/a%20','//tmp/a','/tmp/../a']:
            self.config['linux_inputs']['onboarding']=name
            with self.assertRaises(Blocked):self.verify()

    def test_valid_archive_filenames_are_data_while_relative_escape_still_blocks(self):
        root=self.root/'archive-name-fixture';(root/'src').mkdir(parents=True)
        (root/'src/space (test), v1!.rs').write_bytes(b'data only')
        value=self.inventory(root)
        with patch.object(RUNTIME,'protected',side_effect=self.protected):
            self.assertEqual(RUNTIME.input_tree(root,value),(9,1))
        for name in ['/absolute','../escape','a/../b','a//b','a\\b','a\nname','a\x00b']:
            with self.assertRaises(Blocked):RUNTIME.input_relative_path(name)

    def test_changed_input_without_metadata_or_with_rehashed_metadata_is_not_source_bound(self):
        path=self.roles['onboarding']/'tools/onboarding/acceptance.py'
        path.chmod(0o600);path.write_bytes(b'changed');path.chmod(0o444)
        with self.assertRaises(Blocked):self.verify()
        self.value['roles']['onboarding']=self.inventory(self.roles['onboarding']);self.repin()
        with self.assertRaisesRegex(Blocked,'onboarding'):self.verify()

    def test_source_lock_manifest_or_cargo_replacement_drift_blocks(self):
        for path in [self.lock,self.manifest,Path(self.config['cargo_config'])]:
            raw=path.read_bytes();path.write_bytes(raw+b'changed')
            with self.assertRaises(Blocked):self.verify()
            path.write_bytes(raw)
        cargo=Path(self.config['cargo_config']);cargo.write_text('[source.crates-io]\nregistry="file:///work/cache"\n')
        self.config['protected_files'][str(cargo)]=digest(cargo.read_bytes())
        with self.assertRaisesRegex(Blocked,'Cargo configuration'):self.verify()

    def test_cargo_checksum_rehash_cannot_authorize_changed_package_files(self):
        root=self.roles['directory_sources'];p=root/'example-1.0.0/src/lib.rs'
        p.chmod(0o600);p.write_bytes(b'changed Rust source');p.chmod(0o444)
        self.value['roles']['directory_sources']=self.inventory(root);self.repin()
        with self.assertRaisesRegex(Blocked,'checksum'):self.verify()

    def test_package_additions_or_locked_archive_checksum_changes_block(self):
        self.lock.write_text(self.lock.read_text().replace('a'*64,'b'*64))
        self.value['lock_sha256']=digest(self.lock.read_bytes());self.repin()
        with self.assertRaisesRegex(Blocked,'checksum'):self.verify()

    def test_testdata_archive_rehash_cannot_bypass_frozen_manifest(self):
        root=self.roles['test_data_archives'];path=next(root.iterdir())
        path.chmod(0o600);path.write_bytes(b'changed archive');path.chmod(0o444)
        self.value['roles']['test_data_archives']=self.inventory(root);self.repin()
        with self.assertRaisesRegex(Blocked,'archive bytes'):self.verify()

    def test_missing_extra_writable_linked_special_input_is_rejected(self):
        root=self.roles['onboarding'];root.chmod(0o700)
        p=root/'extra'
        for kind in ['file','directory','symlink','hardlink','fifo']:
            if kind=='file':p.write_bytes(b'extra')
            elif kind=='directory':p.mkdir()
            elif kind=='symlink':p.symlink_to(self.lock)
            elif kind=='hardlink':os.link(self.lock,p)
            else:os.mkfifo(p)
            root.chmod(0o555)
            with self.assertRaises(Blocked):self.verify()
            root.chmod(0o700)
            if kind=='directory':p.rmdir()
            else:p.unlink()
        root.chmod(0o555)
        existing=root/'tools/onboarding/acceptance.py';existing.chmod(0o666)
        with self.assertRaises(Blocked):self.verify()

    def test_declared_file_symlink_hardlink_and_missing_file_are_rejected(self):
        root=self.roles['test_data_archives'];path=next(root.iterdir());old=path.read_bytes()
        root.chmod(0o700)
        for kind in ['symlink','hardlink','missing']:
            path.unlink(missing_ok=True)
            if kind=='symlink':path.symlink_to(self.lock)
            elif kind=='hardlink':os.link(self.lock,path)
            root.chmod(0o555)
            with self.assertRaises(Blocked):self.verify()
            root.chmod(0o700)
        path.write_bytes(old);path.chmod(0o444);root.chmod(0o555)

    def test_strict_types_directories_size_and_combined_budgets_block(self):
        original=copy.deepcopy(self.value)
        for field,new in [('bytes',True),('directories',['unexpected'])]:
            self.value=copy.deepcopy(original);self.value['roles']['onboarding'][field]=new;self.repin()
            with self.assertRaises(Blocked):self.verify()
        self.value=original;self.repin()
        for name,limit in [('MAX_FILE',1),('MAX_TOTAL',1),('MAX_MEMBERS',1),('MAX_INVENTORY',1)]:
            with patch.object(RUNTIME,name,limit),self.assertRaises(Blocked):self.verify()

    def test_unknown_duplicate_or_noncanonical_inventory_cannot_be_authorized(self):
        for raw in [canonical(self.value),b'{"schema":"duplicate",'+self.metadata.read_bytes()[1:]]:
            self.metadata.write_bytes(raw);self.config['protected_files'][str(self.metadata)]=digest(raw)
            with self.assertRaises(Blocked):self.verify()

    def test_inventory_change_during_streaming_is_detected(self):
        original=RUNTIME.file_digest
        def changed(path,row):
            original(path,row)
            if str(path).startswith(str(self.roles['onboarding'])):self.metadata.write_bytes(b'changed')
        with patch.object(RUNTIME,'file_digest',side_effect=changed),self.assertRaises(Blocked):self.verify()

    def test_only_fixed_readonly_roles_are_added_to_sandbox_and_inventories_stay_hidden(self):
        self.config.update(executor_user='worker',records='/control',scratch='/scratch',cache={'root':'/cache'},
                           steps={'linux-test':{'argv':['/usr/bin/python3.14','-I','job.py'],
                             'git_history':False,'timeout_seconds':1260}},executor_memory_bytes=8*1024**3,
                           executor_file_bytes=1024**3)
        self.config['protected_files']['/usr/bin/python3.14']='a'*64
        with patch.object(RUNTIME,'protected',side_effect=self.protected):
            args=ISOLATION.sandbox_arguments(self.config,{'id':'fixture'},'linux-test',Path('/scratch/1'),Path('/cache/1'))
        ro=next(arg for arg in args if arg.startswith('--property=BindReadOnlyPaths='))
        for role,target in RUNTIME.LINUX_MOUNTS.items():self.assertIn(str(self.roles[role])+':'+target,ro)
        self.assertNotIn(str(self.metadata),ro)
        rw=next(arg for arg in args if arg.startswith('--property=BindPaths='))
        self.assertFalse(any(str(path) in rw for path in self.roles.values()))


if __name__=='__main__':unittest.main()
