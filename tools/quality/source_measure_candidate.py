"""Reviewed 0.4.0 consumer candidate; production default remains 0.3.1."""
from __future__ import annotations

from pathlib import Path

from source_measure import SERIES, ast_with_identity

CANDIDATE_IDENTITY = {"analyzer": "harness-gate-rust-measure", "version": "0.4.0",
                      "rule": "mccabe-rust-3"}
CANDIDATE_SERIES = dict(SERIES, analyzer="harness-gate-rust-measure/0.4.0")


def ast_040(source: Path, binary: Path, configuration=None) -> dict:
    return ast_with_identity(source, binary, CANDIDATE_IDENTITY, configuration)
