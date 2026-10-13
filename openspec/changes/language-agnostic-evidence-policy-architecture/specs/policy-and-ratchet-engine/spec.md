# Capability: Policy and Ratchet Engine

## ADDED Requirements

### Requirement: Evaluate generic metrics independently of language
Harness-Gate SHALL evaluate policies against normalized metric/capability names and subject/component scope without requiring language-specific policy branches for equivalent semantics.

#### Scenario: Apply the same CRAP policy across ecosystems
- **GIVEN** normalized Rust, TypeScript, Python, and Java subjects with `risk.crap` metrics in compatible policy contexts
- **WHEN** a maximum-CRAP policy is evaluated
- **THEN** the same policy evaluator processes all subjects
- **AND** language-specific measurement differences remain encoded in their series rather than policy branches.

### Requirement: Support baseline/head ratchet evaluation
The policy engine SHALL compare compatible base/head evidence for changed subjects and SHALL be able to prevent newly introduced or worsened debt while retaining unchanged historical debt as explicit debt/informational evidence.

#### Scenario: Historical debt improves but remains above threshold
- **GIVEN** an unchanged-series subject with base CRAP 64 and head CRAP 55
- **WHEN** a policy allows improvement of legacy debt
- **THEN** the result records improvement and remaining debt
- **AND** does not falsely claim that the subject meets an absolute threshold of 30.

#### Scenario: New regression remains below absolute threshold
- **GIVEN** a changed subject with base CRAP 18 and head CRAP 27
- **WHEN** `deny_regression` is enabled with absolute maximum 30
- **THEN** the policy records a regression according to the configured ratchet semantics
- **AND** does not ignore the regression solely because 27 is below 30.

#### Scenario: Compatible lineage determines classification without sharing split debt
- **GIVEN** validated base/head projects, evidence, and explicit subject mappings
- **WHEN** an incremental rule evaluates subjects
- **THEN** exact IDs are unchanged and one-to-one modify/rename/move mappings are modified
- **AND** unmapped identities and split children are new for policy and inherit no debt allowance
- **AND** missing base, unavailable prior values, ambiguous mappings or incompatible series prevent incremental pass.

#### Scenario: Improved legacy debt coexists with a new threshold failure
- **GIVEN** compatible base CRAP 64 and head CRAP 55, plus a new subject at 31
- **WHEN** maximum 30 allows legacy improvement and denies regression
- **THEN** the ledger records improved remaining debt with absolute compliance false
- **AND** the new subject fails and blocks the aggregate.

### Requirement: Preserve distinct result states
Policy evaluation SHALL preserve at least `pass`, `fail`, `warning`, `informational`, `unsupported`, `not_applicable`, `measurement_error`, and `blocked` states. Required gates SHALL define which non-pass states block delivery.

#### Scenario: Measurement failure on required evidence
- **GIVEN** a required policy whose evidence is `measurement_error`
- **WHEN** aggregate evaluation runs
- **THEN** delivery is blocked fail-closed
- **AND** the result remains distinguishable from a threshold `fail`.

### Requirement: Exceptions document failures but do not silently waive them
Policy exceptions SHALL retain issue/owner/approver/reason/expiry/compensating-control metadata and SHALL NOT automatically convert failed measurements into passing evidence unless a separately defined governance policy explicitly authorizes such behavior.

#### Scenario: Valid temporary exception exists
- **GIVEN** a valid, unexpired exception record for a failed metric
- **WHEN** policy evaluation runs under the current Harness-Gate exception model
- **THEN** the failure and exception are both reported
- **AND** the underlying gate is not silently changed to pass.

#### Scenario: Invalid exception metadata cannot obscure quality failure
- **GIVEN** a failed quality result with exception metadata
- **WHEN** owner, issue, reason, expiry or compensating control is missing, blank or expired
- **THEN** the combined aggregate records a metadata measurement error
- **AND** the original failed result and quality aggregate remain failed
- **AND** a supplied approver is retained as metadata without granting waiver authority.

### Requirement: Produce machine-readable remediation context
A failed or blocked policy result SHALL identify the affected component/subject, metric, observed values, base/head context when applicable, governing policy, evidence references, and accepted remediation classes when they are known.

#### Scenario: Agent consumes a CRAP failure
- **GIVEN** a changed function fails a CRAP policy
- **WHEN** the result is serialized
- **THEN** it includes the subject identity, base/head CRAP, threshold/ratchet rule, evidence links, and remediation classes such as reducing complexity or increasing meaningful coverage.

### Requirement: Use closed typed policies and caller-owned scopes
Policies SHALL use typed limits and a fixed comparison operator set, with no
arbitrary code DSL. Project, component, boundary, changed-subject and
critical-subject selectors SHALL evaluate caller-owned declared subjects in the
requested target. Missing required subject measurements SHALL NOT disappear.

#### Scenario: A collector omits a selected subject
- **GIVEN** a required project policy and a declared subject absent from evidence
- **WHEN** all returned measurements satisfy their thresholds
- **THEN** the missing subject produces `measurement_error` and prevents success.

#### Scenario: Required child cannot be erased by aggregation
- **GIVEN** required children in `fail`, `cancelled`, `measurement_error`, `blocked`, or `skipped`
- **WHEN** aggregation runs
- **THEN** the aggregate remains non-pass and retains each original child state
- **AND** only policy-owned `required: false` exempts these children from blocking
- **AND** informational children do not block.

#### Scenario: All generic metric types share one evaluator
- **GIVEN** synthetic coverage, CRAP, mutation, breaking-change counts and boolean metrics
- **WHEN** typed limits are evaluated across heterogeneous subjects
- **THEN** the same exact comparison implementation evaluates each measurement
- **AND** unavailable values are never converted into numerical success.

### Requirement: Define and retain Rust equivalence acceptance
Rust equivalence acceptance SHALL require the two hash-pinned GH-96 and GH-97
retained runs at distinct head commits in the GH-118 window manifest, with their
original base/head/run identities, and the complete adversarial adapter suite
without skips. Any difference in gate outcomes, debt classification, unsupported
states or measurement-error semantics SHALL block acceptance. This retrospective
acceptance SHALL NOT change release authority or accept a new baseline.

#### Scenario: Replayed window and negative measurement fixtures
- **GIVEN** both pinned archives and the versioned evaluator
- **WHEN** the acceptance runner evaluates the window and negative fixtures
- **THEN** it retains evaluator hashes, per-run results and fixture outcomes
- **AND** missing or duplicated runs, altered archives or failed fixtures prevent acceptance.

#### Scenario: Green aggregate conceals unsupported-state drift
- **GIVEN** an optional Rust measurement is unsupported
- **WHEN** a generic result changes it to success or measurement error, or omits the gate
- **THEN** equivalence acceptance fails even if the aggregate would otherwise pass.
