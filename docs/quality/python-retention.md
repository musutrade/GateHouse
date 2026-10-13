# Python retention and Rust product boundary

GH-152 applies the final dispositions after GH-151's authority transfer, merged
as PR #158 at `63a7f9538b13d4f8888eff6b35663cda40270e70` with successful required
CI. The [original inventory](gh-146/python-boundary.md) remains historical evidence;
the [final inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-152/python-boundary.json) governs current retention.

The released `harness-gate` binary uses its own `harness_gate::quality` library
module (production sources in `tools/harness-gate/quality-core`) and owns
normalized evidence validation, project/subject identity, typed policy evaluation,
requiredness, baseline/lineage/debt/ratchet and exception semantics, cross-component
contracts, aggregation and project reports. `harness-gate quality evaluate` takes
explicit trusted context and does not require Python or fall back to it. The
legacy report field `mode: shadow` is a compatibility field, not an authority switch.
Native tools and external collectors measure; they cannot choose final decisions.
The separate unpublished `quality-replay` package (historically named
`harness-gate-quality-core`) is a development comparator, not a release dependency.
Repository release eligibility still requires the existing `Required Quality
Aggregate` and all its dependencies. A generic pass cannot waive those checks.

## Final C-class dispositions

All eight modules are frozen at the source hashes in the final inventory.
Their accepted generic behavior is retained solely for compatibility replay,
adapter preflight and rollback investigation. They are not a second product core.

| Module | Retained role | Authoritative replacement / boundary |
| --- | --- | --- |
| `harness_evidence.py` | Evidence oracle and adapter normalization helpers | Rust evidence, schema and measurement integrity |
| `project_model.py` | Project/subject oracle and adapter identity helpers | Rust project and subject validation |
| `policy_engine.py` | Frozen policy oracle | Rust policy evaluation and aggregate requiredness |
| `policy_ratchet.py` | Frozen baseline/lineage/debt/exception oracle | Rust ratchet semantics |
| `cross_component.py` | Frozen contract/provenance oracle | Rust cross-component validation |
| `project_report.py` | Frozen configuration/report oracle | Rust generic project reports |
| `collector_runner.py` | Python process transport with frozen reference preflight | Transport may remain Python; Rust independently validates normalized evidence and decides |
| `quality_evidence.py` | Frozen shared schema helper and legacy `complexity-evidence` validation | Generic `harness-evidence/v1` authority is Rust; legacy Rust measurement validation remains required Python tooling |

The Python policy and project-report CLIs now require `--reference-only`.
Their exit codes describe reference replay outcomes and cannot approve a release.
Library APIs remain available to retained tests and adapters. Required tooling
uses `quality_evidence.py` only for legacy measurement integrity; it does not
import the Python generic decision engine. Regression checks pin the frozen
sources and inspect that required-tool dependency boundary. The shipped CLI
corpus test runs Rust evaluation with an empty executable search path.

## A/B/D retention and retirement

- **A — CI/dev tooling:** retain Python for CI orchestration, documentation checks,
  benchmarks, coverage, risk and CLI contracts. Maintain and test it normally.
  Existing required checks retain their blocking responsibilities; language
  uniformity is not a reason to rewrite them.
- **B — Ecosystem adapters:** retain Python/native-tool parsing and normalization
  behind collector protocol v1. Adapter maintenance may extend measurement support
  through separately reviewed contracts. Calls to frozen helpers are preflight
  or normalization only; Rust revalidates inputs. Development contract replay
  helpers that call Python policy/reporting remain reference-only.
- **D — Migration/reference tooling:** freeze the accepted corpus, expected outputs,
  measurement identities and historical acceptance records. Keep replay drivers
  runnable for regression and rollback; allow portability/security repairs with
  unchanged accepted semantics and full differential evidence. Add new behavior
  in Rust with separately reviewed fixtures, never by refreshing the old oracle
  to hide a mismatch. Advisory replay success has no release-approval path.

C-class hash changes require a reviewed explanation and retained before/after
corpus and negative-matrix evidence; changing a hash alone is not acceptance.
Transport or legacy-measurement fixes in the mixed modules must establish that
generic oracle semantics did not change. Product behavior changes belong in Rust
and a separately reviewed contract delta, not in the frozen Python reference.

Retire a C/D implementation only in a follow-up review once every live replay,
adapter and test caller has a replacement, equivalent positive/negative coverage
is retained, and the accepted rollback no longer needs that implementation.
Archive the source/hash, corpus and decision evidence before removal; historical
accepted evidence and baselines are never regenerated or deleted to pass a gate.
Do not automatically select Python when Rust fails. Rollback follows the
[GH-151 reviewed release rollback](gh-151/README.md#rollback) and required CI.

This is a runtime-authority consolidation, not additional ecosystem certification.
The bounded [TypeScript/Angular acceptance](typescript-certification.md) and
[Rust acceptance](rust-equivalence-acceptance.md) retain their original limits;
Python/Java/Go and arbitrary native tools gain no certification from this change.
