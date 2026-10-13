#!/usr/bin/env python3
"""Exercise the real parallel benchmark fixture without a release build or baseline."""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import benchmarks


SAMPLES = 5
DIAGNOSTIC_TAIL_BYTES = 64 * 1024
MAX_WORKER_LOGS = 8


def print_retained_evidence(archive: Path) -> None:
    """Emit bounded diagnostics before the temporary fixture and archive disappear."""
    paths = sorted(archive.rglob("*.log"))[:MAX_WORKER_LOGS]
    paths.extend(archive / name for name in (
        "parallel-state.json", "command-result.json", "test_result.json"
    ))
    print(f"Retained parallel benchmark evidence: {archive}", file=sys.stderr)
    for path in paths:
        print(f"--- {path.relative_to(archive)} ---", file=sys.stderr)
        try:
            with path.open("rb") as stream:
                stream.seek(0, 2)
                size = stream.tell()
                stream.seek(max(0, size - DIAGNOSTIC_TAIL_BYTES))
                tail = stream.read(DIAGNOSTIC_TAIL_BYTES)
            if size > DIAGNOSTIC_TAIL_BYTES:
                print(f"[last {DIAGNOSTIC_TAIL_BYTES} of {size} bytes]", file=sys.stderr)
            print(tail.decode("utf-8", errors="replace"), file=sys.stderr)
        except OSError as error:
            print(f"[unavailable: {error}]", file=sys.stderr)


def run(binary: Path) -> list[dict]:
    # Reuse the benchmark's exact fixture, configuration and sample validation.
    # Five extra verification calls add test-suite cost, without a new timing
    # threshold, retry, baseline or benchmark measurement series.
    with tempfile.TemporaryDirectory(prefix="harness-gate-benchmark-probe-") as directory:
        temporary = Path(directory)
        root = temporary / "fixture"
        raw_root = temporary / "benchmark-runs"
        benchmarks.init_fixture(root)
        benchmarks.configure_interpreter(root)
        limit = benchmarks.configure_execution(root, parallel=True)
        samples = []
        for number in range(1, SAMPLES + 1):
            try:
                samples.append(benchmarks.verification_sample(
                    binary, root, raw_root, number, "parallel", limit
                ))
            except (Exception, SystemExit):
                print_retained_evidence(raw_root / "parallel" / f"sample-{number}")
                raise
        return samples


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    samples = run(args.binary.resolve(strict=True))
    print(json.dumps({"status": "passed", "samples": len(samples), "mode": "parallel"}))


if __name__ == "__main__":
    main()
