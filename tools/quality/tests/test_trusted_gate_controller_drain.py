"""Proc parser/classifier and fenced observation fixtures; no Root host scan."""
import importlib.util
import ctypes
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT=Path(__file__).resolve().parents[3]

def load(name,filename):
    spec=importlib.util.spec_from_file_location(name,ROOT/'tools/trusted_gate/deploy'/filename)
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value

D=load('controller_drain_fixture','controller-drain.py')
BOOT=load('controller_drain_entry_fixture','import-protected-inputs.py')


class ControllerDrainTests(unittest.TestCase):
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory();self.addCleanup(self.temporary.cleanup)
        self.root=Path(self.temporary.name)
        self.process(100,uids=(0,0,0,0));self.process(200,uids=(1000,1000,1000,1000))

    def process(self,pid,uids=(0,0,0,0),state='S',flags=0,comm='ordinary',argv=b'/usr/bin/ordinary\0',
                cwd='/',exe='/usr/bin/ordinary',cgroup=b'0::/system.slice/ordinary.service\n',ppid=1,start=123):
        root=self.root/str(pid);root.mkdir(exist_ok=True)
        fields=[state,str(ppid),'1','1','0','0',str(flags)]+['0']*12+[str(start)]+['0']*30
        (root/'stat').write_bytes(str(pid).encode()+b' ('+comm.encode()+b') '+' '.join(fields).encode()+b'\n')
        status=(f'Name:\t{comm}\nPid:\t{pid}\nTgid:\t{pid}\nPPid:\t{ppid}\nUid:\t'+
                ' '.join(map(str,uids))+'\nGid:\t0 0 0 0\nNSpid:\t'+str(pid)+'\n')
        (root/'status').write_text(status);(root/'cmdline').write_bytes(argv);(root/'cgroup').write_bytes(cgroup)
        for name,target in (('cwd',cwd),('exe',exe)):
            path=root/name
            if path.is_symlink():path.unlink()
            path.symlink_to(target)
        return root

    def view(self):return D.ProcView(self.root)

    def test_safe_proc_fixture_scan_has_only_hashes_and_identity_fields(self):
        result=self.view().scan(100)
        self.assertEqual([row['pid'] for row in result],[100,200])
        self.assertEqual(result[0]['uids'],[0]*4)
        self.assertNotIn('argv',result[0]);self.assertNotIn('cwd',result[0])
        self.assertEqual(len(result[0]['identity_sha256']),64)

    def test_comm_spaces_and_parentheses_preserve_starttime_index(self):
        path=self.process(200,comm='gate) fake (x',start=9876)
        value=D.process_stat((path/'stat').read_bytes(),200)
        self.assertEqual(value['start'],9876);self.assertEqual(value['ppid'],1)
        self.assertNotEqual((path/'stat').read_bytes().split()[21],b'9876')

    def test_any_restricted_uid_or_root_saved_identity_is_classified(self):
        for uids in ((1000,999,1000,1000),(1000,1000,995,1000),(1000,1000,1000,999),
                     (1000,1000,0,1000),(1000,0,1000,1000)):
            self.process(200,uids=uids,argv=b'/usr/bin/python3.14\0/opt/gatehouse/trusted_gate/host.py\0status\0')
            with self.subTest(uids=uids),self.assertRaises(ValueError):self.view().scan(100)

    def test_managed_cwd_executable_argument_and_unit_cgroup_all_block(self):
        variants=({'cwd':'/srv/gatehouse/pilot/repository/mirror.git'},
                  {'exe':'/opt/gatehouse/bin/harness-gate (deleted)'},
                  {'argv':b'/usr/bin/openssl\0-sign\0/etc/gatehouse/checks.private-key.pem\0'},
                  {'argv':b'/usr/bin/systemctl\0gatehouse-core-run.service\0'},
                  {'argv':b'/usr/bin/systemd-run\0--unit=gatehouse-bootstrap-run.service\0'},
                  {'cgroup':b'0::/system.slice/gatehouse-step-run.service\n'})
        for values in variants:
            self.process(200,**values)
            with self.subTest(values=values),self.assertRaises(ValueError):self.view().scan(100)

    def test_unprivileged_operator_markers_are_not_a_root_drain_claim(self):
        self.process(200,uids=(1000,1000,1000,1000),argv=b'/opt/gatehouse/trusted_gate/host.py\0')
        result=self.view().scan(100)
        self.assertFalse(result[1]['managed'])
        self.assertEqual(result[1]['uids'],[1000]*4)

    def test_kernel_empty_cmdline_and_zombie_are_explicit(self):
        self.process(200,flags=D.KTHREAD,argv=b'')
        self.assertTrue(self.view().scan(100)[1]['kernel'])
        self.process(200,state='Z',argv=b'')
        self.assertTrue(self.view().scan(100)[1]['dead'])
        self.process(200,argv=b'')
        with self.assertRaises(ValueError):self.view().scan(100)

    def test_inconsistent_kernel_identity_is_rejected(self):
        self.process(200,uids=(0,1000,0,0),flags=D.KTHREAD,argv=b'')
        with self.assertRaises(ValueError):self.view().scan(100)

    def test_changed_starttime_or_uids_during_observation_rejected(self):
        for target in ('stat','status'):
            view=self.view();read=view.read;calls=[]
            def mutate(fd,name):
                raw=read(fd,name)
                if name==target:
                    calls.append(name)
                    if len(calls)==2:
                        return raw.replace(b'123 ',b'456 ') if target=='stat' else raw.replace(b'0 0 0 0',b'999 999 999 999')
                return raw
            with self.subTest(target=target),patch.object(view,'read',side_effect=mutate),self.assertRaises(ValueError):
                view.observe(100)

    def test_changed_cmdline_cwd_executable_and_cgroup_rejected(self):
        for target in ('cmdline','cwd','exe','cgroup'):
            view=self.view();original=view.link if target in ('cwd','exe') else view.read;calls=[]
            def mutate(fd,name):
                raw=original(fd,name)
                if name==target:
                    calls.append(name)
                    if len(calls)==2:return raw+b'changed'
                return raw
            with self.subTest(target=target),patch.object(view,'link' if target in ('cwd','exe') else 'read',side_effect=mutate),self.assertRaises(ValueError):
                view.observe(100)

    def test_unreadable_or_disappeared_process_is_not_silently_skipped(self):
        for error in (PermissionError('fixture'),FileNotFoundError('fixture')):
            view=self.view()
            with patch.object(view,'read',side_effect=error),self.assertRaises(OSError):view.scan(100)

    def test_pid_list_changes_and_directory_links_fail(self):
        view=self.view()
        with patch.object(view,'pids',side_effect=[[100,200],[100,200,300]]),self.assertRaises(ValueError):view.scan(100)
        (self.root/'300').symlink_to(self.root/'200',target_is_directory=True)
        with self.assertRaises(OSError):self.view().observe(300)

    def test_pid_count_time_file_and_total_budgets_fail_closed(self):
        with patch.object(D,'MAX_PIDS',1),self.assertRaises(ValueError):self.view().pids()
        view=self.view();view.deadline=0
        with self.assertRaises(ValueError):view.pids()
        (self.root/'100/status').write_bytes(b'x'*(D.MAX_FILE+1))
        with self.assertRaises(ValueError):self.view().observe(100)
        view=self.view();view.total=D.MAX_TOTAL
        with self.assertRaises(ValueError):view.observe(100)

    def test_status_duplicates_aliases_mismatch_and_missing_fields_fail(self):
        raw=(self.root/'100/status').read_bytes()
        for value in (raw+b'Uid:\t0 0 0 0\n',raw.replace(b'Pid:\t100',b'Pid:\t101'),
                      raw.replace(b'NSpid:\t100',b'NSpid:\t101'),raw.replace(b'Uid:\t0 0 0 0',b'Uid:\t0 0 0'),
                      raw.replace(b'Uid:\t0 0 0 0',b'Uid:\t-1 0 0 0'),raw.replace(b'Gid:',b'Other:')):
            with self.assertRaises(ValueError):D.process_status(value,100)

    def test_stat_bad_pid_state_truncation_and_numeric_bounds_fail(self):
        raw=(self.root/'100/stat').read_bytes()
        for value in (b'bad',raw.replace(b'100 (',b'101 ('),raw.replace(b') S ',b') Q '),
                      b'100 (x) S 1',raw.replace(b'123 ',b'999999999999999999999999999999 ')):
            with self.assertRaises(ValueError):D.process_stat(value,100)

    def test_caller_must_be_present_live_root_and_not_kernel(self):
        for uid,state,flags in ((1000,'S',0),(0,'Z',0),(0,'S',D.KTHREAD)):
            self.process(100,uids=(uid,)*4,state=state,flags=flags,argv=b'' if flags else b'/usr/bin/ordinary\0')
            with self.assertRaises(ValueError):self.view().scan(100)
        with self.assertRaises(ValueError):self.view().scan(9999)


