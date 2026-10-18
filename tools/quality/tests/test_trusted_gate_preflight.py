"""Preflight rejects drift without running validation or changing logical state."""
import copy
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import sys
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
import urllib.request
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools/trusted_gate/deploy' / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BOOT = load('preflight_entry_tests', 'import-protected-inputs.py')
P = load('preflight_library_tests', 'preflight-history-pilot.py')
R = load('preflight_contract_tests', 'bootstrap-release.py')
KEEP = load('preflight_retention_tests', 'preserve-pilot-state.py')
EFFECTS = load('preflight_transport_tests', 'admit-upgrade-effects.py')


class PreflightTests(unittest.TestCase):
    def setUp(self):
        mask = os.umask(0o077); self.addCleanup(os.umask, mask)
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.volume = self.root / 'pilot'; self.stage = self.root / 'stage'
        self.control = self.volume / 'control'
        for path in (self.control, self.volume / 'repository', self.stage): path.mkdir(parents=True)
        (self.control / 'controller.lock').touch(mode=0o600)
        self.config_path = self.root / 'host.json'; self.marker = self.root / 'marker.json'
        self.B = SimpleNamespace(**{name: getattr(BOOT, name) for name in ('need', 'canonical', 'unique', 'ENV')})
        self.B.owned = self.owned; self.B.read = self.read
        self.B.authenticate = Mock(return_value={'revision': 'a'*40})
        self.builder = SimpleNamespace(quiet_units=Mock(), host_file=lambda path: ('host:'+path).encode())
        self.B.load = lambda helpers, file: {'bootstrap-release.py': R, 'preserve-pilot-state.py': KEEP,
                                             'build-protected-core.py': self.builder}[file]
        self.B.git = lambda source,*args: b'f'*40+b'\n' if args[0]=='rev-parse' else b''
        self.B.frozen_lock = lambda *args: b'locked fixture'
        self.payload = {P.member_name(row[0]): ('source:'+name).encode()
                        for name,row in R.old_source_map().items()}
        self.payload.update({'source.bundle': b'accepted bundle', 'bin/harness-gate': b'accepted release binary'})
        self.payload['metadata.json'] = BOOT.canonical({'repository': P.REPOSITORY, 'head': P.HEAD,
            'base_at_packaging': R.BASE, 'coverage_acceptance': 'transition', 'core_profile': 'release',
            'release_incremental': False, 'worker_git_history_probe': True,
            'core_build': {'sha256': P.digest(self.payload['bin/harness-gate'])}})
        references = {name: self.payload[P.member_name(row[0])] for name,row in R.old_source_map().items()}
        for name in ('install-pilot.py', 'upgrade-history-pilot.py'):
            raw = b'raise AssertionError("old script must never execute")\n'
            (self.stage / name).write_bytes(raw); references[R.DEPLOY+name]=raw
        self.old_inputs = P.digest(BOOT.canonical(R.rows(references)))
        self.archive = self.encoded()
        release_pin = patch.object(P, 'RELEASE', P.digest(self.archive))
        release_pin.start(); self.addCleanup(release_pin.stop)
        (self.stage / 'release.tar').write_bytes(self.archive)
        for name,row in R.old_source_map().items():
            self.put(row[0], self.payload[P.member_name(row[0])], row[1])
        self.put('/opt/gatehouse/bin/harness-gate', self.payload['bin/harness-gate'], 0o755)
        self.put('/etc/gatehouse/pilot.sudoers', self.payload['sudoers'], 0o440)
        self.pins = {path:P.digest(raw) for path,raw in {
            **{row[0]:self.payload[P.member_name(row[0])] for row in R.old_source_map().values()
               if row[0].startswith(('/opt/gatehouse/', '/etc/gatehouse/policy/'))},
            '/opt/gatehouse/bin/harness-gate':self.payload['bin/harness-gate'],
            '/usr/bin/python3.14':b'host:/usr/bin/python3.14', '/usr/bin/git':b'host:/usr/bin/git'}.items()}
        self.config = {'schema':'gatehouse-trusted-host/v1', 'repository':P.REPOSITORY,
            'repository_id':P.REPOSITORY_ID, 'base_branch':'main', 'coverage_acceptance':'transition', 'profile':'full',
            'developer_uid':999, 'executor_user':'gatehouse-executor', 'protected_files':self.pins,
            'core':'/opt/gatehouse/bin/harness-gate', 'policy_directory':'/etc/gatehouse/policy',
            'environment':{'source_revision':P.HEAD, 'release_sha256':P.RELEASE},
            'app_id':4867361,'installation_id':159921491,'app_key':'/etc/gatehouse/checks.private-key.pem',
            'delivery':{'app_id':5069731,'installation_id':164696480,'app_key':'/etc/gatehouse/delivery.private-key.pem'}}
        self.config.update({key:str(self.volume/relative) for key,relative in (
            ('source','source'),('mirror','repository/mirror.git'),('records','control'),('inbox','inbox'),('scratch','scratch'))})
        self.accepted = {'head':P.HEAD, 'run':P.RUN,'release_sha256':P.RELEASE,'coverage_acceptance':'transition',
            'production_gate_enabled':False,'authoritative_publish':'blocked','negative_conclusion':'failure',
            'negative_check':P.NEGATIVE_CHECK,'worker_git_history':'passed within the protected Core boundary step'}
        self.dump(self.config_path,self.config)
        self.dump(self.marker,{'head':P.HEAD,'release_sha256':P.RELEASE,'volume_uuid':'accepted-uuid'})
        self.dump(self.control/'pilot-acceptance.json',self.accepted)
        self.dump(self.volume/'repository/frozen.json', {'repository':P.REPOSITORY,'repository_id':P.REPOSITORY_ID,
            'head':P.HEAD,'base':R.BASE,'bundle_sha256':P.digest(self.payload['source.bundle'])})
        (self.volume/'repository/candidate.bundle').write_bytes(self.payload['source.bundle'])
        self.writer = sqlite3.connect(self.control/'journal.sqlite3',isolation_level=None)
        self.addCleanup(self.writer.close)
        self.writer.executescript('''CREATE TABLE runs (id TEXT PRIMARY KEY,repository TEXT NOT NULL,head TEXT NOT NULL,
            epoch INTEGER NOT NULL,input TEXT NOT NULL,local TEXT NOT NULL,evidence TEXT,remote TEXT NOT NULL,
            check_id INTEGER,pr INTEGER,reason TEXT,created REAL NOT NULL,UNIQUE(repository,head,epoch));
            CREATE TABLE events (sequence INTEGER PRIMARY KEY,run TEXT NOT NULL,time REAL NOT NULL,
            operation TEXT NOT NULL,detail TEXT NOT NULL);''')
        self.inputs={'repository':P.REPOSITORY,'repository_id':P.REPOSITORY_ID,'base':R.BASE,'tree':'f'*40,
                     'policy':P.digest(json.dumps(self.config,sort_keys=True,separators=(',',':')).encode())}
        self.writer.execute('INSERT INTO runs VALUES (?,?,?,?,?,?,?,?,?,?,?,?)',
            (P.RUN,P.REPOSITORY,P.HEAD,1,json.dumps(self.inputs),'failed',None,'pending',None,None,'retained failure',1.0))
        self.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)',(1,P.RUN,1.0,'created','{}'))

    def domain(self):
        return patch.multiple(P,STAGE=self.stage,VOLUME=self.volume,CONFIG=self.config_path,MARKER=self.marker,
                              INSTALLER=P.digest((self.stage/'install-pilot.py').read_bytes()),
                              UPGRADER=P.digest((self.stage/'upgrade-history-pilot.py').read_bytes()),
                              RELEASE=P.digest(self.archive))

    def path(self,path):
        path=Path(path)
        return path if path==self.root or self.root in path.parents else self.root/'fs'/str(path).lstrip('/')

    def owned(self,path,directory=False,private=False):
        path=self.path(path);info=path.lstat()
        for parent in (path,*path.parents): BOOT.need(not parent.is_symlink(),'fixture symlink')
        BOOT.need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode),'fixture type')
        BOOT.need(directory or info.st_nlink==1,'fixture hardlink')
        BOOT.need(not info.st_mode&0o022 and (not private or not info.st_mode&0o077),'fixture permissions')
        return path

    def read(self,path,limit):
        raw=self.owned(path).read_bytes();BOOT.need(len(raw)<=limit,'fixture bound');return raw

    def put(self,path,raw,mode=0o600):
        path=self.path(path);path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw);path.chmod(mode)
        return path

    def dump(self,path,value): path.write_bytes(BOOT.canonical(value))

    def encoded(self):
        target=io.BytesIO()
        with tarfile.open(fileobj=target,mode='w:') as archive:
            for name,raw in sorted(self.payload.items()):
                item=tarfile.TarInfo(name);item.size=len(raw);archive.addfile(item,io.BytesIO(raw))
        return target.getvalue()

    def observe(self,remote_fn=None,**kwargs):
        # Ordinary fixture replaces Root ownership/NSS/kernel/network boundaries;
        # stage parsing, installed bytes, JSON, SQLite, source identities and lease execute.
        with self.domain(),patch.object(R,'OLD_INPUTS',self.old_inputs), \
             patch.object(P.os,'geteuid',return_value=0),patch.object(P,'accounts'), \
             patch.object(P,'keys',return_value={'checks':'private-pin','delivery':'private-pin'}), \
             patch.object(P,'mount_identity',return_value='mount-observation'), \
             patch.object(P,'remote',side_effect=remote_fn,return_value={'base':R.BASE}):
            return P.observe(self.B,self.root/'helpers',self.root/'manifest','a'*64,**kwargs)

    def test_observation_keeps_failed_raw_result_slot_and_database_unchanged(self):
        # The accepted slot may have a retained failed diagnostic after acceptance;
        # this never becomes a new PASS or authorizes retirement.
        with patch.object(P,'configuration',wraps=P.configuration) as verify:
            before=(self.control/'journal.sqlite3').read_bytes()
            result=self.observe()
        self.assertEqual(result['state'],'observed');self.assertFalse(result['administrator_ready'])
        self.assertFalse(result['production_gate_enabled']);self.assertEqual(verify.call_count,2)
        self.assertEqual((self.control/'journal.sqlite3').read_bytes(),before)
        self.assertEqual(self.writer.execute('SELECT local,remote FROM runs').fetchone(),('failed','pending'))
        self.assertEqual(self.writer.execute('SELECT count(*) FROM events').fetchone(),(1,))
        self.assertFalse((self.control/'journal.sqlite3-wal').exists())

    def test_accepted_archive_fixed_hash_before_parse_and_no_old_script_execution(self):
        with self.domain(),patch.object(R,'OLD_INPUTS',self.old_inputs):
            self.assertEqual(P.accepted_release(self.B,R),self.payload)
            (self.stage/'release.tar').write_bytes(b'untrusted')
            with patch.object(P.tarfile,'open',side_effect=AssertionError('must hash before parse')):
                with self.assertRaises(ValueError):P.accepted_release(self.B,R)

    def test_old_installer_or_upgrader_hash_drift_blocks(self):
        with self.domain(),patch.object(R,'OLD_INPUTS',self.old_inputs):
            for name in ('install-pilot.py','upgrade-history-pilot.py'):
                path=self.stage/name;raw=path.read_bytes();path.write_bytes(raw+b'changed')
                with self.assertRaises(ValueError):P.accepted_release(self.B,R)
                path.write_bytes(raw)

    def test_rehashed_release_cannot_change_old_source_reference_or_core_provenance(self):
        with self.domain():
            for name in ('trusted_gate/common.py','bin/harness-gate'):
                old=self.payload[name];self.payload[name]=b'changed';raw=self.encoded()
                (self.stage/'release.tar').write_bytes(raw)
                with patch.object(P,'RELEASE',P.digest(raw)),patch.object(R,'OLD_INPUTS',self.old_inputs):
                    with self.assertRaises(ValueError):P.accepted_release(self.B,R)
                self.payload[name]=old

    def test_complete_installed_inventory_and_os_pins(self):
        self.assertEqual(P.installed(self.B,R,self.payload,self.builder),self.pins)
        self.put('/opt/gatehouse/trusted_gate/runtime.py',b'new code before migration',0o644)
        with self.assertRaises(ValueError):P.installed(self.B,R,self.payload,self.builder)

    def test_installed_bytes_modes_missing_and_links_block(self):
        path=self.path('/opt/gatehouse/bin/harness-gate');raw=path.read_bytes()
        for kind in ('bytes','mode','missing','symlink','hardlink'):
            with self.subTest(kind=kind):
                path.unlink(missing_ok=True)
                if kind=='missing':pass
                elif kind=='symlink':path.symlink_to(self.stage/'release.tar')
                elif kind=='hardlink':os.link(self.stage/'release.tar',path)
                else:path.write_bytes(b'changed' if kind=='bytes' else raw);path.chmod(0o777 if kind=='mode' else 0o755)
                with self.assertRaises((ValueError,FileNotFoundError)):P.installed(self.B,R,self.payload,self.builder)
                if path.exists() or path.is_symlink():path.unlink()
        path.write_bytes(raw);path.chmod(0o755)

    def test_safe_root_bytecode_is_observed_without_execution_unknown_names_block(self):
        cache=self.path('/opt/gatehouse/trusted_gate/__pycache__');cache.mkdir()
        (cache/'github.cpython-314.pyc').write_bytes(b'never execute arbitrary bytecode')
        self.assertEqual(P.installed(self.B,R,self.payload,self.builder),self.pins)
        for name in ('runtime.cpython-314.pyc','github.cpython-311.pyc','unknown.py','github.cpython-314.opt-9.pyc'):
            path=cache/name;path.write_bytes(b'changed')
            with self.assertRaises(ValueError):P.installed(self.B,R,self.payload,self.builder)
            path.unlink()

    def test_configuration_pins_app_key_or_authority_promotion_block(self):
        marker=json.loads(self.marker.read_bytes())
        with self.domain(): P.configuration(self.B,self.config,marker,self.accepted,self.pins,R)
        for key,new in [('protected_files',{}),('coverage_acceptance','accepted'),('app_id',1),('app_key','/tmp/key')]:
            config=copy.deepcopy(self.config);config[key]=new
            with self.assertRaises(ValueError):P.configuration(self.B,config,marker,self.accepted,self.pins,R)
        for key,new in [('production_gate_enabled',True),('authoritative_publish','success'),('negative_check',1),('run','another')]:
            accepted={**self.accepted,key:new}
            with self.assertRaises(ValueError):P.configuration(self.B,self.config,marker,accepted,self.pins,R)

    def test_readonly_wal_rows_and_unknown_or_other_slot_rejected(self):
        with self.domain():
            self.writer.execute('PRAGMA journal_mode=WAL');self.writer.execute('PRAGMA wal_autocheckpoint=0')
            self.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)',(2,P.RUN,2.0,'wal-only','{}'))
            before=(self.control/'journal.sqlite3').read_bytes();wal=(self.control/'journal.sqlite3-wal').read_bytes()
            identity,run,inputs=P.journal(self.B,KEEP,self.config)
            self.assertEqual(identity['events']['rows'],2);self.assertEqual(run['local'],'failed')
            self.assertEqual((self.control/'journal.sqlite3').read_bytes(),before)
            self.assertEqual((self.control/'journal.sqlite3-wal').read_bytes(),wal)
            for column,new in [('head','b'*40),('local','running'),('remote','revoked'),('pr',127)]:
                old=self.writer.execute('SELECT '+column+' FROM runs').fetchone()[0]
                self.writer.execute('UPDATE runs SET '+column+'=?',(new,))
                with self.assertRaises(ValueError):P.journal(self.B,KEEP,self.config)
                self.writer.execute('UPDATE runs SET '+column+'=?',(old,))

    def test_missing_wal_shared_memory_is_not_recreated(self):
        with self.domain():
            (self.control/'journal.sqlite3-wal').touch()
            with self.assertRaisesRegex(ValueError,'shared-memory'):P.journal(self.B,KEEP,self.config)
            self.assertFalse((self.control/'journal.sqlite3-shm').exists())

    def test_unknown_schema_policy_change_and_database_budget_block(self):
        with self.domain():
            self.writer.execute('CREATE VIEW extra AS SELECT * FROM runs')
            with self.assertRaises(ValueError):P.journal(self.B,KEEP,self.config)
            self.writer.execute('DROP VIEW extra')
            with self.assertRaises(ValueError):P.journal(self.B,KEEP,{**self.config,'extra':True})
            with patch.object(KEEP,'MAX_DB',1):
                with self.assertRaises(ValueError):P.journal(self.B,KEEP,self.config)

    def test_frozen_bundle_source_tree_or_worktree_drift_block(self):
        with self.domain():
            P.frozen(self.B,self.inputs,self.payload,R)
            path=self.volume/'repository/candidate.bundle';old=path.read_bytes();path.write_bytes(b'changed')
            with self.assertRaises(ValueError):P.frozen(self.B,self.inputs,self.payload,R)
            path.write_bytes(old)
            with patch.object(self.B,'git',return_value=b'changed'):
                with self.assertRaises(ValueError):P.frozen(self.B,self.inputs,self.payload,R)

    def test_remote_only_requests_read_token_and_repository_base_gets(self):
        client=Mock(token='private-token',expires=100)
        client.repo.side_effect=[{'id':P.REPOSITORY_ID,'full_name':P.REPOSITORY,'default_branch':'main'},
            {'ref':'refs/heads/main','object':{'type':'commit','sha':R.BASE}}]
        cls=Mock(return_value=client)
        with patch.object(P,'client_class',return_value=cls):
            self.assertEqual(P.remote(self.B,self.config,self.payload,R)['permissions'],{'contents':'read'})
        self.assertEqual(cls.call_args.kwargs,{'permissions':{'contents':'read'}})
        self.assertEqual([c.args for c in client.repo.call_args_list],[('GET',''),('GET','/git/ref/heads/main')])
        self.assertIsNone(client.token)

    def test_explicit_proxy_uses_authenticated_https_route_and_readonly_client(self):
        proxy='http://proxy.example.invalid:7890'
        client=Mock(token='private-token',expires=100,opener=urllib.request.build_opener(urllib.request.ProxyHandler({})))
        client.repo.side_effect=[{'id':P.REPOSITORY_ID,'full_name':P.REPOSITORY,'default_branch':'main'},
            {'ref':'refs/heads/main','object':{'type':'commit','sha':R.BASE}}]
        cls=Mock(return_value=client)
        with patch.object(P,'client_class',return_value=cls):
            P.remote(self.B,self.config,self.payload,R,api_proxy=proxy,transport=EFFECTS.transport)
        self.assertEqual(cls.call_args.kwargs,{'permissions':{'contents':'read'}})
        self.assertTrue(any(isinstance(h,urllib.request.ProxyHandler) and h.proxies=={'https':proxy}
                            for h in client.opener.handlers));self.assertIsNone(client.token)

    def test_bad_explicit_route_or_missing_transport_never_falls_back_to_direct_api(self):
        for proxy,transport in (('http://user:secret@proxy.invalid:7890',EFFECTS.transport),
                                ('http://proxy.invalid:7890',None)):
            client=Mock(token='private-token',expires=100,opener=urllib.request.build_opener(urllib.request.ProxyHandler({})))
            with patch.object(P,'client_class',return_value=Mock(return_value=client)),self.assertRaises(ValueError):
                P.remote(self.B,self.config,self.payload,R,api_proxy=proxy,transport=transport)
            client.repo.assert_not_called();self.assertIsNone(client.token)

    def test_observation_passes_explicit_recipe_proxy_to_authenticated_transport(self):
        original=self.B.load;seen=[];proxy='http://proxy.example.invalid:7890'
        def remote(*args,**kwargs):seen.append(kwargs);return {'base':R.BASE,'route':kwargs['api_proxy']}
        with patch.object(self.B,'load',side_effect=lambda helpers,name:EFFECTS if name=='admit-upgrade-effects.py' else original(helpers,name)):
            result=self.observe(remote_fn=remote,api_proxy=proxy)
        self.assertEqual(result['remote']['route'],proxy);self.assertIs(seen[0]['transport'],EFFECTS.transport)

    def test_remote_wrong_repo_moved_base_transport_failure_block_no_fallback(self):
        good=[{'id':P.REPOSITORY_ID,'full_name':P.REPOSITORY,'default_branch':'main'},
              {'ref':'refs/heads/main','object':{'type':'commit','sha':R.BASE}}]
        for mode in ('repository','base','transport'):
            client=Mock(token='private-token');rows=copy.deepcopy(good)
            if mode=='repository':rows[0]['id']=1
            if mode=='base':rows[1]['object']['sha']='b'*40
            client.repo.side_effect=OSError('unavailable') if mode=='transport' else rows
            with patch.object(P,'client_class',return_value=Mock(return_value=client)):
                with self.assertRaises((ValueError,OSError)):P.remote(self.B,self.config,self.payload,R)
            self.assertIsNone(client.token)

    def test_client_loads_authenticated_source_blobs_without_installed_import_or_bytecode(self):
        payload={name:(ROOT/name).read_bytes() for name in ('tools/trusted_gate/common.py','tools/trusted_gate/github.py')}
        payload={'trusted_gate/'+Path(name).name:raw for name,raw in payload.items()}
        cls=P.client_class(payload)
        self.assertEqual(cls.__name__,'GitHub')
        self.assertFalse(any(name.startswith('_gatehouse_preflight_readonly') for name in sys.modules))

    def test_mount_exact_target_uuid_options_backing_and_no_nested_mounts(self):
        self.volume.mkdir(exist_ok=True);image=self.root/'pilot.ext4'
        with image.open('wb') as output:output.truncate(2*1024**3)
        row={'target':str(self.volume),'fstype':'ext4','fsroot':'/','uuid':'accepted-uuid',
             'options':'rw,nodev,nosuid,noatime','source':'/dev/loop7','maj:min':'7:7'}
        info=image.stat()
        loop={'name':'/dev/loop7','back-file':str(image),'maj:min':'7:7','offset':0,'sizelimit':0,'ro':False,
              'back-ino':info.st_ino,'back-maj:min':str(os.major(info.st_dev))+':'+str(os.minor(info.st_dev))}
        marker={'volume_uuid':'accepted-uuid'}
        with self.domain(),patch.object(P,'IMAGE',image):
            for mode in ('valid','uuid','options','backing','alias','nested','readonly','offset'):
                r,l=copy.deepcopy(row),copy.deepcopy(loop)
                if mode=='uuid':r['uuid']='other'
                if mode=='options':r['options']='rw'
                if mode=='backing':l['back-file']='/another/image'
                if mode=='readonly':l['ro']=True
                if mode=='offset':l['offset']=512
                mounts=[r,r] if mode=='nested' else [r];loops=[l,l] if mode=='alias' else [l]
                with patch.object(P,'command',side_effect=[{'filesystems':mounts},{'loopdevices':loops},
                        {'filesystems':[{'target':str(self.volume),'fsroot':'/'}]}]):
                    if mode=='valid':
                        original_stat=Path.stat
                        def device_stat(path,*args,**kwargs):
                            if str(path)=='/dev/loop7':return SimpleNamespace(st_mode=stat.S_IFBLK,st_rdev=self.volume.stat().st_dev)
                            return original_stat(path,*args,**kwargs)
                        with patch.object(Path,'stat',device_stat):
                            self.assertEqual(len(P.mount_identity(self.B,marker)),64)
                    else:
                        with self.assertRaises(ValueError):P.mount_identity(self.B,marker)

    def test_exact_accounts_and_privileged_or_shared_groups_or_running_identity_block(self):
        users=[SimpleNamespace(pw_name='gatehouse-developer',pw_uid=999,pw_gid=982,pw_dir='/srv/gatehouse/developer',pw_shell='/bin/bash'),
               SimpleNamespace(pw_name='gatehouse-executor',pw_uid=995,pw_gid=981,pw_dir='/nonexistent',pw_shell='/usr/sbin/nologin')]
        groups=[SimpleNamespace(gr_name='gatehouse-developer',gr_gid=982,gr_mem=[]),
                SimpleNamespace(gr_name='gatehouse-executor',gr_gid=981,gr_mem=[]),
                SimpleNamespace(gr_name='gatehouse-submit',gr_gid=980,gr_mem=['gatehouse-developer']),
                SimpleNamespace(gr_name='docker',gr_gid=99,gr_mem=[])]
        def by_name(rows,name,key):return next(row for row in rows if getattr(row,key)==name)
        with patch.object(P.pwd,'getpwnam',side_effect=lambda name:by_name(users,name,'pw_name')), \
             patch.object(P.pwd,'getpwuid',side_effect=lambda uid:by_name(users,uid,'pw_uid')), \
             patch.object(P.pwd,'getpwall',return_value=users), \
             patch.object(P.grp,'getgrnam',side_effect=lambda name:by_name(groups,name,'gr_name')), \
             patch.object(P.grp,'getgrgid',side_effect=lambda gid:by_name(groups,gid,'gr_gid')), \
             patch.object(P.os,'getgrouplist',side_effect=lambda name,gid:[gid,980] if gid==982 else [gid]) as membership, \
             patch.object(P.subprocess,'run',return_value=SimpleNamespace(returncode=1,stdout=b'',stderr=b'')) as process:
            P.accounts(self.B)
            membership.side_effect=lambda name,gid:[gid,99]
            with self.assertRaises(ValueError):P.accounts(self.B)
            membership.side_effect=lambda name,gid:[gid,980] if gid==982 else [gid]
            groups[2].gr_mem.append('another')
            with self.assertRaises(ValueError):P.accounts(self.B)
            groups[2].gr_mem.pop();process.return_value.returncode=0
            with self.assertRaises(ValueError):P.accounts(self.B)

    def test_inherited_lease_is_not_closed_and_wrong_fd_or_busy_lease_block(self):
        lease=self.control/'controller.lock'
        with lease.open('rb') as stream:
            fcntl.flock(stream.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):self.observe()
            self.assertEqual(self.observe(lock_fd=stream.fileno())['state'],'observed')
            os.fstat(stream.fileno())
            with lease.open('rb') as rival:
                with self.assertRaises(BlockingIOError):fcntl.flock(rival.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
        with self.config_path.open('rb') as stream:
            with self.assertRaisesRegex(ValueError,'lease'):self.observe(lock_fd=stream.fileno())

    def test_journal_mutation_during_remote_observation_cannot_return_success(self):
        def changed(*args):
            self.writer.execute('INSERT INTO events VALUES (?,?,?,?,?)',(2,P.RUN,2.0,'unexpected','{}'))
            return {'base':R.BASE}
        with self.assertRaisesRegex(ValueError,'changed during preflight'):self.observe(remote_fn=changed)
        self.assertEqual(self.writer.execute('SELECT local,remote FROM runs').fetchone(),('failed','pending'))

    def test_stage_script_change_during_remote_observation_is_not_accepted(self):
        def changed(*args):
            (self.stage/'install-pilot.py').write_bytes(b'changed after initial inspection')
            return {'base':R.BASE}
        with self.assertRaisesRegex(ValueError,'stage script changed'):self.observe(remote_fn=changed)

    def test_real_nonroot_cli_blocks_before_host_read_or_credentials(self):
        if os.geteuid()==0:self.skipTest('requires actual ordinary user')
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/import-protected-inputs.py'),
            'preflight-pilot','--helper-root',str(self.root/'missing'),'--manifest',str(self.root/'missing.json'),
            '--manifest-sha256','a'*64],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'pilot preflight blocked',result.stderr)
        self.assertNotIn(b'private-token',result.stderr)


if __name__=='__main__':unittest.main()
