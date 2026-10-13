# GH-230 authenticated invocation diagnostic

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: incomplete P6/P7; no supported combination, acceptance PR or handoff.
This follows the [operational-error correction](invocation-errors.md). All
previous diagnostic records and archives remain retained.

The new opt-in native test sends an Ed25519-signed v2 request through the existing
Rust Core `adapter run` interface to the freshly assembled private collector.
The key is generated for this test alone and establishes no distribution or
capture trust. The Core binary is the existing checkout build, not an original
released asset. No Core dispatch or policy code changed.

The observed Core results are:

| Request | Result |
| --- | --- |
| Signed input with its context commit subsequently changed | `adapter signature verification failed`; no replay ledger created |
| Original valid signed request | `adapter exited with 1`; the collector refuses the unknown tested Core/protocol/ABI combination |
| Same nonce after that invocation | `adapter request nonce has already been used` |

A separate direct call retains the collector's generic FAIL envelope with the
matching invocation ID, empty evidence and artifacts, and the unknown-combination
reason. Core reports the nonzero process exit before parsing this envelope. The
output directory stays empty. These assertions do not establish a successful
generic collection, a project-configuration positive, or producer launch counts.

The process tracing probe failed:

```text
strace -f -qq -e trace=execve -o target/gh-230/trusted-invocation-progress/logs/strace-probe.log /usr/bin/true
strace: do_test_ptrace_get_syscall_info: PTRACE_TRACEME: Operation not permitted
strace: attach: ptrace(PTRACE_SEIZE, 4): Operation not permitted
```

Its exit code was 1. No tracing bypass was attempted, and no launch count is
inferred from the output directory. The original command result is retained.

The runtime input inventory and assembly passed in 25.107 and 15.511 seconds.
The resulting workspace-local archive contains 1,623,480,320 bytes with SHA-256
`04d2b2a05a11f30c20b7a3d19ea5784c1471c1dc0a39ab50db634c68a7ac8011`.
These are existing-host diagnostic assembly timings, not cold/warm installation
measurements. The native test passed (one test; 22.825 seconds wall time), using
two fresh fixture captures and private native re-export. Both captures' original
binaries, profiles and associated files are retained in the diagnostic archive.
The full runtime archive remains under `target/gh-230/trusted-invocation-progress`;
the diagnostic archive does not substitute for durable complete package retention.

The earlier required Rust checks and 408-test Python discovery remain applicable:
only an opt-in native test and documentation were added in this follow-up.
The new test was run explicitly with the fresh private runtime. Documentation
consistency, strict OpenSpec and whitespace results are recorded alongside it.
The first documentation check failed on the four evidence links before their
files were written; its report is retained alongside the check after retention.
Project-local `config check` and `verify --profile ci --all` remain not applicable
because `.harness-gate/flow.toml` is absent. Hosted CI remains pending.

The released Core and clean supported-host prerequisites in
[recovery](recovery.md#released-core-prerequisite) remain unresolved. Successful
authenticated generic collection, actual Core capability/producer behavior,
clean-host native acceptance and cost measurements remain outstanding. All P6/P7
checkboxes remain unchecked. No release, baseline or Arc-Admin action was taken.

[Exact commands and results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/trusted-invocation-checks.json),
[retained diagnostic bytes](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/trusted-invocation-evidence.tar.gz.parts/manifest.json),
[file inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/trusted-invocation-inventory.json) and
[archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/trusted-invocation-archive.json) describe this follow-up.
