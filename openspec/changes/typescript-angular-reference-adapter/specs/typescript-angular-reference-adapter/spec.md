## ADDED Requirements

### Requirement: Validate a real second ecosystem
The adapter SHALL collect from a runnable locked TypeScript/Angular application
using real build, test and coverage tools, retaining versions, configuration,
invocations, source bytes and raw artifacts for the requested base/head/run/target.
Synthetic envelopes alone SHALL NOT satisfy second-ecosystem acceptance.

#### Scenario: Replay real frontend collection
- **GIVEN** a locked application with tested and untested production branches
- **WHEN** the adapter normalizes its retained coverage output
- **THEN** normalized covered/total counters equal the raw measurements
- **AND** generic policy can reproduce pass and deliberate regression outcomes.

### Requirement: Preserve source identity and explicit capability limits
The adapter SHALL produce unambiguous versioned source-backed subjects and
validate source-map and artifact integrity before policy use. It SHALL declare
every requested capability with the existing capability states. Unsupported
template, branch or risk measurements SHALL NOT become numeric defaults.

#### Scenario: Ambiguous or stale transformation
- **GIVEN** same-named methods or transformed output with stale or ambiguous source maps
- **WHEN** original source identity cannot be established
- **THEN** collection fails closed before favorable policy or baseline use.

#### Scenario: Exact original TypeScript counters
- **GIVEN** a mapped source-backed file, function or method
- **WHEN** line and function coverage are normalized
- **THEN** integer covered/total counters retain native statement-start line and function-hit semantics
- **AND** an empty denominator is unavailable with no numeric value.

#### Scenario: Bound transformation inputs
- **GIVEN** caller-bound original bytes, emitted bytes, parser identity and test source maps
- **WHEN** a basename join, changed artifact, incomplete map or ambiguous original mapping is supplied
- **THEN** measurement fails before any favorable policy or baseline use
- **AND** template/generated multi-source measurements remain unsupported without a separately accepted generic amendment.

#### Scenario: Unmeasured template or risk
- **GIVEN** a requested metric outside the accepted adapter capability matrix
- **WHEN** the response is validated and a blocking policy requires support
- **THEN** the explicit unavailable state is retained and the gate cannot pass.

### Requirement: Reuse generic policy with distinct measurement series
The adapter SHALL emit validated collector-protocol and harness-evidence/v1
records without final delivery decisions. Existing generic policy, lineage,
ratchet and aggregation code SHALL own thresholds and debt. TypeScript series
SHALL remain distinct from Rust and from incompatible frontend tool versions.

#### Scenario: Mixed subject request under collector v1
- **GIVEN** caller-owned file, function, method and source-backed route subjects
- **WHEN** one v1 capability set is requested through the generic collector runner
- **THEN** every returned record explicitly declares every requested capability
- **AND** route execution and unmeasured metrics remain unsupported without numeric defaults
- **AND** capability omission or a per-kind capability object is rejected.

#### Scenario: Bound retained frontend replay
- **GIVEN** a caller-owned receipt binding raw archive/index digests, native revision and replay scope
- **WHEN** current sources/configuration, tool success, maps and provenance validate
- **THEN** both runner transports yield the same normalized facts with unchanged native artifacts
- **AND** failed tools, timeout, malformed output, stale/tampered or undeclared artifacts,
  source-map failures and provenance mismatch yield no usable evidence batch.

#### Scenario: Complete TypeScript series identity
- **GIVEN** compiler, builder, runner, provider, mapping/rule and normalization versions
- **AND** runtime/target, configuration and source-boundary semantics
- **WHEN** any semantic field changes, or a Rust series is supplied as the baseline
- **THEN** generic series compatibility fails and explicit baseline migration is required.

#### Scenario: Instrumentation upgrade
- **GIVEN** a base measurement using different coverage or mapping semantics
- **WHEN** a head measurement is compared without explicit compatible baseline acceptance
- **THEN** the comparison fails closed without losing historical debt.

### Requirement: Exercise real cross-component contract failure
The reference fixture SHALL include real provider/client contract-tool artifacts
and explicit participating components. Generic project aggregation SHALL retain
their provenance and block incompatible contracts.

#### Scenario: Green components with a breaking interface
- **GIVEN** passing frontend and Rust component gates and a real breaking OpenAPI change
- **WHEN** generic policy evaluates compatibility and generated-client drift evidence
- **THEN** the contract gate blocks project pass with raw evidence links.

#### Scenario: Contract measurement cannot establish trustworthy facts
- **GIVEN** failed contract measurement, missing artifacts, a stale generated client,
  or tool/invocation/component provenance that does not match the measured subjects
- **WHEN** the adapter and generic gates evaluate the contract
- **THEN** the project cannot pass, and tool-specific parsing remains outside generic policy.

### Requirement: Record and review architectural mismatches
Every discovered mismatch SHALL retain a reproducer, affected generic contract,
expected/actual behavior and disposition. Required core changes SHALL receive
reviewed spec deltas plus Rust and frontend validation before adoption. Adapters
SHALL NOT conceal mismatches with language-specific policy or fabricated identity.

#### Scenario: Existing subject contract cannot express a measurement
- **GIVEN** an unresolved TS-01, TS-02, TS-03 or newly discovered mismatch
- **WHEN** acceptance is reviewed
- **THEN** the affected capability remains uncertified until evidence resolves it
- **AND** deferral explicitly narrows the support matrix.

### Requirement: Bound certification and preserve release authority
Acceptance SHALL retain two real base/head pairs, one pass and one intentional
regression, plus negative integrity, identity, capability, series and collection
failure cases. Certification SHALL name the exact fixture, versions, environment,
series and supported capabilities. Required CI SHALL pass before closure.
Rust required-check identity and authority SHALL remain unchanged.

#### Scenario: Advisory frontend run succeeds
- **GIVEN** a successful frontend acceptance window
- **WHEN** support status is documented
- **THEN** only evidenced capabilities are certified
- **AND** no required gate, other ecosystem or unmeasured platform is enabled.

#### Scenario: Opt-in retained advisory replay and rollback
- **GIVEN** the locked retained coverage window and native contract artifacts
- **WHEN** the separate manual advisory workflow is dispatched
- **THEN** the existing adapter/evidence/policy flow and negative fixtures replay without duplicate native collection
- **AND** machine-readable reports and raw evidence are retained even on failure
- **AND** disabling the workflow or opt-in adapter selection preserves evidence and accepted baselines
- **AND** Rust required dependencies, aggregate identity and release authority remain unchanged.

## Implementation and review

Task 4.3's [final review](../../../../../docs/quality/gh-136/README.md) links
tasks 1.1–4.2 to merged PRs, successful required CI on their submitted SHAs and
retained native evidence. The [certification matrix](../../../../../docs/quality/typescript-certification.md)
bounds the exact fixture, toolchain, environment, series and capabilities.
No generic amendment is introduced. Review completion does not close the
follow-up before required CI passes on the final review SHA and controller
acceptance; all deferred capabilities and Rust release authority remain unchanged.

Follow the ordered sub-four-hour [tasks](../../tasks.md); their acceptance
criteria define the implementation milestones. Protocol usage is:

```text
caller-bound request -> real tools / retained raw output
  -> validated harness-evidence/v1 -> existing generic policy -> project report
```

The [design](../../design.md) records alternatives and rollback: disable the
advisory adapter/job while preserving evidence, baselines and Rust authority.
