"""The core risk gate consumes a pinned, verifiable measurer release artifact."""
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ci_quality

ROOT = Path(__file__).resolve().parents[3]


class MeasurerPinTests(unittest.TestCase):
    @staticmethod
    def cache() -> Path:
        return Path(os.environ.get('HARNESS_GATE_MEASURER_CACHE', ROOT / 'target/rust-measure-release'))

    @staticmethod
    def crafted_archive(pin: dict, members: dict[str, bytes]) -> Path:
        directory = Path(tempfile.mkdtemp(prefix='measurer-pin-'))
        archive = directory / pin['asset']
        with tarfile.open(archive, 'w:gz') as tar:
            for name, payload in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(payload)
                info.mode = 0o755
                tar.addfile(info, io.BytesIO(payload))
        return archive

    def test_released_artifact_identity_matches_the_pin_and_the_actual_binary(self):
        pin = ci_quality.load_measurer_pin()
        binary = ci_quality.ensure_measurer(self.cache())
        released = json.loads((binary.parent / 'plugin.json').read_text())
        for field in ('analyzer', 'version', 'rule', 'executable'):
            self.assertEqual(pin[field], released[field], field)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'identity.rs'
            source.write_text('fn main() {}\n')
            result = subprocess.run([str(binary), str(source), '--target-cfg'],
                                    input=json.dumps(ci_quality.compiler_configuration()),
                                    capture_output=True, text=True, check=True)
        payload = json.loads(result.stdout)
        self.assertEqual((payload['analyzer'], payload['version'], payload['rule']),
                         (pin['analyzer'], pin['version'], pin['rule']))
        self.assertEqual(payload['configuration'], ci_quality.compiler_configuration())

    def test_verify_measurer_asset_accepts_only_the_pinned_digest(self):
        pin = ci_quality.load_measurer_pin()
        with tempfile.TemporaryDirectory() as directory:
            asset = Path(directory) / pin['asset']
            asset.write_bytes(b'pinned measurer bytes')
            with self.assertRaisesRegex(ValueError, 'digest mismatch'):
                ci_quality.verify_measurer_asset(asset, pin)
            pin['sha256'] = ci_quality.sha256(asset)
            ci_quality.verify_measurer_asset(asset, pin)

    def test_extraction_rejects_members_outside_the_release_root(self):
        pin = ci_quality.load_measurer_pin()
        root = pin['asset'].removesuffix('.tar.gz')
        archive = self.crafted_archive(pin, {f'{root}/{pin["executable"]}': b'elf',
                                             '../escape': b'escape'})
        crafted = dict(pin)
        crafted['sha256'] = ci_quality.sha256(archive)
        with patch.object(ci_quality, 'load_measurer_pin', return_value=crafted):
            with self.assertRaisesRegex(ValueError, 'unsafe measurer archive member'):
                ci_quality.ensure_measurer(archive.parent)

    def test_extraction_requires_the_executable_inside_the_release_root(self):
        pin = ci_quality.load_measurer_pin()
        root = pin['asset'].removesuffix('.tar.gz')
        archive = self.crafted_archive(pin, {f'{root}/README.md': b'no executable'})
        crafted = dict(pin)
        crafted['sha256'] = ci_quality.sha256(archive)
        with patch.object(ci_quality, 'load_measurer_pin', return_value=crafted):
            with self.assertRaisesRegex(ValueError, 'executable missing'):
                ci_quality.ensure_measurer(archive.parent)

    def test_core_risk_stage_no_longer_builds_the_measurer_from_source(self):
        source = (ROOT / 'tools/quality/ci_quality.py').read_text()
        self.assertNotIn('rust-measure/Cargo.toml', source)
        self.assertNotIn("'analyzer-build'", source)
        self.assertIn("'measurer-download'", source)


if __name__ == '__main__':
    unittest.main()
