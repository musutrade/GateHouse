"""Contract tests for protocol publication ordering and credentials. Test-only."""
from __future__ import annotations

from pathlib import Path
import unittest

WORKFLOW = Path(__file__).parents[3] / ".github" / "workflows" / "release.yml"


class ProtocolReleaseWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.source = WORKFLOW.read_text(encoding="utf-8")

    def block(self, name, following):
        start = self.source.index(f"\n  {name}:\n")
        end = self.source.index(f"\n  {following}:\n", start + 1)
        return self.source[start:end]

    def test_protocol_runs_before_quality_and_core_preflight(self):
        protocol = self.block("protocol-publish", "quality")
        quality = self.block("quality", "publish")
        publish = self.block("publish", "publish-crate")
        crate = self.source[self.source.index("\n  publish-crate:\n"):]
        self.assertIn("needs: policy", protocol)
        self.assertIn("environment: release", protocol)
        self.assertIn("CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}", protocol)
        self.assertIn("tools/release/check-registry-credentials.sh", protocol)
        self.assertIn("publish_runtime_protocol.py", protocol)
        self.assertNotIn("--token", protocol)
        self.assertIn("needs: [policy, protocol-publish]", quality)
        self.assertIn("cargo publish --manifest-path tools/harness-gate/Cargo.toml --package harness-gate --locked --dry-run", quality)
        self.assertIn("needs: [policy, build, quality]", publish)
        self.assertIn("needs: publish", crate)
        self.assertNotIn("publish_runtime_protocol.py", quality + publish + crate)

    def test_protocol_publish_has_exact_tag_eligibility_path(self):
        protocol = self.block("protocol-publish", "quality")
        self.assertIn("actions/checkout", protocol)
        self.assertNotIn("--allow-dirty", protocol)
        self.assertIn("python3 tools/release/publish_runtime_protocol.py", protocol)


if __name__ == "__main__":
    unittest.main()
