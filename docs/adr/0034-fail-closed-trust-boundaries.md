# ADR-0034: Enforce Fail-Closed Trust Boundaries

## Status

**Accepted** (2026-08-31)

## Context

Harness-Gate is intended to make a pass/fail decision from repository inputs,
execute configured work, preserve reviewable evidence, manage temporary
resources, and publish verifiable release assets. Those responsibilities cross
several trust boundaries: the Git index and working tree, repository-controlled
configuration, child processes, the report filesystem, a container runtime,
notification endpoints, and the release platform.

The consolidated audit of commit `97da753` found that the normal engineering
checks are healthy, but several boundaries do not yet fail closed:

- `hook` selects scope and secrets from the Git index while architecture gates
  and ordinary external steps can still read the working tree;
- a lease record is treated as sufficient authority to remove a named
  container without proving that the current runtime object has matching
  ownership labels and immutable identity;
- a successful step may reference a missing log while the invocation is still
  published with `evidence_complete = true`, because the report, manifest, and
  filesystem are not compared as one closed artifact set;
- standalone report writers can follow pre-existing symbolic links; and
- release metadata is selected by independent globs, allowing the SBOM to be
  uploaded without being included in the checksum, signature, and provenance
  operations.

These are architectural issues rather than isolated validation mistakes. A
local patch at only one call site would leave another consumer with a different
idea of the invocation input, resource owner, evidence set, or release asset
set. The project therefore needs explicit invariants shared across the
scheduler, process, service, report, installer, and release boundaries.

The threat model for these invariants includes a repository checkout whose
contents may be untrusted, execution on a shared CI host, and a Harness-Gate
process that may have report-directory, network, or container-runtime access.
The operating system account and the configured release trust roots remain
deployment responsibilities. Harness-Gate must not turn ambiguous repository
state into a pass or use repository-controlled metadata alone to authorize a
destructive external side effect.

This decision tightens ADR-0031 and implements the fail-closed direction in
ADR-0032. It also limits the security interpretation of ADR-0033: the current
adapter capability allowlist is a protocol-level declaration check, not an
operating-system network, filesystem, resource, or process sandbox.

## Decision

Harness-Gate will adopt a single invocation trust context and enforce the
following five release-blocking invariants. A boundary validation error is a
verification or publication failure; it is never downgraded to a successful
result, incomplete warning, or best-effort destructive action.

### 1. Bind every node to one immutable invocation input

Every verification invocation has an explicit input descriptor containing at
least:

- input mode (`working-tree`, `staged`, `base`, or `all`);
- canonical project identity and project root;
- immutable source identity where the mode provides one, such as a Git tree or
  index snapshot digest;
- invocation ID and effective configuration digest; and
- the execution root from which gates and external steps read repository
  content.

For `hook`, Harness-Gate materializes the complete Git index as an isolated
invocation input. Scope selection, effective configuration, built-in scans,
architecture rules, and ordinary external steps use that same input root.
Repository discovery may locate the checkout and configuration path before the
snapshot is created, but it must not make the working-tree version of a file
the effective hook input.

A step that intentionally requires the original repository or direct Git-index
access must declare that requirement through a reviewed execution capability.
It cannot acquire different input semantics merely because its current working
directory happens to be the checkout. Reports record the input mode, source
identity, and configuration digest so the decision can be reproduced.

### 2. Treat leases as claims and runtime identity as authority

A lease reserves a logical resource identity; it does not by itself authorize
deletion of an external object. Managed runtime resources carry ownership
metadata that binds all of the following:

- Harness-Gate owner marker and schema version;
- canonical project identity;
- logical resource ID and kind;
- invocation ID; and
- immutable runtime object ID once the resource exists.

The lease filename must be the deterministic key for the recorded resource ID.
After creation, the runtime adapter inspects the object, records its immutable
identity, and verifies the expected labels. Before cleanup or stale reclaim, it
inspects the object again and compares the immutable ID and complete ownership
metadata. A missing label, filename mismatch, changed object ID, malformed
record, failed inspection, or ambiguous owner prevents deletion and produces a
reportable cleanup failure.

Staleness is separate from ownership: expiry may make a proved-owned resource
eligible for reclaim, but never proves ownership. A heartbeat covers the whole
managed-resource lifetime. Platforms without Linux process start identity must
use a platform-appropriate process identity or retain the resource when liveness
cannot be established safely.

### 3. Publish evidence as a closed, invocation-bound set

The invocation owns an artifact registry. Each node declares its required logs
and artifacts using normalized paths relative to the invocation report root.
Before a pass is published, every declared artifact must:

- exist as a regular file;
- resolve below the canonical invocation report root without crossing a
  symbolic link;
- be bound to the current invocation and, where applicable, step ID; and
- have its type, size, and digest recorded.

Finalization compares three sets: artifacts declared by results, artifacts in
the registry/manifest, and publishable files on disk. The sets must agree after
explicitly documented internal temporary-file exclusions. Missing, unexpected,
escaped, replaced, or digest-mismatched evidence sets
`evidence_complete = false` and fails publication. A successful child exit
cannot override an evidence failure.

