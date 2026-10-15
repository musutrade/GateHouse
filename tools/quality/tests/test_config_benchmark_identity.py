from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from config_benchmark_identity import build_identity


class IdentityTests(unittest.TestCase):
    def test_build_overrides_rejected_before_subprocess(self):
        for key in ('RUSTFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_PROFILE_RELEASE_SMALL_OPT_LEVEL',
                    'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER', 'RUSTC_WORKSPACE_WRAPPER'):
            with self.subTest(key=key), patch('subprocess.check_output') as execute:
                with self.assertRaisesRegex(ValueError, key):
                    build_identity(Path('/tmp'), {key: 'override'})
                execute.assert_not_called()

    def test_config_compiler_override_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'.cargo').mkdir()
            (root/'.cargo/config.toml').write_text('[build]\nrustc = "different-compiler"\n')
            with patch('subprocess.check_output') as execute:
                with self.assertRaisesRegex(ValueError, 'build-altering'):
                    build_identity(root, {'CARGO_HOME': str(root/'cargo-home')})
                execute.assert_not_called()

    def test_config_content_changes_identity_without_exposing_content(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'.cargo').mkdir()
            config = root/'.cargo/config.toml'
            config.write_text('[build]\njobs = 1\n')
            environment = {'CARGO_HOME': str(root/'cargo-home'), 'PATH': '/usr/bin'}
            with patch('subprocess.check_output', return_value='rustc fixture\nhost: fixture-target\n'), patch('shutil.which', return_value=__file__):
                first = build_identity(root, environment)
                config.write_text('[build]\njobs = 2\n')
                second = build_identity(root, environment)
            self.assertEqual(first['explicit_target'], 'fixture-target')
            self.assertNotEqual(first['cargo_config_sha256'], second['cargo_config_sha256'])
            self.assertNotIn('jobs =', str(first))


if __name__ == '__main__':
    unittest.main()
