"""Regression checks for the tag-triggered release workflow boundaries."""

from __future__ import annotations

import re
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


if __name__ == "__main__":
    unittest.main()
