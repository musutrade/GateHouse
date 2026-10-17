"""Schema measurements must preserve the input and reject stale or changed bytes."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('schema_measurement',
    ROOT / 'tools/trusted_gate/deploy/check-schemas.py')
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)


class SchemaMeasurementTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        (self.source / 'schema').mkdir()
        (self.source / 'tools/harness-gate').mkdir(parents=True)
        (self.source / 'tools/harness-gate/Cargo.toml').write_text('fixture crate\n')
        for name in ('flow', 'quality'):
            (self.source / 'schema' / (name + '.schema.json')).write_text(name + '\n')
        self.git('init', '--template=')
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-m', 'schema fixture')
        self.head = self.git('rev-parse', 'HEAD')
        self.tree = self.git('rev-parse', 'HEAD:tools/harness-gate')
        self.output = self.root / 'output'
        self.core = self.root / 'core'
        self.exporter()

    def git(self, *args):
        return subprocess.check_output(['/usr/bin/git', '-C', str(self.source), *args],
            stderr=subprocess.PIPE, env=ADAPTER.environment()).decode().strip()

    def exporter(self, extra='', wrong=False):
        # A real subprocess provides controllable export bytes and failure cases.
        self.core.write_text('#!/usr/bin/python3\nfrom pathlib import Path\nimport sys\n'
            + extra + '\nname = "quality" if "--quality" in sys.argv else "flow"\n'
            'root = Path(sys.argv[sys.argv.index("--project-root") + 1])\n'
            '(root / "schema").mkdir(exist_ok=True)\n'
            '(root / "schema" / (name + ".schema.json")).write_text('
            + ('"wrong\\n"' if wrong else 'name + "\\n"') + ')\n')
        self.core.chmod(0o700)
        self.pin = ADAPTER.digest(self.core.read_bytes())

    def measure(self, **changes):
        args = dict(source=self.source, output=self.output, core=self.core,
                    core_sha256=self.pin, head=self.head, crate_tree=self.tree)
        args.update(changes)
        return ADAPTER.measure(**args)

    def test_both_schemas_are_measured_in_fresh_output_without_changing_source(self):
        report = self.measure()
        self.assertEqual(report['status'], 'matched')
        self.assertEqual([row['name'] for row in report['schemas']], ['flow', 'quality'])
        self.assertTrue(all(row['matches'] for row in report['schemas']))
        self.assertEqual(self.git('status', '--porcelain'), '')
        self.assertEqual(json.loads((self.output / 'result.json').read_text()), report)

    def test_mismatch_is_not_a_success_even_when_both_commands_exit_zero(self):
        self.exporter(wrong=True)
        report = self.measure()
        self.assertEqual(report['status'], 'mismatch')
        self.assertFalse(any(row['matches'] for row in report['schemas']))

    def test_wrong_head_crate_tree_or_core_pin_blocks_before_output_creation(self):
        for changes in ({'head': 'a' * 40}, {'crate_tree': 'b' * 40}, {'core_sha256': 'c' * 64}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.measure(**changes)
            self.assertFalse(self.output.exists())

    def test_dirty_source_and_stale_output_are_rejected(self):
        path = self.source / 'schema/flow.schema.json'
        path.write_text('different\n')
        with self.assertRaisesRegex(ValueError, 'tracked bytes'):
            self.measure()
        self.git('checkout', '--', 'schema/flow.schema.json')
        self.output.mkdir()
        (self.output / 'result.json').write_text('{"status":"matched"}\n')
        with self.assertRaises(FileExistsError):
            self.measure()
        self.assertEqual((self.output / 'result.json').read_text(), '{"status":"matched"}\n')

    def test_core_error_retains_non_success_report_and_log(self):
        self.exporter('raise SystemExit(7)')
        with self.assertRaisesRegex(ValueError, 'export failed'):
            self.measure()
        report = json.loads((self.output / 'result.json').read_text())
        self.assertEqual(report['status'], 'measurement_error')
        self.assertEqual(report['schemas'], [])
        self.assertTrue((self.output / 'flow.stderr').is_file())

    def test_timeout_keeps_a_non_success_record(self):
        actual = subprocess.run
        def timeout_core(argv, **kwargs):
            if argv[0] == str(self.core):
                raise subprocess.TimeoutExpired(argv, 30)
            return actual(argv, **kwargs)
        with patch.object(ADAPTER.subprocess, 'run', side_effect=timeout_core):
            with self.assertRaises(subprocess.TimeoutExpired):
                self.measure()
        self.assertEqual(json.loads((self.output / 'result.json').read_text())['status'],
                         'measurement_error')

    def test_hard_linked_core_input_is_rejected(self):
        os.link(self.core, self.root / 'core-alias')
        with self.assertRaisesRegex(ValueError, 'linked'):
            self.measure()
        self.assertFalse(self.output.exists())

    def test_source_mutation_during_export_is_detected(self):
        path = self.source / 'schema/flow.schema.json'
        self.exporter('Path(' + repr(str(path)) + ').write_text("changed\\n")')
        with self.assertRaises(ValueError):
            self.measure()
        self.assertEqual(json.loads((self.output / 'result.json').read_text())['status'],
                         'measurement_error')

    def test_output_cannot_overlap_source_or_follow_links(self):
        for output in (self.source, self.source / 'generated', self.root):
            with self.subTest(output=output), self.assertRaisesRegex(ValueError, 'separate'):
                self.measure(output=output)
        alias = self.root / 'alias'
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'symbolic link'):
            self.measure(output=alias / 'output')
        schema = self.source / 'schema/quality.schema.json'
        saved = self.root / 'saved-schema'
        schema.rename(saved)
        schema.symlink_to(saved)
        with self.assertRaises(ValueError):
            self.measure()

    def test_agent_git_environment_does_not_retarget_the_input(self):
        previous = {key: os.environ.get(key) for key in ('GIT_DIR', 'GIT_WORK_TREE')}
        try:
            os.environ.update(GIT_DIR='/unrelated/metadata', GIT_WORK_TREE='/unrelated/source')
            self.assertEqual(self.measure()['status'], 'matched')
        finally:
            for key, value in previous.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


if __name__ == '__main__':
    unittest.main()
