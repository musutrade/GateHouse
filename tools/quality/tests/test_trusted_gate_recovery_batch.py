"""Native batch Git reads keep complete byte/object comparisons, without a cache."""
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

ROOT=Path(__file__).resolve().parents[3]
SPEC=importlib.util.spec_from_file_location('recovery_batch',ROOT/'tools/trusted_gate/deploy/recover-interrupted-installation.py')
R=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(R)


def need(value,message):
    if not value:raise ValueError(message)


class BatchTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.env={'PATH':'/usr/bin:/bin','LANG':'C.UTF-8',
            'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null'}
        self.git('init','-q');self.git('config','user.name','Fixture');self.git('config','user.email','fixture@example.invalid')
        self.members=[('empty',b''),('binary',b'\x00\xff\n'*19),('text',b'plain\n')]
        for name,raw in self.members:(self.root/name).write_bytes(raw)
        self.git('add','.');self.git('commit','-qm','fixture')
        self.head=self.git('rev-parse','HEAD').decode().strip()
        # Native unprivileged Git; Root ownership is explicitly outside this fixture.
        self.B=SimpleNamespace(need=need,owned=Mock(),ENV=self.env,frozen_lock=Mock())

    def git(self,*args):
        return subprocess.run(['/usr/bin/git','-C',str(self.root),*args],env=self.env,
            capture_output=True,check=True).stdout

    def test_native_all_bytes_empty_binary_duplicate_and_packed_objects(self):
        self.git('gc','--quiet')
        R.batch_blobs(self.B,self.root,self.head,self.members+self.members)
        self.B.owned.assert_called_once_with('/usr/bin/git')

    def test_changed_expected_content_or_missing_object_rejected(self):
        for members in ([('text',b'other\n')],[('absent',b'')]):
            with self.assertRaises(ValueError):R.batch_blobs(self.B,self.root,self.head,members)

    def test_operand_injection_unknown_head_and_budget_rejected_before_process(self):
        for head,members in [('HEAD',self.members),(self.head,[('text\nHEAD',b'')]),
                (self.head,[('../text',b'')]),(self.head,[('/text',b'')]),
                (self.head,[('text',b'x'*(1024**2+1))]),(self.head,[]),(self.head,self.members*44)]:
            with patch.object(R.subprocess,'run') as run:
                with self.assertRaises(ValueError):R.batch_blobs(self.B,self.root,head,members)
                run.assert_not_called()

    def test_corrupt_framing_type_size_id_extra_or_truncation_rejected(self):
        raw=b'plain\n';oid=hashlib.sha1(b'blob 6\0'+raw).hexdigest().encode()
        good=oid+b' blob 6\n'+raw+b'\n'
        for data in (good[:-1],good+b'unknown',good.replace(b'blob',b'tree'),
                good.replace(b' 6\n',b' 5\n'),b'0'*40+good[40:],oid+b' blob 6',
                good.replace(raw,b'other\n')):
            with patch.object(R.subprocess,'run',return_value=SimpleNamespace(stdout=data,stderr=b'')):
                with self.assertRaises(ValueError):R.batch_blobs(self.B,self.root,self.head,[('text',raw)])

    def test_git_failure_timeout_or_warning_does_not_grant_binding(self):
        for failure in (subprocess.TimeoutExpired('git',120),subprocess.CalledProcessError(1,'git')):
            with patch.object(R.subprocess,'run',side_effect=failure):
                with self.assertRaises(type(failure)):R.batch_blobs(self.B,self.root,self.head,self.members)
        with patch.object(R.subprocess,'run',return_value=SimpleNamespace(stdout=b'',stderr=b'warning')):
            with self.assertRaises(ValueError):R.batch_blobs(self.B,self.root,self.head,self.members)

    def test_binding_revalidates_payload_original_base_and_source_before_and_after(self):
        manifest=b'fixture';payload={'helper-manifest.json':manifest,'files/text':b'plain\n','helpers/empty':b''}
        contract=SimpleNamespace(BASE='f'*40,json_value=Mock(return_value={'revision':self.head,'files':{'empty':'pin'}}),
                                 validate=Mock(),source_map=lambda:{'text':'row'})
        result=R.bind_source(self.B,contract,payload,self.root,self.head,R.sha(manifest))
        self.assertEqual(result,{'head':self.head,'source_bound':True,'administrator_ready':False,'production_gate_enabled':False})
        self.assertEqual(self.B.frozen_lock.call_args_list,[unittest.mock.call(self.root,self.head,'f'*40)]*2)
        contract.validate.assert_called_once()
        with self.assertRaises(ValueError):R.bind_source(self.B,contract,payload,self.root,self.head,'0'*64)
        contract.validate.side_effect=ValueError('invalid release')
        with self.assertRaises(ValueError):R.bind_source(self.B,contract,payload,self.root,self.head,R.sha(manifest))

    def test_git_environment_timeout_and_no_shell_are_exact(self):
        native=R.subprocess.run
        with patch.object(R.subprocess,'run',wraps=native) as run:
            R.batch_blobs(self.B,self.root,self.head,self.members)
        self.assertEqual(run.call_count,1);args,kwargs=run.call_args
        self.assertEqual(args[0][-2:],['cat-file','--batch'])
        self.assertNotIn('shell',kwargs);self.assertEqual(kwargs['env'],self.env)
        self.assertEqual(kwargs['timeout'],120);self.assertTrue(kwargs['check'])


if __name__=='__main__':unittest.main()
