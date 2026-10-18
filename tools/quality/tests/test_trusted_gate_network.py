"""Explicit controller routes, with no live credentials or remote side effects."""
from contextlib import redirect_stderr,redirect_stdout
import importlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch
import urllib.error
import urllib.request

ROOT=Path(__file__).resolve().parents[3]
SPEC=importlib.util.spec_from_file_location('network_trusted_gate',ROOT/'tools/trusted_gate/__init__.py',
                                          submodule_search_locations=[str(ROOT/'tools/trusted_gate')])
PACKAGE=importlib.util.module_from_spec(SPEC);sys.modules[SPEC.name]=PACKAGE;SPEC.loader.exec_module(PACKAGE)
C=importlib.import_module(SPEC.name+'.common');G=importlib.import_module(SPEC.name+'.github')
S=importlib.import_module(SPEC.name+'.source');CLI=importlib.import_module(SPEC.name+'.cli')
E=importlib.import_module(SPEC.name+'.execution');R=importlib.import_module(SPEC.name+'.runtime')
PROXY='http://192.168.0.2:7890'
SCOPE='http.https://github.com/owner/project.git.'
MISSING=object()


class ExplicitControllerNetworkTests(unittest.TestCase):
    def client(self,route=None,**options):
        with patch.object(G,'protected',side_effect=lambda path,**kw:Path(path)):
            return G.GitHub(123,456,'/private/fixture.pem','owner/project',https_proxy=route,**options)

    def test_null_and_canonical_credential_free_http_or_https_routes(self):
        for route in (None,PROXY,'https://proxy.example.invalid:443'):
            self.assertEqual(C.https_proxy(route),route)

    def test_credentials_paths_controls_unsupported_schemes_and_noncanonical_routes_block(self):
        values=['http://user:PRIVATE@proxy:80','http://user@proxy:80','http://proxy:80/',
                'http://proxy:80?q=PRIVATE','http://proxy:80#PRIVATE','socks5://proxy:80',
                'http://proxy','http://proxy:0','http://proxy:65536','http://PROXY:80','http://proxy:080',
                'http://[::1]:80','http://proxy:80\n',True,{},1,'x'*257]
        for route in values:
            with self.subTest(route=type(route).__name__),self.assertRaises(C.Blocked) as result:C.https_proxy(route)
            self.assertNotIn('PRIVATE',str(result.exception))

    def test_invalid_route_precedes_key_admission_and_client_construction(self):
        with patch.object(G,'protected',side_effect=AssertionError('key read')):
            with self.assertRaises(C.Blocked):G.GitHub(123,456,'/private/key','owner/project',https_proxy='http://secret@proxy:80')

    def test_default_direct_client_ignores_all_inherited_proxy_settings(self):
        with patch.dict(os.environ,{'HTTPS_PROXY':'http://unreviewed:1','HTTP_PROXY':'http://unreviewed:2','ALL_PROXY':'http://unreviewed:3'}):
            client=self.client()
        self.assertFalse(any(getattr(handler,'proxies',{}) for handler in client.opener.handlers))
        self.assertTrue(any(isinstance(handler,G.NoRedirect) for handler in client.opener.handlers))

    def test_explicit_client_only_adds_https_route_and_preserves_permissions(self):
        permissions={'contents':'write','pull_requests':'write'};client=self.client(PROXY,permissions=permissions)
        handlers=[handler for handler in client.opener.handlers if isinstance(handler,G.ExplicitHTTPSProxy)]
        self.assertEqual([handler.proxies for handler in handlers],[{'https':PROXY}])
        self.assertEqual(client.permissions,permissions)
        self.assertTrue(any(isinstance(handler,urllib.request.HTTPSHandler) for handler in client.opener.handlers))

    def test_explicit_proxy_cannot_be_bypassed_through_no_proxy_environment(self):
        handler=G.ExplicitHTTPSProxy({'https':PROXY});request=urllib.request.Request('https://api.github.com/repos/owner/project')
        with patch.dict(os.environ,{'NO_PROXY':'*','no_proxy':'api.github.com'}), \
             patch.object(urllib.request,'proxy_bypass',side_effect=AssertionError('environment bypass')):
            self.assertIsNone(handler.proxy_open(request,PROXY,'https'))
        self.assertEqual(request.host,'192.168.0.2:7890');self.assertEqual(request._tunnel_host,'api.github.com')
        self.assertIsNone(request.get_header('Proxy-authorization'))

    def test_explicit_handler_and_redirect_guard_reject_foreign_destinations(self):
        handler=G.ExplicitHTTPSProxy({'https':PROXY})
        with self.assertRaises(C.Blocked):handler.proxy_open(urllib.request.Request('https://foreign.invalid/'),PROXY,'https')
        with self.assertRaises(C.Blocked):G.NoRedirect().redirect_request(None,None,302,'','', 'https://foreign.invalid/')

    def test_accepted_old_client_preflight_effects_route_also_ignores_no_proxy(self):
        spec=importlib.util.spec_from_file_location('network_old_transport',ROOT/'tools/trusted_gate/deploy/admit-upgrade-effects.py')
        effects=importlib.util.module_from_spec(spec);spec.loader.exec_module(effects)
        client=SimpleNamespace(opener=urllib.request.build_opener(G.NoRedirect(),urllib.request.ProxyHandler({})))
        effects.transport(SimpleNamespace(need=C.require,canonical=C.canonical),client,PROXY)
        handler=next(h for h in client.opener.handlers if getattr(h,'proxies',{})=={'https':PROXY})
        request=urllib.request.Request('https://api.github.com/repos/owner/project')
        with patch.dict(os.environ,{'NO_PROXY':'*'}),patch.object(urllib.request,'proxy_bypass',side_effect=AssertionError('bypass')):
            handler.proxy_open(request,PROXY,'https')
        self.assertEqual(request.host,'192.168.0.2:7890');self.assertEqual(request._tunnel_host,'api.github.com')
        self.assertTrue(any(isinstance(h,G.NoRedirect) for h in client.opener.handlers))

    def test_api_failure_has_one_attempt_no_direct_retry_and_no_private_error_text(self):
        client=self.client(PROXY);client.opener.open=Mock(side_effect=urllib.error.URLError('PRIVATE_TOKEN_BODY'))
        with self.assertRaises(G.APIError) as result:client.request('GET','/repos/owner/project',token='PRIVATE_FIXTURE_TOKEN')
        self.assertEqual(client.opener.open.call_count,1);self.assertNotIn('PRIVATE',str(result.exception))
        request=client.opener.open.call_args.args[0];self.assertEqual(request.full_url,'https://api.github.com/repos/owner/project')
        self.assertEqual(client.opener.open.call_args.kwargs,{'timeout':30})

    def test_runtime_route_validation_precedes_filesystem_admission(self):
        with patch.object(R,'protected',side_effect=AssertionError('runtime file read')):
            with self.assertRaises(C.Blocked):R.settings({'https_proxy':'http://private@proxy:80','runtime':{}})

    def test_worker_environment_never_inherits_proxy_credentials_or_git_config(self):
        with patch.dict(os.environ,{'HTTPS_PROXY':PROXY,'NO_PROXY':'*','GIT_CONFIG_VALUE_0':'PRIVATE','GITHUB_TOKEN':'PRIVATE'}):
            clean=E.clean_environment()
        self.assertFalse(any(key in clean for key in ('HTTPS_PROXY','NO_PROXY','GIT_CONFIG_VALUE_0','GITHUB_TOKEN')))


