"""Linux-only protocol design experiment, not Core implementation or certification."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time
import uuid


CANCELLED = False


def cancel(_signal, _frame):
    global CANCELLED
    CANCELLED = True


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def strict(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate key')
        result[key] = value
    return result


def save(path, value):
    tmp = path.with_suffix('.tmp')
    with tmp.open('w') as f:
        json.dump(value, f)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    fd = os.open(path.parent, os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def same_generation(marker, lease):
    return marker.exists() and json.loads(marker.read_text())['generation'] == lease['generation']


def finish(marker, lease):
    if not same_generation(marker, lease):
        raise ValueError('generation changed')
    marker.unlink()
    fd = os.open(marker.parent, os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def validate_response(response, request):
    op = request['operation']
    extra = {'outcome', 'object'} if op == 'inspect' else ({'outcome', 'capabilities'} if op == 'capabilities' else {'outcome'})
    if type(response) is not dict or set(response) != set(request) | extra:
        raise ValueError('response shape')
    if any(type(response[k]) is not str or response[k] != v for k, v in request.items()):
        raise ValueError('response identity')
    outcome = response['outcome']
    if type(outcome) is not str:
        raise ValueError('outcome type')
    if op == 'capabilities':
        caps = response['capabilities']
        if outcome != 'completed' or type(caps) is not list or any(type(x) is not str for x in caps) or len(set(caps)) != len(caps):
            raise ValueError('capabilities shape')
    elif op == 'inspect':
        obj = response['object']
        if outcome not in ('present', 'absent', 'unavailable'):
            raise ValueError('inspect outcome')
        if outcome != 'present':
            if obj is not None:
                raise ValueError('absent/unavailable object')
        elif type(obj) is not dict or set(obj) != {'namespace', 'labels', 'name'} or any(type(obj[k]) is not str or not obj[k] for k in ('namespace', 'name')) or type(obj['labels']) is not dict or any(type(k) is not str or type(v) is not str for k,v in obj['labels'].items()):
            raise ValueError('present object')
    elif outcome not in ('completed', 'rejected-before-effect', 'unknown'):
        raise ValueError('remove outcome')
    return response


def invoke(adapter, root, request, expected_digest):
    if CANCELLED:
        raise ValueError('cancelled')
    if digest(adapter) != expected_digest:
        raise ValueError('adapter changed')
    payload = json.dumps(request).encode() + b'\n'
    if len(payload) > 65536:
        raise ValueError('request limit')
    deadline = time.monotonic() + 1.0
    proc = subprocess.Popen([sys.executable, '-I', str(adapter), str(root)], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
                            env={'PATH': '/usr/bin:/bin', 'LANG': 'C'})
    streams = {proc.stdout: bytearray(), proc.stderr: bytearray()}
    selector = selectors.DefaultSelector()
    try:
        os.set_blocking(proc.stdin.fileno(), False)
        selector.register(proc.stdin, selectors.EVENT_WRITE)
        sent = 0
        for stream in streams:
            selector.register(stream, selectors.EVENT_READ)
        while selector.get_map():
            if CANCELLED:
                raise ValueError('cancelled')
            if time.monotonic() >= deadline:
                raise ValueError('deadline')
            for key, _ in selector.select(min(.05, max(0, deadline-time.monotonic()))):
                if key.fileobj is proc.stdin:
                    try:
                        sent += os.write(key.fd, payload[sent:])
                    except BlockingIOError:
                        continue
                    if sent == len(payload):
                        selector.unregister(proc.stdin)
                        proc.stdin.close()
                    continue
                chunk = os.read(key.fd, 4096)
                if not chunk:
                    selector.unregister(key.fileobj)
                else:
                    streams[key.fileobj].extend(chunk)
                    if len(streams[key.fileobj]) > 8192 or sum(map(len, streams.values())) > 12288:
                        raise ValueError('output limit')
        code = proc.wait(timeout=max(.001, deadline-time.monotonic()))
        if code or digest(adapter) != expected_digest:
            raise ValueError('exit or changed adapter')
        response = json.loads(streams[proc.stdout], object_pairs_hook=strict)
        return validate_response(response, request)
    finally:
        # Kill even if the direct child exited: descendants can retain pipes.
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait(timeout=1)
        selector.close()
        proc.stdin.close()
        for stream in streams:
            stream.close()


def clean(root):
    marker = root / 'lease.json'
    with (root / 'resource.lock').open('a') as lock:
        lock_deadline = time.monotonic() + 1.0
        while True:
            if CANCELLED or time.monotonic() >= lock_deadline:
                return 'retained-lock'
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                time.sleep(.01)
        if not marker.exists():
            return 'already-complete'
        lease = json.loads(marker.read_text())
        # These owner facts are fixture inputs, not real PID or heartbeat validation.
        if lease['owner'] != 'proven-dead':
            return 'retained-owner'
        adapter = Path(lease['adapter'])
        binding = {k: lease[k] for k in ('generation', 'namespace', 'target')}
        key_binding = dict(binding, adapter=lease['adapter'], digest=lease['digest'], labels=lease['labels'], name=lease['name'])
        key = hashlib.sha256(json.dumps(key_binding, sort_keys=True).encode() + b'remove').hexdigest()
        if lease.get('key', key) != key:
            return 'retained-key-mismatch'
        lease['key'] = key
        lease.setdefault('state', 'prepared')
        save(marker, lease)
        request = dict(binding, key=key, protocol='experimental-lifecycle/1', invocation=uuid.uuid4().hex, operation='capabilities')
        try:
            preflight = invoke(adapter, root, request, lease['digest'])
            if preflight['outcome'] != 'completed' or set(preflight['capabilities']) != {'inspect', 'remove'}:
                return 'retained-capability'
            request.update(operation='inspect', invocation=uuid.uuid4().hex)
            response = invoke(adapter, root, request, lease['digest'])
            if response['outcome'] == 'absent' and lease['state'] in ('remove-pending', 'unknown'):
                finish(marker, lease)
                return 'reconciled'
            expected = {'namespace': lease['namespace'], 'labels': lease['labels'], 'name': lease['name']}
            if response['outcome'] != 'present' or response['object'] != expected:
                return 'retained-inspection'
            if not same_generation(marker, lease):
                return 'retained-generation'
            prior_state = lease['state']
            lease['state'] = 'remove-pending'
            save(marker, lease)
            request.update(operation='remove', invocation=uuid.uuid4().hex)
            response = invoke(adapter, root, request, lease['digest'])
            if response['outcome'] == 'rejected-before-effect':
                if not same_generation(marker, lease):
                    return 'retained-generation'
                lease['state'] = 'unknown' if prior_state in ('unknown', 'remove-pending') else 'prepared'
                save(marker, lease)
                return 'retained-rejected'
            if response['outcome'] == 'completed':
                finish(marker, lease)
                return 'completed'
        except (ValueError, OSError, subprocess.TimeoutExpired):
            pass
        if not same_generation(marker, lease):
            return 'retained-generation'
        if lease['state'] not in ('remove-pending', 'unknown'):
            return 'retained-before-remove'
        lease['state'] = 'unknown'
        save(marker, lease)
        return 'unknown'


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    print(clean(Path(sys.argv[1])))
