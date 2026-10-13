#!/usr/bin/env python3
"""Replay pinned Arc-Admin syntax inventory; never coverage evidence."""
import argparse
import json
from pathlib import Path
import subprocess

from rust_native import digest, require, write_json

COMMIT = "e5a1ee5f7ec6dbae461355106d469384581d4607"
ROOT = Path(__file__).resolve().parents[2]


def replay(source, analyzer, output, expect_original=False):
    source, analyzer = source.resolve(), analyzer.resolve()
    require(source.is_relative_to(ROOT), "source checkout must be inside this workspace")
    require(output.resolve().is_relative_to(ROOT), "evidence must be inside this workspace")
    require(not output.exists(), "refusing to overwrite replay evidence")
    reference = json.loads((ROOT / "docs/quality/rust-native-inventory-replay.json").read_text())
    require(reference["source_commit"] == COMMIT, "unexpected reference revision")
    revision = subprocess.check_output(["git", "-C", source, "rev-parse", "HEAD"], text=True).strip()
    require(revision == COMMIT, "source revision mismatch")
    paths = {str(p.relative_to(source)) for p in (source / "backend/src").rglob("*.rs")}
    require(paths == {r["path"] for r in reference["files"]}, "source inventory omission/addition")
    rows = []
    for original in reference["files"]:
        path = source / original["path"]
        sha = digest(path.read_bytes())
        require(sha == original["sha256"], "source hash mismatch: " + original["path"])
        row = {"path": original["path"], "sha256": sha}
        for mode in ("legacy", "native"):
            command = [str(analyzer), *(["--native-inventory"] if mode == "native" else []), str(path)]
            result = subprocess.run(command, capture_output=True, text=True)
            row[mode] = {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
            if expect_original:
                require(result.returncode == original[mode]["exit_code"], "original replay mismatch: " + original["path"])
        rows.append(row)
    report = {"source_repository": "https://github.com/musutrade/arc-admin", "source_commit": COMMIT,
              "binary_sha256": digest(analyzer.read_bytes()), "certified_llvm_mapping": False,
              "files": rows}
    write_json(output, report)
    return {mode: sum(r[mode]["exit_code"] != 0 for r in rows) for mode in ("legacy", "native")}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--analyzer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expect-original", action="store_true")
    args = parser.parse_args()
    print(json.dumps(replay(args.source, args.analyzer, args.output, args.expect_original)))
