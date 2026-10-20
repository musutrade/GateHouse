import importlib.util
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('protocol_publish', Path(__file__).resolve().parents[1] / 'publish_runtime_protocol.py')
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)


class RegistryIdentity(unittest.TestCase):
    def metadata(self, body=b'package', version='0.1.0'):
        return json.dumps({'version': {'num': version, 'checksum': hashlib.sha256(body).hexdigest()}}).encode()

    def test_absent_is_not_published(self):
        with patch.object(m, 'fetch', return_value=None) as fetch:
            self.assertFalse(m.verify_existing('gatehouse-runtime-protocol', '0.1.0', b'package'))
            self.assertEqual(fetch.call_count, 1)

    def test_existing_requires_metadata_and_download(self):
        with patch.object(m, 'fetch', side_effect=[self.metadata(), b'package']) as fetch:
            self.assertTrue(m.verify_existing('gatehouse-runtime-protocol', '0.1.0', b'package'))
            self.assertEqual(fetch.call_count, 2)

    def test_conflicts_fail_closed(self):
        for responses in ([self.metadata(b'other')], [self.metadata(version='0.2.0')], [self.metadata(), b'other'], [self.metadata(), None]):
            with self.subTest(responses=responses), patch.object(m, 'fetch', side_effect=responses):
                with self.assertRaises(RuntimeError):
                    m.verify_existing('gatehouse-runtime-protocol', '0.1.0', b'package')

    def test_network_failure_is_not_absence(self):
        with patch.object(m, 'fetch', side_effect=TimeoutError):
            with self.assertRaises(TimeoutError):
                m.verify_existing('gatehouse-runtime-protocol', '0.1.0', b'package')