class FencedObservationTests(unittest.TestCase):
    def setUp(self):
        self.fence_value={'head':'a'*40,'transaction':'fixture-tx'}
        self.fence=SimpleNamespace(fenced=Mock(side_effect=lambda B,fd:dict(self.fence_value)))
        self.builder=SimpleNamespace(quiet_units=Mock())
        self.B=SimpleNamespace(need=BOOT.need,canonical=BOOT.canonical,
            authenticate=Mock(return_value={'revision':'a'*40}),load=lambda root,name:{
                'install-transaction.py':self.fence,'build-protected-core.py':self.builder}[name])
        self.rows=[{'pid':100,'start':123}]
        self.view=SimpleNamespace(scan=Mock(side_effect=lambda pid:list(self.rows)))
        for target,key,value in ((os,'geteuid',lambda:0),(os,'getpid',lambda:100)):
            p=patch.object(target,key,side_effect=value)
            p.start();self.addCleanup(p.stop)
        for key,value in (('ProcView',lambda:self.view),('proc_context',lambda:{'pid':'namespace'})):
            p=patch.object(D,key,side_effect=value);p.start();self.addCleanup(p.stop)

    def observe(self):return D.observe(self.B,Path('/fixture/helpers'),Path('/fixture/manifest'),'b'*64,77)

    def test_observation_authenticates_rechecks_fence_and_never_becomes_install_acceptance(self):
        result=self.observe()
        self.assertEqual(result['state'],'observed-empty');self.assertEqual(result['scans'],2)
        self.assertIs(result['controller_drain_proven'],False);self.assertIs(result['administrator_ready'],False)
        self.assertEqual(self.B.authenticate.call_count,2);self.assertEqual(self.fence.fenced.call_count,3)
        self.assertEqual(self.builder.quiet_units.call_count,2)

    def test_nonroot_authentication_or_incomplete_fence_stops_before_scan(self):
        with patch.object(os,'geteuid',return_value=1000),self.assertRaises(ValueError):self.observe()
        self.B.authenticate.side_effect=ValueError('unauthenticated')
        with self.assertRaises(ValueError):self.observe()
        self.B.authenticate.side_effect=None;self.fence.fenced.side_effect=ValueError('not fenced')
        with self.assertRaises(ValueError):self.observe()
        self.view.scan.assert_not_called()

    def test_changing_fence_or_cross_scan_starttime_blocks(self):
        self.fence.fenced.side_effect=[dict(self.fence_value),{**self.fence_value,'head':'c'*40}]
        with self.assertRaises(ValueError):self.observe()
        self.fence.fenced.side_effect=lambda B,fd:dict(self.fence_value)
        self.view.scan.side_effect=[self.rows,[{'pid':100,'start':124}]]
        with self.assertRaises(ValueError):self.observe()

    def test_unit_or_postflight_authentication_failure_never_returns_acceptance(self):
        self.builder.quiet_units.side_effect=ValueError('unit populated')
        with self.assertRaises(ValueError):self.observe()
        self.builder.quiet_units.side_effect=None
        self.B.authenticate.side_effect=[{'revision':'a'*40},ValueError('changed helpers')]
        with self.assertRaises(ValueError):self.observe()

    def test_namespace_changes_between_scans_never_returns_observation(self):
        with patch.object(D,'proc_context',side_effect=[{'pid':'one'},{'pid':'two'}]),self.assertRaises(ValueError):
            self.observe()

    def test_direct_invocation_refuses_an_administrator_entry(self):
        result=subprocess.run([sys.executable,'-B',str(ROOT/'tools/trusted_gate/deploy/controller-drain.py')],
                              capture_output=True,timeout=10)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'no administrator entry',result.stderr)


