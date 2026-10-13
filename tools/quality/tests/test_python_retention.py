"""Keep the frozen Python oracle outside authoritative generic evaluation."""
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
QUALITY = ROOT / 'tools/quality'
INVENTORY = ROOT / 'docs/quality/gh-152/python-boundary.json'


class PythonRetentionTests(unittest.TestCase):
    def test_all_c_modules_have_unchanged_frozen_sources(self):
        original = json.loads((ROOT / 'docs/quality/gh-146/python-boundary.json').read_text())
        current = json.loads(INVENTORY.read_text())
        frozen = {r['module']: r for r in current['modules'] if r['category'] == 'C'}
        self.assertEqual(set(frozen), {r['module'] for r in original['modules']
                                       if r['category'] == 'C'})
        for name, row in frozen.items():
            with self.subTest(module=name):
                self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(),
                                 row['sha256'], 'Frozen oracle change needs reviewed evidence')
                self.assertIn('no generic release-approval path', row['final_disposition'])

    def test_required_measurement_imports_cannot_reach_generic_python_decisions(self):
        rows = json.loads(INVENTORY.read_text())['modules']
        forbidden = {Path(r['module']).stem for r in rows if r['category'] == 'C'}
        # Required legacy complexity measurement validation is the explicit mixed-role exception.
        forbidden.remove('quality_evidence')
        pending = ['ci_quality', 'ci_acceptance', 'coverage', 'critical_paths', 'critical_paths_collect',
                   'production_coverage', 'risk', 'function_risk', 'contracts',
                   'docs_consistency']
        visited = set()
        while pending:
            name = pending.pop()
            if name in visited:
                continue
            visited.add(name)
            self.assertNotIn(name, forbidden)
            tree = ast.parse((QUALITY / (name + '.py')).read_text())
            for node in ast.walk(tree):
                names = ([alias.name for alias in node.names] if isinstance(node, ast.Import)
                         else [node.module] if isinstance(node, ast.ImportFrom) else [])
                pending.extend(n for n in names if n and (QUALITY / (n + '.py')).is_file())
        self.assertIn('quality_evidence', visited)

    def test_reference_clis_reject_implicit_decision_use_before_reading_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'result.json'
            commands = {
                'policy_engine.py': [arg for name in (
                    'policy', 'evidence', 'project', 'source-root', 'artifact-root', 'expected')
                    for arg in ('--' + name, str(Path(directory) / 'missing'))],
                'project_report.py': ['check'],
            }
            for script, args in commands.items():
                with self.subTest(script=script):
                    result = subprocess.run([sys.executable, str(QUALITY / script),
                                             *args, '--output', str(output)],
                                            capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 2)
                    self.assertIn('--reference-only', result.stderr)
                    self.assertFalse(output.exists())
