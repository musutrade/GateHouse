"""Version routing keeps immutable legacy identities and rejects env overrides."""
from pathlib import Path
import os
import subprocess
import shlex
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]


class RepositoryCutoverTests(unittest.TestCase):
    def route(self, version):
        script = (ROOT / 'install.sh').read_text().removesuffix('main "$@"\n')
        script += '\nVERSION="$1"\nvalidate_version\nselect_core_repository\nprintf "%s\\n%s\\n" "$REPO" "$RELEASE_BASE_URL"\n'
        env = dict(os.environ, REPO='evil/repo', RELEASE_BASE_URL='https://evil.invalid')
        return subprocess.run(['bash', '-c', script, 'cutover-test', version], env=env,
                              text=True, capture_output=True)

    def test_legacy_versions_keep_original_owner(self):
        for version in ('v0.1.0','v0.3.3','v0.4.0','v0.4.8','v0.4.8-rc.1','v0.4.8+build'):
            with self.subTest(version=version):
                result = self.route(version)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.splitlines(), ['musutrade/Harness-Gate','https://github.com/musutrade/Harness-Gate/releases/download'])

    def test_new_versions_use_new_owner_without_fallback(self):
        for version in ('v0.4.9','v0.4.9-rc.1','v0.4.10','v0.5.0','v1.0.0'):
            with self.subTest(version=version):
                result = self.route(version)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.splitlines(), ['musutrade/GateHouse','https://github.com/musutrade/GateHouse/releases/download'])

    def test_malformed_versions_never_select_a_repository(self):
        for version in ('latest','0.4.9','v00.4.9','v0.4.9-01','v0.4.9;evil'):
            with self.subTest(version=version):
                self.assertNotEqual(self.route(version).returncode, 0)

    def test_full_installer_contracts_with_new_mock_signer(self):
        source = (ROOT / 'tools/release/tests/test_install.sh').read_text()
        lines = source.splitlines()
        self.assertEqual(sum(line.startswith('ROOT=') for line in lines), 1)
        lines = ['ROOT=' + shlex.quote(str(ROOT)) if line.startswith('ROOT=') else line for line in lines]
        # Reuse every positive and tamper/failure scenario, changing only the
        # mock release version and expected repository/certificate identity.
        script = ('\n'.join(lines) + '\n').replace('v0.3.3', 'v0.4.9').replace('musutrade/Harness-Gate', 'musutrade/GateHouse')
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'new-owner-contract.sh'
            path.write_text(script)
            result = subprocess.run(['bash', str(path)], text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stdout[-4000:] + result.stderr[-4000:])
            self.assertIn('installer integrity tests: pass', result.stdout)