class ProcContextTests(unittest.TestCase):
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory();self.addCleanup(self.temporary.cleanup)
        self.root=Path(self.temporary.name);(self.root/'self').mkdir()
        (self.root/'self/uid_map').write_text('0 0 4294967295\n')
        self.info=SimpleNamespace(st_mode=0o40755,st_uid=0)
        self.magic=0x9fa0
        def statfs(fd,buffer):
            raw=self.magic.to_bytes(ctypes.sizeof(ctypes.c_long),sys.byteorder,signed=True)
            ctypes.memmove(buffer,raw,len(raw));return 0
        self.lib=SimpleNamespace(fstatfs=Mock(side_effect=statfs))
        self.link=lambda path:Path(path).name+':[1]'
        for target,key,value in ((D,'PROC',self.root),(os,'geteuid',lambda:0),
                                 (os,'uname',lambda:SimpleNamespace(machine='x86_64')),
                                 (os,'readlink',lambda path:self.link(path)),
                                 (D.ctypes,'CDLL',lambda *a,**k:self.lib)):
            p=patch.object(target,key,side_effect=value) if callable(value) else patch.object(target,key,value)
            p.start();self.addCleanup(p.stop)
        p=patch.object(Path,'lstat',return_value=self.info);p.start();self.addCleanup(p.stop)

    def test_kernel_proc_and_initial_namespace_fixture_is_required(self):
        self.assertEqual(D.proc_context(),{name:name+':[1]' for name in ('pid','user','mnt')})

    def test_nonroot_or_wrong_architecture_stops_before_proc_open(self):
        with patch.object(os,'geteuid',return_value=1000),self.assertRaises(ValueError):D.proc_context()
        with patch.object(os,'uname',return_value=SimpleNamespace(machine='aarch64')),self.assertRaises(ValueError):D.proc_context()

    def test_nonproc_filesystem_and_statfs_error_are_rejected(self):
        self.magic=0xef53
        with self.assertRaises(ValueError):D.proc_context()
        self.lib.fstatfs.side_effect=None;self.lib.fstatfs.return_value=-1
        with self.assertRaises(ValueError):D.proc_context()

    def test_namespace_or_user_map_mismatch_and_mapping_budget_are_rejected(self):
        self.link=lambda path:'different' if '/1/' in str(path) else 'original'
        with self.assertRaises(ValueError):D.proc_context()
        self.link=lambda path:Path(path).name+':[1]'
        for raw in ('0 1000 1\n','x'*4097):
            (self.root/'self/uid_map').write_text(raw)
            with self.assertRaises(ValueError):D.proc_context()

    def test_unprotected_or_link_proc_mount_is_rejected(self):
        for mode,uid in ((0o40755,1000),(0o40777,0),(0o120777,0)):
            self.info.st_mode=mode;self.info.st_uid=uid
            with self.assertRaises(ValueError):D.proc_context()


if __name__=='__main__':unittest.main()
