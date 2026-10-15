#!/usr/bin/env python3
"""Explicit config-loader attribution supplement; no existing baseline reset."""
import argparse
import json
import math
import os
import subprocess
import time
from pathlib import Path

from config_benchmark_identity import build_identity
from quality_common import CRATE, metadata, sha256, stats, write_json


def run(output: Path) -> None:
    root = CRATE.parent.parent
    inputs = [CRATE / "Cargo.toml", CRATE / "Cargo.lock", Path(__file__).resolve(), Path(__file__).with_name("config_benchmark_identity.py").resolve(),
              Path(__file__).with_name("quality_common.py").resolve(),
              *sorted((CRATE / "quality-core").rglob("*.rs")),
              *sorted((CRATE / "src").rglob("*.rs")),
              CRATE / "fixtures/preset-compat/rust-api.flow.toml"]
    identities = {str(p.relative_to(root)): sha256(p) for p in inputs}
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True)
    environment = os.environ.copy()
    removed = [name for name in ("REPORT_DIR", "HARNESS_GATE_REPORTS", "HARNESS_GATE_SECRETS_CONFIG") if name in environment]
    for name in removed:
        del environment[name]
    build = build_identity(root, environment)
    command = ["cargo", "test", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked",
               "--profile", "release-small", "--target", build["explicit_target"], "--bin", "harness-gate",
               "config::tests::configuration_loader_measurement", "--", "--ignored", "--exact",
               "--nocapture", "--test-threads=1"]
    started = time.monotonic()
    result = subprocess.run(command, cwd=CRATE.parent.parent, env=environment,
                            capture_output=True, text=True, timeout=900)
    wall = time.monotonic() - started
    if result.returncode:
        raise RuntimeError(f"configuration measurement failed: {result.stderr[-8000:]}\n{result.stdout[-8000:]}")
    prefix = "CONFIG_BENCHMARK_JSON="
    records = [line.split(prefix, 1)[1] for line in result.stdout.splitlines() if prefix in line]
    if len(records) != 1:
        raise RuntimeError("expected exactly one configuration measurement record")
    sample = json.loads(records[0])
    if sample.get("series") != "config-loader/1" or sample.get("equivalent") is not True:
        raise RuntimeError("invalid measurement identity or inequivalent loaders")
    repetitions = sample["repetitions"]
    if type(repetitions) is not int or repetitions != 100:
        raise RuntimeError("invalid repetition identity")
    if sample.get("fixture_sha256") != sha256(CRATE / "fixtures/preset-compat/rust-api.flow.toml"):
        raise RuntimeError("fixture identity mismatch")
    if any(sha256(root / name) != digest for name, digest in identities.items()):
        raise RuntimeError("measurement source changed during run")
    for name in ("memory_batch_seconds", "warm_file_batch_seconds"):
        raw = sample[name]
        if len(raw) != 5 or any(type(v) not in (float, int) or not math.isfinite(v) or v <= 0 for v in raw):
            raise RuntimeError("invalid sample count/durations")
        sample[name.replace("batch_seconds", "per_load_seconds")] = stats(v / repetitions for v in raw)
    if build_identity(root, environment) != build:
        raise RuntimeError("build identity changed during measurement")
    report = {**metadata(tool="config-benchmark"), "profile": "release-small", "build_identity": build, "source_sha256": identities, "workspace_status": dirty, "command": command,
              "command_wall_seconds_including_build": wall, "removed_override_names": removed,
              "measurement": sample, "threshold_policy": "supplement only; existing baseline and thresholds unchanged"}
    write_json(output, report)
    print(f"config-loader/1: memory median {sample['memory_per_load_seconds']['median']:.6f}s; warm file median {sample['warm_file_per_load_seconds']['median']:.6f}s")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="local result path, outside tracked source")
    run(parser.parse_args().output)
