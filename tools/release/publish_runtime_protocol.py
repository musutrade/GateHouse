#!/usr/bin/env python3
"""Publish protocol once; authenticate registry bytes and compare packaged inputs."""
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile
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


def package_contents(package, name, version):
    """Compare regular crate members; only a clean packaging commit may differ."""
    prefix = f"{name}-{version}/"
    files = {}
    total = 0
    with tarfile.open(fileobj=io.BytesIO(package), mode="r:gz") as archive:
        for member in archive:
            relative = member.name.removeprefix(prefix)
            if (not member.name.startswith(prefix) or not member.isfile()
                    or any(part in ("", ".", "..") for part in relative.split("/"))
                    or relative in files or len(files) >= 4096):
                raise RuntimeError("invalid or duplicate protocol package member")
            total += member.size
            if member.size < 0 or total > MAX_BYTES:
                raise RuntimeError("protocol package contents exceed limit")
            data = archive.extractfile(member).read()
            if relative == ".cargo_vcs_info.json":
                stamp = json.loads(data)
                git = stamp["git"]
                if (not re.fullmatch(r"[0-9a-f]{40}", git["sha1"])
                        or ("dirty" in git and git["dirty"] is not False)):
                    raise RuntimeError("invalid or dirty protocol packaging commit")
                # path_in_vcs, dirty and all other fields remain comparison inputs.
                del git["sha1"]
                data = json.dumps(stamp, sort_keys=True, separators=(",", ":")).encode()
            files[relative] = (member.mode, data)
    if ".cargo_vcs_info.json" not in files or "Cargo.toml" not in files:
        raise RuntimeError("protocol package lacks manifest or VCS identity")
    return files


def verify_existing(name, version, package):
    metadata = fetch(f"https://crates.io/api/v1/crates/{name}/{version}")
    if metadata is None:
        return False
    entry = json.loads(metadata)["version"]
    if entry["num"] != version:
        raise RuntimeError("existing protocol version differs from requested version")
    downloaded = fetch(f"https://static.crates.io/crates/{name}/{name}-{version}.crate")
    if downloaded is None or hashlib.sha256(downloaded).hexdigest() != entry["checksum"]:
        raise RuntimeError("registry package download does not match registry checksum")
    if downloaded != package:
        if package_contents(downloaded, name, version) != package_contents(package, name, version):
            raise RuntimeError("existing protocol package contents differ from local package")
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
