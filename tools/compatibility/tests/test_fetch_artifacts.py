"""Guard tests for the external compatibility fetcher.

No network access: archives are synthesized in memory. The CI workflow still
performs the real pinned download immediately after these guards.
"""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location(
    "fetch_artifacts", ROOT / "tools/compatibility/fetch_artifacts.py")
fetch_artifacts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch_artifacts)

ROW = {"tag": "tag", "archive": "a.tar.gz", "prefix": "p", "sha256": "0" * 64}


def archive(members) -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as stream:
        for name, data, kind in members:
            info = tarfile.TarInfo(name)
            if kind == "symlink":
                info.type = tarfile.SYMTYPE
                info.linkname = "p/target"
                stream.addfile(info)
            elif kind == "dir":
                info.type = tarfile.DIRTYPE
                stream.addfile(info)
            else:
                info.size = len(data)
                stream.addfile(info, io.BytesIO(data))
    return buffer.getvalue()


def row_for(data: bytes, **overrides) -> dict:
    row = dict(ROW, sha256=hashlib.sha256(data).hexdigest())
    row.update(overrides)
    return row


class FetchGuardTests(unittest.TestCase):
    def setUp(self):
        self.destination = Path(tempfile.mkdtemp(prefix="fetch-guard-"))
        self.addCleanup(shutil.rmtree, self.destination, True)

    def assert_rejected(self, data: bytes, message: str, **overrides):
        with self.assertRaises(ValueError) as raised:
            fetch_artifacts.unpack(data, row_for(data, **overrides), self.destination)
        self.assertIn(message, str(raised.exception))

    def test_regular_and_directory_members_are_extracted(self):
        data = archive([("p/dir", b"", "dir"), ("p/dir/file.txt", b"payload", "file")])
        installed = fetch_artifacts.unpack(data, row_for(data), self.destination)
        self.assertEqual(installed.name, "p")
        self.assertEqual((installed / "dir/file.txt").read_bytes(), b"payload")

    def test_digest_mismatch_is_rejected_before_extraction(self):
        data = archive([("p/file", b"x", "file")])
        row = row_for(data, sha256="f" * 64)
        with self.assertRaises(ValueError) as raised:
            fetch_artifacts.unpack(data, row, self.destination)
        self.assertIn("digest mismatch", str(raised.exception))
        self.assertEqual(list(self.destination.iterdir()), [])

    def test_unsafe_segment_aliases_are_rejected_before_normalization(self):
        payload = b"payload"
        cases = {
            "dot segment": "p/./file",
            "leading dot segment": "p/./p/file",
            "empty segment": "p//file",
            "trailing dot segment": "p/./",
            "parent traversal": "p/../p/file",
            "absolute path": "/p/file",
            "missing prefix": "other/file",
            "duplicate member": None,
        }
        for label, name in cases.items():
            with self.subTest(case=label):
                members = [("p/file", payload, "file"), ("p/file", payload, "file")] if name is None \
                    else [(name, payload, "file")]
                data = archive(members)
                self.assert_rejected(data, "unsafe or duplicate external archive member")

    def test_link_and_oversize_limits_are_rejected(self):
        link = archive([("p/file", b"", "symlink")])
        self.assert_rejected(link, "unsafe or duplicate external archive member")
        oversized = archive([("p/big", b"y" * (fetch_artifacts.LIMIT + 1), "file")])
        self.assert_rejected(oversized, "exceeds compatibility limits")
        many = archive([(f"p/f{index}", b"x", "file") for index in range(101)])
        self.assert_rejected(many, "exceeds compatibility limits")

    def test_pinned_registry_entries_are_well_formed(self):
        config = json.loads((ROOT / "tools/compatibility/artifacts.json").read_text())
        self.assertEqual(config["schema"], "gatehouse-external-compatibility/v1")
        self.assertEqual(config["repository"], "musutrade/GateProbes")
        self.assertEqual(set(config["artifacts"]), {"github-check", "onboarding-presets"})
        for name, entry in config["artifacts"].items():
            with self.subTest(artifact=name):
                self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
                self.assertEqual(entry["prefix"], entry["archive"].removesuffix(".tar.gz"))
                self.assertEqual(entry["tag"].split("-v")[-1],
                                 entry["archive"].removesuffix(".tar.gz").split("-")[-1])


if __name__ == "__main__":
    unittest.main()
