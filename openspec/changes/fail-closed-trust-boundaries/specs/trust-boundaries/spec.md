# Fail-Closed Trust Boundaries Specification

## ADDED Requirements

### Requirement: One immutable invocation input

The system SHALL create one invocation input descriptor before constructing a
verification plan. For hook invocations, it SHALL materialize the complete Git
index and SHALL use that root for effective configuration, scope, built-in
gates, architecture rules, and ordinary external steps. It SHALL record the
input mode, immutable source identity, and configuration digest.

#### Scenario: Staged violation differs from working tree

- **WHEN** the index contains an architecture, formatting, or lint violation
- **AND** the corresponding working-tree file is clean
- **THEN** hook verification fails using the staged content
- **AND THEN** its report identifies the staged source identity

#### Scenario: Working-tree violation is not staged

- **WHEN** the index is clean and the working tree contains a violation
- **THEN** hook verification is unaffected by the unstaged content

### Requirement: Runtime removal requires proved ownership

The system SHALL treat a lease as a claim rather than removal authority. Before
removing or reclaiming a runtime object, it SHALL compare the deterministic
lease filename, canonical project identity, resource kind and ID, invocation
ID, complete runtime labels, and immutable runtime object ID. Any missing,
malformed, failed, or mismatched proof SHALL prevent removal and produce a
structured cleanup failure.

#### Scenario: Forged stale lease names another container

- **WHEN** a stale lease is forged, renamed, or points at an object whose labels
  or immutable ID differ
- **THEN** cleanup does not invoke the runtime remove operation
- **AND THEN** the lease and failure evidence remain available for inspection

#### Scenario: Proved-owned stale object is reclaimed

- **WHEN** a stale lease filename and record agree with the inspected runtime
  labels and immutable object ID
- **THEN** cleanup removes that object and only then removes the lease

### Requirement: Confined atomic output publication

The system SHALL route predictable reports, extracted logs, migration output,
cleanup evidence, and generated schemas through a shared publisher. The
publisher SHALL confine paths below an explicit root, reject symlink or invalid
components and targets, write a new sibling temporary file completely, and
atomically publish it where supported.

#### Scenario: Output target is a symlink

- **WHEN** a predictable output target or parent component is a symbolic link
- **THEN** publication fails without modifying the link target

#### Scenario: Normal output is published

- **WHEN** all parent components are confined directories and the destination
  is absent or a regular file
- **THEN** readers observe either the prior complete file or the new complete file

### Requirement: Evidence forms a closed invocation-bound set

The system SHALL register required invocation artifacts and SHALL compare
result declarations, registry entries, manifest entries, and publishable disk
files before reporting complete evidence. Every artifact SHALL be a confined
regular file bound to the current invocation and SHALL have a recorded kind,
size, and SHA-256 digest. The manifest SHALL be published last.

#### Scenario: Required log is missing or replaced

- **WHEN** a successful step's required log is missing, a symlink, stale,
  escaped, or digest-mismatched
- **THEN** the invocation fails publication
- **AND THEN** `evidence_complete` is false

#### Scenario: Disk contains undeclared evidence

- **WHEN** a publishable file exists without a matching declaration and
  registry entry
- **THEN** closed-set validation fails and no successful manifest is published

### Requirement: One explicit release asset inventory

The release system SHALL generate one machine-readable inventory and SHALL use
it for checksums, signatures, certificates, provenance, verification, and
upload. The applicable subject set for every integrity operation SHALL equal
the final upload set. A missing, extra, unverifiable, unsigned, or unattested
asset SHALL stop release creation.

#### Scenario: SBOM is included in every integrity operation

- **WHEN** release metadata is generated for platform binaries and a CycloneDX SBOM
- **THEN** the binaries and SBOM are present in the checksum, signature,
  certificate, provenance, verification, and upload subject sets

#### Scenario: Inventory and upload differ

- **WHEN** an unlisted file is selected for upload or an inventory member lacks
  required integrity metadata
- **THEN** release publication fails before credentials create or update a release
