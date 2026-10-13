# Reproducible function risk evidence

`tools/quality/risk.py` implements OpenSpec tasks 4.1–4.5 of
[`strict-json-results-and-risk-based-quality-gates`](../../openspec/changes/strict-json-results-and-risk-based-quality-gates/tasks.md).
It consumes retained base/head measurement bundles and emits `risk.json` and
`risk.md`. This is an incremental gate, not an accepted production baseline.
GH-96 integrates the validated GH-94 series in required CI; see
[ADR-0039](../adr/0039-required-risk-and-traceability-gates.md). This page preserves
the version 1 bundle interface. Whole-project acceptance remains separate. The policy owner supplies current
mandatory source identities and reviewed boundary-test links.

## Run

```bash
python3 tools/quality/risk.py \
  --base target/quality/base/bundle.json \
  --head target/quality/head/bundle.json \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" \
  --policy target/quality/risk-policy.json \
  --output target/quality/risk.json
```

Both SHAs must be full commit IDs present in the current repository. Source
snapshots are compared byte-for-byte to immutable Git objects; another checkout
is not read. Missing base evidence is a measurement error. The exit code is 1
for a measurement error or gate failure, and 0 for `pass` or `pass-with-debt`.
The latter explicitly preserves historical violations and is not full acceptance.

## Bundle schema 1

Every field below is required. An artifact descriptor is
`{"path":"relative/path","sha256":"<64 lowercase hex characters>"}`.
Artifacts must be nonempty and remain within the bundle directory after path
resolution. Their SHA-256 digests are verified before use.

| Field | Value |
| --- | --- |
| `schema_version`, `commit` | `1`, full expected Git SHA |
| `target` | Actual target triple |
| `tools` | Nonempty version strings for exactly `rustc`, `llvm_cov`, `llvm_profdata`, `cargo_llvm_cov`, `nextest`, `python` |
| `profile`, `instrumentation` | `dev`, `-C instrument-coverage` |
| `branch` | Exactly `{"status":"unsupported","reason":"<tool limitation>","tool_version":"<llvm_cov version>"}` |
| `source_root` | Bundle-relative retained crate directory with complete `src/**/*.rs` |
| `coverage_root` | Original absolute crate path embedded in LLVM/LCOV; used only to resolve source identities |
| `inventory` | Artifact descriptor for the complete production coverage inventory |
| `complexity` | Artifact descriptors for per-source analyzer records, with matching commit and source bytes |
| `llvm`, `lcov`, `cobertura` | Artifact descriptors extended with `run_id` and `profiles_sha256` |
| `run` | Run provenance described below |

The original LLVM JSON and LCOV can remain byte-identical when a retained source
snapshot is relocated. `coverage_root` maps their original filenames to the
retained source tree; it is not a directory to read source files from.
Production coverage validation independently checks inventory completeness,
source exclusions and LLVM/LCOV counters. Cobertura is retained and checked for
a parseable coverage root; CRAP is calculated from LLVM source regions.

`run` requires:

- `id`: a unique run identifier containing only letters, digits, `_` or `-`;
  `started_ns` and `finished_ns`: positive ordered timestamps.
- `commit`, `target`, `instrumentation`: matching bundle values;
  `status: "passed"` and the actual nonempty test `command`.
- `results`: artifact descriptor for nextest JUnit. At least one test must have
  executed successfully. Failed cases or suites invalidate the run. Test IDs
  are unique `classname::name` pairs; skipped tests cannot satisfy boundary links.
- `binaries`: entries with unique `id`, `role` (`cli` or `test`) and `artifact`.
  Both roles are required. Retained executables must have a supported executable
  header and LLVM profiling sections.
- `profiles`: entries with `run_id`, `binary_id` and `artifact`. Every binary
  needs a profile. Each `.profraw` path must include the run-ID directory, carry
  an LLVM raw-profile header, and have an mtime within the recorded run interval.
  Retention must preserve these timestamps.

Each coverage export references that run and the digest returned by
`risk.digest(run["profiles"])`: SHA-256 of sorted-key, compact JSON encoded as
UTF-8. This ties exports to a specific retained profile set. The collector must
record true commands, versions and binary/profile provenance. These checks
detect stale, absent, mixed and corrupt declared evidence; they are not a signed
attestation or a re-execution of the collector. Raw-profile headers are checked,
not fully decoded by this Python consumer. Production collection and baseline
review must retain the corresponding LLVM merge/export commands and results.

