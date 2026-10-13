"""A delegated plugin test may leave core CI only while its handoff is intact."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run_core


class CoreTestSelectionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tests = self.root / run_core.TESTS
        self.tests.mkdir(parents=True)
        self.core = self.tests / 'test_core.py'
        self.core.write_text('core')
        self.plugin = self.tests / 'test_plugin.py'
        self.plugin.write_text('plugin')
        self.source = self.root / 'tools/quality/rust-source-risk/plugin.py'
        self.source.parent.mkdir()
        self.source.write_text('implementation')
        digest = hashlib.sha256(self.plugin.read_bytes()).hexdigest()
        self.receipt = {
            'schema': 'core-plugin-test-handoff/v1',
            'repository': 'musutrade/GateProbes', 'revision': 'a' * 40,
            'ci_run': 'https://github.com/musutrade/GateProbes/actions/runs/123',
            'delegated_tests': {'test_plugin': {
                'sha256': digest, 'path': 'plugins/example/test_plugin.py',
                'command': 'python3 -m unittest test_plugin'}},
            'guarded_sources': {
                'tools/quality/tests/test_plugin.py': digest,
                'tools/quality/rust-source-risk/plugin.py': hashlib.sha256(self.source.read_bytes()).hexdigest()},
            'source_risk_step': {'guarded_root': 'tools/quality/rust-source-risk',
                                 'tracked_files': ['tools/quality/rust-source-risk/plugin.py']},
        }
        original = run_core.tracked_files
        def tracked(root, directory):
            if root == self.root:
                return sorted(path.relative_to(root).as_posix()
                              for path in (root / directory).rglob('*') if path.is_file())
            return original(root, directory)
        mock = patch.object(run_core, 'tracked_files', side_effect=tracked)
        mock.start()
        self.addCleanup(mock.stop)
        self.manifest = self.root / run_core.RECEIPT
        self.manifest.parent.mkdir(parents=True)
        self.save()

    def save(self):
        self.manifest.write_text(json.dumps(self.receipt))

    def test_new_core_tests_run_by_default(self):
        self.assertEqual(run_core.select(self.root), ['test_core'])
        (self.tests / 'test_new.py').write_text('new')
        self.assertEqual(run_core.select(self.root), ['test_core', 'test_new'])

    def test_nested_package_tests_are_selected(self):
        package = self.tests / 'nested'
        package.mkdir()
        (package / '__init__.py').write_text('')
        (package / 'test_regression.py').write_text('new')
        self.assertEqual(run_core.select(self.root), ['nested.test_regression', 'test_core'])

    def test_undiscoverable_nested_test_fails_instead_of_disappearing(self):
        package = self.tests / 'nested'
        package.mkdir()
        (package / 'test_regression.py').write_text('new')
        with self.assertRaisesRegex(ValueError, 'needs __init__.py'):
            run_core.select(self.root)

    def test_changed_plugin_source_or_test_invalidates_handoff(self):
        for path in (self.source, self.plugin):
            with self.subTest(path=path):
                previous = path.read_bytes()
                path.write_text('changed')
                with self.assertRaisesRegex(ValueError, 'handoff is stale'):
                    run_core.select(self.root)
                path.write_bytes(previous)

    def copy_accepted_core_input(self, relative):
        # Use the real receipt so an omitted dependency cannot pass this test.
        accepted = run_core.read_receipt(run_core.ROOT)['guarded_sources']
        self.assertIn(relative, accepted)
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((run_core.ROOT / relative).read_bytes())
        self.receipt['guarded_sources'][relative] = accepted[relative]
        self.save()
        self.assertEqual(run_core.select(self.root), ['test_core'])
        return path

    def test_changed_core_evidence_helpers_invalidate_project_handoff(self):
        for name in ('harness_evidence', 'project_model', 'quality_evidence', 'collector_runner'):
            relative = f'tools/quality/{name}.py'
            with self.subTest(path=relative):
                path = self.copy_accepted_core_input(relative)
                previous = path.read_bytes()
                path.write_bytes(previous + b'\n# Changed Core consumer implementation.\n')
                with self.assertRaisesRegex(ValueError, 'handoff is stale: ' + relative):
                    run_core.select(self.root)
                path.write_bytes(previous)

    def test_changed_core_evidence_schema_invalidates_project_handoff(self):
        for name in ('harness-evidence', 'project-model'):
            relative = f'tools/quality/schema/{name}.schema.json'
            with self.subTest(path=relative):
                path = self.copy_accepted_core_input(relative)
                previous = path.read_bytes()
                schema = json.loads(previous)
                schema['required'].append('new_core_required_field')
                path.write_text(json.dumps(schema))
                with self.assertRaisesRegex(ValueError, 'handoff is stale: ' + relative):
                    run_core.select(self.root)
                path.write_bytes(previous)

    def test_missing_guarded_input_fails(self):
        self.source.unlink()
        with self.assertRaisesRegex(ValueError, 'missing guarded source'):
            run_core.select(self.root)

    def test_new_plugin_test_or_source_requires_renewed_parity(self):
        for name in ('test_added.py', 'new_impl.py'):
            path = self.source.parent / name
            with self.subTest(name=name):
                path.write_text('new')
                with self.assertRaisesRegex(ValueError, 'tracked files changed'):
                    run_core.select(self.root)
                path.unlink()

    def test_source_risk_file_list_cannot_drop_digest_checks(self):
        self.receipt['source_risk_step']['tracked_files'].append('unrecorded.py')
        self.save()
        with self.assertRaisesRegex(ValueError, 'exact guarded file list'):
            run_core.select(self.root)

    def test_missing_or_malformed_receipt_fails(self):
        self.manifest.unlink()
        with self.assertRaises(OSError):
            run_core.select(self.root)
        for content in ('null', '{}', '[]', '{', '{"schema":1,"schema":2}'):
            with self.subTest(content=content):
                self.manifest.write_text(content)
                with self.assertRaises(ValueError):
                    run_core.select(self.root)

    def test_unpinned_or_wrong_owner_handoff_fails(self):
        for field, value in (('revision', 'main'), ('repository', 'other/repo'),
                             ('ci_run', 'https://example.com/123')):
            previous = self.receipt[field]
            with self.subTest(field=field):
                self.receipt[field] = value
                self.save()
                with self.assertRaises(ValueError):
                    run_core.select(self.root)
                self.receipt[field] = previous

    def test_delegation_requires_test_digest_and_execution_evidence(self):
        row = self.receipt['delegated_tests']['test_plugin']
        for field, value in (('sha256', '0' * 64), ('path', 'other/test_plugin.py'),
                             ('command', '')):
            previous = row[field]
            with self.subTest(field=field):
                row[field] = value
                self.save()
                with self.assertRaises(ValueError):
                    run_core.select(self.root)
                row[field] = previous

    def test_empty_selection_fails(self):
        self.core.unlink()
        with self.assertRaisesRegex(ValueError, 'selection is empty'):
            run_core.select(self.root)

    def test_repository_workflow_uses_guarded_selection(self):
        workflow = (run_core.ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('python3 tools/quality/tests/run_core.py', workflow)
        self.assertNotIn('cargo test --locked --manifest-path tools/quality/rust-source-risk', workflow)
        self.assertNotIn('cargo build --locked --manifest-path tools/quality/rust-source-risk', workflow)
        delegated = run_core.read_receipt(run_core.ROOT)['delegated_tests']
        for required in ('test_rust_reference', 'test_rust_equivalence', 'test_preset_packs',
                         'test_typescript_policy', 'test_collector_runner', 'test_python_retention'):
            self.assertNotIn(required, delegated)
            self.assertTrue((run_core.ROOT / run_core.TESTS / f'{required}.py').is_file())


if __name__ == '__main__':
    unittest.main()
