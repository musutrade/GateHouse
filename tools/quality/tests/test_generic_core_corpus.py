"""The frozen oracle is replayable and cannot silently accept changed goldens."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / 'tools/quality/fixtures/generic-core'
REQUIRED_FIELDS = ('purpose', 'callers', 'ci_role', 'release_impact',
                   'authoritative_status', 'final_disposition')


def validate_python_boundary(root: Path, inventory: dict) -> None:
    """Core, frozen and oracle rows must exist; plugin rows need explicit approval."""
    rows = inventory['modules']
    modules = [row['module'] for row in rows]
    if len(modules) != len(set(modules)):
        raise ValueError('duplicate module rows in the python boundary inventory')
    present = {str(path.relative_to(root)) for path in (root / 'tools/quality').glob('*.py')}
    unregistered = sorted(present - set(modules))
    if unregistered:
        raise ValueError('unregistered tools/quality python modules: ' + ', '.join(unregistered))
    for row in rows:
        if row['category'] not in 'ABCD':
            raise ValueError(f"unknown python boundary category: {row['module']}")
        for field in REQUIRED_FIELDS:
            if not row.get(field):
                raise ValueError(f"missing {field} for {row['module']}")
        if row['module'] in present:
            continue
        if not row.get('plugin_owner'):
            raise ValueError('missing required core module: ' + row['module'])


spec = importlib.util.spec_from_file_location('generic_core_replay', FIXTURE / 'replay.py')
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)


class FrozenCorpusTests(unittest.TestCase):
    def test_portable_errors_preserve_logical_paths_and_non_reason_strings(self):
        value = {'reason': "missing '/tmp/case/head/source/a.rs'",
                 'artifacts': [{'path': '/tmp/case/head/source/a.rs'}]}
        actual = replay.portable_errors(value, Path('/tmp/case'))
        self.assertEqual(actual['reason'], "missing '$CASE_ROOT/head/source/a.rs'")
        self.assertEqual(actual['artifacts'], value['artifacts'])

    def test_missing_file_wording_preserves_all_semantic_fields(self):
        expected = dict(accepted=False, reason_class='MeasurementError',
                        reason='artifact/source src/a: [Errno 2] No such file or directory: /tmp/a')
        variants = [
            '[WinError 2] The system cannot find the file specified: C:/a',
            '[WinError 3] The system cannot find the path specified: C:/a',
            'The system cannot find the file specified. (os error 2)',
            'The system cannot find the path specified. (os error 3)',
            'No such file or directory (os error 2)',
        ]
        for reason in variants:
            actual = dict(expected, reason='artifact/source src/a: ' + reason)
            self.assertTrue(replay.oracle_matches(expected, actual), reason)
            for key, value in [('accepted', True), ('reason_class', 'ModelError'),
                               ('reason', 'artifact/source src/b: ' + reason),
                               ('reason', 'artifact/source src/a: Permission denied'),
                               ('reason', 'artifact/source src/a: [WinError 5] Access is denied')]:
                self.assertFalse(replay.oracle_matches(expected, dict(actual, **{key: value})))
        self.assertFalse(replay.oracle_matches({'value': True}, {'value': 1}))
        self.assertFalse(replay.oracle_matches({'reason': expected['reason']}, {}))

    def test_every_frozen_case_matches_full_python_output(self):
        result = replay.replay()
        manifest = json.loads((FIXTURE / 'manifest.json').read_text())
        self.assertEqual(result['passed'], len(manifest['cases']))
        self.assertGreaterEqual(result['passed'], 33)
        self.assertFalse(result['authoritative'])
        states = {case['state'] for case in result['cases']}
        self.assertTrue({'pass', 'fail', 'measurement_error', 'blocked', 'rejected'} <= states)

    def test_changed_expected_output_is_rejected_before_evaluation(self):
        manifest = json.loads((FIXTURE / 'manifest.json').read_text())
        name = manifest['cases'][0]['expected']
        manifest = copy.deepcopy(manifest)
        manifest['files'] = {name: manifest['files'][name]}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'manifest.json').write_text(json.dumps(manifest))
            (root / name).write_bytes(b'{}\n')
            with self.assertRaisesRegex(ValueError, 'changed corpus file'):
                replay.replay(root)

    def test_inventory_covers_all_production_python_modules(self):
        inventory = json.loads((ROOT / 'docs/quality/gh-146/python-boundary.json').read_text())
        validate_python_boundary(ROOT, inventory)


class PythonBoundaryTests(unittest.TestCase):
    """Only per-file approved plugin absences are allowed; everything else fails."""

    def fixture(self, files, rows):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / 'tools/quality').mkdir(parents=True)
        for name, content in files.items():
            (root / 'tools/quality' / name).write_text(content)
        return root, {'modules': rows}

    @staticmethod
    def row(module, category='B', **extra):
        return {'module': module, 'category': category, 'purpose': 'p', 'callers': ['x'],
                'ci_role': 'c', 'release_impact': 'r', 'authoritative_status': 'a',
                'final_disposition': 'd', **extra}

    def test_present_core_rows_and_approved_plugin_absence_pass(self):
        root, inventory = self.fixture(
            {'core.py': 'CORE = 1\n'},
            [self.row('tools/quality/core.py', category='A')])
        validate_python_boundary(root, inventory)
        root, inventory = self.fixture(
            {'core.py': 'CORE = 1\n'},
            [self.row('tools/quality/core.py', category='A'),
             self.row('tools/quality/plugin.py', plugin_owner='musutrade/GateProbes')])
        validate_python_boundary(root, inventory)

    def test_missing_core_oracle_or_frozen_row_fails(self):
        for category in ('A', 'C', 'D'):
            with self.subTest(category=category):
                root, inventory = self.fixture(
                    {'core.py': 'CORE = 1\n'},
                    [self.row('tools/quality/core.py', category='A'),
                     self.row('tools/quality/missing.py', category=category)])
                with self.assertRaisesRegex(ValueError, 'missing required core module'):
                    validate_python_boundary(root, inventory)

    def test_unapproved_plugin_absence_fails(self):
        root, inventory = self.fixture(
            {'core.py': 'CORE = 1\n'},
            [self.row('tools/quality/core.py', category='A'),
             self.row('tools/quality/plugin.py')])
        with self.assertRaisesRegex(ValueError, 'missing required core module'):
            validate_python_boundary(root, inventory)

    def test_present_but_unregistered_module_fails(self):
        root, inventory = self.fixture(
            {'core.py': 'CORE = 1\n', 'stray.py': 'STRAY = 1\n'},
            [self.row('tools/quality/core.py', category='A')])
        with self.assertRaisesRegex(ValueError, 'unregistered'):
            validate_python_boundary(root, inventory)

    def test_duplicate_rows_fail(self):
        root, inventory = self.fixture(
            {'core.py': 'CORE = 1\n'},
            [self.row('tools/quality/core.py', category='A'),
             self.row('tools/quality/core.py', category='A')])
        with self.assertRaisesRegex(ValueError, 'duplicate module rows'):
            validate_python_boundary(root, inventory)
