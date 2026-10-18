#!/usr/bin/env python3
"""Run the frozen Rust shadow reference with a stat-bound read-only cache.

The cache is an execution-only optimization: it imports the unchanged
``rust_reference.py`` module, then memoizes two exact read patterns used during
evidence projection.  Every cached result is keyed by the resolved path plus
the file's inode/mtime/size identity.  A changed file, a different expected
digest, or a failed validation falls back to the frozen implementation.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import sys

import harness_evidence
import rust_reference


@contextmanager
def caches():
    """Install the read cache for one shadow invocation and report hit counts."""
    original_sha256 = rust_reference.sha256
    original_file = harness_evidence._file
    hashes = {}
    verified = set()
    stats = {'sha256_hits': 0, 'sha256_misses': 0,
             'file_hits': 0, 'file_misses': 0}

    def identity(path: Path):
        resolved = path.resolve(strict=True)
        stat = resolved.stat()
        return (str(resolved), stat.st_mtime_ns, stat.st_ctime_ns,
                stat.st_size, getattr(stat, 'st_ino', 0))

    def cached_sha256(path):
        try:
            key = identity(Path(path))
        except (OSError, RuntimeError, TypeError):
            return original_sha256(path)
        if key in hashes:
            stats['sha256_hits'] += 1
            return hashes[key]
        stats['sha256_misses'] += 1
        value = original_sha256(path)
        hashes[key] = value
        return value

    def cached_file(root, path, digest, size=None):
        try:
            root_resolved = Path(root).resolve(strict=True)
            candidate = (root_resolved / path).resolve(strict=True)
            stat = candidate.stat()
            key = (str(root_resolved), str(candidate), digest, size,
                   stat.st_mtime_ns, stat.st_ctime_ns, stat.st_size,
                   getattr(stat, 'st_ino', 0))
        except (OSError, RuntimeError, TypeError):
            return original_file(root, path, digest, size)
        if key in verified:
            stats['file_hits'] += 1
            return None
        stats['file_misses'] += 1
        result = original_file(root, path, digest, size)
        verified.add(key)
        return result

    rust_reference.sha256 = cached_sha256
    harness_evidence._file = cached_file
    try:
        yield stats
    finally:
        rust_reference.sha256 = original_sha256
        harness_evidence._file = original_file


def main(argv=None):
    """Parse the wrapper-only flag, then delegate to the frozen reference."""
    argv = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--show-stats', action='store_true')
    options, remaining = parser.parse_known_args(argv)
    previous = sys.argv
    sys.argv = [previous[0], *remaining]
    try:
        with caches() as stats:
            result = rust_reference.main()
    finally:
        sys.argv = previous
    if options.show_stats:
        print('shadow-cache: ' + json.dumps(stats, sort_keys=True), flush=True)
    return result


if __name__ == '__main__':
    raise SystemExit(main())
