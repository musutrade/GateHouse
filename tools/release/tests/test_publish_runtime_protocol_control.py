"""Control-flow tests for the protocol publication helper. Test-only."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

SPEC = importlib.util.spec_from_file_location(
    "protocol_publish_control",
    Path(__file__).resolve().parents[1] / "publish_runtime_protocol.py",
)
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)


class ControlFlow(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="protocol-publish-control-")
        self.addCleanup(self.temp.cleanup)
        self.target = Path(self.temp.name)
        package = tomllib.loads(m.MANIFEST.read_text())["package"]
        self.package = self.target / "package" / f"{package['name']}-{package['version']}.crate"
        self.package.parent.mkdir(parents=True)
        self.package.write_bytes(b"exact-package")
        self.metadata = json.dumps({"target_directory": str(self.target)}).encode()
        self.commands = []

    def check_output(self, args, **kwargs):
        if args[:2] == ["git", "status"]:
            return b""
        if "metadata" in args:
            return self.metadata
        raise AssertionError(args)

    def fake_run(self, args, **kwargs):
        self.commands.append(args)
        return subprocess.CompletedProcess(args, 0)

    def command_count(self, word):
        return sum(1 for args in self.commands if word in args)

    def test_clean_checkout_is_required_before_package_or_upload(self):
        with patch.object(m.subprocess, "check_output", return_value=b" M dirty"):
            with patch.object(m.subprocess, "run") as run:
                with self.assertRaisesRegex(RuntimeError, "clean checkout"):
                    m.main()
                run.assert_not_called()

    def test_existing_matching_package_skips_dry_run_and_upload(self):
        with patch.object(m.subprocess, "check_output", side_effect=self.check_output):
            with patch.object(m.subprocess, "run", side_effect=self.fake_run):
                with patch.object(m, "verify_existing", return_value=True):
                    m.main()
        self.assertEqual(self.command_count("package"), 1)
        self.assertEqual(self.command_count("publish"), 0)
        self.assertEqual(sum(1 for args in self.commands if "cargo" in args), 1)

    def test_existing_mismatch_refuses_before_upload(self):
        with patch.object(m.subprocess, "check_output", side_effect=self.check_output):
            with patch.object(m.subprocess, "run", side_effect=self.fake_run):
                with patch.object(m, "verify_existing", side_effect=RuntimeError("mismatch")):
                    with self.assertRaisesRegex(RuntimeError, "mismatch"):
                        m.main()
        self.assertEqual(self.command_count("publish"), 0)

    def test_absent_runs_dry_run_upload_then_bounded_visibility(self):
        with patch.object(m.subprocess, "check_output", side_effect=self.check_output):
            with patch.object(m.subprocess, "run", side_effect=self.fake_run):
                with patch.object(m, "verify_existing", side_effect=[False, False, True]) as verify:
                    with patch.object(m.time, "sleep") as sleep:
                        m.main()
        self.assertEqual(self.command_count("package"), 1)
        self.assertEqual(sum(1 for args in self.commands if "publish" in args), 2)
        self.assertEqual(verify.call_count, 3)
        sleep.assert_called_once_with(10)

    def test_upload_failure_is_not_retried(self):
        def fake_run(args, **kwargs):
            self.commands.append(args)
            if "publish" in args and "--dry-run" not in args:
                raise subprocess.CalledProcessError(1, args)
            return subprocess.CompletedProcess(args, 0)

        with patch.object(m.subprocess, "check_output", side_effect=self.check_output):
            with patch.object(m.subprocess, "run", side_effect=fake_run):
                with patch.object(m, "verify_existing", return_value=False) as verify:
                    with patch.object(m.time, "sleep") as sleep:
                        with self.assertRaises(subprocess.CalledProcessError):
                            m.main()
        self.assertEqual(sum(1 for args in self.commands if "publish" in args and "--dry-run" not in args), 1)
        verify.assert_called_once()
        sleep.assert_not_called()

    def test_visibility_timeout_fails_without_upload_retry(self):
        with patch.object(m.subprocess, "check_output", side_effect=self.check_output):
            with patch.object(m.subprocess, "run", side_effect=self.fake_run):
                with patch.object(m, "verify_existing", return_value=False) as verify:
                    with patch.object(m.time, "sleep") as sleep:
                        with self.assertRaisesRegex(RuntimeError, "visibility is unverified"):
                            m.main()
        self.assertEqual(sum(1 for args in self.commands if "publish" in args and "--dry-run" not in args), 1)
        self.assertEqual(verify.call_count, 5)
        self.assertEqual(sleep.call_count, 3)

    def test_registry_404_is_absence_but_other_http_errors_propagate(self):
        with patch.object(m.urllib.request, "urlopen", side_effect=HTTPError("u", 404, "missing", {}, None)):
            self.assertIsNone(m.fetch("https://example.invalid/version"))
        with patch.object(m.urllib.request, "urlopen", side_effect=HTTPError("u", 500, "server", {}, None)):
            with self.assertRaises(HTTPError):
                m.fetch("https://example.invalid/version")


if __name__ == "__main__":
    unittest.main()
