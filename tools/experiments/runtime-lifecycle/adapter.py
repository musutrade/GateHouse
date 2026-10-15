"""Fake engine only: never invokes Docker, Podman or a real service."""
import json
import os
from pathlib import Path
import sys
import time

root = Path(sys.argv[1])
request = json.loads(sys.stdin.readline())
mode = (root / 'mode').read_text()
engine = json.loads((root / 'engine.json').read_text())
response = {k: request[k] for k in ('protocol', 'invocation', 'key', 'generation', 'namespace', 'operation', 'target')}
if mode == 'wrong_protocol':
    response['protocol'] = 'invalid'
if mode == 'malformed':
    print('{')
    sys.exit(0)
if mode == 'overflow':
    print('x' * 20000)
    sys.exit(0)
if mode in ('tree_timeout', 'tree_cancel') and request['operation'] == 'remove':
    child = os.fork()
    if child == 0:
        time.sleep(10)
        os._exit(0)
    (root/'tree.json').write_text(json.dumps([os.getpid(), child]))
    time.sleep(10)
if mode == 'timeout_before' and request['operation'] == 'remove':
    time.sleep(10)
if request['operation'] == 'capabilities':
    response.update(outcome='completed', capabilities=['inspect'] if mode == 'missing_capability' else ['inspect', 'remove'])
elif request['operation'] == 'inspect':
    response['outcome'] = 'unavailable' if mode == 'unavailable' else ('present' if request['target'] in engine else 'absent')
    response['object'] = engine.get(request['target']) if response['outcome']=='present' else None
    if mode == 'replace_generation':
        lease = json.loads((root/'lease.json').read_text())
        lease['generation'] = 'replacement-generation'
        (root/'lease.json').write_text(json.dumps(lease))
else:
    with (root / 'operations.log').open('a') as f:
        f.write(json.dumps(request) + '\n')
    if mode == 'rejected':
        response['outcome'] = 'rejected-before-effect'
    else:
        engine.pop(request['target'], None)
        (root / 'engine.json').write_text(json.dumps(engine))
        if mode == 'lost_reply':
            os._exit(3)
        if mode == 'timeout_after':
            time.sleep(10)
        response['outcome'] = 'completed'
print(json.dumps(response))
