"""Failure boundaries for the native Angular fixture, not adapter certification."""

import importlib.util
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "fixtures/typescript-angular/collect.py"
SPEC = importlib.util.spec_from_file_location("angular_collection", SCRIPT)
collection = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(collection)


class AngularCollectionTests(unittest.TestCase):
    def test_retained_native_and_failed_build_artifacts(self):
        evidence = SCRIPT.parent / "evidence"
        for archive, expected_status in (("native.tar.gz", "complete"),
                                         ("failed-build.tar.gz", "failed")):
            with self.subTest(archive=archive), tarfile.open(evidence / archive) as tar:
                manifest = json.load(tar.extractfile("manifest.json"))
                self.assertEqual(manifest["status"], expected_status)
                for name, record in manifest["artifacts"].items():
                    data = tar.extractfile(name).read()
                    self.assertEqual(len(data), record["bytes"], name)
                    self.assertEqual(hashlib.sha256(data).hexdigest(), record["sha256"], name)
                statuses = [c["exit_status"] for c in manifest["commands"]]
                if expected_status == "complete":
                    self.assertTrue(all(status == 0 for status in statuses))
                    self.assertEqual(tar.extractfile("installed-0.json").read(),
                                     tar.extractfile("installed-1.json").read())
                    results = json.load(tar.extractfile("coverage/test-results.json"))
                    self.assertTrue(results["success"])
                    self.assertGreater(results["numTotalTests"], 0)
                else:
                    self.assertIn(1, statuses)

    def test_failed_command_retains_status_and_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "run"
            run = collection.Collection(output)
            with self.assertRaisesRegex(RuntimeError, "command failed"):
                run.run([sys.executable, "-c", "print('broken build'); raise SystemExit(7)"])
            manifest = json.loads((output / "manifest.json").read_text())
            self.assertEqual(manifest["status"], "failed")
            self.assertEqual(manifest["commands"][0]["exit_status"], 7)
            self.assertIn("broken build", (output / "command-00.log").read_text())

    def test_completed_output_cannot_be_reused(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "run"
            run = collection.Collection(output)
            run.manifest["status"] = "complete"
            run.save()
            with self.assertRaises(FileExistsError):
                collection.Collection(output)

    def test_missing_tool_and_timeout_remain_failed(self):
        for error in (FileNotFoundError("missing tool"),
                      subprocess.TimeoutExpired(["ng", "build"], 600)):
            with self.subTest(error=error), tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "run"
                run = collection.Collection(output)
                with patch.object(collection.subprocess, "run", side_effect=error):
                    with self.assertRaises(type(error)):
                        run.run(["ng", "build"])
                manifest = json.loads((output / "manifest.json").read_text())
                self.assertEqual(manifest["status"], "failed")
                self.assertIsNone(manifest["commands"][0]["exit_status"])
                self.assertIn("error", manifest["commands"][0])

    def test_successful_tool_without_artifacts_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(RuntimeError, "Build output or source maps missing"):
                collection.validate_outputs(Path(tmp))


if __name__ == "__main__":
    unittest.main()
