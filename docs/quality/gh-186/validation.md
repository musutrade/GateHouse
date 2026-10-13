# GH-186: Configured CI integration evidence

Historical submission record. GH-198's [final hosted reconciliation](../gh-187/hosted-closure.md)
supersedes the pending-at-submission status below with run 34419625673, attempt 1,
and successful Required Quality Aggregate job 102695650692.

Scope: OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 8.1–8.4.
See [ADR-0048](../../adr/0048-configured-ci-workflow-acceptance.md). Tasks 8.1–8.2
have local implementation evidence; hosted-dependent tasks 8.3–8.4 remain pending.
This record does not accept the whole proposal or claim a green hosted aggregate.

## Integration and assurance

The required Linux Test job exports and validates a receipt from its normal
nextest invocation and uploads `workflow-acceptance-<run>-<attempt>` for 30 days.
The matrix in `tools/quality/fixtures/workflow/ci-matrix.json` selects component,
capability, policy, producer, series and profile data. The Rust fixture driver and
Python receipt validator contain no ecosystem dispatch. There are no new jobs. The synthetic matrix is Linux-only; existing native
macOS/Windows tests remain unchanged. The existing Linux coverage invocation also
runs this test as part of its normal suite; that adds test work, not another
authoritative measurement collection. Hosted cost must include both jobs.
The repository itself has no project-local quality configuration; the test creates
isolated temporary configured projects, not a fabricated root gate.

Twelve cases cover Rust, Angular/reference, mixed Angular+Rust and an unregistered
`nebula-unregistered-2049` ecosystem, each passing, violating policy and missing
required capability data. The unknown shape uses bundle-size policy and arbitrary
producer/tool/runtime/series metadata through exactly the same pipeline. Rust
coverage/CRAP and Angular's optional unsupported CRAP states are explicit fixture
data. These are synthetic protocol/policy tests, not native tool measurements.
They supplement the unchanged native platform and reference/certification suites.

For each case, the signed producer launches once. Its executable is then removed;
collection and normal `verify` reuse the pinned response and raw artifacts. The
ledger remains at one launch, even after negative verification. Direct released
Rust evaluation and verify have identical complete project-report values (gates,
blockers, remediation, indexes and aggregate). Metadata, capability and series
preservation is asserted separately. Forty negative checks (ten per passing
shape) reject digest, binding, schema, invocation, series, subject, capability,
artifact, source and missing-envelope changes, yielding a blocked FAIL report.
Unknown custom metric semantics are not silently accepted; the existing collector
suite also exercises configured custom metrics reaching the core rejection path.

Production ownership remains `quality-coverage -> ci_quality.py collect -> seal`.
Generic Shadow validates the independent manifest digest before reference
projection. Neither the acceptance receipt validator nor the aggregate collects
measurements. Frozen topology/artifact/aggregate tests retain missing, failed,
cancelled and skipped-child negatives and immutable inventory validation. Linux,
macOS and Windows nextest, native push builds/contracts, tool versions and
Required Quality Aggregate's lightweight step allowlist are unchanged. The new
Rust test module is excluded from production coverage and changed-source risk.
No compiled cache, build-once scheme, baseline reset or threshold change is added.

## Retained evidence and local commands

`local-acceptance.json.gz` contains the exact local receipt (deterministic gzip).
`local-summary.json` records its uncompressed SHA-256 and measured test time.
Receipt file entries retain base64 bytes plus SHA-256 for configurations, source,
artifacts, trusted state/keys and direct reports. Positive and negative unified
reports are embedded. Temporary absolute paths are intentionally preserved;
empty hosted identity fields identify local execution, not a hosted attestation.
These fixture keys are public test keys, not deployment credentials.

All 379 Rust tests and 346 Python tests passed, as did fmt, Clippy, docs
consistency and strict OpenSpec validation. `validation.json` lists exact commands,
exit codes and full-log hashes. Complete
logs are uncommitted under `target/quality/gh-186/`. Cargo-dependent commands use
`CARGO_TARGET_DIR=$PWD/target` because the default target is read-only. The initial
focused nextest command without that override failed with
`failed to open /home/gem/cargo-target/debug/.cargo-build-lock: Read-only file system (os error 30)`;
no tests ran in that attempt. The default `.git` is also mounted read-only:
`git add .github/workflows/ci.yml` failed to create `.git/index.lock`. Delivery
uses temporary Git metadata in `target/quality/gh-186/delivery.git` with this same
workspace as its work tree; the mounted local Git refs cannot be updated. Root `harness-gate config check` and
`harness-gate verify --profile ci --all` are not applicable: this checkout has no
`.harness-gate/flow.toml` declaring a CI profile.

## Hosted cost and controller acceptance

The accepted [hosted cost model and rollback record](../ci-topology/after-state.md)
retains baseline runs 34303413318, 34301967575 and 34291282719 and after runs
34312685548 and 34311132169, including successful historical aggregates, job/step
identity, runner work by OS and tool/cache/upload costs. Those cohorts do not
measure GH-186. They support keeping ineffective compiled caches and build-once
experiments reverted. This change adds one acceptance test and receipt validation/
upload to Linux Test; it adds no measurement job. Test elapsed seconds are not
billed minutes, total job duration or a causal performance estimate.

After the final push, the controller owns CI observation and acceptance. Retain
this head's completed run/attempt and jobs, checkout SHA, required conclusions,
receipt/summary and artifact digest. Normalize the hosted record with:

```bash
python3 tools/quality/ci_timing.py --input <retained-hosted-input.json> --output <hosted-timing.json>
```

Use the existing `ci-topology/hosted-after-input.json` envelope and frozen topology
contract; distinguish PR head from GitHub's tested merge SHA. Compare required
critical path, total and per-OS runner wall minutes, Test duration and receipt
validation/upload overhead with the retained cohort. Report changes in source,
compiler and runner contention; do not infer causality from one run. Reject or
rework avoidable added cost if assurance benefit does not justify it. The upload
step uses the existing timing normalizer's artifact category. Check all required
children and Required Quality Aggregate at this submitted SHA. Only then can
8.3–8.4 be checked off. This unattended implementation does not poll CI, and no
hosted cost improvement or aggregate success is asserted here.
