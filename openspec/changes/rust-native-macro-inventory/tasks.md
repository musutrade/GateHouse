# Tasks (GH-219)

Each task is bounded to less than four hours and follows the parent proposal.

- [x] M1 [P0/S] Reproduce four legacy AST failures against pinned Arc-Admin source and retain per-file hashes.
- [x] M2 [P0/M] Add independent native syntax identity and tracing/select expression traversal; preserve nested closure ownership and spans.
- [x] M3 [P0/S] Reject unknown/ambiguous macro grammars and generated code; document unproven runtime mapping and source-counter limits.
- [x] M4.1 [P0/M] Pass local checks: 392 product nextest tests, 381 Python tests, 5 native parser tests, fmt/Clippy, docs consistency and strict OpenSpec validation; retain true source replay.
- [x] M4.2 [P0/S] Pass required hosted CI and accept the PR before closing GH-219. PR #222 merged as `bfceab076ad3a1760c75ab412337ddc93959ec4d`; merge and successful required-check evidence is retained in `docs/quality/gh-220/pr-222.json`. This does not complete GH-215.