The manifest is generated only after redaction and artifact validation, and is
published last. It is the integrity description of the finalized invocation,
not a best-effort directory listing. Manifest verification repeats the closed-
set comparison rather than validating only entries that happen to be listed.

### 4. Route repository-adjacent output through a safe publisher

All reports, extracted logs, migration results, cleanup evidence, schemas, and
other Harness-Gate-owned outputs use one filesystem publication abstraction.
That abstraction:

- accepts an explicit allowed output root;
- rejects symbolic links and non-regular targets or path components;
- creates a non-colliding temporary file in the destination directory with
  create-new semantics;
- writes and synchronizes complete content before publication;
- replaces only the destination directory entry rather than following its
  target; and
- atomically renames the temporary file into place where the platform permits.

Security claims must match the primitive used on each platform. If the
implementation cannot prevent a same-user concurrent path substitution, it
must either use directory-handle-relative/no-follow operations or document and
reject that deployment model. Callers do not bypass this abstraction with
direct `fs::write` for predictable repository-adjacent paths.

### 5. Derive release operations from one explicit asset inventory

The release workflow constructs one machine-readable inventory of publishable
assets. Platform binaries, the CycloneDX SBOM, checksum manifest, and other
release metadata are selected explicitly; independent shell globs are not the
source of truth.

Every applicable asset in that inventory is covered by the required checksum,
Sigstore signature/certificate, and provenance or attestation operation. The
workflow verifies those products and compares the inventory with the upload
set before creating or updating a release. A missing, extra, unsigned,
unattested, or unverifiable asset blocks publication.

The publish job also requires the repository's test, formatting, lint,
dependency-audit, contract, and documentation gates. Release credentials are
available only in a protected publication environment, and third-party Actions
used on a privileged path are pinned to reviewed commit identities.

### Supporting trust-boundary rules

The five invariants above block release until implemented. The following rules
complete the same architecture in the next delivery stage:

1. Adapter signing continues to authenticate the adapter executable
   declaration in protocol version 1. The request file is trusted orchestration
   input. Before requests may originate from an untrusted repository or enter
   the verification DAG, a new protocol version must sign the canonical full
   request and bind nonce, validity window, invocation, step, configuration
   digest, arguments, environment, capabilities, timeout, input, and artifact
   root.
2. Adapter capabilities remain protocol allowlists unless a platform sandbox
   maps them to enforceable OS policy. Documentation must say so. Process-group
   termination is a bounded cleanup attempt, not proof of complete descendant
   containment. Captured output uses byte limits, disk quotas, and reader
   deadlines so an escaped descendant cannot retain a pipe indefinitely.
3. Audit, parsed-log, report, webhook, and release metadata outputs use one
   redaction policy before leaving the invocation boundary.
4. Installers download into new temporary files and verify version, checksum,
   signature, issuer, and repository identity before installation. Mutable
   remote scripts are not the recommended installation boundary.
5. Network destinations derived from project configuration are subject to an
   explicit deployment policy. A deployment that accepts untrusted
   configuration defaults to denying loopback, link-local, and private network
   destinations unless an administrator allowlists them.

Typed failure codes, structured configuration diagnostics, parser strictness,
polling and scheduler performance, and repository hygiene remain important
follow-up work. They do not weaken or replace the five trust-boundary
invariants.

## Consequences

### Positive

- A hook result describes the content being committed rather than an accidental
  mixture of index and working-tree state.
- Repository metadata cannot independently authorize deletion of an external
  runtime object.
- A `PASS` with `evidence_complete = true` proves that the declared invocation
  evidence existed, was confined to the invocation, and matched its manifest
  at finalization.
- Predictable output paths no longer provide a supported route to overwrite a
  symbolic-link target outside the output root.
- Release consumers can verify that binaries and the SBOM belong to the same
  reviewed publication set.
- Adapter documentation distinguishes executable authenticity, request trust,
  process cleanup, and OS sandboxing instead of combining them into one
  ambiguous isolation claim.

### Negative

- Staged hooks require temporary storage and extra Git I/O. Tools that assume
  the original checkout as their working directory may require an explicit
  execution capability or configuration change.
- Strict evidence finalization can turn an otherwise successful command into a
  failed verification when its log is missing or altered. This is intentional
  but may expose previously tolerated step behavior.
- Cleanup retains resources when ownership cannot be proved. Operators need a
  separate, explicitly authorized manual recovery procedure for such orphans.
- Runtime adapters must support inspection and immutable object identities, and
  existing leases/resources may require migration rather than automatic
  reclaim.
- Safe cross-platform filesystem publication and bounded process containment
  require more implementation and test complexity than ordinary path-based
  writes and process groups.
- Release publication becomes dependent on the complete quality pipeline and
  external signing/attestation services, so an unavailable trust service blocks
  release instead of silently weakening it.
- A future fully bound adapter request requires a protocol version change and a
  signer/verifier migration.

## Alternatives Considered

