# Collector protocol v1

Current runtime ownership and Python freeze/retirement rules are defined in the
[Python retention policy](python-retention.md). The released Rust core owns final
generic decisions; Python interfaces below remain adapter/reference tooling.


GH-113 implements OpenSpec tasks 4.1–4.4 as a standalone development/CI Python
boundary in [collector_runner.py](../../tools/quality/collector_runner.py), beside
the [normalized evidence contract](harness-evidence.md). It does not change the
release binary, existing Rust collection, workflow checks, or release authority
recorded in [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md).
Real Rust projection, generic numerical policy and equivalence acceptance remain
later OpenSpec tasks. Synthetic ecosystem data does not certify real adapters.

## Request and response

The [protocol schema](../../schema/collector-protocol.schema.json)
defines `Request`, `Response` and `Error` envelope shapes. The runner additionally
validates nested context/collector, every evidence record and artifact against
the existing normalized schema and the caller-owned project model. Unknown
envelope fields, including final release decisions, are rejected.

A request contains these required fields:

| Field | Meaning |
| --- | --- |
| `schema` | `harness-collector-request/v1` |
| `project`, `component` | IDs in the caller-owned project model |
| `collector` | Expected collector `name` and `version` |
| `context` | Exact `commit`, `base_commit`, `target`, `run` identity |
| `requested_capabilities` | Nonempty, unique generic metric names |
| `workspace_root` | Existing canonical absolute source directory |
| `output_root` | Existing, empty canonical absolute directory dedicated to this invocation |
| `parameters` | Policy-independent collection options or retained-input locations |

Requests use the same strict JSON domain as normalized evidence: integers and
typed/exact values instead of floating-point numbers. The output root may be
inside the workspace, but cannot equal or contain it. The caller chooses the
project model, provenance, expected collector, executable/arguments and deadline;
collectors cannot rewrite those expectations through their response. Internal
adapters receive a deep copy of the request.

Responses contain exactly `schema: harness-collector-response/v1`, `evidence`,
`artifacts`, and `error`. Success has a nonempty evidence array, a raw artifact
inventory, and `error: null`. Inventory entries are full normalized `Artifact`
descriptors and must exactly match references in evidence. Failure has empty
evidence/artifact arrays and an error object containing `code` (`measurement_error`
or `adapter_error`) and a nonempty `message`. Partial evidence alongside an error
is rejected. A failure may leave diagnostics on disk; none becomes policy input.

Each record must explicitly declare every requested capability. Unsupported
capabilities retain their reason and artifact provenance without a numeric value.
A valid record with a `measurement_error` capability stays measurement-error
evidence for the downstream consumer; it never becomes a successful measurement.
Tool, parser/rule, platform, normalization and series metadata retain the existing
normalized contract. The boundary validates canonical series identity and collector/version consistency.
Base/head series compatibility remains a separate precondition of later comparison.

## Invocation and policy input

`run_collector(adapter, request, project=project)` returns only validated
`harness-evidence/v1` records, or raises `CollectionError` with a stable `code` and
`as_dict()` representation. It never returns thresholds or release outcomes.

`InternalAdapter(callable)` invokes `callable(request) -> response`. It supports
reference adapters that project retained evidence without re-running tools.
`SubprocessAdapter(tuple_of_argv, timeout_seconds=60)` uses explicit argv without
a shell, workspace cwd, one UTF-8 JSON request on stdin, and one UTF-8 JSON response
on stdout. Stdout cannot contain logging, extra JSON documents, duplicate object
keys or nonfinite constants. Stderr is diagnostic-only and is not evidence.
The caller retains the request, argv/deadline and normalized collector/tool/series
identity as invocation provenance. Neither transport is imported by policy code.

Both transports enter the same response validation function. The existing
capability availability consumer demonstrates this with identical available and
blocked results; task 5's numerical/release evaluator is not implemented here.

## Fail-closed errors and limits

| Code | Cause |
| --- | --- |
| `invalid_request` | Invalid request/model/root/capability/deadline |
| `subprocess_error` | Executable cannot start or process I/O fails |
| `subprocess_exit` | Nonzero exit, even if stdout contains valid evidence |
| `timeout` | Deadline exceeded; POSIX process group killed and reaped |
| `malformed_json` | Invalid UTF-8/JSON, duplicate keys, nonfinite constants, extra document |
| `invalid_response` | Wrong response shape, release decision field, mixed error/evidence |
| `measurement_error`, `adapter_error` | Explicit adapter failure or internal exception |
| `stale_context` | Commit/base/target/run mismatch |
| `duplicate_subject` | Repeated subject/measurement-series pair in the batch |
| `missing_capability` | Requested capability silently omitted |
| `output_root_escape` | Noncanonical artifact path, output symlink, or replaced output root |
| `undeclared_artifact` | Unregistered output file, mismatched reference, or unused inventory entry |
| `missing_artifact` | Declared raw artifact missing from disk |
| `artifact_tampering` | Raw artifact hash or byte count mismatch |
| `invalid_artifact`, `invalid_evidence` | Remaining filesystem, schema, identity, source or series violation |

All output files must be declared and linked. Symlinks and nonregular artifact
files are rejected. Source bytes and raw bytes are validated against caller-bound
subject identity and artifact digests before records reach a consumer.

This is an evidence validation boundary, not an operating-system sandbox.
Adapters execute with the runner's permissions; concurrent hostile filesystem
mutation is outside this contract. Internal code is trusted to terminate and has
no forced timeout. Subprocess deadlines kill the process group on POSIX; on other
platforms only the immediate process is killed. Output size budgets and portable
process-tree containment are not introduced by this issue. Consumers loading
retained files later must revalidate digests with `validate_evidence`, as the
existing capability consumer does.

See [synthetic fixtures](../../tools/quality/fixtures/collectors/README.md) and
[GH-113 validation](gh-113-validation.md) for reproducible evidence.
