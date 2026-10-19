"""#133 common result boundary: unprivileged enforced network denial.

The existing Rust directory_input_test.rs removes GitHub tokens and traps
git/gh/herdr/systemctl/sudo on PATH; that is an input scrub, not an enforced
network boundary. This test reuses the real signed Core consumer fixture
(common.py) inside an available unprivileged network-denied execution boundary
(bwrap --unshare-net or unshare -Urn) and negatively probes the denial with a
local-listener control: the same probe succeeds outside the boundary and must
fail inside it.

If no unprivileged mechanism is available the test reports the concrete missing
evidence and skips; it never substitutes a sentinel for enforcement.
"""
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import common  # noqa: E402

PROBE = """\
import socket, sys
sock = socket.socket()
sock.settimeout(2)
try:
    sock.connect((sys.argv[1], int(sys.argv[2])))
    print('CONNECTED')
except OSError as error:
    print('DENIED', type(error).__name__, error.errno)
"""

CHILD = """\
import json, os, sys
from pathlib import Path
repo, work = sys.argv[1], Path(sys.argv[2])
sys.path.insert(0, repo + '/tools/consumers/tests')
import common
consumer = common.consumer()
root = work / 'root'
root.mkdir(mode=0o700)
_, verified = common.build(root)
config = common.config_for(verified)
payload = consumer.mapping(config, verified)
api = common.FakeAPI(common.remote_for(config))
confirmed = consumer.deliver(config, payload, work / 'state.db', api)
print(json.dumps({
    'uid': os.getuid(),
    'env': sorted(os.environ),
    'historical_integrity': verified['historical_integrity'],
    'status': verified['result']['status'],
    'conclusion': confirmed['conclusion'],
    'calls': [call[0] for call in api.calls],
}))
"""


def probe(mechanism, port):
    script = mechanism + [sys.executable, '-c', PROBE, '127.0.0.1', str(port)]
    return subprocess.run(script, capture_output=True, text=True, timeout=60,
                          env={'PATH': os.defpath, 'LANG': 'C.UTF-8'})


def detect_mechanism():
    """Return (label, command prefix, evidence) for unprivileged netns denial."""
    details = []
    bwrap = shutil.which('bwrap')
    if bwrap:
        prefix = [bwrap, '--unshare-net', '--dev-bind', '/', '/', '--chdir',
                  str(common.REPO), '--']
        probe_result = subprocess.run(prefix + [sys.executable, '-c', 'print("OK")'],
                                      capture_output=True, text=True, timeout=60)
        if probe_result.returncode == 0:
            digest = hashlib.sha256(Path(bwrap).read_bytes()).hexdigest()
            version = subprocess.run([bwrap, '--version'], capture_output=True,
                                     text=True).stdout.strip()
            return ('bwrap --unshare-net', prefix,
                    {'binary': bwrap, 'sha256': digest, 'version': version})
        details.append('bwrap: ' + (probe_result.stderr.strip() or 'unavailable'))
    unshare = shutil.which('unshare')
    if unshare:
        prefix = [unshare, '-Urn', '--']
        probe_result = subprocess.run(prefix + [sys.executable, '-c', 'print("OK")'],
                                      capture_output=True, text=True, timeout=60)
        if probe_result.returncode == 0:
            digest = hashlib.sha256(Path(unshare).read_bytes()).hexdigest()
            return ('unshare -Urn', prefix,
                    {'binary': unshare, 'sha256': digest})
        details.append('unshare -Urn: ' + (probe_result.stderr.strip() or 'unavailable'))
    return None, None, details


class NetworkBoundary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not common.CORE.is_file():
            raise unittest.SkipTest(f'built Core binary is required: {common.CORE}')
        cls.label, cls.prefix, evidence = detect_mechanism()
        if cls.prefix is None:
            raise unittest.SkipTest(
                'no unprivileged enforceable network denial: ' + '; '.join(evidence))
        cls.evidence = evidence
        print(json.dumps({'network_boundary': cls.label, **evidence}))

    def test_probe_discriminates_local_listener(self):
        listener = socket.socket()
        listener.bind(('127.0.0.1', 0))
        listener.listen(1)
        port = listener.getsockname()[1]
        accepted = []

        def serve():
            connection, _ = listener.accept()
            accepted.append(connection)

        server = threading.Thread(target=serve, daemon=True)
        server.start()
        try:
            outside = probe([], port)
            self.assertIn('CONNECTED', outside.stdout,
                          'control probe must succeed outside the boundary')
            inside = probe(self.prefix, port)
            self.assertIn('DENIED', inside.stdout,
                          'network denial must reject the control probe')
        finally:
            listener.close()
            for connection in accepted:
                connection.close()

    def test_real_core_result_completes_without_publisher_access(self):
        with tempfile.TemporaryDirectory(prefix='gatehouse-boundary-') as work:
            work = Path(work)
            child = work / 'child.py'
            child.write_text(CHILD)
            env = {'PATH': os.defpath, 'LANG': 'C.UTF-8', 'HOME': str(work),
                   'GATEHOUSE_CORE': str(common.CORE)}
            completed = subprocess.run(
                self.prefix + [sys.executable, str(child), str(common.REPO), str(work)],
                capture_output=True, text=True, timeout=900, env=env)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            result = json.loads(completed.stdout.strip().splitlines()[-1])
            self.assertNotEqual(result['uid'], 0, 'boundary must not run as root')
            for credential in ('GITHUB_TOKEN', 'GH_TOKEN', 'GATEHOUSE_CHECK_TOKEN',
                               'GITHUB_ACTIONS', 'ACTIONS_RUNTIME_TOKEN',
                               'ACTIONS_ID_TOKEN_REQUEST_TOKEN'):
                self.assertNotIn(credential, result['env'])
            self.assertEqual(result['historical_integrity'], 'verified')
            self.assertEqual(result['status'], 'PASS')
            self.assertEqual(result['conclusion'], 'success')
            self.assertEqual(result['calls'], ['GET', 'PATCH'])


if __name__ == '__main__':
    unittest.main()
