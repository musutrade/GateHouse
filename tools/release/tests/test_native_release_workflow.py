"""The native tag route must retain Core's publication trust boundary."""
import importlib.util
from pathlib import Path
import re
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / 'retired/workflows/native-collector-release.yml'
ACTIVE = ROOT / '.github/workflows/native-collector-release.yml'
SPEC = importlib.util.spec_from_file_location('release_inventory', ROOT / 'tools/release/release_inventory.py')
assert SPEC and SPEC.loader
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class NativeReleaseWorkflowTests(unittest.TestCase):
    def test_retired_entry_is_archived_and_not_active(self):
        self.assertFalse(ACTIVE.exists())
        source = WORKFLOW.read_text()
        self.assertIn('RETIRED', source.splitlines()[0])
        self.assertIn('6aaa234249574c9a321a8819213b4c936c02605e', source)

    def test_native_publication_requires_eligibility_and_actual_binary_acceptance(self):
        source = WORKFLOW.read_text()
        policy, build, publish = source.split('  policy:\n')[1].split('  build:\n')[0], \
            source.split('  build:\n')[1].split('  publish:\n')[0], source.split('  publish:\n')[1]
        self.assertIn('--tag-prefix rust-collector-v', policy)
        self.assertIn('--manifest tools/quality/rust-native-plugin/Cargo.toml', policy)
        self.assertIn('refs/remotes/origin/main', policy)
        self.assertIn("needs.policy.result == 'success'", build)
        self.assertIn("!startsWith(github.ref, 'refs/tags/')", build)
        self.assertIn('python tools/quality/rust-native-plugin/accept.py', build)
        self.assertIn('needs: [policy, build]', publish)
        self.assertIn('environment: release', publish)
        self.assertIn("if: startsWith(github.ref, 'refs/tags/rust-collector-v')", publish)
        self.assertIn('pattern: native-release-assets-${{ github.run_id }}-*', publish)
        self.assertIn('merge-multiple: true', publish)
        self.assertNotIn('native-acceptance-', publish)
        for operation in ('generate', 'checksums', 'list', '"${verify_args[@]}"'):
            self.assertIn('tools/release/release_inventory.py ' + operation, publish)
        self.assertIn('--certificate-identity "https://github.com/${GITHUB_REPOSITORY}/.github/workflows/native-collector-release.yml@${GITHUB_REF}"', publish)
        self.assertNotIn('--certificate-identity-regexp', publish)
        self.assertIn('gh attestation verify', publish)
        self.assertIn('refusing to overwrite it', publish)
        self.assertNotIn('publish-crate', source)

    def test_each_core_platform_requires_its_own_native_acceptance_and_sbom(self):
        source = WORKFLOW.read_text()
        build = source.split('  build:\n')[1].split('  publish:\n')[0]
        publish = source.split('  publish:\n')[1]
        for target, asset in (
            ('x86_64-unknown-linux-gnu', 'harness-gate-rust-collector-linux-amd64'),
            ('x86_64-apple-darwin', 'harness-gate-rust-collector-macos-amd64'),
            ('aarch64-apple-darwin', 'harness-gate-rust-collector-macos-arm64'),
            ('x86_64-pc-windows-msvc', 'harness-gate-rust-collector-windows-amd64.exe'),
        ):
            self.assertIn('target: ' + target, build)
            self.assertIn('asset: ' + asset, build)
            self.assertIn('--binary ' + asset, publish)
            self.assertIn('--sbom ' + asset.removesuffix('.exe') + '.sbom.cdx.json', publish)
        self.assertIn('fail-fast: false', build)
        self.assertIn('--target ${{ matrix.target }}', build)
        self.assertIn('native-acceptance-${{ github.run_id }}-${{ matrix.target }}', build)

    def test_native_publish_inventory_seals_all_four_platform_sboms(self):
        source = WORKFLOW.read_text()
        publish = source.split('  publish:\n')[1]
        subjects = re.findall(r'--(binary|sbom) ([\w.-]+)', publish)
        binaries = {name for kind, name in subjects if kind == 'binary'}
        sboms = {name for kind, name in subjects if kind == 'sbom'}
        self.assertEqual(len(binaries), 4)
        self.assertEqual(len(sboms), 4)
        with tempfile.TemporaryDirectory(prefix='native-release-inventory-') as temporary:
            dist = Path(temporary)
            path = dist / 'release-inventory.json'
            args = ['generate', '--dist', str(dist), '--output', str(path)]
            for kind, name in subjects:
                (dist / name).write_bytes(name.encode())
                args.extend(['--' + kind, name])
            self.assertEqual(inventory.main(args), 0)
            data = inventory.load(path)
            self.assertEqual({a['name'] for a in data['assets']}, binaries | sboms)
            inventory.write_checksums(dist, data)
            integrity = inventory.list_operation(data, 'sign')
            self.assertTrue((binaries | sboms).issubset(integrity))
            for name in integrity:
                # These placeholders exercise inventory completeness only;
                # the release workflow separately verifies real signatures.
                (dist / (name + '.sig')).write_text('fixture signature')
                (dist / (name + '.crt')).write_text('fixture certificate')
            inventory.verify(dist, path, integrity)
            (dist / sorted(sboms)[0]).write_bytes(b'tampered platform SBOM')
            with self.assertRaisesRegex(inventory.InventoryError, 'modified asset'):
                inventory.verify(dist, path, integrity)


if __name__ == '__main__':
    unittest.main()
