# Rust reference adapter and shadow comparison

GH-116 implements OpenSpec tasks 7.1–7.5 through
[`rust_reference.py`](../../tools/quality/rust_reference.py). Run it against an
existing retained `ci_quality.py` candidate, using that candidate's original
identity and a fresh output directory outside the candidate directory:

```bash
python3 tools/quality/rust_reference.py \
  --candidate target/quality/retained/candidate.json \
  --head-sha "$HEAD_SHA" --base-sha "$BASE_SHA" --run-id "$RUN_ID" \
  --output target/quality/rust-shadow
```

This command verifies the candidate's required stage set, identities and artifact
hashes, then projects retained production and base/head risk reports. It reads
source bytes from the retained archives and binds every projected subject to its
source digest. Original runner paths in production reports are resolved by
artifact name inside the retained candidate, never by accessing that runner.
The command invokes no subprocess, compiler, tests, LLVM collection or analyzer.
It writes separate project, policy and evidence JSON files plus `shadow.json`.
The candidate, raw artifacts, accepted baselines and required CI remain unchanged.

## Preserved facts and series

Production line, function and region covered/total integers become exact ratios.
Boundary blocking flags and the original line threshold determine the compiled
policy. File digests, boundary rows, percentages and all native counters remain
in `subject.metadata.rust_native`, encoded as JSON text because native reports
include display floats. Aggregate and boundary subjects bind to the retained
production report; file and function subjects bind to source bytes. Native
inventory/metrics versions, inventory digest and Rust toolchain identify the
production series; the complete native series is retained in project metadata.

Risk rows retain native names, kinds, spans, syntax/source hashes, LLVM instance
and raw region counts, CC, `crap_line` display values and `crap_exact` integer
pairs. `risk.crap` uses an exact rational value with a positive denominator;
repeating fractions must not be rounded into decimal thresholds. Rational and
decimal CRAP series remain distinct. The adapter verifies CC from raw syntax
counters and CRAP from raw line counts. Original analyzer, rule, instrumentation,
mapping, selection and tool provenance enter the generic series fingerprint.
Base/head generic series must match. Only the current risk contract and the
explicitly frozen pre-selection GH-96/GH-97 contract are recognized; this does
not establish compatibility between those series.

`shadow.json` retains the native comparison's subject identities and decisions
verbatim, including duplicate-syntax queue matching and source/span changes.
Generic subject IDs are source-bound envelopes; they do not replace native
identity matching. The adapter compiles the existing Rust policy: selected
functions and changed functions with CC >10 require 80% lines, 80% regions and
CRAP <=30; other changed functions require CRAP <=30; unchanged nonselected debt
remains accepted and visible. It does not apply the separate generic no-regression
ratchet to Rust, because that would change the accepted Rust contract.

Branch capability is always `unsupported`, without a numeric value. Positive
coverage denominators are `supported`; empty counts are `not_applicable` and
their native 0/0 counters remain metadata. Missing, malformed, stale or tampered
required inputs produce `measurement_error`, never fabricated measurements.

## Authority and compatibility

The current candidate verifier, production threshold evaluator and native risk
comparison run against the retained inputs. Generic production outcomes and
every risk identity, debt decision and outcome must match. The legacy coverage
and isolated critical-path matrix stage results pass through unchanged; this
adapter does not redefine those metrics or rerun their collectors. Failed,
cancelled, skipped and errored required stages keep the combined result failed.

Any mismatch sets `compatible: false`, `migration_blocked: true` and records a
compatibility failure. A matching failed gate is compatible but still blocks
migration. The CLI exits zero only when compatibility holds and the current Rust
candidate passes. These results are shadow evidence, not baseline acceptance or
permission to replace any required job. [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md)
and the [migration inventory](migration-compatibility.md) retain authority.

## Historical fixtures

The [fixture manifest](../../tools/quality/fixtures/rust-reference/history.json)
pins existing archive hashes instead of copying or rewriting historical inputs.
The GH-97 candidate replays all 212 functions, including 75 debt classifications,
and production raw counts. Explicit derivations exercise a failed production
threshold without changing counts, missing/tampered measurement inputs,
unchanged debt and low/high-complexity changed-function ratchets. Tests also
inject generic-policy disagreement and required-stage failure states.

The accepted Phase 1 performance baseline predates the production/risk candidate
contract. Its fixture preserves its bytes, commit and series and confirms that
the adapter rejects it as incompatible. GH-97 is a retained passing candidate,
not an accepted production baseline. No fixture invents missing historical
coverage or implies a baseline approval. See the
[validation record](gh-116-validation.md) for actual checks and limitations.

## Generic shadow rollout and acceptance

GH-118 adds an advisory workflow that downloads the current collection artifact
and uses the same base/head/run identities. The [defined two-run acceptance
window](rust-equivalence-acceptance.md), [validation evidence](gh-118-validation.md)
and [ADR-0040](../adr/0040-language-agnostic-evidence-policy.md) record completed
local equivalence validation. Rust remains the sole required release authority;
changing that authority requires a separate reviewed rollout.
