"""Retained reruns must preserve parity, missing-input blocking and profile identity."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3] / 'docs/dogfood/arc-admin/remediation'
SPEC = importlib.util.spec_from_file_location('arc_remediation', ROOT / 'reproduce.py')
remediation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(remediation)


class ArcAdminRemediationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / 'remediation'
        shutil.copytree(ROOT, self.root)

    def change_report(self, engine, update):
        relative = f'evidence/{engine}/test_result.json'
        path = self.root / relative
        report = json.loads(path.read_text())
        update(report)
        path.write_text(json.dumps(report))
        manifest_path = self.root / 'manifest.json'
        manifest = json.loads(manifest_path.read_text())
        manifest['artifacts'][relative] = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest_path.write_text(json.dumps(manifest))

    def test_complete_rerun(self):
        result = remediation.verify(self.root)
        self.assertEqual(result['application_results_per_engine'], 27)
        self.assertEqual(result['controlled_quality'], 17)

    def test_rehashed_missing_command_rejected(self):
        self.change_report('execution', lambda report: report['steps'].pop())
        with self.assertRaises(AssertionError):
            remediation.verify(self.root)

    def test_rehashed_false_quality_pass_rejected(self):
        self.change_report('quality', lambda report: report.update(passed=True))
        with self.assertRaises(AssertionError):
            remediation.verify(self.root)

    def test_rehashed_lost_profile_rejected(self):
        self.change_report('quality', lambda report:
                           report['quality']['participation'].update(profile=None))
        with self.assertRaises(AssertionError):
            remediation.verify(self.root)


if __name__ == '__main__':
    unittest.main()
