import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

here=Path(__file__).resolve().parent
cases=[]
for mode in ('concurrent', 'changed_adapter', 'missing_adapter'):
    with tempfile.TemporaryDirectory(prefix='lease-concurrent-') as tmp:
        root=Path(tmp)
        adapter=root/'adapter.py';adapter.write_bytes((here/'adapter.py').read_bytes())
        obj={'namespace':'fixture/user1','name':'name','labels':{'project':'owned'}}
        lease={'generation':'one','namespace':obj['namespace'],'target':'old-id','labels':obj['labels'],'name':obj['name'],'owner':'proven-dead','adapter':str(adapter),'digest':hashlib.sha256(adapter.read_bytes()).hexdigest()}
        (root/'lease.json').write_text(json.dumps(lease));(root/'engine.json').write_text(json.dumps({'old-id':obj,'replacement-id':obj}));(root/'mode').write_text('normal' if mode=='concurrent' else 'lost_reply')
        args=[sys.executable,'-I',str(here/'host.py'),str(root)]
        if mode=='concurrent':
            a=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            b=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            outputs=[p.communicate(timeout=5) for p in (a,b)]
            assert a.returncode==b.returncode==0,outputs
            assert sorted(x[0].strip() for x in outputs)==['already-complete','completed'],outputs
            assert len((root/'operations.log').read_text().splitlines())==1
            assert not (root/'lease.json').exists()
        else:
            first=subprocess.run(args,capture_output=True,text=True,check=True,timeout=5)
            assert first.stdout.strip()=='unknown'
            (root/'mode').write_text('normal')
            if mode=='changed_adapter': adapter.write_text(adapter.read_text()+'\n# replacement\n')
            else: adapter.unlink()
            recovery=subprocess.run(args,capture_output=True,text=True,check=True,timeout=5)
            assert recovery.stdout.strip()=='unknown'
            assert (root/'lease.json').exists()
            assert len((root/'operations.log').read_text().splitlines())==1
        assert 'replacement-id' in json.loads((root/'engine.json').read_text())
        cases.append({'case':mode,'passed':True})
(here/'concurrency-result.json').write_text(json.dumps({'cases':cases,'scope':'Linux fake runtime; cooperative resource lock and fixed adapter recovery only','source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (here/'host.py',here/'adapter.py',Path(__file__))}},indent=2)+'\n')
print('3 concurrency/artifact recovery cases passed')
