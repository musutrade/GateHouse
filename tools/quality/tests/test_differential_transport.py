"""Temporary transport stays lossless without changing persisted report bytes."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import differential_transport as differential
import replay


class DifferentialTransportTests(unittest.TestCase):
    value = {'schema': 'generic-core-differential-input/v1', 'cases': [
        {'id': 'é🚀', 'integer': 10 ** 100, 'negative': -(10 ** 100),
         'ordered': [True, 1, 1.0, None, {}, {'nullable': None}],
         'text': 'line\nquote"\\tab\t'}]}

    def test_prepare_cli_keeps_values_and_uses_compact_input(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'input.json'
            with patch.object(sys, 'argv', ['differential.py', '--prepare', directory,
                                           '--output', str(output)]), \
                    patch.object(differential, 'prepare', return_value=self.value):
                self.assertEqual(differential.main(), 0)
            data = output.read_bytes()
            self.assertTrue(replay.same_json(json.loads(data), self.value))
            self.assertLess(len(data), len(replay.canonical(self.value)))

    def test_replay_cli_compacts_only_input_and_preserves_report_encoding(self):
        report = {'case_count': 1, 'mismatch_count': 0, 'extra': {'text': 'é'}}

        def run(argv):
            data = Path(argv[1]).read_bytes()
            self.assertTrue(replay.same_json(json.loads(data), self.value))
            self.assertLess(len(data), len(replay.canonical(self.value)))
            Path(argv[2]).write_text(json.dumps(report))
            return subprocess.CompletedProcess(argv, 0)

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'report.json'
            with patch.object(sys, 'argv', ['differential.py', '--rust-replay', 'unused',
                                           '--output', str(output)]), \
                    patch.object(differential, 'prepare', return_value=self.value), \
                    patch.object(differential.subprocess, 'run', side_effect=run), \
                    patch('builtins.print'):
                self.assertEqual(differential.main(), 0)
            self.assertEqual(output.read_bytes(), replay.canonical(report))

    def test_invalid_values_preserve_encoding_failures(self):
        for value, exception in [(float('nan'), ValueError), (float('inf'), ValueError),
                                 ('\ud800', UnicodeEncodeError), (object(), TypeError)]:
            with self.subTest(value_type=type(value).__name__):
                with self.assertRaises(exception):
                    differential.transport_bytes(value)


if __name__ == '__main__':
    unittest.main()
