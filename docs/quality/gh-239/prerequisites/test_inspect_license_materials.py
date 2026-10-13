import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

from inspect_license_materials import inspect


class InspectionTests(unittest.TestCase):
    def archive(self, root, entries):
        path = Path(root) / 'fixture.tar'
        with tarfile.open(path, 'w') as archive:
            for name, data in entries:
                member = tarfile.TarInfo(name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        return path

    def test_tamper_extra_missing_duplicate_and_traversal(self):
        expected = {'payload': {'sha256': hashlib.sha256(b'original').hexdigest()}}
        cases = [ [('payload', b'tampered')],
                  [('payload', b'original'), ('extra', b'')], [],
                  [('payload', b'original'), ('payload', b'original')],
                  [('../payload', b'original')] ]
        with tempfile.TemporaryDirectory() as root:
            for entries in cases:
                with self.subTest(entries=entries), self.assertRaises(ValueError):
                    inspect(self.archive(root, entries), expected)

    def test_verified_payload_remains_pending(self):
        with tempfile.TemporaryDirectory() as root:
            result = inspect(self.archive(root, [('payload', b'original')]),
                             {'payload': {'sha256': hashlib.sha256(b'original').hexdigest()}})
            self.assertEqual(result['payloads'][0]['redistribution'], 'pending')
            self.assertEqual(result['payload_count'], 1)


if __name__ == '__main__':
    unittest.main()
