#!/usr/bin/env python3
"""Publish the exact packaged protocol once; reject conflicting registry bytes."""
import hashlib
import json
from pathlib import Path
import subprocess
import time
import tomllib
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "tools/harness-gate/runtime-protocol/Cargo.toml"
MAX_BYTES = 16 * 1024 * 1024


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "GateHouse-release"})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read(MAX_BYTES + 1)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise
    if len(data) > MAX_BYTES:
        raise RuntimeError("registry response exceeds limit")
    return data


def verify_existing(name, version, package):
    metadata = fetch(f"https://crates.io/api/v1/crates/{name}/{version}")
    if metadata is None:
        return False
    entry = json.loads(metadata)["version"]
    digest = hashlib.sha256(package).hexdigest()
    if entry["num"] != version or entry["checksum"] != digest:
        raise RuntimeError("existing protocol version differs from exact local package")
    downloaded = fetch(f"https://static.crates.io/crates/{name}/{name}-{version}.crate")
    if downloaded is None or hashlib.sha256(downloaded).hexdigest() != digest:
        raise RuntimeError("registry package download does not match local package")
    return True


def main():
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
        raise RuntimeError("publication requires a clean checkout")
    info = tomllib.loads(MANIFEST.read_text())["package"]
    name, version = info["name"], info["version"]
    if name != "gatehouse-runtime-protocol":
        raise RuntimeError("unexpected package")
    command = ["cargo", "publish", "--manifest-path", str(MANIFEST), "--locked"]
    subprocess.run(["cargo", "package", "--manifest-path", str(MANIFEST), "--locked"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--manifest-path", str(MANIFEST), "--format-version", "1", "--no-deps"], cwd=ROOT))
    package = (Path(metadata["target_directory"]) / "package" / f"{name}-{version}.crate").read_bytes()
    if verify_existing(name, version, package):
        print("Exact protocol package already published; upload skipped")
        return
    subprocess.run(command + ["--dry-run"], cwd=ROOT, check=True)
    subprocess.run(command, cwd=ROOT, check=True)
    for attempt in range(4):
        if verify_existing(name, version, package):
            print("Published protocol package independently verified")
            return
        if attempt < 3:
            time.sleep(10)
    raise RuntimeError("protocol upload returned but registry visibility is unverified; inspect before retry")


if __name__ == "__main__":
    main()
