"""Comparison-only encoding preserves the frozen oracle's JSON semantics."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

FIXTURE = Path(__file__).resolve().parents[1] / 'fixtures/generic-core'
sys.path.insert(0, str(FIXTURE))
import authority
import replay


class OracleComparisonTests(unittest.TestCase):
    def test_compact_equality_matches_canonical_equality(self):
        values = [
            None, False, True, 0, 1, -1, 0.0, -0.0, 1.0, 1.25, 1e-20,
            1e100, 2 ** 100, '', '1', 'true', '\n\t"\\', 'é🚀', 'e\u0301',
            [], {}, [1, 2], [2, 1], (1, 2), {'value': None},
            {'a': [True, 1, 1.0], 'z': {'text': 'é🚀'}},
            {'z': {'text': 'é🚀'}, 'a': [True, 1, 1.0]},
            {1: 'integer key'}, {'1': 'integer key'},
        ]
        for expected in values:
            for actual in values:
                with self.subTest(expected=expected, actual=actual):
                    self.assertEqual(replay.same_json(expected, actual),
                                     replay.canonical(expected) == replay.canonical(actual))

    def test_python_equal_values_keep_distinct_json_types(self):
        for expected, actual in [(True, 1), (False, 0), (1, 1.0), (0.0, -0.0)]:
            with self.subTest(expected=expected, actual=actual):
                self.assertFalse(replay.same_json(expected, actual))
                self.assertFalse(replay.oracle_matches({'value': expected}, {'value': actual}))
                self.assertEqual(authority.compare(expected, actual), [dict(
                    path='', expected=expected, actual=actual, classification='mismatch')])

    def test_invalid_values_keep_canonical_exception_classes(self):
        circular = []
        circular.append(circular)
        values = [float('nan'), float('inf'), -float('inf'), object(), {1}, b'bytes',
                  {'mixed': 1, 2: 'keys'}, '\ud800', {'nested': '\udfff'}, circular]
        for value in values:
            with self.subTest(value_type=type(value).__name__):
                try:
                    replay.canonical(value)
                except (TypeError, ValueError, UnicodeEncodeError) as error:
                    exception = type(error)
                else:
                    self.fail('Invalid-value fixture must fail canonical serialization')
                for expected, actual in [(value, None), (None, value), (value, value)]:
                    for compare in (replay.same_json, replay.oracle_matches, authority.compare):
                        with self.subTest(compare=compare.__name__):
                            with self.assertRaises(exception):
                                compare(expected, actual)

    def test_persisted_canonical_bytes_are_unchanged(self):
        self.assertEqual(replay.canonical({'z': 'é', 'a': [True, 1.0]}),
                         '{\n  "a": [\n    true,\n    1.0\n  ],\n  "z": "é"\n}\n'.encode('utf-8'))

    def test_equal_comparisons_do_not_use_pretty_serialization(self):
        expected = {'b': [True, 1.0, None], 'a': {'message': 'é'}}
        actual = {'a': {'message': 'é'}, 'b': [True, 1.0, None]}
        with patch.object(replay, 'canonical', side_effect=AssertionError('comparison is not output')):
            self.assertTrue(replay.oracle_matches(expected, actual))
            self.assertEqual(authority.compare(expected, actual), [])

    def test_only_reason_missing_file_wording_is_equivalent(self):
        expected = 'artifact/source src/a: [Errno 2] No such file or directory: /tmp/a'
        variants = [
            '[WinError 2] The system cannot find the file specified: C:/a',
            '[WinError 3] The system cannot find the path specified: C:/a',
            'The system cannot find the file specified. (os error 2)',
            'The system cannot find the path specified. (os error 3)',
            'No such file or directory (os error 2)',
        ]
        for variant in variants:
            actual = 'artifact/source src/a: ' + variant
            with self.subTest(variant=variant):
                self.assertTrue(replay.oracle_matches({'nested': [{'reason': expected}]},
                                                      {'nested': [{'reason': actual}]}))
                self.assertEqual(authority.compare({'reason': expected}, {'reason': actual}),
                                 [dict(path='/reason', expected=expected, actual=actual,
                                       classification='os-missing-file-wording')])
                self.assertFalse(replay.oracle_matches(expected, actual))
                self.assertFalse(replay.oracle_matches({'message': expected}, {'message': actual}))
                self.assertFalse(replay.oracle_matches({'reason': expected},
                                                      {'reason': actual.replace('src/a:', 'src/b:')}))
        for actual in ['artifact/source src/a: Permission denied',
                       'artifact/source src/a: [WinError 5] Access is denied']:
            with self.subTest(actual=actual):
                self.assertFalse(replay.oracle_matches({'reason': expected}, {'reason': actual}))
                self.assertEqual(authority.compare({'reason': expected}, {'reason': actual})[0]
                                 ['classification'], 'mismatch')

    def test_authority_retains_every_leaf_and_escaped_diff_path(self):
        old_reason = 'artifact/source src/a: [Errno 2] No such file or directory: /tmp/a'
        new_reason = 'artifact/source src/a: No such file or directory (os error 2)'
        expected = {'a/b~c': [True, {'reason': old_reason}, {'missing': None}], 'gone': 1}
        actual = {'a/b~c': [1, {'reason': new_reason}, {'added': None}], 'new': 2}
        self.assertEqual(authority.compare(expected, actual), [
            dict(path='/a~1b~0c/0', expected=True, actual=1, classification='mismatch'),
            dict(path='/a~1b~0c/1/reason', expected=old_reason, actual=new_reason,
                 classification='os-missing-file-wording'),
            dict(path='/a~1b~0c/2/added', expected_present=False, actual_present=True,
                 classification='mismatch'),
            dict(path='/a~1b~0c/2/missing', expected_present=True, actual_present=False,
                 classification='mismatch'),
            dict(path='/gone', expected_present=True, actual_present=False, classification='mismatch'),
            dict(path='/new', expected_present=False, actual_present=True, classification='mismatch'),
        ])

    def test_list_order_length_and_missing_null_are_mismatches(self):
        for expected, actual in [([1, 2], [2, 1]), ([1], [1, 2]), ({}, {'value': None})]:
            with self.subTest(expected=expected, actual=actual):
                self.assertFalse(replay.oracle_matches(expected, actual))
                differences = authority.compare(expected, actual)
                self.assertTrue(differences)
                self.assertTrue(all(item['classification'] == 'mismatch' for item in differences))


class OracleProfileTests(unittest.TestCase):
    def load_driver(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location('oracle_profile_driver',
                                                      FIXTURE / 'profile_oracles.py')
        driver = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(driver)
        return driver

    def run_driver(self, output, status):
        import json
        import subprocess
        driver = self.load_driver()
        def run(command, **kwargs):
            if '--worker' in command:
                directory = Path(command[command.index('--output') + 1])
                if status == 0:
                    (directory / 'timing.json').write_text(json.dumps({
                        'cpu_seconds': 1, 'wall_seconds': 2,
                        'profiled': '--profiled' in command}))
                return subprocess.CompletedProcess(command, status, 'output', 'diagnostic')
            return subprocess.CompletedProcess(command, 0)
        with patch.object(sys, 'argv', ['profile_oracles.py', '--output', str(output),
                                      '--samples', '1', '--workloads', 'policy']), \
                patch.object(driver.subprocess, 'run', side_effect=run), \
                patch.object(driver.subprocess, 'check_output', side_effect=['a' * 40, '']), \
                patch.object(driver.platform, 'platform', return_value='test-platform'), \
                patch('builtins.print'):
            driver.main()

    def test_completed_profile_separates_instrumented_and_fresh_runs(self):
        import json
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'profile'
            self.run_driver(output, 0)
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual(summary['status'], 'completed')
            self.assertFalse(summary['authoritative'])
            self.assertEqual([row['profiled'] for row in summary['workloads']['policy']],
                             [True, False])
            self.assertEqual(len(summary['interpreter_startup_seconds']), 1)
            self.assertTrue(summary['sources'])

    def test_failed_profile_retains_logs_and_propagates_failure(self):
        import json
        import subprocess
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'profile'
            with self.assertRaises(subprocess.CalledProcessError):
                self.run_driver(output, 7)
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual(summary['status'], 'failed')
            self.assertEqual(summary['failure']['returncode'], 7)
            self.assertEqual(summary['failure']['sample'], 0)
            self.assertEqual((output / 'policy/profiled/stderr.txt').read_text(), 'diagnostic')
            self.assertEqual(summary['workloads']['policy'], [])

    def test_existing_output_cannot_be_overwritten(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(FileExistsError):
                self.run_driver(Path(directory), 0)
