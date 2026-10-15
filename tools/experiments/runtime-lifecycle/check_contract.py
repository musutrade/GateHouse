import copy
import fcntl
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import host

cases=[]
request=dict(protocol='experimental-lifecycle/1',invocation='i',key='k',generation='g',namespace='n',operation='inspect',target='t')
invalid=[None,[],dict(request,outcome='absent',object={}),dict(request,outcome='present',object=None),dict(request,outcome=1,object=None),dict(request,outcome='present',object={'namespace':1,'labels':{},'name':'n'})]
for i,value in enumerate(invalid):
    try: host.validate_response(value,request)
    except ValueError: cases.append({'case':f'invalid-shape-{i}','passed':True})
    else: raise AssertionError(value)
for caps in [None,'inspect',[{}],['inspect','inspect']]:
    req=dict(request,operation='capabilities')
    try: host.validate_response(dict(req,outcome='completed',capabilities=caps),req)
    except ValueError: cases.append({'case':f'invalid-capabilities-{len(cases)}','passed':True})
    else: raise AssertionError(caps)
with tempfile.TemporaryDirectory(prefix='lease-contract-') as tmp:
    root=Path(tmp)
    with (root/'resource.lock').open('w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        for cancel in (False,True):
            start=time.monotonic();p=subprocess.Popen([sys.executable,'-I',str(Path(host.__file__)),str(root)],stdout=subprocess.PIPE,text=True)
            if cancel:
                time.sleep(.1);p.terminate()
            out,_=p.communicate(timeout=3)
            assert p.returncode==0 and out.strip()=='retained-lock'
            assert time.monotonic()-start<2
            cases.append({'case':'lock-cancel' if cancel else 'lock-timeout','passed':True})
    adapter=root/'not-reading.py';adapter.write_text('import time\ntime.sleep(10)\n')
    req=dict(request,target='x'*60000)
    start=time.monotonic()
    try: host.invoke(adapter,root,req,host.digest(adapter))
    except ValueError as e: assert str(e)=='deadline',str(e)
    else: raise AssertionError('request write did not time out')
    assert time.monotonic()-start<2
    cases.append({'case':'non-reading-input-deadline','passed':True})
root=Path(__file__).parent
(root/'contract-result.json').write_text(json.dumps({'cases':cases,'scope':'10 direct response validator cases and 3 actual subprocess input/lock cases','source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (Path(host.__file__),Path(__file__))}},indent=2)+'\n')
print(f'{len(cases)} contract/bounds cases passed')
