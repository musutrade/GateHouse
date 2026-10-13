"""The retained corpus must reject false PASS, absent cases and fabricated metrics."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3] / 'docs/dogfood/arc-admin'
SPEC = importlib.util.spec_from_file_location('arc_negative', ROOT / 'negative/reproduce.py')
negative = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(negative)


class ArcAdminNegativeTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name) / 'negative'
        shutil.copytree(ROOT / 'negative', self.root)
        for name in ['sources', 'import', 'quality']:
            shutil.copytree(ROOT / name, self.root.parent / name)

    def change_quality(self, case_id, update):
        path = self.root / 'evidence/quality.json'
        document = json.loads(path.read_text())
        update(next(c for c in document['cases'] if c['id'] == case_id))
        path.write_text(json.dumps(document))
        manifest_path = self.root / 'manifest.json'
        manifest = json.loads(manifest_path.read_text())
        manifest['artifacts']['evidence/quality.json'] = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest_path.write_text(json.dumps(manifest))

    def test_complete_retained_corpus(self):
        self.assertEqual(negative.verify(self.root)['quality'], 17)

    def test_rehashed_false_pass_is_rejected(self):
        self.change_quality('crash', lambda c: c['report'].update(passed=True))
        with self.assertRaises(AssertionError):
            negative.verify(self.root)

    def test_rehashed_omission_promoted_to_pass_is_rejected(self):
        self.change_quality('hook-omitted', lambda c: c['report']['quality'].update(status='pass'))
        with self.assertRaises(AssertionError):
            negative.verify(self.root)

    def test_rehashed_fabricated_angular_crap_is_rejected(self):
        self.change_quality('angular-reference-pass', lambda c:
                            c['report']['quality']['evidence'][0]['metrics'].append(
                                {'name': 'risk.crap', 'value': {'type': 'decimal', 'value': '0'}}))
        with self.assertRaises(AssertionError):
            negative.verify(self.root)

    def test_reset_debt_is_rejected(self):
        self.change_quality('crap-ratchet', lambda c:
                            next(iter(c['report']['quality']['project_report']['gates'].values()))[
                                'record']['ratchet'].update(debt='none'))
        with self.assertRaises(AssertionError):
            negative.verify(self.root)

    def test_corrupted_evidence_is_rejected(self):
        (self.root / 'evidence/commands.json').write_text('{}')
        with self.assertRaisesRegex(AssertionError, 'commands.json'):
            negative.verify(self.root)


if __name__ == '__main__':
    unittest.main()
