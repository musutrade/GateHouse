"""Private native-proc/file-engine integration; Root/preparation/API are replaced.

Actual sealed recipes, file modes/receipts/renames and bounded proc parsing run.
No native Root namespace, systemd, protected capture or remote acceptance occurs.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT=Path(__file__).resolve().parents[3]


def load(name,path):
    spec=importlib.util.spec_from_file_location(name,ROOT/path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX=load('installation_inputs_file_fixture','tools/quality/tests/test_trusted_gate_file_install.py')
PROC=load('installation_inputs_proc_fixture','tools/quality/tests/test_trusted_gate_controller_drain.py')
A=load('installation_inputs_library','tools/trusted_gate/deploy/observe-installation-inputs.py')
I=FIX.I;FENCE=FIX.FENCE;BOOT=FIX.BOOT


class InstallationInputsTests(unittest.TestCase):
    def setUp(self):
        self.m=FIX.ReviewedFileTransactionTests();self.m.setUp();self.addCleanup(self.m.doCleanups)
        self.f=self.m.f;f=self.f;self.B=f.B;self.B.ENV=BOOT.ENV
        self.B.authenticate=Mock(return_value={'revision':f.head})
        self.recipe_path=f.root/'installation-inputs.json';self.unit_root=f.root/'system-units';self.unit_root.mkdir(mode=0o700)
        paths={name:str(f.root/name) for name in A.PATHS}
        for name,path in paths.items():
            if name=='preservation':Path(path).mkdir(mode=0o700)
            else:BOOT.write(Path(path),b'authenticated boundary fixture',0o400)
        self.value={'schema':'gatehouse-installation-inputs/v1','head':f.head,'helper_manifest_sha256':'a'*64,
                    'paths':paths,'pins':{name:'b'*64 for name in A.PINS},'api_proxy':None}
        self.save_recipe()
        self.checked={'head':f.head,'helper_manifest_sha256':'a'*64,'preservation_sha256':'b'*64,
            'configuration_binding_sha256':'b'*64,'inspection_binding_sha256':'b'*64,
            'captured_core_policy_checked':True,'configuration_sha256':I.digest(self.m.materials['files'][str(f.config)]['after']),
            'plan_sha256':'c'*64,'installation_admitted':False,'administrator_ready':False,'production_gate_enabled':False}
        self.effect={'head':f.head,'helper_manifest_sha256':'a'*64,'preservation_sha256':'b'*64,'phase':'retired',
            'remote_revocation_confirmed':True,'retained_publication_verified':True,
            'api_transport_sha256':A.digest(BOOT.canonical({'https_proxy':None})),
            'installation_admitted':False,'administrator_ready':False,'production_gate_enabled':False}
        self.checker=SimpleNamespace(verify_completed=Mock(side_effect=lambda *args:copy.deepcopy(self.checked)))
        self.effects=SimpleNamespace(observe=Mock(side_effect=lambda *args,**kwargs:copy.deepcopy(self.effect)))
        self.builder=SimpleNamespace(worker_identity=Mock(),quiet_units=Mock(),idle=Mock())
        self.proc=PROC.ControllerDrainTests();self.proc.setUp();self.addCleanup(self.proc.doCleanups)
        self.context=Mock(return_value={'pid':'pid:[1]','user':'user:[1]','mnt':'mnt:[1]'})
        self.view=PROC.D.ProcView(self.proc.root)
        self.drain=SimpleNamespace(proc_context=self.context,ProcView=lambda:PROC.D.ProcView(self.proc.root))
        self.B.load=lambda helpers,name:{'install-transaction.py':FENCE,'install-reviewed-files.py':I,
            'bootstrap-release.py':self.m.contract,'check-core-policy.py':self.checker,
            'admit-upgrade-effects.py':self.effects,'controller-drain.py':self.drain,
            'build-protected-core.py':self.builder}[name]
        self.materializer=Mock(side_effect=lambda *args:copy.deepcopy(self.m.materials))
        self.command=Mock(side_effect=self.systemctl)

    def save_recipe(self):
        raw=BOOT.canonical(self.value)
        if self.recipe_path.exists():
            self.recipe_path.chmod(0o600);self.recipe_path.write_bytes(raw);self.recipe_path.chmod(0o400)
        else:BOOT.write(self.recipe_path,raw,0o400)
        self.recipe_pin=A.digest(raw)

    def systemctl(self,argv,**kwargs):
        self.assertEqual(argv[:2],['/usr/bin/systemctl','show']);self.assertIn(argv[2],A.UNITS)
        row={'Id':argv[2],'LoadState':'not-found','ActiveState':'inactive','SubState':'dead','UnitFileState':'',
             'FragmentPath':'','DropInPaths':'','Triggers':'','TriggeredBy':'','NeedDaemonReload':'no'}
        return SimpleNamespace(stdout=('\n'.join(k+'='+v for k,v in row.items())+'\n').encode(),stderr=b'')

    def observe(self,phase='before-files',**kwargs):
        with patch.object(A,'UNIT_ROOTS',(self.unit_root,)),patch.object(A,'materials',self.materializer), \
             patch.object(A.subprocess,'run',self.command),patch.object(A.os,'geteuid',return_value=0), \
             patch.object(A.os,'getpid',return_value=100):
            return A.observe(self.B,self.f.root/'helpers',self.f.root/'manifest','a'*64,self.recipe_path,
                             kwargs.pop('recipe_pin',self.recipe_pin),kwargs.pop('lease_fd',self.f.lease_fd),phase=phase,**kwargs)

    def guarded_prepare(self):
        def guard():
            self.observe('file-checkpoint' if self.m.receipt.exists() else 'before-files');return True
        I.prepare(self.B,FENCE,self.m.contract,self.m.materials,self.f.lease_fd,guard)
        return guard

    def test_before_files_observation_is_readonly_and_not_installation_acceptance(self):
        before=self.f.barrier.read_bytes()
        with patch.object(self.B,'write',side_effect=AssertionError('write')),patch.object(self.B,'record',side_effect=AssertionError('record')):
            result=self.observe()
        self.assertEqual(result['phase'],'before-files');self.assertEqual(result['scans'],2)
        self.assertEqual(result['processes'],2);self.assertTrue(result['enclosing_file_checkpoint_required'])
        for flag in ('installation_admitted','administrator_ready','production_gate_enabled','controller_drain_proven'):
            self.assertIs(result[flag],False)
        self.assertFalse(self.m.receipt.exists());self.assertEqual(self.f.barrier.read_bytes(),before)
        self.assertEqual(self.checker.verify_completed.call_count,2);self.assertEqual(self.effects.observe.call_count,2)

    def test_actual_file_engine_callback_allows_only_its_current_fresh_temporary_then_full_disabled_replay(self):
        guard=self.guarded_prepare()
        result=I.resume(self.B,FENCE,self.m.contract,self.m.materials,self.f.lease_fd,guard)
        self.assertEqual(result['phase'],'installed-disabled');self.assertEqual(len(result['files_done']),28)
        observed=self.observe('installed-disabled');self.assertFalse(observed['enclosing_file_checkpoint_required'])
        for entry in self.f.entries:self.assertEqual(entry.stat().st_mode&0o777,0)
        for flag in ('installation_admitted','administrator_ready','production_gate_enabled'):self.assertIs(observed[flag],False)

    def test_checkpoint_prerequisites_never_adopt_retained_temp_and_engine_rejects_it(self):
        guard=self.guarded_prepare();target=self.m.first_replace();temp=I.temporary(Path(target),self.f.transaction)
        self.m.crash_after_intent(target);BOOT.write(temp,self.m.materials['files'][target]['after'],self.m.materials['files'][target]['mode'])
        self.assertTrue(self.observe('file-checkpoint')['enclosing_file_checkpoint_required'])
        with self.assertRaisesRegex(ValueError,'retain partial'):
            I.resume(self.B,FENCE,self.m.contract,self.m.materials,self.f.lease_fd,guard)
        self.assertTrue(temp.exists())

    def test_standalone_completion_requires_full_engine_validation(self):
        self.m.prepare()
        with self.assertRaisesRegex(ValueError,'not complete'):self.observe('installed-disabled')
        self.m.resume();target=Path(self.m.read_state()['files_done'][0]['path']);target.chmod(0o600);target.write_bytes(b'changed')
        with self.assertRaises(ValueError):self.observe('installed-disabled')

    def test_recipe_pin_mode_head_helper_or_inventory_changes_are_rejected(self):
        with self.assertRaisesRegex(ValueError,'sealed reviewed'):self.observe(recipe_pin='f'*64)
        self.recipe_path.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'sealed reviewed'):self.observe()
        self.recipe_path.chmod(0o400)
        for key,value in (('head','f'*40),('helper_manifest_sha256','f'*64),('quality_passed',True)):
            original=copy.deepcopy(self.value);self.value[key]=value;self.save_recipe()
            with self.assertRaisesRegex(ValueError,'identity or inventory'):self.observe()
            self.value=original;self.save_recipe()

    def test_recipe_missing_extra_or_non_digest_pins_and_linked_input_block(self):
        original=copy.deepcopy(self.value)
        for pins in ({},dict(self.value['pins'],unknown='a'*64),dict(self.value['pins'],core=True)):
            self.value['pins']=pins;self.save_recipe()
            with self.assertRaisesRegex(ValueError,'identity or inventory'):self.observe()
        self.value=original;self.save_recipe();path=Path(self.value['paths']['host']);path.unlink();path.symlink_to('missing')
        with self.assertRaises(ValueError):self.observe()

    def test_preparation_inspection_failed_or_changed_binding_blocks(self):
        for key,value in (('captured_core_policy_checked',False),('installation_admitted',True),
                          ('inspection_binding_sha256','f'*64),('head','f'*40)):
            original=self.checked[key];self.checked[key]=value
            with self.subTest(key=key),self.assertRaisesRegex(ValueError,'preparation/inspection'):self.observe()
            self.checked[key]=original
        self.checker.verify_completed.side_effect=ValueError('prepared input changed')
        with self.assertRaisesRegex(ValueError,'prepared input changed'):self.observe()

    def test_retirement_remote_copy_route_or_authority_drift_blocks(self):
        for key,value in (('phase','revoked'),('remote_revocation_confirmed',False),('retained_publication_verified',False),
                          ('api_transport_sha256','f'*64),('preservation_sha256','f'*64),('administrator_ready',True)):
            original=self.effect[key];self.effect[key]=value
            with self.subTest(key=key),self.assertRaisesRegex(ValueError,'retired remote/publication'):self.observe()
            self.effect[key]=original

    def test_explicit_route_is_forwarded_and_bound_to_recipe(self):
        self.value['api_proxy']='http://proxy.example.invalid:7890';self.save_recipe()
        with self.assertRaisesRegex(ValueError,'retired remote/publication'):self.observe()
        self.effect['api_transport_sha256']=A.digest(BOOT.canonical({'https_proxy':self.value['api_proxy']}))
        self.observe();self.assertEqual(self.effects.observe.call_args.kwargs,{'api_proxy':self.value['api_proxy']})

    def test_unknown_existing_file_receipt_or_directory_blocks_initial_observation(self):
        self.m.prepare()
        with self.assertRaisesRegex(ValueError,'existing file installation receipt'):self.observe()
        self.m.receipt.unlink();next(iter(I.DIRECTORIES)).mkdir(mode=0o700)
        with self.assertRaisesRegex(ValueError,'fresh installation directory'):self.observe()

    def test_entry_reopened_active_barrier_or_wrong_material_fence_blocks(self):
        entry=self.f.entries[0];entry.chmod(0o755)
        with self.assertRaises(ValueError):self.observe()
        entry.chmod(0);raw=self.f.barrier.read_bytes();value=json.loads(raw);value['phase']='active'
        self.f.barrier.write_bytes(BOOT.canonical(value))
        with self.assertRaisesRegex(ValueError,'changed or advanced'):self.observe()
        self.f.barrier.write_bytes(raw);self.m.materials['files'][str(self.f.config)]['after']=b'other configuration'
        with self.assertRaisesRegex(ValueError,'original fenced generation'):self.observe()

    def test_managed_root_or_restricted_process_blocks_real_proc_scan(self):
        self.proc.process(200,uids=(995,995,995,995))
        with self.assertRaises(ValueError):self.observe()
        self.proc.process(200,argv=b'/usr/bin/python3.14\0/opt/gatehouse/trusted_gate/host.py\0')
        with self.assertRaises(ValueError):self.observe()

    def test_namespace_or_process_identity_changes_between_scans_block(self):
        self.context.side_effect=[{'pid':'pid:[1]'}, {'pid':'pid:[2]'}]
        with self.assertRaisesRegex(ValueError,'namespace changed'):self.observe()
        self.context.side_effect=None
        class Changing:
            def __init__(self,outer):self.outer=outer;self.calls=0
            def scan(self,caller):
                self.calls+=1
                if self.calls==2:self.outer.proc.process(200,start=999)
                return self.outer.view.scan(caller)
        self.drain.ProcView=lambda:Changing(self)
        with self.assertRaisesRegex(ValueError,'process observations changed'):self.observe()

    def test_unknown_unit_dropin_or_dangling_link_blocks_without_manager_change(self):
        path=self.unit_root/A.UNITS[0];path.symlink_to('missing')
        with self.assertRaisesRegex(ValueError,'unit or drop-in appeared'):self.observe()
        path.unlink();dropin=self.unit_root/(A.UNITS[1]+'.d');dropin.mkdir(mode=0o700)
        with self.assertRaisesRegex(ValueError,'unit or drop-in appeared'):self.observe()
        self.assertTrue(dropin.exists())

    def test_loaded_enabled_triggered_reload_or_malformed_manager_state_blocks(self):
        real=self.systemctl
        for key,value in (('LoadState','loaded'),('ActiveState','active'),('UnitFileState','enabled'),
                          ('TriggeredBy','other.timer'),('NeedDaemonReload','yes')):
            def command(argv,key=key,value=value,**kwargs):
                result=real(argv,**kwargs)
                lines=result.stdout.decode().splitlines();result.stdout=('\n'.join(
                    key+'='+value if line.startswith(key+'=') else line for line in lines)+'\n').encode()
                return result
            self.command.side_effect=command
            with self.subTest(key=key),self.assertRaisesRegex(ValueError,'autostart is not proven'):self.observe()
        self.command.side_effect=lambda *args,**kwargs:SimpleNamespace(stdout=b'Id=unexpected\nId=duplicate\n',stderr=b'')
        with self.assertRaisesRegex(ValueError,'properties differ'):self.observe()

    def test_late_remote_or_inspection_drift_is_rejected(self):
        first=copy.deepcopy(self.effect);second=copy.deepcopy(first);second['journal_sha256']='f'*64
        self.effects.observe.side_effect=[first,second]
        with self.assertRaisesRegex(ValueError,'remote/publication state changed'):self.observe()
        self.effects.observe.side_effect=lambda *args,**kwargs:copy.deepcopy(self.effect)
        checked=copy.deepcopy(self.checked);checked['plan_sha256']='d'*64
        self.checker.verify_completed.side_effect=[copy.deepcopy(self.checked),checked]
        with self.assertRaisesRegex(ValueError,'preparation/inspection changed'):self.observe()

    def test_late_material_substitution_is_rejected_before_returning(self):
        original=copy.deepcopy(self.m.materials);changed=copy.deepcopy(original)
        changed['files'][self.m.first_replace()]['after']=b'changed source material'
        self.materializer.side_effect=[original,changed]
        with self.assertRaisesRegex(ValueError,'materials changed during'):self.observe()

    def test_recipe_rewrite_during_inspection_cannot_use_the_old_pin(self):
        calls=0
        def check(*args):
            nonlocal calls
            calls+=1
            if calls==2:
                self.value['api_proxy']='http://new-route.invalid:80';self.save_recipe()
            return copy.deepcopy(self.checked)
        self.checker.verify_completed.side_effect=check
        with self.assertRaisesRegex(ValueError,'sealed reviewed'):self.observe()

    def test_late_autostart_unit_is_retained_and_blocks(self):
        path=self.unit_root/A.UNITS[0]
        def effect(*args,**kwargs):
            if not path.exists():BOOT.write(path,b'unknown reconcile unit',0o600)
            return copy.deepcopy(self.effect)
        self.effects.observe.side_effect=effect
        with self.assertRaisesRegex(ValueError,'unit or drop-in appeared'):self.observe()
        self.assertEqual(path.read_bytes(),b'unknown reconcile unit')

    def test_file_receipt_drift_inside_live_observation_is_rejected(self):
        self.m.prepare();calls=0
        def effect(*args,**kwargs):
            nonlocal calls
            calls+=1
            if calls==2:
                state=self.m.read_state();state['entries_enabled']=True;self.m.save_state(state)
            return copy.deepcopy(self.effect)
        self.effects.observe.side_effect=effect
        with self.assertRaisesRegex(ValueError,'file checkpoint context'):self.observe('file-checkpoint')

    def test_unavailable_or_ambiguous_manager_observation_blocks(self):
        self.command.side_effect=OSError('system manager unavailable')
        with self.assertRaisesRegex(OSError,'manager unavailable'):self.observe()
        self.command.side_effect=lambda *args,**kwargs:SimpleNamespace(stdout=b'',stderr=b'unavailable')
        with self.assertRaisesRegex(ValueError,'ambiguous'):self.observe()

    def test_active_worker_budget_and_invalid_phase_lease_fail_before_preparation(self):
        for phase in ('active','complete',True):
            with self.assertRaisesRegex(ValueError,'known installation phase'):self.observe(phase)
        for descriptor in (None,True,-1):
            with self.assertRaisesRegex(ValueError,'known installation phase'):self.observe(lease_fd=descriptor)
        with self.f.config.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError,'lease'):self.observe(lease_fd=wrong.fileno())
        with patch.object(A,'MAX_SECONDS',-1),self.assertRaisesRegex(ValueError,'time budget'):self.observe()
        self.builder.quiet_units.side_effect=ValueError('worker active')
        with self.assertRaisesRegex(ValueError,'worker active'):self.observe()
        self.checker.verify_completed.assert_not_called()

    def test_nonroot_and_direct_execution_have_no_administrator_entry(self):
        with patch.object(A.os,'geteuid',return_value=1000),patch.object(self.B,'load') as imported:
            with self.assertRaisesRegex(ValueError,'administrator'):
                A.observe(self.B,*([None]*6),phase='before-files')
        imported.assert_not_called()
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/observe-installation-inputs.py')],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'no administrator entry',result.stderr)


class InstallationMaterialAssemblyTests(unittest.TestCase):
    def setUp(self):
        self.m=FIX.ReviewedFileMaterialTests();self.m.setUp();self.addCleanup(self.m.doCleanups)
        m=self.m;self.f=m.f;f=self.f;self.B=SimpleNamespace(**vars(f.B))
        self.volume=f.root/'linux';(self.volume/'bootstrap-core').mkdir(parents=True,mode=0o700)
        (self.volume/'bootstrap-host-config').mkdir(mode=0o700)
        self.core=self.volume/'bootstrap-core/harness-gate';BOOT.write(self.core,m.core,0o555)
        self.config=self.volume/'bootstrap-host-config/host.json';BOOT.write(self.config,BOOT.canonical(m.config),0o600)
        self.plan=self.volume/'bootstrap-host-config/install-plan.json';BOOT.write(self.plan,b'reviewed plan fixture',0o400)
        raw=f.encoded();BOOT.write(f.archive,raw,0o600)
        self.value={'paths':{'archive':str(f.archive),'preservation':str(f.root/'preservation')},
                    'pins':{'archive':A.digest(raw),'preservation':'b'*64}}
        self.checked={'configuration_sha256':A.digest(self.config.read_bytes()),'plan_sha256':A.digest(self.plan.read_bytes())}
        self.preflight=SimpleNamespace(accepted_release=Mock(return_value={'bin/harness-gate':m.old_core}))
        self.compose=SimpleNamespace(retained_configuration=Mock(return_value=m.old))
        self.B.load=lambda helpers,name:{'bootstrap-release.py':FIX.R,'preflight-history-pilot.py':self.preflight,
            'compose-linux-host.py':self.compose,'build-protected-core.py':SimpleNamespace(MAX_BINARY=I.MAX_FILE),
            'preserve-pilot-state.py':SimpleNamespace(),'install-reviewed-files.py':I}[name]

    def assemble(self):
        f=self.f
        with patch.object(A,'VOLUME',self.volume):
            return A.materials(self.B,f.helpers,f.manifest,f.manifest_pin,f.code,self.value,self.checked)

    def test_full_release_decode_and_materialization_derive_exact28_disabled_destinations(self):
        result=self.assemble();self.assertEqual(len(result['files']),28);self.assertEqual(result['head'],self.f.head)
        for path in I.ENTRIES:self.assertEqual(result['files'][str(path)]['mode'],0)
        self.assertEqual(result['files'][str(I.CORE)]['after'],self.m.core)

    def test_archive_config_and_plan_must_match_external_pins(self):
        raw=self.f.archive.read_bytes();self.f.archive.write_bytes(b'changed archive')
        with self.assertRaises(ValueError):self.assemble()
        self.f.archive.write_bytes(raw);self.checked['plan_sha256']='f'*64
        with self.assertRaisesRegex(ValueError,'completed inspection'):self.assemble()
        self.checked['plan_sha256']=A.digest(self.plan.read_bytes());self.checked['configuration_sha256']='f'*64
        with self.assertRaisesRegex(ValueError,'completed inspection'):self.assemble()

    def test_old_or_captured_core_substitution_and_mode_drift_are_rejected(self):
        self.preflight.accepted_release.return_value={'bin/harness-gate':b'unknown old Core'}
        with self.assertRaisesRegex(ValueError,'material pins'):self.assemble()
        self.preflight.accepted_release.return_value={'bin/harness-gate':self.m.old_core};self.core.chmod(0o644)
        with self.assertRaisesRegex(ValueError,'Core mode differs'):self.assemble()
        self.core.write_bytes(b'unknown new Core');self.core.chmod(0o555)
        with self.assertRaisesRegex(ValueError,'material pins'):self.assemble()

    def test_rehashed_archive_cannot_replace_fixed_old_source_references(self):
        name=next(iter(FIX.R.old_sources()));self.f.payload['before/'+name]=b'changed old reference';self.f.seal()
        raw=self.f.encoded();self.f.archive.write_bytes(raw);self.value['pins']['archive']=A.digest(raw)
        with self.assertRaisesRegex(ValueError,'source inventory differs'):self.assemble()


if __name__=='__main__':unittest.main()
