"""Regression checks for the tag-triggered release workflow boundaries."""

from __future__ import annotations

import re
import os
import subprocess
import unittest
from pathlib import Path


WORKFLOW = Path(__file__).parents[3] / ".github" / "workflows" / "release.yml"


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.source = WORKFLOW.read_text(encoding="utf-8")

    def test_core_publication_is_independent_of_plugin_builds(self) -> None:
        self.assertIn("needs: [policy, build, quality]", self.source)
        self.assertNotRegex(self.source, r"(?m)^  collectors:")
        self.assertNotIn("package_independent_collectors.py", self.source)
        self.assertNotIn("release-harness-gate-independent-collectors", self.source)
        inventory = self.source.split("- name: Generate release inventory", 1)[1].split(
            "- name: Generate checksum manifest", 1)[0]
        self.assertNotIn("--package ", inventory)
        self.assertEqual(re.findall(r"--binary (\S+)", self.source), [
            "harness-gate-linux-amd64",
            "harness-gate-macos-amd64",
            "harness-gate-macos-arm64",
            "harness-gate-windows-amd64.exe",
        ])
        self.assertEqual(re.findall(r"--sbom (\S+)", self.source),
                         ["harness-gate.sbom.cdx.json"])

    def test_quality_restores_verified_data_before_consuming_it(self) -> None:
        quality = self.source.split("\n  quality:\n", 1)[1].split("\n  publish:\n", 1)[0]
        self.assertIn("uses: ./.github/actions/test-data", quality)
        self.assertLess(quality.index("uses: ./.github/actions/test-data"),
                        quality.index("name: Run locked tests"))
        action = (WORKFLOW.parents[1] / "actions/test-data/action.yml").read_text()
        self.assertIn("tools/maintenance/test_data.py", action)

    def test_publication_download_excludes_policy_evidence(self) -> None:
        """Only build-matrix artifacts may be copied into the release dist."""

        self.assertIn("pattern: release-harness-gate-*", self.source)
        self.assertNotIn("pattern: release-*\n", self.source)
        self.assertIn("name: release-policy-${{ github.run_id }}", self.source)
        self.assertIn("name: release-${{ matrix.asset_name }}", self.source)

        download_block = re.search(
            r"- name: Download release assets(?P<block>.*?)(?=\n\s*- name: Set up Python)",
            self.source,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(download_block)
        assert download_block is not None
        self.assertNotIn("release-policy", download_block.group("block"))

    def test_credentials_are_checked_before_any_publication(self) -> None:
        publish, crate = self.source.split("\n  publish:\n", 1)[1].split(
            "\n  publish-crate:\n", 1)
        for block, next_step in ((publish, "name: Download release assets"),
                                 (crate, "name: Install Rust")):
            with self.subTest(next_step=next_step):
                self.assertIn("environment: release", block)
                self.assertLess(block.index("run: bash tools/release/check-registry-credentials.sh"),
                                block.index(next_step))
                self.assertIn("CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}", block)
        self.assertIn("needs: [policy, build, quality]", publish)
        self.assertIn("needs: publish", crate)
        self.assertNotIn("--token", crate)
        self.assertIn("--package harness-gate --locked", crate)

    def test_credential_check_rejects_missing_and_blank_without_disclosure(self) -> None:
        script = WORKFLOW.parents[2] / "tools/release/check-registry-credentials.sh"
        for token in (None, "", " \t\n", "fixture-secret-$()`;value"):
            with self.subTest(token_present=bool(token)):
                env = {key: value for key, value in os.environ.items()
                       if key != "CARGO_REGISTRY_TOKEN"}
                if token is not None:
                    env["CARGO_REGISTRY_TOKEN"] = token
                result = subprocess.run(["bash", "-x", str(script)], env=env,
                                        capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0 if token and token.strip() else 1)
                if result.returncode:
                    self.assertIn("CARGO_REGISTRY_TOKEN is missing or blank", result.stderr)
                else:
                    self.assertNotIn(token, result.stdout + result.stderr)
                    self.assertNotIn("::error::", result.stderr)


if __name__ == "__main__":
    unittest.main()
