# Source-risk rc.1 local acceptance

Independent plugin: `tools/quality/rust-source-risk`, no Core modifications.
The installed release-mode AST binary plus Python collector passed a real
CodexSymphony backend capture and signed Core 0.4.5 collection/evaluation.

The snapshot contains 10 source functions. Maximum CC is 7, maximum CRAP is
15428/2197. Every function satisfies required CRAP ≤10, source line ≥80% and
source region ≥80%. Five backend integration tests exercise real PostgreSQL,
HTTP startup, shutdown and configuration errors. Five AST tests and twelve
measurement/protocol tests include real instrumented CC=10/11 functions and an
async function constructed without being polled. The latter must remain uncovered.

Core rejects stale context, expired request, signature modification, replay and
artifact modification. Three separate synthetic-value comparisons verify Core's
exact threshold: 10 passes; 11 and 10.0001 fail. Those synthetic comparisons are
not represented as measured source coverage.

`codexsymphony-evidence.tar.gz` retains the source snapshot, receipts, re-exported
LLVM evidence and public verification materials; no private keys. The acceptance
JSON records exact metrics, identities, package hashes and the external native
replay archive. The ~77 MB native replay archive is local and is intentionally
not committed to Git. It is required for independent binary/counter re-export.

This is a local candidate, not a published/signed plugin release, complete
multi-collector production gate, or production trust provisioner. MIR remains a
separate diagnostic series. Unsupported source macros or mapping kinds fail closed.
