# TypeScript/Angular retained collector v1

GH-131 implements only OpenSpec `typescript-angular-reference-adapter` task 3.1.
[typescript_reference.py](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/typescript_reference.py) projects
the real GH-129 native archive and GH-130 parser index using the existing
[source semantics](typescript-source-semantics.md), [project model](project-model.md)
and [collector protocol](collector-protocol.md). It returns measurement facts in
`harness-evidence/v1`; policy, ratchets, contract integration and certification
remain tasks 3.2–4.3. This follows [ADR-0040](../adr/0040-language-agnostic-evidence-policy.md)
without changing generic schemas, runner, policy or aggregation.

## Invocation and caller-owned inputs

Use `run_collector(InternalAdapter(typescript_reference.collect), request,
project=project)` or `run_collector(SubprocessAdapter((sys.executable,
absolute_adapter_path), timeout_seconds=60), request, project=project)`.
The CLI reads one request from stdin and writes one response to stdout. Always
consume it through the generic runner so artifact, source, project, capability
and response validation applies. The runner owns process deadlines and rejects
nonzero exits and malformed stdout before returning any evidence.

The request uses `harness-collector-request/v1`, collector
`{"name":"typescript-reference","version":"1"}`, component `typescript`,
the caller's exact context and generic requested capability names. The workspace
root is the fixture root containing `app/`, `provider/` and `toolchain.json`.
The caller supplies an empty output directory and these `parameters`:

| Parameter | Contract |
| --- | --- |
| `native` | Canonical workspace-relative path to retained `native.tar.gz` |
| `index` | Canonical workspace-relative path to the pinned compiler source index |
| `subjects` | Nonempty unique list of exact caller-owned project subjects; each produces one record |
| `receipt` | Caller-owned object described below; never discovered in tool output |

The receipt contains exactly `schema: typescript-collector-replay/v1`,
`native_sha256`, `index_sha256`, `coverage_root`, `native_revision`, and `request`.
The last field is an exact copy of the request's `project`, `component`,
`collector`, `context`, `requested_capabilities` and `parameters.subjects`
(under key `subjects`). `binding(request)` constructs this scope binding.
`coverage_root` is the original absolute coverage prefix, interpreted only as a
native path identifier; the adapter never reads that historical workspace.
`native_revision` must match the retained manifest. The caller must obtain and
trust those pins independently of the archive being measured.

This is explicit retained replay, not an assertion that tools ran at the replay
commit/run. GH-129's manifest predates the fixture commit and records its dirty
working tree, argv, original revision and environment. Its bytes are retained
unchanged, while the receipt binds their use to the caller's replay context.
Every original source and all declared configuration bytes must equal the current
workspace snapshot. Changed source, config, digest, native revision or replay
scope fails closed. Receipts provide integrity against caller expectations,
not independent authentication of a caller's claims.

Each distinct source retains its own archive, index and receipt copies because
the existing Artifact contract binds every artifact to one source. Methods in
the same source share that inventory. This deliberate disk duplication preserves
the generic provenance semantics. No package installation or native tool rerun
is performed during replay.

## TS-03 disposition

The existing v1 request is a single set of requested metrics for all returned
subjects. A mixed file/function/method/route request fits it without a generic
request-scope amendment:

| Subject/measurement | Explicit state |
| --- | --- |
| Mapped file/function/method line and function coverage | `supported`, exact native integer covered/total counters |
| Empty native denominator | `not_applicable`, no numeric metric |
| Caller-declared source-backed route | `unsupported` for all metrics; source coverage does not measure route execution |
| Template-bearing or generated source | `unsupported` for all metrics |
| Branch, complexity, CRAP and any other requested generic metric outside the measured matrix | `unsupported`, with reason and raw provenance |

Source-backed unavailable subjects come from the caller's project; the adapter
does not invent route or template identities. Supported source identities must
match the versioned compiler/source-map normalization. All returned records
declare all requested metrics, plus the fixed GH-130 measurement matrix. Extra
requested metrics extend the canonical series metric contract with their generic
types. Unsupported values never receive numeric defaults. The runner rejects
omission, even for a route. A per-kind metric object is rejected as an invalid
request rather than interpreted as a hidden scope extension.

## Validation boundary

Incomplete or failed native collections, malformed parser output, changed or
undeclared native artifacts, stale current bytes and invalid mapped coverage
produce an error response with empty evidence/artifact arrays. The runner also
rejects response provenance mismatch, omission, output tampering, undeclared
output, final decision fields, subprocess failure, timeout and malformed stdout.
Diagnostics left after failure never become policy input.

[GH-131 validation](gh-131/README.md) includes replay through both transports,
the exact native counter oracle, explicit unavailable states and adversarial
tests. No second-ecosystem certification or required-gate migration is claimed.
