# Tasks: Fail-Closed Trust Boundaries

**Parent:** [proposal.md](proposal.md), [design.md](design.md),
[trust-boundaries specification](specs/trust-boundaries/spec.md), and
[ADR-0034](../../../docs/adr/0034-fail-closed-trust-boundaries.md)
**Status:** Implemented; acceptance evidence reviewed against PR #60 and CI
run `33434212725` on 2026-08-31.

Tasks are bounded to less than four hours. A task is checked only after its
focused tests pass and its acceptance evidence is reviewable.

## 1. Invocation Input (R-01)

- [x] **1.1 (P0, M)** Add the invocation input descriptor and Git index snapshot allocator.
  **Acceptance:** The descriptor owns its temporary root and records mode,
  project identity, source identity, execution root, and configuration digest.
- [x] **1.2 (P0, M)** Route hook configuration, scope, gates, and external steps through the snapshot root.
  **Acceptance:** No ordinary hook plan node obtains repository content from
  the working-tree root or inherited current directory.
- [x] **1.3 (P0, M)** Add opposing index/working-tree architecture, fmt, and lint fixtures.
  **Acceptance:** Staged-only failures fail; unstaged-only failures do not alter
  the hook result; machine metadata identifies the same snapshot.

## 2. Runtime Ownership (R-02)

- [x] **2.1 (P0, M)** Extend leases with canonical project, resource kind,
  invocation, runtime labels, and immutable object identity.
  **Acceptance:** The deterministic filename and all ownership fields validate
  together; existing records fail closed through an explicit migration path.
- [x] **2.2 (P0, M)** Add runtime inspect and pre-remove ownership verification.
  **Acceptance:** Cleanup invokes remove only after fresh inspection proves
  full label and immutable-ID equality.
- [x] **2.3 (P0, M)** Add forged, cross-project, malformed, active, renamed, and
  identity/label mismatch tests for fake Docker and Podman adapters.
  **Acceptance:** Every ambiguous case records failure and makes zero remove calls.

## 3. Confined Publication (R-04)

- [x] **3.1 (P0, M)** Implement the root-confined atomic publisher.
  **Acceptance:** Create-new sibling writes, sync, rename, containment, target
  type, and parent-component validation have focused tests.
- [x] **3.2 (P0, M)** Migrate reports, parsed logs, cleanup evidence, migrations,
  and generated schemas to the publisher.
  **Acceptance:** No predictable repository-adjacent output bypasses the shared boundary.
- [x] **3.3 (P0, S)** Add target/parent symlink, directory, collision, and
  normal replacement tests on supported platforms.
  **Acceptance:** External targets remain unchanged and normal readers never see partial content.

## 4. Closed Evidence (R-03)

- [x] **4.1 (P0, M)** Add the invocation artifact registry and declaration bindings.
  **Acceptance:** Required artifacts carry invocation, optional step, kind,
  normalized path, size, and digest.
- [x] **4.2 (P0, M)** Implement report/registry/manifest/disk closed-set validation and manifest-last publication.
  **Acceptance:** All four views agree before `evidence_complete` can become true.
- [x] **4.3 (P0, M)** Add missing, unlink-open, symlink, stale-invocation,
  undeclared-file, replacement, and digest-mismatch tests.
  **Acceptance:** Every mismatch blocks publication and leaves evidence incomplete.

## 5. Release Inventory (R-05)

- [x] **5.1 (P0, M)** Generate a deterministic machine-readable release inventory.
  **Acceptance:** Every platform binary and the CycloneDX SBOM has an explicit identity.
- [x] **5.2 (P0, M)** Drive checksum, signing, attestation, verification, and upload from the inventory.
  **Acceptance:** Independent globs do not define integrity or upload subject sets.
- [x] **5.3 (P0, M)** Add an offline release fixture for exact-set and tamper verification.
  **Acceptance:** Missing, extra, modified, unsigned, unattested, or unlisted assets block publication.

## 6. Closeout

- [x] **6.1 (P0, S)** Run locked tests, formatter, strict Clippy, dependency audit,
  contracts, documentation consistency, and the release fixture.
  **Acceptance:** Local checks and Linux/macOS/Windows required CI are green.
- [x] **6.2 (P0, S)** Synchronize ADR, README/configuration, schemas, changelog,
  and OpenSpec status with the implemented guarantees.
  **Acceptance:** ADR-0034 becomes Accepted only after every Phase 0 item and
  required CI check has evidence linked here.

## Evidence Review

Implementation is delivered in pull request
[#60](https://github.com/musutrade/Harness-Gate/pull/60). CI run
[33434212725](https://github.com/musutrade/Harness-Gate/actions/runs/33434212725)
passed the Linux, macOS, and Windows test matrices, CLI contracts, performance
baseline, release inventory, and Required Quality Aggregate. Local verification
also passed 215 Rust unit tests, 17 CLI contract tests, 11 integration tests,
formatting, strict Clippy, staged-hook verification, and the ten release
inventory tests. The local Windows cross-compilation check was unavailable
without Microsoft `lib.exe`; the supported Windows path is covered by CI.
