"""Required CI receipt integrity, topology and arbitrary metadata contracts."""
import ast
import base64
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_acceptance as acceptance


def receipt_fixture():
    matrix = {'shapes': [{'id': 'arbitrary-id'}], 'modes': ['pass'],
              'profile': 'custom-profile', 'negative_cases': ['tamper']}
    matrix_bytes = json.dumps(matrix).encode()
    direct = {'opaque-rust-decision': 'preserve exactly'}
    payload = b'{}'
    state = {'retained': {'arbitrary-producer': {'path': 'retained.json',
                                               'sha256': acceptance.digest(payload)}},
             'artifacts': {'raw.json': acceptance.digest(payload)}, 'artifact_root': 'artifacts'}
    files = {name: {'base64': base64.b64encode(data).decode(), 'sha256': acceptance.digest(data)}
             for name, data in {'direct-report.json': json.dumps(direct).encode(),
                                '.harness-gate/workflow-state.json': json.dumps(state).encode(),
                                'retained.json': payload, 'artifacts/raw.json': payload}.items()}
    case = {'shape': 'arbitrary-id', 'mode': 'pass', 'profile': 'custom-profile', 'status': 'pass',
            'direct_parity': True, 'producer_launches': 1, 'reuse_launches': 0, 'seconds': 1.0,
            'files': files, 'report': {'quality': {'project_report': direct,
                                                 'producers': {'arbitrary-producer': 'retained'}}},
            'negatives': {'tamper': {'status': 'FAIL', 'quality': {'status': 'blocked',
                                                                 'error': 'hash mismatch'}}}}
    return {'schema': 'quality-ci-acceptance/v1', 'matrix_sha256': acceptance.digest(matrix_bytes),
            'identity': {}, 'seconds': 1.0, 'cases': [case]}, matrix_bytes


class AcceptanceTests(unittest.TestCase):
    def test_arbitrary_profile_shape_and_producer_use_same_receipt_path(self):
        receipt, matrix = receipt_fixture()
        summary = acceptance.validate(receipt, matrix, {})
        self.assertEqual(summary['status'], 'pass')
        self.assertEqual(summary['cases'][0]['profile'], 'custom-profile')

    def test_incomplete_duplicate_stale_or_forged_receipts_fail_closed(self):
        receipt, matrix = receipt_fixture()
        mutations = {
            'missing': lambda r: r.update(cases=[]),
            'duplicate': lambda r: r['cases'].append(copy.deepcopy(r['cases'][0])),
            'matrix': lambda r: r.update(matrix_sha256='0' * 64),
            'identity': lambda r: r.update(identity={'GITHUB_RUN_ID': 'stale'}),
            'recollection': lambda r: r['cases'][0].update(reuse_launches=1),
            'skipped': lambda r: r['cases'][0].update(producer_launches=0),
            'negative': lambda r: r['cases'][0].update(negatives={}),
            'parity': lambda r: r['cases'][0]['report']['quality'].update(project_report={}),
            'tamper': lambda r: r['cases'][0]['files']['artifacts/raw.json'].update(base64='e30K'),
            'unsafe-path': lambda r: r['cases'][0]['files'].update({'../escape': {}}),
            'time': lambda r: r['cases'][0].update(seconds=float('nan')),
            'fresh-producer': lambda r: r['cases'][0]['report']['quality'].update(producers={'p': 'collected'}),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                changed = copy.deepcopy(receipt)
                mutate(changed)
                with self.assertRaises(ValueError):
                    acceptance.validate(changed, matrix, {})

    def test_missing_receipt_removes_stale_success(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / 'summary.json'
            output.write_text('{"status":"pass"}')
            self.assertEqual(acceptance.main(['--directory', temp]), 1)
            self.assertFalse(output.exists())

    def test_ci_keeps_single_owner_and_required_receipt_without_dispatch(self):
        workflow = (acceptance.ROOT / '.github/workflows/ci.yml').read_text()
        test_job = workflow.split('  test:\n', 1)[1].split('  test-cross-platform:\n', 1)[0]
        self.assertIn('HARNESS_GATE_CI_ACCEPTANCE:', test_job)
        self.assertIn('python3 tools/quality/ci_acceptance.py --directory', test_job)
        self.assertNotIn('continue-on-error', test_job)
        self.assertEqual(workflow.count('ci_quality.py collect'), 1)
        self.assertNotIn('cache-target: true', workflow)
        self.assertNotIn('build-once', workflow)
        tree = ast.parse(Path(acceptance.__file__).read_text())
        strings = {node.value for node in ast.walk(tree)
                   if isinstance(node, ast.Constant) and isinstance(node.value, str)}
        self.assertFalse(strings & {'rust', 'angular', 'go', 'vue', 'ecosystem', 'language', 'framework'})
        self.assertFalse(any(isinstance(node, (ast.Import, ast.ImportFrom)) and
                             'subprocess' in ast.unparse(node) for node in ast.walk(tree)))
        rust = (acceptance.ROOT / 'tools/harness-gate/src/config/quality/collectors/ci_acceptance.rs').read_text()
        for name in ('rust', 'angular', 'go', 'vue', 'ecosystem', 'framework', 'language'):
            self.assertNotIn('"' + name + '"', rust)
        # The unknown shape selects a different supported capability from data.
        matrix = json.loads(acceptance.MATRIX.read_bytes())
        unknown = next(shape for shape in matrix['shapes'] if shape['id'] == 'unknown')
        self.assertEqual(unknown['components'][0]['metadata']['ecosystem'], 'nebula-unregistered-2049')
        self.assertEqual(unknown['components'][0]['capabilities'][0]['name'], 'bundle.size')
        self.assertEqual(matrix['unsupported_capability'], 'nebula.flux')


if __name__ == '__main__':
    unittest.main()