## Source mapping and metrics

The series records schema 1, analyzer 0.1.1, its rule/toolchain, mapping
`function-location-1`, formula `crap-line-1`, policy `incremental-risk-1`,
inventory/metric versions, target, measurement tools, profile, instrumentation
and branch status. Base/head series must match exactly. A tool or counting-rule
change requires a separately reviewed baseline; old evidence is never silently
reinterpreted. Source hashes and commit IDs are provenance, not series keys.

The consumer regenerates complexity records from retained source bytes, then
joins LLVM function-region envelopes to unique source spans using UTF-8 byte
columns. Qualified names and locations distinguish same-name functions.
Generic instantiations and closures keep their raw instance indexes and names,
but counters deduplicate by source location using the maximum hit count. An
unexecuted function must still appear with zero counters. Missing complexity,
missing LLVM functions, ambiguous mapping and unsupported expansions fail closed.

Line coverage is the union of executable intervals on each physical line.
Nested regions override enclosing counters, skipped/gap intervals contribute no
executable lines, and child closure spans are removed from their parent's
counts. Function coverage counts one source function after instance deduplication;
region coverage counts distinct executable regions. Each is reported separately.
Branch coverage is explicitly unsupported for this series, without invented
zero/100 percentages. A branch-capable series needs a reviewed implementation
and baseline; missing branch support never disables line or region gates.

The analyzer is lexical with the syntax limits documented in
[its contract](complexity-analyzer.md). Macro expansions, nested items that the
analyzer does not emit, or LLVM spans that cannot be joined are measurement
errors. This implementation does not claim every Rust syntax form is supported.

For each source function, with `cov = covered executable lines / executable lines`:

```text
crap_line = CC² × (1 − cov)³ + CC
```

`CC=10, cov=80/100` produces 10.8. The JSON includes a numeric `crap_line` and
`crap_line_exact` numerator/denominator. All decisions use the exact rational
value; only Markdown display is rounded. Every row retains source identity,
raw complexity counts, source digest, instance/region details and links/digests
for the raw LLVM report, complexity artifact and bundle.

## Incremental policy schema 1

The following arrays are required even when empty:

```json
{
  "schema_version": 1,
  "mandatory_symbols": [],
  "hotspot_symbols": [],
  "identity_mappings": [],
  "boundary_tests": [],
  "exceptions": []
}
```

Use exact analyzer symbol IDs for selections. The six hotspots listed in the
OpenSpec design are always selected. A selected built-in hotspot disappearing
from its original path/name requires a reviewed mapping to its successor, which
retains the selection. An empty policy cannot erase these built-in selections.

Automatic identity matching requires a unique path, qualified name and symbol
kind. Unchanged function bytes tolerate line shifts; source moves/renames require
`{"base":"<old ID>","head":"<new ID>","reason":"<review rationale>"}`
in `identity_mappings`, or are treated as new functions. Mappings are one-to-one;
split functions cannot all inherit one historical identity. Mapped moves are
rechecked even when the body is identical. Removed identities remain in the report.

Every new, modified or moved function must have `crap_line <= 30`. A function
with `CC > 10`, or selected as a hotspot/mandatory symbol, also requires line
coverage and region coverage independently at least 80%, plus a passing linked
boundary test. Selected functions remain blocking even when unchanged. Other
unchanged violations are `debt`, listed separately, never function passes.

Each `boundary_tests` entry requires `symbol`, `test_id`, a nonempty failure
`observable`, `status: "passed"`, the head `commit`, `target`, `run_id`, and
`results` equal to the head run's retained JUnit artifact descriptor. The test
must actually occur as passed in that artifact. Review must establish that the
named test exercises the stated observable; this script cannot infer assertions
from a test name.

Each exception requires nonempty `symbol`, `issue`, `owner`, `approver`, `reason`,
`expires` (ISO date) and `compensating_controls`. An exception expiring on or
before the evaluation date fails the gate. A valid exception remains visible
but never waives CRAP, coverage or boundary-test failures, consistent with
[ADR-0025](../adr/0025-phase-1-quality-baseline-gates.md).

See [GH-92 validation](gh-92-validation.md) for fixtures, actual check results and
the distinction between tooling evidence and production acceptance.
