# Rust collector measurement and delivery contract

GH-227 implements P0.1–P1.2 of the
[independent delivery OpenSpec](../../openspec/changes/package-official-rust-collector-for-independent-delivery/proposal.md).
The relevant [Engineering Policy](../engineering-policy.md) semantics remain
unchanged: collectors measure; released Rust Core decides requiredness, thresholds,
baseline/lineage, debt/ratchet, aggregation and final outcomes. Accepted CRAP,
coverage, capability and source-scope rules remain unchanged. There is no policy
or release fallback, baseline reset, platform certification or GH-215 enablement.

## Existing boundaries and exits

These source references describe existing behavior, not newly delivered commands.

| Entry/source | Completion and failure | Authority |
| --- | --- | --- |
| [collector_runner.py](../../tools/quality/collector_runner.py), `run_collector`, `SubprocessAdapter` | Development `harness-collector-request/v1` / `harness-collector-response/v1`; exit zero alone is insufficient. Validate mutually exclusive error/evidence, exact context/collector/capabilities, artifact inventory and hashes. Any nonzero exit rejects even parseable stdout; operational errors expose no partial evidence. | Frozen protocol/normalization reference; no release decision. |
| [Rust project collectors](../../tools/harness-gate/src/config/quality/collectors.rs), `Response`, collection validation | Released Core sends `harness-project-collector-request/v1` through the generic adapter. Response has `schema_version: "1"`, transport `status: "PASS"`, matching invocation, artifacts, and `collection` with `harness-project-collector-response/v1`, evidence and no error. Checks expected producer/series claims and retained artifact integrity. | This envelope is distinct from the development protocol. The standalone collector implements the binding; a Python development response is not directly a Core response. `PASS` here is transport completion, not quality acceptance. |
| [Rust process adapter](../../tools/harness-gate/src/process/adapter.rs), `run`, `run_process` | Host approves/signs requests and validates invocation, process exit and output. Nonzero execution blocks ingestion. | Host execution/capture trust, independent of package signing. |
| [rust_native_driver.py](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-native-plugin/rust_native_driver.py), `certify`, `main` | `certify` verifies the independent anchor, exact retained inventory, original tool paths/hashes and selection; reruns profdata merge and LLVM export against retained binaries/profiles. Complete report may have `passed: false`; legacy CLI writes it then returns 1. Integrity/tool/missing-file exceptions are failures too. Collection subcommands return 0 on completion. | Historical threshold-derived fields remain raw history, not a new collector verdict. An arbitrary exit 1 cannot be upgraded to success. |
| [rust_native.py](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/rust_native.py), `certify`, CLI | Earlier fixture certification also uses threshold-derived nonzero exits. | Historical fixture/series contract, not an independently delivered runtime. |
| [rust_native_classify.py](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-native-plugin/rust_native_classify.py), `classify`, `main` | Exit depends on `classification_complete`; reports preserve `measurement_passed` and `baseline_accepted: false`. Archive replay and retained-binary native re-export remain distinct. | Classification does not accept policy or baseline. |
| [rust_native_policy.py](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-native-plugin/rust_native_policy.py), `compatibility`, `require_history`, `evaluate`, `main` | History equality binds series, scope, tools, flags/cfg/build inputs, adapter, hotspots and projection. Evaluation invokes Rust Core and preserves its decision/nonzero exit. | Development preparation helper; no new authoritative Python policy path. |

## Typed measurement outcome

[rust_collector_contract.py](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-native-plugin/rust_collector_contract.py) defines
`MeasurementResult = MeasurementComplete | MeasurementFailure` and a development
protocol adapter `measure`. `MeasurementComplete.records` contains only validated
normalized facts. `MeasurementFailure` has a diagnostic code/message and no usable
records. It preserves the frozen runner's validation and failure codes.

Transport completion preserves all six capability states: `supported`,
`unsupported`, `not_configured`, `not_collected`, `not_applicable`, and
`measurement_error`. A complete envelope describing a capability error does not
turn that capability into a valid measurement. Missing/error capability values are
not zero-filled; Core applies existing requiredness. A valid supported coverage
measurement with a positive denominator and zero covered lines is still a valid
measurement. It carries no collector threshold or release verdict.

P2 must connect this distinction to a standalone native entry without changing
legacy CLI exits. It must obtain complete authenticated facts from the native API,
not suppress subprocess errors or strip arbitrary malformed verdict fields.
The GH-227 synthetic low-coverage and mocked legacy-exit tests verify contracts
only. They do not establish native positive measurement or delivery compatibility.

## Manifest and compatibility matrix

New candidates use `rust-collector-delivery/v2`, defined by the
[plugin manifest schema](../../tools/quality/schema/rust-collector-plugin.schema.json).
The package pins compiler, tools, normalization code, payloads and compatibility.
Project subjects, configuration digest and normalized series belong to the host's
`quality-trusted-state/v1` and authenticated collection request. They are no longer
baked into the package. The v2 manifest declares this configuration authority and
rejects the old project-specific fields. Core still authenticates the exact
configuration and claims; the collector binds its projection implementation and
compiler to the requested series and checks the certified capture's native identity.

This permits fresh project configurations after installation without rebuilding
the plugin. A changed compiler owner identifier still requires fresh subject
bindings; it does not establish historical equivalence or reset a baseline.
The v1 schema and its exact packaged configuration/series checks remain supported
without migration or relaxed validation. Both versions require the same reviewed
compatibility tuple, payload checks and independently authenticated capture.

