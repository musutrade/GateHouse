"""Frozen collector output for Core policy tests; producer parity lives in GateProbes.

No TypeScript parser/adapter runs here. Generic validators still verify the real
retained source and artifact bytes, context and series on every policy evaluation.
"""
import copy
import hashlib
import io
import json
import os
import sys
from pathlib import Path
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'fixtures/core-typescript-policy.json'


def read():
    return json.loads(FIXTURE.read_text())


class PolicyFixture:
    def __init__(self, test):
        temporary = tempfile.TemporaryDirectory()
        test.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.document = read()
        # Read generic evidence by digest. No plugin source/fixture checkout is
        # required: the native bytes are a member of an immutable data release.
        sys.path.insert(0, str(ROOT.parents[1] / 'tools/maintenance'))
        import test_data
        manifest = test_data.read_manifest(ROOT.parents[1] / 'test-data/manifest.json')
        member = 'tools/quality/fixtures/typescript-angular/evidence/native.tar.gz'
        row = next(row for row in manifest['archives'] if member in row['files'])
        test.assertEqual(row['files'][member]['sha256'], self.document['native_sha256'])
        cache = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'gatehouse-test-data'
        archive_path = test_data.fetch_archive(row, cache, offline=False)
        with tarfile.open(archive_path) as archive:
            entry = archive.getmember(member)
            test.assertTrue(entry.isfile())
            test.assertEqual(entry.size, row['files'][member]['size'])
            native = archive.extractfile(entry).read()
        corpus = ROOT / 'fixtures/generic-core'
        blobs = corpus / 'artifacts.tar.gz'
        test.assertEqual(hashlib.sha256(blobs.read_bytes()).hexdigest(),
                         json.loads((corpus / 'manifest.json').read_text())['files']['artifacts.tar.gz'])
        with tarfile.open(blobs) as archive:
            entry = archive.getmember(self.document['index_sha256'])
            test.assertTrue(entry.isfile())
            index = archive.extractfile(entry).read()
        for name, payload in [('native', native), ('index', index)]:
            test.assertEqual(hashlib.sha256(payload).hexdigest(), self.document[name + '_sha256'])
        self.contexts = {}
        for label in ('head', 'base'):
            source, output = self.root / (label + '-sources'), self.root / (label + '-output')
            source.mkdir(); output.mkdir()
            with tarfile.open(fileobj=io.BytesIO(native), mode='r:gz') as archive:
                for member in archive.getmembers():
                    if member.name.startswith('sources/'):
                        relative = Path(member.name.removeprefix('sources/'))
                        test.assertFalse(relative.is_absolute() or '..' in relative.parts)
                        test.assertTrue(member.isfile())
                        path = source / relative
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(archive.extractfile(member).read())
            row = self.document[label]
            payloads = {'native.tar.gz': native, 'source-index.json': index,
                        'receipt.json': row['receipt'].encode()}
            for record in row['records']:
                for artifact in record['artifacts']:
                    relative = Path(artifact['path'])
                    test.assertFalse(relative.is_absolute() or '..' in relative.parts)
                    payload = payloads[relative.name]
                    test.assertEqual(hashlib.sha256(payload).hexdigest(), artifact['sha256'])
                    test.assertEqual(len(payload), artifact['bytes'])
                    path = output / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(payload)
            self.contexts[label] = dict(project=copy.deepcopy(self.document['project']),
                source_root=source, artifact_root=output, expected=copy.deepcopy(row['context']))
