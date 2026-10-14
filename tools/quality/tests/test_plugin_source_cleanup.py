"""Physical removal must not be hidden by the disposable CI isolation step."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import core_isolation_check as isolation


class PluginSourceCleanupTests(unittest.TestCase):
    def test_real_checkout_has_no_removed_active_source(self):
        isolation.check_source_cleanup(isolation.ROOT)

    def fixture(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for relative in isolation.REQUIRED_CORE_INPUTS:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.touch()
        inventory = root / 'docs/quality/issue-13/removal-inventory.json'
        inventory.parent.mkdir(parents=True)
        inventory.write_text(json.dumps({'schema': 'core-plugin-source-cleanup/v1',
            'paths': list(isolation.PLUGIN_IMPLEMENTATION),
            'files': {'tools/quality/tests/test_rust_native.py': {}}}))
        return root

    def test_restored_implementation_and_delegated_tests_fail(self):
        for relative in ('tools/quality/rust_native.py',
                         'tools/quality/tests/test_rust_native.py'):
            with self.subTest(path=relative):
                root = self.fixture()
                (root / relative).parent.mkdir(parents=True, exist_ok=True)
                (root / relative).touch()
                with self.assertRaisesRegex(SystemExit, 'plugin source restored'):
                    isolation.check_source_cleanup(root)

    def test_dangling_plugin_symlink_is_not_absence(self):
        root = self.fixture()
        (root / 'tools/quality/rust_native.py').symlink_to('missing.py')
        with self.assertRaisesRegex(SystemExit, 'plugin source restored'):
            isolation.check_source_cleanup(root)

    def test_removal_does_not_make_core_inputs_optional(self):
        root = self.fixture()
        (root / 'schema/flow.schema.json').unlink()
        with self.assertRaisesRegex(SystemExit, 'required core inputs are missing'):
            isolation.check_source_cleanup(root)
