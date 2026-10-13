"""Release sharding and fail-closed publication; no GitHub mutations."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_user_installer as builder
import collector_assets as assets
import publish_user_installer as publisher


class PublicationTest(unittest.TestCase):
    def exercise(self, corrupt=False):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            objects = {}
            for index in range(901):
                digest = f'{index:064x}'
                objects[digest] = {'name': digest + '.gz'}
                (output / (digest + '.gz')).write_bytes(b'object')
            (output / 'install-rust.sh').write_bytes(b'installer')
            descriptor = {'objects': objects}
            locations = builder.object_locations(descriptor, 'https://example.test/installer-v1')
            catalog = {'components': descriptor, 'object_base_urls': locations}
            uploads, edits, pages_read = {}, [], []

            def run(command, check):
                self.assertTrue(check)
                if command[2] == 'create':
                    paths = command[command.index('--notes-file') + 2:]
                    uploads[command[3]] = [{'name': Path(p).name, 'digest': 'sha256:' + assets.sha(Path(p))}
                                           for p in paths]
                elif command[2] == 'edit':
                    edits.append(command[3])

            def read(command):
                endpoint = command[-1]
                if '/tags/' in endpoint:
                    return json.dumps({'id': endpoint.rsplit('/', 1)[1], 'draft': True})
                self.assertIn('--paginate', command)
                self.assertIn('--slurp', command)
                tag = endpoint.split('/releases/')[1].split('/')[0]
                rows = uploads[tag]
                pages_read.append(tag)
                if corrupt and tag.endswith('objects-2'):
                    rows = [dict(rows[0], digest='sha256:' + 'f' * 64)]
                return json.dumps([rows[i:i + 100] for i in range(0, len(rows), 100)])

            with patch.object(publisher.production, 'create_or_verify_tag'), \
                    patch.object(publisher.subprocess, 'run', side_effect=run), \
                    patch.object(publisher.subprocess, 'check_output', side_effect=read):
                if corrupt:
                    with self.assertRaisesRegex(ValueError, 'uploaded installer assets differ'):
                        publisher.publish_assets(output, catalog, 'installer-v1', 'a' * 40, output / 'notes')
                    self.assertEqual(edits, [])
                else:
                    result = publisher.publish_assets(output, catalog, 'installer-v1', 'a' * 40, output / 'notes')
                    self.assertEqual(sorted(map(len, result.values())), [1, 1, 900])
                    self.assertEqual(edits[-1], 'installer-v1')
                    self.assertEqual(set(pages_read), set(result))
                    self.assertEqual(set(edits[:-1]), {'installer-v1-objects-1', 'installer-v1-objects-2'})

    def test_shards_and_all_paginated_assets_verified_before_installer_publication(self):
        self.exercise()

    def test_failed_shard_verification_keeps_all_releases_draft(self):
        self.exercise(corrupt=True)


if __name__ == '__main__':
    unittest.main()