class IndependentGitNetworkTests(unittest.TestCase):
    def fixture(self,route=PROXY):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup);root=Path(temp.name);mirror=root/'mirror.git'
        subprocess.run(['/usr/bin/git','init','--bare',str(mirror)],env=E.clean_environment(),capture_output=True,check=True)
        config={'source':str(root/'source'),'mirror':str(mirror),'repository':'owner/project','repository_id':10,
                'base_branch':'main','max_bundle_bytes':1024}
        if route is not MISSING:config['https_proxy']=route
        reader=SimpleNamespace(repo=Mock(side_effect=lambda method,path:{'object':{'sha':'b'*40}} if path else {'id':10}),
                               installation_token=Mock(return_value='PRIVATE_FIXTURE_TOKEN'))
        journal=SimpleNamespace(outstanding=Mock(return_value=[]))
        return root,mirror,config,reader,journal

    def capture(self,route=PROXY,error=None):
        root,mirror,config,reader,journal=self.fixture(route)
        with patch.object(S,'protected',side_effect=lambda p,**kw:Path(p)), \
             patch.object(S.subprocess,'run',side_effect=error) if error else patch.object(S.subprocess,'run',return_value=SimpleNamespace(returncode=1)) as run:
            with self.assertRaises(C.Blocked) as result:S.prepare(config,reader,journal,b'bundle','a'*40)
            call=run.call_args
        return root,mirror,call,result.exception

    def options(self,env):return {env['GIT_CONFIG_KEY_'+str(i)]:env['GIT_CONFIG_VALUE_'+str(i)] for i in range(int(env['GIT_CONFIG_COUNT']))}

    def test_baseline_fetch_has_scoped_explicit_route_tls_redirect_and_root_token_environment(self):
        root,mirror,call,error=self.capture();args=call.args[0];env=call.kwargs['env'];options=self.options(env)
        self.assertEqual(options[SCOPE+'proxy'],PROXY)
        self.assertEqual(options[SCOPE+'sslVerify'],'true')
        self.assertEqual(options[SCOPE+'followRedirects'],'false')
        self.assertTrue(options[SCOPE+'extraheader'].startswith('AUTHORIZATION: basic '))
        self.assertNotIn('PRIVATE_FIXTURE_TOKEN',str(args));self.assertNotIn('AUTHORIZATION',str(args))
        self.assertEqual(args[-2:],['https://github.com/owner/project.git','b'*40])
        self.assertEqual(call.kwargs['timeout'],120);self.assertTrue((root/'frozen.json').exists())
        self.assertFalse((root/'source').exists());self.assertNotIn('PRIVATE',str(error))
        self.assertFalse(any(key in env for key in ('HTTPS_PROXY','NO_PROXY','ALL_PROXY')))

    def test_explicit_null_clears_proxy_and_legacy_omission_keeps_reviewed_mirror_route(self):
        for route in (None,MISSING):
            _,_,call,_=self.capture(route);options=self.options(call.kwargs['env'])
            if route is None:self.assertEqual(options[SCOPE+'proxy'],'')
            else:self.assertNotIn(SCOPE+'proxy',options)

    def test_invalid_route_blocks_before_journal_api_token_freeze_or_fetch(self):
        root,mirror,config,reader,journal=self.fixture('http://private@proxy:80')
        with patch.object(S.subprocess,'run',side_effect=AssertionError('network call')):
            with self.assertRaises(C.Blocked):S.prepare(config,reader,journal,b'bundle','a'*40)
        journal.outstanding.assert_not_called();reader.repo.assert_not_called();reader.installation_token.assert_not_called()
        self.assertFalse((root/'frozen.json').exists())

    def test_failed_or_timed_out_fetch_never_retries_direct_or_exposes_transport_output(self):
        for error in (None,subprocess.TimeoutExpired(['git','fetch'],120,output=b'PRIVATE',stderr=b'PRIVATE')):
            with self.subTest(kind=type(error).__name__):
                _,_,call,rejected=self.capture(PROXY,error)
                self.assertNotIn('PRIVATE',str(rejected));self.assertEqual(self.options(call.kwargs['env'])[SCOPE+'proxy'],PROXY)

    def test_native_git_urlmatch_environment_overrides_local_route_without_config_write(self):
        _,mirror,call,_=self.capture();env=call.kwargs['env'];path=mirror/'config'
        with path.open('a') as output:output.write('\n[http "https://github.com/owner/project.git"]\n proxy = http://obsolete:1\n sslVerify = false\n followRedirects = true\n')
        before=path.read_bytes()
        for key,value in (('http.proxy',PROXY),('http.sslVerify','true'),('http.followRedirects','false')):
            raw=subprocess.check_output(['/usr/bin/git','-C',str(mirror),'config','--get-urlmatch',key,'https://github.com/owner/project.git'],env=env)
            self.assertEqual(raw.decode().strip(),value)
        self.assertEqual(path.read_bytes(),before)


