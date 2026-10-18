"""Execution backend changes must preserve measurement scope and fail closed."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location('coverage_collection',
                                             Path(__file__).resolve().parents[1] / 'coverage.py')
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


class CoverageRunnerTests(unittest.TestCase):
    def collect(self, directory, runner, covered=80, fail_command=None):
        output = directory / 'coverage.json'
        calls = []

        def execute(argv, **kwargs):
            self.assertTrue(kwargs['check'])
            calls.append(argv)
            if len(calls) == fail_command:
                raise subprocess.CalledProcessError(1, argv)
            if '--json' in argv:
                raw = {'data': [{'files': [
                    {'filename': f'/repo/src/{module}/mod.rs',
                     'summary': {'lines': {'covered': covered, 'count': 100}}}
                    for module in coverage.CORE_MODULES]}]}
                Path(argv[argv.index('--output-path') + 1]).write_text(json.dumps(raw))

        with patch.object(coverage.subprocess, 'run', side_effect=execute), \
                patch.object(coverage, 'metadata', side_effect=lambda **kw: {'commit': 'test', **kw}):
            coverage.run(output, 80, test_runner=runner)
        return calls, json.loads(output.read_text())

    def test_backends_preserve_scope_exports_thresholds_and_record_actual_command(self):
        with tempfile.TemporaryDirectory() as temporary:
            results = []
            for runner in ('cargo', 'nextest'):
                calls, report = self.collect(Path(temporary) / runner, runner)
                results.append(report)
                self.assertEqual(len(calls), 3)
                for argv in calls:
                    self.assertEqual(argv[argv.index('--package') + 1], 'harness-gate')
                    self.assertIn('--locked', argv)
                    self.assertNotIn('--workspace', argv)
                self.assertEqual(report['command'], subprocess.list2cmdline(calls[0]))
                self.assertEqual(report['test_runner'], runner)
                self.assertEqual([flag for argv in calls[1:] for flag in argv
                                  if flag in ('--lcov', '--cobertura')], ['--lcov', '--cobertura'])
                if runner == 'nextest':
                    self.assertEqual(calls[0][2:8],
                                     ['nextest', '--profile', 'default', '--retries', '0', '--no-fail-fast'])
            for key in ('modules', 'aggregate', 'threshold', 'source_boundaries',
                        'blocking_boundaries', 'exclusion_list_version'):
                self.assertEqual(results[0][key], results[1][key])

    def test_test_or_export_failure_cannot_become_success(self):
        for runner in ('cargo', 'nextest'):
            for number in (1, 2, 3):
                with self.subTest(runner=runner, command=number), \
                        tempfile.TemporaryDirectory() as temporary:
                    directory = Path(temporary)
                    with self.assertRaises(subprocess.CalledProcessError):
                        self.collect(directory, runner, fail_command=number)
                    self.assertFalse((directory / 'coverage.json').exists())

    def test_low_coverage_still_fails_both_backends_and_retains_report(self):
        for runner in ('cargo', 'nextest'):
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                with self.assertRaises(SystemExit):
                    self.collect(directory, runner, covered=79)
                report = json.loads((directory / 'coverage.json').read_text())
                self.assertEqual(report['aggregate']['status'], 'fail')

    def test_existing_raw_report_uses_the_same_module_boundary_without_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            raw = directory / 'coverage.raw.json'
            raw.write_text(json.dumps({'data': [{'files': [
                {'filename': f'/repo/src/{module}/mod.rs',
                 'summary': {'lines': {'covered': 80, 'count': 100}}}
                for module in coverage.CORE_MODULES]}]}))
            output = directory / 'coverage.json'
            with patch.object(coverage, 'metadata',
                              side_effect=lambda **kw: {'commit': 'test', **kw}):
                self.assertEqual(coverage.report(raw, output, 80, ['llvm-cov', 'export']), 0)
            report = json.loads(output.read_text())
            self.assertEqual(report['test_runner'], 'nextest')
            self.assertEqual(report['aggregate']['status'], 'pass')
            self.assertEqual(report['aggregate']['covered'], 80 * len(coverage.CORE_MODULES))
            self.assertEqual(report['aggregate']['executable'], 100 * len(coverage.CORE_MODULES))

    def test_unknown_backend_is_rejected_before_execution(self):
        with patch.object(coverage.subprocess, 'run') as execute:
            with self.assertRaises(ValueError):
                coverage.run(Path('unused'), 80, test_runner='unknown')
            execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
