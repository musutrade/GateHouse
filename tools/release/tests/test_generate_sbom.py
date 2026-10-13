"""Regression checks for credential-free SBOM source references."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).parents[1] / "generate-sbom.py"
SPEC = importlib.util.spec_from_file_location("generate_sbom", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class GenerateSbomTests(unittest.TestCase):
    def test_component_removes_source_userinfo(self) -> None:
        component = MODULE.component(
            {
                "name": "private-dependency",
                "version": "1.2.3",
                "source": "git+https://build-user:super-secret@example.test/repo#abc",
            }
        )
        url = component["externalReferences"][0]["url"]
        self.assertEqual(url, "git+https://example.test/repo#abc")
        self.assertNotIn("super-secret", url)

    def test_ssh_userinfo_is_not_published(self) -> None:
        self.assertEqual(
            MODULE.redact_source_url("git+ssh://deploy@example.test/repo"),
            "git+ssh://example.test/repo",
        )


if __name__ == "__main__":
    unittest.main()
