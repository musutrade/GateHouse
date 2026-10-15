#!/usr/bin/env python3
"""Experimental, opt-in parallel Risk collection; no measured speedup assumed."""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import TYPE_CHECKING

from source_measure import compiler_configuration

if TYPE_CHECKING:
    from ci_quality import Collector


def risk_parallel(collector: Collector) -> None:
    """Wait for both workers on failure and preserve the original exception.

    Separate Cargo target directories keep coverage builds and profiles apart.
    The same collection, integrity checks and comparison serve both modes.
    """
    base, head = collector.report['base_sha'], collector.report['commit']
    collector._validate_risk_scope(base, head)
    binary = collector.measurer()
    target = compiler_configuration()['target']
    snapshots = {label: collector.snapshot(label, commit)
                 for label, commit in (('base', base), ('head', head))}
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = []
        for label, commit in (('base', base), ('head', head)):
            environment = collector.environment.copy()
            environment['CARGO_TARGET_DIR'] = str(collector.directory / 'build' / label)
            futures.append(executor.submit(
                collector._collect_commit_coverage, label, commit, snapshots[label],
                binary, target, env=environment))
        for future in as_completed(futures):
            # Executor exit waits for in-flight work even when result() raises.
            # Do not wrap errors: Collector.collect records supported failures.
            future.result()
    collector._compare_risk(base, head)
