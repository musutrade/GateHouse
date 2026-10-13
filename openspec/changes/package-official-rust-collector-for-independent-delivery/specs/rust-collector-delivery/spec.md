# Capability: Rust Collector Independent Delivery

## ADDED Requirements

### Requirement: Independent immutable bundle
The official collector SHALL have an independent exact version and self-contained declared runtime for the certified Linux x86_64 host ABI, without a source checkout or global toolchain mutation.

#### Scenario: Clean installation
- **GIVEN** a supported host has no Harness-Gate source tree or Python installation
- **WHEN** the verified bundle is installed
- **THEN** doctor and real fixture collection use the private runtime; unsupported ABI fails before sampling.

### Requirement: Delivery manifest compatibility
The signed manifest SHALL bind payload identities, tools, capabilities and explicit Core/protocol compatibility. Package SemVer SHALL NOT imply measurement-series compatibility.

The [GH-227 contract](../../../../../docs/quality/rust-collector-delivery-contract.md)
specifies the strict schema and exact reviewed matrix. Unknown combinations SHALL
fail before sampling. Declared versions alone SHALL NOT establish tested support.

#### Scenario: Malformed manifest or wrong runtime tool
- **GIVEN** missing/unknown manifest fields, duplicate keys, unsafe payload paths, or mismatched observed tool bytes/version
- **WHEN** delivery preflight runs
- **THEN** it rejects the input before any producer launch and supplies no usable measurement evidence.

#### Scenario: Unknown Core or ABI combination
- **GIVEN** no unique reviewed receipt matches the exact manifest, Core binary/commit/version, protocols, tools and host ABI
- **WHEN** preflight evaluates the combination
- **THEN** sampling is blocked, including when the tested matrix is empty; no version or policy fallback is selected.

#### Scenario: Relocated historical capture
- **GIVEN** a capture records different tool paths or incompatible tool/series identities
- **WHEN** comparison or re-export is requested
- **THEN** the incompatible history is rejected without rewriting its anchor or baseline.

#### Scenario: Proposed series transition lacks complete evidence
- **GIVEN** relocation or repackaging is proposed without retained original bytes, independent anchors, fresh native re-exports and reviewed identity/lineage comparison
- **WHEN** historical compatibility is requested
- **THEN** the transition remains rejected; package version/path changes do not establish equivalence, reset a baseline or alias a measurement series.

### Requirement: Measurement-only transport
The collector SHALL reuse the existing v1 request/response and normalized evidence contracts, preserving all error and capability states. Final quality decisions SHALL remain in released Rust Core.

#### Scenario: Valid low coverage
- **GIVEN** complete authenticated collection has low coverage
- **WHEN** the collector responds and Core evaluates
- **THEN** valid facts are delivered without a collector release verdict and Core retains the real policy failure.

### Requirement: Operational failure remains blocking
The product entry SHALL distinguish measurement completion from operational failure without translating arbitrary legacy nonzero exits to success.

#### Scenario: Failed legacy subprocess
- **GIVEN** a producer fails and leaves partial output
- **WHEN** the standalone collector processes its result
- **THEN** no usable success evidence is emitted and required verification remains blocked.

### Requirement: Checked installation lifecycle
Installation SHALL verify exact-tag origin, complete inventory, checksums and provenance before safe extraction and atomic activation. Rollback and uninstall SHALL preserve project evidence and unrelated assets.

#### Scenario: Interrupted or malicious installation
- **GIVEN** an active verified version exists
- **WHEN** installation is interrupted or the archive contains traversal or undeclared payloads
- **THEN** activation is refused and the prior version and project evidence remain unchanged.

#### Scenario: Package supplies its own verifier
- **GIVEN** no independently authenticated host verifier and public key have been provisioned
- **WHEN** a package offers a verifier or runtime alongside its signature
- **THEN** that code SHALL NOT establish the package's root of trust; installation remains blocked until host trust is supplied independently.

#### Scenario: Interrupted removal with unrelated evidence
- **GIVEN** removal has a durable ownership journal and unexpected project evidence exists inside the version tree
- **WHEN** recovery resumes
- **THEN** cleanup refuses the unexpected tree, preserves the evidence and does not broaden manifest ownership.

