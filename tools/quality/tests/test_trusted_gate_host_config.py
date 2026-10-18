"""Configuration cannot promote partial records, stale pins or an ordinary Core.

Root/source-release/import/kernel/worker boundaries are explicitly replaced;
small real Git, copied inputs, metadata pins and the controller verifier run.
"""
import copy
import fcntl
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

ROOT=Path(__file__).resolve().parents[3]


def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


FIX=load('host_config_seal_fixture',ROOT/'tools/quality/tests/test_trusted_gate_linux_seal.py')
C=load('host_config_library',ROOT/'tools/trusted_gate/deploy/compose-linux-host.py')
BUILD=load('host_config_builder',ROOT/'tools/trusted_gate/deploy/build-protected-core.py')
R=load('host_config_release',ROOT/'tools/trusted_gate/deploy/bootstrap-release.py')
BOOT=FIX.BOOT


class HostConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.fixture=FIX.LinuxInputSealingTests();self.fixture.setUp();self.addCleanup(self.fixture.doCleanups)
        f=self.fixture;sealed=f.seal();self.root=f.root;self.volume=f.volume;self.control=f.control;self.head=f.head
        self.source_root=self.volume/'bootstrap-source';self.source_root.mkdir()
        self.source=self.source_root/'source';f.source.rename(self.source)
        self.inputs=self.volume/'bootstrap-inputs';f.inputs.rename(self.inputs)
        tools=self.inputs/'tools';tools.mkdir()
        original=f.fixture.config['runtime'];Path(original['root']).chmod(0o700)
        Path(original['root']).rename(tools/'toolchain');(tools/'toolchain').chmod(0o555)
        Path(original['inventory']).rename(tools/'inventory.json')
        self.sealed_root=self.volume/'bootstrap-linux-inputs'
        sealed['base']=C.BASE
        sealed['linux_inputs']['directory_sources']=str(self.inputs/'dependencies/sources')
        self.sealed=sealed
        self.seal_pin=self.rewrite_seal()
        self.payload={'files/'+name:(ROOT/name).read_bytes() for name in R.source_map()}
        # The fixture pins a tiny onboarding tree rather than the real469-file source.
        runtime_key='files/tools/trusted_gate/runtime.py'
        self.payload[runtime_key]=self.payload[runtime_key].replace(
            FIX.FIX.RUNTIME.ONBOARDING_SHA256.encode(),f.fixture.onboarding_pin.encode())
        self.payload.update({'before/'+name:('synthetic old '+name).encode() for name in R.old_sources()})
        self.payload['source.bundle']=b'fixture source bundle'
        BOOT.write(self.source_root/'candidate.bundle',self.payload['source.bundle'],0o400)
        self.tree='e'*40
        BOOT.write(self.source_root/'frozen.json',BOOT.canonical({'repository':'musutrade/GateHouse','repository_id':1414069625,
            'head':self.head,'base':C.BASE,'bundle_sha256':C.digest(self.payload['source.bundle'])}),0o400)
        BOOT.write(self.source_root/'stage.json',BOOT.canonical({'schema':'gatehouse-bootstrap-source/v1','state':'staged',
            'head':self.head,'base':C.BASE,'helper_manifest_sha256':'a'*64,'bundle_sha256':C.digest(self.payload['source.bundle']),
            'tree':self.tree}),0o600)
        self.archive=self.root/'release.tar';BOOT.write(self.archive,b'source archive fixture',0o600)
        self.manifest=self.root/'manifest.json';BOOT.write(self.manifest,b'fixture code manifest',0o400)
        self.core_root=self.volume/'bootstrap-core';self.core_root.mkdir()
        BOOT.write(self.core_root/'harness-gate',b'Root capture fixture; never executed',0o555)
        BOOT.write(self.core_root/'cargo-config.toml',C.CARGO)
        self.host={'cc':'/usr/bin/fixture-cc','ar':'/usr/bin/fixture-ar','env':'/usr/bin/env'}
        unit='gatehouse-bootstrap-6fa8dc35-6d4c-4e51-8959-24c220513f61.service'
        argv,env=BUILD.command(BOOT,self.core_root/'source',self.inputs,self.core_root/'worker',
                               self.core_root/'cargo-config.toml',unit,self.host)
        self.record={'schema':'gatehouse-core-bootstrap/v1','state':'captured','head':self.head,'base':C.BASE,
            'helper_revision':self.head,'helper_manifest_sha256':'a'*64,'input_binding_sha256':'b'*64,
            'host_recipe_sha256':'c'*64,'profile':'release','release_incremental':False,'worker_uid':995,'worker_gid':981,
            'target':'x86_64-unknown-linux-gnu','worker_stop_proven':True,'exit_code':0,'production_gate_enabled':False,
            'unit':unit,'source':{'tree':BUILD.CORE_TREE},'argv':argv,'environment':env,
            'binary':{'sha256':C.digest((self.core_root/'harness-gate').read_bytes()),'bytes':(self.core_root/'harness-gate').stat().st_size}}
        self.core_pin=self.rewrite_core()
        self.B=SimpleNamespace(**{name:getattr(BOOT,name) for name in (
            'need','path_operand','write','record','canonical','unique','ENV')})
        self.B.owned=f.owned;self.B.read=self.read
        self.B.authenticate=Mock(return_value={'revision':self.head})
        def git(source,*args):
            if args[0]=='status':return b''
            if args==('rev-parse','HEAD^{tree}'):return self.tree.encode()+b'\n'
            if args[0]=='rev-parse' and args[1].endswith(':tools/harness-gate'):return BUILD.CORE_TREE.encode()+b'\n'
            raise AssertionError(args)
        self.B.git=git
        self.imported={'helper_revision':self.head,'helper_manifest_sha256':'a'*64,
            'tool_inventory_sha256':C.digest((tools/'inventory.json').read_bytes()),'tool_lock_sha256':'b'*64}
        self.os_files={'/usr/bin/python3.14':b'fixed OS Python fixture','/usr/bin/git':b'fixed OS Git fixture'}
        self.builder=SimpleNamespace(pilot_slot=Mock(),worker_identity=Mock(),quiet_units=Mock(),idle=Mock(),
            imported_inputs=Mock(side_effect=lambda *args:copy.deepcopy(self.imported)),json_pin=BUILD.json_pin,
            CORE_TREE=BUILD.CORE_TREE,MAX_BINARY=BUILD.MAX_BINARY,host_inputs=Mock(return_value=self.host),command=BUILD.command,
            stopped=Mock(),verify_snapshot=Mock(),host_file=Mock(side_effect=lambda name:self.os_files[name]))
        self.contract=SimpleNamespace(**{name:getattr(R,name) for name in ('MAX_ARCHIVE','MAX_BUNDLE','source_map','plan','ORDER')})
        self.contract.inspect=Mock();self.contract.decode=Mock(side_effect=lambda *args:(self.payload,{}))
        self.contract.bind_source=Mock()
        self.preflight=SimpleNamespace(observe=Mock(return_value={'logical_snapshot':'fixed659'}))
        self.storage=SimpleNamespace(admit=Mock())
        self.B.load=lambda helpers,name:{'build-protected-core.py':self.builder,'prepare-linux-volume.py':self.storage,
            'bootstrap-release.py':self.contract,'preflight-history-pilot.py':self.preflight,'linux-transition-policy.py':R.LINUX}[name]
        self.config_path=self.root/'host.json'
        self.old={'records':str(self.control),'inbox':str(self.control.parent/'inbox'),'app_id':4867361,
            'delivery':{'app_id':5069731,'installation_id':164696480,'app_key':'/etc/gatehouse/delivery.private-key.pem'},
            'coverage_acceptance':'transition','protected_files':{'/opt/gatehouse/bin/harness-gate':'d'*64,
                **{name:C.digest(raw) for name,raw in self.os_files.items()}},
            'unknown_extension':{'can_publish':True},'evidence_url_prefix':'https://unreviewed.invalid/'}
        BOOT.write(self.config_path,BOOT.canonical(self.old),0o600)
        self.output=self.volume/'bootstrap-host-config'

    def read(self,path,limit):
        path=self.B.owned(path);raw=path.read_bytes();BOOT.need(len(raw)<=limit,'fixture read bound');return raw

    def rewrite_seal(self):
        path=self.sealed_root/'role-binding.json';path.chmod(0o600);path.write_bytes(BOOT.canonical(self.sealed));path.chmod(0o400)
        pin=C.digest(path.read_bytes());state={key:self.sealed[key] for key in (
            'schema','state','head','base','helper_manifest_sha256','input_binding_sha256','administrator_ready','production_gate_enabled')}
        BOOT.record(self.sealed_root/'seal.json',{**state,'binding_sha256':pin})
        return pin

    def rewrite_core(self):
        path=self.core_root/'build.json'
        if path.exists():path.write_bytes(BOOT.canonical(self.record))
        else:BOOT.write(path,BOOT.canonical(self.record),0o600)
        return C.digest(path.read_bytes())

    def compose(self,**options):
        with patch.multiple(C,VOLUME=self.volume,CONTROL=self.control,INBOX=self.control.parent/'inbox',CONFIG=self.config_path), \
             patch.object(C.os,'geteuid',return_value=0):
            return C.compose(self.B,self.root/'helpers',self.manifest,'a'*64,self.archive,C.digest(self.archive.read_bytes()),
                'b'*64,self.seal_pin,self.core_pin,self.root/'host-recipe','c'*64,self.root/'volume-recipe','d'*64,**options)

    def test_explicit_administrator_api_route_is_rechecked_on_both_preflights(self):
        proxy='http://proxy.example.invalid:7890';self.compose(api_proxy=proxy)
        self.assertEqual(self.preflight.observe.call_count,2)
        self.assertTrue(all(call.kwargs['api_proxy']==proxy for call in self.preflight.observe.call_args_list))

    def test_small_input_configuration_roundtrip_pins_all_files_without_optional_old_overlays(self):
        before=self.config_path.read_bytes();result=self.compose()
        config=json.loads((self.output/'host.json').read_bytes());plan=json.loads((self.output/'install-plan.json').read_bytes())
        self.assertEqual(result['state'],'composed');self.assertEqual(result['execution_pins'],29)
        self.assertEqual(result['runtime_verification']['linux_inputs']['packages'],1)
        self.assertFalse(result['administrator_ready']);self.assertFalse(result['captured_core_policy_checked'])
        self.assertFalse(result['production_gate_enabled']);self.assertEqual(result['authoritative_publish'],'blocked')
        self.assertEqual(config['coverage_acceptance'],'transition');self.assertEqual(config['coverage'],R.LINUX.COVERAGE)
        self.assertNotIn('unknown_extension',config);self.assertNotIn('evidence_url_prefix',config)
        self.assertEqual(config['records'],str(self.control));self.assertEqual(config['delivery'],self.old['delivery'])
        self.assertEqual(config['steps']['linux-test']['capture_files'],R.LINUX.worker()['capture_files'])
        self.assertEqual(config['steps']['linux-test']['cache_identity']['release_incremental'],'not-applicable')
        self.assertTrue(config['steps']['pilot-boundary']['run_arguments'])
        self.assertEqual(config['steps']['pilot-boundary']['argv'][-2:],['@gatehouse:head@','@gatehouse:base@'])
        self.assertEqual(len(plan['files']),27);self.assertEqual(plan['files']['captured-Core']['after_sha256'],self.record['binary']['sha256'])
        for name in ['run-linux-test.py','prepare-linux-workspace.py']:
            self.assertIn('/opt/gatehouse/linux-test/'+name,config['protected_files'])
        for name,mode in [('host.json',0o600),('install-plan.json',0o400),('binding.json',0o400)]:
            self.assertEqual(stat.S_IMODE((self.output/name).stat().st_mode),mode)
        self.assertEqual(C.digest((self.output/'host.json').read_bytes()),result['configuration_sha256'])
        self.assertEqual(C.digest((self.output/'binding.json').read_bytes()),json.loads((self.output/'compose.json').read_bytes())['binding_sha256'])
        self.assertEqual(self.config_path.read_bytes(),before)
        self.assertFalse((self.volume/'scratch').exists());self.assertFalse((self.volume/'cache').exists())
        self.assertFalse(any(name.startswith('_gatehouse_config_') for name in sys.modules))

    def test_existing_output_is_never_reused(self):
        self.compose()
        with self.assertRaisesRegex(ValueError,'never reuse'):self.compose()
        self.assertEqual(json.loads((self.output/'compose.json').read_bytes())['state'],'composed')

    def test_completed_candidate_can_be_reverified_with_external_pin_without_any_writes(self):
        result=self.compose();pin=C.digest((self.output/'binding.json').read_bytes())
        before={path:path.read_bytes() for path in self.output.iterdir()}
        with patch.object(self.B,'write',side_effect=AssertionError('unexpected write')), \
             patch.object(self.B,'record',side_effect=AssertionError('unexpected state change')):
            self.assertEqual(self.compose(existing_pin=pin),result)
        self.assertEqual({path:path.read_bytes() for path in self.output.iterdir()},before)

    def test_existing_candidate_requires_pin_complete_state_and_exact_outputs(self):
        self.compose();pin=C.digest((self.output/'binding.json').read_bytes())
        with self.assertRaises(ValueError):self.compose(existing_pin='f'*64)
        status=self.output/'compose.json';before=status.read_bytes();value=json.loads(before);value['state']='blocked'
        status.write_bytes(BOOT.canonical(value))
        with self.assertRaises(ValueError):self.compose(existing_pin=pin)
        self.assertEqual(status.read_bytes(),BOOT.canonical(value));status.write_bytes(before)
        path=self.output/'host.json';path.write_bytes(b'changed')
        with self.assertRaises(ValueError):self.compose(existing_pin=pin)
        self.assertEqual(status.read_bytes(),before)

    def test_rehashed_existing_binding_cannot_promote_acceptance(self):
        self.compose();path=self.output/'binding.json';value=json.loads(path.read_bytes());value['administrator_ready']=True
        path.chmod(0o600);path.write_bytes(BOOT.canonical(value));path.chmod(0o400)
        with self.assertRaises(ValueError):self.compose(existing_pin=C.digest(path.read_bytes()))

    def test_existing_candidate_mode_drift_cannot_be_reverified(self):
        self.compose();pin=C.digest((self.output/'binding.json').read_bytes());(self.output/'host.json').chmod(0o644)
        with self.assertRaisesRegex(ValueError,'mode'):self.compose(existing_pin=pin)

    def test_wrong_seal_external_pin_blocks_even_valid_completed_record(self):
        self.seal_pin='0'*64
        with self.assertRaises(ValueError):self.compose()
        self.assertFalse(self.output.exists())

    def test_incomplete_or_blocked_seal_status_cannot_be_promoted_by_binding_presence(self):
        path=self.sealed_root/'seal.json';original=path.read_bytes()
        for state in ['incomplete','blocked']:
            value=json.loads(original);value['state']=state;path.write_bytes(BOOT.canonical(value))
            with self.assertRaises(ValueError):self.compose()
        self.assertFalse(self.output.exists())

    def test_rehashed_seal_cannot_change_paths_source_revision_or_authority(self):
        original=copy.deepcopy(self.sealed)
        changes=[('head','f'*40),('helper_manifest_sha256','f'*64),('administrator_ready',0),('production_gate_enabled',True)]
        for key,value in changes:
            self.sealed=copy.deepcopy(original);self.sealed[key]=value;self.seal_pin=self.rewrite_seal()
            with self.assertRaises(ValueError):self.compose()
        self.sealed=copy.deepcopy(original);self.sealed['linux_inputs']['onboarding']=str(self.source)
        self.seal_pin=self.rewrite_seal()
        with self.assertRaises(ValueError):self.compose()

    def test_rehashed_core_record_must_still_match_completed995_release_semantics(self):
        original=copy.deepcopy(self.record)
        for key,value in [('state','running'),('head','f'*40),('worker_uid',1000),('worker_stop_proven',False),
                          ('exit_code',False),('profile','debug'),('release_incremental',True),('helper_manifest_sha256','f'*64),
                          ('unit','gatehouse-bootstrap-00000000-0000-0000-0000-000000000000.service')]:
            self.record=copy.deepcopy(original);self.record[key]=value;self.core_pin=self.rewrite_core()
            with self.assertRaises(ValueError):self.compose()
        self.assertFalse(self.output.exists())

    def test_core_recipe_arguments_cannot_be_replaced_even_with_new_record_pin(self):
        self.record['argv']=['/usr/bin/true'];self.core_pin=self.rewrite_core()
        with self.assertRaisesRegex(ValueError,'recipe'):self.compose()

    def test_core_captured_mode_bytes_or_external_record_pin_must_match(self):
        binary=self.core_root/'harness-gate';binary.chmod(0o755)
        with self.assertRaises(ValueError):self.compose()
        binary.chmod(0o555);self.core_pin='f'*64
        with self.assertRaises(ValueError):self.compose()
        self.core_pin=C.digest((self.core_root/'build.json').read_bytes());binary.chmod(0o755);binary.write_bytes(b'changed');binary.chmod(0o555)
        with self.assertRaises(ValueError):self.compose()

    def test_runtime_data_hashes_are_reverified_instead_of_trusting_seal_record(self):
        path=self.inputs/'tools/toolchain/bin/rustc';path.chmod(0o755);path.write_bytes(b'changed');path.chmod(0o555)
        with self.assertRaises(ValueError):self.compose()
        self.assertFalse(self.output.exists())

    def test_os_pins_cannot_drift_from_accepted_old_configuration(self):
        self.old['protected_files']['/usr/bin/git']='f'*64;self.config_path.write_bytes(BOOT.canonical(self.old))
        with self.assertRaisesRegex(ValueError,'OS tool'):self.compose()

    def test_staging_freeze_or_candidate_bundle_drift_blocks(self):
        path=self.source_root/'candidate.bundle';path.chmod(0o600);path.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError,'staged source'):self.compose()

    def test_imported_helper_release_cannot_differ(self):
        self.imported['helper_manifest_sha256']='f'*64
        with self.assertRaisesRegex(ValueError,'another helper'):self.compose()

    def test_preflight_volume_identity_and_stopped_worker_boundaries_fail_before_output(self):
        for api,name in [(self.preflight,'observe'),(self.storage,'admit'),(self.builder,'pilot_slot'),
                         (self.builder,'worker_identity'),(self.builder,'stopped'),(self.builder,'verify_snapshot')]:
            function=getattr(api,name);function.side_effect=ValueError('boundary blocked')
            with self.assertRaises(ValueError):self.compose()
            self.assertFalse(self.output.exists());function.side_effect=None

    def test_source_release_binding_is_required_before_composition(self):
        self.contract.bind_source.side_effect=ValueError('release Git mismatch')
        with self.assertRaises(ValueError):self.compose()
        self.assertFalse(self.output.exists())

    def test_postflight_pilot_drift_retains_blocked_candidate_without_changing_old_config(self):
        old=self.config_path.read_bytes();self.preflight.observe.side_effect=[{'logical_snapshot':'old'},{'logical_snapshot':'changed'}]
        with self.assertRaisesRegex(ValueError,'pilot changed'):self.compose()
        self.assertEqual(json.loads((self.output/'compose.json').read_bytes())['state'],'blocked')
        self.assertFalse((self.output/'binding.json').exists());self.assertEqual(self.config_path.read_bytes(),old)

    def test_postcheck_input_mutation_keeps_outputs_private_and_blocked(self):
        self.builder.imported_inputs.side_effect=[copy.deepcopy(self.imported),ValueError('input changed')]
        with self.assertRaises(ValueError):self.compose()
        self.assertEqual(json.loads((self.output/'compose.json').read_bytes())['state'],'blocked')

    def test_candidate_output_mutation_cannot_complete(self):
        original=self.B.read
        def read(path,limit):
            raw=original(path,limit)
            return b'changed' if Path(path)==self.output/'host.json' else raw
        self.B.read=read
        with self.assertRaises(ValueError):self.compose()
        self.assertEqual(json.loads((self.output/'compose.json').read_bytes())['state'],'blocked')

    def test_pre_and_post_proofs_are_repeated(self):
        self.compose()
        self.assertEqual(self.preflight.observe.call_count,2);self.assertEqual(self.storage.admit.call_count,2)
        self.assertEqual(self.contract.bind_source.call_count,2);self.assertEqual(self.builder.imported_inputs.call_count,2)
        self.assertEqual(self.builder.stopped.call_count,2);self.assertEqual(self.B.authenticate.call_count,2)

    def test_exact_inherited_lease_stays_held_and_wrong_descriptor_blocks(self):
        with (self.control/'controller.lock').open('r+b') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):self.compose()
            self.compose(lock_fd=lock.fileno());os.fstat(lock.fileno())
            with (self.control/'controller.lock').open('r+b') as rival:
                with self.assertRaises(BlockingIOError):fcntl.flock(rival.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
        with self.config_path.open('rb') as other:
            with self.assertRaisesRegex(ValueError,'lease'):self.compose(lock_fd=other.fileno())

    def test_real_nonroot_cli_rejects_before_reading_credentials_or_creating_output(self):
        if os.geteuid()==0:self.skipTest('requires actual ordinary user')
        argv=[sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),'compose-linux-host']
        for name in ['helper-root','manifest','release','host-recipe','tool-recipe']:argv+=['--'+name,str(self.root/'missing')]
        for name in ['manifest-sha256','release-sha256','input-binding-sha256','linux-binding-sha256','core-record-sha256',
                     'host-recipe-sha256','tool-recipe-sha256']:argv+=['--'+name,'a'*64]
        result=subprocess.run(argv,capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'host configuration blocked',result.stderr)
        self.assertFalse(self.output.exists())


if __name__=='__main__':unittest.main()
