import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

here=Path(__file__).resolve().parent
cases=[]
for mode in ('tree_timeout','tree_cancel'):
    with tempfile.TemporaryDirectory(prefix='lease-tree-') as tmp:
        root=Path(tmp);adapter=here/'adapter.py'
        obj={'namespace':'fixture/user1','name':'name','labels':{'project':'owned'}}
        lease={'generation':'one','namespace':obj['namespace'],'target':'old-id','labels':obj['labels'],'name':obj['name'],'owner':'proven-dead','adapter':str(adapter),'digest':hashlib.sha256(adapter.read_bytes()).hexdigest()}
        (root/'lease.json').write_text(json.dumps(lease));(root/'engine.json').write_text(json.dumps({'old-id':obj}));(root/'mode').write_text(mode)
        started=time.monotonic();p=subprocess.Popen([sys.executable,'-I',str(here/'host.py'),str(root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        deadline=started+3
        while not (root/'tree.json').exists() and time.monotonic()<deadline and p.poll() is None: time.sleep(.01)
        assert (root/'tree.json').exists(),'adapter did not start tree'
        # File creation can precede write completion; read only after complete JSON.
        while True:
            try: pids=json.loads((root/'tree.json').read_text());break
            except json.JSONDecodeError:
                assert time.monotonic()<deadline
                time.sleep(.01)
        if mode=='tree_cancel': os.kill(p.pid,signal.SIGTERM)
        out,err=p.communicate(timeout=4);elapsed=time.monotonic()-started
        assert p.returncode==0 and out.strip()=='unknown',(out,err,p.returncode)
        assert elapsed<3,elapsed
        assert json.loads((root/'lease.json').read_text())['state']=='unknown'
        assert 'old-id' in json.loads((root/'engine.json').read_text())
        assert not (root/'operations.log').exists()
        for pid in pids:
            stat=Path(f'/proc/{pid}/stat')
            until=time.monotonic()+1
            while stat.exists() and stat.read_text().split(') ',1)[1].split()[0]!='Z' and time.monotonic()<until: time.sleep(.01)
            assert not stat.exists() or stat.read_text().split(') ',1)[1].split()[0]=='Z',pid
        cases.append({'case':mode,'passed':True,'elapsed_seconds':elapsed,'descendants':'absent or exited zombie; no running process'})
(here/'termination-result.json').write_text(json.dumps({'cases':cases,'scope':'Linux process group termination; descendants do not escape process group','source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (here/'host.py',here/'adapter.py',Path(__file__))}},indent=2)+'\n')
print('2 timeout/cancellation process-tree cases passed')
