"""Real bounded transport copies and inode checks; only Root admission is mocked."""
from contextlib import contextmanager, redirect_stderr, redirect_stdout
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[3]
SPEC=importlib.util.spec_from_file_location('administrator_staging',ROOT/'tools/trusted_gate/deploy/stage-administrator-inputs.py')
S=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(S)
SPEC=importlib.util.spec_from_file_location('administrator_packaging',ROOT/'tools/trusted_gate/deploy/package-administrator-inputs.py')
P=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(P)


class AdministratorStagingTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.source=self.root/'inputs';self.source.mkdir(mode=0o700)
        self.parent=self.root/'protected';self.parent.mkdir(mode=0o700);self.head='a'*40
        self.destination=self.parent/('gatehouse-issue98-linux-'+self.head[:12])
        self.temporary=self.destination.with_name('.'+self.destination.name+'.tmp')
        self.patch(S,'PARENT',self.parent);self.patch(S.os,'geteuid',lambda:0)
        self.patch(S,'root_owned',lambda info:info.st_uid==os.getuid())
        real_directory=S.directory
        @contextmanager
        def admission(path,*,protected=False):
            # Fixture ancestors in /tmp are not an actual protected Root tree.
            with real_directory(path) as fd:yield fd
        self.patch(S,'directory',admission)
        self.rows={}
        for name,data,mode in (
            ('helpers/tools/trusted_gate/deploy/import-protected-inputs.py',b'# importer\n',0o400),
            ('helpers/tools/trusted_gate/deploy/stage-administrator-inputs.py',b'# stage\n',0o400),
            ('bootstrap-source.tar',b'source-only fixture',0o400),('host-recipe.json',b'host',0o400),
            ('volume-tools.json',b'tools',0o400),('tool-archives/tool.tar.xz',b'toolarchive',0o400),
            ('crate-archives/crate.crate',b'cratearchive',0o400),('test-archives/test.tar.gz',b'testarchive',0o400),
            ('onboarding/tools/acceptance.py',b'# fixture executable\n',0o500)):
            self.put(name,data,mode)
        code={'schema':'gatehouse-bootstrap-code/v1','revision':self.head,'files':{
            name[8:]:row[1] for name,row in self.rows.items() if name.startswith('helpers/')}}
        self.put('helper-manifest.json',S.canonical(code));self.helperpin=self.rows['helper-manifest.json'][1]
        names={'release':'bootstrap-source.tar','host_recipe':'host-recipe.json','tool_recipe':'volume-tools.json',
               'tool_archives':'tool-archives','crate_archives':'crate-archives','test_archives':'test-archives','onboarding':'onboarding'}
        self.recipe={'schema':'gatehouse-administrator-upgrade/v1','head':self.head,'helper_manifest_sha256':self.helperpin,
                     'paths':{key:str(self.destination/name) for key,name in names.items()},
                     'pins':{key:self.rows[names[key]][1] for key in ('release','host_recipe','tool_recipe')},'api_proxy':None}
        self.put('administrator.json',S.canonical(self.recipe));self.seal()
        previous=os.umask(0o022);os.umask(previous);self.addCleanup(os.umask,previous)

    def patch(self,obj,name,value):
        p=patch.object(obj,name,value);p.start();self.addCleanup(p.stop)

    def put(self,name,raw,mode=0o400):
        path=self.source/name;path.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        if path.exists():path.chmod(0o600);path.unlink()
        S.write(path,raw,mode);self.rows[name]=[len(raw),S.digest(raw),mode]

    def seal(self):
        self.value={'schema':'gatehouse-administrator-transport/v1','head':self.head,
                    'helper_manifest_sha256':self.helperpin,'files':copy.deepcopy(self.rows)}
        self.raw=S.canonical(self.value);self.pin=S.digest(self.raw)
        path=self.source/S.META
        if path.exists():path.chmod(0o600);path.unlink()
        S.write(path,self.raw)

    def run_stage(self):return S.stage(self.source,self.pin)

    def test_copy_exact_private_tree_and_readonly_completed_retry(self):
        result=self.run_stage();self.assertFalse(result['administrator_ready']);self.assertFalse(result['production_gate_enabled'])
        self.assertFalse(self.temporary.exists())
        for name,row in self.rows.items():
            target=self.destination/name;self.assertEqual(target.read_bytes(),(self.source/name).read_bytes())
            self.assertEqual(stat.S_IMODE(target.stat().st_mode),row[2]);self.assertEqual(target.stat().st_nlink,1)
        before={str(p.relative_to(self.destination)):S.identity(p.stat()) for p in self.destination.rglob('*')}
        with patch.object(S,'write',side_effect=AssertionError('retry write')):self.assertEqual(self.run_stage(),result)
        self.assertEqual(before,{str(p.relative_to(self.destination)):S.identity(p.stat()) for p in self.destination.rglob('*')})

    def test_nonroot_rejected_before_reading_source(self):
        with patch.object(S.os,'geteuid',lambda:999),patch.object(S,'directory',side_effect=AssertionError('input read')):
            with self.assertRaisesRegex(ValueError,'authentication'):self.run_stage()

    def test_manifest_pin_and_noncanonical_duplicate_fields_rejected(self):
        for raw,pin in ((self.raw,'0'*64),(self.raw+b' ',S.digest(self.raw+b' ')),
                        (b'{"schema":1,"schema":1}\n',S.digest(b'{"schema":1,"schema":1}\n'))):
            with self.subTest(raw=raw[:20]),self.assertRaises(ValueError):S.manifest(raw,pin)
        self.assertFalse(self.destination.exists())

    def test_unknown_missing_or_empty_directory_rejected_before_copy(self):
        for name,kind in (('extra','file'),('empty','dir'),('onboarding/unreviewed','dir')):
            path=self.source/name
            if kind=='file':path.write_bytes(b'unknown')
            else:path.mkdir()
            with self.subTest(name=name),self.assertRaisesRegex(ValueError,'inventory'):self.run_stage()
            if kind=='file':path.unlink()
            else:path.rmdir()
        (self.source/'crate-archives/crate.crate').unlink()
        with self.assertRaisesRegex(ValueError,'inventory'):self.run_stage()
        self.assertFalse(self.temporary.exists())

    def test_symlink_leaf_directory_and_source_ancestor_rejected(self):
        target=self.source/'crate-archives/crate.crate';raw=target.read_bytes();target.unlink();target.symlink_to(self.source/'host-recipe.json')
        with self.assertRaises(ValueError):self.run_stage()
        target.unlink();S.write(target,raw)
        directory=self.source/'tool-archives';moved=self.source/'saved';directory.rename(moved);directory.symlink_to(moved,target_is_directory=True)
        with self.assertRaises(ValueError):self.run_stage()
        directory.unlink();moved.rename(directory)
        link=self.root/'alias';link.symlink_to(self.source,target_is_directory=True)
        with self.assertRaises(OSError):S.stage(link,self.pin)

    def test_hardlink_fifo_and_privileged_mode_rejected(self):
        target=self.source/'crate-archives/crate.crate';alias=self.root/'hardlink';os.link(target,alias)
        with self.assertRaisesRegex(ValueError,'type'):self.run_stage()
        alias.unlink();target.chmod(0o4400)
        with self.assertRaisesRegex(ValueError,'type'):self.run_stage()
        target.chmod(0o400);target.unlink();os.mkfifo(target)
        with self.assertRaisesRegex(ValueError,'type'):self.run_stage()

    def test_digest_mismatch_does_not_create_partial_stage(self):
        self.put('crate-archives/crate.crate',b'tampered')
        with self.assertRaisesRegex(ValueError,'digest'):self.run_stage()
        self.assertFalse(self.temporary.exists())

    def test_partial_copy_is_retained_and_never_adopted(self):
        real=S.stream
        def broken(fd,name,expected=None,output=None,**kw):
            if output is not None:
                output.write(b'partial');raise OSError('interrupted copy')
            return real(fd,name,expected,output,**kw)
        with patch.object(S,'stream',side_effect=broken),self.assertRaisesRegex(OSError,'interrupted'):self.run_stage()
        self.assertTrue(self.temporary.exists());self.assertFalse(self.destination.exists())
        before={str(p.relative_to(self.temporary)):p.read_bytes() for p in self.temporary.rglob('*') if p.is_file()}
        with self.assertRaisesRegex(ValueError,'unfinished'):self.run_stage()
        self.assertEqual(before,{str(p.relative_to(self.temporary)):p.read_bytes() for p in self.temporary.rglob('*') if p.is_file()})

    def test_final_rename_response_loss_replays_without_copy(self):
        real=S.os.rename
        def lost(source,destination):real(source,destination);raise OSError('rename response lost')
        with patch.object(S.os,'rename',side_effect=lost),self.assertRaisesRegex(OSError,'response'):self.run_stage()
        with patch.object(S,'write',side_effect=AssertionError('repeat copy')):
            self.assertEqual(self.run_stage()['head'],self.head)

    def test_existing_stage_unknown_file_wrong_mode_or_completion_blocks(self):
        self.run_stage();extra=self.destination/'unknown';S.write(extra,b'extra')
        with self.assertRaisesRegex(ValueError,'inventory'):self.run_stage()
        extra.unlink();leaf=self.destination/'host-recipe.json';leaf.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'mode'):self.run_stage()
        leaf.chmod(0o400);leaf=self.destination/S.COMPLETE;leaf.chmod(0o600);leaf.write_bytes(b'{}\n');leaf.chmod(0o400)
        with self.assertRaisesRegex(ValueError,'completion'):self.run_stage()

    def test_member_stream_rejects_inode_replacement_during_copy(self):
        path=self.source/'host-recipe.json'
        class Replace:
            def write(other,raw):
                path.unlink();S.write(path,raw)
        with S.directory(self.source) as fd:
            with self.assertRaisesRegex(ValueError,'changed'):S.stream(fd,path.name,output=Replace())

    def test_fd_ancestor_swap_never_reads_replacement_directory(self):
        with S.directory(self.source) as fd:
            moved=self.root/'moved';self.source.rename(moved);self.source.mkdir();(self.source/'host-recipe.json').write_bytes(b'foreign')
            self.assertEqual(S.read(fd,'host-recipe.json',1024),b'host')

    def test_transport_role_paths_modes_sizes_and_collision_closed(self):
        for name,row in (('../escape',[1,'0'*64,0o400]),('/absolute',[1,'0'*64,0o400]),
                         ('unknown/data',[1,'0'*64,0o400]),('tool-archives/executable',[1,'0'*64,0o500]),
                         ('onboarding/big',[S.MAX_FILE+1,'0'*64,0o400]),('onboarding/bool',[True,'0'*64,0o400]),
                         ('onboarding/tools',[1,'0'*64,0o400])):
            value=copy.deepcopy(self.value);value['files'][name]=row;raw=S.canonical(value)
            with self.subTest(name=name),self.assertRaises(ValueError):S.manifest(raw,S.digest(raw))

    def test_total_count_metadata_and_stream_budgets_enforced(self):
        with patch.object(S,'MAX_TOTAL',1),self.assertRaisesRegex(ValueError,'byte'):S.manifest(self.raw,self.pin)
        with patch.object(S,'MAX_FILES',1),self.assertRaises(ValueError):S.manifest(self.raw,self.pin)
        with patch.object(S,'MAX_METADATA',1),self.assertRaisesRegex(ValueError,'metadata'):self.run_stage()
        with S.directory(self.source) as fd:
            with self.assertRaisesRegex(ValueError,'budget'):S.stream(fd,'host-recipe.json',limit=1)

    def test_helper_and_recipe_bindings_must_match_transport_head_and_fixed_stage(self):
        for field,value in (('head','b'*40),('paths',{}),('pins',{}),('helper_manifest_sha256','b'*64)):
            recipe=copy.deepcopy(self.recipe);recipe[field]=value;self.put('administrator.json',S.canonical(recipe));self.seal()
            with self.subTest(field=field),self.assertRaisesRegex(ValueError,'recipe'):self.run_stage()
        self.put('administrator.json',S.canonical(self.recipe))
        code=json.loads((self.source/'helper-manifest.json').read_bytes());code['revision']='b'*40
        self.put('helper-manifest.json',S.canonical(code));self.helperpin=self.rows['helper-manifest.json'][1];self.seal()
        with self.assertRaisesRegex(ValueError,'helper identity'):self.run_stage()

    def test_unprotected_root_predicate_rejects_destination(self):
        self.run_stage()
        with patch.object(S,'root_owned',lambda info:False),self.assertRaisesRegex(ValueError,'directory mode'):self.run_stage()

    def test_transport_and_completion_metadata_cannot_be_executable(self):
        self.run_stage()
        for name in (S.META,S.COMPLETE):
            path=self.destination/name;path.chmod(0o500)
            with self.subTest(name=name),self.assertRaisesRegex(ValueError,'metadata mode'):self.run_stage()
            path.chmod(0o400)

    def test_cli_redacts_exception_content(self):
        out,err=io.StringIO(),io.StringIO()
        with patch('sys.argv',['stage','--source',str(self.source),'--transport-sha256',self.pin]), \
             patch.object(S,'stage',side_effect=ValueError('PRIVATE_MATERIAL')),redirect_stdout(out),redirect_stderr(err):
            self.assertEqual(S.main(),1)
        self.assertNotIn('PRIVATE_MATERIAL',out.getvalue()+err.getvalue());self.assertIn('ValueError',err.getvalue())


