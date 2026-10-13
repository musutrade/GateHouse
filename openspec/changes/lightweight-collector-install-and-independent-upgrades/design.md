# Design

## Existing dependencies (T1)
`install_collector.py` copies all six release assets into `.delivery`, extracts the tar, and on every select/rollback verifies signatures, all release subjects, installed hashes and tar file modes. `rust_collector_delivery.py` excludes the six controls from its exact runtime inventory. `collector_transport.py` caches compressed plugin/toolchain bundles and expanded objects but reconstructs a full tar for each install. The bootstrap pins its verifier and production uses both RSA and Sigstore. Release provenance, eligibility and SPDX remain mandatory. The existing runtime requires private regular files, exact compiler/LLVM byte identities and reviewed Core/protocol/ABI tuples; arbitrary PATH version matching is insufficient.

## Chosen approach
Retain signed controls and an archive recipe, replay the original tar hash from installed files without retaining its body. Check archive paths, sizes and modes as well as the signed manifest. Share only installer-owned, digest-and-mode-addressed regular files using hard links; never link user tools into the store. Inspect every installed byte before selection. Retained versions own references; garbage collection removes only unreferenced store objects while holding the lifecycle lock. Runtime paths remain private and unchanged. Copy exact compatible host bytes into the private store; independent compressed objects make missing-file downloads possible. The authenticated installer catalog pins the descriptor; the original signed release inventory remains the final authority.

## Transactions and migration
Prepare in staging, verify and self-check before atomic activation. Legacy installs continue to verify. Migration first verifies the complete old archive, writes a replay receipt, verifies its equivalence, then drops the redundant archive; interrupted migration accepts both metadata forms and can retry. No project or external toolchain cleanup. Explicit export reconstructs and verifies the full original release archive.

## Policy
No normative Engineering Policy delta: exact measurement and capture identities, release authority, baseline review, fail-closed capability reporting and licenses remain mandatory. Plugin delivery versions are independent of tool byte identities and Core identities; compatible project dependency updates use existing collection. A changed compiler/projection/series requires the existing reviewed series transition, never automatic baseline replacement.

## Alternatives
Deleting tar alone breaks mode and archive verification. Symlinks to PATH tools violate private-runtime capture identity. Comparing SemVer alone misses ABI and compiler-private identity. Reflinks are filesystem-specific; private hard links allow explicit reference accounting while full verification detects modification.

## Release asset limits
The protected installer catalog maps each content digest to an immutable component release with at most 900 objects. Publication checks every page of uploaded asset digests before publishing any draft; component releases become public before the installer entry. This avoids GitHub's 1,000-assets-per-release limit without bundling unrelated downloads. Offline kits retain the same flat digest filenames.
