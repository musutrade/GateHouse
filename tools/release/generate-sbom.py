#!/usr/bin/env python3
"""Create a deterministic CycloneDX SBOM from cargo metadata."""

import argparse
import hashlib
import json
import os
import re
from datetime import datetime, timezone
from pathlib import Path


_SOURCE_USERINFO = re.compile(r"(?P<scheme>://)[^/?#\s@]+@")


def redact_source_url(source: str) -> str:
    """Remove URL userinfo so credentials cannot enter the published SBOM."""

    return _SOURCE_USERINFO.sub(r"\g<scheme>", source)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def component(package: dict) -> dict:
    name = package["name"]
    version = package["version"]
    value = {
        "type": "library",
        "bom-ref": f"pkg:cargo/{name}@{version}",
        "name": name,
        "version": version,
        "purl": f"pkg:cargo/{name}@{version}",
    }
    if package.get("source"):
        value["externalReferences"] = [
            {"type": "distribution", "url": redact_source_url(package["source"])}
        ]
    return value


def run(metadata_path: Path, lockfile_path: Path, output_path: Path) -> None:
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    packages = sorted(metadata.get("packages", []), key=lambda package: (package["name"], package["version"]))
    root_id = metadata.get("resolve", {}).get("root")
    root = next(package for package in packages if package.get("id") == root_id)
    commit = os.environ.get("GITHUB_SHA", "unknown")
    toolchain = os.environ.get("RUSTUP_TOOLCHAIN", "stable")
    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "timestamp": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
            "component": component(root),
            "properties": [
                {"name": "source.commit", "value": commit},
                {"name": "build.toolchain", "value": toolchain},
                {"name": "cargo.lock.sha256", "value": sha256(lockfile_path)},
            ],
        },
        "components": [component(package) for package in packages],
    }
    output_path.write_text(json.dumps(bom, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--lockfile", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    run(args.metadata, args.lockfile, args.output)