#### Scenario: Independently provisioned standalone bootstrap
- **GIVEN** a host without Python or a source checkout and an administrator-authenticated launcher, capsule digest and host trust
- **WHEN** the bootstrap capsule is missing or its private snapshot fails that independent digest
- **THEN** installation rejects it before extraction or execution; a positive installation requires separately retained fresh-host evidence.

### Requirement: Protected independent publication
Publication SHALL use an explicit independent inventory, SBOM and protected release approval while preserving existing Core required checks.

#### Scenario: Explicit single-maintainer approval
- **GIVEN** the reviewed single-maintainer exception for the canonical collector environment and pinned owner `higoalespn` (GitHub user ID `23396976`)
- **WHEN** that owner manually approves a run they triggered
- **THEN** a v2 eligibility receipt SHALL record the single-maintainer mode and exact owner; required manual review, no administrator bypass, exact-main CI and all signature/inventory checks SHALL remain mandatory.

#### Scenario: Personal approval identity or receipt mismatch
- **GIVEN** an incorrect/additional reviewer, wrong environment, administrator bypass, or a personal-mode environment in a v1 receipt
- **WHEN** release eligibility or installed provenance is checked
- **THEN** verification SHALL reject; no general self-review fallback or independent-review claim SHALL be inferred.

#### Scenario: Incomplete release assets
- **GIVEN** a required signature, attestation or inventory subject is missing
- **WHEN** RC publication is attempted
- **THEN** publication fails without weakening checks or substituting unsigned assets.

#### Scenario: RSA alone does not satisfy production Sigstore verification
- **GIVEN** a production inventory with a valid RSA signature but no valid Sigstore bundle for the exact approved workflow identity and OIDC issuer
- **WHEN** production verification runs with an independently authenticated verifier and trusted root
- **THEN** verification fails, including for a wrong certificate identity, missing inclusion evidence or tampered inventory; synthetic verifier tests SHALL NOT establish real Sigstore acceptance.

#### Scenario: Unsigned candidate preparation
- **GIVEN** real pinned build inputs but incomplete production approvals
- **WHEN** preparation inventories the unsigned archive, manifest and SBOM
- **THEN** the receipt identifies the actual bytes as ineligible and SHALL NOT invent successful CI, licensing, compatibility, environment or signing evidence.

#### Scenario: Nonpublishing rehearsal
- **GIVEN** the GH-229 workflow has read permissions and requires the existing protected release environment
- **WHEN** its delivery rehearsal uses a disposable key and synthetic payloads
- **THEN** it creates no tag or release and does not claim production eligibility, protected approval or native measurement from the local rehearsal.

### Requirement: Capture trust and retention
Delivery signatures SHALL NOT substitute for host capture authentication. Native re-export acceptance SHALL retain all original required bytes under reviewed anchors.

#### Scenario: Missing original binaries
- **GIVEN** a historical report is available but original binaries are absent
- **WHEN** native re-export acceptance is evaluated
- **THEN** archive replay is identified as such and does not satisfy fresh re-export acceptance.

### Requirement: Bounded rollout
RC delivery SHALL NOT transfer project authority, accept baselines, complete GH-215 or certify additional platforms. Stable promotion SHALL await separately accepted fresh Arc-Admin integration.

#### Scenario: Installed RC but incomplete project inputs
- **GIVEN** the RC is installed but trusted workflow state or lineage is missing
- **WHEN** the project attempts required verification
- **THEN** verification stays blocked and its original required gates remain authoritative.

## Implementation and review

Sequence and effort are in [design](../../design.md) and [tasks](../../tasks.md).
The design includes proposed command examples, alternatives, success metrics and rollback.
This capability adds delivery constraints; existing collector protocol requirements are unchanged.
The development v1 protocol and released Core project-collector/adapter envelopes
remain distinct as inventoried in the contract; integration must validate the
applicable envelope without moving final policy authority. Relevant Engineering
Policy semantics remain unchanged. Synthetic contract tests are not native positive
measurement or independently delivered Core/ABI certification.
