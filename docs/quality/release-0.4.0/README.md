# 0.4.0 candidate compatibility

The authentic v0.3.7 release fails the native/Core test because it has no
`quality` command. The 0.4.0 candidate built from accepted main plus release
metadata passed all nine native driver tests, including real native debt policy
evaluation. [Candidate identity](candidate.json) and [test output](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/release-0.4.0/native-tests.log)
retain the exact binary identity and test result. The test uses fresh captures
in the assigned GH-230 workspace, not an earlier task's native capture.

This is candidate compatibility, not publication or full GH-230 acceptance.
Relevant Engineering Policy semantics are unchanged. Formal asset identity,
Sigstore/provenance and the same test against distributed bytes are verified
separately after the protected publication workflow.

## Local release checks

[Exact commands, timings and exits](checks.json), [runner](run-gates.py) and
[full logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/release-0.4.0/validation-logs.tar.gz) retain the checks. Rust nextest: 392 passed,
zero skipped. Python: 402 tests, three opt-in native classes skipped; the nine
native tests above ran explicitly. Strict all-target/all-feature Clippy, CLI
snapshots, documentation consistency and verified Cargo packaging passed.
Formatting, 42 release tests, installer integrity tests, dependency audit and
strict OpenSpec also passed. The initial build exposed the workspace replay
crate's old ^0.3 dependency; it was updated to ^0.4 before the successful build.
Initial OpenSpec validation required an explicit metadata-only skip_specs marker;
this release changes no normative spec and all validation remains strict.
No measurement baseline or retained fixture was rewritten.

## Publication selection regression

A real `cargo publish --dry-run` initially failed because workspace defaults
selected the private `harness-gate-quality-core` crate (`publish = false`).
The workflow now explicitly selects `--package harness-gate`. The matching
`cargo publish --manifest-path tools/harness-gate/Cargo.toml --package harness-gate
--locked --dry-run --allow-dirty` passed package verification and stopped before
upload as requested. Both logs are retained; all 42 release tests passed again.
No crate or tag was published by this local dry-run.