The draft-07 [schema](../../tools/quality/schema/rust-collector-delivery.schema.json)
defines `Manifest`, `Environment`, `Matrix`, and `CaptureIdentity`. Root validation
is `Manifest`; the others use their definition references. Unknown fields, missing
fields, duplicate JSON keys, nonfinite numbers and invalid digests are rejected by
the contract loader. Payload paths are unique, relative, traversal-free POSIX paths;
each tool must name an inventoried payload with the same hash. This shape/path
check does not replace P4/P5 extraction, symlink or byte-inventory verification.

| Identity | Required meaning |
| --- | --- |
| Package | Independent exact collector SemVer and source commit; payload path/hash/role; dependency and license inventory digests. |
| Core/protocol | Explicit candidate Core version, commit and binary SHA-256, plus development request/response, project request/response, adapter result and normalized evidence versions. No ranges, inferred latest version or policy fallback. |
| Tools/runtime | Driver, rustc, rustc driver library, llvm-cov, llvm-profdata and private Python paths, versions and SHA-256; additional runtime dependencies inventoried. Host probes must observe actual executable/library bytes, not copy manifest declarations. |
| ABI | Initial target only `x86_64-unknown-linux-gnu`; exact tested glibc, kernel and runtime dependency inventory digest. No guessed minimum or ABI family equivalence. |
| Capabilities | Unique metric name, supported/unsupported package capability, scope and reason. Runtime evidence still preserves all six capability states. A package declaration does not establish policy requiredness. |
| Measurement | Full native series object, normalized measurement-series identities, compiler commit/inventory schema, LLVM version, adapter/classifier/projection digests, normalization identity, configuration digest and source boundary. |

The [repository development matrix](../../tools/quality/rust-collector-compatibility.json)
remains empty by default. The published RC has a separate
[reviewed compatibility matrix](https://github.com/musutrade/Harness-Gate/releases/download/rust-collector-review-7165558-v1/compatibility.json)
and source-bound receipt; see [current compatibility](../release-status.md#compatibility).
It certifies the exact Core 0.4.0 and host row, not an inferred Core 0.4.1 or a
Linux distribution family. Source pins remain rustc commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM `22.1.6` and
`rustc-mir-block-inventory/3`. Synthetic fixture identities are not production
matrix receipts.

`preflight(manifest, matrix, observed)` requires a declared Core and exactly one
reviewed row matching the manifest SHA-256 and entire observed environment.
Manifest hashes use the repository canonical JSON encoding (`harness_evidence._canonical`).
Environment arrays retain order; reordering requires the same reviewed canonical
representation. A row binds an exact environment and a retained receipt path/hash
of kind `reviewed-native-capture-and-core-evaluation`. Changed package bytes,
Core/protocol, ABI or tools fail before a producer is launched. Declaring support
in a manifest alone is insufficient; an empty matrix permits no sampling.

The delivery lifecycle must authenticate both manifest and matrix, verify receipt
bytes and runtime probes, then call preflight before collection. This module
checks the contract, not signatures or capture anchors. Populate a row only after
review of real clean-host capture, complete retained-binary re-export and exact
released Core evaluation (including valid low coverage and blocking operational
negatives). Retain source, executable binaries, profiles, exports, inventory,
commands/results, tool hashes and independent anchors with the receipt. A missing
receipt, partial export or missing original binary is not positive native evidence.
Distribution signatures do not authenticate runtime captures.

## Relocation and series transition

Package-relative payload paths allow a future verified installation layout; they
do not make historical capture paths interchangeable. `CaptureIdentity` binds
measurement identities, absolute captured tool paths/hashes/versions, ABI,
flags, cfg/build inputs, source scope and hotspots. `require_same_capture_identity`
requires exact equality, rejecting relocation or changed tools without rewriting
the original. Existing native certification still checks original absolute paths.

There is no automatic transition override. A proposed transition requires a
separate reviewed record identifying old/new manifests, paths, all measurement
identities and source scopes; original anchored bytes; fresh complete native
captures and re-exports for both environments; measured ownership/denominator,
normalization/classification and tool equivalence or differences; and explicit
Core lineage/history handling approved under existing policy. Changed semantics
require a distinct series. Preserve original captures and anchors even if a
future transition is accepted. Never remove path identity, reseal history, alias
series or reset a baseline to obtain compatibility. The historical
`llvm-file-summary-unfiltered/1` series remains distinct from MIR production data.

Related authority decisions: [ADR-0040](../adr/0040-language-agnostic-evidence-policy.md)
and [ADR-0049](../adr/0049-project-owned-validation-extension-boundary.md).
Validation and limitations are retained in [GH-227 evidence](gh-227/validation.md).


## Installed generic invocation (GH-230)

The [capture binding schema](../../tools/quality/schema/rust-project-collector-binding.schema.json)
binds the authenticated generic request to native owners, the independent capture
anchor and digest-pinned delivery manifest/matrix. The host signs the exact
`collect --binding ABSOLUTE_PATH --binding-sha256 SHA256` arguments using existing
Core configuration and trusted request machinery. The collector observes exact
runtime/tool/Core/ABI identities before native re-export and normalized projection;
Core remains the sole policy authority. See the
[configuration fixture](../../tools/quality/tests/rust_collector_config_fixture.py)
and [local acceptance receipt](gh-230/acceptance.md) for executable examples, the
frozen candidate tuple, independent producer counts and retained original bytes.
Earlier P6 future-tense descriptions above are historical contract milestones.
Unknown shipped combinations still fail closed; fixture evidence does not authorize
publication, baseline acceptance, relocation equivalence or Arc-Admin integration.
