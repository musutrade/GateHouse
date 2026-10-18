"""A Linux template must not erase old file identity or claim completed migration."""
import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT/'tools/trusted_gate/deploy'/file)
    module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


BOOT=load('linux_policy_entry_tests','import-protected-inputs.py')
R=load('linux_policy_release_tests','bootstrap-release.py')
POLICY=R.LINUX


class LinuxPolicyTests(unittest.TestCase):
    def setUp(self):
        self.flow=(ROOT/POLICY.FLOW).read_bytes()
        self.raw=(ROOT/POLICY.INVENTORY).read_bytes()
        self.B=SimpleNamespace(**{name:getattr(BOOT,name) for name in ('need','canonical','unique')})

    def test_committed_policy_is_partial_and_both_profiles_keep_all_required_steps(self):
        result=POLICY.validate(self.B,self.flow,self.raw)
        self.assertFalse(result['administrator_ready']);self.assertFalse(result['host_configuration_complete'])
        self.assertEqual(set(result['coverage']),{'builtin.secret-scan','builtin.architecture-audit',
                         'pilot-boundary','pilot-host-contracts','linux-test'})
        value=json.loads(self.raw)
        self.assertIn('native-macOS-and-Windows',value['pending_acceptance'])

    def test_dropped_requirement_profile_or_launcher_is_rejected_even_after_inventory_rehash(self):
        for old,new in [(b', "linux-test"]',b']'),(b'"full", "hook"',b'"full"'),
                        (b'program = "gatehouse-step"',b'program = "bash"'),
                        (b'parallel = false',b'parallel = true')]:
            changed=self.flow.replace(old,new)
            self.assertNotEqual(changed,self.flow)
            raw=BOOT.canonical(POLICY.inventory(changed))
            with self.assertRaises(ValueError):POLICY.validate(self.B,changed,raw)

    def test_moved_reports_builtin_override_or_fast_scope_cannot_be_added(self):
        for changed in [self.flow.replace(b'.gatehouse-host/reports',b'/tmp/reports'),
                        self.flow.replace(b'unmatched = "all"',b'unmatched = "none"'),
                        self.flow+b'\n[builtin]\nsecret_scan = false\n']:
            with self.assertRaises(ValueError):POLICY.validate(self.B,changed,BOOT.canonical(POLICY.inventory(changed)))

    def test_coverage_publication_and_acceptance_flags_cannot_be_promoted(self):
        for field,new in [('coverage_acceptance','complete'),('policy_scope','full-project'),
            ('coverage',[]),('pending_acceptance',[]),('authoritative_publish','success'),
            ('host_configuration_complete',True),('captured_core_policy_checked',True),
            ('administrator_ready',True),('production_gate_enabled',True),('production_gate_enabled',0)]:
            value=json.loads(self.raw);value[field]=new
            with self.assertRaises(ValueError):POLICY.validate(self.B,self.flow,BOOT.canonical(value))

    def test_worker_argv_history_run_tokens_and_readonly_inputs_cannot_change(self):
        for changed in ('argv','git_history','run_arguments','capture_files','required_readonly_mounts','host_configuration_required'):
            value=json.loads(self.raw)
            if changed in value['worker_template']:value['worker_template'][changed]=[] if changed in ('argv','capture_files') else False
            else:value[changed]={}
            with self.assertRaises(ValueError):POLICY.validate(self.B,self.flow,BOOT.canonical(value))

    def test_receipt_tokens_stay_in_argv_and_cache_inputs_cover_changed_configuration(self):
        worker=POLICY.worker()
        tokens={arg for arg in worker['argv'] if arg.startswith('@gatehouse:')}
        self.assertEqual(tokens,{'@gatehouse:head@','@gatehouse:base@','@gatehouse:receipt@','@gatehouse:test-data-sha256@'})
        self.assertTrue(worker['run_arguments']);self.assertTrue(worker['git_history'])
        self.assertNotIn('GITHUB_TOKEN',worker['env']);self.assertNotIn('PATH',worker['env'])
        self.assertNotIn('toolchain_files',worker)
        self.assertIn('toolchain_files',json.loads(self.raw)['host_configuration_required'])
        self.assertIn('test-data/manifest.json',worker['cache_inputs']['configuration'])

    def test_capture_inventory_is_finite_and_never_enumerates_onboarding_secrets(self):
        names=POLICY.worker()['capture_files']
        self.assertEqual(len(names),85);self.assertEqual(len(names),len(set(names)))
        self.assertEqual({name for name in names if '/onboarding/' in name},
            {'linux-test/onboarding/report.json'} | {'linux-test/onboarding/'+str(index).zfill(2)+'.'+stream+'.log'
                for index in range(1,30) for stream in ('stdout','stderr')})
        self.assertFalse(any('*' in name or '..' in Path(name).parts or Path(name).is_absolute() for name in names))
        self.assertEqual({name for name in names if name.endswith('.stdout')},
                         {'linux-test/'+name+'.stdout' for name in POLICY.COMMANDS})

    def test_policy_flow_digest_duplicate_json_or_noncanonical_metadata_block(self):
        for raw in [self.raw.replace(json.loads(self.raw)['flow_sha256'].encode(),b'0'*64),
                    b'{"schema":"duplicate",'+self.raw[1:],json.dumps(json.loads(self.raw),indent=2).encode()]:
            with self.assertRaises(ValueError):POLICY.validate(self.B,self.flow,raw)

    def test_new_flow_template_is_bound_to_the_existing_destination_before_digest(self):
        payload={'before/'+name:('old:'+name).encode() for name in R.old_sources()}
        payload.update({'files/'+name:('new:'+name).encode() for name in R.source_map()})
        row=R.plan(payload)[POLICY.FLOW]
        self.assertEqual(row['destination'],'/etc/gatehouse/policy/flow.toml')
        old=payload['before/'+R.DEPLOY+'pilot.flow.toml']
        self.assertEqual(row['before_sha256'],hashlib.sha256(old).hexdigest())
        self.assertIsNone(R.plan(payload)[POLICY.INVENTORY]['before_sha256'])

    def test_duplicate_old_or_new_destinations_are_rejected(self):
        old=R.old_source_map();current=R.source_map()
        payload={'before/'+name:b'old' for name in old}
        payload.update({'files/'+name:b'new' for name in current})
        for attribute,mapping in [('old_source_map',old),('source_map',current)]:
            mapping={**mapping,'injected-alias':next(iter(mapping.values()))}
            payload['before/injected-alias']=b'old';payload['files/injected-alias']=b'new'
            with patch.object(R,attribute,return_value=mapping):
                with self.assertRaisesRegex(ValueError,'ambiguous'):R.plan(payload)

    def test_fixed_old_inventory_keeps_pilot_policy_and_new_runtime_files_are_additions(self):
        self.assertEqual(len(R.old_sources()),23);self.assertEqual(len(R.old_source_map()),21)
        self.assertIn(R.DEPLOY+'pilot.flow.toml',R.old_sources())
        self.assertNotIn(POLICY.FLOW,R.old_sources());self.assertNotIn(POLICY.INVENTORY,R.old_sources())
        old_destinations={row[0] for row in R.old_source_map().values()}
        new_destinations={row[0] for row in R.source_map().values()}
        self.assertTrue(old_destinations<=new_destinations)
        self.assertEqual(len(R.source_map()),26)
        self.assertIn('/etc/systemd/system/srv-gatehouse-linux.mount',new_destinations)
        self.assertNotIn('/etc/systemd/system/srv-gatehouse-linux.mount',old_destinations)
        self.assertEqual(R.source_map()[R.DEPLOY+'srv-gatehouse-pilot.mount'],
                         R.old_source_map()[R.DEPLOY+'srv-gatehouse-pilot.mount'])
        self.assertIn('/opt/gatehouse/linux-test/run-linux-test.py',new_destinations)
        self.assertIn('/opt/gatehouse/linux-test/prepare-linux-workspace.py',new_destinations)

    def test_rehashed_release_cannot_remove_the_linux_requirement(self):
        code={'schema':'gatehouse-bootstrap-code/v1','revision':'a'*40,'files':{}}
        manifest=BOOT.canonical(code)
        payload={'before/'+name:('old:'+name).encode() for name in R.old_sources()}
        payload.update({'files/'+name:('new:'+name).encode() for name in R.source_map()})
        for name,row in R.source_map().items():
            if row[2]=='retain':payload['files/'+name]=payload['before/'+name]
        payload.update({'metadata.json':BOOT.canonical(R.metadata('a'*40)),
                        'helper-manifest.json':manifest,'source.bundle':b'fixture',
                        'files/'+POLICY.FLOW:self.flow,'files/'+POLICY.INVENTORY:self.raw})
        before={name:payload['before/'+name] for name in R.old_sources()}
        def seal():
            payload['payload-manifest.json']=BOOT.canonical({'schema':'gatehouse-bootstrap-payload/v1',
                'files':R.rows({name:raw for name,raw in payload.items() if name!='payload-manifest.json'}),
                'installed_plan':R.plan(payload),'source_binding_required':True})
        with patch.object(R,'OLD_INPUTS',hashlib.sha256(BOOT.canonical(R.rows(before))).hexdigest()):
            seal();R.validate(self.B,payload,code,manifest)
            changed=self.flow.replace(b', "linux-test"]',b']')
            payload['files/'+POLICY.FLOW]=changed
            payload['files/'+POLICY.INVENTORY]=BOOT.canonical(POLICY.inventory(changed))
            seal()
            with self.assertRaisesRegex(ValueError,'flow differs'):R.validate(self.B,payload,code,manifest)


if __name__=='__main__':unittest.main()
