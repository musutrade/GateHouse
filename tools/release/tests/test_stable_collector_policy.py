"""Repository governance tests; not part of the installed Rust plugin."""
import os
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[3]


class StableCollectorPolicyTests(unittest.TestCase):
    def test_legacy_publication_hold_rejects_all_mutating_operations(self):
        text = (ROOT / '.github/workflows/rust-collector-release.yml').read_text()
        start = text.index('      - name: Enforce stable Rust collector migration hold')
        end = text.index('      - uses:', start)
        block = text[start:end]
        script = '\n'.join(line[10:] for line in block.split('        run: |\n', 1)[1].splitlines())
        self.assertLess(end, text.index('      - name: Require existing protected environment'))
        for operation in ('', 'SIGNING', 'PUBLICATION', 'INSTALLER'):
            env = dict(os.environ, SIGNING='', PUBLICATION='', INSTALLER='')
            if operation:
                env[operation] = 'reviewed-packet-sha256'
            result = subprocess.run(['bash', '-e', '-c', script], env=env, capture_output=True, text=True)
            with self.subTest(operation=operation or 'read-only rehearsal'):
                self.assertEqual(result.returncode, int(bool(operation)))
                if operation:
                    self.assertIn('pure-Rust stable-interface collector', result.stderr)
        jobs = text.split('\n  dry-run:', 1)[1]
        for job in ('sign-private-candidate', 'publish', 'publish-installer'):
            # All mutation jobs must depend on the rejecting protection job.
            match = re.search(r'^  ' + re.escape(job) + r':\n(.*?)(?=^  [\w-]+:|\Z)', jobs, re.M | re.S)
            self.assertIsNotNone(match, job)
            self.assertIn('needs: protection', match[1])
        self.assertEqual(text.count('needs: protection'), 4)

    def test_required_ci_has_no_direct_unstable_compiler_backend(self):
        source = (ROOT / '.github/workflows/ci.yml').read_text()
        for forbidden in ('RUSTC_BOOTSTRAP', 'rustc_private', '@nightly',
                          'rust-native-driver/bootstrap.py', 'rustc-dev'):
            self.assertNotIn(forbidden, source)
        self.assertNotRegex(source, r'(?m)(?:^|\s)-Z(?:\s|[a-z])')
        # Every toolchain step uses the SHA-pinned commit of the action's
        # `stable` branch, whose default toolchain input is `stable`.
        refs = re.findall(r'dtolnay/rust-toolchain@\S+(?: # \S+)?', source)
        self.assertTrue(refs)
        self.assertEqual(set(refs), {
            'dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable'})
        self.assertIn('python3 -m unittest discover -s tools/release/tests', source)


if __name__ == '__main__':
    unittest.main()
