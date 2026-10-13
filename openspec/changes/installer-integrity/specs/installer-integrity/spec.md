# Installer Integrity

## ADDED Requirements

### Requirement: Installer uses an immutable explicit version

The installer SHALL require a strict `v`-prefixed SemVer and SHALL construct
all release and source URLs from that exact tag. It SHALL NOT discover or
execute a mutable `latest` or `main` path.

#### Scenario: Missing version fails closed

- **WHEN** the installer is invoked without `--version` or
  `HARNESS_GATE_VERSION`
- **THEN** it exits before creating a download or changing the destination

### Requirement: Integrity precedes publication

The installer SHALL verify the selected binary's SHA-256 manifest entry and
Sigstore certificate. Verification SHALL require the token.actions issuer and
the exact Harness-Gate release workflow and version-tag identity.

#### Scenario: Modified binary is not installed

- **WHEN** the downloaded binary differs from its manifest digest
- **THEN** installation fails and an existing destination target is unchanged

#### Scenario: Wrong signing identity is rejected

- **WHEN** the certificate issuer, repository workflow, or tag identity does
  not match the expected release
- **THEN** installation fails before destination publication

### Requirement: Publication is confined and atomic

The installer SHALL reject symlink path components, symlink targets,
non-regular targets, unsafe destination permissions, and traversal. It SHALL
publish through a create-new sibling and same-directory rename only after all
integrity checks pass.

#### Scenario: Symlink target is preserved

- **WHEN** the destination executable is a symlink
- **THEN** installation fails and the symlink target remains unchanged

## Implementation Plan

| Phase | Scope | Exit evidence |
| --- | --- | --- |
| 1 | Script and offline shims | Shell contract fixture passes |
| 2 | Documentation and CI | No mutable install recommendation; aggregate runs fixture |
| 3 | Closeout | PR/main CI and ADR/OpenSpec evidence |
| 4 | Operational | Next release installs successfully from published metadata |

## Alternatives Considered

- Latest-release discovery was rejected because it is mutable.
- Digest-only verification was rejected because the manifest itself needs an
  authenticated signer identity.
- Direct `mv` from the current directory was rejected because it exposes
  partial downloads and symlink hazards.

## Rollback Plan

Revert the installer through protected `main` and publish a new immutable tag;
never mutate an existing release tag. Failed verification leaves the prior
installation intact.