class ControllerClientRouteTests(unittest.TestCase):
    def test_actual_configuration_rejects_bad_route_before_lease_journal_or_clients(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);config={'schema':'gatehouse-trusted-host/v1','repository':'owner/project','base_branch':'main',
                'profile':'full','app_id':123,'developer_uid':999,'https_proxy':'http://private@proxy:80',
                **{name:str(root/name) for name in ('records','mirror','scratch','policy_directory','source','inbox')},
                'cache':{'root':str(root/'cache')},'freshness_seconds':60,'timeout_seconds':60}
            with patch.object(CLI.os,'geteuid',return_value=0),patch.object(CLI,'protected',side_effect=lambda p,**kw:Path(p)), \
                 patch.object(CLI,'controller_configuration',return_value=config), \
                 patch.object(CLI,'submission_inbox',side_effect=Path),patch.object(CLI,'unprivileged_identity') as identity, \
                 patch.object(CLI,'Journal') as journal,patch.object(CLI,'GitHub') as clients,redirect_stderr(io.StringIO()):
                self.assertEqual(CLI.main(['status']),1)
            journal.assert_not_called();clients.assert_not_called();identity.assert_not_called()
            self.assertFalse((root/'records').exists())

    def test_both_checks_and_delivery_clients_receive_same_pinned_route_with_existing_permissions(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'controller.lock').touch()
            config={'records':str(root),'repository':'owner/project','app_id':123,'installation_id':456,'app_key':'/private/checks',
                    'https_proxy':PROXY,'inbox':'/unused','developer_uid':999,
                    'delivery':{'app_id':789,'installation_id':987,'app_key':'/private/delivery'}}
            client=Mock();publisher=Mock();journal=Mock();factory=Mock(side_effect=[client,publisher])
            with patch.object(CLI,'configuration',return_value=config),patch.object(CLI,'controller_configuration',return_value=config), \
                 patch.object(CLI,'Journal',return_value=journal),patch.object(CLI,'Executor'),patch.object(CLI,'Control'), \
                 patch.object(CLI,'GitHub',factory),patch.object(CLI,'deliver',return_value={'state':'fixture'}), \
                 patch.object(CLI,'read_submission',return_value=b'fixture body'),redirect_stdout(io.StringIO()):
                self.assertEqual(CLI.main(['deliver','fixture-run','--title','fixture','--body-file','/unused/body']),0)
            self.assertEqual(factory.call_args_list[0].kwargs,{'https_proxy':PROXY})
            self.assertEqual(factory.call_args_list[1].kwargs,{'permissions':{'contents':'write','pull_requests':'write'},'https_proxy':PROXY})
            journal.close.assert_called_once()


if __name__=='__main__':unittest.main()