- **Patch only the reproduced call sites:** rejected because other gates,
  report writers, resource types, or release steps could retain different trust
  semantics. The invariants must be owned by shared boundaries.
- **Define `hook` as a working-tree check:** rejected because the command is a
  commit gate and already selects staged scope. Working-tree state is neither a
  stable nor accurate description of the proposed commit.
- **Run only built-in gates against the staged snapshot:** rejected because a
  pass would still combine different source states when external fmt, lint,
  compile, or test nodes use the checkout.
- **Trust a lease marker and managed-name prefix:** rejected because both are
  repository-writable claims and do not identify the current runtime object.
- **Build the manifest from whatever files remain:** rejected because absence
  from a directory listing cannot prove that required evidence was produced.
- **Report incomplete evidence as a warning:** rejected because a successful
  gate without its required evidence is not auditable and must not satisfy a
  fail-closed policy.
- **Keep per-command output writers with local symlink checks:** rejected because
  path safety and atomic publication would drift across commands and platforms.
- **Use release globs with additional comments or naming conventions:** rejected
  because filename punctuation already caused security operations and upload
  operations to select different sets.
- **Treat adapter capability declarations as an OS sandbox:** rejected because
  validation of names does not enforce network, filesystem, or resource access.
  A real sandbox remains a separate, platform-specific design decision.
- **Make a full adapter sandbox a P0 prerequisite:** rejected for this decision
  because the adapter entry point is explicit and opt-in, while the five P0
  failures affect ordinary gate or release paths. The project must narrow its
  claims now and may adopt enforceable sandboxing through a later ADR.

## Rollout and Verification

Implementation is staged so each boundary can be reviewed independently while
preserving the final invariants.

### Phase 0: Release blockers

1. Introduce the invocation input descriptor and staged snapshot execution
   root. Add opposing index/working-tree fixtures for architecture, formatting,
   and lint nodes.
2. Extend the runtime adapter with inspect/identity operations and bind leases
   to immutable runtime IDs and complete labels. Prove that forged, cross-
   project, malformed, active, and label-mismatched leases never invoke remove.
3. Add the invocation artifact registry, closed-set finalization, and
   manifest-last publication. Test missing logs, symlinks, stale invocation
   files, undeclared artifacts, and digest changes.
4. Migrate every predictable output to the safe publisher and test target and
   parent-component symlinks on each supported platform.
5. Replace release globs with an explicit asset inventory and prove offline
   verification for every binary, the SBOM, checksum manifest, signatures, and
   provenance before upload.

### Phase 1: Boundary completion

- version and bind adapter requests before accepting untrusted request input;
- align README, configuration docs, ADR-0033, and OpenSpec with the actual
  protocol-level capability and process-lifecycle guarantees;
- add output, log, disk, and reader-deadline budgets;
- apply redaction to standalone audit and log parsing;
- verify installer downloads and remove mutable-script installation guidance;
  and
- gate privileged release jobs on the complete quality pipeline.

### Acceptance evidence

The Phase 0 invariants were implemented in pull request
[#60](https://github.com/musutrade/Harness-Gate/pull/60), and the following
evidence was reviewed before accepting this ADR:

- regression tests demonstrate both failure and success paths for each of the
  five invariants;
- Linux, macOS, and Windows test/contract matrices pass where the boundary is
  supported, with an explicit fail-closed result for unsupported behavior;
- local Rust tests (215 unit, 17 CLI contract, and 11 integration tests),
  formatting, strict Clippy, staged-hook verification, and the ten release
  inventory tests pass;
- CI run
  [33434212725](https://github.com/musutrade/Harness-Gate/actions/runs/33434212725)
  passes the Linux, macOS, and Windows matrices, CLI contracts, performance
  baseline, release inventory, and Required Quality Aggregate checks; and
- public documentation contains no unsupported OS-sandbox or complete-process-
  tree containment claim.

The local Windows cross-compilation check could not run because the host lacks
Microsoft `lib.exe`; the Windows implementation and tests are covered by the
green CI matrix. Production release and canary evidence remain operational
follow-up records and are not prerequisites for this architectural decision.

## Related

- [OpenSpec: Fail-Closed Trust Boundaries](../../openspec/changes/fail-closed-trust-boundaries/proposal.md)
- [ADR-0025: Establish Phase 1 Quality Baseline Gates](0025-phase-1-quality-baseline-gates.md)
- [ADR-0027: Unify Built-in Gates and Configured Steps in a Verification Plan](0027-unified-verification-plan.md)
- [ADR-0031: Harden Gate Boundaries and Delivery Contracts](0031-harden-gate-boundaries.md)
- [ADR-0032: Define Harness-Gate Capability Contracts and the DevRail Boundary](0032-harness-gate-devrail-capability-contracts.md)
- [ADR-0033: Signed Out-of-Process Adapter Protocol](0033-signed-out-of-process-adapter-protocol.md)
- [Machine-result JSON Schema](../../schema/machine-result.schema.json)
- [Artifact Manifest JSON Schema](../../schema/artifact-manifest.schema.json)
