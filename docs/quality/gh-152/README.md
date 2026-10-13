# GH-152 Python demotion and consolidation closure

Scope: OpenSpec tasks 7.1–7.3 and final closure preparation. No collector protocol,
Rust product semantics, metric thresholds, workflow dependencies or ecosystem
certification changes are introduced.

## Predecessor acceptance

[GH-151 / PR #158](https://github.com/musutrade/Harness-Gate/pull/158) merged at
`63a7f9538b13d4f8888eff6b35663cda40270e70`, this issue's starting baseline.
The [observed GitHub record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-152/predecessor-acceptance.json) pins head
`8059d70d488fb5b3755cde248b43dee4d9f29d92`, merge identity and successful required
checks, including `Required Quality Aggregate`, Test and Generic Quality Shadow.
This satisfies the predecessor dependency; it does not claim CI for this PR.

## Final dispositions and boundary

The [final inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-152/python-boundary.json) covers all 31 production Python modules
and freezes all eight C-class source hashes. Each module explicitly declares its
non-authoritative reference status. Python policy/report CLIs require
`--reference-only`; existing explicit reference invocations retain their contract
outputs and exit semantics. Implicit decision use fails before input/output access.
Required measurement imports cannot reach Python generic decisions. The frozen
legacy complexity-evidence validator remains a required measurement helper, and
Python collector transport remains supported.

The [retention policy](../python-retention.md) specifies A/B maintenance and C/D
freeze, repair, rollback and retirement requirements. The release binary owns
all final generic decisions through `harness-gate quality evaluate`; Python
references cannot approve releases or serve as automatic fallback. Existing
required CI retains release eligibility responsibility. The bounded Rust and
TypeScript/Angular evidence gains no additional ecosystem certification.

## Validation and final acceptance

The [validation record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-152/validation.json) and [complete logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-152/validation.tar.gz)
record exact commands, results and environment limitations. Baseline Python
validation passed 293 tests before edits. Final checks cover frozen-source drift,
required Python import boundaries, CLI reference opt-in, both Rust/Angular corpora,
negative cases, Python regressions, formatting, Clippy, documentation consistency
and strict OpenSpec validation. All 332 Rust tests and 296 Python tests passed;
formatting, Clippy, documentation consistency and strict OpenSpec validation
also passed.

The first unmodified Cargo test command failed because the shared Cargo cache is
read-only when downloading `tinyvec-1.13.2.crate` (OS error 30). The retry uses
workspace-local `CARGO_HOME` and `CARGO_TARGET_DIR`; shared cache and build paths
are not modified. This checkout has no `.harness-gate/flow.toml` declaring `ci`:
`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed. No generic configuration was invented.

The workspace Git metadata is also read-only (`.git/index.lock`, OS error 30).
Commit/push use a copy of this workspace's metadata under
`target/quality/gh-152/delivery-git` and the same `symphony/GH-152` branch.
The handoff pins the pushed commit; original metadata remains at the baseline.

Task 7.3's final required-CI acceptance and full change closure remain conditional
on successful hosted checks for this PR and controller merge. The named OpenSpec
change remains in place for strict validation; it is not prematurely archived or
marked fully accepted. The controller owns final acceptance, merge and issue
closure. Historical stage records retain their original submission-time status.

## Risks and rollback

Existing manual callers of the frozen Python decision CLIs must add
`--reference-only` for replay or move authoritative evaluation to the Rust CLI.
No accepted JSON shape changes. Freeze checks intentionally require explicit
review for future C-source edits, including transport/legacy fixes. Retirement
must preserve original evidence and provide replacements for all live callers.
Rollback follows [GH-151](../gh-151/README.md#rollback); a Rust error must never
silently fall back to favorable Python output.

Related records: [ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md),
[architecture](../architecture-closure.md),
[proposal](../../../openspec/changes/consolidate-generic-quality-core-into-rust/proposal.md),
[design](../../../openspec/changes/consolidate-generic-quality-core-into-rust/design.md),
[tasks](../../../openspec/changes/consolidate-generic-quality-core-into-rust/tasks.md).
