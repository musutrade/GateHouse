"""Passive Core replies cannot stand in for runtime validation or quality results."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[3]


def load(name,file):
    spec=importlib.util.spec_from_file_location(name,ROOT/'tools/trusted_gate/deploy'/file)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


B=load('core_policy_protocol_entry','import-protected-inputs.py')
C=load('core_policy_protocol','check-core-policy.py')
FLOW=(ROOT/'tools/trusted_gate/deploy/linux-transition.flow.toml').read_bytes()


def explanation(code):
    return [{'code':code,'message':'Explicit protocol fixture explanation.'}]


def envelope(profile='full'):
    nodes=[]
    for i,identity in enumerate(C.IDS):
        nodes.append({'id':identity,'label':identity,'kind':'builtin-gate' if i<2 else 'external-step',
            'component':None if i<2 else ('linux-test' if i==4 else 'pilot'),
            'profiles':None if i<2 else ['full','hook'],'input':None if i<2 else 'snapshot',
            'depends_on':[] if i==0 else [C.IDS[0] if i==1 else C.IDS[1]],
            'required_by':[C.IDS[1]] if i==0 else (['linux-test','pilot-boundary','pilot-host-contracts'] if i==1 else []),
            'selected':True,'services':None if i<2 else [],
            'reasons':explanation('MANDATORY_BUILTIN_GATE' if i<2 else 'COMPONENT_AND_PROFILE')})
    return {'schema':'verification-plan/v1','status':'planned','verification_result':None,'diagnostics':[],
        'plan':{'components':[{'id':identity,'selected':True,'depends_on_components':[],'impact_sources':[],
            'impact_chain':[],'rules':[0],'reasons':explanation('ALL_COMPONENTS')} for identity in ['linux-test','pilot']],
        'configuration_digest':C.CONFIGURATION_DIGEST,'execution_order':list(C.IDS),'nodes':nodes,
        'notice':'Plan only; no runtime inputs validated or verification result produced.','paths':[],'profile':profile,
        'profiles':[{'id':identity,'selected':identity==profile,'reasons':explanation(
            'PROFILE_REQUESTED' if identity==profile else 'PROFILE_NOT_SELECTED')} for identity in ['full','hook']],
        'project':'gatehouse-linux-test-transition','quality':{'enabled':False,'reasons':[
            {'code':'QUALITY_NOT_CONFIGURED','message':'No quality.toml is configured.'}]},
        'rules':[{'index':0,'patterns':['**'],'components':['linux-test','pilot'],'paths':[],
                  'selected':False,'reasons':explanation('RULE_NOT_MATCHED')}],
        'runtime_inputs':'not_validated','scope':{'changed_files':[],'components':['linux-test','pilot'],
            'mode':'all','unmatched_files':[]},'services':[]}}


class CorePolicyProtocolTests(unittest.TestCase):
    def check(self,value,profile='full'):
        return C.check_plan(B,B.canonical(value),profile,FLOW)

    def test_successful_configuration_report_requires_exact_typed_complete_diagnostics(self):
        good={'schema_version':1,'valid':True,'truncated':False,'diagnostics':[]}
        self.assertTrue(C.check_report(B,B.canonical(good))['valid'])
        for key,value in [('schema_version',True),('valid',1),('valid',False),('truncated',True),
                          ('diagnostics',[{'code':'warning'}]),('administrator_ready',True)]:
            item={**good,key:value}
            with self.assertRaises(ValueError):C.check_report(B,B.canonical(item))

    def test_both_profiles_preserve_exact_coverage_and_never_return_verification(self):
        for profile in ['full','hook']:
            value=self.check(envelope(profile),profile)
            self.assertEqual(value['selected_nodes'],C.IDS)
            self.assertEqual(value['runtime_inputs'],'not_validated');self.assertIsNone(value['verification_result'])
        with self.assertRaises(ValueError):self.check(envelope(),'ci')

    def test_invalid_diagnostics_completed_or_quality_result_envelopes_rejected(self):
        for key,value in [('status','completed'),('verification_result',{'passed':True}),
                          ('diagnostics',[{'code':'PARTIAL'}]),('schema','verification-result/v1')]:
            data=envelope();data[key]=value
            with self.assertRaises(ValueError):self.check(data)

    def test_digest_profile_project_or_claimed_runtime_validation_drift_rejected(self):
        for key,value in [('configuration_digest','sha256:'+'0'*64),('profile','hook'),
                          ('project','other-project'),('runtime_inputs','validated')]:
            data=envelope();data['plan'][key]=value
            with self.assertRaises(ValueError):self.check(data)

    def test_raw_policy_and_effective_core_configuration_are_separate_fixed_pins(self):
        self.assertNotEqual('sha256:'+C.hashlib.sha256(FLOW).hexdigest(),C.CONFIGURATION_DIGEST)
        with self.assertRaises(ValueError):C.check_plan(B,B.canonical(envelope()),'full',FLOW+b'\n')

    def test_missing_duplicate_reordered_or_extra_required_nodes_rejected(self):
        for changed in [C.IDS[:-1],C.IDS+['extra'],C.IDS[:-1]+[C.IDS[0]],list(reversed(C.IDS))]:
            data=envelope();data['plan']['execution_order']=changed
            with self.assertRaises(ValueError):self.check(data)
        data=envelope();data['plan']['nodes'].pop()
        with self.assertRaises(ValueError):self.check(data)

    def test_unselected_or_waived_node_cannot_be_explained_as_selected(self):
        for changed in [False,1]:
            data=envelope();data['plan']['nodes'][4]['selected']=changed
            with self.assertRaises(ValueError):self.check(data)
        data=envelope();data['plan']['nodes'][4]['reasons']=explanation('WAIVER_APPLIED')
        with self.assertRaises(ValueError):self.check(data)

    def test_builtin_dependency_chain_and_required_by_must_remain_intact(self):
        for index,key,value in [(1,'depends_on',[]),(4,'depends_on',[]),(0,'required_by',[]),(1,'required_by',[])]:
            data=envelope();data['plan']['nodes'][index][key]=value
            with self.assertRaises(ValueError):self.check(data)

    def test_node_profiles_services_kind_and_execution_inputs_cannot_change(self):
        for key,value in [('profiles',['full']),('services',['service']),('input','working-tree'),
                          ('kind','builtin-gate'),('component','pilot')]:
            data=envelope();data['plan']['nodes'][4][key]=value
            with self.assertRaises(ValueError):self.check(data)

    def test_scope_or_quality_promotion_and_service_selection_rejected(self):
        for key,value in [('services',['daemon']),('paths',['src/main.rs']),
                          ('scope',{'mode':'staged'}),('quality',{'enabled':True})]:
            data=envelope();data['plan'][key]=value
            with self.assertRaises(ValueError):self.check(data)

    def test_profiles_components_and_all_rule_use_strict_json_boolean_integer_types(self):
        data=envelope();data['plan']['profiles'][0]['selected']=1
        with self.assertRaises(ValueError):self.check(data)
        data=envelope();data['plan']['components'][0]['rules']=[False]
        with self.assertRaises(ValueError):self.check(data)
        data=envelope();data['plan']['rules'][0]['index']=False
        with self.assertRaises(ValueError):self.check(data)
        data=envelope();data['plan']['components'][0]['selected']=False
        with self.assertRaises(ValueError):self.check(data)

    def test_unknown_api_fields_cannot_add_unreviewed_authority(self):
        for where in ['envelope','plan','node']:
            data=envelope();target=data if where=='envelope' else (data['plan'] if where=='plan' else data['plan']['nodes'][0])
            target['waived']=True
            with self.assertRaises(ValueError):self.check(data)

    def test_duplicate_nonfinite_invalid_or_oversized_json_rejected(self):
        for raw in [b'{"valid":true,"valid":false}',b'{"value":NaN}',b'\xff',b'',b'not-json']:
            with self.assertRaises((ValueError,UnicodeError)):C.check_report(B,raw)
        with patch.object(C,'MAX_REPLY',8),self.assertRaises(ValueError):C.decode(B,b'{"long":"response"}')

    def test_command_inventory_contains_only_configuration_and_two_passive_plans(self):
        commands=C.commands(B,'/trusted/captured/harness-gate','/private/inspection/source',
            '/private/inspection/source/.gatehouse-host/flow.toml')
        self.assertEqual([name for name,argv in commands],['configuration','full','hook'])
        self.assertEqual(commands[0][1][-4:],['config','check','--format','json'])
        for profile,argv in commands[1:]:self.assertEqual(argv[-6:],['verify','--all','--profile',profile,'--plan','--json'])
        self.assertFalse(any('step' in argv or 'publish' in argv or '--apply' in argv for name,argv in commands))

    def test_commands_reject_outside_flow_unsafe_operands_and_binary_inside_view(self):
        for core,root,flow in [('/private/source/core','/private/source','/private/source/.gatehouse-host/flow.toml'),
            ('/trusted/core','/private/source','/outside/flow.toml'),('/trusted/core','/private/../source','/private/source/.gatehouse-host/flow.toml')]:
            with self.assertRaises(ValueError):C.commands(B,core,root,flow)


if __name__=='__main__':unittest.main()
