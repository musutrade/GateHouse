import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
results = []

def run(root):
    return subprocess.run([sys.executable, '-I', str(HERE/'host.py'), str(root)], check=True, capture_output=True, text=True, timeout=5).stdout.strip()

for mode in ('normal', 'lost_reply', 'timeout_after', 'timeout_before', 'unavailable', 'wrong_protocol', 'malformed', 'overflow', 'rejected', 'foreign', 'live_owner', 'bad_digest', 'bad_key', 'missing_capability', 'replace_generation'):
    with tempfile.TemporaryDirectory(prefix='lease-experiment-') as tmp:
        root = Path(tmp)
        obj = {'namespace': 'fixture-engine/user1', 'labels': {'project': 'fixture-only'}, 'name': 'shared-name'}
        engine = {'id-old': obj, 'id-replacement': dict(obj, labels={'project': 'other'})}
        lease = {'generation': 'gen1', 'namespace': obj['namespace'], 'target': 'id-old', 'labels': obj['labels'], 'name': obj['name'], 'owner': 'proven-dead', 'adapter': str(HERE/'adapter.py'), 'digest': hashlib.sha256((HERE/'adapter.py').read_bytes()).hexdigest()}
        if mode == 'foreign': engine['id-old'] = dict(obj, labels={'project': 'other'})
        if mode == 'live_owner': lease['owner'] = 'alive'
        if mode == 'bad_digest': lease['digest'] = '0'*64
        if mode == 'bad_key': lease['key'] = 'wrong'
        (root/'engine.json').write_text(json.dumps(engine))
        (root/'lease.json').write_text(json.dumps(lease))
        (root/'mode').write_text(mode)
        first = run(root)
        after = json.loads((root/'engine.json').read_text())
        assert after['id-replacement'] == engine['id-replacement'], mode
        calls = [json.loads(x) for x in (root/'operations.log').read_text().splitlines()] if (root/'operations.log').exists() else []
        assert all(x['target'] == 'id-old' for x in calls), mode
        if mode == 'replace_generation':
            assert json.loads((root/'lease.json').read_text())['generation']=='replacement-generation'
            assert first=='retained-generation'
        recovery = None
        if mode == 'normal':
            assert first == 'completed' and not (root/'lease.json').exists() and 'id-old' not in after and len(calls)==1
        elif mode in ('lost_reply', 'timeout_after'):
            assert first == 'unknown' and (root/'lease.json').exists() and 'id-old' not in after and len(calls)==1
            (root/'mode').write_text('unavailable')
            assert run(root) == 'retained-inspection' and (root/'lease.json').exists()
            (root/'mode').write_text('normal')
            recovery = run(root)
            assert recovery == 'reconciled' and not (root/'lease.json').exists()
            assert len((root/'operations.log').read_text().splitlines())==1
        else:
            assert (root/'lease.json').exists() and 'id-old' in after, mode
            assert len(calls) == (1 if mode=='rejected' else 0), mode
        results.append({'case':mode,'first':first,'recovery':recovery,'remove_calls':len(calls),'passed':True})
report={'scope':'Standalone Linux fake runtime subprocess experiment, not production Core integration. No real container runtime invoked.', 'cases':results,'source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (HERE/'host.py',HERE/'adapter.py',Path(__file__))},'remaining':['real engine and endpoint enforcement','startup/heartbeat/parent-exit integration','cross-platform and escaped-descendant termination','complete wire/output failure combinations','installation/upgrade/rollback acceptance']}
(HERE/'result.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'{len(results)} cases passed; remaining requirements explicitly recorded')
