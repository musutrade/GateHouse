# Frozen generic-core compatibility corpus

GH-146 freezes OpenSpec `consolidate-generic-quality-core-into-rust` task 1.2.
This is a non-authoritative Python oracle, prior to implementing the Rust
candidate or performing differential/authority-transfer acceptance (tasks 2–7).
The existing Rust required CI path remains authoritative under ADR-0040.

Run from the repository root:

```bash
python3 tools/quality/fixtures/generic-core/replay.py --output target/quality/generic-core-replay.json
```

No Cargo, Node, Angular build, coverage collection, Git checkout, network access,
or original collection directory is needed. Python and the existing quality
modules replay the complete normalized inputs against retained source/artifact
bytes. The unit suite also runs this replay and rejects changed golden files.

## Inputs and provenance

[manifest.json](manifest.json) enumerates each input, expected output, originating
retained run, explicit mutation, validation outcome and aggregate state. It pins
compressed/input/output bytes and existing external schema files with SHA-256.
`origins` identifies the retained collection artifacts and their hashes; these
are provenance, not replay dependencies. `artifacts.tar.gz` stores deduplicated
source and raw artifact bytes under their SHA-256 names. Replay verifies every
member before reconstructing per-case head/base roots in a temporary directory.
Large JSON files use deterministic gzip storage; `gzip -dc FILE.json.gz` exposes
the full reviewable JSON. Small JSON files remain directly readable.

Each `generic-core-case/v1` input carries `head` and optional `base`, with exact
project, evidence records, caller-owned expected commit/base/target/run context,
and source/artifact path-to-blob maps. Top-level policy, optional selection,
mappings, exceptions and fixed UTC `now` are explicit. An omitted base or
selection is intentional; replay must not infer it from records. Source/artifact
roots are transport locations reconstructed from the maps, never trusted from
collector-controlled absolute paths. Expected outputs contain evidence validation,
full `harness-policy-results/v1` and `harness-project-report/v1` objects, or the
exact validation/evaluation error class and message. Error messages, gate reasons,
blockers, comparisons, debt, lineage, indexes and evidence links are all frozen.

## Coverage and bounded claims

- Both GH-96/GH-97 retained Rust candidates: production and base/head risk
  projections, preserving the existing counters, identities and native results.
- GH-134 real Angular compatible/regression base/head pairs: exact normalized
  coverage counters, absolute threshold and legacy/regressed debt outcomes.
- GH-133 compatible and breaking provider/consumer contracts: full relationships,
  local green components, cross-component blocking and artifact provenance.
- Native-derived Rust and Angular negative mutations: missing evidence, stale
  caller context, changed artifact bytes, missing source, duplicate subject,
  malformed typed value, unknown capability and required unsupported capability.
- Angular missing base, incompatible normalization series, invalid exception
  metadata and empty caller selection; contract unknown-producer rejection.

Native adapter parsing negatives continue to live in the retained adapter suites.
This corpus does not claim the later consolidated negative matrix or Rust
candidate acceptance is complete. Rust and Angular coverage here is bounded to
the retained runs and accepted measurement series; no new ecosystem is certified.

## Canonicalization and change control

The canonical value encoding is UTF-8 JSON with sorted object keys, two-space
indentation, `ensure_ascii=False`, `allow_nan=False`, and one trailing LF.
Array order, null versus omitted fields, integer counters, decimal strings,
ratio numerator/denominator, IDs, digests, states and reasons are preserved.
No rounding, timestamp masking, sorting arrays or mismatch allowlist is permitted.
The sole path canonicalization replaces the replay-owned temporary case directory
with `$CASE_ROOT` in `reason` strings from OS missing-file diagnostics. The
remaining path, error number and message are exact; structured evidence links
and all other strings are unchanged. Temporary root locations are reconstructed;
output evidence paths remain the original logical links. The exception-review
clock is the input ISO-8601 UTC value, converted to a timezone-aware datetime.
Comparison is byte-exact after gzip decompression and canonical serialization
of the actual Python result. Gzip headers are not machine semantics but their
stored bytes are also pinned for integrity.

External schemas remain in `tools/quality/schema`, pinned by the manifest;
project/evidence/policy inputs use their existing v1 contracts. Result/report
schemas are identified by their existing v1 fields and frozen complete goldens.
The corpus/manifest/replay v1 identifiers describe test transport only, not a new
product API. A schema or golden change requires an explicit reviewed compatibility
delta and new retained validation evidence. Replay never updates goldens. Keep
this corpus through authority transfer and freeze it afterward until replacement
coverage is reviewed; do not delete native evidence or baselines during migration.

## Rust differential replay (GH-150)

Tasks 5.1–5.4 add `differential.py`, which verifies this same frozen oracle before
passing explicit context and retained bytes to the Rust candidate example.
See [commands, comparator rules and acceptance evidence](../../../../docs/quality/gh-150/README.md).
The original Python replay above remains available. No golden/schema bytes,
required gates or release authority change. Both the Rust replay and the Python
reference remain non-authoritative; authority transfer is still tasks 6–7.

## Profiling replay transport

`profile_oracles.py` runs fresh policy, evidence, replay and differential workers
with separate instrumented and ordinary samples. It is opt-in diagnostic tooling;
see the [GH-291 measurement record](../../../../docs/quality/ci-topology/issue291.md).
Transient equality uses compact sorted UTF-8 JSON while persisted canonical
serialization, frozen expected bytes and every semantic comparison stay unchanged.
