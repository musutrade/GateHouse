#!/usr/bin/env python3
"""Offline contract tests for the release inventory boundary."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).parents[1] / "release_inventory.py"
SPEC = importlib.util.spec_from_file_location("release_inventory", MODULE_PATH)
assert SPEC and SPEC.loader
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class ReleaseInventoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="harness-gate-release-fixture-")
        self.dist = Path(self.temp.name) / "dist"
        self.dist.mkdir()
        for name, content in {
            "harness-gate-linux-amd64": b"linux",
            "harness-gate-windows-amd64.exe": b"windows",
            "harness-gate.sbom.cdx.json": b'{"bomFormat":"CycloneDX"}\n',
        }.items():
            (self.dist / name).write_bytes(content)
        self.inventory_path = self.dist / inventory.INVENTORY_FILE
        self.data = inventory.generate(
            self.dist,
            self.inventory_path,
            ["harness-gate-linux-amd64", "harness-gate-windows-amd64.exe"],
            "harness-gate.sbom.cdx.json",
            "v0.0.0-fixture",
            "fixture-commit",
            "https://github.com/example/fixture",
        )
        inventory.write_checksums(self.dist, self.data)
        for name in inventory.list_operation(self.data, "sign"):
            (self.dist / f"{name}.sig").write_text("signature\n")
            (self.dist / f"{name}.crt").write_text("certificate\n")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def attested(self) -> list[str]:
        return inventory.list_operation(self.data, "attest")

    def assert_valid(self) -> None:
        inventory.verify(self.dist, self.inventory_path, self.attested())

    def test_exact_inventory_is_valid(self) -> None:
        self.assert_valid()

    def test_collector_package_is_signed_attested_and_tamper_checked(self) -> None:
        package = "collector-0.1.0-rc.1.tgz"
        (self.dist / package).write_bytes(b"tested collector")
        self.data = inventory.generate(
            self.dist, self.inventory_path,
            ["harness-gate-linux-amd64", "harness-gate-windows-amd64.exe"],
            "harness-gate.sbom.cdx.json", "v0.4.6", "exact-source",
            "https://github.com/example/fixture", packages=[package],
        )
        inventory.write_checksums(self.dist, self.data)
        for name in inventory.list_operation(self.data, "sign"):
            (self.dist / f"{name}.sig").write_text("signature\n")
            (self.dist / f"{name}.crt").write_text("certificate\n")
        self.assertIn(package, self.attested())
        self.assertIn(package, inventory.list_operation(self.data, "sign"))
        self.assert_valid()
        (self.dist / package).write_bytes(b"changed collector")
        with self.assertRaises(inventory.InventoryError):
            self.assert_valid()

    def test_missing_asset_blocks(self) -> None:
        (self.dist / "harness-gate-linux-amd64").unlink()
        with self.assertRaisesRegex(inventory.InventoryError, "missing"):
            self.assert_valid()

    def test_extra_asset_blocks(self) -> None:
        (self.dist / "unlisted.asset").write_bytes(b"extra")
        with self.assertRaisesRegex(inventory.InventoryError, "extra/unlisted"):
            self.assert_valid()

    def test_modified_asset_blocks(self) -> None:
        (self.dist / "harness-gate-linux-amd64").write_bytes(b"modified")
        with self.assertRaisesRegex(inventory.InventoryError, "modified"):
            self.assert_valid()

    def test_unsigned_asset_blocks(self) -> None:
        (self.dist / "harness-gate-linux-amd64.sig").unlink()
        with self.assertRaisesRegex(inventory.InventoryError, "missing"):
            self.assert_valid()

    def test_unattested_asset_blocks(self) -> None:
        attested = self.attested()[:-1]
        with self.assertRaisesRegex(inventory.InventoryError, "unattested"):
            inventory.verify(self.dist, self.inventory_path, attested)

    def test_unlisted_attestation_blocks(self) -> None:
        attested = self.attested() + ["unlisted.asset"]
        with self.assertRaisesRegex(inventory.InventoryError, "unlisted attestation"):
            inventory.verify(self.dist, self.inventory_path, attested)

    def test_asset_declaration_tampering_blocks(self) -> None:
        self.data["assets"][0]["size_bytes"] += 1
        self.inventory_path.write_text(json.dumps(self.data), encoding="utf-8")
        with self.assertRaisesRegex(inventory.InventoryError, "asset declaration"):
            inventory.verify(self.dist, self.inventory_path, self.attested())

    def test_inventory_path_binding_blocks(self) -> None:
        other = self.dist / "renamed-inventory.json"
        other.write_bytes(self.inventory_path.read_bytes())
        with self.assertRaisesRegex(inventory.InventoryError, "inventory_file"):
            inventory.load(other)

    def test_invalid_checksum_hex_blocks(self) -> None:
        checksum = self.dist / inventory.CHECKSUM_FILE
        checksum.write_text("z" * 64 + "  harness-gate-linux-amd64\n", encoding="utf-8")
        with self.assertRaisesRegex(inventory.InventoryError, "malformed checksum"):
            inventory.read_checksums(checksum)


if __name__ == "__main__":
    unittest.main()
