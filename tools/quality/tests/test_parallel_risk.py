"""Exercise the real collector with deterministic stand-ins for external tools."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality as gate
import ci_quality_parallel as parallel
from quality_common import sha256


class ParallelRiskTests(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        self.stack.enter_context(patch.object(gate, 'metadata', return_value={}))
        self.collector = gate.Collector(self.root / 'candidate', 'base', 'head', 'test')

    def prepare_tools(self, *, overlap=False, fail=False, stale=False):
        c = self.collector
        self.scope = self.stack.enter_context(patch.object(c, '_validate_risk_scope'))
        self.stack.enter_context(patch.object(c, 'measurer', return_value=Path('/measurer')))
        self.stack.enter_context(patch.object(c, 'snapshot', side_effect=lambda label, commit: self.root / label))
        self.compare = self.stack.enter_context(patch.object(c, '_compare_risk'))
        for module in (gate, parallel):
            self.stack.enter_context(patch.object(module, 'compiler_configuration', return_value={'target': 'test-target'}))
        self.calls = []
        self.barrier = threading.Barrier(2) if overlap else None
        self.failed = threading.Event()
        self.release = threading.Event()
        self.head_finished = threading.Event()

        def run(argv, *, cwd, env, stdout, stderr):
            args = list(map(str, argv))
            self.calls.append((args, env.copy()))
            def value(option):
                return Path(args[args.index(option) + 1])
            if 'prepare' in args:
                value('--manifest').write_text(json.dumps({'files': {}, 'absent_sources': []}))
            elif 'measure' in args:
                value('--output').write_text(json.dumps({
                    'manifest_sha256': sha256(value('--manifest')),
                    'llvm_sha256': 'stale' if stale else sha256(value('--llvm'))}))
            elif '--json' in args:
                is_base = value('--output-path').name.startswith('base-')
                if self.barrier:
                    self.barrier.wait(timeout=5)
                if fail:
                    if is_base:
                        self.failed.set()
                        return subprocess.CompletedProcess(args, 1)
                    if not self.release.wait(timeout=5):
                        raise AssertionError('test failed to release head')
                    self.head_finished.set()
                value('--output-path').write_text('{}')
            else:
                value('--output-path').write_text('coverage export')
            return subprocess.CompletedProcess(args, 0)
        self.stack.enter_context(patch.object(gate.subprocess, 'run', side_effect=run))

    def test_default_and_explicit_environment_values_in_fresh_process(self):
        # Avoid reloading a shared module and leaking its globals across tests.
        for value, expected in ((None, 'False'), ('0', 'False'), ('1', 'True')):
            env = dict(os.environ)
            env.pop('CI_PARALLEL_RISK', None)
            if value is not None:
                env['CI_PARALLEL_RISK'] = value
            result = subprocess.check_output(
                [sys.executable, '-B', '-c', 'import ci_quality; print(ci_quality.PARALLEL_RISK)'],
                cwd=Path(gate.__file__).parent, env=env, text=True)
            self.assertEqual(result.strip(), expected)

    def test_actual_routing(self):
        for enabled in (False, True):
            with self.subTest(enabled=enabled), patch.object(gate, 'PARALLEL_RISK', enabled), \
                    patch.object(self.collector, '_risk_sequential') as serial, \
                    patch.object(parallel, 'risk_parallel') as concurrent:
                self.collector.risk()
                self.assertEqual(serial.call_count, int(not enabled))
                self.assertEqual(concurrent.call_count, int(enabled))

    def test_parallel_overlap_and_clean_environment_reach_actual_commands(self):
        polluted = {key: 'external-pollution' for key in (
            'LLVM_PROFILE_FILE', 'CARGO_BUILD_TARGET', 'CARGO_LLVM_COV_TARGET_DIR',
            'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')}
        with patch.dict(os.environ, polluted):
            self.collector = gate.Collector(self.root / 'clean', 'base', 'head', 'test')
            original = self.collector.environment.copy()
            self.prepare_tools(overlap=True)
            with patch.object(gate, 'PARALLEL_RISK', True):
                self.collector.risk()
        self.scope.assert_called_once_with('base', 'head')
        self.compare.assert_called_once_with('base', 'head')
        self.assertEqual(len(self.calls), 10)
        target_dirs = set()
        for args, env in self.calls:
            self.assertTrue(set(polluted).isdisjoint(env))
            label = 'base' if any('base-' in arg for arg in args) else 'head'
            target = str(self.collector.directory / 'build' / label)
            self.assertEqual(env['CARGO_TARGET_DIR'], target)
            target_dirs.add(target)
        self.assertEqual(len(target_dirs), 2)
        self.assertEqual(self.collector.environment, original)
        report = json.loads((self.collector.directory / 'candidate.json').read_text())
        self.assertEqual(len(report['commands']), 10)
        self.assertEqual(len({row['name'] for row in report['commands']}), 10)

    def test_sequential_reuses_original_build_directory_and_shared_checks(self):
        self.prepare_tools()
        with patch.object(gate, 'PARALLEL_RISK', False):
            self.collector.risk()
        self.scope.assert_called_once_with('base', 'head')
        self.compare.assert_called_once_with('base', 'head')
        self.assertEqual(len(self.calls), 10)
        self.assertTrue(all(env['CARGO_TARGET_DIR'] == str(self.collector.directory / 'build')
                            for _, env in self.calls))
        names = [row['name'] for row in self.collector.report['commands']]
        self.assertLess(names.index('base-measure'), names.index('head-prepare'))

    def test_failure_waits_for_head_skips_compare_and_persists_failure(self):
        self.prepare_tools(overlap=True, fail=True)
        c = self.collector
        with patch.object(gate, 'PARALLEL_RISK', True), patch.object(gate, 'STAGES', ('risk',)), \
                patch.object(gate, 'git_sha', return_value='head'), \
                patch.object(gate, 'require_committed_sources'), ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(c.collect)
            try:
                self.assertTrue(self.failed.wait(timeout=5))
                self.assertFalse(future.done(), 'failed collector returned while head was active')
            finally:
                self.release.set()
            with self.assertRaisesRegex(ValueError, 'failed, cancelled, skipped or incomplete'):
                future.result(timeout=5)
        self.assertTrue(self.head_finished.is_set())
        self.compare.assert_not_called()
        report = json.loads((c.directory / 'candidate.json').read_text())
        self.assertEqual(report['stages']['risk']['status'], 'failure')
        self.assertIn('base-coverage exited 1', report['stages']['risk']['error'])
        self.assertIn('head-measure', [row['name'] for row in report['commands']])

    def test_stale_profiles_rejected_by_both_modes(self):
        self.prepare_tools(stale=True)
        for mode in (False, True):
            with self.subTest(parallel=mode), patch.object(gate, 'PARALLEL_RISK', mode):
                with self.assertRaisesRegex(ValueError, 'stale risk profiles'):
                    self.collector.risk()
        self.compare.assert_not_called()

    def test_concurrent_real_commands_persist_all_records(self):
        c = self.collector
        with ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(c.command, f'child-{i}',
                       [sys.executable, '-c', f'print({i}); raise SystemExit({i % 2})'],
                       allowed=(0, 1)) for i in range(12)]
            for future in futures:
                future.result(timeout=10)
        report = json.loads((c.directory / 'candidate.json').read_text())
        records = {row['name']: row for row in report['commands']}
        self.assertEqual(len(records), 12)
        for i in range(12):
            self.assertEqual(records[f'child-{i}']['exit_code'], i % 2)
            self.assertGreaterEqual(records[f'child-{i}']['seconds'], 0)
            self.assertEqual((c.directory / f'child-{i}.log').read_text().strip(), str(i))


if __name__ == '__main__':
    unittest.main()
