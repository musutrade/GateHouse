# Design: Fail-Closed Trust Boundaries

## Invocation Input

An `InvocationInput` owns the effective project root, source mode, source
identity, configuration digest, and temporary snapshot lifetime. Working-tree,
base, and all modes retain their existing roots. Hook mode materializes the
complete Git index into a private temporary directory and reloads effective
configuration from that directory. Scope, built-in gates, architecture rules,
and ordinary external steps receive the effective project rooted there.

The original checkout remains available only to operations that explicitly
need repository metadata. A command cannot acquire working-tree semantics from
an inherited current directory. Invocation metadata records the mode, source
identity, and configuration digest.

## Resource Ownership

A lease filename is derived from its logical resource ID. Lease records bind
the canonical project identity, resource kind, invocation, expected labels,
runtime name, and immutable runtime object ID. After container creation, the
runtime adapter inspects the object and completes the lease identity.

Cleanup and stale reclaim inspect the current object immediately before any
remove operation. Removal is authorized only when the filename, record, full
label set, and immutable object ID agree. Expiry establishes eligibility, not
ownership. Ambiguity retains the object and emits a cleanup failure.

## Confined Publication

A shared publisher accepts an allowed root and relative destination. It rejects
absolute/traversal paths, symlink or non-directory parent components, and
symlink/non-regular destinations. It creates a unique sibling file with
create-new semantics, writes and synchronizes the content, and renames the
entry into place. Callers cannot fall back to direct writes after validation
fails.

On platforms where path-based rename cannot exclude a same-user concurrent
substitution, the documented trust model retains that limitation until a
directory-handle-relative primitive is available. Pre-existing target and
parent-component attacks are nevertheless rejected on every supported
platform.

## Closed Evidence

Each invocation has an artifact registry keyed by normalized relative path.
Nodes register required artifacts with invocation ID, optional step ID, and
kind. Finalization validates every registered entry as a confined regular file,
then compares the registered paths, report-declared paths, manifest paths, and
publishable on-disk paths. Explicit internal temporary files are the only
exclusions.

The machine result is first rendered with `evidence_complete = false`. After
redaction and closed-set validation, its final digest-bearing artifact list is
rendered, the manifest is generated, and the manifest is published last. A
validation or publication failure leaves the result failed and never emits a
successful complete-evidence claim.

## Release Inventory

The release build produces a deterministic JSON inventory containing every
platform binary and the CycloneDX SBOM. Release metadata generation consumes
that file and explicitly adds the checksum manifest, signatures,
certificates, and provenance references. Verification compares the inventory,
integrity subjects, and upload candidates before release creation.

Shell globs may enumerate files only inside the inventory generator; they are
not independently repeated by checksum, signing, attestation, verification, or
upload stages. Any missing or extra member stops publication.

## Rollback

Each boundary is delivered as a coherent commit and can be reverted with its
tests and OpenSpec task update. There is no compatibility fallback from staged
input to the working tree, from failed ownership proof to deletion, from
incomplete evidence to pass, or from inventory failure to best-effort upload.
Operators may retain ambiguous resources and incomplete invocation directories
for manual investigation.

## Implementation Plan and Timeline

| Phase | Scope | Exit evidence |
| --- | --- | --- |
| 1 | Invocation input | Opposing staged/working-tree architecture, fmt, and lint fixtures |
| 2 | Runtime ownership | Fake Docker/Podman inspection and no-remove contract tests |
| 3 | Publisher and evidence | Cross-platform symlink tests and closed-set artifact tests |
| 4 | Release inventory | Offline fixture proving exact coverage and tamper failure |
| 5 | Closeout | Full quality suite and Linux/macOS/Windows CI |
