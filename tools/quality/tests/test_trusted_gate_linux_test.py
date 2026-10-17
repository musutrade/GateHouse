"""Tool exits alone cannot manufacture a complete Linux Test measurement."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('linux_test_job',
    ROOT / 'tools/trusted_gate/deploy/run-linux-test.py')
JOB = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(JOB)


@unittest.skipUnless(sys.platform == 'linux' and os.geteuid() != 0,
                     'Linux Test adapter requires a real unprivileged Linux caller')
class LinuxTestJobTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.plugin = self.root / 'plugin'
        (self.plugin / 'tools/onboarding').mkdir(parents=True)
        (self.plugin / 'tools/onboarding/acceptance.py').write_text('# fixture\n')
        self.output = self.root / 'job'
        self.environment = {'PATH': '/usr/bin:/bin', 'CARGO_HOME': str(self.root / 'cargo'),
                            'CARGO_TARGET_DIR': str(self.root / 'target')}
        self.plugin_pin = JOB.directory_digest(self.plugin)
        self.args = [self.root / 'source', self.root / 'history', self.root / 'archives',
                     self.output, 'a' * 40, 'b' * 40, 'c' * 64,
                     self.plugin, '12345']

    def fake_prepare(self, source, history, destination, *args):
        destination.mkdir()
        (destination / '.github/workflows').mkdir(parents=True)
        (destination / '.github/workflows/ci.yml').write_bytes((ROOT / '.github/workflows/ci.yml').read_bytes())
        return {'state': 'prepared'}

    def invoke(self, execution):
        with patch.dict(os.environ, self.environment, clear=True), \
             patch.object(JOB, 'ONBOARDING_SHA256', self.plugin_pin), \
             patch.object(JOB.INPUTS, 'prepare', side_effect=self.fake_prepare), \
             patch.object(JOB.subprocess, 'run', side_effect=execution):
            return JOB.run(*self.args)

    def test_zero_tool_exits_without_actual_rust_export_remain_blocked(self):
        with self.assertRaises(FileNotFoundError):
            self.invoke(lambda *args, **kwargs: SimpleNamespace(returncode=0))
        report = json.loads((self.output / 'result.json').read_text())
        self.assertEqual(report['state'], 'blocked')
        self.assertEqual(len(report['commands']), 4)
        self.assertTrue(all(row['exit_code'] == 0 for row in report['commands']))

    def test_first_failed_command_stops_without_running_other_checks(self):
        calls = []
        def execute(argv, **kwargs):
            calls.append(argv)
            return SimpleNamespace(returncode=7 if argv[0:2] == ['cargo', 'nextest'] else 0)
        with self.assertRaisesRegex(ValueError, 'command failed: nextest'):
            self.invoke(execute)
        self.assertEqual(len(calls), 2)
        report = json.loads((self.output / 'result.json').read_text())
        self.assertEqual(report['state'], 'blocked')
        self.assertEqual(report['commands'][-1]['exit_code'], 7)

    def test_timeout_retains_a_blocked_record_without_retry(self):
        def timeout(argv, **kwargs):
            raise subprocess.TimeoutExpired(argv, 1200)
        with self.assertRaises(subprocess.TimeoutExpired):
            self.invoke(timeout)
        report = json.loads((self.output / 'result.json').read_text())
        self.assertEqual(report['state'], 'blocked')
        self.assertEqual(len(report['commands']), 1)
        self.assertEqual(report['commands'][0]['state'], 'running')

    def test_credentials_and_agent_configuration_are_not_forwarded(self):
        supplied = {**self.environment, 'GH_TOKEN': 'fixture-value', 'GIT_DIR': '/wrong',
                    'PYTHONPATH': '/wrong', 'LD_PRELOAD': '/wrong'}
        env = JOB.clean_environment(self.output, 'a' * 40, '12345', supplied)
        self.assertFalse(set(env) & {'GH_TOKEN', 'GIT_DIR', 'PYTHONPATH', 'LD_PRELOAD'})
        self.assertEqual(env['CARGO_NET_OFFLINE'], 'true')
        self.assertEqual(env['HOME'], str(self.output / 'home'))

    def test_compiler_overrides_and_unpinned_onboarding_fail_before_creation(self):
        for key in ('RUSTFLAGS', 'CARGO_PROFILE_TEST_OPT_LEVEL', 'CARGO_BUILD_TARGET',
                    'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS'):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'compiler/profile'):
                JOB.clean_environment(self.output, 'a' * 40, '12345', {**self.environment, key: 'override'})
        (self.plugin / 'tools/onboarding/acceptance.py').write_text('# changed\n')
        with self.assertRaisesRegex(ValueError, 'reviewed pin'):
            self.invoke(lambda *args, **kwargs: SimpleNamespace(returncode=0))
        self.assertFalse(self.output.exists())

    def test_compiled_cargo_hardlinks_are_snapshotted_without_relaxing_input_guards(self):
        core, snapshot = self.root / 'compiled-core', self.root / 'snapshot'
        core.write_bytes(b'compiled fixture')
        os.link(core, self.root / 'deps-executable')
        value = JOB.built_core_digest(core, snapshot)
        self.assertEqual(value, JOB.INPUTS.digest(core.read_bytes()))
        self.assertEqual(snapshot.stat().st_nlink, 1)
        self.assertTrue(snapshot.stat().st_mode & 0o100)
        with self.assertRaisesRegex(ValueError, 'linked'):
            JOB.INPUTS.regular(core)
        core.write_bytes(b'changed compiled fixture')
        self.assertNotEqual(JOB.built_core_digest(core), value)
        self.assertEqual(snapshot.read_bytes(), b'compiled fixture')

    def test_onboarding_symlink_cannot_escape_its_inventory(self):
        (self.plugin / 'external').symlink_to(self.root / 'other', target_is_directory=True)
        with self.assertRaises(ValueError):
            JOB.directory_digest(self.plugin)

    def test_readonly_onboarding_input_gets_an_exact_writable_disposable_copy(self):
        catalog = self.plugin / 'products/onboarding/products.json'
        catalog.parent.mkdir(parents=True)
        catalog.write_text('{"fixture":true}\n')
        executable = self.plugin / 'fixture.sh'
        executable.write_text('#!/bin/sh\nexit 0\n')
        executable.chmod(0o555)
        pin = JOB.directory_digest(self.plugin)
        for root, directories, names in os.walk(self.plugin):
            for name in names:
                path = Path(root) / name
                path.chmod(0o555 if path.stat().st_mode & 0o111 else 0o444)
            Path(root).chmod(0o555)
        try:
            destination = self.root / 'onboarding-copy'
            with patch.object(JOB, 'ONBOARDING_SHA256', pin):
                JOB.copy_onboarding(self.plugin, destination)
            self.assertEqual(JOB.directory_digest(destination), pin)
            derived = self.root / 'upgrade-source'
            shutil.copytree(destination / 'products/onboarding', derived)
            (derived / 'products.json').write_text('{"upgraded":true}\n')
            self.assertEqual(JOB.directory_digest(self.plugin), pin)
            self.assertEqual(catalog.read_text(), '{"fixture":true}\n')
            self.assertTrue((destination / 'fixture.sh').stat().st_mode & 0o100)
            self.assertTrue((destination / 'products/onboarding/products.json').stat().st_mode & 0o200)
        finally:
            for root, directories, names in os.walk(self.plugin):
                Path(root).chmod(0o700)

    def test_workflow_changes_require_parity_review_before_any_test_command(self):
        original = self.fake_prepare
        def changed_workflow(*args):
            result = original(*args)
            (args[2] / '.github/workflows/ci.yml').write_text('changed workload\n')
            return result
        with patch.object(self, 'fake_prepare', side_effect=changed_workflow):
            with self.assertRaisesRegex(ValueError, 'workflow changed'):
                self.invoke(lambda *args, **kwargs: SimpleNamespace(returncode=0))
        report = json.loads((self.output / 'result.json').read_text())
        self.assertEqual(report['state'], 'blocked')
        self.assertEqual(report['commands'], [])
