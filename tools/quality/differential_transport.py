#!/usr/bin/env python3
"""Core-owned compact transport over the unchanged compatibility oracle."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "fixtures/generic-core"))
from differential import prepare
import replay


def transport_bytes(value):
    """Compact temporary Rust input; persisted oracle/report bytes stay canonical."""
    return (json.dumps(value, sort_keys=True, separators=(',', ':'),
                       ensure_ascii=False, allow_nan=False) + '\n').encode('utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rust-replay', type=Path, help='built differential_replay example')
    parser.add_argument('--prepare', type=Path, help='retain explicit input and roots for Rust tests')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if bool(args.prepare) == bool(args.rust_replay):
        parser.error('specify exactly one of --prepare and --rust-replay')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.prepare:
        args.output.write_bytes(transport_bytes(prepare(args.prepare.resolve())))
        return 0
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        input_path = work / 'input.json'
        input_path.write_bytes(transport_bytes(prepare(work)))
        completed = subprocess.run([str(args.rust_replay.resolve()), str(input_path), str(args.output)])
        if completed.returncode not in (0, 1):
            return completed.returncode
        result = json.loads(args.output.read_bytes())
        # Only replay-owned roots in human diagnostics become portable.
        result = replay.portable_errors(result, work)
        args.output.write_bytes(replay.canonical(result))
        print(f"{result['case_count']} retained Rust/Angular/contract cases; "
              f"{result['mismatch_count']} unexplained field mismatches; non-authoritative")
        return completed.returncode


if __name__ == '__main__':
    raise SystemExit(main())
