"""Exercise scheduling and profile isolation without compiling Rust in unit tests."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import critical_paths_collect as collector


class ParallelCollectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.evidence = self.root / 'evidence/bundle.json'
        self.inventory = self.root / 'inventory.toml'
        self.inventory.write_text('version = 2')
        self.policy = self.root / 'policy.json'
        self.policy.write_text('{}')
        self.rows = [{'id': name, 'binary': 'fixture', 'test': name, 'platforms': ['linux'],
                      'observable': 'fails closed', 'assertions': ['assert!(failed);']}
                     for name in ('first', 'second')]
        self.barrier = threading.Barrier(2)
        self.guard = threading.Lock()
        self.commands = []
        self.active = 0
        self.peak = 0
        self.exports = []
        self.failure = None
        for name, value in [('ROOT', self.root), ('CRATE', self.root / 'crate'),
                            ('INVENTORY', self.inventory), ('POLICY', self.policy)]:
            self.enterContext(patch.object(collector, name, value))
        self.enterContext(patch.object(collector.tomllib, 'loads', return_value={'paths': self.rows}))
        self.enterContext(patch.object(collector, 'validate_inventory'))
        self.enterContext(patch.object(collector, 'metadata', return_value={'target': 'x86_64-unknown-linux-gnu'}))
        self.enterContext(patch.object(collector, 'git_sha', return_value='a' * 40))
        self.enterContext(patch.object(collector, 'require_committed_sources'))
        self.enterContext(patch.object(collector, 'source_identity', return_value={'source': 'digest'}))
        self.enterContext(patch.object(collector.subprocess, 'check_output', return_value='version'))
        self.enterContext(patch.object(collector.subprocess, 'run', side_effect=self.execute))

    def execute(self, argv, *, env, stdout, stderr, **kwargs):
        with self.guard:
            self.commands.append(argv)
        build = Path(env['CARGO_TARGET_DIR'])
        code = 0
        if 'show-env' in argv:
            stdout.write(f"CARGO_LLVM_COV_TARGET_DIR='{build}'\n")
        elif argv[1:3] == ['nextest', 'list']:
            stdout.write('{}')
            # Build/list-time counters must not appear in a test's report.
            (build / 'build-only.profraw').write_text('foreign build')
            if self.failure == 'build':
                code = 1
        elif argv[1:3] == ['nextest', 'run']:
            name = 'first' if 'test(=first)' in argv[argv.index('-E') + 1] else 'second'
            with self.guard:
                self.active += 1
                self.peak = max(self.peak, self.active)
            if self.barrier:
                self.barrier.wait(timeout=5)
            # Real show-env emits e.g. 'harness-gate-%p-%8m.profraw'; LLVM replaces
            # '%p'/'%m' (with an optional width) per process. Expand both forms to a
            # unique token so the two concurrent runs never collide in the shared root.
            pattern = env['LLVM_PROFILE_FILE']
            pattern = re.sub(r'%p', '1', pattern)
            pattern = re.sub(r'%(?:\d+)?m', '2', pattern)
            profile = Path(pattern)
            if self.failure != 'profiles':
                profile.write_text(name)
            stdout.write(name)
            with self.guard:
                self.active -= 1
            code = 1 if self.failure == name else 0
        elif 'clean' in argv:
            self.assertIn('--profraw-only', argv)
            self.assertNotIn('--workspace', argv)
            for profile in build.glob('*.profraw'):
                profile.unlink()
            if self.failure == 'clean':
                code = 1
        elif 'report' in argv:
            profiles = [p.read_text() for p in build.glob('*.profraw')]
            self.exports.append(profiles)
            Path(argv[argv.index('--output-path') + 1]).write_text(json.dumps(profiles))
            if self.failure == 'report':
                code = 1
        return subprocess.CompletedProcess(argv, code)

    def test_build_once_parallel_tests_and_disjoint_reports(self):
        collector.collect(self.evidence, jobs=2)
        self.assertEqual(self.peak, 2)
        self.assertEqual(sum(c[1:3] == ['nextest', 'list'] for c in self.commands), 1)
        self.assertEqual(self.exports, [['first'], ['second']])
        bundle = json.loads(self.evidence.read_text())
        self.assertEqual(list(bundle['runs']), ['first', 'second'])
        self.assertEqual(bundle['jobs'], 2)
        self.assertFalse(Path(bundle['build']['directory']).exists())
        for run in bundle['runs'].values():
            self.assertEqual(run['test_exit'], 0)
            self.assertEqual(run['coverage_exit'], 0)
            argv = run['commands'][0]
            self.assertIn('--binaries-metadata', argv)
            self.assertEqual(argv[argv.index('--retries') + 1], '0')
            self.assertEqual(len(list((self.evidence.parent / run['run_id'] / 'profiles').glob('*.profraw'))), 1)
        self.assertFalse((self.root / 'target/critical-path-collection.lock').exists())

    def test_serial_option_keeps_same_per_test_profiles(self):
        self.barrier = None
        collector.collect(self.evidence, jobs=1)
        self.assertEqual(self.peak, 1)
        self.assertEqual(self.exports, [['first'], ['second']])

    def test_failures_never_publish_bundle_and_keep_diagnostics(self):
        for failure in ('build', 'first', 'profiles', 'clean', 'report'):
            with self.subTest(failure=failure):
                self.failure = failure
                self.evidence = self.root / failure / 'bundle.json'
                self.evidence.parent.mkdir()
                self.evidence.write_text('stale PASS')
                with self.assertRaises(ValueError):
                    collector.collect(self.evidence, jobs=2)
                self.assertFalse(self.evidence.exists())
                self.assertTrue(list(self.evidence.parent.rglob('*.command.json')))
                self.assertFalse((self.root / 'target/critical-path-collection.lock').exists())

    def test_invalid_parallelism_does_not_start_commands(self):
        for jobs in (0, -1, 9):
            with self.subTest(jobs=jobs), self.assertRaises(ValueError):
                collector.collect(self.evidence, jobs=jobs)
        self.assertEqual(self.commands, [])

    def test_overlapping_collection_is_rejected(self):
        lock = self.root / 'target/critical-path-collection.lock'
        lock.mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, 'collection already active'):
            collector.collect(self.evidence)
        self.assertEqual(self.commands, [])
        self.assertTrue(lock.is_dir())

    def test_prune_old_builds_keeps_newest_failed_trees(self):
        build_root = self.root / 'target/critical-path-build'
        for index, name in enumerate(['oldest', 'older', 'mid', 'newest']):
            path = build_root / name
            path.mkdir(parents=True)
            os.utime(path, (index, index))
        lock = self.root / 'target/critical-path-collection.lock'
        lock.mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, 'collection already active'):
            collector.collect(self.evidence)
        self.assertEqual(sorted(p.name for p in build_root.iterdir()),
                         ['mid', 'newest', 'older'])

    def test_source_change_rejects_completed_tests(self):
        with patch.object(collector, 'source_identity', side_effect=[{'a': 'before'}, {'a': 'after'}]):
            with self.assertRaisesRegex(ValueError, 'source/commit changed'):
                collector.collect(self.evidence)
        self.assertFalse(self.evidence.exists())


if __name__ == '__main__':
    unittest.main()
