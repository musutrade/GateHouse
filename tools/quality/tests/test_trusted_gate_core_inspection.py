"""Privileged inspection cannot reuse a stale configuration or execute verification.

Root/provenance/kernel boundaries are replaced explicitly. Git copy/inventory,
composition re-verification, JSON replies and private status files are exercised.
"""
import fcntl
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
    spec=importlib.util.spec_from_file_location(name,path);module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module);return module


FIX=load('core_inspection_config_fixture',ROOT/'tools/quality/tests/test_trusted_gate_host_config.py')
PROTO=load('core_inspection_protocol_fixture',ROOT/'tools/quality/tests/test_trusted_gate_core_policy.py')
C=PROTO.C;B0=PROTO.B
STAGE=load('core_inspection_stage',ROOT/'tools/trusted_gate/deploy/stage-linux-inputs.py')
GIT=load('core_inspection_git',ROOT/'tools/trusted_gate/deploy/stage-protected-source.py')


class CoreInspectionTests(unittest.TestCase):
    def setUp(self):
        self.f=FIX.HostConfigurationTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        f=self.f;self.configuration=f.compose();self.pin=C.hashlib.sha256((f.output/'binding.json').read_bytes()).hexdigest()
        self.B=SimpleNamespace(**vars(f.B))
        self.composer=SimpleNamespace(compose=Mock(side_effect=lambda *args,**kw:f.compose(
            existing_pin=kw['existing_pin'],lock_fd=kw['lock_fd'])))
        self.B.load=lambda helpers,name:{'compose-linux-host.py':self.composer,'build-protected-core.py':f.builder,
            'stage-linux-inputs.py':STAGE,'bootstrap-release.py':f.contract,'stage-protected-source.py':GIT}[name]
        self.destination=f.volume/'bootstrap-core-inspection';self.view={'fixture':'private source inventory'}
        self.responses={'configuration':B0.canonical({'schema_version':1,'valid':True,'truncated':False,'diagnostics':[]}),
            **{profile:B0.canonical(PROTO.envelope(profile)) for profile in ['full','hook']}}
        def run(B,builder,dest,label,argv,env):
            raw=self.responses[label];B.write(dest/(label+'.stdout'),raw,0o600);B.write(dest/(label+'.stderr'),b'',0o600)
            B.record(dest/(label+'.unit.json'),{'state':'stopped','worker_stop_proven':True,'exit_code':0})
            return raw
        self.run=Mock(side_effect=run)
        self.copy=Mock(return_value=self.view);self.inventory=Mock(side_effect=lambda *args:dict(self.view))

    def inspect(self,**options):
        f=self.f
        with patch.multiple(C,VOLUME=f.volume,CONTROL=f.control),patch.object(C.os,'geteuid',return_value=0), \
             patch.object(C,'copy_view',self.copy),patch.object(C,'view_inventory',self.inventory), \
             patch.object(C,'run_command',self.run):
            return C.inspect(self.B,f.root/'helpers',f.manifest,'a'*64,f.archive,C.hashlib.sha256(f.archive.read_bytes()).hexdigest(),
                'b'*64,f.seal_pin,f.core_pin,self.pin,f.root/'host-recipe','c'*64,f.root/'volume-recipe','d'*64,**options)

    def blocked(self):
        value=json.loads((self.destination/'inspection.json').read_bytes())
        self.assertEqual(value['state'],'blocked');self.assertFalse(value['captured_core_policy_checked'])
        self.assertFalse((self.destination/'binding.json').exists())

    def test_completed_configuration_is_reverified_and_only_three_passive_commands_are_checked(self):
        before={p:p.read_bytes() for p in self.f.output.iterdir()};result=self.inspect()
        self.assertEqual(result['state'],'checked');self.assertTrue(result['captured_core_policy_checked'])
        self.assertFalse(result['administrator_ready']);self.assertFalse(result['production_gate_enabled'])
        self.assertEqual(result['authoritative_publish'],'blocked');self.assertEqual(self.composer.compose.call_count,2)
        self.assertEqual(self.run.call_count,3);self.assertEqual(self.inventory.call_count,3)
        self.assertEqual(result['replies']['full']['selected_nodes'],C.IDS)
        for call in self.run.call_args_list:
            label,argv=call.args[3:5]
            if label!='configuration':self.assertIn('--plan',argv)
        self.assertEqual({p:p.read_bytes() for p in self.f.output.iterdir()},before)
        self.assertEqual(C.hashlib.sha256((self.destination/'binding.json').read_bytes()).hexdigest(),
            json.loads((self.destination/'inspection.json').read_bytes())['binding_sha256'])

    def test_wrong_configuration_pin_or_failed_provenance_blocks_before_new_output(self):
        self.pin='f'*64
        with self.assertRaises(ValueError):self.inspect()
        self.assertFalse(self.destination.exists());self.copy.assert_not_called();self.run.assert_not_called()

    def test_prior_inspection_directory_is_retained_and_never_reused(self):
        self.inspect()
        with self.assertRaisesRegex(ValueError,'never reuse'):self.inspect()
        self.assertEqual(json.loads((self.destination/'inspection.json').read_bytes())['state'],'checked')

    def test_failed_config_report_or_profile_plan_retains_blocked_output(self):
        self.responses['configuration']=B0.canonical({'schema_version':1,'valid':False,'truncated':False,'diagnostics':[]})
        with self.assertRaises(ValueError):self.inspect()
        self.blocked();self.assertEqual(self.run.call_count,1)

    def test_dropped_hook_requirement_cannot_complete_inspection(self):
        plan=PROTO.envelope('hook');plan['plan']['nodes'].pop();self.responses['hook']=B0.canonical(plan)
        with self.assertRaises(ValueError):self.inspect()
        self.blocked();self.assertEqual(self.run.call_count,3)

    def test_copy_mutation_between_commands_blocks_before_later_commands(self):
        self.inventory.side_effect=[{'fixture':'changed'}]
        with self.assertRaisesRegex(ValueError,'view changed'):self.inspect()
        self.blocked();self.assertEqual(self.run.call_count,1)

    def test_postcheck_configuration_or_runtime_drift_blocks_completion(self):
        self.composer.compose.side_effect=[self.configuration,ValueError('postcheck input drift')]
        with self.assertRaises(ValueError):self.inspect()
        self.blocked()

    def test_parser_failure_keeps_private_evidence_and_old_config_unchanged(self):
        before=self.f.config_path.read_bytes();self.run.side_effect=subprocess.TimeoutExpired(['fixed-passive-command'],50)
        with self.assertRaises(subprocess.TimeoutExpired):self.inspect()
        self.blocked();self.assertEqual(self.f.config_path.read_bytes(),before)

    def test_response_changed_after_validation_cannot_be_bound_as_checked(self):
        original=self.composer.compose.side_effect
        def review(*args,**kwargs):
            result=original(*args,**kwargs)
            if self.run.call_count==3:(self.destination/'configuration.stdout').write_bytes(b'changed')
            return result
        self.composer.compose.side_effect=review
        with self.assertRaisesRegex(ValueError,'response changed'):self.inspect()
        self.blocked()

    def test_inherited_lease_is_exact_and_never_released_by_inner_inspections(self):
        with (self.f.control/'controller.lock').open('r+b') as lease:
            fcntl.flock(lease,fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):self.inspect()
            self.inspect(lock_fd=lease.fileno());os.fstat(lease.fileno())
            with (self.f.control/'controller.lock').open('r+b') as rival:
                with self.assertRaises(BlockingIOError):fcntl.flock(rival,fcntl.LOCK_EX|fcntl.LOCK_NB)
        with self.f.config_path.open('rb') as wrong:
            with self.assertRaisesRegex(ValueError,'lease'):self.inspect(lock_fd=wrong.fileno())

    def test_transient_parser_unit_is_readonly_networkless_bounded_and_stopped(self):
        self.destination.mkdir()
        self.f.builder.stopped.reset_mock()
        def run(argv,**kwargs):
            if argv[0]=='/usr/bin/systemd-run':kwargs['stdout'].write(self.responses['configuration'])
            return SimpleNamespace(returncode=0)
        with patch.object(C,'VOLUME',self.f.volume),patch.object(C.subprocess,'run',side_effect=run) as invoked:
            actual=C.run_command(self.B,self.f.builder,self.destination,'configuration',
                C.commands(self.B,'/work/inspection-core','/work/source','/work/source/.gatehouse-host/flow.toml')[0][1],'/usr/bin/env')
        self.assertEqual(actual,self.responses['configuration']);argv=invoked.call_args_list[0].args[0]
        for property in ['User=root','ProtectSystem=strict','PrivateNetwork=yes','CapabilityBoundingSet=',
                         'NoNewPrivileges=yes','RuntimeMaxSec=30','MemoryMax='+str(1024**3)]:
            self.assertIn('--property='+property,argv)
        self.assertIn('-i',argv);self.assertIn('GIT_CONFIG_GLOBAL=/dev/null',argv)
        self.assertTrue(next(arg for arg in argv if arg.startswith('--property=BindReadOnlyPaths=')).endswith(':/work/inspection-core'))
        self.assertEqual(invoked.call_args_list[-1].args[0][:2],['/usr/bin/systemctl','stop'])
        self.f.builder.stopped.assert_called()

    def test_transient_parser_is_stopped_even_on_timeout_or_bad_exit(self):
        self.destination.mkdir()
        self.f.builder.stopped.reset_mock()
        def run(argv,**kwargs):
            if argv[0]=='/usr/bin/systemd-run':raise subprocess.TimeoutExpired(argv,50)
            return SimpleNamespace(returncode=0)
        with patch.object(C.subprocess,'run',side_effect=run) as invoked,self.assertRaises(subprocess.TimeoutExpired):
            C.run_command(self.B,self.f.builder,self.destination,'configuration',
                C.commands(self.B,'/work/inspection-core','/work/source','/work/source/.gatehouse-host/flow.toml')[0][1],'/usr/bin/env')
        self.assertEqual(invoked.call_args_list[-1].args[0][:2],['/usr/bin/systemctl','stop'])
        self.f.builder.stopped.assert_called()

    def test_unknown_cgroup_stop_never_sets_worker_stop_proven(self):
        self.destination.mkdir();self.f.builder.stopped.reset_mock();self.f.builder.stopped.side_effect=ValueError('unknown worker')
        def run(argv,**kwargs):return SimpleNamespace(returncode=0)
        with patch.object(C.subprocess,'run',side_effect=run),self.assertRaises(ValueError):
            C.run_command(self.B,self.f.builder,self.destination,'configuration',
                C.commands(self.B,'/work/inspection-core','/work/source','/work/source/.gatehouse-host/flow.toml')[0][1],'/usr/bin/env')
        record=json.loads((self.destination/'configuration.unit.json').read_bytes())
        self.assertFalse(record['worker_stop_proven']);self.assertEqual(record['state'],'starting')

    def test_nonpassive_or_unrecognized_command_cannot_start_a_unit(self):
        self.destination.mkdir()
        with patch.object(C.subprocess,'run') as run,self.assertRaises(ValueError):
            C.run_command(self.B,self.f.builder,self.destination,'configuration',['/work/inspection-core','verify'],'/usr/bin/env')
        run.assert_not_called();self.assertFalse(any(self.destination.iterdir()))

    def test_real_git_copy_has_independent_store_and_leaves_original_source_without_overlay(self):
        f=self.f;self.destination.mkdir()
        def git(where,*args):return subprocess.check_output(['/usr/bin/git','-C',str(where),*args],env=B0.ENV,stderr=subprocess.PIPE)
        original=git(f.source,'status','--porcelain');mirror=f.source_root/'mirror.git'
        git(f.root,'clone','--mirror','--no-hardlinks',str(f.source),str(mirror))
        bundle=f.root/'inspection.bundle';git(f.source,'bundle','create',str(bundle),'HEAD')
        payload={**f.payload,'source.bundle':bundle.read_bytes()}
        real_owned=B0.owned
        def owned(path,**kwargs):
            p=Path(path)
            return f.B.owned(p,**kwargs) if p==f.root or f.root in p.parents else real_owned(p,**kwargs)
        context=SimpleNamespace(**vars(self.B));context.owned=owned;context.frozen_lock=B0.frozen_lock;context.readonly=B0.readonly
        with patch.object(B0,'owned',side_effect=owned),patch.multiple(C,VOLUME=f.volume,BASE=f.head):
            view=C.copy_view(context,f.root/'helpers',payload,f.head,self.destination)
            self.assertEqual(C.view_inventory(context,STAGE,self.destination,f.head),view)
        self.assertFalse((f.source/'.gatehouse-host').exists());self.assertEqual(git(f.source,'status','--porcelain'),original)
        self.assertEqual(git(self.destination/'source','rev-parse','HEAD').decode().strip(),f.head)
        for parent,dirs,names in os.walk(self.destination/'mirror.git'):
            for name in names:self.assertEqual((Path(parent)/name).stat().st_nlink,1)

    def test_real_nonroot_cli_stops_before_credentials_and_output(self):
        if os.geteuid()==0:self.skipTest('requires actual ordinary user')
        argv=[sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),'inspect-core-policy']
        for name in ['helper-root','manifest','release','host-recipe','tool-recipe']:argv+=['--'+name,str(self.f.root/'missing')]
        for name in ['manifest-sha256','release-sha256','input-binding-sha256','linux-binding-sha256','core-record-sha256',
            'configuration-binding-sha256','host-recipe-sha256','tool-recipe-sha256']:argv+=['--'+name,'a'*64]
        result=subprocess.run(argv,capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'Core policy inspection blocked',result.stderr)
        self.assertFalse(self.destination.exists())


if __name__=='__main__':unittest.main()
