"""Pinned official archives remain unprivileged inputs until protected import."""
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
import urllib.request
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('tool_inputs',
    ROOT / 'tools/trusted_gate/deploy/acquire-linux-tools.py')
TOOLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOLS)


class Response(io.BytesIO):
    def __init__(self, data, length=None):
        super().__init__(data)
        self.headers = {} if length is None else {'Content-Length': length}


@unittest.skipIf(os.geteuid() == 0, 'archive acquisition requires an unprivileged operator')
class ToolInputTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / 'inputs.lock.json'
        self.value = json.loads((ROOT / 'tools/trusted_gate/deploy/linux-tools.lock.json').read_text())
        self.payloads = {}
        for role, row in self.value['artifacts'].items():
            data = (role + ' official archive fixture').encode()
            self.payloads[row['url']] = data
            row['sha256'] = hashlib.sha256(data).hexdigest()
        self.save()
        self.destination = self.root / 'archives'

    def cleanup(self):
        for root, dirs, files in os.walk(self.root):
            os.chmod(root, 0o700)
        self.temp.cleanup()

    def save(self):
        self.manifest.write_text(json.dumps(self.value))
        self.pin = hashlib.sha256(self.manifest.read_bytes()).hexdigest()

    def response(self, request, timeout):
        return Response(self.payloads[request.full_url])

    def acquire(self, response=None):
        with patch.object(TOOLS.urllib.request.OpenerDirector, 'open',
                          side_effect=response or self.response):
            return TOOLS.acquire(self.manifest, self.pin, self.destination)

    def test_exact_eight_pinned_archives_retained_readonly_without_extraction(self):
        value = self.acquire()
        self.assertEqual(value['state'], 'acquired')
        self.assertEqual(len(value['archives']), 8)
        self.assertEqual(value['bytes'], sum(map(len, self.payloads.values())))
        self.assertEqual(len(list(self.destination.iterdir())), 9)
        self.assertEqual(self.destination.stat().st_mode & 0o777, 0o555)
        for row in value['archives'].values():
            self.assertEqual((self.destination / row['file']).stat().st_mode & 0o777, 0o444)
        self.assertEqual(json.loads(self.destination.with_name('archives-acquisition.json').read_text())['state'], 'acquired')

    def test_bad_digest_blocks_before_any_network_or_output(self):
        with patch.object(TOOLS.urllib.request.OpenerDirector, 'open') as network:
            with self.assertRaisesRegex(ValueError, 'reviewed digest'):
                TOOLS.acquire(self.manifest, '0' * 64, self.destination)
        network.assert_not_called()
        self.assertFalse(self.destination.exists())

    def test_unknown_versions_origins_inventory_fields_and_limits_block(self):
        original = copy.deepcopy(self.value)
        mutations = [lambda v: v['versions'].update(rust='other'),
                     lambda v: v['artifacts']['cargo'].update(url='https://example.invalid/cargo.tar.xz'),
                     lambda v: v['artifacts']['cargo'].update(max_bytes=True),
                     lambda v: v['artifacts']['cargo'].update(max_bytes=TOOLS.MAX_ARCHIVE + 1),
                     lambda v: v['artifacts']['cargo'].update(sha256='bad'),
                     lambda v: v['artifacts']['cargo'].update(command='untrusted'),
                     lambda v: v['artifacts'].pop('cargo'),
                     lambda v: v.update(unexpected='field')]
        for mutation in mutations:
            self.value = copy.deepcopy(original)
            mutation(self.value); self.save()
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                TOOLS.lock(self.manifest, self.pin)
        self.manifest.write_text('{"schema":1,"schema":2}')
        self.pin = hashlib.sha256(self.manifest.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            TOOLS.lock(self.manifest, self.pin)

    def test_corruption_retains_partial_and_blocks_reuse_without_error_secrets(self):
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.acquire(lambda request, timeout: Response(b'corrupted'))
        state = self.destination.with_name('archives-acquisition.json')
        self.assertEqual(json.loads(state.read_text())['state'], 'blocked')
        self.assertEqual(len(list(self.destination.glob('*.partial'))), 1)
        with self.assertRaisesRegex(ValueError, 'retain old or partial'):
            self.acquire()

    def test_network_error_retains_completed_files_and_redacts_diagnostics(self):
        count = 0
        def response(request, timeout):
            nonlocal count
            count += 1
            if count == 2:
                raise OSError('signed-url-or-proxy-secret')
            return self.response(request, timeout)
        with self.assertRaises(OSError):
            self.acquire(response)
        state = self.destination.with_name('archives-acquisition.json').read_text()
        self.assertNotIn('signed-url-or-proxy-secret', state)
        self.assertEqual(len(json.loads(state)['archives']), 1)
        self.assertEqual(json.loads(state)['state'], 'blocked')

    def test_header_truncation_stream_size_and_combined_budgets_block(self):
        row = next(iter(self.value['artifacts'].values()))
        data = self.payloads[row['url']]
        for length, remaining, body in (('bad', 1000, data), ('0', 1000, data),
                                        (str(len(data) + 1), 1000, data),
                                        (str(len(data)), 1, data), (None, 1, data),
                                        (None, 1000, b'')):
            output = self.root / 'partial'
            with self.subTest(length=length, remaining=remaining, body=body), patch.object(
                    TOOLS.urllib.request.OpenerDirector, 'open', return_value=Response(body, length)):
                with self.assertRaises(ValueError):
                    TOOLS.download(row, output, remaining)
            output.unlink(missing_ok=True)
        with patch.object(TOOLS, 'MAX_TOTAL', 1), self.assertRaises(ValueError):
            self.acquire()

    def test_old_record_linked_parent_and_root_execution_block(self):
        state = self.destination.with_name('archives-acquisition.json')
        state.write_bytes(b'prior evidence')
        with self.assertRaisesRegex(ValueError, 'retain'):
            self.acquire()
        self.assertEqual(state.read_bytes(), b'prior evidence')
        state.unlink()
        linked = self.root / 'linked'
        linked.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'linked'):
            TOOLS.acquire(self.manifest, self.pin, linked / 'archives')
        with patch.object(TOOLS.os, 'geteuid', return_value=0), self.assertRaisesRegex(ValueError, 'unprivileged'):
            self.acquire()

    def test_source_lock_mutation_blocks_success(self):
        original = TOOLS.download
        def changed(row, destination, remaining):
            size = original(row, destination, remaining)
            self.manifest.write_bytes(self.manifest.read_bytes() + b'\n')
            return size
        with patch.object(TOOLS, 'download', side_effect=changed), self.assertRaisesRegex(ValueError, 'lock changed'):
            self.acquire()

    def test_only_reviewed_https_release_redirects_are_allowed(self):
        redirect = TOOLS.ReviewedRedirect()
        request = urllib.request.Request(TOOLS.NEXTEST_BASE + 'asset.tar.gz')
        allowed = 'https://release-assets.githubusercontent.com/path?signature=not-logged'
        self.assertEqual(redirect.redirect_request(request, None, 302, '', {}, allowed).full_url, allowed)
        for target in ('http://github.com/path', 'https://example.invalid/asset',
                       'https://user:secret@github.com/path', 'https://github.com:444/path',
                       'https://github.com/path#fragment', 'file:///tmp/artifact'):
            with self.subTest(target=target), self.assertRaises(ValueError):
                redirect.redirect_request(request, None, 302, '', {}, target)
        request = urllib.request.Request('https://nodejs.org/dist/archive')
        with self.assertRaisesRegex(ValueError, 'redirect'):
            redirect.redirect_request(request, None, 302, '', {}, allowed)


if __name__ == '__main__':
    unittest.main()