class AdministratorPackagingBoundaryTests(unittest.TestCase):
    def test_packager_rejects_root_before_import_or_output(self):
        with patch.object(P.os,'geteuid',lambda:0),patch.object(P,'load',side_effect=AssertionError('import')):
            with self.assertRaisesRegex(ValueError,'unprivileged'):P.package(ROOT,Path('/unused'),*[Path('/unused')]*6)

    def test_data_copy_matches_expected_hash_and_seals_mode(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'source';source.write_bytes(b'archive')
            destination=root/'copy';row=P.copy(source,destination,[7,S.digest(b'archive')],0o400)
            self.assertEqual(row,[7,S.digest(b'archive'),0o400]);self.assertEqual(destination.read_bytes(),b'archive')
            with self.assertRaisesRegex(ValueError,'digest'):P.copy(source,root/'bad',[7,'0'*64])
            self.assertTrue((root/'bad').exists())

    def test_reviewed_loader_pins_program_before_exec_and_only_stages_inputs(self):
        entry=P.launcher(Path('/reviewed/inputs'),'a'*40,'b'*64,'c'*64).decode()
        body=entry.split("<<'GATEHOUSE_REVIEWED_STAGE'\n",1)[1].rsplit('GATEHOUSE_REVIEWED_STAGE\n',1)[0]
        compile(body,'loader','exec')
        self.assertIn("source=Path('/reviewed/inputs')",body)
        self.assertIn("loader=parent/'gatehouse-issue98-stage-loader-aaaaaaaaaaaa'",body)
        self.assertIn("hashlib.sha256(raw).hexdigest()!='"+'c'*64+"'",body)
        self.assertIn("'--transport-sha256','"+'b'*64+"'",body)
        self.assertNotIn('administrator-upgrade',entry);self.assertNotIn('shell=True',entry)
        self.assertIn("os.O_EXCL|os.O_NOFOLLOW",body);self.assertIn("'-I',str(target)",body)

    def test_dirty_source_rejected_before_output_or_archive_work(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);output=root/'output'
            with patch.object(P.subprocess,'check_output',return_value=b' M changed\n'):
                with self.assertRaisesRegex(ValueError,'commit reviewed'):
                    P.package(root,output,*[root/'unused']*6)
            self.assertFalse(output.exists())


if __name__=='__main__':unittest.main()
