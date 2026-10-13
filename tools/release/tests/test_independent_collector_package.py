"""Reject incomplete distribution bytes before any source-tree fallback."""
import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'collector_package', Path(__file__).parents[1] / 'package_independent_collectors.py')
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class MeasurerPackageTests(unittest.TestCase):
    def test_measurer_archive_name_is_exact(self):
        self.assertEqual(package.measurer_archive_name('0.3.1'),
                         'harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz')

    def test_measurer_identity_pins_cargo_name_and_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'Cargo.toml').write_text(
                '[package]\nname = "harness-gate-rust-measure"\nversion = "0.3.1"\n')
            (root / 'plugin.json').write_text(json.dumps(
                {'name': 'harness-gate-rust-measure', 'version': '0.3.1'}))
            self.assertEqual(package.measurer_identity(root)['version'], '0.3.1')
            (root / 'plugin.json').write_text(json.dumps(
                {'name': 'harness-gate-rust-measure', 'version': '0.3.2'}))
            with self.assertRaisesRegex(RuntimeError, 'version mismatch'):
                package.measurer_identity(root)
            (root / 'plugin.json').write_text(json.dumps(
                {'name': 'other-measurer', 'version': '0.3.1'}))
            with self.assertRaisesRegex(RuntimeError, 'name mismatch'):
                package.measurer_identity(root)

    def test_checked_in_measurer_identity_matches_the_risk_series(self):
        identity = package.measurer_identity(package.ROOT / 'tools/quality/rust-measure')
        self.assertEqual(identity['analyzer'], 'harness-gate-rust-measure')
        self.assertEqual(identity['version'], '0.3.1')
        self.assertEqual(identity['rule'], 'mccabe-rust-3')


class ExtractedPackageTests(unittest.TestCase):
    def test_missing_runtime_lockfile_blocks_the_archive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            payload = root / 'collector-fixture'
            payload.mkdir()
            for name in ('plugin.py', 'measure.py', 'capture.py', 'plugin.json',
                         'inventory', 'LICENSE', 'THIRD_PARTY_NOTICES.txt'):
                (payload / name).write_text('fixture')
            archive = root / 'collector-fixture.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                stream.add(payload, arcname=payload.name)
            with patch.object(package, 'run') as run:
                with self.assertRaisesRegex(RuntimeError, 'ast/Cargo.lock'):
                    package.accept_rust_archive(archive, root, root / 'consumer')
                run.assert_not_called()
