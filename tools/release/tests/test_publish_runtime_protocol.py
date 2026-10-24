import importlib.util
import hashlib
import io
import json
from pathlib import Path
import unittest
import tarfile
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
        for responses in ([self.metadata(b'other'), b'package'], [self.metadata(version='0.2.0')], [self.metadata(), b'other'], [self.metadata(), None]):
            with self.subTest(responses=responses), patch.object(m, 'fetch', side_effect=responses):
                with self.assertRaises(RuntimeError):
                    m.verify_existing('gatehouse-runtime-protocol', '0.1.0', b'package')

    def test_network_failure_is_not_absence(self):
        with patch.object(m, 'fetch', side_effect=TimeoutError):
            with self.assertRaises(TimeoutError):
                m.verify_existing('gatehouse-runtime-protocol', '0.1.0', b'package')


class RepackagedProtocol(unittest.TestCase):
    metadata = RegistryIdentity.metadata

    def crate(self, sha='a' * 40, changes=None, extra=(), omit=()):
        files = {
            '.cargo_vcs_info.json': json.dumps({'git': {'sha1': sha}, 'path_in_vcs': 'tools/harness-gate/runtime-protocol'}).encode(),
            'Cargo.toml': b'[package]\nname="gatehouse-runtime-protocol"\nversion="0.1.0"\n',
            'Cargo.toml.orig': b'original manifest',
            'Cargo.lock': b'locked dependencies',
            'src/lib.rs': b'pub struct Protocol;\n',
        }
        files.update(changes or {})
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode='w:gz') as archive:
            for name, data in [*files.items(), *extra]:
                if name in omit:
                    continue
                member = tarfile.TarInfo('gatehouse-runtime-protocol-0.1.0/' + name)
                member.size = len(data)
                member.mode = 0o644
                archive.addfile(member, io.BytesIO(data))
        return output.getvalue()

    def verify_pair(self, published, local):
        with patch.object(m, 'fetch', side_effect=[self.metadata(published), published]):
            return m.verify_existing('gatehouse-runtime-protocol', '0.1.0', local)

    def test_only_packaging_commit_can_change(self):
        published, local = self.crate(), self.crate(sha='b' * 40)
        self.assertNotEqual(hashlib.sha256(published).digest(), hashlib.sha256(local).digest())
        self.assertTrue(self.verify_pair(published, local))

    def test_source_manifest_lock_added_and_removed_files_rejected(self):
        for name in ('Cargo.toml', 'Cargo.toml.orig', 'Cargo.lock', 'src/lib.rs', 'new-file'):
            with self.subTest(name=name), self.assertRaisesRegex(RuntimeError, 'contents differ'):
                self.verify_pair(self.crate(), self.crate(changes={name: b'changed'}))
        with self.assertRaisesRegex(RuntimeError, 'contents differ'):
            self.verify_pair(self.crate(), self.crate(omit=('src/lib.rs',)))

    def test_vcs_path_dirty_and_unknown_fields_not_ignored(self):
        for stamp in (
            {'git': {'sha1': 'b' * 40}, 'path_in_vcs': 'different'},
            {'git': {'sha1': 'b' * 40, 'dirty': True}, 'path_in_vcs': 'tools/harness-gate/runtime-protocol'},
            {'git': {'sha1': 'not-a-commit'}, 'path_in_vcs': 'tools/harness-gate/runtime-protocol'},
            {'git': {'sha1': 'b' * 40}, 'path_in_vcs': 'tools/harness-gate/runtime-protocol', 'extra': True},
        ):
            with self.subTest(stamp=stamp), self.assertRaises(RuntimeError):
                self.verify_pair(self.crate(), self.crate(changes={'.cargo_vcs_info.json': json.dumps(stamp).encode()}))
        with self.assertRaisesRegex(RuntimeError, 'lacks manifest or VCS'):
            self.verify_pair(self.crate(), self.crate(omit=('.cargo_vcs_info.json',)))

    def test_archive_aliases_duplicates_and_size_limit_rejected(self):
        for name in ('../escape', 'src/../lib.rs', '/absolute', 'src/lib.rs'):
            with self.subTest(name=name), self.assertRaisesRegex(RuntimeError, 'invalid or duplicate'):
                self.verify_pair(self.crate(), self.crate(extra=((name, b'extra'),)))
        with patch.object(m, 'MAX_BYTES', 10), self.assertRaisesRegex(RuntimeError, 'exceed limit'):
            self.verify_pair(self.crate(), self.crate(sha='b' * 40))

    def test_registry_checksum_is_verified_before_content_equivalence(self):
        published, local = self.crate(), self.crate(sha='b' * 40)
        with patch.object(m, 'fetch', side_effect=[self.metadata(local), published]):
            with self.assertRaisesRegex(RuntimeError, 'registry checksum'):
                m.verify_existing('gatehouse-runtime-protocol', '0.1.0', local)
